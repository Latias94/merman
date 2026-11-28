//! Bounded decoding of Mermaid directive bodies.

#[cfg(feature = "yaml")]
use crate::yaml::grid_config;
#[cfg(not(feature = "yaml"))]
#[path = "../grid_config.rs"]
mod grid_config;
#[cfg(feature = "yaml-editor")]
mod json5_evidence;
mod json5_value;

#[cfg(feature = "yaml-editor")]
pub use json5_evidence::Json5ConfigKeyEvidence;

use super::{
    DirectiveArgument, DirectiveBlock, MAX_DIRECTIVE_CONFIG_PARSE_BYTES, ScannedDirective,
    normalize_directive_quotes_controlled, scan_directive_body,
};
use crate::scan::config::config_nesting_exceeds_limit_controlled;
use crate::scan::controlled::{ControlledScanCheckpoints, push_str_controlled};
use grid_config::GridPlacementsSource;
use merman_model::json::drop_value_nonrecursive;
use merman_model::{MAX_DIAGRAM_NESTING_DEPTH, OperationControl, OperationControlResult};
use serde_json::Value;
use std::borrow::Cow;
#[cfg(feature = "yaml-editor")]
use std::ops::Range;

/// A directive decoded from its raw body, with optional body-relative key evidence.
#[derive(Debug)]
pub struct DecodedDirective {
    pub ty: String,
    pub args: Option<Value>,
    /// Grid placement order and key ranges relative to the raw directive body.
    pub grid_placements: Option<GridPlacementsSource>,
    #[cfg(feature = "yaml-editor")]
    pub keyword_span: Range<usize>,
    #[cfg(feature = "yaml-editor")]
    pub config_keys: Vec<Json5ConfigKeyEvidence>,
    #[cfg(feature = "yaml-editor")]
    pub rewrite_safe: bool,
    #[cfg(feature = "yaml-editor")]
    pub arguments_complete: bool,
}

pub struct DirectiveCapture {
    pub directive: Option<DecodedDirective>,
    pub error: Option<String>,
}

/// Decodes one complete directive body using the existing bounded JSON5 parser.
pub fn decode_directive_controlled(
    raw: &str,
    control: &OperationControl,
) -> OperationControlResult<DirectiveCapture> {
    decode_directive_impl(
        raw,
        #[cfg(feature = "yaml-editor")]
        false,
        #[cfg(feature = "yaml-editor")]
        true,
        control,
    )
}

/// Also captures exact bounds and local JSON5 keys. An incomplete body yields only its keyword.
#[cfg(feature = "yaml-editor")]
pub fn decode_directive_with_evidence_controlled(
    raw: &str,
    complete: bool,
    control: &OperationControl,
) -> OperationControlResult<DirectiveCapture> {
    decode_directive_impl(raw, true, complete, control)
}

/// Copies the detector input with the given scanner-produced blocks removed.
///
/// # Panics
///
/// The blocks must belong to this input and have sorted, non-overlapping UTF-8 ranges.
pub fn remove_directive_blocks_controlled<'a>(
    input: &'a str,
    blocks: &[DirectiveBlock],
    control: &OperationControl,
) -> OperationControlResult<Cow<'a, str>> {
    if blocks.is_empty() {
        control.checkpoint()?;
        return Ok(Cow::Borrowed(input));
    }

    let mut retained_bytes = input.len();
    for (index, block) in blocks.iter().enumerate() {
        if index.is_multiple_of(32) {
            control.checkpoint()?;
        }
        retained_bytes = retained_bytes.saturating_sub(block.range.len());
    }
    let mut output = String::with_capacity(retained_bytes);
    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    let mut cursor = 0usize;
    for block in blocks {
        push_str_controlled(
            &mut output,
            &input[cursor..block.range.start],
            &mut checkpoints,
        )?;
        cursor = block.range.end;
    }
    push_str_controlled(&mut output, &input[cursor..], &mut checkpoints)?;
    checkpoints.finish()?;
    Ok(Cow::Owned(output))
}

fn decode_directive_impl(
    raw: &str,
    #[cfg(feature = "yaml-editor")] capture_keys: bool,
    #[cfg(feature = "yaml-editor")] parse_arguments: bool,
    control: &OperationControl,
) -> OperationControlResult<DirectiveCapture> {
    // Incomplete authoring evidence only needs the directive keyword. Avoid walking an
    // arbitrarily large unfinished argument merely to normalize quotes that will not be parsed.
    #[cfg(feature = "yaml-editor")]
    if !parse_arguments {
        return parse_directive_capture_controlled(raw, capture_keys, false, control);
    }
    let normalized = normalize_directive_quotes_controlled(raw, control)?;
    parse_directive_capture_controlled(
        normalized.as_ref(),
        #[cfg(feature = "yaml-editor")]
        capture_keys,
        #[cfg(feature = "yaml-editor")]
        true,
        control,
    )
}

fn parse_directive_capture_controlled(
    raw: &str,
    #[cfg(feature = "yaml-editor")] capture_keys: bool,
    #[cfg(feature = "yaml-editor")] parse_arguments: bool,
    control: &OperationControl,
) -> OperationControlResult<DirectiveCapture> {
    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    #[cfg(not(feature = "yaml-editor"))]
    let parse_arguments = true;
    let Some(ScannedDirective {
        keyword: ty,
        #[cfg(feature = "yaml-editor")]
        keyword_span,
        argument,
        ..
    }) = scan_directive_body(raw, parse_arguments, &mut checkpoints)?
    else {
        checkpoints.finish()?;
        return Ok(DirectiveCapture {
            directive: None,
            error: None,
        });
    };

    #[cfg(feature = "yaml-editor")]
    if !parse_arguments {
        checkpoints.finish()?;
        return Ok(DirectiveCapture {
            directive: Some(DecodedDirective {
                ty,
                args: None,
                grid_placements: None,
                keyword_span,
                config_keys: Vec::new(),
                rewrite_safe: false,
                arguments_complete: false,
            }),
            error: None,
        });
    }

    #[cfg(feature = "yaml-editor")]
    let mut config_keys = Vec::new();
    #[cfg(feature = "yaml-editor")]
    let mut rewrite_safe = true;
    #[cfg(feature = "yaml-editor")]
    let mut arguments_complete = true;
    let mut grid_placements = None;

    let args = match argument {
        Some(DirectiveArgument::Structured {
            source: rest,
            #[cfg(feature = "yaml-editor")]
                offset: rest_offset,
            ..
        }) => {
            if rest.len() > MAX_DIRECTIVE_CONFIG_PARSE_BYTES {
                return Ok(DirectiveCapture {
                    directive: Some(DecodedDirective {
                        ty,
                        args: None,
                        grid_placements: None,
                        #[cfg(feature = "yaml-editor")]
                        keyword_span,
                        #[cfg(feature = "yaml-editor")]
                        config_keys,
                        #[cfg(feature = "yaml-editor")]
                        rewrite_safe: false,
                        #[cfg(feature = "yaml-editor")]
                        arguments_complete: false,
                    }),
                    error: Some(format!(
                        "directive config exceeds the safe parser budget of {MAX_DIRECTIVE_CONFIG_PARSE_BYTES} bytes"
                    )),
                });
            }
            if config_nesting_exceeds_limit_controlled(rest, control)? {
                return Ok(DirectiveCapture {
                    directive: Some(DecodedDirective {
                        ty,
                        args: None,
                        grid_placements: None,
                        #[cfg(feature = "yaml-editor")]
                        keyword_span,
                        #[cfg(feature = "yaml-editor")]
                        config_keys,
                        #[cfg(feature = "yaml-editor")]
                        rewrite_safe: false,
                        #[cfg(feature = "yaml-editor")]
                        arguments_complete: false,
                    }),
                    error: Some(format!(
                        "config nesting exceeds {MAX_DIAGRAM_NESTING_DEPTH} levels"
                    )),
                });
            }
            checkpoints.finish()?;
            let captured = parse_directive_config_value_controlled(
                rest,
                raw,
                matches!(ty.as_str(), "init" | "initialize"),
                #[cfg(feature = "yaml-editor")]
                capture_keys,
                control,
            )?;
            grid_placements = captured.grid_placements;
            #[cfg(feature = "yaml-editor")]
            {
                arguments_complete = captured.value.is_some();
            }
            #[cfg(feature = "yaml-editor")]
            {
                rewrite_safe &= captured.rewrite_safe;
            }
            #[cfg(feature = "yaml-editor")]
            {
                config_keys = captured
                    .keys
                    .into_iter()
                    .map(|mut key| {
                        key.span = key.span.map(|span| {
                            rest_offset.saturating_add(span.start)
                                ..rest_offset.saturating_add(span.end)
                        });
                        key
                    })
                    .collect();
            }
            captured.value
        }
        Some(DirectiveArgument::Literal(value)) => {
            #[cfg(feature = "yaml-editor")]
            {
                rewrite_safe = false;
            }
            Some(Value::String(value))
        }
        None => None,
    };

    checkpoints.finish()?;
    Ok(DirectiveCapture {
        directive: Some(DecodedDirective {
            ty,
            args,
            grid_placements,
            #[cfg(feature = "yaml-editor")]
            keyword_span,
            #[cfg(feature = "yaml-editor")]
            config_keys,
            #[cfg(feature = "yaml-editor")]
            rewrite_safe,
            #[cfg(feature = "yaml-editor")]
            arguments_complete,
        }),
        error: None,
    })
}

struct DirectiveConfigCapture {
    value: Option<Value>,
    grid_placements: Option<GridPlacementsSource>,
    #[cfg(feature = "yaml-editor")]
    keys: Vec<Json5ConfigKeyEvidence>,
    #[cfg(feature = "yaml-editor")]
    rewrite_safe: bool,
}

fn parse_directive_config_value_controlled(
    input: &str,
    raw: &str,
    capture_grid: bool,
    #[cfg(feature = "yaml-editor")] capture_keys: bool,
    control: &OperationControl,
) -> OperationControlResult<DirectiveConfigCapture> {
    control.checkpoint()?;
    // `json5` has no cancellation hook. The caller enforces a hard input and nesting bound, so
    // this is a bounded atomic parser region rather than an unbounded cancellation gap.
    let mut captured = {
        #[cfg(feature = "yaml-editor")]
        {
            if capture_keys {
                let captured = json5_evidence::parse_json5_config(input, capture_grid);
                DirectiveConfigCapture {
                    value: captured.value,
                    grid_placements: captured.grid_placements,
                    keys: captured.keys,
                    #[cfg(feature = "yaml-editor")]
                    rewrite_safe: captured.rewrite_safe,
                }
            } else {
                let (value, grid_placements) = json5_value::parse_json5_value(input, capture_grid);
                DirectiveConfigCapture {
                    #[cfg(feature = "yaml-editor")]
                    rewrite_safe: value.is_some(),
                    value,
                    grid_placements,
                    keys: Vec::new(),
                }
            }
        }
        #[cfg(not(feature = "yaml-editor"))]
        {
            let (value, grid_placements) = json5_value::parse_json5_value(input, capture_grid);
            DirectiveConfigCapture {
                value,
                grid_placements,
            }
        }
    };
    let rebase_spans = (|| -> OperationControlResult<()> {
        control.checkpoint()?;
        if let Some(placements) = &mut captured.grid_placements {
            for (index, span) in placements.key_spans_mut().enumerate() {
                if index != 0 && index.is_multiple_of(64) {
                    control.checkpoint()?;
                }
                *span = span
                    .take()
                    .and_then(|range| json5_value::key_span_in(raw, range));
            }
        }
        Ok(())
    })();
    if let Err(cancelled) = rebase_spans {
        if let Some(value) = captured.value {
            drop_value_nonrecursive(value);
        }
        return Err(cancelled);
    }
    Ok(captured)
}

#[cfg(feature = "yaml-editor")]
mod order {
    use super::{MAX_DIRECTIVE_CONFIG_PARSE_BYTES, normalize_directive_quotes_controlled};
    use crate::config_order::{AllOrderedJson, MAX_CONFIG_REWRITE_DEPTH, clean_construct};
    use crate::scan::config::max_flow_collection_depth_controlled;
    use crate::scan::controlled::{ControlledScanCheckpoints, trim_whitespace_bounds_controlled};
    use merman_model::json::ordered::OrderedJson;
    use merman_model::{OperationControl, OperationControlResult};
    use serde_json::Value;

    pub fn read_ordered_init_directive_controlled(
        text: &str,
        max_nesting_depth: usize,
        control: &OperationControl,
    ) -> OperationControlResult<Option<OrderedJson>> {
        if text.len() > MAX_DIRECTIVE_CONFIG_PARSE_BYTES {
            return Ok(None);
        }
        let max_nesting_depth = max_nesting_depth.min(MAX_CONFIG_REWRITE_DEPTH);
        let cleaned = clean_construct(text, control)?;
        let Some(raw) = cleaned
            .text()
            .strip_prefix("%%{")
            .and_then(|s| s.strip_suffix("}%%"))
        else {
            return Ok(None);
        };
        let normalized = normalize_directive_quotes_controlled(raw, control)?;
        let mut checkpoints = ControlledScanCheckpoints::new(control)?;
        let Some((_, argument)) = normalized.split_once(':') else {
            return Ok(Some(Value::Null.into()));
        };
        let (start, end) = trim_whitespace_bounds_controlled(argument, &mut checkpoints)?;
        let argument = &argument[start..end];
        if argument.is_empty() {
            return Ok(Some(Value::Null.into()));
        }
        if max_flow_collection_depth_controlled::<true>(argument, &mut checkpoints)?
            > max_nesting_depth
        {
            return Ok(None);
        }
        checkpoints.finish()?;
        let order = json5::from_str::<AllOrderedJson>(argument)
            .ok()
            .map(|value| value.0);
        control.checkpoint()?;
        Ok(order)
    }
}

#[cfg(feature = "yaml-editor")]
pub use order::read_ordered_init_directive_controlled;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_json5_preserves_authored_order_original_slots_and_latest_key_origins() {
        let raw = r#" init : {grid: {placements: {
            'service.🚀[1]': [null, {row: NaN, column: -2, row: .5, '\u006cook': 'classic'}, [], {}],
            z: {row: 1}, a: {}, z: {column: 2}
        }}} "#;
        let check = |directive: DecodedDirective| {
            let args = directive.args.unwrap();
            let placements = directive.grid_placements.unwrap();
            assert_eq!(&raw[placements.key_span.unwrap()], "placements");
            assert_eq!(
                placements
                    .entries
                    .iter()
                    .map(|entry| entry.id.as_str())
                    .collect::<Vec<_>>(),
                ["service.🚀[1]", "z", "a"]
            );
            let entry = &placements.entries[0];
            assert_eq!(&raw[entry.key_span.clone().unwrap()], "service.🚀[1]");
            let slots = &entry.occurrences;
            assert_eq!(
                slots.iter().map(|slot| slot.index).collect::<Vec<_>>(),
                [Some(1), Some(3)]
            );
            assert!(slots[1].fields.is_empty());
            let fields = &slots[0].fields;
            assert_eq!(
                fields
                    .iter()
                    .map(|field| field.name.as_str())
                    .collect::<Vec<_>>(),
                ["row", "column", "look"]
            );
            assert_eq!(
                fields[0].key_span.as_ref().unwrap().start,
                raw.find("row: .5").unwrap()
            );
            assert_eq!(fields[0].nonfinite, None);
            assert_eq!(&raw[fields[1].key_span.clone().unwrap()], "column");
            assert!(fields[2].key_span.is_none());
            assert_eq!(args["grid"]["placements"][&entry.id][1]["row"], 0.5);
            assert!(args["grid"]["placements"][&entry.id][0].is_null());
            let replaced = &placements.entries[1];
            assert_eq!(
                replaced.key_span.as_ref().unwrap().start,
                raw.rfind("z:").unwrap()
            );
            assert_eq!(replaced.occurrences[0].index, None);
            assert_eq!(replaced.occurrences[0].fields[0].name, "column");
        };
        let control = OperationControl::new();
        check(
            decode_directive_controlled(raw, &control)
                .unwrap()
                .directive
                .unwrap(),
        );
        #[cfg(feature = "yaml-editor")]
        check(
            decode_directive_with_evidence_controlled(raw, true, &control)
                .unwrap()
                .directive
                .unwrap(),
        );
    }

    #[test]
    fn grid_json5_nonfinite_coordinates_keep_their_number_provenance() {
        let raw = "initialize: {grid: {placements: {service: [\
                   {row: NaN, column: -Infinity}, {column: +Infinity, row: 'NaN'}\
                   ]}}}";
        let check = |directive: DecodedDirective| {
            let args = directive.args.unwrap();
            let source = directive.grid_placements.unwrap();
            let slots = &source.entries[0].occurrences;
            assert!(slots[0].fields[0].nonfinite.unwrap().is_nan());
            assert_eq!(slots[0].fields[1].nonfinite, Some(f64::NEG_INFINITY));
            assert_eq!(slots[1].fields[0].nonfinite, Some(f64::INFINITY));
            assert_eq!(slots[1].fields[1].nonfinite, None);
            let placements = &args["grid"]["placements"]["service"];
            assert_eq!(
                placements,
                &serde_json::json!([{"row": null, "column": null}, {"column": null, "row": "NaN"}])
            );
        };
        let control = OperationControl::new();
        check(
            decode_directive_controlled(raw, &control)
                .unwrap()
                .directive
                .unwrap(),
        );
        #[cfg(feature = "yaml-editor")]
        {
            let directive = decode_directive_with_evidence_controlled(raw, true, &control)
                .unwrap()
                .directive
                .unwrap();
            assert!(!directive.rewrite_safe);
            check(directive);
        }
    }

    #[test]
    fn non_grid_numbers_and_failed_arguments_keep_their_previous_decode_policy() {
        let raw = "init: {grid: {placements: {A: {nested: {row: NaN}}}}}";
        let control = OperationControl::new();
        let directive = decode_directive_controlled(raw, &control)
            .unwrap()
            .directive
            .unwrap();
        assert!(directive.args.unwrap()["grid"]["placements"]["A"]["nested"]["row"].is_null());
        let source = directive.grid_placements.unwrap();
        assert_eq!(source.entries[0].occurrences[0].fields[0].nonfinite, None);
        #[cfg(feature = "yaml-editor")]
        {
            let editor = decode_directive_with_evidence_controlled(raw, true, &control)
                .unwrap()
                .directive
                .unwrap();
            assert!(editor.args.is_none());
            assert!(editor.grid_placements.is_none());
            assert!(!editor.arguments_complete);
        }
        let non_init =
            decode_directive_controlled("other: {grid: {placements: {A: {row: 1}}}}", &control)
                .unwrap()
                .directive
                .unwrap();
        assert!(non_init.args.is_some());
        assert!(non_init.grid_placements.is_none());
        let replaced = decode_directive_controlled(
            "init: {grid: {placements: {A: {row: 1}}}, grid: {rowGap: 20}}",
            &control,
        )
        .unwrap()
        .directive
        .unwrap();
        assert!(replaced.args.is_some());
        assert!(replaced.grid_placements.is_none());
        let malformed =
            decode_directive_controlled("init: {grid: {placements: {A: {row: 1}}}} {}", &control)
                .unwrap()
                .directive
                .unwrap();
        assert!(malformed.args.is_none());
        assert!(malformed.grid_placements.is_none());
    }

    #[cfg(feature = "yaml-editor")]
    #[test]
    fn directive_evidence_stays_relative_to_the_original_utf8_body() {
        let raw = r#"  init : { title: '🚀', 'theme': 'dark', '\u006cook': 'classic', items: [{ nested: true }] } "#;
        let control = OperationControl::new();
        let capture = decode_directive_with_evidence_controlled(raw, true, &control).unwrap();
        assert!(capture.error.is_none());
        let directive = capture.directive.unwrap();
        assert_eq!(&raw[directive.keyword_span], "init");
        assert_eq!(directive.args.unwrap()["look"], "classic");
        assert!(
            !directive
                .config_keys
                .iter()
                .any(|key| key.path.matches(&["items", "nested"]))
        );
        let theme = directive
            .config_keys
            .iter()
            .find(|key| key.path.matches(&["theme"]))
            .unwrap();
        assert_eq!(&raw[theme.span.clone().unwrap()], "theme");
        let escaped = directive
            .config_keys
            .iter()
            .find(|key| key.path.matches(&["look"]))
            .unwrap();
        assert!(escaped.span.is_none());
        assert!(!escaped.rewrite_safe);
    }

    #[test]
    fn directive_json5_decodes_literal_keys_and_preserves_numeric_and_duplicate_semantics() {
        let decode = |raw| {
            let capture = decode_directive_controlled(raw, &OperationControl::new()).unwrap();
            assert!(capture.error.is_none());
            capture.directive.expect("init directive")
        };
        let directive = decode(
            r#"init: {
                '$serde_json::private::RawValue': 'true',
                values: [/* JSON5 */ {
                    '\u0024serde_json::private::Number': '123',
                    hex: +0x10, fraction: .5, large: 18446744073709551615, nan: NaN,
                }],
                theme: 'dark',
                '$serde_json::private::RawValue': 'false',
            }"#,
        );
        let args = directive.args.expect("decoded argument");
        let args = args.as_object().unwrap();
        assert_eq!(args.len(), 3);
        assert_eq!(args["$serde_json::private::RawValue"], "false");
        assert_eq!(args["theme"], "dark");
        let values = args["values"].as_array().unwrap();
        assert_eq!(values.len(), 1);
        let value = values[0].as_object().unwrap();
        assert_eq!(value.len(), 5);
        assert_eq!(value["$serde_json::private::Number"], "123");
        assert_eq!(value["hex"], 16);
        assert_eq!(value["fraction"].as_f64(), Some(0.5));
        assert_eq!(value["large"].as_u64(), Some(u64::MAX));
        assert!(value["nan"].is_null());

        for raw in ["init: {theme: 'dark'} {}", "init: {theme: 'dark'} /*"] {
            let directive = decode(raw);
            assert!(directive.args.is_none(), "{raw}");
        }
    }
}

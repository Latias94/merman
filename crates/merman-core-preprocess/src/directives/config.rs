//! Bounded decoding of Mermaid directive bodies.

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
                #[cfg(feature = "yaml-editor")]
                capture_keys,
                control,
            )?;
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
    #[cfg(feature = "yaml-editor")]
    keys: Vec<Json5ConfigKeyEvidence>,
    #[cfg(feature = "yaml-editor")]
    rewrite_safe: bool,
}

fn parse_directive_config_value_controlled(
    input: &str,
    #[cfg(feature = "yaml-editor")] capture_keys: bool,
    control: &OperationControl,
) -> OperationControlResult<DirectiveConfigCapture> {
    control.checkpoint()?;
    // `json5` has no cancellation hook. The caller enforces a hard input and nesting bound, so
    // this is a bounded atomic parser region rather than an unbounded cancellation gap.
    let captured = {
        #[cfg(feature = "yaml-editor")]
        {
            if capture_keys {
                let captured = json5_evidence::parse_json5_config(input);
                DirectiveConfigCapture {
                    value: captured.value,
                    keys: captured.keys,
                    #[cfg(feature = "yaml-editor")]
                    rewrite_safe: captured.rewrite_safe,
                }
            } else {
                let value = json5_value::parse_json5_value(input);
                DirectiveConfigCapture {
                    #[cfg(feature = "yaml-editor")]
                    rewrite_safe: value.is_some(),
                    value,
                    keys: Vec::new(),
                }
            }
        }
        #[cfg(not(feature = "yaml-editor"))]
        {
            DirectiveConfigCapture {
                value: json5_value::parse_json5_value(input),
            }
        }
    };
    if let Err(cancelled) = control.checkpoint() {
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

//! Frontmatter decoding from fenced source to fields and construct-local evidence.

#[cfg(feature = "yaml-editor")]
use super::split_frontmatter_block_controlled;
use super::{
    FrontmatterBlockLocation, FrontmatterByteSpan, dedent_frontmatter_body_controlled,
    find_physical_line_ending_controlled, frontmatter_fields_from_yaml_value,
    frontmatter_has_prefix_controlled, locate_frontmatter_block_controlled,
};
#[cfg(feature = "yaml-editor")]
use crate::config_order::{
    JSON_ESCAPE_EXPANSION, MAX_CONFIG_REWRITE_DEPTH, clean_construct, ordered_yaml_json,
};
#[cfg(feature = "yaml-editor")]
use crate::directives::MAX_DIRECTIVE_CONFIG_PARSE_BYTES;
use crate::scan::controlled::ControlledScanCheckpoints;
use crate::yaml::GridPlacementsSource;
#[cfg(feature = "yaml-editor")]
use crate::yaml::{YamlConfigKeyEvidence, YamlInsertOrder, YamlInsertOrderRequest};
use crate::{MappedSource, SourceEdit};
use merman_model::SourceSpan;
use merman_model::config::MermaidConfig;
use merman_model::family::FAMILY_CATALOG;
#[cfg(feature = "yaml-editor")]
use merman_model::json::ordered::OrderedJson;
use merman_model::json::{clone_value_nonrecursive, drop_value_nonrecursive};
use merman_model::{MAX_DIAGRAM_NESTING_DEPTH, OperationControl, OperationControlResult};
use serde_json::{Map, Value};
use std::borrow::Cow;
use std::sync::OnceLock;

/// One frontmatter block. Ranges address the input to this reader, before outer source mapping.
pub struct DecodedFrontmatter<'a> {
    pub full: FrontmatterByteSpan,
    pub result: Result<FrontmatterFields, String>,
    /// Grid placement order and key ranges in this decoder's input.
    pub grid_placements: Option<GridPlacementsSource>,
    #[cfg(feature = "yaml-editor")]
    pub body: FrontmatterByteSpan,
    #[cfg(feature = "yaml-editor")]
    pub keys: Vec<YamlConfigKeyEvidence>,
    #[cfg(feature = "yaml-editor")]
    pub rewrite_safe: bool,
    #[cfg(feature = "yaml-editor")]
    pub insert_order: Option<YamlInsertOrder>,
    pub indent: &'a str,
}

pub struct FrontmatterFields {
    pub fields: Map<String, Value>,
    pub title: Option<String>,
    pub display_mode: Option<String>,
}

impl FrontmatterFields {
    pub fn config(&self) -> Option<&Value> {
        self.fields
            .get("config")
            .filter(|value| frontmatter_truthy(value))
    }

    /// Projects Merman's root-level compatibility fields, then merges Mermaid's `config` value.
    pub fn project_config_values(&self) -> MermaidConfig {
        let mut config = MermaidConfig::empty_object();
        for (namespace, value) in routed_frontmatter_values(|key| self.fields.get(key)) {
            config.set_value(namespace, clone_value_nonrecursive(value));
        }
        if let Some(value) = self.config() {
            config.deep_merge(value);
        }
        config
    }

    /// Projects authored order for root-level compatibility fields in an existing block.
    #[cfg(feature = "yaml-editor")]
    pub fn project_compatibility_order(fields: &OrderedJson) -> OrderedJson {
        let mut order = OrderedJson::from(Value::Object(Map::new()));
        let mut config = order.object_mut().expect("empty config object");
        let fields = fields.view();
        for (namespace, value) in routed_frontmatter_values(|key| fields.get(key)) {
            config.insert(namespace.to_owned(), value.to_ordered());
        }
        order
    }
}

/// Decodes a frontmatter block using the standard YAML limits.
pub fn decode_frontmatter_controlled<'a>(
    input: &'a str,
    control: &OperationControl,
) -> OperationControlResult<Option<DecodedFrontmatter<'a>>> {
    decode_frontmatter_impl(
        input,
        #[cfg(feature = "yaml-editor")]
        false,
        control,
    )
}

/// Also captures key ranges and insertion order from the same YAML parse.
#[cfg(feature = "yaml-editor")]
pub fn decode_frontmatter_with_evidence_controlled<'a>(
    input: &'a str,
    control: &OperationControl,
) -> OperationControlResult<Option<DecodedFrontmatter<'a>>> {
    decode_frontmatter_impl(input, true, control)
}

fn decode_frontmatter_impl<'a>(
    input: &'a str,
    #[cfg(feature = "yaml-editor")] capture_keys: bool,
    control: &OperationControl,
) -> OperationControlResult<Option<DecodedFrontmatter<'a>>> {
    control.checkpoint()?;
    let Some(location) = locate_frontmatter_block_controlled(input, control)? else {
        return Ok(None);
    };
    let body = &input[location.body.start..location.body.end];
    #[cfg(not(feature = "yaml-editor"))]
    let capture_keys = false;
    let yaml_input =
        dedented_frontmatter_yaml_input_controlled(body, location.indent, capture_keys, control)?;
    control.checkpoint()?;
    #[cfg(feature = "yaml-editor")]
    let (mut captured, insert_order) = {
        let insert_order_request = capture_keys.then(|| YamlInsertOrderRequest {
            routes: frontmatter_config_routes(),
            insert_key: "config",
            max_bytes: JSON_ESCAPE_EXPANSION * MAX_DIRECTIVE_CONFIG_PARSE_BYTES,
            max_depth: MAX_CONFIG_REWRITE_DEPTH,
        });
        crate::yaml::parse_yaml_frontmatter_capture_controlled(
            yaml_input.text(),
            MAX_DIAGRAM_NESTING_DEPTH,
            &["title", "displayMode"],
            insert_order_request,
            control,
        )?
    };
    #[cfg(not(feature = "yaml-editor"))]
    let mut captured = crate::yaml::parse_yaml_frontmatter_capture_controlled(
        yaml_input.text(),
        MAX_DIAGRAM_NESTING_DEPTH,
        &["title", "displayMode"],
        control,
    )?;
    #[cfg(feature = "yaml-editor")]
    let keys = captured
        .keys
        .into_iter()
        .map(|mut key| {
            // The outer source edit map is applied by the caller.
            let mapped = key.span.take().and_then(|span| {
                map_frontmatter_yaml_span_to_preprocessed_source(
                    &yaml_input,
                    location.body.start,
                    span,
                )
            });
            let (span, exact) = mapped.map_or((None, false), |(span, exact)| {
                (Some(span.start..span.end), exact)
            });
            key.span = span;
            key.rewrite_safe &= exact;
            key
        })
        .collect();

    let result = match frontmatter_fields_from_yaml_value(captured.value) {
        Ok(fields) => {
            if let Err(cancelled) = control.checkpoint() {
                drop_value_nonrecursive(Value::Object(fields));
                return Err(cancelled);
            }
            let title = frontmatter_text(
                fields.get("title"),
                captured.collection_text.remove("title"),
            );
            let display_mode = frontmatter_text(
                fields.get("displayMode"),
                captured.collection_text.remove("displayMode"),
            );
            Ok(FrontmatterFields {
                fields,
                title,
                display_mode,
            })
        }
        Err(message) => Err(message),
    };
    let mut grid_placements = captured.grid_placements;
    if let Some(placements) = &mut grid_placements
        && let Err(cancelled) =
            map_grid_placement_spans_controlled(placements, &yaml_input, body, location, control)
    {
        if let Ok(fields) = result {
            drop_value_nonrecursive(Value::Object(fields.fields));
        }
        return Err(cancelled);
    }
    Ok(Some(DecodedFrontmatter {
        full: location.full,
        #[cfg(feature = "yaml-editor")]
        body: location.body,
        indent: location.indent,
        #[cfg(feature = "yaml-editor")]
        keys,
        #[cfg(feature = "yaml-editor")]
        rewrite_safe: captured.rewrite_safe,
        #[cfg(feature = "yaml-editor")]
        insert_order,
        grid_placements,
        result,
    }))
}

fn routed_frontmatter_values<T>(
    lookup: impl Fn(&str) -> Option<T>,
) -> impl Iterator<Item = (&'static str, T)> {
    frontmatter_config_routes()
        .iter()
        .filter_map(move |&(key, namespace)| lookup(key).map(|value| (namespace, value)))
}

/// Root-level compatibility keys in assignment order, before the explicit `config` merge.
pub fn frontmatter_config_routes() -> &'static [(&'static str, &'static str)] {
    static ROUTES: OnceLock<Vec<(&'static str, &'static str)>> = OnceLock::new();
    ROUTES.get_or_init(|| {
        let mut aliases: Vec<_> = FAMILY_CATALOG
            .iter()
            .flat_map(|family| {
                family.variants.iter().filter_map(|variant| {
                    variant.frontmatter_alias_order.map(|order| {
                        let namespace = family
                            .config
                            .expect("config aliases require a family namespace")
                            .namespace;
                        (order, (variant.id, namespace))
                    })
                })
            })
            .collect();
        aliases.sort_by_key(|(order, _)| *order);
        aliases
            .into_iter()
            .map(|(_, route)| route)
            .chain(
                frontmatter_config_namespaces()
                    .iter()
                    .map(|&key| (key, key)),
            )
            .collect()
    })
}

pub fn frontmatter_config_namespaces() -> &'static [&'static str] {
    static NAMESPACES: OnceLock<Vec<&'static str>> = OnceLock::new();
    NAMESPACES
        .get_or_init(|| {
            let mut namespaces: Vec<_> = FAMILY_CATALOG
                .iter()
                .filter_map(|family| {
                    family
                        .config
                        .map(|config| (config.frontmatter_order, config.namespace))
                })
                .collect();
            namespaces.sort_by_key(|(order, _)| *order);
            namespaces
                .into_iter()
                .map(|(_, namespace)| namespace)
                .collect()
        })
        .as_slice()
}

enum FrontmatterYamlInput<'a> {
    Plain(Cow<'a, str>),
    Mapped(Box<MappedSource>),
}

impl FrontmatterYamlInput<'_> {
    fn text(&self) -> &str {
        match self {
            Self::Plain(text) => text.as_ref(),
            Self::Mapped(source) => source.text(),
        }
    }

    fn try_map_span_to_body(&self, span: SourceSpan) -> Option<SourceSpan> {
        match self {
            Self::Plain(_) => Some(span),
            Self::Mapped(source) => source.try_map_span(span),
        }
    }

    #[cfg(feature = "yaml-editor")]
    fn try_map_enclosing_span_to_body(&self, span: SourceSpan) -> Option<SourceSpan> {
        match self {
            Self::Plain(_) => Some(span),
            Self::Mapped(source) => source.try_map_enclosing_span(span),
        }
    }
}

#[cfg(feature = "yaml-editor")]
fn map_frontmatter_yaml_span_to_preprocessed_source(
    yaml_input: &FrontmatterYamlInput<'_>,
    body_start: usize,
    span: std::ops::Range<usize>,
) -> Option<(SourceSpan, bool)> {
    let parser_span = SourceSpan::new(span.start, span.end);
    let exact = yaml_input.try_map_span_to_body(parser_span);
    let body_span = exact.or_else(|| yaml_input.try_map_enclosing_span_to_body(parser_span))?;
    Some((
        SourceSpan::new(
            body_start.checked_add(body_span.start)?,
            body_start.checked_add(body_span.end)?,
        ),
        exact.is_some(),
    ))
}

fn map_grid_placement_spans_controlled(
    placements: &mut GridPlacementsSource,
    yaml_input: &FrontmatterYamlInput<'_>,
    body: &str,
    location: FrontmatterBlockLocation<'_>,
    control: &OperationControl,
) -> OperationControlResult<()> {
    // Plain decoding dedents without a source map. Reuse the controlled dedent mapping for
    // indented Grid frontmatter after YAML has succeeded.
    let mapped =
        if !location.indent.is_empty() && matches!(yaml_input, FrontmatterYamlInput::Plain(_)) {
            Some(dedented_frontmatter_yaml_input_controlled(
                body,
                location.indent,
                true,
                control,
            )?)
        } else {
            None
        };
    let source = mapped.as_ref().unwrap_or(yaml_input);
    for (index, span) in placements.key_spans_mut().enumerate() {
        if index.is_multiple_of(64) {
            control.checkpoint()?;
        }
        *span = span.take().and_then(|range| {
            let span = source.try_map_span_to_body(SourceSpan::new(range.start, range.end))?;
            Some(
                location.body.start.checked_add(span.start)?
                    ..location.body.start.checked_add(span.end)?,
            )
        });
    }
    Ok(())
}

fn dedented_frontmatter_yaml_input_controlled<'a>(
    body: &'a str,
    indent: &str,
    capture_mapping: bool,
    control: &OperationControl,
) -> OperationControlResult<FrontmatterYamlInput<'a>> {
    if !capture_mapping {
        return dedent_frontmatter_body_controlled(body, indent, control)
            .map(FrontmatterYamlInput::Plain);
    }
    if indent.is_empty() {
        control.checkpoint()?;
        return Ok(FrontmatterYamlInput::Plain(Cow::Borrowed(body)));
    }

    let mut source = MappedSource::new_controlled(body, true, control)?;
    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    let mut edits = Vec::new();
    let mut line_start = 0usize;
    loop {
        let line_ending = find_physical_line_ending_controlled(body, line_start, &mut checkpoints)?;
        let (line_end, next_line_start) = line_ending.unwrap_or((body.len(), body.len()));
        let line = &body[line_start..line_end];
        if frontmatter_has_prefix_controlled(line, indent, &mut checkpoints)? {
            edits.push(SourceEdit::delete(line_start..line_start + indent.len()));
        }
        if line_ending.is_none() || next_line_start == body.len() {
            break;
        }
        line_start = next_line_start;
    }
    checkpoints.finish()?;
    source.apply_edits(edits, control)?;
    Ok(FrontmatterYamlInput::Mapped(Box::new(source)))
}

pub fn parse_frontmatter_yaml_fields(input: &str) -> Result<Map<String, Value>, String> {
    let control = OperationControl::new();
    parse_frontmatter_yaml_fields_controlled(input, &control)
        .expect("a private parse control cannot be cancelled")
}

/// Parses frontmatter YAML fields while observing cooperative cancellation.
pub fn parse_frontmatter_yaml_fields_controlled(
    input: &str,
    control: &OperationControl,
) -> OperationControlResult<Result<Map<String, Value>, String>> {
    let parsed =
        crate::yaml::parse_yaml_value_controlled(input, MAX_DIAGRAM_NESTING_DEPTH, control)?;
    Ok(frontmatter_fields_from_yaml_value(parsed))
}

fn frontmatter_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

fn frontmatter_text(value: Option<&Value>, collection: Option<String>) -> Option<String> {
    match value.filter(|value| frontmatter_truthy(value))? {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null => None,
        Value::Array(_) | Value::Object(_) => collection,
    }
}

/// Replays one already-selected frontmatter span for authored order. An empty block yields
/// `Some(None)`. The caller supplies the advisory byte and depth budgets.
#[cfg(feature = "yaml-editor")]
pub fn read_ordered_frontmatter_controlled(
    text: &str,
    max_bytes: usize,
    max_nesting_depth: usize,
    control: &OperationControl,
) -> OperationControlResult<Option<Option<OrderedJson>>> {
    let max_bytes = max_bytes.min(MAX_DIRECTIVE_CONFIG_PARSE_BYTES);
    if text.len() > max_bytes {
        return Ok(None);
    }
    let max_nesting_depth = max_nesting_depth.min(MAX_CONFIG_REWRITE_DEPTH);
    let cleaned = clean_construct(text, control)?;
    let Some(block) = split_frontmatter_block_controlled(cleaned.text(), control)? else {
        return Ok(None);
    };
    let Some(json) = crate::yaml::parse_yaml_ordered_document_controlled(
        &block.dedented_body,
        max_bytes,
        max_nesting_depth,
        control,
    )?
    else {
        return Ok(None);
    };
    Ok(if json.is_empty() {
        Some(None)
    } else {
        ordered_yaml_json(&json).map(Some)
    })
}

/// Reads the bounded compatibility-order JSON captured for a new `config` field.
#[cfg(feature = "yaml-editor")]
pub fn parse_frontmatter_insert_order_json(json: &str) -> Option<OrderedJson> {
    if json.len() > JSON_ESCAPE_EXPANSION * MAX_DIRECTIVE_CONFIG_PARSE_BYTES {
        return None;
    }
    ordered_yaml_json(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn frontmatter_value_projection_replaces_aliases_before_merging_explicit_config() {
        let input = concat!(
            "---\n",
            "flowchart: {keep: canonical, items: [a, {z: 1}]}\n",
            "flowchart-elk: {elkOnly: true}\n",
            "flowchart-v2: {v2Only: true}\n",
            "config:\n",
            "  flowchart: {items: [a, {z: 1}, b], curve: linear}\n",
            "  fontFamily: Test Font\n",
            "  gantt: {displayMode: configured}\n",
            "displayMode: root\n",
            "---\n",
        );
        let fields = decode_frontmatter_controlled(input, &OperationControl::new())
            .unwrap()
            .unwrap()
            .result
            .unwrap();
        assert_eq!(
            fields.project_config_values().as_value(),
            &json!({
                "flowchart": {"keep": "canonical", "items": ["a", {"z": 1}, {"z": 1}, "b"], "curve": "linear"},
                "fontFamily": "Test Font",
                "gantt": {"displayMode": "configured"},
            })
        );
        assert_eq!(fields.display_mode.as_deref(), Some("root"));
        assert_eq!(fields.fields["flowchart"]["items"], json!(["a", {"z": 1}]));
    }

    #[cfg(feature = "yaml-editor")]
    #[test]
    fn frontmatter_order_projection_keeps_winning_members_in_authored_order() {
        let input = concat!(
            "---\n",
            "sequence: {actorMargin: 12}\n",
            "flowchart: {z: true, a: false}\n",
            "flowchart-elk: {elkOnly: true}\n",
            "flowchart-v2: {v2Only: true}\n",
            "config: {flowchart: {fromConfig: true}, theme: dark}\n",
            "---\n",
        );
        let fields = read_ordered_frontmatter_controlled(
            input,
            MAX_DIRECTIVE_CONFIG_PARSE_BYTES,
            MAX_CONFIG_REWRITE_DEPTH,
            &OperationControl::new(),
        )
        .unwrap()
        .flatten()
        .unwrap();
        let order = FrontmatterFields::project_compatibility_order(&fields);
        assert_eq!(
            serde_json::to_string(&order.view()).unwrap(),
            r#"{"flowchart":{"z":true,"a":false},"sequence":{"actorMargin":12}}"#
        );
        assert_eq!(fields.value["config"]["theme"], "dark");
    }

    #[test]
    fn grid_frontmatter_keeps_order_slots_and_indented_utf8_source_ranges() {
        let input = concat!(
            "\u{2003}---\r\n",
            "\u{2003}title: 🚀\r\n",
            "\u{2003}grid: { placements: { outside: {} } }\r\n",
            "\u{2003}config:\r\n",
            "\u{2003}  grid:\r\n",
            "\u{2003}    placements:\r\n",
            "\u{2003}      'z.🚀[1]': [null, { column: 0.5, 'row': -4 }, [], {}]\r\n",
            "\u{2003}      a: { row: 3, column: 2 }\r\n",
            "\u{2003}---\r\n",
            "flowchart LR\r\n"
        );
        let check = |decoded: DecodedFrontmatter<'_>| {
            let result = decoded.result.ok().unwrap();
            let placements = decoded.grid_placements.unwrap();
            assert_eq!(
                placements.key_span.unwrap().start,
                input.rfind("placements:").unwrap()
            );
            assert_eq!(
                placements
                    .entries
                    .iter()
                    .map(|entry| entry.id.as_str())
                    .collect::<Vec<_>>(),
                ["z.🚀[1]", "a"]
            );
            let entry = &placements.entries[0];
            assert_eq!(&input[entry.key_span.clone().unwrap()], "z.🚀[1]");
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
                ["column", "row"]
            );
            assert_eq!(&input[fields[0].key_span.clone().unwrap()], "column");
            assert_eq!(&input[fields[1].key_span.clone().unwrap()], "row");
            assert_eq!(
                fields[1].key_span.as_ref().unwrap().start,
                input.find("'row'").unwrap() + 1
            );
            let values = &result.config().unwrap()["grid"]["placements"][&entry.id];
            assert_eq!(values[1]["column"], 0.5);
            assert!(values[0].is_null());
            assert_eq!(placements.entries[1].occurrences[0].index, None);
        };
        let control = OperationControl::new();
        check(
            decode_frontmatter_controlled(input, &control)
                .unwrap()
                .unwrap(),
        );
        #[cfg(feature = "yaml-editor")]
        check(
            decode_frontmatter_with_evidence_controlled(input, &control)
                .unwrap()
                .unwrap(),
        );
    }

    #[test]
    fn grid_frontmatter_alias_keeps_order_with_the_local_id_as_span_fallback() {
        let input = concat!(
            "---\n",
            "template: &placement { column: 1, row: 2 }\n",
            "config: {grid: {placements: {service: *placement}}}\n",
            "---\n",
            "flowchart LR\n"
        );
        let decoded = decode_frontmatter_controlled(input, &OperationControl::new())
            .unwrap()
            .unwrap();
        let entry = &decoded.grid_placements.unwrap().entries[0];
        assert_eq!(&input[entry.key_span.clone().unwrap()], "service");
        let fields = &entry.occurrences[0].fields;
        assert_eq!(
            fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>(),
            ["column", "row"]
        );
        assert!(fields.iter().all(|field| field.key_span.is_none()));
        assert_eq!(
            decoded.result.ok().unwrap().config().unwrap()["grid"]["placements"]["service"]["row"],
            2
        );
    }

    #[test]
    fn grid_frontmatter_errors_publish_no_order_or_spans() {
        let raw = "---\nconfig: {grid: {placements: {A: {row: 1, row: 2}}}}\n---\nflowchart LR\n";
        let decoded = decode_frontmatter_controlled(raw, &OperationControl::new())
            .unwrap()
            .unwrap();
        assert!(decoded.result.is_err());
        assert!(decoded.grid_placements.is_none());
    }

    #[cfg(feature = "yaml-editor")]
    #[test]
    fn indented_frontmatter_keys_address_the_original_utf8_bytes() {
        let input = concat!(
            "\u{2003}---\r\n",
            "\u{2003}title: [🚀, café]\r\n",
            "\u{2003}config:\r\n",
            "\u{2003}  café: true\r\n",
            "\u{2003}---\r\n",
            "sequenceDiagram\r\n",
        );
        let control = OperationControl::new();
        let plain = decode_frontmatter_controlled(input, &control)
            .unwrap()
            .unwrap();
        let rich = decode_frontmatter_with_evidence_controlled(input, &control)
            .unwrap()
            .unwrap();
        assert!(plain.keys.is_empty());
        assert_eq!(plain.full, rich.full);
        assert_eq!(
            plain.result.unwrap().fields,
            rich.result.as_ref().unwrap().fields
        );
        assert_eq!(
            rich.result.unwrap().title.as_deref(),
            Some(r#"["🚀","café"]"#)
        );
        let key = rich
            .keys
            .iter()
            .find(|key| key.path.matches(&["config", "café"]))
            .unwrap();
        let span = key.span.clone().unwrap();
        assert_eq!(&input[span.clone()], "café");
        assert_eq!(span.start, input.rfind("café").unwrap());
        assert!(key.rewrite_safe);
    }
}

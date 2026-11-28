//! Admission for decoded source `grid.placements` from Mermaid 12.4.0 `gridPlacement.ts`.

mod source;

use crate::{Error, MermaidConfig, OperationControl, OperationControlResult, ParseDiagnostic};
use merman_core_preprocess::grid_coordinate::{
    GridCoordinateError, GridCoordinateField, GridCoordinateValue, validate_grid_coordinate,
};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub(in crate::preprocess) use source::{GridSource, merge_source_grid};

pub(in crate::preprocess) fn sanitize_section(
    config: &mut Value,
    source: &GridSource,
    control: &OperationControl,
) -> OperationControlResult<crate::Result<()>> {
    let Some(placements) = config
        .get_mut("grid")
        .and_then(|grid| grid.get_mut("placements"))
    else {
        return Ok(Ok(()));
    };
    match sanitize_source_grid_placements(placements, source, control)? {
        Ok(Some(value)) => crate::config::replace_value_nonrecursive(placements, value),
        Ok(None) => {}
        Err(error) => {
            let diagnostic = source.attach_span(coordinate_diagnostic(&error), &error);
            return Ok(Err(Error::SourceConfig { diagnostic }));
        }
    }
    Ok(Ok(()))
}

fn coordinate_diagnostic(error: &GridCoordinateError<'_>) -> ParseDiagnostic {
    ParseDiagnostic::new(error.to_string()).with_code(GridCoordinateError::CODE)
}

pub(crate) fn validate_grid_for_parser<T>(
    parsed: crate::Result<T>,
    config: &MermaidConfig,
    diagram_type: &str,
    builtin: bool,
    control: &OperationControl,
) -> OperationControlResult<crate::Result<T>> {
    use merman_model::family::{DiagramFamilyId, diagram_type_family_id};

    let Ok(model) = parsed else { return Ok(parsed) };
    // The Flow family parsers admit the map at their DB boundary before inline validation.
    // A custom parser replacing a built-in parser still uses this map check.
    if config.get_str("layout") != Some("grid")
        || (builtin
            && matches!(
                diagram_type_family_id(diagram_type),
                Some(
                    DiagramFamilyId::FLOWCHART
                        | DiagramFamilyId::SWIMLANE
                        | DiagramFamilyId::AGENTFLOW
                )
            ))
    {
        return Ok(Ok(model));
    }
    Ok(validate_configured_grid(config, diagram_type, control)?.map(|()| model))
}

pub(crate) fn validate_configured_grid(
    config: &MermaidConfig,
    diagram_type: &str,
    control: &OperationControl,
) -> OperationControlResult<crate::Result<()>> {
    if config.get_str("layout") != Some("grid") {
        return Ok(Ok(()));
    }
    let Some(placements) = config
        .get_path(&["grid", "placements"])
        .and_then(Value::as_object)
    else {
        return Ok(Ok(()));
    };
    for (id, entry) in ordered_entries(placements, &GridSource::default(), control)? {
        for (index, placement) in placement_occurrences(entry) {
            control.checkpoint()?;
            let Some(placement) = placement.as_object() else {
                continue;
            };
            for field in [GridCoordinateField::Row, GridCoordinateField::Column] {
                if let Err(error) = validate_grid_coordinate(
                    id,
                    index,
                    field,
                    placement.get(field.as_str()).map(GridCoordinateValue::from),
                ) {
                    return Ok(Err(Error::diagram_parse_diagnostic(
                        diagram_type,
                        coordinate_diagnostic(&error),
                    )));
                }
            }
        }
    }
    control.checkpoint()?;
    Ok(Ok(()))
}

/// Filters a borrowed source placement dictionary into a new value. Only retained keys and
/// scalar fields are copied. `None` leaves a non-container to the surrounding source policy.
fn sanitize_source_grid_placements<'a>(
    placements: &'a Value,
    source: &GridSource,
    control: &OperationControl,
) -> OperationControlResult<Result<Option<Value>, GridCoordinateError<'a>>> {
    control.checkpoint()?;
    let placements = match placements {
        Value::Array(_) => return Ok(Ok(Some(Value::Array(Vec::new())))),
        Value::Object(placements) => placements,
        _ => return Ok(Ok(None)),
    };

    let mut output = Map::new();
    'ids: for (id, entry) in ordered_entries(placements, source, control)? {
        control.checkpoint()?;
        if matches!(id, "__proto__" | "constructor" | "prototype") {
            continue;
        }
        let sanitized = match entry {
            Value::Object(item) => match sanitize_placement(id, None, item, source, control)? {
                Ok(value) => value,
                Err(error) => return Ok(Err(error)),
            },
            Value::Array(items) if !items.is_empty() => {
                // A malformed slot drops its entire ID before any coordinate is checked.
                for item in items {
                    control.checkpoint()?;
                    if !item.is_object() {
                        continue 'ids;
                    }
                }
                let mut sanitized = Vec::with_capacity(items.len());
                for (index, item) in items.iter().enumerate() {
                    let item = item.as_object().expect("all slots are objects");
                    match sanitize_placement(id, Some(index), item, source, control)? {
                        Ok(value) => sanitized.push(value),
                        Err(error) => return Ok(Err(error)),
                    }
                }
                Value::Array(sanitized)
            }
            _ => continue,
        };
        output.insert(id.to_owned(), sanitized);
    }
    control.checkpoint()?;
    Ok(Ok(Some(Value::Object(output))))
}

fn ordered_entries<'a>(
    placements: &'a Map<String, Value>,
    source: &GridSource,
    control: &OperationControl,
) -> OperationControlResult<impl Iterator<Item = (&'a str, &'a Value)> + use<'a>> {
    // Object.keys visits canonical array-index IDs numerically before other authored IDs.
    let mut ordered = BTreeMap::new();
    for (id, entry) in placements {
        control.checkpoint()?;
        let order = match js_array_index(id) {
            Some(index) => (false, index as usize),
            None => (true, source.entry_order(id).unwrap_or(usize::MAX)),
        };
        ordered.insert((order, id.as_str()), entry);
    }
    Ok(ordered.into_iter().map(|((_, id), value)| (id, value)))
}

fn js_array_index(id: &str) -> Option<u32> {
    if id != "0" && !matches!(id.as_bytes().first(), Some(b'1'..=b'9')) {
        return None;
    }
    id.parse::<u32>().ok().filter(|&index| index < u32::MAX)
}

fn placement_occurrences(entry: &Value) -> impl Iterator<Item = (Option<usize>, &Value)> {
    let items = entry.as_array();
    items.is_none().then_some((None, entry)).into_iter().chain(
        items
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(index, item)| (Some(index), item)),
    )
}

fn sanitize_placement<'a>(
    id: &'a str,
    index: Option<usize>,
    placement: &'a Map<String, Value>,
    source: &GridSource,
    control: &OperationControl,
) -> OperationControlResult<Result<Value, GridCoordinateError<'a>>> {
    control.checkpoint()?;
    let mut output = Map::new();
    for (field, nonfinite) in source.coordinate_order(id, index) {
        control.checkpoint()?;
        let Some(value) = placement.get(field.as_str()) else {
            continue;
        };
        let coordinate = nonfinite.map_or_else(|| value.into(), GridCoordinateValue::Number);
        if let Err(error) = validate_grid_coordinate(id, index, field, Some(coordinate)) {
            return Ok(Err(error));
        }
        output.insert(field.as_str().to_owned(), value.clone());
    }
    for (key, accepted) in [
        ("horizontalAlign", ["left", "center", "right"]),
        ("verticalAlign", ["top", "center", "bottom"]),
    ] {
        if let Some(value) = placement
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| accepted.contains(value))
        {
            output.insert(key.to_owned(), Value::String(value.to_owned()));
        }
    }
    Ok(Ok(Value::Object(output)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SourceSpan;
    use merman_core_preprocess::directives::decode_directive_controlled;
    use merman_model::OperationPhase;
    use serde_json::json;

    #[test]
    fn source_keeps_literal_ids_and_filters_the_fixed_placement_shape() {
        let input = json!({
            "constructor": {"row": null},
            "prototype": {"row": null},
            "__proto__": {"row": null},
            "protocolGateway": {"row": 0, "column": -2, "theme": "dark", "horizontalAlign": "left"},
            "__proto__hack": {"horizontalAlign": "bottom", "verticalAlign": "center"},
            "prototypeA": {"verticalAlign": "top"},
            "constructorNode": {"horizontalAlign": "center"},
            "constraintSolver": {"nested": {"row": null}},
            "group.with[1]/💡": [{"row": -3, "horizontalAlign": "right", "verticalAlign": "bottom"}, {}],
            "": {},
            "scalar": true,
            "null": null,
        });
        assert_eq!(
            sanitize(&input),
            Some(json!({
                "protocolGateway": {"row": 0, "column": -2, "horizontalAlign": "left"},
                "__proto__hack": {"verticalAlign": "center"},
                "prototypeA": {"verticalAlign": "top"},
                "constructorNode": {"horizontalAlign": "center"},
                "constraintSolver": {},
                "group.with[1]/💡": [{"row": -3, "horizontalAlign": "right", "verticalAlign": "bottom"}, {}],
                "": {},
            }))
        );
    }

    #[test]
    fn malformed_occurrence_lists_are_dropped_before_reading_their_coordinates() {
        let input = json!({
            "empty": [],
            "null-later": [{"row": 0.5}, null],
            "array-first": [[], {"column": "1"}],
            "scalar-last": [{"row": true}, "bad"],
            "valid": [{}, {"column": 2}],
        });
        assert_eq!(
            sanitize(&input),
            Some(json!({"valid": [{}, {"column": 2}]}))
        );
    }

    #[test]
    fn placements_array_is_emptied_and_scalars_are_left_to_the_caller() {
        assert_eq!(sanitize(&json!([{"row": null}])), Some(json!([])));
        assert_eq!(sanitize(&json!("unchanged")), None);
        assert_eq!(sanitize(&Value::Null), None);
    }

    #[test]
    fn source_field_order_and_occurrence_indexes_determine_the_first_error() {
        for (object, field) in [
            (r#"{"row":null,"column":"1"}"#, GridCoordinateField::Row),
            (r#"{"column":"1","row":null}"#, GridCoordinateField::Column),
        ] {
            let (placements, source) =
                decoded(&format!(r#"{{"service":[{{}},{object},{{"row":true}}]}}"#));
            let error =
                sanitize_source_grid_placements(&placements, &source, &OperationControl::new())
                    .unwrap()
                    .unwrap_err();
            assert_eq!(error.placement_id, "service");
            assert_eq!(error.occurrence_index, Some(1));
            assert_eq!(error.field, field);
        }
    }

    #[test]
    fn source_ids_follow_javascript_index_order_then_authored_order() {
        for (text, expected_id) in [
            (
                r#"{"z":{"row":null},"10":{"row":null},"2":{"row":null}}"#,
                "2",
            ),
            (
                r#"{"4294967295":{"row":null},"4294967294":{"row":null}}"#,
                "4294967294",
            ),
            (
                r#"{"z":{"row":null},"01":{"row":null},"+1":{"row":null},"-0":{"row":null},"a":{"row":null}}"#,
                "z",
            ),
        ] {
            let (placements, source) = decoded(text);
            let error =
                sanitize_source_grid_placements(&placements, &source, &OperationControl::new())
                    .unwrap()
                    .unwrap_err();
            assert_eq!(error.placement_id, expected_id);
            assert_eq!(error.occurrence_index, None);
        }
    }

    #[test]
    fn cancellation_uses_the_callers_phase_and_has_no_partial_value() {
        let input = json!({"service": [{"row": 1}, {"row": null}]});
        let control = OperationControl::new().for_phase(OperationPhase::Parse);
        control.cancel_after_checkpoints(4);
        let cancellation =
            sanitize_source_grid_placements(&input, &GridSource::default(), &control).unwrap_err();
        assert_eq!(cancellation.phase, OperationPhase::Parse);
    }

    fn sanitize(placements: &Value) -> Option<Value> {
        sanitize_source_grid_placements(
            placements,
            &GridSource::default(),
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap()
    }

    fn decoded(text: &str) -> (Value, GridSource) {
        use crate::preprocess::PreprocessedSource;
        let raw = format!("init: {{grid: {{placements: {text}}}}}");
        let control = OperationControl::new();
        let source = PreprocessedSource::new_controlled(&raw, false, &control).unwrap();
        let decoded = decode_directive_controlled(&raw, &control)
            .unwrap()
            .directive
            .unwrap();
        let origins = GridSource::capture(
            &source,
            decoded.grid_placements,
            0,
            SourceSpan::new(0, raw.len()),
            &control,
        )
        .unwrap();
        (decoded.args.unwrap()["grid"]["placements"].clone(), origins)
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn source_arrays_merge_by_slot_and_init_directives_replace_by_id() {
        let parser = crate::SemanticParser::new();
        let frontmatter = "---\r\nconfig:\r\n  grid:\r\n    placements:\r\n      service: [{row: 1, column: -2}, {row: -3}]\r\n---\r\n";
        let init = "%%{init: {\"grid\": {\"placements\": {\"service\": [{\"row\": 5}]}}}}%%\n";
        let body = "flowchart LR\nservice --> target\n";
        let source = format!("{frontmatter}{init}{body}");
        let result = parser.parse_diagram_sync(&source).unwrap();
        assert_eq!(
            result.metadata.config.as_value()["grid"]["placements"]["service"],
            json!([
                {"row": 5, "column": -2}, {"row": -3},
            ])
        );

        let prior = "%%{init: {\"grid\": {\"placements\": {\"service\": [{\"column\": 3}, {\"row\": 7}], \"spare\": {\"row\": 1}}}}}%%\n";
        let result = parser
            .parse_diagram_sync(&format!("{prior}{init}{body}"))
            .unwrap();
        assert_eq!(
            result.metadata.config.as_value()["grid"]["placements"],
            json!({
                "service": [{"row": 5}], "spare": {"row": 1},
            })
        );
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn grid_diagnostics_keep_the_winning_field_and_original_byte_span() {
        let source = concat!(
            "\u{feff}---\r\nconfig:\r\n  grid:\r\n    placements:\r\n      'a.💡[1]': [{row: 1, column: -2}, {}]\r\n---\r\n",
            "%%{init: {grid: {placements: {'a.💡[1]': [{column: NaN, row: -1}]}}}}%%\r\n",
            "flowchart LR\nA\n",
        );
        let assert_error = |error: Error| {
            let Error::SourceConfig { diagnostic } = error else {
                panic!("{error}")
            };
            assert_eq!(diagnostic.code(), Some(GridCoordinateError::CODE));
            assert_eq!(
                diagnostic.span_kind(),
                crate::ParseDiagnosticSpanKind::Exact
            );
            let start = source.find("column: NaN").unwrap();
            assert_eq!(
                diagnostic.span(),
                Some(SourceSpan::new(start, start + "column".len()))
            );
            assert_eq!(
                diagnostic.message(),
                r#"GRID_INVALID_COORDINATE: Invalid column for placement "a.💡[1][0]": expected a safe integer; received NaN (number)."#
            );
        };
        let parser = crate::SemanticParser::new();
        assert_error(parser.parse_diagram_sync(source).unwrap_err());
        #[cfg(feature = "presentation")]
        assert_error(
            crate::MermaidEngine::new()
                .parse_metadata_sync(source)
                .unwrap_err(),
        );
        #[cfg(feature = "editor-facts")]
        match parser.capture_diagram_snapshot_sync(source) {
            crate::SemanticSnapshotCapture::Failed { error, .. } => assert_error(error),
            other => panic!("{other:?}"),
        }
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn grid_source_validation_respects_single_and_multiple_init_detection_order() {
        let parser = crate::SemanticParser::new();
        let bad = "%%{init: {\"grid\": {\"placements\": {\"service\": {\"row\": 1.5}}}}}%%\n";
        let good = "%%{init: {\"grid\": {\"placements\": {\"service\": {\"row\": 2}}}}}%%\n";
        assert!(matches!(
            parser.parse_diagram_sync(&format!("{bad}no diagram")),
            Err(Error::DetectType(_))
        ));
        assert!(matches!(
            parser.parse_diagram_sync(&format!("{bad}{good}no diagram")),
            Err(Error::SourceConfig { .. })
        ));
        let frontmatter = "---\nconfig:\n  grid:\n    placements:\n      A: {row: 1.5}\n---\n";
        let noop = "%%{init: {\"look\": \"classic\"}}%%\n";
        assert!(matches!(
            parser.parse_diagram_sync(&format!("{frontmatter}{noop}{noop}no diagram")),
            Err(Error::DetectType(_))
        ));
        assert!(matches!(
            parser.parse_diagram_sync(&format!("{frontmatter}%%{{init}}%%\nno diagram")),
            Err(Error::SourceConfig { .. })
        ));
        assert!(matches!(
            parser.parse_diagram_sync(&format!("{bad}flowchart LR\nA -->")),
            Err(Error::SourceConfig { .. })
        ));
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn grid_single_init_can_clear_frontmatter_overrides_before_admission() {
        let parser = crate::SemanticParser::new();
        let frontmatter = "---\nconfig:\n  grid:\n    placements:\n      A: [{row: 0.5}]\n---\n";
        let body = "flowchart LR\nA\n";
        for (init, expected) in [
            (r#"{"grid":null}"#, None),
            (r#"{"grid":{"placements":null}}"#, Some(json!({}))),
            (
                r#"{"grid":{"placements":{"A":{"column":2}}}}"#,
                Some(json!({"placements": {}})),
            ),
            (
                r#"{"grid":{"placements":{"A":{"4294967294":{"column":2}}}}}"#,
                Some(json!({"placements": {}})),
            ),
        ] {
            let source = format!("{frontmatter}%%{{init: {init}}}%%\n{body}");
            let parsed = parser.parse_diagram_sync(&source).unwrap();
            assert_eq!(
                parsed.metadata.config.get("grid"),
                expected.as_ref(),
                "{init}"
            );
        }
    }

    #[test]
    fn grid_init_keeps_explicit_type_preprocessing_independent_of_detection() {
        let control = OperationControl::new();
        let source =
            "%%{init: {\"grid\": {\"placements\": {\"A\": {\"row\": 1}}}}}%%\ncustom input";
        let preprocessed = crate::preprocess::preprocess_builtin_semantic_controlled(
            source,
            Some("custom-test"),
            &control,
        )
        .unwrap()
        .unwrap();
        assert_eq!(preprocessed.code(), "custom input");
        assert_eq!(
            preprocessed.config.as_value()["grid"]["placements"]["A"]["row"],
            1
        );
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn grid_frontmatter_alias_preserves_nonfinite_coordinate_through_slot_merges() {
        let parser = crate::SemanticParser::new();
        let frontmatter = "---\ncoordinate: &bad .nan\nconfig:\n  grid:\n    placements:\n      A: [{row: *bad}]\n---\n";
        let retain_row =
            "%%{init: {\"grid\": {\"placements\": {\"A\": {\"0\": {\"column\": 2}}}}}}%%\n";
        let replace_row = "%%{init: {\"grid\": {\"placements\": {\"A\": [{\"row\": 0}]}}}}%%\n";
        let body = "flowchart LR\nA\n";
        let error = parser
            .parse_diagram_sync(&format!("{frontmatter}{retain_row}{body}"))
            .unwrap_err();
        let Error::SourceConfig { diagnostic } = error else {
            panic!("{error}")
        };
        assert_eq!(diagnostic.code(), Some(GridCoordinateError::CODE));
        assert!(diagnostic.message().ends_with("received NaN (number)."));
        let result = parser
            .parse_diagram_sync(&format!("{frontmatter}{replace_row}{body}"))
            .unwrap();
        assert_eq!(
            result.metadata.config.as_value()["grid"]["placements"]["A"],
            json!([{"row": 0}])
        );
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn configured_grid_validates_after_body_and_checks_row_before_column() {
        let parser = crate::SemanticParser::new().with_site_config(MermaidConfig::from_value(json!({
            "layout": "grid", "grid": {"placements": {"unknown": [false, {"column": "x", "row": null}]}},
        })));
        let assert_error = |error: Error, code| {
            let Error::DiagramParse { diagnostic, .. } = error else {
                panic!("{error}")
            };
            assert_eq!(diagnostic.code(), code);
            if code.is_some() {
                assert!(
                    diagnostic
                        .message()
                        .contains(r#"Invalid row for placement "unknown[1]""#)
                );
            }
        };
        assert_error(
            parser.parse_diagram_sync("flowchart LR\nA-->").unwrap_err(),
            None,
        );
        assert_error(
            parser.parse_diagram_sync("flowchart LR\nA").unwrap_err(),
            Some(GridCoordinateError::CODE),
        );
        #[cfg(feature = "presentation")]
        {
            let engine = crate::MermaidEngine::new().with_site_config(crate::presentation::SourcePresentationConfig::from_value(json!({
                "layout": "grid", "grid": {"placements": {"unknown": [false, {"column": "x", "row": null}]}},
            })));
            assert_error(
                engine
                    .parse_diagram_for_render_model_sync(
                        "flowchart LR\nA",
                        crate::ParseOptions::strict(),
                    )
                    .unwrap_err(),
                Some(GridCoordinateError::CODE),
            );
        }
    }
}

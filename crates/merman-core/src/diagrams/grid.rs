use crate::{
    Error, MermaidConfig, OperationControl, OperationControlResult, ParseDiagnostic,
    ParseDiagnosticSpanKind, Result,
};
use merman_core_flow::grid::InlineGridCoordinates;
use merman_core_preprocess::grid_coordinate::GridCoordinateError;

pub(super) fn validate_inline_grid(
    config: &MermaidConfig,
    diagram_type: &str,
    coordinates: &InlineGridCoordinates,
    control: &OperationControl,
) -> OperationControlResult<Result<()>> {
    if config.get_str("layout") != Some("grid") {
        return Ok(Ok(()));
    }
    // Mermaid validates the effective placement map before the DB inspects its inline fields.
    if let Err(error) = crate::preprocess::validate_configured_grid(config, diagram_type, control)?
    {
        return Ok(Err(error));
    }
    Ok(coordinates
        .validate_controlled(control)?
        .map_err(|(error, span)| {
            Error::diagram_parse_diagnostic(
                diagram_type,
                ParseDiagnostic::new(error.to_string())
                    .with_code(GridCoordinateError::CODE)
                    .with_span(span, ParseDiagnosticSpanKind::Exact),
            )
        }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn inline_grid_diagnostics_agree_across_semantic_render_and_editor_paths() {
        let token = "@{ row: 0.5 }";
        let source = concat!(
            "\u{feff}---\r\nconfig:\r\n  layout: grid\r\n---\r\n",
            "flowchart LR\r\nA[服务]\r\nA@{ row: -1 }\r\nA@{ row: 0.5 }\r\n",
        );
        let start = source.find(token).unwrap();
        let assert_error = |error: &Error| {
            let Error::DiagramParse { diagnostic, .. } = error else {
                panic!("{error}")
            };
            assert_eq!(diagnostic.code(), Some(GridCoordinateError::CODE));
            assert_eq!(
                diagnostic.span(),
                Some(crate::SourceSpan::new(start, start + token.len()))
            );
            assert!(diagnostic.message().ends_with("received 0.5 (number)."));
        };
        let parser = crate::SemanticParser::new();
        assert_error(&parser.parse_diagram_sync(source).unwrap_err());
        #[cfg(feature = "editor-facts")]
        {
            let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                parser.capture_diagram_snapshot_sync(source)
            else {
                panic!("expected a parsed snapshot")
            };
            let crate::SemanticParseOutcome::Failed(error) = snapshot.outcome() else {
                panic!("expected an inline coordinate error")
            };
            assert_error(error);
            assert!(matches!(
                snapshot.editor_facts(),
                crate::ParsedEditorFacts::Available(_)
            ));
        }
        #[cfg(feature = "presentation")]
        {
            let engine = crate::MermaidEngine::new();
            assert_error(
                &engine
                    .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
                    .unwrap_err(),
            );
            #[cfg(feature = "compat-json")]
            assert_error(
                &engine
                    .parse_diagram_sync(source, crate::ParseOptions::strict())
                    .unwrap_err(),
            );
        }
    }

    #[cfg(feature = "diagram-agentflow")]
    #[test]
    fn inline_grid_agentflow_errors_come_from_the_completed_connector_slot() {
        let source = "---\nconfig:\n  layout: grid\n---\nagentflow LR\nDock@{row: false}\nconnector Dock\nDock@{ column: 1.25 }\n";
        let error = crate::SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("{error}")
        };
        assert!(
            diagnostic
                .message()
                .contains(r#"Invalid column for placement "Dock""#)
        );
        let start = source.rfind("@{").unwrap();
        assert_eq!(
            diagnostic.span(),
            Some(crate::SourceSpan::new(start, source.len() - 1))
        );
    }
}

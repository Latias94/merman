use crate::{
    Error, MermaidConfig, OperationControl, OperationControlResult, ParseDiagnostic,
    ParseDiagnosticSpanKind, Result,
};
use merman_core_flow::grid::InlineGridCoordinates;
use merman_core_preprocess::grid_coordinate::GridCoordinateError;
use merman_model::grid::GridInlinePlacements;

pub(super) fn validate_inline_grid(
    config: &MermaidConfig,
    diagram_type: &str,
    coordinates: &InlineGridCoordinates,
    capture_placements: bool,
    control: &OperationControl,
) -> OperationControlResult<Result<GridInlinePlacements>> {
    if config.get_str("layout") != Some("grid") {
        return Ok(Ok(GridInlinePlacements::default()));
    }
    // Mermaid validates the effective placement map before the DB inspects its inline fields.
    if let Err(error) = crate::preprocess::validate_configured_grid(config, diagram_type, control)?
    {
        return Ok(Err(error));
    }
    let placements = if capture_placements {
        coordinates.validate_placements_controlled(control)?
    } else {
        coordinates
            .validate_controlled(control)?
            .map(|()| GridInlinePlacements::default())
    };
    Ok(placements.map_err(|(error, span)| {
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

    #[cfg(all(feature = "diagram-flowchart", feature = "presentation"))]
    #[test]
    fn inline_grid_flowchart_render_model_retains_validated_placement() {
        use merman_model::grid::{GridHorizontalAlign, GridPlacementOwner};
        use merman_model::resources::{
            InputResourceLimitId, InputResourcePolicy, ResourceProfile,
            model_complexity_from_presentation,
        };

        let source = "---\nconfig:\n  layout: grid\n---\nflowchart LR\nA[Service]\nA@{row: -2, column: 0, horizontalAlign: left}\n";
        let engine = crate::MermaidEngine::new();
        let parse = |source: &str| {
            engine
                .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
                .unwrap()
                .unwrap()
        };
        let parsed = parse(source);
        let context = parsed.render_context().flowchart_render_context().unwrap();
        let [entry] = context.grid_placements().entries() else {
            panic!("expected the source-authored placement");
        };
        assert_eq!(
            (entry.id.as_str(), entry.owner),
            ("A", GridPlacementOwner::Vertex(0))
        );
        assert_eq!(entry.placement.row.unwrap().get(), -2);
        assert_eq!(entry.placement.column.unwrap().get(), 0);
        assert_eq!(
            entry.placement.horizontal_align,
            Some(GridHorizontalAlign::Left)
        );

        let dagre = parse(&source.replace("layout: grid", "layout: dagre"));
        assert!(
            dagre
                .render_context()
                .flowchart_render_context()
                .unwrap()
                .grid_placements()
                .entries()
                .is_empty()
        );
        let grid_complexity = model_complexity_from_presentation(parsed.presentation());
        let dagre_complexity = model_complexity_from_presentation(dagre.presentation());
        assert_eq!(
            grid_complexity.text_bytes,
            dagre_complexity.text_bytes + context.grid_placements().retained_bytes(),
        );
        let policy = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        policy
            .with_limit(
                InputResourceLimitId::MaxModelTextBytes,
                grid_complexity.text_bytes,
            )
            .unwrap()
            .check_model_complexity(grid_complexity)
            .unwrap();
        let error = policy
            .with_limit(
                InputResourceLimitId::MaxModelTextBytes,
                grid_complexity.text_bytes - 1,
            )
            .unwrap()
            .check_model_complexity(grid_complexity)
            .unwrap_err();
        assert_eq!(error.limit, "max_model_text_bytes");
        assert_eq!(error.actual, grid_complexity.text_bytes);
    }

    #[cfg(all(feature = "diagram-agentflow", feature = "presentation"))]
    #[test]
    fn inline_grid_agentflow_render_model_retains_connector_placement() {
        use merman_model::grid::{GridPlacementOwner, GridVerticalAlign};

        let source = "---\nconfig:\n  layout: grid\n---\nagentflow LR\nconnector Dock\nDock@{column: -1, verticalAlign: bottom}\n";
        let parsed = crate::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let crate::RenderSemanticModel::Agentflow(model) = parsed.model() else {
            panic!("expected an Agentflow render model");
        };
        let [entry] = model.visuals.grid_placements().entries() else {
            panic!("expected the connector placement");
        };
        assert_eq!(
            (entry.id.as_str(), entry.owner),
            ("Dock", GridPlacementOwner::Connector(0))
        );
        assert_eq!(entry.placement.column.unwrap().get(), -1);
        assert_eq!(
            entry.placement.vertical_align,
            Some(GridVerticalAlign::Bottom)
        );
    }
}

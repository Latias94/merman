#![cfg(feature = "diagram-info")]

use merman::{OperationControl, Renderer};

#[test]
fn semantic_artifact_exposes_canonical_meaning_without_presentation_features() {
    let artifact = Renderer::new()
        .parse_semantic("info", OperationControl::new())
        .expect("semantic parsing succeeds")
        .expect("Info should be detected");

    assert_eq!(artifact.diagram_type(), "info");
    assert!(matches!(
        artifact.model(),
        merman::model::SemanticDiagramModel::Info(_)
    ));
}

#[test]
fn semantic_parser_applies_site_config_without_evaluating_presentation() {
    let source = "%%{init: {\"themeVariables\": {\"primaryColor\": \"not-a-color\"}}}%%\ninfo";
    let renderer =
        Renderer::new().with_semantic_parser(merman::SemanticParser::new().with_site_config(
            merman::MermaidConfig::from_value(serde_json::json!({"logLevel": "debug"})),
        ));
    let artifact = renderer
        .parse_semantic(source, OperationControl::new())
        .expect("semantic parsing must not evaluate theme variables")
        .expect("Info should be detected");

    assert_eq!(
        artifact.metadata().semantic_config.get_str("logLevel"),
        Some("debug")
    );
    assert_eq!(
        artifact.metadata().config.as_value()["themeVariables"]["primaryColor"],
        "not-a-color"
    );
}

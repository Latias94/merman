use crate::*;
use futures::executor::block_on;
use serde_json::json;

#[cfg(feature = "large-features")]
#[test]
fn full_build_detects_mindmap() {
    let engine = Engine::new();
    let res = block_on(engine.parse_metadata("mindmap\n  root", ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.diagram_type, "mindmap");
}

#[cfg(not(feature = "large-features"))]
#[test]
fn tiny_build_does_not_detect_mindmap() {
    let engine = Engine::new();
    let err =
        block_on(engine.parse_metadata("mindmap\n  root", ParseOptions::default())).unwrap_err();
    assert!(
        err.to_string()
            .contains("No diagram type detected matching given configuration")
    );
}

#[cfg(feature = "large-features")]
#[test]
fn full_build_detects_flowchart_elk_and_sets_layout() {
    let engine = Engine::new();
    let res = block_on(engine.parse_metadata("flowchart-elk TD\nA-->B", ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.diagram_type, "flowchart-elk");
    assert_eq!(res.effective_config.get_str("layout"), Some("elk"));
}

#[cfg(not(feature = "large-features"))]
#[test]
fn tiny_build_flowchart_elk_falls_back_to_flowchart_v2() {
    let engine = Engine::new();
    let res = block_on(engine.parse_metadata("flowchart-elk TD\nA-->B", ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.diagram_type, "flowchart-v2");
    assert_eq!(res.effective_config.get_str("layout"), None);
}

#[test]
fn engine_with_site_config_preserves_default_renderer_for_detection() {
    let engine = Engine::new().with_site_config({
        let mut cfg = MermaidConfig::empty_object();
        cfg.set_value("securityLevel", json!("sandbox"));
        cfg
    });

    let text = r#"classDiagram
class Class1
"#;
    let res = block_on(engine.parse_metadata(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.diagram_type, "classDiagram");
}

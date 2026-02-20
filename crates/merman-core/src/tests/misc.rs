use crate::*;
use futures::executor::block_on;
use serde_json::json;

#[test]
fn parse_graph_defaults_to_flowchart_v2() {
    let engine = Engine::new();
    let res = block_on(engine.parse_metadata("graph TD;A-->B;", ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.diagram_type, "flowchart-v2");
    assert_eq!(res.config.as_value(), &json!({}));
}

#[test]
fn parse_merges_frontmatter_and_directive_config() {
    let engine = Engine::new();
    let text = r#"---
config:
  theme: forest
  flowchart:
    htmlLabels: true
---
%%{init: { 'theme': 'base' } }%%
graph TD;A-->B;"#;

    let res = block_on(engine.parse_metadata(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(
        res.config.as_value(),
        &json!({
            "theme": "base",
            "flowchart": {
                "htmlLabels": true
            }
        })
    );
}

#[test]
fn parse_diagram_as_sync_matches_auto_detect_for_flowchart_v2() {
    let engine = Engine::new();
    let input = "flowchart TD; A[Start]-->B[End];";

    let auto = engine
        .parse_diagram_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(auto.meta.diagram_type, "flowchart-v2");

    let known = engine
        .parse_diagram_as_sync("flowchart-v2", input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    assert_eq!(known.meta.diagram_type, "flowchart-v2");
    assert_eq!(known.model, auto.model);
}

#[test]
fn parse_metadata_as_sync_moves_init_config_without_detection() {
    let engine = Engine::new();
    let input = "%%{init: {\"config\": {\"htmlLabels\": true}}}%%\nflowchart TD; A-->B;";

    let meta = engine
        .parse_metadata_as_sync("flowchart-v2", input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    // Mermaid special-case: `flowchart-v2` config is stored under `flowchart`.
    assert_eq!(meta.config.get_bool("flowchart.htmlLabels"), Some(true));
}

#[test]
fn parse_metadata_as_sync_preserves_flowchart_elk_layout_side_effect() {
    let engine = Engine::new();
    let input = "flowchart-elk TD\nA-->B;";

    let meta = engine
        .parse_metadata_as_sync("flowchart-elk", input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(meta.effective_config.get_str("layout"), Some("elk"));
}

#[test]
fn parse_sanitizes_frontmatter_title_like_mermaid_common_db() {
    let engine = Engine::new();
    let text = r#"---
title: "Flowchart v2 arrows: graph direction \"<\""
---
graph <
A-->B
"#;

    let meta = block_on(engine.parse_metadata(text, ParseOptions::default()))
        .unwrap()
        .unwrap();

    assert_eq!(
        meta.title,
        Some(r#"Flowchart v2 arrows: graph direction "&lt;""#.to_string())
    );
}

#[test]
fn parse_merges_init_directive_numeric_values_like_upstream() {
    let engine = Engine::new();
    let text = r#"%%{init: { 'logLevel': 0 } }%%
sequenceDiagram
Alice->Bob: Hi
"#;

    let res = block_on(engine.parse_metadata(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.diagram_type, "sequence");
    assert_eq!(res.config.as_value(), &json!({ "logLevel": 0 }));
}

#[test]
fn parse_returns_malformed_frontmatter_error_for_unclosed_frontmatter() {
    let engine = Engine::new();
    let err = block_on(engine.parse_metadata(
        "---\ntitle: a malformed YAML front-matter\n",
        ParseOptions::default(),
    ))
    .unwrap_err();
    assert!(err.to_string().contains("Malformed YAML front-matter"));
}

#[test]
fn parse_can_suppress_unknown_diagram_errors() {
    let engine = Engine::new();
    let res = block_on(engine.parse_metadata(
        "this is not a mermaid diagram definition",
        ParseOptions {
            suppress_errors: true,
        },
    ))
    .unwrap();
    assert!(res.is_none());
}

#[test]
fn parse_sanitizes_common_db_fields_in_strict_mode() {
    let engine = Engine::new();
    let text = r#"sequenceDiagram
title: <script>alert(1)</script><b>t</b>
accTitle: <script>alert(1)</script><b>a</b>
accDescr: <script>alert(1)</script><b>d</b>
Alice->Bob:Hello"#;

    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();

    assert_eq!(res.model["title"], json!("<b>t</b>"));
    assert_eq!(res.model["accTitle"], json!("<b>a</b>"));
    assert_eq!(res.model["accDescr"], json!("<b>d</b>"));
}

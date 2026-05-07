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
fn parse_lenient_failures_use_error_diagram_across_engine_entrypoints() {
    let engine = Engine::new();
    let input = "flowchart TD\nA -->";
    let options = ParseOptions::lenient();

    let parsed = engine.parse_diagram_sync(input, options).unwrap().unwrap();
    assert_suppressed_error_diagram(&parsed);

    let parsed = engine
        .parse_diagram_as_sync("flowchart-v2", input, options)
        .unwrap()
        .unwrap();
    assert_suppressed_error_diagram(&parsed);

    let parsed = engine
        .parse_diagram_for_render_model_sync(input, options)
        .unwrap()
        .unwrap();
    assert_suppressed_error_render_diagram(&parsed);

    let parsed = engine
        .parse_diagram_for_render_model_as_sync("flowchart-v2", input, options)
        .unwrap()
        .unwrap();
    assert_suppressed_error_render_diagram(&parsed);
}

fn assert_suppressed_error_diagram(parsed: &ParsedDiagram) {
    assert_eq!(parsed.meta.diagram_type, "error");
    assert_eq!(parsed.model["type"], json!("error"));
}

fn assert_suppressed_error_render_diagram(parsed: &ParsedDiagramRender) {
    assert_eq!(parsed.meta.diagram_type, "error");
    match &parsed.model {
        RenderSemanticModel::Json(model) => assert_eq!(model["type"], json!("error")),
        other => panic!("suppressed parse failures must render through JSON, got {other:?}"),
    }
}

#[test]
fn parse_sequence_render_model_uses_typed_variant_without_changing_json_parse() {
    let engine = Engine::new();
    let input = "sequenceDiagram\nAlice->>Bob: Hi";

    let parsed = engine
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    assert_eq!(parsed.meta.diagram_type, "sequence");
    match parsed.model {
        RenderSemanticModel::Sequence(model) => {
            assert_eq!(model.actor_order, ["Alice", "Bob"]);
            assert_eq!(model.messages[0].message_text(), "Hi");
        }
        other => panic!("sequence render parse should return typed model, got {other:?}"),
    }

    let parsed_json = engine
        .parse_diagram_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed_json.model["type"], json!("sequence"));
    assert_eq!(parsed_json.model["messages"][0]["message"], json!("Hi"));
}

#[test]
fn parse_kanban_render_model_uses_typed_variant_without_changing_json_parse() {
    let engine = Engine::new();
    let input = "kanban\n  Todo\n    item1\n  Doing\n    item2";

    let parsed = engine
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    assert_eq!(parsed.meta.diagram_type, "kanban");
    match parsed.model {
        RenderSemanticModel::Kanban(model) => {
            assert_eq!(model.nodes.len(), 4);
            assert!(model.nodes[0].is_group);
            assert_eq!(model.nodes[0].label, "Todo");
            assert_eq!(model.nodes[1].label, "item1");
            assert_eq!(model.nodes[1].parent_id.as_deref(), Some("Todo"));
        }
        other => panic!("kanban render parse should return typed model, got {other:?}"),
    }

    let parsed_json = engine
        .parse_diagram_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed_json.model["type"], json!("kanban"));
    assert_eq!(parsed_json.model["nodes"][0]["label"], json!("Todo"));
    assert_eq!(parsed_json.model["nodes"][1]["label"], json!("item1"));
}

#[test]
fn parse_gantt_render_model_uses_typed_variant_without_changing_json_parse() {
    let engine = Engine::new();
    let input = r#"
gantt
dateFormat YYYY-MM-DD
title Typed Gantt
section Alpha
Task 1: id1, 2024-01-01, 2d
"#;

    let parsed = engine
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    assert_eq!(parsed.meta.diagram_type, "gantt");
    match parsed.model {
        RenderSemanticModel::Gantt(model) => {
            assert_eq!(model.title.as_deref(), Some("Typed Gantt"));
            assert_eq!(model.date_format, "YYYY-MM-DD");
            assert_eq!(model.tasks.len(), 1);
            assert_eq!(model.tasks[0].id, "id1");
            assert_eq!(model.tasks[0].task, "Task 1");
            assert_eq!(model.tasks[0].section, "Alpha");
        }
        other => panic!("gantt render parse should return typed model, got {other:?}"),
    }

    let parsed_json = engine
        .parse_diagram_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed_json.model["type"], json!("gantt"));
    assert_eq!(parsed_json.model["title"], json!("Typed Gantt"));
    assert_eq!(parsed_json.model["tasks"][0]["id"], json!("id1"));
    assert_eq!(parsed_json.model["tasks"][0]["task"], json!("Task 1"));
}

#[test]
fn parse_pie_render_model_uses_typed_variant_without_changing_json_parse() {
    let engine = Engine::new();
    let input = r#"
pie showData title Typed Pie
  "Alpha" : 60
  "Beta" : 40
"#;

    let parsed = engine
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    assert_eq!(parsed.meta.diagram_type, "pie");
    match parsed.model {
        RenderSemanticModel::Pie(model) => {
            assert!(model.show_data);
            assert_eq!(model.title.as_deref(), Some("Typed Pie"));
            assert_eq!(model.sections.len(), 2);
            assert_eq!(model.sections[0].label, "Alpha");
            assert_eq!(model.sections[0].value, 60.0);
        }
        other => panic!("pie render parse should return typed model, got {other:?}"),
    }

    let parsed_json = engine
        .parse_diagram_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed_json.model["type"], json!("pie"));
    assert_eq!(parsed_json.model["showData"], json!(true));
    assert_eq!(parsed_json.model["title"], json!("Typed Pie"));
    assert_eq!(parsed_json.model["sections"][0]["label"], json!("Alpha"));
    assert_eq!(parsed_json.model["sections"][0]["value"], json!(60.0));
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

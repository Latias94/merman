use crate::diagrams::xychart::{XyChartAxisRenderModel, XyChartPlotType};
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
fn render_semantic_model_kind_reports_canonical_names() {
    let sequence = render_model_for("sequenceDiagram\nAlice->>Bob: Hi");
    assert_eq!(sequence.kind(), "sequence");

    let flowchart = render_model_for("flowchart TD\nA-->B");
    assert_eq!(flowchart.kind(), "flowchart");

    let er = render_model_for("erDiagram\nCUSTOMER ||--o{ ORDER : places");
    assert_eq!(er.kind(), "er");

    let json_model = RenderSemanticModel::Json(json!({ "type": "custom" }));
    assert_eq!(json_model.kind(), "json");
}

#[test]
fn render_semantic_model_supports_diagram_type_aliases() {
    let sequence = render_model_for("sequenceDiagram\nAlice->>Bob: Hi");
    assert!(sequence.supports_diagram_type("sequence"));
    assert!(sequence.supports_diagram_type("zenuml"));
    assert!(!sequence.supports_diagram_type("flowchart-v2"));

    let flowchart = render_model_for("flowchart TD\nA-->B");
    assert!(flowchart.supports_diagram_type("flowchart-v2"));
    assert!(flowchart.supports_diagram_type("flowchart"));
    assert!(flowchart.supports_diagram_type("flowchart-elk"));
    assert!(!flowchart.supports_diagram_type("sequence"));

    let er = render_model_for("erDiagram\nCUSTOMER ||--o{ ORDER : places");
    assert!(er.supports_diagram_type("er"));
    assert!(er.supports_diagram_type("erDiagram"));
    assert!(!er.supports_diagram_type("classDiagram"));

    let json_model = RenderSemanticModel::Json(json!({ "type": "custom" }));
    assert!(json_model.supports_diagram_type("unknown-plugin"));
}

fn render_model_for(input: &str) -> RenderSemanticModel {
    Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .model
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
fn parse_xychart_render_model_uses_typed_variant_without_changing_json_parse() {
    let engine = Engine::new();
    let input = r#"
xychart horizontal
title "Typed XYChart"
accTitle: XY accTitle
accDescr: XY accDescription
x-axis "X Axis" [Alpha, Beta]
y-axis "Y Axis" 1 --> 5
bar "Series 1" [1, 2]
line "Series 2" [2, 3]
"#;

    let parsed = engine
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    assert_eq!(parsed.meta.diagram_type, "xychart");
    match parsed.model {
        RenderSemanticModel::XyChart(model) => {
            assert_eq!(model.orientation, "horizontal");
            assert_eq!(model.title.as_deref(), Some("Typed XYChart"));
            assert_eq!(model.acc_title.as_deref(), Some("XY accTitle"));
            assert_eq!(model.acc_descr.as_deref(), Some("XY accDescription"));
            assert_eq!(
                model.x_axis,
                XyChartAxisRenderModel::Band {
                    title: "X Axis".to_string(),
                    categories: vec!["Alpha".to_string(), "Beta".to_string()],
                }
            );
            assert_eq!(
                model.y_axis,
                XyChartAxisRenderModel::Linear {
                    title: "Y Axis".to_string(),
                    min: Some(1.0),
                    max: Some(5.0),
                }
            );
            assert_eq!(model.plots.len(), 2);
            assert_eq!(model.plots[0].plot_type, XyChartPlotType::Bar);
            assert_eq!(model.plots[0].values, vec![1.0, 2.0]);
            assert_eq!(
                model.plots[0].data,
                vec![
                    ("Alpha".to_string(), Some(1.0)),
                    ("Beta".to_string(), Some(2.0)),
                ]
            );
            assert_eq!(model.plots[1].plot_type, XyChartPlotType::Line);
        }
        other => panic!("xychart render parse should return typed model, got {other:?}"),
    }

    let parsed_json = engine
        .parse_diagram_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed_json.model["type"], json!("xychart"));
    assert_eq!(parsed_json.model["title"], json!("Typed XYChart"));
    assert_eq!(parsed_json.model["xAxis"]["type"], json!("band"));
    assert_eq!(
        parsed_json.model["xAxis"]["categories"],
        json!(["Alpha", "Beta"])
    );
    assert_eq!(parsed_json.model["yAxis"]["type"], json!("linear"));
    assert_eq!(parsed_json.model["yAxis"]["min"], json!(1.0));
    assert_eq!(parsed_json.model["yAxis"]["max"], json!(5.0));
    assert_eq!(parsed_json.model["plots"][0]["type"], json!("bar"));
    assert_eq!(parsed_json.model["plots"][1]["type"], json!("line"));
    assert!(parsed_json.model.get("config").is_some());
}

#[test]
fn parse_packet_render_model_uses_typed_variant_without_changing_json_parse() {
    let engine = Engine::new();
    let input = r#"
packet
title Typed Packet
accTitle: Packet accTitle
accDescr: Packet accDescription
+8: "byte"
+16: "word"
"#;

    let parsed = engine
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    assert_eq!(parsed.meta.diagram_type, "packet");
    match parsed.model {
        RenderSemanticModel::Packet(model) => {
            assert_eq!(model.title.as_deref(), Some("Typed Packet"));
            assert_eq!(model.acc_title.as_deref(), Some("Packet accTitle"));
            assert_eq!(model.acc_descr.as_deref(), Some("Packet accDescription"));
            assert_eq!(model.packet.len(), 1);
            assert_eq!(model.packet[0].len(), 2);
            assert_eq!(model.packet[0][0].start, 0);
            assert_eq!(model.packet[0][0].end, 7);
            assert_eq!(model.packet[0][0].bits, 8);
            assert_eq!(model.packet[0][0].label, "byte");
        }
        other => panic!("packet render parse should return typed model, got {other:?}"),
    }

    let parsed_json = engine
        .parse_diagram_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed_json.model["type"], json!("packet"));
    assert_eq!(parsed_json.model["title"], json!("Typed Packet"));
    assert_eq!(parsed_json.model["accTitle"], json!("Packet accTitle"));
    assert_eq!(
        parsed_json.model["accDescr"],
        json!("Packet accDescription")
    );
    assert_eq!(parsed_json.model["packet"][0][0]["label"], json!("byte"));
    assert_eq!(parsed_json.model["packet"][0][0]["bits"], json!(8));
}

#[test]
fn parse_timeline_render_model_uses_typed_variant_without_changing_json_parse() {
    let engine = Engine::new();
    let input = r#"
timeline
title Typed Timeline
accTitle: Timeline accTitle
accDescr: Timeline accDescription
section Alpha
Task 1: event 1: event 2
"#;

    let parsed = engine
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    assert_eq!(parsed.meta.diagram_type, "timeline");
    match parsed.model {
        RenderSemanticModel::Timeline(model) => {
            assert_eq!(model.title.as_deref(), Some("Typed Timeline"));
            assert_eq!(model.acc_title.as_deref(), Some("Timeline accTitle"));
            assert_eq!(model.acc_descr.as_deref(), Some("Timeline accDescription"));
            assert_eq!(model.sections.as_slice(), ["Alpha"]);
            assert_eq!(model.tasks.len(), 1);
            assert_eq!(model.tasks[0].id, 0);
            assert_eq!(model.tasks[0].section, "Alpha");
            assert_eq!(model.tasks[0].task_type, "Alpha");
            assert_eq!(model.tasks[0].task, "Task 1");
            assert_eq!(model.tasks[0].events.as_slice(), ["event 1", "event 2"]);
        }
        other => panic!("timeline render parse should return typed model, got {other:?}"),
    }

    let parsed_json = engine
        .parse_diagram_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed_json.model["type"], json!("timeline"));
    assert_eq!(parsed_json.model["title"], json!("Typed Timeline"));
    assert_eq!(parsed_json.model["sections"][0], json!("Alpha"));
    assert_eq!(parsed_json.model["tasks"][0]["id"], json!(0));
    assert_eq!(parsed_json.model["tasks"][0]["type"], json!("Alpha"));
    assert_eq!(parsed_json.model["tasks"][0]["task"], json!("Task 1"));
    assert_eq!(
        parsed_json.model["tasks"][0]["events"],
        json!(["event 1", "event 2"])
    );
}

#[test]
fn parse_journey_render_model_uses_typed_variant_without_changing_json_parse() {
    let engine = Engine::new();
    let input = r#"
journey
title Typed Journey
accTitle: Journey accTitle
accDescr: Journey accDescription
section Shopping
Get keys: 5: Dad
Drive: bad-score: Dad, Mum
"#;

    let parsed = engine
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    assert_eq!(parsed.meta.diagram_type, "journey");
    match parsed.model {
        RenderSemanticModel::Journey(model) => {
            assert_eq!(model.title.as_deref(), Some("Typed Journey"));
            assert_eq!(model.acc_title.as_deref(), Some("Journey accTitle"));
            assert_eq!(model.acc_descr.as_deref(), Some("Journey accDescription"));
            assert_eq!(model.sections.as_slice(), ["Shopping"]);
            assert_eq!(model.actors.as_slice(), ["Dad", "Mum"]);
            assert_eq!(model.tasks.len(), 2);
            assert_eq!(model.tasks[0].score, 5);
            assert!(!model.tasks[0].score_is_nan);
            assert_eq!(model.tasks[0].people.as_slice(), ["Dad"]);
            assert_eq!(model.tasks[1].task, "Drive");
            assert!(model.tasks[1].score_is_nan);
            assert_eq!(model.tasks[1].people.as_slice(), ["Dad", "Mum"]);
        }
        other => panic!("journey render parse should return typed model, got {other:?}"),
    }

    let parsed_json = engine
        .parse_diagram_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed_json.model["type"], json!("journey"));
    assert_eq!(parsed_json.model["title"], json!("Typed Journey"));
    assert_eq!(parsed_json.model["actors"], json!(["Dad", "Mum"]));
    assert_eq!(parsed_json.model["tasks"][0]["score"], json!(5));
    assert!(parsed_json.model["tasks"][0].get("scoreIsNaN").is_none());
    assert_eq!(parsed_json.model["tasks"][1]["scoreIsNaN"], json!(true));
    assert_eq!(
        parsed_json.model["tasks"][1]["people"],
        json!(["Dad", "Mum"])
    );
}

#[test]
fn parse_requirement_render_model_uses_typed_variant_without_changing_json_parse() {
    let engine = Engine::new();
    let input = r##"
requirementDiagram
accTitle: Requirement accTitle
accDescr: Requirement accDescription
direction LR
requirement req_login:::critical {
  id: REQ-1
  text: "Login must work"
  risk: high
  verifymethod: test
}
element api {
  type: service
  docRef: docs/api.md
}
class api external
classDef critical fill:#f9f,stroke:#333,color:#111
classDef external stroke:#0f0
req_login - verifies -> api
"##;

    let parsed = engine
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    assert_eq!(parsed.meta.diagram_type, "requirement");
    match parsed.model {
        RenderSemanticModel::Requirement(model) => {
            assert_eq!(model.acc_title.as_deref(), Some("Requirement accTitle"));
            assert_eq!(
                model.acc_descr.as_deref(),
                Some("Requirement accDescription")
            );
            assert_eq!(model.direction, "LR");
            assert_eq!(model.requirements.len(), 1);
            assert_eq!(model.requirements[0].name, "req_login");
            assert_eq!(model.requirements[0].node_type, "Requirement");
            assert_eq!(model.requirements[0].requirement_id, "REQ-1");
            assert_eq!(model.requirements[0].text, "Login must work");
            assert_eq!(model.requirements[0].risk, "High");
            assert_eq!(model.requirements[0].verify_method, "Test");
            assert!(
                model.requirements[0]
                    .classes
                    .iter()
                    .any(|c| c == "critical")
            );
            assert!(
                model.requirements[0]
                    .css_styles
                    .iter()
                    .any(|s| s == "fill:#f9f")
            );
            assert_eq!(model.elements.len(), 1);
            assert_eq!(model.elements[0].name, "api");
            assert_eq!(model.elements[0].element_type, "service");
            assert_eq!(model.elements[0].doc_ref, "docs/api.md");
            assert!(model.elements[0].classes.iter().any(|c| c == "external"));
            assert_eq!(model.relationships.len(), 1);
            assert_eq!(model.relationships[0].rel_type, "verifies");
            assert_eq!(model.relationships[0].src, "req_login");
            assert_eq!(model.relationships[0].dst, "api");
            assert!(model.classes.contains_key("critical"));
            assert!(model.classes.contains_key("external"));
        }
        other => panic!("requirement render parse should return typed model, got {other:?}"),
    }

    let parsed_json = engine
        .parse_diagram_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed_json.model["type"], json!("requirement"));
    assert_eq!(parsed_json.model["accTitle"], json!("Requirement accTitle"));
    assert_eq!(
        parsed_json.model["accDescr"],
        json!("Requirement accDescription")
    );
    assert_eq!(parsed_json.model["direction"], json!("LR"));
    assert_eq!(
        parsed_json.model["requirements"][0]["name"],
        json!("req_login")
    );
    assert_eq!(
        parsed_json.model["requirements"][0]["requirementId"],
        json!("REQ-1")
    );
    assert_eq!(
        parsed_json.model["requirements"][0]["verifyMethod"],
        json!("Test")
    );
    assert_eq!(
        parsed_json.model["elements"][0]["docRef"],
        json!("docs/api.md")
    );
    assert_eq!(
        parsed_json.model["relationships"][0]["type"],
        json!("verifies")
    );
    assert!(parsed_json.model.get("config").is_some());
}

#[test]
fn parse_sankey_render_model_uses_typed_variant_without_changing_json_parse() {
    let engine = Engine::new();
    let input = r#"
sankey-beta
Source,Target,10
Target,Done,2.5
"#;

    let parsed = engine
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    assert_eq!(parsed.meta.diagram_type, "sankey");
    match parsed.model {
        RenderSemanticModel::Sankey(model) => {
            assert_eq!(model.graph.nodes.len(), 3);
            assert_eq!(model.graph.nodes[0].id, "Source");
            assert_eq!(model.graph.nodes[1].id, "Target");
            assert_eq!(model.graph.nodes[2].id, "Done");
            assert_eq!(model.graph.links.len(), 2);
            assert_eq!(model.graph.links[0].source, "Source");
            assert_eq!(model.graph.links[0].target, "Target");
            assert_eq!(model.graph.links[0].value, json!(10));
            assert_eq!(model.graph.links[1].source, "Target");
            assert_eq!(model.graph.links[1].target, "Done");
            assert_eq!(model.graph.links[1].value, json!(2.5));
        }
        other => panic!("sankey render parse should return typed model, got {other:?}"),
    }

    let parsed_json = engine
        .parse_diagram_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed_json.model["type"], json!("sankey"));
    assert_eq!(
        parsed_json.model["graph"]["nodes"][0]["id"],
        json!("Source")
    );
    assert_eq!(
        parsed_json.model["graph"]["links"][0],
        json!({
            "source": "Source",
            "target": "Target",
            "value": 10,
        })
    );
    assert_eq!(parsed_json.model["graph"]["links"][1]["value"], json!(2.5));
    assert!(parsed_json.model.get("config").is_some());
}

#[test]
fn parse_radar_render_model_uses_typed_variant_without_changing_json_parse() {
    let engine = Engine::new();
    let input = r#"
radar-beta
title Typed Radar
accTitle: Radar accTitle
accDescr: Radar accDescription
axis A["Axis A"], B["Axis B"], C["Axis C"]
curve first["First Curve"]{1,2,3}
curve second{ C: 9, A: 7, B: 8 }
showLegend false
ticks 4
min 1
max 10
graticule polygon
"#;

    let parsed = engine
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    assert_eq!(parsed.meta.diagram_type, "radar");
    match parsed.model {
        RenderSemanticModel::Radar(model) => {
            assert_eq!(model.title.as_deref(), Some("Typed Radar"));
            assert_eq!(model.acc_title.as_deref(), Some("Radar accTitle"));
            assert_eq!(model.acc_descr.as_deref(), Some("Radar accDescription"));
            assert_eq!(model.axes.len(), 3);
            assert_eq!(model.axes[0].name, "A");
            assert_eq!(model.axes[0].label, "Axis A");
            assert_eq!(model.curves.len(), 2);
            assert_eq!(model.curves[0].label, "First Curve");
            assert_eq!(model.curves[0].entries, vec![json!(1), json!(2), json!(3)]);
            assert_eq!(model.curves[1].entries, vec![json!(7), json!(8), json!(9)]);
            assert!(!model.options.show_legend);
            assert_eq!(model.options.ticks, json!(4));
            assert_eq!(model.options.min, json!(1));
            assert_eq!(model.options.max, Some(json!(10)));
            assert_eq!(model.options.graticule, "polygon");
        }
        other => panic!("radar render parse should return typed model, got {other:?}"),
    }

    let parsed_json = engine
        .parse_diagram_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed_json.model["type"], json!("radar"));
    assert_eq!(parsed_json.model["title"], json!("Typed Radar"));
    assert_eq!(parsed_json.model["accTitle"], json!("Radar accTitle"));
    assert_eq!(
        parsed_json.model["axes"][0],
        json!({"name": "A", "label": "Axis A"})
    );
    assert_eq!(
        parsed_json.model["curves"][1],
        json!({"name": "second", "label": "second", "entries": [7, 8, 9]})
    );
    assert_eq!(
        parsed_json.model["options"],
        json!({
            "showLegend": false,
            "ticks": 4,
            "max": 10,
            "min": 1,
            "graticule": "polygon",
        })
    );
    assert!(parsed_json.model.get("config").is_some());
}

#[test]
fn parse_info_render_model_uses_typed_variant_without_changing_json_parse() {
    let engine = Engine::new();
    let input = r#"
info
showInfo
"#;

    let parsed = engine
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    assert_eq!(parsed.meta.diagram_type, "info");
    match parsed.model {
        RenderSemanticModel::Info(model) => {
            assert!(model.show_info);
        }
        other => panic!("info render parse should return typed model, got {other:?}"),
    }

    let parsed_json = engine
        .parse_diagram_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed_json.model["type"], json!("info"));
    assert_eq!(parsed_json.model["showInfo"], json!(true));
}

#[test]
fn parse_er_render_model_uses_typed_variant_without_changing_json_parse() {
    let engine = Engine::new();
    let input = r#"
erDiagram
accTitle: ER accTitle
accDescr: ER accDescription
CUSTOMER ||--o{ ORDER : places
CUSTOMER {
  string id
}
"#;

    let parsed = engine
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();

    assert_eq!(parsed.meta.diagram_type, "er");
    match parsed.model {
        RenderSemanticModel::Er(model) => {
            assert_eq!(model.acc_title.as_deref(), Some("ER accTitle"));
            assert_eq!(model.acc_descr.as_deref(), Some("ER accDescription"));
            assert_eq!(model.direction, "TB");
            assert!(model.entities.contains_key("CUSTOMER"));
            assert!(model.entities.contains_key("ORDER"));
            assert_eq!(model.relationships.len(), 1);
            assert_eq!(model.relationships[0].entity_a, "entity-CUSTOMER-0");
            assert_eq!(model.relationships[0].entity_b, "entity-ORDER-1");
            assert_eq!(model.relationships[0].role_a, "places");
            assert_eq!(model.relationships[0].rel_spec.card_a, "ZERO_OR_MORE");
            assert_eq!(model.relationships[0].rel_spec.card_b, "ONLY_ONE");
            assert_eq!(model.relationships[0].rel_spec.rel_type, "IDENTIFYING");
        }
        other => panic!("er render parse should return typed model, got {other:?}"),
    }

    let parsed_json = engine
        .parse_diagram_sync(input, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed_json.model["type"], json!("er"));
    assert_eq!(parsed_json.model["accTitle"], json!("ER accTitle"));
    assert_eq!(
        parsed_json.model["relationships"][0]["roleA"],
        json!("places")
    );
    assert!(parsed_json.model.get("constants").is_some());
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

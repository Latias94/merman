use crate::*;
use futures::executor::block_on;
use serde_json::json;

#[test]
fn parse_diagram_state_v2_alias_and_colon_description() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
state "Small State 1" as namedState1"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    assert_eq!(res.meta.diagram_type, "stateDiagram");
    assert_eq!(
        res.model["states"]["namedState1"]["descriptions"][0],
        json!("Small State 1")
    );

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
namedState1 : Small State 1"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    assert_eq!(
        res.model["states"]["namedState1"]["descriptions"][0],
        json!("Small State 1")
    );

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
namedState1:Small State 1"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    assert_eq!(
        res.model["states"]["namedState1"]["descriptions"][0],
        json!("Small State 1")
    );
}

#[test]
fn parse_diagram_state_v2_multibyte_ids_do_not_panic() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
顧客 --> 完了: 送信
"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();

    assert_eq!(res.model["relations"][0]["id1"], json!("顧客"));
    assert_eq!(res.model["relations"][0]["id2"], json!("完了"));
    assert_eq!(res.model["edges"][0]["label"], json!("送信"));
}

#[test]
fn parse_diagram_state_v2_groups_and_unsafe_ids() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
state "Small State 1" as namedState1
state "Big State 1" as bigState1 {
  bigState1InternalState
}
namedState1 --> bigState1: should point to \nBig State 1 container

state "Small State 2" as namedState2
state bigState2 {
  bigState2InternalState
}
namedState2 --> bigState2: should point to \nbigState2 container"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();

    assert_eq!(
        res.model["states"]["bigState1"]["doc"][0]["id"],
        json!("bigState1InternalState")
    );
    assert_eq!(
        res.model["states"]["bigState2"]["doc"][0]["id"],
        json!("bigState2InternalState")
    );
    assert_eq!(res.model["relations"][0]["id1"], json!("namedState1"));
    assert_eq!(res.model["relations"][0]["id2"], json!("bigState1"));
    assert_eq!(res.model["relations"][1]["id1"], json!("namedState2"));
    assert_eq!(res.model["relations"][1]["id2"], json!("bigState2"));

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
[*] --> __proto__
__proto__ --> [*]"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    assert!(res.model["states"]["__proto__"].is_object());
    assert!(res.model["states"]["root_start"].is_object());
    assert!(res.model["states"]["root_end"].is_object());
}

#[test]
fn parse_diagram_state_v2_classdef_class_and_shorthand() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
classDef exampleStyleClass background:#bbb,border:1.5px solid red;
a --> b:::exampleStyleClass
class a exampleStyleClass"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();

    assert_eq!(
        res.model["styleClasses"]["exampleStyleClass"]["styles"][0],
        json!("background:#bbb")
    );
    assert_eq!(
        res.model["styleClasses"]["exampleStyleClass"]["styles"][1],
        json!("border:1.5px solid red")
    );
    assert_eq!(
        res.model["states"]["a"]["classes"][0],
        json!("exampleStyleClass")
    );
    assert_eq!(
        res.model["states"]["b"]["classes"][0],
        json!("exampleStyleClass")
    );

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
classDef exampleStyleClass background:#bbb,border:1px solid red;
[*]:::exampleStyleClass --> b"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    assert_eq!(
        res.model["states"]["root_start"]["classes"][0],
        json!("exampleStyleClass")
    );

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
classDef exampleStyleClass background:#bbb,border:1px solid red;
a-->b
class a,b,c, d, e exampleStyleClass"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    for id in ["a", "b", "c", "d", "e"] {
        assert_eq!(
            res.model["states"][id]["classes"][0],
            json!("exampleStyleClass")
        );
    }
}

#[test]
fn parse_diagram_state_v2_style_statement_sets_node_styles_and_ignores_comments() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
id1
id2
style id1,id2 background:#bbb, font-weight:bold, font-style:italic;"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();

    assert_eq!(
        res.model["nodes"][0]["cssStyles"],
        json!(["background:#bbb", "font-weight:bold", "font-style:italic"])
    );
    assert_eq!(
        res.model["nodes"][1]["cssStyles"],
        json!(["background:#bbb", "font-weight:bold", "font-style:italic"])
    );

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
[*] --> Moving
Moving --> Still
Moving --> Crash
state Moving {
%% comment inside state
slow  --> fast
}"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();

    assert_eq!(
        res.model["states"]["Moving"]["doc"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn parse_diagram_state_v2_click_and_href_store_links() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
S1
click S1 "https://example.com" "Go to Example""#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    assert_eq!(
        res.model["links"]["S1"]["url"],
        json!("https://example.com")
    );
    assert_eq!(res.model["links"]["S1"]["tooltip"], json!("Go to Example"));

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
S2
click S2 href "https://example.com""#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    assert_eq!(
        res.model["links"]["S2"]["url"],
        json!("https://example.com")
    );
    assert_eq!(res.model["links"]["S2"]["tooltip"], json!(""));
}

#[test]
fn parse_diagram_state_v2_note_right_of_and_block_note() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
[*] --> A
note right of A : This is a note"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    assert_eq!(
        res.model["states"]["A"]["note"]["position"],
        json!("right of")
    );
    assert_eq!(
        res.model["states"]["A"]["note"]["text"],
        json!("This is a note")
    );

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
[*] --> A
note right of A
  line1
  line2
end note"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    let note_text = res.model["states"]["A"]["note"]["text"].as_str().unwrap();
    assert_eq!(note_text, "line1\nline2");

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram
foo: bar
note "This is a floating note" as N1"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    // Mermaid `@11.12.2` treats floating notes as a no-op in state diagrams.
    // (See upstream `stateDiagram floating notes` specs.)
    assert!(res.model["states"].get("N1").is_none());
}

#[test]
fn parse_diagram_state_v2_getdata_edges_and_note_edges() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
A --> B: hello"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();

    assert_eq!(res.model["edges"][0]["start"], json!("A"));
    assert_eq!(res.model["edges"][0]["end"], json!("B"));
    assert_eq!(res.model["edges"][0]["label"], json!("hello"));
    assert_eq!(res.model["edges"][0]["arrowhead"], json!("normal"));

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
[*] --> A
note left of A : note text"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();

    let note_edge = res.model["edges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["arrowhead"] == json!("none"))
        .unwrap();
    assert_eq!(note_edge["classes"], json!("transition note-edge"));
}

#[test]
fn parse_diagram_state_v2_uses_neo_arrow_type_when_look_is_neo() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram(
        r#"%%{init: {"look": "neo"}}%%
stateDiagram-v2
A --> B: hello"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();

    assert_eq!(
        res.model["edges"][0]["arrowTypeEnd"],
        json!("arrow_barb_neo")
    );
}

#[test]
fn parse_diagram_state_v2_sanitizes_edge_labels_like_mermaid_common() {
    let engine = Engine::new();
    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
A --> B: hello<script>alert(1)</script>world"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    assert_eq!(res.model["edges"][0]["label"], json!("helloworld"));
}

#[test]
fn parse_diagram_state_v2_getdata_dom_id_counter_and_note_padding_match_mermaid() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram(
        r#"stateDiagram-v2
A --> B
note right of A : note text"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();

    let nodes = res.model["nodes"].as_array().unwrap();
    let node_a = nodes.iter().find(|n| n["id"] == json!("A")).unwrap();
    let node_b = nodes.iter().find(|n| n["id"] == json!("B")).unwrap();
    let note_group = nodes
        .iter()
        .find(|n| n["id"] == json!("A----parent"))
        .unwrap();
    let note_node = nodes
        .iter()
        .find(|n| n["id"] == json!("A----note-1"))
        .unwrap();

    assert_eq!(node_a["domId"], json!("state-A-1"));
    assert_eq!(node_b["domId"], json!("state-B-0"));
    assert_eq!(note_group["domId"], json!("state-A----parent-1"));
    assert_eq!(note_node["domId"], json!("state-A----note-1"));
    assert_eq!(note_group["padding"], json!(16));
    assert_eq!(note_node["padding"], json!(15));
    assert_eq!(note_node["parentId"], json!("A----parent"));
}

fn deep_state_composite_chain(depth: usize) -> String {
    let mut input = String::from("stateDiagram-v2\n");
    for level in 0..depth {
        input.push_str(&format!("state S{level} {{\n"));
    }
    input.push_str("Leaf\n");
    for _ in 0..depth {
        input.push_str("}\n");
    }
    input
}

#[test]
fn state_deep_composite_chain_semantic_and_render_model_use_heap_traversal() {
    const DEPTH: usize = 1200;
    let input = deep_state_composite_chain(DEPTH);
    let engine = Engine::new();

    let parsed = block_on(engine.parse_diagram(&input, ParseOptions::strict()))
        .expect("parse ok")
        .expect("diagram detected");
    assert_eq!(parsed.meta.diagram_type, "stateDiagram");
    assert!(parsed.model["states"]["S0"]["doc"].is_array());
    assert!(
        parsed.model["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .any(|node| node["id"] == json!("Leaf"))
    );

    let parsed = engine
        .parse_diagram_for_render_model_sync(&input, ParseOptions::strict())
        .expect("render model parse ok")
        .expect("diagram detected");
    assert_eq!(parsed.meta.diagram_type, "stateDiagram");
}

#[test]
fn parse_state_editor_facts_preserve_parser_state_spans() {
    let engine = Engine::new();
    let text = r#"stateDiagram-v2
[*] --> Idle
Idle --> Running
Idle: Waiting state
Idle --> Running: starts
state Running {
  [*] --> Active
}
state "Paused State" as Paused
note right of Running : Running details
note "Floating note" as note1
classDef activeStyle fill:#0f0,border:#333
class Idle, Running activeStyle
style Running fill:#f00
accTitle: Lifecycle chart
accDescr: Shows state transitions
click Running "https://example.com/run" "Run details""#;
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("stateDiagram", text, ParseOptions::strict())
        .unwrap()
        .expect("state editor facts");

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);

    let symbol_at = |name: &str, start: usize| {
        facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == name && symbol.selection.start == start)
            .unwrap_or_else(|| panic!("missing symbol {name} at {start}"))
    };

    let idle_start = text.find("Idle").unwrap();
    assert_eq!(
        symbol_at("Idle", idle_start).selection.end,
        idle_start + "Idle".len()
    );

    let running_start = text.find("Running").unwrap();
    assert_eq!(
        symbol_at("Running", running_start).selection.end,
        running_start + "Running".len()
    );

    let idle_relation_source_start = text.find("Idle --> Running").unwrap();
    assert!(facts.symbols.iter().any(|symbol| {
        symbol.name == "Idle"
            && symbol.detail.as_deref() == Some("state reference")
            && symbol.selection.start == idle_relation_source_start
    }));

    let running_relation_target_start = text.find("Idle --> Running").unwrap() + "Idle --> ".len();
    assert!(facts.symbols.iter().any(|symbol| {
        symbol.name == "Running"
            && symbol.detail.as_deref() == Some("state reference")
            && symbol.selection.start == running_relation_target_start
    }));

    let active_start = text.find("Active").unwrap();
    assert_eq!(
        symbol_at("Active", active_start).selection.end,
        active_start + "Active".len()
    );

    let paused_start = text.rfind("Paused").unwrap();
    assert_eq!(
        symbol_at("Paused", paused_start).selection.end,
        paused_start + "Paused".len()
    );

    let display_label = facts
        .symbols
        .iter()
        .find(|symbol| {
            symbol.name == "Paused State" && symbol.detail.as_deref() == Some("state display label")
        })
        .unwrap();
    assert_eq!(display_label.role, EditorSemanticRole::Payload);

    let state_description = facts
        .symbols
        .iter()
        .find(|symbol| {
            symbol.name == "Waiting state" && symbol.detail.as_deref() == Some("state description")
        })
        .unwrap();
    assert_eq!(state_description.role, EditorSemanticRole::Payload);

    let relation_label = facts
        .symbols
        .iter()
        .find(|symbol| {
            symbol.name == "starts" && symbol.detail.as_deref() == Some("state relation label")
        })
        .unwrap();
    assert_eq!(relation_label.role, EditorSemanticRole::Payload);

    let positioned_note = facts
        .symbols
        .iter()
        .find(|symbol| {
            symbol.name == "Running details" && symbol.detail.as_deref() == Some("state note")
        })
        .unwrap();
    assert_eq!(positioned_note.role, EditorSemanticRole::Payload);

    let floating_note = facts
        .symbols
        .iter()
        .find(|symbol| {
            symbol.name == "Floating note" && symbol.detail.as_deref() == Some("state note")
        })
        .unwrap();
    assert_eq!(floating_note.role, EditorSemanticRole::Payload);
    assert!(!facts.symbols.iter().any(|symbol| symbol.name == "note1"));

    let active_style_start = text.find("activeStyle").unwrap();
    let active_style = facts
        .symbols
        .iter()
        .find(|symbol| {
            symbol.name == "activeStyle"
                && symbol.detail.as_deref() == Some("state class definition")
        })
        .unwrap();
    assert_eq!(active_style.role, EditorSemanticRole::Outline);
    assert_eq!(active_style.selection.start, active_style_start);

    let idle_class_target = facts
        .symbols
        .iter()
        .find(|symbol| {
            symbol.name == "Idle" && symbol.detail.as_deref() == Some("state class target")
        })
        .unwrap();
    assert_eq!(idle_class_target.role, EditorSemanticRole::Entity);

    let running_style = facts
        .symbols
        .iter()
        .find(|symbol| {
            symbol.name == "fill:#f00" && symbol.detail.as_deref() == Some("state style")
        })
        .unwrap();
    assert_eq!(running_style.role, EditorSemanticRole::Payload);
    assert!(running_style.selection.start > running_style.span.start);

    let acc_title = facts
        .symbols
        .iter()
        .find(|symbol| symbol.name == "Lifecycle chart")
        .unwrap();
    assert_eq!(acc_title.role, EditorSemanticRole::Payload);
    assert_eq!(
        acc_title.detail.as_deref(),
        Some("state accessibility title")
    );

    let acc_descr = facts
        .symbols
        .iter()
        .find(|symbol| symbol.name == "Shows state transitions")
        .unwrap();
    assert_eq!(acc_descr.role, EditorSemanticRole::Payload);
    assert_eq!(
        acc_descr.detail.as_deref(),
        Some("state accessibility description")
    );

    let click_url = facts
        .symbols
        .iter()
        .find(|symbol| symbol.name == "https://example.com/run")
        .unwrap();
    assert_eq!(click_url.role, EditorSemanticRole::Payload);
    assert_eq!(click_url.detail.as_deref(), Some("state click url"));

    let click_tooltip = facts
        .symbols
        .iter()
        .find(|symbol| symbol.name == "Run details")
        .unwrap();
    assert_eq!(click_tooltip.role, EditorSemanticRole::Payload);
    assert_eq!(click_tooltip.detail.as_deref(), Some("state click tooltip"));
}

#[test]
fn parse_state_editor_facts_record_expected_syntax_spans() {
    let engine = Engine::new();
    let text = concat!(
        "stateDiagram-v2\n",
        "state \"Small State\" as namedState\n",
        "namedState: Waiting state\n",
        "classDef exampleStyleClass background:#bbb,border:1px solid red\n",
        "a --> b:::exampleStyleClass\n",
        "class namedState exampleStyleClass\n",
        "style namedState fill:#f00\n",
        "click namedState \"https://example.com/run\" \"Run details\"",
    );
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("stateDiagram", text, ParseOptions::strict())
        .unwrap()
        .expect("state editor facts");

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);

    assert_expected_syntax_covers(
        &facts,
        EditorExpectedSyntaxKind::Payload,
        text,
        "state \"Small State\" as namedState",
        "Small State",
        "state display label payload",
    );
    assert_expected_syntax_covers(
        &facts,
        EditorExpectedSyntaxKind::NodeIdentifier,
        text,
        "state \"Small State\" as namedState",
        "namedState",
        "state alias node identifier",
    );
    assert_expected_syntax_covers(
        &facts,
        EditorExpectedSyntaxKind::Payload,
        text,
        "namedState: Waiting state",
        "Waiting state",
        "state description payload",
    );
    assert_expected_syntax_covers(
        &facts,
        EditorExpectedSyntaxKind::Payload,
        text,
        "classDef exampleStyleClass background:#bbb,border:1px solid red",
        "exampleStyleClass",
        "state class definition payload",
    );
    assert_expected_syntax_covers(
        &facts,
        EditorExpectedSyntaxKind::NodeIdentifier,
        text,
        "a --> b:::exampleStyleClass",
        "b",
        "state inline class node identifier",
    );
    assert_expected_syntax_covers(
        &facts,
        EditorExpectedSyntaxKind::Payload,
        text,
        "a --> b:::exampleStyleClass",
        "exampleStyleClass",
        "state inline class payload",
    );
    assert_expected_syntax_covers(
        &facts,
        EditorExpectedSyntaxKind::IdList,
        text,
        "class namedState exampleStyleClass",
        "namedState",
        "state class target list",
    );
    assert_expected_syntax_covers(
        &facts,
        EditorExpectedSyntaxKind::IdList,
        text,
        "style namedState fill:#f00",
        "namedState",
        "state style target list",
    );
    assert_expected_syntax_covers(
        &facts,
        EditorExpectedSyntaxKind::Payload,
        text,
        "style namedState fill:#f00",
        "fill:#f00",
        "state style payload",
    );
    assert_expected_syntax_covers(
        &facts,
        EditorExpectedSyntaxKind::NodeIdentifier,
        text,
        "click namedState \"https://example.com/run\" \"Run details\"",
        "namedState",
        "state click target",
    );
    assert_expected_syntax_covers(
        &facts,
        EditorExpectedSyntaxKind::Payload,
        text,
        "click namedState \"https://example.com/run\" \"Run details\"",
        "https://example.com/run",
        "state click url",
    );
    assert_expected_syntax_covers(
        &facts,
        EditorExpectedSyntaxKind::Payload,
        text,
        "click namedState \"https://example.com/run\" \"Run details\"",
        "Run details",
        "state click tooltip",
    );
}

#[test]
fn parse_state_editor_facts_recovers_from_incomplete_input() {
    let engine = Engine::new();
    let text = "stateDiagram-v2\nIdle --> Running\nRunning -->";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("stateDiagram", text, ParseOptions::strict())
        .unwrap()
        .expect("state editor facts");

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert_eq!(facts.diagnostics.len(), 1);
    let diagnostic = &facts.diagnostics[0];
    assert!(diagnostic.message.contains("state parser recovered"));
    assert_eq!(
        diagnostic.span,
        Some(SourceSpan::new(text.len(), text.len()))
    );
    assert!(facts.symbols.iter().any(|symbol| symbol.name == "Idle"));
    assert!(facts.symbols.iter().any(|symbol| symbol.name == "Running"));
}

fn assert_expected_syntax_covers(
    facts: &EditorSemanticFacts,
    kind: EditorExpectedSyntaxKind,
    text: &str,
    marker: &str,
    target: &str,
    label: &str,
) {
    let marker_start = text
        .find(marker)
        .unwrap_or_else(|| panic!("missing {label} source text"));
    let target_start = text[marker_start..]
        .find(target)
        .map(|offset| marker_start + offset)
        .unwrap_or_else(|| panic!("missing {label} target"));
    let end = target_start + target.len();
    assert!(
        facts.expected_syntax.iter().any(|expected| {
            expected.kind == kind && expected.span.start <= target_start && expected.span.end >= end
        }),
        "missing {label}"
    );
}

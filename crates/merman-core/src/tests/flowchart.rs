use crate::*;
use futures::executor::block_on;
use serde_json::json;

#[test]
fn parse_diagram_flowchart_basic_graph() {
    let engine = Engine::new();
    let text = "graph TD;A-->B;";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.meta.diagram_type, "flowchart-v2");
    assert_eq!(
        res.model,
        json!({
            "type": "flowchart-v2",
            "keyword": "graph",
            "direction": "TB",
            "accTitle": null,
            "accDescr": null,
            "classDefs": {},
            "tooltips": {},
            "edgeDefaults": { "style": [], "interpolate": null },
            "vertexCalls": ["A", "B"],
            "nodes": [
                { "id": "A", "label": "A", "labelType": "text", "shape": null, "layoutShape": "squareRect", "icon": null, "form": null, "pos": null, "img": null, "constraint": null, "assetWidth": null, "assetHeight": null, "styles": [], "classes": [], "link": null, "linkTarget": null, "haveCallback": false },
                { "id": "B", "label": "B", "labelType": "text", "shape": null, "layoutShape": "squareRect", "icon": null, "form": null, "pos": null, "img": null, "constraint": null, "assetWidth": null, "assetHeight": null, "styles": [], "classes": [], "link": null, "linkTarget": null, "haveCallback": false }
            ],
            "edges": [
                { "from": "A", "to": "B", "id": "L_A_B_0", "isUserDefinedId": false, "arrow": "-->", "type": "arrow_point", "stroke": "normal", "length": 1, "label": null, "labelType": "text", "style": [], "classes": [], "interpolate": null, "animate": null, "animation": null }
            ],
            "subgraphs": []
        })
    );

    let typed = crate::diagrams::flowchart::parse_flowchart_model_for_render(text, &res.meta)
        .expect("flowchart typed model");
    assert_eq!(
        crate::diagrams::flowchart::render_model_to_compat_json(&typed, &res.meta)
            .expect("flowchart compatibility projection"),
        res.model,
        "Flowchart typed compatibility projection must preserve the exact public JSON"
    );
}

#[test]
fn parse_swimlane_reuses_flowchart_semantics_and_editor_facts() {
    let engine = Engine::new();
    let text = "swimlane-beta LR\nA[Start] --> B[Done]\n";
    let parsed = engine
        .parse_diagram_snapshot_sync(text)
        .unwrap()
        .expect("swimlane parses through flowchart semantics");

    assert_eq!(parsed.metadata().diagram_type, "swimlane");
    assert_eq!(
        parsed.metadata().effective_config.get_str("layout"),
        Some("swimlane")
    );
    assert_eq!(
        parsed
            .outcome()
            .parsed_model()
            .expect("expected parsed snapshot")["type"],
        json!("swimlane")
    );
    assert_eq!(
        parsed
            .outcome()
            .parsed_model()
            .expect("expected parsed snapshot")["keyword"],
        json!("swimlane-beta")
    );
    assert_eq!(
        parsed
            .outcome()
            .parsed_model()
            .expect("expected parsed snapshot")["direction"],
        json!("LR")
    );
    assert_eq!(
        parsed
            .outcome()
            .parsed_model()
            .expect("expected parsed snapshot")["nodes"][0]["id"],
        json!("A")
    );
    assert_eq!(
        parsed
            .outcome()
            .parsed_model()
            .expect("expected parsed snapshot")["edges"][0]["from"],
        json!("A")
    );

    let ParsedEditorFacts::Available(facts) = parsed.editor_facts() else {
        panic!("swimlane should reuse flowchart editor facts");
    };
    let a_start = text.find("A[").expect("A node");
    let a = facts
        .symbols
        .iter()
        .find(|symbol| symbol.name == "A")
        .expect("A editor symbol");
    assert_eq!(a.selection.start, a_start);
    assert_eq!(a.selection.end, a_start + "A".len());
}

#[test]
fn combined_flowchart_variants_construct_one_token_and_accessibility_trace() {
    let cases = [
        ("flowchart-v2", "flowchart TD"),
        ("flowchart", "graph TD"),
        ("flowchart-elk", "flowchart-elk TD"),
        ("swimlane", "swimlane-beta LR"),
    ];
    let engine = Engine::new();

    for (diagram_type, header) in cases {
        for (tail, should_parse) in [
            ("accTitle: One pass\nA --> B\n", true),
            ("accTitle: One pass\nA((\n", false),
        ] {
            crate::diagrams::flowchart::reset_flowchart_token_trace_construction_count();
            crate::diagrams::flowchart::reset_flowchart_accessibility_scan_count();
            let source = format!("{header}\n{tail}");
            let snapshot = engine
                .parse_diagram_snapshot_with_type_sync(diagram_type, &source)
                .unwrap()
                .expect("built-in Flowchart variant snapshot");

            assert_eq!(
                snapshot.outcome().parsed_model().is_some(),
                should_parse,
                "{diagram_type} strict parser outcome"
            );
            if !should_parse {
                let DiagramParseOutcome::Failed(error) = snapshot.outcome() else {
                    unreachable!("partial recovery token must not satisfy the strict parser");
                };
                assert!(error.to_string().contains("Unterminated node label"));
                let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                    panic!("failed Flowchart construction must retain editor facts");
                };
                assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            }
            assert_eq!(
                crate::diagrams::flowchart::flowchart_token_trace_construction_count(),
                1,
                "{diagram_type} token trace"
            );
            assert_eq!(
                crate::diagrams::flowchart::flowchart_accessibility_scan_count(),
                1,
                "{diagram_type} accessibility scan"
            );
        }
    }
}

#[test]
fn flowchart_accessibility_statements_emit_lexer_owned_unicode_lexemes() {
    let source = concat!(
        "flowchart TD\n",
        "  accTitle : 结账流程\n",
        "accDescr {\n  第一行\n  第二行\n}\n",
        "A --> B\n",
    );
    let snapshot = Engine::new()
        .parse_diagram_snapshot_with_type_sync("flowchart-v2", source)
        .unwrap()
        .expect("Flowchart snapshot");
    let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
        panic!("Flowchart editor facts");
    };
    let lexemes = facts
        .lexemes()
        .iter()
        .map(|lexeme| {
            let span = lexeme.span();
            (lexeme.kind(), &source[span.start..span.end])
        })
        .collect::<Vec<_>>();

    for keyword in ["accTitle", "accDescr"] {
        assert!(lexemes.contains(&(EditorLexemeKind::Keyword, keyword)));
    }
    for delimiter in [":", "{", "}"] {
        assert!(lexemes.contains(&(EditorLexemeKind::Delimiter, delimiter)));
    }
    for value in ["结账流程", "第一行\n  第二行"] {
        assert!(lexemes.contains(&(EditorLexemeKind::String, value)));
    }
}

#[test]
fn parse_swimlane_reuses_flowchart_apostrophe_semantics() {
    let engine = Engine::new();
    let text = "swimlane-beta LR\nsubgraph Supplier\nA[Update the RFQs based on the supplier's response]\nB[Done]\nend\nA -->|'Owner's review'| B\n";
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::strict()))
        .expect("Swimlane accepts apostrophes through the shared Flowchart parser")
        .expect("swimlane diagram detected");

    assert_eq!(parsed.meta.diagram_type, "swimlane");
    assert_eq!(
        parsed.model["nodes"][0]["label"],
        json!("Update the RFQs based on the supplier's response")
    );
    assert_eq!(parsed.model["nodes"][0]["labelType"], json!("text"));
    assert_eq!(parsed.model["edges"][0]["label"], json!("'Owner's review'"));
    assert_eq!(parsed.model["edges"][0]["labelType"], json!("text"));
}

#[test]
fn parse_swimlane_layout_default_respects_user_config_precedence() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
        "layout": "dagre"
    })));

    let site_default = engine
        .parse_metadata_sync("swimlane-beta LR\nA-->B\n")
        .expect("swimlane metadata");
    assert_eq!(
        site_default.effective_config.get_str("layout"),
        Some("swimlane")
    );

    let user_override = engine
        .parse_metadata_sync("%%{init: {\"layout\": \"elk\"}}%%\nswimlane-beta LR\nA-->B\n")
        .expect("swimlane metadata with user layout");
    assert_eq!(user_override.config.get_str("layout"), Some("elk"));
    assert_eq!(
        user_override.effective_config.get_str("layout"),
        Some("elk")
    );

    let cleared_override = engine
        .parse_metadata_sync("%%{init: {\"layout\": null}}%%\nswimlane-beta LR\nA-->B\n")
        .expect("swimlane metadata with a null layout override");
    assert_eq!(
        cleared_override.effective_config.get_str("layout"),
        Some("swimlane")
    );

    let known_type = engine
        .parse_metadata_with_type_sync("swimlane", "swimlane-beta LR\nA-->B\n")
        .expect("known-type swimlane metadata");
    assert_eq!(
        known_type.effective_config.get_str("layout"),
        Some("swimlane")
    );
}

#[test]
fn parse_swimlane_render_model_reuses_flowchart_semantics() {
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync("swimlane-beta LR\nA-->B\n", ParseOptions::strict())
        .expect("swimlane render parse succeeds")
        .expect("swimlane render model");

    assert_eq!(parsed.metadata().diagram_type, "swimlane");
    assert_eq!(
        parsed.metadata().effective_config.get_str("layout"),
        Some("swimlane")
    );
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("swimlane should reuse the flowchart semantic model");
    };
    assert_eq!(model.keyword, "swimlane-beta");
    assert_eq!(model.direction.as_deref(), Some("LR"));
    assert_eq!(model.nodes.len(), 2);
    assert_eq!(model.edges.len(), 1);
}

#[test]
fn parse_diagram_flowchart_rejects_non_grammar_acc_description_alias() {
    let snapshot = Engine::new()
        .parse_diagram_snapshot_with_type_sync(
            "flowchart-v2",
            "flowchart TD\naccDescription: Flow description\nA-->B\n",
        )
        .unwrap()
        .expect("Flowchart snapshot");

    assert!(matches!(snapshot.outcome(), DiagramParseOutcome::Failed(_)));
}

#[test]
fn parse_diagram_flowchart_tolerates_edge_labels() {
    let engine = Engine::new();
    let text = "graph TD;A--x|text including URL space|B;";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.meta.diagram_type, "flowchart-v2");
    assert_eq!(
        res.model["edges"][0],
        json!({
            "from": "A",
            "to": "B",
            "id": "L_A_B_0",
            "isUserDefinedId": false,
            "arrow": "--x",
            "type": "arrow_cross",
            "stroke": "normal",
            "length": 1,
            "label": "text including URL space",
            "labelType": "text",
            "style": [],
            "classes": [],
            "interpolate": null,
            "animate": null,
            "animation": null
        })
    );
    assert_eq!(res.model["subgraphs"], json!([]));
}

#[test]
fn parse_diagram_flowchart_supports_inline_nodes() {
    let engine = Engine::new();
    let text = "graph TD;A[Start]-->B{Is it?};";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.meta.diagram_type, "flowchart-v2");
    assert_eq!(
        res.model,
        json!({
            "type": "flowchart-v2",
            "keyword": "graph",
            "direction": "TB",
            "accTitle": null,
            "accDescr": null,
            "classDefs": {},
            "tooltips": {},
            "edgeDefaults": { "style": [], "interpolate": null },
            "vertexCalls": ["A", "B"],
            "nodes": [
                { "id": "A", "label": "Start", "labelType": "text", "shape": "square", "layoutShape": "squareRect", "icon": null, "form": null, "pos": null, "img": null, "constraint": null, "assetWidth": null, "assetHeight": null, "styles": [], "classes": [], "link": null, "linkTarget": null, "haveCallback": false },
                { "id": "B", "label": "Is it?", "labelType": "text", "shape": "diamond", "layoutShape": "diamond", "icon": null, "form": null, "pos": null, "img": null, "constraint": null, "assetWidth": null, "assetHeight": null, "styles": [], "classes": [], "link": null, "linkTarget": null, "haveCallback": false }
            ],
            "edges": [
                { "from": "A", "to": "B", "id": "L_A_B_0", "isUserDefinedId": false, "arrow": "-->", "type": "arrow_point", "stroke": "normal", "length": 1, "label": null, "labelType": "text", "style": [], "classes": [], "interpolate": null, "animate": null, "animation": null }
            ],
            "subgraphs": []
        })
    );
}

#[test]
fn parse_diagram_flowchart_allows_dashes_in_node_ids() {
    let engine = Engine::new();
    let text = r#"
flowchart
    wi-fi["a node with dashes in its name"]
"#;
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.meta.diagram_type, "flowchart-v2");
    assert_eq!(res.model["nodes"][0]["id"], json!("wi-fi"));
    assert_eq!(
        res.model["nodes"][0]["label"],
        json!("a node with dashes in its name")
    );
}

#[test]
fn parse_diagram_flowchart_supports_quoted_edge_labels_and_pipe_labels_with_whitespace() {
    let engine = Engine::new();
    let text = r#"
flowchart TD
A[Node 1] -- "Some text" --> B[Node 2]
B --> |Other text| C[Node 3]
"#;
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.meta.diagram_type, "flowchart-v2");
    let edges = res.model["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 2);
    assert_eq!(edges[0]["label"], json!("Some text"));
    assert_eq!(edges[0]["labelType"], json!("string"));
    assert_eq!(edges[1]["label"], json!("Other text"));
    assert_eq!(edges[1]["labelType"], json!("text"));
}

#[test]
fn parse_diagram_flowchart_edge_stroke_and_type_normal_thick_dotted() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram("graph TD;A-->B;", ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edges"][0]["type"], json!("arrow_point"));
    assert_eq!(res.model["edges"][0]["stroke"], json!("normal"));
    assert_eq!(res.model["edges"][0]["length"], json!(1));

    let res = block_on(engine.parse_diagram("graph TD;A==>B;", ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edges"][0]["type"], json!("arrow_point"));
    assert_eq!(res.model["edges"][0]["stroke"], json!("thick"));
    assert_eq!(res.model["edges"][0]["length"], json!(1));

    let res = block_on(engine.parse_diagram("graph TD;A-.->B;", ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edges"][0]["type"], json!("arrow_point"));
    assert_eq!(res.model["edges"][0]["stroke"], json!("dotted"));
    assert_eq!(res.model["edges"][0]["length"], json!(1));
}

#[test]
fn parse_diagram_flowchart_double_ended_arrows() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram("graph TD;A<-->B;", ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edges"][0]["type"], json!("double_arrow_point"));
    assert_eq!(res.model["edges"][0]["stroke"], json!("normal"));
    assert_eq!(res.model["edges"][0]["length"], json!(1));
}

#[test]
fn parse_diagram_flowchart_edge_text_new_notation() {
    let engine = Engine::new();
    let text = "graph TD;A-- text including URL space and send -->B;";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edges"][0]["type"], json!("arrow_point"));
    assert_eq!(
        res.model["edges"][0]["label"],
        json!("text including URL space and send")
    );
}

#[test]
fn parse_diagram_flowchart_edge_text_new_notation_double_ended() {
    let engine = Engine::new();
    let text = "graph TD;A<-- text -->B;";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edges"][0]["type"], json!("double_arrow_point"));
    assert_eq!(res.model["edges"][0]["label"], json!("text"));
}

#[test]
fn parse_diagram_flowchart_invisible_edge() {
    let engine = Engine::new();
    let res = block_on(engine.parse_diagram("graph TD;A~~~B;", ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edges"][0]["type"], json!("arrow_open"));
    assert_eq!(res.model["edges"][0]["stroke"], json!("invisible"));
    assert_eq!(res.model["edges"][0]["length"], json!(1));
}

#[test]
fn parse_diagram_flowchart_edges_spec_open_cross_circle() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram("graph TD;A---B;", ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edges"][0]["type"], json!("arrow_open"));
    assert_eq!(res.model["edges"][0]["stroke"], json!("normal"));
    assert_eq!(res.model["edges"][0]["length"], json!(1));

    let res = block_on(engine.parse_diagram("graph TD;A--xB;", ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edges"][0]["type"], json!("arrow_cross"));
    assert_eq!(res.model["edges"][0]["stroke"], json!("normal"));
    assert_eq!(res.model["edges"][0]["length"], json!(1));

    let res = block_on(engine.parse_diagram("graph TD;A--oB;", ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edges"][0]["type"], json!("arrow_circle"));
    assert_eq!(res.model["edges"][0]["stroke"], json!("normal"));
    assert_eq!(res.model["edges"][0]["length"], json!(1));
}

#[test]
fn parse_diagram_flowchart_edges_spec_edge_ids_and_node_metadata_do_not_conflict() {
    let engine = Engine::new();
    let text = "flowchart LR\nA id1@-->B\nA@{ shape: 'rect' }\n";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edges"][0]["id"], json!("id1"));
    assert_eq!(res.model["edges"][0]["type"], json!("arrow_point"));
}

#[test]
fn parse_diagram_flowchart_edges_spec_edge_length_matrix() {
    let engine = Engine::new();
    let assert_edge = |diagram: String,
                       expected_type: &str,
                       expected_stroke: &str,
                       expected_length: usize,
                       expected_label: Option<&str>| {
        let res = block_on(engine.parse_diagram(&diagram, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let e = &res.model["edges"][0];
        assert_eq!(e["type"], json!(expected_type), "diagram: {diagram}");
        assert_eq!(e["stroke"], json!(expected_stroke), "diagram: {diagram}");
        assert_eq!(e["length"], json!(expected_length), "diagram: {diagram}");
        match expected_label {
            Some(label) => assert_eq!(e["label"], json!(label), "diagram: {diagram}"),
            None => assert!(e["label"].is_null(), "diagram: {diagram}"),
        }
    };

    for length in 1..=3 {
        assert_edge(
            format!("graph TD;\nA -{}- B;", "-".repeat(length)),
            "arrow_open",
            "normal",
            length,
            None,
        );
        assert_edge(
            format!("graph TD;\nA -- Label -{}- B;", "-".repeat(length)),
            "arrow_open",
            "normal",
            length,
            Some("Label"),
        );
        assert_edge(
            format!("graph TD;\nA -{}> B;", "-".repeat(length)),
            "arrow_point",
            "normal",
            length,
            None,
        );
        assert_edge(
            format!("graph TD;\nA -- Label -{}> B;", "-".repeat(length)),
            "arrow_point",
            "normal",
            length,
            Some("Label"),
        );
        assert_edge(
            format!("graph TD;\nA <-{}> B;", "-".repeat(length)),
            "double_arrow_point",
            "normal",
            length,
            None,
        );
        assert_edge(
            format!("graph TD;\nA <-- Label -{}> B;", "-".repeat(length)),
            "double_arrow_point",
            "normal",
            length,
            Some("Label"),
        );
    }

    for length in 1..=3 {
        assert_edge(
            format!("graph TD;\nA ={}= B;", "=".repeat(length)),
            "arrow_open",
            "thick",
            length,
            None,
        );
        assert_edge(
            format!("graph TD;\nA == Label ={}= B;", "=".repeat(length)),
            "arrow_open",
            "thick",
            length,
            Some("Label"),
        );
        assert_edge(
            format!("graph TD;\nA ={}> B;", "=".repeat(length)),
            "arrow_point",
            "thick",
            length,
            None,
        );
        assert_edge(
            format!("graph TD;\nA == Label ={}> B;", "=".repeat(length)),
            "arrow_point",
            "thick",
            length,
            Some("Label"),
        );
        assert_edge(
            format!("graph TD;\nA <={}> B;", "=".repeat(length)),
            "double_arrow_point",
            "thick",
            length,
            None,
        );
        assert_edge(
            format!("graph TD;\nA <== Label ={}> B;", "=".repeat(length)),
            "double_arrow_point",
            "thick",
            length,
            Some("Label"),
        );
    }

    for length in 1..=3 {
        assert_edge(
            format!("graph TD;\nA -{}- B;", ".".repeat(length)),
            "arrow_open",
            "dotted",
            length,
            None,
        );
        assert_edge(
            format!("graph TD;\nA -. Label {}- B;", ".".repeat(length)),
            "arrow_open",
            "dotted",
            length,
            Some("Label"),
        );
        assert_edge(
            format!("graph TD;\nA -{}-> B;", ".".repeat(length)),
            "arrow_point",
            "dotted",
            length,
            None,
        );
        assert_edge(
            format!("graph TD;\nA -. Label {}-> B;", ".".repeat(length)),
            "arrow_point",
            "dotted",
            length,
            Some("Label"),
        );
        assert_edge(
            format!("graph TD;\nA <-{}-> B;", ".".repeat(length)),
            "double_arrow_point",
            "dotted",
            length,
            None,
        );
        assert_edge(
            format!("graph TD;\nA <-. Label {}-> B;", ".".repeat(length)),
            "double_arrow_point",
            "dotted",
            length,
            Some("Label"),
        );
    }
}

#[test]
fn parse_diagram_flowchart_edges_spec_keywords_as_edge_labels_in_double_ended_edges() {
    let engine = Engine::new();

    let keywords = [
        "graph",
        "flowchart",
        "flowchart-elk",
        "style",
        "default",
        "linkStyle",
        "interpolate",
        "classDef",
        "class",
        "href",
        "call",
        "click",
        "_self",
        "_blank",
        "_parent",
        "_top",
        "end",
        "subgraph",
        "kitty",
    ];

    let edges = [
        ("x--", "--x", "normal", "double_arrow_cross"),
        ("x==", "==x", "thick", "double_arrow_cross"),
        ("x-.", ".-x", "dotted", "double_arrow_cross"),
        ("o--", "--o", "normal", "double_arrow_circle"),
        ("o==", "==o", "thick", "double_arrow_circle"),
        ("o-.", ".-o", "dotted", "double_arrow_circle"),
        ("<--", "-->", "normal", "double_arrow_point"),
        ("<==", "==>", "thick", "double_arrow_point"),
        ("<-.", ".->", "dotted", "double_arrow_point"),
    ];

    for (edge_start, edge_end, stroke, edge_type) in edges {
        for keyword in keywords {
            let diagram = format!("graph TD;\nA {edge_start} {keyword} {edge_end} B;");
            let res = block_on(engine.parse_diagram(&diagram, ParseOptions::default()))
                .unwrap()
                .unwrap();
            let e = &res.model["edges"][0];
            assert_eq!(e["type"], json!(edge_type), "diagram: {diagram}");
            assert_eq!(e["stroke"], json!(stroke), "diagram: {diagram}");
            assert_eq!(e["label"], json!(keyword), "diagram: {diagram}");
            assert_eq!(e["labelType"], json!("text"), "diagram: {diagram}");
        }
    }
}

#[test]
fn parse_diagram_flowchart_node_data_basic_shape_data_statements() {
    let engine = Engine::new();

    let res = block_on(
        engine.parse_diagram("flowchart TB\nD@{ shape: rounded}", ParseOptions::default()),
    )
    .unwrap()
    .unwrap();
    let nodes = res.model["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["id"], json!("D"));
    assert_eq!(nodes[0]["layoutShape"], json!("rounded"));
    assert_eq!(nodes[0]["label"], json!("D"));

    let res = block_on(engine.parse_diagram(
        "flowchart TB\nD@{ shape: rounded }",
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    let nodes = res.model["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["layoutShape"], json!("rounded"));
    assert_eq!(nodes[0]["label"], json!("D"));
}

#[test]
fn parse_diagram_flowchart_node_data_shape_data_with_amp_and_edges() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram(
        "flowchart TB\nD@{ shape: rounded } & E",
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    let nodes = res.model["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0]["id"], json!("D"));
    assert_eq!(nodes[0]["layoutShape"], json!("rounded"));
    assert_eq!(nodes[0]["label"], json!("D"));
    assert_eq!(nodes[1]["id"], json!("E"));
    assert_eq!(nodes[1]["label"], json!("E"));

    let res = block_on(engine.parse_diagram(
        "flowchart TB\nD@{ shape: rounded } --> E",
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    let nodes = res.model["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0]["id"], json!("D"));
    assert_eq!(nodes[0]["layoutShape"], json!("rounded"));
    assert_eq!(nodes[1]["id"], json!("E"));
}

#[test]
fn parse_diagram_flowchart_node_data_shape_data_whitespace_variants() {
    let engine = Engine::new();

    for diagram in [
        "flowchart TB\nD@{shape: rounded}",
        "flowchart TB\nD@{       shape: rounded}",
        "flowchart TB\nD@{ shape: rounded         }",
    ] {
        let res = block_on(engine.parse_diagram(diagram, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1, "diagram: {diagram}");
        assert_eq!(nodes[0]["id"], json!("D"), "diagram: {diagram}");
        assert_eq!(
            nodes[0]["layoutShape"],
            json!("rounded"),
            "diagram: {diagram}"
        );
        assert_eq!(nodes[0]["label"], json!("D"), "diagram: {diagram}");
    }
}

#[test]
fn parse_diagram_flowchart_node_data_shape_data_accepts_datastore() {
    let engine = Engine::new();

    for shape in ["datastore", "data-store"] {
        let diagram = format!("flowchart TB\nD@{{ shape: {shape}, label: \"Datastore\" }}");
        let res = block_on(engine.parse_diagram(&diagram, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1, "diagram: {diagram}");
        assert_eq!(nodes[0]["id"], json!("D"), "diagram: {diagram}");
        assert_eq!(nodes[0]["layoutShape"], json!(shape), "diagram: {diagram}");
        assert_eq!(nodes[0]["label"], json!("Datastore"), "diagram: {diagram}");
        assert_eq!(
            nodes[0]["labelType"],
            json!("markdown"),
            "diagram: {diagram}"
        );
    }
}

#[test]
fn parse_diagram_flowchart_node_data_shape_data_accepts_document_variants() {
    let engine = Engine::new();

    for shape in [
        "doc",
        "document",
        "docs",
        "documents",
        "st-doc",
        "stacked-document",
        "lin-doc",
        "lined-document",
        "tag-doc",
        "tagged-document",
    ] {
        let diagram = format!("flowchart TB\nD@{{ shape: {shape}, label: \"Doc\" }}");
        let res = block_on(engine.parse_diagram(&diagram, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1, "diagram: {diagram}");
        assert_eq!(nodes[0]["id"], json!("D"), "diagram: {diagram}");
        assert_eq!(nodes[0]["layoutShape"], json!(shape), "diagram: {diagram}");
        assert_eq!(nodes[0]["label"], json!("Doc"), "diagram: {diagram}");
    }
}

#[test]
fn parse_diagram_flowchart_node_data_shape_data_accepts_process_variants() {
    let engine = Engine::new();

    for shape in [
        "st-rect",
        "stacked-rectangle",
        "processes",
        "procs",
        "tag-rect",
        "tag-proc",
        "tagged-process",
        "tagged-rectangle",
        "lin-rect",
        "lin-proc",
        "lined-process",
        "lined-rectangle",
        "shaded-process",
    ] {
        let diagram = format!("flowchart TB\nP@{{ shape: {shape}, label: \"Proc\" }}");
        let res = block_on(engine.parse_diagram(&diagram, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), 1, "diagram: {diagram}");
        assert_eq!(nodes[0]["id"], json!("P"), "diagram: {diagram}");
        assert_eq!(nodes[0]["layoutShape"], json!(shape), "diagram: {diagram}");
        assert_eq!(nodes[0]["label"], json!("Proc"), "diagram: {diagram}");
    }
}

#[test]
fn parse_diagram_flowchart_node_data_shape_data_amp_and_edge_matrix() {
    let engine = Engine::new();

    let cases = [
        (
            "flowchart TB\nD@{ shape: rounded } & E --> F",
            3usize,
            "D",
            "rounded",
        ),
        (
            "flowchart TB\nD@{ shape: rounded } & E@{ shape: rounded } --> F",
            3usize,
            "D",
            "rounded",
        ),
        (
            "flowchart TB\nD@{ shape: rounded } & E@{ shape: rounded } --> F & G@{ shape: rounded }",
            4usize,
            "D",
            "rounded",
        ),
        (
            "flowchart TB\nD@{ shape: rounded } & E@{ shape: rounded } --> F@{ shape: rounded } & G@{ shape: rounded }",
            4usize,
            "D",
            "rounded",
        ),
        (
            "flowchart TB\nD@{ shape: rounded } & E@{ shape: rounded } --> F{ shape: rounded } & G{ shape: rounded }    ",
            4usize,
            "D",
            "rounded",
        ),
    ];

    for (diagram, expected_nodes, first_id, first_layout) in cases {
        let res = block_on(engine.parse_diagram(diagram, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let nodes = res.model["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), expected_nodes, "diagram: {diagram}");
        assert_eq!(nodes[0]["id"], json!(first_id), "diagram: {diagram}");
        assert_eq!(
            nodes[0]["layoutShape"],
            json!(first_layout),
            "diagram: {diagram}"
        );
    }
}

#[test]
fn parse_diagram_flowchart_node_data_shape_data_allows_brace_in_multiline_string() {
    let engine = Engine::new();

    let text = r#"flowchart TB
A@{
  label: "This is }"
  other: "clock"
}
"#;
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let nodes = res.model["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["label"], json!("This is }"));
}

#[test]
fn parse_diagram_flowchart_node_data_multiple_properties_same_line() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram(
        "flowchart TB\nD@{ shape: rounded , label: \"DD\"}",
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    let nodes = res.model["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["id"], json!("D"));
    assert_eq!(nodes[0]["layoutShape"], json!("rounded"));
    assert_eq!(nodes[0]["label"], json!("DD"));
    assert_eq!(nodes[0]["labelType"], json!("markdown"));
}

#[test]
fn parse_diagram_flowchart_node_data_label_type_defaults_to_markdown_but_can_be_overridden() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram(
        "flowchart TB\nA@{ label: \"Default markdown\" }\nB@{ label: \"Plain text\", labelType: text }",
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    let nodes = res.model["nodes"].as_array().unwrap();
    let find = |id: &str| nodes.iter().find(|n| n["id"] == json!(id)).unwrap();
    assert_eq!(find("A")["labelType"], json!("markdown"));
    assert_eq!(find("B")["labelType"], json!("text"));
}

#[test]
fn parse_diagram_flowchart_node_data_link_to_node_with_more_data_multiline_yaml() {
    let engine = Engine::new();

    let text = r#"flowchart TB
A --> D@{
  shape: circle
  other: "clock"
}
"#;
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let nodes = res.model["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0]["id"], json!("A"));
    assert_eq!(nodes[0]["layoutShape"], json!("squareRect"));
    assert_eq!(nodes[0]["label"], json!("A"));
    assert_eq!(nodes[1]["id"], json!("D"));
    assert_eq!(nodes[1]["layoutShape"], json!("circle"));
    assert_eq!(nodes[1]["label"], json!("D"));
    assert_eq!(res.model["edges"].as_array().unwrap().len(), 1);
}

#[test]
fn parse_diagram_flowchart_node_data_nodes_after_each_other() {
    let engine = Engine::new();
    let text = r#"flowchart TB
A[hello]
B@{
  shape: circle
  other: "clock"
}
C[Hello]@{
  shape: circle
  other: "clock"
}
"#;
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let nodes = res.model["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes[0]["id"], json!("A"));
    assert_eq!(nodes[0]["label"], json!("hello"));
    assert_eq!(nodes[0]["layoutShape"], json!("squareRect"));
    assert_eq!(nodes[1]["id"], json!("B"));
    assert_eq!(nodes[1]["label"], json!("B"));
    assert_eq!(nodes[1]["layoutShape"], json!("circle"));
    assert_eq!(nodes[2]["id"], json!("C"));
    assert_eq!(nodes[2]["label"], json!("Hello"));
    assert_eq!(nodes[2]["layoutShape"], json!("circle"));
}

#[test]
fn parse_diagram_flowchart_node_data_shape_data_allows_brace_and_at_in_strings() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram(
        "flowchart TB\nA@{ label: \"This is }\" }",
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    let nodes = res.model["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["layoutShape"], json!("squareRect"));
    assert_eq!(nodes[0]["label"], json!("This is }"));

    let res = block_on(engine.parse_diagram(
        "flowchart TB\nA@{ label: \"This is a string with @\" }",
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    let nodes = res.model["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["label"], json!("This is a string with @"));
}

#[test]
fn parse_diagram_flowchart_node_data_shape_data_rejects_internal_only_shape_variants() {
    let engine = Engine::new();

    for shape in [
        "forkJoin",
        "stateStart",
        "stateEnd",
        "rect_left_inv_arrow",
        "iconSquare",
        "iconCircle",
        "iconRounded",
        "imageSquare",
    ] {
        let diagram = format!("flowchart TB\nA@{{ shape: {shape}, label: \"Internal\" }}");
        let err = block_on(engine.parse_diagram(&diagram, ParseOptions::default())).unwrap_err();
        assert!(
            err.to_string().contains(&format!(
                "No such shape: {shape}. Shape names should be lowercase."
            )),
            "diagram: {diagram}\nerror: {err}"
        );
    }
}

#[test]
fn parse_diagram_flowchart_node_data_shape_validation_errors() {
    let engine = Engine::new();

    let err = block_on(engine.parse_diagram(
        "flowchart TB\nA@{ shape: this-shape-does-not-exist }",
        ParseOptions::default(),
    ))
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("No such shape: this-shape-does-not-exist.")
    );

    let err = block_on(engine.parse_diagram(
        "flowchart TB\nA@{ shape: rect_left_inv_arrow }",
        ParseOptions::default(),
    ))
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("No such shape: rect_left_inv_arrow. Shape names should be lowercase.")
    );
}

#[test]
fn parse_diagram_flowchart_node_data_multiline_strings_match_mermaid() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram(
        r#"flowchart TB
A@{
  label: |
    This is a
    multiline string
  other: "clock"
}
"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    let nodes = res.model["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["label"], json!("This is a\nmultiline string\n"));

    let res = block_on(engine.parse_diagram(
        r#"flowchart TB
A@{
  label: "This is a
    multiline string"
  other: "clock"
}
"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    let nodes = res.model["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0]["label"], json!("This is a<br/>multiline string"));
}

#[test]
fn parse_diagram_flowchart_node_data_labels_across_multi_nodes_and_edges() {
    let engine = Engine::new();

    let text = r#"flowchart TB
n2["label for n2"] & n4@{ label: "label for n4"} & n5@{ label: "label for n5"}
"#;
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let nodes = res.model["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes[0]["label"], json!("label for n2"));
    assert_eq!(nodes[1]["label"], json!("label for n4"));
    assert_eq!(nodes[2]["label"], json!("label for n5"));

    let text = r#"flowchart TD
A["A"] --> B["for B"] & C@{ label: "for c"} & E@{label : "for E"}
D@{label: "for D"}
"#;
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let nodes = res.model["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 5);
    assert_eq!(nodes[0]["label"], json!("A"));
    assert_eq!(nodes[1]["label"], json!("for B"));
    assert_eq!(nodes[2]["label"], json!("for c"));
    assert_eq!(nodes[3]["label"], json!("for E"));
    assert_eq!(nodes[4]["label"], json!("for D"));
}

#[test]
fn parse_diagram_flowchart_node_data_allows_at_in_labels_across_shapes() {
    let engine = Engine::new();

    let text = r#"flowchart TD
A["@A@"] --> B["@for@ B@"] & C@{ label: "@for@ c@"} & E{"`@for@ E@`"} & D(("@for@ D@"))
H1{{"@for@ H@"}}
H2{{"`@for@ H@`"}}
Q1{"@for@ Q@"}
Q2{"`@for@ Q@`"}
AS1>"@for@ AS@"]
AS2>"`@for@ AS@`"]
"#;
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let nodes = res.model["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 11);
    for (i, node) in nodes.iter().enumerate() {
        assert!(
            node["label"].as_str().unwrap().contains("@for@") || node["label"] == json!("@A@"),
            "node {i}: {:?}",
            node
        );
    }
    assert_eq!(nodes[0]["label"], json!("@A@"));
    assert_eq!(nodes[1]["label"], json!("@for@ B@"));
    assert_eq!(nodes[2]["label"], json!("@for@ c@"));
    assert_eq!(nodes[3]["label"], json!("@for@ E@"));
    assert_eq!(nodes[4]["label"], json!("@for@ D@"));
    assert_eq!(nodes[5]["label"], json!("@for@ H@"));
    assert_eq!(nodes[6]["label"], json!("@for@ H@"));
    assert_eq!(nodes[7]["label"], json!("@for@ Q@"));
    assert_eq!(nodes[8]["label"], json!("@for@ Q@"));
    assert_eq!(nodes[9]["label"], json!("@for@ AS@"));
    assert_eq!(nodes[10]["label"], json!("@for@ AS@"));
}

#[test]
fn parse_diagram_flowchart_node_data_unique_edge_ids_with_groups() {
    let engine = Engine::new();

    let text = r#"flowchart TD
A & B e1@--> C & D
A1 e2@--> C1 & D1
"#;
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["nodes"].as_array().unwrap().len(), 7);
    let edges = res.model["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 6);
    assert_eq!(edges[0]["id"], json!("L_A_C_0"));
    assert_eq!(edges[1]["id"], json!("L_A_D_0"));
    assert_eq!(edges[2]["id"], json!("e1"));
    assert_eq!(edges[3]["id"], json!("L_B_D_0"));
    assert_eq!(edges[4]["id"], json!("e2"));
    assert_eq!(edges[5]["id"], json!("L_A1_D1_0"));
}

#[test]
fn parse_diagram_flowchart_node_data_redefined_edge_id_becomes_auto_id() {
    let engine = Engine::new();

    let text = r#"flowchart TD
A & B e1@--> C & D
A1 e1@--> C1 & D1
"#;
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let edges = res.model["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 6);
    assert_eq!(edges[0]["id"], json!("L_A_C_0"));
    assert_eq!(edges[1]["id"], json!("L_A_D_0"));
    assert_eq!(edges[2]["id"], json!("e1"));
    assert_eq!(edges[3]["id"], json!("L_B_D_0"));
    assert_eq!(edges[4]["id"], json!("L_A1_C1_0"));
    assert_eq!(edges[5]["id"], json!("L_A1_D1_0"));
}

#[test]
fn parse_diagram_flowchart_node_data_overrides_edge_animate() {
    let engine = Engine::new();

    let text = r#"flowchart TD
A e1@--> B
C e2@--> D
E e3@--> F
e1@{ animate: true }
e2@{ animate: false }
e3@{ animate: true }
e3@{ animate: false }
"#;
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let edges = res.model["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 3);
    assert_eq!(edges[0]["id"], json!("e1"));
    assert_eq!(edges[0]["animate"], json!(true));
    assert_eq!(edges[1]["id"], json!("e2"));
    assert_eq!(edges[1]["animate"], json!(false));
    assert_eq!(edges[2]["id"], json!("e3"));
    assert_eq!(edges[2]["animate"], json!(false));
}

#[test]
fn parse_diagram_flowchart_markdown_strings_in_nodes_and_edges() {
    let engine = Engine::new();
    let text = "flowchart\nA[\"`The cat in **the** hat`\"]-- \"`The *bat* in the chat`\" -->B[\"The dog in the hog\"] -- \"The rat in the mat\" -->C;";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();

    let nodes = res.model["nodes"].as_array().unwrap();
    let find_node = |id: &str| nodes.iter().find(|n| n["id"] == json!(id)).unwrap();
    let node_a = find_node("A");
    let node_b = find_node("B");

    assert_eq!(node_a["label"], json!("The cat in **the** hat"));
    assert_eq!(node_a["labelType"], json!("markdown"));
    assert_eq!(node_b["label"], json!("The dog in the hog"));
    assert_eq!(node_b["labelType"], json!("string"));

    let edges = res.model["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 2);
    assert_eq!(edges[0]["from"], json!("A"));
    assert_eq!(edges[0]["to"], json!("B"));
    assert_eq!(edges[0]["type"], json!("arrow_point"));
    assert_eq!(edges[0]["label"], json!("The *bat* in the chat"));
    assert_eq!(edges[0]["labelType"], json!("markdown"));
    assert_eq!(edges[1]["from"], json!("B"));
    assert_eq!(edges[1]["to"], json!("C"));
    assert_eq!(edges[1]["type"], json!("arrow_point"));
    assert_eq!(edges[1]["label"], json!("The rat in the mat"));
    assert_eq!(edges[1]["labelType"], json!("string"));
}

#[test]
fn parse_diagram_flowchart_edge_text_matches_pinned_jison_states() {
    let engine = Engine::new();
    let source = concat!(
        "flowchart TD\n",
        "A -- \"foo --> bar\" --> B\n",
        "B -- \"`foo --> bar`\" --> C\n",
        "C\n\n\u{00A0}--> D\n",
        "D -- a-b --> E\n",
        "E == \"a=b\" ==> F\n",
        "F -. \"a.b\" .-> G\n",
        "G -->|\"a|b\"| H\n",
        "H --\u{0085}--> I\n",
        "I -- \" \" --> J\n",
        "J -- \"` `\" --> K\n",
    );
    let parsed = block_on(engine.parse_diagram(source, ParseOptions::strict()))
        .expect("valid pinned-Jison edge text forms")
        .expect("diagram detected");
    let edges = parsed.model["edges"].as_array().expect("edges");
    let edge = |from: &str| {
        edges
            .iter()
            .find(|edge| edge["from"] == json!(from))
            .unwrap_or_else(|| panic!("missing edge from {from}"))
    };

    assert_eq!(edge("A")["label"], json!("foo --> bar"));
    assert_eq!(edge("A")["labelType"], json!("string"));
    assert_eq!(edge("B")["label"], json!("foo --> bar"));
    assert_eq!(edge("B")["labelType"], json!("markdown"));
    assert_eq!(edge("C")["to"], json!("D"));
    assert_eq!(edge("C")["label"], json!(null));
    assert_eq!(edge("D")["label"], json!("a-b"));
    assert_eq!(edge("E")["label"], json!("a=b"));
    assert_eq!(edge("F")["label"], json!("a.b"));
    assert_eq!(edge("G")["label"], json!("a|b"));
    assert_eq!(edge("H")["label"], json!("\u{0085}"));
    assert_eq!(edge("I")["label"], json!(""));
    assert_eq!(edge("J")["label"], json!(""));

    for invalid in [
        "flowchart TD\nA -- --> B\n",
        "flowchart TD\nA --\u{00A0}--> B\n",
        "flowchart TD\nA -- \"\" --> B\n",
        "flowchart TD\nA -- \"``\" --> B\n",
        "flowchart TD\nA -- \"a\"\"b\" --> B\n",
        "flowchart TD\nA -- a--b --> B\n",
        "flowchart TD\nA == a=b ==> B\n",
        "flowchart TD\nA -. a.b .-> B\n",
    ] {
        assert!(
            block_on(engine.parse_diagram(invalid, ParseOptions::strict())).is_err(),
            "pinned Mermaid rejects {invalid:?}",
        );
    }
}

#[test]
fn parse_diagram_flowchart_edge_text_skips_long_internal_whitespace_runs() {
    let engine = Engine::new();
    let whitespace = "\u{00a0}".repeat(4096);
    let source = format!("flowchart TD\nA -- prefix{whitespace}suffix --> B\n");
    let parsed = block_on(engine.parse_diagram(&source, ParseOptions::strict()))
        .expect("long internal whitespace must parse")
        .expect("diagram detected");
    assert_eq!(
        parsed.model["edges"][0]["label"],
        json!(format!("prefix{whitespace}suffix"))
    );
}

#[test]
fn parse_diagram_flowchart_plain_node_labels_can_span_indented_lines() {
    let engine = Engine::new();
    let text = "     flowchart TB
     foo[**Bold Foo**] --> bar
     bar[Multiline
     bar]";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();

    assert_eq!(res.meta.diagram_type, "flowchart-v2");
    assert_eq!(res.model["keyword"], json!("flowchart"));
    assert_eq!(res.model["direction"], json!("TB"));

    let nodes = res.model["nodes"].as_array().unwrap();
    let find_node = |id: &str| nodes.iter().find(|n| n["id"] == json!(id)).unwrap();
    assert_eq!(find_node("foo")["label"], json!("**Bold Foo**"));
    assert_eq!(find_node("foo")["labelType"], json!("text"));
    assert_eq!(find_node("bar")["label"], json!("Multiline\n     bar"));
    assert_eq!(find_node("bar")["labelType"], json!("text"));

    let edges = res.model["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0]["from"], json!("foo"));
    assert_eq!(edges[0]["to"], json!("bar"));
}

#[test]
fn parse_diagram_flowchart_apostrophes_are_plain_text() {
    let engine = Engine::new();
    let text = "flowchart TD\nA[Reviews the supplier's response]\nB[[Owner's task]]\nC['Literal quotes']\nD[bare `tick` text]\nA -->|'Known issue'| B --> C --> D\n";
    let res = block_on(engine.parse_diagram(text, ParseOptions::strict()))
        .expect("apostrophes are ordinary characters in the Jison text state")
        .expect("diagram detected");

    let nodes = res.model["nodes"].as_array().unwrap();
    let find_node = |id: &str| nodes.iter().find(|node| node["id"] == json!(id)).unwrap();
    assert_eq!(
        find_node("A")["label"],
        json!("Reviews the supplier's response")
    );
    assert_eq!(find_node("A")["labelType"], json!("text"));
    assert_eq!(find_node("B")["label"], json!("Owner's task"));
    assert_eq!(find_node("B")["labelType"], json!("text"));
    assert_eq!(find_node("C")["label"], json!("'Literal quotes'"));
    assert_eq!(find_node("C")["labelType"], json!("text"));
    assert_eq!(find_node("D")["label"], json!("bare `tick` text"));
    assert_eq!(find_node("D")["labelType"], json!("text"));

    let edges = res.model["edges"].as_array().unwrap();
    assert_eq!(edges[0]["label"], json!("'Known issue'"));
    assert_eq!(edges[0]["labelType"], json!("text"));
}

#[test]
fn parse_diagram_flowchart_markdown_strings_in_subgraphs() {
    let engine = Engine::new();
    let text = r#"flowchart LR
subgraph "One"
  a("`The **cat**
  in the hat`") -- "1o" --> b{{"`The **dog** in the hog`"}}
end
subgraph "`**Two**`"
  c("`The **cat**
  in the hat`") -- "`1o **ipa**`" --> d("The dog in the hog")
end"#;
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();

    let subgraphs = res.model["subgraphs"].as_array().unwrap();
    assert_eq!(subgraphs.len(), 2);
    assert_eq!(subgraphs[0]["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(subgraphs[0]["title"], json!("One"));
    assert_eq!(subgraphs[0]["labelType"], json!("text"));
    assert_eq!(subgraphs[1]["nodes"].as_array().unwrap().len(), 2);
    assert_eq!(subgraphs[1]["title"], json!("**Two**"));
    assert_eq!(subgraphs[1]["labelType"], json!("markdown"));
}

#[test]
fn parse_diagram_flowchart_header_direction_shorthand() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram("graph >;A-->B;", ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["direction"], json!("LR"));

    let res = block_on(engine.parse_diagram("graph <;A-->B;", ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["direction"], json!("RL"));

    let res = block_on(engine.parse_diagram("graph ^;A-->B;", ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["direction"], json!("BT"));

    let res = block_on(engine.parse_diagram("graph v;A-->B;", ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["direction"], json!("TB"));
}

#[test]
fn parse_diagram_flowchart_v_is_node_id_not_direction() {
    let engine = Engine::new();
    let res = block_on(engine.parse_diagram("graph TD;A--xv(my text);", ParseOptions::default()))
        .unwrap()
        .unwrap();

    let v = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("v"))
        .unwrap();
    assert_eq!(v["label"], json!("my text"));
    assert_eq!(v["shape"], json!("round"));
    assert_eq!(res.model["edges"][0]["type"], json!("arrow_cross"));
}

#[test]
fn parse_diagram_flowchart_v_in_node_ids_variants_from_flow_text_spec() {
    let engine = Engine::new();
    let text = "graph TD;A--xv(my text);A--xcsv(my text);A--xava(my text);A--xva(my text);";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();

    assert_eq!(res.model["edges"].as_array().unwrap().len(), 4);
    for edge in res.model["edges"].as_array().unwrap() {
        assert_eq!(edge["type"], json!("arrow_cross"));
    }

    let nodes = res.model["nodes"].as_array().unwrap();
    let find = |id: &str| nodes.iter().find(|n| n["id"] == json!(id)).unwrap();

    assert_eq!(find("v")["label"], json!("my text"));
    assert_eq!(find("csv")["label"], json!("my text"));
    assert_eq!(find("ava")["label"], json!("my text"));
    assert_eq!(find("va")["label"], json!("my text"));
}

#[test]
fn parse_diagram_flowchart_edge_label_supports_quoted_strings() {
    let engine = Engine::new();
    let res = block_on(engine.parse_diagram(
        "graph TD;V-- \"test string()\" -->a[v]",
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    assert_eq!(res.model["edges"][0]["label"], json!("test string()"));
    assert_eq!(res.model["edges"][0]["labelType"], json!("string"));
}

#[test]
fn parse_diagram_flowchart_edge_label_old_notation_without_spaces() {
    let engine = Engine::new();
    let res = block_on(engine.parse_diagram(
        "graph TD;A--text including URL space and send-->B;",
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    assert_eq!(
        res.model["edges"][0]["label"],
        json!("text including URL space and send")
    );
    assert_eq!(res.model["edges"][0]["type"], json!("arrow_point"));
}

#[test]
fn parse_diagram_flowchart_edge_labels_can_span_multiple_lines() {
    let engine = Engine::new();
    let text = "graph TD;A--o|text space|B;\n B-->|more text with space|C;";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edges"].as_array().unwrap().len(), 2);
    assert_eq!(res.model["edges"][0]["type"], json!("arrow_circle"));
    assert_eq!(res.model["edges"][1]["type"], json!("arrow_point"));
    assert_eq!(
        res.model["edges"][1]["label"],
        json!("more text with space")
    );
}

#[test]
fn parse_diagram_flowchart_vertex_shapes_from_flow_text_spec() {
    let engine = Engine::new();
    let text = r#"graph TD;
A_node-->B[This is square];
A_node-->C(Chimpansen hoppar);
A_node-->D{Diamond};
A_node-->E((Circle));
A_node-->F(((Double circle)));
A_node-->G{{Hex}};
A_node-->H[[Subroutine]];
A_node-->I(-Ellipse-);
A_node-->J([Stadium]);
A_node-->K[(Cylinder)];
A_node-->L>Odd];
A_node-->M[/Lean right/];
A_node-->N[\Lean left\];
A_node-->O[/Trapezoid\];
A_node-->P[\Inv trapezoid/];
"#;
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();

    let nodes = res.model["nodes"].as_array().unwrap();
    let find = |id: &str| nodes.iter().find(|n| n["id"] == json!(id)).unwrap();

    assert_eq!(find("B")["shape"], json!("square"));
    assert_eq!(find("C")["shape"], json!("round"));
    assert_eq!(find("D")["shape"], json!("diamond"));
    assert_eq!(find("E")["shape"], json!("circle"));
    assert_eq!(find("F")["shape"], json!("doublecircle"));
    assert_eq!(find("G")["shape"], json!("hexagon"));
    assert_eq!(find("H")["shape"], json!("subroutine"));
    assert_eq!(find("I")["shape"], json!("ellipse"));
    assert_eq!(find("J")["shape"], json!("stadium"));
    assert_eq!(find("K")["shape"], json!("cylinder"));
    assert_eq!(find("L")["shape"], json!("odd"));
    assert_eq!(find("M")["shape"], json!("lean_right"));
    assert_eq!(find("N")["shape"], json!("lean_left"));
    assert_eq!(find("O")["shape"], json!("trapezoid"));
    assert_eq!(find("P")["shape"], json!("inv_trapezoid"));
}

#[test]
fn parse_diagram_flowchart_rect_border_syntax_sets_rect_shape() {
    let engine = Engine::new();
    let text = "graph TD;A_node-->B[|borders:lt|This node has a graph as text];";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let b = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("B"))
        .unwrap();
    assert_eq!(b["shape"], json!("rect"));
    assert_eq!(b["label"], json!("This node has a graph as text"));
}

#[test]
fn parse_diagram_flowchart_odd_vertex_allows_id_ending_with_minus() {
    let engine = Engine::new();
    let text = "graph TD;A_node-->odd->Vertex Text];";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();

    let odd = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("odd-"))
        .unwrap();
    assert_eq!(odd["shape"], json!("odd"));
    assert_eq!(odd["label"], json!("Vertex Text"));
}

#[test]
fn parse_diagram_flowchart_allows_brackets_inside_quoted_square_labels() {
    let engine = Engine::new();
    let text = "graph TD;A[\"chimpansen hoppar ()[]\"] --> C;";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let a = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("A"))
        .unwrap();
    assert_eq!(a["shape"], json!("square"));
    assert_eq!(a["label"], json!("chimpansen hoppar ()[]"));
    assert_eq!(a["labelType"], json!("string"));
}

#[test]
fn parse_diagram_flowchart_classifies_labels_before_flowdb_trim() {
    let engine = Engine::new();
    let nel = '\u{0085}';
    let source = format!(
        "flowchart TD\nA[\"{nel}A{nel}\"]\nB[|borders:t|{nel}B{nel}]\nC[\"&nbsp;C&nbsp;\"]\nD[\"#nbsp;D#nbsp;\"]\n"
    );
    let parsed = block_on(engine.parse_diagram(&source, ParseOptions::strict()))
        .expect("valid labels must parse")
        .expect("diagram detected");
    let nodes = parsed.model["nodes"].as_array().expect("nodes");
    let label = |id: &str| {
        nodes
            .iter()
            .find(|node| node["id"] == json!(id))
            .unwrap_or_else(|| panic!("missing node {id}"))["label"]
            .as_str()
            .expect("label")
    };

    assert_eq!(label("A"), format!("{nel}A{nel}"));
    assert_eq!(label("B"), format!("{nel}B{nel}"));
    assert_eq!(label("C"), "\u{00A0}C\u{00A0}");
    assert_eq!(label("D"), "\u{00A0}D\u{00A0}");

    let edge_source = "flowchart TD\nE --\u{00A0}\"Edge\"\u{FEFF}--> F\nM --\u{FEFF}\"`Markdown`\"\u{00A0}--> N\n";
    let parsed = block_on(engine.parse_diagram(edge_source, ParseOptions::strict()))
        .expect("ECMAScript whitespace around edge text belongs to the link tokens")
        .expect("diagram detected");
    let edges = parsed.model["edges"].as_array().expect("edges");
    assert_eq!(edges[0]["label"], json!("Edge"));
    assert_eq!(edges[0]["labelType"], json!("string"));
    assert_eq!(edges[1]["label"], json!("Markdown"));
    assert_eq!(edges[1]["labelType"], json!("markdown"));

    for invalid in [
        "flowchart TD\nA[\u{FEFF}\"foo\"\u{FEFF}]\n",
        "flowchart TD\nA[\u{00A0}\"foo\"\u{00A0}]\n",
        "flowchart TD\nA -->|\u{FEFF}\"foo\"\u{FEFF}| B\n",
        "flowchart TD\nA --\u{0085}\"foo\"\u{0085}--> B\n",
        "flowchart TD\nA[\u{FEFF}|borders:t|Label]\n",
        "flowchart TD\nsubgraph SG[\u{FEFF}\"Title\"\u{FEFF}]\nA\nend\n",
    ] {
        assert!(
            block_on(engine.parse_diagram(invalid, ParseOptions::strict())).is_err(),
            "Mermaid rejects mixed TEXT/STR label tokens: {invalid:?}",
        );
    }
}

#[test]
fn parse_diagram_flowchart_subgraph_quotes_do_not_support_backslash_escapes() {
    let engine = Engine::new();
    let source = "flowchart TD\nsubgraph SG[\"A\\\"]B\"]\nA\nend\n";

    assert!(
        block_on(engine.parse_diagram(source, ParseOptions::strict())).is_err(),
        "Mermaid's Jison string state closes on every double quote",
    );
}

#[test]
fn parse_diagram_flowchart_flow_text_error_cases_from_upstream_spec() {
    let engine = Engine::new();

    let err = block_on(engine.parse_diagram(
        "graph TD; A[This is a () in text];",
        ParseOptions::default(),
    ))
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("Invalid text label: contains structural characters; quote it to use them")
    );

    let err = block_on(engine.parse_diagram(
        "graph TD;A(this node has \"string\" and text)-->|this link has \"string\" and text|C;",
        ParseOptions::default(),
    ))
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("Invalid text label: contains structural characters; quote it to use them")
    );

    let err = block_on(engine.parse_diagram(
        "graph TD; A[This is a \\\"()\\\" in text];",
        ParseOptions::default(),
    ))
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("Unterminated node label (missing `]`)")
    );

    let err = block_on(engine.parse_diagram(
        "graph TD; A[\"This is a \"()\" in text\"];",
        ParseOptions::default(),
    ))
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("Invalid string label: contains nested quotes")
    );

    let err = block_on(engine.parse_diagram(
        "graph TD; node[hello ) world] --> works",
        ParseOptions::default(),
    ))
    .unwrap_err();
    assert!(
        err.to_string()
            .contains("Invalid text label: contains structural characters; quote it to use them")
    );

    let err = block_on(engine.parse_diagram("graph\nX(- My Text (", ParseOptions::default()))
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("Unterminated node label (missing `-)`)")
    );
}

#[test]
fn parse_diagram_flowchart_keywords_in_vertex_text_across_shapes() {
    let engine = Engine::new();

    let keywords = [
        "graph",
        "flowchart",
        "flowchart-elk",
        "style",
        "default",
        "linkStyle",
        "interpolate",
        "classDef",
        "class",
        "href",
        "call",
        "click",
        "_self",
        "_blank",
        "_parent",
        "_top",
        "end",
        "subgraph",
        "kitty",
    ];

    let shapes: [(&str, &str, &str); 14] = [
        ("[", "]", "square"),
        ("(", ")", "round"),
        ("{", "}", "diamond"),
        ("(-", "-)", "ellipse"),
        ("([", "])", "stadium"),
        (">", "]", "odd"),
        ("[(", ")]", "cylinder"),
        ("(((", ")))", "doublecircle"),
        ("[/", "\\]", "trapezoid"),
        ("[\\", "/]", "inv_trapezoid"),
        ("[/", "/]", "lean_right"),
        ("[\\", "\\]", "lean_left"),
        ("[[", "]]", "subroutine"),
        ("{{", "}}", "hexagon"),
    ];

    for keyword in keywords {
        for (open, close, shape) in shapes {
            let text = format!(
                "graph TD;A_{keyword}_node-->B{open}This node has a {keyword} as text{close};"
            );
            let res = block_on(engine.parse_diagram(&text, ParseOptions::default()))
                .unwrap()
                .unwrap();
            let b = res
                .model
                .get("nodes")
                .and_then(|v| v.as_array())
                .unwrap()
                .iter()
                .find(|n| n["id"] == json!("B"))
                .unwrap();
            assert_eq!(b["shape"], json!(shape));
            assert_eq!(
                b["label"],
                json!(format!("This node has a {keyword} as text"))
            );
        }

        let rect_text = format!(
            "graph TD;A_{keyword}_node-->B[|borders:lt|This node has a {keyword} as text];"
        );
        let res = block_on(engine.parse_diagram(&rect_text, ParseOptions::default()))
            .unwrap()
            .unwrap();
        let b = res
            .model
            .get("nodes")
            .and_then(|v| v.as_array())
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!("B"))
            .unwrap();
        assert_eq!(b["shape"], json!("rect"));
        assert_eq!(
            b["label"],
            json!(format!("This node has a {keyword} as text"))
        );
    }
}

#[test]
fn parse_diagram_flowchart_allows_slashes_in_lean_vertices() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram(
        "graph TD;A_node-->B[/This node has a / as text/];",
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    let b = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("B"))
        .unwrap();
    assert_eq!(b["shape"], json!("lean_right"));
    assert_eq!(b["label"], json!("This node has a / as text"));

    let res = block_on(engine.parse_diagram(
        r#"graph TD;A_node-->B[\This node has a \ as text\];"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    let b = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("B"))
        .unwrap();
    assert_eq!(b["shape"], json!("lean_left"));
    assert_eq!(b["label"], json!(r#"This node has a \ as text"#));
}

#[test]
fn parse_diagram_flowchart_misc_vertex_text_cases_from_flow_text_spec() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram(
        "graph TD;A-->C{Chimpansen hoppar ???-???};",
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    let c = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("C"))
        .unwrap();
    assert_eq!(c["shape"], json!("diamond"));
    assert_eq!(c["label"], json!("Chimpansen hoppar ???-???"));

    let res = block_on(engine.parse_diagram(
        "graph TD;A-->C(Chimpansen hoppar ???  <br> -  ???);",
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    let c = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("C"))
        .unwrap();
    assert_eq!(c["shape"], json!("round"));
    assert_eq!(c["label"], json!("Chimpansen hoppar ???  <br> -  ???"));

    let res =
        block_on(engine.parse_diagram("graph TD;A-->C(妖忘折忘抖抉);", ParseOptions::default()))
            .unwrap()
            .unwrap();
    let c = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("C"))
        .unwrap();
    assert_eq!(c["label"], json!("妖忘折忘抖抉"));

    let res =
        block_on(engine.parse_diagram(r#"graph TD;A-->C(c:\windows);"#, ParseOptions::default()))
            .unwrap()
            .unwrap();
    let c = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("C"))
        .unwrap();
    assert_eq!(c["label"], json!(r#"c:\windows"#));
}

#[test]
fn parse_diagram_flowchart_ellipse_vertex_text_and_unterminated_ellipse_errors() {
    let engine = Engine::new();

    let ok = block_on(engine.parse_diagram(
        "graph TD\nA(-this is an ellipse-)-->B",
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    let a = ok.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("A"))
        .unwrap();
    assert_eq!(a["shape"], json!("ellipse"));
    assert_eq!(a["label"], json!("this is an ellipse"));

    let bad = block_on(engine.parse_diagram("graph\nX(- My Text (", ParseOptions::default()));
    assert!(bad.is_err());
}

#[test]
fn parse_diagram_flowchart_question_and_unicode_in_node_and_edge_text() {
    let engine = Engine::new();

    let res = block_on(engine.parse_diagram("graph TD;A(?)-->|?|C;", ParseOptions::default()))
        .unwrap()
        .unwrap();
    let a = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("A"))
        .unwrap();
    assert_eq!(a["label"], json!("?"));
    assert_eq!(res.model["edges"][0]["label"], json!("?"));

    let res = block_on(engine.parse_diagram(
        "graph TD;A(谷豕那角??)-->|谷豕那角??|C;",
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();
    let a = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("A"))
        .unwrap();
    assert_eq!(a["label"], json!("谷豕那角??"));
    assert_eq!(res.model["edges"][0]["label"], json!("谷豕那角??"));

    let res = block_on(
        engine.parse_diagram("graph TD;A(,.?!+-*)-->|,.?!+-*|C;", ParseOptions::default()),
    )
    .unwrap()
    .unwrap();
    let a = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("A"))
        .unwrap();
    assert_eq!(a["label"], json!(",.?!+-*"));
    assert_eq!(res.model["edges"][0]["label"], json!(",.?!+-*"));
}

#[test]
fn parse_diagram_flowchart_node_label_invalid_mixed_text_and_quotes_errors() {
    let engine = Engine::new();

    let bad = block_on(engine.parse_diagram(
        "graph TD; A[This is a () in text];",
        ParseOptions::default(),
    ));
    assert!(bad.is_err());

    let bad = block_on(engine.parse_diagram(
        "graph TD;A(this node has \"string\" and text)-->|this link has \"string\" and text|C;",
        ParseOptions::default(),
    ));
    assert!(bad.is_err());

    let bad = block_on(engine.parse_diagram(
        "graph TD; A[This is a \\\"()\\\" in text];",
        ParseOptions::default(),
    ));
    assert!(bad.is_err());

    let bad = block_on(engine.parse_diagram(
        "graph TD; A[\"This is a \"()\" in text\"];",
        ParseOptions::default(),
    ));
    assert!(bad.is_err());

    let bad = block_on(engine.parse_diagram(
        "graph TD; node[hello ) world] --> works",
        ParseOptions::default(),
    ));
    assert!(bad.is_err());
}

#[test]
fn parse_diagram_flowchart_supports_subgraph_block() {
    let engine = Engine::new();
    let text = "graph TD;subgraph S;A-->B;end;";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.meta.diagram_type, "flowchart-v2");
    assert_eq!(
        res.model["subgraphs"],
        json!([{
            "id": "S",
            "nodes": ["B", "A"],
            "title": "S",
            "classes": [],
            "styles": [],
            "dir": null,
            "hasExplicitDir": false,
            "labelType": "text"
        }])
    );
}

#[test]
fn parse_diagram_flowchart_supports_nested_subgraphs() {
    let engine = Engine::new();
    let text = "graph TD;subgraph Outer;subgraph Inner;A-->B;end;end;";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(
        res.model["subgraphs"],
        json!([{
            "id": "Inner",
            "nodes": ["B", "A"],
            "title": "Inner",
            "classes": [],
            "styles": [],
            "dir": null,
            "hasExplicitDir": false,
            "labelType": "text"
        }, {
            "id": "Outer",
            "nodes": ["Inner"],
            "title": "Outer",
            "classes": [],
            "styles": [],
            "dir": null,
            "hasExplicitDir": false,
            "labelType": "text"
        }])
    );
}

#[test]
fn parse_diagram_flowchart_subgraph_supports_explicit_id_and_title() {
    let engine = Engine::new();
    let text = "graph TD;subgraph ide1[one];A-->B;end;";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(
        res.model["subgraphs"],
        json!([{
            "id": "ide1",
            "nodes": ["B", "A"],
            "title": "one",
            "classes": [],
            "styles": [],
            "dir": null,
            "hasExplicitDir": false,
            "labelType": "text"
        }])
    );
}

#[test]
fn parse_diagram_flowchart_subgraph_title_with_spaces_uses_auto_id() {
    let engine = Engine::new();
    let text = "graph TD;subgraph number as labels;A-->B;end;";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(
        res.model["subgraphs"],
        json!([{
            "id": "subGraph0",
            "nodes": ["B", "A"],
            "title": "number as labels",
            "classes": [],
            "styles": [],
            "dir": null,
            "hasExplicitDir": false,
            "labelType": "text"
        }])
    );
}

#[test]
fn parse_diagram_flowchart_subgraph_direction_statement_sets_dir() {
    let engine = Engine::new();
    let text = "graph LR;subgraph TOP;direction TD;A-->B;end;";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(
        res.model["subgraphs"],
        json!([{
            "id": "TOP",
            "nodes": ["B", "A"],
            "title": "TOP",
            "classes": [],
            "styles": [],
            "dir": "TD",
            "hasExplicitDir": true,
            "labelType": "text"
        }])
    );
}

#[test]
fn parse_diagram_flowchart_subgraph_inherits_global_direction_when_enabled() {
    let mut site = MermaidConfig::empty_object();
    site.set_value("flowchart.inheritDir", json!(true));
    let engine = Engine::new().with_site_config(site);
    let text = "graph LR;subgraph TOP;A-->B;end;";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["subgraphs"][0]["dir"], json!("LR"));
    assert_eq!(res.model["subgraphs"][0]["hasExplicitDir"], json!(false));
}

#[test]
fn parse_diagram_flowchart_subgraph_tab_indentation_matches_mermaid_membership_order() {
    let engine = Engine::new();
    let text = "graph TB\nsubgraph One\n\ta1-->a2\nend";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(
        res.model["subgraphs"],
        json!([{
            "id": "One",
            "nodes": ["a2", "a1"],
            "title": "One",
            "classes": [],
            "styles": [],
            "dir": null,
            "hasExplicitDir": false,
            "labelType": "text"
        }])
    );
}

#[test]
fn parse_diagram_flowchart_subgraph_chain_membership_order_matches_mermaid() {
    let engine = Engine::new();
    let text = "graph TB\nsubgraph One\n\ta1-->a2-->a3\nend";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(
        res.model["subgraphs"][0]["nodes"],
        json!(["a3", "a2", "a1"])
    );
}

#[test]
fn parse_diagram_flowchart_subgraph_title_with_spaces_in_quotes_uses_auto_id() {
    let engine = Engine::new();
    let text = "graph TB\nsubgraph \"Some Title\"\n\ta1-->a2\nend";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["subgraphs"][0]["title"], json!("Some Title"));
    assert_eq!(res.model["subgraphs"][0]["id"], json!("subGraph0"));
}

#[test]
fn parse_diagram_flowchart_subgraph_id_and_title_notation() {
    let engine = Engine::new();
    let text = "graph TB\nsubgraph some-id[Some Title]\n\ta1-->a2\nend";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["subgraphs"][0]["id"], json!("some-id"));
    assert_eq!(res.model["subgraphs"][0]["title"], json!("Some Title"));
    assert_eq!(res.model["subgraphs"][0]["labelType"], json!("text"));
}

#[test]
fn parse_diagram_flowchart_subgraph_bracket_quoted_title_sets_label_type_string() {
    let engine = Engine::new();
    let text = "graph TD;subgraph uid2[\"text of doom\"];c-->d;end;";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["subgraphs"][0]["id"], json!("uid2"));
    assert_eq!(res.model["subgraphs"][0]["title"], json!("text of doom"));
    assert_eq!(res.model["subgraphs"][0]["labelType"], json!("string"));
}

#[test]
fn parse_diagram_flowchart_subgraph_space_and_multiline_quotes_match_pinned_jison() {
    let engine = Engine::new();
    for separator in ['\u{00a0}', '\u{feff}', '\u{202f}'] {
        let source = format!("flowchart TD\nsubgraph{separator}Name\nA\nend\n");
        let parsed = block_on(engine.parse_diagram(&source, ParseOptions::strict()))
            .expect("ECMAScript SPACE token must parse")
            .expect("diagram detected");
        assert_eq!(parsed.model["subgraphs"][0]["id"], json!("Name"));
        assert_eq!(parsed.model["subgraphs"][0]["title"], json!("Name"));
    }

    for source in [
        "flowchart TD\nsubgraph SG[\"Line one\nLine two\"]\nA\nend\n",
        "flowchart TD\nsubgraph \"Line one\nLine two\"\nA\nend\n",
    ] {
        let parsed = block_on(engine.parse_diagram(source, ParseOptions::strict()))
            .expect("quoted subgraph line breaks must parse")
            .expect("diagram detected");
        assert_eq!(
            parsed.model["subgraphs"][0]["title"],
            json!("Line one\nLine two")
        );
    }
}

#[test]
fn parse_diagram_flowchart_subgraph_markdown_title_sets_label_type_markdown() {
    let engine = Engine::new();
    let text = "graph TD\nsubgraph \"`**Two**`\"\nA-->B\nend";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["subgraphs"][0]["title"], json!("**Two**"));
    assert_eq!(res.model["subgraphs"][0]["labelType"], json!("markdown"));
}

#[test]
fn parse_diagram_flowchart_subgraph_single_quotes_and_bare_backticks_stay_text() {
    let engine = Engine::new();
    let text = "graph TD\nsubgraph quoted['Literal quotes']\nA\nend\nsubgraph ticks[bare `tick` title]\nB\nend";
    let res = block_on(engine.parse_diagram(text, ParseOptions::strict()))
        .expect("single quotes and bare backticks are ordinary text")
        .expect("diagram detected");

    let subgraphs = res.model["subgraphs"].as_array().unwrap();
    let find = |id: &str| {
        subgraphs
            .iter()
            .find(|subgraph| subgraph["id"] == json!(id))
            .unwrap()
    };
    assert_eq!(find("quoted")["title"], json!("'Literal quotes'"));
    assert_eq!(find("quoted")["labelType"], json!("text"));
    assert_eq!(find("ticks")["title"], json!("bare `tick` title"));
    assert_eq!(find("ticks")["labelType"], json!("text"));
}

#[test]
fn parse_diagram_flowchart_duplicate_subgraph_membership_matches_mermaid_makeuniq() {
    let engine = Engine::new();
    let text = "graph TD\nsubgraph A\nB\nend\nsubgraph X\nB\nend\nB-->C\n";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();

    assert_eq!(
        res.model["subgraphs"],
        json!([{
            "id": "A",
            "nodes": ["B"],
            "title": "A",
            "classes": [],
            "styles": [],
            "dir": null,
            "hasExplicitDir": false,
            "labelType": "text"
        }, {
            "id": "X",
            "nodes": [],
            "title": "X",
            "classes": [],
            "styles": [],
            "dir": null,
            "hasExplicitDir": false,
            "labelType": "text"
        }])
    );
}

#[test]
fn parse_diagram_flowchart_subgraph_supports_amp_group_syntax_minimally() {
    let engine = Engine::new();
    let text = "graph TD\nsubgraph myTitle\na & b --> c & e\nend";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let nodes = res.model["subgraphs"][0]["nodes"].as_array().unwrap();
    let as_set: std::collections::HashSet<String> = nodes
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(as_set.contains("a"));
    assert!(as_set.contains("b"));
    assert!(as_set.contains("c"));
    assert!(as_set.contains("e"));
}

#[test]
fn parse_diagram_flowchart_style_statement_applies_vertex_styles() {
    let engine = Engine::new();
    let text = "graph TD;style Q background:#fff;";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let q = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("Q"))
        .unwrap();
    assert_eq!(q["styles"], json!(["background:#fff"]));
    assert_eq!(
        res.model["warningFacts"],
        json!([
            {
                "ruleId": FLOWCHART_UNKNOWN_STYLE_TARGET_WARNING_RULE_ID,
                "message": "Style applied to unknown node \"Q\". This may indicate a typo. The node will be created automatically.",
                "span": { "start": 15, "end": 16 }
            }
        ])
    );
    assert_eq!(
        res.model["warnings"],
        json!([
            "Style applied to unknown node \"Q\". This may indicate a typo. The node will be created automatically."
        ])
    );
}

#[test]
fn parse_diagram_flowchart_classdef_and_class_assign_work() {
    let engine = Engine::new();
    let text =
        "graph TD;classDef exClass background:#bbb,border:1px solid red;a-->b;class a,b exClass;";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(
        res.model["classDefs"]["exClass"],
        json!(["background:#bbb", "border:1px solid red"])
    );
    let a = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("a"))
        .unwrap();
    let b = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("b"))
        .unwrap();
    assert_eq!(a["classes"][0], json!("exClass"));
    assert_eq!(b["classes"][0], json!("exClass"));
}

#[test]
fn parse_diagram_flowchart_inline_vertex_class_via_style_separator() {
    let engine = Engine::new();
    // Mermaid `encodeEntities(...)` treats `#bbb;` as an entity placeholder when semicolons
    // are used as statement separators. Use newlines to match upstream parsing behavior.
    let text = "graph TD\nclassDef exClass background:#bbb\nA-->B[test]:::exClass\n";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let b = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("B"))
        .unwrap();
    assert_eq!(b["classes"][0], json!("exClass"));
}

#[test]
fn parse_diagram_flowchart_linkstyle_applies_edge_style_and_validates_bounds() {
    let engine = Engine::new();
    let ok = "graph TD\nA-->B\nlinkStyle 0 stroke-width:1px;";
    let res = block_on(engine.parse_diagram(ok, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edges"][0]["style"][0], json!("stroke-width:1px"));

    let bad = "graph TD\nA-->B\nlinkStyle 1 stroke-width:1px;";
    let err = block_on(engine.parse_diagram(bad, ParseOptions::default())).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Diagram parse error (flowchart-v2): The index 1 for linkStyle is out of bounds. Valid indices for linkStyle are between 0 and 0. (Help: Ensure that the index is within the range of existing edges.)"
    );
}

#[test]
fn parse_diagram_flowchart_linkstyle_default_interpolate_sets_edge_defaults() {
    let engine = Engine::new();
    let text = "graph TD\nA-->B\nlinkStyle default interpolate basis";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edgeDefaults"]["interpolate"], json!("basis"));
}

#[test]
fn parse_diagram_flowchart_linkstyle_numbered_interpolate_sets_edges() {
    let engine = Engine::new();
    let text =
        "graph TD\nA-->B\nA-->C\nlinkStyle 0 interpolate basis\nlinkStyle 1 interpolate cardinal";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edges"][0]["interpolate"], json!("basis"));
    assert_eq!(res.model["edges"][1]["interpolate"], json!("cardinal"));
}

#[test]
fn parse_diagram_flowchart_linkstyle_multi_numbered_interpolate_sets_edges() {
    let engine = Engine::new();
    let text = "graph TD\nA-->B\nA-->C\nlinkStyle 0,1 interpolate basis";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edges"][0]["interpolate"], json!("basis"));
    assert_eq!(res.model["edges"][1]["interpolate"], json!("basis"));
}

#[test]
fn parse_diagram_flowchart_edge_curve_properties_using_edge_id() {
    let engine = Engine::new();
    let text =
        "graph TD\nA e1@-->B\nA uniqueName@-->C\ne1@{curve: basis}\nuniqueName@{curve: cardinal}";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edges"][0]["id"], json!("e1"));
    assert_eq!(res.model["edges"][1]["id"], json!("uniqueName"));
    assert_eq!(res.model["edges"][0]["interpolate"], json!("basis"));
    assert_eq!(res.model["edges"][1]["interpolate"], json!("cardinal"));
}

#[test]
fn parse_diagram_flowchart_edge_curve_properties_does_not_override_default() {
    let engine = Engine::new();
    let text =
        "graph TD\nA e1@-->B\nA-->C\nlinkStyle default interpolate linear\ne1@{curve: stepAfter}";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edgeDefaults"]["interpolate"], json!("linear"));
    assert_eq!(res.model["edges"][0]["interpolate"], json!("stepAfter"));
}

#[test]
fn parse_diagram_flowchart_edge_curve_properties_mixed_with_line_interpolation() {
    let engine = Engine::new();
    let text = "graph TD\nA e1@-->B-->D\nA-->C e4@-->D-->E\nlinkStyle default interpolate linear\nlinkStyle 1 interpolate basis\ne1@{curve: monotoneX}\ne4@{curve: stepBefore}";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["edgeDefaults"]["interpolate"], json!("linear"));
    assert_eq!(res.model["edges"][0]["interpolate"], json!("monotoneX"));
    assert_eq!(res.model["edges"][1]["interpolate"], json!("basis"));
    assert_eq!(res.model["edges"][3]["interpolate"], json!("stepBefore"));
}

#[test]
fn parse_diagram_flowchart_click_link_sets_link_and_tooltip_and_clickable_class() {
    let engine = Engine::new();
    let text = "graph TD\nA-->B\nclick A href \"click.html\" \"tooltip\" _blank";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let a = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("A"))
        .unwrap();
    assert_eq!(a["link"], json!("click.html"));
    assert_eq!(a["linkTarget"], json!("_blank"));
    assert_eq!(res.model["tooltips"]["A"], json!("tooltip"));
    assert_eq!(a["classes"][0], json!("clickable"));
}

#[test]
fn parse_diagram_flowchart_click_link_sanitizes_javascript_urls_when_not_loose() {
    let engine = Engine::new();
    let text = "graph TD\nA-->B\nclick A href \"javascript:alert(1)\" \"tooltip\" _blank";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let a = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("A"))
        .unwrap();
    assert_eq!(a["link"], json!("about:blank"));
    assert_eq!(a["linkTarget"], json!("_blank"));
}

#[test]
fn parse_diagram_flowchart_style_statement_supports_multiple_styles() {
    let engine = Engine::new();
    let text = "graph TD;style R background:#fff,border:1px solid red;";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let r = res.model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == json!("R"))
        .unwrap();
    assert_eq!(
        r["styles"],
        json!(["background:#fff", "border:1px solid red"])
    );
}

#[test]
fn parse_diagram_flowchart_classdef_supports_multiple_classes() {
    let engine = Engine::new();
    let text = "graph TD;classDef firstClass,secondClass background:#bbb,border:1px solid red;";
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(
        res.model["classDefs"]["firstClass"],
        json!(["background:#bbb", "border:1px solid red"])
    );
    assert_eq!(
        res.model["classDefs"]["secondClass"],
        json!(["background:#bbb", "border:1px solid red"])
    );
}

#[test]
fn parse_diagram_flowchart_inline_vertex_class_in_groups_matches_mermaid_style_spec() {
    let engine = Engine::new();
    let text = r#"
graph TD
  classDef C1 stroke-dasharray:4
  classDef C2 stroke-dasharray:6
  A & B:::C1 & D:::C1 --> E:::C2
"#;
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let find = |id: &str| {
        res.model["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["id"] == json!(id))
            .unwrap()
            .clone()
    };
    assert!(find("A")["classes"].as_array().unwrap().is_empty());
    assert_eq!(find("B")["classes"][0], json!("C1"));
    assert_eq!(find("D")["classes"][0], json!("C1"));
    assert_eq!(find("E")["classes"][0], json!("C2"));
}

#[test]
fn parse_diagram_flowchart_keyword_flowchart() {
    let engine = Engine::new();
    let res = block_on(engine.parse_diagram("flowchart TD\nA-->B", ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.meta.diagram_type, "flowchart-v2");
    assert_eq!(res.model["keyword"], json!("flowchart"));
    assert_eq!(res.model["direction"], json!("TB"));
    assert_eq!(res.model["subgraphs"], json!([]));
    assert!(res.model.get("warningFacts").is_none());
    assert!(res.model.get("warnings").is_none());
}

#[test]
fn parse_diagram_flowchart_without_direction_preserves_source_and_warns() {
    let engine = Engine::new();
    let res = block_on(engine.parse_diagram("flowchart\nA-->B", ParseOptions::default()))
        .unwrap()
        .unwrap();

    assert_eq!(res.meta.diagram_type, "flowchart-v2");
    assert_eq!(res.model["direction"], json!(null));
    assert_eq!(
        res.model["warningFacts"],
        json!([
            {
                "ruleId": FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID,
                "message": "flowchart headers should declare an explicit direction such as `TB`, `TD`, `BT`, `LR`, or `RL`",
                "span": { "start": 0, "end": 9 },
                "fixSpan": { "start": 9, "end": 9 }
            }
        ])
    );
    assert_eq!(
        res.model["warnings"],
        json!([
            "flowchart headers should declare an explicit direction such as `TB`, `TD`, `BT`, `LR`, or `RL`"
        ])
    );
}

#[test]
fn parse_flowchart_render_model_carries_missing_direction_warning_fact() {
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync("flowchart\nA-->B", ParseOptions::strict())
        .unwrap()
        .unwrap();

    match parsed.model() {
        RenderSemanticModel::Flowchart(model) => {
            assert_eq!(model.direction.as_deref(), Some("TB"));
            assert_eq!(model.warning_facts.len(), 1);
            assert_eq!(
                model.warning_facts[0].rule_id,
                FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID
            );
            assert_eq!(model.warning_facts[0].span, Some(SourceSpan::new(0, 9)));
            assert_eq!(model.warning_facts[0].fix_span, Some(SourceSpan::new(9, 9)));
        }
        other => panic!("flowchart render parse should return typed model, got {other:?}"),
    }
}

#[test]
fn parse_flowchart_render_model_remaps_missing_direction_warning_fact_after_frontmatter() {
    let engine = Engine::new();
    let text = "---\ntitle: Demo\n---\nflowchart\nA-->B";
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let flowchart_start = text.find("flowchart").expect("flowchart header");

    match parsed.model() {
        RenderSemanticModel::Flowchart(model) => {
            assert_eq!(
                model.warning_facts[0].span,
                Some(SourceSpan::new(
                    flowchart_start,
                    flowchart_start + "flowchart".len()
                ))
            );
            assert_eq!(
                model.warning_facts[0].fix_span,
                Some(SourceSpan::new(
                    flowchart_start + "flowchart".len(),
                    flowchart_start + "flowchart".len()
                ))
            );
        }
        other => panic!("flowchart render parse should return typed model, got {other:?}"),
    }
}

#[test]
fn parse_flowchart_warning_fact_span_survives_entity_preprocess() {
    let engine = Engine::new();
    let text = "flowchart\n  classDef cat fill:#f9d5e5\n  A:::cat\n";
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::strict()))
        .unwrap()
        .unwrap();

    assert_eq!(
        parsed.model["warningFacts"][0]["span"],
        json!({ "start": 0, "end": 9 })
    );
    assert_eq!(
        parsed.model["warningFacts"][0]["fixSpan"],
        json!({ "start": 9, "end": 9 })
    );
}

#[test]
fn parse_flowchart_warning_fact_span_uses_context_after_frontmatter_and_entity_preprocess() {
    let engine = Engine::new();
    let text = "---\nconfig:\n  flowchart:\n    htmlLabels: true\n---\n\nflowchart\n  classDef cat fill:#f9d5e5\n  A:::cat\n";
    let parsed = block_on(engine.parse_diagram(text, ParseOptions::strict()))
        .unwrap()
        .unwrap();
    let flowchart_start = text
        .rfind("\nflowchart\n")
        .map(|offset| offset + 1)
        .expect("body flowchart header");

    assert_eq!(
        parsed.model["warningFacts"][0]["span"],
        json!({ "start": flowchart_start, "end": flowchart_start + "flowchart".len() })
    );
    assert_eq!(
        parsed.model["warningFacts"][0]["fixSpan"],
        json!({ "start": flowchart_start + "flowchart".len(), "end": flowchart_start + "flowchart".len() })
    );
}

#[test]
fn parse_flowchart_editor_facts_preserve_parser_node_id_spans() {
    let engine = Engine::new();
    let text = "flowchart TD\nA-->B\n";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart-v2", text)
        .unwrap()
        .expect("flowchart editor facts");

    let symbol = |name: &str| {
        facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == name)
            .unwrap_or_else(|| panic!("missing symbol {name}"))
    };

    let a_start = text.find("A-->").unwrap();
    let b_start = text.find("-->B").unwrap() + "-->".len();

    assert_eq!(symbol("A").selection.start, a_start);
    assert_eq!(symbol("A").selection.end, a_start + "A".len());
    assert_eq!(symbol("B").selection.start, b_start);
    assert_eq!(symbol("B").selection.end, b_start + "B".len());
}

#[test]
fn parse_flowchart_editor_facts_match_flowdb_subgraph_ids() {
    let engine = Engine::new();
    let text = "flowchart TD\nsubgraph Explicit\u{feff}[Title]\nA\nend\nsubgraph Auto\u{00a0}Name\nB\nend\n";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart-v2", text)
        .unwrap()
        .expect("flowchart editor facts");

    let explicit = facts
        .symbols
        .iter()
        .find(|symbol| symbol.name == "Explicit")
        .expect("explicit subgraph symbol");
    let explicit_start = text.find("Explicit").unwrap();
    assert_eq!(explicit.selection.start, explicit_start);
    assert_eq!(explicit.selection.end, explicit_start + "Explicit".len());
    assert!(
        facts
            .symbols
            .iter()
            .all(|symbol| symbol.name != "Auto\u{00a0}Name"),
        "FlowDB auto-generates the id for a bare subgraph title containing ECMAScript whitespace"
    );
}

#[test]
fn parse_flowchart_editor_facts_accept_legacy_flowchart_type() {
    let engine = Engine::new();
    let text = "flowchart TD\nA-->B\n";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart", text)
        .unwrap()
        .expect("legacy flowchart editor facts");

    let symbol = facts
        .symbols
        .iter()
        .find(|symbol| symbol.name == "A")
        .expect("flowchart node symbol");
    let a_start = text.find("A-->").unwrap();
    assert_eq!(symbol.selection.start, a_start);
    assert_eq!(symbol.selection.end, a_start + "A".len());
}

#[test]
fn parse_flowchart_editor_facts_preserve_hyphenated_node_id_spans() {
    let engine = Engine::new();
    let text = "flowchart TD\nwi-fi[\"a node with dashes in its name\"]\n";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart-v2", text)
        .unwrap()
        .expect("flowchart editor facts");

    let node = facts
        .symbols
        .iter()
        .find(|symbol| symbol.name == "wi-fi")
        .expect("hyphenated flowchart node symbol");
    let start = text.find("wi-fi").unwrap();
    assert_eq!(node.selection.start, start);
    assert_eq!(node.selection.end, start + "wi-fi".len());
}

#[test]
fn parse_flowchart_editor_facts_emit_label_payload_spans() {
    let engine = Engine::new();
    let text = "flowchart TD\nA[\"Start node\"] & C -->|go| B{\"Decision\"} & D\n";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart-v2", text)
        .unwrap()
        .expect("flowchart editor facts");

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);

    let symbol_with_detail = |name: &str, detail: &str| {
        facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == name && symbol.detail.as_deref() == Some(detail))
            .unwrap_or_else(|| panic!("missing symbol {name} with detail {detail}"))
    };

    let start_label = symbol_with_detail("Start node", "flowchart node label");
    let start_label_open = text.find("[\"Start node\"]").unwrap();
    let start_label_text = text.find("Start node").unwrap();
    assert_eq!(start_label.role, EditorSemanticRole::Payload);
    assert_eq!(start_label.kind, EditorSemanticKind::String);
    assert_eq!(start_label.span.start, start_label_open);
    assert_eq!(start_label.selection.start, start_label_text);
    assert_eq!(
        start_label.selection.end,
        start_label_text + "Start node".len()
    );

    let edge_label = symbol_with_detail("go", "flowchart edge label");
    let edge_label_open = text.find("|go|").unwrap();
    let edge_label_text = text.find("go").unwrap();
    assert_eq!(edge_label.role, EditorSemanticRole::Payload);
    assert_eq!(edge_label.kind, EditorSemanticKind::String);
    assert_eq!(edge_label.span.start, edge_label_open);
    assert_eq!(edge_label.selection.start, edge_label_text);
    assert_eq!(edge_label.selection.end, edge_label_text + "go".len());

    let decision_label = symbol_with_detail("Decision", "flowchart node label");
    let decision_label_open = text.find("{\"Decision\"}").unwrap();
    let decision_label_text = text.find("Decision").unwrap();
    assert_eq!(decision_label.role, EditorSemanticRole::Payload);
    assert_eq!(decision_label.kind, EditorSemanticKind::String);
    assert_eq!(decision_label.span.start, decision_label_open);
    assert_eq!(decision_label.selection.start, decision_label_text);
    assert_eq!(
        decision_label.selection.end,
        decision_label_text + "Decision".len()
    );

    for payload in ["Start node", "go", "Decision"] {
        let start = text.find(payload).unwrap();
        assert!(
            facts.expected_syntax.iter().any(|expected| {
                expected.kind == EditorExpectedSyntaxKind::Payload
                    && expected.span.start <= start
                    && expected.span.end >= start + payload.len()
            }),
            "missing flowchart payload expected syntax for {payload:?}"
        );
    }

    assert_eq!(
        facts
            .symbols
            .iter()
            .filter(|symbol| symbol.detail.as_deref() == Some("flowchart edge label"))
            .count(),
        1
    );
}

#[test]
fn parse_flowchart_editor_facts_recover_label_payload_spans() {
    let engine = Engine::new();
    let text = "flowchart TD\nA[\"Start node\"] -->|go| B{\"Decision\"}\nC-->";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart-v2", text)
        .unwrap()
        .expect("flowchart editor facts");

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);

    let symbol_with_detail = |name: &str, detail: &str| {
        facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == name && symbol.detail.as_deref() == Some(detail))
            .unwrap_or_else(|| panic!("missing symbol {name} with detail {detail}"))
    };

    let start_label = symbol_with_detail("Start node", "flowchart node label");
    let start_label_open = text.find("[\"Start node\"]").unwrap();
    let start_label_text = text.find("Start node").unwrap();
    assert_eq!(start_label.role, EditorSemanticRole::Payload);
    assert_eq!(start_label.kind, EditorSemanticKind::String);
    assert_eq!(start_label.span.start, start_label_open);
    assert_eq!(start_label.selection.start, start_label_text);
    assert_eq!(
        start_label.selection.end,
        start_label_text + "Start node".len()
    );

    let edge_label = symbol_with_detail("go", "flowchart edge label");
    let edge_label_open = text.find("|go|").unwrap();
    let edge_label_text = text.find("go").unwrap();
    assert_eq!(edge_label.role, EditorSemanticRole::Payload);
    assert_eq!(edge_label.kind, EditorSemanticKind::String);
    assert_eq!(edge_label.span.start, edge_label_open);
    assert_eq!(edge_label.selection.start, edge_label_text);
    assert_eq!(edge_label.selection.end, edge_label_text + "go".len());

    let decision_label = symbol_with_detail("Decision", "flowchart node label");
    let decision_label_open = text.find("{\"Decision\"}").unwrap();
    let decision_label_text = text.find("Decision").unwrap();
    assert_eq!(decision_label.role, EditorSemanticRole::Payload);
    assert_eq!(decision_label.kind, EditorSemanticKind::String);
    assert_eq!(decision_label.span.start, decision_label_open);
    assert_eq!(decision_label.selection.start, decision_label_text);
    assert_eq!(
        decision_label.selection.end,
        decision_label_text + "Decision".len()
    );
}

#[test]
fn parse_flowchart_editor_facts_emit_directive_payload_spans() {
    let engine = Engine::new();
    let text = "flowchart TD\nclassDef hot fill:#f00,stroke:#333;\nA-->B\nstyle A fill:#fff\nclass A,B hot\n";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart-v2", text)
        .unwrap()
        .expect("flowchart editor facts");

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);

    let symbol_at = |name: &str, detail: &str, start: usize| {
        facts
            .symbols
            .iter()
            .find(|symbol| {
                symbol.name == name
                    && symbol.detail.as_deref() == Some(detail)
                    && symbol.selection.start == start
            })
            .unwrap_or_else(|| panic!("missing symbol {name} with detail {detail} at {start}"))
    };

    let class_def_start = text.find("classDef hot").unwrap() + "classDef ".len();
    let class_def = symbol_at("hot", "flowchart class definition", class_def_start);
    assert_eq!(class_def.role, EditorSemanticRole::Outline);
    assert_eq!(class_def.kind, EditorSemanticKind::Property);

    let class_def_style_start = text.find("fill:#f00,stroke:#333").unwrap();
    let class_def_style = symbol_at(
        "fill:#f00,stroke:#333",
        "flowchart class definition style",
        class_def_style_start,
    );
    assert_eq!(class_def_style.role, EditorSemanticRole::Payload);
    assert_eq!(class_def_style.kind, EditorSemanticKind::String);
    assert_eq!(
        class_def_style.selection,
        SourceSpan::new(
            class_def_style_start,
            class_def_style_start + "fill:#f00,stroke:#333".len(),
        )
    );

    let style_target_start = text.find("style A").unwrap() + "style ".len();
    let style_target = symbol_at("A", "flowchart style target", style_target_start);
    assert_eq!(style_target.role, EditorSemanticRole::Entity);
    assert_eq!(style_target.kind, EditorSemanticKind::Module);

    let style_payload_start = text.find("fill:#fff").unwrap();
    let style_payload = symbol_at("fill:#fff", "flowchart style", style_payload_start);
    assert_eq!(style_payload.role, EditorSemanticRole::Payload);
    assert_eq!(style_payload.kind, EditorSemanticKind::String);

    let class_target_a_start = text.find("class A,B").unwrap() + "class ".len();
    let class_target_a = symbol_at("A", "flowchart class target", class_target_a_start);
    assert_eq!(class_target_a.role, EditorSemanticRole::Entity);
    assert_eq!(class_target_a.kind, EditorSemanticKind::Module);

    let class_target_b_start = class_target_a_start + "A,".len();
    let class_target_b = symbol_at("B", "flowchart class target", class_target_b_start);
    assert_eq!(class_target_b.role, EditorSemanticRole::Entity);
    assert_eq!(class_target_b.kind, EditorSemanticKind::Module);

    let class_name_start = text.rfind("hot").unwrap();
    let class_name = symbol_at("hot", "flowchart class name", class_name_start);
    assert_eq!(class_name.role, EditorSemanticRole::Payload);
    assert_eq!(class_name.kind, EditorSemanticKind::Property);
}

#[test]
fn parse_flowchart_editor_facts_recover_directive_payload_spans() {
    let engine = Engine::new();
    let text =
        "flowchart TD\nclassDef hot fill:#f00,stroke:#333\nstyle A fill:#fff\nclass A,B hot\nC-->";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart-v2", text)
        .unwrap()
        .expect("flowchart editor facts");

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);

    let symbol_at = |name: &str, detail: &str, start: usize| {
        facts
            .symbols
            .iter()
            .find(|symbol| {
                symbol.name == name
                    && symbol.detail.as_deref() == Some(detail)
                    && symbol.selection.start == start
            })
            .unwrap_or_else(|| panic!("missing symbol {name} with detail {detail} at {start}"))
    };

    let class_def_start = text.find("classDef hot").unwrap() + "classDef ".len();
    assert_eq!(
        symbol_at("hot", "flowchart class definition", class_def_start).role,
        EditorSemanticRole::Outline
    );

    let class_def_style_start = text.find("fill:#f00,stroke:#333").unwrap();
    assert_eq!(
        symbol_at(
            "fill:#f00,stroke:#333",
            "flowchart class definition style",
            class_def_style_start,
        )
        .role,
        EditorSemanticRole::Payload
    );

    let style_target_start = text.find("style A").unwrap() + "style ".len();
    assert_eq!(
        symbol_at("A", "flowchart style target", style_target_start).role,
        EditorSemanticRole::Entity
    );

    let style_payload_start = text.find("fill:#fff").unwrap();
    assert_eq!(
        symbol_at("fill:#fff", "flowchart style", style_payload_start).role,
        EditorSemanticRole::Payload
    );

    let class_target_b_start = text.find("class A,B").unwrap() + "class A,".len();
    assert_eq!(
        symbol_at("B", "flowchart class target", class_target_b_start).role,
        EditorSemanticRole::Entity
    );

    let class_name_start = text.rfind("hot").unwrap();
    assert_eq!(
        symbol_at("hot", "flowchart class name", class_name_start).role,
        EditorSemanticRole::Payload
    );
}

#[test]
fn parse_flowchart_editor_facts_emit_shape_value_expected_syntax() {
    let engine = Engine::new();
    let text = "flowchart TD\nA@{\n  shape: rounded\n}\n";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart-v2", text)
        .unwrap()
        .expect("flowchart editor facts");

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);

    let shape_start = text.find("rounded").unwrap();
    assert!(
        facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::ShapeValue
                && expected.span == SourceSpan::new(shape_start, shape_start + "rounded".len())
        }),
        "missing shape value expected syntax"
    );
}

#[test]
fn parse_flowchart_editor_facts_emit_standalone_shape_data_node_symbol() {
    let engine = Engine::new();
    let text = "flowchart TD\nD@{ shape: rounded }\nD --> E\n";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart-v2", text)
        .unwrap()
        .expect("flowchart editor facts");

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);

    let standalone_d_start = text.find("D@{").unwrap();
    let standalone_d = facts
        .symbols
        .iter()
        .find(|symbol| {
            symbol.name == "D"
                && symbol.detail.as_deref() == Some("flowchart node")
                && symbol.selection == SourceSpan::new(standalone_d_start, standalone_d_start + 1)
        })
        .expect("standalone shapeData node symbol");

    assert_eq!(standalone_d.role, EditorSemanticRole::Entity);
    assert_eq!(standalone_d.kind, EditorSemanticKind::Module);
}

#[test]
fn parse_flowchart_editor_facts_do_not_emit_edge_shape_data_as_node_symbol() {
    let engine = Engine::new();
    let text = "flowchart TD\nA e1@--> B\ne1@{ curve: basis }\n";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart-v2", text)
        .unwrap()
        .expect("flowchart editor facts");

    let edge_shape_data_start = text.rfind("e1@{").unwrap();
    assert!(
        !facts.symbols.iter().any(|symbol| {
            symbol.name == "e1"
                && symbol.detail.as_deref() == Some("flowchart node")
                && symbol.selection
                    == SourceSpan::new(edge_shape_data_start, edge_shape_data_start + 2)
        }),
        "edge shapeData target must not be projected as a node symbol"
    );
}

#[test]
fn parse_flowchart_editor_facts_emit_direction_value_expected_syntax() {
    let engine = Engine::new();
    let text = "flowchart TD\nsubgraph group\ndirection LR\nend\n";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart-v2", text)
        .unwrap()
        .expect("flowchart editor facts");

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);

    let dir_start = text.find("LR").unwrap();
    assert!(
        facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::DirectionValue
                && expected.span == SourceSpan::new(dir_start, dir_start + "LR".len())
        }),
        "missing direction value expected syntax"
    );
}

#[test]
fn parse_flowchart_editor_facts_emit_shape_trigger_expected_syntax() {
    let engine = Engine::new();
    let text = "flowchart TD\nA((\n";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart-v2", text)
        .unwrap()
        .expect("flowchart editor facts");

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);

    let trigger_start = text.find("((").unwrap();
    assert!(
        facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::ShapeTrigger
                && expected.span == SourceSpan::new(trigger_start, trigger_start + 2)
        }),
        "missing shape trigger expected syntax"
    );
}

#[test]
fn parse_flowchart_editor_facts_recover_shape_value_expected_syntax() {
    let engine = Engine::new();
    let text = "flowchart TD\nA@{\n  shape: rounded\n}\nC-->";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart-v2", text)
        .unwrap()
        .expect("flowchart editor facts");

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);

    let shape_start = text.find("rounded").unwrap();
    assert!(
        facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::ShapeValue
                && expected.span == SourceSpan::new(shape_start, shape_start + "rounded".len())
        }),
        "missing recovered shape value expected syntax"
    );
}

#[test]
fn parse_flowchart_editor_facts_recover_unterminated_shape_data_value() {
    let engine = Engine::new();
    let text = "flowchart TD\nA@{ shape: rou";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart-v2", text)
        .unwrap()
        .expect("flowchart editor facts");

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);

    let shape_start = text.find("rou").unwrap();
    assert!(facts.expected_syntax.iter().any(|expected| {
        expected.kind == EditorExpectedSyntaxKind::ShapeValue
            && expected.span == SourceSpan::new(shape_start, shape_start + "rou".len())
    }));

    let error = engine
        .parse_diagram_sync(text, ParseOptions::strict())
        .expect_err("unterminated shape data must fail strict parsing");
    assert!(
        error.to_string().contains("Unterminated shape data"),
        "{error}"
    );
}

#[test]
fn parse_flowchart_editor_facts_preserve_directive_prefixes() {
    let engine = Engine::new();
    let text = concat!(
        "%%{init: {\"theme\": \"dark\"}}%%\n",
        "flowchart TD\n",
        "accTitle: Flow title\n",
        "accDescr: Flow description\n",
        "classDef hot fill:#f00\n",
        "A-->B\n",
    );
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart-v2", text)
        .unwrap()
        .expect("flowchart editor facts");

    assert!(
        facts
            .directive_prefixes
            .iter()
            .any(|prefix| prefix == "init")
    );
    assert!(
        facts
            .directive_prefixes
            .iter()
            .any(|prefix| prefix == "classDef")
    );
    assert!(
        facts
            .directive_prefixes
            .iter()
            .any(|prefix| prefix == "accTitle")
    );
    assert!(
        !facts
            .directive_prefixes
            .iter()
            .any(|prefix| prefix == "accDescription")
    );
    assert!(
        facts
            .directive_prefixes
            .iter()
            .any(|prefix| prefix == "accDescr")
    );

    let a = facts
        .symbols
        .iter()
        .find(|symbol| symbol.name == "A")
        .expect("node A editor symbol");
    let a_start = text.find("A-->").unwrap();
    assert_eq!(a.selection.start, a_start);
    assert_eq!(a.selection.end, a_start + "A".len());
}

#[test]
fn parse_flowchart_editor_facts_recovers_from_incomplete_input() {
    let engine = Engine::new();
    let text = "flowchart TD\nsubgraph group\nA-->B\nC-->";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart-v2", text)
        .unwrap()
        .expect("flowchart editor facts");

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);

    let symbol = |name: &str| {
        facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == name)
            .unwrap_or_else(|| panic!("missing symbol {name}"))
    };

    let group_start = text.find("group").unwrap();
    assert_eq!(symbol("group").selection.start, group_start);
    assert_eq!(symbol("group").selection.end, group_start + "group".len());

    let c_start = text.find("C-->").unwrap();
    assert_eq!(symbol("C").selection.start, c_start);
    assert_eq!(symbol("C").selection.end, c_start + "C".len());

    assert!(facts.expected_syntax.iter().any(|expected| {
        expected.kind == EditorExpectedSyntaxKind::NodeIdentifier
            && expected.span == SourceSpan::new(text.len(), text.len())
    }));
}

#[test]
fn parse_flowchart_eof_diagnostic_precedes_a_trailing_line_ending() {
    let source = "flowchart TD\nA-->\n";
    let snapshot = Engine::new()
        .parse_diagram_snapshot_with_type_sync("flowchart-v2", source)
        .unwrap()
        .expect("flowchart snapshot");
    let DiagramParseOutcome::Failed(Error::DiagramParse { diagnostic, .. }) = snapshot.outcome()
    else {
        panic!("incomplete edge must return a structured parse diagnostic");
    };

    assert_eq!(
        diagnostic.span(),
        Some(SourceSpan::new(source.len() - 1, source.len() - 1))
    );
}

#[test]
fn parse_flowchart_editor_facts_recovers_from_malformed_label_without_hanging() {
    let engine = Engine::new();
    let text = "flowchart TD\nA[bad (label)]\nB-->C\n";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart-v2", text)
        .unwrap()
        .expect("flowchart editor facts");

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert!(facts.symbols.iter().any(|symbol| symbol.name == "A"));
    assert!(facts.symbols.iter().any(|symbol| symbol.name == "B"));
    assert!(facts.symbols.iter().any(|symbol| symbol.name == "C"));
}

#[test]
fn parse_flowchart_editor_facts_expect_target_after_pipe_edge_label() {
    let engine = Engine::new();
    let text = "flowchart TD\nA-->B\nA -->|go|";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("flowchart-v2", text)
        .unwrap()
        .expect("flowchart editor facts");

    assert!(facts.expected_syntax.iter().any(|expected| {
        expected.kind == EditorExpectedSyntaxKind::NodeIdentifier
            && expected.span == SourceSpan::new(text.len(), text.len())
    }));
}

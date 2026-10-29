#![cfg(feature = "diagram-flowchart")]

use merman_core::{
    DiagramParseOutcome, EditorSemanticCompleteness, EditorSemanticRole, Engine, Error,
    ParseDiagnosticSpanKind, ParseOptions, ParsedEditorFacts, RenderSemanticModel, SourceSpan,
};
use serde_json::json;

#[test]
fn flowchart_percent_identifiers_and_preprocessed_comments_match_mermaid() {
    type Case = (
        &'static str,
        &'static [&'static str],
        &'static [(&'static str, &'static str)],
    );
    // Mermaid 12.1.0's pinned grammar and comment preprocessing accepts these sources. A bare marker has no
    // comment body, and markers following a semicolon remain ordinary node identifiers.
    let cases: &[Case] = &[
        (
            "flowchart TD\nA-->B\n%% comment\nC-->D",
            &["A", "B", "C", "D"],
            &[("A", "B"), ("C", "D")],
        ),
        (
            "flowchart TD\nA-->B\n  \t%% comment\nC-->D",
            &["A", "B", "C", "D"],
            &[("A", "B"), ("C", "D")],
        ),
        (
            "flowchart TD\nA-->B\n\u{00a0}%% comment\nC-->D",
            &["A", "B", "C", "D"],
            &[("A", "B"), ("C", "D")],
        ),
        (
            "flowchart TD\nA-->B\n%% comment",
            &["A", "B"],
            &[("A", "B")],
        ),
        ("flowchart TD\n%%", &["%%"], &[]),
        ("flowchart TD\n%%\n", &["%%"], &[]),
        ("flowchart TD\n%%\nA-->B", &["%%", "A", "B"], &[("A", "B")]),
        (
            "flowchart TD\n  %%\nA-->B",
            &["%%", "A", "B"],
            &[("A", "B")],
        ),
        ("flowchart TD\n%% \nA-->B", &["A", "B"], &[("A", "B")]),
        ("flowchart TD\n%-->B", &["%", "B"], &[("%", "B")]),
        ("flowchart TD\n%A-->B", &["%A", "B"], &[("%A", "B")]),
        ("flowchart TD\nA%-->B", &["A%", "B"], &[("A%", "B")]),
        ("flowchart TD\nA%%B-->C", &["A%%B", "C"], &[("A%%B", "C")]),
        (
            "flowchart TD\nA-->B%%tail",
            &["A", "B%%tail"],
            &[("A", "B%%tail")],
        ),
        (
            "flowchart TD\nA-->B; %%tail",
            &["A", "B", "%%tail"],
            &[("A", "B")],
        ),
        ("flowchart TD\nA-->B; %%", &["A", "B", "%%"], &[("A", "B")]),
        (
            "%% leading comment\nflowchart TD\nA-->B",
            &["A", "B"],
            &[("A", "B")],
        ),
        (
            "flowchart TD\r\n%% 中文😀 comment\r\n  %% indented\r\n雪%-->B%%tail\r\n%% eof",
            &["雪%", "B%%tail"],
            &[("雪%", "B%%tail")],
        ),
        (
            "flowchart TD\r\n%%\r\n雪%-->B\r\n",
            &["%%", "雪%", "B"],
            &[("雪%", "B")],
        ),
    ];
    let engine = Engine::new();
    for &(source, expected_ids, expected_edges) in cases {
        let check_model = |model: &serde_json::Value| {
            let ids = model["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|node| node["id"].as_str().unwrap())
                .collect::<Vec<_>>();
            let edges = model["edges"]
                .as_array()
                .unwrap()
                .iter()
                .map(|edge| (edge["from"].as_str().unwrap(), edge["to"].as_str().unwrap()))
                .collect::<Vec<_>>();
            assert_eq!(ids, expected_ids, "{source}");
            assert_eq!(edges, expected_edges, "{source}");
        };
        for known_type in [false, true] {
            for options in [ParseOptions::strict(), ParseOptions::lenient()] {
                let parsed = if known_type {
                    engine.parse_diagram_with_type_sync("flowchart-v2", source, options)
                } else {
                    engine.parse_diagram_sync(source, options)
                }
                .unwrap()
                .unwrap();
                check_model(&parsed.model);
                let typed = if known_type {
                    engine.parse_diagram_for_render_model_with_type_sync(
                        "flowchart-v2",
                        source,
                        options,
                    )
                } else {
                    engine.parse_diagram_for_render_model_sync(source, options)
                }
                .unwrap()
                .unwrap();
                assert!(matches!(typed.model(), RenderSemanticModel::Flowchart(_)));
                assert_eq!(
                    typed.model().compatibility_json(typed.metadata()).unwrap(),
                    parsed.model,
                    "typed percent/comment semantics: {source}"
                );
            }
            let snapshot = if known_type {
                engine.parse_diagram_snapshot_with_type_sync("flowchart-v2", source)
            } else {
                engine.parse_diagram_snapshot_sync(source)
            }
            .unwrap()
            .unwrap();
            check_model(snapshot.outcome().parsed_model().expect("accepted source"));
            let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("accepted percent source must retain editor facts: {source}");
            };
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        }
    }
}

#[test]
fn flowchart_percent_directive_prefixes_remain_identifiers() {
    let engine = Engine::new();
    for prefix in [
        "direction",
        "click",
        "default",
        "call",
        "href",
        "accTitle",
        "accDescr",
        "LR",
        "RL",
        "TB",
        "TD",
        "BT",
        "BR",
        "v",
    ] {
        for suffix in ["%", "%%tail", "-tail"] {
            let id = format!("{prefix}{suffix}");
            let source = format!("flowchart TD\n{id}-->B");
            let parsed = engine
                .parse_diagram_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            assert_eq!(parsed.model["nodes"][0]["id"], id);
            assert_eq!(parsed.model["edges"][0]["from"], id);
        }
    }
}

#[test]
fn flowchart_class_names_preserve_reserved_keyword_prefixes_and_source_spans() {
    let engine = Engine::new();
    let fixture = include_str!(
        "../../../fixtures/flowchart/upstream_cypress_flowchart_v2_spec_65_3_bigger_font_from_classes_026.mmd"
    );
    let parsed = engine
        .parse_diagram_sync(fixture, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let nodes = parsed.model["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 4);
    assert_eq!(nodes[0]["classes"], json!(["class1", "class1"]));
    assert_eq!(nodes[1]["classes"], json!(["class2"]));
    assert_eq!(nodes[2]["classes"], json!(["class2"]));
    assert_eq!(nodes[3]["classes"], json!(["larger"]));
    assert_eq!(
        parsed.model["classDefs"]["larger"],
        json!(["font-size:30px", "fill:yellow"])
    );
    assert_eq!(parsed.model["edges"].as_array().unwrap().len(), 3);

    for class_name in ["class1", "class_1", "classDefinition", "_blank1"] {
        let source = format!(
            "%% deleted 中文😀\r\nflowchart TD\r\nclassDef {class_name} fill:red\r\nA:::{class_name} --> {class_name}\r\nclass A {class_name}\r\n"
        );
        let parsed = engine
            .parse_diagram_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.model["nodes"][0]["id"], "A");
        assert_eq!(
            parsed.model["nodes"][0]["classes"],
            json!([class_name, class_name])
        );
        assert_eq!(parsed.model["classDefs"][class_name], json!(["fill:red"]));
        assert_eq!(parsed.model["edges"][0]["to"], class_name);

        let snapshot = engine
            .parse_diagram_snapshot_with_type_sync("flowchart-v2", &source)
            .unwrap()
            .unwrap();
        let DiagramParseOutcome::Parsed { model, .. } = snapshot.outcome() else {
            panic!("a class name with a non-boundary suffix must parse: {class_name}");
        };
        assert_eq!(
            model["nodes"][0]["classes"],
            json!([class_name, class_name])
        );
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("class declarations and assignments must retain source spans");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        for (start, role) in [
            (
                source.find(class_name).unwrap(),
                EditorSemanticRole::ClassDefinition,
            ),
            (
                source.rfind(class_name).unwrap(),
                EditorSemanticRole::Payload,
            ),
        ] {
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == class_name
                    && symbol.role == role
                    && symbol.selection == SourceSpan::new(start, start + class_name.len())
            }));
        }
    }
}

#[test]
fn flowchart_literal_keyword_boundaries_preserve_ids_classes_and_source_spans() {
    let engine = Engine::new();
    for keyword in [
        "graph",
        "flowchart",
        "flowchart-elk",
        "swimlane-beta",
        "end",
        "style",
        "linkStyle",
        "classDef",
        "class",
        "subgraph",
        "interpolate",
        "_self",
        "_blank",
        "_parent",
        "_top",
    ] {
        for suffix in ["1", "_1", "word", "%", "-tail", "雪"] {
            let id = format!("{keyword}{suffix}");
            let accepted = matches!(suffix, "1" | "_1" | "word") && keyword != "flowchart-elk";
            for shorthand in [false, true] {
                let body = if shorthand {
                    format!("A:::{id}-->B")
                } else {
                    format!("{id}-->B")
                };
                let source = format!("%% deleted 中文😀\r\nflowchart TD\r\n{body}\r\n");
                let parsed = engine.parse_diagram_sync(&source, ParseOptions::strict());
                if accepted {
                    let parsed = parsed.unwrap().unwrap();
                    if shorthand {
                        assert_eq!(parsed.model["nodes"][0]["id"], "A");
                        assert_eq!(parsed.model["nodes"][0]["classes"], json!([id]));
                    } else {
                        assert_eq!(parsed.model["nodes"][0]["id"], id);
                        assert_eq!(parsed.model["edges"][0]["from"], id);
                    }
                    continue;
                }

                let matched_keyword =
                    if keyword == "flowchart-elk" && matches!(suffix, "1" | "_1" | "word") {
                        "flowchart"
                    } else {
                        keyword
                    };
                let start = source.rfind(&id).unwrap();
                let expected = SourceSpan::new(start, start + matched_keyword.len());
                let Error::DiagramParse { diagnostic, .. } = parsed.unwrap_err() else {
                    panic!("a keyword boundary must remain a parse error: {source}");
                };
                assert_eq!(diagnostic.span(), Some(expected), "{source}");
                assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
                let snapshot = engine
                    .parse_diagram_snapshot_sync(&source)
                    .unwrap()
                    .unwrap();
                let DiagramParseOutcome::Failed(Error::DiagramParse { diagnostic, .. }) =
                    snapshot.outcome()
                else {
                    panic!("editor capture must retain the keyword boundary failure: {source}");
                };
                assert_eq!(diagnostic.span(), Some(expected), "{source}");
            }
        }
    }
}

#[test]
fn flowchart_literal_header_keywords_keep_header_admission() {
    let engine = Engine::new();
    for header in ["graph", "flowchart", "flowchart-elk", "swimlane-beta"] {
        let source = format!("{header} TD\nA-->B\n");
        let parsed = engine
            .parse_diagram_with_type_sync("flowchart-v2", &source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.model["edges"][0]["from"], "A");
        for suffix in ["1", "_1", "word", "%", "-tail", "雪"] {
            let invalid = format!("{header}{suffix} TD\nA-->B\n");
            assert!(
                engine
                    .parse_diagram_sync(&invalid, ParseOptions::strict())
                    .is_err(),
                "{invalid}"
            );
            assert!(
                engine
                    .parse_diagram_with_type_sync("flowchart-v2", &invalid, ParseOptions::strict())
                    .is_err(),
                "{invalid}"
            );
        }
    }
}

#[test]
fn flowchart_percent_after_header_direction_is_not_a_body_node() {
    let engine = Engine::new();
    for direction in ["LR", "RL", "TB", "TD", "BT"] {
        for suffix in ["%", "%%tail"] {
            let source = format!("flowchart {direction}{suffix}\nA-->B");
            assert!(
                engine
                    .parse_diagram_sync(&source, ParseOptions::strict())
                    .is_err(),
                "{source}"
            );
            let snapshot = engine
                .parse_diagram_snapshot_sync(&source)
                .unwrap()
                .unwrap();
            assert!(
                matches!(snapshot.outcome(), DiagramParseOutcome::Failed(_)),
                "{source}"
            );
        }
    }
    for source in [
        "flowchart LR A-->B",
        "flowchart LR ;A-->B",
        "flowchart;A-->B",
        "flowchart ;A-->B",
        "flowchart X-->B",
        "flowchart direction LR\nA-->B",
        "flowchart style A fill:red",
    ] {
        assert!(
            engine
                .parse_diagram_sync(source, ParseOptions::strict())
                .is_err(),
            "{source}"
        );
    }
    for (header, direction) in [
        ("LR", "LR"),
        ("BR", "BR"),
        ("<", "RL"),
        (">", "LR"),
        ("^", "BT"),
        ("v", "TB"),
    ] {
        for separator in [";", "\n", " \t\n"] {
            let source = format!("flowchart {header}{separator}A-->B");
            let parsed = engine
                .parse_diagram_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            assert_eq!(parsed.model["direction"], direction, "{source}");
            assert_eq!(parsed.model["nodes"][0]["id"], "A", "{source}");
            assert_eq!(parsed.model["edges"][0]["to"], "B", "{source}");
        }
        let source = format!("flowchart {header}");
        let parsed = engine
            .parse_diagram_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.model["direction"], direction, "{source}");
    }
}

#[test]
fn flowchart_inline_percent_tokens_retain_parse_failures_and_error_suppression() {
    let engine = Engine::new();
    for source in [
        "flowchart TD\nA-->B %% comment",
        "flowchart TD\nA-->B %%",
        "flowchart TD\nA-->B %%tail",
        "flowchart TD\nA-->B %",
        "flowchart TD\nA-->B %% comment\nC-->D",
        "flowchart TD\nA-->B; %% comment",
        "flowchart TD %% comment\nA-->B",
    ] {
        for known_type in [false, true] {
            let strict_json = if known_type {
                engine.parse_diagram_with_type_sync("flowchart-v2", source, ParseOptions::strict())
            } else {
                engine.parse_diagram_sync(source, ParseOptions::strict())
            }
            .expect_err("inline percent tokens must reach the grammar");
            assert!(matches!(strict_json, Error::DiagramParse { .. }));
            let strict_typed = if known_type {
                engine.parse_diagram_for_render_model_with_type_sync(
                    "flowchart-v2",
                    source,
                    ParseOptions::strict(),
                )
            } else {
                engine.parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            }
            .expect_err("typed parsing must retain the same grammar failure");
            assert_eq!(strict_typed.to_string(), strict_json.to_string());

            let lenient_json = if known_type {
                engine.parse_diagram_with_type_sync("flowchart-v2", source, ParseOptions::lenient())
            } else {
                engine.parse_diagram_sync(source, ParseOptions::lenient())
            }
            .unwrap()
            .unwrap();
            let lenient_typed = if known_type {
                engine.parse_diagram_for_render_model_with_type_sync(
                    "flowchart-v2",
                    source,
                    ParseOptions::lenient(),
                )
            } else {
                engine.parse_diagram_for_render_model_sync(source, ParseOptions::lenient())
            }
            .unwrap()
            .unwrap();
            assert_eq!(lenient_json.meta.diagram_type, "error");
            assert_eq!(lenient_json.model["errorMessage"], strict_json.to_string());
            let RenderSemanticModel::Error(error_model) = lenient_typed.model() else {
                panic!("lenient typed parsing must use the existing error family");
            };
            assert_eq!(
                error_model.error_message.as_deref(),
                Some(strict_json.to_string().as_str())
            );

            let snapshot = if known_type {
                engine.parse_diagram_snapshot_with_type_sync("flowchart-v2", source)
            } else {
                engine.parse_diagram_snapshot_sync(source)
            }
            .unwrap()
            .unwrap();
            let DiagramParseOutcome::Failed(error) = snapshot.outcome() else {
                panic!("snapshots must retain the original failure: {source}");
            };
            assert_eq!(error.to_string(), strict_json.to_string());
            let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("failed inline source must retain recovery facts: {source}");
            };
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        }
    }
}

#[test]
fn flowchart_percent_labels_match_mermaid_preprocessing() {
    let engine = Engine::new();
    for (source, expected_label, expected_kind) in [
        ("flowchart TD\nA[100%% done]-->B", "100%% done", "text"),
        (
            "flowchart TD\nA[\"100%% done\"]-->B",
            "100%% done",
            "string",
        ),
        (
            "flowchart TD\nA[\"`100%% done`\"]-->B",
            "100%% done",
            "markdown",
        ),
        (
            "flowchart TD\nA[\"first\n%% comment\nlast\"]-->B",
            "first\nlast",
            "string",
        ),
        (
            "flowchart TD\nA[\"`first\n%% comment\nlast`\"]-->B",
            "first\nlast",
            "markdown",
        ),
    ] {
        let parsed = engine
            .parse_diagram_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(
            parsed.model["nodes"][0]["label"], expected_label,
            "{source}"
        );
        assert_eq!(
            parsed.model["nodes"][0]["labelType"], expected_kind,
            "{source}"
        );
        let typed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(
            typed.model().compatibility_json(typed.metadata()).unwrap(),
            parsed.model
        );
    }
    for (source, expected_kind) in [
        ("flowchart TD\nA-->|\"100%% done\"|B", "string"),
        ("flowchart TD\nA-->|\"`100%% done`\"|B", "markdown"),
    ] {
        let parsed = engine
            .parse_diagram_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.model["edges"][0]["label"], "100%% done");
        assert_eq!(parsed.model["edges"][0]["labelType"], expected_kind);
    }
}

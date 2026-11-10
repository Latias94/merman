use super::*;
use merman_model::families::flowchart::FlowDroppedSubgraphMember;

fn metadata() -> crate::SemanticMetadata {
    crate::SemanticMetadata {
        diagram_type: "flowchart-v2".to_string(),
        config: MermaidConfig::empty_object(),
        semantic_config: MermaidConfig::empty_object(),
        title: None,
    }
}

fn parse(source: &str) -> FlowchartModel {
    parse_flowchart_model_for_render(source, &metadata()).expect("valid Flowchart source")
}

#[cfg(feature = "editor-facts")]
#[test]
fn unclosed_label_recovery_reuses_quote_frames_without_losing_later_valid_labels() {
    let suffix = concat!(
        "B[\"`first\n雪 [value] (round) | -->\nlast`\"]\n",
        "C[\"first\n雪 [value] (round) | -->\nlast\"]\nD[valid]\n",
    );
    for repetitions in [0, 64, 128, 256] {
        let source = format!("flowchart TD\n{}{suffix}", "A[\"`\"]\n".repeat(repetitions));
        let mut lexer = Lexer::recovering(&source);
        let tokens = lexer
            .by_ref()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        let labels = tokens
            .iter()
            .filter_map(|(_, token, _)| match token {
                Tok::NodeLabel(label) => Some(label),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(labels.len(), repetitions + 3);
        assert!(
            labels[..repetitions]
                .iter()
                .all(|label| label.recovery_error.is_some())
        );
        assert!(
            labels[repetitions..]
                .iter()
                .all(|label| label.recovery_error.is_none())
        );
        assert_eq!(labels[repetitions].text.kind, TitleKind::Markdown);
        assert_eq!(labels[repetitions + 1].text.kind, TitleKind::String);
        for label in &labels[repetitions..repetitions + 2] {
            assert_eq!(label.text.text, "first\n雪 [value] (round) | -->\nlast");
        }
        assert_eq!(labels[repetitions + 2].text.text, "valid");
        assert!(
            lexer.label_scan_work() <= 4 * source.len(),
            "quote scan work must be linear: {repetitions} lines, {} steps for {} bytes",
            lexer.label_scan_work(),
            source.len(),
        );
        assert_eq!(
            parse_flowchart_model_for_render(&source, &metadata()).is_ok(),
            repetitions == 0,
            "unclosed Markdown must remain invalid",
        );
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn long_label_scan_observes_cancellation_before_materializing_its_payload() {
    let source = format!("flowchart TD\nA[\"`{}`\"]\n", "雪".repeat(32_768));
    let control = OperationControl::new().for_phase(crate::OperationPhase::Parse);
    control.cancel_after_checkpoints(0);
    let mut lexer = Lexer::recovering(&source).with_control(&control);
    assert!(lexer.by_ref().any(|item| item.is_err()));
    assert!(control.is_cancelled());
    assert_eq!(lexer.label_scan_work(), 4096);
    assert!(lexer.next().is_none());

    let control = OperationControl::new().for_phase(crate::OperationPhase::Parse);
    control.cancel_after_checkpoints(1);
    assert!(matches!(
        construct_flowchart_token_trace(&source, &control),
        Err(crate::OperationCancelled {
            phase: crate::OperationPhase::Parse,
            reason: crate::CancelReason::Requested,
        })
    ));
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn subgraphs_keep_the_parsed_string_payload_and_contextual_token_kind() {
    let parser = crate::SemanticParser::new();
    for (raw, title, explicit_kind, bare_kind) in [
        (r#"""``"`ok`""#, "`ok`", "string", "text"),
        (r#""`ok`""#, "ok", "markdown", "markdown"),
        ("`ok`", "`ok`", "text", "text"),
    ] {
        let source = format!("flowchart TD\nsubgraph SG[{raw}]\nA\nend\nsubgraph {raw}\nB\nend\n");
        let assert_subgraphs = |model: &FlowchartModel| {
            assert_eq!(model.subgraphs.len(), 2);
            assert_eq!(model.subgraphs[0].id, "SG");
            assert_eq!(model.subgraphs[1].id, title);
            for (subgraph, kind) in model.subgraphs.iter().zip([explicit_kind, bare_kind]) {
                assert_eq!(subgraph.title, title, "{source:?}");
                assert_eq!(subgraph.label_type.as_deref(), Some(kind), "{source:?}");
            }
        };
        assert_subgraphs(&parse(&source));
        for known_type in [false, true] {
            let parsed = if known_type {
                parser.parse_diagram_with_type_sync("flowchart-v2", &source)
            } else {
                parser.parse_diagram_sync(&source)
            }
            .expect("single STR and MD_STR subgraph headers must parse");
            let crate::SemanticDiagramModel::Flowchart(model) = parsed.model else {
                panic!("expected Flowchart model");
            };
            assert_subgraphs(&model);
        }
    }
}

#[test]
fn empty_quote_transitions_keep_the_actual_payload_selection() {
    let source = "flowchart TD\nA[\"\"``\"`\"]\n";
    let tokens = Lexer::new(source)
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap();
    let label = tokens
        .iter()
        .find_map(|(_, token, _)| match token {
            Tok::NodeLabel(label) => Some(&label.text),
            _ => None,
        })
        .unwrap();
    assert_eq!(label.text, "`");
    assert_eq!(label.kind, TitleKind::String);
    assert_eq!(label.selection, Some(SourceSpan::new(20, 21)));
    assert_eq!(parse(source).nodes[0].label.as_deref(), Some("`"));
}

#[cfg(all(feature = "editor-facts", feature = "diagram-flowchart"))]
#[test]
fn payload_selections_survive_unicode_crlf_preprocessing_and_editor_recovery() {
    use crate::{
        EditorSemanticCompleteness, ParsedEditorFacts, SemanticParser, SemanticSnapshotCapture,
    };

    let parser = SemanticParser::new();
    for (raw, payload, offset) in [
        (r#"""``"`""#.to_string(), "`", 5),
        ("\"\"``\"\u{00a0}`雪`\u{feff}\"".to_string(), "`雪`", 7),
    ] {
        let prefix = "\u{feff}---\r\ntitle: 雪😀\r\n---\r\n%% 中文😀\r\nflowchart TD\r\n";
        let mut source = prefix.to_string();
        let mut expected = Vec::new();
        for (before, after, detail) in [
            ("A[", "]\r\n", "flowchart node label"),
            ("A -->|", "| B\r\n", "flowchart edge label"),
            ("B -- ", " --> C\r\n", "flowchart edge label"),
        ] {
            source.push_str(before);
            let start = source.len() + offset;
            expected.push((detail, SourceSpan::new(start, start + payload.len())));
            source.push_str(&raw);
            source.push_str(after);
        }
        if payload == "`" {
            source.push_str("subgraph ");
            let start = source.len() + offset;
            expected.push(("subgraph", SourceSpan::new(start, start + payload.len())));
            source.push_str(&raw);
            source.push_str("\r\nD\r\nend\r\n");
        }
        source.push_str("subgraph ");
        let start = source.len() + offset;
        expected.push(("subgraph", SourceSpan::new(start, start + payload.len())));
        source.push_str(&raw);
        source.push_str("[Title]\r\nE\r\nend\r\n");

        for known_type in [false, true] {
            let parsed = if known_type {
                parser.parse_diagram_with_type_sync("flowchart-v2", &source)
            } else {
                parser.parse_diagram_sync(&source)
            }
            .expect("empty quote transitions must preserve a single string token");
            let crate::SemanticDiagramModel::Flowchart(model) = parsed.model else {
                panic!("expected Flowchart model");
            };
            assert_eq!(model.nodes[0].label.as_deref(), Some(payload));
            assert!(
                model
                    .edges
                    .iter()
                    .all(|edge| edge.label.as_deref() == Some(payload))
            );
        }
        for suffix in ["", "F -->"] {
            let source = format!("{source}{suffix}");
            let SemanticSnapshotCapture::Snapshot(snapshot) =
                parser.capture_diagram_snapshot_sync(&source)
            else {
                panic!("source snapshot must survive label recovery");
            };
            let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("expected Flowchart editor facts");
            };
            assert_eq!(
                facts.completeness,
                if suffix.is_empty() {
                    EditorSemanticCompleteness::Complete
                } else {
                    EditorSemanticCompleteness::Recovered
                },
            );
            for (detail, selection) in &expected {
                assert!(
                    facts.symbols.iter().any(|symbol| {
                        symbol.name == payload
                            && symbol.detail.as_deref() == Some(*detail)
                            && symbol.selection == *selection
                    }),
                    "missing {detail} selection {selection:?} in {source:?}",
                );
                assert_eq!(&source[selection.start..selection.end], payload);
            }
        }
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn parser_differential_markdown_replays_use_the_complete_source_pipeline() {
    let parser = crate::SemanticParser::new();
    for known_type in [false, true] {
        let parse_source = |source: &str| {
            if known_type {
                parser.parse_diagram_with_type_sync("flowchart-v2", source)
            } else {
                parser.parse_diagram_sync(source)
            }
        };
        let invalid = "flowchart TD\nA[\"`first\n  X[\"雪 &amp; text\"]\nlast`\"]";
        assert!(parse_source(invalid).is_err(), "{invalid:?}");

        let source = "flowchart TD\nA[\"`first\n%% comment\n\n%% comment\nlast`\"]";
        let parsed = parse_source(source).expect("comment runs must preserve valid Markdown");
        let crate::SemanticDiagramModel::Flowchart(model) = parsed.model else {
            panic!("expected Flowchart model");
        };
        assert_eq!(model.nodes.len(), 1);
        assert_eq!(model.nodes[0].id, "A");
        assert_eq!(model.nodes[0].label.as_deref(), Some("first\nlast"));
        assert_eq!(model.nodes[0].label_type.as_deref(), Some("markdown"));

        let html = "flowchart TD\nA[\"`first\n<span title=\"雪\">text</span>\nlast`\"]";
        assert!(
            parse_source(html).is_ok(),
            "HTML attribute quotes must be normalized before Markdown lexing: {html:?}",
        );
    }
}

#[test]
fn markdown_empty_quotes_preserve_token_payloads_in_raw_and_public_parsers() {
    let parser = crate::SemanticParser::new();
    for (diagram_type, header) in [
        #[cfg(feature = "diagram-flowchart")]
        ("flowchart-v2", "flowchart TD"),
        #[cfg(feature = "diagram-swimlane")]
        ("swimlane", "swimlane-beta TD"),
    ] {
        for raw in [r#""`""ok`""#, r#""`ok`""""#, r#""`""ok`""""#] {
            let source = format!(
                "{header}\nA[{raw}]\nF({raw})\nG{{{raw}}}\nA -->|{raw}| B\nB -- {raw} --> C\nsubgraph SG[{raw}]\nD\nend\nsubgraph {raw}\nE\nend\n"
            );
            let assert_labels = |model: &FlowchartModel| {
                for id in ["A", "F", "G"] {
                    let node = model.nodes.iter().find(|node| node.id == id).unwrap();
                    assert_eq!(node.label.as_deref(), Some("ok"), "{source:?}");
                    assert_eq!(node.label_type.as_deref(), Some("markdown"));
                }
                assert_eq!(model.edges.len(), 2);
                for edge in &model.edges {
                    assert_eq!(edge.label.as_deref(), Some("ok"), "{source:?}");
                    assert_eq!(edge.label_type.as_deref(), Some("markdown"));
                }
                assert_eq!(model.subgraphs.len(), 2);
                for subgraph in &model.subgraphs {
                    assert_eq!(subgraph.title, "ok", "{source:?}");
                    assert_eq!(subgraph.label_type.as_deref(), Some("markdown"));
                }
                assert_eq!(model.subgraphs[1].id, "ok", "{source:?}");
            };
            assert_labels(&parse(&source));
            for line_ending in ["\n", "\r\n"] {
                let source = source.replace('\n', line_ending);
                for known_type in [false, true] {
                    let parsed = if known_type {
                        parser.parse_diagram_with_type_sync(diagram_type, &source)
                    } else {
                        parser.parse_diagram_sync(&source)
                    }
                    .expect("empty quote transitions must preserve the Markdown token");
                    let crate::SemanticDiagramModel::Flowchart(model) = parsed.model else {
                        panic!("expected Flowchart model");
                    };
                    assert_labels(&model);
                }
            }
        }
    }
}

#[test]
fn invalid_markdown_nodes_keep_original_label_spans_before_valid_nodes() {
    let parser = crate::SemanticParser::new();
    let raw = r#""`雪 `code` last`""#;
    for (diagram_type, header) in [
        #[cfg(feature = "diagram-flowchart")]
        ("flowchart-v2", "flowchart TD"),
        #[cfg(feature = "diagram-swimlane")]
        ("swimlane", "swimlane-beta TD"),
    ] {
        for (open, close) in [("[", "]"), ("(", ")"), ("{", "}")] {
            let body = format!("{header}\n雪[valid]\nA{open}{raw}{close}\nB[valid]\n");
            let mut tokens = Lexer::new(&body);
            let error = tokens
                .find_map(|token| token.err())
                .expect("invalid Markdown node");
            let start = body.find(raw).unwrap();
            assert_eq!(error.span, Some(SourceSpan::new(start, start + raw.len())));
            assert!(tokens.any(|token| matches!(token, Ok((_, Tok::Id(id), _)) if id == "B")));

            for line_ending in ["\n", "\r\n"] {
                let source = format!("---\ntitle: 中文😀\n---\n%% 中文😀\n{body}")
                    .replace('\n', line_ending);
                let start = source.find(raw).unwrap();
                for known_type in [false, true] {
                    let parsed = if known_type {
                        parser.parse_diagram_with_type_sync(diagram_type, &source)
                    } else {
                        parser.parse_diagram_sync(&source)
                    };
                    let Err(crate::Error::DiagramParse { diagnostic, .. }) = parsed else {
                        panic!("expected a Markdown label diagnostic: {source:?}");
                    };
                    assert_eq!(
                        diagnostic.span_kind(),
                        crate::ParseDiagnosticSpanKind::Exact
                    );
                    assert_eq!(
                        diagnostic.span(),
                        Some(SourceSpan::new(start, start + raw.len())),
                        "{source:?}",
                    );
                }
            }
        }
    }
}

#[test]
fn markdown_labels_reject_invalid_jison_tokens_in_nodes_edges_and_subgraphs() {
    let parser = crate::SemanticParser::new();
    for raw in [
        "\"`first\n  X[\"雪 &amp; text\"]\nlast`\"",
        r#""`first "quoted" last`""#,
        r#""`first \"quoted\" last`""#,
        r#""`first `code` last`""#,
        r#""``""#,
        r#""`unterminated""#,
        r#""`ok""`""#,
        "\"\"",
        concat!("\"first\"", "\"last\""),
        concat!("\"`first`\"", "\"`last`\""),
        concat!("\"first\"", "\"`last`\""),
        concat!("\"`first`\"", "\"last\""),
    ] {
        for statement in [
            format!("A[{raw}]"),
            format!("A({raw})"),
            format!("A{{{raw}}}"),
            format!("A -->|{raw}| B"),
            format!("A -- {raw} --> B"),
            format!("subgraph SG[{raw}]\nB\nend"),
            format!("subgraph {raw}\nB\nend"),
        ] {
            let source = format!("flowchart TD\n{statement}");
            assert!(
                parse_flowchart_model_for_render(&source, &metadata()).is_err(),
                "invalid Markdown token must be rejected: {source:?}",
            );
            for (diagram_type, header) in [
                #[cfg(feature = "diagram-flowchart")]
                ("flowchart-v2", "flowchart TD"),
                #[cfg(feature = "diagram-swimlane")]
                ("swimlane", "swimlane-beta TD"),
            ] {
                let source = format!("{header}\n{statement}");
                for known_type in [false, true] {
                    let parsed = if known_type {
                        parser.parse_diagram_with_type_sync(diagram_type, &source)
                    } else {
                        parser.parse_diagram_sync(&source)
                    };
                    assert!(parsed.is_err(), "invalid label tokens: {source:?}");
                }
            }
        }
    }
}

#[test]
fn markdown_comment_combinations_preserve_labels_in_all_shared_parser_paths() {
    for (payload, expected) in [
        ("first\n%% comment\n\n%% comment\nlast", "first\nlast"),
        ("first\n\n \t\n%% comment\nlast", "first\nlast"),
        ("first\n%% comment\n\nlast", "first\n\nlast"),
        ("first\n\nlast", "first\n\nlast"),
        ("first\n%% \"`invalid `code`\"\nlast", "first\nlast"),
        (
            "first &quot;雪&quot; &amp; text\n%% comment\n\n%% comment\nlast",
            "first \"雪\" & text\nlast",
        ),
    ] {
        let raw = format!("\"`{payload}`\"");
        let input = format!(
            "flowchart TD\nA[{raw}] -->|{raw}| B\nB -- {raw} --> C\nsubgraph SG[{raw}]\nD\nend\nsubgraph {raw}\nE\nend\n"
        );
        for line_ending in ["\n", "\r\n"] {
            let input = input.replace('\n', line_ending);
            let preprocessed = crate::preprocess::preprocess_builtin_semantic_controlled(
                &input,
                Some("flowchart-v2"),
                &OperationControl::new(),
            )
            .expect("active operation")
            .expect("source preprocessing");
            let model = parse(preprocessed.code());
            let node = model.nodes.iter().find(|node| node.id == "A").unwrap();
            assert_eq!(node.label.as_deref(), Some(expected), "{input:?}");
            assert_eq!(node.label_type.as_deref(), Some("markdown"));
            assert_eq!(model.edges.len(), 2);
            for edge in &model.edges {
                assert_eq!(edge.label.as_deref(), Some(expected), "{input:?}");
                assert_eq!(edge.label_type.as_deref(), Some("markdown"));
            }
            assert_eq!(model.subgraphs.len(), 2);
            for subgraph in &model.subgraphs {
                assert_eq!(subgraph.title, expected, "{input:?}");
                assert_eq!(subgraph.label_type.as_deref(), Some("markdown"));
            }
        }
    }
}

#[test]
fn attached_edge_ids_preserve_node_shapes_and_endpoints() {
    for node in [
        "A[bla]",
        "A(bla)",
        "A{bla}",
        "A((bla))",
        "A>bla]",
        "A([bla])",
        "A[[bla]]",
        "A[(bla)]",
        "A{{bla}}",
        "A[/bla/]",
        "A@{shape:circle, label: bla}",
        "A@{shape:circle, }",
        "A@{shape:circle,}",
        "A@{shape:circle}",
        "A@{shape: circle}",
    ] {
        for space in ["", " "] {
            let source = format!("flowchart TD\n{node}{space}e1@--> B");
            let model = parse(&source);
            assert_eq!(model.nodes.len(), 2, "{source}");
            assert_eq!(model.edges.len(), 1, "{source}");
            let edge = &model.edges[0];
            assert_eq!((&*edge.id, &*edge.from, &*edge.to), ("e1", "A", "B"));
            assert!(edge.is_user_defined_id, "{source}");
        }
    }
}

#[test]
fn chained_shape_data_and_trailing_spaces_keep_metadata() {
    for tail in ["   ", " \t\r\n", "   ;"] {
        for body in [
            "A@{shape: circle}",
            "B --> A@{shape: circle}",
            "A@{shape: circle}e1@--> B@{shape: circle}e2@--> C",
        ] {
            let model = parse(&format!("flowchart TD\n{body}{tail}"));
            let node = model.nodes.iter().find(|node| node.id == "A").unwrap();
            assert_eq!(node.shape.as_deref(), Some("circle"));
            if body.contains("e2@") {
                assert_eq!(model.edges[0].id, "e1");
                assert_eq!(model.edges[1].id, "e2");
                assert_eq!(model.nodes[1].shape.as_deref(), Some("circle"));
            }
        }
    }
}

#[test]
fn edge_ids_reject_node_brackets_and_label_delimiters() {
    for edge in ["e[1]", "e(1)", "e{1}", "e|1|"] {
        let source = format!("flowchart TD\nA {edge}@--> B");
        assert!(
            parse_flowchart_model_for_render(&source, &metadata()).is_err(),
            "{source}"
        );
    }
}

#[test]
fn edge_id_lexing_keeps_original_utf8_spans() {
    let source = "flowchart TD\nA[\"标签\"]边.1@--> B\n";
    let tokens = Lexer::new(source)
        .collect::<std::result::Result<Vec<_>, _>>()
        .expect("attached edge id must tokenize");
    let (start, _, end) = tokens
        .iter()
        .find(|(_, token, _)| matches!(token, Tok::EdgeId(id) if id == "边.1"))
        .expect("separate edge identifier token");
    assert_eq!(&source[*start..*end], "边.1@");
    assert!(tokens.iter().any(|(start, token, end)| {
        matches!(token, Tok::Id(id) if id == "A") && &source[*start..*end] == "A"
    }));
}

#[test]
fn step_metadata_accepts_positive_javascript_integers_without_a_safe_integer_cap() {
    for (value, expected) in [
        ("1", 1.0),
        ("1.0", 1.0),
        ("2", 2.0),
        ("9007199254740992", 9007199254740992.0),
        ("1e100", 1e100),
    ] {
        let model = parse(&format!(
            "flowchart LR\nA e1@--> B\ne1@{{ step: {value} }}\n"
        ));
        assert_eq!(model.edges[0].step, Some(expected), "{value}");
        assert_eq!(model.step_count(), expected, "{value}");
    }
}

#[test]
fn invalid_step_assignments_are_ignored_and_preserve_the_previous_valid_value() {
    for value in [
        "0", "-0", "-1", "1.5", "true", "null", "'2'", ".inf", ".nan", "[2]",
    ] {
        for previous in ["", "e1@{ step: 3 }\n"] {
            let source = format!("flowchart LR\nA e1@--> B\n{previous}e1@{{ step: {value} }}\n");
            let model = parse(&source);
            let expected = (!previous.is_empty()).then_some(3.0);
            assert_eq!(model.edges[0].step, expected, "{source}");
        }
    }
    let model = parse("flowchart LR\nA e1@--> B\ne1@{ step: 3 }\ne1@{ step: 1 }\n");
    assert_eq!(model.edges[0].step, Some(1.0));
}

#[test]
fn step_count_uses_the_maximum_with_repeated_steps_and_gaps() {
    let model = parse(concat!(
        "flowchart LR\nA e1@--> B\nB e2@--> C\nB e3@--> D\nC e4@--> E\n",
        "e1@{ step: 1 }\ne2@{ step: 4 }\ne3@{ step: 4 }\ne4@{ animate: true }\n",
    ));
    assert_eq!(model.step_count(), 4.0);
    assert_eq!(
        model.edges.iter().map(|edge| edge.step).collect::<Vec<_>>(),
        [Some(1.0), Some(4.0), Some(4.0), None]
    );
    let wire = serde_json::to_value(&model).unwrap();
    assert!(wire["edges"][3].get("step").is_none());
    for edge in wire["edges"].as_array().unwrap() {
        assert!(edge.get("stepCount").is_none());
        assert!(edge.get("stepDuration").is_none());
        assert!(edge.get("showStepNumber").is_none());
    }
}

#[test]
fn step_metadata_follows_declaration_order_and_subgraph_precedence() {
    let model = parse(concat!(
        "flowchart LR\ne1@{ step: 2 }\nA e1@--> B\n",
        "subgraph e1\nC\nend\ne1@{ step: 3 }\n",
    ));
    assert_eq!(model.edges[0].step, None);
    assert_eq!(
        model.subgraphs[0].metadata.as_ref().unwrap()["step"],
        json!(3)
    );
}

#[test]
fn author_metadata_drops_internal_keys_without_creating_empty_subgraph_metadata() {
    for value in ["laneRole: pool", "laneIndex: 1", "drawnExtent: [5, 5]"] {
        let model = parse(&format!(
            "flowchart TD\nsubgraph lane\nA\nend\nlane@{{ {value} }}\n"
        ));
        assert_eq!(model.subgraphs[0].metadata, None, "{value}");
    }
    let model = parse(concat!(
        "flowchart TD\nsubgraph lane\nA\nend\n",
        "lane@{ laneRole: pool, laneIndex: 1, drawnExtent: 5, algorithm: elk.box, custom: keep }\n",
        "lane@{ laneRole: lane, laneIndex: 2 }\n",
    ));
    assert_eq!(
        model.subgraphs[0].metadata,
        Some(json!({"algorithm": "elk.box", "custom": "keep"}))
    );
}

#[test]
fn comma_click_tooltips_keep_missing_targets_and_overwrite_each_target_independently() {
    for action in ["\"https://example.test\"", "call cb()"] {
        let source = format!(
            "flowchart TD\nA\nclick A,Later {action} \"first\"\nLater\nclick Later,Missing {action} \"second\"\n"
        );
        let parsed = parse_flowchart_semantic_source(&source, &metadata()).unwrap();
        assert_eq!(
            parsed.tooltips,
            HashMap::from([
                ("A".to_string(), "first".to_string()),
                ("Later".to_string(), "second".to_string()),
                ("Missing".to_string(), "second".to_string()),
            ]),
        );
        assert!(!parsed.nodes.iter().any(|node| node.id == "Missing"));
    }
}

#[test]
fn dropped_subgraph_members_keep_first_owner_and_source_order() {
    let model = parse(concat!(
        "flowchart LR\nsubgraph one\na\nb\nend\n",
        "subgraph two\nb\na\nc\nend\nsubgraph three\na\nend\n",
    ));
    assert_eq!(model.subgraphs[0].nodes, ["a", "b"]);
    assert_eq!(model.subgraphs[1].nodes, ["c"]);
    assert!(model.subgraphs[2].nodes.is_empty());
    assert_eq!(
        model.dropped_subgraph_members,
        [
            FlowDroppedSubgraphMember {
                node: "b".to_string(),
                kept_by: "one".to_string(),
                dropped_from: "two".to_string()
            },
            FlowDroppedSubgraphMember {
                node: "a".to_string(),
                kept_by: "one".to_string(),
                dropped_from: "two".to_string()
            },
            FlowDroppedSubgraphMember {
                node: "a".to_string(),
                kept_by: "one".to_string(),
                dropped_from: "three".to_string()
            },
        ]
    );
    let wire = serde_json::to_value(&model).unwrap();
    let restored: FlowchartModel = serde_json::from_value(wire).unwrap();
    assert_eq!(
        restored.dropped_subgraph_members,
        model.dropped_subgraph_members
    );
}

#[test]
fn nested_repeated_and_unrelated_subgraphs_do_not_create_false_dropped_members() {
    for source in [
        "flowchart LR\nsubgraph one\na\nend\nsubgraph two\nb\nend",
        "flowchart LR\nsubgraph outer\nsubgraph inner\na\nend\nend",
        "flowchart LR\nsubgraph L1\nsubgraph M\nsubgraph I\na\nend\nend\na\nend",
        "flowchart LR\nsubgraph S\nx\nend\nsubgraph S\nx\ny\nend",
        "flowchart LR\nsubgraph S\nS\nend",
    ] {
        assert!(
            parse(source).dropped_subgraph_members.is_empty(),
            "{source}"
        );
    }
    let source = "flowchart LR\nsubgraph one\na\nend\nsubgraph two\na\nend";
    assert_eq!(parse(source).dropped_subgraph_members.len(), 1);
    assert!(parse("flowchart LR\na").dropped_subgraph_members.is_empty());
}

#[cfg(all(feature = "editor-facts", feature = "diagram-flowchart"))]
#[test]
fn attached_edge_ids_preserve_editor_spans_references_and_rename_during_recovery() {
    use crate::{
        EditorSemanticCompleteness, ParsedEditorFacts, SemanticParser, SemanticSnapshotCapture,
    };

    let prefix = "\u{feff}---\r\ntitle: Original source\r\n---\r\n";
    let body = concat!(
        "flowchart TD\r\nA[\"标签\"]edge.1@--> B\r\n",
        "edge.1@{ step: 2 }\r\nA --> C\r\n",
    );
    for suffix in ["", "C -->"] {
        let source = format!("{prefix}{body}{suffix}");
        let SemanticSnapshotCapture::Snapshot(snapshot) =
            SemanticParser::new().capture_diagram_snapshot_sync(&source)
        else {
            panic!("family recovery must retain a source snapshot");
        };
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("Flowchart editor facts must be available");
        };
        assert_eq!(
            facts.completeness,
            if suffix.is_empty() {
                EditorSemanticCompleteness::Complete
            } else {
                EditorSemanticCompleteness::Recovered
            }
        );
        let occurrences = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == "A")
            .collect::<Vec<_>>();
        assert_eq!(occurrences.len(), 2);
        assert_eq!(occurrences[0].role, EditorSemanticRole::Entity);
        assert_eq!(occurrences[1].role, EditorSemanticRole::Reference);
        for symbol in occurrences {
            assert_eq!(&source[symbol.selection.start..symbol.selection.end], "A");
            assert_eq!(symbol.rename_policy, EditorRenamePolicy::FlowchartNodeId);
        }
        assert!(
            !facts
                .symbols
                .iter()
                .any(|symbol| symbol.name == "edge.1" && symbol.role.contributes_references())
        );
    }
}

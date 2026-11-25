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

#[test]
fn leading_dot_node_ids_parse_as_vertices() {
    let model = parse("flowchart TB\n    .a --> ...\n");
    let nodes = model
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(nodes, [".a", "..."]);
    let edges = model
        .edges
        .iter()
        .map(|edge| (edge.from.as_str(), edge.to.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(edges, [(".a", "...")]);
}

#[test]
fn ampersands_group_vertices_only_between_space_tokens() {
    for (statement, expected) in [
        (".a&B --> C", &[".a&B"][..]),
        ("A&end.n --> C", &["A&end.n"]),
        ("1&B --> C", &["1&B"]),
        ("  & --> C", &["&"]),
        ("A & & --> C", &["A", "&"]),
        (".a \t&\u{a0}B --> C", &[".a", "B"]),
        ("A[First] & B --> C", &["A", "B"]),
    ] {
        let model = parse(&format!("flowchart TB\n{statement}\n"));
        let from = model
            .edges
            .iter()
            .map(|edge| edge.from.as_str())
            .collect::<Vec<_>>();
        assert_eq!(from, expected, "{statement}");
        assert!(model.edges.iter().all(|edge| edge.to == "C"));
    }
    let chain = parse("flowchart TB\nA --> |label| & --> B\n");
    assert_eq!(chain.edges[0].to, "&");
    assert_eq!(chain.edges[1].from, "&");

    for statement in [
        ".a &B --> C",
        ".a& B --> C",
        "A[First]&B --> C",
        "A &\nB",
        "&end.n --> C",
        "1end.n --> C",
        "A,style.n --> C",
    ] {
        let source = format!("flowchart TB\n{statement}\n");
        assert!(
            parse_flowchart_model_for_render(&source, &metadata()).is_err(),
            "{statement}"
        );
    }
}

#[test]
fn dots_in_node_strings_and_at_link_boundaries_follow_flow_jison() {
    let single = parse("flowchart TB\n.a.-b\n");
    assert_eq!(single.nodes.len(), 1);
    assert_eq!(single.nodes[0].id, ".a.-b");
    assert!(single.edges.is_empty());

    for (statement, from, to, stroke, length) in [
        (".a.-->B", ".a.", "B", FlowEdgeStroke::Normal, 1),
        (
            "./src --> ../build",
            "./src",
            "../build",
            FlowEdgeStroke::Normal,
            1,
        ),
        ("B .-> C", "B", "C", FlowEdgeStroke::Dotted, 1),
        (".a-.->B", ".a", "B", FlowEdgeStroke::Dotted, 1),
        ("1..->B", "1", "B", FlowEdgeStroke::Dotted, 2),
        ("A甲.->B", "A甲", "B", FlowEdgeStroke::Dotted, 1),
        ("A,v.->B", "A,v", "B", FlowEdgeStroke::Dotted, 1),
        ("default.->B", "default", "B", FlowEdgeStroke::Dotted, 1),
    ] {
        let model = parse(&format!("flowchart TB\n{statement}\n"));
        let [edge] = model.edges.as_slice() else {
            panic!("expected one edge for {statement}");
        };
        assert_eq!(
            (edge.from.as_str(), edge.to.as_str()),
            (from, to),
            "{statement}"
        );
        assert_eq!(
            (edge.stroke_kind, edge.length),
            (stroke, length),
            "{statement}"
        );
    }

    for statement in ["B.->C", ".a.->B", "...-->B"] {
        let source = format!("flowchart TB\n{statement}\n");
        assert!(
            parse_flowchart_model_for_render(&source, &metadata()).is_err(),
            "{statement}"
        );
    }
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
        assert_eq!(
            parse_flowchart_model_for_render(&source, &metadata()).is_ok(),
            repetitions == 0,
            "unclosed Markdown must remain invalid",
        );
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn flowchart_editor_trace_propagates_label_scan_cancellation() {
    let source = format!("flowchart TD\nA[\"`{}`\"]\n", "雪".repeat(32_768));
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
fn single_node_special_character_ids_match_flow_singlenode() {
    // flow-singlenode.spec.js exercises these twelve characters in all three positions.
    let special_chars = ['#', ':', '0', '&', ',', '*', '.', '\\', 'v', '-', '/', '_'];
    let other_node_string_chars = ['!', '$', '\'', '+', '`', '?'];
    for ch in special_chars.into_iter().chain(other_node_string_chars) {
        for id in [ch.to_string(), format!("{ch}node"), format!("node{ch}")] {
            let source = format!("graph TD; {id} --> A");
            let model = parse_flowchart_model_for_render(&source, &metadata())
                .unwrap_or_else(|error| panic!("{source:?}: {error}"));
            let [from, to] = model.nodes.as_slice() else {
                panic!("expected two nodes in {source:?}");
            };
            let [edge] = model.edges.as_slice() else {
                panic!("expected one edge in {source:?}");
            };
            assert_eq!(from.id, id, "{source:?}");
            assert_eq!(from.label.as_deref(), Some(id.as_str()), "{source:?}");
            assert_eq!(to.id, "A", "{source:?}");
            assert_eq!((&edge.from, &edge.to), (&from.id, &to.id), "{source:?}");
        }
    }
}

#[test]
fn href_is_reserved_only_before_whitespace_at_a_token_boundary() {
    for (statement, id) in [
        (":href; A", ":href"),
        (":href[Label]", ":href"),
        (":href-->A", ":href"),
        ("A-->:href; B-->C", ":href"),
        ("href-->A", "href"),
        (":hrefSuffix", ":hrefSuffix"),
        ("foohref", "foohref"),
    ] {
        let model = parse(&format!("flowchart LR\n{statement}\n"));
        assert!(model.nodes.iter().any(|node| node.id == id), "{statement}");
    }

    for statement in [
        ":href",
        "A-->:href",
        "foo:href",
        "#href",
        "9href",
        "雪href",
        "href\t;A",
        ":href\u{a0};A",
        "style :href fill:red",
        "class :href hot",
        "classDef href fill:red",
        "class A :href",
    ] {
        for end in ["", "\n", "\r\n"] {
            let source = format!("flowchart LR\n{statement}{end}");
            let keyword = source.find("href").unwrap();
            let error = Lexer::new(&source).find_map(|token| token.err()).unwrap();
            assert_eq!(
                error.span,
                Some(SourceSpan::new(keyword, keyword + 4)),
                "{source:?}"
            );
            assert_eq!(
                error.message, "Unexpected flowchart keyword href",
                "{source:?}"
            );
            assert!(parse_flowchart_model_for_render(&source, &metadata()).is_err());
        }
    }

    let model = parse("flowchart LR\n:href[Link]\nclick :href href \"https://example.test\"\n");
    assert_eq!(
        model.nodes[0].link.as_deref(),
        Some("https://example.test/")
    );
}

#[test]
fn directive_identifiers_with_apostrophes_preserve_the_next_statement() {
    let model = parse(concat!(
        "flowchart TD\n",
        "A --> O'Brien\n",
        "style O'Brien fill:red\n",
        "classDef 'hot stroke:blue; class O'Brien,A 'hot\n",
        "click O'Brien \"https://example.test/a;b\"; C --> D\n",
    ));
    let [first, styled, _, _] = model.nodes.as_slice() else {
        panic!("all four authored nodes must survive directive scanning");
    };
    assert_eq!(styled.id, "O'Brien");
    assert_eq!(styled.styles, ["fill:red"]);
    assert_eq!(styled.classes, ["'hot", "clickable"]);
    assert_eq!(first.classes, ["'hot"]);
    assert_eq!(model.class_defs["'hot"], ["stroke:blue"]);
    assert_eq!(styled.link.as_deref(), Some("https://example.test/a;b"));
    let [_, next] = model.edges.as_slice() else {
        panic!("the next edge must survive directive scanning");
    };
    assert_eq!((next.from.as_str(), next.to.as_str()), ("C", "D"));
}

#[test]
fn directive_payload_distinguishes_apostrophes_from_quoted_delimiters() {
    let model = parse(concat!(
        "flowchart LR\n",
        "A --> B\n",
        "style B font-family:O'Brien,fill:'a;b'\n",
        "classDef warm font-family:雪'例\n",
        "linkStyle 0 font-family:O'Brien\n",
        "click B O'Brien\n",
        "C --> D\n",
    ));
    let [_, styled, _, _] = model.nodes.as_slice() else {
        panic!("bare apostrophes must not consume subsequent statements");
    };
    assert_eq!(styled.styles, ["font-family:O'Brien", "fill:'a;b'"]);
    assert_eq!(model.class_defs["warm"], ["font-family:雪'例"]);
    let [first, next] = model.edges.as_slice() else {
        panic!("both edges must survive directive scanning");
    };
    assert_eq!(first.style, ["font-family:O'Brien", "fill:none"]);
    assert_eq!((next.from.as_str(), next.to.as_str()), ("C", "D"));

    for separator in ['\n', ';'] {
        let model = parse(&format!(
            "flowchart LR\nA-->B\nstyle B font-family:'Foo{separator}C-->D\n"
        ));
        assert_eq!(model.nodes[1].styles, ["font-family:'Foo"]);
        assert_eq!(model.edges.len(), 2);
    }
}

#[test]
fn single_node_special_character_ids_preserve_token_boundaries() {
    let parse_statement = |statement: &str| {
        let source = format!("graph TD; {statement}");
        parse_flowchart_model_for_render(&source, &metadata())
            .unwrap_or_else(|error| panic!("{source:?}: {error}"))
    };

    let styled = parse_statement("#:*:::hot --> A");
    assert_eq!(styled.nodes[0].id, "#:*");
    assert_eq!(styled.nodes[0].classes, ["hot"]);

    for (statement, from, stroke, length) in [
        ("#.->A", "#", FlowEdgeStroke::Dotted, 1),
        ("*.->A", "*", FlowEdgeStroke::Dotted, 1),
        ("node:..->A", "node:", FlowEdgeStroke::Dotted, 2),
        ("A+B==>A", "A+B", FlowEdgeStroke::Thick, 1),
        ("?node====A", "?node", FlowEdgeStroke::Thick, 2),
    ] {
        let model = parse_statement(statement);
        let [edge] = model.edges.as_slice() else {
            panic!("expected one edge in {statement:?}");
        };
        assert_eq!(
            (edge.from.as_str(), edge.to.as_str()),
            (from, "A"),
            "{statement:?}"
        );
        assert_eq!(
            (edge.stroke_kind, edge.length),
            (stroke, length),
            "{statement:?}"
        );
    }

    for (statement, label) in [
        ("->odd]", "odd"),
        ("A\n->odd]", "odd"),
        (r#"A --> ->"a]b"]"#, "a]b"),
        (r#"->"hello[world]"]"#, "hello[world]"),
        ("->hello\nworld]", "hello\nworld"),
    ] {
        let model = parse_statement(statement);
        let odd = model.nodes.iter().find(|node| node.id == "-").unwrap();
        assert_eq!(odd.shape.as_deref(), Some("odd"), "{statement:?}");
        assert_eq!(odd.label.as_deref(), Some(label), "{statement:?}");
    }

    for statement in [
        "->",
        "->unfinished",
        r#"->"quoted ]""#,
        "->\nB[later]",
        "A ->",
        "A ->\nB[later]",
    ] {
        let source = format!("graph TD; {statement}");
        let start = source.find("->").unwrap();
        let span = SourceSpan::new(start, start + 2);
        let error = Lexer::new(&source)
            .find_map(|item| item.err())
            .unwrap_or_else(|| panic!("missing partial operator error in {source:?}"));
        assert_eq!(
            error.message, "incomplete flowchart edge operator",
            "{source:?}"
        );
        assert_eq!(error.span, Some(span), "{source:?}");
        #[cfg(feature = "editor-facts")]
        assert!(
            error
                .expected_syntax
                .contains(&FlowchartLexicalExpectation {
                    kind: FlowchartExpectationKind::Operator,
                    span,
                }),
            "{source:?}"
        );
    }

    for id in [
        "node#end.v",
        "A+end.foo",
        r#"A"B"#,
        r#"12A"q""#,
        r#"A,+"q""#,
        r#"A#*"`q`""#,
    ] {
        let model = parse_statement(&format!("{id} --> Z"));
        assert_eq!(model.nodes[0].id, id);
        assert_eq!(model.nodes[0].label.as_deref(), Some(id));
        assert_eq!(model.edges[0].from, id);
    }

    for statement in [
        "#end.v",
        "*end.v",
        "node#.->A",
        "node*.->A",
        "->hello[world]",
        "->]",
        "end+foo",
        "1end?tail",
        r#""q" --> Z"#,
        r#""`q`" --> Z"#,
        r#"12"q" --> Z"#,
        r#"A,"q" --> Z"#,
        r#"#"q" --> Z"#,
        r#"*"q" --> Z"#,
        r#"&"q" --> Z"#,
        r#"v"q" --> Z"#,
        r#"default"q" --> Z"#,
        r#"甲"q" --> Z"#,
        "=",
        "A=B",
        "=node==>Z",
        "A=== =foo",
        "node= ==>Z",
        "A---=foo",
    ] {
        let source = format!("graph TD; {statement}");
        assert!(
            parse_flowchart_model_for_render(&source, &metadata()).is_err(),
            "{source:?}"
        );
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
fn flowchart_accessibility_values_follow_subgraph_membership_order() {
    // flow.jison returns the trimmed statement value; FlowDB.addSubGraph claims it on close.
    for directive in ["accTitle: A", "accDescr: A", "accDescr { A }"] {
        let source = format!(
            "flowchart TD\nsubgraph G\n{directive}\nX\nend\nsubgraph Other\nA --> B\nend\n"
        );
        let model = parse(&source);
        assert_eq!(model.subgraphs[0].nodes, ["A", "X"], "{directive}");
        assert_eq!(model.subgraphs[1].nodes, ["B"], "{directive}");
        assert_eq!(model.vertex_calls, ["X", "A", "B"], "{directive}");
        let ids: Vec<_> = model.nodes.iter().map(|node| node.id.as_str()).collect();
        assert_eq!(ids, ["X", "A", "B"], "{directive}");
        assert_eq!(
            model.dropped_subgraph_members,
            [FlowDroppedSubgraphMember {
                node: "A".into(),
                kept_by: "G".into(),
                dropped_from: "Other".into(),
            }]
        );
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn flowchart_accessibility_filters_self_duplicates_and_empty_blocks() {
    let source = concat!(
        "flowchart TD\nsubgraph G\n",
        "accTitle: G\naccDescr: Disconnected node\naccTitle: Disconnected node\n",
        "accDescr { \u{feff}\n  first\n   second  }accDescr {}\n",
        "accDescr { \n\t} X\nend\naccTitle: Root only\n",
    );
    let parsed = crate::SemanticParser::new()
        .parse_diagram_sync(source)
        .unwrap();
    let crate::SemanticDiagramModel::Flowchart(model) = parsed.model else {
        panic!("expected a Flowchart");
    };
    assert_eq!(
        model.subgraphs[0].nodes,
        ["Disconnected node", "first\n   second", "X"]
    );
    assert_eq!(model.vertex_calls, ["X"]);
    assert_eq!(model.acc_title.as_deref(), Some("Root only"));
    assert_eq!(model.acc_descr.as_deref(), Some("first\nsecond"));
    assert!(model.dropped_subgraph_members.is_empty());
}

#[test]
fn flowchart_accessibility_colon_consumes_whitespace_before_its_value() {
    for keyword in ["accTitle", "accDescr"] {
        let source = format!("flowchart TD\nsubgraph G\n{keyword}: \n  X\nend\nA --> B\n");
        let model = parse(&source);
        assert_eq!(model.subgraphs[0].nodes, ["X"], "{keyword}");
        assert_eq!(model.vertex_calls, ["A", "B"], "{keyword}");
        let model = parse(&format!("flowchart TD\n{keyword}: previous\n{keyword}:"));
        let value = if keyword == "accTitle" {
            model.acc_title
        } else {
            model.acc_descr
        };
        assert_eq!(value.as_deref(), Some(""), "{keyword}");
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

#[test]
fn direction_without_a_direction_value_is_a_vertex_id() {
    // flow.jison only lexes `direction\s+(TB|BT|RL|LR|TD)` as a direction statement.
    for (body, expected_nodes, expected_edges) in [
        ("direction", &["direction"][..], &[][..]),
        ("direction ", &["direction"], &[]),
        (
            "direction --> B",
            &["direction", "B"],
            &[("direction", "B")],
        ),
        (
            "A --> direction",
            &["A", "direction"],
            &[("A", "direction")],
        ),
        ("directionX", &["directionX"], &[]),
    ] {
        let model = parse(&format!("flowchart TB\n{body}\n"));
        let nodes = model
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(nodes, expected_nodes, "{body:?}");
        let edges = model
            .edges
            .iter()
            .map(|edge| (edge.from.as_str(), edge.to.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(edges, expected_edges, "{body:?}");
    }

    let model = parse("graph TB\n  subgraph Data\n    direction\n    Redux\n  end\n");
    let subgraph = &model.subgraphs[0];
    assert_eq!(subgraph.dir, None);
    assert_eq!(subgraph.nodes, ["direction", "Redux"]);
}

#[test]
fn direction_values_follow_the_upstream_prefix_rule() {
    for (body, expected_dir) in [
        ("direction LR", "LR"),
        ("direction LRfoo", "LR"),
        ("direction\n    RL", "RL"),
        ("direction \t BT", "BT"),
        ("direction LRfoo direction TB", "TB"),
        ("direction TD direction LR", "LR"),
        ("A[direction LR]", "LR"),
        ("  class direction RL", "RL"),
    ] {
        let model = parse(&format!("graph TB\nsubgraph S\n{body}\nA\nend\n"));
        assert_eq!(
            model.subgraphs[0].dir.as_deref(),
            Some(expected_dir),
            "{body:?}"
        );
        assert_eq!(model.subgraphs[0].nodes, ["A"], "{body:?}");
    }

    assert!(parse_flowchart_model_for_render("graph TB\ndirection X\n", &metadata()).is_err());
    for prior_keyword in ["class direction RL", "default direction LR"] {
        assert!(
            parse_flowchart_model_for_render(&format!("graph TB\n{prior_keyword}\n"), &metadata())
                .is_err()
        );
    }
}

#[test]
fn attached_default_shape_data_preserves_direction_text_and_edge() {
    let model = parse("flowchart TB\ndefault@{label: \"direction LR\", shape: rect} --> B\n");
    assert_eq!(model.nodes.len(), 2);
    let node = model
        .nodes
        .iter()
        .find(|node| node.id == "default")
        .unwrap();
    assert_eq!(node.label.as_deref(), Some("direction LR"));
    assert_eq!(node.shape.as_deref(), Some("rect"));
    assert_eq!(model.edges.len(), 1);
    let edge = &model.edges[0];
    assert_eq!((edge.from.as_str(), edge.to.as_str()), ("default", "B"));

    let separated = "flowchart TB\ndefault @{label: \"direction LR\", shape: rect} --> B\n";
    assert!(parse_flowchart_model_for_render(separated, &metadata()).is_err());
}

#[test]
fn direction_rules_use_statement_separators_outside_labels() {
    for statement in [
        "A[\"direction LR; x\"]",
        "A[\"x; direction LR\"]",
        "A -->|x; y| B[direction LR]",
        "A@{ label: \"x; direction LR\", shape: rect }",
        "direction\nLR A[\"x;y\"]",
    ] {
        let model = parse(&format!("flowchart TB\nsubgraph S\n{statement};Z\nend\n"));
        assert_eq!(model.subgraphs[0].dir.as_deref(), Some("LR"), "{statement}");
        assert_eq!(model.subgraphs[0].nodes, ["Z"], "{statement}");
    }
    let model = parse("flowchart TD\nsubgraph S\nA[\"x;y\"];direction LR;B --> C\nend\n");
    assert_eq!(model.subgraphs[0].dir.as_deref(), Some("LR"));
    assert_eq!(model.nodes.len(), 3);
    assert_eq!((&*model.edges[0].from, &*model.edges[0].to), ("B", "C"));
}

#[test]
fn closing_a_subgraph_consumes_the_indentation_before_a_direction_match() {
    for directive in ["class direction LR", "style direction LR"] {
        let source =
            format!("flowchart TD\nsubgraph Outer\nsubgraph Inner\nB\nend\n  {directive}\nend\n");
        assert!(
            parse_flowchart_model_for_render(&source, &metadata()).is_err(),
            "{directive}"
        );
    }
    for gap in ["\n  ", " ", "\u{2028}", "\u{2029}"] {
        let model = parse(&format!(
            "flowchart TD\nsubgraph Outer\nsubgraph Inner\nB\nend{gap}direction LR\nend\n"
        ));
        assert_eq!(model.subgraphs.last().unwrap().dir.as_deref(), Some("LR"));
    }
}

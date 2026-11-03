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

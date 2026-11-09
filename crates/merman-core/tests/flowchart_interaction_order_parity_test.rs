#![cfg(feature = "diagram-flowchart")]

use merman_core::SemanticParser;
use merman_model::families::flowchart::FlowchartModel;
use merman_model::semantic::SemanticDiagramModel;

fn parse(source: &str) -> FlowchartModel {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("valid Flowchart source");
    let SemanticDiagramModel::Flowchart(model) = parsed.model else {
        panic!("expected the canonical Flowchart model");
    };
    model
}

#[test]
fn inline_classes_route_to_existing_edges_without_creating_vertices() {
    for newline in ["\n", "\r\n"] {
        for target in ["e:::hot", "e[Ignored]:::hot"] {
            let source = format!("flowchart TD\nA e@-->B\n{target}\n").replace('\n', newline);
            let model = parse(&source);
            assert!(
                model
                    .nodes
                    .iter()
                    .map(|node| node.id.as_str())
                    .eq(["A", "B"])
            );
            assert_eq!(model.edges[0].id, "e");
            assert_eq!(model.edges[0].classes, ["hot"], "{source:?}");
            assert!(model.edges[0].label.is_none());
        }
    }
}

#[test]
fn inline_classes_reach_completed_subgraphs_and_their_same_id_vertex() {
    let model = parse("flowchart TD\nsubgraph G\nA\nend\nG:::hot\n");
    assert!(
        model
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .eq(["A", "G"])
    );
    assert_eq!(model.nodes[1].classes, ["hot"]);
    assert_eq!(model.subgraphs[0].classes, ["hot"]);
    assert_eq!(model.subgraphs[0].nodes, ["A"]);
    let (classes, styles) = model.effective_subgraph_css(0, &model.subgraphs[0]);
    assert_eq!(classes, ["hot"]);
    assert!(styles.is_empty());
}

#[test]
fn inline_classes_interleave_with_clicks_classes_and_subgraph_metadata() {
    let model = parse(concat!(
        "flowchart TD\ne[Before]\nA e@-->B\nsubgraph e\nC\nend\n",
        "class e first\nclick e \"https://example.test\"\n",
        "e:::last\ne@{ curve: linear }\n",
    ));
    assert!(
        model
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .eq(["e", "A", "B", "C"])
    );
    assert_eq!(model.nodes[0].label.as_deref(), Some("Before"));
    assert_eq!(
        model.nodes[0].link.as_deref(),
        Some("https://example.test/")
    );
    assert_eq!(model.nodes[0].classes, ["first", "clickable", "last"]);
    assert_eq!(model.edges[0].classes, ["first", "clickable", "last"]);
    assert_eq!(model.subgraphs[0].classes, ["first", "clickable", "last"]);
    assert_eq!(model.edges[0].interpolate, None);
    assert_eq!(
        model.subgraphs[0].metadata.as_ref().unwrap()["curve"],
        "linear"
    );
}

#[test]
fn inline_classes_do_not_apply_to_edges_declared_later() {
    let model = parse(concat!(
        "flowchart TD\ne:::early\nA e@-->B\nclass e middle\n",
        "e:::late\nclick e \"https://example.test\"\n",
    ));
    assert_eq!(model.nodes[0].id, "e");
    assert_eq!(
        model.nodes[0].classes,
        ["early", "middle", "late", "clickable"]
    );
    assert_eq!(model.edges[0].classes, ["middle", "late", "clickable"]);
}

#[test]
fn inline_class_comma_targets_keep_the_literal_vertex_and_split_class_targets() {
    let model = parse("flowchart TD\nA\nB\nA,B:::hot\n");
    assert!(
        model
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .eq(["A", "B", "A,B"])
    );
    assert_eq!(model.nodes[0].classes, ["hot"]);
    assert_eq!(model.nodes[1].classes, ["hot"]);
    assert!(model.nodes[2].classes.is_empty());
}

#[test]
fn comma_click_targets_apply_links_and_preserve_per_target_statement_order() {
    for newline in ["\n", "\r\n"] {
        let source = concat!(
            "flowchart TD\nA\nB\nclass A,B before\n",
            "click A,B,Later \"https://example.test\" \"tip\" _blank\n",
            "Later\nclass A,B,Later after\n",
        )
        .replace('\n', newline);
        let model = parse(&source);
        assert!(
            model
                .nodes
                .iter()
                .map(|node| node.id.as_str())
                .eq(["A", "B", "Later"])
        );
        for node in &model.nodes[..2] {
            assert_eq!(node.link.as_deref(), Some("https://example.test/"));
            assert_eq!(node.link_target.as_deref(), Some("_blank"));
            assert_eq!(node.classes, ["before", "clickable", "after"]);
        }
        assert_eq!(model.nodes[2].link, None);
        assert_eq!(model.nodes[2].classes, ["after"]);
    }
}

#[test]
fn comma_click_targets_route_classes_to_edges_and_completed_subgraphs() {
    let model = parse(concat!(
        "flowchart TD\nA\nX e@-->Y\nsubgraph G\nB\nend\n",
        "click A,e,G,Later \"https://example.test\" \"tip\"\n",
        "Later\nclass A,e,G,Later after\n",
    ));
    assert!(
        model
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .eq(["A", "X", "Y", "B", "Later"])
    );
    assert_eq!(
        model.nodes[0].link.as_deref(),
        Some("https://example.test/")
    );
    assert_eq!(model.nodes[0].classes, ["clickable", "after"]);
    assert_eq!(model.edges[0].classes, ["clickable", "after"]);
    assert_eq!(model.subgraphs[0].classes, ["clickable", "after"]);
    assert_eq!(model.nodes[4].link, None);
    assert_eq!(model.nodes[4].classes, ["after"]);
}

#[test]
fn comma_click_callbacks_keep_security_gating_repetition_and_existing_targets() {
    for security_level in ["strict", "loose"] {
        for callback in ["cb", "call cb()", "call cb(\"a,b\")"] {
            let parser = SemanticParser::new().with_site_config(
                merman_model::config::MermaidConfig::from_value(serde_json::json!({
                    "securityLevel": security_level,
                })),
            );
            let source =
                format!("flowchart TD\nA\nB\nclick A,B,A,Later {callback} \"tip\"\nLater\n");
            let parsed = parser.parse_diagram_sync(&source).unwrap();
            let SemanticDiagramModel::Flowchart(model) = parsed.model else {
                panic!("expected the canonical Flowchart model");
            };
            assert!(
                model
                    .nodes
                    .iter()
                    .map(|node| node.id.as_str())
                    .eq(["A", "B", "Later"])
            );
            assert_eq!(model.nodes[0].classes, ["clickable", "clickable"]);
            assert_eq!(model.nodes[1].classes, ["clickable"]);
            assert!(model.nodes[2].classes.is_empty());
            assert_eq!(model.nodes[0].have_callback, security_level == "loose");
            assert_eq!(model.nodes[1].have_callback, security_level == "loose");
            assert!(!model.nodes[2].have_callback);
        }
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn comma_click_targets_keep_exact_utf8_ranges_in_success_and_recovery() {
    use merman_core::{
        EditorRenamePolicy, EditorSemanticCompleteness, EditorSemanticRole, ParsedEditorFacts,
        SemanticSnapshotCapture,
    };
    use merman_model::source::SourceSpan;

    for newline in ["\n", "\r\n"] {
        for (action, recovered) in [("href \"https://example.test\"", false), ("href", true)] {
            let source = format!(
                "\u{feff}---\ntitle: 原稿😀\n---\nflowchart TD\n甲\n乙\nclick 甲,乙,未来 {action}\n未来\n"
            )
            .replace('\n', newline);
            let parser = SemanticParser::new();
            assert_eq!(parser.parse_diagram_sync(&source).is_err(), recovered);
            let SemanticSnapshotCapture::Snapshot(snapshot) =
                parser.capture_diagram_snapshot_sync(&source)
            else {
                panic!("Flowchart source must retain its snapshot");
            };
            assert_eq!(snapshot.outcome().parsed_model().is_none(), recovered);
            let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("Flowchart source must retain editor facts");
            };
            assert_eq!(
                facts.completeness,
                if recovered {
                    EditorSemanticCompleteness::Recovered
                } else {
                    EditorSemanticCompleteness::Complete
                },
            );
            assert_eq!(facts.diagnostics.is_empty(), !recovered);
            let mut cursor = source.find("click ").unwrap() + "click ".len();
            for id in ["甲", "乙", "未来"] {
                let span = SourceSpan::new(cursor, cursor + id.len());
                let symbol = facts
                    .symbols
                    .iter()
                    .find(|symbol| {
                        symbol.name == id
                            && symbol.detail.as_deref() == Some("flowchart interaction target")
                            && symbol.span == span
                    })
                    .expect("each click target retains its own authored range");
                assert_eq!(symbol.selection, span);
                assert_eq!(symbol.role, EditorSemanticRole::Reference);
                assert_eq!(symbol.rename_policy, EditorRenamePolicy::FlowchartNodeId);
                assert_eq!(&source[span.start..span.end], id);
                cursor += id.len() + 1;
            }
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == "未来" && symbol.role == EditorSemanticRole::Entity
            }));
            let SemanticSnapshotCapture::Snapshot(recovery) = parser
                .capture_diagram_recovery_snapshot_controlled_sync(
                    &source,
                    &merman_model::operation::OperationControl::new(),
                )
                .unwrap()
            else {
                panic!("Flowchart recovery must retain its snapshot");
            };
            let ParsedEditorFacts::Available(recovery_facts) = recovery.editor_facts() else {
                panic!("Flowchart recovery must retain diagnostics");
            };
            assert_eq!(recovery_facts.completeness, facts.completeness);
            assert_eq!(recovery_facts.diagnostics, facts.diagnostics);
            assert!(recovery_facts.symbols.is_empty());
        }
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn existing_edge_targets_do_not_become_phantom_node_definitions() {
    use merman_core::{ParsedEditorFacts, SemanticSnapshotCapture};
    for body in ["A e@-->B\ne:::hot\nC\n", "A e@-->B -->e:::hot\nC\n"] {
        let source = format!("flowchart TD\n{body}");
        let SemanticSnapshotCapture::Snapshot(snapshot) =
            SemanticParser::new().capture_diagram_snapshot_sync(&source)
        else {
            panic!("Flowchart snapshot");
        };
        assert!(snapshot.outcome().parsed_model().is_some());
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("Flowchart editor facts");
        };
        assert!(
            !facts
                .symbols
                .iter()
                .any(|symbol| symbol.name == "e" && symbol.role.contributes_outline())
        );
        for id in ["A", "B", "C"] {
            assert!(facts.symbols.iter().any(|symbol| symbol.name == id));
        }
    }
}

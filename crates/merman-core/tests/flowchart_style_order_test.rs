#![cfg(feature = "diagram-flowchart")]

use merman_core::SemanticParser;
use merman_model::families::flowchart::FlowchartModel;
use merman_model::semantic::SemanticDiagramModel;
use merman_model::warnings::FLOWCHART_UNKNOWN_STYLE_TARGET_WARNING_RULE_ID;

fn parse(source: &str) -> FlowchartModel {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("valid Flowchart source");
    let SemanticDiagramModel::Flowchart(model) = parsed.model else {
        panic!("Flowchart source must retain its canonical model");
    };
    model
}

#[test]
fn style_created_vertices_reserve_identity_in_statement_order() {
    for newline in ["\n", "\r\n"] {
        for (body, expected) in [
            ("style Ghost fill:red\nA-->B\n", vec!["Ghost", "A", "B"]),
            ("style Ghost fill:red\nA-->Ghost\n", vec!["Ghost", "A"]),
            (
                "A-->B\nstyle Later fill:red\nC-->D\n",
                vec!["A", "B", "Later", "C", "D"],
            ),
            (
                "subgraph G\nstyle Ghost fill:red\nA-->B\nend\n",
                vec!["Ghost", "A", "B"],
            ),
        ] {
            let source = format!("flowchart TD\n{body}").replace('\n', newline);
            let model = parse(&source);
            assert!(
                model.nodes.iter().map(|node| node.id.as_str()).eq(expected),
                "{source}"
            );
            let styled = model
                .nodes
                .iter()
                .find(|node| !node.styles.is_empty())
                .unwrap();
            assert_eq!(styled.styles, ["fill:red"]);
            assert_eq!(model.warning_facts.len(), 1);
            assert_eq!(
                model.warning_facts[0].rule_id,
                FLOWCHART_UNKNOWN_STYLE_TARGET_WARNING_RULE_ID
            );
        }
    }
}

#[test]
fn later_definitions_keep_style_created_vertex_position_and_interactions() {
    let model = parse(concat!(
        "flowchart TD\nstyle Ghost fill:red\n",
        "class Ghost accent\nclick Ghost \"https://example.test\"\n",
        "A-->Ghost[Later]\nstyle Ghost stroke:blue\nB\n",
    ));
    assert!(
        model
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .eq(["Ghost", "A", "B"])
    );
    let ghost = &model.nodes[0];
    assert_eq!(ghost.label.as_deref(), Some("Later"));
    assert_eq!(ghost.styles, ["fill:red", "stroke:blue"]);
    assert_eq!(ghost.classes, ["accent", "clickable"]);
    assert_eq!(ghost.link.as_deref(), Some("https://example.test/"));
    assert_eq!(model.warning_facts.len(), 1);
    assert_eq!(model.edges[0].from, "A");
    assert_eq!(model.edges[0].to, "Ghost");
}

#[test]
fn styles_keep_same_id_vertex_separate_from_subgraph_css_and_membership() {
    for body in [
        "style G fill:red\nsubgraph G\nA\nend\nB\n",
        "subgraph G\nA\nend\nstyle G fill:red\nB\n",
    ] {
        let model = parse(&format!("flowchart TD\n{body}class G accent\n"));
        let group = &model.subgraphs[0];
        assert_eq!(group.nodes, ["A"]);
        assert!(group.styles.is_empty());
        assert_eq!(group.classes, ["accent"]);
        let vertex = model.nodes.iter().find(|node| node.id == "G").unwrap();
        assert_eq!(vertex.styles, ["fill:red"]);
        assert_eq!(vertex.classes, ["accent"]);
        let (classes, styles) = model.effective_subgraph_css(0, group);
        assert_eq!(classes, ["accent"]);
        assert_eq!(styles, ["fill:red"]);
        assert_eq!(model.warning_facts.len(), 1);
    }
}

#[test]
fn style_after_an_existing_raw_edge_id_does_not_create_a_vertex() {
    let model = parse("flowchart TD\nA edge@-->B\nstyle edge fill:red\nC\n");
    assert!(
        model
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .eq(["A", "B", "C"])
    );
    assert!(model.warning_facts.is_empty());
    assert!(model.edges[0].style.is_empty());

    let model = parse("flowchart TD\nstyle edge fill:red\nA edge@-->B\nC\n");
    assert!(
        model
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .eq(["edge", "A", "B", "C"])
    );
    assert_eq!(model.nodes[0].styles, ["fill:red"]);
    assert_eq!(model.warning_facts.len(), 1);
    assert!(model.edges[0].style.is_empty());
}

#[test]
fn link_style_fill_fallback_uses_each_authored_fragment_prefix() {
    for (styles, expected) in [
        ("stroke:red,fill:blue", vec!["stroke:red", "fill:blue"]),
        (
            "stroke:red, fill:blue",
            vec!["stroke:red", " fill:blue", "fill:none"],
        ),
        (
            "stroke:red,   fill:blue",
            vec!["stroke:red", "   fill:blue", "fill:none"],
        ),
        (
            "fill-opacity:0.5,stroke:red",
            vec!["fill-opacity:0.5", "stroke:red"],
        ),
        ("stroke:red", vec!["stroke:red", "fill:none"]),
    ] {
        let model = parse(&format!("flowchart TD\nA-->B\nlinkStyle 0 {styles}\n"));
        assert_eq!(model.edges[0].style, expected, "{styles}");
    }
    let model = parse("flowchart TD\nA-->B\nlinkStyle default stroke:red, fill:blue\n");
    assert_eq!(
        model.edge_defaults.unwrap().style,
        ["stroke:red", " fill:blue"]
    );
}

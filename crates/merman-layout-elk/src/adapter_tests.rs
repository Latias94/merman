//! Public elkjs 0.12.0 observations collected by `elkjs_0_12_0_oracle.cjs`.

use super::*;

fn node(id: &str, parent: Option<&str>, kind: NodeKind) -> Node {
    Node {
        id: id.into(),
        kind,
        label_text: None,
        container: Default::default(),
        width: 40.0,
        height: 20.0,
        parent: parent.map(str::to_owned),
        direction: None,
        hierarchy_handling: None,
        layer_constraint: None,
        port_alignment: None,
        label: None,
    }
}

fn nested_direction_graph() -> Graph {
    let mut outer = node("outer", None, NodeKind::Group);
    outer.direction = Some(Direction::Up);
    outer.hierarchy_handling = Some(HierarchyHandling::SeparateChildren);
    Graph {
        id: "root".into(),
        direction: Direction::Down,
        nodes: vec![
            outer,
            node("inner", Some("outer"), NodeKind::Group),
            node("a", Some("inner"), NodeKind::Leaf),
            node("b", Some("inner"), NodeKind::Leaf),
        ],
        edges: vec![Edge {
            id: "a-b".into(),
            source: "a".into(),
            target: "b".into(),
            label: None,
            terminal_labels: Vec::new(),
            minlen: 1,
            inside_self_loops_yo: false,
        }],
        ..Default::default()
    }
}

#[test]
fn separate_layered_scope_resolves_its_own_direction() {
    for (direction, expected) in [
        (None, Direction::Right),
        (Some(Direction::Left), Direction::Left),
    ] {
        let mut graph = nested_direction_graph();
        graph.nodes[1].direction = direction;
        let index = HierarchyIndex::build(&graph, &mut NoopWorkControl).unwrap();
        let outer = index.child_scope_by_anchor[0].unwrap();
        let inner = index.child_scope_by_anchor[1].unwrap();
        assert_eq!(index.scopes[0].direction, Direction::Down);
        assert_eq!(index.scopes[outer].direction, Direction::Up);
        assert_eq!(index.scopes[inner].direction, expected);
        let output = layout(&graph).unwrap();
        let source = output.nodes.iter().find(|node| node.id == "a").unwrap();
        let target = output.nodes.iter().find(|node| node.id == "b").unwrap();
        assert_eq!(source.x < target.x, expected == Direction::Right);
        assert!((source.y - target.y).abs() < 1e-7);
    }
}

#[test]
fn included_layered_hierarchy_keeps_compound_direction_inheritance() {
    let mut graph = nested_direction_graph();
    graph.nodes[0].hierarchy_handling = Some(HierarchyHandling::IncludeChildren);
    let index = HierarchyIndex::build(&graph, &mut NoopWorkControl).unwrap();
    assert_eq!(index.scopes.len(), 1);
    let output = layout(&graph).unwrap();
    let source = output.nodes.iter().find(|node| node.id == "a").unwrap();
    let target = output.nodes.iter().find(|node| node.id == "b").unwrap();
    assert!(source.y < target.y);
    assert!((source.x - target.x).abs() < 1e-7);
}

#[test]
fn spore_keeps_separately_laid_out_radial_title_margin() {
    use source_port::algorithms::{box_layout, spore_overlap};

    let mut group = node("g", None, NodeKind::Group);
    group.container = ContainerNodeOptions {
        algorithm: Some(Algorithm::Radial),
        padding: 8.0,
    };
    group.label = Some(Label {
        width: 300.0,
        height: 10.0,
    });
    let mut sibling = node("sibling", None, NodeKind::Leaf);
    sibling.width = 400.0;
    sibling.height = 30.0;
    let graph = Graph {
        id: "root".into(),
        nodes: vec![group, node("child", Some("g"), NodeKind::Leaf), sibling],
        options: LayoutOptions {
            algorithm: Algorithm::SporeOverlap,
            ..Default::default()
        },
        ..Default::default()
    };
    let source_derived_size = source_port::LSize {
        width: 70.0,
        height: 60.0,
    };
    let source_derived_margin = source_port::LMargin {
        right: 230.0,
        ..Default::default()
    };
    assert_eq!(
        container::scope_title_margin(graph.nodes[0].label, source_derived_size),
        source_derived_margin,
        "elkjs 0.12.0 SPOrE importer retains the title at its imported origin"
    );
    let expected = spore_overlap::layout(
        &[
            spore_overlap::Node {
                width: source_derived_size.width,
                height: source_derived_size.height,
                margin: box_layout::Padding {
                    top: source_derived_margin.top,
                    right: source_derived_margin.right,
                    bottom: source_derived_margin.bottom,
                    left: source_derived_margin.left,
                },
                ..Default::default()
            },
            spore_overlap::Node {
                width: 400.0,
                height: 30.0,
                ..Default::default()
            },
        ],
        &[],
        &spore_overlap::Options::default(),
        &mut || panic!("distinct source-derived centers must not need jitter"),
        &mut NoopWorkControl,
    )
    .unwrap();
    let output = layout(&graph).unwrap();
    let group = output.nodes.iter().find(|node| node.id == "g").unwrap();
    assert_eq!(
        (group.width, group.height),
        (source_derived_size.width, source_derived_size.height),
        "elkjs 0.12.0 Radial scope dimensions"
    );
    for (id, position) in ["g", "sibling"].into_iter().zip(expected.nodes) {
        let node = output.nodes.iter().find(|node| node.id == id).unwrap();
        assert!((node.x - node.width / 2.0 - position.x).abs() < 1e-7);
        assert!((node.y - node.height / 2.0 - position.y).abs() < 1e-7);
    }
    for (id, x, y) in [("g", 8.0, 46.0), ("sibling", 426.0, 8.0)] {
        let node = output.nodes.iter().find(|node| node.id == id).unwrap();
        assert!((node.x - node.width / 2.0 - x).abs() < 1e-7);
        assert!((node.y - node.height / 2.0 - y).abs() < 1e-7);
    }
    let child = output.nodes.iter().find(|node| node.id == "child").unwrap();
    assert!((child.x - child.width / 2.0 - (group.x - group.width / 2.0) - 15.0).abs() < 1e-7);
    assert!((child.y - child.height / 2.0 - (group.y - group.height / 2.0) - 25.0).abs() < 1e-7);
}

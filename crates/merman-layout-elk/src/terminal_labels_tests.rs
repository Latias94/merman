use super::*;

fn node(id: &str, parent: Option<&str>) -> Node {
    Node {
        id: id.into(),
        kind: NodeKind::Leaf,
        label_text: None,
        container: Default::default(),
        width: 90.0,
        height: 80.0,
        parent: parent.map(str::to_owned),
        direction: None,
        hierarchy_handling: None,
        layer_constraint: None,
        port_alignment: None,
        label: None,
    }
}

fn edge(id: &str, source: &str, target: &str) -> Edge {
    Edge {
        id: id.into(),
        source: source.into(),
        target: target.into(),
        label: Some(Label {
            width: 36.0,
            height: 18.0,
        }),
        terminal_labels: TerminalLabelKey::ALL
            .into_iter()
            .map(|key| TerminalLabel {
                key,
                label: Label {
                    width: 18.0,
                    height: 16.5,
                },
            })
            .collect(),
        minlen: 1,
        inside_self_loops_yo: false,
    }
}

#[test]
fn terminal_label_identity_survives_flat_compound_and_separate_children_layout() {
    for handling in [
        None,
        Some(HierarchyHandling::IncludeChildren),
        Some(HierarchyHandling::SeparateChildren),
    ] {
        let mut nodes = vec![
            node("A", handling.map(|_| "left")),
            node("B", handling.map(|_| "right")),
        ];
        if let Some(handling) = handling {
            for id in ["left", "right"] {
                let mut group = node(id, None);
                group.kind = NodeKind::Group;
                group.hierarchy_handling = Some(handling);
                group.label = Some(Label {
                    width: 42.0,
                    height: 20.0,
                });
                nodes.insert(0, group);
            }
        }
        let graph = Graph {
            nodes,
            edges: vec![edge("A-B", "A", "B")],
            direction: Direction::Right,
            ..Default::default()
        };
        let result = layout(&graph).unwrap();
        let labels = &result.edges[0].labels;
        assert_eq!(labels.len(), 5, "{handling:?}: {labels:?}");
        assert_eq!(
            labels
                .iter()
                .filter(|label| label.terminal.is_none())
                .count(),
            1
        );
        for key in TerminalLabelKey::ALL {
            let terminal = labels
                .iter()
                .find(|label| label.terminal == Some(key))
                .unwrap();
            assert_eq!((terminal.width, terminal.height), (18.0, 16.5));
            assert!(terminal.x.is_finite() && terminal.y.is_finite());
            assert!(
                terminal.x != 0.0 || terminal.y != 0.0,
                "placed terminal: {terminal:?}"
            );
            let endpoint = result
                .nodes
                .iter()
                .find(|node| node.id == if key.at_start() { "A" } else { "B" })
                .unwrap();
            let other = result
                .nodes
                .iter()
                .find(|node| node.id == if key.at_start() { "B" } else { "A" })
                .unwrap();
            let center = (
                terminal.x + terminal.width / 2.0,
                terminal.y + terminal.height / 2.0,
            );
            assert!(
                (center.0 - endpoint.x).hypot(center.1 - endpoint.y)
                    < (center.0 - other.x).hypot(center.1 - other.y),
                "{handling:?} {key:?}: {terminal:?}"
            );
        }
    }
}

#[test]
fn terminal_labels_stay_unplaced_in_non_layered_providers() {
    for algorithm in [Algorithm::MrTree, Algorithm::Force, Algorithm::Box] {
        let graph = Graph {
            nodes: vec![node("A", None), node("B", None)],
            edges: vec![edge("A-B", "A", "B")],
            options: LayoutOptions {
                algorithm,
                ..Default::default()
            },
            ..Default::default()
        };
        let result = layout(&graph).unwrap();
        assert!(
            result
                .edges
                .iter()
                .flat_map(|edge| &edge.labels)
                .all(|label| label.terminal.is_none())
        );
    }
}

#[test]
fn terminal_label_identity_survives_kernel_cycle_reversal() {
    let graph = Graph {
        nodes: vec![node("A", None), node("B", None), node("C", None)],
        edges: vec![
            edge("A-B", "A", "B"),
            edge("B-C", "B", "C"),
            edge("C-A", "C", "A"),
        ],
        ..Default::default()
    };
    let result = layout(&graph).unwrap();
    for edge in result.edges {
        for key in TerminalLabelKey::ALL {
            assert_eq!(
                edge.labels
                    .iter()
                    .filter(|label| label.terminal == Some(key))
                    .count(),
                1,
                "{} keeps {key:?} exactly once",
                edge.id
            );
        }
    }
}

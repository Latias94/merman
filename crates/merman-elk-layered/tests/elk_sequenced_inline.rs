use merman_elk_layered::*;

fn input(loop_count: usize, options: LayeredOptions) -> ElkInputGraph {
    ElkInputGraph {
        id: "root".into(),
        nodes: vec![ElkInputNode {
            id: "n0".into(),
            width: 80.0,
            height: 40.0,
            parent: None,
            direction: None,
            hierarchy_handling: None,
            layer_constraint: None,
            port_constraints: None,
            port_alignment: None,
            node_label_placement: NodeLabelPlacement::Fixed,
            node_flexibility: NodeFlexibility::None,
            ports_surrounding: None,
            nested_options: None,
            label: None,
        }],
        edges: (0..loop_count)
            .map(|index| ElkInputEdge {
                id: format!("e{index}"),
                source: "n0".into(),
                target: "n0".into(),
                labels: vec![ElkInputLabel::center("loop", 60.0, 18.0)],
                minlen: 1,
                inside_self_loops_yo: false,
                model_order: None,
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
            })
            .collect(),
        options,
    }
}

fn route(input: &ElkInputGraph) -> LGraph {
    let mut graph = import_graph(input).unwrap();
    let executed =
        execute_processors_until_processor(&mut graph, ProcessorKind::SelfLoopRouter).unwrap();
    assert_eq!(executed.last(), Some(&ProcessorKind::SelfLoopRouter));
    assert!(!executed.contains(&ProcessorKind::NorthSouthPortPreprocessor));
    assert_eq!(graph.self_loop_holders.len(), 1);
    graph
}

fn assert_labels(
    graph: &LGraph,
    side: PortSide,
    expected_alignments: &[SelfLoopLabelAlignment],
    inline: bool,
) {
    let holder = &graph.self_loop_holders[0];
    let node = &graph.layerless_nodes[holder.node];
    let mut loops = holder
        .hyper_loops
        .iter()
        .filter(|hyper_loop| hyper_loop.occupied_sides == [side] && hyper_loop.labels.is_some())
        .collect::<Vec<_>>();
    loops.sort_by_key(|hyper_loop| hyper_loop.leftmost_port.unwrap());
    assert_eq!(loops.len(), expected_alignments.len());

    for (hyper_loop, &alignment) in loops.into_iter().zip(expected_alignments) {
        assert_eq!(hyper_loop.self_loop_type, Some(SelfLoopType::OneSide));
        let labels = hyper_loop.labels.as_ref().unwrap();
        assert_eq!(labels.side, Some(side));
        assert_eq!(labels.alignment, Some(alignment));
        let reference_port = match (side, alignment) {
            (_, SelfLoopLabelAlignment::Center) => None,
            (PortSide::North, SelfLoopLabelAlignment::Right)
            | (PortSide::South, SelfLoopLabelAlignment::Left) => hyper_loop.rightmost_port,
            (PortSide::North, SelfLoopLabelAlignment::Left)
            | (PortSide::South, SelfLoopLabelAlignment::Right) => hyper_loop.leftmost_port,
            _ => panic!("unexpected label alignment: {side:?}, {alignment:?}"),
        };
        assert_eq!(labels.alignment_reference_port, reference_port);
        let expected_x = match alignment {
            SelfLoopLabelAlignment::Center => (node.size.width - labels.size.width) / 2.0,
            SelfLoopLabelAlignment::Left | SelfLoopLabelAlignment::Right => {
                let port = &node.ports[reference_port.unwrap()];
                port.position.x + port.anchor.x
                    - if alignment == SelfLoopLabelAlignment::Right {
                        labels.size.width
                    } else {
                        0.0
                    }
            }
            SelfLoopLabelAlignment::Top => unreachable!(),
        };
        assert!((labels.position.x - expected_x).abs() < 1e-8);

        for label_ref in &labels.label_refs {
            let label = &graph.edges[label_ref.edge].labels[label_ref.label];
            assert_eq!(label.inline, inline);
            let expected_label_x = expected_x
                + match alignment {
                    SelfLoopLabelAlignment::Left => 0.0,
                    SelfLoopLabelAlignment::Right => labels.size.width - label.size.width,
                    SelfLoopLabelAlignment::Center => (labels.size.width - label.size.width) / 2.0,
                    SelfLoopLabelAlignment::Top => unreachable!(),
                };
            assert!((label.position.x - expected_label_x).abs() < 1e-8);
        }
    }
}

#[test]
fn sequenced_single_loop_preserves_inline_label() {
    let graph = route(&input(
        1,
        LayeredOptions {
            self_loop_ordering: SelfLoopOrderingStrategy::Sequenced,
            ..Default::default()
        },
    ));

    assert_labels(
        &graph,
        PortSide::North,
        &[SelfLoopLabelAlignment::Center],
        true,
    );
}

#[test]
fn sequenced_loop_labels_match_worker_coordinates_by_edge_identity() {
    for (distribution, extent, expected) in [
        (
            SelfLoopDistributionStrategy::North,
            (178.28571428571428, 122.0),
            [
                (12.0, 42.0),
                (59.14285714285714, 12.0),
                (106.28571428571428, 42.0),
            ],
        ),
        (
            SelfLoopDistributionStrategy::NorthSouth,
            (160.0, 120.0),
            [(12.0, 12.0), (50.0, 90.0), (88.0, 12.0)],
        ),
    ] {
        let mut graph = import_graph(&input(
            3,
            LayeredOptions {
                self_loop_ordering: SelfLoopOrderingStrategy::Sequenced,
                self_loop_distribution: distribution,
                ..Default::default()
            },
        ))
        .unwrap();
        execute_ported_processors(&mut graph).unwrap();
        let size = graph.exported_root_size();
        assert!((size.width - extent.0).abs() < 1e-8);
        assert!((size.height - extent.1).abs() < 1e-8);
        for (index, (x, y)) in expected.into_iter().enumerate() {
            let edge = graph
                .edges
                .iter()
                .find(|edge| edge.id == format!("e{index}"))
                .unwrap();
            assert_eq!(edge.labels.len(), 1);
            let label = &edge.labels[0];
            let actual_x = label.position.x + graph.offset.x + graph.padding.left;
            let actual_y = label.position.y + graph.offset.y + graph.padding.top;
            assert!(
                (actual_x - x).abs() < 1e-8,
                "{distribution:?} e{index}: {actual_x} != {x}"
            );
            assert!(
                (actual_y - y).abs() < 1e-8,
                "{distribution:?} e{index}: {actual_y} != {y}"
            );
        }
    }
}

#[test]
fn sequenced_labels_align_outward_in_pairs_on_north_and_south() {
    use SelfLoopLabelAlignment::{Center, Left, Right};

    let cases: &[(&[SelfLoopLabelAlignment], &[SelfLoopLabelAlignment])] = &[
        (&[Center], &[Center]),
        (&[Right, Left], &[Left, Right]),
        (&[Right, Center, Left], &[Left, Center, Right]),
        (&[Right, Right, Left, Left], &[Left, Left, Right, Right]),
        (
            &[Right, Right, Center, Left, Left],
            &[Left, Left, Center, Right, Right],
        ),
    ];
    for &(north, south) in cases {
        for multiple_labels in [false, true] {
            let mut input = input(
                north.len() + south.len(),
                LayeredOptions {
                    self_loop_ordering: SelfLoopOrderingStrategy::Sequenced,
                    self_loop_distribution: SelfLoopDistributionStrategy::NorthSouth,
                    ..Default::default()
                },
            );
            if multiple_labels {
                for edge in &mut input.edges {
                    edge.labels
                        .push(ElkInputLabel::center("detail", 40.0, 12.0));
                }
            }
            let graph = route(&input);

            assert_labels(&graph, PortSide::North, north, true);
            assert_labels(&graph, PortSide::South, south, true);
        }
    }
}

#[test]
fn sequenced_label_groups_skip_unlabeled_loops() {
    use SelfLoopLabelAlignment::{Left, Right};

    let mut input = input(
        6,
        LayeredOptions {
            self_loop_ordering: SelfLoopOrderingStrategy::Sequenced,
            self_loop_distribution: SelfLoopDistributionStrategy::NorthSouth,
            ..Default::default()
        },
    );
    input.edges[0].labels.clear();
    input.edges[5].labels.clear();
    let graph = route(&input);

    assert_eq!(graph.self_loop_holders[0].hyper_loops.len(), 6);
    assert_labels(&graph, PortSide::North, &[Right, Left], true);
    assert_labels(&graph, PortSide::South, &[Left, Right], true);
    for edge in [0, 5] {
        assert!(graph.edges[edge].labels.is_empty());
    }
}

#[test]
fn default_stacked_labels_still_clear_inline_on_north_and_south() {
    let graph = route(&input(
        6,
        LayeredOptions {
            self_loop_distribution: SelfLoopDistributionStrategy::NorthSouth,
            ..Default::default()
        },
    ));

    assert_eq!(
        graph.options.self_loop_ordering,
        SelfLoopOrderingStrategy::Stacked
    );
    for side in [PortSide::North, PortSide::South] {
        assert_labels(&graph, side, &[SelfLoopLabelAlignment::Center; 3], false);
    }
}

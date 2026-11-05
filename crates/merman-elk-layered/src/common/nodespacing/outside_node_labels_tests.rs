use super::*;
use crate::graph::{LNode, LPoint, PortType};
use crate::options::{ElkDirection, ElkPadding, LayeredOptions};

fn labeled_graph(placement: NodeLabelPlacement, direction: ElkDirection) -> LGraph {
    let mut graph = LGraph::new(
        "root",
        LayeredOptions {
            direction,
            ..Default::default()
        },
    );
    let mut node = LNode::new("node", 80.0, 40.0, None);
    node.position = LPoint { x: 13.0, y: 19.0 };
    node.node_label_placement = placement;
    node.labels.push(LLabel::new("first", 20.0, 10.0));
    graph.layerless_nodes.push(node);
    graph
}

fn positions(graph: &LGraph) -> Vec<(f64, f64)> {
    graph.layerless_nodes[0]
        .labels
        .iter()
        .map(|label| (label.position.x, label.position.y))
        .collect()
}

#[test]
fn outside_locations_place_labels_and_reserve_only_their_external_bounds() {
    use NodeLabelPlacement::*;

    let cases = [
        (OutsideTopLeft, (0.0, -15.0), (15.0, 0.0, 0.0, 0.0)),
        (OutsideTopCenter, (30.0, -15.0), (15.0, 0.0, 0.0, 0.0)),
        (OutsideTopRight, (60.0, -15.0), (15.0, 0.0, 0.0, 0.0)),
        (OutsideBottomLeft, (0.0, 45.0), (0.0, 15.0, 0.0, 0.0)),
        (OutsideBottomCenter, (30.0, 45.0), (0.0, 15.0, 0.0, 0.0)),
        (OutsideBottomRight, (60.0, 45.0), (0.0, 15.0, 0.0, 0.0)),
        (OutsideLeftTop, (-25.0, 0.0), (0.0, 0.0, 25.0, 0.0)),
        (OutsideLeftCenter, (-25.0, 15.0), (0.0, 0.0, 25.0, 0.0)),
        (OutsideLeftBottom, (-25.0, 30.0), (0.0, 0.0, 25.0, 0.0)),
        (OutsideRightTop, (85.0, 0.0), (0.0, 0.0, 0.0, 25.0)),
        (OutsideRightCenter, (85.0, 15.0), (0.0, 0.0, 0.0, 25.0)),
        (OutsideRightBottom, (85.0, 30.0), (0.0, 0.0, 0.0, 25.0)),
    ];

    for (placement, expected_position, (top, bottom, left, right)) in cases {
        let mut graph = labeled_graph(placement, ElkDirection::Right);
        calculate_label_and_node_sizes(&mut graph, [0]);
        calculate_node_margins(&mut graph, [0]);

        assert_eq!(positions(&graph), [expected_position], "{placement:?}");
        assert_eq!(
            graph.layerless_nodes[0].margin,
            LMargin {
                top,
                bottom,
                left,
                right
            },
            "{placement:?}"
        );
        assert_eq!(
            graph.layerless_nodes[0].size,
            LSize {
                width: 80.0,
                height: 40.0
            },
            "{placement:?}"
        );
    }
}

#[test]
fn horizontal_mode_aligns_each_label_within_the_same_outside_group() {
    use NodeLabelPlacement::*;

    let cases = [
        (OutsideTopLeft, [(0.0, -30.0), (0.0, -17.0)]),
        (OutsideTopCenter, [(30.0, -30.0), (25.0, -17.0)]),
        (OutsideTopRight, [(60.0, -30.0), (50.0, -17.0)]),
        (OutsideBottomLeft, [(0.0, 45.0), (0.0, 58.0)]),
        (OutsideBottomCenter, [(30.0, 45.0), (25.0, 58.0)]),
        (OutsideBottomRight, [(60.0, 45.0), (50.0, 58.0)]),
        (OutsideLeftTop, [(-25.0, 0.0), (-35.0, 13.0)]),
        (OutsideLeftCenter, [(-25.0, 7.5), (-35.0, 20.5)]),
        (OutsideLeftBottom, [(-25.0, 15.0), (-35.0, 28.0)]),
        (OutsideRightTop, [(85.0, 0.0), (85.0, 13.0)]),
        (OutsideRightCenter, [(85.0, 7.5), (85.0, 20.5)]),
        (OutsideRightBottom, [(85.0, 15.0), (85.0, 28.0)]),
    ];

    for direction in [
        ElkDirection::Right,
        ElkDirection::Left,
        ElkDirection::Undefined,
    ] {
        for (placement, expected) in cases {
            let mut graph = labeled_graph(placement, direction);
            graph.options.spacing.label_label = 3.0;
            graph.layerless_nodes[0]
                .labels
                .push(LLabel::new("second", 30.0, 12.0));

            calculate_label_and_node_sizes(&mut graph, [0]);

            assert_eq!(positions(&graph), expected, "{direction:?} {placement:?}");
        }
    }
}

#[test]
fn vertical_mode_aligns_each_column_within_the_same_outside_group() {
    use NodeLabelPlacement::*;

    let cases = [
        (OutsideTopLeft, [(0.0, -15.0), (23.0, -17.0)]),
        (OutsideTopCenter, [(13.5, -15.0), (36.5, -17.0)]),
        (OutsideTopRight, [(27.0, -15.0), (50.0, -17.0)]),
        (OutsideBottomLeft, [(0.0, 45.0), (23.0, 45.0)]),
        (OutsideBottomCenter, [(13.5, 45.0), (36.5, 45.0)]),
        (OutsideBottomRight, [(27.0, 45.0), (50.0, 45.0)]),
        (OutsideLeftTop, [(-58.0, 0.0), (-35.0, 0.0)]),
        (OutsideLeftCenter, [(-58.0, 15.0), (-35.0, 14.0)]),
        (OutsideLeftBottom, [(-58.0, 30.0), (-35.0, 28.0)]),
        (OutsideRightTop, [(85.0, 0.0), (108.0, 0.0)]),
        (OutsideRightCenter, [(85.0, 15.0), (108.0, 14.0)]),
        (OutsideRightBottom, [(85.0, 30.0), (108.0, 28.0)]),
    ];

    for direction in [ElkDirection::Down, ElkDirection::Up] {
        for (placement, expected) in cases {
            let mut graph = labeled_graph(placement, direction);
            graph.options.spacing.label_label = 3.0;
            graph.layerless_nodes[0]
                .labels
                .push(LLabel::new("second", 30.0, 12.0));

            calculate_label_and_node_sizes(&mut graph, [0]);

            assert_eq!(positions(&graph), expected, "{direction:?} {placement:?}");
        }
    }
}

#[test]
fn outside_spacing_is_independent_of_inside_label_padding_and_parent_minimum() {
    let mut graph = labeled_graph(NodeLabelPlacement::OutsideBottomCenter, ElkDirection::Right);
    graph.options.node_labels_padding = ElkPadding::uniform(30.0);
    graph.options.spacing.label_node = 11.0;
    graph.options.node_size_minimum = Some(LSize {
        width: 200.0,
        height: 150.0,
    });
    graph.options.node_size_include_labels = true;

    calculate_label_and_node_sizes(&mut graph, [0]);
    calculate_node_margins(&mut graph, [0]);

    assert_eq!(positions(&graph), [(30.0, 51.0)]);
    assert_eq!(graph.layerless_nodes[0].margin.bottom, 21.0);
    assert_eq!(
        graph.layerless_nodes[0].size,
        LSize {
            width: 80.0,
            height: 40.0
        }
    );

    graph.layerless_nodes[0].size = LSize {
        width: 120.0,
        height: 60.0,
    };
    calculate_label_and_node_sizes(&mut graph, [0]);

    assert_eq!(positions(&graph), [(50.0, 71.0)]);
}

#[test]
fn outside_groups_overhang_fixed_nodes_instead_of_expanding_the_inside_grid() {
    use NodeLabelPlacement::*;

    let cases = [
        (OutsideTopLeft, [(0.0, -30.0), (0.0, -17.0)]),
        (OutsideTopCenter, [(-5.0, -30.0), (-10.0, -17.0)]),
        (OutsideTopRight, [(-10.0, -30.0), (-20.0, -17.0)]),
        (OutsideLeftTop, [(-25.0, 0.0), (-35.0, 13.0)]),
        (OutsideLeftCenter, [(-25.0, -9.5), (-35.0, 3.5)]),
        (OutsideLeftBottom, [(-25.0, -19.0), (-35.0, -6.0)]),
    ];
    for (placement, expected) in cases {
        let mut graph = labeled_graph(placement, ElkDirection::Right);
        graph.options.spacing.label_label = 3.0;
        graph.layerless_nodes[0].size = LSize {
            width: 10.0,
            height: 6.0,
        };
        graph.layerless_nodes[0]
            .labels
            .push(LLabel::new("second", 30.0, 12.0));

        calculate_label_and_node_sizes(&mut graph, [0]);

        assert_eq!(positions(&graph), expected, "{placement:?}");
        assert_eq!(
            graph.layerless_nodes[0].size,
            LSize {
                width: 10.0,
                height: 6.0
            }
        );
    }
}

#[test]
fn fixed_labels_keep_raw_positions_and_still_contribute_to_margins() {
    let mut graph = labeled_graph(NodeLabelPlacement::Fixed, ElkDirection::Right);
    graph.layerless_nodes[0].labels[0].position = LPoint { x: -7.0, y: -11.0 };
    let mut second = LLabel::new("second", 30.0, 12.0);
    second.position = LPoint { x: 92.0, y: 47.0 };
    graph.layerless_nodes[0].labels.push(second);
    let labels = graph.layerless_nodes[0].labels.clone();

    calculate_label_and_node_sizes(&mut graph, [0]);
    calculate_node_margins(&mut graph, [0]);

    assert_eq!(graph.layerless_nodes[0].labels, labels);
    assert_eq!(
        graph.layerless_nodes[0].margin,
        LMargin {
            top: 11.0,
            bottom: 19.0,
            left: 7.0,
            right: 42.0
        }
    );
}

#[test]
fn outside_groups_share_margins_with_ports_without_using_port_label_offsets() {
    use NodeLabelPlacement::*;

    let cases = [
        (
            OutsideTopCenter,
            LMargin {
                top: 30.0,
                bottom: 20.0,
                left: 29.0,
                right: 27.0,
            },
            [(30.0, -30.0), (25.0, -17.0)],
        ),
        (
            OutsideBottomCenter,
            LMargin {
                top: 22.0,
                bottom: 30.0,
                left: 29.0,
                right: 27.0,
            },
            [(30.0, 45.0), (25.0, 58.0)],
        ),
        (
            OutsideLeftCenter,
            LMargin {
                top: 22.0,
                bottom: 20.0,
                left: 35.0,
                right: 27.0,
            },
            [(-25.0, 7.5), (-35.0, 20.5)],
        ),
        (
            OutsideRightCenter,
            LMargin {
                top: 22.0,
                bottom: 20.0,
                left: 29.0,
                right: 35.0,
            },
            [(85.0, 7.5), (85.0, 20.5)],
        ),
    ];
    let mut baseline = labeled_graph(NodeLabelPlacement::Fixed, ElkDirection::Right);
    baseline.layerless_nodes[0].labels.clear();
    baseline.layerless_nodes[0].port_constraints = PortConstraints::FixedPos;
    baseline.options.spacing.label_label = 3.0;
    for (side, offset, label_position, label_width) in [
        (PortSide::North, 4.0, LPoint { x: 2.0, y: -10.0 }, 10.0),
        (PortSide::South, 4.0, LPoint { x: 1.0, y: 50.0 }, 10.0),
        (PortSide::West, 2.0, LPoint { x: -21.0, y: 3.0 }, 10.0),
        (PortSide::East, 3.0, LPoint { x: 9.0, y: -4.0 }, 15.0),
    ] {
        let mut port = LPort::new(format!("{side:?}"), 0, PortType::Output);
        port.size = LSize {
            width: 6.0,
            height: 8.0,
        };
        port.position = LPoint { x: 13.0, y: 17.0 };
        port.border_offset = Some(offset);
        port.set_side(side);
        let mut label = LLabel::new("port", label_width, 6.0);
        label.position = label_position;
        port.labels.push(label);
        baseline.layerless_nodes[0].ports.push(port);
    }
    calculate_label_and_node_sizes(&mut baseline, [0]);

    for (placement, expected_margin, expected_positions) in cases {
        let mut graph = baseline.clone();
        graph.layerless_nodes[0].node_label_placement = placement;
        graph.layerless_nodes[0].labels = vec![
            LLabel::new("first", 20.0, 10.0),
            LLabel::new("second", 30.0, 12.0),
        ];

        calculate_label_and_node_sizes(&mut graph, [0]);
        calculate_node_margins(&mut graph, [0]);

        assert_eq!(
            graph.layerless_nodes[0].ports,
            baseline.layerless_nodes[0].ports
        );
        assert_eq!(
            graph.layerless_nodes[0].margin, expected_margin,
            "{placement:?}"
        );
        assert_eq!(positions(&graph), expected_positions, "{placement:?}");
    }
}

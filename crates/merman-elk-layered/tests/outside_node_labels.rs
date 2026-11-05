use merman_elk_layered::{
    CrossingMinimizationStrategy, ElkDirection, ElkInputEdge, ElkInputGraph, ElkInputLabel,
    ElkInputNode, ElkPadding, LGraph, LLabel, LNode, LPoint, LSize, LayeredOptions,
    LayeringStrategy, NodeFlexibility, NodeLabelPlacement, NodePlacementStrategy, PipelineError,
    ProcessorKind, SpacingOptions, WorkControl, WorkError, execute_ported_processors,
    execute_processors_until_processor, execute_processors_until_processor_with_work_control,
    import_graph,
};

fn graph(placement: NodeLabelPlacement, direction: ElkDirection) -> LGraph {
    let mut graph = LGraph::new(
        "root",
        LayeredOptions {
            direction,
            padding: ElkPadding::uniform(0.0),
            separate_connected_components: false,
            ..Default::default()
        },
    );
    let mut node = LNode::new("node", 80.0, 40.0, None);
    node.node_label_placement = placement;
    let mut label = LLabel::new("label", 20.0, 10.0);
    label.position = LPoint { x: -99.0, y: 13.0 };
    label.source_index = Some(7);
    node.labels.push(label);
    graph.layerless_nodes.push(node);
    graph
}

#[test]
fn public_pipeline_preserves_outside_locations_through_direction_transforms_and_bounds() {
    use NodeLabelPlacement::*;

    let cases = [
        (OutsideTopLeft, (0.0, -15.0), (80.0, 55.0)),
        (OutsideTopCenter, (30.0, -15.0), (80.0, 55.0)),
        (OutsideTopRight, (60.0, -15.0), (80.0, 55.0)),
        (OutsideBottomLeft, (0.0, 45.0), (80.0, 55.0)),
        (OutsideBottomCenter, (30.0, 45.0), (80.0, 55.0)),
        (OutsideBottomRight, (60.0, 45.0), (80.0, 55.0)),
        (OutsideLeftTop, (-25.0, 0.0), (105.0, 40.0)),
        (OutsideLeftCenter, (-25.0, 15.0), (105.0, 40.0)),
        (OutsideLeftBottom, (-25.0, 30.0), (105.0, 40.0)),
        (OutsideRightTop, (85.0, 0.0), (105.0, 40.0)),
        (OutsideRightCenter, (85.0, 15.0), (105.0, 40.0)),
        (OutsideRightBottom, (85.0, 30.0), (105.0, 40.0)),
    ];
    for direction in [
        ElkDirection::Right,
        ElkDirection::Left,
        ElkDirection::Down,
        ElkDirection::Up,
    ] {
        for (placement, expected_label, expected_root) in cases {
            let mut graph = graph(placement, direction);

            let executed = execute_ported_processors(&mut graph).unwrap();

            assert!(executed.contains(&ProcessorKind::LabelAndNodeSizeProcessor));
            assert!(executed.contains(&ProcessorKind::InnermostNodeMarginCalculator));
            let node = &graph.layerless_nodes[0];
            let label = &node.labels[0];
            assert_eq!(
                (label.position.x, label.position.y),
                expected_label,
                "{direction:?} {placement:?}"
            );
            assert_eq!(node.node_label_placement, placement);
            assert_eq!(
                node.size,
                LSize {
                    width: 80.0,
                    height: 40.0
                }
            );
            assert_eq!(label.source_index, Some(7));
            let size = graph.exported_root_size();
            assert_eq!(
                (size.width, size.height),
                expected_root,
                "{direction:?} {placement:?}"
            );
            let label_x = node.position.x + graph.offset.x + graph.padding.left + label.position.x;
            let label_y = node.position.y + graph.offset.y + graph.padding.top + label.position.y;
            assert!(label_x >= 0.0 && label_x + label.size.width <= size.width);
            assert!(label_y >= 0.0 && label_y + label.size.height <= size.height);
        }
    }
}

#[test]
fn public_pipeline_places_the_whole_outside_group_before_restoring_input_direction() {
    let cases = [
        (ElkDirection::Right, [(30.0, 45.0), (25.0, 58.0)]),
        (ElkDirection::Left, [(30.0, 45.0), (25.0, 58.0)]),
        (ElkDirection::Down, [(30.0, 45.0), (25.0, 58.0)]),
        (ElkDirection::Up, [(30.0, 60.0), (25.0, 45.0)]),
    ];
    for (direction, expected) in cases {
        let mut graph = graph(NodeLabelPlacement::OutsideBottomCenter, direction);
        graph.options.spacing.label_label = 3.0;
        graph.layerless_nodes[0]
            .labels
            .push(LLabel::new("second", 30.0, 12.0));

        execute_ported_processors(&mut graph).unwrap();

        let positions = graph.layerless_nodes[0]
            .labels
            .iter()
            .map(|label| (label.position.x, label.position.y))
            .collect::<Vec<_>>();
        assert_eq!(positions, expected, "{direction:?}");
        assert_eq!(
            graph.exported_root_size(),
            LSize {
                width: 80.0,
                height: 70.0
            }
        );
    }
}

fn linear_segments_input(nodes: &[(&str, f64)], label_height: f64) -> ElkInputGraph {
    ElkInputGraph {
        id: "root".into(),
        options: LayeredOptions {
            direction: ElkDirection::Right,
            layering_strategy: LayeringStrategy::LongestPath,
            crossing_minimization_strategy: CrossingMinimizationStrategy::None,
            node_placement_strategy: NodePlacementStrategy::LinearSegments,
            spacing: SpacingOptions {
                edge_node: 10.0,
                label_node: 5.0,
                ..Default::default()
            },
            ..Default::default()
        },
        nodes: nodes
            .iter()
            .copied()
            .map(|(id, height)| ElkInputNode {
                id: id.into(),
                width: 80.0,
                height,
                parent: None,
                direction: None,
                hierarchy_handling: None,
                layer_constraint: None,
                port_constraints: None,
                port_alignment: None,
                node_label_placement: if id == "N" {
                    NodeLabelPlacement::OutsideBottomCenter
                } else {
                    NodeLabelPlacement::Fixed
                },
                node_flexibility: NodeFlexibility::None,
                ports_surrounding: None,
                nested_options: None,
                label: (id == "N").then(|| ElkInputLabel::center("outside", 80.0, label_height)),
            })
            .collect(),
        edges: nodes
            .windows(2)
            .map(|pair| (pair[0].0, pair[1].0))
            .chain([("S", "T")])
            .map(|(source, target)| ElkInputEdge {
                id: format!("{source}{target}"),
                source: source.into(),
                target: target.into(),
                labels: Vec::new(),
                minlen: 1,
                inside_self_loops_yo: false,
                model_order: None,
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
            })
            .collect(),
    }
}

#[test]
fn imported_linear_segments_long_edge_preserves_outside_label_clearance() {
    let input = linear_segments_input(
        &[("S", 40.0), ("N", 40.0), ("M", 100.0), ("T", 40.0)],
        200.0,
    );
    let mut graph = import_graph(&input).unwrap();

    let executed = execute_ported_processors(&mut graph).unwrap();

    assert!(executed.contains(&ProcessorKind::LinearSegmentsNodePlacer));
    let node = graph
        .layerless_nodes
        .iter()
        .find(|node| node.id == "N")
        .unwrap();
    let label = &node.labels[0];
    assert_eq!(
        node.size,
        LSize {
            width: 80.0,
            height: 40.0
        }
    );
    assert_eq!(label.position, LPoint { x: 0.0, y: 45.0 });
    let left = node.position.x + label.position.x;
    let top = node.position.y + label.position.y;
    let right = left + label.size.width;
    let bottom = top + label.size.height;
    let edge = graph.edges.iter().find(|edge| edge.id == "ST").unwrap();
    let mut route = Vec::new();
    let source = &graph.layerless_nodes[edge.source.node];
    let source_port = &source.ports[edge.source.port];
    route.push(LPoint {
        x: source.position.x + source_port.position.x + source_port.anchor.x,
        y: source.position.y + source_port.position.y + source_port.anchor.y,
    });
    route.extend_from_slice(&edge.bend_points);
    let target = &graph.layerless_nodes[edge.target.node];
    let target_port = &target.ports[edge.target.port];
    route.push(LPoint {
        x: target.position.x + target_port.position.x + target_port.anchor.x,
        y: target.position.y + target_port.position.y + target_port.anchor.y,
    });
    let clearance = input.options.spacing.edge_node;
    for segment in route.windows(2) {
        let [start, end] = [segment[0], segment[1]];
        assert!(
            start.x == end.x || start.y == end.y,
            "non-orthogonal ST segment: {segment:?}"
        );
        assert!(
            start.x.max(end.x) <= left - clearance
                || start.x.min(end.x) >= right + clearance
                || start.y.max(end.y) <= top - clearance
                || start.y.min(end.y) >= bottom + clearance,
            "ST segment {segment:?} must keep {clearance} clearance from N's outside label \
             [left={left}, top={top}, right={right}, bottom={bottom}]"
        );
    }
}

#[test]
fn imported_linear_segments_large_finite_centers_do_not_overflow_the_mean() {
    let input = linear_segments_input(
        &[
            ("S", 40.0),
            ("N", 40.0),
            ("M", 100.0),
            ("K", 40.0),
            ("T", 40.0),
        ],
        1e308,
    );
    let mut graph = import_graph(&input).unwrap();
    execute_ported_processors(&mut graph).unwrap();

    assert!(graph.size.width.is_finite() && graph.size.height.is_finite());
    for node in &graph.layerless_nodes {
        assert!(node.position.x.is_finite() && node.position.y.is_finite());
    }
    for edge in &graph.edges {
        assert!(
            edge.bend_points
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
        );
    }
}

#[test]
fn root_minimum_does_not_resize_a_fixed_child_to_fit_an_outside_label() {
    let mut graph = graph(NodeLabelPlacement::OutsideBottomCenter, ElkDirection::Right);
    graph.options.node_size_minimum = Some(LSize {
        width: 200.0,
        height: 150.0,
    });
    graph.layerless_nodes[0].size = LSize {
        width: 10.0,
        height: 6.0,
    };
    graph.layerless_nodes[0].labels[0].size = LSize {
        width: 30.0,
        height: 12.0,
    };

    execute_ported_processors(&mut graph).unwrap();

    let node = &graph.layerless_nodes[0];
    assert_eq!(
        node.size,
        LSize {
            width: 10.0,
            height: 6.0
        }
    );
    assert_eq!(node.labels[0].position, LPoint { x: -10.0, y: 11.0 });
    assert_eq!(
        graph.exported_root_size(),
        LSize {
            width: 200.0,
            height: 150.0
        }
    );
}

struct InterruptControl {
    accepted_charges: Vec<usize>,
    limit: usize,
    interrupt_on_check: bool,
}

impl WorkControl for InterruptControl {
    fn check(&mut self, _units: usize) -> Result<(), WorkError> {
        if self.interrupt_on_check && self.accepted_charges.len() >= self.limit {
            Err(WorkError::Interrupted)
        } else {
            Ok(())
        }
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        if self.accepted_charges.len() >= self.limit {
            return Err(WorkError::Interrupted);
        }
        self.accepted_charges.push(units);
        Ok(())
    }
}

#[test]
fn work_control_rejects_label_placement_without_changing_the_existing_graph_context() {
    let mut original = graph(NodeLabelPlacement::OutsideBottomCenter, ElkDirection::Down);
    original.layerless_nodes[0]
        .labels
        .push(LLabel::new("second", 30.0, 12.0));
    let target = ProcessorKind::LabelAndNodeSizeProcessor;
    let mut expected = original.clone();
    let mut measured = InterruptControl {
        accepted_charges: Vec::new(),
        limit: usize::MAX,
        interrupt_on_check: false,
    };
    let executed =
        execute_processors_until_processor_with_work_control(&mut expected, target, &mut measured)
            .unwrap();
    assert_eq!(executed.last(), Some(&target));
    let predecessor = executed[executed.len() - 2];
    let mut before = original.clone();
    execute_processors_until_processor(&mut before, predecessor).unwrap();
    assert_ne!(
        expected.layerless_nodes[0].labels,
        before.layerless_nodes[0].labels
    );
    let limit = measured.accepted_charges.len() - 1;

    for interrupt_on_check in [true, false] {
        let mut rejected = original.clone();
        let mut control = InterruptControl {
            accepted_charges: Vec::new(),
            limit,
            interrupt_on_check,
        };

        assert_eq!(
            execute_processors_until_processor_with_work_control(
                &mut rejected,
                target,
                &mut control
            ),
            Err(PipelineError::Work(WorkError::Interrupted))
        );
        assert_eq!(control.accepted_charges, measured.accepted_charges[..limit]);
        assert_eq!(rejected, before);
    }

    let mut accepted = original;
    let mut control = InterruptControl {
        accepted_charges: Vec::new(),
        limit: measured.accepted_charges.len(),
        interrupt_on_check: true,
    };
    assert_eq!(
        execute_processors_until_processor_with_work_control(&mut accepted, target, &mut control)
            .unwrap(),
        executed
    );
    assert_eq!(accepted, expected);
    assert_eq!(control.accepted_charges, measured.accepted_charges);
}

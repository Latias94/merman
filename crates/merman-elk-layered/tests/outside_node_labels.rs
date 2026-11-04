use merman_elk_layered::{
    ElkDirection, ElkPadding, LGraph, LLabel, LNode, LPoint, LSize, LayeredOptions,
    NodeLabelPlacement, PipelineError, ProcessorKind, WorkControl, WorkError,
    execute_ported_processors, execute_processors_until_processor,
    execute_processors_until_processor_with_work_control,
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

use merman_elk_layered::*;

struct ScoringControl {
    remaining: usize,
    used: usize,
}

impl ScoringControl {
    fn new(remaining: usize) -> Self {
        Self { remaining, used: 0 }
    }
}

impl WorkControl for ScoringControl {
    fn check(&mut self, units: usize) -> Result<(), WorkError> {
        if units > self.remaining {
            Err(WorkError::Interrupted)
        } else {
            Ok(())
        }
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.check(units)?;
        self.remaining -= units;
        self.used += units;
        Ok(())
    }
}

fn input_graph(ids: &[&str], edges: &[(usize, usize)], options: LayeredOptions) -> LGraph {
    import_graph(&ElkInputGraph {
        id: "scoring".into(),
        options,
        nodes: ids
            .iter()
            .map(|id| ElkInputNode {
                id: (*id).into(),
                width: 20.0,
                height: 10.0,
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
            })
            .collect(),
        edges: edges
            .iter()
            .enumerate()
            .map(|(index, &(source, target))| ElkInputEdge {
                id: format!("edge-{index}"),
                source: ids[source].into(),
                target: ids[target].into(),
                labels: Vec::new(),
                minlen: 1,
                inside_self_loops_yo: false,
                model_order: Some(index),
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
            })
            .collect(),
    })
    .unwrap()
}

fn options() -> LayeredOptions {
    LayeredOptions {
        direction: ElkDirection::Right,
        hierarchy_handling: HierarchyHandling::IncludeChildren,
        separate_connected_components: false,
        greedy_switch_type: GreedySwitchType::Off,
        greedy_switch_hierarchical_type: GreedySwitchType::Off,
        consider_model_order_strategy: OrderingStrategy::NodesAndEdges,
        consider_model_order_crossing_counter_node_influence: 10.0,
        thoroughness: 2,
        random_seed: 31,
        ..LayeredOptions::default()
    }
}

fn crossed_graph() -> LGraph {
    input_graph(
        &["Top", "Bottom", "Left", "Right"],
        &[(0, 3), (1, 2)],
        options(),
    )
}

#[test]
fn fixed_port_diagnostic_scores_model_order_before_any_port_sort_cache_exists() {
    for constraints in [PortConstraints::FixedOrder, PortConstraints::FixedPos] {
        let mut options = options();
        options.consider_model_order_strategy = OrderingStrategy::PreferNodes;
        options.consider_model_order_port_model_order = false;
        options.consider_model_order_crossing_counter_node_influence = 1.0;
        options.consider_model_order_crossing_counter_port_influence = 1.0;
        let mut graph = input_graph(&["S", "A", "B"], &[(0, 2), (0, 1)], options);
        graph.layerless_nodes[0].port_constraints = constraints;

        let mut prefix = graph.clone();
        execute_ported_compound_processors_until_processor(
            &mut prefix,
            ProcessorKind::SortByInputModelProcessor,
        )
        .unwrap();
        assert!(
            prefix.layerless_nodes[0]
                .ports
                .iter()
                .all(|port| port.long_edge_target_node.is_none())
        );

        let mut expected = graph.clone();
        let expected_result = inspect_compound_crossings_after_processor(
            &mut expected,
            ProcessorKind::SortByInputModelProcessor,
        )
        .unwrap();
        let trace = expected_result.1.as_ref().unwrap();
        assert_eq!(trace.runs[0].initial_crossings, 2.0, "{constraints:?}");
        assert!(!trace.runs[0].early_returned);

        let mut actual = graph.clone();
        let mut measured = ScoringControl::new(usize::MAX);
        assert_eq!(
            inspect_compound_crossings_after_processor_with_work_control(
                &mut actual,
                ProcessorKind::SortByInputModelProcessor,
                &mut measured,
            )
            .unwrap(),
            expected_result
        );
        assert_eq!(actual, expected);
        let mut exact = ScoringControl::new(measured.used);
        assert_eq!(
            inspect_compound_crossings_after_processor_with_work_control(
                &mut graph,
                ProcessorKind::SortByInputModelProcessor,
                &mut exact,
            )
            .unwrap(),
            expected_result
        );
        assert_eq!(exact.remaining, 0);
        assert_eq!(graph, expected);
    }
}

#[test]
fn weighted_diagnostic_retains_nonzero_first_attempt_and_randomizes_the_second_attempt() {
    let mut graph = crossed_graph();
    let (_, trace) = inspect_compound_crossings_after_processor(
        &mut graph,
        ProcessorKind::SortByInputModelProcessor,
    )
    .unwrap();
    let trace = trace.unwrap();
    assert_eq!(trace.runs.len(), 2);
    assert_eq!(trace.runs[0].initial_crossings, 1.0);
    assert_eq!(trace.runs[0].crossings, 10.0);
    assert!(trace.runs[0].first_try_with_initial_order);
    assert!(!trace.runs[0].early_returned);
    assert!(!trace.runs[1].first_try_with_initial_order);
    assert!(trace.layer_sweeps.iter().any(|sweep| {
        sweep.forward
            && sweep
                .after
                .iter()
                .map(|node| node.node_id.as_str())
                .collect::<Vec<_>>()
                == ["Right", "Left"]
    }));
}

#[derive(Clone, Copy, Debug)]
enum EntryPoint {
    FlatPhase,
    FlatProcessor,
    FlatComplete,
    CompoundPhase,
    CompoundProcessor,
    CompoundComplete,
}

fn run(entry: EntryPoint, graph: &mut LGraph, work: &mut dyn WorkControl) -> PipelineResult<()> {
    match entry {
        EntryPoint::FlatPhase => {
            execute_processors_until_with_work_control(graph, LayeredPhase::P3NodeOrdering, work)
                .map(|_| ())
        }
        EntryPoint::FlatProcessor => execute_processors_until_processor_with_work_control(
            graph,
            ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
            work,
        )
        .map(|_| ()),
        EntryPoint::FlatComplete => {
            execute_ported_processors_with_work_control(graph, work).map(|_| ())
        }
        EntryPoint::CompoundPhase => execute_ported_compound_processors_until_with_work_control(
            graph,
            LayeredPhase::P3NodeOrdering,
            work,
        )
        .map(|_| ()),
        EntryPoint::CompoundProcessor => {
            execute_ported_compound_processors_until_processor_with_work_control(
                graph,
                ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
                work,
            )
            .map(|_| ())
        }
        EntryPoint::CompoundComplete => {
            execute_ported_compound_processors_with_work_control(graph, work).map(|_| ())
        }
    }
}

fn run_unrestricted(entry: EntryPoint, graph: &mut LGraph) -> PipelineResult<()> {
    match entry {
        EntryPoint::FlatPhase => {
            execute_processors_until(graph, LayeredPhase::P3NodeOrdering).map(|_| ())
        }
        EntryPoint::FlatProcessor => execute_processors_until_processor(
            graph,
            ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
        )
        .map(|_| ()),
        EntryPoint::FlatComplete => execute_ported_processors(graph).map(|_| ()),
        EntryPoint::CompoundPhase => {
            execute_ported_compound_processors_until(graph, LayeredPhase::P3NodeOrdering)
                .map(|_| ())
        }
        EntryPoint::CompoundProcessor => execute_ported_compound_processors_until_processor(
            graph,
            ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
        )
        .map(|_| ()),
        EntryPoint::CompoundComplete => execute_ported_compound_processors(graph).map(|_| ()),
    }
}

#[test]
fn public_flat_and_compound_callers_preserve_weighted_orders_and_controlled_random_streams() {
    let original = crossed_graph();
    let mut reference = original.clone();
    inspect_compound_crossings_after_processor(
        &mut reference,
        ProcessorKind::SortByInputModelProcessor,
    )
    .unwrap();
    for entry in [
        EntryPoint::FlatPhase,
        EntryPoint::FlatProcessor,
        EntryPoint::FlatComplete,
        EntryPoint::CompoundPhase,
        EntryPoint::CompoundProcessor,
        EntryPoint::CompoundComplete,
    ] {
        let mut expected = original.clone();
        run_unrestricted(entry, &mut expected).unwrap();
        let mut actual = original.clone();
        let mut measured = ScoringControl::new(usize::MAX);
        run(entry, &mut actual, &mut measured).unwrap();
        assert_eq!(actual, expected, "{entry:?}");
        if matches!(
            entry,
            EntryPoint::FlatComplete | EntryPoint::CompoundComplete
        ) {
            assert!(actual.layers.is_empty(), "{entry:?}");
            for layer in &reference.layers {
                for pair in layer.nodes.windows(2) {
                    assert!(
                        actual.layerless_nodes[pair[0]].position.y
                            < actual.layerless_nodes[pair[1]].position.y,
                        "{entry:?}: {pair:?}"
                    );
                }
            }
        } else {
            assert_eq!(
                actual
                    .layers
                    .iter()
                    .map(|layer| &layer.nodes)
                    .collect::<Vec<_>>(),
                reference
                    .layers
                    .iter()
                    .map(|layer| &layer.nodes)
                    .collect::<Vec<_>>(),
                "{entry:?}"
            );
        }
        assert!(measured.used > 0);
        assert_eq!(
            run(
                entry,
                &mut original.clone(),
                &mut ScoringControl::new(measured.used - 1)
            ),
            Err(PipelineError::Work(WorkError::Interrupted)),
            "{entry:?}"
        );
        let mut exact_graph = original.clone();
        let mut exact = ScoringControl::new(measured.used);
        run(entry, &mut exact_graph, &mut exact).unwrap();
        assert_eq!(exact.remaining, 0, "{entry:?}");
        assert_eq!(exact_graph, expected, "{entry:?}");
    }
}

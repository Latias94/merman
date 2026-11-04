use merman_elk_layered::{
    ElkDirection, ElkInputEdge, ElkInputGraph, ElkInputNode, GraphExecution, GreedySwitchType,
    HierarchyHandling, HierarchySweepDebugTrace, LGraph, LayeredOptions, NodeFlexibility,
    NodeLabelPlacement, OrderingStrategy, PipelineError, PipelineResult, ProcessorKind,
    WorkControl, WorkError, execute_ported_compound_processors_until_processor,
    execute_ported_compound_processors_until_processor_with_work_control, import_graph,
    inspect_compound_crossings_after_processor,
    inspect_compound_crossings_after_processor_with_work_control,
};

struct ScoringControl {
    remaining: usize,
    charges: Vec<usize>,
    interrupt_after: Option<usize>,
}

impl ScoringControl {
    fn new(remaining: usize) -> Self {
        Self {
            remaining,
            charges: Vec::new(),
            interrupt_after: None,
        }
    }
}

impl WorkControl for ScoringControl {
    fn check(&mut self, units: usize) -> Result<(), WorkError> {
        if units > self.remaining
            || self
                .interrupt_after
                .is_some_and(|limit| self.charges.len() >= limit)
        {
            Err(WorkError::Interrupted)
        } else {
            Ok(())
        }
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.check(units)?;
        self.remaining -= units;
        self.charges.push(units);
        Ok(())
    }
}

fn input_node(id: String, parent: Option<&str>) -> ElkInputNode {
    ElkInputNode {
        id,
        width: 20.0,
        height: 10.0,
        parent: parent.map(str::to_owned),
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
    }
}

fn fan_in_graph(nested: bool, node_orders: bool) -> LGraph {
    let options = LayeredOptions {
        direction: ElkDirection::Right,
        hierarchy_handling: HierarchyHandling::IncludeChildren,
        greedy_switch_type: GreedySwitchType::Off,
        greedy_switch_hierarchical_type: GreedySwitchType::Off,
        consider_model_order_strategy: OrderingStrategy::NodesAndEdges,
        consider_model_order_crossing_counter_node_influence: 1.0,
        consider_model_order_crossing_counter_port_influence: 1.0,
        thoroughness: 3,
        random_seed: 31,
        ..LayeredOptions::default()
    };
    let parent = nested.then_some("group");
    let mut nodes = (0..12)
        .map(|index| input_node(format!("leaf-{index}"), parent))
        .collect::<Vec<_>>();
    nodes.push(input_node("sink".into(), parent));
    if nested {
        let mut group = input_node("group".into(), None);
        group.nested_options = Some(Box::new(options.clone()));
        nodes.insert(0, group);
    }
    let edges = (0..12)
        .map(|index| ElkInputEdge {
            id: format!("edge-{index}"),
            source: format!("leaf-{index}"),
            target: "sink".into(),
            labels: Vec::new(),
            minlen: 1,
            inside_self_loops_yo: false,
            model_order: Some(index),
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
        })
        .collect();
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".into(),
        options,
        nodes,
        edges,
    })
    .unwrap();
    if !node_orders {
        let scope = if nested {
            graph.layerless_nodes[0]
                .nested_graph
                .as_deref_mut()
                .unwrap()
        } else {
            &mut graph
        };
        for node in &mut scope.layerless_nodes {
            node.model_order = None;
        }
    }
    graph
}

#[derive(Clone, Copy, Debug)]
enum EntryPoint {
    Processor,
    Diagnostic,
}

type ExecutionTrace = (Vec<GraphExecution>, Option<HierarchySweepDebugTrace>);

fn run_controlled(
    entry: EntryPoint,
    graph: &mut LGraph,
    work: &mut dyn WorkControl,
) -> PipelineResult<ExecutionTrace> {
    match entry {
        EntryPoint::Processor => {
            execute_ported_compound_processors_until_processor_with_work_control(
                graph,
                ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
                work,
            )
            .map(|executions| (executions, None))
        }
        EntryPoint::Diagnostic => inspect_compound_crossings_after_processor_with_work_control(
            graph,
            ProcessorKind::SortByInputModelProcessor,
            work,
        ),
    }
}

fn run_unrestricted(entry: EntryPoint, graph: &mut LGraph) -> PipelineResult<ExecutionTrace> {
    match entry {
        EntryPoint::Processor => execute_ported_compound_processors_until_processor(
            graph,
            ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
        )
        .map(|executions| (executions, None)),
        EntryPoint::Diagnostic => inspect_compound_crossings_after_processor(
            graph,
            ProcessorKind::SortByInputModelProcessor,
        ),
    }
}

fn assert_public_scoring_boundary(entry: EntryPoint) {
    for nested in [false, true] {
        for node_orders in [false, true] {
            let graph = fan_in_graph(nested, node_orders);
            let mut expected = graph.clone();
            let expected_trace = run_unrestricted(entry, &mut expected).unwrap();

            let mut prepared = graph.clone();
            let mut prefix = ScoringControl::new(usize::MAX);
            execute_ported_compound_processors_until_processor_with_work_control(
                &mut prepared,
                ProcessorKind::SortByInputModelProcessor,
                &mut prefix,
            )
            .unwrap();

            let mut measured = ScoringControl::new(usize::MAX);
            let mut actual = graph.clone();
            assert_eq!(
                run_controlled(entry, &mut actual, &mut measured).unwrap(),
                expected_trace
            );
            assert_eq!(
                actual, expected,
                "{entry:?}, nested: {nested}, nodes: {node_orders}"
            );
            let sweep_index = prefix.charges.len();
            assert_eq!(&measured.charges[..sweep_index], prefix.charges.as_slice());
            assert!(measured.charges[sweep_index] > 0);
            let first_scoring_charge = (sweep_index + 1..measured.charges.len())
                .find(|index| measured.charges[*index] > 0)
                .expect("model-order scoring must reach the public control after sweep admission");
            let prepaid = prefix.charges.iter().sum::<usize>() + measured.charges[sweep_index];
            let required = usize::MAX - measured.remaining;
            assert!(required > prepaid);

            for budget in [prepaid, required - 1] {
                let mut limited = ScoringControl::new(budget);
                let mut rejected = graph.clone();
                assert_eq!(
                    run_controlled(entry, &mut rejected, &mut limited),
                    Err(PipelineError::Work(WorkError::Interrupted)),
                    "{entry:?}, nested: {nested}, nodes: {node_orders}, budget: {budget}"
                );
                if nested {
                    assert!(rejected.layerless_nodes[0].nested_graph.is_some());
                }
            }

            let interrupt_after = first_scoring_charge + 2;
            let mut interrupted = ScoringControl::new(usize::MAX);
            interrupted.interrupt_after = Some(interrupt_after);
            let mut rejected = graph.clone();
            assert_eq!(
                run_controlled(entry, &mut rejected, &mut interrupted),
                Err(PipelineError::Work(WorkError::Interrupted))
            );
            assert_eq!(interrupted.charges, measured.charges[..interrupt_after]);
            if nested {
                assert!(rejected.layerless_nodes[0].nested_graph.is_some());
            }

            let mut fresh = ScoringControl::new(required);
            let mut retry = graph;
            assert_eq!(
                run_controlled(entry, &mut retry, &mut fresh).unwrap(),
                expected_trace
            );
            assert_eq!(fresh.remaining, 0);
            assert_eq!(fresh.charges, measured.charges);
            assert_eq!(retry, expected);
        }
    }
}

#[test]
fn public_nonzero_influence_and_nested_sweeps_enforce_model_order_work() {
    assert_public_scoring_boundary(EntryPoint::Processor);
}

#[test]
fn public_nonzero_influence_and_nested_diagnostics_enforce_model_order_work() {
    assert_public_scoring_boundary(EntryPoint::Diagnostic);
}

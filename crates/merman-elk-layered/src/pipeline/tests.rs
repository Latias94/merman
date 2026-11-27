use super::plan::is_source_ported_processor;
use super::*;
use crate::graph::{LLabel, LNode, LNodeKind, LPort, Layer, PortSide, PortType};
use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputLabel, ElkInputNode, import_graph};
use crate::options::{
    CrossingMinimizationStrategy, CycleBreakingStrategy, EdgeRouting, ElkDirection, FixedAlignment,
    GreedySwitchType, LayerConstraint, LayeredOptions, LayeringStrategy, NodePlacementStrategy,
    OrderingStrategy, PortConstraints, WrappingStrategy,
};
use crate::p3order::{
    counting::CrossingsCounter, process_port_sides, sort_port_lists,
    sweep::minimize_crossings_layer_sweep_hierarchical_with_type,
};

fn kinds(options: &LayeredOptions) -> Vec<ProcessorKind> {
    assemble_processors(options)
        .into_iter()
        .map(|slot| slot.kind)
        .collect()
}

fn graph_kinds(graph: &LGraph) -> Vec<ProcessorKind> {
    assemble_processors_for_graph(graph)
        .into_iter()
        .map(|slot| slot.kind)
        .collect()
}

fn assert_processors_are_source_ported(case: &str, processors: Vec<ProcessorKind>) {
    let unsupported = processors
        .into_iter()
        .filter(|kind| !is_source_ported_processor(*kind))
        .collect::<Vec<_>>();

    assert!(
        unsupported.is_empty(),
        "{case} reached unported ELK processors: {unsupported:?}"
    );
}

fn node(id: &str) -> ElkInputNode {
    ElkInputNode {
        id: id.to_string(),
        width: 80.0,
        height: 40.0,
        parent: None,
        direction: None,
        hierarchy_handling: None,
        layer_constraint: None,
        port_alignment: None,
        port_constraints: None,
        node_label_placement: crate::options::NodeLabelPlacement::Fixed,
        node_flexibility: crate::options::NodeFlexibility::None,
        ports_surrounding: None,
        nested_options: None,
        label: None,
    }
}

fn edge(id: &str, source: &str, target: &str) -> ElkInputEdge {
    ElkInputEdge {
        id: id.to_string(),
        source: source.to_string(),
        target: target.to_string(),
        labels: Vec::new(),
        minlen: 1,
        inside_self_loops_yo: false,
        model_order: None,
        priority_direction: 0,
        priority_shortness: 0,
        priority_straightness: 0,
    }
}

fn p3_options() -> LayeredOptions {
    LayeredOptions {
        direction: ElkDirection::Right,
        greedy_switch_type: GreedySwitchType::Off,
        ..LayeredOptions::default()
    }
}

#[derive(Debug, Clone)]
struct BudgetWorkControl {
    remaining: usize,
    charged: usize,
    checks: Vec<usize>,
}

impl BudgetWorkControl {
    fn new(remaining: usize) -> Self {
        Self {
            remaining,
            charged: 0,
            checks: Vec::new(),
        }
    }
}

impl WorkControl for BudgetWorkControl {
    fn check(&mut self, units: usize) -> Result<(), WorkError> {
        self.checks.push(units);
        if units <= self.remaining {
            Ok(())
        } else {
            Err(WorkError::Interrupted)
        }
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.check(units)?;
        self.remaining -= units;
        self.charged += units;
        Ok(())
    }
}

fn bidirected_cycle_graph(node_count: usize) -> LGraph {
    let nodes = (0..node_count)
        .map(|index| node(&format!("n{index}")))
        .collect::<Vec<_>>();
    let mut edges = Vec::with_capacity(node_count * 2);
    for index in 0..node_count {
        let next = (index + 1) % node_count;
        edges.push(edge(
            &format!("forward-{index}"),
            &format!("n{index}"),
            &format!("n{next}"),
        ));
        edges.push(edge(
            &format!("backward-{index}"),
            &format!("n{next}"),
            &format!("n{index}"),
        ));
    }

    import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            consider_model_order_strategy: OrderingStrategy::NodesAndEdges,
            ..LayeredOptions::default()
        },
        nodes,
        edges,
    })
    .unwrap()
}

fn parallel_long_edge_graph(edge_count: usize) -> LGraph {
    disjoint_long_edge_graph(edge_count, edge_count + 1)
}

fn disjoint_long_edge_graph(edge_count: usize, target_layer: usize) -> LGraph {
    assert!(target_layer > 0);
    let mut nodes = Vec::with_capacity(edge_count * 2);
    let mut edges = Vec::with_capacity(edge_count);
    for index in 0..edge_count {
        nodes.push(node(&format!("source-{index}")));
        nodes.push(node(&format!("target-{index}")));
        edges.push(edge(
            &format!("edge-{index}"),
            &format!("source-{index}"),
            &format!("target-{index}"),
        ));
    }

    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::default(),
        nodes,
        edges,
    })
    .unwrap();
    graph.clear_layers();
    for index in 0..edge_count {
        graph.set_node_layer(index * 2, 0);
        graph.set_node_layer(index * 2 + 1, target_layer);
    }
    graph
}

fn shared_target_long_edge_graph_with_span(edge_count: usize, target_layer: usize) -> LGraph {
    assert!(target_layer > 0);
    let mut nodes = (0..edge_count)
        .map(|index| node(&format!("source-{index}")))
        .collect::<Vec<_>>();
    nodes.push(node("target"));
    let edges = (0..edge_count)
        .map(|index| {
            edge(
                &format!("edge-{index}"),
                &format!("source-{index}"),
                "target",
            )
        })
        .collect::<Vec<_>>();

    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            merge_edges: true,
            ..LayeredOptions::default()
        },
        nodes,
        edges,
    })
    .unwrap();
    graph.clear_layers();
    for source in 0..edge_count {
        graph.set_node_layer(source, 0);
    }
    graph.set_node_layer(edge_count, target_layer);
    graph
}

fn shared_port_cycle_graph(parallel_edge_count: usize) -> LGraph {
    let mut edges = Vec::with_capacity(parallel_edge_count * 2);
    for index in 0..parallel_edge_count {
        edges.push(edge(&format!("A-B-{index}"), "A", "B"));
        edges.push(edge(&format!("B-A-{index}"), "B", "A"));
    }
    import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            merge_edges: true,
            consider_model_order_strategy: OrderingStrategy::NodesAndEdges,
            ..LayeredOptions::default()
        },
        nodes: vec![node("A"), node("B")],
        edges,
    })
    .unwrap()
}

fn shared_port_dag_graph(parallel_edge_count: usize) -> LGraph {
    let edges = (0..parallel_edge_count)
        .map(|index| edge(&format!("A-B-{index}"), "A", "B"))
        .collect::<Vec<_>>();
    import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            merge_edges: true,
            ..LayeredOptions::default()
        },
        nodes: vec![node("A"), node("B")],
        edges,
    })
    .unwrap()
}

fn shared_port_dag_with_independent_edge(parallel_edge_count: usize) -> LGraph {
    let mut edges = (0..parallel_edge_count)
        .map(|index| edge(&format!("A-B-{index}"), "A", "B"))
        .collect::<Vec<_>>();
    edges.push(edge("C-D", "C", "D"));
    import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            merge_edges: true,
            consider_model_order_strategy: OrderingStrategy::NodesAndEdges,
            ..LayeredOptions::default()
        },
        nodes: vec![node("A"), node("B"), node("C"), node("D")],
        edges,
    })
    .unwrap()
}

fn dedicated_port_star_graph(edge_count: usize) -> LGraph {
    let mut nodes = vec![node("hub")];
    let mut edges = Vec::with_capacity(edge_count);
    for index in 0..edge_count {
        let target = format!("target-{index}");
        nodes.push(node(&target));
        edges.push(edge(&format!("edge-{index}"), "hub", &target));
    }
    import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::default(),
        nodes,
        edges,
    })
    .unwrap()
}

fn one_edge_collector_graph(source_id: &str) -> LGraph {
    import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            merge_edges: true,
            ..LayeredOptions::default()
        },
        nodes: vec![node(source_id), node("target")],
        edges: vec![edge("edge", source_id, "target")],
    })
    .unwrap()
}

fn deep_compound_chain(graph_count: usize) -> LGraph {
    assert!(graph_count > 0);
    let options = LayeredOptions {
        hierarchy_handling: crate::options::HierarchyHandling::IncludeChildren,
        greedy_switch_type: GreedySwitchType::Off,
        greedy_switch_hierarchical_type: GreedySwitchType::Off,
        ..LayeredOptions::default()
    };
    let mut nested = None;
    for depth in (0..graph_count).rev() {
        let mut graph = LGraph::new(format!("graph-{depth}"), options.clone());
        if depth > 0 {
            graph.parent_node_id = Some(format!("node-{}", depth - 1));
        }
        if let Some(child) = nested.take() {
            let mut holder = LNode::new(format!("node-{depth}"), 1.0, 1.0, None);
            holder.compound = true;
            holder.nested_graph = Some(child);
            graph.layerless_nodes.push(holder);
        }
        nested = Some(Box::new(graph));
    }
    *nested.expect("a positive graph count creates the root graph")
}

fn branched_compound_graph() -> LGraph {
    let options = LayeredOptions {
        hierarchy_handling: crate::options::HierarchyHandling::IncludeChildren,
        greedy_switch_type: GreedySwitchType::Off,
        greedy_switch_hierarchical_type: GreedySwitchType::Off,
        ..LayeredOptions::default()
    };

    let mut first_grandchild = LGraph::new("first-grandchild", options.clone());
    first_grandchild.parent_node_id = Some("first-grandchild-holder".to_string());
    let mut first_child = LGraph::new("first-child", options.clone());
    first_child.parent_node_id = Some("first-holder".to_string());
    let mut first_grandchild_holder = LNode::new("first-grandchild-holder", 1.0, 1.0, None);
    first_grandchild_holder.compound = true;
    first_grandchild_holder.nested_graph = Some(Box::new(first_grandchild));
    first_child.layerless_nodes.push(first_grandchild_holder);

    let mut second_grandchild = LGraph::new("second-grandchild", options.clone());
    second_grandchild.parent_node_id = Some("second-grandchild-holder".to_string());
    let mut second_child = LGraph::new("second-child", options.clone());
    second_child.parent_node_id = Some("second-holder".to_string());
    let mut second_grandchild_holder = LNode::new("second-grandchild-holder", 1.0, 1.0, None);
    second_grandchild_holder.compound = true;
    second_grandchild_holder.nested_graph = Some(Box::new(second_grandchild));
    second_child.layerless_nodes.push(second_grandchild_holder);

    let mut root = LGraph::new("root", options);
    let mut first_holder = LNode::new("first-holder", 1.0, 1.0, None);
    first_holder.compound = true;
    first_holder.nested_graph = Some(Box::new(first_child));
    let mut second_holder = LNode::new("second-holder", 1.0, 1.0, None);
    second_holder.compound = true;
    second_holder.nested_graph = Some(Box::new(second_child));
    root.layerless_nodes.extend([first_holder, second_holder]);
    root
}

fn high_thoroughness_diagnostic_graph() -> LGraph {
    let mut options = LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down);
    options.thoroughness = 64;
    import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options,
        nodes: vec![
            node("top-a"),
            node("top-b"),
            node("bottom-a"),
            node("bottom-b"),
        ],
        // K2,2 has an unavoidable crossing, so the debug barycenter sweep cannot terminate
        // early at zero and must execute every configured thoroughness run.
        edges: vec![
            edge("aa", "top-a", "bottom-a"),
            edge("ab", "top-a", "bottom-b"),
            edge("ba", "top-b", "bottom-a"),
            edge("bb", "top-b", "bottom-b"),
        ],
    })
    .unwrap()
}

fn assert_processor_budget_boundaries(graph: &LGraph, kind: ProcessorKind) {
    assert_processor_budget_boundaries_with_mutation(graph, kind, true);
}

fn assert_noop_processor_budget_boundaries(graph: &LGraph, kind: ProcessorKind) {
    assert_processor_budget_boundaries_with_mutation(graph, kind, false);
}

fn assert_processor_budget_boundaries_with_mutation(
    graph: &LGraph,
    kind: ProcessorKind,
    expect_mutation: bool,
) {
    let required = processor_work_units(graph, kind).unwrap();
    assert!(required > 0);

    let mut below_graph = graph.clone();
    let mut below = BudgetWorkControl::new(required - 1);
    assert_eq!(
        execute_processor_with_work_control(&mut below_graph, kind, &mut below),
        Err(PipelineError::Work(WorkError::Interrupted))
    );
    assert_eq!(below_graph, *graph, "rejection must precede mutation");
    assert_eq!(below.charged, 0);
    assert_eq!(below.remaining, required - 1);

    let mut exact_graph = graph.clone();
    let mut exact = BudgetWorkControl::new(required);
    execute_processor_with_work_control(&mut exact_graph, kind, &mut exact).unwrap();
    assert_eq!(
        exact_graph != *graph,
        expect_mutation,
        "{kind:?} boundary fixture mutation expectation changed"
    );
    assert_eq!(exact.charged, required);
    assert_eq!(exact.remaining, 0);

    let mut above_graph = graph.clone();
    let mut above = BudgetWorkControl::new(required + 1);
    execute_processor_with_work_control(&mut above_graph, kind, &mut above).unwrap();
    assert_eq!(
        above_graph, exact_graph,
        "budget headroom changed semantics"
    );
    assert_eq!(above.charged, required);
    assert_eq!(above.remaining, 1);
}

#[test]
fn superlinear_processors_enforce_transactional_budget_boundaries() {
    let cycle_graph = bidirected_cycle_graph(4);
    for kind in [
        ProcessorKind::GreedyCycleBreaker,
        ProcessorKind::GreedyModelOrderCycleBreaker,
        ProcessorKind::DepthFirstCycleBreaker,
        ProcessorKind::InteractiveCycleBreaker,
        ProcessorKind::ModelOrderCycleBreaker,
    ] {
        assert_processor_budget_boundaries(&cycle_graph, kind);
    }

    let long_edge_graph = parallel_long_edge_graph(3);
    assert_processor_budget_boundaries(&long_edge_graph, ProcessorKind::LongEdgeSplitter);

    let mut joined_graph = disjoint_long_edge_graph(3, 2);
    split_long_edges(&mut joined_graph);
    assert_processor_budget_boundaries(&joined_graph, ProcessorKind::LongEdgeJoiner);

    let mut shared_target_graph = shared_target_long_edge_graph_with_span(32, 2);
    split_long_edges(&mut shared_target_graph);
    assert_processor_budget_boundaries(&shared_target_graph, ProcessorKind::LongEdgeJoiner);
}

#[test]
fn flexible_node_placement_rejects_insufficient_work_before_mutation() {
    let mut graph = high_thoroughness_diagnostic_graph();
    graph.options.node_placement_strategy = NodePlacementStrategy::NetworkSimplex;
    execute_processors_until(&mut graph, LayeredPhase::P3NodeOrdering).unwrap();
    for node in &mut graph.layerless_nodes {
        node.node_flexibility = crate::NodeFlexibility::PortPosition;
        node.size.height = 160.0;
    }
    assert_processor_budget_boundaries(&graph, ProcessorKind::NetworkSimplexPlacer);
}

#[test]
fn long_edge_joiner_work_is_linear_in_disjoint_segment_count() {
    for edge_count in [1usize, 8, 32, 128] {
        let mut graph = disjoint_long_edge_graph(edge_count, 2);
        split_long_edges(&mut graph);
        let base = local_graph_work_units(&graph).unwrap();

        // Each edge creates one two-port dummy and one pair. The preflight and layer rebuild
        // visit three nodes per edge, while the global port/adjacency index is built once.
        assert_eq!(
            long_edge_joiner_work_units(&graph),
            Ok(base + 26 * edge_count)
        );
        assert_eq!(
            processor_work_units(&graph, ProcessorKind::LongEdgeJoiner),
            long_edge_joiner_work_units(&graph)
        );
    }
}

#[test]
fn long_edge_joiner_work_is_linear_for_shared_target_fanin() {
    for edge_count in [1usize, 8, 32, 128, 512] {
        let mut graph = shared_target_long_edge_graph_with_span(edge_count, 2);
        split_long_edges(&mut graph);
        let base = local_graph_work_units(&graph).unwrap();

        // The collector's incoming adjacency is indexed once for the whole graph. It must not
        // be multiplied by the number of long-edge dummies that share that target port.
        assert_eq!(
            long_edge_joiner_work_units(&graph),
            Ok(base + 23 * edge_count + 3)
        );
    }
}

#[test]
fn long_edge_joiner_work_skips_index_cost_without_dummies() {
    let graph = disjoint_long_edge_graph(32, 1);
    let base = local_graph_work_units(&graph).unwrap();
    let layer_memberships = graph
        .layers
        .iter()
        .map(|layer| layer.nodes.len())
        .sum::<usize>();

    assert_eq!(
        long_edge_joiner_work_units(&graph),
        Ok(base + layer_memberships)
    );
}

#[test]
fn compound_preflight_charges_hierarchy_label_payload_before_materialization() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph.hierarchy_edges.push(crate::graph::HierarchyEdge {
        id: "ab".into(),
        source_node_id: "A".into(),
        target_node_id: "B".into(),
        source_port_key: "A:source".into(),
        target_port_key: "B:target".into(),
        source_path: vec!["group".into()],
        target_path: Vec::new(),
        labels: Vec::new(),
        minlen: 1,
        model_order: None,
        priority_direction: 0,
        priority_shortness: 0,
        priority_straightness: 0,
    });
    let base = local_compound_preprocess_work_units(&graph).unwrap();
    for count in [1, 128, 4096] {
        graph.hierarchy_edges[0].labels = (0..count)
            .map(|index| crate::graph::LLabel::new(index.to_string(), 12.0, 10.0))
            .collect();
        let required = local_compound_preprocess_work_units(&graph).unwrap();
        assert_eq!(required, base + count);
        let mut below = BudgetWorkControl::new(required - 1);
        assert_eq!(
            charge_compound_preprocess_work(&graph, &mut below),
            Err(PipelineError::Work(WorkError::Interrupted))
        );
        assert_eq!(below.charged, 0);
        let mut exact = BudgetWorkControl::new(required);
        charge_compound_preprocess_work(&graph, &mut exact).unwrap();
        assert_eq!(exact.charged, required);
    }
}

#[test]
fn end_label_sorter_work_sums_owner_local_label_sorts() {
    let expected_sort_work = [(1usize, 0usize), (8, 24), (16, 64), (32, 160)];
    for (label_count, sort_work) in expected_sort_work {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        let mut node = LNode::new("node", 10.0, 10.0, Some(0));
        let mut port = LPort::new("port", 0, PortType::Output);
        port.labels = (0..label_count)
            .map(|index| LLabel::new(format!("label-{index}"), 1.0, 1.0))
            .collect();
        node.ports.push(port);
        graph.layerless_nodes.push(node);

        let base = local_graph_work_units(&graph).unwrap();
        assert_eq!(
            end_label_sorter_work_units(&graph),
            Ok(base + label_count + sort_work)
        );
        assert_eq!(
            processor_work_units(&graph, ProcessorKind::EndLabelSorter),
            end_label_sorter_work_units(&graph)
        );
    }

    let mut graph = LGraph::new("root", LayeredOptions::default());
    let mut node = LNode::new("node", 10.0, 10.0, Some(0));
    for port_index in 0..2 {
        let mut port = LPort::new(format!("port-{port_index}"), 0, PortType::Output);
        port.labels = (0..8)
            .map(|label_index| LLabel::new(format!("label-{port_index}-{label_index}"), 1.0, 1.0))
            .collect();
        node.ports.push(port);
    }
    graph.layerless_nodes.push(node);
    let base = local_graph_work_units(&graph).unwrap();
    // Two local 8-item sorts cost 2*24, not one global 16-item sort (64).
    assert_eq!(end_label_sorter_work_units(&graph), Ok(base + 16 + 48));
}

#[test]
fn port_model_order_processors_charge_only_owner_local_amplification() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    let mut node = LNode::new("node", 10.0, 10.0, Some(0));
    node.ports.push(LPort::new("port-0", 0, PortType::Output));
    node.ports.push(LPort::new("port-1", 0, PortType::Output));
    graph.layerless_nodes.push(node);
    graph.layers.push(Layer {
        nodes: vec![0],
        ..Layer::default()
    });

    // One node, two ports, and one layer give an independently countable base of four.
    assert_eq!(local_graph_work_units(&graph), Ok(4));
    assert_eq!(hierarchy_work_units(&graph), Ok(4));

    // Free ports make PortListSorter visit the owner node without sorting or rewriting it.
    assert_eq!(port_list_sorter_work_units(&graph), Ok(4 + 1));
    assert_eq!(sort_by_input_model_work_units(&graph), Ok(4 + 5 + 4 + 8));
    assert_eq!(
        processor_work_units(&graph, ProcessorKind::PortListSorter),
        Ok(5)
    );
    assert_eq!(
        processor_work_units(&graph, ProcessorKind::SortByInputModelProcessor),
        Ok(21)
    );

    graph.layerless_nodes[0].port_constraints = PortConstraints::FixedOrder;
    assert_eq!(port_list_sorter_work_units(&graph), Ok(4 + 1 + 2 + 2 + 8));
    assert_eq!(sort_by_input_model_work_units(&graph), Ok(4 + 5));
}

#[test]
fn fixed_side_identity_keeps_the_local_three_and_four_port_bounds() {
    for (count, expected) in [(3, 39), (4, 91)] {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        graph.options.port_sorting_strategy = crate::options::PortSortingStrategy::InputOrder;
        let mut node = LNode::new("owner", 10.0, 10.0, Some(0));
        node.port_constraints = PortConstraints::FixedSide;
        for index in 0..count {
            let mut port = LPort::new(format!("north-{index}"), 0, PortType::Output);
            port.side = PortSide::North;
            node.ports.push(port);
        }
        graph.layerless_nodes.push(node);
        graph.layers.push(Layer {
            nodes: vec![0],
            ..Layer::default()
        });
        let before = graph.clone();
        assert_eq!(port_list_sorter_work_units(&graph), Ok(expected));
        let mut below = BudgetWorkControl::new(expected - 1);
        assert_eq!(
            execute_processor_with_work_control(
                &mut graph,
                ProcessorKind::PortListSorter,
                &mut below
            ),
            Err(PipelineError::Work(WorkError::Interrupted))
        );
        assert_eq!(graph, before);
        let mut exact = BudgetWorkControl::new(expected);
        execute_processor_with_work_control(&mut graph, ProcessorKind::PortListSorter, &mut exact)
            .unwrap();
        assert_eq!(exact.charged, expected);
        assert_eq!(graph, before);
    }
}

#[test]
fn model_order_sort_dynamic_budget_preserves_order_and_stops_before_failed_permutation() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    let mut node = LNode::new("node", 10.0, 10.0, Some(0));
    node.ports.push(LPort::new("west", 0, PortType::Input));
    node.ports.push(LPort::new("east", 0, PortType::Output));
    node.ports[0].side = PortSide::West;
    node.ports[1].side = PortSide::East;
    graph.layerless_nodes.push(node);
    graph.layers.push(Layer {
        nodes: vec![0],
        ..Layer::default()
    });
    let kind = ProcessorKind::SortByInputModelProcessor;
    let floor = processor_work_units(&graph, kind).unwrap();
    let mut measured = BudgetWorkControl::new(usize::MAX);
    let mut expected = graph.clone();
    execute_processor_with_work_control(&mut expected, kind, &mut measured).unwrap();
    assert!(measured.charged > floor);
    assert_eq!(expected.layerless_nodes[0].ports[0].id, "east");
    let mut insufficient = BudgetWorkControl::new(measured.charged - 1);
    let mut interrupted = graph.clone();
    assert_eq!(
        execute_processor_with_work_control(&mut interrupted, kind, &mut insufficient),
        Err(PipelineError::Work(WorkError::Interrupted))
    );
    assert_eq!(
        interrupted, graph,
        "a rejected comparison must not install its port permutation"
    );
    let mut exact = BudgetWorkControl::new(measured.charged);
    execute_processor_with_work_control(&mut graph, kind, &mut exact).unwrap();
    assert_eq!(graph, expected);
    assert_eq!(exact.remaining, 0);
}

#[test]
fn barycenter_budget_tracks_executed_sweeps_and_preserves_seeded_order() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Right),
        nodes: vec![node("a"), node("b"), node("c"), node("d")],
        edges: vec![edge("ad", "a", "d"), edge("bc", "b", "c")],
    })
    .unwrap();
    layer_network_simplex(&mut graph).unwrap();
    process_port_sides(&mut graph);
    sort_port_lists(&mut graph);
    graph.layers[0].nodes = vec![0, 1];
    graph.layers[1].nodes = vec![2, 3];
    let kind = ProcessorKind::LayerSweepCrossingMinimizerBarycenter;
    let floor = processor_work_units(&graph, kind).unwrap();
    let mut denied = graph.clone();
    let mut below_floor = BudgetWorkControl::new(floor - 1);
    assert_eq!(
        execute_processor_with_work_control(&mut denied, kind, &mut below_floor),
        Err(PipelineError::Work(WorkError::Interrupted))
    );
    assert_eq!(
        denied, graph,
        "the scan floor must reject before port-ID initialization"
    );

    let mut expected = graph.clone();
    execute_processor(&mut expected, kind).unwrap();
    assert_eq!(
        crate::p3order::counting::CrossingsCounter::new().count_all_crossings(&expected),
        0
    );
    let mut measured = BudgetWorkControl::new(usize::MAX);
    let mut actual = graph.clone();
    execute_processor_with_work_control(&mut actual, kind, &mut measured).unwrap();
    assert_eq!(
        actual, expected,
        "work control must preserve node/port orders and RNG state"
    );
    assert!(measured.charged > floor);
    let mut insufficient = BudgetWorkControl::new(measured.charged - 1);
    assert_eq!(
        execute_processor_with_work_control(&mut graph.clone(), kind, &mut insufficient),
        Err(PipelineError::Work(WorkError::Interrupted))
    );
    let mut exact = BudgetWorkControl::new(measured.charged);
    execute_processor_with_work_control(&mut graph, kind, &mut exact).unwrap();
    assert_eq!(graph, expected);
    assert_eq!(exact.remaining, 0);

    // An initially crossing-free model order stops after its first try regardless of
    // thoroughness. No unexecuted attempts or sweeps should consume a work budget.
    let mut low = expected.clone();
    low.options.thoroughness = 1;
    let mut high = low.clone();
    high.options.thoroughness = 100;
    let mut low_budget = BudgetWorkControl::new(usize::MAX);
    let mut high_budget = BudgetWorkControl::new(usize::MAX);
    execute_processor_with_work_control(&mut low, kind, &mut low_budget).unwrap();
    execute_processor_with_work_control(&mut high, kind, &mut high_budget).unwrap();
    assert_eq!(low_budget.charged, high_budget.charged);
    high.options.thoroughness = 1;
    assert_eq!(low, high);
}

#[test]
fn barycenter_high_degree_early_return_charges_linear_port_restore() {
    const DEGREE: usize = 1024;
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            merge_edges: false,
            ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Right)
        },
        nodes: vec![node("a"), node("b")],
        edges: (0..DEGREE)
            .map(|index| edge(&format!("ab-{index}"), "a", "b"))
            .collect(),
    })
    .unwrap();
    graph.set_node_layer(0, 0);
    graph.set_node_layer(1, 1);
    process_port_sides(&mut graph);
    sort_port_lists(&mut graph);
    assert!(
        graph
            .layerless_nodes
            .iter()
            .all(|node| node.ports.len() == DEGREE)
    );
    // Establish a crossing-free fixture independently of PortListSorter:
    // west ports run bottom-to-top, opposite the east source-port order.
    let mut target_order = (0..DEGREE).collect::<Vec<_>>();
    target_order.sort_by_key(|&port| {
        std::cmp::Reverse(graph.layerless_nodes[1].ports[port].incoming_edges[0])
    });
    graph.reorder_node_ports(1, target_order);
    assert_eq!(CrossingsCounter::new().count_all_crossings(&graph), 0);
    let context = ReorderNodePortsWorkContext::new(&graph).unwrap();
    let minimum_restore_work = 8 * DEGREE * 2
        + context.node_work(&graph, 0).unwrap()
        + context.node_work(&graph, 1).unwrap();
    let kind = ProcessorKind::LayerSweepCrossingMinimizerBarycenter;
    let mut measured = BudgetWorkControl::new(usize::MAX);
    let mut result = graph.clone();
    execute_processor_with_work_control(&mut result, kind, &mut measured).unwrap();
    assert!(measured.checks.last().copied().unwrap() >= minimum_restore_work);
    assert_eq!(CrossingsCounter::new().count_all_crossings(&result), 0);
    assert_eq!(graph.edges, result.edges);
    let mut insufficient = BudgetWorkControl::new(measured.charged - 1);
    assert_eq!(
        execute_processor_with_work_control(&mut graph.clone(), kind, &mut insufficient),
        Err(PipelineError::Work(WorkError::Interrupted))
    );
    let mut exact = BudgetWorkControl::new(measured.charged);
    execute_processor_with_work_control(&mut graph, kind, &mut exact).unwrap();
    assert_eq!(graph, result);
    assert_eq!(exact.remaining, 0);
}

#[test]
fn prepared_port_reorder_work_matches_the_per_owner_reference() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    for node_index in 0..16 {
        let mut node = LNode::new(format!("node-{node_index}"), 10.0, 10.0, Some(node_index));
        node.port_constraints = PortConstraints::FixedOrder;
        node.ports.push(LPort::new(
            format!("port-{node_index}-0"),
            node_index,
            PortType::Output,
        ));
        node.ports.push(LPort::new(
            format!("port-{node_index}-1"),
            node_index,
            PortType::Output,
        ));
        graph.layerless_nodes.push(node);
    }
    graph.layers.push(Layer {
        nodes: (0..graph.layerless_nodes.len()).collect(),
        ..Layer::default()
    });
    graph.layerless_nodes[0].nested_graph = Some(Box::new(deep_compound_chain(64)));

    let context = ReorderNodePortsWorkContext::new(&graph).unwrap();
    for node_index in 0..graph.layerless_nodes.len() {
        assert_eq!(
            context.node_work(&graph, node_index),
            unprepared_reorder_node_ports_work_units(&graph, node_index)
        );
    }
}

#[test]
fn single_graph_configuration_preflights_the_complete_nested_hierarchy() {
    let graph = deep_compound_chain(32);
    let required = hierarchy_work_units(&graph).unwrap();

    for runner in 0..3 {
        let mut below_graph = graph.clone();
        let before = below_graph.clone();
        let mut below = BudgetWorkControl::new(required - 1);
        let result = match runner {
            0 => execute_processors_until_with_work_control(
                &mut below_graph,
                LayeredPhase::P1CycleBreaking,
                &mut below,
            )
            .map(|_| ()),
            1 => execute_processors_until_processor_with_work_control(
                &mut below_graph,
                ProcessorKind::PortListSorter,
                &mut below,
            )
            .map(|_| ()),
            _ => execute_ported_processors_with_work_control(&mut below_graph, &mut below)
                .map(|_| ()),
        };
        assert_eq!(result, Err(PipelineError::Work(WorkError::Interrupted)));
        assert_eq!(below_graph, before);
        assert_eq!(below.charged, 0);
        assert_eq!(below.remaining, required - 1);
    }

    let mut exact_graph = graph.clone();
    let mut exact = BudgetWorkControl::new(required);
    let exact_processors = prepare_single_graph_processors(&mut exact_graph, &mut exact).unwrap();
    assert_eq!(exact.charged, required);
    assert_eq!(exact.remaining, 0);

    let mut above_graph = graph;
    let mut above = BudgetWorkControl::new(required + 1);
    let above_processors = prepare_single_graph_processors(&mut above_graph, &mut above).unwrap();
    assert_eq!(above_graph, exact_graph);
    assert_eq!(above_processors, exact_processors);
    assert_eq!(above.charged, required);
    assert_eq!(above.remaining, 1);
}

#[test]
fn greedy_cycle_breaker_work_curve_covers_quadratic_candidate_scans() {
    let mut previous_quadratic = None;
    for node_count in [8usize, 16, 32, 64] {
        let graph = bidirected_cycle_graph(node_count);
        let (ports, adjacency) = graph_port_and_adjacency_work_units(&graph).unwrap();
        let linear = checked_mul(
            checked_sum([node_count, ports, adjacency, graph.edges.len()]).unwrap(),
            4,
        )
        .unwrap();
        let work = greedy_cycle_breaker_work_units(&graph).unwrap();
        let quadratic = work - linear - edge_reversal_work_units(&graph).unwrap();

        assert_eq!(quadratic, 3 * node_count * node_count);
        for kind in [
            ProcessorKind::GreedyCycleBreaker,
            ProcessorKind::GreedyModelOrderCycleBreaker,
        ] {
            assert_eq!(
                processor_work_units(&graph, kind),
                greedy_cycle_breaker_work_units(&graph)
            );
        }
        if let Some(previous) = previous_quadratic {
            assert_eq!(quadratic, previous * 4);
        }
        previous_quadratic = Some(quadratic);
    }
}

#[test]
fn greedy_cycle_breaker_work_curve_tracks_shared_port_removal_amplification() {
    for parallel_edge_count in [8usize, 16, 32, 64] {
        let graph = shared_port_cycle_graph(parallel_edge_count);
        let adjacency_work = adapted_edge_reversal_structure_work_units(&graph)
            .unwrap()
            .adjacency;

        assert_eq!(
            adjacency_work,
            16 * parallel_edge_count * parallel_edge_count
        );
    }
}

#[test]
fn adapted_reversal_skips_collector_lookup_for_dedicated_ports() {
    for edge_count in [8usize, 16, 32, 64] {
        let graph = dedicated_port_star_graph(edge_count);
        assert!(
            graph
                .layerless_nodes
                .iter()
                .flat_map(|node| &node.ports)
                .all(|port| port.collector_type.is_none())
        );

        let work = adapted_edge_reversal_work_units(&graph, 1).unwrap();
        // Each dedicated endpoint has degree one: four adjacency units plus three payload
        // units per edge, with no node-wide collector lookup or materialization.
        assert_eq!(work, 7 * edge_count);
    }
}

#[test]
fn collector_materialization_accounts_for_complete_node_id_once() {
    let short = one_edge_collector_graph("s");
    let long_id = format!("s{}", "x".repeat(4_096));
    let long = one_edge_collector_graph(&long_id);

    let short_work = adapted_edge_reversal_work_units(&short, 2).unwrap();
    let long_work = adapted_edge_reversal_work_units(&long, 2).unwrap();
    assert_eq!(long_work - short_work, 4_096);

    // Interactive can reverse an edge in both ordered batches, but each missing opposite
    // collector is appended at most once and then reused.
    let one_pass = adapted_edge_reversal_work_units(&short, 1).unwrap();
    let two_passes = adapted_edge_reversal_work_units(&short, 2).unwrap();
    let materialization =
        collector_materialization_work_units(&short.layerless_nodes[0], PortType::Input).unwrap()
            + collector_materialization_work_units(&short.layerless_nodes[1], PortType::Output)
                .unwrap();
    assert_eq!(two_passes, 2 * one_pass - materialization);
}

#[test]
fn constraint_irrelevant_direct_sides_skip_mutation_bound() {
    for (node, constraint, fixed_side) in [
        (0, crate::options::LayerConstraint::First, PortSide::West),
        (1, crate::options::LayerConstraint::Last, PortSide::East),
    ] {
        let graph = &mut shared_port_dag_graph(64);
        graph.layerless_nodes[node].layer_constraint = constraint;
        graph.layerless_nodes[node].port_constraints = PortConstraints::FixedSide;
        for port in &mut graph.layerless_nodes[node].ports {
            port.set_side(fixed_side);
        }

        assert!(!edge_and_layer_constraint_reversal_may_mutate(graph));
        let base = local_graph_work_units(graph).unwrap();
        let (ports, incoming, outgoing) =
            graph_port_and_directional_adjacency_work_units(graph).unwrap();
        assert_eq!(
            processor_work_units(graph, ProcessorKind::EdgeAndLayerConstraintEdgeReverser),
            Ok(base + 3 * graph.layerless_nodes.len() + 2 * ports + 4 * (incoming + outgoing))
        );
        assert_noop_processor_budget_boundaries(
            graph,
            ProcessorKind::EdgeAndLayerConstraintEdgeReverser,
        );
    }
}

#[test]
fn non_greedy_cycle_breakers_skip_mutation_bound_on_monotonic_dag() {
    let graph = shared_port_dag_graph(64);
    let base = local_graph_work_units(&graph).unwrap();
    let (ports, incoming, outgoing) =
        graph_port_and_directional_adjacency_work_units(&graph).unwrap();
    let nodes = graph.layerless_nodes.len();

    for (kind, expected) in [
        (
            ProcessorKind::DepthFirstCycleBreaker,
            base + 5 * nodes + 2 * ports + incoming + 3 * outgoing,
        ),
        (
            ProcessorKind::InteractiveCycleBreaker,
            base + 4 * nodes + 2 * ports + 6 * outgoing,
        ),
        (
            ProcessorKind::ModelOrderCycleBreaker,
            base + 2 * nodes + ports + 4 * outgoing,
        ),
    ] {
        assert_eq!(processor_work_units(&graph, kind), Ok(expected));
        assert_noop_processor_budget_boundaries(&graph, kind);
    }
}

#[test]
fn edge_reversal_work_is_independent_of_vec_capacity() {
    let compact = shared_port_cycle_graph(8);
    let mut reserved = compact.clone();
    reserved.layerless_nodes.reserve(128);
    reserved.edges.reserve(128);
    reserved.layers.reserve(128);
    for node in &mut reserved.layerless_nodes {
        node.ports.reserve(128);
        node.labels.reserve(128);
        for port in &mut node.ports {
            port.incoming_edges.reserve(128);
            port.outgoing_edges.reserve(128);
        }
    }
    for edge in &mut reserved.edges {
        edge.labels.reserve(128);
        edge.bend_points.reserve(128);
    }

    assert_eq!(reserved, compact);
    for kind in [
        ProcessorKind::DepthFirstCycleBreaker,
        ProcessorKind::InteractiveCycleBreaker,
        ProcessorKind::ModelOrderCycleBreaker,
        ProcessorKind::EdgeAndLayerConstraintEdgeReverser,
        ProcessorKind::ReversedEdgeRestorer,
    ] {
        assert_eq!(
            processor_work_units(&reserved, kind),
            processor_work_units(&compact, kind),
            "{kind:?} exposed allocation history through the public budget"
        );
    }
}

#[test]
fn constraint_and_restorer_skip_quadratic_mutation_on_normal_collector_dags() {
    let mut previous_constraint = None;
    let mut previous_restorer = None;
    for edge_count in [8usize, 16, 32, 64] {
        let mut graph = shared_port_dag_graph(edge_count);
        graph.set_node_layer(0, 0);
        graph.set_node_layer(1, 1);

        assert!(!edge_and_layer_constraint_reversal_may_mutate(&graph));
        assert_eq!(marked_edge_restoration_work_units(&graph), Ok(0));
        assert!(
            adapted_edge_reversal_structure_work_units(&graph)
                .unwrap()
                .adjacency
                >= 4 * edge_count.pow(2)
        );

        let constraint =
            processor_work_units(&graph, ProcessorKind::EdgeAndLayerConstraintEdgeReverser)
                .unwrap();
        let restorer = processor_work_units(&graph, ProcessorKind::ReversedEdgeRestorer).unwrap();
        if let Some(previous) = previous_constraint {
            assert!(constraint < previous * 3);
        }
        if let Some(previous) = previous_restorer {
            assert!(restorer < previous * 3);
        }
        previous_constraint = Some(constraint);
        previous_restorer = Some(restorer);

        assert_noop_processor_budget_boundaries(
            &graph,
            ProcessorKind::EdgeAndLayerConstraintEdgeReverser,
        );
        assert_noop_processor_budget_boundaries(&graph, ProcessorKind::ReversedEdgeRestorer);
    }
}

#[test]
fn restorer_mutation_tracks_only_marked_edges() {
    for parallel_edge_count in [8usize, 16, 32, 64] {
        let mut graph = shared_port_cycle_graph(parallel_edge_count);
        graph.set_node_layer(0, 0);
        graph.set_node_layer(1, 1);
        for edge in &mut graph.edges {
            edge.reversed = true;
        }

        let mutation = marked_edge_restoration_work_units(&graph).unwrap();
        assert_eq!(
            mutation,
            8 * parallel_edge_count * parallel_edge_count + 6 * parallel_edge_count
        );
    }
}

#[test]
fn restorer_counts_marked_incoming_and_outgoing_lists_separately() {
    let parallel_edge_count = 64usize;
    let mut graph = shared_port_cycle_graph(parallel_edge_count);
    graph.set_node_layer(0, 0);
    graph.set_node_layer(1, 1);

    let input_port = graph.layerless_nodes[0]
        .ports
        .iter()
        .position(|port| port.collector_type == Some(PortType::Input))
        .unwrap();
    let output_port = graph.layerless_nodes[0]
        .ports
        .iter()
        .position(|port| port.collector_type == Some(PortType::Output))
        .unwrap();
    let marked_edges =
        std::mem::take(&mut graph.layerless_nodes[0].ports[output_port].outgoing_edges);
    for &edge in &marked_edges {
        graph.edges[edge].source.port = input_port;
        graph.edges[edge].reversed = true;
    }
    graph.layerless_nodes[0].ports[input_port].outgoing_edges = marked_edges;

    assert_eq!(
        marked_edge_restoration_work_units(&graph),
        Ok(4 * parallel_edge_count * parallel_edge_count + 3 * parallel_edge_count)
    );
    assert_processor_budget_boundaries(&graph, ProcessorKind::ReversedEdgeRestorer);
}

#[test]
fn restorer_counts_sparse_marks_against_the_complete_shared_lists() {
    for parallel_edge_count in [8usize, 16, 32, 64] {
        let mut graph = shared_port_dag_graph(parallel_edge_count);
        graph.set_node_layer(0, 0);
        graph.set_node_layer(1, 1);
        graph.edges[0].reversed = true;

        assert_eq!(
            marked_edge_restoration_work_units(&graph),
            Ok(4 * parallel_edge_count + 3)
        );
        assert_processor_budget_boundaries(&graph, ProcessorKind::ReversedEdgeRestorer);
    }
}

#[test]
fn mutating_constraint_and_restorer_honor_exact_budget_boundaries() {
    let mut constraint_graph = shared_port_dag_graph(8);
    constraint_graph.layerless_nodes[0].layer_constraint = crate::options::LayerConstraint::Last;
    assert!(edge_and_layer_constraint_reversal_may_mutate(
        &constraint_graph
    ));
    assert_processor_budget_boundaries(
        &constraint_graph,
        ProcessorKind::EdgeAndLayerConstraintEdgeReverser,
    );

    let mut restorer_graph = shared_port_cycle_graph(8);
    restorer_graph.set_node_layer(0, 0);
    restorer_graph.set_node_layer(1, 1);
    for edge in &mut restorer_graph.edges {
        edge.reversed = true;
    }
    assert!(marked_edge_restoration_work_units(&restorer_graph).unwrap() > 0);
    assert_processor_budget_boundaries(&restorer_graph, ProcessorKind::ReversedEdgeRestorer);
}

#[test]
fn non_greedy_reversal_processors_use_owner_specific_work_shapes() {
    for parallel_edge_count in [8usize, 16, 32, 64] {
        let graph = shared_port_cycle_graph(parallel_edge_count);
        let base = local_graph_work_units(&graph).unwrap();
        let (ports, incoming, outgoing) =
            graph_port_and_directional_adjacency_work_units(&graph).unwrap();
        let nodes = graph.layerless_nodes.len();
        let adjacency = incoming + outgoing;
        let adapted_once = adapted_edge_reversal_work_units(&graph, 1).unwrap();

        assert_eq!(
            processor_work_units(&graph, ProcessorKind::DepthFirstCycleBreaker),
            Ok(base + 5 * nodes + 2 * ports + incoming + 3 * outgoing + adapted_once)
        );
        assert_eq!(
            processor_work_units(&graph, ProcessorKind::InteractiveCycleBreaker),
            Ok(base
                + 4 * nodes
                + 2 * ports
                + 6 * outgoing
                + adapted_edge_reversal_work_units(&graph, 2).unwrap())
        );
        assert_eq!(
            processor_work_units(&graph, ProcessorKind::ModelOrderCycleBreaker),
            Ok(base
                + 2 * nodes
                + ports
                + 4 * outgoing
                + model_order_edge_reversal_work_units(&graph).unwrap())
        );

        assert!(!edge_and_layer_constraint_reversal_may_mutate(&graph));
        assert_eq!(
            processor_work_units(&graph, ProcessorKind::EdgeAndLayerConstraintEdgeReverser),
            Ok(base + 3 * nodes + 2 * ports + 4 * adjacency)
        );
        assert_eq!(
            processor_work_units(&graph, ProcessorKind::ReversedEdgeRestorer),
            Ok(base + reversed_edge_restorer_preparation_work_units(&graph).unwrap())
        );
    }
}

#[test]
fn candidate_scoped_reversals_exclude_unrelated_shared_collector_quadratic_work() {
    for parallel_edge_count in [64usize, 128, 256, 512] {
        let graph = shared_port_dag_with_independent_edge(parallel_edge_count);
        let whole_graph_mutation = adapted_edge_reversal_work_units(&graph, 1).unwrap();
        assert_eq!(
            whole_graph_mutation,
            4 * parallel_edge_count * parallel_edge_count + 9 * parallel_edge_count + 95
        );

        let mut constraint_graph = graph.clone();
        constraint_graph.layerless_nodes[0].layer_constraint = LayerConstraint::First;
        constraint_graph.layerless_nodes[1].layer_constraint = LayerConstraint::Last;
        constraint_graph.layerless_nodes[2].layer_constraint = LayerConstraint::Last;
        let constraint_mutation =
            edge_and_layer_constraint_reversal_work_units(&constraint_graph).unwrap();
        assert_eq!(constraint_mutation, 97);
        assert_processor_budget_boundaries(
            &constraint_graph,
            ProcessorKind::EdgeAndLayerConstraintEdgeReverser,
        );
        let mut constrained = constraint_graph;
        execute_processor(
            &mut constrained,
            ProcessorKind::EdgeAndLayerConstraintEdgeReverser,
        )
        .unwrap();
        assert!(
            constrained.edges[..parallel_edge_count]
                .iter()
                .all(|edge| !edge.reversed)
        );
        assert!(constrained.edges[parallel_edge_count].reversed);

        let mut model_order_graph = graph;
        model_order_graph.layerless_nodes[0].model_order = Some(0);
        model_order_graph.layerless_nodes[1].model_order = Some(1);
        model_order_graph.layerless_nodes[2].model_order = Some(3);
        model_order_graph.layerless_nodes[3].model_order = Some(2);
        let model_order_mutation =
            model_order_edge_reversal_work_units(&model_order_graph).unwrap();
        assert_eq!(model_order_mutation, 97);
        assert_processor_budget_boundaries(
            &model_order_graph,
            ProcessorKind::ModelOrderCycleBreaker,
        );
        let mut ordered = model_order_graph;
        execute_processor(&mut ordered, ProcessorKind::ModelOrderCycleBreaker).unwrap();
        assert!(
            ordered.edges[..parallel_edge_count]
                .iter()
                .all(|edge| !edge.reversed)
        );
        assert!(ordered.edges[parallel_edge_count].reversed);
    }
}

#[test]
fn exact_reversal_plans_precheck_the_linear_graph_floor() {
    let graph = shared_port_dag_with_independent_edge(512);
    let floor = local_graph_work_units(&graph).unwrap();
    assert!(floor > 0);

    let mut constraint_graph = graph.clone();
    constraint_graph.layerless_nodes[2].layer_constraint = LayerConstraint::Last;
    let original_constraint_graph = constraint_graph.clone();
    let mut constraint_budget = BudgetWorkControl::new(floor - 1);
    assert_eq!(
        execute_processor_with_work_control(
            &mut constraint_graph,
            ProcessorKind::EdgeAndLayerConstraintEdgeReverser,
            &mut constraint_budget,
        ),
        Err(PipelineError::Work(WorkError::Interrupted))
    );
    assert_eq!(constraint_graph, original_constraint_graph);
    assert_eq!(constraint_budget.checks, [floor]);
    assert_eq!(constraint_budget.charged, 0);

    let mut model_order_graph = graph;
    model_order_graph.layerless_nodes[2].model_order = Some(3);
    model_order_graph.layerless_nodes[3].model_order = Some(2);
    let original_model_order_graph = model_order_graph.clone();
    let mut model_order_budget = BudgetWorkControl::new(floor - 1);
    assert_eq!(
        execute_processor_with_work_control(
            &mut model_order_graph,
            ProcessorKind::ModelOrderCycleBreaker,
            &mut model_order_budget,
        ),
        Err(PipelineError::Work(WorkError::Interrupted))
    );
    assert_eq!(model_order_graph, original_model_order_graph);
    assert_eq!(model_order_budget.checks, [floor]);
    assert_eq!(model_order_budget.charged, 0);
}

#[test]
fn long_edge_splitter_work_curve_tracks_total_rank_span() {
    let edge_count = 4usize;
    for target_layer in [2usize, 4, 8, 16, 32] {
        let graph = disjoint_long_edge_graph(edge_count, target_layer);
        let base = local_graph_work_units(&graph).unwrap();
        let work = long_edge_splitter_work_units(&graph).unwrap();
        let expected_splits = edge_count * (target_layer - 1);

        assert_eq!(work - base, expected_splits * 10);
        assert_eq!(
            processor_work_units(&graph, ProcessorKind::LongEdgeSplitter),
            Ok(work)
        );
    }
}

#[test]
fn long_edge_splitter_work_curve_is_linear_in_edge_count_at_fixed_span() {
    let target_layer = 9usize;
    let mut previous = None;
    for edge_count in [4usize, 8, 16, 32] {
        let graph = disjoint_long_edge_graph(edge_count, target_layer);
        let base = local_graph_work_units(&graph).unwrap();
        let incremental = long_edge_splitter_work_units(&graph).unwrap() - base;

        assert_eq!(incremental, edge_count * (target_layer - 1) * 10);
        if let Some(previous) = previous {
            assert_eq!(incremental, previous * 2);
        }
        previous = Some(incremental);
    }
}

#[test]
fn long_edge_splitter_work_curve_tracks_shared_target_removal_amplification() {
    let target_layer = 9usize;
    for edge_count in [4usize, 8, 16, 32] {
        let graph = shared_target_long_edge_graph_with_span(edge_count, target_layer);
        let base = local_graph_work_units(&graph).unwrap();
        let work = long_edge_splitter_work_units(&graph).unwrap();
        let split_count = edge_count * (target_layer - 1);
        let target_rewrite_work = 2 * split_count * edge_count;

        assert_eq!(work - base, split_count * 8 + target_rewrite_work);
    }
}

#[test]
fn long_edge_splitter_work_ignores_same_layer_and_unlayered_targets() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::default(),
        nodes: vec![node("A"), node("B"), node("C"), node("layer-anchor")],
        edges: vec![edge("same", "A", "B"), edge("unlayered", "A", "C")],
    })
    .unwrap();
    graph.clear_layers();
    graph.set_node_layer(0, 0);
    graph.set_node_layer(1, 0);
    graph.set_node_layer(3, 2);

    assert_eq!(
        long_edge_splitter_work_units(&graph),
        local_graph_work_units(&graph)
    );
}

#[test]
fn long_edge_splitter_work_accounts_for_repeated_head_label_moves() {
    let mut head = ElkInputLabel::center("head", 20.0, 10.0);
    head.placement = crate::graph::EdgeLabelPlacement::Head;
    let mut labelled = edge("long", "A", "D");
    labelled.labels = vec![head];
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::default(),
        nodes: vec![node("A"), node("B"), node("C"), node("D")],
        edges: vec![labelled],
    })
    .unwrap();
    graph.clear_layers();
    for node_index in 0..4 {
        graph.set_node_layer(node_index, node_index);
    }

    let base = local_graph_work_units(&graph).unwrap();
    // Two splits: 16 structural units, four target-list rewrite units, and two complete
    // clone/scan/move passes over the downstream head label.
    assert_eq!(long_edge_splitter_work_units(&graph).unwrap() - base, 28);
}

#[test]
fn long_edge_splitter_work_accounts_for_quadratic_head_label_removals() {
    let mut head = ElkInputLabel::center("head-0", 20.0, 10.0);
    head.placement = crate::graph::EdgeLabelPlacement::Head;
    let mut labelled = edge("long", "A", "E");
    labelled.labels = vec![head];
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::default(),
        nodes: vec![node("A"), node("B"), node("C"), node("D"), node("E")],
        edges: vec![labelled],
    })
    .unwrap();
    for index in 1..4 {
        let mut label = crate::graph::LLabel::new(format!("head-{index}"), 20.0, 10.0);
        label.placement = crate::graph::EdgeLabelPlacement::Head;
        graph.edges[0].labels.push(label);
    }
    graph.clear_layers();
    for node_index in 0..5 {
        graph.set_node_layer(node_index, node_index);
    }

    assert_eq!(split_edge_payload_work_units(&graph.edges[0], 3), Ok(66));
    let base = local_graph_work_units(&graph).unwrap();
    // Three splits add 24 structural units and six target-list rewrite units. Four head labels
    // add 22 units on the first split and on each later split: clone + scan + moves + 3+2+1
    // shifted tail elements from repeated `Vec::remove(0)`.
    assert_eq!(long_edge_splitter_work_units(&graph).unwrap() - base, 96);
}

#[test]
fn compound_arena_plan_work_curve_is_linear_in_depth() {
    for graph_count in [8usize, 16, 32, 64, 128] {
        let graph = deep_compound_chain(graph_count);
        let node_count = graph_count - 1;
        let nested_graph_count = graph_count - 1;
        assert_eq!(
            hierarchy_arena_plan_work_units(&graph).unwrap(),
            graph_count * 4 + node_count + nested_graph_count
        );
    }
}

#[test]
fn compound_arena_stores_one_parent_and_reusable_slot_per_graph() {
    for graph_count in [8usize, 16, 32, 64, 128] {
        let graph = deep_compound_chain(graph_count);
        let arena = CompoundGraphArena::new(&graph);
        let parent_links = arena
            .entries
            .iter()
            .filter(|entry| entry.parent.is_some())
            .count();

        assert_eq!(arena.entries.len(), graph_count);
        assert_eq!(arena.algorithms.len(), graph_count);
        assert_eq!(arena.graph_slots.len(), graph_count);
        assert!(arena.graph_slots.iter().all(Option::is_none));
        assert_eq!(parent_links, graph_count - 1);
    }
}

#[test]
fn compound_arena_preserves_official_reverse_discovery_order() {
    let root = branched_compound_graph();
    let arena = CompoundGraphArena::new(&root);
    let graph_ids = arena
        .algorithms
        .iter()
        .rev()
        .map(|algorithm| algorithm.execution.graph_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        graph_ids,
        [
            "first-grandchild",
            "second-grandchild",
            "second-child",
            "first-child",
            "root",
        ]
    );
}

#[test]
fn public_compound_execution_reports_official_reverse_discovery_order() {
    let mut root = branched_compound_graph();
    let executions = execute_ported_compound_processors_until_processor(
        &mut root,
        ProcessorKind::GreedyCycleBreaker,
    )
    .unwrap();
    let graph_ids = executions
        .iter()
        .map(|execution| execution.graph_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        graph_ids,
        [
            "first-grandchild",
            "second-grandchild",
            "second-child",
            "first-child",
            "root",
        ]
    );
}

#[test]
fn compound_round_reattaches_the_hierarchy_when_a_child_processor_fails() {
    let options = LayeredOptions::default();
    let child = LGraph::new("child", options.clone());
    let mut holder = LNode::new("holder", 1.0, 1.0, None);
    holder.nested_graph = Some(Box::new(child));
    let mut root = LGraph::new("root", options);
    root.layerless_nodes.push(holder);
    let mut arena = CompoundGraphArena::new(&root);
    let child_index = arena
        .algorithms
        .iter()
        .position(|algorithm| algorithm.execution.graph_id == "child")
        .unwrap();
    arena.algorithms[child_index].processors = vec![ProcessorSlot {
        phase: None,
        kind: ProcessorKind::CommentPreprocessor,
    }];
    let mut work_control = NoopWorkControl;

    assert_eq!(
        execute_compound_graph_round(
            &mut root,
            &arena.entries,
            &mut arena.graph_slots,
            &mut arena.algorithms,
            None,
            &mut work_control,
        ),
        Err(PipelineError::UnsupportedProcessor {
            kind: ProcessorKind::CommentPreprocessor,
        })
    );
    assert_eq!(
        root.layerless_nodes[0]
            .nested_graph
            .as_deref()
            .map(|graph| graph.id.as_str()),
        Some("child")
    );
    assert!(arena.graph_slots.iter().all(Option::is_none));
}

#[test]
fn compound_round_reattaches_the_complete_hierarchy_when_work_control_panics() {
    struct PanicOnCharge;

    impl WorkControl for PanicOnCharge {
        fn check(&mut self, _units: usize) -> Result<(), WorkError> {
            Ok(())
        }

        fn charge(&mut self, _units: usize) -> Result<(), WorkError> {
            panic!("injected work-control panic")
        }
    }

    let mut root = deep_compound_chain(3);
    let mut arena = CompoundGraphArena::new(&root);
    for algorithm in &mut arena.algorithms {
        algorithm.processors.clear();
    }
    let grandchild_index = arena
        .algorithms
        .iter()
        .position(|algorithm| algorithm.execution.graph_id == "graph-2")
        .unwrap();
    arena.algorithms[grandchild_index].processors = vec![ProcessorSlot {
        phase: None,
        kind: ProcessorKind::DirectionPreprocessor,
    }];
    let mut work_control = PanicOnCharge;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = execute_compound_graph_round(
            &mut root,
            &arena.entries,
            &mut arena.graph_slots,
            &mut arena.algorithms,
            None,
            &mut work_control,
        );
    }));

    assert!(result.is_err());
    let child = root.layerless_nodes[0]
        .nested_graph
        .as_deref()
        .expect("the first detached graph must be restored during unwind");
    assert_eq!(child.id, "graph-1");
    let grandchild = child.layerless_nodes[0]
        .nested_graph
        .as_deref()
        .expect("the deepest detached graph must be restored during unwind");
    assert_eq!(grandchild.id, "graph-2");
    assert!(arena.graph_slots.iter().all(Option::is_none));
}

#[test]
fn compound_round_restores_already_detached_graphs_when_detachment_panics() {
    let mut root = deep_compound_chain(3);
    let mut arena = CompoundGraphArena::new(&root);
    let grandchild_index = arena
        .algorithms
        .iter()
        .position(|algorithm| algorithm.execution.graph_id == "graph-2")
        .unwrap();
    arena.entries[grandchild_index]
        .parent
        .as_mut()
        .expect("the grandchild must have an arena parent")
        .node = usize::MAX;
    let mut work_control = NoopWorkControl;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = execute_compound_graph_round(
            &mut root,
            &arena.entries,
            &mut arena.graph_slots,
            &mut arena.algorithms,
            None,
            &mut work_control,
        );
    }));

    assert!(result.is_err());
    let child = root.layerless_nodes[0]
        .nested_graph
        .as_deref()
        .expect("the graph detached before the panic must be restored");
    assert_eq!(child.id, "graph-1");
    assert_eq!(
        child.layerless_nodes[0]
            .nested_graph
            .as_deref()
            .map(|graph| graph.id.as_str()),
        Some("graph-2")
    );
    assert!(arena.graph_slots.iter().all(Option::is_none));
}

#[test]
fn compound_round_reuses_the_arena_graph_slots() {
    let mut root = deep_compound_chain(8);
    let mut arena = CompoundGraphArena::new(&root);
    for algorithm in &mut arena.algorithms {
        algorithm.processors.clear();
    }
    let slots_ptr = arena.graph_slots.as_ptr();
    let slots_capacity = arena.graph_slots.capacity();
    let mut work_control = NoopWorkControl;

    for _ in 0..2 {
        execute_compound_graph_round(
            &mut root,
            &arena.entries,
            &mut arena.graph_slots,
            &mut arena.algorithms,
            None,
            &mut work_control,
        )
        .unwrap();
        assert!(arena.graph_slots.iter().all(Option::is_none));
        assert_eq!(arena.graph_slots.as_ptr(), slots_ptr);
        assert_eq!(arena.graph_slots.capacity(), slots_capacity);
    }
}

#[test]
fn compound_arena_plan_work_curve_accounts_for_flat_node_scans() {
    for node_count in [8usize, 16, 32, 64, 128] {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        for index in 0..node_count {
            graph
                .layerless_nodes
                .push(LNode::new(format!("node-{index}"), 1.0, 1.0, None));
        }

        assert_eq!(
            hierarchy_arena_plan_work_units(&graph).unwrap(),
            node_count + 4
        );
    }
}

#[test]
fn compound_arena_plan_rejects_before_materializing_the_index() {
    let mut graph = deep_compound_chain(32);
    let required = hierarchy_arena_plan_work_units(&graph).unwrap();

    let mut below = BudgetWorkControl::new(required - 1);
    assert!(matches!(
        build_compound_graph_arena_with_work_control(&mut graph, &mut below),
        Err(PipelineError::Work(WorkError::Interrupted))
    ));
    assert_eq!(below.charged, 0);
    assert_eq!(below.remaining, required - 1);

    let mut exact = BudgetWorkControl::new(required);
    let exact_arena = build_compound_graph_arena_with_work_control(&mut graph, &mut exact).unwrap();
    assert_eq!(exact_arena.entries.len(), 32);
    assert_eq!(exact_arena.graph_slots.len(), 32);
    assert!(exact_arena.graph_slots.iter().all(Option::is_none));
    assert_eq!(exact.charged, required);
    assert_eq!(exact.remaining, 0);

    let mut above = BudgetWorkControl::new(required + 1);
    let above_arena = build_compound_graph_arena_with_work_control(&mut graph, &mut above).unwrap();
    assert_eq!(above_arena.entries, exact_arena.entries);
    assert_eq!(above_arena.algorithms, exact_arena.algorithms);
    assert_eq!(above_arena.graph_slots.len(), exact_arena.graph_slots.len());
    assert_eq!(above.charged, required);
    assert_eq!(above.remaining, 1);
}

#[test]
fn hierarchy_preflight_checks_each_admitted_stack_prefix_before_allocation() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    for index in 0..128 {
        let mut holder = LNode::new(format!("holder-{index}"), 1.0, 1.0, None);
        holder.compound = true;
        holder.nested_graph = Some(Box::new(LGraph::new(
            format!("child-{index}"),
            LayeredOptions::default(),
        )));
        graph.layerless_nodes.push(holder);
    }
    let root_work = local_hierarchy_arena_plan_work_units(&graph).unwrap();
    let reserved_prefix = root_work + 128;

    let mut rejected = BudgetWorkControl::new(reserved_prefix - 1);
    assert_eq!(
        charge_hierarchy_arena_plan_work(&graph, &mut rejected),
        Err(PipelineError::Work(WorkError::Interrupted))
    );
    assert_eq!(rejected.checks, vec![root_work, reserved_prefix]);
    assert_eq!(rejected.charged, 0);

    let required = hierarchy_arena_plan_work_units(&graph).unwrap();
    let mut exact = BudgetWorkControl::new(required);
    charge_hierarchy_arena_plan_work(&graph, &mut exact).unwrap();

    assert_eq!(exact.checks.first(), Some(&root_work));
    assert_eq!(exact.checks.last(), Some(&required));
    assert!(exact.checks.windows(2).all(|window| window[0] <= window[1]));
    assert_eq!(exact.charged, required);
    assert_eq!(exact.remaining, 0);
}

#[test]
fn hierarchy_processor_preflight_preserves_reachable_overflow_priority() {
    let mut root_overflow = bidirected_cycle_graph(2);
    root_overflow.options.thoroughness = usize::MAX;

    let mut limited = BudgetWorkControl::new(0);
    assert_eq!(
        hierarchy_processor_work_units_with_preflight(
            &root_overflow,
            ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
            0,
            &mut limited,
        ),
        Err(WorkError::ArithmeticOverflow)
    );
    assert!(limited.checks.is_empty());
    assert_eq!(limited.charged, 0);

    let mut unlimited = NoopWorkControl;
    assert_eq!(
        hierarchy_processor_work_units_with_preflight(
            &root_overflow,
            ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
            0,
            &mut unlimited,
        ),
        Err(WorkError::ArithmeticOverflow)
    );

    let mut root = LGraph::new("root", LayeredOptions::default());
    let mut child = bidirected_cycle_graph(2);
    child.options.thoroughness = usize::MAX;
    let mut holder = LNode::new("holder", 1.0, 1.0, None);
    holder.compound = true;
    holder.nested_graph = Some(Box::new(child));
    root.layerless_nodes.push(holder);

    let root_work = local_hierarchy_work_units(&root).unwrap();
    let root_prefix = root_work * root.options.thoroughness.max(1);
    let mut child_limited = BudgetWorkControl::new(root_prefix);
    assert_eq!(
        hierarchy_processor_work_units_with_preflight(
            &root,
            ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
            0,
            &mut child_limited,
        ),
        Err(WorkError::ArithmeticOverflow)
    );
    assert_eq!(child_limited.checks, vec![root_prefix]);
    assert_eq!(child_limited.charged, 0);
}

#[test]
fn diagnostic_crossing_sweep_and_trace_share_one_budget_tranche() {
    const HIERARCHY_WORK: usize = 18;
    const EDGE_COUNT: usize = 4;
    const THOROUGHNESS: usize = 64;
    const DIAGNOSTIC_WORK: usize = HIERARCHY_WORK * (THOROUGHNESS * EDGE_COUNT + 1);

    let target = ProcessorKind::PortListSorter;
    let graph = high_thoroughness_diagnostic_graph();

    let mut prepared = graph.clone();
    let mut prefix = BudgetWorkControl::new(usize::MAX);
    execute_ported_compound_processors_until_processor_with_work_control(
        &mut prepared,
        target,
        &mut prefix,
    )
    .unwrap();
    assert_eq!(hierarchy_work_units(&prepared), Ok(HIERARCHY_WORK));
    assert_eq!(
        hierarchy_processor_work_units(
            &prepared,
            ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
        ),
        Ok(HIERARCHY_WORK * THOROUGHNESS * EDGE_COUNT)
    );
    let required = checked_add(prefix.charged, DIAGNOSTIC_WORK).unwrap();

    let mut early_graph = graph.clone();
    let mut early = BudgetWorkControl::new(
        checked_add(
            prefix.charged,
            HIERARCHY_WORK * THOROUGHNESS * EDGE_COUNT - 1,
        )
        .unwrap(),
    );
    assert_eq!(
        inspect_compound_crossings_after_processor_with_work_control(
            &mut early_graph,
            target,
            &mut early,
        ),
        Err(PipelineError::Work(WorkError::Interrupted))
    );
    assert_eq!(early_graph, prepared);
    assert_eq!(early.charged, prefix.charged);

    let mut below_graph = graph.clone();
    let mut below = BudgetWorkControl::new(required - 1);
    assert_eq!(
        inspect_compound_crossings_after_processor_with_work_control(
            &mut below_graph,
            target,
            &mut below,
        ),
        Err(PipelineError::Work(WorkError::Interrupted))
    );
    assert_eq!(below_graph, prepared);
    assert_eq!(below.charged, prefix.charged);
    assert_eq!(below.remaining, DIAGNOSTIC_WORK - 1);

    let mut exact_graph = graph.clone();
    let mut exact = BudgetWorkControl::new(required);
    let (_, exact_trace) = inspect_compound_crossings_after_processor_with_work_control(
        &mut exact_graph,
        target,
        &mut exact,
    )
    .unwrap();
    assert_eq!(
        exact_trace.as_ref().map(|trace| trace.runs.len()),
        Some(THOROUGHNESS)
    );
    assert_eq!(exact.charged, required);
    assert_eq!(exact.remaining, 0);

    let mut above_graph = graph;
    let mut above = BudgetWorkControl::new(required + 1);
    let above_result = inspect_compound_crossings_after_processor_with_work_control(
        &mut above_graph,
        target,
        &mut above,
    )
    .unwrap();
    assert_eq!(above_graph, exact_graph);
    assert_eq!(above_result.1, exact_trace);
    assert_eq!(above.charged, required);
    assert_eq!(above.remaining, 1);
}

#[test]
fn processor_work_rejects_configured_iteration_overflow_before_execution() {
    let mut graph = LGraph::new(
        "root",
        LayeredOptions {
            thoroughness: usize::MAX,
            ..LayeredOptions::default()
        },
    );
    graph
        .layerless_nodes
        .push(LNode::new("A", 80.0, 40.0, None));

    assert_eq!(
        processor_work_units(&graph, ProcessorKind::NetworkSimplexLayerer),
        Err(WorkError::ArithmeticOverflow)
    );
}

#[test]
fn full_runner_rejects_unported_strategies_before_mutating_graph() {
    let cases = [
        (
            LayeredOptions {
                layering_strategy: LayeringStrategy::BreadthFirstModelOrder,
                ..LayeredOptions::default()
            },
            ProcessorKind::BreadthFirstModelOrderLayerer,
        ),
        (
            LayeredOptions {
                crossing_minimization_strategy: CrossingMinimizationStrategy::Interactive,
                ..LayeredOptions::default()
            },
            ProcessorKind::InteractiveCrossingMinimizer,
        ),
        (
            LayeredOptions {
                node_placement_strategy: NodePlacementStrategy::Interactive,
                ..LayeredOptions::default()
            },
            ProcessorKind::InteractiveNodePlacer,
        ),
        (
            LayeredOptions {
                edge_routing: EdgeRouting::Polyline,
                ..LayeredOptions::default()
            },
            ProcessorKind::PolylineEdgeRouter,
        ),
        (
            LayeredOptions {
                wrapping_strategy: WrappingStrategy::SingleEdge,
                ..LayeredOptions::default()
            },
            ProcessorKind::SingleEdgeGraphWrapper,
        ),
    ];

    for (options, expected) in cases {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options,
            nodes: vec![node("A"), node("B")],
            edges: vec![edge("A-B", "A", "B")],
        })
        .unwrap();
        let before = graph.clone();

        let result = execute_ported_processors(&mut graph);

        assert_eq!(
            result,
            Err(PipelineError::UnsupportedProcessor { kind: expected })
        );
        assert_eq!(
            graph, before,
            "{expected:?} mutated the graph before failing"
        );
    }
}

#[test]
fn full_runner_charges_validation_before_rejecting_an_unported_processor() {
    let graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            layering_strategy: LayeringStrategy::BreadthFirstModelOrder,
            ..LayeredOptions::default()
        },
        nodes: vec![node("A"), node("B")],
        edges: vec![edge("A-B", "A", "B")],
    })
    .unwrap();
    let required = hierarchy_work_units(&graph).unwrap();

    let mut below_graph = graph.clone();
    let mut below = BudgetWorkControl::new(required - 1);
    assert_eq!(
        execute_ported_processors_with_work_control(&mut below_graph, &mut below),
        Err(PipelineError::Work(WorkError::Interrupted))
    );
    assert_eq!(below_graph, graph);
    assert_eq!(below.charged, 0);

    let mut exact_graph = graph.clone();
    let mut exact = BudgetWorkControl::new(required);
    assert_eq!(
        execute_ported_processors_with_work_control(&mut exact_graph, &mut exact),
        Err(PipelineError::UnsupportedProcessor {
            kind: ProcessorKind::BreadthFirstModelOrderLayerer,
        })
    );
    assert_eq!(exact_graph, graph);
    assert_eq!(exact.charged, required);
    assert_eq!(exact.remaining, 0);
}

#[test]
fn compound_full_runner_rejects_child_unported_processor_before_mutation() {
    let mut graph = deep_compound_chain(2);
    graph.layerless_nodes[0]
        .nested_graph
        .as_deref_mut()
        .expect("the fixture should contain one child graph")
        .options
        .layering_strategy = LayeringStrategy::BreadthFirstModelOrder;
    let before = graph.clone();

    assert_eq!(
        execute_ported_compound_processors(&mut graph),
        Err(PipelineError::UnsupportedProcessor {
            kind: ProcessorKind::BreadthFirstModelOrderLayerer,
        })
    );
    assert_eq!(graph, before);
}

#[test]
fn full_runner_reports_first_unported_processor_in_pipeline_order() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            layering_strategy: LayeringStrategy::BreadthFirstModelOrder,
            node_placement_strategy: NodePlacementStrategy::Interactive,
            edge_routing: EdgeRouting::Splines,
            wrapping_strategy: WrappingStrategy::MultiEdge,
            ..LayeredOptions::default()
        },
        nodes: vec![node("A"), node("B")],
        edges: vec![edge("A-B", "A", "B")],
    })
    .unwrap();

    assert_eq!(
        execute_ported_processors(&mut graph),
        Err(PipelineError::UnsupportedProcessor {
            kind: ProcessorKind::BreadthFirstModelOrderLayerer,
        })
    );
}

#[test]
fn compound_ported_processor_preflight_is_small_stack_safe() {
    const GRAPH_COUNT: usize = 4_096;

    std::thread::Builder::new()
        .name("elk-ported-processor-preflight-small-stack".to_string())
        .stack_size(64 * 1024)
        .spawn(|| {
            let mut graph = deep_compound_chain(GRAPH_COUNT);
            let mut leaf = &mut graph;
            for _ in 1..GRAPH_COUNT {
                leaf = leaf.layerless_nodes[0]
                    .nested_graph
                    .as_deref_mut()
                    .expect("the deep compound fixture should retain every child graph");
            }
            leaf.options.layering_strategy = LayeringStrategy::BreadthFirstModelOrder;

            let graph = std::mem::ManuallyDrop::new(graph);
            assert_eq!(
                validate_ported_graph_processors(&graph),
                Err(PipelineError::UnsupportedProcessor {
                    kind: ProcessorKind::BreadthFirstModelOrderLayerer,
                })
            );
        })
        .expect("the small-stack preflight thread should start")
        .join()
        .expect("ported processor validation must not recurse with hierarchy depth");
}

#[test]
fn layered_baseline_processor_sequence_matches_elkjs_0_9_3_logging() {
    assert_eq!(
        kinds(&LayeredOptions::default()),
        vec![
            ProcessorKind::EdgeAndLayerConstraintEdgeReverser,
            ProcessorKind::GreedyCycleBreaker,
            ProcessorKind::LayerConstraintPreprocessor,
            ProcessorKind::NetworkSimplexLayerer,
            ProcessorKind::LayerConstraintPostprocessor,
            ProcessorKind::LongEdgeSplitter,
            ProcessorKind::PortSideProcessor,
            ProcessorKind::PortListSorter,
            ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
            ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch,
            ProcessorKind::InLayerConstraintProcessor,
            ProcessorKind::LabelAndNodeSizeProcessor,
            ProcessorKind::InnermostNodeMarginCalculator,
            ProcessorKind::BKNodePlacer,
            ProcessorKind::LayerSizeAndGraphHeightCalculator,
            ProcessorKind::OrthogonalEdgeRouter,
            ProcessorKind::LongEdgeJoiner,
            ProcessorKind::EndLabelSorter,
            ProcessorKind::ReversedEdgeRestorer,
        ]
    );
}

#[test]
fn mermaid_flowchart_defaults_insert_direction_model_order_and_hierarchy_processors() {
    let options = LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down);
    assert_eq!(
        kinds(&options),
        vec![
            ProcessorKind::DirectionPreprocessor,
            ProcessorKind::EdgeAndLayerConstraintEdgeReverser,
            ProcessorKind::GreedyCycleBreaker,
            ProcessorKind::LayerConstraintPreprocessor,
            ProcessorKind::NetworkSimplexLayerer,
            ProcessorKind::LayerConstraintPostprocessor,
            ProcessorKind::LongEdgeSplitter,
            ProcessorKind::PortSideProcessor,
            ProcessorKind::PortListSorter,
            ProcessorKind::SortByInputModelProcessor,
            ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
            ProcessorKind::InLayerConstraintProcessor,
            ProcessorKind::LabelAndNodeSizeProcessor,
            ProcessorKind::InnermostNodeMarginCalculator,
            ProcessorKind::BKNodePlacer,
            ProcessorKind::LayerSizeAndGraphHeightCalculator,
            ProcessorKind::OrthogonalEdgeRouter,
            ProcessorKind::LongEdgeJoiner,
            ProcessorKind::EndLabelSorter,
            ProcessorKind::ReversedEdgeRestorer,
            ProcessorKind::HierarchicalNodeResizer,
            ProcessorKind::DirectionPostprocessor,
        ]
    );
}

#[test]
fn mermaid_wrapping_flags_do_not_enable_wrapping_processors_without_strategy() {
    let options = LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down);
    let processors = kinds(&options);
    assert!(!processors.contains(&ProcessorKind::BreakingPointInserter));
    assert!(!processors.contains(&ProcessorKind::BreakingPointProcessor));
    assert!(!processors.contains(&ProcessorKind::BreakingPointRemover));
}

#[test]
fn mermaid_reachable_flowchart_elk_processors_are_source_ported() {
    for direction in [
        ElkDirection::Right,
        ElkDirection::Left,
        ElkDirection::Down,
        ElkDirection::Up,
    ] {
        assert_processors_are_source_ported(
            &format!("Mermaid flowchart direction {direction:?}"),
            kinds(&LayeredOptions::mermaid_flowchart_defaults(direction)),
        );
    }

    for cycle_breaking_strategy in [
        CycleBreakingStrategy::Greedy,
        CycleBreakingStrategy::DepthFirst,
        CycleBreakingStrategy::Interactive,
        CycleBreakingStrategy::ModelOrder,
        CycleBreakingStrategy::GreedyModelOrder,
    ] {
        assert_processors_are_source_ported(
            &format!("public cycleBreakingStrategy {cycle_breaking_strategy:?}"),
            kinds(&LayeredOptions {
                cycle_breaking_strategy,
                ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
            }),
        );
    }

    for node_placement_strategy in [
        NodePlacementStrategy::BrandesKoepf,
        NodePlacementStrategy::Simple,
        NodePlacementStrategy::LinearSegments,
        NodePlacementStrategy::NetworkSimplex,
    ] {
        assert_processors_are_source_ported(
            &format!("public nodePlacementStrategy {node_placement_strategy:?}"),
            kinds(&LayeredOptions {
                node_placement_strategy,
                ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
            }),
        );
    }

    for node_placement_bk_fixed_alignment in [
        FixedAlignment::None,
        FixedAlignment::LeftUp,
        FixedAlignment::RightUp,
        FixedAlignment::LeftDown,
        FixedAlignment::RightDown,
        FixedAlignment::Balanced,
    ] {
        assert_processors_are_source_ported(
            &format!("public nodePlacementAlignment {node_placement_bk_fixed_alignment:?}"),
            kinds(&LayeredOptions {
                node_placement_bk_fixed_alignment,
                ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
            }),
        );
    }

    for consider_model_order_strategy in [
        OrderingStrategy::None,
        OrderingStrategy::NodesAndEdges,
        OrderingStrategy::PreferEdges,
        OrderingStrategy::PreferNodes,
    ] {
        assert_processors_are_source_ported(
            &format!("public considerModelOrder {consider_model_order_strategy:?}"),
            kinds(&LayeredOptions {
                consider_model_order_strategy,
                ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
            }),
        );
    }

    assert_processors_are_source_ported(
        "public forceNodeModelOrder",
        kinds(&LayeredOptions {
            force_node_model_order: true,
            ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
        }),
    );

    let merge_edges_graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            merge_edges: true,
            ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
        },
        nodes: vec![node("A"), node("B"), node("C")],
        edges: vec![
            edge("A-C-1", "A", "C"),
            edge("A-C-2", "A", "C"),
            edge("B-C", "B", "C"),
        ],
    })
    .unwrap();
    assert_processors_are_source_ported(
        "public mergeEdges hyperedge graph",
        graph_kinds(&merge_edges_graph),
    );

    let mut constrained_entry = node("A");
    constrained_entry.layer_constraint = Some(crate::options::LayerConstraint::First);
    let keep_entry_graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        nodes: vec![constrained_entry, node("B"), node("C")],
        edges: vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
    })
    .unwrap();
    assert_processors_are_source_ported(
        "public keepEntryNodeOnTop layer constraint",
        graph_kinds(&keep_entry_graph),
    );

    let mut head = ElkInputLabel::center("head", 20.0, 10.0);
    head.placement = crate::graph::EdgeLabelPlacement::Head;
    let mut end_label_edge = edge("A-B", "A", "B");
    end_label_edge.labels = vec![head];
    let label_and_self_loop_graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        nodes: vec![node("A"), node("B")],
        edges: vec![
            ElkInputEdge {
                labels: vec![ElkInputLabel::center("center", 28.0, 12.0)],
                ..edge("A-A", "A", "A")
            },
            end_label_edge,
        ],
    })
    .unwrap();
    assert_processors_are_source_ported(
        "Mermaid flowchart labels and self loops",
        graph_kinds(&label_and_self_loop_graph),
    );

    let mut cluster = node("cluster");
    cluster.hierarchy_handling = Some(crate::options::HierarchyHandling::IncludeChildren);
    let mut child = node("A");
    child.parent = Some("cluster".to_string());
    let mut compound_graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        nodes: vec![cluster, child, node("B")],
        edges: vec![edge("A-B", "A", "B")],
    })
    .unwrap();
    preprocess_source_ported_compound_graph(&mut compound_graph);
    assert_processors_are_source_ported(
        "Mermaid flowchart compound root graph",
        graph_kinds(&compound_graph),
    );
    let nested_graph = compound_graph.layerless_nodes[0]
        .nested_graph
        .as_ref()
        .expect("compound fixture should create a nested graph");
    assert_processors_are_source_ported(
        "Mermaid flowchart compound child graph",
        graph_kinds(nested_graph),
    );
}

#[test]
fn greedy_model_order_cycle_breaking_strategy_assembles_processor() {
    let options = LayeredOptions {
        cycle_breaking_strategy: CycleBreakingStrategy::GreedyModelOrder,
        ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
    };
    assert!(kinds(&options).contains(&ProcessorKind::GreedyModelOrderCycleBreaker));
}

#[test]
fn documented_node_placement_strategies_assemble_public_processors() {
    let cases = [
        (
            NodePlacementStrategy::BrandesKoepf,
            ProcessorKind::BKNodePlacer,
        ),
        (
            NodePlacementStrategy::Simple,
            ProcessorKind::SimpleNodePlacer,
        ),
        (
            NodePlacementStrategy::LinearSegments,
            ProcessorKind::LinearSegmentsNodePlacer,
        ),
        (
            NodePlacementStrategy::NetworkSimplex,
            ProcessorKind::NetworkSimplexPlacer,
        ),
    ];

    for (strategy, expected_processor) in cases {
        let options = LayeredOptions {
            node_placement_strategy: strategy,
            ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
        };

        assert!(
            kinds(&options).contains(&expected_processor),
            "{strategy:?} should assemble {expected_processor:?}"
        );
    }
}

#[test]
fn merge_edges_hyperedge_graph_assembles_dummy_merger() {
    let graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            merge_edges: true,
            ..LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down)
        },
        nodes: vec![node("A"), node("B"), node("C")],
        edges: vec![
            edge("A-C-1", "A", "C"),
            edge("A-C-2", "A", "C"),
            edge("B-C", "B", "C"),
        ],
    })
    .unwrap();

    assert!(graph.options.graph_has_hyperedges);
    assert!(
        graph_kinds(&graph).contains(&ProcessorKind::HyperedgeDummyMerger),
        "mergeEdges hyperedge graph should assemble HyperedgeDummyMerger"
    );
}

#[test]
fn graph_properties_insert_label_self_loop_and_external_port_processors() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        nodes: vec![
            ElkInputNode {
                id: "cluster".to_string(),
                width: 0.0,
                height: 0.0,
                parent: None,
                direction: None,
                hierarchy_handling: Some(crate::options::HierarchyHandling::IncludeChildren),
                layer_constraint: None,
                port_alignment: None,
                port_constraints: None,
                node_label_placement: crate::options::NodeLabelPlacement::Fixed,
                node_flexibility: crate::options::NodeFlexibility::None,
                ports_surrounding: None,
                nested_options: None,
                label: None,
            },
            ElkInputNode {
                id: "A".to_string(),
                width: 80.0,
                height: 40.0,
                parent: Some("cluster".to_string()),
                direction: None,
                hierarchy_handling: None,
                layer_constraint: None,
                port_alignment: None,
                port_constraints: None,
                node_label_placement: crate::options::NodeLabelPlacement::Fixed,
                node_flexibility: crate::options::NodeFlexibility::None,
                ports_surrounding: None,
                nested_options: None,
                label: None,
            },
        ],
        edges: vec![
            ElkInputEdge {
                id: "cluster-A".to_string(),
                source: "cluster".to_string(),
                target: "A".to_string(),
                labels: vec![ElkInputLabel::center("inside", 24.0, 12.0)],
                minlen: 1,
                inside_self_loops_yo: false,
                model_order: None,
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
            },
            ElkInputEdge {
                id: "A-A".to_string(),
                source: "A".to_string(),
                target: "A".to_string(),
                labels: Vec::new(),
                minlen: 1,
                inside_self_loops_yo: false,
                model_order: None,
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
            },
        ],
    })
    .unwrap();
    preprocess_source_ported_compound_graph(&mut graph);

    let nested = graph.layerless_nodes[0].nested_graph.as_ref().unwrap();
    let processors = graph_kinds(nested);
    assert!(processors.contains(&ProcessorKind::HierarchicalPortConstraintProcessor));
    assert!(processors.contains(&ProcessorKind::HierarchicalPortDummySizeProcessor));
    assert!(processors.contains(&ProcessorKind::HierarchicalPortPositionProcessor));
    assert!(processors.contains(&ProcessorKind::HierarchicalPortOrthogonalEdgeRouter));
    assert!(processors.contains(&ProcessorKind::LabelDummyInserter));
    assert!(processors.contains(&ProcessorKind::LabelDummySwitcher));
    assert!(processors.contains(&ProcessorKind::LabelDummyRemover));
    assert!(processors.contains(&ProcessorKind::SelfLoopPreProcessor));
    assert!(processors.contains(&ProcessorKind::SelfLoopRouter));
    assert!(processors.contains(&ProcessorKind::SelfLoopPostProcessor));
}

#[test]
fn inside_self_loops_enable_self_loop_processors_on_nested_graphs() {
    let mut input = ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        nodes: vec![ElkInputNode {
            id: "A".to_string(),
            width: 80.0,
            height: 40.0,
            parent: None,
            direction: None,
            hierarchy_handling: Some(crate::options::HierarchyHandling::IncludeChildren),
            layer_constraint: None,
            port_alignment: None,
            port_constraints: None,
            node_label_placement: crate::options::NodeLabelPlacement::Fixed,
            node_flexibility: crate::options::NodeFlexibility::None,
            ports_surrounding: None,
            nested_options: None,
            label: None,
        }],
        edges: vec![ElkInputEdge {
            id: "A-A".to_string(),
            source: "A".to_string(),
            target: "A".to_string(),
            labels: Vec::new(),
            minlen: 1,
            inside_self_loops_yo: true,
            model_order: None,
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
        }],
    };
    input.options.inside_self_loops_activate = true;

    let mut graph = import_graph(&input).unwrap();

    preprocess_source_ported_compound_graph(&mut graph);

    let nested = graph.layerless_nodes[0]
        .nested_graph
        .as_ref()
        .expect("inside self-loop should create a nested graph");
    let processors = graph_kinds(nested);

    assert!(processors.contains(&ProcessorKind::SelfLoopPreProcessor));
    assert!(processors.contains(&ProcessorKind::SelfLoopPortRestorer));
    assert!(processors.contains(&ProcessorKind::SelfLoopRouter));
    assert!(processors.contains(&ProcessorKind::SelfLoopPostProcessor));
}

#[test]
fn hierarchical_port_orthogonal_router_runs_for_east_west_external_ports() {
    let mut graph = LGraph::new(
        "root",
        LayeredOptions {
            port_constraints: PortConstraints::FixedPos,
            ..LayeredOptions::default()
        },
    );
    let port = push_test_external_dummy(&mut graph, "west", PortSide::West);
    graph.layerless_nodes[port].port_ratio_or_position = 20.0;
    graph.set_node_layer(port, 0);

    execute_processor(
        &mut graph,
        ProcessorKind::HierarchicalPortOrthogonalEdgeRouter,
    )
    .unwrap();

    assert_eq!(graph.layerless_nodes[port].position.y, 20.0);
}

#[test]
fn hierarchical_port_orthogonal_router_runs_for_north_south_external_ports() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    let port = push_test_external_dummy(&mut graph, "north", PortSide::North);
    graph.set_node_layer(port, 0);

    execute_processor(
        &mut graph,
        ProcessorKind::HierarchicalPortOrthogonalEdgeRouter,
    )
    .unwrap();

    assert_eq!(graph.layerless_nodes[port].position.y, 0.0);
}

#[test]
fn execute_processors_until_p3_runs_source_ported_processor_sequence() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: p3_options(),
        nodes: vec![node("A"), node("B"), node("C")],
        edges: vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
    })
    .unwrap();

    let executed = execute_processors_until(&mut graph, LayeredPhase::P3NodeOrdering).unwrap();

    assert_eq!(
        executed,
        vec![
            ProcessorKind::EdgeAndLayerConstraintEdgeReverser,
            ProcessorKind::GreedyCycleBreaker,
            ProcessorKind::LayerConstraintPreprocessor,
            ProcessorKind::NetworkSimplexLayerer,
            ProcessorKind::LayerConstraintPostprocessor,
            ProcessorKind::LongEdgeSplitter,
            ProcessorKind::PortSideProcessor,
            ProcessorKind::PortListSorter,
            ProcessorKind::LayerSweepCrossingMinimizerBarycenter,
        ]
    );
    assert_eq!(graph.layers.len(), 3);
    assert_eq!(
        graph
            .layerless_nodes
            .iter()
            .filter(|node| node.hidden)
            .count(),
        0
    );
}

#[test]
fn execute_processors_until_p3_minimizes_crossings() {
    let input = ElkInputGraph {
        id: "root".to_string(),
        options: p3_options(),
        nodes: vec![node("Top"), node("Bottom"), node("Left"), node("Right")],
        edges: vec![
            edge("Top-Right", "Top", "Right"),
            edge("Bottom-Left", "Bottom", "Left"),
        ],
    };
    let mut before_graph = import_graph(&input).unwrap();

    layer_network_simplex(&mut before_graph).unwrap();
    process_port_sides(&mut before_graph);
    sort_port_lists(&mut before_graph);
    let top = before_graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Top")
        .unwrap();
    let bottom = before_graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Bottom")
        .unwrap();
    let left = before_graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Left")
        .unwrap();
    let right = before_graph
        .layerless_nodes
        .iter()
        .position(|node| node.id == "Right")
        .unwrap();
    before_graph.layers[0].nodes = vec![top, bottom];
    before_graph.layers[1].nodes = vec![left, right];
    let before = CrossingsCounter::new().count_all_crossings(&before_graph);

    let mut graph = import_graph(&input).unwrap();
    execute_processors_until(&mut graph, LayeredPhase::P3NodeOrdering).unwrap();

    let after = CrossingsCounter::new().count_all_crossings(&graph);
    assert_eq!(before, 1);
    assert_eq!(after, 0);
}

#[test]
fn execute_processors_until_p4_runs_two_sided_greedy_switch_processor() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            direction: ElkDirection::Right,
            greedy_switch_activation_threshold: 0,
            ..LayeredOptions::default()
        },
        nodes: vec![node("Top"), node("Bottom"), node("Left"), node("Right")],
        edges: vec![
            edge("Top-Right", "Top", "Right"),
            edge("Bottom-Left", "Bottom", "Left"),
        ],
    })
    .unwrap();

    let executed = execute_processors_until(&mut graph, LayeredPhase::P4NodePlacement).unwrap();

    assert!(executed.contains(&ProcessorKind::LayerSweepCrossingMinimizerBarycenter));
    assert!(executed.contains(&ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch));
    assert_eq!(executed.last(), Some(&ProcessorKind::BKNodePlacer));
}

#[test]
fn execute_processors_until_p4_runs_one_sided_greedy_switch_processor() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            direction: ElkDirection::Right,
            greedy_switch_type: GreedySwitchType::OneSided,
            greedy_switch_activation_threshold: 0,
            ..LayeredOptions::default()
        },
        nodes: vec![node("A"), node("B"), node("C")],
        edges: vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
    })
    .unwrap();

    let executed = execute_processors_until(&mut graph, LayeredPhase::P4NodePlacement).unwrap();

    assert!(executed.contains(&ProcessorKind::LayerSweepCrossingMinimizerOneSidedGreedySwitch));
    assert_eq!(executed.last(), Some(&ProcessorKind::BKNodePlacer));
}

#[test]
fn execute_processors_until_p3_runs_mermaid_down_direction_preprocessor() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        nodes: vec![node("A"), node("B"), node("C")],
        edges: vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
    })
    .unwrap();

    let executed = execute_processors_until(&mut graph, LayeredPhase::P3NodeOrdering).unwrap();

    assert_eq!(executed[0], ProcessorKind::DirectionPreprocessor);
    assert!(executed.contains(&ProcessorKind::SortByInputModelProcessor));
    assert_eq!(
        executed.last(),
        Some(&ProcessorKind::LayerSweepCrossingMinimizerBarycenter)
    );
    assert_eq!(graph.layers.len(), 3);
}

#[test]
fn execute_processors_until_p4_runs_bk_node_placer_after_prerequisites() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            direction: ElkDirection::Right,
            greedy_switch_type: GreedySwitchType::Off,
            ..LayeredOptions::default()
        },
        nodes: vec![node("A"), node("B"), node("C")],
        edges: vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
    })
    .unwrap();

    let executed = execute_processors_until(&mut graph, LayeredPhase::P4NodePlacement).unwrap();

    assert!(executed.contains(&ProcessorKind::LabelAndNodeSizeProcessor));
    assert!(executed.contains(&ProcessorKind::InnermostNodeMarginCalculator));
    assert_eq!(executed.last(), Some(&ProcessorKind::BKNodePlacer));
    for node in &graph.layerless_nodes {
        for port in &node.ports {
            if port.side == crate::graph::PortSide::East {
                assert_eq!(port.position.x, node.size.width);
            }
            if port.side == crate::graph::PortSide::West {
                assert_eq!(port.position.x, 0.0);
            }
        }
    }
    assert!(graph.layerless_nodes.iter().any(|node| {
        node.ports
            .iter()
            .any(|port| port.side != crate::graph::PortSide::Undefined)
    }));
    for layer in &graph.layers {
        let mut bottom = f64::NEG_INFINITY;
        for node in &layer.nodes {
            let lnode = &graph.layerless_nodes[*node];
            assert!(lnode.position.y - lnode.margin.top > bottom);
            bottom = lnode.position.y + lnode.size.height + lnode.margin.bottom;
        }
    }
}

#[test]
fn execute_processors_until_p5_runs_orthogonal_router() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            direction: ElkDirection::Right,
            greedy_switch_type: GreedySwitchType::Off,
            ..LayeredOptions::default()
        },
        nodes: vec![node("A"), node("B")],
        edges: vec![edge("A-B", "A", "B")],
    })
    .unwrap();

    let executed = execute_processors_until(&mut graph, LayeredPhase::P5EdgeRouting).unwrap();

    assert_eq!(executed.last(), Some(&ProcessorKind::OrthogonalEdgeRouter));
    assert!(
        graph
            .layerless_nodes
            .iter()
            .all(|node| node.position.y.is_finite())
    );
    assert!(graph.size.height > 0.0);
    assert!(graph.size.width > 0.0);
    assert!(graph.layers.iter().all(|layer| layer.size.width > 0.0));
    assert!(graph.layers.iter().all(|layer| layer.size.height > 0.0));
    assert!(graph.edges.iter().all(|edge| {
        edge.bend_points
            .iter()
            .all(|point| point.x.is_finite() && point.y.is_finite())
    }));
}

#[test]
fn source_ported_self_loop_runs_through_self_loop_lifecycle() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        nodes: vec![node("A")],
        edges: vec![edge("A-A", "A", "A")],
    })
    .unwrap();

    let executed = execute_ported_processors(&mut graph).unwrap();

    assert!(executed.contains(&ProcessorKind::SelfLoopPreProcessor));
    assert!(executed.contains(&ProcessorKind::SelfLoopPortRestorer));
    assert!(executed.contains(&ProcessorKind::SelfLoopRouter));
    assert!(executed.contains(&ProcessorKind::SelfLoopPostProcessor));
    assert!(graph.edge_source_attached(0));
    assert!(graph.edge_target_attached(0));
    assert_eq!(graph.edges[0].source.node, graph.edges[0].target.node);
    assert!(graph.edges[0].bend_points.len() >= 2);
    assert!(
        graph.edges[0]
            .bend_points
            .iter()
            .all(|point| point.x.is_finite() && point.y.is_finite())
    );
    assert!(graph.layerless_nodes[0].margin.top > 0.0);
}

#[test]
fn assembled_processors_keep_long_edge_joiner_after_p5_router() {
    let processors = kinds(&p3_options());
    let router = processors
        .iter()
        .position(|kind| *kind == ProcessorKind::OrthogonalEdgeRouter)
        .unwrap();
    let joiner = processors
        .iter()
        .position(|kind| *kind == ProcessorKind::LongEdgeJoiner)
        .unwrap();

    assert!(joiner > router);
}

#[test]
fn long_edge_joiner_processor_removes_split_dummies_after_p5() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            direction: ElkDirection::Right,
            greedy_switch_type: GreedySwitchType::Off,
            ..LayeredOptions::default()
        },
        nodes: vec![node("A"), node("B"), node("C")],
        edges: vec![
            edge("A-B", "A", "B"),
            edge("B-C", "B", "C"),
            edge("A-C", "A", "C"),
        ],
    })
    .unwrap();

    execute_processors_until(&mut graph, LayeredPhase::P5EdgeRouting).unwrap();
    assert!(
        graph
            .layerless_nodes
            .iter()
            .any(|node| node.kind == crate::graph::LNodeKind::LongEdge)
    );

    execute_processor(&mut graph, ProcessorKind::LongEdgeJoiner).unwrap();

    assert!(!graph.layers.iter().any(|layer| {
        layer
            .nodes
            .iter()
            .any(|node| graph.layerless_nodes[*node].kind == crate::graph::LNodeKind::LongEdge)
    }));
}

#[test]
fn source_ported_plain_flowchart_runs_through_reversed_edge_restorer() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            direction: ElkDirection::Right,
            greedy_switch_type: GreedySwitchType::Off,
            ..LayeredOptions::default()
        },
        nodes: vec![node("A"), node("B"), node("C")],
        edges: vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
    })
    .unwrap();

    let executed = execute_ported_processors(&mut graph).unwrap();

    assert!(executed.contains(&ProcessorKind::EndLabelSorter));
    assert!(executed.contains(&ProcessorKind::ReversedEdgeRestorer));
    assert_eq!(executed.last(), Some(&ProcessorKind::ReversedEdgeRestorer));
    assert!(graph.edges.iter().all(|edge| !edge.reversed));
}

#[test]
fn source_ported_default_elk_runs_through_two_sided_greedy_switch() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::default(),
        nodes: vec![node("A"), node("B"), node("C")],
        edges: vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
    })
    .unwrap();

    let executed = execute_ported_processors(&mut graph).unwrap();

    assert!(executed.contains(&ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch));
    assert_eq!(executed.last(), Some(&ProcessorKind::ReversedEdgeRestorer));
    assert!(graph.size.height > 0.0);
    assert!(graph.size.width > 0.0);
}

#[test]
fn source_ported_center_label_flowchart_runs_through_label_dummy_lifecycle() {
    let mut labelled = edge("A-C", "A", "C");
    labelled.labels = vec![ElkInputLabel::center("choice", 48.0, 12.0)];
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        nodes: vec![node("A"), node("B"), node("C")],
        edges: vec![edge("A-B", "A", "B"), edge("B-C", "B", "C"), labelled],
    })
    .unwrap();

    let executed = execute_ported_processors(&mut graph).unwrap();

    assert!(executed.contains(&ProcessorKind::LabelDummyInserter));
    assert!(executed.contains(&ProcessorKind::LabelDummySwitcher));
    assert!(executed.contains(&ProcessorKind::LabelDummyRemover));
    assert!(
        !graph
            .layers
            .iter()
            .flat_map(|layer| layer.nodes.iter().copied())
            .any(|node| graph.layerless_nodes[node].kind == crate::graph::LNodeKind::Label)
    );
    let restored = graph
        .edges
        .iter()
        .find(|edge| edge.id == "A-C" && !edge.labels.is_empty())
        .expect("center label should be restored to an A-C segment");
    assert_eq!(restored.labels[0].text, "choice");
}

#[test]
fn single_graph_hierarchy_work_preserves_layout_randomness_and_budget_boundaries() {
    for (nodes, edges) in [
        (vec![node("A")], vec![]),
        (
            vec![node("A"), node("B"), node("C"), node("D")],
            vec![
                edge("AD", "A", "D"),
                edge("BC", "B", "C"),
                edge("AC", "A", "C"),
                edge("BD", "B", "D"),
            ],
        ),
        (
            vec![node("A"), node("B"), node("C")],
            vec![
                edge("AB", "A", "B"),
                edge("BC", "B", "C"),
                edge("AC", "A", "C"),
                edge("BB", "B", "B"),
            ],
        ),
    ] {
        for ordering in [OrderingStrategy::None, OrderingStrategy::NodesAndEdges] {
            let options = LayeredOptions {
                direction: ElkDirection::Right,
                hierarchy_handling: crate::HierarchyHandling::IncludeChildren,
                consider_model_order_strategy: ordering,
                thoroughness: 7,
                random_seed: 31,
                ..Default::default()
            };
            let mut graph = import_graph(&ElkInputGraph {
                id: "root".into(),
                options,
                nodes: nodes.clone(),
                edges: edges.clone(),
            })
            .unwrap();
            execute_ported_compound_processors_until_processor(
                &mut graph,
                ProcessorKind::PortListSorter,
            )
            .unwrap();
            let mut expected = graph.clone();
            minimize_crossings_layer_sweep_hierarchical_with_type(
                &mut expected,
                CrossMinType::Barycenter,
            );
            let mut actual = graph.clone();
            let mut measured = BudgetWorkControl::new(usize::MAX);
            crate::p3order::sweep::minimize_single_graph_hierarchical_barycenter_with_work_control(
                &mut actual,
                &mut measured,
            )
            .unwrap();
            assert_eq!(
                actual, expected,
                "all graph state and random streams, {ordering:?}"
            );
            assert!(measured.charged > 0);
            for budget in [0, measured.charged / 2, measured.charged - 1] {
                let mut limited = BudgetWorkControl::new(budget);
                assert_eq!(crate::p3order::sweep::minimize_single_graph_hierarchical_barycenter_with_work_control(
                    &mut graph.clone(), &mut limited), Err(WorkError::Interrupted));
            }
            let mut exact = BudgetWorkControl::new(measured.charged);
            crate::p3order::sweep::minimize_single_graph_hierarchical_barycenter_with_work_control(
                &mut graph, &mut exact,
            )
            .unwrap();
            assert_eq!(graph, expected);
            assert_eq!(exact.remaining, 0);
        }
    }
}

// Reference: pinned elkjs 0.9.3, layered, DOWN, base spacing 40, padding 12.
#[test]
fn connected_components_pack_isolated_nodes_but_not_include_children() {
    for (separate, hierarchy, expected) in [
        (
            true,
            crate::HierarchyHandling::SeparateChildren,
            (160.0, 215.0),
        ),
        (
            false,
            crate::HierarchyHandling::SeparateChildren,
            (512.0, 61.0),
        ),
        (
            true,
            crate::HierarchyHandling::IncludeChildren,
            (512.0, 61.0),
        ),
    ] {
        let options = LayeredOptions {
            direction: ElkDirection::Down,
            spacing: crate::SpacingOptions::layered_base_value(40.0),
            separate_connected_components: separate,
            hierarchy_handling: hierarchy,
            padding: crate::ElkPadding::uniform(12.0),
            ..Default::default()
        };
        let nodes = ["A", "B", "C"]
            .into_iter()
            .map(|id| {
                let mut node = node(id);
                node.width = 136.0;
                node.height = 37.0;
                node
            })
            .collect();
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".into(),
            options,
            nodes,
            edges: vec![],
        })
        .unwrap();
        execute_ported_processors(&mut graph).unwrap();
        let size = graph.exported_root_size();
        assert_eq!(
            (size.width, size.height),
            expected,
            "{separate}/{hierarchy:?}"
        );
        if separate && hierarchy == crate::HierarchyHandling::SeparateChildren {
            for (index, node) in graph.layerless_nodes.iter().enumerate() {
                assert_eq!(
                    node.id,
                    ["A", "B", "C"][index],
                    "stable input arena indices"
                );
                assert_eq!(
                    (
                        node.position.x + graph.offset.x,
                        node.position.y + graph.offset.y
                    ),
                    (0.0, index as f64 * 77.0)
                );
            }
        }
    }
}

#[test]
fn connected_components_translate_nodes_and_edges_together() {
    let options = LayeredOptions {
        direction: ElkDirection::Down,
        spacing: crate::SpacingOptions::layered_base_value(40.0),
        padding: crate::ElkPadding::uniform(12.0),
        ..Default::default()
    };
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for index in 0..3 {
        let mut start = node(&format!("start{index}"));
        start.width = 14.0;
        start.height = 14.0;
        let mut end = node(&format!("end{index}"));
        end.width = 115.0;
        end.height = 37.0;
        edges.push(edge(&format!("e{index}"), &start.id, &end.id));
        nodes.extend([start, end]);
    }
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".into(),
        options,
        nodes,
        edges,
    })
    .unwrap();
    execute_ported_processors(&mut graph).unwrap();
    assert_eq!(
        graph.exported_root_size(),
        crate::LSize {
            width: 294.0,
            height: 246.0
        }
    );
    for (index, expected) in [(12.0, 66.0), (167.0, 66.0), (12.0, 197.0)]
        .into_iter()
        .enumerate()
    {
        let node = &graph.layerless_nodes[index * 2 + 1];
        assert_eq!(
            (
                node.position.x + graph.padding.left + graph.offset.x,
                node.position.y + graph.padding.top + graph.offset.y
            ),
            expected
        );
    }
    let edge = &graph.edges[2];
    for (port_ref, expected) in [(edge.source, (69.5, 157.0)), (edge.target, (69.5, 197.0))] {
        let node = &graph.layerless_nodes[port_ref.node];
        let port = &node.ports[port_ref.port];
        assert_eq!(
            (
                node.position.x
                    + port.position.x
                    + port.anchor.x
                    + graph.padding.left
                    + graph.offset.x,
                node.position.y
                    + port.position.y
                    + port.anchor.y
                    + graph.padding.top
                    + graph.offset.y
            ),
            expected
        );
    }
}

#[test]
fn connected_components_keep_root_minimum_and_restore_label_and_dummy_references() {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for component in 0..2 {
        let ids = [
            format!("c{component}A"),
            format!("c{component}B"),
            format!("c{component}C"),
        ];
        nodes.extend(ids.iter().map(|id| node(id)));
        edges.push(edge(&format!("c{component}AB"), &ids[0], &ids[1]));
        edges.push(edge(&format!("c{component}BC"), &ids[1], &ids[2]));
        let mut long = edge(&format!("c{component}AC"), &ids[0], &ids[2]);
        long.labels = vec![ElkInputLabel::center(
            format!("center{component}"),
            50.0,
            18.0,
        )];
        edges.push(long);
        let mut loop_edge = edge(&format!("c{component}BB"), &ids[1], &ids[1]);
        loop_edge.labels = vec![ElkInputLabel::center(
            format!("loop{component}"),
            32.0,
            12.0,
        )];
        edges.push(loop_edge);
    }
    let ids = nodes.iter().map(|node| node.id.clone()).collect::<Vec<_>>();
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".into(),
        options: LayeredOptions {
            direction: ElkDirection::Right,
            ..Default::default()
        },
        nodes,
        edges,
    })
    .unwrap();
    let mut with_minimum = graph.clone();
    let minimum = crate::LSize {
        width: 900.0,
        height: 800.0,
    };
    with_minimum.options.node_size_minimum = Some(minimum);
    execute_ported_processors(&mut graph).unwrap();
    execute_ported_processors(&mut with_minimum).unwrap();
    assert_eq!(with_minimum.options.node_size_minimum, Some(minimum));
    assert_eq!(with_minimum.exported_root_size(), minimum);
    for (index, id) in ids.iter().enumerate() {
        assert_eq!(&graph.layerless_nodes[index].id, id);
        assert_eq!(
            graph.layerless_nodes[index].position, with_minimum.layerless_nodes[index].position,
            "root minimum is applied after packing, through the root offset"
        );
    }
    for (index, node) in graph.layerless_nodes.iter().enumerate() {
        for (port_index, port) in node.ports.iter().enumerate() {
            assert_eq!(port.node, index);
            for &edge in &port.incoming_edges {
                assert_eq!(
                    graph.edges[edge].target,
                    crate::PortRef {
                        node: index,
                        port: port_index
                    }
                );
            }
            for &edge in &port.outgoing_edges {
                assert_eq!(
                    graph.edges[edge].source,
                    crate::PortRef {
                        node: index,
                        port: port_index
                    }
                );
            }
        }
    }
    for edge in &graph.edges {
        for endpoint in [edge.source, edge.target] {
            assert!(endpoint.port < graph.layerless_nodes[endpoint.node].ports.len());
        }
        for label in &edge.labels {
            assert!(
                label
                    .end_label_edge
                    .is_none_or(|index| index < graph.edges.len())
            );
        }
    }
    for component in 0..2 {
        for suffix in ["AC", "BB"] {
            let edge = &graph.edges[component * 4 + if suffix == "AC" { 2 } else { 3 }];
            assert_eq!(edge.id, format!("c{component}{suffix}"));
            assert_eq!(
                edge.labels.len(),
                1,
                "label must return from its dummy to the original edge"
            );
        }
    }
}

#[test]
fn connected_components_share_random_stream_instead_of_replaying_seed() {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for component in 0..2 {
        for index in 0..3 {
            let id = format!("c{component}n{index}");
            let mut n = node(&id);
            n.width = 30.0;
            n.height = 20.0;
            nodes.push(n);
            edges.push(edge(
                &format!("c{component}e{index}"),
                &id,
                &format!("c{component}n{}", (index + 1) % 3),
            ));
        }
    }
    let options = LayeredOptions {
        direction: ElkDirection::Right,
        random_seed: 1,
        ..Default::default()
    };
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".into(),
        options,
        nodes,
        edges,
    })
    .unwrap();
    execute_ported_processors(&mut graph).unwrap();
    for (component, expected) in [[0.0, 50.0, 100.0], [100.0, 0.0, 50.0]]
        .into_iter()
        .enumerate()
    {
        let nodes = &graph.layerless_nodes[component * 3..component * 3 + 3];
        let left = nodes
            .iter()
            .map(|node| node.position.x)
            .fold(f64::INFINITY, f64::min);
        let actual = nodes
            .iter()
            .map(|node| node.position.x - left)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "component {component}");
    }
}

#[test]
fn source_ported_mermaid_defaults_run_through_hierarchical_resizer_for_flat_graph() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        nodes: vec![node("A"), node("B"), node("C")],
        edges: vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
    })
    .unwrap();

    let executed = execute_ported_processors(&mut graph).unwrap();

    assert!(executed.contains(&ProcessorKind::HierarchicalNodeResizer));
    assert_eq!(
        executed.last(),
        Some(&ProcessorKind::DirectionPostprocessor)
    );
    assert!(graph.layers.is_empty());
    assert!(
        graph
            .layerless_nodes
            .iter()
            .all(|node| node.layer_index.is_none())
    );
    assert!(graph.size.height > 0.0);
    assert!(graph.size.width > 0.0);
}

#[test]
fn source_ported_compound_runner_executes_bottom_up_and_resizes_parent_node() {
    let mut cluster = node("cluster");
    cluster.width = 1.0;
    cluster.height = 1.0;
    cluster.hierarchy_handling = Some(crate::options::HierarchyHandling::IncludeChildren);
    let mut child_a = node("A");
    child_a.parent = Some("cluster".to_string());
    let mut child_b = node("B");
    child_b.parent = Some("cluster".to_string());
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        nodes: vec![cluster, child_a, child_b, node("C")],
        edges: vec![edge("A-B", "A", "B")],
    })
    .unwrap();

    let executed = execute_ported_compound_processors(&mut graph).unwrap();

    assert_eq!(executed.len(), 2);
    assert_eq!(executed[0].graph_id, "cluster");
    assert_eq!(executed[1].graph_id, "root");
    assert!(
        executed[0]
            .processors
            .contains(&ProcessorKind::HierarchicalNodeResizer)
    );
    assert!(
        !executed[0]
            .processors
            .iter()
            .any(|kind| kind.is_hierarchy_aware())
    );
    assert!(
        executed[1]
            .processors
            .iter()
            .any(|kind| kind.is_hierarchy_aware())
    );
    let cluster = graph
        .layerless_nodes
        .iter()
        .find(|node| node.id == "cluster")
        .unwrap();
    let nested = cluster.nested_graph.as_ref().unwrap();
    assert!(cluster.size.width >= nested.size.width);
    assert!(cluster.size.height >= nested.size.height);
    assert!(nested.layers.is_empty());
}

#[test]
fn compound_runner_finishes_child_tail_between_root_hierarchy_aware_processors() {
    let mut cluster = node("cluster");
    cluster.width = 1.0;
    cluster.height = 1.0;
    cluster.hierarchy_handling = Some(crate::options::HierarchyHandling::IncludeChildren);
    let mut child_a = node("A");
    child_a.parent = Some("cluster".to_string());
    let mut child_b = node("B");
    child_b.parent = Some("cluster".to_string());
    let mut options = LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down);
    options.greedy_switch_hierarchical_type = GreedySwitchType::TwoSided;
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options,
        nodes: vec![cluster, child_a, child_b, node("C")],
        edges: vec![edge("A-B", "A", "B"), edge("cluster-C", "cluster", "C")],
    })
    .unwrap();

    let executed = execute_ported_compound_processors(&mut graph).unwrap();

    let child_execution = &executed[0].processors;
    let root_execution = &executed[1].processors;
    assert!(
        root_execution
            .iter()
            .filter(|kind| kind.is_hierarchy_aware())
            .count()
            > 1
    );
    assert!(child_execution.contains(&ProcessorKind::HierarchicalNodeResizer));
    assert_eq!(
        child_execution.last(),
        Some(&ProcessorKind::DirectionPostprocessor)
    );
    let cluster = graph
        .layerless_nodes
        .iter()
        .find(|node| node.id == "cluster")
        .unwrap();
    assert!(cluster.size.width > 1.0);
    assert!(cluster.size.height > 1.0);
    assert!(cluster.nested_graph.as_ref().unwrap().layers.is_empty());
}

#[test]
fn compound_same_name_domains_preserve_parent_origins_and_routes() {
    let mut options = LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Right);
    options.hierarchy_handling = crate::options::HierarchyHandling::IncludeChildren;
    let nodes = [
        ("cluster", None),
        ("inside", Some("cluster")),
        ("peer", Some("cluster")),
        ("other", Some("cluster")),
        ("outside", None),
        ("outside2", None),
    ]
    .map(|(name, parent)| ElkInputNode {
        width: 50.0,
        height: 25.0,
        parent: parent.map(str::to_owned),
        ..node(name)
    });
    let pairs = [
        ("outside", "inside"),
        ("inside", "outside"),
        ("inside", "peer"),
        ("peer", "inside"),
        ("inside", "other"),
        ("outside2", "inside"),
    ];

    for rotation in [0, 1] {
        let mut ordered = pairs;
        ordered.rotate_left(rotation);
        let mut output = [None, None];
        for (index, root_id) in ["root", "cluster"].into_iter().enumerate() {
            let input = ElkInputGraph {
                id: root_id.to_owned(),
                options: options.clone(),
                nodes: nodes.to_vec(),
                edges: ordered
                    .into_iter()
                    .enumerate()
                    .map(|(edge_index, (src, dst))| edge(&format!("edge-{edge_index}"), src, dst))
                    .collect(),
            };
            let mut graph = import_graph(&input).unwrap();
            execute_ported_compound_processors(&mut graph).unwrap();
            let parent = &graph.layerless_nodes[0];
            let child = parent.nested_graph.as_deref().unwrap();
            assert_eq!(parent.ports.len(), 3);
            for (port_index, port) in parent.ports.iter().enumerate() {
                let dummy = port.port_dummy.as_ref().unwrap();
                assert!(dummy.belongs_to(child));
                let origin = child.layerless_nodes[dummy.node]
                    .origin_port
                    .as_ref()
                    .unwrap();
                assert!(origin.belongs_to(&graph));
                assert_eq!(
                    origin.port,
                    crate::PortRef {
                        node: 0,
                        port: port_index
                    }
                );
            }
            output[index] = Some((
                parent
                    .ports
                    .iter()
                    .map(|port| (port.side, port.position, port.anchor))
                    .collect::<Vec<_>>(),
                graph
                    .edges
                    .iter()
                    .chain(&child.edges)
                    .map(|edge| (edge.source, edge.target, edge.bend_points.clone()))
                    .collect::<Vec<_>>(),
            ));
        }
        assert_eq!(output[0], output[1]);
    }
}

#[test]
fn source_ported_compound_runner_routes_cross_hierarchy_edges() {
    let mut cluster = node("cluster");
    cluster.hierarchy_handling = Some(crate::options::HierarchyHandling::IncludeChildren);
    let mut child = node("A");
    child.parent = Some("cluster".to_string());
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        nodes: vec![cluster, child],
        edges: vec![edge("cluster-A", "cluster", "A")],
    })
    .unwrap();

    let executed = execute_ported_compound_processors(&mut graph).unwrap();

    assert_eq!(executed.len(), 2);
    assert!(
        graph
            .layerless_nodes
            .iter()
            .filter(|node| node.kind == crate::graph::LNodeKind::ExternalPort)
            .all(|node| node.layer_index.is_none())
    );
    assert!(graph.size.width > 0.0);
    assert!(graph.size.height > 0.0);
}

#[test]
fn minimum_size_and_content_alignment_match_elkjs_provider_results() {
    use crate::options::{ElkPadding, HierarchyHandling};

    // Captured from pinned elkjs 0.9.3. INCLUDE_CHILDREN applies its resizer before
    // direction postprocessing; its DOWN behavior is intentionally not normalized.
    for (direction, handling, expected_size, expected_position) in [
        (
            ElkDirection::Right,
            HierarchyHandling::SeparateChildren,
            (120.0, 80.0),
            (40.0, 0.0),
        ),
        (
            ElkDirection::Down,
            HierarchyHandling::SeparateChildren,
            (120.0, 80.0),
            (40.0, 0.0),
        ),
        (
            ElkDirection::Right,
            HierarchyHandling::IncludeChildren,
            (120.0, 80.0),
            (40.0, 0.0),
        ),
        (
            ElkDirection::Down,
            HierarchyHandling::IncludeChildren,
            (120.0, 120.0),
            (0.0, 50.0),
        ),
    ] {
        let mut child = node("a");
        child.width = 40.0;
        child.height = 20.0;
        let mut graph = import_graph(&ElkInputGraph {
            id: "r".into(),
            options: LayeredOptions {
                direction,
                hierarchy_handling: handling,
                padding: ElkPadding::uniform(0.0),
                node_size_minimum: Some(LSize {
                    width: 120.0,
                    height: 80.0,
                }),
                horizontal_content_alignment: ContentAlignment::Center,
                ..Default::default()
            },
            nodes: vec![child],
            edges: vec![],
        })
        .unwrap();
        execute_ported_processors(&mut graph).unwrap();
        let size = graph.exported_root_size();
        let child = &graph.layerless_nodes[0];
        assert_eq!(
            (size.width, size.height),
            expected_size,
            "{direction:?} {handling:?}"
        );
        assert_eq!(
            (
                child.position.x + graph.offset.x + graph.padding.left,
                child.position.y + graph.offset.y + graph.padding.top
            ),
            expected_position,
            "{direction:?} {handling:?}",
        );
    }
}

#[test]
fn compound_title_minimum_matches_elkjs_symmetric_label_grid() {
    use crate::options::{ElkPadding, HierarchyHandling, NodeLabelPlacement};

    let mut group = node("g");
    group.width = 0.0;
    group.height = 0.0;
    group.node_label_placement = NodeLabelPlacement::InsideTopCenter;
    group.label = Some(ElkInputLabel::center("label", 100.0, 10.0));
    group.nested_options = Some(Box::new(LayeredOptions {
        padding: ElkPadding::uniform(0.0),
        node_size_minimum: Some(LSize::default()),
        node_size_include_labels: true,
        ..Default::default()
    }));
    let mut child = node("a");
    child.width = 1.0;
    child.height = 1.0;
    child.parent = Some("g".into());
    let mut graph = import_graph(&ElkInputGraph {
        id: "r".into(),
        options: LayeredOptions {
            direction: ElkDirection::Right,
            hierarchy_handling: HierarchyHandling::IncludeChildren,
            ..Default::default()
        },
        nodes: vec![group, child],
        edges: vec![],
    })
    .unwrap();
    execute_ported_compound_processors(&mut graph).unwrap();
    let group = &graph.layerless_nodes[0];
    let nested = group.nested_graph.as_ref().unwrap();
    assert_eq!(
        group.size,
        LSize {
            width: 110.0,
            height: 30.0
        }
    );
    assert_eq!(
        graph.exported_root_size(),
        LSize {
            width: 134.0,
            height: 54.0
        }
    );
    assert_eq!(
        nested.layerless_nodes[0].position.y + nested.offset.y + nested.padding.top,
        15.0
    );
}

#[test]
fn non_finite_minimum_and_overflowing_title_are_rejected() {
    for width in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".into(),
            options: LayeredOptions {
                node_size_minimum: Some(LSize {
                    width,
                    height: 80.0,
                }),
                horizontal_content_alignment: ContentAlignment::Center,
                ..Default::default()
            },
            nodes: vec![node("child")],
            edges: vec![],
        })
        .unwrap();
        assert!(matches!(
            execute_ported_processors(&mut graph),
            Err(PipelineError::NodeSize(_))
        ));
        assert_eq!(graph.offset, LPoint::default());
    }
    let mut options = LayeredOptions {
        node_size_include_labels: true,
        ..Default::default()
    };
    assert!(
        options
            .include_inside_top_center_label_minimum(LSize {
                width: 40.0,
                height: f64::MAX
            })
            .is_err()
    );
    assert_eq!(options.node_size_minimum, None);
}

#[test]
fn minimum_size_defaults_only_apply_when_constraint_is_enabled() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    resize_graph_to_minimum(&mut graph).unwrap();
    assert_eq!(actual_graph_size(&graph), LSize::default());
    graph.options.node_size_minimum = Some(LSize {
        width: 0.0,
        height: -1.0,
    });
    resize_graph_to_minimum(&mut graph).unwrap();
    assert_eq!(
        actual_graph_size(&graph),
        LSize {
            width: 20.0,
            height: 20.0
        }
    );
}

#[test]
fn hierarchical_resizer_moves_east_and_south_external_ports_when_graph_grows() {
    let mut graph = LGraph::new("root", LayeredOptions::default());
    graph.graph_properties.external_ports = true;
    graph.padding.left = 5.0;
    graph.padding.right = 5.0;
    graph.padding.top = 7.0;
    graph.padding.bottom = 7.0;
    graph.size = LSize {
        width: 10.0,
        height: 20.0,
    };

    let mut east_node = LNode::new("east", 0.0, 0.0, None);
    east_node.kind = LNodeKind::ExternalPort;
    east_node.external_port_side = PortSide::East;
    east_node.position.x = 12.0;
    let east = graph.layerless_nodes.len();
    graph.layerless_nodes.push(east_node);

    let mut south_node = LNode::new("south", 0.0, 0.0, None);
    south_node.kind = LNodeKind::ExternalPort;
    south_node.external_port_side = PortSide::South;
    south_node.position.y = 25.0;
    let south = graph.layerless_nodes.len();
    graph.layerless_nodes.push(south_node);

    resize_graph_no_really_i_mean_it(
        &mut graph,
        LSize {
            width: 20.0,
            height: 34.0,
        },
        LSize {
            width: 50.0,
            height: 74.0,
        },
    );

    assert_eq!(graph.layerless_nodes[east].position.x, 42.0);
    assert_eq!(graph.layerless_nodes[south].position.y, 65.0);
    assert_eq!(graph.size.width, 40.0);
    assert_eq!(graph.size.height, 60.0);
}

#[test]
fn layered_node_resize_moves_ports_when_compound_has_no_external_ports() {
    let mut node = LNode::new("cluster", 100.0, 50.0, None);
    node.port_constraints = PortConstraints::Free;
    node.ports.push(crate::graph::LPort::new(
        "east".to_string(),
        0,
        crate::graph::PortType::Output,
    ));
    node.ports[0].set_side(PortSide::East);
    node.ports[0].position = LPoint { x: 100.0, y: 25.0 };
    node.ports.push(crate::graph::LPort::new(
        "south".to_string(),
        0,
        crate::graph::PortType::Output,
    ));
    node.ports[1].set_side(PortSide::South);
    node.ports[1].position = LPoint { x: 50.0, y: 50.0 };

    resize_layered_node(
        &mut node,
        LSize {
            width: 150.0,
            height: 100.0,
        },
        true,
        true,
    );

    assert_eq!(node.size.width, 150.0);
    assert_eq!(node.size.height, 100.0);
    assert_eq!(node.ports[0].position, LPoint { x: 150.0, y: 50.0 });
    assert_eq!(node.ports[1].position, LPoint { x: 75.0, y: 100.0 });
}

#[test]
fn layered_node_resize_keeps_external_port_positions_fixed() {
    let mut node = LNode::new("cluster", 100.0, 50.0, None);
    node.port_constraints = PortConstraints::FixedPos;
    node.ports.push(crate::graph::LPort::new(
        "external".to_string(),
        0,
        crate::graph::PortType::Input,
    ));
    node.ports[0].set_side(PortSide::West);
    node.ports[0].position = LPoint { x: -2.0, y: 20.0 };

    resize_layered_node(
        &mut node,
        LSize {
            width: 150.0,
            height: 100.0,
        },
        false,
        true,
    );

    assert_eq!(node.size.width, 150.0);
    assert_eq!(node.size.height, 100.0);
    assert_eq!(node.ports[0].position, LPoint { x: -2.0, y: 20.0 });
}

#[test]
fn reversed_edge_restorer_processor_restores_cycle_breaking_edges() {
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            direction: ElkDirection::Right,
            greedy_switch_type: GreedySwitchType::Off,
            ..LayeredOptions::default()
        },
        nodes: vec![node("A"), node("B")],
        edges: vec![edge("A-B", "A", "B"), edge("B-A", "B", "A")],
    })
    .unwrap();

    execute_processors_until(&mut graph, LayeredPhase::P5EdgeRouting).unwrap();
    assert!(graph.edges.iter().any(|edge| edge.reversed));

    execute_processor(&mut graph, ProcessorKind::ReversedEdgeRestorer).unwrap();

    assert!(graph.edges.iter().all(|edge| !edge.reversed));
}

#[test]
fn source_ported_end_label_flowchart_runs_through_end_label_lifecycle() {
    let mut head = ElkInputLabel::center("head", 20.0, 10.0);
    head.placement = crate::graph::EdgeLabelPlacement::Head;
    let mut labelled_edge = edge("A-B", "A", "B");
    labelled_edge.labels = vec![head];
    let mut graph = import_graph(&ElkInputGraph {
        id: "root".to_string(),
        options: LayeredOptions {
            direction: ElkDirection::Right,
            greedy_switch_type: GreedySwitchType::Off,
            ..LayeredOptions::default()
        },
        nodes: vec![node("A"), node("B")],
        edges: vec![labelled_edge],
    })
    .unwrap();

    let executed = execute_ported_processors(&mut graph).unwrap();

    assert!(executed.contains(&ProcessorKind::LabelSideSelector));
    assert!(executed.contains(&ProcessorKind::EndLabelPreprocessor));
    assert!(executed.contains(&ProcessorKind::EndLabelSorter));
    assert!(executed.contains(&ProcessorKind::EndLabelPostprocessor));
    let edge = graph.edges.iter().find(|edge| edge.id == "A-B").unwrap();
    let label = edge
        .labels
        .iter()
        .find(|label| label.placement == crate::graph::EdgeLabelPlacement::Head)
        .expect("head label should be restored to its original edge");
    assert_eq!(label.text, "head");
    assert_eq!(label.size.width, 20.0);
    assert_eq!(label.size.height, 10.0);
    assert!(label.position.x.is_finite());
    assert!(label.position.y.is_finite());
    assert_eq!(label.end_label_edge, Some(0));
    assert!(
        graph
            .layerless_nodes
            .iter()
            .flat_map(|node| node.ports.iter())
            .all(|port| port.labels.is_empty() && port.end_label_cell.is_none())
    );
}

#[test]
fn graph_aware_greedy_switch_matches_graph_configurator_activation_rules() {
    let mut options = LayeredOptions {
        hierarchy_handling: crate::options::HierarchyHandling::SeparateChildren,
        direction: ElkDirection::Right,
        greedy_switch_activation_threshold: 1,
        ..LayeredOptions::default()
    };
    let mut graph = LGraph::new("root", options.clone());
    graph
        .layerless_nodes
        .push(crate::graph::LNode::new("A", 10.0, 10.0, Some(0)));
    graph
        .layerless_nodes
        .push(crate::graph::LNode::new("B", 10.0, 10.0, Some(1)));
    assert!(
        !graph_kinds(&graph)
            .contains(&ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch)
    );

    options.greedy_switch_activation_threshold = 0;
    graph.options = options;
    assert!(
        graph_kinds(&graph)
            .contains(&ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch)
    );

    let mut nested = LGraph::new(
        "cluster",
        LayeredOptions {
            hierarchy_handling: crate::options::HierarchyHandling::IncludeChildren,
            greedy_switch_hierarchical_type: crate::options::GreedySwitchType::TwoSided,
            ..LayeredOptions::default()
        },
    );
    nested.parent_node_id = Some("cluster".to_string());
    assert!(
        !graph_kinds(&nested)
            .contains(&ProcessorKind::LayerSweepCrossingMinimizerTwoSidedGreedySwitch)
    );
}

fn push_test_external_dummy(graph: &mut LGraph, id: &str, side: PortSide) -> usize {
    let node = graph.layerless_nodes.len();
    let mut dummy = LNode::new(id, 0.0, 0.0, None);
    dummy.kind = LNodeKind::ExternalPort;
    dummy.external_port_side = side;
    graph.layerless_nodes.push(dummy);
    graph
        .add_port(node, PortType::Input, side.opposed(), Default::default())
        .unwrap();
    node
}

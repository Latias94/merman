use super::*;
use crate::graph::{LPoint, LayeredEdge};
use crate::options::LayeredOptions;
use crate::p3order::long_edge_target_node_preprocessing;

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

    fn used(&self) -> usize {
        self.charges.iter().sum()
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

fn scoring_graph(ids: &[&str]) -> LGraph {
    let mut graph = LGraph::new(
        "scoring",
        LayeredOptions {
            consider_model_order_strategy: OrderingStrategy::NodesAndEdges,
            consider_model_order_crossing_counter_node_influence: 10.0,
            thoroughness: 2,
            random_seed: 31,
            separate_connected_components: false,
            ..LayeredOptions::default()
        },
    );
    graph.random = JavaRandom::new(31);
    for (index, id) in ids.iter().enumerate() {
        graph
            .layerless_nodes
            .push(LNode::new(*id, 20.0, 10.0, Some(index)));
    }
    graph
}

fn connect(graph: &mut LGraph, source_node: usize, target_node: usize, order: usize) {
    let source = graph
        .add_port(
            source_node,
            PortType::Output,
            PortSide::East,
            LPoint::default(),
        )
        .unwrap();
    let target = graph
        .add_port(
            target_node,
            PortType::Input,
            PortSide::West,
            LPoint::default(),
        )
        .unwrap();
    graph
        .add_edge(LayeredEdge {
            id: format!("edge-{}", graph.edges.len()),
            source,
            target,
            source_node_id: graph.layerless_nodes[source_node].id.clone(),
            target_node_id: graph.layerless_nodes[target_node].id.clone(),
            labels: Vec::new(),
            minlen: 1,
            reversed: false,
            bend_points: Vec::new(),
            model_order: Some(order),
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
            thickness: 1.0,
            original_opposite_port: None,
            compound_segment: None,
        })
        .unwrap();
}

fn fixed_port_graph(constraints: PortConstraints, long_edge: bool) -> LGraph {
    let mut graph = scoring_graph(&["S", "A", "B"]);
    graph.options.consider_model_order_strategy = OrderingStrategy::PreferNodes;
    graph.options.consider_model_order_port_model_order = false;
    graph
        .options
        .consider_model_order_crossing_counter_node_influence = 1.0;
    graph
        .options
        .consider_model_order_crossing_counter_port_influence = 1.0;
    graph.layerless_nodes[0].port_constraints = constraints;
    graph.set_node_layer(0, 0);
    graph.set_node_layer(1, 1 + usize::from(long_edge));
    graph.set_node_layer(2, 1 + usize::from(long_edge));
    if long_edge {
        let mut dummy = LNode::new("long-edge", 0.0, 0.0, None);
        dummy.kind = LNodeKind::LongEdge;
        graph.layerless_nodes.push(dummy);
        graph.set_node_layer(3, 1);
        connect(&mut graph, 0, 3, 0);
        connect(&mut graph, 3, 2, 0);
    } else {
        connect(&mut graph, 0, 2, 0);
    }
    connect(&mut graph, 0, 1, 1);
    graph
}

fn crossed_graph() -> LGraph {
    let mut graph = scoring_graph(&["Top", "Bottom", "Left", "Right"]);
    graph.set_node_layer(0, 0);
    graph.set_node_layer(1, 0);
    graph.set_node_layer(2, 1);
    graph.set_node_layer(3, 1);
    connect(&mut graph, 0, 3, 0);
    connect(&mut graph, 1, 2, 1);
    graph
}

fn layer_order(graph: &LGraph) -> Vec<Vec<usize>> {
    graph
        .layers
        .iter()
        .map(|layer| layer.nodes.clone())
        .collect()
}

fn first_attempt(
    graph: &mut LGraph,
    control: &mut dyn WorkControl,
) -> (f64, GraphInfoHolder, JavaRandom) {
    initialize_crossing_minimization_port_ids(graph);
    let mut info = GraphInfoHolder::new(graph);
    let distributor =
        BarycenterPortDistributor::new(PortDistributorKind::NodeRelative, &info.current_node_order);
    let mut heuristic = BarycenterHeuristic::new(graph, graph.random.clone(), distributor);
    let reorder = ReorderNodePortsWorkContext::new(graph).unwrap();
    let score =
        minimize_crossings_with_counter(graph, &mut info, &mut heuristic, true, control, &reorder)
            .unwrap();
    (score, info, heuristic.random)
}

#[test]
fn fixed_port_scoring_resolves_targets_without_mutating_or_requiring_the_sort_cache() {
    for constraints in [PortConstraints::FixedOrder, PortConstraints::FixedPos] {
        for long_edge in [false, true] {
            let graph = fixed_port_graph(constraints, long_edge);
            let before = graph.clone();
            let source_layer = vec![vec![0]];
            let mut fresh = ScoringControl::new(usize::MAX);
            assert_eq!(
                count_model_order_port_changes(&graph, &source_layer, &mut fresh),
                Ok(1)
            );
            assert_eq!(graph, before);

            let mut cached = graph.clone();
            let mut preprocessing = ScoringControl::new(usize::MAX);
            long_edge_target_node_preprocessing(&mut cached, 0, &mut preprocessing).unwrap();
            let cached_before = cached.clone();
            let mut reused = ScoringControl::new(usize::MAX);
            assert_eq!(
                count_model_order_port_changes(&cached, &source_layer, &mut reused),
                Ok(1)
            );
            assert_eq!(cached, cached_before);
            assert!(preprocessing.used() > 0);
            assert_eq!(fresh.used(), preprocessing.used() + reused.used());

            for (candidate, measured) in [(&graph, &fresh), (&cached, &reused)] {
                for budget in [0, measured.used() - 1] {
                    let mut limited = ScoringControl::new(budget);
                    assert_eq!(
                        count_model_order_port_changes(candidate, &source_layer, &mut limited),
                        Err(WorkError::Interrupted)
                    );
                }
                let mut exact = ScoringControl::new(measured.used());
                assert_eq!(
                    count_model_order_port_changes(candidate, &source_layer, &mut exact),
                    Ok(1)
                );
                assert_eq!(exact.remaining, 0);
                assert_eq!(exact.charges, measured.charges);
            }
            assert_eq!(graph, before);
            assert_eq!(cached, cached_before);
        }
    }
}

#[test]
fn missing_target_resolution_observes_mid_walk_interrupts_and_fresh_retries() {
    let graph = fixed_port_graph(PortConstraints::FixedOrder, true);
    let before = graph.clone();
    let order = vec![vec![0]];
    let mut interrupted = ScoringControl::new(usize::MAX);
    interrupted.interrupt_after = Some(1);
    assert_eq!(
        count_model_order_port_changes(&graph, &order, &mut interrupted),
        Err(WorkError::Interrupted)
    );
    assert_eq!(interrupted.charges.len(), 1);
    assert_eq!(graph, before);
    let mut fresh = ScoringControl::new(usize::MAX);
    assert_eq!(
        count_model_order_port_changes(&graph, &order, &mut fresh),
        Ok(1)
    );
    assert_eq!(graph, before);
}

#[test]
fn shared_score_includes_node_and_fixed_port_weights_and_retains_fractional_scores() {
    let mut graph = crossed_graph();
    let order = vec![vec![0, 1], vec![3, 2]];
    assert_eq!(
        CrossingsCounter::new().count_all_crossings_in_order(&graph, &order),
        0
    );
    for weight in [10.0, 0.25] {
        graph
            .options
            .consider_model_order_crossing_counter_node_influence = weight;
        let mut control = ScoringControl::new(usize::MAX);
        assert_eq!(
            count_barycenter_crossing_score(&graph, &order, &mut control),
            Ok(weight)
        );
        assert!(control.used() > 0);
    }
    for constraints in [PortConstraints::FixedOrder, PortConstraints::FixedPos] {
        let graph = fixed_port_graph(constraints, false);
        let order = layer_order(&graph);
        assert_eq!(
            CrossingsCounter::new().count_all_crossings_in_order(&graph, &order),
            1
        );
        assert_eq!(
            count_barycenter_crossing_score(&graph, &order, &mut ScoringControl::new(usize::MAX)),
            Ok(2.0)
        );
    }
}

#[test]
fn port_influence_alone_and_no_strategy_keep_the_geometry_counter_gate() {
    let mut graph = fixed_port_graph(PortConstraints::FixedOrder, false);
    let order = layer_order(&graph);
    for strategy in [OrderingStrategy::PreferNodes, OrderingStrategy::None] {
        graph.options.consider_model_order_strategy = strategy;
        for (node_influence, port_influence) in [(0.0, 10.0), (0.0, 0.0)] {
            graph
                .options
                .consider_model_order_crossing_counter_node_influence = node_influence;
            graph
                .options
                .consider_model_order_crossing_counter_port_influence = port_influence;
            let mut control = ScoringControl::new(0);
            assert!(!use_node_port_order_crossing_counter(&graph));
            assert_eq!(
                count_barycenter_crossing_score(&graph, &order, &mut control),
                Ok(1.0)
            );
            assert!(control.charges.is_empty());
        }
    }
    graph
        .options
        .consider_model_order_crossing_counter_node_influence = 10.0;
    let mut control = ScoringControl::new(0);
    assert!(use_node_port_order_crossing_counter(&graph));
    assert_eq!(
        count_barycenter_crossing_score(&graph, &order, &mut control),
        Ok(1.0)
    );
    assert!(control.charges.is_empty());
}

#[test]
fn flat_attempt_keeps_a_nonzero_weighted_score_after_geometry_reaches_zero() {
    let mut graph = crossed_graph();
    let mut control = ScoringControl::new(usize::MAX);
    let (score, info, _) = first_attempt(&mut graph, &mut control);
    assert_eq!(score, 10.0);
    let best = info.currently_best_node_and_port_order.unwrap();
    assert_eq!(best.node_order, vec![vec![0, 1], vec![3, 2]]);
    assert_eq!(
        CrossingsCounter::new().count_all_crossings_in_order(&graph, &best.node_order),
        0
    );
    assert_eq!(
        count_barycenter_crossing_score(
            &graph,
            &best.node_order,
            &mut ScoringControl::new(usize::MAX)
        ),
        Ok(10.0)
    );
}

#[test]
fn flat_initial_zero_exit_requires_zero_weighted_score_and_consumes_one_direction_draw() {
    let mut graph = crossed_graph();
    graph.layers[1].nodes = vec![3, 2];
    let mut control = ScoringControl::new(usize::MAX);
    assert_eq!(first_attempt(&mut graph.clone(), &mut control).0, 10.0);

    graph.layerless_nodes[3].model_order = Some(2);
    graph.layerless_nodes[2].model_order = Some(3);
    let before = layer_order(&graph);
    let mut expected_random = graph.random.clone();
    expected_random.next_bool();
    let (score, info, random) = first_attempt(&mut graph, &mut ScoringControl::new(usize::MAX));
    assert_eq!(score, 0.0);
    assert_eq!(info.current_node_order, before);
    assert_eq!(random, expected_random);
}

#[test]
fn flat_weighted_attempts_continue_to_thoroughness_and_enforce_fresh_control_budgets() {
    let mut graph = crossed_graph();
    let mut first_only = graph.clone();
    first_only.options.thoroughness = 1;
    let mut first_control = ScoringControl::new(usize::MAX);
    minimize_crossings_layer_sweep_with_work_control(&mut first_only, &mut first_control).unwrap();

    let mut expected = graph.clone();
    let mut measured = ScoringControl::new(usize::MAX);
    let changed =
        minimize_crossings_layer_sweep_with_work_control(&mut expected, &mut measured).unwrap();
    assert!(measured.used() > first_control.used());
    assert_ne!(expected.random, first_only.random);

    let mut unrestricted = graph.clone();
    assert_eq!(minimize_crossings_layer_sweep(&mut unrestricted), changed);
    assert_eq!(unrestricted, expected);
    for budget in [0, measured.used() - 1] {
        assert_eq!(
            minimize_crossings_layer_sweep_with_work_control(
                &mut graph.clone(),
                &mut ScoringControl::new(budget)
            ),
            Err(WorkError::Interrupted)
        );
    }
    let mut exact = ScoringControl::new(measured.used());
    assert_eq!(
        minimize_crossings_layer_sweep_with_work_control(&mut graph, &mut exact),
        Ok(changed)
    );
    assert_eq!(exact.remaining, 0);
    assert_eq!(exact.charges, measured.charges);
    assert_eq!(graph, expected);
}

#[test]
fn flat_geometry_runs_keep_the_same_work_and_random_stream_with_port_influence_alone() {
    for strategy in [OrderingStrategy::NodesAndEdges, OrderingStrategy::None] {
        let mut original = crossed_graph();
        original.options.consider_model_order_strategy = strategy;
        original
            .options
            .consider_model_order_crossing_counter_node_influence = 0.0;
        let mut expected = original.clone();
        let mut measured = ScoringControl::new(usize::MAX);
        let changed =
            minimize_crossings_layer_sweep_with_work_control(&mut expected, &mut measured).unwrap();
        let mut actual = original;
        actual
            .options
            .consider_model_order_crossing_counter_port_influence = 10.0;
        let mut control = ScoringControl::new(measured.used());
        assert_eq!(
            minimize_crossings_layer_sweep_with_work_control(&mut actual, &mut control),
            Ok(changed)
        );
        actual
            .options
            .consider_model_order_crossing_counter_port_influence = 0.0;
        assert_eq!(actual, expected);
        assert_eq!(control.remaining, 0);
        assert_eq!(control.charges, measured.charges);
    }
}

#[test]
fn flat_weighted_scoring_keeps_nested_graphs_outside_the_local_sweep() {
    let mut graph = crossed_graph();
    let nested = crossed_graph();
    graph.layerless_nodes[0].nested_graph = Some(Box::new(nested.clone()));
    minimize_crossings_layer_sweep_with_work_control(
        &mut graph,
        &mut ScoringControl::new(usize::MAX),
    )
    .unwrap();
    assert_eq!(
        graph.layerless_nodes[0].nested_graph.as_deref(),
        Some(&nested)
    );
}

#[test]
fn flat_and_hierarchical_weighted_attempts_preserve_the_same_orders_and_random_stream() {
    let mut flat = crossed_graph();
    let mut hierarchical = flat.clone();
    let trace = debug_crossings_layer_sweep_hierarchical_with_work_control(
        &mut hierarchical,
        CrossMinType::Barycenter,
        &mut ScoringControl::new(usize::MAX),
    )
    .unwrap()
    .unwrap();
    minimize_crossings_layer_sweep_with_work_control(
        &mut flat,
        &mut ScoringControl::new(usize::MAX),
    )
    .unwrap();
    assert_eq!(trace.runs.len(), 2);
    assert_eq!(trace.runs[0].initial_crossings, 1.0);
    assert_eq!(trace.runs[0].crossings, 10.0);
    assert!(!trace.runs[0].early_returned);
    assert!(!trace.runs[1].first_try_with_initial_order);
    assert_eq!(layer_order(&flat), layer_order(&hierarchical));
    assert_eq!(flat.random, hierarchical.random);
}

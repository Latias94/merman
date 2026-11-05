use super::*;
use crate::graph::{LNode, LPoint, LayeredEdge, PortType};
use crate::options::LayeredOptions;
use crate::random::JavaRandom;
use crate::work::NoopWorkControl;

struct ScoringWork {
    remaining: usize,
    charges: Vec<usize>,
    interrupt_after: Option<usize>,
}

impl ScoringWork {
    fn new(remaining: usize) -> Self {
        Self {
            remaining,
            charges: Vec::new(),
            interrupt_after: None,
        }
    }
}

impl WorkControl for ScoringWork {
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

#[derive(Clone, Copy, Debug)]
enum ScoringOwner {
    Nodes,
    Ports,
}

fn count_changes(
    owner: ScoringOwner,
    graph: &LGraph,
    work: &mut dyn WorkControl,
) -> Result<usize, WorkError> {
    let layers = graph
        .layers
        .iter()
        .map(|layer| layer.nodes.clone())
        .collect::<Vec<_>>();
    match owner {
        ScoringOwner::Nodes => {
            count_model_order_node_changes(graph, &layers, OrderingStrategy::NodesAndEdges, work)
        }
        ScoringOwner::Ports => count_model_order_port_changes(graph, &layers, work),
    }
}

fn fan_in_graph(width: usize) -> LGraph {
    let mut graph = LGraph::new(
        "scoring-fan-in",
        LayeredOptions {
            consider_model_order_strategy: OrderingStrategy::NodesAndEdges,
            consider_model_order_crossing_counter_node_influence: 1.0,
            consider_model_order_crossing_counter_port_influence: 1.0,
            thoroughness: 3,
            random_seed: 31,
            ..LayeredOptions::default()
        },
    );
    graph.random = JavaRandom::new(31);
    for index in 0..width {
        graph.layerless_nodes.push(LNode::new(
            format!("leaf-{index}"),
            10.0,
            10.0,
            Some(width - index),
        ));
        graph.set_node_layer(index, 0);
    }
    graph
        .layerless_nodes
        .push(LNode::new("sink", 10.0, 10.0, Some(0)));
    graph.set_node_layer(width, 1);
    for index in 0..width {
        let source = graph
            .add_port(index, PortType::Output, PortSide::East, LPoint::default())
            .unwrap();
        let target = graph
            .add_port(width, PortType::Input, PortSide::West, LPoint::default())
            .unwrap();
        graph
            .add_edge(LayeredEdge {
                id: format!("edge-{index}"),
                source,
                target,
                source_node_id: format!("leaf-{index}"),
                target_node_id: "sink".into(),
                labels: Vec::new(),
                minlen: 1,
                reversed: false,
                bend_points: Vec::new(),
                model_order: Some(index),
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
                thickness: 1.0,
                original_opposite_port: None,
                compound_segment: None,
            })
            .unwrap();
    }
    graph
}

fn shared_port_fan_out_graph(reference_width: usize, sink_count: usize) -> LGraph {
    assert!(reference_width >= 2);
    let mut graph = LGraph::new(
        "scoring-shared-ports",
        LayeredOptions {
            consider_model_order_strategy: OrderingStrategy::NodesAndEdges,
            consider_model_order_crossing_counter_node_influence: 1.0,
            consider_model_order_crossing_counter_port_influence: 1.0,
            ..LayeredOptions::default()
        },
    );
    for index in 0..reference_width {
        graph
            .layerless_nodes
            .push(LNode::new(format!("reference-{index}"), 10.0, 10.0, None));
        graph.set_node_layer(index, 0);
    }
    let sources = [reference_width - 2, reference_width - 1].map(|node| {
        graph
            .add_port(node, PortType::Output, PortSide::East, LPoint::default())
            .unwrap()
    });
    for index in 0..sink_count {
        let sink = graph.layerless_nodes.len();
        graph
            .layerless_nodes
            .push(LNode::new(format!("sink-{index}"), 10.0, 10.0, None));
        graph.set_node_layer(sink, 1);
        for source in sources {
            let target = graph
                .add_port(sink, PortType::Input, PortSide::West, LPoint::default())
                .unwrap();
            graph
                .add_edge(LayeredEdge {
                    id: format!("edge-{}-{sink}", source.node),
                    source,
                    target,
                    source_node_id: graph.layerless_nodes[source.node].id.clone(),
                    target_node_id: graph.layerless_nodes[sink].id.clone(),
                    labels: Vec::new(),
                    minlen: 1,
                    reversed: false,
                    bend_points: Vec::new(),
                    model_order: None,
                    priority_direction: 0,
                    priority_shortness: 0,
                    priority_straightness: 0,
                    thickness: 1.0,
                    original_opposite_port: None,
                    compound_segment: None,
                })
                .unwrap();
        }
    }
    graph
}

#[test]
fn node_scoring_accounts_each_comparison_and_relation_update() {
    let graph = fan_in_graph(3);
    let before = graph.clone();
    let mut work = ScoringWork::new(27);

    assert_eq!(count_changes(ScoringOwner::Nodes, &graph, &mut work), Ok(3));
    assert_eq!(work.charges, vec![6, 2, 6, 2, 6, 5]);
    assert_eq!(work.remaining, 0);
    assert_eq!(graph, before);
}

#[test]
fn port_scoring_accounts_current_relation_sets_in_source_pair_order() {
    let graph = fan_in_graph(3);
    let before = graph.clone();
    let mut work = ScoringWork::new(19);

    assert_eq!(count_changes(ScoringOwner::Ports, &graph, &mut work), Ok(3));
    assert_eq!(work.charges, vec![3, 1, 6, 1, 6, 1, 1]);
    assert_eq!(work.remaining, 0);
    assert_eq!(graph, before);
}

#[test]
fn port_scoring_charges_each_shared_source_scan_without_comparable_node_orders() {
    let reference_width = 129;
    let sink_count = 8;
    let graph = shared_port_fan_out_graph(reference_width, sink_count);
    let before = graph.clone();
    let mut nodes = ScoringWork::new(0);
    assert_eq!(
        count_changes(ScoringOwner::Nodes, &graph, &mut nodes),
        Ok(0)
    );
    assert!(nodes.charges.is_empty());

    let mut relation_only = ScoringWork::new(3 * sink_count);
    assert_eq!(
        count_changes(ScoringOwner::Ports, &graph, &mut relation_only),
        Err(WorkError::Interrupted)
    );
    assert_eq!(relation_only.remaining, 0);

    let per_sink = 3 + reference_width - 1;
    let required = sink_count * per_sink;
    let mut exact = ScoringWork::new(required);
    assert_eq!(
        count_changes(ScoringOwner::Ports, &graph, &mut exact),
        Ok(sink_count)
    );
    assert_eq!(exact.remaining, 0);
    let mut expected = Vec::new();
    for _ in 0..sink_count {
        expected.push(3);
        expected.extend(vec![1; reference_width - 1]);
    }
    assert_eq!(exact.charges, expected);

    let mut limited = ScoringWork::new(required - 1);
    assert_eq!(
        count_changes(ScoringOwner::Ports, &graph, &mut limited),
        Err(WorkError::Interrupted)
    );
    assert_eq!(graph, before);
}

#[test]
fn port_scoring_and_sorting_only_charge_the_visited_reference_prefix() {
    let reference_width = 1_024;
    let sink_count = 8;
    let mut graph = shared_port_fan_out_graph(reference_width, sink_count);
    graph.layers[0].nodes.rotate_right(2);
    let before = graph.clone();
    let mut exact = ScoringWork::new(4 * sink_count);
    assert_eq!(
        count_changes(ScoringOwner::Ports, &graph, &mut exact),
        Ok(sink_count)
    );
    assert_eq!(exact.remaining, 0);
    assert_eq!(exact.charges, [3, 1].repeat(sink_count));

    let mut sorting = ScoringWork::new(4);
    assert_eq!(
        sorted_ports_by_model_order(
            &graph,
            reference_width,
            &graph.layers[0].nodes,
            &HashMap::new(),
            &mut sorting,
        ),
        Ok(vec![1, 0])
    );
    assert_eq!(sorting.remaining, 0);
    assert_eq!(sorting.charges, vec![3, 1]);
    assert_eq!(graph, before);
}

#[test]
fn port_comparison_rejects_before_resolving_an_unadmitted_reference() {
    let reference_width = 129;
    let graph = shared_port_fan_out_graph(reference_width, 1);
    let orders = HashMap::new();
    for admitted_prefix in [0, reference_width - 2] {
        let mut comparator = ModelOrderPortComparator::new(
            &graph,
            reference_width,
            &graph.layers[0].nodes,
            OrderingStrategy::NodesAndEdges,
            &orders,
            false,
        );
        let mut work = ScoringWork::new(3 + admitted_prefix);
        assert_eq!(
            comparator.compare_with_work(0, 1, &mut work),
            Err(WorkError::Interrupted)
        );
        assert_eq!(work.remaining, 0);
        assert_eq!(work.charges[0], 3);
        assert_eq!(work.charges[1..], vec![1; admitted_prefix]);
        assert!(comparator.bigger_than.values().all(HashSet::is_empty));
        assert!(comparator.smaller_than.values().all(HashSet::is_empty));
    }
}

#[test]
fn cached_port_comparison_does_not_charge_a_second_reference_scan() {
    let reference_width = 129;
    let graph = shared_port_fan_out_graph(reference_width, 1);
    let orders = HashMap::new();
    let mut comparator = ModelOrderPortComparator::new(
        &graph,
        reference_width,
        &graph.layers[0].nodes,
        OrderingStrategy::NodesAndEdges,
        &orders,
        false,
    );
    let mut first = ScoringWork::new(3 + reference_width - 1);
    assert_eq!(
        comparator.compare_with_work(0, 1, &mut first),
        Ok(Ordering::Greater)
    );
    assert_eq!(first.remaining, 0);
    let mut cached = ScoringWork::new(1);
    assert_eq!(
        comparator.compare_with_work(1, 0, &mut cached),
        Ok(Ordering::Less)
    );
    assert_eq!(cached.remaining, 0);
    assert_eq!(cached.charges, vec![1]);
}

#[test]
fn port_scoring_admits_the_actual_same_layer_long_edge_reference_prefix() {
    let reference_width = 129;
    let mut graph = shared_port_fan_out_graph(reference_width, 1);
    for node in 0..reference_width {
        graph.set_node_layer(node, 1);
    }
    graph.layers[1].nodes.rotate_left(1);
    for source in [reference_width - 2, reference_width - 1] {
        graph.layerless_nodes[source].kind = LNodeKind::LongEdge;
    }
    let previous = graph.layerless_nodes.len();
    graph
        .layerless_nodes
        .push(LNode::new("previous-layer", 10.0, 10.0, None));
    graph.set_node_layer(previous, 0);
    let before = graph.clone();
    assert_eq!(graph.layers[0].nodes.len(), 1);

    let mut previous_layer_bound = ScoringWork::new(3 + graph.layers[0].nodes.len());
    assert_eq!(
        count_changes(ScoringOwner::Ports, &graph, &mut previous_layer_bound),
        Err(WorkError::Interrupted)
    );
    let mut exact = ScoringWork::new(3 + reference_width - 1);
    assert_eq!(
        count_changes(ScoringOwner::Ports, &graph, &mut exact),
        Ok(1)
    );
    assert_eq!(exact.remaining, 0);
    assert_eq!(exact.charges[0], 3);
    assert_eq!(exact.charges[1..], vec![1; reference_width - 1]);
    assert_eq!(graph, before);
}

#[test]
fn node_scoring_excludes_unordered_nodes_before_pair_enumeration() {
    let mut graph = fan_in_graph(256);
    for node in &mut graph.layerless_nodes {
        node.model_order = None;
    }
    let mut work = ScoringWork::new(0);
    assert_eq!(count_changes(ScoringOwner::Nodes, &graph, &mut work), Ok(0));
    assert!(work.charges.is_empty());

    graph.layerless_nodes[0].model_order = Some(3);
    graph.layerless_nodes[128].model_order = Some(2);
    graph.layerless_nodes[255].model_order = Some(1);
    let before = graph.clone();
    let mut work = ScoringWork::new(usize::MAX);
    assert_eq!(count_changes(ScoringOwner::Nodes, &graph, &mut work), Ok(3));
    assert_eq!(work.charges.len(), 6);
    assert_eq!(graph, before);
}

#[test]
fn node_and_port_scoring_enforce_budget_boundaries_and_retry_with_fresh_control() {
    let width = 24;
    let graph = fan_in_graph(width);
    let before = graph.clone();
    let expected = width * (width - 1) / 2;

    for owner in [ScoringOwner::Nodes, ScoringOwner::Ports] {
        assert_eq!(
            count_changes(owner, &graph, &mut NoopWorkControl),
            Ok(expected)
        );
        let mut measured = ScoringWork::new(usize::MAX);
        assert_eq!(count_changes(owner, &graph, &mut measured), Ok(expected));
        let required = usize::MAX - measured.remaining;
        assert!(required > width * width, "{owner:?}: {required}");

        for budget in [0, required / 2, required - 1] {
            let mut limited = ScoringWork::new(budget);
            assert_eq!(
                count_changes(owner, &graph, &mut limited),
                Err(WorkError::Interrupted),
                "{owner:?}, budget {budget}"
            );
            assert_eq!(graph, before);
        }

        let mut fresh = ScoringWork::new(required);
        assert_eq!(count_changes(owner, &graph, &mut fresh), Ok(expected));
        assert_eq!(fresh.remaining, 0);
        assert_eq!(fresh.charges, measured.charges);
        assert_eq!(graph, before);
    }
}

#[test]
fn node_and_port_scoring_observe_mid_count_interrupts_and_allow_fresh_retry() {
    let graph = fan_in_graph(24);
    let before = graph.clone();
    for owner in [ScoringOwner::Nodes, ScoringOwner::Ports] {
        let mut interrupted = ScoringWork::new(usize::MAX);
        interrupted.interrupt_after = Some(3);
        assert_eq!(
            count_changes(owner, &graph, &mut interrupted),
            Err(WorkError::Interrupted),
            "{owner:?}"
        );
        assert_eq!(interrupted.charges.len(), 3);
        assert!(interrupted.remaining > usize::MAX / 2);
        assert_eq!(graph, before);

        let mut fresh = ScoringWork::new(usize::MAX);
        assert_eq!(count_changes(owner, &graph, &mut fresh), Ok(276));
        assert!(fresh.charges.len() > interrupted.charges.len());
        assert_eq!(graph, before);
    }
}

#[test]
fn scoring_checks_interruption_before_accessing_a_layer_owner() {
    let graph = LGraph::new("interrupted", LayeredOptions::default());
    let layers = vec![vec![usize::MAX, usize::MAX]];
    let mut nodes = ScoringWork::new(usize::MAX);
    nodes.interrupt_after = Some(0);
    assert_eq!(
        count_model_order_node_changes(
            &graph,
            &layers,
            OrderingStrategy::NodesAndEdges,
            &mut nodes,
        ),
        Err(WorkError::Interrupted)
    );
    let mut ports = ScoringWork::new(usize::MAX);
    ports.interrupt_after = Some(0);
    assert_eq!(
        count_model_order_port_changes(&graph, &layers, &mut ports),
        Err(WorkError::Interrupted)
    );
}

#[test]
fn hierarchy_scoring_preserves_graph_state_and_random_streams_with_work_control() {
    use super::sweep::{
        CrossMinType, minimize_crossings_layer_sweep_hierarchical_with_type,
        minimize_crossings_layer_sweep_hierarchical_with_work_control,
    };

    for nested in [false, true] {
        let mut graph = fan_in_graph(12);
        if nested {
            let mut child = fan_in_graph(8);
            child.random = JavaRandom::new(17);
            child.parent_node_id = Some("holder".into());
            let holder_index = graph.layerless_nodes.len();
            let mut holder = LNode::new("holder", 10.0, 10.0, Some(holder_index));
            holder.compound = true;
            holder.nested_graph = Some(Box::new(child));
            graph.layerless_nodes.push(holder);
            graph.set_node_layer(holder_index, 0);
        }
        let mut expected = graph.clone();
        let expected_changed = minimize_crossings_layer_sweep_hierarchical_with_type(
            &mut expected,
            CrossMinType::Barycenter,
        );
        let mut measured = ScoringWork::new(usize::MAX);
        let mut actual = graph.clone();
        assert_eq!(
            minimize_crossings_layer_sweep_hierarchical_with_work_control(
                &mut actual,
                CrossMinType::Barycenter,
                &mut measured,
            ),
            Ok(expected_changed)
        );
        assert_eq!(actual, expected, "nested: {nested}");
        let required = usize::MAX - measured.remaining;
        assert!(required > 0);

        let mut limited = ScoringWork::new(0);
        assert_eq!(
            minimize_crossings_layer_sweep_hierarchical_with_work_control(
                &mut graph.clone(),
                CrossMinType::Barycenter,
                &mut limited,
            ),
            Err(WorkError::Interrupted)
        );
        let mut interrupted = ScoringWork::new(usize::MAX);
        interrupted.interrupt_after = Some(3);
        assert_eq!(
            minimize_crossings_layer_sweep_hierarchical_with_work_control(
                &mut graph.clone(),
                CrossMinType::Barycenter,
                &mut interrupted,
            ),
            Err(WorkError::Interrupted)
        );

        let mut fresh = ScoringWork::new(required);
        assert_eq!(
            minimize_crossings_layer_sweep_hierarchical_with_work_control(
                &mut graph,
                CrossMinType::Barycenter,
                &mut fresh,
            ),
            Ok(expected_changed)
        );
        assert_eq!(fresh.remaining, 0);
        assert_eq!(fresh.charges, measured.charges);
        assert_eq!(graph, expected, "nested: {nested}");
    }
}

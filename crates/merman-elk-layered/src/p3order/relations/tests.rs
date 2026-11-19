use super::*;
use crate::graph::{LGraph, LNode, LNodeKind, LPoint, LayeredEdge, PortSide, PortType};
use crate::options::{LayeredOptions, OrderingStrategy};
use crate::p3order::{
    ModelOrderNodeComparator, ModelOrderPortComparator, model_order_relation_update_work,
    model_order_relation_work,
};
use crate::work::{WorkControl, WorkError};

#[derive(Clone, Default)]
struct LegacyRelations {
    bigger_than: HashMap<usize, HashSet<usize>>,
    smaller_than: HashMap<usize, HashSet<usize>>,
}

impl LegacyRelations {
    fn update(&mut self, bigger: usize, smaller: usize) {
        let bigger_than = &mut self.bigger_than;
        let smaller_than = &mut self.smaller_than;
        bigger_than.entry(bigger).or_default();
        bigger_than.entry(smaller).or_default();
        smaller_than.entry(bigger).or_default();
        smaller_than.entry(smaller).or_default();

        let smaller_bigger_than = bigger_than[&smaller].clone();
        let bigger_smaller_than = smaller_than[&bigger].clone();

        bigger_than.get_mut(&bigger).unwrap().insert(smaller);
        smaller_than.get_mut(&smaller).unwrap().insert(bigger);

        for very_small in smaller_bigger_than {
            bigger_than.get_mut(&bigger).unwrap().insert(very_small);
            smaller_than.entry(very_small).or_default().insert(bigger);
            let bigger_smaller_than = smaller_than[&bigger].clone();
            smaller_than
                .get_mut(&very_small)
                .unwrap()
                .extend(bigger_smaller_than);
        }

        for very_big in bigger_smaller_than {
            smaller_than.get_mut(&smaller).unwrap().insert(very_big);
            bigger_than.entry(very_big).or_default().insert(smaller);
            let smaller_bigger_than = bigger_than[&smaller].clone();
            bigger_than
                .get_mut(&very_big)
                .unwrap()
                .extend(smaller_bigger_than);
        }
    }

    fn cached_ordering(&mut self, first: usize, second: usize) -> Option<Ordering> {
        for key in [first, second] {
            self.bigger_than.entry(key).or_default();
            self.smaller_than.entry(key).or_default();
        }
        if self.bigger_than[&first].contains(&second) {
            return Some(Ordering::Greater);
        }
        if self.bigger_than[&second].contains(&first) {
            return Some(Ordering::Less);
        }
        if self.smaller_than[&first].contains(&second) {
            return Some(Ordering::Less);
        }
        if self.smaller_than[&second].contains(&first) {
            return Some(Ordering::Greater);
        }
        None
    }

    fn candidate(&self) -> ModelOrderRelations {
        ModelOrderRelations {
            bigger_than: self.bigger_than.clone(),
            smaller_than: self.smaller_than.clone(),
            ..ModelOrderRelations::default()
        }
    }
}

fn assert_state(candidate: &ModelOrderRelations, legacy: &LegacyRelations) {
    assert_eq!(candidate.bigger_than, legacy.bigger_than);
    assert_eq!(candidate.smaller_than, legacy.smaller_than);
}

fn assert_observations(candidate: &ModelOrderRelations, legacy: &LegacyRelations, keys: &[usize]) {
    assert_state(candidate, legacy);
    for &first in keys {
        for &second in keys {
            assert_eq!(
                model_order_relation_update_work(
                    first,
                    second,
                    &candidate.bigger_than,
                    &candidate.smaller_than,
                ),
                model_order_relation_update_work(
                    first,
                    second,
                    &legacy.bigger_than,
                    &legacy.smaller_than,
                ),
            );
            assert_eq!(
                model_order_relation_work(
                    first,
                    second,
                    &candidate.bigger_than,
                    &candidate.smaller_than,
                ),
                model_order_relation_work(first, second, &legacy.bigger_than, &legacy.smaller_than,),
            );

            let mut probe = ModelOrderRelations {
                bigger_than: candidate.bigger_than.clone(),
                smaller_than: candidate.smaller_than.clone(),
                ..ModelOrderRelations::default()
            };
            let mut reference = legacy.clone();
            assert_eq!(
                probe.cached_ordering(first, second),
                reference.cached_ordering(first, second),
            );
            assert_state(&probe, &reference);
        }
    }
}

fn update_both(
    candidate: &mut ModelOrderRelations,
    legacy: &mut LegacyRelations,
    bigger: usize,
    smaller: usize,
) {
    let smaller_count = legacy.bigger_than.get(&smaller).map_or(0, HashSet::len);
    let larger_count = legacy.smaller_than.get(&bigger).map_or(0, HashSet::len);
    candidate.update(bigger, smaller);
    legacy.update(bigger, smaller);
    assert_state(candidate, legacy);
    assert_eq!(candidate.smaller_snapshot.len(), smaller_count);
    assert_eq!(candidate.larger_snapshot.len(), larger_count);
}

#[test]
fn all_two_key_states_match_legacy_through_two_updates() {
    let keys = [7, usize::MAX];
    let pairs = [
        (keys[0], keys[0]),
        (keys[0], keys[1]),
        (keys[1], keys[0]),
        (keys[1], keys[1]),
    ];
    for state in 0..5usize.pow(4) {
        let mut seed = LegacyRelations::default();
        let mut digits = state;
        for rows in [&mut seed.bigger_than, &mut seed.smaller_than] {
            for &key in &keys {
                let row = digits % 5;
                digits /= 5;
                if row != 0 {
                    let members = keys
                        .iter()
                        .enumerate()
                        .filter_map(|(bit, &member)| {
                            ((row - 1) & (1 << bit) != 0).then_some(member)
                        })
                        .collect();
                    rows.insert(key, members);
                }
            }
        }
        assert_observations(&seed.candidate(), &seed, &keys);
        for &first in &pairs {
            for &second in &pairs {
                let mut candidate = seed.candidate();
                let mut legacy = seed.clone();
                for (bigger, smaller) in [first, second] {
                    update_both(&mut candidate, &mut legacy, bigger, smaller);
                    assert_observations(&candidate, &legacy, &keys);
                }
            }
        }
    }
}

#[test]
fn cached_ordering_retains_all_four_priorities() {
    use Ordering::{Greater, Less};
    let expected = [
        None,
        Some(Greater),
        Some(Less),
        Some(Greater),
        Some(Less),
        Some(Greater),
        Some(Less),
        Some(Greater),
        Some(Greater),
        Some(Greater),
        Some(Less),
        Some(Greater),
        Some(Less),
        Some(Greater),
        Some(Less),
        Some(Greater),
    ];
    for (mask, expected) in expected.into_iter().enumerate() {
        let mut legacy = LegacyRelations::default();
        for (bit, key, member) in [(0, 11, 23), (1, 23, 11)] {
            if mask & (1 << bit) != 0 {
                legacy.bigger_than.entry(key).or_default().insert(member);
            }
        }
        for (bit, key, member) in [(2, 11, 23), (3, 23, 11)] {
            if mask & (1 << bit) != 0 {
                legacy.smaller_than.entry(key).or_default().insert(member);
            }
        }
        let mut candidate = legacy.candidate();
        assert_eq!(candidate.cached_ordering(11, 23), expected, "mask {mask}");
        assert_eq!(legacy.cached_ordering(11, 23), expected, "mask {mask}");
        assert_state(&candidate, &legacy);
    }
}

#[test]
fn mixed_alias_updates_and_intermediate_queries_match_legacy() {
    let mut legacy = LegacyRelations {
        bigger_than: HashMap::from([
            (9, HashSet::from([9, 11, usize::MAX])),
            (11, HashSet::from([27])),
            (5, HashSet::new()),
        ]),
        smaller_than: HashMap::from([
            (11, HashSet::from([9, 11, 27])),
            (9, HashSet::from([usize::MAX])),
            (99, HashSet::new()),
        ]),
    };
    let keys = [5, 9, 11, 27, 99, usize::MAX];
    let mut candidate = legacy.candidate();
    for (bigger, smaller) in [
        (11, 9),
        (9, 11),
        (11, 11),
        (9, 9),
        (27, usize::MAX),
        (11, 9),
        (usize::MAX, 11),
        (99, 5),
        (5, 99),
    ] {
        assert_observations(&candidate, &legacy, &keys);
        update_both(&mut candidate, &mut legacy, bigger, smaller);
        for (first, second) in [(smaller, bigger), (bigger, bigger), (5, 99)] {
            assert_eq!(
                candidate.cached_ordering(first, second),
                legacy.cached_ordering(first, second),
            );
            assert_state(&candidate, &legacy);
        }
    }
    assert_observations(&candidate, &legacy, &keys);
}

#[test]
fn entry_snapshots_reuse_capacity_without_leaking_previous_members() {
    let mut legacy = LegacyRelations {
        bigger_than: HashMap::from([(50, (0..8).collect())]),
        smaller_than: HashMap::from([(40, (8..16).collect())]),
    };
    let mut candidate = legacy.candidate();
    update_both(&mut candidate, &mut legacy, 40, 50);
    let buffers = (
        candidate.smaller_snapshot.as_ptr(),
        candidate.larger_snapshot.as_ptr(),
        candidate.smaller_snapshot.capacity(),
        candidate.larger_snapshot.capacity(),
    );
    update_both(&mut candidate, &mut legacy, 1_000, 2_000);
    assert_eq!(candidate.bigger_than[&1_000], HashSet::from([2_000]));
    assert_eq!(candidate.smaller_than[&2_000], HashSet::from([1_000]));
    update_both(&mut candidate, &mut legacy, 4_000, 50);
    assert_eq!(
        (
            candidate.smaller_snapshot.as_ptr(),
            candidate.larger_snapshot.as_ptr(),
            candidate.smaller_snapshot.capacity(),
            candidate.larger_snapshot.capacity(),
        ),
        buffers,
    );
    assert_observations(&candidate, &legacy, &[0, 8, 40, 50, 1_000, 2_000, 4_000]);
}

#[test]
fn clear_drops_facts_and_reuses_scratch_without_carrying_members_forward() {
    let mut legacy = LegacyRelations {
        bigger_than: HashMap::from([(50, (0..8).collect())]),
        smaller_than: HashMap::from([(40, (8..16).collect())]),
    };
    let mut candidate = legacy.candidate();
    update_both(&mut candidate, &mut legacy, 40, 50);
    let buffers = (
        candidate.smaller_snapshot.as_ptr(),
        candidate.larger_snapshot.as_ptr(),
        candidate.smaller_snapshot.capacity(),
        candidate.larger_snapshot.capacity(),
    );
    candidate.clear();
    let mut legacy = LegacyRelations::default();
    assert_state(&candidate, &legacy);
    assert!(candidate.smaller_snapshot.is_empty());
    assert!(candidate.larger_snapshot.is_empty());
    update_both(&mut candidate, &mut legacy, 1_000, 2_000);
    assert_eq!(
        (
            candidate.smaller_snapshot.as_ptr(),
            candidate.larger_snapshot.as_ptr(),
            candidate.smaller_snapshot.capacity(),
            candidate.larger_snapshot.capacity(),
        ),
        buffers,
    );
    assert_observations(&candidate, &legacy, &[40, 50, 1_000, 2_000]);
}

#[test]
fn clone_copies_facts_without_copying_propagation_scratch() {
    let mut legacy = LegacyRelations {
        bigger_than: HashMap::from([(50, HashSet::from([40, 50, 60]))]),
        smaller_than: HashMap::from([(40, HashSet::from([40, 50, 70]))]),
    };
    let mut candidate = legacy.candidate();
    update_both(&mut candidate, &mut legacy, 40, 50);
    assert!(!candidate.smaller_snapshot.is_empty());
    assert!(!candidate.larger_snapshot.is_empty());
    let mut cloned = candidate.clone();
    assert_observations(&cloned, &legacy, &[40, 50, 60, 70]);
    assert_eq!(cloned.smaller_snapshot.capacity(), 0);
    assert_eq!(cloned.larger_snapshot.capacity(), 0);
    let mut cloned_legacy = legacy.clone();
    update_both(&mut cloned, &mut cloned_legacy, 70, 60);
    assert_observations(&cloned, &cloned_legacy, &[40, 50, 60, 70]);
    assert_state(&candidate, &legacy);
}

#[derive(Debug, PartialEq, Eq)]
enum WorkEvent {
    Check(usize),
    Charge(usize),
}

struct TraceWork {
    remaining: usize,
    events: Vec<WorkEvent>,
    reject_charge: bool,
}

impl TraceWork {
    fn new(remaining: usize) -> Self {
        Self {
            remaining,
            events: Vec::new(),
            reject_charge: false,
        }
    }
}

impl WorkControl for TraceWork {
    fn check(&mut self, units: usize) -> Result<(), WorkError> {
        self.events.push(WorkEvent::Check(units));
        if units > self.remaining {
            Err(WorkError::Interrupted)
        } else {
            Ok(())
        }
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.events.push(WorkEvent::Charge(units));
        if self.reject_charge {
            return Err(WorkError::Interrupted);
        }
        self.remaining = self
            .remaining
            .checked_sub(units)
            .ok_or(WorkError::Interrupted)?;
        Ok(())
    }
}

#[test]
fn snapshot_updates_admit_exact_work_and_preserve_state_on_check_or_charge_failure() {
    use WorkEvent::{Charge, Check};
    let crossed = LegacyRelations {
        bigger_than: HashMap::from([(40, HashSet::from([10, 20])), (50, HashSet::from([10, 20]))]),
        smaller_than: HashMap::from([(40, HashSet::from([10, 20])), (50, HashSet::from([10, 20]))]),
    };
    let reverse = LegacyRelations {
        bigger_than: HashMap::from([(40, HashSet::from([50]))]),
        smaller_than: HashMap::from([(50, HashSet::from([40]))]),
    };
    let graph = LGraph::new("snapshot-update-work", LayeredOptions::default());
    for (name, seed, large, small, required) in [
        ("empty", LegacyRelations::default(), 40, 50, 2),
        (
            "one-sided",
            LegacyRelations {
                bigger_than: HashMap::from([(50, HashSet::from([10, 20]))]),
                ..LegacyRelations::default()
            },
            40,
            50,
            8,
        ),
        ("crossed-overlap", crossed.clone(), 40, 50, 22),
        ("self", crossed, 40, 40, 22),
        ("reverse", reverse.clone(), 50, 40, 10),
        ("already-learned", reverse, 40, 50, 2),
    ] {
        let smaller_members = seed.bigger_than.get(&small).cloned().unwrap_or_default();
        let larger_members = seed.smaller_than.get(&large).cloned().unwrap_or_default();
        for (budget, reject_charge) in [(required - 1, false), (required, true), (required, false)]
        {
            let mut comparator =
                ModelOrderNodeComparator::new(&graph, &[], OrderingStrategy::NodesAndEdges);
            comparator.relations = seed.candidate();
            comparator.relations.smaller_snapshot = vec![91, 92];
            comparator.relations.larger_snapshot = vec![93];
            let buffers = (
                comparator.relations.smaller_snapshot.as_ptr(),
                comparator.relations.larger_snapshot.as_ptr(),
                comparator.relations.smaller_snapshot.capacity(),
                comparator.relations.larger_snapshot.capacity(),
            );
            let map_capacities = (
                comparator.relations.bigger_than.capacity(),
                comparator.relations.smaller_than.capacity(),
            );
            assert_eq!(
                model_order_relation_update_work(
                    large,
                    small,
                    &comparator.relations.bigger_than,
                    &comparator.relations.smaller_than,
                ),
                Ok(required),
                "{name}",
            );
            let mut work = TraceWork::new(budget);
            work.reject_charge = reject_charge;
            let result = comparator.update_bigger_smaller(large, small, &mut work);
            if budget < required || reject_charge {
                assert_eq!(result, Err(WorkError::Interrupted), "{name}");
                assert_eq!(work.remaining, budget);
                assert_state(&comparator.relations, &seed);
                assert_eq!(comparator.relations.smaller_snapshot, [91, 92]);
                assert_eq!(comparator.relations.larger_snapshot, [93]);
                assert_eq!(
                    (
                        comparator.relations.smaller_snapshot.as_ptr(),
                        comparator.relations.larger_snapshot.as_ptr(),
                        comparator.relations.smaller_snapshot.capacity(),
                        comparator.relations.larger_snapshot.capacity(),
                    ),
                    buffers,
                );
                assert_eq!(
                    (
                        comparator.relations.bigger_than.capacity(),
                        comparator.relations.smaller_than.capacity(),
                    ),
                    map_capacities,
                );
                let mut expected_events = vec![Check(required)];
                if reject_charge {
                    expected_events.push(Charge(required));
                }
                assert_eq!(work.events, expected_events);
            } else {
                assert_eq!(result, Ok(()), "{name}");
                assert_eq!(work.remaining, 0);
                assert_eq!(work.events, [Check(required), Charge(required)]);
                let mut expected = seed.clone();
                expected.update(large, small);
                assert_state(&comparator.relations, &expected);
                assert_eq!(
                    comparator
                        .relations
                        .smaller_snapshot
                        .iter()
                        .copied()
                        .collect::<HashSet<_>>(),
                    smaller_members,
                );
                assert_eq!(
                    comparator
                        .relations
                        .larger_snapshot
                        .iter()
                        .copied()
                        .collect::<HashSet<_>>(),
                    larger_members,
                );
            }
        }
    }
}

#[test]
fn port_precharge_failure_preserves_facts_keys_and_scratch_before_graph_access() {
    use WorkEvent::{Charge, Check};
    let graph = LGraph::new("snapshot-port-rejection", LayeredOptions::default());
    let orders = HashMap::new();
    let seed = LegacyRelations {
        bigger_than: HashMap::from([(50, HashSet::from([10, 20]))]),
        smaller_than: HashMap::from([(40, HashSet::from([60, 70]))]),
    };
    for (budget, reject_charge) in [(22, false), (23, true)] {
        let mut comparator = ModelOrderPortComparator::new(
            &graph,
            usize::MAX,
            &[],
            OrderingStrategy::NodesAndEdges,
            &orders,
            false,
        );
        comparator.relations = seed.candidate();
        comparator.relations.smaller_snapshot = vec![91, 92];
        comparator.relations.larger_snapshot = vec![93];
        let buffers = (
            comparator.relations.smaller_snapshot.as_ptr(),
            comparator.relations.larger_snapshot.as_ptr(),
            comparator.relations.smaller_snapshot.capacity(),
            comparator.relations.larger_snapshot.capacity(),
        );
        let map_capacities = (
            comparator.relations.bigger_than.capacity(),
            comparator.relations.smaller_than.capacity(),
        );
        let mut work = TraceWork::new(budget);
        work.reject_charge = reject_charge;
        assert_eq!(
            comparator.compare_with_work(40, 50, &mut work),
            Err(WorkError::Interrupted),
        );
        assert_eq!(work.remaining, budget);
        assert_state(&comparator.relations, &seed);
        assert_eq!(comparator.relations.smaller_snapshot, [91, 92]);
        assert_eq!(comparator.relations.larger_snapshot, [93]);
        assert_eq!(
            (
                comparator.relations.smaller_snapshot.as_ptr(),
                comparator.relations.larger_snapshot.as_ptr(),
                comparator.relations.smaller_snapshot.capacity(),
                comparator.relations.larger_snapshot.capacity(),
            ),
            buffers,
        );
        assert_eq!(
            (
                comparator.relations.bigger_than.capacity(),
                comparator.relations.smaller_than.capacity(),
            ),
            map_capacities,
        );
        let mut expected_events = vec![Check(23)];
        if reject_charge {
            expected_events.push(Charge(23));
        }
        assert_eq!(work.events, expected_events);
    }
}

fn connect(graph: &mut LGraph, source: usize, target: usize) {
    let source_port = graph
        .add_port(source, PortType::Output, PortSide::East, LPoint::default())
        .unwrap();
    let target_port = graph
        .add_port(target, PortType::Input, PortSide::West, LPoint::default())
        .unwrap();
    graph
        .add_edge(LayeredEdge {
            id: format!("{source}-{target}"),
            source: source_port,
            target: target_port,
            source_node_id: graph.layerless_nodes[source].id.clone(),
            target_node_id: graph.layerless_nodes[target].id.clone(),
            labels: Vec::new(),
            minlen: 1,
            reversed: false,
            bend_points: Vec::new(),
            model_order: None,
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
            thickness: 0.0,
            original_opposite_port: None,
            compound_segment: None,
        })
        .unwrap();
}

#[test]
fn feedback_admission_preserves_each_snapshot_prefix_and_legacy_query_result() {
    use WorkEvent::{Charge, Check};
    let mut graph = LGraph::new("relation-feedback", LayeredOptions::default());
    for (index, layer, kind) in [
        (0, 0, LNodeKind::Normal),
        (1, 1, LNodeKind::Normal),
        (2, 2, LNodeKind::Normal),
        (3, 1, LNodeKind::LongEdge),
        (4, 1, LNodeKind::LongEdge),
    ] {
        let mut node = LNode::new(format!("node-{index}"), 10.0, 10.0, None);
        node.kind = kind;
        graph.layerless_nodes.push(node);
        graph.set_node_layer(index, layer);
    }
    for (source, target) in [(0, 4), (4, 1), (1, 3), (3, 2)] {
        connect(&mut graph, source, target);
    }
    for budget in [10, 11, 20, 21] {
        let mut comparator = ModelOrderNodeComparator::new(
            &graph,
            &graph.layers[0].nodes,
            OrderingStrategy::NodesAndEdges,
        );
        comparator.before_ports = true;
        let mut work = TraceWork::new(budget);
        let result = comparator.compare_with_work(3, 4, &mut work);
        let mut legacy = LegacyRelations::default();
        assert_eq!(legacy.cached_ordering(3, 4), None);
        let mut expected_events = vec![Check(9), Charge(9), Check(2)];
        if budget >= 11 {
            legacy.update(4, 3);
            expected_events.extend([Charge(2), Check(10)]);
        }
        if budget == 21 {
            legacy.update(3, 4);
            expected_events.push(Charge(10));
            assert_eq!(result, Ok(Ordering::Greater));
        } else {
            assert_eq!(result, Err(WorkError::Interrupted));
        }
        assert_eq!(work.events, expected_events);
        assert_observations(&comparator.relations, &legacy, &[3, 4]);
        if budget == 21 {
            assert_eq!(legacy.cached_ordering(3, 4), Some(Ordering::Greater));
            assert_eq!(legacy.cached_ordering(4, 3), Some(Ordering::Greater));
            assert!(comparator.relations.bigger_than[&3].contains(&3));
            assert!(comparator.relations.smaller_than[&4].contains(&4));
        }
    }
}

#[test]
fn helper_continuation_records_a_node_outside_the_sorted_layer() {
    let mut graph = LGraph::new("relation-domain", LayeredOptions::default());
    for (index, layer, order) in [
        (0, 0, Some(5)),
        (1, 1, Some(7)),
        (2, 1, Some(9)),
        (3, 1, None),
    ] {
        graph
            .layerless_nodes
            .push(LNode::new(format!("node-{index}"), 10.0, 10.0, order));
        graph.set_node_layer(index, layer);
    }
    graph.layerless_nodes[3].kind = LNodeKind::LongEdge;
    connect(&mut graph, 0, 3);
    connect(&mut graph, 3, 1);
    assert!(!graph.layers[1].nodes.contains(&0));
    let mut comparator = ModelOrderNodeComparator::new(
        &graph,
        &graph.layers[0].nodes,
        OrderingStrategy::NodesAndEdges,
    );
    let mut work = TraceWork::new(13);
    assert_eq!(
        comparator.compare_with_work(3, 2, &mut work),
        Ok(Ordering::Less)
    );
    assert_eq!(work.remaining, 0);
    let mut legacy = LegacyRelations::default();
    legacy.cached_ordering(3, 2);
    legacy.cached_ordering(0, 2);
    legacy.update(2, 0);
    legacy.update(2, 3);
    assert_observations(&comparator.relations, &legacy, &[0, 1, 2, 3]);
    assert!(comparator.relations.bigger_than[&2].contains(&0));
}

#[test]
fn equal_node_model_orders_learn_the_first_comparison_direction() {
    let mut graph = LGraph::new("relation-ties", LayeredOptions::default());
    for index in 0..2 {
        graph
            .layerless_nodes
            .push(LNode::new(format!("node-{index}"), 10.0, 10.0, Some(7)));
    }
    for (first, second) in [(0, 1), (1, 0)] {
        let mut comparator =
            ModelOrderNodeComparator::new(&graph, &[], OrderingStrategy::PreferNodes);
        let mut work = TraceWork::new(4);
        assert_eq!(
            comparator.compare_with_work(first, second, &mut work),
            Ok(Ordering::Less),
        );
        let mut legacy = LegacyRelations::default();
        legacy.update(second, first);
        assert_observations(&comparator.relations, &legacy, &[0, 1]);
        assert_eq!(
            comparator.compare_with_work(second, first, &mut work),
            Ok(Ordering::Greater),
        );
        assert_eq!(work.remaining, 0);
        assert_state(&comparator.relations, &legacy);
    }
}

#[test]
fn port_self_comparison_keeps_legacy_admission_and_non_equal_results() {
    use WorkEvent::{Charge, Check};
    let mut graph = LGraph::new("relation-port-self", LayeredOptions::default());
    graph
        .layerless_nodes
        .push(LNode::new("owner", 10.0, 10.0, None));
    graph
        .add_port(0, PortType::Output, PortSide::East, LPoint::default())
        .unwrap();
    let target_orders = HashMap::new();
    let mut comparator = ModelOrderPortComparator::new(
        &graph,
        0,
        &[],
        OrderingStrategy::NodesAndEdges,
        &target_orders,
        false,
    );
    let mut rejected = TraceWork::new(2);
    assert_eq!(
        comparator.compare_with_work(0, 0, &mut rejected),
        Err(WorkError::Interrupted),
    );
    assert_eq!(rejected.events, [Check(3)]);
    assert_state(&comparator.relations, &LegacyRelations::default());

    let mut work = TraceWork::new(4);
    assert_eq!(
        comparator.compare_with_work(0, 0, &mut work),
        Ok(Ordering::Less),
    );
    let mut legacy = LegacyRelations::default();
    legacy.update(0, 0);
    assert_observations(&comparator.relations, &legacy, &[0]);
    assert_eq!(
        comparator.compare_with_work(0, 0, &mut work),
        Ok(Ordering::Greater),
    );
    assert_eq!(work.events, [Check(3), Charge(3), Check(1), Charge(1)]);
    assert_eq!(work.remaining, 0);
    assert_state(&comparator.relations, &legacy);
}

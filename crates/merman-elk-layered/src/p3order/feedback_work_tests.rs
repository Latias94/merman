use super::*;
use crate::graph::{LNode, LPoint, LayeredEdge, PortType};
use crate::options::LayeredOptions;

struct FeedbackWorkBudget {
    remaining: usize,
    charges: Vec<usize>,
    rejected: Option<usize>,
}

impl FeedbackWorkBudget {
    fn new(remaining: usize) -> Self {
        Self {
            remaining,
            charges: Vec::new(),
            rejected: None,
        }
    }
}

impl WorkControl for FeedbackWorkBudget {
    fn check(&mut self, units: usize) -> Result<(), WorkError> {
        if units > self.remaining {
            self.rejected = Some(units);
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

fn feedback_graph() -> (LGraph, usize, usize) {
    let mut graph = LGraph::new("feedback-work", LayeredOptions::default());
    for (id, layer, kind) in [
        ("source", 0, LNodeKind::Normal),
        ("reference", 1, LNodeKind::Normal),
        ("target", 2, LNodeKind::Normal),
        ("first", 1, LNodeKind::LongEdge),
        ("second", 1, LNodeKind::LongEdge),
    ] {
        let node = graph.layerless_nodes.len();
        let mut data = LNode::new(id, 10.0, 10.0, None);
        data.kind = kind;
        graph.layerless_nodes.push(data);
        graph.set_node_layer(node, layer);
    }

    let first = 3;
    let second = 4;
    for (source, target) in [(0, second), (second, 1), (1, first), (first, 2)] {
        let source_port = graph
            .add_port(source, PortType::Output, PortSide::East, LPoint::default())
            .unwrap();
        let target_port = graph
            .add_port(target, PortType::Input, PortSide::West, LPoint::default())
            .unwrap();
        graph
            .add_edge(LayeredEdge {
                id: format!("feedback-{source}-{target}"),
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
    (graph, first, second)
}

#[test]
fn before_ports_feedback_stops_before_unadmitted_reverse_update() {
    let (graph, first, second) = feedback_graph();
    let before = graph.clone();
    let previous = &graph.layers[0].nodes;
    let mut comparator =
        ModelOrderNodeComparator::new(&graph, previous, OrderingStrategy::NodesAndEdges);
    comparator.before_ports = true;
    let mut work = FeedbackWorkBudget::new(20);

    assert_eq!(
        comparator.compare_with_work(first, second, &mut work),
        Err(WorkError::Interrupted)
    );
    assert_eq!(work.charges, vec![9, 2]);
    assert_eq!(work.rejected, Some(10));
    assert_eq!(work.remaining, 9);
    assert_eq!(
        comparator.relations.bigger_members(second),
        HashSet::from([first])
    );
    assert_eq!(
        comparator.relations.smaller_members(first),
        HashSet::from([second])
    );
    assert!(comparator.relations.bigger_members(first).is_empty());
    assert!(comparator.relations.smaller_members(second).is_empty());
    assert_eq!(graph, before);
}

#[test]
fn before_ports_feedback_sort_admits_snapshot_reverse_work() {
    let (graph, first, second) = feedback_graph();
    let before = graph.clone();
    let mut work = FeedbackWorkBudget::new(21);

    assert_eq!(
        sorted_nodes_by_model_order(
            &graph,
            &[first, second],
            &graph.layers[0].nodes,
            true,
            &mut work,
        ),
        Ok(vec![second, first])
    );
    assert_eq!(work.charges, vec![9, 2, 10]);
    assert_eq!(work.rejected, None);
    assert_eq!(work.remaining, 0);
    assert_eq!(graph, before);
}

#[test]
fn model_order_work_admits_new_crossing_closure() {
    let mut relations = ModelOrderRelations::default();
    for owner in [0, 1, 3, 4] {
        relations.update(owner, owner + 1);
    }
    for large in 0..3 {
        for small in 3..6 {
            assert!(!relations.bigger_members(large).contains(&small));
            assert!(!relations.smaller_members(small).contains(&large));
        }
    }
    assert_eq!(relations.update_work(2, 3), Ok(22));
    assert_eq!(relations.comparison_work(2, 3), Ok((23, None)));

    relations.update(2, 3);

    for owner in 0..6 {
        assert_eq!(
            relations.bigger_members(owner),
            (owner + 1..6).collect::<HashSet<_>>()
        );
        assert_eq!(
            relations.smaller_members(owner),
            (0..owner).collect::<HashSet<_>>()
        );
    }
    assert_eq!(
        relations.comparison_work(2, 3),
        Ok((1, Some(Ordering::Greater)))
    );
}

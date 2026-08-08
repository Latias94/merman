//! Add border segments for compound graphs.
//!
//! Dagre materializes per-rank `"border"` dummy nodes along each cluster so the ordering and
//! positioning steps can route edges around clusters. This mirrors upstream `add-border-segments.js`.

use crate::graphlib::Graph;
use crate::work::{checked_add, checked_mul, checked_unparented_leaf_parent_batch_work};
use crate::{EdgeLabel, GraphLabel, NodeLabel};
use crate::{NoopWorkControl, WorkControl, WorkError};
use rustc_hash::FxHashMap;

#[derive(Default)]
struct DummyNodeIdGen {
    next_suffix: FxHashMap<&'static str, usize>,
}

impl DummyNodeIdGen {
    fn add_dummy_node(
        &mut self,
        g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
        label: NodeLabel,
        prefix: &'static str,
    ) -> Result<String, WorkError> {
        let suffix = match self.next_suffix.get(&prefix).copied() {
            Some(v) => v,
            None => {
                if !g.has_node(prefix) {
                    g.set_node(prefix, label);
                    self.next_suffix.insert(prefix, 1);
                    return Ok(prefix.to_string());
                }
                self.next_suffix.insert(prefix, 1);
                1
            }
        };

        // The legacy port used `for i in 1.. { format!("{prefix}{i}") ; has_node(...) }`,
        // which is O(n^2) and alloc-heavy. Keep the exact naming scheme but use a per-prefix
        // monotonic counter to make the common case O(1).
        let mut next = suffix;
        loop {
            let id = format!("{prefix}{next}");
            if !g.has_node(&id) {
                g.set_node(id.clone(), label);
                self.next_suffix.insert(prefix, checked_add(next, 1)?);
                return Ok(id);
            }
            next = checked_add(next, 1)?;
        }
    }
}

pub fn add_border_segments(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let mut work_control = NoopWorkControl;
    add_border_segments_controlled(g, &mut work_control)
        .expect("the checked no-op Dugong work control cannot reject border-segment work");
}

pub(crate) fn add_border_segments_controlled(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<(), WorkError> {
    if !g.options().compound {
        return Ok(());
    }
    // Admit the metadata scan before inspecting rank spans, then admit the complete expansion
    // before allocating traversal snapshots, border arrays, dummy nodes, edges, or parent state.
    work_control.charge(g.node_order_slot_count())?;
    let work_plan = checked_border_segment_work_plan(g)?;
    work_control.charge(work_plan.execution_work_units)?;

    let roots: Vec<String> = g
        .children_root()
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    let mut ids = DummyNodeIdGen::default();
    let mut parent_assignments = Vec::with_capacity(work_plan.parent_assignment_count);

    let mut stack: Vec<(String, bool)> = roots.into_iter().rev().map(|v| (v, false)).collect();
    while let Some((v, expanded)) = stack.pop() {
        if expanded {
            add_border_segments_for_node(g, &v, &mut ids, &mut parent_assignments)?;
            continue;
        }

        stack.push((v.clone(), true));
        let children: Vec<String> = g.children_iter(&v).map(|s| s.to_string()).collect();
        for child in children.into_iter().rev() {
            stack.push((child, false));
        }
    }
    debug_assert_eq!(parent_assignments.len(), work_plan.parent_assignment_count);
    if !parent_assignments.is_empty() {
        g.try_set_unparented_leaf_parents_ix(&parent_assignments)
            .expect("fresh border segments must satisfy the construction-only parent batch");
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BorderSegmentWorkPlan {
    execution_work_units: usize,
    parent_assignment_count: usize,
}

fn checked_border_segment_work_plan(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<BorderSegmentWorkPlan, WorkError> {
    let mut span_work = 0usize;
    let mut array_slots = 0usize;
    let mut parent_assignment_count = 0usize;
    let mut hierarchy_order_slots = g.root_child_order_slot_count();
    let mut arithmetic_error = None;
    g.for_each_node(|id, node| {
        if arithmetic_error.is_some() {
            return;
        }
        let result = (|| {
            hierarchy_order_slots =
                checked_add(hierarchy_order_slots, g.child_order_slot_count(id))?;
            let Some((min_rank, max_rank)) = node.min_rank.zip(node.max_rank) else {
                return Ok(());
            };
            if max_rank < min_rank {
                return Ok(());
            }
            let span = usize::try_from(i64::from(max_rank) - i64::from(min_rank) + 1)
                .map_err(|_| WorkError::ArithmeticOverflow)?;
            span_work = checked_add(span_work, checked_mul(span, 6)?)?;
            parent_assignment_count = checked_add(parent_assignment_count, checked_mul(span, 2)?)?;
            let slots = usize::try_from(i64::from(max_rank.max(0)) + 1)
                .map_err(|_| WorkError::ArithmeticOverflow)?;
            array_slots = checked_add(array_slots, checked_mul(slots, 2)?)?;
            Ok(())
        })();
        if let Err(error) = result {
            arithmetic_error = Some(error);
        }
    });
    if let Some(error) = arithmetic_error {
        return Err(error);
    }

    let future_node_slots = checked_add(g.node_slot_count(), parent_assignment_count)?;
    let parent_work =
        checked_unparented_leaf_parent_batch_work(future_node_slots, parent_assignment_count, 0)?;
    let execution_work_units = checked_add(
        checked_add(hierarchy_order_slots, checked_add(span_work, array_slots)?)?,
        parent_work,
    )?;
    Ok(BorderSegmentWorkPlan {
        execution_work_units,
        parent_assignment_count,
    })
}

fn add_border_segments_for_node(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    v: &str,
    ids: &mut DummyNodeIdGen,
    parent_assignments: &mut Vec<(usize, usize)>,
) -> Result<(), WorkError> {
    let Some((min_rank, max_rank)) = g.node(v).and_then(|n| Some((n.min_rank?, n.max_rank?)))
    else {
        return Ok(());
    };

    if max_rank < min_rank {
        return Ok(());
    }
    let max_rank_usize =
        usize::try_from(i64::from(max_rank.max(0))).map_err(|_| WorkError::ArithmeticOverflow)?;
    let border_slots = checked_add(max_rank_usize, 1)?;
    if let Some(n) = g.node_mut(v) {
        n.border_left = vec![None; border_slots];
        n.border_right = vec![None; border_slots];
    }

    let mut prev_left: Option<String> = None;
    let mut prev_right: Option<String> = None;

    for rank in min_rank..=max_rank {
        let left = add_border_node(g, ids, v, rank, BorderSide::Left, parent_assignments)?;
        if let Some(prev) = prev_left {
            g.set_edge_with_label(
                prev,
                left.clone(),
                EdgeLabel {
                    weight: 1.0,
                    ..Default::default()
                },
            );
        }
        prev_left = Some(left);

        let right = add_border_node(g, ids, v, rank, BorderSide::Right, parent_assignments)?;
        if let Some(prev) = prev_right {
            g.set_edge_with_label(
                prev,
                right.clone(),
                EdgeLabel {
                    weight: 1.0,
                    ..Default::default()
                },
            );
        }
        prev_right = Some(right);
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum BorderSide {
    Left,
    Right,
}

fn add_border_node(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ids: &mut DummyNodeIdGen,
    sg: &str,
    rank: i32,
    side: BorderSide,
    parent_assignments: &mut Vec<(usize, usize)>,
) -> Result<String, WorkError> {
    let (border_type, prefix) = match side {
        BorderSide::Left => ("borderLeft", "_bl"),
        BorderSide::Right => ("borderRight", "_br"),
    };
    let curr = ids.add_dummy_node(
        g,
        NodeLabel {
            width: 0.0,
            height: 0.0,
            rank: Some(rank),
            dummy: Some("border".to_string()),
            border_type: Some(border_type.to_string()),
            ..Default::default()
        },
        prefix,
    )?;

    if let Some(n) = g.node_mut(sg) {
        let idx = rank.max(0) as usize;
        match side {
            BorderSide::Left => n.border_left[idx] = Some(curr.clone()),
            BorderSide::Right => n.border_right[idx] = Some(curr.clone()),
        }
    }

    let child_ix = g
        .node_ix(&curr)
        .expect("a freshly inserted border node must have a node index");
    let parent_ix = g
        .node_ix(sg)
        .expect("the border segment owner must remain live");
    parent_assignments.push((child_ix, parent_ix));
    Ok(curr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphlib::GraphOptions;

    #[derive(Default)]
    struct RejectSecondCharge {
        calls: Vec<usize>,
    }

    impl WorkControl for RejectSecondCharge {
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.calls.push(units);
            if self.calls.len() == 2 {
                Err(WorkError::Interrupted)
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn controlled_border_segments_reject_after_planning_before_mutation() {
        let mut graph = Graph::new(GraphOptions {
            compound: true,
            ..GraphOptions::default()
        });
        graph.set_graph(GraphLabel::default());
        graph.set_node(
            "cluster",
            NodeLabel {
                min_rank: Some(1),
                max_rank: Some(3),
                ..NodeLabel::default()
            },
        );
        graph.set_parent("leaf", "cluster");

        let before_nodes = graph.node_ids();
        let before_children = graph
            .children("cluster")
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let before_cluster = graph.node("cluster").cloned().expect("cluster label");
        let planning_work = graph.node_order_slot_count();
        let mut work_control = RejectSecondCharge::default();

        let error = add_border_segments_controlled(&mut graph, &mut work_control).unwrap_err();

        assert_eq!(error, WorkError::Interrupted);
        assert_eq!(work_control.calls.len(), 2);
        assert_eq!(work_control.calls[0], planning_work);
        assert_eq!(graph.node_ids(), before_nodes);
        assert_eq!(graph.children("cluster"), before_children);
        assert_eq!(graph.node("cluster"), Some(&before_cluster));
    }

    #[test]
    fn border_parent_batch_preserves_postorder_rank_and_side_order() {
        let mut graph = Graph::new(GraphOptions {
            compound: true,
            ..GraphOptions::default()
        });
        graph.set_graph(GraphLabel::default());
        graph.set_node(
            "cluster",
            NodeLabel {
                min_rank: Some(1),
                max_rank: Some(2),
                ..NodeLabel::default()
            },
        );
        graph.set_parent("leaf", "cluster");

        add_border_segments(&mut graph);

        let cluster = graph.node("cluster").cloned().expect("cluster label");
        let left_1 = cluster.border_left[1].as_deref().expect("left rank 1");
        let right_1 = cluster.border_right[1].as_deref().expect("right rank 1");
        let left_2 = cluster.border_left[2].as_deref().expect("left rank 2");
        let right_2 = cluster.border_right[2].as_deref().expect("right rank 2");
        assert_eq!(
            graph.children("cluster"),
            vec!["leaf", left_1, right_1, left_2, right_2]
        );
    }
}

//! Add border segments for compound graphs.
//!
//! Dagre materializes per-rank `"border"` dummy nodes along each cluster so the ordering and
//! positioning steps can route edges around clusters. This mirrors upstream `add-border-segments.js`.

use crate::graphlib::Graph;
use crate::work::{checked_add, checked_mul};
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
    work_control.charge(border_segment_work_units(g)?)?;

    let roots: Vec<String> = g
        .children_root()
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    let mut ids = DummyNodeIdGen::default();

    let mut stack: Vec<(String, bool)> = roots.into_iter().rev().map(|v| (v, false)).collect();
    while let Some((v, expanded)) = stack.pop() {
        if expanded {
            add_border_segments_for_node(g, &v, &mut ids)?;
            continue;
        }

        stack.push((v.clone(), true));
        let children: Vec<String> = g.children_iter(&v).map(|s| s.to_string()).collect();
        for child in children.into_iter().rev() {
            stack.push((child, false));
        }
    }
    Ok(())
}

fn border_segment_work_units(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<usize, WorkError> {
    let mut span_work = 0usize;
    let mut array_slots = 0usize;
    for id in g.node_ids() {
        let Some((min_rank, max_rank)) = g.node(&id).and_then(|n| Some((n.min_rank?, n.max_rank?)))
        else {
            continue;
        };
        if max_rank < min_rank {
            continue;
        }
        let span = usize::try_from(i64::from(max_rank) - i64::from(min_rank) + 1)
            .map_err(|_| WorkError::ArithmeticOverflow)?;
        span_work = checked_add(span_work, checked_mul(span, 6)?)?;
        let slots = usize::try_from(i64::from(max_rank.max(0)) + 1)
            .map_err(|_| WorkError::ArithmeticOverflow)?;
        array_slots = checked_add(array_slots, checked_mul(slots, 2)?)?;
    }
    checked_add(
        checked_add(g.node_count(), g.edge_count())?,
        checked_add(span_work, array_slots)?,
    )
}

fn add_border_segments_for_node(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    v: &str,
    ids: &mut DummyNodeIdGen,
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
        let left = add_border_node(g, ids, "borderLeft", "_bl", v, rank, true)?;
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

        let right = add_border_node(g, ids, "borderRight", "_br", v, rank, false)?;
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

fn add_border_node(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ids: &mut DummyNodeIdGen,
    prop: &str,
    prefix: &'static str,
    sg: &str,
    rank: i32,
    is_left: bool,
) -> Result<String, WorkError> {
    let curr = ids.add_dummy_node(
        g,
        NodeLabel {
            width: 0.0,
            height: 0.0,
            rank: Some(rank),
            dummy: Some("border".to_string()),
            border_type: Some(prop.to_string()),
            ..Default::default()
        },
        prefix,
    )?;

    if let Some(n) = g.node_mut(sg) {
        let idx = rank.max(0) as usize;
        if is_left {
            n.border_left[idx] = Some(curr.clone());
        } else {
            n.border_right[idx] = Some(curr.clone());
        }
    }

    g.set_parent_ref(curr.as_str(), sg);
    Ok(curr)
}

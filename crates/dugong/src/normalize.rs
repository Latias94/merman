//! Normalize long edges by inserting dummy nodes.
//!
//! This mirrors Dagre's `normalize.js` / `undo.js` behavior. It materializes intermediate
//! `"edge"` / `"edge-label"` nodes so ranking/ordering/positioning can treat long edges as a
//! chain of rank-adjacent edges.

use crate::graphlib::{EdgeKey, Graph};
use crate::{DummyEdge, DummyKind, EdgeLabel, GraphLabel, NodeLabel, OriginalEdge, Point};
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Default)]
struct DummyNodeIdGen {
    next_suffix: FxHashMap<&'static str, usize>,
}

impl DummyNodeIdGen {
    fn unique_id(
        &mut self,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        prefix: &'static str,
    ) -> String {
        let suffix = match self.next_suffix.get(&prefix).copied() {
            Some(v) => v,
            None => {
                if !g.has_node(prefix) {
                    self.next_suffix.insert(prefix, 1);
                    return prefix.to_string();
                }
                self.next_suffix.insert(prefix, 1);
                1
            }
        };

        // Keep the legacy naming scheme (`prefix`, `prefix1`, `prefix2`, ...) while avoiding
        // repeated scans from suffix 1 for every dummy node.
        let mut next = suffix;
        loop {
            let id = format!("{prefix}{next}");
            if !g.has_node(&id) {
                self.next_suffix.insert(prefix, next + 1);
                return id;
            }
            next += 1;
        }
    }
}

fn add_dummy_node(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ids: &mut DummyNodeIdGen,
    label: NodeLabel,
    prefix: &'static str,
) -> String {
    let id = ids.unique_id(g, prefix);
    g.set_node(id.clone(), label);
    id
}

/// Splits long edges into rank-adjacent segments.
///
/// The first dummy of each chain owns the original edge key and label.
pub fn run(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let to_normalize: Vec<_> = g
        .edges()
        .filter(|e| {
            let v_rank = g.node(&e.v).and_then(|n| n.rank).unwrap_or(0);
            let w_rank = g.node(&e.w).and_then(|n| n.rank).unwrap_or(0);
            w_rank != v_rank + 1
        })
        .cloned()
        .collect();
    run_planned(g, to_normalize);
}

/// Applies a caller-prepared stable edge snapshot.
///
/// The controlled layout pipeline prepares this list behind its work boundary so the graph scan
/// and key allocation are admitted before normalization mutates the temporary layout graph.
pub(crate) fn run_planned(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    to_normalize: Vec<EdgeKey>,
) {
    g.graph_mut().dummy_chains.clear();
    let mut ids = DummyNodeIdGen::default();
    for e in to_normalize {
        normalize_edge(g, &mut ids, e);
    }
}

fn normalize_edge(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ids: &mut DummyNodeIdGen,
    e: EdgeKey,
) {
    let v_rank = g.node(&e.v).and_then(|n| n.rank).unwrap_or(0);
    let w_rank = g.node(&e.w).and_then(|n| n.rank).unwrap_or(0);
    let Some(label) = g.edge_mut_by_key(&e) else {
        return;
    };
    let first_rank = v_rank + 1;
    if w_rank == first_rank {
        return;
    }

    let mut original_label = std::mem::take(label);
    let _ = g.remove_edge_key(&e);

    original_label.points = Vec::new();
    let weight = original_label.weight;
    let label_rank = original_label.label_rank;
    let (width, height, labelpos) = (
        original_label.width,
        original_label.height,
        original_label.labelpos,
    );
    let mut prev = e.v.clone();
    let w = e.w.clone();
    let name = e.name.clone();
    // Reparenting and undo find the original edge through GraphLabel::dummy_chains.
    let mut original = Some(OriginalEdge {
        key: e,
        label: original_label,
    });

    for r in first_rank..w_rank {
        let dummy_edge = original
            .take()
            .map_or(DummyEdge::None, |edge| DummyEdge::Original(Box::new(edge)));
        let dummy_id = add_dummy_node(
            g,
            ids,
            NodeLabel {
                width: 0.0,
                height: 0.0,
                rank: Some(r),
                dummy: Some(DummyKind::Edge),
                dummy_edge,
                ..Default::default()
            },
            "_d",
        );

        if r == first_rank {
            g.graph_mut().dummy_chains.push(dummy_id.clone());
        }

        if label_rank == Some(r)
            && let Some(n) = g.node_mut(&dummy_id)
        {
            n.width = width;
            n.height = height;
            n.dummy = Some(DummyKind::EdgeLabel);
            n.labelpos = Some(labelpos);
        }

        g.set_edge_named(
            prev.clone(),
            dummy_id.clone(),
            name.clone(),
            Some(EdgeLabel {
                weight,
                ..Default::default()
            }),
        );
        prev = dummy_id;
    }

    g.set_edge_named(
        prev,
        w,
        name,
        Some(EdgeLabel {
            weight,
            ..Default::default()
        }),
    );
}

/// Restores the original edges after [`run`].
///
/// The graph must retain the disjoint, acyclic, single-successor dummy chains produced by [`run`].
/// Debug builds assert these phase invariants.
pub fn undo(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let chains = g.graph().dummy_chains.clone();
    let mut dummy_nodes: Vec<String> = Vec::new();
    let mut scheduled: FxHashSet<usize> = FxHashSet::default();
    let mut restored_edges: Vec<(EdgeKey, EdgeLabel)> = Vec::with_capacity(chains.len());

    for start in chains {
        if g.node_ix(&start)
            .is_some_and(|start_ix| scheduled.contains(&start_ix))
        {
            continue;
        }
        let Some(start_node) = g.node(&start) else {
            continue;
        };
        let Some(OriginalEdge {
            key,
            label: mut orig_label,
        }) = start_node.dummy_edge.original().cloned()
        else {
            continue;
        };

        let mut v = start.clone();
        while let Some(node) = g.node(&v) {
            if node.dummy.is_none() {
                break;
            }
            let Some(v_ix) = g.node_ix(&v) else {
                break;
            };
            if !scheduled.insert(v_ix) {
                break;
            }
            #[cfg(debug_assertions)]
            {
                let successors = g.successors(&v);
                debug_assert_eq!(
                    successors.len(),
                    1,
                    "normalize::undo requires single-successor dummy chains"
                );
                debug_assert!(
                    g.node_ix(successors[0])
                        .is_none_or(|successor_ix| !scheduled.contains(&successor_ix)),
                    "normalize::undo requires disjoint acyclic dummy chains"
                );
            }
            let w = g
                .first_successor(&v)
                .map(|s| s.to_string())
                .unwrap_or_default();

            if let (Some(x), Some(y)) = (node.x, node.y) {
                orig_label.points.push(Point { x, y });
                if node.dummy == Some(DummyKind::EdgeLabel) {
                    orig_label.x = Some(x);
                    orig_label.y = Some(y);
                    orig_label.width = node.width;
                    orig_label.height = node.height;
                }
            }

            dummy_nodes.push(v);
            v = w;
            if v.is_empty() {
                break;
            }
        }

        restored_edges.push((key, orig_label));
    }

    g.remove_nodes(dummy_nodes.iter().map(String::as_str));
    for (edge, label) in restored_edges {
        g.set_edge_key(edge, label);
    }
}

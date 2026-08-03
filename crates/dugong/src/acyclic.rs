//! Break cycles by reversing a feedback arc set (FAS).
//!
//! This mirrors Dagre's `acyclic.js`. Mermaid uses the default (DFS-based) variant by default,
//! but can opt into the greedy strategy.

use crate::graphlib::{EdgeKey, Graph};
use crate::work::{ceil_log2, checked_add, checked_mul, checked_n_log_n};
use crate::{EdgeLabel, GraphLabel, NodeLabel};
use crate::{NoopWorkControl, WorkControl, WorkError};
use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};

#[derive(Debug, Default)]
struct ReverseNameGen {
    next_by_endpoints: HashMap<(String, String), usize>,
}

impl ReverseNameGen {
    fn next(
        &mut self,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        v: &str,
        w: &str,
    ) -> Result<String, WorkError> {
        let endpoints = (v.to_string(), w.to_string());
        let mut next = self.next_by_endpoints.get(&endpoints).copied().unwrap_or(1);
        loop {
            let candidate = format!("rev{next}");
            if !g.has_edge(v, w, Some(&candidate)) {
                self.next_by_endpoints
                    .insert(endpoints, checked_add(next, 1)?);
                return Ok(candidate);
            }
            next = checked_add(next, 1)?;
        }
    }
}

pub fn run(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let mut work_control = NoopWorkControl;
    run_controlled(g, &mut work_control)
        .expect("the checked no-op Dugong work control cannot reject acyclic work");
}

pub(crate) fn run_controlled(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<(), WorkError> {
    let fas = if g
        .graph()
        .acyclicer
        .as_deref()
        .is_some_and(|s| s == "greedy")
    {
        work_control.charge(greedy_work_units(g)?)?;
        crate::greedy_fas::greedy_fas_with_weight(g, |lbl: &EdgeLabel| {
            if !lbl.weight.is_finite() {
                return 0;
            }
            lbl.weight.round() as i64
        })
    } else {
        work_control.charge(checked_add(
            g.node_count(),
            checked_mul(g.edge_count(), 2)?,
        )?)?;
        dfs_fas(g)
    };

    let mut reverse_names = ReverseNameGen::default();
    for e in fas.into_iter().filter(|e| e.v != e.w) {
        let Some(label) = g.edge_by_key(&e).cloned() else {
            continue;
        };
        let _ = g.remove_edge_key(&e);

        let mut label = label;
        label.forward_name = e.name.clone();
        label.reversed = true;

        let name = reverse_names.next(g, &e.w, &e.v)?;
        g.set_edge_named(e.w, e.v, Some(name), Some(label));
    }
    Ok(())
}

fn greedy_work_units(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>) -> Result<usize, WorkError> {
    let nodes = g.node_count();
    let edges = g.edge_count();
    let node_work = checked_mul(checked_n_log_n(nodes)?, 3)?;
    let edge_steps = checked_add(ceil_log2(nodes).max(1), 6)?;
    checked_add(node_work, checked_mul(edges, edge_steps)?)
}

pub fn undo(g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let edge_keys = g.edge_keys();
    for e in edge_keys {
        let Some(label) = g.edge_by_key(&e).cloned() else {
            continue;
        };
        if !label.reversed {
            continue;
        }
        let _ = g.remove_edge_key(&e);

        let mut label = label;
        let forward_name = label.forward_name.take();
        label.reversed = false;
        label.points.reverse();
        g.set_edge_named(e.w, e.v, forward_name, Some(label));
    }
}

fn dfs_fas(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>) -> Vec<EdgeKey> {
    // Ported from Dagre `lib/acyclic.js` (dfsFAS) as used by Mermaid `@11.12.2`.
    let mut fas: Vec<EdgeKey> = Vec::new();
    let mut stack: HashSet<String> = HashSet::default();
    let mut visited: HashSet<String> = HashSet::default();

    struct DfsFrame {
        v: String,
        edges: Vec<EdgeKey>,
        next_edge: usize,
    }

    fn push_frame(
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        v: String,
        visited: &mut HashSet<String>,
        stack: &mut HashSet<String>,
        frames: &mut Vec<DfsFrame>,
    ) {
        visited.insert(v.clone());
        stack.insert(v.clone());
        frames.push(DfsFrame {
            edges: g.out_edges(&v, None),
            v,
            next_edge: 0,
        });
    }

    fn dfs_iterative(
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        root: &str,
        visited: &mut HashSet<String>,
        stack: &mut HashSet<String>,
        fas: &mut Vec<EdgeKey>,
    ) {
        if visited.contains(root) {
            return;
        }

        let mut frames: Vec<DfsFrame> = Vec::new();
        push_frame(g, root.to_string(), visited, stack, &mut frames);

        while !frames.is_empty() {
            let next = {
                let frame = match frames.last_mut() {
                    Some(frame) => frame,
                    None => break,
                };
                if frame.next_edge < frame.edges.len() {
                    let edge = frame.edges[frame.next_edge].clone();
                    frame.next_edge += 1;
                    Some(edge)
                } else {
                    None
                }
            };

            let Some(e) = next else {
                let Some(frame) = frames.pop() else {
                    break;
                };
                stack.remove(&frame.v);
                continue;
            };

            if e.v == e.w {
                continue;
            }
            if stack.contains(&e.w) {
                fas.push(e);
            } else if !visited.contains(&e.w) {
                push_frame(g, e.w.clone(), visited, stack, &mut frames);
            }
        }
    }

    // Dagre's `dfsFAS` iterates nodes in `g.nodes()` order (insertion order).
    for v in g.nodes() {
        dfs_iterative(g, v, &mut visited, &mut stack, &mut fas);
    }
    fas
}

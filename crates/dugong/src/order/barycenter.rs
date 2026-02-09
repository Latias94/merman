//! Barycenter computation and conflict resolution.
//!
//! Ported from Dagre's `barycenter`, `resolveConflicts`, and `sortSubgraph` helpers.

use super::{OrderEdgeWeight, OrderNodeLabel};
use crate::graphlib::Graph;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct BarycenterEntry {
    pub v: String,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

pub fn barycenter<N, E, G>(g: &Graph<N, E, G>, movable: &[String]) -> Vec<BarycenterEntry>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    movable
        .iter()
        .map(|v| {
            let in_edges = g.in_edges(v, None);
            if in_edges.is_empty() {
                return BarycenterEntry {
                    v: v.clone(),
                    barycenter: None,
                    weight: None,
                };
            }

            let mut sum: f64 = 0.0;
            let mut weight: f64 = 0.0;
            for e in in_edges {
                let edge_weight = g.edge_by_key(&e).map(|e| e.weight()).unwrap_or(0.0);
                let u_order = g
                    .node(&e.v)
                    .and_then(|n| n.order())
                    .map(|n| n as f64)
                    .unwrap_or(0.0);
                sum += edge_weight * u_order;
                weight += edge_weight;
            }

            BarycenterEntry {
                v: v.clone(),
                barycenter: Some(sum / weight),
                weight: Some(weight),
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct SortEntry {
    pub vs: Vec<String>,
    pub i: usize,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

#[derive(Debug, Clone)]
struct ConflictEntry {
    indegree: usize,
    ins: Vec<String>,
    outs: Vec<String>,
    vs: Vec<String>,
    i: usize,
    barycenter: Option<f64>,
    weight: Option<f64>,
    merged: bool,
}

pub fn resolve_conflicts<N, E, G>(
    entries: &[BarycenterEntry],
    cg: &Graph<N, E, G>,
) -> Vec<SortEntry>
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
{
    let mut mapped: HashMap<String, ConflictEntry> = HashMap::new();
    for (i, entry) in entries.iter().enumerate() {
        mapped.insert(
            entry.v.clone(),
            ConflictEntry {
                indegree: 0,
                ins: Vec::new(),
                outs: Vec::new(),
                vs: vec![entry.v.clone()],
                i,
                barycenter: entry.barycenter,
                weight: entry.weight,
                merged: false,
            },
        );
    }

    for e in cg.edges() {
        let Some(_) = mapped.get(&e.v) else {
            continue;
        };
        let Some(_) = mapped.get(&e.w) else {
            continue;
        };

        if let Some(w_entry) = mapped.get_mut(&e.w) {
            w_entry.indegree += 1;
        }
        if let Some(v_entry) = mapped.get_mut(&e.v) {
            v_entry.outs.push(e.w.clone());
        }
    }

    let mut source_set: Vec<String> = mapped
        .iter()
        .filter_map(|(k, v)| {
            if v.indegree == 0 {
                Some(k.clone())
            } else {
                None
            }
        })
        .collect();

    let mut processed: Vec<String> = Vec::new();
    while let Some(v) = source_set.pop() {
        processed.push(v.clone());

        let ins = mapped.get(&v).map(|e| e.ins.clone()).unwrap_or_default();

        // Match upstream `.reverse().forEach(...)` on the "in" list.
        for u in ins.into_iter().rev() {
            if mapped.get(&u).map(|e| e.merged).unwrap_or(true) {
                continue;
            }
            let (u_bary, v_bary) = {
                let Some(u_entry) = mapped.get(&u) else {
                    continue;
                };
                let Some(v_entry) = mapped.get(&v) else {
                    continue;
                };
                (u_entry.barycenter, v_entry.barycenter)
            };
            let should_merge = match (u_bary, v_bary) {
                (None, _) => true,
                (_, None) => true,
                (Some(ub), Some(vb)) => ub >= vb,
            };
            if should_merge {
                merge_conflict_entries(&mut mapped, &v, &u);
            }
        }

        let outs = mapped.get(&v).map(|e| e.outs.clone()).unwrap_or_default();
        for w in outs {
            if let Some(w_entry) = mapped.get_mut(&w) {
                w_entry.ins.push(v.clone());
            }
            let w_indegree = {
                let Some(w_entry) = mapped.get_mut(&w) else {
                    continue;
                };
                w_entry.indegree = w_entry.indegree.saturating_sub(1);
                w_entry.indegree
            };
            if w_indegree == 0 {
                source_set.push(w);
            }
        }
    }

    let mut out: Vec<SortEntry> = Vec::new();
    for id in processed {
        let Some(entry) = mapped.get(&id) else {
            continue;
        };
        if entry.merged {
            continue;
        }
        out.push(SortEntry {
            vs: entry.vs.clone(),
            i: entry.i,
            barycenter: entry.barycenter,
            weight: entry.weight,
        });
    }
    out
}

// The conflict resolution algorithm needs a helper that can mutate two entries in-place.
// We keep it as a standalone function to make the port easy to review.
fn merge_conflict_entries(mapped: &mut HashMap<String, ConflictEntry>, target: &str, source: &str) {
    let (target_bary, target_weight, source_bary, source_weight, source_vs, source_i) = {
        let (Some(t), Some(s)) = (mapped.get(target), mapped.get(source)) else {
            return;
        };
        (
            t.barycenter,
            t.weight,
            s.barycenter,
            s.weight,
            s.vs.clone(),
            s.i,
        )
    };

    let mut sum: f64 = 0.0;
    let mut weight: f64 = 0.0;
    if let (Some(b), Some(w)) = (target_bary, target_weight) {
        if w != 0.0 {
            sum += b * w;
            weight += w;
        }
    }
    if let (Some(b), Some(w)) = (source_bary, source_weight) {
        if w != 0.0 {
            sum += b * w;
            weight += w;
        }
    }

    let Some(t) = mapped.get_mut(target) else {
        return;
    };
    t.vs = source_vs.into_iter().chain(t.vs.drain(..)).collect();
    if weight != 0.0 {
        t.barycenter = Some(sum / weight);
        t.weight = Some(weight);
    }
    t.i = t.i.min(source_i);

    if let Some(s) = mapped.get_mut(source) {
        s.merged = true;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SortResult {
    pub vs: Vec<String>,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

pub fn sort(entries: &[SortEntry], bias_right: bool) -> SortResult {
    let mut sortable: Vec<SortEntry> = Vec::new();
    let mut unsortable: Vec<SortEntry> = Vec::new();

    for entry in entries {
        if entry.barycenter.is_some() {
            sortable.push(entry.clone());
        } else {
            unsortable.push(entry.clone());
        }
    }

    unsortable.sort_by(|a, b| b.i.cmp(&a.i));

    sortable.sort_by(|a, b| {
        let a_bc = a.barycenter.unwrap_or(0.0);
        let b_bc = b.barycenter.unwrap_or(0.0);
        if a_bc < b_bc {
            std::cmp::Ordering::Less
        } else if a_bc > b_bc {
            std::cmp::Ordering::Greater
        } else if !bias_right {
            a.i.cmp(&b.i)
        } else {
            b.i.cmp(&a.i)
        }
    });

    let mut parts: Vec<Vec<String>> = Vec::new();
    let mut sum: f64 = 0.0;
    let mut weight: f64 = 0.0;
    let mut vs_index: usize = 0;

    fn consume_unsortable(
        parts: &mut Vec<Vec<String>>,
        unsortable: &mut Vec<SortEntry>,
        mut index: usize,
    ) -> usize {
        while let Some(last) = unsortable.last() {
            if last.i > index {
                break;
            }
            let Some(last) = unsortable.pop() else {
                break;
            };
            parts.push(last.vs);
            index += 1;
        }
        index
    }

    vs_index = consume_unsortable(&mut parts, &mut unsortable, vs_index);

    for entry in sortable {
        vs_index += entry.vs.len();
        parts.push(entry.vs.clone());
        if let (Some(bc), Some(w)) = (entry.barycenter, entry.weight) {
            sum += bc * w;
            weight += w;
        }
        vs_index = consume_unsortable(&mut parts, &mut unsortable, vs_index);
    }

    let vs: Vec<String> = parts.into_iter().flatten().collect();
    if weight != 0.0 {
        SortResult {
            vs,
            barycenter: Some(sum / weight),
            weight: Some(weight),
        }
    } else {
        SortResult {
            vs,
            barycenter: None,
            weight: None,
        }
    }
}

pub fn sort_subgraph<N, E, G, CN, CE, CG>(
    g: &Graph<N, E, G>,
    v: &str,
    cg: &Graph<CN, CE, CG>,
    bias_right: bool,
) -> SortResult
where
    N: Default + OrderNodeLabel + Clone + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
    CN: Default + 'static,
    CE: Default + 'static,
    CG: Default,
{
    let mut movable: Vec<String> = g.children(v).into_iter().map(|s| s.to_string()).collect();

    let (border_left, border_right) = g.node(v).map_or((None, None), |node| {
        (
            node.border_left().map(|s| s.to_string()),
            node.border_right().map(|s| s.to_string()),
        )
    });

    if let (Some(bl), Some(br)) = (border_left.as_deref(), border_right.as_deref()) {
        movable.retain(|w| w != bl && w != br);
    }

    let mut subgraphs: HashMap<String, SortResult> = HashMap::new();

    let mut barycenters = barycenter(g, &movable);
    for entry in &mut barycenters {
        if !g.children(&entry.v).is_empty() {
            let subgraph_result = sort_subgraph(g, &entry.v, cg, bias_right);
            if subgraph_result.barycenter.is_some() {
                merge_barycenters(entry, &subgraph_result);
            }
            subgraphs.insert(entry.v.clone(), subgraph_result);
        }
    }

    let mut entries = resolve_conflicts(&barycenters, cg);
    expand_subgraphs(&mut entries, &subgraphs);

    let mut result = sort(&entries, bias_right);

    if let (Some(bl), Some(br)) = (border_left, border_right) {
        let mut out: Vec<String> = Vec::with_capacity(result.vs.len() + 2);
        out.push(bl.clone());
        out.extend(result.vs);
        out.push(br.clone());
        result.vs = out;

        if !g.predecessors(&bl).is_empty() {
            let bl_pred = g.predecessors(&bl).first().cloned();
            let br_pred = g.predecessors(&br).first().cloned();
            let (Some(bl_pred), Some(br_pred)) = (bl_pred, br_pred) else {
                return result;
            };

            let bl_order = g.node(bl_pred).and_then(|n| n.order()).unwrap_or(0) as f64;
            let br_order = g.node(br_pred).and_then(|n| n.order()).unwrap_or(0) as f64;

            let bc = result.barycenter.unwrap_or(0.0);
            let w = result.weight.unwrap_or(0.0);
            let denom = w + 2.0;
            result.barycenter = Some((bc * w + bl_order + br_order) / denom);
            result.weight = Some(denom);
        }
    }

    result
}

fn expand_subgraphs(entries: &mut [SortEntry], subgraphs: &HashMap<String, SortResult>) {
    for entry in entries {
        let mut out: Vec<String> = Vec::new();
        for v in &entry.vs {
            if let Some(sg) = subgraphs.get(v) {
                out.extend(sg.vs.iter().cloned());
            } else {
                out.push(v.clone());
            }
        }
        entry.vs = out;
    }
}

fn merge_barycenters(target: &mut BarycenterEntry, other: &SortResult) {
    let Some(other_bc) = other.barycenter else {
        return;
    };
    let other_w = other.weight.unwrap_or(0.0);

    if let (Some(bc), Some(w)) = (target.barycenter, target.weight) {
        let denom = w + other_w;
        target.barycenter = Some((bc * w + other_bc * other_w) / denom);
        target.weight = Some(denom);
    } else {
        target.barycenter = Some(other_bc);
        target.weight = Some(other_w);
    }
}

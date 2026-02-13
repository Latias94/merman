//! Barycenter computation and conflict resolution.
//!
//! Ported from Dagre's `barycenter`, `resolveConflicts`, and `sortSubgraph` helpers.

use super::{OrderEdgeWeight, OrderNodeLabel};
use crate::graphlib::Graph;
use rustc_hash::FxHashMap as HashMap;
use std::time::{Duration, Instant};

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
            let Some(v_ix) = g.node_ix(v.as_str()) else {
                return BarycenterEntry {
                    v: v.clone(),
                    barycenter: None,
                    weight: None,
                };
            };

            let mut saw_edge = false;
            let mut sum: f64 = 0.0;
            let mut weight: f64 = 0.0;
            g.for_each_in_edge_ix(v_ix, None, |u_ix, _w_ix, _ek, lbl| {
                saw_edge = true;
                let edge_weight = lbl.weight();
                let u_order = g
                    .node_label_by_ix(u_ix)
                    .and_then(|n| n.order())
                    .map(|n| n as f64)
                    .unwrap_or(0.0);
                sum += edge_weight * u_order;
                weight += edge_weight;
            });
            if !saw_edge {
                return BarycenterEntry {
                    v: v.clone(),
                    barycenter: None,
                    weight: None,
                };
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
    ins: Vec<usize>,
    outs: Vec<usize>,
    vs: Vec<usize>,
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
    let mut id_to_ix: HashMap<&str, usize> = HashMap::default();
    let mut conflicts: Vec<ConflictEntry> = Vec::with_capacity(entries.len());
    for (ix, entry) in entries.iter().enumerate() {
        id_to_ix.insert(entry.v.as_str(), ix);
        conflicts.push(ConflictEntry {
            indegree: 0,
            ins: Vec::new(),
            outs: Vec::new(),
            vs: vec![ix],
            i: ix,
            barycenter: entry.barycenter,
            weight: entry.weight,
            merged: false,
        });
    }

    for e in cg.edges() {
        let Some(&v_ix) = id_to_ix.get(e.v.as_str()) else {
            continue;
        };
        let Some(&w_ix) = id_to_ix.get(e.w.as_str()) else {
            continue;
        };

        conflicts[w_ix].indegree += 1;
        conflicts[v_ix].outs.push(w_ix);
    }

    // Keep the original "source_set" ordering as close as possible to the previous HashMap-based
    // port by iterating `id_to_ix` (FxHashMap) rather than using the input slice order.
    let mut source_set: Vec<usize> = id_to_ix
        .iter()
        .filter_map(|(_, &ix)| {
            if conflicts[ix].indegree == 0 {
                Some(ix)
            } else {
                None
            }
        })
        .collect();

    let mut processed: Vec<usize> = Vec::new();
    while let Some(v_ix) = source_set.pop() {
        processed.push(v_ix);

        let ins = std::mem::take(&mut conflicts[v_ix].ins);

        // Match upstream `.reverse().forEach(...)` on the "in" list.
        for u in ins.into_iter().rev() {
            if conflicts[u].merged {
                continue;
            }
            let u_bary = conflicts[u].barycenter;
            let v_bary = conflicts[v_ix].barycenter;
            let should_merge = match (u_bary, v_bary) {
                (None, _) => true,
                (_, None) => true,
                (Some(ub), Some(vb)) => ub >= vb,
            };
            if should_merge {
                merge_conflict_entries(&mut conflicts, v_ix, u);
            }
        }

        let outs = std::mem::take(&mut conflicts[v_ix].outs);
        for w_ix in outs {
            conflicts[w_ix].ins.push(v_ix);
            conflicts[w_ix].indegree = conflicts[w_ix].indegree.saturating_sub(1);
            if conflicts[w_ix].indegree == 0 {
                source_set.push(w_ix);
            }
        }
    }

    let mut out: Vec<SortEntry> = Vec::new();
    for id in processed {
        let entry = &conflicts[id];
        if entry.merged {
            continue;
        }
        let mut vs: Vec<String> = Vec::with_capacity(entry.vs.len());
        for &ix in &entry.vs {
            vs.push(entries[ix].v.clone());
        }
        out.push(SortEntry {
            vs,
            i: entry.i,
            barycenter: entry.barycenter,
            weight: entry.weight,
        });
    }
    out
}

// The conflict resolution algorithm needs a helper that can mutate two entries in-place.
// We keep it as a standalone function to make the port easy to review.
fn merge_conflict_entries(mapped: &mut [ConflictEntry], target: usize, source: usize) {
    if target == source {
        return;
    }

    let (t, s) = if target < source {
        let (left, right) = mapped.split_at_mut(source);
        (&mut left[target], &mut right[0])
    } else {
        let (left, right) = mapped.split_at_mut(target);
        (&mut right[0], &mut left[source])
    };

    let target_bary = t.barycenter;
    let target_weight = t.weight;
    let source_bary = s.barycenter;
    let source_weight = s.weight;
    let source_i = s.i;

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

    let source_vs = std::mem::take(&mut s.vs);
    let target_vs = std::mem::take(&mut t.vs);
    let mut merged_vs: Vec<usize> = Vec::with_capacity(source_vs.len() + target_vs.len());
    merged_vs.extend(source_vs);
    merged_vs.extend(target_vs);
    t.vs = merged_vs;

    if weight != 0.0 {
        t.barycenter = Some(sum / weight);
        t.weight = Some(weight);
    }
    t.i = t.i.min(source_i);

    s.merged = true;
}

#[derive(Debug, Clone, PartialEq)]
pub struct SortResult {
    pub vs: Vec<String>,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BarycenterEntryIx {
    pub v_ix: usize,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SortEntryIx {
    pub vs: Vec<usize>,
    pub i: usize,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SortResultIx {
    pub vs: Vec<usize>,
    pub barycenter: Option<f64>,
    pub weight: Option<f64>,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct SortSubgraphTimings {
    pub total: Duration,
    pub build_movable: Duration,
    pub barycenter: Duration,
    pub resolve_conflicts: Duration,
    pub expand_subgraphs: Duration,
    pub sort: Duration,
    pub border_adjust: Duration,
}

pub fn sort(entries: &[SortEntry], bias_right: bool) -> SortResult {
    let mut total_len: usize = 0;
    let mut sortable: Vec<usize> = Vec::new();
    let mut unsortable: Vec<usize> = Vec::new();

    for (ix, entry) in entries.iter().enumerate() {
        total_len += entry.vs.len();
        if entry.barycenter.is_some() {
            sortable.push(ix);
        } else {
            unsortable.push(ix);
        }
    }

    unsortable.sort_by(|&a, &b| entries[b].i.cmp(&entries[a].i));

    sortable.sort_by(|&a, &b| {
        let a_entry = &entries[a];
        let b_entry = &entries[b];
        let a_bc = a_entry.barycenter.unwrap_or(0.0);
        let b_bc = b_entry.barycenter.unwrap_or(0.0);
        if a_bc < b_bc {
            std::cmp::Ordering::Less
        } else if a_bc > b_bc {
            std::cmp::Ordering::Greater
        } else if !bias_right {
            a_entry.i.cmp(&b_entry.i)
        } else {
            b_entry.i.cmp(&a_entry.i)
        }
    });

    let mut out: Vec<String> = Vec::with_capacity(total_len);
    let mut sum: f64 = 0.0;
    let mut weight: f64 = 0.0;
    let mut vs_index: usize = 0;

    fn consume_unsortable(
        out: &mut Vec<String>,
        entries: &[SortEntry],
        unsortable: &mut Vec<usize>,
        mut index: usize,
    ) -> usize {
        while let Some(&last_ix) = unsortable.last() {
            let last = &entries[last_ix];
            if last.i > index {
                break;
            }
            let Some(last_ix) = unsortable.pop() else {
                break;
            };
            out.extend(entries[last_ix].vs.iter().cloned());
            index += 1;
        }
        index
    }

    vs_index = consume_unsortable(&mut out, entries, &mut unsortable, vs_index);

    for entry_ix in sortable {
        let entry = &entries[entry_ix];
        vs_index += entry.vs.len();
        out.extend(entry.vs.iter().cloned());
        if let (Some(bc), Some(w)) = (entry.barycenter, entry.weight) {
            sum += bc * w;
            weight += w;
        }
        vs_index = consume_unsortable(&mut out, entries, &mut unsortable, vs_index);
    }

    if weight != 0.0 {
        SortResult {
            vs: out,
            barycenter: Some(sum / weight),
            weight: Some(weight),
        }
    } else {
        SortResult {
            vs: out,
            barycenter: None,
            weight: None,
        }
    }
}

pub(crate) fn sort_ix(entries: &[SortEntryIx], bias_right: bool) -> SortResultIx {
    let mut total_len: usize = 0;
    let mut sortable: Vec<usize> = Vec::new();
    let mut unsortable: Vec<usize> = Vec::new();

    for (ix, entry) in entries.iter().enumerate() {
        total_len += entry.vs.len();
        if entry.barycenter.is_some() {
            sortable.push(ix);
        } else {
            unsortable.push(ix);
        }
    }

    unsortable.sort_by(|&a, &b| entries[b].i.cmp(&entries[a].i));

    sortable.sort_by(|&a, &b| {
        let a_entry = &entries[a];
        let b_entry = &entries[b];
        let a_bc = a_entry.barycenter.unwrap_or(0.0);
        let b_bc = b_entry.barycenter.unwrap_or(0.0);
        if a_bc < b_bc {
            std::cmp::Ordering::Less
        } else if a_bc > b_bc {
            std::cmp::Ordering::Greater
        } else if !bias_right {
            a_entry.i.cmp(&b_entry.i)
        } else {
            b_entry.i.cmp(&a_entry.i)
        }
    });

    let mut out: Vec<usize> = Vec::with_capacity(total_len);
    let mut sum: f64 = 0.0;
    let mut weight: f64 = 0.0;
    let mut vs_index: usize = 0;

    fn consume_unsortable(
        out: &mut Vec<usize>,
        entries: &[SortEntryIx],
        unsortable: &mut Vec<usize>,
        mut index: usize,
    ) -> usize {
        while let Some(&last_ix) = unsortable.last() {
            let last = &entries[last_ix];
            if last.i > index {
                break;
            }
            let Some(last_ix) = unsortable.pop() else {
                break;
            };
            out.extend(entries[last_ix].vs.iter().copied());
            index += 1;
        }
        index
    }

    vs_index = consume_unsortable(&mut out, entries, &mut unsortable, vs_index);

    for entry_ix in sortable {
        let entry = &entries[entry_ix];
        vs_index += entry.vs.len();
        out.extend(entry.vs.iter().copied());
        if let (Some(bc), Some(w)) = (entry.barycenter, entry.weight) {
            sum += bc * w;
            weight += w;
        }
        vs_index = consume_unsortable(&mut out, entries, &mut unsortable, vs_index);
    }

    if weight != 0.0 {
        SortResultIx {
            vs: out,
            barycenter: Some(sum / weight),
            weight: Some(weight),
        }
    } else {
        SortResultIx {
            vs: out,
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

    let mut subgraphs: HashMap<String, SortResult> = HashMap::default();

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

        let (Some(bl_pred), Some(br_pred)) = (g.first_predecessor(&bl), g.first_predecessor(&br))
        else {
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

    result
}

pub(crate) fn sort_subgraph_ix<N, E, G, CN, CE, CG>(
    g: &Graph<N, E, G>,
    v: &str,
    cg: &Graph<CN, CE, CG>,
    bias_right: bool,
) -> SortResultIx
where
    N: Default + OrderNodeLabel + Clone + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
    CN: Default + 'static,
    CE: Default + 'static,
    CG: Default,
{
    let mut movable: Vec<usize> = g.children_iter(v).filter_map(|s| g.node_ix(s)).collect();

    let (border_left, border_right) = g.node(v).map_or((None, None), |node| {
        (node.border_left(), node.border_right())
    });

    let border_left_ix = border_left.and_then(|s| g.node_ix(s));
    let border_right_ix = border_right.and_then(|s| g.node_ix(s));

    if border_left_ix.is_some() && border_right_ix.is_some() {
        movable.retain(|&w_ix| Some(w_ix) != border_left_ix && Some(w_ix) != border_right_ix);
    }

    let mut subgraphs: HashMap<usize, SortResultIx> = HashMap::default();

    let mut barycenters = barycenter_ix(g, &movable);

    for entry in &mut barycenters {
        let Some(entry_id) = g.node_id_by_ix(entry.v_ix) else {
            continue;
        };
        if g.children_iter(entry_id).next().is_none() {
            continue;
        }

        let subgraph_result = sort_subgraph_ix(g, entry_id, cg, bias_right);
        if subgraph_result.barycenter.is_some() {
            merge_barycenters_ix(entry, &subgraph_result);
        }
        subgraphs.insert(entry.v_ix, subgraph_result);
    }

    let mut entries = resolve_conflicts_ix(g, &barycenters, cg);

    expand_subgraphs_ix(&mut entries, &subgraphs);

    let mut result = sort_ix(&entries, bias_right);

    if let (Some(bl), Some(br)) = (border_left, border_right) {
        let (Some(bl_ix), Some(br_ix)) = (border_left_ix, border_right_ix) else {
            return result;
        };

        let mut out: Vec<usize> = Vec::with_capacity(result.vs.len() + 2);
        out.push(bl_ix);
        out.extend(result.vs);
        out.push(br_ix);
        result.vs = out;

        let (Some(bl_pred), Some(br_pred)) = (g.first_predecessor(bl), g.first_predecessor(br))
        else {
            return result;
        };
        let (Some(bl_pred_ix), Some(br_pred_ix)) = (g.node_ix(bl_pred), g.node_ix(br_pred)) else {
            return result;
        };

        let bl_order = g
            .node_label_by_ix(bl_pred_ix)
            .and_then(|n| n.order())
            .unwrap_or(0) as f64;
        let br_order = g
            .node_label_by_ix(br_pred_ix)
            .and_then(|n| n.order())
            .unwrap_or(0) as f64;

        let bc = result.barycenter.unwrap_or(0.0);
        let w = result.weight.unwrap_or(0.0);
        let denom = w + 2.0;
        result.barycenter = Some((bc * w + bl_order + br_order) / denom);
        result.weight = Some(denom);
    }

    result
}

pub(crate) fn sort_subgraph_with_timings_ix<N, E, G, CN, CE, CG>(
    g: &Graph<N, E, G>,
    v: &str,
    cg: &Graph<CN, CE, CG>,
    bias_right: bool,
    timings: &mut SortSubgraphTimings,
) -> SortResultIx
where
    N: Default + OrderNodeLabel + Clone + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
    CN: Default + 'static,
    CE: Default + 'static,
    CG: Default,
{
    sort_subgraph_timed_ix(g, v, cg, bias_right, timings, 0)
}

fn sort_subgraph_timed_ix<N, E, G, CN, CE, CG>(
    g: &Graph<N, E, G>,
    v: &str,
    cg: &Graph<CN, CE, CG>,
    bias_right: bool,
    timings: &mut SortSubgraphTimings,
    depth: usize,
) -> SortResultIx
where
    N: Default + OrderNodeLabel + Clone + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
    CN: Default + 'static,
    CE: Default + 'static,
    CG: Default,
{
    let total_start = (depth == 0).then(Instant::now);

    let build_movable_start = Instant::now();
    let mut movable: Vec<usize> = g.children_iter(v).filter_map(|s| g.node_ix(s)).collect();

    let (border_left, border_right) = g.node(v).map_or((None, None), |node| {
        (node.border_left(), node.border_right())
    });

    let border_left_ix = border_left.and_then(|s| g.node_ix(s));
    let border_right_ix = border_right.and_then(|s| g.node_ix(s));

    if border_left_ix.is_some() && border_right_ix.is_some() {
        movable.retain(|&w_ix| Some(w_ix) != border_left_ix && Some(w_ix) != border_right_ix);
    }
    timings.build_movable += build_movable_start.elapsed();

    let mut subgraphs: HashMap<usize, SortResultIx> = HashMap::default();

    let barycenter_start = Instant::now();
    let mut barycenters = barycenter_ix(g, &movable);
    timings.barycenter += barycenter_start.elapsed();

    for entry in &mut barycenters {
        let Some(entry_id) = g.node_id_by_ix(entry.v_ix) else {
            continue;
        };
        if g.children_iter(entry_id).next().is_none() {
            continue;
        }

        let subgraph_result =
            sort_subgraph_timed_ix(g, entry_id, cg, bias_right, timings, depth + 1);
        if subgraph_result.barycenter.is_some() {
            merge_barycenters_ix(entry, &subgraph_result);
        }
        subgraphs.insert(entry.v_ix, subgraph_result);
    }

    let resolve_start = Instant::now();
    let mut entries = resolve_conflicts_ix(g, &barycenters, cg);
    timings.resolve_conflicts += resolve_start.elapsed();

    let expand_start = Instant::now();
    expand_subgraphs_ix(&mut entries, &subgraphs);
    timings.expand_subgraphs += expand_start.elapsed();

    let sort_start = Instant::now();
    let mut result = sort_ix(&entries, bias_right);
    timings.sort += sort_start.elapsed();

    if let (Some(bl), Some(br)) = (border_left, border_right) {
        let (Some(bl_ix), Some(br_ix)) = (border_left_ix, border_right_ix) else {
            return result;
        };

        let border_start = Instant::now();
        let mut out: Vec<usize> = Vec::with_capacity(result.vs.len() + 2);
        out.push(bl_ix);
        out.extend(result.vs);
        out.push(br_ix);
        result.vs = out;

        let (Some(bl_pred), Some(br_pred)) = (g.first_predecessor(bl), g.first_predecessor(br))
        else {
            return result;
        };
        let (Some(bl_pred_ix), Some(br_pred_ix)) = (g.node_ix(bl_pred), g.node_ix(br_pred)) else {
            return result;
        };

        let bl_order = g
            .node_label_by_ix(bl_pred_ix)
            .and_then(|n| n.order())
            .unwrap_or(0) as f64;
        let br_order = g
            .node_label_by_ix(br_pred_ix)
            .and_then(|n| n.order())
            .unwrap_or(0) as f64;

        let bc = result.barycenter.unwrap_or(0.0);
        let w = result.weight.unwrap_or(0.0);
        let denom = w + 2.0;
        result.barycenter = Some((bc * w + bl_order + br_order) / denom);
        result.weight = Some(denom);
        timings.border_adjust += border_start.elapsed();
    }

    if let Some(s) = total_start {
        timings.total += s.elapsed();
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

fn expand_subgraphs_ix(entries: &mut [SortEntryIx], subgraphs: &HashMap<usize, SortResultIx>) {
    for entry in entries {
        let mut out: Vec<usize> = Vec::new();
        for &v_ix in &entry.vs {
            if let Some(sg) = subgraphs.get(&v_ix) {
                out.extend(sg.vs.iter().copied());
            } else {
                out.push(v_ix);
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

fn merge_barycenters_ix(target: &mut BarycenterEntryIx, other: &SortResultIx) {
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

fn barycenter_ix<N, E, G>(g: &Graph<N, E, G>, movable_ix: &[usize]) -> Vec<BarycenterEntryIx>
where
    N: Default + OrderNodeLabel + 'static,
    E: Default + OrderEdgeWeight + 'static,
    G: Default,
{
    movable_ix
        .iter()
        .map(|&v_ix| {
            let mut saw_edge = false;
            let mut sum: f64 = 0.0;
            let mut weight: f64 = 0.0;
            g.for_each_in_edge_ix(v_ix, None, |u_ix, _w_ix, _ek, lbl| {
                saw_edge = true;
                let edge_weight = lbl.weight();
                let u_order = g
                    .node_label_by_ix(u_ix)
                    .and_then(|n| n.order())
                    .map(|n| n as f64)
                    .unwrap_or(0.0);
                sum += edge_weight * u_order;
                weight += edge_weight;
            });

            if !saw_edge {
                return BarycenterEntryIx {
                    v_ix,
                    barycenter: None,
                    weight: None,
                };
            }

            BarycenterEntryIx {
                v_ix,
                barycenter: Some(sum / weight),
                weight: Some(weight),
            }
        })
        .collect()
}

#[derive(Debug, Clone)]
struct ConflictEntryIx {
    indegree: usize,
    ins: Vec<usize>,
    outs: Vec<usize>,
    vs: Vec<usize>,
    i: usize,
    barycenter: Option<f64>,
    weight: Option<f64>,
    merged: bool,
}

fn resolve_conflicts_ix<N, E, G, CN, CE, CG>(
    g: &Graph<N, E, G>,
    entries: &[BarycenterEntryIx],
    cg: &Graph<CN, CE, CG>,
) -> Vec<SortEntryIx>
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
    CN: Default + 'static,
    CE: Default + 'static,
    CG: Default,
{
    let mut id_to_ix: HashMap<usize, usize> = HashMap::default();
    let mut conflicts: Vec<ConflictEntryIx> = Vec::with_capacity(entries.len());
    for (ix, entry) in entries.iter().enumerate() {
        id_to_ix.insert(entry.v_ix, ix);
        conflicts.push(ConflictEntryIx {
            indegree: 0,
            ins: Vec::new(),
            outs: Vec::new(),
            vs: vec![ix],
            i: ix,
            barycenter: entry.barycenter,
            weight: entry.weight,
            merged: false,
        });
    }

    for e in cg.edges() {
        let Some(v_node_ix) = g.node_ix(e.v.as_str()) else {
            continue;
        };
        let Some(w_node_ix) = g.node_ix(e.w.as_str()) else {
            continue;
        };

        let Some(&v_ix) = id_to_ix.get(&v_node_ix) else {
            continue;
        };
        let Some(&w_ix) = id_to_ix.get(&w_node_ix) else {
            continue;
        };

        conflicts[w_ix].indegree += 1;
        conflicts[v_ix].outs.push(w_ix);
    }

    let mut source_set: Vec<usize> = id_to_ix
        .iter()
        .filter_map(|(_, &ix)| {
            if conflicts[ix].indegree == 0 {
                Some(ix)
            } else {
                None
            }
        })
        .collect();

    let mut processed: Vec<usize> = Vec::new();
    while let Some(v_ix) = source_set.pop() {
        processed.push(v_ix);

        let ins = std::mem::take(&mut conflicts[v_ix].ins);

        for u in ins.into_iter().rev() {
            if conflicts[u].merged {
                continue;
            }
            let u_bary = conflicts[u].barycenter;
            let v_bary = conflicts[v_ix].barycenter;
            let should_merge = match (u_bary, v_bary) {
                (None, _) => true,
                (_, None) => true,
                (Some(ub), Some(vb)) => ub >= vb,
            };
            if should_merge {
                merge_conflict_entries_ix(&mut conflicts, v_ix, u);
            }
        }

        let outs = std::mem::take(&mut conflicts[v_ix].outs);
        for w_ix in outs {
            conflicts[w_ix].ins.push(v_ix);
            conflicts[w_ix].indegree = conflicts[w_ix].indegree.saturating_sub(1);
            if conflicts[w_ix].indegree == 0 {
                source_set.push(w_ix);
            }
        }
    }

    let mut out: Vec<SortEntryIx> = Vec::new();
    for id in processed {
        let entry = &conflicts[id];
        if entry.merged {
            continue;
        }
        let mut vs: Vec<usize> = Vec::with_capacity(entry.vs.len());
        for &ix in &entry.vs {
            vs.push(entries[ix].v_ix);
        }
        out.push(SortEntryIx {
            vs,
            i: entry.i,
            barycenter: entry.barycenter,
            weight: entry.weight,
        });
    }
    out
}

fn merge_conflict_entries_ix(mapped: &mut [ConflictEntryIx], target: usize, source: usize) {
    if target == source {
        return;
    }

    let (t, s) = if target < source {
        let (left, right) = mapped.split_at_mut(source);
        (&mut left[target], &mut right[0])
    } else {
        let (left, right) = mapped.split_at_mut(target);
        (&mut right[0], &mut left[source])
    };

    let target_bary = t.barycenter;
    let target_weight = t.weight;
    let source_bary = s.barycenter;
    let source_weight = s.weight;
    let source_i = s.i;

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

    let source_vs = std::mem::take(&mut s.vs);
    let target_vs = std::mem::take(&mut t.vs);
    let mut merged_vs: Vec<usize> = Vec::with_capacity(source_vs.len() + target_vs.len());
    merged_vs.extend(source_vs);
    merged_vs.extend(target_vs);
    t.vs = merged_vs;

    if weight != 0.0 {
        t.barycenter = Some(sum / weight);
        t.weight = Some(weight);
    }
    t.i = t.i.min(source_i);

    s.merged = true;
}

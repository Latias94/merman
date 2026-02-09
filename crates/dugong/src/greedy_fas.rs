//! Greedy feedback arc set (FAS) selection.
//!
//! Ported from Dagre's `greedy-fas.js`. This is used by `acyclic` when `acyclicer=greedy`.

use crate::graphlib::{EdgeKey, Graph};
use std::collections::{HashMap, HashSet, VecDeque, hash_map::Entry};

pub fn greedy_fas<N, E, G>(g: &Graph<N, E, G>) -> Vec<EdgeKey>
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
{
    greedy_fas_with_weight(g, |_| 1)
}

pub fn greedy_fas_with_weight<N, E, G>(
    g: &Graph<N, E, G>,
    weight_fn: impl Fn(&E) -> i64,
) -> Vec<EdgeKey>
where
    N: Default + 'static,
    E: Default + 'static,
    G: Default,
{
    if g.node_count() <= 1 {
        return Vec::new();
    }

    // Aggregate multi-edges into a simple graph with summed weights.
    //
    // Note: Upstream Dagre (JS) preserves insertion order for `g.nodes()` / `g.edges()` and
    // the derived `inEdges(v)` / `outEdges(v)` traversals. GreedyFAS is sensitive to that
    // ordering because it uses stable queues (List.enqueue + List.dequeue).
    //
    // For parity, keep node initialization in `g.node_ids()` order and keep the aggregated
    // adjacency order based on the *first occurrence* of each `(v, w)` in `g.edges()`.
    let node_ids = g.node_ids();
    let mut in_w: HashMap<String, i64> = HashMap::new();
    let mut out_w: HashMap<String, i64> = HashMap::new();
    for v in &node_ids {
        in_w.insert(v.clone(), 0);
        out_w.insert(v.clone(), 0);
    }

    let mut edge_w: HashMap<(String, String), i64> = HashMap::new();
    let mut edge_order: Vec<(String, String)> = Vec::new();
    let mut max_in: i64 = 0;
    let mut max_out: i64 = 0;

    for e in g.edges() {
        let w = g.edge_by_key(e).map(&weight_fn).unwrap_or(1);
        let key = (e.v.clone(), e.w.clone());
        match edge_w.entry(key.clone()) {
            Entry::Vacant(v) => {
                v.insert(w);
                edge_order.push(key);
            }
            Entry::Occupied(mut o) => {
                *o.get_mut() += w;
            }
        }
        let o = out_w.entry(e.v.clone()).or_insert(0);
        *o += w;
        max_out = max_out.max(*o);
        let i = in_w.entry(e.w.clone()).or_insert(0);
        *i += w;
        max_in = max_in.max(*i);
    }

    let bucket_len: usize = (max_out + max_in + 3).max(3) as usize;
    let zero_idx: i64 = max_in + 1;
    let mut buckets: Vec<VecDeque<String>> = (0..bucket_len).map(|_| VecDeque::new()).collect();
    let mut bucket_of: HashMap<String, usize> = HashMap::new();

    for v in &node_ids {
        assign_bucket(v, &in_w, &out_w, &mut buckets, zero_idx, &mut bucket_of);
    }

    // Build adjacency for the aggregated graph (for efficient updates).
    let mut in_edges: HashMap<String, Vec<(String, i64)>> = HashMap::new();
    let mut out_edges: HashMap<String, Vec<(String, i64)>> = HashMap::new();
    for (v, w) in &edge_order {
        let wgt = edge_w.get(&(v.clone(), w.clone())).copied().unwrap_or(0);
        out_edges
            .entry(v.clone())
            .or_default()
            .push((w.clone(), wgt));
        in_edges
            .entry(w.clone())
            .or_default()
            .push((v.clone(), wgt));
    }

    let mut alive: HashSet<String> = node_ids.iter().cloned().collect();
    let mut results: Vec<(String, String)> = Vec::new();

    while !alive.is_empty() {
        // Drain sinks (out == 0).
        while let Some(v) = pop_bucket(&mut buckets[0], &alive) {
            remove_node(
                &v,
                &mut alive,
                &mut buckets,
                zero_idx,
                &mut bucket_of,
                &mut in_w,
                &mut out_w,
                &in_edges,
                &out_edges,
                None,
            );
        }

        // Drain sources (in == 0).
        let last = buckets.len() - 1;
        while let Some(v) = pop_bucket(&mut buckets[last], &alive) {
            remove_node(
                &v,
                &mut alive,
                &mut buckets,
                zero_idx,
                &mut bucket_of,
                &mut in_w,
                &mut out_w,
                &in_edges,
                &out_edges,
                None,
            );
        }

        if alive.is_empty() {
            break;
        }

        // Pick a node from the highest non-extreme bucket and collect its predecessor edges.
        let mut picked: Option<String> = None;
        for i in (1..last).rev() {
            if let Some(v) = pop_bucket(&mut buckets[i], &alive) {
                picked = Some(v);
                break;
            }
        }

        let Some(v) = picked else {
            // Should not happen, but avoid an infinite loop.
            let v = node_ids
                .iter()
                .find(|id| alive.contains(*id))
                .cloned()
                .or_else(|| alive.iter().next().cloned());
            let Some(v) = v else {
                break;
            };
            remove_node(
                &v,
                &mut alive,
                &mut buckets,
                zero_idx,
                &mut bucket_of,
                &mut in_w,
                &mut out_w,
                &in_edges,
                &out_edges,
                None,
            );
            continue;
        };

        let mut preds: Vec<(String, String)> = Vec::new();
        remove_node(
            &v,
            &mut alive,
            &mut buckets,
            zero_idx,
            &mut bucket_of,
            &mut in_w,
            &mut out_w,
            &in_edges,
            &out_edges,
            Some(&mut preds),
        );
        results.extend(preds);
    }

    // Expand multi-edges back to concrete edge keys from the original graph.
    let mut out: Vec<EdgeKey> = Vec::new();
    for (v, w) in results {
        out.extend(g.out_edges(&v, Some(&w)));
    }
    out
}

fn pop_bucket(bucket: &mut VecDeque<String>, alive: &HashSet<String>) -> Option<String> {
    while let Some(v) = bucket.pop_back() {
        if alive.contains(&v) {
            return Some(v);
        }
    }
    None
}

fn assign_bucket(
    v: &str,
    in_w: &HashMap<String, i64>,
    out_w: &HashMap<String, i64>,
    buckets: &mut [VecDeque<String>],
    zero_idx: i64,
    bucket_of: &mut HashMap<String, usize>,
) {
    if let Some(prev) = bucket_of.get(v).copied() {
        if let Some(pos) = buckets[prev].iter().position(|x| x == v) {
            buckets[prev].remove(pos);
        }
    }

    let in_v = in_w.get(v).copied().unwrap_or(0);
    let out_v = out_w.get(v).copied().unwrap_or(0);
    let idx: usize = if out_v == 0 {
        0
    } else if in_v == 0 {
        buckets.len() - 1
    } else {
        let raw = out_v - in_v + zero_idx;
        raw.clamp(0, (buckets.len() - 1) as i64) as usize
    };

    buckets[idx].push_front(v.to_string());
    bucket_of.insert(v.to_string(), idx);
}

fn remove_node(
    v: &str,
    alive: &mut HashSet<String>,
    buckets: &mut [VecDeque<String>],
    zero_idx: i64,
    bucket_of: &mut HashMap<String, usize>,
    in_w: &mut HashMap<String, i64>,
    out_w: &mut HashMap<String, i64>,
    in_edges: &HashMap<String, Vec<(String, i64)>>,
    out_edges: &HashMap<String, Vec<(String, i64)>>,
    collect_predecessors: Option<&mut Vec<(String, String)>>,
) {
    if !alive.remove(v) {
        return;
    }

    if let Some(preds) = collect_predecessors {
        if let Some(ins) = in_edges.get(v) {
            for (u, _) in ins {
                if alive.contains(u) {
                    preds.push((u.clone(), v.to_string()));
                }
            }
        }
    }

    if let Some(ins) = in_edges.get(v) {
        for (u, wgt) in ins {
            if !alive.contains(u) {
                continue;
            }
            if let Some(o) = out_w.get_mut(u) {
                *o -= *wgt;
            }
            assign_bucket(u, in_w, out_w, buckets, zero_idx, bucket_of);
        }
    }

    if let Some(outs) = out_edges.get(v) {
        for (w, wgt) in outs {
            if !alive.contains(w) {
                continue;
            }
            if let Some(i) = in_w.get_mut(w) {
                *i -= *wgt;
            }
            assign_bucket(w, in_w, out_w, buckets, zero_idx, bucket_of);
        }
    }

    in_w.remove(v);
    out_w.remove(v);
    bucket_of.remove(v);
}

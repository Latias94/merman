//! Dagre layout pipelines.
//!
//! This module hosts the public entrypoints (`layout`, `layout_dagreish`) and keeps `lib.rs`
//! focused on crate-level exports.

use crate::graphlib;
use crate::{
    EdgeLabel, GraphLabel, LabelPos, NodeLabel, Point, RankDir, acyclic, add_border_segments,
    coordinate_system, nesting_graph, normalize, order, parent_dummy_chains, position, rank,
    self_edges, util,
};

pub fn layout(g: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    // Dagre breaks cycles before ranking. Keep self-loops out of the feedback arc set since
    // reversing a self-loop doesn't make the graph acyclic, and self-loops should not constrain
    // rank assignment.
    acyclic::run(g);

    let graph = g.graph().clone();
    let edge_keys: Vec<graphlib::EdgeKey> = g.edges().cloned().collect();

    let mut max_edge_label_width: f64 = 0.0;
    let mut max_edge_label_height: f64 = 0.0;
    for e in &edge_keys {
        if let Some(lbl) = g.edge(&e.v, &e.w, e.name.as_deref()) {
            max_edge_label_width = max_edge_label_width.max(lbl.width);
            max_edge_label_height = max_edge_label_height.max(lbl.height);
        }
    }

    // A minimal parity-oriented approximation:
    // - in TB/BT: long edge labels tend to push nodes apart horizontally (cross-axis)
    // - in LR/RL: long edge labels tend to push ranks apart horizontally (axis)
    let node_sep = match graph.rankdir {
        RankDir::TB | RankDir::BT => graph.nodesep.max(max_edge_label_width),
        RankDir::LR | RankDir::RL => graph.nodesep.max(max_edge_label_height),
    };
    let rank_sep = match graph.rankdir {
        RankDir::TB | RankDir::BT => graph.ranksep,
        RankDir::LR | RankDir::RL => graph.ranksep.max(max_edge_label_width),
    };

    let node_ids: Vec<String> = g.nodes().map(|s| s.to_string()).collect();
    let node_ids: Vec<String> = node_ids
        .into_iter()
        .filter(|id| !(g.options().compound && !g.children(id).is_empty()))
        .collect();

    let mut indegree: std::collections::HashMap<String, usize> =
        node_ids.iter().map(|id| (id.clone(), 0)).collect();
    for e in g.edges() {
        if e.v == e.w {
            continue;
        }
        if let Some(v) = indegree.get_mut(&e.w) {
            *v += 1;
        }
    }

    // Deterministic Kahn order: initial nodes in insertion order.
    let mut queue: std::collections::VecDeque<String> = node_ids
        .iter()
        .filter(|id| indegree.get(*id).copied().unwrap_or(0) == 0)
        .cloned()
        .collect();

    let mut topo: Vec<String> = Vec::new();
    while let Some(n) = queue.pop_front() {
        topo.push(n.clone());

        // Traverse outgoing edges in edge insertion order.
        let mut out: Vec<String> = Vec::new();
        for e in g.edges() {
            if e.v == n {
                if e.v == e.w {
                    continue;
                }
                out.push(e.w.clone());
            }
        }
        for w in out {
            if let Some(v) = indegree.get_mut(&w) {
                *v = v.saturating_sub(1);
                if *v == 0 {
                    queue.push_back(w);
                }
            }
        }
    }

    // If the graph has a cycle, fall back to insertion order for now.
    if topo.len() != node_ids.len() {
        topo = node_ids.clone();
    }

    let mut rank: std::collections::HashMap<String, usize> =
        node_ids.iter().map(|id| (id.clone(), 0)).collect();
    for n in &topo {
        let r = rank.get(n).copied().unwrap_or(0);
        for e in g.edges() {
            if e.v != *n {
                continue;
            }
            if e.v == e.w {
                continue;
            }
            let minlen = g
                .edge(&e.v, &e.w, e.name.as_deref())
                .map(|l| l.minlen)
                .unwrap_or(1)
                .max(1);
            let next = r.saturating_add(minlen);
            let entry = rank.entry(e.w.clone()).or_insert(0);
            if next > *entry {
                *entry = next;
            }
        }
    }

    if g.options().compound {
        // Compact ranks inside compound nodes where a common rank is feasible, to minimize cluster height.
        // This is a small parity-oriented step to match upstream Dagre behavior for subgraphs.
        let parents: Vec<String> = g
            .node_ids()
            .into_iter()
            .filter(|id| !g.children(id).is_empty())
            .collect();

        for parent in parents {
            let children: Vec<String> = g
                .children(&parent)
                .into_iter()
                .map(|s| s.to_string())
                .collect();
            let targets: Vec<String> = children
                .into_iter()
                .filter(|c| rank.contains_key(c))
                .collect();
            if targets.len() < 2 {
                continue;
            }

            // Preserve insertion order (children are stored deterministically).
            let mut min_needed: usize = 0;
            let mut max_allowed: usize = usize::MAX / 4;

            for child in &targets {
                let mut min_rank: usize = 0;
                for ek in g.in_edges(child, None) {
                    let Some(&pred_rank) = rank.get(&ek.v) else {
                        continue;
                    };
                    let minlen = g.edge_by_key(&ek).map(|e| e.minlen).unwrap_or(1).max(1);
                    min_rank = min_rank.max(pred_rank.saturating_add(minlen));
                }

                let mut max_rank: usize = usize::MAX / 4;
                for ek in g.out_edges(child, None) {
                    let Some(&succ_rank) = rank.get(&ek.w) else {
                        continue;
                    };
                    let minlen = g.edge_by_key(&ek).map(|e| e.minlen).unwrap_or(1).max(1);
                    let upper = succ_rank.saturating_sub(minlen);
                    max_rank = max_rank.min(upper);
                }

                min_needed = min_needed.max(min_rank);
                max_allowed = max_allowed.min(max_rank);
            }

            if min_needed <= max_allowed {
                for child in &targets {
                    rank.insert(child.clone(), min_needed);
                }
            }
        }
    }

    let max_rank = rank.values().copied().max().unwrap_or(0);
    let mut ranks: Vec<Vec<String>> = vec![Vec::new(); max_rank + 1];
    for id in &node_ids {
        let r = rank.get(id).copied().unwrap_or(0);
        if let Some(layer) = ranks.get_mut(r) {
            layer.push(id.clone());
        }
    }

    fn node_size(g: &graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>, id: &str) -> (f64, f64) {
        match g.node(id) {
            Some(n) => (n.width, n.height),
            None => (0.0, 0.0),
        }
    }

    let mut gap_extra: Vec<f64> = vec![0.0; ranks.len().saturating_sub(1)];
    for e in g.edges() {
        if e.v == e.w {
            continue;
        }
        let Some(v_rank) = rank.get(&e.v).copied() else {
            continue;
        };
        let Some(w_rank) = rank.get(&e.w).copied() else {
            continue;
        };
        if w_rank != v_rank.saturating_add(1) {
            continue;
        }
        let Some(lbl) = g.edge(&e.v, &e.w, e.name.as_deref()) else {
            continue;
        };
        if lbl.height <= 0.0 {
            continue;
        }
        if let Some(extra) = gap_extra.get_mut(v_rank) {
            *extra = extra.max(lbl.height);
        }
    }

    let mut rank_heights: Vec<f64> = Vec::with_capacity(ranks.len());
    let mut rank_widths: Vec<f64> = Vec::with_capacity(ranks.len());
    for ids in &ranks {
        let mut h: f64 = 0.0;
        let mut w: f64 = 0.0;
        for (i, id) in ids.iter().enumerate() {
            let (nw, nh) = node_size(g, id);
            h = h.max(nh);
            w += nw;
            if i + 1 < ids.len() {
                w += node_sep;
            }
        }
        rank_heights.push(h);
        rank_widths.push(w);
    }
    let max_rank_width = rank_widths.iter().copied().fold(0.0_f64, |a, b| a.max(b));

    let mut y_cursor: f64 = 0.0;
    for (rank_idx, ids) in ranks.iter().enumerate() {
        let rank_h = rank_heights[rank_idx];
        let y = y_cursor + rank_h / 2.0;

        let rank_w = rank_widths[rank_idx];
        let mut x_cursor = (max_rank_width - rank_w) / 2.0;
        for id in ids {
            let (nw, _) = node_size(g, id);
            let x = x_cursor + nw / 2.0;
            if let Some(n) = g.node_mut(id) {
                n.x = Some(x);
                n.y = Some(y);
            }
            x_cursor += nw + node_sep;
        }

        y_cursor += rank_h;
        if rank_idx + 1 < ranks.len() {
            y_cursor += rank_sep + gap_extra.get(rank_idx).copied().unwrap_or(0.0);
        }
    }

    let total_height = y_cursor;

    for e in &edge_keys {
        let Some((sx, sy, sw, sh)) = g
            .node(&e.v)
            .map(|n| (n.x.unwrap_or(0.0), n.y.unwrap_or(0.0), n.width, n.height))
        else {
            continue;
        };
        let Some((tx, ty, tw, th)) = g
            .node(&e.w)
            .map(|n| (n.x.unwrap_or(0.0), n.y.unwrap_or(0.0), n.width, n.height))
        else {
            continue;
        };

        let Some(lbl) = g.edge_mut(&e.v, &e.w, e.name.as_deref()) else {
            continue;
        };
        lbl.points.clear();
        lbl.x = None;
        lbl.y = None;

        if e.v == e.w {
            // A minimal self-loop shape that satisfies upstream dagre invariants:
            // - TB/BT: all points are to the right of the node center (x > node.x)
            // - LR/RL: after rankdir transforms, all points are below the node center (y > node.y)
            // and all points stay within the node's height/2 on the cross-axis.
            let x0 = sx + sw / 2.0 + graph.edgesep.max(1.0);
            let x1 = x0 + graph.edgesep.max(1.0);
            let y0 = sy;
            let y_top = sy - sh / 2.0;
            let y_bot = sy + sh / 2.0;

            lbl.points.extend([
                Point { x: x0, y: y0 },
                Point { x: x0, y: y_top },
                Point { x: x1, y: y_top },
                Point { x: x1, y: y0 },
                Point { x: x1, y: y_bot },
                Point { x: x0, y: y_bot },
                Point { x: x0, y: y0 },
            ]);

            continue;
        }

        let start = Point {
            x: sx,
            y: sy + sh / 2.0,
        };
        let end = Point {
            x: tx,
            y: ty - th / 2.0,
        };

        let minlen = lbl.minlen.max(1);
        let count = 2 * minlen + 1;
        for i in 0..count {
            let t = (i as f64) / ((count - 1) as f64);
            lbl.points.push(Point {
                x: start.x + (end.x - start.x) * t,
                y: start.y + (end.y - start.y) * t,
            });
        }

        if lbl.width > 0.0 || lbl.height > 0.0 {
            if let Some(mid) = lbl.points.get(count / 2).copied() {
                let mut ex = mid.x;
                let ey = mid.y;
                match lbl.labelpos {
                    LabelPos::C => {}
                    LabelPos::L => ex -= lbl.labeloffset + lbl.width / 2.0,
                    LabelPos::R => ex += lbl.labeloffset + lbl.width / 2.0,
                }
                lbl.x = Some(ex);
                lbl.y = Some(ey);
            }
        }

        let _ = (sw, tw);
    }

    match graph.rankdir {
        RankDir::TB => {}
        RankDir::BT => {
            for id in &node_ids {
                if let Some(n) = g.node_mut(id) {
                    if let Some(y) = n.y {
                        n.y = Some(total_height - y);
                    }
                }
            }
            for e in &edge_keys {
                if let Some(lbl) = g.edge_mut(&e.v, &e.w, e.name.as_deref()) {
                    for p in &mut lbl.points {
                        p.y = total_height - p.y;
                    }
                    if let Some(y) = lbl.y {
                        lbl.y = Some(total_height - y);
                    }
                }
            }
        }
        RankDir::LR => {
            for id in &node_ids {
                if let Some(n) = g.node_mut(id) {
                    let (Some(x), Some(y)) = (n.x, n.y) else {
                        continue;
                    };
                    n.x = Some(y);
                    n.y = Some(x);
                }
            }
            for e in &edge_keys {
                if let Some(lbl) = g.edge_mut(&e.v, &e.w, e.name.as_deref()) {
                    for p in &mut lbl.points {
                        (p.x, p.y) = (p.y, p.x);
                    }
                    if let (Some(x), Some(y)) = (lbl.x, lbl.y) {
                        lbl.x = Some(y);
                        lbl.y = Some(x);
                    }
                }
            }
        }
        RankDir::RL => {
            for id in &node_ids {
                if let Some(n) = g.node_mut(id) {
                    let (Some(x), Some(y)) = (n.x, n.y) else {
                        continue;
                    };
                    n.x = Some(total_height - y);
                    n.y = Some(x);
                }
            }
            for e in &edge_keys {
                if let Some(lbl) = g.edge_mut(&e.v, &e.w, e.name.as_deref()) {
                    for p in &mut lbl.points {
                        let new_x = total_height - p.y;
                        (p.x, p.y) = (new_x, p.x);
                    }
                    if let (Some(x), Some(y)) = (lbl.x, lbl.y) {
                        lbl.x = Some(total_height - y);
                        lbl.y = Some(x);
                    }
                }
            }
        }
    }

    // Restore original edge directions and names, keeping computed points.
    acyclic::undo(g);
}

/// Experimental Dagre-style pipeline for non-compound graphs.
///
/// This uses the parity-oriented building blocks (`rank`, `normalize`, `order`, BK positioning)
/// that are already in this crate, but is not yet the default `layout()` implementation.
///
/// For compound graphs, this falls back to `layout()` for now.
pub fn layout_dagreish(g: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    // Mirror Dagre's `makeSpaceForEdgeLabels` so edge-label proxy ranks become integers
    // (we later materialize label nodes in `normalize::run`).
    g.graph_mut().ranksep /= 2.0;
    let rankdir = g.graph().rankdir;
    for ek in g.edge_keys() {
        if let Some(e) = g.edge_mut_by_key(&ek) {
            e.minlen = e.minlen.max(1).saturating_mul(2);
            if !matches!(e.labelpos, LabelPos::C) {
                match rankdir {
                    RankDir::TB | RankDir::BT => e.width += e.labeloffset,
                    RankDir::LR | RankDir::RL => e.height += e.labeloffset,
                }
            }
        }
    }

    // Dagre removes self-loops before ranking/normalization and re-inserts them during positioning
    // via dummy "selfedge" nodes. This avoids invalid rank constraints and gives self-loops a
    // deterministic, spacing-aware offset in BK positioning.
    self_edges::remove_self_edges(g);

    acyclic::run(g);

    // Mermaid's dagre adapter always enables `compound: true`, and Dagre's ranker expects a
    // connected graph. Nesting graph connects components (even if there are no explicit
    // subgraphs), preventing network-simplex from panicking on disconnected inputs.
    if g.options().compound {
        nesting_graph::run(g);
    }

    // Match upstream Dagre: ranking runs on a non-compound view of the graph so cluster nodes
    // (nodes with children) do not participate in ranking / network-simplex connectivity.
    //
    // `nesting_graph::run` materializes border nodes and nesting edges; those border nodes are
    // leaf nodes and remain in the non-compound graph, providing the constraints Dagre expects.
    let mut rank_graph = util::as_non_compound_graph(g);
    rank::rank(&mut rank_graph);
    // Mirror Dagre's JS behavior: `rank(asNonCompoundGraph(g))` mutates the same label objects
    // for leaf nodes, but does not propagate ranks to compound nodes (nodes with children).
    //
    // In Rust we don't share label objects between graphs, so we copy ranks explicitly for leaf
    // nodes only.
    for v in g.node_ids() {
        if !g.children(&v).is_empty() {
            continue;
        }
        let Some(rank) = rank_graph.node(&v).and_then(|n| n.rank) else {
            continue;
        };
        if let Some(n) = g.node_mut(&v) {
            n.rank = Some(rank);
        }
    }

    // Mirror Dagre's `injectEdgeLabelProxies` / `removeEdgeLabelProxies` to compute label ranks.
    // These label ranks are used by `normalize::run` to materialize `edge-label` dummy nodes with
    // the correct width/height, letting BK positioning account for edge labels.
    for ek in g.edge_keys() {
        let Some(edge) = g.edge_by_key(&ek) else {
            continue;
        };
        if edge.width <= 0.0 || edge.height <= 0.0 {
            continue;
        }
        let Some(v_rank) = g.node(&ek.v).and_then(|n| n.rank) else {
            continue;
        };
        let Some(w_rank) = g.node(&ek.w).and_then(|n| n.rank) else {
            continue;
        };
        let rank = (w_rank - v_rank) / 2 + v_rank;
        g.set_node(
            util::unique_id("_ep"),
            NodeLabel {
                rank: Some(rank),
                dummy: Some("edge-proxy".to_string()),
                edge_obj: Some(ek.clone()),
                ..Default::default()
            },
        );
    }

    util::remove_empty_ranks(g);

    // Match upstream Dagre: `nestingGraph.cleanup` must happen before ordering/positioning.
    if g.options().compound {
        nesting_graph::cleanup(g);
    }

    util::normalize_ranks(g);

    // Remove edge label proxy nodes, storing their rank on the corresponding edge label.
    let node_ids = g.node_ids();
    for v in node_ids {
        let Some(node) = g.node(&v).cloned() else {
            continue;
        };
        if node.dummy.as_deref() != Some("edge-proxy") {
            continue;
        }
        let Some(edge_obj) = node.edge_obj.clone() else {
            let _ = g.remove_node(&v);
            continue;
        };
        if let Some(lbl) = g.edge_mut_by_key(&edge_obj) {
            lbl.label_rank = node.rank;
        }
        let _ = g.remove_node(&v);
    }

    // Dagre uses `assignRankMinMax` to annotate compound nodes with their rank span, derived from
    // the `nestingGraph` border top/bottom nodes. This rank span is later used by subgraph
    // ordering and border segment generation.
    if g.options().compound {
        let node_ids = g.node_ids();
        for v in node_ids {
            let Some(node) = g.node(&v).cloned() else {
                continue;
            };
            let (Some(bt), Some(bb)) = (node.border_top.clone(), node.border_bottom.clone()) else {
                continue;
            };
            let (Some(min_rank), Some(max_rank)) = (
                g.node(&bt).and_then(|n| n.rank),
                g.node(&bb).and_then(|n| n.rank),
            ) else {
                continue;
            };
            if let Some(n) = g.node_mut(&v) {
                n.min_rank = Some(min_rank);
                n.max_rank = Some(max_rank);
            }
        }
    }

    normalize::run(g);
    if g.options().compound {
        parent_dummy_chains::parent_dummy_chains(g);
        add_border_segments::add_border_segments(g);
    }
    order::order(
        g,
        order::OrderOptions {
            disable_optimal_order_heuristic: false,
        },
    );

    // Positioning runs in TB coordinates; `coordinate_system::adjust` maps LR/RL/BT into TB.
    coordinate_system::adjust(g);

    // Insert dummy self-edge nodes after ordering and rankdir transforms, so their sizes match the
    // active coordinate system (TB) and they can influence BK x-positioning.
    self_edges::insert_self_edges(g);

    let rank_sep = g.graph().ranksep;
    let layering = util::build_layer_matrix(g);

    let mut prev_y: f64 = 0.0;
    for (idx, layer) in layering.iter().enumerate() {
        let max_h = layer
            .iter()
            .filter_map(|id| g.node(id).map(|n| n.height))
            .fold(0.0_f64, f64::max);
        let y = prev_y + max_h / 2.0;
        for id in layer {
            if let Some(n) = g.node_mut(id) {
                n.y = Some(y);
            }
        }
        prev_y += max_h;
        if idx + 1 < layering.len() {
            prev_y += rank_sep;
        }
    }

    let xs = position::bk::position_x(g);
    for id in g.node_ids() {
        if let Some(n) = g.node_mut(&id) {
            n.x = Some(xs.get(&id).copied().unwrap_or(0.0));
        }
    }

    // Convert dummy self-edge nodes into self-loop edge point sequences and remove the dummy nodes.
    self_edges::position_self_edges(g);

    // Match upstream Dagre: `removeBorderNodes` runs after positioning and before `normalize.undo`.
    // It sets compound-node geometry (x/y/width/height) from border nodes, then removes all
    // border dummy nodes.
    if g.options().compound {
        remove_border_nodes(g);
    }

    normalize::undo(g);
    coordinate_system::undo(g);

    // Translate so the minimum top-left is at (marginx, marginy), matching Dagre's
    // `translateGraph(...)` behavior.
    let mut min_x: f64 = f64::INFINITY;
    let mut min_y: f64 = f64::INFINITY;
    for id in g.node_ids() {
        let Some(n) = g.node(&id) else {
            continue;
        };
        let (Some(x), Some(y)) = (n.x, n.y) else {
            continue;
        };
        min_x = min_x.min(x - n.width / 2.0);
        min_y = min_y.min(y - n.height / 2.0);
    }
    for ek in g.edge_keys() {
        let Some(lbl) = g.edge_by_key(&ek) else {
            continue;
        };
        // Match Dagre's `translateGraph(...)`: it computes min/max based on nodes and edge-label
        // boxes, but does not include intermediate edge points. This can leave some internal spline
        // control points with negative coordinates (which Mermaid preserves in `data-points`), while
        // the rendered path remains within the viewBox because `curveBasis` does not pass through
        // those interior points.
        if let (Some(x), Some(y)) = (lbl.x, lbl.y) {
            min_x = min_x.min(x - lbl.width / 2.0);
            min_y = min_y.min(y - lbl.height / 2.0);
        }
    }

    if min_x.is_finite() && min_y.is_finite() {
        // Dagre shifts the graph by `-(min - margin)` so the smallest x/y becomes `margin`.
        // This is observable in Mermaid flowchart-v2 SVG output where `diagramPadding: 0`
        // still yields a `viewBox` starting at x=8 (the Dagre margin).
        min_x -= g.graph().marginx;
        min_y -= g.graph().marginy;
        let dx = -min_x;
        let dy = -min_y;
        for id in g.node_ids() {
            if let Some(n) = g.node_mut(&id) {
                if let Some(x) = n.x {
                    n.x = Some(x + dx);
                }
                if let Some(y) = n.y {
                    n.y = Some(y + dy);
                }
            }
        }
        for ek in g.edge_keys() {
            if let Some(lbl) = g.edge_mut_by_key(&ek) {
                for p in &mut lbl.points {
                    p.x += dx;
                    p.y += dy;
                }
                if let Some(x) = lbl.x {
                    lbl.x = Some(x + dx);
                }
                if let Some(y) = lbl.y {
                    lbl.y = Some(y + dy);
                }
            }
        }
    }

    // Ensure every edge has at least one internal point (so D3 `curveBasis` emits cubic beziers),
    // and add node intersection endpoints to better match Dagre/Mermaid edge point semantics.
    let edge_keys: Vec<graphlib::EdgeKey> = g.edges().cloned().collect();
    for e in edge_keys {
        let Some((sx, sy, sw, sh)) = g
            .node(&e.v)
            .map(|n| (n.x.unwrap_or(0.0), n.y.unwrap_or(0.0), n.width, n.height))
        else {
            continue;
        };
        let Some((tx, ty, tw, th)) = g
            .node(&e.w)
            .map(|n| (n.x.unwrap_or(0.0), n.y.unwrap_or(0.0), n.width, n.height))
        else {
            continue;
        };

        let Some(lbl) = g.edge_mut(&e.v, &e.w, e.name.as_deref()) else {
            continue;
        };

        let mut internal: Vec<Point> = if lbl.points.is_empty() {
            vec![Point {
                x: (sx + tx) / 2.0,
                y: (sy + ty) / 2.0,
            }]
        } else {
            lbl.points.clone()
        };
        if internal.is_empty() {
            internal.push(Point {
                x: (sx + tx) / 2.0,
                y: (sy + ty) / 2.0,
            });
        }

        let Some(first) = internal.first().copied() else {
            continue;
        };
        let Some(last) = internal.last().copied() else {
            continue;
        };

        let mut pts: Vec<Point> = Vec::with_capacity(internal.len() + 2);

        pts.push(util::intersect_rect(
            util::Rect {
                x: sx,
                y: sy,
                width: sw,
                height: sh,
            },
            first,
        ));
        pts.extend(internal);
        pts.push(util::intersect_rect(
            util::Rect {
                x: tx,
                y: ty,
                width: tw,
                height: th,
            },
            last,
        ));

        lbl.points = pts;

        if (lbl.width > 0.0 || lbl.height > 0.0) && lbl.x.is_none() && lbl.y.is_none() {
            if let Some(mid) = lbl.points.get(lbl.points.len() / 2).copied() {
                let mut ex = mid.x;
                let ey = mid.y;
                match lbl.labelpos {
                    LabelPos::C => {}
                    LabelPos::L => ex -= lbl.labeloffset + lbl.width / 2.0,
                    LabelPos::R => ex += lbl.labeloffset + lbl.width / 2.0,
                }
                lbl.x = Some(ex);
                lbl.y = Some(ey);
            }
        }
    }

    acyclic::undo(g);
}

fn remove_border_nodes(g: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    // First pass: update compound-node geometry from its border nodes.
    let node_ids = g.node_ids();
    for v in &node_ids {
        if g.children(v).is_empty() {
            continue;
        }
        let Some(node) = g.node(v).cloned() else {
            continue;
        };
        let (Some(bt), Some(bb)) = (node.border_top.clone(), node.border_bottom.clone()) else {
            continue;
        };

        let bl = node.border_left.last().and_then(|v| v.as_ref()).cloned();
        let br = node.border_right.last().and_then(|v| v.as_ref()).cloned();
        let (Some(bl), Some(br)) = (bl, br) else {
            continue;
        };

        let Some(t) = g.node(&bt) else {
            continue;
        };
        let Some(b) = g.node(&bb) else {
            continue;
        };
        let Some(l) = g.node(&bl) else {
            continue;
        };
        let Some(r) = g.node(&br) else {
            continue;
        };

        let (Some(ty), Some(by)) = (t.y, b.y) else {
            continue;
        };
        let (Some(lx), Some(rx)) = (l.x, r.x) else {
            continue;
        };

        let width = (rx - lx).abs();
        let height = (by - ty).abs();
        if let Some(n) = g.node_mut(v) {
            n.width = width;
            n.height = height;
            n.x = Some(lx + width / 2.0);
            n.y = Some(ty + height / 2.0);
        }
    }

    // Second pass: remove all border dummy nodes.
    let mut to_remove: Vec<String> = Vec::new();
    for v in g.node_ids() {
        let Some(node) = g.node(&v) else {
            continue;
        };
        if node.dummy.as_deref() == Some("border") {
            to_remove.push(v);
        }
    }
    for v in to_remove {
        let _ = g.remove_node(&v);
    }
}

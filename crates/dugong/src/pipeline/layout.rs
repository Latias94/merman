//! Dagre-style layout pipeline.
//!
//! This pipeline follows upstream Dagre's structure more closely (ranking, normalization,
//! ordering, BK positioning, translation).

use crate::graphlib;
use crate::{
    EdgeLabel, GraphLabel, LabelPos, NodeLabel, Point, RankDir, acyclic, add_border_segments,
    coordinate_system, nesting_graph, normalize, order, parent_dummy_chains, position, rank,
    self_edges, util,
};

pub fn layout(g: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    let mut layout_graph = build_layout_graph(g);
    run_layout(&mut layout_graph);
    update_input_graph(g, &layout_graph);
}

// Dagre runs its mutating phases on a temporary graph built from a whitelist of layout inputs.
// Keeping the same ownership boundary prevents rank doubling, dummy metadata, and other
// algorithm-local state from leaking into a later layout of the caller's graph.
fn build_layout_graph(
    input: &graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel> {
    let mut layout = graphlib::Graph::with_capacity(
        graphlib::GraphOptions {
            multigraph: true,
            compound: true,
            directed: true,
        },
        input.node_count(),
        input.edge_count(),
    );
    let graph = input.graph();
    layout.set_graph(GraphLabel {
        rankdir: graph.rankdir,
        nodesep: graph.nodesep,
        ranksep: graph.ranksep,
        edgesep: graph.edgesep,
        marginx: graph.marginx,
        marginy: graph.marginy,
        align: graph.align.clone(),
        ranker: graph.ranker.clone(),
        acyclicer: graph.acyclicer.clone(),
        ..GraphLabel::default()
    });
    layout.set_default_node_label(NodeLabel::default);
    layout.set_default_edge_label(EdgeLabel::default);

    input.for_each_node(|id, node| {
        layout.set_node(
            id,
            NodeLabel {
                width: node.width,
                height: node.height,
                ..NodeLabel::default()
            },
        );
        if let Some(parent) = input.parent(id) {
            layout.set_parent_ref(id, parent);
        }
    });
    input.for_each_edge(|key, edge| {
        layout.set_edge_key(
            key.clone(),
            EdgeLabel {
                width: edge.width,
                height: edge.height,
                labelpos: edge.labelpos,
                labeloffset: edge.labeloffset,
                minlen: edge.minlen,
                weight: edge.weight,
                ..EdgeLabel::default()
            },
        );
    });

    layout
}

fn update_input_graph(
    input: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layout: &graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
) {
    for id in input.node_ids() {
        let Some(layout_node) = layout.node(&id) else {
            continue;
        };
        let is_compound = !input.children(&id).is_empty();
        let Some(input_node) = input.node_mut(&id) else {
            continue;
        };
        input_node.x = layout_node.x;
        input_node.y = layout_node.y;
        if is_compound {
            input_node.width = layout_node.width;
            input_node.height = layout_node.height;
        }
    }

    for key in input.edge_keys() {
        let Some(layout_edge) = layout.edge_by_key(&key) else {
            continue;
        };
        let Some(input_edge) = input.edge_mut_by_key(&key) else {
            continue;
        };
        input_edge.points.clone_from(&layout_edge.points);
        if layout_edge.x.is_some() {
            input_edge.x = layout_edge.x;
            input_edge.y = layout_edge.y;
        }
    }

    input.graph_mut().width = layout.graph().width;
    input.graph_mut().height = layout.graph().height;
}

fn run_layout(g: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    // Mirror Dagre's `makeSpaceForEdgeLabels` so edge-label proxy ranks become integers
    // (we later materialize label nodes in `normalize::run`).
    g.graph_mut().ranksep /= 2.0;
    let rankdir = g.graph().rankdir;
    g.for_each_edge_mut(|_ek, e| {
        e.minlen = e.minlen.max(1).saturating_mul(2);
        if !matches!(e.labelpos, LabelPos::C) {
            match rankdir {
                RankDir::TB | RankDir::BT => e.width += e.labeloffset,
                RankDir::LR | RankDir::RL => e.height += e.labeloffset,
            }
        }
    });
    // Dagre removes self-loops before ranking/normalization and re-inserts them during positioning
    // via dummy "selfedge" nodes. This avoids invalid rank constraints and gives self-loops a
    // deterministic, spacing-aware offset in BK positioning.
    self_edges::remove_self_edges(g);

    let mut has_compound_structure = false;
    g.for_each_node(|id, _n| {
        if g.parent(id).is_some() || g.children_iter(id).next().is_some() {
            has_compound_structure = true;
        }
    });
    let tiny_simple_chain = g.node_count() == 2 && g.edge_count() == 1 && !has_compound_structure;
    let first_edge_pre = if tiny_simple_chain {
        g.edges().next().cloned()
    } else {
        None
    };

    let ran_acyclic = if tiny_simple_chain || g.edge_count() <= 1 {
        false
    } else {
        acyclic::run(g);
        true
    };
    // Mermaid's dagre adapter always enables `compound: true`, and Dagre's ranker expects a
    // connected graph. Nesting graph connects components (even if there are no explicit
    // subgraphs), preventing network-simplex from panicking on disconnected inputs.
    if g.options().compound && !tiny_simple_chain {
        nesting_graph::run(g);
    }

    // Match upstream Dagre: ranking runs on a non-compound view of the graph so cluster nodes
    // (nodes with children) do not participate in ranking / network-simplex connectivity.
    //
    // `nesting_graph::run` materializes border nodes and nesting edges; those border nodes are
    // leaf nodes and remain in the non-compound graph, providing the constraints Dagre expects.
    if tiny_simple_chain {
        // For the smallest flowcharts (e.g. `A --> B`) network-simplex + ordering overhead
        // dominates total runtime. A deterministic direct rank assignment keeps behavior
        // identical for these cases while cutting fixed-cost work.
        g.for_each_node_mut(|_id, n| n.rank = Some(0));
        if let Some(ek) = first_edge_pre.clone() {
            let minlen = g
                .edge_by_key(&ek)
                .map(|e| e.minlen.max(1) as i32)
                .unwrap_or(1);
            if let Some(n) = g.node_mut(&ek.v) {
                n.rank = Some(0);
            }
            if let Some(n) = g.node_mut(&ek.w) {
                n.rank = Some(minlen);
            }
        }
    } else {
        let mut rank_graph = util::as_non_compound_graph(g);
        rank::rank(&mut rank_graph);
        // Mirror Dagre's JS behavior: `rank(asNonCompoundGraph(g))` mutates the same label objects
        // for leaf nodes, but does not propagate ranks to compound nodes (nodes with children).
        //
        // In Rust we don't share label objects between graphs, so we copy ranks explicitly for leaf
        // nodes only.
        for v in g.node_ids() {
            if g.children_iter(&v).next().is_some() {
                continue;
            }
            let Some(rank) = rank_graph.node(&v).and_then(|n| n.rank) else {
                continue;
            };
            if let Some(n) = g.node_mut(&v) {
                n.rank = Some(rank);
            }
        }
    }
    // Mirror Dagre's `injectEdgeLabelProxies` / `removeEdgeLabelProxies` to compute label ranks.
    // These label ranks are used by `normalize::run` to materialize `edge-label` dummy nodes with
    // the correct width/height, letting BK positioning account for edge labels.
    let mut edge_proxy_nodes: Vec<String> = Vec::new();
    // Only clone edge keys when a proxy is actually needed.
    let mut to_proxy: Vec<(graphlib::EdgeKey, i32)> = Vec::new();
    g.for_each_edge(|ek, edge| {
        if edge.width <= 0.0 || edge.height <= 0.0 {
            return;
        }
        let Some(v_rank) = g.node(&ek.v).and_then(|n| n.rank) else {
            return;
        };
        let Some(w_rank) = g.node(&ek.w).and_then(|n| n.rank) else {
            return;
        };
        let rank = (w_rank - v_rank) / 2 + v_rank;
        to_proxy.push((ek.clone(), rank));
    });

    for (ek, rank) in to_proxy {
        let id = util::unique_id("_ep");
        edge_proxy_nodes.push(id.clone());
        g.set_node(
            id,
            NodeLabel {
                rank: Some(rank),
                dummy: Some("edge-proxy".to_string()),
                edge_obj: Some(ek),
                ..Default::default()
            },
        );
    }

    util::remove_empty_ranks(g);

    // Match upstream Dagre: `nestingGraph.cleanup` must happen before ordering/positioning.
    if g.options().compound && !tiny_simple_chain {
        nesting_graph::cleanup(g);
    }

    util::normalize_ranks(g);

    // Remove edge label proxy nodes, storing their rank on the corresponding edge label.
    for v in std::mem::take(&mut edge_proxy_nodes) {
        let Some(node) = g.node(&v).cloned() else {
            let _ = g.remove_node(&v);
            continue;
        };
        if node.dummy.as_deref() != Some("edge-proxy") {
            let _ = g.remove_node(&v);
            continue;
        }
        let Some(edge_obj) = node.edge_obj else {
            let _ = g.remove_node(&v);
            continue;
        };
        if let Some(lbl) = g.edge_mut_by_key(&edge_obj) {
            lbl.label_rank = node.rank;
        }
        let _ = g.remove_node(&v);
    }

    // Defensive parity: if the caller-provided graph already contained edge-proxy nodes,
    // remove them as well to match the previous best-effort behavior.
    let mut leftovers: Vec<String> = Vec::new();
    g.for_each_node(|id, n| {
        if n.dummy.as_deref() == Some("edge-proxy") {
            leftovers.push(id.to_string());
        }
    });
    for v in leftovers {
        let Some(node) = g.node(&v).cloned() else {
            let _ = g.remove_node(&v);
            continue;
        };
        let Some(edge_obj) = node.edge_obj else {
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
    if g.options().compound && !tiny_simple_chain {
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
    if g.options().compound && !tiny_simple_chain {
        parent_dummy_chains::parent_dummy_chains(g);
        add_border_segments::add_border_segments(g);
    }
    if tiny_simple_chain {
        let mut nodes: Vec<(i32, String)> = Vec::with_capacity(g.node_count());
        g.for_each_node(|id, n| nodes.push((n.rank.unwrap_or(0), id.to_string())));
        nodes.sort_by(|a, b| (a.0, a.1.as_str()).cmp(&(b.0, b.1.as_str())));

        let mut cur_rank: Option<i32> = None;
        let mut next_order: usize = 0;
        for (rank, id) in nodes {
            if cur_rank != Some(rank) {
                cur_rank = Some(rank);
                next_order = 0;
            }
            if let Some(n) = g.node_mut(&id) {
                n.order = Some(next_order);
            }
            next_order += 1;
        }
    } else {
        order::order(
            g,
            order::OrderOptions {
                disable_optimal_order_heuristic: false,
            },
        );
    }
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
    let xs = position::bk::position_x_with_layering(g, &layering);
    g.for_each_node_mut(|id, n| {
        n.x = Some(xs.get(id).copied().unwrap_or(0.0));
    });
    // Convert dummy self-edge nodes into self-loop edge point sequences and remove the dummy nodes.
    self_edges::position_self_edges(g);

    // Match upstream Dagre: `removeBorderNodes` runs after positioning and before `normalize.undo`.
    // It sets compound-node geometry (x/y/width/height) from border nodes, then removes all
    // border dummy nodes.
    if g.options().compound && !tiny_simple_chain {
        super::compound::remove_border_nodes(g);
    }

    normalize::undo(g);
    fixup_edge_label_coords(g);
    coordinate_system::undo(g);

    // Translate so the minimum top-left is at (marginx, marginy), matching Dagre's
    // `translateGraph(...)` behavior.
    let mut min_x: f64 = f64::INFINITY;
    let mut min_y: f64 = f64::INFINITY;
    let mut max_x: f64 = f64::NEG_INFINITY;
    let mut max_y: f64 = f64::NEG_INFINITY;
    g.for_each_node(|_id, n| {
        let (Some(x), Some(y)) = (n.x, n.y) else {
            return;
        };
        min_x = min_x.min(x - n.width / 2.0);
        min_y = min_y.min(y - n.height / 2.0);
        max_x = max_x.max(x + n.width / 2.0);
        max_y = max_y.max(y + n.height / 2.0);
    });
    g.for_each_edge(|_ek, lbl| {
        // Match Dagre's `translateGraph(...)`: it computes min/max based on nodes and edge-label
        // boxes, but does not include intermediate edge points. This can leave some internal spline
        // control points with negative coordinates (which Mermaid preserves in `data-points`), while
        // the rendered path remains within the viewBox because `curveBasis` does not pass through
        // those interior points.
        if let (Some(x), Some(y)) = (lbl.x, lbl.y) {
            min_x = min_x.min(x - lbl.width / 2.0);
            min_y = min_y.min(y - lbl.height / 2.0);
            max_x = max_x.max(x + lbl.width / 2.0);
            max_y = max_y.max(y + lbl.height / 2.0);
        }
    });

    if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
        // Dagre shifts the graph by `-(min - margin)` so the smallest x/y becomes `margin`.
        // This is observable in Mermaid flowchart-v2 SVG output where `diagramPadding: 0`
        // still yields a `viewBox` starting at x=8 (the Dagre margin).
        let margin_x = g.graph().marginx;
        let margin_y = g.graph().marginy;
        min_x -= margin_x;
        min_y -= margin_y;
        let graph_width = max_x - min_x + margin_x;
        let graph_height = max_y - min_y + margin_y;
        let dx = -min_x;
        let dy = -min_y;
        g.for_each_node_mut(|_id, n| {
            if let Some(x) = n.x {
                n.x = Some(x + dx);
            }
            if let Some(y) = n.y {
                n.y = Some(y + dy);
            }
        });
        g.for_each_edge_mut(|_ek, lbl| {
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
        });
        g.graph_mut().width = graph_width;
        g.graph_mut().height = graph_height;
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

        if (lbl.width > 0.0 || lbl.height > 0.0)
            && lbl.x.is_none()
            && lbl.y.is_none()
            && let Some(mid) = lbl.points.get(lbl.points.len() / 2).copied()
        {
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
    if ran_acyclic {
        acyclic::undo(g);
    }
}

/// Restore non-centered edge-label geometry after `makeSpaceForEdgeLabels` and normalization.
///
/// Source: `@dagrejs/dagre` `layout.js::fixupEdgeLabelCoords` at the pinned Dagre revision.
fn fixup_edge_label_coords(graph: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>) {
    graph.for_each_edge_mut(|_key, edge| {
        let Some(mut x) = edge.x else {
            return;
        };
        match edge.labelpos {
            LabelPos::C => return,
            LabelPos::L => {
                edge.width -= edge.labeloffset;
                x -= edge.width / 2.0 + edge.labeloffset;
            }
            LabelPos::R => {
                edge.width -= edge.labeloffset;
                x += edge.width / 2.0 + edge.labeloffset;
            }
        }
        edge.x = Some(x);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_layout_graph_interleaves_node_and_parent_insertion() {
        let mut input = graphlib::Graph::new(graphlib::GraphOptions {
            multigraph: true,
            compound: true,
            directed: true,
        });
        input.set_graph(GraphLabel::default());
        input.set_node(
            "child",
            NodeLabel {
                width: 10.0,
                height: 20.0,
                ..Default::default()
            },
        );
        input.set_node("sibling", NodeLabel::default());
        input.set_node(
            "parent",
            NodeLabel {
                width: 30.0,
                height: 40.0,
                ..Default::default()
            },
        );
        input.set_parent("child", "parent");

        assert_eq!(input.node_ids(), ["child", "sibling", "parent"]);

        let layout = build_layout_graph(&input);

        assert_eq!(layout.node_ids(), ["child", "parent", "sibling"]);
        assert_eq!(layout.parent("child"), Some("parent"));
        let parent = layout.node("parent").expect("parent node");
        assert_eq!((parent.width, parent.height), (30.0, 40.0));
    }
}

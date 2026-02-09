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
        super::compound::remove_border_nodes(g);
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

//! Dagre-style layout pipeline.
//!
//! This pipeline follows upstream Dagre's structure more closely (ranking, normalization,
//! ordering, BK positioning, translation).

use rustc_hash::FxHashSet;

use crate::graphlib;
use crate::work::{
    checked_add, checked_mul, checked_n_log_n, checked_ordered_key_updates,
    checked_unparented_parent_batch_work,
};
use crate::{
    EdgeLabel, GraphLabel, LabelPos, NodeLabel, Point, RankDir, acyclic, add_border_segments,
    coordinate_system, nesting_graph, normalize, order, parent_dummy_chains, position, rank,
    self_edges, util,
};
use crate::{LayoutError, NoopWorkControl, WorkControl, WorkError};

/// Runs the canonical Mermaid-compatible Dagre pipeline transactionally.
///
/// The caller graph is updated only after the temporary layout graph completes successfully.
pub fn layout(
    g: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<(), LayoutError> {
    let mut work_control = NoopWorkControl;
    layout_controlled(g, &mut work_control)
}

/// Runs the canonical Dagre pipeline under caller-owned work control.
///
/// All algorithm-local mutation happens on a temporary graph. The caller graph is updated only
/// after every checked charge and layout phase succeeds.
pub fn layout_controlled(
    g: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<(), LayoutError> {
    let mut layout_graph = build_layout_graph(g, work_control)?;
    run_layout(&mut layout_graph, work_control)?;
    update_input_graph(g, &layout_graph, work_control)?;
    Ok(())
}

// Dagre runs its mutating phases on a temporary graph built from a whitelist of layout inputs.
// Keeping the same ownership boundary prevents rank doubling, dummy metadata, and other
// algorithm-local state from leaking into a later layout of the caller's graph.
fn build_layout_graph(
    input: &graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>, WorkError> {
    let numeric_adjacency_entry_count = if input.is_directed() {
        input.directed_array_index_adjacency_entry_count()
    } else {
        // The layout copy is always directed. An undirected source has no maintained directed
        // adjacency counter, so meter and inspect its edge slots before deriving the target's
        // unique ordered endpoint entries.
        work_control.charge(checked_mul(input.edge_slot_count(), 2)?)?;
        directed_copy_array_index_adjacency_entry_count(input)?
    };
    // Pinned Dagre performs one `setNode` and one `setParent` operation per input node. The latter
    // also recreates parentless ordinary root properties, which the optimized reconstruction
    // replays after the atomic parent batch below.
    let node_iteration_work = checked_mul(input.node_order_slot_count(), 2)?;
    // Every numeric node is inserted into the temporary graph's global node order and root
    // compound bucket. Parent-bucket moves are charged separately by the atomic batch owner.
    let numeric_update_count = checked_mul(input.array_index_node_count(), 2)?;
    let numeric_order_work = checked_ordered_key_updates(input.node_count(), numeric_update_count)?;
    let numeric_adjacency_work =
        checked_ordered_key_updates(input.node_count(), numeric_adjacency_entry_count)?;
    let work = checked_add(
        checked_add(
            checked_add(1, node_iteration_work)?,
            input.edge_slot_count(),
        )?,
        checked_add(numeric_order_work, numeric_adjacency_work)?,
    )?;
    work_control.charge(work)?;

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

    let mut parent_assignments = Vec::new();
    let mut ordinary_root_replays = Vec::new();
    let mut numeric_parent_assignments = 0usize;
    let mut created_unseen_parent = false;
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
            // Pinned Dagre calls `setNode(v)` and then `setParent(v, parent(v))` in the same
            // `nodes()` iteration. Graphlib's `setParent` creates an unseen parent immediately,
            // so preserve that observable node order before batching the parent links themselves.
            if !layout.has_node(parent) {
                layout.set_node(parent, NodeLabel::default());
                created_unseen_parent = true;
            }
            let child_ix = layout
                .node_ix(id)
                .expect("every copied child must exist in the layout graph");
            let parent_ix = layout
                .node_ix(parent)
                .expect("every copied parent must exist in the layout graph");
            parent_assignments.push((child_ix, parent_ix));
            numeric_parent_assignments += usize::from(graphlib::is_javascript_array_index(id));
        } else if !graphlib::is_javascript_array_index(id) {
            ordinary_root_replays.push(
                layout
                    .node_ix(id)
                    .expect("every copied root must exist in the layout graph"),
            );
        }
    });
    // `layout` was created above and has not received any parent links yet. Graphlib still
    // validates that construction-only contract atomically, while its work bound can use the
    // exact zero existing-link count instead of treating every allocated slot as a forest edge.
    let parent_work = checked_unparented_parent_batch_work(
        layout.node_slot_count(),
        0,
        parent_assignments.len(),
        numeric_parent_assignments,
    )?;
    // Without an implicitly created parent, removing all parented nodes from the completed root
    // bucket already leaves ordinary roots in input order. Charge and replay only when the early
    // parent insertion can make the official delete/recreate sequence observable.
    let root_replay_work = if created_unseen_parent {
        checked_mul(ordinary_root_replays.len(), 2)?
    } else {
        0
    };
    let relationship_work = checked_add(parent_work, root_replay_work)?;
    if relationship_work != 0 {
        work_control.charge(relationship_work)?;
    }
    if !parent_assignments.is_empty() {
        layout
            .try_set_unparented_parents_ix(&parent_assignments)
            .expect("copying a valid graph's first parent assignments must remain acyclic");
    }
    // Pinned Mermaid's dagre-d3-es calls `setParent(v, undefined)` for every parentless node.
    // Graphlib deletes and recreates that root property, so ordinary string roots follow input
    // node order even when an earlier child caused its unseen parent to be created out of turn.
    // Numeric roots need no replay because JavaScript always enumerates array-index keys in
    // ascending numeric order. Applying all parent removals first and then replaying every final
    // ordinary root in input order preserves the same observable root enumeration without an
    // ancestor walk per parent assignment.
    if created_unseen_parent {
        for root_ix in ordinary_root_replays {
            layout.clear_parent_ix(root_ix);
        }
    }
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

    Ok(layout)
}

fn directed_copy_array_index_adjacency_entry_count(
    input: &graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<usize, WorkError> {
    if input.is_directed() {
        return Ok(input.directed_array_index_adjacency_entry_count());
    }

    let mut endpoint_pairs = FxHashSet::default();
    let mut entry_count = Some(0usize);
    input.for_each_edge_ix(|v_ix, w_ix, _key, _edge| {
        if entry_count.is_none() || !endpoint_pairs.insert((v_ix, w_ix)) {
            return;
        }
        let updates = usize::from(
            input
                .node_id_by_ix(v_ix)
                .is_some_and(graphlib::is_javascript_array_index),
        ) + usize::from(
            input
                .node_id_by_ix(w_ix)
                .is_some_and(graphlib::is_javascript_array_index),
        );
        entry_count = entry_count.and_then(|total| total.checked_add(updates));
    });
    entry_count.ok_or(WorkError::ArithmeticOverflow)
}

fn update_input_graph(
    input: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
    layout: &graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<(), WorkError> {
    // Inspecting the derived point cardinality is itself an edge-slot scan. Charge that bounded
    // preflight before traversing the temporary graph, then charge the complete mutation tranche
    // before touching the caller-owned graph.
    work_control.charge(layout.edge_slot_count())?;
    let edge_point_work = layout.edges().try_fold(0usize, |total, key| {
        checked_add(
            total,
            layout.edge_by_key(key).map_or(0, |edge| edge.points.len()),
        )
    })?;
    let node_snapshot_work = checked_add(input.node_order_slot_count(), input.node_count())?;
    let edge_snapshot_work = checked_add(input.edge_slot_count(), input.edge_count())?;
    let work = checked_add(
        checked_add(checked_add(1, node_snapshot_work)?, edge_snapshot_work)?,
        edge_point_work,
    )?;
    work_control.charge(work)?;

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
    Ok(())
}

fn run_layout(
    g: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<(), LayoutError> {
    // Mirror Dagre's `makeSpaceForEdgeLabels` so edge-label proxy ranks become integers
    // (we later materialize label nodes in `normalize::run`).
    work_control.charge(g.edge_count())?;
    let spaced_minlens = g
        .edges()
        .map(|edge_key| {
            let minlen = g.edge_by_key(edge_key).map_or(1, |edge| edge.minlen);
            let minlen = checked_mul(minlen, 2)?;
            if minlen > i32::MAX as usize {
                return Err(WorkError::ArithmeticOverflow);
            }
            Ok((edge_key.clone(), minlen))
        })
        .collect::<Result<Vec<_>, WorkError>>()?;
    g.graph_mut().ranksep /= 2.0;
    let rankdir = g.graph().rankdir;
    for (edge_key, minlen) in spaced_minlens {
        if let Some(edge) = g.edge_mut_by_key(&edge_key) {
            edge.minlen = minlen;
        }
    }
    g.for_each_edge_mut(|_ek, e| {
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
    work_control.charge(g.edge_count())?;
    self_edges::remove_self_edges(g);

    work_control.charge(g.node_order_slot_count())?;
    let mut has_compound_structure = false;
    g.for_each_node(|id, _n| {
        if g.parent(id).is_some() || g.children_iter(id).next().is_some() {
            has_compound_structure = true;
        }
    });
    let uses_network_simplex = !matches!(
        g.graph().ranker.as_deref(),
        Some("longest-path" | "tight-tree")
    );
    let tiny_simple_chain = g.node_count() == 2
        && g.edge_count() == 1
        && !has_compound_structure
        && uses_network_simplex;
    let first_edge_pre = if tiny_simple_chain {
        g.edges().next().cloned()
    } else {
        None
    };

    let ran_acyclic = if tiny_simple_chain || g.edge_count() <= 1 {
        false
    } else {
        acyclic::run_controlled(g, work_control)?;
        true
    };
    // Mermaid's dagre adapter always enables `compound: true`, and Dagre's ranker expects a
    // connected graph. Nesting graph connects components (even if there are no explicit
    // subgraphs), preventing network-simplex from panicking on disconnected inputs.
    if g.options().compound && !tiny_simple_chain {
        nesting_graph::run_controlled(g, work_control)?;
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
            let minlen = match g.edge_by_key(&ek) {
                // The shortcut replaces network-simplex, whose `simplify` stage floors a simple
                // edge to one even when the source label explicitly carries zero.
                Some(edge) => {
                    i32::try_from(edge.minlen.max(1)).map_err(|_| WorkError::ArithmeticOverflow)?
                }
                None => 1,
            };
            if let Some(n) = g.node_mut(&ek.v) {
                n.rank = Some(0);
            }
            if let Some(n) = g.node_mut(&ek.w) {
                n.rank = Some(minlen);
            }
        }
    } else {
        let mut rank_graph = build_non_compound_rank_graph(g, work_control)?;
        rank::rank_controlled(&mut rank_graph, work_control)?;
        // Mirror Dagre's JS behavior: `rank(asNonCompoundGraph(g))` mutates the same label objects
        // for leaf nodes, but does not propagate ranks to compound nodes (nodes with children).
        //
        // In Rust we don't share label objects between graphs, so we copy ranks explicitly for leaf
        // nodes only.
        work_control.charge(node_snapshot_work_units(g)?)?;
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
    work_control.charge(g.edge_slot_count())?;
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

    work_control.charge(to_proxy.len())?;
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

    work_control.charge(remove_empty_ranks_work_units(g)?)?;
    util::remove_empty_ranks(g);

    // Match upstream Dagre: `nestingGraph.cleanup` must happen before ordering/positioning.
    if g.options().compound && !tiny_simple_chain {
        charge_graph_scan(g, work_control)?;
        nesting_graph::cleanup(g);
    }

    work_control.charge(checked_mul(g.node_order_slot_count(), 2)?)?;
    util::normalize_ranks(g);

    // Finish every label-rank writeback before the single graph-wide proxy retirement pass.
    charge_graph_scan(g, work_control)?;
    remove_edge_label_proxies(g, edge_proxy_nodes);

    // Dagre uses `assignRankMinMax` to annotate compound nodes with their rank span, derived from
    // the `nestingGraph` border top/bottom nodes. This rank span is later used by subgraph
    // ordering and border segment generation.
    if g.options().compound && !tiny_simple_chain {
        work_control.charge(node_snapshot_work_units(g)?)?;
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

    let normalization_plan = prepare_normalization(g, work_control)?;
    normalize::run_planned(g, normalization_plan);
    if g.options().compound && !tiny_simple_chain {
        parent_dummy_chains::parent_dummy_chains_controlled(g, work_control)?;
        add_border_segments::add_border_segments_controlled(g, work_control)?;
    }
    if tiny_simple_chain {
        work_control.charge(node_snapshot_work_units(g)?)?;
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
        order::order_controlled(
            g,
            order::OrderOptions {
                disable_optimal_order_heuristic: false,
            },
            work_control,
        )?;
    }
    // Positioning runs in TB coordinates; `coordinate_system::adjust` maps LR/RL/BT into TB.
    charge_graph_scan(g, work_control)?;
    coordinate_system::adjust(g);

    // Insert dummy self-edge nodes after ordering and rankdir transforms, so their sizes match the
    // active coordinate system (TB) and they can influence BK x-positioning.
    charge_graph_scan(g, work_control)?;
    self_edges::insert_self_edges(g);

    let rank_sep = g.graph().ranksep;
    work_control.charge(node_snapshot_work_units(g)?)?;
    let layering = util::build_layer_matrix(g);

    let layering_nodes = layering
        .iter()
        .try_fold(0usize, |total, layer| checked_add(total, layer.len()))?;
    work_control.charge(layering_nodes)?;
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
    let xs = position::bk::position_x_with_layering_controlled(g, &layering, work_control)?;
    work_control.charge(g.node_order_slot_count())?;
    g.for_each_node_mut(|id, n| {
        n.x = Some(xs.get(id).copied().unwrap_or(0.0));
    });
    // Convert dummy self-edge nodes into self-loop edge point sequences and remove the dummy nodes.
    charge_graph_scan(g, work_control)?;
    self_edges::position_self_edges(g);

    // Match upstream Dagre: `removeBorderNodes` runs after positioning and before `normalize.undo`.
    // It sets compound-node geometry (x/y/width/height) from border nodes, then removes all
    // border dummy nodes.
    if g.options().compound && !tiny_simple_chain {
        charge_graph_scan(g, work_control)?;
        super::compound::remove_border_nodes(g);
    }

    charge_graph_scan(g, work_control)?;
    normalize::undo(g);
    work_control.charge(g.edge_slot_count())?;
    fixup_edge_label_coords(g);
    charge_graph_scan(g, work_control)?;
    coordinate_system::undo(g);

    // Translate so the minimum top-left is at (marginx, marginy), matching Dagre's
    // `translateGraph(...)` behavior.
    let translation_and_route_plan_work = checked_add(graph_scan_work_units(g)?, g.edge_count())?;
    work_control.charge(translation_and_route_plan_work)?;
    let mut min_x: f64 = f64::INFINITY;
    let mut min_y: f64 = f64::INFINITY;
    let mut max_x: f64 = f64::NEG_INFINITY;
    let mut max_y: f64 = f64::NEG_INFINITY;
    let mut translation_edge_work = Some(0usize);
    let mut edge_route_materialization_work = Some(0usize);
    let mut edge_keys = Vec::with_capacity(g.edge_count());
    g.for_each_node(|_id, n| {
        let (Some(x), Some(y)) = (n.x, n.y) else {
            return;
        };
        min_x = min_x.min(x - n.width / 2.0);
        min_y = min_y.min(y - n.height / 2.0);
        max_x = max_x.max(x + n.width / 2.0);
        max_y = max_y.max(y + n.height / 2.0);
    });
    g.for_each_edge(|edge_key, lbl| {
        edge_keys.push(edge_key.clone());
        translation_edge_work = translation_edge_work.and_then(|total| {
            total
                .checked_add(1)
                .and_then(|next| next.checked_add(lbl.points.len()))
        });
        edge_route_materialization_work = edge_route_materialization_work.and_then(|total| {
            total
                .checked_add(lbl.points.len())
                .and_then(|next| next.checked_add(2))
        });
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
    let translation_edge_work = translation_edge_work.ok_or(WorkError::ArithmeticOverflow)?;
    let edge_route_materialization_work =
        edge_route_materialization_work.ok_or(WorkError::ArithmeticOverflow)?;

    if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
        let translation_work = checked_add(g.node_count(), translation_edge_work)?;
        work_control.charge(translation_work)?;
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
    // Match Dagre's `assignNodeIntersects`: preserve normalized internal points exactly, or aim
    // directly at the opposite node center when normalization produced none.
    work_control.charge(edge_route_materialization_work)?;
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

        let internal = lbl.points.clone();
        let (first, last) = match (internal.first().copied(), internal.last().copied()) {
            (Some(first), Some(last)) => (first, last),
            _ => (Point { x: tx, y: ty }, Point { x: sx, y: sy }),
        };

        // Mermaid's Dagre companion throws when either route point is exactly at the center of
        // its endpoint rectangle. Preserve that observable failure boundary, but keep the caller
        // graph transactional and return a typed Rust error.
        if first.x == sx && first.y == sy {
            return Err(LayoutError::DegenerateEdgeGeometry {
                edge: e.clone(),
                node: e.v.clone(),
            });
        }
        if last.x == tx && last.y == ty {
            return Err(LayoutError::DegenerateEdgeGeometry {
                edge: e.clone(),
                node: e.w.clone(),
            });
        }

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
        charge_graph_scan(g, work_control)?;
        acyclic::undo(g);
    }
    Ok(())
}

fn charge_graph_scan(
    g: &graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<(), WorkError> {
    work_control.charge(graph_scan_work_units(g)?)
}

fn graph_scan_work_units(
    g: &graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<usize, WorkError> {
    checked_add(g.node_order_slot_count(), g.edge_slot_count())
}

fn build_non_compound_rank_graph(
    g: &graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>, WorkError> {
    let numeric_order_work =
        checked_ordered_key_updates(g.node_count(), g.array_index_node_count())?;
    let numeric_adjacency_work = checked_ordered_key_updates(
        g.node_count(),
        g.directed_array_index_adjacency_entry_count(),
    )?;
    work_control.charge(checked_add(
        graph_scan_work_units(g)?,
        checked_add(numeric_order_work, numeric_adjacency_work)?,
    )?)?;
    Ok(util::as_non_compound_graph(g))
}

fn node_snapshot_work_units(
    g: &graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<usize, WorkError> {
    checked_add(g.node_order_slot_count(), g.node_count())
}

fn edge_snapshot_work_units(
    g: &graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<usize, WorkError> {
    checked_add(g.edge_slot_count(), g.edge_count())
}

fn prepare_normalization(
    g: &graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<Vec<graphlib::EdgeKey>, WorkError> {
    // The stable edge snapshot is itself input-dependent work. Admit the slot scan and one key
    // clone per live edge before allocating the plan, then admit every planned edge mutation and
    // derived dummy node before normalization changes the temporary graph.
    work_control.charge(edge_snapshot_work_units(g)?)?;
    let mut to_normalize = Vec::new();
    let mut materialization_work = 0usize;
    for edge in g.edges() {
        let v_rank = g.node(&edge.v).and_then(|node| node.rank).unwrap_or(0);
        let w_rank = g.node(&edge.w).and_then(|node| node.rank).unwrap_or(0);
        if w_rank == v_rank + 1 {
            continue;
        }
        let gap = usize::try_from((i64::from(w_rank) - i64::from(v_rank) - 1).max(0))
            .map_err(|_| WorkError::ArithmeticOverflow)?;
        materialization_work = checked_add(materialization_work, checked_add(1, gap)?)?;
        to_normalize.push(edge.clone());
    }
    work_control.charge(materialization_work)?;
    Ok(to_normalize)
}

fn remove_empty_ranks_work_units(
    g: &graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<usize, WorkError> {
    checked_add(g.node_count(), checked_n_log_n(g.node_count())?)
}

/// Writes proxy ranks back before retiring every proxy through one graph-wide removal pass.
///
/// Generated proxies retain the upstream writeback order. Caller-provided defensive leftovers are
/// processed afterwards in stable node order, matching the former two-loop behavior when multiple
/// proxies target the same edge.
fn remove_edge_label_proxies(
    g: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
    generated_proxy_ids: Vec<String>,
) -> usize {
    remove_edge_label_proxies_with(g, generated_proxy_ids, remove_nodes_batch)
}

fn remove_edge_label_proxies_with<F>(
    g: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
    mut generated_proxy_ids: Vec<String>,
    retire_batch: F,
) -> usize
where
    F: FnOnce(&mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>, &[String]) -> usize,
{
    for id in &generated_proxy_ids {
        write_edge_label_proxy_rank(g, id);
    }

    let leftover_ids = {
        let generated_ids: FxHashSet<&str> =
            generated_proxy_ids.iter().map(String::as_str).collect();
        let mut leftovers = Vec::new();
        g.for_each_node(|id, node| {
            if node.dummy.as_deref() == Some("edge-proxy") && !generated_ids.contains(id) {
                leftovers.push(id.to_string());
            }
        });
        leftovers
    };

    for id in &leftover_ids {
        write_edge_label_proxy_rank(g, id);
    }
    generated_proxy_ids.extend(leftover_ids);

    if generated_proxy_ids.is_empty() {
        return 0;
    }

    retire_batch(g, &generated_proxy_ids)
}

fn remove_nodes_batch(
    g: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ids: &[String],
) -> usize {
    g.remove_nodes(ids.iter().map(String::as_str))
}

fn write_edge_label_proxy_rank(
    g: &mut graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
    id: &str,
) {
    let writeback = g.node(id).and_then(|node| {
        if node.dummy.as_deref() != Some("edge-proxy") {
            return None;
        }
        node.edge_obj.clone().map(|edge_obj| (edge_obj, node.rank))
    });
    let Some((edge_obj, rank)) = writeback else {
        return;
    };
    if let Some(label) = g.edge_mut_by_key(&edge_obj) {
        label.label_rank = rank;
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

    #[derive(Default)]
    struct RecordingWorkControl {
        charges: Vec<usize>,
        remaining: Option<usize>,
    }

    impl RecordingWorkControl {
        fn with_limit(limit: usize) -> Self {
            Self {
                remaining: Some(limit),
                ..Self::default()
            }
        }
    }

    impl WorkControl for RecordingWorkControl {
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.charges.push(units);
            let Some(remaining) = self.remaining else {
                return Ok(());
            };
            let Some(next) = remaining.checked_sub(units) else {
                return Err(WorkError::Interrupted);
            };
            self.remaining = Some(next);
            Ok(())
        }
    }

    fn test_graph() -> graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel> {
        graphlib::Graph::new(graphlib::GraphOptions {
            multigraph: true,
            compound: true,
            ..Default::default()
        })
    }

    fn mixed_adjacency_graph(
        directed: bool,
        numeric: bool,
    ) -> graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = graphlib::Graph::new(graphlib::GraphOptions {
            multigraph: true,
            compound: true,
            directed,
        });
        let ids = if numeric {
            ["0", "node-a", "1", "node-b"]
        } else {
            ["node-0", "node-a", "node-1", "node-b"]
        };
        for id in ids {
            graph.set_node(id, NodeLabel::default());
        }
        graph.set_edge_named(
            ids[0],
            ids[1],
            Some("parallel-0"),
            Some(EdgeLabel::default()),
        );
        graph.set_edge_named(
            ids[0],
            ids[1],
            Some("parallel-1"),
            Some(EdgeLabel::default()),
        );
        graph.set_edge_named(ids[1], ids[2], Some("mixed"), Some(EdgeLabel::default()));
        graph.set_edge_named(ids[0], ids[2], Some("numeric"), Some(EdgeLabel::default()));
        graph.set_edge_named(ids[2], ids[2], Some("self"), Some(EdgeLabel::default()));
        graph.set_edge_named(ids[1], ids[3], Some("ordinary"), Some(EdgeLabel::default()));
        graph
    }

    #[test]
    fn build_layout_graph_precharges_descending_numeric_object_key_work() {
        for width in (0..=10).map(|shift| 1usize << shift) {
            let mut graph = test_graph();
            for index in (0..width).rev() {
                graph.set_node(index.to_string(), NodeLabel::default());
            }

            let height = crate::work::ceil_log2(width) + 1;
            let numeric_updates = width * 2;
            let expected = 1 + width * 2 + numeric_updates * height;

            let mut measured = RecordingWorkControl::default();
            let rebuilt = build_layout_graph(&graph, &mut measured)
                .expect("the unbounded control admits the numeric graph rebuild");
            assert_eq!(measured.charges, vec![expected]);
            assert_eq!(
                rebuilt.node_ids(),
                (0..width)
                    .map(|index| index.to_string())
                    .collect::<Vec<_>>()
            );

            let mut below = RecordingWorkControl::with_limit(expected - 1);
            assert!(matches!(
                build_layout_graph(&graph, &mut below),
                Err(WorkError::Interrupted)
            ));
            assert_eq!(below.charges, vec![expected]);

            for limit in [expected, expected + 1] {
                let mut admitted = RecordingWorkControl::with_limit(limit);
                let rebuilt = build_layout_graph(&graph, &mut admitted)
                    .expect("equal and above numeric object-key budgets succeed");
                assert_eq!(rebuilt.node_count(), width);
            }
        }
    }

    #[test]
    fn build_layout_graph_precharges_undirected_numeric_adjacency_copy() {
        let input = mixed_adjacency_graph(false, true);
        let source_nodes = input.node_ids();
        let source_edges = input.edge_keys();
        let adjacency_entries = 6;
        let scan_work = checked_mul(input.edge_slot_count(), 2).unwrap();
        let numeric_node_work = checked_ordered_key_updates(
            input.node_count(),
            checked_mul(input.array_index_node_count(), 2).unwrap(),
        )
        .unwrap();
        let numeric_adjacency_work =
            checked_ordered_key_updates(input.node_count(), adjacency_entries).unwrap();
        let initial_work = checked_add(
            checked_add(
                checked_add(1, checked_mul(input.node_order_slot_count(), 2).unwrap()).unwrap(),
                input.edge_slot_count(),
            )
            .unwrap(),
            checked_add(numeric_node_work, numeric_adjacency_work).unwrap(),
        )
        .unwrap();
        let exact = checked_add(scan_work, initial_work).unwrap();

        let mut measured = RecordingWorkControl::default();
        let rebuilt = build_layout_graph(&input, &mut measured)
            .expect("the unbounded control admits the undirected numeric copy");
        assert_eq!(measured.charges, [scan_work, initial_work]);
        assert!(rebuilt.is_directed());
        assert_eq!(
            rebuilt.directed_array_index_adjacency_entry_count(),
            adjacency_entries
        );

        let mut below = RecordingWorkControl::with_limit(exact - 1);
        assert!(matches!(
            build_layout_graph(&input, &mut below),
            Err(WorkError::Interrupted)
        ));
        assert_eq!(below.charges, [scan_work, initial_work]);
        assert_eq!(below.remaining, Some(initial_work - 1));
        assert_eq!(input.node_ids(), source_nodes);
        assert_eq!(input.edge_keys(), source_edges);

        for limit in [exact, exact + 1] {
            let mut admitted = RecordingWorkControl::with_limit(limit);
            let rebuilt = build_layout_graph(&input, &mut admitted)
                .expect("equal and above undirected numeric copy budgets succeed");
            assert_eq!(
                rebuilt.directed_array_index_adjacency_entry_count(),
                adjacency_entries
            );
        }

        let directed = mixed_adjacency_graph(true, true);
        assert_eq!(
            directed.directed_array_index_adjacency_entry_count(),
            adjacency_entries
        );
        let directed_initial_work = checked_add(
            checked_add(
                checked_add(1, checked_mul(directed.node_order_slot_count(), 2).unwrap()).unwrap(),
                directed.edge_slot_count(),
            )
            .unwrap(),
            checked_add(
                checked_ordered_key_updates(
                    directed.node_count(),
                    checked_mul(directed.array_index_node_count(), 2).unwrap(),
                )
                .unwrap(),
                checked_ordered_key_updates(
                    directed.node_count(),
                    directed.directed_array_index_adjacency_entry_count(),
                )
                .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let mut directed_measured = RecordingWorkControl::default();
        let rebuilt = build_layout_graph(&directed, &mut directed_measured)
            .expect("the directed copy retains its single exact construction charge");
        assert_eq!(directed_measured.charges, [directed_initial_work]);
        assert_eq!(
            rebuilt.directed_array_index_adjacency_entry_count(),
            adjacency_entries
        );
    }

    #[test]
    fn rank_graph_copy_precharges_mixed_numeric_adjacency() {
        let numeric = mixed_adjacency_graph(true, true);
        let ordinary = mixed_adjacency_graph(true, false);
        let adjacency_entries = numeric.directed_array_index_adjacency_entry_count();
        assert_eq!(adjacency_entries, 6);
        assert_eq!(ordinary.directed_array_index_adjacency_entry_count(), 0);

        let numeric_node_work =
            checked_ordered_key_updates(numeric.node_count(), numeric.array_index_node_count())
                .unwrap();
        let numeric_adjacency_work =
            checked_ordered_key_updates(numeric.node_count(), adjacency_entries).unwrap();
        let numeric_exact = checked_add(
            graph_scan_work_units(&numeric).unwrap(),
            checked_add(numeric_node_work, numeric_adjacency_work).unwrap(),
        )
        .unwrap();
        let ordinary_exact = graph_scan_work_units(&ordinary).unwrap();

        let mut numeric_measured = RecordingWorkControl::default();
        let copied = build_non_compound_rank_graph(&numeric, &mut numeric_measured)
            .expect("the unbounded control admits the numeric rank copy");
        assert_eq!(numeric_measured.charges, [numeric_exact]);
        assert_eq!(
            copied.directed_array_index_adjacency_entry_count(),
            adjacency_entries
        );

        let mut ordinary_measured = RecordingWorkControl::default();
        let copied = build_non_compound_rank_graph(&ordinary, &mut ordinary_measured)
            .expect("the unbounded control admits the ordinary rank copy");
        assert_eq!(ordinary_measured.charges, [ordinary_exact]);
        assert_eq!(copied.directed_array_index_adjacency_entry_count(), 0);
        assert_eq!(
            numeric_exact,
            checked_add(
                ordinary_exact,
                checked_add(numeric_node_work, numeric_adjacency_work).unwrap(),
            )
            .unwrap()
        );

        let source_nodes = numeric.node_ids();
        let source_edges = numeric.edge_keys();
        let mut below = RecordingWorkControl::with_limit(numeric_exact - 1);
        assert!(matches!(
            build_non_compound_rank_graph(&numeric, &mut below),
            Err(WorkError::Interrupted)
        ));
        assert_eq!(below.charges, [numeric_exact]);
        assert_eq!(below.remaining, Some(numeric_exact - 1));
        assert_eq!(numeric.node_ids(), source_nodes);
        assert_eq!(numeric.edge_keys(), source_edges);

        for limit in [numeric_exact, numeric_exact + 1] {
            let mut admitted = RecordingWorkControl::with_limit(limit);
            let copied = build_non_compound_rank_graph(&numeric, &mut admitted)
                .expect("equal and above numeric rank-copy budgets succeed");
            assert_eq!(copied.edge_count(), numeric.edge_count());
        }
    }

    #[test]
    fn build_layout_graph_precharges_numeric_directed_adjacency_work() {
        for width in (0..=8).map(|shift| 1usize << shift) {
            let mut graph = test_graph();
            graph.set_node("source", NodeLabel::default());
            for index in (0..width).rev() {
                let id = index.to_string();
                graph.set_node(id.clone(), NodeLabel::default());
                graph.set_edge("source", id);
            }

            let node_count = width + 1;
            let numeric_updates = width * 3;
            let expected = 1
                + node_count * 2
                + width
                + checked_ordered_key_updates(node_count, numeric_updates).unwrap();

            let mut measured = RecordingWorkControl::default();
            let rebuilt = build_layout_graph(&graph, &mut measured)
                .expect("the unbounded control admits numeric adjacency reconstruction");
            assert_eq!(measured.charges, vec![expected]);
            assert_eq!(
                rebuilt.successors("source"),
                (0..width)
                    .map(|index| index.to_string())
                    .collect::<Vec<_>>()
            );

            let mut below = RecordingWorkControl::with_limit(expected - 1);
            assert!(matches!(
                build_layout_graph(&graph, &mut below),
                Err(WorkError::Interrupted)
            ));
            assert_eq!(below.charges, vec![expected]);

            for limit in [expected, expected + 1] {
                let mut admitted = RecordingWorkControl::with_limit(limit);
                build_layout_graph(&graph, &mut admitted)
                    .expect("equal and above numeric adjacency budgets succeed");
            }
        }
    }

    #[test]
    fn build_layout_graph_meters_undirected_numeric_adjacency_planning() {
        for width in (0..=8).map(|shift| 1usize << shift) {
            let mut graph = graphlib::Graph::new(graphlib::GraphOptions {
                directed: false,
                multigraph: true,
                compound: true,
            });
            graph.set_node("source", NodeLabel::default());
            for index in (0..width).rev() {
                let id = index.to_string();
                graph.set_node(id.clone(), NodeLabel::default());
                graph.set_edge("source", id);
            }

            let node_count = width + 1;
            let planning_work = width * 2;
            let reconstruction_work = 1
                + node_count * 2
                + width
                + checked_ordered_key_updates(node_count, width * 3).unwrap();
            let exact = planning_work + reconstruction_work;

            let mut measured = RecordingWorkControl::default();
            let rebuilt = build_layout_graph(&graph, &mut measured)
                .expect("the unbounded control admits undirected adjacency planning");
            assert_eq!(measured.charges, vec![planning_work, reconstruction_work]);
            assert_eq!(rebuilt.edge_count(), width);

            let mut below = RecordingWorkControl::with_limit(exact - 1);
            assert!(matches!(
                build_layout_graph(&graph, &mut below),
                Err(WorkError::Interrupted)
            ));
            assert_eq!(below.charges, vec![planning_work, reconstruction_work]);

            for limit in [exact, exact + 1] {
                let mut admitted = RecordingWorkControl::with_limit(limit);
                build_layout_graph(&graph, &mut admitted)
                    .expect("equal and above undirected numeric adjacency budgets succeed");
            }
        }
    }

    #[test]
    fn build_layout_graph_batches_deep_numeric_parent_chains_with_bounded_work() {
        for width in (1..=10).map(|shift| 1usize << shift) {
            let mut graph = test_graph();
            for index in 0..width {
                graph.set_node(index.to_string(), NodeLabel::default());
            }
            let source_assignments = (1..width)
                .map(|index| {
                    (
                        graph.node_ix(&index.to_string()).unwrap(),
                        graph.node_ix(&(index - 1).to_string()).unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            graph
                .try_set_unparented_parents_ix(&source_assignments)
                .expect("the source chain is acyclic and uses first assignments");

            let height = crate::work::ceil_log2(width) + 1;
            let initial_work = 1 + width * 2 + width * 2 * height;
            let parent_work = checked_unparented_parent_batch_work(
                width,
                0,
                source_assignments.len(),
                source_assignments.len(),
            )
            .unwrap();

            let mut measured = RecordingWorkControl::default();
            let rebuilt = build_layout_graph(&graph, &mut measured)
                .expect("the unbounded control admits the batched deep chain");
            assert_eq!(measured.charges, vec![initial_work, parent_work]);
            for index in 1..width {
                let child = index.to_string();
                let expected_parent = (index - 1).to_string();
                assert_eq!(rebuilt.parent(&child), Some(expected_parent.as_str()));
            }

            let exact = initial_work + parent_work;
            let mut below = RecordingWorkControl::with_limit(exact - 1);
            assert!(matches!(
                build_layout_graph(&graph, &mut below),
                Err(WorkError::Interrupted)
            ));
            assert_eq!(below.charges, vec![initial_work, parent_work]);

            for limit in [exact, exact + 1] {
                let mut admitted = RecordingWorkControl::with_limit(limit);
                let rebuilt = build_layout_graph(&graph, &mut admitted)
                    .expect("equal and above deep-chain budgets succeed");
                assert_eq!(rebuilt.node_count(), width);
            }
        }
    }

    #[test]
    fn graph_work_helpers_include_interior_order_and_edge_tombstones() {
        let mut graph = test_graph();
        for id in ["a", "b", "c", "d"] {
            graph.set_node(id, NodeLabel::default());
        }
        graph.set_edge("a", "c");
        graph.set_edge("c", "d");
        graph.node_mut("c").unwrap().rank = Some(0);
        graph.node_mut("d").unwrap().rank = Some(3);
        assert!(graph.remove_node("b"));
        assert!(graph.remove_edge("a", "c", None));

        assert_eq!(graph.node_count(), 3);
        assert_eq!(graph.node_order_slot_count(), 4);
        assert_eq!(graph.edge_count(), 1);
        assert_eq!(graph.edge_slot_count(), 2);
        assert_eq!(graph_scan_work_units(&graph), Ok(6));
        assert_eq!(node_snapshot_work_units(&graph), Ok(7));
        assert_eq!(edge_snapshot_work_units(&graph), Ok(3));

        let mut normalize_work = RecordingWorkControl::default();
        let normalization_plan = prepare_normalization(&graph, &mut normalize_work).unwrap();
        assert_eq!(normalize_work.charges, vec![3, 3]);
        assert_eq!(normalization_plan.len(), 1);

        let mut rejected = RecordingWorkControl::with_limit(2);
        assert!(matches!(
            prepare_normalization(&graph, &mut rejected),
            Err(WorkError::Interrupted)
        ));
        assert_eq!(rejected.charges, vec![3]);
    }

    #[test]
    fn edge_label_proxies_write_back_then_retire_in_one_batch() {
        let mut graph = test_graph();
        for id in [
            "a",
            "b",
            "c",
            "d",
            "e",
            "f",
            "cluster",
            "keep_before_child",
            "generated",
            "generated_only",
            "generated_non_proxy",
            "caller_proxy",
            "orphan_proxy",
            "keep_after_child",
        ] {
            graph.set_node(id, NodeLabel::default());
        }
        graph.set_parent("keep_before_child", "cluster");
        graph.set_parent("generated_non_proxy", "cluster");
        graph.set_parent("caller_proxy", "cluster");
        graph.set_parent("orphan_proxy", "cluster");
        graph.set_parent("keep_after_child", "cluster");

        graph.set_edge_named("c", "d", Some("keep_before"), Some(EdgeLabel::default()));
        graph.set_edge_named("a", "b", Some("target"), Some(EdgeLabel::default()));
        graph.set_edge_named(
            "e",
            "f",
            Some("generated_target"),
            Some(EdgeLabel::default()),
        );
        graph.set_edge_named(
            "generated",
            "a",
            Some("drop_generated"),
            Some(EdgeLabel::default()),
        );
        graph.set_edge_named("d", "c", Some("keep_after"), Some(EdgeLabel::default()));
        graph.set_edge_named(
            "caller_proxy",
            "b",
            Some("drop_caller"),
            Some(EdgeLabel::default()),
        );

        let target = graphlib::EdgeKey::new("a", "b", Some("target"));
        let generated_target = graphlib::EdgeKey::new("e", "f", Some("generated_target"));
        graph.set_node(
            "generated",
            NodeLabel {
                rank: Some(4),
                dummy: Some("edge-proxy".to_string()),
                edge_obj: Some(target.clone()),
                ..Default::default()
            },
        );
        graph.set_node(
            "generated_only",
            NodeLabel {
                rank: Some(6),
                dummy: Some("edge-proxy".to_string()),
                edge_obj: Some(generated_target),
                ..Default::default()
            },
        );
        graph.set_node(
            "generated_non_proxy",
            NodeLabel {
                rank: Some(5),
                dummy: Some("other".to_string()),
                edge_obj: Some(target.clone()),
                ..Default::default()
            },
        );
        graph.set_node(
            "caller_proxy",
            NodeLabel {
                rank: Some(7),
                dummy: Some("edge-proxy".to_string()),
                edge_obj: Some(target),
                ..Default::default()
            },
        );
        graph.set_node(
            "orphan_proxy",
            NodeLabel {
                rank: Some(9),
                dummy: Some("edge-proxy".to_string()),
                ..Default::default()
            },
        );

        let mut retirement_passes = 0;
        let removed = remove_edge_label_proxies_with(
            &mut graph,
            vec![
                "generated".to_string(),
                "generated_only".to_string(),
                "missing_generated".to_string(),
                "generated_non_proxy".to_string(),
            ],
            |graph, ids| {
                retirement_passes += 1;
                remove_nodes_batch(graph, ids)
            },
        );

        assert_eq!(removed, 5);
        assert_eq!(retirement_passes, 1);
        for removed_id in [
            "generated",
            "generated_only",
            "generated_non_proxy",
            "caller_proxy",
            "orphan_proxy",
        ] {
            assert!(!graph.has_node(removed_id));
        }
        assert_eq!(
            graph.node_ids(),
            vec![
                "a",
                "b",
                "c",
                "d",
                "e",
                "f",
                "cluster",
                "keep_before_child",
                "keep_after_child",
            ]
        );
        assert_eq!(
            graph.children("cluster"),
            vec!["keep_before_child", "keep_after_child"]
        );
        assert_eq!(
            graph.edge("a", "b", Some("target")).unwrap().label_rank,
            Some(7),
            "the caller-provided leftover must keep the former last-write-wins order",
        );
        assert_eq!(
            graph
                .edge("e", "f", Some("generated_target"))
                .unwrap()
                .label_rank,
            Some(6),
            "a generated proxy must write back independently of leftover collisions",
        );
        assert_eq!(
            graph
                .edge_keys()
                .into_iter()
                .map(|edge| edge.name.unwrap())
                .collect::<Vec<_>>(),
            vec!["keep_before", "target", "generated_target", "keep_after"],
        );
    }

    #[test]
    fn edge_label_proxy_retirement_skips_the_batch_when_no_proxy_exists() {
        let mut graph = test_graph();
        graph.set_node("keep", NodeLabel::default());

        let mut retirement_passes = 0;
        assert_eq!(
            remove_edge_label_proxies_with(&mut graph, Vec::new(), |graph, ids| {
                retirement_passes += 1;
                remove_nodes_batch(graph, ids)
            }),
            0
        );
        assert_eq!(retirement_passes, 0);
        assert!(graph.has_node("keep"));
    }

    fn proxy_curve_graph(
        count: usize,
    ) -> (
        graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
        Vec<String>,
    ) {
        let mut graph = test_graph();
        graph.set_node("a", NodeLabel::default());
        graph.set_node("b", NodeLabel::default());
        graph.set_edge_named("a", "b", Some("target"), Some(EdgeLabel::default()));
        let target = graphlib::EdgeKey::new("a", "b", Some("target"));
        let mut ids = Vec::with_capacity(count);
        for index in 0..count {
            let id = format!("proxy_{index}");
            graph.set_node(
                &id,
                NodeLabel {
                    rank: Some(i32::try_from(index).unwrap()),
                    dummy: Some("edge-proxy".to_string()),
                    edge_obj: Some(target.clone()),
                    ..Default::default()
                },
            );
            ids.push(id);
        }
        (graph, ids)
    }

    #[test]
    fn edge_label_proxy_retirement_curve_reduces_live_removals_to_one_graph_pass() {
        for count in [1, 2, 4, 8, 16, 32, 64, 128] {
            let (mut sequential, ids) = proxy_curve_graph(count);
            let mut sequential_removals = 0;
            for id in &ids {
                write_edge_label_proxy_rank(&mut sequential, id);
                if sequential.remove_node(id) {
                    sequential_removals += 1;
                }
            }

            let (mut batched, ids) = proxy_curve_graph(count);
            let mut graph_passes = 0;
            let removed = remove_edge_label_proxies_with(&mut batched, ids, |graph, ids| {
                graph_passes += 1;
                remove_nodes_batch(graph, ids)
            });

            assert_eq!(sequential_removals, count);
            assert_eq!(removed, count);
            assert_eq!(graph_passes, 1);
            assert_eq!(batched.node_ids(), sequential.node_ids());
            assert_eq!(batched.edge_keys(), sequential.edge_keys());
            assert_eq!(
                batched.edge("a", "b", Some("target")),
                sequential.edge("a", "b", Some("target")),
            );
        }
    }

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

        let mut work_control = NoopWorkControl;
        let layout = build_layout_graph(&input, &mut work_control)
            .expect("the checked no-op work control must admit graph construction");

        assert_eq!(layout.node_ids(), ["child", "parent", "sibling"]);
        assert_eq!(layout.parent("child"), Some("parent"));
        assert_eq!(layout.children_root(), ["sibling", "parent"]);
        let parent = layout.node("parent").expect("parent node");
        assert_eq!((parent.width, parent.height), (30.0, 40.0));
    }

    fn build_layout_graph_sequential_parent_reference(
        input: &graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut layout = graphlib::Graph::new(graphlib::GraphOptions {
            multigraph: true,
            compound: true,
            directed: true,
        });
        layout.set_default_node_label(NodeLabel::default);
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
            } else {
                layout.clear_parent(id);
            }
        });
        layout
    }

    fn assert_parent_shape_matches_sequential_reference(
        input: &graphlib::Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) {
        let expected = build_layout_graph_sequential_parent_reference(input);
        let mut work_control = NoopWorkControl;
        let actual = build_layout_graph(input, &mut work_control)
            .expect("the checked no-op work control must admit graph construction");

        assert_eq!(actual.node_ids(), expected.node_ids());
        assert_eq!(actual.children_root(), expected.children_root());
        for id in expected.node_ids() {
            assert_eq!(actual.parent(&id), expected.parent(&id), "parent of {id}");
            assert_eq!(
                actual.children(&id),
                expected.children(&id),
                "children of {id}"
            );
            let actual_node = actual.node(&id).expect("actual node");
            let expected_node = expected.node(&id).expect("reference node");
            assert_eq!(
                (actual_node.width, actual_node.height),
                (expected_node.width, expected_node.height),
                "copied label of {id}"
            );
        }
    }

    #[test]
    fn build_layout_graph_matches_sequential_graphlib_parent_replay() {
        let cases = [
            (
                vec!["child", "sibling", "parent"],
                vec![("child", "parent")],
            ),
            (
                vec!["leaf", "root", "middle", "tail"],
                vec![("leaf", "middle"), ("middle", "root")],
            ),
            (
                vec!["10", "1", "3", "ordinary-child", "ordinary-parent", "root"],
                vec![("1", "10"), ("ordinary-child", "ordinary-parent")],
            ),
        ];

        for (node_ids, parent_links) in cases {
            let mut input = test_graph();
            for (index, id) in node_ids.into_iter().enumerate() {
                input.set_node(
                    id,
                    NodeLabel {
                        width: index as f64 + 1.0,
                        height: index as f64 + 11.0,
                        ..NodeLabel::default()
                    },
                );
            }
            for (child, parent) in parent_links {
                input.set_parent(child, parent);
            }
            assert_parent_shape_matches_sequential_reference(&input);
        }
    }

    #[test]
    fn build_layout_graph_precharges_implicit_parent_root_replay() {
        let mut input = test_graph();
        for id in ["child", "sibling", "parent"] {
            input.set_node(id, NodeLabel::default());
        }
        input.set_parent("child", "parent");

        let initial_work = 1 + input.node_order_slot_count() * 2;
        let relationship_work = checked_unparented_parent_batch_work(3, 0, 1, 0).unwrap() + 4;
        let exact = initial_work + relationship_work;

        let mut measured = RecordingWorkControl::default();
        let rebuilt = build_layout_graph(&input, &mut measured)
            .expect("the unbounded control admits implicit-parent reconstruction");
        assert_eq!(measured.charges, [initial_work, relationship_work]);
        assert_eq!(rebuilt.children_root(), ["sibling", "parent"]);

        let mut below = RecordingWorkControl::with_limit(exact - 1);
        assert!(matches!(
            build_layout_graph(&input, &mut below),
            Err(WorkError::Interrupted)
        ));
        assert_eq!(below.charges, [initial_work, relationship_work]);

        for limit in [exact, exact + 1] {
            let mut admitted = RecordingWorkControl::with_limit(limit);
            let rebuilt = build_layout_graph(&input, &mut admitted)
                .expect("equal and above implicit-parent budgets succeed");
            assert_eq!(rebuilt.children_root(), ["sibling", "parent"]);
        }
    }
}

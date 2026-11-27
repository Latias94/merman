use crate::layout_config::FlowchartLayoutSettings;
use crate::projection::{FlowchartEdgeKey, FlowchartRenderModelRef};
use merman_layout_elk as elk;
use merman_model::families::flowchart::{FlowSubgraph, FlowchartModel};
use merman_render_core::elk_adapter::{
    drawing_group_frames,
    hierarchy::{charge_adapter_work, checked_adapter_add, checked_adapter_mul},
};
use merman_render_core::layout_work::OperationLayoutWorkControl;
use merman_render_core::{
    Bounds, Error, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint, Result,
};
use std::collections::{HashMap, HashSet};

/// Solved Flow geometry and semantic occurrence keys in provider edge order.
#[derive(Debug)]
pub struct FlowchartElkProjection {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub edge_keys: Vec<FlowchartEdgeKey>,
    pub clusters: Vec<LayoutCluster>,
    pub bounds: Option<Bounds>,
    pub dom_node_order_by_root: HashMap<String, Vec<String>>,
}

/// Projects the measured graph after the operation has restored feedback edge orientation.
pub fn project_layout(
    model: FlowchartRenderModelRef<'_>,
    settings: FlowchartLayoutSettings,
    graph: &elk::Graph,
    layout: elk::LayoutResult,
    mut work_control: Option<&mut OperationLayoutWorkControl>,
) -> Result<FlowchartElkProjection> {
    let FlowchartLayoutSettings {
        cluster_padding,
        title_margin_top,
        title_margin_bottom,
        ..
    } = settings;

    let source_index_work = checked_adapter_add(
        &work_control,
        checked_adapter_add(&work_control, graph.nodes.len(), graph.edges.len())?,
        model.nodes.len(),
    )?;
    charge_adapter_work(&mut work_control, source_index_work)?;
    let source_node_by_id: HashMap<&str, &elk::Node> = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let projected_edges = model.projected_edges().collect::<Vec<_>>();
    if graph.edges.len() != projected_edges.len() {
        return Err(Error::InvalidModel {
            message: format!(
                "ELK transport edge count {} does not match projected edge count {}",
                graph.edges.len(),
                projected_edges.len()
            ),
        });
    }
    let mut source_edge_by_transport_id = HashMap::with_capacity(graph.edges.len());
    for (edge, (owner, projected)) in graph.edges.iter().zip(&projected_edges) {
        let semantic_index = owner.semantic_index();
        if edge.source != projected.from || edge.target != projected.to {
            return Err(Error::InvalidModel {
                message: format!(
                    "ELK transport edge {} endpoints do not match projected owner {}",
                    edge.id, semantic_index
                ),
            });
        }
        if source_edge_by_transport_id
            .insert(edge.id.as_str(), (edge, *owner, projected))
            .is_some()
        {
            return Err(Error::InvalidModel {
                message: format!("ELK graph contains duplicate transport edge {}", edge.id),
            });
        }
    }

    let source_shape_by_id: HashMap<&str, Option<&str>> = model
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.layout_shape.as_deref()))
        .collect();

    let group_frames = drawing_group_frames(
        graph,
        &layout,
        |node| node.label.map_or(0.0, |label| label.width) + node.container.padding,
        &mut work_control,
    )?;

    charge_adapter_work(&mut work_control, layout.nodes.len())?;
    let mut out_nodes = Vec::with_capacity(layout.nodes.len());
    for node in layout.nodes {
        let Some(source) = source_node_by_id.get(node.id.as_str()).copied() else {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned unknown node {}", node.id),
            });
        };
        // Frame contraction changes only the painted container. Provider edge points already
        // use the original owner's absolute origin and must not follow this translation.
        let frame = group_frames.get(node.id.as_str());
        out_nodes.push(LayoutNode {
            id: node.id,
            x: frame.map_or(node.x, |frame| frame.x),
            y: frame.map_or(node.y, |frame| frame.y),
            width: frame.map_or(node.width, |frame| frame.width),
            height: frame.map_or(node.height, |frame| frame.height),
            is_cluster: source.kind == elk::NodeKind::Group,
            label_width: source.label.map(|label| label.width),
            label_height: source.label.map(|label| label.height),
        });
    }

    // Mermaid's applyElkEdgeLayout aligns small nodes before constructing any edge points.
    // Keep ELK's routes intact: a clamped port defines the line onto which its node must move.
    let alignment_work = checked_adapter_add(
        &work_control,
        out_nodes.len(),
        checked_adapter_mul(&work_control, layout.edges.len(), 3)?,
    )?;
    charge_adapter_work(&mut work_control, alignment_work)?;
    {
        let mut nodes_to_align = HashMap::with_capacity(out_nodes.len());
        for node in &mut out_nodes {
            if let Some(source) = source_node_by_id.get(node.id.as_str()) {
                nodes_to_align.insert(source.id.as_str(), (node, false));
            }
        }
        for edge in &layout.edges {
            let (Some(start), Some(end)) = (edge.points.first(), edge.points.last()) else {
                continue;
            };
            let Some((source, _, _)) = source_edge_by_transport_id.get(edge.id.as_str()) else {
                return Err(Error::InvalidModel {
                    message: format!("ELK layout returned unknown edge {}", edge.id),
                });
            };
            if !nodes_to_align.contains_key(source.source.as_str())
                || !nodes_to_align.contains_key(source.target.as_str())
            {
                continue;
            }
            for (id, anchor) in [
                (source.source.as_str(), start),
                (source.target.as_str(), end),
            ] {
                if source_shape_by_id.get(id).copied().flatten() == Some("rect33") {
                    continue;
                }
                if let Some((node, aligned)) = nodes_to_align.get_mut(id) {
                    align_degenerate_node_to_anchor(node, anchor, aligned);
                }
            }
        }
    }

    charge_adapter_work(&mut work_control, out_nodes.len())?;
    let layout_node_by_id: HashMap<&str, &LayoutNode> = out_nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let diagram_direction = model.direction.as_deref().unwrap_or("TB");
    charge_adapter_work(&mut work_control, model.subgraphs.len())?;
    let canonical_subgraphs = canonical_subgraphs(&model);
    let mut clusters = Vec::with_capacity(canonical_subgraphs.len());
    for (_, sg) in canonical_subgraphs {
        if model.collapsed_replacement(sg.id.as_str()).is_some()
            || model.is_subgraph_collapsed(sg.id.as_str())
        {
            continue;
        }
        let Some(node) = layout_node_by_id.get(sg.id.as_str()).copied() else {
            return Err(Error::InvalidModel {
                message: format!("missing ELK layout cluster {}", sg.id),
            });
        };
        let label = source_node_by_id
            .get(sg.id.as_str())
            .and_then(|node| node.label)
            .unwrap_or(elk::Label {
                width: 1.0,
                height: 1.0,
            });
        let title_label = LayoutLabel {
            x: node.x,
            y: node.y - node.height / 2.0 + title_margin_top + label.height / 2.0,
            width: label.width,
            height: label.height,
        };
        let title_w = label.width.max(1.0);
        let diff = if node.width <= title_w {
            (title_w - node.width) / 2.0 - cluster_padding / 2.0
        } else {
            -cluster_padding / 2.0
        };
        clusters.push(LayoutCluster {
            id: sg.id.clone(),
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            diff,
            offset_y: label.height - cluster_padding / 2.0,
            title: sg.title.clone(),
            title_label,
            requested_dir: sg.dir.as_ref().map(|dir| normalize_flow_direction(dir)),
            effective_dir: sg
                .dir
                .as_deref()
                .map(normalize_flow_direction)
                .unwrap_or_else(|| normalize_flow_direction(diagram_direction)),
            padding: cluster_padding,
            title_margin_top,
            title_margin_bottom,
        });
    }
    let cluster_sort_work = comparison_sort_work_units(clusters.len(), &work_control)?;
    charge_adapter_work(&mut work_control, cluster_sort_work)?;
    clusters.sort_by(|a, b| a.id.cmp(&b.id));

    let edge_projection_work = layout.edges.iter().try_fold(0usize, |total, edge| {
        let point_walks = checked_adapter_mul(&work_control, edge.points.len(), 2)?;
        let edge_work = checked_adapter_add(
            &work_control,
            checked_adapter_add(&work_control, point_walks, edge.labels.len())?,
            3,
        )?;
        checked_adapter_add(&work_control, total, edge_work)
    })?;
    // Reject the complete user-sized projection tranche before reserving its output vector.
    charge_adapter_work(&mut work_control, edge_projection_work)?;
    let mut out_edges = Vec::with_capacity(layout.edges.len());
    let mut edge_keys = Vec::with_capacity(layout.edges.len());
    for edge in layout.edges {
        let Some((source, owner, projected_edge)) =
            source_edge_by_transport_id.get(edge.id.as_str()).copied()
        else {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned unknown edge {}", edge.id),
            });
        };
        let Some(semantic_edge) = model.edges.get(owner.semantic_index()) else {
            return Err(Error::InvalidModel {
                message: format!(
                    "ELK layout edge {} references missing semantic owner {}",
                    edge.id,
                    owner.semantic_index()
                ),
            });
        };
        if source.source != projected_edge.from || source.target != projected_edge.to {
            return Err(Error::InvalidModel {
                message: format!(
                    "ELK transport edge {} endpoints do not match semantic owner {}",
                    edge.id,
                    owner.semantic_index()
                ),
            });
        }
        let points = edge
            .points
            .into_iter()
            .map(|point| LayoutPoint {
                x: point.x,
                y: point.y,
            })
            .collect::<Vec<_>>();
        let label = source.label.and_then(|source_label| {
            edge.labels
                .first()
                .map(edge_label_layout)
                .or_else(|| edge_label_position(&points, source_label))
        });
        out_edges.push(LayoutEdge {
            id: semantic_edge.id.clone(),
            from: projected_edge.from.to_owned(),
            to: projected_edge.to.to_owned(),
            from_cluster: endpoint_cluster(projected_edge.from, &layout_node_by_id),
            to_cluster: endpoint_cluster(projected_edge.to, &layout_node_by_id),
            points,
            label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        });
        edge_keys.push(owner);
    }

    let bounds_points = out_edges.iter().try_fold(
        checked_adapter_mul(&work_control, out_nodes.len(), 2)?,
        |total, edge| {
            let label_points = usize::from(edge.label.is_some()) * 2;
            checked_adapter_add(
                &work_control,
                total,
                checked_adapter_add(&work_control, edge.points.len(), label_points)?,
            )
        },
    )?;
    let bounds_work = checked_adapter_mul(&work_control, bounds_points, 2)?;
    charge_adapter_work(&mut work_control, bounds_work)?;
    let bounds = compute_bounds(&out_nodes, &out_edges);
    let dom_node_order_by_root = flowchart_elk_dom_node_order_by_root(graph, &mut work_control)?;

    Ok(FlowchartElkProjection {
        nodes: out_nodes,
        edges: out_edges,
        edge_keys,
        clusters,
        bounds,
        dom_node_order_by_root,
    })
}

/// Keeps the first declaration of each subgraph and its original semantic index.
pub fn canonical_subgraphs(model: &FlowchartModel) -> Vec<(usize, &FlowSubgraph)> {
    let mut seen = HashSet::with_capacity(model.subgraphs.len());
    let mut canonical = Vec::with_capacity(model.subgraphs.len());
    for (semantic_index, subgraph) in model.subgraphs.iter().enumerate() {
        if seen.insert(subgraph.id.as_str()) {
            canonical.push((semantic_index, subgraph));
        }
    }
    canonical
}

fn compute_bounds(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> Option<Bounds> {
    fn corners(x: f64, y: f64, width: f64, height: f64) -> [(f64, f64); 2] {
        let hw = width / 2.0;
        let hh = height / 2.0;
        [(x - hw, y - hh), (x + hw, y + hh)]
    }

    let node_corners = nodes
        .iter()
        .flat_map(|node| corners(node.x, node.y, node.width, node.height));
    let edge_points = edges.iter().flat_map(|edge| {
        let points = edge.points.iter().map(|point| (point.x, point.y));
        let label_corners = edge
            .label
            .iter()
            .flat_map(|label| corners(label.x, label.y, label.width, label.height));
        points.chain(label_corners)
    });
    Bounds::from_points(node_corners.chain(edge_points))
}

// Pinned Mermaid elk/render.ts: alignDegenerateNodeToAnchor. The 12px margin is also
// configured by merman-layout-elk; only sides shorter than twice that margin are affected.
pub fn align_degenerate_node_to_anchor(
    node: &mut LayoutNode,
    anchor: &elk::Point,
    aligned: &mut bool,
) {
    let top = node.y - node.height / 2.0;
    let bottom = node.y + node.height / 2.0;
    let along_width = (anchor.y - top).abs() <= 0.5 || (anchor.y - bottom).abs() <= 0.5;
    let side = if along_width { node.width } else { node.height };
    if side >= 24.0 || *aligned {
        return;
    }
    // Even a negligible first adjustment claims the node; later edges must not move it.
    *aligned = true;
    let delta = if along_width {
        anchor.x - node.x
    } else {
        anchor.y - node.y
    };
    if delta.abs() < 0.01 {
        return;
    }
    if along_width {
        node.x += delta;
    } else {
        node.y += delta;
    }
}

fn flowchart_elk_dom_node_order_by_root(
    graph: &elk::Graph,
    work_control: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<HashMap<String, Vec<String>>> {
    // Mermaid's flat paint list is stably sorted after layout, independently from ELK's
    // recursive graph traversal. Admit that sorting work before retaining output ids.
    let sort_work = comparison_sort_work_units(graph.nodes.len(), work_control)?;
    let dom_work = checked_adapter_add(
        work_control,
        checked_adapter_mul(work_control, graph.nodes.len(), 2)?,
        checked_adapter_add(work_control, 1, sort_work)?,
    )?;
    charge_adapter_work(work_control, dom_work)?;
    let ids = mermaid_elk_paint_order(graph);
    Ok(std::iter::once((String::new(), ids)).collect())
}

/// Matches Mermaid 12.1's `orderNodesForElkPaint`: groups first by parent depth,
/// followed by leaves in their original flat input order.
pub fn mermaid_elk_paint_order(graph: &elk::Graph) -> Vec<String> {
    let node_index_by_id = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let group_depths = mermaid_elk_group_depths(graph, &node_index_by_id);
    let mut indices = (0..graph.nodes.len()).collect::<Vec<_>>();
    indices.sort_by(|left, right| {
        let left_node = &graph.nodes[*left];
        let right_node = &graph.nodes[*right];
        match (left_node.kind, right_node.kind) {
            (elk::NodeKind::Group, elk::NodeKind::Leaf) => std::cmp::Ordering::Less,
            (elk::NodeKind::Leaf, elk::NodeKind::Group) => std::cmp::Ordering::Greater,
            (elk::NodeKind::Group, elk::NodeKind::Group) => {
                group_depths[*left].cmp(&group_depths[*right])
            }
            (elk::NodeKind::Leaf, elk::NodeKind::Leaf) => std::cmp::Ordering::Equal,
        }
    });
    indices
        .into_iter()
        .map(|index| graph.nodes[index].id.clone())
        .collect()
}

/// Computes group-parent depth without recursion or repeated ancestor scans.
/// The generation guard also bounds malformed diagnostic graphs with cycles.
fn mermaid_elk_group_depths(
    graph: &elk::Graph,
    node_index_by_id: &HashMap<&str, usize>,
) -> Vec<usize> {
    let mut depths = vec![0usize; graph.nodes.len()];
    let mut state = vec![0u8; graph.nodes.len()];
    let mut active_generation = vec![usize::MAX; graph.nodes.len()];
    let mut active_position = vec![0usize; graph.nodes.len()];
    let mut generation = 0usize;

    for start in 0..graph.nodes.len() {
        if graph.nodes[start].kind != elk::NodeKind::Group || state[start] == 2 {
            continue;
        }

        generation = generation.saturating_add(1);
        let mut path = Vec::new();
        let mut current = start;
        let terminal_depth = loop {
            if state[current] == 2 {
                break Some(depths[current].saturating_add(1));
            }
            if active_generation[current] == generation {
                let cycle_start = active_position[current];
                let cycle_len = path.len().saturating_sub(cycle_start);
                for &index in &path[cycle_start..] {
                    depths[index] = cycle_len;
                    state[index] = 2;
                }

                // Match the upstream visited-set guard: enter a cycle once, then stop.
                let mut prefix_depth = cycle_len;
                for &index in path[..cycle_start].iter().rev() {
                    depths[index] = prefix_depth;
                    state[index] = 2;
                    prefix_depth = prefix_depth.saturating_add(1);
                }
                break None;
            }

            active_generation[current] = generation;
            active_position[current] = path.len();
            state[current] = 1;
            path.push(current);

            let Some(parent_id) = graph.nodes[current].parent.as_deref() else {
                break Some(0);
            };
            let Some(&parent_index) = node_index_by_id.get(parent_id) else {
                break Some(0);
            };
            if graph.nodes[parent_index].kind != elk::NodeKind::Group {
                break Some(0);
            }
            current = parent_index;
        };

        let Some(mut depth) = terminal_depth else {
            continue;
        };
        for &index in path.iter().rev() {
            depths[index] = depth;
            state[index] = 2;
            depth = depth.saturating_add(1);
        }
    }

    depths
}

fn normalize_flow_direction(dir: &str) -> String {
    let upper = dir.trim().to_uppercase();
    if upper == "TD" {
        "TB".to_string()
    } else {
        upper
    }
}

fn endpoint_cluster(id: &str, layout_node_by_id: &HashMap<&str, &LayoutNode>) -> Option<String> {
    layout_node_by_id
        .get(id)
        .filter(|node| node.is_cluster)
        .map(|node| node.id.clone())
}

fn edge_label_layout(label: &elk::EdgeLabelLayout) -> LayoutLabel {
    LayoutLabel {
        x: label.x + label.width / 2.0,
        y: label.y + label.height / 2.0,
        width: label.width,
        height: label.height,
    }
}

fn edge_label_position(points: &[LayoutPoint], label: elk::Label) -> Option<LayoutLabel> {
    let point = polyline_midpoint(points)?;
    Some(LayoutLabel {
        x: point.x,
        y: point.y,
        width: label.width,
        height: label.height,
    })
}

fn polyline_midpoint(points: &[LayoutPoint]) -> Option<LayoutPoint> {
    match points {
        [] => None,
        [single] => Some(single.clone()),
        _ => {
            let total = points
                .windows(2)
                .map(|pair| (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y))
                .sum::<f64>();
            if !total.is_finite() || total <= 0.0 {
                return points.first().cloned();
            }
            let mut remaining = total / 2.0;
            for pair in points.windows(2) {
                let len = (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y);
                if len <= 0.0 {
                    continue;
                }
                if remaining > len {
                    remaining -= len;
                    continue;
                }
                let t = remaining / len;
                return Some(LayoutPoint {
                    x: pair[0].x + (pair[1].x - pair[0].x) * t,
                    y: pair[0].y + (pair[1].y - pair[0].y) * t,
                });
            }
            points.last().cloned()
        }
    }
}

fn comparison_sort_work_units(
    items: usize,
    work_control: &Option<&mut OperationLayoutWorkControl>,
) -> Result<usize> {
    if items < 2 {
        return Ok(0);
    }
    let levels = usize::BITS as usize - (items - 1).leading_zeros() as usize;
    checked_adapter_mul(work_control, items, levels)
}

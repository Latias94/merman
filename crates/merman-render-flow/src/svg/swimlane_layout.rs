//! Geometry projection from a measured Swimlane layout to the Flow SVG graph.

use crate::cluster_shell::swimlane_terminal_geometry;
use merman_render_core::{
    LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, SwimlaneDirection, SwimlaneEdgeLayout,
    SwimlaneLaneLayout, SwimlaneNodeLayout,
};
use rustc_hash::FxHashMap;

/// Projects SVG geometry in the original node, edge occurrence, and lane order.
///
/// Use the canonical mode of the same layout that supplied these rows. The caller retains the
/// associated edge owners, graph bounds, and route contract.
pub fn project_swimlane_layout(
    nodes: &[SwimlaneNodeLayout],
    edges: &[SwimlaneEdgeLayout],
    lanes: &[SwimlaneLaneLayout],
    direction: SwimlaneDirection,
    canonical: bool,
) -> (Vec<LayoutNode>, Vec<LayoutEdge>, Vec<LayoutCluster>) {
    let label_nodes: FxHashMap<&str, &SwimlaneNodeLayout> = nodes
        .iter()
        .filter(|node| node.is_edge_label)
        .map(|node| (node.id.as_str(), node))
        .collect();

    let nodes = nodes
        .iter()
        .filter(|node| !node.is_edge_label)
        .map(|node| LayoutNode {
            id: node.id.clone(),
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            is_cluster: false,
            label_width: Some(node.label_width),
            label_height: Some(node.label_height),
        })
        .collect();

    let edges = edges
        .iter()
        .map(|edge| {
            let label = edge
                .label_node_id
                .as_deref()
                .and_then(|id| label_nodes.get(id).copied())
                .map(|node| LayoutLabel {
                    x: node.x,
                    y: node.y,
                    width: node.label_width,
                    height: node.label_height,
                });
            LayoutEdge {
                id: edge.id.clone(),
                from: edge.from.clone(),
                to: edge.to.clone(),
                from_cluster: None,
                to_cluster: None,
                points: edge.points.clone(),
                label,
                start_label_left: None,
                start_label_right: None,
                end_label_left: None,
                end_label_right: None,
                start_marker: None,
                end_marker: None,
                stroke_dasharray: None,
            }
        })
        .collect();
    let clusters = lanes
        .iter()
        .map(|lane| {
            let geometry = swimlane_terminal_geometry(lane, direction, canonical);
            LayoutCluster {
                id: lane.id.clone(),
                x: lane.x,
                y: lane.y,
                width: geometry.width,
                height: geometry.height,
                diff: 0.0,
                offset_y: 0.0,
                title: lane.title.clone(),
                title_label: geometry.title_label,
                requested_dir: lane.requested_dir.clone(),
                effective_dir: if canonical {
                    lane.requested_dir.as_deref().unwrap_or(direction.as_str())
                } else {
                    direction.as_str()
                }
                .to_owned(),
                padding: lane.padding,
                title_margin_top: 0.0,
                title_margin_bottom: 0.0,
            }
        })
        .collect();

    (nodes, edges, clusters)
}

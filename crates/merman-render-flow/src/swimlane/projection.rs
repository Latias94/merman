//! Common layout projection for the measured Swimlane provider.

use super::working::{WorkingLayout, WorkingNodeKind};
use merman_render_core::{
    Bounds, SwimlaneDirection, SwimlaneEdgeLayout, SwimlaneLaneLayout, SwimlaneNodeLayout,
};
use std::collections::HashMap;

/// Owned common fields of the facade's Swimlane layout. The facade adds route and render data.
pub type SwimlaneLayoutParts = (
    SwimlaneDirection,
    Vec<SwimlaneNodeLayout>,
    Vec<SwimlaneLaneLayout>,
    Vec<SwimlaneEdgeLayout>,
    Option<Bounds>,
);

fn output_bounds(layout: &WorkingLayout) -> Option<Bounds> {
    let mut points = Vec::new();
    for node in layout
        .nodes
        .values()
        .filter(|node| node.kind != WorkingNodeKind::Dummy)
    {
        let width = if node.kind == WorkingNodeKind::EdgeLabel {
            node.label_width
        } else {
            node.width
        };
        let height = if node.kind == WorkingNodeKind::EdgeLabel {
            node.label_height
        } else {
            node.height
        };
        points.push((node.x - width / 2.0, node.y - height / 2.0));
        points.push((node.x + width / 2.0, node.y + height / 2.0));
    }
    for edge in &layout.original_edges {
        points.extend(edge.points.iter().map(|point| (point.x, point.y)));
    }
    Bounds::from_points(points)
}

/// Projects measured nodes and original edge occurrences in their working order.
pub fn project_layout(
    working: WorkingLayout,
    curve_by_id: &HashMap<&str, &str>,
) -> SwimlaneLayoutParts {
    let bounds = output_bounds(&working);
    let mut nodes = Vec::new();
    let mut lanes = Vec::new();
    for node in working.nodes.into_values() {
        match node.kind {
            WorkingNodeKind::Content | WorkingNodeKind::EdgeLabel => {
                nodes.push(SwimlaneNodeLayout {
                    id: node.id,
                    label: node.label,
                    label_type: node.label_type,
                    shape: node.shape,
                    parent_id: node.parent_id,
                    top_lane_id: node.top_lane_id,
                    x: node.x,
                    y: node.y,
                    width: node.width,
                    height: node.height,
                    label_width: node.label_width,
                    label_height: node.label_height,
                    layer: node.layer,
                    order: node.order,
                    is_edge_label: node.kind == WorkingNodeKind::EdgeLabel,
                })
            }
            WorkingNodeKind::Group => lanes.push(SwimlaneLaneLayout {
                id: node.id,
                title: node.label,
                parent_id: node.parent_id,
                x: node.x,
                y: node.y,
                width: node.width,
                height: node.height,
                padding: node.padding,
                title_label_width: node.label_width,
                title_label_height: node.label_height,
                content_top: node.content_top,
                title_rect: node.title_rect,
                requested_dir: node.requested_dir,
            }),
            WorkingNodeKind::Dummy => {}
        }
    }
    let edges = working
        .original_edges
        .into_iter()
        .map(|edge| {
            let curve = match curve_by_id
                .get(edge.id.as_str())
                .copied()
                .unwrap_or("basis")
            {
                "basis" => "rounded",
                curve => curve,
            };
            SwimlaneEdgeLayout {
                id: edge.id,
                from: edge.from,
                to: edge.to,
                points: edge.points,
                label_node_id: edge.label_node_id,
                reversed_for_layout: edge.reversed_for_layout,
                curve: curve.into(),
            }
        })
        .collect();

    (working.direction, nodes, lanes, edges, bounds)
}

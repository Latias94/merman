use super::BlockShapeGeometry;
use crate::model::{LayoutEdge, LayoutPoint};

fn block_edge_start_marker_inset(arrow: Option<&str>) -> f64 {
    match arrow.unwrap_or("").trim() {
        "arrow_point" => 4.5,
        _ => 0.0,
    }
}

fn block_edge_end_marker_inset(arrow: Option<&str>) -> f64 {
    match arrow.unwrap_or("").trim() {
        "arrow_point" => 4.0,
        _ => 0.0,
    }
}

fn move_point_towards(point: &LayoutPoint, target: &LayoutPoint, distance: f64) -> LayoutPoint {
    if distance.abs() <= 1e-12 {
        return point.clone();
    }
    let dx = target.x - point.x;
    let dy = target.y - point.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= 1e-12 {
        return point.clone();
    }
    LayoutPoint {
        x: point.x + dx / len * distance,
        y: point.y + dy / len * distance,
    }
}

pub(crate) struct BlockEdgePoints {
    /// Points before marker insets, matching Mermaid's `data-points` payload.
    pub(crate) data_points: Vec<LayoutPoint>,
    /// Points after marker insets, matching the emitted path and marker anchors.
    pub(crate) rendered_points: Vec<LayoutPoint>,
}

pub(crate) fn block_render_edge_points(
    edge: &merman_model::families::block::BlockEdge,
    layout_edge: &LayoutEdge,
    shape_geometries_by_id: &std::collections::HashMap<&str, &BlockShapeGeometry>,
) -> BlockEdgePoints {
    let data_points = match (
        shape_geometries_by_id.get(edge.start.as_str()),
        shape_geometries_by_id.get(edge.end.as_str()),
    ) {
        (Some(from), Some(to)) => {
            let mid = layout_edge.points.get(1).cloned().unwrap_or(LayoutPoint {
                x: from.allocated.x + (to.allocated.x - from.allocated.x) / 2.0,
                y: from.allocated.y + (to.allocated.y - from.allocated.y) / 2.0,
            });
            vec![from.intersect(&mid), mid.clone(), to.intersect(&mid)]
        }
        _ => layout_edge.points.clone(),
    };
    let mut rendered_points = data_points.clone();

    if rendered_points.len() >= 2 {
        let start_inset = block_edge_start_marker_inset(edge.arrow_type_start.as_deref());
        if start_inset > 0.0 {
            rendered_points[0] =
                move_point_towards(&rendered_points[0], &rendered_points[1], start_inset);
        }
        let end_inset = block_edge_end_marker_inset(edge.arrow_type_end.as_deref());
        if end_inset > 0.0 {
            let last = rendered_points.len() - 1;
            rendered_points[last] = move_point_towards(
                &rendered_points[last],
                &rendered_points[last - 1],
                end_inset,
            );
        }
    }

    BlockEdgePoints {
        data_points,
        rendered_points,
    }
}

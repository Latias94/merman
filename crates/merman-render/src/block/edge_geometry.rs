use super::{BlockShapeGeometry, BlockTypographyThemePlan};
use crate::model::{BlockDiagramLayout, Bounds, LayoutEdge, LayoutPoint};
use crate::resources::OperationWorkMeter;
use merman_model::families::block::{BlockDiagramRenderModel, BlockEdge};
use merman_theme_contract::v2::EdgeStyle;
use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Default)]
pub(crate) struct BlockEdgeRoutes {
    #[cfg(feature = "layout-shoal")]
    orthogonal: Vec<Option<BlockOrthogonalRoute>>,
}

#[derive(Debug)]
pub(crate) struct BlockOrthogonalRoute {
    pub(crate) path: String,
    pub(crate) bounds: Bounds,
    pub(crate) marker_refs: [LayoutPoint; 2],
}

impl BlockEdgeRoutes {
    pub(crate) fn prepare(
        model: &BlockDiagramRenderModel,
        layout: &mut BlockDiagramLayout,
        typography: &BlockTypographyThemePlan,
        #[cfg(feature = "layout-shoal")] backend: crate::layout_backend::GraphLayoutBackend,
        work: &Arc<OperationWorkMeter>,
    ) -> crate::Result<Self> {
        #[cfg(feature = "layout-shoal")]
        if !layout.edges.is_empty() && backend == crate::layout_backend::GraphLayoutBackend::Shoal {
            return Ok(Self {
                orthogonal: super::orthogonal::prepare(model, layout, typography, work)?,
            });
        }
        #[cfg(not(feature = "layout-shoal"))]
        let _ = (model, layout, typography, work);
        Ok(Self::default())
    }

    pub(crate) fn orthogonal_route(&self, edge_index: usize) -> Option<&BlockOrthogonalRoute> {
        #[cfg(feature = "layout-shoal")]
        {
            self.orthogonal.get(edge_index)?.as_ref()
        }
        #[cfg(not(feature = "layout-shoal"))]
        {
            let _ = edge_index;
            None
        }
    }
}

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
    edge: &BlockEdge,
    layout_edge: &LayoutEdge,
    shape_geometries_by_id: &HashMap<&str, &BlockShapeGeometry>,
    style: Option<EdgeStyle>,
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
            if style == Some(EdgeStyle::Orthogonal) {
                let vertical = (from.allocated.y - to.allocated.y).abs()
                    > (from.allocated.x - to.allocated.x).abs();
                let bends = if vertical {
                    [
                        LayoutPoint {
                            x: from.allocated.x,
                            y: mid.y,
                        },
                        LayoutPoint {
                            x: to.allocated.x,
                            y: mid.y,
                        },
                    ]
                } else {
                    [
                        LayoutPoint {
                            x: mid.x,
                            y: from.allocated.y,
                        },
                        LayoutPoint {
                            x: mid.x,
                            y: to.allocated.y,
                        },
                    ]
                };
                let from_boundary = from.intersect_native(&bends[0]);
                let to_boundary = to.intersect_native(&bends[1]);
                // When the grid slots touch, the exterior bend can fall inside a block.
                // Keep such intervals ordered on the side of the clipped boundary.
                let bend_axis = |a: f64, b: f64| {
                    let low = a.min(b);
                    let high = a.max(b);
                    if vertical { mid.y } else { mid.x }.clamp(low, high)
                };
                let bends = if vertical {
                    bends.map(|point| LayoutPoint {
                        y: bend_axis(from_boundary.y, to_boundary.y),
                        ..point
                    })
                } else {
                    bends.map(|point| LayoutPoint {
                        x: bend_axis(from_boundary.x, to_boundary.x),
                        ..point
                    })
                };
                let mut points = Vec::from([
                    from_boundary,
                    bends[0].clone(),
                    bends[1].clone(),
                    to_boundary,
                ]);
                points.dedup_by(|a, b| a.x == b.x && a.y == b.y);
                points
            } else if style.is_some() {
                vec![
                    from.intersect_native(&mid),
                    mid.clone(),
                    to.intersect_native(&mid),
                ]
            } else {
                vec![from.intersect(&mid), mid.clone(), to.intersect(&mid)]
            }
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

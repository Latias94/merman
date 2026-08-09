use super::super::super::charset::GraphCharset;
use super::super::super::layout::{CanvasCoord, NodeLayout};
use super::super::super::model::{AsciiGraphEdge, GraphDirection, GraphEdgeMarker};
use super::super::cell::edge_line_char;
use super::super::path::StepDirection;
use super::{
    MarkerAnchors, PlannedRouteCells, PlannedRouteLabel, RoutePlan, edge_line_cell, planned_label,
    route_cell,
};
use crate::error::Result;
use crate::resource::ResourceContext;

#[cfg(test)]
pub(super) fn plan_same_rank_direct_route(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let mut resources = super::unbounded_route_resources();
    super::materialize_test_markers(
        plan_same_rank_direct_route_with_resources(
            layouts,
            from,
            to,
            edge,
            charset,
            &mut resources,
        ),
        edge,
        charset,
    )
}

pub(super) fn plan_same_rank_direct_route_with_resources(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    if from.center_y() != to.center_y() {
        return Ok(None);
    }

    let (start, end, points_right) = if to.x > from.right() + 1 {
        (from.right() + 1, to.x - 1, true)
    } else if from.x > to.right() + 1 {
        (to.right() + 1, from.x - 1, false)
    } else {
        return Ok(None);
    };
    if !direct_route_is_clear(layouts, from, to, start, end) {
        return Ok(None);
    }

    let y = from.center_y();
    let line = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let mut cells = PlannedRouteCells::new();
    if points_right {
        if charset.unicode {
            cells.try_push(resources, || {
                edge_line_cell(from.right(), y, charset.right_connector)
            })?;
        }
        if start == end
            && edge.start_marker != GraphEdgeMarker::Open
            && edge.end_marker != GraphEdgeMarker::Open
        {
            return Ok(None);
        }
        let start_anchor = cells.try_push_anchor(
            resources,
            || route_cell(start, y, line),
            StepDirection::Left,
        )?;
        for x in (start + 1)..end {
            cells.try_push(resources, || route_cell(x, y, line))?;
        }
        let end_anchor = if end == start {
            start_anchor
        } else {
            cells.try_push_anchor(resources, || route_cell(end, y, line), StepDirection::Right)?
        };
        let anchors = MarkerAnchors::new(start_anchor, end_anchor);

        let Some(labels) = planned_direct_labels(edge, start, end, y, points_right, charset) else {
            return Ok(None);
        };
        return Ok(Some(RoutePlan::new(cells.into_vec(), labels, anchors)));
    } else {
        if charset.unicode {
            cells.try_push(resources, || {
                edge_line_cell(from.x, y, charset.left_connector)
            })?;
        }
        if start == end
            && edge.start_marker != GraphEdgeMarker::Open
            && edge.end_marker != GraphEdgeMarker::Open
        {
            return Ok(None);
        }
        let end_anchor = cells.try_push_anchor(
            resources,
            || route_cell(start, y, line),
            StepDirection::Left,
        )?;
        let mut start_anchor = end_anchor;
        for x in (start + 1)..=end {
            if x == end {
                start_anchor = cells.try_push_anchor(
                    resources,
                    || route_cell(x, y, line),
                    StepDirection::Right,
                )?;
            } else {
                cells.try_push(resources, || route_cell(x, y, line))?;
            }
        }
        let anchors = MarkerAnchors::new(start_anchor, end_anchor);
        let Some(labels) = planned_direct_labels(edge, start, end, y, points_right, charset) else {
            return Ok(None);
        };
        return Ok(Some(RoutePlan::new(cells.into_vec(), labels, anchors)));
    }
}

#[cfg(test)]
pub(super) fn plan_same_rank_bottom_lane_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let mut resources = super::unbounded_route_resources();
    super::materialize_test_markers(
        plan_same_rank_bottom_lane_route_with_resources(from, to, edge, charset, &mut resources),
        edge,
        charset,
    )
}

pub(super) fn plan_same_rank_bottom_lane_route_with_resources(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    plan_same_rank_bottom_lane_route_with_index_and_resources(from, to, edge, 0, charset, resources)
}

pub(super) fn plan_same_rank_bottom_lane_route_with_index_and_resources(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    lane_index: usize,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    let start_x = from.center_x();
    let end_x = to.center_x();
    if from.center_y() != to.center_y() || start_x == end_x {
        return Ok(None);
    }

    let lane_offset = resources.checked_grid_mul(lane_index, 2)?;
    let bottom_y = resources.checked_grid_add(
        resources.checked_grid_add(from.bottom().max(to.bottom()), 2)?,
        lane_offset,
    )?;
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let min_x = start_x.min(end_x);
    let max_x = start_x.max(end_x);
    let mut cells = PlannedRouteCells::new();

    let start_anchor = cells.try_push_anchor(
        resources,
        || edge_line_cell(start_x, from.bottom(), charset.down_connector),
        StepDirection::Up,
    )?;
    for y in (from.bottom() + 1)..bottom_y {
        cells.try_push(resources, || route_cell(start_x, y, vertical))?;
    }
    let start_corner = if start_x < end_x {
        charset.corner_down_right
    } else {
        charset.bottom_right
    };
    cells.try_push(resources, || route_cell(start_x, bottom_y, start_corner))?;

    for x in (min_x + 1)..max_x {
        cells.try_push(resources, || route_cell(x, bottom_y, horizontal))?;
    }
    let end_corner = if start_x < end_x {
        charset.bottom_right
    } else {
        charset.corner_down_right
    };
    cells.try_push(resources, || route_cell(end_x, bottom_y, end_corner))?;

    let arrow_y = bottom_y - 1;
    let end_anchor = cells.try_push_anchor(
        resources,
        || edge_line_cell(end_x, arrow_y, vertical),
        StepDirection::Up,
    )?;
    let labels = planned_label(
        edge.label.as_deref(),
        CanvasCoord {
            x: min_x,
            y: bottom_y,
        },
        CanvasCoord {
            x: max_x,
            y: bottom_y,
        },
        charset,
    )
    .into_iter()
    .collect();
    let minimum_width = resources.checked_grid_add(max_x, 3)?;
    let minimum_height = resources.checked_grid_add(bottom_y, 1)?;

    Ok(Some(RoutePlan::with_min_canvas_extent(
        cells.into_vec(),
        labels,
        MarkerAnchors::new(start_anchor, end_anchor),
        minimum_width,
        minimum_height,
    )))
}

fn direct_route_is_clear(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    start: usize,
    end: usize,
) -> bool {
    let y = from.center_y();
    layouts
        .iter()
        .filter(|layout| layout.id != from.id && layout.id != to.id)
        .all(|layout| {
            y < layout.y || y > layout.bottom() || end < layout.x || start > layout.right()
        })
}

fn planned_direct_labels(
    edge: &AsciiGraphEdge,
    start: usize,
    end: usize,
    y: usize,
    points_right: bool,
    charset: &GraphCharset,
) -> Option<Vec<PlannedRouteLabel>> {
    let Some(mut label) = planned_label(
        edge.label.as_deref(),
        CanvasCoord { x: start, y },
        CanvasCoord { x: end, y },
        charset,
    ) else {
        return Some(Vec::new());
    };
    if edge.start_marker == GraphEdgeMarker::Open && edge.end_marker == GraphEdgeMarker::Open {
        return Some(vec![label]);
    }

    let left_marker = if points_right {
        edge.start_marker
    } else {
        edge.end_marker
    };
    let right_marker = if points_right {
        edge.end_marker
    } else {
        edge.start_marker
    };
    let available_start = start + usize::from(left_marker != GraphEdgeMarker::Open);
    let available_end = end.checked_sub(usize::from(right_marker != GraphEdgeMarker::Open))?;
    let label_width = label.text.width();
    let available_width = available_end.checked_sub(available_start)? + 1;
    if label_width > available_width {
        return None;
    }

    let max_label_x = available_end.checked_add(1)?.checked_sub(label_width)?;
    let x = label.placement.x().clamp(available_start, max_label_x);
    label.placement = label.placement.with_position(x, label.placement.y());
    Some(vec![label])
}

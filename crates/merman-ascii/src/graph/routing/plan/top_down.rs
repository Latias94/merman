use super::super::super::charset::GraphCharset;
use super::super::super::layout::{CanvasCoord, NodeLayout};
use super::super::super::model::{AsciiGraphEdge, GraphDirection, GraphEdgeMarker};
use super::super::super::shape::GraphNodeShapeSemantics;
use super::super::cell::edge_line_char;
use super::super::label::{
    RoutedLabelText, routed_label_right_of_vertical_route_placement_for_text,
};
use super::super::path::StepDirection;
use super::{
    MarkerAnchors, PlannedRouteCells, PlannedRouteLabel, RoutePlan, edge_line_cell, planned_label,
    route_cell,
};
use crate::error::Result;
use crate::resource::ResourceContext;

#[cfg(test)]
pub(super) fn plan_top_down_direct_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let mut resources = super::unbounded_route_resources();
    super::materialize_test_markers(
        plan_top_down_direct_route_with_resources(from, to, edge, charset, &mut resources),
        edge,
        charset,
    )
}

pub(super) fn plan_top_down_direct_route_with_resources(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    if to.y <= from.bottom() + 1 {
        return Ok(None);
    }

    let x = from.center_x();
    let start = from.bottom() + 1;
    let end = to.y - 1;
    let line = edge_line_char(edge, charset, GraphDirection::TopDown);
    let mut cells = PlannedRouteCells::new();
    let start_anchor = cells.try_push_anchor(
        resources,
        || edge_line_cell(x, from.bottom(), charset.down_connector),
        StepDirection::Up,
    )?;
    for y in start..end {
        cells.try_push(resources, || route_cell(x, y, line))?;
    }
    let end_anchor =
        cells.try_push_anchor(resources, || route_cell(x, end, line), StepDirection::Down)?;

    let labels = planned_label(
        edge.label.as_deref(),
        CanvasCoord { x, y: start },
        CanvasCoord { x, y: end },
        charset,
    )
    .into_iter()
    .collect();

    Ok(Some(RoutePlan::new(
        cells.into_vec(),
        labels,
        MarkerAnchors::new(start_anchor, end_anchor),
    )))
}

#[cfg(test)]
pub(super) fn plan_top_down_bent_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let mut resources = super::unbounded_route_resources();
    super::materialize_test_markers(
        plan_top_down_bent_route_with_resources(from, to, edge, charset, &mut resources),
        edge,
        charset,
    )
}

pub(super) fn plan_top_down_bent_route_with_resources(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    if GraphNodeShapeSemantics::new(from.shape).uses_drop_then_turn_bent_route()
        || GraphNodeShapeSemantics::new(to.shape).uses_drop_then_turn_bent_route()
    {
        return plan_top_down_drop_then_turn_route(from, to, edge, charset, resources);
    }

    plan_top_down_side_bend_route(from, to, edge, charset, resources)
}

fn plan_top_down_side_bend_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    let turn_y = from.center_y();
    let Some(end_y) = to.y.checked_sub(1) else {
        return Ok(None);
    };
    if end_y <= turn_y {
        return Ok(None);
    }

    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let target_x = to.center_x();
    let mut cells = PlannedRouteCells::new();
    let (label_start_x, label_end_x, start_anchor);

    if target_x > from.center_x() {
        if target_x <= from.right() {
            return Ok(None);
        }

        label_start_x = from.right();
        label_end_x = target_x;
        start_anchor = cells.try_push_anchor(
            resources,
            || edge_line_cell(from.right(), turn_y, charset.right_connector),
            StepDirection::Left,
        )?;
        for x in (from.right() + 1)..target_x {
            cells.try_push(resources, || route_cell(x, turn_y, horizontal))?;
        }
        cells.try_push(resources, || {
            route_cell(target_x, turn_y, charset.top_right)
        })?;
    } else {
        if from.x <= target_x {
            return Ok(None);
        }

        label_start_x = target_x;
        label_end_x = from.x;
        start_anchor = cells.try_push_anchor(
            resources,
            || edge_line_cell(from.x, turn_y, charset.left_connector),
            StepDirection::Right,
        )?;
        for x in ((target_x + 1)..from.x).rev() {
            cells.try_push(resources, || route_cell(x, turn_y, horizontal))?;
        }
        cells.try_push(resources, || route_cell(target_x, turn_y, charset.top_left))?;
    }

    for y in (turn_y + 1)..end_y {
        cells.try_push(resources, || route_cell(target_x, y, vertical))?;
    }
    let end_anchor = cells.try_push_anchor(
        resources,
        || route_cell(target_x, end_y, vertical),
        StepDirection::Down,
    )?;

    let labels = planned_label(
        edge.label.as_deref(),
        CanvasCoord {
            x: label_start_x,
            y: turn_y,
        },
        CanvasCoord {
            x: label_end_x,
            y: turn_y,
        },
        charset,
    )
    .into_iter()
    .collect();

    Ok(Some(RoutePlan::new(
        cells.into_vec(),
        labels,
        MarkerAnchors::new(start_anchor, end_anchor),
    )))
}

fn plan_top_down_drop_then_turn_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    let Some(end_y) = to.y.checked_sub(1) else {
        return Ok(None);
    };
    if end_y <= from.bottom() {
        return Ok(None);
    }

    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let source_x = from.center_x();
    let target_x = to.center_x();
    let mut cells = PlannedRouteCells::new();

    let start_anchor = cells.try_push_anchor(
        resources,
        || edge_line_cell(source_x, from.bottom(), charset.down_connector),
        StepDirection::Up,
    )?;
    for y in (from.bottom() + 1)..end_y {
        cells.try_push(resources, || route_cell(source_x, y, vertical))?;
    }

    if target_x > source_x {
        cells.try_push(resources, || {
            route_cell(source_x, end_y, charset.corner_down_right)
        })?;
        for x in (source_x + 1)..target_x {
            cells.try_push(resources, || route_cell(x, end_y, horizontal))?;
        }
    } else {
        cells.try_push(resources, || {
            route_cell(source_x, end_y, charset.corner_right_up)
        })?;
        for x in ((target_x + 1)..source_x).rev() {
            cells.try_push(resources, || route_cell(x, end_y, horizontal))?;
        }
    }

    let end_anchor = cells.try_push_anchor(
        resources,
        || route_cell(target_x, end_y, horizontal),
        StepDirection::Down,
    )?;

    let labels = planned_label(
        edge.label.as_deref(),
        CanvasCoord {
            x: source_x.min(target_x),
            y: end_y,
        },
        CanvasCoord {
            x: source_x.max(target_x),
            y: end_y,
        },
        charset,
    )
    .into_iter()
    .collect();

    Ok(Some(RoutePlan::new(
        cells.into_vec(),
        labels,
        MarkerAnchors::new(start_anchor, end_anchor),
    )))
}

#[cfg(test)]
pub(super) fn plan_top_down_side_entry_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let mut resources = super::unbounded_route_resources();
    super::materialize_test_markers(
        plan_top_down_side_entry_route_with_resources(from, to, edge, charset, &mut resources),
        edge,
        charset,
    )
}

pub(super) fn plan_top_down_side_entry_route_with_resources(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    let y = from.center_y();
    if y < to.y || y > to.bottom() {
        return Ok(None);
    }

    let line = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let mut cells = PlannedRouteCells::new();

    if from.center_x() < to.center_x() {
        if to.x <= from.right() + 1 {
            return Ok(None);
        }
        if charset.unicode {
            cells.try_push(resources, || {
                edge_line_cell(from.right(), y, charset.right_connector)
            })?;
        }
        let start = from.right() + 1;
        let end = to.x - 1;
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
        for x in start..end {
            if x != start {
                cells.try_push(resources, || route_cell(x, y, line))?;
            }
        }
        let end_anchor = if end == start {
            start_anchor
        } else {
            cells.try_push_anchor(resources, || route_cell(end, y, line), StepDirection::Right)?
        };

        let labels = planned_label(
            edge.label.as_deref(),
            CanvasCoord { x: start, y },
            CanvasCoord { x: end, y },
            charset,
        )
        .into_iter()
        .collect();

        return Ok(Some(RoutePlan::new(
            cells.into_vec(),
            labels,
            MarkerAnchors::new(start_anchor, end_anchor),
        )));
    }

    if from.x <= to.right() + 1 {
        return Ok(None);
    }
    if charset.unicode {
        cells.try_push(resources, || {
            edge_line_cell(from.x, y, charset.left_connector)
        })?;
    }
    let start = to.right() + 1;
    let end = from.x - 1;
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
    for x in (start + 1)..from.x {
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

    let labels = planned_label(
        edge.label.as_deref(),
        CanvasCoord { x: start, y },
        CanvasCoord { x: end, y },
        charset,
    )
    .into_iter()
    .collect();

    Ok(Some(RoutePlan::new(
        cells.into_vec(),
        labels,
        MarkerAnchors::new(start_anchor, end_anchor),
    )))
}

#[cfg(test)]
pub(super) fn plan_top_down_back_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let mut resources = super::unbounded_route_resources();
    super::materialize_test_markers(
        plan_top_down_back_route_with_resources(from, to, edge, charset, &mut resources),
        edge,
        charset,
    )
}

pub(super) fn plan_top_down_back_route_with_resources(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    let lane_x = top_down_back_edge_lane_x(from, to);
    let source_y = from.center_y();
    let target_y = to.center_y();
    if source_y <= target_y || lane_x <= from.right() {
        return Ok(None);
    }

    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let mut cells = PlannedRouteCells::new();
    let start_anchor = cells.try_push_anchor(
        resources,
        || edge_line_cell(from.right(), source_y, charset.right_connector),
        StepDirection::Left,
    )?;

    for x in (from.right() + 1)..lane_x {
        cells.try_push(resources, || route_cell(x, source_y, horizontal))?;
    }
    cells.try_push(resources, || {
        route_cell(lane_x, source_y, charset.corner_right_up)
    })?;

    for y in (target_y + 1)..source_y {
        cells.try_push(resources, || route_cell(lane_x, y, vertical))?;
    }
    cells.try_push(resources, || {
        route_cell(lane_x, target_y, charset.top_right)
    })?;

    let end_anchor = cells.try_push_anchor(
        resources,
        || route_cell(to.right() + 1, target_y, horizontal),
        StepDirection::Left,
    )?;
    for x in (to.right() + 2)..lane_x {
        cells.try_push(resources, || route_cell(x, target_y, horizontal))?;
    }
    let labels: Vec<_> =
        planned_top_down_back_label(edge.label.as_deref(), lane_x, target_y, source_y, charset)
            .into_iter()
            .collect();

    let min_width = labels.iter().fold(lane_x + 3, |width, label| {
        width.max(
            label
                .placement
                .canvas_extent_for_lines(label.text.line_count())
                .0
                + 1,
        )
    });

    Ok(Some(RoutePlan::with_min_canvas_extent(
        cells.into_vec(),
        labels,
        MarkerAnchors::new(start_anchor, end_anchor),
        min_width,
        0,
    )))
}

pub(super) fn top_down_back_edge_lane_x(from: &NodeLayout, to: &NodeLayout) -> usize {
    from.right().max(to.right()) + 4
}

fn planned_top_down_back_label(
    label: Option<&str>,
    lane_x: usize,
    target_y: usize,
    source_y: usize,
    charset: &GraphCharset,
) -> Option<PlannedRouteLabel> {
    let text = RoutedLabelText::new_with_profile(label?, charset.width_profile)?;
    let placement = routed_label_right_of_vertical_route_placement_for_text(
        CanvasCoord {
            x: lane_x,
            y: target_y,
        },
        CanvasCoord {
            x: lane_x,
            y: source_y,
        },
        &text,
    )?;

    Some(PlannedRouteLabel::new(text, placement))
}

use super::super::super::charset::GraphCharset;
use super::super::super::layout::{CanvasCoord, NodeLayout};
use super::super::super::model::{AsciiGraphEdge, GraphDirection};
use super::super::path::StepDirection;
use super::{
    MarkerAnchor, MarkerAnchors, PlannedRouteCells, PlannedRouteSegment, RoutePlan, planned_label,
    route_cell_in_segment, route_turn_char,
};
use crate::error::Result;
use crate::resource::ResourceContext;

pub(super) fn plan_axis_aligned_compound_endpoint_route_with_resources(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    if from.id == to.id {
        return Ok(None);
    }

    if let Some(x) = overlapping_interior_axis(from.x, from.width, to.x, to.width, resources)? {
        // Adjacent containers share one legal endpoint berth on their common border.
        if from.bottom() == to.y {
            let port = CanvasCoord { x, y: to.y };
            return plan_axis_aligned_route(
                port,
                port,
                StepDirection::Down,
                edge,
                charset,
                resources,
            );
        }
        if to.bottom() == from.y {
            let port = CanvasCoord { x, y: from.y };
            return plan_axis_aligned_route(
                port,
                port,
                StepDirection::Up,
                edge,
                charset,
                resources,
            );
        }
        if from.bottom() < to.y {
            let start = CanvasCoord {
                x,
                y: resources.checked_grid_add(from.bottom(), 1)?,
            };
            let end = CanvasCoord {
                x,
                y: to
                    .y
                    .checked_sub(1)
                    .ok_or_else(|| resources.work_overflow())?,
            };
            return plan_axis_aligned_route(
                start,
                end,
                StepDirection::Down,
                edge,
                charset,
                resources,
            );
        }
        if to.bottom() < from.y {
            let start = CanvasCoord {
                x,
                y: from
                    .y
                    .checked_sub(1)
                    .ok_or_else(|| resources.work_overflow())?,
            };
            let end = CanvasCoord {
                x,
                y: resources.checked_grid_add(to.bottom(), 1)?,
            };
            return plan_axis_aligned_route(
                start,
                end,
                StepDirection::Up,
                edge,
                charset,
                resources,
            );
        }
    }

    if let Some(y) = overlapping_interior_axis(from.y, from.height, to.y, to.height, resources)? {
        if from.right() == to.x {
            let port = CanvasCoord { x: to.x, y };
            return plan_axis_aligned_route(
                port,
                port,
                StepDirection::Right,
                edge,
                charset,
                resources,
            );
        }
        if to.right() == from.x {
            let port = CanvasCoord { x: from.x, y };
            return plan_axis_aligned_route(
                port,
                port,
                StepDirection::Left,
                edge,
                charset,
                resources,
            );
        }
        if from.right() < to.x {
            let start = CanvasCoord {
                x: resources.checked_grid_add(from.right(), 1)?,
                y,
            };
            let end = CanvasCoord {
                x: to
                    .x
                    .checked_sub(1)
                    .ok_or_else(|| resources.work_overflow())?,
                y,
            };
            return plan_axis_aligned_route(
                start,
                end,
                StepDirection::Right,
                edge,
                charset,
                resources,
            );
        }
        if to.right() < from.x {
            let start = CanvasCoord {
                x: from
                    .x
                    .checked_sub(1)
                    .ok_or_else(|| resources.work_overflow())?,
                y,
            };
            let end = CanvasCoord {
                x: resources.checked_grid_add(to.right(), 1)?,
                y,
            };
            return plan_axis_aligned_route(
                start,
                end,
                StepDirection::Left,
                edge,
                charset,
                resources,
            );
        }
    }

    Ok(None)
}

fn overlapping_interior_axis(
    first_start: usize,
    first_size: usize,
    second_start: usize,
    second_size: usize,
    resources: &ResourceContext,
) -> Result<Option<usize>> {
    let first_end = resources
        .checked_grid_add(first_start, first_size)?
        .checked_sub(1)
        .ok_or_else(|| resources.work_overflow())?;
    let second_end = resources
        .checked_grid_add(second_start, second_size)?
        .checked_sub(1)
        .ok_or_else(|| resources.work_overflow())?;
    let first_inset = usize::from(first_size > 2);
    let second_inset = usize::from(second_size > 2);
    let start = resources
        .checked_grid_add(first_start, first_inset)?
        .max(resources.checked_grid_add(second_start, second_inset)?);
    let end = first_end
        .checked_sub(first_inset)
        .ok_or_else(|| resources.work_overflow())?
        .min(
            second_end
                .checked_sub(second_inset)
                .ok_or_else(|| resources.work_overflow())?,
        );
    if start > end {
        return Ok(None);
    }
    Ok(Some(start + (end - start) / 2))
}

fn plan_axis_aligned_route(
    start: CanvasCoord,
    end: CanvasCoord,
    direction: StepDirection,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    let is_ordered = match direction {
        StepDirection::Right => start.y == end.y && start.x <= end.x,
        StepDirection::Left => start.y == end.y && start.x >= end.x,
        StepDirection::Down => start.x == end.x && start.y <= end.y,
        StepDirection::Up => start.x == end.x && start.y >= end.y,
    };
    if !is_ordered {
        return Ok(None);
    }
    if start == end
        && edge.start_marker != super::super::super::model::GraphEdgeMarker::Open
        && edge.end_marker != super::super::super::model::GraphEdgeMarker::Open
    {
        return Ok(None);
    }

    let orientation = match direction {
        StepDirection::Left | StepDirection::Right => GraphDirection::LeftRight,
        StepDirection::Up | StepDirection::Down => GraphDirection::TopDown,
    };
    let line = super::super::cell::edge_line_char(edge, charset, orientation);
    let mut cells = PlannedRouteCells::new();
    let start_cell = cells.try_push(resources, || boundary_cell(start.x, start.y, line))?;
    let start_anchor = MarkerAnchor::new(start_cell, opposite_direction(direction));
    let mut end_anchor = MarkerAnchor::new(start_cell, direction);
    match direction {
        StepDirection::Right => {
            for x in resources.checked_grid_add(start.x, 1)?..=end.x {
                let cell = cells.try_push(resources, || boundary_cell(x, start.y, line))?;
                if x == end.x {
                    end_anchor = MarkerAnchor::new(cell, direction);
                }
            }
        }
        StepDirection::Left => {
            for x in (end.x..start.x).rev() {
                let cell = cells.try_push(resources, || boundary_cell(x, start.y, line))?;
                if x == end.x {
                    end_anchor = MarkerAnchor::new(cell, direction);
                }
            }
        }
        StepDirection::Down => {
            for y in resources.checked_grid_add(start.y, 1)?..=end.y {
                let cell = cells.try_push(resources, || boundary_cell(start.x, y, line))?;
                if y == end.y {
                    end_anchor = MarkerAnchor::new(cell, direction);
                }
            }
        }
        StepDirection::Up => {
            for y in (end.y..start.y).rev() {
                let cell = cells.try_push(resources, || boundary_cell(start.x, y, line))?;
                if y == end.y {
                    end_anchor = MarkerAnchor::new(cell, direction);
                }
            }
        }
    }
    let labels = planned_label(edge.label.as_deref(), start, end, charset)
        .into_iter()
        .collect();
    Ok(Some(RoutePlan::new(
        cells.into_vec(),
        labels,
        MarkerAnchors::new(start_anchor, end_anchor),
    )))
}

fn opposite_direction(direction: StepDirection) -> StepDirection {
    match direction {
        StepDirection::Up => StepDirection::Down,
        StepDirection::Down => StepDirection::Up,
        StepDirection::Left => StepDirection::Right,
        StepDirection::Right => StepDirection::Left,
    }
}

pub(super) fn plan_compound_endpoint_route_with_resources(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    parallel_index: usize,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    if from.id == to.id {
        return Ok(None);
    }

    if from.center_y() == to.center_y() {
        plan_bottom_lane(from, to, edge, parallel_index, charset, resources)
    } else {
        plan_right_lane(from, to, edge, parallel_index, charset, resources)
    }
}

fn plan_right_lane(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    parallel_index: usize,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    let source_y = from.center_y();
    let target_y = to.center_y();
    let source_x = resources.checked_grid_add(from.right(), 1)?;
    let target_x = resources.checked_grid_add(to.right(), 1)?;
    let lane_offset = resources.checked_grid_mul(parallel_index, 2)?;
    let lane_x = resources.checked_grid_add(
        resources.checked_grid_add(from.right().max(to.right()), 2)?,
        lane_offset,
    )?;
    if lane_x < source_x || lane_x < target_x {
        return Ok(None);
    }

    let horizontal = super::super::cell::edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = super::super::cell::edge_line_char(edge, charset, GraphDirection::TopDown);
    let vertical_direction = if source_y < target_y {
        StepDirection::Down
    } else {
        StepDirection::Up
    };
    let mut cells = PlannedRouteCells::new();
    let start_anchor = cells.try_push_anchor(
        resources,
        || boundary_cell(source_x, source_y, horizontal),
        StepDirection::Left,
    )?;
    for x in (source_x + 1)..lane_x {
        cells.try_push(resources, || boundary_cell(x, source_y, horizontal))?;
    }
    cells.try_push(resources, || {
        boundary_cell(
            lane_x,
            source_y,
            route_turn_char(StepDirection::Right, vertical_direction, charset),
        )
    })?;

    match vertical_direction {
        StepDirection::Down => {
            for y in (source_y + 1)..target_y {
                cells.try_push(resources, || boundary_cell(lane_x, y, vertical))?;
            }
        }
        StepDirection::Up => {
            for y in ((target_y + 1)..source_y).rev() {
                cells.try_push(resources, || boundary_cell(lane_x, y, vertical))?;
            }
        }
        StepDirection::Left | StepDirection::Right => unreachable!(),
    }
    cells.try_push(resources, || {
        boundary_cell(
            lane_x,
            target_y,
            route_turn_char(vertical_direction, StepDirection::Left, charset),
        )
    })?;
    for x in ((target_x + 1)..lane_x).rev() {
        cells.try_push(resources, || boundary_cell(x, target_y, horizontal))?;
    }
    let end_anchor = cells.try_push_anchor(
        resources,
        || boundary_cell(target_x, target_y, horizontal),
        StepDirection::Left,
    )?;
    let labels = planned_label(
        edge.label.as_deref(),
        CanvasCoord {
            x: lane_x,
            y: source_y,
        },
        CanvasCoord {
            x: lane_x,
            y: target_y,
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

fn plan_bottom_lane(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    parallel_index: usize,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    let source_x = from.center_x();
    let target_x = to.center_x();
    if source_x == target_x {
        return Ok(None);
    }
    let source_y = resources.checked_grid_add(from.bottom(), 1)?;
    let target_y = resources.checked_grid_add(to.bottom(), 1)?;
    let lane_offset = resources.checked_grid_mul(parallel_index, 2)?;
    let lane_y = resources.checked_grid_add(
        resources.checked_grid_add(from.bottom().max(to.bottom()), 2)?,
        lane_offset,
    )?;
    let horizontal = super::super::cell::edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = super::super::cell::edge_line_char(edge, charset, GraphDirection::TopDown);
    let horizontal_direction = if source_x < target_x {
        StepDirection::Right
    } else {
        StepDirection::Left
    };
    let mut cells = PlannedRouteCells::new();
    let start_anchor = cells.try_push_anchor(
        resources,
        || boundary_cell(source_x, source_y, vertical),
        StepDirection::Up,
    )?;
    for y in (source_y + 1)..lane_y {
        cells.try_push(resources, || boundary_cell(source_x, y, vertical))?;
    }
    cells.try_push(resources, || {
        boundary_cell(
            source_x,
            lane_y,
            route_turn_char(StepDirection::Down, horizontal_direction, charset),
        )
    })?;
    match horizontal_direction {
        StepDirection::Right => {
            for x in (source_x + 1)..target_x {
                cells.try_push(resources, || boundary_cell(x, lane_y, horizontal))?;
            }
        }
        StepDirection::Left => {
            for x in ((target_x + 1)..source_x).rev() {
                cells.try_push(resources, || boundary_cell(x, lane_y, horizontal))?;
            }
        }
        StepDirection::Up | StepDirection::Down => unreachable!(),
    }
    cells.try_push(resources, || {
        boundary_cell(
            target_x,
            lane_y,
            route_turn_char(horizontal_direction, StepDirection::Up, charset),
        )
    })?;
    for y in ((target_y + 1)..lane_y).rev() {
        cells.try_push(resources, || boundary_cell(target_x, y, vertical))?;
    }
    let end_anchor = cells.try_push_anchor(
        resources,
        || boundary_cell(target_x, target_y, vertical),
        StepDirection::Up,
    )?;
    let labels = planned_label(
        edge.label.as_deref(),
        CanvasCoord {
            x: source_x,
            y: lane_y,
        },
        CanvasCoord {
            x: target_x,
            y: lane_y,
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

fn boundary_cell(x: usize, y: usize, ch: char) -> super::PlannedRouteCell {
    route_cell_in_segment(x, y, ch, PlannedRouteSegment::Boundary)
}

use super::super::super::charset::GraphCharset;
use super::super::super::layout::{CanvasCoord, NodeLayout};
use super::super::super::model::{AsciiGraphEdge, GraphDirection};
use super::super::super::shape::GraphNodeShapeSemantics;
use super::super::cell::edge_line_char;
use super::super::path::StepDirection;
use super::{
    MarkerAnchors, PlannedRouteCells, RoutePlan, edge_line_cell, planned_label, route_cell,
};
use crate::error::Result;
use crate::graph::routing::label::RoutedLabelDescriptor;
use crate::resource::ResourceContext;

#[cfg(test)]
fn test_label(edge: &AsciiGraphEdge, charset: &GraphCharset) -> Option<RoutedLabelDescriptor> {
    edge.label
        .as_deref()
        .and_then(|raw| RoutedLabelDescriptor::for_test(0, raw, charset.width_profile))
}

#[cfg(test)]
pub(super) fn plan_left_right_down_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let mut resources = super::unbounded_route_resources();
    super::materialize_test_markers(
        plan_left_right_down_route_with_resources(
            from,
            to,
            edge,
            test_label(edge, charset),
            charset,
            &mut resources,
        ),
        edge,
        charset,
    )
}

pub(super) fn plan_left_right_down_route_with_resources(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    _label: Option<RoutedLabelDescriptor>,
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

    Ok(Some(RoutePlan::new(
        cells.into_vec(),
        Vec::new(),
        MarkerAnchors::new(start_anchor, end_anchor),
    )))
}

#[cfg(test)]
pub(super) fn plan_left_right_down_then_right_route(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let mut resources = super::unbounded_route_resources();
    super::materialize_test_markers(
        plan_left_right_down_then_right_route_with_resources(
            layouts,
            edges,
            from,
            to,
            edge,
            test_label(edge, charset),
            charset,
            &mut resources,
        ),
        edge,
        charset,
    )
}

// Keep the route geometry, label descriptor, charset, and resource ledger explicit at this
// internal planning seam; bundling them would obscure which inputs affect candidate geometry.
#[allow(clippy::too_many_arguments)]
pub(super) fn plan_left_right_down_then_right_route_with_resources(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    _label: Option<RoutedLabelDescriptor>,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    if !has_left_right_crossing_pair(layouts, edges, from, to) {
        return plan_left_right_basic_down_then_right_route(from, to, edge, charset, resources);
    }

    let source_x = from.center_x();
    let lane_x = lane_x_between(from, to);
    let lane_y = lane_y_between(from, to);
    if lane_y <= from.bottom() || to.x <= lane_x + 1 {
        return Ok(None);
    }

    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let mut cells = PlannedRouteCells::new();
    let start_anchor = cells.try_push_anchor(
        resources,
        || edge_line_cell(source_x, from.bottom(), charset.down_connector),
        StepDirection::Up,
    )?;
    for y in (from.bottom() + 1)..lane_y {
        cells.try_push(resources, || route_cell(source_x, y, vertical))?;
    }
    cells.try_push(resources, || {
        route_cell(source_x, lane_y, charset.corner_down_right)
    })?;

    for line_x in (source_x + 1)..lane_x {
        cells.try_push(resources, || route_cell(line_x, lane_y, horizontal))?;
    }
    cells.try_push(resources, || route_cell(lane_x, lane_y, charset.top_right))?;

    for y in (lane_y + 1)..to.center_y() {
        cells.try_push(resources, || route_cell(lane_x, y, vertical))?;
    }
    let end = to.x - 1;
    cells.try_push(resources, || {
        route_cell(lane_x, to.center_y(), charset.corner_down_right)
    })?;
    for line_x in (lane_x + 1)..end {
        cells.try_push(resources, || route_cell(line_x, to.center_y(), horizontal))?;
    }
    let end_anchor = cells.try_push_anchor(
        resources,
        || route_cell(end, to.center_y(), horizontal),
        StepDirection::Right,
    )?;

    Ok(Some(RoutePlan::new(
        cells.into_vec(),
        Vec::new(),
        MarkerAnchors::new(start_anchor, end_anchor),
    )))
}

#[cfg(test)]
pub(super) fn plan_left_right_right_then_up_route(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let mut resources = super::unbounded_route_resources();
    super::materialize_test_markers(
        plan_left_right_right_then_up_route_with_resources(
            layouts,
            edges,
            from,
            to,
            edge,
            test_label(edge, charset),
            charset,
            &mut resources,
        ),
        edge,
        charset,
    )
}

// Keep the route geometry, label descriptor, charset, and resource ledger explicit at this
// internal planning seam; bundling them would obscure which inputs affect candidate geometry.
#[allow(clippy::too_many_arguments)]
pub(super) fn plan_left_right_right_then_up_route_with_resources(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    _label: Option<RoutedLabelDescriptor>,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    if !has_left_right_reverse_crossing_pair(layouts, edges, from, to) {
        return plan_left_right_basic_right_then_up_route(from, to, edge, charset, resources);
    }

    let source_x = from.center_x();
    let lane_x = lane_x_between(from, to);
    let lane_y = lane_y_between(to, from);
    if lane_x <= source_x || from.y <= lane_y || lane_y <= to.bottom() {
        return Ok(None);
    }

    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let mut cells = PlannedRouteCells::new();
    let start_anchor = cells.try_push_anchor(
        resources,
        || edge_line_cell(source_x, from.y, charset.up_connector),
        StepDirection::Down,
    )?;
    for y in (lane_y + 1)..from.y {
        cells.try_push(resources, || route_cell(source_x, y, vertical))?;
    }
    cells.try_push(resources, || route_cell(source_x, lane_y, charset.top_left))?;

    for x in (source_x + 1)..lane_x {
        cells.try_push(resources, || route_cell(x, lane_y, horizontal))?;
    }
    cells.try_push(resources, || {
        route_cell(lane_x, lane_y, charset.corner_right_up)
    })?;

    for y in (to.center_y() + 1)..lane_y {
        cells.try_push(resources, || route_cell(lane_x, y, vertical))?;
    }
    cells.try_push(resources, || {
        route_cell(lane_x, to.center_y(), charset.top_left)
    })?;

    let end = to.x - 1;
    for x in (lane_x + 1)..end {
        cells.try_push(resources, || route_cell(x, to.center_y(), horizontal))?;
    }
    let end_anchor = cells.try_push_anchor(
        resources,
        || route_cell(end, to.center_y(), horizontal),
        StepDirection::Right,
    )?;

    Ok(Some(RoutePlan::new(
        cells.into_vec(),
        Vec::new(),
        MarkerAnchors::new(start_anchor, end_anchor),
    )))
}

#[cfg(test)]
pub(super) fn plan_left_right_reverse_over_self_loop_route(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let mut resources = super::unbounded_route_resources();
    super::materialize_test_markers(
        plan_left_right_reverse_over_self_loop_route_with_resources(
            layouts,
            from,
            to,
            edge,
            test_label(edge, charset),
            charset,
            &mut resources,
        ),
        edge,
        charset,
    )
}

pub(super) fn plan_left_right_reverse_over_self_loop_route_with_resources(
    layouts: &[NodeLayout],
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    label: Option<RoutedLabelDescriptor>,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    let lane_x = self_loop_right_x(layouts, to);
    if lane_x <= to.right() || from.x <= lane_x {
        return Ok(None);
    }

    let y = to.center_y();
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let mut cells = PlannedRouteCells::new();
    let start_anchor = cells.try_push_anchor(
        resources,
        || edge_line_cell(from.x, y, charset.left_connector),
        StepDirection::Right,
    )?;
    cells.try_push(resources, || route_cell(lane_x, y, charset.down_junction))?;
    for x in (lane_x + 1)..from.x {
        cells.try_push(resources, || route_cell(x, y, horizontal))?;
    }
    let end_anchor = cells.try_push_anchor(
        resources,
        || route_cell(to.right() + 1, y, horizontal),
        StepDirection::Left,
    )?;
    for x in (to.right() + 2)..lane_x {
        cells.try_push(resources, || route_cell(x, y, horizontal))?;
    }
    let labels = planned_label(
        label,
        CanvasCoord {
            x: to.right() + 1,
            y,
        },
        CanvasCoord {
            x: from.x.saturating_sub(1),
            y,
        },
    )
    .into_iter()
    .collect();

    Ok(Some(RoutePlan::with_min_canvas_extent(
        cells.into_vec(),
        labels,
        MarkerAnchors::new(start_anchor, end_anchor),
        from.center_x().max(to.center_x()) + 3,
        0,
    )))
}

#[cfg(test)]
pub(super) fn plan_left_right_self_loop_route(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    from: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let mut resources = super::unbounded_route_resources();
    super::materialize_test_markers(
        plan_left_right_self_loop_route_with_resources(
            layouts,
            edges,
            from,
            edge,
            0,
            test_label(edge, charset),
            charset,
            &mut resources,
        ),
        edge,
        charset,
    )
}

// Keep the route geometry, label descriptor, charset, and resource ledger explicit at this
// internal planning seam; bundling them would obscure which inputs affect candidate geometry.
#[allow(clippy::too_many_arguments)]
pub(super) fn plan_left_right_self_loop_route_with_resources(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    from: &NodeLayout,
    edge: &AsciiGraphEdge,
    parallel_index: usize,
    label: Option<RoutedLabelDescriptor>,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    let y = from.center_y();
    let lane_offset = resources.checked_grid_mul(parallel_index, 2)?;
    let loop_x = resources.checked_grid_add(self_loop_right_x(layouts, from), lane_offset)?;
    let bottom_y = resources.checked_grid_add(
        self_loop_bottom_y_for_edges(layouts, edges, from),
        lane_offset,
    )?;
    let marker_x = resources.checked_grid_add(from.center_x(), parallel_index)?;
    let minimum_bottom_y = resources.checked_grid_add(y, 1)?;
    if loop_x <= from.right() || marker_x >= loop_x || bottom_y <= minimum_bottom_y {
        return Ok(None);
    }

    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let mut cells = PlannedRouteCells::new();
    let mut start_anchor =
        if GraphNodeShapeSemantics::new(from.shape).uses_external_self_loop_connector() {
            Some(cells.try_push_anchor(
                resources,
                || edge_line_cell(from.right(), y, charset.right_connector),
                StepDirection::Left,
            )?)
        } else {
            None
        };
    let first_loop_x = resources.checked_grid_add(from.right(), 1)?;
    for x in first_loop_x..loop_x {
        cells.try_push(resources, || route_cell(x, y, horizontal))?;
    }
    let top_corner = if self_loop_has_right_neighbor(layouts, from) {
        charset.down_junction
    } else {
        charset.top_right
    };
    let top_corner_anchor = if start_anchor.is_none() {
        Some(cells.try_push_anchor(
            resources,
            || route_cell(loop_x, y, top_corner),
            StepDirection::Left,
        )?)
    } else {
        cells.try_push(resources, || route_cell(loop_x, y, top_corner))?;
        None
    };
    start_anchor = start_anchor.or(top_corner_anchor);

    let first_loop_y = resources.checked_grid_add(y, 1)?;
    for line_y in first_loop_y..bottom_y {
        cells.try_push(resources, || route_cell(loop_x, line_y, vertical))?;
    }
    cells.try_push(resources, || {
        route_cell(loop_x, bottom_y, charset.bottom_right)
    })?;

    let first_bottom_x = resources.checked_grid_add(marker_x, 1)?;
    for x in first_bottom_x..loop_x {
        cells.try_push(resources, || route_cell(x, bottom_y, horizontal))?;
    }
    cells.try_push(resources, || {
        route_cell(marker_x, bottom_y, charset.corner_down_right)
    })?;

    let arrow_y = resources.checked_grid_add(from.bottom(), 1)?;
    let first_return_y = resources.checked_grid_add(arrow_y, 1)?;
    for line_y in first_return_y..bottom_y {
        cells.try_push(resources, || route_cell(marker_x, line_y, vertical))?;
    }
    let end_anchor = cells.try_push_anchor(
        resources,
        || edge_line_cell(marker_x, arrow_y, vertical),
        StepDirection::Up,
    )?;

    let Some(start_anchor) = start_anchor else {
        return Ok(None);
    };
    let labels = planned_label(
        label,
        CanvasCoord {
            x: marker_x,
            y: bottom_y,
        },
        CanvasCoord {
            x: loop_x,
            y: bottom_y,
        },
    )
    .into_iter()
    .collect();

    Ok(Some(RoutePlan::new(
        cells.into_vec(),
        labels,
        MarkerAnchors::new(start_anchor, end_anchor),
    )))
}

fn plan_left_right_basic_down_then_right_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    let x = from.center_x();
    let corner_y = to.center_y();
    if corner_y <= from.bottom() || to.x <= x + 1 {
        return Ok(None);
    }

    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let mut cells = PlannedRouteCells::new();
    let start_anchor = cells.try_push_anchor(
        resources,
        || edge_line_cell(x, from.bottom(), charset.down_connector),
        StepDirection::Up,
    )?;
    for y in (from.bottom() + 1)..corner_y {
        cells.try_push(resources, || route_cell(x, y, vertical))?;
    }
    cells.try_push(resources, || {
        route_cell(x, corner_y, charset.corner_down_right)
    })?;

    let end = to.x - 1;
    for line_x in (x + 1)..end {
        cells.try_push(resources, || route_cell(line_x, corner_y, horizontal))?;
    }
    let end_anchor = cells.try_push_anchor(
        resources,
        || route_cell(end, corner_y, horizontal),
        StepDirection::Right,
    )?;

    Ok(Some(RoutePlan::new(
        cells.into_vec(),
        Vec::new(),
        MarkerAnchors::new(start_anchor, end_anchor),
    )))
}

fn plan_left_right_basic_right_then_up_route(
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    let y = from.center_y();
    let corner_x = to.center_x();
    if corner_x <= from.right() || y <= to.bottom() + 1 {
        return Ok(None);
    }

    let vertical = edge_line_char(edge, charset, GraphDirection::TopDown);
    let horizontal = edge_line_char(edge, charset, GraphDirection::LeftRight);
    let mut cells = PlannedRouteCells::new();
    let start_anchor = cells.try_push_anchor(
        resources,
        || edge_line_cell(from.right(), y, charset.right_connector),
        StepDirection::Left,
    )?;
    for x in (from.right() + 1)..corner_x {
        cells.try_push(resources, || route_cell(x, y, horizontal))?;
    }
    cells.try_push(resources, || {
        route_cell(corner_x, y, charset.corner_right_up)
    })?;

    let arrow_y = to.bottom() + 1;
    for line_y in (arrow_y + 1)..y {
        cells.try_push(resources, || route_cell(corner_x, line_y, vertical))?;
    }
    let end_anchor = cells.try_push_anchor(
        resources,
        || route_cell(corner_x, arrow_y, vertical),
        StepDirection::Up,
    )?;

    Ok(Some(RoutePlan::new(
        cells.into_vec(),
        Vec::new(),
        MarkerAnchors::new(start_anchor, end_anchor),
    )))
}

fn has_left_right_crossing_pair(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    upper_source: &NodeLayout,
    lower_target: &NodeLayout,
) -> bool {
    edges.iter().any(|edge| {
        if !edge.stroke.is_visible() {
            return false;
        }
        let Some(other_source) = layouts.iter().find(|layout| layout.id == edge.from) else {
            return false;
        };
        let Some(other_target) = layouts.iter().find(|layout| layout.id == edge.to) else {
            return false;
        };
        other_source.x == upper_source.x
            && other_target.x == lower_target.x
            && other_source.center_y() > upper_source.center_y()
            && other_target.center_y() < lower_target.center_y()
    })
}

fn has_left_right_reverse_crossing_pair(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    lower_source: &NodeLayout,
    upper_target: &NodeLayout,
) -> bool {
    edges.iter().any(|edge| {
        if !edge.stroke.is_visible() {
            return false;
        }
        let Some(other_source) = layouts.iter().find(|layout| layout.id == edge.from) else {
            return false;
        };
        let Some(other_target) = layouts.iter().find(|layout| layout.id == edge.to) else {
            return false;
        };
        other_source.x == lower_source.x
            && other_target.x == upper_target.x
            && other_source.center_y() < lower_source.center_y()
            && other_target.center_y() > upper_target.center_y()
    })
}

fn lane_x_between(from: &NodeLayout, to: &NodeLayout) -> usize {
    if from.x < to.x {
        (from.right() + to.x) / 2
    } else {
        (to.right() + from.x) / 2
    }
}

fn lane_y_between(upper: &NodeLayout, lower: &NodeLayout) -> usize {
    (upper.bottom() + lower.y) / 2
}

pub(super) fn self_loop_right_x(layouts: &[NodeLayout], from: &NodeLayout) -> usize {
    layouts
        .iter()
        .filter(|layout| {
            layout.id != from.id && layout.center_y() == from.center_y() && layout.x > from.x
        })
        .map(|layout| layout.x)
        .min()
        .map(|right_x| (from.right() + right_x) / 2)
        .unwrap_or_else(|| from.right() + 2)
}

pub(super) fn self_loop_bottom_y_for_edges(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    from: &NodeLayout,
) -> usize {
    if has_same_row_reverse_edge_into(layouts, edges, from) {
        from.bottom() + 3
    } else {
        self_loop_bottom_y(from)
    }
}

fn self_loop_has_right_neighbor(layouts: &[NodeLayout], from: &NodeLayout) -> bool {
    layouts.iter().any(|layout| {
        layout.id != from.id && layout.center_y() == from.center_y() && layout.x > from.x
    })
}

fn self_loop_bottom_y(from: &NodeLayout) -> usize {
    from.bottom() + 2
}

fn has_same_row_reverse_edge_into(
    layouts: &[NodeLayout],
    edges: &[AsciiGraphEdge],
    target: &NodeLayout,
) -> bool {
    edges.iter().any(|edge| {
        if !edge.stroke.is_visible() || edge.to != target.id || edge.from == target.id {
            return false;
        }
        let Some(from) = layouts.iter().find(|layout| layout.id == edge.from) else {
            return false;
        };
        from.center_y() == target.center_y() && from.x > target.x
    })
}

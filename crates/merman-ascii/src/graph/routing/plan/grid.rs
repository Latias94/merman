use super::super::super::charset::GraphCharset;
use super::super::super::layout::{CanvasCoord, GraphLayout, GridCoord, NodeLayout};
use super::super::super::model::{AsciiGraphEdge, GraphDirection};
use super::super::cell::edge_line_char;
use super::super::label::{
    RoutedLabelPlacement, RoutedLabelText, routed_label_placement_for_text,
    routed_label_right_of_vertical_route_placement_for_text,
};
use super::super::path::{
    GridPathPortPolicy, Port, PortPair, StepDirection, merge_grid_path,
    route_grid_path_with_resources, step_direction,
};
use super::{
    MarkerAnchor, MarkerAnchors, PlannedCellId, PlannedRouteCells, PlannedRouteLabel,
    PlannedRouteSegment, RoutePlan, edge_line_cell_in_segment, route_cell_in_segment,
    route_turn_char,
};
use crate::error::Result;
use crate::resource::ResourceContext;

#[derive(Debug, Clone, Copy)]
pub(super) struct GridRouteOptions {
    port_policy: GridPathPortPolicy,
    segment: PlannedRouteSegment,
    label_mode: GridRouteLabelMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GridRouteLabelMode {
    InlineLongestSegment,
    FirstVerticalTransitLane,
    LastVerticalTransitLane,
}

#[derive(Clone, Copy)]
struct GridLineSpan {
    from: CanvasCoord,
    to: CanvasCoord,
    direction: StepDirection,
}

impl GridRouteOptions {
    pub(super) fn direct() -> Self {
        Self {
            port_policy: GridPathPortPolicy::DirectionalShortest,
            segment: PlannedRouteSegment::Direct,
            label_mode: GridRouteLabelMode::InlineLongestSegment,
        }
    }

    pub(super) fn with_fixed_ports(start_port: Port, end_port: Port) -> Self {
        Self {
            port_policy: GridPathPortPolicy::Fixed(PortPair::new(start_port, end_port)),
            segment: PlannedRouteSegment::Direct,
            label_mode: GridRouteLabelMode::InlineLongestSegment,
        }
    }

    pub(super) fn with_segment(mut self, segment: PlannedRouteSegment) -> Self {
        self.segment = segment;
        self
    }

    pub(super) fn with_first_vertical_transit_label(mut self) -> Self {
        self.label_mode = GridRouteLabelMode::FirstVerticalTransitLane;
        self
    }

    pub(super) fn with_last_vertical_transit_label(mut self) -> Self {
        self.label_mode = GridRouteLabelMode::LastVerticalTransitLane;
        self
    }
}

#[cfg(test)]
pub(super) fn plan_left_right_grid_path_route(
    graph_layout: &GraphLayout,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
) -> Option<RoutePlan> {
    let mut resources = ResourceContext::new(crate::resource::AsciiResourcePolicy::for_profile(
        merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
    ));
    super::materialize_test_markers(
        plan_left_right_grid_path_route_with_resources(
            graph_layout,
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

pub(super) fn plan_left_right_grid_path_route_with_resources(
    graph_layout: &GraphLayout,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    plan_left_right_grid_path_route_with_options_and_resources(
        graph_layout,
        from,
        to,
        edge,
        charset,
        GridRouteOptions::direct(),
        resources,
    )
}

#[cfg(test)]
pub(super) fn plan_left_right_grid_path_route_with_options(
    graph_layout: &GraphLayout,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    options: GridRouteOptions,
) -> Option<RoutePlan> {
    let mut resources = ResourceContext::new(crate::resource::AsciiResourcePolicy::for_profile(
        merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
    ));
    super::materialize_test_markers(
        plan_left_right_grid_path_route_with_options_and_resources(
            graph_layout,
            from,
            to,
            edge,
            charset,
            options,
            &mut resources,
        ),
        edge,
        charset,
    )
}

pub(super) fn plan_left_right_grid_path_route_with_options_and_resources(
    graph_layout: &GraphLayout,
    from: &NodeLayout,
    to: &NodeLayout,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    options: GridRouteOptions,
    resources: &mut ResourceContext,
) -> Result<Option<RoutePlan>> {
    let Some(route) = route_grid_path_with_resources(
        &graph_layout.nodes,
        from,
        to,
        options.port_policy,
        resources,
    )?
    else {
        return Ok(None);
    };
    let path = route.path;
    let start_port = route.ports.start();
    let end_port = route.ports.end();
    if path.len() < 2 {
        return Ok(None);
    }

    let path = merge_grid_path(path);
    let segment = options.segment;
    let (mut cells, lines_drawn, line_dirs, first_line_cell, last_line_cell) =
        plan_grid_path(graph_layout, &path, edge, charset, segment, resources)?;
    if lines_drawn.is_empty() || line_dirs.is_empty() {
        return Ok(None);
    }
    let (Some(start_cell), Some(end_cell)) = (first_line_cell, last_line_cell) else {
        return Ok(None);
    };
    plan_grid_corners(&mut cells, graph_layout, &path, charset, segment, resources)?;
    plan_grid_box_start(
        &mut cells,
        lines_drawn[0].as_slice(),
        start_port,
        charset,
        segment,
        resources,
    )?;
    let labels = planned_grid_label(
        edge.label.as_deref(),
        &lines_drawn,
        &line_dirs,
        options.label_mode,
        charset,
    )
    .into_iter()
    .collect();

    let start_anchor = MarkerAnchor::new(start_cell, opposite_direction(line_dirs[0]));
    let end_anchor = MarkerAnchor::new(
        end_cell,
        *line_dirs.last().unwrap_or(&end_port.terminal_direction()),
    );
    Ok(Some(RoutePlan::new(
        cells.into_vec(),
        labels,
        MarkerAnchors::new(start_anchor, end_anchor),
    )))
}

fn opposite_direction(direction: StepDirection) -> StepDirection {
    match direction {
        StepDirection::Up => StepDirection::Down,
        StepDirection::Right => StepDirection::Left,
        StepDirection::Down => StepDirection::Up,
        StepDirection::Left => StepDirection::Right,
    }
}

fn planned_grid_label(
    label: Option<&str>,
    lines: &[Vec<CanvasCoord>],
    directions: &[StepDirection],
    mode: GridRouteLabelMode,
    charset: &GraphCharset,
) -> Option<PlannedRouteLabel> {
    let text = RoutedLabelText::new_with_profile(label?, charset.width_profile)?;
    let (line, direction) = grid_label_line(lines, directions, mode)?;
    let first = line.first().copied()?;
    let last = line.last().copied()?;
    let placement = grid_label_placement(&text, first, last, mode, direction)?;
    Some(PlannedRouteLabel::new(text, placement))
}

fn grid_label_line<'a>(
    lines: &'a [Vec<CanvasCoord>],
    directions: &[StepDirection],
    mode: GridRouteLabelMode,
) -> Option<(&'a Vec<CanvasCoord>, StepDirection)> {
    let candidates = lines.iter().zip(directions.iter().copied());
    match mode {
        GridRouteLabelMode::InlineLongestSegment => candidates.max_by_key(|(line, _)| line.len()),
        GridRouteLabelMode::FirstVerticalTransitLane => first_vertical_grid_label_line(candidates),
        GridRouteLabelMode::LastVerticalTransitLane => last_vertical_grid_label_line(candidates),
    }
}

fn first_vertical_grid_label_line<'a>(
    mut candidates: impl Iterator<Item = (&'a Vec<CanvasCoord>, StepDirection)>,
) -> Option<(&'a Vec<CanvasCoord>, StepDirection)> {
    candidates.find(|(_, direction)| matches!(direction, StepDirection::Up | StepDirection::Down))
}

fn last_vertical_grid_label_line<'a>(
    candidates: impl Iterator<Item = (&'a Vec<CanvasCoord>, StepDirection)>,
) -> Option<(&'a Vec<CanvasCoord>, StepDirection)> {
    candidates
        .filter(|(_, direction)| matches!(direction, StepDirection::Up | StepDirection::Down))
        .last()
}

fn grid_label_placement(
    label: &RoutedLabelText,
    first: CanvasCoord,
    last: CanvasCoord,
    mode: GridRouteLabelMode,
    direction: StepDirection,
) -> Option<RoutedLabelPlacement> {
    match mode {
        GridRouteLabelMode::InlineLongestSegment => {
            routed_label_placement_for_text(first, last, label)
        }
        GridRouteLabelMode::FirstVerticalTransitLane
        | GridRouteLabelMode::LastVerticalTransitLane => match direction {
            StepDirection::Up | StepDirection::Down => {
                routed_label_right_of_vertical_route_placement_for_text(first, last, label)
            }
            StepDirection::Left | StepDirection::Right => None,
        },
    }
}

fn plan_grid_path(
    graph_layout: &GraphLayout,
    path: &[GridCoord],
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    segment: PlannedRouteSegment,
    resources: &mut ResourceContext,
) -> Result<(
    PlannedRouteCells,
    Vec<Vec<CanvasCoord>>,
    Vec<StepDirection>,
    Option<PlannedCellId>,
    Option<PlannedCellId>,
)> {
    let mut cells = PlannedRouteCells::new();
    let mut lines_drawn = Vec::new();
    let mut line_dirs = Vec::new();
    let mut first_line_cell = None;
    let mut last_line_cell = None;

    for path_segment in path.windows(2) {
        let direction = step_direction(path_segment[0], path_segment[1]);
        let line_span = GridLineSpan {
            from: graph_layout.grid_to_canvas(path_segment[0]),
            to: graph_layout.grid_to_canvas(path_segment[1]),
            direction,
        };
        let (line, first_cell, last_cell) =
            plan_grid_line(&mut cells, line_span, edge, charset, segment, resources)?;
        if !line.is_empty() {
            lines_drawn.push(line);
            line_dirs.push(direction);
            first_line_cell.get_or_insert(first_cell.expect("non-empty line has a first cell"));
            last_line_cell = last_cell;
        }
    }

    Ok((
        cells,
        lines_drawn,
        line_dirs,
        first_line_cell,
        last_line_cell,
    ))
}

fn plan_grid_line(
    cells: &mut PlannedRouteCells,
    line_span: GridLineSpan,
    edge: &AsciiGraphEdge,
    charset: &GraphCharset,
    segment: PlannedRouteSegment,
    resources: &mut ResourceContext,
) -> Result<(
    Vec<CanvasCoord>,
    Option<PlannedCellId>,
    Option<PlannedCellId>,
)> {
    let GridLineSpan {
        from,
        to,
        direction,
    } = line_span;
    let mut drawn = Vec::new();
    let mut first_cell = None;
    let mut last_cell = None;
    match direction {
        StepDirection::Right => {
            let line = edge_line_char(edge, charset, GraphDirection::LeftRight);
            for x in (from.x + 1)..to.x {
                let cell = cells.try_push(resources, || {
                    route_cell_in_segment(x, from.y, line, segment)
                })?;
                first_cell.get_or_insert(cell);
                last_cell = Some(cell);
                drawn.push(CanvasCoord { x, y: from.y });
            }
        }
        StepDirection::Left => {
            let line = edge_line_char(edge, charset, GraphDirection::LeftRight);
            for x in ((to.x + 1)..from.x).rev() {
                let cell = cells.try_push(resources, || {
                    route_cell_in_segment(x, from.y, line, segment)
                })?;
                first_cell.get_or_insert(cell);
                last_cell = Some(cell);
                drawn.push(CanvasCoord { x, y: from.y });
            }
        }
        StepDirection::Down => {
            let line = edge_line_char(edge, charset, GraphDirection::TopDown);
            for y in (from.y + 1)..to.y {
                let cell = cells.try_push(resources, || {
                    route_cell_in_segment(from.x, y, line, segment)
                })?;
                first_cell.get_or_insert(cell);
                last_cell = Some(cell);
                drawn.push(CanvasCoord { x: from.x, y });
            }
        }
        StepDirection::Up => {
            let line = edge_line_char(edge, charset, GraphDirection::TopDown);
            for y in ((to.y + 1)..from.y).rev() {
                let cell = cells.try_push(resources, || {
                    route_cell_in_segment(from.x, y, line, segment)
                })?;
                first_cell.get_or_insert(cell);
                last_cell = Some(cell);
                drawn.push(CanvasCoord { x: from.x, y });
            }
        }
    }
    Ok((drawn, first_cell, last_cell))
}

fn plan_grid_corners(
    cells: &mut PlannedRouteCells,
    graph_layout: &GraphLayout,
    path: &[GridCoord],
    charset: &GraphCharset,
    segment: PlannedRouteSegment,
    resources: &mut ResourceContext,
) -> Result<()> {
    for index in 1..path.len().saturating_sub(1) {
        let previous = step_direction(path[index - 1], path[index]);
        let next = step_direction(path[index], path[index + 1]);
        let coord = graph_layout.grid_to_canvas(path[index]);
        cells.try_push(resources, || {
            route_cell_in_segment(
                coord.x,
                coord.y,
                route_turn_char(previous, next, charset),
                segment,
            )
        })?;
    }
    Ok(())
}

fn plan_grid_box_start(
    cells: &mut PlannedRouteCells,
    first_line: &[CanvasCoord],
    start_port: Port,
    charset: &GraphCharset,
    segment: PlannedRouteSegment,
    resources: &mut ResourceContext,
) -> Result<()> {
    if !charset.unicode {
        return Ok(());
    }
    let Some(from) = first_line.first().copied() else {
        return Ok(());
    };

    cells.try_push(resources, || match start_port.terminal_direction() {
        StepDirection::Up => {
            edge_line_cell_in_segment(from.x, from.y + 1, charset.up_connector, segment)
        }
        StepDirection::Down => edge_line_cell_in_segment(
            from.x,
            from.y.saturating_sub(1),
            charset.down_connector,
            segment,
        ),
        StepDirection::Left => {
            edge_line_cell_in_segment(from.x + 1, from.y, charset.left_connector, segment)
        }
        StepDirection::Right => edge_line_cell_in_segment(
            from.x.saturating_sub(1),
            from.y,
            charset.right_connector,
            segment,
        ),
    })?;
    Ok(())
}

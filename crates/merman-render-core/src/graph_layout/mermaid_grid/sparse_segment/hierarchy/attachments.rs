use super::{
    AttachmentContext, GridHierarchyEnd, GridPortalSelection, PlannedSegment, invalid, reserved,
};
use crate::Result;
use crate::graph_layout::{Axis, Side};
use crate::resources::OperationWorkMeter;
use kurbo::{Point, Rect};

use super::super::super::forest::GridItemKind;
use super::super::super::route_plan::{GridEndpoint, GridEndpointPlan, legal_side_interval};
use super::super::super::routing::{
    GridAttachment, GridPort, GridRoutingView, grid_item_attachment,
};
use super::super::super::sparse_topology::{
    GridPairedPortal, GridPortalRange, grid_paired_portal, grid_portal_ranges,
};
use super::super::APPROACH;

const ALTERNATIVES: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AttachmentMode {
    Item,
    CompatibilityBoundary,
    InteriorEndpoint,
    InteriorPortal,
    ExteriorPortal,
}

impl AttachmentMode {
    pub(super) fn is_portal(self) -> bool {
        matches!(self, Self::InteriorPortal | Self::ExteriorPortal)
    }
}

#[derive(Clone, Copy)]
pub(super) struct EndpointRef<'a> {
    pub end: GridHierarchyEnd,
    pub plan: &'a GridEndpointPlan,
    pub coordinates: &'a [Option<f64>],
}

impl EndpointRef<'_> {
    fn coordinate(
        self,
        index: usize,
        selections: &[GridPortalSelection],
        work: &OperationWorkMeter,
    ) -> Result<Option<f64>> {
        work.charge(selections.len())?;
        if let Some(selected) = selections
            .iter()
            .find(|s| s.end == self.end && s.chain_index == index)
        {
            return Ok(Some(selected.coordinate));
        }
        self.coordinates
            .get(index)
            .copied()
            .ok_or_else(|| invalid("Missing Grid endpoint coordinate slot"))
    }
}

#[derive(Clone, Copy)]
pub(super) struct Terminal {
    pub owner: usize,
    pub side: Side,
    pub port: Point,
    pub connect: Point,
    pub corridor_axis: Axis,
    pub selection: Option<GridPortalSelection>,
}

impl Terminal {
    pub(super) fn corridor(self) -> GridAttachment {
        GridAttachment {
            port: self.port,
            connect: self.connect,
            corridor_axis: self.corridor_axis,
        }
    }

    pub(super) fn stub_length(self) -> f64 {
        (self.port.x - self.connect.x).abs() + (self.port.y - self.connect.y).abs()
    }
}

pub(super) fn resolve(
    context: AttachmentContext<'_>,
    view: GridRoutingView<'_>,
    plan: &PlannedSegment<'_>,
    terminal_index: usize,
    selections: &[GridPortalSelection],
    work: &OperationWorkMeter,
) -> Result<Terminal> {
    work.charge(1)?;
    let container = plan.container;
    let reference = plan.endpoints[terminal_index];
    let index = plan.indices[terminal_index];
    let mode = plan.modes[terminal_index];
    let end = reference.plan.chain[index];
    let geometry = context
        .prepared
        .geometry
        .item(end.owner)
        .ok_or_else(|| invalid("Missing measured Grid attachment owner"))?;
    let middle = geometry.bounds.center();
    let default = match end.side.axis() {
        Axis::Horizontal => middle.y,
        Axis::Vertical => middle.x,
    };
    let coordinate = reference
        .coordinate(index, selections, work)?
        .unwrap_or(default);
    if !coordinate.is_finite() {
        return Err(invalid("Non-finite Grid attachment coordinate"));
    }
    match mode {
        AttachmentMode::Item => item(context, view, container, end, coordinate),
        AttachmentMode::InteriorPortal | AttachmentMode::ExteriorPortal => {
            let (range, bounds, title) = portal_range(context, end, work)?;
            let coordinate = coordinate.max(range.low).min(range.high);
            let portal =
                grid_paired_portal(range.owner_id, bounds, title, end.side, coordinate, work)?;
            Ok(paired(end.owner, portal, mode, None))
        }
        AttachmentMode::CompatibilityBoundary | AttachmentMode::InteriorEndpoint => {
            if end.owner != container || !is_group(context, end.owner) {
                return Err(invalid("Missing Grid boundary attachment owner"));
            }
            let boundary = face(as_rect(geometry.bounds), end.side);
            let connect = match mode {
                AttachmentMode::InteriorEndpoint => boundary - sign(end.side) * APPROACH,
                _ => {
                    let lines = match end.side.axis() {
                        Axis::Horizontal => view.vertical_corridors,
                        Axis::Vertical => view.horizontal_corridors,
                    };
                    *if sign(end.side) < 0.0 {
                        lines.first()
                    } else {
                        lines.last()
                    }
                    .ok_or_else(|| invalid("Missing Grid boundary corridor"))?
                }
            };
            Ok(Terminal {
                owner: end.owner,
                side: end.side,
                port: point(end.side, boundary, coordinate),
                connect: point(end.side, connect, coordinate),
                corridor_axis: tangent_axis(end.side),
                selection: None,
            })
        }
    }
}

fn item(
    context: AttachmentContext<'_>,
    view: GridRoutingView<'_>,
    container: usize,
    end: GridEndpoint,
    coordinate: f64,
) -> Result<Terminal> {
    let child = context
        .child(container, end.owner)?
        .ok_or_else(|| invalid("Grid item attachment is outside its container"))?;
    let given = grid_item_attachment(
        view,
        GridPort {
            child,
            side: end.side,
            coordinate,
            candidates: &[],
        },
    )?;
    Ok(Terminal {
        owner: end.owner,
        side: end.side,
        port: given.port,
        connect: given.connect,
        corridor_axis: given.corridor_axis,
        selection: None,
    })
}

fn paired(
    owner: usize,
    portal: GridPairedPortal<'_>,
    mode: AttachmentMode,
    selection: Option<GridPortalSelection>,
) -> Terminal {
    Terminal {
        owner,
        side: portal.side,
        port: Point::new(
            (portal.interior.x + portal.exterior.x) / 2.0,
            (portal.interior.y + portal.exterior.y) / 2.0,
        ),
        connect: if mode == AttachmentMode::InteriorPortal {
            portal.interior
        } else {
            portal.exterior
        },
        corridor_axis: tangent_axis(portal.side),
        selection,
    }
}

pub(super) fn portal_alternatives(
    context: AttachmentContext<'_>,
    reference: EndpointRef<'_>,
    index: usize,
    mode: AttachmentMode,
    selections: &[GridPortalSelection],
    work: &OperationWorkMeter,
) -> Result<Vec<Terminal>> {
    let end = reference.plan.chain[index];
    let (range, bounds, title) = portal_range(context, end, work)?;
    let default = center_tangent(bounds, end.side);
    let selected = reference
        .coordinate(index, selections, work)?
        .unwrap_or(default)
        .max(range.low)
        .min(range.high);
    let view = context.containers.container(end.owner)?.view();
    let corridors = match end.side.axis() {
        Axis::Horizontal => view.horizontal_corridors,
        Axis::Vertical => view.vertical_corridors,
    };
    let coordinates = alternative_coordinates(selected, range.low, range.high, corridors, work)?;
    let mut alternatives = reserved(coordinates.len(), work)?;
    for coordinate in coordinates {
        let portal = grid_paired_portal(range.owner_id, bounds, title, end.side, coordinate, work)?;
        alternatives.push(paired(
            end.owner,
            portal,
            mode,
            Some(GridPortalSelection {
                end: reference.end,
                chain_index: index,
                coordinate,
            }),
        ));
    }
    Ok(alternatives)
}

pub(super) fn item_alternatives(
    context: AttachmentContext<'_>,
    view: GridRoutingView<'_>,
    container: usize,
    reference: EndpointRef<'_>,
    index: usize,
    lane_offset: f64,
    work: &OperationWorkMeter,
) -> Result<Vec<Terminal>> {
    let endpoint = reference.plan.chain[index];
    let geometry = &context.prepared.geometry;
    let shell = geometry
        .item(endpoint.owner)
        .ok_or_else(|| invalid("Missing Grid item geometry"))?;
    let center = shell.bounds.center();
    let mut alternatives = reserved(ALTERNATIVES, work)?;
    for side in [Side::Right, Side::Bottom, Side::Left, Side::Top] {
        work.charge(1)?;
        if side == endpoint.side {
            continue;
        }
        if let Some((low, high)) = legal_side_interval(geometry, endpoint.owner, side) {
            let center = match side.axis() {
                Axis::Horizontal => center.y,
                Axis::Vertical => center.x,
            };
            let coordinate = (center + lane_offset).min(high).max(low);
            let mut end = endpoint;
            end.side = side;
            alternatives.push(item(context, view, container, end, coordinate)?);
        }
    }
    Ok(alternatives)
}

fn alternative_coordinates(
    selected: f64,
    low: f64,
    high: f64,
    corridors: &[f64],
    work: &OperationWorkMeter,
) -> Result<Vec<f64>> {
    let mut nearest = reserved(corridors.len(), work)?;
    nearest.extend(
        corridors
            .iter()
            .copied()
            .filter(|&coord| coord >= low && coord <= high),
    );
    let levels = nearest
        .len()
        .checked_ilog2()
        .map_or(0, |depth| depth as usize + 1);
    work.charge(
        nearest
            .len()
            .checked_mul(levels)
            .ok_or_else(|| work.arithmetic_overflow())?,
    )?;
    nearest.sort_unstable_by(|&a, &b| {
        (a - selected)
            .abs()
            .total_cmp(&(b - selected).abs())
            .then_with(|| a.total_cmp(&b))
    });
    let mut alternatives = reserved(ALTERNATIVES, work)?;
    for coordinate in nearest.into_iter().chain([(low + high) / 2.0, low, high]) {
        work.charge(1)?;
        if coordinate != selected && !alternatives.contains(&coordinate) {
            alternatives.push(coordinate);
            if alternatives.len() == ALTERNATIVES {
                break;
            }
        }
    }
    Ok(alternatives)
}

fn portal_range<'id>(
    context: AttachmentContext<'id>,
    end: GridEndpoint,
    work: &OperationWorkMeter,
) -> Result<(GridPortalRange<'id>, Rect, Option<Rect>)> {
    let owner = context
        .prepared
        .forest
        .items
        .get(end.owner)
        .filter(|item| matches!(item.kind, GridItemKind::Group(_)))
        .ok_or_else(|| invalid("Missing Grid paired portal owner"))?;
    let bounds = as_rect(
        context
            .prepared
            .geometry
            .item(end.owner)
            .ok_or_else(|| invalid("Missing Grid group geometry"))?
            .bounds,
    );
    let title = title(&context.prepared.geometry, end.owner);
    let range = grid_portal_ranges(owner.item.id, bounds, title, work)?
        .into_iter()
        .find(|range| range.side == end.side)
        .ok_or_else(|| invalid("No legal Grid portal range"))?;
    Ok((range, bounds, title))
}

pub(super) fn title(
    geometry: &super::super::super::forest::GridForestGeometry,
    owner: usize,
) -> Option<Rect> {
    geometry
        .containers
        .get(owner)?
        .as_ref()?
        .title_bounds
        .map(as_rect)
}

fn is_group(context: AttachmentContext<'_>, owner: usize) -> bool {
    context
        .prepared
        .forest
        .items
        .get(owner)
        .is_some_and(|item| matches!(item.kind, GridItemKind::Group(_)))
}

pub(super) fn as_rect(rect: crate::graph_layout::Rect) -> Rect {
    Rect::new(rect.x, rect.y, rect.x + rect.width, rect.y + rect.height)
}

pub(super) fn face(bounds: Rect, side: Side) -> f64 {
    match side {
        Side::Left => bounds.x0,
        Side::Right => bounds.x1,
        Side::Top => bounds.y0,
        Side::Bottom => bounds.y1,
    }
}

pub(super) fn point(side: Side, normal: f64, tangent: f64) -> Point {
    match side.axis() {
        Axis::Horizontal => Point::new(normal, tangent),
        Axis::Vertical => Point::new(tangent, normal),
    }
}

pub(super) fn sign(side: Side) -> f64 {
    if matches!(side, Side::Left | Side::Top) {
        -1.0
    } else {
        1.0
    }
}

pub(super) fn tangent_axis(side: Side) -> Axis {
    match side.axis() {
        Axis::Horizontal => Axis::Vertical,
        Axis::Vertical => Axis::Horizontal,
    }
}

fn center_tangent(bounds: Rect, side: Side) -> f64 {
    match side.axis() {
        Axis::Horizontal => (bounds.y0 + bounds.y1) / 2.0,
        Axis::Vertical => (bounds.x0 + bounds.x1) / 2.0,
    }
}

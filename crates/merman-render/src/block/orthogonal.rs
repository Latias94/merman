//! Route edges over the grid's placed shapes in drawing coordinates.

use super::edge_geometry::BlockOrthogonalRoute;
use super::{
    BlockRectangleKind, BlockShapeBoundary, BlockShapeGeometry, BlockTypographyThemePlan,
    block_label_is_effectively_empty, block_render_edge_points, decode_block_label_html,
    resolve_block_node_sources,
};
use crate::Result;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{BlockDiagramLayout, LayoutLabel, LayoutPoint};
use crate::resources::OperationWorkMeter;
use crate::svg::BaseEdgeMarkerKind;
use kurbo::{Point, Rect, Vec2};
use merman_layout_shoal::geom::{Bounds, BoundsIndex};
use merman_layout_shoal::{
    OrthogonalSpan, Rect as Obstacle, RoutePoint, Side, Spacing, route_fixed_spans,
};
use merman_model::families::block::{BlockDiagramRenderModel, BlockEdge};
use merman_render_core::graph_layout::{Endpoint, Marker};
use merman_render_core::route_paint::{
    self, Clearance, CurveTerminals, EdgeClearance, ObstacleOwner, RouteStyle, TerminalContact,
};
use merman_render_structured::block::BlockEdgeMarkerGeometry;
use merman_theme_contract::v2::EdgeStyle;
use std::{collections::HashMap, sync::Arc};

mod fallback;

#[cfg(test)]
mod tests;

const CORNER_RADIUS: f64 = 24.0;
const SIDES: [Side; 4] = [Side::Top, Side::Right, Side::Bottom, Side::Left];

#[derive(Clone, Copy)]
struct EdgePlacement {
    occurrence: usize,
    index: usize,
    bodies: [u32; 2],
}

fn edge_marker_geometry(edge: &BlockEdge, start: bool) -> Option<BlockEdgeMarkerGeometry> {
    let arrow = if start {
        edge.arrow_type_start.as_deref()
    } else {
        edge.arrow_type_end.as_deref()
    };
    BaseEdgeMarkerKind::from_arrow(arrow, start).and_then(BlockEdgeMarkerGeometry::from_kind)
}

fn marker_bounds(marker: BlockEdgeMarkerGeometry, reference: Point, outward: Vec2) -> Rect {
    let across = Vec2::new(-outward.y, outward.x) * (marker.routing.width / 2.0);
    let front = reference - outward * marker.front;
    let back = reference + outward * marker.routing.depth;
    Rect::from_points(front - across, front + across)
        .union(Rect::from_points(back - across, back + across))
}

#[derive(Clone, Copy)]
struct Wall {
    rect: Rect,
    owner: ObstacleOwner,
}

impl Wall {
    fn marker(rect: Rect, edge: usize, body: u32, contact: RoutePoint) -> Self {
        Self {
            rect,
            owner: ObstacleOwner::Marker(edge, TerminalContact::new(Endpoint::Node(body), contact)),
        }
    }
}

struct Terminal {
    contact: RoutePoint,
    side: Side,
    anchor: RoutePoint,
    reference: Point,
    marker: Option<BlockEdgeMarkerGeometry>,
}

type Anchor = (RoutePoint, Side);

impl Terminal {
    fn new(
        shape: &BlockShapeGeometry,
        bounds: Rect,
        side: Side,
        marker: Option<BlockEdgeMarkerGeometry>,
        margin: f64,
        across: Option<f64>,
    ) -> Option<Self> {
        if !bounds.is_finite() {
            return None;
        }
        let center = Point::new(shape.allocated.x, shape.allocated.y);
        let across = across.unwrap_or_else(|| across_coordinate(center, side));
        let contact = to_point(&shape.face_contact(side, across)?);
        let normal = normal(side);
        let marker_metrics = marker.map(|marker| marker.routing).unwrap_or_default();
        let depth = marker_metrics.trim + route_paint::LEAD.max(marker_metrics.depth);
        let mut anchor = contact + normal * (depth + margin);
        let reference = contact + normal * marker_metrics.trim;
        // The router reconstructs the high bounds from origin and extent. Compute the
        // setback from both active obstacles, avoiding cancellation near a curved face.
        let exterior = |wall: Rect| match side {
            Side::Top => -wall.y0,
            Side::Right => wall.x0 + wall.width(),
            Side::Bottom => wall.y0 + wall.height(),
            Side::Left => -wall.x0,
        };
        let wall = exterior(bounds);
        let wall = marker.map_or(wall, |marker| {
            wall.max(exterior(marker_bounds(marker, reference, normal)))
        });
        let plane = (wall + margin).next_up();
        match side {
            Side::Top => anchor.y = anchor.y.min(-plane),
            Side::Right => anchor.x = anchor.x.max(plane),
            Side::Bottom => anchor.y = anchor.y.max(plane),
            Side::Left => anchor.x = anchor.x.min(-plane),
        }
        if !contact.is_finite() || !anchor.is_finite() || contact == center || contact == anchor {
            return None;
        }
        Some(Self {
            contact: RoutePoint::new(contact.x, contact.y, side.axis()),
            side,
            anchor: RoutePoint::new(anchor.x, anchor.y, side.axis()),
            reference,
            marker,
        })
    }

    fn marker_bounds(&self) -> Option<Rect> {
        self.marker
            .map(|marker| marker_bounds(marker, self.reference, normal(self.side)))
    }

    fn lead_bounds(&self) -> Rect {
        Rect::from_points(knot_point(self.contact), knot_point(self.anchor))
    }

    fn anchor(&self) -> Anchor {
        (self.anchor, self.side)
    }

    fn port_band(&self, margin: f64) -> Bounds {
        let across = across_coordinate(knot_point(self.contact), self.side);
        let half_pitch = minimum_pitch(margin) / 2.0;
        Bounds {
            x: [across - half_pitch, across + half_pitch],
            y: [0.0, 1.0],
        }
    }

    fn clear(
        &self,
        walls: &[Wall],
        node: u32,
        margin: f64,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<bool> {
        work.charge_adapter(walls.len())?;
        let lead = self.lead_bounds();
        let marker = self.marker_bounds();
        Ok(walls.iter().all(|wall| {
            if wall.owner == ObstacleOwner::Node(node) {
                return true;
            }
            let blocked = wall.rect.inflate(margin, margin);
            !intersects(lead, blocked) && marker.is_none_or(|marker| !intersects(marker, blocked))
        }))
    }
}

/// Keep an existing label attached to its edge with a straight bridge beneath it.
/// The approach and departure are distinct spans guarded by the full label box.
#[derive(Clone, Copy)]
struct LabelPass([Anchor; 2]);

impl LabelPass {
    fn candidates(label: &LayoutLabel, [a, b]: [Point; 2], margin: f64) -> [Self; 2] {
        let rect = label_box(label);
        let pair = |a, b| -> Self {
            Self([a, b].map(|(x, y, side): (_, _, Side)| {
                (RoutePoint::gap_boundary(x, y, side.axis()), side)
            }))
        };
        let horizontal = pair(
            ((rect.x0 - margin).next_down(), label.y, Side::Left),
            (
                (rect.x0 + rect.width() + margin).next_up(),
                label.y,
                Side::Right,
            ),
        );
        let vertical = pair(
            (label.x, (rect.y0 - margin).next_down(), Side::Top),
            (
                label.x,
                (rect.y0 + rect.height() + margin).next_up(),
                Side::Bottom,
            ),
        );
        let orient = |mut pass: Self, forward: bool| {
            if !forward {
                pass.0.reverse();
            }
            pass
        };
        [orient(horizontal, a.x <= b.x), orient(vertical, a.y <= b.y)]
    }

    fn clear(
        self,
        walls: &[Wall],
        edge: usize,
        margin: f64,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<bool> {
        work.charge_adapter(walls.len())?;
        let bridge = self.bridge();
        Ok(bridge.is_finite()
            && walls.iter().all(|wall| {
                wall.owner == ObstacleOwner::EdgeLabel(edge)
                    || !intersects(bridge, wall.rect.inflate(margin, margin))
            }))
    }

    fn bridge(self) -> Rect {
        Rect::from_points(knot_point(self.0[0].0), knot_point(self.0[1].0))
    }
}

struct Request {
    placement: EdgePlacement,
    ends: [Terminal; 2],
    label: Option<LabelPass>,
}

impl Request {
    fn score(&self) -> f64 {
        score(&self.ends[0], &self.ends[1], self.label)
    }

    fn marker_walls(&self) -> [Option<Wall>; 2] {
        std::array::from_fn(|slot| {
            let end = &self.ends[slot];
            let rect = end.marker_bounds()?;
            Some(Wall::marker(
                rect,
                self.placement.occurrence,
                self.placement.bodies[slot],
                end.contact,
            ))
        })
    }

    fn clear(
        &self,
        walls: &[Wall],
        reserved: &Reservations,
        margin: f64,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<bool> {
        let [a, b] = &self.ends;
        let [from, to] = self.placement.bodies;
        if !clear_pair(a, b, margin)
            || !a.clear(walls, from, margin, work)?
            || !b.clear(walls, to, margin, work)?
            || !reserved.clear(a, from, margin, work)?
            || !reserved.clear(b, to, margin, work)?
        {
            return Ok(false);
        }
        self.label.map_or(Ok(true), |label| {
            let bridge = label.bridge().inflate(margin, margin);
            Ok(label.clear(walls, self.placement.occurrence, margin, work)?
                && label_clears_markers(bridge, a, b)
                && reserved.clear_box(bridge, work)?)
        })
    }

    fn spans(&self) -> impl Iterator<Item = OrthogonalSpan> {
        let [a, b] = &self.ends;
        let end = b.anchor();
        let (first_end, second) =
            self.label
                .map_or((end, None), |LabelPass([entry, departure])| {
                    (
                        entry,
                        Some(OrthogonalSpan {
                            start: departure,
                            end,
                        }),
                    )
                });
        std::iter::once(OrthogonalSpan {
            start: a.anchor(),
            end: first_end,
        })
        .chain(second)
    }

    fn solve(self, inner: impl Iterator<Item = RoutePoint>) -> SolvedRequest {
        let [a, b] = self.ends.each_ref().map(|end| end.contact);
        let mut points: Vec<_> = std::iter::once(a)
            .chain(inner)
            .chain(std::iter::once(b))
            .collect();
        points.dedup_by(|a, b| a.x == b.x && a.y == b.y);
        SolvedRequest {
            request: self,
            points,
        }
    }
}

struct SolvedRequest {
    request: Request,
    points: Vec<RoutePoint>,
}

impl SolvedRequest {
    fn paint(
        &self,
        style: RouteStyle,
        clearance: Option<EdgeClearance<'_>>,
        scratch: &mut route_paint::RoutePaintScratch,
    ) -> Result<BlockOrthogonalRoute> {
        let [a, b] = &self.request.ends;
        let marker = |end: &Terminal| -> Marker {
            end.marker.map(|marker| marker.routing).unwrap_or_default()
        };
        let markers = (marker(a), marker(b));
        let path = route_paint::RoutePaintRequest {
            points: &self.points,
            sides: (a.side, b.side),
            style,
            markers,
            curve_terminals: CurveTerminals::Fixed,
            stitches: &[],
            clearance: clearance.map(|guard| guard.with_terminal_contacts((a.contact, b.contact))),
        }
        .paint_solved_svg(scratch)
        .map_err(|error| crate::Error::InvalidModel {
            message: format!("Block route {}: {error}", self.request.placement.occurrence),
        })?;
        Ok(BlockOrthogonalRoute {
            path: path.d,
            bounds: path.bounds,
            marker_refs: [a.reference, b.reference].map(layout_point),
            markers,
        })
    }
}

fn normal(side: Side) -> Vec2 {
    match side {
        Side::Top => Vec2::new(0.0, -1.0),
        Side::Right => Vec2::new(1.0, 0.0),
        Side::Bottom => Vec2::new(0.0, 1.0),
        Side::Left => Vec2::new(-1.0, 0.0),
    }
}

fn shape_box(shape: &BlockShapeGeometry) -> Rect {
    let (x0, y0, x1, y1) = shape.rendered_extents();
    Rect::new(x0, y0, x1, y1)
}

fn across_coordinate(point: Point, side: Side) -> f64 {
    match side {
        Side::Top | Side::Bottom => point.x,
        Side::Left | Side::Right => point.y,
    }
}

fn face_span(bounds: Rect, side: Side) -> [f64; 2] {
    match side {
        Side::Top | Side::Bottom => [bounds.x0, bounds.x1],
        Side::Left | Side::Right => [bounds.y0, bounds.y1],
    }
}

fn contact_work(shape: &BlockShapeGeometry) -> usize {
    match &shape.boundary {
        BlockShapeBoundary::Polygon { points, .. } => points.len().max(1),
        _ => 1,
    }
}

fn minimum_pitch(margin: f64) -> f64 {
    Spacing::default().min_port_pitch.max(2.0 * margin)
}

fn rect_bounds(rect: Rect) -> Bounds {
    Bounds {
        x: [rect.x0, rect.x1],
        y: [rect.y0, rect.y1],
    }
}

fn label_box(label: &LayoutLabel) -> Rect {
    Rect::from_center_size((label.x, label.y), (label.width, label.height))
}

fn knot_point(p: RoutePoint) -> Point {
    Point::new(p.x, p.y)
}
fn to_point(p: &LayoutPoint) -> Point {
    Point::new(p.x, p.y)
}
fn layout_point(point: Point) -> LayoutPoint {
    LayoutPoint {
        x: point.x,
        y: point.y,
    }
}

fn intersects(a: Rect, b: Rect) -> bool {
    a.x0 < b.x1 && b.x0 < a.x1 && a.y0 < b.y1 && b.y0 < a.y1
}

fn clear_pair(a: &Terminal, b: &Terminal, margin: f64) -> bool {
    if knot_point(a.contact) == knot_point(b.contact)
        || knot_point(a.anchor) == knot_point(b.anchor)
    {
        return false;
    }
    let a_marker = a.marker_bounds();
    let b_marker = b.marker_bounds();
    a_marker.is_none_or(|a_mark| {
        let halo = a_mark.inflate(margin, margin);
        !intersects(b.lead_bounds(), halo)
            && b_marker.is_none_or(|b_mark| !intersects(halo, b_mark))
    }) && b_marker.is_none_or(|b_mark| !intersects(a.lead_bounds(), b_mark.inflate(margin, margin)))
}

fn label_clears_markers(bridge: Rect, a: &Terminal, b: &Terminal) -> bool {
    [a, b]
        .into_iter()
        .filter_map(Terminal::marker_bounds)
        .all(|marker| !intersects(bridge, marker))
}

fn backtracking_ends((a, a_side): Anchor, (b, b_side): Anchor) -> usize {
    let between = knot_point(b) - knot_point(a);
    usize::from(between.dot(normal(a_side)) < 0.0) + usize::from(between.dot(normal(b_side)) > 0.0)
}

fn span_score(a: Anchor, b: Anchor) -> f64 {
    let (a_point, b_point) = (a.0, b.0);
    let between = knot_point(b_point) - knot_point(a_point);
    // Favor the ports facing one another when several pairs have the same runway.
    between.x.abs() + between.y.abs() + 40.0 * backtracking_ends(a, b) as f64
}

fn score(a: &Terminal, b: &Terminal, label: Option<LabelPass>) -> f64 {
    let leads = (knot_point(a.anchor) - knot_point(a.contact)).hypot()
        + (knot_point(b.anchor) - knot_point(b.contact)).hypot();
    leads
        + match label {
            Some(LabelPass([entry, departure])) => {
                span_score(a.anchor(), entry)
                    + (knot_point(entry.0) - knot_point(departure.0)).hypot()
                    + span_score(departure, b.anchor())
            }
            None => span_score(a.anchor(), b.anchor()),
        }
}

fn facing_sides(deltas: [Vec2; 2], bounds: [Rect; 2], labeled: bool) -> [Option<Side>; 2] {
    let scaled = std::array::from_fn::<_, 2, _>(|slot| {
        let Vec2 { x, y } = deltas[slot];
        let bounds = bounds[slot];
        Vec2::new(x / bounds.width().max(1.0), y / bounds.height().max(1.0))
    });
    // Unlabeled edges use one facing axis across the gap. Both outlines contribute
    // to its choice so a short or narrow peer can still take a straight runway.
    let horizontal = scaled[0].x.abs() + scaled[1].x.abs() > scaled[0].y.abs() + scaled[1].y.abs();
    scaled.map(|delta| {
        if delta == Vec2::ZERO {
            return None;
        }
        let horizontal = if labeled {
            delta.x.abs() > delta.y.abs()
        } else {
            horizontal
        };
        Some(match (horizontal, delta.x < 0.0, delta.y < 0.0) {
            (true, true, _) => Side::Left,
            (true, false, _) => Side::Right,
            (false, _, true) => Side::Top,
            (false, _, false) => Side::Bottom,
        })
    })
}

#[derive(Default)]
struct Reservations {
    // Terminal leads and label bridges sit outside the spans sent to the router.
    boxes: BoundsIndex,
    ports: HashMap<(u32, u8), BoundsIndex>,
}

impl Reservations {
    fn clear(
        &self,
        end: &Terminal,
        node: u32,
        margin: f64,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<bool> {
        if let Some(ports) = self.ports.get(&(node, end.side as u8))
            && ports
                .any_overlap(end.port_band(margin), work)
                .map_err(|error| work.map_shoal_error(error))?
        {
            return Ok(false);
        }
        if !self.clear_box(end.lead_bounds().inflate(margin, margin), work)? {
            return Ok(false);
        }
        match end.marker_bounds() {
            Some(marker) => self.clear_box(marker.inflate(margin, margin), work),
            None => Ok(true),
        }
    }

    fn clear_box(&self, rect: Rect, work: &mut OperationLayoutWorkControl) -> Result<bool> {
        self.boxes
            .any_overlap(rect_bounds(rect), work)
            .map(|overlap| !overlap)
            .map_err(|error| work.map_shoal_error(error))
    }

    fn reserve(
        &mut self,
        request: &Request,
        margin: f64,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<()> {
        for (end, node) in request.ends.iter().zip(request.placement.bodies) {
            self.ports
                .entry((node, end.side as u8))
                .or_default()
                .insert(end.port_band(margin), work)
                .map_err(|error| work.map_shoal_error(error))?;
        }
        let marker_rects = request.ends.iter().filter_map(Terminal::marker_bounds);
        let leads = request
            .ends
            .iter()
            .map(|end| end.lead_bounds().inflate(margin, margin));
        for rect in marker_rects.chain(leads).chain(
            request
                .label
                .map(|pass| pass.bridge().inflate(margin, margin)),
        ) {
            self.boxes
                .insert(rect_bounds(rect), work)
                .map_err(|error| work.map_shoal_error(error))?;
        }
        Ok(())
    }
}

struct FaceEndpoint {
    request: usize,
    slot: usize,
    node: u32,
    side: Side,
    peer_across: f64,
    peer_angle: f64,
    peer_distance: f64,
}

fn spread_face_terminals(
    requests: &mut [Request],
    shapes: &[BlockShapeGeometry],
    bounds: &[Rect],
    walls: &[Wall],
    margin: f64,
    work: &mut OperationLayoutWorkControl,
) -> Result<()> {
    let count = work.checked_mul(requests.len(), 2)?;
    let levels = count.checked_ilog2().map_or(0, |n| n as usize + 1);
    work.charge_adapter(work.checked_mul(count, levels)?)?;
    let mut endpoints = Vec::with_capacity(count);
    for (request, edge) in requests.iter().enumerate() {
        for (slot, end) in edge.ends.iter().enumerate() {
            let peer = &shapes[edge.placement.bodies[1 - slot] as usize].allocated;
            let peer = Point::new(peer.x, peer.y);
            let contact = knot_point(end.contact);
            let peer_across = across_coordinate(peer, end.side);
            let peer_distance = (peer - contact).dot(normal(end.side));
            endpoints.push(FaceEndpoint {
                request,
                slot,
                node: edge.placement.bodies[slot],
                side: end.side,
                peer_across,
                peer_angle: (peer_across - across_coordinate(contact, end.side))
                    .atan2(peer_distance.max(0.0)),
                peer_distance,
            });
        }
    }
    endpoints.sort_unstable_by(|a, b| {
        (a.node, a.side as u8)
            .cmp(&(b.node, b.side as u8))
            .then_with(|| a.peer_angle.total_cmp(&b.peer_angle))
            .then_with(|| a.peer_across.total_cmp(&b.peer_across))
            // Equal projections order from nearer to farther along the outward normal.
            .then_with(|| a.peer_distance.total_cmp(&b.peer_distance))
            .then_with(|| (a.request, a.slot).cmp(&(b.request, b.slot)))
    });
    let spacing = Spacing::default();
    for face in endpoints.chunk_by(|a, b| a.node == b.node && a.side == b.side) {
        let [first, _, ..] = face else { continue };
        let shape = &shapes[first.node as usize];
        let bounds = bounds[first.node as usize];
        let [lo, hi] = face_span(bounds, first.side);
        let widest = face
            .iter()
            .filter_map(|end| requests[end.request].ends[end.slot].marker)
            .map(|marker| marker.routing.width)
            .fold(0.0, f64::max);
        let required_pitch = (widest + margin).max(minimum_pitch(margin));
        let end_inset = (widest / 2.0 + margin).max(required_pitch / 2.0);
        let room_per_side = (hi - lo - (face.len() - 1) as f64 * required_pitch) / 2.0;
        let inset = spacing.corner_clearance.min(room_per_side).max(end_inset);
        let (mut lo, mut hi) = (lo + inset, hi - inset);
        let span = hi - lo;
        if span < 0.0 || !span.is_finite() {
            continue;
        }
        let slots = 1 + ((span / required_pitch) as usize).min(face.len() - 1);
        let pitch = if slots == face.len() {
            spacing
                .port_pitch
                .max(required_pitch)
                .min(span / (slots - 1) as f64)
        } else {
            // Fill available slots at the minimum pitch. The remaining endpoints
            // keep their draft position and can retry once against the reserved ports.
            let center = lo + span / 2.0;
            let half = (slots - 1) as f64 * required_pitch / 2.0;
            lo = center - half;
            hi = center + half;
            required_pitch
        };
        let mut next = lo;
        for (rank, end) in face.iter().take(slots).enumerate() {
            let last = hi - pitch * (slots - 1 - rank) as f64;
            let across = end.peer_across.clamp(next.min(last), last);
            next = across + pitch;
            work.charge_adapter(contact_work(shape))?;
            let request = &requests[end.request];
            let marker = request.ends[end.slot].marker;
            if let Some(terminal) =
                Terminal::new(shape, bounds, end.side, marker, margin, Some(across))
                && terminal.clear(walls, end.node, margin, work)?
            {
                requests[end.request].ends[end.slot] = terminal;
            }
        }
    }
    Ok(())
}

fn retry_contacts(bounds: [f64; 2], inset: f64, pitch: f64) -> impl Iterator<Item = f64> {
    let [lo, hi] = bounds;
    let center = lo + (hi - lo) / 2.0;
    let (lo, hi) = (lo + inset, hi - inset);
    [
        center,
        center - pitch,
        center + pitch,
        center - 2.0 * pitch,
        center + 2.0 * pitch,
        lo,
        hi,
    ]
    .into_iter()
    .filter(move |coord| (lo..=hi).contains(coord))
}

fn fixed_walls(
    model: &BlockDiagramRenderModel,
    layout: &BlockDiagramLayout,
    bounds: &[Rect],
    edge_indices: &HashMap<&str, usize>,
    work: &mut OperationLayoutWorkControl,
) -> Result<Option<Vec<Wall>>> {
    work.charge_adapter(model.blocks_flat.len())?;
    let sources = resolve_block_node_sources(model);
    let mut walls = Vec::new();
    for (index, (node, shape)) in layout
        .nodes
        .iter()
        .zip(&layout.shape_geometries)
        .enumerate()
    {
        work.charge_adapter(1)?;
        if !matches!(
            shape.boundary,
            BlockShapeBoundary::Rectangle {
                kind: BlockRectangleKind::Composite,
                ..
            }
        ) {
            walls.push(Wall {
                rect: bounds[index],
                owner: ObstacleOwner::Node(
                    u32::try_from(index).map_err(|_| work.record_arithmetic_overflow())?,
                ),
            });
        }
        if let Some(source) = sources.get(node.id.as_str())
            && !block_label_is_effectively_empty(&decode_block_label_html(source.label))
            && let Some(width) = node.label_width.filter(|&w| w > 0.0)
            && let Some(height) = node.label_height.filter(|&h| h > 0.0)
        {
            let x = node.x + shape.label_offset_x(source.block_type);
            walls.push(Wall {
                rect: Rect::from_center_size((x, node.y), (width, height)),
                owner: ObstacleOwner::GroupTitle(0),
            });
        }
    }
    for (occurrence, edge) in model.edges.iter().enumerate() {
        work.charge_adapter(1)?;
        let Some(&index) = edge_indices.get(edge.id.as_str()) else {
            continue;
        };
        let edge_layout = &layout.edges[index];
        if let Some(label) = &edge_layout.label {
            walls.push(Wall {
                rect: label_box(label),
                owner: ObstacleOwner::EdgeLabel(occurrence),
            });
        }
    }
    // Some compatibility-only grid constructions have nonfinite slot geometry.
    if walls.iter().any(|wall| !wall.rect.is_finite()) {
        return Ok(None);
    }
    Ok(Some(walls))
}

pub(super) fn prepare(
    model: &BlockDiagramRenderModel,
    layout: &mut BlockDiagramLayout,
    typography: &BlockTypographyThemePlan,
    meter: &Arc<OperationWorkMeter>,
) -> Result<Vec<Option<BlockOrthogonalRoute>>> {
    if layout.edges.is_empty() || layout.nodes.len() != layout.shape_geometries.len() {
        return Ok(Vec::new());
    }
    let mut work = OperationLayoutWorkControl::new(Arc::clone(meter));
    work.charge_adapter(work.checked_add(layout.nodes.len(), layout.edges.len())?)?;
    let bodies: HashMap<_, _> = layout
        .shape_geometries
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let index = u32::try_from(i).map_err(|_| work.record_arithmetic_overflow())?;
            Ok((r.id.as_str(), index))
        })
        .collect::<Result<_>>()?;
    let geometries: HashMap<_, _> = layout
        .shape_geometries
        .iter()
        .map(|r| (r.id.as_str(), r))
        .collect();
    let edge_indices: HashMap<_, _> = layout
        .edges
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();
    // Duplicate ids retain the renderer's historical first match.
    if bodies.len() != layout.nodes.len() || edge_indices.len() != layout.edges.len() {
        return Ok(Vec::new());
    }
    let edge_style = typography.native().map(|style| style.form.edge_style);
    let route_style = match edge_style {
        Some(EdgeStyle::Curved) => RouteStyle::Rounded(CORNER_RADIUS),
        Some(style) => RouteStyle::from_theme(style),
        None => RouteStyle::Step,
    };
    let bounds = layout
        .shape_geometries
        .iter()
        .map(|shape| {
            work.charge_adapter(contact_work(shape))?;
            Ok(shape_box(shape))
        })
        .collect::<Result<Vec<_>>>()?;
    let Some(walls) = fixed_walls(model, layout, &bounds, &edge_indices, &mut work)? else {
        return Ok(Vec::new());
    };
    let stroke_width = typography
        .css_binding()
        .edge_stroke_width_px
        .unwrap_or(2.0)
        .max(0.0);
    let edge_sep = (stroke_width * 2.0).max(4.0);
    if !edge_sep.is_finite() {
        return Ok(Vec::new());
    }
    let margin = edge_sep / 2.0;
    let fallback = fallback::markers(
        model,
        layout,
        &edge_indices,
        &geometries,
        &bodies,
        edge_style,
        &mut work,
    )?;
    let choose_request = |placement: EdgePlacement,
                          walls: &[Wall],
                          reserved: Option<&Reservations>,
                          work: &mut OperationLayoutWorkControl|
     -> Result<Option<Request>> {
        let EdgePlacement {
            occurrence,
            index,
            bodies,
        } = placement;
        let edge = &model.edges[occurrence];
        let label = layout.edges[index].label.as_ref();
        let centers = bodies.map(|i| {
            let shape = &layout.shape_geometries[i as usize];
            Point::new(shape.allocated.x, shape.allocated.y)
        });
        let targets = label.map_or([centers[1], centers[0]], |label| {
            [Point::new(label.x, label.y); 2]
        });
        let bounds = bodies.map(|node| bounds[node as usize]);
        let facing = facing_sides(
            [targets[0] - centers[0], targets[1] - centers[1]],
            bounds,
            label.is_some(),
        );
        let mut label_passes = Vec::new();
        if let Some(label) = label {
            for pass in LabelPass::candidates(label, centers, margin) {
                if pass.clear(walls, occurrence, margin, work)?
                    && reserved.map_or(Ok(true), |reserved| {
                        reserved.clear_box(pass.bridge().inflate(margin, margin), work)
                    })?
                {
                    label_passes.push(Some(pass));
                }
            }
        } else {
            label_passes.push(None);
        }
        if label_passes.is_empty() {
            return Ok(None);
        }
        let mut ends = [Vec::new(), Vec::new()];
        for (slot, node) in bodies.into_iter().enumerate() {
            let shape = &layout.shape_geometries[node as usize];
            let bounds = bounds[slot];
            let marker = edge_marker_geometry(edge, slot == 0);
            let half_width = marker.map_or(0.0, |marker| marker.routing.width / 2.0);
            let inset = (half_width + margin).max(minimum_pitch(margin) / 2.0);
            let pitch = (2.0 * half_width + margin).max(minimum_pitch(margin));
            for side in SIDES {
                let retry = reserved
                    .into_iter()
                    .flat_map(|_| retry_contacts(face_span(bounds, side), inset, pitch));
                for across in std::iter::once(None).chain(retry.map(Some)) {
                    work.charge_adapter(contact_work(shape))?;
                    if let Some(terminal) =
                        Terminal::new(shape, bounds, side, marker, margin, across)
                        && terminal.clear(walls, node, margin, work)?
                        && reserved.map_or(Ok(true), |reserved| {
                            reserved.clear(&terminal, node, margin, work)
                        })?
                    {
                        ends[slot].push(terminal);
                    }
                }
            }
        }
        let [starts, finishes] = &ends;
        // One retry samples a fixed stencil on each face. Charge even the pairs
        // discarded before scoring so crowded faces cannot bypass the work budget.
        work.charge_adapter(work.checked_mul(
            work.checked_mul(starts.len(), finishes.len())?,
            label_passes.len(),
        )?)?;
        let pair = starts
            .iter()
            .enumerate()
            .flat_map(|(a, start)| {
                let passes = &label_passes;
                finishes
                    .iter()
                    .enumerate()
                    .filter(move |(_, end)| clear_pair(start, end, margin))
                    .flat_map(move |(b, end)| {
                        passes
                            .iter()
                            .filter(move |label| {
                                label.is_none_or(|pass| {
                                    label_clears_markers(
                                        pass.bridge().inflate(margin, margin),
                                        start,
                                        end,
                                    )
                                })
                            })
                            .map(move |&label| {
                                // Facing contacts are useful only when their runways do not pass
                                // one another. Otherwise the straight painter loses its diagonal.
                                let backtracking = label.is_none()
                                    && backtracking_ends(start.anchor(), end.anchor()) > 0;
                                let away =
                                    usize::from(facing[0].is_some_and(|side| side != start.side))
                                        + usize::from(
                                            facing[1].is_some_and(|side| side != end.side),
                                        );
                                ((backtracking, away), score(start, end, label), a, b, label)
                            })
                    })
            })
            .min_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)));
        Ok(pair.map(|(_, _, a, b, label)| {
            let [mut starts, mut finishes] = ends;
            let ends = [starts.swap_remove(a), finishes.swap_remove(b)];
            Request {
                placement,
                ends,
                label,
            }
        }))
    };
    let mut requests = Vec::new();
    let mut candidate = vec![false; model.edges.len()];
    for (occurrence, edge) in model.edges.iter().enumerate() {
        work.charge_adapter(1)?;
        let (Some(&index), Some(&from), Some(&to)) = (
            edge_indices.get(edge.id.as_str()),
            bodies.get(edge.start.as_str()),
            bodies.get(edge.end.as_str()),
        ) else {
            continue;
        };
        if from != to
            && let Some(request) = choose_request(
                EdgePlacement {
                    occurrence,
                    index,
                    bodies: [from, to],
                },
                &walls,
                None,
                &mut work,
            )?
        {
            candidate[occurrence] = true;
            requests.push(request);
        }
    }
    if requests.is_empty() {
        return Ok(Vec::new());
    }
    let mut seating_walls = walls;
    let fixed_count = seating_walls.len();
    seating_walls.extend(
        fallback
            .iter()
            .zip(candidate)
            .filter(|(_, candidate)| !candidate)
            .flat_map(|(markers, _)| markers.iter().flatten().copied()),
    );
    spread_face_terminals(
        &mut requests,
        &layout.shape_geometries,
        &bounds,
        &seating_walls,
        margin,
        &mut work,
    )?;
    let mut admitted = Vec::new();
    let mut rejected = Vec::new();
    let mut reserved = Reservations::default();
    for request in requests {
        if request.clear(&seating_walls, &reserved, margin, &mut work)? {
            reserved.reserve(&request, margin, &mut work)?;
            admitted.push(request);
        } else {
            rejected.push(request);
        }
    }
    for original in rejected {
        if let Some(request) = choose_request(
            original.placement,
            &seating_walls,
            Some(&reserved),
            &mut work,
        )? {
            reserved.reserve(&request, margin, &mut work)?;
            admitted.push(request);
        } else {
            seating_walls.extend(
                fallback[original.placement.occurrence]
                    .iter()
                    .flatten()
                    .copied(),
            );
        }
    }
    // Inner branches take their short corridors first. A farther branch can then leave the
    // shared face before crossing over to its lane instead of sealing off a nearer destination.
    let levels = admitted.len().checked_ilog2().map_or(0, |n| n as usize + 1);
    work.charge_adapter(work.checked_mul(admitted.len(), levels)?)?;
    admitted.sort_unstable_by(|a, b| {
        a.score()
            .total_cmp(&b.score())
            .then_with(|| a.placement.occurrence.cmp(&b.placement.occurrence))
    });
    if admitted.is_empty() {
        return Ok(Vec::new());
    }
    work.charge_adapter(
        work.checked_add(seating_walls.len(), work.checked_mul(admitted.len(), 2)?)?,
    )?;
    let walls: Vec<_> = seating_walls
        .iter()
        .copied()
        .chain(admitted.iter().flat_map(Request::marker_walls).flatten())
        .map(|Wall { rect, .. }| Obstacle {
            x: rect.x0,
            y: rect.y0,
            width: rect.width(),
            height: rect.height(),
        })
        .collect();
    let spans: Vec<_> = admitted.iter().flat_map(Request::spans).collect();
    let mut paths = route_fixed_spans(
        &walls,
        None,
        edge_sep,
        &spans,
        route_style.routing_style(),
        &mut work,
    )
    .map_err(|error| work.map_shoal_error(error))?
    .into_iter();
    work.charge_adapter(model.edges.len())?;
    let mut solved: Vec<_> = (0..model.edges.len()).map(|_| None).collect();
    for request in admitted {
        let first = paths.next().flatten();
        let second = if request.label.is_some() {
            paths.next().flatten()
        } else {
            Some(Vec::new())
        };
        if let (Some(first), Some(second)) = (first, second) {
            work.charge_adapter(
                work.checked_add(work.checked_add(first.len(), second.len())?, 2)?,
            )?;
            let occurrence = request.placement.occurrence;
            solved[occurrence] = Some(request.solve(first.into_iter().chain(second)));
        }
    }
    fallback::restore(&mut solved, &fallback, margin, &mut work)?;
    let clearance = if matches!(route_style, RouteStyle::Rounded(_)) {
        work.charge_adapter(work.checked_add(fixed_count, work.checked_mul(solved.len(), 2)?)?)?;
        let markers = solved
            .iter()
            .zip(fallback)
            .flat_map(|(route, grid)| {
                route
                    .as_ref()
                    .map_or(grid, |route| route.request.marker_walls())
            })
            .flatten();
        let walls: Vec<_> = seating_walls[..fixed_count]
            .iter()
            .copied()
            .chain(markers)
            .map(|wall| (wall.rect, wall.owner))
            .collect();
        let levels = walls.len().checked_ilog2().map_or(0, |n| n as usize + 1);
        work.charge_adapter(work.checked_mul(walls.len(), levels)?)?;
        Some(
            Clearance::new(walls).map_err(|error| crate::Error::InvalidModel {
                message: format!("Block route clearance: {error}"),
            })?,
        )
    } else {
        None
    };
    work.charge_adapter(solved.len())?;
    let mut routes: Vec<_> = (0..model.edges.len()).map(|_| None).collect();
    let mut updates = Vec::new();
    let mut scratch = route_paint::RoutePaintScratch::default();
    for solved in solved.into_iter().flatten() {
        let placement = solved.request.placement;
        let [from, to] = placement.bodies.map(Endpoint::Node);
        let guard = clearance.as_ref().map(|clearance| {
            clearance.for_edge(
                placement.occurrence,
                (from, to),
                Vec2::ZERO,
                stroke_width / 2.0,
            )
        });
        let route = solved.paint(route_style, guard, &mut scratch)?;
        work.charge_adapter(solved.points.len())?;
        let points = solved
            .points
            .into_iter()
            .map(|point| layout_point(knot_point(point)))
            .collect();
        updates.push((placement.index, points));
        routes[placement.occurrence] = Some(route);
    }
    work.charge_adapter(updates.len())?;
    for (index, points) in updates {
        layout.edges[index].points = points;
    }
    Ok(routes)
}

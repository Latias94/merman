//! Route edges over the grid's placed shapes in drawing coordinates.

use super::edge_geometry::BlockOrthogonalRoute;
use super::{
    BlockRectangleKind, BlockShapeBoundary, BlockShapeGeometry, BlockTypographyThemePlan,
    block_label_is_effectively_empty, block_render_edge_points, decode_block_label_html,
    resolve_block_node_sources,
};
use crate::Result;
use crate::graph_layout::Marker;
use crate::graph_layout::ir::Endpoint;
use crate::graph_layout::route_paint::{
    self, Clearance, CurveTerminals, EdgeClearance, ObstacleOwner, RouteStyle,
};
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{BlockDiagramLayout, LayoutLabel, LayoutPoint};
use crate::resources::OperationWorkMeter;
use crate::svg::BaseEdgeMarkerKind;
use kurbo::{Point, Rect, Vec2};
use merman_layout_shoal::{OrthogonalSpan, Rect as Obstacle, RoutePoint, Side, route_fixed_spans};
use merman_model::families::block::{BlockDiagramRenderModel, BlockEdge};
use merman_render_structured::block::BlockEdgeMarkerGeometry;
use merman_theme_contract::v2::EdgeStyle;
use std::{collections::HashMap, sync::Arc};

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

#[derive(Clone, Copy, PartialEq)]
enum Owner {
    Body(usize),
    Marker(usize),
    NodeLabel,
    EdgeLabel(usize),
}

struct Wall {
    rect: Rect,
    owner: Owner,
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
        side: Side,
        marker: Option<BlockEdgeMarkerGeometry>,
        margin: f64,
    ) -> Option<Self> {
        let normal = normal(side);
        let bounds = shape_box(shape);
        let center = Point::new(shape.allocated.x, shape.allocated.y);
        let direction = center + normal * (bounds.width().max(bounds.height()) + 1.0);
        let contact = to_point(&shape.intersect(&layout_point(direction)));
        let face = match side {
            Side::Top => bounds.y0,
            Side::Right => bounds.x1,
            Side::Bottom => bounds.y1,
            Side::Left => bounds.x0,
        };
        let (at, sign) = match side {
            Side::Top | Side::Bottom => (contact.y, normal.y),
            _ => (contact.x, normal.x),
        };
        let marker_metrics = marker.map(|marker| marker.routing).unwrap_or_default();
        let depth = marker_metrics.trim + route_paint::LEAD.max(marker_metrics.depth);
        let anchor = contact + normal * (depth.max((face - at) * sign) + margin);
        if !contact.is_finite() || !anchor.is_finite() || contact == center || contact == anchor {
            return None;
        }
        // Preserve the lead end through collinear knot coalescing in the painter.
        Some(Self {
            contact: RoutePoint::new(contact.x, contact.y, side.axis()),
            side,
            anchor: RoutePoint::gap_boundary(anchor.x, anchor.y, side.axis()),
            reference: contact + normal * marker_metrics.trim,
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

    fn clear(
        &self,
        walls: &[Wall],
        node: usize,
        edge: usize,
        margin: f64,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<bool> {
        work.charge_adapter(walls.len())?;
        let lead = self.lead_bounds();
        let marker = self.marker_bounds();
        Ok(walls.iter().all(|wall| {
            if wall.owner == Owner::Body(node) || wall.owner == Owner::Marker(edge) {
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
        let bridge = Rect::from_points(knot_point(self.0[0].0), knot_point(self.0[1].0));
        Ok(bridge.is_finite()
            && walls.iter().all(|wall| {
                wall.owner == Owner::EdgeLabel(edge)
                    || !intersects(bridge, wall.rect.inflate(margin, margin))
            }))
    }
}

struct Request {
    index: usize,
    occurrence: usize,
    bodies: [usize; 2],
    ends: [Terminal; 2],
    label: Option<LabelPass>,
}

impl Request {
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

    fn paint(
        &self,
        inner: impl Iterator<Item = RoutePoint>,
        style: RouteStyle,
        clearance: Option<EdgeClearance<'_>>,
    ) -> Option<(Vec<LayoutPoint>, BlockOrthogonalRoute)> {
        let [a, b] = &self.ends;
        let mut points: Vec<_> = std::iter::once(a.contact)
            .chain(inner)
            .chain(std::iter::once(b.contact))
            .collect();
        points.dedup_by(|a, b| a.x == b.x && a.y == b.y);
        let marker = |end: &Terminal| -> Marker {
            end.marker.map(|marker| marker.routing).unwrap_or_default()
        };
        let paint = |style, clearance| {
            route_paint::build_svg_path(
                &points,
                (a.side, b.side),
                style,
                (marker(a), marker(b)),
                CurveTerminals::Fixed,
                &[],
                clearance,
            )
            .ok()
        };
        let path = paint(style, clearance).or_else(|| paint(RouteStyle::Linear, None))?;
        let route = BlockOrthogonalRoute {
            path: path.d,
            bounds: path.bounds,
            marker_refs: [a.reference, b.reference].map(layout_point),
        };
        Some((
            points
                .into_iter()
                .map(|k| layout_point(knot_point(k)))
                .collect(),
            route,
        ))
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

fn clear_pair(a: &Terminal, b: &Terminal) -> bool {
    if knot_point(a.contact) == knot_point(b.contact)
        || knot_point(a.anchor) == knot_point(b.anchor)
    {
        return false;
    }
    let a_marker = a.marker_bounds();
    let b_marker = b.marker_bounds();
    a_marker.is_none_or(|a_mark| {
        !intersects(b.lead_bounds(), a_mark)
            && b_marker.is_none_or(|b_mark| !intersects(a_mark, b_mark))
    }) && b_marker.is_none_or(|b_mark| !intersects(a.lead_bounds(), b_mark))
}

fn span_score((a, a_side): Anchor, (b, b_side): Anchor) -> f64 {
    let between = knot_point(b) - knot_point(a);
    // Favor the ports facing one another when several pairs have the same runway.
    between.x.abs()
        + between.y.abs()
        + 40.0
            * (f64::from(between.dot(normal(a_side)) < 0.0)
                + f64::from(between.dot(normal(b_side)) > 0.0))
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

fn terminals(
    shape: &BlockShapeGeometry,
    edge: &BlockEdge,
    start: bool,
    margin: f64,
) -> impl Iterator<Item = Terminal> {
    let marker = edge_marker_geometry(edge, start);
    [Side::Top, Side::Right, Side::Bottom, Side::Left]
        .into_iter()
        .filter_map(move |side| Terminal::new(shape, side, marker, margin))
}

fn clearance_boxes(
    model: &BlockDiagramRenderModel,
    layout: &BlockDiagramLayout,
    geometries: &HashMap<&str, &BlockShapeGeometry>,
    edge_indices: &HashMap<&str, usize>,
    edge_style: Option<EdgeStyle>,
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
                rect: shape_box(shape),
                owner: Owner::Body(index),
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
                owner: Owner::NodeLabel,
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
                owner: Owner::EdgeLabel(occurrence),
            });
        }
        // A fallback edge keeps the marker it had before routing, including diagonal variants.
        let start_marker = edge_marker_geometry(edge, true);
        let end_marker = edge_marker_geometry(edge, false);
        if start_marker.is_some() || end_marker.is_some() {
            let points =
                block_render_edge_points(edge, edge_layout, geometries, edge_style).rendered_points;
            for (marker, first, next) in [
                (start_marker, points.first(), points.get(1)),
                (end_marker, points.last(), points.iter().rev().nth(1)),
            ] {
                if let (Some(marker), Some(first), Some(next)) = (marker, first, next) {
                    let first = to_point(first);
                    let delta = to_point(next) - first;
                    if delta.hypot2() > 0.0 {
                        walls.push(Wall {
                            rect: marker_bounds(marker, first, delta.normalize()),
                            owner: Owner::Marker(occurrence),
                        });
                    }
                }
            }
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
        .map(|(i, r)| (r.id.as_str(), i))
        .collect();
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
    let route_style = edge_style.map_or(RouteStyle::Step, RouteStyle::from_theme);
    let Some(mut walls) = clearance_boxes(
        model,
        layout,
        &geometries,
        &edge_indices,
        edge_style,
        &mut work,
    )?
    else {
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
    let mut requests = Vec::new();
    for (occurrence, edge) in model.edges.iter().enumerate() {
        work.charge_adapter(1)?;
        let (Some(&index), Some(&from), Some(&to)) = (
            edge_indices.get(edge.id.as_str()),
            bodies.get(edge.start.as_str()),
            bodies.get(edge.end.as_str()),
        ) else {
            continue;
        };
        if from == to {
            continue;
        }
        let mut label_passes = Vec::new();
        if let Some(label) = &layout.edges[index].label {
            let centers = [from, to].map(|i| Point::new(layout.nodes[i].x, layout.nodes[i].y));
            for pass in LabelPass::candidates(label, centers, margin) {
                if pass.clear(&walls, occurrence, margin, &mut work)? {
                    label_passes.push(Some(pass));
                }
            }
        } else {
            label_passes.push(None);
        }
        if label_passes.is_empty() {
            continue;
        }
        let mut ends = [Vec::new(), Vec::new()];
        for (slot, node) in [from, to].into_iter().enumerate() {
            for terminal in terminals(&layout.shape_geometries[node], edge, slot == 0, margin) {
                if terminal.clear(&walls, node, occurrence, margin, &mut work)? {
                    ends[slot].push(terminal);
                }
            }
        }
        let [starts, finishes] = &ends;
        let pair = starts
            .iter()
            .enumerate()
            .flat_map(|(a, start)| {
                let passes = &label_passes;
                finishes
                    .iter()
                    .enumerate()
                    .filter(move |(_, end)| clear_pair(start, end))
                    .flat_map(move |(b, end)| {
                        passes
                            .iter()
                            .map(move |&label| (score(start, end, label), a, b, label))
                    })
            })
            .min_by(|a, b| a.0.total_cmp(&b.0));
        if let Some((_, a, b, label)) = pair {
            let [mut starts, mut finishes] = ends;
            let ends = [starts.swap_remove(a), finishes.swap_remove(b)];
            for rect in ends.iter().filter_map(Terminal::marker_bounds) {
                walls.push(Wall {
                    rect,
                    owner: Owner::Marker(occurrence),
                });
            }
            requests.push(Request {
                index,
                occurrence,
                bodies: [from, to],
                ends,
                label,
            });
        }
    }
    if requests.is_empty() {
        return Ok(Vec::new());
    }
    // Include both old and new marker positions so a per-edge fallback cannot invalidate a peer.
    // Check the earlier leads again now that later edges have chosen their marker positions.
    let mut admitted = Vec::new();
    for request in requests {
        let [a, b] = &request.ends;
        let [from, to] = request.bodies;
        if a.clear(&walls, from, request.occurrence, margin, &mut work)?
            && b.clear(&walls, to, request.occurrence, margin, &mut work)?
        {
            if let Some(label) = request.label
                && !label.clear(&walls, request.occurrence, margin, &mut work)?
            {
                continue;
            }
            admitted.push(request);
        }
    }
    work.charge_adapter(work.checked_add(walls.len(), work.checked_mul(admitted.len(), 2)?)?)?;
    let clearance = if route_style == RouteStyle::Curved {
        let levels = walls.len().checked_ilog2().map_or(0, |n| n as usize + 1);
        work.charge_adapter(work.checked_mul(walls.len(), levels)?)?;
        let mut obstacles = Vec::with_capacity(walls.len());
        for wall in &walls {
            let owner = match wall.owner {
                Owner::Body(index) => ObstacleOwner::Node(
                    u32::try_from(index).map_err(|_| work.record_arithmetic_overflow())?,
                ),
                Owner::NodeLabel => ObstacleOwner::GroupTitle(0),
                Owner::EdgeLabel(index) | Owner::Marker(index) => ObstacleOwner::EdgeLabel(index),
            };
            obstacles.push((wall.rect, owner));
        }
        Some(
            Clearance::new(obstacles).map_err(|error| crate::Error::InvalidModel {
                message: format!("Block route clearance: {error}"),
            })?,
        )
    } else {
        None
    };
    let walls: Vec<_> = walls
        .into_iter()
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
    let mut routes: Vec<_> = (0..model.edges.len()).map(|_| None).collect();
    let mut updates = Vec::new();
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
            let [from, to] = request.bodies.map(|index| {
                u32::try_from(index)
                    .map(Endpoint::Node)
                    .map_err(|_| work.record_arithmetic_overflow())
            });
            let endpoints = (from?, to?);
            let guard = clearance.as_ref().map(|clearance| {
                clearance.for_edge(
                    request.occurrence,
                    endpoints,
                    Vec2::ZERO,
                    stroke_width / 2.0,
                )
            });
            if let Some((points, route)) =
                request.paint(first.into_iter().chain(second), route_style, guard)
            {
                updates.push((request.index, points));
                routes[request.occurrence] = Some(route);
            }
        }
    }
    work.charge_adapter(updates.len())?;
    for (index, points) in updates {
        layout.edges[index].points = points;
    }
    Ok(routes)
}

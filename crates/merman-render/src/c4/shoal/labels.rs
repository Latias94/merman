use std::ops::ControlFlow::{Break, Continue};

use kurbo::{BezPath, Line, ParamCurve, ParamCurveNearest, PathSeg, Point, Rect, Shape, Vec2};
use merman_layout_shoal::geom::{Bounds, BoundsIndex, Bvh, BvhEntry};
use merman_render_core::text::line_break::split_mermaid_common_br_lines;
use merman_render_graph::c4::{C4TextBlockLayout, SHAFT_CLEARANCE, measure_c4_relation_label};

use super::super::{C4Measurement, c4_relation_markers};
use crate::Result;
use crate::graph_layout::canonical::CanonicalPaintedEdge;
use crate::graph_layout::ir::Size;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{Bounds as DiagramBounds, C4RelLayout, LayoutPoint};

// A substituted font can move the visible center within the reserved ink clearance.
// That displacement may increase the host distance and decrease the neighbor distance.
const OWNERSHIP_CLEARANCE: f64 = 2.0 * SHAFT_CLEARANCE;

#[derive(Debug)]
pub(super) struct PlacedRelationLabel {
    pub(super) origin: LayoutPoint,
    pub(super) ink: Rect,
    cutout: Rect,
    pub(super) reference: Option<Box<C4TextBlockLayout>>,
}

impl PlacedRelationLabel {
    fn translate(&mut self, shift: Vec2) {
        self.ink = self.ink + shift;
        self.cutout = self.cutout + shift;
        self.origin.x += shift.x;
        self.origin.y += shift.y;
    }
}

/// Bounds relative to the common SVG text origin. Authored offsets are limited to the
/// local caption allowance so a relationship can still use the compact grid's gutters.
pub(super) struct RelationLabel {
    ink: Rect,
    cutout: Rect,
    offset: Vec2,
}

impl RelationLabel {
    pub(super) fn prepare(rel: &mut C4RelLayout, ctx: &C4Measurement<'_>) -> Result<Option<Self>> {
        Ok(
            measure_c4_relation_label(rel, ctx)?.map(|(cutout, offset)| Self {
                ink: ctx.typography.routed_caption_footprint(cutout),
                cutout,
                offset,
            }),
        )
    }

    pub(super) fn size(&self, shaft_outset: f64) -> Size {
        let [side_room, vertical_room] = self.room();
        Size {
            width: self.ink.width() + 2.0 * (side_room + shaft_outset),
            height: self.ink.height() + 2.0 * (vertical_room + shaft_outset),
        }
    }

    fn room(&self) -> [f64; 2] {
        [
            self.offset.x.abs().min(SHAFT_CLEARANCE),
            self.offset.y.abs().min(SHAFT_CLEARANCE),
        ]
    }

    pub(super) fn place(&self, center: Point) -> PlacedRelationLabel {
        let [x, y] = self.room();
        let origin = center - self.ink.center().to_vec2()
            + Vec2::new(self.offset.x.clamp(-x, x), self.offset.y.clamp(-y, y));
        PlacedRelationLabel {
            origin: LayoutPoint {
                x: origin.x,
                y: origin.y,
            },
            ink: self.ink + origin.to_vec2(),
            cutout: self.cutout + origin.to_vec2(),
            reference: None,
        }
    }
}

struct ShaftSegment {
    path: PathSeg,
    bounds: Rect,
}

impl ShaftSegment {
    fn new(path: PathSeg) -> Self {
        Self {
            bounds: path.bounding_box(),
            path,
        }
    }

    fn intersects(&self, rect: Rect) -> bool {
        if !rects_touch(self.bounds, rect) {
            return false;
        }
        if rect.contains(self.path.start()) || rect.contains(self.path.end()) {
            return true;
        }
        if let PathSeg::Line(line) = self.path
            && (line.p0.x == line.p1.x || line.p0.y == line.p1.y)
        {
            // For a straight axis-aligned leg the bounds test is exact, including contact.
            return true;
        }
        let corners = [
            Point::new(rect.x0, rect.y0),
            Point::new(rect.x1, rect.y0),
            Point::new(rect.x1, rect.y1),
            Point::new(rect.x0, rect.y1),
        ];
        (0..4).any(|i| {
            !self
                .path
                .intersect_line(Line::new(corners[i], corners[(i + 1) % 4]))
                .is_empty()
        })
    }

    fn side_offset(&self, preferred: Point, anchor: Point, dx: f64, dy: f64) -> Vec2 {
        let normal = if self.bounds.width() >= self.bounds.height() {
            Vec2::new(0.0, dy)
        } else {
            Vec2::new(dx, 0.0)
        };
        if (preferred - anchor).dot(normal) < 0.0 {
            -normal
        } else {
            normal
        }
    }
}

pub(super) struct PaintedShaft {
    bounds: Rect,
    segments: Vec<ShaftSegment>,
    markers: [Option<Rect>; 2],
}

impl PaintedShaft {
    pub(super) fn from_segments(
        route: &CanonicalPaintedEdge,
        segments: &[PathSeg],
        rough: Option<&BezPath>,
        markers: &[Option<DiagramBounds>; 2],
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Self> {
        let capacity = rough.map_or(segments.len(), |path| path.elements().len());
        work.charge_adapter(capacity)?;
        let mut indexed = Vec::new();
        indexed
            .try_reserve_exact(capacity)
            .map_err(|_| work.map_shoal_error(merman_layout_shoal::LayoutError::AllocationLimit))?;
        if let Some(rough) = rough {
            indexed.extend(rough.segments().map(ShaftSegment::new));
        } else {
            indexed.extend(segments.iter().copied().map(ShaftSegment::new));
        }
        let bounds = &route.path.bounds;
        Ok(Self {
            bounds: Rect::new(bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y),
            segments: indexed,
            markers: markers.each_ref().map(|marker| {
                marker
                    .as_ref()
                    .map(|marker| Rect::new(marker.min_x, marker.min_y, marker.max_x, marker.max_y))
            }),
        })
    }
}

pub(super) struct ShaftIndex<'a> {
    shafts: &'a [PaintedShaft],
    index: Bvh<usize>,
    stroke_outset: f64,
}

impl<'a> ShaftIndex<'a> {
    pub(super) fn new(
        shafts: &'a [PaintedShaft],
        stroke_outset: f64,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Self> {
        work.charge_adapter(shafts.len())?;
        let mut indices = Vec::new();
        indices
            .try_reserve_exact(shafts.len())
            .map_err(|_| work.map_shoal_error(merman_layout_shoal::LayoutError::AllocationLimit))?;
        indices.extend(0..shafts.len());
        let index = build_index(
            indices,
            |&edge| {
                let shaft = &shafts[edge];
                shaft
                    .markers
                    .iter()
                    .flatten()
                    .copied()
                    .fold(shaft.bounds, |bounds, marker| bounds.union(marker))
            },
            work,
        )?;
        Ok(Self {
            shafts,
            index,
            stroke_outset,
        })
    }

    pub(super) fn intersects(
        &self,
        area: Rect,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<bool> {
        let guard = area.inflate(self.stroke_outset, self.stroke_outset);
        self.blocks(guard, area, None, work, |segment, _| {
            Ok(segment.intersects(guard))
        })
    }

    fn blocks(
        &self,
        region: Rect,
        ink: Rect,
        owner: Option<usize>,
        work: &mut OperationLayoutWorkControl,
        mut blocks: impl FnMut(&ShaftSegment, &mut OperationLayoutWorkControl) -> Result<bool>,
    ) -> Result<bool> {
        Ok(self
            .index
            .visit(|entry| -> Result<_> {
                work.charge_adapter(1)?;
                let edge = match entry {
                    BvhEntry::Node(bounds) => {
                        return Ok(Continue(rects_touch(region, rect(bounds))));
                    }
                    BvhEntry::Item(&edge) => edge,
                };
                let shaft = &self.shafts[edge];
                // The owner's shaft may be clipped. Its endpoint markers must stay visible.
                if shaft
                    .markers
                    .iter()
                    .flatten()
                    .any(|&marker| ink.intersect(marker).area() > 0.0)
                {
                    return Ok(Break(()));
                }
                if Some(edge) != owner && rects_touch(region, shaft.bounds) {
                    work.charge_adapter(shaft.segments.len())?;
                    for segment in &shaft.segments {
                        if rects_touch(region, segment.bounds) && blocks(segment, work)? {
                            return Ok(Break(()));
                        }
                    }
                }
                Ok(Continue(true))
            })?
            .is_break())
    }

    fn host_distance(
        &self,
        edge: usize,
        center: Point,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<f64> {
        let segments = &self.shafts[edge].segments;
        work.charge_adapter(segments.len())?;
        Ok(segments
            .iter()
            .map(|segment| segment.path.nearest(center, 1e-3).distance_sq)
            .fold(f64::INFINITY, f64::min)
            .sqrt())
    }

    fn blocks_caption(
        &self,
        edge: usize,
        caption: Rect,
        anchor_distance: Option<f64>,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<bool> {
        let center = caption.center();
        let guard = caption.inflate(self.stroke_outset, self.stroke_outset);
        let mut exact = anchor_distance.is_none();
        let mut reach = match anchor_distance {
            Some(distance) => distance,
            None => self.host_distance(edge, center, work)?,
        } + OWNERSHIP_CLEARANCE;
        if !reach.is_finite() {
            return Ok(true);
        }
        let region = guard.union(Rect::from_center_size(center, (2.0 * reach, 2.0 * reach)));
        self.blocks(region, caption, Some(edge), work, |segment, work| {
            if segment.intersects(guard) {
                return Ok(true);
            }
            let vicinity = Rect::from_center_size(center, (2.0 * reach, 2.0 * reach));
            if !rects_touch(vicinity, segment.bounds) {
                return Ok(false);
            }
            let distance = segment.path.nearest(center, 1e-3).distance_sq;
            if distance > reach * reach {
                return Ok(false);
            }
            // The probe's anchor bounds the owner distance. Measure the exact minimum only
            // when a rival could otherwise reject this seat.
            if !exact {
                reach = self.host_distance(edge, center, work)? + OWNERSHIP_CLEARANCE;
                exact = true;
            }
            Ok(distance <= reach * reach)
        })
    }

    fn own_cutout(
        &self,
        edge: usize,
        cutout: Rect,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Option<Rect>> {
        let guard = cutout.inflate(self.stroke_outset, self.stroke_outset);
        let shaft = &self.shafts[edge];
        if !rects_touch(shaft.bounds, guard) {
            return Ok(None);
        }
        work.charge_adapter(shaft.segments.len())?;
        Ok(shaft
            .segments
            .iter()
            .any(|segment| segment.intersects(guard))
            .then_some(cutout))
    }
}

pub(super) fn reseat(
    routes: &mut [super::PaintedRelation],
    shafts: &ShaftIndex<'_>,
    obstacles: Vec<Rect>,
    horizontal: Option<[f64; 2]>,
    work: &mut OperationLayoutWorkControl,
) -> Result<bool> {
    let hard = build_index(obstacles, |rect| *rect, work)?;
    work.charge_adapter(routes.len())?;
    let mut needs_seat = false;
    let mut placed = BoundsIndex::default();
    for (edge, route) in routes.iter_mut().enumerate() {
        if let Some(label) = &route.label {
            route.seated = route.seated
                && horizontal
                    .is_none_or(|[left, right]| left <= label.ink.x0 && label.ink.x1 <= right)
                && hard_overlap(&hard, label.ink, work)?.is_none()
                && !shafts.blocks_caption(edge, label.ink, None, work)?;
            if route.seated && horizontal.is_some() {
                let ink = bounds(label.ink);
                route.seated = !placed
                    .any_overlap(ink, work)
                    .map_err(|error| work.map_shoal_error(error))?;
                if route.seated {
                    placed
                        .insert(ink, work)
                        .map_err(|error| work.map_shoal_error(error))?;
                }
            }
            route.shaft_cutout = if route.seated {
                shafts.own_cutout(edge, label.cutout, work)?
            } else {
                None
            };
            needs_seat |= !route.seated;
        }
    }
    if !needs_seat {
        return Ok(false);
    }

    let mut occupied_captions = Vec::new();
    if horizontal.is_none() {
        work.charge_adapter(routes.len())?;
        occupied_captions
            .try_reserve_exact(routes.len())
            .map_err(|_| work.map_shoal_error(merman_layout_shoal::LayoutError::AllocationLimit))?;
        occupied_captions.extend(
            routes
                .iter()
                .filter(|route| route.seated)
                .filter_map(|route| route.label.as_ref().map(|label| label.ink)),
        );
    }
    let soft = build_index(occupied_captions, |rect| *rect, work)?;
    let mut exterior = false;
    for (edge, route) in routes.iter_mut().enumerate() {
        work.charge_adapter(1)?;
        let Some(label) = route.label.as_mut().filter(|_| !route.seated) else {
            continue;
        };
        let scene = SeatScene {
            hard: &hard,
            soft: &soft,
            placed: &placed,
            shafts,
            horizontal,
        };
        let Some(shift) = scene.find_seat(edge, label.ink, work)? else {
            exterior = true;
            continue;
        };
        label.translate(shift);
        route.seated = true;
        placed
            .insert(bounds(label.ink), work)
            .map_err(|error| work.map_shoal_error(error))?;
        route.shaft_cutout = shafts.own_cutout(edge, label.cutout, work)?;
    }
    Ok(exterior)
}

/// Keep unseated captions outside all painted shafts and already placed captions. Their
/// visible endpoint reference and original relation ID identify the host at this distance.
pub(super) fn place_exterior(
    routes: &mut [super::PaintedRelation],
    relations: &[C4RelLayout],
    diagram: &DiagramBounds,
    horizontal: Option<[f64; 2]>,
    measure: &C4Measurement<'_>,
    work: &mut OperationLayoutWorkControl,
) -> Result<()> {
    work.charge_adapter(work.checked_mul(routes.len(), 2)?)?;
    let max_width = horizontal.map(|[left, right]| right - left);
    let reference_ink = |reference: &mut C4RelLayout| -> Result<Option<Rect>> {
        Ok(measure_c4_relation_label(reference, measure)?
            .map(|(cutout, _)| measure.typography.routed_caption_footprint(cutout)))
    };
    let mut bottom = diagram.max_y;
    for route in routes.iter().filter(|route| route.seated) {
        if let Some(label) = &route.label {
            bottom = bottom.max(label.ink.y1);
        }
    }
    for (index, (route, relation)) in routes.iter_mut().zip(relations).enumerate() {
        let Some(label) = route.label.as_mut().filter(|_| !route.seated) else {
            continue;
        };
        let arrow = match c4_relation_markers(&relation.rel_type) {
            (true, true) => "↔",
            (true, false) => "←",
            (false, true) => "→",
            (false, false) => "—",
        };
        work.charge_adapter(work.checked_add(relation.from.len(), relation.to.len())?)?;
        let mut reference = relation.clone();
        reference.techn = None;
        reference.label.text = format!("{} {arrow} {} ({})", relation.from, relation.to, index + 1);
        reference.label.line_count = split_mermaid_common_br_lines(&reference.label.text).len();
        let mut ink = reference_ink(&mut reference)?;
        if let Some(width) = max_width
            && ink.is_some_and(|ink| ink.width() > width)
        {
            reference.label.text = measure.wrap_routed_reference(&reference.label.text, width)?;
            reference.label.line_count = split_mermaid_common_br_lines(&reference.label.text).len();
            ink = reference_ink(&mut reference)?;
        }
        if let Some(ink) = ink {
            let y = label.ink.y0 - SHAFT_CLEARANCE - ink.y1;
            reference.label.y = y - label.origin.y;
            label.ink = label.ink.union(ink + Vec2::new(label.origin.x, y));
            label.reference = Some(Box::new(reference.label));
        }
        let x = horizontal.map_or(diagram.min_x, |[left, right]| {
            diagram
                .min_x
                .clamp(left, (right - label.ink.width()).max(left))
        });
        label.translate(Vec2::new(
            x - label.ink.x0,
            bottom + SHAFT_CLEARANCE - label.ink.y0,
        ));
        bottom = label.ink.y1;
        route.seated = true;
        route.shaft_cutout = None;
    }
    Ok(())
}

fn build_index<T>(
    items: Vec<T>,
    extent: impl Fn(&T) -> Rect,
    work: &mut OperationLayoutWorkControl,
) -> Result<Bvh<T>> {
    Bvh::build_controlled(items, |item| bounds(extent(item)), work)
        .map_err(|error| work.map_shoal_error(error))
}

struct SeatScene<'a> {
    hard: &'a Bvh<Rect>,
    soft: &'a Bvh<Rect>,
    placed: &'a BoundsIndex,
    shafts: &'a ShaftIndex<'a>,
    horizontal: Option<[f64; 2]>,
}

impl SeatScene<'_> {
    fn find_seat(
        &self,
        edge: usize,
        ink: Rect,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Option<Vec2>> {
        let segments = &self.shafts.shafts[edge].segments;
        let preferred = ink.center();
        let clearance = self.shafts.stroke_outset + SHAFT_CLEARANCE;
        let dx = ink.width() / 2.0 + clearance;
        let dy = ink.height() / 2.0 + clearance;
        let x_range = self
            .horizontal
            .map(|[left, right]| [left - ink.x0, right - ink.x1]);
        if x_range.is_some_and(|[lo, hi]| lo > hi) {
            return Ok(None);
        }
        let mut best = None::<(f64, Vec2)>;
        let mut blocker: Option<Rect> = None;
        let mut probe = |anchor: Point,
                         offset: Vec2,
                         best: &mut Option<(f64, Vec2)>,
                         work: &mut OperationLayoutWorkControl| {
            work.charge_adapter(1)?;
            let mut shift = anchor + offset - preferred;
            let host_distance = if let Some([lo, hi]) = x_range {
                shift.x = shift.x.clamp(lo, hi);
                (preferred + shift - anchor).hypot()
            } else {
                offset.hypot()
            };
            let caption = ink + shift;
            if !shift.is_finite()
                || blocker.is_some_and(|rect| caption.intersect(rect).area() > 0.0)
            {
                return Ok(None);
            }
            blocker = hard_overlap(self.hard, caption, work)?;
            if blocker.is_some() {
                return Ok(None);
            }
            let overlap = self.soft_overlap(caption, work)?;
            if x_range.is_some() && overlap > 0.0 {
                return Ok(None);
            }
            let score = (overlap, shift.hypot2());
            if best.is_some_and(|(area, old)| score >= (area, old.hypot2()))
                || self
                    .shafts
                    .blocks_caption(edge, caption, Some(host_distance), work)?
            {
                return Ok(None);
            }
            *best = Some((overlap, shift));
            Ok::<_, crate::Error>((overlap == 0.0).then_some(shift))
        };

        // Most marginal seats can stay on their original run. Probe it before sweeping
        // the whole relationship, and stop as soon as a caption is clear of its peers.
        work.charge_adapter(segments.len())?;
        let nearest = segments
            .iter()
            .map(|segment| (segment, segment.path.nearest(preferred, 1e-3)))
            .min_by(|(_, a), (_, b)| a.distance_sq.total_cmp(&b.distance_sq));
        if let Some((segment, nearest)) = nearest {
            let anchor = segment.path.eval(nearest.t);
            let shoulder = segment.side_offset(preferred, anchor, dx, dy) / 2.0;
            for offset in [shoulder, Vec2::ZERO, -shoulder] {
                if let Some(shift) = probe(anchor, offset, &mut best, work)? {
                    return Ok(Some(shift));
                }
            }
        }

        // Search on and just off each run before trying positions beside the shaft. The
        // running score keeps a soft-overlap fallback without a second traversal or sort.
        for scale in [0.5, 1.0] {
            work.charge_adapter(segments.len())?;
            for segment in segments {
                let along = segment.bounds.size();
                let (run, half_with_clearance) = if along.width >= along.height {
                    (along.width, dx)
                } else {
                    (along.height, dy)
                };
                let inset = (half_with_clearance / run).min(0.5);
                let near = segment.path.nearest(preferred, 1e-3).t;
                let stops = [
                    near.clamp(inset, 1.0 - inset),
                    0.5,
                    inset,
                    1.0 - inset,
                    0.0,
                    1.0,
                ];
                for (index, &t) in stops.iter().enumerate() {
                    if stops[..index].contains(&t) {
                        continue;
                    }
                    let anchor = segment.path.eval(t);
                    let side = scale * segment.side_offset(preferred, anchor, dx, dy);
                    let offsets = [side, -side, Vec2::ZERO];
                    let offsets = if scale == 0.5 {
                        &offsets[..]
                    } else {
                        &offsets[..2]
                    };
                    for &offset in offsets {
                        if let Some(shift) = probe(anchor, offset, &mut best, work)? {
                            return Ok(Some(shift));
                        }
                    }
                }
                if scale == 0.5 && x_range.is_some() {
                    let inline_inset = ((half_with_clearance - SHAFT_CLEARANCE) / run).min(0.5);
                    for t in [inline_inset, 1.0 - inline_inset] {
                        if !stops.contains(&t)
                            && let Some(shift) =
                                probe(segment.path.eval(t), Vec2::ZERO, &mut best, work)?
                        {
                            return Ok(Some(shift));
                        }
                    }
                }
            }
        }
        // A tight bend can leave a clear pocket diagonally beyond its elbow, even when
        // every perpendicular probe is blocked. Restrict those probes to the run ends.
        if best.is_none() {
            work.charge_adapter(segments.len())?;
            for segment in segments {
                for anchor in [segment.path.start(), segment.path.end()] {
                    for offset in [-dx, dx]
                        .into_iter()
                        .flat_map(|x| [-dy, dy].map(|y| Vec2::new(x, y)))
                    {
                        if let Some(shift) = probe(anchor, offset, &mut best, work)? {
                            return Ok(Some(shift));
                        }
                    }
                }
            }
        }
        Ok(best.map(|(_, shift)| shift))
    }

    fn soft_overlap(&self, caption: Rect, work: &mut OperationLayoutWorkControl) -> Result<f64> {
        let mut area = 0.0;
        let _ = self.soft.visit(|entry| -> Result<_> {
            work.charge_adapter(1)?;
            Ok(match entry {
                BvhEntry::Node(bounds) => Continue(rects_touch(caption, rect(bounds))),
                BvhEntry::Item(&peer) => {
                    area += caption.intersect(peer).area();
                    Continue(true)
                }
            })
        })?;
        Ok(area
            + self
                .placed
                .overlap_area(bounds(caption), work)
                .map_err(|error| work.map_shoal_error(error))?)
    }
}

fn hard_overlap(
    hard: &Bvh<Rect>,
    caption: Rect,
    work: &mut OperationLayoutWorkControl,
) -> Result<Option<Rect>> {
    let mut blocker = None;
    let _ = hard.visit(|entry| -> Result<_> {
        work.charge_adapter(1)?;
        Ok(match entry {
            BvhEntry::Node(bounds) => Continue(rects_touch(caption, rect(bounds))),
            BvhEntry::Item(&obstacle) if caption.intersect(obstacle).area() > 0.0 => {
                blocker = Some(obstacle);
                Break(())
            }
            BvhEntry::Item(_) => Continue(true),
        })
    })?;
    Ok(blocker)
}

fn rect(bounds: Bounds) -> Rect {
    Rect::new(bounds.x[0], bounds.y[0], bounds.x[1], bounds.y[1])
}

fn bounds(rect: Rect) -> Bounds {
    Bounds {
        x: [rect.x0, rect.x1],
        y: [rect.y0, rect.y1],
    }
}

fn rects_touch(a: Rect, b: Rect) -> bool {
    a.x0 <= b.x1 && b.x0 <= a.x1 && a.y0 <= b.y1 && b.y0 <= a.y1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_layout::route_paint::PaintedRoute;
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
    use std::sync::Arc;

    #[test]
    fn c4_cutout_checks_curve_geometry_and_axis_contact() {
        let elbow = ShaftSegment::new(PathSeg::Quad(kurbo::QuadBez::new(
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 10.0),
        )));
        assert!(!elbow.intersects(Rect::new(0.0, 8.0, 2.0, 10.0)));
        assert!(elbow.intersects(Rect::new(6.0, 2.0, 9.0, 3.0)));
        let line = ShaftSegment::new(PathSeg::Line(Line::new((-5.0, 0.0), (5.0, 0.0))));
        assert!(line.intersects(Rect::new(-2.0, 0.0, 2.0, 1.0)));
    }

    fn constrained_work() -> OperationLayoutWorkControl {
        OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::constrained(),
        )))
    }

    fn horizontal_shaft(y: f64) -> PaintedShaft {
        let segment = ShaftSegment::new(PathSeg::Line(Line::new((0.0, y), (10.0, y))));
        PaintedShaft {
            bounds: segment.bounds,
            segments: vec![segment],
            markers: [None; 2],
        }
    }

    fn initial_caption(ink: Rect) -> super::super::PaintedRelation {
        super::super::PaintedRelation {
            shaft: CanonicalPaintedEdge {
                path: PaintedRoute {
                    d: "M0,0L10,0".into(),
                    bounds: DiagramBounds {
                        min_x: 0.0,
                        min_y: 0.0,
                        max_x: 10.0,
                        max_y: 0.0,
                    },
                },
                marker_points: std::array::from_fn(|_| LayoutPoint { x: 0.0, y: 0.0 }),
                data_points: Vec::new(),
                markers: Default::default(),
            },
            label: Some(PlacedRelationLabel {
                origin: LayoutPoint { x: 0.0, y: 0.0 },
                ink,
                cutout: ink,
                reference: None,
            }),
            seated: true,
            shaft_cutout: None,
        }
    }

    #[test]
    fn c4_cutout_uses_only_the_shaft_owner() {
        let ink = Rect::new(2.0, -2.0, 6.0, 2.0);
        let paths = [horizontal_shaft(0.0), horizontal_shaft(10.0)];
        let mut policy = RenderResourcePolicy::constrained();
        policy
            .apply_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        let mut limited =
            OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(policy)));
        assert!(ShaftIndex::new(&paths, 0.5, &mut limited).is_err());

        let mut work = constrained_work();
        let shafts = ShaftIndex::new(&paths, 0.5, &mut work).unwrap();
        assert_eq!(shafts.own_cutout(0, ink, &mut work).unwrap(), Some(ink));
        assert_eq!(shafts.own_cutout(1, ink, &mut work).unwrap(), None);
    }

    #[test]
    fn c4_initial_caption_nearer_a_foreign_shaft_is_reseated() {
        let paths = [horizontal_shaft(0.0), horizontal_shaft(9.0)];
        let mut work = constrained_work();
        let shafts = ShaftIndex::new(&paths, 0.5, &mut work).unwrap();
        let ink = Rect::new(2.0, 4.0, 6.0, 6.0);
        let mut routes = [initial_caption(ink)];
        assert!(!reseat(&mut routes, &shafts, Vec::new(), None, &mut work).unwrap());
        let caption = routes[0].label.as_ref().unwrap().ink;
        assert_ne!(caption, ink);
        assert!(!shafts.blocks_caption(0, caption, None, &mut work).unwrap());
    }

    #[test]
    fn c4_initial_seat_blocked_by_a_hard_obstacle_moves_or_uses_exterior() {
        let paths = [horizontal_shaft(0.0)];
        let mut work = constrained_work();
        let shafts = ShaftIndex::new(&paths, 0.5, &mut work).unwrap();
        let ink = Rect::new(3.0, -1.0, 7.0, 1.0);
        let obstacle = Rect::new(2.0, -4.0, 9.0, 4.0);
        let mut routes = [initial_caption(ink)];
        assert!(!reseat(&mut routes, &shafts, vec![obstacle], None, &mut work).unwrap());
        let caption = routes[0].label.as_ref().unwrap().ink;
        assert_eq!(caption.intersect(obstacle).area(), 0.0);

        let wall = Rect::new(-30.0, -10.0, 40.0, 10.0);
        let mut routes = [initial_caption(ink)];
        assert!(reseat(&mut routes, &shafts, vec![wall], None, &mut work).unwrap());
        assert!(!routes[0].seated);
        assert!(routes[0].shaft_cutout.is_none());
    }

    #[test]
    fn c4_initial_caption_cannot_cover_its_own_marker() {
        let marker = Rect::new(4.0, -3.0, 9.0, 3.0);
        let mut path = horizontal_shaft(0.0);
        path.markers[1] = Some(marker);
        let paths = [path];
        let mut work = constrained_work();
        let shafts = ShaftIndex::new(&paths, 0.5, &mut work).unwrap();
        let mut routes = [initial_caption(Rect::new(3.0, -1.0, 7.0, 1.0))];
        assert!(!reseat(&mut routes, &shafts, Vec::new(), None, &mut work).unwrap());
        let caption = routes[0].label.as_ref().unwrap();
        assert_eq!(caption.ink.intersect(marker).area(), 0.0);
    }

    #[test]
    fn c4_soft_overlap_fallback_keeps_hard_clearance() {
        let mut work = constrained_work();
        let paths = [horizontal_shaft(0.0), horizontal_shaft(-4.5)];
        let shafts = ShaftIndex::new(&paths, 0.5, &mut work).unwrap();
        let obstacle = Rect::new(-30.0, -20.0, 30.0, 0.5);
        let hard = build_index(vec![obstacle], |rect| *rect, &mut work).unwrap();
        let peer = Rect::new(-30.0, 0.5, 30.0, 4.0);
        let soft = build_index(vec![peer], |rect| *rect, &mut work).unwrap();
        let mut placed = BoundsIndex::default();
        placed
            .insert(bounds(Rect::new(4.0, 1.0, 6.0, 3.0)), &mut work)
            .unwrap();
        let scene = SeatScene {
            hard: &hard,
            soft: &soft,
            placed: &placed,
            shafts: &shafts,
            horizontal: None,
        };
        let ink = Rect::new(3.0, -1.0, 7.0, 1.0);
        let caption = ink + scene.find_seat(0, ink, &mut work).unwrap().unwrap();
        assert_eq!(caption.intersect(obstacle).area(), 0.0);
        assert_eq!(caption.center().y, 3.5);
        assert_eq!(scene.soft_overlap(caption, &mut work).unwrap(), 6.0);

        let bounded = SeatScene {
            horizontal: Some([0.0, 10.0]),
            ..scene
        };
        assert!(bounded.find_seat(0, ink, &mut work).unwrap().is_none());
    }

    #[test]
    fn c4_overlap_fallback_keeps_the_ownership_clearance() {
        let mut work = constrained_work();
        let paths = [horizontal_shaft(0.0), horizontal_shaft(-6.0)];
        let shafts = ShaftIndex::new(&paths, 0.5, &mut work).unwrap();
        let peer = Rect::new(-30.0, -2.7, 40.0, 7.0);
        let soft = build_index(vec![peer], |rect| *rect, &mut work).unwrap();
        let scene = SeatScene {
            hard: &build_index(Vec::new(), |rect| *rect, &mut work).unwrap(),
            soft: &soft,
            placed: &BoundsIndex::default(),
            shafts: &shafts,
            horizontal: None,
        };
        let ink = Rect::new(3.0, -3.0, 7.0, -2.5);
        let caption = ink + scene.find_seat(0, ink, &mut work).unwrap().unwrap();
        let center = caption.center();
        let own = shafts.host_distance(0, center, &mut work).unwrap();
        let foreign = shafts.host_distance(1, center, &mut work).unwrap();
        assert!(caption.intersect(peer).area() > 0.0);
        assert!(foreign > own + OWNERSHIP_CLEARANCE);
    }
}

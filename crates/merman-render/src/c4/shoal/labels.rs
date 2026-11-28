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
    pub(super) reference: Option<Box<C4TextBlockLayout>>,
}

impl PlacedRelationLabel {
    fn translate(&mut self, shift: Vec2) {
        self.ink = self.ink + shift;
        self.origin.x += shift.x;
        self.origin.y += shift.y;
    }
}

/// Bounds relative to the common SVG text origin. Authored offsets are limited to the
/// local caption allowance so a relationship can still use the compact grid's gutters.
pub(super) struct RelationLabel {
    ink: Rect,
    offset: Vec2,
}

impl RelationLabel {
    pub(super) fn prepare(rel: &mut C4RelLayout, ctx: &C4Measurement<'_>) -> Result<Option<Self>> {
        Ok(measure_c4_relation_label(rel, ctx)?.map(|(ink, offset)| Self { ink, offset }))
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
            reference: None,
        }
    }
}

pub(super) struct CaptionInk {
    index: Bvh<CaptionRegion>,
    pub(super) occluded: Vec<bool>,
}

struct CaptionRegion {
    edge: usize,
    ink: Rect,
    vicinity: Rect,
    reach_squared: f64,
}

impl CaptionInk {
    pub(super) fn new(
        labels: impl ExactSizeIterator<Item = Option<Rect>>,
        shafts: &[PaintedShaft],
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Self> {
        let len = labels.len();
        work.charge_adapter(len)?;
        let allocation =
            |_| work.map_shoal_error(merman_layout_shoal::LayoutError::AllocationLimit);
        let mut ink = Vec::new();
        ink.try_reserve_exact(len).map_err(allocation)?;
        let mut occluded = Vec::new();
        occluded.try_reserve_exact(len).map_err(allocation)?;
        occluded.resize(len, false);
        for (edge, (label, shaft)) in labels.zip(shafts).enumerate() {
            let Some(label) = label else { continue };
            work.charge_adapter(shaft.segments.len())?;
            let center = label.center();
            let reach = shaft
                .segments
                .iter()
                .map(|segment| segment.path.nearest(center, 1e-3).distance_sq)
                .fold(f64::INFINITY, f64::min)
                .sqrt()
                + OWNERSHIP_CLEARANCE;
            if !reach.is_finite() {
                occluded[edge] = true;
                continue;
            }
            ink.push(CaptionRegion {
                edge,
                ink: label,
                vicinity: Rect::from_center_size(center, (reach * 2.0, reach * 2.0)),
                reach_squared: reach * reach,
            });
        }
        let index = build_index(ink, |label| label.ink.union(label.vicinity), work)?;
        Ok(Self { index, occluded })
    }

    pub(super) fn own_cutout(
        &mut self,
        edge: usize,
        shaft: &PaintedShaft,
        stroke_outset: f64,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Option<Rect>> {
        let shaft_bounds = shaft.bounds.inflate(stroke_outset, stroke_outset);
        let mut cutout = None;
        let _ = self.index.visit(|entry| -> Result<_> {
            work.charge_adapter(1)?;
            let label = match entry {
                BvhEntry::Node(bounds) => {
                    let region = Rect::new(bounds.x[0], bounds.y[0], bounds.x[1], bounds.y[1]);
                    return Ok(Continue(rects_touch(shaft_bounds, region)));
                }
                BvhEntry::Item(label) => label,
            };
            if self.occluded[label.edge] {
                return Ok(Continue(true));
            }
            let owns_caption = label.edge == edge;
            let guard = label.ink.inflate(stroke_outset, stroke_outset);
            let crosses_ink = rects_touch(shaft.bounds, guard);
            let rivals_owner = !owns_caption && rects_touch(shaft.bounds, label.vicinity);
            if !(crosses_ink || rivals_owner) {
                return Ok(Continue(true));
            }
            work.charge_adapter(shaft.segments.len())?;
            let center = label.ink.center();
            if shaft.segments.iter().any(|segment| {
                (crosses_ink && segment.intersects(guard))
                    || (rivals_owner
                        && segment.is_within(center, label.vicinity, label.reach_squared))
            }) {
                if owns_caption {
                    cutout = Some(label.ink);
                } else {
                    self.occluded[label.edge] = true;
                }
            }
            Ok(Continue(true))
        })?;
        Ok(cutout)
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

    fn is_within(&self, center: Point, vicinity: Rect, reach_squared: f64) -> bool {
        rects_touch(self.bounds, vicinity)
            && self.path.nearest(center, 1e-3).distance_sq <= reach_squared
    }
}

pub(super) struct PaintedShaft {
    bounds: Rect,
    segments: Vec<ShaftSegment>,
}

impl PaintedShaft {
    pub(super) fn from_segments(
        route: &CanonicalPaintedEdge,
        segments: &[PathSeg],
        rough: Option<&BezPath>,
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
        })
    }
}

pub(super) fn reseat(
    routes: &mut [super::PaintedRelation],
    shafts: &[PaintedShaft],
    mut occupied: Vec<Rect>,
    stroke_outset: f64,
    can_expand_gutters: bool,
    work: &mut OperationLayoutWorkControl,
) -> Result<Option<Rect>> {
    work.charge_adapter(routes.len())?;
    let allocation = |work: &OperationLayoutWorkControl| {
        work.map_shoal_error(merman_layout_shoal::LayoutError::AllocationLimit)
    };
    occupied
        .try_reserve(routes.len())
        .map_err(|_| allocation(work))?;
    for route in routes.iter() {
        if route.seated
            && let Some(label) = &route.label
        {
            occupied.push(label.ink);
        }
    }
    let occupied = build_index(occupied, |rect| *rect, work)?;
    let mut shaft_entries = Vec::new();
    for (edge, shaft) in shafts.iter().enumerate() {
        work.charge_adapter(shaft.segments.len())?;
        shaft_entries
            .try_reserve(shaft.segments.len())
            .map_err(|_| allocation(work))?;
        shaft_entries.extend(shaft.segments.iter().map(|segment| (edge, segment)));
    }
    let foreign = build_index(shaft_entries, |(_, segment)| segment.bounds, work)?;
    let mut placed = BoundsIndex::default();
    let mut candidates = Vec::new();
    for (edge, route) in routes.iter_mut().enumerate() {
        work.charge_adapter(1)?;
        let Some(label) = route.label.as_mut().filter(|_| !route.seated) else {
            continue;
        };
        let scene = SeatScene {
            occupied: &occupied,
            placed: &placed,
            foreign: &foreign,
            stroke_outset,
        };
        let Some(shift) = scene.find_seat(
            edge,
            label.ink,
            &shafts[edge],
            &mut candidates,
            !can_expand_gutters,
            work,
        )?
        else {
            if can_expand_gutters {
                return Ok(Some(label.ink));
            }
            continue;
        };
        label.translate(shift);
        route.seated = true;
        placed
            .insert(bounds(label.ink), work)
            .map_err(|error| work.map_shoal_error(error))?;
        work.charge_adapter(shafts[edge].segments.len())?;
        let guard = label.ink.inflate(stroke_outset, stroke_outset);
        route.shaft_cutout = shafts[edge]
            .segments
            .iter()
            .any(|segment| segment.intersects(guard))
            .then_some(label.ink);
    }
    Ok(None)
}

/// Keep unseated captions outside all painted shafts and already placed captions. Their
/// visible endpoint reference and original relation ID identify the host at this distance.
pub(super) fn place_exterior(
    routes: &mut [super::PaintedRelation],
    relations: &[C4RelLayout],
    diagram: &DiagramBounds,
    measure: &C4Measurement<'_>,
    work: &mut OperationLayoutWorkControl,
) -> Result<()> {
    work.charge_adapter(work.checked_mul(routes.len(), 2)?)?;
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
        if let Some((ink, _)) = measure_c4_relation_label(&mut reference, measure)? {
            let y = label.ink.y0 - SHAFT_CLEARANCE - ink.y1;
            reference.label.y = y - label.origin.y;
            label.ink = label.ink.union(ink + Vec2::new(label.origin.x, y));
            label.reference = Some(Box::new(reference.label));
        }
        label.translate(Vec2::new(
            diagram.min_x - label.ink.x0,
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

#[derive(Clone, Copy)]
struct Seat {
    shift: Vec2,
    distance: f64,
    cost: f64,
    ordinal: usize,
}

struct SeatScene<'a> {
    occupied: &'a Bvh<Rect>,
    placed: &'a BoundsIndex,
    foreign: &'a Bvh<(usize, &'a ShaftSegment)>,
    stroke_outset: f64,
}

impl SeatScene<'_> {
    fn find_seat(
        &self,
        edge: usize,
        ink: Rect,
        shaft: &PaintedShaft,
        candidates: &mut Vec<Seat>,
        allow_overlap: bool,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Option<Vec2>> {
        let preferred = ink.center();
        let dx = ink.width() / 2.0 + self.stroke_outset + SHAFT_CLEARANCE;
        let dy = ink.height() / 2.0 + self.stroke_outset + SHAFT_CLEARANCE;
        // Keep a marginal seat on its local run when there is room closer to the shaft.
        // Probe the same run before allocating candidates along the entire relationship.
        work.charge_adapter(shaft.segments.len())?;
        let nearest = shaft
            .segments
            .iter()
            .map(|segment| (segment.path, segment.path.nearest(preferred, 1e-3)))
            .min_by(|(_, a), (_, b)| a.distance_sq.total_cmp(&b.distance_sq));
        if let Some((PathSeg::Line(line), nearest)) = nearest {
            let anchor = line.eval(nearest.t);
            let normal = if line.p0.y == line.p1.y {
                Some(Vec2::new(0.0, dy / 2.0))
            } else if line.p0.x == line.p1.x {
                Some(Vec2::new(dx / 2.0, 0.0))
            } else {
                None
            };
            if let Some(mut normal) = normal {
                if (preferred - anchor).dot(normal) < 0.0 {
                    normal = -normal;
                }
                for offset in [normal, Vec2::ZERO, -normal] {
                    work.charge_adapter(1)?;
                    let shift = anchor + offset - preferred;
                    if !shift.is_finite() {
                        continue;
                    }
                    let caption = ink + shift;
                    if self.overlap_area(caption, false, work)? == 0.0
                        && !self.foreign_shaft_blocks(
                            edge,
                            caption,
                            offset.hypot() + OWNERSHIP_CLEARANCE,
                            work,
                        )?
                    {
                        return Ok(Some(shift));
                    }
                }
            }
        }

        let limit = work.checked_mul(shaft.segments.len(), 54)?;
        work.charge_adapter(work.checked_add(limit, shaft.segments.len())?)?;
        candidates.clear();
        candidates
            .try_reserve(limit)
            .map_err(|_| work.map_shoal_error(merman_layout_shoal::LayoutError::AllocationLimit))?;
        for segment in &shaft.segments {
            let nearest = segment.path.nearest(preferred, 1e-3).t;
            let caption_cross = match segment.path {
                PathSeg::Line(line) if line.p0.y == line.p1.y => Some(ink.width()),
                PathSeg::Line(line) if line.p0.x == line.p1.x => Some(ink.height()),
                _ => None,
            };
            let inset = caption_cross.map_or(0.25, |span| {
                let along = (segment.path.end() - segment.path.start()).hypot();
                ((span / 2.0 + self.stroke_outset + SHAFT_CLEARANCE) / along).min(0.5)
            });
            for t in [
                nearest.clamp(inset, 1.0 - inset),
                0.0,
                inset,
                0.5,
                1.0 - inset,
                1.0,
            ] {
                let anchor = segment.path.eval(t);
                for offset in [-dx, 0.0, dx]
                    .into_iter()
                    .flat_map(|x| [-dy, 0.0, dy].map(|y| Vec2::new(x, y)))
                {
                    let shift = anchor + offset - preferred;
                    let distance = offset.hypot();
                    let cost = shift.hypot() + distance;
                    if cost.is_finite() {
                        candidates.push(Seat {
                            shift,
                            distance,
                            cost,
                            ordinal: candidates.len(),
                        });
                    }
                }
            }
        }
        let len = candidates.len();
        work.charge_adapter(work.checked_mul(len, len.checked_ilog2().unwrap_or(0) as usize + 1)?)?;
        candidates
            .sort_unstable_by(|a, b| a.cost.total_cmp(&b.cost).then(a.ordinal.cmp(&b.ordinal)));
        for &seat in &*candidates {
            work.charge_adapter(1)?;
            let caption = ink + seat.shift;
            // The chosen anchor gives an upper bound on distance to the host shaft.
            if self.overlap_area(caption, false, work)? == 0.0
                && !self.foreign_shaft_blocks(
                    edge,
                    caption,
                    seat.distance + OWNERSHIP_CLEARANCE,
                    work,
                )?
            {
                return Ok(Some(seat.shift));
            }
        }
        if !allow_overlap {
            return Ok(None);
        }

        // Relax obstacle overlap only with proven ownership and foreign-shaft clearance.
        let mut best = None::<(f64, Vec2)>;
        for &seat in &*candidates {
            work.charge_adapter(1)?;
            let caption = ink + seat.shift;
            let area = self.overlap_area(caption, true, work)?;
            if best.is_some_and(|(best_area, _)| area >= best_area) {
                continue;
            }
            work.charge_adapter(shaft.segments.len())?;
            let center = caption.center();
            let distance = shaft
                .segments
                .iter()
                .map(|segment| segment.path.nearest(center, 1e-3).distance_sq)
                .fold(f64::INFINITY, f64::min)
                .sqrt();
            if !self.foreign_shaft_blocks(edge, caption, distance + OWNERSHIP_CLEARANCE, work)? {
                if area == 0.0 {
                    return Ok(Some(seat.shift));
                }
                best = Some((area, seat.shift));
            }
        }
        Ok(best.map(|(_, shift)| shift))
    }

    fn overlap_area(
        &self,
        caption: Rect,
        sum: bool,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<f64> {
        let mut area = 0.0;
        let occupied = self.occupied.visit(|entry| -> Result<_> {
            work.charge_adapter(1)?;
            Ok(match entry {
                BvhEntry::Node(bounds) => Continue(rects_touch(caption, rect(bounds))),
                BvhEntry::Item(&obstacle) => {
                    area += caption.intersect(obstacle).area();
                    if !sum && area > 0.0 {
                        Break(())
                    } else {
                        Continue(true)
                    }
                }
            })
        })?;
        if !occupied.is_break() {
            area += if sum {
                self.placed.overlap_area(bounds(caption), work)
            } else {
                self.placed
                    .any_overlap(bounds(caption), work)
                    .map(|hit| if hit { 1.0 } else { 0.0 })
            }
            .map_err(|error| work.map_shoal_error(error))?;
        }
        Ok(area)
    }

    fn foreign_shaft_blocks(
        &self,
        edge: usize,
        caption: Rect,
        reach: f64,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<bool> {
        let guard = caption.inflate(self.stroke_outset, self.stroke_outset);
        let center = caption.center();
        let neighborhood = Rect::from_center_size(center, (reach * 2.0, reach * 2.0));
        let region = guard.union(neighborhood);
        Ok(self
            .foreign
            .visit(|entry| -> Result<_> {
                work.charge_adapter(1)?;
                Ok(match entry {
                    BvhEntry::Node(bounds) => Continue(rects_touch(region, rect(bounds))),
                    BvhEntry::Item(&(other, segment))
                        if other != edge
                            && (segment.intersects(guard)
                                || segment.is_within(center, neighborhood, reach * reach)) =>
                    {
                        Break(())
                    }
                    BvhEntry::Item(_) => Continue(true),
                })
            })?
            .is_break())
    }
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

    #[test]
    fn c4_cutout_uses_only_the_shaft_owner_even_when_label_boxes_overlap() {
        let own = Rect::new(2.0, -2.0, 6.0, 2.0);
        let foreign = Rect::new(4.0, -2.0, 8.0, 2.0);
        let mut policy = RenderResourcePolicy::constrained();
        policy
            .apply_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        let mut limited =
            OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(policy)));
        assert!(
            CaptionInk::new([Some(own), Some(foreign)].into_iter(), &[], &mut limited).is_err()
        );

        let mut work = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::constrained(),
        )));
        let route = CanonicalPaintedEdge {
            path: PaintedRoute {
                d: "M0,0L10,0".into(),
                bounds: crate::model::Bounds {
                    min_x: 0.0,
                    min_y: 0.0,
                    max_x: 10.0,
                    max_y: 0.0,
                },
            },
            marker_points: std::array::from_fn(|_| LayoutPoint { x: 0.0, y: 0.0 }),
            data_points: Vec::new(),
            markers: Default::default(),
        };
        let line = PathSeg::Line(Line::new((0.0, 0.0), (10.0, 0.0)));
        let shafts = std::array::from_fn::<_, 2, _>(|_| {
            PaintedShaft::from_segments(&route, &[line], None, &mut work).unwrap()
        });
        let mut captions =
            CaptionInk::new([Some(own), Some(foreign)].into_iter(), &shafts, &mut work).unwrap();
        assert_eq!(
            captions.own_cutout(0, &shafts[0], 0.5, &mut work).unwrap(),
            Some(own)
        );
        assert_eq!(captions.occluded, [false, true]);
    }

    #[test]
    fn c4_clear_ink_nearer_a_foreign_shaft_still_needs_reseating() {
        let shaft = |y| {
            let bounds = Rect::new(0.0, y, 10.0, y);
            PaintedShaft {
                bounds,
                segments: vec![ShaftSegment {
                    path: PathSeg::Line(Line::new((0.0, y), (10.0, y))),
                    bounds,
                }],
            }
        };
        let shafts = [shaft(0.0), shaft(9.0)];
        let mut work = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::constrained(),
        )));
        let ink = Rect::new(2.0, 4.0, 6.0, 6.0);
        let mut captions =
            CaptionInk::new([Some(ink), None].into_iter(), &shafts, &mut work).unwrap();
        assert_eq!(
            captions.own_cutout(1, &shafts[1], 0.5, &mut work).unwrap(),
            None
        );
        assert_eq!(captions.occluded, [true, false]);
    }

    #[test]
    fn c4_reseating_minimizes_overlap_among_host_owned_positions() {
        let mut work = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::constrained(),
        )));
        let own = ShaftSegment::new(PathSeg::Line(Line::new((0.0, 0.0), (10.0, 0.0))));
        let host = PaintedShaft {
            bounds: own.bounds,
            segments: vec![own],
        };
        let rival = ShaftSegment::new(PathSeg::Line(Line::new((-20.0, -4.5), (30.0, -4.5))));
        let foreign = build_index(vec![(1, &rival)], |(_, shaft)| shaft.bounds, &mut work).unwrap();
        let obstacle = Rect::new(-30.0, -2.0, 30.0, 4.0);
        let occupied = build_index(vec![obstacle], |rect| *rect, &mut work).unwrap();
        let mut placed = BoundsIndex::default();
        placed
            .insert(bounds(Rect::new(3.0, 2.0, 7.0, 5.0)), &mut work)
            .unwrap();
        let scene = SeatScene {
            occupied: &occupied,
            placed: &placed,
            foreign: &foreign,
            stroke_outset: 0.5,
        };
        let ink = Rect::new(3.0, -1.0, 7.0, 1.0);
        let mut candidates = Vec::new();
        assert!(
            scene
                .find_seat(0, ink, &host, &mut candidates, false, &mut work)
                .unwrap()
                .is_none()
        );
        let caption = ink
            + scene
                .find_seat(0, ink, &host, &mut candidates, true, &mut work)
                .unwrap()
                .unwrap();
        assert_eq!(caption.center().y, 3.5);
        assert_eq!(scene.overlap_area(caption, true, &mut work).unwrap(), 6.0);
    }

    #[test]
    fn c4_overlap_fallback_keeps_the_ownership_clearance() {
        let mut work = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::constrained(),
        )));
        let own = ShaftSegment::new(PathSeg::Line(Line::new((0.0, 0.0), (10.0, 0.0))));
        let host = PaintedShaft {
            bounds: own.bounds,
            segments: vec![own],
        };
        let rival = ShaftSegment::new(PathSeg::Line(Line::new((-20.0, -6.0), (30.0, -6.0))));
        let foreign = build_index(vec![(1, &rival)], |(_, shaft)| shaft.bounds, &mut work).unwrap();
        let wall = Rect::new(-30.0, -2.7, 40.0, 7.0);
        let occupied = build_index(vec![wall], |rect| *rect, &mut work).unwrap();
        let scene = SeatScene {
            occupied: &occupied,
            placed: &BoundsIndex::default(),
            foreign: &foreign,
            stroke_outset: 0.5,
        };
        let ink = Rect::new(3.0, -3.0, 7.0, -2.5);
        let mut candidates = Vec::new();
        assert!(
            scene
                .find_seat(0, ink, &host, &mut candidates, false, &mut work)
                .unwrap()
                .is_none()
        );
        let caption = ink
            + scene
                .find_seat(0, ink, &host, &mut candidates, true, &mut work)
                .unwrap()
                .unwrap();
        let center = caption.center();
        let own = host.segments[0]
            .path
            .nearest(center, 1e-3)
            .distance_sq
            .sqrt();
        let foreign = rival.path.nearest(center, 1e-3).distance_sq.sqrt();
        assert!(foreign > own + OWNERSHIP_CLEARANCE);
    }
}

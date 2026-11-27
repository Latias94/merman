use std::ops::ControlFlow::Continue;

use kurbo::{BezPath, Line, ParamCurve, ParamCurveNearest, PathSeg, Point, Rect, Shape, Vec2};
use merman_layout_shoal::geom::{Bounds, Bvh, BvhEntry};
use merman_render_graph::c4::{SHAFT_CLEARANCE, measure_c4_relation_label};

use super::super::C4Measurement;
use crate::Result;
use crate::graph_layout::canonical::CanonicalPaintedEdge;
use crate::graph_layout::ir::Size;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{C4RelLayout, LayoutPoint};

#[derive(Debug)]
pub(super) struct PlacedRelationLabel {
    pub(super) origin: LayoutPoint,
    pub(super) ink: Rect,
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
        let levels = len.checked_ilog2().unwrap_or(0) as usize;
        work.charge_adapter(work.checked_mul(len, levels + 4)?)?;
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
                + SHAFT_CLEARANCE;
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
        let index = Bvh::build(ink, |label| {
            let region = label.ink.union(label.vicinity);
            Bounds {
                x: [region.x0, region.x1],
                y: [region.y0, region.y1],
            }
        })
        .map_err(|error| work.map_shoal_error(error))?;
        work.charge_adapter(0)?;
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
    pub(super) fn new(
        route: &CanonicalPaintedEdge,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Self> {
        work.charge_adapter(route.path.d.len())?;
        let path = BezPath::from_svg(&route.path.d).map_err(|error| {
            super::invalid(format!("c4: unable to check the painted shaft: {error}"))
        })?;
        work.charge_adapter(path.elements().len())?;
        let mut segments = Vec::new();
        segments
            .try_reserve_exact(path.elements().len())
            .map_err(|_| work.map_shoal_error(merman_layout_shoal::LayoutError::AllocationLimit))?;
        segments.extend(path.segments().map(ShaftSegment::new));
        let bounds = &route.path.bounds;
        Ok(Self {
            bounds: Rect::new(bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y),
            segments,
        })
    }
}

pub(super) fn reseat(
    routes: &mut [super::PaintedRelation],
    shafts: &[PaintedShaft],
    mut occupied: Vec<Rect>,
    stroke_outset: f64,
    work: &mut OperationLayoutWorkControl,
) -> Result<Option<usize>> {
    work.charge_adapter(routes.len())?;
    let allocation = |_| work.map_shoal_error(merman_layout_shoal::LayoutError::AllocationLimit);
    occupied.try_reserve(routes.len()).map_err(allocation)?;
    for route in routes.iter() {
        if route.seated
            && let Some(label) = &route.label
        {
            occupied.push(label.ink);
        }
    }
    for (edge, route) in routes.iter_mut().enumerate() {
        let Some(label) = route.label.as_mut().filter(|_| !route.seated) else {
            continue;
        };
        let Some(shift) = find_seat(edge, label.ink, shafts, &occupied, stroke_outset, work)?
        else {
            return Ok(Some(edge));
        };
        label.ink = label.ink + shift;
        label.origin.x += shift.x;
        label.origin.y += shift.y;
        occupied.push(label.ink);
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

fn find_seat(
    edge: usize,
    ink: Rect,
    shafts: &[PaintedShaft],
    occupied: &[Rect],
    stroke_outset: f64,
    work: &mut OperationLayoutWorkControl,
) -> Result<Option<Vec2>> {
    let preferred = ink.center();
    let dx = ink.width() / 2.0 + stroke_outset + SHAFT_CLEARANCE;
    let dy = ink.height() / 2.0 + stroke_outset + SHAFT_CLEARANCE;
    let mut best = None::<(f64, Vec2)>;
    for segment in &shafts[edge].segments {
        work.charge_adapter(1)?;
        let nearest = segment.path.nearest(preferred, 1e-3).t;
        for t in [nearest, 0.0, 0.25, 0.5, 0.75, 1.0] {
            let anchor = segment.path.eval(t);
            for offset in [-dx, 0.0, dx]
                .into_iter()
                .flat_map(|x| [-dy, 0.0, dy].map(|y| Vec2::new(x, y)))
            {
                work.charge_adapter(1)?;
                let center = anchor + offset;
                let shift = center - preferred;
                let distance = offset.hypot();
                let cost = shift.hypot() + distance;
                if !cost.is_finite() || best.is_some_and(|(old, _)| cost >= old) {
                    continue;
                }
                let caption = ink + shift;
                work.charge_adapter(occupied.len())?;
                if occupied
                    .iter()
                    .any(|&area| caption.intersect(area).area() > 0.0)
                {
                    continue;
                }
                let guard = caption.inflate(stroke_outset, stroke_outset);
                let reach = distance + SHAFT_CLEARANCE;
                let neighborhood = Rect::from_center_size(center, (reach * 2.0, reach * 2.0));
                let mut blocked = false;
                for (other, shaft) in shafts.iter().enumerate() {
                    work.charge_adapter(1)?;
                    if other == edge
                        || !(rects_touch(shaft.bounds, guard)
                            || rects_touch(shaft.bounds, neighborhood))
                    {
                        continue;
                    }
                    work.charge_adapter(shaft.segments.len())?;
                    if shaft.segments.iter().any(|segment| {
                        segment.intersects(guard)
                            || segment.is_within(center, neighborhood, reach * reach)
                    }) {
                        blocked = true;
                        break;
                    }
                }
                if !blocked {
                    best = Some((cost, shift));
                }
            }
        }
    }
    Ok(best.map(|(_, shift)| shift))
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
        let shafts = [
            PaintedShaft::new(&route, &mut work).unwrap(),
            PaintedShaft::new(&route, &mut work).unwrap(),
        ];
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
}

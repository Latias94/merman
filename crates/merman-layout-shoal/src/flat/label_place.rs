//! Place short measured labels in the original gap after routing has finished.

mod clearance;

use super::{Drawing, canonical_size};
use crate::work::{LayoutResult, Work, push, reserved};
use crate::{
    Axis, Direction, Graph, Options, Point, RoutePoint, RoutingStyle, Size, sweep_handle_fraction,
};
use clearance::{Bounds, Clearance, Scene};

pub(super) const MAX_LABELS_IN_GAP: u8 = 4;
pub(super) const MAX_LABELS: usize = 512;

#[derive(Clone, Copy, Debug)]
pub(super) struct InlineLabel {
    pub(super) edge: usize,
    /// Index of the upper occupied rank, before label sublayers are inserted.
    pub(super) gap: u32,
}

pub(super) struct GapPolicy<'a> {
    pub(super) spacing: f64,
    pub(super) restored: &'a [u32],
}

pub(super) fn eligible(size: Size, direction: Direction, rank_spacing: f64) -> bool {
    // The kernel receives a measured box. The height ceiling keeps typical multi-line
    // labels in obstacle tiers even when the configured rank gap is large.
    size.width > 0.0
        && size.height > 0.0
        && size.width <= 120.0
        && size.height <= (rank_spacing * 0.75).min(30.0)
        && canonical_size(direction, size).height < rank_spacing
}

fn scene(
    drawing: &Drawing,
    graph: &Graph,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Clearance>> {
    let mut scene = Scene::default();
    for &rect in &drawing.nodes {
        if !scene.area(Bounds::of(rect), work)? {
            return Ok(None);
        }
    }
    for (index, (id, route)) in drawing.edges.iter().enumerate() {
        work.visit(1)?;
        let spec = &graph.edges[*id as usize];
        for (at, size) in [
            (route.label_center, spec.label),
            (route.start_label_center, spec.start_label),
            (route.end_label_center, spec.end_label),
        ] {
            if let (Some(at), Some(size)) = (at, size)
                && !scene.area(
                    Bounds::label(at, canonical_size(graph.direction, size), 0.0),
                    work,
                )?
            {
                return Ok(None);
            }
        }
        let [first, .., last] = route.points.as_slice() else {
            return Ok(None);
        };
        if !scene.marker(*first, route.ports.0, spec.start_marker, work)?
            || !scene.marker(*last, route.ports.1, spec.end_marker, work)?
            || !scene.stroke(index, &route.points, options.routing, work)?
        {
            return Ok(None);
        }
    }
    Ok(Some(scene.index(work)?))
}

#[derive(Clone, Copy)]
struct Anchor {
    at: Point,
    normal: Point,
    fraction: f64,
    bend_room: f64,
}

fn anchor(a: RoutePoint, b: RoutePoint, t: f64, style: RoutingStyle, fraction: f64) -> Anchor {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let straight_len = dx.hypot(dy);
    let (mut sx, mut sy) = (t, t);
    let (mut vx, mut vy) = (dx, dy);
    if dx != 0.0 && dy != 0.0 && style == RoutingStyle::Curved {
        let ease = sweep_handle_fraction(dx, dy);
        let blend = t * t * (3.0 - 2.0 * t);
        let slope = 6.0 * t * (1.0 - t);
        let run = blend + ease * 3.0 * t * (1.0 - t) * (1.0 - 2.0 * t);
        let tangent = slope + ease * (3.0 - 3.0 * slope);
        match b.axis {
            Axis::Vertical => (sx, sy, vx, vy) = (blend, run, dx * slope, dy * tangent),
            Axis::Horizontal => (sx, sy, vx, vy) = (run, blend, dx * tangent, dy * slope),
        }
    }
    let len = vx.hypot(vy);
    Anchor {
        at: Point {
            x: dx.mul_add(sx, a.x),
            y: dy.mul_add(sy, a.y),
        },
        normal: Point {
            x: -vy / len,
            y: vx / len,
        },
        fraction,
        bend_room: t.min(1.0 - t) * straight_len,
    }
}

fn anchors(
    route: &[RoutePoint],
    style: RoutingStyle,
    work: &mut Work<'_>,
) -> LayoutResult<[Option<Anchor>; 4]> {
    let mut total = 0.0;
    let mut longest = (0.0, 0.0, 0);
    for (i, leg) in route.windows(2).enumerate() {
        work.visit(1)?;
        let size = (leg[1].x - leg[0].x).hypot(leg[1].y - leg[0].y);
        if size > longest.0 {
            longest = (size, total, i);
        }
        total += size;
    }
    let mut anchors = [None; 4];
    if total == 0.0 || !total.is_finite() {
        return Ok(anchors);
    }
    let (size, start, i) = longest;
    anchors[3] = Some(anchor(
        route[i],
        route[i + 1],
        0.5,
        style,
        (start + size * 0.5) / total,
    ));
    let mut start = 0.0;
    for leg in route.windows(2) {
        work.visit(1)?;
        let (a, b) = (leg[0], leg[1]);
        let size = (b.x - a.x).hypot(b.y - a.y);
        for (slot, fraction) in anchors.iter_mut().zip([0.5, 0.35, 0.65]) {
            let distance = fraction * total - start;
            if slot.is_none() && size > 0.0 && (0.0..=size).contains(&distance) {
                *slot = Some(anchor(a, b, distance / size, style, fraction));
            }
        }
        start += size;
    }
    Ok(anchors)
}

struct Label<'a> {
    edge: usize,
    route: &'a [RoutePoint],
    size: Size,
    gap: [f64; 2],
}

impl Label<'_> {
    fn choose(
        &self,
        clearance: &mut Clearance,
        placed: &[(Point, Bounds)],
        options: &Options,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<(Point, Bounds)>> {
        let pad = options.spacing.edge.max(2.0);
        let mut best = None::<(f64, Point, Bounds)>;
        for anchor in anchors(self.route, options.routing, work)?
            .into_iter()
            .flatten()
        {
            let along = self.size.width * 0.5 * anchor.normal.y.abs()
                + self.size.height * 0.5 * anchor.normal.x.abs();
            let across = self.size.width * 0.5 * anchor.normal.x.abs()
                + self.size.height * 0.5 * anchor.normal.y.abs();
            let side_distance =
                across + pad * (anchor.normal.x.abs() + anchor.normal.y.abs()) + 1.0;
            for side in [-1.0, 1.0, 0.0] {
                if !clearance.visit(work)? {
                    return Ok(None);
                }
                let center = side == 0.0;
                let distance = side * side_distance;
                let at = Point {
                    x: distance.mul_add(anchor.normal.x, anchor.at.x),
                    y: distance.mul_add(anchor.normal.y, anchor.at.y),
                };
                let padded = Bounds::label(at, self.size, pad);
                let score = (anchor.fraction - 0.5).abs() * 32.0
                    + (along - anchor.bend_room).clamp(0.0, 12.0)
                    + if center { 10.0 } else { 0.0 };
                if !padded.finite()
                    || padded.y[0] < self.gap[0]
                    || padded.y[1] > self.gap[1]
                    || best.is_some_and(|winner| score >= winner.0)
                    || !clearance.clear(padded, center.then_some(self.edge), work)?
                {
                    continue;
                }
                let mut free = true;
                for &(_, wall) in placed {
                    if !clearance.visit(work)? {
                        return Ok(None);
                    }
                    if padded.overlaps(wall) {
                        free = false;
                        break;
                    }
                }
                let bounds = Bounds::label(at, self.size, 0.0);
                if free && bounds.contains_center(at) {
                    best = Some((score, at, bounds));
                }
            }
        }
        Ok(best.map(|(_, at, bounds)| (at, bounds)))
    }
}

pub(super) enum Placement {
    Placed,
    Restore(Vec<u32>),
    Legacy,
}

pub(super) fn place(
    drawing: &mut Drawing,
    labels: &[InlineLabel],
    gaps: &[[f64; 2]],
    graph: &Graph,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<Placement> {
    let Some(mut clearance) = scene(drawing, graph, options, work)? else {
        return Ok(Placement::Legacy);
    };
    debug_assert_eq!(labels.len(), gaps.len());
    work.charge(labels.len())?;
    let mut placed = reserved(labels.len())?;
    let mut blocked = Vec::new();
    for (&InlineLabel { edge, gap: rank }, &gap) in labels.iter().zip(gaps) {
        let (id, route) = &drawing.edges[edge];
        let size = graph.edges[*id as usize]
            .label
            .expect("inline edge has a measured label");
        let label = Label {
            edge,
            route: &route.points,
            size: canonical_size(graph.direction, size),
            gap,
        };
        match label.choose(&mut clearance, &placed, options, work)? {
            Some(placement) => placed.push(placement),
            None if clearance.exhausted() => return Ok(Placement::Legacy),
            None => push(&mut blocked, rank)?,
        }
    }
    if !blocked.is_empty() {
        if placed.is_empty() {
            return Ok(Placement::Legacy);
        }
        return Ok(Placement::Restore(blocked));
    }
    for (&InlineLabel { edge, .. }, (at, _)) in labels.iter().zip(placed) {
        drawing.edges[edge].1.label_center = Some(at);
    }
    Ok(Placement::Placed)
}

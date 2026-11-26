//! Place short measured labels in the original gap after routing has finished.

mod clearance;

use super::{Component, Drawing, canonical_size, contour_inset};
use crate::work::{LayoutResult, Work, push, reserved};
use crate::{
    Axis, Direction, EdgeSpec, Graph, KnotKind, Options, Point, RoutePoint, RoutingStyle, Side,
    Size, Spacing, sweep_handle_fraction,
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
    pub(super) spacing: Spacing,
    pub(super) restored: &'a [u32],
}

fn label_padding(spacing: Spacing) -> f64 {
    (spacing.edge * 0.35).max(2.0)
}

fn marker_lengths(edge: &EdgeSpec) -> [f64; 2] {
    [edge.start_marker.length, edge.end_marker.length]
}

impl GapPolicy<'_> {
    pub(super) fn admits(&self, size: Size, edge: &EdgeSpec, direction: Direction) -> bool {
        let [start, end] = marker_lengths(edge);
        let depth = canonical_size(direction, size).height;
        // Use the unexpanded gap and budget for markers on the rank faces. The placed
        // routes still need clearance checks against actual contacts and neighboring ink.
        size.width > 0.0
            && size.height > 0.0
            && size.width <= 120.0
            && size.height <= (self.spacing.rank * 0.75).min(30.0)
            && depth + 2.0 * label_padding(self.spacing) + start + end < self.spacing.rank
    }
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
    tangent: Point,
    fraction: f64,
    bend_room: f64,
}

const ARC_STEPS: usize = 12;
const OPTICAL_FRACTIONS: [f64; 11] = [
    0.5, 0.475, 0.525, 0.45, 0.55, 0.425, 0.575, 0.4, 0.6, 0.35, 0.65,
];
const ANCHOR_COUNT: usize = OPTICAL_FRACTIONS.len() + 3;

fn arc_samples(a: RoutePoint, b: RoutePoint, style: RoutingStyle) -> [f64; ARC_STEPS + 1] {
    let mut lengths = [0.0; ARC_STEPS + 1];
    let mut previous = Point { x: a.x, y: a.y };
    for i in 1..=ARC_STEPS {
        let t = i as f64 / ARC_STEPS as f64;
        let at = anchor(a, b, t, style, t).at;
        lengths[i] = lengths[i - 1] + (at.x - previous.x).hypot(at.y - previous.y);
        previous = at;
    }
    lengths
}

fn leg_samples(
    a: RoutePoint,
    b: RoutePoint,
    style: RoutingStyle,
    work: &mut Work<'_>,
) -> LayoutResult<Option<[f64; ARC_STEPS + 1]>> {
    if style == RoutingStyle::Curved && a.x != b.x && a.y != b.y {
        work.visit(ARC_STEPS)?;
        Ok(Some(arc_samples(a, b, style)))
    } else {
        Ok(None)
    }
}

fn anchor(a: RoutePoint, b: RoutePoint, t: f64, style: RoutingStyle, fraction: f64) -> Anchor {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let straight_len = dx.hypot(dy);
    let (mut sx, mut sy) = (t, t);
    let (mut vx, mut vy) = (dx, dy);
    if dx != 0.0 && dy != 0.0 && style == RoutingStyle::Curved {
        let (across, along) = match b.axis {
            Axis::Vertical => (dx, dy),
            Axis::Horizontal => (dy, dx),
        };
        let ease = sweep_handle_fraction(across, along);
        let blend = t * t * (3.0 - 2.0 * t);
        let slope = 6.0 * t * (1.0 - t);
        let run = blend + ease * 3.0 * t * (1.0 - t) * (1.0 - 2.0 * t);
        let tangent = slope + ease * (3.0 - 3.0 * slope);
        let shoulder = if a.kind == KnotKind::ContourDeparture {
            (0.35 * ease * along.abs() / across.abs()).min(0.25)
        } else {
            0.0
        };
        let side = blend + 3.0 * shoulder * t * (1.0 - t).powi(2);
        let side_tangent = slope + 3.0 * shoulder * (1.0 - t) * (1.0 - 3.0 * t);
        match b.axis {
            Axis::Vertical => (sx, sy, vx, vy) = (side, run, dx * side_tangent, dy * tangent),
            Axis::Horizontal => (sx, sy, vx, vy) = (run, side, dx * tangent, dy * side_tangent),
        }
    }
    let len = vx.hypot(vy);
    Anchor {
        at: Point {
            x: dx.mul_add(sx, a.x),
            y: dy.mul_add(sy, a.y),
        },
        tangent: Point {
            x: vx / len,
            y: vy / len,
        },
        fraction,
        bend_room: t.min(1.0 - t) * straight_len,
    }
}

fn anchors(
    route: &[RoutePoint],
    ends: [RoutePoint; 2],
    style: RoutingStyle,
    [start_marker, end_marker]: [f64; 2],
    contour_limits: Option<[f64; 2]>,
    work: &mut Work<'_>,
) -> LayoutResult<[Option<Anchor>; ANCHOR_COUNT]> {
    let legs = || {
        route.windows(2).enumerate().map(|(i, leg)| {
            let a = if i == 0 { ends[0] } else { leg[0] };
            let b = if i + 2 == route.len() {
                ends[1]
            } else {
                leg[1]
            };
            (a, b)
        })
    };
    let mut total = 0.0;
    let mut longest = (0.0, 0.0, ends);
    for (a, b) in legs() {
        work.visit(1)?;
        let size = leg_samples(a, b, style, work)?
            .map_or_else(|| (b.x - a.x).hypot(b.y - a.y), |arc| arc[ARC_STEPS]);
        if size > longest.0 {
            longest = (size, total, [a, b]);
        }
        total += size;
    }
    let mut anchors = [None; ANCHOR_COUNT];
    let shaft = total - start_marker - end_marker;
    if shaft <= 0.0 || !shaft.is_finite() {
        return Ok(anchors);
    }
    let (size, start, [a, b]) = longest;
    anchors[ANCHOR_COUNT - 1] = Some(anchor(
        a,
        b,
        0.5,
        style,
        ((start + size * 0.5 - start_marker) / shaft).clamp(0.0, 1.0),
    ));
    let mut start = 0.0;
    for (a, b) in legs() {
        work.visit(1)?;
        let arc = leg_samples(a, b, style, work)?;
        let size = arc.map_or_else(|| (b.x - a.x).hypot(b.y - a.y), |arc| arc[ARC_STEPS]);
        for (slot, fraction) in anchors.iter_mut().zip(OPTICAL_FRACTIONS) {
            let distance = fraction * shaft + start_marker - start;
            if slot.is_none() && size > 0.0 && (0.0..=size).contains(&distance) {
                let t = if let Some(arc) = arc {
                    let hi = arc.partition_point(|&at| at < distance).clamp(1, ARC_STEPS);
                    let lo = hi - 1;
                    let length = arc[hi] - arc[lo];
                    (lo as f64
                        + if length > 0.0 {
                            (distance - arc[lo]) / length
                        } else {
                            0.0
                        })
                        / ARC_STEPS as f64
                } else {
                    distance / size
                };
                *slot = Some(anchor(a, b, t, style, fraction));
            }
        }
        if let (Some(arc), Some(limits)) = (&arc, contour_limits) {
            for (slot, y) in anchors[OPTICAL_FRACTIONS.len()..ANCHOR_COUNT - 1]
                .iter_mut()
                .zip(limits)
            {
                if slot.is_none() && a.y < y && y < b.y {
                    // An optical midpoint outside the gap can be closer to this
                    // boundary than any of the regularly spaced candidates.
                    let (mut low, mut high) = (0.0_f64, 1.0);
                    for _ in 0..ARC_STEPS {
                        work.visit(1)?;
                        let t = low.midpoint(high);
                        if anchor(a, b, t, style, 0.0).at.y < y {
                            low = t;
                        } else {
                            high = t;
                        }
                    }
                    let t = low.midpoint(high);
                    let sample = t * ARC_STEPS as f64;
                    let i = (sample as usize).min(ARC_STEPS - 1);
                    let length = (arc[i + 1] - arc[i]).mul_add(sample - i as f64, arc[i]);
                    *slot = Some(anchor(
                        a,
                        b,
                        t,
                        style,
                        (start + length - start_marker) / shaft,
                    ));
                }
            }
        }
        start += size;
    }
    Ok(anchors)
}

struct Label<'a> {
    edge: usize,
    route: &'a [RoutePoint],
    ends: [RoutePoint; 2],
    markers: [f64; 2],
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
        let pad = label_padding(options.spacing);
        let contour_departure = self.ends[0].kind == KnotKind::ContourDeparture;
        let along_pad = if contour_departure {
            pad.min(0.25)
        } else {
            pad
        };
        let half_height = self.size.height * 0.5 + along_pad;
        let contour_limits = contour_departure.then_some([
            self.gap[0] + half_height + 0.02,
            self.gap[1] - half_height - 0.02,
        ]);
        let mut best = None::<(f64, Point, Bounds)>;
        for anchor in anchors(
            self.route,
            self.ends,
            options.routing,
            self.markers,
            contour_limits,
            work,
        )?
        .into_iter()
        .flatten()
        {
            if !clearance.visit(work)? {
                return Ok(None);
            }
            let at = anchor.at;
            let along = self.size.width * 0.5 * anchor.tangent.x.abs()
                + self.size.height * 0.5 * anchor.tangent.y.abs();
            // The box face below a clipped departure is already outside its outline.
            // Keep transverse clearance while the label approaches the optical center.
            let padded = Bounds::from_half_extents(at, self.size.width * 0.5 + pad, half_height);
            let score = (anchor.fraction - 0.5).abs() * 32.0
                + (along - anchor.bend_room).clamp(0.0, 12.0) / 24.0;
            if !padded.finite()
                || padded.y[0] < self.gap[0]
                || padded.y[1] > self.gap[1]
                || best.is_some_and(|winner| score >= winner.0)
                || !clearance.clear(padded, self.edge, work)?
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
    component: &Component,
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
        let spec = &graph.edges[*id as usize];
        let size = spec.label.expect("inline edge has a measured label");
        let mut ends = [
            route.points[0],
            *route.points.last().expect("a routed edge"),
        ];
        let link = component.links[edge];
        for (contact, (local, side)) in ends.iter_mut().zip([
            (link.from as usize, route.ports.0),
            (link.to as usize, route.ports.1),
        ]) {
            let node = component.nodes[local];
            let center = drawing.nodes[local].center();
            let offset = match side.axis() {
                Axis::Vertical => contact.x - center.x,
                Axis::Horizontal => contact.y - center.y,
            };
            let inset = contour_inset(node, graph.direction, side, offset);
            match side {
                Side::Top => contact.y += inset,
                Side::Bottom => contact.y -= inset,
                Side::Left => contact.x += inset,
                Side::Right => contact.x -= inset,
            }
        }
        let label = Label {
            edge,
            route: &route.points,
            ends,
            markers: marker_lengths(spec),
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

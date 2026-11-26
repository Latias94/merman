use super::*;
use crate::geom::{Bvh, BvhEntry};
use std::ops::ControlFlow::{Break, Continue};

#[derive(Clone, Copy)]
enum ObstacleKind {
    Solid,
    GroupFrame,
    Route(usize),
}

#[derive(Clone, Copy)]
struct Obstacle {
    bounds: Bounds,
    kind: ObstacleKind,
}

#[derive(Clone, Copy)]
enum ClearanceTier {
    OffAll,
    OnOwnRoute,
    /// The hollow group border can cross the caption in a crowded gutter.
    AcrossFrame,
}

struct Scene {
    obstacles: Bvh<Obstacle>,
    labels: Vec<Caption>,
    clearance: f64,
}

struct Caption {
    ink: Bounds,
    prefix_bottom: f64,
}

impl Scene {
    fn add_caption(&mut self, ink: Bounds, work: &mut Work<'_>) -> LayoutResult<()> {
        work.visit(self.labels.len().checked_ilog2().unwrap_or(0) as usize + 1)?;
        let at = self
            .labels
            .partition_point(|label| label.ink.y[0] <= ink.y[0]);
        let prefix_bottom = if at == 0 {
            ink.y[1]
        } else {
            self.labels[at - 1].prefix_bottom.max(ink.y[1])
        };
        work.charge(self.labels.len() - at + 1)?;
        self.labels
            .try_reserve(1)
            .map_err(|_| LayoutError::AllocationLimit)?;
        self.labels.insert(at, Caption { ink, prefix_bottom });
        for label in &mut self.labels[at + 1..] {
            if label.prefix_bottom >= prefix_bottom {
                break;
            }
            label.prefix_bottom = prefix_bottom;
        }
        Ok(())
    }

    fn fits(
        &self,
        at: Point,
        size: Size,
        scope: Option<Bounds>,
        own_route: usize,
        tier: ClearanceTier,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        let pad = match tier {
            ClearanceTier::OffAll => self.clearance,
            ClearanceTier::OnOwnRoute | ClearanceTier::AcrossFrame => {
                (self.clearance * 0.5).min(4.0)
            }
        };
        let ink = Bounds::from_half_extents(at, size.width / 2.0 + pad, size.height / 2.0 + pad);
        if !ink.finite() {
            return Ok(false);
        }
        if let Some(bounds) = scope
            && (ink.x[0] < bounds.x[0]
                || ink.x[1] > bounds.x[1]
                || ink.y[0] < bounds.y[0]
                || ink.y[1] > bounds.y[1])
        {
            return Ok(false);
        }
        if let Some(last) = self.labels.last()
            && last.prefix_bottom > ink.y[0]
            && self.labels[0].ink.y[0] < ink.y[1]
        {
            work.visit(self.labels.len().ilog2() as usize + 1)?;
            let first = self
                .labels
                .partition_point(|label| label.prefix_bottom <= ink.y[0]);
            for label in &self.labels[first..] {
                work.visit(1)?;
                if label.ink.y[0] >= ink.y[1] {
                    break;
                }
                if ink.overlaps_strictly(label.ink) {
                    return Ok(false);
                }
            }
        }
        let hit = self.obstacles.visit(|entry| -> LayoutResult<_> {
            work.visit(1)?;
            Ok(match entry {
                BvhEntry::Node(node) => Continue(ink.overlaps_strictly(node)),
                BvhEntry::Item(obs) => {
                    let blocks = match obs.kind {
                        ObstacleKind::Solid => true,
                        ObstacleKind::GroupFrame => !matches!(tier, ClearanceTier::AcrossFrame),
                        ObstacleKind::Route(edge) => {
                            edge != own_route || matches!(tier, ClearanceTier::OffAll)
                        }
                    };
                    if blocks && ink.overlaps_strictly(obs.bounds) {
                        Break(())
                    } else {
                        Continue(true)
                    }
                }
            })
        })?;
        Ok(hit.is_continue())
    }

    fn sample_route(
        points: &[RoutePoint],
        lengths: &[f64],
        total: f64,
        markers: [f64; 2],
        t: f64,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<([f64; 2], usize)>> {
        let mut left = (total - markers[0] - markers[1])
            .max(0.0)
            .mul_add(t, markers[0]);
        for (&length, leg) in lengths.iter().zip(points.windows(2)) {
            work.visit(1)?;
            if length <= left {
                left -= length;
                continue;
            }
            let ratio = left / length;
            let [a, b] = [leg[0], leg[1]];
            let at = [a.x + (b.x - a.x) * ratio, a.y + (b.y - a.y) * ratio];
            let cross = usize::from(a.y == b.y);
            return Ok(Some((at, cross)));
        }
        Ok(None)
    }

    #[allow(clippy::too_many_arguments)]
    fn near_route(
        &self,
        points: &[RoutePoint],
        route_index: usize,
        fraction: f64,
        markers: [f64; 2],
        size: Size,
        scope: Option<Bounds>,
        lengths: &mut Vec<f64>,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Point>> {
        lengths.clear();
        lengths
            .try_reserve(points.len().saturating_sub(1))
            .map_err(|_| LayoutError::AllocationLimit)?;
        let mut total = 0.0;
        for leg in points.windows(2) {
            work.visit(1)?;
            let length = (leg[1].x - leg[0].x).abs() + (leg[1].y - leg[0].y).abs();
            lengths.push(length);
            total += length;
        }
        if !total.is_finite() {
            return Err(PRECISION);
        }
        let radius = total.mul_add(0.75, 60.0).min(120.0);
        let margin = [
            size.width / 2.0 + self.clearance,
            size.height / 2.0 + self.clearance,
        ];
        let room = scope.map_or([[0.0, f64::INFINITY]; 2], |bounds| [bounds.x, bounds.y]);
        let seat = |mut candidate: [f64; 2],
                    sample: [f64; 2],
                    tier,
                    best: Option<f64>,
                    work: &mut Work<'_>| {
            work.visit(1)?;
            let pad = match tier {
                ClearanceTier::OffAll => self.clearance,
                _ => (self.clearance * 0.5).min(4.0),
            };
            for ((coord, [lo, hi]), half) in candidate
                .iter_mut()
                .zip(room)
                .zip([size.width / 2.0 + pad, size.height / 2.0 + pad])
            {
                if lo + half > hi - half {
                    return Ok(None);
                }
                *coord = coord.clamp(lo + half, hi - half);
            }
            let [x, y] = candidate;
            let at = Point { x, y };
            let distance = (x - sample[0]).hypot(y - sample[1]);
            if distance > radius
                || best.is_some_and(|best| distance >= best)
                || !self.fits(at, size, scope, route_index, tier, work)?
            {
                return Ok(None);
            }
            Ok::<_, LayoutError>(Some((distance, at)))
        };
        let probes = [
            fraction,
            (fraction + 0.25) / 2.0,
            (fraction + 0.75) / 2.0,
            0.2,
            0.8,
        ];
        let mut samples = [None; 5];
        for (probe, t) in probes.into_iter().enumerate() {
            let Some((at, cross)) = Self::sample_route(points, lengths, total, markers, t, work)?
            else {
                continue;
            };
            samples[probe] = Some((at, cross));
            for sign in [-1.0, 1.0] {
                let mut candidate = at;
                let edge = at[cross] + sign * margin[cross];
                candidate[cross] = if sign < 0.0 {
                    edge.next_down()
                } else {
                    edge.next_up()
                };
                if let Some((_, candidate)) =
                    seat(candidate, at, ClearanceTier::OffAll, None, work)?
                {
                    return Ok(Some(candidate));
                }
            }
        }
        for tier in [ClearanceTier::OnOwnRoute, ClearanceTier::AcrossFrame] {
            for (at, cross) in samples.into_iter().flatten() {
                for sign in [0.0, -1.0, 1.0] {
                    let mut shifted = at;
                    shifted[cross] += sign * (size.height.min(size.width) * 0.5 + 4.0);
                    if let Some((_, shifted)) = seat(shifted, at, tier, None, work)? {
                        return Ok(Some(shifted));
                    }
                }
            }
        }
        for tier in [ClearanceTier::OffAll, ClearanceTier::AcrossFrame] {
            for (mult, radius_fraction) in [(2.0, 0.0), (3.0, 0.0), (0.0, 0.5), (0.0, 0.875)] {
                let mut closest = None::<(f64, Point)>;
                for (at, cross) in samples.into_iter().flatten() {
                    let across = (mult * margin[cross])
                        .max(radius * radius_fraction)
                        .next_up();
                    let along = margin[1 - cross].min(radius / 2.0);
                    for sign in [-1.0, 1.0] {
                        for side in [0.0, -1.0, 1.0] {
                            let mut candidate = at;
                            candidate[cross] += sign * across;
                            candidate[1 - cross] += side * along;
                            if let Some(seat) =
                                seat(candidate, at, tier, closest.map(|(best, _)| best), work)?
                            {
                                closest = Some(seat);
                            }
                        }
                    }
                }
                if let Some((_, at)) = closest {
                    return Ok(Some(at));
                }
            }
        }
        Ok(None)
    }
}

pub(super) fn place(
    context: &Routing<'_>,
    plan: &Plan,
    routes: &mut [EdgeGeometry],
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let root = context.hierarchy.root();
    work.charge(context.drawing.nodes.len())?;
    let mut obstacles = reserved(context.drawing.nodes.len())?;
    for &node_rect in &context.drawing.nodes {
        work.visit(1)?;
        obstacles.push(Obstacle {
            bounds: Bounds::of(node_rect),
            kind: ObstacleKind::Solid,
        });
    }
    for (group, geometry) in context.drawing.groups.iter().enumerate() {
        work.visit(1)?;
        if let Some(title) = context.title(group) {
            push(
                &mut obstacles,
                Obstacle {
                    bounds: Bounds::of(title),
                    kind: ObstacleKind::Solid,
                },
            )?;
        }
        let Bounds { x, y } = Bounds::of(geometry.rect);
        // A caption can occupy the empty interior of a boundary. Only the border carries ink.
        for bounds in [
            Bounds { x, y: [y[0]; 2] },
            Bounds { x, y: [y[1]; 2] },
            Bounds { x: [x[0]; 2], y },
            Bounds { x: [x[1]; 2], y },
        ] {
            push(
                &mut obstacles,
                Obstacle {
                    bounds,
                    kind: ObstacleKind::GroupFrame,
                },
            )?;
        }
    }
    for (edge, route) in routes.iter().enumerate() {
        work.visit(1)?;
        let mut previous: Option<Bounds> = None;
        for leg in route.points.windows(2) {
            work.visit(1)?;
            let bounds = Bounds::between(position(leg[0]), position(leg[1]));
            if let Some(run) = &mut previous
                && ((run.x == bounds.x && run.x[0] == run.x[1])
                    || (run.y == bounds.y && run.y[0] == run.y[1]))
            {
                *run = run.union(bounds);
            } else if let Some(bounds) = previous.replace(bounds) {
                push(
                    &mut obstacles,
                    Obstacle {
                        bounds,
                        kind: ObstacleKind::Route(edge),
                    },
                )?;
            }
        }
        if let Some(bounds) = previous {
            push(
                &mut obstacles,
                Obstacle {
                    bounds,
                    kind: ObstacleKind::Route(edge),
                },
            )?;
        }
    }
    for site in &plan.sites {
        work.visit(1)?;
        let side = if site.inner {
            opposite(site.side)
        } else {
            site.side
        };
        if let Some(bounds) = Bounds::marker_footprint(site.at, side, site.marker) {
            push(
                &mut obstacles,
                Obstacle {
                    bounds,
                    kind: ObstacleKind::Solid,
                },
            )?;
        }
    }
    let mut scene = Scene {
        obstacles: Bvh::build_with_work(obstacles, |obs| obs.bounds, work)?,
        labels: Vec::new(),
        clearance: context.metrics.label_clearance,
    };
    let mut bottom = context.drawing.bounds.height;
    if let Some(ink) = scene.obstacles.root_bounds() {
        bottom = bottom.max(ink.y[1]);
    }
    let mut lengths = Vec::new();
    // Reserve the most constrained captions first. Within a scope, preserve authored order.
    for &scope in context.hierarchy.order.iter().rev() {
        work.visit(1)?;
        let bounds = Bounds::of(context.bounds(scope));
        let constraint = (scope != root).then_some(bounds);
        for part in &plan.scopes[scope] {
            work.visit(1)?;
            let edge = part.edge;
            let spec = &context.graph.edges[edge];
            let route = &mut routes[edge];
            let markers = [spec.start_marker.length, spec.end_marker.length];
            for ((size, fraction), center) in part.labels.into_iter().zip([0.5, 0.15, 0.85]).zip([
                &mut route.label_center,
                &mut route.start_label_center,
                &mut route.end_label_center,
            ]) {
                let Some(size) = size else { continue };
                let mut seat = scene.near_route(
                    &route.points,
                    edge,
                    fraction,
                    markers,
                    size,
                    constraint,
                    &mut lengths,
                    work,
                )?;
                // Placement reserves a caption row in each scope for congested routes.
                let y = bounds.y[1] - context.metrics.margin() - size.height / 2.0;
                let [lo, hi] = bounds.x;
                let inset = size.width / 2.0 + scene.clearance;
                if seat.is_none() {
                    for x in [lo.midpoint(hi), lo + inset, hi - inset] {
                        let at = Point { x, y };
                        if scene.fits(at, size, constraint, edge, ClearanceTier::OffAll, work)? {
                            seat = Some(at);
                            break;
                        }
                    }
                }
                let at = seat.unwrap_or(Point {
                    x: inset,
                    y: bottom + scene.clearance + size.height / 2.0,
                });
                let ink = Bounds::centered(at, size);
                if !ink.finite() {
                    return Err(PRECISION);
                }
                bottom = bottom.max(ink.y[1]);
                *center = Some(at);
                scene.add_caption(ink, work)?;
            }
        }
    }
    Ok(())
}

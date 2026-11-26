use super::*;
use crate::geom::{Bvh, BvhEntry};
use std::ops::ControlFlow::{Break, Continue};

struct Scene {
    walls: Bvh<Bounds>,
    labels: Vec<Bounds>,
    clearance: f64,
}

impl Scene {
    fn fits(
        &self,
        at: Point,
        size: Size,
        bounds: Bounds,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        let ink = Bounds::from_half_extents(
            at,
            size.width / 2.0 + self.clearance,
            size.height / 2.0 + self.clearance,
        );
        if !ink.finite()
            || ink.x[0] < bounds.x[0]
            || ink.x[1] > bounds.x[1]
            || ink.y[0] < bounds.y[0]
            || ink.y[1] > bounds.y[1]
        {
            return Ok(false);
        }
        let hit = self.walls.visit(|entry| -> LayoutResult<_> {
            work.visit(1)?;
            Ok(match entry {
                BvhEntry::Node(node) => Continue(ink.overlaps(node)),
                BvhEntry::Item(wall) if ink.overlaps_strictly(*wall) => Break(()),
                BvhEntry::Item(_) => Continue(true),
            })
        })?;
        if hit.is_break() {
            return Ok(false);
        }
        for &label in &self.labels {
            work.visit(1)?;
            if ink.overlaps_strictly(label) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn near_route(
        &self,
        points: &[RoutePoint],
        fraction: f64,
        markers: [f64; 2],
        size: Size,
        bounds: Bounds,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Point>> {
        let mut lengths = reserved(points.len().saturating_sub(1))?;
        let mut total = 0.0;
        for leg in points.windows(2) {
            work.visit(1)?;
            let length = (leg[1].x - leg[0].x).abs() + (leg[1].y - leg[0].y).abs();
            lengths.push(length);
            total += length;
        }
        let margin = [
            size.width / 2.0 + self.clearance,
            size.height / 2.0 + self.clearance,
        ];
        let room = [
            [bounds.x[0] + margin[0], bounds.x[1] - margin[0]],
            [bounds.y[0] + margin[1], bounds.y[1] - margin[1]],
        ];
        if room.iter().any(|[lo, hi]| lo > hi) {
            return Ok(None);
        }
        let mut next_to_leg = [None; 5];
        for (probe, t) in [
            fraction,
            (fraction + 0.25) / 2.0,
            (fraction + 0.75) / 2.0,
            0.2,
            0.8,
        ]
        .into_iter()
        .enumerate()
        {
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
                next_to_leg[probe] = Some((at, cross));
                for sign in [-1.0, 1.0] {
                    let mut candidate = at;
                    let edge = at[cross] + sign * margin[cross];
                    candidate[cross] = if sign < 0.0 {
                        edge.next_down()
                    } else {
                        edge.next_up()
                    };
                    for (coord, [lo, hi]) in candidate.iter_mut().zip(room) {
                        *coord = coord.clamp(lo, hi);
                    }
                    let candidate = point(candidate);
                    if self.fits(candidate, size, bounds, work)? {
                        return Ok(Some(candidate));
                    }
                }
                break;
            }
        }
        let mut closest = None::<(f64, Point)>;
        for (at, cross) in next_to_leg.into_iter().flatten() {
            for sign in [-1.0, 1.0] {
                for side in [0.0, -1.0, 1.0] {
                    let mut candidate = at;
                    candidate[cross] += sign * (2.0 * margin[cross]).next_up();
                    candidate[1 - cross] += side * margin[1 - cross];
                    for (coord, [lo, hi]) in candidate.iter_mut().zip(room) {
                        *coord = coord.clamp(lo, hi);
                    }
                    let distance = (candidate[0] - at[0]).hypot(candidate[1] - at[1]);
                    if closest.is_none_or(|(best, _)| distance < best)
                        && self.fits(point(candidate), size, bounds, work)?
                    {
                        closest = Some((distance, point(candidate)));
                    }
                }
            }
        }
        Ok(closest.map(|(_, at)| at))
    }
}

pub(super) fn place(
    context: &Routing<'_>,
    plan: &Plan,
    scope: usize,
    walls: &[Rect],
    routes: &mut [Routed],
    drawing_bounds: &mut Size,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let parts = &plan.scopes[scope];
    let sites = &plan.sites;
    let mut labelled = false;
    for part in parts {
        work.visit(1)?;
        labelled |= part.labels.iter().any(Option::is_some);
    }
    if !labelled {
        return Ok(());
    }
    work.charge(walls.len())?;
    let mut geometry = reserved(walls.len())?;
    geometry.extend(walls.iter().copied().map(Bounds::of));
    for (route, part) in routes.iter().zip(parts) {
        for leg in route.points.windows(2) {
            work.visit(1)?;
            push(
                &mut geometry,
                Bounds::between(position(leg[0]), position(leg[1])),
            )?;
        }
        for end in [part.from, part.to] {
            work.visit(1)?;
            let site = sites[end.site];
            if site.marker.length > 0.0 && site.marker.width > 0.0 {
                let side = if end.inner {
                    opposite(site.side)
                } else {
                    site.side
                };
                push(&mut geometry, Bounds::marker(site.at, side, site.marker))?;
            }
        }
    }
    let metrics = context.metrics;
    let scope_bounds = context.bounds(scope);
    let mut scene = Scene {
        walls: Bvh::from_bounds(geometry, work)?,
        labels: Vec::new(),
        clearance: metrics.label_clearance,
    };
    let bounds = Bounds::of(scope_bounds);
    for (route, part) in routes.iter_mut().zip(parts) {
        work.visit(1)?;
        let markers = [
            sites[part.from.site].marker.length,
            sites[part.to.site].marker.length,
        ];
        for (slot, (size, fraction)) in part.labels.into_iter().zip([0.5, 0.15, 0.85]).enumerate() {
            let Some(size) = size else {
                continue;
            };
            let at = if let Some(at) =
                scene.near_route(&route.points, fraction, markers, size, bounds, work)?
            {
                at
            } else {
                let y = scope_bounds.y + scope_bounds.height - metrics.margin() - size.height / 2.0;
                let [lo, hi] = bounds.x;
                let inset = size.width / 2.0 + scene.clearance;
                let candidates = [lo.midpoint(hi), lo + inset, hi - inset];
                let mut seat = None;
                for x in candidates {
                    let at = Point { x, y };
                    if scene.fits(at, size, bounds, work)? {
                        seat = Some(at);
                        break;
                    }
                }
                seat.unwrap_or_else(|| {
                    // Keep a label readable when every nearby channel is occupied.
                    let x = scene.clearance + size.width / 2.0;
                    let y = drawing_bounds.height + scene.clearance + size.height / 2.0;
                    drawing_bounds.height = y + size.height / 2.0 + scene.clearance;
                    drawing_bounds.width = drawing_bounds
                        .width
                        .max(x + size.width / 2.0 + scene.clearance);
                    Point { x, y }
                })
            };
            if ![at.x, at.y, drawing_bounds.width, drawing_bounds.height]
                .into_iter()
                .all(f64::is_finite)
            {
                return Err(PRECISION);
            }
            route.labels[slot] = Some(at);
            push(&mut scene.labels, Bounds::centered(at, size))?;
        }
    }
    Ok(())
}

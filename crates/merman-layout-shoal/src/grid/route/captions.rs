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
    /// Strict: clears all nodes, group frames, titles, markers, and every route (including own).
    OffAll,
    /// On-route: clears all nodes, group frames, titles, markers, and foreign routes (sits on own route).
    OnOwnRoute,
    /// Relaxed on-route: clears all leaf nodes, group titles, markers, and labels; permits crossing
    /// hollow boundary borders or foreign edges rather than detaching the label to the canvas bottom.
    OnOwnRouteClearSolids,
}

struct Scene {
    obstacles: Bvh<Obstacle>,
    labels: Vec<Bounds>,
    clearance: f64,
    is_root: bool,
}

impl Scene {
    fn fits(
        &self,
        at: Point,
        size: Size,
        bounds: Bounds,
        own_route: usize,
        tier: ClearanceTier,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        let pad = match tier {
            ClearanceTier::OffAll => self.clearance,
            ClearanceTier::OnOwnRoute | ClearanceTier::OnOwnRouteClearSolids => {
                (self.clearance * 0.5).min(4.0)
            }
        };
        let ink = Bounds::from_half_extents(at, size.width / 2.0 + pad, size.height / 2.0 + pad);
        if !ink.finite() {
            return Ok(false);
        }
        if !self.is_root
            && (ink.x[0] < bounds.x[0]
                || ink.x[1] > bounds.x[1]
                || ink.y[0] < bounds.y[0]
                || ink.y[1] > bounds.y[1])
        {
            return Ok(false);
        }
        let hit = self.obstacles.visit(|entry| -> LayoutResult<_> {
            work.visit(1)?;
            Ok(match entry {
                BvhEntry::Node(node) => Continue(ink.overlaps(node)),
                BvhEntry::Item(obs) => {
                    let blocks = match (tier, obs.kind) {
                        (ClearanceTier::OffAll, _) => true,
                        (ClearanceTier::OnOwnRoute, ObstacleKind::Route(idx)) => idx != own_route,
                        (ClearanceTier::OnOwnRoute, _) => true,
                        (ClearanceTier::OnOwnRouteClearSolids, ObstacleKind::Solid) => true,
                        (ClearanceTier::OnOwnRouteClearSolids, _) => false,
                    };
                    if blocks && ink.overlaps_strictly(obs.bounds) {
                        Break(())
                    } else {
                        Continue(true)
                    }
                }
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

    fn clamp_to_room(&self, mut candidate: [f64; 2], room: [[f64; 2]; 2]) -> Point {
        for (coord, [lo, hi]) in candidate.iter_mut().zip(room) {
            if !self.is_root {
                if lo <= hi {
                    *coord = coord.clamp(lo, hi);
                }
            } else if *coord < lo {
                *coord = lo;
            }
        }
        point(candidate)
    }

    #[allow(clippy::too_many_arguments)]
    fn near_route(
        &self,
        points: &[RoutePoint],
        route_index: usize,
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
        if !self.is_root && room.iter().any(|[lo, hi]| lo > hi) {
            return Ok(None);
        }
        let probes = [
            fraction,
            (fraction + 0.25) / 2.0,
            (fraction + 0.75) / 2.0,
            0.2,
            0.8,
        ];
        let mut samples = [None; 5];
        for (probe, t) in probes.into_iter().enumerate() {
            let Some((at, cross)) =
                Self::sample_route(points, &lengths, total, markers, t, work)?
            else {
                continue;
            };
            samples[probe] = Some((at, cross));
            // Tier 1: Beside the route, clearing all walls, borders, and routes.
            for sign in [-1.0, 1.0] {
                let mut candidate = at;
                let edge = at[cross] + sign * margin[cross];
                candidate[cross] = if sign < 0.0 {
                    edge.next_down()
                } else {
                    edge.next_up()
                };
                let candidate = self.clamp_to_room(candidate, room);
                if self.fits(
                    candidate,
                    size,
                    bounds,
                    route_index,
                    ClearanceTier::OffAll,
                    work,
                )? {
                    return Ok(Some(candidate));
                }
            }
        }
        // Tier 2: Centered directly on own route, clearing all nodes, group frames, and foreign edges.
        // Tier 3: Centered on own route, clearing all leaf nodes, titles, markers, and labels
        // (allowing wide labels like C4 captions to straddle dashed boundary borders instead of
        // detaching to the canvas bottom).
        for tier in [
            ClearanceTier::OnOwnRoute,
            ClearanceTier::OnOwnRouteClearSolids,
        ] {
            for (at, cross) in samples.into_iter().flatten() {
                let centered = self.clamp_to_room(at, room);
                if self.fits(centered, size, bounds, route_index, tier, work)? {
                    return Ok(Some(centered));
                }
                for sign in [-1.0, 1.0] {
                    let mut shifted = at;
                    shifted[cross] += sign * (size.height.min(size.width) * 0.5 + 4.0);
                    let shifted = self.clamp_to_room(shifted, room);
                    if self.fits(shifted, size, bounds, route_index, tier, work)? {
                        return Ok(Some(shifted));
                    }
                }
            }
        }
        for tier in [
            ClearanceTier::OffAll,
            ClearanceTier::OnOwnRouteClearSolids,
        ] {
            for mult in [2.0_f64, 3.0, 4.0, 5.0] {
                let mut closest = None::<(f64, Point)>;
                for (at, cross) in samples.into_iter().flatten() {
                    for sign in [-1.0, 1.0] {
                        for side in [0.0, -1.0, 1.0] {
                            let mut candidate = at;
                            candidate[cross] += sign * (mult * margin[cross]).next_up();
                            candidate[1 - cross] += side * margin[1 - cross];
                            let clamped = self.clamp_to_room(candidate, room);
                            let distance = (clamped.x - at[0]).hypot(clamped.y - at[1]);
                            if closest.is_none_or(|(best, _)| distance < best)
                                && self.fits(clamped, size, bounds, route_index, tier, work)?
                            {
                                closest = Some((distance, clamped));
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
    scope: usize,
    _walls: &[Rect],
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
    let scope_items = &context.hierarchy.scopes[scope].items;
    let is_root = scope == context.hierarchy.root();
    let total_nodes = context.drawing.nodes.len();
    let total_groups = context.drawing.groups.len();
    work.charge(scope_items.len() + total_nodes + total_groups)?;
    let mut obstacles = reserved(scope_items.len() + total_nodes + total_groups)?;
    // Always protect every leaf node and every group title as solid obstacles so relaxed on-route
    // placement can safely straddle a hollow boundary frame without ever covering a card or title.
    for &node_rect in &context.drawing.nodes {
        work.visit(1)?;
        obstacles.push(Obstacle {
            bounds: Bounds::of(node_rect),
            kind: ObstacleKind::Solid,
        });
    }
    for group in 0..total_groups {
        work.visit(1)?;
        if let Some(title) = context.title(group) {
            obstacles.push(Obstacle {
                bounds: Bounds::of(title),
                kind: ObstacleKind::Solid,
            });
        }
    }
    for &item in scope_items {
        work.visit(1)?;
        if item >= context.hierarchy.node_count {
            obstacles.push(Obstacle {
                bounds: Bounds::of(rect(context.drawing, item)),
                kind: ObstacleKind::GroupFrame,
            });
        }
    }
    for (route_index, (route, part)) in routes.iter().zip(parts).enumerate() {
        for leg in route.points.windows(2) {
            work.visit(1)?;
            push(
                &mut obstacles,
                Obstacle {
                    bounds: Bounds::between(position(leg[0]), position(leg[1])),
                    kind: ObstacleKind::Route(route_index),
                },
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
                push(
                    &mut obstacles,
                    Obstacle {
                        bounds: Bounds::marker(site.at, side, site.marker),
                        kind: ObstacleKind::Solid,
                    },
                )?;
            }
        }
    }
    let metrics = context.metrics;
    let scope_bounds = context.bounds(scope);
    let mut scene = Scene {
        obstacles: Bvh::build_with_work(obstacles, |obs| obs.bounds, work)?,
        labels: Vec::new(),
        clearance: metrics.label_clearance,
        is_root,
    };
    let bounds = Bounds::of(scope_bounds);
    for (route_index, (route, part)) in routes.iter_mut().zip(parts).enumerate() {
        work.visit(1)?;
        let markers = [
            sites[part.from.site].marker.length,
            sites[part.to.site].marker.length,
        ];
        for (slot, (size, fraction)) in part.labels.into_iter().zip([0.5, 0.15, 0.85]).enumerate() {
            let Some(size) = size else {
                continue;
            };
            let at = if let Some(at) = scene.near_route(
                &route.points,
                route_index,
                fraction,
                markers,
                size,
                bounds,
                work,
            )? {
                if is_root {
                    drawing_bounds.width = drawing_bounds
                        .width
                        .max(at.x + size.width / 2.0 + scene.clearance);
                    drawing_bounds.height = drawing_bounds
                        .height
                        .max(at.y + size.height / 2.0 + scene.clearance);
                }
                at
            } else {
                let y = scope_bounds.y + scope_bounds.height - metrics.margin() - size.height / 2.0;
                let [lo, hi] = bounds.x;
                let inset = size.width / 2.0 + scene.clearance;
                let candidates = [lo.midpoint(hi), lo + inset, hi - inset];
                let mut seat = None;
                for x in candidates {
                    let at = Point { x, y };
                    if scene.fits(at, size, bounds, route_index, ClearanceTier::OffAll, work)? {
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

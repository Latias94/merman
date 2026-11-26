//! A scope containing a standalone fan can share a guarded crossbar.

use super::*;
use crate::MIN_MARKER_LEAD;
use crate::ortho::comb::{Comb, MIN_DEGREE};

pub(super) fn plan(
    context: &Routing<'_>,
    plan: &Plan,
    origin: Point,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<(usize, Vec<RoutePoint>)>> {
    match work.compare(|trial| {
        let mut edits = Vec::new();
        for (scope, parts) in plan.scopes.iter().enumerate() {
            trial.visit(1)?;
            let Some(fan) = Fan::new(context, &plan.sites, parts, origin) else {
                continue;
            };
            let bounds = context.bounds(scope);
            if let Some(branches) = fan.route(scope, bounds, trial)? {
                trial.charge(parts.len())?;
                edits
                    .try_reserve(parts.len())
                    .map_err(|_| LayoutError::AllocationLimit)?;
                edits.extend(parts.iter().map(|part| part.edge).zip(branches));
            }
        }
        Ok(edits)
    }) {
        Ok(edits) => Ok(edits.unwrap_or_default()),
        Err(LayoutError::AllocationLimit | LayoutError::InvalidInput(_)) => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

fn node_contact(context: &Routing<'_>, site: Site) -> bool {
    !site.inner && site.owner < context.hierarchy.node_count
}

struct Fan<'a> {
    context: &'a Routing<'a>,
    sites: &'a [Site],
    parts: &'a [Part],
    origin: Point,
    hub_end: usize,
    hub: Site,
}

impl<'a> Fan<'a> {
    fn new(
        context: &'a Routing<'a>,
        sites: &'a [Site],
        parts: &'a [Part],
        origin: Point,
    ) -> Option<Self> {
        if parts.len() < MIN_DEGREE {
            return None;
        }
        let first = [parts[0].from, parts[0].to].map(|end| translated(sites[end.site], origin));
        let hub_end = first.iter().position(|site| {
            node_contact(context, *site)
                && site.marker == MarkerMetrics::default()
                && site.lane.degree == parts.len()
        })?;
        Some(Self {
            context,
            sites,
            parts,
            origin,
            hub_end,
            hub: first[hub_end],
        })
    }

    fn lead(&self, site: Site, runway: f64) -> RoutePoint {
        let (mut lead, _) = self.context.lead(site, false);
        let (axis, sign) = normal(site.side);
        let sign = sign as f64;
        let tip = [site.at.x, site.at.y][axis] * sign;
        let travel = if axis == 0 { &mut lead.x } else { &mut lead.y };
        *travel = (*travel * sign).max(tip + runway.max(MIN_MARKER_LEAD)) * sign;
        lead
    }

    fn route(
        self,
        scope: usize,
        bounds: Rect,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Vec<Vec<RoutePoint>>>> {
        let hub_side = self.hub.side;
        let leaf_side = opposite(hub_side);
        let (axis, sign) = normal(hub_side);
        let depth = |p: RoutePoint| [p.x, p.y][axis] * sign as f64;
        let frame = Bounds::of(bounds);
        let [_, center, _] = self.context.seat(self.hub.owner, hub_side, false);
        let distance = |site: Site| (cross(hub_side, site.at) - center).abs();
        let mut hub = self.hub;
        work.charge(self.parts.len())?;
        let mut leaves = reserved(self.parts.len())?;
        let mut limit = f64::INFINITY;
        for part in self.parts {
            work.visit(1)?;
            let ends = [part.from, part.to];
            let [contact, leaf] = [ends[self.hub_end], ends[1 - self.hub_end]]
                .map(|end| translated(self.sites[end.site], self.origin));
            if contact.owner != hub.owner
                || contact.side != hub_side
                || contact.offset != hub.offset
                || !node_contact(self.context, contact)
                || contact.marker != MarkerMetrics::default()
                || leaf.owner == hub.owner
                || leaf.side != leaf_side
                || !node_contact(self.context, leaf)
                || part.labels.iter().any(Option::is_some)
            {
                return Ok(None);
            }
            if distance(contact) < distance(hub) {
                hub = contact;
            }
            let runway = self.context.graph.edges[part.edge].marker_runways()[1 - self.hub_end];
            let lead = self.lead(leaf, runway);
            leaves.push((leaf, lead));
            limit = limit.min(depth(lead));
        }
        let lead = self.lead(hub, 0.0);
        let start = depth(lead);
        let shelf = start.midpoint(limit);
        let corner = self.context.metrics.separation * 0.5;
        if (shelf - start).min(limit - shelf) < corner || !shelf.is_finite() {
            return Ok(None);
        }
        work.charge(leaves.len())?;
        let mut order = reserved(leaves.len())?;
        order.extend(0..leaves.len());
        let track = |id: usize| cross(leaf_side, leaves[id].0.at);
        work.sort(&mut order, |&a, &b| {
            track(a).total_cmp(&track(b)).then(a.cmp(&b))
        })?;
        work.visit(order.len())?;
        for pair in order.windows(2) {
            let [a, b] = [pair[0], pair[1]];
            let distance = track(b) - track(a);
            let markers = leaves[a].0.marker.width.midpoint(leaves[b].0.marker.width);
            if distance <= 0.0 || distance < markers {
                return Ok(None);
            }
        }
        let walls = self.context.walls(scope, work)?;
        let router = OrthoRouter::new(&walls, self.context.metrics.separation, 0, work)?;
        if !self.terminal_is_clear(hub, lead, &router, frame, work)? {
            return Ok(None);
        }
        let comb = Comb {
            travel: hub_side.axis(),
            shelf: shelf * sign as f64,
            corner,
        };
        work.charge(leaves.len())?;
        let mut routes = reserved(leaves.len())?;
        for (leaf, end) in leaves {
            let middle = dogleg(lead, end, comb.travel, comb.shelf);
            if !clear(&router, &middle, bounds, [hub_side, leaf_side], work)?
                || !self.terminal_is_clear(leaf, end, &router, frame, work)?
            {
                return Ok(None);
            }
            let Some(points) =
                comb.branch(hub.at, position(lead), leaf.at, self.hub_end == 1, work)?
            else {
                return Ok(None);
            };
            if !corners_are_clear(&router, &points, frame, work)? {
                return Ok(None);
            }
            routes.push(points);
        }
        Ok(Some(routes))
    }

    fn terminal_is_clear(
        &self,
        site: Site,
        lead: RoutePoint,
        router: &OrthoRouter,
        frame: Bounds,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        let footprint = Bounds::between(site.at, position(lead)).union(Bounds::marker(
            site.at,
            site.side,
            site.marker,
        ));
        router.area_is_clear(
            footprint,
            frame,
            Some(rect(self.context.drawing, site.owner)),
            work,
        )
    }
}

fn translated(mut site: Site, origin: Point) -> Site {
    site.at.x -= origin.x;
    site.at.y -= origin.y;
    site
}

fn corners_are_clear(
    router: &OrthoRouter,
    points: &[RoutePoint],
    frame: Bounds,
    work: &mut Work<'_>,
) -> LayoutResult<bool> {
    work.visit(points.len())?;
    for corner in points.windows(3) {
        let [before, bend, after] = [corner[0], corner[1], corner[2]];
        if bend.axis != after.axis
            && !router.area_is_clear(
                Bounds::between(position(before), position(after)),
                frame,
                None,
                work,
            )?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoopWorkControl;

    #[test]
    fn an_inside_halo_can_block_a_corner_with_clear_straight_legs() {
        let bounds = Rect {
            x: -20.0,
            y: -20.0,
            width: 100.0,
            height: 100.0,
        };
        let wall = Rect {
            x: 21.0,
            y: 8.0,
            width: 1.0,
            height: 1.0,
        };
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let router = OrthoRouter::new(&[wall], 14.0, 0, &mut work).unwrap();
        let hub = RoutePoint::new(0.0, 0.0, Axis::Horizontal);
        let leaf = RoutePoint::new(60.0, 30.0, Axis::Horizontal);
        let middle = dogleg(hub, leaf, Axis::Horizontal, 30.0);
        assert!(
            clear(
                &router,
                &middle,
                bounds,
                [Side::Right, Side::Left],
                &mut work
            )
            .unwrap()
        );
        let comb = Comb {
            travel: Axis::Horizontal,
            shelf: 30.0,
            corner: 7.0,
        };
        let points = comb
            .branch(
                position(hub),
                position(hub),
                position(leaf),
                false,
                &mut work,
            )
            .unwrap()
            .unwrap();
        assert!(!corners_are_clear(&router, &points, Bounds::of(bounds), &mut work).unwrap());
    }
}

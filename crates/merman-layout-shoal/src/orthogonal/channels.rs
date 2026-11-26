//! Separate coincident parallel legs without moving their neighbors' fixed knots.

use super::*;

const MAX_CHANNEL_ROUTES: usize = 32;

#[derive(Clone, Copy)]
struct Leg {
    edge: usize,
    at: usize,
    line: OrthoSegment,
}

struct Lane {
    leg: Leg,
    approach: [RoutePoint; 2],
    room: [f64; 2],
}

pub(super) struct Scene<'a> {
    pub(super) router: &'a OrthoRouter,
    pub(super) graph: &'a Graph,
    pub(super) drawing: &'a LayoutOutput,
    pub(super) bounds: Rect,
    pub(super) spacing: f64,
}

impl Scene<'_> {
    pub(super) fn nudge(
        &self,
        edges: &[usize],
        routes: &mut Routes<'_>,
        work: &mut Work<'_>,
    ) -> LayoutResult<u64> {
        if self.spacing <= 0.0 || edges.len() < 2 {
            return Ok(0);
        }
        let mut legs = Vec::new();
        for &edge in edges {
            for (from, joint) in routes.projections[edge]
                .legs
                .as_slice()
                .windows(3)
                .enumerate()
            {
                work.visit(1)?;
                if let [
                    ProjectedLeg::Orthogonal(before),
                    ProjectedLeg::Orthogonal(line),
                    ProjectedLeg::Orthogonal(after),
                ] = *joint
                    && before.axis != line.axis
                    && after.axis != line.axis
                {
                    push(
                        &mut legs,
                        Leg {
                            edge,
                            at: from + 1,
                            line,
                        },
                    )?;
                }
            }
        }
        work.charge_sort(legs.len())?;
        legs.sort_unstable_by(|a, b| {
            (a.line.axis == Axis::Vertical)
                .cmp(&(b.line.axis == Axis::Vertical))
                .then_with(|| a.line.fixed.total_cmp(&b.line.fixed))
                .then_with(|| a.line.min.total_cmp(&b.line.min))
                .then_with(|| a.edge.cmp(&b.edge))
                .then_with(|| a.at.cmp(&b.at))
        });
        let mut removed = 0;
        for track in
            legs.chunk_by(|a, b| a.line.axis == b.line.axis && a.line.fixed == b.line.fixed)
        {
            let mut first = 0;
            while first < track.len() {
                let mut end = first + 1;
                let mut extent = track[first].line.max;
                while end < track.len() && track[end].line.min < extent {
                    work.visit(1)?;
                    extent = extent.max(track[end].line.max);
                    end += 1;
                }
                if (2..=MAX_CHANNEL_ROUTES).contains(&(end - first)) {
                    removed += self.spread(&track[first..end], routes, work)?;
                }
                first = end;
            }
        }
        Ok(removed)
    }

    fn spread(
        &self,
        legs: &[Leg],
        routes: &mut Routes<'_>,
        work: &mut Work<'_>,
    ) -> LayoutResult<u64> {
        let mut lanes = reserved(legs.len())?;
        let mut common = [f64::NEG_INFINITY, f64::INFINITY];
        for &leg in legs {
            work.visit(lanes.len() + 1)?;
            if lanes.iter().any(|lane: &Lane| lane.leg.edge == leg.edge) {
                return Ok(0);
            }
            let Some(lane) = self.lane(leg, routes, work)? else {
                return Ok(0);
            };
            common[0] = common[0].max(lane.room[0]);
            common[1] = common[1].min(lane.room[1]);
            lanes.push(lane);
        }
        let intervals = (lanes.len() - 1) as f64;
        let pitch = self.spacing.min((common[1] - common[0]) / intervals);
        if !pitch.is_finite() || pitch <= 0.0 {
            return Ok(0);
        }
        let Some(order) = order(&lanes, work)? else {
            return Ok(0);
        };
        let width = pitch * intervals;
        let start = (legs[0].line.fixed - width * 0.5)
            .max(common[0])
            .min((common[1] - width).max(common[0]));
        let mut last = f64::NEG_INFINITY;
        let mut proposed = reserved(lanes.len())?;
        for (rank, lane) in order.into_iter().map(|i| &lanes[i]).enumerate() {
            let target = (rank as f64).mul_add(pitch, start).min(common[1]);
            if target <= last {
                return Ok(0);
            }
            last = target;
            let Some(points) = self.shift(lane, target, routes, work)? else {
                return Ok(0);
            };
            proposed.push((lane.leg.edge, Candidate::new(points, work)?));
        }
        work.charge_sort(proposed.len())?;
        proposed.sort_unstable_by_key(|(edge, _)| *edge);
        let (before, after) = self.conflicts(&proposed, routes, work)?;
        if after.0 <= before.0 && after.1 < before.1 {
            for (edge, candidate) in proposed {
                routes.replace(edge, candidate);
            }
            return Ok(before.0 - after.0);
        }
        Ok(0)
    }

    fn lane(
        &self,
        leg: Leg,
        routes: &Routes<'_>,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Lane>> {
        let route = routes.points(leg.edge);
        let before = route[leg.at - 1];
        let after = route[leg.at + 2];
        // A neighboring channel may have changed one of these corners in a previous trial.
        let [
            ProjectedLeg::Orthogonal(incoming),
            ProjectedLeg::Orthogonal(line),
            ProjectedLeg::Orthogonal(outgoing),
        ] = routes.projections[leg.edge].legs.as_slice()[leg.at - 1..leg.at + 2]
        else {
            return Ok(None);
        };
        if line != leg.line || incoming.axis == line.axis || outgoing.axis == line.axis {
            return Ok(None);
        }
        let Some(mut room) = self.router.channel(leg.line, Some(self.bounds), work)? else {
            return Ok(None);
        };
        let spec = &self.graph.edges[leg.edge];
        let first = (leg.at == 1).then_some(spec.start_marker);
        let last = (leg.at + 3 == route.len()).then_some(spec.end_marker);
        for (joint, terminal) in [(before, first), (after, last)] {
            let at = across(leg.line.axis, joint);
            let lead = terminal.map_or(0.0, |marker| marker.length.max(MIN_MARKER_LEAD));
            if at < leg.line.fixed {
                room[0] = room[0].max((at + lead).max(at.next_up()));
            } else {
                room[1] = room[1].min((at - lead).min(at.next_down()));
            }
        }
        Ok((room[0] <= room[1]).then_some(Lane {
            leg,
            approach: [before, after],
            room,
        }))
    }

    fn shift(
        &self,
        lane: &Lane,
        target: f64,
        routes: &Routes<'_>,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Vec<RoutePoint>>> {
        let Leg { edge, at, line } = lane.leg;
        let route = routes.points(edge);
        for (i, approach) in (at..at + 2).zip(lane.approach) {
            let old = route[i];
            let new = shifted(line.axis, old, target);
            // Shortening a neighboring leg adds no ink. Check any newly extended interval.
            if (target - across(line.axis, approach)).abs()
                > (line.fixed - across(line.axis, approach)).abs()
                && let Some(extension) = OrthoSegment::between(old, new)
                && self
                    .router
                    .channel(extension, Some(self.bounds), work)?
                    .is_none()
            {
                return Ok(None);
            }
        }
        work.charge(route.len())?;
        let mut output = reserved(route.len())?;
        output.extend_from_slice(route);
        output[at] = shifted(line.axis, output[at], target);
        output[at + 1] = shifted(line.axis, output[at + 1], target);
        Ok(extent_preserved(route, &output, root_bounds(self.drawing), work)?.then_some(output))
    }

    fn conflicts(
        &self,
        proposed: &[(usize, Candidate)],
        routes: &Routes<'_>,
        work: &mut Work<'_>,
    ) -> LayoutResult<((u64, usize), (u64, usize))> {
        let mut before = (0, 0);
        let mut after = (0, 0);
        for (edge, candidate) in proposed {
            let original = routes.view(*edge);
            let current_bounds = routes.edge_bounds[*edge];
            let mut pending = proposed.iter().peekable();
            work.visit(routes.edge_bounds.len())?;
            for (other, &prior_bounds) in routes.edge_bounds.iter().enumerate() {
                let replacement = pending.next_if(|(index, _)| *index == other);
                // Compare each pair of simultaneously moved routes once.
                if replacement.is_some() && other <= *edge {
                    continue;
                }
                if current_bounds.overlaps(prior_bounds) {
                    let (crossings, overlaps) = intersections(original, routes.view(other), work)?;
                    before.0 += crossings;
                    before.1 += overlaps;
                }
                let next_bounds = replacement.map_or(prior_bounds, |(_, route)| route.bounds);
                if candidate.bounds.overlaps(next_bounds) {
                    let next =
                        replacement.map_or_else(|| routes.view(other), |(_, route)| route.view());
                    let (crossings, overlaps) = intersections(candidate.view(), next, work)?;
                    after.0 += crossings;
                    after.1 += overlaps;
                }
            }
        }
        Ok((before, after))
    }
}

fn across(axis: Axis, point: RoutePoint) -> f64 {
    match axis {
        Axis::Horizontal => point.y,
        Axis::Vertical => point.x,
    }
}

fn along(axis: Axis, point: RoutePoint) -> f64 {
    match axis {
        Axis::Horizontal => point.x,
        Axis::Vertical => point.y,
    }
}

fn shifted(axis: Axis, point: RoutePoint, fixed: f64) -> RoutePoint {
    match axis {
        Axis::Horizontal => RoutePoint { y: fixed, ..point },
        Axis::Vertical => RoutePoint { x: fixed, ..point },
    }
}

fn order(lanes: &[Lane], work: &mut Work<'_>) -> LayoutResult<Option<Vec<usize>>> {
    // A leg whose approach is inside a peer's span must stay on its approach side of that peer.
    // A cyclic ordering requires another route, so leave that channel to the visibility pass.
    let n = lanes.len();
    let mut precedes = filled(n, 0u32)?;
    let mut incoming = filled(n, 0u32)?;
    for (a, lane) in lanes.iter().enumerate() {
        let line = lane.leg.line;
        for (b, peer) in lanes.iter().enumerate().filter(|(b, _)| *b != a) {
            for joint in lane.approach {
                work.visit(1)?;
                let at = along(line.axis, joint);
                if peer.leg.line.min < at && at < peer.leg.line.max {
                    let (from, to) = if across(line.axis, joint) < line.fixed {
                        (a, b)
                    } else {
                        (b, a)
                    };
                    let bit = 1 << to;
                    if precedes[from] & bit == 0 {
                        precedes[from] |= bit;
                        incoming[to] += 1;
                    }
                }
            }
        }
    }
    let mut order = reserved(n)?;
    let mut taken = 0u32;
    for _ in 0..n {
        work.visit(n)?;
        let next = (0..n)
            .filter(|&i| taken & (1 << i) == 0 && incoming[i] == 0)
            .min_by_key(|&i| {
                let lane = &lanes[i];
                let line = lane.leg.line;
                let down = lane
                    .approach
                    .into_iter()
                    .filter(|&p| across(line.axis, p) > line.fixed)
                    .count();
                (down, lane.leg.edge, lane.leg.at)
            });
        let Some(next) = next else { return Ok(None) };
        taken |= 1 << next;
        order.push(next);
        let mut children = precedes[next];
        while children != 0 {
            work.visit(1)?;
            incoming[children.trailing_zeros() as usize] -= 1;
            children &= children - 1;
        }
    }
    Ok(Some(order))
}

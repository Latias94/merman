use super::*;
use crate::geom::{Bvh, BvhEntry};
use std::ops::ControlFlow::{Break, Continue};

#[derive(Clone, Copy)]
struct SolidObstacle {
    bounds: Bounds,
    is_frame: bool,
}

#[derive(Clone, Copy)]
struct RouteObstacle {
    bounds: Bounds,
    edge: usize,
}

#[derive(Clone, Copy)]
enum ClearanceTier {
    OffAll,
    OnOwnRoute,
    /// The hollow group border can cross the caption in a crowded gutter.
    AcrossFrame,
    /// Last-resort local placement on a trapped parallel wire: clears every solid obstacle,
    /// title, marker, and previously placed caption while allowing foreign route lines behind it.
    AcrossForeignRoute,
    /// Extended local rescue for heavily congested gutters before falling back to distant
    /// scope-bottom or diagram-bottom rows: keeps a thin clearance margin from solids/captions.
    TightAcrossForeignRoute,
}

#[derive(Clone, Copy)]
enum FitResult {
    Clear,
    /// The caption ink clears all required obstacles, but its tether crosses this foreign route leg.
    TetherBlocked(Bounds),
    Blocked,
}

struct Scene {
    solids: Bvh<SolidObstacle>,
    routes: Bvh<RouteObstacle>,
    labels: Vec<Caption>,
    clearance: f64,
}

struct Caption {
    ink: Bounds,
    prefix_bottom: f64,
}

/// Returns `true` if the straight tether segment `from -> to` crosses `bounds` strictly
/// after its anchor `from` (`t > 1e-6`), separating a caption candidate from its own route.
fn segment_crosses_bounds(from: Point, to: Point, bounds: Bounds) -> bool {
    if !from.x.is_finite()
        || !from.y.is_finite()
        || !to.x.is_finite()
        || !to.y.is_finite()
        || !bounds.finite()
    {
        return false;
    }
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    if !dx.is_finite() || !dy.is_finite() || (dx == 0.0 && dy == 0.0) {
        return false;
    }
    let mut t_min = 0.0_f64;
    let mut t_max = 1.0_f64;
    for (origin, delta, [lo, hi]) in [(from.x, dx, bounds.x), (from.y, dy, bounds.y)] {
        if delta == 0.0 {
            if origin < lo || origin > hi {
                return false;
            }
        } else {
            let t0 = (lo - origin) / delta;
            let t1 = (hi - origin) / delta;
            if !t0.is_finite() || !t1.is_finite() {
                return false;
            }
            t_min = t_min.max(t0.min(t1));
            t_max = t_max.min(t0.max(t1));
            if t_min > t_max {
                return false;
            }
        }
    }
    t_max > 1e-6 && t_min <= 1.0
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

    #[allow(clippy::too_many_arguments)]
    fn fits(
        &self,
        at: Point,
        size: Size,
        scope: Option<Bounds>,
        own_route: usize,
        tether: Option<Point>,
        tier: ClearanceTier,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        Ok(matches!(
            self.fits_classified(at, size, scope, own_route, tether, &[], tier, work)?,
            FitResult::Clear
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn fits_classified(
        &self,
        at: Point,
        size: Size,
        scope: Option<Bounds>,
        own_route: usize,
        tether: Option<Point>,
        cached_blockers: &[Bounds],
        tier: ClearanceTier,
        work: &mut Work<'_>,
    ) -> LayoutResult<FitResult> {
        let pad = match tier {
            ClearanceTier::OffAll => self.clearance,
            ClearanceTier::OnOwnRoute
            | ClearanceTier::AcrossFrame
            | ClearanceTier::AcrossForeignRoute => (self.clearance * 0.5).min(4.0),
            ClearanceTier::TightAcrossForeignRoute => (self.clearance * 0.25).min(1.5),
        };
        let ink = Bounds::from_half_extents(at, size.width / 2.0 + pad, size.height / 2.0 + pad);
        if !ink.finite() {
            return Ok(FitResult::Blocked);
        }
        if let Some(bounds) = scope
            && (ink.x[0] < bounds.x[0]
                || ink.x[1] > bounds.x[1]
                || ink.y[0] < bounds.y[0]
                || ink.y[1] > bounds.y[1])
        {
            return Ok(FitResult::Blocked);
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
                    return Ok(FitResult::Blocked);
                }
            }
        }
        let allow_frame = matches!(
            tier,
            ClearanceTier::AcrossFrame
                | ClearanceTier::AcrossForeignRoute
                | ClearanceTier::TightAcrossForeignRoute
        );
        // Phase 1a: check solid nodes, titles, markers, and (unless relaxed) hollow group frames.
        let hit_solid = self.solids.visit(|entry| -> LayoutResult<_> {
            work.visit(1)?;
            Ok(match entry {
                BvhEntry::Node(node) => Continue(ink.overlaps_strictly(node)),
                BvhEntry::Item(obs) => {
                    if (!obs.is_frame || !allow_frame) && ink.overlaps_strictly(obs.bounds) {
                        Break(())
                    } else {
                        Continue(true)
                    }
                }
            })
        })?;
        if hit_solid.is_break() {
            return Ok(FitResult::Blocked);
        }
        // Phase 1b: tight `ink`-only check against route segments (skipped in foreign-route tiers).
        if !matches!(
            tier,
            ClearanceTier::AcrossForeignRoute | ClearanceTier::TightAcrossForeignRoute
        ) {
            let block_own = matches!(tier, ClearanceTier::OffAll);
            let hit_route = self.routes.visit(|entry| -> LayoutResult<_> {
                work.visit(1)?;
                Ok(match entry {
                    BvhEntry::Node(node) => Continue(ink.overlaps_strictly(node)),
                    BvhEntry::Item(obs) => {
                        if (block_own || obs.edge != own_route) && ink.overlaps_strictly(obs.bounds)
                        {
                            Break(())
                        } else {
                            Continue(true)
                        }
                    }
                })
            })?;
            if hit_route.is_break() {
                return Ok(FitResult::Blocked);
            }
        }
        // Phase 2: only after the tight ink box is clear, verify that the straight tether from
        // the route anchor to `at` does not hop across a foreign route in the routes-only BVH.
        if let Some(origin) = tether
            && (origin.x != at.x || origin.y != at.y)
        {
            for &blocker in cached_blockers {
                if segment_crosses_bounds(origin, at, blocker) {
                    return Ok(FitResult::TetherBlocked(blocker));
                }
            }
            let tether_bounds = Bounds::between(origin, at);
            let mut blocker = None;
            let crossed = self.routes.visit(|entry| -> LayoutResult<_> {
                work.visit(1)?;
                Ok(match entry {
                    BvhEntry::Node(node) => Continue(tether_bounds.overlaps(node)),
                    BvhEntry::Item(obs) => {
                        if obs.edge != own_route && segment_crosses_bounds(origin, at, obs.bounds) {
                            blocker = Some(obs.bounds);
                            Break(())
                        } else {
                            Continue(true)
                        }
                    }
                })
            })?;
            if crossed.is_break()
                && let Some(blocker) = blocker
            {
                return Ok(FitResult::TetherBlocked(blocker));
            }
        }
        Ok(FitResult::Clear)
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
        // Small exact cache of foreign route segments that already blocked a tether on this route,
        // plus the best Phase-1-clear rescue candidate (lower priority rank wins, then distance).
        let mut tether_blockers = [Bounds {
            x: [0.0; 2],
            y: [0.0; 2],
        }; 4];
        let mut blocker_count = 0usize;
        let mut rescue = None::<(u8, f64, Point)>;
        let mut seat = |mut candidate: [f64; 2],
                        sample: [f64; 2],
                        tier,
                        check_tether: bool,
                        rescue_rank: Option<u8>,
                        rescue: &mut Option<(u8, f64, Point)>,
                        best: Option<f64>,
                        work: &mut Work<'_>| {
            work.visit(1)?;
            let pad = match tier {
                ClearanceTier::OffAll => self.clearance,
                ClearanceTier::TightAcrossForeignRoute => (self.clearance * 0.25).min(1.5),
                _ => (self.clearance * 0.5).min(4.0),
            };
            for ((coord, [lo, hi]), half) in candidate
                .iter_mut()
                .zip(room)
                .zip([size.width / 2.0 + pad, size.height / 2.0 + pad])
            {
                let min_coord = lo + half;
                let max_coord = hi - half;
                if min_coord.is_nan() || max_coord.is_nan() || min_coord > max_coord {
                    return Ok(None);
                }
                *coord = coord.clamp(min_coord, max_coord);
            }
            let [x, y] = candidate;
            let at = Point { x, y };
            let distance = (x - sample[0]).hypot(y - sample[1]);
            if !distance.is_finite()
                || distance > radius
                || best.is_some_and(|best| distance >= best)
            {
                return Ok(None);
            }
            let tether = check_tether.then_some(Point {
                x: sample[0],
                y: sample[1],
            });
            let can_improve_rescue = rescue_rank.is_some_and(|rank| {
                rescue.is_none_or(|(best_rank, best_dist, _)| {
                    rank < best_rank || (rank == best_rank && distance < best_dist)
                })
            });
            // Exact fast-reject: if a cached foreign route already crosses this tether, the
            // candidate can never be `FitResult::Clear`; when it also cannot improve `rescue`,
            // skip both BVH traversals completely.
            if let Some(origin) = tether
                && !can_improve_rescue
                && (origin.x != at.x || origin.y != at.y)
                && tether_blockers[..blocker_count]
                    .iter()
                    .any(|&blocker| segment_crosses_bounds(origin, at, blocker))
            {
                return Ok(None);
            }
            match self.fits_classified(
                at,
                size,
                scope,
                route_index,
                tether,
                &tether_blockers[..blocker_count],
                tier,
                work,
            )? {
                FitResult::Clear => Ok::<_, LayoutError>(Some((distance, at))),
                FitResult::TetherBlocked(blocker) => {
                    if blocker_count < tether_blockers.len()
                        && !tether_blockers[..blocker_count]
                            .iter()
                            .any(|b| b.x == blocker.x && b.y == blocker.y)
                    {
                        tether_blockers[blocker_count] = blocker;
                        blocker_count += 1;
                    }
                    if let Some(rank) = rescue_rank
                        && can_improve_rescue
                    {
                        *rescue = Some((rank, distance, at));
                    }
                    Ok(None)
                }
                FitResult::Blocked => Ok(None),
            }
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
                if let Some((_, candidate)) = seat(
                    candidate,
                    at,
                    ClearanceTier::OffAll,
                    true,
                    Some(0),
                    &mut rescue,
                    None,
                    work,
                )? {
                    return Ok(Some(candidate));
                }
            }
        }
        for (tier_idx, tier) in [ClearanceTier::OnOwnRoute, ClearanceTier::AcrossFrame]
            .into_iter()
            .enumerate()
        {
            let rank = 1 + tier_idx as u8;
            for (at, cross) in samples.into_iter().flatten() {
                for sign in [0.0, -1.0, 1.0] {
                    let mut shifted = at;
                    shifted[cross] += sign * (size.height.min(size.width) * 0.5 + 4.0);
                    if let Some((_, shifted)) =
                        seat(shifted, at, tier, true, Some(rank), &mut rescue, None, work)?
                    {
                        return Ok(Some(shifted));
                    }
                }
            }
        }
        for (tier_idx, tier) in [ClearanceTier::OffAll, ClearanceTier::AcrossFrame]
            .into_iter()
            .enumerate()
        {
            for (ring_idx, (mult, radius_fraction)) in
                [(2.0, 0.0), (3.0, 0.0), (0.0, 0.5), (0.0, 0.875)]
                    .into_iter()
                    .enumerate()
            {
                let rank = (ring_idx < 3).then_some(3 + (tier_idx * 3 + ring_idx) as u8);
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
                            if let Some(seat) = seat(
                                candidate,
                                at,
                                tier,
                                true,
                                rank,
                                &mut rescue,
                                closest.map(|(best, _)| best),
                                work,
                            )? {
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
        // Local rescue for trapped wires (such as the center wire of a parallel bundle or a tight
        // gutter) before falling back to distant scope-bottom or diagram-margin rows: first use
        // any Phase-1-clear candidate already discovered above (with only tether relaxed), and
        // otherwise allow foreign route lines behind the caption while still clearing every solid
        // node, title, marker, and previously placed caption.
        if let Some((_, _, at)) = rescue {
            return Ok(Some(at));
        }
        for (at, cross) in samples.into_iter().flatten() {
            for sign in [0.0, -1.0, 1.0] {
                let mut shifted = at;
                shifted[cross] += sign * (size.height.min(size.width) * 0.5 + 4.0);
                if let Some((_, shifted)) = seat(
                    shifted,
                    at,
                    ClearanceTier::AcrossForeignRoute,
                    false,
                    None,
                    &mut rescue,
                    None,
                    work,
                )? {
                    return Ok(Some(shifted));
                }
            }
        }
        for (mult, radius_fraction) in [(2.0, 0.0), (3.0, 0.0), (0.0, 0.5)] {
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
                        if let Some(seat) = seat(
                            candidate,
                            at,
                            ClearanceTier::AcrossForeignRoute,
                            false,
                            None,
                            &mut rescue,
                            closest.map(|(best, _)| best),
                            work,
                        )? {
                            closest = Some(seat);
                        }
                    }
                }
            }
            if let Some((_, at)) = closest {
                return Ok(Some(at));
            }
        }
        self.extended_local_rescue(
            points,
            lengths,
            total,
            markers,
            route_index,
            size,
            scope,
            radius,
            work,
        )
    }

    /// Extended local rescue invoked only when standard near-route tiers fail, avoiding
    /// 200-600 px teleports to scope-bottom or diagram-bottom rows in congested gutters.
    #[allow(clippy::too_many_arguments)]
    fn extended_local_rescue(
        &self,
        points: &[RoutePoint],
        lengths: &[f64],
        total: f64,
        markers: [f64; 2],
        route_index: usize,
        size: Size,
        scope: Option<Bounds>,
        base_radius: f64,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Point>> {
        let mut anchors = [([0.0; 2], 0usize); 11];
        let mut anchor_count = 0usize;
        let mut push_anchor = |at: [f64; 2], cross: usize| {
            if anchor_count < anchors.len()
                && !anchors[..anchor_count]
                    .iter()
                    .any(|(prev, _)| (prev[0] - at[0]).abs() < 4.0 && (prev[1] - at[1]).abs() < 4.0)
            {
                anchors[anchor_count] = (at, cross);
                anchor_count += 1;
            }
        };
        // Prioritize the longest route legs first so multi-leg routes sample open corridor runs.
        let mut longest = [(0.0_f64, 0usize); 4];
        for (idx, &len) in lengths.iter().enumerate() {
            work.visit(1)?;
            if len <= 0.0 {
                continue;
            }
            for slot in 0..longest.len() {
                if len > longest[slot].0 {
                    for shift in (slot + 1..longest.len()).rev() {
                        longest[shift] = longest[shift - 1];
                    }
                    longest[slot] = (len, idx);
                    break;
                }
            }
        }
        for (len, idx) in longest {
            if len <= 0.0 {
                continue;
            }
            let a = points[idx];
            let b = points[idx + 1];
            let mid = [(a.x + b.x) * 0.5, (a.y + b.y) * 0.5];
            let cross = usize::from(a.y == b.y);
            push_anchor(mid, cross);
        }
        for t in [0.5, 0.35, 0.65, 0.2, 0.8, 0.1, 0.9] {
            if let Some((at, cross)) = Self::sample_route(points, lengths, total, markers, t, work)?
            {
                push_anchor(at, cross);
            }
        }
        if anchor_count == 0 {
            return Ok(None);
        }
        let extended_radius = (base_radius * 2.0).max(240.0);
        let room = scope.map_or([[0.0, f64::INFINITY]; 2], |bounds| [bounds.x, bounds.y]);
        for tier in [
            ClearanceTier::OffAll,
            ClearanceTier::AcrossFrame,
            ClearanceTier::AcrossForeignRoute,
            ClearanceTier::TightAcrossForeignRoute,
        ] {
            let pad = match tier {
                ClearanceTier::OffAll => self.clearance,
                ClearanceTier::TightAcrossForeignRoute => (self.clearance * 0.25).min(1.5),
                _ => (self.clearance * 0.5).min(4.0),
            };
            let step = [size.width / 2.0 + pad, size.height / 2.0 + pad];
            let mut closest = None::<(f64, Point)>;
            for &(sample, cross) in &anchors[..anchor_count] {
                let along_base = step[1 - cross].min(extended_radius * 0.5);
                for (mult, radius_fraction) in [
                    (0.0, 0.0),
                    (1.0, 0.0),
                    (1.5, 0.0),
                    (2.0, 0.0),
                    (2.5, 0.0),
                    (3.0, 0.0),
                    (4.0, 0.0),
                    (0.0, 0.55),
                    (0.0, 0.72),
                    (0.0, 0.88),
                    (0.0, 0.98),
                ] {
                    let across_dist = (mult * step[cross])
                        .max(extended_radius * radius_fraction)
                        .next_up();
                    let signs: &[f64] = if mult == 0.0 && radius_fraction == 0.0 {
                        &[0.0]
                    } else {
                        &[-1.0, 1.0]
                    };
                    for &sign in signs {
                        for along_mult in [0.0, -0.5, 0.5, -1.0, 1.0, -1.5, 1.5] {
                            work.visit(1)?;
                            let mut candidate = sample;
                            candidate[cross] += sign * across_dist;
                            candidate[1 - cross] += along_mult * along_base;
                            let mut valid = true;
                            for ((coord, [lo, hi]), half) in
                                candidate.iter_mut().zip(room).zip(step)
                            {
                                let min_c = lo + half;
                                let max_c = hi - half;
                                if min_c.is_nan() || max_c.is_nan() || min_c > max_c {
                                    valid = false;
                                    break;
                                }
                                *coord = coord.clamp(min_c, max_c);
                            }
                            if !valid {
                                continue;
                            }
                            let at = Point {
                                x: candidate[0],
                                y: candidate[1],
                            };
                            let dist = (at.x - sample[0]).hypot(at.y - sample[1]);
                            if !dist.is_finite()
                                || dist > extended_radius
                                || closest.is_some_and(|(best, _)| dist >= best)
                            {
                                continue;
                            }
                            if matches!(
                                self.fits_classified(
                                    at,
                                    size,
                                    scope,
                                    route_index,
                                    None,
                                    &[],
                                    tier,
                                    work,
                                )?,
                                FitResult::Clear
                            ) {
                                closest = Some((dist, at));
                            }
                        }
                    }
                }
            }
            if let Some((_, at)) = closest {
                return Ok(Some(at));
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
    let solid_cap = context
        .drawing
        .nodes
        .len()
        .saturating_add(context.drawing.groups.len().saturating_mul(5))
        .saturating_add(plan.sites.len());
    work.charge(solid_cap)?;
    let mut solids = reserved(solid_cap)?;
    for &node_rect in &context.drawing.nodes {
        work.visit(1)?;
        solids.push(SolidObstacle {
            bounds: Bounds::of(node_rect),
            is_frame: false,
        });
    }
    for (group, geometry) in context.drawing.groups.iter().enumerate() {
        work.visit(1)?;
        if let Some(title) = context.title(group) {
            push(
                &mut solids,
                SolidObstacle {
                    bounds: Bounds::of(title),
                    is_frame: false,
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
                &mut solids,
                SolidObstacle {
                    bounds,
                    is_frame: true,
                },
            )?;
        }
    }
    let route_cap = routes.len().saturating_mul(2);
    work.charge(route_cap)?;
    let mut route_obstacles = reserved(route_cap)?;
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
                push(&mut route_obstacles, RouteObstacle { bounds, edge })?;
            }
        }
        if let Some(bounds) = previous {
            push(&mut route_obstacles, RouteObstacle { bounds, edge })?;
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
                &mut solids,
                SolidObstacle {
                    bounds,
                    is_frame: false,
                },
            )?;
        }
    }
    let mut scene = Scene {
        solids: Bvh::build_with_work(solids, |obs| obs.bounds, work)?,
        routes: Bvh::build_with_work(route_obstacles, |obs| obs.bounds, work)?,
        labels: Vec::new(),
        clearance: context.metrics.label_clearance,
    };
    let mut bottom = context.drawing.bounds.height;
    if let Some(ink) = scene.solids.root_bounds() {
        bottom = bottom.max(ink.y[1]);
    }
    if let Some(ink) = scene.routes.root_bounds() {
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
                        if scene.fits(
                            at,
                            size,
                            constraint,
                            edge,
                            None,
                            ClearanceTier::OffAll,
                            work,
                        )? {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KnotKind, NoopWorkControl};

    #[test]
    fn parallel_route_captions_do_not_jump_across_foreign_wires() {
        let mut noop = NoopWorkControl;
        let mut work = Work::new(&mut noop);
        // Two closely spaced parallel horizontal routes: edge 0 at y=160.4, edge 1 at y=167.4.
        // Place a solid blocker above the center of edge 0 so probe 0's upper candidate is blocked,
        // and ensure edge 0 does not jump below edge 1 (and edge 1 does not jump above edge 0).
        let solids = vec![SolidObstacle {
            bounds: Bounds {
                x: [310.0, 390.0],
                y: [100.0, 155.0],
            },
            is_frame: false,
        }];
        let routes = vec![
            RouteObstacle {
                bounds: Bounds {
                    x: [200.0, 500.0],
                    y: [160.4, 160.4],
                },
                edge: 0,
            },
            RouteObstacle {
                bounds: Bounds {
                    x: [200.0, 500.0],
                    y: [167.4, 167.4],
                },
                edge: 1,
            },
        ];
        let mut scene = Scene {
            solids: Bvh::build_with_work(solids, |obs| obs.bounds, &mut work).unwrap(),
            routes: Bvh::build_with_work(routes, |obs| obs.bounds, &mut work).unwrap(),
            labels: Vec::new(),
            clearance: 8.0,
        };
        let route0 = [
            RoutePoint {
                x: 200.0,
                y: 160.4,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
            RoutePoint {
                x: 500.0,
                y: 160.4,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
        ];
        let route1 = [
            RoutePoint {
                x: 200.0,
                y: 167.4,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
            RoutePoint {
                x: 500.0,
                y: 167.4,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
        ];
        let size = Size {
            width: 80.0,
            height: 20.0,
        };
        let mut lengths = Vec::new();
        let seat0 = scene
            .near_route(
                &route0,
                0,
                0.5,
                [0.0, 0.0],
                size,
                None,
                &mut lengths,
                &mut work,
            )
            .unwrap()
            .expect("edge 0 should seat on its own side");
        assert!(
            seat0.y < 160.4,
            "edge 0 caption jumped across edge 1 (y=167.4) to y={}",
            seat0.y
        );
        scene
            .add_caption(Bounds::centered(seat0, size), &mut work)
            .unwrap();

        let seat1 = scene
            .near_route(
                &route1,
                1,
                0.5,
                [0.0, 0.0],
                size,
                None,
                &mut lengths,
                &mut work,
            )
            .unwrap()
            .expect("edge 1 should seat on its own side");
        assert!(
            seat1.y > 167.4,
            "edge 1 caption jumped across edge 0 (y=160.4) to y={}",
            seat1.y
        );
    }

    #[test]
    fn trapped_center_wire_in_parallel_bundle_seats_locally_instead_of_teleporting() {
        let mut noop = NoopWorkControl;
        let mut work = Work::new(&mut noop);
        // Three tightly packed parallel horizontal routes spaced 7px apart:
        // the middle wire (edge 1) has a foreign route on both sides within label height,
        // so strict tether & foreign-route checks cannot place it without a local rescue.
        let routes = vec![
            RouteObstacle {
                bounds: Bounds {
                    x: [100.0, 400.0],
                    y: [160.0, 160.0],
                },
                edge: 0,
            },
            RouteObstacle {
                bounds: Bounds {
                    x: [100.0, 400.0],
                    y: [167.0, 167.0],
                },
                edge: 1,
            },
            RouteObstacle {
                bounds: Bounds {
                    x: [100.0, 400.0],
                    y: [174.0, 174.0],
                },
                edge: 2,
            },
        ];
        let scene = Scene {
            solids: Bvh::build_with_work(Vec::new(), |obs: &SolidObstacle| obs.bounds, &mut work)
                .unwrap(),
            routes: Bvh::build_with_work(routes, |obs| obs.bounds, &mut work).unwrap(),
            labels: Vec::new(),
            clearance: 8.0,
        };
        let center_route = [
            RoutePoint {
                x: 100.0,
                y: 167.0,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
            RoutePoint {
                x: 400.0,
                y: 167.0,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
        ];
        let size = Size {
            width: 90.0,
            height: 24.0,
        };
        let mut lengths = Vec::new();
        let seat = scene
            .near_route(
                &center_route,
                1,
                0.5,
                [0.0, 0.0],
                size,
                None,
                &mut lengths,
                &mut work,
            )
            .unwrap()
            .expect("trapped center wire must seat locally rather than teleporting to bottom row");
        assert!(
            (seat.y - 167.0).abs() <= 45.0 && (100.0..=400.0).contains(&seat.x),
            "expected local seat near (250, 167), got {seat:?}"
        );
    }

    #[test]
    fn non_finite_caption_dimensions_do_not_panic_and_segment_crosses_bounds_rejects_nan() {
        let mut noop = NoopWorkControl;
        let mut work = Work::new(&mut noop);
        assert!(!segment_crosses_bounds(
            Point {
                x: f64::NAN,
                y: 0.0
            },
            Point { x: 10.0, y: 10.0 },
            Bounds {
                x: [0.0, 20.0],
                y: [0.0, 20.0],
            },
        ));
        assert!(!segment_crosses_bounds(
            Point { x: 0.0, y: 0.0 },
            Point { x: 10.0, y: 10.0 },
            Bounds {
                x: [f64::NAN, 20.0],
                y: [0.0, 20.0],
            },
        ));
        let scene = Scene {
            solids: Bvh::build_with_work(Vec::new(), |obs: &SolidObstacle| obs.bounds, &mut work)
                .unwrap(),
            routes: Bvh::build_with_work(Vec::new(), |obs: &RouteObstacle| obs.bounds, &mut work)
                .unwrap(),
            labels: Vec::new(),
            clearance: f64::MAX,
        };
        let route = [
            RoutePoint {
                x: 10.0,
                y: 20.0,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
            RoutePoint {
                x: 200.0,
                y: 20.0,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
        ];
        let mut lengths = Vec::new();
        let seat = scene
            .near_route(
                &route,
                0,
                0.5,
                [0.0, 0.0],
                Size {
                    width: 1e308,
                    height: 20.0,
                },
                None,
                &mut lengths,
                &mut work,
            )
            .unwrap();
        assert!(seat.is_none());
    }

    #[test]
    fn congested_caption_uses_extended_local_rescue_within_local_radius() {
        let mut noop = NoopWorkControl;
        let mut work = Work::new(&mut noop);
        // Block the immediate 120px neighborhood above and below the route with solid cards so
        // standard rings fail, while leaving an open pocket ~150px below the route.
        let solids = vec![
            SolidObstacle {
                bounds: Bounds {
                    x: [0.0, 600.0],
                    y: [0.0, 195.0],
                },
                is_frame: false,
            },
            SolidObstacle {
                bounds: Bounds {
                    x: [0.0, 600.0],
                    y: [205.0, 325.0],
                },
                is_frame: false,
            },
        ];
        let routes = vec![RouteObstacle {
            bounds: Bounds {
                x: [100.0, 400.0],
                y: [200.0, 200.0],
            },
            edge: 0,
        }];
        let scene = Scene {
            solids: Bvh::build_with_work(solids, |obs| obs.bounds, &mut work).unwrap(),
            routes: Bvh::build_with_work(routes, |obs| obs.bounds, &mut work).unwrap(),
            labels: Vec::new(),
            clearance: 8.0,
        };
        let route = [
            RoutePoint {
                x: 100.0,
                y: 200.0,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
            RoutePoint {
                x: 400.0,
                y: 200.0,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
        ];
        let mut lengths = Vec::new();
        let seat = scene
            .near_route(
                &route,
                0,
                0.5,
                [0.0, 0.0],
                Size {
                    width: 160.0,
                    height: 50.0,
                },
                None,
                &mut lengths,
                &mut work,
            )
            .unwrap()
            .expect("extended local rescue should find the open pocket at y ~ 360");
        assert!(
            seat.y >= 350.0 && seat.y <= 400.0,
            "expected extended local seat near y=360, got {seat:?}"
        );
    }
}

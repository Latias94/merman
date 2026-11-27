use super::super::route_plan::{
    PIXEL_EPSILON, SIDES, legal_side_interval, side_index, side_midpoint, tangential, title_bottom,
};
use super::{
    End, EndpointRef, GridEndpointCandidate, GridRoutePorts, MIN_PORT_SEPARATION, OwnerEndpoint,
    PortPlanner, allocation_error, clamp, compare_numbers, reserve, separate_coordinates,
    sort_charge,
};
use crate::Result;
use crate::graph_layout::{Axis, Point, Rect, Side};
use crate::resources::OperationWorkMeter;

const EDGE_CLEARANCE: f64 = 24.0;
const TERMINAL_APPROACH: f64 = 20.0;

#[derive(Clone, Copy)]
struct Slot {
    side: Side,
    coordinate: f64,
}

struct Demand {
    endpoint: EndpointRef,
    opposite: Point,
    aligned: bool,
    slot: Option<Slot>,
    alternatives: [Option<f64>; 4],
}

struct Owner {
    slot: usize,
    bounds: Rect,
    titled: bool,
    intervals: [Option<(f64, f64)>; 4],
    max_key_bytes: usize,
}

impl Owner {
    fn preferred_side(&self, opposite: Point) -> Side {
        let center = self.bounds.center();
        let dx = opposite.x - center.x;
        let dy = opposite.y - center.y;
        if dx.abs() >= dy.abs() || (dy < 0.0 && self.titled) {
            if dx >= 0.0 { Side::Right } else { Side::Left }
        } else if dy >= 0.0 {
            Side::Bottom
        } else {
            Side::Top
        }
    }

    fn side_preferences(&self, opposite: Point) -> [Side; 4] {
        let preferred = self.preferred_side(opposite);
        let mut sides = SIDES;
        sides.sort_unstable_by(|&a, &b| {
            (a != preferred)
                .cmp(&(b != preferred))
                .then_with(|| {
                    compare_numbers(
                        distance(side_midpoint(self.bounds, a), opposite),
                        distance(side_midpoint(self.bounds, b), opposite),
                    )
                })
                .then_with(|| side_index(a).cmp(&side_index(b)))
        });
        sides
    }
}

#[derive(Default)]
struct SideSlots {
    capacity: usize,
    count: usize,
    reserved: bool,
    fixed_demands: Vec<usize>,
    occupied: Vec<f64>,
}

impl SideSlots {
    fn record_fixed(&mut self, demand: usize) -> Result<()> {
        self.fixed_demands
            .try_reserve(1)
            .map_err(|_| allocation_error())?;
        self.fixed_demands.push(demand);
        self.count += 1;
        Ok(())
    }
}

impl PortPlanner<'_> {
    pub(super) fn allocate_regular(
        &self,
        endpoints: &mut [OwnerEndpoint],
        routes: &mut [GridRoutePorts],
    ) -> Result<()> {
        sort_charge(endpoints.len(), 1, self.work)?;
        endpoints
            .sort_unstable_by_key(|entry| (entry.owner, entry.endpoint.route, entry.endpoint.end));
        self.work.charge(endpoints.len())?;
        let mut demands = Vec::new();
        let mut indices = Vec::new();
        let mut coordinates = Vec::new();
        for endpoints in endpoints.chunk_by(|a, b| a.owner == b.owner) {
            self.work.charge(endpoints.len())?;
            demands.clear();
            reserve(&mut demands, endpoints.len())?;
            let slot = endpoints[0].owner;
            let bounds = self.bounds(slot)?;
            let center = bounds.center();
            let mut owner = Owner {
                slot,
                bounds,
                titled: title_bottom(self.geometry, slot).is_some(),
                intervals: SIDES.map(|side| legal_side_interval(self.geometry, slot, side)),
                max_key_bytes: 0,
            };
            let mut aligned_count = 0;
            for entry in endpoints {
                self.work.charge(1)?;
                let endpoint = entry.endpoint;
                owner.max_key_bytes = owner.max_key_bytes.max(self.keys[endpoint.route].bytes);
                let opposite = self
                    .bounds(endpoint.end.opposite(&self.plans[endpoint.route]))?
                    .center();
                let aligned = (opposite.x - center.x).abs() <= PIXEL_EPSILON
                    || (opposite.y - center.y).abs() <= PIXEL_EPSILON;
                aligned_count += usize::from(aligned);
                demands.push(Demand {
                    endpoint,
                    opposite,
                    aligned,
                    slot: None,
                    alternatives: [None; 4],
                });
            }
            let coordinated_owner = aligned_count != 0 && aligned_count != demands.len();
            sort_charge(demands.len(), owner.max_key_bytes, self.work)?;
            demands.sort_unstable_by(|a, b| self.compare_demands(a, b));
            self.allocate_owner_slots(&owner, &mut demands, &mut coordinates)?;

            // Each side's full-owner sort has the same inputs for every endpoint at this owner.
            self.work.charge(demands.len())?;
            indices.clear();
            reserve(&mut indices, demands.len())?;
            indices.extend(0..demands.len());
            for side in SIDES {
                let i = side_index(side);
                if owner.intervals[i].is_some() {
                    self.preferred_coordinates(
                        &owner,
                        side,
                        &demands,
                        &mut indices,
                        &mut coordinates,
                    )?;
                    for (&demand, &coordinate) in indices.iter().zip(&coordinates) {
                        self.work.charge(1)?;
                        demands[demand].alternatives[i] = Some(coordinate);
                    }
                }
            }

            for demand in &demands {
                self.work.charge(1)?;
                let plan = &self.plans[demand.endpoint.route];
                let route = &mut routes[demand.endpoint.route];
                route.coordinated |= coordinated_owner && !demand.aligned;
                let candidates = endpoint_candidates(
                    &owner,
                    demand,
                    plan.lane.count,
                    plan.lane.offset,
                    coordinated_owner,
                    self.work,
                )?;
                match demand.endpoint.end {
                    End::Source => route.sources = candidates,
                    End::Target => route.targets = candidates,
                }
            }
        }
        Ok(())
    }

    fn compare_demands(&self, a: &Demand, b: &Demand) -> std::cmp::Ordering {
        let a = a.endpoint;
        let b = b.endpoint;
        self.compare_routes(a.route, b.route)
            .then_with(|| a.end.cmp(&b.end))
            .then_with(|| a.route.cmp(&b.route))
    }

    fn preferred_coordinates(
        &self,
        owner: &Owner,
        side: Side,
        demands: &[Demand],
        indices: &mut [usize],
        coordinates: &mut Vec<f64>,
    ) -> Result<()> {
        coordinates.clear();
        let Some((low, high)) = owner.intervals[side_index(side)] else {
            return Ok(());
        };
        if indices.is_empty() {
            return Ok(());
        }
        sort_charge(indices.len(), owner.max_key_bytes, self.work)?;
        indices.sort_unstable_by(|&a, &b| {
            let a = &demands[a];
            let b = &demands[b];
            compare_numbers(tangential(a.opposite, side), tangential(b.opposite, side))
                .then_with(|| self.compare_demands(a, b))
        });
        self.work.charge(indices.len())?;
        reserve(coordinates, indices.len())?;
        if indices.len() == 1 {
            coordinates.push(clamp(tangential(owner.bounds.center(), side), low, high));
        } else if (high - low) / ((indices.len() - 1) as f64) < MIN_PORT_SEPARATION {
            for index in 0..indices.len() {
                self.work.charge(1)?;
                coordinates.push(low + index as f64 * (high - low) / (indices.len() - 1) as f64);
            }
        } else {
            for &index in indices.iter() {
                self.work.charge(1)?;
                coordinates.push(clamp(tangential(demands[index].opposite, side), low, high));
            }
            separate_coordinates(coordinates, low, high, self.work)?;
        }
        Ok(())
    }

    fn allocate_owner_slots(
        &self,
        owner: &Owner,
        demands: &mut [Demand],
        coordinates: &mut Vec<f64>,
    ) -> Result<()> {
        let mut slots = owner.intervals.map(|interval| SideSlots {
            capacity: interval.map_or(0, |(low, high)| {
                (((high - low) / MIN_PORT_SEPARATION).floor() as usize).saturating_add(1)
            }),
            ..SideSlots::default()
        });
        for (index, demand) in demands.iter().enumerate() {
            self.work.charge(1)?;
            if !demand.aligned {
                continue;
            }
            self.work.charge(SIDES.len())?;
            for side in owner.side_preferences(demand.opposite) {
                let slot = &mut slots[side_index(side)];
                if slot.count < slot.capacity {
                    slot.record_fixed(index)?;
                    slot.reserved = true;
                    break;
                }
            }
        }
        for side in SIDES {
            let slot = &mut slots[side_index(side)];
            if slot.reserved {
                self.preferred_coordinates(
                    owner,
                    side,
                    demands,
                    &mut slot.fixed_demands,
                    &mut slot.occupied,
                )?;
            }
        }

        for (index, demand) in demands.iter_mut().enumerate() {
            self.work.charge(1)?;
            if demand.aligned {
                continue;
            }
            self.work.charge(SIDES.len())?;
            let mut fallback = None;
            for side in owner.side_preferences(demand.opposite) {
                let i = side_index(side);
                let slot = &mut slots[i];
                if slot.count >= slot.capacity {
                    continue;
                }
                fallback.get_or_insert(i);
                if !slot.reserved {
                    fallback = Some(i);
                    break;
                }
                let (low, high) = owner.intervals[i].expect("only legal sides have capacity");
                let desired = clamp(tangential(demand.opposite, side), low, high);
                if let Some(coordinate) =
                    nearest_free_coordinate(low, high, desired, &slot.occupied, self.work)?
                {
                    slot.occupied
                        .try_reserve(1)
                        .map_err(|_| allocation_error())?;
                    slot.occupied.push(coordinate);
                    slot.count += 1;
                    demand.slot = Some(Slot { side, coordinate });
                    fallback = None;
                    break;
                }
            }
            if let Some(i) = fallback {
                slots[i].record_fixed(index)?;
            }
        }
        for side in SIDES {
            let slot = &mut slots[side_index(side)];
            self.preferred_coordinates(owner, side, demands, &mut slot.fixed_demands, coordinates)?;
            for (&index, &coordinate) in slot.fixed_demands.iter().zip(coordinates.iter()) {
                self.work.charge(1)?;
                demands[index].slot = Some(Slot { side, coordinate });
            }
        }
        Ok(())
    }
}

fn nearest_free_coordinate(
    low: f64,
    high: f64,
    desired: f64,
    occupied: &[f64],
    work: &OperationWorkMeter,
) -> Result<Option<f64>> {
    let mut best = None;
    for candidate in [desired, low, high].into_iter().chain(
        occupied
            .iter()
            .flat_map(|&placed| [placed - EDGE_CLEARANCE, placed + EDGE_CLEARANCE]),
    ) {
        work.charge(1)?;
        if candidate < low - PIXEL_EPSILON || candidate > high + PIXEL_EPSILON {
            continue;
        }
        if best.is_some_and(|prior: f64| {
            !compare_numbers((candidate - desired).abs(), (prior - desired).abs())
                .then_with(|| compare_numbers(candidate, prior))
                .is_lt()
        }) {
            continue;
        }
        let mut clear = true;
        for &placed in occupied {
            work.charge(1)?;
            if (candidate - placed).abs() < EDGE_CLEARANCE - PIXEL_EPSILON {
                clear = false;
                break;
            }
        }
        if clear {
            if candidate == desired {
                return Ok(Some(candidate));
            }
            best = Some(candidate);
        }
    }
    Ok(best)
}

fn port_point(bounds: Rect, side: Side, coordinate: f64) -> Point {
    let mut point = side_midpoint(bounds, side);
    match side.axis() {
        Axis::Horizontal => point.y = coordinate,
        Axis::Vertical => point.x = coordinate,
    }
    point
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

fn endpoint_candidates(
    owner: &Owner,
    demand: &Demand,
    bundle_size: usize,
    lane_offset: f64,
    coordinated_owner: bool,
    work: &OperationWorkMeter,
) -> Result<Vec<GridEndpointCandidate>> {
    // Each end has at most the allocated slot or bundle lane plus one geometric port per side.
    let mut ranked: [Option<(GridEndpointCandidate, f64)>; 5] = [None; 5];
    let mut len = 0;
    work.charge(ranked.len())?;
    let mut add = |side: Side, coordinate: f64, score: f64| {
        if !owner.intervals[side_index(side)]
            .is_some_and(|(low, high)| coordinate >= low && coordinate <= high)
        {
            return;
        }
        let port = port_point(owner.bounds, side, coordinate);
        if ranked[..len]
            .iter()
            .flatten()
            .any(|(candidate, _)| candidate.side == side && candidate.port == port)
        {
            return;
        }
        let (dx, dy) = match side {
            Side::Right => (TERMINAL_APPROACH, 0.0),
            Side::Bottom => (0.0, TERMINAL_APPROACH),
            Side::Left => (-TERMINAL_APPROACH, 0.0),
            Side::Top => (0.0, -TERMINAL_APPROACH),
        };
        let connect = Point {
            x: port.x + dx,
            y: port.y + dy,
        };
        ranked[len] = Some((
            GridEndpointCandidate {
                owner: owner.slot,
                side,
                port,
                connect,
                rank: 0,
                on_assigned_side: demand
                    .slot
                    .filter(|_| coordinated_owner)
                    .map(|slot| side == slot.side),
            },
            score,
        ));
        len += 1;
    };
    if let Some(slot) = demand.slot.filter(|_| bundle_size == 1) {
        add(slot.side, slot.coordinate, -2.0);
    }
    if bundle_size > 1 {
        let side = owner.preferred_side(demand.opposite);
        add(
            side,
            tangential(owner.bounds.center(), side) + lane_offset,
            -1.0,
        );
    }
    for side in SIDES {
        if let Some(coordinate) = demand.alternatives[side_index(side)] {
            add(
                side,
                coordinate,
                distance(port_point(owner.bounds, side, coordinate), demand.opposite),
            );
        }
    }
    sort_charge(len, 1, work)?;
    ranked[..len].sort_unstable_by(|a, b| {
        let (a, a_score) = a.expect("occupied candidate");
        let (b, b_score) = b.expect("occupied candidate");
        compare_numbers(a_score, b_score).then_with(|| side_index(a.side).cmp(&side_index(b.side)))
    });
    let mut result = Vec::new();
    work.charge(len)?;
    reserve(&mut result, len)?;
    for (rank, (mut candidate, _)) in ranked.into_iter().flatten().enumerate() {
        candidate.rank = rank;
        result.push(candidate);
    }
    Ok(result)
}

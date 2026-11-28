use super::super::route_plan::{
    CLEARANCE, GridEndpoint, PIXEL_EPSILON, legal_side_interval, side_index, side_midpoint,
    tangential, title_bottom,
};
use super::{
    End, EndpointRef, GridItemKind, GridPortPlan, MIN_PORT_SEPARATION, PortPlanner,
    allocation_error, clamp, code_units, compare_numbers, filled, missing_geometry, parent,
    reserve, separate_coordinates, sort_charge,
};
use crate::Result;
use crate::graph_layout::{Axis, Rect, Side};
use std::cmp::Reverse;
use std::collections::HashMap;

#[derive(Clone, Copy)]
struct Demand {
    endpoint: EndpointRef,
    chain_index: usize,
    ordinal: usize,
    inner_coordinate: f64,
}

struct OwnerSide {
    owner: usize,
    side: Side,
    all_compact: bool,
    depth: usize,
    ordinal: usize,
    demands: Vec<Demand>,
    max_edge_id_bytes: usize,
}

impl PortPlanner<'_> {
    pub(super) fn allocate_hierarchy(&self, result: &mut GridPortPlan) -> Result<()> {
        let mut owner_sides = self.collect_owner_sides(&result.ordered_routes)?;
        sort_charge(owner_sides.len(), 1, self.work)?;
        owner_sides
            .sort_unstable_by_key(|group| (group.all_compact, Reverse(group.depth), group.ordinal));

        let mut desired = Vec::new();
        for group in &mut owner_sides {
            self.work.charge(1)?;
            self.allocate_owner_side(group, result, &mut desired)?;
        }
        self.align_hierarchy(result)
    }

    fn collect_owner_sides(&self, ordered_routes: &[usize]) -> Result<Vec<OwnerSide>> {
        let mut groups: Vec<OwnerSide> = Vec::new();
        let mut group_for_side = HashMap::new();
        let mut depths = Vec::new();
        if !ordered_routes.is_empty() && !self.forest.postorder.is_empty() {
            self.work.charge(self.forest.items.len())?;
            depths = filled(self.forest.items.len(), 0)?;
            // A reverse postorder visits each group after its parent, including on deep chains.
            for &slot in self.forest.postorder.iter().rev() {
                self.work.charge(1)?;
                depths[slot] = parent(self.forest, slot)
                    .and_then(|parent| depths.get(parent))
                    .copied()
                    .unwrap_or(0)
                    + 1;
            }
        }
        for &route in ordered_routes {
            self.work.charge(1)?;
            let plan = &self.plans[route];
            if plan.edge.source == plan.edge.target {
                continue;
            }
            for end in End::BOTH {
                for (chain_index, entry) in end.plan(plan).chain.iter().enumerate() {
                    self.work.charge(1)?;
                    let key = (entry.owner, side_index(entry.side));
                    let index = if let Some(&index) = group_for_side.get(&key) {
                        index
                    } else {
                        group_for_side
                            .try_reserve(1)
                            .map_err(|_| allocation_error())?;
                        groups.try_reserve(1).map_err(|_| allocation_error())?;
                        let index = groups.len();
                        group_for_side.insert(key, index);
                        let depth = parent(self.forest, entry.owner)
                            .and_then(|parent| depths.get(parent))
                            .copied()
                            .unwrap_or(0)
                            + 1;
                        groups.push(OwnerSide {
                            owner: entry.owner,
                            side: entry.side,
                            all_compact: true,
                            depth,
                            ordinal: index,
                            demands: Vec::new(),
                            max_edge_id_bytes: 0,
                        });
                        index
                    };
                    let group = &mut groups[index];
                    group.all_compact &= entry.compact_portal;
                    group.max_edge_id_bytes = group.max_edge_id_bytes.max(plan.edge.id.len());
                    group
                        .demands
                        .try_reserve(1)
                        .map_err(|_| allocation_error())?;
                    group.demands.push(Demand {
                        endpoint: EndpointRef { route, end },
                        chain_index,
                        ordinal: group.demands.len(),
                        inner_coordinate: 0.0,
                    });
                }
            }
        }
        Ok(groups)
    }

    fn entry(&self, demand: Demand) -> &GridEndpoint {
        &demand
            .endpoint
            .end
            .plan(&self.plans[demand.endpoint.route])
            .chain[demand.chain_index]
    }

    fn compare_tangential_demands(&self, a: Demand, b: Demand) -> std::cmp::Ordering {
        compare_numbers(
            self.entry(a).opposite_coordinate,
            self.entry(b).opposite_coordinate,
        )
        .then_with(|| {
            code_units(
                self.plans[a.endpoint.route].edge.id,
                self.plans[b.endpoint.route].edge.id,
            )
        })
        .then_with(|| a.ordinal.cmp(&b.ordinal))
    }

    fn allocate_owner_side(
        &self,
        group: &mut OwnerSide,
        result: &mut GridPortPlan,
        desired: &mut Vec<f64>,
    ) -> Result<()> {
        let bounds = self.bounds(group.owner)?;
        let (low, high) =
            demand_interval(bounds, group.side, title_bottom(self.geometry, group.owner));
        let span = (high - low).max(0.0);
        let compact_portals = group.all_compact
            && matches!(
                self.forest
                    .items
                    .get(group.owner)
                    .ok_or_else(missing_geometry)?
                    .kind,
                GridItemKind::Group(_)
            );
        let count = group.demands.len();
        sort_charge(count, group.max_edge_id_bytes, self.work)?;
        group
            .demands
            .sort_unstable_by(|&a, &b| self.compare_tangential_demands(a, b));

        let center = (low + high) / 2.0;
        if !(compact_portals && count == 1)
            && self.is_isolated_bundle(group, result, center, low, high)?
        {
            for &demand in &group.demands {
                self.work.charge(1)?;
                let coordinate = center + self.plans[demand.endpoint.route].lane.offset;
                assign(result, demand, coordinate);
            }
            return Ok(());
        }

        if compact_portals && span >= MIN_PORT_SEPARATION * (count - 1) as f64 {
            let container = self
                .geometry
                .containers
                .get(group.owner)
                .and_then(Option::as_ref)
                .ok_or_else(missing_geometry)?;
            let corridors = match group.side.axis() {
                Axis::Horizontal => &container.horizontal_corridors,
                Axis::Vertical => &container.vertical_corridors,
            };
            if count == 1 {
                let demand = group.demands[0];
                let coordinate = self.preferred_portal(demand, result, corridors, low, high)?;
                assign(result, demand, coordinate);
                return Ok(());
            }
            for demand in &mut group.demands {
                self.work.charge(1)?;
                demand.inner_coordinate = self.inner_coordinate(*demand, result)?;
            }
            sort_charge(count, group.max_edge_id_bytes, self.work)?;
            group.demands.sort_unstable_by(|&a, &b| {
                compare_numbers(a.inner_coordinate, b.inner_coordinate)
                    .then_with(|| self.compare_tangential_demands(a, b))
            });
            desired.clear();
            self.work.charge(count)?;
            reserve(desired, count)?;
            for &demand in &group.demands {
                desired.push(self.preferred_portal(demand, result, corridors, low, high)?);
            }
            separate_coordinates(desired, low, high, self.work)?;
            for (&demand, &coordinate) in group.demands.iter().zip(desired.iter()) {
                self.work.charge(1)?;
                assign(result, demand, coordinate);
            }
            return Ok(());
        }

        if count == 1 || span <= 0.0 {
            // Upstream assigns only the first demand on a collapsed side.
            assign(result, group.demands[0], center);
            return Ok(());
        }
        let step = span / (count - 1) as f64;
        for (index, &demand) in group.demands.iter().enumerate() {
            self.work.charge(1)?;
            assign(result, demand, low + step * index as f64);
        }
        Ok(())
    }

    fn is_isolated_bundle(
        &self,
        group: &OwnerSide,
        result: &GridPortPlan,
        center: f64,
        low: f64,
        high: f64,
    ) -> Result<bool> {
        let key = &self.keys[group.demands[0].endpoint.route];
        for demand in &group.demands {
            let index = demand.endpoint.route;
            let plan = &self.plans[index];
            self.work.charge(1)?;
            let coordinate = center + plan.lane.offset;
            if plan.lane.count != group.demands.len()
                || result.endpoint_incident_counts[plan.edge.source] != plan.lane.count
                || result.endpoint_incident_counts[plan.edge.target] != plan.lane.count
                || !(coordinate >= low && coordinate <= high)
            {
                return Ok(false);
            }
            self.work.charge(key.bytes.max(self.keys[index].bytes))?;
            if !self.keys[index].pair.cmp(key.pair).is_eq() {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn inner_slot(&self, demand: Demand, result: &GridPortPlan) -> Option<(f64, &GridEndpoint)> {
        let index = demand.chain_index.checked_sub(1)?;
        let EndpointRef { route, end } = demand.endpoint;
        let coordinate = result.routes[route].coordinates(end)[index]?;
        Some((coordinate, &end.plan(&self.plans[route]).chain[index]))
    }

    fn inner_coordinate(&self, demand: Demand, result: &GridPortPlan) -> Result<f64> {
        let outer = self.entry(demand);
        match self.inner_slot(demand, result) {
            Some((coordinate, inner)) if outer.side.axis() == inner.side.axis() => Ok(coordinate),
            Some((_, inner)) => Ok(tangential(
                side_midpoint(self.bounds(inner.owner)?, inner.side),
                outer.side,
            )),
            None => Ok(outer.preferred_coordinate),
        }
    }

    fn preferred_portal(
        &self,
        demand: Demand,
        result: &GridPortPlan,
        corridors: &[f64],
        low: f64,
        high: f64,
    ) -> Result<f64> {
        self.work.charge(1)?;
        let outer = self.entry(demand);
        let preferred = self
            .inner_slot(demand, result)
            .filter(|(_, inner)| inner.side.axis() == outer.side.axis())
            .map_or(outer.preferred_coordinate, |(coordinate, _)| coordinate);
        let clamped = clamp(preferred, low, high);
        let mut nearest = clamped;
        let mut distance = f64::INFINITY;
        for &coordinate in corridors {
            self.work.charge(1)?;
            let delta = (coordinate - clamped).abs();
            if delta < distance {
                nearest = coordinate;
                distance = delta;
            }
        }
        Ok(if distance <= PIXEL_EPSILON {
            nearest
        } else {
            clamped
        })
    }

    fn align_hierarchy(&self, result: &mut GridPortPlan) -> Result<()> {
        for &index in &result.ordered_routes {
            self.work.charge(1)?;
            let plan = &self.plans[index];
            if plan.lane.count != 1 {
                continue;
            }
            let route = &mut result.routes[index];
            let (Some(Some(source)), Some(Some(target))) = (
                route.source_coordinates.last().copied(),
                route.target_coordinates.last().copied(),
            ) else {
                continue;
            };
            if (source - target).abs() > PIXEL_EPSILON {
                continue;
            }
            let axis = plan
                .source
                .chain
                .last()
                .expect("validated endpoint chain")
                .side
                .axis();
            let entries = || plan.source.chain.iter().chain(&plan.target.chain);
            let mut fits = [true, true];
            for entry in entries() {
                self.work.charge(1)?;
                if entry.side.axis() != axis {
                    fits = [false, false];
                    break;
                }
                let interval = legal_side_interval(self.geometry, entry.owner, entry.side);
                for (fit, coordinate) in fits.iter_mut().zip([source, target]) {
                    *fit &= interval.is_some_and(|(low, high)| {
                        coordinate >= low - PIXEL_EPSILON && coordinate <= high + PIXEL_EPSILON
                    });
                }
                if fits == [false, false] {
                    break;
                }
            }
            let Some(coordinate) = [source, target]
                .into_iter()
                .zip(fits)
                .find_map(|(coordinate, fit)| fit.then_some(coordinate))
            else {
                continue;
            };
            for end in End::BOTH {
                for slot in route.coordinates_mut(end) {
                    self.work.charge(1)?;
                    *slot = Some(coordinate);
                }
            }
        }
        Ok(())
    }
}

fn demand_interval(bounds: Rect, side: Side, title: Option<f64>) -> (f64, f64) {
    // Demand allocation uses the raw interval even on a blocked top or a collapsed side. Later
    // attachment validation and candidate generation use the legal interval.
    match side.axis() {
        Axis::Horizontal => (
            (bounds.y + CLEARANCE)
                .max(title.map_or(f64::NEG_INFINITY, |bottom| bottom + CLEARANCE)),
            bounds.y + bounds.height - CLEARANCE,
        ),
        Axis::Vertical => (bounds.x + CLEARANCE, bounds.x + bounds.width - CLEARANCE),
    }
}

fn assign(result: &mut GridPortPlan, demand: Demand, coordinate: f64) {
    let EndpointRef { route, end } = demand.endpoint;
    result.routes[route].coordinates_mut(end)[demand.chain_index] = Some(coordinate);
}

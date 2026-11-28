//! Measured endpoint allocation from Mermaid 12.4.0 `routerPlanning.ts`.

mod hierarchy;
mod regular;

use super::forest::{GridForest, GridForestGeometry, GridItemKind};
use super::geometry::reserve;
use super::route_plan::{
    GridEndpointPlan, GridRoutePlan, allocation_error, code_units, parent, sort_charge,
};
use crate::graph_layout::{Point, Rect, Side};
use crate::resources::OperationWorkMeter;
use crate::{Error, Result};
use std::cmp::Ordering;

const MIN_PORT_SEPARATION: f64 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct GridEndpointCandidate {
    pub owner: usize,
    pub side: Side,
    pub port: Point,
    pub connect: Point,
    pub rank: usize,
    pub on_assigned_side: Option<bool>,
}

#[derive(Debug, Default)]
pub(super) struct GridRoutePorts {
    pub sources: Vec<GridEndpointCandidate>,
    pub targets: Vec<GridEndpointCandidate>,
    pub source_coordinates: Vec<Option<f64>>,
    pub target_coordinates: Vec<Option<f64>>,
    pub eligible: bool,
    pub coordinated: bool,
    pub fast_eligible: bool,
}

impl GridRoutePorts {
    fn coordinates(&self, end: End) -> &[Option<f64>] {
        match end {
            End::Source => &self.source_coordinates,
            End::Target => &self.target_coordinates,
        }
    }

    fn coordinates_mut(&mut self, end: End) -> &mut [Option<f64>] {
        match end {
            End::Source => &mut self.source_coordinates,
            End::Target => &mut self.target_coordinates,
        }
    }
}

#[derive(Debug)]
pub(super) struct GridPortPlan {
    /// One entry per input plan, with coordinates in each original endpoint chain's order.
    pub routes: Vec<GridRoutePorts>,
    /// Input plan indices in canonical route order.
    pub ordered_routes: Vec<usize>,
    /// Indexed by measured item slot. Each self-loop contributes one incident edge.
    pub endpoint_incident_counts: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum End {
    Source,
    Target,
}

impl End {
    const BOTH: [Self; 2] = [Self::Source, Self::Target];

    fn plan<'a>(self, plan: &'a GridRoutePlan<'_>) -> &'a GridEndpointPlan {
        match self {
            Self::Source => &plan.source,
            Self::Target => &plan.target,
        }
    }

    fn opposite(self, plan: &GridRoutePlan<'_>) -> usize {
        match self {
            Self::Source => plan.edge.target,
            Self::Target => plan.edge.source,
        }
    }
}

#[derive(Clone, Copy)]
struct EndpointRef {
    route: usize,
    end: End,
}

struct OwnerEndpoint {
    owner: usize,
    endpoint: EndpointRef,
}

// The separators participate in the comparison. A tuple of IDs sorts differently when one ID is
// a prefix of another. Streaming the original key also preserves upstream's `|` pair identity.
#[derive(Clone, Copy)]
struct JoinedIds<'a>(&'a str, &'a str);

impl JoinedIds<'_> {
    fn cmp(self, other: Self) -> Ordering {
        let units = |id: Self| {
            id.0.encode_utf16()
                .chain(std::iter::once(u16::from(b'|')))
                .chain(id.1.encode_utf16())
        };
        units(self).cmp(units(other))
    }
}

struct RouteKey<'a> {
    directed: JoinedIds<'a>,
    pair: JoinedIds<'a>,
    class: u8,
    boundaries: usize,
    bytes: usize,
}

struct PortPlanner<'a> {
    forest: &'a GridForest<'a>,
    geometry: &'a GridForestGeometry,
    plans: &'a [GridRoutePlan<'a>],
    keys: Vec<RouteKey<'a>>,
    work: &'a OperationWorkMeter,
}

/// Prepares ports for the plans returned by `plan_grid_edges` in input edge order.
/// Route slots keep separate occurrences bound to their own endpoint geometry.
pub(super) fn prepare_grid_ports(
    forest: &GridForest<'_>,
    geometry: &GridForestGeometry,
    plans: &[GridRoutePlan<'_>],
    work: &OperationWorkMeter,
) -> Result<GridPortPlan> {
    work.charge(forest.items.len())?;
    let mut result = GridPortPlan {
        routes: Vec::new(),
        ordered_routes: Vec::new(),
        endpoint_incident_counts: filled(forest.items.len(), 0)?,
    };
    work.charge(plans.len())?;
    reserve(&mut result.routes, plans.len())?;
    reserve(&mut result.ordered_routes, plans.len())?;
    let mut keys = Vec::new();
    reserve(&mut keys, plans.len())?;
    let mut max_key_bytes = 0;
    let mut owner_endpoints = Vec::new();

    for (index, plan) in plans.iter().enumerate() {
        work.charge(1)?;
        let edge = plan.edge;
        let id = |slot: usize| {
            forest
                .items
                .get(slot)
                .filter(|item| !matches!(item.kind, GridItemKind::EdgeLabel))
                .map(|item| item.item.id)
                .filter(|id| !id.is_empty())
        };
        let (Some(source), Some(target)) = (id(edge.source), id(edge.target)) else {
            return Err(Error::InvalidModel {
                message: format!(
                    "GRID_MISSING_ENDPOINT: Missing endpoint for edge \"{}\"",
                    edge.id
                ),
            });
        };
        let source_len = plan.source.chain.len();
        let target_len = plan.target.chain.len();
        if source_len == 0 || target_len == 0 {
            return Err(missing_geometry());
        }
        let chain_len = source_len
            .checked_add(target_len)
            .ok_or_else(|| work.arithmetic_overflow())?;
        let key_bytes = source
            .len()
            .checked_add(target.len())
            .and_then(|bytes| bytes.checked_add(edge.id.len()))
            .and_then(|bytes| bytes.checked_add(1))
            .ok_or_else(|| work.arithmetic_overflow())?;
        work.charge(key_bytes)?;
        let directed = JoinedIds(source, target);
        let pair = if code_units(source, target).is_gt() {
            JoinedIds(target, source)
        } else {
            directed
        };
        max_key_bytes = max_key_bytes.max(key_bytes);
        keys.push(RouteKey {
            directed,
            pair,
            class: route_class(forest, plan),
            boundaries: chain_len - 2,
            bytes: key_bytes,
        });
        work.charge(chain_len)?;
        let eligible = edge.source != edge.target
            && source_len == 1
            && target_len == 1
            && !plan.source.boundary
            && !plan.target.boundary;
        result.routes.push(GridRoutePorts {
            source_coordinates: filled(source_len, None)?,
            target_coordinates: filled(target_len, None)?,
            eligible,
            ..GridRoutePorts::default()
        });
        result.ordered_routes.push(index);
        result.endpoint_incident_counts[edge.source] += 1;
        if edge.source != edge.target {
            result.endpoint_incident_counts[edge.target] += 1;
        }
        if eligible {
            work.charge(2)?;
            owner_endpoints
                .try_reserve(2)
                .map_err(|_| allocation_error())?;
            owner_endpoints.extend([
                OwnerEndpoint {
                    owner: edge.source,
                    endpoint: EndpointRef {
                        route: index,
                        end: End::Source,
                    },
                },
                OwnerEndpoint {
                    owner: edge.target,
                    endpoint: EndpointRef {
                        route: index,
                        end: End::Target,
                    },
                },
            ]);
        }
    }

    let planner = PortPlanner {
        forest,
        geometry,
        plans,
        keys,
        work,
    };
    planner.allocate_regular(&mut owner_endpoints, &mut result.routes)?;
    drop(owner_endpoints);

    for (index, route) in result.routes.iter_mut().enumerate() {
        work.charge(1)?;
        route.fast_eligible = route.eligible
            && !route.coordinated
            && plans[index].lane.count == 1
            && !route.sources.is_empty()
            && !route.targets.is_empty();
    }
    sort_charge(result.ordered_routes.len(), max_key_bytes, work)?;
    result.ordered_routes.sort_unstable_by(|&a, &b| {
        let a_key = &planner.keys[a];
        let b_key = &planner.keys[b];
        let products = |index: usize| {
            let route = &result.routes[index];
            route.sources.len() * route.targets.len()
        };
        a_key
            .class
            .cmp(&b_key.class)
            .then_with(|| b_key.boundaries.cmp(&a_key.boundaries))
            .then_with(|| products(a).cmp(&products(b)))
            .then_with(|| planner.compare_routes(a, b))
            .then_with(|| a.cmp(&b))
    });
    planner.allocate_hierarchy(&mut result)?;
    work.charge(0)?;
    Ok(result)
}

fn route_class(forest: &GridForest<'_>, plan: &GridRoutePlan<'_>) -> u8 {
    let edge = plan.edge;
    if edge.source == edge.target {
        0
    } else if has_ancestor_endpoint(plan) {
        1
    } else if parent(forest, edge.source) != parent(forest, edge.target) {
        2
    } else if plan.lane.count > 1 {
        3
    } else {
        4
    }
}

pub(super) fn has_ancestor_endpoint(plan: &GridRoutePlan<'_>) -> bool {
    // The chains reach the ancestor's parent when they resolve the LCA.
    plan.source.boundary
        || plan.target.boundary
        || plan
            .source
            .chain
            .last()
            .is_some_and(|entry| entry.owner == plan.edge.target)
        || plan
            .target
            .chain
            .last()
            .is_some_and(|entry| entry.owner == plan.edge.source)
}

impl PortPlanner<'_> {
    fn bounds(&self, slot: usize) -> Result<Rect> {
        self.geometry
            .item(slot)
            .map(|item| item.bounds)
            .ok_or_else(missing_geometry)
    }

    fn compare_routes(&self, a: usize, b: usize) -> Ordering {
        let a_key = &self.keys[a];
        let b_key = &self.keys[b];
        a_key
            .pair
            .cmp(b_key.pair)
            .then_with(|| a_key.directed.cmp(b_key.directed))
            .then_with(|| code_units(self.plans[a].edge.id, self.plans[b].edge.id))
    }
}

fn missing_geometry() -> Error {
    Error::InvalidModel {
        message: "GRID_INVALID_CONTAINMENT: A Grid port has no measured owner".to_owned(),
    }
}

fn filled<T: Clone>(len: usize, value: T) -> Result<Vec<T>> {
    let mut values = Vec::new();
    reserve(&mut values, len)?;
    values.resize(len, value);
    Ok(values)
}

fn compare_numbers(a: f64, b: f64) -> Ordering {
    // JavaScript's subtraction comparators treat signed zeros and NaN as ties.
    (a - b).partial_cmp(&0.0).unwrap_or(Ordering::Equal)
}

fn clamp(value: f64, low: f64, high: f64) -> f64 {
    // The raw hierarchy interval can be reversed on a narrow side.
    value.min(high).max(low)
}

fn separate_coordinates(
    coordinates: &mut [f64],
    low: f64,
    high: f64,
    work: &OperationWorkMeter,
) -> Result<()> {
    let (first, rest) = coordinates
        .split_first_mut()
        .expect("nonempty port coordinate list");
    work.charge(1)?;
    let mut previous = *first;
    let mut desired_sum = 0.0 + previous;
    for coordinate in rest {
        work.charge(1)?;
        desired_sum += *coordinate;
        *coordinate = coordinate.max(previous + MIN_PORT_SEPARATION);
        previous = *coordinate;
    }
    let mut limit = high;
    for coordinate in coordinates.iter_mut().rev() {
        work.charge(1)?;
        *coordinate = coordinate.min(limit);
        limit = *coordinate - MIN_PORT_SEPARATION;
    }
    let mut assigned_sum = 0.0;
    for coordinate in coordinates.iter() {
        work.charge(1)?;
        assigned_sum += coordinate;
    }
    let len = coordinates.len() as f64;
    let shift = clamp(
        desired_sum / len - assigned_sum / len,
        low - coordinates[0],
        high - coordinates[coordinates.len() - 1],
    );
    for coordinate in coordinates {
        work.charge(1)?;
        *coordinate += shift;
    }
    Ok(())
}

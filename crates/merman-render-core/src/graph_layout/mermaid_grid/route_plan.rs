//! Hierarchical endpoint ownership and bundle lane order from Mermaid 12.4.0 `routerPlanning.ts`.

use super::forest::{GridForest, GridForestGeometry, GridItemKind};
use super::geometry::reserve;
use crate::graph_layout::{Axis, Point, Rect, Side};
use crate::resources::OperationWorkMeter;
use crate::{Error, Result};
use std::collections::{BTreeMap, HashSet};

pub(super) const PIXEL_EPSILON: f64 = 1.0;
pub(super) const CLEARANCE: f64 = 6.0;
const LANE_SEPARATION: f64 = 8.0;
pub(super) const SIDES: [Side; 4] = [Side::Right, Side::Bottom, Side::Left, Side::Top];

#[derive(Debug, Clone, Copy)]
pub(super) struct GridRouteEdge<'a> {
    pub id: &'a str,
    pub source: usize,
    pub target: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GridEndpoint {
    pub owner: usize,
    pub side: Side,
    pub opposite_coordinate: f64,
    pub preferred_coordinate: f64,
    pub compact_portal: bool,
    ambiguous: u8,
}

#[derive(Debug)]
pub(super) struct GridEndpointPlan {
    pub chain: Vec<GridEndpoint>,
    pub boundary: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct GridBundleLane {
    pub pair: usize,
    pub index: usize,
    pub count: usize,
    pub offset: f64,
}

#[derive(Debug)]
pub(super) struct GridRoutePlan<'a> {
    pub edge: GridRouteEdge<'a>,
    pub container: usize,
    pub source: GridEndpointPlan,
    pub target: GridEndpointPlan,
    pub lane: GridBundleLane,
}

pub(super) fn side_index(side: Side) -> usize {
    match side {
        Side::Right => 0,
        Side::Bottom => 1,
        Side::Left => 2,
        Side::Top => 3,
    }
}

fn side_bit(side: Side) -> u8 {
    1 << side_index(side)
}

fn item_bounds(geometry: &GridForestGeometry, slot: usize) -> Option<Rect> {
    geometry.item(slot).map(|item| item.bounds)
}

pub(super) fn title_bottom(geometry: &GridForestGeometry, slot: usize) -> Option<f64> {
    let title = geometry.containers.get(slot)?.as_ref()?.title_bounds?;
    Some(title.y + title.height)
}

pub(super) fn legal_side_interval(
    geometry: &GridForestGeometry,
    owner: usize,
    side: Side,
) -> Option<(f64, f64)> {
    let rect = item_bounds(geometry, owner)?;
    let title = title_bottom(geometry, owner);
    let (low, high) = match side.axis() {
        Axis::Horizontal => (
            (rect.y + CLEARANCE).max(title.map_or(f64::NEG_INFINITY, |bottom| bottom + CLEARANCE)),
            rect.y + rect.height - CLEARANCE,
        ),
        Axis::Vertical if side == Side::Top && title.is_some() => return None,
        Axis::Vertical => (rect.x + CLEARANCE, rect.x + rect.width - CLEARANCE),
    };
    (low <= high).then_some((low, high))
}

pub(super) fn tangential(point: Point, side: Side) -> f64 {
    match side.axis() {
        Axis::Horizontal => point.y,
        Axis::Vertical => point.x,
    }
}

pub(super) fn side_midpoint(rect: Rect, side: Side) -> Point {
    let center = rect.center();
    match side {
        Side::Right => Point {
            x: rect.x + rect.width,
            ..center
        },
        Side::Bottom => Point {
            y: rect.y + rect.height,
            ..center
        },
        Side::Left => Point {
            x: rect.x,
            ..center
        },
        Side::Top => Point {
            y: rect.y,
            ..center
        },
    }
}

pub(super) fn parent(forest: &GridForest<'_>, slot: usize) -> Option<usize> {
    forest.parents.get(slot).copied().flatten()
}

pub(super) fn allocation_error() -> Error {
    Error::LayoutAllocationFailed { backend: "grid" }
}

fn container_error() -> Error {
    Error::InvalidModel {
        message: "GRID_INVALID_CONTAINMENT: A Grid endpoint has no measured parent group"
            .to_owned(),
    }
}

fn ancestor(
    forest: &GridForest<'_>,
    ancestor: usize,
    node: usize,
    work: &OperationWorkMeter,
) -> Result<bool> {
    let mut cursor = parent(forest, node);
    while let Some(current) = cursor {
        work.charge(1)?;
        if current == ancestor {
            return Ok(true);
        }
        cursor = parent(forest, current);
    }
    Ok(false)
}

fn common_container(
    forest: &GridForest<'_>,
    edge: GridRouteEdge<'_>,
    work: &OperationWorkMeter,
) -> Result<usize> {
    let mut target_chain = HashSet::new();
    let mut cursor = parent(forest, edge.target);
    while let Some(current) = cursor {
        work.charge(1)?;
        if current == edge.source {
            return parent(forest, edge.source).ok_or_else(container_error);
        }
        target_chain
            .try_reserve(1)
            .map_err(|_| allocation_error())?;
        target_chain.insert(current);
        cursor = parent(forest, current);
    }
    cursor = parent(forest, edge.source);
    while let Some(current) = cursor {
        work.charge(1)?;
        if current == edge.target {
            return parent(forest, edge.target).ok_or_else(container_error);
        }
        if target_chain.contains(&current) {
            return Ok(current);
        }
        cursor = parent(forest, current);
    }
    Err(container_error())
}

fn endpoint(
    forest: &GridForest<'_>,
    geometry: &GridForestGeometry,
    endpoint: usize,
    other: usize,
    container: usize,
    work: &OperationWorkMeter,
) -> Result<GridEndpointPlan> {
    let mut chain = Vec::new();
    let endpoint_center = item_bounds(geometry, endpoint)
        .ok_or_else(container_error)?
        .center();
    let other_center = item_bounds(geometry, other)
        .ok_or_else(container_error)?
        .center();
    let mut current = endpoint;
    loop {
        work.charge(1)?;
        let parent = parent(forest, current).ok_or_else(container_error)?;
        let center = item_bounds(geometry, current)
            .ok_or_else(container_error)?
            .center();
        let own_title = title_bottom(geometry, current).is_some();
        let parent_title = title_bottom(geometry, parent).is_some();
        let dx = other_center.x - center.x;
        let dy = other_center.y - center.y;
        let horizontal = if dx >= 0.0 { Side::Right } else { Side::Left };
        let vertical = if dy >= 0.0 { Side::Bottom } else { Side::Top };
        let top_blocked = own_title || parent_title;
        let mut ambiguous = if dy < 0.0 && dx.abs() <= PIXEL_EPSILON && top_blocked {
            side_bit(Side::Left) | side_bit(Side::Right)
        } else if (dx.abs() - dy.abs()).abs() <= PIXEL_EPSILON && dx.abs() > PIXEL_EPSILON {
            side_bit(horizontal) | side_bit(vertical)
        } else {
            0
        };
        if top_blocked {
            ambiguous &= !side_bit(Side::Top)
        }
        if ambiguous.count_ones() <= 1 {
            ambiguous = 0
        }
        let side = if dx.abs() >= dy.abs() || (vertical == Side::Top && top_blocked) {
            horizontal
        } else {
            vertical
        };
        chain.try_reserve(1).map_err(|_| allocation_error())?;
        chain.push(GridEndpoint {
            owner: current,
            side,
            opposite_coordinate: tangential(other_center, side),
            preferred_coordinate: tangential(endpoint_center, side),
            compact_portal: current != endpoint,
            ambiguous,
        });
        if current == container
            && matches!(forest.items[current].kind, GridItemKind::Group(_))
            && ancestor(forest, current, other, work)?
        {
            return Ok(GridEndpointPlan {
                chain,
                boundary: true,
            });
        }
        if parent == container {
            return Ok(GridEndpointPlan {
                chain,
                boundary: false,
            });
        }
        if parent >= forest.items.len() {
            return Err(container_error());
        }
        current = parent;
    }
}

pub(super) fn code_units(left: &str, right: &str) -> std::cmp::Ordering {
    left.encode_utf16().cmp(right.encode_utf16())
}

pub(super) fn sort_charge(
    count: usize,
    max_key_len: usize,
    work: &OperationWorkMeter,
) -> Result<()> {
    let cost = count
        .checked_mul(count.checked_ilog2().unwrap_or(0) as usize + 1)
        .and_then(|cost| cost.checked_mul(max_key_len.max(1)))
        .ok_or_else(|| work.arithmetic_overflow())?;
    work.charge(cost)?;
    Ok(())
}

pub(super) fn plan_grid_edges<'a>(
    forest: &GridForest<'_>,
    geometry: &GridForestGeometry,
    edges: &[GridRouteEdge<'a>],
    work: &OperationWorkMeter,
) -> Result<Vec<GridRoutePlan<'a>>> {
    work.charge(edges.len())?;
    let mut plans = Vec::new();
    reserve(&mut plans, edges.len())?;
    let mut pairs = BTreeMap::<String, Vec<(usize, String)>>::new();
    let mut endpoint_order = Vec::new();
    let endpoints = edges
        .len()
        .checked_mul(2)
        .ok_or_else(|| work.arithmetic_overflow())?;
    work.charge(endpoints)?;
    reserve(&mut endpoint_order, endpoints)?;
    for (index, &edge) in edges.iter().enumerate() {
        work.charge(1)?;
        let fixed = |slot: usize| {
            forest
                .items
                .get(slot)
                .filter(|item| !matches!(item.kind, GridItemKind::EdgeLabel))
                .map(|item| item.item.id)
        };
        let (Some(source), Some(target)) = (fixed(edge.source), fixed(edge.target)) else {
            return Err(Error::InvalidModel {
                message: format!(
                    "GRID_MISSING_ENDPOINT: Missing endpoint for edge \"{}\"",
                    edge.id
                ),
            });
        };
        let size = source
            .len()
            .checked_add(target.len())
            .and_then(|size| size.checked_add(edge.id.len()))
            .ok_or_else(|| work.arithmetic_overflow())?;
        work.charge(
            size.checked_mul(4)
                .ok_or_else(|| work.arithmetic_overflow())?,
        )?;
        let pair = if code_units(source, target).is_lt() {
            format!("{source}|{target}")
        } else {
            format!("{target}|{source}")
        };
        let directed = format!("{source}|{target}");
        endpoint_order.extend([
            (index, false, format!("{directed}|{}|source", edge.id)),
            (index, true, format!("{directed}|{}|target", edge.id)),
        ]);
        let entries = pairs.entry(pair).or_default();
        entries.try_reserve(1).map_err(|_| allocation_error())?;
        entries.push((index, directed));
        let container = common_container(forest, edge, work)?;
        plans.push(GridRoutePlan {
            edge,
            container,
            source: endpoint(forest, geometry, edge.source, edge.target, container, work)?,
            target: endpoint(forest, geometry, edge.target, edge.source, container, work)?,
            lane: GridBundleLane {
                pair: 0,
                index: 0,
                count: 1,
                offset: 0.0,
            },
        });
    }
    for (pair, entries) in pairs.values_mut().enumerate() {
        let mut max_key = 0usize;
        for (index, directed) in entries.iter() {
            work.charge(1)?;
            max_key = max_key.max(directed.len().saturating_add(edges[*index].id.len()));
        }
        sort_charge(entries.len(), max_key, work)?;
        entries.sort_by(|(a, a_key), (b, b_key)| {
            code_units(a_key, b_key).then_with(|| code_units(edges[*a].id, edges[*b].id))
        });
        let count = entries.len();
        for (index, (edge, _)) in entries.iter().enumerate() {
            work.charge(1)?;
            plans[*edge].lane = GridBundleLane {
                pair,
                index,
                count,
                offset: (index as f64 - (count as f64 - 1.0) / 2.0) * LANE_SEPARATION,
            };
        }
    }
    work.charge(forest.items.len())?;
    let mut reservations = Vec::new();
    reserve(&mut reservations, forest.items.len())?;
    reservations.resize(forest.items.len(), [0usize; 4]);
    for plan in &plans {
        for end in plan.source.chain.iter().chain(&plan.target.chain) {
            work.charge(1)?;
            if end.ambiguous == 0 {
                reservations[end.owner][side_index(end.side)] += 1;
            }
        }
    }
    let mut max_key = 0usize;
    for (_, _, key) in &endpoint_order {
        work.charge(1)?;
        max_key = max_key.max(key.len());
    }
    sort_charge(endpoint_order.len(), max_key, work)?;
    endpoint_order.sort_by(|(_, _, a), (_, _, b)| code_units(a, b));
    for (index, target, _) in endpoint_order {
        let plan = &mut plans[index];
        resolve_ambiguous(
            if target {
                &mut plan.target
            } else {
                &mut plan.source
            },
            geometry,
            &mut reservations,
            work,
        )?;
    }
    for plan in &mut plans {
        let source = plan.source.chain.last().expect("nonempty source plan");
        let target = plan.target.chain.last().expect("nonempty target plan");
        let source_mid = side_midpoint(
            item_bounds(geometry, source.owner).ok_or_else(container_error)?,
            source.side,
        );
        let target_mid = side_midpoint(
            item_bounds(geometry, target.owner).ok_or_else(container_error)?,
            target.side,
        );
        for end in &mut plan.source.chain {
            work.charge(1)?;
            end.opposite_coordinate = tangential(target_mid, end.side);
        }
        for end in &mut plan.target.chain {
            work.charge(1)?;
            end.opposite_coordinate = tangential(source_mid, end.side);
        }
    }
    work.charge(0)?;
    Ok(plans)
}

fn resolve_ambiguous(
    plan: &mut GridEndpointPlan,
    geometry: &GridForestGeometry,
    reservations: &mut [[usize; 4]],
    work: &OperationWorkMeter,
) -> Result<()> {
    let mut legal = 0b1111u8;
    let mut counts = [0usize; 4];
    let mut preferred = None;
    for end in &plan.chain {
        work.charge(1)?;
        if end.ambiguous == 0 {
            continue;
        }
        preferred.get_or_insert(end.side);
        legal &= end.ambiguous;
        for side in SIDES {
            if legal & side_bit(side) == 0 {
                continue;
            }
            let index = side_index(side);
            if legal_side_interval(geometry, end.owner, side).is_some() {
                counts[index] = counts[index]
                    .checked_add(reservations[end.owner][index])
                    .ok_or_else(|| work.arithmetic_overflow())?;
            } else {
                legal &= !side_bit(side);
            }
        }
    }
    let Some(preferred) = preferred else {
        return Ok(());
    };
    let Some(side) = SIDES
        .into_iter()
        .filter(|&side| legal & side_bit(side) != 0)
        .min_by_key(|&side| (counts[side_index(side)], side != preferred))
    else {
        return Ok(());
    };
    for end in &mut plan.chain {
        work.charge(1)?;
        if end.ambiguous == 0 {
            continue;
        }
        end.side = side;
        reservations[end.owner][side_index(side)] += 1;
        end.preferred_coordinate = tangential(
            item_bounds(geometry, end.owner)
                .ok_or_else(container_error)?
                .center(),
            side,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_layout::Size;
    use crate::graph_layout::mermaid_grid::forest::{GridMeasuredItem, layout_measured_forest};
    use crate::graph_layout::mermaid_grid::geometry::{
        GridGroupMeasure, GridSpacing, GridTrackGaps,
    };
    use crate::graph_layout::mermaid_grid::placement::{
        GridCoordinate, GridItem, GridPlacementOptions, GridPlacementSpec,
    };
    use crate::resources::RenderResourcePolicy;
    use std::collections::HashMap;

    const LEAF: GridItemKind = GridItemKind::Leaf(Size {
        width: 40.0,
        height: 40.0,
    });
    const GROUP: GridItemKind = GridItemKind::Group(GridGroupMeasure {
        padding: 20.0,
        title: None,
    });

    fn item(
        id: &'static str,
        parent_id: Option<&'static str>,
        kind: GridItemKind,
        row: i64,
        column: i64,
    ) -> GridMeasuredItem<'static> {
        GridMeasuredItem {
            parent_id,
            kind,
            item: GridItem {
                id,
                authored_id: None,
                source_ordinal: None,
                inline: GridPlacementSpec {
                    row: GridCoordinate::new(row),
                    column: GridCoordinate::new(column),
                    ..GridPlacementSpec::default()
                },
            },
        }
    }

    fn forest_geometry<'a>(
        items: &'a [GridMeasuredItem<'a>],
        work: &OperationWorkMeter,
    ) -> (GridForest<'a>, GridForestGeometry) {
        let forest = GridForest::new(items, work).unwrap();
        let geometry = layout_measured_forest(
            &forest,
            &HashMap::new(),
            GridPlacementOptions::default(),
            GridSpacing {
                row_gap: 50.0,
                column_gap: 50.0,
                cell_gap: 20.0,
                container_padding: 20.0,
                title_gap: 8.0,
            },
            |_, _, _| Ok(GridTrackGaps::default()),
            work,
        )
        .unwrap();
        (forest, geometry)
    }

    #[test]
    fn endpoint_chain_walks_outward_and_ancestor_edges_keep_the_outer_container() {
        let items = [
            item("outside", None, LEAF, 1, 1),
            item("outer", None, GROUP, 1, 2),
            item("inner", Some("outer"), GROUP, 1, 1),
            item("child", Some("inner"), LEAF, 1, 1),
        ];
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let (forest, geometry) = forest_geometry(&items, &work);
        let input = [
            GridRouteEdge {
                id: "cross",
                source: 0,
                target: 3,
            },
            GridRouteEdge {
                id: "ancestor",
                source: 1,
                target: 3,
            },
        ];
        let output = plan_grid_edges(&forest, &geometry, &input, &work).unwrap();
        for plan in &output {
            assert_eq!(plan.container, items.len());
            assert!(!plan.source.boundary && !plan.target.boundary);
            assert_eq!(
                plan.target
                    .chain
                    .iter()
                    .map(|entry| (entry.owner, entry.compact_portal))
                    .collect::<Vec<_>>(),
                [(3, false), (2, true), (1, true)]
            );
        }
        assert_eq!(output[0].edge.id, "cross");
        assert_eq!(output[0].source.chain[0].side, Side::Right);
        assert_eq!(output[0].target.chain.last().unwrap().side, Side::Left);
        assert_eq!(
            output[0].target.chain.last().unwrap().opposite_coordinate,
            geometry.item(0).unwrap().bounds.center().y
        );
    }

    #[test]
    fn bundle_order_uses_directed_code_units_and_then_edge_id() {
        let items = [item("a", None, LEAF, 1, 1), item("a0", None, LEAF, 1, 2)];
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let (forest, geometry) = forest_geometry(&items, &work);
        let input = [
            GridRouteEdge {
                id: "z",
                source: 0,
                target: 1,
            },
            GridRouteEdge {
                id: "other",
                source: 1,
                target: 0,
            },
            GridRouteEdge {
                id: "a",
                source: 0,
                target: 1,
            },
        ];
        let output = plan_grid_edges(&forest, &geometry, &input, &work).unwrap();
        assert_eq!(
            output
                .iter()
                .map(|plan| (plan.lane.index, plan.lane.count, plan.lane.offset))
                .collect::<Vec<_>>(),
            [(2, 3, 8.0), (0, 3, -8.0), (1, 3, 0.0)]
        );
    }

    #[test]
    fn group_title_blocks_top_and_cuts_side_port_intervals() {
        let titled = GridItemKind::Group(GridGroupMeasure {
            padding: 20.0,
            title: Some(Size {
                width: 72.0,
                height: 24.0,
            }),
        });
        let items = [
            item("outer", None, titled, 2, 1),
            item("inside", Some("outer"), LEAF, 1, 1),
            item("above", None, LEAF, 1, 1),
        ];
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let (forest, geometry) = forest_geometry(&items, &work);
        assert!(legal_side_interval(&geometry, 0, Side::Top).is_none());
        let (low, _) = legal_side_interval(&geometry, 0, Side::Right).unwrap();
        assert_eq!(low, title_bottom(&geometry, 0).unwrap() + CLEARANCE);
        let plans = plan_grid_edges(
            &forest,
            &geometry,
            &[GridRouteEdge {
                id: "up",
                source: 1,
                target: 2,
            }],
            &work,
        )
        .unwrap();
        assert!(
            plans[0]
                .source
                .chain
                .iter()
                .all(|entry| entry.side != Side::Top)
        );
    }
}

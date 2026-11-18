use super::super::{PhysicalPortSide, RelationComponentAdapter, layout_allocation_failed};
use super::batch::LayeredRouteBatchError;
use super::boxes::PlacedRelationGraphBox;
use super::route::{
    LayeredRelationPhysicalPort, LayeredRelationRouteGeometry, LayeredRelationRoutePlan,
    LayeredRelationRouteProfile, RelationOverlay, materialize_layered_relation_route_plan,
};
use super::scene::{LayeredRelationScene, LayeredRelationSummaryReason};
use crate::Result;
use crate::resource::ResourceContext;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

#[derive(Clone, Copy)]
struct ChannelGrid<'a> {
    width: usize,
    height: usize,
    occupied: &'a [bool],
    retained: usize,
}

struct EdgeChannels<'a, 'boxes> {
    scene: &'a LayeredRelationScene<'boxes>,
    source: &'a PlacedRelationGraphBox<'boxes>,
    target: &'a PlacedRelationGraphBox<'boxes>,
    sources: &'a [LayeredRelationPhysicalPort],
    targets: &'a [LayeredRelationPhysicalPort],
    preferred: PhysicalPortSide,
    label_width: usize,
    label_height: usize,
}

struct ChannelRoute {
    source: LayeredRelationPhysicalPort,
    target: LayeredRelationPhysicalPort,
    path: Vec<(usize, usize)>,
}

#[derive(Clone, Copy)]
enum LabelLane {
    Between,
    Before,
    After,
}

/// Bounded occupancy grids distinguish real terminals from box clearance.
/// Search never borrows another edge's stem or label area. A failed semantic
/// candidate is reported to the existing component-summary transaction.
pub(super) fn plan_private_routes<'text, R, A>(
    scene: &LayeredRelationScene<'_>,
    relations: &[&R],
    adapter: &A,
    resources: &ResourceContext,
) -> std::result::Result<Vec<LayeredRelationRoutePlan>, LayeredRouteBatchError>
where
    A: RelationComponentAdapter<'text, R>,
{
    let width = scene.width();
    let height = scene.height();
    let cells = resources.checked_grid_mul(width, height)?;
    resources.charge_layout_work(cells)?;
    resources.grid_extent(resources.checked_grid_mul(cells, 2)?, 1)?;
    let mut occupied = filled(cells, false)?;
    let mut obstacles = filled(cells, false)?;
    for node in scene.placed_boxes() {
        reserve_rect(
            &mut occupied,
            width,
            node.x(),
            node.y(),
            node.right() + 1,
            node.bottom() + 1,
            resources,
        )?;
        reserve_rect(
            &mut obstacles,
            width,
            node.x().saturating_sub(1),
            node.y().saturating_sub(1),
            (node.right() + 2).min(width),
            (node.bottom() + 2).min(height),
            resources,
        )?;
    }
    let mut order = Vec::new();
    order
        .try_reserve_exact(scene.draw_order().len())
        .map_err(|_| layout_allocation_failed())?;
    order.extend(scene.draw_order().iter().map(|&(index, _)| index));
    resources.charge_layout_work(super::comparison_sort_work(order.len(), resources)?)?;
    order.sort_unstable_by_key(|&index| {
        let edge = scene.edge(index);
        let endpoints = scene.edge_endpoints(index);
        let backward = endpoints.is_some_and(|(source, target)| {
            if scene.direction().is_horizontal() {
                (source.x() >= target.x()) != scene.direction().is_reversed()
            } else {
                source.y() >= target.y()
            }
        });
        let span = endpoints
            .map(|(source, target)| {
                source.center_x().abs_diff(target.center_x()) + source.y().abs_diff(target.y())
            })
            .unwrap_or(0);
        (
            !backward,
            span,
            edge.map(|edge| edge.route_source_id()),
            edge.map(|edge| edge.route_target_id()),
            index,
        )
    });
    let mut routes = Vec::new();
    routes
        .try_reserve_exact(order.len())
        .map_err(|_| layout_allocation_failed())?;
    let mut retained_routes = routes.capacity();
    for index in order {
        resources.checkpoint()?;
        let relation = relations.get(index).copied().ok_or_else(route_collision)?;
        let edge = scene.edge(index).ok_or_else(route_collision)?;
        let (source, target) = scene.edge_endpoints(index).ok_or_else(route_collision)?;
        let style = adapter.layered_route_style(relation)?;
        let preferred = if scene.direction().is_horizontal() {
            if scene.direction().is_reversed() {
                PhysicalPortSide::Left
            } else {
                PhysicalPortSide::Right
            }
        } else {
            PhysicalPortSide::Bottom
        };
        let port_grid = ChannelGrid {
            width,
            height,
            occupied: &occupied,
            retained: 0,
        };
        let sources = ports(
            source,
            true,
            style.profile(),
            preferred,
            port_grid,
            resources,
        )?;
        let targets = ports(
            target,
            false,
            style.profile(),
            preferred.opposite(),
            port_grid,
            resources,
        )?;
        let retained = resources.checked_grid_add(
            resources.checked_grid_add(occupied.len(), retained_routes)?,
            resources.checked_grid_add(sources.capacity(), targets.capacity())?,
        )?;
        let search_grid = ChannelGrid {
            width,
            height,
            occupied: &obstacles,
            retained,
        };
        let request = EdgeChannels {
            scene,
            source,
            target,
            sources: &sources,
            targets: &targets,
            preferred,
            label_width: edge.label_width(),
            label_height: edge.label_line_count(),
        };
        let mut accepted = None;
        let mut label_failure = false;
        for attempt in 0..4 {
            let candidate = if attempt == 0 {
                search_path(&sources, &targets, preferred, search_grid, resources)?
            } else {
                let lane = match attempt {
                    1 => LabelLane::Between,
                    2 => LabelLane::Before,
                    _ => LabelLane::After,
                };
                label_lane_path(&request, search_grid, lane, resources)?
            };
            let Some(ChannelRoute {
                source: source_port,
                target: target_port,
                path: searched,
            }) = candidate
            else {
                continue;
            };
            let path = connect_box_contacts(
                source_port,
                target_port,
                source,
                target,
                &searched,
                resources,
            )?;
            let mut geometry = LayeredRelationRouteGeometry::from_channel_path(
                source_port,
                target_port,
                &path,
                (0, 0),
                resources,
            )?;
            // Labels may replace a straight part of their own edge, but never
            // a turn, a terminal, or another edge. Exterior lanes provide a
            // measured label runway when a shortest route has no reading area.
            for offset in 0..path.len() {
                resources.checkpoint()?;
                let middle = path.len() / 2;
                let position = if offset % 2 == 0 {
                    middle + offset / 2
                } else {
                    middle.saturating_sub(offset / 2 + 1)
                };
                let Some(&(x, y)) = path.get(position) else {
                    continue;
                };
                geometry.set_private_label_anchor(
                    x,
                    y.saturating_sub(edge.label_line_count().saturating_sub(1) / 2),
                );
                let overlays = adapter.layered_relation_overlays(relation, &geometry, resources)?;
                if overlays_clear(&overlays, width, height, &occupied, resources)?
                    && labels_avoid_turns_and_terminals(&overlays, &path, resources)?
                {
                    accepted = Some((source_port, target_port, path, geometry, overlays));
                    break;
                }
            }
            if accepted.is_some() {
                break;
            }
            label_failure = true;
        }
        let Some((source_port, target_port, path, geometry, overlays)) = accepted else {
            return Err(if label_failure {
                LayeredRouteBatchError::Semantic(LayeredRelationSummaryReason::OverlayCollision)
            } else {
                route_collision()
            });
        };
        if !scene.edge_ports_fit(index, &geometry) || !geometry.connects_boxes(source, target) {
            return Err(route_collision());
        }
        for &(x, y) in &path {
            resources.charge_layout_work(1)?;
            occupied[y * width + x] = true;
            obstacles[y * width + x] = true;
        }
        for overlay in &overlays {
            if let Some(bounds) = overlay.bounds() {
                let padding = 1;
                reserve_rect(
                    &mut occupied,
                    width,
                    bounds.left.saturating_sub(padding),
                    bounds.top,
                    bounds.right.saturating_add(padding).min(width),
                    bounds.bottom,
                    resources,
                )?;
                reserve_rect(
                    &mut obstacles,
                    width,
                    bounds.left.saturating_sub(padding),
                    bounds.top,
                    bounds.right.saturating_add(padding).min(width),
                    bounds.bottom,
                    resources,
                )?;
            }
        }
        reserve_port(&mut occupied, width, source_port, source, resources)?;
        reserve_port(&mut occupied, width, target_port, target, resources)?;
        retained_routes = resources.checked_grid_add(retained_routes, path.len())?;
        routes.push(materialize_layered_relation_route_plan(
            geometry, style, resources, overlays,
        )?);
    }
    Ok(routes)
}

fn ports(
    node: &PlacedRelationGraphBox<'_>,
    source: bool,
    profile: LayeredRelationRouteProfile,
    preferred: PhysicalPortSide,
    grid: ChannelGrid<'_>,
    resources: &ResourceContext,
) -> Result<Vec<LayeredRelationPhysicalPort>> {
    let ChannelGrid {
        width,
        height,
        occupied,
        ..
    } = grid;
    resources.checkpoint()?;
    let mut result = Vec::new();
    let perimeter =
        resources.checked_grid_mul(resources.checked_grid_add(node.width(), node.height())?, 2)?;
    resources.charge_layout_work(perimeter)?;
    result
        .try_reserve_exact(perimeter)
        .map_err(|_| layout_allocation_failed())?;
    for side in [preferred, preferred.opposite()]
        .into_iter()
        .chain(PhysicalPortSide::ALL)
    {
        if result
            .iter()
            .any(|port: &LayeredRelationPhysicalPort| port.side == side)
        {
            continue;
        }
        let horizontal = matches!(side, PhysicalPortSide::Left | PhysicalPortSide::Right);
        let (start, length) = if horizontal {
            (node.y(), node.height())
        } else {
            (node.x(), node.width())
        };
        // Endpoint labels may reserve a straight runway beyond the terminal,
        // but the terminal itself always touches its box.
        let label_runway = if horizontal {
            0
        } else {
            profile.endpoint_label_gap
        };
        let marker_width = if source {
            profile.source_marker_widths
        } else {
            profile.target_marker_widths
        }[usize::from(horizontal)];
        let marker_offset = resources.checked_grid_add(
            1,
            if side == PhysicalPortSide::Right {
                marker_width / 2
            } else {
                0
            },
        )?;
        let mut path_offset = resources
            .checked_grid_add(
                label_runway,
                if source {
                    profile.source_path_start_offset
                } else {
                    resources.checked_grid_add(profile.target_path_end_offset_from_target, 1)?
                },
            )?
            .max(2);
        if horizontal {
            path_offset = path_offset.max(resources.checked_grid_add(
                marker_offset,
                if side == PhysicalPortSide::Left {
                    marker_width / 2
                } else {
                    marker_width.saturating_sub(1) / 2
                },
            )?);
        }
        for offset in 0..length {
            resources.checkpoint()?;
            if length > 2 && (offset == 0 || offset + 1 == length) {
                continue;
            }
            let contact = resources.checked_grid_add(start, offset)?;
            let coordinates = match side {
                PhysicalPortSide::Top => node
                    .y()
                    .checked_sub(marker_offset)
                    .zip(node.y().checked_sub(path_offset))
                    .map(|(marker, path)| (contact, marker, contact, path)),
                PhysicalPortSide::Bottom => Some((
                    contact,
                    resources.checked_grid_add(node.bottom(), marker_offset)?,
                    contact,
                    resources.checked_grid_add(node.bottom(), path_offset)?,
                )),
                PhysicalPortSide::Left => node
                    .x()
                    .checked_sub(marker_offset)
                    .zip(node.x().checked_sub(path_offset))
                    .map(|(marker, path)| (marker, contact, path, contact)),
                PhysicalPortSide::Right => Some((
                    resources.checked_grid_add(node.right(), marker_offset)?,
                    contact,
                    resources.checked_grid_add(node.right(), path_offset)?,
                    contact,
                )),
            };
            let Some((marker_x, marker_y, path_x, path_y)) = coordinates else {
                continue;
            };
            if marker_x >= width || marker_y >= height || path_x >= width || path_y >= height {
                continue;
            }
            let Some(left) = marker_x.checked_sub(marker_width / 2) else {
                continue;
            };
            if left + marker_width > width {
                continue;
            }
            let port = LayeredRelationPhysicalPort {
                side,
                marker_x,
                marker_y,
                path_x,
                path_y,
            };
            if let Some((label_width, label_height)) =
                profile.endpoint_label_sizes[usize::from(!source)]
            {
                let (anchor, top) = port.private_endpoint_label_anchor(label_height, resources)?;
                let Some(left) = anchor.left(label_width) else {
                    continue;
                };
                let right = resources.checked_grid_add(left, label_width)?;
                let bottom = resources.checked_grid_add(top, label_height)?;
                // Select a usable contact before searching its path. Moving a central
                // caption cannot repair endpoint text that already overlaps a box.
                if right > width
                    || bottom > height
                    || !rect_clear(
                        occupied,
                        width,
                        left.saturating_sub(1),
                        top,
                        right.saturating_add(1).min(width),
                        bottom,
                        resources,
                    )?
                {
                    continue;
                }
            }
            if !occupied[path_y * width + path_x]
                && rect_clear(
                    occupied,
                    width,
                    left,
                    marker_y,
                    left + marker_width,
                    marker_y + 1,
                    resources,
                )?
                && port_clear(occupied, width, port, node, resources)?
            {
                result.push(port);
            }
        }
    }
    Ok(result)
}

fn search_path(
    sources: &[LayeredRelationPhysicalPort],
    targets: &[LayeredRelationPhysicalPort],
    preferred: PhysicalPortSide,
    grid: ChannelGrid<'_>,
    resources: &ResourceContext,
) -> Result<Option<ChannelRoute>> {
    let ChannelGrid {
        width,
        height,
        occupied,
        retained,
    } = grid;
    if sources.is_empty() || targets.is_empty() {
        return Ok(None);
    }
    let cells = resources.checked_grid_mul(width, height)?;
    let states = resources.checked_work_mul(cells, 4)?;
    resources.charge_layout_work(resources.checked_work_mul(states, 2)?)?;
    // Only distance and predecessor grids scale with heading count. Targets
    // are sparse; the reconstructed root identifies its source without an
    // owner grid. The heap and actual reconstruction coexistence are admitted
    // separately before either capacity grows.
    let resident = resources.checked_grid_add(
        resources.checked_grid_add(occupied.len(), retained)?,
        resources.checked_grid_add(resources.checked_grid_mul(states, 2)?, targets.len())?,
    )?;
    resources.grid_extent(resources.checked_grid_add(resident, sources.len())?, 1)?;
    let mut distances = filled(states, usize::MAX)?;
    let mut parents = filled(states, usize::MAX)?;
    let mut goals = Vec::new();
    goals
        .try_reserve_exact(targets.len())
        .map_err(|_| layout_allocation_failed())?;
    for &port in targets {
        goals.push((
            (port.path_y * width + port.path_x) * 4 + heading(port.side.opposite()),
            port,
        ));
    }
    resources.charge_layout_work(super::comparison_sort_work(goals.len(), resources)?)?;
    goals.sort_unstable_by_key(|&(state, _)| state);
    let lookup_work =
        usize::try_from(targets.len().ilog2()).map_err(|_| resources.work_overflow())? + 1;
    let min_x = targets.iter().map(|port| port.path_x).min().unwrap_or(0);
    let max_x = targets.iter().map(|port| port.path_x).max().unwrap_or(0);
    let min_y = targets.iter().map(|port| port.path_y).min().unwrap_or(0);
    let max_y = targets.iter().map(|port| port.path_y).max().unwrap_or(0);
    let heuristic =
        |x: usize, y: usize| x.abs_diff(x.clamp(min_x, max_x)) + y.abs_diff(y.clamp(min_y, max_y));
    let mut pending = BinaryHeap::new();
    pending
        .try_reserve_exact(sources.len())
        .map_err(|_| layout_allocation_failed())?;
    for source in sources {
        if occupied[source.path_y * width + source.path_x] {
            continue;
        }
        let state = (source.path_y * width + source.path_x) * 4 + heading(source.side);
        let cost = if source.side == preferred { 0 } else { 4 };
        if cost < distances[state] {
            distances[state] = cost;
            pending.push(Reverse((
                cost + heuristic(source.path_x, source.path_y),
                cost,
                state,
            )));
        }
    }
    while let Some(Reverse((_, cost, state))) = pending.pop() {
        resources.checkpoint()?;
        resources.charge_layout_work(lookup_work)?;
        if distances[state] != cost {
            continue;
        }
        let cell = state / 4;
        let (x, y) = (cell % width, cell / width);
        if let Ok(goal) = goals.binary_search_by_key(&state, |&(state, _)| state) {
            let target = goals[goal].1;
            drop(pending);
            let mut current = state;
            let mut count = 0usize;
            loop {
                resources.checkpoint()?;
                resources.charge_layout_work(1)?;
                // Search has finished, so its distance cells can mark visits
                // without allocating another full grid for reconstruction.
                if distances[(current / 4) * 4] == usize::MAX - 1 {
                    return Ok(None);
                }
                distances[(current / 4) * 4] = usize::MAX - 1;
                count = resources.checked_grid_add(count, 1)?;
                if parents[current] == usize::MAX {
                    break;
                }
                current = parents[current];
            }
            resources.charge_layout_work(sources.len())?;
            let Some(&source) = sources.iter().find(|port| {
                (port.path_y * width + port.path_x) * 4 + heading(port.side) == current
            }) else {
                return Ok(None);
            };
            resources.grid_extent(resources.checked_grid_add(resident, count)?, 1)?;
            let mut path = Vec::new();
            path.try_reserve_exact(count)
                .map_err(|_| layout_allocation_failed())?;
            current = state;
            loop {
                resources.checkpoint()?;
                resources.charge_layout_work(1)?;
                path.push(((current / 4) % width, (current / 4) / width));
                if parents[current] == usize::MAX {
                    break;
                }
                current = parents[current];
            }
            path.reverse();
            return Ok(Some(ChannelRoute {
                source,
                target,
                path,
            }));
        }
        for (next_x, next_y, direction) in [
            (Some(x), y.checked_sub(1), PhysicalPortSide::Top),
            (
                x.checked_add(1).filter(|x| *x < width),
                Some(y),
                PhysicalPortSide::Right,
            ),
            (
                Some(x),
                y.checked_add(1).filter(|y| *y < height),
                PhysicalPortSide::Bottom,
            ),
            (x.checked_sub(1), Some(y), PhysicalPortSide::Left),
        ] {
            resources.charge_layout_work(lookup_work)?;
            let (Some(next_x), Some(next_y)) = (next_x, next_y) else {
                continue;
            };
            let next_heading = heading(direction);
            if parents[state] == usize::MAX && next_heading != state % 4 {
                continue;
            }
            if next_heading == (state % 4 + 2) % 4 {
                continue;
            }
            let next_cell = next_y * width + next_x;
            if occupied[next_cell] {
                continue;
            }
            let next_state = next_cell * 4 + next_heading;
            let mut next_cost =
                resources.checked_work_add(cost, if state % 4 == next_heading { 1 } else { 4 })?;
            if let Ok(goal) = goals.binary_search_by_key(&next_state, |&(state, _)| state)
                && goals[goal].1.side != preferred.opposite()
            {
                next_cost = resources.checked_work_add(next_cost, 4)?;
            }
            if next_cost < distances[next_state] {
                distances[next_state] = next_cost;
                parents[next_state] = state;
                if pending.len() == pending.capacity() {
                    let growth = pending.capacity().max(8);
                    let requested = resources.checked_grid_add(pending.capacity(), growth)?;
                    resources.grid_extent(resources.checked_grid_add(resident, requested)?, 1)?;
                    pending
                        .try_reserve_exact(growth)
                        .map_err(|_| layout_allocation_failed())?;
                }
                pending.push(Reverse((
                    resources.checked_work_add(next_cost, heuristic(next_x, next_y))?,
                    next_cost,
                    next_state,
                )));
            }
        }
    }
    Ok(None)
}

fn label_lane_path(
    request: &EdgeChannels<'_, '_>,
    grid: ChannelGrid<'_>,
    lane: LabelLane,
    resources: &ResourceContext,
) -> Result<Option<ChannelRoute>> {
    let &EdgeChannels {
        scene,
        source,
        target,
        sources,
        targets,
        preferred,
        label_width,
        label_height,
    } = request;
    let ChannelGrid {
        width,
        height,
        occupied,
        retained,
    } = grid;
    if label_width == 0 || label_height == 0 {
        return Ok(None);
    }
    let horizontal = scene.direction().is_horizontal();
    let nodes = scene.placed_boxes();
    let (mut first, mut second, label_left, label_top) = if horizontal {
        let mid = if matches!(lane, LabelLane::Between) {
            if source.x() < target.x() {
                resources.checked_grid_add(source.right(), target.x())? / 2
            } else {
                resources.checked_grid_add(target.right(), source.x())? / 2
            }
        } else {
            resources.checked_grid_add(source.center_x(), target.center_x())? / 2
        };
        let lane = match lane {
            LabelLane::Between => Some(target.y() + target.height() / 2),
            LabelLane::Before => nodes
                .iter()
                .map(|node| node.y())
                .min()
                .unwrap_or(0)
                .checked_sub(label_height / 2 + 3),
            LabelLane::After => Some(resources.checked_grid_add(
                nodes.iter().map(|node| node.bottom()).max().unwrap_or(0),
                label_height / 2 + 3,
            )?),
        };
        let Some(lane) = lane else {
            return Ok(None);
        };
        let Some(left) = mid.checked_sub(label_width / 2) else {
            return Ok(None);
        };
        let Some(top) = lane.checked_sub(label_height / 2) else {
            return Ok(None);
        };
        let Some(first_x) = left.checked_sub(1) else {
            return Ok(None);
        };
        (
            (first_x, lane),
            (resources.checked_grid_add(left, label_width)?, lane),
            left,
            top,
        )
    } else {
        let source_y = resources.checked_grid_add(source.y(), source.height() / 2)?;
        let target_y = resources.checked_grid_add(target.y(), target.height() / 2)?;
        let mid = if matches!(lane, LabelLane::Between) {
            if source.y() < target.y() {
                resources.checked_grid_add(source.bottom(), target.y())? / 2
            } else {
                resources.checked_grid_add(target.bottom(), source.y())? / 2
            }
        } else {
            resources.checked_grid_add(source_y, target_y)? / 2
        };
        let lane = match lane {
            LabelLane::Between => Some(target.center_x()),
            LabelLane::Before => nodes
                .iter()
                .map(|node| node.x())
                .min()
                .unwrap_or(0)
                .checked_sub(label_width / 2 + 3),
            LabelLane::After => Some(resources.checked_grid_add(
                nodes.iter().map(|node| node.right()).max().unwrap_or(0),
                label_width / 2 + 3,
            )?),
        };
        let Some(lane) = lane else {
            return Ok(None);
        };
        let Some(left) = lane.checked_sub(label_width / 2) else {
            return Ok(None);
        };
        let Some(top) = mid.checked_sub(label_height / 2) else {
            return Ok(None);
        };
        let Some(first_y) = top.checked_sub(1) else {
            return Ok(None);
        };
        (
            (lane, first_y),
            (lane, resources.checked_grid_add(top, label_height)?),
            left,
            top,
        )
    };
    let label_right = resources.checked_grid_add(label_left, label_width)?;
    let label_bottom = resources.checked_grid_add(label_top, label_height)?;
    if first.0 >= width
        || second.0 >= width
        || first.1 >= height
        || second.1 >= height
        || label_right >= width
        || label_bottom >= height
    {
        return Ok(None);
    }
    if !rect_clear(
        occupied,
        width,
        label_left.saturating_sub(1),
        label_top,
        label_right + 1,
        label_bottom,
        resources,
    )? {
        return Ok(None);
    }
    if if horizontal {
        source.x() > target.x()
    } else {
        source.y() > target.y()
    } {
        std::mem::swap(&mut first, &mut second);
    }
    let bridge_length =
        resources.checked_grid_add(first.0.abs_diff(second.0), first.1.abs_diff(second.1))?;
    resources.grid_extent(
        resources.checked_grid_add(resources.checked_grid_mul(occupied.len(), 2)?, retained)?,
        1,
    )?;
    resources.charge_layout_work(occupied.len())?;
    let mut mask = filled(occupied.len(), false)?;
    mask.copy_from_slice(occupied);
    reserve_rect(
        &mut mask,
        width,
        label_left.saturating_sub(1),
        label_top,
        label_right + 1,
        label_bottom,
        resources,
    )?;
    let mut point = first;
    while point != second {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        if occupied[point.1 * width + point.0] {
            return Ok(None);
        }
        mask[point.1 * width + point.0] = true;
        point = step_toward(point, second);
    }
    if occupied[second.1 * width + second.0] {
        return Ok(None);
    }
    mask[first.1 * width + first.0] = false;
    mask[second.1 * width + second.0] = false;
    let bridge_direction = travel_direction(first, second);
    let waypoint = |(x, y), side| LayeredRelationPhysicalPort {
        side,
        marker_x: x,
        marker_y: y,
        path_x: x,
        path_y: y,
    };
    let retained_grids = resources.checked_grid_add(retained, occupied.len())?;
    let Some(ChannelRoute {
        source: source_port,
        mut path,
        ..
    }) = search_path(
        sources,
        &[waypoint(first, bridge_direction.opposite())],
        preferred,
        ChannelGrid {
            occupied: &mask,
            retained: retained_grids,
            ..grid
        },
        resources,
    )?
    else {
        return Ok(None);
    };
    for &(x, y) in &path {
        mask[y * width + x] = true;
    }
    mask[second.1 * width + second.0] = false;
    let retained = resources.checked_grid_add(retained_grids, path.capacity())?;
    let Some(ChannelRoute {
        target: target_port,
        path: tail,
        ..
    }) = search_path(
        &[waypoint(second, bridge_direction)],
        targets,
        preferred,
        ChannelGrid {
            occupied: &mask,
            retained,
            ..grid
        },
        resources,
    )?
    else {
        return Ok(None);
    };
    resources.charge_layout_work(resources.checked_work_add(bridge_length, tail.len())?)?;
    path.try_reserve_exact(resources.checked_grid_add(bridge_length, tail.len())?)
        .map_err(|_| layout_allocation_failed())?;
    point = first;
    while point != second {
        point = step_toward(point, second);
        path.push(point);
    }
    path.extend(tail.into_iter().skip(1));
    Ok(Some(ChannelRoute {
        source: source_port,
        target: target_port,
        path,
    }))
}

fn overlays_clear(
    overlays: &[RelationOverlay],
    width: usize,
    height: usize,
    occupied: &[bool],
    resources: &ResourceContext,
) -> Result<bool> {
    for (index, overlay) in overlays.iter().enumerate() {
        if !overlay.fits(width, height) {
            return Ok(false);
        }
        if let Some(bounds) = overlay.bounds() {
            let padding = usize::from(matches!(overlay, RelationOverlay::Label { .. }));
            if !rect_clear(
                occupied,
                width,
                bounds.left.saturating_sub(padding),
                bounds.top,
                bounds.right.saturating_add(padding).min(width),
                bounds.bottom,
                resources,
            )? {
                return Ok(false);
            }
            for other in &overlays[index + 1..] {
                resources.charge_layout_work(1)?;
                if other.overlaps_rect(
                    bounds.left,
                    bounds.top,
                    bounds.right.saturating_sub(1),
                    bounds.bottom.saturating_sub(1),
                ) {
                    return Ok(false);
                }
            }
        }
    }
    Ok(true)
}

fn labels_avoid_turns_and_terminals(
    overlays: &[RelationOverlay],
    path: &[(usize, usize)],
    resources: &ResourceContext,
) -> Result<bool> {
    for overlay in overlays {
        let RelationOverlay::Label { endpoint, .. } = overlay else {
            continue;
        };
        let Some(bounds) = overlay.bounds() else {
            continue;
        };
        let mut first = None;
        for (index, &(x, y)) in path.iter().enumerate() {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            if x < bounds.left || x >= bounds.right || y < bounds.top || y >= bounds.bottom {
                continue;
            }
            if *endpoint {
                return Ok(false);
            }
            if index == 0 || index + 1 == path.len() {
                return Ok(false);
            }
            let previous = path[index - 1];
            let next = path[index + 1];
            if (previous.0 == x) != (next.0 == x) {
                return Ok(false);
            }
            if let Some((first_x, first_y)) = first {
                if x != first_x && y != first_y {
                    return Ok(false);
                }
            } else {
                first = Some((x, y));
            }
        }
    }
    Ok(true)
}

fn contact_point(
    port: LayeredRelationPhysicalPort,
    node: &PlacedRelationGraphBox<'_>,
) -> (usize, usize) {
    match port.side {
        PhysicalPortSide::Top => (port.path_x, node.y() - 1),
        PhysicalPortSide::Bottom => (port.path_x, node.bottom() + 1),
        PhysicalPortSide::Left => (node.x() - 1, port.path_y),
        PhysicalPortSide::Right => (node.right() + 1, port.path_y),
    }
}

fn connect_box_contacts(
    source_port: LayeredRelationPhysicalPort,
    target_port: LayeredRelationPhysicalPort,
    source: &PlacedRelationGraphBox<'_>,
    target: &PlacedRelationGraphBox<'_>,
    searched: &[(usize, usize)],
    resources: &ResourceContext,
) -> Result<Vec<(usize, usize)>> {
    let source_contact = contact_point(source_port, source);
    let target_contact = contact_point(target_port, target);
    let source_path = (source_port.path_x, source_port.path_y);
    let target_path = (target_port.path_x, target_port.path_y);
    let source_length =
        source_contact.0.abs_diff(source_path.0) + source_contact.1.abs_diff(source_path.1);
    let target_length =
        target_contact.0.abs_diff(target_path.0) + target_contact.1.abs_diff(target_path.1);
    let count = resources.checked_grid_add(
        searched.len(),
        resources.checked_grid_add(source_length, target_length)?,
    )?;
    resources.charge_layout_work(count)?;
    let mut path = Vec::new();
    path.try_reserve_exact(count)
        .map_err(|_| layout_allocation_failed())?;
    let mut current = source_contact;
    while current != source_path {
        path.push(current);
        current = step_toward(current, source_path);
    }
    path.extend_from_slice(searched);
    current = target_path;
    while current != target_contact {
        current = step_toward(current, target_contact);
        path.push(current);
    }
    Ok(path)
}

fn step_toward((x, y): (usize, usize), (target_x, target_y): (usize, usize)) -> (usize, usize) {
    if x < target_x {
        (x + 1, y)
    } else if x > target_x {
        (x - 1, y)
    } else if y < target_y {
        (x, y + 1)
    } else if y > target_y {
        (x, y - 1)
    } else {
        (x, y)
    }
}

fn heading(side: PhysicalPortSide) -> usize {
    match side {
        PhysicalPortSide::Top => 0,
        PhysicalPortSide::Right => 1,
        PhysicalPortSide::Bottom => 2,
        PhysicalPortSide::Left => 3,
    }
}

fn travel_direction(from: (usize, usize), to: (usize, usize)) -> PhysicalPortSide {
    if from.0 < to.0 {
        PhysicalPortSide::Right
    } else if from.0 > to.0 {
        PhysicalPortSide::Left
    } else if from.1 < to.1 {
        PhysicalPortSide::Bottom
    } else {
        PhysicalPortSide::Top
    }
}

fn port_rect(
    port: LayeredRelationPhysicalPort,
    node: &PlacedRelationGraphBox<'_>,
) -> (usize, usize, usize, usize) {
    match port.side {
        PhysicalPortSide::Top => (
            port.path_x,
            port.path_y.min(port.marker_y),
            port.path_x + 1,
            node.y(),
        ),
        PhysicalPortSide::Bottom => (
            port.path_x,
            node.bottom() + 1,
            port.path_x + 1,
            port.path_y.max(port.marker_y) + 1,
        ),
        PhysicalPortSide::Left => (
            port.path_x.min(port.marker_x),
            port.path_y,
            node.x(),
            port.path_y + 1,
        ),
        PhysicalPortSide::Right => (
            node.right() + 1,
            port.path_y,
            port.path_x.max(port.marker_x) + 1,
            port.path_y + 1,
        ),
    }
}

fn port_clear(
    occupied: &[bool],
    width: usize,
    port: LayeredRelationPhysicalPort,
    node: &PlacedRelationGraphBox<'_>,
    resources: &ResourceContext,
) -> Result<bool> {
    let (left, top, right, bottom) = port_rect(port, node);
    rect_clear(occupied, width, left, top, right, bottom, resources)
}

fn reserve_port(
    occupied: &mut [bool],
    width: usize,
    port: LayeredRelationPhysicalPort,
    node: &PlacedRelationGraphBox<'_>,
    resources: &ResourceContext,
) -> Result<()> {
    let (left, top, right, bottom) = port_rect(port, node);
    reserve_rect(occupied, width, left, top, right, bottom, resources)
}

fn rect_clear(
    occupied: &[bool],
    width: usize,
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
    resources: &ResourceContext,
) -> Result<bool> {
    for y in top..bottom {
        resources.checkpoint()?;
        resources.charge_layout_work(right.saturating_sub(left))?;
        if occupied[y * width + left..y * width + right]
            .iter()
            .any(|cell| *cell)
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn reserve_rect(
    occupied: &mut [bool],
    width: usize,
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
    resources: &ResourceContext,
) -> Result<()> {
    for y in top..bottom {
        resources.checkpoint()?;
        resources.charge_layout_work(right.saturating_sub(left))?;
        occupied[y * width + left..y * width + right].fill(true);
    }
    Ok(())
}

fn filled<T: Clone>(len: usize, value: T) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(len)
        .map_err(|_| layout_allocation_failed())?;
    values.resize(len, value);
    Ok(values)
}

fn route_collision() -> LayeredRouteBatchError {
    LayeredRouteBatchError::Semantic(LayeredRelationSummaryReason::RouteCollision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::AsciiColorRole;
    use crate::options::TerminalWidthProfile;
    use crate::relation_graph::{RelationGraphBox, RelationGraphLabel};
    use crate::safe_text::{ComposedTextPlan, DeferredTextRegistry};
    use crate::{AsciiError, AsciiResourceLimitId, AsciiResourcePolicy};

    fn port(x: usize, y: usize, side: PhysicalPortSide) -> LayeredRelationPhysicalPort {
        LayeredRelationPhysicalPort {
            side,
            marker_x: x,
            marker_y: y,
            path_x: x,
            path_y: y,
        }
    }

    #[test]
    fn endpoint_label_runways_keep_class_and_er_markers_at_frame_contacts() {
        let resources = ResourceContext::new(AsciiResourcePolicy::default());
        let relation_box = RelationGraphBox::new(
            "node".to_string(),
            vec![
                "+-----+".to_string(),
                "| Box |".to_string(),
                "+-----+".to_string(),
            ],
            7,
        );
        let node = PlacedRelationGraphBox::for_test("node", &relation_box, 8, 8);
        let occupied = vec![false; 32 * 24];
        for profile in [
            LayeredRelationRouteProfile::class_with_endpoint_labels(3)
                .with_marker_widths([1, 1], [1, 2]),
            LayeredRelationRouteProfile::er(),
        ] {
            for source in [true, false] {
                let ports = ports(
                    &node,
                    source,
                    profile,
                    PhysicalPortSide::Bottom,
                    ChannelGrid {
                        width: 32,
                        height: 24,
                        occupied: &occupied,
                        retained: 0,
                    },
                    &resources,
                )
                .expect("port enumeration should fit");
                for side in PhysicalPortSide::ALL {
                    let matching = ports
                        .iter()
                        .filter(|port| port.side == side)
                        .collect::<Vec<_>>();
                    assert!(!matching.is_empty(), "every frame side should offer a port");
                    for port in matching {
                        assert!(port.fits_box(&node));
                        let horizontal =
                            matches!(side, PhysicalPortSide::Left | PhysicalPortSide::Right);
                        let width = if source {
                            profile.source_marker_widths
                        } else {
                            profile.target_marker_widths
                        }[usize::from(horizontal)];
                        let left = port.marker_x - width / 2;
                        match side {
                            PhysicalPortSide::Top => assert_eq!(port.marker_y + 1, node.y()),
                            PhysicalPortSide::Bottom => {
                                assert_eq!(port.marker_y, node.bottom() + 1)
                            }
                            PhysicalPortSide::Left => assert_eq!(left + width, node.x()),
                            PhysicalPortSide::Right => assert_eq!(left, node.right() + 1),
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn endpoint_labels_cannot_replace_their_own_stem_or_borrow_occupied_cells() {
        let resources = ResourceContext::new(AsciiResourcePolicy::default());
        let mut deferred = DeferredTextRegistry::new();
        let lines = ["1", "0..*"]
            .into_iter()
            .map(|text| {
                deferred.try_register(
                    ComposedTextPlan::try_new(&resources, 1, |push| push(text))?,
                    TerminalWidthProfile::Unicode,
                    &resources,
                )
            })
            .collect::<Result<Vec<_>>>()
            .unwrap();
        let label =
            RelationGraphLabel::try_from_lines(lines, TerminalWidthProfile::Unicode, &resources)
                .unwrap();
        let path = (2..=10).map(|y| (5, y)).collect::<Vec<_>>();
        let geometry = LayeredRelationRouteGeometry::from_channel_path(
            port(5, 2, PhysicalPortSide::Bottom),
            port(5, 10, PhysicalPortSide::Top),
            &path,
            (5, 6),
            &resources,
        )
        .unwrap();
        let (anchor, y) = geometry
            .endpoint_label_anchor(false, label.line_count(), &resources)
            .unwrap();
        let beside =
            RelationOverlay::endpoint_label(anchor, y, label.clone(), AsciiColorRole::EdgeLabel);
        let bounds = beside.bounds().unwrap();
        assert_eq!(
            (bounds.left, bounds.top, bounds.right, bounds.bottom),
            (7, 9, 11, 11)
        );
        assert!(
            labels_avoid_turns_and_terminals(std::slice::from_ref(&beside), &path, &resources)
                .unwrap()
        );

        let plan = LayeredRelationRoutePlan::new(
            geometry,
            '|',
            '-',
            super::super::draw::RelationLineChars::new(['-', '|', '.', ':'], '+'),
            vec![beside.clone()],
        );
        let mut canvas = crate::canvas::Canvas::new(14, 14);
        plan.draw_route_at(&mut canvas, &resources).unwrap();
        plan.draw_overlays_at(&mut canvas, &resources).unwrap();
        for y in [9, 10] {
            assert_eq!(canvas.get(5, y), Some('|'));
            assert_eq!(
                canvas.get_color(7, y),
                Some(crate::canvas::CanvasColor::Role(AsciiColorRole::EdgeLabel)),
                "different label widths must share the same left edge"
            );
        }

        let mut on_stem = RelationOverlay::label(5, 6, label, AsciiColorRole::EdgeLabel);
        assert!(labels_avoid_turns_and_terminals(&[on_stem.clone()], &path, &resources).unwrap());
        if let RelationOverlay::Label { endpoint, .. } = &mut on_stem {
            *endpoint = true;
        }
        assert!(!labels_avoid_turns_and_terminals(&[on_stem], &path, &resources).unwrap());

        let mut occupied = vec![false; 14 * 14];
        assert!(
            overlays_clear(std::slice::from_ref(&beside), 14, 14, &occupied, &resources).unwrap()
        );
        occupied[10 * 14 + 8] = true;
        assert!(
            !overlays_clear(std::slice::from_ref(&beside), 14, 14, &occupied, &resources).unwrap()
        );
        assert!(!overlays_clear(&[beside], 10, 14, &[false; 10 * 14], &resources).unwrap());
    }

    #[test]
    fn channel_search_keeps_the_source_and_target_normals() {
        let resources = ResourceContext::new(AsciiResourcePolicy::default());
        let occupied = vec![false; 9 * 8];
        let source = port(2, 2, PhysicalPortSide::Right);
        let target = port(6, 4, PhysicalPortSide::Bottom);
        let ChannelRoute { path, .. } = search_path(
            &[source],
            &[target],
            PhysicalPortSide::Right,
            ChannelGrid {
                width: 9,
                height: 8,
                occupied: &occupied,
                retained: 0,
            },
            &resources,
        )
        .expect("search should fit")
        .expect("a private route should exist");
        assert_eq!(&path[..2], &[(2, 2), (3, 2)]);
        assert_eq!(&path[path.len() - 2..], &[(6, 5), (6, 4)]);
        assert!(
            path.windows(2)
                .all(|pair| pair[0].0.abs_diff(pair[1].0) + pair[0].1.abs_diff(pair[1].1) == 1)
        );
        for blocked in [(3, 2), (6, 5)] {
            let mut occupied = occupied.clone();
            occupied[blocked.1 * 9 + blocked.0] = true;
            assert!(
                search_path(
                    &[source],
                    &[target],
                    PhysicalPortSide::Right,
                    ChannelGrid {
                        width: 9,
                        height: 8,
                        occupied: &occupied,
                        retained: 0
                    },
                    &resources
                )
                .expect("blocked search should fit")
                .is_none(),
                "a blocked normal must not turn at the marker"
            );
        }
    }

    #[test]
    fn channel_search_admits_all_resident_grids_and_heap_together() {
        // Occupancy + two heading arrays + one sparse target + the two-cell
        // path. The heap is released before the path is reconstructed.
        const CELLS: usize = 12;
        const RESIDENT: usize = CELLS + 2 * (4 * CELLS) + 1 + 2;
        let occupied = vec![false; CELLS];
        let source = port(1, 1, PhysicalPortSide::Right);
        let target = port(2, 1, PhysicalPortSide::Left);
        for (limit, succeeds) in [(RESIDENT, true), (RESIDENT - 1, false)] {
            let policy = AsciiResourcePolicy::unbounded()
                .with_limit(AsciiResourceLimitId::MaxGridCells, limit)
                .expect("valid limit");
            let result = search_path(
                &[source],
                &[target],
                PhysicalPortSide::Right,
                ChannelGrid {
                    width: 4,
                    height: 3,
                    occupied: &occupied,
                    retained: 0,
                },
                &ResourceContext::new(policy),
            );
            if succeeds {
                assert_eq!(
                    result
                        .expect("exact resident limit should fit")
                        .expect("adjacent route")
                        .path,
                    [(1, 1), (2, 1)]
                );
            } else {
                assert!(
                    matches!(result, Err(AsciiError::ResourceLimitExceeded(details))
                    if details.limit == AsciiResourceLimitId::MaxGridCells && details.actual == RESIDENT && details.max == limit)
                );
            }
        }
    }
}

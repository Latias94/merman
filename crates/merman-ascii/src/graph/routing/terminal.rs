use super::occupancy::RouteCandidateScore;
use super::path::{RouteObstacles, StepDirection};
use super::plan::{EdgeRouteRequest, NodeAttachment, NodeAttachments, RoutePlan};
use super::{admit_route_candidate, layout_allocation_failed};
use crate::error::Result;
use crate::graph::layout::{CanvasCoord, NodeLayout, ROUTING_GRID_MARGIN};
use crate::graph::routing::cell::{DIR_DOWN, DIR_LEFT, DIR_RIGHT, DIR_UP};
use crate::graph::routing::label::RoutedLabelDescriptor;
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use rustc_hash::FxHashMap as HashMap;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

const DIRECTIONS: [StepDirection; 4] = [
    StepDirection::Right,
    StepDirection::Down,
    StepDirection::Left,
    StepDirection::Up,
];

// This is a quality-repair effort bound across all port pairs, not an output quota.
// Resource and cancellation failures still propagate through the operation's shared ledger.
const MAX_REPAIR_EXPANSIONS: usize = 4_096;

#[derive(Clone, Copy)]
struct TerminalPort {
    attachment: NodeAttachment,
    berth: CanvasCoord,
}

/// Resolve congestion at actual terminal-cell ports, after the cheaper coarse routes have
/// been compared. The finite scene bounds and the operation ledger bound every search.
pub(super) fn plan_terminal_route(
    request: EdgeRouteRequest<'_>,
    obstacles: RouteObstacles<'_>,
    label: Option<RoutedLabelDescriptor>,
    incumbent: Option<RouteCandidateScore>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<(RouteCandidateScore, RoutePlan)>> {
    execution.checkpoint(merman_model::OperationPhase::Layout)?;
    if request.from.id == request.to.id {
        return Ok(None);
    }
    let starts = terminal_ports(request.from, request, resources)?;
    let ends = terminal_ports(request.to, request, resources)?;
    let mut pairs = Vec::new();
    pairs
        .try_reserve(resources.checked_work_mul(starts.len(), ends.len())?)
        .map_err(|_| layout_allocation_failed())?;
    for start in starts {
        for end in &ends {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            let distance = distance(start.berth, end.berth, resources)?;
            let ordinal = pairs.len();
            pairs.push((
                obstacles.scene.target_port_usage(end.attachment),
                distance,
                ordinal,
                start,
                *end,
            ));
        }
    }
    crate::graph::layout::charge_sort_work(pairs.len(), resources)?;
    pairs.sort_unstable_by_key(|(usage, distance, ordinal, _, _)| (*usage, *distance, *ordinal));
    let extent = obstacles.scene.occupied_extent();
    let mut max_x = extent.x;
    let mut max_y = extent.y;
    for node in &request.graph_layout.nodes {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        max_x = max_x.max(node.right());
        max_y = max_y.max(node.bottom());
    }
    let bounds = CanvasCoord {
        x: resources.checked_grid_add(max_x, ROUTING_GRID_MARGIN)?,
        y: resources.checked_grid_add(max_y, ROUTING_GRID_MARGIN)?,
    };
    let mut expansions_left = if incumbent.is_some() {
        MAX_REPAIR_EXPANSIONS
    } else {
        usize::MAX
    };
    for (_, _, _, start, end) in pairs {
        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        let geometry = SearchGeometry {
            start,
            end,
            bounds,
            obstacles,
        };
        let Some(path) = find_path(geometry, &mut expansions_left, resources, execution)? else {
            if expansions_left == 0 {
                break;
            }
            continue;
        };
        let hosts = super::plan::orthogonal::terminal_label_hosts(&path, label, resources)?;
        for host in hosts {
            execution.checkpoint(merman_model::OperationPhase::Layout)?;
            let Some(plan) = super::plan::orthogonal::plan_terminal_path(
                &path,
                NodeAttachments {
                    start: start.attachment,
                    end: end.attachment,
                },
                request.edge,
                host,
                request.charset,
                resources,
            )?
            else {
                continue;
            };
            let Some((score, plan)) = admit_route_candidate(request, obstacles, plan, resources)?
            else {
                continue;
            };
            if incumbent.is_none_or(|best| score < best) {
                return Ok(Some((score, plan)));
            }
        }
    }
    Ok(None)
}

fn terminal_ports(
    node: &NodeLayout,
    request: EdgeRouteRequest<'_>,
    resources: &ResourceContext,
) -> Result<Vec<TerminalPort>> {
    let mut ports = Vec::new();
    ports
        .try_reserve(12)
        .map_err(|_| layout_allocation_failed())?;
    for outward in DIRECTIONS {
        for offset in [0, -1, 1] {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            let berth = match outward {
                StepDirection::Left | StepDirection::Right => {
                    let Some(y) = node.center_y().checked_add_signed(offset) else {
                        continue;
                    };
                    if y <= node.y || y >= node.bottom() {
                        continue;
                    }
                    let x = match outward {
                        StepDirection::Left => {
                            let Some(x) = node.x.checked_sub(1) else {
                                continue;
                            };
                            x
                        }
                        _ => resources.checked_grid_add(node.right(), 1)?,
                    };
                    CanvasCoord { x, y }
                }
                StepDirection::Up | StepDirection::Down => {
                    let Some(x) = node.center_x().checked_add_signed(offset) else {
                        continue;
                    };
                    if x <= node.x || x >= node.right() {
                        continue;
                    }
                    let y = match outward {
                        StepDirection::Up => {
                            let Some(y) = node.y.checked_sub(1) else {
                                continue;
                            };
                            y
                        }
                        _ => resources.checked_grid_add(node.bottom(), 1)?,
                    };
                    CanvasCoord { x, y }
                }
            };
            if let Some(attachment) =
                NodeAttachment::resolve(node, berth, outward.opposite(), request.charset)
            {
                ports.push(TerminalPort { attachment, berth });
            }
        }
    }
    Ok(ports)
}

#[derive(Clone, Copy)]
struct SearchGeometry<'a> {
    start: TerminalPort,
    end: TerminalPort,
    bounds: CanvasCoord,
    obstacles: RouteObstacles<'a>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Position {
    coord: CanvasCoord,
    heading: StepDirection,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Cost {
    total: usize,
    shared: usize,
    length: usize,
    bends: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct OpenEntry {
    position: Position,
    cost: Cost,
    priority: Cost,
    sequence: usize,
}

impl Ord for OpenEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .priority
            .cmp(&self.priority)
            .then_with(|| other.sequence.cmp(&self.sequence))
    }
}

impl PartialOrd for OpenEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn find_path(
    geometry: SearchGeometry<'_>,
    expansions_left: &mut usize,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<Vec<CanvasCoord>>> {
    let attachments = Some(NodeAttachments {
        start: geometry.start.attachment,
        end: geometry.end.attachment,
    });
    for port in [geometry.start, geometry.end] {
        if !geometry.obstacles.scene.route_cell_is_clear(
            geometry.obstacles.routes,
            geometry.obstacles.owner,
            attachments,
            port.berth,
            axis_mask(port.attachment.outward),
            resources,
        )? {
            return Ok(None);
        }
    }
    let initial = Position {
        coord: geometry.start.berth,
        heading: geometry.start.attachment.outward,
    };
    let zero = Cost {
        total: 0,
        shared: 0,
        length: 0,
        bends: 0,
    };
    let mut open = BinaryHeap::new();
    let mut costs = HashMap::default();
    let mut parents = HashMap::<Position, Position>::default();
    costs
        .try_reserve(1)
        .map_err(|_| layout_allocation_failed())?;
    costs.insert(initial, zero);
    open.try_reserve(1)
        .map_err(|_| layout_allocation_failed())?;
    open.push(OpenEntry {
        position: initial,
        cost: zero,
        priority: Cost {
            total: distance(initial.coord, geometry.end.berth, resources)?,
            shared: 0,
            length: distance(initial.coord, geometry.end.berth, resources)?,
            bends: 0,
        },
        sequence: 0,
    });
    let mut sequence = 0;
    while let Some(entry) = open.pop() {
        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        if *expansions_left == 0 {
            return Ok(None);
        }
        *expansions_left -= 1;
        resources.charge_layout_work(1)?;
        if costs
            .get(&entry.position)
            .is_some_and(|cost| entry.cost > *cost)
        {
            continue;
        }
        if entry.position.coord == geometry.end.berth
            && entry.position.heading == geometry.end.attachment.outward.opposite()
        {
            let mut path = Vec::new();
            path.try_reserve(resources.checked_work_add(entry.cost.length, 1)?)
                .map_err(|_| layout_allocation_failed())?;
            let mut cursor = entry.position;
            path.push(cursor.coord);
            while let Some(previous) = parents.get(&cursor).copied() {
                resources.checkpoint()?;
                resources.charge_layout_work(1)?;
                path.push(previous.coord);
                cursor = previous;
            }
            path.reverse();
            return Ok(Some(path));
        }
        for heading in DIRECTIONS {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            if heading == entry.position.heading.opposite()
                || (entry.position == initial && heading != initial.heading)
            {
                continue;
            }
            let Some(next) = neighbor(entry.position.coord, heading, geometry.bounds) else {
                continue;
            };
            if next == geometry.end.berth && heading != geometry.end.attachment.outward.opposite() {
                continue;
            }
            let corner_mask =
                direction_mask(entry.position.heading.opposite()) | direction_mask(heading);
            if !geometry.obstacles.scene.route_cell_is_clear(
                geometry.obstacles.routes,
                geometry.obstacles.owner,
                attachments,
                entry.position.coord,
                corner_mask,
                resources,
            )? {
                continue;
            }
            if !geometry.obstacles.scene.route_cell_is_clear(
                geometry.obstacles.routes,
                geometry.obstacles.owner,
                attachments,
                next,
                axis_mask(heading),
                resources,
            )? {
                continue;
            }
            let position = Position {
                coord: next,
                heading,
            };
            let congestion = geometry.obstacles.scene.route_cell_congestion(
                geometry.obstacles.routes,
                geometry.obstacles.owner,
                attachments,
                next,
                resources,
            )?;
            let shared = resources.checked_work_add(entry.cost.shared, congestion)?;
            let length = resources.checked_work_add(entry.cost.length, 1)?;
            let cost = Cost {
                total: resources.checked_work_add(length, shared)?,
                shared,
                length,
                bends: resources.checked_work_add(
                    entry.cost.bends,
                    usize::from(heading != entry.position.heading),
                )?,
            };
            if costs.get(&position).is_some_and(|current| *current <= cost) {
                continue;
            }
            if !costs.contains_key(&position) {
                costs
                    .try_reserve(1)
                    .map_err(|_| layout_allocation_failed())?;
                parents
                    .try_reserve(1)
                    .map_err(|_| layout_allocation_failed())?;
            }
            costs.insert(position, cost);
            parents.insert(position, entry.position);
            sequence = resources.checked_work_add(sequence, 1)?;
            open.try_reserve(1)
                .map_err(|_| layout_allocation_failed())?;
            open.push(OpenEntry {
                position,
                cost,
                sequence,
                priority: Cost {
                    total: resources.checked_work_add(
                        cost.total,
                        distance(next, geometry.end.berth, resources)?,
                    )?,
                    shared: cost.shared,
                    length: resources.checked_work_add(
                        cost.length,
                        distance(next, geometry.end.berth, resources)?,
                    )?,
                    bends: cost.bends,
                },
            });
        }
    }
    Ok(None)
}

fn neighbor(
    coord: CanvasCoord,
    heading: StepDirection,
    bounds: CanvasCoord,
) -> Option<CanvasCoord> {
    match heading {
        StepDirection::Left => coord.x.checked_sub(1).map(|x| CanvasCoord { x, ..coord }),
        StepDirection::Up => coord.y.checked_sub(1).map(|y| CanvasCoord { y, ..coord }),
        StepDirection::Right if coord.x < bounds.x => Some(CanvasCoord {
            x: coord.x + 1,
            ..coord
        }),
        StepDirection::Down if coord.y < bounds.y => Some(CanvasCoord {
            y: coord.y + 1,
            ..coord
        }),
        _ => None,
    }
}

fn axis_mask(heading: StepDirection) -> u8 {
    match heading {
        StepDirection::Left | StepDirection::Right => DIR_LEFT | DIR_RIGHT,
        StepDirection::Up | StepDirection::Down => DIR_UP | DIR_DOWN,
    }
}

fn direction_mask(heading: StepDirection) -> u8 {
    match heading {
        StepDirection::Left => DIR_LEFT,
        StepDirection::Right => DIR_RIGHT,
        StepDirection::Up => DIR_UP,
        StepDirection::Down => DIR_DOWN,
    }
}

fn distance(from: CanvasCoord, to: CanvasCoord, resources: &ResourceContext) -> Result<usize> {
    resources.checked_work_add(from.x.abs_diff(to.x), from.y.abs_diff(to.y))
}

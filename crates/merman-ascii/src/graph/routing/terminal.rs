use super::geometry::{CompoundRouteGeometry, FrameSegment};
use super::occupancy::RouteCandidateScore;
use super::path::{RouteObstacles, StepDirection};
use super::plan::{EdgeRouteRequest, EndpointGeometry, NodeAttachment, NodeAttachments, RoutePlan};
use super::{admit_route_candidate, layout_allocation_failed};
use crate::error::Result;
use crate::graph::layout::{CanvasCoord, ROUTING_GRID_MARGIN};
use crate::graph::model::{GraphEdgeMarker, GraphGroupKind, GraphNodeShape};
use crate::graph::routing::cell::{DIR_DOWN, DIR_LEFT, DIR_RIGHT, DIR_UP};
use crate::graph::routing::label::RoutedLabelDescriptor;
use crate::operation::{AsciiExecution, SearchAllowance};
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

// One scene shares this quality-repair bound across all its edges and port pairs.
// Resource and cancellation failures still propagate through the operation's shared ledger.
pub(super) const MAX_REPAIR_EXPANSIONS: usize = 4_096;

pub(super) enum SearchBudget<'a> {
    #[cfg(test)]
    Required,
    Optional(&'a mut usize),
    Limited(&'a SearchAllowance),
    Replacement(&'a SearchAllowance),
}

mod channels;
mod direct;
mod loops;
pub(super) use direct::direct_path;
pub(super) use direct::plan_direct_route;
pub(super) use loops::plan_self_loop_candidates;

#[derive(Clone, Copy)]
struct TerminalPort {
    attachment: NodeAttachment,
    berth: CanvasCoord,
    usage: usize,
    ordinal: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct PortPairEntry {
    priority: (usize, bool, usize, usize, usize),
    end_index: usize,
    end_limit: usize,
    increasing: bool,
}

impl Ord for PortPairEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other.priority.cmp(&self.priority)
    }
}

impl PartialOrd for PortPairEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Within a usage tier and one endpoint side, berth coordinates lie on a straight line.
// Manhattan distance increases away from the nearest intercept in either direction. Merge
// those two streams per source, retaining at most eight entries per source instead of the
// Cartesian product. Usage tiers and original ordinals preserve exhaustive-sort ordering.
struct PortPairs {
    starts: Vec<TerminalPort>,
    ends: Vec<TerminalPort>,
    frontier: BinaryHeap<PortPairEntry>,
    next_usage_group: usize,
    preferred: Option<StepDirection>,
    allow_fallback: bool,
}

impl PortPairs {
    fn new(
        starts: Vec<TerminalPort>,
        mut ends: Vec<TerminalPort>,
        resources: &ResourceContext,
    ) -> Result<Self> {
        crate::graph::layout::charge_sort_work(ends.len(), resources)?;
        ends.sort_unstable_by_key(|port| {
            (
                port.usage,
                direction_mask(port.attachment.outward),
                port_intercept(*port),
            )
        });
        Ok(Self {
            starts,
            ends,
            frontier: BinaryHeap::new(),
            next_usage_group: 0,
            preferred: None,
            allow_fallback: true,
        })
    }

    fn prefer_forward(&mut self, forward: Option<StepDirection>, allow_fallback: bool) {
        self.preferred = forward;
        self.allow_fallback = allow_fallback;
    }

    fn next(
        &mut self,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Option<(TerminalPort, TerminalPort)>> {
        if self.starts.is_empty() || self.ends.is_empty() {
            return Ok(None);
        }
        while self.frontier.is_empty() {
            if self.next_usage_group == self.ends.len() {
                return Ok(None);
            }
            let usage = self.ends[self.next_usage_group].usage;
            while self.next_usage_group < self.ends.len()
                && self.ends[self.next_usage_group].usage == usage
            {
                let first = self.next_usage_group;
                let outward = self.ends[first].attachment.outward;
                while self.next_usage_group < self.ends.len()
                    && self.ends[self.next_usage_group].usage == usage
                    && self.ends[self.next_usage_group].attachment.outward == outward
                {
                    execution.checkpoint(merman_model::OperationPhase::Layout)?;
                    resources.charge_layout_work(1)?;
                    self.next_usage_group += 1;
                }
                let last = self.next_usage_group;
                for start_index in 0..self.starts.len() {
                    let start = self.starts[start_index];
                    if !self.allow_fallback
                        && let Some(forward) = self.preferred
                    {
                        resources.checkpoint()?;
                        resources.charge_layout_work(1)?;
                        let preferred_pair =
                            start.attachment.outward == forward && outward == forward.opposite();
                        if !preferred_pair {
                            continue;
                        }
                    }
                    let intercept = match outward {
                        StepDirection::Left | StepDirection::Right => start.berth.y,
                        StepDirection::Up | StepDirection::Down => start.berth.x,
                    };
                    let mut lower = first;
                    let mut upper = last;
                    while lower < upper {
                        execution.checkpoint(merman_model::OperationPhase::Layout)?;
                        resources.charge_layout_work(1)?;
                        let middle = lower + (upper - lower) / 2;
                        if port_intercept(self.ends[middle]) < intercept {
                            lower = middle + 1;
                        } else {
                            upper = middle;
                        }
                    }
                    if lower > first {
                        self.push(start_index, lower - 1, first, false, resources)?;
                    }
                    if lower < last {
                        self.push(start_index, lower, last, true, resources)?;
                    }
                }
            }
        }
        let Some(entry) = self.frontier.pop() else {
            return Ok(None);
        };
        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        let start_index = entry.priority.3;
        let next = if entry.increasing {
            entry
                .end_index
                .checked_add(1)
                .filter(|next| *next < entry.end_limit)
        } else {
            entry
                .end_index
                .checked_sub(1)
                .filter(|next| *next >= entry.end_limit)
        };
        if let Some(next) = next {
            self.push(
                start_index,
                next,
                entry.end_limit,
                entry.increasing,
                resources,
            )?;
        }
        Ok(Some((self.starts[start_index], self.ends[entry.end_index])))
    }

    fn push(
        &mut self,
        start_index: usize,
        end_index: usize,
        end_limit: usize,
        increasing: bool,
        resources: &ResourceContext,
    ) -> Result<()> {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        let start = self.starts[start_index];
        let end = self.ends[end_index];
        let entry = PortPairEntry {
            priority: (
                start.usage,
                self.preferred.is_some_and(|forward| {
                    start.attachment.outward != forward
                        || end.attachment.outward != forward.opposite()
                }),
                distance(start.berth, end.berth, resources)?,
                start_index,
                end.ordinal,
            ),
            end_index,
            end_limit,
            increasing,
        };
        self.frontier
            .try_reserve(1)
            .map_err(|_| layout_allocation_failed())?;
        self.frontier.push(entry);
        Ok(())
    }
}

fn port_intercept(port: TerminalPort) -> usize {
    match port.attachment.outward {
        StepDirection::Left | StepDirection::Right => port.berth.y,
        StepDirection::Up | StepDirection::Down => port.berth.x,
    }
}

/// Resolve congestion at actual terminal-cell ports after finite channels have been compared.
/// The finite scene bounds and the operation ledger bound every search.
#[cfg(test)]
pub(super) fn plan_terminal_route(
    request: EdgeRouteRequest<'_>,
    obstacles: RouteObstacles<'_>,
    label: Option<RoutedLabelDescriptor>,
    incumbent: Option<RouteCandidateScore>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<(RouteCandidateScore, RoutePlan)>> {
    let mut repair_expansions = MAX_REPAIR_EXPANSIONS;
    plan_terminal_route_with_budget(
        request,
        obstacles,
        label,
        incumbent.map(|score| (score, None)),
        if incumbent.is_some() {
            SearchBudget::Optional(&mut repair_expansions)
        } else {
            SearchBudget::Required
        },
        resources,
        execution,
    )
}

pub(super) fn plan_terminal_route_with_budget(
    request: EdgeRouteRequest<'_>,
    obstacles: RouteObstacles<'_>,
    label: Option<RoutedLabelDescriptor>,
    incumbent: Option<(RouteCandidateScore, Option<NodeAttachments>)>,
    budget: SearchBudget<'_>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<(RouteCandidateScore, RoutePlan)>> {
    let initial_expansions = match &budget {
        #[cfg(test)]
        SearchBudget::Required => usize::MAX,
        SearchBudget::Optional(remaining) => **remaining,
        SearchBudget::Limited(allowance) | SearchBudget::Replacement(allowance) => {
            allowance.remaining().0
        }
    };
    let work_start = resources.layout_work_used();
    let work_allowance = match &budget {
        SearchBudget::Limited(allowance) | SearchBudget::Replacement(allowance) => {
            allowance.remaining().1
        }
        _ => usize::MAX,
    };
    let share_pairs = matches!(&budget, SearchBudget::Replacement(_)) && incumbent.is_none();
    let mut expansions_left = initial_expansions;
    let result = plan_terminal_route_inner(
        request,
        obstacles,
        label,
        incumbent,
        &mut expansions_left,
        share_pairs,
        work_start.saturating_add(work_allowance),
        resources,
        execution,
    );
    match budget {
        SearchBudget::Limited(allowance) | SearchBudget::Replacement(allowance) => allowance.debit(
            initial_expansions - expansions_left,
            resources.layout_work_used().saturating_sub(work_start),
        ),
        SearchBudget::Optional(remaining) => *remaining = expansions_left,
        #[cfg(test)]
        SearchBudget::Required => {}
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn plan_terminal_route_inner(
    request: EdgeRouteRequest<'_>,
    obstacles: RouteObstacles<'_>,
    label: Option<RoutedLabelDescriptor>,
    incumbent: Option<(RouteCandidateScore, Option<NodeAttachments>)>,
    expansions_left: &mut usize,
    share_pairs: bool,
    work_end: usize,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<(RouteCandidateScore, RoutePlan)>> {
    execution.checkpoint(merman_model::OperationPhase::Layout)?;
    if *expansions_left == 0 || resources.layout_work_used() >= work_end {
        return Ok(None);
    }
    let forward = super::forward_route_direction(request, resources)?;
    let incumbent_is_preferred = forward.is_some()
        && incumbent
            .is_some_and(|(_, attachments)| super::contacts_follow_direction(forward, attachments));
    let allow_fallback =
        incumbent.is_none_or(|(score, _)| !incumbent_is_preferred || score.port_reuse() != (0, 0));
    for group in &request.graph_layout.groups {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        if (group.id == request.from.id || group.id == request.to.id)
            && group.kind != GraphGroupKind::Container
        {
            return Ok(None);
        }
    }
    let from =
        EndpointGeometry::for_route(request.from, request.to, request.graph_layout, resources)?;
    let to =
        EndpointGeometry::for_route(request.to, request.from, request.graph_layout, resources)?;
    let self_loop = request.from.id == request.to.id;
    let mut starts = terminal_ports(from, request, resources, execution)?;
    let mut ends = terminal_ports(to, request, resources, execution)?;
    for start in &mut starts {
        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        if request.edge.start_marker != GraphEdgeMarker::Open {
            start.usage = obstacles.scene.source_port_usage(start.attachment);
        }
    }
    for end in &mut ends {
        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        end.usage = obstacles.scene.target_port_usage(end.attachment);
    }
    let extent = obstacles.scene.occupied_extent();
    let mut max_x = extent.x;
    let mut max_y = extent.y;
    for node in &request.graph_layout.nodes {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        max_x = max_x.max(node.right());
        max_y = max_y.max(node.bottom());
    }
    for group in &request.graph_layout.groups {
        resources.checkpoint()?;
        resources.charge_layout_work(1)?;
        max_x = max_x.max(group.right());
        max_y = max_y.max(group.bottom());
    }
    let bounds = CanvasCoord {
        x: resources.checked_grid_add(max_x, ROUTING_GRID_MARGIN)?,
        y: resources.checked_grid_add(max_y, ROUTING_GRID_MARGIN)?,
    };
    let channels =
        channels::ChannelAxes::new(obstacles, &starts, &ends, bounds, resources, execution)?;
    let mut remaining_pairs = resources.checked_work_mul(starts.len(), ends.len())?;
    let mut pairs = PortPairs::new(starts, ends, resources)?;
    pairs.prefer_forward(forward, allow_fallback);
    while *expansions_left != 0 && resources.layout_work_used() < work_end {
        let Some((start, end)) = pairs.next(resources, execution)? else {
            break;
        };
        let pair_allowance = if share_pairs {
            (*expansions_left).div_ceil(remaining_pairs.max(1))
        } else {
            *expansions_left
        };
        remaining_pairs = remaining_pairs.saturating_sub(1);
        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        if self_loop && start.attachment.contact == end.attachment.contact {
            continue;
        }
        let geometry = SearchGeometry {
            start,
            end,
            bounds,
            obstacles,
            groups: CompoundRouteGeometry {
                layout: request.graph_layout,
                from: request.from,
                to: request.to,
                attachments: Some(NodeAttachments {
                    start: start.attachment,
                    end: end.attachment,
                }),
            },
        };
        let mut pair_remaining = pair_allowance;
        let path = find_path_with_allowance(
            geometry,
            &channels,
            &mut pair_remaining,
            work_end,
            resources,
            execution,
        );
        *expansions_left -= pair_allowance - pair_remaining;
        let path = path?;
        let Some(path) = path else {
            if *expansions_left == 0 {
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
            if incumbent.is_none_or(|incumbent| {
                super::route_candidate_is_better(forward, (score, plan.attachments), incumbent)
            }) {
                return Ok(Some((score, plan)));
            }
        }
    }
    Ok(None)
}

fn terminal_ports(
    geometry: EndpointGeometry<'_>,
    request: EdgeRouteRequest<'_>,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<TerminalPort>> {
    let node = geometry.layout;
    let mut ports = Vec::new();
    for outward in DIRECTIONS {
        let (first, center, last) = match outward {
            StepDirection::Left | StepDirection::Right => (node.y, node.center_y(), node.bottom()),
            StepDirection::Up | StepDirection::Down => (node.x, node.center_x(), node.right()),
        };
        for offset in 0..=center.abs_diff(first).max(last.abs_diff(center)) {
            let contacts = [
                center.checked_sub(offset),
                (offset != 0).then(|| center.checked_add(offset)).flatten(),
            ];
            for intercept in contacts.into_iter().flatten() {
                execution.checkpoint(merman_model::OperationPhase::Layout)?;
                resources.charge_layout_work(1)?;
                if intercept < first
                    || intercept > last
                    || (node.shape != GraphNodeShape::Text
                        && (intercept == first || intercept == last))
                {
                    continue;
                }
                let berth = match outward {
                    StepDirection::Left | StepDirection::Right => {
                        let y = intercept;
                        let x = match outward {
                            StepDirection::Left => {
                                let Some(x) = (if geometry.interior {
                                    node.x.checked_add(1)
                                } else {
                                    node.x.checked_sub(1)
                                }) else {
                                    continue;
                                };
                                x
                            }
                            _ if geometry.interior => node.right() - 1,
                            _ => resources.checked_grid_add(node.right(), 1)?,
                        };
                        CanvasCoord { x, y }
                    }
                    StepDirection::Up | StepDirection::Down => {
                        let x = intercept;
                        let y = match outward {
                            StepDirection::Up => {
                                let Some(y) = (if geometry.interior {
                                    node.y.checked_add(1)
                                } else {
                                    node.y.checked_sub(1)
                                }) else {
                                    continue;
                                };
                                y
                            }
                            _ if geometry.interior => node.bottom() - 1,
                            _ => resources.checked_grid_add(node.bottom(), 1)?,
                        };
                        CanvasCoord { x, y }
                    }
                };
                let departure = if geometry.interior {
                    outward.opposite()
                } else {
                    outward
                };
                if let Some(attachment) =
                    geometry.resolve(berth, departure.opposite(), request.charset)
                {
                    ports
                        .try_reserve(1)
                        .map_err(|_| layout_allocation_failed())?;
                    ports.push(TerminalPort {
                        attachment,
                        berth,
                        usage: 0,
                        ordinal: ports.len(),
                    });
                }
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
    groups: CompoundRouteGeometry<'a>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Position {
    coord: CanvasCoord,
    heading: StepDirection,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ChannelPosition {
    point: channels::ChannelPoint,
    heading: StepDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Cost {
    total: usize,
    shared: usize,
    length: usize,
    bends: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct OpenEntry {
    position: ChannelPosition,
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

#[cfg(test)]
fn find_path(
    geometry: SearchGeometry<'_>,
    channels: &channels::ChannelAxes,
    expansions_left: &mut usize,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<Vec<CanvasCoord>>> {
    find_path_with_allowance(
        geometry,
        channels,
        expansions_left,
        usize::MAX,
        resources,
        execution,
    )
}

fn find_path_with_allowance(
    geometry: SearchGeometry<'_>,
    channels: &channels::ChannelAxes,
    expansions_left: &mut usize,
    work_end: usize,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<Vec<CanvasCoord>>> {
    let attachments = Some(NodeAttachments {
        start: geometry.start.attachment,
        end: geometry.end.attachment,
    });
    for port in [geometry.start, geometry.end] {
        if geometry
            .groups
            .segment(port.attachment.contact, port.berth, resources, execution)?
            == FrameSegment::Blocked
            || !geometry.obstacles.scene.route_cell_is_clear(
                geometry.obstacles.routes,
                geometry.obstacles.owner,
                attachments,
                port.berth,
                axis_mask(port.attachment.outward),
                resources,
            )?
        {
            return Ok(None);
        }
        if geometry.start.berth != geometry.end.berth
            && !terminal_corridor_is_open(geometry, port, resources, execution)?
        {
            return Ok(None);
        }
    }
    let Some(point) = channels.locate(geometry.start.berth, resources, execution)? else {
        return Ok(None);
    };
    let initial = ChannelPosition {
        point,
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
    let mut parents = HashMap::<ChannelPosition, ChannelPosition>::default();
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
            total: distance(geometry.start.berth, geometry.end.berth, resources)?,
            shared: 0,
            length: distance(geometry.start.berth, geometry.end.berth, resources)?,
            bends: 0,
        },
        sequence: 0,
    });
    let mut sequence = 0;
    while let Some(entry) = open.pop() {
        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        if costs
            .get(&entry.position)
            .is_some_and(|cost| entry.cost > *cost)
        {
            continue;
        }
        if *expansions_left == 0 || resources.layout_work_used() >= work_end {
            return Ok(None);
        }
        *expansions_left -= 1;
        let Some(coord) = channels.coord(entry.position.point) else {
            return Ok(None);
        };
        if coord == geometry.end.berth
            && entry.position.heading == geometry.end.attachment.outward.opposite()
        {
            return reconstruct_path(entry, &parents, channels, resources, execution);
        }
        for heading in DIRECTIONS {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            if heading == entry.position.heading.opposite()
                || (entry.position == initial && heading != initial.heading)
            {
                continue;
            }
            let Some(next_point) =
                channels.neighbor(entry.position.point, heading, resources, execution)?
            else {
                continue;
            };
            let Some(next) = channels.coord(next_point) else {
                return Ok(None);
            };
            if next == geometry.end.berth && heading != geometry.end.attachment.outward.opposite() {
                continue;
            }
            let Some(segment) = channels::segment_cost(
                geometry,
                Position {
                    coord,
                    heading: entry.position.heading,
                },
                next,
                heading,
                resources,
                execution,
            )?
            else {
                continue;
            };
            let position = ChannelPosition {
                point: next_point,
                heading,
            };
            let shared = resources.checked_work_add(entry.cost.shared, segment.shared)?;
            let length = resources.checked_work_add(entry.cost.length, segment.length)?;
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

fn reconstruct_path(
    winner: OpenEntry,
    parents: &HashMap<ChannelPosition, ChannelPosition>,
    channels: &channels::ChannelAxes,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<Vec<CanvasCoord>>> {
    execution.checkpoint(merman_model::OperationPhase::Layout)?;
    let mut path = Vec::new();
    path.try_reserve(resources.checked_work_add(winner.cost.length, 1)?)
        .map_err(|_| layout_allocation_failed())?;
    let mut cursor = winner.position;
    let Some(mut coord) = channels.coord(cursor.point) else {
        return Ok(None);
    };
    path.push(coord);
    while let Some(previous) = parents.get(&cursor).copied() {
        let Some(previous_coord) = channels.coord(previous.point) else {
            return Ok(None);
        };
        while coord != previous_coord {
            execution.checkpoint(merman_model::OperationPhase::Layout)?;
            resources.charge_layout_work(1)?;
            // Parent links join adjacent critical coordinates on one axis. Expand only
            // this winning chain; the frontier never stores intervening terminal cells.
            if coord.x < previous_coord.x {
                coord.x += 1;
            } else if coord.x > previous_coord.x {
                coord.x -= 1;
            } else if coord.y < previous_coord.y {
                coord.y += 1;
            } else {
                coord.y -= 1;
            }
            path.try_reserve(1)
                .map_err(|_| layout_allocation_failed())?;
            path.push(coord);
        }
        cursor = previous;
    }
    for index in 0..path.len() / 2 {
        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        let opposite = path.len() - 1 - index;
        path.swap(index, opposite);
    }
    Ok(Some(path))
}

// Unit traversal is used only for semantic inspection and forced-port preflight,
// never to generate search states.
fn unit_neighbor(
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

fn terminal_corridor_is_open(
    geometry: SearchGeometry<'_>,
    port: TerminalPort,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<bool> {
    let attachments = Some(NodeAttachments {
        start: geometry.start.attachment,
        end: geometry.end.attachment,
    });
    let outward = port.attachment.outward;
    let mut previous = port.berth;
    loop {
        execution.checkpoint(merman_model::OperationPhase::Layout)?;
        resources.charge_layout_work(1)?;
        let Some(transit) = unit_neighbor(previous, outward, geometry.bounds) else {
            return Ok(false);
        };
        if geometry
            .groups
            .segment(previous, transit, resources, execution)?
            == FrameSegment::Blocked
            || !geometry.obstacles.scene.route_cell_is_clear(
                geometry.obstacles.routes,
                geometry.obstacles.owner,
                attachments,
                transit,
                axis_mask(outward),
                resources,
            )?
        {
            return Ok(false);
        }
        if transit == geometry.start.berth || transit == geometry.end.berth {
            return Ok(true);
        }
        // A frame crossing requires a straight run. Continue that forced corridor until
        // a legal turn is possible, rather than searching the scene for a sealed approach.
        for heading in DIRECTIONS {
            if heading == outward || heading == outward.opposite() {
                continue;
            }
            let Some(next) = unit_neighbor(transit, heading, geometry.bounds) else {
                continue;
            };
            let mask = direction_mask(outward.opposite()) | direction_mask(heading);
            if geometry
                .groups
                .segment(transit, next, resources, execution)?
                != FrameSegment::Blocked
                && geometry.obstacles.scene.route_cell_is_clear(
                    geometry.obstacles.routes,
                    geometry.obstacles.owner,
                    attachments,
                    transit,
                    mask,
                    resources,
                )?
                && geometry.obstacles.scene.route_cell_is_clear(
                    geometry.obstacles.routes,
                    geometry.obstacles.owner,
                    attachments,
                    next,
                    axis_mask(heading),
                    resources,
                )?
            {
                return Ok(true);
            }
        }
        previous = transit;
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

#[cfg(test)]
mod tests;

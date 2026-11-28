use super::super::super::routing::{GridRoutingView, map_grid_corridor};
use super::super::super::sparse_search::{
    GridSearchOptions, GridSearchQuery, GridSearchTerminal, ValidatedGridSearchTopology,
    find_grid_sparse_route,
};
use super::super::super::sparse_topology::{
    GridOverlayInput, GridRoutingTopology, GridTopologyLimit,
};
use super::super::constraints::{Constraints, RouteGeometry, attach_ports};
use super::super::{
    AttemptError, GridSegmentCaps, GridSegmentLimit, GridSegmentTopology, GridSegmentWorkspace,
    invalid, reserved,
};
use super::attachments::Terminal;
use super::{
    GridHierarchyError, GridHierarchyOccupancy, GridHierarchySegmentKind, GridPortalSelection,
};
use crate::resources::OperationWorkMeter;
use kurbo::Point;
use merman_model::OperationPhase;

pub(super) struct Input<'a, 'view, 'id> {
    pub container: usize,
    pub view: GridRoutingView<'view>,
    pub geometry: RouteGeometry<'view>,
    pub terminals: [Terminal; 2],
    pub topology: Option<GridSegmentTopology<'view, 'id>>,
    pub variant: usize,
    pub allow_shared_pair_corridors: bool,
    pub pair_routes: &'a [&'a [Point]],
    pub occupancy: Option<GridHierarchyOccupancy<'a>>,
}

pub(super) struct SegmentRoute {
    pub points: Vec<Point>,
    pub kind: GridHierarchySegmentKind,
    pub selections: [Option<GridPortalSelection>; 2],
}

impl SegmentRoute {
    fn selected(
        points: Vec<Point>,
        kind: GridHierarchySegmentKind,
        [start, end]: [Terminal; 2],
    ) -> Self {
        Self {
            points,
            kind,
            selections: [start.selection, end.selection],
        }
    }

    fn compatibility(points: Vec<Point>, kind: GridHierarchySegmentKind) -> Self {
        Self {
            points,
            kind,
            selections: [None; 2],
        }
    }
}

pub(super) fn route(
    input: Input<'_, '_, '_>,
    caps: GridSegmentCaps,
    workspace: &mut GridSegmentWorkspace,
    work: &OperationWorkMeter,
    alternatives: impl FnOnce() -> crate::Result<[Vec<Terminal>; 2]>,
) -> Result<SegmentRoute, GridHierarchyError> {
    work.checkpoint(OperationPhase::Layout)?;
    let [start, end] = input.terminals;
    let container = input.container;
    let sparse = input.topology.is_some();
    let constraints = Constraints::from_routes(
        input.geometry,
        sparse.then_some(input.pair_routes),
        input
            .occupancy
            .filter(|_| sparse)
            .map(|occupied| occupied.foreign_routes),
        work,
    )?;
    let legacy = || {
        map_grid_corridor(
            input.view,
            start.corridor(),
            end.corridor(),
            input.variant,
            work,
        )
    };
    let Some(topology) = input.topology else {
        let points = legacy()?;
        let valid = constraints.clears_geometry(&points, work)?;
        work.checkpoint(OperationPhase::Layout)?;
        return if valid {
            Ok(SegmentRoute::compatibility(
                points,
                GridHierarchySegmentKind::Compatibility,
            ))
        } else {
            Err(GridHierarchyError::InvalidCompatibility { container })
        };
    };

    let fallback = |limit| -> Result<_, GridHierarchyError> {
        let points = legacy()?;
        let valid = constraints.validates(&points, false, work)?;
        work.checkpoint(OperationPhase::Layout)?;
        if valid {
            Ok(SegmentRoute::compatibility(
                points,
                GridHierarchySegmentKind::ResourceFallback(limit),
            ))
        } else {
            Err(GridHierarchyError::InvalidResourceFallback { container, limit })
        }
    };
    let base = match topology {
        GridSegmentTopology::Sparse(base) => base,
        GridSegmentTopology::Compatibility(limit) => return fallback(limit.into()),
    };
    let mut attempts = reserved(16, work)?;
    attempts.push(input.terminals);
    let mut alternatives = Some(alternatives);
    let pass_order: &[bool] = if input
        .occupancy
        .is_some_and(|occupied| occupied.indexed_segments)
    {
        &[true, false]
    } else {
        &[false]
    };
    for &avoid_occupied in pass_order {
        let mut index = 0;
        while let Some(&attempt) = attempts.get(index) {
            work.charge(1)?;
            let [a, b] = attempt;
            if a.connect.x == b.connect.x || a.connect.y == b.connect.y {
                let points = attach_ports(a.port, &[a.connect, b.connect], b.port, work)?;
                if constraints.validates(&points, avoid_occupied, work)? {
                    work.checkpoint(OperationPhase::Layout)?;
                    return Ok(SegmentRoute::selected(
                        points,
                        GridHierarchySegmentKind::Direct,
                        attempt,
                    ));
                }
            }
            match search_pair(
                SearchInput {
                    base,
                    endpoints: attempt,
                    lengths: [a.stub_length(), b.stub_length()],
                    lane: None,
                    preflight_overlay: true,
                    avoid_occupied,
                },
                &constraints,
                caps,
                workspace,
                work,
            ) {
                Ok(OverlayPath::Route(points)) => {
                    if constraints.validates(&points, avoid_occupied, work)? {
                        work.checkpoint(OperationPhase::Layout)?;
                        return Ok(SegmentRoute::selected(
                            points,
                            GridHierarchySegmentKind::Sparse,
                            attempt,
                        ));
                    }
                }
                Ok(OverlayPath::MissingEndpoint | OverlayPath::NoRoute) => {}
                Err(AttemptError::Layout(error)) => return Err(error.into()),
                Err(AttemptError::Limit(limit)) => return fallback(limit),
            }
            if index == 0
                && let Some(alternatives) = alternatives.take()
            {
                let [starts, ends] = alternatives()?;
                // The start and end owners do not change. Only the accepted alternate can
                // publish its coordinate, and the caller stages that update after validation.
                for &other in &ends {
                    work.charge(1)?;
                    attempts.push([start, other]);
                }
                for &other in &starts {
                    work.charge(1)?;
                    attempts.push([other, end]);
                }
                for &a in &starts {
                    for &b in &ends {
                        work.charge(1)?;
                        attempts.push([a, b]);
                    }
                }
            }
            index += 1;
        }
    }

    let points = legacy()?;
    let clear = constraints.clears_geometry(&points, work)?;
    let valid = clear
        && (constraints.respects_pair(&points, false, work)?
            || (input.allow_shared_pair_corridors
                && constraints.respects_pair(&points, true, work)?));
    work.checkpoint(OperationPhase::Layout)?;
    if valid {
        Ok(SegmentRoute::compatibility(
            points,
            GridHierarchySegmentKind::CompatibilityRecovery,
        ))
    } else {
        Err(GridHierarchyError::NoHierarchyRoute { container })
    }
}

pub(super) struct SearchInput<'base, 'id> {
    pub base: &'base GridRoutingTopology<'id>,
    pub endpoints: [Terminal; 2],
    pub lengths: [f64; 2],
    pub lane: Option<Point>,
    /// Hierarchy checks live topology bytes before search. Loops pass them into search admission.
    pub preflight_overlay: bool,
    pub avoid_occupied: bool,
}

pub(super) enum OverlayPath {
    MissingEndpoint,
    NoRoute,
    Route(Vec<Point>),
}

pub(super) fn search_pair(
    input: SearchInput<'_, '_>,
    constraints: &Constraints<'_>,
    caps: GridSegmentCaps,
    workspace: &mut GridSegmentWorkspace,
    work: &OperationWorkMeter,
) -> Result<OverlayPath, AttemptError> {
    work.checkpoint(OperationPhase::Layout)?;
    workspace
        .budget
        .check_available(caps.search)
        .map_err(GridSegmentLimit::from)?;
    let base = input.base;
    let base_bytes = base.estimated_bytes();
    let retained = match caps.retained_base_bytes {
        0 => base_bytes,
        bytes if bytes >= base_bytes => bytes,
        _ => {
            return Err(
                invalid("Grid retained topology estimate excludes the current base").into(),
            );
        }
    };
    let [start, end] = input.endpoints;
    let overlay = base.overlay(
        GridOverlayInput {
            source: start.connect,
            target: end.connect,
            lane_points: input.lane.as_slice(),
        },
        &mut workspace.overlay,
        work,
    )?;
    let added_bytes = overlay
        .estimated_bytes()
        .checked_sub(base_bytes)
        .ok_or_else(|| invalid("Grid overlay estimate excludes the current base"))?;
    let live_bytes = retained
        .checked_add(added_bytes)
        .ok_or_else(|| work.arithmetic_overflow())?;
    work.checkpoint(OperationPhase::Layout)?;
    if input.preflight_overlay && live_bytes > caps.max_topology_bytes {
        return Err(
            GridSegmentLimit::Topology(GridTopologyLimit::EstimatedMemory {
                actual: live_bytes,
                max: caps.max_topology_bytes,
            })
            .into(),
        );
    }
    let source = overlay.vertex_id(start.connect, work)?;
    let target = overlay.vertex_id(end.connect, work)?;
    let (Some(source), Some(target)) = (source, target) else {
        return Ok(OverlayPath::MissingEndpoint);
    };
    let topology = ValidatedGridSearchTopology::new(&overlay, work)?;
    let mut search_caps = caps.search;
    search_caps
        .max_estimated_bytes
        .get_or_insert(caps.max_topology_bytes);
    let terminal = |vertex, endpoint: Terminal, approach_length| GridSearchTerminal {
        vertex,
        approach_length,
        orientation: Some(endpoint.side.axis()),
        owner_side: Some(endpoint.side),
    };
    let query = GridSearchQuery {
        source: terminal(source, start, input.lengths[0]),
        target: terminal(target, end, input.lengths[1]),
        endpoint_candidate_rank: 0,
    };
    let found = find_grid_sparse_route(
        &topology,
        query,
        GridSearchOptions {
            caps: search_caps,
            estimated_bytes_base: live_bytes,
            ..GridSearchOptions::default()
        },
        &mut workspace.search,
        &mut workspace.budget,
        work,
        |a, b| constraints.allows_arc(a, b, input.avoid_occupied, work),
    )?;
    match found {
        Some(route) => Ok(OverlayPath::Route(attach_ports(
            start.port,
            &route.points,
            end.port,
            work,
        )?)),
        None => Ok(OverlayPath::NoRoute),
    }
}

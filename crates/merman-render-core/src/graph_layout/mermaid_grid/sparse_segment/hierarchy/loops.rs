use super::super::super::forest::GridItemKind;
use super::super::super::label_spacing::LABEL_CLEARANCE;
use super::super::super::prepared::PreparedGrid;
use super::super::super::route_plan::{legal_side_interval, parent, side_index};
use super::super::super::routing_context::{GridRoutingBases, GridRoutingContainers};
use super::super::constraints::{Constraints, RouteGeometry, normalize_points};
use super::super::{APPROACH, AttemptError};
use super::attachments::{Terminal, as_rect, face, point, sign, tangent_axis, title};
use super::segment::{OverlayPath, SearchInput, search_pair};
use super::{
    GridHierarchyError, GridHierarchySegmentKind, GridSegmentCaps, GridSegmentTopology,
    GridSegmentWorkspace, invalid, reserved, topology,
};
use crate::graph_layout::{Axis, Side, Size};
use crate::resources::OperationWorkMeter;
use kurbo::{Point, Rect};
use merman_model::OperationPhase;

const PORT_GAP: f64 = 18.0;
const PORT_OFFSET_STEP: f64 = 4.0;
const LOOP_APPROACH: f64 = 14.0;
const TRACK_GAP: f64 = 18.0;
const CORNER_CLEARANCE: f64 = 4.0;
const LABEL_SPAN_MARGIN: f64 = 2.0;

#[derive(Clone, Copy)]
pub(in super::super::super) struct GridLoopInput<'a> {
    /// Count non-loop endpoint demands for this owner, indexed by `route_plan::side_index`.
    pub owner_side_counts: [usize; 4],
    /// Count committed loops for this owner, indexed by `route_plan::side_index`.
    pub committed_loop_counts: [usize; 4],
    pub committed_pair_routes: &'a [&'a [Point]],
}

#[derive(Debug)]
pub(in super::super::super) struct GridLoopRoute {
    pub points: Vec<Point>,
    pub side: Side,
    /// The previous committed count for this side. Commit `index + 1` after admission.
    pub index: usize,
    pub kind: GridHierarchySegmentKind,
}

#[allow(
    clippy::too_many_arguments,
    reason = "A loop borrows its session's inputs, state and meter."
)]
pub(in super::super::super) fn route_grid_self_loop(
    prepared: &PreparedGrid<'_>,
    containers: &GridRoutingContainers<'_>,
    bases: &GridRoutingBases<'_>,
    route_slot: usize,
    input: GridLoopInput<'_>,
    caps: GridSegmentCaps,
    workspace: &mut GridSegmentWorkspace,
    work: &OperationWorkMeter,
) -> Result<GridLoopRoute, GridHierarchyError> {
    work.checkpoint(OperationPhase::Layout)?;
    let plan = prepared
        .plans
        .get(route_slot)
        .ok_or_else(|| invalid("Missing Grid loop slot"))?;
    let owner = plan.edge.source;
    if owner != plan.edge.target {
        return Err(invalid("Grid self-loop has different endpoints").into());
    }
    let geometry = &prepared.geometry;
    let owner_bounds = as_rect(
        geometry
            .item(owner)
            .ok_or_else(|| invalid("Missing Grid loop owner"))?
            .bounds,
    );
    let container =
        parent(&prepared.forest, owner).ok_or_else(|| invalid("Missing Grid loop parent"))?;
    let base = topology(bases, container)?;
    let owner_child = containers.local_slot(owner)?;
    let view = containers.container(container)?.view();
    let titled = title(geometry, owner).is_some();
    let sides = ordered_sides(input, titled, work)?;
    let label = if prepared
        .label_requirements
        .self_loops
        .contains_key(&route_slot)
    {
        Some(
            prepared
                .edges
                .get(route_slot)
                .and_then(|edge| edge.label)
                .ok_or_else(|| invalid("Missing measured Grid loop label"))?
                .size,
        )
    } else {
        None
    };
    let constraints = Constraints::from_routes(
        RouteGeometry {
            children: view.children,
            terminals: [Some(owner_child); 2],
            require_approaches: true,
        },
        Some(input.committed_pair_routes),
        None,
        work,
    )?;

    let legacy = |limit| -> Result<_, GridHierarchyError> {
        let obstacles = compatibility_obstacles(prepared, owner, work)?;
        for (side, index) in sides.clone() {
            work.charge(1)?;
            let Some(points) = loop_candidate(owner_bounds, side, index, label, work)? else {
                continue;
            };
            if intersects_obstacles(&points, &obstacles, work)? {
                continue;
            }
            // The compatibility router picks the first raw-obstacle-clear loop. The outer
            // sparse fallback validates exactly that candidate, even if a later side is free.
            let valid = constraints.validates(&points, false, work)?;
            work.checkpoint(OperationPhase::Layout)?;
            return if valid {
                Ok(GridLoopRoute {
                    points,
                    side,
                    index,
                    kind: GridHierarchySegmentKind::ResourceFallback(limit),
                })
            } else {
                Err(GridHierarchyError::InvalidResourceFallback { container, limit })
            };
        }
        Err(GridHierarchyError::NoCompatibilityLoop { owner })
    };
    let base = match base {
        GridSegmentTopology::Sparse(base) => base,
        GridSegmentTopology::Compatibility(limit) => return legacy(limit.into()),
    };
    if label.is_some() {
        for (side, index) in sides.clone() {
            work.charge(1)?;
            if let Some(points) = loop_candidate(owner_bounds, side, index, label, work)?
                && constraints.validates(&points, false, work)?
            {
                work.checkpoint(OperationPhase::Layout)?;
                return Ok(GridLoopRoute {
                    points,
                    side,
                    index,
                    kind: GridHierarchySegmentKind::LabelAwareLoop,
                });
            }
        }
    }

    let caps = GridSegmentCaps {
        retained_base_bytes: bases.retained_bytes,
        ..caps
    };
    for (side, index) in sides.clone() {
        work.charge(1)?;
        let Some((low, high)) = legal_side_interval(geometry, owner, side) else {
            continue;
        };
        let Some(endpoints) = sparse_attachments(owner, owner_bounds, side, index, low, high)
        else {
            continue;
        };
        let [a, b] = endpoints.map(|end| end.connect);
        let lane = Some(Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0));
        let result = search_pair(
            SearchInput {
                base,
                endpoints,
                lengths: [APPROACH; 2],
                lane,
                preflight_overlay: false,
                avoid_occupied: false,
            },
            &constraints,
            caps,
            workspace,
            work,
        );
        match result {
            Ok(OverlayPath::Route(points)) => {
                if constraints.validates(&points, false, work)? {
                    work.checkpoint(OperationPhase::Layout)?;
                    return Ok(GridLoopRoute {
                        points,
                        side,
                        index,
                        kind: GridHierarchySegmentKind::Sparse,
                    });
                }
            }
            Ok(OverlayPath::NoRoute) => {}
            Ok(OverlayPath::MissingEndpoint) => {
                return Err(GridHierarchyError::MissingEndpointProjection { container });
            }
            Err(AttemptError::Layout(error)) => return Err(error.into()),
            Err(AttemptError::Limit(limit)) => return legacy(limit),
        }
    }
    work.checkpoint(OperationPhase::Layout)?;
    Err(GridHierarchyError::NoSelfLoop { owner })
}

fn ordered_sides(
    input: GridLoopInput<'_>,
    titled: bool,
    work: &OperationWorkMeter,
) -> crate::Result<impl Iterator<Item = (Side, usize)> + Clone> {
    let mut ranked = [
        (Side::Left, 0, 0),
        (Side::Right, 0, 0),
        (Side::Top, 0, 0),
        (Side::Bottom, 0, 0),
    ];
    for (side, index, load) in &mut ranked {
        work.charge(1)?;
        let slot = side_index(*side);
        *index = input.committed_loop_counts[slot];
        *load = if titled && *side == Side::Top {
            0
        } else {
            input.owner_side_counts[slot]
                .checked_add(*index)
                .ok_or_else(|| work.arithmetic_overflow())?
        };
    }
    ranked.sort_by_key(|&(_, _, load)| load);
    Ok(ranked
        .into_iter()
        .filter(move |&(side, _, _)| !titled || side != Side::Top)
        .map(|(side, index, _)| (side, index)))
}

fn sparse_attachments(
    owner: usize,
    bounds: Rect,
    side: Side,
    index: usize,
    low: f64,
    high: f64,
) -> Option<[Terminal; 2]> {
    if high - low < PORT_GAP {
        return None;
    }
    let center = (low + high) / 2.0;
    let offset = index as f64 * PORT_OFFSET_STEP;
    let start = center - PORT_GAP / 2.0 - offset;
    let end = center + PORT_GAP / 2.0 + offset;
    if start < low || end > high {
        return None;
    }
    let face = face(bounds, side);
    let connect = face + sign(side) * APPROACH;
    Some([start, end].map(|coordinate| Terminal {
        owner,
        side,
        port: point(side, face, coordinate),
        connect: point(side, connect, coordinate),
        corridor_axis: tangent_axis(side),
        selection: None,
    }))
}

fn loop_candidate(
    bounds: Rect,
    side: Side,
    index: usize,
    label: Option<Size>,
    work: &OperationWorkMeter,
) -> crate::Result<Option<Vec<Point>>> {
    let vertical = side.axis() == Axis::Horizontal;
    let center = if vertical {
        bounds.center().y
    } else {
        bounds.center().x
    };
    let base_depth = LOOP_APPROACH + (index as f64 + 1.0) * TRACK_GAP;
    let offset = index as f64 * PORT_OFFSET_STEP;
    let start = center - PORT_GAP / 2.0 - offset;
    let end = center + PORT_GAP / 2.0 + offset;
    let face = face(bounds, side);
    let at = |depth, coordinate| point(side, face + sign(side) * depth, coordinate);
    let Some(label) = label else {
        return normalize_points(
            &[
                at(0.0, start),
                at(LOOP_APPROACH, start),
                at(base_depth, start),
                at(base_depth, end),
                at(LOOP_APPROACH, end),
                at(0.0, end),
            ],
            work,
        )
        .map(Some);
    };

    let (along, cross) = if vertical {
        (label.height, label.width)
    } else {
        (label.width, label.height)
    };
    let span = (PORT_GAP + offset * 2.0).max(along + LABEL_CLEARANCE * 2.0 + LABEL_SPAN_MARGIN);
    let depth = base_depth.max(cross / 2.0 + LABEL_CLEARANCE);
    let direct_start = center - span / 2.0;
    let direct_end = center + span / 2.0;
    let (low, high) = if vertical {
        (bounds.y0, bounds.y1)
    } else {
        (bounds.x0, bounds.x1)
    };
    if direct_start - low >= CORNER_CLEARANCE && high - direct_end >= CORNER_CLEARANCE {
        return normalize_points(
            &[
                at(0.0, direct_start),
                at(depth, direct_start),
                at(depth, direct_end),
                at(0.0, direct_end),
            ],
            work,
        )
        .map(Some);
    }
    if start < low || end > high {
        return Ok(None);
    }
    let shoulder = LOOP_APPROACH.max(APPROACH);
    let shoulder_depth = depth.max(shoulder + cross / 2.0 + LABEL_CLEARANCE);
    normalize_points(
        &[
            at(0.0, start),
            at(shoulder, start),
            at(shoulder, direct_start),
            at(shoulder_depth, direct_start),
            at(shoulder_depth, direct_end),
            at(shoulder, direct_end),
            at(shoulder, end),
            at(0.0, end),
        ],
        work,
    )
    .map(Some)
}

fn compatibility_obstacles(
    prepared: &PreparedGrid<'_>,
    owner: usize,
    work: &OperationWorkMeter,
) -> crate::Result<Vec<Rect>> {
    let mut obstacles = reserved(prepared.forest.items.len().saturating_sub(1), work)?;
    for (slot, item) in prepared.forest.items.iter().enumerate() {
        work.charge(1)?;
        if slot == owner {
            continue;
        }
        match item.kind {
            GridItemKind::Leaf(_) => {
                let bounds = prepared
                    .geometry
                    .item(slot)
                    .ok_or_else(|| invalid("Missing measured Grid loop obstacle"))?
                    .bounds;
                obstacles.push(as_rect(bounds));
            }
            GridItemKind::Group(_) => obstacles.extend(title(&prepared.geometry, slot)),
            GridItemKind::EdgeLabel => {}
        }
    }
    Ok(obstacles)
}

fn intersects_obstacles(
    points: &[Point],
    obstacles: &[Rect],
    work: &OperationWorkMeter,
) -> crate::Result<bool> {
    for obstacle in obstacles {
        work.charge(1)?;
        for pair in points.windows(2) {
            work.charge(1)?;
            let (a, b) = (pair[0], pair[1]);
            let intersects = if a.y == b.y {
                a.y >= obstacle.y0
                    && a.y <= obstacle.y1
                    && a.x.min(b.x).max(obstacle.x0) < a.x.max(b.x).min(obstacle.x1)
            } else if a.x == b.x {
                a.x >= obstacle.x0
                    && a.x <= obstacle.x1
                    && a.y.min(b.y).max(obstacle.y0) < a.y.max(b.y).min(obstacle.y1)
            } else {
                false
            };
            if intersects {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

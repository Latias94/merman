use super::{MarkerOccupant, SceneOccupancy, TerminalClaim};
use crate::error::{AsciiError, Result};
use crate::graph::charset::GraphCharset;
use crate::graph::routing::layout_allocation_failed;
use crate::graph::routing::plan::{MarkerCandidate, MarkerEndpoint};
use crate::graph::routing::{PreparedRoute, sort_levels};
#[cfg(test)]
use crate::resource::AsciiResourceLimitId;
use crate::resource::ResourceContext;

#[cfg(test)]
pub(in crate::graph::routing) fn allocate_marker_berths(
    routes: &mut [PreparedRoute],
    occupancy: &mut SceneOccupancy<'_>,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
    diagram_type: &'static str,
) -> Result<()> {
    let marker_capacity = routes.len().checked_mul(2).ok_or_else(|| {
        resources
            .policy()
            .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
    })?;
    let mut pending = Vec::new();
    pending
        .try_reserve(marker_capacity)
        .map_err(|_| layout_allocation_failed())?;
    collect_marker_requests(
        routes,
        0..routes.len(),
        occupancy,
        resources,
        diagram_type,
        &mut pending,
    )?;
    allocate_pending_markers(routes, occupancy, charset, resources, diagram_type, pending)
}

pub(in crate::graph::routing) fn allocate_committed_route_marker_berths(
    routes: &mut [PreparedRoute],
    route_index: usize,
    occupancy: &mut SceneOccupancy<'_>,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
    diagram_type: &'static str,
) -> Result<()> {
    let mut pending = Vec::new();
    pending
        .try_reserve(2)
        .map_err(|_| layout_allocation_failed())?;
    collect_marker_requests(
        routes,
        route_index..route_index + 1,
        occupancy,
        resources,
        diagram_type,
        &mut pending,
    )?;
    allocate_pending_markers(routes, occupancy, charset, resources, diagram_type, pending)
}

fn collect_marker_requests(
    routes: &[PreparedRoute],
    indices: std::ops::Range<usize>,
    occupancy: &mut SceneOccupancy<'_>,
    resources: &mut ResourceContext,
    diagram_type: &'static str,
    pending: &mut Vec<PendingMarker>,
) -> Result<()> {
    for route_index in indices {
        let route = &routes[route_index];
        for endpoint in [MarkerEndpoint::Start, MarkerEndpoint::End] {
            let candidates = route
                .plan
                .marker_candidates(endpoint, diagram_type, resources)?
                .ok_or(AsciiError::UnsupportedFeature {
                    diagram_type,
                    feature: "endpoint markers without a straight terminal berth",
                })?;
            if !candidates.is_empty() {
                for candidate in candidates
                    .iter()
                    .filter(|candidate| !candidate.is_primary())
                {
                    resources.charge_layout_work(1)?;
                    occupancy.register_terminal_claim(
                        candidate.coord,
                        TerminalClaim {
                            route_index,
                            endpoint,
                            point_direction: candidate.point_direction,
                            primary: false,
                        },
                    )?;
                }
                pending.push(PendingMarker {
                    route_index,
                    endpoint,
                    candidates,
                });
            }
        }
    }

    Ok(())
}

fn allocate_pending_markers(
    routes: &mut [PreparedRoute],
    occupancy: &mut SceneOccupancy<'_>,
    charset: &GraphCharset,
    resources: &mut ResourceContext,
    diagram_type: &'static str,
    mut pending: Vec<PendingMarker>,
) -> Result<()> {
    let sort_work = resources.checked_work_mul(pending.len(), sort_levels(pending.len()))?;
    resources.charge_layout_work(sort_work)?;
    pending.sort_by(|left, right| {
        left.candidates
            .len()
            .cmp(&right.candidates.len())
            .then_with(|| {
                routes[left.route_index]
                    .owner
                    .canonical_edge_index
                    .cmp(&routes[right.route_index].owner.canonical_edge_index)
            })
            .then_with(|| {
                marker_endpoint_order(left.endpoint).cmp(&marker_endpoint_order(right.endpoint))
            })
    });

    for marker in pending {
        let mut selected = None;
        let mut predecessor = None;
        for candidate in marker.candidates.iter().copied() {
            resources.charge_layout_work(1)?;
            if !marker_candidate_continues_chain(predecessor, candidate) {
                break;
            }
            match occupancy.marker_candidate_disposition(
                routes,
                marker.route_index,
                marker.endpoint,
                candidate,
                resources,
            )? {
                MarkerCandidateDisposition::Available => {
                    selected = Some(candidate);
                    break;
                }
                MarkerCandidateDisposition::CompatiblePassThrough => {
                    predecessor = Some(candidate);
                }
                MarkerCandidateDisposition::Blocked => break,
            }
        }
        let Some(candidate) = selected else {
            return Err(AsciiError::RoutingSearchExhausted {
                diagram_type,
                edge_index: routes[marker.route_index].owner.source_edge_index,
            });
        };
        routes[marker.route_index].plan.materialize_marker_at(
            marker.endpoint,
            candidate,
            charset,
            diagram_type,
        )?;
        occupancy.suppress_route_terminal_tail(
            marker.route_index,
            candidate.terminal_tail(),
            &routes[marker.route_index].plan,
            resources,
            diagram_type,
        )?;
        let cell = routes[marker.route_index]
            .plan
            .materialized_marker_cell(marker.endpoint, diagram_type)?
            .ok_or(AsciiError::UnsupportedFeature {
                diagram_type,
                feature: "routes with unmaterialized endpoint markers",
            })?;
        occupancy.occupy_marker(
            cell.coord,
            MarkerOccupant {
                route_index: marker.route_index,
                endpoint: marker.endpoint,
                point_direction: candidate.point_direction,
                shared_group: routes[marker.route_index]
                    .owner
                    .shared_marker_group(marker.endpoint, cell.coord),
            },
            resources,
            diagram_type,
        )?;
    }
    Ok(())
}

struct PendingMarker {
    route_index: usize,
    endpoint: MarkerEndpoint,
    candidates: Vec<MarkerCandidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::graph::routing) enum MarkerCandidateDisposition {
    Available,
    CompatiblePassThrough,
    Blocked,
}

pub(super) fn marker_candidate_continues_chain(
    predecessor: Option<MarkerCandidate>,
    candidate: MarkerCandidate,
) -> bool {
    match predecessor {
        Some(predecessor) => candidate.follows_terminal_predecessor(predecessor),
        None => candidate.is_primary(),
    }
}

const fn marker_endpoint_order(endpoint: MarkerEndpoint) -> u8 {
    match endpoint {
        MarkerEndpoint::Start => 0,
        MarkerEndpoint::End => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{OccupiedRect, label_reservation};
    use super::*;
    use crate::AsciiRenderOptions;
    use crate::color::AsciiColorRole;
    use crate::graph::layout::{CanvasCoord, GraphLayout, layout_graph};
    use crate::graph::model::{AsciiGraph, GraphDirection, GraphEdgeMarker, GraphEdgeStroke};
    use crate::graph::routing::RouteOwner;
    use crate::graph::routing::cell::{DIR_DOWN, DIR_LEFT, DIR_RIGHT, DIR_UP};
    use crate::graph::routing::conflict::{MAX_BLOCKER_WITNESSES, ReservationKind};
    use crate::graph::routing::path::StepDirection;
    use crate::graph::routing::plan::{
        LabelAnchor, MarkerAnchor, MarkerAnchors, NodeAttachment, NodeAttachments, PlannedCellId,
        PlannedRouteCell, PlannedRouteCellKind, PlannedRoutePaint, PlannedRouteSegment, RoutePlan,
    };
    use crate::resource::AsciiResourcePolicy;
    use merman_model::OperationPhase;

    fn empty_layout() -> GraphLayout {
        layout_graph(
            &AsciiGraph::new(GraphDirection::LeftRight),
            &AsciiRenderOptions::ascii(),
        )
    }

    fn route(canonical_edge_index: usize, y: usize) -> PreparedRoute {
        let cells = (0..7)
            .map(|x| PlannedRouteCell {
                coord: CanvasCoord { x, y },
                ch: '-',
                kind: PlannedRouteCellKind::EdgeLine,
                segment: PlannedRouteSegment::Direct,
                stroke: GraphEdgeStroke::Normal,
                directions: DIR_LEFT | DIR_RIGHT,
                unicode: false,
                paint: PlannedRoutePaint::role(AsciiColorRole::EdgeLine),
            })
            .collect();
        let mut plan = RoutePlan::new(
            cells,
            Vec::new(),
            MarkerAnchors::new(
                MarkerAnchor::new(PlannedCellId::new(0), StepDirection::Left),
                MarkerAnchor::new(PlannedCellId::new(6), StepDirection::Right),
            ),
        );
        plan.attachments = Some(NodeAttachments {
            start: NodeAttachment {
                contact: CanvasCoord { x: 0, y },
                outward: StepDirection::Right,
                connector: None,
            },
            end: NodeAttachment {
                contact: CanvasCoord { x: 6, y },
                outward: StepDirection::Left,
                connector: None,
            },
        });
        PreparedRoute::for_test(plan, canonical_edge_index)
    }

    #[test]
    fn path_evidence_finds_old_and_overlapping_owners_and_preserves_clearance() {
        let layout = empty_layout();
        let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let mut routes = vec![
            route(91, 4),
            route(17, 10),
            route(33, 20),
            route(5, 30),
            route(7, 40),
            route(3, 4),
        ];
        let owner = RouteOwner::for_test(100, "other-source", "other-target");
        let scene = SceneOccupancy::try_new(&routes, &layout, &mut resources, "flowchart").unwrap();
        let queries = [
            (CanvasCoord { x: 3, y: 4 }, DIR_LEFT | DIR_RIGHT),
            (CanvasCoord { x: 3, y: 4 }, DIR_UP | DIR_DOWN),
            (CanvasCoord { x: 99, y: 99 }, DIR_LEFT | DIR_RIGHT),
        ];
        let plain: Vec<_> = queries
            .iter()
            .map(|&(coord, directions)| {
                scene
                    .route_cell_is_clear(&routes, &owner, None, coord, directions, &mut resources)
                    .unwrap()
            })
            .collect();
        assert_eq!(plain, [false, true, true]);
        assert!(scene.take_blocker_collection(&resources).unwrap().is_none());
        scene.begin_blocker_collection(&resources).unwrap();
        for ((coord, directions), clear) in queries.into_iter().zip(plain) {
            assert_eq!(
                scene
                    .route_cell_is_clear(&routes, &owner, None, coord, directions, &mut resources,)
                    .unwrap(),
                clear
            );
        }
        let evidence = scene.take_blocker_collection(&resources).unwrap().unwrap();
        assert_eq!(
            evidence
                .blockers
                .iter()
                .map(|owner| owner.canonical_edge_index)
                .collect::<Vec<_>>(),
            [3, 91]
        );
        assert!(
            evidence
                .blockers
                .iter()
                .all(|owner| owner.contains(ReservationKind::Route))
        );
        assert!(!evidence.static_restriction);
        assert!(!evidence.capacity_restriction);
        assert!(!evidence.truncated);

        routes.reverse();
        let reordered =
            SceneOccupancy::try_new(&routes, &layout, &mut resources, "flowchart").unwrap();
        reordered.begin_blocker_collection(&resources).unwrap();
        assert!(
            !reordered
                .route_cell_is_clear(
                    &routes,
                    &owner,
                    None,
                    queries[0].0,
                    queries[0].1,
                    &mut resources,
                )
                .unwrap()
        );
        assert_eq!(
            reordered
                .take_blocker_collection(&resources)
                .unwrap()
                .unwrap(),
            evidence
        );
    }

    #[test]
    fn terminal_evidence_excludes_compatible_and_transverse_claims() {
        let layout = empty_layout();
        let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let mut routes = [route(51, 4), route(82, 10), route(7, 20)];
        routes[0].owner.from = "a".to_owned();
        let owner = RouteOwner::for_test(100, "a", "b");
        let mut scene =
            SceneOccupancy::try_new(&routes, &layout, &mut resources, "flowchart").unwrap();
        let coord = CanvasCoord { x: 99, y: 99 };
        for route_index in 0..3 {
            scene
                .register_terminal_claim(
                    coord,
                    TerminalClaim {
                        route_index,
                        endpoint: MarkerEndpoint::Start,
                        point_direction: StepDirection::Right,
                        primary: route_index != 2,
                    },
                )
                .unwrap();
        }
        let directions = DIR_UP | DIR_DOWN;
        assert!(
            !scene
                .route_cell_is_clear(&routes, &owner, None, coord, directions, &mut resources)
                .unwrap()
        );
        scene.begin_blocker_collection(&resources).unwrap();
        assert!(
            !scene
                .route_cell_is_clear(&routes, &owner, None, coord, directions, &mut resources)
                .unwrap()
        );
        let evidence = scene.take_blocker_collection(&resources).unwrap().unwrap();
        assert_eq!(evidence.blockers.len(), 1);
        assert_eq!(evidence.blockers[0].canonical_edge_index, 82);
        assert!(evidence.blockers[0].contains(ReservationKind::Port));
    }

    #[test]
    fn marker_and_label_rejections_report_their_semantic_reservations() {
        let layout = empty_layout();
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
        let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let mut committed = route(73, 4);
        committed.plan = committed
            .plan
            .with_marker_requests(GraphEdgeMarker::Open, GraphEdgeMarker::Point, "flowchart")
            .unwrap();
        let mut routes = [committed];
        let mut scene =
            SceneOccupancy::try_new(&routes, &layout, &mut resources, "flowchart").unwrap();
        allocate_marker_berths(
            &mut routes,
            &mut scene,
            &charset,
            &mut resources,
            "flowchart",
        )
        .unwrap();
        let mut candidate_route = route(99, 4);
        candidate_route.owner.from = "other-source".to_owned();
        candidate_route.owner.to = "other-target".to_owned();
        let candidate = candidate_route
            .plan
            .terminal_candidate(MarkerEndpoint::End, "flowchart")
            .unwrap();
        let plain = scene
            .marker_candidate_disposition_before_commit(
                &routes,
                &candidate_route.owner,
                &candidate_route.plan,
                MarkerEndpoint::End,
                candidate,
                &mut resources,
            )
            .unwrap();
        assert_eq!(plain, MarkerCandidateDisposition::Blocked);
        scene.begin_blocker_collection(&resources).unwrap();
        assert_eq!(
            scene
                .marker_candidate_disposition_before_commit(
                    &routes,
                    &candidate_route.owner,
                    &candidate_route.plan,
                    MarkerEndpoint::End,
                    candidate,
                    &mut resources,
                )
                .unwrap(),
            plain
        );
        let evidence = scene.take_blocker_collection(&resources).unwrap().unwrap();
        assert_eq!(evidence.blockers.len(), 1);
        assert_eq!(evidence.blockers[0].canonical_edge_index, 73);
        assert!(evidence.blockers[0].contains(ReservationKind::Marker));

        let label = OccupiedRect::try_new(7, 8, 2, 2, &resources).unwrap();
        scene.occupy_label(label, 41, &mut resources).unwrap();
        let label_cells = scene.labels.len();
        let extent = scene.occupied_extent();
        let mut vertical = route(100, 8)
            .plan
            .terminal_candidate(MarkerEndpoint::End, "flowchart")
            .unwrap();
        vertical.point_direction = StepDirection::Down;
        let plain = scene
            .marker_reading_clearance_is_clear(
                &routes,
                &candidate_route.owner,
                &candidate_route.plan,
                MarkerEndpoint::End,
                vertical,
                &resources,
            )
            .unwrap();
        assert!(!plain);
        scene.begin_blocker_collection(&resources).unwrap();
        assert_eq!(
            scene
                .marker_reading_clearance_is_clear(
                    &routes,
                    &candidate_route.owner,
                    &candidate_route.plan,
                    MarkerEndpoint::End,
                    vertical,
                    &resources,
                )
                .unwrap(),
            plain
        );
        assert!(
            !scene
                .route_cell_is_clear(
                    &routes,
                    &candidate_route.owner,
                    None,
                    CanvasCoord { x: 8, y: 9 },
                    DIR_LEFT | DIR_RIGHT,
                    &mut resources,
                )
                .unwrap()
        );
        let evidence = scene.take_blocker_collection(&resources).unwrap().unwrap();
        assert_eq!(evidence.blockers.len(), 1);
        assert_eq!(evidence.blockers[0].canonical_edge_index, 41);
        assert!(evidence.blockers[0].contains(ReservationKind::Label));
        assert_eq!(scene.labels.len(), label_cells);
        assert_eq!(scene.occupied_extent(), extent);
    }

    #[test]
    fn label_gap_attributes_all_actual_route_owners_and_keeps_its_bounds() {
        let layout = empty_layout();
        let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let routes = [route(73, 4), route(17, 5), route(91, 11)];
        let scene = SceneOccupancy::try_new(&routes, &layout, &mut resources, "flowchart").unwrap();
        let owner = RouteOwner::for_test(100, "other-source", "other-target");
        let anchor = LabelAnchor::Segment {
            start: CanvasCoord { x: 0, y: 6 },
            end: CanvasCoord { x: 6, y: 6 },
            route_segment: None,
        };
        let text = OccupiedRect::try_new(2, 3, 2, 1, &resources).unwrap();
        let reservation = label_reservation(anchor, text, scene.label_lane_radius, &resources)
            .unwrap()
            .unwrap();
        assert!(reservation.contains(2, 4));
        assert!(reservation.contains(2, 5));
        assert!(!reservation.contains(2, 6));
        let plain = scene
            .label_candidate_is_clear(&routes, usize::MAX, &owner, anchor, text, &mut resources)
            .unwrap();
        assert!(!plain);
        scene.begin_blocker_collection(&resources).unwrap();
        assert_eq!(scene.label_candidate_is_clear(
            &routes, usize::MAX, &owner, anchor, text, &mut resources,
        ).unwrap(), plain);
        let evidence = scene.take_blocker_collection(&resources).unwrap().unwrap();
        assert_eq!(
            evidence
                .blockers
                .iter()
                .map(|owner| owner.canonical_edge_index)
                .collect::<Vec<_>>(),
            [17, 73]
        );
        assert!(
            evidence
                .blockers
                .iter()
                .all(|owner| owner.contains(ReservationKind::Route))
        );
    }

    #[test]
    fn group_restriction_does_not_request_unrelated_route_removal() {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("inside", "Inside");
        graph.add_group_with_style(
            "foreign",
            "Foreign",
            None,
            vec!["inside".to_owned()],
            crate::graph::model::GraphGroupStyle::default(),
        );
        let layout = layout_graph(&graph, &AsciiRenderOptions::ascii());
        let group = &layout.groups[0];
        let coord = CanvasCoord {
            x: group.x,
            y: group.y,
        };
        let routes = [route(73, coord.y)];
        let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let scene = SceneOccupancy::try_new(&routes, &layout, &mut resources, "flowchart").unwrap();
        let owner = RouteOwner::for_test(100, "other-source", "other-target");
        assert!(
            !scene
                .route_cell_is_clear(
                    &routes,
                    &owner,
                    None,
                    coord,
                    DIR_LEFT | DIR_RIGHT,
                    &mut resources
                )
                .unwrap()
        );
        scene.begin_blocker_collection(&resources).unwrap();
        assert!(
            !scene
                .route_cell_is_clear(
                    &routes,
                    &owner,
                    None,
                    coord,
                    DIR_LEFT | DIR_RIGHT,
                    &mut resources
                )
                .unwrap()
        );
        let evidence = scene.take_blocker_collection(&resources).unwrap().unwrap();
        assert!(evidence.static_restriction);
        assert!(!evidence.capacity_restriction);
        assert!(evidence.blockers.is_empty());
    }

    #[test]
    fn shared_marker_evidence_retains_contributors_across_replay() {
        let layout = empty_layout();
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
        let mut routes = vec![route(73, 0), route(17, 0)];
        for route in &mut routes {
            route.owner.set_source_group_for_test(Some(0));
            route.plan = route
                .plan
                .clone()
                .with_marker_requests(GraphEdgeMarker::Circle, GraphEdgeMarker::Open, "flowchart")
                .unwrap();
        }
        let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let mut scene =
            SceneOccupancy::try_new(&routes, &layout, &mut resources, "flowchart").unwrap();
        allocate_committed_route_marker_berths(
            &mut routes,
            0,
            &mut scene,
            &charset,
            &mut resources,
            "flowchart",
        )
        .unwrap();
        let owner = RouteOwner::for_test(100, "other-source", "other-target");
        let coord = CanvasCoord { x: 0, y: 0 };
        scene.begin_blocker_collection(&resources).unwrap();
        assert!(
            !scene
                .route_cell_is_clear(
                    &routes,
                    &owner,
                    None,
                    coord,
                    DIR_LEFT | DIR_RIGHT,
                    &mut resources,
                )
                .unwrap()
        );
        let evidence = scene.take_blocker_collection(&resources).unwrap().unwrap();
        assert_eq!(evidence.blockers.len(), 1);
        assert_eq!(evidence.blockers[0].canonical_edge_index, 73);

        allocate_committed_route_marker_berths(
            &mut routes,
            1,
            &mut scene,
            &charset,
            &mut resources,
            "flowchart",
        )
        .unwrap();
        scene.begin_blocker_collection(&resources).unwrap();
        assert!(
            !scene
                .route_cell_is_clear(
                    &routes,
                    &owner,
                    None,
                    coord,
                    DIR_LEFT | DIR_RIGHT,
                    &mut resources,
                )
                .unwrap()
        );
        let evidence = scene.take_blocker_collection(&resources).unwrap().unwrap();
        assert_eq!(
            evidence
                .blockers
                .iter()
                .map(|owner| owner.canonical_edge_index)
                .collect::<Vec<_>>(),
            [17, 73]
        );
        assert!(
            evidence
                .blockers
                .iter()
                .all(|owner| owner.contains(ReservationKind::Marker))
        );
        scene = scene
            .rebuild_for_routes(&routes, &[0], &mut resources, "flowchart")
            .unwrap();
        routes.remove(0);
        scene.begin_blocker_collection(&resources).unwrap();
        assert!(
            !scene
                .route_cell_is_clear(
                    &routes,
                    &owner,
                    None,
                    coord,
                    DIR_LEFT | DIR_RIGHT,
                    &mut resources,
                )
                .unwrap()
        );
        let evidence = scene.take_blocker_collection(&resources).unwrap().unwrap();
        assert_eq!(evidence.blockers.len(), 1);
        assert_eq!(evidence.blockers[0].canonical_edge_index, 17);
    }

    #[test]
    fn collector_scans_stop_on_work_exhaustion_and_cancellation() {
        let layout = empty_layout();
        let routes: Vec<_> = (0..MAX_BLOCKER_WITNESSES * 2)
            .map(|index| route(index * 7, 4))
            .collect();
        let owner = RouteOwner::for_test(1000, "other-source", "other-target");
        for cancel in [false, true] {
            let mut setup = ResourceContext::new(AsciiResourcePolicy::unbounded());
            let scene = SceneOccupancy::try_new(&routes, &layout, &mut setup, "flowchart").unwrap();
            let policy = if cancel {
                AsciiResourcePolicy::unbounded()
            } else {
                AsciiResourcePolicy::unbounded()
                    .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 256)
                    .unwrap()
            };
            let mut resources = ResourceContext::new(policy);
            scene.begin_blocker_collection(&resources).unwrap();
            if cancel {
                scene.control.cancel_after_checkpoints(8);
            }
            let error = scene
                .route_cell_is_clear(
                    &routes,
                    &owner,
                    None,
                    CanvasCoord { x: 3, y: 4 },
                    DIR_LEFT | DIR_RIGHT,
                    &mut resources,
                )
                .unwrap_err();
            if cancel {
                assert!(matches!(error, AsciiError::Cancelled(cancelled)
                    if cancelled.phase == OperationPhase::Layout));
            } else {
                assert!(matches!(error, AsciiError::ResourceLimitExceeded(details)
                    if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits));
            }
            let collection = scene.blockers.borrow();
            let evidence = collection.as_ref().unwrap();
            assert!(!evidence.blockers.is_empty());
            assert!(evidence.blockers.len() < MAX_BLOCKER_WITNESSES);
            assert!(!evidence.truncated);
            assert!(resources.layout_work_used() > MAX_BLOCKER_WITNESSES);
            assert_eq!(
                scene.route_cells[&CanvasCoord { x: 3, y: 4 }].owners.len(),
                routes.len()
            );
        }
    }
}

use super::{
    MarkerOccupant, OccupiedRect, SceneOccupancy, TerminalClaim, label_reservation,
    try_reserve_hash_map,
};
use crate::error::{AsciiError, Result};
use crate::graph::routing::PreparedRoute;
use crate::graph::routing::plan::MarkerEndpoint;
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use merman_model::OperationPhase;

impl<'layout> SceneOccupancy<'layout> {
    /// Replays retained contributors without changing the incumbent, even on failure.
    pub(in crate::graph::routing) fn rebuild_for_routes(
        &self,
        routes: &[PreparedRoute],
        omitted: &[usize],
        resources: &mut ResourceContext,
        diagram_type: &'static str,
    ) -> Result<Self> {
        self.checkpoint_layout()?;
        debug_assert_eq!(routes.len(), self.route_bounds.len());
        debug_assert!(omitted.iter().all(|&index| index < routes.len()));
        debug_assert!(omitted.windows(2).all(|pair| pair[0] < pair[1]));
        // This view shares the original ledger, including all failed replay work.
        let mut replay_resources =
            resources.controlled(self.control.clone(), OperationPhase::Layout);
        let resources = &mut replay_resources;
        let policy = resources.policy();
        // Admit constructor storage: bounds, two terminal/marker slots, both ports,
        // and an initial label slot per route. Immutable protection is shared with the trial.
        resources.charge_layout_work_product(self.route_capacity, 8)?;
        let mut rebuilt = Self::try_new_with_protection(
            std::rc::Rc::clone(&self.label_regions),
            self.route_capacity,
            self.label_lane_radius,
            std::rc::Rc::clone(&self.protected),
            resources,
            AsciiExecution::new(&self.control, &policy),
        )?;

        for (old_index, route) in routes.iter().enumerate() {
            resources.charge_layout_work(omitted.len())?;
            if omitted.binary_search(&old_index).is_ok() {
                continue;
            }
            let route_index = rebuilt.route_bounds.len();
            rebuilt.checkpoint_layout()?;
            resources.charge_layout_work(route.plan.cells.len())?;
            try_reserve_hash_map(&mut rebuilt.route_cells, route.plan.cells.len())?;
            rebuilt.commit_route_geometry(route_index, route, resources)?;
            for endpoint in [MarkerEndpoint::Start, MarkerEndpoint::End] {
                if let Some(cell) = route
                    .plan
                    .materialized_marker_cell(endpoint, diagram_type)?
                {
                    rebuilt.occupy_marker(
                        cell.coord,
                        MarkerOccupant {
                            route_index,
                            endpoint,
                            point_direction: route.plan.marker_point_direction(endpoint),
                            shared_group: route.owner.shared_marker_group(endpoint, cell.coord),
                        },
                        resources,
                        diagram_type,
                    )?;
                }
            }
            for label in &route.plan.labels {
                rebuilt.checkpoint_layout()?;
                let text = OccupiedRect::try_new(
                    label.placement.x(),
                    label.placement.y(),
                    label.placement.width(),
                    label.line_count(),
                    resources,
                )?;
                let reservation =
                    label_reservation(label.anchor, text, self.label_lane_radius, resources)?
                        .ok_or(AsciiError::UnsupportedFeature {
                            diagram_type,
                            feature: "finalized edge label without a host reservation",
                        })?;
                resources.charge_layout_work(reservation.cell_count(resources)?)?;
                rebuilt.occupy_label(reservation, route.owner.canonical_edge_index, resources)?;
            }
        }
        // Marker relocation changes the plan's anchor. Preserve the original candidate
        // windows instead of inventing new reservations behind the relocated marker.
        for (&coord, claims) in &self.terminal_claims {
            for claim in claims {
                rebuilt.checkpoint_layout()?;
                resources.charge_layout_work(resources.checked_work_add(omitted.len(), 1)?)?;
                if omitted.binary_search(&claim.route_index).is_err() {
                    let route_index = claim.route_index
                        - omitted.partition_point(|&index| index < claim.route_index);
                    rebuilt.register_terminal_claim(
                        coord,
                        TerminalClaim {
                            route_index,
                            ..*claim
                        },
                    )?;
                }
            }
        }
        rebuilt.checkpoint_layout()?;
        Ok(rebuilt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsciiRenderOptions;
    use crate::color::AsciiColorRole;
    use crate::graph::charset::GraphCharset;
    use crate::graph::layout::{CanvasCoord, GraphLayout, layout_graph};
    use crate::graph::model::{AsciiGraph, GraphDirection, GraphEdgeMarker, GraphEdgeStroke};
    use crate::graph::routing::label::{RoutedLabelPlacement, RoutedLabelText};
    use crate::graph::routing::path::StepDirection;
    use crate::graph::routing::plan::{
        LabelAnchor, MarkerAnchor, MarkerAnchors, NodeAttachment, NodeAttachments, PlannedCellId,
        PlannedRouteCell, PlannedRouteCellKind, PlannedRouteLabel, PlannedRoutePaint,
        PlannedRouteSegment, RoutePlan,
    };
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};

    fn empty_layout() -> GraphLayout {
        layout_graph(
            &AsciiGraph::new(GraphDirection::LeftRight),
            &AsciiRenderOptions::ascii(),
        )
    }

    fn route(canonical_index: usize, y: usize) -> PreparedRoute {
        route_with_length(canonical_index, y, 6)
    }

    fn route_with_length(canonical_index: usize, y: usize, length: usize) -> PreparedRoute {
        let cells = (0..length)
            .map(|x| PlannedRouteCell {
                coord: CanvasCoord { x, y },
                ch: '-',
                kind: PlannedRouteCellKind::EdgeLine,
                segment: PlannedRouteSegment::Direct,
                stroke: GraphEdgeStroke::Normal,
                directions: 0,
                unicode: false,
                paint: PlannedRoutePaint::role(AsciiColorRole::EdgeLine),
            })
            .collect();
        let mut plan = RoutePlan::new(
            cells,
            Vec::new(),
            MarkerAnchors::new(
                MarkerAnchor::new(PlannedCellId::new(0), StepDirection::Left),
                MarkerAnchor::new(PlannedCellId::new(length - 1), StepDirection::Right),
            ),
        );
        plan.attachments = Some(NodeAttachments {
            start: NodeAttachment {
                contact: CanvasCoord { x: 0, y },
                outward: StepDirection::Right,
                connector: None,
            },
            end: NodeAttachment {
                contact: CanvasCoord { x: length - 1, y },
                outward: StepDirection::Left,
                connector: None,
            },
        });
        PreparedRoute::for_test(plan, canonical_index)
    }

    fn geometry(scene: &SceneOccupancy<'_>) -> String {
        format!(
            "{:?}",
            (
                &scene.route_cells,
                &scene.route_bounds,
                &scene.terminal_claims,
                &scene.source_ports,
                &scene.target_ports,
                &scene.markers,
                &scene.labels,
                &scene.label_owners,
                &scene.protected,
                scene.occupied_extent,
            )
        )
    }

    #[test]
    fn removing_a_merge_contributor_preserves_the_shared_marker_with_dense_indices() {
        for direction in [GraphDirection::LeftRight, GraphDirection::TopDown] {
            for marker in [
                GraphEdgeMarker::Point,
                GraphEdgeMarker::Circle,
                GraphEdgeMarker::Cross,
            ] {
                let mut graph = AsciiGraph::new(direction);
                graph.add_node("target", "Target");
                for id in ["a", "b", "c"] {
                    graph.add_node(id, id);
                    graph.add_edge(id, "target");
                }
                for (index, edge) in graph.edges.iter_mut().enumerate() {
                    edge.end_marker = marker;
                    edge.start_marker = if index % 2 == 0 {
                        GraphEdgeMarker::Circle
                    } else {
                        GraphEdgeMarker::Cross
                    };
                    edge.label = Some(format!("if-{}", edge.from));
                }
                // Keep condition reading margins inside the private legs. This test owns
                // replay of an admitted merge, rather than choosing a denser presentation.
                let options = AsciiRenderOptions::unicode()
                    .with_graph_padding_x(12)
                    .with_graph_padding_y(12);
                let charset = GraphCharset::for_options(&options);
                let layout = layout_graph(&graph, &options);
                let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
                let routes = crate::graph::routing::prepare_route_scene_with_resources(
                    &graph,
                    &layout,
                    &graph.edges,
                    &charset,
                    &mut resources,
                )
                .unwrap()
                .routes;
                assert!(
                    routes
                        .iter()
                        .all(|route| route.owner.allocated_branch.is_some()),
                    "merge allocation was not retained: {direction:?}, {marker:?}, nodes={:?}, owners={:?}",
                    layout.nodes,
                    routes.iter().map(|route| &route.owner).collect::<Vec<_>>()
                );
                let common = routes[0]
                    .plan
                    .materialized_marker_cell(MarkerEndpoint::End, "flowchart")
                    .unwrap()
                    .unwrap();
                let scene =
                    SceneOccupancy::try_new(&routes, &layout, &mut resources, "flowchart").unwrap();
                let scene = scene
                    .rebuild_for_routes(&routes, &[], &mut resources, "flowchart")
                    .unwrap();
                let original = geometry(&scene);
                for omitted in [
                    &[][..],
                    &[0][..],
                    &[1][..],
                    &[2][..],
                    &[0, 1][..],
                    &[0, 2][..],
                    &[1, 2][..],
                    &[0, 1, 2][..],
                ] {
                    let rebuilt = scene
                        .rebuild_for_routes(&routes, omitted, &mut resources, "flowchart")
                        .unwrap();
                    assert!(std::rc::Rc::ptr_eq(&scene.protected, &rebuilt.protected));
                    let retained = 3 - omitted.len();
                    assert_eq!(rebuilt.markers.len(), retained + usize::from(retained > 0));
                    if retained > 0 {
                        let representative = rebuilt.markers.get(&common.coord).unwrap();
                        assert_eq!(representative.route_index, 0);
                        assert_eq!(representative.shared_group, Some(0));
                        assert_eq!(representative.endpoint, MarkerEndpoint::End);
                    }
                    assert_eq!(rebuilt.route_bounds.len(), retained);
                    for (old_index, route) in routes.iter().enumerate() {
                        if omitted.contains(&old_index) {
                            continue;
                        }
                        let source_marker = route
                            .plan
                            .materialized_marker_cell(MarkerEndpoint::Start, "flowchart")
                            .unwrap()
                            .unwrap();
                        let source = rebuilt.markers.get(&source_marker.coord).unwrap();
                        assert_eq!(source.endpoint, MarkerEndpoint::Start);
                        assert_eq!(source.shared_group, None);
                        assert_eq!(
                            source.route_index,
                            old_index - omitted.partition_point(|&index| index < old_index)
                        );
                        assert_eq!(route.plan.labels.len(), 1);
                        let label = &route.plan.labels[0];
                        for y in label.placement.y()..label.placement.y() + label.line_count() {
                            for x in
                                label.placement.x()..label.placement.x() + label.placement.width()
                            {
                                assert!(rebuilt.labels.contains(&CanvasCoord { x, y }));
                            }
                        }
                        assert!(rebuilt.label_owners.iter().any(|owner| {
                            owner.canonical_edge_index == route.owner.canonical_edge_index
                                && owner
                                    .reservation
                                    .contains(label.placement.x(), label.placement.y())
                        }));
                    }
                    assert_eq!(geometry(&scene), original);
                }
            }
        }
    }

    #[test]
    fn replay_retains_admitted_capacity_for_replacements_and_later_edges() {
        let layout = empty_layout();
        let policy = AsciiResourcePolicy::unbounded();
        let mut resources = ResourceContext::new(policy);
        let routes = [route(11, 0)];
        let mut scene = SceneOccupancy::try_new_for_routes(
            &layout,
            64,
            2,
            &mut resources,
            AsciiExecution::for_test(&policy),
        )
        .unwrap();
        scene
            .commit_route(
                0,
                &routes[0],
                routes[0]
                    .plan
                    .terminal_candidate(MarkerEndpoint::Start, "flowchart")
                    .unwrap(),
                routes[0]
                    .plan
                    .terminal_candidate(MarkerEndpoint::End, "flowchart")
                    .unwrap(),
                &mut resources,
            )
            .unwrap();
        let before = resources.layout_work_used();
        let rebuilt = scene
            .rebuild_for_routes(&routes, &[], &mut resources, "flowchart")
            .unwrap();
        assert_eq!(rebuilt.route_capacity, 64);
        assert!(rebuilt.route_bounds.capacity() >= 64);
        assert!(rebuilt.target_ports.capacity() >= 64);
        assert!(rebuilt.source_ports.capacity() >= 64);
        assert!(rebuilt.markers.capacity() >= 128);
        assert!(resources.layout_work_used() - before >= 64 * 8);
    }

    #[test]
    fn removing_one_contributor_preserves_shared_cells_with_dense_indices() {
        let layout = empty_layout();
        let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let mut routes = vec![route(11, 0), route(77, 0)];
        let mut scene =
            SceneOccupancy::try_new(&routes, &layout, &mut resources, "flowchart").unwrap();
        assert_eq!(
            scene.route_cells[&CanvasCoord { x: 2, y: 0 }].owners.len(),
            2
        );
        scene = scene
            .rebuild_for_routes(&routes, &[0], &mut resources, "flowchart")
            .unwrap();
        routes.remove(0);
        assert_eq!(routes[0].owner.canonical_edge_index, 77);
        assert!(scene.route_bounds.capacity() >= 2);
        assert_eq!(scene.route_cells.len(), 6);
        assert_eq!(scene.route_bounds.len(), 1);
        assert!(
            scene
                .route_cells
                .values()
                .all(|cell| cell.owners.len() == 1 && cell.owners[0].route_index == 0)
        );
        assert!(
            scene
                .terminal_claims
                .values()
                .flatten()
                .all(|claim| claim.route_index == 0)
        );
        assert_eq!(
            scene.target_port_usage(routes[0].plan.attachments.unwrap().end),
            1
        );
    }

    #[test]
    fn removal_clears_markers_label_gaps_and_ports_without_changing_finalized_plans() {
        let layout = empty_layout();
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
        let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let mut routes = vec![route(11, 20), route(77, 0)];
        for route in &mut routes {
            route.plan = route
                .plan
                .clone()
                .with_markers(
                    GraphEdgeMarker::Circle,
                    GraphEdgeMarker::Point,
                    &charset,
                    "flowchart",
                )
                .unwrap();
            let start = route.plan.cells[0].coord;
            let end = route.plan.cells[5].coord;
            let mut label = PlannedRouteLabel::new(
                RoutedLabelText::new("tag").unwrap(),
                RoutedLabelPlacement::new(1, start.y + 3, 3),
            );
            label.anchor = LabelAnchor::Segment {
                start,
                end,
                route_segment: None,
            };
            route.plan.labels.push(label);
        }
        let mut scene =
            SceneOccupancy::try_new(&routes, &layout, &mut resources, "flowchart").unwrap();
        super::super::marker::allocate_marker_berths(
            &mut routes,
            &mut scene,
            &charset,
            &mut resources,
            "flowchart",
        )
        .unwrap();
        super::super::labels::allocate_route_label_placements(
            &mut routes,
            &mut scene,
            &mut resources,
            "flowchart",
        )
        .unwrap();
        assert_eq!(scene.markers.len(), 4);
        assert!(scene.labels.contains(&CanvasCoord { x: 1, y: 21 }));
        assert!(
            scene
                .terminal_claims
                .values()
                .flatten()
                .any(|claim| !claim.primary)
        );
        let plan = routes[1].plan.clone();
        scene.begin_blocker_collection(&resources).unwrap();
        scene = scene
            .rebuild_for_routes(&routes, &[0], &mut resources, "flowchart")
            .unwrap();
        let removed_ports = routes.remove(0).plan.attachments.unwrap();
        assert!(scene.blockers.borrow().is_none());
        assert_eq!(routes[0].plan, plan);
        assert_eq!(scene.markers.len(), 2);
        assert!(scene.markers.contains_key(&CanvasCoord { x: 5, y: 0 }));
        assert!(scene.labels.contains(&CanvasCoord { x: 1, y: 1 }));
        assert!(scene.labels.iter().all(|coord| coord.y < 4));
        assert!(scene.terminal_claims.iter().all(
            |(coord, claims)| coord.y == 0 && claims.iter().all(|claim| claim.route_index == 0)
        ));
        assert_eq!(scene.source_port_usage(removed_ports.start), 0);
        assert_eq!(scene.target_port_usage(removed_ports.end), 0);
        assert_eq!(
            scene.source_port_usage(routes[0].plan.attachments.unwrap().start),
            1
        );
        assert_eq!(scene.occupied_extent(), CanvasCoord { x: 5, y: 3 });
        assert_eq!(scene.label_owners.len(), 1);
        assert_eq!(scene.label_owners[0].canonical_edge_index, 77);
        let owner =
            crate::graph::routing::RouteOwner::for_test(100, "other-source", "other-target");
        scene.begin_blocker_collection(&resources).unwrap();
        for coord in [CanvasCoord { x: 1, y: 1 }, CanvasCoord { x: 5, y: 0 }] {
            assert!(
                !scene
                    .route_cell_is_clear(
                        &routes,
                        &owner,
                        None,
                        coord,
                        crate::graph::routing::cell::DIR_LEFT
                            | crate::graph::routing::cell::DIR_RIGHT,
                        &mut resources,
                    )
                    .unwrap()
            );
        }
        let evidence = scene.take_blocker_collection(&resources).unwrap().unwrap();
        assert_eq!(evidence.blockers.len(), 1);
        assert_eq!(evidence.blockers[0].canonical_edge_index, 77);
        assert!(
            evidence.blockers[0].contains(crate::graph::routing::conflict::ReservationKind::Label)
        );
        assert!(
            evidence.blockers[0].contains(crate::graph::routing::conflict::ReservationKind::Marker)
        );
    }

    #[test]
    fn relocated_markers_keep_their_original_terminal_claim_windows_on_replay() {
        let layout = empty_layout();
        let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
        let mut resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let mut routes = vec![route_with_length(11, 0, 21), route_with_length(77, 0, 21)];
        for route in &mut routes {
            route.plan = route
                .plan
                .clone()
                .with_markers(
                    GraphEdgeMarker::Open,
                    GraphEdgeMarker::Point,
                    &charset,
                    "flowchart",
                )
                .unwrap();
        }
        let mut scene =
            SceneOccupancy::try_new(&routes, &layout, &mut resources, "flowchart").unwrap();
        super::super::marker::allocate_marker_berths(
            &mut routes,
            &mut scene,
            &charset,
            &mut resources,
            "flowchart",
        )
        .unwrap();
        assert_eq!(
            routes[1]
                .plan
                .terminal_candidate(MarkerEndpoint::End, "flowchart")
                .unwrap()
                .coord
                .x,
            19
        );
        let rebuilt = scene
            .rebuild_for_routes(&routes, &[], &mut resources, "flowchart")
            .unwrap();
        assert_eq!(rebuilt.terminal_claims, scene.terminal_claims);
        assert!(
            !rebuilt
                .terminal_claims
                .contains_key(&CanvasCoord { x: 12, y: 0 })
        );
        assert!(
            rebuilt.terminal_claims[&CanvasCoord { x: 20, y: 0 }]
                .iter()
                .all(|claim| claim.primary)
        );
        for x in 12..21 {
            let coord = CanvasCoord { x, y: 0 };
            let owner = crate::graph::routing::RouteOwner::for_test(99, "other", "endpoint");
            assert_eq!(
                scene
                    .route_cell_is_clear(
                        &routes,
                        &owner,
                        None,
                        coord,
                        crate::graph::routing::cell::DIR_LEFT
                            | crate::graph::routing::cell::DIR_RIGHT,
                        &mut resources
                    )
                    .unwrap(),
                rebuilt
                    .route_cell_is_clear(
                        &routes,
                        &owner,
                        None,
                        coord,
                        crate::graph::routing::cell::DIR_LEFT
                            | crate::graph::routing::cell::DIR_RIGHT,
                        &mut resources
                    )
                    .unwrap(),
            );
        }
    }

    #[test]
    fn quota_and_cancellation_failures_preserve_geometry_and_spent_work() {
        let layout = empty_layout();
        for cancel in [false, true] {
            let policy = if cancel {
                AsciiResourcePolicy::unbounded()
            } else {
                AsciiResourcePolicy::unbounded()
                    .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 64)
                    .unwrap()
            };
            let mut resources = ResourceContext::new(policy);
            let scene =
                SceneOccupancy::try_new(&[route(11, 0)], &layout, &mut resources, "flowchart")
                    .unwrap();
            let before = geometry(&scene);
            if cancel {
                scene.control.cancel_after_checkpoints(10);
            } else {
                resources
                    .charge_layout_work(64 - resources.layout_work_used() - 16)
                    .unwrap();
            }
            let spent = resources.layout_work_used();
            let error = scene
                .rebuild_for_routes(&[route(11, 0)], &[], &mut resources, "flowchart")
                .unwrap_err();
            if cancel {
                assert!(matches!(error, AsciiError::Cancelled(cancelled)
                    if cancelled.phase == OperationPhase::Layout));
            } else {
                assert!(matches!(error, AsciiError::ResourceLimitExceeded(details)
                    if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits));
            }
            assert_eq!(geometry(&scene), before);
            assert!(resources.layout_work_used() > spent);
        }
    }
}

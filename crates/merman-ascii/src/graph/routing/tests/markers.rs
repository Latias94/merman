use super::*;

#[test]
fn conflicting_marker_owners_receive_independent_terminal_berths() {
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
    let mut routes = vec![
        PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Circle, 3), 0),
        PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Cross, 3), 1),
    ];
    let mut resources = unbounded_resources();

    allocate_test_marker_berths(&mut routes, &charset, &mut resources).unwrap();

    let first = routes[0]
        .plan
        .materialized_marker_cell(MarkerEndpoint::End, "flowchart")
        .unwrap()
        .unwrap();
    let second = routes[1]
        .plan
        .materialized_marker_cell(MarkerEndpoint::End, "flowchart")
        .unwrap()
        .unwrap();
    assert_ne!(first.coord, second.coord);
    assert_eq!((first.ch, second.ch), ('o', 'x'));
}

#[test]
fn relocated_marker_suppresses_only_its_route_local_terminal_tail() {
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
    let mut routes = vec![
        PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Circle, 3), 0),
        PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Cross, 3), 1),
    ];
    let mut resources = unbounded_resources();
    let options = AsciiRenderOptions::ascii();
    let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
    let mut occupancy =
        SceneOccupancy::try_new(&routes, &graph_layout, &mut resources, "flowchart").unwrap();

    allocate_marker_berths(
        &mut routes,
        &mut occupancy,
        &charset,
        &mut resources,
        "flowchart",
    )
    .unwrap();

    let mut local_canvas = RawCanvas::with_width_profile(3, 1, TerminalWidthProfile::Unicode);
    let mut local_cells = RouteCells::new();
    paint_route_plan(
        &mut RouteDrawing::new(&mut local_canvas, &mut local_cells),
        &routes[1].plan,
    )
    .unwrap();
    assert!(routes[1].plan.is_cell_suppressed(PlannedCellId::new(2)));
    assert!(!routes[0].plan.is_cell_suppressed(PlannedCellId::new(2)));
    let shared_owners = &occupancy
        .route_cells
        .get(&CanvasCoord { x: 2, y: 0 })
        .expect("the unsuppressed route must retain the shared coordinate")
        .owners;
    assert!(
        shared_owners
            .iter()
            .any(|owner| { owner.route_index == 0 && owner.cell == PlannedCellId::new(2) })
    );
    assert!(
        !shared_owners
            .iter()
            .any(|owner| { owner.route_index == 1 && owner.cell == PlannedCellId::new(2) })
    );
    assert_eq!(local_canvas.get(0, 0), Some('-'));
    assert_eq!(local_canvas.get(1, 0), Some('x'));
    assert_eq!(
        local_canvas.get(2, 0),
        Some(' '),
        "the relocated marker must terminate its own route instead of producing -x-"
    );

    let mut shared_canvas = RawCanvas::with_width_profile(3, 1, TerminalWidthProfile::Unicode);
    let mut shared_cells = RouteCells::new();
    let mut drawing = RouteDrawing::new(&mut shared_canvas, &mut shared_cells);
    for route in &routes {
        route.paint_body(&mut drawing).unwrap();
    }
    for route in &routes {
        route.paint_markers(&mut drawing).unwrap();
    }
    assert_eq!(shared_canvas.get(0, 0), Some('-'));
    assert_eq!(shared_canvas.get(1, 0), Some('x'));
    assert_eq!(
        shared_canvas.get(2, 0),
        Some('o'),
        "suppressing one route's terminal tail must preserve the other route's marker ownership"
    );
}

#[test]
fn relocated_marker_suppresses_a_three_cell_mixed_body_tail() {
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
    let mut plan = RoutePlan::new(
        vec![
            planned_cell(0, 0, '-', PlannedRouteCellKind::EdgeLine),
            planned_cell(1, 0, '-', PlannedRouteCellKind::RouteCell),
            planned_cell(2, 0, '-', PlannedRouteCellKind::EdgeLine),
            planned_cell(3, 0, '-', PlannedRouteCellKind::RouteCell),
            planned_cell(4, 0, '-', PlannedRouteCellKind::EdgeLine),
        ],
        Vec::new(),
        MarkerAnchors::new(
            MarkerAnchor::new(PlannedCellId::new(0), StepDirection::Left),
            MarkerAnchor::new(PlannedCellId::new(4), StepDirection::Right),
        ),
    )
    .with_marker_requests(GraphEdgeMarker::Open, GraphEdgeMarker::Point, "flowchart")
    .unwrap();
    let mut resources = unbounded_resources();
    let candidates = plan
        .marker_candidates(MarkerEndpoint::End, "flowchart", &mut resources)
        .unwrap();
    let candidate = candidates[3];

    plan.materialize_marker_at(MarkerEndpoint::End, candidate, &charset, "flowchart")
        .unwrap();

    let mut canvas = RawCanvas::with_width_profile(5, 1, TerminalWidthProfile::Unicode);
    let mut route_cells = RouteCells::new();
    paint_route_plan(&mut RouteDrawing::new(&mut canvas, &mut route_cells), &plan).unwrap();
    assert_eq!(
        candidate.terminal_tail(),
        &[
            PlannedCellId::new(4),
            PlannedCellId::new(3),
            PlannedCellId::new(2),
        ]
    );
    for suppressed in candidate.terminal_tail() {
        assert!(plan.is_cell_suppressed(*suppressed));
    }
    assert_eq!(canvas.get(0, 0), Some('-'));
    assert_eq!(canvas.get(1, 0), Some('>'));
    assert_eq!(canvas.get(2, 0), Some(' '));
    assert_eq!(canvas.get(3, 0), Some(' '));
    assert_eq!(canvas.get(4, 0), Some(' '));
}

#[test]
fn parallel_markers_occupy_a_contiguous_terminal_chain() {
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
    let mut routes = vec![
        PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Circle, 4), 0),
        PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Cross, 4), 1),
        PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Point, 4), 2),
    ];
    let mut resources = unbounded_resources();

    allocate_test_marker_berths(&mut routes, &charset, &mut resources).unwrap();

    let markers = routes
        .iter()
        .map(|route| {
            let marker = route
                .plan
                .materialized_marker_cell(MarkerEndpoint::End, "flowchart")
                .unwrap()
                .unwrap();
            (marker.coord.x, marker.ch)
        })
        .collect::<Vec<_>>();
    assert_eq!(markers, [(3, 'o'), (2, 'x'), (1, '>')]);
    for (route_index, route) in routes.iter().enumerate() {
        let marker_x = markers[route_index].0;
        assert!(
            route
                .plan
                .active_cells()
                .map(|(_, cell)| cell)
                .all(|cell| cell.coord.x <= marker_x),
            "each route must terminate at its independently allocated marker"
        );
    }
}

#[test]
fn identical_marker_glyphs_from_different_edges_do_not_coalesce() {
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
    let mut routes = vec![
        PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Circle, 3), 0),
        PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Circle, 3), 1),
    ];
    let mut resources = unbounded_resources();

    allocate_test_marker_berths(&mut routes, &charset, &mut resources).unwrap();

    let coords = routes
        .iter()
        .map(|route| {
            route
                .plan
                .materialized_marker_cell(MarkerEndpoint::End, "flowchart")
                .unwrap()
                .unwrap()
                .coord
        })
        .collect::<Vec<_>>();
    assert_ne!(coords[0], coords[1]);
}

#[test]
fn marker_berth_exhaustion_is_explicit() {
    let charset = GraphCharset::for_options(&AsciiRenderOptions::ascii());
    let mut routes = vec![
        PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Circle, 1), 0),
        PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Cross, 1), 1),
    ];
    let mut resources = unbounded_resources();

    let error = allocate_test_marker_berths(&mut routes, &charset, &mut resources)
        .expect_err("one terminal cell cannot host two marker owners");

    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "independent endpoint marker berth exhausted",
        }
    );
}

#[test]
fn route_score_rejects_an_interior_marker_berth_past_an_unrelated_crossing() {
    let options = AsciiRenderOptions::ascii();
    let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
    let marker_route = marker_request_plan_at_y(GraphEdgeMarker::Point, 3, 1);
    let crossing_route = vertical_route_plan_at_x(2);
    let existing_routes = vec![PreparedRoute::for_test_with_endpoints(
        crossing_route,
        0,
        "other-a",
        "other-b",
    )];
    let marker_route = PreparedRoute::for_test_with_endpoints(marker_route, 1, "source", "target");
    let mut resources = unbounded_resources();
    let occupancy =
        SceneOccupancy::try_new(&existing_routes, &graph_layout, &mut resources, "flowchart")
            .unwrap();

    let score = occupancy
        .score_route(
            &existing_routes,
            &marker_route.plan,
            &marker_route.owner,
            &mut resources,
            "flowchart",
        )
        .unwrap();

    assert!(
        score.is_none(),
        "an unrelated crossing at the terminal must force another route candidate"
    );
}

#[test]
fn route_score_rejects_a_later_crossing_of_a_reserved_primary_terminal() {
    let options = AsciiRenderOptions::ascii();
    let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
    let existing_routes = vec![PreparedRoute::for_test_with_endpoints(
        marker_request_plan_at_y(GraphEdgeMarker::Point, 3, 1),
        0,
        "source",
        "target",
    )];
    let crossing_route = PreparedRoute::for_test_with_endpoints(
        vertical_route_plan_at_x(2),
        1,
        "other-a",
        "other-b",
    );
    let fallback_route = PreparedRoute::for_test_with_endpoints(
        vertical_route_plan_at_x(3),
        1,
        "other-a",
        "other-b",
    );
    let mut resources = unbounded_resources();
    let occupancy =
        SceneOccupancy::try_new(&existing_routes, &graph_layout, &mut resources, "flowchart")
            .unwrap();

    let crossing_score = occupancy
        .score_route(
            &existing_routes,
            &crossing_route.plan,
            &crossing_route.owner,
            &mut resources,
            "flowchart",
        )
        .unwrap();
    let fallback_score = occupancy
        .score_route(
            &existing_routes,
            &fallback_route.plan,
            &fallback_route.owner,
            &mut resources,
            "flowchart",
        )
        .unwrap();

    assert!(
        crossing_score.is_none(),
        "a later route must not overwrite an already committed primary terminal corridor"
    );
    assert!(
        fallback_score.is_some(),
        "rejecting the crossing candidate must leave a clear fallback admissible"
    );
}

#[test]
fn route_score_allows_an_incident_route_to_share_a_terminal_corridor() {
    let options = AsciiRenderOptions::ascii();
    let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
    let existing_routes = vec![PreparedRoute::for_test_with_endpoints(
        marker_request_plan_at_y(GraphEdgeMarker::Point, 3, 1),
        0,
        "source",
        "shared",
    )];
    let incident_route =
        PreparedRoute::for_test_with_endpoints(vertical_route_plan_at_x(2), 1, "shared", "other");
    let mut resources = unbounded_resources();
    let occupancy =
        SceneOccupancy::try_new(&existing_routes, &graph_layout, &mut resources, "flowchart")
            .unwrap();

    let score = occupancy
        .score_route(
            &existing_routes,
            &incident_route.plan,
            &incident_route.owner,
            &mut resources,
            "flowchart",
        )
        .unwrap();

    assert!(
        score.is_some(),
        "routes incident to the same authored endpoint may share its terminal corridor"
    );
}

#[test]
fn marker_candidate_preflight_charges_every_shared_owner_claim_scan() {
    const ROUTE_COUNT: usize = 4;
    const EXPECTED_WORK: usize = ROUTE_COUNT * 2 + ROUTE_COUNT * (ROUTE_COUNT + 1) / 2;

    let options = AsciiRenderOptions::ascii();
    let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
    let existing_routes = (0..ROUTE_COUNT)
        .map(|route_index| {
            PreparedRoute::for_test_with_endpoints(
                marker_request_plan(GraphEdgeMarker::Point, 3),
                route_index,
                "source",
                "target",
            )
        })
        .collect::<Vec<_>>();
    let candidate_route = PreparedRoute::for_test_with_endpoints(
        marker_request_plan(GraphEdgeMarker::Point, 3),
        ROUTE_COUNT,
        "source",
        "target",
    );
    let candidate = candidate_route
        .plan
        .terminal_candidate(MarkerEndpoint::End, "flowchart")
        .expect("test marker route should expose a terminal candidate");
    let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
    let mut setup_resources = ResourceContext::new(unbounded);
    let occupancy = SceneOccupancy::try_new(
        &existing_routes,
        &graph_layout,
        &mut setup_resources,
        "flowchart",
    )
    .expect("shared terminal routes should build occupancy");

    let mut measured_resources = ResourceContext::new(unbounded);
    let disposition = occupancy
        .marker_candidate_disposition_before_commit(
            &existing_routes,
            &candidate_route.owner,
            MarkerEndpoint::End,
            candidate,
            &mut measured_resources,
        )
        .expect("unbounded shared-terminal scan should pass");
    assert_eq!(disposition, MarkerCandidateDisposition::Available);
    assert_eq!(measured_resources.layout_work_used(), EXPECTED_WORK);

    let exact_policy = unbounded
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, EXPECTED_WORK)
        .expect("exact marker-scan work limit should be valid");
    let mut exact_resources = ResourceContext::new(exact_policy);
    assert_eq!(
        occupancy
            .marker_candidate_disposition_before_commit(
                &existing_routes,
                &candidate_route.owner,
                MarkerEndpoint::End,
                candidate,
                &mut exact_resources,
            )
            .expect("exact marker-scan work should pass"),
        MarkerCandidateDisposition::Available
    );
    assert_eq!(exact_resources.layout_work_used(), EXPECTED_WORK);

    let below_policy = unbounded
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, EXPECTED_WORK - 1)
        .expect("max-minus-one marker-scan work limit should be valid");
    let mut below_resources = ResourceContext::new(below_policy);
    let error = occupancy
        .marker_candidate_disposition_before_commit(
            &existing_routes,
            &candidate_route.owner,
            MarkerEndpoint::End,
            candidate,
            &mut below_resources,
        )
        .expect_err("max-minus-one marker-scan work should reject");
    assert!(matches!(
        error,
        AsciiError::ResourceLimitExceeded(details)
            if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                && details.actual == EXPECTED_WORK
                && details.max == EXPECTED_WORK - 1
    ));
}

#[test]
fn route_score_does_not_reserve_an_unused_secondary_marker_corridor() {
    let options = AsciiRenderOptions::ascii();
    let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
    let existing_routes = vec![PreparedRoute::for_test_with_endpoints(
        marker_request_plan_at_y(GraphEdgeMarker::Circle, 4, 1),
        0,
        "source",
        "target",
    )];
    let crossing_route = PreparedRoute::for_test_with_endpoints(
        vertical_route_plan_at_x(2),
        1,
        "other-a",
        "other-b",
    );
    let mut resources = unbounded_resources();
    let occupancy =
        SceneOccupancy::try_new(&existing_routes, &graph_layout, &mut resources, "flowchart")
            .unwrap();

    let crossing_score = occupancy
        .score_route(
            &existing_routes,
            &crossing_route.plan,
            &crossing_route.owner,
            &mut resources,
            "flowchart",
        )
        .unwrap();

    assert!(
        crossing_score.is_some(),
        "an unused fallback marker berth must not reject an otherwise valid crossing route"
    );
}

#[test]
fn marker_allocation_rejects_an_interior_berth_past_an_unrelated_crossing() {
    let options = AsciiRenderOptions::ascii();
    let charset = GraphCharset::for_options(&options);
    let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
    let marker_route = marker_request_plan_at_y(GraphEdgeMarker::Point, 3, 1);
    let crossing_route = vertical_route_plan_at_x(2);
    let mut routes = vec![
        PreparedRoute::for_test_with_endpoints(marker_route, 0, "source", "target"),
        PreparedRoute::for_test_with_endpoints(crossing_route, 1, "other-a", "other-b"),
    ];
    let mut resources = unbounded_resources();
    let mut occupancy =
        SceneOccupancy::try_new(&routes, &graph_layout, &mut resources, "flowchart").unwrap();

    let error = allocate_marker_berths(
        &mut routes,
        &mut occupancy,
        &charset,
        &mut resources,
        "flowchart",
    )
    .expect_err("an endpoint marker must not move behind an unrelated crossing");

    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "independent endpoint marker berth exhausted",
        }
    );
}

#[test]
fn marker_allocation_does_not_jump_over_an_unrelated_interior_crossing() {
    let options = AsciiRenderOptions::ascii();
    let charset = GraphCharset::for_options(&options);
    let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
    let crossing_route = RoutePlan::new_without_markers_for_test(
        (0..=2)
            .map(|y| planned_cell(2, y, '|', PlannedRouteCellKind::EdgeLine))
            .collect(),
        Vec::new(),
    );
    let mut routes = vec![
        PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Circle, 4), 0),
        PreparedRoute::for_test(marker_request_plan(GraphEdgeMarker::Cross, 4), 1),
        PreparedRoute::for_test_with_endpoints(crossing_route, 2, "other-a", "other-b"),
    ];
    let mut resources = unbounded_resources();
    let mut occupancy =
        SceneOccupancy::try_new(&routes, &graph_layout, &mut resources, "flowchart").unwrap();

    let error = allocate_marker_berths(
        &mut routes,
        &mut occupancy,
        &charset,
        &mut resources,
        "flowchart",
    )
    .expect_err("a marker must not jump past an unrelated crossing to a deeper berth");

    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "independent endpoint marker berth exhausted",
        }
    );
}

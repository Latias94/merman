use super::*;

#[test]
fn edge_canvas_extent_accounts_for_boundary_grid_path_label_width() {
    let options = AsciiRenderOptions::ascii();
    let charset = GraphCharset::for_options(&options);
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_node("y", "Y");
    graph.add_group_with_style(
        "one",
        "LR Group",
        Some(GraphDirection::LeftRight),
        vec!["a".to_string(), "b".to_string()],
        Default::default(),
    );
    graph.add_edge("a", "b");
    graph.add_edge_with_attrs(
        "b",
        "y",
        GraphEdgeAttrs {
            label: Some("boundary label with enough width".to_string()),
            ..Default::default()
        },
    );
    let graph_layout = layout_graph(&graph, &options);
    let edge = &graph.edges[1];
    let from =
        endpoint_layout(&graph_layout, &edge.from, &charset).expect("source layout should exist");
    let to =
        endpoint_layout(&graph_layout, &edge.to, &charset).expect("target layout should exist");
    let plan = plan_edge_route(EdgeRouteRequest {
        graph: &graph,
        graph_layout: &graph_layout,
        edges: &graph.edges,
        from: &from,
        to: &to,
        edge_index: 1,
        edge,
        charset: &charset,
    })
    .expect("boundary route should plan");
    let label = plan.labels.first().expect("boundary route should label");
    let (required_width, _) = label.placement.canvas_extent();

    let scene = prepare_route_scene(&graph, &graph_layout, &graph.edges, &charset)
        .expect("boundary scene should render");
    let (edge_width, _) = scene.canvas_extent();

    assert!(
        edge_width >= required_width,
        "edge canvas extent should reserve boundary label width {required_width}, got {edge_width}; plan: {plan:?}"
    );
}

#[test]
fn canonical_edge_order_ignores_generated_ids_and_is_permutation_stable() {
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_edge_with_attrs(
        "a",
        "b",
        GraphEdgeAttrs {
            label: Some("zeta".to_string()),
            end_marker: GraphEdgeMarker::Circle,
            ..Default::default()
        },
    );
    graph.add_edge_with_attrs(
        "a",
        "b",
        GraphEdgeAttrs {
            label: Some("alpha".to_string()),
            end_marker: GraphEdgeMarker::Cross,
            ..Default::default()
        },
    );

    let mut generated_left = graph.edges[0].clone();
    generated_left.id = Some("generated-z".to_string());
    generated_left.is_user_defined_id = false;
    let mut generated_right = generated_left.clone();
    generated_right.id = Some("generated-a".to_string());
    assert_eq!(
        compare_edges(&generated_left, &generated_right),
        Ordering::Equal
    );

    let mut explicit_left = generated_left.clone();
    explicit_left.id = Some("edge-a".to_string());
    explicit_left.is_user_defined_id = true;
    let mut explicit_right = generated_left;
    explicit_right.id = Some("edge-b".to_string());
    explicit_right.is_user_defined_id = true;
    assert_eq!(
        compare_edges(&explicit_left, &explicit_right),
        Ordering::Less
    );

    let forward = graph.edges.clone();
    let mut reversed = forward.clone();
    reversed.reverse();
    let mut forward_resources = unbounded_resources();
    let mut reversed_resources = unbounded_resources();
    let canonical_forward = canonicalize_edges(&forward, &mut forward_resources).unwrap();
    let canonical_reversed = canonicalize_edges(&reversed, &mut reversed_resources).unwrap();

    assert_eq!(
        canonical_forward.values.len(),
        canonical_reversed.values.len()
    );
    for (left, right) in canonical_forward
        .values
        .iter()
        .zip(canonical_reversed.values.iter())
    {
        assert_eq!(compare_edges(left, right), Ordering::Equal);
    }
}

#[test]
fn prepared_route_scene_is_stable_across_edge_permutations() {
    let options = AsciiRenderOptions::ascii();
    let charset = GraphCharset::for_options(&options);
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    for id in ["a", "b", "c"] {
        graph.add_node(id, id.to_uppercase());
    }
    graph.add_edge_with_attrs(
        "a",
        "b",
        GraphEdgeAttrs {
            end_marker: GraphEdgeMarker::Open,
            ..Default::default()
        },
    );
    graph.add_edge_with_attrs(
        "b",
        "c",
        GraphEdgeAttrs {
            end_marker: GraphEdgeMarker::Open,
            ..Default::default()
        },
    );
    graph.add_edge_with_attrs(
        "a",
        "c",
        GraphEdgeAttrs {
            end_marker: GraphEdgeMarker::Open,
            ..Default::default()
        },
    );
    let graph_layout = layout_graph(&graph, &options);
    let forward = prepare_route_scene(&graph, &graph_layout, &graph.edges, &charset)
        .expect("canonical edge order should route the forward declaration");
    let mut permuted_edges = graph.edges.clone();
    permuted_edges.rotate_left(1);
    let permuted = prepare_route_scene(&graph, &graph_layout, &permuted_edges, &charset)
        .expect("canonical edge order should route the edge permutation");

    assert_eq!(
        route_scene_signature(&forward),
        route_scene_signature(&permuted)
    );
}

#[test]
fn prepared_route_scene_prefers_a_clear_direct_route_over_extra_marker_berths() {
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        let charset = GraphCharset::for_options(&options);
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        graph.add_node("a", "A");
        graph.add_node("b", "B");
        graph.add_edge("a", "b");
        let graph_layout = layout_graph(&graph, &options);

        let scene = prepare_route_scene(&graph, &graph_layout, &graph.edges, &charset)
            .expect("a clear direct edge should produce a prepared route scene");
        let route = &scene.routes[0].plan;
        let source = graph_layout
            .nodes
            .iter()
            .find(|node| node.id == "a")
            .expect("source node should be laid out");

        assert!(
            route
                .cells
                .iter()
                .all(|cell| cell.coord.y == source.center_y()),
            "marker relocation capacity must not outrank a shorter collision-free route: {route:?}"
        );
    }
}

#[test]
fn prepare_route_scene_reports_missing_endpoint_layouts_before_painting() {
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_edge("a", "missing");
    let options = AsciiRenderOptions::ascii();
    let graph_layout = layout_graph(&graph, &options);
    let charset = GraphCharset::for_options(&options);

    let error = match prepare_route_scene(&graph, &graph_layout, &graph.edges, &charset) {
        Ok(_) => panic!("scene planning should fail on missing endpoint layouts"),
        Err(error) => error,
    };

    assert_eq!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "edges with missing endpoint layouts",
        }
    );
}

#[test]
fn prepare_route_scene_tracks_canvas_extent_for_each_route_plan() {
    let options = AsciiRenderOptions::ascii();
    let charset = GraphCharset::for_options(&options);
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_node("c", "C");
    graph.add_edge("a", "b");
    graph.add_edge_with_attrs(
        "b",
        "c",
        GraphEdgeAttrs {
            label: Some("wide label".to_string()),
            ..Default::default()
        },
    );
    let graph_layout = layout_graph(&graph, &options);

    let scene = prepare_route_scene(&graph, &graph_layout, &graph.edges, &charset)
        .expect("supported graph should produce a prepared route scene");

    let mut expected_width = 0;
    let mut expected_height = 0;
    for route in &scene.routes {
        let (plan_width, plan_height) = route.plan.canvas_extent();
        expected_width = expected_width.max(plan_width);
        expected_height = expected_height.max(plan_height);
    }

    assert_eq!(scene.canvas_extent(), (expected_width, expected_height));
}

#[test]
fn overlapping_route_cells_accept_exact_work_limit_and_reject_max_minus_one() {
    let options = AsciiRenderOptions::ascii();
    let charset = GraphCharset::for_options(&options);
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    let open_edge = GraphEdgeAttrs {
        end_marker: GraphEdgeMarker::Open,
        ..Default::default()
    };
    graph.add_edge_with_attrs("a", "b", open_edge.clone());
    graph.add_edge_with_attrs("a", "b", open_edge);
    let graph_layout = layout_graph(&graph, &options);
    let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
    let mut measured_resources = ResourceContext::new(unbounded);
    prepare_route_scene_with_resources(
        &graph,
        &graph_layout,
        &graph.edges,
        &charset,
        &mut measured_resources,
    )
    .expect("overlapping routes should plan");
    let exact = measured_resources.layout_work_used();
    assert!(exact > 1, "test graph should plan overlapping route cells");

    let exact_policy = unbounded
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact)
        .expect("exact layout-work limit should be valid");
    let mut exact_resources = ResourceContext::new(exact_policy);
    prepare_route_scene_with_resources(
        &graph,
        &graph_layout,
        &graph.edges,
        &charset,
        &mut exact_resources,
    )
    .expect("exact planned-cell work limit should pass");

    let below_policy = unbounded
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact - 1)
        .expect("max-minus-one layout-work limit should be valid");
    let mut below_resources = ResourceContext::new(below_policy);
    let error = match prepare_route_scene_with_resources(
        &graph,
        &graph_layout,
        &graph.edges,
        &charset,
        &mut below_resources,
    ) {
        Ok(_) => panic!("max-minus-one planned-cell work limit should fail"),
        Err(error) => error,
    };
    let AsciiError::ResourceLimitExceeded(details) = error else {
        panic!("expected a layout-work resource error, got {error:?}");
    };
    assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
    assert_eq!(details.actual, exact);
    assert_eq!(details.max, exact - 1);
}

#[test]
fn scene_geometry_accepts_exact_work_limit_and_rejects_max_minus_one() {
    let options = AsciiRenderOptions::ascii();
    let graph_layout = grouped_graph_layout(&options);
    let routes = Vec::new();
    let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
    let mut measured_resources = ResourceContext::new(unbounded);
    let measured =
        SceneOccupancy::try_new(&routes, &graph_layout, &mut measured_resources, "flowchart")
            .expect("group border and title geometry should precompute");
    assert!(
        measured
            .protected
            .iter()
            .any(|geometry| geometry.kind == ProtectedKind::GroupTitle)
    );
    let exact = measured_resources.layout_work_used();
    assert!(exact > 1);

    let exact_policy = unbounded
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact)
        .expect("exact scene-geometry work limit should be valid");
    let mut exact_resources = ResourceContext::new(exact_policy);
    SceneOccupancy::try_new(&routes, &graph_layout, &mut exact_resources, "flowchart")
        .expect("exact scene-geometry work limit should pass");
    assert_eq!(exact_resources.layout_work_used(), exact);

    let below_policy = unbounded
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact - 1)
        .expect("max-minus-one scene-geometry work limit should be valid");
    let mut below_resources = ResourceContext::new(below_policy);
    let error = SceneOccupancy::try_new(&routes, &graph_layout, &mut below_resources, "flowchart")
        .expect_err("max-minus-one scene-geometry work limit should fail");
    let AsciiError::ResourceLimitExceeded(details) = error else {
        panic!("expected a layout-work resource error, got {error:?}");
    };
    assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
    assert_eq!(details.actual, exact);
    assert_eq!(details.max, exact - 1);
}

#[test]
fn route_extent_reports_checked_cell_geometry_overflow() {
    let plan = RoutePlan::new_without_markers_for_test(
        vec![planned_cell(
            usize::MAX,
            0,
            '-',
            PlannedRouteCellKind::RouteCell,
        )],
        Vec::new(),
    );
    let resources = ResourceContext::new(AsciiResourcePolicy::for_profile(
        ResourceProfile::UnboundedForTrustedInput,
    ));

    let error = plan
        .canvas_extent_with_resources(&resources)
        .expect_err("overflowing route-cell geometry should fail");
    let AsciiError::ResourceLimitExceeded(details) = error else {
        panic!("expected a grid resource error, got {error:?}");
    };
    assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
    assert_eq!(details.actual, usize::MAX);
}

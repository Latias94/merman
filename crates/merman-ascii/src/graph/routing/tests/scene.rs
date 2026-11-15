use super::*;

fn assert_allocated_geometry(route: &PreparedRoute) {
    let branch = route
        .owner
        .allocated_branch
        .expect("a shared route owns its allocation");
    assert_eq!(route.plan.attachments, Some(branch.attachments));
    for (_, cell) in route.plan.active_cells() {
        let coord = cell.coord;
        assert!(
            coord == branch.attachments.start.contact
                || branch.points.windows(2).any(|pair| {
                    let (from, to) = (pair[0], pair[1]);
                    (from.x == to.x
                        && coord.x == from.x
                        && (from.y.min(to.y)..=from.y.max(to.y)).contains(&coord.y))
                        || (from.y == to.y
                            && coord.y == from.y
                            && (from.x.min(to.x)..=from.x.max(to.x)).contains(&coord.x))
                }),
            "route {} -> {} left its allocated channel at {coord:?}",
            route.owner.from,
            route.owner.to
        );
    }
}

#[test]
fn allocated_branch_trunks_share_only_their_declared_source_group() {
    let graph = AsciiGraph::new(GraphDirection::LeftRight);
    let layout = layout_graph(&graph, &AsciiRenderOptions::ascii());
    let mut resources = unbounded_resources();
    let plan = RoutePlan::new_without_markers_for_test(
        (0..11)
            .map(|x| planned_cell(x, 0, '-', PlannedRouteCellKind::EdgeLine))
            .collect(),
        Vec::new(),
    );
    let mut route = PreparedRoute::for_test_with_endpoints(plan, 2, "source", "first");
    route.owner.set_source_group_for_test(Some(2));
    let routes = [route];
    let scene = SceneOccupancy::try_new(&routes, &layout, &mut resources, "flowchart").unwrap();
    let coord = CanvasCoord { x: 5, y: 0 };
    let horizontal = cell::DIR_LEFT | cell::DIR_RIGHT;
    let vertical = cell::DIR_UP | cell::DIR_DOWN;
    let mut compatible = RouteOwner::for_test(3, "source", "second");
    compatible.set_source_group_for_test(Some(2));
    assert!(
        scene
            .route_cell_is_clear(
                &routes,
                &compatible,
                None,
                coord,
                horizontal,
                &mut resources
            )
            .unwrap()
    );
    assert_eq!(
        scene
            .route_cell_congestion(&routes, &compatible, None, coord, &mut resources)
            .unwrap(),
        0
    );
    for group in [None, Some(7)] {
        let mut unrelated = RouteOwner::for_test(7, "different", "third");
        unrelated.set_source_group_for_test(group);
        assert!(
            !scene
                .route_cell_is_clear(&routes, &unrelated, None, coord, horizontal, &mut resources)
                .unwrap()
        );
        assert!(
            scene
                .route_cell_is_clear(&routes, &unrelated, None, coord, vertical, &mut resources)
                .unwrap()
        );
    }
    compatible.set_source_group_for_test(None);
    assert!(
        !scene
            .route_cell_is_clear(
                &routes,
                &compatible,
                None,
                coord,
                horizontal,
                &mut resources
            )
            .unwrap(),
        "endpoint text alone cannot assign a branch group"
    );
}

#[test]
fn complete_fanouts_honor_joint_channels_in_both_orientations() {
    for direction in [GraphDirection::LeftRight, GraphDirection::TopDown] {
        for count in [2, 8, 12, 16] {
            for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
                let mut graph = AsciiGraph::new(direction);
                graph.add_node("source", "Dispatcher");
                for index in 0..count {
                    let id = format!("target-{index:02}");
                    graph.add_node(&id, format!("Job {index}"));
                    graph.add_edge("source", &id);
                }
                let layout = layout_graph(&graph, &options);
                let charset = GraphCharset::for_options(&options);
                let mut resources = unbounded_resources();
                let scene = prepare_route_scene_with_resources(
                    &graph,
                    &layout,
                    &graph.edges,
                    &charset,
                    &mut resources,
                )
                .unwrap();
                assert_eq!(scene.routes.len(), count);
                let start = scene.routes[0].plan.attachments.unwrap().start;
                for route in &scene.routes {
                    assert_eq!(
                        route.owner.shared_channel_group(),
                        Some(0),
                        "a local path must not dismantle the joint channel: {direction:?}, {count}, {}",
                        route.owner.to
                    );
                    let ports = route.plan.attachments.unwrap();
                    assert_eq!(ports.start, start);
                    assert_eq!(
                        ports.start.outward,
                        if direction == GraphDirection::LeftRight {
                            StepDirection::Right
                        } else {
                            StepDirection::Down
                        }
                    );
                    assert_eq!(ports.end.outward, ports.start.outward.opposite());
                    assert_allocated_geometry(route);
                    assert!(
                        route
                            .plan
                            .materialized_marker_cell(MarkerEndpoint::End, "flowchart")
                            .unwrap()
                            .is_some()
                    );
                }
            }
        }
    }
}

#[test]
fn complete_fanins_keep_private_sources_and_share_one_target_marker() {
    for direction in [GraphDirection::LeftRight, GraphDirection::TopDown] {
        for count in [2, 8, 12, 16] {
            for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
                let mut graph = AsciiGraph::new(direction);
                graph.add_node("target", "Collector");
                for index in 0..count {
                    let id = format!("source-{index:02}");
                    graph.add_node(&id, format!("Job {index}"));
                    graph.add_edge_with_attrs(
                        &id,
                        "target",
                        GraphEdgeAttrs {
                            start_marker: if index % 2 == 0 {
                                GraphEdgeMarker::Circle
                            } else {
                                GraphEdgeMarker::Cross
                            },
                            end_marker: GraphEdgeMarker::Point,
                            ..GraphEdgeAttrs::default()
                        },
                    );
                }
                let layout = layout_graph(&graph, &options);
                let mut resources = unbounded_resources();
                let scene = prepare_route_scene_with_resources(
                    &graph,
                    &layout,
                    &graph.edges,
                    &GraphCharset::for_options(&options),
                    &mut resources,
                )
                .unwrap();
                assert_eq!(scene.routes.len(), count);
                let end = scene.routes[0].plan.attachments.unwrap().end;
                let marker = scene.routes[0]
                    .plan
                    .materialized_marker_cell(MarkerEndpoint::End, "flowchart")
                    .unwrap()
                    .unwrap()
                    .coord;
                let mut sources = std::collections::HashSet::new();
                for route in &scene.routes {
                    assert!(
                        route.owner.allocated_branch.is_some(),
                        "a merge needs a joint channel: {direction:?}, {count}, {}",
                        route.owner.from
                    );
                    let ports = route.plan.attachments.unwrap();
                    assert_eq!(ports.end, end);
                    assert!(sources.insert(ports.start.contact));
                    assert_eq!(ports.end.outward, ports.start.outward.opposite());
                    assert_allocated_geometry(route);
                    let cell = route
                        .plan
                        .materialized_marker_cell(MarkerEndpoint::End, "flowchart")
                        .unwrap()
                        .unwrap();
                    assert_eq!(cell.coord, marker);
                    assert!(
                        route
                            .plan
                            .materialized_marker_cell(MarkerEndpoint::Start, "flowchart")
                            .unwrap()
                            .is_some()
                    );
                }
            }
        }
    }
}

#[test]
fn an_unrelated_transverse_crossing_does_not_dismantle_a_joint_channel() {
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    for (id, text) in [
        ("a", "A"),
        ("b", "B"),
        ("source", "S"),
        ("t0", "T0"),
        ("t1", "T1"),
    ] {
        graph.add_node(id, text);
    }
    for (from, to) in [("a", "b"), ("source", "t0"), ("source", "t1")] {
        graph.add_edge(from, to);
    }
    let options = AsciiRenderOptions::ascii();
    let mut layout = layout_graph(&graph, &options);
    for (node, (x, y, width)) in
        layout
            .nodes
            .iter_mut()
            .zip([(2, 6, 5), (31, 6, 5), (6, 12, 7), (22, 0, 7), (22, 24, 7)])
    {
        node.x = x;
        node.y = y;
        node.width = width;
        node.height = 5;
    }
    layout.grid_aligned = false;
    let mut resources = unbounded_resources();
    let scene = prepare_route_scene_with_resources(
        &graph,
        &layout,
        &graph.edges,
        &GraphCharset::for_options(&options),
        &mut resources,
    )
    .unwrap();
    let crossing = CanvasCoord { x: 14, y: 8 };
    let unrelated = scene
        .routes
        .iter()
        .find(|route| route.owner.from == "a")
        .unwrap();
    assert!(
        unrelated
            .plan
            .active_cells()
            .any(|(_, cell)| cell.coord == crossing)
    );
    let upper = scene
        .routes
        .iter()
        .find(|route| route.owner.to == "t0")
        .unwrap();
    assert!(
        upper
            .plan
            .active_cells()
            .any(|(_, cell)| cell.coord == crossing)
    );
    for route in scene
        .routes
        .iter()
        .filter(|route| route.owner.from == "source")
    {
        assert_eq!(route.owner.shared_channel_group(), Some(1));
        assert_allocated_geometry(route);
    }
}

#[test]
fn aligned_scenes_do_not_allocate_or_charge_an_unused_feedback_order() {
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_edge("a", "b");
    let layout = layout_graph(&graph, &AsciiRenderOptions::ascii());
    assert!(layout.grid_aligned);
    let resources = ResourceContext::new(
        AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap(),
    );
    assert!(
        plan_route_order(&graph, &layout, &graph.edges, &resources)
            .unwrap()
            .is_none()
    );
    assert_eq!(resources.layout_work_used(), 0);
}

#[test]
fn invisible_parallel_constraints_do_not_inflate_feedback_planning_work() {
    let mut graph = AsciiGraph::new(GraphDirection::TopDown);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_edge("a", "b");
    let mut layout = layout_graph(&graph, &AsciiRenderOptions::ascii());
    layout.grid_aligned = false;
    graph.edges[0].stroke = GraphEdgeStroke::Invisible;
    graph.edges.resize(10_000, graph.edges[0].clone());
    let resources = ResourceContext::new(
        AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 20_000)
            .unwrap(),
    );
    assert!(
        plan_route_order(&graph, &layout, &graph.edges, &resources)
            .unwrap()
            .unwrap()
            .is_empty()
    );
}

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
    assert_eq!(
        label.placement.width(),
        "boundary label with enough width".len()
    );

    let scene = prepare_route_scene(&graph, &graph_layout, &graph.edges, &charset)
        .expect("boundary scene should render");
    let (edge_width, _) = scene.canvas_extent();
    let required_width = scene
        .routes
        .iter()
        .flat_map(|route| &route.plan.labels)
        .map(|label| {
            label
                .placement
                .canvas_extent_for_lines(label.line_count())
                .0
        })
        .max()
        .expect("prepared boundary label should exist");

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
fn branching_chain_keeps_converging_edges_on_distinct_target_ports() {
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        for direction in [GraphDirection::LeftRight, GraphDirection::TopDown] {
            for compound in [false, true] {
                let charset = GraphCharset::for_options(&options);
                let graph = branching_chain_graph(compound, direction);
                let layout = layout_graph(&graph, &options);
                let scene = prepare_route_scene(&graph, &layout, &graph.edges, &charset)
                    .unwrap_or_else(|error| {
                        panic!("all five routes must survive: {error}\n{layout:#?}")
                    });
                assert_eq!(scene.routes.len(), 5);
                let contacts = scene
                    .routes
                    .iter()
                    .filter(|route| route.owner.to == "C")
                    .map(|route| route.plan.attachments.unwrap().end.contact)
                    .collect::<rustc_hash::FxHashSet<_>>();
                assert_eq!(
                    contacts.len(),
                    3,
                    "independent incoming edges should use distinct ports before stacking markers"
                );
                for group in &layout.groups {
                    for route in &scene.routes {
                        for (_, cell) in route.plan.active_cells() {
                            let inside = group.x < cell.coord.x
                                && cell.coord.x < group.right()
                                && group.y < cell.coord.y
                                && cell.coord.y < group.bottom();
                            if group.id == "foreign" {
                                assert!(!inside, "routes must not enter an unrelated frame");
                            } else {
                                assert!(
                                    inside,
                                    "internal routes must stay inside every common frame"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

fn branching_chain_graph(compound: bool, direction: GraphDirection) -> AsciiGraph {
    let mut graph = AsciiGraph::new(direction);
    for id in ["A", "B", "C", "D"] {
        graph.add_node(id, id);
    }
    for (from, to) in [("A", "B"), ("A", "C"), ("B", "C"), ("B", "D"), ("D", "C")] {
        graph.add_edge(from, to);
    }
    if compound {
        for (id, members) in [
            ("inner", vec!["A", "B", "C", "D"]),
            ("outer", vec!["inner", "foreign"]),
            ("foreign", vec![]),
        ] {
            graph.add_group_with_style(
                id,
                id,
                Some(direction),
                members.into_iter().map(str::to_owned).collect(),
                crate::graph::model::GraphGroupStyle::default(),
            );
        }
    }
    graph
}

#[test]
fn terminal_port_repair_observes_exact_work_and_limit_minus_one() {
    for graph in [
        branching_chain_graph(false, GraphDirection::LeftRight),
        branching_chain_graph(true, GraphDirection::LeftRight),
        group_endpoint_merge_graph(),
        group_self_loop_graph(),
        concurrent_region_skip_graph(),
    ] {
        let options = AsciiRenderOptions::unicode();
        let charset = GraphCharset::for_options(&options);
        let layout = layout_graph(&graph, &options);
        let policy = AsciiResourcePolicy::unbounded();
        let mut measured = ResourceContext::new(policy);
        let expected = prepare_route_scene_with_resources(
            &graph,
            &layout,
            &graph.edges,
            &charset,
            &mut measured,
        )
        .unwrap()
        .materialize(&measured)
        .unwrap();
        let exact = measured.layout_work_used();
        let mut admitted = ResourceContext::new(
            policy
                .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact)
                .unwrap(),
        );
        let actual = prepare_route_scene_with_resources(
            &graph,
            &layout,
            &graph.edges,
            &charset,
            &mut admitted,
        )
        .unwrap()
        .materialize(&admitted)
        .unwrap();
        assert_eq!(
            route_scene_signature(&actual),
            route_scene_signature(&expected)
        );
        assert_eq!(admitted.layout_work_used(), exact);
        let mut rejected = ResourceContext::new(
            policy
                .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact - 1)
                .unwrap(),
        );
        let error = match prepare_route_scene_with_resources(
            &graph,
            &layout,
            &graph.edges,
            &charset,
            &mut rejected,
        )
        .and_then(|plan| plan.materialize(&rejected))
        {
            Ok(_) => panic!("terminal port repair must charge all attempted work"),
            Err(error) => error,
        };
        assert!(matches!(error, AsciiError::ResourceLimitExceeded(details)
        if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits && details.actual == exact));
    }
}

fn group_endpoint_merge_graph() -> AsciiGraph {
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    for id in ["A", "B", "C", "member", "Z"] {
        graph.add_node(id, id);
    }
    graph.add_group_with_style(
        "group",
        "Group",
        None,
        vec!["member".into()],
        crate::graph::model::GraphGroupStyle::default(),
    );
    for from in ["A", "B", "C"] {
        graph.add_edge_with_attrs(
            from,
            "group",
            GraphEdgeAttrs {
                label: Some(format!("via {from}")),
                ..GraphEdgeAttrs::default()
            },
        );
    }
    graph.add_edge("group", "Z");
    graph
}

fn group_self_loop_graph() -> AsciiGraph {
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    graph.add_node("A", "A");
    graph.add_node("Z", "Z");
    graph.add_group_with_style(
        "group",
        "Buffer",
        None,
        vec!["A".into()],
        crate::graph::model::GraphGroupStyle::default(),
    );
    for label in ["first", "second", "third"] {
        graph.add_edge_with_attrs(
            "group",
            "group",
            GraphEdgeAttrs {
                label: Some(label.into()),
                ..GraphEdgeAttrs::default()
            },
        );
    }
    graph.add_edge("group", "Z");
    graph
}

fn concurrent_region_skip_graph() -> AsciiGraph {
    use crate::graph::model::{GraphGroupKind, GraphGroupStyle};
    let mut graph = AsciiGraph::new_for_diagram("state", GraphDirection::TopDown);
    let mut regions = Vec::new();
    for id in ["A", "B", "C", "D"] {
        graph.add_node(id, id);
        let region = format!("region-{id}");
        graph.add_group_with_kind_and_style(
            &region,
            "",
            None,
            vec![id.into()],
            GraphGroupKind::Divider,
            GraphGroupStyle::default(),
        );
        regions.push(region);
    }
    graph.add_group_with_style(
        "outer",
        "RUNNING",
        None,
        regions,
        GraphGroupStyle::default(),
    );
    for (from, to, label) in [("A", "D", "send"), ("D", "A", "retry")] {
        graph.add_edge_with_attrs(
            from,
            to,
            GraphEdgeAttrs {
                label: Some(label.into()),
                ..GraphEdgeAttrs::default()
            },
        );
    }
    graph
}

#[test]
fn group_self_loops_use_distinct_return_contacts_and_keep_transit_outside() {
    let graph = group_self_loop_graph();
    let options = AsciiRenderOptions::unicode();
    let charset = GraphCharset::for_options(&options);
    let layout = layout_graph(&graph, &options);
    let scene = prepare_route_scene(&graph, &layout, &graph.edges, &charset).unwrap();
    let group = layout
        .groups
        .iter()
        .find(|group| group.id == "group")
        .unwrap();
    let loops: Vec<_> = scene
        .routes
        .iter()
        .filter(|route| route.owner.to == "group")
        .collect();
    let contacts: rustc_hash::FxHashSet<_> = loops
        .iter()
        .map(|route| route.plan.attachments.unwrap().end.contact)
        .collect();
    assert_eq!(contacts.len(), 3);
    for route in loops {
        let attachments = route.plan.attachments.unwrap();
        assert_ne!(attachments.start.contact, attachments.end.contact);
        for (_, cell) in route.plan.active_cells() {
            assert!(
                !(group.x < cell.coord.x
                    && cell.coord.x < group.right()
                    && group.y < cell.coord.y
                    && cell.coord.y < group.bottom())
            );
        }
    }
}

#[test]
fn group_merge_uses_distinct_border_contacts_without_entering_its_member() {
    let graph = group_endpoint_merge_graph();
    let options = AsciiRenderOptions::unicode();
    let charset = GraphCharset::for_options(&options);
    let layout = layout_graph(&graph, &options);
    let scene = prepare_route_scene(&graph, &layout, &graph.edges, &charset).unwrap();
    let group = layout
        .groups
        .iter()
        .find(|group| group.id == "group")
        .unwrap();
    let contacts = scene
        .routes
        .iter()
        .filter(|route| route.owner.to == "group")
        .map(|route| route.plan.attachments.unwrap().end.contact)
        .collect::<rustc_hash::FxHashSet<_>>();
    assert_eq!(contacts.len(), 3);
    for route in &scene.routes {
        for (_, cell) in route.plan.active_cells() {
            assert!(
                !(group.x < cell.coord.x
                    && cell.coord.x < group.right()
                    && group.y < cell.coord.y
                    && cell.coord.y < group.bottom()),
                "a container endpoint must keep external transit out of its interior"
            );
        }
    }
}

#[test]
fn terminal_cell_search_observes_cancellation_inside_the_frontier() {
    let options = AsciiRenderOptions::unicode();
    let charset = GraphCharset::for_options(&options);
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_edge("a", "b");
    let layout = layout_graph(&graph, &options);
    let policy = AsciiResourcePolicy::unbounded();
    let mut resources = ResourceContext::new(policy);
    let occupancy = SceneOccupancy::try_new(&[], &layout, &mut resources, "flowchart").unwrap();
    let from = endpoint_layout(&layout, "a", &charset).unwrap();
    let to = endpoint_layout(&layout, "b", &charset).unwrap();
    let request = EdgeRouteRequest {
        graph: &graph,
        graph_layout: &layout,
        edges: &graph.edges,
        from: &from,
        to: &to,
        edge_index: 0,
        edge: &graph.edges[0],
        charset: &charset,
    };
    let owner = RouteOwner::for_test(0, "a", "b");
    let control = OperationControl::new();
    control.cancel_after_checkpoints(3);
    let error = match terminal::plan_terminal_route(
        request,
        path::RouteObstacles {
            scene: &occupancy,
            routes: &[],
            owner: &owner,
        },
        None,
        None,
        &mut resources,
        AsciiExecution::new(&control, &policy),
    ) {
        Ok(_) => panic!("the terminal search must cancel before returning a route"),
        Err(error) => error,
    };
    assert!(matches!(error, AsciiError::Cancelled(cancelled)
        if cancelled.phase == OperationPhase::Layout && cancelled.reason == CancelReason::Requested));
}

#[test]
fn terminal_cell_search_crosses_owned_frames_inside_the_common_parent() {
    let options = AsciiRenderOptions::unicode();
    let charset = GraphCharset::for_options(&options);
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    for id in ["A", "B", "X"] {
        graph.add_node(id, id);
    }
    graph.add_edge("A", "B");
    for (id, members) in [
        ("left", vec!["A"]),
        ("right", vec!["B"]),
        ("foreign", vec!["X"]),
        ("outer", vec!["left", "right", "foreign"]),
    ] {
        graph.add_group_with_style(
            id,
            id,
            Some(GraphDirection::LeftRight),
            members.into_iter().map(str::to_owned).collect(),
            crate::graph::model::GraphGroupStyle::default(),
        );
    }
    let layout = layout_graph(&graph, &options);
    let policy = AsciiResourcePolicy::unbounded();
    let mut resources = ResourceContext::new(policy);
    let topology = GraphGroupTopology::try_new(&graph, &mut resources).unwrap();
    let occupancy = SceneOccupancy::try_new(&[], &layout, &mut resources, "flowchart").unwrap();
    let from = endpoint_layout(&layout, "A", &charset).unwrap();
    let to = endpoint_layout(&layout, "B", &charset).unwrap();
    let request = EdgeRouteRequest {
        graph: &graph,
        graph_layout: &layout,
        edges: &graph.edges,
        from: &from,
        to: &to,
        edge_index: 0,
        edge: &graph.edges[0],
        charset: &charset,
    };
    let mut owner = RouteOwner::for_test(0, "A", "B");
    owner.boundary_group_indices =
        route_boundary_group_indices(&graph, Some(&topology), &graph.edges[0], &mut resources)
            .unwrap();
    let (_, plan) = terminal::plan_terminal_route(
        request,
        path::RouteObstacles {
            scene: &occupancy,
            routes: &[],
            owner: &owner,
        },
        None,
        None,
        &mut resources,
        AsciiExecution::for_test(&policy),
    )
    .unwrap()
    .expect("terminal ports must connect nodes across their owned sibling frames");
    for group in &layout.groups {
        let mut crossings = 0;
        for (_, cell) in plan.active_cells() {
            let coord = cell.coord;
            let within_x = (group.x..=group.right()).contains(&coord.x);
            let within_y = (group.y..=group.bottom()).contains(&coord.y);
            let inside = group.x < coord.x
                && coord.x < group.right()
                && group.y < coord.y
                && coord.y < group.bottom();
            match group.id.as_str() {
                "outer" => assert!(inside, "the route must stay inside its common parent"),
                "foreign" => assert!(!(within_x && within_y), "an unrelated frame is reserved"),
                _ => {
                    if within_y && (coord.x == group.x || coord.x == group.right()) {
                        assert!(group.y < coord.y && coord.y < group.bottom());
                        assert_eq!(cell.directions, cell::DIR_LEFT | cell::DIR_RIGHT);
                        crossings += 1;
                    } else if within_x && (coord.y == group.y || coord.y == group.bottom()) {
                        assert!(group.x < coord.x && coord.x < group.right());
                        assert_eq!(cell.directions, cell::DIR_UP | cell::DIR_DOWN);
                        crossings += 1;
                    }
                }
            }
        }
        if matches!(group.id.as_str(), "left" | "right") {
            assert_eq!(crossings, 1, "each owned frame must be crossed once");
        }
    }
}

#[test]
fn obstacle_aware_feedback_preserves_contacts_and_exact_work_limit() {
    let options = AsciiRenderOptions::unicode();
    let charset = GraphCharset::for_options(&options);
    let graph = obstacle_aware_feedback_graph();
    let layout = layout_graph(&graph, &options);
    let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
    let mut measured = ResourceContext::new(unbounded);
    let scene =
        prepare_route_scene_with_resources(&graph, &layout, &graph.edges, &charset, &mut measured)
            .expect("the feedback must find a safe path outside its opposite branch");
    let feedback = scene
        .routes
        .iter()
        .find(|route| route.owner.from == "D" && route.owner.to == "B")
        .unwrap();
    let outgoing = scene
        .routes
        .iter()
        .find(|route| route.owner.from == "B" && route.owner.to == "D")
        .unwrap();
    let attachments = feedback.plan.attachments.unwrap();
    let target = layout.nodes.iter().find(|node| node.id == "B").unwrap();
    let marker = feedback
        .plan
        .terminal_candidate(MarkerEndpoint::End, "flowchart")
        .unwrap();
    assert!(attachments.end.allows_escape(marker.coord));
    assert_eq!(marker.point_direction, attachments.end.outward.opposite());
    assert!(
        (target.x..=target.right()).contains(&attachments.end.contact.x)
            && (target.y..=target.bottom()).contains(&attachments.end.contact.y),
        "the feedback must enter Review, not its sibling or source"
    );
    let feedback_transit = feedback
        .plan
        .active_cells()
        .map(|(_, cell)| cell.coord)
        .filter(|coord| *coord != attachments.start.contact && *coord != attachments.end.contact)
        .collect::<rustc_hash::FxHashSet<_>>();
    assert!(
        outgoing
            .plan
            .active_cells()
            .all(|(_, cell)| !feedback_transit.contains(&cell.coord)),
        "opposite authored edges must not share a transit path"
    );

    let exact = measured.layout_work_used();
    let exact_policy = unbounded
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact)
        .unwrap();
    let mut admitted = ResourceContext::new(exact_policy);
    prepare_route_scene_with_resources(&graph, &layout, &graph.edges, &charset, &mut admitted)
        .expect("exact work must include every obstacle-aware frontier and parent admission");
    assert_eq!(admitted.layout_work_used(), exact);

    let below_policy = unbounded
        .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact - 1)
        .unwrap();
    let mut rejected = ResourceContext::new(below_policy);
    let error = match prepare_route_scene_with_resources(
        &graph,
        &layout,
        &graph.edges,
        &charset,
        &mut rejected,
    ) {
        Ok(_) => panic!("one work unit below the complete obstacle-aware scene must fail"),
        Err(error) => error,
    };
    assert!(matches!(error, AsciiError::ResourceLimitExceeded(details)
        if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits && details.actual == exact));
}

#[test]
fn scene_aware_grid_cancellation_precedes_projected_cell_work_debit() {
    use path::{Port, PortPair, RouteObstacles, route_grid_path_avoiding_obstacles};

    let options = AsciiRenderOptions::unicode();
    let charset = GraphCharset::for_options(&options);
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    graph.add_node("a", "A");
    graph.add_node("b", "B");
    graph.add_edge("a", "b");
    let layout = layout_graph(&graph, &options);
    let from = layout.nodes.iter().find(|node| node.id == "a").unwrap();
    let to = layout.nodes.iter().find(|node| node.id == "b").unwrap();
    assert!(layout.groups.is_empty());
    assert_eq!(from.grid.y, to.grid.y);
    assert!(from.grid.x + 3 < to.grid.x);

    let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
    let owner = RouteOwner::for_test(0, "a", "b");
    let mut work_at_projection = None;
    // Measure cancellation at the first projected cell, repeat with exactly that budget,
    // then keep the scene active to verify that the pending protected-geometry debit fails.
    for cancel_scene in [true, true, false] {
        let policy = work_at_projection.map_or(unbounded, |work| {
            unbounded
                .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, work)
                .unwrap()
        });
        let scene_control = OperationControl::new();
        let search_control = OperationControl::new();
        let mut setup = ResourceContext::new(unbounded);
        let occupancy = SceneOccupancy::try_new_for_routes(
            &layout,
            1,
            options
                .flowchart_layout()
                .graph_policy()
                .edge_label_lane_radius,
            &mut setup,
            AsciiExecution::new(&scene_control, &unbounded),
        )
        .unwrap();
        assert_eq!(occupancy.protected.len(), layout.nodes.len());
        let mut resources = ResourceContext::new(policy);
        // Coarse search and resource accounting stay active independently of the scene.
        // With no groups, the scene control is first checked inside projected cell inspection.
        if cancel_scene {
            scene_control.cancel_after_checkpoints(0);
        }
        let error = route_grid_path_avoiding_obstacles(
            &layout,
            from,
            to,
            PortPair::new(Port::Right, Port::Left),
            &charset,
            RouteObstacles {
                scene: &occupancy,
                routes: &[],
                owner: &owner,
            },
            &mut resources,
            AsciiExecution::new(&search_control, &policy),
        )
        .expect_err("projected-cell failures must propagate instead of returning a grid path");

        assert_eq!(scene_control.is_cancelled(), cancel_scene);
        assert!(!search_control.is_cancelled());
        let work = resources.layout_work_used();
        if cancel_scene {
            assert!(matches!(error, AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested));
        } else {
            assert!(matches!(error, AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.max == work
                    && details.actual == work + occupancy.protected.len()));
        }
        if let Some(expected) = work_at_projection {
            assert_eq!(work, expected);
        } else {
            assert!(work > 0, "coarse search must run before scene cancellation");
            work_at_projection = Some(work);
        }
    }
}

fn obstacle_aware_feedback_graph() -> AsciiGraph {
    let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
    graph.add_node("A", "Start");
    graph.add_node_with_shape_and_style(
        "B",
        "Review?",
        GraphNodeShape::Diamond,
        Default::default(),
    );
    graph.add_node("C", "Merge");
    graph.add_node("D", "Fix");
    graph.add_edge("A", "B");
    for (target, label) in [("C", "ok"), ("D", "ng")] {
        graph.add_edge_with_attrs(
            "B",
            target,
            GraphEdgeAttrs {
                label: Some(label.to_string()),
                ..Default::default()
            },
        );
    }
    graph.add_edge("D", "B");
    graph
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

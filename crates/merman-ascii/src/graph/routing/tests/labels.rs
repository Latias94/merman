use super::*;

#[test]
fn route_scene_relocates_labels_that_cover_endpoint_markers() {
    let options = AsciiRenderOptions::ascii();
    let charset = GraphCharset::for_options(&options);
    let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
    let mut plan = marker_request_plan(GraphEdgeMarker::Point, 6);
    plan.labels.push(PlannedRouteLabel::new(
        RoutedLabelText::new("enter").unwrap(),
        RoutedLabelPlacement::new(1, 0, 5),
    ));
    let mut routes = vec![PreparedRoute::for_test(plan, 0)];
    let mut resources = unbounded_resources();
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
    allocate_route_label_placements(&mut routes, &mut occupancy, &mut resources, "flowchart")
        .expect("route labels should move to an independent local lane");

    let marker = routes[0]
        .plan
        .materialized_marker_cell(MarkerEndpoint::End, "flowchart")
        .unwrap()
        .unwrap();
    let label = &routes[0].plan.labels[0];
    let label_rect = OccupiedRect::try_new(
        label.placement.x(),
        label.placement.y(),
        label.placement.width(),
        label.text.line_count(),
        &resources,
    )
    .unwrap();
    assert!(!label_rect.contains(marker.coord.x, marker.coord.y));
}

#[test]
fn route_label_relocates_instead_of_covering_an_unrelated_route() {
    let options = AsciiRenderOptions::ascii();
    let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
    let mut label = PlannedRouteLabel::new(
        RoutedLabelText::new("tag").unwrap(),
        RoutedLabelPlacement::new(1, 2, 3),
    );
    label.anchor = LabelAnchor::Segment {
        start: CanvasCoord { x: 0, y: 2 },
        end: CanvasCoord { x: 4, y: 2 },
        route_segment: Some(PlannedRouteSegment::Direct),
    };
    let labeled_route = RoutePlan::new_without_markers_for_test(
        (0..=4)
            .map(|x| planned_cell(x, 2, '-', PlannedRouteCellKind::EdgeLine))
            .collect(),
        vec![label],
    );
    let crossing_route = RoutePlan::new_without_markers_for_test(
        (0..=4)
            .map(|y| planned_cell(2, y, '|', PlannedRouteCellKind::EdgeLine))
            .collect(),
        Vec::new(),
    );
    let mut routes = vec![
        PreparedRoute::for_test_with_endpoints(labeled_route, 0, "a", "b"),
        PreparedRoute::for_test_with_endpoints(crossing_route, 1, "c", "d"),
    ];
    let mut resources = unbounded_resources();

    allocate_test_label_placements(&mut routes, &graph_layout, &mut resources).unwrap();

    let label = &routes[0].plan.labels[0];
    let footprint = OccupiedRect::try_new(
        label.placement.x(),
        label.placement.y(),
        label.placement.width(),
        label.text.line_count(),
        &resources,
    )
    .unwrap();
    assert!(
        routes[1]
            .plan
            .cells
            .iter()
            .all(|cell| !footprint.contains(cell.coord.x, cell.coord.y))
    );
    assert_eq!(
        label.anchor,
        LabelAnchor::Segment {
            start: CanvasCoord { x: 0, y: 2 },
            end: CanvasCoord { x: 4, y: 2 },
            route_segment: Some(PlannedRouteSegment::Direct),
        }
    );
}

#[test]
fn label_can_cover_only_its_own_host_segment() {
    let options = AsciiRenderOptions::ascii();
    let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
    let anchor = LabelAnchor::Segment {
        start: CanvasCoord { x: 0, y: 2 },
        end: CanvasCoord { x: 4, y: 2 },
        route_segment: Some(PlannedRouteSegment::Direct),
    };
    let plan = RoutePlan::new_without_markers_for_test(
        vec![
            planned_cell(0, 2, '-', PlannedRouteCellKind::EdgeLine),
            planned_cell(1, 2, '-', PlannedRouteCellKind::EdgeLine),
            planned_cell(2, 2, '-', PlannedRouteCellKind::EdgeLine),
            planned_cell(3, 2, '-', PlannedRouteCellKind::EdgeLine),
            planned_cell(4, 2, '-', PlannedRouteCellKind::EdgeLine),
            planned_cell(2, 1, '|', PlannedRouteCellKind::EdgeLine),
        ],
        Vec::new(),
    );
    let routes = vec![PreparedRoute::for_test(plan, 0)];
    let mut resources = unbounded_resources();
    let occupancy =
        SceneOccupancy::try_new(&routes, &graph_layout, &mut resources, "flowchart").unwrap();
    let host_cell = OccupiedRect::try_new(2, 2, 1, 1, &resources).unwrap();
    let non_host_cell = OccupiedRect::try_new(2, 1, 1, 1, &resources).unwrap();

    assert!(
        occupancy
            .label_candidate_is_clear(0, anchor, host_cell, &mut resources)
            .unwrap()
    );
    assert!(
        !occupancy
            .label_candidate_is_clear(0, anchor, non_host_cell, &mut resources)
            .unwrap()
    );
}

#[test]
fn route_scene_relocates_labels_away_from_nodes_groups_and_other_labels() {
    let options = AsciiRenderOptions::ascii();
    let graph_layout = simple_graph_layout(&options);
    let node = &graph_layout.nodes[0];
    let node_route = labeled_plan(node.x, node.y, "node");
    let mut node_routes = vec![PreparedRoute::for_test(node_route, 0)];
    let mut resources = unbounded_resources();
    allocate_test_label_placements(&mut node_routes, &graph_layout, &mut resources)
        .expect("route labels should move away from node geometry");
    let label = &node_routes[0].plan.labels[0];
    let label_rect = OccupiedRect::try_new(
        label.placement.x(),
        label.placement.y(),
        label.placement.width(),
        label.text.line_count(),
        &resources,
    )
    .unwrap();
    let node_rect =
        OccupiedRect::try_new(node.x, node.y, node.width, node.height, &resources).unwrap();
    assert!(!label_rect.intersects(node_rect));

    let group_layout = grouped_graph_layout(&options);
    let group = &group_layout.groups[0];
    let group_route = labeled_plan(group.x, group.y, "group");
    let mut group_routes = vec![PreparedRoute::for_test(group_route, 0)];
    let mut resources = unbounded_resources();
    allocate_test_label_placements(&mut group_routes, &group_layout, &mut resources)
        .expect("route labels should move away from group borders and titles");
    let label = &group_routes[0].plan.labels[0];
    let label_rect = OccupiedRect::try_new(
        label.placement.x(),
        label.placement.y(),
        label.placement.width(),
        label.text.line_count(),
        &resources,
    )
    .unwrap();
    let occupancy =
        SceneOccupancy::try_new(&group_routes, &group_layout, &mut resources, "flowchart").unwrap();
    assert!(
        occupancy
            .protected
            .iter()
            .all(|protected| !protected.shape.intersects(label_rect))
    );

    let mut routes = vec![
        PreparedRoute::for_test(labeled_plan(100, 100, "first"), 0),
        PreparedRoute::for_test(labeled_plan(100, 100, "second"), 1),
    ];
    let mut resources = unbounded_resources();
    allocate_test_label_placements(&mut routes, &graph_layout, &mut resources)
        .expect("duplicate label anchors should receive independent local lanes");
    let footprints = routes
        .iter()
        .map(|route| {
            let label = &route.plan.labels[0];
            OccupiedRect::try_new(
                label.placement.x(),
                label.placement.y(),
                label.placement.width(),
                label.text.line_count(),
                &resources,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(!footprints[0].intersects(footprints[1]));
}

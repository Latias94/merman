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
        label.line_count(),
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
        label.line_count(),
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
            .label_candidate_is_clear(0, &routes[0].owner, anchor, host_cell, &mut resources)
            .unwrap()
    );
    assert!(
        !occupancy
            .label_candidate_is_clear(0, &routes[0].owner, anchor, non_host_cell, &mut resources)
            .unwrap()
    );
}

#[test]
fn label_cannot_move_across_an_unrelated_parallel_route() {
    let options = AsciiRenderOptions::ascii();
    let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
    let anchor = LabelAnchor::Segment {
        start: CanvasCoord { x: 2, y: 6 },
        end: CanvasCoord { x: 18, y: 6 },
        route_segment: Some(PlannedRouteSegment::Direct),
    };
    let routes: Vec<_> = [6, 4]
        .into_iter()
        .enumerate()
        .map(|(index, y)| {
            PreparedRoute::for_test_with_endpoints(
                RoutePlan::new_without_markers_for_test(
                    (2..=18)
                        .map(|x| planned_cell(x, y, '-', PlannedRouteCellKind::EdgeLine))
                        .collect(),
                    Vec::new(),
                ),
                index,
                if index == 0 { "a" } else { "c" },
                if index == 0 { "b" } else { "d" },
            )
        })
        .collect();
    let mut resources = unbounded_resources();
    let occupancy =
        SceneOccupancy::try_new(&routes, &graph_layout, &mut resources, "flowchart").unwrap();
    let detached = OccupiedRect::try_new(5, 3, 5, 1, &resources).unwrap();
    let adjacent = OccupiedRect::try_new(5, 5, 5, 1, &resources).unwrap();

    assert!(
        !occupancy
            .label_candidate_is_clear(0, &routes[0].owner, anchor, detached, &mut resources)
            .unwrap(),
        "a label separated from its host by another route would identify the wrong edge"
    );
    assert!(
        occupancy
            .label_candidate_is_clear(0, &routes[0].owner, anchor, adjacent, &mut resources)
            .unwrap()
    );
}

#[test]
fn committed_label_protects_the_gap_to_its_host_from_later_routes() {
    let options = AsciiRenderOptions::ascii();
    let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
    let mut label = PlannedRouteLabel::new(
        RoutedLabelText::new("tag").unwrap(),
        RoutedLabelPlacement::new(5, 4, 3),
    );
    label.anchor = LabelAnchor::Segment {
        start: CanvasCoord { x: 2, y: 6 },
        end: CanvasCoord { x: 18, y: 6 },
        route_segment: Some(PlannedRouteSegment::Direct),
    };
    let mut routes = vec![PreparedRoute::for_test_with_endpoints(
        RoutePlan::new_without_markers_for_test(
            (2..=18)
                .map(|x| planned_cell(x, 6, '-', PlannedRouteCellKind::EdgeLine))
                .collect(),
            vec![label],
        ),
        0,
        "a",
        "b",
    )];
    let mut resources = unbounded_resources();
    let mut occupancy =
        SceneOccupancy::try_new(&routes, &graph_layout, &mut resources, "flowchart").unwrap();
    allocate_route_label_placements(&mut routes, &mut occupancy, &mut resources, "flowchart")
        .unwrap();
    assert_eq!(routes[0].plan.labels[0].placement.y(), 4);
    let later_route = PreparedRoute::for_test_with_endpoints(
        RoutePlan::new_without_markers_for_test(
            (2..=18)
                .map(|x| planned_cell(x, 5, '-', PlannedRouteCellKind::EdgeLine))
                .collect(),
            Vec::new(),
        ),
        1,
        "c",
        "d",
    );
    assert!(
        !occupancy
            .try_admit_route(1, &later_route, &mut resources, "flowchart")
            .unwrap(),
        "a later route cannot separate an already committed label from its host"
    );
}

#[test]
fn route_scene_relocates_labels_away_from_nodes_groups_and_other_labels() {
    let options = AsciiRenderOptions::ascii();
    let graph_layout = simple_graph_layout(&options);
    let node = &graph_layout.nodes[0];
    let node_route = RoutePlan::new_without_markers_for_test(
        vec![planned_cell(
            node.right() + 1,
            node.y + 1,
            '|',
            PlannedRouteCellKind::EdgeLine,
        )],
        vec![PlannedRouteLabel::new(
            RoutedLabelText::new("node").unwrap(),
            RoutedLabelPlacement::new(node.x, node.y, 4),
        )],
    );
    let mut node_routes = vec![PreparedRoute::for_test(node_route, 0)];
    let mut resources = unbounded_resources();
    allocate_test_label_placements(&mut node_routes, &graph_layout, &mut resources)
        .expect("route labels should move away from node geometry");
    let label = &node_routes[0].plan.labels[0];
    let label_rect = OccupiedRect::try_new(
        label.placement.x(),
        label.placement.y(),
        label.placement.width(),
        label.line_count(),
        &resources,
    )
    .unwrap();
    let node_rect =
        OccupiedRect::try_new(node.x, node.y, node.width, node.height, &resources).unwrap();
    assert!(!label_rect.intersects(node_rect));

    let group_layout = grouped_graph_layout(&options);
    let group = &group_layout.groups[0];
    let group_route = RoutePlan::new_without_markers_for_test(
        vec![planned_cell(
            group.right() + 1,
            group.y + 2,
            '|',
            PlannedRouteCellKind::EdgeLine,
        )],
        vec![PlannedRouteLabel::new(
            RoutedLabelText::new("group").unwrap(),
            RoutedLabelPlacement::new(group.right(), group.y + 1, 5),
        )],
    );
    let mut group_routes = vec![PreparedRoute::for_test(group_route, 0)];
    let mut resources = unbounded_resources();
    allocate_test_label_placements(&mut group_routes, &group_layout, &mut resources)
        .expect("route labels should move away from group borders and titles");
    let label = &group_routes[0].plan.labels[0];
    let label_rect = OccupiedRect::try_new(
        label.placement.x(),
        label.placement.y(),
        label.placement.width(),
        label.line_count(),
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
    assert!(
        label_rect.x > group.right(),
        "an external label must stay outside the group"
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
                label.line_count(),
                &resources,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    assert!(!footprints[0].intersects(footprints[1]));
}

#[test]
fn independent_labels_require_one_reading_column_on_overlapping_rows() {
    let options = AsciiRenderOptions::ascii();
    let graph_layout = layout_graph(&AsciiGraph::new(GraphDirection::TopDown), &options);
    let mut routes = vec![
        PreparedRoute::for_test(
            RoutePlan::new_without_markers_for_test(
                vec![planned_cell(0, 4, '-', PlannedRouteCellKind::EdgeLine)],
                vec![PlannedRouteLabel::new(
                    RoutedLabelText::new("submit").unwrap(),
                    RoutedLabelPlacement::new(1, 2, 6),
                )],
            ),
            0,
        ),
        PreparedRoute::for_test(
            RoutePlan::new_without_markers_for_test(
                vec![planned_cell(0, 6, '-', PlannedRouteCellKind::EdgeLine)],
                vec![PlannedRouteLabel::new(
                    RoutedLabelText::new("changes requested").unwrap(),
                    RoutedLabelPlacement::new(7, 2, 17),
                )],
            ),
            1,
        ),
    ];
    let mut resources = unbounded_resources();
    allocate_test_label_placements(&mut routes, &graph_layout, &mut resources).unwrap();
    let first = &routes[0].plan.labels[0];
    let second = &routes[1].plan.labels[0];
    assert!(
        first.placement.y() != second.placement.y()
            || first.placement.x() + first.placement.width() < second.placement.x()
            || second.placement.x() + second.placement.width() < first.placement.x()
    );
}

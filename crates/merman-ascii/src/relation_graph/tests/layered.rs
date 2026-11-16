use super::*;

#[test]
fn layered_relation_gap_grows_with_label_line_count() {
    let boxes = [
        RelationGraphBox::new("top".to_string(), vec!["A".to_string()], 1),
        RelationGraphBox::new("bottom".to_string(), vec!["B".to_string()], 1),
    ];
    let no_label_edges = vec![LayeredRelationEdge::new("top", "bottom", 0, 0)];
    let one_line_edges = vec![LayeredRelationEdge::new("top", "bottom", 0, 1)];
    let two_line_edges = vec![LayeredRelationEdge::new("top", "bottom", 0, 2)];

    let box_refs = boxes.iter().collect::<Vec<_>>();
    let mut resources = test_resources(AsciiResourcePolicy::default());
    let no_label_plan = plan_layered_relation_boxes(&box_refs, &no_label_edges, 1, &mut resources)
        .expect("unlabeled layered relation should plan");
    let mut resources = test_resources(AsciiResourcePolicy::default());
    let one_line_plan = plan_layered_relation_boxes(&box_refs, &one_line_edges, 1, &mut resources)
        .expect("single-line labeled relation should plan");
    let mut resources = test_resources(AsciiResourcePolicy::default());
    let two_line_plan = plan_layered_relation_boxes(&box_refs, &two_line_edges, 1, &mut resources)
        .expect("multiline labeled relation should plan");

    assert_eq!(no_label_plan.height(), 5);
    assert_eq!(one_line_plan.height(), 6);
    assert_eq!(two_line_plan.height(), 7);
}

#[test]
fn layered_relation_plan_reserves_width_for_reverse_spanning_edges() {
    let boxes = [
        RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
        RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        RelationGraphBox::new("c".to_string(), vec!["C".to_string()], 1),
    ];
    let edges = vec![
        LayeredRelationEdge::new("a", "b", 0, 0),
        LayeredRelationEdge::new("b", "c", 0, 0),
        LayeredRelationEdge::new("c", "a", 0, 0),
    ];

    let box_refs = boxes.iter().collect::<Vec<_>>();
    let mut resources = test_resources(AsciiResourcePolicy::default());
    let plan = plan_layered_relation_boxes(&box_refs, &edges, 1, &mut resources)
        .expect("cyclic plan should render");

    assert!(plan.width() > 3);
}

#[test]
fn layered_relation_plan_reserves_width_for_reverse_parallel_lanes() {
    let boxes = [
        RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
        RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
    ];
    let edges = vec![
        LayeredRelationEdge::new("a", "b", 0, 0),
        LayeredRelationEdge::new("b", "a", 0, 0),
    ];

    let box_refs = boxes.iter().collect::<Vec<_>>();
    let mut resources = test_resources(AsciiResourcePolicy::default());
    let plan = plan_layered_relation_boxes(&box_refs, &edges, 1, &mut resources)
        .expect("bidirectional plan should render");

    assert!(plan.width() > 3);
}

fn multi_parent_edges(endpoints: [(&'static str, &'static str); 4]) -> Vec<LayeredRelationEdge> {
    endpoints
        .into_iter()
        .map(|(source, target)| LayeredRelationEdge::new(source, target, 2, 1))
        .collect()
}

fn multi_parent_scene<'a>(
    boxes: &'a [RelationGraphBox],
    edges: Vec<LayeredRelationEdge>,
    resources: &mut ResourceContext,
) -> LayeredRelationScene<'a> {
    let box_refs = boxes.iter().collect::<Vec<_>>();
    LayeredRelationScene::new(
        &box_refs,
        edges,
        4,
        TerminalWidthProfile::Unicode,
        resources,
    )
    .expect("directed multi-parent component should use its bounded planar scene")
}

fn multi_parent_geometries(
    scene: &LayeredRelationScene<'_>,
    resources: &ResourceContext,
) -> Vec<LayeredRelationRouteGeometry> {
    let relations = (0..scene.draw_order().len())
        .map(|index| {
            let edge = scene.edge(index).expect("edge must exist");
            (
                edge.route_source_id().to_string(),
                edge.route_target_id().to_string(),
            )
        })
        .collect::<Vec<_>>();
    let refs = relations.iter().collect::<Vec<_>>();
    let adapter = TestRelationAdapter {
        summary_reason: Cell::new(None),
        overlap: TestRelationOverlap::None,
    };
    plan_layered_route_batch(scene, &refs, resources, &adapter)
        .expect("a planar component should have disjoint private routes")
        .0
        .into_iter()
        .map(|route| route.geometry().clone())
        .collect()
}

#[test]
fn directed_diamond_preserves_ranks_and_has_four_disjoint_routes() {
    let boxes = multi_parent_boxes();
    let edges = multi_parent_edges([("a", "c"), ("a", "d"), ("c", "b"), ("d", "b")]);
    let mut resources = test_resources(AsciiResourcePolicy::default());
    let scene = multi_parent_scene(&boxes, edges, &mut resources);
    assert!(scene.uses_private_channels());
    let y = |id: &str| {
        scene
            .placed_boxes()
            .iter()
            .find(|node| node.id() == id)
            .expect("node must be placed")
            .y()
    };
    assert!(y("a") < y("c"));
    assert_eq!(y("c"), y("d"));
    assert!(y("d") < y("b"));
    let geometries = multi_parent_geometries(&scene, &resources);
    assert_eq!(geometries.len(), 4);
    for (index, geometry) in geometries.iter().enumerate() {
        assert!(geometry.fits(scene.width(), scene.height()));
        assert!(!scene.route_geometry_overlaps_box(geometry));
        assert!(
            scene
                .placed_boxes()
                .iter()
                .any(|node| geometry.source_port().fits_box(node))
        );
        assert!(
            scene
                .placed_boxes()
                .iter()
                .any(|node| geometry.target_port().fits_box(node))
        );
        for other in &geometries[index + 1..] {
            assert!(!geometry.overlaps(other));
        }
    }
}

#[test]
fn multi_parent_geometry_is_stable_when_declarations_are_reordered() {
    let boxes = multi_parent_boxes();
    let mut first_resources = test_resources(AsciiResourcePolicy::default());
    let first = multi_parent_scene(
        &boxes,
        multi_parent_edges([("a", "c"), ("a", "d"), ("b", "c"), ("b", "d")]),
        &mut first_resources,
    );
    let mut second_resources = test_resources(AsciiResourcePolicy::default());
    let second = multi_parent_scene(
        &boxes,
        multi_parent_edges([("b", "d"), ("b", "c"), ("a", "d"), ("a", "c")]),
        &mut second_resources,
    );

    let placements = |scene: &LayeredRelationScene<'_>| {
        let mut placed = scene
            .placed_boxes()
            .iter()
            .map(|placed| (placed.id().to_owned(), placed.x(), placed.y()))
            .collect::<Vec<_>>();
        placed.sort_unstable();
        placed
    };
    assert_eq!(placements(&first), placements(&second));
}

#[test]
fn cyclic_channel_draws_are_continuous_between_the_actual_box_contacts() {
    use std::collections::{BTreeSet, VecDeque};

    for size in [3, 4, 5] {
        for direction in [
            RelationDirection::TopDown,
            RelationDirection::BottomUp,
            RelationDirection::LeftRight,
            RelationDirection::RightLeft,
        ] {
            let boxes = (0..size)
                .map(|index| {
                    RelationGraphBox::new(
                        format!("Node{index}"),
                        vec![
                            "+-------+".to_string(),
                            format!("| Node{index} |"),
                            "+-------+".to_string(),
                        ],
                        9,
                    )
                })
                .collect::<Vec<_>>();
            let relations = (0..size)
                .map(|index| {
                    (
                        format!("Node{index}"),
                        format!("Node{}", (index + 1) % size),
                    )
                })
                .collect::<Vec<_>>();
            let adapter = TestRelationAdapter {
                summary_reason: Cell::new(None),
                overlap: TestRelationOverlap::None,
            };
            let edges = relations
                .iter()
                .map(|relation| {
                    adapter
                        .build_edges(relation)
                        .with_reversed_route(direction == RelationDirection::BottomUp)
                })
                .collect();
            let mut resources = test_resources(AsciiResourcePolicy::default());
            let scene = LayeredRelationScene::new_with_direction(
                &boxes.iter().collect::<Vec<_>>(),
                edges,
                4,
                TerminalWidthProfile::Unicode,
                direction.transform(),
                &mut resources,
            )
            .expect("cycle scene should fit");
            let (routes, _) = plan_layered_route_batch(
                &scene,
                &relations.iter().collect::<Vec<_>>(),
                &resources,
                &adapter,
            )
            .expect("cycle routes should fit");
            assert_eq!(routes.len(), size);
            for route in &routes {
                let geometry = route.geometry();
                let endpoints = (0..size)
                    .filter_map(|index| scene.edge_endpoints(index))
                    .filter(|&(source, target)| geometry.connects_boxes(source, target))
                    .collect::<Vec<_>>();
                assert_eq!(
                    endpoints.len(),
                    1,
                    "each drawn route must connect exactly its two semantic endpoints"
                );
                let (source, target) = endpoints[0];
                let contact = |port: LayeredRelationPhysicalPort,
                               node: &PlacedRelationGraphBox<'_>| {
                    match port.side() {
                        PhysicalPortSide::Top => (port.x(), node.y() - 1),
                        PhysicalPortSide::Right => (node.right() + 1, port.marker_y()),
                        PhysicalPortSide::Bottom => (port.x(), node.bottom() + 1),
                        PhysicalPortSide::Left => (node.x() - 1, port.marker_y()),
                    }
                };
                let start = contact(geometry.source_port(), source);
                let end = contact(geometry.target_port(), target);
                let mut canvas = Canvas::new(scene.width(), scene.height());
                route
                    .draw_route_at(&mut canvas, &resources)
                    .expect("all channel segments should draw");
                let mut cells = BTreeSet::new();
                for y in 0..scene.height() {
                    for x in 0..scene.width() {
                        if canvas.get(x, y).is_some_and(|ch| ch != ' ') {
                            cells.insert((x, y));
                        }
                    }
                }
                assert!(
                    cells.contains(&start) && cells.contains(&end),
                    "both actual box contacts must be painted"
                );
                let mut visited = BTreeSet::from([start]);
                let mut queue = VecDeque::from([start]);
                while let Some((x, y)) = queue.pop_front() {
                    for point in [
                        (x.checked_sub(1), Some(y)),
                        (x.checked_add(1), Some(y)),
                        (Some(x), y.checked_sub(1)),
                        (Some(x), y.checked_add(1)),
                    ] {
                        if let (Some(x), Some(y)) = point
                            && cells.contains(&(x, y))
                            && visited.insert((x, y))
                        {
                            queue.push_back((x, y));
                        }
                    }
                }
                assert!(
                    visited.contains(&end),
                    "drawn cells must connect source contact to target contact"
                );
                assert_eq!(
                    visited, cells,
                    "no detached segment may survive geometry materialization"
                );
                assert!(!scene.route_overlaps_box(route));
            }
            for (index, route) in routes.iter().enumerate() {
                for other in &routes[index + 1..] {
                    assert!(
                        !route.route_overlaps(other),
                        "every edge must retain its own channel"
                    );
                }
            }
        }
    }
}

#[test]
fn layered_relation_route_plan_draws_route_and_overlays() {
    let top_box = RelationGraphBox::new("top".to_string(), vec!["AAA".to_string()], 3);
    let bottom_box = RelationGraphBox::new("bottom".to_string(), vec!["BBB".to_string()], 3);
    let placed = vec![
        PlacedRelationGraphBox::for_test("top", &top_box, 0, 0),
        PlacedRelationGraphBox::for_test("bottom", &bottom_box, 0, 4),
    ];
    let resources = test_resources(AsciiResourcePolicy::default());
    let geometry = plan_layered_relation_route(
        LayeredRelationRouteRequest::new(
            &placed,
            &placed[0],
            &placed[1],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0, 0),
        ),
        &resources,
    )
    .expect("route geometry should fit");
    let route = LayeredRelationRoutePlan::new(
        geometry.clone(),
        '|',
        '-',
        RelationLineChars::new(['-', '|', '.', ':'], '+'),
        vec![
            RelationOverlay::text(
                geometry.source_x(),
                geometry.source_marker_y(),
                "T".to_string(),
                AsciiColorRole::EdgeArrow,
                TerminalWidthProfile::Unicode,
            ),
            RelationOverlay::text(
                (geometry.source_x() + geometry.target_x()) / 2,
                geometry.route_y() - 1,
                "L".to_string(),
                AsciiColorRole::EdgeLabel,
                TerminalWidthProfile::Unicode,
            ),
            RelationOverlay::text(
                geometry.target_x(),
                geometry.target_marker_y(),
                "B".to_string(),
                AsciiColorRole::EdgeArrow,
                TerminalWidthProfile::Unicode,
            ),
        ],
    );
    let mut canvas = Canvas::new(3, 5);
    let resources = test_resources(AsciiResourcePolicy::default());

    route
        .draw_route_at(&mut canvas, &resources)
        .expect("test route should fit");
    route
        .draw_overlays_at(&mut canvas, &resources)
        .expect("test overlays should fit");

    assert_eq!(canvas.get(1, 1), Some('T'));
    assert_eq!(canvas.get(1, 2), Some('L'));
    assert_eq!(canvas.get(1, 3), Some('B'));
    assert_eq!(
        canvas.get_color(1, 1),
        Some(crate::canvas::CanvasColor::Role(AsciiColorRole::EdgeArrow))
    );
    assert_eq!(
        canvas.get_color(1, 2),
        Some(crate::canvas::CanvasColor::Role(AsciiColorRole::EdgeLabel))
    );
}

#[test]
fn layered_relation_route_label_y_follows_source_to_target_direction() {
    let top_box = RelationGraphBox::new("top".to_string(), vec!["AAA".to_string()], 3);
    let bottom_box = RelationGraphBox::new("bottom".to_string(), vec!["BBB".to_string()], 3);
    let placed = vec![
        PlacedRelationGraphBox::for_test("top", &top_box, 0, 0),
        PlacedRelationGraphBox::for_test("bottom", &bottom_box, 0, 10),
    ];

    let resources = test_resources(AsciiResourcePolicy::default());
    let downward = plan_layered_relation_route(
        LayeredRelationRouteRequest::new(
            &placed,
            &placed[0],
            &placed[1],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0, 0),
        ),
        &resources,
    )
    .expect("downward route should fit");
    let resources = test_resources(AsciiResourcePolicy::default());
    let upward = plan_layered_relation_route(
        LayeredRelationRouteRequest::new(
            &placed,
            &placed[1],
            &placed[0],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0, 0),
        ),
        &resources,
    )
    .expect("upward route should fit");

    assert_eq!(downward.label_y_after_source(), 2);
    assert_eq!(upward.label_y_after_source(), 8);
}

#[test]
fn layered_relation_route_profile_reserves_rows_for_multiline_endpoint_labels() {
    let top_box = RelationGraphBox::new("top".to_string(), vec!["AAA".to_string()], 3);
    let bottom_box = RelationGraphBox::new("bottom".to_string(), vec!["BBB".to_string()], 3);
    let placed = vec![
        PlacedRelationGraphBox::for_test("top", &top_box, 0, 0),
        PlacedRelationGraphBox::for_test("bottom", &bottom_box, 0, 10),
    ];

    let resources = test_resources(AsciiResourcePolicy::default());
    let geometry = plan_layered_relation_route(
        LayeredRelationRouteRequest::new(
            &placed,
            &placed[0],
            &placed[1],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0, 2),
        ),
        &resources,
    )
    .expect("labeled route should fit");

    assert_eq!(geometry.source_marker_y(), 3);
    assert_eq!(geometry.label_y_after_source(), 4);
    assert_eq!(geometry.route_y(), 7);
    assert_eq!(geometry.target_marker_y(), 7);
}

#[test]
fn layered_relation_route_plan_avoids_intermediate_boxes() {
    let top_box = RelationGraphBox::new("top".to_string(), vec!["AAA".to_string()], 3);
    let middle_box = RelationGraphBox::new("middle".to_string(), vec!["MMMMMMM".to_string()], 7);
    let bottom_box = RelationGraphBox::new("bottom".to_string(), vec!["BBB".to_string()], 3);
    let placed = vec![
        PlacedRelationGraphBox::for_test("top", &top_box, 0, 0),
        PlacedRelationGraphBox::for_test("middle", &middle_box, 0, 4),
        PlacedRelationGraphBox::for_test("bottom", &bottom_box, 0, 10),
    ];

    let resources = test_resources(AsciiResourcePolicy::default());
    let geometry = plan_layered_relation_route(
        LayeredRelationRouteRequest::new(
            &placed,
            &placed[0],
            &placed[2],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0, 0),
        ),
        &resources,
    )
    .expect("spanning route should fit");

    assert_eq!(geometry.source_x(), 7);
    assert_eq!(geometry.target_x(), 7);
    assert_eq!(geometry.route_y(), 9);
}

#[test]
fn layered_relation_route_uses_right_exterior_when_left_has_no_margin() {
    let top_box = RelationGraphBox::new("top".to_string(), vec!["top".to_string()], 18);
    let middle_left_box =
        RelationGraphBox::new("middle-left".to_string(), vec!["left".to_string()], 15);
    let middle_right_box =
        RelationGraphBox::new("middle-right".to_string(), vec!["right".to_string()], 15);
    let bottom_box = RelationGraphBox::new("bottom".to_string(), vec!["bottom".to_string()], 21);
    let placed = vec![
        PlacedRelationGraphBox::for_test("top", &top_box, 9, 0),
        PlacedRelationGraphBox::for_test("middle-left", &middle_left_box, 0, 4),
        PlacedRelationGraphBox::for_test("middle-right", &middle_right_box, 28, 4),
        PlacedRelationGraphBox::for_test("bottom", &bottom_box, 44, 10),
    ];

    let resources = test_resources(AsciiResourcePolicy::default());
    let lane_offset = spanning_lane_offset_around_intermediate_boxes(
        &placed, &placed[0], &placed[3], 0, &resources,
    )
    .expect("a blocked route at the left canvas edge should use the right exterior");

    assert_eq!(lane_offset, 26);
}

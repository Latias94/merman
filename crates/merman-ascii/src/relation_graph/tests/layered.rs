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

    let options = AsciiRenderOptions::ascii();
    let box_refs = boxes.iter().collect::<Vec<_>>();
    let mut resources = test_resources(&options);
    let no_label_plan = plan_layered_relation_boxes(&box_refs, &no_label_edges, 1, &mut resources)
        .expect("unlabeled layered relation should plan");
    let mut resources = test_resources(&options);
    let one_line_plan = plan_layered_relation_boxes(&box_refs, &one_line_edges, 1, &mut resources)
        .expect("single-line labeled relation should plan");
    let mut resources = test_resources(&options);
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

    let options = AsciiRenderOptions::ascii();
    let box_refs = boxes.iter().collect::<Vec<_>>();
    let mut resources = test_resources(&options);
    let plan = plan_layered_relation_boxes(&box_refs, &edges, 1, &mut resources)
        .expect("cyclic plan should render");

    assert_eq!(plan.width(), 7);
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

    let options = AsciiRenderOptions::ascii();
    let box_refs = boxes.iter().collect::<Vec<_>>();
    let mut resources = test_resources(&options);
    let plan = plan_layered_relation_boxes(&box_refs, &edges, 1, &mut resources)
        .expect("bidirectional plan should render");

    assert_eq!(plan.width(), 7);
}

#[test]
fn layered_relation_route_plan_draws_route_and_overlays() {
    let top_box = RelationGraphBox::new("top".to_string(), vec!["AAA".to_string()], 3);
    let bottom_box = RelationGraphBox::new("bottom".to_string(), vec!["BBB".to_string()], 3);
    let placed = vec![
        PlacedRelationGraphBox::for_test("top", &top_box, 0, 0),
        PlacedRelationGraphBox::for_test("bottom", &bottom_box, 0, 4),
    ];
    let options = AsciiRenderOptions::ascii();
    let mut resources = test_resources(&options);
    let geometry = plan_layered_relation_route(
        LayeredRelationRouteRequest::new(
            &placed,
            &placed[0],
            &placed[1],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0, 0),
        ),
        &mut resources,
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

    route
        .draw_route_at(&mut canvas)
        .expect("test route should fit");
    route
        .draw_overlays_at(&mut canvas)
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

    let options = AsciiRenderOptions::ascii();
    let mut resources = test_resources(&options);
    let downward = plan_layered_relation_route(
        LayeredRelationRouteRequest::new(
            &placed,
            &placed[0],
            &placed[1],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0, 0),
        ),
        &mut resources,
    )
    .expect("downward route should fit");
    let mut resources = test_resources(&options);
    let upward = plan_layered_relation_route(
        LayeredRelationRouteRequest::new(
            &placed,
            &placed[1],
            &placed[0],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0, 0),
        ),
        &mut resources,
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

    let options = AsciiRenderOptions::ascii();
    let mut resources = test_resources(&options);
    let geometry = plan_layered_relation_route(
        LayeredRelationRouteRequest::new(
            &placed,
            &placed[0],
            &placed[1],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0, 2),
        ),
        &mut resources,
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

    let options = AsciiRenderOptions::ascii();
    let mut resources = test_resources(&options);
    let geometry = plan_layered_relation_route(
        LayeredRelationRouteRequest::new(
            &placed,
            &placed[0],
            &placed[2],
            0,
            LayeredRelationRouteProfile::new(1, 1, 1, 0, 0),
        ),
        &mut resources,
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

    let options = AsciiRenderOptions::ascii();
    let mut resources = test_resources(&options);
    let lane_offset = spanning_lane_offset_around_intermediate_boxes(
        &placed,
        &placed[0],
        &placed[3],
        0,
        &mut resources,
    )
    .expect("a blocked route at the left canvas edge should use the right exterior");

    assert_eq!(lane_offset, 26);
}

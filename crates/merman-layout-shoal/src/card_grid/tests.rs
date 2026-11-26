use super::layout_with_control;
use crate::{
    EdgeId, EdgeSpec, Graph, GroupId, GroupSpec, LayoutError, MarkerMetrics, NodeId, NodeSpec,
    NoopWorkControl, Options, Point, Rect, RoutePoint, Side, Size, WidthPolicy, WorkControl,
    WorkError,
};

fn cards(count: u32) -> Graph {
    let mut graph = Graph::default();
    graph.nodes.extend((0..count).map(|seq| {
        NodeSpec::new(
            Size {
                width: 80.0,
                height: 44.0,
            },
            seq,
        )
    }));
    graph
}

fn edge(from: u32, to: u32) -> EdgeSpec {
    EdgeSpec::new(NodeId(from).into(), NodeId(to).into())
}

#[test]
fn card_chain_wraps_deterministically_and_folds_to_available_width() {
    let mut graph = cards(9);
    graph.groups.push(GroupSpec::new(0));
    for node in &mut graph.nodes {
        node.parent = Some(GroupId(0));
    }
    graph.edges.extend((0..8).map(|i| edge(i, i + 1)));
    let mut options = Options {
        capture_node_ranks: true,
        spacing: crate::Spacing {
            node: 30.0,
            rank: 30.0,
            edge: 4.0,
            ..crate::Spacing::default()
        },
        ..Options::default()
    };
    let natural = layout_with_control(&graph, &options, &mut NoopWorkControl).unwrap();
    assert_eq!(natural.node_ranks, [0, 0, 0, 1, 1, 1, 2, 2, 2]);
    assert_eq!(natural.nodes[0].center().x, natural.nodes[3].center().x);
    assert_eq!(natural.nodes[0].center().y, natural.nodes[1].center().y);

    options.width_policy = WidthPolicy::FitWidth {
        max_width: 240.0,
        allow_turn: false,
        turn_breakpoint: 640.0,
        fold_wide_ranks: true,
    };
    let narrow = layout_with_control(&graph, &options, &mut NoopWorkControl).unwrap();
    assert!(narrow.bounds.width <= 240.0, "{:?}", narrow.bounds);
    assert_eq!(narrow.node_ranks, [0, 0, 1, 1, 2, 2, 3, 3, 4]);
    assert_eq!(narrow.edges.len(), graph.edges.len());
    let boundary = narrow.groups[0].rect;
    assert!(narrow.nodes.iter().all(|node| {
        boundary.x < node.x
            && node.x + node.width < boundary.x + boundary.width
            && boundary.y < node.y
            && node.y + node.height < boundary.y + boundary.height
    }));
    assert_eq!(
        narrow,
        layout_with_control(&graph, &options, &mut NoopWorkControl).unwrap()
    );
}

fn crosses(rect: Rect, [a, b]: [RoutePoint; 2]) -> bool {
    if a.x == b.x {
        return rect.x < a.x
            && a.x < rect.x + rect.width
            && a.y.max(b.y) > rect.y
            && a.y.min(b.y) < rect.y + rect.height;
    }
    assert_eq!(a.y, b.y, "route leg must be orthogonal");
    rect.y < a.y
        && a.y < rect.y + rect.height
        && a.x.max(b.x) > rect.x
        && a.x.min(b.x) < rect.x + rect.width
}

#[test]
fn aligned_routes_stay_straight_or_detour_into_distinct_channels() {
    let mut graph = cards(9);
    let mut blocked = edge(3, 5);
    blocked.end_marker = MarkerMetrics {
        length: 11.0,
        width: 8.0,
    };
    graph.edges.extend([edge(0, 1), blocked, blocked]);
    let output = layout_with_control(&graph, &Options::default(), &mut NoopWorkControl).unwrap();

    let straight = &output.edges[0];
    assert_eq!(straight.ports, (Side::Right, Side::Left));
    assert!(straight.points.iter().all(|p| p.y == straight.points[0].y));
    let blocker = output.nodes[4];
    for detour in &output.edges[1..] {
        assert!(detour.points.iter().any(|p| p.y != detour.points[0].y));
        assert!(
            detour
                .points
                .windows(2)
                .all(|leg| !crosses(blocker, [leg[0], leg[1]])),
            "{:?}",
            detour.points
        );
    }
    let track = |points: &[RoutePoint]| {
        let center = blocker.center().x;
        points
            .windows(2)
            .find_map(|leg| {
                let [a, b] = [leg[0], leg[1]];
                (a.x.min(b.x) < center && center < a.x.max(b.x)).then_some(a.y)
            })
            .unwrap()
    };
    assert_ne!(
        track(&output.edges[1].points),
        track(&output.edges[2].points)
    );
}

#[test]
fn diagonal_relation_uses_one_elbow_at_the_empty_cell() {
    let mut graph = cards(3);
    graph.edges.push(edge(1, 2));
    let output = layout_with_control(&graph, &Options::default(), &mut NoopWorkControl).unwrap();
    let route = &output.edges[0];
    assert_eq!(route.ports, (Side::Bottom, Side::Right));
    assert_eq!(
        route
            .points
            .windows(3)
            .filter(|leg| leg[1].axis != leg[2].axis)
            .count(),
        1
    );
    assert!(
        route
            .points
            .windows(2)
            .all(|leg| !crosses(output.nodes[0], [leg[0], leg[1]]))
    );
}

#[test]
fn visibility_cap_falls_back_to_clear_card_gutters() {
    let mut graph = cards(15);
    for node in &mut graph.nodes {
        node.size.width = 100.0 + f64::from(node.seq % 7) * 3.0;
        node.size.height = 50.0 + f64::from(node.seq % 11) * 7.0;
    }
    let mut relation = edge(14, 0);
    relation.end_marker = MarkerMetrics {
        length: 11.0,
        width: 11.0,
    };
    graph.edges.push(relation);
    let mut options = Options::default();
    options.caps.max_ortho_grid_nodes = 0;
    let output = layout_with_control(&graph, &options, &mut NoopWorkControl).unwrap();
    let route = &output.edges[0];
    assert!(route.points.windows(2).all(|leg| {
        output
            .nodes
            .iter()
            .all(|&card| !crosses(card, [leg[0], leg[1]]))
    }));
}

fn on_face(rect: Rect, p: RoutePoint, side: Side) -> bool {
    let dx = p.x - rect.x;
    let dy = p.y - rect.y;
    let epsilon = 1e-8;
    let (face_delta, cross, length) = match side {
        Side::Top => (dy, dx, rect.width),
        Side::Right => (dx - rect.width, dy, rect.height),
        Side::Bottom => (dy - rect.height, dx, rect.width),
        Side::Left => (dx, dy, rect.height),
    };
    face_delta.abs() < epsilon && cross > 0.0 && cross < length
}

#[test]
fn nested_titles_and_routes_keep_their_input_occurrences() {
    let mut graph = cards(3);
    let mut outer = GroupSpec::new(0);
    outer.title = Some(Size {
        width: 100.0,
        height: 18.0,
    });
    let mut inner = outer;
    inner.seq = 1;
    inner.parent = Some(GroupId(0));
    graph.groups.extend([outer, inner]);
    graph.nodes[1].parent = Some(GroupId(1));
    let mut forward = edge(0, 1);
    forward.label = Some(Size {
        width: 32.0,
        height: 12.0,
    });
    let back = EdgeSpec::new(NodeId(1).into(), GroupId(0).into());
    graph.edges.extend([forward, back]);

    let output = layout_with_control(&graph, &Options::default(), &mut NoopWorkControl).unwrap();
    assert_eq!(output.groups[1].parent, Some(GroupId(0)));
    for (group, inner_top) in output
        .groups
        .iter()
        .zip([output.groups[1].rect.y, output.nodes[1].y])
    {
        let Point { x, y } = group.title_center.unwrap();
        assert_eq!(x, group.rect.center().x);
        assert!(group.rect.y < y && y + 9.0 < inner_top);
    }
    assert!(output.edges[0].label_center.is_some());
    let stitches: Vec<_> = output
        .border_stitches
        .iter()
        .filter(|s| s.edge == EdgeId(0))
        .collect();
    assert_eq!(stitches.len(), 2);
    assert_eq!(stitches[0].group, GroupId(0));
    assert_eq!(stitches[1].group, GroupId(1));
    assert!(stitches[0].point < stitches[1].point);
    for stitch in stitches {
        assert!(on_face(
            output.groups[stitch.group.0 as usize].rect,
            output.edges[0].points[stitch.point],
            stitch.side,
        ));
    }
    let back = &output.edges[1];
    assert!(on_face(output.nodes[1], back.points[0], back.ports.0));
    assert!(on_face(
        output.groups[0].rect,
        *back.points.last().unwrap(),
        back.ports.1,
    ));
}

#[test]
fn work_control_can_stop_card_grid_placement() {
    struct RefuseCharges;

    impl WorkControl for RefuseCharges {
        fn check(&mut self) -> Result<(), WorkError> {
            Ok(())
        }

        fn charge(&mut self, _units: usize) -> Result<(), WorkError> {
            Err(WorkError::Interrupted)
        }
    }

    assert_eq!(
        layout_with_control(&cards(9), &Options::default(), &mut RefuseCharges),
        Err(LayoutError::Work(WorkError::Interrupted)),
    );
}

#[test]
fn same_row_person_cards_keep_direct_neighbor_straight_and_avoid_crossing() {
    let mut graph = Graph::default();
    for (seq, height, anchor_y) in [
        (0, 78.0, Some(43.0)),
        (1, 78.0, Some(43.0)),
        (2, 70.0, None),
        (3, 70.0, None),
        (4, 300.0, None),
    ] {
        let mut node = NodeSpec::new(
            Size {
                width: 216.0,
                height,
            },
            seq,
        );
        node.anchor_y = anchor_y;
        graph.nodes.push(node);
    }
    graph.edges.extend([edge(0, 2), edge(1, 2)]);
    let mut options = Options::default();
    options.packing = crate::PackingPolicy::AspectTargeted { target_ratio: 3.0 };
    let output = layout_with_control(&graph, &options, &mut NoopWorkControl).unwrap();
    // Confirm cards 0, 1, 2 sit on the same top row with aligned horizontal anchors.
    assert!(output.nodes[0].x < output.nodes[1].x && output.nodes[1].x < output.nodes[2].x);
    let direct = &output.edges[1].points;
    let direct_y = direct[0].y;
    assert!(
        direct.iter().all(|p| (p.y - direct_y).abs() < 1e-6),
        "adjacent Person -> System should stay dead straight horizontally, got {direct:?}"
    );
    let detour = &output.edges[0].points;
    let detour_end_y = detour.last().unwrap().y;
    assert!(
        detour_end_y > direct_y,
        "bottom-detouring edge should enter below the straight neighbor ({detour_end_y} vs {direct_y})"
    );
    assert!(
        detour.iter().all(|p| p.y >= direct_y - 1e-6),
        "detouring edge must not cross above the direct neighbor: {detour:?}"
    );
}

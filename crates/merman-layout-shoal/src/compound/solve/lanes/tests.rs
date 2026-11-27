use super::*;
use crate::{GroupId, MarkerMetrics, RoutingStyle, layout};

const DIRECTIONS: [Direction; 4] = [
    Direction::Down,
    Direction::Up,
    Direction::Right,
    Direction::Left,
];

fn swimlane(owners: &[u32], edges: &[(u32, u32)]) -> Graph {
    let lane_count = owners.iter().copied().max().unwrap() + 1;
    Graph {
        nodes: owners
            .iter()
            .enumerate()
            .map(|(seq, &lane)| {
                let mut node = NodeSpec::new(
                    Size {
                        width: 48.0,
                        height: 48.0,
                    },
                    seq as u32,
                );
                node.parent = Some(GroupId(lane));
                node
            })
            .collect(),
        groups: (0..lane_count).map(GroupSpec::new).collect(),
        lane_order: (0..lane_count).map(GroupId).collect(),
        edges: edges
            .iter()
            .map(|&(a, b)| EdgeSpec::new(NodeId(a).into(), NodeId(b).into()))
            .collect(),
        ..Graph::default()
    }
}

fn orthogonal() -> Options {
    Options {
        routing: RoutingStyle::Orthogonal,
        capture_node_ranks: true,
        compact_ranks: false,
        ..Options::default()
    }
}

fn clear_nodes(route: &EdgeGeometry, drawing: &LayoutOutput) {
    for points in route.points.windows(2) {
        let [a, b] = [point(points[0]), point(points[1])];
        assert!(a.x == b.x || a.y == b.y);
        for &node in &drawing.nodes {
            assert!(!fit::blocks(node, a, b), "{node:?} blocks {a:?} to {b:?}");
        }
    }
}

#[test]
fn simple_cross_lane_handoffs_use_one_corner_in_all_directions() {
    let mut graph = swimlane(&[0, 1], &[(0, 1)]);
    for group in &mut graph.groups {
        group.padding = crate::Insets {
            top: 28.0,
            right: 28.0,
            bottom: 28.0,
            left: 28.0,
        };
    }
    graph.edges[0].label = Some(Size {
        width: 16.0,
        height: 10.0,
    });
    graph.edges[0].end_marker = MarkerMetrics {
        length: 9.0,
        width: 7.0,
    };
    for direction in DIRECTIONS {
        graph.direction = direction;
        let drawing = layout(&graph, &orthogonal()).unwrap();
        let route = &drawing.edges[0];
        assert_eq!(route.points.len(), 3, "{direction:?}: {route:?}");
        assert_ne!(route.ports.0.axis(), route.ports.1.axis());
        assert!(route.label_center.is_some());
        clear_nodes(route, &drawing);
    }
}

#[test]
fn handoffs_keep_the_ranked_route_when_both_corners_enter_nodes() {
    let graph = swimlane(&[0, 0, 1, 1], &[(0, 1), (2, 3), (0, 3)]);
    let drawing = layout(&graph, &orthogonal()).unwrap();
    let route = &drawing.edges[2];
    assert!(route.points.len() > 3, "{route:?}");
    clear_nodes(route, &drawing);
}

fn claims_swimlane() -> (Graph, Options) {
    // Measured boxes from the TB claims diagram with native font measurement.
    let mut graph = swimlane(
        &[0, 0, 0, 1, 2, 2, 1, 3],
        &[
            (0, 1),
            (4, 5),
            (1, 4),
            (5, 3),
            (3, 6),
            (3, 1),
            (6, 7),
            (7, 2),
        ],
    );
    for (node, (width, height)) in graph.nodes.iter_mut().zip([
        (171.25, 45.0),
        (152.0, 45.0),
        (171.25, 45.0),
        (171.0, 171.0),
        (152.0, 45.0),
        (152.0, 66.0),
        (152.0, 66.0),
        (152.0, 66.0),
    ]) {
        node.size = Size { width, height };
    }
    graph.nodes[3].contour = crate::ShapeContourHint::Diamond;
    graph.nodes[3].port_span = Some(20.0);
    graph.lane_order.remove(2);
    graph.groups[2].parent = Some(GroupId(1));
    for (group, width) in graph.groups.iter_mut().zip([101.14, 97.25, 118.28, 122.42]) {
        group.title = Some(Size {
            width,
            height: 21.0,
        });
        group.padding = crate::Insets {
            top: if group.parent.is_some() { 29.0 } else { 8.0 },
            right: 8.0,
            bottom: 8.0,
            left: 8.0,
        };
    }
    for edge in &mut graph.edges {
        edge.end_marker = MarkerMetrics {
            length: 16.62,
            width: 14.0,
        };
        edge.set_marker_runways([0.0, edge.end_marker.length]);
    }
    graph.edges[4].label = Some(Size {
        width: 45.7,
        height: 21.0,
    });
    graph.edges[5].label = Some(Size {
        width: 77.69,
        height: 21.0,
    });
    let mut options = orthogonal();
    options.spacing.node = 50.0;
    options.spacing.rank = 50.0;
    options.compact_ranks = true;
    options.global_orthogonal = true;
    options.group_policy.inherit_crossed_direction = false;
    (graph, options)
}

fn rect_clearance(route: &EdgeGeometry, rect: Rect) -> f64 {
    route
        .points
        .windows(2)
        .map(|leg| {
            let [a, b] = [leg[0], leg[1]];
            assert!(a.x == b.x || a.y == b.y, "{leg:?}");
            let dx = (rect.x - a.x.max(b.x)).max(a.x.min(b.x) - rect.x - rect.width);
            let dy = (rect.y - a.y.max(b.y)).max(a.y.min(b.y) - rect.y - rect.height);
            dx.max(dy).max(0.0)
        })
        .fold(f64::INFINITY, f64::min)
}

#[test]
fn nested_top_entry_clears_the_measured_title_in_both_axes() {
    let (graph, options) = claims_swimlane();
    let drawing = layout(&graph, &options).unwrap();
    let group = &drawing.groups[2];
    let title = centered(group.title_center.unwrap(), graph.groups[2].title.unwrap());
    for route in &drawing.edges {
        let clearance = rect_clearance(route, title);
        assert!(
            clearance >= 8.0 - 1e-9,
            "title clearance {clearance}: {route:?}"
        );
    }
    let stitch = drawing
        .border_stitches
        .iter()
        .find(|stitch| stitch.edge.0 == 2 && stitch.group == GroupId(2))
        .unwrap();
    assert_eq!(stitch.side, Side::Top);
    assert_eq!(drawing.edges[2].points[stitch.point].y, group.rect.y);
}

#[test]
fn cross_lane_return_keeps_edge_spacing_from_an_unrelated_nested_wall() {
    let (graph, options) = claims_swimlane();
    let drawing = layout(&graph, &options).unwrap();
    let route = &drawing.edges[5];
    assert!(route.reversed_for_ranking && route.label_center.is_some());
    let clearance = rect_clearance(route, drawing.groups[2].rect);
    assert!(
        clearance >= options.spacing.edge - 1e-9,
        "wall clearance {clearance}: {route:?}"
    );
    assert_eq!(layout(&graph, &options).unwrap(), drawing);
}

#[test]
fn same_lane_feedback_uses_a_reserved_margin_in_all_directions() {
    let mut graph = swimlane(&[0, 0, 0], &[(0, 1), (1, 2), (2, 0)]);
    for direction in DIRECTIONS {
        graph.direction = direction;
        let drawing = layout(&graph, &orthogonal()).unwrap();
        let route = drawing
            .edges
            .iter()
            .find(|edge| edge.reversed_for_ranking)
            .unwrap();
        let [a, b, c, d] = route.points.as_slice() else {
            panic!("{direction:?}: {route:?}")
        };
        let side = route.ports.0;
        assert_eq!(side, route.ports.1);
        assert_ne!(side.axis(), direction.axis());
        let rail = cross(flow_sides(direction).0, point(*b));
        assert_eq!(rail, cross(flow_sides(direction).0, point(*c)));
        let shell = crate::geom::Bounds::of(drawing.groups[0].rect);
        let shell_span = if direction.axis() == Axis::Vertical {
            shell.x
        } else {
            shell.y
        };
        assert!(shell_span[0] < rail && rail < shell_span[1]);
        let outward = |tip| match side {
            Side::Left | Side::Top => rail < cross(flow_sides(direction).0, point(tip)),
            Side::Right | Side::Bottom => rail > cross(flow_sides(direction).0, point(tip)),
        };
        assert!(outward(*a) && outward(*d));
        clear_nodes(route, &drawing);
    }
}

#[test]
fn close_compacts_only_empty_turn_free_transition_depth() {
    let mut graph = swimlane(&[0, 0, 0], &[(0, 1), (1, 2)]);
    graph.edges[0].min_rank_span = 4;
    for direction in DIRECTIONS {
        graph.direction = direction;
        let mut options = orthogonal();
        let full = layout(&graph, &options).unwrap();
        options.compact_ranks = true;
        let close = layout(&graph, &options).unwrap();
        let depth = |drawing: &LayoutOutput, a: usize, b: usize| {
            let start = flow_sides(direction).0;
            let coord = |node: Rect| match direction.axis() {
                Axis::Vertical => node.center().y,
                Axis::Horizontal => node.center().x,
            };
            (coord(drawing.nodes[b]) - coord(drawing.nodes[a])).abs()
                - super::depth(start, graph.nodes[a].size)
        };
        assert_eq!(full.node_ranks, close.node_ranks);
        assert!(depth(&close, 0, 1) < depth(&full, 0, 1));
        assert!(depth(&close, 0, 1) >= options.spacing.rank);
        assert!((depth(&close, 1, 2) - depth(&full, 1, 2)).abs() < 1e-9);
        for route in &close.edges {
            clear_nodes(route, &close);
        }
    }
    graph.direction = Direction::Down;
    graph.edges[0].label = Some(Size {
        width: 34.0,
        height: 14.0,
    });
    let mut options = orthogonal();
    let full = layout(&graph, &options).unwrap();
    options.compact_ranks = true;
    assert_eq!(layout(&graph, &options).unwrap(), full);
}

#[test]
fn swimlane_fixed_offset_survives_column_width_ulp_rounding() {
    let mut graph = swimlane(&[0, 1], &[]);
    // Lane 0 shifts lane 1 past x = 1000.0; lane 1's `min_width = 2 * 30.3 = 60.6` loses 1 ULP
    // when reconstructed as `(1000.0 + 60.6) - 1000.0 = 60.59999999999991 < 60.6`.
    graph.groups[0].min_size.width = 1000.0;
    graph.groups[1].padding = crate::Insets::default();
    graph.nodes[1].size = Size {
        width: 10.0,
        height: 10.0,
    };
    let mut edge = EdgeSpec::new(NodeId(0).into(), GroupId(1).into());
    edge.target_port = PortSideHint::FixedOffset {
        side: Side::Top,
        offset: 30.3,
    };
    graph.edges.push(edge);
    let drawing = layout(&graph, &orthogonal()).unwrap();
    let lane = drawing.groups[1].rect;
    let contact = drawing.edges[0].points.last().unwrap();
    assert!(
        contact.x >= lane.x && contact.x <= lane.x + lane.width,
        "fixed offset contact {contact:?} must lie on lane face {lane:?}"
    );
}

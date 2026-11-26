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

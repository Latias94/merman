use merman_layout_shoal::{
    Direction, EdgeSpec, Graph, MarkerMetrics, NodeId, NodeSpec, Options, Rect, RoutingStyle, Side,
    Size, layout,
};

#[test]
fn measured_class_lane_uses_the_space_beside_its_tall_sibling() {
    // Shoal measurements for Request -> Huge, Request -> Cache -> Decode -> Response.
    // Huge declares ten members. The other classes contain only their name and empty sections.
    let sizes = [
        (68.52, 81.0),
        (176.11, 303.0),
        (56.48, 81.0),
        (62.5, 81.0),
        (74.54, 81.0),
    ];
    let mut graph = Graph::default();
    for (from, to) in [(0, 1), (0, 2), (2, 3), (3, 4)] {
        let mut edge = EdgeSpec::new(NodeId(from).into(), NodeId(to).into());
        edge.start_marker = MarkerMetrics {
            length: 6.0,
            width: 0.0,
        };
        edge.end_marker = MarkerMetrics {
            length: 13.0,
            width: 20.0,
        };
        graph.edges.push(edge);
    }
    let mut options = Options::default();
    options.capture_node_ranks = true;
    for direction in [
        Direction::Down,
        Direction::Up,
        Direction::Right,
        Direction::Left,
    ] {
        graph.direction = direction;
        let horizontal = matches!(direction, Direction::Left | Direction::Right);
        graph.nodes = sizes
            .into_iter()
            .enumerate()
            .map(|(seq, (w, h))| {
                let (width, height) = if horizontal { (h, w) } else { (w, h) };
                NodeSpec::new(Size { width, height }, seq as u32)
            })
            .collect();
        for (style, baseline_along) in [
            (RoutingStyle::Curved, 690.0),
            (RoutingStyle::Orthogonal, 711.0),
            (RoutingStyle::Polyline, 690.0),
        ] {
            options.routing = style;
            for span in [1, 3] {
                graph.edges[2].min_rank_span = span;
                options.compact_ranks = false;
                let full = layout(&graph, &options).unwrap();
                options.compact_ranks = true;
                let tight = layout(&graph, &options).unwrap();
                assert_eq!(tight.node_ranks, full.node_ranks);
                assert_eq!(tight.stats, full.stats);
                let axes = |s: Size| {
                    if horizontal {
                        (s.height, s.width)
                    } else {
                        (s.width, s.height)
                    }
                };
                let (cross, along) = axes(full.bounds);
                let (new_cross, new_along) = axes(tight.bounds);
                assert_eq!(new_cross, cross);
                assert_eq!(
                    along,
                    baseline_along + f64::from(span - 1) * options.spacing.rank
                );
                assert_eq!(new_along, along - 111.0);
                let face = |r: Rect, side: Side| match side {
                    Side::Top => r.y,
                    Side::Bottom => r.y + r.height,
                    Side::Left => r.x,
                    Side::Right => r.x + r.width,
                };
                for (i, from, to, span) in [(2, 2, 3, span), (3, 3, 4, 1)] {
                    let route = &tight.edges[i];
                    assert_eq!(route.ports, full.edges[i].ports);
                    let [a, b] = route.points.as_slice() else {
                        panic!("expected a straight shaft")
                    };
                    assert_eq!(a.axis, route.ports.0.axis());
                    assert_eq!(b.axis, route.ports.1.axis());
                    let (across, from_axis, to_axis) = if horizontal {
                        (a.y - b.y, a.x, b.x)
                    } else {
                        (a.x - b.x, a.y, b.y)
                    };
                    assert_eq!(across, 0.0);
                    assert_eq!(from_axis, face(tight.nodes[from], route.ports.0));
                    assert_eq!(to_axis, face(tight.nodes[to], route.ports.1));
                    assert_eq!(
                        (to_axis - from_axis).abs(),
                        f64::from(span) * options.spacing.rank
                    );
                }
                assert_eq!(layout(&graph, &options).unwrap(), tight);
            }
        }
    }
}

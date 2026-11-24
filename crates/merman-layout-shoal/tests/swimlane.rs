use merman_layout_shoal::{
    Axis, Direction, EdgeGeometry, EdgeId, EdgeSpec, Endpoint, Graph, GroupId, GroupSpec, Insets,
    LayoutError, LayoutScratch, MarkerMetrics, NodeId, NodeSpec, Options, Point, PortSideHint,
    Rect, RoutePoint, RoutingStyle, Side, Size, WidthPolicy, WorkControl, WorkError, layout,
    layout_with_control,
};

const EPS: f64 = 1e-7;
const DIRECTIONS: [Direction; 4] = [
    Direction::Down,
    Direction::Up,
    Direction::Right,
    Direction::Left,
];

fn size(width: f64, height: f64) -> Size {
    Size { width, height }
}

fn group(seq: u32, parent: Option<u32>, title: Size) -> GroupSpec {
    let mut spec = GroupSpec::new(seq);
    spec.parent = parent.map(GroupId);
    spec.padding = Insets {
        top: 24.0,
        right: 17.0,
        bottom: 19.0,
        left: 21.0,
    };
    spec.title = Some(title);
    spec
}

fn node(seq: u32, parent: u32) -> NodeSpec {
    let mut spec = NodeSpec::new(size(48.0, 32.0), seq);
    spec.parent = Some(GroupId(parent));
    spec
}

fn edge(source: impl Into<Endpoint>, target: impl Into<Endpoint>) -> EdgeSpec {
    EdgeSpec::new(source.into(), target.into())
}

fn label(point: Point, size: Size) -> Rect {
    Rect {
        x: point.x - size.width / 2.0,
        y: point.y - size.height / 2.0,
        width: size.width,
        height: size.height,
    }
}

fn contains(outer: Rect, inner: Rect) -> bool {
    inner.x >= outer.x - EPS
        && inner.y >= outer.y - EPS
        && inner.x + inner.width <= outer.x + outer.width + EPS
        && inner.y + inner.height <= outer.y + outer.height + EPS
}

fn interior_overlap(a: Rect, b: Rect) -> bool {
    a.x.max(b.x) + EPS < (a.x + a.width).min(b.x + b.width)
        && a.y.max(b.y) + EPS < (a.y + a.height).min(b.y + b.height)
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() <= EPS, "{a} differs from {b}");
}

fn start_side(direction: Direction) -> Side {
    match direction {
        Direction::Down => Side::Top,
        Direction::Up => Side::Bottom,
        Direction::Left => Side::Right,
        Direction::Right => Side::Left,
    }
}

fn spans(direction: Direction, rect: Rect) -> ([f64; 2], [f64; 2]) {
    let [x, y] = [
        [rect.x, rect.x + rect.width],
        [rect.y, rect.y + rect.height],
    ];
    match start_side(direction).axis() {
        Axis::Vertical => (x, y),
        Axis::Horizontal => (y, x),
    }
}

fn on_face(point: RoutePoint, rect: Rect, side: Side) {
    assert!(contains(
        rect,
        label(
            Point {
                x: point.x,
                y: point.y
            },
            Size::default()
        )
    ));
    match side {
        Side::Top => near(point.y, rect.y),
        Side::Right => near(point.x, rect.x + rect.width),
        Side::Bottom => near(point.y, rect.y + rect.height),
        Side::Left => near(point.x, rect.x),
    }
}

fn along(end: RoutePoint, next: RoutePoint, side: Side) -> f64 {
    match side.axis() {
        Axis::Vertical => near(end.x, next.x),
        Axis::Horizontal => near(end.y, next.y),
    }
    match side {
        Side::Top => end.y - next.y,
        Side::Right => next.x - end.x,
        Side::Bottom => next.y - end.y,
        Side::Left => end.x - next.x,
    }
}

fn enters(points: &[RoutePoint], rect: Rect) -> bool {
    points.windows(2).any(|pair| {
        let [a, b] = [pair[0], pair[1]];
        let mut enter = 0.0_f64;
        let mut leave = 1.0_f64;
        for (from, to, min, length) in [
            (a.x, b.x, rect.x, rect.width),
            (a.y, b.y, rect.y, rect.height),
        ] {
            let (lo, hi) = (min + EPS, min + length - EPS);
            if lo >= hi {
                return false;
            }
            if from == to {
                if from <= lo || from >= hi {
                    return false;
                }
            } else {
                let p = (lo - from) / (to - from);
                let q = (hi - from) / (to - from);
                enter = enter.max(p.min(q));
                leave = leave.min(p.max(q));
            }
        }
        enter < leave
    })
}

fn ends(geometry: &EdgeGeometry) -> [RoutePoint; 2] {
    [geometry.points[0], *geometry.points.last().unwrap()]
}

fn shared_ranks() -> Graph {
    let mut graph = Graph::default();
    graph.groups = vec![
        group(0, None, size(150.0, 94.0)),
        group(1, None, size(39.0, 18.0)),
        group(2, None, size(60.0, 18.0)),
        group(3, None, size(46.0, 20.0)),
    ];
    graph.lane_order = [2, 0, 3, 1].map(GroupId).into();
    graph.nodes = [2, 3, 2, 3, 2, 3, 1]
        .into_iter()
        .enumerate()
        .map(|(i, owner)| node(i as u32, owner))
        .collect();
    graph.edges = [(0, 3), (1, 2), (2, 5), (3, 4)]
        .map(|(a, b)| edge(NodeId(a), NodeId(b)))
        .into();
    graph
}

#[test]
fn stitched_lane_edges_report_the_crossing_between_distinct_edges() {
    let mut graph = Graph::default();
    graph.groups = [GroupSpec::new(0), GroupSpec::new(1)].into();
    graph.lane_order = [GroupId(0), GroupId(1)].into();
    graph.nodes = [node(0, 0), node(1, 1), node(2, 0), node(3, 1)].into();
    graph.edges = [edge(NodeId(0), NodeId(3)), edge(NodeId(1), NodeId(2))].into();
    for direction in DIRECTIONS {
        graph.direction = direction;
        let drawing = layout(&graph, &Options::default()).unwrap();
        assert_eq!(drawing.stats.routed_crossings, 1, "{direction:?}");
    }
}

#[test]
fn one_rank_grid_spans_declared_lanes_empty_tracks_and_islands_in_four_directions() {
    let mut graph = shared_ranks();
    let mut options = Options::default();
    options.capture_node_ranks = true;
    for direction in DIRECTIONS {
        graph.direction = direction;
        let drawing = layout(&graph, &options).unwrap();
        let flow = start_side(direction);
        let mut previous = None;
        let content = spans(direction, drawing.groups[2].rect).1;
        for &GroupId(lane) in &graph.lane_order {
            let lane = lane as usize;
            let geometry = &drawing.groups[lane];
            let (cross, along) = spans(direction, geometry.rect);
            assert!(cross[1] > cross[0]);
            near(along[0], content[0]);
            near(along[1], content[1]);
            if let Some(border) = previous {
                near(cross[0], border);
            }
            previous = Some(cross[1]);
            assert_eq!(geometry.effective_direction, direction);
            assert_eq!(geometry.parent, None);
            let title = geometry.title_center.unwrap();
            let measured = label(title, graph.groups[lane].title.unwrap());
            assert!(contains(geometry.rect, measured));
            let (title_cross, _) = spans(direction, measured);
            near(
                title_cross[0].midpoint(title_cross[1]),
                cross[0].midpoint(cross[1]),
            );
            let coord = match flow.axis() {
                Axis::Vertical => title.y,
                Axis::Horizontal => title.x,
            };
            let start = matches!(flow, Side::Top | Side::Left);
            assert_eq!(coord < along[0].midpoint(along[1]), start);
        }
        for (i, &rect) in drawing.nodes.iter().enumerate() {
            let lane = graph.nodes[i].parent.unwrap().0 as usize;
            let shell = drawing.groups[lane].rect;
            let spec = &graph.groups[lane];
            let pad = spec.padding;
            let inside = Rect {
                x: shell.x + pad.left,
                y: shell.y + pad.top,
                width: shell.width - pad.left - pad.right,
                height: shell.height - pad.top - pad.bottom,
            };
            assert!(contains(inside, rect), "{direction:?}: {inside:?} {rect:?}");
            assert!(!interior_overlap(
                label(
                    drawing.groups[lane].title_center.unwrap(),
                    spec.title.unwrap()
                ),
                rect
            ));
        }
        let ranks = &drawing.node_ranks;
        assert!(ranks[0] < ranks[2] && ranks[2] < ranks[4]);
        for (a, b) in [(0, 1), (2, 3), (4, 5), (0, 6)] {
            assert_eq!(ranks[a], ranks[b]);
            let a = spans(direction, drawing.nodes[a]).1;
            let b = spans(direction, drawing.nodes[b]).1;
            near(a[0], b[0]);
        }
        assert!(drawing.border_stitches.is_empty());
        assert_eq!(drawing, layout(&graph, &options).unwrap());
    }
}

#[test]
fn ordinary_nested_groups_keep_their_parents_and_one_stitch_per_crossed_group() {
    let mut graph = Graph::default();
    graph.groups = vec![
        group(0, None, size(93.0, 19.0)),
        group(1, None, size(110.0, 19.0)),
        group(2, Some(0), size(84.0, 22.0)),
        group(3, Some(2), size(48.0, 19.0)),
        group(4, Some(1), size(86.0, 24.0)),
    ];
    graph.lane_order = vec![GroupId(0), GroupId(1)];
    graph.nodes = vec![node(0, 3), node(1, 4), node(2, 0), node(3, 1)];
    graph.edges = vec![edge(NodeId(0), NodeId(1)), edge(NodeId(2), NodeId(3))];
    graph.edges[0].label = Some(size(62.0, 21.0));
    graph.edges[0].end_marker = MarkerMetrics {
        width: 11.0,
        length: 8.0,
    };
    graph.edges[0].end_label = Some(size(25.0, 14.0));
    for direction in DIRECTIONS {
        graph.direction = direction;
        let drawing = layout(&graph, &Options::default()).unwrap();
        assert_eq!(drawing.groups[2].parent, Some(GroupId(0)));
        assert_eq!(drawing.groups[3].parent, Some(GroupId(2)));
        assert_eq!(drawing.groups[4].parent, Some(GroupId(1)));
        for (group, parent) in [(2, 0), (3, 2), (4, 1)] {
            assert!(contains(
                drawing.groups[parent].rect,
                drawing.groups[group].rect
            ));
        }
        let path = &drawing.edges[0];
        let stitches = &drawing.border_stitches;
        assert_eq!(
            stitches.iter().map(|s| s.group.0).collect::<Vec<_>>(),
            [3, 2, 4]
        );
        for seam in stitches {
            assert_eq!(seam.edge, EdgeId(0));
            on_face(
                path.points[seam.point],
                drawing.groups[seam.group.0 as usize].rect,
                seam.side,
            );
        }
        assert!(stitches.windows(2).all(|s| s[0].point < s[1].point));
        let [source, target] = ends(path);
        on_face(source, drawing.nodes[0], path.ports.0);
        on_face(target, drawing.nodes[1], path.ports.1);
        assert!(path.label_center.is_some() && path.end_label_center.is_some());
        for (index, geometry) in drawing.groups.iter().enumerate() {
            let title = label(
                geometry.title_center.unwrap(),
                graph.groups[index].title.unwrap(),
            );
            assert!(!enters(&path.points, title), "{direction:?} title {index}");
        }
    }
}

#[test]
fn shell_to_member_respects_fixed_contacts_inward_marker_shafts_and_mixed_labels() {
    let mut graph = Graph::default();
    graph.groups = vec![group(0, None, size(100.0, 48.0))];
    graph.lane_order = vec![GroupId(0)];
    graph.nodes = vec![node(0, 0)];
    let mut connection = edge(GroupId(0), NodeId(0));
    connection.start_marker = MarkerMetrics {
        length: 13.0,
        width: 12.0,
    };
    connection.end_marker = MarkerMetrics {
        length: 8.0,
        width: 8.0,
    };
    connection.label = Some(size(49.0, 19.0));
    connection.start_label = Some(size(42.0, 13.0));
    connection.end_label = Some(size(38.0, 13.0));
    let mut options = Options::default();
    options.routing = RoutingStyle::Orthogonal;
    for direction in DIRECTIONS {
        graph.direction = direction;
        let side = start_side(direction);
        connection.source_port = PortSideHint::FixedOffset { side, offset: 5.0 };
        graph.edges = vec![connection];
        let drawing =
            layout(&graph, &options).unwrap_or_else(|err| panic!("{direction:?}: {err:?}"));
        assert_eq!(drawing.stats.dummy_nodes, 0);
        assert!(drawing.border_stitches.is_empty());
        let group = &drawing.groups[0];
        let path = &drawing.edges[0];
        let [source, target] = ends(path);
        on_face(source, group.rect, side);
        on_face(target, drawing.nodes[0], path.ports.1);
        assert_eq!(path.ports, (side, side));
        let axis_value = match side.axis() {
            Axis::Vertical => source.x - group.rect.center().x,
            Axis::Horizontal => source.y - group.rect.center().y,
        };
        near(axis_value, 5.0);
        assert!(along(source, path.points[1], side) <= -connection.start_marker.length);
        assert!(
            along(target, path.points[path.points.len() - 2], side) >= connection.end_marker.length
        );
        let title = label(group.title_center.unwrap(), graph.groups[0].title.unwrap());
        assert!(!enters(&path.points, title));
        let middle = label(path.label_center.unwrap(), connection.label.unwrap());
        let start = label(
            path.start_label_center.unwrap(),
            connection.start_label.unwrap(),
        );
        let end = label(
            path.end_label_center.unwrap(),
            connection.end_label.unwrap(),
        );
        for (a, b) in [
            (title, middle),
            (title, start),
            (title, end),
            (middle, start),
            (middle, end),
            (start, end),
        ] {
            assert!(!interior_overlap(a, b), "{direction:?} {a:?} {b:?}");
        }
        assert!(!enters(&path.points, start));
        assert!(!enters(&path.points, end));
    }
}

#[test]
fn empty_lane_shells_accept_ordinary_parallel_edges_and_distinct_self_loop_ports() {
    let mut graph = Graph::default();
    graph.groups = vec![
        group(0, None, size(67.0, 15.0)),
        group(1, None, size(152.0, 20.0)),
    ];
    graph.lane_order = vec![GroupId(0), GroupId(1)];
    graph.nodes = vec![node(0, 0)];
    let mut marked = edge(NodeId(0), GroupId(1));
    marked.end_marker = MarkerMetrics {
        length: 9.0,
        width: 11.0,
    };
    let mut loop_edge = edge(GroupId(1), GroupId(1));
    loop_edge.label = Some(size(53.0, 17.0));
    graph.edges = vec![marked, marked, edge(GroupId(0), GroupId(1)), loop_edge];
    for direction in DIRECTIONS {
        graph.direction = direction;
        let drawing = layout(&graph, &Options::default())
            .unwrap_or_else(|err| panic!("{direction:?}: {err:?}"));
        assert_eq!(drawing.edges.len(), graph.edges.len());
        let to = drawing.groups[1].rect;
        let contacts = drawing.edges.iter().map(ends).collect::<Vec<_>>();
        for (id, [source, target]) in contacts.iter().copied().enumerate() {
            let path = &drawing.edges[id];
            let from = match id {
                0 | 1 => drawing.nodes[0],
                2 => drawing.groups[0].rect,
                _ => to,
            };
            on_face(source, from, path.ports.0);
            on_face(target, to, path.ports.1);
            for (index, geometry) in drawing.groups.iter().enumerate() {
                let title = label(
                    geometry.title_center.unwrap(),
                    graph.groups[index].title.unwrap(),
                );
                assert!(
                    !enters(&path.points, title),
                    "{direction:?} edge {id}, title {index}"
                );
            }
        }
        assert_ne!(contacts[0][1], contacts[1][1]);
        assert_ne!(contacts[3][0], contacts[3][1]);
        assert!(drawing.edges[3].points.len() >= 4);
        let loop_label = label(
            drawing.edges[3].label_center.unwrap(),
            loop_edge.label.unwrap(),
        );
        assert!(!enters(&drawing.edges[3].points, loop_label));
        assert!(!interior_overlap(to, loop_label));
    }
}

#[test]
fn an_automatic_contact_can_change_faces_when_a_label_blocks_the_narrow_face() {
    let mut graph = Graph::default();
    graph.direction = Direction::Right;
    graph.lane_order = vec![GroupId(0), GroupId(1)];
    graph.groups = vec![
        group(0, None, size(59.0, 18.0)),
        group(1, None, size(146.0, 39.0)),
    ];
    for shell in &mut graph.groups {
        shell.padding.right = 19.0;
        shell.padding.bottom = 16.0;
    }
    graph.nodes = vec![node(0, 0), node(1, 1)];
    graph.nodes[0].size = size(49.0, 20.0);
    graph.nodes[1].size = size(28.0, 23.0);
    let mut own = edge(GroupId(0), NodeId(0));
    own.end_marker = MarkerMetrics {
        length: 12.0,
        width: 13.0,
    };
    let mut other = edge(NodeId(1), NodeId(0));
    other.end_marker = MarkerMetrics {
        length: 6.0,
        width: 11.0,
    };
    other.end_label = Some(size(21.0, 14.0));
    graph.edges = vec![own, other];
    let mut options = Options::default();
    options.routing = RoutingStyle::Orthogonal;
    let drawing = layout(&graph, &options).unwrap();
    let route = &drawing.edges[0];
    let target = ends(route)[1];
    assert_ne!(route.ports.1, Side::Left);
    on_face(target, drawing.nodes[0], route.ports.1);
    assert!(
        along(target, route.points[route.points.len() - 2], route.ports.1) >= own.end_marker.length
    );
    let foreign = label(
        drawing.edges[1].end_label_center.unwrap(),
        other.end_label.unwrap(),
    );
    assert!(!enters(&route.points, foreign));

    graph.edges[0].target_port = PortSideHint::FixedOffset {
        side: Side::Left,
        offset: 0.0,
    };
    assert!(matches!(
        layout(&graph, &options),
        Err(LayoutError::InvalidInput(_))
    ));
}

#[test]
fn invalid_lane_membership_and_explicit_width_fitting_report_errors() {
    let graph = shared_ranks();
    let mut options = Options::default();
    let invalid = |graph: &Graph| {
        assert!(matches!(
            layout(graph, &Options::default()),
            Err(LayoutError::InvalidInput(_))
        ));
    };
    for change in [
        (|graph: &mut Graph| graph.lane_order.push(GroupId(2))) as fn(&mut Graph),
        |graph| graph.lane_order[0] = GroupId(65),
        |graph| graph.groups[0].parent = Some(GroupId(2)),
        |graph| {
            graph.lane_order.pop();
        },
        |graph| graph.nodes[0].parent = None,
    ] {
        let mut invalid_graph = graph.clone();
        change(&mut invalid_graph);
        invalid(&invalid_graph);
    }
    options.width_policy = WidthPolicy::FitWidth {
        max_width: 400.0,
        allow_turn: false,
        turn_breakpoint: 0.0,
        fold_wide_ranks: false,
    };
    assert!(matches!(
        layout(&graph, &options),
        Err(LayoutError::Unsupported(_))
    ));
}

#[test]
fn lane_errors_leave_scratch_reusable_and_empty_shells_keep_extent() {
    #[derive(Default)]
    struct Meter {
        units: usize,
        limit: Option<usize>,
    }
    impl WorkControl for Meter {
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.units += units;
            if self.limit.is_some_and(|cap| self.units > cap) {
                Err(WorkError::Interrupted)
            } else {
                Ok(())
            }
        }
        fn check(&mut self) -> Result<(), WorkError> {
            Ok(())
        }
    }

    let graph = shared_ranks();
    let options = Options::default();
    let mut scratch = LayoutScratch::default();
    let mut meter = Meter::default();
    let expected = layout_with_control(&graph, &options, &mut scratch, &mut meter).unwrap();
    for limit in [0, meter.units / 2, meter.units - 1] {
        let mut stopped = Meter {
            limit: Some(limit),
            ..Default::default()
        };
        assert_eq!(
            layout_with_control(&graph, &options, &mut scratch, &mut stopped),
            Err(LayoutError::Work(WorkError::Interrupted))
        );
        assert_eq!(
            layout_with_control(&graph, &options, &mut scratch, &mut Meter::default()).unwrap(),
            expected
        );
    }

    let mut empty = graph;
    empty.nodes.clear();
    empty.edges.clear();
    empty.groups[0].title = None;
    empty.groups[0].padding = Insets::default();
    empty.groups[0].min_size = Size::default();
    let drawing = layout(&empty, &options).unwrap();
    assert!(drawing.nodes.is_empty() && drawing.edges.is_empty());
    assert!(drawing.groups[0].rect.width > 0.0 && drawing.groups[0].rect.height > 0.0);
    assert!(contains(
        Rect {
            width: drawing.bounds.width,
            height: drawing.bounds.height,
            ..Rect::default()
        },
        drawing.groups[0].rect
    ));
}

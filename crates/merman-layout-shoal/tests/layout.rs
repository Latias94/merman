use merman_layout_shoal::{
    Axis, Direction, EdgeSpec, Endpoint, Graph, GroupId, GroupSpec, LayoutError, LayoutOutput,
    LayoutScratch, NodeId, NodeSpec, NoopWorkControl, Options, Point, PortSideHint, Rect,
    RoutePoint, RoutingStyle, Side, Size, WorkControl, WorkError, layout, layout_with_control,
};

const DIRECTIONS: [Direction; 4] = [
    Direction::Down,
    Direction::Up,
    Direction::Right,
    Direction::Left,
];
const EPS: f64 = 1e-8;

fn size(width: f64, height: f64) -> Size {
    Size { width, height }
}

fn edge(source: usize, target: usize) -> EdgeSpec {
    EdgeSpec::new(
        NodeId(source.try_into().unwrap()).into(),
        NodeId(target.try_into().unwrap()).into(),
    )
}

fn node(endpoint: Endpoint) -> usize {
    let Endpoint::Node(id) = endpoint else {
        panic!("expected a node endpoint")
    };
    id.0 as usize
}

fn graph(sizes: Vec<Size>, edges: Vec<EdgeSpec>) -> Graph {
    let mut g = Graph::default();
    g.nodes = sizes
        .into_iter()
        .enumerate()
        .map(|(seq, size)| NodeSpec::new(size, seq as u32))
        .collect();
    g.edges = edges;
    g
}

fn position(p: &RoutePoint) -> Point {
    Point { x: p.x, y: p.y }
}

fn along(direction: Direction, p: Point) -> f64 {
    match direction {
        Direction::Down => p.y,
        Direction::Up => -p.y,
        Direction::Right => p.x,
        Direction::Left => -p.x,
    }
}

#[derive(Debug, Clone, Copy)]
struct BoxExtents {
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
}

impl BoxExtents {
    fn new(center: Point, size: Size) -> Self {
        Self {
            x0: center.x - size.width * 0.5,
            x1: center.x + size.width * 0.5,
            y0: center.y - size.height * 0.5,
            y1: center.y + size.height * 0.5,
        }
    }

    fn overlaps(self, other: Self) -> bool {
        self.x0.max(other.x0) + EPS < self.x1.min(other.x1)
            && self.y0.max(other.y0) + EPS < self.y1.min(other.y1)
    }

    /// Intersect the segment's parameter interval with the open interior in each axis.
    fn crosses(self, a: Point, b: Point) -> bool {
        let (mut enter, mut leave) = (0.0_f64, 1.0_f64);
        for (low, high, start, end) in [(self.x0, self.x1, a.x, b.x), (self.y0, self.y1, a.y, b.y)]
        {
            let (low, high) = (low + EPS, high - EPS);
            if low >= high {
                return false;
            }
            if start == end {
                if start <= low || start >= high {
                    return false;
                }
            } else {
                let t1 = (low - start) / (end - start);
                let t2 = (high - start) / (end - start);
                enter = enter.max(t1.min(t2));
                leave = leave.min(t1.max(t2));
            }
        }
        enter < leave
    }
}

fn on_segment(p: Point, a: Point, b: Point) -> bool {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let t = if dx.abs() > dy.abs() {
        (p.x - a.x) / dx
    } else {
        (p.y - a.y) / dy
    };
    (-EPS..=1.0 + EPS).contains(&t) && (a.x + t * dx - p.x).hypot(a.y + t * dy - p.y) < EPS
}

fn assert_contact(rect: Rect, p: &RoutePoint, next: &RoutePoint, side: Side) {
    assert_eq!(p.axis, side.axis());
    match side {
        Side::Top | Side::Bottom => {
            assert!((rect.x..=rect.x + rect.width).contains(&p.x));
            assert_eq!(
                p.y,
                if side == Side::Top {
                    rect.y
                } else {
                    rect.y + rect.height
                }
            );
            assert_eq!(next.x, p.x);
        }
        Side::Left | Side::Right => {
            assert!((rect.y..=rect.y + rect.height).contains(&p.y));
            assert_eq!(
                p.x,
                if side == Side::Left {
                    rect.x
                } else {
                    rect.x + rect.width
                }
            );
            assert_eq!(next.y, p.y);
        }
    }
}

fn check_geometry(g: &Graph, options: &Options, drawing: &LayoutOutput) {
    assert_eq!(drawing.direction, g.direction);
    assert_eq!(drawing.nodes.len(), g.nodes.len());
    assert_eq!(drawing.edges.len(), g.edges.len());
    assert!(drawing.groups.is_empty());
    let nodes: Vec<_> = drawing
        .nodes
        .iter()
        .zip(&g.nodes)
        .map(|(r, spec)| {
            assert_eq!(size(r.width, r.height), spec.size);
            BoxExtents {
                x0: r.x,
                y0: r.y,
                x1: r.x + r.width,
                y1: r.y + r.height,
            }
        })
        .collect();
    let mut extents = nodes.clone();
    let mut labels = Vec::new();
    let ports = match g.direction {
        Direction::Down => (Side::Bottom, Side::Top),
        Direction::Up => (Side::Top, Side::Bottom),
        Direction::Right => (Side::Right, Side::Left),
        Direction::Left => (Side::Left, Side::Right),
    };
    for (id, (route, e)) in drawing.edges.iter().zip(&g.edges).enumerate() {
        let (a, b) = (node(e.source), node(e.target));
        let pts = &route.points;
        let last = pts.len() - 1;
        assert!((4..=8).contains(&pts.len()));
        assert_eq!(route.ports, ports);
        assert_contact(drawing.nodes[a], &pts[0], &pts[1], ports.0);
        assert_contact(drawing.nodes[b], &pts[last], &pts[last - 1], ports.1);
        assert!(!route.reversed_for_ranking);
        assert!(route.start_label_center.is_none() && route.end_label_center.is_none());
        let along = |p: &RoutePoint| along(g.direction, position(p));
        for pair in pts.windows(2) {
            if options.routing == RoutingStyle::Orthogonal {
                match pair[1].axis {
                    Axis::Vertical => assert_eq!(pair[0].x, pair[1].x),
                    Axis::Horizontal => assert_eq!(pair[0].y, pair[1].y),
                }
                assert!(along(&pair[1]) >= along(&pair[0]));
            } else {
                assert_eq!(pair[1].axis, ports.1.axis());
                assert!(along(&pair[1]) > along(&pair[0]), "edge {id} {pair:?}");
            }
        }
        let start = along(&pts[1]) - along(&pts[0]);
        let end = along(&pts[last]) - along(&pts[last - 1]);
        assert!(start > 0.0 && start + EPS >= e.start_marker.length);
        assert!(end > 0.0 && end + EPS >= e.end_marker.length);
        assert!(
            along(&pts[last]) - along(&pts[0]) + EPS
                >= f64::from(e.min_rank_span.max(1)) * options.spacing.rank
        );
        assert_eq!(e.label.is_some(), route.label_center.is_some());
        if let (Some(p), Some(s)) = (route.label_center, e.label) {
            assert!(
                pts.windows(2)
                    .any(|seg| on_segment(p, position(&seg[0]), position(&seg[1]))),
                "label off route {id}"
            );
            let rect = BoxExtents::new(p, s);
            labels.push((id, rect));
            extents.push(rect);
        }
    }
    for (i, rect) in extents.iter().enumerate() {
        for other in &extents[i + 1..] {
            assert!(
                !rect.overlaps(*other),
                "overlapping boxes {rect:?} {other:?}"
            );
        }
    }
    for (id, (route, e)) in drawing.edges.iter().zip(&g.edges).enumerate() {
        for pair in route.points.windows(2) {
            let (a, b) = (position(&pair[0]), position(&pair[1]));
            for (index, rect) in nodes.iter().enumerate() {
                if index != node(e.source) && index != node(e.target) {
                    assert!(
                        !rect.crosses(a, b),
                        "edge {id} crosses node {index}: {pair:?} {rect:?}"
                    );
                }
            }
            for &(owner, rect) in &labels {
                if owner != id {
                    assert!(
                        !rect.crosses(a, b),
                        "edge {id} crosses label {owner}: {pair:?} {rect:?}"
                    );
                }
            }
        }
        extents.extend(
            route
                .points
                .iter()
                .map(|p| BoxExtents::new(position(p), Size::default())),
        );
    }
    let bounds = drawing.bounds;
    for (expected, actual) in [
        (extents.iter().map(|r| r.x0).reduce(f64::min), 0.0),
        (extents.iter().map(|r| r.x1).reduce(f64::max), bounds.width),
        (extents.iter().map(|r| r.y0).reduce(f64::min), 0.0),
        (extents.iter().map(|r| r.y1).reduce(f64::max), bounds.height),
    ] {
        assert!((expected.unwrap_or(0.0) - actual).abs() < EPS);
    }
}

#[test]
fn empty_and_interleaved_components_preserve_occurrences_and_spacing() {
    let mut options = Options::default();
    options.spacing.node = 16.0;
    options.spacing.component = 23.0;
    let mut g = graph(
        vec![size(18.0, 42.0); 7],
        vec![edge(6, 2), edge(3, 0), edge(5, 3)],
    );
    g.edges[1].label = Some(size(97.0, 23.0));
    for direction in DIRECTIONS {
        let mut empty = Graph::default();
        empty.direction = direction;
        let d = layout(&empty, &options).unwrap();
        check_geometry(&empty, &options, &d);
        assert_eq!(d.stats.components, 0);
        g.direction = direction;
        let d = layout(&g, &options).unwrap();
        check_geometry(&g, &options, &d);
        assert_eq!(d.stats.components, 4);
        let cross = |rect: Rect| match direction {
            Direction::Down | Direction::Up => (rect.x, rect.x + rect.width),
            _ => (rect.y, rect.y + rect.height),
        };
        let label = d.edges[1].label_center.unwrap();
        let size = g.edges[1].label.unwrap();
        let label_end = match direction {
            Direction::Down | Direction::Up => label.x + size.width * 0.5,
            _ => label.y + size.height * 0.5,
        };
        assert!(
            cross(d.nodes[1]).0 >= cross(d.nodes[0]).1.max(label_end) + options.spacing.component
        );
        assert!(cross(d.nodes[2]).0 >= cross(d.nodes[1]).1 + options.spacing.component);
        assert!(cross(d.nodes[4]).0 >= cross(d.nodes[2]).1 + options.spacing.component);
    }
}

#[test]
fn straight_edges_keep_both_exterior_waypoints_and_marker_setbacks() {
    let mut options = Options::default();
    options.spacing.node = 0.0;
    options.spacing.rank = 0.0;
    let mut g = graph(vec![size(73.0, 11.0), size(14.0, 53.0)], vec![edge(1, 0)]);
    g.edges[0].label = Some(size(131.0, 61.0));
    g.edges[0].start_marker.length = 79.0;
    g.edges[0].end_marker.length = 57.0;
    for direction in DIRECTIONS {
        g.direction = direction;
        let d = layout(&g, &options).unwrap();
        check_geometry(&g, &options, &d);
        assert_eq!(d.edges[0].points.len(), 4);
    }
    g.nodes.iter_mut().for_each(|n| n.size = Size::default());
    g.edges[0] = edge(1, 0);
    check_geometry(&g, &options, &layout(&g, &options).unwrap());
}

#[test]
fn longest_path_satisfies_competing_minimum_rank_spans() {
    let mut options = Options::default();
    options.spacing.rank = 100.0;
    let mut g = graph(
        vec![size(10.0, 10.0); 3],
        vec![edge(1, 2), edge(0, 2), edge(0, 1)],
    );
    for (e, span) in g.edges.iter_mut().zip([2, 6, 3]) {
        e.min_rank_span = span;
    }
    let d = layout(&g, &options).unwrap();
    check_geometry(&g, &options, &d);
    assert_eq!(d.nodes[1].y - d.nodes[0].y, 310.0);
    assert_eq!(d.nodes[2].y - d.nodes[1].y, 310.0);
    g.edges.fill(edge(0, 1));
    let one = layout(&g, &options).unwrap();
    for e in &mut g.edges {
        e.min_rank_span = 0;
    }
    assert_eq!(layout(&g, &options).unwrap(), one);
}

#[test]
fn parallel_occurrences_have_distinct_paths_even_for_point_nodes() {
    let mut g = graph(vec![Size::default(); 2], vec![edge(0, 1); 9]);
    let mut options = Options::default();
    options.spacing.node = 0.0;
    options.spacing.rank = 0.0;
    for (i, e) in g.edges.iter_mut().enumerate().step_by(2) {
        e.label = Some(size(10.0 + 7.0 * i as f64, 31.0 - i as f64));
    }
    for direction in DIRECTIONS {
        g.direction = direction;
        for style in [
            RoutingStyle::Curved,
            RoutingStyle::Orthogonal,
            RoutingStyle::Polyline,
        ] {
            options.routing = style;
            let d = layout(&g, &options).unwrap();
            check_geometry(&g, &options, &d);
            for (i, route) in d.edges.iter().enumerate() {
                assert!(
                    d.edges[i + 1..]
                        .iter()
                        .all(|other| other.points != route.points)
                );
            }
        }
    }
}

#[test]
fn mixed_fans_long_edges_and_parallel_labels_clear_unrelated_boxes_in_all_directions() {
    for seed in 0..24 {
        let nodes = (0..10)
            .map(|i| size((31 * i % 73) as f64, (17 * i % 51) as f64))
            .collect();
        let mut g = graph(nodes, Vec::new());
        let mut options = Options::default();
        options.spacing.node = f64::from(seed % 3);
        options.spacing.rank = f64::from(seed % 7);
        let mut state = seed + 1u32;
        for i in 0..10 {
            for j in (i + 1..10).rev() {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                if state % 5 >= 2 {
                    continue;
                }
                let mut e = edge((3 * i + 7) % 10, (3 * j + 7) % 10);
                e.min_rank_span = state % 4;
                e.start_marker.length = f64::from(state % 13);
                e.end_marker.length = f64::from(state % 9);
                if state & 4 != 0 {
                    e.label = Some(size(f64::from(state % 121), f64::from(state % 63)));
                }
                g.edges.push(e);
                if state.is_multiple_of(11) {
                    g.edges.push(e);
                }
            }
        }
        for direction in DIRECTIONS {
            g.direction = direction;
            let d = layout(&g, &options).unwrap();
            check_geometry(&g, &options, &d);
            assert_eq!(layout(&g, &options).unwrap(), d);
        }
    }
}

#[test]
fn cycles_and_invalid_measurements_have_distinct_errors() {
    let options = Options::default();
    let mut g = graph(
        vec![size(10.0, 8.0); 3],
        vec![edge(0, 1), edge(1, 2), edge(2, 1)],
    );
    assert_eq!(
        layout(&g, &options),
        Err(LayoutError::Unsupported("directed cycles"))
    );
    g.edges[2] = edge(2, 2);
    assert_eq!(
        layout(&g, &options),
        Err(LayoutError::Unsupported("self-loops"))
    );
    let invalid = |g: &Graph| {
        assert!(matches!(
            layout(g, &options),
            Err(LayoutError::InvalidInput(_))
        ))
    };
    g.edges[2] = edge(0, 3);
    invalid(&g);
    g.edges[2].source = GroupId(0).into();
    invalid(&g);
    g.edges.truncate(1);
    for value in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut bad = options;
        bad.spacing.rank = value;
        assert!(matches!(
            layout(&g, &bad),
            Err(LayoutError::InvalidInput(_))
        ));
        let mut bad = options;
        bad.spacing.node = value;
        assert!(matches!(
            layout(&g, &bad),
            Err(LayoutError::InvalidInput(_))
        ));
        for s in [size(1.0, value), size(value, 1.0)] {
            let mut bad = g.clone();
            bad.nodes[0].size = s;
            invalid(&bad);
            let mut bad = g.clone();
            bad.edges[0].label = Some(s);
            invalid(&bad);
        }
        let mut bad = g.clone();
        bad.edges[0].start_marker.length = value;
        invalid(&bad);
        let mut bad = g.clone();
        bad.edges[0].end_marker.width = value;
        invalid(&bad);
        let mut bad = g.clone();
        bad.nodes[0].port_span = Some(value);
        invalid(&bad);
    }
    let mut bad = g.clone();
    bad.nodes[0].anchor_y = Some(9.0);
    invalid(&bad);
    let mut bad = g.clone();
    bad.edges[0].weight = 0.0;
    invalid(&bad);
    let mut bad = g.clone();
    bad.nodes[0].parent = Some(GroupId(4));
    invalid(&bad);
    let mut bad = g.clone();
    bad.edges[0].target_port = PortSideHint::FixedOffset {
        side: Side::Top,
        offset: 6.0,
    };
    invalid(&bad);
    g.edges[0].start_label = Some(Size::default());
    assert_eq!(
        layout(&g, &options),
        Err(LayoutError::Unsupported("terminal labels"))
    );
    g.groups.push(GroupSpec::new(0));
    assert_eq!(
        layout(&g, &options),
        Err(LayoutError::Unsupported("subgraphs"))
    );
}
#[derive(Default)]
struct Meter {
    total: usize,
    budget: Option<usize>,
    checks: usize,
    reject_check: Option<usize>,
}

impl WorkControl for Meter {
    fn check(&mut self) -> Result<(), WorkError> {
        self.checks += 1;
        if self.reject_check == Some(self.checks) {
            Err(WorkError::Interrupted)
        } else {
            Ok(())
        }
    }
    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        assert!(units > 0);
        let next = self
            .total
            .checked_add(units)
            .ok_or(WorkError::ArithmeticOverflow)?;
        if self.budget.is_some_and(|limit| next > limit) {
            return Err(WorkError::Interrupted);
        }
        self.total = next;
        Ok(())
    }
}

fn measured(g: &Graph, options: &Options) -> (LayoutOutput, Meter) {
    let mut meter = Meter::default();
    let drawing =
        layout_with_control(g, options, &mut LayoutScratch::default(), &mut meter).unwrap();
    (drawing, meter)
}

#[test]
fn maximum_logical_rank_never_allocates_by_span_and_overflow_is_explicit() {
    let mut g = graph(vec![size(2.0, 2.0); 3], vec![edge(0, 1)]);
    let mut opt = Options::default();
    opt.spacing.rank = 0.0;
    let (ordinary, meter) = measured(&g, &opt);
    g.edges[0].min_rank_span = u32::MAX;
    let (wide, wide_meter) = measured(&g, &opt);
    assert_eq!(wide, ordinary);
    assert_eq!(wide_meter.total, meter.total);
    opt.spacing.rank = 1024.0 / f64::from(u32::MAX);
    let (d, span_meter) = measured(&g, &opt);
    assert_eq!(span_meter.total, meter.total);
    assert!(d.nodes[1].y - d.nodes[0].y >= 1026.0);
    g.edges.push(edge(1, 2));
    let overflow = Err(LayoutError::Work(WorkError::ArithmeticOverflow));
    assert_eq!(layout(&g, &opt), overflow);
    g.edges.pop();
    opt.spacing.rank = f64::MAX;
    assert_eq!(layout(&g, &opt), overflow);
    opt.spacing.rank = 0.0;
    g.edges[0].start_marker.length = f64::MAX;
    assert!(matches!(
        layout(&g, &opt),
        Err(LayoutError::InvalidInput(_))
    ));
}

#[test]
fn coarse_float_coordinates_preserve_marker_room_or_reject_collapsed_geometry() {
    let mut g = graph(
        vec![size(16.0, 2f64.powi(58)), size(8.0, 512.0)],
        vec![edge(0, 1)],
    );
    g.edges[0].start_marker.length = 75.0;
    g.edges[0].end_marker.length = 128.0;
    let mut opt = Options::default();
    opt.spacing.rank = 1024.0;
    let d = layout(&g, &opt).unwrap();
    let pts = &d.edges[0].points;
    assert!(pts[1].y - pts[0].y >= 75.0);
    assert!(d.nodes[1].y - pts[pts.len() - 2].y >= 128.0);
    let mut packed = graph(
        vec![size(2f64.powi(62), 2.0), Size::default(), Size::default()],
        vec![edge(1, 2); 2],
    );
    opt.spacing.component = 0.0;
    assert!(matches!(
        layout(&packed, &opt),
        Err(LayoutError::InvalidInput(_))
    ));
    packed.nodes.truncate(1);
    packed.edges.clear();
    assert_eq!(
        layout(&packed, &opt).unwrap().bounds.width,
        packed.nodes[0].size.width
    );
    packed.nodes[0].size.width = f64::from_bits(1);
    assert!(matches!(
        layout(&packed, &opt),
        Err(LayoutError::InvalidInput(_))
    ));
}

#[test]
fn disjoint_long_intervals_reuse_a_lane_and_cross_only_their_own_label() {
    let make = |groups: usize| {
        let last = groups * 3;
        let mut g = graph(
            vec![size(24.0, 12.0); last + 1],
            (0..last).map(|i| edge(i, i + 1)).collect(),
        );
        for i in (0..last).step_by(3) {
            let mut e = edge(i, i + 3);
            e.label = Some(size(81.0, 11.0));
            g.edges.push(e);
        }
        g
    };
    let opt = Options::default();
    let small = layout(&make(1), &opt).unwrap();
    let g = make(30);
    let d = layout(&g, &opt).unwrap();
    check_geometry(&g, &opt, &d);
    assert_eq!(small.bounds.width, d.bounds.width);
    let lane = small.edges.last().unwrap().label_center.unwrap().x;
    assert!(
        d.edges
            .iter()
            .filter_map(|route| route.label_center)
            .all(|p| p.x == lane)
    );
}

fn hubs(fan: usize) -> Graph {
    let mut g = graph(vec![size(8.0, 5.0); fan + 3], Vec::new());
    for leaf in (2..2 + fan).rev() {
        g.edges
            .extend([edge(0, leaf), edge(1, leaf), edge(leaf, fan + 2)]);
    }
    g
}

#[test]
fn wide_shuffled_fan_orders_source_ports_by_target_position() {
    let leaves = 4093;
    let mut g = graph(vec![size(48.0, 10.0); leaves + 1], Vec::new());
    for i in 0..leaves {
        g.edges.push(edge(0, 1 + i * 1657 % leaves));
    }
    let d = layout(&g, &Options::default()).unwrap();
    let mut by_target: Vec<_> = d
        .edges
        .iter()
        .zip(&g.edges)
        .map(|(r, e)| (d.nodes[node(e.target)].x, r.points[0].x))
        .collect();
    by_target.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    assert!(by_target.windows(2).all(|p| p[0].1 < p[1].1));
}

#[test]
fn wide_shared_fans_and_overlapping_long_edges_have_bounded_work_and_route_size() {
    fn shortcuts(n: usize) -> Graph {
        let mut g = graph(
            vec![size(8.0, 5.0); n],
            (1..n).map(|i| edge(i - 1, i)).collect(),
        );
        g.edges.extend((2..n).rev().map(|i| edge(0, i)));
        g
    }
    let options = Options::default();
    for build in [hubs as fn(usize) -> Graph, shortcuts] {
        let (_, base) = measured(&build(2048), &options);
        let g = build(4096);
        let (d, meter) = measured(&g, &options);
        assert!(
            meter.total < 3 * base.total,
            "doubling input used {} vs {} units",
            meter.total,
            base.total
        );
        assert!(meter.total < (g.nodes.len() + g.edges.len()) * 200);
        assert_eq!(d.edges.len(), g.edges.len());
        for (e, route) in g.edges.iter().zip(d.edges) {
            assert!((4..=6).contains(&route.points.len()));
            assert_contact(
                d.nodes[node(e.source)],
                &route.points[0],
                &route.points[1],
                route.ports.0,
            );
            let last = route.points.len() - 1;
            assert_contact(
                d.nodes[node(e.target)],
                &route.points[last],
                &route.points[last - 1],
                route.ports.1,
            );
        }
    }
}

#[test]
fn caller_can_interrupt_at_admission_during_work_and_before_delivery() {
    let g = hubs(1800);
    let options = Options::default();
    let (accepted, full) = measured(&g, &options);
    let mut scratch = LayoutScratch::default();
    for reject in [1, full.checks / 3, full.checks / 2, full.checks] {
        let mut meter = Meter {
            reject_check: Some(reject),
            ..Meter::default()
        };
        assert_eq!(
            layout_with_control(&g, &options, &mut scratch, &mut meter),
            Err(LayoutError::Work(WorkError::Interrupted))
        );
        assert_eq!(meter.checks, reject);
    }
    for budget in [
        0,
        full.total / 2,
        full.total - 1,
        full.total,
        full.total + 1,
    ] {
        let mut meter = Meter {
            budget: Some(budget),
            ..Meter::default()
        };
        let drawn = layout_with_control(&g, &options, &mut scratch, &mut meter);
        if budget < full.total {
            assert_eq!(drawn, Err(LayoutError::Work(WorkError::Interrupted)));
        } else {
            assert_eq!(drawn.unwrap(), accepted);
            assert_eq!(meter.total, full.total);
        }
        assert!(meter.total <= budget);
    }
}

#[test]
fn scratch_reuses_capacity_and_honors_total_limit_after_failure_and_growth() {
    let small = hubs(5);
    let large = hubs(256);
    let options = Options::default();
    let accepted = layout(&small, &options).unwrap();
    let mut cyclic = large.clone();
    cyclic.edges.push(edge(2, 0));
    for limit in [0, 32, 64, 2 * 1024 * 1024] {
        let mut scratch = LayoutScratch::with_max_retained_bytes(limit);
        let first =
            layout_with_control(&small, &options, &mut scratch, &mut NoopWorkControl).unwrap();
        assert_eq!(first, accepted);
        let retained = scratch.retained_bytes();
        assert!(retained <= limit);
        if limit >= 32 {
            assert!(retained > 0);
        }
        let again =
            layout_with_control(&small, &options, &mut scratch, &mut NoopWorkControl).unwrap();
        assert_eq!(again, first);
        assert_eq!(scratch.retained_bytes(), retained);
        assert_eq!(
            layout_with_control(&cyclic, &options, &mut scratch, &mut NoopWorkControl),
            Err(LayoutError::Unsupported("directed cycles"))
        );
        assert!(scratch.retained_bytes() <= limit);
        let grew =
            layout_with_control(&large, &options, &mut scratch, &mut NoopWorkControl).unwrap();
        assert_eq!(grew, layout(&large, &options).unwrap());
        assert!(scratch.retained_bytes() <= limit);
        assert_eq!(
            layout_with_control(&small, &options, &mut scratch, &mut NoopWorkControl).unwrap(),
            first
        );
    }
}

#[test]
fn fixed_offsets_spans_and_side_anchors_use_the_physical_face_after_rotation() {
    let mut g = graph(vec![size(71.5, 53.125); 3], vec![edge(0, 1), edge(0, 2)]);
    let options = Options::default();
    g.nodes[0].port_span = Some(0.0);
    g.nodes[1].anchor_y = Some(7.25);
    for (direction, source, target) in [
        (Direction::Down, Side::Bottom, Side::Top),
        (Direction::Up, Side::Top, Side::Bottom),
        (Direction::Right, Side::Right, Side::Left),
        (Direction::Left, Side::Left, Side::Right),
    ] {
        g.direction = direction;
        g.edges[1].source_port = PortSideHint::FixedOffset {
            side: source,
            offset: -11.875,
        };
        g.edges[0].target_port = PortSideHint::Side(target);
        let d = layout(&g, &options).unwrap();
        check_geometry(&g, &options, &d);
        let c = d.nodes[0].center();
        let p = d.edges[0].points[0];
        let fixed = d.edges[1].points[0];
        if source.axis() == Axis::Horizontal {
            assert_eq!(p.y, c.y);
            assert_eq!(fixed.y, c.y - 11.875);
            assert_eq!(d.edges[0].points.last().unwrap().y, d.nodes[1].y + 7.25);
        } else {
            assert_eq!(p.x, c.x);
            assert_eq!(fixed.x, c.x - 11.875);
        }
    }
}

#[test]
fn zero_spacing_keeps_both_contacts_even_when_they_coincide() {
    let mut graph = graph(vec![Size::default(); 2], vec![edge(0, 1)]);
    let mut options = Options::default();
    options.spacing.edge = 0.0;
    options.spacing.rank = 0.0;
    for direction in DIRECTIONS {
        graph.direction = direction;
        let d = layout(&graph, &options).unwrap();
        assert_eq!(d.bounds, Size::default());
        let route = &d.edges[0];
        assert!(route.points.len() >= 2);
        let last = route.points.len() - 1;
        assert_contact(
            d.nodes[0],
            &route.points[0],
            &route.points[1],
            route.ports.0,
        );
        assert_contact(
            d.nodes[1],
            &route.points[last],
            &route.points[last - 1],
            route.ports.1,
        );
    }
    options.spacing.rank = 32.0;
    graph.edges[0].label = Some(size(2.125, 5.75));
    graph.nodes.push(NodeSpec::new(size(19.25, 13.125), 2));
    graph.edges.push(edge(0, 2));
    for direction in DIRECTIONS {
        graph.direction = direction;
        check_geometry(&graph, &options, &layout(&graph, &options).unwrap());
    }
}

#[test]
fn normalization_rejects_a_marker_shaft_lost_to_coordinate_precision() {
    let options = Options::default();
    for direction in [Direction::Up, Direction::Left] {
        let mut g = graph(vec![size(512.0, 512.0); 3], vec![edge(0, 1)]);
        g.nodes[2].size = if direction == Direction::Up {
            size(512.0, 2f64.powi(58))
        } else {
            size(2f64.powi(58), 512.0)
        };
        g.direction = direction;
        g.edges[0].start_marker.length = 7.0;
        assert!(matches!(
            layout(&g, &options),
            Err(LayoutError::InvalidInput(_))
        ));
    }
}

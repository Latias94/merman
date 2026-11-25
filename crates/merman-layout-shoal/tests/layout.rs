use merman_layout_shoal::{
    Axis, CyclePolicy, Direction, EdgeGeometry, EdgeSpec, Endpoint, Graph, GroupId, GroupSpec,
    LayoutError, LayoutGeometry, LayoutOutput, LayoutScratch, MarkerMetrics, NodeId, NodeSpec,
    NoopWorkControl, Options, PackingPolicy, Point, PortSideHint, Rect, RoutePoint, RoutingStyle,
    Side, Size, WorkControl, WorkError, layout, layout_geometry_with_control, layout_with_control,
};

const DIRECTIONS: [Direction; 4] = [
    Direction::Down,
    Direction::Up,
    Direction::Right,
    Direction::Left,
];
const ROUTING_STYLES: [RoutingStyle; 3] = [
    RoutingStyle::Curved,
    RoutingStyle::Orthogonal,
    RoutingStyle::Polyline,
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

fn flow_ports(direction: Direction) -> (Side, Side) {
    match direction {
        Direction::Down => (Side::Bottom, Side::Top),
        Direction::Up => (Side::Top, Side::Bottom),
        Direction::Right => (Side::Right, Side::Left),
        Direction::Left => (Side::Left, Side::Right),
    }
}

fn normal(side: Side, point: Point) -> f64 {
    match side {
        Side::Bottom => point.y,
        Side::Top => -point.y,
        Side::Right => point.x,
        Side::Left => -point.x,
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
    fn of(rect: Rect) -> Self {
        Self {
            x0: rect.x,
            x1: rect.x + rect.width,
            y0: rect.y,
            y1: rect.y + rect.height,
        }
    }

    fn new(center: Point, size: Size) -> Self {
        let bounds = Self {
            x0: center.x - size.width * 0.5,
            x1: center.x + size.width * 0.5,
            y0: center.y - size.height * 0.5,
            y1: center.y + size.height * 0.5,
        };
        assert!(size.width == 0.0 || bounds.x0 < center.x && center.x < bounds.x1);
        assert!(size.height == 0.0 || bounds.y0 < center.y && center.y < bounds.y1);
        bounds
    }

    fn overlaps(self, other: Self) -> bool {
        self.x0.max(other.x0) + EPS < self.x1.min(other.x1)
            && self.y0.max(other.y0) + EPS < self.y1.min(other.y1)
    }

    fn span(self, side: Side) -> (f64, f64) {
        match side {
            Side::Bottom => (self.y0, self.y1),
            Side::Top => (-self.y1, -self.y0),
            Side::Right => (self.x0, self.x1),
            Side::Left => (-self.x1, -self.x0),
        }
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
    if a == b {
        return (a.x - p.x).hypot(a.y - p.y) < EPS;
    }
    let t = if dx.abs() > dy.abs() {
        (p.x - a.x) / dx
    } else {
        (p.y - a.y) / dy
    };
    (-EPS..=1.0 + EPS).contains(&t) && (a.x + t * dx - p.x).hypot(a.y + t * dy - p.y) < EPS
}

fn on_polyline(point: Point, route: &EdgeGeometry) -> bool {
    route
        .points
        .windows(2)
        .any(|pair| on_segment(point, position(&pair[0]), position(&pair[1])))
}

fn distinct_polylines(first: &EdgeGeometry, second: &EdgeGeometry) -> bool {
    [(first, second), (second, first)]
        .into_iter()
        .any(|(route, other)| {
            route
                .points
                .iter()
                .any(|p| !on_polyline(position(p), other))
        })
}

fn assert_contact(rect: Rect, p: &RoutePoint, side: Side) {
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
        }
    }
}

fn marker_room<'a>(side: Side, mut knots: impl Iterator<Item = &'a RoutePoint>) -> f64 {
    let tip = position(knots.next().unwrap());
    let mut previous = tip;
    for knot in knots {
        let next = position(knot);
        if next == previous {
            continue;
        }
        let advance = normal(side, next) - normal(side, previous);
        if previous == tip {
            assert!(advance >= 0.0, "route turns into its {side:?} face");
        }
        if advance <= 0.0 {
            break;
        }
        previous = next;
    }
    normal(side, previous) - normal(side, tip)
}

fn check_terminals(graph: &Graph, nodes: &[Rect], spec: &EdgeSpec, route: &EdgeGeometry) {
    let points = &route.points;
    assert!(points.len() >= 2);
    assert!(points.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
    let first = &points[0];
    let last = points.last().unwrap();
    let (source, target) = (node(spec.source), node(spec.target));
    let (tail, head) = route.ports;
    let (mut from, mut to) = flow_ports(graph.direction);
    if route.reversed_for_ranking {
        std::mem::swap(&mut from, &mut to);
    }
    for (index, point, side, hint, auto) in [
        (source, first, tail, spec.source_port, from),
        (target, last, head, spec.target_port, to),
    ] {
        assert_contact(nodes[index], point, side);
        match hint {
            PortSideHint::Auto if source != target && side.axis() == auto.axis() => {
                assert_eq!(side, auto);
            }
            PortSideHint::Side(requested) => assert_eq!(side, requested),
            PortSideHint::FixedOffset {
                side: requested,
                offset,
            } => {
                assert_eq!(side, requested);
                let center = nodes[index].center();
                let (actual, origin) = match side.axis() {
                    Axis::Vertical => (point.x, center.x),
                    Axis::Horizontal => (point.y, center.y),
                };
                assert!((actual - (origin + offset)).abs() < EPS);
            }
            PortSideHint::Auto => {}
        }
    }
    let start = marker_room(tail, points.iter());
    let end = marker_room(head, points.iter().rev());
    for (available, required) in [
        (start, spec.start_marker.length),
        (end, spec.end_marker.length),
    ] {
        assert!(
            required == 0.0 || available > 0.0 && available + EPS >= required,
            "marker needs {required}, has {available}: {route:?}"
        );
    }
    if tail != head && tail.axis() == head.axis() {
        let gap = normal(tail, position(last)) - normal(tail, position(first));
        if start == gap && end == gap {
            assert!(gap + EPS >= spec.start_marker.length + spec.end_marker.length);
        }
    }
}

fn check_geometry(g: &Graph, options: &Options, drawing: &LayoutOutput) {
    assert_eq!(drawing.direction, g.direction);
    assert_eq!(drawing.nodes.len(), g.nodes.len());
    assert_eq!(drawing.edges.len(), g.edges.len());
    assert!(drawing.groups.is_empty());
    assert_eq!(
        drawing.stats.reversed_edges,
        drawing
            .edges
            .iter()
            .filter(|e| e.reversed_for_ranking)
            .count()
    );
    assert!(drawing.stats.dummy_nodes <= options.caps.max_dummies);
    let nodes: Vec<_> = drawing
        .nodes
        .iter()
        .zip(&g.nodes)
        .map(|(r, spec)| {
            assert_eq!(size(r.width, r.height), spec.size);
            let bounds = BoxExtents::of(*r);
            let center = r.center();
            assert!(r.width == 0.0 || bounds.x0 < center.x && center.x < bounds.x1);
            assert!(r.height == 0.0 || bounds.y0 < center.y && center.y < bounds.y1);
            bounds
        })
        .collect();
    let mut extents = nodes.clone();
    let mut labels = Vec::new();
    let flow = flow_ports(g.direction).0;
    for (id, (route, e)) in drawing.edges.iter().zip(&g.edges).enumerate() {
        let (a, b) = (node(e.source), node(e.target));
        let pts = &route.points;
        check_terminals(g, &drawing.nodes, e, route);
        if a != b {
            let (lo, hi) = if route.reversed_for_ranking {
                (b, a)
            } else {
                (a, b)
            };
            let gap = nodes[hi].span(flow).0 - nodes[lo].span(flow).1;
            assert!(
                gap + EPS >= f64::from(e.min_rank_span.max(1)) * options.spacing.rank,
                "edge {id} rank gap {gap}"
            );
            if drawing.stats.direct_fallback_edges == 0
                && e.source_port == PortSideHint::Auto
                && e.target_port == PortSideHint::Auto
                && g.nodes[a].anchor_y.is_none()
                && g.nodes[b].anchor_y.is_none()
            {
                let travel = if route.reversed_for_ranking {
                    flow_ports(g.direction).1
                } else {
                    flow
                };
                assert!(
                    pts.windows(2).all(|pair| {
                        normal(travel, position(&pair[0]))
                            <= normal(travel, position(&pair[1])) + EPS
                    }),
                    "edge {id} backtracks between ranks"
                );
            }
        }
        if options.routing == RoutingStyle::Orthogonal {
            for pair in pts.windows(2) {
                match pair[1].axis {
                    Axis::Vertical => assert_eq!(pair[0].x, pair[1].x),
                    Axis::Horizontal => assert_eq!(pair[0].y, pair[1].y),
                }
            }
        }
        if a != b
            && let Some(center) = route.label_center
        {
            let straight = pts.iter().all(|point| point.x == pts[0].x)
                || pts.iter().all(|point| point.y == pts[0].y);
            if straight || options.routing != RoutingStyle::Curved {
                assert!(on_polyline(center, route), "middle label off route {id}");
            }
        }
        for (center, measured) in [
            (route.label_center, e.label),
            (route.start_label_center, e.start_label),
            (route.end_label_center, e.end_label),
        ] {
            assert_eq!(center.is_some(), measured.is_some(), "edge {id}");
            if let (Some(center), Some(measured)) = (center, measured) {
                let rect = BoxExtents::new(center, measured);
                labels.push((id, rect));
                extents.push(rect);
            }
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
            let hits = |rect: BoxExtents| {
                if options.routing == RoutingStyle::Curved && a.x != b.x && a.y != b.y {
                    rect.crosses(a, a) || rect.crosses(b, b)
                } else {
                    rect.crosses(a, b)
                }
            };
            for (index, rect) in nodes.iter().enumerate() {
                if index != node(e.source) && index != node(e.target) {
                    assert!(
                        !hits(*rect),
                        "edge {id} crosses node {index}: {pair:?} {rect:?}"
                    );
                }
            }
            for &(owner, rect) in &labels {
                if owner != id {
                    assert!(
                        !hits(rect),
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
    assert!(
        extents
            .iter()
            .all(|r| [r.x0, r.x1, r.y0, r.y1].into_iter().all(f64::is_finite))
    );
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
    options.packing = PackingPolicy::Strip;
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
fn aspect_and_fit_width_wrap_components_while_strip_follows_the_cross_axis() {
    let side = 20.0;
    let gap = 8.0;
    let width = 2.0 * side + gap;
    let mut g = graph(vec![size(side, side); 12], vec![]);
    let mut options = Options::default();
    options.spacing.component = gap;
    let default_packing = options.packing;
    for direction in DIRECTIONS {
        g.direction = direction;
        let across = |s: Size| match direction {
            Direction::Down | Direction::Up => s.width / s.height,
            Direction::Right | Direction::Left => s.height / s.width,
        };
        options.packing = PackingPolicy::Strip;
        let strip = layout(&g, &options).unwrap();
        check_geometry(&g, &options, &strip);
        let strip_depth = match direction {
            Direction::Down | Direction::Up => strip.bounds.height,
            Direction::Right | Direction::Left => strip.bounds.width,
        };
        assert_eq!(strip_depth, side);
        let mut square_ratio = 0.0;
        for policy in [
            default_packing,
            PackingPolicy::AspectTargeted { target_ratio: 1.0 },
            PackingPolicy::AspectTargeted { target_ratio: 4.0 },
            PackingPolicy::FitWidth { max_width: width },
        ] {
            options.packing = policy;
            let drawing = layout(&g, &options).unwrap();
            check_geometry(&g, &options, &drawing);
            assert_eq!(drawing.stats.components, g.nodes.len());
            assert!(drawing.bounds.width > side && drawing.bounds.height > side);
            for (index, rect) in drawing.nodes.iter().enumerate() {
                for other in &drawing.nodes[index + 1..] {
                    assert!(
                        (rect.x - other.x).abs() >= side + gap
                            || (rect.y - other.y).abs() >= side + gap
                    );
                }
            }
            match policy {
                PackingPolicy::AspectTargeted { target_ratio } => {
                    let ratio = across(drawing.bounds);
                    assert!(
                        (ratio / target_ratio).ln().abs()
                            < (across(strip.bounds) / target_ratio).ln().abs()
                    );
                    if target_ratio == 1.0 {
                        square_ratio = ratio;
                    } else if target_ratio == 4.0 {
                        assert!(ratio > square_ratio);
                    }
                }
                PackingPolicy::FitWidth { max_width } => {
                    assert!(drawing.bounds.width <= max_width);
                    assert_eq!(drawing.nodes[0].y, drawing.nodes[1].y);
                    assert!(drawing.nodes[2].y >= drawing.nodes[0].y + side + gap);
                }
                PackingPolicy::Strip | PackingPolicy::BalancedAspect { .. } => {
                    panic!("packing policy outside this fixture")
                }
            }
            assert_eq!(layout(&g, &options).unwrap(), drawing);
        }
    }
}

#[test]
fn straight_edges_reserve_both_markers_and_the_middle_label() {
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
        let route = &d.edges[0];
        let side = route.ports.0;
        let start = normal(side, position(&route.points[0]));
        let end = normal(side, position(route.points.last().unwrap()));
        assert!(end - start + EPS >= g.edges[0].start_marker.length + g.edges[0].end_marker.length);
        assert!((start..=end).contains(&normal(side, route.label_center.unwrap())));
    }
    g.nodes.iter_mut().for_each(|n| n.size = Size::default());
    g.edges[0] = edge(1, 0);
    check_geometry(&g, &options, &layout(&g, &options).unwrap());
}

#[test]
fn competing_minimum_rank_spans_are_feasible_in_every_direction() {
    let mut options = Options::default();
    options.spacing.rank = 100.0;
    let mut g = graph(
        vec![size(10.0, 10.0); 3],
        vec![edge(1, 2), edge(0, 2), edge(0, 1)],
    );
    for (e, span) in g.edges.iter_mut().zip([2, 6, 3]) {
        e.min_rank_span = span;
    }
    for direction in DIRECTIONS {
        g.direction = direction;
        let d = layout(&g, &options).unwrap();
        assert_eq!(d.stats.reversed_edges, 0);
        check_geometry(&g, &options, &d);
    }
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
        for style in ROUTING_STYLES {
            options.routing = style;
            let d = layout(&g, &options).unwrap();
            check_geometry(&g, &options, &d);
            for (i, route) in d.edges.iter().enumerate() {
                for other in &d.edges[i + 1..] {
                    assert_ne!(other.points, route.points);
                    if style != RoutingStyle::Curved {
                        assert!(distinct_polylines(route, other));
                    }
                }
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
            for style in ROUTING_STYLES {
                options.routing = style;
                let d = layout(&g, &options).unwrap();
                assert_eq!(d.stats.reversed_edges, 0);
                check_geometry(&g, &options, &d);
                assert_eq!(layout(&g, &options).unwrap(), d);
            }
        }
    }
}

#[test]
fn cycles_reverse_only_inside_each_scc_and_return_semantic_contacts() {
    let mut g = graph(
        vec![size(36.0, 24.0); 4],
        vec![edge(1, 2), edge(2, 3), edge(0, 1), edge(3, 2), edge(1, 0)],
    );
    for (index, e) in g.edges.iter_mut().enumerate() {
        e.start_marker.length = 3.0 + index as f64;
        e.end_marker.length = 11.0 + index as f64;
    }
    let mut options = Options::default();
    for direction in DIRECTIONS {
        g.direction = direction;
        for policy in [CyclePolicy::SccGreedyFas, CyclePolicy::SccDfs] {
            options.cycle_policy = policy;
            for style in ROUTING_STYLES {
                options.routing = style;
                let drawing = layout(&g, &options).unwrap();
                check_geometry(&g, &options, &drawing);
                assert_eq!(drawing.stats.components, 1);
                assert_eq!(drawing.stats.scc_cyclic_count, 2);
                assert_eq!(drawing.stats.reversed_edges, 2);
                assert!(!drawing.edges[0].reversed_for_ranking);
                for (forward, back) in [(1, 3), (2, 4)] {
                    assert_ne!(
                        drawing.edges[forward].reversed_for_ranking,
                        drawing.edges[back].reversed_for_ranking
                    );
                }
            }
        }
    }
}

#[test]
fn self_loops_keep_separate_exterior_routes_labels_and_semantic_ends() {
    let mut g = graph(
        vec![size(48.0, 36.0); 2],
        vec![edge(0, 0), edge(0, 1), edge(0, 0), edge(0, 0)],
    );
    let loops = [0, 2, 3];
    for &index in &loops {
        g.edges[index].label = Some(size(12.0 + index as f64, 9.0));
        g.edges[index].start_marker.length = 3.0;
        g.edges[index].end_marker.length = 5.0;
    }
    let mut options = Options::default();
    for direction in DIRECTIONS {
        g.direction = direction;
        let (outer, other) = match direction {
            Direction::Down | Direction::Up => (Side::Right, Side::Left),
            Direction::Right | Direction::Left => (Side::Bottom, Side::Top),
        };
        for style in ROUTING_STYLES {
            options.routing = style;
            let drawing = layout(&g, &options).unwrap();
            check_geometry(&g, &options, &drawing);
            assert_eq!(drawing.stats.scc_cyclic_count, 0);
            assert_eq!(drawing.stats.reversed_edges, 0);
            let owner = BoxExtents::of(drawing.nodes[0]);
            for (index, side) in loops.into_iter().zip([outer, other, outer]) {
                let route = &drawing.edges[index];
                assert_eq!(route.ports, (side, side));
                let face = owner.span(side).1;
                assert!(
                    route
                        .points
                        .iter()
                        .any(|p| normal(side, position(p)) > face)
                );
                assert!(
                    route
                        .points
                        .iter()
                        .all(|p| normal(side, position(p)) + EPS >= face)
                );
                let label =
                    BoxExtents::new(route.label_center.unwrap(), g.edges[index].label.unwrap());
                assert!(label.span(side).0 + EPS >= face);
            }
            assert_ne!(drawing.edges[0].points, drawing.edges[3].points);
        }
    }
}

fn marker_box(contact: &RoutePoint, side: Side, marker: MarkerMetrics) -> BoxExtents {
    let mut center = position(contact);
    let half = marker.length * 0.5;
    match side {
        Side::Top => center.y -= half,
        Side::Bottom => center.y += half,
        Side::Left => center.x -= half,
        Side::Right => center.x += half,
    }
    let footprint = match side.axis() {
        Axis::Vertical => size(marker.width, marker.length),
        Axis::Horizontal => size(marker.length, marker.width),
    };
    BoxExtents::new(center, footprint)
}

#[test]
fn terminal_labels_stay_with_their_semantic_ends_and_clear_all_marker_boxes() {
    let mut input = graph(
        vec![size(56.0, 40.0); 4],
        vec![edge(1, 2), edge(2, 1), edge(2, 3)],
    );
    input.nodes[0].size.width = 200.0;
    input.edges[0].start_label = Some(size(17.0, 11.0));
    input.edges[1].end_label = Some(size(11.0, 15.0));
    input.edges[2].start_label = Some(size(19.0, 10.0));
    input.edges[2].label = Some(size(31.0, 13.0));
    input.edges[2].end_label = Some(size(14.0, 18.0));
    for spec in &mut input.edges {
        spec.start_marker = MarkerMetrics {
            length: 15.0,
            width: 12.0,
        };
        spec.end_marker = MarkerMetrics {
            length: 9.0,
            width: 18.0,
        };
    }
    let mut options = Options::default();
    options.packing = PackingPolicy::FitWidth {
        max_width: input.nodes[0].size.width,
    };
    options.spacing.rank = 160.0;
    for direction in DIRECTIONS {
        input.direction = direction;
        for style in ROUTING_STYLES {
            options.routing = style;
            let drawing = layout(&input, &options).unwrap();
            check_geometry(&input, &options, &drawing);
            assert_eq!(drawing.stats.components, 2);
            assert_eq!(drawing.stats.reversed_edges, 1);
            let next_row = drawing.nodes[0].y + drawing.nodes[0].height + options.spacing.component;
            let mut markers = Vec::new();
            let mut labels = Vec::new();
            for (route, spec) in drawing.edges.iter().zip(&input.edges) {
                assert!(route.points.iter().all(|point| point.y + EPS >= next_row));
                let tail = &route.points[0];
                let head = route.points.last().unwrap();
                for (at, far, side, marker, center, measured) in [
                    (
                        tail,
                        head,
                        route.ports.0,
                        spec.start_marker,
                        route.start_label_center,
                        spec.start_label,
                    ),
                    (
                        head,
                        tail,
                        route.ports.1,
                        spec.end_marker,
                        route.end_label_center,
                        spec.end_label,
                    ),
                ] {
                    markers.push(marker_box(at, side, marker));
                    if let (Some(center), Some(measured)) = (center, measured) {
                        let label = BoxExtents::new(center, measured);
                        assert!(label.y0 + EPS >= next_row);
                        let tip = normal(side, position(at));
                        assert!(
                            label.span(side).0 + EPS
                                >= tip + marker.length + options.spacing.end_label_along
                        );
                        assert!(
                            normal(side, center) - tip
                                < normal(side, position(far)) - normal(side, center)
                        );
                        labels.push(label);
                    }
                }
            }
            assert!(
                labels
                    .iter()
                    .all(|label| markers.iter().all(|m| !label.overlaps(*m)))
            );
        }
    }
}

#[test]
fn invalid_endpoints_and_measurements_report_input_errors() {
    let options = Options::default();
    let mut g = graph(vec![size(10.0, 8.0); 2], vec![edge(0, 1)]);
    let invalid = |g: &Graph| {
        assert!(matches!(
            layout(g, &options),
            Err(LayoutError::InvalidInput(_))
        ))
    };
    let mut bad = g.clone();
    bad.edges[0].target = NodeId(2).into();
    invalid(&bad);
    let mut bad = g.clone();
    bad.edges[0].source = GroupId(0).into();
    invalid(&bad);
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
            let mut bad = g.clone();
            bad.edges[0].start_label = Some(s);
            invalid(&bad);
            let mut bad = g.clone();
            bad.edges[0].end_label = Some(s);
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
    g.groups.push(GroupSpec::new(0));
    g.edges[0].target = GroupId(1).into();
    invalid(&g);
}
#[derive(Default)]
struct Meter {
    total: usize,
    budget: Option<usize>,
    denied_total: Option<usize>,
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
            self.denied_total = Some(next);
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
fn geometry_only_omits_flat_crossing_work_and_preserves_preview_geometry() {
    let g = graph(
        vec![size(42.0, 24.0); 4],
        vec![edge(0, 2), edge(0, 3), edge(1, 2), edge(1, 3)],
    );
    for capture_node_ranks in [false, true] {
        let mut options = Options::default();
        options.capture_node_ranks = capture_node_ranks;
        let (full, full_meter) = measured(&g, &options);
        assert!(full.stats.routed_crossings > 0);
        let mut geometry_meter = Meter::default();
        let geometry = layout_geometry_with_control(
            &g,
            &options,
            &mut LayoutScratch::default(),
            &mut geometry_meter,
        )
        .unwrap();
        assert_eq!(geometry, LayoutGeometry::from(full));
        if capture_node_ranks {
            assert_eq!(geometry_meter.total, full_meter.total);
        } else {
            assert!(geometry_meter.total < full_meter.total);
        }
    }
}

#[test]
fn two_maximum_spans_keep_sparse_work_storage_and_their_physical_separation() {
    let mut g = graph(vec![size(2.0, 2.0); 3], vec![edge(0, 1), edge(1, 2)]);
    g.edges[0].label = Some(size(10.0, 4.0));
    let mut opt = Options::default();
    opt.caps.max_dummies = 1;
    opt.spacing.rank = 1024.0 / f64::from(u32::MAX);
    let (_, ordinary) = measured(&g, &opt);
    for e in &mut g.edges {
        e.min_rank_span = u32::MAX;
    }
    let mut control = Meter {
        budget: Some(4 * ordinary.total),
        ..Meter::default()
    };
    let drawing =
        layout_with_control(&g, &opt, &mut LayoutScratch::default(), &mut control).unwrap();
    check_geometry(&g, &opt, &drawing);
    assert_eq!(drawing.stats.dummy_nodes, 1);
    assert_eq!(drawing.stats.direct_fallback_edges, 0);
    for pair in drawing.nodes.windows(2) {
        assert!(pair[1].y - (pair[0].y + pair[0].height) + EPS >= 1024.0);
    }
}

#[test]
fn coordinate_overflow_and_unrepresentable_marker_geometry_report_errors() {
    let mut g = graph(vec![size(2.0, 2.0); 2], vec![edge(0, 1)]);
    let mut opt = Options::default();
    g.edges[0].min_rank_span = u32::MAX;
    opt.spacing.rank = f64::MAX;
    assert_eq!(
        layout(&g, &opt),
        Err(LayoutError::Work(WorkError::ArithmeticOverflow))
    );
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
    check_geometry(&g, &opt, &d);
    let mut packed = graph(
        vec![size(2f64.powi(62), 2.0), Size::default(), Size::default()],
        vec![edge(1, 2); 2],
    );
    opt.spacing.component = 0.0;
    opt.packing = PackingPolicy::Strip;
    opt.routing = RoutingStyle::Polyline;
    match layout(&packed, &opt) {
        Ok(d) => {
            check_geometry(&packed, &opt, &d);
            assert!(distinct_polylines(&d.edges[0], &d.edges[1]));
        }
        Err(LayoutError::InvalidInput(_)) => {}
        Err(other) => panic!("unexpected precision error: {other:?}"),
    }
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
fn ordinary_long_edge_uses_an_interior_channel_and_direct_cap_preserves_its_label() {
    let last = 10;
    let mut g = graph(
        vec![size(24.0, 18.0); last + 1],
        (0..last).map(|index| edge(index, index + 1)).collect(),
    );
    let mut bypass = edge(0, last);
    bypass.label = Some(size(31.0, 13.0));
    bypass.start_marker.length = 7.0;
    bypass.end_marker.length = 11.0;
    g.edges.push(bypass);
    for direction in DIRECTIONS {
        g.direction = direction;
        let (cross, bookend) = match direction {
            Direction::Down | Direction::Up => (Side::Right, size(480.0, 24.0)),
            Direction::Right | Direction::Left => (Side::Bottom, size(24.0, 480.0)),
        };
        g.nodes[0].size = bookend;
        g.nodes[last].size = bookend;
        let mut options = Options::default();
        for style in ROUTING_STYLES {
            options.routing = style;
            let drawing = layout(&g, &options).unwrap();
            check_geometry(&g, &options, &drawing);
            assert_eq!(drawing.stats.direct_fallback_edges, 0);
            assert_eq!(drawing.stats.dummy_nodes, last - 1);
            let (lo, hi) = drawing
                .nodes
                .iter()
                .map(|r| BoxExtents::of(*r).span(cross))
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (a, b)| {
                    (lo.min(a), hi.max(b))
                });
            let route = &drawing.edges[last];
            assert!(
                route
                    .points
                    .iter()
                    .all(|p| (lo - EPS..=hi + EPS).contains(&normal(cross, position(p))))
            );
            let label =
                BoxExtents::new(route.label_center.unwrap(), bypass.label.unwrap()).span(cross);
            assert!(lo - EPS <= label.0 && label.1 <= hi + EPS);

            let mut capped = options;
            capped.caps.max_dummies = 0;
            let direct = layout(&g, &capped).unwrap();
            check_geometry(&g, &capped, &direct);
            assert_eq!(direct.stats.dummy_nodes, 0);
            assert_eq!(direct.stats.direct_fallback_edges, 1);
            assert!(direct.edges[last].label_center.is_some());
        }
    }
}

#[test]
fn disjoint_long_edges_clear_the_intermediate_nodes_and_each_others_labels() {
    let intervals = 30;
    let last = 3 * intervals;
    let mut input = graph(
        vec![size(24.0, 12.0); last + 1],
        (0..last).map(|index| edge(index, index + 1)).collect(),
    );
    for source in (0..last).step_by(3) {
        let mut bypass = edge(source, source + 3);
        bypass.label = Some(size(81.0, 11.0));
        input.edges.push(bypass);
    }
    let mut options = Options::default();
    for style in ROUTING_STYLES {
        options.routing = style;
        let drawing = layout(&input, &options).unwrap();
        assert_eq!(drawing.stats.direct_fallback_edges, 0);
        check_geometry(&input, &options, &drawing);
    }
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
    for (leaves, source_width) in [(4093, 48.0), (17, 640.0)] {
        let mut g = graph(vec![size(48.0, 10.0); leaves + 1], Vec::new());
        g.nodes[0].size.width = source_width;
        for i in 0..leaves {
            g.edges.push(edge(0, 1 + i * 1657 % leaves));
        }
        let options = Options::default();
        let d = layout(&g, &options).unwrap();
        let mut by_target: Vec<_> = d
            .edges
            .iter()
            .zip(&g.edges)
            .map(|(r, e)| {
                check_terminals(&g, &d.nodes, e, r);
                (d.nodes[node(e.target)].x, r.points[0].x)
            })
            .collect();
        by_target.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
        for pair in by_target.windows(2) {
            let pitch = pair[1].1 - pair[0].1;
            assert!(pitch >= 0.0);
            if leaves == 17 {
                assert!(pitch + EPS >= options.spacing.min_port_pitch);
            }
        }
    }
}

#[test]
fn wide_shared_fans_and_overlapping_long_edges_respect_work_and_expansion_caps() {
    fn shortcuts(n: usize) -> Graph {
        let mut g = graph(
            vec![size(8.0, 5.0); n],
            (1..n).map(|i| edge(i - 1, i)).collect(),
        );
        g.edges.extend((2..n).rev().map(|i| edge(0, i)));
        g
    }
    let mut options = Options::default();
    options.caps.max_dummies = 128;
    let interactive_work = 14_100_000;
    for (build, needs_fallback) in [(hubs as fn(usize) -> Graph, false), (shortcuts, true)] {
        let admitted = build(128);
        let mut scratch = LayoutScratch::default();
        let mut meter = Meter {
            budget: Some(interactive_work),
            ..Meter::default()
        };
        let drawing = layout_with_control(&admitted, &options, &mut scratch, &mut meter).unwrap();
        check_geometry(&admitted, &options, &drawing);
        assert_eq!(drawing.stats.direct_fallback_edges > 0, needs_fallback);
        let point_count: usize = drawing.edges.iter().map(|route| route.points.len()).sum();
        assert!(point_count <= meter.total && meter.total <= interactive_work);

        let large = build(4096);
        let budget = meter.total;
        let mut meter = Meter {
            budget: Some(budget),
            ..Meter::default()
        };
        match layout_with_control(&large, &options, &mut scratch, &mut meter) {
            Ok(drawing) => {
                assert_eq!(drawing.edges.len(), large.edges.len());
                assert!(drawing.stats.dummy_nodes <= options.caps.max_dummies);
                assert_eq!(drawing.stats.direct_fallback_edges > 0, needs_fallback);
                for (route, spec) in drawing.edges.iter().zip(&large.edges) {
                    check_terminals(&large, &drawing.nodes, spec, route);
                }
            }
            Err(LayoutError::Work(WorkError::Interrupted)) => {
                assert!(
                    meter
                        .denied_total
                        .is_some_and(|requested| requested > budget)
                );
            }
            Err(error) => panic!("unexpected wide graph error: {error}"),
        }
        assert!(meter.total <= budget);
    }
}

#[test]
fn doubling_a_sparse_chain_has_bounded_work_growth() {
    let chain = |n| {
        graph(
            vec![size(12.0, 8.0); n],
            (1..n).map(|i| edge(i - 1, i)).collect(),
        )
    };
    let options = Options::default();
    let (_, small) = measured(&chain(2048), &options);
    let (drawing, large) = measured(&chain(4096), &options);
    assert_eq!(drawing.stats.dummy_nodes, 0);
    assert_eq!(drawing.edges.len(), 4095);
    assert!(
        large.total > small.total && large.total < 3 * small.total,
        "{} vs {}",
        large.total,
        small.total
    );
}

#[test]
fn caller_can_interrupt_at_admission_during_work_and_before_delivery() {
    let g = hubs(64);
    let options = Options::default();
    let (accepted, full) = measured(&g, &options);
    let mut scratch = LayoutScratch::default();
    assert!(full.checks >= 6 && full.total > 0);
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
        if reject == full.checks {
            assert_eq!(meter.total, full.total);
        }
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
            assert!(
                meter
                    .denied_total
                    .is_some_and(|requested| requested > budget)
            );
        } else {
            assert_eq!(drawn.unwrap(), accepted);
            assert_eq!(meter.total, full.total);
            assert!(meter.denied_total.is_none());
        }
        assert!(meter.total <= budget);
    }
    let mut overflow = Meter {
        total: usize::MAX,
        ..Meter::default()
    };
    assert_eq!(
        layout_with_control(&g, &options, &mut scratch, &mut overflow),
        Err(LayoutError::Work(WorkError::ArithmeticOverflow))
    );
}

#[test]
fn scratch_is_reusable_within_its_total_limit_after_failure_and_growth() {
    let small = hubs(5);
    let large = hubs(96);
    let options = Options::default();
    let accepted = layout(&small, &options).unwrap();
    let (grown, full) = measured(&large, &options);
    for limit in [0, 32, 64, 2 * 1024 * 1024] {
        let mut scratch = LayoutScratch::with_max_retained_bytes(limit);
        let first =
            layout_with_control(&small, &options, &mut scratch, &mut NoopWorkControl).unwrap();
        assert_eq!(first, accepted);
        assert!(scratch.retained_bytes() <= limit);
        let again =
            layout_with_control(&small, &options, &mut scratch, &mut NoopWorkControl).unwrap();
        assert_eq!(again, first);
        assert!(scratch.retained_bytes() <= limit);
        let mut interrupted = Meter {
            reject_check: Some(full.checks),
            ..Meter::default()
        };
        assert_eq!(
            layout_with_control(&large, &options, &mut scratch, &mut interrupted),
            Err(LayoutError::Work(WorkError::Interrupted))
        );
        assert!(scratch.retained_bytes() <= limit);
        let grew =
            layout_with_control(&large, &options, &mut scratch, &mut NoopWorkControl).unwrap();
        assert_eq!(grew, grown);
        assert!(scratch.retained_bytes() <= limit);
        assert_eq!(
            layout_with_control(&small, &options, &mut scratch, &mut NoopWorkControl).unwrap(),
            first
        );
        assert!(scratch.retained_bytes() <= limit);
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
        let mut detour = g.clone();
        let (tail, head) = match source.axis() {
            Axis::Vertical => (Side::Left, Side::Right),
            Axis::Horizontal => (Side::Top, Side::Bottom),
        };
        detour.edges[1].source_port = PortSideHint::FixedOffset {
            side: tail,
            offset: -11.875,
        };
        detour.edges[1].target_port = PortSideHint::FixedOffset {
            side: head,
            offset: 5.125,
        };
        let mut options = options;
        options.routing = RoutingStyle::Orthogonal;
        check_geometry(&detour, &options, &layout(&detour, &options).unwrap());
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
        check_geometry(&graph, &options, &d);
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
fn normalization_preserves_representable_marker_shafts_or_reports_precision_loss() {
    let mut options = Options::default();
    options.packing = PackingPolicy::Strip;
    for direction in [Direction::Up, Direction::Left] {
        let mut g = graph(vec![size(512.0, 512.0); 3], vec![edge(0, 1)]);
        g.nodes[2].size = if direction == Direction::Up {
            size(512.0, 2f64.powi(58))
        } else {
            size(2f64.powi(58), 512.0)
        };
        g.direction = direction;
        g.edges[0].start_marker.length = 7.0;
        match layout(&g, &options) {
            Ok(drawing) => {
                check_geometry(&g, &options, &drawing);
                let route = &drawing.edges[0];
                let tip = normal(route.ports.0, position(&route.points[0]));
                assert!(tip + g.edges[0].start_marker.length > tip);
            }
            Err(LayoutError::InvalidInput(_)) => {}
            Err(other) => panic!("unexpected normalization error: {other:?}"),
        }
    }
}

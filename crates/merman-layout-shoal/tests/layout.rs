use merman_layout_shoal::{
    Direction, Edge, Graph, Layout, LayoutError, NodeId, Point, Size, layout, layout_with_control,
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

fn edge(source: usize, target: usize) -> Edge {
    Edge {
        source: NodeId(source),
        target: NodeId(target),
        min_rank_span: 1,
        label: None,
        start_clearance: 0.0,
        end_clearance: 0.0,
    }
}

fn graph(nodes: Vec<Size>, edges: Vec<Edge>) -> Graph {
    Graph {
        direction: Direction::Down,
        node_spacing: 16.0,
        rank_spacing: 40.0,
        nodes,
        edges,
    }
}

fn along(direction: Direction, p: Point) -> f64 {
    match direction {
        Direction::Down => p.y,
        Direction::Up => -p.y,
        Direction::Right => p.x,
        Direction::Left => -p.x,
    }
}

fn depth(direction: Direction, s: Size) -> f64 {
    match direction {
        Direction::Down | Direction::Up => s.height,
        Direction::Right | Direction::Left => s.width,
    }
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
}

impl Rect {
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

fn check_geometry(g: &Graph, drawing: &Layout) {
    assert_eq!(drawing.centers.len(), g.nodes.len());
    assert_eq!(drawing.edges.len(), g.edges.len());
    let nodes: Vec<_> = drawing
        .centers
        .iter()
        .zip(&g.nodes)
        .map(|(&p, &s)| Rect::new(p, s))
        .collect();
    let mut labels = Vec::new();
    let mut extents = nodes.clone();
    for (id, (route, e)) in drawing.edges.iter().zip(&g.edges).enumerate() {
        let (a, b) = (e.source.0, e.target.0);
        let pts = &route.points;
        assert!((4..=6).contains(&pts.len()));
        assert_eq!(pts[0], drawing.centers[a]);
        assert_eq!(pts[pts.len() - 1], drawing.centers[b]);
        for pair in pts.windows(2) {
            assert!(
                along(g.direction, pair[1]) > along(g.direction, pair[0]),
                "{id} {pair:?}"
            );
        }
        let start_bound =
            along(g.direction, drawing.centers[a]) + depth(g.direction, g.nodes[a]) / 2.0;
        let end_bound =
            along(g.direction, drawing.centers[b]) - depth(g.direction, g.nodes[b]) / 2.0;
        let start = along(g.direction, pts[1]) - start_bound;
        let end = end_bound - along(g.direction, pts[pts.len() - 2]);
        assert!(start > 0.0 && start + EPS >= e.start_clearance);
        assert!(end > 0.0 && end + EPS >= e.end_clearance);
        assert!(end_bound - start_bound + EPS >= e.min_rank_span.max(1) as f64 * g.rank_spacing);
        assert_eq!(e.label.is_some(), route.label_center.is_some());
        if let (Some(p), Some(s)) = (route.label_center, e.label) {
            assert!(
                pts.windows(2).any(|seg| on_segment(p, seg[0], seg[1])),
                "label off route {id}"
            );
            let rect = Rect::new(p, s);
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
            let (a, b) = (pair[0], pair[1]);
            for (node, rect) in nodes.iter().enumerate() {
                if node != e.source.0 && node != e.target.0 {
                    assert!(
                        !rect.crosses(a, b),
                        "edge {id} crosses node {node}: {pair:?} {rect:?}"
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
        extents.extend(route.points.iter().map(|&p| Rect::new(p, Size::default())));
    }
    let bounds = drawing.bounds;
    for (expected, actual) in [
        (extents.iter().map(|r| r.x0).reduce(f64::min), bounds.min_x),
        (extents.iter().map(|r| r.x1).reduce(f64::max), bounds.max_x),
        (extents.iter().map(|r| r.y0).reduce(f64::min), bounds.min_y),
        (extents.iter().map(|r| r.y1).reduce(f64::max), bounds.max_y),
    ] {
        assert!((expected.unwrap_or(0.0) - actual).abs() < EPS);
    }
}

#[test]
fn empty_and_interleaved_components_preserve_occurrences_and_spacing() {
    assert_eq!(layout(&graph(vec![], vec![])).unwrap(), Layout::default());
    let mut g = graph(
        vec![size(18.0, 42.0); 7],
        vec![edge(6, 2), edge(3, 0), edge(5, 3)],
    );
    g.edges[1].label = Some(size(97.0, 23.0));
    for direction in DIRECTIONS {
        g.direction = direction;
        let d = layout(&g).unwrap();
        check_geometry(&g, &d);
        let cross = |p: Point| match direction {
            Direction::Down | Direction::Up => p.x,
            _ => p.y,
        };
        let first_label = cross(d.edges[1].label_center.unwrap());
        let label_size = g.edges[1].label.unwrap();
        let first_half = cross(Point {
            x: label_size.width,
            y: label_size.height,
        }) / 2.0;
        let node_half = cross(Point {
            x: g.nodes[0].width,
            y: g.nodes[0].height,
        }) / 2.0;
        let first_end = (first_label + first_half).max(cross(d.centers[0]) + node_half);
        assert!(cross(d.centers[1]) - node_half >= first_end + g.node_spacing);
        assert!(cross(d.centers[2]) > cross(d.centers[1]));
        assert!(cross(d.centers[4]) > cross(d.centers[2]));
    }
}

#[test]
fn straight_edges_keep_both_exterior_waypoints_and_marker_setbacks() {
    let mut g = graph(
        vec![size(73.0, 11.0), size(14.0, 53.0)],
        vec![Edge {
            start_clearance: 79.0,
            end_clearance: 57.0,
            label: Some(size(131.0, 61.0)),
            ..edge(1, 0)
        }],
    );
    g.node_spacing = 0.0;
    g.rank_spacing = 0.0;
    for direction in DIRECTIONS {
        g.direction = direction;
        let d = layout(&g).unwrap();
        check_geometry(&g, &d);
        assert_eq!(d.edges[0].points.len(), 4);
    }
    g.nodes.fill(Size::default());
    g.edges[0] = edge(1, 0);
    check_geometry(&g, &layout(&g).unwrap());
}

#[test]
fn longest_path_satisfies_competing_minimum_rank_spans() {
    let mut g = graph(
        vec![size(10.0, 10.0); 3],
        vec![
            Edge {
                min_rank_span: 2,
                ..edge(1, 2)
            },
            Edge {
                min_rank_span: 6,
                ..edge(0, 2)
            },
            Edge {
                min_rank_span: 3,
                ..edge(0, 1)
            },
        ],
    );
    g.rank_spacing = 100.0;
    let d = layout(&g).unwrap();
    check_geometry(&g, &d);
    assert_eq!(d.centers[1].y - d.centers[0].y, 310.0);
    assert_eq!(d.centers[2].y - d.centers[1].y, 310.0);
    g.edges.fill(edge(0, 1));
    let one = layout(&g).unwrap();
    g.edges[0].min_rank_span = 0;
    g.edges[1].min_rank_span = 0;
    g.edges[2].min_rank_span = 0;
    assert_eq!(layout(&g).unwrap(), one);
}

#[test]
fn parallel_occurrences_have_distinct_paths_even_for_point_nodes() {
    let mut g = graph(vec![Size::default(); 2], vec![edge(0, 1); 9]);
    for (i, e) in g.edges.iter_mut().enumerate().step_by(2) {
        e.label = Some(size(10.0 + 7.0 * i as f64, 31.0 - i as f64));
    }
    g.node_spacing = 0.0;
    g.rank_spacing = 0.0;
    for direction in DIRECTIONS {
        g.direction = direction;
        let d = layout(&g).unwrap();
        check_geometry(&g, &d);
        for (i, route) in d.edges.iter().enumerate() {
            assert!(
                d.edges[i + 1..]
                    .iter()
                    .all(|other| other.points != route.points)
            );
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
        g.node_spacing = (seed % 3) as f64;
        g.rank_spacing = (seed % 7) as f64;
        let mut state = seed + 1u32;
        for i in 0..10 {
            for j in (i + 1..10).rev() {
                state = state.wrapping_mul(1664525).wrapping_add(1013904223);
                if state % 5 >= 2 {
                    continue;
                }
                let mut e = edge((3 * i + 7) % 10, (3 * j + 7) % 10);
                e.min_rank_span = (state % 4) as usize;
                e.start_clearance = (state % 13) as f64;
                e.end_clearance = (state % 9) as f64;
                if state & 4 != 0 {
                    e.label = Some(size((state % 121) as f64, (state % 63) as f64));
                }
                g.edges.push(e);
                if state.is_multiple_of(11) {
                    g.edges.push(e);
                }
            }
        }
        for direction in DIRECTIONS {
            g.direction = direction;
            let d = layout(&g).unwrap();
            check_geometry(&g, &d);
            assert_eq!(layout(&g).unwrap(), d);
        }
    }
}

#[test]
fn cycles_and_invalid_measurements_have_distinct_errors() {
    let mut g = graph(
        vec![size(10.0, 8.0); 3],
        vec![edge(0, 1), edge(1, 2), edge(2, 1)],
    );
    assert!(matches!(layout(&g), Err(LayoutError::Unsupported(_))));
    g.edges[2] = edge(2, 2);
    assert!(matches!(layout(&g), Err(LayoutError::Unsupported(_))));
    g.edges[2] = edge(0, 3);
    assert!(matches!(layout(&g), Err(LayoutError::InvalidInput(_))));
    g.edges[2] = edge(usize::MAX, 0);
    assert!(matches!(layout(&g), Err(LayoutError::InvalidInput(_))));
    g.edges.truncate(1);
    let invalid = |g: &Graph| assert!(matches!(layout(g), Err(LayoutError::InvalidInput(_))));
    for value in [-1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut bad = g.clone();
        bad.rank_spacing = value;
        invalid(&bad);
        let mut bad = g.clone();
        bad.node_spacing = value;
        invalid(&bad);
        for s in [size(1.0, value), size(value, 1.0)] {
            let mut bad = g.clone();
            bad.nodes[0] = s;
            invalid(&bad);
            let mut bad = g.clone();
            bad.edges[0].label = Some(s);
            invalid(&bad);
        }
        let mut bad = g.clone();
        bad.edges[0].start_clearance = value;
        invalid(&bad);
        let mut bad = g.clone();
        bad.edges[0].end_clearance = value;
        invalid(&bad);
    }
}

fn measured(g: &Graph) -> (Layout, usize) {
    let mut total = 0usize;
    let layout = layout_with_control(g, |units| {
        assert!(units > 0);
        total = total.checked_add(units).unwrap();
        Ok::<_, ()>(())
    })
    .unwrap();
    (layout, total)
}

#[test]
fn maximum_logical_rank_never_allocates_by_span_and_overflow_is_explicit() {
    let mut g = graph(vec![size(2.0, 2.0); 3], vec![edge(0, 1)]);
    g.rank_spacing = 0.0;
    let ordinary = measured(&g);
    g.edges[0].min_rank_span = usize::MAX;
    assert_eq!(measured(&g), ordinary);
    g.rank_spacing = 1024.0 / usize::MAX as f64;
    let (d, units) = measured(&g);
    assert_eq!(units, ordinary.1);
    assert!(d.centers[1].y - d.centers[0].y >= 1026.0);
    g.edges.push(edge(1, 2));
    assert_eq!(layout(&g), Err(LayoutError::Overflow));
    g.edges.pop();
    g.rank_spacing = f64::MAX;
    assert_eq!(layout(&g), Err(LayoutError::Overflow));
    g.rank_spacing = 0.0;
    g.edges[0].start_clearance = f64::MAX;
    assert_eq!(layout(&g), Err(LayoutError::Overflow));
}

#[test]
fn coarse_float_coordinates_preserve_marker_room_or_reject_collapsed_geometry() {
    let mut g = graph(
        vec![size(16.0, 2f64.powi(58)), size(8.0, 512.0)],
        vec![Edge {
            start_clearance: 75.0,
            end_clearance: 128.0,
            ..edge(0, 1)
        }],
    );
    g.rank_spacing = 1024.0;
    let d = layout(&g).unwrap();
    let source_face = d.centers[0].y + g.nodes[0].height / 2.0;
    assert!(d.edges[0].points[1].y - source_face >= 75.0);
    let target_face = d.centers[1].y - g.nodes[1].height / 2.0;
    let points = &d.edges[0].points;
    assert!(target_face - points[points.len() - 2].y >= 128.0);

    let mut packed = graph(
        vec![size(2f64.powi(62), 2.0), Size::default(), Size::default()],
        vec![edge(1, 2); 2],
    );
    packed.node_spacing = 0.0;
    assert_eq!(layout(&packed), Err(LayoutError::Overflow));
    packed.nodes.truncate(1);
    packed.edges.clear();
    assert_eq!(layout(&packed).unwrap().bounds.max_x, packed.nodes[0].width);
    packed.nodes[0].width = f64::from_bits(1);
    assert_eq!(layout(&packed), Err(LayoutError::Overflow));
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
            g.edges.push(Edge {
                label: Some(size(81.0, 11.0)),
                ..edge(i, i + 3)
            });
        }
        g
    };
    let small = layout(&make(1)).unwrap();
    let g = make(30);
    let d = layout(&g).unwrap();
    check_geometry(&g, &d);
    assert_eq!(
        small.bounds.max_x - small.bounds.min_x,
        d.bounds.max_x - d.bounds.min_x
    );
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
    // The stride permutes a prime number of targets across several sort runs.
    let leaves = 4093;
    let mut g = graph(vec![size(24.0, 10.0); leaves + 1], Vec::new());
    for i in 0..leaves {
        g.edges.push(edge(0, 1 + i * 1657 % leaves));
    }
    let d = layout(&g).unwrap();
    let mut by_target: Vec<_> = d
        .edges
        .iter()
        .zip(&g.edges)
        .map(|(r, e)| (d.centers[e.target.0].x, r.points[1].x))
        .collect();
    by_target.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    assert!(by_target.windows(2).all(|pair| pair[0].1 < pair[1].1));
}

#[test]
fn wide_shared_fans_and_overlapping_long_edges_have_bounded_work_and_route_size() {
    let shortcuts = |n: usize| {
        let mut g = graph(
            vec![size(8.0, 5.0); n],
            (1..n).map(|i| edge(i - 1, i)).collect(),
        );
        g.edges.extend((2..n).rev().map(|i| edge(0, i)));
        g
    };
    for build in [hubs as fn(usize) -> Graph, shortcuts] {
        let (_, baseline) = measured(&build(2048));
        let g = build(4096);
        let (d, total) = measured(&g);
        assert!(
            total < 3 * baseline,
            "doubling input used {total} vs {baseline}"
        );
        assert!(total < (g.nodes.len() + g.edges.len()) * 200);
        assert_eq!(d.edges.len(), g.edges.len());
        for (e, route) in g.edges.iter().zip(d.edges) {
            assert!((4..=6).contains(&route.points.len()));
            assert_eq!(route.points.first(), Some(&d.centers[e.source.0]));
            assert_eq!(route.points.last(), Some(&d.centers[e.target.0]));
        }
    }
}

#[test]
fn caller_can_interrupt_at_admission_during_work_and_before_delivery() {
    let g = hubs(1800);
    let mut charges = Vec::new();
    let accepted = layout_with_control(&g, |units| {
        charges.push(units);
        Ok::<_, ()>(())
    })
    .unwrap();
    assert_eq!(accepted, layout(&g).unwrap());
    for reject in [0, charges.len() / 3, charges.len() / 2, charges.len() - 1] {
        let mut calls = 0;
        let error = layout_with_control(&g, |units| {
            assert_eq!(units, charges[calls]);
            calls += 1;
            if calls - 1 == reject {
                Err(format!("cancel at {reject}"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert_eq!(
            error,
            LayoutError::Interrupted(format!("cancel at {reject}"))
        );
        assert_eq!(calls, reject + 1);
    }
}

use super::*;
use crate::{NodeId, NoopWorkControl, WorkControl};

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}
fn size(w: f64, h: f64) -> Size {
    Size {
        width: w,
        height: h,
    }
}
fn p(x: f64, y: f64) -> Point {
    Point { x, y }
}
fn edge(a: u32, b: u32) -> EdgeSpec {
    EdgeSpec::new(NodeId(a).into(), NodeId(b).into())
}
fn seed_edge(a: Rect, b: Rect) -> EdgeGeometry {
    EdgeGeometry {
        points: vec![
            route_point(p(a.center().x, a.y + a.height), Axis::Vertical),
            route_point(p(b.center().x, b.y), Axis::Vertical),
        ],
        ports: (Side::Bottom, Side::Top),
        label_center: None,
        start_label_center: None,
        end_label_center: None,
        reversed_for_ranking: false,
    }
}
fn fixture(nodes: Vec<Rect>, ends: &[(u32, u32)]) -> (Drawing, Expanded, Component, Graph) {
    let mut graph = Graph::default();
    graph.nodes.extend(
        nodes
            .iter()
            .enumerate()
            .map(|(id, r)| NodeSpec::new(size(r.width, r.height), id as u32)),
    );
    graph.edges.extend(ends.iter().map(|&(a, b)| edge(a, b)));
    let mut c = Component::default();
    c.ids.extend(0..nodes.len() as u32);
    c.nodes = graph.nodes.clone();
    let mut edges = Vec::new();
    for (i, &(a, b)) in ends.iter().enumerate() {
        if a != b {
            c.links.push(super::super::Link {
                from: a,
                to: b,
                edge: i as u32,
                span: 1,
                weight: 1.0,
            });
            edges.push((i as u32, seed_edge(nodes[a as usize], nodes[b as usize])));
        }
    }
    let mut g = Expanded::default();
    g.direct.resize(c.links.len(), false);
    (Drawing { nodes, edges }, g, c, graph)
}
fn solve(d: &mut Drawing, g: &Expanded, c: &Component, graph: &Graph) -> LayoutResult<()> {
    let mut ctrl = NoopWorkControl;
    let mut work = Work::new(&mut ctrl);
    finish(d, g, c, graph, &Options::default(), &mut work)?;
    work.finish()
}
// Independent axis/interval checks on the public route, including its terminal shafts.
fn check_path(points: &[RoutePoint], boxes: &[Rect]) {
    for seg in points.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        if a.x == b.x && a.y == b.y {
            continue;
        }
        assert_eq!(
            b.axis,
            if a.x == b.x {
                Axis::Vertical
            } else {
                Axis::Horizontal
            }
        );
        assert!(a.x == b.x || a.y == b.y);
        for r in boxes {
            let hit = if a.x == b.x {
                r.x < a.x
                    && a.x < r.x + r.width
                    && r.y.max(a.y.min(b.y)) < (r.y + r.height).min(a.y.max(b.y))
            } else {
                r.y < a.y
                    && a.y < r.y + r.height
                    && r.x.max(a.x.min(b.x)) < (r.x + r.width).min(a.x.max(b.x))
            };
            assert!(!hit, "{seg:?} hits {r:?}");
        }
    }
}
fn label_rect(at: Point, s: Size) -> Rect {
    rect(
        at.x - s.width / 2.0,
        at.y - s.height / 2.0,
        s.width,
        s.height,
    )
}
fn disjoint(a: Rect, b: Rect) -> bool {
    a.x + a.width <= b.x || b.x + b.width <= a.x || a.y + a.height <= b.y || b.y + b.height <= a.y
}

#[test]
fn alternate_loops_keep_the_true_box_and_place_a_tall_label_in_its_reserved_band() {
    let owner = rect(0.0, 0.0, 20.0, 20.0);
    let wall = rect(36.0, -19.0, 21.0, 14.0);
    let (mut d, g, mut c, mut graph) = fixture(vec![owner, wall], &[(0, 0), (0, 0), (0, 0)]);
    for (i, (right, reach)) in [(true, 42.0), (false, 22.0), (true, 84.0)]
        .into_iter()
        .enumerate()
    {
        c.loops.push(super::super::Loop {
            edge: i as u32,
            node: 0,
            right,
            reach,
        });
        if i != 1 {
            graph.edges[i].label = Some(size(20.0, 60.0));
        }
    }
    solve(&mut d, &g, &c, &graph).unwrap();
    assert_eq!(d.nodes[0], owner);
    assert_eq!(
        d.edges
            .iter()
            .map(|(id, e)| (*id, e.ports.0))
            .collect::<Vec<_>>(),
        [(0, Side::Right), (1, Side::Left), (2, Side::Right)]
    );
    for (i, (_, e)) in d.edges.iter().enumerate() {
        assert_eq!(e.points.len(), 4);
        check_path(&e.points, &d.nodes);
        if let Some(at) = e.label_center {
            let r = label_rect(at, graph.edges[i].label.unwrap());
            assert!(disjoint(r, wall) && disjoint(r, owner));
            assert!(r.x >= e.points[1].x);
            assert!(r.x + r.width <= owner.x + owner.width + c.loops[i].reach);
        }
    }
    assert_ne!(d.edges[0].1.points[0].y, d.edges[2].1.points[0].y);
}

#[test]
fn fixed_and_anchored_sides_route_through_their_middle_label_around_foreign_boxes() {
    let (mut d, g, mut c, mut graph) = fixture(
        vec![
            rect(0., 0., 20., 30.),
            rect(110., 100., 30., 40.),
            rect(30., -10., 22., 85.),
            rect(60., 50., 18., 75.),
        ],
        &[(0, 1)],
    );
    c.nodes[1].anchor_y = Some(7.0);
    graph.edges[0].source_port = PortSideHint::FixedOffset {
        side: Side::Right,
        offset: -9.0,
    };
    graph.edges[0].target_port = PortSideHint::Side(Side::Left);
    graph.edges[0].label = Some(size(14., 12.));
    d.edges[0].1.label_center = Some(p(83., 20.));
    d.edges[0].1.reversed_for_ranking = true;
    solve(&mut d, &g, &c, &graph).unwrap();
    let e = &d.edges[0].1;
    assert_eq!(e.ports, (Side::Right, Side::Left));
    assert_eq!(point(e.points[0]), p(20., 6.));
    assert_eq!(point(*e.points.last().unwrap()), p(110., 107.));
    assert!(e.reversed_for_ranking);
    check_path(&e.points, &d.nodes);
    let label = e.label_center.unwrap();
    assert!(e.points.windows(2).any(|s| label.x >= s[0].x.min(s[1].x)
        && label.x <= s[0].x.max(s[1].x)
        && label.y >= s[0].y.min(s[1].y)
        && label.y <= s[0].y.max(s[1].y)));
}

#[test]
fn physical_anchor_and_signed_fixed_offsets_survive_all_canonical_transforms() {
    for dir in [
        Direction::Down,
        Direction::Up,
        Direction::Right,
        Direction::Left,
    ] {
        let mut spec = NodeSpec::new(size(40., 20.), 0);
        spec.anchor_y = Some(3.0);
        let s = canonical_size(dir, spec.size);
        let rect = rect(40., 50., s.width, s.height);
        let o = Port {
            at: rect.center(),
            side: Side::Bottom,
        };
        let anchored = resolve(o, PortSideHint::Side(Side::Right), &spec, rect, dir).unwrap();
        let fixed = resolve(
            o,
            PortSideHint::FixedOffset {
                side: Side::Right,
                offset: 6.0,
            },
            &spec,
            rect,
            dir,
        )
        .unwrap();
        let (side, anchor, off) = match dir {
            Direction::Down => (Side::Right, p(80., 53.), p(80., 66.)),
            Direction::Up => (Side::Right, p(80., 67.), p(80., 54.)),
            Direction::Right => (Side::Bottom, p(43., 90.), p(56., 90.)),
            Direction::Left => (Side::Top, p(43., 50.), p(56., 50.)),
        };
        assert_eq!(anchored.side, side);
        assert_eq!(anchored.at, anchor);
        assert_eq!(fixed.at, off);
    }
}

#[test]
fn capped_direct_edges_get_clear_routes_and_distinct_deferred_labels() {
    let (mut d, mut g, c, mut graph) = fixture(
        vec![
            rect(0., 0., 20., 20.),
            rect(0., 160., 20., 20.),
            rect(-8., 65., 36., 45.),
            rect(72., 0., 16., 20.),
            rect(72., 160., 16., 20.),
        ],
        &[(0, 1), (0, 1), (3, 4)],
    );
    g.direct = [true, true, false].into();
    graph.edges[0].label = Some(size(42., 22.));
    graph.edges[1].label = Some(size(42., 22.));
    graph.edges[2].label = Some(size(18., 20.));
    d.edges[2].1.label_center = Some(p(80., 90.));
    let ordinary = d.edges[2].1.clone();
    solve(&mut d, &g, &c, &graph).unwrap();
    assert_eq!(d.edges[2].1, ordinary);
    let mut boxes = d.nodes.clone();
    boxes.push(label_rect(p(80., 90.), size(18., 20.)));
    for i in 0..2 {
        let e = &d.edges[i].1;
        let label = label_rect(e.label_center.unwrap(), graph.edges[i].label.unwrap());
        assert!(boxes.iter().all(|r| disjoint(*r, label)));
        check_path(&e.points, &boxes);
        boxes.push(label);
    }
    check_path(
        &d.edges[0].1.points,
        &[label_rect(
            d.edges[1].1.label_center.unwrap(),
            graph.edges[1].label.unwrap(),
        )],
    );
}

#[test]
fn terminal_labels_clear_later_markers_middle_labels_nodes_and_one_another() {
    let (mut d, g, c, mut graph) = fixture(
        vec![
            rect(0., 0., 24., 20.),
            rect(0., 120., 24., 20.),
            rect(21., 0., 18., 20.),
            rect(21., 120., 18., 20.),
            rect(-35., 20., 36., 35.),
        ],
        &[(0, 1), (2, 3)],
    );
    for spec in &mut graph.edges {
        spec.start_label = Some(size(25., 17.));
        spec.end_label = Some(size(29., 19.));
    }
    graph.edges[1].start_marker = MarkerMetrics {
        length: 45.,
        width: 37.,
    };
    graph.edges[1].end_marker = MarkerMetrics {
        length: 37.,
        width: 31.,
    };
    graph.edges[0].label = Some(size(35., 20.));
    d.edges[0].1.label_center = Some(p(17., 70.));
    solve(&mut d, &g, &c, &graph).unwrap();
    let mut occupied = d.nodes.clone();
    occupied.extend([
        rect(11.5, 20., 37., 45.),
        rect(14.5, 83., 31., 37.),
        label_rect(p(17., 70.), size(35., 20.)),
    ]);
    for (id, e) in &d.edges {
        let spec = &graph.edges[*id as usize];
        for (p, s) in [
            (e.start_label_center, spec.start_label),
            (e.end_label_center, spec.end_label),
        ] {
            let label = label_rect(p.unwrap(), s.unwrap());
            assert!(
                occupied.iter().all(|r| disjoint(*r, label)),
                "{label:?} vs {occupied:?}"
            );
            occupied.push(label);
        }
    }
}

#[test]
fn interlocking_corridor_is_admitted_before_grid_allocation_and_clears_every_wall() {
    // Each terminal has one open exit. The source must leave up and the target is
    // accessible only from below. None of the two-elbow outside corridors connects them.
    let boxes = [
        rect(-3., -1., 2., 2.),
        rect(1., -1., 2., 2.),
        rect(-1., 1., 2., 2.),
        rect(7., -1., 2., 2.),
        rect(11., -1., 2., 2.),
        rect(9., -3., 2., 2.),
    ];
    struct Meter {
        max: usize,
        used: usize,
    }
    impl WorkControl for Meter {
        fn check(&mut self) -> Result<(), WorkError> {
            Ok(())
        }
        fn charge(&mut self, n: usize) -> Result<(), WorkError> {
            if self.used + n > self.max {
                return Err(WorkError::Interrupted);
            }
            self.used += n;
            Ok(())
        }
    }
    let draw = |budget| -> LayoutResult<(Vec<Point>, usize)> {
        let mut meter = Meter {
            max: budget,
            used: 0,
        };
        let mut w = Work::new(&mut meter);
        let mut obstacles = Obstacles::new();
        for b in boxes {
            obstacles.add(Owner::Node, Bounds::of(b)?, &mut w)?;
        }
        let mut path = Vec::new();
        let ports = [
            Port {
                at: p(0., 0.),
                side: Side::Top,
            },
            Port {
                at: p(10., 0.),
                side: Side::Bottom,
            },
        ];
        let walls = obstacles.for_route(0, ports, &mut w)?;
        corridor::connect(&mut path, p(0., 0.), p(10., 0.), &walls, 2., &mut w)?;
        w.finish()?;
        Ok((path, meter.used))
    };
    let (path, units) = draw(usize::MAX).unwrap();
    assert!(path.len() > 4);
    let pts: Vec<_> = path
        .iter()
        .enumerate()
        .map(|(i, &pt)| {
            route_point(
                pt,
                if i == 0 || path[i - 1].x == pt.x {
                    Axis::Vertical
                } else {
                    Axis::Horizontal
                },
            )
        })
        .collect();
    check_path(&pts, &boxes);
    assert!(units > 100);
    assert_eq!(draw(units).unwrap().0, path);
    assert_eq!(draw(100), Err(LayoutError::Work(WorkError::Interrupted)));
}

#[test]
fn coincident_loop_anchors_still_enclose_area_and_markers_can_share_a_shaft() {
    let owner = rect(0., 0., 20., 20.);
    let (mut d, mut g, mut c, mut graph) = fixture(
        vec![owner, rect(70., 90., 20., 20.)],
        &[(0, 1), (0, 1), (0, 0)],
    );
    g.direct.fill(true);
    for e in &mut graph.edges {
        e.source_port = PortSideHint::FixedOffset {
            side: Side::Right,
            offset: 0.,
        };
        e.start_marker = MarkerMetrics {
            length: 9.,
            width: 6.,
        };
    }
    graph.edges[2].target_port = graph.edges[2].source_port;
    graph.edges[2].end_marker = graph.edges[2].start_marker;
    c.loops.push(super::super::Loop {
        edge: 2,
        node: 0,
        right: true,
        reach: 22.,
    });
    solve(&mut d, &g, &c, &graph).unwrap();
    let at = p(20., 10.);
    for (_, e) in &d.edges {
        assert_eq!(point(e.points[0]), at);
        assert!(e.points[1].x - at.x >= 9.);
        check_path(&e.points, &d.nodes);
    }
    let ring = &d.edges[2].1.points;
    assert_eq!(point(*ring.last().unwrap()), at);
    assert!(ring.iter().any(|q| q.y > at.y) && ring.iter().any(|q| q.y < at.y));
    assert!(ring.iter().any(|q| q.x > at.x && q.y != at.y));
}

#[test]
fn a_missing_dummy_label_moves_out_of_a_sealed_pocket() {
    let (mut d, mut g, c, mut graph) = fixture(
        vec![
            rect(0., 0., 20., 20.),
            rect(0., 160., 20., 20.),
            rect(-20., 65., 60., 5.),
            rect(-20., 110., 60., 5.),
            rect(-20., 65., 5., 50.),
            rect(35., 65., 5., 50.),
        ],
        &[(0, 1)],
    );
    g.direct[0] = true;
    graph.edges[0].label = Some(size(8., 7.));
    solve(&mut d, &g, &c, &graph).unwrap();
    let e = &d.edges[0].1;
    let label = label_rect(e.label_center.unwrap(), graph.edges[0].label.unwrap());
    assert!(label.x + label.width <= -20. || label.x >= 40.);
    check_path(&e.points, &d.nodes);
}

#[test]
fn point_node_loops_have_area_and_named_top_ports_carry_their_label() {
    for height in [0.0, 30.0] {
        let (mut d, g, mut c, mut graph) = fixture(vec![rect(10., 25., 40., height)], &[(0, 0)]);
        c.loops.push(super::super::Loop {
            edge: 0,
            node: 0,
            right: true,
            reach: 42.,
        });
        graph.edges[0].label = Some(size(20., 18.));
        if height > 0.0 {
            graph.edges[0].source_port = PortSideHint::FixedOffset {
                side: Side::Top,
                offset: -7.,
            };
            graph.edges[0].target_port = PortSideHint::FixedOffset {
                side: Side::Top,
                offset: 3.,
            };
        }
        solve(&mut d, &g, &c, &graph).unwrap();
        let e = &d.edges[0].1;
        check_path(&e.points, &d.nodes);
        if height == 0.0 {
            assert!(e.points.iter().any(|p| p.y > 25.) && e.points.iter().any(|p| p.y < 25.));
        } else {
            assert_eq!(e.ports, (Side::Top, Side::Top));
            assert_eq!(point(e.points[0]), p(23., 25.));
            assert_eq!(point(*e.points.last().unwrap()), p(33., 25.));
            let label = label_rect(e.label_center.unwrap(), graph.edges[0].label.unwrap());
            assert!(
                label.y + label.height <= e.points.iter().map(|p| p.y).reduce(f64::min).unwrap()
            );
        }
    }
}

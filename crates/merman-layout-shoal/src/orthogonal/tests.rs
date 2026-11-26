use super::*;
use crate::{Direction, NodeSpec, NoopWorkControl, WorkControl, WorkError, layout_with_control};

fn knots(points: &[(f64, f64)]) -> Vec<RoutePoint> {
    points
        .iter()
        .map(|&(x, y)| RoutePoint::new(x, y, Axis::Vertical))
        .collect()
}

fn edge(points: &[(f64, f64)], ports: (Side, Side)) -> EdgeGeometry {
    EdgeGeometry {
        points: knots(points),
        ports,
        label_center: None,
        start_label_center: None,
        end_label_center: None,
        shared_bus: false,
        corridor_clearance: 0.0,
        reversed_for_ranking: false,
    }
}

fn spec(source: u32, target: u32) -> EdgeSpec {
    let mut spec = EdgeSpec::new(
        Endpoint::Node(crate::NodeId(source)),
        Endpoint::Node(crate::NodeId(target)),
    );
    spec.start_marker = MarkerMetrics {
        width: 8.0,
        length: 10.0,
    };
    spec.end_marker = spec.start_marker;
    spec
}

fn route_intersections(
    a: &[RoutePoint],
    b: &[RoutePoint],
    work: &mut Work<'_>,
) -> LayoutResult<(u64, usize)> {
    let (_, projection_a) = Projection::new(a, work)?;
    let (_, projection_b) = Projection::new(b, work)?;
    intersections(
        RouteView {
            points: a,
            projection: &projection_a,
        },
        RouteView {
            points: b,
            projection: &projection_b,
        },
        work,
    )
}

#[test]
fn crossings_count_edge_pairs_and_overlaps_count_every_coincident_leg_pair() {
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let main = knots(&[(0., 0.), (10., 0.), (10., 8.), (0., 8.)]);
    for (peer, expected) in [
        (
            knots(&[(3., -2.), (3., 10.), (6., 10.), (6., 8.), (1., 8.)]),
            (1, 1),
        ),
        (knots(&[(10., 0.), (10., -5.)]), (0, 0)),
        (knots(&[(0., -2.), (10., 4.)]), (1, 0)),
    ] {
        assert_eq!(
            route_intersections(&main, &peer, &mut work).unwrap(),
            expected
        );
        assert_eq!(
            route_intersections(&peer, &main, &mut work).unwrap(),
            expected
        );
    }
    let repeated = knots(&[(0., 0.), (5., 0.), (5., -0.), (0., 0.)]);
    let coincident = knots(&[(1., 0.), (4., 0.), (1., 0.)]);
    assert_eq!(
        route_intersections(&repeated, &coincident, &mut work).unwrap(),
        (0, 4)
    );

    let tiny = f64::MIN_POSITIVE;
    let horizontal = knots(&[(-tiny, 0.), (tiny, 0.)]);
    let vertical = knots(&[(0., -tiny), (0., tiny)]);
    assert_eq!(
        route_intersections(&horizontal, &vertical, &mut work).unwrap(),
        (0, 0)
    );
}

#[derive(Default)]
struct Meter(usize);

impl WorkControl for Meter {
    fn check(&mut self) -> Result<(), WorkError> {
        Ok(())
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.0 += units;
        Ok(())
    }
}

#[test]
fn disjoint_routes_charge_for_projection_and_edge_bounds() {
    for (edge_count, legs) in [(32, 8), (MAX_ROUTES, 32)] {
        let drawing = LayoutOutput {
            edges: (0..edge_count)
                .map(|i| {
                    let points: Vec<_> = (0..=legs)
                        .map(|p| ((i * 100 + p / 2) as f64, p.div_ceil(2) as f64))
                        .collect();
                    edge(&points, (Side::Bottom, Side::Top))
                })
                .collect(),
            ..Default::default()
        };
        let mut meter = Meter::default();
        let mut work = Work::new(&mut meter);
        let routes = Routes::new(&drawing.edges, &mut work).unwrap();
        for i in [0, edge_count / 2, edge_count - 1] {
            let score = routes
                .quality(i, routes.edge_bounds[i], routes.view(i), &mut work)
                .unwrap();
            assert_eq!((score.crossings, score.overlaps), (0, 0));
            assert_eq!((score.bends, score.length), (legs - 1, legs as f64));
        }
        work.finish().unwrap();
        assert!(
            meter.0 >= edge_count * legs + (edge_count - 1) * 3,
            "{}",
            meter.0
        );
        assert!(meter.0 <= edge_count * (legs + 8), "{}", meter.0);
    }
}

#[test]
fn quality_sees_new_envelopes_and_legs_after_each_replacement() {
    let edges = [
        edge(&[(4., 1.), (4., 9.)], (Side::Bottom, Side::Top)),
        edge(
            &[(1., 0.), (1., 10.), (2., 10.), (2., 0.)],
            (Side::Bottom, Side::Bottom),
        ),
    ];
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut routes = Routes::new(&edges, &mut work).unwrap();
    for (replacement, expected) in [
        (&edges[1].points[..], (0, 0)),
        (
            &knots(&[(1., 0.), (1., 5.), (6., 5.), (6., 8.), (2., 8.), (2., 0.)]),
            (1, 0),
        ),
        (
            &knots(&[(1., 0.), (1., 2.), (4., 2.), (4., 7.), (2., 7.), (2., 0.)]),
            (0, 1),
        ),
        (&edges[1].points[..], (0, 0)),
    ] {
        routes.replace(1, Candidate::new(replacement.to_vec(), &mut work).unwrap());
        let score = routes
            .quality(0, routes.edge_bounds[0], routes.view(0), &mut work)
            .unwrap();
        assert_eq!((score.crossings, score.overlaps), expected);
    }
}

#[test]
fn dense_leg_comparisons_poll_cancellation() {
    struct CancelAfterFirstCharge(bool);

    impl WorkControl for CancelAfterFirstCharge {
        fn check(&mut self) -> Result<(), WorkError> {
            if self.0 {
                Err(WorkError::Interrupted)
            } else {
                Ok(())
            }
        }

        fn charge(&mut self, _units: usize) -> Result<(), WorkError> {
            self.0 = true;
            Ok(())
        }
    }

    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let main = Candidate::new(knots(&[(0., 0.), (10., 0.), (0., 0.)]), &mut work).unwrap();
    let peers = (0..=2048)
        .map(|i| RoutePoint::new(1. + (i % 2) as f64, 0., Axis::Horizontal))
        .collect();
    let peers = Candidate::new(peers, &mut work).unwrap();
    let mut cancel = CancelAfterFirstCharge(false);
    assert_eq!(
        intersections(main.view(), peers.view(), &mut Work::new(&mut cancel)),
        Err(LayoutError::Work(WorkError::Interrupted))
    );
}

#[test]
fn parallel_legs_separate_in_approach_order_without_moving_their_contacts() {
    let graph = Graph {
        edges: vec![spec(0, 1), spec(2, 3)],
        ..Graph::default()
    };
    let drawing = LayoutOutput {
        bounds: Size {
            width: 100.0,
            height: 100.0,
        },
        edges: vec![
            edge(
                &[(30., 20.), (30., 50.), (70., 50.), (70., 20.)],
                (Side::Bottom, Side::Bottom),
            ),
            edge(
                &[(20., 80.), (20., 50.), (80., 50.), (80., 80.)],
                (Side::Top, Side::Top),
            ),
        ],
        ..Default::default()
    };
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let bounds = root_bounds(&drawing);
    let markers = obstacles(&graph, &drawing, &mut work).unwrap();
    let router = OrthoRouter::new(&markers, 4.0, 256, &mut work).unwrap();
    let mut routes = Routes::new(&drawing.edges, &mut work).unwrap();
    channels::Scene {
        router: &router,
        graph: &graph,
        drawing: &drawing,
        bounds,
        spacing: 4.0,
    }
    .nudge(&[0, 1], &mut routes, &mut work)
    .unwrap();
    let [Some(a), Some(b)] = routes.changes.as_slice() else {
        panic!("expected both legs to move")
    };
    assert_eq!([a[1].y, b[1].y], [48.0, 52.0]);
    assert_eq!(route_intersections(a, b, &mut work).unwrap(), (0, 0));
    for (points, original) in [a, b].into_iter().zip(drawing.edges) {
        assert_eq!(points.first(), original.points.first());
        assert_eq!(points.last(), original.points.last());
        assert!((points[0].y - points[1].y).abs() >= 10.0);
        assert!((points[3].y - points[2].y).abs() >= 10.0);
    }
}

#[test]
fn channel_shift_rejects_a_new_crossing_outside_the_old_route_bounds() {
    let mut graph = Graph {
        edges: vec![spec(0, 1), spec(2, 3)],
        ..Graph::default()
    };
    let mut drawing = LayoutOutput {
        bounds: Size {
            width: 100.,
            height: 100.,
        },
        edges: vec![
            edge(
                &[(30., 20.), (30., 50.), (70., 50.), (70., 20.)],
                (Side::Bottom, Side::Bottom),
            ),
            edge(
                &[(20., 20.), (20., 50.), (80., 50.), (80., 20.)],
                (Side::Bottom, Side::Bottom),
            ),
        ],
        ..Default::default()
    };
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let router = OrthoRouter::new(&[], 4., 256, &mut work).unwrap();
    let shift = |graph: &Graph, drawing: &LayoutOutput, work: &mut Work<'_>| {
        let mut routes = Routes::new(&drawing.edges, work).unwrap();
        channels::Scene {
            router: &router,
            graph,
            drawing,
            bounds: root_bounds(drawing),
            spacing: 4.,
        }
        .nudge(&[0, 1], &mut routes, work)
        .unwrap();
        routes.changes
    };
    let changes = shift(&graph, &drawing, &mut work);
    assert_eq!(changes[1].as_ref().unwrap()[1].y, 52.);

    graph.edges.push(spec(4, 5));
    drawing
        .edges
        .push(edge(&[(50., 51.), (50., 56.)], (Side::Bottom, Side::Top)));
    assert!(
        shift(&graph, &drawing, &mut work)
            .iter()
            .all(Option::is_none)
    );
}

#[test]
fn a_clearer_route_keeps_the_box_faces_and_rejects_an_obstacle_through_its_shortcut() {
    let graph = Graph {
        nodes: (0..3)
            .map(|i| {
                NodeSpec::new(
                    Size {
                        width: 16.,
                        height: 12.,
                    },
                    i,
                )
            })
            .collect(),
        edges: vec![spec(0, 1)],
        ..Graph::default()
    };
    let nodes = [
        Rect {
            x: 40.,
            y: 0.,
            width: 20.,
            height: 20.,
        },
        Rect {
            x: 40.,
            y: 80.,
            width: 20.,
            height: 20.,
        },
        Rect {
            x: 43.,
            y: 40.,
            width: 14.,
            height: 20.,
        },
    ];
    let points = [
        (50., 20.),
        (50., 34.),
        (25., 34.),
        (25., 72.),
        (50., 72.),
        (50., 80.),
    ];
    let mut options = Options {
        global_orthogonal: true,
        routing: RoutingStyle::Orthogonal,
        ..Default::default()
    };
    options.spacing.edge = 4.;
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    work.charge(10_000).unwrap();
    for blocked in [false, true] {
        let mut drawing = LayoutOutput {
            nodes: nodes[..2 + usize::from(blocked)].to_vec(),
            bounds: Size {
                width: 100.,
                height: 100.,
            },
            edges: vec![edge(&points, (Side::Bottom, Side::Top))],
            ..Default::default()
        };
        // Both ends leave at least 12 px of clear lead before the first turn.
        drawing.edges[0].points[3].y = 67.;
        drawing.edges[0].points[4].y = 67.;
        refine(&graph, &mut drawing, &options, &mut work).unwrap();
        let points = &drawing.edges[0].points;
        assert_eq!(point(points[0]), Point { x: 50., y: 20. });
        assert_eq!(point(*points.last().unwrap()), Point { x: 50., y: 80. });
        assert_eq!(points.iter().all(|p| p.x == 50.), !blocked);
        if blocked {
            for pair in points.windows(2) {
                let leg = OrthoSegment::between(pair[0], pair[1]).unwrap();
                if leg.axis == Axis::Vertical && leg.min < 60. && leg.max > 40. {
                    assert!(leg.fixed <= 41. || leg.fixed >= 59.);
                }
            }
        }
    }
}

#[test]
fn optional_work_rejection_keeps_the_complete_layered_drawing() {
    struct RequiredOnly(usize);
    impl WorkControl for RequiredOnly {
        fn check(&mut self) -> Result<(), WorkError> {
            Ok(())
        }
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.0 += units;
            Ok(())
        }
    }
    let graph = Graph {
        direction: Direction::Right,
        nodes: (0..4)
            .map(|i| {
                NodeSpec::new(
                    Size {
                        width: 40.,
                        height: 30.,
                    },
                    i,
                )
            })
            .collect(),
        edges: vec![spec(0, 1), spec(0, 2), spec(1, 3), spec(2, 3)],
        ..Graph::default()
    };
    let mut options = Options {
        routing: RoutingStyle::Orthogonal,
        ..Default::default()
    };
    let solve = |options: &Options| {
        let mut control = RequiredOnly(0);
        let drawing =
            layout_with_control(&graph, options, &mut Default::default(), &mut control).unwrap();
        (drawing, control.0)
    };
    let baseline = solve(&options);
    options.global_orthogonal = true;
    assert_eq!(solve(&options), baseline);
}

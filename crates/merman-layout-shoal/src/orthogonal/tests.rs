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
    let mut changes = vec![None, None];
    channels::Scene {
        router: &router,
        graph: &graph,
        drawing: &drawing,
        bounds,
        spacing: 4.0,
    }
    .nudge(&[0, 1], &mut changes, &mut work)
    .unwrap();
    let [Some(a), Some(b)] = changes.as_slice() else {
        panic!("expected both legs to move")
    };
    assert_eq!([a[1].y, b[1].y], [48.0, 52.0]);
    assert_eq!(intersections(a, b, &mut work).unwrap(), (0, 0));
    for (points, original) in [a, b].into_iter().zip(drawing.edges) {
        assert_eq!(points.first(), original.points.first());
        assert_eq!(points.last(), original.points.last());
        assert!((points[0].y - points[1].y).abs() >= 10.0);
        assert!((points[3].y - points[2].y).abs() >= 10.0);
    }
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

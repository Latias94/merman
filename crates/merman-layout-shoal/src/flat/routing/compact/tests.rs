use super::*;
use crate::{NodeId, NoopWorkControl};

fn edge(a: (f64, f64), b: (f64, f64), ports: (Side, Side)) -> EdgeGeometry {
    EdgeGeometry {
        points: vec![
            RoutePoint::new(a.0, a.1, ports.0.axis()),
            RoutePoint::new(b.0, b.1, ports.1.axis()),
        ],
        ports,
        label_center: None,
        start_label_center: None,
        end_label_center: None,
        shared_bus: false,
        corridor_clearance: 0.0,
        reversed_for_ranking: false,
    }
}

fn try_slide(approach: EdgeSpec, wall: EdgeSpec, route: EdgeGeometry, earliest: f64) -> Drawing {
    let graph = Graph {
        edges: vec![approach, wall],
        ..Graph::default()
    };
    let mut drawing = Drawing {
        nodes: vec![
            Rect {
                x: 10.0,
                y: 0.0,
                width: 20.0,
                height: 20.0,
            },
            Rect {
                x: 10.0,
                y: 120.0,
                width: 20.0,
                height: 20.0,
            },
        ],
        edges: vec![
            (
                0,
                edge((20.0, 20.0), (20.0, 120.0), (Side::Bottom, Side::Top)),
            ),
            (1, route),
        ],
    };
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut remaining = MAX_WORK;
    let mut scene = Scene::read(&drawing, &graph, &mut remaining, &mut work)
        .unwrap()
        .unwrap();
    slide(
        &mut drawing,
        &graph,
        &mut scene,
        1,
        (0, None),
        earliest,
        4.0,
    );
    drawing
}

#[test]
fn a_foreign_shaft_marker_or_label_can_hold_the_original_rank_position() {
    let spec = EdgeSpec::new(NodeId(0).into(), NodeId(1).into());
    let outside = edge((0.0, 75.0), (0.0, 112.0), (Side::Bottom, Side::Top));
    assert_eq!(
        try_slide(spec, spec, outside.clone(), 70.0).nodes[1].y,
        70.0
    );

    let shaft = edge((0.0, 111.0), (50.0, 111.0), (Side::Right, Side::Left));
    let mut marked = spec;
    marked.end_marker = MarkerMetrics {
        length: 8.0,
        width: 40.0,
    };
    let mut labelled = spec;
    let mut label_route = outside.clone();
    labelled.label = Some(Size {
        width: 20.0,
        height: 6.0,
    });
    label_route.label_center = Some(Point { x: 30.0, y: 109.0 });
    for (wall, route) in [(spec, shaft), (marked, outside), (labelled, label_route)] {
        let drawing = try_slide(spec, wall, route.clone(), 70.0);
        assert_eq!(drawing.nodes[1].y, 120.0);
        assert_eq!(drawing.edges[0].1.points[1].y, 120.0);
        assert_eq!(drawing.edges[1].1, route);
    }
}

#[test]
fn the_shortened_shaft_keeps_the_sum_of_both_marker_leads() {
    let mut approach = EdgeSpec::new(NodeId(0).into(), NodeId(1).into());
    let wall = approach;
    approach.start_marker = MarkerMetrics {
        length: 38.0,
        width: 8.0,
    };
    approach.end_marker = MarkerMetrics {
        length: 28.0,
        width: 8.0,
    };
    let outside = edge((0.0, 75.0), (0.0, 112.0), (Side::Bottom, Side::Top));
    let drawing = try_slide(approach, wall, outside, 30.0);
    let route = &drawing.edges[0].1;
    assert_eq!(drawing.nodes[1].y, 86.0);
    assert_eq!(route.points[1].y, drawing.nodes[1].y);
    assert_eq!(route.points[1].y - route.points[0].y, 66.0);
    assert_eq!(route.ports, (Side::Bottom, Side::Top));
}

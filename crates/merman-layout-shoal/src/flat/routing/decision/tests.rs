use super::*;
use crate::flat::Link;
use crate::{
    EdgeGeometry, EdgeSpec, Endpoint, NodeId, NodeSpec, NoopWorkControl, Rect, ShapeContourHint,
};

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn route(x: f64, y: f64, end_x: f64, end_y: f64, label: Option<Point>) -> EdgeGeometry {
    EdgeGeometry {
        points: vec![
            RoutePoint::new(x, y, Axis::Vertical),
            RoutePoint::new(x, end_y - 10.0, Axis::Vertical),
            RoutePoint::new(end_x, end_y - 10.0, Axis::Horizontal),
            RoutePoint::new(end_x, end_y, Axis::Vertical),
        ],
        ports: (Side::Bottom, Side::Top),
        label_center: label,
        start_label_center: None,
        end_label_center: None,
        corridor_clearance: 0.0,
        shared_bus: false,
        reversed_for_ranking: false,
    }
}

#[test]
fn replayed_labels_clear_foreign_nodes_shafts_and_markers_or_keep_both_original_routes() {
    for obstruction in ["clear", "node", "shaft", "marker"] {
        let x = if obstruction == "shaft" { 45.0 } else { 60.0 };
        let mut drawing = Drawing {
            nodes: vec![
                rect(80.0, 40.0, 40.0, 30.0),
                rect(15.0, 155.0, 30.0, 20.0),
                rect(155.0, 155.0, 30.0, 20.0),
                rect(x - 0.5, 104.0, 1.0, 1.0),
                rect(x - 0.5, 151.0, 1.0, 1.0),
            ],
            edges: vec![
                (
                    0,
                    route(95.0, 70.0, 30.0, 155.0, Some(Point { x: 60.0, y: 145.0 })),
                ),
                (
                    1,
                    route(
                        105.0,
                        70.0,
                        170.0,
                        155.0,
                        Some(Point { x: 140.0, y: 145.0 }),
                    ),
                ),
                (2, route(x, 105.0, x, 151.0, None)),
            ],
        };
        if obstruction == "node" {
            drawing.nodes.push(rect(44.0, 100.0, 3.0, 40.0));
        }
        let mut graph = Graph {
            nodes: drawing
                .nodes
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    NodeSpec::new(
                        Size {
                            width: r.width,
                            height: r.height,
                        },
                        i as u32,
                    )
                })
                .collect(),
            ..Graph::default()
        };
        graph.nodes[0].contour = ShapeContourHint::Diamond;
        let links: Vec<_> = [(0, 1), (0, 2), (3, 4)]
            .into_iter()
            .enumerate()
            .map(|(i, (from, to))| {
                let mut edge =
                    EdgeSpec::new(Endpoint::Node(NodeId(from)), Endpoint::Node(NodeId(to)));
                if i < 2 {
                    edge.label = Some(Size {
                        width: 30.0,
                        height: 16.0,
                    });
                    edge.end_marker = MarkerMetrics {
                        length: 20.0,
                        width: 14.0,
                    };
                } else if obstruction == "marker" {
                    edge.end_marker = MarkerMetrics {
                        length: 47.0,
                        width: 42.0,
                    };
                }
                graph.edges.push(edge);
                Link {
                    from,
                    to,
                    edge: i as u32,
                    span: 1,
                    weight: 1.0,
                }
            })
            .collect();
        let component = Component {
            nodes: graph.nodes.clone(),
            links,
            ..Default::default()
        };
        let expanded = Expanded {
            rank: vec![0, 1, 1, 0, 1, 1],
            direct: vec![false; 3],
            ..Default::default()
        };
        let options = Options {
            routing: RoutingStyle::Orthogonal,
            ..Default::default()
        };
        let before = drawing.edges.clone();
        dock(
            &mut drawing,
            &component,
            &expanded,
            &graph,
            &options,
            &mut Work::new(&mut NoopWorkControl),
        )
        .unwrap();
        if obstruction == "clear" {
            for (index, side) in [(0, Side::Left), (1, Side::Right)] {
                let edge = &drawing.edges[index].1;
                assert_eq!(edge.ports, (side, Side::Top));
                let [_, bend, end] = edge.points.as_slice() else {
                    panic!("expected a single elbow")
                };
                let label = edge.label_center.unwrap();
                assert_eq!(label.x, bend.x);
                assert!(bend.y < label.y && label.y + 8.0 < end.y - 20.0);
            }
        } else {
            assert_eq!(drawing.edges, before, "{obstruction} blocks the left label");
        }
    }
}

const OFFSETS: [(Direction, f64, f64); 4] = [
    (Direction::Down, 8.0, 5.0),
    (Direction::Up, 8.0, -5.0),
    (Direction::Right, 5.0, 8.0),
    (Direction::Left, 5.0, -8.0),
];

fn offset_decision(direction: Direction) -> (Drawing, Component, Expanded, Graph) {
    let drawing = Drawing {
        nodes: vec![
            rect(80.0, 40.0, 40.0, 30.0),
            rect(10.0, 155.0, 40.0, 20.0),
            rect(150.0, 155.0, 40.0, 20.0),
        ],
        edges: vec![
            (0, route(95.0, 70.0, 30.0, 155.0, None)),
            (1, route(105.0, 70.0, 170.0, 155.0, None)),
        ],
    };
    let mut graph = Graph {
        direction,
        nodes: drawing
            .nodes
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let size = Size {
                    width: r.width,
                    height: r.height,
                };
                NodeSpec::new(canonical_size(direction, size), i as u32)
            })
            .collect(),
        ..Graph::default()
    };
    graph.nodes[0].contour = ShapeContourHint::OffsetDiamond { x: 8.0, y: 5.0 };
    graph.nodes[1].contour = ShapeContourHint::OffsetDiamond { x: -6.0, y: -8.0 };
    let links = (1..=2)
        .map(|to| {
            let mut edge = EdgeSpec::new(Endpoint::Node(NodeId(0)), Endpoint::Node(NodeId(to)));
            edge.start_marker = MarkerMetrics {
                length: 20.0,
                width: 14.0,
            };
            graph.edges.push(edge);
            Link {
                from: 0,
                to,
                edge: to - 1,
                span: 1,
                weight: 1.0,
            }
        })
        .collect();
    let component = Component {
        nodes: graph.nodes.clone(),
        links,
        ..Default::default()
    };
    let expanded = Expanded {
        rank: vec![0, 1, 1],
        direct: vec![false; 2],
        ..Default::default()
    };
    (drawing, component, expanded, graph)
}

#[test]
fn offset_decision_docks_at_the_painted_vertices_in_all_directions() {
    for ((direction, dx, dy), target_inset) in OFFSETS.into_iter().zip([-5.0, 11.0, -2.0, 10.0]) {
        let (mut drawing, component, expanded, graph) = offset_decision(direction);
        let options = Options {
            routing: RoutingStyle::Orthogonal,
            ..Default::default()
        };
        dock(
            &mut drawing,
            &component,
            &expanded,
            &graph,
            &options,
            &mut Work::new(&mut NoopWorkControl),
        )
        .unwrap();
        for (i, (side, face_x, lane)) in [(Side::Left, 80.0, 30.0), (Side::Right, 120.0, 170.0)]
            .into_iter()
            .enumerate()
        {
            let edge = &drawing.edges[i].1;
            assert_eq!(edge.ports, (side, Side::Top), "{direction:?}");
            let y = 55.0 + dy;
            assert_eq!(
                edge.points,
                [
                    RoutePoint::new(face_x, y, Axis::Horizontal),
                    RoutePoint::new(lane, y, Axis::Horizontal),
                    RoutePoint::new(lane, 155.0, Axis::Vertical),
                ],
                "{direction:?} {side:?}"
            );
            assert_eq!(
                contact(&drawing, &component, direction, 0, side, edge.points[0]),
                Point { x: face_x + dx, y },
                "{direction:?} {side:?}"
            );
        }
        assert_eq!(
            contact(
                &drawing,
                &component,
                direction,
                1,
                Side::Top,
                drawing.edges[0].1.points[2],
            ),
            Point {
                x: 30.0,
                y: 155.0 + target_inset,
            },
            "{direction:?}"
        );
    }
}

#[test]
fn offset_decision_keeps_both_original_routes_when_its_painted_marker_is_blocked() {
    let (mut drawing, mut component, expanded, mut graph) = offset_decision(Direction::Down);
    // Above the shaft and past the reach of a marker at the measured right face.
    drawing.nodes.push(rect(145.0, 55.0, 1.0, 1.0));
    graph.nodes.push(NodeSpec::new(
        Size {
            width: 1.0,
            height: 1.0,
        },
        3,
    ));
    component.nodes.push(graph.nodes[3]);
    let before = drawing.edges.clone();
    let options = Options {
        routing: RoutingStyle::Orthogonal,
        ..Default::default()
    };
    dock(
        &mut drawing,
        &component,
        &expanded,
        &graph,
        &options,
        &mut Work::new(&mut NoopWorkControl),
    )
    .unwrap();
    assert_eq!(drawing.edges, before);
}

#[test]
fn offset_diamond_blocks_labels_at_a_protruding_vertex_and_clears_its_empty_corner() {
    for (direction, dx, dy) in OFFSETS {
        let (mut drawing, component, _, graph) = offset_decision(direction);
        drawing.edges.clear();
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let index = walls(&drawing, &component, &graph, &mut work).unwrap();
        let protruding = Bounds::from_half_extents(
            Point {
                x: 118.0 + dx,
                y: 55.0 + dy,
            },
            1.0,
            1.0,
        );
        let empty_corner = Bounds::from_half_extents(
            Point {
                x: 84.0 + dx,
                y: 55.0 + dy - 10.0 * dy.signum(),
            },
            1.0,
            1.0,
        );
        let is_clear = [protruding, empty_corner]
            .map(|label| clear(&index, label, [0, 1], None, &mut work).unwrap());
        assert_eq!(is_clear, [false, true], "{direction:?}");
    }
}

use super::*;
use crate::flat::Link;
use crate::{EdgeGeometry, EdgeSpec, Endpoint, NodeId, NodeSpec, NoopWorkControl, Rect};

#[test]
fn replayed_labels_clear_foreign_nodes_shafts_and_markers_or_keep_both_original_routes() {
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

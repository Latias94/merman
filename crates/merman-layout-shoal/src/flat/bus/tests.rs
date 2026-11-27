use super::*;
use crate::{EdgeGeometry, EdgeSpec, NodeId, NodeSpec, NoopWorkControl, ShapeContourHint, Size};

const FAN_SIZE: usize = 6;

fn fixture(incoming: bool, obstruction: Option<Rect>) -> (Graph, Component, Drawing) {
    let hub = Rect {
        x: -42.0,
        y: if incoming { 124.0 } else { 0.0 },
        width: 84.0,
        height: 36.0,
    };
    let mut nodes = vec![hub];
    nodes.extend([-180.0, -120.0, -60.0, 60.0, 120.0, 180.0].map(|x| Rect {
        x: x - 15.0,
        y: if incoming { 0.0 } else { 124.0 },
        width: 30.0,
        height: 36.0,
    }));
    nodes.extend(obstruction);
    let marker = MarkerMetrics {
        length: 10.0,
        width: 8.0,
    };
    let mut graph = Graph {
        nodes: nodes
            .iter()
            .enumerate()
            .map(|(id, r)| {
                NodeSpec::new(
                    Size {
                        width: r.width,
                        height: r.height,
                    },
                    id as u32,
                )
            })
            .collect(),
        ..Graph::default()
    };
    let contact = |node: usize, side: Side, dx: f64| {
        let rect = nodes[node];
        RoutePoint::new(
            rect.center().x + dx,
            if side == Side::Bottom {
                rect.y + rect.height
            } else {
                rect.y
            },
            Axis::Vertical,
        )
    };
    let mut edges = Vec::new();
    for index in 0..FAN_SIZE {
        let (source, target) = if incoming {
            (index + 1, 0)
        } else {
            (0, index + 1)
        };
        let mut spec = EdgeSpec::new(NodeId(source as u32).into(), NodeId(target as u32).into());
        spec.start_marker = marker;
        spec.end_marker = marker;
        graph.edges.push(spec);
        let hub_dx = -20.0 + 8.0 * index as f64;
        let from = contact(source, Side::Bottom, if incoming { 0.0 } else { hub_dx });
        let to = contact(target, Side::Top, if incoming { hub_dx } else { 0.0 });
        let y = 57.0 + 7.0 * index as f64;
        edges.push((
            index as u32,
            EdgeGeometry {
                points: vec![
                    from,
                    RoutePoint::new(from.x, y, Axis::Vertical),
                    RoutePoint::new(to.x, y, Axis::Horizontal),
                    to,
                ],
                ports: (Side::Bottom, Side::Top),
                label_center: None,
                start_label_center: None,
                end_label_center: None,
                shared_bus: false,
                corridor_clearance: 0.0,
                reversed_for_ranking: false,
            },
        ));
    }
    let options = Options::default();
    let component = super::super::validate::collect(
        &graph,
        &options,
        true,
        &mut Work::new(&mut NoopWorkControl),
    )
    .unwrap()
    .pop()
    .unwrap();
    (graph, component, Drawing { nodes, edges })
}

fn apply(drawing: &mut Drawing, c: &Component, graph: &Graph, options: &Options) -> bool {
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    work.charge(10_000).unwrap();
    let result = bundle(drawing, c, graph, options, &mut work).unwrap();
    work.finish().unwrap();
    result
}

fn assert_bus_points(drawing: &Drawing, edges: &[(u32, EdgeGeometry)], incoming: bool) -> f64 {
    let mut shelf = None;
    for ((id, route), (old_id, old)) in drawing.edges.iter().zip(edges) {
        assert_eq!(id, old_id);
        assert_eq!(route.ports, old.ports);
        assert!(route.shared_bus);
        assert_eq!(route.corridor_clearance, 0.0);
        assert_eq!(route.reversed_for_ranking, old.reversed_for_ranking);
        let points = &route.points;
        let (leaf, hub) = if incoming {
            (points.first().unwrap(), points.last().unwrap())
        } else {
            (points.last().unwrap(), points.first().unwrap())
        };
        assert_eq!(
            Some(leaf),
            if incoming {
                old.points.first()
            } else {
                old.points.last()
            }
        );
        assert!(hub.x.abs() <= 20.0);
        assert_eq!(hub.y, if incoming { 124.0 } else { 36.0 });
        let mut horizontal = false;
        for segment in points.windows(2) {
            let [a, b] = [segment[0], segment[1]];
            if a.y == b.y {
                assert_ne!(a.x, b.x);
                assert_eq!(b.axis, Axis::Horizontal);
                assert_eq!(*shelf.get_or_insert(a.y), a.y);
                horizontal = true;
            } else {
                assert_eq!(a.x, b.x);
                assert_eq!(b.axis, Axis::Vertical);
                assert!(a.y < b.y);
            }
        }
        assert!(horizontal);
    }
    shelf.unwrap()
}

#[test]
fn rejects_curved_polyline_and_explicit_opt_out() {
    let (graph, c, mut drawing) = fixture(true, None);
    let before = drawing.edges.clone();
    for (routing, bus_routing) in [
        (RoutingStyle::Orthogonal, false),
        (RoutingStyle::Curved, true),
        (RoutingStyle::Polyline, true),
    ] {
        let options = Options {
            routing,
            bus_routing,
            ..Options::default()
        };
        assert!(!apply(&mut drawing, &c, &graph, &options));
        assert_eq!(drawing.edges, before);
    }
}

#[test]
fn default_layout_bundles_six_unlabelled_orthogonal_branches() {
    let (mut graph, _, _) = fixture(false, None);
    let options = Options {
        routing: RoutingStyle::Orthogonal,
        ..Options::default()
    };
    let drawing = crate::layout(&graph, &options).unwrap();
    assert!(drawing.edges.iter().all(|edge| edge.shared_bus));

    graph.edges[0].end_label = Some(Size {
        width: 18.0,
        height: 8.0,
    });
    let drawing = crate::layout(&graph, &options).unwrap();
    assert!(drawing.edges.iter().all(|edge| !edge.shared_bus));

    graph.edges[0].end_label = None;
    graph.edges.pop();
    let drawing = crate::layout(&graph, &options).unwrap();
    assert!(drawing.edges.iter().all(|edge| !edge.shared_bus));
}

#[test]
fn sparse_scene_can_exceed_the_old_wall_count_cutoff() {
    let (mut graph, mut c, mut drawing) = fixture(false, None);
    for i in 0..44_000 {
        drawing.nodes.push(Rect {
            x: 1_000.0 + f64::from(i) * 60.0,
            y: 1_000.0,
            width: 30.0,
            height: 36.0,
        });
        let id = graph.nodes.len() as u32;
        graph.nodes.push(NodeSpec::new(
            Size {
                width: 30.0,
                height: 36.0,
            },
            id,
        ));
        c.nodes.push(graph.nodes[id as usize]);
    }
    let before = drawing.edges.clone();
    let options = Options {
        routing: RoutingStyle::Orthogonal,
        ..Options::default()
    };
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    work.charge(1_600_000).unwrap();
    assert!(bundle(&mut drawing, &c, &graph, &options, &mut work).unwrap());
    assert_bus_points(&drawing, &before, false);
}

#[test]
fn side_turns_share_the_reach_of_marked_and_unmarked_ports() {
    let (mut graph, c, mut drawing) = fixture(false, None);
    for (i, offset, marker) in [
        (0, -8.0, MarkerMetrics::default()),
        (
            1,
            8.0,
            MarkerMetrics {
                length: 12.0,
                width: 8.0,
            },
        ),
    ] {
        graph.edges[i].start_marker = marker;
        graph.edges[i].source_port = PortSideHint::FixedOffset {
            side: Side::Left,
            offset,
        };
        let edge = &mut drawing.edges[i].1;
        edge.ports.0 = Side::Left;
        edge.points[0] = RoutePoint::new(-42.0, 18.0 + offset, Axis::Horizontal);
    }
    let before = drawing.edges.clone();
    let options = Options {
        routing: RoutingStyle::Orthogonal,
        ..Options::default()
    };
    assert!(apply(&mut drawing, &c, &graph, &options));
    let mut turns = Vec::new();
    for (i, (_, route)) in drawing.edges[..2].iter().enumerate() {
        let contact = route.points[0];
        assert_eq!(contact, before[i].1.points[0]);
        assert_eq!(route.points.last(), before[i].1.points.last());
        let turn = route
            .points
            .iter()
            .find(|point| point.y != contact.y)
            .unwrap();
        assert!(contact.x - turn.x > 12.0);
        turns.push(turn.x);
    }
    assert_eq!(turns[0], turns[1]);
}

#[test]
fn a_marker_on_a_translated_contour_can_seal_the_bus() {
    let (mut graph, mut c, mut drawing) = fixture(
        false,
        Some(Rect {
            x: -62.0,
            y: 20.0,
            width: 3.0,
            height: 3.0,
        }),
    );
    graph.edges[0].source_port = PortSideHint::FixedOffset {
        side: Side::Left,
        offset: 0.0,
    };
    drawing.edges[0].1.ports.0 = Side::Left;
    drawing.edges[0].1.points[0] = RoutePoint::new(-42.0, 18.0, Axis::Horizontal);
    let before = drawing.edges.clone();
    let options = Options {
        routing: RoutingStyle::Orthogonal,
        ..Options::default()
    };
    assert!(apply(&mut drawing, &c, &graph, &options));

    drawing.edges.clone_from(&before);
    let contour = ShapeContourHint::OffsetDiamond { x: -12.0, y: 0.0 };
    c.nodes[0].contour = contour;
    graph.nodes[0].contour = contour;
    assert!(!apply(&mut drawing, &c, &graph, &options));
    assert_eq!(drawing.edges, before);
}

#[test]
fn a_translated_foreign_contour_can_seal_the_hub_shaft() {
    let (mut graph, mut c, mut drawing) = fixture(
        false,
        Some(Rect {
            x: 5.0,
            y: 36.0,
            width: 8.0,
            height: 35.0,
        }),
    );
    let before = drawing.edges.clone();
    let options = Options {
        routing: RoutingStyle::Orthogonal,
        ..Options::default()
    };
    assert!(apply(&mut drawing, &c, &graph, &options));

    drawing.edges.clone_from(&before);
    let contour = ShapeContourHint::OffsetDiamond { x: -10.0, y: 0.0 };
    c.nodes.last_mut().unwrap().contour = contour;
    graph.nodes.last_mut().unwrap().contour = contour;
    assert!(!apply(&mut drawing, &c, &graph, &options));
    assert_eq!(drawing.edges, before);
}

#[test]
fn a_wall_inside_the_rounded_side_turn_keeps_the_old_routes() {
    let (mut graph, c, mut drawing) = fixture(
        false,
        Some(Rect {
            x: -68.0,
            y: 19.0,
            width: 2.0,
            height: 3.0,
        }),
    );
    graph.edges[0].source_port = PortSideHint::FixedOffset {
        side: Side::Left,
        offset: 0.0,
    };
    drawing.edges[0].1.ports.0 = Side::Left;
    drawing.edges[0].1.points[0] = RoutePoint::new(-42.0, 18.0, Axis::Horizontal);
    let before = drawing.edges.clone();
    let options = Options {
        routing: RoutingStyle::Orthogonal,
        ..Options::default()
    };
    assert!(!apply(&mut drawing, &c, &graph, &options));
    assert_eq!(drawing.edges, before);
}

#[test]
fn bundles_both_semantic_directions_onto_one_shaft() {
    let options = Options {
        routing: RoutingStyle::Orthogonal,
        bus_routing: true,
        ..Options::default()
    };
    for incoming in [false, true] {
        let (graph, c, mut drawing) = fixture(incoming, None);
        drawing.edges[0].1.corridor_clearance = 1.0;
        let before = drawing.edges.clone();
        assert!(apply(&mut drawing, &c, &graph, &options));
        let shelf = assert_bus_points(&drawing, &before, incoming);
        let shaft = |geometry: &EdgeGeometry| {
            let contacts = &geometry.points;
            let hub = contacts[if incoming { contacts.len() - 1 } else { 0 }];
            assert!((hub.y - shelf).abs() > 10.0);
            hub.x
        };
        let x = shaft(&drawing.edges[0].1);
        for (_, geometry) in &drawing.edges[1..] {
            assert_eq!(shaft(geometry), x);
        }
    }
}

#[test]
fn slides_past_an_obstacle_and_keeps_the_old_routes_when_sealed() {
    let options = Options {
        routing: RoutingStyle::Orthogonal,
        bus_routing: true,
        ..Options::default()
    };
    let obstruction = Rect {
        x: 24.0,
        y: 71.0,
        width: 16.0,
        height: 18.0,
    };
    let (graph, c, mut drawing) = fixture(true, Some(obstruction));
    let before = drawing.edges.clone();
    assert!(apply(&mut drawing, &c, &graph, &options));
    let shelf = assert_bus_points(&drawing, &before, true);
    assert!(shelf < obstruction.y || shelf > obstruction.y + obstruction.height);

    let (graph, c, mut drawing) = fixture(
        true,
        Some(Rect {
            y: 36.0,
            height: 88.0,
            ..obstruction
        }),
    );
    let before = drawing.edges.clone();
    assert!(!apply(&mut drawing, &c, &graph, &options));
    assert_eq!(drawing.edges, before);
}

#[test]
fn reuses_the_selected_side_and_keeps_distinct_authored_offsets() {
    let (mut graph, c, mut drawing) = fixture(true, None);
    for (i, y) in [(0, 139.0), (1, 145.0)] {
        let edge = &mut drawing.edges[i].1;
        edge.ports.1 = Side::Left;
        *edge.points.last_mut().unwrap() = RoutePoint::new(-42.0, y, Axis::Horizontal);
    }
    graph.edges[0].target_port = PortSideHint::FixedOffset {
        side: Side::Left,
        offset: -3.0,
    };
    graph.edges[2].target_port = PortSideHint::FixedOffset {
        side: Side::Top,
        offset: -4.0,
    };
    graph.edges[5].target_port = PortSideHint::FixedOffset {
        side: Side::Top,
        offset: 20.0,
    };
    let before = drawing.edges.clone();
    let options = Options {
        routing: RoutingStyle::Orthogonal,
        bus_routing: true,
        ..Options::default()
    };
    assert!(apply(&mut drawing, &c, &graph, &options));
    for (index, (_, edge)) in drawing.edges.iter().enumerate() {
        let old = &before[index].1;
        assert_eq!(edge.points.first(), old.points.first());
        assert_eq!(edge.ports, old.ports);
        let target = edge.points.last().unwrap();
        if [0, 2, 5].contains(&index) {
            assert_eq!(Some(target), old.points.last());
        }
        if index <= 1 {
            assert_eq!(target.y, 139.0);
            let turn = edge
                .points
                .iter()
                .rev()
                .find(|point| point.y != target.y)
                .unwrap();
            assert!(target.x - turn.x >= graph.edges[index].end_marker.length);
        }
    }
}

#[test]
fn a_foreign_loop_label_can_seal_the_bus() {
    let (mut graph, mut c, mut drawing) = fixture(true, None);
    let loop_id = graph.edges.len() as u32;
    let mut spec = EdgeSpec::new(NodeId(1).into(), NodeId(1).into());
    spec.end_label = Some(Size {
        width: 16.0,
        height: 88.0,
    });
    graph.edges.push(spec);
    c.loops.push(super::super::Loop {
        edge: loop_id,
        node: 1,
        right: true,
        reach: 22.0,
    });
    drawing.edges.push((
        loop_id,
        EdgeGeometry {
            points: vec![
                RoutePoint::new(-165.0, 10.0, Axis::Horizontal),
                RoutePoint::new(-143.0, 10.0, Axis::Horizontal),
                RoutePoint::new(-143.0, 26.0, Axis::Vertical),
                RoutePoint::new(-165.0, 26.0, Axis::Horizontal),
            ],
            ports: (Side::Right, Side::Right),
            label_center: None,
            start_label_center: None,
            end_label_center: Some(Point { x: 32.0, y: 80.0 }),
            shared_bus: false,
            corridor_clearance: 0.0,
            reversed_for_ranking: false,
        },
    ));
    let before = drawing.edges.clone();
    let options = Options {
        routing: RoutingStyle::Orthogonal,
        bus_routing: true,
        ..Options::default()
    };
    assert!(!apply(&mut drawing, &c, &graph, &options));
    assert_eq!(drawing.edges, before);
}

#[test]
fn declined_optional_work_keeps_every_route() {
    struct Decline;
    impl crate::WorkControl for Decline {
        fn check(&mut self) -> Result<(), crate::WorkError> {
            Ok(())
        }
        fn charge(&mut self, _: usize) -> Result<(), crate::WorkError> {
            Ok(())
        }
        fn try_charge_optional(
            &mut self,
            units: usize,
            _: usize,
        ) -> Result<bool, crate::WorkError> {
            Ok(units == 0)
        }
    }

    let (graph, c, mut drawing) = fixture(true, None);
    let before = drawing.edges.clone();
    let mut control = Decline;
    let mut work = Work::new(&mut control);
    work.charge(10_000).unwrap();
    let options = Options {
        routing: RoutingStyle::Orthogonal,
        bus_routing: true,
        ..Options::default()
    };
    assert!(!bundle(&mut drawing, &c, &graph, &options, &mut work).unwrap());
    assert_eq!(drawing.edges, before);
}

#[test]
fn separate_fixed_shafts_must_clear_each_others_marker_ink() {
    let (mut graph, c, mut drawing) = fixture(true, None);
    for (i, offset) in [(0, -17.0), (5, 17.0)] {
        graph.edges[i].target_port = PortSideHint::FixedOffset {
            side: Side::Top,
            offset,
        };
        let points = &mut drawing.edges[i].1.points;
        points[2].x = offset;
        points[3].x = offset;
    }
    let before = drawing.edges.clone();
    let options = Options {
        routing: RoutingStyle::Orthogonal,
        bus_routing: true,
        ..Options::default()
    };
    for edge in &mut graph.edges {
        edge.end_marker.width = 40.0;
    }
    assert!(!apply(&mut drawing, &c, &graph, &options));
    assert_eq!(drawing.edges, before);
    for edge in &mut graph.edges {
        edge.end_marker.width = 20.0;
    }
    assert!(apply(&mut drawing, &c, &graph, &options));
    assert_eq!(drawing.edges[0].1.points.last(), before[0].1.points.last());
    assert_eq!(drawing.edges[5].1.points.last(), before[5].1.points.last());
}

#[test]
fn coincident_terminal_rows_keep_the_previous_geometry() {
    let (graph, c, mut drawing) = fixture(true, None);
    drawing.nodes[0].y = 36.0;
    for (_, edge) in &mut drawing.edges {
        edge.points.truncate(2);
        edge.points[1].y = 36.0;
    }
    let before = drawing.edges.clone();
    let options = Options {
        routing: RoutingStyle::Orthogonal,
        bus_routing: true,
        ..Options::default()
    };
    assert!(!apply(&mut drawing, &c, &graph, &options));
    assert_eq!(drawing.edges, before);
}

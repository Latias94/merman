use super::*;
use crate::compound::hierarchy;
use crate::{
    BorderStitch, Direction, EdgeGeometry, EdgeId, EdgeSpec, GroupGeometry, GroupId, GroupSpec,
    NodeId, NodeSpec, NoopWorkControl, Options, Side, Size, TruncatedRoute, WorkControl,
};

fn geometry(points: Vec<RoutePoint>) -> EdgeGeometry {
    EdgeGeometry {
        points,
        ports: (Side::Bottom, Side::Top),
        label_center: None,
        start_label_center: None,
        end_label_center: None,
        reversed_for_ranking: false,
    }
}

fn route(points: &[(f64, f64)], axis: Axis) -> EdgeGeometry {
    geometry(
        points
            .iter()
            .map(|&(x, y)| RoutePoint::new(x, y, axis))
            .collect(),
    )
}

fn drawing(edges: Vec<EdgeGeometry>) -> (Graph, LayoutOutput) {
    let mut graph = Graph::default();
    for index in 0..edges.len() as u32 {
        let source = NodeId(2 * index);
        let target = NodeId(2 * index + 1);
        graph.nodes.push(NodeSpec::new(Size::default(), source.0));
        graph.nodes.push(NodeSpec::new(Size::default(), target.0));
        graph
            .edges
            .push(EdgeSpec::new(source.into(), target.into()));
    }
    let output = LayoutOutput {
        nodes: vec![Rect::default(); graph.nodes.len()],
        edges,
        ..Default::default()
    };
    (graph, output)
}

fn count(graph: &Graph, out: &LayoutOutput, style: RoutingStyle) -> u64 {
    crossings(graph, out, style, &mut Work::new(&mut NoopWorkControl)).unwrap()
}

fn probe_at_cubic_quarter(controls: [(f64, f64); 4]) -> [(f64, f64); 2] {
    let (mut x, mut y) = (0., 0.);
    for ((cx, cy), weight) in controls.into_iter().zip([27., 27., 9., 1.]) {
        x += weight * cx / 64.;
        y += weight * cy / 64.;
    }
    [(x - 1., y), (x + 1., y)]
}

#[test]
fn offset_legs_follow_the_arrival_axis_and_distinguish_all_three_styles() {
    let styles = [
        RoutingStyle::Curved,
        RoutingStyle::Orthogonal,
        RoutingStyle::Polyline,
    ];
    for axis in [Axis::Vertical, Axis::Horizontal] {
        let oriented = |points: &[(f64, f64)]| {
            let mut edge = route(points, axis);
            if axis == Axis::Horizontal {
                for knot in &mut edge.points {
                    std::mem::swap(&mut knot.x, &mut knot.y);
                }
            }
            edge
        };
        let handle = 100. * sweep_handle_fraction(100., 100.);
        let quarter =
            probe_at_cubic_quarter([(0., 0.), (0., handle), (100., 100. - handle), (100., 100.)]);
        for (probe, expected) in [
            (quarter, [1, 0, 0]),
            ([(98., 75.), (102., 75.)], [0, 1, 0]),
            ([(18., 20.), (22., 20.)], [0, 0, 1]),
            ([(12., 20.), (16., 20.)], [0, 0, 0]),
        ] {
            let mut sweep = oriented(&[(0., 0.), (100., 100.)]);
            sweep.points[0].axis = if axis == Axis::Vertical {
                Axis::Horizontal
            } else {
                Axis::Vertical
            };
            let (graph, out) = drawing(vec![sweep, oriented(&probe)]);
            assert_eq!(styles.map(|style| count(&graph, &out, style)), expected);
        }
    }
}

#[test]
fn curved_corners_share_their_leg_and_ignore_redundant_straight_knots() {
    let corner = route(
        &[(0., 0.), (0., 30.), (0., 30.), (0., 100.), (100., 100.)],
        Axis::Vertical,
    );
    let quarter = probe_at_cubic_quarter([(0., 0.), (0., 62.), (38., 100.), (100., 100.)]);
    let (graph, out) = drawing(vec![corner, route(&quarter, Axis::Horizontal)]);
    assert_eq!(count(&graph, &out, RoutingStyle::Curved), 1);
    assert_eq!(count(&graph, &out, RoutingStyle::Orthogonal), 0);
    assert_eq!(count(&graph, &out, RoutingStyle::Polyline), 0);
    let mut miss = out;
    miss.edges[1] = route(&[(18., 55.), (22., 55.)], Axis::Horizontal);
    assert_eq!(count(&graph, &miss, RoutingStyle::Curved), 0);

    let (_, out) = drawing(vec![route(
        &[(0., 0.), (0., 100.), (100., 100.), (100., 200.)],
        Axis::Vertical,
    )]);
    let pieces = flatten(
        &out,
        RoutingStyle::Curved,
        &mut Work::new(&mut NoopWorkControl),
    )
    .unwrap();
    assert!(pieces.len() <= 2 * MAX_CURVE_CHORDS);
    assert!(pieces.windows(2).all(|pair| pair[0].to == pair[1].from));
    assert!(
        pieces
            .iter()
            .any(|piece| piece.to == Point { x: 50., y: 100. })
    );
}

#[test]
fn a_curved_route_keeps_the_painted_group_border_stitch() {
    let (mut graph, mut out) = drawing(vec![
        route(&[(0., 0.), (0., 100.), (100., 100.)], Axis::Vertical),
        route(&[(-1., 80.), (1., 80.)], Axis::Horizontal),
    ]);
    graph.groups.push(GroupSpec::new(0));
    out.groups.push(GroupGeometry {
        parent: None,
        rect: Rect {
            x: -50.,
            y: 0.,
            width: 100.,
            height: 100.,
        },
        title_center: None,
        effective_direction: Direction::Down,
    });
    assert_eq!(count(&graph, &out, RoutingStyle::Curved), 0);
    out.border_stitches.push(BorderStitch {
        edge: EdgeId(0),
        point: 1,
        group: GroupId(0),
        side: Side::Bottom,
    });
    assert_eq!(count(&graph, &out, RoutingStyle::Curved), 1);
    let quarter = probe_at_cubic_quarter([(0., 0.), (0., 62.), (38., 100.), (100., 100.)]);
    out.edges[1] = route(&quarter, Axis::Horizontal);
    assert_eq!(count(&graph, &out, RoutingStyle::Curved), 0);
}

#[test]
fn counts_distinct_pairs_and_transverse_knot_crossings_but_excludes_touches() {
    let line = || route(&[(0., 0.), (100., 0.)], Axis::Horizontal);
    for (other, expected) in [
        (vec![(10., -20.), (30., 10.), (50., -20.), (70., 10.)], 1),
        (vec![(25., -10.), (50., 0.), (70., 30.)], 1),
        (vec![(25., 10.), (50., 0.), (75., 15.)], 0),
        (vec![(25., 10.), (50., 0.)], 0),
        (vec![(0., 0.), (100., 0.)], 0),
        (vec![(25., 10.), (40., 0.), (60., 0.), (80., -10.)], 0),
    ] {
        let (graph, out) = drawing(vec![line(), route(&other, Axis::Vertical)]);
        assert_eq!(
            count(&graph, &out, RoutingStyle::Polyline),
            expected,
            "{other:?}"
        );
    }
    for (other, expected) in [
        ([(30., 0.), (40., 40.), (60., 80.)], 1),
        ([(30., 80.), (40., 40.), (50., 80.)], 0),
    ] {
        let (graph, out) = drawing(vec![
            route(&[(0., 40.), (40., 40.), (80., 65.)], Axis::Vertical),
            route(&other, Axis::Vertical),
        ]);
        assert_eq!(count(&graph, &out, RoutingStyle::Polyline), expected);
    }
}

#[test]
fn fan_uses_the_drawn_node_or_group_box_and_allows_distant_incident_crossings() {
    let rect = Rect {
        x: 100.,
        y: 50.,
        width: 80.,
        height: 40.,
    };
    let group = GroupGeometry {
        parent: None,
        rect,
        title_center: None,
        effective_direction: Direction::Down,
    };
    for gap in [23.75, 24., 25.] {
        for source_first in [false, true] {
            for source_second in [false, true] {
                let (mut graph, mut out) = drawing(vec![
                    route(&[(128., 90.), (128., 180.)], Axis::Vertical),
                    route(&[(122., 90.), (134., 90. + 2. * gap)], Axis::Vertical),
                ]);
                out.nodes[0] = rect;
                graph.groups.push(GroupSpec::new(0));
                out.groups.push(group.clone());
                assert_eq!(count(&graph, &out, RoutingStyle::Polyline), 1);
                for (edge, at_source) in [source_first, source_second].into_iter().enumerate() {
                    if at_source {
                        graph.edges[edge].source = NodeId(0).into();
                    } else {
                        graph.edges[edge].target = NodeId(0).into();
                        out.edges[edge].points.reverse();
                    }
                }
                let expected = u64::from(gap >= FAN);
                assert_eq!(count(&graph, &out, RoutingStyle::Polyline), expected);
                out.truncated_routes = graph
                    .edges
                    .iter()
                    .enumerate()
                    .map(|(index, edge)| {
                        let drawn = |end| {
                            if end == Endpoint::Node(NodeId(0)) {
                                GroupId(0).into()
                            } else {
                                end
                            }
                        };
                        TruncatedRoute {
                            edge: EdgeId(index as u32),
                            source: drawn(edge.source),
                            target: drawn(edge.target),
                        }
                    })
                    .collect();
                graph.edges[0] = EdgeSpec::new(NodeId(0).into(), NodeId(1).into());
                graph.edges[1] = EdgeSpec::new(NodeId(2).into(), NodeId(3).into());
                assert_eq!(count(&graph, &out, RoutingStyle::Polyline), expected);
            }
        }
    }
}

#[test]
fn y_bands_count_different_edge_occurrences_and_bound_long_span_replication() {
    let mut routes = Vec::new();
    for index in 1..=40 {
        let offset = f64::from(index);
        routes.push(route(&[(-1., offset), (41., offset)], Axis::Horizontal));
        routes.push(route(&[(offset, -1.), (offset, 41.)], Axis::Vertical));
    }
    let (mut graph, out) = drawing(routes);
    graph.edges[0].target = graph.edges[0].source;
    assert_eq!(count(&graph, &out, RoutingStyle::Polyline), 40 * 40);
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let pieces = flatten(&out, RoutingStyle::Polyline, &mut work).unwrap();
    let bands = Bands::build(&pieces, &mut work).unwrap().unwrap();
    assert!(bands.bins.len() > 1);
    assert!(bands.bins.iter().map(Vec::len).sum::<usize>() <= 3 * pieces.len());
}

#[test]
fn band_queries_agree_with_exhaustive_integer_orientation() {
    let mut state = 41_u32;
    let mut coordinate = || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        i64::from((state >> 8) % 97)
    };
    let segments: Vec<_> = (0..120)
        .map(|_| [(coordinate(), coordinate()), (coordinate(), coordinate())])
        .collect();
    let opposite = |line: &[(i64, i64); 2], ends: &[(i64, i64); 2]| {
        let [(ax, ay), (bx, by)] = *line;
        let sides = ends.map(|(x, y)| ((bx - ax) * (y - ay) - (by - ay) * (x - ax)).signum());
        sides[0] * sides[1] < 0
    };
    let mut expected = 0;
    for (index, first) in segments.iter().enumerate() {
        for second in &segments[..index] {
            expected += u64::from(opposite(first, second) && opposite(second, first));
        }
    }
    let (graph, out) = drawing(
        segments
            .into_iter()
            .map(|line| route(&line.map(|(x, y)| (x as f64, y as f64)), Axis::Vertical))
            .collect(),
    );
    assert_eq!(count(&graph, &out, RoutingStyle::Polyline), expected);
}

struct Budget {
    remaining: usize,
}

impl WorkControl for Budget {
    fn check(&mut self) -> Result<(), WorkError> {
        Ok(())
    }
    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.remaining = self
            .remaining
            .checked_sub(units)
            .ok_or(WorkError::Interrupted)?;
        Ok(())
    }
}

#[test]
fn disjoint_intervals_have_a_subquadratic_budget_and_dense_queries_can_be_interrupted() {
    let count = 512;
    let (graph, mut out) = drawing(
        (0..count)
            .map(|index| {
                let left = 2. * index as f64;
                route(&[(left, 0.), (left + 1., 100.)], Axis::Vertical)
            })
            .collect(),
    );
    let evaluate = |out: &LayoutOutput, remaining| {
        let mut budget = Budget { remaining };
        let mut work = Work::new(&mut budget);
        let count = crossings(&graph, out, RoutingStyle::Polyline, &mut work)?;
        work.finish()?;
        Ok(count)
    };
    assert_eq!(evaluate(&out, count * 64), Ok(0));
    for (index, edge) in out.edges.iter_mut().enumerate() {
        edge.points[1].x = (count - index) as f64 * 2.;
    }
    assert_eq!(
        evaluate(&out, count * 64),
        Err(LayoutError::Work(WorkError::Interrupted))
    );
    assert_eq!(
        evaluate(&out, usize::MAX),
        Ok((count * (count - 1) / 2) as u64)
    );
}

#[test]
fn proper_intersections_preserve_tiny_and_large_scales_and_reject_nonfinite_routes() {
    for scale in [1e-290, 1., 1e290] {
        let (graph, mut out) = drawing(vec![
            route(&[(-scale, -scale), (scale, scale)], Axis::Vertical),
            route(&[(-scale, scale), (scale, -scale)], Axis::Vertical),
        ]);
        assert_eq!(count(&graph, &out, RoutingStyle::Polyline), 1);
        out.edges[1].points[1].y = f64::NAN;
        let result = crossings(
            &graph,
            &out,
            RoutingStyle::Curved,
            &mut Work::new(&mut NoopWorkControl),
        );
        assert_eq!(
            result,
            Err(LayoutError::Work(WorkError::ArithmeticOverflow))
        );
    }
    for (long, short) in [
        ([(0., 0.), (1e290, 1e290)], [(0., 1e-290), (1e-290, 0.)]),
        (
            [(-1e290, -1e290), (1e290, 1e290)],
            [(0., 1e-290), (1e-290, 0.)],
        ),
        (
            [(0., 0.), (1e290, 1e-290)],
            [(6e289, 5e-291), (6e289, 7e-291)],
        ),
    ] {
        for order in [[long, short], [short, long]] {
            let (graph, out) = drawing(
                order
                    .iter()
                    .map(|line| route(line, Axis::Vertical))
                    .collect(),
            );
            assert_eq!(count(&graph, &out, RoutingStyle::Polyline), 1);
        }
    }
}

fn suggested(
    direction: Direction,
    origin: (f64, f64),
    far_ports: &[(f64, Option<f64>)],
    own_endpoint: bool,
    hints: &[Option<f64>],
) -> Option<Vec<Option<f64>>> {
    let mut graph = Graph {
        direction,
        groups: vec![GroupSpec::new(0), GroupSpec::new(1)],
        ..Default::default()
    };
    for (index, parent) in [Some(1), Some(0), Some(0), None].into_iter().enumerate() {
        let mut node = NodeSpec::new(Size::default(), index as u32);
        node.parent = parent.map(GroupId);
        graph.nodes.push(node);
    }
    let transit = [(1, 3), (3, 2)].into_iter().cycle().take(far_ports.len());
    for (source, target) in [(0, 3), (1, 3)].into_iter().chain(transit) {
        graph
            .edges
            .push(EdgeSpec::new(NodeId(source).into(), NodeId(target).into()));
    }
    if own_endpoint {
        graph
            .edges
            .push(EdgeSpec::new(NodeId(1).into(), GroupId(0).into()));
    }
    let options = Options::default();
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let hierarchy = hierarchy::build(&graph, &options, &mut work).unwrap();
    let (near, far) = flow_sides(direction);
    let mut sides = vec![far; hierarchy.borders.len()];
    let rect = Rect {
        x: origin.0,
        y: origin.1,
        width: 100.,
        height: 100.,
    };
    let mut output = LayoutOutput {
        groups: vec![
            GroupGeometry {
                rect,
                parent: None,
                effective_direction: direction,
                title_center: None
            };
            2
        ],
        edges: vec![geometry(vec![]); graph.edges.len()],
        ..Default::default()
    };
    for (id, border) in hierarchy.borders.iter().enumerate() {
        let (cross, turn) = match border.edge {
            0 => (10., None),
            1 => {
                sides[id] = near;
                (30., Some(-100.))
            }
            edge => far_ports.get(edge - 2).copied().unwrap_or((90., None)),
        };
        let side = sides[id];
        let at = |cross: f64, depth: f64| {
            let (x, y) = match side {
                Side::Top => (cross, -depth),
                Side::Bottom => (cross, 100. + depth),
                Side::Left => (-depth, cross),
                Side::Right => (100. + depth, cross),
            };
            RoutePoint::new(x + rect.x, y + rect.y, side.axis())
        };
        let port = at(cross, 0.);
        let mut inner = vec![at(75., -25.), port, at(65., -10.), port];
        let mut outer = vec![port, at(cross, 10.), at(cross + 0.25, 15.)];
        if let Some(turn) = turn {
            outer.extend([at(turn, 20.), port, at(190., 50.)]);
        }
        if !border.outgoing {
            inner.reverse();
            outer.reverse();
        }
        let stitched = &mut output.edges[border.edge].points;
        for &(level, _) in &hierarchy.paths[border.edge] {
            let part = if level == border.level {
                &inner
            } else {
                &outer
            };
            let skip = usize::from(!stitched.is_empty());
            if skip != 0 {
                output.border_stitches.push(BorderStitch {
                    edge: EdgeId(border.edge as u32),
                    point: stitched.len() - 1,
                    group: GroupId(border.level as u32),
                    side,
                });
            }
            stitched.extend_from_slice(&part[skip..]);
        }
    }
    output.border_stitches.reverse();
    let ctx = Pass {
        graph: &graph,
        hierarchy: &hierarchy,
        options: &options,
        sides: &sides,
        items: &[],
        ports: hints,
    };
    port_hints(&ctx, &output, &mut work).unwrap()
}

#[test]
fn inverted_far_ports_read_the_first_outer_turn_in_either_semantic_direction() {
    let previous = [Some(91.), Some(92.), None, None];
    let adjusted = [Some(91.), Some(92.), Some(80.), Some(10.)];
    let ports = [(20., Some(80.)), (60., Some(10.))];
    for dir in [
        Direction::Down,
        Direction::Up,
        Direction::Left,
        Direction::Right,
    ] {
        assert_eq!(
            suggested(dir, (0., 0.), &ports, false, &previous),
            Some(adjusted.to_vec())
        );
        assert_eq!(suggested(dir, (0., 0.), &ports, false, &adjusted), None);
    }
}

#[test]
fn a_retry_orders_straight_ports_between_inverted_turns_in_drawing_coordinates() {
    let previous = [Some(91.), Some(92.), Some(180.), Some(310.), Some(250.)];
    let ports = [(20., Some(80.)), (60., Some(10.)), (40., None)];
    for dir in [
        Direction::Down,
        Direction::Up,
        Direction::Left,
        Direction::Right,
    ] {
        for origin in [(0., 0.), (120., 250.)] {
            let cross = if dir.axis() == Axis::Vertical {
                origin.0
            } else {
                origin.1
            };
            let adjusted = [
                previous[0],
                previous[1],
                Some(cross + 80.),
                Some(cross + 10.),
                Some(cross + 40.),
            ];
            assert_eq!(
                suggested(dir, origin, &ports, false, &previous),
                Some(adjusted.to_vec()),
                "{dir:?} at {origin:?}"
            );
        }
    }
}

#[test]
fn a_straight_port_can_invert_with_a_turning_port() {
    assert_eq!(
        suggested(
            Direction::Down,
            (0., 0.),
            &[(20., None), (60., Some(10.))],
            false,
            &[None; 4],
        ),
        Some(vec![None, None, Some(20.), Some(10.)])
    );
}

#[test]
fn a_group_endpoint_without_an_outer_leg_blocks_a_partial_face_update() {
    let previous = [Some(91.), Some(92.), Some(180.), Some(310.), Some(350.)];
    assert_eq!(
        suggested(
            Direction::Down,
            (120., 250.),
            &[(20., Some(80.)), (60., Some(10.))],
            true,
            &previous,
        ),
        None
    );
}

#[test]
fn tied_ports_tied_turns_straight_exits_and_ordered_faces_do_not_trigger_a_retry() {
    for ports in [
        [(20., Some(80.)), (20., Some(10.))],
        [(20., Some(50.)), (60., Some(50.))],
        [(20., None), (60., None)],
        [(20., None), (60., Some(80.))],
        [(20., Some(10.)), (60., Some(80.))],
    ] {
        assert_eq!(
            suggested(Direction::Down, (0., 0.), &ports, false, &[None; 4]),
            None
        );
    }
}

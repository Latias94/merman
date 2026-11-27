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
        shared_bus: false,
        corridor_clearance: 0.0,
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

fn curved_chords(out: &LayoutOutput) -> Vec<[Point; 2]> {
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut centerlines = trace(out, RoutingStyle::Curved, &mut work).unwrap();
    let mut chords = Vec::new();
    for index in 0..centerlines.primitives.len() {
        centerlines.expand(index, &mut work).unwrap();
        chords.extend(centerlines.points(index).windows(2).map(|p| [p[0], p[1]]));
    }
    chords
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
    let arm = MAX_CORNER_ARM;
    let quarter = probe_at_cubic_quarter([
        (0., 100. - arm),
        (0., 100. - arm + TURN_HANDLE * arm),
        (arm - TURN_HANDLE * arm, 100.),
        (arm, 100.),
    ]);
    let (graph, out) = drawing(vec![corner, route(&quarter, Axis::Horizontal)]);
    assert_eq!(count(&graph, &out, RoutingStyle::Curved), 1);
    assert_eq!(count(&graph, &out, RoutingStyle::Orthogonal), 0);
    assert_eq!(count(&graph, &out, RoutingStyle::Polyline), 0);
    let mut miss = out;
    miss.edges[1] = route(&[(18., 55.), (22., 55.)], Axis::Horizontal);
    assert_eq!(count(&graph, &miss, RoutingStyle::Curved), 0);
    // A long straight corridor meeting a short jog keeps its straight trunk bounded.
    let (_, banana) = drawing(vec![route(
        &[(0., 0.), (0., 360.), (4., 360.), (4., 374.)],
        Axis::Vertical,
    )]);
    let banana_pieces = curved_chords(&banana);
    assert!(
        banana_pieces
            .iter()
            .any(|[from, to]| from.x == 0. && to.x == 0. && to.y - from.y >= 350.),
        "long straight leg must not bow into a banana arc"
    );

    let (_, out) = drawing(vec![route(
        &[(0., 0.), (0., 100.), (100., 100.), (100., 200.)],
        Axis::Vertical,
    )]);
    let pieces = curved_chords(&out);
    assert!(pieces.len() <= 2 * MAX_CURVE_CHORDS + 3);
    assert!(pieces.windows(2).all(|pair| pair[0][1] == pair[1][0]));
    assert!(
        pieces
            .iter()
            .any(|[from, to]| from.y == 100. && to.y == 100. && from.x < to.x)
    );
}

#[test]
fn a_curved_route_keeps_the_painted_group_border_stitch() {
    let (mut graph, mut out) = drawing(vec![
        route(&[(0., 0.), (0., 100.), (100., 100.)], Axis::Vertical),
        route(&[(-1., 92.), (1., 92.)], Axis::Horizontal),
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
        title_struck: false,
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
    let arm = MAX_CORNER_ARM;
    let quarter = probe_at_cubic_quarter([
        (0., 100. - arm),
        (0., 100. - arm + TURN_HANDLE * arm),
        (arm - TURN_HANDLE * arm, 100.),
        (arm, 100.),
    ]);
    out.edges[1] = route(&quarter, Axis::Horizontal);
    assert_eq!(count(&graph, &out, RoutingStyle::Curved), 0);
}

#[test]
fn curved_crossings_follow_adjacent_chords_at_a_sample_or_stitch() {
    let probe = route(&[(2048., 2040.), (2048., 2056.)], Axis::Vertical);
    let (mut graph, mut out) = drawing(vec![
        route(&[(0., 0.), (4096., 4096.)], Axis::Vertical),
        probe.clone(),
    ]);
    // The middle of this symmetric sweep is a sampled vertex.
    assert_eq!(count(&graph, &out, RoutingStyle::Curved), 1);
    graph.edges[1].source = graph.edges[0].source;
    out.nodes[0].x = 2048.;
    out.nodes[0].y = 2048.;
    assert_eq!(count(&graph, &out, RoutingStyle::Curved), 0);

    out.nodes[0] = Rect::default();
    graph.groups.push(GroupSpec::new(0));
    out.groups.push(GroupGeometry {
        parent: None,
        rect: Rect::default(),
        title_center: None,
        title_struck: false,
        effective_direction: Direction::Down,
    });
    out.border_stitches.push(BorderStitch {
        edge: EdgeId(0),
        point: 1,
        group: GroupId(0),
        side: Side::Bottom,
    });
    for (end, expected) in [((4096., 4096.), 1), ((0., 4096.), 0)] {
        let path = [(0., 0.), (2048., 2048.), end];
        out.edges[0] = route(&path, Axis::Vertical);
        assert_eq!(count(&graph, &out, RoutingStyle::Curved), expected);
        out.edges[0].points.reverse();
        assert_eq!(count(&graph, &out, RoutingStyle::Curved), expected);
    }
}

#[test]
fn common_sweep_controls_with_different_chord_counts_can_cross() {
    let threshold = 1600. / 3.;
    for axis in [Axis::Vertical, Axis::Horizontal] {
        let edges = [threshold - 0.01, threshold + 0.01].map(|end| {
            let end = match axis {
                Axis::Vertical => (end, 40.),
                Axis::Horizontal => (40., end),
            };
            route(&[(0., 0.), end], axis)
        });
        for order in [[0, 1], [1, 0]] {
            let (graph, out) = drawing(order.map(|i| edges[i].clone()).into());
            // The continuous sweeps are ordered, but their 40- and 41-chord interpolants cross.
            assert_eq!(count(&graph, &out, RoutingStyle::Curved), 1);
        }
    }
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
        title_struck: false,
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
    assert_eq!(
        count_crossings(&graph, &out, RoutingStyle::Polyline, 2, &mut work),
        Ok(2)
    );
    let centerlines = trace(&out, RoutingStyle::Polyline, &mut work).unwrap();
    let bands = Bands::build(&centerlines.primitives, &mut work)
        .unwrap()
        .unwrap();
    assert!(bands.bins.len() > 1);
    assert!(bands.bins.iter().map(Vec::len).sum::<usize>() <= 3 * centerlines.primitives.len());
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

fn count_with_budget(
    graph: &Graph,
    out: &LayoutOutput,
    style: RoutingStyle,
    remaining: usize,
) -> LayoutResult<u64> {
    let mut budget = Budget { remaining };
    let mut work = Work::new(&mut budget);
    let count = crossings(graph, out, style, &mut work)?;
    work.finish()?;
    Ok(count)
}

#[test]
fn disjoint_curve_envelopes_do_not_need_chords() {
    let edges = 256;
    let (graph, out) = drawing(
        (0..edges)
            .map(|index| {
                let x = 8192. * index as f64;
                route(&[(x, 0.), (x + 4096., 4096.)], Axis::Vertical)
            })
            .collect(),
    );
    assert_eq!(
        count_with_budget(&graph, &out, RoutingStyle::Curved, edges * 64),
        Ok(0)
    );
}

#[test]
fn overlapping_curve_envelopes_fit_a_linear_chord_budget_per_pair() {
    let edges = 32;
    let (graph, out) = drawing(
        (0..edges)
            .map(|index| {
                let x = 32. * index as f64;
                let mut edge = route(&[(x, 0.), (x + 4096., 4096.)], Axis::Vertical);
                if index % 2 == 1 {
                    edge.points.reverse();
                }
                edge
            })
            .collect(),
    );
    let pairs = edges * (edges - 1) / 2;
    assert_eq!(
        count_with_budget(
            &graph,
            &out,
            RoutingStyle::Curved,
            6 * MAX_CURVE_CHORDS * pairs,
        ),
        Ok(0)
    );
    assert_eq!(
        count_with_budget(&graph, &out, RoutingStyle::Curved, edges * 64),
        Err(LayoutError::Work(WorkError::Interrupted))
    );
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
        count_with_budget(&graph, out, RoutingStyle::Polyline, remaining)
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
                title_center: None,
                title_struck: false,
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

#[test]
fn monotone_sweep_matches_bands_across_star_fans_tricky_crossings_and_limits() {
    let assert_equivalent = |graph: &Graph, out: &LayoutOutput, style: RoutingStyle, limit: u64| {
        let sweep = count_crossings_impl(
            graph,
            out,
            style,
            limit,
            true,
            &mut Work::new(&mut NoopWorkControl),
        )
        .unwrap();
        let bands = count_crossings_impl(
            graph,
            out,
            style,
            limit,
            false,
            &mut Work::new(&mut NoopWorkControl),
        )
        .unwrap();
        assert_eq!(
            sweep, bands,
            "sweep ({sweep}) != bands ({bands}) for {style:?} limit={limit}"
        );
        sweep
    };

    // 1. 64-edge curved star fan with alternating directions and mixed crossings.
    let mut routes = Vec::new();
    for i in 0..64 {
        let x0 = if i < 40 {
            500.0
        } else {
            200.0 + (i as f64) * 12.0
        };
        let x1 = ((i * 29) % 64) as f64 * 18.0;
        let mut r = route(&[(x0, 0.0), (x1, 400.0)], Axis::Vertical);
        if i % 3 == 0 {
            r.points.reverse();
        }
        routes.push(r);
    }
    // Add horizontal non-monotone edges (`partition.other`) cutting across the fan.
    for h in 0..6 {
        let y = 60.0 + h as f64 * 55.0;
        routes.push(route(&[(0.0, y), (1150.0, y)], Axis::Horizontal));
    }
    let (graph, out) = drawing(routes);
    let total = assert_equivalent(&graph, &out, RoutingStyle::Curved, u64::MAX);
    assert!(total > 20, "expected substantial crossings, got {total}");
    assert_equivalent(&graph, &out, RoutingStyle::Curved, 3);
    assert_equivalent(&graph, &out, RoutingStyle::Curved, 15);
    assert_equivalent(&graph, &out, RoutingStyle::Polyline, u64::MAX);

    // 2. Triple intersections, coincident vertices, and double-crossing lens (>= 32 primitives).
    let mut tricky = vec![
        // Three edges meeting at (200, 200) outside any shared endpoint fan.
        route(&[(100.0, 0.0), (300.0, 400.0)], Axis::Vertical),
        route(&[(200.0, 0.0), (200.0, 400.0)], Axis::Vertical),
        route(&[(300.0, 0.0), (100.0, 400.0)], Axis::Vertical),
        // Double-crossing lens: short early-bend cubic vs long late-bend cubic.
        route(
            &[(350.0, 0.0), (430.0, 120.0), (350.0, 400.0)],
            Axis::Vertical,
        ),
        route(&[(390.0, 0.0), (390.0, 400.0)], Axis::Vertical),
    ];
    // Pad past MIN_SWEEP_PRIMITIVES with overlapping curves so the sweep path activates.
    for i in 0..34 {
        let x0 = 50.0 + (i as f64) * 11.0;
        let x1 = 420.0 - ((i * 7) % 34) as f64 * 10.0;
        tricky.push(route(&[(x0, 10.0), (x1, 390.0)], Axis::Vertical));
    }
    let (graph2, out2) = drawing(tricky);
    let tricky_total = assert_equivalent(&graph2, &out2, RoutingStyle::Curved, u64::MAX);
    assert!(tricky_total > 10);
    assert_equivalent(&graph2, &out2, RoutingStyle::Polyline, u64::MAX);

    // 3. Horizontal segments touching exact chain starts, ends, interior cubic vertices,
    // and triple intersections must match `Bands` bit-for-bit via `EV_HORIZ`.
    let mut horiz_case = Vec::new();
    for i in 0..40 {
        let x0 = 40.0 + (i as f64) * 14.0;
        let x1 = 600.0 - ((i * 13) % 40) as f64 * 13.0;
        let mut r = route(&[(x0, 0.0), (x1, 400.0)], Axis::Vertical);
        if i % 2 == 1 {
            r.points.reverse();
        }
        horiz_case.push(r);
    }
    // Horizontal lines at exact y_top (0.0), y_bot (400.0), exact midpoint (200.0), and
    // multiple interior heights including overlapping horizontal-vs-horizontal segments.
    for (y, x0, x1) in [
        (0.0, 20.0, 620.0),
        (0.0, 100.0, 500.0),
        (200.0, 20.0, 620.0),
        (200.0, 200.0, 420.0),
        (400.0, 20.0, 620.0),
        (125.0, 150.0, 480.0),
        (275.0, 80.0, 540.0),
    ] {
        horiz_case.push(route(&[(x0, y), (x1, y)], Axis::Horizontal));
    }
    let (graph3, out3) = drawing(horiz_case);
    let h_total = assert_equivalent(&graph3, &out3, RoutingStyle::Curved, u64::MAX);
    assert!(h_total > 30);
    assert_equivalent(&graph3, &out3, RoutingStyle::Curved, 7);
    assert_equivalent(&graph3, &out3, RoutingStyle::Polyline, u64::MAX);
}

#[test]
fn monotone_sweep_handles_cancelling_coords_large_horiz_and_non_x_monotone_cubics() {
    let assert_equivalent = |graph: &Graph, out: &LayoutOutput, style: RoutingStyle| -> u64 {
        let mut noop = NoopWorkControl;
        let mut work = Work::new(&mut noop);
        let sweep_count = count_crossings_impl(graph, out, style, u64::MAX, true, &mut work)
            .expect("sweep count");
        let band_count = count_crossings_impl(graph, out, style, u64::MAX, false, &mut work)
            .expect("bands count");
        assert_eq!(sweep_count, band_count, "sweep vs bands mismatch");
        sweep_count
    };

    // 1. Cancelling opposite-sign large coordinates crossing near 0:
    // `ord_a = [(-1e8, -49e8), (1e8, 49e8)]` crosses `ord_b = [(1e-8, 0.0), (-1e-8, 1e-7)]`.
    // Include extra overlapping vertical chains to trigger the monotone sweep (>= 32 primitives).
    let mut cancelling = vec![
        route(&[(-1e8, -49e8), (1e8, 49e8)], Axis::Vertical),
        route(&[(1e-8, 0.0), (-1e-8, 1e-7)], Axis::Vertical),
    ];
    for i in 0..34 {
        let x = 200.0 + (i as f64) * 5.0;
        cancelling.push(route(&[(x, -100.0), (x + 10.0, 100.0)], Axis::Vertical));
    }
    let (g1, o1) = drawing(cancelling);
    let c1 = assert_equivalent(&g1, &o1, RoutingStyle::Polyline);
    assert!(c1 >= 1, "cancelling-coordinate crossing must be detected");

    // 2. Large-coordinate horizontal segment (`>= 1e9`) where unscaled `1e-9` vanishes in f64:
    // the L-shaped route turns at `(base, 50.0)` and the sloped chain crosses cleanly through
    // the vertex from top-right to bottom-left right at `h.x_min`.
    let base = 1.0e9_f64;
    let mut large_horiz = vec![
        route(&[(base + 20.0, 0.0), (base - 20.0, 100.0)], Axis::Vertical),
        route(
            &[(base, 10.0), (base, 50.0), (base + 500.0, 50.0)],
            Axis::Horizontal,
        ),
    ];
    for i in 0..34 {
        let x = base + 50.0 + (i as f64) * 10.0;
        large_horiz.push(route(&[(x, 0.0), (x + 5.0, 100.0)], Axis::Vertical));
    }
    let (g2, o2) = drawing(large_horiz);
    let c2 = assert_equivalent(&g2, &o2, RoutingStyle::Polyline);
    assert!(
        c2 >= 35,
        "large-coordinate EV_HORIZ crossings must match Bands (got {c2})"
    );

    // 3. Strictly `y`-monotone but non-`x`-monotone curved corner (`(0,0) -> (100,100) -> (0,200)`
    // via right-angle corner) whose interior `x`-bulge crosses a vertical line at `x = 80`.
    let mut bulge = vec![
        route(
            &[(0.0, 0.0), (100.0, 0.0), (100.0, 200.0), (0.0, 200.0)],
            Axis::Vertical,
        ),
        route(&[(80.0, -20.0), (80.0, 220.0)], Axis::Vertical),
    ];
    for i in 0..34 {
        let x = 200.0 + (i as f64) * 6.0;
        bulge.push(route(&[(x, 0.0), (x + 12.0, 200.0)], Axis::Vertical));
    }
    let (g3, o3) = drawing(bulge);
    let c3 = assert_equivalent(&g3, &o3, RoutingStyle::Curved);
    assert!(c3 >= 1, "non-x-monotone curve crossing must match Bands");

    // 4. Mixed-endpoint coincident-`x` bundle where two distinct endpoints both sit within
    // `FAN = 24.0` of the shared coordinate: fan peers from endpoint A must not shield chains
    // from endpoint B during coincident-`x` neighbor walks.
    let mut mixed_routes = Vec::new();
    // 4 edges from endpoint A starting at (100, 100) fanning right and crossing at y > 160.
    for i in 0..4 {
        let xb = 120.0 + (i as f64) * 15.0;
        mixed_routes.push(route(&[(100.0, 100.0), (xb, 240.0)], Axis::Vertical));
    }
    // 3 edges from endpoint B starting at (100, 100) fanning wider right (crossing A's edges
    // below the fan radius via a bend) and one passing straight through (100, 100) from above.
    mixed_routes.push(route(
        &[(85.0, 60.0), (100.0, 100.0), (190.0, 240.0)],
        Axis::Vertical,
    ));
    mixed_routes.push(route(&[(100.0, 100.0), (180.0, 240.0)], Axis::Vertical));
    mixed_routes.push(route(&[(100.0, 100.0), (110.0, 240.0)], Axis::Vertical));
    // Add transverse horizontal/sloped lines crossing the fan well outside FAN.
    mixed_routes.push(route(
        &[(100.0, 100.0), (150.0, 200.0), (90.0, 260.0)],
        Axis::Vertical,
    ));
    for i in 0..34 {
        let x = 300.0 + (i as f64) * 6.0;
        mixed_routes.push(route(&[(x, 60.0), (x + 5.0, 260.0)], Axis::Vertical));
    }
    let (mut g4, mut o4) = drawing(mixed_routes);
    // Assign edges 0..4 to shared endpoint NodeId(0) at (100, 100), edges 4..8 to NodeId(2) at (100, 110).
    o4.nodes[0] = Rect {
        x: 96.0,
        y: 96.0,
        width: 8.0,
        height: 8.0,
    };
    o4.nodes[2] = Rect {
        x: 96.0,
        y: 106.0,
        width: 8.0,
        height: 8.0,
    };
    for edge in &mut g4.edges[0..4] {
        edge.source = NodeId(0).into();
    }
    for edge in &mut g4.edges[4..8] {
        edge.source = NodeId(2).into();
    }
    let c4 = assert_equivalent(&g4, &o4, RoutingStyle::Polyline);
    assert!(c4 >= 1, "mixed-endpoint coincident fan must match Bands");

    // 5. Subnormal `dy` produces infinite slope `dx / dy`: must never false-reject crossings.
    let mut subnormal = vec![
        route(&[(0.0, 0.0), (100.0, 1e-320)], Axis::Vertical),
        route(&[(50.0, -10.0), (50.0, 10.0)], Axis::Vertical),
    ];
    for i in 0..34 {
        let x = 200.0 + (i as f64) * 5.0;
        subnormal.push(route(&[(x, -10.0), (x + 2.0, 10.0)], Axis::Vertical));
    }
    let (g5, o5) = drawing(subnormal);
    let c5 = assert_equivalent(&g5, &o5, RoutingStyle::Polyline);
    assert_eq!(c5, 1, "subnormal-dy segment crossing must match Bands");

    // 6. Multi-chord EV_CROSS chord-boundary graze followed by a downstream crossing:
    // two curved primitives sharing a touching knot/chord graze before crossing on a later chord.
    let mut graze_curves = vec![
        route(&[(0.0, 0.0), (100.0, 100.0)], Axis::Vertical),
        route(&[(100.0, 0.0), (100.0, 100.0)], Axis::Vertical),
        // Two opposite-curvature S-curves whose chord interpolants touch near mid-height and cross.
        route(&[(0.0, 0.0), (500.0, 40.0)], Axis::Vertical),
        route(&[(0.0, 0.0), (501.0, 40.0)], Axis::Vertical),
        // Multi-segment curved double-bend touching then crossing.
        route(
            &[(0.0, 0.0), (60.0, 100.0), (0.0, 200.0), (120.0, 300.0)],
            Axis::Vertical,
        ),
        route(
            &[(60.0, 0.0), (60.0, 100.0), (80.0, 200.0), (20.0, 300.0)],
            Axis::Vertical,
        ),
    ];
    for i in 0..34 {
        let x = 600.0 + (i as f64) * 8.0;
        graze_curves.push(route(&[(x, 0.0), (x + 20.0, 300.0)], Axis::Vertical));
    }
    let (g6, o6) = drawing(graze_curves);
    let c6 = assert_equivalent(&g6, &o6, RoutingStyle::Curved);
    assert!(
        c6 >= 1,
        "chord-boundary graze followed by crossing must match Bands"
    );
}

#[test]
fn monotone_sweep_matches_bands_on_eps_bundles_large_x_fans_inverted_lenses_and_huge_y() {
    let assert_equivalent = |graph: &Graph, out: &LayoutOutput, style: RoutingStyle| -> u64 {
        let mut noop = NoopWorkControl;
        let mut work = Work::new(&mut noop);
        let sweep_count = count_crossings_impl(graph, out, style, u64::MAX, true, &mut work)
            .expect("sweep count");
        let band_count = count_crossings_impl(graph, out, style, u64::MAX, false, &mut work)
            .expect("bands count");
        assert_eq!(sweep_count, band_count, "sweep vs bands mismatch");
        sweep_count
    };

    // (a) Non-transitive `eps` foreign-neighbor bundle on both left and right walks:
    // `F` sits 1.5*eps away from `target` (non-coincident with `target`), while `C1`/`N` sit
    // within `0.8*eps` of `F` and `0.8*eps` of `target`. Back-filling `pure_left`/`pure_right`
    // across `C1`/`C2` would let `N` skip checking `F`.
    let mut eps_routes = vec![
        // Left foreign chain active before y = 0.0, passing through (100.0 - 1.5e-7, 0.0)
        // (`dx/dy = +0.2`) and slanting right across C1, C2, target, and N.
        route(
            &[(98.0 - 1.5e-7, -10.0), (140.0 - 1.5e-7, 200.0)],
            Axis::Vertical,
        ),
        // Right foreign chain active before y = 0.0, passing through (100.0 + 1.5e-7, 0.0)
        // (`dx/dy = -0.2`) and slanting left across C1_r, C2_r, target_r, and N_r.
        route(
            &[(102.0 + 1.5e-7, -10.0), (60.0 + 1.5e-7, 200.0)],
            Axis::Vertical,
        ),
        // Fan A chains on left of target: C1, C2, target, then N inserted after target.
        route(&[(100.0 - 0.8e-7, 0.0), (80.0, 200.0)], Axis::Vertical),
        route(&[(100.0 - 0.4e-7, 0.0), (85.0, 200.0)], Axis::Vertical),
        route(&[(100.0, 0.0), (95.0, 200.0)], Axis::Vertical),
        route(&[(100.0 - 0.7e-7, 0.0), (75.0, 200.0)], Axis::Vertical),
        // Fan A chains on right of target: C1_r, C2_r, target_r, then N_r inserted after target_r.
        route(&[(100.0 + 0.8e-7, 0.0), (120.0, 200.0)], Axis::Vertical),
        route(&[(100.0 + 0.4e-7, 0.0), (115.0, 200.0)], Axis::Vertical),
        route(&[(100.0, 0.0), (105.0, 200.0)], Axis::Vertical),
        route(&[(100.0 + 0.7e-7, 0.0), (125.0, 200.0)], Axis::Vertical),
    ];
    for i in 0..32 {
        let x = 300.0 + (i as f64) * 5.0;
        eps_routes.push(route(&[(x, -10.0), (x + 10.0, 200.0)], Axis::Vertical));
    }
    let (mut g_eps, mut o_eps) = drawing(eps_routes);
    o_eps.nodes[4] = Rect {
        x: 96.0,
        y: -4.0,
        width: 8.0,
        height: 8.0,
    };
    for edge in &mut g_eps.edges[2..10] {
        edge.source = NodeId(4).into();
    }
    let c_eps = assert_equivalent(&g_eps, &o_eps, RoutingStyle::Polyline);
    assert!(c_eps >= 8, "expected foreign-bundle crossings, got {c_eps}");

    // (b) Large-`|x|` (`100_000+`) fan boundary where `coincident_eps(x) ≈ 1e-4 >> 1e-6`:
    // fan chains straddling the exact `FAN = 24.0` boundary must not prune crossings outside `FAN`.
    let base = 100_000.0_f64;
    let fan_edge = base + 20.0 + FAN;
    let mut big_x_routes = vec![
        route(
            &[(fan_edge - 6e-5, 10.0), (fan_edge + 20.0, 210.0)],
            Axis::Vertical,
        ),
        route(
            &[(fan_edge - 3e-5, 10.0), (fan_edge + 10.0, 210.0)],
            Axis::Vertical,
        ),
        route(
            &[(fan_edge + 2e-5, 10.0), (fan_edge - 20.0, 210.0)],
            Axis::Vertical,
        ),
        route(
            &[(fan_edge + 4e-5, 10.0), (fan_edge - 30.0, 210.0)],
            Axis::Vertical,
        ),
    ];
    for i in 0..32 {
        let x = base + 200.0 + (i as f64) * 5.0;
        big_x_routes.push(route(&[(x, 10.0), (x + 8.0, 210.0)], Axis::Vertical));
    }
    let (mut g_big, mut o_big) = drawing(big_x_routes);
    o_big.nodes[0] = Rect {
        x: base,
        y: 0.0,
        width: 20.0,
        height: 20.0,
    };
    for edge in &mut g_big.edges[0..4] {
        edge.source = NodeId(0).into();
    }
    let c_big = assert_equivalent(&g_big, &o_big, RoutingStyle::Polyline);
    assert!(
        c_big >= 1,
        "large-x fan boundary crossings must match Bands"
    );

    // (c) Inverted double-crossing lens + `EV_HORIZ` slicing across the lens interior:
    // chains cross twice with an early swap and a return crossing, intersected by horizontal lines.
    let mut lens_routes = vec![
        route(
            &[(100.0, 0.0), (140.0, 50.0), (140.0, 150.0), (100.0, 200.0)],
            Axis::Vertical,
        ),
        route(
            &[(120.0, 0.0), (110.0, 50.0), (110.0, 150.0), (120.0, 200.0)],
            Axis::Vertical,
        ),
        // Same-start inverted crossing pair crossing immediately and again before y_hi.
        route(
            &[(115.0, 0.0), (135.0, 100.0), (105.0, 200.0)],
            Axis::Vertical,
        ),
        route(
            &[(115.0, 0.0), (115.0, 100.0), (130.0, 200.0)],
            Axis::Vertical,
        ),
        // Horizontal probes across the lens interior and crossing vertices.
        route(&[(90.0, 40.0), (150.0, 40.0)], Axis::Horizontal),
        route(&[(90.0, 100.0), (150.0, 100.0)], Axis::Horizontal),
        route(&[(90.0, 160.0), (150.0, 160.0)], Axis::Horizontal),
    ];
    for i in 0..32 {
        let x = 250.0 + (i as f64) * 6.0;
        lens_routes.push(route(&[(x, 0.0), (x + 12.0, 200.0)], Axis::Vertical));
    }
    let (g_lens, o_lens) = drawing(lens_routes);
    assert_equivalent(&g_lens, &o_lens, RoutingStyle::Polyline);
    let c_lens = assert_equivalent(&g_lens, &o_lens, RoutingStyle::Curved);
    assert!(
        c_lens >= 10,
        "inverted lens + EV_HORIZ crossings must match Bands"
    );

    // (d) Huge `y` (`-1e308..1e308`) where `dy = y1 - y0 = +inf` must NOT be misclassified
    // as a zero-slope vertical sweep chain (`dx / +inf == 0.0`).
    let mut huge_y_routes = vec![
        route(&[(-100.0, -1e308), (100.0, 1e308)], Axis::Vertical),
        route(&[(0.0, -100.0), (0.0, 100.0)], Axis::Vertical),
    ];
    for i in 0..34 {
        let x = 300.0 + (i as f64) * 5.0;
        huge_y_routes.push(route(&[(x, -100.0), (x + 8.0, 100.0)], Axis::Vertical));
    }
    let (g_huge, o_huge) = drawing(huge_y_routes);
    let mut noop = NoopWorkControl;
    let mut work = Work::new(&mut noop);
    let mut cl = trace(&o_huge, RoutingStyle::Polyline, &mut work).unwrap();
    let (mask, _) = sweep::x_overlap_mask(&cl.primitives, &mut work).unwrap();
    let partition = sweep::MonotonePartition::build(&mut cl, &mask, &mut work)
        .unwrap()
        .expect("sweep partition should activate for >= 32 chains");
    assert!(
        !partition.in_sweep[0] && partition.other.contains(&0),
        "non-finite dy segment must be excluded from sweep chains and routed to Bands"
    );
    let sweep_huge = count_crossings_impl(
        &g_huge,
        &o_huge,
        RoutingStyle::Polyline,
        u64::MAX,
        true,
        &mut Work::new(&mut NoopWorkControl),
    );
    let bands_huge = count_crossings_impl(
        &g_huge,
        &o_huge,
        RoutingStyle::Polyline,
        u64::MAX,
        false,
        &mut Work::new(&mut NoopWorkControl),
    );
    assert_eq!(
        sweep_huge, bands_huge,
        "sweep and Bands must agree on non-finite dy segment"
    );
}

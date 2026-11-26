use super::*;
use crate::flat::{Expanded, physical_side, prepare_component, refine, routing, validate};
use crate::{Direction, NodeId, NodeSpec, NoopWorkControl, Size, WorkControl};

// Flowchart reserves the full viewport and rotational depth, while its SVG arrow uses meet.
const POINT_INK_WIDTH: f64 = 294.0 / 23.0;
const POINT_MARKER: MarkerMetrics = MarkerMetrics {
    length: 16.619429464123963,
    width: 14.0,
};

fn measured_fan(direction: Direction, count: u32, marked_source: bool, audit: bool) -> Graph {
    let horizontal = direction.axis() == Axis::Horizontal;
    let size = if horizontal {
        Size {
            width: 45.0,
            height: 152.0,
        }
    } else {
        Size {
            width: 152.0,
            height: 45.0,
        }
    };
    let mut graph = Graph {
        direction,
        ..Graph::default()
    };
    for seq in 0..count + 2 {
        let mut node = NodeSpec::new(size, seq);
        node.port_span = Some(136.0);
        node.perpendicular_port_span = Some(29.0);
        graph.nodes.push(node);
    }
    let edge = |from, to, marked| {
        let mut edge = EdgeSpec::new(NodeId(from).into(), NodeId(to).into());
        edge.start_marker.length = 6.0;
        if marked {
            edge.start_marker = POINT_MARKER;
        }
        edge.end_marker = POINT_MARKER;
        edge
    };
    for i in 2..count + 2 {
        graph.edges.push(edge(0, i, marked_source));
    }
    for i in 2..count + 2 {
        graph.edges.push(edge(i, 1, false));
    }
    if audit {
        for i in 0..7 {
            let mut audit = edge(i + 2, count + 1 - i, false);
            audit.label = Some(Size {
                width: 28.0,
                height: 21.0,
            });
            graph.edges.push(audit);
        }
    }
    graph
}

fn initial(graph: &Graph, options: &Options) -> (Component, Drawing) {
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let c = validate::components(graph, options, &mut work)
        .unwrap()
        .pop()
        .unwrap();
    let mut expanded = Expanded::default();
    let (orientation, _) = prepare_component(&c, graph, options, &mut work, &mut expanded).unwrap();
    let mut x = super::super::position::assign(
        &mut expanded,
        &c,
        Some(&orientation),
        graph,
        options,
        &mut work,
    )
    .unwrap();
    refine::refine(
        &mut expanded,
        &c,
        &orientation,
        graph,
        options,
        &mut x,
        &mut work,
    )
    .unwrap();
    let mut drawing =
        routing::route(&expanded, &c, &orientation, graph, options, &x, &mut work).unwrap();
    super::super::extras::finish(&mut drawing, &expanded, &c, graph, options, &mut work).unwrap();
    (c, drawing)
}

fn apply(drawing: &mut Drawing, c: &Component, graph: &Graph, options: &Options) {
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    spread(drawing, c, graph, options, &mut work).unwrap();
    work.finish().unwrap();
}

fn check_hub(edges: &[EdgeGeometry], end: End, dir: Direction) -> Vec<usize> {
    let [main, left, right] =
        [end.face(), Side::Left, Side::Right].map(|face| physical_side(dir, face));
    let mut primary = Vec::new();
    let mut sides = [Vec::new(), Vec::new()];
    for (index, edge) in edges.iter().enumerate() {
        let side = end.side(edge);
        let at = end.point(&edge.points, 0);
        let next = end.point(&edge.points, 1);
        assert_eq!(at.axis, side.axis());
        let (along, across) = match side {
            Side::Top => (at.y - next.y, at.x == next.x),
            Side::Bottom => (next.y - at.y, at.x == next.x),
            Side::Left => (at.x - next.x, at.y == next.y),
            Side::Right => (next.x - at.x, at.y == next.y),
        };
        let offset = if side.axis() == Axis::Vertical {
            at.x
        } else {
            at.y
        };
        assert!(across && along >= POINT_MARKER.length, "{at:?} {next:?}");
        if side == main {
            primary.push((offset, index));
        } else {
            assert!(side == left || side == right);
            sides[usize::from(side == right)].push(offset);
        }
    }
    for offsets in &mut sides {
        assert_eq!(offsets.len(), 3);
        offsets.sort_by(f64::total_cmp);
        assert!(
            offsets
                .windows(2)
                .all(|pair| pair[1] - pair[0] >= POINT_MARKER.width)
        );
    }
    assert_eq!(primary.len(), 10);
    primary.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert!(
        primary
            .windows(2)
            .all(|pair| pair[1].0 - pair[0].0 >= POINT_INK_WIDTH * MARKER_PITCH)
    );
    primary.into_iter().map(|(_, index)| index).collect()
}

#[test]
fn measured_sink_moves_only_the_ordered_outer_occurrences_and_preserves_marker_leads() {
    let graph = measured_fan(Direction::Down, 16, false, true);
    let options = Options::default();
    let (c, mut drawing) = initial(&graph, &options);
    let old = drawing.edges.clone();
    let mut before = (0..16).collect::<Vec<_>>();
    before.sort_by(|&a, &b| {
        let x = |i: usize| old[16 + i].1.points.last().unwrap().x;
        x(a).total_cmp(&x(b))
    });
    apply(&mut drawing, &c, &graph, &options);
    let routes: Vec<_> = drawing.edges[16..32]
        .iter()
        .map(|(_, e)| e.clone())
        .collect();
    assert_eq!(
        check_hub(&routes, End::Target, Direction::Down),
        before[3..13]
    );
    assert_eq!(&drawing.edges[..16], &old[..16]);
    assert_eq!(&drawing.edges[32..], &old[32..]);
    for (old, new) in old[16..32].iter().zip(&drawing.edges[16..32]) {
        assert_eq!(old.1.points[0], new.1.points[0]);
    }
}

#[test]
fn marked_fanout_and_fanin_use_the_correct_physical_faces_in_all_directions() {
    for direction in [
        Direction::Down,
        Direction::Up,
        Direction::Right,
        Direction::Left,
    ] {
        let graph = measured_fan(direction, 16, true, false);
        let options = Options::default();
        let routes = crate::layout(&graph, &options).unwrap().edges;
        check_hub(&routes[..16], End::Source, direction);
        check_hub(&routes[16..], End::Target, direction);
    }
}

#[test]
fn a_foreign_label_or_authored_primary_attachment_preserves_the_entire_row() {
    let mut graph = measured_fan(Direction::Down, 16, false, true);
    let options = Options::default();
    let (c, mut drawing) = initial(&graph, &options);
    let left = drawing.nodes[1].x;
    let y = drawing.nodes[1].center().y;
    drawing.edges[32].1.label_center = Some(Point { x: left - 12.0, y });
    let before = drawing.edges.clone();
    apply(&mut drawing, &c, &graph, &options);
    assert_eq!(drawing.edges, before);

    graph.edges[17].target_port = PortSideHint::FixedOffset {
        side: Side::Top,
        offset: 3.0,
    };
    let (c, mut drawing) = initial(&graph, &options);
    let before = drawing.edges.clone();
    apply(&mut drawing, &c, &graph, &options);
    assert_eq!(drawing.edges, before);
}

#[test]
fn lead_only_ports_keep_their_existing_routes_even_below_minimum_pitch() {
    let mut graph = measured_fan(Direction::Down, 16, false, true);
    // One visible marker cannot have a crowding conflict with another visible marker.
    for edge in &mut graph.edges[17..32] {
        edge.end_marker.width = 0.0;
    }
    let mut options = Options::default();
    options.spacing.min_port_pitch = 15.0;
    let (c, mut drawing) = initial(&graph, &options);
    let before = drawing.edges.clone();
    apply(&mut drawing, &c, &graph, &options);
    assert_eq!(drawing.edges, before);
}

#[derive(Default)]
struct Meter {
    total: usize,
    budget: Option<usize>,
}

impl WorkControl for Meter {
    fn check(&mut self) -> Result<(), WorkError> {
        Ok(())
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        let total = self.total + units;
        if self.budget.is_some_and(|budget| total > budget) {
            return Err(WorkError::Interrupted);
        }
        self.total = total;
        Ok(())
    }
}

#[test]
fn a_250_leaf_fan_without_face_capacity_uses_only_linear_work() {
    let graph = measured_fan(Direction::Down, 250, true, false);
    let options = Options::default();
    let (c, mut drawing) = initial(&graph, &options);
    let before = drawing.edges.clone();
    let mut meter = Meter::default();
    let mut work = Work::new(&mut meter);
    spread(&mut drawing, &c, &graph, &options, &mut work).unwrap();
    work.finish().unwrap();
    assert_eq!(drawing.edges, before);
    assert!(meter.total <= 2 * c.nodes.len() + c.links.len());
}

#[test]
fn sparse_hubs_spread_within_the_wall_query_budget() {
    let mut graph = measured_fan(Direction::Down, 16, true, false);
    let options = Options::default();
    let mut previous = NodeId(1);
    for i in 18..500 {
        graph.nodes.push(NodeSpec::new(graph.nodes[2].size, i));
        let next = NodeId(i);
        graph
            .edges
            .push(EdgeSpec::new(previous.into(), next.into()));
        previous = next;
    }
    let (c, mut drawing) = initial(&graph, &options);
    let mut meter = Meter {
        budget: Some(30_000),
        ..Meter::default()
    };
    let mut work = Work::new(&mut meter);
    spread(&mut drawing, &c, &graph, &options, &mut work).unwrap();
    work.finish().unwrap();

    let routes: Vec<_> = drawing.edges[..32].iter().map(|(_, e)| e.clone()).collect();
    check_hub(&routes[..16], End::Source, Direction::Down);
    check_hub(&routes[16..], End::Target, Direction::Down);
}

#[test]
fn interrupted_spread_restores_earlier_runs_before_retrying_with_the_same_scratch() {
    let graph = measured_fan(Direction::Down, 16, true, false);
    let options = Options::default();
    let (c, original) = initial(&graph, &options);
    let mut drawing = Drawing {
        nodes: original.nodes.clone(),
        edges: original.edges.clone(),
    };
    let mut scratch = HubScratch::default();
    let mut meter = Meter::default();
    spread_with_scratch(
        &mut drawing,
        &c,
        &graph,
        &options,
        &mut scratch,
        &mut Work::new(&mut meter),
    )
    .unwrap();
    let expected = std::mem::replace(&mut drawing.edges, original.edges.clone());
    assert_ne!(expected[..16], original.edges[..16]);
    assert_ne!(expected[16..], original.edges[16..]);

    meter.budget = Some(meter.total - 1);
    meter.total = 0;
    assert!(matches!(
        spread_with_scratch(
            &mut drawing,
            &c,
            &graph,
            &options,
            &mut scratch,
            &mut Work::new(&mut meter),
        ),
        Err(crate::LayoutError::Work(WorkError::Interrupted))
    ));
    assert_eq!(drawing.edges, original.edges);

    spread_with_scratch(
        &mut drawing,
        &c,
        &graph,
        &options,
        &mut scratch,
        &mut Work::new(&mut NoopWorkControl),
    )
    .unwrap();
    assert_eq!(drawing.edges, expected);
}

fn separated_routes(count: u32) -> (Graph, Drawing) {
    let mut graph = Graph::default();
    let mut drawing = Drawing::default();
    for i in 0..count {
        let mut spec = EdgeSpec::new(NodeId(0).into(), NodeId(1).into());
        spec.start_marker = MarkerMetrics {
            length: 3.0,
            width: 2.0,
        };
        spec.end_marker = spec.start_marker;
        graph.edges.push(spec);
        let x = f64::from(i) * 40.0;
        drawing.edges.push((
            i,
            EdgeGeometry {
                points: vec![
                    RoutePoint::new(x, 0.0, Axis::Vertical),
                    RoutePoint::new(x, 40.0, Axis::Vertical),
                ],
                ports: (Side::Bottom, Side::Top),
                label_center: None,
                start_label_center: None,
                end_label_center: None,
                corridor_clearance: 2.0,
                shared_bus: false,
                reversed_for_ranking: false,
            },
        ));
    }
    (graph, drawing)
}

fn shifted_contact(graph: &Graph, drawing: &Drawing, edge: usize, end: End, x: f64) -> Edit {
    let (id, route) = &drawing.edges[edge];
    let contact = point(end.point(&route.points, 0));
    let peer = point(end.peer().point(&route.points, 0));
    let at = Point { x, ..contact };
    let guard = Point {
        x,
        y: at.y + 6.0 * end.outer_y(),
    };
    let fixed = Point {
        y: at.y + 12.0 * end.outer_y(),
        ..contact
    };
    let mut points = [at, guard, fixed, peer]
        .map(|p| RoutePoint::new(p.x, p.y, Axis::Vertical))
        .to_vec();
    if end == End::Target {
        points.reverse();
    }
    Edit {
        side: end.face(),
        body: [Bounds::between(at, guard), Bounds::between(guard, fixed)],
        retained: Bounds::between(fixed, peer),
        envelope: Bounds::path(&points),
        mark: marker(at, end.face(), end.marker(&graph.edges[*id as usize])),
        points,
    }
}

fn linear_allows(graph: &Graph, drawing: &Drawing, moved: End, proposed: &[(usize, Edit)]) -> bool {
    for (edge, edit) in proposed {
        for (i, (id, route)) in drawing.edges.iter().enumerate() {
            let patch = proposed.iter().find(|(edge, _)| *edge == i);
            for end in End::BOTH {
                if i == *edge && end == moved {
                    continue;
                }
                let bounds = match patch {
                    Some((_, other)) if end == moved => other.mark,
                    _ => marker(
                        point(end.point(&route.points, 0)),
                        end.side(route),
                        end.marker(&graph.edges[*id as usize]),
                    ),
                };
                if bounds.is_some_and(|bounds| edit.blocked_by(bounds)) {
                    return false;
                }
            }
            if i != *edge
                && let Some(mark) = edit.mark
                && match patch {
                    Some((_, other)) => {
                        other.retained.overlaps_strictly(mark)
                            || other.body.iter().any(|part| part.overlaps_strictly(mark))
                    }
                    None => Bounds::path(&route.points).overlaps_strictly(mark),
                }
            {
                return false;
            }
        }
    }
    true
}

#[test]
fn proposals_match_linear_clearance_for_replaced_parts_and_strict_marker_contact() {
    let (graph, drawing) = separated_routes(20);
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut proof = Clearance::new(&drawing, &graph, &mut work).unwrap();
    proof.remaining = usize::MAX;
    for end in End::BOTH {
        for count in [2, 10] {
            for (x, expected) in [(0.0, true), (4.0, true), (8.0, false), (10.0, true)] {
                // x=0 uses the old contact. x=4 lies between the first edit's protected
                // parts. At x=8 the markers overlap, and at x=10 they touch at the edge.
                let proposed = (0..count)
                    .map(|i| {
                        let x = match i {
                            0 => 8.0,
                            1 => x,
                            _ => i as f64 * 40.0 + 4.0,
                        };
                        (i, shifted_contact(&graph, &drawing, i, end, x))
                    })
                    .collect::<Vec<_>>();
                assert_eq!(linear_allows(&graph, &drawing, end, &proposed), expected);
                assert_eq!(proof.allows(end, &proposed, &mut work), Ok(expected));
            }
        }
    }

    // The unchanged peer marker still protects this edge, even without a moved marker.
    let mut proposed = [(0, shifted_contact(&graph, &drawing, 0, End::Source, 8.0))];
    proposed[0].1.mark = None;
    for (x, expected) in [(1.0, true), (0.5, false)] {
        let p = Point { x, y: 38.0 };
        proposed[0].1.body[0] = Bounds::between(p, p);
        assert_eq!(
            linear_allows(&graph, &drawing, End::Source, &proposed),
            expected
        );
        assert_eq!(
            proof.allows(End::Source, &proposed, &mut work),
            Ok(expected)
        );
    }
}

#[test]
fn successive_runs_replace_cached_bounds_and_preserve_the_other_end() {
    let (mut graph, mut drawing) = separated_routes(32);
    graph.edges[1].start_marker.width = 0.0;
    graph.edges[1].end_marker.width = 0.0;
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut proof = Clearance::new(&drawing, &graph, &mut work).unwrap();
    let mut saved = Vec::new();
    let check =
        |proof: &mut Clearance, drawing: &Drawing, work: &mut Work<'_>, end, edge, x, expected| {
            let trial = [(edge, shifted_contact(&graph, drawing, edge, end, x))];
            assert_eq!(linear_allows(&graph, drawing, end, &trial), expected);
            assert_eq!(proof.allows(end, &trial, work), Ok(expected));
        };
    check(&mut proof, &drawing, &mut work, End::Source, 1, 0.0, false);
    check(&mut proof, &drawing, &mut work, End::Source, 2, 4.0, true);

    for (end, edges) in [
        (End::Source, 0..1),
        (End::Target, 0..1),
        (End::Source, 4..10),
    ] {
        let mut proposed = edges
            .map(|i| {
                let x = i as f64 * 40.0 + 8.0;
                (i, shifted_contact(&graph, &drawing, i, end, x))
            })
            .collect::<Vec<_>>();
        assert!(proof.allows(end, &proposed, &mut work).unwrap());
        proof
            .accept(end, &mut proposed, &mut drawing, &mut saved, &mut work)
            .unwrap();

        check(&mut proof, &drawing, &mut work, End::Source, 1, 0.0, true);
        check(&mut proof, &drawing, &mut work, End::Source, 1, 8.0, false);
        check(&mut proof, &drawing, &mut work, End::Source, 2, 4.0, false);
    }
    check(&mut proof, &drawing, &mut work, End::Target, 1, 8.0, false);
    assert!(proof.changed.is_empty());
}

#[test]
fn clearance_checks_proposal_indices_and_rejects_an_incomplete_walk() {
    let (graph, drawing) = separated_routes(16);
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut proof = Clearance::new(&drawing, &graph, &mut work).unwrap();
    for edges in [[1, 0], [0, 0], [0, drawing.edges.len()]] {
        let bad = edges.map(|i| (i, shifted_contact(&graph, &drawing, 0, End::Source, 4.0)));
        assert!(matches!(
            proof.allows(End::Source, &bad, &mut work),
            Err(LayoutError::InvalidInput(_))
        ));
    }
    let proposed = [(0, shifted_contact(&graph, &drawing, 0, End::Source, 4.0))];
    assert!(proof.allows(End::Source, &proposed, &mut work).unwrap());
    proof.remaining = 3;
    assert!(!proof.allows(End::Source, &proposed, &mut work).unwrap());
    assert_eq!(proof.remaining, 0);
}

#[test]
fn sparse_clearance_queries_use_subquadratic_work() {
    let mut previous = None;
    for count in [128, 512, 2048] {
        let (graph, drawing) = separated_routes(count);
        let mut proof =
            Clearance::new(&drawing, &graph, &mut Work::new(&mut NoopWorkControl)).unwrap();
        let proposed = (0..count)
            .step_by(4)
            .map(|i| {
                let x = f64::from(i) * 40.0 + 4.0;
                (
                    i as usize,
                    shifted_contact(&graph, &drawing, i as usize, End::Source, x),
                )
            })
            .collect::<Vec<_>>();
        let remaining = proof.remaining;
        let mut meter = Meter::default();
        let mut work = Work::new(&mut meter);
        assert!(proof.allows(End::Source, &proposed, &mut work).unwrap());
        work.finish().unwrap();
        let visits = remaining - proof.remaining;
        if let Some((old_visits, old_total)) = previous {
            assert!(
                visits < old_visits * 8,
                "{count} routes took {visits} visits"
            );
            assert!(
                meter.total < old_total * 8,
                "{count} routes took {} units",
                meter.total
            );
        }
        let linear_visits = drawing.edges.len() * proposed.len();
        assert!(
            visits < linear_visits,
            "{count} routes took {visits} visits"
        );
        if count == 2048 {
            assert!(visits < linear_visits / 8);
        }
        previous = Some((visits, meter.total));
    }
}

#[test]
fn wall_queries_distinguish_strict_overlap_from_contact_at_the_boundary() {
    let walls = (0..113)
        .map(|index| {
            let x = f64::from(index) * 4.0;
            Bounds {
                x: [x, x + 2.0],
                y: [0.0, 2.0],
            }
        })
        .collect::<Vec<_>>();
    let point = |x, y| Bounds {
        x: [x; 2],
        y: [y; 2],
    };
    let clear = point(-1.0, -1.0);
    let mut edit = Edit {
        points: Vec::new(),
        side: Side::Top,
        body: [clear; 2],
        retained: clear,
        envelope: clear,
        mark: Some(point(197.0, 1.0)),
    };
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let mut clearance = Clearance {
        walls: Bvh::from_bounds(walls.clone(), &mut work).unwrap(),
        index: RouteIndex::new(std::iter::empty(), &mut work).unwrap(),
        routes: Vec::new(),
        changed: Vec::new(),
        remaining: walls.len() * CLEARANCE_PASSES,
    };
    assert!(!clearance.allows_walls(&edit, &mut work).unwrap());

    edit.mark = None;
    assert!(clearance.allows_walls(&edit, &mut work).unwrap());
    for wall in &walls {
        let [left, right] = wall.x;
        edit.body[0] = point(left, 1.0);
        assert!(clearance.allows_walls(&edit, &mut work).unwrap());
        edit.body[0] = point(left.midpoint(right), 1.0);
        assert!(!clearance.allows_walls(&edit, &mut work).unwrap());
    }
}

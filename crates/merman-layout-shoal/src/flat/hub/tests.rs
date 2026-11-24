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
    assert!(meter.0 <= 2 * c.nodes.len() + c.links.len());
}

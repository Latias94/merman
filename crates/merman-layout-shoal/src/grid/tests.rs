use super::*;
use crate::geom::Bounds;
use crate::{EdgeSpec, GroupSpec, NodeSpec, NoopWorkControl, Rect, RoutingStyle};

fn node(seq: u32, parent: Option<u32>) -> NodeSpec {
    let mut node = NodeSpec::new(
        Size {
            width: 60.0,
            height: 40.0,
        },
        seq,
    );
    node.parent = parent.map(GroupId);
    node
}

fn group(seq: u32, parent: Option<u32>) -> GroupSpec {
    let mut group = GroupSpec::new(seq);
    group.parent = parent.map(GroupId);
    group.title = Some(Size {
        width: 78.0,
        height: 20.0,
    });
    group
}

fn edge(source: u32, target: u32, a: Side, b: Side) -> EdgeSpec {
    let mut edge = EdgeSpec::new(NodeId(source).into(), NodeId(target).into());
    edge.source_port = PortSideHint::Side(a);
    edge.target_port = PortSideHint::Side(b);
    edge
}

fn solve(graph: &Graph, alignments: &[Alignment]) -> LayoutGeometry {
    let options = Options {
        routing: RoutingStyle::Orthogonal,
        capture_node_ranks: true,
        ..Default::default()
    };
    layout_with_control(graph, alignments, &options, &mut NoopWorkControl).unwrap()
}

fn at(knot: crate::RoutePoint) -> Point {
    Point {
        x: knot.x,
        y: knot.y,
    }
}

#[test]
fn four_faces_form_a_grid_and_keep_straight_routes() {
    let graph = Graph {
        direction: Direction::Right,
        nodes: (0..5).map(|i| node(i, None)).collect(),
        edges: vec![
            edge(0, 1, Side::Right, Side::Left),
            edge(0, 2, Side::Bottom, Side::Top),
            edge(0, 3, Side::Left, Side::Right),
            edge(0, 4, Side::Top, Side::Bottom),
        ],
        ..Default::default()
    };
    let solved = solve(&graph, &[]);
    let [center, right, bottom, left, top] = solved
        .nodes
        .iter()
        .map(|r| r.center())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    assert!(left.x < center.x && center.x < right.x && top.y < center.y && center.y < bottom.y);
    assert_eq!(
        (left.y, right.y, top.x, bottom.x),
        (center.y, center.y, center.x, center.x)
    );
    for path in &solved.edges {
        assert_eq!(path.points.len(), 4);
        assert!(path.points.iter().all(|p| p.axis == path.ports.0.axis()));
    }
    assert_eq!(solved, solve(&graph, &[]));
}

#[test]
fn declared_rows_and_columns_take_precedence_over_conflicting_wires() {
    let graph = Graph {
        direction: Direction::Right,
        nodes: (0..4).map(|i| node(i, Some(0))).collect(),
        groups: vec![group(0, None)],
        edges: vec![
            edge(0, 1, Side::Bottom, Side::Top),
            edge(1, 2, Side::Bottom, Side::Right),
            edge(2, 3, Side::Right, Side::Left),
        ],
        ..Default::default()
    };
    let align = |first, next, axis| Alignment {
        first: NodeId(first),
        next: NodeId(next),
        axis,
    };
    let solved = solve(
        &graph,
        &[
            align(0, 1, Axis::Horizontal),
            align(2, 3, Axis::Horizontal),
            align(0, 2, Axis::Vertical),
            align(1, 3, Axis::Vertical),
        ],
    );
    let [a, b, c, d] = solved
        .nodes
        .iter()
        .map(|r| r.center())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    assert!(a.x < b.x && a.y < c.y);
    assert_eq!((a.y, c.y), (b.y, d.y));
    assert_eq!((a.x, b.x), (c.x, d.x));
    assert_eq!(solved.edges[0].ports, (Side::Bottom, Side::Top));
}

fn contains(outer: Rect, inner: Rect) -> bool {
    let (a, b) = (Bounds::of(outer), Bounds::of(inner));
    a.x[0] <= b.x[0] && a.x[1] >= b.x[1] && a.y[0] <= b.y[0] && a.y[1] >= b.y[1]
}

#[test]
fn nested_and_empty_groups_pack_disjointly_and_keep_border_stitches() {
    let mut interior = edge(0, 2, Side::Right, Side::Left);
    interior.label = Some(Size {
        width: 45.0,
        height: 14.0,
    });
    let mut boundary = EdgeSpec::new(GroupId(0).into(), NodeId(1).into());
    boundary.source_port = PortSideHint::Side(Side::Left);
    boundary.target_port = PortSideHint::Side(Side::Left);
    let graph = Graph {
        direction: Direction::Right,
        nodes: vec![node(0, Some(1)), node(1, Some(0)), node(2, None)],
        groups: vec![group(0, None), group(1, Some(0)), group(2, None)],
        edges: vec![interior, boundary],
        ..Default::default()
    };
    let solved = solve(&graph, &[]);
    let outer = solved.groups[0].rect;
    let inner = solved.groups[1].rect;
    let empty = solved.groups[2].rect;
    assert!(
        contains(outer, inner)
            && contains(inner, solved.nodes[0])
            && contains(outer, solved.nodes[1])
    );
    assert!(!Bounds::of(outer).overlaps_strictly(Bounds::of(empty)));
    assert!(!Bounds::of(inner).overlaps_strictly(Bounds::of(solved.nodes[1])));
    assert!(!Bounds::of(outer).overlaps_strictly(Bounds::of(solved.nodes[2])));
    assert!(solved.edges[0].label_center.is_some());
    let [inner_border, outer_border] = solved.border_stitches.as_slice() else {
        panic!("missing nested stitches")
    };
    assert_eq!(
        (inner_border.group, outer_border.group),
        (GroupId(1), GroupId(0))
    );
    assert!(inner_border.point < outer_border.point);
    for stitch in &solved.border_stitches {
        let p = at(solved.edges[0].points[stitch.point]);
        let rect = solved.groups[stitch.group.0 as usize].rect;
        assert!(contains(
            rect,
            Rect {
                x: p.x,
                y: p.y,
                ..Default::default()
            }
        ));
        assert!(
            p.x == rect.x
                || p.x == rect.x + rect.width
                || p.y == rect.y
                || p.y == rect.y + rect.height
        );
    }
    let route = &solved.edges[1].points;
    assert_eq!(route[0].x, outer.x);
    assert!(route[1].x > route[0].x && route[1].y == route[0].y);
}

#[test]
fn blocked_facing_ports_find_an_orthogonal_channel_around_the_middle_node() {
    let graph = Graph {
        direction: Direction::Right,
        nodes: (0..3).map(|i| node(i, None)).collect(),
        edges: vec![edge(0, 2, Side::Right, Side::Left)],
        ..Default::default()
    };
    let solved = solve(
        &graph,
        &[
            Alignment {
                first: NodeId(0),
                next: NodeId(1),
                axis: Axis::Horizontal,
            },
            Alignment {
                first: NodeId(1),
                next: NodeId(2),
                axis: Axis::Horizontal,
            },
        ],
    );
    let blocker = Bounds::of(solved.nodes[1]);
    let route = &solved.edges[0].points;
    assert!(route.len() > 4);
    for leg in route.windows(2) {
        let (a, b) = (at(leg[0]), at(leg[1]));
        assert!(a.x == b.x || a.y == b.y);
        assert!(!Bounds::between(a, b).overlaps_strictly(blocker));
    }
}

#[test]
fn capped_visibility_routes_through_the_measured_grid_gutters() {
    let mut nodes: Vec<_> = (0..9).map(|i| node(i, None)).collect();
    nodes[3].size.width = 20.0;
    nodes[5].size.width = 20.0;
    let graph = Graph {
        direction: Direction::Right,
        nodes,
        edges: vec![edge(3, 5, Side::Right, Side::Left)],
        ..Default::default()
    };
    let mut alignments = Vec::new();
    for i in 0..9 {
        if i % 3 < 2 {
            alignments.push(Alignment {
                first: NodeId(i),
                next: NodeId(i + 1),
                axis: Axis::Horizontal,
            });
        }
        if i < 6 {
            alignments.push(Alignment {
                first: NodeId(i),
                next: NodeId(i + 3),
                axis: Axis::Vertical,
            });
        }
    }
    let mut options = Options::default();
    options.caps.max_ortho_grid_nodes = 0;
    let solved = layout_with_control(&graph, &alignments, &options, &mut NoopWorkControl).unwrap();
    let route = &solved.edges[0].points;
    assert!(route.len() >= 6);
    for leg in route.windows(2) {
        let (a, b) = (at(leg[0]), at(leg[1]));
        assert!(a.x == b.x || a.y == b.y);
        for &node in &solved.nodes {
            assert!(!Bounds::between(a, b).overlaps_strictly(Bounds::of(node)));
        }
    }
}

#[test]
fn opposite_boundary_leads_can_meet_at_one_point() {
    let mut wire = EdgeSpec::new(GroupId(0).into(), NodeId(0).into());
    wire.source_port = PortSideHint::Side(Side::Left);
    wire.target_port = PortSideHint::Side(Side::Left);
    wire.start_marker.length = 13.0;
    wire.end_marker.length = 13.0;
    let graph = Graph {
        nodes: vec![node(0, Some(0))],
        groups: vec![group(0, None)],
        edges: vec![wire],
        ..Default::default()
    };
    let solved = solve(&graph, &[]);
    let [start, joint, end] = solved.edges[0].points.as_slice() else {
        panic!("expected one shared lead knot");
    };
    assert_eq!(
        (start.x, end.x),
        (solved.groups[0].rect.x, solved.nodes[0].x)
    );
    assert_eq!((start.y, joint.y), (end.y, end.y));
    assert!(start.x < joint.x && joint.x < end.x);
}

#[test]
fn facing_fan_uses_distinct_tracks_within_its_work_budget() {
    use crate::ortho::OrthoSegment;

    let graph = Graph {
        direction: Direction::Right,
        nodes: (0..80).map(|i| node(i, None)).collect(),
        edges: (1..80)
            .map(|i| edge(0, i, Side::Right, Side::Left))
            .collect(),
        ..Default::default()
    };
    let solved = layout_with_control(&graph, &[], &Options::default(), &mut Limit(50_000)).unwrap();
    let tracks: Vec<_> = solved
        .edges
        .iter()
        .flat_map(|edge| {
            edge.points
                .windows(2)
                .filter_map(|leg| OrthoSegment::between(leg[0], leg[1]))
        })
        .filter(|leg| leg.axis == Axis::Vertical)
        .collect();
    assert_eq!(tracks.len(), 78);
    for (index, leg) in tracks.iter().enumerate() {
        assert!(
            tracks[..index].iter().all(|peer| {
                peer.fixed != leg.fixed || peer.max <= leg.min || leg.max <= peer.min
            })
        );
    }
}

struct Limit(usize);

impl WorkControl for Limit {
    fn check(&mut self) -> Result<(), WorkError> {
        Ok(())
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.0 = self.0.checked_sub(units).ok_or(WorkError::Interrupted)?;
        Ok(())
    }
}

#[test]
fn interrupted_grid_solve_leaves_input_reusable() {
    let graph = Graph {
        nodes: (0..40).map(|i| node(i, None)).collect(),
        edges: (1..40)
            .map(|i| edge(0, i, Side::Right, Side::Left))
            .collect(),
        ..Default::default()
    };
    let options = Options::default();
    assert_eq!(
        layout_with_control(&graph, &[], &options, &mut Limit(80)),
        Err(LayoutError::Work(WorkError::Interrupted))
    );
    assert!(layout_with_control(&graph, &[], &options, &mut NoopWorkControl).is_ok());
}

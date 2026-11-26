use super::*;
use crate::geom::Bounds;
use crate::{EdgeSpec, GroupSpec, KnotKind, NodeSpec, NoopWorkControl, Rect, RoutingStyle};

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
fn automatic_top_title_keeps_the_center_entry_straight_between_two_fans() {
    let mut title = group(0, None);
    title.title = Some(Size {
        width: 40.0,
        height: 20.0,
    });
    title.title_position = crate::TitlePosition::TopAuto;
    title.padding = crate::Insets {
        left: 8.0,
        right: 8.0,
        top: 22.0,
        bottom: 8.0,
    };
    let mut graph = Graph {
        direction: Direction::Right,
        nodes: (0..6)
            .map(|id| {
                let mut item = node(id, (id < 3).then_some(0));
                item.size = Size {
                    width: 16.0,
                    height: 36.0,
                };
                item
            })
            .collect(),
        groups: vec![title],
        ..Default::default()
    };
    for (from, to) in [(4, 1), (3, 0), (5, 2), (3, 0), (5, 2)] {
        let mut branch = edge(from, to, Side::Bottom, Side::Top);
        branch.source_port = PortSideHint::FixedOffset {
            side: Side::Bottom,
            offset: 0.0,
        };
        graph.edges.push(branch);
    }
    let alignments = [
        (0, 1, Axis::Horizontal),
        (1, 2, Axis::Horizontal),
        (3, 4, Axis::Horizontal),
        (4, 5, Axis::Horizontal),
        (3, 0, Axis::Vertical),
        (4, 1, Axis::Vertical),
        (5, 2, Axis::Vertical),
    ]
    .map(|(first, next, axis)| Alignment {
        first: NodeId(first),
        next: NodeId(next),
        axis,
    });
    let options = Options {
        bus_routing: false,
        ..Default::default()
    };
    let solved = layout_with_control(&graph, &alignments, &options, &mut NoopWorkControl).unwrap();
    let x = solved.nodes[1].center().x;
    let route = &solved.edges[0].points;
    assert!(route.iter().all(|knot| knot.x == x), "{route:?}");
    assert!(route.windows(2).all(|pair| pair[0].y < pair[1].y));
    let stitch = solved
        .border_stitches
        .iter()
        .find(|stitch| stitch.edge == crate::EdgeId(0))
        .unwrap();
    assert_eq!(
        (stitch.side, route[stitch.point].y),
        (Side::Top, solved.groups[0].rect.y)
    );
    let title_left = solved.groups[0].title_center.unwrap().x - 20.0;
    assert!(title_left - x >= options.spacing.edge);

    graph.groups[0].title_position = crate::TitlePosition::TopCenter;
    let centered =
        layout_with_control(&graph, &alignments, &options, &mut NoopWorkControl).unwrap();
    assert_eq!(centered.groups[0].title_center.unwrap().x, x);
    assert!(centered.edges[0].points.iter().any(|knot| knot.x != x));
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
fn card_captions_clear_other_scopes_and_stay_near_their_relationships() {
    let mut graph = Graph {
        direction: Direction::Right,
        nodes: vec![
            node(0, None),
            node(1, Some(1)),
            node(2, Some(1)),
            node(3, Some(0)),
        ],
        groups: vec![group(0, None), group(1, Some(0))],
        ..Default::default()
    };
    for (index, node) in graph.nodes.iter_mut().enumerate() {
        node.size = Size {
            width: 216.0,
            height: if index == 0 { 170.0 } else { 80.0 },
        };
    }
    for group in &mut graph.groups {
        group.title = Some(Size {
            width: 100.0,
            height: 26.0,
        });
    }
    let marker = crate::MarkerMetrics {
        length: 11.0,
        width: 11.0,
    };
    for (source, target, width, height) in
        [(0, 1, 310.0, 22.0), (1, 2, 80.0, 22.0), (1, 3, 110.0, 33.0)]
    {
        let mut relation = EdgeSpec::new(NodeId(source).into(), NodeId(target).into());
        relation.label = Some(Size { width, height });
        relation.end_marker = marker;
        if source == 0 {
            relation.start_marker = marker;
        }
        graph.edges.push(relation);
    }
    let mut options = Options::default();
    options.spacing.edge = 6.0;
    options.spacing.node = 50.0;
    options.spacing.rank = 50.0;
    let solve =
        || crate::card_grid::layout_with_control(&graph, &options, &mut NoopWorkControl).unwrap();
    let drawing = solve();
    let captions: Vec<_> = drawing
        .edges
        .iter()
        .zip(&graph.edges)
        .map(|(route, spec)| Bounds::centered(route.label_center.unwrap(), spec.label.unwrap()))
        .collect();
    for (i, caption) in captions.iter().enumerate() {
        for other in &captions[..i] {
            assert!(
                !caption.overlaps_strictly(*other),
                "captions from separate scopes overlap: {caption:?}, {other:?}"
            );
        }
    }
    for (i, caption) in captions.iter().enumerate() {
        for &card in &drawing.nodes {
            assert!(!caption.overlaps_strictly(Bounds::of(card)));
        }
        let center = drawing.edges[i].label_center.unwrap();
        let distance = drawing.edges[i]
            .points
            .windows(2)
            .map(|leg| {
                let segment = Bounds::between(at(leg[0]), at(leg[1]));
                (center.x - center.x.clamp(segment.x[0], segment.x[1]))
                    .hypot(center.y - center.y.clamp(segment.y[0], segment.y[1]))
            })
            .fold(f64::INFINITY, f64::min);
        assert!(
            distance <= 120.0,
            "caption {i} is {distance}px from its relationship"
        );
    }
    assert_eq!(drawing, solve());
}

#[test]
fn tall_card_caption_fits_between_rows_at_the_same_width() {
    let mut graph = Graph {
        nodes: vec![node(0, Some(0)), node(1, Some(0))],
        groups: vec![group(0, None)],
        edges: vec![edge(0, 1, Side::Bottom, Side::Top)],
        ..Default::default()
    };
    for node in &mut graph.nodes {
        node.size = Size {
            width: 216.0,
            height: 80.0,
        };
    }
    let wire = &mut graph.edges[0];
    wire.end_marker = crate::MarkerMetrics {
        length: 11.0,
        width: 11.0,
    };
    let options = Options::default();
    let solve = |graph: &Graph| {
        crate::card_grid::layout_with_control(graph, &options, &mut NoopWorkControl).unwrap()
    };
    let unlabelled = solve(&graph);
    let size = Size {
        width: 184.0,
        height: 78.0,
    };
    graph.edges[0].label = Some(size);
    let drawing = solve(&graph);
    assert_eq!(drawing.bounds.width, unlabelled.bounds.width);
    let label = Bounds::centered(drawing.edges[0].label_center.unwrap(), size);
    let [upper, lower] = drawing.nodes.as_slice() else {
        panic!("expected two cards")
    };
    assert_eq!(upper.x, lower.x);
    assert!(label.y[0] > upper.y + upper.height && label.y[1] < lower.y);
    assert!(label.y[1] <= lower.y - graph.edges[0].end_marker.length);
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

fn fan(hub: Side, leaf: Side, incoming: bool) -> Graph {
    Graph {
        nodes: (0..7).map(|i| node(i, None)).collect(),
        edges: (1..7)
            .map(|i| {
                let mut branch = if incoming {
                    edge(i, 0, leaf, hub)
                } else {
                    edge(0, i, hub, leaf)
                };
                let port = PortSideHint::FixedOffset {
                    side: hub,
                    offset: 4.0,
                };
                if incoming {
                    branch.target_port = port;
                } else {
                    branch.source_port = port;
                }
                branch
            })
            .collect(),
        ..Default::default()
    }
}

fn face(at: Point, rect: Rect, side: Side) -> bool {
    match side {
        Side::Top => at.y == rect.y && (rect.x..=rect.x + rect.width).contains(&at.x),
        Side::Right => {
            at.x == rect.x + rect.width && (rect.y..=rect.y + rect.height).contains(&at.y)
        }
        Side::Bottom => {
            at.y == rect.y + rect.height && (rect.x..=rect.x + rect.width).contains(&at.x)
        }
        Side::Left => at.x == rect.x && (rect.y..=rect.y + rect.height).contains(&at.y),
    }
}

#[test]
fn clear_fan_shares_one_guarded_crossbar_on_all_four_faces() {
    for (hub, leaf) in [
        (Side::Right, Side::Left),
        (Side::Left, Side::Right),
        (Side::Bottom, Side::Top),
        (Side::Top, Side::Bottom),
    ] {
        for incoming in [false, true] {
            let mut graph = fan(hub, leaf, incoming);
            let marker = crate::MarkerMetrics {
                length: 12.0,
                width: 10.0,
            };
            if incoming {
                graph.edges[0].start_marker = marker;
            } else {
                graph.edges[0].end_marker = marker;
            }
            let result = solve(&graph, &[]);
            let hub_center = result.nodes[0].center();
            let axis = usize::from(hub.axis() == Axis::Vertical);
            let cross = |p: Point| [p.x, p.y][1 - axis];
            let along = |p: Point| [p.x, p.y][axis];
            let mut shelf = None;
            for (index, route) in result.edges.iter().enumerate() {
                let start = route.points[0];
                let end = *route.points.last().unwrap();
                let (hub_contact, leaf_contact) =
                    if incoming { (end, start) } else { (start, end) };
                assert!(face(at(hub_contact), result.nodes[0], hub));
                assert_eq!(cross(at(hub_contact)), cross(hub_center) + 4.0);
                assert!(face(at(leaf_contact), result.nodes[index + 1], leaf));
                assert!(route.shared_bus);
                assert!(route.points.iter().any(|p| p.kind == KnotKind::GapBoundary));
                for leg in route.points.windows(2) {
                    let (a, b) = (at(leg[0]), at(leg[1]));
                    if cross(a) != cross(b) {
                        assert_eq!(along(a), along(b));
                        assert_eq!(*shelf.get_or_insert(along(a)), along(a));
                    }
                    for &node in &result.nodes {
                        assert!(!Bounds::between(a, b).overlaps_strictly(Bounds::of(node)));
                    }
                }
            }
            assert!(shelf.is_some());
            assert_eq!(result, solve(&graph, &[]));
        }
    }
}

#[test]
fn grid_comb_falls_back_atomically_when_a_branch_or_budget_rejects_it() {
    let mut graph = fan(Side::Right, Side::Left, false);
    let options = Options {
        routing: RoutingStyle::Orthogonal,
        ..Default::default()
    };
    let mut unbundled = options;
    unbundled.bus_routing = false;
    let ordinary = |graph: &Graph, alignments: &[Alignment]| {
        layout_with_control(graph, alignments, &unbundled, &mut NoopWorkControl).unwrap()
    };
    let fallback = layout_with_control(&graph, &[], &options, &mut Limit(50_000)).unwrap();
    assert_eq!(fallback, ordinary(&graph, &[]));

    let mut baseline = Limit(50_000);
    layout_with_control(&graph, &[], &unbundled, &mut baseline).unwrap();
    let required = 50_000 - baseline.0;
    let mut budget = OptionalBudget {
        limit: Limit(required),
        first_trial: None,
        cancel_on_trial: false,
    };
    assert_eq!(
        layout_with_control(&graph, &[], &options, &mut budget).unwrap(),
        fallback
    );
    assert_eq!(budget.first_trial, Some(0));

    budget = OptionalBudget {
        limit: Limit(50_000),
        first_trial: None,
        cancel_on_trial: true,
    };
    assert_eq!(
        layout_with_control(&graph, &[], &options, &mut budget),
        Err(LayoutError::Work(WorkError::Interrupted))
    );

    graph.edges.last_mut().unwrap().source_port = PortSideHint::FixedOffset {
        side: Side::Right,
        offset: -4.0,
    };
    assert_eq!(solve(&graph, &[]).edges, ordinary(&graph, &[]).edges);
    graph = fan(Side::Right, Side::Left, false);

    graph.nodes.push(node(7, None));
    let alignments = [
        Alignment {
            first: NodeId(0),
            next: NodeId(7),
            axis: Axis::Horizontal,
        },
        Alignment {
            first: NodeId(7),
            next: NodeId(1),
            axis: Axis::Horizontal,
        },
    ];
    let fallback =
        layout_with_control(&graph, &alignments, &options, &mut NoopWorkControl).unwrap();
    assert_eq!(fallback.edges, ordinary(&graph, &alignments).edges);
}

struct Limit(usize);

struct OptionalBudget {
    limit: Limit,
    first_trial: Option<usize>,
    cancel_on_trial: bool,
}

impl WorkControl for OptionalBudget {
    fn check(&mut self) -> Result<(), WorkError> {
        Ok(())
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.limit.charge(units)
    }

    fn try_charge_optional(&mut self, units: usize, headroom: usize) -> Result<bool, WorkError> {
        self.first_trial.get_or_insert(self.limit.0);
        if self.cancel_on_trial && units != 0 {
            return Err(WorkError::Interrupted);
        }
        if self.limit.0.saturating_sub(headroom) < units || self.limit.0 < headroom {
            return Ok(false);
        }
        self.charge(units)?;
        Ok(true)
    }
}

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

#[test]
fn collinear_face_nudges_are_deterministic_and_respect_work_budget() {
    // Hub node 0 has 12 outgoing edges sharing the Right face to targets stacked vertically,
    // exercising collinear port nudging and metered re-sorting.
    let mut nodes = vec![node(0, None)];
    let mut edges = Vec::new();
    for i in 1..=12 {
        nodes.push(node(i, None));
        edges.push(edge(0, i, Side::Right, Side::Left));
    }
    let graph = Graph {
        nodes,
        edges,
        ..Default::default()
    };
    let options = Options::default();
    let first = layout_with_control(&graph, &[], &options, &mut NoopWorkControl).unwrap();
    let second = layout_with_control(&graph, &[], &options, &mut NoopWorkControl).unwrap();
    assert_eq!(first.edges, second.edges);
    assert_eq!(
        layout_with_control(&graph, &[], &options, &mut Limit(40)),
        Err(LayoutError::Work(WorkError::Interrupted))
    );
}

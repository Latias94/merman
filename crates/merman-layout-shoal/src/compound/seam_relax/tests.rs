use super::*;
use crate::{
    EdgeGeometry, EdgeId, EdgeSpec, GroupGeometry, GroupSpec, MarkerMetrics, NodeId, NodeSpec,
    NoopWorkControl, Options, WorkControl, WorkError,
};

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn group(rect: Rect, parent: Option<GroupId>) -> GroupGeometry {
    GroupGeometry {
        rect,
        parent,
        title_center: None,
        title_struck: false,
        effective_direction: Default::default(),
    }
}

fn edge(axis: Axis, ports: (Side, Side), points: &[(f64, f64)]) -> EdgeGeometry {
    EdgeGeometry {
        points: points
            .iter()
            .map(|&(x, y)| RoutePoint::new(x, y, axis))
            .collect(),
        ports,
        label_center: None,
        start_label_center: None,
        end_label_center: None,
        shared_bus: false,
        corridor_clearance: 0.0,
        reversed_for_ranking: false,
    }
}

fn graph(output: &LayoutOutput, parents: [GroupId; 2]) -> Graph {
    let mut graph = Graph::default();
    for (index, slot) in output.groups.iter().enumerate() {
        let mut group = GroupSpec::new(index as u32);
        group.parent = slot.parent;
        graph.groups.push(group);
    }
    for (index, &bounds) in output.nodes.iter().enumerate() {
        let mut node = NodeSpec::new(
            Size {
                width: bounds.width,
                height: bounds.height,
            },
            index as u32,
        );
        node.parent = parents.get(index).copied();
        graph.nodes.push(node);
    }
    graph
        .edges
        .push(EdgeSpec::new(NodeId(0).into(), NodeId(1).into()));
    graph
}

fn seam(point: usize, group: u32, side: Side) -> BorderStitch {
    BorderStitch {
        edge: EdgeId(0),
        point,
        group: GroupId(group),
        side,
    }
}

#[test]
fn group_endpoint_and_child_stitch_follow_the_same_peer_order() {
    let mut graph = Graph {
        direction: crate::Direction::Right,
        ..Graph::default()
    };
    for (id, (parent, title_width)) in [
        (Some(1), 75.74),
        (Some(3), 73.92),
        (Some(3), 77.84),
        (None, 79.94),
    ]
    .into_iter()
    .enumerate()
    {
        let mut group = GroupSpec::new(7 + id as u32);
        group.parent = parent.map(GroupId);
        group.title = Some(Size {
            width: title_width,
            height: 21.0,
        });
        group.title_position = crate::TitlePosition::TopAuto;
        group.padding = crate::Insets {
            left: 8.0,
            right: 8.0,
            top: 37.0,
            bottom: 8.0,
        };
        graph.groups.push(group);
    }
    for (id, parent) in [Some(0), Some(0), Some(1), Some(2), Some(2), None, None]
        .into_iter()
        .enumerate()
    {
        let (width, height) = match id {
            5 => (171.25, 45.0),
            6 => (65.45, 65.45),
            _ => (152.0, 45.0),
        };
        let mut node = NodeSpec::new(Size { width, height }, id as u32);
        node.parent = parent.map(GroupId);
        graph.nodes.push(node);
    }
    let node = |id| Endpoint::Node(NodeId(id));
    let group = |id| Endpoint::Group(GroupId(id));
    for (from, to, label) in [
        (node(0), node(1), None),
        (node(1), node(2), None),
        (node(3), node(4), None),
        (group(0), group(2), Some(57.4)),
        (node(2), node(4), Some(60.62)),
        (node(5), group(3), None),
        (group(2), node(6), None),
    ] {
        let mut wire = EdgeSpec::new(from, to);
        wire.end_marker = MarkerMetrics {
            length: 4.0,
            width: 8.0,
        };
        wire.label = label.map(|width| Size {
            width,
            height: 21.0,
        });
        graph.edges.push(wire);
    }

    let solved = crate::layout(&graph, &Options::default()).unwrap();
    let child = solved
        .border_stitches
        .iter()
        .find(|s| s.edge == EdgeId(1))
        .unwrap();
    let transit = &solved.edges[1].points;
    let direct = &solved.edges[3].points;
    assert_eq!(direct[0].x, transit[child.point].x);
    assert!(
        transit[child.point].y < direct[0].y,
        "{transit:?}; {direct:?}"
    );
    assert_eq!(solved.stats.routed_crossings, 0);

    graph.edges[3].source_port = PortSideHint::Side(Side::Right);
    let pinned = crate::layout(&graph, &Options::default()).unwrap();
    assert_eq!(pinned.stats.routed_crossings, 1);
    assert_eq!(pinned.groups, solved.groups);
    assert_eq!(pinned.border_stitches, solved.border_stitches);
    for (index, (a, b)) in pinned.edges.iter().zip(&solved.edges).enumerate() {
        if index == 3 {
            assert_eq!(a.ports, b.ports);
            assert_eq!(a.label_center, b.label_center);
            assert_eq!(a.points[2..], b.points[2..]);
        } else {
            assert_eq!(a, b);
        }
    }
    graph.edges[3].source_port = PortSideHint::FixedOffset {
        side: Side::Right,
        offset: -1.5,
    };
    let fixed = crate::layout(&graph, &Options::default()).unwrap();
    assert_eq!(fixed.edges[3].ports.0, Side::Right);
    assert_eq!(
        fixed.edges[3].points[0].y,
        fixed.groups[0].rect.center().y - 1.5
    );
    assert_eq!(fixed.stats.routed_crossings, 1);
}

fn run(
    graph: &Graph,
    options: &Options,
    drawing: &mut LayoutOutput,
    control: &mut impl WorkControl,
) {
    let mut work = Work::new(control);
    work.charge(2_000).unwrap();
    relax(graph, options, drawing, &mut work).unwrap();
}

fn aligned_ports() -> (Graph, LayoutOutput) {
    let output = LayoutOutput {
        nodes: vec![
            rect(677., 61.75, 152., 45.),
            rect(8., 61.75, 152., 45.),
            rect(217., 37., 152., 45.),
            rect(475., 37., 152., 45.),
        ],
        groups: vec![
            group(rect(0., 0., 397., 117.), None),
            group(rect(447., 0., 390., 117.), None),
        ],
        edges: vec![edge(
            Axis::Horizontal,
            (Side::Left, Side::Right),
            &[
                (677., 92.25),
                (655., 92.25),
                (639., 109.),
                (627., 109.),
                (455., 109.),
                (451., 109.),
                (451., 105.),
                (447., 105.),
                (397., 105.),
                (393., 105.),
                (393., 109.),
                (389., 109.),
                (217., 109.),
                (195., 109.),
                (183., 92.25),
                (160., 92.25),
            ],
        )],
        border_stitches: vec![seam(7, 1, Side::Left), seam(8, 0, Side::Right)],
        ..Default::default()
    };
    let mut graph = graph(&output, [GroupId(1), GroupId(0)]);
    graph.edges[0].end_marker = MarkerMetrics {
        width: 12.,
        length: 10.,
    };
    (graph, output)
}

fn offset_ports() -> (Graph, LayoutOutput) {
    let output = LayoutOutput {
        nodes: vec![rect(72., 461., 40., 45.), rect(148., 661., 107., 85.)],
        groups: vec![
            group(rect(0., 0., 420., 897.), None),
            group(rect(8., 37., 404., 497.), Some(GroupId(0))),
            GroupGeometry {
                title_center: Some(Point { x: 201.5, y: 602.5 }),
                ..group(rect(108., 584., 187., 285.), Some(GroupId(0)))
            },
        ],
        edges: vec![edge(
            Axis::Vertical,
            (Side::Bottom, Side::Top),
            &[
                (92., 506.),
                (92., 526.),
                (92., 534.),
                (92., 538.),
                (92., 580.),
                (151.61, 580.),
                (151.61, 584.),
                (151.61, 619.),
                (193.5, 639.945),
                (193.5, 661.),
            ],
        )],
        border_stitches: vec![seam(2, 1, Side::Bottom), seam(6, 2, Side::Top)],
        ..Default::default()
    };
    let mut graph = graph(&output, [GroupId(1), GroupId(2)]);
    graph.groups[2].title = Some(Size {
        width: 87.78,
        height: 21.,
    });
    graph.edges[0].end_marker = MarkerMetrics {
        width: 12.,
        length: 10.,
    };
    (graph, output)
}

#[test]
fn opposing_ports_flatten_the_inboard_legs_at_the_existing_face_height() {
    let (graph, mut output) = aligned_ports();
    let old = output.edges[0].points.clone();
    run(
        &graph,
        &Options::default(),
        &mut output,
        &mut NoopWorkControl,
    );
    let route = &output.edges[0].points;
    assert_eq!(&route[..2], &old[..2]);
    assert_eq!(&route[route.len() - 2..], &old[old.len() - 2..]);
    assert_eq!(
        route[2..6].iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(),
        [(639., 105.), (447., 105.), (397., 105.), (195., 105.)]
    );
    assert_eq!(
        output.border_stitches,
        [seam(3, 1, Side::Left), seam(4, 0, Side::Right)]
    );
}

#[test]
fn offset_ports_replace_only_the_outer_step_and_keep_the_title_runway() {
    let (graph, mut output) = offset_ports();
    let old = output.clone();
    run(
        &graph,
        &Options::default(),
        &mut output,
        &mut NoopWorkControl,
    );
    assert_eq!(&output.edges[0].points[..3], &old.edges[0].points[..3]);
    assert_eq!(&output.edges[0].points[3..], &old.edges[0].points[6..]);
    assert_eq!(output.groups, old.groups);
    assert_eq!(
        output.border_stitches,
        [seam(2, 1, Side::Bottom), seam(3, 2, Side::Top)]
    );
}

fn close_ports() -> (Graph, LayoutOutput) {
    let (graph, mut output) = aligned_ports();
    for group in &mut output.groups {
        group.rect.height = 135.;
    }
    let points = &mut output.edges[0].points;
    points[8].y = 101.;
    points[9].y = 101.;
    points.splice(
        8..8,
        [
            RoutePoint::new(422., 105., Axis::Horizontal),
            RoutePoint::new(422., 101., Axis::Horizontal),
        ],
    );
    output.border_stitches[1].point += 2;
    (graph, output)
}

#[test]
fn polyline_sibling_ports_share_the_inboard_lane() {
    let (graph, mut output) = close_ports();
    let options = Options {
        routing: RoutingStyle::Polyline,
        ..Options::default()
    };
    run(&graph, &options, &mut output, &mut NoopWorkControl);
    let points = &output.edges[0].points;
    let [first, last] = output.border_stitches.as_slice() else {
        panic!("expected sibling stitches")
    };
    assert_eq!(
        (points[first.point].y, points[last.point].y),
        (109.0, 109.0)
    );
}

#[test]
fn orthogonal_sibling_ports_share_a_lane_with_orthogonal_inboard_leads() {
    let (graph, mut output) = close_ports();
    let points = &mut output.edges[0].points;
    let mut first = points[1];
    first.x = points[2].x;
    points.insert(2, first);
    let end = points.len() - 2;
    let mut last = points[end];
    last.y = points[end - 1].y;
    last.axis = Axis::Horizontal;
    points.insert(end, last);
    points[3].axis = Axis::Vertical;
    points[end + 1].axis = Axis::Vertical;
    for stitch in &mut output.border_stitches {
        stitch.point += 1;
    }
    let options = Options {
        routing: RoutingStyle::Orthogonal,
        ..Options::default()
    };
    run(&graph, &options, &mut output, &mut NoopWorkControl);
    let points = &output.edges[0].points;
    let [first, last] = output.border_stitches.as_slice() else {
        panic!("expected sibling stitches")
    };
    assert_eq!((points[first.point].y, points[last.point].y), (109., 109.));
    for pair in points.windows(2) {
        assert!(
            pair[0].x == pair[1].x || pair[0].y == pair[1].y,
            "{points:?}"
        );
    }
}

#[test]
fn close_ports_share_the_inboard_lane_without_moving_either_terminal() {
    let (graph, mut output) = close_ports();
    let old = output.edges[0].points.clone();
    run(
        &graph,
        &Options::default(),
        &mut output,
        &mut NoopWorkControl,
    );
    let points = &output.edges[0].points;
    assert_eq!(&points[..2], &old[..2]);
    assert_eq!(&points[points.len() - 2..], &old[old.len() - 2..]);
    assert_eq!(
        points[2..6].iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(),
        [(639., 109.), (447., 109.), (397., 109.), (195., 109.)]
    );
    assert_eq!(
        output.border_stitches,
        [seam(3, 1, Side::Left), seam(4, 0, Side::Right)]
    );
}

#[test]
fn sliding_keeps_the_title_clearance_reserved_by_the_local_solver() {
    let (mut graph, mut drawing) = close_ports();
    for (index, x) in [(4, 477.), (5, 467.), (6, 467.)] {
        drawing.edges[0].points[index].x = x;
    }
    drawing.groups[1].title_center = Some(Point { x: 459., y: 111.25 });
    graph.groups[1].title = Some(Size {
        width: 8.,
        height: 4.,
    });
    run(
        &graph,
        &Options::default(),
        &mut drawing,
        &mut NoopWorkControl,
    );
    let [source, target] = drawing.border_stitches.as_slice() else {
        panic!("expected both sibling contacts");
    };
    assert_eq!(drawing.edges[0].points[source.point].y, 105.);
    assert_eq!(drawing.edges[0].points[target.point].y, 101.);
}

#[test]
fn a_collapsible_anchor_keeps_the_fixed_seam_fallback() {
    let (graph, mut output) = close_ports();
    let points = &mut output.edges[0].points;
    for point in &mut points[2..7] {
        point.y = 105.;
    }
    points[6].x = 449.;
    let mut protected = output.clone();
    protected.edges[0].points[5].kind = KnotKind::GapBoundary;
    for (mut drawing, expected) in [(output, [105., 101.]), (protected, [107., 107.])] {
        run(
            &graph,
            &Options::default(),
            &mut drawing,
            &mut NoopWorkControl,
        );
        assert_eq!(
            drawing
                .border_stitches
                .iter()
                .map(|seam| drawing.edges[0].points[seam.point].y)
                .collect::<Vec<_>>(),
            expected,
        );
    }
}

#[test]
fn a_group_terminal_on_the_same_face_keeps_its_existing_pitch() {
    let (mut graph, mut output) = close_ports();
    let mut neighbor = EdgeSpec::new(GroupId(1).into(), NodeId(1).into());
    neighbor.source_port = PortSideHint::FixedOffset {
        side: Side::Left,
        offset: 47.5,
    };
    graph.edges.push(neighbor);
    output.edges.push(edge(
        Axis::Horizontal,
        (Side::Left, Side::Right),
        &[(447., 115.), (187., 115.)],
    ));
    let old = output.edges[1].clone();
    run(
        &graph,
        &Options::default(),
        &mut output,
        &mut NoopWorkControl,
    );
    for stitch in output.border_stitches {
        assert_eq!(output.edges[0].points[stitch.point].y, 107.);
    }
    assert_eq!(output.edges[1], old);
}

#[test]
fn an_accepted_sweep_reserves_space_outside_its_old_edge_bounds() {
    let (mut graph, mut output) = offset_ports();
    graph
        .edges
        .push(EdgeSpec::new(NodeId(0).into(), NodeId(1).into()));
    output.edges.push(edge(
        Axis::Horizontal,
        (Side::Left, Side::Right),
        &[(85., 630.), (83., 630.)],
    ));
    let mut control = NoopWorkControl;
    let mut work = Work::new(&mut control);
    let obstacles = Obstacles::new(&graph, &output, &mut work).unwrap();
    let bounds = Bounds::of(rect(92., 534., 59.61, 50.));
    let edit = Edit {
        edge: 1,
        stitch: 2,
        point: 3,
        route: Vec::new(),
        footprint: Bounds::of(rect(83., 560., 30., 70.)),
    };
    assert!(
        obstacles
            .clear(
                bounds,
                0,
                [GroupId(1), GroupId(2)],
                &output,
                std::iter::empty(),
                &mut work
            )
            .unwrap()
    );
    assert!(
        !obstacles
            .clear(
                bounds,
                0,
                [GroupId(1), GroupId(2)],
                &output,
                [(edit.edge, edit.footprint)].into_iter(),
                &mut work
            )
            .unwrap()
    );
}

struct OptionalBudget {
    remaining: usize,
    comparisons: Vec<usize>,
}

impl OptionalBudget {
    fn new(remaining: usize) -> Self {
        Self {
            remaining,
            comparisons: Vec::new(),
        }
    }
}

impl WorkControl for OptionalBudget {
    fn check(&mut self) -> Result<(), WorkError> {
        Ok(())
    }

    fn charge(&mut self, _units: usize) -> Result<(), WorkError> {
        Ok(())
    }

    fn try_charge_optional(&mut self, units: usize, _headroom: usize) -> Result<bool, WorkError> {
        if units == 0 {
            self.comparisons.push(self.remaining);
        }
        let Some(remaining) = self.remaining.checked_sub(units) else {
            return Ok(false);
        };
        self.remaining = remaining;
        Ok(true)
    }
}

#[test]
fn budget_decline_discards_a_partially_prepared_edit() {
    let (graph, old) = close_ports();
    let mut drawing = old.clone();
    let mut control = OptionalBudget::new(2_000);
    run(&graph, &Options::default(), &mut drawing, &mut control);
    assert_ne!(drawing, old);
    let [first, second] = control.comparisons.as_slice() else {
        panic!("expected both optional comparisons");
    };
    let fixed_cost = first - second;
    let slid_cost = second - control.remaining;
    assert!(slid_cost > 1);
    let mut drawing = old.clone();
    run(
        &graph,
        &Options::default(),
        &mut drawing,
        &mut OptionalBudget::new(fixed_cost - 1),
    );
    assert_eq!(drawing, old);

    let mut drawing = old;
    run(
        &graph,
        &Options::default(),
        &mut drawing,
        &mut OptionalBudget::new(fixed_cost + slid_cost - 1),
    );
    assert_eq!(
        drawing
            .border_stitches
            .iter()
            .map(|s| drawing.edges[0].points[s.point].y)
            .collect::<Vec<_>>(),
        [105., 101.]
    );
}

#[test]
fn existing_pins_markers_and_foreign_edge_occurrences_block_local_edits() {
    let (graph, output) = offset_ports();
    for case in 0..6 {
        let mut graph = graph.clone();
        let mut drawing = output.clone();
        let mut options = Options::default();
        match case {
            0 => {
                graph.edges[0].source_port = PortSideHint::FixedOffset {
                    side: Side::Bottom,
                    offset: 0.,
                };
            }
            1 => drawing.edges[0].points[4].kind = KnotKind::GapBoundary,
            2 => options.routing = RoutingStyle::Orthogonal,
            3 => {
                graph
                    .edges
                    .push(EdgeSpec::new(NodeId(0).into(), NodeId(1).into()));
                drawing.edges.push(edge(
                    Axis::Horizontal,
                    (Side::Right, Side::Left),
                    &[(80., 560.), (180., 560.)],
                ));
            }
            4 => {
                let mut spec = EdgeSpec::new(NodeId(0).into(), NodeId(1).into());
                spec.end_marker = MarkerMetrics {
                    width: 20.,
                    length: 10.,
                };
                graph.edges.push(spec);
                drawing.edges.push(edge(
                    Axis::Horizontal,
                    (Side::Left, Side::Right),
                    &[(170., 560.), (155., 560.)],
                ));
            }
            _ => {
                graph.groups[0].title = Some(Size {
                    width: 30.,
                    height: 10.,
                });
                drawing.groups[0].title_center = Some(Point { x: 115., y: 560. });
            }
        }
        let expected = drawing.clone();
        run(&graph, &options, &mut drawing, &mut NoopWorkControl);
        assert_eq!(drawing, expected, "case {case}");
    }
}

fn parallel_ports(order: [usize; 2], offsets: [f64; 2]) -> (Graph, LayoutOutput, Options) {
    let pairs = order.map(|id| {
        let lane = 100. + 32. * id as f64;
        let track = lane + offsets[id];
        edge(
            Axis::Horizontal,
            (Side::Right, Side::Left),
            &[
                (30., lane + 15.),
                (40., lane + 15.),
                (55., lane + 15.),
                (70., track),
                (80., lane),
                (130., lane),
                (140., track),
                (155., lane + 15.),
                (170., lane + 15.),
                (190., lane + 15.),
            ],
        )
    });
    let output = LayoutOutput {
        nodes: vec![
            rect(10., 110., 20., 10.),
            rect(190., 110., 20., 10.),
            rect(10., 142., 20., 10.),
            rect(190., 142., 20., 10.),
        ],
        groups: vec![
            group(rect(0., 0., 80., 220.), None),
            group(rect(130., 0., 90., 220.), None),
        ],
        edges: pairs.into(),
        border_stitches: vec![
            seam(4, 0, Side::Right),
            seam(5, 1, Side::Left),
            BorderStitch {
                edge: EdgeId(1),
                ..seam(4, 0, Side::Right)
            },
            BorderStitch {
                edge: EdgeId(1),
                ..seam(5, 1, Side::Left)
            },
        ],
        ..Default::default()
    };
    let mut model = graph(&output, [GroupId(0), GroupId(1)]);
    model.nodes[2].parent = Some(GroupId(0));
    model.nodes[3].parent = Some(GroupId(1));
    model.edges = order
        .map(|id| {
            EdgeSpec::new(
                NodeId((id * 2) as u32).into(),
                NodeId((id * 2 + 1) as u32).into(),
            )
        })
        .into();
    let mut options = Options::default();
    options.spacing.min_port_pitch = 32.;
    (model, output, options)
}

fn corridor_lanes(drawing: &LayoutOutput, order: [usize; 2]) -> [[f64; 2]; 2] {
    let mut lanes = [[0.; 2]; 2];
    for stitch in &drawing.border_stitches {
        let edge = stitch.edge.0 as usize;
        lanes[order[edge]][stitch.group.0 as usize] = drawing.edges[edge].points[stitch.point].y;
    }
    lanes
}

#[test]
fn parallel_seams_project_together_in_either_occurrence_order() {
    for order in [[0, 1], [1, 0]] {
        for offsets in [[5., 5.], [7., 3.]] {
            let (graph, mut drawing, options) = parallel_ports(order, offsets);
            run(&graph, &options, &mut drawing, &mut NoopWorkControl);
            assert_eq!(corridor_lanes(&drawing, order), [[105.; 2], [137.; 2]]);
        }
    }
}

#[test]
fn opposite_semantic_directions_still_respect_the_tighter_face() {
    for order in [[0, 1], [1, 0]] {
        let (mut graph, mut drawing, options) = parallel_ports(order, [7., 3.]);
        let upper = order.iter().position(|&id| id == 1).unwrap();
        let edge = &mut drawing.edges[upper];
        edge.points[4].y = 129.;
        edge.points.reverse();
        edge.ports = (edge.ports.1, edge.ports.0);
        let spec = &mut graph.edges[upper];
        std::mem::swap(&mut spec.source, &mut spec.target);
        let seams = &mut drawing.border_stitches[2 * upper..2 * upper + 2];
        seams.swap(0, 1);
        for seam in seams {
            seam.point = edge.points.len() - 1 - seam.point;
        }
        run(&graph, &options, &mut drawing, &mut NoopWorkControl);
        assert_eq!(corridor_lanes(&drawing, order), [[105.; 2], [137.; 2]]);
    }
}

#[test]
fn rejecting_one_member_keeps_the_pitch_of_the_original_batch() {
    for order in [[0, 1], [1, 0]] {
        let (mut graph, mut drawing, options) = parallel_ports(order, [5., 5.]);
        let upper = order.iter().position(|&id| id == 1).unwrap();
        graph.edges[upper].label = Some(Size {
            width: 4.,
            height: 2.,
        });
        drawing.edges[upper].label_center = Some(Point { x: 105., y: 137. });
        let original = drawing.clone();
        run(&graph, &options, &mut drawing, &mut NoopWorkControl);
        assert_eq!(drawing, original);
    }
}

#[test]
fn budget_decline_keeps_both_members_of_a_corridor_at_their_original_ports() {
    let (graph, original, options) = parallel_ports([0, 1], [5., 5.]);
    let mut drawing = original.clone();
    let mut budget = OptionalBudget::new(2_000);
    run(&graph, &options, &mut drawing, &mut budget);
    assert_eq!(corridor_lanes(&drawing, [0, 1]), [[105.; 2], [137.; 2]]);
    let mut drawing = original.clone();
    run(
        &graph,
        &options,
        &mut drawing,
        &mut OptionalBudget::new(2_000 - budget.remaining - 1),
    );
    assert_eq!(drawing, original);
}

#[test]
fn face_order_protects_mid_edge_labels_on_moved_prefix() {
    // Group 0 sits above Group 1; edge 0 (stitched) exits bottom face at x=80 and goes straight
    // down, while edge 1 (group-to-group, movable) exits bottom face at x=40 and bends right to
    // x=120 — inverted on the face. Placing a mid-edge label on edge 1's moved prefix must block
    // `face_order` from detaching the route from its label.
    let make_scene = |with_label: bool| {
        let mut movable = edge(
            Axis::Vertical,
            (Side::Bottom, Side::Top),
            &[(40., 100.), (40., 140.), (120., 160.), (120., 200.)],
        );
        movable.label_center = with_label.then_some(Point { x: 80., y: 150. });
        let output = LayoutOutput {
            nodes: vec![rect(70., 20., 20., 20.), rect(70., 220., 20., 20.)],
            groups: vec![
                group(rect(0., 0., 180., 100.), None),
                group(rect(0., 200., 180., 100.), None),
            ],
            edges: vec![
                edge(
                    Axis::Vertical,
                    (Side::Bottom, Side::Top),
                    &[(80., 40.), (80., 100.), (80., 200.), (80., 220.)],
                ),
                movable,
            ],
            border_stitches: vec![
                BorderStitch {
                    edge: EdgeId(0),
                    group: GroupId(0),
                    side: Side::Bottom,
                    point: 1,
                },
                BorderStitch {
                    edge: EdgeId(0),
                    group: GroupId(1),
                    side: Side::Top,
                    point: 2,
                },
            ],
            stats: crate::LayoutStats {
                routed_crossings: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut model = graph(&output, [GroupId(0), GroupId(1)]);
        let mut movable_spec = EdgeSpec::new(GroupId(0).into(), GroupId(1).into());
        movable_spec.label = with_label.then_some(Size {
            width: 24.,
            height: 12.,
        });
        model.edges = vec![
            EdgeSpec::new(NodeId(0).into(), NodeId(1).into()),
            movable_spec,
        ];
        (model, output)
    };

    let options = Options::default();
    let (model_unlabeled, mut drawing_unlabeled) = make_scene(false);
    let orig_unlabeled = drawing_unlabeled.edges[1].points[0].x;
    run(
        &model_unlabeled,
        &options,
        &mut drawing_unlabeled,
        &mut NoopWorkControl,
    );
    assert_eq!(
        drawing_unlabeled.edges[1].points[0].x, 120.,
        "unlabeled movable edge should straighten to lane 120 (was {orig_unlabeled})"
    );

    let (model_labeled, mut drawing_labeled) = make_scene(true);
    let original = drawing_labeled.clone();
    run(
        &model_labeled,
        &options,
        &mut drawing_labeled,
        &mut NoopWorkControl,
    );
    assert_eq!(
        drawing_labeled.edges[1].points, original.edges[1].points,
        "mid-edge label on moved prefix must prevent face_order from detaching route"
    );
}

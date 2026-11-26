use merman_layout_shoal::{
    Axis, Direction, EdgeSpec, Endpoint, Graph, GroupId, GroupSpec, Insets, LayoutError,
    LayoutOutput, LayoutScratch, MarkerMetrics, NodeId, NodeSpec, Options, PackingPolicy,
    PortSideHint, Rect, RoutePoint, RoutingStyle, Side, Size, WidthPolicy, WorkControl, WorkError,
    layout, layout_with_control,
};

fn size(width: f64, height: f64) -> Size {
    Size { width, height }
}

fn graph(count: u32, measured: Size) -> Graph {
    let mut graph = Graph::default();
    graph.nodes = (0..count)
        .map(|sequence| NodeSpec::new(measured, sequence))
        .collect();
    graph
}

fn edge(from: u32, to: u32) -> EdgeSpec {
    EdgeSpec::new(NodeId(from).into(), NodeId(to).into())
}

fn fit(max_width: f64, allow_turn: bool, fold_wide_ranks: bool) -> WidthPolicy {
    WidthPolicy::FitWidth {
        max_width,
        allow_turn,
        turn_breakpoint: 640.0,
        fold_wide_ranks,
    }
}

#[test]
fn connected_components_wrap_before_spacing_changes() {
    let mut graph = graph(4, size(70.0, 40.0));
    graph.edges = vec![edge(1, 0), edge(3, 2)];
    graph.nodes[0].seq = 3;
    graph.nodes[3].seq = 0;
    let mut options = Options::default();
    options.packing = PackingPolicy::Strip;
    let natural = layout(&graph, &options).unwrap();
    assert!(natural.bounds.width > 100.0);

    options.width_policy = fit(100.0, false, false);
    let fitted = layout(&graph, &options).unwrap();
    assert_eq!(fitted.bounds.width, 70.0);
    assert_eq!(
        fitted.nodes[0].y - fitted.nodes[1].y,
        40.0 + options.spacing.rank
    );
    assert_eq!(
        fitted.nodes[2].y - fitted.nodes[3].y,
        40.0 + options.spacing.rank
    );
    assert!(fitted.node_ranks.is_empty());

    options.capture_node_ranks = true;
    let mut ranked = layout(&graph, &options).unwrap();
    assert_eq!(ranked.node_ranks, [1, 0, 1, 0]);
    ranked.node_ranks.clear();
    assert_eq!(ranked, fitted);
}

#[test]
fn compaction_scales_spacing_and_only_the_plain_part_of_group_padding() {
    let mut fork = graph(3, size(100.0, 40.0));
    fork.edges = vec![edge(0, 1), edge(0, 2)];
    let mut options = Options::default();
    options.width_policy = fit(230.0, false, false);
    let compact = layout(&fork, &options).unwrap();
    assert_eq!(
        compact.bounds,
        size(
            200.0 + options.spacing.node * 0.5,
            80.0 + options.spacing.rank * 0.75
        )
    );
    assert_eq!(
        compact.nodes[1].y - compact.nodes[0].y,
        40.0 + options.spacing.rank * 0.75
    );

    let mut grouped = graph(1, size(100.0, 40.0));
    let mut group = GroupSpec::new(0);
    group.title = Some(size(80.0, 30.0));
    group.padding = Insets {
        top: 50.0,
        right: 40.0,
        bottom: 20.0,
        left: 40.0,
    };
    grouped.groups.push(group);
    grouped.nodes[0].parent = Some(GroupId(0));
    options.width_policy = fit(160.0, false, false);
    let compact = layout(&grouped, &options).unwrap();
    assert_eq!(compact.bounds, size(148.0, 94.0));
    assert_eq!(compact.nodes[0].y, 30.0 + 20.0 * 0.6);
    assert_eq!(compact.groups[0].title_center.unwrap().y, 21.0);
}

#[test]
fn narrow_horizontal_flow_turns_without_rotating_boxes_or_physical_ports() {
    for direction in [Direction::Right, Direction::Left] {
        let mut graph = graph(4, size(140.0, 36.0));
        graph.direction = direction;
        graph.edges = vec![edge(0, 1), edge(1, 2), edge(2, 3)];
        graph.edges[0].source_port = PortSideHint::FixedOffset {
            side: Side::Bottom,
            offset: 22.0,
        };
        graph.edges[0].target_port = PortSideHint::FixedOffset {
            side: Side::Top,
            offset: -16.0,
        };
        let mut options = Options::default();
        options.width_policy = fit(240.0, true, false);
        let drawing = layout(&graph, &options).unwrap();
        assert_eq!(drawing.direction, Direction::Down);
        assert!(drawing.bounds.width <= 240.0);
        assert!(
            drawing
                .nodes
                .iter()
                .all(|node| { (node.width, node.height) == (140.0, 36.0) })
        );
        let edge = &drawing.edges[0];
        assert_eq!(edge.ports, (Side::Bottom, Side::Top));
        assert_eq!(edge.points[0].x, drawing.nodes[0].center().x + 22.0);
        assert_eq!(
            edge.points.last().unwrap().x,
            drawing.nodes[1].center().x - 16.0
        );

        options.width_policy = fit(240.0, false, false);
        let unturned = layout(&graph, &options).unwrap();
        assert_eq!(unturned.direction, direction);
        assert!(unturned.bounds.width > 240.0);

        options.width_policy = WidthPolicy::FitWidth {
            max_width: 240.0,
            allow_turn: true,
            turn_breakpoint: 240.0,
            fold_wide_ranks: false,
        };
        assert_eq!(layout(&graph, &options).unwrap(), unturned);
    }
}

#[test]
fn horizontal_turn_preserves_explicit_crossed_group_direction_and_stitches() {
    let mut graph = graph(7, size(120.0, 36.0));
    graph.direction = Direction::Left;
    for node in &mut graph.nodes[..2] {
        node.parent = Some(GroupId(0));
    }
    let mut group = GroupSpec::new(0);
    group.direction = Some(Direction::Up);
    group.padding = Insets {
        top: 12.0,
        right: 12.0,
        bottom: 12.0,
        left: 12.0,
    };
    graph.groups.push(group);
    graph.edges = (0..6).map(|from| edge(from, from + 1)).collect();
    let mut options = Options::default();
    options.width_policy = fit(415.0, true, false);
    let turned = layout(&graph, &options).unwrap();
    assert_eq!(turned.direction, Direction::Down);
    assert_eq!(turned.groups[0].effective_direction, Direction::Up);
    assert!(turned.nodes[0].y > turned.nodes[1].y);
    assert_eq!(turned.border_stitches.len(), 1);
    let stitch = turned.border_stitches[0];
    let knot = turned.edges[stitch.edge.0 as usize].points[stitch.point];
    let group = turned.groups[0].rect;
    assert!((group.x..=group.x + group.width).contains(&knot.x));
    assert_eq!(knot.y, group.y + group.height);
}

#[test]
fn horizontal_turn_uses_the_measured_perpendicular_port_span() {
    let mut graph = graph(6, size(96.0, 60.0));
    graph.direction = Direction::Right;
    graph.nodes[0].port_span = Some(48.0);
    graph.nodes[0].perpendicular_port_span = Some(0.0);
    graph.edges = vec![
        edge(0, 1),
        edge(0, 2),
        edge(1, 3),
        edge(2, 3),
        edge(3, 4),
        edge(4, 5),
    ];
    let mut options = Options::default();
    options.width_policy = fit(450.0, true, false);
    let turned = layout(&graph, &options).unwrap();
    assert_eq!(turned.direction, Direction::Down);
    for edge in &turned.edges[..2] {
        assert_eq!(edge.ports.0, Side::Bottom);
        assert_eq!(edge.points[0].x, turned.nodes[0].center().x);
    }
}

#[test]
fn horizontal_turn_rotates_flow_relative_bars_and_keeps_their_ports_apart() {
    let mut graph = graph(6, size(96.0, 60.0));
    graph.direction = Direction::Right;
    for id in [0, 3] {
        let bar = &mut graph.nodes[id];
        bar.size = size(4.0, 72.0);
        bar.flow_relative_axis = Some(Axis::Horizontal);
        bar.port_span = Some(48.0);
        bar.perpendicular_port_span = Some(0.0);
    }
    graph.edges = vec![
        edge(0, 1),
        edge(0, 2),
        edge(1, 3),
        edge(2, 3),
        edge(3, 4),
        edge(4, 5),
    ];
    let mut options = Options::default();
    options.width_policy = fit(400.0, true, false);
    let turned = layout(&graph, &options).unwrap();
    assert_eq!(turned.direction, Direction::Down);
    assert_eq!((turned.nodes[0].width, turned.nodes[0].height), (72.0, 4.0));
    assert_eq!(
        (turned.nodes[1].width, turned.nodes[1].height),
        (96.0, 60.0)
    );
    assert_eq!((turned.nodes[3].width, turned.nodes[3].height), (72.0, 4.0));
    assert_eq!(turned.edges[0].ports.0, Side::Bottom);
    assert_eq!(turned.edges[1].ports.0, Side::Bottom);
    assert!((turned.edges[0].points[0].x - turned.edges[1].points[0].x).abs() >= 16.0);
    assert_eq!(turned.edges[2].ports.1, Side::Top);
    assert_eq!(turned.edges[3].ports.1, Side::Top);
    let first = turned.edges[2].points.last().unwrap();
    let second = turned.edges[3].points.last().unwrap();
    assert!((first.x - second.x).abs() >= 16.0);
}

fn enters(rect: Rect, from: RoutePoint, to: RoutePoint) -> bool {
    let (mut near, mut far) = (0.0_f64, 1.0_f64);
    for (start, end, low, high) in [
        (from.x, to.x, rect.x, rect.x + rect.width),
        (from.y, to.y, rect.y, rect.y + rect.height),
    ] {
        let (low, high) = (low + 1e-8, high - 1e-8);
        if start == end {
            if start <= low || start >= high {
                return false;
            }
        } else {
            let first = (low - start) / (end - start);
            let last = (high - start) / (end - start);
            near = near.max(first.min(last));
            far = far.min(first.max(last));
        }
    }
    near < far
}

fn overlaps(left: Rect, right: Rect) -> bool {
    left.x.max(right.x) + 1e-8 < (left.x + left.width).min(right.x + right.width)
        && left.y.max(right.y) + 1e-8 < (left.y + left.height).min(right.y + right.height)
}

fn marker_rect(contact: RoutePoint, side: Side, marker: MarkerMetrics) -> Rect {
    match side {
        Side::Top | Side::Bottom => Rect {
            x: contact.x - marker.width * 0.5,
            y: if side == Side::Top {
                contact.y - marker.length
            } else {
                contact.y
            },
            width: marker.width,
            height: marker.length,
        },
        Side::Left | Side::Right => Rect {
            x: if side == Side::Left {
                contact.x - marker.length
            } else {
                contact.x
            },
            y: contact.y - marker.width * 0.5,
            width: marker.length,
            height: marker.width,
        },
    }
}

fn check_boxes_and_markers(graph: &Graph, drawing: &LayoutOutput) {
    for (index, &node) in drawing.nodes.iter().enumerate() {
        assert!(
            drawing.nodes[..index]
                .iter()
                .all(|&other| !overlaps(node, other))
        );
    }
    for (index, (spec, route)) in graph.edges.iter().zip(&drawing.edges).enumerate() {
        let [Endpoint::Node(start), Endpoint::Node(end)] = [spec.source, spec.target] else {
            unreachable!();
        };
        let markers = [
            marker_rect(route.points[0], route.ports.0, spec.start_marker),
            marker_rect(
                *route.points.last().unwrap(),
                route.ports.1,
                spec.end_marker,
            ),
        ];
        for (id, &node) in drawing.nodes.iter().enumerate() {
            assert!(
                route
                    .points
                    .windows(2)
                    .all(|leg| !enters(node, leg[0], leg[1])),
                "route {index} enters node {id}"
            );
            if id != start.0 as usize && id != end.0 as usize {
                assert!(
                    markers
                        .into_iter()
                        .all(|footprint| !overlaps(footprint, node)),
                    "marker of route {index} overlaps node {id}"
                );
            }
        }
    }
}

fn wide_marker_components() -> Graph {
    let mut graph = graph(3, size(40.0, 30.0));
    graph.nodes[2].size.height = 100.0;
    let mut relation = edge(0, 1);
    relation.start_marker = MarkerMetrics {
        width: 180.0,
        length: 12.0,
    };
    graph.edges.push(relation);
    graph
}

#[test]
fn fit_width_wraps_an_isolate_clear_of_a_wide_start_marker() {
    let graph = wide_marker_components();
    let mut options = Options::default();
    options.packing = PackingPolicy::FitWidth { max_width: 140.0 };
    let legacy = layout(&graph, &options).unwrap();
    assert_eq!(legacy.bounds.width, 130.0);
    assert_eq!(legacy.nodes[2].x, 90.0);

    options.width_policy = fit(140.0, false, false);
    let fitted = layout(&graph, &options).unwrap();
    let route = &fitted.edges[0];
    let marker = marker_rect(route.points[0], route.ports.0, graph.edges[0].start_marker);
    assert_eq!(
        fitted.bounds.width, 180.0,
        "{marker:?}, {:?}",
        fitted.nodes[2]
    );
    assert_eq!(marker.x, 0.0);
    assert!(!overlaps(marker, fitted.nodes[2]));
    check_boxes_and_markers(&graph, &fitted);
}

#[test]
fn fit_width_includes_the_physical_marker_envelope_in_a_compound_tile() {
    let mut graph = wide_marker_components();
    graph.groups.push(GroupSpec::new(0));
    for node in &mut graph.nodes[..2] {
        node.parent = Some(GroupId(0));
    }
    let mut options = Options::default();
    options.width_policy = fit(140.0, false, false);
    let drawing = layout(&graph, &options).unwrap();
    let route = &drawing.edges[0];
    let marker = marker_rect(route.points[0], route.ports.0, graph.edges[0].start_marker);
    let group = drawing.groups[0].rect;
    assert_eq!(drawing.bounds.width, 180.0, "{marker:?}, {group:?}");
    assert_eq!((group.x, group.width), (0.0, 180.0));
    assert_eq!(marker.x, group.x);
    assert!(!overlaps(marker, drawing.nodes[2]));
}

#[test]
fn folded_width_reports_the_wide_marker_at_the_shared_hub() {
    let mut graph = graph(13, size(90.0, 40.0));
    graph.edges = (1..13)
        .map(|leaf| {
            let mut relation = edge(leaf, 0);
            relation.end_marker = MarkerMetrics {
                width: 500.0,
                length: 12.0,
            };
            relation
        })
        .collect();
    let mut options = Options::default();
    options.width_policy = fit(425.0, false, true);
    let drawing = layout(&graph, &options).unwrap();
    assert!(
        drawing.nodes[1..]
            .windows(2)
            .any(|row| row[0].y != row[1].y)
    );
    assert_eq!(drawing.bounds.width, 500.0);
    for route in &drawing.edges {
        let tip = route.points.last().unwrap();
        assert_eq!(tip.x, 250.0);
        assert_eq!(tip.y, drawing.nodes[0].y);
    }
    check_boxes_and_markers(&graph, &drawing);
}

#[test]
fn native_fan_rows_fit_with_every_leaf_and_marker_clear() {
    let mut graph = graph(250, size(152.0, 45.0));
    graph.edges = (1..250)
        .map(|leaf| {
            let mut relation = edge(leaf, 0);
            relation.start_marker = MarkerMetrics {
                length: 10.0,
                width: 8.0,
            };
            relation.end_marker = MarkerMetrics {
                length: 12.0,
                width: 10.0,
            };
            relation
        })
        .collect();
    let mut options = Options::default();
    let natural = layout(&graph, &options).unwrap();
    assert_eq!(
        natural.bounds.width,
        249.0 * 152.0 + 248.0 * options.spacing.node
    );
    options.width_policy = fit(1_200.0, false, true);
    options.capture_node_ranks = true;
    let drawing = layout(&graph, &options).unwrap();
    assert!(drawing.bounds.width <= 1_200.0, "{:?}", drawing.bounds);
    assert_eq!(drawing.nodes.len(), graph.nodes.len());
    assert_eq!(drawing.edges.len(), graph.edges.len());
    assert!(
        drawing.nodes[1..]
            .windows(2)
            .any(|row| row[0].y != row[1].y)
    );
    assert_eq!(drawing.node_ranks[0], 1);
    assert_eq!(drawing.node_ranks[1..], [0; 249]);
    assert_eq!(drawing.stats.routed_crossings, 0);
    for (index, leaf) in drawing.nodes[1..].iter().enumerate() {
        let route = &drawing.edges[index];
        let start = route.points[0];
        let next = route.points[1];
        let end = *route.points.last().unwrap();
        let prior = route.points[route.points.len() - 2];
        assert_eq!(route.ports, (Side::Bottom, Side::Top));
        assert_eq!(start.x, leaf.center().x);
        assert_eq!(start.y, leaf.y + leaf.height);
        assert_eq!(end.y, drawing.nodes[0].y);
        assert_eq!((next.x, prior.x), (start.x, end.x));
        assert!(next.y - start.y >= 10.0);
        assert!(end.y - prior.y >= 12.0);
        assert_eq!(end, *drawing.edges[0].points.last().unwrap());
    }
    check_boxes_and_markers(&graph, &drawing);
}

#[test]
fn fan_folds_beside_ordinary_labelled_cross_rank_edges() {
    let leaf_count = 42;
    let mut graph = graph(leaf_count + 6, size(90.0, 40.0));
    graph.edges = (1..=leaf_count).map(|leaf| edge(leaf, 0)).collect();
    graph.edges.extend([
        edge(leaf_count + 1, leaf_count + 2),
        edge(leaf_count + 2, 0),
        edge(leaf_count + 2, leaf_count + 3),
        edge(0, leaf_count + 4),
        edge(leaf_count + 3, leaf_count + 5),
        edge(leaf_count + 4, leaf_count + 5),
    ]);
    let ordinary_id = leaf_count as usize + 1;
    let ordinary = &mut graph.edges[ordinary_id];
    ordinary.min_rank_span = 2;
    ordinary.label = Some(size(111.0, 32.0));
    ordinary.start_label = Some(size(43.0, 15.0));
    let mut options = Options::default();
    options.width_policy = fit(540.0, false, false);
    assert!(layout(&graph, &options).unwrap().bounds.width > 540.0);

    options.width_policy = fit(540.0, false, true);
    let drawing = layout(&graph, &options).unwrap();
    assert!(drawing.bounds.width <= 540.0, "{:?}", drawing.bounds);
    assert!(
        drawing.edges[..leaf_count as usize]
            .iter()
            .all(|edge| edge.label_center.is_none())
    );
    let route = &drawing.edges[ordinary_id];
    for (at, size) in [
        (route.label_center.unwrap(), size(111.0, 32.0)),
        (route.start_label_center.unwrap(), size(43.0, 15.0)),
    ] {
        let label = Rect {
            x: at.x - size.width * 0.5,
            y: at.y - size.height * 0.5,
            width: size.width,
            height: size.height,
        };
        assert!(drawing.nodes.iter().all(|&node| !overlaps(label, node)));
        assert!(drawing.edges[..leaf_count as usize].iter().all(|edge| {
            edge.points
                .windows(2)
                .all(|leg| !enters(label, leg[0], leg[1]))
        }));
    }
    check_boxes_and_markers(&graph, &drawing);

    options.caps.max_dummies = 0;
    let capped = layout(&graph, &options).unwrap();
    assert_eq!(capped.stats.dummy_nodes, 0);
    assert!(capped.stats.direct_fallback_edges > 0);
    assert!(capped.edges[ordinary_id].label_center.is_some());
    assert!(capped.edges[ordinary_id].start_label_center.is_some());
    check_boxes_and_markers(&graph, &capped);
}

#[test]
fn fan_out_reflects_and_keeps_each_fixed_leaf_port() {
    let mut graph = graph(22, size(94.0, 34.0));
    graph.direction = Direction::Up;
    graph.edges = (1..22)
        .map(|leaf| {
            let mut spec = edge(0, leaf);
            spec.source_port = PortSideHint::FixedOffset {
                side: Side::Top,
                offset: -11.0,
            };
            spec.target_port = PortSideHint::FixedOffset {
                side: Side::Bottom,
                offset: 15.0,
            };
            spec.start_marker = MarkerMetrics {
                length: 12.0,
                width: 9.0,
            };
            spec.end_marker = spec.start_marker;
            spec
        })
        .collect();
    let mut options = Options::default();
    options.routing = RoutingStyle::Orthogonal;
    options.width_policy = fit(425.0, false, true);
    let drawing = layout(&graph, &options).unwrap();
    assert!(drawing.bounds.width <= 425.0);
    for (leaf, edge) in drawing.nodes[1..].iter().zip(&drawing.edges) {
        assert_eq!(edge.ports, (Side::Top, Side::Bottom));
        assert_eq!(edge.points[0].x, drawing.nodes[0].center().x - 11.0);
        assert!(edge.points[0].y - edge.points[1].y >= 12.0);
        let [.., prior, end] = edge.points.as_slice() else {
            unreachable!()
        };
        assert_eq!(end.x, leaf.center().x + 15.0);
        assert_eq!(prior.x, end.x);
        assert!(prior.y - end.y >= 12.0);
        assert!(
            edge.points
                .windows(2)
                .all(|leg| leg[0].x == leg[1].x || leg[0].y == leg[1].y)
        );
    }
    check_boxes_and_markers(&graph, &drawing);
}

#[test]
fn an_unfoldable_captioned_fan_reports_the_actual_overflow() {
    let mut graph = graph(14, size(94.0, 34.0));
    graph.edges = (1..14)
        .map(|leaf| {
            let mut spec = edge(leaf, 0);
            spec.label = Some(size(51.0, 13.0));
            spec
        })
        .collect();
    let mut options = Options::default();
    options.width_policy = fit(425.0, false, false);
    let regular = layout(&graph, &options).unwrap();
    options.width_policy = fit(425.0, false, true);
    let folded = layout(&graph, &options).unwrap();
    assert!(folded.bounds.width > 425.0);
    assert_eq!(folded, regular);
    assert!(
        folded
            .edges
            .iter()
            .all(|route| route.label_center.is_some())
    );
}

#[derive(Default)]
struct Meter {
    used: usize,
    limit: Option<usize>,
    cancel_at: Option<usize>,
}

impl WorkControl for Meter {
    fn check(&mut self) -> Result<(), WorkError> {
        if self.cancel_at.is_some_and(|limit| self.used >= limit) {
            return Err(WorkError::Interrupted);
        }
        Ok(())
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.used += units;
        if self.limit.is_some_and(|limit| self.used > limit) {
            return Err(WorkError::Interrupted);
        }
        Ok(())
    }
}

#[test]
fn fit_width_marker_bounds_consume_the_shared_work_budget() {
    let graph = wide_marker_components();
    let mut options = Options::default();
    options.packing = PackingPolicy::FitWidth { max_width: 500.0 };
    let mut scratch = LayoutScratch::default();
    let mut legacy = Meter::default();
    layout_with_control(&graph, &options, &mut scratch, &mut legacy).unwrap();

    options.width_policy = fit(500.0, false, false);
    let mut fit = Meter::default();
    let fitted = layout_with_control(&graph, &options, &mut scratch, &mut fit).unwrap();
    assert!(fit.used > legacy.used);
    let mut denied = Meter {
        limit: Some(legacy.used),
        ..Default::default()
    };
    assert_eq!(
        layout_with_control(&graph, &options, &mut scratch, &mut denied),
        Err(LayoutError::Work(WorkError::Interrupted)),
    );
    let mut enough = Meter {
        limit: Some(fit.used),
        ..Default::default()
    };
    assert_eq!(
        layout_with_control(&graph, &options, &mut scratch, &mut enough).unwrap(),
        fitted,
    );
    assert_eq!(enough.used, fit.used);
}

#[test]
fn width_trials_share_budget_and_cancel_without_poisoning_scratch() {
    let mut graph = graph(24, size(94.0, 34.0));
    graph.edges = (1..24).map(|leaf| edge(leaf, 0)).collect();
    let mut options = Options::default();
    options.packing = PackingPolicy::FitWidth { max_width: 425.0 };
    let mut scratch = LayoutScratch::default();
    let mut natural = Meter::default();
    layout_with_control(&graph, &options, &mut scratch, &mut natural).unwrap();

    options.width_policy = fit(425.0, false, true);
    for mut control in [
        Meter {
            limit: Some(natural.used),
            ..Default::default()
        },
        Meter {
            cancel_at: Some(natural.used),
            ..Default::default()
        },
    ] {
        assert_eq!(
            layout_with_control(&graph, &options, &mut scratch, &mut control),
            Err(LayoutError::Work(WorkError::Interrupted)),
        );
        assert!(control.used >= natural.used);
    }
    let mut retried = Meter::default();
    let drawing = layout_with_control(&graph, &options, &mut scratch, &mut retried).unwrap();
    let mut fresh = Meter::default();
    let expected =
        layout_with_control(&graph, &options, &mut LayoutScratch::default(), &mut fresh).unwrap();
    assert!(drawing.bounds.width <= 425.0);
    assert_eq!(drawing, expected);
    assert_eq!(retried.used, fresh.used);
    assert!(retried.used > natural.used);
}

#[test]
fn invalid_width_parameters_keep_grid_validation_independent() {
    let graph = graph(1, size(100.0, 40.0));
    let mut options = Options::default();
    for (max_width, turn_breakpoint) in [
        (0.0, 5.0),
        (f64::NAN, 5.0),
        (5.0, -1.0),
        (5.0, f64::INFINITY),
    ] {
        options.width_policy = WidthPolicy::FitWidth {
            max_width,
            turn_breakpoint,
            allow_turn: false,
            fold_wide_ranks: false,
        };
        assert_eq!(
            layout(&graph, &options),
            Err(LayoutError::InvalidInput("invalid width policy")),
        );
    }
    options.width_policy = fit(100.0, false, false);
    options.grid_quantum = Some(1.0);
    assert_eq!(
        layout(&graph, &options),
        Err(LayoutError::Unsupported("grid layout")),
    );
}

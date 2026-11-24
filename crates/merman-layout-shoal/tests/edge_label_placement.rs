use merman_layout_shoal::{
    Direction, EdgeSpec, Graph, LayoutError, LayoutOutput, LayoutScratch, MarkerMetrics, NodeId,
    NodeSpec, Options, Point, Rect, RoutingStyle, Size, WorkControl, WorkError, layout,
    layout_with_control,
};

const DIRECTIONS: [Direction; 4] = [
    Direction::Down,
    Direction::Up,
    Direction::Right,
    Direction::Left,
];

fn size(width: f64, height: f64) -> Size {
    Size { width, height }
}

fn graph(direction: Direction, nodes: &[Size], edges: &[(u32, u32)]) -> Graph {
    let mut graph = Graph::default();
    graph.direction = direction;
    graph.nodes = nodes
        .iter()
        .enumerate()
        .map(|(i, &size)| NodeSpec::new(size, i as u32))
        .collect();
    graph.edges = edges
        .iter()
        .map(|&(from, to)| EdgeSpec::new(NodeId(from).into(), NodeId(to).into()))
        .collect();
    graph
}

#[test]
fn a_short_measured_label_stays_centered_on_its_shaft_in_the_original_gap() {
    for direction in DIRECTIONS {
        for routing in [
            RoutingStyle::Curved,
            RoutingStyle::Orthogonal,
            RoutingStyle::Polyline,
        ] {
            let mut options = Options::default();
            options.routing = routing;
            let mut graph = graph(direction, &[size(60.0, 40.0); 2], &[(0, 1)]);
            let unlabelled = layout(&graph, &options).unwrap();
            let measured = size(23.0, 16.0);
            graph.edges[0].label = Some(measured);
            let legacy = layout(&graph, &options).unwrap();
            options.inline_labels = true;
            let inline = layout(&graph, &options).unwrap();
            assert_eq!(inline.nodes, unlabelled.nodes, "{direction:?} {routing:?}");
            assert_eq!(inline.edges[0].points, unlabelled.edges[0].points);
            assert_eq!(inline.edges[0].ports, unlabelled.edges[0].ports);
            assert_eq!(inline.stats.dummy_nodes, 0);
            assert!(legacy.stats.dummy_nodes > inline.stats.dummy_nodes);
            let edge = &inline.edges[0];
            let center = edge.label_center.unwrap();
            let label = rect(center, measured);
            let start = edge.points.first().unwrap();
            let end = edge.points.last().unwrap();
            assert!(inline.nodes.iter().all(|&node| !overlaps(node, label)));
            assert_eq!(center.x, start.x.midpoint(end.x));
            assert_eq!(center.y, start.y.midpoint(end.y));
        }
    }
}

fn rect(center: Point, size: Size) -> Rect {
    Rect {
        x: center.x - size.width * 0.5,
        y: center.y - size.height * 0.5,
        width: size.width,
        height: size.height,
    }
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height
}

fn opt_in() -> Options {
    let mut options = Options::default();
    options.inline_labels = true;
    options.capture_node_ranks = true;
    options
}

fn legacy(graph: &Graph, options: &Options) -> LayoutOutput {
    let mut options = *options;
    options.inline_labels = false;
    layout(graph, &options).unwrap()
}

#[test]
fn tall_wide_and_long_labels_keep_their_subrank_or_existing_rank() {
    let mut graph = graph(
        Direction::Down,
        &[size(60.0, 40.0); 4],
        &[(0, 1), (1, 2), (2, 3), (0, 2)],
    );
    graph.edges[0].label = Some(size(50.0, 38.0));
    graph.edges[1].label = Some(size(121.0, 16.0));
    graph.edges[3].label = Some(size(20.0, 16.0));
    let options = opt_in();
    let old = legacy(&graph, &options);
    assert_eq!(old.node_ranks, vec![0, 2, 4, 5]);
    assert_eq!(layout(&graph, &options).unwrap(), old);

    graph.nodes.truncate(2);
    graph.edges.truncate(1);
    graph.edges[0].label = Some(size(22.0, 16.0));
    graph.edges[0].min_rank_span = 7;
    assert_eq!(layout(&graph, &options).unwrap(), legacy(&graph, &options));
}

#[test]
fn separate_parallel_occurrences_and_dense_ranks_keep_their_labels_in_place() {
    for arcs in [
        &[(0, 1), (0, 1), (1, 0)][..],
        &[(0, 1), (0, 2), (0, 3), (0, 4), (0, 5)][..],
    ] {
        let mut graph = graph(Direction::Down, &[size(75.0, 44.0); 6], arcs);
        for (index, e) in graph.edges.iter_mut().enumerate() {
            e.label = Some(size(20.0 + index as f64, 17.0));
        }
        let options = opt_in();
        let old = legacy(&graph, &options);
        assert!(old.stats.dummy_nodes >= arcs.len());
        assert_eq!(layout(&graph, &options).unwrap(), old);
        assert!(old.edges.iter().all(|e| e.label_center.is_some()));
    }
}

fn blocked_by_markers(graph: &mut Graph) {
    graph.edges[0].label = Some(size(24.0, 18.0));
    graph.edges[0].start_marker = MarkerMetrics {
        length: 25.0,
        width: 90.0,
    };
    graph.edges[0].end_marker = graph.edges[0].start_marker;
}

#[test]
fn a_gap_without_marker_room_splits_and_the_next_gap_can_still_place_inline() {
    let mut graph = graph(Direction::Down, &[size(60.0, 40.0); 3], &[(0, 1), (1, 2)]);
    blocked_by_markers(&mut graph);
    graph.edges[1].label = Some(size(24.0, 18.0));
    let options = opt_in();
    let old = legacy(&graph, &options);
    let new = layout(&graph, &options).unwrap();
    assert_eq!(old.node_ranks, [0, 2, 4]);
    assert_eq!(new.node_ranks, [0, 2, 3]);
    assert_eq!(new.edges[0], old.edges[0]);
    assert_eq!(new.stats.dummy_nodes, 1);
    assert!(new.bounds.height < old.bounds.height);
    let last_label = rect(
        new.edges[1].label_center.unwrap(),
        graph.edges[1].label.unwrap(),
    );
    assert!(new.nodes.iter().all(|&node| !overlaps(node, last_label)));
}

#[test]
fn a_through_edge_reserves_tiers_at_its_shared_rank_faces() {
    let mut graph = graph(
        Direction::Down,
        &[size(60.0, 40.0); 4],
        &[(0, 1), (1, 2), (2, 3), (0, 2)],
    );
    for edge in &mut graph.edges[..3] {
        edge.label = Some(size(36.0, 20.0));
    }
    let drawing = layout(&graph, &opt_in()).unwrap();
    assert_eq!(drawing.node_ranks, [0, 2, 4, 5]);
    let last_edge = &drawing.edges[2];
    assert!(
        last_edge
            .points
            .iter()
            .all(|p| p.x == last_edge.label_center.unwrap().x)
    );
}

#[test]
fn a_foreign_marker_restores_its_blocked_gap_and_keeps_a_later_inline_label() {
    let mut graph = graph(
        Direction::Down,
        &[size(60.0, 40.0); 4],
        &[(0, 1), (0, 2), (1, 3)],
    );
    let measured = size(36.0, 20.0);
    let marker = MarkerMetrics {
        length: 22.0,
        width: 200.0,
    };
    graph.edges[0].label = Some(measured);
    graph.edges[1].end_marker = marker;
    graph.edges[2].label = Some(size(24.0, 18.0));
    let drawing = layout(&graph, &opt_in()).unwrap();
    assert_eq!(drawing.node_ranks, [0, 2, 2, 3]);
    let target = drawing.edges[1].points.last().unwrap();
    let recovered_label = drawing.edges[0].label_center.unwrap();
    assert!(recovered_label.y + measured.height * 0.5 < target.y - marker.length);
    assert!(drawing.edges[2].label_center.is_some());
}

struct Budget {
    used: usize,
    limit: usize,
}

impl WorkControl for Budget {
    fn check(&mut self) -> Result<(), WorkError> {
        Ok(())
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        if units > self.limit - self.used {
            return Err(WorkError::Interrupted);
        }
        self.used += units;
        Ok(())
    }
}

#[test]
fn horizontal_gap_budgets_both_visible_markers_before_trying_inline_placement() {
    let marker = MarkerMetrics {
        length: 7.0,
        width: 10.0,
    };
    let measured = size(34.0, 18.0);
    for direction in [Direction::Right, Direction::Left] {
        let mut graph = graph(direction, &[size(60.0, 40.0); 2], &[(0, 1)]);
        graph.edges[0].label = Some(measured);
        graph.edges[0].start_marker = marker;
        graph.edges[0].end_marker = marker;
        let mut options = opt_in();
        let mut scratch = LayoutScratch::default();
        let mut meter = Budget {
            used: 0,
            limit: usize::MAX,
        };
        options.inline_labels = false;
        let split = layout_with_control(&graph, &options, &mut scratch, &mut meter).unwrap();
        let mut same_budget = Budget {
            used: 0,
            limit: meter.used,
        };
        options.inline_labels = true;
        assert_eq!(
            layout_with_control(&graph, &options, &mut scratch, &mut same_budget).unwrap(),
            split
        );
        assert_eq!(same_budget.used, meter.used);
        assert_eq!(split.stats.dummy_nodes, 1);

        // A width-zero straight lead should leave room for a label on the shaft.
        graph.edges[0].start_marker.width = 0.0;
        let inline = layout(&graph, &options).unwrap();
        assert_eq!(inline.stats.dummy_nodes, 0);
        let edge = &inline.edges[0];
        let [start, .., end] = edge.points.as_slice() else {
            panic!("the label should have a routed edge");
        };
        let label = edge.label_center.unwrap();
        assert_eq!(label.y, start.y);
        assert_eq!(label.y, end.y);
        let padding = options.spacing.edge;
        assert!((label.x - start.x).abs() > measured.width * 0.5 + padding);
        assert!((label.x - end.x).abs() > measured.width * 0.5 + padding + marker.length);
    }
}

#[test]
fn work_rejection_and_zero_dummy_capacity_recover_without_losing_labels() {
    let mut graph = graph(Direction::Down, &[size(60.0, 40.0); 2], &[(0, 1)]);
    blocked_by_markers(&mut graph);
    let mut scratch = LayoutScratch::default();
    for cap in [1, 0] {
        let mut options = opt_in();
        options.caps.max_dummies = cap;
        let old = legacy(&graph, &options);
        let mut meter = Budget {
            used: 0,
            limit: usize::MAX,
        };
        assert_eq!(
            layout_with_control(&graph, &options, &mut scratch, &mut meter).unwrap(),
            old
        );
        assert!(old.edges[0].label_center.is_some());
        for limit in [1, meter.used - 1] {
            let mut meter = Budget { used: 0, limit };
            assert_eq!(
                layout_with_control(&graph, &options, &mut scratch, &mut meter),
                Err(LayoutError::Work(WorkError::Interrupted))
            );
        }
        let mut meter = Budget {
            used: 0,
            limit: meter.used,
        };
        assert_eq!(
            layout_with_control(&graph, &options, &mut scratch, &mut meter).unwrap(),
            old
        );
    }
}

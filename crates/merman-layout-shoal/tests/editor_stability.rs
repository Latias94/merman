use merman_layout_shoal::{
    Direction, EdgeSpec, Endpoint, Graph, GroupId, GroupSpec, LayoutOutput, LayoutScratch, NodeId,
    NodeSpec, Options, OrderHint, Point, PositionHint, Rect, RoutePoint, RoutingStyle, Side, Size,
    WidthPolicy, WorkControl, WorkError, layout, layout_with_control,
};

struct Sequence {
    name: &'static str,
    edges: &'static [(u32, u32)],
    leaf_parent: u32,
    added_edge: (u32, u32),
}

const SEQUENCES: [Sequence; 2] = [
    Sequence {
        name: "shared-relays",
        edges: &[
            (0, 3),
            (1, 3),
            (1, 4),
            (2, 3),
            (2, 4),
            (2, 5),
            (3, 6),
            (3, 7),
            (3, 8),
            (4, 6),
            (4, 7),
            (5, 7),
            (5, 8),
        ],
        leaf_parent: 1,
        added_edge: (0, 4),
    },
    Sequence {
        name: "shared-sinks",
        edges: &[
            (0, 3),
            (0, 5),
            (1, 3),
            (1, 4),
            (1, 5),
            (2, 4),
            (2, 5),
            (3, 6),
            (4, 6),
            (4, 7),
            (5, 6),
            (5, 8),
        ],
        leaf_parent: 0,
        added_edge: (2, 3),
    },
];

#[derive(Clone, Copy, Debug)]
enum Edit {
    Relabel,
    Restyle,
    AddLeaf(u32),
    AddEdge(u32, u32),
}

fn horizontal(direction: Direction) -> bool {
    matches!(direction, Direction::Right | Direction::Left)
}

fn node(index: u32, direction: Direction) -> NodeSpec {
    let (along, cross) = (36.0, 78.0 + f64::from(index % 3) * 4.0);
    let (width, height) = if horizontal(direction) {
        (along, cross)
    } else {
        (cross, along)
    };
    NodeSpec::new(Size { width, height }, index)
}

fn edge(source: u32, target: u32) -> EdgeSpec {
    EdgeSpec::new(NodeId(source).into(), NodeId(target).into())
}

fn sequence_graph(sequence: &Sequence, direction: Direction) -> Graph {
    let mut graph = Graph::default();
    graph.direction = direction;
    graph.nodes = (0..9).map(|index| node(index, direction)).collect();
    graph.edges = sequence
        .edges
        .iter()
        .map(|&(source, target)| edge(source, target))
        .collect();
    graph
}

impl Edit {
    fn apply(self, graph: &mut Graph, options: &mut Options) {
        match self {
            Self::Relabel => {
                let size = &mut graph.nodes[4].size;
                if horizontal(graph.direction) {
                    size.height += 60.0;
                } else {
                    size.width += 60.0;
                }
            }
            Self::Restyle => options.routing = RoutingStyle::Orthogonal,
            Self::AddLeaf(parent) => {
                let leaf = u32::try_from(graph.nodes.len()).unwrap();
                graph.nodes.push(node(leaf, graph.direction));
                graph.edges.push(edge(parent, leaf));
            }
            Self::AddEdge(source, target) => graph.edges.push(edge(source, target)),
        }
    }
}

fn cross_center(rect: Rect, direction: Direction) -> f64 {
    let center = rect.center();
    if horizontal(direction) {
        center.y
    } else {
        center.x
    }
}

fn with_previous_hints(graph: &Graph, previous: &LayoutOutput) -> Graph {
    assert_eq!(previous.nodes.len(), previous.node_ranks.len());
    let mut hinted = graph.clone();
    for (node, (&rect, &rank)) in hinted
        .nodes
        .iter_mut()
        .zip(previous.nodes.iter().zip(&previous.node_ranks))
    {
        node.order_hint = Some(OrderHint {
            rank: i32::try_from(rank).unwrap(),
            cross_key: cross_center(rect, previous.direction),
        });
        node.order_hint_direction = Some(previous.direction);
    }
    hinted
}

fn node_index(endpoint: Endpoint) -> usize {
    let Endpoint::Node(NodeId(index)) = endpoint else {
        panic!("expected a leaf endpoint");
    };
    index as usize
}

fn incident_signatures(graph: &Graph) -> Vec<u64> {
    // The fixture's NodeIds survive every edit. Include direction and multiplicity.
    let mut signatures = vec![0_u64; graph.nodes.len()];
    for edge in &graph.edges {
        let from = node_index(edge.source);
        let to = node_index(edge.target);
        for (node, peer, outgoing) in [(from, to, true), (to, from, false)] {
            signatures[node] = signatures[node]
                .wrapping_mul(1_099_511_628_211)
                .wrapping_add(2 * peer as u64 + u64::from(outgoing) + 1);
        }
    }
    signatures
}

fn with_previous_positions(
    graph: &Graph,
    previous: &LayoutOutput,
    old_signatures: &[u64],
    new_signatures: &[u64],
) -> Graph {
    let mut hinted = with_previous_hints(graph, previous);
    let (mut min_x, mut min_y) = (f64::INFINITY, f64::INFINITY);
    for rect in &previous.nodes {
        min_x = min_x.min(rect.x);
        min_y = min_y.min(rect.y);
    }
    for (index, rect) in previous.nodes.iter().enumerate() {
        let center = rect.center();
        hinted.nodes[index].set_position_hint(
            PositionHint {
                center: Point {
                    x: center.x - min_x,
                    y: center.y - min_y,
                },
                size: Size {
                    width: rect.width,
                    height: rect.height,
                },
                incident_signature: Some(old_signatures[index]),
            },
            Some(new_signatures[index]),
        );
    }
    hinted
}

struct Meter {
    limit: usize,
    used: usize,
    comparisons: usize,
}

impl Meter {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            used: 0,
            comparisons: 0,
        }
    }
}

impl WorkControl for Meter {
    fn check(&mut self) -> Result<(), WorkError> {
        Ok(())
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        let next = self
            .used
            .checked_add(units)
            .ok_or(WorkError::ArithmeticOverflow)?;
        if next > self.limit {
            return Err(WorkError::Interrupted);
        }
        self.used = next;
        Ok(())
    }

    fn try_charge_optional(&mut self, units: usize, headroom: usize) -> Result<bool, WorkError> {
        self.comparisons += usize::from(units == 0);
        if units.saturating_add(headroom) > self.limit - self.used {
            return Ok(false);
        }
        self.charge(units)?;
        Ok(true)
    }
}

#[test]
fn previous_frames_only_switch_to_a_reference_that_improves_crossing_quality() {
    struct CrossingEdit {
        name: &'static str,
        sizes: &'static [(u32, u32)],
        edges: &'static [(u32, u32)],
        added: (u32, u32),
        group_members: &'static [u32],
        tradeoff: bool,
    }

    let cases = [
        CrossingEdit {
            name: "DAG",
            sizes: &[
                (86, 34),
                (90, 43),
                (93, 45),
                (92, 22),
                (90, 40),
                (68, 44),
                (62, 28),
                (94, 46),
            ],
            edges: &[
                (0, 2),
                (0, 5),
                (1, 5),
                (1, 6),
                (2, 5),
                (3, 4),
                (4, 7),
                (6, 7),
            ],
            added: (4, 5),
            group_members: &[],
            tradeoff: false,
        },
        CrossingEdit {
            name: "SCC",
            sizes: &[
                (43, 16),
                (26, 33),
                (65, 40),
                (51, 25),
                (39, 43),
                (89, 22),
                (84, 28),
                (33, 36),
            ],
            edges: &[
                (0, 3),
                (1, 4),
                (2, 5),
                (2, 7),
                (3, 5),
                (4, 6),
                (5, 2),
                (5, 6),
                (5, 7),
                (6, 0),
                (6, 2),
                (6, 3),
                (7, 6),
            ],
            added: (6, 7),
            group_members: &[],
            tradeoff: false,
        },
        CrossingEdit {
            name: "compound",
            sizes: &[
                (52, 48),
                (42, 17),
                (54, 43),
                (51, 52),
                (49, 29),
                (54, 29),
                (46, 26),
                (47, 20),
            ],
            edges: &[
                (0, 1),
                (0, 5),
                (0, 7),
                (1, 2),
                (2, 1),
                (2, 6),
                (3, 4),
                (3, 5),
                (4, 2),
                (4, 5),
                (5, 3),
                (5, 4),
                (5, 7),
                (6, 1),
                (6, 4),
                (7, 1),
            ],
            added: (5, 6),
            group_members: &[0, 1, 2, 3],
            tradeoff: false,
        },
        CrossingEdit {
            name: "SCC with unchanged routed crossings",
            sizes: &[(40, 24); 4],
            edges: &[(0, 1), (0, 2), (2, 0), (2, 3), (3, 0), (3, 2)],
            added: (0, 3),
            group_members: &[],
            tradeoff: false,
        },
        CrossingEdit {
            name: "compound DAG with a topology and routing tradeoff",
            sizes: &[(40, 24); 5],
            edges: &[(0, 2), (0, 4), (1, 3), (1, 4), (2, 3), (2, 4)],
            added: (3, 4),
            group_members: &[3, 4],
            tradeoff: true,
        },
    ];
    for case in cases {
        let mut graph = Graph::default();
        if !case.group_members.is_empty() {
            graph.groups.push(GroupSpec::new(0));
        }
        for (index, &(width, height)) in case.sizes.iter().enumerate() {
            let index = index as u32;
            let mut node = NodeSpec::new(
                Size {
                    width: f64::from(width),
                    height: f64::from(height),
                },
                index,
            );
            node.parent = case.group_members.contains(&index).then_some(GroupId(0));
            graph.nodes.push(node);
        }
        graph
            .edges
            .extend(case.edges.iter().map(|&(from, to)| edge(from, to)));
        let mut options = Options::default();
        options.capture_node_ranks = true;
        let previous = layout(&graph, &options).unwrap();
        graph.edges.push(edge(case.added.0, case.added.1));
        let stateless = layout(&graph, &options).unwrap();
        let graph = with_previous_hints(&graph, &previous);
        let mut scratch = LayoutScratch::default();
        let mut full = Meter::new(usize::MAX);
        let hinted = layout_with_control(&graph, &options, &mut scratch, &mut full).unwrap();
        if case.tradeoff {
            assert!(hinted.stats.topological_crossings < stateless.stats.topological_crossings);
            assert!(hinted.stats.routed_crossings > stateless.stats.routed_crossings);
        } else {
            assert!(
                hinted.stats.topological_crossings <= stateless.stats.topological_crossings
                    && hinted.stats.routed_crossings <= stateless.stats.routed_crossings,
                "{}: hinted {:?}, stateless {:?}",
                case.name,
                hinted.stats,
                stateless.stats,
            );
        }

        let mut primary = Meter::new(usize::MAX);
        let first = layout_with_control(
            &graph,
            &options,
            &mut scratch,
            &mut PrimaryOnly(&mut primary),
        )
        .unwrap();
        let mut exact = Meter::new(primary.used);
        let constrained = layout_with_control(&graph, &options, &mut scratch, &mut exact).unwrap();
        assert_eq!(constrained, first);
        assert_eq!(exact.comparisons, full.comparisons, "{}", case.name);
        assert_eq!(exact.used, primary.used);
        if case.tradeoff {
            assert_eq!(hinted, first);
            assert_eq!(exact.comparisons, 1);
            check_fit_width_replays_only_the_selected_trial(&graph, options, &mut scratch);
        }
    }
}

fn check_fit_width_replays_only_the_selected_trial(
    graph: &Graph,
    mut options: Options,
    scratch: &mut LayoutScratch,
) {
    let max_width = 20.0;
    options.width_policy = WidthPolicy::FitWidth {
        max_width,
        allow_turn: false,
        turn_breakpoint: 640.0,
        fold_wide_ranks: false,
    };
    let mut primary = Meter::new(usize::MAX);
    let first =
        layout_with_control(graph, &options, scratch, &mut PrimaryOnly(&mut primary)).unwrap();
    assert!(first.bounds.width > max_width);

    let mut full = Meter::new(usize::MAX);
    let compared = layout_with_control(graph, &options, scratch, &mut full).unwrap();
    assert_eq!(full.comparisons, 1);
    assert!(compared.bounds.width <= first.bounds.width);

    let mut reference = graph.clone();
    for node in &mut reference.nodes {
        node.order_hint = None;
    }
    let mut stateless = Meter::new(usize::MAX);
    layout_with_control(&reference, &options, scratch, &mut stateless).unwrap();
    let compared_work = full.used - primary.used;
    assert!(compared_work > 0 && compared_work < stateless.used);
}

struct PrimaryOnly<'a>(&'a mut Meter);

impl WorkControl for PrimaryOnly<'_> {
    fn check(&mut self) -> Result<(), WorkError> {
        self.0.check()
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.0.charge(units)
    }
}

#[test]
fn unchanged_hints_skip_a_second_solve_for_an_unavoidable_crossing() {
    for cyclic in [false, true] {
        let mut graph = Graph::default();
        graph.nodes = (0..6).map(|index| node(index, graph.direction)).collect();
        graph.edges = (0..3)
            .flat_map(|from| (3..6).map(move |to| edge(from, to)))
            .collect();
        if cyclic {
            graph
                .edges
                .extend((3..6).flat_map(|from| (0..3).map(move |to| edge(from, to))));
        }
        let mut options = Options::default();
        options.capture_node_ranks = true;
        let previous = layout(&graph, &options).unwrap();
        assert_eq!(previous.stats.scc_cyclic_count, usize::from(cyclic));
        assert!(previous.stats.topological_crossings > 0);

        let mut graph = with_previous_hints(&graph, &previous);
        for node in &mut graph.nodes {
            node.order_hint.as_mut().unwrap().rank = 0;
        }
        let mut meter = Meter::new(usize::MAX);
        let output =
            layout_with_control(&graph, &options, &mut LayoutScratch::default(), &mut meter)
                .unwrap();
        assert_eq!(output, previous);
        assert_eq!(meter.comparisons, 0);
    }
}

fn relabelled_graph() -> (Graph, Graph, Options) {
    let mut graph = sequence_graph(&SEQUENCES[1], Direction::Down);
    let mut options = Options::default();
    options.capture_node_ranks = true;
    let previous = layout(&graph, &options).unwrap();
    let signatures = incident_signatures(&graph);
    Edit::Relabel.apply(&mut graph, &mut options);
    let ordered = with_previous_hints(&graph, &previous);
    let positioned = with_previous_positions(&graph, &previous, &signatures, &signatures);
    (ordered, positioned, options)
}

#[test]
fn a_projection_can_be_declined_before_or_during_optional_work() {
    #[derive(Default)]
    struct ProjectionBudget {
        mandatory: usize,
        trials: usize,
        charges: usize,
        decline_at: Option<usize>,
    }

    impl WorkControl for ProjectionBudget {
        fn check(&mut self) -> Result<(), WorkError> {
            Ok(())
        }

        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.mandatory += units;
            Ok(())
        }

        fn try_charge_optional(
            &mut self,
            units: usize,
            _headroom: usize,
        ) -> Result<bool, WorkError> {
            if units == 0 {
                self.trials += 1;
                // Allow only the coordinate projection, leaving the graph-wide comparison out.
                return Ok(self.trials == 1);
            }
            self.charges += 1;
            Ok(self.decline_at != Some(self.charges))
        }
    }

    let (ordered, positioned, options) = relabelled_graph();
    let mut scratch = LayoutScratch::default();
    let mut required = Meter::new(usize::MAX);
    let baseline = layout_with_control(
        &ordered,
        &options,
        &mut scratch,
        &mut PrimaryOnly(&mut required),
    )
    .unwrap();
    let mut exact = Meter::new(required.used);
    assert_eq!(
        layout_with_control(&positioned, &options, &mut scratch, &mut exact).unwrap(),
        baseline
    );
    assert_eq!(exact.used, required.used);
    assert_eq!(exact.comparisons, 1);

    let mut full = ProjectionBudget::default();
    let projected = layout_with_control(&positioned, &options, &mut scratch, &mut full).unwrap();
    assert_ne!(projected.nodes, baseline.nodes);
    assert!(full.charges > 2);
    let mut late = ProjectionBudget {
        decline_at: Some(full.charges),
        ..ProjectionBudget::default()
    };
    assert_eq!(
        layout_with_control(&positioned, &options, &mut scratch, &mut late).unwrap(),
        baseline
    );
    assert_eq!(late.charges, full.charges);
    assert_eq!(late.mandatory, required.used);
    assert_eq!(
        layout_with_control(
            &positioned,
            &options,
            &mut scratch,
            &mut ProjectionBudget::default()
        )
        .unwrap(),
        projected
    );
}

#[test]
fn absent_stale_and_invalid_position_frames_use_the_order_only_geometry_and_work() {
    let (_, positioned, options) = relabelled_graph();
    let mut scratch = LayoutScratch::default();
    for invalidate in [
        |node: &mut NodeSpec| node.order_hint_direction = None,
        |node: &mut NodeSpec| node.order_hint_direction = Some(Direction::Right),
        |node: &mut NodeSpec| node.order_hint.as_mut().unwrap().rank += 1000,
        |node: &mut NodeSpec| node.position_hint.as_mut().unwrap().center.x = f64::NAN,
        |node: &mut NodeSpec| node.position_hint.as_mut().unwrap().size.height = -1.,
    ] {
        let mut graph = positioned.clone();
        for node in &mut graph.nodes {
            invalidate(node);
        }
        let mut hinted = Meter::new(usize::MAX);
        let output = layout_with_control(&graph, &options, &mut scratch, &mut hinted).unwrap();
        for node in &mut graph.nodes {
            node.position_hint = None;
        }
        let mut cleared = Meter::new(usize::MAX);
        let baseline = layout_with_control(&graph, &options, &mut scratch, &mut cleared).unwrap();
        assert_eq!(output, baseline);
        assert_eq!(hinted.used, cleared.used);
        assert_eq!(hinted.comparisons, cleared.comparisons);
    }
}

#[derive(Default)]
struct Movement {
    distance: f64,
    cross_distance: f64,
    survivors: usize,
    inversions: usize,
    pairs: usize,
}

impl Movement {
    fn between(previous: &LayoutOutput, current: &LayoutOutput) -> Self {
        let direction = current.direction;
        assert_eq!(previous.direction, direction);
        let mut movement = Self::default();
        for (index, (&before, &after)) in previous.nodes.iter().zip(&current.nodes).enumerate() {
            let (start, finish) = (before.center(), after.center());
            movement.distance += (finish.x - start.x).hypot(finish.y - start.y);
            movement.cross_distance +=
                (cross_center(after, direction) - cross_center(before, direction)).abs();
            movement.survivors += 1;
            for neighbor in 0..index {
                if previous.node_ranks[neighbor] == previous.node_ranks[index]
                    && current.node_ranks[neighbor] == current.node_ranks[index]
                {
                    let old_delta = cross_center(before, direction)
                        - cross_center(previous.nodes[neighbor], direction);
                    let new_delta = cross_center(after, direction)
                        - cross_center(current.nodes[neighbor], direction);
                    movement.pairs += 1;
                    movement.inversions += usize::from(old_delta * new_delta < 0.0);
                }
            }
        }
        movement
    }

    fn record(&mut self, value: &Self) {
        self.distance += value.distance;
        self.cross_distance += value.cross_distance;
        self.survivors += value.survivors;
        self.inversions += value.inversions;
        self.pairs += value.pairs;
    }

    fn mean(&self) -> f64 {
        self.distance / self.survivors as f64
    }

    fn mean_across(&self) -> f64 {
        self.cross_distance / self.survivors as f64
    }
}

fn check_geometry(graph: &Graph, drawing: &LayoutOutput, routing: RoutingStyle) {
    const EPS: f64 = 1e-8;
    let within = |v, low, high| low - EPS <= v && v <= high + EPS;
    assert_eq!(graph.nodes.len(), drawing.nodes.len());
    assert_eq!(graph.edges.len(), drawing.edges.len());
    for (index, node) in drawing.nodes.iter().enumerate() {
        assert!(node.x.is_finite() && node.y.is_finite());
        assert_eq!(node.width, graph.nodes[index].size.width);
        assert_eq!(node.height, graph.nodes[index].size.height);
        for other in &drawing.nodes[..index] {
            assert!(
                node.x.max(other.x) + EPS >= (node.x + node.width).min(other.x + other.width)
                    || node.y.max(other.y) + EPS
                        >= (node.y + node.height).min(other.y + other.height),
                "overlapping node boxes {node:?} and {other:?}"
            );
        }
    }
    for (spec, geometry) in graph.edges.iter().zip(&drawing.edges) {
        let points = &geometry.points;
        assert!(points.len() >= 2);
        assert!(
            points
                .iter()
                .all(|point| point.x.is_finite() && point.y.is_finite())
        );
        for (end, side, point) in [
            (spec.source, geometry.ports.0, &points[0]),
            (spec.target, geometry.ports.1, points.last().unwrap()),
        ] {
            let r = drawing.nodes[node_index(end)];
            assert_eq!(point.axis, side.axis());
            assert!(within(point.x, r.x, r.x + r.width));
            assert!(within(point.y, r.y, r.y + r.height));
            assert!(match side {
                Side::Top => (point.y - r.y).abs() <= EPS,
                Side::Bottom => (point.y - (r.y + r.height)).abs() <= EPS,
                Side::Left => (point.x - r.x).abs() <= EPS,
                Side::Right => (point.x - (r.x + r.width)).abs() <= EPS,
            });
        }
        let hits = |r: Rect, a: &RoutePoint, b: &RoutePoint| {
            let (mut enter, mut exit) = (0.0_f64, 1.0_f64);
            for (low, high, start, end) in [
                (r.x + EPS, r.x + r.width - EPS, a.x, b.x),
                (r.y + EPS, r.y + r.height - EPS, a.y, b.y),
            ] {
                if start == end {
                    if !(low < start && start < high) {
                        return false;
                    }
                } else {
                    let start_t = (low - start) / (end - start);
                    let end_t = (high - start) / (end - start);
                    enter = enter.max(start_t.min(end_t));
                    exit = exit.min(start_t.max(end_t));
                }
            }
            enter < exit
        };
        for pair in points.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            if routing == RoutingStyle::Orthogonal {
                assert!(a.x == b.x || a.y == b.y);
            }
            for &rect in &drawing.nodes {
                if routing == RoutingStyle::Curved && a.x != b.x && a.y != b.y {
                    // Check the canonical knots for diagonal curved legs.
                    assert!(!hits(rect, a, a) && !hits(rect, b, b));
                } else {
                    assert!(!hits(rect, a, b), "route {a:?} → {b:?} enters {rect:?}");
                }
            }
        }
    }
}

#[test]
fn editor_corpus_preserves_sibling_order_and_positions_at_equal_crossings() {
    let mut total_stateless = Movement::default();
    let mut total_ordered = Movement::default();
    let mut total_positioned = Movement::default();
    let mut improved_leaf_frames = 0;
    let mut improved_edge_frames = 0;
    for direction in [
        Direction::Down,
        Direction::Up,
        Direction::Right,
        Direction::Left,
    ] {
        for sequence in &SEQUENCES {
            let mut graph = sequence_graph(sequence, direction);
            let mut options = Options::default();
            options.capture_node_ranks = true;
            let mut previous_stateless = layout(&graph, &options).unwrap();
            let mut previous_ordered = previous_stateless.clone();
            let mut previous_positioned = previous_stateless.clone();
            let mut previous_signatures = incident_signatures(&graph);
            for edit in [
                Edit::Relabel,
                Edit::Restyle,
                Edit::AddLeaf(sequence.leaf_parent),
                Edit::AddEdge(sequence.added_edge.0, sequence.added_edge.1),
            ] {
                edit.apply(&mut graph, &mut options);
                let stateless = layout(&graph, &options).unwrap();
                let ordered =
                    layout(&with_previous_hints(&graph, &previous_ordered), &options).unwrap();
                let signatures = incident_signatures(&graph);
                let positioned = layout(
                    &with_previous_positions(
                        &graph,
                        &previous_positioned,
                        &previous_signatures,
                        &signatures,
                    ),
                    &options,
                )
                .unwrap();
                let crossings = (
                    stateless.stats.topological_crossings,
                    stateless.stats.routed_crossings,
                );
                for drawing in [&stateless, &ordered, &positioned] {
                    assert_eq!(
                        stateless.node_ranks, drawing.node_ranks,
                        "{direction:?} {edit:?}"
                    );
                    assert_eq!(
                        crossings,
                        (
                            drawing.stats.topological_crossings,
                            drawing.stats.routed_crossings
                        ),
                        "{} {direction:?} {edit:?}",
                        sequence.name
                    );
                    check_geometry(&graph, drawing, options.routing);
                }
                let without = Movement::between(&previous_stateless, &stateless);
                let with = Movement::between(&previous_ordered, &ordered);
                let positions = Movement::between(&previous_positioned, &positioned);
                assert_eq!(without.pairs, with.pairs);
                assert_eq!(with.pairs, positions.pairs);
                assert_eq!(without.survivors, with.survivors);
                assert_eq!(with.survivors, positions.survivors);
                if with.inversions < without.inversions {
                    match edit {
                        Edit::AddLeaf(_) => improved_leaf_frames += 1,
                        Edit::AddEdge(_, _) => improved_edge_frames += 1,
                        _ => {}
                    }
                }
                println!(
                    "{} {direction:?} {edit:?}: mean {:.3} → {:.3} → {:.3}; across {:.3} → {:.3} → {:.3}; inversions {}/{} → {}/{} → {}/{}; crossings {:?}; hard geometry passed in all three modes",
                    sequence.name,
                    without.mean(),
                    with.mean(),
                    positions.mean(),
                    without.mean_across(),
                    with.mean_across(),
                    positions.mean_across(),
                    without.inversions,
                    without.pairs,
                    with.inversions,
                    with.pairs,
                    positions.inversions,
                    positions.pairs,
                    crossings,
                );
                total_stateless.record(&without);
                total_ordered.record(&with);
                total_positioned.record(&positions);
                previous_stateless = stateless;
                previous_ordered = ordered;
                previous_positioned = positioned;
                previous_signatures = signatures;
            }
        }
    }
    println!(
        "Edit corpus, {} surviving nodes: mean displacement {:.3} → {:.3} → {:.3}; across {:.3} → {:.3} → {:.3}; surviving inversions {}/{} → {}/{} → {}/{}",
        total_stateless.survivors,
        total_stateless.mean(),
        total_ordered.mean(),
        total_positioned.mean(),
        total_stateless.mean_across(),
        total_ordered.mean_across(),
        total_positioned.mean_across(),
        total_stateless.inversions,
        total_stateless.pairs,
        total_ordered.inversions,
        total_ordered.pairs,
        total_positioned.inversions,
        total_positioned.pairs,
    );
    assert!(improved_leaf_frames > 0);
    assert!(improved_edge_frames > 0);
    assert!(total_ordered.mean() < total_stateless.mean());
    assert!(total_ordered.inversions < total_stateless.inversions);
    assert!(total_positioned.mean() < total_ordered.mean());
    assert!(total_positioned.mean_across() < total_ordered.mean_across());
    assert!(total_positioned.inversions <= total_ordered.inversions);
}

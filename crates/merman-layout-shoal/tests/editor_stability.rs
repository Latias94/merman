use merman_layout_shoal::{
    Direction, EdgeSpec, Graph, GroupId, GroupSpec, LayoutOutput, LayoutScratch, NodeId, NodeSpec,
    Options, OrderHint, Rect, RoutingStyle, Size, WidthPolicy, WorkControl, WorkError, layout,
    layout_with_control,
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

#[derive(Default)]
struct Movement {
    distance: f64,
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
        self.survivors += value.survivors;
        self.inversions += value.inversions;
        self.pairs += value.pairs;
    }

    fn mean(&self) -> f64 {
        self.distance / self.survivors as f64
    }
}

#[test]
fn editor_corpus_preserves_more_sibling_order_at_equal_crossings() {
    let mut total_stateless = Movement::default();
    let mut total_hinted = Movement::default();
    let mut improved_leaf_frames = 0;
    let mut improved_edge_frames = 0;
    for direction in [
        Direction::Down,
        Direction::Up,
        Direction::Right,
        Direction::Left,
    ] {
        for sequence in &SEQUENCES {
            let mut graph = Graph::default();
            graph.direction = direction;
            graph.nodes = (0..9).map(|index| node(index, direction)).collect();
            graph.edges = sequence
                .edges
                .iter()
                .map(|&(source, target)| edge(source, target))
                .collect();
            let mut options = Options::default();
            options.capture_node_ranks = true;
            let mut previous_stateless = layout(&graph, &options).unwrap();
            let mut previous_hinted = previous_stateless.clone();
            for edit in [
                Edit::Relabel,
                Edit::Restyle,
                Edit::AddLeaf(sequence.leaf_parent),
                Edit::AddEdge(sequence.added_edge.0, sequence.added_edge.1),
            ] {
                edit.apply(&mut graph, &mut options);
                let stateless = layout(&graph, &options).unwrap();
                let hinted =
                    layout(&with_previous_hints(&graph, &previous_hinted), &options).unwrap();
                assert_eq!(
                    stateless.node_ranks, hinted.node_ranks,
                    "{direction:?} {edit:?}"
                );
                let crossings = (
                    stateless.stats.topological_crossings,
                    stateless.stats.routed_crossings,
                );
                assert_eq!(
                    crossings,
                    (
                        hinted.stats.topological_crossings,
                        hinted.stats.routed_crossings
                    ),
                    "{} {direction:?} {edit:?}",
                    sequence.name
                );
                let without = Movement::between(&previous_stateless, &stateless);
                let with = Movement::between(&previous_hinted, &hinted);
                assert_eq!(without.pairs, with.pairs);
                assert_eq!(without.survivors, with.survivors);
                if with.inversions < without.inversions {
                    match edit {
                        Edit::AddLeaf(_) => improved_leaf_frames += 1,
                        Edit::AddEdge(_, _) => improved_edge_frames += 1,
                        _ => {}
                    }
                }
                println!(
                    "{} {direction:?} {edit:?}: mean {:.3} → {:.3}; inversions {}/{} → {}/{}; crossings {:?}",
                    sequence.name,
                    without.mean(),
                    with.mean(),
                    without.inversions,
                    without.pairs,
                    with.inversions,
                    with.pairs,
                    crossings,
                );
                total_stateless.record(&without);
                total_hinted.record(&with);
                previous_stateless = stateless;
                previous_hinted = hinted;
            }
        }
    }
    println!(
        "Edit corpus: mean displacement {:.3} → {:.3}, surviving inversions {}/{} → {}/{}",
        total_stateless.mean(),
        total_hinted.mean(),
        total_stateless.inversions,
        total_stateless.pairs,
        total_hinted.inversions,
        total_hinted.pairs,
    );
    assert!(improved_leaf_frames > 0);
    assert!(improved_edge_frames > 0);
    assert!(total_hinted.mean() < total_stateless.mean());
    assert!(total_hinted.inversions < total_stateless.inversions);
}

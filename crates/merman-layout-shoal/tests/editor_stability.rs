use merman_layout_shoal::{
    Direction, EdgeSpec, Graph, GroupId, GroupSpec, LayoutOutput, NodeId, NodeSpec, Options,
    OrderHint, Rect, RoutingStyle, Size, layout,
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

#[test]
fn previous_frames_preserve_crossing_quality_after_adding_an_edge() {
    struct CrossingEdit {
        name: &'static str,
        sizes: &'static [(u32, u32)],
        edges: &'static [(u32, u32)],
        added: (u32, u32),
        group_members: &'static [u32],
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
        },
        CrossingEdit {
            name: "SCC with unchanged routed crossings",
            sizes: &[(40, 24); 4],
            edges: &[(0, 1), (0, 2), (2, 0), (2, 3), (3, 0), (3, 2)],
            added: (0, 3),
            group_members: &[],
        },
        CrossingEdit {
            name: "compound DAG with a topology and routing tradeoff",
            sizes: &[(40, 24); 5],
            edges: &[(0, 2), (0, 4), (1, 3), (1, 4), (2, 3), (2, 4)],
            added: (3, 4),
            group_members: &[3, 4],
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
        let hinted = layout(&with_previous_hints(&graph, &previous), &options).unwrap();
        assert!(
            hinted.stats.topological_crossings <= stateless.stats.topological_crossings
                && hinted.stats.routed_crossings <= stateless.stats.routed_crossings,
            "{}: hinted {:?}, stateless {:?}",
            case.name,
            hinted.stats,
            stateless.stats,
        );
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

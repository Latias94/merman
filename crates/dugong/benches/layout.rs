use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::position::bk::{find_type1_conflicts, position_x_with_layering};
use dugong::{EdgeLabel, GraphLabel, NodeLabel, layout, normalize};
use std::hint::black_box;

const LAYER_COUNTS: [usize; 6] = [5, 10, 20, 40, 80, 160];
const CONFLICT_INDEX_LAYERS: usize = 4;
const CONFLICT_INDEX_SEGMENT_WIDTH: usize = 8;
const CONFLICT_INDEX_WIDTHS: [usize; 4] = [64, 256, 1024, 4096];
const TYPE2_FALLBACK_WIDTHS: [usize; 5] = [64, 128, 256, 512, 1024];
const ADJACENCY_FANOUTS: [usize; 4] = [64, 256, 1024, 4096];
const PARALLEL_EDGE_RATIOS: [usize; 3] = [1, 10, 100];
const PARALLEL_ENDPOINT_PAIRS: usize = 64;
const ADJACENCY_QUERY_REPETITIONS: usize = 256;

#[derive(Debug, Clone)]
struct LayeredDagSpec {
    node_ids: Vec<String>,
    layers: Vec<Vec<usize>>,
    edges: Vec<(usize, usize)>,
    rank_gap: Option<i32>,
}

impl LayeredDagSpec {
    fn build(&self) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
            directed: true,
            multigraph: true,
            compound: true,
        });
        g.set_graph(GraphLabel::default());
        g.set_default_node_label(NodeLabel::default);
        g.set_default_edge_label(|| EdgeLabel {
            minlen: 1,
            weight: 1.0,
            ..Default::default()
        });

        for (layer_idx, layer) in self.layers.iter().enumerate() {
            for &node_ix in layer {
                let mut node = NodeLabel {
                    width: 160.0,
                    height: 72.0,
                    ..Default::default()
                };
                if let Some(rank_gap) = self.rank_gap {
                    node.rank = Some((layer_idx as i32) * rank_gap);
                }
                g.set_node(self.node_ids[node_ix].clone(), node);
            }
        }

        for &(from, to) in &self.edges {
            g.set_edge_with_label(
                self.node_ids[from].clone(),
                self.node_ids[to].clone(),
                EdgeLabel {
                    minlen: 1,
                    weight: 1.0,
                    ..Default::default()
                },
            );
        }

        g
    }
}

fn build_layered_dag_spec(
    name: &str,
    layer_count: usize,
    layer_width: usize,
    rank_gap: Option<i32>,
) -> LayeredDagSpec {
    let node_ids: Vec<String> = (0..layer_count * layer_width)
        .map(|i| format!("{name}_n{i}"))
        .collect();
    let mut layers: Vec<Vec<usize>> = Vec::with_capacity(layer_count);
    let mut edges: Vec<(usize, usize)> = Vec::new();

    for layer_idx in 0..layer_count {
        let current_layer: Vec<usize> = (0..layer_width)
            .map(|offset| layer_idx * layer_width + offset)
            .collect();

        if !layers.is_empty() {
            let prev_layer = &layers[layers.len() - 1];
            for (index, &node_ix) in current_layer.iter().enumerate() {
                let source = prev_layer[index % prev_layer.len()];
                edges.push((source, node_ix));
                if index % 3 == 0 {
                    let extra_source = prev_layer[(index + 1) % prev_layer.len()];
                    edges.push((extra_source, node_ix));
                }
            }
        }

        layers.push(current_layer);
    }

    LayeredDagSpec {
        node_ids,
        layers,
        edges,
        rank_gap,
    }
}

fn bench_layout(c: &mut Criterion) {
    let mut group = c.benchmark_group("dugong_layout");

    for layer_count in LAYER_COUNTS {
        let spec = build_layered_dag_spec("layout", layer_count, 10, None);
        let node_count = spec.node_ids.len();
        group.throughput(criterion::Throughput::Elements(node_count as u64));
        group.bench_with_input(BenchmarkId::new("plan", node_count), &spec, |b, spec| {
            b.iter_batched(
                || spec.build(),
                |mut g| {
                    layout(black_box(&mut g)).expect("benchmark graph must be valid for layout");
                    black_box(g.node_count());
                },
                BatchSize::LargeInput,
            )
        });
    }

    group.finish();
}

fn bench_graph_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("dugong_graph_build");

    for layer_count in LAYER_COUNTS {
        let spec = build_layered_dag_spec("build", layer_count, 10, None);
        let node_count = spec.node_ids.len();
        group.throughput(criterion::Throughput::Elements(node_count as u64));
        group.bench_with_input(BenchmarkId::new("plan", node_count), &spec, |b, spec| {
            b.iter(|| black_box(spec.build()))
        });
    }

    for ratio in PARALLEL_EDGE_RATIOS {
        let spec = ParallelEdgeSpec::new(PARALLEL_ENDPOINT_PAIRS, ratio);
        group.throughput(criterion::Throughput::Elements(spec.edge_names.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("parallel_edges_per_pair", ratio),
            &spec,
            |b, spec| b.iter(|| black_box(spec.build())),
        );
    }

    group.finish();
}

#[derive(Debug)]
struct ParallelEdgeSpec {
    targets: Vec<String>,
    edge_names: Vec<(usize, String)>,
}

impl ParallelEdgeSpec {
    fn new(endpoint_pairs: usize, edges_per_pair: usize) -> Self {
        let targets = (0..endpoint_pairs)
            .map(|index| format!("parallel-target-{index}"))
            .collect();
        let edge_names = (0..endpoint_pairs)
            .flat_map(|target_ix| {
                (0..edges_per_pair)
                    .map(move |edge_ix| (target_ix, format!("edge-{target_ix}-{edge_ix}")))
            })
            .collect();
        Self {
            targets,
            edge_names,
        }
    }

    fn build(&self) -> Graph<(), (), ()> {
        let mut g = Graph::with_capacity(
            GraphOptions {
                multigraph: true,
                ..Default::default()
            },
            self.targets.len() + 1,
            self.edge_names.len(),
        );
        for (target_ix, name) in &self.edge_names {
            g.set_edge_named(
                "parallel-source",
                self.targets[*target_ix].as_str(),
                Some(name.as_str()),
                None::<()>,
            );
        }
        g
    }
}

#[derive(Debug)]
struct FanoutSpec {
    targets: Vec<String>,
}

impl FanoutSpec {
    fn new(width: usize) -> Self {
        Self {
            targets: (0..width)
                .map(|index| format!("fanout-target-{index}"))
                .collect(),
        }
    }

    fn build(&self) -> Graph<(), (), ()> {
        let mut g = Graph::with_capacity(
            GraphOptions::default(),
            self.targets.len() + 1,
            self.targets.len(),
        );
        for target in &self.targets {
            g.set_edge("fanout-source", target.as_str());
        }
        g
    }
}

fn bench_adjacency_churn(c: &mut Criterion) {
    let mut group = c.benchmark_group("dugong_adjacency_churn");

    for width in ADJACENCY_FANOUTS {
        let spec = FanoutSpec::new(width);
        group.throughput(criterion::Throughput::Elements(width as u64));
        group.bench_with_input(
            BenchmarkId::new("retain_last_then_query", width),
            &spec,
            |b, spec| {
                b.iter_batched(
                    || spec.build(),
                    |mut g| {
                        for target in &spec.targets[..spec.targets.len() - 1] {
                            black_box(g.remove_edge("fanout-source", target, None));
                        }
                        for _ in 0..ADJACENCY_QUERY_REPETITIONS {
                            black_box(g.first_successor("fanout-source"));
                        }
                    },
                    BatchSize::LargeInput,
                )
            },
        );
    }

    group.finish();
}

fn bench_normalize(c: &mut Criterion) {
    let mut group = c.benchmark_group("dugong_normalize");

    for layer_count in LAYER_COUNTS {
        let spec = build_layered_dag_spec("normalize", layer_count, 10, Some(2));
        let node_count = spec.node_ids.len();
        group.throughput(criterion::Throughput::Elements(node_count as u64));

        group.bench_with_input(BenchmarkId::new("run", node_count), &spec, |b, spec| {
            b.iter_batched(
                || spec.build(),
                |mut g| {
                    normalize::run(black_box(&mut g));
                    black_box(g.node_count());
                },
                BatchSize::LargeInput,
            )
        });

        group.bench_with_input(BenchmarkId::new("undo", node_count), &spec, |b, spec| {
            b.iter_batched(
                || {
                    let mut g = spec.build();
                    normalize::run(&mut g);
                    g
                },
                |mut g| {
                    normalize::undo(black_box(&mut g));
                    black_box(g.node_count());
                },
                BatchSize::LargeInput,
            )
        });
    }

    group.finish();
}

fn build_conflict_index_graph(
    layer_count: usize,
    layer_width: usize,
) -> (
    Graph<NodeLabel, EdgeLabel, GraphLabel>,
    Vec<Vec<String>>,
    usize,
) {
    let mut g = Graph::new(GraphOptions::default());
    g.set_graph(GraphLabel::default());
    let layering = (0..layer_count)
        .map(|layer| {
            (0..layer_width)
                .map(|order| format!("conflict-l{layer}-n{order}"))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    for (rank, layer) in layering.iter().enumerate() {
        for (order, id) in layer.iter().enumerate() {
            let is_segment =
                order % CONFLICT_INDEX_SEGMENT_WIDTH == CONFLICT_INDEX_SEGMENT_WIDTH - 1;
            g.set_node(
                id.clone(),
                NodeLabel {
                    width: 160.0,
                    height: 72.0,
                    rank: Some(rank as i32),
                    order: Some(order),
                    dummy: is_segment.then(|| "edge".to_string()),
                    ..Default::default()
                },
            );
        }
    }

    for rank in 1..layer_count {
        for order in 0..layer_width {
            let is_segment =
                order % CONFLICT_INDEX_SEGMENT_WIDTH == CONFLICT_INDEX_SEGMENT_WIDTH - 1;
            let source_order = if is_segment {
                order
            } else {
                let reversed = layer_width - 1 - order;
                if reversed % CONFLICT_INDEX_SEGMENT_WIDTH == CONFLICT_INDEX_SEGMENT_WIDTH - 1 {
                    reversed.saturating_sub(1)
                } else {
                    reversed
                }
            };
            g.set_edge(
                layering[rank - 1][source_order].clone(),
                layering[rank][order].clone(),
            );
        }
    }

    let conflicts = find_type1_conflicts(&g, &layering)
        .values()
        .map(|neighbors| neighbors.len())
        .sum();
    (g, layering, conflicts)
}

fn bench_bk_conflict_index(c: &mut Criterion) {
    let mut group = c.benchmark_group("dugong_bk_conflict_index");

    for width in CONFLICT_INDEX_WIDTHS {
        assert_eq!(width % CONFLICT_INDEX_SEGMENT_WIDTH, 0);
        let input = build_conflict_index_graph(CONFLICT_INDEX_LAYERS, width);
        let expected_conflicts =
            (CONFLICT_INDEX_LAYERS - 1) * (CONFLICT_INDEX_SEGMENT_WIDTH - 1) * width
                / CONFLICT_INDEX_SEGMENT_WIDTH;
        assert_eq!(
            input.2, expected_conflicts,
            "fixture conflict growth must remain exactly linear"
        );
        group.throughput(criterion::Throughput::Elements(input.2 as u64));
        group.bench_with_input(
            BenchmarkId::new("crossing_segments", width),
            &input,
            |b, input| {
                let (g, layering, _) = input;
                b.iter(|| black_box(position_x_with_layering(black_box(g), black_box(layering))))
            },
        );
    }

    group.finish();
}

fn build_type2_fallback_graph(
    intermediate_count: usize,
    missing_border_order: bool,
) -> (Graph<NodeLabel, EdgeLabel, GraphLabel>, Vec<Vec<String>>) {
    let mut g = Graph::new(GraphOptions::default());
    g.set_graph(GraphLabel::default());

    let north = vec![
        "north-low".to_string(),
        "north-middle".to_string(),
        "north-high".to_string(),
    ];
    for (order, id) in north.iter().enumerate() {
        g.set_node(
            id.clone(),
            NodeLabel {
                rank: Some(0),
                order: (!missing_border_order || id != "north-high").then_some(order),
                dummy: Some("dummy".to_string()),
                ..Default::default()
            },
        );
    }

    let mut south = Vec::with_capacity(intermediate_count + 2);
    south.push("border-high".to_string());
    south.extend((0..intermediate_count).map(|index| format!("dummy-{index}")));
    south.push("border-low".to_string());

    for (order, id) in south.iter().enumerate() {
        g.set_node(
            id.clone(),
            NodeLabel {
                rank: Some(1),
                order: Some(order),
                dummy: Some(
                    if id.starts_with("border-") {
                        "border"
                    } else {
                        "dummy"
                    }
                    .to_string(),
                ),
                ..Default::default()
            },
        );
    }

    g.set_edge("north-high", "border-high");
    for id in south.iter().skip(1) {
        g.set_edge("north-low", id);
    }

    (g, vec![north, south])
}

fn bench_bk_type2_fallback(c: &mut Criterion) {
    let mut group = c.benchmark_group("dugong_bk_type2_fallback");

    for width in TYPE2_FALLBACK_WIDTHS {
        for (case, missing_border_order) in [
            ("nonmonotonic_border_order", false),
            ("missing_border_order", true),
        ] {
            let input = build_type2_fallback_graph(width, missing_border_order);
            group.throughput(criterion::Throughput::Elements(width as u64));
            group.bench_with_input(BenchmarkId::new(case, width), &input, |b, (g, layering)| {
                b.iter(|| black_box(position_x_with_layering(black_box(g), black_box(layering))))
            });
        }
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_layout,
    bench_graph_build,
    bench_adjacency_churn,
    bench_normalize,
    bench_bk_conflict_index,
    bench_bk_type2_fallback
);
criterion_main!(benches);

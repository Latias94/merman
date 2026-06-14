use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, NodeLabel, layout_dagreish, normalize};
use std::hint::black_box;
use std::time::Duration;

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
            ..Default::default()
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

fn bench_layout_dagreish(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout_dagreish");
    group.measurement_time(Duration::from_secs(10));

    for layer_count in [10_usize, 25, 50] {
        let spec = build_layered_dag_spec("layout", layer_count, 10, None);
        let node_count = spec.node_ids.len();
        group.bench_with_input(BenchmarkId::new("plan", node_count), &spec, |b, spec| {
            b.iter_batched(
                || spec.build(),
                |mut g| {
                    layout_dagreish(black_box(&mut g));
                    black_box(g.node_count());
                },
                BatchSize::LargeInput,
            )
        });
    }

    group.finish();
}

fn bench_normalize(c: &mut Criterion) {
    let mut group = c.benchmark_group("normalize");
    group.measurement_time(Duration::from_secs(10));

    for layer_count in [10_usize, 25, 50] {
        let spec = build_layered_dag_spec("normalize", layer_count, 10, Some(2));
        let node_count = spec.node_ids.len();

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

criterion_group!(benches, bench_layout_dagreish, bench_normalize);
criterion_main!(benches);

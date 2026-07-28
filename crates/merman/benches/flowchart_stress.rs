use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use merman::svg::{
    LayoutOptions, RenderEnvironment, SvgDebugOptions, SvgRenderOptions, headless_layout_options,
};
use merman_core::{Engine, ParseOptions};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::hint::black_box;

const CLASS_MEDIUM: &str = include_str!("fixtures/class_medium.mmd");
const FLOWCHART_LARGE: &str = include_str!("fixtures/flowchart_large.mmd");
const FLOWCHART_MEDIUM: &str = include_str!("fixtures/flowchart_medium.mmd");
const FLOWCHART_PORTS_HEAVY: &str = include_str!("fixtures/flowchart_ports_heavy.mmd");
const SCALE_POINTS: [usize; 6] = [1, 2, 4, 10, 32, 100];

#[derive(Debug, Clone, Copy)]
enum Curve {
    Nodes,
    Edges,
    Clusters,
    Depth,
    Sparse,
    Dense,
}

impl Curve {
    const ALL: [Self; 6] = [
        Self::Nodes,
        Self::Edges,
        Self::Clusters,
        Self::Depth,
        Self::Sparse,
        Self::Dense,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Nodes => "nodes",
            Self::Edges => "edges",
            Self::Clusters => "clusters",
            Self::Depth => "depth",
            Self::Sparse => "topology_sparse",
            Self::Dense => "topology_dense",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct FlowchartSpec {
    scale: usize,
    nodes: usize,
    edges: usize,
    clusters: usize,
    depth: usize,
}

impl FlowchartSpec {
    fn for_curve(curve: Curve, scale: usize) -> Self {
        let (nodes, edges, clusters, depth) = match curve {
            Curve::Nodes => (3 * scale, 2, 1, 1),
            Curve::Edges => (32, 4 * scale, 4, 2),
            Curve::Clusters => (101, 32, scale, 1),
            Curve::Depth => (101, 16, 100, scale),
            Curve::Sparse => {
                let nodes = scale + 2;
                (nodes, nodes - 1, 2, 1)
            }
            Curve::Dense => {
                let nodes = scale + 2;
                let edges = (4 * (nodes - 1)).min(nodes * (nodes - 1));
                (nodes, edges, 2, 1)
            }
        };
        Self {
            scale,
            nodes,
            edges,
            clusters,
            depth,
        }
    }

    fn source(self) -> String {
        assert!(self.nodes > 1);
        assert!(self.clusters > 0);
        assert!((1..=self.clusters).contains(&self.depth));
        assert!(self.edges <= self.nodes * (self.nodes - 1));

        let mut parent_by_cluster = vec![None; self.clusters];
        for (cluster, parent) in parent_by_cluster
            .iter_mut()
            .enumerate()
            .take(self.depth)
            .skip(1)
        {
            *parent = Some(cluster - 1);
        }

        let mut children_by_cluster = vec![Vec::new(); self.clusters];
        for (cluster, parent) in parent_by_cluster.iter().copied().enumerate() {
            if let Some(parent) = parent {
                children_by_cluster[parent].push(cluster);
            }
        }

        let mut nodes_by_cluster = vec![Vec::new(); self.clusters];
        for node in 0..self.nodes {
            nodes_by_cluster[node % self.clusters].push(node);
        }

        let mut source =
            String::with_capacity(64 + self.nodes * 24 + self.edges * 28 + self.clusters * 28);
        source.push_str("flowchart LR\n");
        for root in 0..self.clusters {
            if parent_by_cluster[root].is_none() {
                emit_cluster(
                    &mut source,
                    root,
                    1,
                    &children_by_cluster,
                    &nodes_by_cluster,
                );
            }
        }

        // Enumerate directed endpoint pairs by distance, then source. This yields no self-loops or
        // duplicate pairs while preserving deterministic edge order at every scale.
        for edge in 0..self.edges {
            let from = edge % self.nodes;
            let distance = 1 + edge / self.nodes;
            let to = (from + distance) % self.nodes;
            writeln!(&mut source, "  n{from} --> n{to}").expect("write to String");
        }
        source
    }

    fn case_id(self, source: &str) -> String {
        let digest = format!("{:x}", Sha256::digest(source.as_bytes()));
        format!(
            "s{}_n{}_e{}_c{}_d{}_sha{}",
            self.scale,
            self.nodes,
            self.edges,
            self.clusters,
            self.depth,
            &digest[..12]
        )
    }
}

fn emit_cluster(
    source: &mut String,
    cluster: usize,
    indentation: usize,
    children_by_cluster: &[Vec<usize>],
    nodes_by_cluster: &[Vec<usize>],
) {
    let padding = "  ".repeat(indentation);
    writeln!(source, "{padding}subgraph c{cluster}[C{cluster}]").expect("write to String");
    for &node in &nodes_by_cluster[cluster] {
        writeln!(source, "{padding}  n{node}[N{node}]").expect("write to String");
    }
    for &child in &children_by_cluster[cluster] {
        emit_cluster(
            source,
            child,
            indentation + 1,
            children_by_cluster,
            nodes_by_cluster,
        );
    }
    writeln!(source, "{padding}end").expect("write to String");
}

fn bench_registered_operation_gates(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::strict();
    let layout = headless_layout_options();
    let environment = RenderEnvironment::deterministic();
    let cases = [
        ("flowchart_large", FLOWCHART_LARGE),
        ("flowchart_ports_heavy", FLOWCHART_PORTS_HEAVY),
        ("class_medium", CLASS_MEDIUM),
    ];

    let mut public = c.benchmark_group("end_to_end");
    for (name, source) in cases {
        let svg_opts = SvgRenderOptions {
            diagram_id: Some(merman::svg::sanitize_svg_id(name)),
            ..SvgRenderOptions::default()
        };
        public.bench_with_input(BenchmarkId::from_parameter(name), source, |b, source| {
            b.iter(|| {
                let svg = merman::svg::render_svg_sync(
                    &engine,
                    black_box(source),
                    parse_opts,
                    &layout,
                    &svg_opts,
                )
                .expect("render")
                .expect("supported diagram");
                black_box(svg.len());
            });
        });
    }
    public.finish();

    let mut prepare = c.benchmark_group("layout");
    for (name, source) in cases {
        let parsed = engine
            .parse_diagram_for_render_model_sync(source, parse_opts)
            .expect("parse")
            .expect("supported diagram");
        prepare.bench_with_input(BenchmarkId::from_parameter(name), &parsed, |b, parsed| {
            b.iter_batched(
                || {
                    (
                        (*parsed).clone(),
                        environment.begin_session().expect("render session"),
                    )
                },
                |(parsed, session)| {
                    black_box(
                        merman_render::family::prepare(parsed, &layout, session).expect("prepare"),
                    );
                },
                BatchSize::SmallInput,
            );
        });
    }
    prepare.finish();
}

fn bench_flowchart_curves(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::strict();
    let layout = headless_layout_options();
    let environment = RenderEnvironment::deterministic();

    for curve in Curve::ALL {
        let mut public = c.benchmark_group(format!("flowchart_public_{}", curve.name()));
        for scale in SCALE_POINTS {
            let spec = FlowchartSpec::for_curve(curve, scale);
            let source = spec.source();
            let case_id = spec.case_id(&source);
            public.bench_with_input(
                BenchmarkId::from_parameter(case_id),
                &source,
                |b, source| {
                    let svg_opts = SvgRenderOptions {
                        diagram_id: Some("flowchart-curve".to_string()),
                        ..SvgRenderOptions::default()
                    };
                    b.iter(|| {
                        let svg = merman::svg::render_svg_sync(
                            &engine,
                            black_box(source.as_str()),
                            parse_opts,
                            &layout,
                            &svg_opts,
                        )
                        .expect("render")
                        .expect("supported diagram");
                        black_box(svg.len());
                    });
                },
            );
        }
        public.finish();

        let mut prepare = c.benchmark_group(format!("flowchart_prepare_{}", curve.name()));
        for scale in SCALE_POINTS {
            let spec = FlowchartSpec::for_curve(curve, scale);
            let source = spec.source();
            let case_id = spec.case_id(&source);
            let parsed = engine
                .parse_diagram_for_render_model_sync(&source, parse_opts)
                .expect("parse")
                .expect("supported diagram");
            prepare.bench_with_input(
                BenchmarkId::from_parameter(case_id),
                &parsed,
                |b, parsed| {
                    b.iter_batched(
                        || {
                            (
                                (*parsed).clone(),
                                environment.begin_session().expect("render session"),
                            )
                        },
                        |(parsed, session)| {
                            black_box(
                                merman_render::family::prepare(parsed, &layout, session)
                                    .expect("prepare"),
                            );
                        },
                        BatchSize::SmallInput,
                    );
                },
            );
        }
        prepare.finish();
    }
}

fn bench_emit_svg_controls(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::strict();
    let layout: LayoutOptions = headless_layout_options();
    let environment = RenderEnvironment::deterministic();
    let mut group = c.benchmark_group("emit_svg_stress");

    for (name, input) in [
        ("flowchart_medium", FLOWCHART_MEDIUM),
        ("flowchart_ports_heavy", FLOWCHART_PORTS_HEAVY),
    ] {
        let parsed = engine
            .parse_diagram_for_render_model_sync(input, parse_opts)
            .expect("parse")
            .expect("supported diagram");
        let svg_opts = SvgRenderOptions {
            diagram_id: Some(merman::svg::sanitize_svg_id(name)),
            ..SvgRenderOptions::default()
        };
        group.bench_function(name, |b| {
            b.iter_batched(
                || {
                    merman_render::family::prepare(
                        parsed.clone(),
                        &layout,
                        environment.begin_session().expect("render session"),
                    )
                    .expect("prepare")
                },
                |artifact| {
                    let svg = artifact
                        .render_svg(&svg_opts, &SvgDebugOptions::default())
                        .expect("render");
                    black_box(svg.svg().len());
                },
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_registered_operation_gates,
    bench_flowchart_curves,
    bench_emit_svg_controls
);
criterion_main!(benches);

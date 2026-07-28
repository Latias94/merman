use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use merman::svg::{
    LayoutOptions, RenderEnvironment, SvgDebugOptions, SvgRenderOptions, headless_layout_options,
};
use merman_core::resources::FlowchartComplexity;
use merman_core::{Engine, ParseOptions, ParsedDiagramRender, RenderSemanticModel};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
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
    BoundedDegree,
    Dense,
}

impl Curve {
    const ALL: [Self; 7] = [
        Self::Nodes,
        Self::Edges,
        Self::Clusters,
        Self::Depth,
        Self::Sparse,
        Self::BoundedDegree,
        Self::Dense,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Nodes => "nodes",
            Self::Edges => "edges",
            Self::Clusters => "clusters",
            Self::Depth => "depth",
            Self::Sparse => "topology_sparse",
            Self::BoundedDegree => "topology_bounded_degree",
            Self::Dense => "topology_dense",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EdgeTopology {
    AllOrdinaryNodes,
    PreserveIsolatedCluster,
}

#[derive(Debug, Clone, Copy)]
struct FlowchartSpec {
    scale: Option<usize>,
    // This is the public resource-model N: ordinary nodes plus subgraph nodes.
    nodes: usize,
    edges: usize,
    clusters: usize,
    depth: usize,
    edge_topology: EdgeTopology,
}

impl FlowchartSpec {
    fn for_curve(curve: Curve, scale: usize) -> Self {
        let (nodes, edges, clusters, depth) = match curve {
            Curve::Nodes => (3 * scale, 2, 1, 1),
            Curve::Edges => (32, 4 * scale, 4, 2),
            Curve::Clusters => (201, 32, scale, 1),
            Curve::Depth => (201, 16, 100, scale),
            Curve::Sparse => {
                let ordinary_nodes = scale + 2;
                (ordinary_nodes + 2, ordinary_nodes - 1, 2, 1)
            }
            Curve::BoundedDegree => {
                let ordinary_nodes = scale + 2;
                let edges = (4 * (ordinary_nodes - 1)).min(ordinary_nodes * (ordinary_nodes - 1));
                (ordinary_nodes + 2, edges, 2, 1)
            }
            Curve::Dense => {
                // Keep the same modest vertex curve as the sparse/bounded-degree controls. At
                // 100x this is 102 ordinary vertices and 2,575 directed edges, large enough to
                // expose density costs without constructing the complete 10,302-edge digraph.
                let ordinary_nodes = scale + 2;
                let edges = ordinary_nodes * (ordinary_nodes - 1) / 4;
                (ordinary_nodes + 2, edges, 2, 1)
            }
        };
        Self {
            scale: Some(scale),
            nodes,
            edges,
            clusters,
            depth,
            edge_topology: EdgeTopology::AllOrdinaryNodes,
        }
    }

    fn for_cluster_edge_panel(clusters: usize, edges: usize) -> Self {
        assert!([4, 100].contains(&clusters));
        assert!([32, 400].contains(&edges));
        Self {
            scale: None,
            nodes: 201,
            edges,
            clusters,
            depth: 1,
            edge_topology: EdgeTopology::PreserveIsolatedCluster,
        }
    }

    fn ordinary_nodes(self) -> usize {
        self.nodes
            .checked_sub(self.clusters)
            .expect("N includes one node for every subgraph")
    }

    fn source(self) -> String {
        assert!(self.clusters > 0);
        assert!((1..=self.clusters).contains(&self.depth));
        let ordinary_nodes = self.ordinary_nodes();
        assert!(ordinary_nodes > 1);

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
        for node in 0..ordinary_nodes {
            nodes_by_cluster[node % self.clusters].push(node);
        }

        let edge_nodes = match self.edge_topology {
            EdgeTopology::AllOrdinaryNodes => (0..ordinary_nodes).collect::<Vec<_>>(),
            EdgeTopology::PreserveIsolatedCluster => {
                let isolated_cluster = self.clusters - 1;
                assert!(!nodes_by_cluster[isolated_cluster].is_empty());
                (0..ordinary_nodes)
                    .filter(|node| node % self.clusters != isolated_cluster)
                    .collect::<Vec<_>>()
            }
        };
        assert!(edge_nodes.len() > 1);
        assert!(self.edges <= edge_nodes.len() * (edge_nodes.len() - 1));

        let mut source =
            String::with_capacity(64 + ordinary_nodes * 24 + self.edges * 28 + self.clusters * 28);
        source.push_str("flowchart LR\n");
        for (root, parent) in parent_by_cluster.iter().enumerate() {
            if parent.is_none() {
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
            let from_index = edge % edge_nodes.len();
            let distance = 1 + edge / edge_nodes.len();
            let to_index = (from_index + distance) % edge_nodes.len();
            let from = edge_nodes[from_index];
            let to = edge_nodes[to_index];
            writeln!(&mut source, "  n{from} --> n{to}").expect("write to String");
        }
        source
    }

    fn case_id(self, digest: &str) -> String {
        let dimensions = format!(
            "n{}_e{}_c{}_d{}_sha{}",
            self.nodes,
            self.edges,
            self.clusters,
            self.depth,
            &digest[..12]
        );
        self.scale
            .map_or(dimensions.clone(), |scale| format!("s{scale}_{dimensions}"))
    }
}

struct SyntheticCase {
    source: String,
    case_id: String,
    parsed: ParsedDiagramRender,
}

fn source_digest(source: &str) -> String {
    format!("{:x}", Sha256::digest(source.as_bytes()))
}

fn assert_cluster_edge_panel_roles(model: &merman_core::diagrams::flowchart::FlowchartModel) {
    let subgraph_ids = model
        .subgraphs
        .iter()
        .map(|subgraph| subgraph.id.as_str())
        .collect::<HashSet<_>>();
    let mut boundary_connected = 0usize;
    let mut isolated = 0usize;
    let mut extractable = 0usize;

    for subgraph in &model.subgraphs {
        let members = subgraph
            .nodes
            .iter()
            .map(String::as_str)
            .filter(|member| !subgraph_ids.contains(member))
            .collect::<HashSet<_>>();
        assert!(!members.is_empty(), "panel clusters must contain a node");

        let has_boundary_edge = model
            .edges
            .iter()
            .any(|edge| members.contains(edge.from.as_str()) ^ members.contains(edge.to.as_str()));
        let has_incident_edge = model
            .edges
            .iter()
            .any(|edge| members.contains(edge.from.as_str()) || members.contains(edge.to.as_str()));
        boundary_connected += usize::from(has_boundary_edge);
        isolated += usize::from(!has_incident_edge);
        extractable += usize::from(!has_boundary_edge);
    }

    assert!(
        boundary_connected > 0,
        "panel needs a boundary-connected cluster"
    );
    assert!(isolated > 0, "panel needs an isolated cluster");
    assert!(extractable > 0, "panel needs an extractable cluster");
}

fn validated_synthetic_case(
    engine: &Engine,
    parse_opts: ParseOptions,
    spec: FlowchartSpec,
) -> SyntheticCase {
    let source = spec.source();
    let repeated_source = spec.source();
    assert_eq!(
        source, repeated_source,
        "synthetic source must be deterministic"
    );

    let digest = source_digest(&source);
    assert_eq!(digest, source_digest(&repeated_source));
    let case_id = spec.case_id(&digest);

    let parsed = engine
        .parse_diagram_for_render_model_sync(&source, parse_opts)
        .expect("parse synthetic flowchart")
        .expect("supported synthetic flowchart");
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("synthetic flowchart must produce a typed flowchart model");
    };
    let complexity = FlowchartComplexity::from_model(model);
    assert_eq!(
        (
            complexity.nodes,
            complexity.edges,
            complexity.subgraphs,
            complexity.subgraph_depth,
        ),
        (spec.nodes, spec.edges, spec.clusters, spec.depth),
        "synthetic case dimensions must match its case id: {case_id}"
    );

    let mut endpoints = HashSet::with_capacity(model.edges.len());
    for edge in &model.edges {
        assert!(
            endpoints.insert((edge.from.as_str(), edge.to.as_str())),
            "synthetic case contains duplicate directed endpoints: {case_id} {} -> {}",
            edge.from,
            edge.to
        );
    }
    if spec.edge_topology == EdgeTopology::PreserveIsolatedCluster {
        assert_cluster_edge_panel_roles(model);
    }

    SyntheticCase {
        source,
        case_id,
        parsed,
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
        ("flowchart_medium", FLOWCHART_MEDIUM),
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

#[allow(clippy::too_many_arguments)]
fn register_synthetic_cases(
    c: &mut Criterion,
    public_group_name: &str,
    prepare_group_name: &str,
    diagram_id: &str,
    cases: &[SyntheticCase],
    engine: &Engine,
    parse_opts: ParseOptions,
    layout: &LayoutOptions,
    environment: &RenderEnvironment,
) {
    let mut public = c.benchmark_group(public_group_name);
    for case in cases {
        let svg_opts = SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        };
        public.bench_with_input(
            BenchmarkId::from_parameter(case.case_id.as_str()),
            &case.source,
            |b, source| {
                b.iter(|| {
                    let svg = merman::svg::render_svg_sync(
                        engine,
                        black_box(source.as_str()),
                        parse_opts,
                        layout,
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

    let mut prepare = c.benchmark_group(prepare_group_name);
    for case in cases {
        prepare.bench_with_input(
            BenchmarkId::from_parameter(case.case_id.as_str()),
            &case.parsed,
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
                            merman_render::family::prepare(parsed, layout, session)
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

fn bench_flowchart_curves(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::strict();
    let layout = headless_layout_options();
    let environment = RenderEnvironment::deterministic();

    // N is FlowchartComplexity::nodes, so each declared case includes its subgraph count. Build
    // and validate every source before registering any Criterion input; a bad generator therefore
    // fails as harness setup rather than producing a mislabeled timing row.
    let curve_cases = Curve::ALL
        .into_iter()
        .map(|curve| {
            let cases = SCALE_POINTS
                .into_iter()
                .map(|scale| {
                    validated_synthetic_case(
                        &engine,
                        parse_opts,
                        FlowchartSpec::for_curve(curve, scale),
                    )
                })
                .collect::<Vec<_>>();
            (curve, cases)
        })
        .collect::<Vec<_>>();
    let panel_cases = [4, 100]
        .into_iter()
        .flat_map(|clusters| [32, 400].into_iter().map(move |edges| (clusters, edges)))
        .map(|(clusters, edges)| {
            validated_synthetic_case(
                &engine,
                parse_opts,
                FlowchartSpec::for_cluster_edge_panel(clusters, edges),
            )
        })
        .collect::<Vec<_>>();

    for (curve, cases) in &curve_cases {
        let public_group_name = format!("flowchart_public_{}", curve.name());
        let prepare_group_name = format!("flowchart_prepare_{}", curve.name());
        register_synthetic_cases(
            c,
            &public_group_name,
            &prepare_group_name,
            "flowchart-curve",
            cases,
            &engine,
            parse_opts,
            &layout,
            &environment,
        );
    }

    register_synthetic_cases(
        c,
        "flowchart_public_cluster_edge_panel",
        "flowchart_prepare_cluster_edge_panel",
        "flowchart-cluster-edge-panel",
        &panel_cases,
        &engine,
        parse_opts,
        &layout,
        &environment,
    );
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

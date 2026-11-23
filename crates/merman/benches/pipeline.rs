use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use merman::model::ManagedJson;
use merman::svg::{LayoutOptions, RenderResourcePolicy, SvgDebugOptions, SvgRenderOptions};
use merman_core::{DetectorRegistry, MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{hint::black_box, sync::OnceLock};

#[path = "support/json_identity.rs"]
mod json_identity;

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutputIdentity {
    output_kind: &'static str,
    output_bytes: usize,
    output_sha256: String,
    svg_elements: Option<usize>,
}

fn sha256_bytes(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn output_identity(
    output_kind: &str,
    output: &[u8],
    svg_elements: Option<usize>,
) -> OutputIdentity {
    assert!(!output.is_empty(), "benchmark produced empty output");
    OutputIdentity {
        output_kind: match output_kind {
            "typed_render_model" => "typed_render_model",
            "editor_snapshot" => "editor_snapshot",
            "compatibility_json" => "compatibility_json",
            "preprocessed_diagram" => "preprocessed_diagram",
            "prepared_layout" => "prepared_layout",
            "svg" => "svg",
            _ => panic!("unknown benchmark output kind: {output_kind}"),
        },
        output_bytes: output.len(),
        output_sha256: sha256_bytes(output),
        svg_elements,
    }
}

fn emit_preflight(group: &str, name: &str, identity: &OutputIdentity) {
    let receipt = json!({
        "schema_version": 1,
        "benchmark": format!("{group}/{name}"),
        "output_kind": identity.output_kind,
        "output_bytes": identity.output_bytes,
        "output_sha256": identity.output_sha256,
        "svg_elements": identity.svg_elements,
    });
    eprintln!(
        "[bench][preflight] {}",
        serde_json::to_string(&receipt).expect("preflight receipt must serialize")
    );
}

fn json_output_identity(output_kind: &str, value: &ManagedJson) -> OutputIdentity {
    let output = json_identity::json_identity_bytes(value);
    output_identity(output_kind, &output, None)
}

fn svg_output_identity(svg: &str) -> OutputIdentity {
    let document = roxmltree::Document::parse(svg).expect("benchmark output must be valid SVG XML");
    assert_eq!(
        document.root_element().tag_name().name(),
        "svg",
        "benchmark output root must be svg"
    );
    let element_count = document
        .descendants()
        .filter(|node| node.is_element())
        .count();
    output_identity("svg", svg.as_bytes(), Some(element_count))
}

fn exact_benchmark() -> Option<&'static str> {
    static EXACT: OnceLock<Option<String>> = OnceLock::new();
    EXACT
        .get_or_init(|| {
            let arguments = std::env::args().collect::<Vec<_>>();
            arguments
                .windows(2)
                .find(|pair| pair[0] == "--exact")
                .map(|pair| pair[1].clone())
        })
        .as_deref()
}

fn benchmark_selected(group: &str, name: &str) -> bool {
    exact_benchmark().is_none_or(|exact| {
        exact
            .strip_prefix(group)
            .and_then(|value| value.strip_prefix('/'))
            == Some(name)
    })
}

fn verify_postflight(
    group: &str,
    name: &str,
    preflight: &OutputIdentity,
    postflight: impl FnOnce() -> OutputIdentity,
) {
    let Some(exact) = exact_benchmark() else {
        return;
    };
    let Some(suffix) = exact
        .strip_prefix(group)
        .and_then(|value| value.strip_prefix('/'))
    else {
        return;
    };
    if suffix != name {
        return;
    }
    assert_eq!(
        &postflight(),
        preflight,
        "{group}/{name} output identity changed after timed iterations"
    );
    eprintln!("[bench][postflight] {group}/{name}");
}

macro_rules! fixture {
    ($name:literal) => {
        ($name, include_str!(concat!("fixtures/", $name, ".mmd")))
    };
    ($name:literal, $path:literal) => {
        ($name, include_str!($path))
    };
}

// Generated from tools/bench/corpus.json by tools/bench/pipeline_fixtures.py --write.
#[rustfmt::skip]
const DIAGRAM_FIXTURES: &[(&str, &str)] = &[
    fixture!("flowchart_weave_dagre_classic"),
    fixture!("flowchart_ports_heavy_dagre_classic"),
    fixture!("flowchart_fanout_returns_dagre_classic"),
    fixture!("flowchart_long_edge_labels_dagre_classic"),
    fixture!("flowchart_medium_dagre_classic"),
    fixture!("flowchart_nested_clusters_dagre_classic"),
    fixture!("flowchart_tiny"),
    fixture!("flowchart_small"),
    fixture!("flowchart_medium"),
    fixture!("flowchart_large"),
    fixture!("flowchart_ports_heavy"),
    fixture!("flowchart_weave"),
    fixture!("flowchart_backedges_subgraphs"),
    fixture!("flowchart_sparse_components"),
    fixture!("flowchart_lanes_crossfeed"),
    fixture!("flowchart_grid_feedback"),
    fixture!("flowchart_fanout_returns"),
    fixture!("flowchart_label_collision"),
    fixture!("flowchart_nested_clusters"),
    fixture!("flowchart_elk_nested_directions", "support/corpus/flowchart_elk_nested_directions.mmd"),
    fixture!("flowchart_asymmetric_components"),
    fixture!("flowchart_parallel_merges"),
    fixture!("flowchart_long_edge_labels"),
    fixture!("flowchart_svg_label_reuse"),
    fixture!("flowchart_selfloop_bidi"),
    fixture!("flowchart_component_packing"),
    fixture!("flowchart_direction_conflict"),
    fixture!("flowchart_parallel_label_stack"),
    fixture!("class_tiny"),
    fixture!("class_medium"),
    fixture!("class_namespace_dense", "fixtures/stress_class_dense_namespaces_generics_001.mmd"),
    fixture!("class_nested_namespaces"),
    fixture!("class_nested_namespaces_large"),
    fixture!("state_tiny"),
    fixture!("state_medium"),
    fixture!("sequence_tiny"),
    fixture!("sequence_medium"),
    fixture!("sequence_actor_only_control"),
    fixture!("sequence_block_repeat_low"),
    fixture!("sequence_block_repeat_medium"),
    fixture!("sequence_block_repeat_high"),
    fixture!("sequence_block_unique_high"),
    fixture!("er_medium"),
    fixture!("info_medium"),
    fixture!("pie_medium"),
    fixture!("mindmap_medium"),
    fixture!("journey_medium"),
    fixture!("timeline_medium"),
    fixture!("gantt_medium"),
    fixture!("requirement_medium"),
    fixture!("gitgraph_medium"),
    fixture!("c4_medium"),
    fixture!("sankey_medium"),
    fixture!("quadrant_medium"),
    fixture!("zenuml_medium"),
    fixture!("block_medium"),
    fixture!("packet_medium"),
    fixture!("kanban_medium"),
    fixture!("architecture_medium"),
    fixture!("radar_medium"),
    fixture!("treemap_medium"),
    fixture!("xychart_medium"),
    fixture!("venn_medium"),
    fixture!("swimlane_medium"),
    fixture!("eventmodeling_medium"),
    fixture!("treeview_medium"),
    fixture!("ishikawa_medium"),
    fixture!("railroad_medium"),
    fixture!("railroad_abnf_medium"),
    fixture!("railroad_ebnf_medium"),
    fixture!("railroad_peg_medium"),
    fixture!("wardley_medium"),
    fixture!("cynefin_medium"),
    fixture!("error_basic"),
    fixture!("agentflow_basic", "support/corpus/agentflow_basic.mmd"),
    fixture!("usecase_basic", "support/corpus/usecase_basic.mmd"),
];

#[rustfmt::skip]
const FRONTMATTER_FIXTURES: &[(&str, &str)] = &[
    fixture!("frontmatter_basic"),
    fixture!("frontmatter_indented"),
    fixture!("frontmatter_deep_config"),
];
// End of generated pipeline fixtures.

fn fixtures(group: &str) -> impl Iterator<Item = (&'static str, &'static str)> {
    let entries = if group == "frontmatter_preprocess" {
        FRONTMATTER_FIXTURES
    } else {
        DIAGRAM_FIXTURES
    };
    entries
        .iter()
        .copied()
        .filter(move |(name, _)| benchmark_selected(group, name))
}

fn bench_parse(c: &mut Criterion) {
    let engine = MermaidEngine::new();
    let parse_opts = ParseOptions::strict();

    let mut group = c.benchmark_group("parse");
    for (name, input) in fixtures("parse") {
        let output_identity = || {
            let parsed = engine
                .parse_diagram_for_render_model_sync(input, parse_opts)
                .unwrap_or_else(|error| panic!("parse/{name} preflight failed: {error}"))
                .unwrap_or_else(|| panic!("parse/{name} preflight returned no render model"));
            let projection = parsed
                .compatibility_json()
                .unwrap_or_else(|error| panic!("parse/{name} projection failed: {error}"));
            json_output_identity("typed_render_model", &projection)
        };
        let preflight = output_identity();
        emit_preflight("parse", name, &preflight);
        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let parsed = engine
                    .parse_diagram_for_render_model_sync(black_box(data), parse_opts)
                    .expect("parse benchmark")
                    .expect("parse benchmark render model");
                black_box(parsed);
            })
        });
        verify_postflight("parse", name, &preflight, output_identity);
    }
    group.finish();
}

fn bench_compatibility_json_parse(c: &mut Criterion) {
    let engine = MermaidEngine::new();
    let parse_opts = ParseOptions::strict();

    let mut group = c.benchmark_group("compatibility_json_parse");
    for (name, input) in fixtures("compatibility_json_parse") {
        let diagram_type = engine
            .parse_metadata_sync(input)
            .unwrap_or_else(|error| {
                panic!("compatibility_json_parse/{name} metadata preflight failed: {error}")
            })
            .diagram_type;
        let output_identity = || {
            let parsed = engine
                .parse_diagram_with_type_sync(&diagram_type, input, parse_opts)
                .unwrap_or_else(|error| {
                    panic!(
                        "compatibility_json_parse/{name} parse_with_type({diagram_type}) preflight failed: {error}"
                    )
                })
                .unwrap_or_else(|| {
                    panic!("compatibility_json_parse/{name} preflight returned no JSON model")
                });
            json_output_identity("compatibility_json", &parsed.model)
        };
        let preflight = output_identity();
        emit_preflight("compatibility_json_parse", name, &preflight);

        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let parsed = engine
                    .parse_diagram_with_type_sync(&diagram_type, black_box(data), parse_opts)
                    .expect("compatibility JSON parse benchmark")
                    .expect("compatibility JSON parse benchmark model");
                black_box(parsed);
            })
        });
        verify_postflight(
            "compatibility_json_parse",
            name,
            &preflight,
            output_identity,
        );
    }
    group.finish();
}

fn bench_parse_cold_engine(c: &mut Criterion) {
    let parse_opts = ParseOptions::strict();

    let mut group = c.benchmark_group("parse_cold_engine");
    for (name, input) in fixtures("parse_cold_engine") {
        // Tracks request-style usage that constructs a fresh MermaidEngine per parse, so parse wins are
        // not accidentally attributed only to the hot shared-engine benchmark shape.
        let output_identity = || {
            let engine = MermaidEngine::new();
            let parsed = engine
                .parse_diagram_for_render_model_sync(input, parse_opts)
                .unwrap_or_else(|error| {
                    panic!("parse_cold_engine/{name} preflight failed: {error}")
                })
                .unwrap_or_else(|| {
                    panic!("parse_cold_engine/{name} preflight returned no render model")
                });
            let projection = parsed.compatibility_json().unwrap_or_else(|error| {
                panic!("parse_cold_engine/{name} projection failed: {error}")
            });
            json_output_identity("typed_render_model", &projection)
        };
        let preflight = output_identity();
        emit_preflight("parse_cold_engine", name, &preflight);

        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let engine = MermaidEngine::new();
                let parsed = engine
                    .parse_diagram_for_render_model_sync(black_box(data), parse_opts)
                    .expect("cold-engine parse benchmark")
                    .expect("cold-engine parse benchmark render model");
                black_box(parsed);
            })
        });
        verify_postflight("parse_cold_engine", name, &preflight, output_identity);
    }
    group.finish();
}

fn bench_editor_snapshot(c: &mut Criterion) {
    let engine = MermaidEngine::new();
    let mut group = c.benchmark_group("editor_snapshot");
    for (name, input) in fixtures("editor_snapshot") {
        let output_identity = || {
            let snapshot = engine
                .parse_diagram_snapshot_sync(input)
                .unwrap_or_else(|error| panic!("editor_snapshot/{name} failed: {error}"))
                .unwrap_or_else(|| panic!("editor_snapshot/{name} returned no snapshot"));
            let model = snapshot
                .outcome()
                .parsed_model()
                .unwrap_or_else(|| panic!("editor_snapshot/{name} retained a failed parse"));
            let mut model_bytes = Vec::new();
            model.write_json(&mut model_bytes).expect("snapshot JSON");
            let mut canonical_model: serde_json::Value =
                serde_json::from_slice(&model_bytes).expect("snapshot model JSON");
            // Object member order is not meaning (State stores records in a HashMap).
            // Preserve every array order and value while stabilizing this identity only.
            canonical_model.sort_all_objects();
            let merman_core::DiagramParseOutcome::Parsed { warning_facts, .. } = snapshot.outcome()
            else {
                unreachable!("successful snapshot checked above")
            };
            // Facts have no wire serializer. Debug captures all their typed fields for this
            // harness-only identity; neither formatting nor JSON export enters the timed call.
            let mut projection = json!({
                "diagram_type": snapshot.metadata().diagram_type,
                "config": snapshot.metadata().config.as_value(),
                "effective_config": snapshot.metadata().effective_config.as_value(),
                "model": canonical_model,
                "warnings": format!("{warning_facts:?}"),
                "facts": format!("{:?}", snapshot.editor_facts()),
                "source_config": format!("{:?}", snapshot.source_config()),
                "recovered_directive": snapshot.recovered_incomplete_directive(),
            });
            projection.sort_all_objects();
            json_output_identity("editor_snapshot", &projection.into())
        };
        let preflight = output_identity();
        emit_preflight("editor_snapshot", name, &preflight);
        group.bench_with_input(BenchmarkId::from_parameter(name), &input, |b, data| {
            b.iter(|| {
                black_box(
                    engine
                        .parse_diagram_snapshot_sync(black_box(data))
                        .expect("editor snapshot benchmark")
                        .expect("editor snapshot benchmark result"),
                );
            });
        });
        verify_postflight("editor_snapshot", name, &preflight, output_identity);
    }
    group.finish();
}

fn bench_frontmatter_preprocess(c: &mut Criterion) {
    let registry = DetectorRegistry::pinned_mermaid_baseline();

    let mut group = c.benchmark_group("frontmatter_preprocess");
    for (name, input) in fixtures("frontmatter_preprocess") {
        let output_identity = || {
            let pre = merman_core::preprocess_diagram_with_known_type(
                input,
                &registry,
                Some("flowchart-v2"),
            )
            .unwrap_or_else(|error| {
                panic!("frontmatter_preprocess/{name} preflight failed: {error}")
            });
            let projection = json!({
                "code": pre.code(),
                "title": pre.title.as_deref(),
                "config": pre.config.as_value(),
            });
            json_output_identity("preprocessed_diagram", &projection.into())
        };
        let preflight = output_identity();
        emit_preflight("frontmatter_preprocess", name, &preflight);

        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let pre = merman_core::preprocess_diagram_with_known_type(
                    black_box(data),
                    &registry,
                    Some("flowchart-v2"),
                )
                .expect("frontmatter preprocess benchmark");
                black_box(pre.code().len());
                black_box(pre.title.as_deref().map_or(0, str::len));
                black_box(pre.config.as_value().as_object().map_or(0, |map| map.len()));
            })
        });
        verify_postflight("frontmatter_preprocess", name, &preflight, output_identity);
    }
    group.finish();
}

fn bench_layout(c: &mut Criterion) {
    let engine = MermaidEngine::new();
    let parse_opts = ParseOptions::strict();
    let layout = LayoutOptions::headless_svg_defaults();
    // Native batch fixtures include layouts beyond the interactive preview budget.
    let environment = RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::trusted_native());

    let mut group = c.benchmark_group("layout");
    for (name, input) in fixtures("layout") {
        let parsed = engine
            .parse_diagram_for_render_model_sync(input, parse_opts)
            .unwrap_or_else(|error| panic!("layout/{name} parse preflight failed: {error}"))
            .unwrap_or_else(|| panic!("layout/{name} preflight returned no render model"));
        let output_identity = || {
            let artifact = merman_render::family::prepare(
                parsed.presentation(),
                &layout,
                environment.begin_session().expect("render session"),
            )
            .unwrap_or_else(|error| panic!("layout/{name} preflight failed: {error}"));
            let projection = artifact
                .layout_json()
                .unwrap_or_else(|error| panic!("layout/{name} projection failed: {error}"));
            json_output_identity("prepared_layout", &projection)
        };
        let preflight = output_identity();
        emit_preflight("layout", name, &preflight);

        group.bench_with_input(BenchmarkId::from_parameter(name), &parsed, |b, data| {
            b.iter_batched(
                || (data, environment.begin_session().expect("render session")),
                |(parsed, session)| {
                    let artifact =
                        merman_render::family::prepare(parsed.presentation(), &layout, session)
                            .expect("layout");
                    black_box(artifact);
                },
                BatchSize::SmallInput,
            )
        });
        verify_postflight("layout", name, &preflight, output_identity);
    }
    group.finish();
}

fn bench_render(c: &mut Criterion) {
    let engine = MermaidEngine::new();
    let parse_opts = ParseOptions::strict();
    let layout = LayoutOptions::headless_svg_defaults();
    // Native batch fixtures include layouts beyond the interactive preview budget.
    let environment = RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::trusted_native());

    let mut group = c.benchmark_group("render");
    for (name, input) in fixtures("render") {
        let parsed = engine
            .parse_diagram_for_render_model_sync(input, parse_opts)
            .unwrap_or_else(|error| panic!("render/{name} parse preflight failed: {error}"))
            .unwrap_or_else(|| panic!("render/{name} preflight returned no render model"));
        let svg_opts = SvgRenderOptions {
            diagram_id: Some(merman::svg::sanitize_svg_id(name)),
            ..SvgRenderOptions::default()
        };

        let output_identity = || {
            let artifact = merman_render::family::prepare(
                parsed.presentation(),
                &layout,
                environment.begin_session().expect("render session"),
            )
            .unwrap_or_else(|error| panic!("render/{name} layout preflight failed: {error}"));
            let rendered = artifact
                .render_svg(&svg_opts, &SvgDebugOptions::default())
                .unwrap_or_else(|error| panic!("render/{name} SVG preflight failed: {error}"));
            svg_output_identity(rendered.svg())
        };
        let preflight = output_identity();
        emit_preflight("render", name, &preflight);

        group.bench_with_input(BenchmarkId::from_parameter(name), &parsed, |b, data| {
            b.iter_batched(
                || {
                    merman_render::family::prepare(
                        data.presentation(),
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
            )
        });
        verify_postflight("render", name, &preflight, output_identity);
    }
    group.finish();
}

fn bench_end_to_end(c: &mut Criterion) {
    let engine = MermaidEngine::new();
    let parse_opts = ParseOptions::strict();
    let layout = LayoutOptions::headless_svg_defaults();
    let resources = RenderResourcePolicy::trusted_native();
    let renderer = merman::Renderer::new()
        .with_presentation_engine(engine.clone())
        .with_parse_options(parse_opts)
        .with_resource_policy(*resources.input_policy());

    let mut group = c.benchmark_group("end_to_end");
    for (name, input) in fixtures("end_to_end") {
        let svg_opts = SvgRenderOptions {
            diagram_id: Some(merman::svg::sanitize_svg_id(name)),
            ..SvgRenderOptions::default()
        };
        let request = || merman::SvgRequest {
            environment: merman::SvgEnvironment::deterministic().with_resource_policy(resources),
            layout: layout.clone(),
            options: svg_opts.clone(),
            ..Default::default()
        };

        let output_identity = || {
            let output = renderer
                .render(merman::RenderRequest::svg(
                    input,
                    merman::OperationControl::new(),
                    request(),
                ))
                .unwrap_or_else(|error| panic!("end_to_end/{name} preflight failed: {error}"));
            let merman::RenderOutput::Svg(Some(svg)) = output else {
                panic!("end_to_end/{name} preflight returned no SVG");
            };
            svg_output_identity(svg.svg())
        };
        let preflight = output_identity();
        emit_preflight("end_to_end", name, &preflight);

        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let output = renderer
                    .render(merman::RenderRequest::svg(
                        black_box(data),
                        merman::OperationControl::new(),
                        request(),
                    ))
                    .expect("end-to-end benchmark render");
                let merman::RenderOutput::Svg(Some(svg)) = output else {
                    panic!("end-to-end benchmark SVG");
                };
                black_box(svg.svg().len());
            });
        });
        verify_postflight("end_to_end", name, &preflight, output_identity);
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_parse,
    bench_compatibility_json_parse,
    bench_parse_cold_engine,
    bench_editor_snapshot,
    bench_frontmatter_preprocess,
    bench_layout,
    bench_render,
    bench_end_to_end
);
criterion_main!(benches);

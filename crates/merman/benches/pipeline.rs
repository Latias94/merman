use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use merman::svg::{
    LayoutOptions, RenderEnvironment, SvgDebugOptions, SvgRenderOptions, headless_layout_options,
};
use merman_core::{DetectorRegistry, Engine, ParseOptions};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{hint::black_box, sync::OnceLock};

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutputIdentity {
    output_kind: &'static str,
    output_bytes: usize,
    output_sha256: String,
    svg_elements: Option<usize>,
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
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

fn json_output_identity(output_kind: &str, value: &Value) -> OutputIdentity {
    let output = serde_json::to_vec(value).expect("benchmark JSON output must serialize");
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

fn fixtures() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "flowchart_tiny",
            include_str!("fixtures/flowchart_tiny.mmd"),
        ),
        (
            "flowchart_small",
            include_str!("fixtures/flowchart_small.mmd"),
        ),
        (
            "flowchart_medium",
            include_str!("fixtures/flowchart_medium.mmd"),
        ),
        (
            "flowchart_large",
            include_str!("fixtures/flowchart_large.mmd"),
        ),
        (
            "flowchart_ports_heavy",
            include_str!("fixtures/flowchart_ports_heavy.mmd"),
        ),
        (
            "flowchart_weave",
            include_str!("fixtures/flowchart_weave.mmd"),
        ),
        (
            "flowchart_backedges_subgraphs",
            include_str!("fixtures/flowchart_backedges_subgraphs.mmd"),
        ),
        (
            "flowchart_sparse_components",
            include_str!("fixtures/flowchart_sparse_components.mmd"),
        ),
        (
            "flowchart_lanes_crossfeed",
            include_str!("fixtures/flowchart_lanes_crossfeed.mmd"),
        ),
        (
            "flowchart_grid_feedback",
            include_str!("fixtures/flowchart_grid_feedback.mmd"),
        ),
        (
            "flowchart_fanout_returns",
            include_str!("fixtures/flowchart_fanout_returns.mmd"),
        ),
        (
            "flowchart_label_collision",
            include_str!("fixtures/flowchart_label_collision.mmd"),
        ),
        (
            "flowchart_nested_clusters",
            include_str!("fixtures/flowchart_nested_clusters.mmd"),
        ),
        (
            "flowchart_asymmetric_components",
            include_str!("fixtures/flowchart_asymmetric_components.mmd"),
        ),
        (
            "flowchart_parallel_merges",
            include_str!("fixtures/flowchart_parallel_merges.mmd"),
        ),
        (
            "flowchart_long_edge_labels",
            include_str!("fixtures/flowchart_long_edge_labels.mmd"),
        ),
        (
            "flowchart_selfloop_bidi",
            include_str!("fixtures/flowchart_selfloop_bidi.mmd"),
        ),
        (
            "flowchart_component_packing",
            include_str!("fixtures/flowchart_component_packing.mmd"),
        ),
        (
            "flowchart_direction_conflict",
            include_str!("fixtures/flowchart_direction_conflict.mmd"),
        ),
        (
            "flowchart_parallel_label_stack",
            include_str!("fixtures/flowchart_parallel_label_stack.mmd"),
        ),
        ("class_tiny", include_str!("fixtures/class_tiny.mmd")),
        ("class_medium", include_str!("fixtures/class_medium.mmd")),
        (
            "class_namespace_dense",
            include_str!("fixtures/stress_class_dense_namespaces_generics_001.mmd"),
        ),
        ("state_tiny", include_str!("fixtures/state_tiny.mmd")),
        ("state_medium", include_str!("fixtures/state_medium.mmd")),
        ("sequence_tiny", include_str!("fixtures/sequence_tiny.mmd")),
        (
            "sequence_medium",
            include_str!("fixtures/sequence_medium.mmd"),
        ),
        ("er_medium", include_str!("fixtures/er_medium.mmd")),
        ("info_medium", include_str!("fixtures/info_medium.mmd")),
        ("pie_medium", include_str!("fixtures/pie_medium.mmd")),
        (
            "mindmap_medium",
            include_str!("fixtures/mindmap_medium.mmd"),
        ),
        (
            "journey_medium",
            include_str!("fixtures/journey_medium.mmd"),
        ),
        (
            "timeline_medium",
            include_str!("fixtures/timeline_medium.mmd"),
        ),
        ("gantt_medium", include_str!("fixtures/gantt_medium.mmd")),
        (
            "requirement_medium",
            include_str!("fixtures/requirement_medium.mmd"),
        ),
        (
            "gitgraph_medium",
            include_str!("fixtures/gitgraph_medium.mmd"),
        ),
        ("c4_medium", include_str!("fixtures/c4_medium.mmd")),
        ("sankey_medium", include_str!("fixtures/sankey_medium.mmd")),
        (
            "quadrant_medium",
            include_str!("fixtures/quadrant_medium.mmd"),
        ),
        ("zenuml_medium", include_str!("fixtures/zenuml_medium.mmd")),
        ("block_medium", include_str!("fixtures/block_medium.mmd")),
        ("packet_medium", include_str!("fixtures/packet_medium.mmd")),
        ("kanban_medium", include_str!("fixtures/kanban_medium.mmd")),
        (
            "architecture_medium",
            include_str!("fixtures/architecture_medium.mmd"),
        ),
        ("radar_medium", include_str!("fixtures/radar_medium.mmd")),
        (
            "treemap_medium",
            include_str!("fixtures/treemap_medium.mmd"),
        ),
        (
            "xychart_medium",
            include_str!("fixtures/xychart_medium.mmd"),
        ),
        ("venn_medium", include_str!("fixtures/venn_medium.mmd")),
        (
            "swimlane_medium",
            include_str!("fixtures/swimlane_medium.mmd"),
        ),
        (
            "eventmodeling_medium",
            include_str!("fixtures/eventmodeling_medium.mmd"),
        ),
        (
            "treeview_medium",
            include_str!("fixtures/treeview_medium.mmd"),
        ),
        (
            "ishikawa_medium",
            include_str!("fixtures/ishikawa_medium.mmd"),
        ),
        (
            "railroad_medium",
            include_str!("fixtures/railroad_medium.mmd"),
        ),
        (
            "railroad_abnf_medium",
            include_str!("fixtures/railroad_abnf_medium.mmd"),
        ),
        (
            "railroad_ebnf_medium",
            include_str!("fixtures/railroad_ebnf_medium.mmd"),
        ),
        (
            "railroad_peg_medium",
            include_str!("fixtures/railroad_peg_medium.mmd"),
        ),
        (
            "wardley_medium",
            include_str!("fixtures/wardley_medium.mmd"),
        ),
        (
            "cynefin_medium",
            include_str!("fixtures/cynefin_medium.mmd"),
        ),
        ("error_basic", include_str!("fixtures/error_basic.mmd")),
    ]
}

fn frontmatter_fixtures() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "frontmatter_basic",
            include_str!("fixtures/frontmatter_basic.mmd"),
        ),
        (
            "frontmatter_indented",
            include_str!("fixtures/frontmatter_indented.mmd"),
        ),
        (
            "frontmatter_deep_config",
            include_str!("fixtures/frontmatter_deep_config.mmd"),
        ),
    ]
}

fn bench_parse(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::strict();

    let mut group = c.benchmark_group("parse");
    for (name, input) in fixtures() {
        if !benchmark_selected("parse", name) {
            continue;
        }
        let output_identity = || {
            let parsed = engine
                .parse_diagram_for_render_model_sync(input, parse_opts)
                .unwrap_or_else(|error| panic!("parse/{name} preflight failed: {error}"))
                .unwrap_or_else(|| panic!("parse/{name} preflight returned no render model"));
            let projection = parsed
                .model()
                .compatibility_json(parsed.metadata())
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
    let engine = Engine::new();
    let parse_opts = ParseOptions::strict();

    let mut group = c.benchmark_group("compatibility_json_parse");
    for (name, input) in fixtures() {
        if !benchmark_selected("compatibility_json_parse", name) {
            continue;
        }
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
    for (name, input) in fixtures() {
        if !benchmark_selected("parse_cold_engine", name) {
            continue;
        }
        // Tracks request-style usage that constructs a fresh Engine per parse, so parse wins are
        // not accidentally attributed only to the hot shared-engine benchmark shape.
        let output_identity = || {
            let engine = Engine::new();
            let parsed = engine
                .parse_diagram_for_render_model_sync(input, parse_opts)
                .unwrap_or_else(|error| {
                    panic!("parse_cold_engine/{name} preflight failed: {error}")
                })
                .unwrap_or_else(|| {
                    panic!("parse_cold_engine/{name} preflight returned no render model")
                });
            let projection = parsed
                .model()
                .compatibility_json(parsed.metadata())
                .unwrap_or_else(|error| {
                    panic!("parse_cold_engine/{name} projection failed: {error}")
                });
            json_output_identity("typed_render_model", &projection)
        };
        let preflight = output_identity();
        emit_preflight("parse_cold_engine", name, &preflight);

        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let engine = Engine::new();
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

fn bench_frontmatter_preprocess(c: &mut Criterion) {
    let registry = DetectorRegistry::pinned_mermaid_baseline();

    let mut group = c.benchmark_group("frontmatter_preprocess");
    for (name, input) in frontmatter_fixtures() {
        if !benchmark_selected("frontmatter_preprocess", name) {
            continue;
        }
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
            json_output_identity("preprocessed_diagram", &projection)
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
    let engine = Engine::new();
    let parse_opts = ParseOptions::strict();
    let layout: LayoutOptions = headless_layout_options();
    let environment = RenderEnvironment::deterministic();

    let mut group = c.benchmark_group("layout");
    for (name, input) in fixtures() {
        if !benchmark_selected("layout", name) {
            continue;
        }
        let parsed = engine
            .parse_diagram_for_render_model_sync(input, parse_opts)
            .unwrap_or_else(|error| panic!("layout/{name} parse preflight failed: {error}"))
            .unwrap_or_else(|| panic!("layout/{name} preflight returned no render model"));
        let output_identity = || {
            let artifact = merman_render::family::prepare(
                parsed.clone(),
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
                || {
                    (
                        (*data).clone(),
                        environment.begin_session().expect("render session"),
                    )
                },
                |(parsed, session)| {
                    let artifact =
                        merman_render::family::prepare(parsed, &layout, session).expect("layout");
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
    let engine = Engine::new();
    let parse_opts = ParseOptions::strict();
    let layout: LayoutOptions = headless_layout_options();
    let environment = RenderEnvironment::deterministic();

    let mut group = c.benchmark_group("render");
    for (name, input) in fixtures() {
        if !benchmark_selected("render", name) {
            continue;
        }
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
                parsed.clone(),
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
                        (*data).clone(),
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
    let engine = Engine::new();
    let parse_opts = ParseOptions::strict();
    let layout = headless_layout_options();

    let mut group = c.benchmark_group("end_to_end");
    for (name, input) in fixtures() {
        if !benchmark_selected("end_to_end", name) {
            continue;
        }
        let svg_opts = SvgRenderOptions {
            diagram_id: Some(merman::svg::sanitize_svg_id(name)),
            ..SvgRenderOptions::default()
        };

        let output_identity = || {
            let svg = merman::svg::render_svg_sync(&engine, input, parse_opts, &layout, &svg_opts)
                .unwrap_or_else(|error| panic!("end_to_end/{name} preflight failed: {error}"))
                .unwrap_or_else(|| panic!("end_to_end/{name} preflight returned no SVG"));
            svg_output_identity(&svg)
        };
        let preflight = output_identity();
        emit_preflight("end_to_end", name, &preflight);

        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let svg = merman::svg::render_svg_sync(
                    &engine,
                    black_box(data),
                    parse_opts,
                    &layout,
                    &svg_opts,
                )
                .expect("end-to-end benchmark render")
                .expect("end-to-end benchmark SVG");
                black_box(svg.len());
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
    bench_frontmatter_preprocess,
    bench_layout,
    bench_render,
    bench_end_to_end
);
criterion_main!(benches);

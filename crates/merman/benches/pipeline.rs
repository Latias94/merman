use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use merman::svg::{
    LayoutOptions, RenderEnvironment, SvgDebugOptions, SvgPipeline, SvgRenderOptions,
    headless_layout_options,
};
use merman_core::{DetectorRegistry, Engine, ParseOptions};
use std::hint::black_box;

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
        // Skip fixtures that are not yet supported by `merman` to keep the bench runnable while
        // we expand coverage. Unsupported fixtures should be tracked separately as parity work.
        if engine
            .parse_diagram_for_render_model_sync(input, parse_opts)
            .is_err()
        {
            eprintln!("[bench][skip][parse] {name}: parse error");
            continue;
        }
        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let parsed =
                    engine.parse_diagram_for_render_model_sync(black_box(data), parse_opts);
                let parsed = match parsed {
                    Ok(v) => v,
                    Err(_) => return,
                };
                black_box(parsed.is_some());
            })
        });
    }
    group.finish();
}

fn bench_compatibility_json_parse(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::strict();

    let mut group = c.benchmark_group("compatibility_json_parse");
    for (name, input) in fixtures() {
        let diagram_type = match engine.parse_metadata_sync(input) {
            Ok(v) => v.diagram_type,
            Err(_) => {
                eprintln!("[bench][skip][compatibility_json_parse] {name}: metadata error");
                continue;
            }
        };

        // Pre-check that the known-type parse succeeds.
        if engine
            .parse_diagram_with_type_sync(&diagram_type, input, parse_opts)
            .is_err()
        {
            eprintln!(
                "[bench][skip][compatibility_json_parse] {name}: parse_with_type({diagram_type}) error"
            );
            continue;
        }

        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let parsed =
                    engine.parse_diagram_with_type_sync(&diagram_type, black_box(data), parse_opts);
                let parsed = match parsed {
                    Ok(v) => v,
                    Err(_) => return,
                };
                black_box(parsed.is_some());
            })
        });
    }
    group.finish();
}

fn bench_parse_cold_engine(c: &mut Criterion) {
    let parse_opts = ParseOptions::strict();

    let mut group = c.benchmark_group("parse_cold_engine");
    for (name, input) in fixtures() {
        // Tracks request-style usage that constructs a fresh Engine per parse, so parse wins are
        // not accidentally attributed only to the hot shared-engine benchmark shape.
        let engine = Engine::new();
        if engine
            .parse_diagram_for_render_model_sync(input, parse_opts)
            .is_err()
        {
            eprintln!("[bench][skip][parse_cold_engine] {name}: parse error");
            continue;
        }

        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let engine = Engine::new();
                let parsed =
                    engine.parse_diagram_for_render_model_sync(black_box(data), parse_opts);
                let parsed = match parsed {
                    Ok(v) => v,
                    Err(_) => return,
                };
                black_box(parsed.is_some());
            })
        });
    }
    group.finish();
}

fn bench_frontmatter_preprocess(c: &mut Criterion) {
    let registry = DetectorRegistry::pinned_mermaid_baseline();

    let mut group = c.benchmark_group("frontmatter_preprocess");
    for (name, input) in frontmatter_fixtures() {
        let pre = match merman_core::preprocess_diagram_with_known_type(
            input,
            &registry,
            Some("flowchart-v2"),
        ) {
            Ok(v) => v,
            Err(_) => {
                eprintln!("[bench][skip][frontmatter_preprocess] {name}: preprocess error");
                continue;
            }
        };

        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let pre = match merman_core::preprocess_diagram_with_known_type(
                    black_box(data),
                    &registry,
                    Some("flowchart-v2"),
                ) {
                    Ok(v) => v,
                    Err(_) => return,
                };
                black_box(pre.code().len());
                black_box(pre.title.as_deref().map_or(0, str::len));
                black_box(pre.config.as_value().as_object().map_or(0, |map| map.len()));
            })
        });

        black_box(pre.code().len());
        black_box(pre.title.as_deref().map_or(0, str::len));
        black_box(pre.config.as_value().as_object().map_or(0, |map| map.len()));
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
        let parsed = match engine.parse_diagram_for_render_model_sync(input, parse_opts) {
            Ok(Some(v)) => v,
            Ok(None) => continue,
            Err(_) => {
                eprintln!("[bench][skip][layout] {name}: parse error");
                continue;
            }
        };

        // Pre-check that layout works.
        if merman_render::family::prepare(
            parsed.clone(),
            &layout,
            environment.begin_session().expect("render session"),
        )
        .is_err()
        {
            eprintln!("[bench][skip][layout] {name}: layout error");
            continue;
        }

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
        let parsed = match engine.parse_diagram_for_render_model_sync(input, parse_opts) {
            Ok(Some(v)) => v,
            Ok(None) => continue,
            Err(_) => {
                eprintln!("[bench][skip][render] {name}: parse error");
                continue;
            }
        };
        let svg_opts = SvgRenderOptions {
            diagram_id: Some(merman::svg::sanitize_svg_id(name)),
            ..SvgRenderOptions::default()
        };

        // Pre-check that SVG rendering works.
        let preflight = merman_render::family::prepare(
            parsed.clone(),
            &layout,
            environment.begin_session().expect("render session"),
        );
        let Ok(preflight) = preflight else {
            eprintln!("[bench][skip][render] {name}: layout error");
            continue;
        };
        if preflight
            .render_svg(&svg_opts, &SvgDebugOptions::default())
            .is_err()
        {
            eprintln!("[bench][skip][render] {name}: svg render error");
            continue;
        }

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
    }
    group.finish();
}

fn bench_end_to_end(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::strict();
    let layout = headless_layout_options();

    let mut group = c.benchmark_group("end_to_end");
    for (name, input) in fixtures() {
        let svg_opts = SvgRenderOptions {
            diagram_id: Some(merman::svg::sanitize_svg_id(name)),
            ..SvgRenderOptions::default()
        };

        // Pre-check end-to-end viability once to keep the bench stable.
        if merman::svg::render_svg_sync(&engine, input, parse_opts, &layout, &svg_opts).is_err() {
            eprintln!("[bench][skip][end_to_end] {name}: svg render error");
            continue;
        }

        group.bench_with_input(BenchmarkId::from_parameter(name), input, |b, data| {
            b.iter(|| {
                let svg = match merman::svg::render_svg_sync(
                    &engine,
                    black_box(data),
                    parse_opts,
                    &layout,
                    &svg_opts,
                ) {
                    Ok(Some(v)) => v,
                    Ok(None) => return,
                    Err(_) => return,
                };
                black_box(svg.len());
            });
        });
    }
    group.finish();
}

fn bench_resvg_end_to_end(c: &mut Criterion) {
    let renderer = merman::svg::HeadlessRenderer::new();
    let pipeline = SvgPipeline::resvg_safe();
    let input = include_str!("fixtures/kanban_medium.mmd");

    renderer
        .render_resvg_compatible_svg_with_pipeline_sync(input, &pipeline)
        .expect("ResvgSafe preflight")
        .expect("detected benchmark diagram");

    let mut group = c.benchmark_group("resvg_end_to_end");
    group.bench_function("kanban_medium", |b| {
        b.iter(|| {
            let svg = renderer
                .render_resvg_compatible_svg_with_pipeline_sync(black_box(input), &pipeline)
                .expect("ResvgSafe render")
                .expect("detected benchmark diagram");
            black_box(svg.as_str().len());
        })
    });
    group.finish();
}

#[cfg(feature = "png")]
fn bench_png_end_to_end(c: &mut Criterion) {
    let renderer = merman::svg::HeadlessRenderer::new();
    let options = merman::svg::export::RasterOptions::default();
    let input = include_str!("fixtures/kanban_medium.mmd");

    renderer
        .render_png_sync(input, &options)
        .expect("PNG preflight")
        .expect("detected benchmark diagram");

    let mut group = c.benchmark_group("png_end_to_end");
    group.bench_function("kanban_medium", |b| {
        b.iter(|| {
            let png = renderer
                .render_png_sync(black_box(input), &options)
                .expect("PNG render")
                .expect("detected benchmark diagram");
            black_box(png.len());
        })
    });
    group.finish();
}

#[cfg(not(feature = "png"))]
fn bench_png_end_to_end(_: &mut Criterion) {}

criterion_group!(
    benches,
    bench_parse,
    bench_compatibility_json_parse,
    bench_parse_cold_engine,
    bench_frontmatter_preprocess,
    bench_layout,
    bench_render,
    bench_end_to_end,
    bench_resvg_end_to_end,
    bench_png_end_to_end
);
criterion_main!(benches);

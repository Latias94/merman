use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use merman::svg::{
    LayoutOptions, RenderEnvironment, SvgDebugOptions, SvgRenderOptions, headless_layout_options,
};
use merman_core::{Engine, ParseOptions};
use std::hint::black_box;

const FLOWCHART_MEDIUM: &str = include_str!("fixtures/flowchart_medium.mmd");
const FLOWCHART_PORTS_HEAVY: &str = include_str!("fixtures/flowchart_ports_heavy.mmd");

fn bench_flowchart_stress(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::strict();
    let layout: LayoutOptions = headless_layout_options();
    let environment = RenderEnvironment::deterministic();

    let mut group = c.benchmark_group("render_stress");
    group.sample_size(50);

    // Render-only stress: amplify micro-bench timings by batching many renders per iteration so
    // small A/B changes are less likely to be lost in noise.
    for (name, input, repeats) in [
        ("flowchart_medium_x50", FLOWCHART_MEDIUM, 50usize),
        ("flowchart_ports_heavy_x20", FLOWCHART_PORTS_HEAVY, 20usize),
    ] {
        let parsed = engine
            .parse_diagram_for_render_model_sync(input, parse_opts)
            .expect("parse")
            .expect("supported diagram");
        let svg_opts = SvgRenderOptions {
            diagram_id: Some(merman::svg::sanitize_svg_id(name)),
            ..SvgRenderOptions::default()
        };
        // Pre-check that rendering works once, outside measurement.
        merman_render::family::prepare(
            parsed.clone(),
            &layout,
            environment.begin_session().expect("render session"),
        )
        .expect("prepare")
        .render_svg(&svg_opts, &SvgDebugOptions::default())
        .expect("render");

        let render_layout = layout.clone();
        let render_environment = environment.clone();
        group.bench_function(name, move |b| {
            b.iter_batched(
                || {
                    (0..repeats)
                        .map(|_| {
                            merman_render::family::prepare(
                                parsed.clone(),
                                &render_layout,
                                render_environment.begin_session().expect("render session"),
                            )
                            .expect("prepare")
                        })
                        .collect::<Vec<_>>()
                },
                |artifacts| {
                    let mut acc: usize = 0;
                    for artifact in artifacts {
                        let svg = artifact
                            .render_svg(&svg_opts, &SvgDebugOptions::default())
                            .expect("render");
                        acc ^= svg.svg().len();
                    }
                    black_box(acc);
                },
                BatchSize::LargeInput,
            );
        });
    }

    group.finish();
}

criterion_group!(benches, bench_flowchart_stress);
criterion_main!(benches);

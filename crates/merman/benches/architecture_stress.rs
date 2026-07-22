use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use merman::render::{
    LayoutOptions, RenderEnvironment, SvgDebugOptions, SvgRenderOptions, headless_layout_options,
};
use merman_core::{Engine, ParseOptions};
use std::hint::black_box;

const ARCH_MANY_SERVICES_ONE_GROUP: &str =
    include_str!("fixtures/stress_architecture_batch3_many_services_one_group_059.mmd");

fn bench_architecture_stress(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::strict();
    let layout: LayoutOptions = headless_layout_options();
    let environment = RenderEnvironment::deterministic();

    let parsed = engine
        .parse_diagram_for_render_model_sync(ARCH_MANY_SERVICES_ONE_GROUP, parse_opts)
        .expect("parse")
        .expect("supported diagram");
    let svg_opts = SvgRenderOptions {
        diagram_id: Some(merman::render::sanitize_svg_id(
            "stress_architecture_batch3_many_services_one_group_059",
        )),
        ..SvgRenderOptions::default()
    };
    merman_render::family::prepare(
        parsed.clone(),
        &layout,
        environment.begin_session().expect("render session"),
    )
    .expect("prepare")
    .render_svg(&svg_opts, &SvgDebugOptions::default())
    .expect("render");

    let mut group = c.benchmark_group("render_stress");
    group.sample_size(50);

    // Architecture render is very fast (µs-scale) on medium fixtures, so we batch to get stable
    // signals from per-render fixed-cost changes.
    group.bench_function("architecture_many_services_one_group_x200", move |b| {
        b.iter_batched(
            || {
                (0..200)
                    .map(|_| {
                        merman_render::family::prepare(
                            parsed.clone(),
                            &layout,
                            environment.begin_session().expect("render session"),
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

    group.finish();
}

criterion_group!(benches, bench_architecture_stress);
criterion_main!(benches);

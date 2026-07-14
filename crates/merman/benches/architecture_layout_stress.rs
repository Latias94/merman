use criterion::{Criterion, criterion_group, criterion_main};
use merman::render::{
    LayoutOptions, RenderEnvironment, TextMeasurementPhase, headless_layout_options,
};
use merman_core::{Engine, ParseOptions, RenderSemanticModel};
use merman_render::architecture::layout_architecture_diagram_typed;
use std::hint::black_box;

const ARCH_REASONABLE_HEIGHT: &str =
    include_str!("fixtures/upstream_architecture_layout_reasonable_height.mmd");

fn bench_architecture_layout_stress(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::strict();
    let layout: LayoutOptions = headless_layout_options();
    let session = RenderEnvironment::parity()
        .begin_session()
        .expect("render session");

    let parsed = engine
        .parse_diagram_for_render_model_sync(ARCH_REASONABLE_HEIGHT, parse_opts)
        .expect("parse")
        .expect("supported diagram");
    let RenderSemanticModel::Architecture(model) = &parsed.model else {
        panic!("expected architecture render model");
    };
    let ambient_seed = session.seed().seed().get();
    let measurer = session.text_measurer(TextMeasurementPhase::Layout);

    let mut group = c.benchmark_group("layout_stress");
    group.sample_size(50);

    // Architecture layout is fast (µs–ms scale depending on fixture), so we batch to get stable
    // signals from fixed-cost + allocation changes inside the FCoSE/manatee pipeline.
    group.bench_function("architecture_reasonable_height_layout_x50", move |b| {
        b.iter(|| {
            let mut acc: usize = 0;
            for _ in 0..50usize {
                let layouted = layout_architecture_diagram_typed(
                    black_box(model),
                    parsed.meta.effective_config.as_value(),
                    &measurer,
                    layout.use_manatee_layout,
                    ambient_seed,
                )
                .expect("layout");
                acc ^= layouted.nodes.len();
                acc ^= layouted.edges.len();
            }
            black_box(acc);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_architecture_layout_stress);
criterion_main!(benches);

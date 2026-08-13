use criterion::{Criterion, criterion_group, criterion_main};
use merman_core::{Engine, ParseOptions};
use merman_render::{LayoutOptions, environment::RenderEnvironment, family};
use std::hint::black_box;

const ARCH_REASONABLE_HEIGHT: &str =
    include_str!("fixtures/upstream_architecture_layout_reasonable_height.mmd");

fn bench_architecture_layout_stress(c: &mut Criterion) {
    let engine = Engine::new();
    let parse_opts = ParseOptions::strict();
    let environment = RenderEnvironment::deterministic();

    let parsed = engine
        .parse_diagram_for_render_model_sync(ARCH_REASONABLE_HEIGHT, parse_opts)
        .expect("parse")
        .expect("supported diagram");
    let layout_options = LayoutOptions::headless_svg_defaults();

    let mut group = c.benchmark_group("layout_stress");
    group.sample_size(50);

    // Benchmark the canonical operation-owned preparation seam. The parsed model clone is part of
    // the public API cost because raw family layout entry points are intentionally crate-private.
    group.bench_function("architecture_reasonable_height_prepare_x50", move |b| {
        b.iter(|| {
            for _ in 0..50usize {
                let session = environment.begin_session().expect("render session");
                let artifact = family::prepare(black_box(parsed.clone()), &layout_options, session)
                    .expect("prepare");
                black_box(artifact.family_kind());
            }
        });
    });

    group.finish();
}

criterion_group!(benches, bench_architecture_layout_stress);
criterion_main!(benches);

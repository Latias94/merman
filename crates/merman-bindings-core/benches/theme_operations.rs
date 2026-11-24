use criterion::{Criterion, criterion_group, criterion_main};
use merman::theme::{Appearance, BuiltinPreset, ThemeCompiler};
use merman_bindings_core::BindingEngine;
use std::hint::black_box;

fn bench_theme_operations(criterion: &mut Criterion) {
    let engine = BindingEngine::new(b"").unwrap();
    let compiler = ThemeCompiler::new();
    for preset in [BuiltinPreset::Studio, BuiltinPreset::Cyberpunk] {
        let id = preset.as_str().as_bytes();
        let request = format!(r#"{{"document":{},"appearance":"dark"}}"#, preset.json());
        let request = request.as_bytes();
        let resolved = engine.resolve_theme(request).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&resolved).unwrap()["ok"],
            true
        );

        criterion.bench_function(&format!("theme_compile_preset/{preset}"), |bench| {
            bench.iter(|| {
                black_box(
                    compiler
                        .compile_preset(black_box(preset), Some(Appearance::Dark))
                        .unwrap(),
                )
            });
        });
        criterion.bench_function(&format!("theme_export_preset/{preset}"), |bench| {
            bench.iter(|| black_box(engine.export_theme_preset(black_box(id)).unwrap()));
        });
        criterion.bench_function(&format!("theme_resolve/{preset}"), |bench| {
            bench.iter(|| black_box(engine.resolve_theme(black_box(request)).unwrap()));
        });
    }
    criterion.bench_function("theme_catalog/reused_engine", |bench| {
        bench.iter(|| black_box(engine.metadata_json(black_box("theme-catalog")).unwrap()));
    });
}

criterion_group!(benches, bench_theme_operations);
criterion_main!(benches);

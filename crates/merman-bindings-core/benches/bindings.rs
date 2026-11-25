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

// The request overlay fixture uses the Info parser. SVG-only builds run the theme cases.
#[cfg(all(feature = "analysis", feature = "ascii", feature = "diagram-info"))]
mod request_overlay {
    use criterion::{BenchmarkId, Criterion};
    use merman_bindings_core::{BindingEngine, BindingOperationRequest, BindingOperationResult};
    use serde_json::Value;
    use std::hint::black_box;

    const SOURCE: &[u8] = include_bytes!("fixtures/info_fixed_cost.mmd");
    const BASE_OPTIONS: &[u8] = br#"{
      "version": 4,
      "runtime_policy": "deterministic",
      "resources": {"profile": "trusted-native"}
    }"#;
    const EMPTY_OPTIONS: &[u8] = b"";
    const VERSION_ONLY_OPTIONS: &[u8] = br#"{"version":4}"#;
    const RESOURCE_OVERRIDE_OPTIONS: &[u8] =
        br#"{"version":4,"resources":{"limits":{"max_source_bytes":4096}}}"#;

    fn execute(engine: &BindingEngine, options_json: &'static [u8]) -> BindingOperationResult {
        engine
            .execute(
                BindingOperationRequest::new("semantic-json", SOURCE)
                    .with_options_json(options_json),
            )
            .expect("semantic operation")
    }

    fn assert_result_contract(result: &BindingOperationResult) {
        assert_eq!(
            result.data(),
            br#"{"type":"info","showInfo":true}"#,
            "fixed semantic JSON bytes"
        );
        let metadata: Value =
            serde_json::from_slice(result.metadata_json()).expect("metadata JSON");
        assert_eq!(
            metadata,
            serde_json::json!({
                "version": 1,
                "operation_id": "semantic-json",
                "media_type": "application/json",
                "runtime_policy": "deterministic",
                "byte_length": result.data().len(),
            })
        );
    }

    fn bench_request_options(
        criterion: &mut Criterion,
        engine: &BindingEngine,
        group_name: &str,
        request_options: &'static [u8],
    ) {
        let request = BindingOperationRequest::new("semantic-json", SOURCE)
            .with_options_json(request_options);

        let mut group = criterion.benchmark_group(group_name);
        group.bench_with_input(
            BenchmarkId::from_parameter("info_fixed_cost"),
            &request,
            |b, request| {
                b.iter(|| {
                    let result = engine
                        .execute(black_box(request.clone()))
                        .expect("semantic operation");
                    black_box(result);
                });
            },
        );
        group.finish();
    }

    pub(super) fn bench_request_overlays(criterion: &mut Criterion) {
        let engine = BindingEngine::from_options(BASE_OPTIONS).expect("binding engine");
        let expected = execute(&engine, EMPTY_OPTIONS);
        assert_result_contract(&expected);
        assert_eq!(execute(&engine, VERSION_ONLY_OPTIONS), expected);
        assert_eq!(execute(&engine, RESOURCE_OVERRIDE_OPTIONS), expected);
        drop(expected);

        bench_request_options(
            criterion,
            &engine,
            "binding_request_empty_analysis_ascii_svg",
            EMPTY_OPTIONS,
        );
        bench_request_options(
            criterion,
            &engine,
            "binding_request_version_only_analysis_ascii_svg",
            VERSION_ONLY_OPTIONS,
        );
        bench_request_options(
            criterion,
            &engine,
            "binding_request_resource_override_analysis_ascii_svg",
            RESOURCE_OVERRIDE_OPTIONS,
        );
    }
}

fn bench_bindings(criterion: &mut Criterion) {
    #[cfg(all(feature = "analysis", feature = "ascii", feature = "diagram-info"))]
    request_overlay::bench_request_overlays(criterion);
    bench_theme_operations(criterion);
}

criterion_group!(benches, bench_bindings);
criterion_main!(benches);

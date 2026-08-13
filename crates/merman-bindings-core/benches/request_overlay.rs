use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use merman_bindings_core::{BindingEngine, BindingOperationRequest, BindingOperationResult};
use serde_json::Value;
use std::hint::black_box;

const SOURCE: &[u8] = include_bytes!("fixtures/info_fixed_cost.mmd");
const BASE_OPTIONS: &[u8] = br#"{
  "version": 2,
  "runtime_policy": "deterministic",
  "resources": {"profile": "trusted-native"}
}"#;
const EMPTY_OPTIONS: &[u8] = b"";
const VERSION_ONLY_OPTIONS: &[u8] = br#"{"version":2}"#;
const RESOURCE_OVERRIDE_OPTIONS: &[u8] =
    br#"{"version":2,"resources":{"limits":{"max_source_bytes":4096}}}"#;

fn execute(engine: &BindingEngine, options_json: &'static [u8]) -> BindingOperationResult {
    engine
        .execute(
            BindingOperationRequest::new("semantic-json", SOURCE).with_options_json(options_json),
        )
        .expect("semantic operation")
}

fn assert_result_contract(result: &BindingOperationResult) {
    assert_eq!(
        result.data(),
        br#"{"type":"info","showInfo":true}"#,
        "fixed semantic JSON bytes"
    );
    let metadata: Value = serde_json::from_slice(result.metadata_json()).expect("metadata JSON");
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
    let request =
        BindingOperationRequest::new("semantic-json", SOURCE).with_options_json(request_options);

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

fn bench_request_overlays(criterion: &mut Criterion) {
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

criterion_group!(benches, bench_request_overlays);
criterion_main!(benches);

#![cfg(all(feature = "svg", feature = "diagram-flowchart"))]

use merman_bindings_core::{BindingEngine, BindingEngineServices, BindingStatus, OperationControl};

#[test]
fn retained_source_outputs_share_frozen_semantics_and_independent_controls() {
    let engine =
        BindingEngine::new(br#"{"site_config":{"flowchart":{"htmlLabels":false}}}"#).unwrap();
    let source = "flowchart LR\nA[Hello] --> B[World]";
    let prepared = engine
        .prepare_source(source.as_bytes(), OperationControl::new())
        .unwrap();
    let services = BindingEngineServices::new();
    let first = prepared
        .render_svg(b"", OperationControl::new(), &services)
        .unwrap();
    assert!(String::from_utf8_lossy(&first).contains("Hello"));
    let cancelled = OperationControl::new();
    cancelled.cancel();
    assert_eq!(
        prepared
            .render_svg(b"", cancelled, &services)
            .unwrap_err()
            .status(),
        BindingStatus::Cancelled
    );
    assert_eq!(
        first,
        prepared
            .render_svg(b"", OperationControl::new(), &services)
            .unwrap()
    );
    let layout: serde_json::Value = serde_json::from_slice(
        &prepared
            .layout_json(b"", OperationControl::new(), &services)
            .unwrap(),
    )
    .unwrap();
    assert!(layout.is_object());
    let geometry: serde_json::Value = serde_json::from_slice(
        &prepared
            .edge_geometry_json(b"", OperationControl::new(), &services)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(geometry["schema_version"], 1);
}

#[test]
fn retained_output_rejects_frozen_settings_and_readmits_resources() {
    let engine = BindingEngine::new(b"").unwrap();
    let prepared = engine
        .prepare_source(b"flowchart LR\nA --> B", OperationControl::new())
        .unwrap();
    let services = BindingEngineServices::new();
    for options in [
        br#"{"parse":{"suppress_errors":true}}"#.as_slice(),
        br#"{"site_config":{}}"#,
        br#"{"analysis":{"fixed_today":"2030-01-01"}}"#,
        br#"{"fixed_local_offset_minutes":60}"#,
        br#"{"theme":null}"#,
    ] {
        assert_eq!(
            prepared
                .render_svg(options, OperationControl::new(), &services)
                .unwrap_err()
                .status(),
            BindingStatus::OptionsJsonError
        );
    }
    assert_eq!(
        prepared
            .render_svg(
                br#"{"resources":{"limits":{"max_source_bytes":1}}}"#,
                OperationControl::new(),
                &services
            )
            .unwrap_err()
            .status(),
        BindingStatus::ResourceLimitExceeded
    );
    assert!(
        prepared
            .render_svg(b"", OperationControl::new(), &services)
            .is_ok()
    );
    assert_eq!(
        engine
            .prepare_source(b"", OperationControl::new())
            .err()
            .unwrap()
            .status(),
        BindingStatus::NoDiagram
    );
}

#[test]
fn retained_result_uses_the_same_metadata_and_theme_owner_as_one_shot() {
    let engine = BindingEngine::new(
        br#"{"theme":{"preset":"editor-light"},"site_config":{"flowchart":{"htmlLabels":false}}}"#,
    )
    .unwrap();
    let source = b"flowchart LR\nA[Same] --> B[Output]";
    let once = engine
        .execute(merman_bindings_core::BindingOperationRequest::new(
            "svg", source,
        ))
        .unwrap();
    let prepared = engine
        .prepare_source(source, OperationControl::new())
        .unwrap();
    let repeated = prepared
        .render_svg_result(b"", OperationControl::new(), &BindingEngineServices::new())
        .unwrap();
    assert_eq!(once, repeated);
}

#[test]
fn retained_source_outlives_the_lazy_engine_and_keeps_runtime_and_theme_defaults() {
    let options = br#"{
        "theme":{"preset":"editor-light"},
        "site_config":{"flowchart":{"htmlLabels":false}}
    }"#;
    let runtime_policy = merman::runtime::RuntimePolicy::deterministic()
        .with_fixed_unix_millis(1_771_113_600_000)
        .with_fixed_seed(42);
    let engine = BindingEngine::with_runtime_policy(options, runtime_policy).unwrap();
    let source = b"flowchart LR\nA[Retained] --> B[Output]";
    let prepared = engine
        .prepare_source(source, OperationControl::new())
        .unwrap();
    let expected = engine
        .execute(merman_bindings_core::BindingOperationRequest::new(
            "svg", source,
        ))
        .unwrap();
    drop(engine);
    let services = BindingEngineServices::new();
    assert_eq!(
        prepared
            .render_svg_result(b"", OperationControl::new(), &services)
            .unwrap(),
        expected,
    );
    let cancelled = OperationControl::new();
    cancelled.cancel();
    assert_eq!(
        prepared
            .render_svg(b"", cancelled, &services)
            .unwrap_err()
            .status(),
        BindingStatus::Cancelled,
    );
    assert_eq!(
        prepared
            .render_svg_result(b"", OperationControl::new(), &services)
            .unwrap(),
        expected,
    );
}

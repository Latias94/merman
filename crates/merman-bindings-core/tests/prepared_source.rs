#![cfg(all(feature = "svg", feature = "diagram-flowchart"))]

use merman_bindings_core::{
    BindingEngine, BindingEngineServices, BindingOperationRequest, BindingStatus, OperationControl,
};

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
        br#"{"site_config":{},"theme":"studio"}"#,
        br#"{"analysis":{"fixed_today":"2030-01-01"}}"#,
        br#"{"fixed_unix_millis":375}"#,
        br#"{"fixed_local_offset_minutes":60}"#,
        br#"{"runtime_policy":"deterministic"}"#,
    ] {
        assert_eq!(
            prepared
                .render_svg(options, OperationControl::new(), &services)
                .unwrap_err()
                .status(),
            BindingStatus::OptionsJsonError
        );
    }
    let profile_error = prepared
        .render_svg(
            br#"{"theme":"studio","resources":{"profile":"trusted-native"}}"#,
            OperationControl::new(),
            &services,
        )
        .unwrap_err();
    assert_eq!(profile_error.status(), BindingStatus::OptionsJsonError);
    assert!(
        profile_error
            .message()
            .contains("transport ceiling profile")
    );
    assert_eq!(
        prepared
            .render_svg(
                br#"{"theme":"studio","resources":{"limits":{"max_source_bytes":1}}}"#,
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
fn retained_result_inherits_replaces_and_clears_the_theme_per_output() {
    let engine = BindingEngine::new(
        br#"{
            "theme":{"preset":"studio","appearance":"light"},
            "site_config":{"theme":"forest","flowchart":{"htmlLabels":false}},
            "svg":{"diagram_id":"prepared-theme"}
        }"#,
    )
    .unwrap();
    let source = b"flowchart LR\nA[Same] --> B[Output]";
    let prepared = engine
        .prepare_source(source, OperationControl::new())
        .unwrap();
    let services = BindingEngineServices::new();
    let redraw = |options: &[u8]| {
        prepared
            .render_svg_result(options, OperationControl::new(), &services)
            .unwrap()
    };
    let render = |options: &[u8]| {
        engine
            .execute(BindingOperationRequest::new("svg", source).with_options_json(options))
            .unwrap()
    };

    let light = redraw(b"");
    assert_eq!(light, render(b""));
    assert_eq!(
        light.metadata().theme_outcome().unwrap()["appearance"],
        "light"
    );

    let dark_options = br#"{"theme":{"preset":"studio","appearance":"dark"}}"#;
    let dark = redraw(dark_options);
    assert_eq!(dark, render(dark_options));
    assert_eq!(
        dark.metadata().theme_outcome().unwrap()["appearance"],
        "dark"
    );
    assert_ne!(dark.data(), light.data());

    let clear_options = br#"{"theme":"none"}"#;
    let cleared = redraw(clear_options);
    assert_eq!(cleared, render(clear_options));
    assert_eq!(
        cleared.metadata().theme_outcome().unwrap()["mode"],
        "mermaid_parity"
    );
    assert_eq!(redraw(br#"{"theme":null}"#), cleared);
    assert_eq!(redraw(b""), light);
}

#[test]
fn retained_source_outlives_the_lazy_engine_and_keeps_runtime_and_theme_defaults() {
    let options = br#"{
        "theme":{"preset":"studio"},
        "site_config":{"flowchart":{"htmlLabels":false}}
    }"#;
    let runtime_policy = merman::model::runtime::RuntimePolicy::deterministic()
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

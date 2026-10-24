use merman_bindings_core::{BindingStatus, BindingTransportKey, RuntimePolicyExposure, TargetKey};
use serde_json::Value;
#[cfg(feature = "native-fonts")]
use serde_json::json;

use crate::runtime::Runtime;
use crate::wire::{self, Config, Request};
use crate::{ABI_VERSION, RESULT_DESCRIPTOR_SIZE};

fn input(runtime: &mut Runtime, bytes: &[u8]) -> (u32, u32) {
    if bytes.is_empty() {
        return (0, 0);
    }
    let len = u32::try_from(bytes.len()).unwrap();
    let offset = runtime.alloc(len);
    assert_ne!(offset, 0);
    let allocation = runtime
        .inputs
        .entries
        .iter_mut()
        .find(|entry| entry.offset == offset)
        .unwrap();
    allocation.bytes.copy_from_slice(bytes);
    (offset, len)
}

fn fields(runtime: &Runtime, handle: u32) -> [u32; 10] {
    let result = runtime.result(handle).expect("owned result");
    let mut values = [0; 10];
    for (value, bytes) in values
        .iter_mut()
        .zip(result.bytes[..RESULT_DESCRIPTOR_SIZE].chunks_exact(4))
    {
        *value = u32::from_le_bytes(bytes.try_into().unwrap());
    }
    assert_eq!(values[0], ABI_VERSION);
    assert_eq!(values[1], 40);
    assert_eq!(runtime.result_info(handle), result.offset);
    values
}

fn segment(runtime: &Runtime, handle: u32, field: usize) -> &[u8] {
    let values = fields(runtime, handle);
    if values[field + 1] == 0 {
        assert_eq!(values[field], 0);
        return &[];
    }
    let result = runtime.result(handle).unwrap();
    let start = (values[field] - result.offset) as usize;
    &result.bytes[start..start + values[field + 1] as usize]
}

fn error_json(runtime: &Runtime, handle: u32) -> Value {
    assert_eq!(fields(runtime, handle)[2], wire::STATUS_ERROR);
    assert_eq!(fields(runtime, handle)[3], 0);
    assert!(segment(runtime, handle, 6).is_empty());
    let json: Value = serde_json::from_slice(segment(runtime, handle, 8)).unwrap();
    assert_eq!(json["version"], 1);
    assert_eq!(json["ok"], false);
    assert!(json["code"].is_i64());
    assert!(json["code_name"].is_string());
    json
}

#[test]
fn wasi_has_native_semantics_without_callbacks_icons_or_ambient_runtime() {
    let contract = crate::artifact_contract();
    assert_eq!(contract.target(), TargetKey::Native);
    assert_eq!(
        contract.runtime_policy_exposure(),
        RuntimePolicyExposure::DeterministicOnly
    );
    assert_eq!(BindingTransportKey::Wasi.id(), "wasi");
    let services: Vec<_> = contract
        .constructor_service_keys()
        .map(|key| key.id())
        .collect();
    if cfg!(feature = "native-fonts") {
        assert_eq!(services, ["native-font-context"]);
    } else {
        assert!(services.is_empty());
    }
    assert_eq!(contract.system_adapter_keys().count(), 0);
}

#[test]
#[should_panic(expected = "WASI transport requires deterministic-only runtime policy")]
fn wasi_cannot_materialize_a_native_runtime_policy() {
    let _ = merman_bindings_core::ArtifactContractSpec::new(
        TargetKey::Native,
        BindingTransportKey::Wasi,
    )
    .with_runtime_policy_exposure(RuntimePolicyExposure::BindingOptions)
    .materialize();
}

#[test]
fn checked_in_descriptor_matches_the_runtime_layout_and_quotas() {
    let descriptor: Value =
        serde_json::from_str(include_str!("../../../contracts/abi/merman-wasi-v1.json")).unwrap();
    assert_eq!(descriptor["abi_version"], ABI_VERSION);
    assert_eq!(descriptor["binding_transport"], "wasi");
    assert_eq!(descriptor["semantic_target"], "native");
    assert_eq!(
        descriptor["descriptor"]["size_bytes"],
        RESULT_DESCRIPTOR_SIZE
    );
    let names = [
        "version",
        "size",
        "status",
        "engine",
        "media_ptr",
        "media_len",
        "data_ptr",
        "data_len",
        "json_ptr",
        "json_len",
    ];
    for (index, name) in names.into_iter().enumerate() {
        assert_eq!(descriptor["descriptor"]["fields"][index]["name"], name);
        assert_eq!(
            descriptor["descriptor"]["fields"][index]["offset"],
            index * 4
        );
    }
    let limits = crate::runtime::Limits::default();
    for (key, value) in [
        ("live_inputs", limits.inputs),
        ("input_bytes_each", limits.input_size),
        ("input_bytes_total", limits.input_bytes),
        ("envelope_bytes", limits.json_bytes),
        ("live_results", limits.results),
        ("result_bytes_each_including_descriptor", limits.result_size),
        (
            "result_bytes_total_including_descriptors",
            limits.result_bytes,
        ),
        ("live_engines", limits.engines),
        (
            "engine_font_input_bytes_each_before_deduplication",
            limits.engine_font_bytes,
        ),
        (
            "engine_font_input_bytes_total_before_deduplication",
            limits.font_bytes,
        ),
    ] {
        assert_eq!(descriptor["limits"][key], value);
    }
    assert_eq!(descriptor["exports"].as_array().unwrap().len(), 9);
}

#[test]
fn catalog_is_owned_and_uses_the_independent_abi_epoch() {
    let mut runtime = Runtime::default();
    let result = runtime.runtime_catalog();
    assert_ne!(result, 0);
    assert_eq!(fields(&runtime, result)[2], wire::STATUS_OK);
    assert_eq!(segment(&runtime, result, 4), b"application/json");
    let catalog: Value = serde_json::from_slice(segment(&runtime, result, 6)).unwrap();
    assert_eq!(catalog["transport_api_version"], ABI_VERSION);
    assert_eq!(catalog["options_schema_versions"], serde_json::json!([2]));
    let capability_ids = catalog["capabilities"]["capability_ids"]
        .as_array()
        .unwrap();
    assert_eq!(
        capability_ids.iter().any(|id| id == "native-fonts"),
        cfg!(feature = "native-fonts")
    );
    assert!(
        catalog["capabilities"]["system_adapter_ids"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        runtime.free(runtime.result_info(result)),
        wire::INVALID_ALLOCATION
    );
    assert_eq!(runtime.result_free(result), 0);
    assert_eq!(runtime.result_info(result), 0);
    assert_eq!(runtime.result_free(result), wire::INVALID_HANDLE);
    let next = runtime.runtime_catalog();
    assert!(next > result);
}

#[test]
fn input_registry_checks_lifetimes_subranges_overflow_and_exact_free() {
    let mut runtime = Runtime::default();
    let (offset, len) = input(&mut runtime, b"0123456789");
    assert_eq!(runtime.inputs.borrow(offset + 2, 3).unwrap(), b"234");
    assert!(runtime.inputs.borrow(offset, len + 1).is_err());
    assert!(runtime.inputs.borrow(u32::MAX, 2).is_err());
    assert!(runtime.inputs.borrow(0, 1).is_err());
    assert!(runtime.inputs.borrow(1, 0).is_err());
    assert_eq!(runtime.inputs.borrow(0, 0).unwrap(), b"");
    assert_eq!(runtime.free(offset + 1), wire::INVALID_ALLOCATION);
    assert_eq!(runtime.free(offset), 0);
    assert!(runtime.inputs.borrow(offset, len).is_err());
    assert_eq!(runtime.free(offset), wire::INVALID_ALLOCATION);
    assert_eq!(runtime.free(0), wire::INVALID_ALLOCATION);
    assert_eq!(runtime.alloc(0), 0);
}

#[test]
fn invalid_constructor_allocations_return_owned_errors_and_leave_no_engine() {
    let mut runtime = Runtime::default();
    let result = runtime.engine_new(u32::MAX, 2, 0, 0);
    assert_eq!(
        error_json(&runtime, result)["code_name"],
        "MERMAN_INVALID_ARGUMENT"
    );
    assert_eq!(runtime.counts().0, 0);
    let info = runtime.result_info(result);
    let other = runtime.engine_new(info, 40, 0, 0);
    assert_eq!(
        error_json(&runtime, other)["code_name"],
        "MERMAN_INVALID_ARGUMENT"
    );
    let good_catalog = runtime.runtime_catalog();
    assert_eq!(fields(&runtime, good_catalog)[2], 0);
}

#[test]
fn envelope_version_raw_options_and_empty_option_detection_are_transport_only() {
    let config: Config<'_> = wire::parse(
        br#"{"version":1,"options": {"version":2,"version":2}}"#,
        512,
    )
    .unwrap();
    assert_eq!(
        config.options.unwrap().get(),
        r#"{"version":2,"version":2}"#
    );
    for raw in ["{}", "{  }", " {\n } "] {
        let raw = serde_json::value::RawValue::from_string(raw.to_owned()).unwrap();
        assert!(wire::nonempty_options(Some(&raw)).is_none());
    }
    for raw in ["[]", "17", r#"{"resources":{"profile":"constrained"}}"#] {
        let raw = serde_json::value::RawValue::from_string(raw.to_owned()).unwrap();
        assert!(wire::nonempty_options(Some(&raw)).is_some());
    }
    let request: Request<'_> =
        wire::parse(br#"{"version":1,"operation_id":"\u0073vg"}"#, 512).unwrap();
    assert_eq!(request.operation_id, "svg");
    assert!(wire::check_version(2).is_err());
    assert!(wire::parse::<Config<'_>>(br#"{"version":1,"unknown":true}"#, 512).is_err());
    assert!(wire::parse::<Config<'_>>(br#"{"version":1}"#, 2).is_err());
}

#[test]
fn input_result_and_handle_budgets_fail_without_releasing_other_owners() {
    let mut runtime = Runtime::default();
    runtime.limits.inputs = 1;
    runtime.limits.input_size = 4;
    runtime.limits.input_bytes = 4;
    assert_eq!(runtime.alloc(5), 0);
    let allocation = runtime.alloc(4);
    assert_ne!(allocation, 0);
    assert_eq!(runtime.alloc(1), 0);
    assert_eq!(runtime.free(allocation), 0);
    assert_ne!(runtime.alloc(1), 0);
    runtime.limits.results = 1;
    let first = runtime.runtime_catalog();
    assert_ne!(first, 0);
    assert_eq!(runtime.runtime_catalog(), 0);
    assert_ne!(runtime.result_info(first), 0);
    assert_eq!(runtime.result_free(first), 0);
    assert_ne!(runtime.runtime_catalog(), 0);
    let mut exhausted = Runtime::default();
    exhausted.exhaust_handles(u32::MAX);
    assert_eq!(exhausted.runtime_catalog(), 0);
    assert_eq!(exhausted.counts(), (0, 0, 0, 0));
}

#[test]
fn oversized_results_become_small_owned_errors_when_the_error_fits() {
    let mut runtime = Runtime::default();
    runtime.limits.result_size = 1024;
    runtime.limits.result_bytes = 1024;
    let result = runtime.runtime_catalog();
    let error = error_json(&runtime, result);
    assert_eq!(error["code_name"], "MERMAN_RESOURCE_LIMIT_EXCEEDED");
    assert!(runtime.counts().3 <= 1024);
    runtime.result_free(result);
    runtime.limits.result_bytes = 1;
    assert_eq!(runtime.runtime_catalog(), 0);
}

#[cfg(not(feature = "native-fonts"))]
#[test]
fn no_font_feature_constructor_returns_typed_missing_capability() {
    let mut runtime = Runtime::default();
    let (offset, len) = input(&mut runtime, br#"{"version":1}"#);
    let result = runtime.engine_new(offset, len, 0, 0);
    let error = error_json(&runtime, result);
    assert_eq!(error["kind"], "missing-capability");
    assert_eq!(error["capability_id"], "native-fonts");
    assert_eq!(runtime.counts().0, 0);
}

#[cfg(feature = "native-fonts")]
fn new_engine(runtime: &mut Runtime, options: Option<Value>, bundled: bool) -> (u32, u32) {
    let fonts = include_bytes!("../../merman-export/tests/fonts/Cousine-Regular.ttf");
    let (fonts_off, fonts_len) = input(runtime, fonts);
    let mut config = json!({"version": 1, "fonts": [{"offset": 0, "length": fonts_len}], "bundled_fonts": bundled});
    if let Some(options) = options {
        config["options"] = options;
    }
    let (config_off, config_len) = input(runtime, &serde_json::to_vec(&config).unwrap());
    let result = runtime.engine_new(config_off, config_len, fonts_off, fonts_len);
    assert_eq!(runtime.free(config_off), 0);
    assert_eq!(runtime.free(fonts_off), 0);
    assert_eq!(
        fields(runtime, result)[2],
        0,
        "{}",
        String::from_utf8_lossy(segment(runtime, result, 8))
    );
    (fields(runtime, result)[3], result)
}

#[cfg(feature = "native-fonts")]
fn execute(
    runtime: &mut Runtime,
    engine: u32,
    operation: &str,
    source: &[u8],
    options: Option<Value>,
) -> u32 {
    let mut request = json!({"version": 1, "operation_id": operation});
    if let Some(options) = options {
        request["options"] = options;
    }
    let (request_off, request_len) = input(runtime, &serde_json::to_vec(&request).unwrap());
    let (source_off, source_len) = input(runtime, source);
    let result = runtime.execute(engine, request_off, request_len, source_off, source_len);
    assert_eq!(runtime.free(request_off), 0);
    if source_len > 0 {
        assert_eq!(runtime.free(source_off), 0);
    }
    result
}

#[cfg(feature = "native-fonts")]
#[test]
fn engine_and_result_handles_are_disjoint_never_reissued_and_results_outlive_close() {
    let mut runtime = Runtime::default();
    let (engine, constructor) = new_engine(&mut runtime, None, false);
    assert_ne!(engine, constructor);
    assert_eq!(runtime.engine_close(constructor), wire::INVALID_HANDLE);
    assert_eq!(runtime.result_free(engine), wire::INVALID_HANDLE);
    let result = runtime.runtime_catalog();
    let saved = runtime.result(result).unwrap().bytes.to_vec();
    assert_eq!(runtime.result_free(constructor), 0);
    assert_eq!(runtime.engine_close(engine), 0);
    assert_eq!(runtime.engine_close(engine), wire::INVALID_HANDLE);
    assert_eq!(runtime.result(result).unwrap().bytes.as_ref(), saved);
    assert_eq!(runtime.counts().2, 0);
    let stale = execute(&mut runtime, engine, "semantic-json", b"", None);
    assert_eq!(
        error_json(&runtime, stale)["code_name"],
        "MERMAN_INVALID_ARGUMENT"
    );
    let (next_engine, next_constructor) = new_engine(&mut runtime, None, false);
    assert!(next_engine > engine && next_constructor > constructor);
}

#[cfg(feature = "native-fonts")]
#[test]
fn constructor_result_failure_is_transactional_even_at_handle_exhaustion() {
    for last in [0, u32::MAX - 1] {
        let mut runtime = Runtime::default();
        if last == 0 {
            runtime.limits.results = 0;
        } else {
            runtime.exhaust_handles(last);
        }
        let fonts = include_bytes!("../../merman-export/tests/fonts/Cousine-Regular.ttf");
        let (font_off, font_len) = input(&mut runtime, fonts);
        let config = json!({"version": 1, "fonts": [{"offset": 0, "length": font_len}], "bundled_fonts": false});
        let (config_off, config_len) = input(&mut runtime, &serde_json::to_vec(&config).unwrap());
        assert_eq!(
            runtime.engine_new(config_off, config_len, font_off, font_len),
            0
        );
        assert_eq!(runtime.counts(), (0, 0, 0, 0));
    }
}

#[cfg(feature = "native-fonts")]
#[test]
fn malformed_fonts_ranges_and_font_engine_budgets_leave_no_partial_engine() {
    for (offset, length, bytes) in [
        (0, 5, b"bad".as_slice()),
        (u32::MAX, 2, b"bad"),
        (0, 3, b"bad"),
    ] {
        let mut runtime = Runtime::default();
        let (fonts_off, fonts_len) = input(&mut runtime, bytes);
        let config = json!({"version": 1, "fonts": [{"offset": offset, "length": length}], "bundled_fonts": false});
        let (config_off, config_len) = input(&mut runtime, &serde_json::to_vec(&config).unwrap());
        let result = runtime.engine_new(config_off, config_len, fonts_off, fonts_len);
        error_json(&runtime, result);
        assert_eq!(runtime.counts().0, 0);
        assert_eq!(runtime.counts().2, 0);
    }
    let mut runtime = Runtime::default();
    let (engine, _) = new_engine(&mut runtime, None, false);
    runtime.limits.engines = 1;
    let (config_off, config_len) = input(&mut runtime, br#"{"version":1}"#);
    let result = runtime.engine_new(config_off, config_len, 0, 0);
    assert_eq!(
        error_json(&runtime, result)["code_name"],
        "MERMAN_RESOURCE_LIMIT_EXCEEDED"
    );
    assert_eq!(runtime.counts().0, 1);
    assert_eq!(runtime.engine_close(engine), 0);
    runtime.limits.engine_font_bytes = 1;
    let fonts = include_bytes!("../../merman-export/tests/fonts/Cousine-Regular.ttf");
    let (font_off, font_len) = input(&mut runtime, fonts);
    let config =
        json!({"version": 1, "fonts": [{"offset": 0, "length": font_len}], "bundled_fonts": false});
    let (off, len) = input(&mut runtime, &serde_json::to_vec(&config).unwrap());
    let result = runtime.engine_new(off, len, font_off, font_len);
    assert_eq!(
        error_json(&runtime, result)["code_name"],
        "MERMAN_RESOURCE_LIMIT_EXCEEDED"
    );
    assert_eq!(runtime.counts().0, 0);
}

#[cfg(all(feature = "native-fonts", feature = "diagram-flowchart"))]
#[test]
fn real_svg_and_raw_png_survive_errors_input_release_and_engine_close() {
    const SOURCE: &[u8] = b"flowchart TD\n A[Hello] --> B[World]\n";
    let mut runtime = Runtime::default();
    let (engine, constructor) = new_engine(&mut runtime, Some(json!({"version": 2})), false);
    runtime.result_free(constructor);
    for (operation, source, options) in [
        ("svg", b"flowchart ???".as_slice(), None),
        ("svg", b"\xff".as_slice(), None),
        ("svg", SOURCE, Some(json!({"unknown_option": true}))),
        ("svg", SOURCE, Some(json!(17))),
        ("svg", SOURCE, Some(json!({"version": 3}))),
        ("not-a-real-operation", SOURCE, None),
    ] {
        let error = execute(&mut runtime, engine, operation, source, options);
        error_json(&runtime, error);
        runtime.result_free(error);
    }
    let invalid_input = runtime.execute(engine, u32::MAX, 2, 0, 0);
    error_json(&runtime, invalid_input);
    runtime.result_free(invalid_input);
    let svg = execute(
        &mut runtime,
        engine,
        "svg",
        SOURCE,
        Some(json!({"version": 2})),
    );
    assert_eq!(
        fields(&runtime, svg)[2],
        0,
        "{}",
        String::from_utf8_lossy(segment(&runtime, svg, 8))
    );
    assert_eq!(segment(&runtime, svg, 4), b"image/svg+xml");
    let text = std::str::from_utf8(segment(&runtime, svg, 6)).unwrap();
    assert!(text.contains("<svg") && text.contains("Hello") && text.contains("World"));
    assert!(!text.contains("base64,") && !text.contains("@font-face"));
    let metadata: Value = serde_json::from_slice(segment(&runtime, svg, 8)).unwrap();
    assert_eq!(metadata["operation_id"], "svg");
    assert!(metadata["svg_viewport"]["width"].as_f64().unwrap() > 0.0);
    #[cfg(feature = "png")]
    let png = {
        let result = execute(&mut runtime, engine, "png", SOURCE, None);
        assert_eq!(
            fields(&runtime, result)[2],
            0,
            "{}",
            String::from_utf8_lossy(segment(&runtime, result, 8))
        );
        assert_eq!(segment(&runtime, result, 4), b"image/png");
        let bytes = segment(&runtime, result, 6);
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(u32::from_be_bytes(bytes[16..20].try_into().unwrap()) > 0);
        assert!(u32::from_be_bytes(bytes[20..24].try_into().unwrap()) > 0);
        let metadata: Value = serde_json::from_slice(segment(&runtime, result, 8)).unwrap();
        assert_eq!(
            metadata["output_plan"]["width_px"],
            u32::from_be_bytes(bytes[16..20].try_into().unwrap())
        );
        assert_eq!(
            metadata["output_plan"]["height_px"],
            u32::from_be_bytes(bytes[20..24].try_into().unwrap())
        );
        result
    };
    let saved = segment(&runtime, svg, 6).to_vec();
    assert_eq!(runtime.engine_close(engine), 0);
    assert_eq!(segment(&runtime, svg, 6), saved);
    #[cfg(feature = "png")]
    assert_eq!(runtime.result_free(png), 0);
    assert_eq!(runtime.result_free(svg), 0);
    assert_eq!(runtime.counts(), (0, 0, 0, 0));
}

#[cfg(feature = "native-fonts")]
#[test]
fn all_advertised_metadata_remains_callable_through_the_shared_engine() {
    let mut runtime = Runtime::default();
    let (engine, _) = new_engine(&mut runtime, None, false);
    for key in crate::artifact_contract().metadata_keys() {
        let result = execute(&mut runtime, engine, key.id(), b"", None);
        assert_eq!(fields(&runtime, result)[2], 0, "metadata {}", key.id());
        let _: Value = serde_json::from_slice(segment(&runtime, result, 6)).unwrap();
        assert_eq!(runtime.result_free(result), 0);
    }
}

#[cfg(feature = "bundled-fonts")]
#[test]
fn bundled_font_constructor_needs_no_host_font_blob() {
    let mut runtime = Runtime::default();
    let (offset, len) = input(&mut runtime, br#"{"version":1}"#);
    let result = runtime.engine_new(offset, len, 0, 0);
    assert_eq!(
        fields(&runtime, result)[2],
        0,
        "{}",
        String::from_utf8_lossy(segment(&runtime, result, 8))
    );
    let engine = fields(&runtime, result)[3];
    assert!(runtime.counts().2 > 0);
    assert_eq!(runtime.free(offset), 0);
    assert_eq!(runtime.result_free(result), 0);
    assert_eq!(runtime.engine_close(engine), 0);
}

#[cfg(all(
    feature = "native-fonts",
    feature = "diagram-flowchart",
    feature = "math"
))]
#[test]
fn default_interactive_and_legacy_constrained_math_preserve_maintenance_policy() {
    const SOURCE: &[u8] = b"flowchart TD\nA[\"$$x^2$$\"] --> B[Done]";
    let mut runtime = Runtime::default();
    let (engine, constructor) = new_engine(&mut runtime, None, false);
    runtime.result_free(constructor);
    let svg = execute(&mut runtime, engine, "svg", SOURCE, None);
    assert_eq!(
        fields(&runtime, svg)[2],
        0,
        "{}",
        String::from_utf8_lossy(segment(&runtime, svg, 8))
    );
    assert!(
        std::str::from_utf8(segment(&runtime, svg, 6))
            .unwrap()
            .contains("<svg")
    );
    runtime.result_free(svg);
    let constrained_svg = execute(
        &mut runtime,
        engine,
        "svg",
        SOURCE,
        Some(json!({"resources": {"profile": "constrained"}})),
    );
    assert_eq!(
        fields(&runtime, constrained_svg)[2],
        0,
        "{}",
        String::from_utf8_lossy(segment(&runtime, constrained_svg, 8))
    );
    runtime.result_free(constrained_svg);
    let recovered = execute(&mut runtime, engine, "svg", SOURCE, Some(json!({})));
    assert_eq!(fields(&runtime, recovered)[2], 0);
    runtime.result_free(recovered);
    #[cfg(feature = "png")]
    {
        let png = execute(&mut runtime, engine, "png", SOURCE, None);
        assert_eq!(
            fields(&runtime, png)[2],
            0,
            "{}",
            String::from_utf8_lossy(segment(&runtime, png, 8))
        );
        assert!(segment(&runtime, png, 6).starts_with(b"\x89PNG\r\n\x1a\n"));
        runtime.result_free(png);
        let png = execute(
            &mut runtime,
            engine,
            "png",
            SOURCE,
            Some(json!({"resources": {"profile": "constrained"}})),
        );
        assert_eq!(fields(&runtime, png)[2], 0);
        assert!(segment(&runtime, png, 6).starts_with(b"\x89PNG\r\n\x1a\n"));
        runtime.result_free(png);
    }
    runtime.engine_close(engine);
    let (constrained, constructor) = new_engine(
        &mut runtime,
        Some(json!({"resources": {"profile": "constrained"}})),
        false,
    );
    runtime.result_free(constructor);
    for operation in [
        "svg",
        #[cfg(feature = "png")]
        "png",
    ] {
        let result = execute(
            &mut runtime,
            constrained,
            operation,
            SOURCE,
            Some(json!({})),
        );
        assert_eq!(
            fields(&runtime, result)[2],
            0,
            "{}",
            String::from_utf8_lossy(segment(&runtime, result, 8))
        );
        runtime.result_free(result);
    }
    runtime.engine_close(constrained);
}

#[test]
fn descriptor_status_is_not_a_native_or_binding_status_alias() {
    let error = merman_bindings_core::BindingError::new(BindingStatus::ParseError, "test");
    let mut runtime = Runtime::default();
    let result = runtime.engine_new(u32::MAX, 1, 0, 0);
    assert_eq!(fields(&runtime, result)[2], wire::STATUS_ERROR);
    assert_ne!(wire::STATUS_ERROR, error.status().code() as u32);
}

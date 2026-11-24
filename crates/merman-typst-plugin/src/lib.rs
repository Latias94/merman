//! Typst WebAssembly plugin bridge for `merman`.
//!
//! This crate exposes one versioned Typst transport contract. The package wrapper may present
//! convenient render, analysis, and theme-authoring helpers, but every plugin operation returns the
//! same closed transport envelope so hosts can handle failures without depending on trap behavior.

use merman_bindings_core::{
    ArtifactContractSpec, BindingError, BindingOperationRequest, BindingTransportKey,
    CapabilityKey, MetadataKey, OperationKey, RuntimePolicyExposure, TargetKey,
    ValidatedArtifactContract,
};
use serde_json::{Value, json};

const TYPST_RESULT_PAYLOAD_SCHEMA_VERSION: u32 = 1;
pub const TYPST_RUNTIME_CATALOG_SCHEMA_VERSION: u32 =
    merman_bindings_core::RUNTIME_CATALOG_SCHEMA_VERSION;
/// Closed binding-operation allowlist owned by the Typst transport.
///
/// These are semantic operation IDs from the shared capability descriptor, not WebAssembly export
/// names. Artifact profiles may select a subset, but must never infer additional operations merely
/// because their capabilities could support them.
pub const TYPST_TRANSPORT_OPERATION_KEYS: &[OperationKey] = &[
    OperationKey::AnalysisJson,
    OperationKey::ExportThemePreset,
    OperationKey::ResolveTheme,
    OperationKey::Svg,
];
const RENDER_OPERATION: &str = "render-svg";
const ANALYZE_OPERATION: &str = "analyze";
const THEME_OPERATION: &str = "theme-operation";
const TYPST_OPERATION_ID_MAX_UTF8_BYTES: usize = 128;
const TYPST_RESOURCE_PROFILE: &str = "constrained";
const LAYOUT_WORK_LIMIT: &str = "max_layout_work_units";
const THEME_OPERATION_KEYS: &[OperationKey] =
    &[OperationKey::ExportThemePreset, OperationKey::ResolveTheme];
const TYPST_OPERATIONS: &[OperationKey] = &[
    #[cfg(feature = "analysis")]
    OperationKey::AnalysisJson,
    #[cfg(feature = "svg")]
    OperationKey::ExportThemePreset,
    #[cfg(feature = "svg")]
    OperationKey::ResolveTheme,
    #[cfg(feature = "svg")]
    OperationKey::Svg,
];
const TYPST_SUPPLEMENTAL_CAPABILITIES: &[CapabilityKey] = &[
    #[cfg(feature = "layout-cytoscape")]
    CapabilityKey::LayoutCytoscape,
    #[cfg(feature = "layout-dagre")]
    CapabilityKey::LayoutDagre,
    #[cfg(feature = "layout-elk")]
    CapabilityKey::LayoutElk,
];
static ARTIFACT_CONTRACT: ValidatedArtifactContract =
    ArtifactContractSpec::new(TargetKey::Typst, BindingTransportKey::Typst)
        .with_operations(TYPST_OPERATIONS)
        .with_metadata(&[MetadataKey::ThemeCatalog])
        .with_supplemental_capabilities(TYPST_SUPPLEMENTAL_CAPABILITIES)
        .with_runtime_policy_exposure(RuntimePolicyExposure::DeterministicOnly)
        .materialize();

#[cfg(target_arch = "wasm32")]
mod wasm_exports;

// Preserve the Rust paths previously emitted at the crate root by the protocol macro.
#[cfg(target_arch = "wasm32")]
#[doc(hidden)]
pub use wasm_exports::{
    __wasm_minimal_protocol_internal_function_abi_version,
    __wasm_minimal_protocol_internal_function_analyze_json,
    __wasm_minimal_protocol_internal_function_capabilities_json,
    __wasm_minimal_protocol_internal_function_package_version,
    __wasm_minimal_protocol_internal_function_render_svg_json,
    __wasm_minimal_protocol_internal_function_theme_catalog_json,
    __wasm_minimal_protocol_internal_function_theme_operation_json,
};

include!("generated/typst_plugin_abi.rs");

#[cfg(all(
    any(
        feature = "layout-cytoscape",
        feature = "layout-dagre",
        feature = "layout-elk"
    ),
    not(feature = "svg")
))]
compile_error!(
    "Typst layout features must enable the crate `svg` feature so constrained resources cannot be bypassed"
);

pub fn abi_version() -> &'static [u8] {
    TYPST_PLUGIN_ABI_VERSION_BYTES
}

pub fn package_version() -> Vec<u8> {
    env!("CARGO_PKG_VERSION").as_bytes().to_vec()
}

pub fn capabilities_json() -> Vec<u8> {
    typst_capabilities_json()
}

/// Returns the shared theme catalog for the current artifact.
pub fn theme_catalog_json() -> Vec<u8> {
    let operation = MetadataKey::ThemeCatalog.id();
    let result = typst_artifact_contract().metadata_json(operation);
    typst_theme_payload(operation, result)
}

fn typst_artifact_contract() -> &'static ValidatedArtifactContract {
    &ARTIFACT_CONTRACT
}

fn typst_capabilities_json() -> Vec<u8> {
    let catalog = typst_artifact_contract().runtime_catalog(TYPST_PLUGIN_ABI_VERSION);
    serde_json::to_vec(&catalog).expect("the checked Typst capability catalog is serializable")
}

pub fn render_svg_json(source: &[u8], options_json: &[u8]) -> Vec<u8> {
    match execute_typst_once_data(OperationKey::Svg, source, options_json) {
        Ok(svg) => match std::str::from_utf8(&svg) {
            Ok(svg) => typst_success_payload(RENDER_OPERATION, json!({ "svg": svg })),
            Err(error) => typst_internal_error_payload(
                RENDER_OPERATION,
                format!("render_svg returned non-UTF-8 SVG: {error}"),
            ),
        },
        Err(error) => typst_binding_error_payload(RENDER_OPERATION, &error),
    }
}

pub fn analyze_json(source: &[u8], options_json: &[u8]) -> Vec<u8> {
    match execute_typst_once_data(OperationKey::AnalysisJson, source, options_json) {
        Ok(analysis_json) => match serde_json::from_slice::<Value>(&analysis_json) {
            Ok(analysis) => {
                typst_success_payload(ANALYZE_OPERATION, json!({ "analysis": analysis }))
            }
            Err(error) => typst_internal_error_payload(
                ANALYZE_OPERATION,
                format!("analyze_json returned invalid canonical JSON: {error}"),
            ),
        },
        Err(error) => typst_binding_error_payload(ANALYZE_OPERATION, &error),
    }
}

/// Executes one versioned theme-authoring operation through the shared binding contract.
///
/// The operation ID selects preset export or document resolution. Rendering and analysis keep
/// their dedicated exports.
pub fn theme_operation_json(operation_id: &[u8], input: &[u8], options_json: &[u8]) -> Vec<u8> {
    let operation_id = match admit_typst_operation_id(operation_id) {
        Ok(operation_id) => operation_id,
        Err(error) => return typst_binding_error_payload(THEME_OPERATION, &error),
    };
    let operation = match OperationKey::from_id(operation_id) {
        Some(operation) if THEME_OPERATION_KEYS.contains(&operation) => operation,
        Some(operation) => {
            let error = merman_bindings_core::BindingError::unsupported_operation(
                "the requested operation is not a Typst theme authoring operation",
            );
            return typst_binding_error_payload(operation.id(), &error);
        }
        None => {
            let error = merman_bindings_core::BindingError::unsupported_operation(
                "unknown Typst theme authoring operation",
            );
            return typst_binding_error_payload(THEME_OPERATION, &error);
        }
    };
    let output = typst_artifact_contract().execute_once_data(
        BindingOperationRequest::new(operation.id(), input).with_options_json(options_json),
    );
    typst_theme_payload(operation.id(), output)
}

fn typst_theme_payload(
    operation: &str,
    output: Result<Vec<u8>, merman_bindings_core::BindingError>,
) -> Vec<u8> {
    match output {
        Ok(output) => match serde_json::from_slice::<Value>(&output) {
            Ok(result) => typst_success_payload(operation, json!({ "result": result })),
            Err(error) => typst_internal_error_payload(
                operation,
                format!("theme operation returned invalid canonical JSON: {error}"),
            ),
        },
        Err(error) => typst_binding_error_payload(operation, &error),
    }
}

fn admit_typst_operation_id(
    operation_id: &[u8],
) -> Result<&str, merman_bindings_core::BindingError> {
    if operation_id.is_empty() || operation_id.len() > TYPST_OPERATION_ID_MAX_UTF8_BYTES {
        return Err(merman_bindings_core::BindingError::invalid_argument(
            format!(
                "theme operation id must contain 1 to {TYPST_OPERATION_ID_MAX_UTF8_BYTES} UTF-8 bytes"
            ),
        ));
    }
    let operation_id = std::str::from_utf8(operation_id).map_err(|_| {
        merman_bindings_core::BindingError::invalid_argument(
            "theme operation id must be valid UTF-8",
        )
    })?;
    if operation_id.chars().any(char::is_control) {
        return Err(merman_bindings_core::BindingError::invalid_argument(
            "theme operation id must not contain control characters",
        ));
    }
    Ok(operation_id)
}

fn typst_success_payload(operation: &str, data: Value) -> Vec<u8> {
    serde_json::to_vec(&typst_result_payload(
        operation,
        merman_bindings_core::BindingStatus::Ok,
        None,
        None,
        None,
        Some(data),
    ))
    .expect("Typst result envelope is serializable")
}

fn typst_binding_error_payload(
    operation: &str,
    error: &merman_bindings_core::BindingError,
) -> Vec<u8> {
    let mut payload = typst_result_payload(
        operation,
        error.status(),
        Some(error.kind().id()),
        error.capability_id(),
        Some(error.message()),
        None,
    );
    if let Some(details) = merman_bindings_core::binding_error_details_json(error)
        .expect("binding error details are serializable")
    {
        payload["details"] = details;
    }
    serde_json::to_vec(&payload).expect("Typst result envelope is serializable")
}

fn typst_internal_error_payload(operation: &str, message: String) -> Vec<u8> {
    serde_json::to_vec(&typst_result_payload(
        operation,
        merman_bindings_core::BindingStatus::InternalError,
        Some("generic"),
        None,
        Some(&message),
        None,
    ))
    .expect("Typst result envelope is serializable")
}

fn typst_result_payload(
    operation: &str,
    status: merman_bindings_core::BindingStatus,
    kind: Option<&str>,
    capability_id: Option<&str>,
    message: Option<&str>,
    data: Option<Value>,
) -> Value {
    let ok = status == merman_bindings_core::BindingStatus::Ok;
    json!({
        "version": TYPST_RESULT_PAYLOAD_SCHEMA_VERSION,
        "operation": operation,
        "ok": ok,
        "code": status.code(),
        "code_name": status.code_name(),
        "kind": if ok { None } else { kind },
        "capability_id": if ok { None } else { capability_id },
        "message": message,
        "data": data,
    })
}

fn execute_typst_once_data(
    operation: OperationKey,
    source: &[u8],
    options_json: &[u8],
) -> Result<Vec<u8>, BindingError> {
    let ceiling_override = typst_layout_work_ceiling(operation);
    typst_artifact_contract().execute_once_data_with_resource_ceiling(
        BindingOperationRequest::new(operation.id(), source).with_options_json(options_json),
        TYPST_RESOURCE_PROFILE,
        ceiling_override.as_slice(),
        &[LAYOUT_WORK_LIMIT],
        apply_typst_options_policy,
    )
}

fn typst_layout_work_ceiling(operation: OperationKey) -> Option<(&'static str, usize)> {
    match operation {
        #[cfg(feature = "svg")]
        OperationKey::Svg => {
            let contract = merman_bindings_core::binding_resource_contract();
            let interactive = contract
                .profiles
                .iter()
                .find(|profile| profile.id == "interactive")
                .expect("the shared resource contract includes the interactive profile");
            // Document layouts use the calibrated interactive work allowance while other limits
            // retain the constrained host policy. Explicit caller profiles keep their own defaults.
            Some((
                LAYOUT_WORK_LIMIT,
                interactive.limits[LAYOUT_WORK_LIMIT]
                    .expect("interactive layout work has a finite allowance"),
            ))
        }
        _ => None,
    }
}

fn apply_typst_options_policy(options: &mut Value) -> Result<(), BindingError> {
    let root = options
        .as_object_mut()
        .ok_or_else(|| BindingError::internal("normalized Typst options must be an object"))?;

    match root.get("runtime_policy") {
        Some(Value::String(policy)) if policy == "native" => {
            return Err(BindingError::missing_capability(
                "system-clock",
                "runtime_policy=native is not available in the Typst transport",
            ));
        }
        Some(Value::Null) | None => {
            root.insert("runtime_policy".to_string(), Value::from("deterministic"));
        }
        Some(_) => {}
    }

    if TYPST_OPERATIONS.contains(&OperationKey::Svg) {
        let environment = root
            .entry("environment".to_string())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if environment.is_null() {
            *environment = Value::Object(serde_json::Map::new());
        }
        let environment = environment.as_object_mut().ok_or_else(|| {
            BindingError::invalid_options_json(
                "invalid options_json: `environment` must be an object",
            )
        })?;
        match environment.get("math_renderer") {
            Some(Value::String(renderer)) if renderer.trim().eq_ignore_ascii_case("ratex") => {
                return Err(BindingError::missing_capability(
                    "math",
                    "environment.math_renderer=ratex is not available in the Typst transport",
                ));
            }
            Some(Value::String(renderer)) if renderer.trim().eq_ignore_ascii_case("none") => {}
            Some(Value::String(renderer)) => {
                return Err(BindingError::invalid_argument(format!(
                    "unsupported environment.math_renderer: {renderer}"
                )));
            }
            Some(_) => {
                return Err(BindingError::invalid_options_json(
                    "invalid options_json: `environment.math_renderer` must be a string",
                ));
            }
            None => {}
        }
        environment.insert("math_renderer".to_string(), Value::from("none"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Build the equivalent wire request for comparison with the ordinary binding executor.
    fn reference_options_json(
        operation: OperationKey,
        options_json: &[u8],
    ) -> Result<Vec<u8>, BindingError> {
        let ceiling_override = typst_layout_work_ceiling(operation);
        let normalized = merman_bindings_core::apply_resource_ceiling_json_with_allowed_overrides(
            options_json,
            TYPST_RESOURCE_PROFILE,
            ceiling_override.as_slice(),
            &[LAYOUT_WORK_LIMIT],
        )?;
        let mut options: Value = serde_json::from_slice(&normalized).unwrap();
        apply_typst_options_policy(&mut options)?;
        Ok(serde_json::to_vec(&options).unwrap())
    }

    #[test]
    fn abi_version_is_stable() {
        assert_eq!(TYPST_PLUGIN_ABI_VERSION, 4);
        assert_eq!(TYPST_PLUGIN_ABI_VERSION_BYTES, b"4");
        assert_eq!(abi_version(), b"4");
    }

    #[test]
    fn theme_catalog_exposes_schema_two() {
        let payload: Value = serde_json::from_slice(&theme_catalog_json()).unwrap();
        assert_success_envelope(&payload, "theme-catalog");
        let catalog = &payload["data"]["result"];
        assert_eq!(catalog["schema_version"], 2);
        assert!(
            catalog["mermaid_themes"]
                .as_array()
                .unwrap()
                .contains(&json!("forest"))
        );
        assert_eq!(
            catalog["presets"].as_array().unwrap().len(),
            if cfg!(feature = "svg") { 13 } else { 0 }
        );
        assert!(catalog["families"].is_array());
        let runtime: Value = serde_json::from_slice(&capabilities_json()).unwrap();
        assert_eq!(runtime["metadata_ids"], json!(["theme-catalog"]));
    }

    #[test]
    fn package_version_matches_crate_version() {
        assert_eq!(package_version(), env!("CARGO_PKG_VERSION").as_bytes());
    }

    #[test]
    fn capabilities_json_exposes_the_flat_artifact_runtime_catalog() {
        let payload: Value = serde_json::from_slice(&capabilities_json()).expect("valid JSON");
        let expected_catalog = typst_artifact_contract().runtime_catalog(TYPST_PLUGIN_ABI_VERSION);

        assert_eq!(payload, serde_json::to_value(&expected_catalog).unwrap());
        assert_eq!(
            payload["schema_version"],
            TYPST_RUNTIME_CATALOG_SCHEMA_VERSION
        );
        assert_eq!(payload["transport_api_version"], TYPST_PLUGIN_ABI_VERSION);
        assert!(payload.get("runtime_contract").is_none());
        assert!(payload.get("capability_vocabulary").is_none());
        assert!(payload.get("payload_schema_version").is_none());

        let capabilities = &payload["capabilities"];
        let capability_ids = capabilities["capability_ids"].as_array().unwrap();
        for omitted in ["ascii", "jpeg", "math", "pdf", "png"] {
            assert!(
                !capability_ids.iter().any(|id| id == omitted),
                "Typst must not advertise uncallable capability {omitted}"
            );
        }
        assert!(
            capabilities["system_adapter_ids"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        if let Some(text_measurement) = capabilities["text_measurement"].as_object() {
            assert_eq!(
                text_measurement["provider_ids"],
                json!([merman_bindings_core::TEXT_MEASUREMENT_PROVIDER_DETERMINISTIC])
            );
        }
    }

    #[test]
    fn typst_target_projection_tracks_resolved_backend_and_closed_operation_set() {
        let projected = typst_artifact_contract().runtime_capabilities();
        assert_eq!(
            projected.operation_ids,
            TYPST_OPERATIONS
                .iter()
                .copied()
                .map(OperationKey::id)
                .collect::<Vec<_>>()
        );
        assert!(
            TYPST_OPERATIONS
                .iter()
                .all(|operation| TYPST_TRANSPORT_OPERATION_KEYS.contains(operation))
        );
        assert_eq!(
            TYPST_TRANSPORT_OPERATION_KEYS
                .iter()
                .copied()
                .map(OperationKey::id)
                .collect::<Vec<_>>(),
            [
                "analysis-json",
                "export-theme-preset",
                "resolve-theme",
                "svg",
            ]
        );
        assert_eq!(
            projected.has_operation("analysis-json"),
            cfg!(feature = "analysis")
        );
        assert_eq!(projected.has_operation("svg"), cfg!(feature = "svg"));
        assert!(!projected.has_capability("embedded-fonts"));
        assert_eq!(
            projected.has_operation("resolve-theme"),
            cfg!(feature = "svg")
        );
        assert_eq!(
            projected.has_operation("export-theme-preset"),
            cfg!(feature = "svg")
        );
        #[cfg(feature = "svg")]
        {
            assert_eq!(
                projected.capability_ids.contains(&"layout-cytoscape"),
                cfg!(feature = "layout-cytoscape")
            );
            assert_eq!(
                projected.has_capability("layout-dagre"),
                cfg!(feature = "layout-dagre")
            );
            assert_eq!(
                projected.capability_ids.contains(&"layout-elk"),
                cfg!(feature = "layout-elk")
            );
        }
    }

    #[test]
    fn typst_execution_is_bound_to_the_closed_operation_set() {
        let error = typst_artifact_contract()
            .execute_once_data(BindingOperationRequest::new(
                "semantic-json",
                b"flowchart TD\nA --> B",
            ))
            .expect_err("the Typst transport must not inherit the Rust semantic operation");

        assert_eq!(
            error.status(),
            merman_bindings_core::BindingStatus::UnsupportedOperation
        );
        assert!(error.message().contains("not exposed by target `typst`"));
    }

    #[test]
    fn typst_data_execution_matches_full_errors() {
        for (operation, export, transport_operation) in [
            (
                OperationKey::Svg,
                render_svg_json as fn(&[u8], &[u8]) -> Vec<u8>,
                RENDER_OPERATION,
            ),
            (OperationKey::AnalysisJson, analyze_json, ANALYZE_OPERATION),
        ] {
            for (source, options) in [
                (b"".as_slice(), b"{".as_slice()),
                (b"".as_slice(), b"\xff".as_slice()),
                (b"\xff".as_slice(), b"".as_slice()),
                (b"".as_slice(), br#"{"runtime_policy":"native"}"#),
                (b"".as_slice(), br#"{"runtime_policy":" Native "}"#),
                (
                    b"".as_slice(),
                    br#"{"unknown_option":true,"runtime_policy":"native"}"#,
                ),
                (
                    b"flowchart TD\nA --> B".as_slice(),
                    br#"{"resources":{"limits":{"max_source_bytes":-1}}}"#.as_slice(),
                ),
            ] {
                let full_error = match reference_options_json(operation, options) {
                    Ok(options) => typst_artifact_contract()
                        .execute_once(
                            BindingOperationRequest::new(operation.id(), source)
                                .with_options_json(&options),
                        )
                        .expect_err("invalid request must fail through the full executor"),
                    Err(error) => error,
                };
                let payload: Value = serde_json::from_slice(&export(source, options)).unwrap();
                let expected: Value = serde_json::from_slice(&typst_binding_error_payload(
                    transport_operation,
                    &full_error,
                ))
                .unwrap();
                assert_eq!(payload, expected, "{}", operation.id());
                assert_error_envelope(
                    &payload,
                    transport_operation,
                    full_error.status().code_name(),
                );
            }
        }
    }

    #[cfg(all(feature = "svg", feature = "diagram-flowchart"))]
    #[test]
    fn typst_data_execution_preserves_svg_output_and_resource_ceiling() {
        let source = b"flowchart TD\nA[Hello] --> B[World]";
        let options_json = br#"{
            "version":4,
            "merman":{"site_config":{"theme":"dark"}},
            "svg":{"diagram_id":"typst-data-output"}
        }"#;
        let normalized = reference_options_json(OperationKey::Svg, options_json).unwrap();
        let full = typst_artifact_contract()
            .execute_once(
                BindingOperationRequest::new("svg", source).with_options_json(&normalized),
            )
            .unwrap();
        let payload: Value =
            serde_json::from_slice(&render_svg_json(source, options_json)).unwrap();
        assert_success_envelope(&payload, RENDER_OPERATION);
        let data = payload["data"]["svg"].as_str().unwrap().as_bytes();
        assert_eq!(data, full.data());
        assert_eq!(full.metadata().operation_id(), "svg");
        assert_eq!(full.metadata().byte_length(), data.len() as u64);

        let limited_options = br#"{"analysis":{"resources":{"limits":{"max_source_bytes":1}}}}"#;
        let normalized = reference_options_json(OperationKey::Svg, limited_options).unwrap();
        let full_error = typst_artifact_contract()
            .execute_once(
                BindingOperationRequest::new("svg", source).with_options_json(&normalized),
            )
            .unwrap_err();
        let payload: Value =
            serde_json::from_slice(&render_svg_json(source, limited_options)).unwrap();
        let expected: Value =
            serde_json::from_slice(&typst_binding_error_payload(RENDER_OPERATION, &full_error))
                .unwrap();
        assert_eq!(payload, expected);
        let resource = full_error
            .resource_details()
            .expect("source resource limit");
        assert_eq!(resource.limit_id, "max_source_bytes");
        assert_eq!(resource.profile, "constrained");
        assert_eq!(resource.max, 1);
    }

    #[cfg(feature = "svg")]
    #[test]
    fn typst_data_execution_preserves_duplicate_option_diagnostics() {
        for (options, code_name, field) in [
            (
                br#"{"theme":"studio","theme":"none"}"#.as_slice(),
                "MERMAN_OPTIONS_JSON_ERROR",
                "theme",
            ),
            (
                br#"{"theme":{"preset":"studio","preset":"paper"},"runtime_policy":"native"}"#,
                "MERMAN_OPTIONS_JSON_ERROR",
                "preset",
            ),
            (
                br#"{"theme":{"document":{"schema_version":2,"extends":"studio","extends":"paper"}}}"#,
                "MERMAN_INVALID_ARGUMENT",
                "extends",
            ),
            (
                br#"{"svg":{"viewbox_padding":2,"viewBoxPadding":3}}"#,
                "MERMAN_OPTIONS_JSON_ERROR",
                "viewbox_padding",
            ),
        ] {
            for (operation, export) in [
                (RENDER_OPERATION, render_svg_json as fn(&[u8], &[u8]) -> Vec<u8>),
                (ANALYZE_OPERATION, analyze_json),
            ] {
                let payload: Value = serde_json::from_slice(&export(b"", options)).unwrap();
                assert_error_envelope(&payload, operation, code_name);
                let message = payload["message"].as_str().unwrap();
                assert!(message.contains("duplicate") && message.contains(field), "{payload}");
            }
        }
    }

    #[test]
    fn typst_data_execution_preserves_options_json_byte_ceiling() {
        let max = merman_bindings_core::BINDING_OPTIONS_JSON_MAX_BYTES;
        let oversized = vec![0xff; max + 1];
        let payload: Value = serde_json::from_slice(&analyze_json(b"", &oversized)).unwrap();
        assert_error_envelope(
            &payload,
            ANALYZE_OPERATION,
            "MERMAN_RESOURCE_LIMIT_EXCEEDED",
        );
        let details = &payload["details"]["resource"];
        assert_eq!(details["limit_id"], "max_options_json_bytes");
        assert_eq!(details["actual"], max + 1);
        assert_eq!(details["max"], max);

        // The first input only exceeds the ceiling after the resource profile is inserted.
        // The second one only exceeds it after the transport policy is applied.
        let resource_options_len =
            br#"{"site_config":{"padding":""},"resources":{"profile":"constrained"}}"#.len();
        let transport_policy_len = br#","runtime_policy":"deterministic""#.len()
            + if cfg!(feature = "svg") {
                br#","environment":{"math_renderer":"none"}"#.len()
            } else {
                0
            };
        for empty_options_len in [
            resource_options_len,
            resource_options_len + transport_policy_len,
        ] {
            let padding = "x".repeat(max + 1 - empty_options_len);
            let options = format!(r#"{{"site_config":{{"padding":"{padding}"}}}}"#);
            assert!(options.len() <= max);
            let payload: Value =
                serde_json::from_slice(&analyze_json(b"", options.as_bytes())).unwrap();
            assert_error_envelope(
                &payload,
                ANALYZE_OPERATION,
                "MERMAN_RESOURCE_LIMIT_EXCEEDED",
            );
            assert_eq!(payload["details"]["resource"], *details);
        }
    }

    #[cfg(feature = "svg")]
    #[test]
    fn theme_operation_json_exports_and_resolves_a_v2_theme() {
        let payload: Value = serde_json::from_slice(&theme_operation_json(
            b"export-theme-preset",
            b"studio",
            b"",
        ))
        .unwrap();
        assert_success_envelope(&payload, "export-theme-preset");
        let document = &payload["data"]["result"];
        assert_eq!(document["schema_version"], 2);
        assert_eq!(document["id"], "studio");

        let request =
            json!({"document": document, "appearance": "dark", "auto_fix_contrast": true});
        let payload: Value = serde_json::from_slice(&theme_operation_json(
            b"resolve-theme",
            &serde_json::to_vec(&request).unwrap(),
            b"",
        ))
        .unwrap();
        assert_success_envelope(&payload, "resolve-theme");
        let result = &payload["data"]["result"];
        assert_eq!(result["ok"], true);
        assert_eq!(result["resolved"]["appearance"], "dark");
        assert!(result["notices"].is_array());
    }

    #[cfg(feature = "svg")]
    #[test]
    fn theme_operation_json_preserves_v2_document_diagnostics() {
        let payload: Value = serde_json::from_slice(&theme_operation_json(
            b"resolve-theme",
            br#"{"document":{"schema_version":2,"extends":"studio","typography":{"base_size_px":"large"}}}"#,
            b"",
        ))
        .unwrap();
        assert_success_envelope(&payload, "resolve-theme");
        let result = &payload["data"]["result"];
        assert_eq!(result["ok"], false);
        assert_eq!(result["path"], "typography.base_size_px");
        assert!(result["line"].is_number());
        assert!(result["column"].is_number());
    }

    #[cfg(feature = "svg")]
    #[test]
    fn theme_operation_json_rejects_non_theme_operations() {
        let payload: Value =
            serde_json::from_slice(&theme_operation_json(b"svg", b"flowchart TD\nA --> B", b""))
                .expect("valid theme-operation error envelope");

        assert_error_envelope(&payload, "svg", "MERMAN_UNSUPPORTED_OPERATION");
        assert!(
            payload["message"]
                .as_str()
                .is_some_and(|message| message.contains("theme authoring"))
        );
    }

    #[cfg(feature = "svg")]
    #[test]
    fn theme_operation_json_bounds_untrusted_operation_ids_before_echoing_them() {
        let oversized = vec![b'x'; TYPST_OPERATION_ID_MAX_UTF8_BYTES + 1];
        let payload: Value =
            serde_json::from_slice(&theme_operation_json(&oversized, br#"{}"#, b""))
                .expect("valid bounded operation-id error envelope");

        assert_error_envelope(&payload, THEME_OPERATION, "MERMAN_INVALID_ARGUMENT");
        assert!(!payload.to_string().contains(&"x".repeat(32)));

        let payload: Value = serde_json::from_slice(&theme_operation_json(
            b"resolve-theme\nforged",
            br#"{}"#,
            b"",
        ))
        .expect("valid control-character error envelope");

        assert_error_envelope(&payload, THEME_OPERATION, "MERMAN_INVALID_ARGUMENT");
        assert!(!payload.to_string().contains("forged"));

        let unknown = b"future-theme-operation-identifier";
        let payload: Value = serde_json::from_slice(&theme_operation_json(unknown, br#"{}"#, b""))
            .expect("valid unknown-operation error envelope");

        assert_error_envelope(&payload, THEME_OPERATION, "MERMAN_UNSUPPORTED_OPERATION");
        assert!(
            !payload
                .to_string()
                .contains("future-theme-operation-identifier")
        );
    }

    #[test]
    fn typst_transport_aligns_default_layout_work_with_interactive() {
        let expected = if cfg!(feature = "svg") {
            json!({ "profile": "constrained", "limits": { "max_layout_work_units": 14_100_000 } })
        } else {
            json!({ "profile": "constrained" })
        };
        for input in [
            b"".as_slice(),
            br#"{}"#,
            br#"{"resources":{}}"#,
            br#"{"resources":{"profile":null}}"#,
        ] {
            let options =
                reference_options_json(OperationKey::Svg, input).expect("default Typst options");
            let payload: Value = serde_json::from_slice(&options).expect("valid options JSON");
            assert_eq!(payload["runtime_policy"], "deterministic");
            assert_eq!(payload["resources"], expected);
            if cfg!(feature = "svg") {
                assert_eq!(payload["environment"]["math_renderer"], "none");
            } else {
                assert!(payload.get("environment").is_none());
            }
        }
    }

    #[test]
    fn typst_explicit_constrained_profile_keeps_its_stricter_defaults() {
        let options = reference_options_json(
            OperationKey::Svg,
            br#"{"resources":{"profile":"constrained"}}"#,
        )
        .expect("explicit constrained profile");
        let payload: Value = serde_json::from_slice(&options).expect("valid options JSON");
        assert_eq!(payload["resources"], json!({ "profile": "constrained" }));
    }

    #[test]
    fn typst_transport_treats_null_target_policy_fields_as_unspecified() {
        let has_svg = typst_artifact_contract()
            .runtime_capabilities()
            .has_operation("svg");
        let input = if has_svg {
            br#"{"runtime_policy":null,"environment":null}"#.as_slice()
        } else {
            br#"{"runtime_policy":null}"#.as_slice()
        };
        let options =
            reference_options_json(OperationKey::Svg, input).expect("null transport policy fields");
        let payload: Value = serde_json::from_slice(&options).expect("valid options JSON");

        assert_eq!(payload["runtime_policy"], "deterministic");
        if has_svg {
            assert_eq!(payload["environment"]["math_renderer"], "none");
        } else {
            assert!(payload.get("environment").is_none());
        }
    }

    #[test]
    fn typst_resource_policy_preserves_stricter_caller_limits() {
        for wrapper in [None, Some("analysis"), Some("merman")] {
            let analysis = json!({ "resources": { "limits": {
                "max_source_bytes": 4096,
                "max_model_items": 1
            } } });
            let input = wrapper
                .map(|wrapper| json!({ wrapper: analysis.clone() }))
                .unwrap_or(analysis);
            let options =
                reference_options_json(OperationKey::Svg, &serde_json::to_vec(&input).unwrap())
                    .expect("valid options");
            let payload: Value = serde_json::from_slice(&options).expect("valid options JSON");
            let resources = wrapper
                .map(|wrapper| &payload[wrapper]["resources"])
                .unwrap_or(&payload["resources"]);

            assert_eq!(resources["profile"], "constrained");
            assert_eq!(resources["limits"]["max_source_bytes"], Value::from(4096));
            assert_eq!(resources["limits"]["max_model_items"], Value::from(1));
            assert_eq!(payload["runtime_policy"], "deterministic");
            assert_eq!(payload.get("resources").is_some(), wrapper.is_none());
        }
    }

    #[cfg(feature = "svg")]
    #[test]
    fn typst_resource_policy_accepts_explicit_layout_work_limits() {
        for wrapper in [None, Some("analysis"), Some("merman")] {
            for limit in [1, 125_000, 1_000_000, 14_100_000, 20_000_000] {
                let analysis = json!({ "resources": {
                    "profile": null,
                    "limits": { "max_layout_work_units": limit }
                } });
                let input = wrapper
                    .map(|wrapper| json!({ wrapper: analysis.clone() }))
                    .unwrap_or(analysis);
                let options =
                    reference_options_json(OperationKey::Svg, &serde_json::to_vec(&input).unwrap())
                        .expect("explicit finite layout allowance");
                let payload: Value = serde_json::from_slice(&options).unwrap();
                let resources = wrapper
                    .map(|wrapper| &payload[wrapper]["resources"])
                    .unwrap_or(&payload["resources"]);

                assert_eq!(resources["profile"], "constrained");
                assert_eq!(
                    resources["limits"],
                    json!({ "max_layout_work_units": limit })
                );
                assert_eq!(payload.get("resources").is_some(), wrapper.is_none());
                assert_eq!(payload["runtime_policy"], "deterministic");
                assert_eq!(payload["environment"]["math_renderer"], "none");
            }
        }
    }

    #[cfg(feature = "svg")]
    #[test]
    fn typst_resource_policy_rejects_invalid_layout_work_limits() {
        for value in [
            "0",
            "-1",
            "1.5",
            "null",
            "true",
            r#""250000""#,
            "18446744073709551616",
            "1e100",
        ] {
            let input =
                format!(r#"{{"resources":{{"limits":{{"max_layout_work_units":{value}}}}}}}"#);
            let error = reference_options_json(OperationKey::Svg, input.as_bytes()).unwrap_err();
            let expected = if value == "0" {
                merman_bindings_core::BindingStatus::InvalidArgument
            } else {
                merman_bindings_core::BindingStatus::OptionsJsonError
            };
            assert_eq!(error.status(), expected, "value={value}: {error:?}");
        }

        for input in [
            br#"{"resources":null}"#.as_slice(),
            br#"{"resources":{"limits":null}}"#.as_slice(),
            br#"{"resources":{"limits":[]}}"#.as_slice(),
        ] {
            let error = reference_options_json(OperationKey::Svg, input).unwrap_err();
            assert_eq!(
                error.status(),
                merman_bindings_core::BindingStatus::OptionsJsonError
            );
        }
    }

    #[cfg(feature = "svg")]
    #[test]
    fn typst_layout_allowance_does_not_loosen_other_limits_or_profiles() {
        for mut resources in [
            json!({ "profile": "interactive" }),
            json!({ "profile": "trusted-native" }),
            json!({ "profile": "unbounded-for-trusted-input" }),
            json!({ "limits": { "max_source_bytes": 1048577 } }),
            json!({ "limits": { "max_model_items": 1000000000 } }),
            json!({ "limits": { "max_svg_bytes": 12582913 } }),
        ] {
            let limits = resources
                .as_object_mut()
                .unwrap()
                .entry("limits")
                .or_insert_with(|| json!({}));
            limits["max_layout_work_units"] = json!(1_000_000);
            let input = serde_json::to_vec(&json!({ "resources": resources })).unwrap();
            let error = reference_options_json(OperationKey::Svg, &input).unwrap_err();
            assert_eq!(
                error.status(),
                merman_bindings_core::BindingStatus::OptionsJsonError
            );
            assert!(
                error.message().contains("loosen the transport ceiling"),
                "{error:?}"
            );
        }
    }

    #[test]
    fn typst_resource_policy_preserves_analysis_wrapper_shape() {
        for wrapper in ["analysis", "merman"] {
            let input = format!(r#"{{ "{wrapper}": {{ "site_config": {{ "theme": "dark" }} }} }}"#);
            let options = reference_options_json(OperationKey::Svg, input.as_bytes())
                .expect("valid wrapped options");
            let payload: Value = serde_json::from_slice(&options).expect("valid options JSON");

            let expected = if cfg!(feature = "svg") {
                json!({ "profile": "constrained", "limits": { "max_layout_work_units": 14_100_000 } })
            } else {
                json!({ "profile": "constrained" })
            };
            assert_eq!(payload[wrapper]["resources"], expected);
            assert_eq!(payload["runtime_policy"], "deterministic");
            assert!(payload[wrapper].get("runtime_policy").is_none());
            assert!(payload[wrapper].get("environment").is_none());
            if typst_artifact_contract()
                .runtime_capabilities()
                .has_operation("svg")
            {
                assert_eq!(payload["environment"]["math_renderer"], "none");
            }
            assert!(payload.get("resources").is_none());
        }
    }

    #[test]
    fn typst_target_policy_rejects_native_runtime_and_ratex_math() {
        for (operation, export) in [
            (
                RENDER_OPERATION,
                render_svg_json as fn(&[u8], &[u8]) -> Vec<u8>,
            ),
            (ANALYZE_OPERATION, analyze_json),
        ] {
            let native: Value =
                serde_json::from_slice(&export(b"", br#"{"runtime_policy":"native"}"#)).unwrap();
            assert_error_envelope(&native, operation, "MERMAN_UNSUPPORTED_OPERATION");
            assert_eq!(native["kind"], "missing-capability");
            assert_eq!(native["capability_id"], "system-clock");

            if cfg!(feature = "svg") {
                let ratex: Value = serde_json::from_slice(&export(
                    b"",
                    br#"{"environment":{"math_renderer":" RaTeX "}}"#,
                ))
                .unwrap();
                assert_error_envelope(&ratex, operation, "MERMAN_UNSUPPORTED_OPERATION");
                assert_eq!(ratex["kind"], "missing-capability");
                assert_eq!(ratex["capability_id"], "math");

                let null_math: Value = serde_json::from_slice(&export(
                    b"",
                    br#"{"environment":{"math_renderer":null}}"#,
                ))
                .unwrap();
                assert_error_envelope(&null_math, operation, "MERMAN_OPTIONS_JSON_ERROR");
                assert_eq!(
                    null_math["message"],
                    "invalid options_json: `environment.math_renderer` must be a string"
                );
            }
        }
    }

    #[test]
    fn typst_resource_policy_rejects_looser_profiles_and_limits() {
        for input in [
            br#"{"resources":{"profile":"trusted-native"}}"#.as_slice(),
            br#"{"analysis":{"resources":{"limits":{"max_source_bytes":1048577}}}}"#.as_slice(),
            br#"{"merman":{"resources":{"limits":{"max_model_items":1000000000}}}}"#.as_slice(),
        ] {
            let error = reference_options_json(OperationKey::Svg, input).unwrap_err();
            assert_eq!(
                error.status(),
                merman_bindings_core::BindingStatus::OptionsJsonError
            );
            assert!(error.message().contains("loosen the transport ceiling"));
        }
    }

    #[test]
    fn null_resource_profile_cannot_bypass_the_typst_limits() {
        let options = reference_options_json(
            OperationKey::Svg,
            br#"{"resources":{"profile":null,"limits":{"max_source_bytes":4096}}}"#,
        )
        .expect("valid options");
        let payload: Value = serde_json::from_slice(&options).expect("valid options JSON");

        assert_eq!(payload["resources"]["profile"], "constrained");
        assert_eq!(
            payload["resources"]["limits"]["max_source_bytes"],
            Value::from(4096)
        );
    }

    #[test]
    fn malformed_wrappers_fail_closed_before_resource_policy_selection() {
        for options in [
            br#"{"merman":null,"resources":{"profile":"trusted-native"}}"#.as_slice(),
            br#"{"analysis":[]}"#.as_slice(),
        ] {
            let error = reference_options_json(OperationKey::Svg, options).unwrap_err();
            assert_eq!(
                error.status(),
                merman_bindings_core::BindingStatus::OptionsJsonError
            );
            assert!(error.message().contains("wrapper must be an object"));
        }

        let error = reference_options_json(OperationKey::Svg, br#"{"analysis":{},"merman":{}}"#)
            .unwrap_err();
        assert_eq!(
            error.status(),
            merman_bindings_core::BindingStatus::OptionsJsonError
        );
        assert!(error.message().contains("must not contain both"));

        for wrapper in ["analysis", "merman"] {
            let input = json!({
                "resources": { "limits": { "max_layout_work_units": 1_000_000 } },
                wrapper: {}
            });
            let error =
                reference_options_json(OperationKey::Svg, &serde_json::to_vec(&input).unwrap())
                    .unwrap_err();
            assert_eq!(
                error.status(),
                merman_bindings_core::BindingStatus::OptionsJsonError
            );
            assert!(error.message().contains("must not mix top-level resources"));
        }
    }

    #[cfg(feature = "svg")]
    #[test]
    fn render_rejects_malformed_wrappers_with_a_structured_error() {
        let payload: Value = serde_json::from_slice(&render_svg_json(
            b"flowchart TD\nA --> B",
            br#"{"merman":null,"resources":{"profile":"trusted-native"}}"#,
        ))
        .expect("valid JSON payload");

        assert_error_envelope(&payload, RENDER_OPERATION, "MERMAN_OPTIONS_JSON_ERROR");
        assert_eq!(payload["kind"], "generic");
    }

    #[cfg(all(feature = "svg", feature = "diagram-flowchart"))]
    #[test]
    fn render_svg_json_preserves_original_utf8_parse_diagnostics() {
        let source = "\u{feff}---\ntitle: 中文\n---\n%% 注释\nflowchart TD\nA --> ]";
        let payload: Value = serde_json::from_slice(&render_svg_json(source.as_bytes(), b""))
            .expect("valid parse-error envelope");

        assert_error_envelope(&payload, RENDER_OPERATION, "MERMAN_PARSE_ERROR");
        assert_eq!(payload["code"], 5);
        assert_eq!(payload["kind"], "generic");
        assert!(payload["capability_id"].is_null());
        assert_eq!(
            payload["details"],
            json!({
                "diagnostic": {
                    "code": "merman.parse.diagram_parse",
                    "span": { "start": 54, "end": 55, "kind": "exact" },
                    "field": null,
                    "diagram_type": "flowchart-v2",
                    "requested_max_width": null,
                    "actual_width": null,
                    "width_profile": null,
                    "fallback_reason": null,
                }
            }),
        );
    }

    #[test]
    fn typst_error_details_preserve_wide_integer_resource_counts() {
        let max = merman_bindings_core::RUNTIME_CATALOG_MAX_SAFE_INTEGER + 1;
        let error = merman_bindings_core::BindingError::resource_limit_with_cause(
            merman_bindings_core::BindingResourceLimitCause::ArithmeticOverflow,
            "layout_model",
            "max_layout_work_units",
            u64::MAX,
            max,
            "constrained",
            "layout work accounting overflowed",
        );
        let payload: Value =
            serde_json::from_slice(&typst_binding_error_payload(RENDER_OPERATION, &error))
                .expect("valid resource error envelope");

        assert_error_envelope(&payload, RENDER_OPERATION, "MERMAN_RESOURCE_LIMIT_EXCEEDED");
        assert_eq!(
            payload["details"],
            json!({
                "resource": {
                    "cause": "arithmetic_overflow",
                    "limit_id": "max_layout_work_units",
                    "phase": "layout_model",
                    "actual": u64::MAX,
                    "max": max,
                    "profile": "constrained",
                }
            }),
        );
        assert_eq!(
            payload["details"]["resource"]["actual"].as_u64(),
            Some(u64::MAX)
        );
        assert_eq!(payload["details"]["resource"]["max"].as_u64(), Some(max));
    }

    #[test]
    fn typst_error_details_preserve_cancellation_and_omit_absent_details() {
        let control = merman_bindings_core::OperationControl::new();
        control.cancel();
        let cancelled = control
            .checkpoint_at(merman_bindings_core::OperationPhase::Layout)
            .expect_err("cancelled operation");
        let error = merman_bindings_core::BindingError::cancelled(cancelled);
        let payload: Value =
            serde_json::from_slice(&typst_binding_error_payload(RENDER_OPERATION, &error))
                .expect("valid cancellation envelope");

        assert_error_envelope(&payload, RENDER_OPERATION, "MERMAN_CANCELLED");
        assert_eq!(
            payload["details"],
            json!({ "cancellation": { "reason": "requested", "phase": "layout" } }),
        );

        let plain = merman_bindings_core::BindingError::invalid_argument("invalid request");
        let payload: Value =
            serde_json::from_slice(&typst_binding_error_payload(RENDER_OPERATION, &plain))
                .expect("valid plain error envelope");
        assert_error_envelope(&payload, RENDER_OPERATION, "MERMAN_INVALID_ARGUMENT");
        assert!(payload.get("details").is_none());
    }

    #[cfg(feature = "svg")]
    #[test]
    fn typst_error_details_preserve_icon_registry_context() {
        let error = merman_bindings_core::BindingError::icon_registry_invalid_utf8(
            3,
            "icon pack is not UTF-8",
        );
        let payload: Value =
            serde_json::from_slice(&typst_binding_error_payload(RENDER_OPERATION, &error))
                .expect("valid icon-registry error envelope");

        assert_error_envelope(&payload, RENDER_OPERATION, "MERMAN_UTF8_ERROR");
        assert_eq!(
            payload["details"],
            json!({
                "icon_registry": {
                    "kind_id": "invalid_utf8",
                    "pack_index": 3,
                    "registration_name": null,
                }
            }),
        );
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn analysis_rejects_malformed_wrappers_with_a_structured_error() {
        let payload: Value = serde_json::from_slice(&analyze_json(
            b"flowchart TD\nA --> B",
            br#"{"analysis":[]}"#,
        ))
        .expect("valid JSON payload");

        assert_error_envelope(&payload, ANALYZE_OPERATION, "MERMAN_OPTIONS_JSON_ERROR");
        assert_eq!(payload["kind"], "generic");
    }

    #[cfg(feature = "svg")]
    #[test]
    fn render_svg_json_returns_the_shared_success_envelope() {
        let payload: Value = serde_json::from_slice(&render_svg_json(
            b"flowchart TD\nA[Hello] --> B[World]",
            b"",
        ))
        .expect("valid JSON payload");

        assert_success_envelope(&payload, RENDER_OPERATION);
        assert!(payload["data"]["svg"].as_str().unwrap().contains("<svg"));
        assert!(payload["data"]["svg"].as_str().unwrap().contains("Hello"));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn render_svg_json_keeps_issue_89_typography_in_the_resvg_safe_transport() {
        let payload: Value = serde_json::from_slice(&render_svg_json(
            br#"classDiagram
    class User {
        +String name
    }"#,
            br#"{"svg":{"pipeline":"resvg-safe"}}"#,
        ))
        .expect("valid JSON payload");

        assert_success_envelope(&payload, RENDER_OPERATION);
        let svg = payload["data"]["svg"].as_str().expect("SVG payload");
        assert!(!svg.contains("<foreignObject"), "{svg}");
        assert!(svg.contains("User"), "{svg}");
        let user_text_tag = svg
            .split_once(">User</text>")
            .expect("class User label")
            .0
            .rsplit_once("<text")
            .expect("class User text element")
            .1;
        assert!(
            user_text_tag.contains("font-size:14px") || user_text_tag.contains("font-size: 14px"),
            "Mermaid 12 redux-color typography must survive on the class label: {user_text_tag}"
        );
    }

    #[cfg(feature = "svg")]
    #[test]
    fn render_svg_json_keeps_v2_typography_in_the_resvg_safe_transport() {
        let payload: Value = serde_json::from_slice(&render_svg_json(
            b"flowchart TD\nA[Hello] --> B[World]",
            br#"{
                "theme": {
                    "schema_version": 2,
                    "extends": "studio",
                    "typography": {"sans": ["Typst Explicit Sans", "sans-serif"], "base_size_px": 18}
                },
                "svg": { "pipeline": "resvg-safe" }
            }"#,
        ))
        .expect("valid JSON payload");

        assert_success_envelope(&payload, RENDER_OPERATION);
        let svg = payload["data"]["svg"].as_str().expect("SVG payload");
        assert!(!svg.contains("<foreignObject"), "{svg}");
        let label = svg
            .split_once(">Hello</text>")
            .expect("flowchart Hello label")
            .0
            .rsplit_once("<text")
            .expect("flowchart Hello text element")
            .1;
        assert!(label.contains("Typst Explicit Sans"), "{label}");
        assert!(
            label.contains("font-size:18px") || label.contains("font-size: 18px"),
            "{label}"
        );
    }

    #[cfg(all(feature = "svg", feature = "diagram-flowchart"))]
    #[test]
    fn render_svg_json_dagre_follows_the_typst_feature() {
        let source = b"---\nconfig:\n  layout: dagre\n---\nflowchart TD\nA --> B";
        let payload: Value = serde_json::from_slice(&render_svg_json(source, b"")).unwrap();
        if cfg!(feature = "layout-dagre") {
            assert_success_envelope(&payload, RENDER_OPERATION);
            assert!(payload["data"]["svg"].as_str().unwrap().contains("<svg"));
        } else {
            assert_error_envelope(&payload, RENDER_OPERATION, "MERMAN_UNSUPPORTED_OPERATION");
            assert_eq!(payload["kind"], "missing-capability");
            assert_eq!(payload["capability_id"], "layout-dagre");
        }
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn render_svg_json_renders_flowchart_elk_from_default_artifact() {
        for source in [
            b"flowchart-elk TD\nA[Hello] --> B[World]".as_slice(),
            b"---\nconfig:\n  layout: elk\n---\nflowchart TD\nA[Hello] --> B[World]".as_slice(),
        ] {
            let payload: Value =
                serde_json::from_slice(&render_svg_json(source, b"")).expect("valid JSON payload");

            assert_success_envelope(&payload, RENDER_OPERATION);
            let svg = payload["data"]["svg"].as_str().unwrap();
            assert!(svg.contains("Hello"));
            let options = reference_options_json(OperationKey::Svg, b"").unwrap();
            let full = typst_artifact_contract()
                .execute_once(
                    BindingOperationRequest::new("svg", source).with_options_json(&options),
                )
                .unwrap();
            assert_eq!(svg.as_bytes(), full.data());
        }
    }

    #[cfg(feature = "layout-cytoscape")]
    #[test]
    fn complete_typst_build_renders_architecture() {
        let payload: Value = serde_json::from_slice(&render_svg_json(
            b"architecture-beta\n  service api(server)[API service]\n",
            b"",
        ))
        .expect("valid JSON payload");

        assert_success_envelope(&payload, RENDER_OPERATION);
        assert!(payload["data"]["svg"].as_str().unwrap().contains("<svg"));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn render_svg_json_uses_typst_resource_profile_by_default() {
        let source = format!("flowchart TD\nA[{}]", "x".repeat(1024 * 1024));
        let payload: Value = serde_json::from_slice(&render_svg_json(source.as_bytes(), b""))
            .expect("valid JSON payload");

        assert_error_envelope(&payload, RENDER_OPERATION, "MERMAN_RESOURCE_LIMIT_EXCEEDED");
        assert!(
            payload["message"]
                .as_str()
                .unwrap()
                .contains("max_source_bytes")
        );
        assert_eq!(
            payload["details"]["resource"]["limit_id"],
            "max_source_bytes"
        );
        assert_eq!(payload["details"]["resource"]["profile"], "constrained");
    }

    #[cfg(all(feature = "diagram-mindmap", feature = "layout-cytoscape"))]
    #[test]
    fn render_svg_json_mindmap_uses_default_and_explicit_layout_work_allowances() {
        use std::fmt::Write as _;

        let mut source = String::from("mindmap\n  root[Root]\n");
        for branch in 0..8 {
            writeln!(source, "    branch{branch}[Branch {branch}]").unwrap();
            for leaf in 0..8 {
                writeln!(source, "      leaf{branch}_{leaf}[Leaf {leaf}]").unwrap();
            }
        }
        let options = json!({
            "svg": { "diagram_id": "typst-mindmap-layout-work" }
        });
        let default: Value = serde_json::from_slice(&render_svg_json(
            source.as_bytes(),
            &serde_json::to_vec(&options).unwrap(),
        ))
        .expect("default-budget result");
        assert_success_envelope(&default, RENDER_OPERATION);

        let mut limited_options = options.clone();
        limited_options["resources"] = json!({ "limits": { "max_layout_work_units": 125_000 } });
        let limited: Value = serde_json::from_slice(&render_svg_json(
            source.as_bytes(),
            &serde_json::to_vec(&limited_options).unwrap(),
        ))
        .expect("stricter-budget result");
        assert_error_envelope(&limited, RENDER_OPERATION, "MERMAN_RESOURCE_LIMIT_EXCEEDED");
        let resource = &limited["details"]["resource"];
        assert_eq!(resource["limit_id"], "max_layout_work_units", "{limited}");
        assert_eq!(resource["max"], 125_000);
        assert_eq!(resource["cause"], "ceiling");
        assert_eq!(resource["profile"], "constrained");

        let options = serde_json::to_vec(&json!({
            "runtime_policy": "deterministic",
            "environment": { "math_renderer": "none" },
            "resources": {
                "profile": "constrained",
                "limits": { "max_layout_work_units": 20_000_000 }
            },
            "svg": { "diagram_id": "typst-mindmap-layout-work" }
        }))
        .unwrap();
        let raised: Value = serde_json::from_slice(&render_svg_json(source.as_bytes(), &options))
            .expect("explicit-budget result");
        assert_eq!(raised["ok"], true, "{raised}");
        assert_success_envelope(&raised, RENDER_OPERATION);
        assert_eq!(default["data"]["svg"], raised["data"]["svg"]);
        let reference = typst_artifact_contract()
            .execute_once_data(
                BindingOperationRequest::new("svg", source.as_bytes()).with_options_json(&options),
            )
            .expect("same finite policy through the shared engine");
        assert_eq!(
            raised["data"]["svg"].as_str().unwrap().as_bytes(),
            reference
        );
    }

    #[cfg(feature = "svg")]
    #[test]
    fn render_svg_json_returns_a_structured_error_envelope() {
        let payload: Value =
            serde_json::from_slice(&render_svg_json(b"", b"")).expect("valid JSON payload");

        assert_error_envelope(&payload, RENDER_OPERATION, "MERMAN_NO_DIAGRAM");
        assert!(!payload["message"].as_str().unwrap().is_empty());
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn analyze_json_returns_the_shared_success_envelope() {
        for (options, valid) in [
            (b"".as_slice(), true),
            (
                br#"{"analysis":{"resources":{"limits":{"max_source_bytes":1}}}}"#.as_slice(),
                false,
            ),
        ] {
            let payload: Value =
                serde_json::from_slice(&analyze_json(b"flowchart TD\nA --> B", options))
                    .expect("valid JSON payload");

            assert_success_envelope(&payload, ANALYZE_OPERATION);
            let analysis = &payload["data"]["analysis"];
            assert_eq!(
                analysis["version"],
                merman_bindings_core::ANALYSIS_PAYLOAD_VERSION
            );
            assert_eq!(analysis["valid"], valid);
            assert!(analysis["diagnostics"].as_array().is_some());

            let options = reference_options_json(OperationKey::AnalysisJson, options).unwrap();
            let full = typst_artifact_contract()
                .execute_once(
                    BindingOperationRequest::new("analysis-json", b"flowchart TD\nA --> B")
                        .with_options_json(&options),
                )
                .unwrap();
            let expected: Value = serde_json::from_slice(full.data()).unwrap();
            assert_eq!(analysis, &expected);
        }
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn analyze_json_returns_a_structured_error_envelope_for_option_failures() {
        for (options, code_name, message) in [
            (
                b"{".as_slice(),
                "MERMAN_OPTIONS_JSON_ERROR",
                "invalid options_json",
            ),
            (
                br#"{"resources":{"limits":{"max_model_items":1}}}"#,
                "MERMAN_INVALID_ARGUMENT",
                "not available for the single-diagram analysis operation",
            ),
        ] {
            let payload: Value =
                serde_json::from_slice(&analyze_json(b"flowchart TD\nA --> B", options)).unwrap();
            assert_error_envelope(&payload, ANALYZE_OPERATION, code_name);
            assert!(
                payload["message"].as_str().unwrap().contains(message),
                "{payload}"
            );
        }
    }

    #[cfg(all(feature = "analysis", feature = "svg"))]
    #[test]
    fn analyze_json_checks_transport_policy_before_operation_resource_scope() {
        for (options, capability) in [
            (
                br#"{"runtime_policy":"native","resources":{"limits":{"max_model_items":1}}}"#
                    .as_slice(),
                "system-clock",
            ),
            (
                br#"{"environment":{"math_renderer":"ratex"},"resources":{"limits":{"max_model_items":1}}}"#,
                "math",
            ),
        ] {
            let payload: Value = serde_json::from_slice(&analyze_json(b"", options)).unwrap();
            assert_error_envelope(&payload, ANALYZE_OPERATION, "MERMAN_UNSUPPORTED_OPERATION");
            assert_eq!(payload["capability_id"], capability);
        }
    }

    fn assert_success_envelope(payload: &Value, operation: &str) {
        assert_eq!(payload["version"], TYPST_RESULT_PAYLOAD_SCHEMA_VERSION);
        assert_eq!(payload["operation"], operation);
        assert_eq!(payload["ok"], true);
        assert_eq!(payload["code"], 0);
        assert_eq!(payload["code_name"], "MERMAN_OK");
        assert!(payload["kind"].is_null());
        assert!(payload["capability_id"].is_null());
        assert!(payload["message"].is_null());
        assert!(payload["data"].is_object());
    }

    fn assert_error_envelope(payload: &Value, operation: &str, code_name: &str) {
        assert_eq!(payload["version"], TYPST_RESULT_PAYLOAD_SCHEMA_VERSION);
        assert_eq!(payload["operation"], operation);
        assert_eq!(payload["ok"], false);
        assert_eq!(payload["code_name"], code_name);
        assert!(payload["kind"].is_string());
        assert!(payload["message"].is_string());
        assert!(payload["data"].is_null());
    }
}

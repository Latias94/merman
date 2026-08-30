//! Typst WebAssembly plugin bridge for `merman`.
//!
//! This crate exposes one versioned Typst transport contract. The package wrapper may present
//! convenient render, analysis, and theme-authoring helpers, but every plugin operation returns the
//! same closed transport envelope so hosts can handle failures without depending on trap behavior.

use merman_bindings_core::{
    ArtifactContractSpec, BindingOperationRequest, BindingTransportKey, CapabilityKey,
    OperationKey, RuntimePolicyExposure, TargetKey, ValidatedArtifactContract,
};
use serde_json::{json, Value};

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
    OperationKey::DescribeThemeSupportJson,
    OperationKey::ExportThemePresetJson,
    OperationKey::MaterializeThemeJson,
    OperationKey::Svg,
];
const RENDER_OPERATION: &str = "render-svg";
const ANALYZE_OPERATION: &str = "analyze";
const THEME_OPERATION: &str = "theme-operation";
const TYPST_OPERATION_ID_MAX_UTF8_BYTES: usize = 128;
const THEME_OPERATION_KEYS: &[OperationKey] = &[
    OperationKey::DescribeThemeSupportJson,
    OperationKey::ExportThemePresetJson,
    OperationKey::MaterializeThemeJson,
];
const TYPST_OPERATIONS: &[OperationKey] = &[
    #[cfg(feature = "analysis")]
    OperationKey::AnalysisJson,
    #[cfg(feature = "svg")]
    OperationKey::DescribeThemeSupportJson,
    #[cfg(feature = "svg")]
    OperationKey::ExportThemePresetJson,
    #[cfg(feature = "svg")]
    OperationKey::MaterializeThemeJson,
    #[cfg(feature = "svg")]
    OperationKey::Svg,
];
const TYPST_SUPPLEMENTAL_CAPABILITIES: &[CapabilityKey] = &[
    #[cfg(feature = "layout-cytoscape")]
    CapabilityKey::LayoutCytoscape,
    #[cfg(feature = "layout-elk")]
    CapabilityKey::LayoutElk,
];
static ARTIFACT_CONTRACT: ValidatedArtifactContract =
    ArtifactContractSpec::new(TargetKey::Typst, BindingTransportKey::Typst)
        .with_operations(TYPST_OPERATIONS)
        .with_supplemental_capabilities(TYPST_SUPPLEMENTAL_CAPABILITIES)
        .with_runtime_policy_exposure(RuntimePolicyExposure::DeterministicOnly)
        .materialize();

#[cfg(target_arch = "wasm32")]
wasm_minimal_protocol::initiate_protocol!();

include!("generated/typst_plugin_abi.rs");

#[cfg(all(
    any(feature = "layout-cytoscape", feature = "layout-elk"),
    not(feature = "svg")
))]
compile_error!(
    "Typst layout features must enable the crate `svg` feature so constrained resources cannot be bypassed"
);

#[cfg_attr(target_arch = "wasm32", wasm_minimal_protocol::wasm_func)]
pub fn abi_version() -> &'static [u8] {
    TYPST_PLUGIN_ABI_VERSION_BYTES
}

#[cfg_attr(target_arch = "wasm32", wasm_minimal_protocol::wasm_func)]
pub fn package_version() -> Vec<u8> {
    env!("CARGO_PKG_VERSION").as_bytes().to_vec()
}

#[cfg_attr(target_arch = "wasm32", wasm_minimal_protocol::wasm_func)]
pub fn capabilities_json() -> Vec<u8> {
    typst_capabilities_json()
}

fn typst_artifact_contract() -> &'static ValidatedArtifactContract {
    &ARTIFACT_CONTRACT
}

fn typst_capabilities_json() -> Vec<u8> {
    let catalog = typst_artifact_contract().runtime_catalog(TYPST_PLUGIN_ABI_VERSION);
    serde_json::to_vec(&catalog).expect("the checked Typst capability catalog is serializable")
}

#[cfg_attr(target_arch = "wasm32", wasm_minimal_protocol::wasm_func)]
pub fn render_svg_json(source: &[u8], options_json: &[u8]) -> Vec<u8> {
    let options_json = match typst_options_json(options_json) {
        Ok(options_json) => options_json,
        Err(error) => return typst_binding_error_payload(RENDER_OPERATION, &error),
    };
    match execute_typst_operation("svg", source, &options_json) {
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

#[cfg_attr(target_arch = "wasm32", wasm_minimal_protocol::wasm_func)]
pub fn analyze_json(source: &[u8], options_json: &[u8]) -> Vec<u8> {
    let options_json = match typst_options_json(options_json) {
        Ok(options_json) => options_json,
        Err(error) => return typst_binding_error_payload(ANALYZE_OPERATION, &error),
    };
    match execute_typst_operation("analysis-json", source, &options_json) {
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
/// The operation ID is restricted to the three theme-authoring rows advertised by the Typst
/// artifact. Rendering and analysis keep their dedicated exports.
#[cfg_attr(target_arch = "wasm32", wasm_minimal_protocol::wasm_func)]
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
    let options_json = match typst_options_json(options_json) {
        Ok(options_json) => options_json,
        Err(error) => return typst_binding_error_payload(operation.id(), &error),
    };
    match execute_typst_operation(operation.id(), input, &options_json) {
        Ok(output) => match serde_json::from_slice::<Value>(&output) {
            Ok(result) => typst_success_payload(operation.id(), json!({ "result": result })),
            Err(error) => typst_internal_error_payload(
                operation.id(),
                format!("theme operation returned invalid canonical JSON: {error}"),
            ),
        },
        Err(error) => typst_binding_error_payload(operation.id(), &error),
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

fn execute_typst_operation(
    operation_id: &str,
    source: &[u8],
    options_json: &[u8],
) -> Result<Vec<u8>, merman_bindings_core::BindingError> {
    typst_artifact_contract()
        .execute_once(
            BindingOperationRequest::new(operation_id, source).with_options_json(options_json),
        )
        .map(merman_bindings_core::BindingOperationResult::into_data)
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
    let mut details = serde_json::Map::new();
    if let Some(resource) = error.resource_details() {
        details.insert("resource".to_string(), json!(resource));
    }
    if let Some(icon_registry) = error.icon_registry_details() {
        details.insert("icon_registry".to_string(), json!(icon_registry));
    }
    if let Some(cancellation) = error.cancellation_details() {
        details.insert("cancellation".to_string(), json!(cancellation));
    }
    #[cfg(feature = "svg")]
    if let Some(theme_authoring) = error.theme_authoring_details() {
        details.insert("theme_authoring".to_string(), json!(theme_authoring));
    }
    if !details.is_empty() {
        payload["details"] = Value::Object(details);
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

fn typst_options_json(options_json: &[u8]) -> Result<Vec<u8>, merman_bindings_core::BindingError> {
    let normalized =
        merman_bindings_core::apply_resource_ceiling_json(options_json, "constrained", &[])?;
    let mut options = serde_json::from_slice::<Value>(&normalized).map_err(|error| {
        merman_bindings_core::BindingError::new(
            merman_bindings_core::BindingStatus::InternalError,
            format!("failed to decode normalized Typst options: {error}"),
        )
    })?;
    let root = options.as_object_mut().ok_or_else(|| {
        merman_bindings_core::BindingError::new(
            merman_bindings_core::BindingStatus::InternalError,
            "normalized Typst options must be an object",
        )
    })?;

    match root.get("runtime_policy") {
        Some(Value::String(policy)) if policy.trim().eq_ignore_ascii_case("native") => {
            return Err(merman_bindings_core::BindingError::missing_capability(
                "system-clock",
                "runtime_policy=native is not available in the Typst transport",
            ));
        }
        Some(Value::String(policy)) if policy.trim().eq_ignore_ascii_case("deterministic") => {
            root.insert(
                "runtime_policy".to_string(),
                Value::String("deterministic".to_string()),
            );
        }
        Some(Value::Null) | None => {
            root.insert(
                "runtime_policy".to_string(),
                Value::String("deterministic".to_string()),
            );
        }
        Some(_) => {}
    }

    if typst_artifact_contract()
        .runtime_capabilities()
        .has_operation("svg")
    {
        let environment = root
            .entry("environment".to_string())
            .or_insert_with(|| json!({}));
        if environment.is_null() {
            *environment = json!({});
        }
        let environment = environment.as_object_mut().ok_or_else(|| {
            merman_bindings_core::BindingError::new(
                merman_bindings_core::BindingStatus::OptionsJsonError,
                "invalid options_json: `environment` must be an object",
            )
        })?;
        match environment.get("math_renderer") {
            Some(Value::String(renderer)) if renderer.trim().eq_ignore_ascii_case("ratex") => {
                return Err(merman_bindings_core::BindingError::missing_capability(
                    "math",
                    "environment.math_renderer=ratex is not available in the Typst transport",
                ));
            }
            Some(Value::String(renderer)) if renderer.trim().eq_ignore_ascii_case("none") => {
                environment.insert(
                    "math_renderer".to_string(),
                    Value::String("none".to_string()),
                );
            }
            Some(Value::String(renderer)) => {
                return Err(merman_bindings_core::BindingError::new(
                    merman_bindings_core::BindingStatus::InvalidArgument,
                    format!("unsupported environment.math_renderer: {renderer}"),
                ));
            }
            Some(_) => {
                return Err(merman_bindings_core::BindingError::new(
                    merman_bindings_core::BindingStatus::OptionsJsonError,
                    "invalid options_json: `environment.math_renderer` must be a string",
                ));
            }
            None => {
                environment.insert(
                    "math_renderer".to_string(),
                    Value::String("none".to_string()),
                );
            }
        }
    }

    serde_json::to_vec(&options).map_err(|error| {
        merman_bindings_core::BindingError::new(
            merman_bindings_core::BindingStatus::InternalError,
            format!("failed to encode normalized Typst options: {error}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_version_is_stable() {
        assert_eq!(TYPST_PLUGIN_ABI_VERSION, 3);
        assert_eq!(TYPST_PLUGIN_ABI_VERSION_BYTES, b"3");
        assert_eq!(abi_version(), b"3");
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
        assert!(capabilities["system_adapter_ids"]
            .as_array()
            .unwrap()
            .is_empty());
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
        assert!(TYPST_OPERATIONS
            .iter()
            .all(|operation| TYPST_TRANSPORT_OPERATION_KEYS.contains(operation)));
        assert_eq!(
            TYPST_TRANSPORT_OPERATION_KEYS
                .iter()
                .copied()
                .map(OperationKey::id)
                .collect::<Vec<_>>(),
            [
                "analysis-json",
                "describe-theme-support-json",
                "export-theme-preset-json",
                "materialize-theme-json",
                "svg",
            ]
        );
        assert_eq!(
            projected.has_operation("analysis-json"),
            cfg!(feature = "analysis")
        );
        assert_eq!(projected.has_operation("svg"), cfg!(feature = "svg"));
        assert_eq!(
            projected.has_operation("materialize-theme-json"),
            cfg!(feature = "svg")
        );
        assert_eq!(
            projected.has_operation("describe-theme-support-json"),
            cfg!(feature = "svg")
        );
        assert_eq!(
            projected.has_operation("export-theme-preset-json"),
            cfg!(feature = "svg")
        );
        #[cfg(feature = "svg")]
        {
            assert_eq!(
                projected.capability_ids.contains(&"layout-cytoscape"),
                cfg!(feature = "layout-cytoscape")
            );
            assert_eq!(
                projected.capability_ids.contains(&"layout-elk"),
                cfg!(feature = "layout-elk")
            );
        }
    }

    #[test]
    fn typst_execution_is_bound_to_the_closed_operation_set() {
        let error = execute_typst_operation("semantic-json", b"flowchart TD\nA --> B", b"")
            .expect_err("the Typst transport must not inherit the Rust semantic operation");

        assert_eq!(
            error.status(),
            merman_bindings_core::BindingStatus::UnsupportedOperation
        );
        assert!(error.message().contains("not exposed by target `typst`"));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn theme_operation_json_materializes_the_shared_definition_contract() {
        let definition = br##"{
            "authoring_schema_version": 1,
            "expansion_version": 1,
            "tokens": {"canvas": "#0f172a", "text": "#e5e7eb"}
        }"##;
        let payload: Value = serde_json::from_slice(&theme_operation_json(
            b"materialize-theme-json",
            definition,
            b"",
        ))
        .expect("valid theme-operation envelope");

        assert_success_envelope(&payload, "materialize-theme-json");
        assert_eq!(payload["data"]["result"]["authoring_schema_version"], 1);
        assert_eq!(payload["data"]["result"]["expansion_version"], 1);
        assert_eq!(payload["data"]["result"]["spec_schema_version"], 1);
        assert!(payload["data"]["result"]["spec"]["styles"]
            .as_array()
            .is_some_and(|styles| !styles.is_empty()));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn theme_operation_json_rejects_non_theme_operations() {
        let payload: Value =
            serde_json::from_slice(&theme_operation_json(b"svg", b"flowchart TD\nA --> B", b""))
                .expect("valid theme-operation error envelope");

        assert_error_envelope(&payload, "svg", "MERMAN_UNSUPPORTED_OPERATION");
        assert!(payload["message"]
            .as_str()
            .is_some_and(|message| message.contains("theme authoring")));
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
            b"materialize-theme-json\nforged",
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
        assert!(!payload
            .to_string()
            .contains("future-theme-operation-identifier"));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn theme_operation_json_preserves_contract_owned_authoring_details() {
        let payload: Value = serde_json::from_slice(&theme_operation_json(
            b"materialize-theme-json",
            br#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{"series":[]}}"#,
            b"",
        ))
        .expect("valid theme-authoring error envelope");

        assert_error_envelope(
            &payload,
            "materialize-theme-json",
            "MERMAN_INVALID_ARGUMENT",
        );
        assert_eq!(
            payload["details"]["theme_authoring"]["diagnostics"][0]["code"],
            "theme-authoring.empty-series"
        );
        assert_eq!(
            payload["details"]["theme_authoring"]["diagnostics"][0]["path"],
            "/tokens/series"
        );
    }

    #[test]
    fn typst_transport_keeps_a_fixed_constrained_resource_policy() {
        let options = typst_options_json(b"").expect("default Typst options");
        let payload: Value = serde_json::from_slice(&options).expect("valid options JSON");
        assert_eq!(payload["runtime_policy"], "deterministic");
        assert_eq!(payload["resources"]["profile"], "constrained");
        if typst_artifact_contract()
            .runtime_capabilities()
            .has_operation("svg")
        {
            assert_eq!(payload["environment"]["math_renderer"], "none");
        } else {
            assert!(payload.get("environment").is_none());
        }
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
        let options = typst_options_json(input).expect("null transport policy fields");
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
        let options = typst_options_json(br#"{"resources":{"limits":{"max_source_bytes":4096}}}"#)
            .expect("valid options");
        let payload: Value = serde_json::from_slice(&options).expect("valid options JSON");

        assert_eq!(payload["resources"]["profile"], "constrained");
        assert_eq!(
            payload["resources"]["limits"]["max_source_bytes"],
            Value::from(4096)
        );
    }

    #[test]
    fn typst_resource_policy_preserves_analysis_wrapper_shape() {
        for wrapper in ["analysis", "merman"] {
            let input = format!(r#"{{ "{wrapper}": {{ "site_config": {{ "theme": "dark" }} }} }}"#);
            let options = typst_options_json(input.as_bytes()).expect("valid wrapped options");
            let payload: Value = serde_json::from_slice(&options).expect("valid options JSON");

            assert_eq!(
                payload[wrapper]["resources"],
                json!({ "profile": "constrained" })
            );
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
        let native = typst_options_json(br#"{"runtime_policy":"native"}"#).unwrap_err();
        assert_eq!(
            native.status(),
            merman_bindings_core::BindingStatus::UnsupportedOperation
        );
        assert_eq!(
            native.kind(),
            merman_bindings_core::BindingErrorKind::MissingCapability
        );
        assert_eq!(native.capability_id(), Some("system-clock"));

        if typst_artifact_contract()
            .runtime_capabilities()
            .has_operation("svg")
        {
            let ratex =
                typst_options_json(br#"{"environment":{"math_renderer":"ratex"}}"#).unwrap_err();
            assert_eq!(
                ratex.status(),
                merman_bindings_core::BindingStatus::UnsupportedOperation
            );
            assert_eq!(
                ratex.kind(),
                merman_bindings_core::BindingErrorKind::MissingCapability
            );
            assert_eq!(ratex.capability_id(), Some("math"));
        }
    }

    #[test]
    fn explicit_looser_resource_profile_is_rejected() {
        let error =
            typst_options_json(br#"{"resources":{"profile":"trusted-native"}}"#).unwrap_err();
        assert_eq!(
            error.status(),
            merman_bindings_core::BindingStatus::OptionsJsonError
        );
        assert!(error.message().contains("loosen the transport ceiling"));
    }

    #[test]
    fn null_resource_profile_cannot_bypass_the_typst_limits() {
        let options = typst_options_json(
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
            let error = typst_options_json(options).unwrap_err();
            assert_eq!(
                error.status(),
                merman_bindings_core::BindingStatus::OptionsJsonError
            );
            assert!(error.message().contains("wrapper must be an object"));
        }

        let error = typst_options_json(br#"{"analysis":{},"merman":{}}"#).unwrap_err();
        assert_eq!(
            error.status(),
            merman_bindings_core::BindingStatus::OptionsJsonError
        );
        assert!(error.message().contains("must not contain both"));
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
        assert!(
            svg.contains("font-size:16px") || svg.contains("font-size: 16px"),
            "class fallback typography must survive the Typst transport: {svg}"
        );
    }

    #[cfg(feature = "svg")]
    #[test]
    fn render_svg_json_keeps_explicit_typography_in_the_resvg_safe_transport() {
        let payload: Value = serde_json::from_slice(&render_svg_json(
            br#"classDiagram
    class User {
        +String name
    }"#,
            br#"{
                "presentation": {
                    "theme": {
                        "font_family": "Typst Explicit Sans",
                        "font_size": "18px"
                    }
                },
                "svg": { "pipeline": "resvg-safe" }
            }"#,
        ))
        .expect("valid JSON payload");

        assert_success_envelope(&payload, RENDER_OPERATION);
        let svg = payload["data"]["svg"].as_str().expect("SVG payload");
        assert!(!svg.contains("<foreignObject"), "{svg}");
        assert!(svg.contains("Typst Explicit Sans"), "{svg}");
        assert!(
            svg.contains("font-size:18px") || svg.contains("font-size: 18px"),
            "explicit Typst typography must reach the fallback text: {svg}"
        );
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn render_svg_json_renders_flowchart_elk_from_default_artifact() {
        let payload: Value = serde_json::from_slice(&render_svg_json(
            b"flowchart-elk TD\nA[Hello] --> B[World]",
            b"",
        ))
        .expect("valid JSON payload");

        assert_success_envelope(&payload, RENDER_OPERATION);
        assert!(payload["data"]["svg"].as_str().unwrap().contains("Hello"));
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
        assert!(payload["message"]
            .as_str()
            .unwrap()
            .contains("max_source_bytes"));
        assert_eq!(
            payload["details"]["resource"]["limit_id"],
            "max_source_bytes"
        );
        assert_eq!(payload["details"]["resource"]["profile"], "constrained");
    }

    #[cfg(feature = "svg")]
    #[test]
    fn typst_options_preflight_measures_raw_theme_bytes_before_value_normalization() {
        let raw_padding = " ".repeat(600 * 1024);
        let options_json = format!(r#"{{"theme":{{{raw_padding}"preset":"base"}}}}"#);

        let error = typst_options_json(options_json.as_bytes())
            .expect_err("raw encoded theme bytes must be checked before Value normalization");
        assert_eq!(
            error.status(),
            merman_bindings_core::BindingStatus::ResourceLimitExceeded
        );
        let resource = error
            .resource_details()
            .expect("raw theme input rejection must remain structured");
        assert_eq!(resource.limit_id, "max_theme_encoded_bytes");
        assert_eq!(resource.profile, "constrained");

        let compact = json!({ "theme": { "preset": "base" } }).to_string();
        typst_options_json(compact.as_bytes())
            .expect("the same semantic theme without raw padding must remain below the ceiling");
    }

    #[cfg(feature = "svg")]
    #[test]
    fn render_svg_json_rejects_oversized_font_input_before_base64_decoding() {
        let oversized_base64 = "A".repeat(600 * 1024);
        let options = json!({
            "theme": {
                "spec": {
                    "assets": {
                        "fonts": [{
                            "id": "oversized-font",
                            "format": "truetype",
                            "data_base64": oversized_base64
                        }]
                    }
                }
            }
        });
        let options_json = options.to_string();
        let preflight_error = typst_options_json(options_json.as_bytes())
            .expect_err("Typst options preflight must reject before typed font decoding");
        assert_eq!(
            preflight_error.status(),
            merman_bindings_core::BindingStatus::ResourceLimitExceeded
        );
        let preflight_resource = preflight_error
            .resource_details()
            .expect("Typst options preflight rejection must remain structured");
        assert_eq!(preflight_resource.limit_id, "max_theme_encoded_bytes");
        assert_eq!(preflight_resource.profile, "constrained");

        let payload: Value = serde_json::from_slice(&render_svg_json(
            b"flowchart TD\nA --> B",
            options_json.as_bytes(),
        ))
        .expect("valid JSON payload");

        assert_error_envelope(&payload, RENDER_OPERATION, "MERMAN_RESOURCE_LIMIT_EXCEEDED");
        assert_eq!(
            payload["details"]["resource"]["limit_id"],
            "max_theme_encoded_bytes"
        );
        assert_eq!(payload["details"]["resource"]["profile"], "constrained");
    }

    #[cfg(feature = "svg")]
    #[test]
    fn render_svg_json_reports_effect_graph_compile_limits() {
        let effects = (0..9)
            .map(|index| {
                json!({
                    "kind": "graph",
                    "id": format!("effect-{index}"),
                    "primitives": [{
                        "kind": "gaussian-blur",
                        "std_deviation": 1.0
                    }]
                })
            })
            .collect::<Vec<_>>();
        let options = json!({ "theme": { "spec": { "effects": effects } } });
        let options_json = options.to_string();
        let normalized = typst_options_json(options_json.as_bytes())
            .expect("Typst options normalization should admit the encoded effect input");
        let compile_error = typst_artifact_contract()
            .create_engine(&normalized)
            .err()
            .expect("the constrained Typst engine must reject excess effect graphs");
        assert_eq!(
            compile_error.status(),
            merman_bindings_core::BindingStatus::ResourceLimitExceeded
        );
        let compile_resource = compile_error
            .resource_details()
            .expect("Typst compile rejection must remain structured");
        assert_eq!(compile_resource.limit_id, "max_effect_graphs");
        assert_eq!(compile_resource.profile, "constrained");

        let payload: Value = serde_json::from_slice(&render_svg_json(
            b"flowchart TD\nA --> B",
            options_json.as_bytes(),
        ))
        .expect("valid JSON payload");

        assert_error_envelope(&payload, RENDER_OPERATION, "MERMAN_RESOURCE_LIMIT_EXCEEDED");
        assert_eq!(
            payload["details"]["resource"]["limit_id"],
            "max_effect_graphs"
        );
        assert_eq!(payload["details"]["resource"]["profile"], "constrained");
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
        let payload: Value = serde_json::from_slice(&analyze_json(b"flowchart TD\nA --> B", b""))
            .expect("valid JSON payload");

        assert_success_envelope(&payload, ANALYZE_OPERATION);
        let analysis = &payload["data"]["analysis"];
        assert_eq!(
            analysis["version"],
            merman_bindings_core::ANALYSIS_PAYLOAD_VERSION
        );
        assert_eq!(analysis["valid"], true);
        assert!(analysis["diagnostics"].as_array().is_some());
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn analyze_json_returns_a_structured_error_envelope_for_option_failures() {
        let payload: Value = serde_json::from_slice(&analyze_json(b"flowchart TD\nA --> B", b"{"))
            .expect("valid JSON payload");

        assert_error_envelope(&payload, ANALYZE_OPERATION, "MERMAN_OPTIONS_JSON_ERROR");
    }

    #[cfg(any(feature = "svg", feature = "analysis"))]
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

    #[cfg(any(feature = "svg", feature = "analysis"))]
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

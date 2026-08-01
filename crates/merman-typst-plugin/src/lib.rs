//! Typst WebAssembly plugin bridge for `merman`.
//!
//! This crate exposes one versioned Typst transport contract. The package wrapper may present
//! convenient render and analysis helpers, but every plugin operation returns the same closed
//! transport envelope so hosts can handle failures without depending on trap behavior.

use serde_json::{json, Value};

const TYPST_RESULT_PAYLOAD_SCHEMA_VERSION: u32 = 1;
pub const TYPST_RUNTIME_CATALOG_SCHEMA_VERSION: u32 =
    merman_bindings_core::RUNTIME_CATALOG_SCHEMA_VERSION;
/// Canonical binding operations exposed by the Typst transport.
///
/// These are semantic operation IDs from the shared capability descriptor, not WebAssembly export
/// names. Keep this list closed so the package and artifact smoke tests can reject accidental
/// transport expansion.
pub const TYPST_BINDING_OPERATION_IDS: &[&str] = &["analysis-json", "svg"];
const RENDER_OPERATION: &str = "render-svg";
const ANALYZE_OPERATION: &str = "analyze";

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

fn typst_capability_surface() -> merman_bindings_core::ArtifactCapabilitySurface {
    merman_bindings_core::compiled_runtime_capability_surface()
        .project_to_descriptor_target(
            "typst",
            merman_bindings_core::TextMeasurementProviderProjection::VendoredOnly,
        )
        .expect("the Typst target is declared by the capability descriptor")
}

fn typst_capabilities_json() -> Vec<u8> {
    let catalog = merman_bindings_core::runtime_catalog_for(
        TYPST_PLUGIN_ABI_VERSION,
        typst_capability_surface(),
    );
    serde_json::to_vec(&catalog).expect("the checked Typst capability catalog is serializable")
}

#[cfg_attr(target_arch = "wasm32", wasm_minimal_protocol::wasm_func)]
pub fn render_svg_json(source: &[u8], options_json: &[u8]) -> Vec<u8> {
    let options_json = match typst_options_json(options_json) {
        Ok(options_json) => options_json,
        Err(error) => return typst_binding_error_payload(RENDER_OPERATION, &error),
    };
    match merman_bindings_core::render_svg(source, &options_json) {
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
    match merman_bindings_core::analyze_json(source, &options_json) {
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
    if let Some(resource) = error.resource_details() {
        payload["details"] = json!({ "resource": resource });
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

    if typst_capability_surface()
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
        assert_eq!(TYPST_PLUGIN_ABI_VERSION, 2);
        assert_eq!(TYPST_PLUGIN_ABI_VERSION_BYTES, b"2");
        assert_eq!(abi_version(), b"2");
    }

    #[test]
    fn package_version_matches_crate_version() {
        assert_eq!(package_version(), env!("CARGO_PKG_VERSION").as_bytes());
    }

    #[test]
    fn capabilities_json_exposes_the_flat_artifact_runtime_catalog() {
        let payload: Value = serde_json::from_slice(&capabilities_json()).expect("valid JSON");
        let expected_catalog = merman_bindings_core::runtime_catalog_for(
            TYPST_PLUGIN_ABI_VERSION,
            typst_capability_surface(),
        );

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
        assert!(!capabilities["capability_ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|id| id == "ascii" || id == "math"));
        assert!(capabilities["system_adapter_ids"]
            .as_array()
            .unwrap()
            .is_empty());
        if let Some(text_measurement) = capabilities["text_measurement"].as_object() {
            assert_eq!(
                text_measurement["provider_ids"],
                json!([merman_bindings_core::TEXT_MEASUREMENT_PROVIDER_VENDORED])
            );
        }
    }

    #[test]
    fn typst_target_projection_tracks_resolved_backend_and_closed_operation_set() {
        let projected = typst_capability_surface().runtime_capabilities();
        let backend = merman_bindings_core::compiled_runtime_capabilities();
        assert!(projected
            .operation_ids
            .iter()
            .all(|operation| TYPST_BINDING_OPERATION_IDS.contains(operation)));
        for operation in TYPST_BINDING_OPERATION_IDS {
            assert_eq!(
                projected.has_operation(operation),
                backend.has_operation(operation),
                "the Typst projection must follow the resolved backend for {operation}"
            );
        }
        for layout in ["layout-cytoscape", "layout-elk"] {
            assert_eq!(
                projected.capability_ids.contains(&layout),
                backend.capability_ids.contains(&layout),
                "the capability descriptor admits {layout} for Typst"
            );
        }
    }

    #[test]
    fn typst_transport_keeps_a_fixed_constrained_resource_policy() {
        let options = typst_options_json(b"").expect("default Typst options");
        let payload: Value = serde_json::from_slice(&options).expect("valid options JSON");
        assert_eq!(payload["runtime_policy"], "deterministic");
        assert_eq!(payload["resources"]["profile"], "constrained");
        if typst_capability_surface()
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
        let has_svg = typst_capability_surface()
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
            if typst_capability_surface()
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

        if typst_capability_surface()
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

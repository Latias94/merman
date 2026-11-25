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
mod tests;

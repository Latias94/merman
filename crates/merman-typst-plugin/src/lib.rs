//! Typst WebAssembly plugin bridge for `merman`.
//!
//! This crate intentionally mirrors the shared binding facade instead of exposing a
//! Typst-specific rendering stack. The Typst package can pass the same options JSON
//! used by the browser/native bindings, while the compiled wasm exports the minimal
//! protocol functions that Typst can call.

use std::fmt::{self, Display, Formatter};

#[cfg(all(feature = "analysis", not(feature = "render")))]
const TYPST_ANALYSIS_MAX_SOURCE_BYTES: usize = 1024 * 1024;

#[cfg(target_arch = "wasm32")]
wasm_minimal_protocol::initiate_protocol!();

include!(concat!(env!("OUT_DIR"), "/typst_plugin_abi.rs"));

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypstPluginError {
    message: String,
}

impl TypstPluginError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for TypstPluginError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for TypstPluginError {}

impl From<merman_bindings_core::BindingError> for TypstPluginError {
    fn from(error: merman_bindings_core::BindingError) -> Self {
        Self::new(format!(
            "{}: {}",
            error.status().code_name(),
            error.message()
        ))
    }
}

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

fn typst_capabilities_json() -> Vec<u8> {
    let capabilities = project_typst_capabilities(merman_bindings_core::binding_capabilities());
    merman_bindings_core::binding_capabilities_json_for(capabilities)
        .expect("BindingCapabilities contains only infallibly serializable fields")
}

fn project_typst_capabilities(
    mut capabilities: merman_bindings_core::BindingCapabilities,
) -> merman_bindings_core::BindingCapabilities {
    capabilities.ascii = false;
    capabilities.ratex_math = false;
    capabilities.editor_language = false;
    capabilities.text_measurement.host_callback = false;
    capabilities.text_measurement.font_assets = false;
    capabilities
}

#[cfg_attr(target_arch = "wasm32", wasm_minimal_protocol::wasm_func)]
pub fn render_svg_json(source: &[u8], options_json: &[u8]) -> Vec<u8> {
    let options_json = typst_options_json(options_json);
    match merman_bindings_core::render_svg(source, &options_json) {
        Ok(svg) => match std::str::from_utf8(&svg) {
            Ok(svg) => merman_bindings_core::render_payload_json_bytes(
                merman_bindings_core::BindingStatus::Ok,
                None,
                Some(svg),
            ),
            Err(error) => {
                let message = format!("render_svg returned non-UTF-8 SVG: {error}");
                merman_bindings_core::render_payload_json_bytes(
                    merman_bindings_core::BindingStatus::InternalError,
                    Some(message.as_str()),
                    None,
                )
            }
        },
        Err(error) => merman_bindings_core::render_payload_json_bytes(
            error.status(),
            Some(error.message()),
            None,
        ),
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_minimal_protocol::wasm_func)]
pub fn analyze_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, TypstPluginError> {
    let options_json = typst_options_json(options_json);
    merman_bindings_core::analyze_json(source, &options_json).map_err(TypstPluginError::from)
}

#[cfg(any(feature = "render", feature = "analysis"))]
fn typst_options_json(options_json: &[u8]) -> Vec<u8> {
    if options_json.is_empty() {
        return serde_json::to_vec(&serde_json::json!({
            "resources": typst_resource_options()
        }))
        .expect("Typst resource options are infallibly serializable");
    }

    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(options_json) else {
        return options_json.to_vec();
    };
    let Some(object) = value.as_object_mut() else {
        return options_json.to_vec();
    };
    object.insert("resources".to_string(), typst_resource_options());
    serde_json::to_vec(&value).expect("serde_json::Value serialization is infallible")
}

#[cfg(feature = "render")]
fn typst_resource_options() -> serde_json::Value {
    serde_json::json!({ "profile": "constrained" })
}

#[cfg(all(feature = "analysis", not(feature = "render")))]
fn typst_resource_options() -> serde_json::Value {
    serde_json::json!({
        "limits": { "max_source_bytes": TYPST_ANALYSIS_MAX_SOURCE_BYTES }
    })
}

#[cfg(not(any(feature = "render", feature = "analysis")))]
fn typst_options_json(options_json: &[u8]) -> Vec<u8> {
    options_json.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

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
    fn capabilities_json_reports_text_measurement_boundary() {
        let payload: Value = serde_json::from_slice(&capabilities_json()).expect("valid JSON");

        let backend = merman_bindings_core::binding_capabilities();
        assert_eq!(payload["render"], backend.render);
        assert_eq!(payload["analysis"], backend.analysis);
        assert_eq!(payload["ascii"], false);
        assert_eq!(payload["core_host"], backend.core_host);
        assert_eq!(payload["cytoscape_layout"], backend.cytoscape_layout);
        assert_eq!(payload["elk_layout"], backend.elk_layout);
        assert_eq!(payload["ratex_math"], false);
        assert_eq!(payload["editor_language"], false);
        assert_eq!(
            payload["text_measurement"]["vendored"],
            backend.text_measurement.vendored
        );
        assert_eq!(
            payload["text_measurement"]["deterministic"],
            backend.text_measurement.deterministic
        );
        assert_eq!(payload["text_measurement"]["host_callback"], false);
        assert_eq!(payload["text_measurement"]["font_assets"], false);
    }

    #[test]
    fn typst_capabilities_preserve_backend_owned_layout_availability() {
        let backend = merman_bindings_core::BindingCapabilities {
            render: true,
            analysis: true,
            ascii: true,
            core_host: true,
            cytoscape_layout: true,
            elk_layout: true,
            ratex_math: true,
            editor_language: true,
            text_measurement: merman_bindings_core::TextMeasurementCapabilities {
                vendored: true,
                deterministic: true,
                host_callback: true,
                font_assets: true,
            },
        };

        let projected = project_typst_capabilities(backend);
        assert!(projected.cytoscape_layout);
        assert!(projected.elk_layout);
        assert!(projected.render);
        assert!(projected.analysis);
        assert!(projected.core_host);
        assert!(projected.text_measurement.vendored);
        assert!(projected.text_measurement.deterministic);
        assert!(!projected.ascii);
        assert!(!projected.ratex_math);
        assert!(!projected.editor_language);
        assert!(!projected.text_measurement.host_callback);
        assert!(!projected.text_measurement.font_assets);
    }

    #[cfg(any(feature = "render", feature = "analysis"))]
    #[test]
    fn typst_resource_policy_replaces_caller_resource_options() {
        let options = typst_options_json(br#"{"resources":{"limits":{"max_source_bytes":4096}}}"#);
        let payload: Value = serde_json::from_slice(&options).expect("valid options JSON");

        assert_eq!(payload["resources"], typst_resource_options());
    }

    #[cfg(any(feature = "render", feature = "analysis"))]
    #[test]
    fn explicit_resource_profile_cannot_bypass_typst_limits() {
        let options = typst_options_json(br#"{"resources":{"profile":"trusted-native"}}"#);
        let payload: Value = serde_json::from_slice(&options).expect("valid options JSON");

        assert_eq!(payload["resources"], typst_resource_options());
    }

    #[cfg(any(feature = "render", feature = "analysis"))]
    #[test]
    fn null_resources_select_the_typst_profile() {
        let options = typst_options_json(br#"{"resources":null}"#);
        let payload: Value = serde_json::from_slice(&options).expect("valid options JSON");

        assert_eq!(payload["resources"], typst_resource_options());
    }

    #[cfg(any(feature = "render", feature = "analysis"))]
    #[test]
    fn null_resource_profile_cannot_bypass_the_typst_limits() {
        let options = typst_options_json(
            br#"{"resources":{"profile":null,"limits":{"max_source_bytes":4096}}}"#,
        );
        let payload: Value = serde_json::from_slice(&options).expect("valid options JSON");

        assert_eq!(payload["resources"], typst_resource_options());
    }

    #[cfg(feature = "render")]
    #[test]
    fn render_svg_json_returns_success_payload() {
        let payload: Value = serde_json::from_slice(&render_svg_json(
            b"flowchart TD\nA[Hello] --> B[World]",
            b"",
        ))
        .expect("valid JSON payload");

        assert_eq!(payload["version"], 1);
        assert_eq!(payload["ok"], true);
        assert_eq!(payload["code_name"], "MERMAN_OK");
        assert!(payload["message"].is_null());
        assert!(payload["svg"].as_str().unwrap().contains("<svg"));
        assert!(payload["svg"].as_str().unwrap().contains("Hello"));
    }

    #[cfg(feature = "elk-layout")]
    #[test]
    fn render_svg_json_renders_flowchart_elk_from_default_artifact() {
        let payload: Value = serde_json::from_slice(&render_svg_json(
            b"flowchart-elk TD\nA[Hello] --> B[World]",
            b"",
        ))
        .expect("valid JSON payload");

        assert_eq!(payload["ok"], true);
        assert_eq!(payload["code_name"], "MERMAN_OK");
        assert!(payload["svg"].as_str().unwrap().contains("Hello"));
    }

    #[cfg(feature = "cytoscape-layout")]
    #[test]
    fn complete_typst_build_renders_architecture() {
        let payload: Value = serde_json::from_slice(&render_svg_json(
            b"architecture-beta\n  service api(server)[API service]\n",
            b"",
        ))
        .expect("valid JSON payload");

        assert_eq!(payload["ok"], true);
        assert_eq!(payload["code_name"], "MERMAN_OK");
        assert!(payload["svg"].as_str().unwrap().contains("<svg"));
    }

    #[cfg(feature = "render")]
    #[test]
    fn render_svg_json_uses_typst_resource_profile_by_default() {
        let source = format!("flowchart TD\nA[{}]", "x".repeat(1024 * 1024));
        let payload: Value = serde_json::from_slice(&render_svg_json(source.as_bytes(), b""))
            .expect("valid JSON payload");

        assert_eq!(payload["ok"], false);
        assert_eq!(payload["code_name"], "MERMAN_RESOURCE_LIMIT_EXCEEDED");
        assert!(payload["message"]
            .as_str()
            .unwrap()
            .contains("max_source_bytes"));
    }

    #[cfg(feature = "render")]
    #[test]
    fn render_svg_json_returns_error_payload() {
        let payload: Value =
            serde_json::from_slice(&render_svg_json(b"", b"")).expect("valid JSON payload");

        assert_eq!(payload["version"], 1);
        assert_eq!(payload["ok"], false);
        assert_eq!(payload["code_name"], "MERMAN_NO_DIAGRAM");
        assert!(!payload["message"].as_str().unwrap().is_empty());
        assert!(payload["svg"].is_null());
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn analyze_json_returns_the_canonical_analysis_payload() {
        let payload: Value = serde_json::from_slice(
            &analyze_json(b"flowchart TD\nA --> B", b"").expect("analysis payload"),
        )
        .expect("valid JSON payload");

        assert_eq!(
            payload["version"],
            merman_bindings_core::ANALYSIS_PAYLOAD_VERSION
        );
        assert_eq!(payload["valid"], true);
        assert!(payload["diagnostics"].as_array().is_some());
    }
}

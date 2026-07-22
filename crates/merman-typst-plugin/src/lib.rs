//! Typst WebAssembly plugin bridge for `merman`.
//!
//! This crate intentionally mirrors the shared binding facade instead of exposing a
//! Typst-specific rendering stack. The Typst package can pass the same options JSON
//! used by the browser/native bindings, while the compiled wasm exports the minimal
//! protocol functions that Typst can call.

use std::fmt::{self, Display, Formatter};

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
    let capabilities = merman_bindings_core::BindingCapabilities {
        render: cfg!(feature = "render"),
        analysis: cfg!(feature = "analysis"),
        ascii: false,
        core_full: cfg!(feature = "core-full"),
        core_host: cfg!(feature = "core-host"),
        elk_layout: cfg!(feature = "elk-layout"),
        ratex_math: false,
        editor_language: false,
        text_measurement: merman_bindings_core::TextMeasurementCapabilities {
            vendored: cfg!(feature = "render"),
            deterministic: cfg!(feature = "render"),
            host_callback: false,
            font_assets: false,
        },
    };
    merman_bindings_core::binding_capabilities_json_for(capabilities)
        .expect("BindingCapabilities contains only infallibly serializable fields")
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
        return br#"{"resources":{"profile":"constrained"}}"#.to_vec();
    }

    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(options_json) else {
        return options_json.to_vec();
    };
    let Some(object) = value.as_object_mut() else {
        return options_json.to_vec();
    };
    object.insert(
        "resources".to_string(),
        serde_json::json!({ "profile": "constrained" }),
    );
    serde_json::to_vec(&value).expect("serde_json::Value serialization is infallible")
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

        assert_eq!(payload["render"], cfg!(feature = "render"));
        assert_eq!(payload["analysis"], cfg!(feature = "analysis"));
        assert_eq!(payload["ascii"], false);
        assert_eq!(payload["core_full"], cfg!(feature = "core-full"));
        assert_eq!(payload["core_host"], cfg!(feature = "core-host"));
        assert_eq!(payload["elk_layout"], cfg!(feature = "elk-layout"));
        assert_eq!(payload["ratex_math"], false);
        assert_eq!(payload["editor_language"], false);
        assert_eq!(
            payload["text_measurement"]["vendored"],
            cfg!(feature = "render")
        );
        assert_eq!(
            payload["text_measurement"]["deterministic"],
            cfg!(feature = "render")
        );
        assert_eq!(payload["text_measurement"]["host_callback"], false);
        assert_eq!(payload["text_measurement"]["font_assets"], false);
    }

    #[cfg(any(feature = "render", feature = "analysis"))]
    #[test]
    fn typst_resource_policy_replaces_caller_resource_options() {
        let options = typst_options_json(br#"{"resources":{"limits":{"max_source_bytes":4096}}}"#);
        let payload: Value = serde_json::from_slice(&options).expect("valid options JSON");

        assert_eq!(payload["resources"]["profile"], "constrained");
        assert!(payload["resources"].get("limits").is_none());
    }

    #[cfg(any(feature = "render", feature = "analysis"))]
    #[test]
    fn explicit_resource_profile_cannot_bypass_typst_limits() {
        let options = typst_options_json(br#"{"resources":{"profile":"trusted-native"}}"#);
        let payload: Value = serde_json::from_slice(&options).expect("valid options JSON");

        assert_eq!(payload["resources"]["profile"], "constrained");
    }

    #[cfg(any(feature = "render", feature = "analysis"))]
    #[test]
    fn null_resources_select_the_typst_profile() {
        let options = typst_options_json(br#"{"resources":null}"#);
        let payload: Value = serde_json::from_slice(&options).expect("valid options JSON");

        assert_eq!(payload["resources"]["profile"], "constrained");
    }

    #[cfg(any(feature = "render", feature = "analysis"))]
    #[test]
    fn null_resource_profile_cannot_bypass_the_typst_limits() {
        let options = typst_options_json(
            br#"{"resources":{"profile":null,"limits":{"max_source_bytes":4096}}}"#,
        );
        let payload: Value = serde_json::from_slice(&options).expect("valid options JSON");

        assert_eq!(payload["resources"]["profile"], "constrained");
        assert!(payload["resources"].get("limits").is_none());
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

    #[cfg(feature = "render")]
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

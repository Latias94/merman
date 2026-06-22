//! Typst WebAssembly plugin bridge for `merman`.
//!
//! This crate intentionally mirrors the shared binding facade instead of exposing a
//! Typst-specific rendering stack. The Typst package can pass the same options JSON
//! used by the browser/native bindings, while the compiled wasm exports the minimal
//! protocol functions that Typst can call.

use std::fmt::{self, Display, Formatter};

#[cfg(target_arch = "wasm32")]
wasm_minimal_protocol::initiate_protocol!();

const ABI_VERSION: &[u8] = b"1";

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
    ABI_VERSION
}

#[cfg_attr(target_arch = "wasm32", wasm_minimal_protocol::wasm_func)]
pub fn package_version() -> Vec<u8> {
    env!("CARGO_PKG_VERSION").as_bytes().to_vec()
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
pub fn validate_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, TypstPluginError> {
    let options_json = typst_options_json(options_json);
    merman_bindings_core::validate_json(source, &options_json).map_err(TypstPluginError::from)
}

#[cfg(feature = "render")]
fn typst_options_json(options_json: &[u8]) -> Vec<u8> {
    if options_json.is_empty() {
        return br#"{"resources":{"profile":"typst-package"}}"#.to_vec();
    }

    let Ok(mut value) = serde_json::from_slice::<serde_json::Value>(options_json) else {
        return options_json.to_vec();
    };
    let Some(object) = value.as_object_mut() else {
        return options_json.to_vec();
    };
    object
        .entry("resources")
        .or_insert_with(|| serde_json::json!({ "profile": "typst-package" }));
    serde_json::to_vec(&value).unwrap_or_else(|_| options_json.to_vec())
}

#[cfg(not(feature = "render"))]
fn typst_options_json(options_json: &[u8]) -> Vec<u8> {
    options_json.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn abi_version_is_stable() {
        assert_eq!(abi_version(), b"1");
    }

    #[test]
    fn package_version_matches_crate_version() {
        assert_eq!(package_version(), env!("CARGO_PKG_VERSION").as_bytes());
    }

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
}

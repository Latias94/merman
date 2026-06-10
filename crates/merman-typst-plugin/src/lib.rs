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
pub fn render_svg(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, TypstPluginError> {
    merman_bindings_core::render_svg(source, options_json).map_err(TypstPluginError::from)
}

#[cfg_attr(target_arch = "wasm32", wasm_minimal_protocol::wasm_func)]
pub fn validate_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, TypstPluginError> {
    merman_bindings_core::validate_json(source, options_json).map_err(TypstPluginError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_version_is_stable() {
        assert_eq!(abi_version(), b"1");
    }

    #[test]
    fn package_version_matches_crate_version() {
        assert_eq!(package_version(), env!("CARGO_PKG_VERSION").as_bytes());
    }

    #[test]
    fn render_svg_returns_svg_bytes() {
        let svg = String::from_utf8(
            render_svg(b"flowchart TD\nA[Hello] --> B[World]", b"").expect("render svg"),
        )
        .expect("valid UTF-8 SVG");

        assert!(svg.contains("<svg"));
        assert!(svg.contains("Hello"));
        assert!(svg.contains("World"));
    }

    #[test]
    fn render_svg_errors_are_displayable_for_typst() {
        let error = render_svg(b"", b"").expect_err("empty input should fail");

        assert!(error.to_string().contains("MERMAN_NO_DIAGRAM"));
    }
}

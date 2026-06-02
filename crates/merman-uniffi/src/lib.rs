#![forbid(unsafe_code)]

//! UniFFI bindings for `merman`.
//!
//! This crate exposes an idiomatic generated-binding surface over `merman-bindings-core`. It does
//! not replace the canonical C ABI in `merman-ffi`.

use merman_bindings_core::{BindingError, BindingStatus};
use serde_json::Value;
use std::sync::Arc;

pub const MERMAN_UNIFFI_ABI_VERSION: u32 = 2;

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum MermanError {
    #[error("{code_name}: {message}")]
    Binding {
        code: i32,
        code_name: String,
        message: String,
    },
}

impl MermanError {
    pub fn from_binding(error: BindingError) -> Self {
        let status = error.status();
        Self::Binding {
            code: status.code(),
            code_name: status.code_name().to_string(),
            message: error.message().to_string(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        let status = BindingStatus::InternalError;
        Self::Binding {
            code: status.code(),
            code_name: status.code_name().to_string(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Default, uniffi::Object)]
pub struct MermanEngine;

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanValidationResult {
    pub valid: bool,
    pub error: Option<String>,
    pub code: i32,
    pub code_name: String,
}

#[uniffi::export]
impl MermanEngine {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }

    pub fn abi_version(&self) -> u32 {
        MERMAN_UNIFFI_ABI_VERSION
    }

    pub fn package_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    pub fn render_svg(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_output(merman_bindings_core::render_svg(
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
        ))
    }

    pub fn render_ascii(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_output(render_ascii_binding(
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
        ))
    }

    pub fn parse_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_output(merman_bindings_core::parse_json(
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
        ))
    }

    pub fn layout_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_output(merman_bindings_core::layout_json(
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
        ))
    }

    pub fn validate(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<MermanValidationResult, MermanError> {
        validation_output(merman_bindings_core::validate_json(
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
        ))
    }

    pub fn supported_diagrams(&self) -> Vec<String> {
        string_vec(merman_bindings_core::supported_diagrams())
    }

    pub fn ascii_supported_diagrams(&self) -> Vec<String> {
        string_vec(merman_bindings_core::ascii_supported_diagrams())
    }

    pub fn themes(&self) -> Vec<String> {
        string_vec(merman_bindings_core::supported_themes())
    }
}

fn options_bytes(options_json: Option<&str>) -> &[u8] {
    options_json.unwrap_or_default().as_bytes()
}

fn string_output(result: Result<Vec<u8>, BindingError>) -> Result<String, MermanError> {
    let bytes = result.map_err(MermanError::from_binding)?;
    String::from_utf8(bytes)
        .map_err(|err| MermanError::internal(format!("binding output was not UTF-8: {err}")))
}

fn validation_output(
    result: Result<Vec<u8>, BindingError>,
) -> Result<MermanValidationResult, MermanError> {
    let bytes = result.map_err(MermanError::from_binding)?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|err| MermanError::internal(format!("validation JSON decode failed: {err}")))?;
    let object = value
        .as_object()
        .ok_or_else(|| MermanError::internal("validation JSON was not an object"))?;
    let valid = object
        .get("valid")
        .and_then(Value::as_bool)
        .ok_or_else(|| MermanError::internal("validation JSON missing valid"))?;
    let code = object
        .get("code")
        .and_then(Value::as_i64)
        .ok_or_else(|| MermanError::internal("validation JSON missing code"))?;
    let code_name = object
        .get("code_name")
        .and_then(Value::as_str)
        .ok_or_else(|| MermanError::internal("validation JSON missing code_name"))?;
    let error = object
        .get("error")
        .and_then(Value::as_str)
        .map(str::to_string);

    Ok(MermanValidationResult {
        valid,
        error,
        code: code as i32,
        code_name: code_name.to_string(),
    })
}

fn render_ascii_binding(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    #[cfg(feature = "ascii")]
    {
        merman_bindings_core::render_ascii(source, options_json)
    }
    #[cfg(not(feature = "ascii"))]
    {
        let _ = (source, options_json);
        Err(BindingError::new(
            BindingStatus::UnsupportedFormat,
            "ASCII rendering requires the ascii feature",
        ))
    }
}

fn string_vec(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

uniffi::setup_scaffolding!();

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn engine() -> Arc<MermanEngine> {
        MermanEngine::new()
    }

    #[test]
    fn engine_renders_svg() {
        let svg = engine()
            .render_svg("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
            .unwrap();

        assert!(svg.contains("<svg"));
        assert!(svg.contains("Hello"));
        assert!(svg.contains("World"));
    }

    #[test]
    fn engine_exposes_versions() {
        let engine = engine();

        assert_eq!(engine.abi_version(), MERMAN_UNIFFI_ABI_VERSION);
        assert_eq!(engine.package_version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn engine_accepts_options_json() {
        let svg = engine()
            .render_svg(
                "flowchart TD\nA[Hello]".to_string(),
                Some(
                    r#"{
                        "layout": { "text_measurer": "deterministic" },
                        "svg": { "diagram_id": "uniffi diagram", "pipeline": "readable" }
                    }"#
                    .to_string(),
                ),
            )
            .unwrap();

        assert!(svg.contains("id=\"uniffi-diagram\""));
        assert!(svg.contains("data-merman-foreignobject"));
    }

    #[test]
    fn engine_renders_ascii() {
        let text = engine()
            .render_ascii("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
            .unwrap();

        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn engine_returns_semantic_json() {
        let json: Value = serde_json::from_str(
            &engine()
                .parse_json("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
                .unwrap(),
        )
        .unwrap();

        assert_eq!(
            json.get("type").and_then(Value::as_str),
            Some("flowchart-v2")
        );
    }

    #[test]
    fn engine_returns_layout_json() {
        let json: Value = serde_json::from_str(
            &engine()
                .layout_json("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
                .unwrap(),
        )
        .unwrap();

        assert!(json.get("meta").is_some());
        assert!(json.get("layout").is_some());
    }

    #[test]
    fn engine_validates_source() {
        let result = engine()
            .validate("flowchart TD\nA[Hello]".to_string(), None)
            .unwrap();

        assert!(result.valid);
        assert_eq!(result.code_name, BindingStatus::Ok.code_name());

        let result = engine().validate("".to_string(), None).unwrap();
        assert!(!result.valid);
        assert_eq!(result.code_name, BindingStatus::NoDiagram.code_name());
        assert!(result.error.unwrap().contains("no Mermaid diagram"));
    }

    #[test]
    fn engine_exposes_metadata() {
        let engine = engine();

        assert!(
            engine
                .supported_diagrams()
                .contains(&"flowchart".to_string())
        );
        assert!(
            engine
                .ascii_supported_diagrams()
                .contains(&"sequence".to_string())
        );
        assert!(engine.themes().contains(&"default".to_string()));
    }

    #[test]
    fn engine_error_preserves_binding_status() {
        let err = engine()
            .render_svg("flowchart TD\nA".to_string(), Some("{".to_string()))
            .unwrap_err();

        let MermanError::Binding {
            code,
            code_name,
            message,
        } = err;
        assert_eq!(code, BindingStatus::OptionsJsonError.code());
        assert_eq!(code_name, BindingStatus::OptionsJsonError.code_name());
        assert!(message.contains("invalid options_json"));
    }
}

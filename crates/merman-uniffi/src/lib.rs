#![forbid(unsafe_code)]

//! UniFFI bindings for `merman`.
//!
//! This crate exposes an idiomatic generated-binding surface over `merman-bindings-core`. It does
//! not replace the canonical C ABI in `merman-ffi`.

use merman_bindings_core::{BindingError, BindingStatus};
use std::sync::Arc;

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

#[uniffi::export]
impl MermanEngine {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
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
}

fn options_bytes(options_json: Option<&str>) -> &[u8] {
    options_json.unwrap_or_default().as_bytes()
}

fn string_output(result: Result<Vec<u8>, BindingError>) -> Result<String, MermanError> {
    let bytes = result.map_err(MermanError::from_binding)?;
    String::from_utf8(bytes)
        .map_err(|err| MermanError::internal(format!("binding output was not UTF-8: {err}")))
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

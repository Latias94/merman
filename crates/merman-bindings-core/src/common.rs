#[cfg(any(feature = "render", feature = "ascii"))]
use serde::Deserialize;
use serde::Serialize;

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingStatus {
    Ok = 0,
    InvalidArgument = 1,
    Utf8Error = 2,
    OptionsJsonError = 3,
    NoDiagram = 4,
    ParseError = 5,
    RenderError = 6,
    UnsupportedFormat = 7,
    Panic = 8,
    InternalError = 9,
}

impl BindingStatus {
    pub const fn code(self) -> i32 {
        self as i32
    }

    pub const fn code_name(self) -> &'static str {
        match self {
            Self::Ok => "MERMAN_OK",
            Self::InvalidArgument => "MERMAN_INVALID_ARGUMENT",
            Self::Utf8Error => "MERMAN_UTF8_ERROR",
            Self::OptionsJsonError => "MERMAN_OPTIONS_JSON_ERROR",
            Self::NoDiagram => "MERMAN_NO_DIAGRAM",
            Self::ParseError => "MERMAN_PARSE_ERROR",
            Self::RenderError => "MERMAN_RENDER_ERROR",
            Self::UnsupportedFormat => "MERMAN_UNSUPPORTED_FORMAT",
            Self::Panic => "MERMAN_PANIC",
            Self::InternalError => "MERMAN_INTERNAL_ERROR",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingError {
    status: BindingStatus,
    message: String,
}

impl BindingError {
    pub fn new(status: BindingStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub const fn status(&self) -> BindingStatus {
        self.status
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Serialize)]
struct ErrorPayload<'a> {
    version: u32,
    ok: bool,
    code: i32,
    code_name: &'a str,
    message: &'a str,
}

#[derive(Debug, Serialize)]
struct ValidationPayload<'a> {
    valid: bool,
    error: Option<String>,
    code: i32,
    code_name: &'a str,
}

#[cfg(any(feature = "render", feature = "ascii"))]
#[derive(Debug, Default, Deserialize)]
pub(crate) struct BindingOptions {
    #[allow(dead_code)]
    pub(crate) version: Option<u32>,
    pub(crate) site_config: Option<serde_json::Value>,
    pub(crate) parse: Option<ParseOptionsJson>,
    #[cfg(feature = "render")]
    pub(crate) layout: Option<LayoutOptionsJson>,
    #[cfg(feature = "render")]
    pub(crate) svg: Option<SvgOptionsJson>,
}

#[cfg(any(feature = "render", feature = "ascii"))]
#[derive(Debug, Default, Deserialize)]
pub(crate) struct ParseOptionsJson {
    pub(crate) suppress_errors: Option<bool>,
}

#[cfg(feature = "render")]
#[derive(Debug, Default, Deserialize)]
pub(crate) struct LayoutOptionsJson {
    pub(crate) viewport_width: Option<f64>,
    pub(crate) viewport_height: Option<f64>,
    pub(crate) text_measurer: Option<String>,
    pub(crate) math_renderer: Option<String>,
}

#[cfg(feature = "render")]
#[derive(Debug, Default, Deserialize)]
pub(crate) struct SvgOptionsJson {
    pub(crate) diagram_id: Option<String>,
    pub(crate) pipeline: Option<String>,
    pub(crate) scoped_css: Option<String>,
    pub(crate) css_override_policy: Option<String>,
    pub(crate) root_background_color: Option<String>,
    pub(crate) drop_native_duplicate_fallbacks: Option<bool>,
}

pub fn error_payload_json_bytes(status: BindingStatus, message: &str) -> Vec<u8> {
    let payload = ErrorPayload {
        version: 1,
        ok: false,
        code: status.code(),
        code_name: status.code_name(),
        message,
    };
    serde_json::to_vec(&payload).unwrap_or_else(|_| {
        format!(
            r#"{{"version":1,"ok":false,"code":{},"code_name":"{}","message":"internal error payload serialization failed"}}"#,
            BindingStatus::InternalError.code(),
            BindingStatus::InternalError.code_name()
        )
        .into_bytes()
    })
}

pub(crate) fn validation_payload_json(
    result: Result<(), BindingError>,
) -> Result<Vec<u8>, BindingError> {
    let payload = match result {
        Ok(()) => ValidationPayload {
            valid: true,
            error: None,
            code: BindingStatus::Ok.code(),
            code_name: BindingStatus::Ok.code_name(),
        },
        Err(error) => ValidationPayload {
            valid: false,
            error: Some(error.message().to_string()),
            code: error.status().code(),
            code_name: error.status().code_name(),
        },
    };
    serde_json::to_vec(&payload).map_err(internal_json_error)
}

#[cfg(any(feature = "render", feature = "ascii"))]
pub(crate) fn parse_options(bytes: &[u8]) -> Result<BindingOptions, BindingError> {
    if bytes.is_empty() {
        return Ok(BindingOptions::default());
    }
    let text = std::str::from_utf8(bytes).map_err(|err| {
        BindingError::new(
            BindingStatus::Utf8Error,
            format!("invalid options_json UTF-8: {err}"),
        )
    })?;
    serde_json::from_str(text).map_err(|err| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid options_json: {err}"),
        )
    })
}

#[cfg(any(feature = "render", feature = "ascii"))]
pub(crate) fn source_text(bytes: &[u8]) -> Result<&str, BindingError> {
    let source = std::str::from_utf8(bytes).map_err(|err| {
        BindingError::new(
            BindingStatus::Utf8Error,
            format!("invalid source UTF-8: {err}"),
        )
    })?;
    if source.trim().is_empty() {
        return Err(no_diagram_error());
    }
    Ok(source)
}

#[cfg(any(feature = "render", feature = "ascii"))]
pub(crate) fn binding_site_config(
    options: &BindingOptions,
) -> Result<Option<merman::MermaidConfig>, BindingError> {
    let Some(site_config) = options.site_config.as_ref() else {
        return Ok(None);
    };
    if !site_config.is_object() {
        return Err(BindingError::new(
            BindingStatus::InvalidArgument,
            "site_config must be a JSON object",
        ));
    }
    Ok(Some(merman::MermaidConfig::from_value(site_config.clone())))
}

#[cfg(any(feature = "render", feature = "ascii"))]
pub(crate) fn no_diagram_error() -> BindingError {
    BindingError::new(BindingStatus::NoDiagram, "no Mermaid diagram detected")
}

pub(crate) fn internal_json_error(err: serde_json::Error) -> BindingError {
    BindingError::new(
        BindingStatus::InternalError,
        format!("failed to serialize JSON output: {err}"),
    )
}

#[cfg(feature = "render")]
pub(crate) fn finite_positive(value: f64, name: &'static str) -> Result<f64, BindingError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(BindingError::new(
            BindingStatus::InvalidArgument,
            format!("{name} must be a finite positive number"),
        ))
    }
}

#[cfg(feature = "render")]
pub(crate) fn normalize_option(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(feature = "render")]
pub(crate) fn css_declaration_value(value: &str, name: &str) -> Result<String, BindingError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(BindingError::new(
            BindingStatus::InvalidArgument,
            format!("{name} must be a non-empty CSS value"),
        ));
    }

    let invalid = trimmed
        .chars()
        .any(|ch| ch.is_control() || matches!(ch, ';' | '"' | '\'' | '<' | '>' | '{' | '}'));
    if invalid {
        return Err(BindingError::new(
            BindingStatus::InvalidArgument,
            format!("{name} must be a single CSS declaration value"),
        ));
    }

    Ok(trimmed.to_string())
}

#[cfg(any(not(feature = "render"), not(feature = "ascii")))]
pub(crate) fn feature_required_error(operation: &str, feature: &str) -> BindingError {
    BindingError::new(
        BindingStatus::UnsupportedFormat,
        format!("{operation} requires the {feature} feature"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn error_payload_json_uses_public_code_names() {
        let payload = error_payload_json_bytes(BindingStatus::RenderError, "failed");
        let json: Value = serde_json::from_slice(&payload).unwrap();

        assert_eq!(json["code"], BindingStatus::RenderError.code());
        assert_eq!(json["code_name"], BindingStatus::RenderError.code_name());
        assert_eq!(json["message"], "failed");
    }
}

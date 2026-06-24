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
    ResourceLimitExceeded = 10,
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
            Self::ResourceLimitExceeded => "MERMAN_RESOURCE_LIMIT_EXCEEDED",
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
struct RenderPayload<'a> {
    version: u32,
    ok: bool,
    code: i32,
    code_name: &'a str,
    message: Option<&'a str>,
    svg: Option<&'a str>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct BindingOptions {
    #[allow(dead_code)]
    pub(crate) version: Option<u32>,
    #[serde(flatten)]
    pub(crate) analysis: merman_analysis::AnalysisOptionsJson,
    #[cfg(feature = "render")]
    pub(crate) host_theme: Option<HostThemeOptionsJson>,
    #[cfg(feature = "render")]
    pub(crate) layout: Option<LayoutOptionsJson>,
    #[cfg(feature = "render")]
    pub(crate) svg: Option<SvgOptionsJson>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct LayoutOptionsJson {
    pub(crate) viewport_width: Option<f64>,
    pub(crate) viewport_height: Option<f64>,
    pub(crate) text_measurer: Option<String>,
    pub(crate) math_renderer: Option<String>,
    pub(crate) flowchart_elk_backend: Option<String>,
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

#[cfg(feature = "render")]
#[derive(Debug, Default, Deserialize)]
pub(crate) struct HostThemeOptionsJson {
    pub(crate) preset: Option<String>,
    pub(crate) appearance: Option<String>,
    pub(crate) font_family: Option<String>,
    pub(crate) font_size: Option<String>,
    pub(crate) roles: Option<HostThemeRolesJson>,
    pub(crate) series_palette: Option<Vec<String>>,
    pub(crate) output: Option<HostThemeOutputJson>,
    #[serde(default, alias = "themeVariables")]
    pub(crate) theme_variables: Option<serde_json::Map<String, serde_json::Value>>,
    pub(crate) site_config: Option<serde_json::Value>,
}

#[cfg(feature = "render")]
#[derive(Debug, Default, Deserialize)]
pub(crate) struct HostThemeRolesJson {
    pub(crate) canvas: Option<String>,
    pub(crate) surface: Option<String>,
    pub(crate) surface_alt: Option<String>,
    pub(crate) surface_muted: Option<String>,
    pub(crate) text: Option<String>,
    pub(crate) subtle_text: Option<String>,
    pub(crate) border: Option<String>,
    pub(crate) line: Option<String>,
    pub(crate) edge_label_background: Option<String>,
    pub(crate) cluster_background: Option<String>,
    pub(crate) cluster_border: Option<String>,
    pub(crate) note_background: Option<String>,
    pub(crate) note_border: Option<String>,
    pub(crate) note_text: Option<String>,
    pub(crate) actor_background: Option<String>,
    pub(crate) actor_border: Option<String>,
    pub(crate) actor_text: Option<String>,
    pub(crate) activation_background: Option<String>,
    pub(crate) activation_border: Option<String>,
    pub(crate) error: Option<String>,
    pub(crate) warning: Option<String>,
    pub(crate) success: Option<String>,
}

#[cfg(feature = "render")]
#[derive(Debug, Default, Deserialize)]
pub(crate) struct HostThemeOutputJson {
    pub(crate) pipeline: Option<String>,
    pub(crate) css_override_policy: Option<String>,
    pub(crate) root_background: Option<String>,
    pub(crate) drop_native_duplicate_fallbacks: Option<bool>,
    pub(crate) scoped_css: Option<String>,
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

pub fn render_payload_json_bytes(
    status: BindingStatus,
    message: Option<&str>,
    svg: Option<&str>,
) -> Vec<u8> {
    let payload = RenderPayload {
        version: 1,
        ok: status == BindingStatus::Ok,
        code: status.code(),
        code_name: status.code_name(),
        message,
        svg,
    };
    serde_json::to_vec(&payload).unwrap_or_else(|_| {
        error_payload_json_bytes(
            BindingStatus::InternalError,
            "render payload serialization failed",
        )
    })
}

pub(crate) fn validation_payload_json_from_analysis(
    payload: &merman_analysis::AnalysisPayload,
) -> Result<Vec<u8>, BindingError> {
    #[derive(Serialize)]
    struct LegacyValidationPayload<'a> {
        valid: bool,
        error: Option<&'a str>,
        message: Option<&'a str>,
        code: i32,
        code_name: &'a str,
    }

    let first_error = payload.diagnostics.iter().find(|diagnostic| {
        matches!(
            diagnostic.severity,
            merman_analysis::DiagnosticSeverity::Error
        )
    });
    let legacy = LegacyValidationPayload {
        valid: payload.valid,
        error: first_error.map(|diagnostic| diagnostic.message.as_str()),
        message: first_error.map(|diagnostic| diagnostic.message.as_str()),
        code: first_error
            .and_then(|diagnostic| diagnostic.code)
            .unwrap_or(BindingStatus::Ok.code()),
        code_name: first_error
            .and_then(|diagnostic| diagnostic.code_name.as_deref())
            .unwrap_or(BindingStatus::Ok.code_name()),
    };
    serde_json::to_vec(&legacy).map_err(internal_json_error)
}

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

pub(crate) fn source_text_utf8(bytes: &[u8]) -> Result<&str, BindingError> {
    let source = std::str::from_utf8(bytes).map_err(|err| {
        BindingError::new(
            BindingStatus::Utf8Error,
            format!("invalid source UTF-8: {err}"),
        )
    })?;
    Ok(source)
}

#[cfg(any(feature = "render", feature = "ascii"))]
pub(crate) fn source_text(bytes: &[u8]) -> Result<&str, BindingError> {
    let source = source_text_utf8(bytes)?;
    if source.trim().is_empty() {
        return Err(no_diagram_error());
    }
    Ok(source)
}

pub(crate) fn binding_site_config(
    options: &BindingOptions,
) -> Result<Option<merman::MermaidConfig>, BindingError> {
    let Some(site_config) = options.analysis.site_config.as_ref() else {
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

pub(crate) fn binding_fixed_today(
    options: &BindingOptions,
) -> Result<Option<chrono::NaiveDate>, BindingError> {
    let Some(today) = options.analysis.fixed_today.as_deref() else {
        return Ok(None);
    };
    chrono::NaiveDate::parse_from_str(today, "%Y-%m-%d")
        .map(Some)
        .map_err(|_| {
            BindingError::new(
                BindingStatus::InvalidArgument,
                "fixed_today must be a date in YYYY-MM-DD format",
            )
        })
}

pub(crate) fn binding_fixed_local_offset_minutes(
    options: &BindingOptions,
) -> Result<Option<i32>, BindingError> {
    let Some(offset_minutes) = options.analysis.fixed_local_offset_minutes else {
        return Ok(None);
    };
    let valid = offset_minutes
        .checked_mul(60)
        .and_then(chrono::FixedOffset::east_opt)
        .is_some();
    if !valid {
        return Err(BindingError::new(
            BindingStatus::InvalidArgument,
            "fixed_local_offset_minutes must be between -1439 and 1439",
        ));
    }
    Ok(Some(offset_minutes))
}

pub(crate) fn analysis_options(
    options: &BindingOptions,
) -> Result<merman_analysis::AnalysisOptions, BindingError> {
    options
        .analysis
        .to_analysis_options()
        .map_err(|err| BindingError::new(BindingStatus::InvalidArgument, err.to_string()))
}

impl From<merman_analysis::AnalysisOptionsJsonError> for BindingError {
    fn from(error: merman_analysis::AnalysisOptionsJsonError) -> Self {
        BindingError::new(BindingStatus::InvalidArgument, error.to_string())
    }
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

    #[test]
    fn render_payload_json_returns_svg_or_error_shape() {
        let payload = render_payload_json_bytes(BindingStatus::Ok, None, Some("<svg/>"));
        let json: Value = serde_json::from_slice(&payload).unwrap();

        assert_eq!(json["version"], 1);
        assert_eq!(json["ok"], true);
        assert_eq!(json["code"], BindingStatus::Ok.code());
        assert_eq!(json["code_name"], BindingStatus::Ok.code_name());
        assert!(json["message"].is_null());
        assert_eq!(json["svg"], "<svg/>");

        let payload =
            render_payload_json_bytes(BindingStatus::RenderError, Some("render failed"), None);
        let json: Value = serde_json::from_slice(&payload).unwrap();

        assert_eq!(json["version"], 1);
        assert_eq!(json["ok"], false);
        assert_eq!(json["code"], BindingStatus::RenderError.code());
        assert_eq!(json["code_name"], BindingStatus::RenderError.code_name());
        assert_eq!(json["message"], "render failed");
        assert!(json["svg"].is_null());
    }
}

use serde::Deserialize;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::resource_contract::{
    BindingResourceScope, binding_resource_contract, resource_limit_descriptor,
    resource_profile_value,
};

/// Current schema for constructor and request options JSON.
///
/// Version 2 is intentionally not wire-compatible with the alpha.3 grammar that used version 1:
/// layout/environment fields and resource limits moved, and deprecated fields are rejected rather
/// than silently reinterpreted. Omitted versions use the current grammar for convenience-only
/// callers; durable SDK integrations should send this explicit version.
pub const BINDING_OPTIONS_SCHEMA_VERSION: u32 = 2;
pub const BINDING_RESULT_PAYLOAD_VERSION: u32 = 1;
const BINDING_ANALYSIS_OPTION_KEYS: [&str; 5] = [
    "fixed_today",
    "fixed_local_offset_minutes",
    "site_config",
    "resources",
    "lint",
];

/// Runtime policy selected explicitly for one binding engine or one-shot operation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BindingRuntimePolicy {
    #[default]
    Deterministic,
    Native,
}

impl BindingRuntimePolicy {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Deterministic => "deterministic",
            Self::Native => "native",
        }
    }
}

/// Stable machine-readable classification carried by binding errors.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BindingErrorKind {
    #[default]
    Generic,
    UnknownOperation,
    MissingCapability,
    Busy,
    ReentrantCall,
}

impl BindingErrorKind {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::UnknownOperation => "unknown-operation",
            Self::MissingCapability => "missing-capability",
            Self::Busy => "busy",
            Self::ReentrantCall => "reentrant-call",
        }
    }
}

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
    UnsupportedOperation = 7,
    Panic = 8,
    InternalError = 9,
    ResourceLimitExceeded = 10,
    Busy = 11,
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
            Self::UnsupportedOperation => "MERMAN_UNSUPPORTED_OPERATION",
            Self::Panic => "MERMAN_PANIC",
            Self::InternalError => "MERMAN_INTERNAL_ERROR",
            Self::ResourceLimitExceeded => "MERMAN_RESOURCE_LIMIT_EXCEEDED",
            Self::Busy => "MERMAN_BUSY",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingError {
    status: BindingStatus,
    kind: BindingErrorKind,
    capability_id: Option<&'static str>,
    resource: Option<BindingResourceErrorDetails>,
    message: String,
}

/// Structured resource failure details carried by the additive error JSON payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct BindingResourceErrorDetails {
    pub limit_id: &'static str,
    pub phase: &'static str,
    pub actual: u64,
    pub max: u64,
    pub profile: &'static str,
}

impl BindingError {
    pub fn new(status: BindingStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            kind: BindingErrorKind::Generic,
            capability_id: None,
            resource: None,
            message: message.into(),
        }
    }

    pub fn unknown_operation(message: impl Into<String>) -> Self {
        Self {
            status: BindingStatus::UnsupportedOperation,
            kind: BindingErrorKind::UnknownOperation,
            capability_id: None,
            resource: None,
            message: message.into(),
        }
    }

    pub fn missing_capability(capability_id: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: BindingStatus::UnsupportedOperation,
            kind: BindingErrorKind::MissingCapability,
            capability_id: Some(capability_id),
            resource: None,
            message: message.into(),
        }
    }

    pub fn reentrant_call(message: impl Into<String>) -> Self {
        Self {
            status: BindingStatus::InvalidArgument,
            kind: BindingErrorKind::ReentrantCall,
            capability_id: None,
            resource: None,
            message: message.into(),
        }
    }

    pub fn busy(message: impl Into<String>) -> Self {
        Self {
            status: BindingStatus::Busy,
            kind: BindingErrorKind::Busy,
            capability_id: None,
            resource: None,
            message: message.into(),
        }
    }

    pub fn resource_limit(
        phase: &'static str,
        limit_id: &'static str,
        actual: u64,
        max: u64,
        profile: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            status: BindingStatus::ResourceLimitExceeded,
            kind: BindingErrorKind::Generic,
            capability_id: None,
            resource: Some(BindingResourceErrorDetails {
                limit_id,
                phase,
                actual,
                max,
                profile,
            }),
            message: message.into(),
        }
    }

    pub const fn status(&self) -> BindingStatus {
        self.status
    }

    pub const fn kind(&self) -> BindingErrorKind {
        self.kind
    }

    pub const fn capability_id(&self) -> Option<&'static str> {
        self.capability_id
    }

    pub const fn resource_details(&self) -> Option<BindingResourceErrorDetails> {
        self.resource
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

pub(crate) fn input_resource_limit_error(
    error: merman::resources::InputResourceLimitExceeded,
) -> BindingError {
    let message = error.to_string();
    BindingError::resource_limit(
        error.phase.as_str(),
        error.limit,
        error.actual as u64,
        error.max as u64,
        error.profile.id(),
        message,
    )
}

#[derive(Debug, Serialize)]
struct ErrorPayload<'a> {
    version: u32,
    ok: bool,
    code: i32,
    code_name: &'a str,
    kind: &'a str,
    capability_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<ErrorDetails<'a>>,
    message: &'a str,
}

#[derive(Debug, Serialize)]
struct ErrorDetails<'a> {
    resource: &'a BindingResourceErrorDetails,
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
    pub(crate) runtime_policy: Option<BindingRuntimePolicy>,
    #[serde(flatten)]
    pub(crate) analysis: BindingAnalysisOptionsJson,
    pub(crate) parse: Option<ParseOptionsJson>,
    #[cfg(feature = "svg")]
    pub(crate) presentation: Option<PresentationOptionsJson>,
    #[cfg(feature = "ascii")]
    pub(crate) ascii: Option<AsciiOptionsJson>,
    #[cfg(feature = "svg")]
    pub(crate) layout: Option<LayoutOptionsJson>,
    #[cfg(feature = "svg")]
    pub(crate) environment: Option<RenderEnvironmentOptionsJson>,
    #[cfg(feature = "svg")]
    pub(crate) svg: Option<SvgOptionsJson>,
    #[cfg(any(feature = "png", feature = "jpeg"))]
    pub(crate) raster: Option<RasterOptionsJson>,
    #[cfg(feature = "jpeg")]
    pub(crate) jpeg: Option<JpegOptionsJson>,
    #[cfg(feature = "pdf")]
    pub(crate) pdf: Option<PdfOptionsJson>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct BindingAnalysisOptionsJson {
    pub(crate) fixed_today: Option<String>,
    pub(crate) fixed_local_offset_minutes: Option<i32>,
    pub(crate) site_config: Option<Value>,
    pub(crate) resources: Option<ResourceOptionsJson>,
    #[cfg(feature = "analysis")]
    pub(crate) lint: Option<merman_analysis::LintOptionsJson>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ParseOptionsJson {
    pub(crate) suppress_errors: Option<bool>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResourceOptionsJson {
    pub(crate) profile: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) limits: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct BaseBindingOptions {
    normalized_wire: Arc<Value>,
    resource_ceiling: ResourceOptionsJson,
}

#[derive(Debug)]
pub(crate) enum BindingRequestOverlay {
    Unchanged,
    Override {
        normalized_wire: Value,
        requested_resources: Option<ResourceOptionsJson>,
    },
}

#[cfg(feature = "ascii")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AsciiOptionsJson {
    pub(crate) charset: Option<String>,
    #[serde(default, alias = "defaultDirection")]
    pub(crate) default_direction: Option<String>,
    #[serde(default, alias = "colorMode")]
    pub(crate) color_mode: Option<String>,
    pub(crate) theme: Option<AsciiThemeOptionsJson>,
    #[serde(default, alias = "boxBorderPadding")]
    pub(crate) box_border_padding: Option<usize>,
    #[serde(default, alias = "graphPaddingX")]
    pub(crate) graph_padding_x: Option<usize>,
    #[serde(default, alias = "graphPaddingY")]
    pub(crate) graph_padding_y: Option<usize>,
    #[serde(default, alias = "sequenceParticipantSpacing")]
    pub(crate) sequence_participant_spacing: Option<usize>,
    #[serde(default, alias = "sequenceMessageSpacing")]
    pub(crate) sequence_message_spacing: Option<usize>,
    #[serde(default, alias = "sequenceSelfMessageWidth")]
    pub(crate) sequence_self_message_width: Option<usize>,
    #[serde(default, alias = "sequenceMirrorActors")]
    pub(crate) sequence_mirror_actors: Option<bool>,
    #[serde(default, alias = "xychartVerticalPlotHeight")]
    pub(crate) xychart_vertical_plot_height: Option<usize>,
    #[serde(default, alias = "xychartCategoryBandWidth")]
    pub(crate) xychart_category_band_width: Option<usize>,
    #[serde(default, alias = "xychartHorizontalPlotWidth")]
    pub(crate) xychart_horizontal_plot_width: Option<usize>,
    #[serde(default, alias = "relationSummaryDiagnostics")]
    pub(crate) relation_summary_diagnostics: Option<bool>,
}

#[cfg(feature = "ascii")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AsciiThemeOptionsJson {
    #[serde(default, alias = "fg")]
    pub(crate) foreground: Option<String>,
    #[serde(default, alias = "bg")]
    pub(crate) background: Option<String>,
    pub(crate) line: Option<String>,
    pub(crate) accent: Option<String>,
    pub(crate) muted: Option<String>,
    pub(crate) surface: Option<String>,
    pub(crate) border: Option<String>,
}

#[cfg(feature = "svg")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LayoutOptionsJson {
    pub(crate) container_width: Option<f64>,
    pub(crate) container_height: Option<f64>,
}

#[cfg(feature = "svg")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RenderEnvironmentOptionsJson {
    pub(crate) text_measurement: Option<String>,
    pub(crate) math_renderer: Option<String>,
}

#[cfg(feature = "svg")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SvgOptionsJson {
    pub(crate) diagram_id: Option<String>,
    #[serde(default, alias = "viewBoxPadding")]
    pub(crate) viewbox_padding: Option<f64>,
    pub(crate) pipeline: Option<String>,
    pub(crate) scoped_css: Option<String>,
    pub(crate) css_override_policy: Option<String>,
    pub(crate) root_background_color: Option<String>,
    pub(crate) drop_native_duplicate_fallbacks: Option<bool>,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RasterOptionsJson {
    pub(crate) scale: Option<f64>,
    pub(crate) background: Option<String>,
    pub(crate) fit_to: Option<RasterFitOptionsJson>,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RasterFitOptionsJson {
    pub(crate) width: Option<u32>,
    pub(crate) height: Option<u32>,
}

#[cfg(feature = "jpeg")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct JpegOptionsJson {
    pub(crate) quality: Option<u8>,
}

#[cfg(feature = "pdf")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PdfOptionsJson {
    pub(crate) background: Option<String>,
    #[serde(default, alias = "filterScale")]
    pub(crate) filter_scale: Option<f64>,
    pub(crate) page_policy: Option<PdfPageOptionsJson>,
}

#[cfg(feature = "pdf")]
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum PdfPageOptionsJson {
    FitSvg,
    Fixed { width_pt: f64, height_pt: f64 },
    FitCssWidth { max_width_px: f64 },
}

#[cfg(feature = "svg")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PresentationOptionsJson {
    pub(crate) profile: Option<String>,
    pub(crate) theme: Option<PresentationThemeOptionsJson>,
}

#[cfg(feature = "svg")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PresentationThemeOptionsJson {
    pub(crate) preset: Option<String>,
    pub(crate) appearance: Option<String>,
    pub(crate) font_family: Option<String>,
    pub(crate) font_size: Option<String>,
    pub(crate) roles: Option<BTreeMap<String, String>>,
    pub(crate) series_palette: Option<Vec<String>>,
}

pub fn error_payload_json_bytes(status: BindingStatus, message: &str) -> Vec<u8> {
    error_payload_json_bytes_with_details(status, BindingErrorKind::Generic, None, None, message)
}

pub fn binding_error_payload_json_bytes(error: &BindingError) -> Vec<u8> {
    error_payload_json_bytes_with_details(
        error.status(),
        error.kind(),
        error.capability_id(),
        error.resource.as_ref(),
        error.message(),
    )
}

fn error_payload_json_bytes_with_details(
    status: BindingStatus,
    kind: BindingErrorKind,
    capability_id: Option<&str>,
    resource: Option<&BindingResourceErrorDetails>,
    message: &str,
) -> Vec<u8> {
    let payload = ErrorPayload {
        version: BINDING_RESULT_PAYLOAD_VERSION,
        ok: false,
        code: status.code(),
        code_name: status.code_name(),
        kind: kind.id(),
        capability_id,
        details: resource.map(|resource| ErrorDetails { resource }),
        message,
    };
    serde_json::to_vec(&payload).unwrap_or_else(|_| {
        format!(
            r#"{{"version":{},"ok":false,"code":{},"code_name":"{}","kind":"generic","capability_id":null,"message":"internal error payload serialization failed"}}"#,
            BINDING_RESULT_PAYLOAD_VERSION,
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
        version: BINDING_RESULT_PAYLOAD_VERSION,
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

#[cfg(feature = "analysis")]
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
    let fallback_status = first_error
        .map(legacy_validation_fallback_status)
        .unwrap_or(BindingStatus::Ok);
    let legacy = LegacyValidationPayload {
        valid: payload.valid,
        error: first_error.map(|diagnostic| diagnostic.message.as_str()),
        message: first_error.map(|diagnostic| diagnostic.message.as_str()),
        code: first_error
            .and_then(|diagnostic| diagnostic.code)
            .unwrap_or(fallback_status.code()),
        code_name: first_error
            .and_then(|diagnostic| diagnostic.code_name.as_deref())
            .unwrap_or(fallback_status.code_name()),
    };
    serde_json::to_vec(&legacy).map_err(internal_json_error)
}

#[cfg(feature = "analysis")]
fn legacy_validation_fallback_status(
    diagnostic: &merman_analysis::AnalysisDiagnostic,
) -> BindingStatus {
    match diagnostic.category {
        merman_analysis::DiagnosticCategory::Resource => BindingStatus::ResourceLimitExceeded,
        merman_analysis::DiagnosticCategory::Render => BindingStatus::RenderError,
        merman_analysis::DiagnosticCategory::Internal => BindingStatus::InternalError,
        merman_analysis::DiagnosticCategory::Parse
        | merman_analysis::DiagnosticCategory::Semantic
        | merman_analysis::DiagnosticCategory::Config
        | merman_analysis::DiagnosticCategory::Compatibility
        | merman_analysis::DiagnosticCategory::Layout => BindingStatus::ParseError,
    }
}

pub(crate) fn parse_options(bytes: &[u8]) -> Result<BindingOptions, BindingError> {
    parse_options_value(&options_json_value(bytes)?)
}

pub(crate) fn parse_base_options(
    bytes: &[u8],
) -> Result<(BindingOptions, BaseBindingOptions), BindingError> {
    let wire = options_json_value(bytes)?;
    let typed = parse_options_value(&wire)?;
    let resource_ceiling = typed.analysis.resources.clone().unwrap_or_default();
    Ok((
        typed,
        BaseBindingOptions {
            normalized_wire: Arc::new(normalize_analysis_wrapper(wire)),
            resource_ceiling,
        },
    ))
}

fn parse_options_value(value: &Value) -> Result<BindingOptions, BindingError> {
    validate_options_schema_version(value)?;
    reject_ambiguous_analysis_wrappers(value)?;
    reject_removed_host_theme(value)?;
    reject_null_presentation_values(value)?;
    reject_uncompiled_option_groups(value)?;
    #[cfg(feature = "svg")]
    reject_removed_layout_fields(value)?;
    #[cfg(feature = "ascii")]
    reject_removed_ascii_resource_field(value)?;
    reject_removed_nested_analysis_parse_option(value)?;
    let mut options: BindingOptions = serde_json::from_value(value.clone()).map_err(|err| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid options_json: {err}"),
        )
    })?;
    reject_unknown_options_json_fields(value)?;
    options.analysis = binding_analysis_options_json_from_json_value(value)?;
    validate_resource_contract_ids(options.analysis.resources.as_ref())?;
    Ok(options)
}

fn reject_removed_host_theme(value: &Value) -> Result<(), BindingError> {
    if value
        .as_object()
        .is_some_and(|options| options.contains_key("host_theme"))
    {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "options group `host_theme` was removed; use `presentation.profile` for first-party profiles, `presentation.theme` for theme values, top-level `site_config` for Mermaid overrides, and `svg` for output policy",
        ));
    }
    Ok(())
}

fn reject_null_presentation_values(value: &Value) -> Result<(), BindingError> {
    let Some(presentation) = value.get("presentation") else {
        return Ok(());
    };
    let Some(presentation) = presentation.as_object() else {
        if presentation.is_null() {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                "options group `presentation` must be an object, not null",
            ));
        }
        return Ok(());
    };

    for key in ["profile", "theme"] {
        if presentation.get(key).is_some_and(Value::is_null) {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                format!("options field `presentation.{key}` must not be null"),
            ));
        }
    }
    if let Some(theme) = presentation.get("theme").and_then(Value::as_object) {
        for (key, value) in theme {
            if value.is_null() {
                return Err(BindingError::new(
                    BindingStatus::OptionsJsonError,
                    format!("options field `presentation.theme.{key}` must not be null"),
                ));
            }
        }
    }
    Ok(())
}

fn validate_options_schema_version(value: &Value) -> Result<(), BindingError> {
    let Some(raw_version) = value.get("version") else {
        return Ok(());
    };
    let Some(version) = raw_version
        .as_u64()
        .and_then(|version| u32::try_from(version).ok())
    else {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "options_json schema version must be an unsigned 32-bit integer",
        ));
    };
    if version != BINDING_OPTIONS_SCHEMA_VERSION {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            format!(
                "unsupported options_json schema version {version}; expected {BINDING_OPTIONS_SCHEMA_VERSION}"
            ),
        ));
    }
    Ok(())
}

fn reject_ambiguous_analysis_wrappers(value: &Value) -> Result<(), BindingError> {
    let Some(root) = value.as_object() else {
        return Ok(());
    };
    if root.contains_key("analysis") && root.contains_key("merman") {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "options JSON must not contain both `analysis` and `merman` wrappers",
        ));
    }
    Ok(())
}

fn reject_unknown_options_json_fields(value: &Value) -> Result<(), BindingError> {
    let Some(root) = value.as_object() else {
        return Ok(());
    };

    for (key, nested) in root {
        if matches!(key.as_str(), "analysis" | "merman") {
            let Some(wrapper) = nested.as_object() else {
                return Err(BindingError::new(
                    BindingStatus::OptionsJsonError,
                    format!("options JSON `{key}` wrapper must be an object"),
                ));
            };
            for nested_key in wrapper.keys() {
                if !BINDING_ANALYSIS_OPTION_KEYS.contains(&nested_key.as_str()) {
                    return Err(BindingError::new(
                        BindingStatus::OptionsJsonError,
                        format!(
                            "unknown options_json field `{key}.{nested_key}` for schema {BINDING_OPTIONS_SCHEMA_VERSION}"
                        ),
                    ));
                }
            }
            continue;
        }

        let known = key == "version"
            || key == "runtime_policy"
            || key == "parse"
            || BINDING_ANALYSIS_OPTION_KEYS.contains(&key.as_str())
            || matches!(
                key.as_str(),
                "presentation"
                    | "ascii"
                    | "layout"
                    | "environment"
                    | "svg"
                    | "raster"
                    | "jpeg"
                    | "pdf"
            );
        if !known {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                format!(
                    "unknown options_json field `{key}` for schema {BINDING_OPTIONS_SCHEMA_VERSION}"
                ),
            ));
        }
    }
    Ok(())
}

pub(crate) fn parse_request_overlay(
    request_options_json: &[u8],
    resource_scope: BindingResourceScope,
) -> Result<BindingRequestOverlay, BindingError> {
    let request_value = options_json_value(request_options_json)?;
    if is_unchanged_request(&request_value) {
        return Ok(BindingRequestOverlay::Unchanged);
    }

    validate_output_options_for_scope(&request_value, resource_scope)?;
    if request_selects_runtime_policy(&request_value) {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "request options_json cannot set runtime_policy; configure it when creating the engine",
        ));
    }
    parse_options_value(&request_value)?;

    let mut normalized_wire = normalize_analysis_wrapper(request_value);
    let requested_resources = take_request_resource_options(&mut normalized_wire, resource_scope)?;
    Ok(BindingRequestOverlay::Override {
        normalized_wire,
        requested_resources,
    })
}

impl BaseBindingOptions {
    pub(crate) fn validate_unchanged_request(&self) -> Result<(), BindingError> {
        parse_options_value(&self.normalized_wire).map(drop)
    }

    pub(crate) fn apply_overlay(
        &self,
        overlay: BindingRequestOverlay,
    ) -> Result<BindingOptions, BindingError> {
        let BindingRequestOverlay::Override {
            normalized_wire,
            requested_resources,
        } = overlay
        else {
            unreachable!("unchanged request overlays borrow the base engine");
        };

        let mut merged = self.normalized_wire.as_ref().clone();
        merge_json_value(&mut merged, normalized_wire);
        if let Some(requested_resources) = requested_resources {
            let resources = tighten_resource_options(&self.resource_ceiling, &requested_resources)?;
            merged
                .as_object_mut()
                .expect("validated binding options always normalize to an object")
                .insert(
                    "resources".to_string(),
                    serde_json::to_value(resources).map_err(internal_json_error)?,
                );
        }
        parse_options_value(&merged)
    }
}

fn is_unchanged_request(value: &Value) -> bool {
    let Some(options) = value.as_object() else {
        return false;
    };
    options.is_empty()
        || (options.len() == 1
            && options
                .get("version")
                .and_then(Value::as_u64)
                .is_some_and(|version| version == u64::from(BINDING_OPTIONS_SCHEMA_VERSION)))
}

fn reject_uncompiled_option_groups(value: &Value) -> Result<(), BindingError> {
    let Some(options) = value.as_object() else {
        return Ok(());
    };
    for (group, compiled) in [
        ("lint", cfg!(feature = "analysis")),
        ("ascii", cfg!(feature = "ascii")),
        ("presentation", cfg!(feature = "svg")),
        ("layout", cfg!(feature = "svg")),
        ("environment", cfg!(feature = "svg")),
        ("svg", cfg!(feature = "svg")),
        ("raster", cfg!(any(feature = "png", feature = "jpeg"))),
        ("jpeg", cfg!(feature = "jpeg")),
        ("pdf", cfg!(feature = "pdf")),
    ] {
        let present = options.contains_key(group)
            || (group == "lint"
                && ["analysis", "merman"].iter().any(|wrapper| {
                    options
                        .get(*wrapper)
                        .and_then(Value::as_object)
                        .is_some_and(|nested| nested.contains_key(group))
                }));
        if present && !compiled {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                format!("options group `{group}` is not available in this artifact"),
            ));
        }
    }
    Ok(())
}

fn validate_output_options_for_scope(
    value: &Value,
    scope: BindingResourceScope,
) -> Result<(), BindingError> {
    let Some(options) = value.as_object() else {
        return Ok(());
    };
    for group in ["raster", "jpeg", "pdf"] {
        let accepted = match group {
            "raster" => matches!(
                scope,
                BindingResourceScope::Png | BindingResourceScope::Jpeg
            ),
            "jpeg" => matches!(scope, BindingResourceScope::Jpeg),
            "pdf" => matches!(scope, BindingResourceScope::Pdf),
            _ => unreachable!("output option groups are closed"),
        };
        if options.contains_key(group) && !accepted {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                format!("request options group `{group}` does not apply to this operation"),
            ));
        }
    }
    Ok(())
}

fn validate_resource_contract_ids(
    resources: Option<&ResourceOptionsJson>,
) -> Result<(), BindingError> {
    let Some(resources) = resources else {
        return Ok(());
    };
    for (id, value) in &resources.limits {
        let Some(descriptor) = resource_limit_descriptor(id) else {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!(
                    "resource limit id `{id}` is not part of resource contract schema {BINDING_OPTIONS_SCHEMA_VERSION}"
                ),
            ));
        };
        if !descriptor.overridable {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!("resource limit id `{id}` is not overridable"),
            ));
        }
        if *value < descriptor.minimum_value {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!(
                    "resources.limits.{id} must be at least {}",
                    descriptor.minimum_value
                ),
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_one_shot_resource_options(
    options_json: &[u8],
    resource_scope: BindingResourceScope,
) -> Result<(), BindingError> {
    let value = options_json_value(options_json)?;
    validate_output_options_for_scope(&value, resource_scope)?;
    let mut value = normalize_analysis_wrapper(value);
    let _ = take_request_resource_options(&mut value, resource_scope)?;
    Ok(())
}

fn take_request_resource_options(
    value: &mut Value,
    resource_scope: BindingResourceScope,
) -> Result<Option<ResourceOptionsJson>, BindingError> {
    let root = value.as_object_mut().ok_or_else(|| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            "invalid request options_json: options root must be an object",
        )
    })?;
    let Some(resources) = root.remove("resources") else {
        return Ok(None);
    };
    if resources.is_null() {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "request options_json cannot clear the engine resource ceiling",
        ));
    }
    let resources = serde_json::from_value::<ResourceOptionsJson>(resources).map_err(|error| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid request options_json resources: {error}"),
        )
    })?;
    validate_resource_limits_for_scope(&resources, resource_scope)?;
    Ok(Some(resources))
}

fn validate_resource_limits_for_scope(
    resources: &ResourceOptionsJson,
    resource_scope: BindingResourceScope,
) -> Result<(), BindingError> {
    for id in resources.limits.keys() {
        if resource_limit_descriptor(id).is_none() || resource_scope.accepts(id) {
            continue;
        }
        return Err(BindingError::new(
            BindingStatus::InvalidArgument,
            format!(
                "resource limit id `{id}` is not available for the {} operation",
                resource_scope.description()
            ),
        ));
    }
    Ok(())
}

fn request_selects_runtime_policy(value: &Value) -> bool {
    value.as_object().is_some_and(|options| {
        options.contains_key("runtime_policy")
            || ["analysis", "merman"].iter().any(|wrapper| {
                options
                    .get(*wrapper)
                    .and_then(Value::as_object)
                    .is_some_and(|wrapped| wrapped.contains_key("runtime_policy"))
            })
    })
}

fn merge_json_value(base: &mut Value, request: Value) {
    match (base, request) {
        (Value::Object(base), Value::Object(request)) => {
            for (key, value) in request {
                match base.get_mut(&key) {
                    Some(base_value) => merge_json_value(base_value, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, request) => *base = request,
    }
}

fn normalize_analysis_wrapper(value: Value) -> Value {
    let Value::Object(mut options) = value else {
        return value;
    };

    let wrapper = ["merman", "analysis"].into_iter().find(|wrapper| {
        options
            .get(*wrapper)
            .and_then(Value::as_object)
            .is_some_and(binding_analysis_option_keys_present)
    });
    let Some(wrapper) = wrapper else {
        return Value::Object(options);
    };
    let Some(Value::Object(mut analysis)) = options.remove(wrapper) else {
        unreachable!("selected wrapper has an object value");
    };
    for key in BINDING_ANALYSIS_OPTION_KEYS {
        if let Some(value) = analysis.remove(key) {
            options.insert(key.to_string(), value);
        }
    }
    Value::Object(options)
}

#[cfg(feature = "svg")]
fn reject_removed_layout_fields(value: &Value) -> Result<(), BindingError> {
    let Some(layout) = value.get("layout").and_then(Value::as_object) else {
        return Ok(());
    };
    for (legacy, replacement) in [
        ("text_measurer", "environment.text_measurement"),
        ("math_renderer", "environment.math_renderer"),
        ("viewport_width", "layout.container_width"),
        ("viewport_height", "layout.container_height"),
    ] {
        if layout.contains_key(legacy) {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                format!("layout.{legacy} was removed; use {replacement}"),
            ));
        }
    }
    Ok(())
}

#[cfg(feature = "ascii")]
fn reject_removed_ascii_resource_field(value: &Value) -> Result<(), BindingError> {
    let Some(ascii) = value.get("ascii").and_then(Value::as_object) else {
        return Ok(());
    };
    if ascii.contains_key("max_grid_cells") || ascii.contains_key("maxGridCells") {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "ascii.max_grid_cells was removed; use resources.limits.max_ascii_grid_cells",
        ));
    }
    Ok(())
}

fn binding_analysis_options_json_from_json_value(
    value: &Value,
) -> Result<BindingAnalysisOptionsJson, BindingError> {
    reject_removed_nested_analysis_parse_option(value)?;
    let options_value = binding_analysis_options_root_value(value)?;
    serde_json::from_value(options_value.clone()).map_err(|err| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid analysis options JSON: {err}"),
        )
    })
}

fn binding_analysis_options_root_value(value: &Value) -> Result<&Value, BindingError> {
    let Value::Object(map) = value else {
        return Ok(value);
    };

    if binding_analysis_option_keys_present(map) {
        if ["merman", "analysis"]
            .iter()
            .any(|key| map.get(*key).is_some_and(Value::is_object))
        {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                "options JSON must not mix top-level analysis options with `analysis` or `merman` wrappers",
            ));
        }
        return Ok(value);
    }

    let mut wrapped_keys = ["merman", "analysis"].into_iter().filter(|key| {
        map.get(*key)
            .and_then(Value::as_object)
            .is_some_and(binding_analysis_option_keys_present)
    });
    if let Some(key) = wrapped_keys.next() {
        if wrapped_keys.next().is_some() {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                "options JSON must not contain both `merman` and `analysis` wrappers with analysis options",
            ));
        }
        return Ok(map
            .get(key)
            .expect("checked key existence and object shape"));
    }

    Ok(value)
}

fn binding_analysis_option_keys_present(map: &Map<String, Value>) -> bool {
    BINDING_ANALYSIS_OPTION_KEYS
        .iter()
        .any(|key| map.contains_key(*key))
}

fn reject_removed_nested_analysis_parse_option(value: &Value) -> Result<(), BindingError> {
    let Value::Object(map) = value else {
        return Ok(());
    };

    if ["merman", "analysis"].iter().any(|key| {
        map.get(*key)
            .and_then(Value::as_object)
            .is_some_and(|options| options.contains_key("parse"))
    }) {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "analysis option `parse` was removed; use top-level `parse` only for parse, render, or ASCII operations",
        ));
    }
    Ok(())
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

#[cfg(feature = "analysis")]
pub(crate) fn source_descriptor_for_uri(uri: &str) -> merman_analysis::SourceDescriptor {
    merman_analysis::source_descriptor_for_uri(uri)
}

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

pub(crate) fn binding_runtime_policy_from(
    options: &BindingOptions,
    mut runtime_policy: merman::runtime::RuntimePolicy,
) -> Result<merman::runtime::RuntimePolicy, BindingError> {
    let today = binding_fixed_today(options)?;
    let offset_minutes = binding_fixed_local_offset_minutes(options)?;
    if let Some(offset_minutes) = offset_minutes {
        runtime_policy = runtime_policy
            .try_with_fixed_local_offset_minutes(offset_minutes)
            .map_err(|err| BindingError::new(BindingStatus::InvalidArgument, err.to_string()))?;
    }

    if let Some(today) = today {
        runtime_policy = runtime_policy
            .try_with_fixed_today_at_local_midnight(today)
            .map_err(|err| BindingError::new(BindingStatus::InvalidArgument, err.to_string()))?;
    }

    Ok(runtime_policy)
}

pub(crate) fn runtime_policy_error(error: merman::runtime::RuntimePolicyError) -> BindingError {
    if let Some(capability) = error.missing_capability() {
        BindingError::missing_capability(capability.id(), error.to_string())
    } else {
        BindingError::new(BindingStatus::RenderError, error.to_string())
    }
}

pub(crate) fn selected_runtime_policy(
    options: &BindingOptions,
) -> Result<(BindingRuntimePolicy, merman::runtime::RuntimePolicy), BindingError> {
    let selection = options.runtime_policy.unwrap_or_default();
    let runtime_policy = match selection {
        BindingRuntimePolicy::Deterministic => merman::runtime::RuntimePolicy::deterministic(),
        BindingRuntimePolicy::Native => {
            merman::runtime::RuntimePolicy::try_native().map_err(runtime_policy_error)?
        }
    };
    Ok((selection, runtime_policy))
}

pub(crate) fn reject_selected_runtime_policy(
    options: &BindingOptions,
    constructor: &str,
) -> Result<(), BindingError> {
    if options.runtime_policy.is_some() {
        return Err(BindingError::new(
            BindingStatus::InvalidArgument,
            format!(
                "runtime_policy cannot be combined with the `{constructor}` engine constructor"
            ),
        ));
    }
    Ok(())
}

#[cfg(feature = "analysis")]
pub(crate) fn artifact_analysis_options(
    options: &BindingOptions,
) -> Result<merman_analysis::AnalysisOptions, BindingError> {
    let max_source_bytes = binding_input_resource_policy(options.analysis.resources.as_ref())?
        .value(merman::resources::InputResourceLimitId::MaxSourceBytes);
    let default_resources = ResourceOptionsJson::default();
    let max_document_diagrams = effective_resource_limits(
        options
            .analysis
            .resources
            .as_ref()
            .unwrap_or(&default_resources),
    )?[merman_analysis::MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID];
    let mut limits = BTreeMap::new();
    if let Some(max_source_bytes) = max_source_bytes {
        limits.insert("max_source_bytes".to_string(), max_source_bytes);
    }
    if let Some(max_document_diagrams) = max_document_diagrams {
        limits.insert(
            merman_analysis::MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID.to_string(),
            max_document_diagrams,
        );
    }

    let analysis = merman_analysis::AnalysisOptionsJson {
        fixed_today: options.analysis.fixed_today.clone(),
        fixed_local_offset_minutes: options.analysis.fixed_local_offset_minutes,
        site_config: options.analysis.site_config.clone(),
        resources: (!limits.is_empty()).then_some(merman_analysis::ResourceOptionsJson { limits }),
        lint: options.analysis.lint.clone(),
    };
    analysis
        .to_analysis_options()
        .map_err(|err| BindingError::new(BindingStatus::InvalidArgument, err.to_string()))
}

pub(crate) fn binding_input_resource_policy(
    resources: Option<&ResourceOptionsJson>,
) -> Result<merman::resources::InputResourcePolicy, BindingError> {
    let profile = resources
        .and_then(|resources| resources.profile.as_deref())
        .map(|id| {
            merman::resources::ResourceProfile::from_id(id).ok_or_else(|| {
                BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("unsupported resources.profile: {id}"),
                )
            })
        })
        .transpose()?
        .unwrap_or(merman::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE);
    let mut policy = merman::resources::InputResourcePolicy::for_profile(profile);
    if let Some(resources) = resources {
        for (id, value) in &resources.limits {
            if let Some(input_id) = merman::resources::InputResourceLimitId::from_stable_id(id) {
                policy.apply_limit(input_id, *value).map_err(|error| {
                    BindingError::new(BindingStatus::InvalidArgument, error.to_string())
                })?;
                continue;
            }
            if resource_limit_descriptor(id).is_some() {
                continue;
            }
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!("resource limit id `{id}` is not available for this artifact"),
            ));
        }
    }
    Ok(policy)
}

#[cfg(feature = "ascii")]
pub(crate) fn binding_ascii_grid_cells(
    resources: Option<&ResourceOptionsJson>,
) -> Result<usize, BindingError> {
    let default_resources = ResourceOptionsJson::default();
    let values = effective_resource_limits(resources.unwrap_or(&default_resources))?;
    Ok(values
        .get(merman::ascii::MAX_ASCII_GRID_CELLS_RESOURCE_LIMIT_ID)
        .copied()
        .flatten()
        .unwrap_or(usize::MAX))
}

#[cfg(feature = "svg")]
pub(crate) fn binding_resource_policy(
    resources: Option<&ResourceOptionsJson>,
) -> Result<merman::svg::RenderResourcePolicy, BindingError> {
    let profile = binding_resource_profile(resources)?;
    let mut limits = merman::svg::RenderResourcePolicy::for_profile(profile);
    if let Some(resources) = resources {
        for (id, value) in &resources.limits {
            if resource_limit_descriptor(id).is_some()
                && merman::svg::ResourceLimitId::from_stable_id(id).is_none()
            {
                continue;
            }
            limits.apply_override(id, *value).map_err(|error| {
                BindingError::new(BindingStatus::InvalidArgument, error.to_string())
            })?;
        }
    }
    Ok(limits)
}

#[cfg(any(feature = "svg", feature = "png", feature = "jpeg", feature = "pdf"))]
fn binding_resource_profile(
    resources: Option<&ResourceOptionsJson>,
) -> Result<merman::resources::ResourceProfile, BindingError> {
    Ok(resources
        .and_then(|resources| resources.profile.as_deref())
        .map(|id| {
            merman::resources::ResourceProfile::from_id(id).ok_or_else(|| {
                BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("unsupported resources.profile: {id}"),
                )
            })
        })
        .transpose()?
        .unwrap_or(merman::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE))
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub(crate) struct BindingExportResourceOptions {
    pub(crate) profile: merman::resources::ResourceProfile,
    #[cfg(any(feature = "png", feature = "jpeg"))]
    pub(crate) raster_size_limit: merman::svg::export::RasterSizeLimit,
    pub(crate) embedded_image_limit: merman::svg::export::EmbeddedImageLimit,
    #[cfg(feature = "pdf")]
    pub(crate) pdf_filter_image_limit: merman::svg::export::PdfFilterImageLimit,
    pub(crate) conversion_limits: merman::svg::export::SvgConversionLimits,
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub(crate) fn binding_export_resource_options(
    resources: Option<&ResourceOptionsJson>,
) -> Result<BindingExportResourceOptions, BindingError> {
    let default_resources = ResourceOptionsJson::default();
    let values = effective_resource_limits(resources.unwrap_or(&default_resources))?;
    let profile = binding_resource_profile(resources)?;

    Ok(BindingExportResourceOptions {
        profile,
        #[cfg(any(feature = "png", feature = "jpeg"))]
        raster_size_limit: merman::svg::export::RasterSizeLimit::new(
            export_resource_u32(
                &values,
                merman::svg::export::MAX_RASTER_WIDTH_RESOURCE_LIMIT_ID,
            )?,
            export_resource_u32(
                &values,
                merman::svg::export::MAX_RASTER_HEIGHT_RESOURCE_LIMIT_ID,
            )?,
            export_resource_u64(
                &values,
                merman::svg::export::MAX_RASTER_PIXELS_RESOURCE_LIMIT_ID,
            )?,
        ),
        embedded_image_limit: merman::svg::export::EmbeddedImageLimit::new(
            export_resource_u64(
                &values,
                merman::svg::export::MAX_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID,
            )?,
            export_resource_u64(
                &values,
                merman::svg::export::MAX_TOTAL_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID,
            )?,
            export_resource_u64(
                &values,
                merman::svg::export::MAX_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
            )?,
            export_resource_u64(
                &values,
                merman::svg::export::MAX_TOTAL_EMBEDDED_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
            )?,
        ),
        #[cfg(feature = "pdf")]
        pdf_filter_image_limit: merman::svg::export::PdfFilterImageLimit::new(export_resource_u64(
            &values,
            merman::svg::export::MAX_PDF_FILTER_IMAGE_PIXELS_RESOURCE_LIMIT_ID,
        )?),
        conversion_limits: merman::svg::export::SvgConversionLimits::default_safe(),
    })
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn export_resource_value(
    values: &BTreeMap<&'static str, Option<usize>>,
    stable_id: &'static str,
) -> Option<usize> {
    values
        .get(stable_id)
        .copied()
        .expect("compiled export resource descriptor must have a profile value")
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn export_resource_u32(
    values: &BTreeMap<&'static str, Option<usize>>,
    stable_id: &'static str,
) -> Result<Option<u32>, BindingError> {
    export_resource_value(values, stable_id)
        .map(|value| {
            u32::try_from(value).map_err(|_| {
                BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("resources.limits.{stable_id} exceeds the u32 export boundary"),
                )
            })
        })
        .transpose()
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn export_resource_u64(
    values: &BTreeMap<&'static str, Option<usize>>,
    stable_id: &'static str,
) -> Result<Option<u64>, BindingError> {
    export_resource_value(values, stable_id)
        .map(|value| {
            u64::try_from(value).map_err(|_| {
                BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("resources.limits.{stable_id} exceeds the u64 export boundary"),
                )
            })
        })
        .transpose()
}

pub fn resource_options_json(
    profile_id: Option<&str>,
    overrides: &[(&str, usize)],
) -> Result<Vec<u8>, BindingError> {
    let profile = profile_id
        .map(|profile_id| {
            merman::resources::ResourceProfile::from_id(profile_id).ok_or_else(|| {
                BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("unsupported resources.profile: {profile_id}"),
                )
            })
        })
        .transpose()?;
    let mut limits = BTreeMap::new();
    for &(id, value) in overrides {
        if limits.insert(id, value).is_some() {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!("duplicate resource limit override: {id}"),
            ));
        }
        let Some(descriptor) = resource_limit_descriptor(id) else {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!("resource limit id `{id}` is not available for this build"),
            ));
        };
        if !descriptor.overridable {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!("resource limit id `{id}` is not overridable"),
            ));
        }
        if value < descriptor.minimum_value {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!(
                    "resource limit `{id}` must be at least {}",
                    descriptor.minimum_value
                ),
            ));
        }
    }

    let mut root = Map::new();
    root.insert(
        "version".to_string(),
        Value::from(BINDING_OPTIONS_SCHEMA_VERSION),
    );
    if profile.is_some() || !limits.is_empty() {
        let mut resources = Map::new();
        if let Some(profile) = profile {
            resources.insert("profile".to_string(), Value::from(profile.id()));
        }
        if !limits.is_empty() {
            resources.insert(
                "limits".to_string(),
                serde_json::to_value(limits).map_err(internal_json_error)?,
            );
        }
        root.insert("resources".to_string(), Value::Object(resources));
    }
    serde_json::to_vec(&Value::Object(root)).map_err(internal_json_error)
}

/// Applies a transport-owned resource ceiling while preserving stricter caller limits.
///
/// The caller options may use direct analysis fields or exactly one `analysis`/`merman` wrapper.
/// A caller-selected profile is accepted only when its effective limits are no looser than the
/// transport ceiling. The returned JSON always names the ceiling profile and materializes any
/// stricter effective limits as explicit overrides.
pub fn apply_resource_ceiling_json(
    options_json: &[u8],
    ceiling_profile_id: &str,
    ceiling_overrides: &[(&str, usize)],
) -> Result<Vec<u8>, BindingError> {
    let ceiling_json = resource_options_json(Some(ceiling_profile_id), ceiling_overrides)?;
    let ceiling = parse_options(&ceiling_json)?
        .analysis
        .resources
        .ok_or_else(|| {
            BindingError::new(
                BindingStatus::InternalError,
                "generated resource ceiling omitted resources",
            )
        })?;
    let mut value = options_json_value(options_json)?;
    let root = value.as_object_mut().ok_or_else(|| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            "invalid options_json: options root must be an object",
        )
    })?;

    let wrappers = ["analysis", "merman"]
        .into_iter()
        .filter(|key| root.contains_key(*key))
        .collect::<Vec<_>>();
    for key in &wrappers {
        if !root.get(*key).is_some_and(Value::is_object) {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                format!("invalid options_json: `{key}` wrapper must be an object"),
            ));
        }
    }
    if wrappers.len() > 1 {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "options JSON must not contain both `analysis` and `merman` wrappers",
        ));
    }
    if !wrappers.is_empty() && root.contains_key("resources") {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "options JSON must not mix top-level resources with an `analysis` or `merman` wrapper",
        ));
    }

    let target = match wrappers.first() {
        Some(key) => root
            .get_mut(*key)
            .and_then(Value::as_object_mut)
            .expect("wrapper shape was validated"),
        None => root,
    };
    let requested = target
        .get("resources")
        .cloned()
        .map(|resources| {
            serde_json::from_value::<ResourceOptionsJson>(resources).map_err(|error| {
                BindingError::new(
                    BindingStatus::OptionsJsonError,
                    format!("invalid options_json resources: {error}"),
                )
            })
        })
        .transpose()?;
    let resources = requested
        .as_ref()
        .map(|requested| tighten_resource_options(&ceiling, requested))
        .transpose()?
        .unwrap_or(ceiling);
    target.insert(
        "resources".to_string(),
        serde_json::to_value(resources).map_err(internal_json_error)?,
    );

    let serialized = serde_json::to_vec(&value).map_err(internal_json_error)?;
    parse_options(&serialized)?;
    Ok(serialized)
}

fn options_json_value(options_json: &[u8]) -> Result<Value, BindingError> {
    if options_json.is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    let text = std::str::from_utf8(options_json).map_err(|error| {
        BindingError::new(
            BindingStatus::Utf8Error,
            format!("invalid options_json UTF-8: {error}"),
        )
    })?;
    serde_json::from_str(text).map_err(|error| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid options_json: {error}"),
        )
    })
}

fn tighten_resource_options(
    ceiling: &ResourceOptionsJson,
    requested: &ResourceOptionsJson,
) -> Result<ResourceOptionsJson, BindingError> {
    let mut candidate = if requested.profile.is_none() {
        ceiling.clone()
    } else {
        ResourceOptionsJson {
            profile: requested.profile.clone(),
            limits: BTreeMap::new(),
        }
    };
    candidate.limits.extend(requested.limits.clone());

    let ceiling_values = effective_resource_limits(ceiling)?;
    let candidate_values = effective_resource_limits(&candidate)?;
    let mut tightened = ceiling.clone();
    for (id, ceiling_value) in ceiling_values {
        let candidate_value = candidate_values
            .get(id)
            .copied()
            .expect("resource policy projections use the same stable IDs");
        match (ceiling_value, candidate_value) {
            (Some(maximum), Some(requested)) if requested <= maximum => {
                if requested < maximum {
                    tightened.limits.insert(id.to_string(), requested);
                }
            }
            (None, Some(requested)) => {
                tightened.limits.insert(id.to_string(), requested);
            }
            (None, None) => {}
            (Some(maximum), Some(requested)) => {
                return Err(resource_ceiling_error(id, requested.to_string(), maximum));
            }
            (Some(maximum), None) => {
                return Err(resource_ceiling_error(id, "unbounded".to_string(), maximum));
            }
        }
    }
    Ok(tightened)
}

fn resource_ceiling_error(id: &str, requested: String, maximum: usize) -> BindingError {
    BindingError::new(
        BindingStatus::OptionsJsonError,
        format!(
            "resources would loosen the transport ceiling for `{id}`: requested {requested}, maximum {maximum}"
        ),
    )
}

fn effective_resource_limits(
    resources: &ResourceOptionsJson,
) -> Result<BTreeMap<&'static str, Option<usize>>, BindingError> {
    let profile = resources
        .profile
        .as_deref()
        .map(|id| {
            merman::resources::ResourceProfile::from_id(id).ok_or_else(|| {
                BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("unsupported resources.profile: {id}"),
                )
            })
        })
        .transpose()?
        .unwrap_or(merman::resources::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE);
    validate_resource_contract_ids(Some(resources))?;

    let mut values = binding_resource_contract()
        .limits
        .into_iter()
        .map(|descriptor| {
            (
                descriptor.stable_id,
                resource_profile_value(profile, descriptor.stable_id)
                    .expect("compiled resource descriptors must have profile values"),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (id, value) in &resources.limits {
        let stable_id = resource_limit_descriptor(id)
            .expect("validated resource limit must have a compiled descriptor")
            .stable_id;
        values.insert(stable_id, Some(*value));
    }
    Ok(values)
}

pub fn render_resource_options_unavailable() -> BindingError {
    BindingError::new(
        BindingStatus::UnsupportedOperation,
        "resource options requires at least one resource-aware operation",
    )
}

#[cfg(feature = "analysis")]
impl From<merman_analysis::AnalysisOptionsJsonError> for BindingError {
    fn from(error: merman_analysis::AnalysisOptionsJsonError) -> Self {
        BindingError::new(BindingStatus::InvalidArgument, error.to_string())
    }
}

pub(crate) fn no_diagram_error() -> BindingError {
    BindingError::new(BindingStatus::NoDiagram, "no Mermaid diagram detected")
}

pub(crate) fn internal_json_error(err: serde_json::Error) -> BindingError {
    BindingError::new(
        BindingStatus::InternalError,
        format!("failed to serialize JSON output: {err}"),
    )
}

#[cfg(feature = "svg")]
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

#[cfg(feature = "svg")]
pub(crate) fn normalize_option(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(feature = "svg")]
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

#[allow(dead_code)]
pub(crate) fn feature_required_error(operation: &str, feature: &'static str) -> BindingError {
    BindingError::missing_capability(
        feature,
        format!("{operation} requires the {feature} feature"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn resolve_request_options(
        base: &[u8],
        request: &[u8],
        scope: BindingResourceScope,
    ) -> Result<BindingOptions, BindingError> {
        let (_, base) = parse_base_options(base)?;
        Ok(match parse_request_overlay(request, scope)? {
            BindingRequestOverlay::Unchanged => {
                base.validate_unchanged_request()?;
                parse_options_value(&base.normalized_wire)?
            }
            overlay => base.apply_overlay(overlay)?,
        })
    }

    #[test]
    fn error_payload_json_uses_public_code_names() {
        let payload = error_payload_json_bytes(BindingStatus::RenderError, "failed");
        let json: Value = serde_json::from_slice(&payload).unwrap();

        assert_eq!(json["code"], BindingStatus::RenderError.code());
        assert_eq!(json["code_name"], BindingStatus::RenderError.code_name());
        assert_eq!(json["kind"], "generic");
        assert!(json["capability_id"].is_null());
        assert_eq!(json["message"], "failed");
        assert!(json.get("details").is_none());

        let error = BindingError::missing_capability("layout-elk", "ELK is unavailable");
        let payload = binding_error_payload_json_bytes(&error);
        let json: Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(json["kind"], "missing-capability");
        assert_eq!(json["capability_id"], "layout-elk");

        let error = BindingError::resource_limit(
            "embedded_image_decode",
            "max_embedded_image_bytes",
            5,
            4,
            "constrained",
            "embedded image is too large",
        );
        let payload = binding_error_payload_json_bytes(&error);
        let json: Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(json["code_name"], "MERMAN_RESOURCE_LIMIT_EXCEEDED");
        assert_eq!(
            json["details"]["resource"]["limit_id"],
            "max_embedded_image_bytes"
        );
        assert_eq!(
            json["details"]["resource"]["phase"],
            "embedded_image_decode"
        );
        assert_eq!(json["details"]["resource"]["actual"], 5);
        assert_eq!(json["details"]["resource"]["max"], 4);
        assert_eq!(json["details"]["resource"]["profile"], "constrained");
    }

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn output_option_groups_follow_constructor_and_request_scopes() {
        parse_options(
            br#"{
                "raster":{"scale":2,"fit_to":{"width":640}},
                "jpeg":{"quality":85},
                "pdf":{"page_policy":{"kind":"fit-css-width","max_width_px":800}}
            }"#,
        )
        .expect("the reusable constructor accepts the compiled artifact union");

        let jpeg = resolve_request_options(
            b"",
            br#"{"raster":{"scale":2},"jpeg":{"quality":85}}"#,
            BindingResourceScope::Jpeg,
        )
        .expect("JPEG accepts shared raster and JPEG-specific options");
        assert_eq!(jpeg.jpeg.and_then(|options| options.quality), Some(85));

        for (scope, request, group) in [
            (
                BindingResourceScope::Png,
                br#"{"jpeg":{"quality":85}}"# as &[u8],
                "jpeg",
            ),
            (
                BindingResourceScope::Pdf,
                br#"{"raster":{"scale":2}}"#,
                "raster",
            ),
            (
                BindingResourceScope::Svg,
                br#"{"pdf":{"background":"white"}}"#,
                "pdf",
            ),
        ] {
            let error = resolve_request_options(b"", request, scope).unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
            assert!(error.message().contains(group), "{error:?}");
        }
    }

    #[cfg(not(any(feature = "png", feature = "jpeg", feature = "pdf")))]
    #[test]
    fn uncompiled_output_option_groups_are_rejected() {
        let error = parse_options(br#"{"raster":{"scale":2}}"#).unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("not available in this artifact"));
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

    #[test]
    fn request_options_deeply_override_engine_options() {
        let options = resolve_request_options(
            br#"{
                "version": 2,
                "parse": { "suppress_errors": false },
                "resources": {
                    "profile": "interactive",
                    "limits": {
                        "max_source_bytes": 4096,
                        "max_model_items": 128
                    }
                }
            }"#,
            br#"{
                "parse": { "suppress_errors": true },
                "resources": {
                    "limits": { "max_source_bytes": 2048 }
                }
            }"#,
            BindingResourceScope::Model,
        )
        .unwrap();
        assert_eq!(options.version, Some(2));
        assert_eq!(
            options.parse.and_then(|parse| parse.suppress_errors),
            Some(true)
        );
        let resources = options.analysis.resources.unwrap();
        assert_eq!(resources.profile.as_deref(), Some("interactive"));
        assert_eq!(resources.limits.get("max_source_bytes"), Some(&2048));
        assert_eq!(resources.limits.get("max_model_items"), Some(&128));
    }

    #[test]
    fn request_options_cannot_select_runtime_policy() {
        let error = resolve_request_options(
            br#"{"runtime_policy":"deterministic"}"#,
            br#"{"runtime_policy":"native"}"#,
            BindingResourceScope::Model,
        )
        .unwrap_err();

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("cannot set runtime_policy"));
    }

    #[test]
    fn wrapped_request_options_cannot_hide_runtime_policy() {
        for wrapper in ["analysis", "merman"] {
            let request = format!(r#"{{"{wrapper}":{{"runtime_policy":"native"}}}}"#);
            let error = resolve_request_options(
                b"",
                request.as_bytes(),
                BindingResourceScope::AnalysisDiagram,
            )
            .unwrap_err();

            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
            assert!(error.message().contains("cannot set runtime_policy"));
        }
    }

    #[test]
    fn request_options_normalize_wrapped_analysis_before_merging() {
        let options = resolve_request_options(
            br#"{"resources":{"limits":{"max_source_bytes":4096}}}"#,
            br#"{"analysis":{"fixed_today":"2026-08-01","resources":{"limits":{"max_source_bytes":2048}}}}"#,
            BindingResourceScope::AnalysisDiagram,
        )
        .unwrap();
        assert_eq!(
            options
                .analysis
                .resources
                .as_ref()
                .and_then(|resources| resources.limits.get("max_source_bytes")),
            Some(&2048)
        );
        assert_eq!(options.analysis.fixed_today.as_deref(), Some("2026-08-01"));
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn analysis_document_limit_tightens_without_entering_core_policy() {
        let base = br#"{
            "resources": {
                "profile": "interactive",
                "limits": {
                    "max_source_bytes": 4096,
                    "max_document_diagrams": 32
                }
            }
        }"#;
        let options = resolve_request_options(
            base,
            br#"{"resources":{"limits":{"max_document_diagrams":16}}}"#,
            BindingResourceScope::DocumentAnalysis,
        )
        .unwrap();
        let resources = options.analysis.resources.as_ref().unwrap();
        assert_eq!(
            resources
                .limits
                .get(merman_analysis::MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID),
            Some(&16)
        );
        assert_eq!(
            binding_input_resource_policy(Some(resources))
                .unwrap()
                .value(merman::resources::InputResourceLimitId::MaxSourceBytes),
            Some(4096)
        );

        #[cfg(feature = "analysis")]
        assert_eq!(
            artifact_analysis_options(&options)
                .unwrap()
                .max_document_diagrams(),
            Some(16)
        );

        let error = resolve_request_options(
            base,
            br#"{"resources":{"limits":{"max_document_diagrams":33}}}"#,
            BindingResourceScope::DocumentAnalysis,
        )
        .unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("max_document_diagrams"));
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn analysis_document_limit_is_not_available_to_single_diagram_requests() {
        let error = resolve_request_options(
            b"",
            br#"{"resources":{"limits":{"max_document_diagrams":16}}}"#,
            BindingResourceScope::AnalysisDiagram,
        )
        .unwrap_err();

        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("single-diagram analysis"));
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn analysis_document_limit_accepts_zero() {
        let options = resolve_request_options(
            br#"{"resources":{"profile":"interactive"}}"#,
            br#"{"resources":{"limits":{"max_document_diagrams":0}}}"#,
            BindingResourceScope::DocumentAnalysis,
        )
        .unwrap();

        assert_eq!(
            artifact_analysis_options(&options)
                .unwrap()
                .max_document_diagrams(),
            Some(0)
        );
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn analysis_document_limit_uses_the_selected_profile() {
        let options = parse_options(br#"{"resources":{"profile":"constrained"}}"#).unwrap();

        assert_eq!(
            artifact_analysis_options(&options)
                .unwrap()
                .max_document_diagrams(),
            Some(128)
        );
    }

    #[test]
    fn request_options_normalize_wrapped_engine_options_before_merging() {
        let options = resolve_request_options(
            br#"{"merman":{"resources":{"profile":"interactive","limits":{"max_source_bytes":4096}}}}"#,
            br#"{"resources":{"limits":{"max_source_bytes":2048,"max_model_items":128}}}"#,
            BindingResourceScope::Model,
        )
        .unwrap();
        let resources = options.analysis.resources.unwrap();
        assert_eq!(resources.profile.as_deref(), Some("interactive"));
        assert_eq!(resources.limits.get("max_source_bytes"), Some(&2048));
        assert_eq!(resources.limits.get("max_model_items"), Some(&128));
    }

    #[test]
    fn options_reject_ambiguous_analysis_wrappers_at_the_input_boundary() {
        for input in [
            br#"{"merman":{},"analysis":{}}"#.as_slice(),
            br#"{"merman":{"fixed_today":"2025-01-01"},"analysis":{}}"#.as_slice(),
            br#"{"merman":{},"analysis":{"fixed_today":"2025-01-01"}}"#.as_slice(),
        ] {
            let error = parse_options(input).expect_err("dual wrappers are ambiguous");
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
            assert!(
                error
                    .message()
                    .contains("must not contain both `analysis` and `merman` wrappers"),
                "unexpected error: {error:?}"
            );
        }
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn request_ascii_alias_spelling_collisions_remain_duplicate_field_errors() {
        for (base, request, field) in [
            (
                br#"{"ascii":{"default_direction":"leftToRight"}}"#.as_slice(),
                br#"{"ascii":{"defaultDirection":"topDown"}}"#.as_slice(),
                "default_direction",
            ),
            (
                br#"{"ascii":{"colorMode":"none"}}"#.as_slice(),
                br#"{"ascii":{"color_mode":"ansi"}}"#.as_slice(),
                "color_mode",
            ),
            (
                br#"{"ascii":{"colorMode":null}}"#.as_slice(),
                br#"{"ascii":{"color_mode":"none"}}"#.as_slice(),
                "color_mode",
            ),
        ] {
            let error = resolve_request_options(base, request, BindingResourceScope::Model)
                .expect_err("different wire spellings survive recursive object merge");

            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
            assert!(
                error
                    .message()
                    .contains(&format!("duplicate field `{field}`")),
                "field={field}, error={error:?}"
            );
        }

        let options = resolve_request_options(
            br#"{"ascii":{"colorMode":null}}"#,
            br#"{"ascii":{"colorMode":"ansi"}}"#,
            BindingResourceScope::Model,
        )
        .expect("the same raw key replaces its null base value");
        assert_eq!(
            options
                .ascii
                .as_ref()
                .and_then(|ascii| ascii.color_mode.as_deref()),
            Some("ansi")
        );
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn request_ascii_alias_collision_uses_merged_wire_order() {
        let base = br#"{
            "ascii": {
                "default_direction": "leftToRight",
                "color_mode": "none"
            }
        }"#;

        for (request, first_duplicate) in [
            (
                br#"{"ascii":{"colorMode":"ansi","defaultDirection":"topDown"}}"#.as_slice(),
                "color_mode",
            ),
            (
                br#"{"ascii":{"defaultDirection":"topDown","colorMode":"ansi"}}"#.as_slice(),
                "default_direction",
            ),
        ] {
            let error = resolve_request_options(base, request, BindingResourceScope::Model)
                .expect_err("both aliases collide after merge");

            assert!(
                error
                    .message()
                    .contains(&format!("duplicate field `{first_duplicate}`")),
                "expected first duplicate {first_duplicate}, error={error:?}"
            );
        }
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn request_ascii_nested_theme_alias_error_precedes_later_top_level_alias_error() {
        let error = resolve_request_options(
            br##"{
                "ascii": {
                    "color_mode": "none",
                    "theme": {
                        "foreground": "#ffffff",
                        "background": "#000000"
                    }
                }
            }"##,
            br##"{
                "ascii": {
                    "colorMode": "ansi",
                    "theme": { "fg": "#eeeeee" }
                }
            }"##,
            BindingResourceScope::Model,
        )
        .expect_err("theme and top-level aliases both collide");

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(
            error.message().contains("duplicate field `foreground`"),
            "the in-place theme object is visited before the appended colorMode key: {error:?}"
        );
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn request_resource_ceiling_error_precedes_cross_document_alias_error() {
        let error = resolve_request_options(
            br#"{
                "resources": {
                    "profile": "constrained",
                    "limits": { "max_source_bytes": 64 }
                },
                "ascii": { "color_mode": "none" }
            }"#,
            br#"{
                "resources": { "limits": { "max_source_bytes": 65 } },
                "ascii": { "colorMode": "ansi" }
            }"#,
            BindingResourceScope::Model,
        )
        .expect_err("resource tightening runs before merged options are reparsed");

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(
            error.message().contains("loosen the transport ceiling"),
            "unexpected error: {error:?}"
        );
        assert!(!error.message().contains("duplicate field"));
    }

    #[cfg(any(feature = "svg", feature = "ascii"))]
    #[test]
    fn parse_options_accepts_analysis_wrapper_without_dropping_binding_options() {
        let options = parse_options(
            br#"{
                "parse": { "suppress_errors": true },
                "analysis": {
                    "resources": { "limits": { "max_source_bytes": 4 } }
                },
                "version": 2,
                "svg": { "pipeline": "resvg-safe" }
            }"#,
        )
        .unwrap();

        assert_eq!(options.version, Some(2));
        assert_eq!(
            options
                .parse
                .as_ref()
                .and_then(|parse| parse.suppress_errors),
            Some(true)
        );
        assert_eq!(
            options
                .analysis
                .resources
                .as_ref()
                .and_then(|resources| resources.limits.get("max_source_bytes")),
            Some(&4)
        );
        #[cfg(feature = "svg")]
        assert_eq!(
            binding_resource_policy(options.analysis.resources.as_ref())
                .unwrap()
                .value(merman::svg::ResourceLimitId::MaxSourceBytes),
            Some(4)
        );
        #[cfg(feature = "svg")]
        assert_eq!(
            options.svg.as_ref().and_then(|svg| svg.pipeline.as_deref()),
            Some("resvg-safe")
        );
    }

    #[test]
    fn options_schema_v2_rejects_legacy_versions_and_unknown_fields() {
        let version = parse_options(br#"{ "version": 1 }"#).unwrap_err();
        assert_eq!(version.status(), BindingStatus::OptionsJsonError);
        assert!(version.message().contains("expected 2"));

        #[cfg(feature = "svg")]
        {
            let legacy = parse_options(br#"{ "version": 1, "layout": { "viewport_width": 640 } }"#)
                .unwrap_err();
            assert!(legacy.message().contains("expected 2"));
        }
        #[cfg(feature = "ascii")]
        {
            let legacy =
                parse_options(br#"{ "version": 1, "ascii": { "maxGridCells": 42 } }"#).unwrap_err();
            assert!(legacy.message().contains("expected 2"));
        }

        for input in [
            br#"{ "version": 2, "tyop": true }"#.as_slice(),
            br#"{ "version": 2, "analysis": { "tyop": true } }"#.as_slice(),
            br#"{ "version": 2, "parse": { "tyop": true } }"#.as_slice(),
        ] {
            let error = parse_options(input).unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
            assert!(error.message().contains("unknown"), "{error:?}");
        }

        #[cfg(feature = "analysis")]
        for input in [
            br#"{ "version": 2, "lint": { "profiel": "strict" } }"#.as_slice(),
            br#"{ "version": 2, "lint": { "rule_severities": [{ "rule_id": "merman.authoring.flowchart.explicit_direction", "severity": "warning", "tyop": true }] } }"#.as_slice(),
        ] {
            let error = parse_options(input).unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
            assert!(error.message().contains("unknown"), "{error:?}");
        }

        for input in [
            br#"{ "version": "2" }"#.as_slice(),
            br#"{ "version": 2.0 }"#.as_slice(),
            br#"{ "version": -1 }"#.as_slice(),
            br#"{ "version": 4294967296 }"#.as_slice(),
        ] {
            let error = parse_options(input).unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
            assert!(error.message().contains("unsigned 32-bit integer"));
        }

        let flat = parse_options(br#"{ "resources": { "max_source_bytes": 4 } }"#).unwrap_err();
        assert_eq!(flat.status(), BindingStatus::OptionsJsonError);
        assert!(flat.message().contains("unknown field `max_source_bytes`"));
    }

    #[test]
    fn options_schema_v2_rejects_groups_not_compiled_into_the_artifact() {
        for (group, compiled, input) in [
            (
                "lint",
                cfg!(feature = "analysis"),
                br#"{"version":2,"lint":{"profile":"strict"}}"#.as_slice(),
            ),
            (
                "lint",
                cfg!(feature = "analysis"),
                br#"{"version":2,"analysis":{"lint":{"profile":"strict"}}}"#.as_slice(),
            ),
            (
                "ascii",
                cfg!(feature = "ascii"),
                br#"{"version":2,"ascii":{}}"#.as_slice(),
            ),
            (
                "presentation",
                cfg!(feature = "svg"),
                br#"{"version":2,"presentation":{}}"#.as_slice(),
            ),
            (
                "layout",
                cfg!(feature = "svg"),
                br#"{"version":2,"layout":{}}"#.as_slice(),
            ),
            (
                "environment",
                cfg!(feature = "svg"),
                br#"{"version":2,"environment":{}}"#.as_slice(),
            ),
            (
                "svg",
                cfg!(feature = "svg"),
                br#"{"version":2,"svg":{}}"#.as_slice(),
            ),
            (
                "raster",
                cfg!(any(feature = "png", feature = "jpeg")),
                br#"{"version":2,"raster":{}}"#.as_slice(),
            ),
            (
                "jpeg",
                cfg!(feature = "jpeg"),
                br#"{"version":2,"jpeg":{}}"#.as_slice(),
            ),
            (
                "pdf",
                cfg!(feature = "pdf"),
                br#"{"version":2,"pdf":{}}"#.as_slice(),
            ),
        ] {
            if compiled {
                continue;
            }
            let error = parse_options(input).unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
            assert!(error.message().contains(group), "{error:?}");
            assert!(error.message().contains("not available"), "{error:?}");
        }
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn resource_ceiling_preserves_stricter_limits_and_wrapper_shape() {
        for wrapper in [None, Some("analysis"), Some("merman")] {
            let input = match wrapper {
                Some(wrapper) => format!(
                    r#"{{"{wrapper}":{{"site_config":{{"theme":"dark"}},"resources":{{"limits":{{"max_source_bytes":4096,"max_document_diagrams":64}}}}}}}}"#
                ),
                None => {
                    r#"{"parse":{"suppress_errors":true},"resources":{"limits":{"max_source_bytes":4096,"max_document_diagrams":64}}}"#
                        .to_string()
                }
            };
            let constrained =
                apply_resource_ceiling_json(input.as_bytes(), "constrained", &[]).unwrap();
            let value: Value = serde_json::from_slice(&constrained).unwrap();
            let resources = wrapper
                .map(|wrapper| &value[wrapper]["resources"])
                .unwrap_or(&value["resources"]);

            assert_eq!(resources["profile"], "constrained");
            assert_eq!(resources["limits"]["max_source_bytes"], 4096);
            assert_eq!(resources["limits"]["max_document_diagrams"], 64);
            assert!(value.get("resources").is_some() == wrapper.is_none());
            parse_options(&constrained).unwrap();
        }
    }

    #[test]
    fn resource_ceiling_rejects_looser_profiles_and_overrides() {
        for input in [
            br#"{"resources":{"profile":"trusted-native"}}"#.as_slice(),
            br#"{"resources":{"limits":{"max_source_bytes":2097152}}}"#.as_slice(),
        ] {
            let error = apply_resource_ceiling_json(input, "constrained", &[]).unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
            assert!(error.message().contains("loosen the transport ceiling"));
        }
    }

    #[test]
    fn resource_ceiling_rejects_ambiguous_or_malformed_wrappers() {
        for input in [
            br#"{"analysis":{},"merman":{}}"#.as_slice(),
            br#"{"analysis":[]}"#.as_slice(),
            br#"{"merman":null}"#.as_slice(),
            br#"{"resources":{"profile":"constrained"},"analysis":{}}"#.as_slice(),
        ] {
            let error = apply_resource_ceiling_json(input, "constrained", &[]).unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        }
    }

    #[cfg(feature = "svg")]
    #[test]
    fn resource_options_builder_uses_the_descriptor_and_rejects_invalid_overrides() {
        let json = resource_options_json(
            Some("constrained"),
            &[("max_source_bytes", 4096), ("max_svg_bytes", 8192)],
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(value["version"], BINDING_OPTIONS_SCHEMA_VERSION);
        assert_eq!(value["resources"]["profile"], "constrained");
        assert_eq!(value["resources"]["limits"]["max_source_bytes"], 4096);

        for error in [
            resource_options_json(Some("missing"), &[]).unwrap_err(),
            resource_options_json(Some("interactive"), &[("unknown_limit", 1)]).unwrap_err(),
            resource_options_json(
                Some("interactive"),
                &[("max_source_bytes", 1), ("max_source_bytes", 2)],
            )
            .unwrap_err(),
        ] {
            assert_eq!(error.status(), BindingStatus::InvalidArgument);
        }
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn export_backend_hard_caps_cannot_be_overridden() {
        let hard_cap = merman::svg::export::MAX_SVG_CONVERSION_ISOLATION_DEPTH_RESOURCE_LIMIT_ID;
        for error in [
            resource_options_json(Some("interactive"), &[(hard_cap, 1)]).unwrap_err(),
            parse_options(format!(r#"{{"resources":{{"limits":{{"{hard_cap}":1}}}}}}"#).as_bytes())
                .unwrap_err(),
        ] {
            assert_eq!(error.status(), BindingStatus::InvalidArgument);
            assert!(error.message().contains("not overridable"));
        }
    }

    #[cfg(all(
        not(feature = "svg"),
        not(feature = "analysis"),
        not(feature = "ascii")
    ))]
    #[test]
    fn semantic_only_resource_options_accept_all_semantic_limits() {
        let json = resource_options_json(
            Some("constrained"),
            &[("max_source_bytes", 4096), ("max_model_items", 1)],
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(value["resources"]["limits"]["max_source_bytes"], 4096);
        assert_eq!(value["resources"]["limits"]["max_model_items"], 1);
    }

    #[cfg(all(feature = "analysis", not(feature = "svg"), not(feature = "ascii")))]
    #[test]
    fn analysis_artifact_resource_options_include_semantic_model_limits() {
        let json = resource_options_json(
            Some("constrained"),
            &[("max_source_bytes", 4096), ("max_model_items", 1)],
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(value["resources"]["limits"]["max_source_bytes"], 4096);
        assert_eq!(value["resources"]["limits"]["max_model_items"], 1);
    }

    #[cfg(all(
        not(feature = "svg"),
        not(feature = "analysis"),
        not(feature = "ascii")
    ))]
    #[test]
    fn semantic_only_build_accepts_top_level_parse_options() {
        let options = parse_options(br#"{ "parse": { "suppress_errors": true } }"#).unwrap();
        assert_eq!(
            options
                .parse
                .as_ref()
                .and_then(|parse| parse.suppress_errors),
            Some(true)
        );
    }

    #[test]
    fn parse_options_rejects_removed_nested_analysis_parse_option() {
        for wrapper in ["analysis", "merman"] {
            let input =
                format!(r#"{{ "{wrapper}": {{ "parse": {{ "suppress_errors": true }} }} }}"#);
            let err = parse_options(input.as_bytes()).unwrap_err();
            assert_eq!(err.status(), BindingStatus::OptionsJsonError);
            assert!(
                err.message()
                    .contains("analysis option `parse` was removed"),
                "unexpected error: {err:?}"
            );
        }
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn parse_options_accepts_merman_wrapper() {
        let options = parse_options(
            br#"{
                "merman": {
                    "lint": {
                        "profile": "recommended"
                    }
                }
            }"#,
        )
        .unwrap();

        assert_eq!(
            options
                .analysis
                .lint
                .as_ref()
                .and_then(|lint| lint.profile.as_deref()),
            Some("recommended")
        );
    }

    #[test]
    fn parse_options_rejects_mixed_direct_and_wrapped_analysis_options() {
        let err = parse_options(
            br#"{
                "resources": { "limits": { "max_source_bytes": 4 } },
                "analysis": {
                    "resources": { "limits": { "max_model_items": 4 } }
                }
            }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::OptionsJsonError);
        assert!(
            err.message()
                .contains("must not mix top-level analysis options"),
            "{err:?}"
        );
    }
}

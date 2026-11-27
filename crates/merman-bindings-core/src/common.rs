use serde::{Deserialize, Serialize};
#[cfg(feature = "analysis")]
use std::collections::BTreeMap;
use std::str::FromStr;

mod error;
mod options;
mod options_value;
mod options_wire;
mod resources;

pub use error::{
    BindingCancellationErrorDetails, BindingDiagnosticErrorDetails, BindingDiagnosticSpan,
    BindingError, BindingErrorBody, BindingErrorKind, BindingIconRegistryErrorDetails,
    BindingJsSafeResourceErrorDetails, BindingResourceErrorDetails, BindingResourceLimitCause,
    BindingStatus, binding_error_details_json, binding_error_js_details_json,
    binding_error_js_payload_json_bytes, binding_error_payload_json_bytes,
    error_payload_json_bytes,
};
#[cfg(feature = "svg")]
pub(crate) use options::LightSvgOverride;
#[cfg(test)]
pub(crate) use options::parse_options;
pub(crate) use options::{
    BaseBindingOptions, BindingRequestOverlay, parse_base_options_for_artifact,
    parse_request_overlay_for_artifact, validate_one_shot_resource_options,
};
use options_value::OptionsValue;
#[cfg(feature = "ascii")]
pub(crate) use options_wire::AsciiOptionsJson;
pub(crate) use options_wire::BindingOptions;
#[cfg(feature = "pdf")]
pub(crate) use options_wire::PdfPageOptionsJson;
#[cfg(feature = "analysis")]
use options_wire::ResourceOptionsJson;
#[cfg(feature = "analysis")]
pub use resources::analysis_options_with_resource_ceiling;
#[cfg(feature = "ascii")]
pub(crate) use resources::binding_ascii_resource_policy;
#[cfg(feature = "svg")]
pub(crate) use resources::binding_resource_policy;
#[cfg(feature = "analysis")]
use resources::effective_resource_limits;
#[cfg(feature = "svg")]
pub(crate) use resources::enforce_compact_options_budget;
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
pub(crate) use resources::{BindingExportResourceOptions, binding_export_resource_options};
pub use resources::{
    apply_resource_ceiling_json, apply_resource_ceiling_json_with_allowed_overrides,
    enforce_options_json_byte_budget, enforce_options_json_byte_len,
    render_resource_options_unavailable, resource_options_json,
};
pub(crate) use resources::{
    binding_input_resource_policy, parse_one_shot_options_with_resource_ceiling,
};

/// Current schema for constructor and request options JSON.
///
/// Version 4 uses Theme v2 selections, with separate graphical background and export paint options.
/// An omitted version selects the current grammar. SDK integrations can send the version explicitly.
pub const BINDING_OPTIONS_SCHEMA_VERSION: u32 = 4;
pub const BINDING_RESULT_PAYLOAD_VERSION: u32 = 1;
const BINDING_JSON_SAFE_INTEGER_MAX: u64 = crate::RUNTIME_CATALOG_MAX_SAFE_INTEGER;

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

pub(crate) fn parse_error(error: impl Into<merman::TerminalDiagnostic>) -> BindingError {
    let error = error.into();
    let message = error.terminal_safe_message();
    let status = match error.class() {
        merman::TerminalDiagnosticClass::Parse => BindingStatus::ParseError,
        merman::TerminalDiagnosticClass::ResourceLimit => BindingStatus::ResourceLimitExceeded,
        merman::TerminalDiagnosticClass::Internal => BindingStatus::InternalError,
        merman::TerminalDiagnosticClass::Cancelled => BindingStatus::Cancelled,
        merman::TerminalDiagnosticClass::RuntimePolicy => BindingStatus::RenderError,
        _ => BindingStatus::InternalError,
    };
    BindingError::new(status, message).with_diagnostic_details(binding_diagnostic_details(
        error.terminal_diagnostic_details(),
    ))
}

pub(crate) fn binding_diagnostic_details(
    details: merman::TerminalDiagnosticDetails,
) -> BindingDiagnosticErrorDetails {
    let span = details.span.and_then(|span| {
        Some(BindingDiagnosticSpan {
            start: u64::try_from(span.start).ok()?,
            end: u64::try_from(span.end).ok()?,
            kind: match details.span_kind? {
                merman::ParseDiagnosticSpanKind::Exact => "exact",
                merman::ParseDiagnosticSpanKind::InsertionPoint => "insertion-point",
                merman::ParseDiagnosticSpanKind::Fallback => "fallback",
            },
        })
    });
    BindingDiagnosticErrorDetails {
        code: details.code,
        span,
        field: details.field,
        diagram_type: details.diagram_type,
        requested_max_width: details
            .requested_max_width
            .and_then(|value| u64::try_from(value).ok()),
        actual_width: details
            .actual_width
            .and_then(|value| u64::try_from(value).ok()),
        width_profile: details.width_profile,
        fallback_reason: details.fallback_reason,
    }
}

pub(crate) fn input_resource_limit_error(
    error: merman::model::resources::InputResourceLimitExceeded,
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

pub(crate) fn core_error(error: merman::Error) -> BindingError {
    match error {
        merman::Error::OperationCancelled(error) => BindingError::cancelled(error),
        merman::Error::RuntimePolicy(error) => runtime_policy_error(error),
        merman::Error::ThemeEvaluationLimit(error) => {
            BindingError::new(BindingStatus::ResourceLimitExceeded, error.to_string())
        }
        error => parse_error(error),
    }
}

#[cfg(feature = "svg")]
pub(crate) fn icon_registry_error_status(
    kind: merman::svg::IconRegistryBuildErrorKind,
) -> BindingStatus {
    match kind {
        merman::svg::IconRegistryBuildErrorKind::AllocationFailed
        | merman::svg::IconRegistryBuildErrorKind::ArithmeticOverflow => {
            BindingStatus::InternalError
        }
        merman::svg::IconRegistryBuildErrorKind::InvalidUtf8 => BindingStatus::Utf8Error,
        merman::svg::IconRegistryBuildErrorKind::ResourceLimitExceeded => {
            BindingStatus::ResourceLimitExceeded
        }
        _ => BindingStatus::InvalidArgument,
    }
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

/// An options document decoded during transport admission.
#[doc(hidden)]
pub struct DecodedOperationOptions<'a> {
    pub(crate) json: &'a str,
    value: OptionsValue,
}

impl<'a> DecodedOperationOptions<'a> {
    /// Retains the original JSON for consumers that diagnose embedded documents.
    pub fn decode(
        json: &'a str,
        invalid_json: impl FnOnce(serde_json::Error) -> BindingError,
    ) -> Result<Self, BindingError> {
        enforce_options_json_byte_len(json.len())?;
        let value = options_value::decode_options(json).map_err(invalid_json)?;
        Ok(Self { json, value })
    }
}

pub(crate) enum BindingRequestOptions<'a> {
    Raw(&'a [u8]),
    Decoded(DecodedOperationOptions<'a>),
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
) -> Result<Option<merman::model::time::CivilDate>, BindingError> {
    let Some(today) = options.analysis.fixed_today.as_deref() else {
        return Ok(None);
    };
    merman::model::time::CivilDate::from_str(today)
        .map(Some)
        .map_err(|_| {
            BindingError::new(
                BindingStatus::InvalidArgument,
                "fixed_today must be a canonical civil date such as YYYY-MM-DD or +10000-MM-DD",
            )
        })
}

pub(crate) fn binding_fixed_local_offset_minutes(
    options: &BindingOptions,
) -> Result<Option<i32>, BindingError> {
    let Some(offset_minutes) = options.analysis.fixed_local_offset_minutes else {
        return Ok(None);
    };
    let valid = merman::model::time::UtcOffset::from_minutes(offset_minutes).is_some();
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
    mut runtime_policy: merman::model::runtime::RuntimePolicy,
) -> Result<merman::model::runtime::RuntimePolicy, BindingError> {
    let today = binding_fixed_today(options)?;
    let unix_millis = options.analysis.fixed_unix_millis;
    if let Some(unix_millis) = unix_millis {
        const MAX_FIXED_UNIX_MILLIS: i64 = 8_640_000_000_000_000;
        if !(-MAX_FIXED_UNIX_MILLIS..=MAX_FIXED_UNIX_MILLIS).contains(&unix_millis) {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                "fixed_unix_millis must be between -8640000000000000 and 8640000000000000",
            ));
        }
        if today.is_some() {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                "fixed_today and fixed_unix_millis must not both be set",
            ));
        }
        runtime_policy = runtime_policy
            .with_fixed_today(None)
            .with_fixed_unix_millis(unix_millis);
    }
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

pub(crate) fn runtime_policy_error(
    error: impl Into<merman::TerminalRuntimePolicyError>,
) -> BindingError {
    let error = error.into();
    let safe_message = error.terminal_safe_message();
    if let Some(capability) = error.missing_capability() {
        BindingError::missing_capability(capability.id(), safe_message).with_diagnostic_details(
            binding_diagnostic_details(error.terminal_diagnostic_details()),
        )
    } else {
        BindingError::new(BindingStatus::RenderError, safe_message).with_diagnostic_details(
            binding_diagnostic_details(error.terminal_diagnostic_details()),
        )
    }
}

pub(crate) fn selected_runtime_policy(
    options: &BindingOptions,
) -> Result<(BindingRuntimePolicy, merman::model::runtime::RuntimePolicy), BindingError> {
    let selection = options.runtime_policy.unwrap_or_default();
    let runtime_policy = match selection {
        BindingRuntimePolicy::Deterministic => {
            merman::model::runtime::RuntimePolicy::deterministic()
        }
        BindingRuntimePolicy::Native => {
            merman::model::runtime::RuntimePolicy::try_native().map_err(runtime_policy_error)?
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
        .value(merman::model::resources::InputResourceLimitId::MaxSourceBytes);
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
        fixed_unix_millis: options.analysis.fixed_unix_millis,
        fixed_local_offset_minutes: options.analysis.fixed_local_offset_minutes,
        site_config: options.analysis.site_config.clone(),
        resources: (!limits.is_empty()).then_some(merman_analysis::ResourceOptionsJson { limits }),
        lint: options.analysis.lint.clone(),
    };
    analysis
        .to_analysis_options()
        .map_err(|err| BindingError::new(BindingStatus::InvalidArgument, err.to_string()))
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
mod tests;

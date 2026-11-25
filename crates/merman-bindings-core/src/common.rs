use serde::Deserialize;
use serde::Serialize;
use serde_json::{Map, Value};
#[cfg(feature = "analysis")]
use std::collections::BTreeMap;
use std::str::FromStr;
use std::sync::Arc;

use crate::artifact_contract::{DEFAULT_ARTIFACT_SNAPSHOT, ValidatedArtifactContract};
use crate::option_contract::BindingOptionGroupKey;
use crate::resource_contract::{
    BindingResourceLimitDescriptor, BindingResourceScope, resource_limit_descriptor,
};

mod error;
mod options_value;
mod resources;

#[cfg(test)]
use crate::resource_contract::{BINDING_OPTIONS_JSON_LIMIT_ID, BINDING_OPTIONS_JSON_MAX_BYTES};
use options_value::{OptionsValue, OptionsValueRef};
#[cfg(feature = "analysis")]
pub use resources::analysis_options_with_resource_ceiling;
#[cfg(feature = "ascii")]
pub(crate) use resources::binding_ascii_resource_policy;
#[cfg(feature = "svg")]
pub(crate) use resources::binding_resource_policy;
#[cfg(any(feature = "analysis", test))]
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
use resources::{options_json_value, tighten_resource_options};

pub use error::{
    BindingCancellationErrorDetails, BindingDiagnosticErrorDetails, BindingDiagnosticSpan,
    BindingError, BindingErrorBody, BindingErrorKind, BindingIconRegistryErrorDetails,
    BindingJsSafeResourceErrorDetails, BindingResourceErrorDetails, BindingResourceLimitCause,
    BindingStatus, binding_error_details_json, binding_error_js_details_json,
    binding_error_js_payload_json_bytes, binding_error_payload_json_bytes,
    error_payload_json_bytes,
};

/// Current schema for constructor and request options JSON.
///
/// Version 4 uses Theme v2 selections, with separate graphical background and export paint options.
/// An omitted version selects the current grammar. SDK integrations can send the version explicitly.
pub const BINDING_OPTIONS_SCHEMA_VERSION: u32 = 4;
pub const BINDING_RESULT_PAYLOAD_VERSION: u32 = 1;
const BINDING_JSON_SAFE_INTEGER_MAX: u64 = crate::RUNTIME_CATALOG_MAX_SAFE_INTEGER;
const BINDING_ANALYSIS_OPTION_KEYS: [&str; 6] = [
    "fixed_today",
    "fixed_unix_millis",
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

mod options_wire;

#[cfg(feature = "ascii")]
pub(crate) use options_wire::AsciiOptionsJson;
pub(crate) use options_wire::BindingOptions;
#[cfg(feature = "pdf")]
pub(crate) use options_wire::PdfPageOptionsJson;
use options_wire::{BindingAnalysisOptionsJson, ResourceOptionsJson};

#[derive(Debug, Clone)]
pub(crate) struct BaseBindingOptions {
    normalized_wire: Arc<OptionsValue>,
    resource_ceiling: ResourceOptionsJson,
    #[cfg(feature = "svg")]
    has_analysis_wrapper: bool,
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

#[cfg(any(feature = "svg", feature = "ascii"))]
#[derive(Debug)]
pub(crate) enum BindingThemeOverlay {
    Inherit,
    Replace(Option<merman::theme::ThemeSelection>),
}

#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "The overlay keeps the validated request payload and resource policy together."
)]
pub(crate) enum BindingRequestOverlay {
    Unchanged,
    Override {
        normalized_wire: OptionsValue,
        requested_resources: Option<ResourceOptionsJson>,
        #[cfg(any(feature = "svg", feature = "ascii"))]
        theme: BindingThemeOverlay,
    },
}

#[cfg(feature = "svg")]
pub(crate) struct LightSvgOverride {
    pub(crate) diagram_id: Option<String>,
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

#[cfg(test)]
pub(crate) fn parse_options(bytes: &[u8]) -> Result<BindingOptions, BindingError> {
    let wire = options_json_value(bytes)?;
    parse_options_with_theme(bytes, wire.view(), &DEFAULT_ARTIFACT_SNAPSHOT)
}

#[cfg(test)]
pub(crate) fn parse_base_options(
    bytes: &[u8],
) -> Result<(BindingOptions, BaseBindingOptions), BindingError> {
    parse_base_options_for_contract(bytes, &DEFAULT_ARTIFACT_SNAPSHOT)
}

pub(crate) fn parse_base_options_for_artifact(
    bytes: &[u8],
    artifact_contract: &ValidatedArtifactContract,
) -> Result<(BindingOptions, BaseBindingOptions), BindingError> {
    parse_base_options_for_contract(bytes, artifact_contract)
}

fn parse_base_options_for_contract(
    bytes: &[u8],
    artifact_contract: &ValidatedArtifactContract,
) -> Result<(BindingOptions, BaseBindingOptions), BindingError> {
    let wire = options_json_value(bytes)?;
    let typed = parse_options_with_theme(bytes, wire.view(), artifact_contract)?;
    let base = base_options_from_wire(wire, &typed);
    Ok((typed, base))
}

fn base_options_from_wire(wire: OptionsValue, typed: &BindingOptions) -> BaseBindingOptions {
    let resource_ceiling = typed.analysis.resources.clone().unwrap_or_default();
    // Constructor decoding and overlay merging can see different fields from a wrapper.
    #[cfg(feature = "svg")]
    let has_analysis_wrapper =
        wire.value.get("analysis").is_some() || wire.value.get("merman").is_some();
    let normalized_wire = normalize_analysis_wrapper(wire);
    #[cfg(any(feature = "svg", feature = "ascii"))]
    let normalized_wire = {
        let mut normalized_wire = normalized_wire;
        normalized_wire
            .object_mut()
            .expect("validated binding options always normalize to an object")
            .swap_remove("theme");
        normalized_wire
    };
    BaseBindingOptions {
        normalized_wire: Arc::new(normalized_wire),
        resource_ceiling,
        #[cfg(feature = "svg")]
        has_analysis_wrapper,
    }
}

fn parse_options_with_theme(
    bytes: &[u8],
    wire: OptionsValueRef<'_>,
    artifact: &ValidatedArtifactContract,
) -> Result<BindingOptions, BindingError> {
    let options = parse_options_value_for_contract(wire, artifact)?;
    #[cfg(any(feature = "svg", feature = "ascii"))]
    let options = BindingOptions {
        theme: crate::native_theme::binding_theme(bytes, wire.value)?,
        ..options
    };
    #[cfg(not(any(feature = "svg", feature = "ascii")))]
    let _ = bytes;
    Ok(options)
}

fn parse_options_value(value: &Value) -> Result<BindingOptions, BindingError> {
    parse_options_value_for_contract(value.into(), &DEFAULT_ARTIFACT_SNAPSHOT)
}

fn parse_options_value_for_contract(
    wire: OptionsValueRef<'_>,
    artifact_contract: &ValidatedArtifactContract,
) -> Result<BindingOptions, BindingError> {
    let value = wire.value;
    validate_options_schema_version(value)?;
    reject_ambiguous_analysis_wrappers(value)?;
    reject_removed_host_theme(value)?;
    reject_removed_presentation(value)?;
    reject_unavailable_option_groups(value, artifact_contract)?;
    #[cfg(feature = "svg")]
    reject_removed_layout_fields(value)?;
    reject_untrusted_binding_css_options(wire)?;
    #[cfg(feature = "ascii")]
    reject_removed_ascii_resource_field(value)?;
    reject_removed_nested_analysis_parse_option(value)?;
    let options = if value.is_object() {
        let entries = wire.entries().filter(|(key, _)| {
            !matches!(*key, "analysis" | "merman" | "theme")
                && !BINDING_ANALYSIS_OPTION_KEYS.contains(key)
        });
        BindingOptions::deserialize(serde::de::value::MapDeserializer::new(entries))
    } else {
        BindingOptions::deserialize(wire)
    };
    let mut options = options.map_err(|err| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid options_json: {err}"),
        )
    })?;
    options.version = Some(BINDING_OPTIONS_SCHEMA_VERSION);
    #[cfg(feature = "svg")]
    {
        options.text_measurement_selector_explicit = value
            .as_object()
            .and_then(|root| root.get("environment"))
            .and_then(Value::as_object)
            .is_some_and(|environment| environment.contains_key("text_measurement"));
    }
    reject_unknown_options_json_fields(wire)?;
    options.analysis = binding_analysis_options_json_from_json_value(wire)?;
    validate_artifact_resource_options(options.analysis.resources.as_ref(), artifact_contract)?;
    Ok(options)
}

fn reject_removed_host_theme(value: &Value) -> Result<(), BindingError> {
    if value
        .as_object()
        .is_some_and(|options| options.contains_key("host_theme"))
    {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "options group `host_theme` was removed; use top-level `theme` for compiled diagram themes, `site_config` for Mermaid behavior overrides, and `svg` for output policy",
        ));
    }
    Ok(())
}

fn reject_removed_presentation(value: &Value) -> Result<(), BindingError> {
    if value
        .as_object()
        .is_some_and(|options| options.contains_key("presentation"))
    {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "options group `presentation` was removed; use top-level `theme`, `site_config`, layout configuration, and `svg` as independent owners",
        ));
    }
    Ok(())
}

fn reject_untrusted_binding_css_options(wire: OptionsValueRef<'_>) -> Result<(), BindingError> {
    let (options_value, _) = binding_analysis_options_root_value(wire)?;
    if let Some(site_config) = options_value
        .get("site_config")
        .filter(|site_config| site_config.value.is_object())
    {
        if site_config.get("themeCSS").is_some() {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                "site_config.themeCSS is not accepted by general bindings; use the typed `theme` schema instead, or let a trusted Rust or native CLI host own raw CSS explicitly",
            ));
        }
        if site_config.get("secure").is_some() {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                "site_config.secure is not accepted by general bindings because callers cannot replace the host security boundary; configure it only through a trusted Rust or native CLI host",
            ));
        }
        for field in ["maxEdges", "max_edges"] {
            if site_config.get(field).is_some() {
                return Err(BindingError::new(
                    BindingStatus::OptionsJsonError,
                    format!(
                        "site_config.{field} is not accepted by general bindings because it can raise the parser edge budget beyond the host resource ceiling"
                    ),
                ));
            }
        }
        for field in [
            "securityLevel",
            "security_level",
            "dompurifyConfig",
            "dompurify_config",
        ] {
            if site_config.get(field).is_some() {
                return Err(BindingError::new(
                    BindingStatus::OptionsJsonError,
                    format!(
                        "site_config.{field} is not accepted by general bindings because it can replace the host SVG security policy"
                    ),
                ));
            }
        }
        for field in ["themeVariables", "theme_variables"] {
            if let Some(theme_variables) = site_config.get(field) {
                let path = format!("site_config.{field}");
                reject_untrusted_theme_variable_values(theme_variables, &path)?;
            }
        }
    }

    #[cfg(feature = "svg")]
    let Some(svg) = wire
        .value
        .as_object()
        .and_then(|options| options.get("svg"))
        .and_then(Value::as_object)
    else {
        return Ok(());
    };

    #[cfg(feature = "svg")]
    for field in [
        "scoped_css",
        "scopedCss",
        "css_override_policy",
        "cssOverridePolicy",
    ] {
        if svg.contains_key(field) {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                format!(
                    "svg.{field} is not accepted by general bindings; host-owned CSS controls are restricted to a trusted Rust or native CLI host, so use the typed `theme` schema or `svg.root_background_color` instead"
                ),
            ));
        }
    }

    Ok(())
}

fn reject_untrusted_theme_variable_values(
    wire: OptionsValueRef<'_>,
    path: &str,
) -> Result<(), BindingError> {
    match wire.value {
        Value::String(value) if !is_safe_binding_css_value(value) => Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            format!(
                "{path} contains a CSS value that is not accepted by general bindings; use a typed theme value or a trusted Rust or native CLI host"
            ),
        )),
        Value::Array(values) => wire
            .items(values)
            .enumerate()
            .try_for_each(|(index, value)| {
                reject_untrusted_theme_variable_values(value, &format!("{path}[{index}]"))
            }),
        Value::Object(_) => wire.entries().try_for_each(|(key, value)| {
            reject_untrusted_theme_variable_values(value, &format!("{path}.{key}"))
        }),
        _ => Ok(()),
    }
}

fn is_safe_binding_css_value(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    !value.bytes().any(|byte| {
        byte.is_ascii_control() || matches!(byte, b'<' | b'>' | b'{' | b'}' | b';' | b'\\')
    }) && !value.contains("/*")
        && !value.contains("*/")
        && !lower.contains("url(")
        && !lower.contains("expression(")
        && !lower.contains("@import")
        && !lower.contains("<!--")
        && !lower.contains("-->")
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

fn reject_unknown_options_json_fields(wire: OptionsValueRef<'_>) -> Result<(), BindingError> {
    for (key, nested) in wire.entries() {
        if matches!(key, "analysis" | "merman") {
            if !nested.value.is_object() {
                return Err(BindingError::new(
                    BindingStatus::OptionsJsonError,
                    format!("options JSON `{key}` wrapper must be an object"),
                ));
            }
            // The wrapped analysis root is forward-compatible. Its owner validates known fields
            // and ignores future fields; the binding layer must not maintain a second key table.
            continue;
        }

        if BindingOptionGroupKey::from_id(key).is_none() {
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

#[cfg(test)]
pub(crate) fn parse_request_overlay(
    request_options_json: &[u8],
    resource_scope: BindingResourceScope,
) -> Result<BindingRequestOverlay, BindingError> {
    parse_request_overlay_for_artifact(
        BindingRequestOptions::Raw(request_options_json),
        resource_scope,
        &DEFAULT_ARTIFACT_SNAPSHOT,
    )
}

pub(crate) fn parse_request_overlay_for_artifact(
    request_options: BindingRequestOptions<'_>,
    resource_scope: BindingResourceScope,
    artifact_contract: &ValidatedArtifactContract,
) -> Result<BindingRequestOverlay, BindingError> {
    let (request_options_json, request_value) = match request_options {
        BindingRequestOptions::Raw(json) => (json, options_json_value(json)?),
        BindingRequestOptions::Decoded(options) => (options.json.as_bytes(), options.value),
    };
    if is_unchanged_request(&request_value.value) {
        return Ok(BindingRequestOverlay::Unchanged);
    }

    validate_output_options_for_scope(&request_value.value, resource_scope)?;
    if request_selects_runtime_policy(&request_value.value) {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "request options_json cannot set runtime_policy; configure it when creating the engine",
        ));
    }
    let options = parse_options_with_theme(
        request_options_json,
        request_value.view(),
        artifact_contract,
    )?;
    let mut normalized_wire = normalize_analysis_wrapper(request_value);
    #[cfg(any(feature = "svg", feature = "ascii"))]
    let theme = if normalized_wire
        .object_mut()
        .expect("validated options normalize to an object")
        .swap_remove("theme")
        .is_some()
    {
        BindingThemeOverlay::Replace(options.theme)
    } else {
        BindingThemeOverlay::Inherit
    };
    #[cfg(not(any(feature = "svg", feature = "ascii")))]
    let _ = options;
    let resources = normalized_wire
        .object_mut()
        .expect("validated options normalize to an object")
        .swap_remove("resources");
    let requested_resources =
        parse_request_resource_options(resources.as_ref().map(OptionsValue::view), resource_scope)?;
    Ok(BindingRequestOverlay::Override {
        normalized_wire,
        requested_resources,
        #[cfg(any(feature = "svg", feature = "ascii"))]
        theme,
    })
}

#[cfg(any(feature = "svg", feature = "ascii"))]
impl BindingRequestOverlay {
    #[cfg(feature = "svg")]
    pub(crate) fn take_light_svg_override(&mut self) -> Option<LightSvgOverride> {
        let Self::Override {
            normalized_wire,
            requested_resources: None,
            theme: BindingThemeOverlay::Inherit,
        } = self
        else {
            return None;
        };
        let options = normalized_wire.value.as_object_mut()?;
        if options.len() != 1 + usize::from(options.contains_key("version")) {
            return None;
        }
        let svg = options.get_mut("svg")?.as_object_mut()?;
        if svg.len() != 1 {
            return None;
        }
        // The typed request was validated before constructing the overlay.
        let diagram_id = match svg.get_mut("diagram_id")?.take() {
            Value::String(id) => Some(id),
            Value::Null => None,
            _ => unreachable!("validated svg.diagram_id is a string or null"),
        };
        Some(LightSvgOverride { diagram_id })
    }

    pub(crate) fn select_theme(
        &self,
        base_theme: Option<&merman::theme::ThemeSelection>,
    ) -> Option<merman::theme::ThemeSelection> {
        match self {
            Self::Override {
                theme: BindingThemeOverlay::Replace(theme),
                ..
            } => theme.clone(),
            _ => base_theme.cloned(),
        }
    }

    #[cfg(feature = "svg")]
    pub(crate) fn validate_prepared_output(&self) -> Result<(), BindingError> {
        if let Self::Override {
            normalized_wire, ..
        } = self
        {
            let disallowed = normalized_wire.view().entries().find(|(key, _)| {
                !matches!(
                    *key,
                    "version" | "background" | "environment" | "layout" | "svg"
                )
            });
            if let Some((key, _)) = disallowed {
                return Err(BindingError::invalid_options_json(format!(
                    "prepared output cannot set `{key}`. Prepare a new source to change parse configuration"
                )));
            }
        }
        Ok(())
    }
}

impl BaseBindingOptions {
    #[cfg(feature = "svg")]
    pub(crate) const fn has_analysis_wrapper(&self) -> bool {
        self.has_analysis_wrapper
    }

    #[cfg(feature = "svg")]
    pub(crate) fn prepared_output_defaults(
        &self,
        artifact: &ValidatedArtifactContract,
    ) -> Result<(BindingOptions, Self), BindingError> {
        let mut normalized = self.normalized_wire.as_ref().clone();
        normalized
            .object_mut()
            .expect("base options are normalized objects")
            .retain(|key| {
                matches!(
                    key,
                    "version" | "resources" | "background" | "environment" | "layout" | "svg"
                )
            });
        let options = parse_options_value_for_contract(normalized.view(), artifact)?;
        Ok((
            options,
            Self {
                normalized_wire: Arc::new(normalized),
                resource_ceiling: self.resource_ceiling.clone(),
                has_analysis_wrapper: self.has_analysis_wrapper,
            },
        ))
    }

    pub(crate) fn validate_unchanged_request(
        &self,
        artifact_contract: &ValidatedArtifactContract,
    ) -> Result<(), BindingError> {
        parse_options_value_for_contract(self.normalized_wire.view(), artifact_contract).map(drop)
    }

    pub(crate) fn apply_overlay(
        &self,
        overlay: BindingRequestOverlay,
        artifact_contract: &ValidatedArtifactContract,
    ) -> Result<BindingOptions, BindingError> {
        let BindingRequestOverlay::Override {
            normalized_wire,
            requested_resources,
            ..
        } = overlay
        else {
            unreachable!("unchanged request overlays borrow the base engine");
        };

        let mut merged = self.normalized_wire.as_ref().clone();
        merged.merge(normalized_wire);
        if let Some(requested_resources) = requested_resources {
            let resources = tighten_resource_options(&self.resource_ceiling, &requested_resources)?;
            merged
                .object_mut()
                .expect("validated binding options always normalize to an object")
                .insert(
                    "resources".to_string(),
                    serde_json::to_value(resources)
                        .map_err(internal_json_error)?
                        .into(),
                );
        }
        parse_options_value_for_contract(merged.view(), artifact_contract)
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

fn reject_unavailable_option_groups(
    value: &Value,
    artifact_contract: &ValidatedArtifactContract,
) -> Result<(), BindingError> {
    let Some(options) = value.as_object() else {
        return Ok(());
    };
    for key in BindingOptionGroupKey::ALL {
        let group = key.id();
        let present = options.contains_key(group)
            || (group == "lint"
                && ["analysis", "merman"].iter().any(|wrapper| {
                    options
                        .get(*wrapper)
                        .and_then(Value::as_object)
                        .is_some_and(|nested| nested.contains_key(group))
                }));
        let available = artifact_contract.exposes_option_group(*key);
        if present && !available {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                format!(
                    "options group `{group}` is not available because it is not exposed by target `{}`",
                    artifact_contract.target().id()
                ),
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
    for group in ["export_matte", "raster", "jpeg", "pdf"] {
        let accepted = match group {
            "export_matte" => matches!(
                scope,
                BindingResourceScope::Png | BindingResourceScope::Jpeg | BindingResourceScope::Pdf
            ),
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

fn validate_artifact_resource_options(
    resources: Option<&ResourceOptionsJson>,
    artifact_contract: &ValidatedArtifactContract,
) -> Result<(), BindingError> {
    let Some(resources) = resources else {
        return Ok(());
    };
    for (id, value) in &resources.limits {
        let descriptor = compiled_resource_limit_descriptor(id)?;
        if !artifact_contract.exposes_resource_limit(&descriptor) {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!(
                    "resource limit id `{id}` is not exposed by target `{}`",
                    artifact_contract.target().id()
                ),
            ));
        }
        validate_resource_limit_override(id, *value, descriptor)?;
    }
    Ok(())
}

fn validate_compiled_resource_options(
    resources: Option<&ResourceOptionsJson>,
) -> Result<(), BindingError> {
    let Some(resources) = resources else {
        return Ok(());
    };
    for (id, value) in &resources.limits {
        let descriptor = compiled_resource_limit_descriptor(id)?;
        validate_resource_limit_override(id, *value, descriptor)?;
    }
    Ok(())
}

fn compiled_resource_limit_descriptor(
    id: &str,
) -> Result<BindingResourceLimitDescriptor, BindingError> {
    resource_limit_descriptor(id).ok_or_else(|| {
        BindingError::new(
            BindingStatus::InvalidArgument,
            format!(
                "resource limit id `{id}` is not part of resource contract schema {BINDING_OPTIONS_SCHEMA_VERSION}"
            ),
        )
    })
}

fn validate_resource_limit_override(
    id: &str,
    value: usize,
    descriptor: BindingResourceLimitDescriptor,
) -> Result<(), BindingError> {
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
                "resources.limits.{id} must be at least {}",
                descriptor.minimum_value
            ),
        ));
    }
    Ok(())
}

pub(crate) fn validate_one_shot_resource_options(
    options_json: &[u8],
    resource_scope: BindingResourceScope,
) -> Result<(), BindingError> {
    let wire = options_json_value(options_json)?;
    validate_output_options_for_scope(&wire.value, resource_scope)?;
    if !wire.value.is_object() {
        return Err(BindingError::invalid_options_json(
            "invalid request options_json: options root must be an object",
        ));
    }
    // Preflight observes the same resource field as wrapper normalization. Full schema and
    // wrapper validation still follows operation-scoped resource validation.
    let root = wire.view();
    let resources = normalization_wrapper(&wire.value)
        .and_then(|wrapper| root.get(wrapper)?.get("resources"))
        .or_else(|| root.get("resources"));
    parse_request_resource_options(resources, resource_scope).map(drop)
}

fn parse_request_resource_options(
    resources: Option<OptionsValueRef<'_>>,
    resource_scope: BindingResourceScope,
) -> Result<Option<ResourceOptionsJson>, BindingError> {
    let Some(resources) = resources else {
        return Ok(None);
    };
    if resources.value.is_null() {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "request options_json cannot clear the engine resource ceiling",
        ));
    }
    let resources = ResourceOptionsJson::deserialize(resources).map_err(|error| {
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

fn normalization_wrapper(value: &Value) -> Option<&'static str> {
    ["merman", "analysis"]
        .into_iter()
        .find(|wrapper| value.get(*wrapper).is_some_and(Value::is_object))
}

fn normalize_analysis_wrapper(mut value: OptionsValue) -> OptionsValue {
    let Some(wrapper) = normalization_wrapper(&value.value) else {
        return value;
    };
    let mut options = value
        .object_mut()
        .expect("options with an analysis wrapper");
    let mut analysis = options.swap_remove(wrapper).expect("selected wrapper");
    let mut analysis = analysis
        .object_mut()
        .expect("selected wrapper is an object");
    for key in BINDING_ANALYSIS_OPTION_KEYS {
        if let Some(value) = analysis.swap_remove(key) {
            options.insert(key.to_string(), value);
        }
    }
    value
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

#[cfg(feature = "analysis")]
fn binding_analysis_options_json_from_json_value(
    wire: OptionsValueRef<'_>,
) -> Result<BindingAnalysisOptionsJson, BindingError> {
    reject_removed_nested_analysis_parse_option(wire.value)?;
    let (options_value, wrapped) = binding_analysis_options_root_value(wire)?;
    let resources = options_value
        .get("resources")
        .filter(|value| !value.value.is_null())
        .map(|value| {
            ResourceOptionsJson::deserialize(value).map_err(|err| {
                BindingError::new(
                    BindingStatus::OptionsJsonError,
                    format!("invalid analysis options JSON: {err}"),
                )
            })
        })
        .transpose()?;

    let mut analysis_value = options_value.to_ordered();
    let mut analysis = analysis_value.object_mut().ok_or_else(|| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            "invalid analysis options JSON: analysis options JSON must be an object",
        )
    })?;
    if !wrapped {
        analysis.retain(|key| BINDING_ANALYSIS_OPTION_KEYS.contains(&key));
    }
    let analysis_contract = merman_analysis::AnalysisConfigContract::current();
    if let Some(mut resource_options) = analysis.child_object("resources") {
        resource_options.swap_remove("profile");
        if let Some(mut limits) = resource_options.child_object("limits") {
            limits.retain(|limit_id| analysis_contract.accepts_resource_limit(limit_id));
        }
    }
    let decoded = analysis_contract
        .decode_ordered_json(analysis_value.view())
        .map_err(BindingError::from)?;
    Ok(BindingAnalysisOptionsJson {
        fixed_today: decoded.fixed_today,
        fixed_unix_millis: decoded.fixed_unix_millis,
        fixed_local_offset_minutes: decoded.fixed_local_offset_minutes,
        site_config: decoded.site_config,
        resources,
        lint: decoded.lint,
    })
}

#[cfg(not(feature = "analysis"))]
fn binding_analysis_options_json_from_json_value(
    wire: OptionsValueRef<'_>,
) -> Result<BindingAnalysisOptionsJson, BindingError> {
    reject_removed_nested_analysis_parse_option(wire.value)?;
    let (options_value, _) = binding_analysis_options_root_value(wire)?;
    BindingAnalysisOptionsJson::deserialize(options_value).map_err(|err| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid analysis options JSON: {err}"),
        )
    })
}

fn binding_analysis_options_root_value(
    wire: OptionsValueRef<'_>,
) -> Result<(OptionsValueRef<'_>, bool), BindingError> {
    let Value::Object(map) = wire.value else {
        return Ok((wire, false));
    };

    let wrapper = ["merman", "analysis"]
        .into_iter()
        .find(|key| map.contains_key(*key));
    if binding_analysis_option_keys_present(map) {
        if wrapper.is_some() {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                "options JSON must not mix top-level analysis options with `analysis` or `merman` wrappers",
            ));
        }
        return Ok((wire, false));
    }

    if let Some(key) = wrapper {
        let wrapped = wire
            .get(key)
            .expect("selected wrapper key must exist in the options object");
        if !wrapped.value.is_object() {
            return Err(BindingError::new(
                BindingStatus::OptionsJsonError,
                format!("options JSON `{key}` wrapper must be an object"),
            ));
        }
        return Ok((wrapped, true));
    }

    Ok((wire, false))
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

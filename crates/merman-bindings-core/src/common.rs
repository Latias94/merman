use serde::Deserialize;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub const BINDING_OPTIONS_SCHEMA_VERSION: u32 = 1;
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
    message: String,
}

impl BindingError {
    pub fn new(status: BindingStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            kind: BindingErrorKind::Generic,
            capability_id: None,
            message: message.into(),
        }
    }

    pub fn unknown_operation(message: impl Into<String>) -> Self {
        Self {
            status: BindingStatus::UnsupportedOperation,
            kind: BindingErrorKind::UnknownOperation,
            capability_id: None,
            message: message.into(),
        }
    }

    pub fn missing_capability(capability_id: &'static str, message: impl Into<String>) -> Self {
        Self {
            status: BindingStatus::UnsupportedOperation,
            kind: BindingErrorKind::MissingCapability,
            capability_id: Some(capability_id),
            message: message.into(),
        }
    }

    pub fn reentrant_call(message: impl Into<String>) -> Self {
        Self {
            status: BindingStatus::InvalidArgument,
            kind: BindingErrorKind::ReentrantCall,
            capability_id: None,
            message: message.into(),
        }
    }

    pub fn busy(message: impl Into<String>) -> Self {
        Self {
            status: BindingStatus::Busy,
            kind: BindingErrorKind::Busy,
            capability_id: None,
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
    kind: &'a str,
    capability_id: Option<&'a str>,
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
    pub(crate) runtime_policy: Option<BindingRuntimePolicy>,
    #[serde(flatten)]
    pub(crate) analysis: BindingAnalysisOptionsJson,
    pub(crate) parse: Option<ParseOptionsJson>,
    #[cfg(feature = "svg")]
    pub(crate) host_theme: Option<HostThemeOptionsJson>,
    #[cfg(feature = "ascii")]
    pub(crate) ascii: Option<AsciiOptionsJson>,
    #[cfg(feature = "svg")]
    pub(crate) layout: Option<LayoutOptionsJson>,
    #[cfg(feature = "svg")]
    pub(crate) environment: Option<RenderEnvironmentOptionsJson>,
    #[cfg(feature = "svg")]
    pub(crate) svg: Option<SvgOptionsJson>,
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

#[cfg(feature = "ascii")]
#[derive(Debug, Default, Deserialize)]
pub(crate) struct AsciiOptionsJson {
    pub(crate) charset: Option<String>,
    #[serde(default, alias = "defaultDirection")]
    pub(crate) default_direction: Option<String>,
    #[serde(default, alias = "colorMode")]
    pub(crate) color_mode: Option<String>,
    pub(crate) theme: Option<AsciiThemeOptionsJson>,
    #[serde(default, alias = "sequenceMirrorActors")]
    pub(crate) sequence_mirror_actors: Option<bool>,
    #[serde(default, alias = "xychartVerticalPlotHeight")]
    pub(crate) xychart_vertical_plot_height: Option<usize>,
    #[serde(default, alias = "xychartCategoryBandWidth")]
    pub(crate) xychart_category_band_width: Option<usize>,
    #[serde(default, alias = "xychartHorizontalPlotWidth")]
    pub(crate) xychart_horizontal_plot_width: Option<usize>,
    #[serde(default, alias = "maxGridCells")]
    pub(crate) max_grid_cells: Option<usize>,
    #[serde(default, alias = "relationSummaryDiagnostics")]
    pub(crate) relation_summary_diagnostics: Option<bool>,
}

#[cfg(feature = "ascii")]
#[derive(Debug, Default, Deserialize)]
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
pub(crate) struct SvgOptionsJson {
    pub(crate) diagram_id: Option<String>,
    pub(crate) pipeline: Option<String>,
    pub(crate) scoped_css: Option<String>,
    pub(crate) css_override_policy: Option<String>,
    pub(crate) root_background_color: Option<String>,
    pub(crate) drop_native_duplicate_fallbacks: Option<bool>,
}

#[cfg(feature = "svg")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HostThemeOptionsJson {
    pub(crate) preset: Option<String>,
    pub(crate) appearance: Option<String>,
    pub(crate) font_family: Option<String>,
    pub(crate) font_size: Option<String>,
    pub(crate) roles: Option<HostThemeRolesJson>,
    pub(crate) series_palette: Option<Vec<String>>,
    pub(crate) output: Option<HostThemeOutputJson>,
    pub(crate) theme_variables: Option<serde_json::Map<String, serde_json::Value>>,
    pub(crate) site_config: Option<serde_json::Value>,
}

#[cfg(feature = "svg")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
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

#[cfg(feature = "svg")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HostThemeOutputJson {
    pub(crate) pipeline: Option<String>,
    pub(crate) css_override_policy: Option<String>,
    pub(crate) root_background: Option<String>,
    pub(crate) drop_native_duplicate_fallbacks: Option<bool>,
    pub(crate) scoped_css: Option<String>,
}

pub fn error_payload_json_bytes(status: BindingStatus, message: &str) -> Vec<u8> {
    error_payload_json_bytes_with_details(status, BindingErrorKind::Generic, None, message)
}

pub fn binding_error_payload_json_bytes(error: &BindingError) -> Vec<u8> {
    error_payload_json_bytes_with_details(
        error.status(),
        error.kind(),
        error.capability_id(),
        error.message(),
    )
}

fn error_payload_json_bytes_with_details(
    status: BindingStatus,
    kind: BindingErrorKind,
    capability_id: Option<&str>,
    message: &str,
) -> Vec<u8> {
    let payload = ErrorPayload {
        version: BINDING_RESULT_PAYLOAD_VERSION,
        ok: false,
        code: status.code(),
        code_name: status.code_name(),
        kind: kind.id(),
        capability_id,
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
    if bytes.is_empty() {
        return Ok(BindingOptions::default());
    }
    let text = std::str::from_utf8(bytes).map_err(|err| {
        BindingError::new(
            BindingStatus::Utf8Error,
            format!("invalid options_json UTF-8: {err}"),
        )
    })?;
    let value: serde_json::Value = serde_json::from_str(text).map_err(|err| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid options_json: {err}"),
        )
    })?;
    #[cfg(feature = "svg")]
    reject_removed_layout_fields(&value)?;
    let mut options: BindingOptions = serde_json::from_value(value.clone()).map_err(|err| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid options_json: {err}"),
        )
    })?;
    if let Some(version) = options.version
        && version != BINDING_OPTIONS_SCHEMA_VERSION
    {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            format!(
                "unsupported options_json schema version {version}; expected {BINDING_OPTIONS_SCHEMA_VERSION}"
            ),
        ));
    }
    options.analysis = binding_analysis_options_json_from_json_value(&value)?;
    validate_resource_contract_ids(options.analysis.resources.as_ref())?;
    Ok(options)
}

fn validate_resource_contract_ids(
    resources: Option<&ResourceOptionsJson>,
) -> Result<(), BindingError> {
    let Some(resources) = resources else {
        return Ok(());
    };
    if let Some(id) = resources
        .limits
        .keys()
        .find(|id| !BindingResourceScope::is_known_limit(id))
    {
        return Err(BindingError::new(
            BindingStatus::InvalidArgument,
            format!("resource limit id `{id}` is not part of resource contract schema 1"),
        ));
    }
    Ok(())
}

pub(crate) fn merge_request_options(
    base_options_json: &[u8],
    request_options_json: &[u8],
    resource_scope: BindingResourceScope,
) -> Result<Vec<u8>, BindingError> {
    parse_options(request_options_json)?;
    let request_value: Value = serde_json::from_slice(request_options_json).map_err(|err| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid request options_json: {err}"),
        )
    })?;
    if request_selects_runtime_policy(&request_value) {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "request options_json cannot set runtime_policy; configure it when creating the engine",
        ));
    }
    let mut request_value = normalize_analysis_wrapper(request_value);
    let requested_resources = take_request_resource_options(&mut request_value, resource_scope)?;

    let mut merged = if base_options_json.is_empty() {
        Value::Object(Map::new())
    } else {
        serde_json::from_slice(base_options_json)
            .map_err(|err| {
                BindingError::new(
                    BindingStatus::InternalError,
                    format!("stored engine options_json is invalid: {err}"),
                )
            })
            .map(normalize_analysis_wrapper)?
    };
    merge_json_value(&mut merged, request_value);
    if let Some(requested_resources) = requested_resources {
        let ceiling = parse_options(base_options_json)?
            .analysis
            .resources
            .unwrap_or_default();
        let resources = tighten_resource_options(&ceiling, &requested_resources)?;
        merged
            .as_object_mut()
            .expect("validated binding options always normalize to an object")
            .insert(
                "resources".to_string(),
                serde_json::to_value(resources).map_err(internal_json_error)?,
            );
    }
    serde_json::to_vec(&merged).map_err(internal_json_error)
}

pub(crate) fn validate_one_shot_resource_options(
    options_json: &[u8],
    resource_scope: BindingResourceScope,
) -> Result<(), BindingError> {
    let mut value = normalize_analysis_wrapper(options_json_value(options_json)?);
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
        if !BindingResourceScope::is_known_limit(id) || resource_scope.accepts(id) {
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

    let analysis = merman_analysis::AnalysisOptionsJson {
        fixed_today: options.analysis.fixed_today.clone(),
        fixed_local_offset_minutes: options.analysis.fixed_local_offset_minutes,
        site_config: options.analysis.site_config.clone(),
        resources: max_source_bytes.map(|max_source_bytes| merman_analysis::ResourceOptionsJson {
            limits: std::iter::once(("max_source_bytes".to_string(), max_source_bytes)).collect(),
        }),
        lint: options.analysis.lint.clone(),
    };
    analysis
        .to_analysis_options()
        .map_err(|err| BindingError::new(BindingStatus::InvalidArgument, err.to_string()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BindingResourceScope {
    Analysis,
    Model,
    Layout,
    Render,
}

impl BindingResourceScope {
    fn is_known_limit(stable_id: &str) -> bool {
        merman::resources::InputResourceLimitId::from_stable_id(stable_id).is_some()
            || Self::is_render_limit(stable_id)
    }

    fn is_render_limit(stable_id: &str) -> bool {
        matches!(
            stable_id,
            "max_layout_work_units" | "max_svg_bytes" | "max_svg_elements"
        )
    }

    fn accepts(self, stable_id: &str) -> bool {
        let input = merman::resources::InputResourceLimitId::from_stable_id(stable_id);
        match self {
            Self::Analysis => {
                input == Some(merman::resources::InputResourceLimitId::MaxSourceBytes)
            }
            Self::Model => input.is_some(),
            Self::Layout => input.is_some() || stable_id == "max_layout_work_units",
            Self::Render => input.is_some() || Self::is_render_limit(stable_id),
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::Analysis => "analysis",
            Self::Model => "semantic-model",
            Self::Layout => "layout",
            Self::Render => "render",
        }
    }
}

pub(crate) const fn input_resource_limit_available_for_build(
    _id: merman::resources::InputResourceLimitId,
) -> bool {
    // semantic-json is a base operation in every build and enforces the complete input policy.
    true
}

fn resource_limit_available_for_build(id: &str) -> bool {
    if let Some(input_id) = merman::resources::InputResourceLimitId::from_stable_id(id) {
        return input_resource_limit_available_for_build(input_id);
    }
    #[cfg(feature = "svg")]
    {
        matches!(
            merman::svg::ResourceLimitId::from_stable_id(id),
            Some(merman::svg::ResourceLimitId::Render(_))
        )
    }
    #[cfg(not(feature = "svg"))]
    false
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
                if !input_resource_limit_available_for_build(input_id) {
                    return Err(BindingError::new(
                        BindingStatus::InvalidArgument,
                        format!("resource limit id `{id}` is not available for this artifact"),
                    ));
                }
                policy.apply_limit(input_id, *value).map_err(|error| {
                    BindingError::new(BindingStatus::InvalidArgument, error.to_string())
                })?;
                continue;
            }
            if resource_limit_available_for_build(id) {
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

#[cfg(feature = "svg")]
pub(crate) fn binding_resource_policy(
    resources: Option<&ResourceOptionsJson>,
) -> Result<merman::svg::RenderResourcePolicy, BindingError> {
    let profile = resources
        .and_then(|resources| resources.profile.as_deref())
        .map(|id| {
            merman::svg::RenderResourceProfile::from_id(id).ok_or_else(|| {
                BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("unsupported resources.profile: {id}"),
                )
            })
        })
        .transpose()?
        .unwrap_or(merman::svg::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE);
    let mut limits = merman::svg::RenderResourcePolicy::for_profile(profile);
    if let Some(resources) = resources {
        for (id, value) in &resources.limits {
            limits.apply_override(id, *value).map_err(|error| {
                BindingError::new(BindingStatus::InvalidArgument, error.to_string())
            })?;
        }
    }
    Ok(limits)
}

pub fn resource_options_json(
    profile_id: &str,
    overrides: &[(&str, usize)],
) -> Result<Vec<u8>, BindingError> {
    let profile = merman::resources::ResourceProfile::from_id(profile_id).ok_or_else(|| {
        BindingError::new(
            BindingStatus::InvalidArgument,
            format!("unsupported resources.profile: {profile_id}"),
        )
    })?;
    #[cfg(feature = "svg")]
    let mut render_policy = merman::svg::RenderResourcePolicy::for_profile(profile);
    #[cfg(not(feature = "svg"))]
    let mut input_policy = merman::resources::InputResourcePolicy::for_profile(profile);
    let mut limits = BTreeMap::new();
    for &(id, value) in overrides {
        if limits.insert(id, value).is_some() {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!("duplicate resource limit override: {id}"),
            ));
        }
        if !resource_limit_available_for_build(id) {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!("resource limit id `{id}` is not available for this build"),
            ));
        }
        #[cfg(feature = "svg")]
        render_policy.apply_override(id, value).map_err(|error| {
            BindingError::new(BindingStatus::InvalidArgument, error.to_string())
        })?;
        #[cfg(not(feature = "svg"))]
        input_policy.apply_override(id, value).map_err(|error| {
            BindingError::new(BindingStatus::InvalidArgument, error.to_string())
        })?;
    }

    let resources = if limits.is_empty() {
        serde_json::json!({ "profile": profile.id() })
    } else {
        serde_json::json!({ "profile": profile.id(), "limits": limits })
    };
    serde_json::to_vec(&serde_json::json!({
        "version": BINDING_OPTIONS_SCHEMA_VERSION,
        "resources": resources,
    }))
    .map_err(internal_json_error)
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
    let ceiling_json = resource_options_json(ceiling_profile_id, ceiling_overrides)?;
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

#[cfg(feature = "svg")]
fn effective_resource_limits(
    resources: &ResourceOptionsJson,
) -> Result<BTreeMap<&'static str, Option<usize>>, BindingError> {
    let policy = binding_resource_policy(Some(resources))?;
    Ok(merman::svg::ResourceLimitId::ALL
        .into_iter()
        .map(|id| (id.as_str(), policy.value(id)))
        .collect())
}

#[cfg(not(feature = "svg"))]
fn effective_resource_limits(
    resources: &ResourceOptionsJson,
) -> Result<BTreeMap<&'static str, Option<usize>>, BindingError> {
    let policy = binding_input_resource_policy(Some(resources))?;
    Ok(merman::resources::InputResourceLimitId::ALL
        .into_iter()
        .filter(|id| input_resource_limit_available_for_build(*id))
        .map(|id| (id.as_str(), policy.value(id)))
        .collect())
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

    #[test]
    fn error_payload_json_uses_public_code_names() {
        let payload = error_payload_json_bytes(BindingStatus::RenderError, "failed");
        let json: Value = serde_json::from_slice(&payload).unwrap();

        assert_eq!(json["code"], BindingStatus::RenderError.code());
        assert_eq!(json["code_name"], BindingStatus::RenderError.code_name());
        assert_eq!(json["kind"], "generic");
        assert!(json["capability_id"].is_null());
        assert_eq!(json["message"], "failed");

        let error = BindingError::missing_capability("layout-elk", "ELK is unavailable");
        let payload = binding_error_payload_json_bytes(&error);
        let json: Value = serde_json::from_slice(&payload).unwrap();
        assert_eq!(json["kind"], "missing-capability");
        assert_eq!(json["capability_id"], "layout-elk");
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
        let merged = merge_request_options(
            br#"{
                "version": 1,
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
        let value: Value = serde_json::from_slice(&merged).unwrap();

        assert_eq!(value["version"], 1);
        assert_eq!(value["parse"]["suppress_errors"], true);
        assert_eq!(value["resources"]["profile"], "interactive");
        assert_eq!(value["resources"]["limits"]["max_source_bytes"], 2048);
        assert_eq!(value["resources"]["limits"]["max_model_items"], 128);
    }

    #[test]
    fn request_options_cannot_select_runtime_policy() {
        let error = merge_request_options(
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
            let error =
                merge_request_options(b"", request.as_bytes(), BindingResourceScope::Analysis)
                    .unwrap_err();

            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
            assert!(error.message().contains("cannot set runtime_policy"));
        }
    }

    #[test]
    fn request_options_normalize_wrapped_analysis_before_merging() {
        let merged = merge_request_options(
            br#"{"resources":{"limits":{"max_source_bytes":4096}}}"#,
            br#"{"analysis":{"lint":{"profile":"recommended"}}}"#,
            BindingResourceScope::Analysis,
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&merged).unwrap();
        let _options = parse_options(&merged).unwrap();

        assert_eq!(
            value["resources"]["limits"]["max_source_bytes"],
            Value::from(4096)
        );
        assert_eq!(value["lint"]["profile"], "recommended");
        assert!(value.get("analysis").is_none());
        #[cfg(feature = "analysis")]
        assert_eq!(
            _options
                .analysis
                .lint
                .as_ref()
                .and_then(|lint| lint.profile.as_deref()),
            Some("recommended")
        );
    }

    #[test]
    fn request_options_normalize_wrapped_engine_options_before_merging() {
        let merged = merge_request_options(
            br#"{"merman":{"resources":{"profile":"interactive","limits":{"max_source_bytes":4096}}}}"#,
            br#"{"resources":{"limits":{"max_source_bytes":2048,"max_model_items":128}}}"#,
            BindingResourceScope::Model,
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&merged).unwrap();
        parse_options(&merged).unwrap();

        assert_eq!(value["resources"]["profile"], "interactive");
        assert_eq!(value["resources"]["limits"]["max_source_bytes"], 2048);
        assert_eq!(value["resources"]["limits"]["max_model_items"], 128);
        assert!(value.get("merman").is_none());
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
                "version": 1,
                "svg": { "pipeline": "resvg-safe" }
            }"#,
        )
        .unwrap();

        assert_eq!(options.version, Some(1));
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
    fn parse_options_rejects_unknown_schema_versions_and_flat_resource_limits() {
        let version = parse_options(br#"{ "version": 2 }"#).unwrap_err();
        assert_eq!(version.status(), BindingStatus::OptionsJsonError);
        assert!(version.message().contains("expected 1"));

        let flat = parse_options(br#"{ "resources": { "max_source_bytes": 4 } }"#).unwrap_err();
        assert_eq!(flat.status(), BindingStatus::OptionsJsonError);
        assert!(flat.message().contains("unknown field `max_source_bytes`"));
    }

    #[test]
    fn resource_ceiling_preserves_stricter_limits_and_wrapper_shape() {
        for wrapper in [None, Some("analysis"), Some("merman")] {
            let input = match wrapper {
                Some(wrapper) => format!(
                    r#"{{"{wrapper}":{{"site_config":{{"theme":"dark"}},"resources":{{"limits":{{"max_source_bytes":4096}}}}}}}}"#
                ),
                None => {
                    r#"{"parse":{"suppress_errors":true},"resources":{"limits":{"max_source_bytes":4096}}}"#
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
            "constrained",
            &[("max_source_bytes", 4096), ("max_svg_bytes", 8192)],
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(value["version"], 1);
        assert_eq!(value["resources"]["profile"], "constrained");
        assert_eq!(value["resources"]["limits"]["max_source_bytes"], 4096);

        for error in [
            resource_options_json("missing", &[]).unwrap_err(),
            resource_options_json("interactive", &[("unknown_limit", 1)]).unwrap_err(),
            resource_options_json(
                "interactive",
                &[("max_source_bytes", 1), ("max_source_bytes", 2)],
            )
            .unwrap_err(),
        ] {
            assert_eq!(error.status(), BindingStatus::InvalidArgument);
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
            "constrained",
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
            "constrained",
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
                    "lint": { "profile": "recommended" }
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

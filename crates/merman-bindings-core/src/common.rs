#[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
use serde::Deserialize;
use serde::Serialize;
#[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
use serde_json::{Map, Value};
#[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
use std::collections::BTreeMap;

pub const BINDING_OPTIONS_SCHEMA_VERSION: u32 = 1;
pub const BINDING_RESULT_PAYLOAD_VERSION: u32 = 1;

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

#[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
#[derive(Debug, Default, Deserialize)]
pub(crate) struct BindingOptions {
    #[allow(dead_code)]
    pub(crate) version: Option<u32>,
    #[serde(flatten)]
    pub(crate) analysis: BindingAnalysisOptionsJson,
    #[cfg(any(feature = "render", feature = "ascii"))]
    pub(crate) parse: Option<ParseOptionsJson>,
    #[cfg(feature = "render")]
    pub(crate) host_theme: Option<HostThemeOptionsJson>,
    #[cfg(feature = "ascii")]
    pub(crate) ascii: Option<AsciiOptionsJson>,
    #[cfg(feature = "render")]
    pub(crate) layout: Option<LayoutOptionsJson>,
    #[cfg(feature = "render")]
    pub(crate) environment: Option<RenderEnvironmentOptionsJson>,
    #[cfg(feature = "render")]
    pub(crate) svg: Option<SvgOptionsJson>,
}

#[allow(dead_code)]
#[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
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
#[cfg(any(feature = "render", feature = "ascii"))]
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct ParseOptionsJson {
    pub(crate) suppress_errors: Option<bool>,
}

#[allow(dead_code)]
#[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ResourceOptionsJson {
    pub(crate) profile: Option<String>,
    #[serde(default)]
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

#[cfg(feature = "render")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LayoutOptionsJson {
    pub(crate) container_width: Option<f64>,
    pub(crate) container_height: Option<f64>,
}

#[cfg(feature = "render")]
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RenderEnvironmentOptionsJson {
    pub(crate) text_measurement: Option<String>,
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

#[cfg(feature = "render")]
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

#[cfg(feature = "render")]
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

#[cfg(feature = "render")]
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
    let payload = ErrorPayload {
        version: BINDING_RESULT_PAYLOAD_VERSION,
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

#[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
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
    #[cfg(feature = "render")]
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
    Ok(options)
}

#[cfg(feature = "render")]
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

#[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
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

#[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
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

#[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
fn binding_analysis_option_keys_present(map: &Map<String, Value>) -> bool {
    [
        "fixed_today",
        "fixed_local_offset_minutes",
        "site_config",
        "resources",
        "lint",
    ]
    .iter()
    .any(|key| map.contains_key(*key))
}

#[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
fn reject_removed_nested_analysis_parse_option(value: &Value) -> Result<(), BindingError> {
    let Value::Object(map) = value else {
        return Ok(());
    };

    #[cfg(not(any(feature = "render", feature = "ascii")))]
    if map.contains_key("parse") {
        return Err(BindingError::new(
            BindingStatus::OptionsJsonError,
            "analysis option `parse` was removed; this build has no parse, render, or ASCII operation",
        ));
    }

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

#[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
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

#[cfg(any(feature = "render", feature = "ascii"))]
pub(crate) fn source_text(bytes: &[u8]) -> Result<&str, BindingError> {
    let source = source_text_utf8(bytes)?;
    if source.trim().is_empty() {
        return Err(no_diagram_error());
    }
    Ok(source)
}

#[cfg(any(feature = "render", feature = "ascii"))]
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

#[cfg(any(feature = "render", feature = "ascii"))]
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

#[cfg(any(feature = "render", feature = "ascii"))]
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

#[cfg(any(feature = "render", feature = "ascii"))]
#[derive(Clone)]
pub(crate) struct BindingLocalTimePolicy {
    pub(crate) today: Option<chrono::NaiveDate>,
    pub(crate) time_zone: merman::time::LocalTimeZone,
    #[cfg(feature = "render")]
    pub(crate) explicit: bool,
}

#[cfg(any(feature = "render", feature = "ascii"))]
pub(crate) fn binding_local_time_policy(
    options: &BindingOptions,
) -> Result<BindingLocalTimePolicy, BindingError> {
    let today = binding_fixed_today(options)?;
    let offset_minutes = binding_fixed_local_offset_minutes(options)?;
    let time_zone = match offset_minutes {
        Some(offset_minutes) => merman::time::LocalTimeZone::fixed(offset_minutes),
        None => {
            #[cfg(feature = "core-host")]
            {
                Ok(merman::time::LocalTimeZone::system())
            }
            #[cfg(not(feature = "core-host"))]
            {
                Ok(merman::time::LocalTimeZone::utc())
            }
        }
    }
    .map_err(|err| BindingError::new(BindingStatus::InvalidArgument, err.to_string()))?;

    if let Some(today) = today {
        let midnight = today
            .and_hms_opt(0, 0, 0)
            .expect("every valid date has a valid midnight");
        if time_zone.datetime_from_naive_local(midnight).is_none() {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!(
                    "fixed_today is outside the supported range of the selected local timezone: {today}"
                ),
            ));
        }
    }

    Ok(BindingLocalTimePolicy {
        today,
        time_zone,
        #[cfg(feature = "render")]
        explicit: today.is_some() || offset_minutes.is_some(),
    })
}

#[cfg(feature = "analysis")]
pub(crate) fn analysis_options(
    options: &BindingOptions,
) -> Result<merman_analysis::AnalysisOptions, BindingError> {
    analysis_options_for_resource_operation(options, InputResourceOperation::Analysis)
}

#[cfg(feature = "analysis")]
pub(crate) fn artifact_analysis_options(
    options: &BindingOptions,
) -> Result<merman_analysis::AnalysisOptions, BindingError> {
    analysis_options_for_resource_operation(options, InputResourceOperation::ArtifactUnion)
}

#[cfg(feature = "analysis")]
fn analysis_options_for_resource_operation(
    options: &BindingOptions,
    operation: InputResourceOperation,
) -> Result<merman_analysis::AnalysisOptions, BindingError> {
    let max_source_bytes =
        binding_input_resource_policy(options.analysis.resources.as_ref(), operation)?
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

#[cfg(any(feature = "analysis", feature = "ascii"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputResourceOperation {
    // ABI 2 has one shared options object. One-shot calls validate their exact operation, while a
    // cached multi-operation engine accepts the artifact union and projects only owned limits.
    // ABI 3 moves this choice into each generic operation request.
    #[cfg(feature = "analysis")]
    Analysis,
    #[cfg(feature = "ascii")]
    Ascii,
    ArtifactUnion,
}

#[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
pub(crate) const fn input_resource_limit_available_for_build(
    id: merman::resources::InputResourceLimitId,
) -> bool {
    if cfg!(feature = "render") {
        return true;
    }
    match id {
        merman::resources::InputResourceLimitId::MaxSourceBytes => {
            cfg!(feature = "analysis") || cfg!(feature = "ascii")
        }
        merman::resources::InputResourceLimitId::MaxFlowchartNodes
        | merman::resources::InputResourceLimitId::MaxFlowchartEdges
        | merman::resources::InputResourceLimitId::MaxFlowchartSubgraphs
        | merman::resources::InputResourceLimitId::MaxClassNodes
        | merman::resources::InputResourceLimitId::MaxClassEdges
        | merman::resources::InputResourceLimitId::MaxClassNamespaces
        | merman::resources::InputResourceLimitId::MaxLabelBytes => cfg!(feature = "ascii"),
        merman::resources::InputResourceLimitId::MaxZenumlParticipants
        | merman::resources::InputResourceLimitId::MaxZenumlStatements
        | merman::resources::InputResourceLimitId::MaxZenumlFragments => false,
    }
}

#[cfg(any(feature = "analysis", feature = "ascii"))]
const fn input_resource_limit_available_for_operation(
    operation: InputResourceOperation,
    id: merman::resources::InputResourceLimitId,
) -> bool {
    match operation {
        InputResourceOperation::ArtifactUnion => input_resource_limit_available_for_build(id),
        #[cfg(feature = "analysis")]
        InputResourceOperation::Analysis => {
            cfg!(feature = "analysis")
                && matches!(id, merman::resources::InputResourceLimitId::MaxSourceBytes)
        }
        #[cfg(feature = "ascii")]
        InputResourceOperation::Ascii => {
            cfg!(feature = "ascii")
                && matches!(
                    id,
                    merman::resources::InputResourceLimitId::MaxSourceBytes
                        | merman::resources::InputResourceLimitId::MaxFlowchartNodes
                        | merman::resources::InputResourceLimitId::MaxFlowchartEdges
                        | merman::resources::InputResourceLimitId::MaxFlowchartSubgraphs
                        | merman::resources::InputResourceLimitId::MaxClassNodes
                        | merman::resources::InputResourceLimitId::MaxClassEdges
                        | merman::resources::InputResourceLimitId::MaxClassNamespaces
                        | merman::resources::InputResourceLimitId::MaxLabelBytes
                )
        }
    }
}

#[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
fn resource_limit_available_for_build(id: &str) -> bool {
    if let Some(input_id) = merman::resources::InputResourceLimitId::from_stable_id(id) {
        return input_resource_limit_available_for_build(input_id);
    }
    #[cfg(feature = "render")]
    {
        matches!(
            merman::render::ResourceLimitId::from_stable_id(id),
            Some(merman::render::ResourceLimitId::Render(_))
        )
    }
    #[cfg(not(feature = "render"))]
    false
}

#[cfg(any(feature = "analysis", feature = "ascii"))]
pub(crate) fn binding_input_resource_policy(
    resources: Option<&ResourceOptionsJson>,
    operation: InputResourceOperation,
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
                if !input_resource_limit_available_for_operation(operation, input_id) {
                    return Err(BindingError::new(
                        BindingStatus::InvalidArgument,
                        format!(
                            "resource limit id `{id}` is not available for the {operation:?} operation"
                        ),
                    ));
                }
                policy.apply_limit(input_id, *value).map_err(|error| {
                    BindingError::new(BindingStatus::InvalidArgument, error.to_string())
                })?;
                continue;
            }
            if operation == InputResourceOperation::ArtifactUnion
                && resource_limit_available_for_build(id)
            {
                continue;
            }
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                format!(
                    "resource limit id `{id}` is not available for the {operation:?} operation"
                ),
            ));
        }
    }
    Ok(policy)
}

#[cfg(feature = "render")]
pub(crate) fn binding_resource_policy(
    resources: Option<&ResourceOptionsJson>,
) -> Result<merman::render::RenderResourcePolicy, BindingError> {
    let profile = resources
        .and_then(|resources| resources.profile.as_deref())
        .map(|id| {
            merman::render::RenderResourceProfile::from_id(id).ok_or_else(|| {
                BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("unsupported resources.profile: {id}"),
                )
            })
        })
        .transpose()?
        .unwrap_or(merman::render::GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE);
    let mut limits = merman::render::RenderResourcePolicy::for_profile(profile);
    if let Some(resources) = resources {
        for (id, value) in &resources.limits {
            limits.apply_override(id, *value).map_err(|error| {
                BindingError::new(BindingStatus::InvalidArgument, error.to_string())
            })?;
        }
    }
    Ok(limits)
}

#[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
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
    #[cfg(feature = "render")]
    let mut render_policy = merman::render::RenderResourcePolicy::for_profile(profile);
    #[cfg(not(feature = "render"))]
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
        #[cfg(feature = "render")]
        render_policy.apply_override(id, value).map_err(|error| {
            BindingError::new(BindingStatus::InvalidArgument, error.to_string())
        })?;
        #[cfg(not(feature = "render"))]
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

#[cfg(not(any(feature = "analysis", feature = "render", feature = "ascii")))]
pub fn resource_options_json(
    profile_id: &str,
    overrides: &[(&str, usize)],
) -> Result<Vec<u8>, BindingError> {
    let _ = (profile_id, overrides);
    Err(render_resource_options_unavailable())
}

pub fn render_resource_options_unavailable() -> BindingError {
    BindingError::new(
        BindingStatus::UnsupportedFormat,
        "resource options requires at least one resource-aware operation",
    )
}

#[cfg(feature = "analysis")]
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

#[allow(dead_code)]
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

    #[cfg(any(feature = "render", feature = "ascii"))]
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
        #[cfg(feature = "render")]
        assert_eq!(
            binding_resource_policy(options.analysis.resources.as_ref())
                .unwrap()
                .value(merman::render::ResourceLimitId::MaxSourceBytes),
            Some(4)
        );
        #[cfg(feature = "render")]
        assert_eq!(
            options.svg.as_ref().and_then(|svg| svg.pipeline.as_deref()),
            Some("resvg-safe")
        );
    }

    #[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
    #[test]
    fn parse_options_rejects_unknown_schema_versions_and_flat_resource_limits() {
        let version = parse_options(br#"{ "version": 2 }"#).unwrap_err();
        assert_eq!(version.status(), BindingStatus::OptionsJsonError);
        assert!(version.message().contains("expected 1"));

        let flat = parse_options(br#"{ "resources": { "max_source_bytes": 4 } }"#).unwrap_err();
        assert_eq!(flat.status(), BindingStatus::OptionsJsonError);
        assert!(flat.message().contains("unknown field `max_source_bytes`"));
    }

    #[cfg(feature = "render")]
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
            resource_options_json("interactive", &[("max_svg_tree_depth", 1)]).unwrap_err(),
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
        not(feature = "render"),
        not(feature = "analysis"),
        not(feature = "ascii")
    ))]
    #[test]
    fn resource_options_builder_reports_a_typed_missing_operation_capability() {
        let error = resource_options_json("constrained", &[]).unwrap_err();
        assert_eq!(error.status(), BindingStatus::UnsupportedFormat);
        assert_eq!(
            error.message(),
            "resource options requires at least one resource-aware operation"
        );
    }

    #[cfg(all(feature = "analysis", not(feature = "render"), not(feature = "ascii")))]
    #[test]
    fn analysis_only_resource_options_accept_only_the_source_limit() {
        let json = resource_options_json("constrained", &[("max_source_bytes", 4096)]).unwrap();
        let value: Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(value["resources"]["limits"]["max_source_bytes"], 4096);

        let error =
            resource_options_json("constrained", &[("max_flowchart_nodes", 1)]).unwrap_err();
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("not available for this build"));
    }

    #[cfg(all(feature = "analysis", feature = "ascii", not(feature = "render")))]
    #[test]
    fn operation_scope_rejects_sibling_limits_but_artifact_union_accepts_them() {
        let options = parse_options(
            br#"{ "resources": { "profile": "constrained", "limits": { "max_flowchart_nodes": 1 } } }"#,
        )
        .unwrap();

        let error = analysis_options(&options).unwrap_err();
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("Analysis operation"));

        artifact_analysis_options(&options).unwrap();
        let ascii = binding_input_resource_policy(
            options.analysis.resources.as_ref(),
            InputResourceOperation::Ascii,
        )
        .unwrap();
        assert_eq!(
            ascii.value(merman::resources::InputResourceLimitId::MaxFlowchartNodes),
            Some(1)
        );
    }

    #[cfg(all(feature = "analysis", not(any(feature = "render", feature = "ascii"))))]
    #[test]
    fn analysis_only_build_rejects_top_level_parse_options() {
        let err = parse_options(br#"{ "parse": { "suppress_errors": true } }"#).unwrap_err();
        assert_eq!(err.status(), BindingStatus::OptionsJsonError);
        assert!(
            err.message()
                .contains("analysis option `parse` was removed")
        );
    }

    #[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
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

    #[cfg(any(feature = "analysis", feature = "render", feature = "ascii"))]
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

#![forbid(unsafe_code)]

//! UniFFI bindings for `merman`.
//!
//! This crate exposes an idiomatic generated-binding surface over `merman-bindings-core`. It does
//! not replace the canonical C ABI in `merman-ffi`.

use merman_bindings_core::{
    ArtifactCapabilitySurface, BindingEngine, BindingEngineAdmission, BindingEngineAdmissionMode,
    BindingError, BindingErrorKind, BindingStatus, TextMeasurementProviderProjection,
};
#[cfg(feature = "svg")]
use merman_bindings_core::{HostTextMeasurementError, HostTextMeasurer};
use serde_json::Value;
use std::sync::{Arc, OnceLock};

/// Version of the direct UniFFI binding API.
///
/// This version belongs to the generated UniFFI surface only. It is intentionally independent
/// from both the C ABI and the text-measurement protocol, whose versions are owned by their
/// respective descriptors.
pub const UNIFFI_BINDING_API_VERSION: u32 = 3;

static SUPPORTED_DIAGRAMS: OnceLock<Vec<String>> = OnceLock::new();
static ASCII_CAPABILITIES: OnceLock<Vec<MermanAsciiCapability>> = OnceLock::new();
static SUPPORTED_THEMES: OnceLock<Vec<String>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MermanErrorKind {
    Generic,
    UnknownOperation,
    MissingCapability,
    ReentrantCall,
    Busy,
}

impl From<BindingErrorKind> for MermanErrorKind {
    fn from(kind: BindingErrorKind) -> Self {
        match kind {
            BindingErrorKind::Generic => Self::Generic,
            BindingErrorKind::UnknownOperation => Self::UnknownOperation,
            BindingErrorKind::MissingCapability => Self::MissingCapability,
            BindingErrorKind::ReentrantCall => Self::ReentrantCall,
            BindingErrorKind::Busy => Self::Busy,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanResourceErrorDetails {
    pub cause: String,
    pub limit_id: String,
    pub phase: String,
    pub actual: u64,
    pub max: u64,
    pub profile: String,
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum MermanError {
    #[error("{code_name}: {message}")]
    Binding {
        code: i32,
        code_name: String,
        kind: MermanErrorKind,
        capability_id: Option<String>,
        resource: Option<MermanResourceErrorDetails>,
        message: String,
    },
}

impl MermanError {
    pub fn from_binding(error: BindingError) -> Self {
        let status = error.status();
        let resource = error
            .resource_details()
            .map(|details| MermanResourceErrorDetails {
                cause: details.cause.to_string(),
                limit_id: details.limit_id.to_string(),
                phase: details.phase.to_string(),
                actual: details.actual,
                max: details.max,
                profile: details.profile.to_string(),
            });
        Self::Binding {
            code: status.code(),
            code_name: status.code_name().to_string(),
            kind: error.kind().into(),
            capability_id: error.capability_id().map(str::to_string),
            resource,
            message: error.message().to_string(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        let status = BindingStatus::InternalError;
        Self::Binding {
            code: status.code(),
            code_name: status.code_name().to_string(),
            kind: MermanErrorKind::Generic,
            capability_id: None,
            resource: None,
            message: message.into(),
        }
    }
}

#[cfg(not(feature = "svg"))]
fn missing_capability_error(capability_id: &'static str, message: &'static str) -> MermanError {
    MermanError::from_binding(BindingError::missing_capability(capability_id, message))
}

impl From<uniffi::UnexpectedUniFFICallbackError> for MermanError {
    fn from(error: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::internal(format!("host text measurer callback failed: {error}"))
    }
}

include!("generated/resource_contract.rs");

#[derive(Debug, Default, uniffi::Object)]
pub struct MermanEngine;

/// An immutable reusable engine with transport-neutral operation admission.
///
/// Callback-free engines admit concurrent operations. Engines constructed with a host text
/// measurer serialize operations, and calls made while that engine's callback is active fail with
/// [`MermanErrorKind::ReentrantCall`].
#[derive(uniffi::Object)]
pub struct MermanReusableEngine {
    engine: BindingEngine,
    admission: Arc<BindingEngineAdmission>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanValidationResult {
    pub valid: bool,
    pub error: Option<String>,
    pub code: i32,
    pub code_name: String,
}

/// A transport-neutral operation request.
///
/// `operation_id` is validated by the canonical binding-operation catalog. `uri` is required only by
/// document operations. `options_json` configures a one-shot engine or overrides a reusable
/// engine's baseline for this operation.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanOperationRequest {
    pub operation_id: String,
    pub source: String,
    pub uri: Option<String>,
    pub options_json: Option<String>,
}

/// Binary-safe output returned by [`MermanEngine::execute`] and reusable engines.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanOperationResult {
    pub operation_id: String,
    pub media_type: String,
    pub data: Vec<u8>,
    pub metadata_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanDiagramFamilyCapability {
    pub diagram_type: String,
    pub logical_family_kind: String,
    pub metadata_id: Option<String>,
    pub render_model_kind: Option<String>,
    pub has_detector: bool,
    pub has_semantic_parser: bool,
    pub has_editor_parser: bool,
    pub has_combined_parser: bool,
    pub has_render_parser: bool,
    pub has_header: bool,
    pub config_namespace: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanLintRuleCatalogEntry {
    pub id: String,
    pub description: String,
    pub evidence: Vec<String>,
    pub default_severity: String,
    pub category: String,
    pub default_enabled: bool,
    pub default_profile: String,
    pub origin: String,
    pub configurable: bool,
    pub fixable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanAsciiCapabilityEvidence {
    pub kind: String,
    pub source: String,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanAsciiCapability {
    pub diagram_type: String,
    pub display_name: String,
    pub support_level: String,
    pub summary_fallback: bool,
    pub supported_semantics: Vec<String>,
    pub limits: Vec<String>,
    pub evidence: Vec<MermanAsciiCapabilityEvidence>,
}

include!("generated/text_measurement_abi.rs");

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MermanTextMeasureRequest {
    pub operation: MermanTextMeasurementOperation,
    pub phase: MermanTextMeasurementPhase,
    pub text: String,
    pub font_family: Option<String>,
    pub font_size: f64,
    pub font_weight: Option<String>,
    pub font_style: String,
    pub max_width: Option<f64>,
    pub line_height: f64,
    pub letter_spacing: f64,
    pub word_spacing: f64,
    pub wrap_mode: MermanTextWrapMode,
    pub direction: MermanTextDirection,
    pub white_space: MermanTextWhiteSpace,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct MermanTextMeasureResult {
    pub result_kind: MermanTextMeasurementResultKind,
    pub width: f64,
    pub height: f64,
    pub length: f64,
    pub line_count: u64,
    pub bbox_left: Option<f64>,
    pub bbox_right: Option<f64>,
    pub raw_width: Option<f64>,
}

/// Synchronous host text measurement supplied when a reusable engine is constructed.
///
/// Foreign implementations return ordinary errors through UniFFI's generated trampoline. They
/// must not unwind, throw, or otherwise perform a non-local exit across that FFI boundary.
#[uniffi::export(with_foreign)]
pub trait MermanTextMeasurer: Send + Sync {
    fn measure(
        &self,
        request: MermanTextMeasureRequest,
    ) -> Result<Option<MermanTextMeasureResult>, MermanError>;
}

#[cfg(feature = "svg")]
struct UniffiHostTextMeasurer {
    callback: Arc<dyn MermanTextMeasurer>,
    admission: Arc<BindingEngineAdmission>,
}

#[cfg(feature = "svg")]
impl UniffiHostTextMeasurer {
    fn new(callback: Arc<dyn MermanTextMeasurer>, admission: Arc<BindingEngineAdmission>) -> Self {
        Self {
            callback,
            admission,
        }
    }

    fn call_host(
        &self,
        request: merman_bindings_core::HostTextMeasurementRequest<'_>,
    ) -> merman_bindings_core::HostMeasurementResult {
        let _callback_guard = self
            .admission
            .enter_callback()
            .map_err(|error| HostTextMeasurementError::new(error.to_string()))?;
        let result = match self.callback.measure(MermanTextMeasureRequest {
            operation: uniffi_measurement_operation(request.operation),
            phase: uniffi_measurement_phase(request.phase),
            text: request.text.to_string(),
            font_family: request.style.font_family.clone(),
            font_size: request.style.font_size,
            font_weight: request.style.font_weight.clone(),
            font_style: request
                .style
                .font_style
                .clone()
                .unwrap_or_else(|| "normal".to_string()),
            max_width: request.max_width,
            line_height: uniffi_line_height(request.style, request.wrap_mode),
            letter_spacing: 0.0,
            word_spacing: 0.0,
            wrap_mode: uniffi_wrap_mode(request.wrap_mode),
            direction: MermanTextDirection::Auto,
            white_space: uniffi_white_space(request.max_width, request.wrap_mode),
        }) {
            Ok(Some(result)) => result,
            Ok(None) => return Ok(None),
            Err(error) => return Err(HostTextMeasurementError::new(error.to_string())),
        };

        merman_bindings_core::decode_host_text_measurement(
            request,
            merman_bindings_core::HostTextMeasurementRecord {
                result_kind: Some(uniffi_result_kind(result.result_kind)),
                width: Some(result.width),
                height: Some(result.height),
                line_count: Some(i128::from(result.line_count)),
                length: Some(result.length),
                bbox_left: result.bbox_left,
                bbox_right: result.bbox_right,
                raw_width: result.raw_width,
            },
        )
        .map(Some)
    }
}

#[cfg(feature = "svg")]
impl HostTextMeasurer for UniffiHostTextMeasurer {
    fn measure(
        &self,
        request: merman_bindings_core::HostTextMeasurementRequest<'_>,
    ) -> merman_bindings_core::HostMeasurementResult {
        self.call_host(request)
    }
}

#[cfg(feature = "svg")]
fn uniffi_line_height(
    style: &merman_bindings_core::TextStyle,
    wrap_mode: merman_bindings_core::WrapMode,
) -> f64 {
    let factor = match wrap_mode {
        merman_bindings_core::WrapMode::SvgLike
        | merman_bindings_core::WrapMode::SvgLikeSingleRun => 1.1,
        merman_bindings_core::WrapMode::HtmlLike => 1.5,
    };
    style.font_size.max(1.0) * factor
}

#[cfg(feature = "svg")]
fn uniffi_white_space(
    max_width: Option<f64>,
    wrap_mode: merman_bindings_core::WrapMode,
) -> MermanTextWhiteSpace {
    match wrap_mode {
        merman_bindings_core::WrapMode::HtmlLike if max_width.is_some() => {
            MermanTextWhiteSpace::BreakSpaces
        }
        merman_bindings_core::WrapMode::HtmlLike => MermanTextWhiteSpace::Nowrap,
        merman_bindings_core::WrapMode::SvgLike
        | merman_bindings_core::WrapMode::SvgLikeSingleRun => MermanTextWhiteSpace::Normal,
    }
}

fn uniffi_lint_rule(rule: merman_bindings_core::RuleCatalogEntry) -> MermanLintRuleCatalogEntry {
    MermanLintRuleCatalogEntry {
        id: rule.id.to_string(),
        description: rule.description.to_string(),
        evidence: rule
            .evidence
            .iter()
            .map(|evidence| evidence.to_string())
            .collect(),
        default_severity: rule.default_severity.to_string(),
        category: rule.category.to_string(),
        default_enabled: rule.default_enabled,
        default_profile: rule.default_profile.to_string(),
        origin: rule.origin.to_string(),
        configurable: rule.configurable,
        fixable: rule.fixable,
    }
}

#[uniffi::export]
impl MermanEngine {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }

    pub fn binding_api_version(&self) -> u32 {
        UNIFFI_BINDING_API_VERSION
    }

    pub fn package_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// Returns the complete, self-validating runtime catalog for this artifact.
    ///
    /// The catalog atomically combines the artifact's runtime contract with the
    /// descriptor-owned vocabulary required to validate it. Foreign bindings
    /// must consume this endpoint instead of composing separate metadata reads.
    pub fn runtime_catalog_json(&self) -> Result<String, MermanError> {
        string_output(merman_bindings_core::runtime_catalog_json_for(
            UNIFFI_BINDING_API_VERSION,
            native_transport_capability_surface(),
        ))
    }

    /// Returns the presentation catalog projected to this native artifact.
    pub fn presentation_catalog_json(&self) -> Result<String, MermanError> {
        string_output(merman_bindings_core::presentation_catalog_json_for(
            &native_transport_capability_surface(),
        ))
    }

    /// Executes a descriptor-owned output operation with a fresh engine configuration.
    pub fn execute(
        &self,
        request: MermanOperationRequest,
    ) -> Result<MermanOperationResult, MermanError> {
        execute_once_operation(&request)
    }

    pub fn render_svg(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute(operation_request("svg", source, options_json)))
    }

    pub fn render_png(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<Vec<u8>, MermanError> {
        binary_operation_output(self.execute(operation_request("png", source, options_json)))
    }

    pub fn render_jpeg(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<Vec<u8>, MermanError> {
        binary_operation_output(self.execute(operation_request("jpeg", source, options_json)))
    }

    pub fn render_pdf(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<Vec<u8>, MermanError> {
        binary_operation_output(self.execute(operation_request("pdf", source, options_json)))
    }

    pub fn render_ascii(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute(operation_request("ascii", source, options_json)))
    }

    pub fn parse_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute(operation_request(
            "semantic-json",
            source,
            options_json,
        )))
    }

    pub fn layout_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute(operation_request(
            "layout-json",
            source,
            options_json,
        )))
    }

    pub fn analyze_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute(operation_request(
            "analysis-json",
            source,
            options_json,
        )))
    }

    pub fn analyze_document_json(
        &self,
        source: String,
        options_json: Option<String>,
        uri: String,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute(operation_request_with_uri(
            "document-analysis-json",
            source,
            uri,
            options_json,
        )))
    }

    pub fn analyze_document_facts_json(
        &self,
        source: String,
        options_json: Option<String>,
        uri: String,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute(operation_request_with_uri(
            "document-analysis-facts-json",
            source,
            uri,
            options_json,
        )))
    }

    pub fn validate(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<MermanValidationResult, MermanError> {
        validation_operation_output(self.execute(operation_request(
            "validation-json",
            source,
            options_json,
        )))
    }

    pub fn reusable_engine(
        &self,
        options_json: Option<String>,
    ) -> Result<Arc<MermanReusableEngine>, MermanError> {
        MermanReusableEngine::new(options_json)
    }

    pub fn supported_diagrams(&self) -> Vec<String> {
        cached_string_vec(
            &SUPPORTED_DIAGRAMS,
            merman_bindings_core::supported_diagrams,
        )
    }

    pub fn ascii_capabilities(&self) -> Vec<MermanAsciiCapability> {
        ASCII_CAPABILITIES
            .get_or_init(|| {
                merman_bindings_core::ascii_capabilities()
                    .into_iter()
                    .map(|capability| MermanAsciiCapability {
                        diagram_type: capability.diagram_type.to_string(),
                        display_name: capability.display_name.to_string(),
                        support_level: capability.support_level.to_string(),
                        summary_fallback: capability.summary_fallback,
                        supported_semantics: capability
                            .supported_semantics
                            .iter()
                            .map(|semantic| (*semantic).to_string())
                            .collect(),
                        limits: capability
                            .limits
                            .iter()
                            .map(|limit| (*limit).to_string())
                            .collect(),
                        evidence: capability
                            .evidence
                            .into_iter()
                            .map(|evidence| MermanAsciiCapabilityEvidence {
                                kind: evidence.kind.to_string(),
                                source: evidence.source.to_string(),
                                note: evidence.note.to_string(),
                            })
                            .collect(),
                    })
                    .collect()
            })
            .clone()
    }

    pub fn supported_themes(&self) -> Vec<String> {
        cached_string_vec(&SUPPORTED_THEMES, merman_bindings_core::supported_themes)
    }

    pub fn diagram_family_capabilities(&self) -> Vec<MermanDiagramFamilyCapability> {
        merman_bindings_core::diagram_family_capabilities()
            .into_iter()
            .map(|capability| MermanDiagramFamilyCapability {
                diagram_type: capability.diagram_type.to_string(),
                logical_family_kind: capability.logical_family_kind.to_string(),
                metadata_id: capability.metadata_id.map(str::to_string),
                render_model_kind: capability.render_model_kind.map(str::to_string),
                has_detector: capability.has_detector,
                has_semantic_parser: capability.has_semantic_parser,
                has_editor_parser: capability.has_editor_parser,
                has_combined_parser: capability.has_combined_parser,
                has_render_parser: capability.has_render_parser,
                has_header: capability.has_header,
                config_namespace: capability.config_namespace.map(str::to_string),
            })
            .collect()
    }

    pub fn lint_rule_catalog(&self) -> Result<Vec<MermanLintRuleCatalogEntry>, MermanError> {
        Ok(merman_bindings_core::lint_rule_catalog()
            .map_err(MermanError::from_binding)?
            .into_iter()
            .map(uniffi_lint_rule)
            .collect())
    }

    pub fn configurable_lint_rule_catalog(
        &self,
    ) -> Result<Vec<MermanLintRuleCatalogEntry>, MermanError> {
        Ok(merman_bindings_core::configurable_lint_rule_catalog()
            .map_err(MermanError::from_binding)?
            .into_iter()
            .map(uniffi_lint_rule)
            .collect())
    }
}

#[uniffi::export]
impl MermanReusableEngine {
    #[uniffi::constructor]
    pub fn new(options_json: Option<String>) -> Result<Arc<Self>, MermanError> {
        let engine = binding_engine_for_transport(options_bytes(options_json.as_deref()))
            .map_err(MermanError::from_binding)?;
        Ok(Arc::new(Self {
            engine,
            admission: BindingEngineAdmission::new(BindingEngineAdmissionMode::Concurrent),
        }))
    }

    pub fn render_svg(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute(operation_request("svg", source, options_json)))
    }

    pub fn render_png(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<Vec<u8>, MermanError> {
        binary_operation_output(self.execute(operation_request("png", source, options_json)))
    }

    pub fn render_jpeg(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<Vec<u8>, MermanError> {
        binary_operation_output(self.execute(operation_request("jpeg", source, options_json)))
    }

    pub fn render_pdf(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<Vec<u8>, MermanError> {
        binary_operation_output(self.execute(operation_request("pdf", source, options_json)))
    }

    /// Executes an operation using the reusable baseline plus request-local option overrides.
    pub fn execute(
        &self,
        request: MermanOperationRequest,
    ) -> Result<MermanOperationResult, MermanError> {
        self.with_reusable_operation(|engine| {
            execute_operation(
                engine,
                &request,
                options_bytes(request.options_json.as_deref()),
            )
        })
    }

    pub fn render_ascii(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute(operation_request("ascii", source, options_json)))
    }

    pub fn parse_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute(operation_request(
            "semantic-json",
            source,
            options_json,
        )))
    }

    pub fn layout_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute(operation_request(
            "layout-json",
            source,
            options_json,
        )))
    }

    pub fn analyze_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute(operation_request(
            "analysis-json",
            source,
            options_json,
        )))
    }

    pub fn analyze_document_json(
        &self,
        source: String,
        options_json: Option<String>,
        uri: String,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute(operation_request_with_uri(
            "document-analysis-json",
            source,
            uri,
            options_json,
        )))
    }

    pub fn analyze_document_facts_json(
        &self,
        source: String,
        options_json: Option<String>,
        uri: String,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute(operation_request_with_uri(
            "document-analysis-facts-json",
            source,
            uri,
            options_json,
        )))
    }

    pub fn validate(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<MermanValidationResult, MermanError> {
        validation_operation_output(self.execute(operation_request(
            "validation-json",
            source,
            options_json,
        )))
    }
}

#[uniffi::export]
impl MermanEngine {
    /// Constructs an immutable callback-enabled reusable engine.
    pub fn reusable_engine_with_text_measurer(
        &self,
        options_json: Option<String>,
        measurer: Arc<dyn MermanTextMeasurer>,
    ) -> Result<Arc<MermanReusableEngine>, MermanError> {
        MermanReusableEngine::with_text_measurer(options_json, measurer)
    }
}

#[uniffi::export]
impl MermanReusableEngine {
    /// Constructs an immutable reusable engine with a host text measurer.
    #[uniffi::constructor]
    pub fn with_text_measurer(
        options_json: Option<String>,
        measurer: Arc<dyn MermanTextMeasurer>,
    ) -> Result<Arc<Self>, MermanError> {
        #[cfg(feature = "svg")]
        {
            let engine = binding_engine_for_transport(options_bytes(options_json.as_deref()))
                .map_err(MermanError::from_binding)?;
            let admission = BindingEngineAdmission::new(BindingEngineAdmissionMode::HostCallback);
            let host_text_measurer = Arc::new(UniffiHostTextMeasurer::new(
                measurer,
                Arc::clone(&admission),
            ));
            let engine = engine.with_host_text_measurer(host_text_measurer);
            Ok(Arc::new(Self { engine, admission }))
        }
        #[cfg(not(feature = "svg"))]
        {
            drop((options_json, measurer));
            Err(missing_capability_error(
                "svg",
                "host text measurement requires the svg capability",
            ))
        }
    }
}

impl MermanReusableEngine {
    fn with_reusable_operation<T>(
        &self,
        run: impl FnOnce(&BindingEngine) -> Result<T, MermanError>,
    ) -> Result<T, MermanError> {
        let _operation = self
            .admission
            .enter_operation()
            .map_err(|error| MermanError::from_binding(error.into()))?;
        run(&self.engine)
    }
}

fn options_bytes(options_json: Option<&str>) -> &[u8] {
    options_json.unwrap_or_default().as_bytes()
}

fn binding_engine_for_transport(options_json: &[u8]) -> Result<BindingEngine, BindingError> {
    BindingEngine::from_options(options_json)
}

fn native_transport_capability_surface() -> ArtifactCapabilitySurface {
    #[cfg(feature = "svg")]
    let text_measurement = TextMeasurementProviderProjection::PreserveCompiled;
    #[cfg(not(feature = "svg"))]
    let text_measurement = TextMeasurementProviderProjection::VendoredOnly;

    merman_bindings_core::binding_transport_capability_surface()
        .project_to_descriptor_target("native", text_measurement)
        .expect("the UniFFI transport exposes a valid native capability surface")
}

fn execute_once_operation(
    request: &MermanOperationRequest,
) -> Result<MermanOperationResult, MermanError> {
    let result = merman_bindings_core::execute_once(binding_operation_request(
        request,
        options_bytes(request.options_json.as_deref()),
    ))
    .map_err(MermanError::from_binding)?;
    operation_result(result)
}

fn operation_request(
    operation_id: &str,
    source: String,
    options_json: Option<String>,
) -> MermanOperationRequest {
    MermanOperationRequest {
        operation_id: operation_id.to_string(),
        source,
        uri: None,
        options_json,
    }
}

fn operation_request_with_uri(
    operation_id: &str,
    source: String,
    uri: String,
    options_json: Option<String>,
) -> MermanOperationRequest {
    MermanOperationRequest {
        operation_id: operation_id.to_string(),
        source,
        uri: Some(uri),
        options_json,
    }
}

fn execute_operation(
    engine: &BindingEngine,
    request: &MermanOperationRequest,
    options_json: &[u8],
) -> Result<MermanOperationResult, MermanError> {
    let result = engine
        .execute(binding_operation_request(request, options_json))
        .map_err(MermanError::from_binding)?;
    operation_result(result)
}

fn binding_operation_request<'a>(
    request: &'a MermanOperationRequest,
    options_json: &'a [u8],
) -> merman_bindings_core::BindingOperationRequest<'a> {
    merman_bindings_core::BindingOperationRequest {
        operation_id: &request.operation_id,
        source: request.source.as_bytes(),
        uri: request.uri.as_deref().map(str::as_bytes),
        options_json,
    }
}

fn operation_result(
    result: merman_bindings_core::BindingOperationResult,
) -> Result<MermanOperationResult, MermanError> {
    let metadata_json = String::from_utf8(result.metadata_json).map_err(|error| {
        MermanError::internal(format!("operation metadata was not UTF-8: {error}"))
    })?;

    Ok(MermanOperationResult {
        operation_id: result.operation.operation_id().to_string(),
        media_type: result.media_type.to_string(),
        data: result.data,
        metadata_json,
    })
}

fn binary_operation_output(
    result: Result<MermanOperationResult, MermanError>,
) -> Result<Vec<u8>, MermanError> {
    result.map(|result| result.data)
}

fn string_operation_output(
    result: Result<MermanOperationResult, MermanError>,
) -> Result<String, MermanError> {
    let result = result?;
    String::from_utf8(result.data)
        .map_err(|error| MermanError::internal(format!("operation output was not UTF-8: {error}")))
}

fn validation_operation_output(
    result: Result<MermanOperationResult, MermanError>,
) -> Result<MermanValidationResult, MermanError> {
    let result = result?;
    validation_output(Ok(result.data))
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

fn string_vec(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn cached_string_vec(
    cache: &OnceLock<Vec<String>>,
    values: fn() -> &'static [&'static str],
) -> Vec<String> {
    cache.get_or_init(|| string_vec(values())).clone()
}

uniffi::setup_scaffolding!();

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "analysis")]
    use merman_bindings_core::ANALYSIS_FACTS_PAYLOAD_VERSION;
    use serde_json::Value;
    #[cfg(feature = "svg")]
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[cfg(feature = "svg")]
    use std::sync::{Barrier, Condvar, Mutex as StdMutex, Weak, mpsc};
    #[cfg(feature = "svg")]
    use std::thread;
    #[cfg(feature = "svg")]
    use std::time::Duration;

    fn engine() -> Arc<MermanEngine> {
        MermanEngine::new()
    }

    fn assert_missing_capability(error: &MermanError, expected_capability_id: &str) {
        let MermanError::Binding {
            code,
            code_name,
            kind,
            capability_id,
            ..
        } = error;
        assert_eq!(*code, BindingStatus::UnsupportedOperation.code());
        assert_eq!(
            code_name.as_str(),
            BindingStatus::UnsupportedOperation.code_name()
        );
        assert_eq!(*kind, MermanErrorKind::MissingCapability);
        assert_eq!(capability_id.as_deref(), Some(expected_capability_id));
    }

    #[cfg(feature = "svg")]
    fn assert_reentrant_error(error: &MermanError) {
        let MermanError::Binding {
            code,
            code_name,
            kind,
            message,
            ..
        } = error;
        assert_eq!(*code, BindingStatus::InvalidArgument.code());
        assert_eq!(
            code_name.as_str(),
            BindingStatus::InvalidArgument.code_name()
        );
        assert_eq!(*kind, MermanErrorKind::ReentrantCall);
        assert!(message.contains("cannot be re-entered from its callback"));
    }

    #[cfg(feature = "svg")]
    fn assert_busy_error(error: &MermanError) {
        let MermanError::Binding {
            code,
            code_name,
            kind,
            message,
            ..
        } = error;
        assert_eq!(*code, BindingStatus::Busy.code());
        assert_eq!(code_name.as_str(), BindingStatus::Busy.code_name());
        assert_eq!(*kind, MermanErrorKind::Busy);
        assert!(message.contains("reusable engine is busy"));
    }

    #[cfg(feature = "svg")]
    struct CountingTextMeasurer {
        calls: AtomicUsize,
        font_styles: StdMutex<Vec<String>>,
        operations: StdMutex<Vec<MermanTextMeasurementOperation>>,
    }

    #[cfg(feature = "svg")]
    struct FailingTextMeasurer {
        calls: AtomicUsize,
    }

    #[cfg(feature = "svg")]
    struct MissingTextMeasurer {
        calls: AtomicUsize,
    }

    #[cfg(feature = "svg")]
    struct FixedTextMeasurer {
        result: MermanTextMeasureResult,
    }

    #[cfg(feature = "svg")]
    struct BlockingFailingTextMeasurer {
        state: (StdMutex<BlockingFailingTextMeasurerState>, Condvar),
    }

    #[cfg(feature = "svg")]
    struct ReentrantRenderTextMeasurer {
        engine: StdMutex<Option<Weak<MermanReusableEngine>>>,
    }

    #[cfg(feature = "svg")]
    struct CrossThreadReentrantTextMeasurer {
        engine: StdMutex<Option<Weak<MermanReusableEngine>>>,
    }

    #[cfg(feature = "svg")]
    struct BlockingFailingTextMeasurerState {
        entered: bool,
        released: bool,
    }

    #[cfg(not(feature = "svg"))]
    struct UnavailableTextMeasurer;

    #[cfg(not(feature = "svg"))]
    impl MermanTextMeasurer for UnavailableTextMeasurer {
        fn measure(
            &self,
            _request: MermanTextMeasureRequest,
        ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
            Ok(None)
        }
    }

    #[cfg(feature = "svg")]
    impl CountingTextMeasurer {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                calls: AtomicUsize::new(0),
                font_styles: StdMutex::new(Vec::new()),
                operations: StdMutex::new(Vec::new()),
            })
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }

        fn saw_font_style(&self, expected: &str) -> bool {
            self.font_styles
                .lock()
                .unwrap()
                .iter()
                .any(|font_style| font_style == expected)
        }

        fn saw_operation(&self, expected: MermanTextMeasurementOperation) -> bool {
            self.operations.lock().unwrap().contains(&expected)
        }
    }

    #[cfg(feature = "svg")]
    impl FailingTextMeasurer {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                calls: AtomicUsize::new(0),
            })
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[cfg(feature = "svg")]
    impl MissingTextMeasurer {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                calls: AtomicUsize::new(0),
            })
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[cfg(feature = "svg")]
    impl BlockingFailingTextMeasurer {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                state: (
                    StdMutex::new(BlockingFailingTextMeasurerState {
                        entered: false,
                        released: false,
                    }),
                    Condvar::new(),
                ),
            })
        }

        fn wait_until_entered(&self) {
            let (lock, cvar) = &self.state;
            let mut state = lock.lock().unwrap();
            while !state.entered {
                state = cvar.wait(state).unwrap();
            }
        }

        fn release(&self) {
            let (lock, cvar) = &self.state;
            let mut state = lock.lock().unwrap();
            state.released = true;
            cvar.notify_all();
        }
    }

    #[cfg(feature = "svg")]
    impl ReentrantRenderTextMeasurer {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                engine: StdMutex::new(None),
            })
        }

        fn set_engine(&self, engine: &Arc<MermanReusableEngine>) {
            *self.engine.lock().unwrap() = Some(Arc::downgrade(engine));
        }
    }

    #[cfg(feature = "svg")]
    impl CrossThreadReentrantTextMeasurer {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                engine: StdMutex::new(None),
            })
        }

        fn set_engine(&self, engine: &Arc<MermanReusableEngine>) {
            *self.engine.lock().unwrap() = Some(Arc::downgrade(engine));
        }
    }

    #[cfg(feature = "svg")]
    impl MermanTextMeasurer for CountingTextMeasurer {
        fn measure(
            &self,
            request: MermanTextMeasureRequest,
        ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.font_styles
                .lock()
                .unwrap()
                .push(request.font_style.clone());
            self.operations.lock().unwrap().push(request.operation);
            assert!(request.font_size.is_finite());
            assert!(request.line_height.is_finite());
            let width = (request.text.chars().count() as f64 * 9.0).max(1.0);
            let height = request.line_height.max(1.0);
            let mut result = MermanTextMeasureResult {
                result_kind: MermanTextMeasurementResultKind::Metrics,
                width: 0.0,
                height: 0.0,
                length: 0.0,
                line_count: 0,
                bbox_left: None,
                bbox_right: None,
                raw_width: None,
            };
            match request.operation {
                MermanTextMeasurementOperation::Measure
                | MermanTextMeasurementOperation::Wrapped
                | MermanTextMeasurementOperation::MermaidCalculateTextDimensions => {
                    result.width = width;
                    result.height = height;
                    result.line_count = 1;
                }
                MermanTextMeasurementOperation::ComputedLength
                | MermanTextMeasurementOperation::SimpleBBoxWidth
                | MermanTextMeasurementOperation::RawBBoxWidth
                | MermanTextMeasurementOperation::BoundingClientRectWidth
                | MermanTextMeasurementOperation::TspanBBoxWidth
                | MermanTextMeasurementOperation::WrapProbeBBoxWidth
                | MermanTextMeasurementOperation::CanvasMeasureTextWidth => {
                    result.result_kind = MermanTextMeasurementResultKind::Length;
                    result.length = width;
                }
                MermanTextMeasurementOperation::TspanBBoxHeight
                | MermanTextMeasurementOperation::SimpleBBoxHeight
                | MermanTextMeasurementOperation::RawBBoxHeight => {
                    result.result_kind = MermanTextMeasurementResultKind::Length;
                    result.length = height;
                }
                MermanTextMeasurementOperation::CreateTextBBoxYOffset => {
                    result.result_kind = MermanTextMeasurementResultKind::Length;
                    result.length = -1.0;
                }
                MermanTextMeasurementOperation::CreateTextMiddleBBoxYOffset => {
                    result.result_kind = MermanTextMeasurementResultKind::Length;
                    result.length = -2.0;
                }
                MermanTextMeasurementOperation::BBoxX
                | MermanTextMeasurementOperation::BBoxXWithAsciiOverhang
                | MermanTextMeasurementOperation::TitleBBoxX => {
                    result.result_kind = MermanTextMeasurementResultKind::HorizontalExtents;
                    result.bbox_left = Some(width / 2.0);
                    result.bbox_right = Some(width / 2.0);
                }
                MermanTextMeasurementOperation::WrappedWithRawWidth => {
                    result.result_kind = MermanTextMeasurementResultKind::WrappedWithRawWidth;
                    result.width = width;
                    result.height = height;
                    result.line_count = 1;
                    result.raw_width = Some(width);
                }
            }
            Ok(Some(result))
        }
    }

    #[cfg(feature = "svg")]
    impl MermanTextMeasurer for FailingTextMeasurer {
        fn measure(
            &self,
            _request: MermanTextMeasureRequest,
        ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(MermanError::internal("test host measurer failed"))
        }
    }

    #[cfg(feature = "svg")]
    impl MermanTextMeasurer for MissingTextMeasurer {
        fn measure(
            &self,
            _request: MermanTextMeasureRequest,
        ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        }
    }

    #[cfg(feature = "svg")]
    impl MermanTextMeasurer for FixedTextMeasurer {
        fn measure(
            &self,
            _request: MermanTextMeasureRequest,
        ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
            Ok(Some(self.result))
        }
    }

    #[cfg(feature = "svg")]
    impl MermanTextMeasurer for BlockingFailingTextMeasurer {
        fn measure(
            &self,
            _request: MermanTextMeasureRequest,
        ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
            let (lock, cvar) = &self.state;
            let mut state = lock.lock().unwrap();
            state.entered = true;
            cvar.notify_all();
            while !state.released {
                state = cvar.wait(state).unwrap();
            }
            Err(MermanError::internal("blocked host measurer failed"))
        }
    }

    #[cfg(feature = "svg")]
    impl MermanTextMeasurer for ReentrantRenderTextMeasurer {
        fn measure(
            &self,
            _request: MermanTextMeasureRequest,
        ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
            let engine = self
                .engine
                .lock()
                .unwrap()
                .as_ref()
                .expect("reentrant render measurer should have an engine")
                .upgrade()
                .expect("reentrant engine should remain alive during its callback");
            let error = engine
                .render_svg("flowchart TD\nNested[Call]".to_string(), None)
                .expect_err("same-engine callback reentry must be rejected");
            assert_reentrant_error(&error);
            Err(error)
        }
    }

    #[cfg(feature = "svg")]
    impl MermanTextMeasurer for CrossThreadReentrantTextMeasurer {
        fn measure(
            &self,
            _request: MermanTextMeasureRequest,
        ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
            let engine = self
                .engine
                .lock()
                .unwrap()
                .as_ref()
                .expect("cross-thread reentrant measurer should have an engine")
                .upgrade()
                .expect("reentrant engine should remain alive during its callback");
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                let result = engine.render_svg("flowchart TD\nNested[Call]".to_string(), None);
                tx.send(result).ok();
            });

            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(Err(error)) => {
                    assert_reentrant_error(&error);
                    Err(error)
                }
                Ok(Ok(_)) => Err(MermanError::internal(
                    "cross-thread reentrant render succeeded",
                )),
                Err(_) => Err(MermanError::internal(
                    "cross-thread reentrant render did not finish",
                )),
            }
        }
    }

    #[test]
    fn engine_render_svg_matches_the_resolved_transport_surface() {
        let result = engine().render_svg("flowchart TD\nA[Hello] --> B[World]".to_string(), None);

        if native_transport_capability_surface()
            .runtime_capabilities()
            .has_operation("svg")
        {
            let svg = result.unwrap();
            assert!(svg.contains("<svg"));
            assert!(svg.contains("Hello"));
            assert!(svg.contains("World"));
        } else {
            let MermanError::Binding {
                code,
                code_name,
                kind,
                capability_id,
                resource,
                message,
            } = result.unwrap_err();
            assert_eq!(code, BindingStatus::UnsupportedOperation.code());
            assert_eq!(code_name, BindingStatus::UnsupportedOperation.code_name());
            assert_eq!(kind, MermanErrorKind::MissingCapability);
            assert_eq!(capability_id.as_deref(), Some("svg"));
            assert_eq!(resource, None);
            assert_eq!(message, "SVG rendering requires the svg feature");
        }
    }

    #[test]
    fn generic_operation_uses_the_descriptor_owned_semantic_path() {
        let result = engine()
            .execute(MermanOperationRequest {
                operation_id: "semantic-json".to_string(),
                source: "flowchart TD\nA[Hello] --> B[World]".to_string(),
                uri: None,
                options_json: Some(r#"{"runtime_policy":"deterministic"}"#.to_string()),
            })
            .unwrap();

        assert_eq!(result.operation_id, "semantic-json");
        assert_eq!(result.media_type, "application/json");
        assert!(
            String::from_utf8(result.data)
                .unwrap()
                .contains("flowchart-v2")
        );
        let metadata: Value = serde_json::from_str(&result.metadata_json).unwrap();
        assert_eq!(metadata["operation_id"], "semantic-json");
        assert_eq!(metadata["version"], 1);
        assert_eq!(metadata["runtime_policy"], "deterministic");
    }

    #[test]
    fn generic_transport_preserves_every_compiled_operation_result_contract() {
        let one_shot = engine();
        let reusable = MermanReusableEngine::new(None).unwrap();

        for operation_id in merman_bindings_core::compiled_operation_kind_ids() {
            let operation =
                merman_bindings_core::BindingOperationKind::from_id(operation_id).unwrap();
            let request = MermanOperationRequest {
                operation_id: operation_id.to_string(),
                source: "flowchart TD\nA --> B".to_string(),
                uri: operation
                    .requires_uri()
                    .then(|| "file:///diagram.mmd".to_string()),
                options_json: None,
            };
            let one_shot_result = one_shot
                .execute(request.clone())
                .unwrap_or_else(|error| panic!("one-shot `{operation_id}` failed: {error}"));
            let reusable_result = reusable
                .execute(request)
                .unwrap_or_else(|error| panic!("reusable `{operation_id}` failed: {error}"));

            assert_eq!(one_shot_result, reusable_result, "operation={operation_id}");
            assert_eq!(one_shot_result.operation_id, operation_id);
            assert_eq!(one_shot_result.media_type, operation.media_type());
            let metadata: Value = serde_json::from_str(&one_shot_result.metadata_json).unwrap();
            assert_eq!(metadata["operation_id"], operation_id);
            assert_eq!(metadata["media_type"], operation.media_type());
            assert_eq!(metadata["byte_length"], one_shot_result.data.len());
        }
    }

    #[cfg(feature = "svg")]
    #[test]
    fn generic_operation_exposes_the_svg_capability_plan() {
        let result = engine()
            .execute(MermanOperationRequest {
                operation_id: "svg-plan-json".to_string(),
                source: "flowchart TD\nA[Hello] --> B[World]".to_string(),
                uri: None,
                options_json: None,
            })
            .unwrap();

        assert_eq!(result.operation_id, "svg-plan-json");
        assert_eq!(result.media_type, "application/json");
        let plan: Value = serde_json::from_slice(&result.data).unwrap();
        assert_eq!(plan["planned_operation_id"], "svg");
        assert_eq!(plan["missing_capability_ids"], serde_json::json!([]));
        assert_eq!(plan["ready"], true);
    }

    #[test]
    fn generic_one_shot_native_policy_matches_the_owner_adapter_probe() {
        let request = MermanOperationRequest {
            operation_id: "semantic-json".to_string(),
            source: "flowchart TD\nA --> B".to_string(),
            uri: None,
            options_json: Some(r#"{"runtime_policy":"native"}"#.to_string()),
        };
        let compiled = merman_bindings_core::compiled_runtime_capabilities().system_adapter_ids;
        let missing = ["system-clock", "system-timezone", "system-random"]
            .into_iter()
            .find(|capability| !compiled.contains(capability));

        match missing {
            Some(expected_capability_id) => {
                let error = engine().execute(request).unwrap_err();
                assert_missing_capability(&error, expected_capability_id);
            }
            None => {
                let result = engine().execute(request).unwrap();
                let metadata: Value = serde_json::from_str(&result.metadata_json).unwrap();
                assert_eq!(metadata["runtime_policy"], "native");
            }
        }
    }

    #[test]
    fn generic_operation_distinguishes_unknown_operation_from_missing_capability() {
        let error = engine()
            .execute(MermanOperationRequest {
                operation_id: "not-an-operation".to_string(),
                source: "flowchart TD\nA --> B".to_string(),
                uri: None,
                options_json: None,
            })
            .expect_err("unknown operation must fail before dispatch");

        let MermanError::Binding {
            kind,
            capability_id,
            ..
        } = error;
        assert_eq!(kind, MermanErrorKind::UnknownOperation);
        assert_eq!(capability_id, None);
    }

    #[cfg(feature = "png")]
    #[test]
    fn engine_exposes_real_png_output() {
        let bytes = engine()
            .render_png("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
            .unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }

    #[cfg(feature = "jpeg")]
    #[test]
    fn engine_exposes_real_jpeg_output() {
        let bytes = engine()
            .render_jpeg("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
            .unwrap();
        assert_eq!(&bytes[..3], b"\xff\xd8\xff");
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn engine_exposes_real_pdf_output() {
        let bytes = engine()
            .render_pdf("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
            .unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
    }

    #[cfg(feature = "layout-cytoscape")]
    #[test]
    fn complete_uniffi_build_renders_architecture() {
        let svg = engine()
            .render_svg(
                "architecture-beta\n  service api(server)[API service]\n".to_string(),
                None,
            )
            .unwrap();

        assert!(svg.contains("<svg"));
    }

    #[test]
    fn engine_exposes_transport_owned_versions() {
        let engine = engine();

        assert_eq!(engine.binding_api_version(), UNIFFI_BINDING_API_VERSION);
        assert_eq!(engine.package_version(), env!("CARGO_PKG_VERSION"));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn uniffi_preserves_exact_host_measurement_operations() {
        assert_eq!(
            uniffi_measurement_operation(
                merman_bindings_core::TextMeasurementOperation::MermaidCalculateTextDimensions,
            ),
            MermanTextMeasurementOperation::MermaidCalculateTextDimensions
        );
        assert_eq!(
            uniffi_measurement_operation(
                merman_bindings_core::TextMeasurementOperation::CanvasMeasureTextWidth,
            ),
            MermanTextMeasurementOperation::CanvasMeasureTextWidth
        );
        assert_eq!(
            uniffi_measurement_operation(
                merman_bindings_core::TextMeasurementOperation::CreateTextMiddleBBoxYOffset,
            ),
            MermanTextMeasurementOperation::CreateTextMiddleBBoxYOffset
        );
        assert_eq!(
            uniffi_measurement_operation(
                merman_bindings_core::TextMeasurementOperation::RawBBoxHeight,
            ),
            MermanTextMeasurementOperation::RawBBoxHeight
        );
        assert_eq!(
            merman_bindings_core::HostTextMeasurementResultKind::expected_for_operation(
                merman_bindings_core::TextMeasurementOperation::MermaidCalculateTextDimensions,
            ),
            merman_bindings_core::HostTextMeasurementResultKind::Metrics
        );
        assert_eq!(
            merman_bindings_core::HostTextMeasurementResultKind::expected_for_operation(
                merman_bindings_core::TextMeasurementOperation::CanvasMeasureTextWidth,
            ),
            merman_bindings_core::HostTextMeasurementResultKind::Length
        );
        assert_eq!(
            merman_bindings_core::HostTextMeasurementResultKind::expected_for_operation(
                merman_bindings_core::TextMeasurementOperation::CreateTextMiddleBBoxYOffset,
            ),
            merman_bindings_core::HostTextMeasurementResultKind::Length
        );
        assert_eq!(
            merman_bindings_core::HostTextMeasurementResultKind::expected_for_operation(
                merman_bindings_core::TextMeasurementOperation::RawBBoxHeight,
            ),
            merman_bindings_core::HostTextMeasurementResultKind::Length
        );

        let callback = CountingTextMeasurer::new();
        let admission = BindingEngineAdmission::new(BindingEngineAdmissionMode::HostCallback);
        let _operation = admission.enter_operation().expect("operation admission");
        let host = UniffiHostTextMeasurer::new(callback, admission);
        let style = merman_bindings_core::TextStyle::default();
        let result = host
            .call_host(merman_bindings_core::HostTextMeasurementRequest {
                operation:
                    merman_bindings_core::TextMeasurementOperation::CreateTextMiddleBBoxYOffset,
                phase: merman_bindings_core::TextMeasurementPhase::SvgBBox,
                text: "middle",
                style: &style,
                max_width: None,
                wrap_mode: merman_bindings_core::WrapMode::SvgLike,
            })
            .expect("callback transport")
            .expect("handled middle y-offset");
        let merman_bindings_core::HostTextMeasurement::Length(result) = result else {
            panic!("middle y-offset must use the length result shape");
        };
        assert_eq!(result, -2.0);

        let raw_height = host
            .call_host(merman_bindings_core::HostTextMeasurementRequest {
                operation: merman_bindings_core::TextMeasurementOperation::RawBBoxHeight,
                phase: merman_bindings_core::TextMeasurementPhase::SvgBBox,
                text: "raw-height",
                style: &style,
                max_width: None,
                wrap_mode: merman_bindings_core::WrapMode::SvgLike,
            })
            .expect("callback transport")
            .expect("handled raw bbox height");
        let merman_bindings_core::HostTextMeasurement::Length(raw_height) = raw_height else {
            panic!("raw bbox height must use the length result shape");
        };
        assert!(raw_height > 0.0);
    }

    #[cfg(feature = "svg")]
    #[test]
    fn uniffi_checked_decoder_rejects_oversized_counts_and_half_extents() {
        let style = merman_bindings_core::TextStyle::default();
        let admission = BindingEngineAdmission::new(BindingEngineAdmissionMode::HostCallback);
        let _operation = admission.enter_operation().expect("operation admission");
        let request = |operation| merman_bindings_core::HostTextMeasurementRequest {
            operation,
            phase: merman_bindings_core::TextMeasurementPhase::SvgBBox,
            text: "x",
            style: &style,
            max_width: None,
            wrap_mode: merman_bindings_core::WrapMode::SvgLike,
        };

        let oversized = UniffiHostTextMeasurer::new(
            Arc::new(FixedTextMeasurer {
                result: MermanTextMeasureResult {
                    result_kind: MermanTextMeasurementResultKind::Metrics,
                    width: 1.0,
                    height: 1.0,
                    length: 0.0,
                    line_count: u64::MAX,
                    bbox_left: None,
                    bbox_right: None,
                    raw_width: None,
                },
            }),
            Arc::clone(&admission),
        );
        assert!(
            oversized
                .call_host(request(
                    merman_bindings_core::TextMeasurementOperation::Measure,
                ))
                .is_err()
        );

        let half_extents = UniffiHostTextMeasurer::new(
            Arc::new(FixedTextMeasurer {
                result: MermanTextMeasureResult {
                    result_kind: MermanTextMeasurementResultKind::HorizontalExtents,
                    width: 0.0,
                    height: 0.0,
                    length: 0.0,
                    line_count: 0,
                    bbox_left: Some(1.0),
                    bbox_right: None,
                    raw_width: None,
                },
            }),
            admission,
        );
        assert!(
            half_extents
                .call_host(request(
                    merman_bindings_core::TextMeasurementOperation::BBoxX,
                ))
                .is_err()
        );
    }

    #[cfg(feature = "svg")]
    #[test]
    fn engine_accepts_options_json() {
        let svg = engine()
            .render_svg(
                "flowchart TD\nA[Hello]".to_string(),
                Some(
                    r#"{
                        "environment": { "text_measurement": "deterministic" },
                        "svg": { "diagram_id": "uniffi diagram", "pipeline": "readable" }
                    }"#
                    .to_string(),
                ),
            )
            .unwrap();

        assert!(svg.contains("id=\"uniffi-diagram\""));
        assert!(svg.contains("data-merman-foreignobject"));
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn engine_renders_ascii() {
        let text = engine()
            .render_ascii("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
            .unwrap();

        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[cfg(feature = "svg")]
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

    #[cfg(feature = "svg")]
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

    #[cfg(feature = "analysis")]
    #[test]
    fn engine_returns_analysis_json() {
        let json: Value = serde_json::from_str(
            &engine()
                .analyze_json("flowchart TD\nA[Hello]".to_string(), None)
                .unwrap(),
        )
        .unwrap();

        assert_eq!(json["version"], 1);
        assert_eq!(json["valid"], true);
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn engine_returns_document_analysis_json() {
        let source = "# Example\n\n```mermaid\nflowchart TD\nA[Hello]\n```\n";
        let json: Value = serde_json::from_str(
            &engine()
                .analyze_document_json(
                    source.to_string(),
                    None,
                    "file:///tmp/example.md".to_string(),
                )
                .unwrap(),
        )
        .unwrap();

        assert_eq!(json["version"], 1);
        assert_eq!(json["source"]["kind"], "markdown");
        assert_eq!(json["valid"], true);
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn engine_returns_document_facts_json() {
        let source = "# Example\n\n```mermaid\nflowchart TD\nA[Hello]\n```\n";
        let json: Value = serde_json::from_str(
            &engine()
                .analyze_document_facts_json(
                    source.to_string(),
                    None,
                    "file:///tmp/example.md".to_string(),
                )
                .unwrap(),
        )
        .unwrap();

        assert_eq!(json["version"], ANALYSIS_FACTS_PAYLOAD_VERSION);
        assert_eq!(json["source"]["kind"], "markdown");
        assert_eq!(json["diagrams"][0]["source_id"], "mermaid-fence-1");
        assert_eq!(
            json["diagrams"][0]["syntax"]["fact_source"],
            "parser_complete"
        );
        assert!(
            json["diagrams"][0]["syntax"]["semantic_items"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| {
                    item["name"] == "A" && item["rename_policy"] == "flowchart_node_id"
                }))
        );

        let unavailable: Value = serde_json::from_str(
            &engine()
                .analyze_document_facts_json(
                    "```mermaid\nunknownDiagram\nPretendNode --> OtherNode\n```\n".to_string(),
                    None,
                    "file:///tmp/unknown.md".to_string(),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(unavailable["version"], ANALYSIS_FACTS_PAYLOAD_VERSION);
        assert_eq!(
            unavailable["diagrams"][0]["syntax"]["fact_source"],
            "unavailable"
        );
        assert_eq!(
            unavailable["diagrams"][0]["syntax"]["semantic_items"],
            serde_json::json!([])
        );
    }

    #[cfg(feature = "analysis")]
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
        let runtime_catalog_value: serde_json::Value =
            serde_json::from_str(&engine.runtime_catalog_json().unwrap()).unwrap();
        let runtime_capability_ids = runtime_catalog_value["capabilities"]["capability_ids"]
            .as_array()
            .expect("runtime capability IDs");
        let has_ascii = runtime_capability_ids.iter().any(|id| id == "ascii");
        let has_analysis = runtime_capability_ids.iter().any(|id| id == "analysis");
        let has_svg = runtime_capability_ids.iter().any(|id| id == "svg");

        assert!(
            engine
                .supported_diagrams()
                .contains(&"flowchart".to_string())
        );
        let ascii_capabilities = engine.ascii_capabilities();
        if has_ascii {
            let sequence = ascii_capabilities
                .iter()
                .find(|capability| capability.diagram_type == "sequence")
                .expect("expected UniFFI ASCII capabilities to include sequence");
            assert_eq!(sequence.support_level, "full");

            let gantt = ascii_capabilities
                .iter()
                .find(|capability| capability.diagram_type == "gantt")
                .expect("expected UniFFI ASCII capabilities to include gantt");
            assert_eq!(gantt.support_level, "summary");
            assert!(!gantt.summary_fallback);

            let class = ascii_capabilities
                .iter()
                .find(|capability| capability.diagram_type == "class")
                .expect("expected UniFFI ASCII capabilities to include class");
            assert_eq!(class.support_level, "partial");
            assert!(class.summary_fallback);
        } else {
            assert!(ascii_capabilities.is_empty());
        }
        assert!(engine.supported_themes().contains(&"default".to_string()));
        let presentation_catalog: serde_json::Value =
            serde_json::from_str(&engine.presentation_catalog_json().unwrap()).unwrap();
        assert_eq!(presentation_catalog["schema_version"], 1);
        if has_svg {
            assert!(
                presentation_catalog["theme_presets"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|preset| preset["id"] == "one-dark")
            );
            assert_eq!(presentation_catalog["profiles"][0]["id"], "merman-modern");
        } else {
            assert!(
                presentation_catalog["theme_presets"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            assert!(
                presentation_catalog["profiles"]
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
        }
        let capabilities = engine.diagram_family_capabilities();
        assert!(capabilities.iter().any(|capability| {
            capability.diagram_type == "flowchart"
                && capability.logical_family_kind == "flowchart"
                && capability.metadata_id.as_deref() == Some("flowchart")
                && capability.render_model_kind.as_deref() == Some("flowchart")
                && capability.has_detector
                && capability.has_semantic_parser
                && capability.has_editor_parser
                && capability.has_combined_parser
                && capability.has_render_parser
                && !capability.has_header
                && capability.config_namespace.as_deref() == Some("flowchart")
        }));
        if has_analysis {
            let lint_rules = engine.lint_rule_catalog().unwrap();
            assert!(lint_rules.iter().any(|rule| {
                rule.id == "merman.authoring.flowchart.explicit_direction"
                    && rule.origin == "merman_authoring"
                    && rule.default_profile == "recommended"
                    && rule
                        .evidence
                        .contains(&"docs/adr/0072-lint-rule-governance.md".to_string())
            }));
            assert!(
                engine
                    .configurable_lint_rule_catalog()
                    .unwrap()
                    .iter()
                    .all(|rule| rule.configurable && rule.category != "internal")
            );
        } else {
            let lint_error = engine.lint_rule_catalog().unwrap_err();
            assert_missing_capability(&lint_error, "analysis");
            let configurable_error = engine.configurable_lint_rule_catalog().unwrap_err();
            assert_missing_capability(&configurable_error, "analysis");
        }
        let runtime_catalog = runtime_catalog_value
            .as_object()
            .expect("runtime catalog must be an object");
        assert_eq!(
            runtime_catalog
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from([
                "capabilities",
                "metadata_ids",
                "options_schema_versions",
                "output_contracts",
                "package_version",
                "payload_schemas",
                "registry",
                "resources",
                "schema_version",
                "transport_api_version",
            ])
        );
        assert_eq!(
            runtime_catalog["schema_version"],
            merman_bindings_core::RUNTIME_CATALOG_SCHEMA_VERSION
        );
        assert_eq!(
            runtime_catalog["transport_api_version"],
            UNIFFI_BINDING_API_VERSION
        );
        assert_eq!(
            runtime_catalog["capabilities"],
            serde_json::to_value(native_transport_capability_surface().runtime_capabilities())
                .unwrap()
        );
        assert!(
            !runtime_catalog["capabilities"]["system_adapter_ids"]
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id == "system-timing")
        );
        assert_eq!(
            runtime_catalog["capabilities"]["capability_ids"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|id| id == "svg")),
            has_svg
        );
        assert!(runtime_catalog.get("features").is_none());
        assert_eq!(
            runtime_catalog["options_schema_versions"],
            serde_json::json!([merman_bindings_core::BINDING_OPTIONS_SCHEMA_VERSION])
        );
        assert!(
            runtime_catalog["payload_schemas"]
                .as_array()
                .is_some_and(|schemas| schemas
                    .iter()
                    .any(|schema| schema["id"] == "binding-result"))
        );
        assert!(
            runtime_catalog["metadata_ids"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|id| id == "diagram-family-capabilities"))
        );
        assert_eq!(
            runtime_catalog["resources"]["general_binding_default_profile"],
            "interactive"
        );
    }

    #[test]
    fn typed_resource_options_builder_uses_the_shared_descriptor() {
        let json = resource_options_json(
            Some(MermanResourceProfile::Constrained),
            vec![MermanResourceLimitOverride {
                id: MermanResourceOverrideId::MaxSourceBytes,
                value: 4096,
            }],
        )
        .unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["version"], 2);
        assert_eq!(value["resources"]["profile"], "constrained");
        assert_eq!(value["resources"]["limits"]["max_source_bytes"], 4096);

        let inherited = resource_options_json(None, Vec::new()).unwrap();
        assert_eq!(inherited, r#"{"version":2}"#);
    }

    #[cfg(feature = "svg")]
    #[test]
    fn reusable_engine_reuses_options() {
        let reusable = MermanReusableEngine::new(Some(
            r#"{
                "environment": { "text_measurement": "deterministic" },
                "svg": { "diagram_id": "uniffi reusable", "pipeline": "readable" }
            }"#
            .to_string(),
        ))
        .unwrap();

        let svg = reusable
            .render_svg("flowchart TD\nA[Hello]".to_string(), None)
            .unwrap();
        assert!(svg.contains("id=\"uniffi-reusable\""));
        assert!(svg.contains("data-merman-foreignobject"));

        let request_svg = reusable
            .render_svg(
                "flowchart TD\nA[Hello]".to_string(),
                Some(r#"{"svg":{"diagram_id":"request override"}}"#.to_string()),
            )
            .unwrap();
        assert!(request_svg.contains("id=\"request-override\""));
        assert!(request_svg.contains("data-merman-foreignobject"));

        let baseline_svg = reusable
            .render_svg("flowchart TD\nA[Hello]".to_string(), None)
            .unwrap();
        assert!(baseline_svg.contains("id=\"uniffi-reusable\""));
    }

    #[test]
    fn reusable_engine_rejects_request_runtime_policy() {
        let reusable = MermanReusableEngine::new(None).unwrap();
        let error = reusable
            .execute(MermanOperationRequest {
                operation_id: "semantic-json".to_string(),
                source: "flowchart TD\nA --> B".to_string(),
                uri: None,
                options_json: Some(r#"{"runtime_policy":"deterministic"}"#.to_string()),
            })
            .unwrap_err();
        let MermanError::Binding {
            code,
            code_name,
            message,
            ..
        } = error;
        assert_eq!(code, BindingStatus::OptionsJsonError.code());
        assert_eq!(code_name, BindingStatus::OptionsJsonError.code_name());
        assert!(message.contains("cannot set runtime_policy"));
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn reusable_engine_returns_document_analysis_json() {
        let reusable = MermanReusableEngine::new(Some(
            r#"{ "version": 2, "analysis": { "lint": { "profile": "strict" } } }"#.to_string(),
        ))
        .unwrap();
        let source = "# Example\n\n```mermaid\nflowchart TD\nA[Hello]\n```\n";
        let json: Value = serde_json::from_str(
            &reusable
                .analyze_document_json(
                    source.to_string(),
                    None,
                    "file:///tmp/example.md".to_string(),
                )
                .unwrap(),
        )
        .unwrap();

        assert_eq!(json["source"]["kind"], "markdown");
        assert_eq!(json["valid"], true);
    }

    #[cfg(feature = "svg")]
    #[test]
    fn reusable_engine_uses_host_text_measurer() {
        let measurer = CountingTextMeasurer::new();
        let reusable = MermanReusableEngine::with_text_measurer(None, measurer.clone()).unwrap();

        let svg = reusable
            .render_svg(
                "flowchart TD\nA[Measured label] --> B[Done]\nclassDef emphasized font-style:italic\nclass A emphasized"
                    .to_string(),
                Some(r#"{"svg":{"diagram_id":"request-measured"}}"#.to_string()),
            )
            .unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("id=\"request-measured\""));
        assert!(measurer.calls() > 0);
        assert!(measurer.saw_font_style("italic"));
        assert!(measurer.saw_operation(MermanTextMeasurementOperation::Wrapped));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn callback_free_reusable_engine_admits_concurrent_operations() {
        let reusable = MermanReusableEngine::new(None).unwrap();
        let entered = Arc::new(Barrier::new(3));
        let release = Arc::new(Barrier::new(3));
        let handles = (0..2)
            .map(|_| {
                let reusable = reusable.clone();
                let entered = Arc::clone(&entered);
                let release = Arc::clone(&release);
                thread::spawn(move || {
                    reusable.with_reusable_operation(|_| {
                        entered.wait();
                        release.wait();
                        Ok(())
                    })
                })
            })
            .collect::<Vec<_>>();

        entered.wait();
        release.wait();
        for handle in handles {
            handle.join().unwrap().unwrap();
        }
    }

    #[cfg(feature = "svg")]
    #[test]
    fn reusable_engine_falls_back_when_host_text_measurer_errors() {
        let measurer = FailingTextMeasurer::new();
        let reusable = MermanReusableEngine::with_text_measurer(None, measurer.clone()).unwrap();

        let svg = reusable
            .render_svg(
                "flowchart TD\nA[Measured label] --> B[Done]".to_string(),
                None,
            )
            .unwrap();

        assert!(svg.contains("<svg"));
        assert!(measurer.calls() > 0);
    }

    #[cfg(feature = "svg")]
    #[test]
    fn reusable_engine_layout_falls_back_when_host_text_measurer_errors() {
        let measurer = FailingTextMeasurer::new();
        let reusable = MermanReusableEngine::with_text_measurer(None, measurer.clone()).unwrap();

        let layout = reusable
            .layout_json(
                "flowchart TD\nA[Measured label] --> B[Done]".to_string(),
                None,
            )
            .unwrap();

        let layout: Value = serde_json::from_str(&layout).unwrap();
        assert!(layout.get("layout").is_some());
        assert!(measurer.calls() > 0);
    }

    #[cfg(feature = "svg")]
    #[test]
    fn reusable_engine_falls_back_when_host_text_measurer_returns_none() {
        let measurer = MissingTextMeasurer::new();
        let reusable = MermanReusableEngine::with_text_measurer(None, measurer.clone()).unwrap();

        let svg = reusable
            .render_svg(
                "flowchart TD\nA[Measured label] --> B[Done]".to_string(),
                None,
            )
            .unwrap();

        assert!(svg.contains("<svg"));
        assert!(measurer.calls() > 0);
    }

    #[cfg(feature = "svg")]
    #[test]
    fn callback_reusable_engine_serializes_operation_admission() {
        let reusable =
            MermanReusableEngine::with_text_measurer(None, CountingTextMeasurer::new()).unwrap();
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let first = reusable.clone();
        let first_entered = Arc::clone(&entered);
        let first_release = Arc::clone(&release);
        let first_handle = thread::spawn(move || {
            first.with_reusable_operation(|_| {
                first_entered.wait();
                first_release.wait();
                Ok(())
            })
        });

        entered.wait();
        let error = reusable
            .parse_json("flowchart TD\nA --> B".to_string(), None)
            .expect_err("a callback engine must reject a competing operation");
        assert_busy_error(&error);
        release.wait();
        first_handle.join().unwrap().unwrap();
        reusable
            .parse_json("flowchart TD\nA --> B".to_string(), None)
            .expect("operation admission must be released when the first call returns");
    }

    #[cfg(feature = "svg")]
    #[test]
    fn reusable_engines_isolate_active_host_callbacks() {
        let blocking_measurer = BlockingFailingTextMeasurer::new();
        let first =
            MermanReusableEngine::with_text_measurer(None, blocking_measurer.clone()).unwrap();
        let first_render = first.clone();
        let first_handle = thread::spawn(move || {
            first_render.render_svg(
                "flowchart TD\nA[Measured label] --> B[Done]".to_string(),
                None,
            )
        });

        blocking_measurer.wait_until_entered();

        let second = MermanReusableEngine::new(None).unwrap();
        let (second_done_tx, second_done_rx) = mpsc::channel();
        let second_handle = thread::spawn(move || {
            second_done_tx
                .send(second.render_svg(
                    "flowchart TD\nC[Independent] --> D[Engine]".to_string(),
                    None,
                ))
                .unwrap();
        });
        let second_result = second_done_rx.recv_timeout(Duration::from_secs(1));

        blocking_measurer.release();

        let first_svg = first_handle.join().unwrap().unwrap();
        second_handle.join().unwrap();
        let second_svg = second_result
            .expect("independent engine render must not wait for another engine callback")
            .expect("independent engine render must not be rejected by another engine callback");
        assert!(first_svg.contains("<svg"));
        assert!(second_svg.contains("<svg"));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn reusable_engine_conservatively_rejects_independent_call_during_active_callback() {
        let blocking_measurer = BlockingFailingTextMeasurer::new();
        let reusable =
            MermanReusableEngine::with_text_measurer(None, blocking_measurer.clone()).unwrap();
        let first_render = reusable.clone();
        let first_handle = thread::spawn(move || {
            first_render.render_svg(
                "flowchart TD\nA[Measured label] --> B[Done]".to_string(),
                None,
            )
        });

        blocking_measurer.wait_until_entered();

        let independent_call = reusable.clone();
        let (done_tx, done_rx) = mpsc::channel();
        let independent_handle = thread::spawn(move || {
            done_tx
                .send(independent_call.render_svg(
                    "flowchart TD\nC[Independent] --> D[Same engine]".to_string(),
                    None,
                ))
                .unwrap();
        });
        let independent_result = done_rx.recv_timeout(Duration::from_secs(1));

        blocking_measurer.release();

        let first_svg = first_handle.join().unwrap().unwrap();
        independent_handle.join().unwrap();
        let error = independent_result
            .expect("same-engine call must fail instead of waiting for the active callback")
            .expect_err("same-engine call must be conservatively rejected");
        assert_reentrant_error(&error);
        assert!(first_svg.contains("<svg"));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn reusable_engine_rejects_same_engine_render_reentry_from_callback() {
        let measurer = ReentrantRenderTextMeasurer::new();
        let reusable = MermanReusableEngine::with_text_measurer(None, measurer.clone()).unwrap();
        measurer.set_engine(&reusable);

        let svg = reusable
            .render_svg(
                "flowchart TD\nA[Measured label] --> B[Done]".to_string(),
                None,
            )
            .unwrap();

        assert!(svg.contains("<svg"));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn reusable_engine_rejects_cross_thread_same_engine_render_reentry_without_blocking() {
        let measurer = CrossThreadReentrantTextMeasurer::new();
        let reusable = MermanReusableEngine::with_text_measurer(None, measurer.clone()).unwrap();
        measurer.set_engine(&reusable);

        let svg = reusable
            .render_svg(
                "flowchart TD\nA[Measured label] --> B[Done]".to_string(),
                None,
            )
            .unwrap();

        assert!(svg.contains("<svg"));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn reusable_engine_keeps_callback_alive_until_in_flight_operation_returns() {
        let measurer = BlockingFailingTextMeasurer::new();
        let weak_measurer = Arc::downgrade(&measurer);
        let reusable = MermanReusableEngine::with_text_measurer(None, measurer.clone()).unwrap();
        let render_engine = reusable.clone();
        let render = thread::spawn(move || {
            render_engine.render_svg(
                "flowchart TD\nA[Measured label] --> B[Done]".to_string(),
                None,
            )
        });

        measurer.wait_until_entered();
        drop(measurer);
        drop(reusable);
        let retained_measurer = weak_measurer
            .upgrade()
            .expect("the in-flight operation must retain its callback");
        retained_measurer.release();
        drop(retained_measurer);
        assert!(render.join().unwrap().unwrap().contains("<svg"));
        assert!(
            weak_measurer.upgrade().is_none(),
            "the callback must be released after the operation and final engine owner return"
        );
    }

    #[cfg(not(feature = "svg"))]
    #[test]
    fn text_measurer_api_remains_visible_and_reports_missing_svg_capability() {
        let constructor_error =
            match MermanReusableEngine::with_text_measurer(None, Arc::new(UnavailableTextMeasurer))
            {
                Ok(_) => panic!("text-measurer constructor must require svg"),
                Err(error) => error,
            };
        assert_missing_capability(&constructor_error, "svg");

        let engine_error = match engine()
            .reusable_engine_with_text_measurer(None, Arc::new(UnavailableTextMeasurer))
        {
            Ok(_) => panic!("engine text-measurer constructor must require svg"),
            Err(error) => error,
        };
        assert_missing_capability(&engine_error, "svg");
    }

    #[cfg(feature = "svg")]
    #[test]
    fn uniffi_error_preserves_structured_resource_details() {
        let error = MermanError::from_binding(BindingError::resource_limit(
            "embedded_image_decode",
            "max_embedded_image_bytes",
            5,
            4,
            "constrained",
            "embedded image is too large",
        ));
        let MermanError::Binding { resource, .. } = error;
        assert_eq!(
            resource,
            Some(MermanResourceErrorDetails {
                cause: "ceiling".to_string(),
                limit_id: "max_embedded_image_bytes".to_string(),
                phase: "embedded_image_decode".to_string(),
                actual: 5,
                max: 4,
                profile: "constrained".to_string(),
            })
        );

        let error = MermanError::from_binding(BindingError::resource_limit_with_cause(
            "arithmetic_overflow",
            "layout_model",
            "max_layout_work_units",
            u64::MAX,
            800_000,
            "interactive",
            "layout work accounting overflowed",
        ));
        let MermanError::Binding { resource, .. } = error;
        assert_eq!(
            resource.expect("resource details").cause,
            "arithmetic_overflow"
        );
    }

    #[cfg(feature = "svg")]
    #[test]
    fn engine_error_preserves_binding_status() {
        let err = engine()
            .render_svg("flowchart TD\nA".to_string(), Some("{".to_string()))
            .unwrap_err();

        let MermanError::Binding {
            code,
            code_name,
            kind,
            capability_id,
            resource,
            message,
        } = err;
        assert_eq!(code, BindingStatus::OptionsJsonError.code());
        assert_eq!(code_name, BindingStatus::OptionsJsonError.code_name());
        assert_eq!(kind, MermanErrorKind::Generic);
        assert_eq!(capability_id, None);
        assert_eq!(resource, None);
        assert!(message.contains("invalid options_json"));
    }
}

#![forbid(unsafe_code)]
// UniFFI exports one structured error by value across every generated method. Boxing it would
// change the foreign-language API without reducing the serialized error payload.
#![allow(clippy::result_large_err)]

//! UniFFI bindings for `merman`.
//!
//! This crate exposes an idiomatic generated-binding surface over `merman-bindings-core`. It does
//! not replace the canonical C ABI in `merman-ffi`.

use merman::OperationControl;
use merman_bindings_core::BindingEngineServices;
use merman_bindings_core::{
    BindingAsciiOutputPlan, BindingEngine, BindingEngineAdmission, BindingEngineAdmissionError,
    BindingEngineAdmissionMode, BindingError, BindingErrorKind, BindingOperationMetadata,
    BindingOutputPlan, BindingStatus, OperationKey, ValidatedArtifactContract,
};
#[cfg(feature = "svg")]
use merman_bindings_core::{
    BindingIconRegistry, HostTextMeasurementError, HostTextMeasurer, IconPack, build_icon_registry,
};
use serde_json::Value;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// Version of the direct UniFFI binding API.
///
/// This version belongs to the generated UniFFI surface only. It is intentionally independent
/// from both the C ABI and the text-measurement protocol, whose versions are owned by their
/// respective descriptors.
pub const UNIFFI_BINDING_API_VERSION: u32 = 8;

// UniFFI method checksums do not protect every nested record layout. API 8 replaces the API 7
// probe before decoding the nullable ASCII presentation and overflow fields.
#[cfg(test)]
const UNIFFI_BINDING_API_OLD_VERSION_METHOD_CHECKSUMS: [u16; 2] = [60_120, 21_723];

static SUPPORTED_DIAGRAMS: OnceLock<Vec<String>> = OnceLock::new();
static ASCII_CAPABILITIES: OnceLock<Vec<MermanAsciiCapability>> = OnceLock::new();
static SUPPORTED_THEMES: OnceLock<Vec<String>> = OnceLock::new();
static ARTIFACT_CONTRACT: ValidatedArtifactContract =
    merman_bindings_core::native_sdk_artifact_contract!(UniFfi);

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

/// Structured cancellation details preserved across the generated binding boundary.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanCancelledDetails {
    pub reason: String,
    pub phase: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanDiagnosticSpan {
    pub start: u64,
    pub end: u64,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanDiagnosticErrorDetails {
    pub code: String,
    pub span: Option<MermanDiagnosticSpan>,
    pub field: Option<String>,
    pub diagram_type: Option<String>,
    pub requested_max_width: Option<u64>,
    pub actual_width: Option<u64>,
    pub width_profile: Option<String>,
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanIconRegistryErrorDetails {
    pub kind_id: String,
    pub pack_index: Option<u64>,
    pub registration_name: Option<String>,
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
        diagnostic: Option<MermanDiagnosticErrorDetails>,
        icon_registry: Option<MermanIconRegistryErrorDetails>,
        cancellation: Option<MermanCancelledDetails>,
        /// Complete core-owned details object, including versioned theme-authoring diagnostics.
        details_json: Option<String>,
        message: String,
    },
}

impl MermanError {
    pub fn from_binding(error: BindingError) -> Self {
        let status = error.status();
        let resource = error
            .resource_details()
            .map(|details| MermanResourceErrorDetails {
                cause: details.cause.as_str().to_string(),
                limit_id: details.limit_id.to_string(),
                phase: details.phase.to_string(),
                actual: details.actual,
                max: details.max,
                profile: details.profile.to_string(),
            });
        let diagnostic = error
            .diagnostic_details()
            .map(|details| MermanDiagnosticErrorDetails {
                code: details.code.clone(),
                span: details.span.map(|span| MermanDiagnosticSpan {
                    start: span.start,
                    end: span.end,
                    kind: span.kind.to_string(),
                }),
                field: details.field.clone(),
                diagram_type: details.diagram_type.clone(),
                requested_max_width: details.requested_max_width,
                actual_width: details.actual_width,
                width_profile: details.width_profile.clone(),
                fallback_reason: details.fallback_reason.clone(),
            });
        let icon_registry =
            error
                .icon_registry_details()
                .map(|details| MermanIconRegistryErrorDetails {
                    kind_id: details.kind_id.to_string(),
                    pack_index: details.pack_index,
                    registration_name: details.registration_name.clone(),
                });
        let cancellation = error
            .cancellation_details()
            .map(|details| MermanCancelledDetails {
                reason: details.reason.to_string(),
                phase: details.phase.to_string(),
            });
        let details_json = match merman_bindings_core::binding_error_js_details_json(&error)
            .and_then(|details| {
                details
                    .map(|details| serde_json::to_string(&details))
                    .transpose()
            }) {
            Ok(details_json) => details_json,
            Err(error) => {
                return Self::internal(format!("failed to encode binding error details: {error}"));
            }
        };
        Self::Binding {
            code: status.code(),
            code_name: status.code_name().to_string(),
            kind: error.kind().into(),
            capability_id: error.capability_id().map(str::to_string),
            resource,
            diagnostic,
            icon_registry,
            cancellation,
            details_json,
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
            diagnostic: None,
            icon_registry: None,
            cancellation: None,
            details_json: None,
            message: message.into(),
        }
    }

    fn invalid_argument(message: impl Into<String>) -> Self {
        Self::from_binding(BindingError::invalid_argument(message))
    }
}

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
pub struct Merman;

/// An immutable reusable engine with constructor-owned services and transport-neutral admission.
///
/// Callback-free engines admit concurrent operations. Engines constructed with a host text
/// measurer serialize operations, and calls made while that engine's callback is active fail with
/// [`MermanErrorKind::ReentrantCall`].
#[derive(uniffi::Object)]
pub struct MermanEngine {
    engine: Mutex<Option<Arc<BindingEngine>>>,
    admission: Arc<BindingEngineAdmission>,
}

#[derive(Debug, uniffi::Object)]
pub struct MermanIconPack {
    json: String,
    registration_name: Option<String>,
}

#[derive(uniffi::Object)]
pub struct MermanIconRegistry {
    #[cfg(feature = "svg")]
    registry: BindingIconRegistry,
}

/// Cloneable operation-scoped cancellation and relative-deadline control.
///
/// Passing the same object to a request and retaining another foreign-language reference shares
/// one atomic control state. Cancellation is cooperative: opaque host callbacks are observed only
/// when they return to a renderer checkpoint.
#[derive(Debug, uniffi::Object)]
pub struct MermanOperationControl {
    control: OperationControl,
}

#[uniffi::export]
impl MermanOperationControl {
    /// Creates a control with an optional relative timeout in milliseconds.
    #[uniffi::constructor]
    pub fn new(timeout_ms: Option<u64>) -> Arc<Self> {
        let control = OperationControl::new();
        if let Some(timeout_ms) = timeout_ms {
            control.set_deadline(Duration::from_millis(timeout_ms));
        }
        Arc::new(Self { control })
    }

    /// Requests cooperative cancellation. This method is safe to call from another thread while
    /// a request is executing.
    pub fn cancel(&self) {
        self.control.cancel();
    }

    /// Reports whether cancellation was requested on this shared control.
    pub fn is_cancelled(&self) -> bool {
        self.control.is_cancelled()
    }
}

impl MermanOperationControl {
    fn clone_control(&self) -> OperationControl {
        self.control.clone()
    }
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
/// engine's baseline for this operation. `control` optionally supplies a shared cancellation and
/// deadline context; execution clones it before entering the synchronous binding path.
#[derive(Debug, Clone, uniffi::Record)]
pub struct MermanOperationRequestV4 {
    pub operation_id: String,
    pub source: String,
    pub uri: Option<String>,
    pub options_json: Option<String>,
    pub control: Option<Arc<MermanOperationControl>>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MermanRasterOutputPlan {
    pub requested_width_px: f64,
    pub requested_height_px: f64,
    pub width_px: u32,
    pub height_px: u32,
    pub requested_scale: f64,
    pub effective_scale: f64,
    pub limited: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MermanPdfFilterImagesOutputPlan {
    pub filtered_groups: u64,
    pub requested_scale: f64,
    pub effective_scale: f64,
    pub requested_image_pixels: u64,
    pub effective_image_pixels: u64,
    pub limited: bool,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MermanAsciiOutputPlan {
    pub schema_version: u16,
    pub family: String,
    pub projection: String,
    pub encoding: String,
    pub primary_width: Option<u64>,
    pub primary_height: Option<u64>,
    pub emitted_width: u64,
    pub emitted_height: u64,
    pub width_profile: String,
    pub layout_profile: Option<String>,
    pub requested_layout_profile: String,
    pub requested_graph_presentation: String,
    pub compact_attempted: bool,
    pub requested_max_width: Option<u64>,
    pub primary_overflowed: Option<bool>,
    pub emitted_overflowed: bool,
    pub outcome: String,
    pub fallback_capability: String,
    pub fallback_attempted: bool,
    pub fallback_reason: Option<String>,
    pub trimmed: bool,
    pub lossiness: String,
}

/// Open output-plan projection for foreign languages.
///
/// Consumers switch on `kind`. Known payloads are optional conveniences; future kinds remain
/// lossless through `raw_json` without adding a closed UniFFI enum variant.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MermanOutputPlan {
    pub kind: String,
    pub raw_json: String,
    pub raster: Option<MermanRasterOutputPlan>,
    pub pdf_filter_images: Option<MermanPdfFilterImagesOutputPlan>,
    pub ascii: Option<MermanAsciiOutputPlan>,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MermanOperationMetadata {
    pub version: u32,
    pub operation_id: String,
    pub media_type: String,
    pub runtime_policy: String,
    pub byte_length: u64,
    pub output_plan: Option<MermanOutputPlan>,
    pub raw_json: String,
}

/// Binary-safe output returned by one-shot and reusable execution.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MermanOperationResult {
    pub operation_id: String,
    pub media_type: String,
    pub data: Vec<u8>,
    pub metadata: MermanOperationMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanDiagramFamilyCapability {
    pub diagram_type: String,
    pub family_id: String,
    pub metadata_id: Option<String>,
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
    pub tags: Vec<String>,
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
    pub semantic_coverage: Option<String>,
    pub primary_projection: String,
    pub structured_text_fallback: bool,
    pub layout_profiles: Vec<String>,
    pub graph_presentations: Vec<String>,
    pub width_profiles: Vec<String>,
    pub encodings: Vec<String>,
    pub fallback_encodings: Vec<String>,
    /// Compatibility view derived from semantic coverage and the primary projection.
    pub support_level: String,
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

/// Immutable constructor-owned services for [`MermanEngine`].
///
/// A service bundle may be shared across engine constructions. Icon registries are already sealed,
/// and the text measurer is retained without being invoked during construction.
#[derive(uniffi::Object)]
pub struct MermanEngineServices {
    icon_registry: Option<Arc<MermanIconRegistry>>,
    text_measurer: Option<Arc<dyn MermanTextMeasurer>>,
}

#[uniffi::export]
impl MermanIconPack {
    #[uniffi::constructor]
    pub fn new(json: String, registration_name: Option<String>) -> Arc<Self> {
        Arc::new(Self {
            json,
            registration_name,
        })
    }

    pub fn json(&self) -> String {
        self.json.clone()
    }

    pub fn registration_name(&self) -> Option<String> {
        self.registration_name.clone()
    }
}

#[uniffi::export]
impl MermanEngineServices {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            icon_registry: None,
            text_measurer: None,
        })
    }

    pub fn with_icon_registry(&self, icon_registry: Arc<MermanIconRegistry>) -> Arc<Self> {
        Arc::new(Self {
            icon_registry: Some(icon_registry),
            text_measurer: self.text_measurer.clone(),
        })
    }

    pub fn with_text_measurer(&self, text_measurer: Arc<dyn MermanTextMeasurer>) -> Arc<Self> {
        Arc::new(Self {
            icon_registry: self.icon_registry.clone(),
            text_measurer: Some(text_measurer),
        })
    }
}

#[uniffi::export]
impl MermanIconRegistry {
    /// Transactionally validates Iconify JSON packs and seals an immutable reusable registry.
    #[uniffi::constructor]
    pub fn from_packs(packs: Vec<Arc<MermanIconPack>>) -> Result<Arc<Self>, MermanError> {
        #[cfg(feature = "svg")]
        {
            let packs = packs
                .iter()
                .map(|pack| match pack.registration_name.as_deref() {
                    Some(name) => IconPack::new(pack.json.as_bytes()).with_registration_name(name),
                    None => IconPack::new(pack.json.as_bytes()),
                })
                .collect::<Vec<_>>();
            let registry = build_icon_registry(packs).map_err(MermanError::from_binding)?;
            Ok(Arc::new(Self { registry }))
        }
        #[cfg(not(feature = "svg"))]
        {
            if packs.is_empty() {
                return Ok(Arc::new(Self {}));
            }
            drop(packs);
            Err(missing_capability_error(
                "svg",
                "icon registries require the svg capability",
            ))
        }
    }

    pub fn len(&self) -> u64 {
        #[cfg(feature = "svg")]
        {
            u64::try_from(self.registry.len()).unwrap_or(u64::MAX)
        }
        #[cfg(not(feature = "svg"))]
        {
            0
        }
    }

    pub fn is_empty(&self) -> bool {
        #[cfg(feature = "svg")]
        {
            self.registry.is_empty()
        }
        #[cfg(not(feature = "svg"))]
        {
            true
        }
    }
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
        use merman_bindings_core::HostTextWhiteSpaceCode;

        let _callback_guard = self
            .admission
            .enter_callback()
            .map_err(|error| HostTextMeasurementError::new(error.to_string()))?;
        let transport = merman_bindings_core::host_text_measurement_transport_fields(request);
        let white_space = match transport.white_space {
            code if code == HostTextWhiteSpaceCode::Normal.external_code() => {
                MermanTextWhiteSpace::Normal
            }
            code if code == HostTextWhiteSpaceCode::Nowrap.external_code() => {
                MermanTextWhiteSpace::Nowrap
            }
            code if code == HostTextWhiteSpaceCode::BreakSpaces.external_code() => {
                MermanTextWhiteSpace::BreakSpaces
            }
            _ => {
                return Err(HostTextMeasurementError::new(
                    "unknown shared text white-space code",
                ));
            }
        };
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
            line_height: transport.line_height,
            letter_spacing: 0.0,
            word_spacing: 0.0,
            wrap_mode: uniffi_wrap_mode(request.wrap_mode),
            direction: MermanTextDirection::Auto,
            white_space,
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
        tags: rule.tags.into_iter().map(str::to_string).collect(),
        default_enabled: rule.default_enabled,
        default_profile: rule.default_profile.to_string(),
        origin: rule.origin.to_string(),
        configurable: rule.configurable,
        fixable: rule.fixable,
    }
}

#[uniffi::export]
impl Merman {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }

    pub fn binding_api_version_v8(&self) -> u32 {
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
        string_output(native_artifact_contract().runtime_catalog_json(UNIFFI_BINDING_API_VERSION))
    }

    /// Returns one catalog selected by this exact transport contract.
    pub fn metadata_json(&self, id: String) -> Result<String, MermanError> {
        string_output(native_artifact_contract().metadata_json(&id))
    }

    /// Returns the versioned theme catalog JSON projected to this native artifact.
    pub fn theme_catalog_json(&self) -> Result<String, MermanError> {
        string_output(native_artifact_contract().metadata_json("theme-catalog"))
    }

    /// Executes a descriptor-owned output operation with a fresh engine configuration.
    pub fn execute(
        &self,
        request: MermanOperationRequestV4,
    ) -> Result<MermanOperationResult, MermanError> {
        execute_once_operation(&request)
    }

    pub fn render_svg(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request(
            OperationKey::Svg,
            source,
            options_json,
        )))
    }

    pub fn render_png(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<Vec<u8>, MermanError> {
        self.execute_data(operation_request(OperationKey::Png, source, options_json))
    }

    pub fn render_png_result(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<MermanOperationResult, MermanError> {
        self.execute(operation_request(OperationKey::Png, source, options_json))
    }

    pub fn render_jpeg(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<Vec<u8>, MermanError> {
        self.execute_data(operation_request(OperationKey::Jpeg, source, options_json))
    }

    pub fn render_jpeg_result(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<MermanOperationResult, MermanError> {
        self.execute(operation_request(OperationKey::Jpeg, source, options_json))
    }

    pub fn render_pdf(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<Vec<u8>, MermanError> {
        self.execute_data(operation_request(OperationKey::Pdf, source, options_json))
    }

    pub fn render_pdf_result(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<MermanOperationResult, MermanError> {
        self.execute(operation_request(OperationKey::Pdf, source, options_json))
    }

    pub fn render_ascii(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request(
            OperationKey::Ascii,
            source,
            options_json,
        )))
    }

    pub fn render_ascii_result(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<MermanOperationResult, MermanError> {
        self.execute(operation_request(OperationKey::Ascii, source, options_json))
    }

    pub fn parse_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request(
            OperationKey::SemanticJson,
            source,
            options_json,
        )))
    }

    pub fn layout_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request(
            OperationKey::LayoutJson,
            source,
            options_json,
        )))
    }

    pub fn edge_geometry_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request(
            OperationKey::EdgeGeometryJson,
            source,
            options_json,
        )))
    }

    pub fn svg_plan_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request(
            OperationKey::SvgPlanJson,
            source,
            options_json,
        )))
    }

    pub fn analyze_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request(
            OperationKey::AnalysisJson,
            source,
            options_json,
        )))
    }

    pub fn analysis_facts_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request(
            OperationKey::AnalysisFactsJson,
            source,
            options_json,
        )))
    }

    pub fn analyze_document_json(
        &self,
        source: String,
        uri: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request_with_uri(
            OperationKey::DocumentAnalysisJson,
            source,
            uri,
            options_json,
        )))
    }

    pub fn analyze_document_facts_json(
        &self,
        source: String,
        uri: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request_with_uri(
            OperationKey::DocumentAnalysisFactsJson,
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
        validation_operation_output(self.execute_data(operation_request(
            OperationKey::ValidationJson,
            source,
            options_json,
        )))
    }

    pub fn supported_diagrams(&self) -> Vec<String> {
        cached_string_vec(
            &SUPPORTED_DIAGRAMS,
            merman_bindings_core::supported_diagrams,
        )
    }

    pub fn ascii_capabilities(&self) -> Vec<MermanAsciiCapability> {
        if !native_artifact_contract()
            .runtime_capabilities()
            .has_capability("ascii")
        {
            return Vec::new();
        }
        ASCII_CAPABILITIES
            .get_or_init(|| {
                merman_bindings_core::ascii_capabilities()
                    .into_iter()
                    .map(|capability| MermanAsciiCapability {
                        diagram_type: capability.diagram_type.to_string(),
                        display_name: capability.display_name.to_string(),
                        semantic_coverage: capability.semantic_coverage.map(str::to_string),
                        primary_projection: capability.primary_projection.to_string(),
                        structured_text_fallback: capability.structured_text_fallback,
                        layout_profiles: capability
                            .layout_profiles
                            .iter()
                            .map(|profile| (*profile).to_string())
                            .collect(),
                        graph_presentations: capability
                            .graph_presentations
                            .iter()
                            .map(|policy| (*policy).to_string())
                            .collect(),
                        width_profiles: capability
                            .width_profiles
                            .iter()
                            .map(|profile| (*profile).to_string())
                            .collect(),
                        encodings: capability
                            .encodings
                            .iter()
                            .map(|encoding| (*encoding).to_string())
                            .collect(),
                        fallback_encodings: capability
                            .fallback_encodings
                            .iter()
                            .map(|encoding| (*encoding).to_string())
                            .collect(),
                        support_level: capability.support_level.to_string(),
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
                family_id: capability.family_id.to_string(),
                metadata_id: capability.metadata_id.map(str::to_string),
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
        if !native_artifact_contract()
            .runtime_capabilities()
            .has_capability("analysis")
        {
            return Err(missing_capability_error(
                "analysis",
                "lint rule catalog requires the analysis capability",
            ));
        }
        Ok(merman_bindings_core::lint_rule_catalog()
            .map_err(MermanError::from_binding)?
            .into_iter()
            .map(uniffi_lint_rule)
            .collect())
    }

    pub fn configurable_lint_rule_catalog(
        &self,
    ) -> Result<Vec<MermanLintRuleCatalogEntry>, MermanError> {
        if !native_artifact_contract()
            .runtime_capabilities()
            .has_capability("analysis")
        {
            return Err(missing_capability_error(
                "analysis",
                "configurable lint rule catalog requires the analysis capability",
            ));
        }
        Ok(merman_bindings_core::configurable_lint_rule_catalog()
            .map_err(MermanError::from_binding)?
            .into_iter()
            .map(uniffi_lint_rule)
            .collect())
    }
}

impl Merman {
    fn execute_data(&self, request: MermanOperationRequestV4) -> Result<Vec<u8>, MermanError> {
        native_artifact_contract()
            .execute_once_data(binding_operation_request(
                &request,
                options_bytes(request.options_json.as_deref()),
            ))
            .map_err(MermanError::from_binding)
    }
}

#[uniffi::export]
impl MermanEngine {
    #[uniffi::constructor]
    pub fn new(
        options_json: Option<String>,
        services: Option<Arc<MermanEngineServices>>,
    ) -> Result<Arc<Self>, MermanError> {
        let admission_mode = if services
            .as_ref()
            .and_then(|services| services.text_measurer.as_ref())
            .is_some()
        {
            BindingEngineAdmissionMode::HostCallback
        } else {
            BindingEngineAdmissionMode::Concurrent
        };
        let admission = BindingEngineAdmission::new(admission_mode);
        let services = uniffi_engine_services(services.as_deref(), &admission)?;
        let engine = native_artifact_contract()
            .create_engine_with_services(options_bytes(options_json.as_deref()), services)
            .map_err(MermanError::from_binding)?;
        Ok(Arc::new(Self {
            engine: Mutex::new(Some(Arc::new(engine))),
            admission,
        }))
    }

    pub fn render_svg(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request(
            OperationKey::Svg,
            source,
            options_json,
        )))
    }

    pub fn render_png(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<Vec<u8>, MermanError> {
        self.execute_data(operation_request(OperationKey::Png, source, options_json))
    }

    pub fn render_png_result(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<MermanOperationResult, MermanError> {
        self.execute(operation_request(OperationKey::Png, source, options_json))
    }

    pub fn render_jpeg(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<Vec<u8>, MermanError> {
        self.execute_data(operation_request(OperationKey::Jpeg, source, options_json))
    }

    pub fn render_jpeg_result(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<MermanOperationResult, MermanError> {
        self.execute(operation_request(OperationKey::Jpeg, source, options_json))
    }

    pub fn render_pdf(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<Vec<u8>, MermanError> {
        self.execute_data(operation_request(OperationKey::Pdf, source, options_json))
    }

    pub fn render_pdf_result(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<MermanOperationResult, MermanError> {
        self.execute(operation_request(OperationKey::Pdf, source, options_json))
    }

    /// Executes an operation using the reusable baseline plus request-local option overrides.
    pub fn execute(
        &self,
        request: MermanOperationRequestV4,
    ) -> Result<MermanOperationResult, MermanError> {
        self.with_reusable_operation(|engine| {
            execute_operation(
                engine,
                &request,
                options_bytes(request.options_json.as_deref()),
            )
        })
    }

    /// Returns metadata projected through this reusable engine's resource policy.
    pub fn metadata_json(&self, id: String) -> Result<String, MermanError> {
        self.with_reusable_operation(|engine| string_output(engine.metadata_json(&id)))
    }

    pub fn theme_catalog_json(&self) -> Result<String, MermanError> {
        self.metadata_json("theme-catalog".to_string())
    }

    pub fn render_ascii(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request(
            OperationKey::Ascii,
            source,
            options_json,
        )))
    }

    pub fn render_ascii_result(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<MermanOperationResult, MermanError> {
        self.execute(operation_request(OperationKey::Ascii, source, options_json))
    }

    pub fn parse_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request(
            OperationKey::SemanticJson,
            source,
            options_json,
        )))
    }

    pub fn layout_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request(
            OperationKey::LayoutJson,
            source,
            options_json,
        )))
    }

    pub fn edge_geometry_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request(
            OperationKey::EdgeGeometryJson,
            source,
            options_json,
        )))
    }

    pub fn svg_plan_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request(
            OperationKey::SvgPlanJson,
            source,
            options_json,
        )))
    }

    pub fn analyze_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request(
            OperationKey::AnalysisJson,
            source,
            options_json,
        )))
    }

    pub fn analysis_facts_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request(
            OperationKey::AnalysisFactsJson,
            source,
            options_json,
        )))
    }

    pub fn analyze_document_json(
        &self,
        source: String,
        uri: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request_with_uri(
            OperationKey::DocumentAnalysisJson,
            source,
            uri,
            options_json,
        )))
    }

    pub fn analyze_document_facts_json(
        &self,
        source: String,
        uri: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_operation_output(self.execute_data(operation_request_with_uri(
            OperationKey::DocumentAnalysisFactsJson,
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
        validation_operation_output(self.execute_data(operation_request(
            OperationKey::ValidationJson,
            source,
            options_json,
        )))
    }

    /// Closes this engine without waiting for active operations.
    ///
    /// Busy and reentrant failures preserve the complete engine for retry. Success detaches the
    /// service graph under synchronization and drops foreign callbacks after releasing the lock.
    /// Repeated and concurrent close calls are idempotent.
    pub fn close(&self) -> Result<(), MermanError> {
        match self
            .admission
            .try_close_detaching(|| self.lock_engine().take())
        {
            Ok(()) => Ok(()),
            Err(BindingEngineAdmissionError::Closed) => Ok(()),
            Err(error) => Err(MermanError::from_binding(error.into())),
        }
    }
}

impl MermanEngine {
    fn execute_data(&self, request: MermanOperationRequestV4) -> Result<Vec<u8>, MermanError> {
        self.with_reusable_operation(|engine| {
            engine
                .execute_data(binding_operation_request(
                    &request,
                    options_bytes(request.options_json.as_deref()),
                ))
                .map_err(MermanError::from_binding)
        })
    }

    fn with_reusable_operation<T>(
        &self,
        run: impl FnOnce(&BindingEngine) -> Result<T, MermanError>,
    ) -> Result<T, MermanError> {
        let _operation = self
            .admission
            .enter_operation()
            .map_err(|error| MermanError::from_binding(error.into()))?;
        let engine = self
            .lock_engine()
            .as_ref()
            .map(Arc::clone)
            .ok_or_else(|| MermanError::invalid_argument("the reusable engine is closed"))?;
        run(&engine)
    }

    fn lock_engine(&self) -> std::sync::MutexGuard<'_, Option<Arc<BindingEngine>>> {
        self.engine
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn uniffi_engine_services(
    services: Option<&MermanEngineServices>,
    admission: &Arc<BindingEngineAdmission>,
) -> Result<BindingEngineServices, MermanError> {
    let Some(services) = services else {
        return Ok(BindingEngineServices::new());
    };

    #[cfg(feature = "svg")]
    {
        let mut result = BindingEngineServices::new();
        if let Some(registry) = services
            .icon_registry
            .as_ref()
            .filter(|registry| !registry.is_empty())
        {
            result = result.with_icon_registry(registry.registry.clone());
        }
        if let Some(measurer) = services.text_measurer.as_ref() {
            result = result.with_host_text_measurer(Arc::new(UniffiHostTextMeasurer::new(
                Arc::clone(measurer),
                Arc::clone(admission),
            )));
        }
        Ok(result)
    }
    #[cfg(not(feature = "svg"))]
    {
        let _ = admission;
        if services
            .icon_registry
            .as_ref()
            .is_some_and(|registry| !registry.is_empty())
            || services.text_measurer.is_some()
        {
            return Err(missing_capability_error(
                "svg",
                "constructor services require the svg capability",
            ));
        }
        Ok(BindingEngineServices::new())
    }
}

fn options_bytes(options_json: Option<&str>) -> &[u8] {
    options_json.unwrap_or_default().as_bytes()
}

fn native_artifact_contract() -> &'static ValidatedArtifactContract {
    &ARTIFACT_CONTRACT
}

fn execute_once_operation(
    request: &MermanOperationRequestV4,
) -> Result<MermanOperationResult, MermanError> {
    let result = native_artifact_contract()
        .execute_once(binding_operation_request(
            request,
            options_bytes(request.options_json.as_deref()),
        ))
        .map_err(MermanError::from_binding)?;
    operation_result(result)
}

fn operation_request(
    operation: OperationKey,
    source: String,
    options_json: Option<String>,
) -> MermanOperationRequestV4 {
    MermanOperationRequestV4 {
        operation_id: operation.id().to_string(),
        source,
        uri: None,
        options_json,
        control: None,
    }
}

fn operation_request_with_uri(
    operation: OperationKey,
    source: String,
    uri: String,
    options_json: Option<String>,
) -> MermanOperationRequestV4 {
    let mut request = operation_request(operation, source, options_json);
    request.uri = Some(uri);
    request
}

fn execute_operation(
    engine: &BindingEngine,
    request: &MermanOperationRequestV4,
    options_json: &[u8],
) -> Result<MermanOperationResult, MermanError> {
    let result = engine
        .execute(binding_operation_request(request, options_json))
        .map_err(MermanError::from_binding)?;
    operation_result(result)
}

fn binding_operation_request<'a>(
    request: &'a MermanOperationRequestV4,
    options_json: &'a [u8],
) -> merman_bindings_core::BindingOperationRequest<'a> {
    let binding_request = merman_bindings_core::BindingOperationRequest::new(
        &request.operation_id,
        request.source.as_bytes(),
    )
    .with_optional_uri(request.uri.as_deref().map(str::as_bytes))
    .with_options_json(options_json);
    match request
        .control
        .as_ref()
        .map(|control| control.clone_control())
    {
        Some(control) => binding_request.with_control(control),
        None => binding_request,
    }
}

fn operation_result(
    result: merman_bindings_core::BindingOperationResult,
) -> Result<MermanOperationResult, MermanError> {
    let (operation, media_type, data, metadata) = result.into_parts();

    Ok(MermanOperationResult {
        operation_id: operation.operation_id().to_string(),
        media_type: media_type.to_string(),
        data,
        metadata: uniffi_operation_metadata(metadata)?,
    })
}

fn uniffi_operation_metadata(
    metadata: BindingOperationMetadata,
) -> Result<MermanOperationMetadata, MermanError> {
    let raw_json = String::from_utf8(metadata.json_bytes().to_vec()).map_err(|error| {
        MermanError::internal(format!("operation metadata was not UTF-8: {error}"))
    })?;
    let output_plan = metadata.output_plan().map(uniffi_output_plan).transpose()?;

    Ok(MermanOperationMetadata {
        version: metadata.version(),
        operation_id: metadata.operation_id().to_string(),
        media_type: metadata.media_type().to_string(),
        runtime_policy: metadata.runtime_policy().to_string(),
        byte_length: metadata.byte_length(),
        output_plan,
        raw_json,
    })
}

fn uniffi_output_plan(plan: &BindingOutputPlan) -> Result<MermanOutputPlan, MermanError> {
    let kind = plan.kind().to_string();
    let raw_json = serde_json::to_string(plan).map_err(|error| {
        MermanError::internal(format!(
            "operation output plan serialization failed: {error}"
        ))
    })?;
    Ok(match plan {
        BindingOutputPlan::Ascii(plan) => MermanOutputPlan {
            kind,
            raw_json,
            raster: None,
            pdf_filter_images: None,
            ascii: Some(uniffi_ascii_output_plan(plan)),
        },
        BindingOutputPlan::Raster(plan) => MermanOutputPlan {
            kind,
            raw_json,
            raster: Some(MermanRasterOutputPlan {
                requested_width_px: plan.requested_width_px(),
                requested_height_px: plan.requested_height_px(),
                width_px: plan.width_px(),
                height_px: plan.height_px(),
                requested_scale: plan.requested_scale(),
                effective_scale: plan.effective_scale(),
                limited: plan.limited(),
            }),
            pdf_filter_images: None,
            ascii: None,
        },
        BindingOutputPlan::PdfFilterImages(plan) => MermanOutputPlan {
            kind,
            raw_json,
            raster: None,
            pdf_filter_images: Some(MermanPdfFilterImagesOutputPlan {
                filtered_groups: plan.filtered_groups(),
                requested_scale: f64::from(plan.requested_scale()),
                effective_scale: f64::from(plan.effective_scale()),
                requested_image_pixels: plan.requested_image_pixels(),
                effective_image_pixels: plan.effective_image_pixels(),
                limited: plan.limited(),
            }),
            ascii: None,
        },
        _ => MermanOutputPlan {
            kind,
            raw_json,
            raster: None,
            pdf_filter_images: None,
            ascii: None,
        },
    })
}

fn uniffi_ascii_output_plan(plan: &BindingAsciiOutputPlan) -> MermanAsciiOutputPlan {
    MermanAsciiOutputPlan {
        schema_version: plan.schema_version(),
        family: plan.family().to_string(),
        projection: plan.projection().to_string(),
        encoding: plan.encoding().to_string(),
        primary_width: plan.primary_width(),
        primary_height: plan.primary_height(),
        emitted_width: plan.emitted_width(),
        emitted_height: plan.emitted_height(),
        width_profile: plan.width_profile().to_string(),
        layout_profile: plan.layout_profile().map(str::to_owned),
        requested_layout_profile: plan.requested_layout_profile().to_string(),
        requested_graph_presentation: plan.requested_graph_presentation().to_string(),
        compact_attempted: plan.compact_attempted(),
        requested_max_width: plan.requested_max_width(),
        primary_overflowed: plan.primary_overflowed(),
        emitted_overflowed: plan.emitted_overflowed(),
        outcome: plan.outcome().to_string(),
        fallback_capability: plan.fallback_capability().to_string(),
        fallback_attempted: plan.fallback_attempted(),
        fallback_reason: plan.fallback_reason().map(str::to_owned),
        trimmed: plan.trimmed(),
        lossiness: plan.lossiness().to_string(),
    }
}

fn string_operation_output(result: Result<Vec<u8>, MermanError>) -> Result<String, MermanError> {
    String::from_utf8(result?)
        .map_err(|error| MermanError::internal(format!("operation output was not UTF-8: {error}")))
}

fn string_output(result: Result<Vec<u8>, BindingError>) -> Result<String, MermanError> {
    let bytes = result.map_err(MermanError::from_binding)?;
    String::from_utf8(bytes)
        .map_err(|err| MermanError::internal(format!("binding output was not UTF-8: {err}")))
}

fn validation_operation_output(
    result: Result<Vec<u8>, MermanError>,
) -> Result<MermanValidationResult, MermanError> {
    let bytes = result?;
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
mod tests;

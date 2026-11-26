use crate::artifact_contract::ValidatedArtifactContract;
use crate::capability::{OperationKey, operation_is_compiled};
use crate::common::BindingRequestOptions;
use crate::payload_contract::BINDING_OPERATION_SCHEMA_VERSION;
use crate::resource_contract::BindingResourceScope;
use crate::{
    BindingEngine, BindingEngineServices, BindingError, BindingStatus, DecodedOperationOptions,
};
use merman::{OperationControl, OperationPhase};
use serde::Serialize;
use serde_json::{Map, Value};
#[cfg(test)]
use std::cell::Cell;
use std::sync::Arc;

#[cfg(test)]
thread_local! {
    static METADATA_SERIALIZATION_COUNT: Cell<u64> = const { Cell::new(0) };
}

#[cfg(test)]
fn reset_metadata_serialization_count() {
    METADATA_SERIALIZATION_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
fn metadata_serialization_count() -> u64 {
    METADATA_SERIALIZATION_COUNT.with(Cell::get)
}

/// A versioned, transport-neutral operation selected from the canonical capability descriptor.
///
/// Operation IDs, maturity, capability prerequisites, media types, and URI requirements come
/// exclusively from `capabilities/feature-surface-v1.json`. Transport-specific numeric codes are
/// deliberately outside this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BindingOperationKind(OperationKey);

/// An operation admitted by one immutable artifact contract.
///
/// The private field prevents dispatchers from constructing this token without passing contract
/// admission first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AdmittedArtifactOperation(BindingOperationKind);

impl AdmittedArtifactOperation {
    pub(crate) const fn operation(self) -> BindingOperationKind {
        self.0
    }
}

impl BindingOperationKind {
    pub fn all() -> impl Iterator<Item = Self> + 'static {
        OperationKey::ALL.iter().copied().map(Self)
    }

    pub fn from_id(id: &str) -> Result<Self, BindingError> {
        OperationKey::from_id(id)
            .map(Self)
            .ok_or_else(|| BindingError::unknown_operation(format!("unknown operation `{id}`")))
    }

    #[must_use]
    pub const fn key(self) -> OperationKey {
        self.0
    }

    #[must_use]
    pub const fn operation_id(self) -> &'static str {
        self.0.spec().id
    }

    /// Returns the open maturity identifier owned by the canonical capability descriptor.
    #[must_use]
    pub const fn maturity(self) -> &'static str {
        self.0.spec().maturity
    }

    #[must_use]
    pub const fn media_type(self) -> &'static str {
        self.0.spec().media_type
    }

    /// Returns the stable semantic kind of bytes carried in the request's physical `source` slot.
    #[must_use]
    pub const fn input_kind(self) -> &'static str {
        self.0.spec().input_kind
    }

    /// Returns the optional public capability that gates this operation's availability.
    ///
    /// Semantic JSON deliberately returns `None`: canonical parsing is a base binding operation,
    /// not a fake `semantic` feature.
    #[must_use]
    pub const fn availability_capability_id(self) -> Option<&'static str> {
        match self.0.spec().capability {
            Some(capability) => Some(capability.id()),
            None => None,
        }
    }

    #[must_use]
    pub const fn requires_uri(self) -> bool {
        self.0.spec().requires_uri
    }

    const fn is_theme_operation(self) -> bool {
        matches!(
            self.key(),
            OperationKey::ExportThemePreset | OperationKey::ResolveTheme
        )
    }

    pub(crate) const fn resource_scope(self) -> BindingResourceScope {
        match self.key() {
            OperationKey::AnalysisJson
            | OperationKey::AnalysisFactsJson
            | OperationKey::ValidationJson => BindingResourceScope::AnalysisDiagram,
            OperationKey::DocumentAnalysisJson | OperationKey::DocumentAnalysisFactsJson => {
                BindingResourceScope::DocumentAnalysis
            }
            OperationKey::ExportThemePreset | OperationKey::ResolveTheme => {
                BindingResourceScope::ThemeAuthoring
            }
            OperationKey::SemanticJson => BindingResourceScope::Model,
            OperationKey::SvgPlanJson => BindingResourceScope::Svg,
            OperationKey::Ascii => BindingResourceScope::Ascii,
            OperationKey::LayoutJson => BindingResourceScope::Layout,
            OperationKey::EdgeGeometryJson => BindingResourceScope::EdgeGeometry,
            OperationKey::Svg => BindingResourceScope::Svg,
            OperationKey::Png => BindingResourceScope::Png,
            OperationKey::Jpeg => BindingResourceScope::Jpeg,
            OperationKey::Pdf => BindingResourceScope::Pdf,
        }
    }
}

impl ValidatedArtifactContract {
    pub(crate) fn admit_operation(
        &self,
        operation: BindingOperationKind,
    ) -> Result<AdmittedArtifactOperation, BindingError> {
        let id = operation.operation_id();
        if self.exposes_operation(operation.key()) {
            return Ok(AdmittedArtifactOperation(operation));
        }
        if let Some(capability) = operation.key().spec().capability
            && !self.exposes_capability(capability)
        {
            return Err(BindingError::missing_capability(
                capability.id(),
                format!(
                    "operation `{id}` requires capability `{}`, which is not exposed by target `{}`",
                    capability.id(),
                    self.target().id()
                ),
            ));
        }
        Err(BindingError::unsupported_operation(format!(
            "operation `{id}` is not exposed by target `{}`",
            self.target().id()
        )))
    }
}

/// Borrowed request consumed by the shared binding execution path.
#[derive(Debug, Clone)]
pub struct BindingOperationRequest<'a> {
    operation_id: &'a str,
    source: &'a [u8],
    uri: Option<&'a [u8]>,
    options_json: &'a [u8],
    operation_control: Option<OperationControl>,
}

impl<'a> BindingOperationRequest<'a> {
    /// Creates a request with no document URI and no request-local option overlay.
    ///
    /// Validation deliberately remains at execution time so operation resolution and URI-shape
    /// errors keep their documented precedence over malformed option JSON.
    #[must_use]
    pub const fn new(operation_id: &'a str, source: &'a [u8]) -> Self {
        Self {
            operation_id,
            source,
            uri: None,
            options_json: b"",
            operation_control: None,
        }
    }

    /// Attaches a document URI; an empty byte slice is normalized to omission.
    #[must_use]
    pub const fn with_uri(mut self, uri: &'a [u8]) -> Self {
        self.uri = if uri.is_empty() { None } else { Some(uri) };
        self
    }

    /// Attaches an optional document URI; `None` and an empty byte slice are equivalent.
    #[must_use]
    pub const fn with_optional_uri(mut self, uri: Option<&'a [u8]>) -> Self {
        self.uri = match uri {
            Some(uri) if !uri.is_empty() => Some(uri),
            Some(_) | None => None,
        };
        self
    }

    #[must_use]
    pub const fn with_options_json(mut self, options_json: &'a [u8]) -> Self {
        self.options_json = options_json;
        self
    }

    /// Attaches a caller-owned operation control. Execution creates a request-local child so
    /// cancellation and deadline changes remain visible without leaking one request's terminal
    /// resource outcome into later requests that reuse the caller's control token.
    #[must_use]
    pub fn with_control(mut self, control: OperationControl) -> Self {
        self.operation_control = Some(control);
        self
    }

    #[must_use]
    pub const fn operation_id(&self) -> &'a str {
        self.operation_id
    }

    #[must_use]
    pub const fn source(&self) -> &'a [u8] {
        self.source
    }

    /// Returns the operation input bytes using the descriptor-owned input semantics.
    ///
    /// `source()` remains as the physical wire-field compatibility projection.
    #[must_use]
    pub const fn input(&self) -> &'a [u8] {
        self.source
    }

    #[must_use]
    pub const fn uri(&self) -> Option<&'a [u8]> {
        self.uri
    }

    #[must_use]
    pub const fn options_json(&self) -> &'a [u8] {
        self.options_json
    }

    #[must_use]
    pub fn operation_control(&self) -> Option<&OperationControl> {
        self.operation_control.as_ref()
    }

    #[must_use]
    pub(crate) fn control_or_default(&self) -> OperationControl {
        self.operation_control
            .as_ref()
            .map_or_else(OperationControl::new, OperationControl::child)
    }
}

/// Owned result from a binding operation.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BindingOperationResult {
    operation: BindingOperationKind,
    media_type: &'static str,
    data: Vec<u8>,
    metadata: BindingOperationMetadata,
}

impl BindingOperationResult {
    #[must_use]
    pub const fn operation(&self) -> BindingOperationKind {
        self.operation
    }

    #[must_use]
    pub const fn media_type(&self) -> &'static str {
        self.media_type
    }

    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    #[must_use]
    pub const fn metadata(&self) -> &BindingOperationMetadata {
        &self.metadata
    }

    #[must_use]
    pub fn metadata_json(&self) -> &[u8] {
        self.metadata.json_bytes()
    }

    #[must_use]
    pub fn into_data(self) -> Vec<u8> {
        self.data
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        BindingOperationKind,
        &'static str,
        Vec<u8>,
        BindingOperationMetadata,
    ) {
        (self.operation, self.media_type, self.data, self.metadata)
    }
}

#[derive(Debug)]
pub(crate) struct BindingOperationOutput {
    data: Vec<u8>,
    details: BindingOperationDetails,
}

#[derive(Debug, Default)]
struct BindingOperationDetails {
    output_plan: Option<BindingOutputPlan>,
    layout_adaptation: Option<BindingLayoutAdaptationObservation>,
    theme_outcome: Option<Value>,
    shoal_layout_frame: Option<Value>,
}

#[derive(Debug)]
struct BindingOperationExecution {
    operation: BindingOperationKind,
    output: BindingOperationOutput,
}

impl BindingOperationExecution {
    fn into_data(self) -> Vec<u8> {
        self.output.into_data()
    }

    fn into_result(
        self,
        runtime_policy_id: &'static str,
    ) -> Result<BindingOperationResult, BindingError> {
        operation_result(self.operation, runtime_policy_id, self.output)
    }
}

struct PreparedEngineExecution {
    engine: BindingEngine,
    admitted: AdmittedArtifactOperation,
    control: OperationControl,
}

fn execute_theme_output(
    admitted: AdmittedArtifactOperation,
    source: &[u8],
    options_json: &[u8],
    control: &OperationControl,
) -> Result<BindingOperationOutput, BindingError> {
    execute_controlled_output(control, || {
        #[cfg(any(feature = "svg", feature = "ascii"))]
        {
            crate::native_theme::validate_operation_options(
                admitted.operation().operation_id(),
                options_json,
            )?;
            let data = match admitted.operation().key() {
                OperationKey::ExportThemePreset => {
                    crate::native_theme::export_theme_preset_json(source)?
                }
                OperationKey::ResolveTheme => {
                    crate::native_theme::resolve_theme_request_json(source)?
                }
                _ => unreachable!("expected a theme operation"),
            };
            Ok(BindingOperationOutput::plain(data))
        }
        #[cfg(not(any(feature = "svg", feature = "ascii")))]
        {
            let _ = (admitted, source, options_json);
            Err(crate::common::feature_required_error(
                "theme operations",
                "svg",
            ))
        }
    })
}

fn execute_controlled_output(
    control: &OperationControl,
    execute: impl FnOnce() -> Result<BindingOperationOutput, BindingError>,
) -> Result<BindingOperationOutput, BindingError> {
    control
        .checkpoint_at(OperationPhase::Parse)
        .map_err(BindingError::cancelled)?;
    let output = execute()?;
    control
        .checkpoint_at(OperationPhase::Postprocess)
        .map_err(BindingError::cancelled)?;
    Ok(output)
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum BindingOutputPlan {
    Ascii(Box<BindingAsciiOutputPlan>),
    Raster(BindingRasterOutputPlan),
    PdfFilterImages(BindingPdfFilterImageOutputPlan),
    Unknown(BindingUnknownOutputPlan),
}

impl BindingOutputPlan {
    #[must_use]
    pub fn kind(&self) -> &str {
        match self {
            Self::Ascii(_) => "ascii",
            Self::Raster(_) => "raster",
            Self::PdfFilterImages(_) => "pdf-filter-images",
            Self::Unknown(plan) => plan.kind(),
        }
    }
}

impl Serialize for BindingOutputPlan {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Ascii(plan) => BindingAsciiOutputPlanWire {
                kind: "ascii",
                plan,
            }
            .serialize(serializer),
            Self::Raster(plan) => BindingRasterOutputPlanWire {
                kind: "raster",
                plan,
            }
            .serialize(serializer),
            Self::PdfFilterImages(plan) => BindingPdfFilterImageOutputPlanWire {
                kind: "pdf-filter-images",
                plan,
            }
            .serialize(serializer),
            Self::Unknown(plan) => plan.serialize(serializer),
        }
    }
}

/// Logical ASCII report carried by the canonical `ascii` operation.
///
/// The operation data contains the exact encoded text bytes. This plan carries only the
/// provider-neutral metadata needed by a host to make an emission decision without remeasuring
/// ANSI/HTML output or parsing the text.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct BindingAsciiOutputPlan {
    schema_version: u16,
    family: String,
    projection: String,
    encoding: String,
    primary_width: Option<u64>,
    primary_height: Option<u64>,
    emitted_width: u64,
    emitted_height: u64,
    width_profile: String,
    layout_profile: Option<String>,
    requested_layout_profile: String,
    requested_graph_presentation: String,
    compact_attempted: bool,
    requested_max_width: Option<u64>,
    primary_overflowed: Option<bool>,
    emitted_overflowed: bool,
    outcome: String,
    fallback_capability: String,
    fallback_attempted: bool,
    fallback_reason: Option<String>,
    trimmed: bool,
    lossiness: String,
}

#[derive(Serialize)]
struct BindingAsciiOutputPlanWire<'a> {
    kind: &'static str,
    #[serde(flatten)]
    plan: &'a BindingAsciiOutputPlan,
}

impl BindingAsciiOutputPlan {
    #[cfg(feature = "ascii")]
    pub(crate) fn from_output(output: &merman::ascii::AsciiOutput) -> Self {
        Self::from_metadata(output.metadata())
    }

    #[cfg(feature = "ascii")]
    fn from_metadata(metadata: merman::ascii::AsciiOutputMetadata) -> Self {
        Self {
            schema_version: metadata.schema_version,
            family: metadata.family,
            projection: metadata.projection,
            encoding: metadata.encoding,
            primary_width: metadata.primary_width,
            primary_height: metadata.primary_height,
            emitted_width: metadata.emitted_width,
            emitted_height: metadata.emitted_height,
            width_profile: metadata.width_profile,
            layout_profile: metadata.layout_profile,
            requested_layout_profile: metadata.requested_layout_profile,
            requested_graph_presentation: metadata.requested_graph_presentation,
            compact_attempted: metadata.compact_attempted,
            requested_max_width: metadata.requested_max_width,
            primary_overflowed: metadata.primary_overflowed,
            emitted_overflowed: metadata.emitted_overflowed,
            outcome: metadata.outcome,
            fallback_capability: metadata.fallback_capability,
            fallback_attempted: metadata.fallback_attempted,
            fallback_reason: metadata.fallback_reason,
            trimmed: metadata.trimmed,
            lossiness: metadata.lossiness,
        }
    }

    #[must_use]
    pub const fn schema_version(&self) -> u16 {
        self.schema_version
    }

    #[must_use]
    pub fn family(&self) -> &str {
        &self.family
    }

    #[must_use]
    pub fn projection(&self) -> &str {
        &self.projection
    }

    #[must_use]
    pub fn encoding(&self) -> &str {
        &self.encoding
    }

    /// Measured primary width, unavailable when planning produced no primary geometry.
    #[must_use]
    pub const fn primary_width(&self) -> Option<u64> {
        self.primary_width
    }

    /// Measured primary height, with the same availability as [`Self::primary_width`].
    #[must_use]
    pub const fn primary_height(&self) -> Option<u64> {
        self.primary_height
    }

    #[must_use]
    pub const fn emitted_width(&self) -> u64 {
        self.emitted_width
    }

    #[must_use]
    pub const fn emitted_height(&self) -> u64 {
        self.emitted_height
    }

    #[must_use]
    pub fn width_profile(&self) -> &str {
        &self.width_profile
    }

    /// Effective primary layout, unavailable when no primary geometry exists.
    #[must_use]
    pub fn layout_profile(&self) -> Option<&str> {
        self.layout_profile.as_deref()
    }

    /// Layout profile requested by the host, including automatic selection.
    #[must_use]
    pub fn requested_layout_profile(&self) -> &str {
        &self.requested_layout_profile
    }

    /// Requested graph policy; [`Self::projection`] records the emitted presentation.
    #[must_use]
    pub fn requested_graph_presentation(&self) -> &str {
        &self.requested_graph_presentation
    }

    /// Whether automatic selection attempted a Compact candidate.
    #[must_use]
    pub const fn compact_attempted(&self) -> bool {
        self.compact_attempted
    }

    #[must_use]
    pub const fn requested_max_width(&self) -> Option<u64> {
        self.requested_max_width
    }

    /// Primary width overflow, unavailable when no primary geometry was measured.
    #[must_use]
    pub const fn primary_overflowed(&self) -> Option<bool> {
        self.primary_overflowed
    }

    /// Whether the emitted output exceeds the requested width.
    #[must_use]
    pub const fn emitted_overflowed(&self) -> bool {
        self.emitted_overflowed
    }

    #[must_use]
    pub fn outcome(&self) -> &str {
        &self.outcome
    }

    #[must_use]
    pub fn fallback_capability(&self) -> &str {
        &self.fallback_capability
    }

    #[must_use]
    pub const fn fallback_attempted(&self) -> bool {
        self.fallback_attempted
    }

    #[must_use]
    pub fn fallback_reason(&self) -> Option<&str> {
        self.fallback_reason.as_deref()
    }

    #[must_use]
    pub const fn trimmed(&self) -> bool {
        self.trimmed
    }

    #[must_use]
    pub fn lossiness(&self) -> &str {
        &self.lossiness
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct BindingRasterOutputPlan {
    requested_width_px: f64,
    requested_height_px: f64,
    width_px: u32,
    height_px: u32,
    requested_scale: f64,
    effective_scale: f64,
    limited: bool,
}

#[derive(Serialize)]
struct BindingRasterOutputPlanWire<'a> {
    kind: &'static str,
    #[serde(flatten)]
    plan: &'a BindingRasterOutputPlan,
}

impl BindingRasterOutputPlan {
    #[must_use]
    pub const fn requested_width_px(&self) -> f64 {
        self.requested_width_px
    }

    #[must_use]
    pub const fn requested_height_px(&self) -> f64 {
        self.requested_height_px
    }

    #[must_use]
    pub const fn width_px(&self) -> u32 {
        self.width_px
    }

    #[must_use]
    pub const fn height_px(&self) -> u32 {
        self.height_px
    }

    #[must_use]
    pub const fn requested_scale(&self) -> f64 {
        self.requested_scale
    }

    #[must_use]
    pub const fn effective_scale(&self) -> f64 {
        self.effective_scale
    }

    #[must_use]
    pub const fn limited(&self) -> bool {
        self.limited
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct BindingPdfFilterImageOutputPlan {
    filtered_groups: u64,
    requested_scale: f32,
    effective_scale: f32,
    requested_image_pixels: u64,
    effective_image_pixels: u64,
    limited: bool,
}

#[derive(Serialize)]
struct BindingPdfFilterImageOutputPlanWire<'a> {
    kind: &'static str,
    #[serde(flatten)]
    plan: &'a BindingPdfFilterImageOutputPlan,
}

impl BindingPdfFilterImageOutputPlan {
    #[must_use]
    pub const fn filtered_groups(&self) -> u64 {
        self.filtered_groups
    }

    #[must_use]
    pub const fn requested_scale(&self) -> f32 {
        self.requested_scale
    }

    #[must_use]
    pub const fn effective_scale(&self) -> f32 {
        self.effective_scale
    }

    #[must_use]
    pub const fn requested_image_pixels(&self) -> u64 {
        self.requested_image_pixels
    }

    #[must_use]
    pub const fn effective_image_pixels(&self) -> u64 {
        self.effective_image_pixels
    }

    #[must_use]
    pub const fn limited(&self) -> bool {
        self.limited
    }
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BindingUnknownOutputPlan {
    kind: String,
    value: Value,
}

impl BindingUnknownOutputPlan {
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

impl Serialize for BindingUnknownOutputPlan {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.value.serialize(serializer)
    }
}

/// Actual layout width retained beside operation metadata, independently of terminal output plans.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[non_exhaustive]
pub struct BindingLayoutAdaptationObservation {
    mode: String,
    requested_width: f64,
    actual_width: f64,
    overflowed: bool,
}

impl BindingLayoutAdaptationObservation {
    pub fn mode(&self) -> &str {
        &self.mode
    }
    pub const fn requested_width(&self) -> f64 {
        self.requested_width
    }
    pub const fn actual_width(&self) -> f64 {
        self.actual_width
    }
    pub const fn overflowed(&self) -> bool {
        self.overflowed
    }

    #[cfg(feature = "svg")]
    pub(crate) fn from_render(value: merman::svg::LayoutAdaptationObservation) -> Self {
        Self {
            mode: value.mode().id().to_owned(),
            requested_width: value.requested_width(),
            actual_width: value.actual_width(),
            overflowed: value.overflowed(),
        }
    }
}

/// Typed schema-1 metadata plus the exact JSON bytes received or produced at the boundary.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct BindingOperationMetadata {
    version: u32,
    operation_id: String,
    media_type: String,
    runtime_policy: String,
    byte_length: u64,
    output_plan: Option<BindingOutputPlan>,
    layout_adaptation: Option<BindingLayoutAdaptationObservation>,
    theme_outcome: Option<Value>,
    shoal_layout_frame: Option<Value>,
    #[cfg(feature = "svg")]
    svg_viewport: Option<crate::BindingSvgViewport>,
    json: Arc<[u8]>,
}

impl BindingOperationMetadata {
    pub fn from_json_bytes(json: &[u8]) -> Result<Self, BindingError> {
        let value: Value = serde_json::from_slice(json).map_err(|error| {
            BindingError::invalid_argument(format!("invalid operation metadata JSON: {error}"))
        })?;
        let object = value.as_object().ok_or_else(|| {
            BindingError::invalid_argument(
                "invalid operation metadata JSON: root must be an object",
            )
        })?;
        let version = required_u32(object, "version")?;
        if version != BINDING_OPERATION_SCHEMA_VERSION {
            return Err(BindingError::invalid_argument(format!(
                "unsupported operation metadata schema version {version}; expected {BINDING_OPERATION_SCHEMA_VERSION}"
            )));
        }
        let operation_id = required_string(object, "operation_id")?.to_owned();
        let media_type = required_string(object, "media_type")?.to_owned();
        let runtime_policy = required_string(object, "runtime_policy")?.to_owned();
        let byte_length = required_u64(object, "byte_length")?;
        let output_plan = match object.get("output_plan") {
            None | Some(Value::Null) => None,
            Some(value) => Some(parse_output_plan(value)?),
        };
        let layout_adaptation = match object.get("layout_adaptation") {
            None | Some(Value::Null) => None,
            Some(value) => {
                let object = value.as_object().ok_or_else(|| {
                    BindingError::invalid_argument("layout_adaptation must be an object")
                })?;
                let requested_width = required_f64(object, "requested_width")?;
                let actual_width = required_f64(object, "actual_width")?;
                if !requested_width.is_finite()
                    || requested_width <= 0.0
                    || !actual_width.is_finite()
                    || actual_width < 0.0
                {
                    return Err(BindingError::invalid_argument(
                        "layout_adaptation requested_width must be positive and actual_width must be nonnegative",
                    ));
                }
                Some(BindingLayoutAdaptationObservation {
                    mode: required_string(object, "mode")?.to_owned(),
                    requested_width,
                    actual_width,
                    overflowed: required_bool(object, "overflowed")?,
                })
            }
        };
        let theme_outcome = match object.get("theme_outcome") {
            None | Some(Value::Null) => None,
            Some(value @ Value::Object(fields)) => {
                required_string(fields, "mode")?;
                optional_string(fields, "preset_id")?;
                optional_string(fields, "appearance")?;
                if !required_value(fields, "notices")?.is_array() {
                    return Err(BindingError::invalid_argument(
                        "theme_outcome.notices must be an array",
                    ));
                }
                Some(value.clone())
            }
            Some(_) => {
                return Err(BindingError::invalid_argument(
                    "theme_outcome must be an object",
                ));
            }
        };
        let shoal_layout_frame = match object.get("shoal_layout_frame") {
            None | Some(Value::Null) => None,
            Some(value @ Value::Object(_)) => Some(value.clone()),
            Some(_) => {
                return Err(BindingError::invalid_argument(
                    "shoal_layout_frame must be an object",
                ));
            }
        };

        #[cfg(feature = "svg")]
        let svg_viewport = object
            .get("svg_viewport")
            .filter(|value| !value.is_null())
            .map(|value| {
                serde_json::from_value::<crate::BindingSvgViewport>(value.clone())
                    .map_err(|_| BindingError::invalid_argument("invalid SVG viewport metadata"))
                    .and_then(crate::BindingSvgViewport::validate)
            })
            .transpose()?;

        Ok(Self {
            version,
            operation_id,
            media_type,
            runtime_policy,
            byte_length,
            output_plan,
            layout_adaptation,
            theme_outcome,
            shoal_layout_frame,
            #[cfg(feature = "svg")]
            svg_viewport,
            json: Arc::from(json),
        })
    }

    fn from_execution(
        operation: BindingOperationKind,
        runtime_policy: &str,
        byte_length: u64,
        details: BindingOperationDetails,
        #[cfg(feature = "svg")] svg_viewport: Option<crate::BindingSvgViewport>,
    ) -> Result<Self, BindingError> {
        #[cfg(test)]
        METADATA_SERIALIZATION_COUNT.with(|count| count.set(count.get() + 1));
        let BindingOperationDetails {
            output_plan,
            layout_adaptation,
            theme_outcome,
            shoal_layout_frame,
        } = details;

        let json = serde_json::to_vec(&BindingOperationMetadataWire {
            version: BINDING_OPERATION_SCHEMA_VERSION,
            operation_id: operation.operation_id(),
            media_type: operation.media_type(),
            runtime_policy,
            byte_length,
            output_plan: output_plan.as_ref(),
            layout_adaptation: layout_adaptation.as_ref(),
            theme_outcome: theme_outcome.as_ref(),
            #[cfg(feature = "svg")]
            svg_viewport,
            shoal_layout_frame: shoal_layout_frame.as_ref(),
        })
        .map_err(|error| {
            BindingError::internal(format!("failed to serialize operation metadata: {error}"))
        })?;

        Ok(Self {
            version: BINDING_OPERATION_SCHEMA_VERSION,
            operation_id: operation.operation_id().to_owned(),
            media_type: operation.media_type().to_owned(),
            runtime_policy: runtime_policy.to_owned(),
            byte_length,
            output_plan,
            layout_adaptation,
            theme_outcome,
            shoal_layout_frame,
            #[cfg(feature = "svg")]
            svg_viewport,
            json: Arc::from(json),
        })
    }

    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    #[must_use]
    pub fn operation_id(&self) -> &str {
        &self.operation_id
    }

    #[must_use]
    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    #[must_use]
    pub fn runtime_policy(&self) -> &str {
        &self.runtime_policy
    }

    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    #[must_use]
    pub const fn layout_adaptation(&self) -> Option<&BindingLayoutAdaptationObservation> {
        self.layout_adaptation.as_ref()
    }

    #[must_use]
    pub const fn shoal_layout_frame(&self) -> Option<&Value> {
        self.shoal_layout_frame.as_ref()
    }

    #[must_use]
    pub const fn output_plan(&self) -> Option<&BindingOutputPlan> {
        self.output_plan.as_ref()
    }

    #[must_use]
    pub const fn theme_outcome(&self) -> Option<&Value> {
        self.theme_outcome.as_ref()
    }

    #[must_use]
    #[cfg(feature = "svg")]
    pub const fn svg_viewport(&self) -> Option<crate::BindingSvgViewport> {
        self.svg_viewport
    }

    #[must_use]
    pub fn json_bytes(&self) -> &[u8] {
        &self.json
    }

    #[must_use]
    pub fn into_json_bytes(self) -> Vec<u8> {
        self.json.as_ref().to_vec()
    }
}

impl BindingOperationOutput {
    #[cfg(feature = "svg")]
    pub(crate) fn with_layout_adaptation(
        mut self,
        value: Option<merman::svg::LayoutAdaptationObservation>,
    ) -> Self {
        self.details.layout_adaptation = value.map(BindingLayoutAdaptationObservation::from_render);
        self
    }

    #[cfg(feature = "layout-shoal")]
    pub(crate) fn with_shoal_layout_frame(mut self, frame: Option<Value>) -> Self {
        self.details.shoal_layout_frame = frame;
        self
    }

    pub(crate) fn into_data(self) -> Vec<u8> {
        self.data
    }

    pub(crate) fn plain(data: Vec<u8>) -> Self {
        Self {
            data,
            details: BindingOperationDetails::default(),
        }
    }

    #[cfg(feature = "svg")]
    pub(crate) fn svg(data: Vec<u8>, theme_outcome: Value) -> Self {
        Self {
            data,
            details: BindingOperationDetails {
                theme_outcome: Some(theme_outcome),
                ..Default::default()
            },
        }
    }

    #[cfg(feature = "ascii")]
    pub(crate) fn ascii(output: merman::ascii::AsciiOutput, include_metadata: bool) -> Self {
        if !include_metadata {
            return Self::plain(output.into_bytes());
        }
        let plan = BindingOutputPlan::Ascii(Box::new(BindingAsciiOutputPlan::from_output(&output)));
        Self {
            data: output.into_bytes(),
            details: BindingOperationDetails {
                output_plan: Some(plan),
                ..Default::default()
            },
        }
    }

    #[cfg(any(feature = "png", feature = "jpeg"))]
    pub(crate) fn raster(
        data: Vec<u8>,
        plan: merman::svg::export::RasterPlan,
        theme_outcome: Value,
    ) -> Self {
        Self {
            data,
            details: BindingOperationDetails {
                output_plan: Some(BindingOutputPlan::Raster(BindingRasterOutputPlan {
                    requested_width_px: plan.requested_width_px,
                    requested_height_px: plan.requested_height_px,
                    width_px: plan.width_px,
                    height_px: plan.height_px,
                    requested_scale: plan.requested_scale,
                    effective_scale: plan.effective_scale,
                    limited: plan.limited,
                })),
                theme_outcome: Some(theme_outcome),
                ..Default::default()
            },
        }
    }

    #[cfg(feature = "pdf")]
    pub(crate) fn pdf(
        data: Vec<u8>,
        plan: merman::svg::export::PdfFilterImagePlan,
        theme_outcome: Value,
    ) -> Self {
        Self {
            data,
            details: BindingOperationDetails {
                output_plan: Some(BindingOutputPlan::PdfFilterImages(
                    BindingPdfFilterImageOutputPlan {
                        filtered_groups: plan.filtered_groups as u64,
                        requested_scale: plan.requested_scale,
                        effective_scale: plan.effective_scale,
                        requested_image_pixels: plan.requested_image_pixels,
                        effective_image_pixels: plan.effective_image_pixels,
                        limited: plan.limited,
                    },
                )),
                theme_outcome: Some(theme_outcome),
                ..Default::default()
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct BindingOperationMetadataWire<'a> {
    version: u32,
    operation_id: &'a str,
    media_type: &'a str,
    runtime_policy: &'a str,
    byte_length: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_plan: Option<&'a BindingOutputPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    layout_adaptation: Option<&'a BindingLayoutAdaptationObservation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    theme_outcome: Option<&'a Value>,
    #[cfg(feature = "svg")]
    #[serde(skip_serializing_if = "Option::is_none")]
    svg_viewport: Option<crate::BindingSvgViewport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shoal_layout_frame: Option<&'a Value>,
}

fn parse_output_plan(value: &Value) -> Result<BindingOutputPlan, BindingError> {
    let object = value.as_object().ok_or_else(|| {
        BindingError::invalid_argument(
            "invalid operation metadata JSON: output_plan must be an object",
        )
    })?;
    let kind = required_string(object, "kind")?;
    match kind {
        "ascii" => parse_ascii_output_plan(object)
            .map(Box::new)
            .map(BindingOutputPlan::Ascii),
        "raster" => Ok(BindingOutputPlan::Raster(BindingRasterOutputPlan {
            requested_width_px: required_f64(object, "requested_width_px")?,
            requested_height_px: required_f64(object, "requested_height_px")?,
            width_px: required_u32(object, "width_px")?,
            height_px: required_u32(object, "height_px")?,
            requested_scale: required_f64(object, "requested_scale")?,
            effective_scale: required_f64(object, "effective_scale")?,
            limited: required_bool(object, "limited")?,
        })),
        "pdf-filter-images" => Ok(BindingOutputPlan::PdfFilterImages(
            BindingPdfFilterImageOutputPlan {
                filtered_groups: required_u64(object, "filtered_groups")?,
                requested_scale: required_f32(object, "requested_scale")?,
                effective_scale: required_f32(object, "effective_scale")?,
                requested_image_pixels: required_u64(object, "requested_image_pixels")?,
                effective_image_pixels: required_u64(object, "effective_image_pixels")?,
                limited: required_bool(object, "limited")?,
            },
        )),
        _ => Ok(BindingOutputPlan::Unknown(BindingUnknownOutputPlan {
            kind: kind.to_owned(),
            value: value.clone(),
        })),
    }
}

fn parse_ascii_output_plan(
    object: &Map<String, Value>,
) -> Result<BindingAsciiOutputPlan, BindingError> {
    let schema_version = required_u16(object, "schema_version")?;
    if schema_version != 4 {
        return Err(BindingError::invalid_argument(format!(
            "unsupported ASCII output schema version {schema_version}; expected 4"
        )));
    }
    let plan = BindingAsciiOutputPlan {
        schema_version,
        family: required_string(object, "family")?.to_owned(),
        projection: required_string(object, "projection")?.to_owned(),
        encoding: required_string(object, "encoding")?.to_owned(),
        primary_width: required_nullable_u64(object, "primary_width")?,
        primary_height: required_nullable_u64(object, "primary_height")?,
        emitted_width: required_u64(object, "emitted_width")?,
        emitted_height: required_u64(object, "emitted_height")?,
        width_profile: required_string(object, "width_profile")?.to_owned(),
        layout_profile: required_nullable_string(object, "layout_profile")?.map(str::to_owned),
        requested_layout_profile: required_string(object, "requested_layout_profile")?.to_owned(),
        requested_graph_presentation: required_string(object, "requested_graph_presentation")?
            .to_owned(),
        compact_attempted: required_bool(object, "compact_attempted")?,
        requested_max_width: optional_u64(object, "requested_max_width")?,
        primary_overflowed: required_nullable_bool(object, "primary_overflowed")?,
        emitted_overflowed: required_bool(object, "emitted_overflowed")?,
        outcome: required_string(object, "outcome")?.to_owned(),
        fallback_capability: required_string(object, "fallback_capability")?.to_owned(),
        fallback_attempted: required_bool(object, "fallback_attempted")?,
        fallback_reason: optional_string(object, "fallback_reason")?.map(str::to_owned),
        trimmed: required_bool(object, "trimmed")?,
        lossiness: required_string(object, "lossiness")?.to_owned(),
    };
    let primary_available = plan.primary_width.is_some();
    if plan.primary_height.is_some() != primary_available
        || plan.layout_profile.is_some() != primary_available
        || plan.primary_overflowed.is_some() != primary_available
    {
        return Err(BindingError::invalid_argument(
            "invalid operation metadata JSON: primary_width, primary_height, layout_profile and primary_overflowed must all be null or all be non-null",
        ));
    }
    Ok(plan)
}

fn required_value<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a Value, BindingError> {
    object.get(field).ok_or_else(|| {
        BindingError::invalid_argument(format!(
            "invalid operation metadata JSON: missing required field `{field}`"
        ))
    })
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, BindingError> {
    required_value(object, field)?.as_str().ok_or_else(|| {
        BindingError::invalid_argument(format!(
            "invalid operation metadata JSON: `{field}` must be a string"
        ))
    })
}

fn required_bool(object: &Map<String, Value>, field: &str) -> Result<bool, BindingError> {
    required_value(object, field)?.as_bool().ok_or_else(|| {
        BindingError::invalid_argument(format!(
            "invalid operation metadata JSON: `{field}` must be a boolean"
        ))
    })
}

fn required_u64(object: &Map<String, Value>, field: &str) -> Result<u64, BindingError> {
    required_value(object, field)?.as_u64().ok_or_else(|| {
        BindingError::invalid_argument(format!(
            "invalid operation metadata JSON: `{field}` must be an unsigned 64-bit integer"
        ))
    })
}

fn required_u32(object: &Map<String, Value>, field: &str) -> Result<u32, BindingError> {
    let value = required_u64(object, field)?;
    u32::try_from(value).map_err(|_| {
        BindingError::invalid_argument(format!(
            "invalid operation metadata JSON: `{field}` exceeds unsigned 32-bit range"
        ))
    })
}

fn required_u16(object: &Map<String, Value>, field: &str) -> Result<u16, BindingError> {
    let value = required_u64(object, field)?;
    u16::try_from(value).map_err(|_| {
        BindingError::invalid_argument(format!(
            "invalid operation metadata JSON: `{field}` exceeds unsigned 16-bit range"
        ))
    })
}

fn optional_u64(object: &Map<String, Value>, field: &str) -> Result<Option<u64>, BindingError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_u64().map(Some).ok_or_else(|| {
            BindingError::invalid_argument(format!(
                "invalid operation metadata JSON: `{field}` must be an unsigned 64-bit integer or null"
            ))
        }),
    }
}

fn required_nullable_u64(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<u64>, BindingError> {
    required_value(object, field)?;
    optional_u64(object, field)
}

fn required_nullable_bool(
    object: &Map<String, Value>,
    field: &str,
) -> Result<Option<bool>, BindingError> {
    if required_value(object, field)?.is_null() {
        Ok(None)
    } else {
        required_bool(object, field).map(Some)
    }
}

fn required_nullable_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<Option<&'a str>, BindingError> {
    required_value(object, field)?;
    optional_string(object, field)
}

fn optional_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<Option<&'a str>, BindingError> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value.as_str().map(Some).ok_or_else(|| {
            BindingError::invalid_argument(format!(
                "invalid operation metadata JSON: `{field}` must be a string or null"
            ))
        }),
    }
}

fn required_f64(object: &Map<String, Value>, field: &str) -> Result<f64, BindingError> {
    required_value(object, field)?.as_f64().ok_or_else(|| {
        BindingError::invalid_argument(format!(
            "invalid operation metadata JSON: `{field}` must be a finite JSON number"
        ))
    })
}

fn required_f32(object: &Map<String, Value>, field: &str) -> Result<f32, BindingError> {
    let value = required_f64(object, field)?;
    if !value.is_finite() || value < f64::from(f32::MIN) || value > f64::from(f32::MAX) {
        return Err(BindingError::invalid_argument(format!(
            "invalid operation metadata JSON: `{field}` exceeds finite 32-bit float range"
        )));
    }
    Ok(value as f32)
}

/// Executes one operation through the same transport-neutral semantics as a reusable engine.
pub fn execute_once(
    request: BindingOperationRequest<'_>,
) -> Result<BindingOperationResult, BindingError> {
    crate::artifact_contract::default_artifact_contract()
        .execute_once_with_services(request, BindingEngineServices::new())
}

pub(crate) fn execute_once_data(
    request: BindingOperationRequest<'_>,
) -> Result<Vec<u8>, BindingError> {
    crate::artifact_contract::default_artifact_contract().execute_once_data(request)
}

impl ValidatedArtifactContract {
    /// Executes one operation against this exact transport contract without retaining an engine.
    pub fn execute_once(
        &self,
        request: BindingOperationRequest<'_>,
    ) -> Result<BindingOperationResult, BindingError> {
        self.execute_once_with_services(request, BindingEngineServices::new())
    }

    /// Executes one operation with immutable constructor services against this exact transport
    /// contract without retaining an engine.
    pub fn execute_once_with_services(
        &self,
        request: BindingOperationRequest<'_>,
        services: BindingEngineServices,
    ) -> Result<BindingOperationResult, BindingError> {
        self.execute_once_request(request, services, true)
            .and_then(|(execution, runtime_policy)| execution.into_result(runtime_policy))
    }

    fn execute_once_request(
        &self,
        request: BindingOperationRequest<'_>,
        services: BindingEngineServices,
        include_metadata: bool,
    ) -> Result<(BindingOperationExecution, &'static str), BindingError> {
        let (operation, control) = resolve_one_shot_request(&request)?;
        let (output, runtime_policy) = if operation.is_theme_operation() {
            let admitted = self.admit_operation(operation)?;
            self.validate_engine_services(&services)?;
            let output =
                execute_theme_output(admitted, request.source, request.options_json, &control)?;
            (output, crate::BindingRuntimePolicy::Deterministic.id())
        } else {
            let PreparedEngineExecution {
                engine,
                admitted,
                control,
            } =
                self.prepare_engine_execution(operation, request.options_json, services, control)?;
            let output = engine.execute_admitted_output(
                admitted,
                request.source,
                request.uri,
                control,
                include_metadata,
            )?;
            (output, engine.runtime_policy_id())
        };
        Ok((
            BindingOperationExecution { operation, output },
            runtime_policy,
        ))
    }

    fn prepare_engine_execution(
        &self,
        operation: BindingOperationKind,
        options_json: &[u8],
        services: BindingEngineServices,
        control: OperationControl,
    ) -> Result<PreparedEngineExecution, BindingError> {
        crate::common::validate_one_shot_resource_options(
            options_json,
            operation.resource_scope(),
        )?;
        let engine = self.create_engine_with_services(options_json, services)?;
        control
            .checkpoint_at(OperationPhase::Admission)
            .map_err(BindingError::cancelled)?;
        let admitted = self.admit_operation(operation)?;
        Ok(PreparedEngineExecution {
            engine,
            admitted,
            control,
        })
    }

    /// Executes one operation without producing result metadata.
    pub fn execute_once_data(
        &self,
        request: BindingOperationRequest<'_>,
    ) -> Result<Vec<u8>, BindingError> {
        self.execute_once_data_with_services(request, BindingEngineServices::new())
    }

    /// Executes one data-only operation with immutable constructor services.
    pub fn execute_once_data_with_services(
        &self,
        request: BindingOperationRequest<'_>,
        services: BindingEngineServices,
    ) -> Result<Vec<u8>, BindingError> {
        self.execute_once_request(request, services, false)
            .map(|(execution, _)| execution.into_data())
    }

    /// Executes a diagram operation with a transport-owned resource ceiling and optional theme outcome.
    ///
    /// The caller can tighten the ceiling or use an explicit host-approved limit override.
    /// After validating the request options, `normalize` may set only the `runtime_policy` and
    /// `environment` options. The normalized options remain subject to the artifact contract.
    #[doc(hidden)]
    pub fn execute_once_with_resource_ceiling(
        &self,
        request: BindingOperationRequest<'_>,
        ceiling_profile_id: &str,
        ceiling_overrides: &[(&str, usize)],
        allowed_request_limit_ids: &[&str],
        include_theme_outcome: bool,
        normalize: impl FnOnce(&mut Value) -> Result<(), BindingError>,
    ) -> Result<(Vec<u8>, Option<Value>), BindingError> {
        let (operation, control) = resolve_one_shot_request(&request)?;
        if operation.is_theme_operation() {
            return Err(BindingError::invalid_argument(
                "transport resource ceilings apply only to diagram operations",
            ));
        }
        let (options, base_options) = crate::common::parse_one_shot_options_with_resource_ceiling(
            request.options_json,
            ceiling_profile_id,
            ceiling_overrides,
            allowed_request_limit_ids,
            self,
            operation.resource_scope(),
            normalize,
        )?;
        let engine = BindingEngine::from_parsed_options_for_contract(
            Arc::new(*self),
            &options,
            base_options,
            BindingEngineServices::new(),
        )?;
        control
            .checkpoint_at(OperationPhase::Admission)
            .map_err(BindingError::cancelled)?;
        let admitted = self.admit_operation(operation)?;
        let output = engine.execute_admitted_output(
            admitted,
            request.source,
            request.uri,
            control,
            include_theme_outcome,
        )?;
        Ok((output.data, output.details.theme_outcome))
    }

    /// Executes only SVG rendering, without result metadata or the general operation dispatcher.
    ///
    /// Uses the same request preparation, artifact admission, services, resource validation and
    /// cancellation checkpoints as `execute_once_data_with_services` for an `svg` request.
    pub fn render_svg_once_data_with_services(
        &self,
        source: &[u8],
        options_json: &[u8],
        services: BindingEngineServices,
        operation_control: Option<OperationControl>,
    ) -> Result<Vec<u8>, BindingError> {
        let request = BindingOperationRequest {
            operation_control,
            ..BindingOperationRequest::new("svg", source).with_options_json(options_json)
        };
        let (operation, control) = resolve_one_shot_request(&request)?;
        let PreparedEngineExecution {
            engine, control, ..
        } = self.prepare_engine_execution(operation, options_json, services, control)?;
        execute_controlled_output(&control, || {
            engine.render_svg_output(source, control.clone(), false)
        })
        .map(BindingOperationOutput::into_data)
    }

    /// Executes only diagram analysis JSON, without the general operation dispatcher.
    ///
    /// Uses the same request preparation, artifact admission, services, resource validation and
    /// cancellation checkpoints as `execute_once_data_with_services` for an `analysis-json` request.
    pub fn analyze_json_once_data_with_services(
        &self,
        source: &[u8],
        options_json: &[u8],
        services: BindingEngineServices,
        operation_control: Option<OperationControl>,
    ) -> Result<Vec<u8>, BindingError> {
        let request = BindingOperationRequest {
            operation_control,
            ..BindingOperationRequest::new("analysis-json", source).with_options_json(options_json)
        };
        let (operation, control) = resolve_one_shot_request(&request)?;
        let PreparedEngineExecution {
            engine, control, ..
        } = self.prepare_engine_execution(operation, options_json, services, control)?;
        execute_controlled_output(&control, || {
            engine
                .analyze_json_data(source, &control)
                .map(BindingOperationOutput::plain)
        })
        .map(BindingOperationOutput::into_data)
    }
}

impl BindingEngine {
    /// Executes one operation against this immutable reusable engine.
    pub fn execute(
        &self,
        request: BindingOperationRequest<'_>,
    ) -> Result<BindingOperationResult, BindingError> {
        self.execute_with_decoded_options(request, None)
    }

    /// Executes a reusable request with options already decoded by the transport.
    #[doc(hidden)]
    pub fn execute_with_decoded_options<'a>(
        &self,
        request: BindingOperationRequest<'a>,
        options: Option<DecodedOperationOptions<'a>>,
    ) -> Result<BindingOperationResult, BindingError> {
        self.execute_request(request, options, true)
            .and_then(|execution| execution.into_result(self.runtime_policy_id()))
    }

    /// Executes one operation without producing result metadata.
    pub fn execute_data(
        &self,
        request: BindingOperationRequest<'_>,
    ) -> Result<Vec<u8>, BindingError> {
        self.execute_data_with_decoded_options(request, None)
    }

    /// Executes a data-only request with options already decoded by the transport.
    #[doc(hidden)]
    pub fn execute_data_with_decoded_options<'a>(
        &self,
        request: BindingOperationRequest<'a>,
        options: Option<DecodedOperationOptions<'a>>,
    ) -> Result<Vec<u8>, BindingError> {
        self.execute_request(request, options, false)
            .map(BindingOperationExecution::into_data)
    }

    /// Executes data-only SVG with a request-local ID from a typed transport.
    #[cfg(feature = "svg")]
    #[doc(hidden)]
    pub fn execute_svg_data_with_diagram_id(
        &self,
        source: &[u8],
        diagram_id: &str,
        control: OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        let request =
            BindingOperationRequest::new(OperationKey::Svg.id(), source).with_control(control);
        self.execute_request_with_options(request, false, |operation| {
            self.prepare_svg_diagram_id(operation, diagram_id)
        })
        .map(BindingOperationExecution::into_data)
    }

    fn execute_request<'a>(
        &self,
        request: BindingOperationRequest<'a>,
        options: Option<DecodedOperationOptions<'a>>,
        include_metadata: bool,
    ) -> Result<BindingOperationExecution, BindingError> {
        let (request, options) = match options {
            Some(options) => (
                request.with_options_json(options.json.as_bytes()),
                BindingRequestOptions::Decoded(options),
            ),
            None => {
                let options = BindingRequestOptions::Raw(request.options_json);
                (request, options)
            }
        };
        self.execute_request_with_options(request, include_metadata, |operation| {
            self.prepare_request_overlay(operation, options)
        })
    }

    fn execute_request_with_options(
        &self,
        request: BindingOperationRequest<'_>,
        include_metadata: bool,
        prepare_options: impl FnOnce(
            BindingOperationKind,
        )
            -> Result<crate::engine::PreparedRequestOverlay, BindingError>,
    ) -> Result<BindingOperationExecution, BindingError> {
        let operation = resolve_operation_request(&request)?;
        let control = request.control_or_default();
        control
            .checkpoint_at(OperationPhase::Admission)
            .map_err(BindingError::cancelled)?;
        if operation.is_theme_operation() {
            let admitted = self.admit_operation(operation)?;
            let output =
                execute_theme_output(admitted, request.source, request.options_json, &control)?;
            return Ok(BindingOperationExecution { operation, output });
        }
        let prepared = prepare_options(operation)?;
        let admitted = self.admit_operation(operation)?;
        let output = match prepared {
            crate::engine::PreparedRequestOverlay::Unchanged => self.execute_admitted_output(
                admitted,
                request.source,
                request.uri,
                control.clone(),
                include_metadata,
            ),
            #[cfg(feature = "svg")]
            crate::engine::PreparedRequestOverlay::Svg(overlay) => self.execute_svg_override(
                overlay,
                request.source,
                control.clone(),
                include_metadata,
            ),
            crate::engine::PreparedRequestOverlay::Override(configs) => self
                .execute_request_projection(
                    admitted,
                    *configs,
                    request.source,
                    request.uri,
                    control.clone(),
                    include_metadata,
                ),
        }?;
        control
            .checkpoint_at(OperationPhase::Postprocess)
            .map_err(BindingError::cancelled)?;
        Ok(BindingOperationExecution { operation, output })
    }

    pub(crate) fn execute_admitted_output(
        &self,
        admitted: AdmittedArtifactOperation,
        source: &[u8],
        uri: Option<&[u8]>,
        control: OperationControl,
        include_metadata: bool,
    ) -> Result<BindingOperationOutput, BindingError> {
        execute_controlled_output(&control, || match admitted.operation().key() {
            OperationKey::ExportThemePreset | OperationKey::ResolveTheme => {
                unreachable!("theme operations own their request options")
            }
            OperationKey::Png => self.render_png_output(source, control.clone(), include_metadata),
            OperationKey::Jpeg => {
                self.render_jpeg_output(source, control.clone(), include_metadata)
            }
            OperationKey::Pdf => self.render_pdf_output(source, control.clone(), include_metadata),
            OperationKey::Svg => self.render_svg_output(source, control.clone(), include_metadata),
            OperationKey::SvgPlanJson => self
                .svg_plan_json_data(source, control.clone())
                .map(BindingOperationOutput::plain),
            #[cfg(feature = "ascii")]
            OperationKey::Ascii => self
                .render_ascii_output(source, control.clone())
                .map(|output| BindingOperationOutput::ascii(output, include_metadata)),
            #[cfg(not(feature = "ascii"))]
            OperationKey::Ascii => self
                .render_ascii_data(source, control.clone())
                .map(BindingOperationOutput::plain),
            OperationKey::SemanticJson => self
                .parse_json_data_controlled(source, &control)
                .map(BindingOperationOutput::plain),
            OperationKey::LayoutJson => self
                .layout_json_data(source, control.clone())
                .map(BindingOperationOutput::plain),
            OperationKey::EdgeGeometryJson => self
                .edge_geometry_json_data(source, control.clone())
                .map(BindingOperationOutput::plain),
            OperationKey::AnalysisJson => self
                .analyze_json_data(source, &control)
                .map(BindingOperationOutput::plain),
            OperationKey::AnalysisFactsJson => self
                .analysis_facts_json_data(source, &control)
                .map(BindingOperationOutput::plain),
            OperationKey::ValidationJson => self
                .validate_json_data(source, &control)
                .map(BindingOperationOutput::plain),
            OperationKey::DocumentAnalysisJson => self
                .analyze_document_json_data(
                    source,
                    uri.expect("validated document URI presence"),
                    &control,
                )
                .map(BindingOperationOutput::plain),
            OperationKey::DocumentAnalysisFactsJson => self
                .analyze_document_facts_json_data(
                    source,
                    uri.expect("validated document URI presence"),
                    &control,
                )
                .map(BindingOperationOutput::plain),
        })
    }
}

pub(crate) fn operation_result(
    operation: BindingOperationKind,
    runtime_policy_id: &'static str,
    output: BindingOperationOutput,
) -> Result<BindingOperationResult, BindingError> {
    let BindingOperationOutput { data, details } = output;
    let byte_length = u64::try_from(data.len()).map_err(|_| {
        BindingError::internal("operation result byte length exceeds unsigned 64-bit range")
    })?;
    #[cfg(feature = "svg")]
    let svg_viewport = if operation.key() == OperationKey::Svg {
        crate::svg_viewport::from_svg(&data)?
    } else {
        None
    };
    let metadata = BindingOperationMetadata::from_execution(
        operation,
        runtime_policy_id,
        byte_length,
        details,
        #[cfg(feature = "svg")]
        svg_viewport,
    )?;

    Ok(BindingOperationResult {
        operation,
        media_type: operation.media_type(),
        data,
        metadata,
    })
}

fn resolve_one_shot_request(
    request: &BindingOperationRequest<'_>,
) -> Result<(BindingOperationKind, OperationControl), BindingError> {
    let operation = resolve_operation_request(request)?;
    let control = request.control_or_default();
    control
        .checkpoint_at(OperationPhase::Admission)
        .map_err(BindingError::cancelled)?;
    Ok((operation, control))
}

fn resolve_operation_request(
    request: &BindingOperationRequest<'_>,
) -> Result<BindingOperationKind, BindingError> {
    let operation = BindingOperationKind::from_id(request.operation_id)?;
    if operation.requires_uri() != request.uri.is_some() {
        return Err(BindingError::new(
            BindingStatus::InvalidArgument,
            format!(
                "operation `{}` {} a document URI",
                operation.operation_id(),
                if operation.requires_uri() {
                    "requires"
                } else {
                    "does not accept"
                }
            ),
        ));
    }
    Ok(operation)
}

pub fn compiled_operation_kind_ids() -> Vec<&'static str> {
    BindingOperationKind::all()
        .filter(|operation| operation.is_compiled())
        .map(|operation| operation.operation_id())
        .collect()
}

impl BindingOperationKind {
    #[must_use]
    pub fn is_compiled(self) -> bool {
        operation_is_compiled(self.key())
    }
}

#[cfg(test)]
mod tests;

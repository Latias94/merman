use crate::artifact_contract::ValidatedArtifactContract;
use crate::capability::{OperationKey, operation_is_compiled};
use crate::payload_contract::BINDING_OPERATION_SCHEMA_VERSION;
use crate::resource_contract::BindingResourceScope;
use crate::{BindingEngine, BindingError, BindingStatus};
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

/// A stable, transport-neutral operation selected from the canonical capability descriptor.
///
/// Operation IDs, capability prerequisites, media types, and URI requirements come exclusively
/// from `capabilities/feature-surface-v1.json`. Transport-specific numeric codes are deliberately
/// outside this type.
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

    #[must_use]
    pub const fn media_type(self) -> &'static str {
        self.0.spec().media_type
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

    pub(crate) const fn resource_scope(self) -> BindingResourceScope {
        match self.key() {
            OperationKey::AnalysisJson
            | OperationKey::AnalysisFactsJson
            | OperationKey::ValidationJson => BindingResourceScope::AnalysisDiagram,
            OperationKey::DocumentAnalysisJson | OperationKey::DocumentAnalysisFactsJson => {
                BindingResourceScope::DocumentAnalysis
            }
            OperationKey::SemanticJson | OperationKey::SvgPlanJson => BindingResourceScope::Model,
            OperationKey::Ascii => BindingResourceScope::Ascii,
            OperationKey::LayoutJson => BindingResourceScope::Layout,
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
#[derive(Debug, Clone, Copy)]
pub struct BindingOperationRequest<'a> {
    operation_id: &'a str,
    source: &'a [u8],
    uri: Option<&'a [u8]>,
    options_json: &'a [u8],
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
        }
    }

    #[must_use]
    pub const fn with_uri(mut self, uri: &'a [u8]) -> Self {
        self.uri = Some(uri);
        self
    }

    #[must_use]
    pub const fn with_optional_uri(mut self, uri: Option<&'a [u8]>) -> Self {
        self.uri = uri;
        self
    }

    #[must_use]
    pub const fn with_options_json(mut self, options_json: &'a [u8]) -> Self {
        self.options_json = options_json;
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

    #[must_use]
    pub const fn uri(&self) -> Option<&'a [u8]> {
        self.uri
    }

    #[must_use]
    pub const fn options_json(&self) -> &'a [u8] {
        self.options_json
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
    output_plan: Option<BindingOutputPlan>,
}

#[derive(Debug)]
struct BindingOperationExecution {
    operation: BindingOperationKind,
    output: BindingOperationOutput,
}

impl BindingOperationExecution {
    fn into_data(self) -> Result<Vec<u8>, BindingError> {
        u64::try_from(self.output.data.len()).map_err(|_| {
            BindingError::internal("operation result byte length exceeds unsigned 64-bit range")
        })?;
        Ok(self.output.data)
    }

    fn into_result(
        self,
        runtime_policy_id: &'static str,
    ) -> Result<BindingOperationResult, BindingError> {
        operation_result(self.operation, runtime_policy_id, self.output)
    }
}

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum BindingOutputPlan {
    Raster(BindingRasterOutputPlan),
    PdfFilterImages(BindingPdfFilterImageOutputPlan),
    Unknown(BindingUnknownOutputPlan),
}

impl BindingOutputPlan {
    #[must_use]
    pub fn kind(&self) -> &str {
        match self {
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

        Ok(Self {
            version,
            operation_id,
            media_type,
            runtime_policy,
            byte_length,
            output_plan,
            json: Arc::from(json),
        })
    }

    fn from_execution(
        operation: BindingOperationKind,
        runtime_policy: &str,
        byte_length: u64,
        output_plan: Option<BindingOutputPlan>,
    ) -> Result<Self, BindingError> {
        #[cfg(test)]
        METADATA_SERIALIZATION_COUNT.with(|count| count.set(count.get() + 1));

        let json = serde_json::to_vec(&BindingOperationMetadataWire {
            version: BINDING_OPERATION_SCHEMA_VERSION,
            operation_id: operation.operation_id(),
            media_type: operation.media_type(),
            runtime_policy,
            byte_length,
            output_plan: output_plan.as_ref(),
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
    pub const fn output_plan(&self) -> Option<&BindingOutputPlan> {
        self.output_plan.as_ref()
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
    pub(crate) fn plain(data: Vec<u8>) -> Self {
        Self {
            data,
            output_plan: None,
        }
    }

    #[cfg(any(feature = "png", feature = "jpeg"))]
    pub(crate) fn raster(data: Vec<u8>, plan: merman::svg::export::RasterPlan) -> Self {
        Self {
            data,
            output_plan: Some(BindingOutputPlan::Raster(BindingRasterOutputPlan {
                requested_width_px: plan.requested_width_px,
                requested_height_px: plan.requested_height_px,
                width_px: plan.width_px,
                height_px: plan.height_px,
                requested_scale: plan.requested_scale,
                effective_scale: plan.effective_scale,
                limited: plan.limited,
            })),
        }
    }

    #[cfg(feature = "pdf")]
    pub(crate) fn pdf(data: Vec<u8>, plan: merman::svg::export::PdfFilterImagePlan) -> Self {
        Self {
            data,
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
}

fn parse_output_plan(value: &Value) -> Result<BindingOutputPlan, BindingError> {
    let object = value.as_object().ok_or_else(|| {
        BindingError::invalid_argument(
            "invalid operation metadata JSON: output_plan must be an object",
        )
    })?;
    let kind = required_string(object, "kind")?;
    match kind {
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
    crate::artifact_contract::default_artifact_contract().execute_once(request)
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
        let operation = resolve_operation_request(&request)?;
        crate::common::validate_one_shot_resource_options(
            request.options_json,
            operation.resource_scope(),
        )?;
        let engine = self.create_engine(request.options_json)?;
        let admitted = self.admit_operation(operation)?;
        engine.execute_admitted(admitted, request.source, request.uri)
    }

    pub(crate) fn execute_once_data(
        &self,
        request: BindingOperationRequest<'_>,
    ) -> Result<Vec<u8>, BindingError> {
        let operation = resolve_operation_request(&request)?;
        crate::common::validate_one_shot_resource_options(
            request.options_json,
            operation.resource_scope(),
        )?;
        let engine = self.create_engine(request.options_json)?;
        let admitted = self.admit_operation(operation)?;
        engine.execute_admitted_data(admitted, request.source, request.uri)
    }
}

impl BindingEngine {
    /// Executes one operation against this immutable reusable engine.
    pub fn execute(
        &self,
        request: BindingOperationRequest<'_>,
    ) -> Result<BindingOperationResult, BindingError> {
        self.execute_request(request)
            .and_then(|execution| execution.into_result(self.runtime_policy_id()))
    }

    pub(crate) fn execute_data(
        &self,
        request: BindingOperationRequest<'_>,
    ) -> Result<Vec<u8>, BindingError> {
        self.execute_request(request)
            .and_then(BindingOperationExecution::into_data)
    }

    fn execute_request(
        &self,
        request: BindingOperationRequest<'_>,
    ) -> Result<BindingOperationExecution, BindingError> {
        let operation = resolve_operation_request(&request)?;
        let prepared = self.prepare_request_overlay(operation, request.options_json)?;
        let admitted = self.admit_operation(operation)?;
        let output = match prepared {
            crate::engine::PreparedRequestOverlay::Unchanged => {
                self.execute_admitted_output(admitted, request.source, request.uri)
            }
            crate::engine::PreparedRequestOverlay::Override(configs) => {
                self.execute_request_projection(admitted, configs, request.source, request.uri)
            }
        }?;
        Ok(BindingOperationExecution { operation, output })
    }

    pub(crate) fn execute_admitted(
        &self,
        admitted: AdmittedArtifactOperation,
        source: &[u8],
        uri: Option<&[u8]>,
    ) -> Result<BindingOperationResult, BindingError> {
        self.execute_admitted_request(admitted, source, uri)
            .and_then(|execution| execution.into_result(self.runtime_policy_id()))
    }

    pub(crate) fn execute_admitted_data(
        &self,
        admitted: AdmittedArtifactOperation,
        source: &[u8],
        uri: Option<&[u8]>,
    ) -> Result<Vec<u8>, BindingError> {
        self.execute_admitted_request(admitted, source, uri)
            .and_then(BindingOperationExecution::into_data)
    }

    fn execute_admitted_request(
        &self,
        admitted: AdmittedArtifactOperation,
        source: &[u8],
        uri: Option<&[u8]>,
    ) -> Result<BindingOperationExecution, BindingError> {
        let operation = admitted.operation();
        let output = self.execute_admitted_output(admitted, source, uri)?;
        Ok(BindingOperationExecution { operation, output })
    }

    pub(crate) fn execute_admitted_output(
        &self,
        admitted: AdmittedArtifactOperation,
        source: &[u8],
        uri: Option<&[u8]>,
    ) -> Result<BindingOperationOutput, BindingError> {
        let operation = admitted.operation();
        match operation.key() {
            OperationKey::Png => self.render_png_output(source),
            OperationKey::Jpeg => self.render_jpeg_output(source),
            OperationKey::Pdf => self.render_pdf_output(source),
            OperationKey::Svg => self
                .render_svg_data(source)
                .map(BindingOperationOutput::plain),
            OperationKey::SvgPlanJson => self
                .svg_plan_json_data(source)
                .map(BindingOperationOutput::plain),
            OperationKey::Ascii => self
                .render_ascii_data(source)
                .map(BindingOperationOutput::plain),
            OperationKey::SemanticJson => self
                .parse_json_data(source)
                .map(BindingOperationOutput::plain),
            OperationKey::LayoutJson => self
                .layout_json_data(source)
                .map(BindingOperationOutput::plain),
            OperationKey::AnalysisJson => self
                .analyze_json_data(source)
                .map(BindingOperationOutput::plain),
            OperationKey::AnalysisFactsJson => self
                .analysis_facts_json_data(source)
                .map(BindingOperationOutput::plain),
            OperationKey::ValidationJson => self
                .validate_json_data(source)
                .map(BindingOperationOutput::plain),
            OperationKey::DocumentAnalysisJson => self
                .analyze_document_json_data(source, uri.expect("validated document URI presence"))
                .map(BindingOperationOutput::plain),
            OperationKey::DocumentAnalysisFactsJson => self
                .analyze_document_facts_json_data(
                    source,
                    uri.expect("validated document URI presence"),
                )
                .map(BindingOperationOutput::plain),
        }
    }
}

fn operation_result(
    operation: BindingOperationKind,
    runtime_policy_id: &'static str,
    output: BindingOperationOutput,
) -> Result<BindingOperationResult, BindingError> {
    let BindingOperationOutput { data, output_plan } = output;
    let byte_length = u64::try_from(data.len()).map_err(|_| {
        BindingError::internal("operation result byte length exceeds unsigned 64-bit range")
    })?;
    let metadata = BindingOperationMetadata::from_execution(
        operation,
        runtime_policy_id,
        byte_length,
        output_plan,
    )?;

    Ok(BindingOperationResult {
        operation,
        media_type: operation.media_type(),
        data,
        metadata,
    })
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
mod tests {
    use super::*;

    #[test]
    fn request_builder_preserves_execution_time_validation() {
        let request = BindingOperationRequest::new("document-analysis-json", b"flowchart TD\nA")
            .with_uri(b"file:///diagram.mmd")
            .with_options_json(br#"{"parse":{"suppress_errors":true}}"#);

        assert_eq!(request.operation_id(), "document-analysis-json");
        assert_eq!(request.source(), b"flowchart TD\nA");
        assert_eq!(request.uri(), Some(b"file:///diagram.mmd".as_slice()));
        assert_eq!(
            request.options_json(),
            br#"{"parse":{"suppress_errors":true}}"#
        );

        let defaults = BindingOperationRequest::new("semantic-json", b"flowchart TD\nA");
        assert_eq!(defaults.uri(), None);
        assert_eq!(defaults.options_json(), b"");
    }

    #[test]
    fn typed_metadata_decodes_known_output_plans() {
        let json = br#"{
            "version":1,
            "operation_id":"png",
            "media_type":"image/png",
            "runtime_policy":"custom",
            "byte_length":42,
            "future_top_level":{"kept":true},
            "output_plan":{
                "kind":"raster",
                "requested_width_px":320.5,
                "requested_height_px":200.25,
                "width_px":320,
                "height_px":200,
                "requested_scale":2.0,
                "effective_scale":1.5,
                "limited":true,
                "future_plan_field":"kept"
            }
        }"#;
        let metadata = BindingOperationMetadata::from_json_bytes(json).unwrap();

        assert_eq!(metadata.version(), 1);
        assert_eq!(metadata.operation_id(), "png");
        assert_eq!(metadata.media_type(), "image/png");
        assert_eq!(metadata.runtime_policy(), "custom");
        assert_eq!(metadata.byte_length(), 42);
        assert_eq!(metadata.json_bytes(), json);
        let Some(BindingOutputPlan::Raster(plan)) = metadata.output_plan() else {
            panic!("expected a raster output plan");
        };
        assert_eq!(plan.width_px(), 320);
        assert_eq!(plan.height_px(), 200);
        assert_eq!(plan.effective_scale(), 1.5);
        assert!(plan.limited());
    }

    #[test]
    fn typed_metadata_preserves_unknown_plan_and_exact_original_json() {
        let json = br#"{ "version": 1, "operation_id": "future", "media_type": "application/x-future", "runtime_policy": "future-policy", "byte_length": 7, "output_plan": { "kind": "future-plan", "nested": { "answer": 42 } } }"#;
        let metadata = BindingOperationMetadata::from_json_bytes(json).unwrap();

        assert_eq!(metadata.json_bytes(), json);
        assert_eq!(metadata.runtime_policy(), "future-policy");
        let Some(BindingOutputPlan::Unknown(plan)) = metadata.output_plan() else {
            panic!("expected an unknown output plan");
        };
        assert_eq!(plan.kind(), "future-plan");
        assert_eq!(plan.value()["nested"]["answer"], 42);
    }

    #[test]
    fn known_output_plan_requires_all_schema_one_fields() {
        let error = BindingOperationMetadata::from_json_bytes(
            br#"{"version":1,"operation_id":"png","media_type":"image/png","runtime_policy":"deterministic","byte_length":1,"output_plan":{"kind":"raster"}}"#,
        )
        .unwrap_err();

        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("requested_width_px"));
    }

    #[test]
    fn descriptor_owned_operation_ids_round_trip() {
        let operations = BindingOperationKind::all().collect::<Vec<_>>();
        assert_eq!(operations.len(), 13);
        for operation in operations {
            assert_eq!(
                BindingOperationKind::from_id(operation.operation_id()).unwrap(),
                operation
            );
            assert_eq!(operation.key().spec().id, operation.operation_id());
            assert!(!operation.media_type().is_empty());
        }
    }

    #[test]
    fn svg_capability_planning_is_a_descriptor_owned_operation() {
        let operation = BindingOperationKind::from_id("svg-plan-json").unwrap();

        assert_eq!(operation.availability_capability_id(), Some("svg"));
        assert_eq!(operation.media_type(), "application/json");
        assert!(!operation.requires_uri());
    }

    #[test]
    fn unknown_operation_is_a_typed_error() {
        let error = BindingOperationKind::from_id("bitmap").unwrap_err();
        assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
        assert_eq!(error.kind(), crate::BindingErrorKind::UnknownOperation);
        assert_eq!(error.capability_id(), None);
        assert!(error.message().contains("unknown operation `bitmap`"));
    }

    #[test]
    fn semantic_parse_is_a_base_operation_not_a_fake_feature_capability() {
        let semantic = BindingOperationKind::from_id("semantic-json").unwrap();
        assert_eq!(semantic.availability_capability_id(), None);
        assert!(!semantic.requires_uri());

        for operation in BindingOperationKind::all() {
            if operation.requires_uri() {
                assert!(operation.operation_id().starts_with("document-analysis-"));
            }
        }
    }

    #[test]
    fn transport_options_default_to_deterministic_policy() {
        let engine = BindingEngine::from_options(b"").unwrap();
        let result = engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
            })
            .unwrap();
        let metadata: serde_json::Value = serde_json::from_slice(result.metadata_json()).unwrap();

        assert_eq!(metadata["runtime_policy"], "deterministic");
    }

    #[test]
    fn operation_request_cannot_override_engine_runtime_policy() {
        let engine = BindingEngine::from_options(b"").unwrap();
        let error = engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: br#"{"runtime_policy":"native"}"#,
            })
            .unwrap_err();

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("cannot set runtime_policy"));
    }

    #[test]
    fn operation_request_merges_wrapped_options_over_direct_engine_options() {
        let engine =
            BindingEngine::from_options(br#"{"resources":{"limits":{"max_source_bytes":4096}}}"#)
                .unwrap();
        let result = engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: br#"{"analysis":{"resources":{"limits":{"max_source_bytes":2048}}}}"#,
            })
            .unwrap();
        assert!(!result.data.is_empty());
    }

    #[test]
    fn reusable_semantic_output_is_stable_across_empty_version_and_real_overlays() {
        let engine =
            BindingEngine::from_options(br#"{"parse":{"suppress_errors":false},"version":2}"#)
                .unwrap();
        let execute = |options_json| {
            engine
                .execute(BindingOperationRequest {
                    operation_id: "semantic-json",
                    source: b"flowchart TD\nA --> B",
                    uri: None,
                    options_json,
                })
                .unwrap()
        };

        let empty = execute(b"");
        for unchanged in [
            br#"{}"#.as_slice(),
            br#"{"version":2}"#.as_slice(),
            b"{\n  \"version\": 2\n}".as_slice(),
        ] {
            let result = execute(unchanged);
            assert_eq!(result.data, empty.data);
            assert_eq!(result.metadata_json(), empty.metadata_json());
        }
        let real = execute(br#"{"parse":{"suppress_errors":true}}"#);

        assert_eq!(real.data, empty.data);
        assert_eq!(real.metadata_json(), empty.metadata_json());
    }

    #[test]
    fn reusable_byte_execution_matches_result_data_and_errors_without_metadata() {
        let engine = BindingEngine::from_options(b"").unwrap();
        let request = BindingOperationRequest::new("semantic-json", b"flowchart TD\nA --> B");
        let result = engine.execute(request).unwrap();

        reset_metadata_serialization_count();
        assert_eq!(engine.parse_json(request.source()).unwrap(), result.data());
        assert_eq!(metadata_serialization_count(), 0);

        reset_metadata_serialization_count();
        assert_eq!(engine.execute_data(request).unwrap(), result.data());
        assert_eq!(metadata_serialization_count(), 0);

        for request in [
            BindingOperationRequest::new("unknown-operation", b"flowchart TD\nA --> B")
                .with_options_json(b"{"),
            BindingOperationRequest::new("document-analysis-json", b"flowchart TD\nA --> B")
                .with_options_json(b"{"),
            BindingOperationRequest::new("semantic-json", b"flowchart TD\nA --> B")
                .with_options_json(b"{"),
            BindingOperationRequest::new("semantic-json", b"flowchart TD\nA --> B")
                .with_options_json(br#"{"resources":{"limits":{"max_source_bytes":4}}}"#),
        ] {
            let expected = engine.execute(request).unwrap_err();

            reset_metadata_serialization_count();
            assert_eq!(engine.execute_data(request).unwrap_err(), expected);
            assert_eq!(metadata_serialization_count(), 0);
        }
    }

    #[test]
    fn one_shot_byte_execution_matches_result_data_and_errors_without_metadata() {
        let request = BindingOperationRequest::new("semantic-json", b"flowchart TD\nA --> B");
        let result = execute_once(request).unwrap();

        reset_metadata_serialization_count();
        assert_eq!(
            crate::parse_json(request.source(), b"").unwrap(),
            result.data()
        );
        assert_eq!(metadata_serialization_count(), 0);

        reset_metadata_serialization_count();
        assert_eq!(execute_once_data(request).unwrap(), result.data());
        assert_eq!(metadata_serialization_count(), 0);

        for request in [
            BindingOperationRequest::new("unknown-operation", b"flowchart TD\nA --> B")
                .with_options_json(b"{"),
            BindingOperationRequest::new("document-analysis-json", b"flowchart TD\nA --> B")
                .with_options_json(b"{"),
            BindingOperationRequest::new("semantic-json", b"flowchart TD\nA --> B")
                .with_options_json(b"{"),
            BindingOperationRequest::new("semantic-json", b"flowchart TD\nA --> B")
                .with_options_json(br#"{"resources":{"limits":{"max_source_bytes":4}}}"#),
        ] {
            let expected = execute_once(request).unwrap_err();

            reset_metadata_serialization_count();
            assert_eq!(execute_once_data(request).unwrap_err(), expected);
            assert_eq!(metadata_serialization_count(), 0);
        }
    }

    #[test]
    fn reusable_engine_rejects_ambiguous_analysis_wrappers_at_construction() {
        let error = BindingEngine::from_options(
            br#"{
                "merman": { "fixed_today": "2025-01-01" },
                "analysis": {}
            }"#,
        )
        .err()
        .expect("ambiguous wrappers must fail before a reusable engine is created");

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(
            error
                .message()
                .contains("must not contain both `analysis` and `merman` wrappers"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn unknown_operation_precedes_invalid_request_options() {
        let engine = BindingEngine::from_options(b"").unwrap();
        let error = engine
            .execute(BindingOperationRequest {
                operation_id: "unknown-operation",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"{",
            })
            .expect_err("operation resolution runs before request option parsing");

        assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
        assert_eq!(error.kind(), crate::BindingErrorKind::UnknownOperation);
        assert!(error.message().contains("unknown operation"));
    }

    #[test]
    fn uri_presence_validation_precedes_invalid_request_options() {
        let engine = BindingEngine::from_options(b"").unwrap();

        let missing = engine
            .execute(BindingOperationRequest {
                operation_id: "document-analysis-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"{",
            })
            .expect_err("missing URI is rejected before malformed options");
        assert_eq!(missing.status(), BindingStatus::InvalidArgument);
        assert!(missing.message().contains("requires a document URI"));

        let unexpected = engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: b"flowchart TD\nA --> B",
                uri: Some(b"file:///diagram.mmd"),
                options_json: b"{",
            })
            .expect_err("unexpected URI is rejected before malformed options");
        assert_eq!(unexpected.status(), BindingStatus::InvalidArgument);
        assert!(
            unexpected
                .message()
                .contains("does not accept a document URI")
        );
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn invalid_request_options_precede_invalid_document_uri_bytes() {
        let engine = BindingEngine::from_options(b"").unwrap();
        let invalid_uri = [0xff];

        let options_error = engine
            .execute(BindingOperationRequest {
                operation_id: "document-analysis-json",
                source: b"flowchart TD\nA --> B",
                uri: Some(&invalid_uri),
                options_json: b"{",
            })
            .expect_err("options are parsed before URI bytes are decoded");
        assert_eq!(options_error.status(), BindingStatus::OptionsJsonError);

        let uri_error = engine
            .execute(BindingOperationRequest {
                operation_id: "document-analysis-json",
                source: b"flowchart TD\nA --> B",
                uri: Some(&invalid_uri),
                options_json: b"",
            })
            .expect_err("valid options allow execution to reach URI decoding");
        assert_eq!(uri_error.status(), BindingStatus::Utf8Error);
    }

    #[cfg(feature = "svg")]
    #[test]
    fn semantic_request_preserves_render_option_validation_before_source_errors() {
        let engine = BindingEngine::from_options(b"").unwrap();
        let invalid_source = [0xff];
        let error = engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: &invalid_source,
                uri: None,
                options_json: br#"{"svg":{"pipeline":"invalid-pipeline"}}"#,
            })
            .expect_err("artifact-wide request validation checks the render domain first");

        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(
            error
                .message()
                .contains("unsupported svg.pipeline: invalid-pipeline"),
            "unexpected error: {error:?}"
        );
    }

    #[cfg(all(feature = "analysis", feature = "svg"))]
    #[test]
    fn semantic_request_preserves_analysis_before_render_validation_order() {
        let engine = BindingEngine::from_options(b"").unwrap();
        let error = engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: br#"{
                    "lint": { "profile": "invalid-profile" },
                    "svg": { "pipeline": "invalid-pipeline" }
                }"#,
            })
            .expect_err("artifact-wide request validation checks analysis before rendering");

        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(
            error
                .message()
                .contains("lint.profile must be core, recommended, or strict"),
            "unexpected error: {error:?}"
        );
        assert!(!error.message().contains("svg.pipeline"));
    }

    #[cfg(not(feature = "png"))]
    #[test]
    fn invalid_request_options_precede_missing_operation_capability() {
        let engine = BindingEngine::from_options(b"").unwrap();
        let options_error = engine
            .execute(BindingOperationRequest {
                operation_id: "png",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"{",
            })
            .expect_err("request options are validated before operation execution");
        assert_eq!(options_error.status(), BindingStatus::OptionsJsonError);

        let capability_error = engine
            .execute(BindingOperationRequest {
                operation_id: "png",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
            })
            .expect_err("valid options reach the missing capability check");
        assert_eq!(
            capability_error.status(),
            BindingStatus::UnsupportedOperation
        );
        assert_eq!(
            capability_error.kind(),
            crate::BindingErrorKind::MissingCapability
        );
        assert_eq!(capability_error.capability_id(), Some("png"));
    }

    #[test]
    fn every_compiled_operation_shares_one_shot_and_reusable_result_contracts() {
        let engine = BindingEngine::from_options(b"").unwrap();

        for operation in BindingOperationKind::all().filter(|operation| operation.is_compiled()) {
            for options_json in [
                b"".as_slice(),
                br#"{"parse":{"suppress_errors":false}}"#.as_slice(),
            ] {
                let request = BindingOperationRequest {
                    operation_id: operation.operation_id(),
                    source: b"flowchart TD\nA --> B",
                    uri: operation
                        .requires_uri()
                        .then_some(b"file:///diagram.mmd".as_slice()),
                    options_json,
                };
                let one_shot = execute_once(request).unwrap_or_else(|error| {
                    panic!(
                        "one-shot operation `{}` failed: {}",
                        operation.operation_id(),
                        error.message()
                    )
                });
                let reusable = engine.execute(request).unwrap_or_else(|error| {
                    panic!(
                        "reusable operation `{}` failed: {}",
                        operation.operation_id(),
                        error.message()
                    )
                });

                assert_eq!(
                    one_shot,
                    reusable,
                    "operation={}, options={}",
                    operation.operation_id(),
                    String::from_utf8_lossy(options_json)
                );
                assert_eq!(one_shot.operation, operation);
                assert_eq!(one_shot.media_type, operation.media_type());
                let metadata: serde_json::Value =
                    serde_json::from_slice(one_shot.metadata_json()).unwrap();
                assert_eq!(
                    metadata["operation_id"],
                    operation.operation_id(),
                    "operation={}",
                    operation.operation_id()
                );
                assert_eq!(
                    metadata["media_type"],
                    operation.media_type(),
                    "operation={}",
                    operation.operation_id()
                );
                assert_eq!(
                    metadata["byte_length"],
                    one_shot.data.len(),
                    "operation={}",
                    operation.operation_id()
                );
            }
        }
    }

    #[cfg(feature = "svg")]
    #[test]
    fn one_shot_options_reject_limits_owned_by_another_operation() {
        let error = execute_once(BindingOperationRequest {
            operation_id: "semantic-json",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{"resources":{"limits":{"max_svg_bytes":1024}}}"#,
        })
        .unwrap_err();

        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("max_svg_bytes"));
        assert!(error.message().contains("semantic-model"));
    }

    #[test]
    fn reusable_request_resource_overlays_only_tighten_the_constructor_ceiling() {
        let engine = BindingEngine::from_options(
            br#"{"resources":{"profile":"constrained","limits":{"max_source_bytes":64}}}"#,
        )
        .unwrap();

        for options_json in [
            br#"{"resources":{"profile":"trusted-native"}}"#.as_slice(),
            br#"{"resources":{"limits":{"max_source_bytes":65}}}"#.as_slice(),
            br#"{"resources":null}"#.as_slice(),
        ] {
            let error = engine
                .execute(BindingOperationRequest {
                    operation_id: "semantic-json",
                    source: b"flowchart TD\nA --> B",
                    uri: None,
                    options_json,
                })
                .unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        }
    }

    #[test]
    fn request_resource_tightening_does_not_mutate_the_reusable_engine() {
        let engine = BindingEngine::from_options(
            br#"{"resources":{"profile":"constrained","limits":{"max_source_bytes":64}}}"#,
        )
        .unwrap();
        let request = BindingOperationRequest {
            operation_id: "semantic-json",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{"resources":{"limits":{"max_source_bytes":4}}}"#,
        };
        let error = engine.execute(request).unwrap_err();
        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);

        let baseline = engine
            .execute(BindingOperationRequest {
                options_json: b"",
                ..request
            })
            .unwrap();
        assert!(!baseline.data.is_empty());
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn ascii_request_cannot_widen_the_constructor_grid_ceiling() {
        let engine =
            BindingEngine::from_options(br#"{"resources":{"limits":{"max_ascii_grid_cells":1}}}"#)
                .unwrap();

        let error = engine
            .execute(BindingOperationRequest {
                operation_id: "ascii",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: br#"{"resources":{"limits":{"max_ascii_grid_cells":2}}}"#,
            })
            .unwrap_err();

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("max_ascii_grid_cells"));
    }

    #[test]
    fn one_shot_operation_may_choose_a_nondefault_profile() {
        let result = execute_once(BindingOperationRequest {
            operation_id: "semantic-json",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{"resources":{"profile":"trusted-native"}}"#,
        })
        .unwrap();

        assert!(!result.data.is_empty());
    }

    #[cfg(feature = "svg")]
    #[test]
    fn reusable_constructor_accepts_artifact_union_but_request_scope_rejects_sibling_limit() {
        let engine = BindingEngine::from_options(
            br#"{"resources":{"profile":"constrained","limits":{"max_svg_bytes":1048576}}}"#,
        )
        .unwrap();
        let error = engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: br#"{"resources":{"limits":{"max_svg_bytes":524288}}}"#,
            })
            .unwrap_err();

        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("max_svg_bytes"));
    }

    #[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[test]
    fn output_options_are_artifact_wide_at_construction_and_operation_scoped_per_request() {
        let engine = BindingEngine::from_options(
            br#"{"raster":{"scale":2},"jpeg":{"quality":85},"pdf":{"background":"white"}}"#,
        )
        .expect("constructor accepts the compiled artifact option union");
        engine
            .execute(BindingOperationRequest {
                operation_id: "semantic-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
            })
            .expect("unrelated constructor options do not affect semantic operations");

        let error = execute_once(BindingOperationRequest {
            operation_id: "semantic-json",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{"raster":{"scale":2}}"#,
        })
        .unwrap_err();
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("raster"));
    }

    #[test]
    fn explicit_native_policy_follows_the_default_artifact_exposure() {
        assert_eq!(
            crate::artifact_contract::DEFAULT_RUNTIME_POLICY,
            crate::RuntimePolicyExposure::BindingOptions
        );
        let contract = crate::artifact_contract::default_artifact_contract();
        let capabilities = contract.runtime_capabilities();
        let missing_adapter = contract
            .validate_native_runtime_policy()
            .err()
            .and_then(|error| error.capability_id());

        if let Some(missing_adapter) = missing_adapter {
            assert!(capabilities.system_adapter_ids.is_empty());
            let error = match BindingEngine::from_options(br#"{"runtime_policy":"native"}"#) {
                Ok(_) => panic!("the default artifact accepted an incomplete native policy"),
                Err(error) => error,
            };
            assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
            assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
            assert_eq!(error.capability_id(), Some(missing_adapter));

            let free_function_error =
                crate::parse_json(b"flowchart TD\nA --> B", br#"{"runtime_policy":"native"}"#)
                    .expect_err("free functions must honor the exact transport adapter set");
            assert_eq!(
                free_function_error.status(),
                BindingStatus::UnsupportedOperation
            );
            assert_eq!(
                free_function_error.kind(),
                crate::BindingErrorKind::MissingCapability
            );
            assert_eq!(free_function_error.capability_id(), Some(missing_adapter));
        } else {
            let engine = BindingEngine::from_options(br#"{"runtime_policy":"native"}"#).unwrap();
            let result = engine
                .execute(BindingOperationRequest {
                    operation_id: "semantic-json",
                    source: b"flowchart TD\nA --> B",
                    uri: None,
                    options_json: b"",
                })
                .unwrap();
            let metadata: serde_json::Value =
                serde_json::from_slice(result.metadata_json()).unwrap();

            assert_eq!(metadata["runtime_policy"], "native");
        }
    }

    #[cfg(feature = "svg")]
    #[test]
    fn generic_svg_operation_produces_versioned_metadata() {
        let engine = BindingEngine::new(b"").unwrap();
        let result = engine
            .execute(BindingOperationRequest {
                operation_id: "svg",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
            })
            .unwrap();

        assert_eq!(result.operation.operation_id(), "svg");
        assert_eq!(result.media_type, "image/svg+xml");
        assert!(result.data.starts_with(b"<svg"));
        let metadata: serde_json::Value = serde_json::from_slice(result.metadata_json()).unwrap();
        assert_eq!(metadata["version"], BINDING_OPERATION_SCHEMA_VERSION);
        assert_eq!(metadata["operation_id"], "svg");
        assert_eq!(metadata["byte_length"], result.data.len());
        assert_eq!(metadata["runtime_policy"], "deterministic");
    }

    #[cfg(feature = "svg")]
    #[test]
    fn generic_svg_plan_operation_reports_required_and_missing_capabilities() {
        let engine = BindingEngine::new(b"").unwrap();
        let result = engine
            .execute(BindingOperationRequest {
                operation_id: "svg-plan-json",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
            })
            .unwrap();

        assert_eq!(result.operation.operation_id(), "svg-plan-json");
        assert_eq!(result.media_type, "application/json");
        let plan: serde_json::Value = serde_json::from_slice(&result.data).unwrap();
        assert_eq!(plan["planned_operation_id"], "svg");
        assert_eq!(plan["missing_capability_ids"], serde_json::json!([]));
        assert_eq!(plan["ready"], true);
    }

    #[cfg(feature = "svg")]
    #[test]
    fn empty_request_presentation_inherits_the_reusable_engine_profile() {
        let engine = BindingEngine::new(
            br#"{
                "presentation": { "profile": "merman-modern" },
                "site_config": { "flowchart": { "defaultRenderer": "dagre-wrapper" } }
            }"#,
        )
        .unwrap();
        let execute = |options_json: &[u8]| {
            engine
                .execute(BindingOperationRequest {
                    operation_id: "svg-plan-json",
                    source: b"flowchart TD\nA --> B",
                    uri: None,
                    options_json,
                })
                .unwrap()
                .data
        };

        let baseline = execute(b"");
        let empty_overlay = execute(br#"{"presentation":{}}"#);
        assert_eq!(empty_overlay, baseline);

        let plan: serde_json::Value = serde_json::from_slice(&baseline).unwrap();
        assert_eq!(plan["presentation_profile_id"], "merman-modern");
        assert_eq!(plan["presentation_aspects"][1]["state"], "active");
        assert_eq!(plan["presentation_aspects"][2]["state"], "inactive");
        assert_eq!(plan["ready"], true);
    }

    #[cfg(feature = "svg")]
    #[test]
    fn request_options_override_nested_engine_options_without_mutating_the_baseline() {
        let engine = BindingEngine::new(
            br#"{
                "environment": { "text_measurement": "deterministic" },
                "svg": { "diagram_id": "base engine", "pipeline": "readable" }
            }"#,
        )
        .unwrap();
        let request_result = engine
            .execute(BindingOperationRequest {
                operation_id: "svg",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: br#"{"svg":{"diagram_id":"request override"}}"#,
            })
            .unwrap();
        let request_svg = String::from_utf8(request_result.data).unwrap();
        assert!(request_svg.contains("id=\"request-override\""));
        assert!(request_svg.contains("data-merman-foreignobject"));

        let baseline_result = engine
            .execute(BindingOperationRequest {
                operation_id: "svg",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
            })
            .unwrap();
        let baseline_svg = String::from_utf8(baseline_result.data).unwrap();
        assert!(baseline_svg.contains("id=\"base-engine\""));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn elk_layout_request_follows_the_resolved_dependency_feature_set() {
        let engine = BindingEngine::new(b"").unwrap();
        let result = engine.execute(BindingOperationRequest {
            operation_id: "svg",
            source: b"---\nconfig:\n  layout: elk\n---\nflowchart TD\n  A --> B\n",
            uri: None,
            options_json: b"",
        });

        if merman::svg::layout_elk_available() {
            assert_eq!(result.unwrap().media_type, "image/svg+xml");
        } else {
            let error = result.expect_err("ELK is not compiled");
            assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
            assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
            assert_eq!(error.capability_id(), Some("layout-elk"));
            assert!(error.message().contains("`layout-elk`"));
        }
    }

    #[cfg(feature = "png")]
    #[test]
    fn generic_png_operation_exposes_a_real_binary_output() {
        let engine = BindingEngine::new(b"").unwrap();
        let result = engine
            .execute(BindingOperationRequest {
                operation_id: "png",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
            })
            .unwrap();

        assert_eq!(result.media_type, "image/png");
        assert!(result.data.starts_with(b"\x89PNG\r\n\x1a\n"));
        let metadata: serde_json::Value = serde_json::from_slice(result.metadata_json()).unwrap();
        assert_eq!(metadata["output_plan"]["kind"], "raster");
        assert_eq!(metadata["output_plan"]["limited"], false);
        assert_eq!(metadata["output_plan"]["requested_scale"], 1.0);
        assert_eq!(metadata["output_plan"]["effective_scale"], 1.0);
    }

    #[cfg(feature = "png")]
    #[test]
    fn png_byte_and_result_convenience_paths_share_one_envelope() {
        let source = b"flowchart TD\nA --> B";
        let result = crate::render_png_result(source, b"").unwrap();

        reset_metadata_serialization_count();
        let bytes = crate::render_png(source, b"").unwrap();

        assert_eq!(metadata_serialization_count(), 0);
        assert_eq!(result.operation().operation_id(), "png");
        assert_eq!(result.media_type(), "image/png");
        assert_eq!(result.data(), bytes);
        assert!(matches!(
            result.metadata().output_plan(),
            Some(BindingOutputPlan::Raster(_))
        ));
    }

    #[cfg(feature = "png")]
    #[test]
    fn generic_png_operation_reports_resource_limited_effective_plan() {
        let result = execute_once(BindingOperationRequest {
            operation_id: "png",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{
                "version": 2,
                "raster": {"scale": 20},
                "resources": {"limits": {"max_raster_pixels": 4096}}
            }"#,
        })
        .unwrap();

        let metadata: serde_json::Value = serde_json::from_slice(result.metadata_json()).unwrap();
        let plan = &metadata["output_plan"];
        assert_eq!(plan["kind"], "raster");
        assert_eq!(plan["limited"], true);
        assert_eq!(plan["requested_scale"], 20.0);
        assert!(
            plan["effective_scale"].as_f64().unwrap() < plan["requested_scale"].as_f64().unwrap()
        );
        assert!(plan["width_px"].as_u64().unwrap() * plan["height_px"].as_u64().unwrap() <= 4096);
    }

    #[cfg(feature = "png")]
    #[test]
    fn reusable_png_request_overlay_reports_its_effective_plan_without_mutating_the_engine() {
        let engine = BindingEngine::from_options(
            br#"{"version":2,"resources":{"profile":"trusted-native"}}"#,
        )
        .unwrap();
        let limited = engine
            .execute(BindingOperationRequest {
                operation_id: "png",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: br#"{
                    "version": 2,
                    "raster": {"scale": 20},
                    "resources": {"limits": {"max_raster_pixels": 4096}}
                }"#,
            })
            .unwrap();
        let limited_metadata: serde_json::Value =
            serde_json::from_slice(limited.metadata_json()).unwrap();
        assert_eq!(limited_metadata["output_plan"]["limited"], true);
        assert!(
            limited_metadata["output_plan"]["width_px"]
                .as_u64()
                .unwrap()
                * limited_metadata["output_plan"]["height_px"]
                    .as_u64()
                    .unwrap()
                <= 4096
        );

        let baseline = engine
            .execute(BindingOperationRequest {
                operation_id: "png",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
            })
            .unwrap();
        let baseline_metadata: serde_json::Value =
            serde_json::from_slice(baseline.metadata_json()).unwrap();
        assert_eq!(baseline_metadata["output_plan"]["limited"], false);
        assert_eq!(baseline_metadata["output_plan"]["requested_scale"], 1.0);
    }

    #[cfg(feature = "pdf")]
    #[test]
    fn generic_pdf_operation_reports_effective_filter_plan() {
        let result = execute_once(BindingOperationRequest {
            operation_id: "pdf",
            source: b"flowchart TD\nA --> B",
            uri: None,
            options_json: br#"{"version":2,"pdf":{"filterScale":0.1}}"#,
        })
        .unwrap();

        let metadata: serde_json::Value = serde_json::from_slice(result.metadata_json()).unwrap();
        let plan = &metadata["output_plan"];
        assert_eq!(plan["kind"], "pdf-filter-images");
        assert_eq!(plan["requested_scale"], serde_json::json!(0.1));
        assert_eq!(plan["effective_scale"], serde_json::json!(0.1));
        assert!(
            std::str::from_utf8(result.metadata_json())
                .unwrap()
                .contains("\"requested_scale\":0.1"),
            "schema-1 metadata must preserve the historical f32 JSON representation"
        );
    }

    #[cfg(not(feature = "png"))]
    #[test]
    fn unavailable_operation_is_reported_before_execution() {
        let engine = BindingEngine::new(b"").unwrap();
        let error = engine
            .execute(BindingOperationRequest {
                operation_id: "png",
                source: b"flowchart TD\nA --> B",
                uri: None,
                options_json: b"",
            })
            .expect_err("PNG is not compiled");

        assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
        assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
        assert_eq!(error.capability_id(), Some("png"));
    }
}

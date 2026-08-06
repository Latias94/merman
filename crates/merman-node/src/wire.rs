use std::borrow::Cow;
#[cfg(not(target_arch = "wasm32"))]
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::LazyLock;

use merman_bindings_core::{
    ArtifactContractSpec, BINDING_OPERATION_SCHEMA_VERSION, BindingEngine, BindingError,
    BindingErrorKind, BindingIconRegistryErrorDetails, BindingOperationRequest,
    BindingPayloadSchemaKey, BindingResourceErrorDetails, BindingStatus, BindingTransportKey,
    CAPABILITY_DESCRIPTOR_DIGEST, CapabilityKey, OperationKey, RUNTIME_CATALOG_SCHEMA_VERSION,
    RuntimePolicyExposure, TargetKey, ValidatedArtifactContract,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

const NODE_WIRE_CONTRACT_JSON: &str =
    include_str!("../../../platforms/node/src/generated/node-wire-contract.json");
const NODE_TRANSPORT_API_VERSION: u32 = 1;
const NODE_BINDING_RESULT_PAYLOAD_VERSION: u32 = BindingPayloadSchemaKey::BindingResult.version();
const JSON_SAFE_INTEGER_MAX: u64 = 9_007_199_254_740_991;
const NODE_TARGET: TargetKey = if cfg!(target_arch = "wasm32") {
    TargetKey::Web
} else {
    TargetKey::Native
};
const NODE_OPERATIONS: &[OperationKey] = &[
    #[cfg(feature = "svg")]
    OperationKey::LayoutJson,
    OperationKey::SemanticJson,
    #[cfg(feature = "svg")]
    OperationKey::Svg,
    #[cfg(feature = "svg")]
    OperationKey::SvgPlanJson,
];
const NODE_SUPPLEMENTAL_CAPABILITIES: &[CapabilityKey] = &[
    #[cfg(feature = "layout-cytoscape")]
    CapabilityKey::LayoutCytoscape,
    #[cfg(feature = "layout-elk")]
    CapabilityKey::LayoutElk,
    #[cfg(feature = "math")]
    CapabilityKey::Math,
];

// Keep feature selection in the transport owner. Dependency features may be unified by Cargo.
static ARTIFACT_CONTRACT: ValidatedArtifactContract =
    ArtifactContractSpec::new(NODE_TARGET, BindingTransportKey::Node)
        .with_operations(NODE_OPERATIONS)
        .with_supplemental_capabilities(NODE_SUPPLEMENTAL_CAPABILITIES)
        .with_all_available_metadata()
        .with_runtime_policy_exposure(RuntimePolicyExposure::DeterministicOnly)
        .materialize();
static NODE_WIRE_CONTRACT: LazyLock<NodeWireContract> = LazyLock::new(|| {
    let contract = serde_json::from_str::<NodeWireContract>(NODE_WIRE_CONTRACT_JSON)
        .expect("the embedded Node wire contract must be valid JSON");
    contract
        .validate()
        .expect("the embedded Node wire contract must be internally consistent");
    contract
});

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NodeWireContract {
    schema_version: u32,
    package_id: String,
    artifact_id: String,
    transport_api_version: u32,
    binding_result_payload_version: u32,
    artifact: NodeArtifactProfile,
    documents: NodeDocumentLimitSet,
    fields: NodeFieldLimits,
}

impl NodeWireContract {
    fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "unsupported Node wire contract schema {}",
                self.schema_version
            ));
        }
        if self.package_id != "@mermanjs/node" {
            return Err(format!("unexpected Node package id `{}`", self.package_id));
        }
        if self.artifact_id != "merman-node-static-svg" {
            return Err(format!(
                "unexpected Node artifact id `{}`",
                self.artifact_id
            ));
        }
        if self.transport_api_version != NODE_TRANSPORT_API_VERSION {
            return Err(format!(
                "Node transport API version {} does not match Rust version {NODE_TRANSPORT_API_VERSION}",
                self.transport_api_version
            ));
        }
        if self.binding_result_payload_version != NODE_BINDING_RESULT_PAYLOAD_VERSION {
            return Err(format!(
                "Node binding-result version {} does not match Rust version {NODE_BINDING_RESULT_PAYLOAD_VERSION}",
                self.binding_result_payload_version
            ));
        }
        self.documents.validate()?;
        self.fields.validate(&self.documents)?;
        self.artifact.validate(self.fields)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NodeArtifactProfile {
    capability_ids: Vec<String>,
    output_ids: Vec<String>,
    output_contracts: Vec<NodeArtifactOutputContract>,
    system_adapter_ids: Vec<String>,
    operation_ids: Vec<String>,
    metadata_ids: Vec<String>,
    option_group_ids: Vec<String>,
    constructor_service_ids: Vec<String>,
    text_measurement_provider_ids: Vec<String>,
}

impl NodeArtifactProfile {
    fn validate(&self, fields: NodeFieldLimits) -> Result<(), String> {
        for (label, ids) in [
            ("artifact.capability_ids", &self.capability_ids),
            ("artifact.output_ids", &self.output_ids),
            ("artifact.system_adapter_ids", &self.system_adapter_ids),
            ("artifact.operation_ids", &self.operation_ids),
            ("artifact.metadata_ids", &self.metadata_ids),
            ("artifact.option_group_ids", &self.option_group_ids),
            (
                "artifact.constructor_service_ids",
                &self.constructor_service_ids,
            ),
            (
                "artifact.text_measurement_provider_ids",
                &self.text_measurement_provider_ids,
            ),
        ] {
            if ids.iter().any(String::is_empty) || ids.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(format!(
                    "{label} must contain sorted, unique, non-empty IDs"
                ));
            }
            if ids
                .iter()
                .any(|id| id.len() > fields.capability_id_utf8_bytes)
            {
                return Err(format!("{label} contains an ID beyond the Node field limit"));
            }
        }
        if self.output_contracts.len() != self.output_ids.len()
            || self
                .output_contracts
                .iter()
                .zip(&self.output_ids)
                .any(|(contract, id)| contract.id != *id)
        {
            return Err(
                "artifact.output_contracts must exactly cover artifact.output_ids".to_owned(),
            );
        }
        for contract in &self.output_contracts {
            if contract.media_type.is_empty()
                || contract.media_type.len() > fields.media_type_utf8_bytes
                || !(contract.system_fonts.is_null() || contract.system_fonts.is_object())
                || !(contract.embedded_images.is_null() || contract.embedded_images.is_object())
            {
                return Err("artifact.output_contracts contains an invalid contract".to_owned());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NodeArtifactOutputContract {
    id: String,
    media_type: String,
    system_fonts: Value,
    embedded_images: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NodeDocumentLimitSet {
    identity: NodeDocumentLimits,
    binding_options: NodeDocumentLimits,
    request: NodeDocumentLimits,
    runtime_catalog: NodeDocumentLimits,
    response: NodeDocumentLimits,
    error: NodeDocumentLimits,
    metadata: NodeDocumentLimits,
}

impl NodeDocumentLimitSet {
    fn validate(&self) -> Result<(), String> {
        for (label, limits) in [
            ("identity", self.identity),
            ("binding_options", self.binding_options),
            ("request", self.request),
            ("runtime_catalog", self.runtime_catalog),
            ("response", self.response),
            ("error", self.error),
            ("metadata", self.metadata),
        ] {
            limits.validate(label)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NodeDocumentLimits {
    max_utf8_bytes: usize,
    max_depth: usize,
    max_members: usize,
    max_tokens: usize,
    max_string_utf8_bytes: usize,
}

impl NodeDocumentLimits {
    fn validate(self, label: &str) -> Result<(), String> {
        if self.max_utf8_bytes == 0
            || self.max_depth == 0
            || self.max_members == 0
            || self.max_tokens == 0
            || self.max_string_utf8_bytes == 0
            || self.max_string_utf8_bytes > self.max_utf8_bytes
        {
            return Err(format!(
                "documents.{label} contains invalid zero or document limits"
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct NodeFieldLimits {
    operation_id_utf8_bytes: usize,
    media_type_utf8_bytes: usize,
    metadata_id_utf8_bytes: usize,
    uri_utf8_bytes: usize,
    source_utf8_bytes: usize,
    options_json_utf8_bytes: usize,
    data_utf8_bytes: usize,
    metadata_json_utf8_bytes: usize,
    error_code_name_utf8_bytes: usize,
    error_kind_utf8_bytes: usize,
    error_message_utf8_bytes: usize,
    capability_id_utf8_bytes: usize,
    package_version_utf8_bytes: usize,
    contract_digest_utf8_bytes: usize,
}

impl NodeFieldLimits {
    fn validate(self, documents: &NodeDocumentLimitSet) -> Result<(), String> {
        for (label, limit) in [
            ("operation_id_utf8_bytes", self.operation_id_utf8_bytes),
            ("media_type_utf8_bytes", self.media_type_utf8_bytes),
            ("metadata_id_utf8_bytes", self.metadata_id_utf8_bytes),
            ("uri_utf8_bytes", self.uri_utf8_bytes),
            ("source_utf8_bytes", self.source_utf8_bytes),
            ("options_json_utf8_bytes", self.options_json_utf8_bytes),
            ("data_utf8_bytes", self.data_utf8_bytes),
            ("metadata_json_utf8_bytes", self.metadata_json_utf8_bytes),
            (
                "error_code_name_utf8_bytes",
                self.error_code_name_utf8_bytes,
            ),
            ("error_kind_utf8_bytes", self.error_kind_utf8_bytes),
            ("error_message_utf8_bytes", self.error_message_utf8_bytes),
            ("capability_id_utf8_bytes", self.capability_id_utf8_bytes),
            (
                "package_version_utf8_bytes",
                self.package_version_utf8_bytes,
            ),
            (
                "contract_digest_utf8_bytes",
                self.contract_digest_utf8_bytes,
            ),
        ] {
            if limit == 0 {
                return Err(format!("fields.{label} must be positive"));
            }
        }
        if self.source_utf8_bytes > documents.request.max_string_utf8_bytes
            || self.options_json_utf8_bytes > documents.binding_options.max_utf8_bytes
            || self.data_utf8_bytes > documents.response.max_string_utf8_bytes
            || self.metadata_json_utf8_bytes > documents.metadata.max_utf8_bytes
            || self.error_message_utf8_bytes > documents.error.max_string_utf8_bytes
            || self.package_version_utf8_bytes > documents.identity.max_string_utf8_bytes
            || self.contract_digest_utf8_bytes > documents.identity.max_string_utf8_bytes
        {
            return Err("Node field limits exceed their owning document limits".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Exactly one transport-kind variant is constructed in each candidate build.
pub(crate) enum NodeTransportKind {
    Napi,
    Wasm,
}

impl NodeTransportKind {
    const fn id(self) -> &'static str {
        match self {
            Self::Napi => "napi",
            Self::Wasm => "wasm",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeOperationRequest {
    operation_id: String,
    source: String,
    uri: Option<String>,
    #[serde(default, deserialize_with = "deserialize_present_string")]
    options_json: Option<String>,
}

#[derive(Debug, Serialize)]
struct TransportIdentity<'a> {
    schema_version: u32,
    package_id: &'a str,
    artifact_id: &'a str,
    package_version: &'static str,
    transport_kind: &'static str,
    transport_api_version: u32,
    binding_result_payload_version: u32,
    capability_descriptor_digest: &'static str,
    wire_contract: &'a NodeWireContract,
}

#[derive(Debug, Serialize)]
struct SuccessEnvelope {
    version: u32,
    ok: bool,
    result: SuccessResult,
}

#[derive(Debug, Serialize)]
struct SuccessResult {
    operation_id: String,
    media_type: String,
    data: String,
    metadata_json: String,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope<'a> {
    version: u32,
    ok: bool,
    error: ErrorPayload<'a>,
}

#[derive(Debug, Serialize)]
struct ErrorPayload<'a> {
    code: i32,
    code_name: &'a str,
    kind: &'a str,
    capability_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<ErrorDetails<'a>>,
    message: Cow<'a, str>,
}

#[derive(Debug, Serialize)]
struct ErrorDetails<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    resource: Option<BindingResourceErrorDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon_registry: Option<&'a BindingIconRegistryErrorDetails>,
}

pub(crate) fn create_engine(options_json: &str) -> Result<BindingEngine, BindingError> {
    binding_boundary(|| create_engine_inner(options_json))
}

fn create_engine_inner(options_json: &str) -> Result<BindingEngine, BindingError> {
    validate_binding_options(options_json)?;
    node_artifact_contract().create_engine(options_json.as_bytes())
}

pub(crate) fn transport_identity_wire(kind: NodeTransportKind) -> Result<String, BindingError> {
    binding_boundary(|| transport_identity_wire_inner(kind))
}

fn transport_identity_wire_inner(kind: NodeTransportKind) -> Result<String, BindingError> {
    // Identity is an admission claim, so refuse to emit it if the compiled Rust artifact has
    // drifted from the descriptor embedded below.
    runtime_catalog_wire_inner()?;
    let contract = node_wire_contract();
    ensure_field(
        env!("CARGO_PKG_VERSION"),
        "identity package_version",
        contract.fields.package_version_utf8_bytes,
    )
    .map_err(producer_error)?;
    ensure_field(
        CAPABILITY_DESCRIPTOR_DIGEST,
        "identity capability_descriptor_digest",
        contract.fields.contract_digest_utf8_bytes,
    )
    .map_err(producer_error)?;

    serialize_bounded_json(
        &TransportIdentity {
            schema_version: contract.schema_version,
            package_id: &contract.package_id,
            artifact_id: &contract.artifact_id,
            package_version: env!("CARGO_PKG_VERSION"),
            transport_kind: kind.id(),
            transport_api_version: NODE_TRANSPORT_API_VERSION,
            binding_result_payload_version: NODE_BINDING_RESULT_PAYLOAD_VERSION,
            capability_descriptor_digest: CAPABILITY_DESCRIPTOR_DIGEST,
            wire_contract: contract,
        },
        "identity",
        contract.documents.identity,
    )
    .map_err(producer_error)
}

pub(crate) fn disposed_error() -> BindingError {
    BindingError::invalid_argument("Node transport engine has been disposed")
}

fn node_wire_contract() -> &'static NodeWireContract {
    &NODE_WIRE_CONTRACT
}

fn node_artifact_contract() -> &'static ValidatedArtifactContract {
    &ARTIFACT_CONTRACT
}

pub(crate) fn runtime_catalog_wire() -> Result<String, BindingError> {
    binding_boundary(runtime_catalog_wire_inner)
}

fn runtime_catalog_wire_inner() -> Result<String, BindingError> {
    let bytes = node_artifact_contract().runtime_catalog_json(NODE_TRANSPORT_API_VERSION)?;
    let text = String::from_utf8(bytes).map_err(|error| {
        BindingError::internal(format!("Node runtime catalog was not UTF-8: {error}"))
    })?;
    let catalog = deserialize_bounded_json::<Value>(
        &text,
        "runtime catalog",
        node_wire_contract().documents.runtime_catalog,
    )
    .map_err(producer_error)?;
    validate_runtime_catalog(&catalog).map_err(producer_error)?;
    Ok(text)
}

pub(crate) fn metadata_wire(id: &str) -> Result<String, BindingError> {
    binding_boundary(|| metadata_wire_inner(id))
}

fn metadata_wire_inner(id: &str) -> Result<String, BindingError> {
    let contract = node_wire_contract();
    if id.is_empty() {
        return Err(BindingError::invalid_argument(
            "Node metadata id must be non-empty",
        ));
    }
    ensure_field(id, "metadata id", contract.fields.metadata_id_utf8_bytes)
        .map_err(caller_argument_error)?;
    let bytes = node_artifact_contract().metadata_json(id)?;
    let text = String::from_utf8(bytes)
        .map_err(|error| BindingError::internal(format!("Node metadata was not UTF-8: {error}")))?;
    deserialize_bounded_json::<Value>(&text, "metadata", contract.documents.metadata)
        .map_err(producer_error)?;
    Ok(text)
}

pub(crate) fn execute_wire(engine: &BindingEngine, request_json: &str) -> String {
    match binding_boundary(|| execute_wire_inner(engine, request_json)) {
        Ok(response) => response,
        Err(error) => error_envelope(&error),
    }
}

fn execute_wire_inner(
    engine: &BindingEngine,
    request_json: &str,
) -> Result<String, BindingError> {
    let request = parse_operation_request(request_json)?;

    let result = engine.execute(
        BindingOperationRequest::new(&request.operation_id, request.source.as_bytes())
            .with_optional_uri(request.uri.as_deref().map(str::as_bytes))
            .with_options_json(request.options_json.as_deref().map_or(b"", str::as_bytes)),
    )?;
    success_envelope(result)
}

pub(crate) fn error_envelope(error: &BindingError) -> String {
    match transport_unwind_boundary(|| try_error_envelope(error)) {
        Ok(Ok(envelope)) => envelope,
        Ok(Err(_)) => fallback_error_envelope(),
        Err(()) => fallback_panic_envelope(),
    }
}

fn parse_operation_request(request_json: &str) -> Result<NodeOperationRequest, BindingError> {
    let contract = node_wire_contract();
    let request = deserialize_bounded_json::<NodeOperationRequest>(
        request_json,
        "operation request",
        contract.documents.request,
    )
    .map_err(caller_options_error)?;

    if request.operation_id.is_empty() {
        return Err(BindingError::invalid_options_json(
            "invalid Node operation request: operation_id must be non-empty",
        ));
    }
    ensure_field(
        &request.operation_id,
        "operation request operation_id",
        contract.fields.operation_id_utf8_bytes,
    )
    .map_err(caller_options_error)?;
    ensure_field(
        &request.source,
        "operation request source",
        contract.fields.source_utf8_bytes,
    )
    .map_err(caller_options_error)?;
    if let Some(uri) = request.uri.as_deref() {
        ensure_field(uri, "operation request uri", contract.fields.uri_utf8_bytes)
            .map_err(caller_options_error)?;
    }
    if let Some(options_json) = request.options_json.as_deref() {
        ensure_field(
            options_json,
            "operation request options_json",
            contract.fields.options_json_utf8_bytes,
        )
        .map_err(caller_options_error)?;
        deserialize_bounded_json::<Value>(
            options_json,
            "operation request options_json",
            contract.documents.binding_options,
        )
        .map_err(caller_options_error)?;
    }
    Ok(request)
}

fn validate_binding_options(options_json: &str) -> Result<(), BindingError> {
    if options_json.is_empty() {
        return Ok(());
    }
    let contract = node_wire_contract();
    ensure_field(
        options_json,
        "binding options",
        contract.fields.options_json_utf8_bytes,
    )
    .map_err(caller_options_error)?;
    deserialize_bounded_json::<Value>(
        options_json,
        "binding options",
        contract.documents.binding_options,
    )
    .map_err(caller_options_error)?;
    Ok(())
}

fn success_envelope(
    result: merman_bindings_core::BindingOperationResult,
) -> Result<String, BindingError> {
    let contract = node_wire_contract();
    let (operation, media_type, data, metadata) = result.into_parts();
    let operation_id = operation.operation_id();
    let data_length = u64::try_from(data.len()).map_err(|_| {
        BindingError::internal("Node response data length exceeds the unsigned 64-bit range")
    })?;
    ensure_field(
        operation_id,
        "response operation_id",
        contract.fields.operation_id_utf8_bytes,
    )
    .map_err(producer_error)?;
    ensure_field(
        media_type,
        "response media_type",
        contract.fields.media_type_utf8_bytes,
    )
    .map_err(producer_error)?;
    if metadata.version() != BINDING_OPERATION_SCHEMA_VERSION
        || metadata.operation_id() != operation_id
        || metadata.media_type() != media_type
        || metadata.runtime_policy() != "deterministic"
        || metadata.byte_length() != data_length
    {
        return Err(BindingError::internal(
            "Node operation metadata does not match its result envelope",
        ));
    }

    let data = String::from_utf8(data).map_err(|error| {
        BindingError::internal(format!(
            "Node static-SVG candidate received non-UTF-8 output for `{operation_id}`: {error}"
        ))
    })?;
    ensure_field(&data, "response data", contract.fields.data_utf8_bytes)
        .map_err(producer_error)?;

    let metadata_json = String::from_utf8(metadata.into_json_bytes()).map_err(|error| {
        BindingError::internal(format!("binding metadata was not UTF-8: {error}"))
    })?;
    ensure_field(
        &metadata_json,
        "response metadata_json",
        contract.fields.metadata_json_utf8_bytes,
    )
    .map_err(producer_error)?;
    deserialize_bounded_json::<Value>(
        &metadata_json,
        "nested operation metadata",
        contract.documents.metadata,
    )
    .map_err(producer_error)?;

    serialize_bounded_json(
        &SuccessEnvelope {
            version: NODE_BINDING_RESULT_PAYLOAD_VERSION,
            ok: true,
            result: SuccessResult {
                operation_id: operation_id.to_owned(),
                media_type: media_type.to_owned(),
                data,
                metadata_json,
            },
        },
        "response",
        contract.documents.response,
    )
    .map_err(producer_error)
}

fn try_error_envelope(error: &BindingError) -> Result<String, String> {
    let contract = node_wire_contract();
    let fields = contract.fields;
    validate_error_relation(error)?;
    ensure_field(
        error.status().code_name(),
        "error code_name",
        fields.error_code_name_utf8_bytes,
    )?;
    ensure_field(
        error.kind().id(),
        "error kind",
        fields.error_kind_utf8_bytes,
    )?;
    if let Some(capability_id) = error.capability_id() {
        ensure_field(
            capability_id,
            "error capability_id",
            fields.capability_id_utf8_bytes,
        )?;
    }
    let resource = error.resource_details();
    if resource.is_some_and(|details| {
        details.actual > JSON_SAFE_INTEGER_MAX || details.max > JSON_SAFE_INTEGER_MAX
    }) {
        return Err("error resource details exceed the JSON-safe integer range".to_owned());
    }
    let message = bounded_text(error.message(), fields.error_message_utf8_bytes);
    let details =
        (resource.is_some() || error.icon_registry_details().is_some()).then_some(ErrorDetails {
            resource,
            icon_registry: error.icon_registry_details(),
        });
    let envelope = ErrorEnvelope {
        version: NODE_BINDING_RESULT_PAYLOAD_VERSION,
        ok: false,
        error: ErrorPayload {
            code: error.status().code(),
            code_name: error.status().code_name(),
            kind: error.kind().id(),
            capability_id: error.capability_id(),
            details,
            message,
        },
    };
    let json = serialize_bounded_json(&envelope, "error", contract.documents.error)?;
    validate_bounded_json_text(&json, "response", contract.documents.response)?;
    Ok(json)
}

fn validate_error_relation(error: &BindingError) -> Result<(), String> {
    if error.status() == BindingStatus::Ok {
        return Err("error envelopes cannot carry MERMAN_OK".to_owned());
    }
    let valid = match error.kind() {
        BindingErrorKind::Generic => error.capability_id().is_none(),
        BindingErrorKind::UnknownOperation => {
            error.status() == BindingStatus::UnsupportedOperation && error.capability_id().is_none()
        }
        BindingErrorKind::MissingCapability => {
            error.status() == BindingStatus::UnsupportedOperation && error.capability_id().is_some()
        }
        BindingErrorKind::Busy => error.status() == BindingStatus::Busy,
        BindingErrorKind::ReentrantCall => {
            error.status() == BindingStatus::InvalidArgument && error.capability_id().is_none()
        }
    };
    if !valid {
        return Err("binding error kind/status/capability relation is inconsistent".to_owned());
    }
    Ok(())
}

fn fallback_error_envelope() -> String {
    format!(
        "{{\"version\":{NODE_BINDING_RESULT_PAYLOAD_VERSION},\"ok\":false,\"error\":{{\"code\":9,\"code_name\":\"MERMAN_INTERNAL_ERROR\",\"kind\":\"generic\",\"capability_id\":null,\"message\":\"failed to encode a bounded Node transport error\"}}}}"
    )
}

fn fallback_panic_envelope() -> String {
    format!(
        "{{\"version\":{NODE_BINDING_RESULT_PAYLOAD_VERSION},\"ok\":false,\"error\":{{\"code\":8,\"code_name\":\"MERMAN_PANIC\",\"kind\":\"generic\",\"capability_id\":null,\"message\":\"a Rust panic was caught at the Node transport boundary\"}}}}"
    )
}

fn panic_error() -> BindingError {
    BindingError::new(
        BindingStatus::Panic,
        "a Rust panic was caught at the Node transport boundary",
    )
}

fn binding_boundary<T>(
    operation: impl FnOnce() -> Result<T, BindingError>,
) -> Result<T, BindingError> {
    match transport_unwind_boundary(operation) {
        Ok(result) => result,
        Err(()) => Err(panic_error()),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn transport_unwind_boundary<T>(operation: impl FnOnce() -> T) -> Result<T, ()> {
    catch_unwind(AssertUnwindSafe(operation)).map_err(|_| ())
}

#[cfg(target_arch = "wasm32")]
fn transport_unwind_boundary<T>(operation: impl FnOnce() -> T) -> Result<T, ()> {
    // The wasm-size profile uses panic=abort, so the WebAssembly candidate cannot promise the
    // recoverable native unwind boundary. All ordinary typed failures still share the same wire.
    Ok(operation())
}

fn validate_runtime_catalog(catalog: &Value) -> Result<(), String> {
    let object = catalog
        .as_object()
        .ok_or_else(|| "runtime catalog root must be an object".to_owned())?;
    if object.get("schema_version").and_then(Value::as_u64)
        != Some(u64::from(RUNTIME_CATALOG_SCHEMA_VERSION))
        || object.get("transport_api_version").and_then(Value::as_u64)
            != Some(u64::from(NODE_TRANSPORT_API_VERSION))
        || object.get("package_version").and_then(Value::as_str) != Some(env!("CARGO_PKG_VERSION"))
    {
        return Err("runtime catalog header does not match the Node artifact".to_owned());
    }
    ensure_field(
        env!("CARGO_PKG_VERSION"),
        "runtime catalog package_version",
        node_wire_contract().fields.package_version_utf8_bytes,
    )?;

    let expected = &node_wire_contract().artifact;
    ensure_string_array(
        catalog.pointer("/capabilities/capability_ids"),
        &expected.capability_ids,
        "capabilities.capability_ids",
    )?;
    ensure_string_array(
        catalog.pointer("/capabilities/output_ids"),
        &expected.output_ids,
        "capabilities.output_ids",
    )?;
    ensure_string_array(
        catalog.pointer("/capabilities/system_adapter_ids"),
        &expected.system_adapter_ids,
        "capabilities.system_adapter_ids",
    )?;
    ensure_string_array(
        catalog.pointer("/capabilities/operation_ids"),
        &expected.operation_ids,
        "capabilities.operation_ids",
    )?;
    ensure_string_array(
        catalog.get("metadata_ids"),
        &expected.metadata_ids,
        "metadata_ids",
    )?;
    ensure_string_array(
        catalog.get("option_group_ids"),
        &expected.option_group_ids,
        "option_group_ids",
    )?;
    ensure_string_array(
        catalog.get("constructor_service_ids"),
        &expected.constructor_service_ids,
        "constructor_service_ids",
    )?;
    ensure_string_array(
        catalog.pointer("/capabilities/text_measurement/provider_ids"),
        &expected.text_measurement_provider_ids,
        "capabilities.text_measurement.provider_ids",
    )?;
    ensure_output_contract_array(
        catalog.get("output_contracts"),
        &expected.output_contracts,
    )?;
    ensure_object_id_array(
        catalog.get("constructor_service_contracts"),
        &expected.constructor_service_ids,
        "constructor_service_contracts",
    )?;
    Ok(())
}

fn ensure_output_contract_array(
    value: Option<&Value>,
    expected: &[NodeArtifactOutputContract],
) -> Result<(), String> {
    let actual = value
        .and_then(Value::as_array)
        .ok_or_else(|| "runtime catalog output_contracts must be an array".to_owned())?;
    if actual.len() != expected.len()
        || actual.iter().zip(expected).any(|(actual, expected)| {
            actual.get("id").and_then(Value::as_str) != Some(expected.id.as_str())
                || actual.get("media_type").and_then(Value::as_str)
                    != Some(expected.media_type.as_str())
                || actual.get("system_fonts") != Some(&expected.system_fonts)
                || actual.get("embedded_images") != Some(&expected.embedded_images)
        })
    {
        return Err(
            "runtime catalog output_contracts do not match the embedded Node artifact profile"
                .to_owned(),
        );
    }
    Ok(())
}

fn ensure_string_array(
    value: Option<&Value>,
    expected: &[String],
    label: &str,
) -> Result<(), String> {
    let actual = value
        .and_then(Value::as_array)
        .ok_or_else(|| format!("runtime catalog {label} must be an array"))?;
    if actual.len() != expected.len()
        || actual
            .iter()
            .zip(expected)
            .any(|(actual, expected)| actual.as_str() != Some(expected.as_str()))
    {
        return Err(format!(
            "runtime catalog {label} does not match the embedded Node artifact profile"
        ));
    }
    Ok(())
}

fn ensure_object_id_array(
    value: Option<&Value>,
    expected: &[String],
    label: &str,
) -> Result<(), String> {
    let actual = value
        .and_then(Value::as_array)
        .ok_or_else(|| format!("runtime catalog {label} must be an array"))?;
    if actual.len() != expected.len()
        || actual.iter().zip(expected).any(|(actual, expected)| {
            actual.get("id").and_then(Value::as_str) != Some(expected.as_str())
        })
    {
        return Err(format!(
            "runtime catalog {label} does not match the embedded Node artifact profile"
        ));
    }
    Ok(())
}

fn deserialize_present_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(deserializer).map(Some)
}

fn deserialize_bounded_json<T>(
    source: &str,
    label: &str,
    limits: NodeDocumentLimits,
) -> Result<T, String>
where
    T: DeserializeOwned,
{
    validate_bounded_json_text(source, label, limits)?;
    serde_json::from_str(source).map_err(|error| format!("{label} is not valid JSON: {error}"))
}

fn serialize_bounded_json(
    value: &impl Serialize,
    label: &str,
    limits: NodeDocumentLimits,
) -> Result<String, String> {
    let source = serde_json::to_string(value)
        .map_err(|error| format!("failed to serialize Node {label}: {error}"))?;
    validate_bounded_json_text(&source, label, limits)?;
    Ok(source)
}

fn validate_bounded_json_text(
    source: &str,
    label: &str,
    limits: NodeDocumentLimits,
) -> Result<(), String> {
    if source.len() > limits.max_utf8_bytes {
        return Err(format!(
            "{label} exceeds the {}-byte wire limit",
            limits.max_utf8_bytes
        ));
    }
    scan_bounded_json_text(source, label, limits)
}

fn scan_bounded_json_text(
    source: &str,
    label: &str,
    limits: NodeDocumentLimits,
) -> Result<(), String> {
    let bytes = source.as_bytes();
    let mut stack = Vec::new();
    let mut depth = 0usize;
    let mut members = 0usize;
    let mut tokens = 0usize;
    let mut index = 0usize;

    'scan: while index < bytes.len() {
        match bytes[index] {
            b' ' | b'\t' | b'\n' | b'\r' => index += 1,
            b'"' => {
                let (end, string_bytes) = scan_json_string_token(source, index, label)?;
                if string_bytes > limits.max_string_utf8_bytes {
                    return Err(format!(
                        "{label} contains a string exceeding the {}-byte field limit",
                        limits.max_string_utf8_bytes
                    ));
                }
                tokens += 1;
                index = end;
            }
            opening @ (b'{' | b'[') => {
                stack.push(opening);
                depth += 1;
                tokens += 1;
                if depth > limits.max_depth {
                    return Err(format!(
                        "{label} exceeds the structural depth limit {}",
                        limits.max_depth
                    ));
                }
                index += 1;
            }
            closing @ (b'}' | b']') => {
                let expected = if closing == b'}' { b'{' } else { b'[' };
                if stack.last().copied() != Some(expected) {
                    break 'scan;
                }
                stack.pop();
                depth = depth.saturating_sub(1);
                index += 1;
            }
            b':' => {
                members += 1;
                index += 1;
            }
            b'-' | b'0'..=b'9' => {
                let Some(end) = scan_json_number(bytes, index) else {
                    break 'scan;
                };
                index = end;
                tokens += 1;
            }
            b't' if bytes[index..].starts_with(b"true") => {
                tokens += 1;
                index += 4;
            }
            b'f' if bytes[index..].starts_with(b"false") => {
                tokens += 1;
                index += 5;
            }
            b'n' if bytes[index..].starts_with(b"null") => {
                tokens += 1;
                index += 4;
            }
            _ => index += 1,
        }

        if members > limits.max_members {
            return Err(format!(
                "{label} exceeds the member-work limit {}",
                limits.max_members
            ));
        }
        if tokens > limits.max_tokens {
            return Err(format!(
                "{label} exceeds the token-work limit {}",
                limits.max_tokens
            ));
        }
    }
    Ok(())
}

fn scan_json_string_token(
    source: &str,
    start: usize,
    label: &str,
) -> Result<(usize, usize), String> {
    let bytes = source.as_bytes();
    let mut decoded_bytes = 0usize;
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => return Ok((index + 1, decoded_bytes)),
            b'\\' => {
                let escaped = bytes
                    .get(index + 1)
                    .copied()
                    .ok_or_else(|| format!("{label} contains an unterminated JSON escape"))?;
                if escaped != b'u' {
                    decoded_bytes += 1;
                    index += 2;
                    continue;
                }
                let first = parse_json_unicode_escape(bytes, index, label)?;
                if (0xd800..=0xdbff).contains(&first) {
                    if bytes.get(index + 6) != Some(&b'\\') || bytes.get(index + 7) != Some(&b'u') {
                        return Err(format!(
                            "{label} contains an isolated JSON surrogate escape"
                        ));
                    }
                    let second = parse_json_unicode_escape(bytes, index + 6, label)?;
                    if !(0xdc00..=0xdfff).contains(&second) {
                        return Err(format!(
                            "{label} contains an isolated JSON surrogate escape"
                        ));
                    }
                    decoded_bytes += 4;
                    index += 12;
                } else if (0xdc00..=0xdfff).contains(&first) {
                    return Err(format!(
                        "{label} contains an isolated JSON surrogate escape"
                    ));
                } else {
                    decoded_bytes += char::from_u32(u32::from(first))
                        .expect("non-surrogate u16 is a Unicode scalar")
                        .len_utf8();
                    index += 6;
                }
            }
            byte if byte.is_ascii() => {
                decoded_bytes += 1;
                index += 1;
            }
            _ => {
                let character = source[index..]
                    .chars()
                    .next()
                    .expect("index is within a valid UTF-8 string");
                decoded_bytes += character.len_utf8();
                index += character.len_utf8();
            }
        }
    }
    Ok((source.len(), decoded_bytes))
}

fn parse_json_unicode_escape(bytes: &[u8], slash_index: usize, label: &str) -> Result<u16, String> {
    let digits = bytes
        .get(slash_index + 2..slash_index + 6)
        .ok_or_else(|| format!("{label} contains an incomplete JSON Unicode escape"))?;
    let mut value = 0u16;
    for digit in digits {
        let digit = hex_digit(*digit)
            .map_err(|_| format!("{label} contains an invalid JSON Unicode escape"))?;
        value = value
            .checked_mul(16)
            .and_then(|value| value.checked_add(u16::from(digit)))
            .ok_or_else(|| format!("{label} contains an invalid JSON Unicode escape"))?;
    }
    Ok(value)
}

fn hex_digit(digit: u8) -> Result<u8, String> {
    match digit {
        b'0'..=b'9' => Ok(digit - b'0'),
        b'a'..=b'f' => Ok(digit - b'a' + 10),
        b'A'..=b'F' => Ok(digit - b'A' + 10),
        _ => Err("invalid hexadecimal digit".to_owned()),
    }
}

fn scan_json_number(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start;
    if bytes.get(index) == Some(&b'-') {
        index += 1;
    }
    match bytes.get(index).copied()? {
        b'0' => index += 1,
        b'1'..=b'9' => {
            index += 1;
            while bytes
                .get(index)
                .copied()
                .is_some_and(|byte| byte.is_ascii_digit())
            {
                index += 1;
            }
        }
        _ => return None,
    }
    if bytes.get(index) == Some(&b'.')
        && bytes
            .get(index + 1)
            .copied()
            .is_some_and(|byte| byte.is_ascii_digit())
    {
        index += 2;
        while bytes
            .get(index)
            .copied()
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            index += 1;
        }
    }
    if matches!(bytes.get(index).copied(), Some(b'e' | b'E')) {
        let exponent = index;
        index += 1;
        if matches!(bytes.get(index).copied(), Some(b'+' | b'-')) {
            index += 1;
        }
        if !bytes
            .get(index)
            .copied()
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            return Some(exponent);
        }
        index += 1;
        while bytes
            .get(index)
            .copied()
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            index += 1;
        }
    }
    Some(index)
}

fn ensure_field(value: &str, label: &str, max_utf8_bytes: usize) -> Result<(), String> {
    if value.len() > max_utf8_bytes {
        return Err(format!(
            "{label} exceeds the {max_utf8_bytes}-byte field limit"
        ));
    }
    Ok(())
}

fn bounded_text(value: &str, max_utf8_bytes: usize) -> Cow<'_, str> {
    if value.len() <= max_utf8_bytes {
        return Cow::Borrowed(value);
    }
    let suffix = "...";
    let mut end = max_utf8_bytes.saturating_sub(suffix.len());
    while !value.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    Cow::Owned(format!("{}{}", &value[..end], suffix))
}

fn caller_options_error(message: String) -> BindingError {
    BindingError::invalid_options_json(format!("invalid Node transport JSON: {message}"))
}

fn caller_argument_error(message: String) -> BindingError {
    BindingError::invalid_argument(format!("invalid Node transport field: {message}"))
}

fn producer_error(message: String) -> BindingError {
    BindingError::internal(format!("invalid Node transport output: {message}"))
}

#[cfg(test)]
mod tests {
    use super::{
        NodeDocumentLimits, NodeTransportKind, create_engine, deserialize_bounded_json,
        error_envelope, execute_wire, metadata_wire, node_artifact_contract, node_wire_contract,
        parse_operation_request, runtime_catalog_wire, scan_bounded_json_text,
        transport_identity_wire, validate_bounded_json_text, validate_runtime_catalog,
    };

    #[test]
    fn runtime_catalog_reports_exact_static_svg_artifact() {
        let catalog: serde_json::Value =
            serde_json::from_str(&runtime_catalog_wire().unwrap()).unwrap();
        let capabilities = &catalog["capabilities"];
        let expected = &node_wire_contract().artifact;
        assert_eq!(
            capabilities["capability_ids"],
            serde_json::json!(&expected.capability_ids)
        );
        assert_eq!(
            capabilities["output_ids"],
            serde_json::json!(&expected.output_ids)
        );
        assert_eq!(
            capabilities["operation_ids"],
            serde_json::json!(&expected.operation_ids)
        );
        assert_eq!(
            capabilities["system_adapter_ids"],
            serde_json::json!(&expected.system_adapter_ids)
        );
        assert_eq!(
            capabilities["text_measurement"]["provider_ids"],
            serde_json::json!(&expected.text_measurement_provider_ids)
        );
        assert_eq!(catalog["metadata_ids"], serde_json::json!(&expected.metadata_ids));
        assert_eq!(
            catalog["option_group_ids"],
            serde_json::json!(&expected.option_group_ids)
        );
        assert_eq!(
            catalog["output_contracts"],
            serde_json::to_value(&expected.output_contracts).unwrap()
        );

        let mut drifted = catalog.clone();
        drifted["output_contracts"][0]["media_type"] = serde_json::json!("image/png");
        assert!(validate_runtime_catalog(&drifted).is_err());
    }

    #[test]
    fn transport_identity_embeds_the_complete_wire_contract() {
        for (kind, expected_kind) in [
            (NodeTransportKind::Napi, "napi"),
            (NodeTransportKind::Wasm, "wasm"),
        ] {
            let identity_text = transport_identity_wire(kind).unwrap();
            let identity: serde_json::Value = serde_json::from_str(&identity_text).unwrap();
            assert_eq!(identity["schema_version"], 1);
            assert_eq!(identity["package_id"], "@mermanjs/node");
            assert_eq!(identity["artifact_id"], "merman-node-static-svg");
            assert_eq!(identity["package_version"], env!("CARGO_PKG_VERSION"));
            assert_eq!(identity["transport_kind"], expected_kind);
            assert_eq!(
                identity["capability_descriptor_digest"],
                merman_bindings_core::CAPABILITY_DESCRIPTOR_DIGEST
            );
            assert_eq!(
                identity["wire_contract"],
                serde_json::to_value(node_wire_contract()).unwrap()
            );
        }
    }

    #[test]
    fn bounded_scanner_accepts_exact_limits_and_rejects_plus_one() {
        let base = NodeDocumentLimits {
            max_utf8_bytes: 256,
            max_depth: 2,
            max_members: 2,
            max_tokens: 5,
            max_string_utf8_bytes: 4,
        };
        scan_bounded_json_text(r#"{"a":0,"b":0}"#, "test", base).unwrap();

        let bytes = NodeDocumentLimits {
            max_utf8_bytes: 2,
            max_depth: 1,
            max_members: 1,
            max_tokens: 1,
            max_string_utf8_bytes: 1,
        };
        validate_bounded_json_text("{}", "test", bytes).unwrap();
        assert!(validate_bounded_json_text("{} ", "test", bytes).is_err());

        assert!(
            scan_bounded_json_text(
                r#"{"a":0,"b":0,"c":0}"#,
                "test",
                NodeDocumentLimits {
                    max_members: 2,
                    max_tokens: 7,
                    ..base
                },
            )
            .is_err()
        );
        scan_bounded_json_text("[[0]]", "test", base).unwrap();
        assert!(scan_bounded_json_text("[[[0]]]", "test", base).is_err());
        scan_bounded_json_text(r#""💩""#, "test", base).unwrap();
        assert!(scan_bounded_json_text(r#""💩a""#, "test", base).is_err());
        scan_bounded_json_text(r#""\ud83d\udca9""#, "test", base).unwrap();
        assert!(scan_bounded_json_text(r#""\ud83d""#, "test", base).is_err());
    }

    #[test]
    fn bounded_json_matches_serde_finite_number_semantics() {
        let limits = node_wire_contract().documents.binding_options;
        assert!(
            deserialize_bounded_json::<serde_json::Value>(
                r#"{"value":1e308}"#,
                "finite number",
                limits,
            )
            .is_ok()
        );
        assert!(
            deserialize_bounded_json::<serde_json::Value>(
                r#"{"value":1e309}"#,
                "non-finite number",
                limits,
            )
            .is_err()
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_panic_boundary_returns_the_stable_panic_envelope() {
        let error = super::binding_boundary(|| -> Result<(), merman_bindings_core::BindingError> {
            panic!("synthetic Node transport panic")
        })
        .expect_err("native panic must become a binding error");
        assert_eq!(error.status(), merman_bindings_core::BindingStatus::Panic);
        let envelope: serde_json::Value =
            serde_json::from_str(&error_envelope(&error)).expect("panic envelope");
        assert_eq!(envelope["error"]["code"], 8);
        assert_eq!(envelope["error"]["code_name"], "MERMAN_PANIC");
    }

    #[test]
    fn request_preflight_rejects_field_and_nested_options_overruns() {
        let exact_operation_id = "x".repeat(node_wire_contract().fields.operation_id_utf8_bytes);
        let exact = serde_json::json!({
            "operation_id": exact_operation_id,
            "source": "",
            "uri": null
        });
        parse_operation_request(&serde_json::to_string(&exact).unwrap()).unwrap();

        let plus_one = serde_json::json!({
            "operation_id": "x".repeat(node_wire_contract().fields.operation_id_utf8_bytes + 1),
            "source": "",
            "uri": null
        });
        assert!(parse_operation_request(&serde_json::to_string(&plus_one).unwrap()).is_err());
        assert!(
            parse_operation_request(
                r#"{"operation_id":"svg","source":"x","uri":null,"options_json":"{"}"}"#,
            )
            .is_err()
        );
        assert!(
            parse_operation_request(
                r#"{"operation_id":"svg","source":"x","uri":null,"future":true}"#,
            )
            .is_err()
        );
    }

    #[test]
    fn bounded_deserialization_checks_work_before_materializing_json() {
        let limits = NodeDocumentLimits {
            max_utf8_bytes: 1024,
            max_depth: 8,
            max_members: 2,
            max_tokens: 4,
            max_string_utf8_bytes: 32,
        };
        let dense = r#"{"a":0,"b":0,"c":0}"#;
        assert!(deserialize_bounded_json::<serde_json::Value>(dense, "dense", limits).is_err());
    }

    #[test]
    fn runtime_catalog_metadata_ids_match_the_contract_dispatcher() {
        let catalog: serde_json::Value =
            serde_json::from_str(&runtime_catalog_wire().unwrap()).unwrap();
        let advertised_ids = catalog["metadata_ids"]
            .as_array()
            .expect("Node runtime metadata IDs")
            .iter()
            .map(|id| id.as_str().expect("metadata ID string"))
            .collect::<Vec<_>>();
        let expected_ids = node_artifact_contract()
            .metadata_keys()
            .map(merman_bindings_core::MetadataKey::id)
            .collect::<Vec<_>>();
        assert_eq!(advertised_ids, expected_ids);

        for id in advertised_ids {
            let payload: serde_json::Value =
                serde_json::from_str(&metadata_wire(id).unwrap_or_else(|error| {
                    panic!("advertised Node metadata `{id}` failed: {error:?}")
                }))
                .unwrap_or_else(|error| panic!("Node metadata `{id}` was not JSON: {error}"));
            assert!(!payload.is_null(), "Node metadata `{id}` returned null");
        }

        for key in merman_bindings_core::MetadataKey::ALL {
            if !expected_ids.contains(&key.id()) {
                let error = match metadata_wire(key.id()) {
                    Ok(_) => panic!("unadvertised metadata `{}` succeeded", key.id()),
                    Err(error) => error,
                };
                assert_eq!(
                    error.status(),
                    merman_bindings_core::BindingStatus::UnsupportedOperation
                );
            }
        }
    }

    #[test]
    fn static_svg_transport_preserves_missing_and_unknown_operation_errors() {
        let native = match create_engine(r#"{"runtime_policy":"native"}"#) {
            Ok(_) => panic!("Node static-SVG transport accepted native runtime policy"),
            Err(error) => error,
        };
        assert_eq!(
            native.status(),
            merman_bindings_core::BindingStatus::OptionsJsonError
        );
        assert_eq!(
            native.kind(),
            merman_bindings_core::BindingErrorKind::Generic
        );
        assert_eq!(native.capability_id(), None);
        assert!(native.message().contains("not exposed by target"));

        let engine = create_engine("").unwrap();
        let missing: serde_json::Value = serde_json::from_str(&execute_wire(
            &engine,
            r#"{"operation_id":"png","source":"flowchart TD\nA-->B","uri":null}"#,
        ))
        .expect("Node error envelope");
        assert_eq!(
            missing["error"]["code_name"],
            "MERMAN_UNSUPPORTED_OPERATION"
        );
        assert_eq!(missing["error"]["kind"], "missing-capability");
        assert_eq!(missing["error"]["capability_id"], "png");
        assert!(
            missing["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("not exposed by target"))
        );

        let unknown: serde_json::Value = serde_json::from_str(&execute_wire(
            &engine,
            r#"{"operation_id":"bitmap","source":"flowchart TD\nA-->B","uri":null}"#,
        ))
        .expect("Node error envelope");
        assert_eq!(unknown["error"]["kind"], "unknown-operation");
        assert!(unknown["error"]["capability_id"].is_null());
    }

    #[test]
    fn error_wire_preserves_structured_resource_details() {
        let error = merman_bindings_core::BindingError::resource_limit(
            "embedded_image_decode",
            "max_embedded_image_bytes",
            5,
            4,
            "constrained",
            "embedded image is too large",
        );
        let payload: serde_json::Value =
            serde_json::from_str(&error_envelope(&error)).expect("Node error envelope");

        assert_eq!(
            payload["error"]["details"]["resource"]["limit_id"],
            "max_embedded_image_bytes"
        );
        assert_eq!(
            payload["error"]["details"]["resource"]["profile"],
            "constrained"
        );
    }

    #[test]
    fn disposed_error_is_a_stable_bounded_error_envelope() {
        let first = error_envelope(&super::disposed_error());
        let second = error_envelope(&super::disposed_error());
        assert_eq!(first, second);
        let payload: serde_json::Value = serde_json::from_str(&first).unwrap();
        assert_eq!(payload["ok"], false);
        assert_eq!(payload["error"]["code_name"], "MERMAN_INVALID_ARGUMENT");
        assert_eq!(
            payload["error"]["message"],
            "Node transport engine has been disposed"
        );
    }
}

#[cfg(not(feature = "svg"))]
compile_error!("the Node candidates must be built with the direct static SVG feature set");

#[cfg(not(feature = "layout-cytoscape"))]
compile_error!("the Node candidates require the static layout-cytoscape capability recipe");

#[cfg(not(feature = "layout-dagre"))]
compile_error!("the Node candidates require the static layout-dagre capability recipe");

#[cfg(not(feature = "layout-elk"))]
compile_error!("the Node candidates require the static layout-elk capability recipe");

use std::borrow::Cow;
use std::sync::{LazyLock, OnceLock};
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use merman_bindings_core::transport_boundary::catch_transport_unwind;
use merman_bindings_core::{
    ArtifactContractSpec, BINDING_OPERATION_SCHEMA_VERSION, BindingEngine, BindingEngineServices,
    BindingError, BindingErrorBody, BindingErrorKind, BindingOperationRequest,
    BindingPayloadSchemaKey, BindingStatus, BindingTransportKey, CAPABILITY_DESCRIPTOR_DIGEST,
    CapabilityKey, ConstructorServiceKey, DecodedOperationOptions, NODE_TRANSPORT_API_VERSION,
    OperationControl, OperationKey, OperationPhase, RUNTIME_CATALOG_MAX_SAFE_INTEGER,
    RUNTIME_CATALOG_SCHEMA_VERSION, RuntimePolicyExposure, TargetKey, ValidatedArtifactContract,
    build_icon_registry_from_json,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

const NODE_WIRE_CONTRACT_JSON: &str =
    include_str!("../../../platforms/node/src/generated/node-wire-contract.json");
const NODE_BINDING_RESULT_PAYLOAD_VERSION: u32 = BindingPayloadSchemaKey::BindingResult.version();
const OPERATION_OPTIONS_JSON_LABEL: &str = "operation request options_json";
const NODE_TARGET: TargetKey = if cfg!(target_arch = "wasm32") {
    TargetKey::Web
} else {
    TargetKey::Native
};
const NODE_OPERATIONS: &[OperationKey] = &[
    #[cfg(feature = "svg")]
    OperationKey::ExportThemePreset,
    #[cfg(feature = "svg")]
    OperationKey::LayoutJson,
    #[cfg(feature = "svg")]
    OperationKey::ResolveTheme,
    OperationKey::SemanticJson,
    #[cfg(feature = "svg")]
    OperationKey::Svg,
    #[cfg(feature = "svg")]
    OperationKey::SvgPlanJson,
];
const NODE_SUPPLEMENTAL_CAPABILITIES: &[CapabilityKey] = &[
    #[cfg(feature = "layout-cytoscape")]
    CapabilityKey::LayoutCytoscape,
    #[cfg(feature = "layout-dagre")]
    CapabilityKey::LayoutDagre,
    #[cfg(feature = "layout-elk")]
    CapabilityKey::LayoutElk,
    #[cfg(feature = "layout-shoal")]
    CapabilityKey::LayoutShoal,
    #[cfg(feature = "math")]
    CapabilityKey::Math,
];

// Keep feature selection in the transport owner. Dependency features may be unified by Cargo.
static ARTIFACT_CONTRACT: ValidatedArtifactContract =
    ArtifactContractSpec::new(NODE_TARGET, BindingTransportKey::Node)
        .with_operations(NODE_OPERATIONS)
        .with_supplemental_capabilities(NODE_SUPPLEMENTAL_CAPABILITIES)
        .with_all_available_metadata()
        .with_constructor_services(&[ConstructorServiceKey::IconRegistry])
        .with_runtime_policy_exposure(RuntimePolicyExposure::DeterministicOnly)
        .materialize();
static NODE_WIRE_CONTRACT: LazyLock<NodeWireContract> = LazyLock::new(|| {
    let contract = serde_json::from_str::<NodeWireContract>(NODE_WIRE_CONTRACT_JSON)
        .expect("the embedded Node wire contract must be valid JSON");
    #[cfg(feature = "layout-shoal")]
    let contract = {
        let mut contract = contract;
        let ids = &mut contract.artifact.capability_ids;
        ids.push(CapabilityKey::LayoutShoal.id().to_owned());
        ids.sort_unstable();
        contract
    };
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
                return Err(format!(
                    "{label} contains an ID beyond the Node field limit"
                ));
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
    operation_control: Option<NodeOperationControlRequest>,
    // Omission preserves the result contract used by executeOperation. String helpers request
    // data before execution so metadata is neither projected nor serialized by the producer.
    #[serde(default = "include_metadata_by_default")]
    include_metadata: bool,
}

impl NodeOperationRequest {
    fn binding_request(&self) -> BindingOperationRequest<'_> {
        BindingOperationRequest::new(&self.operation_id, self.source.as_bytes())
            .with_optional_uri(self.uri.as_deref().map(str::as_bytes))
    }
}

fn include_metadata_by_default() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NodeOperationControlRequest {
    timeout_ms: u32,
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
struct SuccessEnvelope<'a> {
    version: u32,
    ok: bool,
    result: SuccessResult<'a>,
}

#[derive(Debug, Serialize)]
struct SuccessResult<'a> {
    operation_id: &'static str,
    media_type: &'static str,
    data: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata_json: Option<&'a str>,
}

impl<'a> SuccessEnvelope<'a> {
    fn new(operation: OperationKey, data: &'a str, metadata_json: Option<&'a str>) -> Self {
        Self {
            version: NODE_BINDING_RESULT_PAYLOAD_VERSION,
            ok: true,
            result: SuccessResult {
                operation_id: operation.id(),
                media_type: operation.spec().media_type,
                data,
                metadata_json,
            },
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope<'a> {
    version: u32,
    ok: bool,
    error: BindingErrorBody<'a, Value>,
}

#[cfg(test)]
pub(crate) fn create_engine(options_json: &str) -> Result<BindingEngine, BindingError> {
    create_engine_with_icon_packs(options_json, None)
}

pub(crate) fn create_engine_with_icon_packs(
    options_json: &str,
    icon_packs_json: Option<&str>,
) -> Result<BindingEngine, BindingError> {
    binding_boundary(|| {
        validate_binding_options(options_json)?;
        let mut services = BindingEngineServices::new();
        if let Some(packs) = icon_packs_json {
            services =
                services.with_icon_registry(build_icon_registry_from_json(packs.as_bytes())?);
        }
        node_artifact_contract().create_engine_with_services(options_json.as_bytes(), services)
    })
}

pub(crate) fn transport_identity_wire(kind: NodeTransportKind) -> Result<String, BindingError> {
    binding_boundary(|| transport_identity_wire_inner(kind))
}

fn transport_identity_wire_inner(kind: NodeTransportKind) -> Result<String, BindingError> {
    // Identity is an admission claim, so refuse to emit it if the compiled Rust artifact has
    // drifted from the descriptor embedded below.
    cached_runtime_catalog()?;
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
    binding_boundary(|| cached_runtime_catalog().map(str::to_owned))
}

fn cached_runtime_catalog() -> Result<&'static str, BindingError> {
    // Identity admission and engines share the same compiled artifact and wire contract.
    static CATALOG: OnceLock<Result<String, BindingError>> = OnceLock::new();
    CATALOG
        .get_or_init(runtime_catalog_wire_inner)
        .as_deref()
        .map_err(Clone::clone)
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

pub(crate) fn metadata_wire_for_engine(
    engine: &BindingEngine,
    id: &str,
) -> Result<String, BindingError> {
    binding_boundary(|| metadata_wire_inner_for_engine(engine, id))
}

fn metadata_wire_inner_for_engine(
    engine: &BindingEngine,
    id: &str,
) -> Result<String, BindingError> {
    let contract = node_wire_contract();
    if id.is_empty() {
        return Err(BindingError::invalid_argument(
            "Node metadata id must be non-empty",
        ));
    }
    ensure_field(id, "metadata id", contract.fields.metadata_id_utf8_bytes)
        .map_err(caller_argument_error)?;
    let bytes = engine.metadata_json(id)?;
    let text = String::from_utf8(bytes)
        .map_err(|error| BindingError::internal(format!("Node metadata was not UTF-8: {error}")))?;
    deserialize_bounded_json::<Value>(&text, "metadata", contract.documents.metadata)
        .map_err(producer_error)?;
    Ok(text)
}

#[cfg(test)]
pub(crate) fn execute_wire(engine: &BindingEngine, request_json: &str) -> String {
    execute_wire_with_control(engine, request_json, admitted_operation_control(None))
}

#[cfg(test)]
pub(crate) fn execute_wire_with_control(
    engine: &BindingEngine,
    request_json: &str,
    control: OperationControl,
) -> String {
    execute_wire_with_admitted_control(engine, request_json, control, None)
}

pub(crate) fn execute_wire_with_admitted_control(
    engine: &BindingEngine,
    request_json: &str,
    control: OperationControl,
    admitted_timeout_ms: Option<u32>,
) -> String {
    match binding_boundary(|| {
        execute_wire_inner(engine, request_json, control, admitted_timeout_ms)
    }) {
        Ok(response) => response,
        Err(error) => error_envelope(&error),
    }
}

pub(crate) fn admitted_operation_control(timeout_ms: Option<u32>) -> OperationControl {
    timeout_ms.map_or_else(OperationControl::new, |timeout_ms| {
        OperationControl::new().with_deadline(Duration::from_millis(u64::from(timeout_ms)))
    })
}

fn execute_wire_inner(
    engine: &BindingEngine,
    request_json: &str,
    control: OperationControl,
    admitted_timeout_ms: Option<u32>,
) -> Result<String, BindingError> {
    let request = parse_operation_request_framing(request_json)?;
    let operation_request = request.binding_request();
    let operation = operation_request.resolve_operation()?;
    validate_wire_control(&request, &control, admitted_timeout_ms)?;
    let options = decode_operation_request_options(request.options_json.as_deref())?;
    control
        .checkpoint_at(OperationPhase::Admission)
        .map_err(BindingError::cancelled)?;
    let operation_request = operation_request.with_control(control);
    if request.include_metadata {
        success_envelope(engine.execute_with_decoded_options(operation_request, options)?)
    } else {
        success_data_envelope(
            operation.key(),
            engine.execute_data_with_decoded_options(operation_request, options)?,
            None,
        )
    }
}

#[cfg(feature = "transport-napi")]
#[derive(Clone, Copy)]
pub(crate) enum DirectSvgOptions<'a> {
    Json(Option<&'a str>),
    DiagramId(&'a str),
}

#[cfg(feature = "transport-napi")]
impl<'a> DirectSvgOptions<'a> {
    fn json(self) -> Option<&'a str> {
        match self {
            Self::Json(json) => json,
            Self::DiagramId(_) => None,
        }
    }
}

#[cfg(feature = "transport-napi")]
pub(crate) fn execute_svg_with_admitted_control(
    engine: &BindingEngine,
    source: &str,
    options: DirectSvgOptions<'_>,
    control: OperationControl,
    admitted_timeout_ms: Option<u32>,
) -> Result<String, BindingError> {
    binding_boundary(|| {
        let fallback;
        let options = match options {
            DirectSvgOptions::DiagramId(id)
                if !direct_svg_id_fits(source, id, admitted_timeout_ms) =>
            {
                fallback =
                    prepare_direct_svg_id_fallback(source, id, &control, admitted_timeout_ms)?;
                DirectSvgOptions::Json(Some(fallback.as_str()))
            }
            options => {
                validate_direct_svg_framing(source, options.json(), admitted_timeout_ms)?;
                control
                    .checkpoint_at(OperationPhase::Admission)
                    .map_err(BindingError::cancelled)?;
                options
            }
        };
        let decoded = decode_operation_request_options(options.json())?;
        control
            .checkpoint_at(OperationPhase::Admission)
            .map_err(BindingError::cancelled)?;
        let data = match options {
            DirectSvgOptions::Json(_) => {
                let request =
                    BindingOperationRequest::new(OperationKey::Svg.id(), source.as_bytes())
                        .with_control(control);
                engine.execute_data_with_decoded_options(request, decoded)?
            }
            DirectSvgOptions::DiagramId(id) => {
                engine.execute_svg_data_with_diagram_id(source.as_bytes(), id, control)?
            }
        };
        direct_svg_response(data)
    })
}

#[cfg(feature = "transport-napi")]
pub(crate) fn direct_svg_field_limits() -> (usize, usize) {
    let fields = node_wire_contract().fields;
    (fields.source_utf8_bytes, fields.options_json_utf8_bytes)
}

#[cfg(feature = "transport-napi")]
fn direct_svg_frame_bytes(has_options: bool, timeout_ms: Option<u32>) -> usize {
    const FRAME_BYTES: usize =
        r#"{"operation_id":"svg","source":"","uri":null,"include_metadata":false}"#.len();
    const OPTIONS_FRAME_BYTES: usize = r#","options_json":"""#.len();
    const TIMEOUT_FRAME_BYTES: usize = r#","operation_control":{"timeout_ms":}"#.len();

    FRAME_BYTES
        + usize::from(has_options) * OPTIONS_FRAME_BYTES
        + timeout_ms.map_or(0, |timeout| {
            TIMEOUT_FRAME_BYTES + timeout.checked_ilog10().unwrap_or(0) as usize + 1
        })
}

#[cfg(feature = "transport-napi")]
fn direct_svg_id_fits(source: &str, id: &str, timeout_ms: Option<u32>) -> bool {
    const ID_OPTIONS_BYTES: usize = br#"{"svg":{"diagram_id":""}}"#.len();
    let contract = node_wire_contract();
    // The virtual option document expands by at most twice when escaped as a request field.
    // Its structure and field bytes are otherwise bounded by this compact JSON shape.
    let max_options_bytes = ID_OPTIONS_BYTES.saturating_add(id.len().saturating_mul(6));
    let max_request_bytes = direct_svg_frame_bytes(true, timeout_ms)
        .saturating_add(source.len().saturating_mul(6))
        .saturating_add(max_options_bytes.saturating_mul(2));
    max_options_bytes <= contract.fields.options_json_utf8_bytes
        && max_request_bytes <= contract.documents.request.max_utf8_bytes
}

#[cfg(feature = "transport-napi")]
fn prepare_direct_svg_id_fallback(
    source: &str,
    id: &str,
    control: &OperationControl,
    timeout_ms: Option<u32>,
) -> Result<String, BindingError> {
    #[derive(Serialize)]
    struct Svg<'a> {
        diagram_id: &'a str,
    }
    #[derive(Serialize)]
    struct Options<'a> {
        svg: Svg<'a>,
    }

    let contract = node_wire_contract();
    validate_direct_svg_source(source, contract.fields.source_utf8_bytes)?;
    let options = Options {
        svg: Svg { diagram_id: id },
    };
    let mut count = CompactJsonByteCount::default();
    serde_json::to_writer(&mut count, &options).map_err(direct_svg_size_error)?;
    let frame_bytes = direct_svg_frame_bytes(true, timeout_ms)
        .saturating_add(count.json_bytes)
        .saturating_add(count.outer_escape_bytes);
    let max = contract.documents.request.max_utf8_bytes;
    if frame_bytes.saturating_add(source.len().saturating_mul(6)) > max {
        let mut source_bytes = JsonByteCount(0);
        serde_json::to_writer(&mut source_bytes, source).map_err(direct_svg_size_error)?;
        validate_document_bytes(
            frame_bytes.saturating_add(source_bytes.0 - 2),
            "operation request",
            max,
        )
        .map_err(caller_options_error)?;
    }
    control
        .checkpoint_at(OperationPhase::Admission)
        .map_err(BindingError::cancelled)?;
    ensure_field_bytes(
        count.json_bytes,
        OPERATION_OPTIONS_JSON_LABEL,
        contract.fields.options_json_utf8_bytes,
    )
    .map_err(caller_options_error)?;
    serde_json::to_string(&options).map_err(|error| {
        BindingError::internal(format!("failed to serialize Node SVG options: {error}"))
    })
}

#[cfg(feature = "transport-napi")]
fn validate_direct_svg_source(source: &str, max_utf8_bytes: usize) -> Result<(), BindingError> {
    ensure_field(source, "operation request source", max_utf8_bytes).map_err(caller_options_error)
}

#[cfg(feature = "transport-napi")]
fn direct_svg_size_error(error: serde_json::Error) -> BindingError {
    BindingError::internal(format!("failed to size Node SVG request: {error}"))
}

#[cfg(feature = "transport-napi")]
fn validate_direct_svg_framing(
    source: &str,
    options_json: Option<&str>,
    timeout_ms: Option<u32>,
) -> Result<(), BindingError> {
    let contract = node_wire_contract();
    validate_direct_svg_source(source, contract.fields.source_utf8_bytes)?;
    let frame_bytes = direct_svg_frame_bytes(options_json.is_some(), timeout_ms);
    let text_bytes = source
        .len()
        .saturating_add(options_json.map_or(0, str::len));
    let max = contract.documents.request.max_utf8_bytes;
    // Only the encoded string contents can expand. All fixed fields fit the structural limits.
    if frame_bytes.saturating_add(text_bytes.saturating_mul(6)) > max {
        let mut count = JsonByteCount(frame_bytes - 2 - options_json.map_or(0, |_| 2));
        serde_json::to_writer(&mut count, source)
            .and_then(|()| {
                options_json.map_or(Ok(()), |json| serde_json::to_writer(&mut count, json))
            })
            .map_err(direct_svg_size_error)?;
        validate_document_bytes(count.0, "operation request", max).map_err(caller_options_error)?;
    }
    Ok(())
}

fn validate_wire_control(
    request: &NodeOperationRequest,
    control: &OperationControl,
    admitted_timeout_ms: Option<u32>,
) -> Result<(), BindingError> {
    control
        .checkpoint_at(OperationPhase::Admission)
        .map_err(BindingError::cancelled)?;
    let request_timeout_ms = request
        .operation_control
        .as_ref()
        .map(|operation_control| operation_control.timeout_ms);
    match (request_timeout_ms, admitted_timeout_ms) {
        (Some(requested), Some(admitted)) if requested != admitted => {
            return Err(BindingError::invalid_argument(
                "Node operation-control timeout does not match transport admission",
            ));
        }
        (None, Some(_)) => {
            return Err(BindingError::invalid_argument(
                "Node transport admitted a timeout missing from the operation request",
            ));
        }
        (Some(_), None) => {
            return Err(BindingError::invalid_argument(
                "Node operation request timeout is missing transport admission",
            ));
        }
        (Some(_), Some(_)) | (None, None) => {}
    }
    Ok(())
}

pub(crate) fn error_envelope(error: &BindingError) -> String {
    match transport_unwind_boundary(|| try_error_envelope(error)) {
        Ok(Ok(envelope)) => envelope,
        Ok(Err(_)) => fallback_error_envelope(),
        Err(()) => fallback_panic_envelope(),
    }
}

#[cfg(test)]
fn parse_operation_request(request_json: &str) -> Result<NodeOperationRequest, BindingError> {
    let request = parse_operation_request_framing(request_json)?;
    decode_operation_request_options(request.options_json.as_deref())?;
    Ok(request)
}

fn parse_operation_request_framing(
    request_json: &str,
) -> Result<NodeOperationRequest, BindingError> {
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
    Ok(request)
}

fn decode_operation_request_options(
    options_json: Option<&str>,
) -> Result<Option<DecodedOperationOptions<'_>>, BindingError> {
    let Some(options_json) = options_json else {
        return Ok(None);
    };
    let contract = node_wire_contract();
    let label = OPERATION_OPTIONS_JSON_LABEL;
    ensure_field(options_json, label, contract.fields.options_json_utf8_bytes)
        .map_err(caller_options_error)?;
    validate_bounded_json_text(options_json, label, contract.documents.binding_options)
        .map_err(caller_options_error)?;
    DecodedOperationOptions::decode(options_json, |error| {
        caller_options_error(format!("{label} is not valid JSON: {error}"))
    })
    .map(Some)
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

    success_data_envelope(operation.key(), data, Some(&metadata_json))
}

fn success_data_envelope(
    operation: OperationKey,
    data: Vec<u8>,
    metadata_json: Option<&str>,
) -> Result<String, BindingError> {
    let data = response_data_text(operation, data)?;
    serialize_bounded_json(
        &SuccessEnvelope::new(operation, &data, metadata_json),
        "response",
        node_wire_contract().documents.response,
    )
    .map_err(producer_error)
}

fn response_data_text(operation: OperationKey, data: Vec<u8>) -> Result<String, BindingError> {
    let data = String::from_utf8(data).map_err(|error| {
        let operation_id = operation.id();
        BindingError::internal(format!(
            "Node static-SVG candidate received non-UTF-8 output for `{operation_id}`: {error}"
        ))
    })?;
    ensure_field(
        &data,
        "response data",
        node_wire_contract().fields.data_utf8_bytes,
    )
    .map_err(producer_error)?;
    Ok(data)
}

#[cfg(feature = "transport-napi")]
fn direct_svg_response(data: Vec<u8>) -> Result<String, BindingError> {
    let data = response_data_text(OperationKey::Svg, data)?;
    static EMPTY_ENVELOPE: LazyLock<(usize, Result<(), String>)> = LazyLock::new(|| {
        let envelope = serde_json::to_string(&SuccessEnvelope::new(OperationKey::Svg, "", None))
            .expect("the fixed SVG response envelope is serializable");
        let shape = scan_bounded_json_text(
            &envelope,
            "response",
            node_wire_contract().documents.response,
        );
        (envelope.len(), shape)
    });
    let (envelope_bytes, shape) = &*EMPTY_ENVELOPE;
    let limits = node_wire_contract().documents.response;
    // serde_json emits at most six bytes per UTF-8 byte (an ASCII control becomes \u00XX).
    // The raw data field already fits the decoded-string ceiling, and its escaped content
    // cannot change the fixed envelope shape.
    const MAX_JSON_BYTES_PER_INPUT_BYTE: usize = 6;
    if envelope_bytes.saturating_add(data.len().saturating_mul(MAX_JSON_BYTES_PER_INPUT_BYTE))
        > limits.max_utf8_bytes
    {
        let mut count = JsonByteCount(0);
        serde_json::to_writer(
            &mut count,
            &SuccessEnvelope::new(OperationKey::Svg, &data, None),
        )
        .map_err(|error| producer_error(format!("failed to serialize Node response: {error}")))?;
        validate_document_bytes(count.0, "response", limits.max_utf8_bytes)
            .map_err(producer_error)?;
    }
    shape
        .as_ref()
        .map_err(|reason| producer_error(reason.clone()))?;
    Ok(data)
}

#[cfg(feature = "transport-napi")]
struct JsonByteCount(usize);

#[cfg(feature = "transport-napi")]
impl std::io::Write for JsonByteCount {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 += bytes.len();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(feature = "transport-napi")]
#[derive(Default)]
struct CompactJsonByteCount {
    json_bytes: usize,
    outer_escape_bytes: usize,
}

#[cfg(feature = "transport-napi")]
impl std::io::Write for CompactJsonByteCount {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let escapes = bytes
            .iter()
            .filter(|&&byte| matches!(byte, b'"' | b'\\'))
            .count();
        self.json_bytes = self.json_bytes.saturating_add(bytes.len());
        self.outer_escape_bytes = self.outer_escape_bytes.saturating_add(escapes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
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
    let diagnostic = error.diagnostic_details();
    if diagnostic
        .and_then(|details| details.span)
        .is_some_and(|span| {
            span.start > RUNTIME_CATALOG_MAX_SAFE_INTEGER
                || span.end > RUNTIME_CATALOG_MAX_SAFE_INTEGER
        })
    {
        return Err("error diagnostic span exceeds the JSON-safe integer range".to_owned());
    }
    let message = bounded_text(error.message(), fields.error_message_utf8_bytes);
    let error = BindingErrorBody::with_js_details(error, &message)
        .map_err(|error| format!("failed to encode binding error details: {error}"))?;
    let envelope = ErrorEnvelope {
        version: NODE_BINDING_RESULT_PAYLOAD_VERSION,
        ok: false,
        error,
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
    catch_transport_unwind(operation).ok_or(())
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
    ensure_output_contract_array(catalog.get("output_contracts"), &expected.output_contracts)?;
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
    validate_document_bytes(source.len(), label, limits.max_utf8_bytes)?;
    scan_bounded_json_text(source, label, limits)
}

fn validate_document_bytes(bytes: usize, label: &str, max_utf8_bytes: usize) -> Result<(), String> {
    if bytes > max_utf8_bytes {
        return Err(format!(
            "{label} exceeds the {max_utf8_bytes}-byte wire limit"
        ));
    }
    Ok(())
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
    ensure_field_bytes(value.len(), label, max_utf8_bytes)
}

pub(crate) fn ensure_field_bytes(
    bytes: usize,
    label: &str,
    max_utf8_bytes: usize,
) -> Result<(), String> {
    if bytes > max_utf8_bytes {
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

pub(crate) fn caller_options_error(message: String) -> BindingError {
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
    #[cfg(feature = "transport-napi")]
    use super::{
        DirectSvgOptions, SuccessEnvelope, direct_svg_id_fits, direct_svg_response,
        execute_svg_with_admitted_control, parse_operation_request_framing,
        prepare_direct_svg_id_fallback, success_data_envelope, validate_direct_svg_framing,
    };
    use super::{
        NodeDocumentLimits, NodeTransportKind, admitted_operation_control, create_engine,
        create_engine_with_icon_packs, deserialize_bounded_json, error_envelope, execute_wire,
        execute_wire_with_admitted_control, execute_wire_with_control, metadata_wire_for_engine,
        node_artifact_contract, node_wire_contract, parse_operation_request, runtime_catalog_wire,
        scan_bounded_json_text, transport_identity_wire, validate_bounded_json_text,
        validate_runtime_catalog,
    };

    #[test]
    fn runtime_catalog_reports_exact_static_svg_artifact() {
        let catalog: serde_json::Value =
            serde_json::from_str(&runtime_catalog_wire().unwrap()).unwrap();
        assert_eq!(
            catalog["transport_api_version"],
            node_wire_contract().transport_api_version
        );
        let capabilities = &catalog["capabilities"];
        let expected = &node_wire_contract().artifact;
        assert_eq!(
            capabilities["capability_ids"],
            serde_json::json!(&expected.capability_ids)
        );
        assert_eq!(
            expected.capability_ids.iter().any(|id| id == "math"),
            cfg!(feature = "math")
        );
        assert_eq!(
            expected
                .capability_ids
                .iter()
                .any(|id| id == "layout-shoal"),
            cfg!(feature = "layout-shoal")
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
        assert_eq!(
            catalog["metadata_ids"],
            serde_json::json!(&expected.metadata_ids)
        );
        assert_eq!(
            catalog["option_group_ids"],
            serde_json::json!(&expected.option_group_ids)
        );
        assert_eq!(
            catalog["constructor_service_ids"],
            serde_json::json!(["icon-registry"])
        );
        assert_eq!(expected.constructor_service_ids, ["icon-registry"]);
        assert!(
            catalog["constructor_service_contracts"][0]["resource_limits"]
                .as_array()
                .unwrap()
                .len()
                > 1
        );
        assert_eq!(
            catalog["output_contracts"],
            serde_json::to_value(&expected.output_contracts).unwrap()
        );

        let mut drifted = catalog.clone();
        drifted["output_contracts"][0]["media_type"] = serde_json::json!("image/png");
        assert!(validate_runtime_catalog(&drifted).is_err());
    }

    #[cfg(feature = "math")]
    #[test]
    fn math_feature_renders_formula_geometry_through_the_owned_transport_contract() {
        let engine = create_engine("{}").unwrap();
        let request = serde_json::json!({
            "operation_id": "svg",
            "source": "flowchart LR\nA[\"$$x^2$$\"]",
            "options_json": "{\"site_config\":{\"htmlLabels\":true}}"
        });
        let response: serde_json::Value =
            serde_json::from_str(&execute_wire(&engine, &request.to_string())).unwrap();
        assert_eq!(response["ok"], true);
        let svg = response["result"]["data"].as_str().unwrap();
        assert!(svg.matches("<svg").count() > 1);
        assert!(svg.contains("<path"));
        assert!(!svg.contains("$$x^2$$"));
    }

    #[test]
    fn loaded_icon_packs_render_with_request_overlays_and_isolated_engines() {
        let packs = serde_json::json!([{
            "json": r#"{"prefix":"original","icons":{"rocket":{"body":"<path data-icon=\"node-registry\" d=\"M0 0H16V16H0z\"/>"}}}"#,
            "registrationName": "fleet",
        }])
        .to_string();
        let registered = create_engine_with_icon_packs("", Some(&packs)).unwrap();
        drop(packs);
        let plain = create_engine("").unwrap();
        let empty = create_engine_with_icon_packs("", Some("[]")).unwrap();
        let request = serde_json::json!({
            "operation_id": "svg",
            "source": "flowchart TD\nA@{ icon: \"fleet:rocket\", label: \"A\" }",
            "options_json": r#"{"svg":{"diagram_id":"node-icon-overlay"}}"#,
        })
        .to_string();
        let render = |engine: &merman_bindings_core::BindingEngine| {
            let result: serde_json::Value =
                serde_json::from_str(&execute_wire(engine, &request)).unwrap();
            assert_eq!(result["ok"], true, "{result}");
            result["result"]["data"].as_str().unwrap().to_owned()
        };
        let plain_svg = render(&plain);
        assert_eq!(render(&empty), plain_svg);
        assert!(!plain_svg.contains("node-registry"));
        assert!(render(&registered).contains("node-registry"));
        drop(registered);
        assert_eq!(render(&plain), plain_svg);
    }

    #[test]
    fn icon_constructor_preserves_typed_content_and_resource_errors() {
        for (json, name, code, kind) in [
            (
                r#"{"prefix":"test","icons":{"bad":{"body":"<path>"}}}"#,
                "fleet".to_owned(),
                "MERMAN_INVALID_ARGUMENT",
                "invalid_xml",
            ),
            (
                r#"{"icons":{}}"#,
                "a".repeat(65),
                "MERMAN_RESOURCE_LIMIT_EXCEEDED",
                "resource_limit_exceeded",
            ),
        ] {
            let packs = serde_json::json!([{"json": json, "registrationName": name}]).to_string();
            let error = create_engine_with_icon_packs("", Some(&packs))
                .err()
                .unwrap();
            let envelope: serde_json::Value =
                serde_json::from_str(&error_envelope(&error)).unwrap();
            assert_eq!(envelope["error"]["code_name"], code);
            assert_eq!(
                envelope["error"]["details"]["icon_registry"]["kind_id"],
                kind
            );
            assert_eq!(
                envelope["error"]["details"]["icon_registry"]["pack_index"],
                0
            );
            if code == "MERMAN_RESOURCE_LIMIT_EXCEEDED" {
                assert_eq!(
                    envelope["error"]["details"]["resource"]["profile"],
                    "constructor-fixed"
                );
                assert_eq!(
                    envelope["error"]["details"]["resource"]["limit_id"],
                    "max_icon_registry_prefix_bytes"
                );
            }
        }
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
                identity["transport_api_version"],
                node_wire_contract().transport_api_version
            );
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

    #[cfg(feature = "transport-napi")]
    #[test]
    fn direct_svg_input_matches_the_escaped_wire_boundary() {
        let options = r#"{"svg":{"diagram_id":"unicode-🧪"}}"#;
        let prefix = "\"\\\t\n\r\u{8}\u{c}\u{1f}é🧪\u{2028}\u{2029}/";
        let mut request = serde_json::json!({
            "operation_id": "svg",
            "source": prefix,
            "uri": null,
            "include_metadata": false,
            "options_json": options,
            "operation_control": {"timeout_ms": u32::MAX},
        });
        let max = node_wire_contract().documents.request.max_utf8_bytes;
        let remaining = max - request.to_string().len();
        let mut source = format!(
            "{prefix}{}{}",
            "\0".repeat(remaining / 6),
            "x".repeat(remaining % 6)
        );
        request["source"] = serde_json::json!(source);
        let exact = request.to_string();
        assert_eq!(exact.len(), max);
        assert!(parse_operation_request_framing(&exact).is_ok());
        assert!(validate_direct_svg_framing(&source, Some(options), Some(u32::MAX)).is_ok());

        source.push('x');
        request["source"] = serde_json::json!(source);
        let legacy = parse_operation_request_framing(&request.to_string()).unwrap_err();
        let direct =
            validate_direct_svg_framing(&source, Some(options), Some(u32::MAX)).unwrap_err();
        assert_eq!(direct.status(), legacy.status());
        assert_eq!(direct.message(), legacy.message());
        assert!(direct.message().contains("wire limit"));
    }

    #[cfg(feature = "transport-napi")]
    #[test]
    fn direct_svg_response_matches_escaped_envelope_and_field_limits() {
        use merman_bindings_core::OperationKey;

        let contract = node_wire_contract();
        let operation = OperationKey::Svg;
        let mut escaped = String::from("\"\\\t\n\r\u{8}\u{c}\u{1f}é🧪\u{2028}\u{2029}/");
        let initial_wire =
            serde_json::to_string(&SuccessEnvelope::new(operation, &escaped, None)).unwrap();
        assert_eq!(
            direct_svg_response(escaped.as_bytes().to_vec()).unwrap(),
            escaped
        );
        let max_wire_bytes = contract.documents.response.max_utf8_bytes;
        let remaining = max_wire_bytes - initial_wire.len();
        escaped.push_str(&"\0".repeat(remaining / 6));
        escaped.push_str(&"x".repeat(remaining % 6));
        assert_eq!(
            direct_svg_response(escaped.as_bytes().to_vec()).unwrap(),
            escaped
        );
        assert_eq!(
            success_data_envelope(operation, escaped.as_bytes().to_vec(), None)
                .unwrap()
                .len(),
            max_wire_bytes
        );

        let max_field_bytes = contract.fields.data_utf8_bytes;
        let exact_field = vec![b'x'; max_field_bytes];
        assert_eq!(
            direct_svg_response(exact_field).unwrap().len(),
            max_field_bytes
        );

        escaped.push('x');
        for (data, expected) in [
            (escaped.into_bytes(), "wire limit"),
            // This exceeds both ceilings. The decoded field keeps priority.
            (vec![0; max_field_bytes + 1], "field limit"),
            (vec![0xff], "non-UTF-8"),
        ] {
            let direct = direct_svg_response(data.clone()).unwrap_err();
            let legacy = success_data_envelope(operation, data, None).unwrap_err();
            assert_eq!(direct.status(), legacy.status());
            assert_eq!(direct.message(), legacy.message());
            assert!(direct.message().contains(expected), "{direct:?}");
        }
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
        let error =
            super::binding_boundary(|| -> Result<(), merman_bindings_core::BindingError> {
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
                r#"{"operation_id":"svg","source":"x","uri":null,"operation_control":{"timeout_ms":-1}}"#,
            )
            .is_err()
        );
        assert!(
            parse_operation_request(
                r#"{"operation_id":"svg","source":"x","uri":null,"operation_control":{"timeout_ms":1,"future":true}}"#,
            )
            .is_err()
        );
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

        let engine = create_engine("").expect("Node engine");
        for id in advertised_ids {
            let payload: serde_json::Value =
                serde_json::from_str(&metadata_wire_for_engine(&engine, id).unwrap_or_else(
                    |error| panic!("advertised Node metadata `{id}` failed: {error:?}"),
                ))
                .unwrap_or_else(|error| panic!("Node metadata `{id}` was not JSON: {error}"));
            assert!(!payload.is_null(), "Node metadata `{id}` returned null");
        }

        for key in merman_bindings_core::MetadataKey::ALL {
            if !expected_ids.contains(&key.id()) {
                let error = match metadata_wire_for_engine(&engine, key.id()) {
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

    #[cfg(feature = "svg")]
    #[test]
    fn data_only_wire_omits_metadata_and_preserves_result_bytes_and_errors() {
        let engine = create_engine("").unwrap();
        let source = "flowchart LR\r\nA[\"北京😀\"] --> B[\"Café\"]\r\n%% quote: \"\\\t\r\n";
        let options = r#"{"site_config":{"htmlLabels":false},"svg":{"diagram_id":"direct-svg"}}"#;
        for operation in ["svg", "semantic-json"] {
            let expected = engine
                .execute(
                    merman_bindings_core::BindingOperationRequest::new(
                        operation,
                        source.as_bytes(),
                    )
                    .with_options_json(options.as_bytes()),
                )
                .unwrap();
            let mut request = serde_json::json!({
                "operation_id": operation,
                "source": source,
                "uri": null,
                "options_json": options,
            });
            let full: serde_json::Value =
                serde_json::from_str(&execute_wire(&engine, &request.to_string())).unwrap();
            assert_eq!(full["ok"], true);
            assert_eq!(
                full["result"]["data"].as_str().unwrap().as_bytes(),
                expected.data(),
            );
            assert_eq!(
                full["result"]["metadata_json"].as_str().unwrap().as_bytes(),
                expected.metadata_json(),
            );
            request["include_metadata"] = false.into();
            let data: serde_json::Value =
                serde_json::from_str(&execute_wire(&engine, &request.to_string())).unwrap();
            assert_eq!(data["ok"], true);
            assert_eq!(
                data["result"]["operation_id"],
                full["result"]["operation_id"]
            );
            assert_eq!(data["result"]["media_type"], full["result"]["media_type"]);
            assert_eq!(data["result"]["data"], full["result"]["data"]);
            assert!(data["result"].get("metadata_json").is_none());

            #[cfg(feature = "transport-napi")]
            if operation == "svg" {
                let direct = execute_svg_with_admitted_control(
                    &engine,
                    source,
                    DirectSvgOptions::Json(Some(options)),
                    admitted_operation_control(None),
                    None,
                )
                .unwrap();
                assert_eq!(direct, data["result"]["data"].as_str().unwrap());
            }
        }
        for (operation, options) in [
            ("unknown-operation", ""),
            ("svg", "{"),
            ("svg", r#"{"version":99}"#),
        ] {
            let mut request = serde_json::json!({
                "operation_id": operation,
                "source": "flowchart TD\nA --> B",
                "uri": null,
                "options_json": options,
            });
            let full: serde_json::Value =
                serde_json::from_str(&execute_wire(&engine, &request.to_string())).unwrap();
            assert_eq!(full["ok"], false);
            request["include_metadata"] = false.into();
            let data: serde_json::Value =
                serde_json::from_str(&execute_wire(&engine, &request.to_string())).unwrap();
            assert_eq!(data, full);

            #[cfg(feature = "transport-napi")]
            if operation == "svg" {
                let error = execute_svg_with_admitted_control(
                    &engine,
                    request["source"].as_str().unwrap(),
                    DirectSvgOptions::Json(Some(options)),
                    admitted_operation_control(None),
                    None,
                )
                .unwrap_err();
                assert_eq!(
                    serde_json::from_str::<serde_json::Value>(&error_envelope(&error)).unwrap(),
                    data
                );
            }
        }
    }

    #[test]
    fn decoded_request_options_keep_duplicate_keys_for_theme_diagnostics() {
        let engine = create_engine("").unwrap();
        let source = "flowchart LR\nA --> B";
        let options = "{\n  \"theme\": {\n    \"schema_version\": 2,\n    \"extends\": \"studio\", \"extends\": \"paper-cut\"\n  }\n}";
        let raw = merman_bindings_core::BindingOperationRequest::new("svg", source.as_bytes())
            .with_options_json(options.as_bytes());
        let expected = engine.execute_data(raw).unwrap_err();
        assert!(expected.message().contains("duplicate key"));
        let envelope = serde_json::json!({
            "operation_id": "svg",
            "source": source,
            "options_json": options,
            "include_metadata": false,
        });
        assert_eq!(
            execute_wire(&engine, &envelope.to_string()),
            error_envelope(&expected)
        );
        #[cfg(feature = "transport-napi")]
        assert_eq!(
            execute_svg_with_admitted_control(
                &engine,
                source,
                DirectSvgOptions::Json(Some(options)),
                admitted_operation_control(None),
                None,
            )
            .unwrap_err(),
            expected,
        );
    }

    #[cfg(feature = "svg")]
    #[test]
    fn theme_catalog_presets_export_through_the_node_transport() {
        let engine = create_engine(r#"{"resources":{"profile":"constrained"}}"#)
            .expect("constrained Node engine");
        let catalog: serde_json::Value = serde_json::from_str(
            &metadata_wire_for_engine(&engine, "theme-catalog").expect("engine theme catalog"),
        )
        .expect("Node theme catalog JSON");
        assert_eq!(catalog["schema_version"], 2);
        let presets = catalog["presets"].as_array().expect("preset catalog");
        assert!(!presets.is_empty());
        for preset in presets {
            let request = serde_json::json!({
                "operation_id": "export-theme-preset",
                "source": preset["id"], "uri": null,
            });
            let result: serde_json::Value =
                serde_json::from_str(&execute_wire(&engine, &request.to_string()))
                    .expect("Node preset export envelope");
            assert_eq!(result["ok"], true, "{}: {result}", preset["id"]);
            assert_eq!(result["result"]["media_type"], "application/json");
            let document: serde_json::Value =
                serde_json::from_str(result["result"]["data"].as_str().unwrap()).unwrap();
            assert_eq!(document["schema_version"], 2);
            assert_eq!(document["id"], preset["id"]);
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
        assert_eq!(payload["error"]["details"]["resource"]["cause"], "ceiling");
        assert_eq!(payload["error"]["details"]["resource"]["actual"], 5);
        assert_eq!(payload["error"]["details"]["resource"]["max"], 4);

        let error = merman_bindings_core::BindingError::resource_limit_with_cause(
            merman_bindings_core::BindingResourceLimitCause::ArithmeticOverflow,
            "layout_model",
            "max_layout_work_units",
            u64::MAX,
            800_000,
            "interactive",
            "layout work accounting overflowed",
        );
        let payload: serde_json::Value =
            serde_json::from_str(&error_envelope(&error)).expect("Node error envelope");
        assert_eq!(
            payload["error"]["code_name"],
            "MERMAN_RESOURCE_LIMIT_EXCEEDED"
        );
        assert_eq!(
            payload["error"]["details"]["resource"]["cause"],
            "arithmetic_overflow"
        );
        assert_eq!(
            payload["error"]["details"]["resource"]["limit_id"],
            "max_layout_work_units"
        );
        assert_eq!(
            payload["error"]["details"]["resource"]["phase"],
            "layout_model"
        );
        assert_eq!(
            payload["error"]["details"]["resource"]["actual"],
            "18446744073709551615"
        );
        assert_eq!(payload["error"]["details"]["resource"]["max"], 800_000);
        assert_eq!(
            payload["error"]["details"]["resource"]["profile"],
            "interactive"
        );
    }

    #[cfg(feature = "svg")]
    #[test]
    fn resolve_theme_wire_preserves_document_diagnostics() {
        let engine = create_engine("").unwrap();
        let document =
            "{\n  \"schema_version\": 2,\n  \"schema_version\": 2,\n  \"extends\": \"studio\"\n}";
        let request = serde_json::json!({
            "operation_id": "resolve-theme",
            "source": serde_json::json!({ "document": document }).to_string(),
            "include_metadata": false,
        });
        let envelope: serde_json::Value =
            serde_json::from_str(&execute_wire(&engine, &request.to_string())).unwrap();
        assert_eq!(envelope["ok"], true, "{envelope}");
        let result: serde_json::Value =
            serde_json::from_str(envelope["result"]["data"].as_str().unwrap()).unwrap();
        assert_eq!(result["ok"], false);
        assert_eq!(result["line"], 3);
        assert!(
            result["message"]
                .as_str()
                .unwrap()
                .contains("duplicate key `schema_version`")
        );
    }

    #[test]
    fn error_wire_preserves_structured_diagnostic_details() {
        use merman_bindings_core::{
            BindingDiagnosticErrorDetails, BindingDiagnosticSpan, BindingError, BindingStatus,
        };

        let error = BindingError::new(BindingStatus::ParseError, "invalid edge")
            .with_diagnostic_details(
                BindingDiagnosticErrorDetails::new("flowchart.edge.invalid")
                    .with_span(BindingDiagnosticSpan::new(3, 8, "exact"))
                    .with_field("edge")
                    .with_diagram_type("flowchart"),
            );
        let payload: serde_json::Value =
            serde_json::from_str(&error_envelope(&error)).expect("Node error envelope");

        let diagnostic = &payload["error"]["details"]["diagnostic"];
        assert_eq!(diagnostic["code"], "flowchart.edge.invalid");
        assert_eq!(diagnostic["span"]["start"], 3);
        assert_eq!(diagnostic["span"]["end"], 8);
        assert_eq!(diagnostic["span"]["kind"], "exact");
        assert_eq!(diagnostic["field"], "edge");
        assert_eq!(diagnostic["diagram_type"], "flowchart");
    }

    #[test]
    fn operation_control_preserves_requested_and_deadline_cancellation_details() {
        let engine = create_engine("").unwrap();
        let deadline: serde_json::Value = serde_json::from_str(
            &execute_wire_with_admitted_control(
                &engine,
                r#"{"operation_id":"semantic-json","source":"flowchart TD\nA-->B","uri":null,"operation_control":{"timeout_ms":0}}"#,
                admitted_operation_control(Some(0)),
                Some(0),
            ),
        )
        .expect("deadline error envelope");
        assert_eq!(deadline["error"]["code_name"], "MERMAN_CANCELLED");
        assert_eq!(
            deadline["error"]["details"]["cancellation"]["reason"],
            "deadline_exceeded"
        );
        assert_eq!(
            deadline["error"]["details"]["cancellation"]["phase"],
            "admission"
        );

        let requested = merman_bindings_core::OperationControl::new();
        requested.cancel();
        let cancelled: serde_json::Value = serde_json::from_str(&execute_wire_with_control(
            &engine,
            r#"{"operation_id":"semantic-json","source":"flowchart TD\nA-->B","uri":null}"#,
            requested,
        ))
        .expect("requested cancellation envelope");
        assert_eq!(cancelled["error"]["code_name"], "MERMAN_CANCELLED");
        assert_eq!(
            cancelled["error"]["details"]["cancellation"]["reason"],
            "requested"
        );
        assert_eq!(
            cancelled["error"]["details"]["cancellation"]["phase"],
            "admission"
        );
        assert!(cancelled["error"]["details"].get("resource").is_none());
    }

    #[test]
    fn cancelled_operation_preserves_canonical_mixed_error_precedence() {
        let engine = create_engine("").unwrap();
        let cases = [
            (
                r#"{"operation_id":"unknown-operation","source":"flowchart TD\nA-->B","uri":null,"options_json":"{"}"#,
                "MERMAN_UNSUPPORTED_OPERATION",
                "unknown-operation",
                Some("unknown operation `unknown-operation`"),
            ),
            (
                r#"{"operation_id":"document-analysis-json","source":"flowchart TD\nA-->B","uri":null,"options_json":"{"}"#,
                "MERMAN_INVALID_ARGUMENT",
                "generic",
                Some("operation `document-analysis-json` requires a document URI"),
            ),
            (
                r#"{"operation_id":"document-analysis-json","source":"flowchart TD\nA-->B","uri":"","options_json":"{"}"#,
                "MERMAN_INVALID_ARGUMENT",
                "generic",
                Some("operation `document-analysis-json` requires a document URI"),
            ),
            (
                r#"{"operation_id":"semantic-json","source":"flowchart TD\nA-->B","uri":"file:///diagram.mmd","options_json":"{"}"#,
                "MERMAN_INVALID_ARGUMENT",
                "generic",
                Some("operation `semantic-json` does not accept a document URI"),
            ),
            (
                r#"{"operation_id":"semantic-json","source":"flowchart TD\nA-->B","uri":"","options_json":"{"}"#,
                "MERMAN_CANCELLED",
                "generic",
                None,
            ),
            (
                r#"{"operation_id":"svg","source":"flowchart TD\nA-->B","uri":null,"include_metadata":false,"options_json":"{"}"#,
                "MERMAN_CANCELLED",
                "generic",
                None,
            ),
        ];

        for (request, expected_code_name, expected_kind, expected_message) in cases {
            let control = merman_bindings_core::OperationControl::new();
            control.cancel();
            let response: serde_json::Value = serde_json::from_str(&execute_wire_with_control(
                &engine,
                request,
                control.clone(),
            ))
            .expect("mixed-error response");

            assert_eq!(response["error"]["code_name"], expected_code_name);
            assert_eq!(response["error"]["kind"], expected_kind);
            if expected_code_name == "MERMAN_CANCELLED" {
                assert_eq!(
                    response["error"]["details"]["cancellation"]["phase"],
                    "admission"
                );
            } else {
                assert_eq!(response["error"]["message"], expected_message.unwrap());
                assert!(response["error"].get("details").is_none());
            }
        }
    }

    #[cfg(feature = "transport-napi")]
    #[test]
    fn direct_svg_deadline_precedes_nested_option_validation() {
        let engine = create_engine("").unwrap();
        let source = "flowchart TD\nA-->B";
        let request = r#"{"operation_id":"svg","source":"flowchart TD\nA-->B","uri":null,"include_metadata":false,"options_json":"{","operation_control":{"timeout_ms":0}}"#;
        let legacy = execute_wire_with_admitted_control(
            &engine,
            request,
            admitted_operation_control(Some(0)),
            Some(0),
        );
        let direct = execute_svg_with_admitted_control(
            &engine,
            source,
            DirectSvgOptions::Json(Some("{")),
            admitted_operation_control(Some(0)),
            Some(0),
        )
        .unwrap_err();
        assert_eq!(error_envelope(&direct), legacy);
        assert_eq!(
            direct.status(),
            merman_bindings_core::BindingStatus::Cancelled
        );
    }

    #[cfg(feature = "transport-napi")]
    #[test]
    fn typed_svg_ids_preserve_json_output_and_deadlines() {
        let engine = create_engine(r#"{"site_config":{"layout":"dagre"}}"#).unwrap();
        let source = "flowchart LR\nA[Hello] --> B[World]";
        for id in ["", "one:部署 😀\"\\\n"] {
            let json = serde_json::json!({"svg": {"diagram_id": id}}).to_string();
            for timeout in [None, Some(0)] {
                let render = |options| {
                    execute_svg_with_admitted_control(
                        &engine,
                        source,
                        options,
                        admitted_operation_control(timeout),
                        timeout,
                    )
                };
                let expected = render(DirectSvgOptions::Json(Some(&json)));
                if timeout.is_none() {
                    assert!(expected.as_ref().is_ok_and(|svg| svg.starts_with("<svg")));
                }
                assert_eq!(render(DirectSvgOptions::DiagramId(id)), expected);
            }
        }
    }

    #[cfg(feature = "transport-napi")]
    #[test]
    fn typed_svg_id_fallback_matches_json_at_the_options_limit() {
        let source = "flowchart LR\nA --> B";
        let prefix = "\"\\\t\n\r\u{8}\u{c}\u{1f}é🧪\u{2028}\u{2029}/";
        let options_json = |id: &str| serde_json::json!({"svg": {"diagram_id": id}}).to_string();
        let max = node_wire_contract().fields.options_json_utf8_bytes;
        let remaining = max - options_json(prefix).len();
        let mut id = format!(
            "{prefix}{}{}",
            "\0".repeat(remaining / 6),
            "x".repeat(remaining % 6)
        );
        let expected = options_json(&id);
        assert_eq!(expected.len(), max);
        assert!(!direct_svg_id_fits(source, &id, None));

        let control = admitted_operation_control(None);
        assert_eq!(
            prepare_direct_svg_id_fallback(source, &id, &control, None).unwrap(),
            expected
        );
        id.push('x');
        let error = prepare_direct_svg_id_fallback(source, &id, &control, None).unwrap_err();
        assert!(error.message().contains("options_json exceeds"));
    }

    #[cfg(feature = "transport-napi")]
    #[test]
    fn typed_svg_id_fallback_preserves_admission_error_precedence() {
        let engine = create_engine("").unwrap();
        let contract = node_wire_contract();
        let id = "\0".repeat(contract.fields.options_json_utf8_bytes / 6 + 1);
        let options = serde_json::json!({"svg": {"diagram_id": &id}}).to_string();
        let timeout = Some(u32::MAX);
        let empty_source_bytes = serde_json::json!({
            "operation_id": "svg",
            "source": "",
            "uri": null,
            "options_json": &options,
            "include_metadata": false,
            "operation_control": {"timeout_ms": u32::MAX},
        })
        .to_string()
        .len();
        let remaining = contract.documents.request.max_utf8_bytes - empty_source_bytes;
        let mut source = format!(
            "{}{}",
            "\0".repeat(remaining / 6),
            "x".repeat(remaining % 6)
        );
        assert!(!direct_svg_id_fits(&source, &id, timeout));

        let reject = |source: &str, cancelled| {
            let control = admitted_operation_control(timeout);
            if cancelled {
                control.cancel();
            }
            let run = |options| {
                execute_svg_with_admitted_control(
                    &engine,
                    source,
                    options,
                    control.clone(),
                    timeout,
                )
                .unwrap_err()
            };
            let error = run(DirectSvgOptions::DiagramId(&id));
            assert_eq!(error, run(DirectSvgOptions::Json(Some(&options))));
            error
        };
        assert!(
            reject(&source, false)
                .message()
                .contains("options_json exceeds")
        );
        let cancelled = reject(&source, true);
        assert_eq!(cancelled.cancellation_details().unwrap().phase, "admission");
        source.push('x');
        assert!(reject(&source, true).message().contains("wire limit"));
    }

    #[test]
    fn admitted_timeout_must_match_the_wire_request_after_deadline_priority() {
        let engine = create_engine("").unwrap();
        let matching: serde_json::Value = serde_json::from_str(
            &execute_wire_with_admitted_control(
                &engine,
                r#"{"operation_id":"semantic-json","source":"flowchart TD\nA-->B","uri":"","operation_control":{"timeout_ms":60000}}"#,
                admitted_operation_control(Some(60_000)),
                Some(60_000),
            ),
        )
        .expect("matching timeout response");
        assert_eq!(matching["ok"], true);

        let request_only: serde_json::Value = serde_json::from_str(&execute_wire(
            &engine,
            r#"{"operation_id":"semantic-json","source":"flowchart TD\nA-->B","uri":null,"operation_control":{"timeout_ms":60000}}"#,
        ))
        .expect("request-only timeout response");
        assert_eq!(
            request_only["error"]["code_name"],
            "MERMAN_INVALID_ARGUMENT"
        );

        for (request, admitted) in [
            (
                r#"{"operation_id":"semantic-json","source":"flowchart TD\nA-->B","uri":null,"operation_control":{"timeout_ms":60000}}"#,
                59_999,
            ),
            (
                r#"{"operation_id":"semantic-json","source":"flowchart TD\nA-->B","uri":null}"#,
                60_000,
            ),
        ] {
            let rejected: serde_json::Value =
                serde_json::from_str(&execute_wire_with_admitted_control(
                    &engine,
                    request,
                    admitted_operation_control(Some(admitted)),
                    Some(admitted),
                ))
                .expect("timeout mismatch response");
            assert_eq!(rejected["error"]["code_name"], "MERMAN_INVALID_ARGUMENT");
        }

        let expired: serde_json::Value = serde_json::from_str(
            &execute_wire_with_admitted_control(
                &engine,
                r#"{"operation_id":"semantic-json","source":"flowchart TD\nA-->B","uri":null,"operation_control":{"timeout_ms":1}}"#,
                admitted_operation_control(Some(0)),
                Some(0),
            ),
        )
        .expect("expired timeout response");
        assert_eq!(expired["error"]["code_name"], "MERMAN_CANCELLED");
        assert_eq!(
            expired["error"]["details"]["cancellation"]["reason"],
            "deadline_exceeded"
        );
        assert_eq!(
            expired["error"]["details"]["cancellation"]["phase"],
            "admission"
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

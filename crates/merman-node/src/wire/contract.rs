//! Static Node artifact identity, runtime catalog, and wire limits.

use std::sync::{LazyLock, OnceLock};

use merman_bindings_core::{
    ArtifactContractSpec, BindingError, BindingPayloadSchemaKey, BindingTransportKey,
    CAPABILITY_DESCRIPTOR_DIGEST, CapabilityKey, ConstructorServiceKey, NODE_TRANSPORT_API_VERSION,
    OperationKey, RUNTIME_CATALOG_SCHEMA_VERSION, RuntimePolicyExposure, TargetKey,
    ValidatedArtifactContract,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::json::{deserialize_bounded_json, ensure_field, serialize_bounded_json};
use super::{binding_boundary, producer_error};

const NODE_WIRE_CONTRACT_JSON: &str =
    include_str!("../../../../platforms/node/src/generated/node-wire-contract.json");
pub(super) const NODE_BINDING_RESULT_PAYLOAD_VERSION: u32 =
    BindingPayloadSchemaKey::BindingResult.version();
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
const NODE_SUPPLEMENTAL_CAPABILITIES: &[CapabilityKey] = merman_bindings_core::mermaid_layout_capabilities![
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
    let mut contract = serde_json::from_str::<NodeWireContract>(NODE_WIRE_CONTRACT_JSON)
        .expect("the embedded Node wire contract must be valid JSON");
    let ids = &mut contract.artifact.capability_ids;
    let canonical_len = ids.len();
    ids.extend(
        NODE_SUPPLEMENTAL_CAPABILITIES
            .iter()
            .filter(|key| {
                matches!(
                    key,
                    CapabilityKey::LayoutGrid
                        | CapabilityKey::LayoutLayered
                        | CapabilityKey::LayoutShoal
                )
            })
            .map(|key| key.id().to_owned()),
    );
    if ids.len() != canonical_len {
        ids.sort_unstable();
    }
    contract
        .validate()
        .expect("the embedded Node wire contract must be internally consistent");
    contract
});

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NodeWireContract {
    schema_version: u32,
    package_id: String,
    artifact_id: String,
    transport_api_version: u32,
    binding_result_payload_version: u32,
    artifact: NodeArtifactProfile,
    pub(super) documents: NodeDocumentLimitSet,
    pub(super) fields: NodeFieldLimits,
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
pub(super) struct NodeDocumentLimitSet {
    identity: NodeDocumentLimits,
    pub(super) binding_options: NodeDocumentLimits,
    pub(super) request: NodeDocumentLimits,
    runtime_catalog: NodeDocumentLimits,
    pub(super) response: NodeDocumentLimits,
    pub(super) error: NodeDocumentLimits,
    pub(super) metadata: NodeDocumentLimits,
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
pub(super) struct NodeDocumentLimits {
    pub(super) max_utf8_bytes: usize,
    pub(super) max_depth: usize,
    pub(super) max_members: usize,
    pub(super) max_tokens: usize,
    pub(super) max_string_utf8_bytes: usize,
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
pub(super) struct NodeFieldLimits {
    pub(super) operation_id_utf8_bytes: usize,
    pub(super) media_type_utf8_bytes: usize,
    pub(super) metadata_id_utf8_bytes: usize,
    pub(super) uri_utf8_bytes: usize,
    pub(super) source_utf8_bytes: usize,
    pub(super) options_json_utf8_bytes: usize,
    pub(super) data_utf8_bytes: usize,
    pub(super) metadata_json_utf8_bytes: usize,
    pub(super) error_code_name_utf8_bytes: usize,
    pub(super) error_kind_utf8_bytes: usize,
    pub(super) error_message_utf8_bytes: usize,
    pub(super) capability_id_utf8_bytes: usize,
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

pub(super) fn node_wire_contract() -> &'static NodeWireContract {
    &NODE_WIRE_CONTRACT
}

pub(super) fn node_artifact_contract() -> &'static ValidatedArtifactContract {
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

#[cfg(test)]
mod tests {
    #[cfg(feature = "diagram-flowchart")]
    use super::NODE_SUPPLEMENTAL_CAPABILITIES;
    use super::{
        NodeTransportKind, node_wire_contract, runtime_catalog_wire, transport_identity_wire,
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

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn explicit_mermaid_layouts_follow_node_owner_policy() {
        use crate::wire::{create_engine, execute_wire};
        use merman_bindings_core::CapabilityKey;

        let engine = create_engine("{}").unwrap();
        for (layout, capability) in [
            ("grid", CapabilityKey::LayoutGrid),
            ("layered", CapabilityKey::LayoutLayered),
        ] {
            let declared = NODE_SUPPLEMENTAL_CAPABILITIES.contains(&capability);
            assert_eq!(
                node_wire_contract()
                    .artifact
                    .capability_ids
                    .iter()
                    .any(|id| id == capability.id()),
                declared
            );
            let source = format!(
                "---\nconfig:\n  layout: {layout}\n---\nflowchart LR\nA[Start] -->|Approved| B[Done]\n"
            );
            let execute = |operation_id| {
                let request = serde_json::json!({ "operation_id": operation_id, "source": source });
                serde_json::from_str::<serde_json::Value>(&execute_wire(
                    &engine,
                    &request.to_string(),
                ))
                .unwrap()
            };
            let plan = execute("svg-plan-json");
            let geometry = execute("layout-json");
            let svg = execute("svg");
            if declared {
                let plan: serde_json::Value =
                    serde_json::from_str(plan["result"]["data"].as_str().unwrap()).unwrap();
                assert_eq!(
                    plan["required_capability_ids"],
                    serde_json::json!([capability.id()])
                );
                assert_eq!(plan["missing_capability_ids"], serde_json::json!([]));
                assert_eq!(plan["ready"], true);
                let geometry: serde_json::Value =
                    serde_json::from_str(geometry["result"]["data"].as_str().unwrap()).unwrap();
                assert_eq!(geometry["meta"]["effective_config"]["layout"], layout);
                assert!(geometry["layout"]["FlowchartV2"].is_object());
                assert!(svg["result"]["data"].as_str().unwrap().starts_with("<svg"));
            } else {
                let expected_kind = if plan["ok"] == true {
                    let plan: serde_json::Value =
                        serde_json::from_str(plan["result"]["data"].as_str().unwrap()).unwrap();
                    assert_eq!(
                        plan["required_capability_ids"],
                        serde_json::json!([capability.id()])
                    );
                    assert_eq!(
                        plan["missing_capability_ids"],
                        serde_json::json!([capability.id()])
                    );
                    assert_eq!(plan["ready"], false);
                    "missing-capability"
                } else {
                    assert_eq!(plan["error"]["code_name"], "MERMAN_UNSUPPORTED_OPERATION");
                    assert_eq!(plan["error"]["kind"], "generic");
                    "generic"
                };
                for response in [geometry, svg] {
                    assert_eq!(response["ok"], false);
                    assert_eq!(
                        response["error"]["code_name"],
                        "MERMAN_UNSUPPORTED_OPERATION"
                    );
                    assert_eq!(response["error"]["kind"], expected_kind);
                    if expected_kind == "missing-capability" {
                        assert_eq!(response["error"]["capability_id"], capability.id());
                    }
                }
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
}

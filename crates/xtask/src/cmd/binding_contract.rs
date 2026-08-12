//! Generated host-language projections of binding-core-owned runtime vocabularies.

use crate::XtaskError;
#[cfg(test)]
use merman_bindings_core::TextMeasurementProviderSource;
use merman_bindings_core::{
    ArtifactContractSpec, BINDING_OPERATION_METADATA_CONTRACT_SCHEMA_VERSION,
    BINDING_OPTIONS_SCHEMA_VERSION, BindingOperationMetadataContract, BindingOptionGroupKey,
    BindingPayloadSchemaKey, BindingTransportKey, BindingUnavailableOperationExpectation,
    CapabilityKey, ConstructorServiceKey, MetadataKey, OperationKey,
    RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN, RUNTIME_CATALOG_IDENTIFIER_PATTERN,
    RUNTIME_CATALOG_MAX_SAFE_INTEGER, RUNTIME_CATALOG_SCHEMA_VERSION,
    RuntimeConstructorResourceLimit, RuntimeOutputContract, RuntimePolicyExposure,
    TEXT_MEASUREMENT_PROTOCOL_VERSION, TargetKey, TextMeasurementProviderKey,
    ValidatedArtifactContract, binding_operation_expectations, operation_metadata_contract,
    runtime_constructor_resource_limits,
};
use serde::Serialize;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const NODE_OUTPUT: &str = "platforms/node/src/generated/binding-contract.mjs";
const NODE_WIRE_OUTPUT: &str = "platforms/node/src/generated/node-wire-contract.json";
const WEB_OUTPUT: &str = "platforms/web/src/generated/binding-contract.ts";
const SHARED_OPERATION_OUTPUT: &str =
    "fixtures/bindings/generated/binding-operation-contract-v1.json";
const UNIFFI_PYTHON_OUTPUT: &str = "crates/merman-uniffi/python/binding_contract.py";
const PYTHON_OUTPUT: &str = "platforms/python/merman/src/merman/_binding_contract.py";
const KOTLIN_OUTPUT: &str = "platforms/android/src/main/kotlin/io/merman/MermanBindingContract.kt";
const DART_OUTPUT: &str = "platforms/flutter/lib/src/generated/binding_contract.dart";
const NODE_STATIC_SVG_OPERATIONS: &[OperationKey] = &[
    OperationKey::LayoutJson,
    OperationKey::SemanticJson,
    OperationKey::Svg,
    OperationKey::SvgPlanJson,
];
const NODE_STATIC_SVG_SUPPLEMENTAL_CAPABILITIES: &[CapabilityKey] =
    &[CapabilityKey::LayoutCytoscape, CapabilityKey::LayoutElk];
const DEFAULT_NATIVE_PREBUILT_OPERATIONS: &[OperationKey] = &[
    OperationKey::AnalysisFactsJson,
    OperationKey::AnalysisJson,
    OperationKey::Ascii,
    OperationKey::DocumentAnalysisFactsJson,
    OperationKey::DocumentAnalysisJson,
    OperationKey::LayoutJson,
    OperationKey::SemanticJson,
    OperationKey::Svg,
    OperationKey::SvgPlanJson,
    OperationKey::ValidationJson,
];
const DEFAULT_NATIVE_PREBUILT_SUPPLEMENTAL_CAPABILITIES: &[CapabilityKey] =
    &[CapabilityKey::LayoutCytoscape, CapabilityKey::LayoutElk];
const DEFAULT_NATIVE_PREBUILT_CONSTRUCTOR_SERVICES: &[ConstructorServiceKey] = &[
    ConstructorServiceKey::HostTextMeasurement,
    ConstructorServiceKey::IconRegistry,
];

#[derive(Serialize)]
struct MetadataProjection {
    id: &'static str,
    required_capability_id: Option<&'static str>,
}

#[derive(Serialize)]
struct CapabilityProjection {
    id: &'static str,
    implication_ids: Vec<&'static str>,
}

#[derive(Serialize)]
struct OptionGroupProjection {
    id: &'static str,
    always_available: bool,
    any_capability_ids: Vec<&'static str>,
    requires_svg_pipeline: bool,
}

#[derive(Serialize)]
struct PayloadSchemaProjection {
    id: &'static str,
    version: u32,
}

#[derive(Serialize)]
struct ConstructorServiceProjection {
    id: &'static str,
    requires_svg_pipeline: bool,
    provided_text_measurement_provider_ids: Vec<&'static str>,
    resource_limits: Vec<RuntimeConstructorResourceLimit>,
}

#[derive(Serialize)]
struct TextMeasurementProviderProjection {
    id: &'static str,
    source: &'static str,
    constructor_service_id: Option<&'static str>,
}

#[derive(Serialize)]
struct TransportExposureProjection {
    id: &'static str,
    payload_schema_ids: Vec<&'static str>,
    constructor_service_candidate_ids: Vec<&'static str>,
}

#[derive(Serialize)]
struct OperationExpectationProjection {
    operation_id: &'static str,
    output_id: Option<&'static str>,
    media_type: &'static str,
    metadata_schema_version: u32,
    requires_uri: bool,
    availability_capability_id: Option<&'static str>,
    compiled_prerequisite_ids: Vec<&'static str>,
    unavailable: Option<BindingUnavailableOperationExpectation>,
}

#[derive(Serialize)]
struct SharedOperationContractProjection {
    schema_version: u32,
    operation_metadata_contract: &'static BindingOperationMetadataContract,
    operation_expectations: Vec<OperationExpectationProjection>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct BindingArtifactProfileProjection {
    capability_ids: Vec<&'static str>,
    output_ids: Vec<&'static str>,
    output_contracts: Vec<RuntimeOutputContract>,
    system_adapter_ids: Vec<&'static str>,
    operation_ids: Vec<&'static str>,
    metadata_ids: Vec<&'static str>,
    option_group_ids: Vec<&'static str>,
    constructor_service_ids: Vec<&'static str>,
    text_measurement_provider_ids: Vec<&'static str>,
}

#[derive(Serialize)]
struct NodeWireContractProjection {
    schema_version: u32,
    package_id: &'static str,
    artifact_id: &'static str,
    transport_api_version: u32,
    binding_result_payload_version: u32,
    artifact: BindingArtifactProfileProjection,
    documents: NodeDocumentLimitSetProjection,
    fields: NodeFieldLimitsProjection,
}

#[derive(Serialize)]
struct NodeDocumentLimitSetProjection {
    identity: NodeDocumentLimitsProjection,
    binding_options: NodeDocumentLimitsProjection,
    request: NodeDocumentLimitsProjection,
    runtime_catalog: NodeDocumentLimitsProjection,
    response: NodeDocumentLimitsProjection,
    error: NodeDocumentLimitsProjection,
    metadata: NodeDocumentLimitsProjection,
}

#[derive(Clone, Copy, Serialize)]
struct NodeDocumentLimitsProjection {
    max_utf8_bytes: usize,
    max_depth: usize,
    max_members: usize,
    max_tokens: usize,
    max_string_utf8_bytes: usize,
}

#[derive(Serialize)]
struct NodeFieldLimitsProjection {
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

struct BindingRegistryProjections {
    capabilities: Vec<CapabilityProjection>,
    metadata: Vec<MetadataProjection>,
    option_groups: Vec<OptionGroupProjection>,
    payload_schemas: Vec<PayloadSchemaProjection>,
    providers: Vec<TextMeasurementProviderProjection>,
    provider_ids: Vec<&'static str>,
    constructor_services: Vec<ConstructorServiceProjection>,
    transports: Vec<TransportExposureProjection>,
}

fn pretty_json(value: &impl Serialize) -> String {
    serde_json::to_string_pretty(value).expect("binding contract projection must serialize")
}

fn kotlin_string_arguments(values: &[&str]) -> String {
    values
        .iter()
        .map(|value| format!("{value:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn upper_snake(value: &str) -> String {
    value.replace('-', "_").to_ascii_uppercase()
}

fn lower_camel(value: &str) -> String {
    let mut parts = value.split(['-', '_']).filter(|part| !part.is_empty());
    let Some(first) = parts.next() else {
        return String::new();
    };
    let mut projected = first.to_owned();
    for part in parts {
        let mut characters = part.chars();
        if let Some(first) = characters.next() {
            projected.push(first.to_ascii_uppercase());
            projected.push_str(characters.as_str());
        }
    }
    projected
}

fn operation_expectation_projections() -> Vec<OperationExpectationProjection> {
    binding_operation_expectations()
        .iter()
        .map(|expectation| OperationExpectationProjection {
            operation_id: expectation.operation_id(),
            output_id: expectation.output_id(),
            media_type: expectation.media_type(),
            metadata_schema_version: expectation.metadata_schema_version(),
            requires_uri: expectation.requires_uri(),
            availability_capability_id: expectation.availability_capability_id(),
            compiled_prerequisite_ids: expectation.compiled_prerequisite_ids().to_vec(),
            unavailable: expectation.unavailable(),
        })
        .collect()
}

fn shared_operation_contract_projection() -> SharedOperationContractProjection {
    SharedOperationContractProjection {
        schema_version: BINDING_OPERATION_METADATA_CONTRACT_SCHEMA_VERSION,
        operation_metadata_contract: operation_metadata_contract(),
        operation_expectations: operation_expectation_projections(),
    }
}

fn artifact_profile_projection(
    contract: ValidatedArtifactContract,
) -> BindingArtifactProfileProjection {
    let catalog = contract.runtime_catalog(1);
    BindingArtifactProfileProjection {
        capability_ids: contract.capability_keys().map(CapabilityKey::id).collect(),
        output_ids: contract.output_keys().map(|key| key.id()).collect(),
        output_contracts: catalog.output_contracts,
        system_adapter_ids: contract
            .system_adapter_keys()
            .map(CapabilityKey::id)
            .collect(),
        operation_ids: contract.operation_keys().map(|key| key.id()).collect(),
        metadata_ids: contract.metadata_keys().map(MetadataKey::id).collect(),
        option_group_ids: contract
            .option_group_keys()
            .map(BindingOptionGroupKey::id)
            .collect(),
        constructor_service_ids: contract
            .constructor_service_keys()
            .map(ConstructorServiceKey::id)
            .collect(),
        text_measurement_provider_ids: contract
            .text_measurement_provider_keys()
            .map(TextMeasurementProviderKey::id)
            .collect(),
    }
}

fn node_artifact_profile_projection(target: TargetKey) -> BindingArtifactProfileProjection {
    artifact_profile_projection(
        ArtifactContractSpec::new(target, BindingTransportKey::Node)
            .with_operations(NODE_STATIC_SVG_OPERATIONS)
            .with_supplemental_capabilities(NODE_STATIC_SVG_SUPPLEMENTAL_CAPABILITIES)
            .with_all_available_metadata()
            .with_runtime_policy_exposure(RuntimePolicyExposure::DeterministicOnly)
            .materialize(),
    )
}

fn android_artifact_profile_projection() -> BindingArtifactProfileProjection {
    artifact_profile_projection(
        ArtifactContractSpec::new(TargetKey::Native, BindingTransportKey::AndroidJni)
            .with_operations(DEFAULT_NATIVE_PREBUILT_OPERATIONS)
            .with_supplemental_capabilities(DEFAULT_NATIVE_PREBUILT_SUPPLEMENTAL_CAPABILITIES)
            .with_all_available_metadata()
            .with_constructor_services(DEFAULT_NATIVE_PREBUILT_CONSTRUCTOR_SERVICES)
            .with_runtime_policy_exposure(RuntimePolicyExposure::BindingOptions)
            .materialize(),
    )
}

fn render_node_wire_contract_json() -> String {
    let native = node_artifact_profile_projection(TargetKey::Native);
    let web = node_artifact_profile_projection(TargetKey::Web);
    assert_eq!(
        native, web,
        "the private Node N-API and WASM candidates must expose one artifact profile"
    );
    let megabyte = 1024 * 1024;
    let contract = NodeWireContractProjection {
        schema_version: 1,
        package_id: "@mermanjs/node",
        artifact_id: "merman-node-static-svg",
        transport_api_version: 1,
        binding_result_payload_version: BindingPayloadSchemaKey::BindingResult.version(),
        artifact: native,
        documents: NodeDocumentLimitSetProjection {
            identity: NodeDocumentLimitsProjection {
                max_utf8_bytes: 4 * 1024,
                max_depth: 8,
                max_members: 128,
                max_tokens: 256,
                max_string_utf8_bytes: 1024,
            },
            binding_options: NodeDocumentLimitsProjection {
                max_utf8_bytes: megabyte,
                max_depth: 64,
                max_members: 65_536,
                max_tokens: 262_144,
                max_string_utf8_bytes: megabyte,
            },
            request: NodeDocumentLimitsProjection {
                max_utf8_bytes: 10 * megabyte,
                max_depth: 8,
                max_members: 16,
                max_tokens: 64,
                max_string_utf8_bytes: 8 * megabyte,
            },
            runtime_catalog: NodeDocumentLimitsProjection {
                max_utf8_bytes: megabyte,
                max_depth: 64,
                max_members: 65_536,
                max_tokens: 262_144,
                max_string_utf8_bytes: megabyte,
            },
            response: NodeDocumentLimitsProjection {
                max_utf8_bytes: 26 * megabyte,
                max_depth: 8,
                max_members: 32,
                max_tokens: 128,
                max_string_utf8_bytes: 16 * megabyte,
            },
            error: NodeDocumentLimitsProjection {
                max_utf8_bytes: 256 * 1024,
                max_depth: 8,
                max_members: 32,
                max_tokens: 128,
                max_string_utf8_bytes: 64 * 1024,
            },
            metadata: NodeDocumentLimitsProjection {
                max_utf8_bytes: 8 * megabyte,
                max_depth: 64,
                max_members: 131_072,
                max_tokens: 524_288,
                max_string_utf8_bytes: 8 * megabyte,
            },
        },
        fields: NodeFieldLimitsProjection {
            operation_id_utf8_bytes: 128,
            media_type_utf8_bytes: 128,
            metadata_id_utf8_bytes: 128,
            uri_utf8_bytes: 16 * 1024,
            source_utf8_bytes: 8 * megabyte,
            options_json_utf8_bytes: megabyte,
            data_utf8_bytes: 16 * megabyte,
            metadata_json_utf8_bytes: 8 * megabyte,
            error_code_name_utf8_bytes: 128,
            error_kind_utf8_bytes: 128,
            error_message_utf8_bytes: 64 * 1024,
            capability_id_utf8_bytes: 128,
            package_version_utf8_bytes: 128,
            contract_digest_utf8_bytes: 128,
        },
    };
    format!("{}\n", pretty_json(&contract))
}

fn binding_registry_projections() -> BindingRegistryProjections {
    let capabilities = CapabilityKey::ALL
        .iter()
        .copied()
        .map(|key| CapabilityProjection {
            id: key.id(),
            implication_ids: key
                .spec()
                .implications
                .iter()
                .map(|capability| capability.id())
                .collect(),
        })
        .collect::<Vec<_>>();
    let metadata = MetadataKey::ALL
        .iter()
        .copied()
        .map(|key| MetadataProjection {
            id: key.id(),
            required_capability_id: key
                .spec()
                .required_capability()
                .map(|capability| capability.id()),
        })
        .collect::<Vec<_>>();
    let option_groups = BindingOptionGroupKey::ALL
        .iter()
        .copied()
        .map(|key| {
            let spec = key.spec();
            OptionGroupProjection {
                id: key.id(),
                always_available: spec.always_available(),
                any_capability_ids: spec
                    .any_capabilities()
                    .iter()
                    .map(|capability| capability.id())
                    .collect(),
                requires_svg_pipeline: spec.requires_svg_pipeline(),
            }
        })
        .collect::<Vec<_>>();
    let payload_schemas = BindingPayloadSchemaKey::ALL
        .iter()
        .copied()
        .map(|key| PayloadSchemaProjection {
            id: key.id(),
            version: key.version(),
        })
        .collect::<Vec<_>>();
    let providers = TextMeasurementProviderKey::ALL
        .iter()
        .copied()
        .map(|key| {
            let provider_source = key.source();
            TextMeasurementProviderProjection {
                id: key.id(),
                source: provider_source.id(),
                constructor_service_id: provider_source
                    .constructor_service()
                    .map(ConstructorServiceKey::id),
            }
        })
        .collect::<Vec<_>>();
    let provider_ids = providers
        .iter()
        .map(|provider| provider.id)
        .collect::<Vec<_>>();
    let constructor_services = ConstructorServiceKey::ALL
        .iter()
        .copied()
        .map(|key| ConstructorServiceProjection {
            id: key.id(),
            requires_svg_pipeline: key.requires_svg_pipeline(),
            provided_text_measurement_provider_ids: providers
                .iter()
                .filter(|provider| provider.constructor_service_id == Some(key.id()))
                .map(|provider| provider.id)
                .collect(),
            resource_limits: runtime_constructor_resource_limits(key),
        })
        .collect::<Vec<_>>();
    let transports = BindingTransportKey::ALL
        .iter()
        .copied()
        .map(|key| TransportExposureProjection {
            id: key.id(),
            payload_schema_ids: key
                .spec()
                .payload_schemas()
                .iter()
                .map(|schema| schema.id())
                .collect(),
            constructor_service_candidate_ids: key
                .spec()
                .constructor_service_candidates()
                .iter()
                .map(|service| service.id())
                .collect(),
        })
        .collect::<Vec<_>>();

    BindingRegistryProjections {
        capabilities,
        metadata,
        option_groups,
        payload_schemas,
        providers,
        provider_ids,
        constructor_services,
        transports,
    }
}

fn render_node_javascript() -> String {
    let projections = binding_registry_projections();
    let operation_expectations = operation_expectation_projections();

    let mut out = String::from(
        "// @generated by `cargo run -p xtask -- gen-binding-contract`.\n// Sources: typed registries in merman-bindings-core. Do not edit directly.\n\n",
    );
    writeln!(
        out,
        "export const RUNTIME_CATALOG_SCHEMA_VERSION = {RUNTIME_CATALOG_SCHEMA_VERSION};"
    )
    .unwrap();
    writeln!(
        out,
        "export const BINDING_OPTIONS_SCHEMA_VERSION = {BINDING_OPTIONS_SCHEMA_VERSION};"
    )
    .unwrap();
    writeln!(
        out,
        "export const TEXT_MEASUREMENT_PROTOCOL_VERSION = {TEXT_MEASUREMENT_PROTOCOL_VERSION};\n"
    )
    .unwrap();
    writeln!(
        out,
        "export const RUNTIME_CATALOG_IDENTIFIER_PATTERN = {RUNTIME_CATALOG_IDENTIFIER_PATTERN:?};"
    )
    .unwrap();
    writeln!(
        out,
        "export const RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN = {RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN:?};"
    )
    .unwrap();
    writeln!(
        out,
        "export const RUNTIME_CATALOG_MAX_SAFE_INTEGER = {RUNTIME_CATALOG_MAX_SAFE_INTEGER};\n"
    )
    .unwrap();
    writeln!(
        out,
        "export const CAPABILITY_SPECS = {};\n",
        pretty_json(&projections.capabilities)
    )
    .unwrap();
    writeln!(
        out,
        "export const BINDING_OPERATION_METADATA_CONTRACT = {};\n",
        pretty_json(operation_metadata_contract())
    )
    .unwrap();
    writeln!(
        out,
        "export const BINDING_OPERATION_EXPECTATIONS = {};\n",
        pretty_json(&operation_expectations)
    )
    .unwrap();
    writeln!(
        out,
        "export const BINDING_PAYLOAD_SCHEMAS = {};\n",
        pretty_json(&projections.payload_schemas)
    )
    .unwrap();
    writeln!(
        out,
        "export const METADATA_SPECS = {};\n",
        pretty_json(&projections.metadata)
    )
    .unwrap();
    writeln!(
        out,
        "export const BINDING_OPTION_GROUP_SPECS = {};\n",
        pretty_json(&projections.option_groups)
    )
    .unwrap();
    writeln!(
        out,
        "export const TEXT_MEASUREMENT_PROVIDER_SPECS = {};\n",
        pretty_json(&projections.providers)
    )
    .unwrap();
    writeln!(
        out,
        "export const TEXT_MEASUREMENT_PROVIDER_IDS = {};",
        pretty_json(&projections.provider_ids)
    )
    .unwrap();
    writeln!(
        out,
        "export const VENDORED_TEXT_MEASUREMENT_PROVIDER_ID = {:?};",
        TextMeasurementProviderKey::Vendored.id()
    )
    .unwrap();
    writeln!(
        out,
        "export const HOST_CALLBACK_TEXT_MEASUREMENT_PROVIDER_ID = {:?};\n",
        TextMeasurementProviderKey::HostCallback.id()
    )
    .unwrap();
    writeln!(
        out,
        "export const CONSTRUCTOR_SERVICE_SPECS = {};",
        pretty_json(&projections.constructor_services)
    )
    .unwrap();
    writeln!(
        out,
        "export const BINDING_TRANSPORT_EXPOSURE_SPECS = {};",
        pretty_json(&projections.transports)
    )
    .unwrap();
    writeln!(
        out,
        "export const HOST_TEXT_MEASUREMENT_CONSTRUCTOR_SERVICE_ID = {:?};",
        ConstructorServiceKey::HostTextMeasurement.id()
    )
    .unwrap();
    writeln!(
        out,
        "export const ICON_REGISTRY_CONSTRUCTOR_SERVICE_ID = {:?};",
        ConstructorServiceKey::IconRegistry.id()
    )
    .unwrap();
    out
}

fn render_web_typescript() -> String {
    let projections = binding_registry_projections();
    let mut out = String::from(
        "// @generated by `cargo run -p xtask -- gen-binding-contract`.\n// Sources: typed registries in merman-bindings-core. Do not edit directly.\n\n",
    );
    writeln!(
        out,
        "export const RUNTIME_CATALOG_SCHEMA_VERSION = {RUNTIME_CATALOG_SCHEMA_VERSION} as const;"
    )
    .unwrap();
    writeln!(
        out,
        "export const BINDING_OPTIONS_SCHEMA_VERSION = {BINDING_OPTIONS_SCHEMA_VERSION} as const;"
    )
    .unwrap();
    writeln!(
        out,
        "export const TEXT_MEASUREMENT_PROTOCOL_VERSION = {TEXT_MEASUREMENT_PROTOCOL_VERSION} as const;\n"
    )
    .unwrap();
    writeln!(
        out,
        "export const RUNTIME_CATALOG_IDENTIFIER_PATTERN = {RUNTIME_CATALOG_IDENTIFIER_PATTERN:?} as const;"
    )
    .unwrap();
    writeln!(
        out,
        "export const RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN = {RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN:?} as const;"
    )
    .unwrap();
    writeln!(
        out,
        "export const RUNTIME_CATALOG_MAX_SAFE_INTEGER = {RUNTIME_CATALOG_MAX_SAFE_INTEGER} as const;\n"
    )
    .unwrap();
    writeln!(
        out,
        "export const CAPABILITY_SPECS = {} as const;\n",
        pretty_json(&projections.capabilities)
    )
    .unwrap();
    writeln!(
        out,
        "export const BINDING_PAYLOAD_SCHEMAS = {} as const;\n",
        pretty_json(&projections.payload_schemas)
    )
    .unwrap();
    writeln!(
        out,
        "export const METADATA_SPECS = {} as const;\n",
        pretty_json(&projections.metadata)
    )
    .unwrap();
    writeln!(
        out,
        "export const BINDING_OPTION_GROUP_SPECS = {} as const;\n",
        pretty_json(&projections.option_groups)
    )
    .unwrap();
    writeln!(
        out,
        "export const TEXT_MEASUREMENT_PROVIDER_SPECS = {} as const;\n",
        pretty_json(&projections.providers)
    )
    .unwrap();
    writeln!(
        out,
        "export const TEXT_MEASUREMENT_PROVIDER_IDS = {} as const;",
        pretty_json(&projections.provider_ids)
    )
    .unwrap();
    writeln!(
        out,
        "export const VENDORED_TEXT_MEASUREMENT_PROVIDER_ID = {:?} as const;",
        TextMeasurementProviderKey::Vendored.id()
    )
    .unwrap();
    writeln!(
        out,
        "export const HOST_CALLBACK_TEXT_MEASUREMENT_PROVIDER_ID = {:?} as const;\n",
        TextMeasurementProviderKey::HostCallback.id()
    )
    .unwrap();
    writeln!(
        out,
        "export const CONSTRUCTOR_SERVICE_SPECS = {} as const;",
        pretty_json(&projections.constructor_services)
    )
    .unwrap();
    writeln!(
        out,
        "export const BINDING_TRANSPORT_EXPOSURE_SPECS = {} as const;",
        pretty_json(&projections.transports)
    )
    .unwrap();
    writeln!(
        out,
        "export const HOST_TEXT_MEASUREMENT_CONSTRUCTOR_SERVICE_ID = {:?} as const;",
        ConstructorServiceKey::HostTextMeasurement.id()
    )
    .unwrap();
    writeln!(
        out,
        "export const ICON_REGISTRY_CONSTRUCTOR_SERVICE_ID = {:?} as const;",
        ConstructorServiceKey::IconRegistry.id()
    )
    .unwrap();
    out
}

fn render_shared_operation_json() -> String {
    let mut json = pretty_json(&shared_operation_contract_projection());
    json.push('\n');
    json
}

fn render_kotlin() -> String {
    let projections = binding_registry_projections();
    let operation_expectations = operation_expectation_projections();
    let android_artifact = android_artifact_profile_projection();
    let mut out = String::from(
        "// This file is @generated by `cargo run -p xtask -- gen-binding-contract`.\n// Sources: typed registries in merman-bindings-core. Do not edit directly.\n\npackage io.merman\n\nimport org.json.JSONObject\n\n",
    );
    writeln!(
        out,
        "internal const val MERMAN_RUNTIME_CATALOG_SCHEMA_VERSION: Int = {RUNTIME_CATALOG_SCHEMA_VERSION}"
    )
    .unwrap();
    writeln!(
        out,
        "internal const val MERMAN_RUNTIME_CATALOG_IDENTIFIER_PATTERN: String = \"{}\\$\"",
        RUNTIME_CATALOG_IDENTIFIER_PATTERN
            .strip_suffix('$')
            .expect("runtime identifier pattern must end with an anchor")
    )
    .unwrap();
    writeln!(
        out,
        "internal const val MERMAN_RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN: String = \"{}\\$\"",
        RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN
            .strip_suffix('$')
            .expect("runtime field identifier pattern must end with an anchor")
    )
    .unwrap();
    out.push_str(
        "internal val MERMAN_RUNTIME_CATALOG_IDENTIFIER_REGEX =\n    Regex(MERMAN_RUNTIME_CATALOG_IDENTIFIER_PATTERN)\ninternal val MERMAN_RUNTIME_CATALOG_FIELD_IDENTIFIER_REGEX =\n    Regex(MERMAN_RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN)\n",
    );
    writeln!(
        out,
        "\ninternal const val MERMAN_BINDING_CONTRACT_OPTIONS_SCHEMA_VERSION: Int = {BINDING_OPTIONS_SCHEMA_VERSION}"
    )
    .unwrap();
    writeln!(
        out,
        "internal const val MERMAN_OPERATION_METADATA_SCHEMA_VERSION: Int = {}",
        operation_metadata_contract().metadata_schema_version()
    )
    .unwrap();
    writeln!(
        out,
        "internal const val MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION: Int = {TEXT_MEASUREMENT_PROTOCOL_VERSION}\n"
    )
    .unwrap();

    out.push_str(
        "internal data class MermanBindingCapabilitySpec(\n    val id: String,\n    val implicationIds: List<String>,\n)\n\n",
    );
    out.push_str(
        "internal data class MermanBindingMetadataSpec(\n    val id: String,\n    val requiredCapabilityId: String?,\n)\n\n",
    );
    out.push_str(
        "internal data class MermanBindingOptionGroupSpec(\n    val id: String,\n    val alwaysAvailable: Boolean,\n    val anyCapabilityIds: Set<String>,\n    val requiresSvgPipeline: Boolean,\n)\n\n",
    );
    out.push_str(
        "internal data class MermanBindingTransportExposureSpec(\n    val id: String,\n    val payloadSchemaIds: Set<String>,\n    val constructorServiceCandidateIds: Set<String>,\n)\n\n",
    );
    out.push_str(
        "internal data class MermanBindingConstructorResourceLimitSpec(\n    val id: String,\n    val phase: String,\n    val unit: String,\n    val description: String,\n    val value: Long,\n)\n\n",
    );
    out.push_str(
        "internal data class MermanBindingConstructorServiceSpec(\n    val id: String,\n    val requiresSvgPipeline: Boolean,\n    val providedTextMeasurementProviderIds: Set<String>,\n    val resourceLimits: List<MermanBindingConstructorResourceLimitSpec>,\n)\n\n",
    );
    out.push_str(
        "internal data class MermanBindingOperationExpectation(\n    val operationId: String,\n    val outputId: String?,\n    val mediaType: String,\n    val metadataSchemaVersion: Int,\n    val requiresUri: Boolean,\n    val availabilityCapabilityId: String?,\n)\n\n",
    );
    out.push_str(
        "internal data class MermanBindingArtifactExpectation(\n    val capabilityIds: List<String>,\n    val outputIds: List<String>,\n    val systemAdapterIds: List<String>,\n    val operationIds: List<String>,\n    val metadataIds: List<String>,\n)\n\n",
    );

    out.push_str("internal object MermanBindingOperationId {\n");
    for expectation in &operation_expectations {
        writeln!(
            out,
            "    internal const val {}: String = {:?}",
            upper_snake(expectation.operation_id),
            expectation.operation_id,
        )
        .unwrap();
    }
    out.push_str("}\n\n");

    out.push_str("internal object MermanBindingMetadataId {\n");
    for spec in &projections.metadata {
        writeln!(
            out,
            "    internal const val {}: String = {:?}",
            upper_snake(spec.id),
            spec.id,
        )
        .unwrap();
    }
    out.push_str("}\n\n");

    out.push_str("internal val MERMAN_BINDING_CAPABILITY_SPECS: Map<String, MermanBindingCapabilitySpec> = listOf(\n");
    for spec in &projections.capabilities {
        writeln!(
            out,
            "    MermanBindingCapabilitySpec({:?}, listOf({})),",
            spec.id,
            kotlin_string_arguments(&spec.implication_ids),
        )
        .unwrap();
    }
    out.push_str(").associateBy(MermanBindingCapabilitySpec::id)\n\n");

    out.push_str("internal val MERMAN_BINDING_METADATA_SPECS: Map<String, MermanBindingMetadataSpec> = listOf(\n");
    for spec in &projections.metadata {
        let required = spec
            .required_capability_id
            .map_or_else(|| "null".to_owned(), |id| format!("{id:?}"));
        writeln!(
            out,
            "    MermanBindingMetadataSpec({:?}, {}),",
            spec.id, required,
        )
        .unwrap();
    }
    out.push_str(").associateBy(MermanBindingMetadataSpec::id)\n\n");

    out.push_str(
        "internal val MERMAN_REQUIRED_PAYLOAD_SCHEMA_VERSIONS: Map<String, Int> = mapOf(\n",
    );
    for schema in &projections.payload_schemas {
        writeln!(out, "    {:?} to {},", schema.id, schema.version).unwrap();
    }
    out.push_str(")\n\n");

    out.push_str("internal val MERMAN_BINDING_OPTION_GROUP_SPECS: Map<String, MermanBindingOptionGroupSpec> = listOf(\n");
    for spec in &projections.option_groups {
        writeln!(
            out,
            "    MermanBindingOptionGroupSpec({:?}, {}, setOf({}), {}),",
            spec.id,
            spec.always_available,
            spec.any_capability_ids
                .iter()
                .map(|id| format!("{id:?}"))
                .collect::<Vec<_>>()
                .join(", "),
            spec.requires_svg_pipeline,
        )
        .unwrap();
    }
    out.push_str(").associateBy(MermanBindingOptionGroupSpec::id)\n\n");

    out.push_str("internal val MERMAN_BINDING_TRANSPORT_EXPOSURE_SPECS: Map<String, MermanBindingTransportExposureSpec> = listOf(\n");
    for spec in &projections.transports {
        writeln!(
            out,
            "    MermanBindingTransportExposureSpec({:?}, setOf({}), setOf({})),",
            spec.id,
            spec.payload_schema_ids
                .iter()
                .map(|id| format!("{id:?}"))
                .collect::<Vec<_>>()
                .join(", "),
            spec.constructor_service_candidate_ids
                .iter()
                .map(|id| format!("{id:?}"))
                .collect::<Vec<_>>()
                .join(", "),
        )
        .unwrap();
    }
    out.push_str(").associateBy(MermanBindingTransportExposureSpec::id)\n\n");

    out.push_str("internal val MERMAN_BINDING_CONSTRUCTOR_SERVICE_SPECS: Map<String, MermanBindingConstructorServiceSpec> = listOf(\n");
    for spec in &projections.constructor_services {
        let resource_limits = spec
            .resource_limits
            .iter()
            .map(|limit| {
                format!(
                    "MermanBindingConstructorResourceLimitSpec({:?}, {:?}, {:?}, {:?}, {}L)",
                    limit.id, limit.phase, limit.unit, limit.description, limit.value,
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            out,
            "    MermanBindingConstructorServiceSpec({:?}, {}, setOf({}), listOf({})),",
            spec.id,
            spec.requires_svg_pipeline,
            spec.provided_text_measurement_provider_ids
                .iter()
                .map(|id| format!("{id:?}"))
                .collect::<Vec<_>>()
                .join(", "),
            resource_limits,
        )
        .unwrap();
    }
    out.push_str(").associateBy(MermanBindingConstructorServiceSpec::id)\n\n");

    out.push_str("internal val MERMAN_BINDING_OPERATION_EXPECTATIONS: List<MermanBindingOperationExpectation> = listOf(\n");
    for expectation in &operation_expectations {
        let output = expectation
            .output_id
            .map_or_else(|| "null".to_owned(), |id| format!("{id:?}"));
        let availability = expectation
            .availability_capability_id
            .map_or_else(|| "null".to_owned(), |id| format!("{id:?}"));
        writeln!(
            out,
            "    MermanBindingOperationExpectation({:?}, {}, {:?}, {}, {}, {}),",
            expectation.operation_id,
            output,
            expectation.media_type,
            expectation.metadata_schema_version,
            expectation.requires_uri,
            availability,
        )
        .unwrap();
    }
    out.push_str(")\n\n");

    writeln!(
        out,
        "internal val MERMAN_ANDROID_ARTIFACT_EXPECTATION = MermanBindingArtifactExpectation(\n    capabilityIds = listOf({}),\n    outputIds = listOf({}),\n    systemAdapterIds = listOf({}),\n    operationIds = listOf({}),\n    metadataIds = listOf({}),\n)\n",
        kotlin_string_arguments(&android_artifact.capability_ids),
        kotlin_string_arguments(&android_artifact.output_ids),
        kotlin_string_arguments(&android_artifact.system_adapter_ids),
        kotlin_string_arguments(&android_artifact.operation_ids),
        kotlin_string_arguments(&android_artifact.metadata_ids),
    )
    .unwrap();
    out.push_str(
        "internal val MERMAN_ANDROID_OUTPUT_CONTRACT_JSON_BY_ID: Map<String, String> = mapOf(\n",
    );
    for contract in &android_artifact.output_contracts {
        let json = serde_json::to_string(contract)
            .expect("Android output contract projection must serialize");
        writeln!(out, "    {:?} to {:?},", contract.id, json).unwrap();
    }
    out.push_str(")\n\n");

    out.push_str(
        r#"internal fun decodeMermanOperationMetadata(json: String): MermanOperationMetadata {
    val metadata = try {
        JSONObject(json)
    } catch (error: Exception) {
        throw MermanException("Invalid Merman operation metadata: ${error.message}")
    }
    val version = metadata.requiredGeneratedInt("version")
    if (version != MERMAN_OPERATION_METADATA_SCHEMA_VERSION) {
        throw MermanException(
            "Unsupported Merman operation metadata schema $version; expected " +
                MERMAN_OPERATION_METADATA_SCHEMA_VERSION,
        )
    }
    val outputPlan = when (val raw = metadata.opt("output_plan")) {
        null, JSONObject.NULL -> null
        is JSONObject -> decodeMermanOutputPlan(raw)
        else -> throw MermanException(
            "Merman operation metadata field `output_plan` must be a JSON object",
        )
    }
    return MermanOperationMetadata(
        version = version,
        operationId = metadata.requiredGeneratedString("operation_id"),
        mediaType = metadata.requiredGeneratedString("media_type"),
        runtimePolicy = metadata.requiredGeneratedString("runtime_policy"),
        byteLength = metadata.requiredGeneratedLong("byte_length"),
        outputPlan = outputPlan,
        rawJson = json,
    )
}

private fun decodeMermanOutputPlan(plan: JSONObject): MermanOutputPlan {
    val kind = plan.requiredGeneratedString("kind")
    return when (kind) {
        "raster" -> MermanRasterOutputPlan(
            requestedWidthPx = plan.requiredGeneratedDouble("requested_width_px"),
            requestedHeightPx = plan.requiredGeneratedDouble("requested_height_px"),
            widthPx = plan.requiredGeneratedInt("width_px"),
            heightPx = plan.requiredGeneratedInt("height_px"),
            requestedScale = plan.requiredGeneratedDouble("requested_scale"),
            effectiveScale = plan.requiredGeneratedDouble("effective_scale"),
            limited = plan.requiredGeneratedBoolean("limited"),
        )
        "pdf-filter-images" -> MermanPdfFilterImagesOutputPlan(
            filteredGroups = plan.requiredGeneratedLong("filtered_groups"),
            requestedScale = plan.requiredGeneratedDouble("requested_scale"),
            effectiveScale = plan.requiredGeneratedDouble("effective_scale"),
            requestedImagePixels = plan.requiredGeneratedLong("requested_image_pixels"),
            effectiveImagePixels = plan.requiredGeneratedLong("effective_image_pixels"),
            limited = plan.requiredGeneratedBoolean("limited"),
        )
        else -> MermanUnknownOutputPlan(kind = kind, rawJson = plan.toString())
    }
}

private fun JSONObject.requiredGeneratedString(key: String): String {
    val value = opt(key)
    if (value !is String || value.isEmpty()) {
        throw MermanException("Merman operation metadata field `$key` must be a non-empty string")
    }
    return value
}

private fun JSONObject.requiredGeneratedLong(key: String): Long {
    val value = when (val raw = opt(key)) {
        is Int -> raw.toLong()
        is Long -> raw
        else -> throw MermanException(
            "Merman operation metadata field `$key` must be a JSON integer",
        )
    }
    if (value < 0L) {
        throw MermanException("Merman operation metadata field `$key` must be unsigned")
    }
    return value
}

private fun JSONObject.requiredGeneratedInt(key: String): Int {
    val value = requiredGeneratedLong(key)
    if (value > Int.MAX_VALUE.toLong()) {
        throw MermanException("Merman operation metadata field `$key` exceeds Int range")
    }
    return value.toInt()
}

private fun JSONObject.requiredGeneratedDouble(key: String): Double {
    val value = when (val raw = opt(key)) {
        is Number -> raw.toDouble()
        else -> throw MermanException(
            "Merman operation metadata field `$key` must be a JSON number",
        )
    }
    if (!value.isFinite()) {
        throw MermanException("Merman operation metadata field `$key` must be finite")
    }
    return value
}

private fun JSONObject.requiredGeneratedBoolean(key: String): Boolean =
    opt(key) as? Boolean ?: throw MermanException(
        "Merman operation metadata field `$key` must be a JSON boolean",
    )
"#,
    );
    out
}

fn write_dart_string_set_argument(out: &mut String, indent: &str, name: &str, ids: &[&str]) {
    if ids.is_empty() {
        writeln!(out, "{indent}{name}: <String>{{}},").unwrap();
        return;
    }

    writeln!(out, "{indent}{name}: <String>{{").unwrap();
    for id in ids {
        writeln!(out, "{indent}  {id:?},").unwrap();
    }
    writeln!(out, "{indent}}},").unwrap();
}

fn write_dart_string_argument(out: &mut String, indent: &str, name: &str, value: &str) {
    let literal = format!("{value:?}");
    let line = format!("{indent}{name}: {literal},");
    if line.chars().count() <= 80 {
        writeln!(out, "{line}").unwrap();
    } else {
        writeln!(out, "{indent}{name}:").unwrap();
        writeln!(out, "{indent}    {literal},").unwrap();
    }
}

fn render_dart() -> String {
    let projections = binding_registry_projections();
    let operation_expectations = operation_expectation_projections();
    let mut out = String::from(
        "// This file is @generated by `cargo run -p xtask -- gen-binding-contract`.\n// Sources: typed registries in merman-bindings-core. Do not edit directly.\n\nimport 'dart:convert';\n\nimport '../operation_metadata.dart';\n\n",
    );
    writeln!(
        out,
        "const int mermanRuntimeCatalogSchemaVersion = {RUNTIME_CATALOG_SCHEMA_VERSION};"
    )
    .unwrap();
    writeln!(
        out,
        "const String mermanRuntimeCatalogIdentifierPattern = r'{RUNTIME_CATALOG_IDENTIFIER_PATTERN}';"
    )
    .unwrap();
    writeln!(
        out,
        "const String mermanRuntimeCatalogFieldIdentifierPattern = r'{RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN}';"
    )
    .unwrap();
    writeln!(
        out,
        "const int mermanBindingOptionsContractSchemaVersion = {BINDING_OPTIONS_SCHEMA_VERSION};"
    )
    .unwrap();
    writeln!(
        out,
        "const int mermanOperationMetadataSchemaVersion = {};",
        operation_metadata_contract().metadata_schema_version()
    )
    .unwrap();
    writeln!(
        out,
        "const int mermanTextMeasurementContractProtocolVersion = {TEXT_MEASUREMENT_PROTOCOL_VERSION};"
    )
    .unwrap();
    writeln!(
        out,
        "const String mermanHostTextMeasurementConstructorServiceId =\n    {:?};",
        ConstructorServiceKey::HostTextMeasurement.id()
    )
    .unwrap();
    writeln!(
        out,
        "const String mermanIconRegistryConstructorServiceId = {:?};\n",
        ConstructorServiceKey::IconRegistry.id()
    )
    .unwrap();

    out.push_str(
        r#"final class MermanBindingCapabilitySpec {
  const MermanBindingCapabilitySpec({
    required this.id,
    required this.implicationIds,
  });

  final String id;
  final Set<String> implicationIds;
}

final class MermanBindingMetadataSpec {
  const MermanBindingMetadataSpec({
    required this.id,
    required this.requiredCapabilityId,
  });

  final String id;
  final String? requiredCapabilityId;
}

"#,
    );

    out.push_str("abstract final class MermanBindingMetadataId {\n");
    for spec in &projections.metadata {
        writeln!(
            out,
            "  static const String {} = {:?};",
            lower_camel(spec.id),
            spec.id,
        )
        .unwrap();
    }
    out.push_str("}\n\n");

    out.push_str(
        r#"final class MermanBindingOptionGroupSpec {
  const MermanBindingOptionGroupSpec({
    required this.id,
    required this.alwaysAvailable,
    required this.anyCapabilityIds,
    required this.requiresSvgPipeline,
  });

  final String id;
  final bool alwaysAvailable;
  final Set<String> anyCapabilityIds;
  final bool requiresSvgPipeline;
}

final class MermanBindingTransportExposureSpec {
  const MermanBindingTransportExposureSpec({
    required this.id,
    required this.payloadSchemaIds,
    required this.constructorServiceCandidateIds,
  });

  final String id;
  final Set<String> payloadSchemaIds;
  final Set<String> constructorServiceCandidateIds;
}

final class MermanBindingConstructorResourceLimitSpec {
  const MermanBindingConstructorResourceLimitSpec({
    required this.id,
    required this.phase,
    required this.unit,
    required this.description,
    required this.value,
  });

  final String id;
  final String phase;
  final String unit;
  final String description;
  final int value;
}

final class MermanBindingConstructorServiceSpec {
  const MermanBindingConstructorServiceSpec({
    required this.id,
    required this.requiresSvgPipeline,
    required this.providedTextMeasurementProviderIds,
    required this.resourceLimits,
  });

  final String id;
  final bool requiresSvgPipeline;
  final Set<String> providedTextMeasurementProviderIds;
  final List<MermanBindingConstructorResourceLimitSpec> resourceLimits;
}

final class MermanBindingOperationExpectation {
  const MermanBindingOperationExpectation({
    required this.operationId,
    required this.outputId,
    required this.mediaType,
    required this.metadataSchemaVersion,
    required this.requiresUri,
    required this.availabilityCapabilityId,
  });

  final String operationId;
  final String? outputId;
  final String mediaType;
  final int metadataSchemaVersion;
  final bool requiresUri;
  final String? availabilityCapabilityId;
}

"#,
    );

    out.push_str(
        "const Map<String, MermanBindingCapabilitySpec> mermanBindingCapabilitySpecs =\n    <String, MermanBindingCapabilitySpec>{\n",
    );
    for spec in &projections.capabilities {
        writeln!(out, "  {:?}: MermanBindingCapabilitySpec(", spec.id).unwrap();
        writeln!(out, "    id: {:?},", spec.id).unwrap();
        write_dart_string_set_argument(&mut out, "    ", "implicationIds", &spec.implication_ids);
        out.push_str("  ),\n");
    }
    out.push_str("};\n\n");

    out.push_str(
        "const Map<String, MermanBindingMetadataSpec> mermanBindingMetadataSpecs =\n    <String, MermanBindingMetadataSpec>{\n",
    );
    for spec in &projections.metadata {
        let required = spec
            .required_capability_id
            .map_or_else(|| "null".to_owned(), |id| format!("{id:?}"));
        writeln!(out, "  {:?}: MermanBindingMetadataSpec(", spec.id).unwrap();
        writeln!(out, "    id: {:?},", spec.id).unwrap();
        writeln!(out, "    requiredCapabilityId: {required},").unwrap();
        out.push_str("  ),\n");
    }
    out.push_str("};\n\n");

    out.push_str("const Map<String, int> mermanRequiredPayloadSchemaVersions = <String, int>{\n");
    for schema in &projections.payload_schemas {
        writeln!(out, "  {:?}: {},", schema.id, schema.version).unwrap();
    }
    out.push_str("};\n\n");

    out.push_str(
        "const Map<String, MermanBindingConstructorServiceSpec>\n    mermanBindingConstructorServiceSpecs =\n    <String, MermanBindingConstructorServiceSpec>{\n",
    );
    for spec in &projections.constructor_services {
        writeln!(out, "  {:?}: MermanBindingConstructorServiceSpec(", spec.id).unwrap();
        writeln!(out, "    id: {:?},", spec.id).unwrap();
        writeln!(
            out,
            "    requiresSvgPipeline: {},",
            spec.requires_svg_pipeline
        )
        .unwrap();
        write_dart_string_set_argument(
            &mut out,
            "    ",
            "providedTextMeasurementProviderIds",
            &spec.provided_text_measurement_provider_ids,
        );
        if spec.resource_limits.is_empty() {
            out.push_str("    resourceLimits: <MermanBindingConstructorResourceLimitSpec>[],\n");
        } else {
            out.push_str("    resourceLimits: <MermanBindingConstructorResourceLimitSpec>[\n");
            for limit in &spec.resource_limits {
                out.push_str("      MermanBindingConstructorResourceLimitSpec(\n");
                writeln!(out, "        id: {:?},", limit.id).unwrap();
                writeln!(out, "        phase: {:?},", limit.phase).unwrap();
                writeln!(out, "        unit: {:?},", limit.unit).unwrap();
                write_dart_string_argument(&mut out, "        ", "description", limit.description);
                writeln!(out, "        value: {},", limit.value).unwrap();
                out.push_str("      ),\n");
            }
            out.push_str("    ],\n");
        }
        out.push_str("  ),\n");
    }
    out.push_str("};\n\n");

    out.push_str(
        "const Map<String, MermanBindingOptionGroupSpec> mermanBindingOptionGroupSpecs =\n    <String, MermanBindingOptionGroupSpec>{\n",
    );
    for spec in &projections.option_groups {
        writeln!(out, "  {:?}: MermanBindingOptionGroupSpec(", spec.id).unwrap();
        writeln!(out, "    id: {:?},", spec.id).unwrap();
        writeln!(out, "    alwaysAvailable: {},", spec.always_available).unwrap();
        write_dart_string_set_argument(
            &mut out,
            "    ",
            "anyCapabilityIds",
            &spec.any_capability_ids,
        );
        writeln!(
            out,
            "    requiresSvgPipeline: {},",
            spec.requires_svg_pipeline
        )
        .unwrap();
        out.push_str("  ),\n");
    }
    out.push_str("};\n\n");

    out.push_str(
        "const Map<String, MermanBindingTransportExposureSpec>\n    mermanBindingTransportExposureSpecs =\n    <String, MermanBindingTransportExposureSpec>{\n",
    );
    for spec in &projections.transports {
        writeln!(out, "  {:?}: MermanBindingTransportExposureSpec(", spec.id).unwrap();
        writeln!(out, "    id: {:?},", spec.id).unwrap();
        write_dart_string_set_argument(
            &mut out,
            "    ",
            "payloadSchemaIds",
            &spec.payload_schema_ids,
        );
        write_dart_string_set_argument(
            &mut out,
            "    ",
            "constructorServiceCandidateIds",
            &spec.constructor_service_candidate_ids,
        );
        out.push_str("  ),\n");
    }
    out.push_str("};\n\n");

    out.push_str(
        "const List<MermanBindingOperationExpectation>\n    mermanBindingOperationExpectations = <MermanBindingOperationExpectation>[\n",
    );
    for expectation in &operation_expectations {
        let availability = expectation
            .availability_capability_id
            .map_or_else(|| "null".to_owned(), |id| format!("{id:?}"));
        out.push_str("  MermanBindingOperationExpectation(\n");
        writeln!(out, "    operationId: {:?},", expectation.operation_id).unwrap();
        let output = expectation
            .output_id
            .map_or_else(|| "null".to_owned(), |id| format!("{id:?}"));
        writeln!(out, "    outputId: {output},").unwrap();
        writeln!(out, "    mediaType: {:?},", expectation.media_type).unwrap();
        writeln!(
            out,
            "    metadataSchemaVersion: {},",
            expectation.metadata_schema_version
        )
        .unwrap();
        writeln!(out, "    requiresUri: {},", expectation.requires_uri).unwrap();
        writeln!(out, "    availabilityCapabilityId: {availability},").unwrap();
        out.push_str("  ),\n");
    }
    out.push_str("];\n\n");

    out.push_str(
        r#"MermanOperationMetadata decodeMermanOperationMetadata(String rawJson) {
  final Object? decoded;
  try {
    decoded = jsonDecode(rawJson);
  } on FormatException catch (error) {
    throw FormatException(
      'invalid Merman operation metadata: ${error.message}',
    );
  }
  final metadata = _requiredGeneratedObject(decoded, 'operation metadata');
  final version = _requiredGeneratedUint32(metadata, 'version');
  if (version != mermanOperationMetadataSchemaVersion) {
    throw FormatException(
      'unsupported Merman operation metadata schema $version; expected '
      '$mermanOperationMetadataSchemaVersion',
    );
  }
  final Object? rawPlan = metadata['output_plan'];
  final MermanOutputPlan? outputPlan = rawPlan == null
      ? null
      : _decodeMermanOutputPlan(
          _requiredGeneratedObject(rawPlan, 'operation metadata output_plan'),
        );
  return MermanOperationMetadata(
    version: version,
    operationId: _requiredGeneratedString(metadata, 'operation_id'),
    mediaType: _requiredGeneratedString(metadata, 'media_type'),
    runtimePolicy: _requiredGeneratedString(metadata, 'runtime_policy'),
    byteLength: _requiredGeneratedUint64(metadata, 'byte_length'),
    outputPlan: outputPlan,
    rawJson: rawJson,
  );
}

MermanOutputPlan _decodeMermanOutputPlan(Map<String, Object?> plan) {
  final kind = _requiredGeneratedString(plan, 'kind');
  return switch (kind) {
    'raster' => MermanRasterOutputPlan(
        requestedWidthPx: _requiredGeneratedDouble(plan, 'requested_width_px'),
        requestedHeightPx:
            _requiredGeneratedDouble(plan, 'requested_height_px'),
        widthPx: _requiredGeneratedUint32(plan, 'width_px'),
        heightPx: _requiredGeneratedUint32(plan, 'height_px'),
        requestedScale: _requiredGeneratedDouble(plan, 'requested_scale'),
        effectiveScale: _requiredGeneratedDouble(plan, 'effective_scale'),
        limited: _requiredGeneratedBool(plan, 'limited'),
      ),
    'pdf-filter-images' => MermanPdfFilterImagesOutputPlan(
        filteredGroups: _requiredGeneratedUint64(plan, 'filtered_groups'),
        requestedScale: _requiredGeneratedDouble(plan, 'requested_scale'),
        effectiveScale: _requiredGeneratedDouble(plan, 'effective_scale'),
        requestedImagePixels:
            _requiredGeneratedUint64(plan, 'requested_image_pixels'),
        effectiveImagePixels:
            _requiredGeneratedUint64(plan, 'effective_image_pixels'),
        limited: _requiredGeneratedBool(plan, 'limited'),
      ),
    _ => MermanUnknownOutputPlan(kind: kind, rawJson: jsonEncode(plan)),
  };
}

Map<String, Object?> _requiredGeneratedObject(Object? value, String name) {
  if (value is! Map<Object?, Object?>) {
    throw FormatException('$name must be a JSON object');
  }
  final result = <String, Object?>{};
  for (final entry in value.entries) {
    final key = entry.key;
    if (key is! String) {
      throw FormatException('$name contains a non-string key');
    }
    result[key] = entry.value;
  }
  return result;
}

String _requiredGeneratedString(Map<String, Object?> value, String key) {
  final field = value[key];
  if (field is! String || field.isEmpty) {
    throw FormatException(
      'operation metadata field `$key` must be a non-empty string',
    );
  }
  return field;
}

int _requiredGeneratedUint32(Map<String, Object?> value, String key) {
  final field = _requiredGeneratedUint64(value, key);
  if (field > 0xffffffff) {
    throw FormatException(
      'operation metadata field `$key` exceeds unsigned 32-bit range',
    );
  }
  return field;
}

int _requiredGeneratedUint64(Map<String, Object?> value, String key) {
  final field = value[key];
  if (field is! int || field < 0) {
    throw FormatException(
      'operation metadata field `$key` must be an unsigned 64-bit JSON integer',
    );
  }
  return field;
}

double _requiredGeneratedDouble(Map<String, Object?> value, String key) {
  final field = value[key];
  if (field is! num) {
    throw FormatException(
      'operation metadata field `$key` must be a JSON number',
    );
  }
  final result = field.toDouble();
  if (!result.isFinite) {
    throw FormatException('operation metadata field `$key` must be finite');
  }
  return result;
}

bool _requiredGeneratedBool(Map<String, Object?> value, String key) {
  final field = value[key];
  if (field is! bool) {
    throw FormatException(
      'operation metadata field `$key` must be a JSON boolean',
    );
  }
  return field;
}
"#,
    );
    out
}

fn render_python() -> String {
    let projections = binding_registry_projections();
    let operation_expectations = operation_expectation_projections();
    let mut out = String::from(
        "# @generated by `cargo run -p xtask -- gen-binding-contract`.\n# Sources: typed registries in merman-bindings-core. Do not edit directly.\n\n",
    );
    writeln!(
        out,
        "RUNTIME_CATALOG_SCHEMA_VERSION = {RUNTIME_CATALOG_SCHEMA_VERSION}"
    )
    .unwrap();
    writeln!(
        out,
        "RUNTIME_CATALOG_IDENTIFIER_PATTERN = {:?}",
        RUNTIME_CATALOG_IDENTIFIER_PATTERN
    )
    .unwrap();
    writeln!(
        out,
        "RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN = {:?}",
        RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN
    )
    .unwrap();
    writeln!(
        out,
        "RUNTIME_CATALOG_MAX_SAFE_INTEGER = {}\n",
        RUNTIME_CATALOG_MAX_SAFE_INTEGER
    )
    .unwrap();
    out.push_str("CAPABILITY_SPECS = (\n");
    for spec in &projections.capabilities {
        out.push_str("    {\n");
        writeln!(out, "        \"id\": {:?},", spec.id).unwrap();
        out.push_str("        \"implication_ids\": (");
        for (index, capability_id) in spec.implication_ids.iter().enumerate() {
            if index != 0 {
                out.push_str(", ");
            }
            write!(out, "{capability_id:?}").unwrap();
        }
        if spec.implication_ids.len() == 1 {
            out.push(',');
        }
        out.push_str("),\n");
        out.push_str("    },\n");
    }
    out.push_str(")\n\n");
    out.push_str("REQUIRED_PAYLOAD_SCHEMA_VERSIONS = {\n");
    for key in BindingPayloadSchemaKey::ALL {
        writeln!(out, "    {:?}: {},", key.id(), key.version()).unwrap();
    }
    out.push_str("}\n\n");

    out.push_str("BINDING_OPERATION_RELATION_SPECS = (\n");
    for spec in &operation_expectations {
        out.push_str("    {\n");
        writeln!(out, "        \"operation_id\": {:?},", spec.operation_id).unwrap();
        match spec.availability_capability_id {
            Some(id) => writeln!(out, "        \"availability_capability_id\": {id:?},").unwrap(),
            None => out.push_str("        \"availability_capability_id\": None,\n"),
        }
        match spec.output_id {
            Some(id) => writeln!(out, "        \"output_id\": {id:?},").unwrap(),
            None => out.push_str("        \"output_id\": None,\n"),
        }
        out.push_str("        \"compiled_prerequisite_ids\": (");
        for (index, capability_id) in spec.compiled_prerequisite_ids.iter().enumerate() {
            if index != 0 {
                out.push_str(", ");
            }
            write!(out, "{capability_id:?}").unwrap();
        }
        if spec.compiled_prerequisite_ids.len() == 1 {
            out.push(',');
        }
        out.push_str("),\n");
        out.push_str("    },\n");
    }
    out.push_str(")\n\n");

    out.push_str("METADATA_SPECS = (\n");
    for spec in &projections.metadata {
        out.push_str("    {\n");
        writeln!(out, "        \"id\": {:?},", spec.id).unwrap();
        match spec.required_capability_id {
            Some(id) => writeln!(out, "        \"required_capability_id\": {id:?},").unwrap(),
            None => out.push_str("        \"required_capability_id\": None,\n"),
        }
        out.push_str("    },\n");
    }
    out.push_str(")\n\n");

    out.push_str("BINDING_OPTION_GROUP_SPECS = (\n");
    for spec in &projections.option_groups {
        out.push_str("    {\n");
        writeln!(out, "        \"id\": {:?},", spec.id).unwrap();
        writeln!(
            out,
            "        \"always_available\": {},",
            if spec.always_available {
                "True"
            } else {
                "False"
            }
        )
        .unwrap();
        out.push_str("        \"any_capability_ids\": (");
        for (index, capability_id) in spec.any_capability_ids.iter().enumerate() {
            if index != 0 {
                out.push_str(", ");
            }
            write!(out, "{capability_id:?}").unwrap();
        }
        if spec.any_capability_ids.len() == 1 {
            out.push(',');
        }
        out.push_str("),\n");
        writeln!(
            out,
            "        \"requires_svg_pipeline\": {},",
            if spec.requires_svg_pipeline {
                "True"
            } else {
                "False"
            }
        )
        .unwrap();
        out.push_str("    },\n");
    }
    out.push_str(")\n\n");

    out.push_str("TEXT_MEASUREMENT_PROVIDER_SPECS = (\n");
    for spec in &projections.providers {
        out.push_str("    {\n");
        writeln!(out, "        \"id\": {:?},", spec.id).unwrap();
        writeln!(out, "        \"source\": {:?},", spec.source).unwrap();
        match spec.constructor_service_id {
            Some(id) => writeln!(out, "        \"constructor_service_id\": {id:?},").unwrap(),
            None => out.push_str("        \"constructor_service_id\": None,\n"),
        }
        out.push_str("    },\n");
    }
    out.push_str(")\n\n");

    out.push_str("TEXT_MEASUREMENT_PROVIDER_IDS = (\n");
    for id in &projections.provider_ids {
        writeln!(out, "    {id:?},").unwrap();
    }
    out.push_str(")\n");
    out.push_str("\nBINDING_TRANSPORT_EXPOSURE_SPECS = (\n");
    for spec in &projections.transports {
        out.push_str("    {\n");
        writeln!(out, "        \"id\": {:?},", spec.id).unwrap();
        out.push_str("        \"payload_schema_ids\": (");
        for (index, id) in spec.payload_schema_ids.iter().enumerate() {
            if index != 0 {
                out.push_str(", ");
            }
            write!(out, "{id:?}").unwrap();
        }
        if spec.payload_schema_ids.len() == 1 {
            out.push(',');
        }
        out.push_str("),\n");
        out.push_str("        \"constructor_service_candidate_ids\": (");
        for (index, id) in spec.constructor_service_candidate_ids.iter().enumerate() {
            if index != 0 {
                out.push_str(", ");
            }
            write!(out, "{id:?}").unwrap();
        }
        if spec.constructor_service_candidate_ids.len() == 1 {
            out.push(',');
        }
        out.push_str("),\n");
        out.push_str("    },\n");
    }
    out.push_str(")\n");
    writeln!(
        out,
        "VENDORED_TEXT_MEASUREMENT_PROVIDER_ID = {:?}",
        TextMeasurementProviderKey::Vendored.id()
    )
    .unwrap();
    writeln!(
        out,
        "HOST_CALLBACK_TEXT_MEASUREMENT_PROVIDER_ID = {:?}\n",
        TextMeasurementProviderKey::HostCallback.id()
    )
    .unwrap();

    out.push_str("CONSTRUCTOR_SERVICE_SPECS = (\n");
    for spec in &projections.constructor_services {
        out.push_str("    {\n");
        writeln!(out, "        \"id\": {:?},", spec.id).unwrap();
        writeln!(
            out,
            "        \"requires_svg_pipeline\": {},",
            if spec.requires_svg_pipeline {
                "True"
            } else {
                "False"
            }
        )
        .unwrap();
        out.push_str("        \"provided_text_measurement_provider_ids\": (");
        for (index, provider_id) in spec
            .provided_text_measurement_provider_ids
            .iter()
            .enumerate()
        {
            if index != 0 {
                out.push_str(", ");
            }
            write!(out, "{provider_id:?}").unwrap();
        }
        if spec.provided_text_measurement_provider_ids.len() == 1 {
            out.push(',');
        }
        out.push_str("),\n");
        out.push_str("        \"resource_limits\": (\n");
        for limit in &spec.resource_limits {
            out.push_str("            {\n");
            writeln!(out, "                \"id\": {:?},", limit.id).unwrap();
            writeln!(out, "                \"phase\": {:?},", limit.phase).unwrap();
            writeln!(out, "                \"unit\": {:?},", limit.unit).unwrap();
            writeln!(
                out,
                "                \"description\": {:?},",
                limit.description,
            )
            .unwrap();
            writeln!(out, "                \"value\": {},", limit.value).unwrap();
            out.push_str("            },\n");
        }
        out.push_str("        ),\n");
        out.push_str("    },\n");
    }
    out.push_str(")\n");
    writeln!(
        out,
        "HOST_TEXT_MEASUREMENT_CONSTRUCTOR_SERVICE_ID = {:?}",
        ConstructorServiceKey::HostTextMeasurement.id()
    )
    .unwrap();
    writeln!(
        out,
        "ICON_REGISTRY_CONSTRUCTOR_SERVICE_ID = {:?}",
        ConstructorServiceKey::IconRegistry.id()
    )
    .unwrap();
    out
}

fn generated_artifacts() -> Vec<(PathBuf, String)> {
    let python = render_python();
    vec![
        (PathBuf::from(NODE_OUTPUT), render_node_javascript()),
        (
            PathBuf::from(NODE_WIRE_OUTPUT),
            render_node_wire_contract_json(),
        ),
        (PathBuf::from(WEB_OUTPUT), render_web_typescript()),
        (
            PathBuf::from(SHARED_OPERATION_OUTPUT),
            render_shared_operation_json(),
        ),
        (PathBuf::from(UNIFFI_PYTHON_OUTPUT), python.clone()),
        (PathBuf::from(PYTHON_OUTPUT), python),
        (PathBuf::from(KOTLIN_OUTPUT), render_kotlin()),
        (PathBuf::from(DART_OUTPUT), render_dart()),
    ]
}

fn write_artifact(root: &Path, path: &Path, contents: &str) -> Result<(), XtaskError> {
    let full = root.join(path);
    match fs::read_to_string(&full) {
        Ok(existing) if existing == contents => return Ok(()),
        Ok(_) => {}
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(XtaskError::ReadFile {
                path: full.display().to_string(),
                source,
            });
        }
    }
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&full, contents).map_err(|source| XtaskError::WriteFile {
        path: full.display().to_string(),
        source,
    })
}

pub(crate) fn gen_binding_contract(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() {
        return Err(XtaskError::Usage);
    }
    let root = crate::cmd::workspace_root();
    for (path, contents) in generated_artifacts() {
        write_artifact(&root, &path, &contents)?;
    }
    Ok(())
}

pub(crate) fn verify_binding_contract_artifacts() -> Result<Option<String>, XtaskError> {
    let root = crate::cmd::workspace_root();
    let mut drift = Vec::new();
    for (path, expected) in generated_artifacts() {
        let full = root.join(&path);
        let actual = fs::read_to_string(&full).map_err(|source| XtaskError::ReadFile {
            path: full.display().to_string(),
            source,
        })?;
        if actual.replace("\r\n", "\n") != expected {
            drift.push(path.display().to_string());
        }
    }
    if drift.is_empty() {
        Ok(None)
    } else {
        Ok(Some(format!(
            "binding contract projections drifted: {}; regenerate with `cargo run -p xtask -- gen-binding-contract`",
            drift.join(", ")
        )))
    }
}

pub(crate) fn verify_binding_contract(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() {
        return Err(XtaskError::Usage);
    }
    match verify_binding_contract_artifacts()? {
        Some(message) => Err(XtaskError::VerifyFailed(message)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_language_projections_are_sorted_and_use_typed_relations() {
        let metadata_ids = MetadataKey::ALL
            .iter()
            .map(|key| key.id())
            .collect::<Vec<_>>();
        assert!(metadata_ids.windows(2).all(|pair| pair[0] < pair[1]));

        let option_ids = BindingOptionGroupKey::ALL
            .iter()
            .map(|key| key.id())
            .collect::<Vec<_>>();
        assert!(option_ids.windows(2).all(|pair| pair[0] < pair[1]));

        for provider in TextMeasurementProviderKey::ALL {
            if let TextMeasurementProviderSource::ConstructorService(service) = provider.source() {
                assert!(ConstructorServiceKey::ALL.contains(&service));
            }
        }

        let generated = render_node_javascript();
        assert!(generated.contains("BINDING_OPERATION_METADATA_CONTRACT"));
        assert!(generated.contains("BINDING_OPERATION_EXPECTATIONS"));
        assert!(generated.contains("CAPABILITY_SPECS"));
        assert!(generated.contains("implication_ids"));
        assert!(generated.contains("RUNTIME_CATALOG_IDENTIFIER_PATTERN"));
        assert!(generated.contains("RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN"));
        assert!(generated.contains("RUNTIME_CATALOG_SCHEMA_VERSION"));
        assert!(generated.contains("RUNTIME_CATALOG_MAX_SAFE_INTEGER"));
        assert!(generated.contains("BINDING_OPTION_GROUP_SPECS"));
        assert!(generated.contains("METADATA_SPECS"));
        assert!(generated.contains("required_capability_id"));
        assert!(generated.contains("always_available"));
        assert!(generated.contains("CONSTRUCTOR_SERVICE_SPECS"));
        assert!(generated.contains("BINDING_TRANSPORT_EXPOSURE_SPECS"));
        assert!(generated.contains("constructor_service_candidate_ids"));
        assert!(generated.contains("BINDING_PAYLOAD_SCHEMAS"));
        assert!(generated.contains("TEXT_MEASUREMENT_PROVIDER_SPECS"));

        let wire_contract = render_node_wire_contract_json();
        assert!(wire_contract.contains("\"artifact_id\": \"merman-node-static-svg\""));
        assert!(wire_contract.contains("\"output_contracts\""));
    }

    #[test]
    fn python_projection_uses_the_typed_payload_schema_registry() {
        let generated = render_python();
        assert!(generated.contains("CAPABILITY_SPECS"));
        assert!(generated.contains("implication_ids"));
        assert!(generated.contains("RUNTIME_CATALOG_IDENTIFIER_PATTERN"));
        assert!(generated.contains("RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN"));
        assert!(generated.contains("RUNTIME_CATALOG_MAX_SAFE_INTEGER"));
        assert!(generated.contains("REQUIRED_PAYLOAD_SCHEMA_VERSIONS"));
        assert!(generated.contains("BINDING_OPERATION_RELATION_SPECS"));
        assert!(generated.contains("METADATA_SPECS"));
        assert!(generated.contains("BINDING_OPTION_GROUP_SPECS"));
        assert!(generated.contains("CONSTRUCTOR_SERVICE_SPECS"));
        assert!(generated.contains("BINDING_TRANSPORT_EXPOSURE_SPECS"));
        assert!(generated.contains("constructor_service_candidate_ids"));
        for key in BindingPayloadSchemaKey::ALL {
            assert!(generated.contains(&format!("{:?}: {}", key.id(), key.version())));
        }
    }

    #[test]
    fn web_projection_uses_immutable_typed_registry_constants() {
        let generated = render_web_typescript();
        assert!(generated.contains("export const CAPABILITY_SPECS ="));
        assert!(generated.contains("implication_ids"));
        assert!(generated.contains("RUNTIME_CATALOG_MAX_SAFE_INTEGER"));
        assert!(generated.contains("export const METADATA_SPECS ="));
        assert!(generated.contains("export const BINDING_OPTION_GROUP_SPECS ="));
        assert!(generated.contains("export const CONSTRUCTOR_SERVICE_SPECS ="));
        assert!(generated.contains("export const BINDING_TRANSPORT_EXPOSURE_SPECS ="));
        assert!(generated.contains("constructor_service_candidate_ids"));
        assert!(generated.contains("provided_text_measurement_provider_ids"));
        assert!(generated.contains(" as const;"));
    }

    #[test]
    fn artifact_list_includes_every_host_language_projection() {
        let paths = generated_artifacts()
            .into_iter()
            .map(|(path, _)| path)
            .collect::<Vec<_>>();
        assert!(paths.contains(&PathBuf::from(NODE_OUTPUT)));
        assert!(paths.contains(&PathBuf::from(WEB_OUTPUT)));
        assert!(paths.contains(&PathBuf::from(UNIFFI_PYTHON_OUTPUT)));
        assert!(paths.contains(&PathBuf::from(PYTHON_OUTPUT)));
        assert!(paths.contains(&PathBuf::from(KOTLIN_OUTPUT)));
        assert!(paths.contains(&PathBuf::from(DART_OUTPUT)));
    }

    #[test]
    fn kotlin_and_dart_projections_include_transport_and_metadata_contracts() {
        let kotlin = render_kotlin();
        assert!(kotlin.contains("MERMAN_BINDING_TRANSPORT_EXPOSURE_SPECS"));
        assert!(kotlin.contains("decodeMermanOperationMetadata"));
        assert!(kotlin.contains("MermanUnknownOutputPlan"));
        assert!(kotlin.contains("MERMAN_BINDING_OPERATION_EXPECTATIONS"));
        assert!(kotlin.contains("MermanBindingMetadataId"));
        assert!(
            kotlin
                .contains("internal const val SUPPORTED_DIAGRAMS: String = \"supported-diagrams\"")
        );

        let dart = render_dart();
        assert!(dart.contains("mermanBindingTransportExposureSpecs"));
        assert!(dart.contains("decodeMermanOperationMetadata"));
        assert!(dart.contains("MermanUnknownOutputPlan"));
        assert!(dart.contains("mermanBindingOperationExpectations"));
        assert!(dart.contains("abstract final class MermanBindingMetadataId"));
        assert!(dart.contains("static const String supportedDiagrams = \"supported-diagrams\";"));
        assert!(dart.contains(
            "const Map<String, MermanBindingCapabilitySpec> mermanBindingCapabilitySpecs =\n"
        ));
    }

    #[test]
    fn dart_projection_exposes_constructor_ids_and_enforces_integer_widths() {
        let dart = render_dart();
        assert!(dart.contains(
            "const String mermanHostTextMeasurementConstructorServiceId =\n    \"host-text-measurement\";"
        ));
        assert!(
            dart.contains(
                "const String mermanIconRegistryConstructorServiceId = \"icon-registry\";"
            )
        );

        for (container, field) in [
            ("metadata", "version"),
            ("plan", "width_px"),
            ("plan", "height_px"),
        ] {
            assert!(dart.contains(&format!("_requiredGeneratedUint32({container}, '{field}')")));
        }
        for (container, field) in [
            ("metadata", "byte_length"),
            ("plan", "filtered_groups"),
            ("plan", "requested_image_pixels"),
            ("plan", "effective_image_pixels"),
        ] {
            assert!(dart.contains(&format!("_requiredGeneratedUint64({container}, '{field}')")));
        }

        assert!(dart.contains("int _requiredGeneratedUint32("));
        assert!(dart.contains("if (field > 0xffffffff)"));
        assert!(dart.contains("int _requiredGeneratedUint64("));
        assert!(!dart.contains("_requiredGeneratedInt("));
    }

    #[test]
    fn shared_operation_projection_consumes_the_complete_typed_matrix() {
        let projection = shared_operation_contract_projection();
        assert_eq!(projection.schema_version, 1);
        assert_eq!(projection.operation_expectations.len(), 13);
        assert_eq!(
            projection.operation_metadata_contract,
            operation_metadata_contract()
        );

        let json = render_shared_operation_json();
        assert!(json.contains("\"operation_metadata_contract\""));
        assert!(json.contains("\"operation_id\": \"semantic-json\""));
        assert!(!json.contains("\"compiled\""));
    }
}

//! Generated host-language projections of binding-core-owned runtime vocabularies.

use crate::XtaskError;
#[cfg(test)]
use merman_bindings_core::TextMeasurementProviderSource;
use merman_bindings_core::{
    BINDING_OPERATION_METADATA_CONTRACT_SCHEMA_VERSION, BINDING_OPTIONS_SCHEMA_VERSION,
    BindingOperationMetadataContract, BindingOptionGroupKey, BindingPayloadSchemaKey,
    BindingTransportKey, BindingUnavailableOperationExpectation, CapabilityKey,
    ConstructorServiceKey, MetadataKey, RUNTIME_CATALOG_FIELD_IDENTIFIER_PATTERN,
    RUNTIME_CATALOG_IDENTIFIER_PATTERN, RUNTIME_CATALOG_MAX_SAFE_INTEGER,
    RUNTIME_CATALOG_SCHEMA_VERSION, TEXT_MEASUREMENT_PROTOCOL_VERSION, TextMeasurementProviderKey,
    binding_operation_expectations, operation_metadata_contract,
};
use serde::Serialize;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const NODE_OUTPUT: &str = "platforms/node/src/generated/binding-contract.mjs";
const WEB_OUTPUT: &str = "platforms/web/src/generated/binding-contract.ts";
const SHARED_OPERATION_OUTPUT: &str =
    "fixtures/bindings/generated/binding-operation-contract-v1.json";
const UNIFFI_PYTHON_OUTPUT: &str = "crates/merman-uniffi/python/binding_contract.py";
const PYTHON_OUTPUT: &str = "platforms/python/merman/src/merman/_binding_contract.py";

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
    vec![
        (PathBuf::from(NODE_OUTPUT), render_node_javascript()),
        (PathBuf::from(WEB_OUTPUT), render_web_typescript()),
        (
            PathBuf::from(SHARED_OPERATION_OUTPUT),
            render_shared_operation_json(),
        ),
        (PathBuf::from(UNIFFI_PYTHON_OUTPUT), render_python()),
        (PathBuf::from(PYTHON_OUTPUT), render_python()),
    ]
}

fn write_artifact(root: &Path, path: &Path, contents: &str) -> Result<(), XtaskError> {
    let full = root.join(path);
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

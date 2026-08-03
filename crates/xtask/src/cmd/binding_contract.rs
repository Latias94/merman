//! Generated host-language projections of binding-core-owned runtime vocabularies.

use crate::XtaskError;
use merman_bindings_core::{
    BINDING_OPERATION_METADATA_CONTRACT_SCHEMA_VERSION, BINDING_OPTIONS_SCHEMA_VERSION,
    BindingOperationMetadataContract, BindingOptionGroupKey, BindingPayloadSchemaKey,
    BindingUnavailableOperationExpectation, ConstructorServiceKey, RUNTIME_CATALOG_SCHEMA_VERSION,
    TEXT_MEASUREMENT_PROTOCOL_VERSION, TextMeasurementProviderKey, TextMeasurementProviderSource,
    binding_operation_expectations, operation_metadata_contract,
};
use serde::Serialize;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const NODE_OUTPUT: &str = "platforms/node/src/generated/binding-contract.mjs";
const SHARED_OPERATION_OUTPUT: &str =
    "fixtures/bindings/generated/binding-operation-contract-v1.json";
const UNIFFI_PYTHON_OUTPUT: &str = "crates/merman-uniffi/python/binding_contract.py";
const PYTHON_OUTPUT: &str = "platforms/python/merman/src/merman/_binding_contract.py";

#[derive(Serialize)]
struct OptionGroupProjection {
    id: &'static str,
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
    provided_text_measurement_provider_ids: Vec<&'static str>,
}

#[derive(Serialize)]
struct TextMeasurementProviderProjection {
    id: &'static str,
    source: &'static str,
    constructor_service_id: Option<&'static str>,
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

fn render_node_javascript() -> String {
    let option_groups = BindingOptionGroupKey::ALL
        .iter()
        .copied()
        .map(|key| {
            let spec = key.spec();
            OptionGroupProjection {
                id: key.id(),
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
            let (source, constructor_service_id) = match key.source() {
                TextMeasurementProviderSource::SvgPipeline => ("svg-pipeline", None),
                TextMeasurementProviderSource::ConstructorService(service) => {
                    ("constructor-service", Some(service.id()))
                }
                _ => unreachable!("the generator must handle every provider source"),
            };
            TextMeasurementProviderProjection {
                id: key.id(),
                source,
                constructor_service_id,
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
            provided_text_measurement_provider_ids: providers
                .iter()
                .filter(|provider| provider.constructor_service_id == Some(key.id()))
                .map(|provider| provider.id)
                .collect(),
        })
        .collect::<Vec<_>>();
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
        pretty_json(&payload_schemas)
    )
    .unwrap();
    writeln!(
        out,
        "export const BINDING_OPTION_GROUP_SPECS = {};\n",
        pretty_json(&option_groups)
    )
    .unwrap();
    writeln!(
        out,
        "export const TEXT_MEASUREMENT_PROVIDER_SPECS = {};\n",
        pretty_json(&providers)
    )
    .unwrap();
    writeln!(
        out,
        "export const TEXT_MEASUREMENT_PROVIDER_IDS = {};",
        pretty_json(&provider_ids)
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
        pretty_json(&constructor_services)
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
    let mut out = String::from(
        "# @generated by `cargo run -p xtask -- gen-binding-contract`.\n# Sources: typed registries in merman-bindings-core. Do not edit directly.\n\n",
    );
    out.push_str("REQUIRED_PAYLOAD_SCHEMA_VERSIONS = {\n");
    for key in BindingPayloadSchemaKey::ALL {
        writeln!(out, "    {:?}: {},", key.id(), key.version()).unwrap();
    }
    out.push_str("}\n");
    out
}

fn generated_artifacts() -> Vec<(PathBuf, String)> {
    vec![
        (PathBuf::from(NODE_OUTPUT), render_node_javascript()),
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
    fn node_projection_is_sorted_and_uses_typed_service_relations() {
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
        assert!(generated.contains("BINDING_OPTION_GROUP_SPECS"));
        assert!(generated.contains("CONSTRUCTOR_SERVICE_SPECS"));
        assert!(generated.contains("BINDING_PAYLOAD_SCHEMAS"));
        assert!(generated.contains("TEXT_MEASUREMENT_PROVIDER_SPECS"));
    }

    #[test]
    fn python_projection_uses_the_typed_payload_schema_registry() {
        let generated = render_python();
        assert!(generated.contains("REQUIRED_PAYLOAD_SCHEMA_VERSIONS"));
        for key in BindingPayloadSchemaKey::ALL {
            assert!(generated.contains(&format!("{:?}: {}", key.id(), key.version())));
        }
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

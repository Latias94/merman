use super::{
    artifact_profiles::{WasmArtifactProfile, load_wasm_size_artifact_profiles},
    typst_profiles::{TypstPackageProfile, TypstProfileCatalog, validate_typst_artifact_profiles},
    wasm_module_surface::{LoadedWasmModule, WasmModuleLoadError, WasmSurfaceProfile},
};
use crate::XtaskError;
#[cfg(test)]
use crate::cmd::typst_profiles::load_typst_profiles;
#[cfg(test)]
use merman_bindings_core::OperationKey;
use merman_bindings_core::{
    BindingOptionGroupKey, BindingPayloadSchemaKey, ConstructorServiceKey, MetadataKey,
};
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::path::Path;
use wasmi::{Caller, Linker, Store, Val, ValType};

const DEFAULT_SOURCE: &[u8] = b"flowchart TD\nA[Hello] --> B[World]";
const DEFAULT_OPTIONS_JSON: &[u8] =
    br#"{"fixed_today":"2026-06-10","fixed_local_offset_minutes":480}"#;
const TYPST_RESULT_PAYLOAD_SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Default)]
struct CallData {
    args: Vec<Vec<u8>>,
    output: Vec<u8>,
    memory_error: Option<MemoryError>,
}

#[derive(Debug, Clone, Copy)]
struct MemoryError {
    offset: u32,
    length: u32,
    write: bool,
}

pub(crate) fn validate_typst_plugin(
    wasm_file: &Path,
    catalog: &TypstProfileCatalog,
    profile: &TypstPackageProfile,
) -> Result<(), XtaskError> {
    validate_typst_plugin_with_input(
        wasm_file,
        catalog,
        profile,
        DEFAULT_SOURCE,
        DEFAULT_OPTIONS_JSON,
    )
}

pub(crate) fn validate_typst_plugin_with_input(
    wasm_file: &Path,
    catalog: &TypstProfileCatalog,
    profile: &TypstPackageProfile,
    source: &[u8],
    options_json: &[u8],
) -> Result<(), XtaskError> {
    let artifact_profile = validate_requested_profile(catalog, profile)?;
    let mut instance = PluginInstance::new(wasm_file)?;

    let expected_abi_version = merman_typst_plugin::abi_version();
    let abi_version = instance.call("abi_version", Vec::new())?;
    if abi_version != expected_abi_version {
        return Err(smoke_error(format!(
            "abi_version returned {:?}; expected ABI bytes {:?}",
            String::from_utf8_lossy(&abi_version),
            String::from_utf8_lossy(expected_abi_version)
        )));
    }

    let expected_package_version = merman_typst_plugin::package_version();
    let package_version = instance.call("package_version", Vec::new())?;
    if package_version != expected_package_version {
        return Err(smoke_error(format!(
            "package_version returned {:?}; expected workspace version {:?}",
            String::from_utf8_lossy(&package_version),
            String::from_utf8_lossy(&expected_package_version)
        )));
    }

    let capabilities = call_json(&mut instance, "capabilities_json", Vec::new())?;
    assert_capability_catalog(&capabilities, &artifact_profile)?;

    let render_output = instance.call(
        "render_svg_json",
        vec![source.to_vec(), options_json.to_vec()],
    )?;
    let render_payload = parse_json("render_svg_json", &render_output)?;
    assert_render_payload(&render_payload)?;

    let analysis_output =
        instance.call("analyze_json", vec![source.to_vec(), options_json.to_vec()])?;
    let analysis_payload = parse_json("analyze_json", &analysis_output)?;
    assert_analysis_payload(&analysis_payload, false)?;

    let diagnostic_probe =
        instance.call("analyze_json", vec![Vec::new(), options_json.to_vec()])?;
    let diagnostic_payload = parse_json("analyze_json diagnostic probe", &diagnostic_probe)?;
    assert_analysis_payload(&diagnostic_payload, true)?;

    let render_error_output =
        instance.call("render_svg_json", vec![source.to_vec(), b"{".to_vec()])?;
    let render_error_payload = parse_json("render_svg_json error probe", &render_error_output)?;
    assert_error_payload(
        &render_error_payload,
        "render-svg",
        "MERMAN_OPTIONS_JSON_ERROR",
    )?;

    let analysis_error_output =
        instance.call("analyze_json", vec![source.to_vec(), b"{".to_vec()])?;
    let analysis_error_payload = parse_json("analyze_json error probe", &analysis_error_output)?;
    assert_error_payload(
        &analysis_error_payload,
        "analyze",
        "MERMAN_OPTIONS_JSON_ERROR",
    )?;

    for (function, operation, label, malicious_options) in [
        (
            "render_svg_json",
            "render-svg",
            "null merman wrapper",
            br#"{"merman":null,"resources":{"profile":"trusted-native"}}"#.as_slice(),
        ),
        (
            "analyze_json",
            "analyze",
            "array analysis wrapper",
            br#"{"analysis":[]}"#.as_slice(),
        ),
    ] {
        let output = instance.call(function, vec![source.to_vec(), malicious_options.to_vec()])?;
        let payload = parse_json(&format!("{function} {label} probe"), &output)?;
        assert_error_payload(&payload, operation, "MERMAN_OPTIONS_JSON_ERROR")?;
        if !payload
            .get("message")
            .and_then(JsonValue::as_str)
            .is_some_and(|message| message.contains("wrapper must be an object"))
        {
            return Err(smoke_error(format!(
                "{function} did not reject the {label} before resource policy selection: {payload}"
            )));
        }
    }

    Ok(())
}

fn validate_requested_profile(
    catalog: &TypstProfileCatalog,
    profile: &TypstPackageProfile,
) -> Result<WasmArtifactProfile, XtaskError> {
    let crate_abi_version = merman_typst_plugin::TYPST_PLUGIN_ABI_VERSION;
    if catalog.plugin_abi_version() != crate_abi_version {
        return Err(smoke_error(format!(
            "Typst package descriptor declares ABI {}; plugin declares ABI {crate_abi_version}",
            catalog.plugin_abi_version(),
        )));
    }
    if catalog.package_profile() != profile {
        return Err(smoke_error(format!(
            "profile `{}` is not the canonical Typst package profile",
            profile.name()
        )));
    }
    let artifact = canonical_typst_artifact_profile(catalog)?;
    if !artifact
        .capabilities
        .iter()
        .any(|capability| capability == "svg")
        || !artifact
            .capabilities
            .iter()
            .any(|capability| capability == "analysis")
    {
        return Err(smoke_error(format!(
            "canonical Typst artifact `{}` must support both render and analysis",
            artifact.id
        )));
    }
    Ok(artifact)
}

fn canonical_typst_artifact_profile(
    catalog: &TypstProfileCatalog,
) -> Result<WasmArtifactProfile, XtaskError> {
    let profiles = load_wasm_size_artifact_profiles().map_err(|error| {
        smoke_error(format!(
            "failed to load canonical Typst artifact recipe `{}`: {error}",
            catalog.artifact_profile_id()
        ))
    })?;
    validate_typst_artifact_profiles(catalog, &profiles)?;
    let matching = profiles
        .iter()
        .filter(|profile| profile.id == catalog.artifact_profile_id())
        .collect::<Vec<_>>();
    let [artifact] = matching.as_slice() else {
        return Err(smoke_error(format!(
            "expected exactly one canonical Typst artifact recipe `{}`, found {}",
            catalog.artifact_profile_id(),
            matching.len()
        )));
    };
    if artifact.semantic_target != "typst" {
        return Err(smoke_error(format!(
            "artifact recipe `{}` is not owned by the Typst target",
            artifact.id
        )));
    }
    Ok((*artifact).clone())
}

fn expected_typst_operation_ids(artifact: &WasmArtifactProfile) -> Vec<String> {
    merman_typst_plugin::TYPST_TRANSPORT_OPERATION_KEYS
        .iter()
        .copied()
        .filter(|operation| {
            let spec = operation.spec();
            spec.targets
                .contains(&merman_bindings_core::TargetKey::Typst)
                && spec.capability.is_none_or(|capability| {
                    artifact.capabilities.iter().any(|id| id == capability.id())
                })
                && spec
                    .compiled_prerequisites
                    .iter()
                    .all(|capability| artifact.capabilities.iter().any(|id| id == capability.id()))
                && spec
                    .output
                    .is_none_or(|output| artifact.outputs.iter().any(|id| id == output.id()))
        })
        .map(|operation| operation.id().to_string())
        .collect()
}

fn expected_typst_option_group_ids(artifact: &WasmArtifactProfile) -> Vec<String> {
    let uses_svg_pipeline = artifact.capabilities.iter().any(|id| id == "svg");
    BindingOptionGroupKey::ALL
        .iter()
        .copied()
        .filter(|key| {
            let spec = key.spec();
            spec.always_available()
                || (spec.requires_svg_pipeline() && uses_svg_pipeline)
                || spec
                    .any_capabilities()
                    .iter()
                    .any(|capability| artifact.capabilities.iter().any(|id| id == capability.id()))
        })
        .map(|key| key.id().to_string())
        .collect()
}

fn call_json(
    instance: &mut PluginInstance,
    name: &str,
    args: Vec<Vec<u8>>,
) -> Result<JsonValue, XtaskError> {
    let output = instance.call(name, args)?;
    parse_json(name, &output)
}

fn parse_json(name: &str, output: &[u8]) -> Result<JsonValue, XtaskError> {
    serde_json::from_slice(output)
        .map_err(|source| smoke_error(format!("{name} returned non-JSON bytes: {source}")))
}

fn assert_capability_catalog(
    payload: &JsonValue,
    artifact: &WasmArtifactProfile,
) -> Result<(), XtaskError> {
    let catalog = required_object(
        payload,
        &[
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
        ],
        "Typst capability catalog",
    )?;
    if catalog.get("schema_version").and_then(JsonValue::as_u64)
        != Some(merman_typst_plugin::TYPST_RUNTIME_CATALOG_SCHEMA_VERSION as u64)
        || catalog
            .get("transport_api_version")
            .and_then(JsonValue::as_u64)
            != Some(merman_typst_plugin::TYPST_PLUGIN_ABI_VERSION as u64)
        || catalog.get("package_version").and_then(JsonValue::as_str)
            != std::str::from_utf8(&merman_typst_plugin::package_version()).ok()
    {
        return Err(smoke_error(format!(
            "capabilities_json returned an invalid Typst catalog header: {payload}"
        )));
    }

    let options_schema_versions = positive_integer_array(
        catalog
            .get("options_schema_versions")
            .expect("validated Typst capability catalog"),
        "Typst options schema versions",
    )?;
    if !options_schema_versions
        .contains(&(merman_bindings_core::BINDING_OPTIONS_SCHEMA_VERSION as u64))
    {
        return Err(smoke_error(format!(
            "Typst runtime catalog does not advertise options schema v{}",
            merman_bindings_core::BINDING_OPTIONS_SCHEMA_VERSION
        )));
    }

    let payload_schema_ids = validated_payload_schema_ids(
        catalog
            .get("payload_schemas")
            .expect("validated Typst capability catalog"),
    )?;
    if payload_schema_ids
        .iter()
        .any(|id| BindingPayloadSchemaKey::from_id(id).is_some())
    {
        return Err(smoke_error(
            "Typst runtime catalog must not advertise binding payload schemas",
        ));
    }

    let metadata_ids = string_array(
        catalog
            .get("metadata_ids")
            .expect("validated Typst capability catalog"),
        "Typst runtime metadata IDs",
    )?;
    if metadata_ids
        .iter()
        .any(|id| MetadataKey::from_id(id).is_some())
    {
        return Err(smoke_error(
            "Typst runtime catalog must not advertise known metadata dispatchers",
        ));
    }

    let runtime_capabilities = catalog
        .get("capabilities")
        .ok_or_else(|| smoke_error("Typst runtime catalog is missing capabilities"))?;
    let runtime_capabilities = required_object(
        runtime_capabilities,
        &[
            "capability_ids",
            "output_ids",
            "operation_ids",
            "system_adapter_ids",
            "text_measurement",
        ],
        "Typst runtime capabilities",
    )?;
    let capability_ids = string_array(
        runtime_capabilities
            .get("capability_ids")
            .expect("closed runtime capabilities object"),
        "Typst runtime capability IDs",
    )?;
    if capability_ids != artifact.capabilities {
        return Err(smoke_error(format!(
            "Typst runtime capability IDs do not match canonical artifact `{}`: expected [{}], found [{}]",
            artifact.id,
            artifact.capabilities.join(","),
            capability_ids.join(",")
        )));
    }
    let output_ids = string_array(
        runtime_capabilities
            .get("output_ids")
            .expect("closed runtime capabilities object"),
        "Typst runtime output IDs",
    )?;
    if output_ids != artifact.outputs {
        return Err(smoke_error(format!(
            "Typst runtime output IDs do not match canonical artifact `{}`: expected [{}], found [{}]",
            artifact.id,
            artifact.outputs.join(","),
            output_ids.join(",")
        )));
    }
    let output_contract_ids = validated_output_contract_ids(
        catalog
            .get("output_contracts")
            .expect("validated Typst capability catalog"),
    )?;
    if output_contract_ids != output_ids {
        return Err(smoke_error(format!(
            "Typst runtime output contracts do not match output IDs: expected [{}], found [{}]",
            output_ids.join(","),
            output_contract_ids.join(",")
        )));
    }
    let operation_ids = string_array(
        runtime_capabilities
            .get("operation_ids")
            .expect("closed runtime capabilities object"),
        "Typst runtime operation IDs",
    )?;
    let expected_operation_ids = expected_typst_operation_ids(artifact);
    if operation_ids != expected_operation_ids {
        return Err(smoke_error(format!(
            "Typst runtime operation IDs do not match the closed plugin transport: expected [{}], found [{}]",
            expected_operation_ids.join(","),
            operation_ids.join(",")
        )));
    }
    let system_adapter_ids = string_array(
        runtime_capabilities
            .get("system_adapter_ids")
            .expect("closed runtime capabilities object"),
        "Typst runtime system adapter IDs",
    )?;
    if !system_adapter_ids.is_empty() {
        return Err(smoke_error(
            "Typst runtime capabilities must not advertise system adapter IDs",
        ));
    }
    if !output_ids
        .iter()
        .all(|output| operation_ids.binary_search(output).is_ok())
    {
        return Err(smoke_error(
            "Typst runtime outputs must also be callable operation IDs",
        ));
    }
    if !system_adapter_ids
        .iter()
        .all(|adapter| capability_ids.binary_search(adapter).is_ok())
    {
        return Err(smoke_error(
            "Typst runtime system adapters must also be capability IDs",
        ));
    }
    let text_measurement = runtime_capabilities
        .get("text_measurement")
        .ok_or_else(|| {
            smoke_error("Typst SVG runtime capabilities must expose text measurement metadata")
        })?;
    let text_measurement = required_object(
        text_measurement,
        &["protocol_version", "provider_ids"],
        "Typst text measurement metadata",
    )?;
    let text_measurement_provider_ids = string_array(
        text_measurement.get("provider_ids").ok_or_else(|| {
            smoke_error("Typst text measurement metadata is missing provider IDs")
        })?,
        "Typst text measurement provider IDs",
    )?;
    if text_measurement
        .get("protocol_version")
        .and_then(JsonValue::as_u64)
        .is_none_or(|version| version == 0)
        || text_measurement_provider_ids != vec!["deterministic".to_string()]
    {
        return Err(smoke_error(format!(
            "Typst text measurement metadata must expose only the deterministic provider: {}",
            JsonValue::Object(text_measurement.clone())
        )));
    }

    if let Some(option_group_ids) = catalog.get("option_group_ids") {
        let option_group_ids = string_array(option_group_ids, "Typst runtime option group IDs")?;
        let expected_option_group_ids = expected_typst_option_group_ids(artifact);
        let known_option_group_ids = option_group_ids
            .iter()
            .filter(|id| BindingOptionGroupKey::from_id(id).is_some())
            .cloned()
            .collect::<Vec<_>>();
        if known_option_group_ids != expected_option_group_ids {
            return Err(smoke_error(format!(
                "Typst runtime option group IDs do not match the artifact: expected [{}], found [{}]",
                expected_option_group_ids.join(","),
                known_option_group_ids.join(",")
            )));
        }
    }

    assert_typst_constructor_services(catalog, &text_measurement_provider_ids)?;

    let mut runtime_ids = capability_ids.clone();
    runtime_ids.sort();
    runtime_ids.dedup();
    if runtime_ids != artifact.runtime_ids {
        return Err(smoke_error(format!(
            "Typst runtime IDs do not match canonical artifact `{}`: expected [{}], found [{}]",
            artifact.id,
            artifact.runtime_ids.join(","),
            runtime_ids.join(",")
        )));
    }

    let registry = required_object(
        catalog
            .get("registry")
            .expect("validated Typst capability catalog"),
        &["diagram_family_count"],
        "Typst registry catalog",
    )?;
    if registry
        .get("diagram_family_count")
        .and_then(JsonValue::as_u64)
        .is_none_or(|count| count == 0)
    {
        return Err(smoke_error(
            "Typst registry catalog must report a positive diagram family count",
        ));
    }

    let resources = required_object(
        catalog
            .get("resources")
            .expect("validated Typst capability catalog"),
        &[
            "general_binding_default_profile",
            "cli_default_profile",
            "limits",
            "profiles",
        ],
        "Typst runtime resource catalog",
    )?;
    if resources
        .get("general_binding_default_profile")
        .and_then(JsonValue::as_str)
        .is_none_or(str::is_empty)
        || resources
            .get("cli_default_profile")
            .and_then(JsonValue::as_str)
            .is_none_or(str::is_empty)
        || !resources.get("limits").is_some_and(JsonValue::is_array)
        || !resources
            .get("profiles")
            .and_then(JsonValue::as_array)
            .is_some_and(|profiles| {
                profiles.iter().any(|profile| {
                    profile.get("id").and_then(JsonValue::as_str) == Some("constrained")
                })
            })
    {
        return Err(smoke_error(
            "Typst runtime resource catalog must publish the constrained resource profile",
        ));
    }

    Ok(())
}

fn required_object<'a>(
    value: &'a JsonValue,
    required: &[&str],
    context: &str,
) -> Result<&'a JsonMap<String, JsonValue>, XtaskError> {
    let object = value
        .as_object()
        .ok_or_else(|| smoke_error(format!("{context} must be a JSON object")))?;
    let missing = required
        .iter()
        .copied()
        .filter(|key| !object.contains_key(*key))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(smoke_error(format!(
            "{context} is missing required fields: [{}]",
            missing.join(", ")
        )));
    }
    Ok(object)
}

fn string_array(value: &JsonValue, context: &str) -> Result<Vec<String>, XtaskError> {
    let array = value
        .as_array()
        .ok_or_else(|| smoke_error(format!("{context} must be an array")))?;
    let values = array
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| smoke_error(format!("{context} must contain only strings")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut sorted = values.clone();
    sorted.sort();
    sorted.dedup();
    if values != sorted {
        return Err(smoke_error(format!(
            "{context} must be sorted and unique: [{}]",
            values.join(",")
        )));
    }
    Ok(values)
}

fn positive_integer_array(value: &JsonValue, context: &str) -> Result<Vec<u64>, XtaskError> {
    let array = value
        .as_array()
        .ok_or_else(|| smoke_error(format!("{context} must be an array")))?;
    let values = array
        .iter()
        .map(|value| {
            value
                .as_u64()
                .filter(|value| *value > 0)
                .ok_or_else(|| smoke_error(format!("{context} must contain positive integers")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut sorted = values.clone();
    sorted.sort_unstable();
    sorted.dedup();
    if values != sorted {
        return Err(smoke_error(format!(
            "{context} must be sorted and unique: {values:?}"
        )));
    }
    Ok(values)
}

fn validated_payload_schema_ids(value: &JsonValue) -> Result<Vec<String>, XtaskError> {
    let schemas = value
        .as_array()
        .ok_or_else(|| smoke_error("Typst runtime payload schemas must be an array"))?;
    let mut ids = Vec::with_capacity(schemas.len());
    for schema in schemas {
        let schema = required_object(schema, &["id", "version"], "Typst runtime payload schema")?;
        let id = schema
            .get("id")
            .and_then(JsonValue::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| smoke_error("Typst runtime payload schema ID must be non-empty"))?;
        if schema
            .get("version")
            .and_then(JsonValue::as_u64)
            .is_none_or(|version| version == 0)
        {
            return Err(smoke_error(
                "Typst runtime payload schema version must be positive",
            ));
        }
        ids.push(id.to_string());
    }
    ensure_sorted_unique(&ids, "Typst runtime payload schema IDs")?;
    Ok(ids)
}

fn validated_output_contract_ids(value: &JsonValue) -> Result<Vec<String>, XtaskError> {
    let contracts = value
        .as_array()
        .ok_or_else(|| smoke_error("Typst runtime output contracts must be an array"))?;
    let mut ids = Vec::with_capacity(contracts.len());
    for contract in contracts {
        let contract = required_object(
            contract,
            &["id", "media_type", "system_fonts", "embedded_images"],
            "Typst runtime output contract",
        )?;
        let id = contract
            .get("id")
            .and_then(JsonValue::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| smoke_error("Typst runtime output contract ID must be non-empty"))?;
        if contract
            .get("media_type")
            .and_then(JsonValue::as_str)
            .is_none_or(str::is_empty)
        {
            return Err(smoke_error(
                "Typst runtime output contract media_type must be non-empty",
            ));
        }
        ids.push(id.to_string());
    }
    ensure_sorted_unique(&ids, "Typst runtime output contract IDs")?;
    Ok(ids)
}

fn assert_typst_constructor_services(
    catalog: &JsonMap<String, JsonValue>,
    text_measurement_provider_ids: &[String],
) -> Result<(), XtaskError> {
    let service_ids = catalog.get("constructor_service_ids");
    let service_contracts = catalog.get("constructor_service_contracts");
    let (Some(service_ids), Some(service_contracts)) = (service_ids, service_contracts) else {
        if service_ids.is_some() || service_contracts.is_some() {
            return Err(smoke_error(
                "Typst constructor service IDs and contracts must appear together",
            ));
        }
        return Ok(());
    };

    let service_ids = string_array(service_ids, "Typst constructor service IDs")?;
    if service_ids
        .iter()
        .any(|id| ConstructorServiceKey::from_id(id).is_some())
    {
        return Err(smoke_error(
            "Typst runtime must not advertise a known constructor service",
        ));
    }
    let contracts = service_contracts
        .as_array()
        .ok_or_else(|| smoke_error("Typst constructor service contracts must be an array"))?;
    let mut contract_ids = Vec::with_capacity(contracts.len());
    for contract in contracts {
        let contract = required_object(
            contract,
            &[
                "id",
                "provided_text_measurement_provider_ids",
                "resource_limits",
            ],
            "Typst constructor service contract",
        )?;
        let id = contract
            .get("id")
            .and_then(JsonValue::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| smoke_error("Typst constructor service ID must be non-empty"))?;
        let provider_ids = string_array(
            contract
                .get("provided_text_measurement_provider_ids")
                .expect("closed constructor service contract"),
            "Typst constructor service provider IDs",
        )?;
        if provider_ids
            .iter()
            .any(|provider| !text_measurement_provider_ids.contains(provider))
        {
            return Err(smoke_error(
                "Typst constructor service names an unavailable text measurement provider",
            ));
        }
        if !provider_ids.is_empty() {
            return Err(smoke_error(
                "Typst constructor services must not claim pipeline-owned text measurement providers",
            ));
        }
        validate_constructor_resource_limits(
            contract
                .get("resource_limits")
                .expect("closed constructor service contract"),
        )?;
        contract_ids.push(id.to_string());
    }
    ensure_sorted_unique(&contract_ids, "Typst constructor service contract IDs")?;
    if contract_ids != service_ids {
        return Err(smoke_error(
            "Typst constructor service contracts must match constructor service IDs",
        ));
    }
    Ok(())
}

fn validate_constructor_resource_limits(value: &JsonValue) -> Result<(), XtaskError> {
    let limits = value
        .as_array()
        .ok_or_else(|| smoke_error("Typst constructor service resource limits must be an array"))?;
    let mut ids = Vec::with_capacity(limits.len());
    for limit in limits {
        let limit = required_object(
            limit,
            &["id", "phase", "unit", "description", "value"],
            "Typst constructor service resource limit",
        )?;
        let id = limit
            .get("id")
            .and_then(JsonValue::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| smoke_error("Typst constructor resource limit ID must be non-empty"))?;
        for field in ["phase", "unit", "description"] {
            if limit
                .get(field)
                .and_then(JsonValue::as_str)
                .is_none_or(str::is_empty)
            {
                return Err(smoke_error(format!(
                    "Typst constructor resource limit {field} must be non-empty"
                )));
            }
        }
        if limit.get("value").and_then(JsonValue::as_u64).is_none() {
            return Err(smoke_error(
                "Typst constructor resource limit value must be a non-negative integer",
            ));
        }
        ids.push(id.to_string());
    }
    ensure_sorted_unique(&ids, "Typst constructor resource limit IDs")
}

fn ensure_sorted_unique(values: &[String], context: &str) -> Result<(), XtaskError> {
    let mut sorted = values.to_vec();
    sorted.sort();
    sorted.dedup();
    if values != sorted {
        return Err(smoke_error(format!(
            "{context} must be sorted and unique: [{}]",
            values.join(",")
        )));
    }
    Ok(())
}

fn assert_render_payload(payload: &JsonValue) -> Result<usize, XtaskError> {
    let object = exact_object(
        payload,
        &[
            "version",
            "operation",
            "ok",
            "code",
            "code_name",
            "kind",
            "capability_id",
            "message",
            "data",
        ],
        "render payload",
    )?;
    if object.get("version").and_then(JsonValue::as_u64)
        != Some(TYPST_RESULT_PAYLOAD_SCHEMA_VERSION)
        || object.get("operation").and_then(JsonValue::as_str) != Some("render-svg")
        || object.get("ok").and_then(JsonValue::as_bool) != Some(true)
        || object.get("code").and_then(JsonValue::as_i64) != Some(0)
        || object.get("code_name").and_then(JsonValue::as_str) != Some("MERMAN_OK")
        || !object.get("kind").is_some_and(JsonValue::is_null)
        || !object.get("capability_id").is_some_and(JsonValue::is_null)
        || !object.get("message").is_some_and(JsonValue::is_null)
    {
        return Err(smoke_error(format!(
            "render_svg_json returned an invalid success payload: {payload}"
        )));
    }
    let data = object
        .get("data")
        .and_then(JsonValue::as_object)
        .ok_or_else(|| smoke_error("render payload `data` must be an object"))?;
    let data_value = JsonValue::Object(data.clone());
    let data = exact_object(&data_value, &["svg"], "render payload data")?;
    let svg = data
        .get("svg")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| smoke_error("render payload data `svg` must be a string"))?;
    let document = roxmltree::Document::parse(svg)
        .map_err(|source| smoke_error(format!("render payload contains invalid SVG: {source}")))?;
    if document.root_element().tag_name().name() != "svg" {
        return Err(smoke_error(
            "render payload document root must be an SVG element",
        ));
    }
    Ok(svg.len())
}

fn assert_analysis_payload(
    payload: &JsonValue,
    require_diagnostic: bool,
) -> Result<(), XtaskError> {
    let object = exact_object(
        payload,
        &[
            "version",
            "operation",
            "ok",
            "code",
            "code_name",
            "kind",
            "capability_id",
            "message",
            "data",
        ],
        "analysis payload envelope",
    )?;
    if object.get("version").and_then(JsonValue::as_u64)
        != Some(TYPST_RESULT_PAYLOAD_SCHEMA_VERSION)
        || object.get("operation").and_then(JsonValue::as_str) != Some("analyze")
        || object.get("ok").and_then(JsonValue::as_bool) != Some(true)
        || object.get("code").and_then(JsonValue::as_i64) != Some(0)
        || object.get("code_name").and_then(JsonValue::as_str) != Some("MERMAN_OK")
        || !object.get("kind").is_some_and(JsonValue::is_null)
        || !object.get("capability_id").is_some_and(JsonValue::is_null)
        || !object.get("message").is_some_and(JsonValue::is_null)
    {
        return Err(smoke_error(format!(
            "analyze_json returned an invalid success envelope: {payload}"
        )));
    }
    let data = object
        .get("data")
        .and_then(JsonValue::as_object)
        .ok_or_else(|| smoke_error("analysis payload envelope `data` must be an object"))?;
    let data_value = JsonValue::Object(data.clone());
    let data = exact_object(&data_value, &["analysis"], "analysis payload envelope data")?;
    let analysis = data
        .get("analysis")
        .ok_or_else(|| smoke_error("analysis payload envelope data is missing analysis"))?;
    let canonical: merman_analysis::AnalysisPayload = serde_json::from_value(analysis.clone())
        .map_err(|source| {
            smoke_error(format!(
                "analyze_json does not match canonical analysis schema {}: {source}",
                merman_analysis::ANALYSIS_PAYLOAD_VERSION
            ))
        })?;
    let canonical_json = serde_json::to_value(&canonical).map_err(|source| {
        smoke_error(format!(
            "failed to serialize the canonical analysis payload: {source}"
        ))
    })?;
    if canonical_json != *analysis {
        return Err(smoke_error(format!(
            "analyze_json analysis data is not closed under canonical analysis schema {}: expected {canonical_json}, found {analysis}",
            merman_analysis::ANALYSIS_PAYLOAD_VERSION
        )));
    }
    if canonical.version != merman_analysis::ANALYSIS_PAYLOAD_VERSION {
        return Err(smoke_error(format!(
            "analyze_json returned schema version {}; expected {}",
            canonical.version,
            merman_analysis::ANALYSIS_PAYLOAD_VERSION
        )));
    }
    if canonical.source != merman_analysis::SourceDescriptor::diagram() {
        return Err(smoke_error(format!(
            "analyze_json returned an invalid source descriptor: {:?}",
            canonical.source
        )));
    }
    if require_diagnostic && canonical.diagnostics.is_empty() {
        return Err(smoke_error(
            "analysis schema probe did not return a diagnostic for empty source",
        ));
    }
    let expected_summary = merman_analysis::Summary::from_diagnostics(&canonical.diagnostics);
    if canonical.summary != expected_summary {
        return Err(smoke_error(
            "analysis summary does not match the returned diagnostics",
        ));
    }
    if canonical.valid != (expected_summary.errors == 0) {
        return Err(smoke_error(
            "analysis payload `valid` does not match its error count",
        ));
    }
    Ok(())
}

fn assert_error_payload(
    payload: &JsonValue,
    operation: &str,
    code_name: &str,
) -> Result<(), XtaskError> {
    let object = exact_object(
        payload,
        &[
            "version",
            "operation",
            "ok",
            "code",
            "code_name",
            "kind",
            "capability_id",
            "message",
            "data",
        ],
        "Typst operation error payload",
    )?;
    if object.get("version").and_then(JsonValue::as_u64)
        != Some(TYPST_RESULT_PAYLOAD_SCHEMA_VERSION)
        || object.get("operation").and_then(JsonValue::as_str) != Some(operation)
        || object.get("ok").and_then(JsonValue::as_bool) != Some(false)
        || object.get("code").and_then(JsonValue::as_i64) == Some(0)
        || object.get("code_name").and_then(JsonValue::as_str) != Some(code_name)
        || !object.get("kind").is_some_and(JsonValue::is_string)
        || !object.get("message").is_some_and(JsonValue::is_string)
        || !object.get("data").is_some_and(JsonValue::is_null)
    {
        return Err(smoke_error(format!(
            "Typst operation returned an invalid structured error payload: {payload}"
        )));
    }
    Ok(())
}

fn exact_object<'a>(
    value: &'a JsonValue,
    expected: &[&str],
    context: &str,
) -> Result<&'a JsonMap<String, JsonValue>, XtaskError> {
    let object = value
        .as_object()
        .ok_or_else(|| smoke_error(format!("{context} must be a JSON object")))?;
    let missing = expected
        .iter()
        .copied()
        .filter(|key| !object.contains_key(*key))
        .collect::<Vec<_>>();
    let extra = object
        .keys()
        .map(String::as_str)
        .filter(|key| !expected.contains(key))
        .collect::<Vec<_>>();
    if !missing.is_empty() || !extra.is_empty() {
        return Err(smoke_error(format!(
            "{context} is not closed: missing [{}], extra [{}]",
            missing.join(", "),
            extra.join(", ")
        )));
    }
    Ok(object)
}

fn smoke_error(message: impl Into<String>) -> XtaskError {
    XtaskError::TypstPluginSmokeFailed(message.into())
}

struct PluginInstance {
    instance: wasmi::Instance,
    store: Store<CallData>,
}

impl PluginInstance {
    fn new(wasm_file: &Path) -> Result<Self, XtaskError> {
        let module = LoadedWasmModule::from_file(wasm_file).map_err(|error| match error {
            WasmModuleLoadError::Read { path, source } => XtaskError::ReadFile {
                path: path.display().to_string(),
                source,
            },
            WasmModuleLoadError::Compile { path, message } => smoke_error(format!(
                "failed to load WebAssembly module {}: {message}",
                path.display()
            )),
        })?;
        let mut failures = module.surface().validate_imports(WasmSurfaceProfile::Typst);
        failures.extend(module.surface().validate_exports(WasmSurfaceProfile::Typst));
        if !failures.is_empty() {
            return Err(smoke_error(format!(
                "plugin does not expose the closed Typst ABI surface:\n{}",
                failures.join("\n")
            )));
        }

        let mut linker = Linker::new(module.engine());
        linker
            .func_wrap(
                "typst_env",
                "wasm_minimal_protocol_send_result_to_host",
                wasm_minimal_protocol_send_result_to_host,
            )
            .map_err(|source| {
                XtaskError::TypstPluginSmokeFailed(format!(
                    "failed to link send_result_to_host: {source}"
                ))
            })?;
        linker
            .func_wrap(
                "typst_env",
                "wasm_minimal_protocol_write_args_to_buffer",
                wasm_minimal_protocol_write_args_to_buffer,
            )
            .map_err(|source| {
                XtaskError::TypstPluginSmokeFailed(format!(
                    "failed to link write_args_to_buffer: {source}"
                ))
            })?;

        let mut store = Store::new(linker.engine(), CallData::default());
        let instance = linker
            .instantiate_and_start(&mut store, module.module())
            .map_err(|source| {
                XtaskError::TypstPluginSmokeFailed(format!(
                    "failed to instantiate WebAssembly module: {source}"
                ))
            })?;

        Ok(Self { instance, store })
    }

    fn call(&mut self, name: &str, args: Vec<Vec<u8>>) -> Result<Vec<u8>, XtaskError> {
        let handle = self
            .instance
            .get_export(&self.store, name)
            .ok_or_else(|| {
                XtaskError::TypstPluginSmokeFailed(format!("missing exported function `{name}`"))
            })?
            .into_func()
            .ok_or_else(|| {
                XtaskError::TypstPluginSmokeFailed(format!("export `{name}` is not a function"))
            })?;

        let ty = handle.ty(&self.store);
        if ty.params().iter().any(|&val| val != ValType::I32) {
            return Err(XtaskError::TypstPluginSmokeFailed(format!(
                "plugin function `{name}` has a non-i32 parameter"
            )));
        }
        if ty.results() != [ValType::I32] {
            return Err(XtaskError::TypstPluginSmokeFailed(format!(
                "plugin function `{name}` does not return exactly one i32"
            )));
        }
        if ty.params().len() != args.len() {
            return Err(XtaskError::TypstPluginSmokeFailed(format!(
                "plugin function `{name}` expects {} arguments, got {}",
                ty.params().len(),
                args.len()
            )));
        }

        let lengths = args
            .iter()
            .map(|arg| {
                i32::try_from(arg.len()).map(Val::I32).map_err(|_| {
                    smoke_error(format!(
                        "plugin function `{name}` argument exceeds the i32 protocol length"
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.store.data_mut().args = args;
        self.store.data_mut().output.clear();
        self.store.data_mut().memory_error = None;

        let mut code = Val::I32(-1);
        handle
            .call(&mut self.store, &lengths, std::slice::from_mut(&mut code))
            .map_err(|source| {
                XtaskError::TypstPluginSmokeFailed(format!("plugin panicked: {source}"))
            })?;

        if let Some(error) = self.store.data_mut().memory_error.take() {
            return Err(XtaskError::TypstPluginSmokeFailed(format!(
                "plugin tried to {} out of bounds at pointer {:#x} with length {}",
                if error.write { "write" } else { "read" },
                error.offset,
                error.length
            )));
        }

        let output = std::mem::take(&mut self.store.data_mut().output);
        match code {
            Val::I32(0) => Ok(output),
            Val::I32(1) => {
                let message = String::from_utf8_lossy(&output);
                Err(XtaskError::TypstPluginSmokeFailed(format!(
                    "plugin returned an error: {message}"
                )))
            }
            _ => Err(XtaskError::TypstPluginSmokeFailed(
                "plugin did not respect the wasm-minimal-protocol return code".to_string(),
            )),
        }
    }
}

fn wasm_minimal_protocol_write_args_to_buffer(mut caller: Caller<CallData>, ptr: u32) {
    let Some(memory) = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
    else {
        caller.data_mut().memory_error = Some(MemoryError {
            offset: ptr,
            length: 0,
            write: true,
        });
        return;
    };

    let args = std::mem::take(&mut caller.data_mut().args);
    let mut offset = ptr as usize;
    for arg in args {
        if memory.write(&mut caller, offset, arg.as_slice()).is_err() {
            caller.data_mut().memory_error = Some(MemoryError {
                offset: offset as u32,
                length: arg.len() as u32,
                write: true,
            });
            return;
        }
        offset += arg.len();
    }
}

fn wasm_minimal_protocol_send_result_to_host(mut caller: Caller<CallData>, ptr: u32, len: u32) {
    let Some(memory) = caller
        .get_export("memory")
        .and_then(|export| export.into_memory())
    else {
        caller.data_mut().memory_error = Some(MemoryError {
            offset: ptr,
            length: len,
            write: false,
        });
        return;
    };

    let start = ptr as usize;
    let length = len as usize;
    let Some(end) = start.checked_add(length) else {
        caller.data_mut().memory_error = Some(MemoryError {
            offset: ptr,
            length: len,
            write: false,
        });
        return;
    };
    if end > memory.data(&caller).len() {
        caller.data_mut().memory_error = Some(MemoryError {
            offset: ptr,
            length: len,
            write: false,
        });
        return;
    }

    let mut output = std::mem::take(&mut caller.data_mut().output);
    output.resize(length, 0);
    if memory.read(&caller, start, &mut output).is_err() {
        caller.data_mut().memory_error = Some(MemoryError {
            offset: ptr,
            length: len,
            write: false,
        });
        return;
    }
    caller.data_mut().output = output;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_artifact_recipe_owns_publish_capabilities() {
        let catalog = load_typst_profiles().expect("Typst package descriptor");
        let artifact = canonical_typst_artifact_profile(&catalog).expect("Typst artifact recipe");

        assert_eq!(
            artifact.id,
            catalog.artifact_profile_id(),
            "the package profile must refer to the one canonical artifact recipe"
        );
        assert!(artifact.capabilities.contains(&"svg".to_string()));
        assert!(artifact.capabilities.contains(&"analysis".to_string()));
        assert_eq!(artifact.runtime_ids, artifact.capabilities);
        assert_eq!(
            expected_typst_operation_ids(&artifact),
            [OperationKey::AnalysisJson.id(), OperationKey::Svg.id()]
        );
    }

    #[test]
    fn typst_operation_projection_intersects_the_transport_allowlist_with_the_artifact() {
        let catalog = load_typst_profiles().expect("Typst package descriptor");
        let canonical = canonical_typst_artifact_profile(&catalog).expect("Typst artifact recipe");

        let mut analysis_only = canonical.clone();
        analysis_only.capabilities = vec!["analysis".to_string()];
        analysis_only.outputs.clear();
        assert_eq!(
            expected_typst_operation_ids(&analysis_only),
            [OperationKey::AnalysisJson.id()]
        );

        let mut svg_only = canonical.clone();
        svg_only.capabilities = vec!["svg".to_string()];
        svg_only.outputs = vec!["svg".to_string()];
        assert_eq!(
            expected_typst_operation_ids(&svg_only),
            [OperationKey::Svg.id()]
        );

        let mut svg_with_layouts = svg_only;
        svg_with_layouts.capabilities = vec![
            "layout-cytoscape".to_string(),
            "layout-elk".to_string(),
            "svg".to_string(),
        ];
        assert_eq!(
            expected_typst_operation_ids(&svg_with_layouts),
            [OperationKey::Svg.id()],
            "supplemental capabilities must not expand the closed Typst transport"
        );
    }

    #[test]
    fn capability_catalog_allows_additive_fields_within_the_current_schema() {
        let profiles = load_typst_profiles().expect("Typst package descriptor");
        let artifact = canonical_typst_artifact_profile(&profiles).expect("Typst artifact recipe");
        let operation_ids = expected_typst_operation_ids(&artifact);
        let output_contracts = artifact
            .outputs
            .iter()
            .map(|id| {
                json!({
                    "id": id,
                    "media_type": "application/octet-stream",
                    "system_fonts": null,
                    "embedded_images": null,
                })
            })
            .collect::<Vec<_>>();
        let mut payload = json!({
            "schema_version": merman_typst_plugin::TYPST_RUNTIME_CATALOG_SCHEMA_VERSION,
            "transport_api_version": merman_typst_plugin::TYPST_PLUGIN_ABI_VERSION,
            "package_version": String::from_utf8(merman_typst_plugin::package_version())
                .expect("UTF-8 package version"),
            "options_schema_versions": [merman_bindings_core::BINDING_OPTIONS_SCHEMA_VERSION],
            "payload_schemas": [],
            "metadata_ids": [],
            "capabilities": {
                "capability_ids": artifact.capabilities,
                "output_ids": artifact.outputs,
                "operation_ids": operation_ids,
                "system_adapter_ids": [],
                "text_measurement": {
                    "protocol_version": 1,
                    "provider_ids": ["deterministic"],
                    "future_measurement_metadata": true,
                },
                "future_capability_metadata": {},
            },
            "output_contracts": output_contracts,
            "registry": {
                "diagram_family_count": 35,
                "future_registry_metadata": true,
            },
            "resources": {
                "general_binding_default_profile": "interactive",
                "cli_default_profile": "trusted-native",
                "limits": [],
                "profiles": [{ "id": "constrained" }],
                "future_resource_metadata": true,
            },
            "future_catalog_metadata": {},
        });

        assert_capability_catalog(&payload, &artifact)
            .expect("schema-1 Typst catalogs must tolerate additive fields");

        for field in [
            "options_schema_versions",
            "payload_schemas",
            "metadata_ids",
            "output_contracts",
        ] {
            let mut missing = payload.clone();
            missing
                .as_object_mut()
                .expect("runtime catalog")
                .remove(field);
            assert!(
                assert_capability_catalog(&missing, &artifact)
                    .unwrap_err()
                    .to_string()
                    .contains(field),
                "missing original schema-1 field `{field}` must fail"
            );
        }

        let mut explicit_additive_sections = payload.clone();
        explicit_additive_sections["option_group_ids"] =
            json!(expected_typst_option_group_ids(&artifact));
        explicit_additive_sections["constructor_service_ids"] = json!([]);
        explicit_additive_sections["constructor_service_contracts"] = json!([]);
        assert_capability_catalog(&explicit_additive_sections, &artifact)
            .expect("present additive sections must satisfy their cross-field contracts");

        explicit_additive_sections
            .as_object_mut()
            .expect("runtime catalog")
            .remove("constructor_service_contracts");
        assert!(
            assert_capability_catalog(&explicit_additive_sections, &artifact)
                .unwrap_err()
                .to_string()
                .contains("must appear together")
        );

        payload["resources"]
            .as_object_mut()
            .expect("resource catalog")
            .remove("profiles");
        assert!(
            assert_capability_catalog(&payload, &artifact)
                .unwrap_err()
                .to_string()
                .contains("missing required fields: [profiles]")
        );
    }

    #[test]
    fn render_payload_requires_the_closed_envelope_shape() {
        let payload = json!({
            "version": 1,
            "operation": "render-svg",
            "ok": true,
            "code": 0,
            "code_name": "MERMAN_OK",
            "kind": null,
            "capability_id": null,
            "message": null,
            "data": {
                "svg": "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
            },
        });
        assert!(assert_render_payload(&payload).is_ok());

        let mut extra = payload;
        extra["legacy"] = JsonValue::Bool(true);
        assert!(
            assert_render_payload(&extra)
                .unwrap_err()
                .to_string()
                .contains("extra [legacy]")
        );
    }

    #[test]
    fn analysis_payload_requires_a_closed_envelope_and_closed_canonical_data() {
        let payload = analysis_envelope(valid_analysis_payload());
        assert!(assert_analysis_payload(&payload, false).is_ok());

        let mut extra = payload.clone();
        extra["data"]["analysis"]["source"]["legacy"] = JsonValue::Bool(true);
        assert!(
            assert_analysis_payload(&extra, false)
                .unwrap_err()
                .to_string()
                .contains("not closed")
        );

        let diagnostic_payload = analysis_envelope(json!({
            "version": 1,
            "valid": false,
            "summary": { "errors": 1, "warnings": 0, "infos": 0, "hints": 0 },
            "source": {
                "kind": "diagram",
                "path": null,
                "diagram_index": null,
                "language": "mermaid",
            },
            "diagnostics": [{
                "id": "parse.no_diagram",
                "severity": "error",
                "category": "parse",
                "message": "No diagram found",
                "code": 4,
                "code_name": "MERMAN_NO_DIAGRAM",
                "diagram_type": null,
                "span": null,
                "related": [],
                "help": null,
            }],
        }));
        assert!(assert_analysis_payload(&diagnostic_payload, true).is_ok());
    }

    fn analysis_envelope(analysis: JsonValue) -> JsonValue {
        json!({
            "version": 1,
            "operation": "analyze",
            "ok": true,
            "code": 0,
            "code_name": "MERMAN_OK",
            "kind": null,
            "capability_id": null,
            "message": null,
            "data": { "analysis": analysis },
        })
    }

    fn valid_analysis_payload() -> JsonValue {
        json!({
            "version": 1,
            "valid": true,
            "summary": { "errors": 0, "warnings": 0, "infos": 0, "hints": 0 },
            "source": {
                "kind": "diagram",
                "path": null,
                "diagram_index": null,
                "language": "mermaid",
            },
            "diagnostics": [],
        })
    }
}

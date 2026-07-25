use super::{
    artifact_profiles::{WasmArtifactProfile, load_wasm_size_artifact_profiles},
    typst_profiles::{
        TypstPackageProfile, TypstProfileCatalog, load_typst_profiles,
        validate_typst_artifact_profiles,
    },
    wasm_module_surface::{LoadedWasmModule, WasmModuleLoadError, WasmSurfaceProfile},
};
use crate::XtaskError;
use serde_json::{Map as JsonMap, Value as JsonValue};
use std::path::{Path, PathBuf};
use wasmi::{Caller, Linker, Store, Val, ValType};

const DEFAULT_SOURCE: &[u8] = b"flowchart TD\nA[Hello] --> B[World]";
const DEFAULT_OPTIONS_JSON: &[u8] =
    br#"{"fixed_today":"2026-06-10","fixed_local_offset_minutes":480}"#;
const TYPST_RESULT_PAYLOAD_SCHEMA_VERSION: u64 = 1;

#[derive(Debug)]
struct TypstPluginSmokeOptions {
    wasm_file: PathBuf,
    profile: Option<String>,
    source: Vec<u8>,
    options_json: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TypstPluginValidationReport {
    render_output_bytes: usize,
    svg_bytes: usize,
    analysis_output_bytes: usize,
}

impl TypstPluginValidationReport {
    pub(crate) const fn render_output_bytes(self) -> usize {
        self.render_output_bytes
    }

    pub(crate) const fn svg_bytes(self) -> usize {
        self.svg_bytes
    }

    pub(crate) const fn analysis_output_bytes(self) -> usize {
        self.analysis_output_bytes
    }
}

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

pub(crate) fn typst_plugin_smoke(args: Vec<String>) -> Result<(), XtaskError> {
    let options = parse_options(args)?;
    let catalog = load_typst_profiles()?;
    let profile = catalog.resolve_package(options.profile.as_deref())?;
    let report = if options.source == DEFAULT_SOURCE && options.options_json == DEFAULT_OPTIONS_JSON
    {
        validate_typst_plugin(&options.wasm_file, &catalog, profile)?
    } else {
        validate_typst_plugin_with_input(
            &options.wasm_file,
            &catalog,
            profile,
            &options.source,
            &options.options_json,
        )?
    };
    println!(
        "typst-plugin-smoke OK wasm={} profile={} render_output_bytes={} svg_bytes={} analysis_output_bytes={}",
        options.wasm_file.display(),
        profile.name(),
        report.render_output_bytes(),
        report.svg_bytes(),
        report.analysis_output_bytes(),
    );
    Ok(())
}

fn parse_options(args: Vec<String>) -> Result<TypstPluginSmokeOptions, XtaskError> {
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        print_usage();
        return Err(XtaskError::Usage);
    }

    let mut wasm_file = None;
    let mut profile = None;
    let mut source = DEFAULT_SOURCE.to_vec();
    let mut options_json = DEFAULT_OPTIONS_JSON.to_vec();

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--wasm" => {
                wasm_file = Some(PathBuf::from(iter.next().ok_or(XtaskError::Usage)?));
            }
            "--profile" => {
                profile = Some(iter.next().ok_or(XtaskError::Usage)?);
            }
            "--source" => {
                source = iter.next().ok_or(XtaskError::Usage)?.into_bytes();
            }
            "--source-file" => {
                let path = PathBuf::from(iter.next().ok_or(XtaskError::Usage)?);
                source = std::fs::read(&path).map_err(|source| XtaskError::ReadFile {
                    path: path.display().to_string(),
                    source,
                })?;
            }
            "--options-json" => {
                options_json = iter.next().ok_or(XtaskError::Usage)?.into_bytes();
            }
            "--options-json-file" => {
                let path = PathBuf::from(iter.next().ok_or(XtaskError::Usage)?);
                options_json = std::fs::read(&path).map_err(|source| XtaskError::ReadFile {
                    path: path.display().to_string(),
                    source,
                })?;
            }
            _ => {
                print_usage();
                return Err(XtaskError::Usage);
            }
        }
    }

    let wasm_file = wasm_file.ok_or_else(|| {
        print_usage();
        XtaskError::Usage
    })?;

    Ok(TypstPluginSmokeOptions {
        wasm_file,
        profile,
        source,
        options_json,
    })
}

fn print_usage() {
    println!("usage: xtask typst-plugin-smoke --wasm <plugin.wasm> [options]");
    println!();
    println!("Options:");
    println!("  --profile <name>             Public package profile (default: publish)");
    println!("  --source <text>              Mermaid source bytes to pass to render_svg_json");
    println!("  --source-file <path>         Read Mermaid source bytes from a file");
    println!("  --options-json <json>        Options JSON bytes to pass to render_svg_json");
    println!("  --options-json-file <path>   Read options JSON bytes from a file");
}

pub(crate) fn validate_typst_plugin(
    wasm_file: &Path,
    catalog: &TypstProfileCatalog,
    profile: &TypstPackageProfile,
) -> Result<TypstPluginValidationReport, XtaskError> {
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
) -> Result<TypstPluginValidationReport, XtaskError> {
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
    let svg_bytes = assert_render_payload(&render_payload)?;

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

    Ok(TypstPluginValidationReport {
        render_output_bytes: render_output.len(),
        svg_bytes,
        analysis_output_bytes: analysis_output.len(),
    })
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
            "package_version",
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
    let operation_ids = string_array(
        runtime_capabilities
            .get("operation_ids")
            .expect("closed runtime capabilities object"),
        "Typst runtime operation IDs",
    )?;
    let expected_operation_ids = merman_typst_plugin::TYPST_BINDING_OPERATION_IDS
        .iter()
        .map(|id| (*id).to_string())
        .collect::<Vec<_>>();
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
        || text_measurement_provider_ids != vec!["vendored".to_string()]
    {
        return Err(smoke_error(format!(
            "Typst text measurement metadata must expose only the vendored provider: {}",
            JsonValue::Object(text_measurement.clone())
        )));
    }

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
    fn cli_defaults_to_publish_and_accepts_only_public_profile_aliases() {
        let options = parse_options(vec!["--wasm".to_string(), "plugin.wasm".to_string()])
            .expect("default smoke options");
        let catalog = load_typst_profiles().expect("Typst package descriptor");

        assert!(options.profile.is_none());
        assert_eq!(
            catalog
                .resolve_package(options.profile.as_deref())
                .expect("default publish profile")
                .name(),
            "publish"
        );

        let options = parse_options(vec![
            "--wasm".to_string(),
            "plugin.wasm".to_string(),
            "--profile".to_string(),
            "publish".to_string(),
        ])
        .expect("named smoke profile");
        assert!(catalog.resolve_package(options.profile.as_deref()).is_ok());
        for private_name in [
            "minimal",
            "typst-full-elk",
            "typst-bridge",
            "typst-render-only-no-elk",
        ] {
            assert!(catalog.resolve_package(Some(private_name)).is_err());
        }
    }

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
    }

    #[test]
    fn capability_catalog_allows_additive_fields_within_the_current_schema() {
        let profiles = load_typst_profiles().expect("Typst package descriptor");
        let artifact = canonical_typst_artifact_profile(&profiles).expect("Typst artifact recipe");
        let mut payload = json!({
            "schema_version": merman_typst_plugin::TYPST_RUNTIME_CATALOG_SCHEMA_VERSION,
            "transport_api_version": merman_typst_plugin::TYPST_PLUGIN_ABI_VERSION,
            "package_version": String::from_utf8(merman_typst_plugin::package_version())
                .expect("UTF-8 package version"),
            "capabilities": {
                "capability_ids": artifact.capabilities,
                "output_ids": artifact.outputs,
                "operation_ids": merman_typst_plugin::TYPST_BINDING_OPERATION_IDS,
                "system_adapter_ids": [],
                "text_measurement": {
                    "protocol_version": 1,
                    "provider_ids": ["vendored"],
                    "future_measurement_metadata": true,
                },
                "future_capability_metadata": {},
            },
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

        assert!(assert_capability_catalog(&payload, &artifact).is_ok());

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

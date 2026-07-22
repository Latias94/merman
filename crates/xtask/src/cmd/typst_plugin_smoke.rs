use super::typst_profiles::{
    TypstProfileCapabilities, TypstProfileCatalog, TypstWasmProfile, load_typst_profiles,
};
use super::wasm_module_surface::{LoadedWasmModule, WasmModuleLoadError, WasmSurfaceProfile};
use crate::XtaskError;
use serde_json::{Map as JsonMap, Value as JsonValue, json};
use std::path::{Path, PathBuf};
use wasmi::{Caller, Linker, Store, Val, ValType};

const DEFAULT_SOURCE: &[u8] = b"flowchart TD\nA[Hello] --> B[World]";
const DEFAULT_OPTIONS_JSON: &[u8] =
    br#"{"fixed_today":"2026-06-10","fixed_local_offset_minutes":480}"#;
const PAYLOAD_SCHEMA_VERSION: u64 = 1;

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
    profile: &TypstWasmProfile,
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
    profile: &TypstWasmProfile,
    source: &[u8],
    options_json: &[u8],
) -> Result<TypstPluginValidationReport, XtaskError> {
    validate_requested_profile(catalog, profile)?;
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
    let expected_capabilities = expected_capabilities_json(profile.capabilities());
    if capabilities != expected_capabilities {
        return Err(smoke_error(format!(
            "capabilities_json does not match profile `{}`: expected {expected_capabilities}, found {capabilities}",
            profile.name()
        )));
    }

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

    Ok(TypstPluginValidationReport {
        render_output_bytes: render_output.len(),
        svg_bytes,
        analysis_output_bytes: analysis_output.len(),
    })
}

fn validate_requested_profile(
    catalog: &TypstProfileCatalog,
    profile: &TypstWasmProfile,
) -> Result<(), XtaskError> {
    let crate_abi_version = merman_typst_plugin::TYPST_PLUGIN_ABI_VERSION;
    if catalog.plugin_abi_version() != crate_abi_version {
        return Err(smoke_error(format!(
            "Typst profile descriptor declares ABI {}; plugin declares ABI {crate_abi_version}",
            catalog.plugin_abi_version(),
        )));
    }
    let is_public = catalog.public_profile_names().into_iter().any(|alias| {
        catalog
            .resolve_package(Some(alias))
            .is_ok_and(|candidate| candidate.name() == profile.name())
    });
    if !is_public {
        return Err(smoke_error(format!(
            "profile `{}` is not a public Typst package profile",
            profile.name()
        )));
    }
    if !profile.capabilities().render || !profile.capabilities().analysis {
        return Err(smoke_error(format!(
            "public Typst package profile `{}` must support both render and analysis",
            profile.name()
        )));
    }
    Ok(())
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

fn expected_capabilities_json(capabilities: &TypstProfileCapabilities) -> JsonValue {
    json!({
        "render": capabilities.render,
        "analysis": capabilities.analysis,
        "ascii": capabilities.ascii,
        "core_host": capabilities.core_host,
        "cytoscape_layout": capabilities.cytoscape_layout,
        "elk_layout": capabilities.elk_layout,
        "ratex_math": capabilities.ratex_math,
        "editor_language": capabilities.editor_language,
        "text_measurement": {
            "vendored": capabilities.text_measurement.vendored,
            "deterministic": capabilities.text_measurement.deterministic,
            "host_callback": capabilities.text_measurement.host_callback,
            "font_assets": capabilities.text_measurement.font_assets,
        }
    })
}

fn assert_render_payload(payload: &JsonValue) -> Result<usize, XtaskError> {
    let object = exact_object(
        payload,
        &["version", "ok", "code", "code_name", "message", "svg"],
        "render payload",
    )?;
    if object.get("version").and_then(JsonValue::as_u64) != Some(PAYLOAD_SCHEMA_VERSION)
        || object.get("ok").and_then(JsonValue::as_bool) != Some(true)
        || object.get("code").and_then(JsonValue::as_i64) != Some(0)
        || object.get("code_name").and_then(JsonValue::as_str) != Some("MERMAN_OK")
        || !object.get("message").is_some_and(JsonValue::is_null)
    {
        return Err(smoke_error(format!(
            "render_svg_json returned an invalid success payload: {payload}"
        )));
    }
    let svg = object
        .get("svg")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| smoke_error("render payload `svg` must be a string"))?;
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
    let canonical: merman_analysis::AnalysisPayload = serde_json::from_value(payload.clone())
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
    if canonical_json != *payload {
        return Err(smoke_error(format!(
            "analyze_json payload is not closed under canonical analysis schema {}: expected {canonical_json}, found {payload}",
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

    #[test]
    fn cli_defaults_to_publish_and_accepts_only_public_profile_aliases() {
        let options = parse_options(vec!["--wasm".to_string(), "plugin.wasm".to_string()])
            .expect("default smoke options");
        let catalog = load_typst_profiles().expect("Typst profile descriptor");

        assert!(options.profile.is_none());
        assert_eq!(
            catalog
                .resolve_package(options.profile.as_deref())
                .expect("default publish profile")
                .name(),
            "typst-full-elk"
        );

        for alias in ["publish", "minimal"] {
            let options = parse_options(vec![
                "--wasm".to_string(),
                "plugin.wasm".to_string(),
                "--profile".to_string(),
                alias.to_string(),
            ])
            .expect("named smoke profile");
            assert!(catalog.resolve_package(options.profile.as_deref()).is_ok());
        }
        for private_name in ["typst-full-elk", "typst-bridge", "typst-render-only-no-elk"] {
            assert!(catalog.resolve_package(Some(private_name)).is_err());
        }
    }

    #[test]
    fn publish_capabilities_are_exactly_descriptor_owned() {
        let catalog = load_typst_profiles().expect("Typst profile descriptor");
        let publish = catalog
            .resolve_package(Some("publish"))
            .expect("publish profile");

        assert_eq!(
            expected_capabilities_json(publish.capabilities()),
            json!({
                "render": true,
                "analysis": true,
                "ascii": false,
                "core_host": false,
                "cytoscape_layout": true,
                "elk_layout": true,
                "ratex_math": false,
                "editor_language": false,
                "text_measurement": {
                    "vendored": true,
                    "deterministic": true,
                    "host_callback": false,
                    "font_assets": false,
                }
            })
        );
    }

    #[test]
    fn render_payload_requires_the_closed_schema_one_shape() {
        let payload = json!({
            "version": 1,
            "ok": true,
            "code": 0,
            "code_name": "MERMAN_OK",
            "message": null,
            "svg": "<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
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
    fn analysis_payload_requires_closed_schema_one_objects() {
        let payload = valid_analysis_payload();
        assert!(assert_analysis_payload(&payload, false).is_ok());

        let mut extra = payload.clone();
        extra["source"]["legacy"] = JsonValue::Bool(true);
        assert!(
            assert_analysis_payload(&extra, false)
                .unwrap_err()
                .to_string()
                .contains("not closed")
        );

        let diagnostic_payload = json!({
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
        });
        assert!(assert_analysis_payload(&diagnostic_payload, true).is_ok());
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

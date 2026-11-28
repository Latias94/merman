#![forbid(unsafe_code)]

//! WebAssembly bindings for browser integrations.
//!
//! The crate intentionally stays thin: binding option parsing, rendering, and error classification
//! are delegated to `merman-bindings-core`. It only extracts browser-transport fields such as the
//! optional operation deadline before dispatch.

use merman_bindings_core::{
    ArtifactContractSpec, BindingError, BindingOperationRequest, BindingOperationResult,
    BindingTransportKey, CapabilityKey, ConstructorServiceKey, OperationControl, OperationKey,
    RuntimeCatalog, RuntimePolicyExposure, TargetKey, TransportCompiledExtensionKey,
    ValidatedArtifactContract, WEB_TRANSPORT_API_VERSION,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use wasm_bindgen::prelude::*;

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
use std::{cell::RefCell, sync::Arc};

#[cfg(feature = "svg")]
mod render_source;
#[cfg(feature = "svg")]
pub use render_source::WasmPreparedSource;

#[cfg(feature = "svg")]
mod render_session;
#[cfg(feature = "svg")]
pub use render_session::WasmRenderSession;

#[cfg(feature = "editor")]
mod editor_language;

#[cfg(feature = "editor")]
pub use editor_language::{
    WasmEditorSession, editor_code_actions, editor_completion_trigger_characters,
    editor_completions, editor_definition, editor_diagnostics, editor_diagram_detection,
    editor_document_symbols, editor_hover, editor_prepare_rename, editor_references, editor_rename,
    editor_search_document_symbols,
};

#[cfg(all(feature = "svg", any(target_arch = "wasm32", test)))]
use merman_bindings_core::WrapMode;

/// Breaking API version for the wasm-bindgen transport.
///
/// This is independent from the native C ABI and the Typst plugin ABI. It changes when the
/// JavaScript/WASM export or runtime-contract wire shape becomes incompatible.
pub const WASM_TRANSPORT_API_VERSION: u32 = WEB_TRANSPORT_API_VERSION;
const WASM_TIMEOUT_MS_MAX: u64 = u32::MAX as u64;
const WASM_OPERATIONS: &[OperationKey] = &[
    #[cfg(feature = "analysis")]
    OperationKey::AnalysisFactsJson,
    #[cfg(feature = "analysis")]
    OperationKey::AnalysisJson,
    #[cfg(feature = "ascii")]
    OperationKey::Ascii,
    #[cfg(feature = "analysis")]
    OperationKey::DocumentAnalysisFactsJson,
    #[cfg(feature = "analysis")]
    OperationKey::DocumentAnalysisJson,
    #[cfg(feature = "svg")]
    OperationKey::EdgeGeometryJson,
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
    #[cfg(feature = "analysis")]
    OperationKey::ValidationJson,
];
const WASM_SUPPLEMENTAL_CAPABILITIES: &[CapabilityKey] = merman_bindings_core::mermaid_layout_capabilities![
    #[cfg(feature = "fixes")]
    CapabilityKey::Fixes,
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
const WASM_TRANSPORT_EXTENSIONS: &[TransportCompiledExtensionKey] = &[
    #[cfg(feature = "editor")]
    TransportCompiledExtensionKey::Editor,
];
const WASM_CONSTRUCTOR_SERVICES: &[ConstructorServiceKey] = &[
    #[cfg(all(feature = "svg", target_arch = "wasm32"))]
    ConstructorServiceKey::HostTextMeasurement,
    #[cfg(feature = "svg")]
    ConstructorServiceKey::IconRegistry,
];

// Keep feature selection in the transport owner. Dependency features may be unified by Cargo.
static ARTIFACT_CONTRACT: ValidatedArtifactContract =
    ArtifactContractSpec::new(TargetKey::Web, BindingTransportKey::Web)
        .with_operations(WASM_OPERATIONS)
        .with_supplemental_capabilities(WASM_SUPPLEMENTAL_CAPABILITIES)
        .with_all_available_metadata()
        .with_constructor_services(WASM_CONSTRUCTOR_SERVICES)
        .with_runtime_policy_exposure(RuntimePolicyExposure::DeterministicOnly)
        .with_transport_extensions(WASM_TRANSPORT_EXTENSIONS)
        .materialize();

fn wasm_artifact_contract() -> &'static ValidatedArtifactContract {
    &ARTIFACT_CONTRACT
}

fn wasm_runtime_catalog() -> RuntimeCatalog {
    wasm_artifact_contract().runtime_catalog(WASM_TRANSPORT_API_VERSION)
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen(js_name = transportApiVersion)]
pub fn transport_api_version() -> u32 {
    WASM_TRANSPORT_API_VERSION
}

#[wasm_bindgen(js_name = packageVersion)]
pub fn package_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[wasm_bindgen(js_name = renderSvg)]
pub fn render_svg(source: &str, options_json: Option<String>) -> Result<String, JsValue> {
    string_result(execute_wasm_operation(
        "svg",
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
        None,
    ))
}

/// Renders once and retains the execution metadata for the returned SVG.
#[wasm_bindgen(js_name = renderSvgResult)]
pub fn render_svg_result(source: &str, options_json: Option<String>) -> Result<JsValue, JsValue> {
    text_operation_result(
        execute_wasm_operation_result(
            "svg",
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
            None,
        ),
        "svg",
    )
}

#[wasm_bindgen(js_name = svgPlanJson)]
pub fn svg_plan_json(source: &str, options_json: Option<String>) -> Result<JsValue, JsValue> {
    json_value_result(execute_wasm_operation(
        "svg-plan-json",
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
        None,
    ))
}

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
#[wasm_bindgen(js_name = renderSvgWithTextMeasurer)]
pub fn render_svg_with_text_measurer(
    source: &str,
    options_json: Option<String>,
    callback: js_sys::Function,
) -> Result<String, JsValue> {
    with_host_text_measure_callback(callback, || {
        let services = merman_bindings_core::BindingEngineServices::new()
            .with_host_text_measurer(Arc::new(WasmHostTextMeasurer));
        string_result(execute_wasm_operation_with_services(
            "svg",
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
            services,
        ))
    })
}

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
#[wasm_bindgen(js_name = renderSvgResultWithTextMeasurer)]
pub fn render_svg_result_with_text_measurer(
    source: &str,
    options_json: Option<String>,
    callback: js_sys::Function,
) -> Result<JsValue, JsValue> {
    with_host_text_measure_callback(callback, || {
        let services = merman_bindings_core::BindingEngineServices::new()
            .with_host_text_measurer(Arc::new(WasmHostTextMeasurer));
        text_operation_result(
            execute_wasm_operation_result_with_services(
                "svg",
                source.as_bytes(),
                options_bytes(options_json.as_deref()),
                services,
            ),
            "svg",
        )
    })
}

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
#[wasm_bindgen(js_name = layoutJsonWithTextMeasurer)]
pub fn layout_json_with_text_measurer(
    source: &str,
    options_json: Option<String>,
    callback: js_sys::Function,
) -> Result<String, JsValue> {
    with_host_text_measure_callback(callback, || {
        let services = merman_bindings_core::BindingEngineServices::new()
            .with_host_text_measurer(Arc::new(WasmHostTextMeasurer));
        string_result(execute_wasm_operation_with_services(
            "layout-json",
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
            services,
        ))
    })
}

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
#[wasm_bindgen(js_name = edgeGeometryJsonWithTextMeasurer)]
pub fn edge_geometry_json_with_text_measurer(
    source: &str,
    options_json: Option<String>,
    callback: js_sys::Function,
) -> Result<String, JsValue> {
    with_host_text_measure_callback(callback, || {
        let services = merman_bindings_core::BindingEngineServices::new()
            .with_host_text_measurer(Arc::new(WasmHostTextMeasurer));
        string_result(execute_wasm_operation_with_services(
            "edge-geometry-json",
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
            services,
        ))
    })
}

#[wasm_bindgen(js_name = parseJson)]
pub fn parse_json(source: &str, options_json: Option<String>) -> Result<String, JsValue> {
    string_result(execute_wasm_operation(
        "semantic-json",
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
        None,
    ))
}

#[wasm_bindgen(js_name = layoutJson)]
pub fn layout_json(source: &str, options_json: Option<String>) -> Result<String, JsValue> {
    string_result(execute_wasm_operation(
        "layout-json",
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
        None,
    ))
}

#[wasm_bindgen(js_name = edgeGeometryJson)]
pub fn edge_geometry_json(source: &str, options_json: Option<String>) -> Result<String, JsValue> {
    string_result(execute_wasm_operation(
        "edge-geometry-json",
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
        None,
    ))
}

#[wasm_bindgen(js_name = renderAscii)]
pub fn render_ascii(source: &str, options_json: Option<String>) -> Result<String, JsValue> {
    string_result(execute_wasm_operation(
        "ascii",
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
        None,
    ))
}

/// Renders once and retains the execution metadata for the returned ASCII text.
#[wasm_bindgen(js_name = renderAsciiResult)]
pub fn render_ascii_result(source: &str, options_json: Option<String>) -> Result<JsValue, JsValue> {
    text_operation_result(
        execute_wasm_operation_result(
            "ascii",
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
            None,
        ),
        "text",
    )
}

#[wasm_bindgen]
pub fn analyze(source: &str, options_json: Option<String>) -> Result<JsValue, JsValue> {
    json_value_result(execute_wasm_operation(
        "analysis-json",
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
        None,
    ))
}

#[wasm_bindgen(js_name = analyzeJson)]
pub fn analyze_json(source: &str, options_json: Option<String>) -> Result<JsValue, JsValue> {
    analyze(source, options_json)
}

#[wasm_bindgen(js_name = analysisFacts)]
pub fn analysis_facts(source: &str, options_json: Option<String>) -> Result<JsValue, JsValue> {
    json_value_result(execute_wasm_operation(
        "analysis-facts-json",
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
        None,
    ))
}

#[wasm_bindgen(js_name = analyzeDocument)]
pub fn analyze_document(
    source: &str,
    uri: String,
    options_json: Option<String>,
) -> Result<JsValue, JsValue> {
    json_value_result(execute_wasm_operation(
        "document-analysis-json",
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
        Some(uri.as_bytes()),
    ))
}

#[wasm_bindgen(js_name = analyzeDocumentFacts)]
pub fn analyze_document_facts(
    source: &str,
    uri: String,
    options_json: Option<String>,
) -> Result<JsValue, JsValue> {
    json_value_result(execute_wasm_operation(
        "document-analysis-facts-json",
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
        Some(uri.as_bytes()),
    ))
}

#[wasm_bindgen]
pub fn validate(source: &str, options_json: Option<String>) -> Result<JsValue, JsValue> {
    json_value_result(execute_wasm_operation(
        "validation-json",
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
        None,
    ))
}

#[wasm_bindgen(js_name = supportedDiagrams)]
pub fn supported_diagrams() -> Result<JsValue, JsValue> {
    json_value_result(wasm_artifact_contract().metadata_json("supported-diagrams"))
}

#[wasm_bindgen(js_name = runtimeCatalog)]
pub fn runtime_catalog() -> Result<JsValue, JsValue> {
    wasm_runtime_catalog()
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen(js_name = diagramFamilyCapabilities)]
pub fn diagram_family_capabilities() -> Result<JsValue, JsValue> {
    json_value_result(wasm_artifact_contract().metadata_json("diagram-family-capabilities"))
}

#[wasm_bindgen(js_name = lintRuleCatalog)]
pub fn lint_rule_catalog() -> Result<JsValue, JsValue> {
    json_value_result(wasm_artifact_contract().metadata_json("lint-rule-catalog"))
}

#[wasm_bindgen(js_name = supportedThemes)]
pub fn supported_themes() -> Result<JsValue, JsValue> {
    json_value_result(wasm_artifact_contract().metadata_json("supported-themes"))
}

#[wasm_bindgen(js_name = themeCatalog)]
pub fn theme_catalog() -> Result<JsValue, JsValue> {
    json_value_result(wasm_artifact_contract().metadata_json("theme-catalog"))
}

#[cfg(feature = "svg")]
#[wasm_bindgen(js_name = exportThemePreset)]
pub fn export_theme_preset(preset_id: &str) -> Result<JsValue, JsValue> {
    json_value_result(execute_wasm_operation(
        "export-theme-preset",
        preset_id.as_bytes(),
        b"",
        None,
    ))
}

#[cfg(feature = "svg")]
#[wasm_bindgen(js_name = resolveTheme)]
pub fn resolve_theme(request_json: &str) -> Result<JsValue, JsValue> {
    json_value_result(execute_wasm_operation(
        "resolve-theme",
        request_json.as_bytes(),
        b"",
        None,
    ))
}

#[wasm_bindgen(js_name = asciiSupportedDiagrams)]
pub fn ascii_supported_diagrams() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(wasm_ascii_supported_diagrams())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen(js_name = asciiCapabilities)]
pub fn ascii_capabilities() -> Result<JsValue, JsValue> {
    json_value_result(wasm_artifact_contract().metadata_json("ascii-capabilities"))
}

fn wasm_ascii_supported_diagrams() -> &'static [&'static str] {
    if wasm_artifact_contract()
        .runtime_capabilities()
        .has_capability("ascii")
    {
        merman_bindings_core::ascii_supported_diagrams()
    } else {
        &[]
    }
}

fn options_bytes(options_json: Option<&str>) -> &[u8] {
    options_json.unwrap_or_default().as_bytes()
}

/// Extracts the WASM transport's optional relative deadline before shared option validation.
///
/// `timeout_ms` is intentionally transport-owned: removing it here keeps the shared binding
/// options schema stable while allowing every one-shot operation to receive the same
/// `OperationControl` deadline. Invalid JSON and invalid UTF-8 remain untouched so the binding
/// layer preserves its established error classification and precedence.
fn wasm_options(options_json: &[u8]) -> Result<(Vec<u8>, Option<Duration>), BindingError> {
    merman_bindings_core::enforce_options_json_byte_budget(options_json)?;
    if options_json.is_empty() {
        return Ok((Vec::new(), None));
    }

    let Ok(text) = std::str::from_utf8(options_json) else {
        return Ok((options_json.to_vec(), None));
    };
    let Ok(mut fields) = serde_json::from_str::<WasmOptionsFields<'_>>(text) else {
        return Ok((options_json.to_vec(), None));
    };
    let mut timeout = None;
    for (key, value) in &fields.0 {
        if key != "timeout_ms" {
            continue;
        }
        let timeout_ms =
            serde_json::from_str::<u64>(value.get()).map_err(|_| invalid_wasm_timeout())?;
        if timeout.is_some() || timeout_ms > WASM_TIMEOUT_MS_MAX {
            return Err(invalid_wasm_timeout());
        }
        timeout = Some(Duration::from_millis(timeout_ms));
    }
    if timeout.is_none() {
        return Ok((options_json.to_vec(), None));
    }
    fields.0.retain(|(key, _)| key != "timeout_ms");
    let normalized = serde_json::to_vec(&fields).map_err(|error| {
        BindingError::internal(format!(
            "failed to normalize WASM transport options: {error}"
        ))
    })?;
    Ok((normalized, timeout))
}

// Preserve member order, duplicate keys, and nested raw values for shared admission.
// A Value or Map round trip would erase duplicate theme fields before they can be rejected.
struct WasmOptionsFields<'a>(Vec<(String, &'a serde_json::value::RawValue)>);

impl<'de> Deserialize<'de> for WasmOptionsFields<'de> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FieldsVisitor;

        impl<'de> serde::de::Visitor<'de> for FieldsVisitor {
            type Value = WasmOptionsFields<'de>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an options object")
            }

            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<Self::Value, M::Error> {
                let mut fields = Vec::new();
                while let Some(field) = map.next_entry()? {
                    fields.push(field);
                }
                Ok(WasmOptionsFields(fields))
            }
        }

        deserializer.deserialize_map(FieldsVisitor)
    }
}

impl Serialize for WasmOptionsFields<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

fn invalid_wasm_timeout() -> BindingError {
    BindingError::invalid_options_json(format!(
        "WASM timeout_ms must be an integer from 0 through {WASM_TIMEOUT_MS_MAX} milliseconds"
    ))
}

fn wasm_operation_control(timeout: Option<Duration>) -> OperationControl {
    timeout.map_or_else(OperationControl::new, |timeout| {
        OperationControl::new().with_deadline(timeout)
    })
}

fn execute_wasm_operation(
    operation_id: &'static str,
    source: &[u8],
    options_json: &[u8],
    uri: Option<&[u8]>,
) -> Result<Vec<u8>, BindingError> {
    let (normalized_options, timeout) = wasm_options(options_json)?;
    wasm_artifact_contract().execute_once_data(
        BindingOperationRequest::new(operation_id, source)
            .with_optional_uri(uri)
            .with_options_json(&normalized_options)
            .with_control(wasm_operation_control(timeout)),
    )
}

fn execute_wasm_operation_result(
    operation_id: &'static str,
    source: &[u8],
    options_json: &[u8],
    uri: Option<&[u8]>,
) -> Result<BindingOperationResult, BindingError> {
    let (normalized_options, timeout) = wasm_options(options_json)?;
    wasm_artifact_contract().execute_once(
        BindingOperationRequest::new(operation_id, source)
            .with_optional_uri(uri)
            .with_options_json(&normalized_options)
            .with_control(wasm_operation_control(timeout)),
    )
}

#[cfg(feature = "svg")]
fn execute_wasm_operation_with_services(
    operation_id: &'static str,
    source: &[u8],
    options_json: &[u8],
    services: merman_bindings_core::BindingEngineServices,
) -> Result<Vec<u8>, BindingError> {
    let (normalized_options, timeout) = wasm_options(options_json)?;
    wasm_artifact_contract().execute_once_data_with_services(
        BindingOperationRequest::new(operation_id, source)
            .with_options_json(&normalized_options)
            .with_control(wasm_operation_control(timeout)),
        services,
    )
}

#[cfg(feature = "svg")]
fn execute_wasm_operation_result_with_services(
    operation_id: &'static str,
    source: &[u8],
    options_json: &[u8],
    services: merman_bindings_core::BindingEngineServices,
) -> Result<BindingOperationResult, BindingError> {
    let (normalized_options, timeout) = wasm_options(options_json)?;
    wasm_artifact_contract().execute_once_with_services(
        BindingOperationRequest::new(operation_id, source)
            .with_options_json(&normalized_options)
            .with_control(wasm_operation_control(timeout)),
        services,
    )
}

fn text_operation_result(
    result: Result<BindingOperationResult, BindingError>,
    text_field: &str,
) -> Result<JsValue, JsValue> {
    let output = result.map_err(binding_error_to_js)?;
    let (_, _, data, metadata) = output.into_parts();
    let text = String::from_utf8(data).map_err(|error| JsValue::from_str(&error.to_string()))?;
    let metadata: serde_json::Value = serde_json::from_slice(metadata.json_bytes())
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    serde_json::json!({ (text_field): text, "metadata": metadata })
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|error| JsValue::from_str(&error.to_string()))
}

fn string_result(result: Result<Vec<u8>, BindingError>) -> Result<String, JsValue> {
    let bytes = result.map_err(binding_error_to_js)?;
    String::from_utf8(bytes).map_err(|err| JsValue::from_str(&err.to_string()))
}

fn json_value_result(result: Result<Vec<u8>, BindingError>) -> Result<JsValue, JsValue> {
    let bytes = result.map_err(binding_error_to_js)?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|err| JsValue::from_str(&err.to_string()))?;
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

pub(crate) fn binding_error_to_js(err: BindingError) -> JsValue {
    let fallback = format!("{}: {}", err.status().code_name(), err.message());
    binding_error_payload_value(&err)
        .and_then(|payload| {
            payload
                .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
                .map_err(|err| err.to_string())
        })
        .unwrap_or_else(|_| JsValue::from_str(&fallback))
}

fn binding_error_payload_value(err: &BindingError) -> Result<serde_json::Value, String> {
    serde_json::from_slice(&merman_bindings_core::binding_error_js_payload_json_bytes(
        err,
    ))
    .map_err(|err| err.to_string())
}

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
thread_local! {
    static HOST_TEXT_MEASURE_CALLBACK: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
}

#[cfg(all(feature = "svg", any(target_arch = "wasm32", test)))]
#[derive(Debug, Serialize)]
struct WasmHostTextMeasureRequest<'a> {
    operation: &'static str,
    phase: &'static str,
    text: &'a str,
    font_family: Option<&'a str>,
    font_size: f64,
    font_weight: Option<&'a str>,
    font_style: &'a str,
    max_width: Option<f64>,
    has_max_width: bool,
    line_height: f64,
    letter_spacing: f64,
    word_spacing: f64,
    wrap_mode: &'static str,
    direction: &'static str,
    white_space: &'static str,
}

#[cfg(all(feature = "svg", any(target_arch = "wasm32", test)))]
fn wasm_host_text_measure_request(
    request: merman_bindings_core::HostTextMeasurementRequest<'_>,
) -> Result<WasmHostTextMeasureRequest<'_>, merman_bindings_core::HostTextMeasurementError> {
    use merman_bindings_core::HostTextWhiteSpaceCode;

    let transport = merman_bindings_core::host_text_measurement_transport_fields(request);
    let white_space = match transport.white_space {
        code if code == HostTextWhiteSpaceCode::Normal.external_code() => "normal",
        code if code == HostTextWhiteSpaceCode::Nowrap.external_code() => "nowrap",
        code if code == HostTextWhiteSpaceCode::BreakSpaces.external_code() => "break-spaces",
        _ => {
            return Err(merman_bindings_core::HostTextMeasurementError::new(
                "unknown shared text white-space code",
            ));
        }
    };
    Ok(WasmHostTextMeasureRequest {
        operation: request.operation.external_name(),
        phase: wasm_measurement_phase(request.phase),
        text: request.text,
        font_family: request.style.font_family.as_deref(),
        font_size: request.style.font_size,
        font_weight: request.style.font_weight.as_deref(),
        font_style: request.style.font_style.as_deref().unwrap_or("normal"),
        max_width: request.max_width,
        has_max_width: request.max_width.is_some(),
        line_height: transport.line_height,
        letter_spacing: 0.0,
        word_spacing: 0.0,
        wrap_mode: wasm_wrap_mode(request.wrap_mode),
        direction: "auto",
        white_space,
    })
}

#[cfg(all(feature = "svg", any(target_arch = "wasm32", test)))]
#[derive(Debug, Deserialize)]
struct WasmHostTextMeasureResult {
    kind: Option<String>,
    width: Option<f64>,
    height: Option<f64>,
    length: Option<f64>,
    line_count: Option<i64>,
    bbox_left: Option<f64>,
    bbox_right: Option<f64>,
    raw_width: Option<f64>,
}

#[cfg(all(feature = "svg", any(target_arch = "wasm32", test)))]
fn decode_wasm_host_text_measurement(
    request: merman_bindings_core::HostTextMeasurementRequest<'_>,
    result: WasmHostTextMeasureResult,
) -> Result<merman_bindings_core::HostTextMeasurement, merman_bindings_core::HostTextMeasurementError>
{
    merman_bindings_core::decode_host_text_measurement(
        request,
        merman_bindings_core::HostTextMeasurementRecord {
            result_kind: result
                .kind
                .as_deref()
                .and_then(merman_bindings_core::HostTextMeasurementResultKind::from_external_name),
            width: result.width,
            height: result.height,
            line_count: result.line_count.map(i128::from),
            length: result.length,
            bbox_left: result.bbox_left,
            bbox_right: result.bbox_right,
            raw_width: result.raw_width,
        },
    )
}

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
struct WasmHostTextMeasurer;

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
fn wasm_host_text_measurement_handled(
    value: &JsValue,
) -> Result<Option<bool>, merman_bindings_core::HostTextMeasurementError> {
    let handled = js_sys::Reflect::get(value, &JsValue::from_str("handled")).map_err(|err| {
        merman_bindings_core::HostTextMeasurementError::invalid_value(js_error_message(&err))
    })?;
    serde_wasm_bindgen::from_value(handled).map_err(|err| {
        merman_bindings_core::HostTextMeasurementError::invalid_value(err.to_string())
    })
}

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
impl WasmHostTextMeasurer {
    fn call_host(
        &self,
        request: merman_bindings_core::HostTextMeasurementRequest<'_>,
    ) -> merman_bindings_core::HostMeasurementResult {
        let external_request = wasm_host_text_measure_request(request)?;
        let external_request = serde_wasm_bindgen::to_value(&external_request)
            .map_err(|err| merman_bindings_core::HostTextMeasurementError::new(err.to_string()))?;

        HOST_TEXT_MEASURE_CALLBACK.with(|slot| {
            let Some(callback) = slot.borrow().clone() else {
                return Ok(None);
            };
            let value = callback
                .call1(&JsValue::NULL, &external_request)
                .map_err(|err| {
                    merman_bindings_core::HostTextMeasurementError::new(js_error_message(&err))
                })?;
            if value.is_null() || value.is_undefined() {
                return Ok(None);
            }

            if wasm_host_text_measurement_handled(&value)? == Some(false) {
                return Ok(None);
            }

            let result: WasmHostTextMeasureResult =
                serde_wasm_bindgen::from_value(value).map_err(|err| {
                    merman_bindings_core::HostTextMeasurementError::invalid_value(err.to_string())
                })?;
            decode_wasm_host_text_measurement(request, result).map(Some)
        })
    }
}

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
impl merman_bindings_core::HostTextMeasurer for WasmHostTextMeasurer {
    fn measure(
        &self,
        request: merman_bindings_core::HostTextMeasurementRequest<'_>,
    ) -> merman_bindings_core::HostMeasurementResult {
        self.call_host(request)
    }
}

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
struct HostTextMeasureCallbackGuard {
    previous_callback: Option<js_sys::Function>,
}

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
impl Drop for HostTextMeasureCallbackGuard {
    fn drop(&mut self) {
        HOST_TEXT_MEASURE_CALLBACK.with(|slot| {
            slot.replace(self.previous_callback.take());
        });
    }
}

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
fn with_host_text_measure_callback<R>(callback: js_sys::Function, f: impl FnOnce() -> R) -> R {
    let previous_callback = HOST_TEXT_MEASURE_CALLBACK.with(|slot| slot.replace(Some(callback)));
    let _guard = HostTextMeasureCallbackGuard { previous_callback };
    f()
}

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
fn js_error_message(err: &JsValue) -> String {
    err.as_string()
        .unwrap_or_else(|| "host text measurer callback failed".to_string())
}

#[cfg(all(feature = "svg", any(target_arch = "wasm32", test)))]
fn wasm_measurement_phase(phase: merman_bindings_core::TextMeasurementPhase) -> &'static str {
    match phase {
        merman_bindings_core::TextMeasurementPhase::Layout => "layout",
        merman_bindings_core::TextMeasurementPhase::Wrap => "wrap",
        merman_bindings_core::TextMeasurementPhase::SvgBBox => "svg-bbox",
        merman_bindings_core::TextMeasurementPhase::ComputedLength => "computed-length",
    }
}

#[cfg(all(feature = "svg", any(target_arch = "wasm32", test)))]
fn wasm_wrap_mode(wrap_mode: WrapMode) -> &'static str {
    match wrap_mode {
        WrapMode::SvgLike => "svg-like",
        WrapMode::SvgLikeSingleRun => "svg-like-single-run",
        WrapMode::HtmlLike => "html-like",
    }
}

#[cfg(test)]
mod tests;

#![forbid(unsafe_code)]

//! WebAssembly bindings for browser integrations.
//!
//! The crate intentionally stays thin: binding option parsing, rendering, and error classification
//! are delegated to `merman-bindings-core`. It only extracts browser-transport fields such as the
//! optional operation deadline before dispatch.

use merman_bindings_core::{
    ArtifactContractSpec, BindingError, BindingOperationRequest, BindingTransportKey,
    CapabilityKey, ConstructorServiceKey, OperationControl, OperationKey, RuntimeCatalog,
    RuntimePolicyExposure, TargetKey, TransportCompiledExtensionKey, ValidatedArtifactContract,
};
use serde::Serialize;
use std::time::Duration;
use wasm_bindgen::prelude::*;

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
use std::{cell::RefCell, sync::Arc};

#[cfg(feature = "editor")]
mod editor_language;

#[cfg(feature = "editor")]
pub use editor_language::{
    WasmEditorSession, editor_code_actions, editor_completion_trigger_characters,
    editor_completions, editor_definition, editor_diagnostics, editor_diagram_detection,
    editor_document_symbols, editor_hover, editor_prepare_rename, editor_references, editor_rename,
    editor_search_document_symbols,
};

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
use merman_bindings_core::{TextStyle, WrapMode};
#[cfg(all(feature = "svg", any(target_arch = "wasm32", test)))]
use serde::Deserialize;

/// Breaking API version for the wasm-bindgen transport.
///
/// This is independent from the native C ABI and the Typst plugin ABI. It changes when the
/// JavaScript/WASM export or runtime-contract wire shape becomes incompatible.
pub const WASM_TRANSPORT_API_VERSION: u32 = 5;
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
    OperationKey::LayoutJson,
    OperationKey::SemanticJson,
    #[cfg(feature = "svg")]
    OperationKey::Svg,
    #[cfg(feature = "svg")]
    OperationKey::SvgPlanJson,
    #[cfg(feature = "analysis")]
    OperationKey::ValidationJson,
];
const WASM_SUPPLEMENTAL_CAPABILITIES: &[CapabilityKey] = &[
    #[cfg(feature = "layout-cytoscape")]
    CapabilityKey::LayoutCytoscape,
    #[cfg(feature = "layout-elk")]
    CapabilityKey::LayoutElk,
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

#[wasm_bindgen(js_name = renderAscii)]
pub fn render_ascii(source: &str, options_json: Option<String>) -> Result<String, JsValue> {
    string_result(execute_wasm_operation(
        "ascii",
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
        None,
    ))
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

#[wasm_bindgen(js_name = presentationCatalog)]
pub fn presentation_catalog() -> Result<JsValue, JsValue> {
    json_value_result(wasm_artifact_contract().metadata_json("presentation-catalog"))
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
    if options_json.is_empty() {
        return Ok((Vec::new(), None));
    }

    let Ok(text) = std::str::from_utf8(options_json) else {
        return Ok((options_json.to_vec(), None));
    };
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(text) else {
        return Ok((options_json.to_vec(), None));
    };
    let Some(object) = value.as_object_mut() else {
        return Ok((options_json.to_vec(), None));
    };
    let Some(timeout_ms) = object.remove("timeout_ms") else {
        return Ok((options_json.to_vec(), None));
    };
    let Some(timeout_ms) = timeout_ms.as_u64() else {
        return Err(invalid_wasm_timeout());
    };
    if timeout_ms > WASM_TIMEOUT_MS_MAX {
        return Err(invalid_wasm_timeout());
    }
    let normalized = serde_json::to_vec(&value).map_err(|error| {
        BindingError::internal(format!(
            "failed to normalize WASM transport options: {error}"
        ))
    })?;
    Ok((normalized, Some(Duration::from_millis(timeout_ms))))
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
    wasm_artifact_contract()
        .execute_once(
            BindingOperationRequest::new(operation_id, source)
                .with_optional_uri(uri)
                .with_options_json(&normalized_options)
                .with_control(wasm_operation_control(timeout)),
        )
        .map(merman_bindings_core::BindingOperationResult::into_data)
}

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
fn execute_wasm_operation_with_services(
    operation_id: &'static str,
    source: &[u8],
    options_json: &[u8],
    services: merman_bindings_core::BindingEngineServices,
) -> Result<Vec<u8>, BindingError> {
    let (normalized_options, timeout) = wasm_options(options_json)?;
    let control = wasm_operation_control(timeout);
    control
        .checkpoint_at(merman_bindings_core::OperationPhase::Admission)
        .map_err(BindingError::cancelled)?;
    let engine =
        wasm_artifact_contract().create_engine_with_services(&normalized_options, services)?;
    control
        .checkpoint_at(merman_bindings_core::OperationPhase::Admission)
        .map_err(BindingError::cancelled)?;
    engine
        .execute(BindingOperationRequest::new(operation_id, source).with_control(control))
        .map(merman_bindings_core::BindingOperationResult::into_data)
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

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
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
        let external_request = WasmHostTextMeasureRequest {
            operation: request.operation.external_name(),
            phase: wasm_measurement_phase(request.phase),
            text: request.text,
            font_family: request.style.font_family.as_deref(),
            font_size: request.style.font_size,
            font_weight: request.style.font_weight.as_deref(),
            font_style: request.style.font_style.as_deref().unwrap_or("normal"),
            max_width: request.max_width,
            has_max_width: request.max_width.is_some(),
            line_height: wasm_line_height(request.style, request.wrap_mode),
            letter_spacing: 0.0,
            word_spacing: 0.0,
            wrap_mode: wasm_wrap_mode(request.wrap_mode),
            direction: "auto",
            white_space: wasm_white_space(request.max_width, request.wrap_mode),
        };
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

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
fn wasm_measurement_phase(phase: merman_bindings_core::TextMeasurementPhase) -> &'static str {
    match phase {
        merman_bindings_core::TextMeasurementPhase::Layout => "layout",
        merman_bindings_core::TextMeasurementPhase::Wrap => "wrap",
        merman_bindings_core::TextMeasurementPhase::SvgBBox => "svg-bbox",
        merman_bindings_core::TextMeasurementPhase::ComputedLength => "computed-length",
    }
}

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
fn wasm_wrap_mode(wrap_mode: WrapMode) -> &'static str {
    match wrap_mode {
        WrapMode::SvgLike => "svg-like",
        WrapMode::SvgLikeSingleRun => "svg-like-single-run",
        WrapMode::HtmlLike => "html-like",
    }
}

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
fn wasm_line_height(style: &TextStyle, wrap_mode: WrapMode) -> f64 {
    let factor = match wrap_mode {
        WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => 1.1,
        WrapMode::HtmlLike => 1.5,
    };
    style.font_size.max(1.0) * factor
}

#[cfg(all(feature = "svg", target_arch = "wasm32"))]
fn wasm_white_space(max_width: Option<f64>, wrap_mode: WrapMode) -> &'static str {
    match wrap_mode {
        WrapMode::HtmlLike if max_width.is_some() => "break-spaces",
        WrapMode::HtmlLike => "nowrap",
        WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => "normal",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "ascii")]
    use merman_ascii_test_contracts::ascii_resource_boundary_contract;
    #[cfg(any(feature = "analysis", feature = "ascii"))]
    use serde_json::Value;

    #[cfg(feature = "ascii")]
    fn wasm_ascii_options(limit_id: &str, value: u64) -> String {
        format!(r#"{{"resources":{{"limits":{{"{limit_id}":{value}}}}}}}"#)
    }

    #[cfg(feature = "ascii")]
    fn assert_wasm_ascii_resource_error(
        error: BindingError,
        expected_limit_id: &str,
        expected_phase: &str,
        expected_max: u64,
    ) -> u64 {
        assert_eq!(
            error.status(),
            merman_bindings_core::BindingStatus::ResourceLimitExceeded
        );
        assert_eq!(error.capability_id(), None);
        let details = error
            .resource_details()
            .expect("WASM ASCII resource errors must expose typed details");
        assert_eq!(details.limit_id, expected_limit_id);
        assert_eq!(details.phase, expected_phase);
        assert_eq!(details.max, expected_max);
        assert_eq!(details.profile, "interactive");
        assert!(details.actual > details.max);

        let payload = binding_error_payload_value(&error).expect("structured WASM error payload");
        let resource = payload["details"]["resource"]
            .as_object()
            .expect("WASM resource details object");
        assert_eq!(resource.len(), 6, "WASM resource error shape changed");
        assert_eq!(resource["cause"], "ceiling");
        assert_eq!(resource["limit_id"], expected_limit_id);
        assert_eq!(resource["phase"], expected_phase);
        assert_eq!(resource["actual"], details.actual);
        assert_eq!(resource["max"], expected_max);
        assert_eq!(resource["profile"], "interactive");
        details.actual
    }

    #[test]
    fn package_version_matches_crate_version() {
        assert_eq!(package_version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn transport_api_version_is_independent_from_host_measurement_protocol() {
        assert_eq!(transport_api_version(), WASM_TRANSPORT_API_VERSION);
        assert_eq!(WASM_TRANSPORT_API_VERSION, 5);
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn ascii_metadata_exposes_the_total_capability_contract() {
        let bytes = wasm_artifact_contract()
            .metadata_json("ascii-capabilities")
            .unwrap();
        let capabilities: Value = serde_json::from_slice(&bytes).unwrap();
        let capabilities = capabilities.as_array().unwrap();

        assert_eq!(capabilities.len(), 31);
        let flowchart = capabilities
            .iter()
            .find(|capability| capability["diagram_type"] == "flowchart")
            .unwrap();
        assert_eq!(flowchart["semantic_coverage"], "partial");
        assert_eq!(flowchart["primary_projection"], "diagrammatic");
        assert_eq!(flowchart["support_level"], "partial");
        assert!(flowchart.get("summary_fallback").is_none());

        let zenuml = capabilities
            .iter()
            .find(|capability| capability["diagram_type"] == "zenuml")
            .unwrap();
        assert!(zenuml["semantic_coverage"].is_null());
        assert_eq!(zenuml["primary_projection"], "none");
        assert_eq!(zenuml["support_level"], "unsupported");
    }

    #[test]
    fn wasm_timeout_option_becomes_a_relative_deadline_and_is_removed_from_shared_options() {
        let (options, timeout) = wasm_options(
            br#"{"version":2,"timeout_ms":125,"resources":{"profile":"constrained"}}"#,
        )
        .unwrap();

        assert_eq!(timeout, Some(Duration::from_millis(125)));
        let value: serde_json::Value = serde_json::from_slice(&options).unwrap();
        assert_eq!(value["version"], 2);
        assert_eq!(value["resources"]["profile"], "constrained");
        assert!(value.get("timeout_ms").is_none());
    }

    #[test]
    fn wasm_timeout_option_rejects_non_integer_values() {
        for options in [
            br#"{"timeout_ms":-1}"#.as_slice(),
            br#"{"timeout_ms":1.5}"#.as_slice(),
            br#"{"timeout_ms":"10"}"#.as_slice(),
            br#"{"timeout_ms":true}"#.as_slice(),
            br#"{"timeout_ms":4294967296}"#.as_slice(),
        ] {
            let error = wasm_options(options).expect_err("invalid timeout_ms must be rejected");
            assert_eq!(
                error.status(),
                merman_bindings_core::BindingStatus::OptionsJsonError
            );
            assert!(error.message().contains("timeout_ms"));
        }
    }

    #[test]
    fn wasm_zero_timeout_returns_structured_deadline_cancellation_at_admission() {
        let error = execute_wasm_operation(
            "semantic-json",
            b"flowchart TD\nA --> B",
            br#"{"timeout_ms":0}"#,
            None,
        )
        .expect_err("a zero deadline must cancel before operation work starts");

        assert_eq!(
            error.status(),
            merman_bindings_core::BindingStatus::Cancelled
        );
        let details = error
            .cancellation_details()
            .expect("cancelled errors carry structured details");
        assert_eq!(details.reason, "deadline_exceeded");
        assert_eq!(details.phase, "admission");

        let payload = binding_error_payload_value(&error).unwrap();
        assert_eq!(payload["code_name"], "MERMAN_CANCELLED");
        assert_eq!(
            payload["details"]["cancellation"]["reason"],
            "deadline_exceeded"
        );
        assert_eq!(payload["details"]["cancellation"]["phase"], "admission");
    }

    #[cfg(feature = "svg")]
    #[test]
    fn host_measurement_operations_match_the_exact_wasm_protocol() {
        let operations = merman_bindings_core::TextMeasurementOperation::ALL
            .map(|operation| (operation.external_code(), operation.external_name()));

        assert_eq!(
            operations,
            [
                (0, "measure"),
                (1, "computed-length"),
                (2, "bbox-x"),
                (3, "bbox-x-with-ascii-overhang"),
                (4, "title-bbox-x"),
                (5, "simple-bbox-width"),
                (6, "raw-bbox-width"),
                (7, "tspan-bbox-width"),
                (8, "tspan-bbox-height"),
                (9, "wrap-probe-bbox-width"),
                (10, "simple-bbox-height"),
                (11, "wrapped"),
                (12, "wrapped-with-raw-width"),
                (13, "bounding-client-rect-width"),
                (14, "create-text-bbox-y-offset"),
                (15, "mermaid-calculate-text-dimensions"),
                (16, "canvas-measure-text-width"),
                (17, "create-text-middle-bbox-y-offset"),
                (18, "raw-bbox-height"),
            ]
        );
    }

    #[cfg(feature = "svg")]
    #[test]
    fn wasm_checked_decoder_rejects_missing_fields_and_oversized_counts() {
        let style = merman_bindings_core::TextStyle::default();
        let request = merman_bindings_core::HostTextMeasurementRequest {
            operation: merman_bindings_core::TextMeasurementOperation::Measure,
            phase: merman_bindings_core::TextMeasurementPhase::Layout,
            text: "x",
            style: &style,
            max_width: None,
            wrap_mode: merman_bindings_core::WrapMode::SvgLike,
        };
        let result = |height, line_count| WasmHostTextMeasureResult {
            kind: Some("metrics".to_string()),
            width: Some(1.0),
            height,
            length: None,
            line_count,
            bbox_left: None,
            bbox_right: None,
            raw_width: None,
        };

        assert!(decode_wasm_host_text_measurement(request, result(None, Some(1))).is_err());
        assert!(decode_wasm_host_text_measurement(request, result(Some(1.0), Some(3))).is_err());
    }

    #[cfg(feature = "svg")]
    #[test]
    fn svg_plan_reports_the_owner_capability_payload() {
        let result = execute_wasm_operation(
            "svg-plan-json",
            b"flowchart TD\nA[Hello] --> B[World]",
            b"",
            None,
        )
        .unwrap();
        let plan: serde_json::Value = serde_json::from_slice(&result).unwrap();

        assert_eq!(plan["planned_operation_id"], "svg");
        assert_eq!(plan["missing_capability_ids"], serde_json::json!([]));
        assert_eq!(plan["ready"], true);
    }

    #[cfg(all(
        feature = "svg",
        not(feature = "layout-cytoscape"),
        not(feature = "layout-elk"),
        not(feature = "math")
    ))]
    #[test]
    fn ambient_render_dependencies_do_not_widen_the_wasm_owner_contract() {
        let capabilities = wasm_artifact_contract().runtime_capabilities();
        assert!(capabilities.has_capability("svg"));

        for (capability_id, source) in [
            (
                "layout-cytoscape",
                "architecture-beta\n  service api(server)[API]",
            ),
            (
                "layout-elk",
                "---\nconfig:\n  layout: elk\n---\nflowchart TD\nA --> B",
            ),
            ("math", "flowchart TD\nA[\"$$x^2$$\"] --> B"),
        ] {
            assert!(!capabilities.has_capability(capability_id));

            let plan =
                execute_wasm_operation("svg-plan-json", source.as_bytes(), b"", None).unwrap();
            let plan: serde_json::Value = serde_json::from_slice(&plan).unwrap();
            assert_eq!(
                plan["required_capability_ids"],
                serde_json::json!([capability_id])
            );
            assert_eq!(
                plan["missing_capability_ids"],
                serde_json::json!([capability_id])
            );
            assert_eq!(plan["ready"], false);

            let error = execute_wasm_operation("svg", source.as_bytes(), b"", None)
                .expect_err("the WASM owner contract must reject ambient render capabilities");
            assert_eq!(
                error.status(),
                merman_bindings_core::BindingStatus::UnsupportedOperation
            );
            assert_eq!(error.capability_id(), Some(capability_id));
        }

        let error = execute_wasm_operation(
            "svg",
            b"flowchart TD\nA --> B",
            br#"{"environment":{"math_renderer":"ratex"}}"#,
            None,
        )
        .expect_err("explicit ratex selection requires owner-selected math");
        assert_eq!(
            error.status(),
            merman_bindings_core::BindingStatus::UnsupportedOperation
        );
        assert_eq!(error.capability_id(), Some("math"));
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn validation_error_uses_binding_status() {
        let json: Value =
            serde_json::from_slice(&merman_bindings_core::validate_json(b"", b"").unwrap())
                .unwrap();

        assert_eq!(json["valid"], false);
        assert_eq!(json["code_name"], "MERMAN_NO_DIAGRAM");
        assert!(
            json["error"]
                .as_str()
                .unwrap()
                .contains("no Mermaid diagram")
        );
    }

    #[cfg(all(target_arch = "wasm32", feature = "analysis"))]
    #[test]
    fn analyze_json_exposes_diagnostics_payload() {
        let value: Value = serde_wasm_bindgen::from_value(analyze_json("", None).unwrap()).unwrap();
        assert_no_diagram_analysis_payload(&value);
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "analysis"))]
    #[test]
    fn analyze_json_exposes_diagnostics_payload() {
        let value: Value =
            serde_json::from_slice(&merman_bindings_core::analyze_json(b"", b"").unwrap()).unwrap();
        assert_no_diagram_analysis_payload(&value);
    }

    #[cfg(feature = "analysis")]
    fn assert_no_diagram_analysis_payload(value: &Value) {
        assert_eq!(value["version"], 1);
        assert_eq!(value["valid"], false);
        assert_eq!(value["diagnostics"][0]["code_name"], "MERMAN_NO_DIAGRAM");
    }

    #[cfg(all(target_arch = "wasm32", feature = "analysis"))]
    #[test]
    fn analysis_facts_exposes_parser_backed_syntax_payload() {
        let value: Value =
            serde_wasm_bindgen::from_value(analysis_facts("flowchart TD\nA-->B\n", None).unwrap())
                .unwrap();
        assert_parser_backed_analysis_facts_payload(&value);
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "analysis"))]
    #[test]
    fn analysis_facts_exposes_parser_backed_syntax_payload() {
        let value: Value = serde_json::from_slice(
            &merman_bindings_core::analysis_facts_json(b"flowchart TD\nA-->B\n", b"").unwrap(),
        )
        .unwrap();
        assert_parser_backed_analysis_facts_payload(&value);
    }

    #[cfg(feature = "analysis")]
    fn assert_parser_backed_analysis_facts_payload(value: &Value) {
        assert_eq!(
            value["version"],
            merman_bindings_core::ANALYSIS_FACTS_PAYLOAD_VERSION
        );
        assert_eq!(value["valid"], true);
        assert_eq!(
            value["diagrams"][0]["syntax"]["fact_source"],
            "parser_complete"
        );
        assert_eq!(value["diagrams"][0]["syntax"]["source_mapped_spans"], true);
        assert_eq!(value["diagrams"][0]["syntax"]["effective_layout"], "dagre");
        assert!(
            value["diagrams"][0]["syntax"]["semantic_items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| {
                    item["name"] == "A"
                        && item["rename_policy"] == "flowchart_node_id"
                        && item["span"]["document"].is_object()
                })
        );
    }

    #[cfg(all(target_arch = "wasm32", feature = "analysis"))]
    #[test]
    fn analysis_facts_serializes_unavailable_body_semantics() {
        let value: Value = serde_wasm_bindgen::from_value(
            analysis_facts("unknownDiagram\nPretendNode --> OtherNode\n", None).unwrap(),
        )
        .unwrap();
        assert_unavailable_analysis_facts_payload(&value);
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "analysis"))]
    #[test]
    fn analysis_facts_serializes_unavailable_body_semantics() {
        let value: Value = serde_json::from_slice(
            &merman_bindings_core::analysis_facts_json(
                b"unknownDiagram\nPretendNode --> OtherNode\n",
                b"",
            )
            .unwrap(),
        )
        .unwrap();
        assert_unavailable_analysis_facts_payload(&value);
    }

    #[cfg(feature = "analysis")]
    fn assert_unavailable_analysis_facts_payload(value: &Value) {
        assert_eq!(
            value["version"],
            merman_bindings_core::ANALYSIS_FACTS_PAYLOAD_VERSION
        );
        let syntax = &value["diagrams"][0]["syntax"];
        assert_eq!(syntax["fact_source"], "unavailable");
        assert_eq!(syntax["parser_backed"], false);
        assert_eq!(syntax["source_mapped_spans"], false);
        assert_eq!(syntax["node_ids"], serde_json::json!([]));
        assert_eq!(syntax["references"], serde_json::json!([]));
        assert_eq!(syntax["outline_items"], serde_json::json!([]));
        assert_eq!(syntax["semantic_items"], serde_json::json!([]));
    }

    #[cfg(all(target_arch = "wasm32", feature = "analysis"))]
    #[test]
    fn analyze_document_exposes_markdown_diagnostics_payload() {
        let value: Value = serde_wasm_bindgen::from_value(
            analyze_document(
                "before\n```mermaid\nflowchart TD\nA-->\n```\nafter\n",
                "file:///tmp/example.md".to_string(),
                None,
            )
            .unwrap(),
        )
        .unwrap();
        assert_markdown_document_analysis_payload(&value);
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "analysis"))]
    #[test]
    fn analyze_document_exposes_markdown_diagnostics_payload() {
        let value: Value = serde_json::from_slice(
            &merman_bindings_core::analyze_document_json(
                b"before\n```mermaid\nflowchart TD\nA-->\n```\nafter\n",
                b"file:///tmp/example.md",
                b"",
            )
            .unwrap(),
        )
        .unwrap();
        assert_markdown_document_analysis_payload(&value);
    }

    #[cfg(feature = "analysis")]
    fn assert_markdown_document_analysis_payload(value: &Value) {
        assert_eq!(value["valid"], false);
        assert_eq!(value["source"]["kind"], "markdown");
        assert_eq!(value["diagnostics"][0]["span"]["line"], 4);
        assert!(
            value["diagnostics"][0]["related"]
                .as_array()
                .unwrap()
                .iter()
                .any(|related| related["message"] == "Mermaid fence 1")
        );
    }

    #[cfg(all(target_arch = "wasm32", feature = "analysis"))]
    #[test]
    fn analyze_document_facts_exposes_markdown_syntax_payload() {
        let value: Value = serde_wasm_bindgen::from_value(
            analyze_document_facts(
                "before\n```mermaid\nflowchart TD\nA@{\n  shape: rou\n}\n```\nafter\n",
                "file:///tmp/example.md".to_string(),
                None,
            )
            .unwrap(),
        )
        .unwrap();
        assert_markdown_document_analysis_facts_payload(&value);
    }

    #[cfg(all(not(target_arch = "wasm32"), feature = "analysis"))]
    #[test]
    fn analyze_document_facts_exposes_markdown_syntax_payload() {
        let value: Value = serde_json::from_slice(
            &merman_bindings_core::analyze_document_facts_json(
                b"before\n```mermaid\nflowchart TD\nA@{\n  shape: rou\n}\n```\nafter\n",
                b"file:///tmp/example.md",
                b"",
            )
            .unwrap(),
        )
        .unwrap();
        assert_markdown_document_analysis_facts_payload(&value);
    }

    #[cfg(feature = "analysis")]
    fn assert_markdown_document_analysis_facts_payload(value: &Value) {
        assert_eq!(
            value["version"],
            merman_bindings_core::ANALYSIS_FACTS_PAYLOAD_VERSION
        );
        assert_eq!(value["valid"], false);
        assert_eq!(value["source"]["kind"], "markdown");
        assert_eq!(value["diagrams"][0]["source_id"], "mermaid-fence-1");
        assert_eq!(value["diagrams"][0]["syntax"]["parser_backed"], true);
        assert!(
            value["diagrams"][0]["syntax"]["expected_syntax"]
                .as_array()
                .unwrap()
                .iter()
                .any(|expected| {
                    expected["kind"] == "shape" && expected["span"]["document"].is_object()
                })
        );
    }

    #[test]
    fn wasm_error_payload_is_structured() {
        let err = merman_bindings_core::render_svg(b"flowchart TD\nA", b"{").unwrap_err();
        let json = binding_error_payload_value(&err).unwrap();

        assert_eq!(json["version"], 1);
        assert_eq!(json["ok"], false);
        assert_eq!(json["code_name"], "MERMAN_OPTIONS_JSON_ERROR");
        assert_eq!(json["kind"], "generic");
        assert!(json["capability_id"].is_null());
        assert!(json["message"].as_str().unwrap().contains("options_json"));

        let err = BindingError::resource_limit(
            "embedded_image_decode",
            "max_embedded_image_bytes",
            5,
            4,
            "constrained",
            "embedded image is too large",
        );
        let json = binding_error_payload_value(&err).unwrap();
        assert_eq!(
            json["details"]["resource"]["limit_id"],
            "max_embedded_image_bytes"
        );
        assert_eq!(json["details"]["resource"]["actual"], 5);
        assert_eq!(json["details"]["resource"]["cause"], "ceiling");

        let err = BindingError::resource_limit_with_cause(
            merman_bindings_core::BindingResourceLimitCause::ArithmeticOverflow,
            "layout_model",
            "max_layout_work_units",
            u64::MAX,
            800_000,
            "interactive",
            "layout work accounting overflowed",
        );
        let json = binding_error_payload_value(&err).unwrap();
        assert_eq!(json["code_name"], "MERMAN_RESOURCE_LIMIT_EXCEEDED");
        assert_eq!(json["details"]["resource"]["cause"], "arithmetic_overflow");
        assert_eq!(
            json["details"]["resource"]["limit_id"],
            "max_layout_work_units"
        );
        assert_eq!(json["details"]["resource"]["phase"], "layout_model");
        assert_eq!(
            json["details"]["resource"]["actual"],
            "18446744073709551615"
        );
        assert_eq!(json["details"]["resource"]["max"], 800_000);
        assert_eq!(json["details"]["resource"]["profile"], "interactive");

        let err = BindingError::new(
            merman_bindings_core::BindingStatus::ParseError,
            "invalid edge",
        )
        .with_diagnostic_details(
            merman_bindings_core::BindingDiagnosticErrorDetails::new("flowchart.edge.invalid")
                .with_span(merman_bindings_core::BindingDiagnosticSpan::new(
                    3, 8, "exact",
                ))
                .with_field("edge")
                .with_diagram_type("flowchart"),
        );
        let json = binding_error_payload_value(&err).unwrap();
        assert_eq!(
            json["details"]["diagnostic"]["code"],
            "flowchart.edge.invalid"
        );
        assert_eq!(json["details"]["diagnostic"]["span"]["start"], 3);
        assert_eq!(json["details"]["diagnostic"]["span"]["end"], 8);
        assert_eq!(json["details"]["diagnostic"]["span"]["kind"], "exact");
        assert_eq!(json["details"]["diagnostic"]["field"], "edge");
        assert_eq!(json["details"]["diagnostic"]["diagram_type"], "flowchart");
    }

    #[test]
    fn wasm_execution_is_bound_to_the_deterministic_web_contract() {
        let error = execute_wasm_operation(
            "semantic-json",
            b"flowchart TD\nA --> B",
            br#"{"runtime_policy":"native"}"#,
            None,
        )
        .expect_err("the Web contract must reject native runtime policy selection");

        assert_eq!(
            error.status(),
            merman_bindings_core::BindingStatus::OptionsJsonError
        );
        assert!(error.message().contains("not exposed by target `web`"));
    }

    #[test]
    fn runtime_catalog_follows_the_wasm_feature_surface_and_local_relations() {
        let catalog = wasm_runtime_catalog();
        assert_eq!(
            catalog
                .payload_schemas
                .iter()
                .map(|schema| (schema.id, schema.version))
                .collect::<Vec<_>>(),
            vec![(
                "binding-result",
                merman_bindings_core::BINDING_RESULT_PAYLOAD_VERSION,
            )]
        );
        let capabilities = catalog.capabilities;

        assert_eq!(
            capabilities.has_capability("analysis"),
            cfg!(feature = "analysis")
        );
        assert_eq!(
            capabilities.has_capability("ascii"),
            cfg!(feature = "ascii")
        );
        assert_eq!(
            wasm_ascii_supported_diagrams().is_empty(),
            !cfg!(feature = "ascii")
        );
        assert_eq!(capabilities.has_capability("svg"), cfg!(feature = "svg"));
        #[cfg(feature = "svg")]
        {
            assert_eq!(
                capabilities.has_capability("layout-cytoscape"),
                cfg!(feature = "layout-cytoscape")
            );
            assert_eq!(
                capabilities.has_capability("layout-elk"),
                cfg!(feature = "layout-elk")
            );
            assert_eq!(capabilities.has_capability("math"), cfg!(feature = "math"));
        }
        assert!(
            capabilities.system_adapter_ids.is_empty(),
            "browser WASM must not claim native system adapters"
        );
        assert_eq!(
            capabilities.has_capability("editor"),
            cfg!(feature = "editor")
        );
        if capabilities.has_capability("svg") {
            let providers = capabilities
                .text_measurement
                .as_ref()
                .expect("SVG surface must report a text-measurement route")
                .provider_ids
                .as_slice();
            assert!(providers.contains(&"vendored"));
            assert_eq!(
                providers.contains(&"host-callback"),
                cfg!(all(feature = "svg", target_arch = "wasm32"))
            );
        } else {
            assert!(capabilities.text_measurement.is_none());
        }

        assert!(
            capabilities
                .output_ids
                .iter()
                .all(|output| capabilities.has_operation(output))
        );
        assert!(
            capabilities
                .system_adapter_ids
                .iter()
                .all(|adapter| capabilities.has_capability(adapter))
        );
    }

    #[test]
    fn runtime_catalog_uses_the_wasm_transport_api_and_resource_catalog() {
        let catalog = wasm_runtime_catalog();
        assert_eq!(
            catalog.schema_version,
            merman_bindings_core::RUNTIME_CATALOG_SCHEMA_VERSION
        );
        assert_eq!(catalog.transport_api_version, WASM_TRANSPORT_API_VERSION);
        #[cfg(target_arch = "wasm32")]
        assert!(
            catalog
                .output_contracts
                .iter()
                .all(|output| output.system_fonts.is_none()),
            "browser WASM cannot discover host system fonts"
        );
        let operation_ids = &catalog.capabilities.operation_ids;
        let resources = catalog.resources;
        assert_eq!(resources.general_binding_default_profile, "interactive");
        assert_eq!(resources.profiles.len(), 4);
        #[cfg(feature = "ascii")]
        for case in ascii_resource_boundary_contract().binding_core_interactive {
            let limit = resources
                .limits
                .iter()
                .find(|limit| limit.id == case.id)
                .unwrap_or_else(|| panic!("WASM resource catalog is missing {}", case.id));
            assert!(
                limit.operation_ids.contains(&"ascii"),
                "WASM resource catalog does not route {} to ASCII",
                case.id
            );
        }
        for limit in resources.limits {
            assert!(
                limit
                    .operation_ids
                    .iter()
                    .all(|operation_id| operation_ids.contains(operation_id)),
                "resource limit {} must only name callable Web operations",
                limit.id
            );
        }
    }

    #[test]
    fn family_capabilities_expose_the_unique_catalog() {
        let capabilities = merman_bindings_core::diagram_family_capabilities();
        assert!(capabilities.iter().any(|capability| {
            capability.diagram_type == "flowchart"
                && capability.logical_family_kind == "flowchart"
                && capability.metadata_id == Some("flowchart")
                && capability.render_model_kind == Some("flowchart")
                && capability.has_detector
                && capability.has_semantic_parser
                && capability.has_editor_parser
                && capability.has_combined_parser
                && capability.has_render_parser
                && !capability.has_header
                && capability.config_namespace == Some("flowchart")
        }));
        assert!(
            capabilities
                .iter()
                .any(|capability| capability.diagram_type == "mindmap")
        );
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn render_ascii_impl_returns_text() {
        let text = string_result(merman_bindings_core::render_ascii(
            b"flowchart TD\nA[Hello] --> B[World]",
            b"",
        ))
        .unwrap();

        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn render_ascii_impl_applies_flowchart_node_label_wrap_width() {
        let text = render_ascii(
            "flowchart TD\nA[\"Alpha Beta Gamma Delta\"]",
            Some(r#"{ "ascii": { "flowchartNodeLabelWrapWidth": 8 } }"#.to_string()),
        )
        .unwrap();

        for expected in ["Alpha", "Beta", "Gamma", "Delta"] {
            assert!(text.contains(expected), "missing {expected:?}:\n{text}");
        }
        assert!(!text.contains("Alpha Beta Gamma Delta"), "{text}");
    }

    #[cfg(feature = "ascii")]
    #[test]
    fn wasm_ascii_operation_preserves_a_typed_exact_resource_boundary() {
        let case = ascii_resource_boundary_contract()
            .transport_representatives
            .wasm_interactive;
        let expected = case.exact;

        let exact_options = wasm_ascii_options(&case.id, expected);
        let output = execute_wasm_operation(
            "ascii",
            case.source.as_bytes(),
            exact_options.as_bytes(),
            None,
        )
        .unwrap_or_else(|error| panic!("exact {} boundary failed: {error:?}", case.id));
        assert!(!output.is_empty());

        let below_options = wasm_ascii_options(&case.id, expected - 1);
        let error = execute_wasm_operation(
            "ascii",
            case.source.as_bytes(),
            below_options.as_bytes(),
            None,
        )
        .expect_err("one-below WASM ASCII boundary must fail");
        let actual = assert_wasm_ascii_resource_error(error, &case.id, &case.phase, expected - 1);
        assert_eq!(actual, expected);
    }
}

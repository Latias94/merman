#![forbid(unsafe_code)]

//! WebAssembly bindings for browser integrations.
//!
//! The crate intentionally stays thin: all parsing, rendering, options parsing, and error
//! classification are delegated to `merman-bindings-core`.

use merman_bindings_core::BindingError;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[cfg(all(feature = "render", target_arch = "wasm32"))]
use std::{cell::RefCell, sync::Arc};

#[cfg(feature = "editor-language")]
mod editor_language;

#[cfg(feature = "editor-language")]
pub use editor_language::{
    editor_code_actions, editor_completions, editor_definition, editor_diagnostics,
    editor_document_symbols, editor_hover, editor_prepare_rename, editor_references, editor_rename,
    editor_semantic_token_legend, editor_semantic_tokens, editor_workspace_symbols,
};

#[cfg(all(feature = "render", target_arch = "wasm32"))]
use merman_bindings_core::{TextStyle, WrapMode};
#[cfg(all(feature = "render", target_arch = "wasm32"))]
use serde::Deserialize;

const WASM_ABI_VERSION: u32 = 2;

#[derive(Debug, Serialize)]
struct WasmErrorPayload<'a> {
    version: u32,
    ok: bool,
    code: i32,
    code_name: &'a str,
    message: &'a str,
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen(js_name = abiVersion)]
pub fn abi_version() -> u32 {
    WASM_ABI_VERSION
}

#[wasm_bindgen(js_name = packageVersion)]
pub fn package_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[wasm_bindgen(js_name = renderSvg)]
pub fn render_svg(source: &str, options_json: Option<String>) -> Result<String, JsValue> {
    string_result(merman_bindings_core::render_svg(
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
    ))
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
#[wasm_bindgen(js_name = renderSvgWithTextMeasurer)]
pub fn render_svg_with_text_measurer(
    source: &str,
    options_json: Option<String>,
    callback: js_sys::Function,
) -> Result<String, JsValue> {
    with_host_text_measure_callback(callback, || {
        let engine =
            merman_bindings_core::BindingEngine::new(options_bytes(options_json.as_deref()))
                .map_err(binding_error_to_js)?;
        let engine = engine.with_host_text_measurer(Arc::new(WasmHostTextMeasurer));
        string_result(engine.render_svg(source.as_bytes()))
    })
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
#[wasm_bindgen(js_name = layoutJsonWithTextMeasurer)]
pub fn layout_json_with_text_measurer(
    source: &str,
    options_json: Option<String>,
    callback: js_sys::Function,
) -> Result<String, JsValue> {
    with_host_text_measure_callback(callback, || {
        let engine =
            merman_bindings_core::BindingEngine::new(options_bytes(options_json.as_deref()))
                .map_err(binding_error_to_js)?;
        let engine = engine.with_host_text_measurer(Arc::new(WasmHostTextMeasurer));
        string_result(engine.layout_json(source.as_bytes()))
    })
}

#[wasm_bindgen(js_name = parseJson)]
pub fn parse_json(source: &str, options_json: Option<String>) -> Result<String, JsValue> {
    string_result(merman_bindings_core::parse_json(
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
    ))
}

#[wasm_bindgen(js_name = layoutJson)]
pub fn layout_json(source: &str, options_json: Option<String>) -> Result<String, JsValue> {
    string_result(merman_bindings_core::layout_json(
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
    ))
}

#[wasm_bindgen(js_name = renderAscii)]
pub fn render_ascii(source: &str, options_json: Option<String>) -> Result<String, JsValue> {
    string_result(merman_bindings_core::render_ascii(
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
    ))
}

#[wasm_bindgen]
pub fn analyze(source: &str, options_json: Option<String>) -> Result<JsValue, JsValue> {
    json_value_result(merman_bindings_core::analyze_json(
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
    ))
}

#[wasm_bindgen(js_name = analyzeJson)]
pub fn analyze_json(source: &str, options_json: Option<String>) -> Result<JsValue, JsValue> {
    analyze(source, options_json)
}

#[wasm_bindgen(js_name = analysisFacts)]
pub fn analysis_facts(source: &str, options_json: Option<String>) -> Result<JsValue, JsValue> {
    json_value_result(merman_bindings_core::analysis_facts_json(
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
    ))
}

#[wasm_bindgen(js_name = analyzeDocument)]
pub fn analyze_document(
    source: &str,
    options_json: Option<String>,
    uri: Option<String>,
) -> Result<JsValue, JsValue> {
    let uri = document_uri(uri);
    json_value_result(merman_bindings_core::analyze_document_json(
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
        uri.as_bytes(),
    ))
}

#[wasm_bindgen(js_name = analyzeDocumentFacts)]
pub fn analyze_document_facts(
    source: &str,
    options_json: Option<String>,
    uri: Option<String>,
) -> Result<JsValue, JsValue> {
    let uri = document_uri(uri);
    json_value_result(merman_bindings_core::analyze_document_facts_json(
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
        uri.as_bytes(),
    ))
}

#[wasm_bindgen]
pub fn validate(source: &str, options_json: Option<String>) -> Result<JsValue, JsValue> {
    json_value_result(merman_bindings_core::validate_json(
        source.as_bytes(),
        options_bytes(options_json.as_deref()),
    ))
}

#[wasm_bindgen(js_name = supportedDiagrams)]
pub fn supported_diagrams() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(merman_bindings_core::supported_diagrams())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen(js_name = bindingCapabilities)]
pub fn binding_capabilities() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&merman_bindings_core::binding_capabilities())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen(js_name = selectedRegistryProfile)]
pub fn selected_registry_profile() -> String {
    merman_bindings_core::selected_registry_profile().to_string()
}

#[wasm_bindgen(js_name = diagramFamilyCapabilities)]
pub fn diagram_family_capabilities() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&merman_bindings_core::diagram_family_capabilities())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen(js_name = lintRuleCatalog)]
pub fn lint_rule_catalog() -> Result<JsValue, JsValue> {
    json_value_result(merman_bindings_core::lint_rule_catalog_json())
}

#[wasm_bindgen(js_name = supportedThemes)]
pub fn supported_themes() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(merman_bindings_core::supported_themes())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen(js_name = supportedHostThemePresets)]
pub fn supported_host_theme_presets() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(merman_bindings_core::supported_host_theme_presets())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen(js_name = asciiSupportedDiagrams)]
pub fn ascii_supported_diagrams() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(merman_bindings_core::ascii_supported_diagrams())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

#[wasm_bindgen(js_name = asciiCapabilities)]
pub fn ascii_capabilities() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(&merman_bindings_core::ascii_capabilities())
        .map_err(|err| JsValue::from_str(&err.to_string()))
}

fn options_bytes(options_json: Option<&str>) -> &[u8] {
    options_json.unwrap_or_default().as_bytes()
}

pub(crate) fn document_uri(uri: Option<String>) -> String {
    uri.filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "file:///merman/document.mmd".to_string())
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
    let payload = wasm_error_payload(&err);
    payload
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .unwrap_or_else(|_| {
            JsValue::from_str(&format!("{}: {}", payload.code_name, payload.message))
        })
}

fn wasm_error_payload(err: &BindingError) -> WasmErrorPayload<'_> {
    WasmErrorPayload {
        version: 1,
        ok: false,
        code: err.status().code(),
        code_name: err.status().code_name(),
        message: err.message(),
    }
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
thread_local! {
    static HOST_TEXT_MEASURE_CALLBACK: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
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

#[cfg(all(feature = "render", target_arch = "wasm32"))]
#[derive(Debug, Deserialize)]
struct WasmHostTextMeasureResult {
    handled: Option<bool>,
    kind: Option<String>,
    width: Option<f64>,
    height: Option<f64>,
    length: Option<f64>,
    line_count: Option<usize>,
    bbox_left: Option<f64>,
    bbox_right: Option<f64>,
    raw_width: Option<f64>,
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
struct WasmHostTextMeasurer;

#[cfg(all(feature = "render", target_arch = "wasm32"))]
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

            let result: WasmHostTextMeasureResult =
                serde_wasm_bindgen::from_value(value).map_err(|err| {
                    merman_bindings_core::HostTextMeasurementError::new(err.to_string())
                })?;
            if result.handled == Some(false) {
                return Ok(None);
            }

            Ok(Some(
                merman_bindings_core::host_text_measurement_from_values(
                    result.kind.as_deref().and_then(
                        merman_bindings_core::HostTextMeasurementResultKind::from_external_name,
                    ),
                    merman_bindings_core::HostTextMeasurementValues {
                        width: result.width.unwrap_or(f64::NAN),
                        height: result.height.unwrap_or(f64::NAN),
                        line_count: result.line_count.unwrap_or(0),
                        length: result.length.unwrap_or(f64::NAN),
                        bbox_left: result.bbox_left.unwrap_or(f64::NAN),
                        bbox_right: result.bbox_right.unwrap_or(f64::NAN),
                        raw_width: result.raw_width,
                    },
                ),
            ))
        })
    }
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
impl merman_bindings_core::HostTextMeasurer for WasmHostTextMeasurer {
    fn measure(
        &self,
        request: merman_bindings_core::HostTextMeasurementRequest<'_>,
    ) -> merman_bindings_core::HostMeasurementResult {
        self.call_host(request)
    }
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
struct HostTextMeasureCallbackGuard {
    previous_callback: Option<js_sys::Function>,
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
impl Drop for HostTextMeasureCallbackGuard {
    fn drop(&mut self) {
        HOST_TEXT_MEASURE_CALLBACK.with(|slot| {
            slot.replace(self.previous_callback.take());
        });
    }
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
fn with_host_text_measure_callback<R>(callback: js_sys::Function, f: impl FnOnce() -> R) -> R {
    let previous_callback = HOST_TEXT_MEASURE_CALLBACK.with(|slot| slot.replace(Some(callback)));
    let _guard = HostTextMeasureCallbackGuard { previous_callback };
    f()
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
fn js_error_message(err: &JsValue) -> String {
    err.as_string()
        .unwrap_or_else(|| "host text measurer callback failed".to_string())
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
fn wasm_measurement_phase(phase: merman_bindings_core::TextMeasurementPhase) -> &'static str {
    match phase {
        merman_bindings_core::TextMeasurementPhase::Layout => "layout",
        merman_bindings_core::TextMeasurementPhase::Wrap => "wrap",
        merman_bindings_core::TextMeasurementPhase::SvgBBox => "svg-bbox",
        merman_bindings_core::TextMeasurementPhase::ComputedLength => "computed-length",
    }
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
fn wasm_wrap_mode(wrap_mode: WrapMode) -> &'static str {
    match wrap_mode {
        WrapMode::SvgLike => "svg-like",
        WrapMode::SvgLikeSingleRun => "svg-like-single-run",
        WrapMode::HtmlLike => "html-like",
    }
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
fn wasm_line_height(style: &TextStyle, wrap_mode: WrapMode) -> f64 {
    let factor = match wrap_mode {
        WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => 1.1,
        WrapMode::HtmlLike => 1.5,
    };
    style.font_size.max(1.0) * factor
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
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
    #[cfg(feature = "analysis")]
    use serde_json::Value;

    #[test]
    fn package_version_matches_crate_version() {
        assert_eq!(package_version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn abi_version_matches_the_host_measurement_protocol() {
        assert_eq!(abi_version(), 2);
    }

    #[cfg(feature = "render")]
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

    #[test]
    fn render_svg_impl_returns_svg() {
        let result = merman_bindings_core::render_svg(b"flowchart TD\nA[Hello] --> B[World]", b"");

        if cfg!(feature = "render") {
            let svg = string_result(result).unwrap();
            assert!(svg.contains("<svg"));
            assert!(svg.contains("Hello"));
        } else {
            let error = result.unwrap_err();
            assert_eq!(
                error.status(),
                merman_bindings_core::BindingStatus::UnsupportedFormat
            );
        }
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

    #[cfg(not(feature = "analysis"))]
    #[test]
    fn analysis_entry_points_report_missing_analysis_feature() {
        let err = merman_bindings_core::validate_json(b"flowchart TD\nA", b"").unwrap_err();
        assert_eq!(
            err.status(),
            merman_bindings_core::BindingStatus::UnsupportedFormat
        );
        assert!(err.message().contains("analysis feature"));
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
        assert!(
            value["diagrams"][0]["syntax"]["semantic_items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| {
                    item["name"] == "A"
                        && item["rename_policy"] == "identifier"
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
                None,
                Some("file:///tmp/example.md".to_string()),
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
                b"",
                b"file:///tmp/example.md",
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
                None,
                Some("file:///tmp/example.md".to_string()),
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
                b"",
                b"file:///tmp/example.md",
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
        let json = serde_json::to_value(wasm_error_payload(&err)).unwrap();

        assert_eq!(json["version"], 1);
        assert_eq!(json["ok"], false);
        if cfg!(feature = "render") {
            assert_eq!(json["code_name"], "MERMAN_OPTIONS_JSON_ERROR");
            assert!(json["message"].as_str().unwrap().contains("options_json"));
        } else {
            assert_eq!(json["code_name"], "MERMAN_UNSUPPORTED_FORMAT");
        }
    }

    #[test]
    fn binding_capabilities_follow_features() {
        let capabilities = merman_bindings_core::binding_capabilities();

        assert_eq!(capabilities.render, cfg!(feature = "render"));
        assert_eq!(capabilities.analysis, cfg!(feature = "analysis"));
        assert_eq!(capabilities.ascii, cfg!(feature = "ascii"));
        assert_eq!(capabilities.core_full, cfg!(feature = "core-full"));
        assert_eq!(capabilities.core_host, cfg!(feature = "core-host"));
        assert_eq!(capabilities.elk_layout, cfg!(feature = "elk-layout"));
        assert_eq!(capabilities.ratex_math, cfg!(feature = "ratex-math"));
        assert_eq!(
            capabilities.editor_language,
            cfg!(feature = "editor-language")
        );
    }

    #[test]
    fn registry_profile_and_family_capabilities_are_exposed() {
        let expected_profile = if cfg!(feature = "core-full") {
            "full"
        } else {
            "tiny"
        };
        assert_eq!(selected_registry_profile(), expected_profile);

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
        assert_eq!(
            capabilities
                .iter()
                .any(|capability| capability.diagram_type == "mindmap"),
            cfg!(feature = "core-full")
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
}

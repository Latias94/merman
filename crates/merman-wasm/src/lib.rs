#![forbid(unsafe_code)]

//! WebAssembly bindings for browser integrations.
//!
//! The crate intentionally stays thin: all parsing, rendering, options parsing, and error
//! classification are delegated to `merman-bindings-core`.

use merman_bindings_core::BindingError;
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[cfg(all(feature = "render", target_arch = "wasm32"))]
use merman_bindings_core::{TextMeasurer, TextMetrics, TextStyle, WrapMode};
#[cfg(all(feature = "render", target_arch = "wasm32"))]
use serde::Deserialize;
#[cfg(all(feature = "render", target_arch = "wasm32"))]
use std::{cell::RefCell, sync::Arc};

const WASM_ABI_VERSION: u32 = 1;

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
        let engine = engine.with_text_measurer(Arc::new(WasmHostTextMeasurer::default()));
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
        let engine = engine.with_text_measurer(Arc::new(WasmHostTextMeasurer::default()));
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

fn options_bytes(options_json: Option<&str>) -> &[u8] {
    options_json.unwrap_or_default().as_bytes()
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

fn binding_error_to_js(err: BindingError) -> JsValue {
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
    text: &'a str,
    font_family: Option<&'a str>,
    font_size: f64,
    font_weight: Option<&'a str>,
    font_style: &'static str,
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
    width: f64,
    height: f64,
    line_count: Option<usize>,
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
#[derive(Default)]
struct WasmHostTextMeasurer {
    fallback: merman_bindings_core::VendoredFontMetricsTextMeasurer,
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
impl WasmHostTextMeasurer {
    fn call_host(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> Option<TextMetrics> {
        let request = WasmHostTextMeasureRequest {
            text,
            font_family: style.font_family.as_deref(),
            font_size: style.font_size,
            font_weight: style.font_weight.as_deref(),
            font_style: "normal",
            max_width,
            has_max_width: max_width.is_some(),
            line_height: wasm_line_height(style, wrap_mode),
            letter_spacing: 0.0,
            word_spacing: 0.0,
            wrap_mode: wasm_wrap_mode(wrap_mode),
            direction: "auto",
            white_space: wasm_white_space(max_width, wrap_mode),
        };
        let request = serde_wasm_bindgen::to_value(&request).ok()?;

        HOST_TEXT_MEASURE_CALLBACK.with(|slot| {
            let callback = slot.borrow().clone()?;
            let value = callback.call1(&JsValue::NULL, &request).ok()?;
            if value.is_null() || value.is_undefined() {
                return None;
            }

            let result: WasmHostTextMeasureResult = serde_wasm_bindgen::from_value(value).ok()?;
            if result.handled == Some(false)
                || !result.width.is_finite()
                || !result.height.is_finite()
                || result.width < 0.0
                || result.height < 0.0
            {
                return None;
            }

            let line_count = result.line_count.unwrap_or(1);
            if line_count == 0 {
                return None;
            }

            Some(TextMetrics {
                width: result.width,
                height: result.height,
                line_count,
            })
        })
    }

    fn measure_with_fallback(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.call_host(text, style, max_width, wrap_mode)
            .unwrap_or_else(|| {
                self.fallback
                    .measure_wrapped(text, style, max_width, wrap_mode)
            })
    }
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
impl TextMeasurer for WasmHostTextMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.call_host(text, style, None, WrapMode::SvgLike)
            .unwrap_or_else(|| self.fallback.measure(text, style))
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.measure_with_fallback(text, style, max_width, wrap_mode)
    }

    fn measure_wrapped_with_raw_width(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> (TextMetrics, Option<f64>) {
        if let Some(metrics) = self.call_host(text, style, max_width, wrap_mode) {
            let raw_width = max_width
                .and_then(|_| self.call_host(text, style, None, wrap_mode))
                .map(|raw| raw.width);
            return (metrics, raw_width);
        }
        self.fallback
            .measure_wrapped_with_raw_width(text, style, max_width, wrap_mode)
    }

    fn measure_wrapped_raw(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.call_host(text, style, max_width, wrap_mode)
            .unwrap_or_else(|| {
                self.fallback
                    .measure_wrapped_raw(text, style, max_width, wrap_mode)
            })
    }
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
struct HostTextMeasureCallbackGuard(Option<js_sys::Function>);

#[cfg(all(feature = "render", target_arch = "wasm32"))]
impl Drop for HostTextMeasureCallbackGuard {
    fn drop(&mut self) {
        HOST_TEXT_MEASURE_CALLBACK.with(|slot| {
            slot.replace(self.0.take());
        });
    }
}

#[cfg(all(feature = "render", target_arch = "wasm32"))]
fn with_host_text_measure_callback<R>(callback: js_sys::Function, f: impl FnOnce() -> R) -> R {
    let previous = HOST_TEXT_MEASURE_CALLBACK.with(|slot| slot.replace(Some(callback)));
    let _guard = HostTextMeasureCallbackGuard(previous);
    f()
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
    use serde_json::Value;

    #[test]
    fn package_version_matches_crate_version() {
        assert_eq!(package_version(), env!("CARGO_PKG_VERSION"));
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

    #[test]
    fn validation_error_uses_binding_status() {
        let json: Value =
            serde_json::from_slice(&merman_bindings_core::validate_json(b"", b"").unwrap())
                .unwrap();

        assert_eq!(json["valid"], false);
        if cfg!(feature = "render") {
            assert_eq!(json["code_name"], "MERMAN_NO_DIAGRAM");
            assert!(
                json["error"]
                    .as_str()
                    .unwrap()
                    .contains("no Mermaid diagram")
            );
        } else {
            assert_eq!(json["code_name"], "MERMAN_UNSUPPORTED_FORMAT");
        }
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
        assert_eq!(capabilities.ascii, cfg!(feature = "ascii"));
        assert_eq!(
            capabilities.core_full,
            cfg!(feature = "core-full") || cfg!(feature = "ascii")
        );
        assert_eq!(
            capabilities.core_host,
            cfg!(feature = "core-host") || cfg!(feature = "ascii")
        );
        assert_eq!(capabilities.elk_layout, cfg!(feature = "elk-layout"));
        assert_eq!(capabilities.ratex_math, cfg!(feature = "ratex-math"));
    }

    #[test]
    fn registry_profile_and_family_capabilities_are_exposed() {
        let expected_profile = if cfg!(feature = "core-full") || cfg!(feature = "ascii") {
            "full"
        } else {
            "tiny"
        };
        assert_eq!(selected_registry_profile(), expected_profile);

        let capabilities = merman_bindings_core::diagram_family_capabilities();
        assert!(
            capabilities
                .iter()
                .any(|capability| capability.diagram_type == "flowchart"
                    && capability.has_semantic_parser
                    && capability.has_render_parser)
        );
        assert_eq!(
            capabilities
                .iter()
                .any(|capability| capability.diagram_type == "mindmap"),
            expected_profile == "full"
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

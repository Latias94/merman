#![forbid(unsafe_code)]

//! WebAssembly bindings for browser integrations.
//!
//! The crate intentionally stays thin: all parsing, rendering, options parsing, and error
//! classification are delegated to `merman-bindings-core`.

use merman_bindings_core::BindingError;
use serde::Serialize;
use wasm_bindgen::prelude::*;

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

#[wasm_bindgen(js_name = supportedThemes)]
pub fn supported_themes() -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(merman_bindings_core::supported_themes())
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
        let svg = string_result(merman_bindings_core::render_svg(
            b"flowchart TD\nA[Hello] --> B[World]",
            b"",
        ))
        .unwrap();

        assert!(svg.contains("<svg"));
        assert!(svg.contains("Hello"));
    }

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

    #[test]
    fn wasm_error_payload_is_structured() {
        let err = merman_bindings_core::render_svg(b"flowchart TD\nA", b"{").unwrap_err();
        let json = serde_json::to_value(wasm_error_payload(&err)).unwrap();

        assert_eq!(json["version"], 1);
        assert_eq!(json["ok"], false);
        assert_eq!(json["code_name"], "MERMAN_OPTIONS_JSON_ERROR");
        assert!(json["message"].as_str().unwrap().contains("options_json"));
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

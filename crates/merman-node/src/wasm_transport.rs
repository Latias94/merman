use wasm_bindgen::prelude::*;

use crate::wire;

#[wasm_bindgen(js_name = "WasmEngine")]
pub struct WasmEngine {
    engine: merman_bindings_core::BindingEngine,
}

#[wasm_bindgen]
impl WasmEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(options_json: String) -> Result<WasmEngine, JsValue> {
        wire::create_engine(&options_json)
            .map(|engine| Self { engine })
            .map_err(|error| {
                serde_wasm_bindgen::to_value(&wire::error_value(&error))
                    .unwrap_or_else(|_| JsValue::from_str(error.message()))
            })
    }

    pub fn execute(&self, request_json: String) -> String {
        wire::execute_wire(&self.engine, &request_json)
    }

    #[wasm_bindgen(js_name = "runtimeCatalogJson")]
    pub fn runtime_catalog_json(&self) -> Result<String, JsValue> {
        wire::runtime_catalog_wire().map_err(|error| {
            serde_wasm_bindgen::to_value(&wire::error_value(&error))
                .unwrap_or_else(|_| JsValue::from_str(error.message()))
        })
    }

    #[wasm_bindgen(js_name = "metadataJson")]
    pub fn metadata_json(&self, id: String) -> Result<String, JsValue> {
        wire::metadata_wire(&id).map_err(|error| {
            serde_wasm_bindgen::to_value(&wire::error_value(&error))
                .unwrap_or_else(|_| JsValue::from_str(error.message()))
        })
    }

    #[wasm_bindgen(js_name = "executeSync")]
    pub fn execute_sync(&self, request_json: String) -> String {
        wire::execute_wire(&self.engine, &request_json)
    }

    pub fn dispose(&mut self) {}
}

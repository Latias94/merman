use wasm_bindgen::prelude::*;

use crate::wire;

#[wasm_bindgen(js_name = "WasmEngine")]
pub struct WasmEngine {
    engine: Option<merman_bindings_core::BindingEngine>,
}

impl WasmEngine {
    fn engine(&self) -> Result<&merman_bindings_core::BindingEngine, JsValue> {
        self.engine
            .as_ref()
            .ok_or_else(|| JsValue::from_str(&wire::error_envelope(&wire::disposed_error())))
    }
}

#[wasm_bindgen]
impl WasmEngine {
    #[wasm_bindgen(constructor)]
    pub fn new(options_json: String) -> Result<WasmEngine, JsValue> {
        wire::create_engine(&options_json)
            .map(|engine| Self {
                engine: Some(engine),
            })
            .map_err(|error| JsValue::from_str(&wire::error_envelope(&error)))
    }

    pub fn execute(
        &self,
        request_json: String,
        timeout_ms: Option<u32>,
    ) -> Result<String, JsValue> {
        Ok(wire::execute_wire_with_admitted_control(
            self.engine()?,
            &request_json,
            wire::admitted_operation_control(timeout_ms),
            timeout_ms,
        ))
    }

    #[wasm_bindgen(js_name = "runtimeCatalogJson")]
    pub fn runtime_catalog_json(&self) -> Result<String, JsValue> {
        self.engine()?;
        wire::runtime_catalog_wire()
            .map_err(|error| JsValue::from_str(&wire::error_envelope(&error)))
    }

    #[wasm_bindgen(js_name = "metadataJson")]
    pub fn metadata_json(&self, id: String) -> Result<String, JsValue> {
        self.engine()?;
        wire::metadata_wire(&id).map_err(|error| JsValue::from_str(&wire::error_envelope(&error)))
    }

    #[wasm_bindgen(js_name = "executeSync")]
    pub fn execute_sync(
        &self,
        request_json: String,
        timeout_ms: Option<u32>,
    ) -> Result<String, JsValue> {
        Ok(wire::execute_wire_with_admitted_control(
            self.engine()?,
            &request_json,
            wire::admitted_operation_control(timeout_ms),
            timeout_ms,
        ))
    }

    pub fn dispose(&mut self) {
        self.engine.take();
    }
}

#[wasm_bindgen(js_name = "transportIdentityJson")]
pub fn transport_identity_json() -> Result<String, JsValue> {
    wire::transport_identity_wire(wire::NodeTransportKind::Wasm)
        .map_err(|error| JsValue::from_str(&wire::error_envelope(&error)))
}

#[cfg(test)]
mod tests {
    use super::WasmEngine;

    #[test]
    fn dispose_is_idempotent_and_removes_the_engine() {
        let mut engine = WasmEngine::new(String::new()).expect("construct WasmEngine");
        engine.dispose();
        engine.dispose();
        assert!(engine.engine.is_none());
    }
}

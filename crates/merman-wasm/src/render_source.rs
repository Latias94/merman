//! A render-capable WASM instance owns one immutable canonical parse snapshot.

use crate::{
    binding_error_to_js, options_bytes, string_result, svg_result, wasm_artifact_contract,
    wasm_operation_control, wasm_options,
};
use merman_bindings_core::{BindingEngineServices, BindingError, BindingPreparedSource};
use std::cell::Cell;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = PreparedSource)]
pub struct WasmPreparedSource {
    source: BindingPreparedSource,
    active: Cell<bool>,
}

#[wasm_bindgen(js_class = PreparedSource)]
impl WasmPreparedSource {
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str, options_json: Option<String>) -> Result<WasmPreparedSource, JsValue> {
        let (options, timeout) =
            wasm_options(options_bytes(options_json.as_deref())).map_err(binding_error_to_js)?;
        let control = wasm_operation_control(timeout);
        control
            .checkpoint_at(merman_bindings_core::OperationPhase::Admission)
            .map_err(BindingError::cancelled)
            .map_err(binding_error_to_js)?;
        let engine = wasm_artifact_contract()
            .create_engine(&options)
            .map_err(binding_error_to_js)?;
        let source = engine
            .prepare_source(source.as_bytes(), control)
            .map_err(binding_error_to_js)?;
        Ok(Self {
            source,
            active: Cell::new(false),
        })
    }

    #[wasm_bindgen(js_name = renderSvg)]
    pub fn render_svg(&self, options_json: Option<String>) -> Result<String, JsValue> {
        self.with_operation(|| {
            string_result(self.output_options(options_json, |options, control| {
                self.source
                    .render_svg(options, control, &BindingEngineServices::new())
            }))
        })
    }

    #[wasm_bindgen(js_name = renderSvgResult)]
    pub fn render_svg_result(&self, options_json: Option<String>) -> Result<JsValue, JsValue> {
        self.with_operation(|| {
            svg_result(self.output_options(options_json, |options, control| {
                self.source
                    .render_svg_result(options, control, &BindingEngineServices::new())
            }))
        })
    }

    #[wasm_bindgen(js_name = layoutJson)]
    pub fn layout_json(&self, options_json: Option<String>) -> Result<String, JsValue> {
        self.with_operation(|| {
            string_result(self.output_options(options_json, |options, control| {
                self.source
                    .layout_json(options, control, &BindingEngineServices::new())
            }))
        })
    }

    #[wasm_bindgen(js_name = edgeGeometryJson)]
    pub fn edge_geometry_json(&self, options_json: Option<String>) -> Result<String, JsValue> {
        self.with_operation(|| {
            string_result(self.output_options(options_json, |options, control| {
                self.source
                    .edge_geometry_json(options, control, &BindingEngineServices::new())
            }))
        })
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = renderSvgWithTextMeasurer)]
    pub fn render_svg_with_text_measurer(
        &self,
        options_json: Option<String>,
        callback: js_sys::Function,
    ) -> Result<String, JsValue> {
        self.with_operation(|| {
            self.with_measurer(callback, |services| {
                string_result(self.output_options(options_json, |options, control| {
                    self.source.render_svg(options, control, &services)
                }))
            })
        })
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = renderSvgResultWithTextMeasurer)]
    pub fn render_svg_result_with_text_measurer(
        &self,
        options_json: Option<String>,
        callback: js_sys::Function,
    ) -> Result<JsValue, JsValue> {
        self.with_operation(|| {
            self.with_measurer(callback, |services| {
                svg_result(self.output_options(options_json, |options, control| {
                    self.source.render_svg_result(options, control, &services)
                }))
            })
        })
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = layoutJsonWithTextMeasurer)]
    pub fn layout_json_with_text_measurer(
        &self,
        options_json: Option<String>,
        callback: js_sys::Function,
    ) -> Result<String, JsValue> {
        self.with_operation(|| {
            self.with_measurer(callback, |services| {
                string_result(self.output_options(options_json, |options, control| {
                    self.source.layout_json(options, control, &services)
                }))
            })
        })
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = edgeGeometryJsonWithTextMeasurer)]
    pub fn edge_geometry_json_with_text_measurer(
        &self,
        options_json: Option<String>,
        callback: js_sys::Function,
    ) -> Result<String, JsValue> {
        self.with_operation(|| {
            self.with_measurer(callback, |services| {
                string_result(self.output_options(options_json, |options, control| {
                    self.source.edge_geometry_json(options, control, &services)
                }))
            })
        })
    }
}

impl WasmPreparedSource {
    fn output_options<T>(
        &self,
        options_json: Option<String>,
        run: impl FnOnce(&[u8], merman_bindings_core::OperationControl) -> Result<T, BindingError>,
    ) -> Result<T, BindingError> {
        let (options, timeout) = wasm_options(options_bytes(options_json.as_deref()))?;
        run(&options, wasm_operation_control(timeout))
    }

    fn with_operation<T>(&self, run: impl FnOnce() -> Result<T, JsValue>) -> Result<T, JsValue> {
        if self.active.replace(true) {
            return Err(binding_error_to_js(BindingError::reentrant_call(
                "a prepared source cannot be re-entered from its text measurement callback",
            )));
        }
        let _guard = ActiveOperation(&self.active);
        run()
    }

    #[cfg(target_arch = "wasm32")]
    fn with_measurer<T>(
        &self,
        callback: js_sys::Function,
        run: impl FnOnce(BindingEngineServices) -> T,
    ) -> T {
        crate::with_host_text_measure_callback(callback, || {
            run(BindingEngineServices::new()
                .with_host_text_measurer(std::sync::Arc::new(crate::WasmHostTextMeasurer)))
        })
    }
}

struct ActiveOperation<'a>(&'a Cell<bool>);
impl Drop for ActiveOperation<'_> {
    fn drop(&mut self) {
        self.0.set(false);
    }
}

//! Immutable, instance-scoped render services for browser hosts.

use crate::{
    binding_error_to_js, execute_wasm_operation_with_services, options_bytes, string_result,
};
use merman_bindings_core::{BindingEngineServices, BindingError, build_icon_registry_from_json};
use std::cell::Cell;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = RenderSession)]
pub struct WasmRenderSession {
    services: BindingEngineServices,
    active: Cell<bool>,
}

#[wasm_bindgen(js_class = RenderSession)]
impl WasmRenderSession {
    #[wasm_bindgen(constructor)]
    pub fn new(icon_packs_json: Option<String>) -> Result<WasmRenderSession, JsValue> {
        let registry =
            build_icon_registry_from_json(icon_packs_json.as_deref().unwrap_or("[]").as_bytes())
                .map_err(binding_error_to_js)?;
        Ok(Self {
            services: BindingEngineServices::new().with_icon_registry(registry),
            active: Cell::new(false),
        })
    }

    #[wasm_bindgen(js_name = renderSvg)]
    pub fn render_svg(
        &self,
        source: &str,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        self.with_operation(|| self.output("svg", source, options_json, self.services.clone()))
    }

    #[wasm_bindgen(js_name = layoutJson)]
    pub fn layout_json(
        &self,
        source: &str,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        self.with_operation(|| {
            self.output("layout-json", source, options_json, self.services.clone())
        })
    }

    #[wasm_bindgen(js_name = edgeGeometryJson)]
    pub fn edge_geometry_json(
        &self,
        source: &str,
        options_json: Option<String>,
    ) -> Result<String, JsValue> {
        self.with_operation(|| {
            self.output(
                "edge-geometry-json",
                source,
                options_json,
                self.services.clone(),
            )
        })
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = renderSvgWithTextMeasurer)]
    pub fn render_svg_with_text_measurer(
        &self,
        source: &str,
        options_json: Option<String>,
        callback: js_sys::Function,
    ) -> Result<String, JsValue> {
        self.with_operation(|| {
            self.with_measurer(callback, |services| {
                self.output("svg", source, options_json, services)
            })
        })
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = layoutJsonWithTextMeasurer)]
    pub fn layout_json_with_text_measurer(
        &self,
        source: &str,
        options_json: Option<String>,
        callback: js_sys::Function,
    ) -> Result<String, JsValue> {
        self.with_operation(|| {
            self.with_measurer(callback, |services| {
                self.output("layout-json", source, options_json, services)
            })
        })
    }

    #[cfg(target_arch = "wasm32")]
    #[wasm_bindgen(js_name = edgeGeometryJsonWithTextMeasurer)]
    pub fn edge_geometry_json_with_text_measurer(
        &self,
        source: &str,
        options_json: Option<String>,
        callback: js_sys::Function,
    ) -> Result<String, JsValue> {
        self.with_operation(|| {
            self.with_measurer(callback, |services| {
                self.output("edge-geometry-json", source, options_json, services)
            })
        })
    }
}

impl WasmRenderSession {
    fn output(
        &self,
        operation: &'static str,
        source: &str,
        options_json: Option<String>,
        services: BindingEngineServices,
    ) -> Result<String, JsValue> {
        string_result(execute_wasm_operation_with_services(
            operation,
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
            services,
        ))
    }

    fn with_operation<T>(&self, run: impl FnOnce() -> Result<T, JsValue>) -> Result<T, JsValue> {
        if self.active.replace(true) {
            return Err(binding_error_to_js(BindingError::reentrant_call(
                "a render session cannot be re-entered from its text measurement callback",
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
            run(self
                .services
                .clone()
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

use merman_bindings_core::BindingEngine;
use napi::{Env, Task, bindgen_prelude::AsyncTask};
use napi_derive::napi;

use crate::wire;

#[napi(js_name = "NativeEngine")]
pub struct NativeEngine {
    engine: BindingEngine,
}

#[napi]
impl NativeEngine {
    #[napi(constructor)]
    pub fn new(options_json: String) -> napi::Result<Self> {
        wire::create_engine(&options_json)
            .map(|engine| Self { engine })
            .map_err(|error| napi::Error::from_reason(wire::error_envelope(&error)))
    }

    #[napi]
    pub fn execute(&self, request_json: String) -> AsyncTask<ExecuteTask> {
        AsyncTask::new(ExecuteTask {
            engine: self.engine.clone(),
            request_json,
        })
    }

    #[napi(js_name = "runtimeCatalogJson")]
    pub fn runtime_catalog_json(&self) -> napi::Result<String> {
        wire::runtime_catalog_wire()
            .map_err(|error| napi::Error::from_reason(wire::error_envelope(&error)))
    }

    #[napi(js_name = "executeSync")]
    pub fn execute_sync(&self, request_json: String) -> String {
        wire::execute_wire(&self.engine, &request_json)
    }

    #[napi]
    pub fn dispose(&mut self) {}
}

pub struct ExecuteTask {
    engine: BindingEngine,
    request_json: String,
}

impl Task for ExecuteTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        Ok(wire::execute_wire(&self.engine, &self.request_json))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(output)
    }
}

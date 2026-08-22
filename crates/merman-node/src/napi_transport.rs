use merman_bindings_core::{BindingEngine, OperationControl};
use napi::{
    Env, Task,
    bindgen_prelude::{AbortSignal, AsyncTask},
};
use napi_derive::napi;

use crate::wire;

#[napi(js_name = "NativeEngine")]
pub struct NativeEngine {
    engine: Option<BindingEngine>,
}

impl NativeEngine {
    fn engine(&self) -> Result<&BindingEngine, napi::Error> {
        self.engine
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason(wire::error_envelope(&wire::disposed_error())))
    }
}

#[napi]
impl NativeEngine {
    #[napi(constructor)]
    pub fn new(options_json: String) -> napi::Result<Self> {
        wire::create_engine(&options_json)
            .map(|engine| Self {
                engine: Some(engine),
            })
            .map_err(|error| napi::Error::from_reason(wire::error_envelope(&error)))
    }

    #[napi]
    pub fn execute(
        &self,
        request_json: String,
        signal: Option<AbortSignal>,
        timeout_ms: Option<u32>,
    ) -> napi::Result<AsyncTask<ExecuteTask>> {
        let engine = self.engine()?.clone();
        let control = wire::admitted_operation_control(timeout_ms);
        if let Some(signal) = signal.as_ref() {
            let cancellation = control.clone();
            signal.on_abort(move || cancellation.cancel());
        }
        // Do not attach the signal to AsyncTask itself. N-API may cancel queued async work before
        // compute(), which would bypass the canonical BindingError cancellation envelope.
        Ok(AsyncTask::new(ExecuteTask {
            admitted_timeout_ms: timeout_ms,
            control,
            engine,
            request_json,
        }))
    }

    #[napi(js_name = "runtimeCatalogJson")]
    pub fn runtime_catalog_json(&self) -> napi::Result<String> {
        self.engine()?;
        wire::runtime_catalog_wire()
            .map_err(|error| napi::Error::from_reason(wire::error_envelope(&error)))
    }

    #[napi(js_name = "metadataJson")]
    pub fn metadata_json(&self, id: String) -> napi::Result<String> {
        self.engine()?;
        wire::metadata_wire(&id)
            .map_err(|error| napi::Error::from_reason(wire::error_envelope(&error)))
    }

    #[napi(js_name = "executeSync")]
    pub fn execute_sync(
        &self,
        request_json: String,
        timeout_ms: Option<u32>,
    ) -> napi::Result<String> {
        Ok(wire::execute_wire_with_admitted_control(
            self.engine()?,
            &request_json,
            wire::admitted_operation_control(timeout_ms),
            timeout_ms,
        ))
    }

    #[napi]
    pub fn dispose(&mut self) {
        self.engine.take();
    }
}

pub struct ExecuteTask {
    admitted_timeout_ms: Option<u32>,
    control: OperationControl,
    engine: BindingEngine,
    request_json: String,
}

impl Task for ExecuteTask {
    type Output = String;
    type JsValue = String;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        Ok(wire::execute_wire_with_admitted_control(
            &self.engine,
            &self.request_json,
            self.control.clone(),
            self.admitted_timeout_ms,
        ))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(output)
    }
}

#[napi(js_name = "transportIdentityJson")]
#[allow(dead_code)] // The N-API registration consumes this private-module export at load time.
pub fn transport_identity_json() -> napi::Result<String> {
    wire::transport_identity_wire(wire::NodeTransportKind::Napi)
        .map_err(|error| napi::Error::from_reason(wire::error_envelope(&error)))
}

#[cfg(test)]
mod tests {
    use super::NativeEngine;

    #[test]
    fn dispose_is_idempotent_and_removes_the_engine() {
        let mut engine = NativeEngine::new(String::new()).expect("construct NativeEngine");
        engine.dispose();
        engine.dispose();
        assert!(engine.engine.is_none());
        let error = engine.engine().err().expect("disposed engine must fail");
        let payload: serde_json::Value =
            serde_json::from_str(&error.reason).expect("disposed error envelope");
        assert_eq!(payload["error"]["code_name"], "MERMAN_INVALID_ARGUMENT");
        assert_eq!(
            payload["error"]["message"],
            "Node transport engine has been disposed"
        );
    }
}

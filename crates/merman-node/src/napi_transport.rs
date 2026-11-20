use merman_bindings_core::{BindingEngine, OperationControl};
use napi::{
    Env, Task,
    bindgen_prelude::{AbortSignal, Either, PromiseRaw, ToNapiValue, TypeName},
};
use napi_derive::napi;
use std::sync::Arc;

use crate::wire;

#[napi(js_name = "NativeEngine")]
pub struct NativeEngine {
    engine: Option<Arc<BindingEngine>>,
}

#[napi(object, object_from_js = false)]
pub struct SvgOperationFailure {
    #[napi(js_name = "errorJson")]
    pub error_json: String,
}

type AdmittedOperation<T> = fn(&BindingEngine, &str, OperationControl, Option<u32>) -> T;

impl NativeEngine {
    fn engine(&self) -> Result<&Arc<BindingEngine>, napi::Error> {
        self.engine
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason(wire::error_envelope(&wire::disposed_error())))
    }

    fn admit_task<T>(
        &self,
        request_json: String,
        signal: Option<AbortSignal>,
        timeout_ms: Option<u32>,
        execute: AdmittedOperation<T>,
    ) -> napi::Result<ExecuteTask<T>> {
        let engine = Arc::clone(self.engine()?);
        let control = wire::admitted_operation_control(timeout_ms);
        // Use cooperative cancellation so queued libuv work still computes the canonical error.
        if let Some(signal) = signal.as_ref() {
            let cancellation = control.clone();
            signal.on_abort(move || cancellation.cancel());
        }
        Ok(ExecuteTask {
            admitted_timeout_ms: timeout_ms,
            control,
            engine,
            request_json,
            execute,
        })
    }
}

#[napi]
impl NativeEngine {
    #[napi(constructor)]
    pub fn new(options_json: String, icon_packs_json: Option<String>) -> napi::Result<Self> {
        wire::create_engine_with_icon_packs(&options_json, icon_packs_json.as_deref())
            .map(|engine| Self {
                engine: Some(Arc::new(engine)),
            })
            .map_err(|error| napi::Error::from_reason(wire::error_envelope(&error)))
    }

    #[napi]
    pub fn execute(
        &self,
        env: Env,
        request_json: String,
        signal: Option<AbortSignal>,
        timeout_ms: Option<u32>,
    ) -> napi::Result<PromiseRaw<'_, String>> {
        let task = self.admit_task(
            request_json,
            signal,
            timeout_ms,
            wire::execute_wire_with_admitted_control,
        )?;
        Ok(env.spawn(task)?.promise_object())
    }

    #[napi(js_name = "executeSvg")]
    pub fn execute_svg(
        &self,
        env: Env,
        request_json: String,
        signal: Option<AbortSignal>,
        timeout_ms: Option<u32>,
    ) -> napi::Result<PromiseRaw<'_, Either<String, SvgOperationFailure>>> {
        let task = self.admit_task(request_json, signal, timeout_ms, execute_direct_svg)?;
        Ok(env.spawn(task)?.promise_object())
    }

    #[napi(js_name = "runtimeCatalogJson")]
    pub fn runtime_catalog_json(&self) -> napi::Result<String> {
        self.engine()?;
        wire::runtime_catalog_wire()
            .map_err(|error| napi::Error::from_reason(wire::error_envelope(&error)))
    }

    #[napi(js_name = "metadataJson")]
    pub fn metadata_json(&self, id: String) -> napi::Result<String> {
        wire::metadata_wire_for_engine(self.engine()?.as_ref(), &id)
            .map_err(|error| napi::Error::from_reason(wire::error_envelope(&error)))
    }

    #[napi(js_name = "executeSync")]
    pub fn execute_sync(
        &self,
        request_json: String,
        timeout_ms: Option<u32>,
    ) -> napi::Result<String> {
        Ok(wire::execute_wire_with_admitted_control(
            self.engine()?.as_ref(),
            &request_json,
            wire::admitted_operation_control(timeout_ms),
            timeout_ms,
        ))
    }

    #[napi(js_name = "executeSvgSync")]
    pub fn execute_svg_sync(
        &self,
        request_json: String,
        timeout_ms: Option<u32>,
    ) -> napi::Result<Either<String, SvgOperationFailure>> {
        Ok(execute_direct_svg(
            self.engine()?.as_ref(),
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

struct ExecuteTask<T> {
    admitted_timeout_ms: Option<u32>,
    control: OperationControl,
    engine: Arc<BindingEngine>,
    request_json: String,
    execute: AdmittedOperation<T>,
}

impl<T: ToNapiValue + TypeName + Send + 'static> Task for ExecuteTask<T> {
    type Output = T;
    type JsValue = T;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        Ok((self.execute)(
            self.engine.as_ref(),
            &self.request_json,
            self.control.clone(),
            self.admitted_timeout_ms,
        ))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(output)
    }
}

fn execute_direct_svg(
    engine: &BindingEngine,
    request_json: &str,
    control: OperationControl,
    admitted_timeout_ms: Option<u32>,
) -> Either<String, SvgOperationFailure> {
    match wire::execute_svg_with_admitted_control(
        engine,
        request_json,
        control,
        admitted_timeout_ms,
    ) {
        Ok(svg) => Either::A(svg),
        Err(error) => Either::B(SvgOperationFailure {
            error_json: wire::error_envelope(&error),
        }),
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
    use super::{NativeEngine, wire::execute_wire_with_admitted_control};
    #[cfg(feature = "diagram-flowchart")]
    use napi::Task;
    use std::sync::Arc;

    #[test]
    fn async_tasks_share_the_retained_engine_until_their_final_release() {
        let mut engine = NativeEngine::new(String::new(), None).expect("construct NativeEngine");
        let retained = Arc::downgrade(engine.engine().unwrap());
        assert_eq!(retained.strong_count(), 1);
        let request =
            r#"{"operation_id":"semantic-json","source":"flowchart TD\nA --> B","uri":null}"#;
        let admit = || {
            engine.admit_task(
                request.to_owned(),
                None,
                None,
                execute_wire_with_admitted_control,
            )
        };
        let first = admit().unwrap();
        let second = admit().unwrap();
        assert_eq!(retained.strong_count(), 3);
        assert!(Arc::ptr_eq(&first.engine, engine.engine().unwrap()));
        assert!(Arc::ptr_eq(&second.engine, &first.engine));

        engine.dispose();
        assert!(
            engine
                .admit_task(
                    request.to_owned(),
                    None,
                    None,
                    execute_wire_with_admitted_control
                )
                .is_err()
        );
        assert_eq!(retained.strong_count(), 2);
        drop(first);
        assert_eq!(retained.strong_count(), 1);
        drop(second);
        assert!(retained.upgrade().is_none());
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn accepted_tasks_compute_after_dispose_with_independent_controls() {
        let mut engine = NativeEngine::new(String::new(), None).expect("construct NativeEngine");
        let request =
            r#"{"operation_id":"semantic-json","source":"flowchart TD\nA --> B","uri":null}"#;
        let admit = || {
            engine.admit_task(
                request.to_owned(),
                None,
                None,
                execute_wire_with_admitted_control,
            )
        };
        let mut cancelled = admit().unwrap();
        let mut accepted = admit().unwrap();
        assert!(Arc::ptr_eq(&cancelled.engine, &accepted.engine));
        cancelled.control.cancel();
        engine.dispose();

        let payload: serde_json::Value =
            serde_json::from_str(&cancelled.compute().unwrap()).unwrap();
        assert_eq!(payload["error"]["code_name"], "MERMAN_CANCELLED");
        assert_eq!(
            payload["error"]["details"]["cancellation"]["reason"],
            "requested",
        );
        let payload: serde_json::Value =
            serde_json::from_str(&accepted.compute().unwrap()).unwrap();
        assert_eq!(payload["ok"], true, "{payload}");
    }

    #[test]
    fn dispose_is_idempotent_and_removes_the_engine() {
        let mut engine = NativeEngine::new(String::new(), None).expect("construct NativeEngine");
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

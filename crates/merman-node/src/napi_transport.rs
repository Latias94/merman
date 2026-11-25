use merman_bindings_core::{BindingEngine, BindingError, OperationControl};
use napi::{
    Env, JsString, Task,
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

impl NativeEngine {
    fn engine(&self) -> Result<&Arc<BindingEngine>, napi::Error> {
        self.engine
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason(wire::error_envelope(&wire::disposed_error())))
    }

    fn admit_task<F>(
        &self,
        signal: Option<AbortSignal>,
        timeout_ms: Option<u32>,
        execute: F,
    ) -> napi::Result<ExecuteTask<F>> {
        let engine = Arc::clone(self.engine()?);
        let control = wire::admitted_operation_control(timeout_ms);
        // Use cooperative cancellation so queued libuv work still computes the canonical error.
        if let Some(signal) = signal.as_ref() {
            let cancellation = control.clone();
            signal.on_abort(move || cancellation.cancel());
        }
        Ok(ExecuteTask {
            control,
            engine,
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
            signal,
            timeout_ms,
            move |engine: &BindingEngine, control| {
                wire::execute_wire_with_admitted_control(engine, &request_json, control, timeout_ms)
            },
        )?;
        Ok(env.spawn(task)?.promise_object())
    }

    #[napi(js_name = "executeSvg")]
    pub fn execute_svg(
        &self,
        env: Env,
        source: JsString<'_>,
        options_json: Option<JsString<'_>>,
        signal: Option<AbortSignal>,
        timeout_ms: Option<u32>,
    ) -> napi::Result<PromiseRaw<'_, Either<String, SvgOperationFailure>>> {
        self.engine()?;
        let (source, options_json) = read_svg_input(source, options_json)?;
        let task = self.admit_task(
            signal,
            timeout_ms,
            move |engine: &BindingEngine, control| {
                execute_direct_svg(
                    engine,
                    &source,
                    wire::DirectSvgOptions::Json(options_json.as_deref()),
                    control,
                    timeout_ms,
                )
            },
        )?;
        Ok(env.spawn(task)?.promise_object())
    }

    #[napi(js_name = "executeSvgWithId")]
    pub fn execute_svg_with_id(
        &self,
        env: Env,
        source: JsString<'_>,
        diagram_id: JsString<'_>,
        signal: Option<AbortSignal>,
        timeout_ms: Option<u32>,
    ) -> napi::Result<PromiseRaw<'_, Either<String, SvgOperationFailure>>> {
        self.engine()?;
        let (source, diagram_id) = read_svg_id_input(source, diagram_id)?;
        let task = self.admit_task(
            signal,
            timeout_ms,
            move |engine: &BindingEngine, control| {
                execute_direct_svg(
                    engine,
                    &source,
                    wire::DirectSvgOptions::DiagramId(&diagram_id),
                    control,
                    timeout_ms,
                )
            },
        )?;
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
        source: JsString<'_>,
        options_json: Option<JsString<'_>>,
        timeout_ms: Option<u32>,
    ) -> napi::Result<Either<String, SvgOperationFailure>> {
        let engine = self.engine()?;
        let (source, options_json) = read_svg_input(source, options_json)?;
        Ok(execute_direct_svg(
            engine.as_ref(),
            &source,
            wire::DirectSvgOptions::Json(options_json.as_deref()),
            wire::admitted_operation_control(timeout_ms),
            timeout_ms,
        ))
    }

    #[napi(js_name = "executeSvgWithIdSync")]
    pub fn execute_svg_with_id_sync(
        &self,
        source: JsString<'_>,
        diagram_id: JsString<'_>,
        timeout_ms: Option<u32>,
    ) -> napi::Result<Either<String, SvgOperationFailure>> {
        let engine = self.engine()?;
        let (source, diagram_id) = read_svg_id_input(source, diagram_id)?;
        Ok(execute_direct_svg(
            engine.as_ref(),
            &source,
            wire::DirectSvgOptions::DiagramId(&diagram_id),
            wire::admitted_operation_control(timeout_ms),
            timeout_ms,
        ))
    }

    #[napi]
    pub fn dispose(&mut self) {
        self.engine.take();
    }
}

struct ExecuteTask<F> {
    control: OperationControl,
    engine: Arc<BindingEngine>,
    execute: F,
}

impl<F, T> Task for ExecuteTask<F>
where
    F: Fn(&BindingEngine, OperationControl) -> T + Send + 'static,
    T: ToNapiValue + TypeName + Send + 'static,
{
    type Output = T;
    type JsValue = T;

    fn compute(&mut self) -> napi::Result<Self::Output> {
        Ok((self.execute)(self.engine.as_ref(), self.control.clone()))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> napi::Result<Self::JsValue> {
        Ok(output)
    }
}

fn read_svg_input(
    source: JsString<'_>,
    options_json: Option<JsString<'_>>,
) -> napi::Result<(String, Option<String>)> {
    let (source_limit, options_limit) = wire::direct_svg_field_limits();
    let source = read_svg_text(source, "operation request source", source_limit)?;
    let options_json = options_json
        .map(|value| read_svg_text(value, "operation request options_json", options_limit))
        .transpose()?;
    Ok((source, options_json))
}

fn read_svg_id_input(
    source: JsString<'_>,
    diagram_id: JsString<'_>,
) -> napi::Result<(String, String)> {
    let (source_limit, options_limit) = wire::direct_svg_field_limits();
    let source = read_svg_text(source, "operation request source", source_limit)?;
    let diagram_id = read_svg_text(diagram_id, "operation request diagram_id", options_limit)?;
    Ok((source, diagram_id))
}

fn read_svg_text(value: JsString<'_>, field: &str, limit: usize) -> napi::Result<String> {
    // Each UTF-16 code unit contributes at most three UTF-8 bytes, even for a replacement
    // scalar. When the unit count is inconclusive, check the exact byte size before copying.
    if value.utf16_len()? > limit / 3 {
        wire::ensure_field_bytes(value.utf8_len()?, field, limit)
            .map_err(wire::caller_options_error)
            .map_err(napi_binding_error)?;
    }
    // UTF-8 conversion in N-API can replace lone UTF-16 surrogates. Decode those units strictly,
    // including for callers invoking the private addon without going through the facade.
    value.into_utf16()?.as_str().map_err(|_| {
        napi_binding_error(wire::caller_options_error(format!(
            "{field} contains an isolated UTF-16 surrogate"
        )))
    })
}

fn napi_binding_error(error: BindingError) -> napi::Error {
    napi::Error::from_reason(wire::error_envelope(&error))
}

fn execute_direct_svg(
    engine: &BindingEngine,
    source: &str,
    options: wire::DirectSvgOptions<'_>,
    control: OperationControl,
    admitted_timeout_ms: Option<u32>,
) -> Either<String, SvgOperationFailure> {
    match wire::execute_svg_with_admitted_control(
        engine,
        source,
        options,
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
    use super::{BindingEngine, NativeEngine, wire::execute_wire_with_admitted_control};
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
        let execute = |engine: &BindingEngine, control| {
            execute_wire_with_admitted_control(engine, request, control, None)
        };
        let admit = || engine.admit_task(None, None, execute);
        let first = admit().unwrap();
        let second = admit().unwrap();
        assert_eq!(retained.strong_count(), 3);
        assert!(Arc::ptr_eq(&first.engine, engine.engine().unwrap()));
        assert!(Arc::ptr_eq(&second.engine, &first.engine));

        engine.dispose();
        assert!(engine.admit_task(None, None, execute).is_err());
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
            engine.admit_task(None, None, |engine: &BindingEngine, control| {
                execute_wire_with_admitted_control(engine, request, control, None)
            })
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

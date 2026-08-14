#![cfg(target_os = "android")]
#![deny(unsafe_op_in_unsafe_fn)]

//! Android's direct bindings-core transport.
//!
//! JNI is intentionally not layered over the C ABI. `JNI_OnLoad` registers this module's small,
//! typed method set, and every diagram operation goes through `BindingEngine::execute` with the
//! generated operation vocabulary.

mod artifact_contract;
mod error;
mod operation_control;
mod token;

use artifact_contract::android_artifact_contract;
use error::binding_error_text;
#[cfg(feature = "svg")]
use jni::objects::Global;
use jni::{
    Env, JavaVM, NativeMethod,
    errors::{Error as JniError, Result as JniResult},
    objects::{JClass, JObject, JObjectArray, JString, JValue},
    strings::JNIString,
    sys::{JNI_ERR, JNI_FALSE, JNI_TRUE, JNI_VERSION_1_6, jboolean, jint, jlong},
};
use merman_bindings_core::{
    BindingEngine, BindingEngineAdmission, BindingEngineAdmissionMode, BindingEngineServices,
    BindingError, BindingOperationRequest, BindingOperationResult, OperationControl,
    OperationPhase,
};
#[cfg(feature = "svg")]
use merman_bindings_core::{BindingIconRegistry, BindingStatus, IconPack, build_icon_registry};
use operation_control::JniOperationControlRegistry;
#[cfg(feature = "svg")]
use std::cell::Cell;
use std::{
    collections::BTreeMap,
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex, OnceLock},
};
use token::next_monotonic_jni_token;

const ANDROID_TRANSPORT_API_VERSION: u32 = 2;

struct JniEngineState {
    engine: Mutex<Option<Arc<BindingEngine>>>,
    admission: Arc<BindingEngineAdmission>,
}

impl JniEngineState {
    fn acquire_engine(&self) -> Option<Arc<BindingEngine>> {
        self.engine
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .map(Arc::clone)
    }

    fn detach_engine(&self) -> Option<Arc<BindingEngine>> {
        self.engine
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

#[derive(Default)]
struct JniEngineRegistry {
    last_token: u64,
    states: BTreeMap<u64, Arc<JniEngineState>>,
}

impl JniEngineRegistry {
    fn register(
        &mut self,
        state: Arc<JniEngineState>,
    ) -> Result<jlong, (Box<BindingError>, Arc<JniEngineState>)> {
        let token = match next_monotonic_jni_token(
            self.last_token,
            "Android engine token space is exhausted",
        ) {
            Ok(token) => token,
            Err(error) => return Err((Box::new(error), state)),
        };
        self.last_token = token;
        let previous = self.states.insert(token, state);
        debug_assert!(previous.is_none(), "Android engine tokens are never reused");
        Ok(token as jlong)
    }

    fn acquire(&self, token: u64) -> Option<Arc<JniEngineState>> {
        self.states.get(&token).map(Arc::clone)
    }

    fn retire(&mut self, token: u64) -> Option<Arc<JniEngineState>> {
        self.states.remove(&token)
    }
}

static ENGINE_REGISTRY: OnceLock<Mutex<JniEngineRegistry>> = OnceLock::new();
static OPERATION_CONTROL_REGISTRY: OnceLock<Mutex<JniOperationControlRegistry>> = OnceLock::new();

#[cfg(feature = "svg")]
struct JniHostTextMeasurer {
    vm: JavaVM,
    callback: Global<JObject<'static>>,
    admission: Arc<BindingEngineAdmission>,
}

#[cfg(feature = "svg")]
impl JniHostTextMeasurer {
    const DEFAULT_FONT_STYLE: &'static str = "normal";
    const DEFAULT_FONT_WEIGHT: &'static str = "normal";

    fn new(
        vm: JavaVM,
        callback: Global<JObject<'static>>,
        admission: Arc<BindingEngineAdmission>,
    ) -> Self {
        Self {
            vm,
            callback,
            admission,
        }
    }

    fn call_host(
        &self,
        request: merman_bindings_core::HostTextMeasurementRequest<'_>,
    ) -> merman_bindings_core::HostMeasurementResult {
        let _callback = self.admission.enter_callback().map_err(|error| {
            merman_bindings_core::HostTextMeasurementError::new(error.to_string())
        })?;
        let callback_failed = Cell::new(false);
        let result = self
            .vm
            .attach_current_thread(
                |env| -> JniResult<Option<merman_bindings_core::HostTextMeasurementRecord>> {
                    let request_object = new_text_measure_request(env, request)?;
                    let result = env
                        .call_method(
                            self.callback.as_obj(),
                            jni::jni_str!("measure"),
                            jni::jni_sig!(
                                (request: io.merman.MermanTextMeasureRequest) -> io.merman.MermanTextMeasureResult
                            ),
                            &[JValue::Object(&request_object)],
                        )
                        .and_then(|value| value.l());
                    let Some(result) =
                        recover_host_callback_result(env, result, &callback_failed)?
                    else {
                        return Ok(None);
                    };
                    if result.is_null() {
                        return Ok(None);
                    }

                    let result: JObject<'_> = result;
                    let result_kind = read_callback_field_i32(
                        env,
                        &result,
                        "resultKind",
                        &callback_failed,
                    )?;
                    let Some(result_kind) = result_kind else {
                        return Ok(None);
                    };
                    let width = read_callback_field_f64(env, &result, "width", &callback_failed)?;
                    let Some(width) = width else {
                        return Ok(None);
                    };
                    let height =
                        read_callback_field_f64(env, &result, "height", &callback_failed)?;
                    let Some(height) = height else {
                        return Ok(None);
                    };
                    let length =
                        read_callback_field_f64(env, &result, "length", &callback_failed)?;
                    let Some(length) = length else {
                        return Ok(None);
                    };
                    let line_count =
                        read_callback_field_i64(env, &result, "lineCount", &callback_failed)?;
                    let Some(line_count) = line_count else {
                        return Ok(None);
                    };
                    let bbox_left =
                        read_callback_field_f64(env, &result, "bboxLeft", &callback_failed)?;
                    let Some(bbox_left) = bbox_left else {
                        return Ok(None);
                    };
                    let bbox_right =
                        read_callback_field_f64(env, &result, "bboxRight", &callback_failed)?;
                    let Some(bbox_right) = bbox_right else {
                        return Ok(None);
                    };
                    let raw_width =
                        read_callback_field_f64(env, &result, "rawWidth", &callback_failed)?;
                    let Some(raw_width) = raw_width else {
                        return Ok(None);
                    };
                    let has_raw_width = read_callback_field_bool(
                        env,
                        &result,
                        "hasRawWidth",
                        &callback_failed,
                    )?;
                    let Some(has_raw_width) = has_raw_width else {
                        return Ok(None);
                    };

                    Ok(Some(merman_bindings_core::HostTextMeasurementRecord {
                        result_kind:
                            merman_bindings_core::HostTextMeasurementResultKind::from_external_code(
                                result_kind,
                            ),
                        width: Some(width),
                        height: Some(height),
                        line_count: Some(i128::from(line_count)),
                        length: Some(length),
                        bbox_left: Some(bbox_left),
                        bbox_right: Some(bbox_right),
                        raw_width: has_raw_width.then_some(raw_width),
                    }))
                },
            )
            .map_err(|error| {
                merman_bindings_core::HostTextMeasurementError::new(format!(
                    "JNI host text measurer failed: {error}"
                ))
            })?;
        if callback_failed.get() {
            return Err(merman_bindings_core::HostTextMeasurementError::new(
                "JNI host text measurer callback failed or returned invalid metrics",
            ));
        }
        result
            .map(|record| merman_bindings_core::decode_host_text_measurement(request, record))
            .transpose()
    }
}

#[cfg(feature = "svg")]
impl merman_bindings_core::HostTextMeasurer for JniHostTextMeasurer {
    fn measure(
        &self,
        request: merman_bindings_core::HostTextMeasurementRequest<'_>,
    ) -> merman_bindings_core::HostMeasurementResult {
        self.call_host(request)
    }
}

#[cfg(feature = "svg")]
fn recover_host_callback_result<T>(
    env: &mut Env<'_>,
    result: JniResult<T>,
    callback_failed: &Cell<bool>,
) -> JniResult<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if is_pending_host_exception(env, &error) => {
            callback_failed.set(true);
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

#[cfg(feature = "svg")]
fn is_pending_host_exception(env: &mut Env<'_>, error: &JniError) -> bool {
    let is_java_exception = matches!(error, JniError::JavaException);
    if env.exception_check() {
        env.exception_clear();
        return true;
    }
    is_java_exception
}

#[cfg(feature = "svg")]
fn read_callback_field_i32(
    env: &mut Env<'_>,
    object: &JObject<'_>,
    field: &str,
    callback_failed: &Cell<bool>,
) -> JniResult<Option<jint>> {
    let value = env
        .get_field(object, JNIString::new(field), jni::jni_sig!(jint))
        .and_then(|value| value.i());
    recover_host_callback_result(env, value, callback_failed)
}

#[cfg(feature = "svg")]
fn read_callback_field_i64(
    env: &mut Env<'_>,
    object: &JObject<'_>,
    field: &str,
    callback_failed: &Cell<bool>,
) -> JniResult<Option<jlong>> {
    let value = env
        .get_field(object, JNIString::new(field), jni::jni_sig!(jlong))
        .and_then(|value| value.j());
    recover_host_callback_result(env, value, callback_failed)
}

#[cfg(feature = "svg")]
fn read_callback_field_f64(
    env: &mut Env<'_>,
    object: &JObject<'_>,
    field: &str,
    callback_failed: &Cell<bool>,
) -> JniResult<Option<f64>> {
    let value = env
        .get_field(object, JNIString::new(field), jni::jni_sig!(f64))
        .and_then(|value| value.d());
    recover_host_callback_result(env, value, callback_failed)
}

#[cfg(feature = "svg")]
fn read_callback_field_bool(
    env: &mut Env<'_>,
    object: &JObject<'_>,
    field: &str,
    callback_failed: &Cell<bool>,
) -> JniResult<Option<bool>> {
    let value = env
        .get_field(object, JNIString::new(field), jni::jni_sig!("Z"))
        .and_then(|value| value.z());
    recover_host_callback_result(env, value, callback_failed)
}

/// Registers Merman's native method table when the JVM loads this library.
///
/// # Safety
///
/// `vm` must be a valid, live `JavaVM` pointer supplied by the JVM for this load event.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn JNI_OnLoad(
    vm: *mut jni::sys::JavaVM,
    _reserved: *mut c_void,
) -> jint {
    if vm.is_null() {
        return JNI_ERR;
    }
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let vm = unsafe { JavaVM::from_raw(vm) };
        vm.with_top_local_frame(register_native_methods)
    }));
    match outcome {
        Ok(Ok(())) => JNI_VERSION_1_6,
        Ok(Err(_)) | Err(_) => JNI_ERR,
    }
}

const MERMAN_METHODS: &[NativeMethod] = &[
    jni::native_method! {
        static fn native_runtime_catalog_json() -> java.lang.String,
    },
    jni::native_method! {
        static fn native_execute(
            operation_id: java.lang.String,
            source: java.lang.String,
            options_json: java.lang.String,
            uri: java.lang.String,
        ) -> io.merman.MermanOperationResult,
    },
    jni::native_method! {
        static fn native_execute_controlled(
            operation_id: java.lang.String,
            source: java.lang.String,
            options_json: java.lang.String,
            uri: java.lang.String,
            control_token: jlong,
        ) -> io.merman.MermanOperationResult,
        name = "nativeExecuteControlled",
    },
    jni::native_method! {
        static fn native_metadata_json(id: java.lang.String) -> java.lang.String,
    },
];

const OPERATION_CONTROL_METHODS: &[NativeMethod] = &[
    jni::native_method! {
        static fn native_operation_control_new(
            timeout_ms: jlong,
            has_timeout_ms: jboolean,
        ) -> jlong,
        name = "nativeNew",
    },
    jni::native_method! {
        static fn native_operation_control_cancel(token: jlong) -> void,
        name = "nativeCancel",
    },
    jni::native_method! {
        static fn native_operation_control_is_cancelled(token: jlong) -> jboolean,
        name = "nativeIsCancelled",
    },
    jni::native_method! {
        static fn native_operation_control_release(token: jlong) -> void,
        name = "nativeRelease",
    },
];

const ENGINE_METHODS: &[NativeMethod] = &[
    jni::native_method! {
        static fn native_engine_new(
            options_json: java.lang.String,
            icon_pack_json: [java.lang.String],
            icon_pack_registration_names: [java.lang.String],
            measurer: io.merman.MermanTextMeasurer,
        ) -> jlong,
        name = "nativeNew",
    },
    jni::native_method! {
        static fn native_engine_try_close(handle: jlong) -> jboolean,
        name = "nativeTryClose",
    },
    jni::native_method! {
        static fn native_engine_execute(
            handle: jlong,
            operation_id: java.lang.String,
            source: java.lang.String,
            options_json: java.lang.String,
            uri: java.lang.String,
        ) -> io.merman.MermanOperationResult,
        name = "nativeExecute",
    },
    jni::native_method! {
        static fn native_engine_execute_controlled(
            handle: jlong,
            operation_id: java.lang.String,
            source: java.lang.String,
            options_json: java.lang.String,
            uri: java.lang.String,
            control_token: jlong,
        ) -> io.merman.MermanOperationResult,
        name = "nativeExecuteControlled",
    },
];

fn register_native_methods(env: &mut Env<'_>) -> JniResult<()> {
    let merman_class = env.find_class(jni::jni_str!("io/merman/Merman"))?;
    // Safety: `jni::native_method!` generates an ABI-checked wrapper for every descriptor.
    unsafe { env.register_native_methods(merman_class, MERMAN_METHODS)? };

    let operation_control_class =
        env.find_class(jni::jni_str!("io/merman/MermanOperationControl"))?;
    // Safety: `jni::native_method!` generates an ABI-checked wrapper for every descriptor.
    unsafe {
        env.register_native_methods(operation_control_class, OPERATION_CONTROL_METHODS)?;
    }

    let engine_class = env.find_class(jni::jni_str!("io/merman/MermanEngine"))?;
    // Safety: `jni::native_method!` generates an ABI-checked wrapper for every descriptor.
    unsafe { env.register_native_methods(engine_class, ENGINE_METHODS) }
}

fn native_runtime_catalog_json<'local>(
    env: &mut Env<'local>,
    _class: JClass<'local>,
) -> JniResult<JString<'local>> {
    result_to_java_string(
        env,
        android_artifact_contract().runtime_catalog_json(ANDROID_TRANSPORT_API_VERSION),
    )
}

fn native_execute<'local>(
    env: &mut Env<'local>,
    _class: JClass<'local>,
    operation_id: JString<'local>,
    source: JString<'local>,
    options_json: JString<'local>,
    uri: JString<'local>,
) -> JniResult<JObject<'local>> {
    native_execute_impl(env, operation_id, source, options_json, uri, None)
}

fn native_execute_controlled<'local>(
    env: &mut Env<'local>,
    _class: JClass<'local>,
    operation_id: JString<'local>,
    source: JString<'local>,
    options_json: JString<'local>,
    uri: JString<'local>,
    control_token: jlong,
) -> JniResult<JObject<'local>> {
    let Some(operation_control) = preflight_operation_control(env, control_token) else {
        return Ok(JObject::null());
    };
    native_execute_impl(
        env,
        operation_id,
        source,
        options_json,
        uri,
        Some(operation_control),
    )
}

fn native_execute_impl<'local>(
    env: &mut Env<'local>,
    operation_id: JString<'local>,
    source: JString<'local>,
    options_json: JString<'local>,
    uri: JString<'local>,
    operation_control: Option<OperationControl>,
) -> JniResult<JObject<'local>> {
    let Some(operation_id) = required_java_string(env, operation_id, "operationId") else {
        return Ok(JObject::null());
    };
    let Some(source) = required_java_string(env, source, "source") else {
        return Ok(JObject::null());
    };
    let Some(options_json) = optional_java_string(env, options_json, "optionsJson") else {
        return Ok(JObject::null());
    };
    let Some(uri) = nullable_java_string(env, uri, "uri") else {
        return Ok(JObject::null());
    };
    let request = binding_operation_request(
        &operation_id,
        &source,
        &options_json,
        uri.as_deref(),
        operation_control,
    );
    result_to_java_operation_result(env, android_artifact_contract().execute_once(request))
}

fn native_operation_control_new<'local>(
    env: &mut Env<'local>,
    _class: JClass<'local>,
    timeout_ms: jlong,
    has_timeout_ms: jboolean,
) -> JniResult<jlong> {
    let result = operation_control_timeout_ms(timeout_ms, has_timeout_ms).and_then(|timeout_ms| {
        operation_control_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .issue(timeout_ms)
    });
    match result {
        Ok(token) => Ok(token as jlong),
        Err(error) => {
            throw_merman_exception(env, binding_error_text(error));
            Ok(0)
        }
    }
}

fn native_operation_control_cancel<'local>(
    env: &mut Env<'local>,
    _class: JClass<'local>,
    token: jlong,
) -> JniResult<()> {
    match acquire_operation_control(token) {
        Ok(control) => control.cancel(),
        Err(error) => throw_merman_exception(env, binding_error_text(error)),
    }
    Ok(())
}

fn native_operation_control_is_cancelled<'local>(
    env: &mut Env<'local>,
    _class: JClass<'local>,
    token: jlong,
) -> JniResult<jboolean> {
    match acquire_operation_control(token) {
        Ok(control) => Ok(if control.is_cancelled() {
            JNI_TRUE
        } else {
            JNI_FALSE
        }),
        Err(error) => {
            throw_merman_exception(env, binding_error_text(error));
            Ok(JNI_FALSE)
        }
    }
}

fn native_operation_control_release<'local>(
    env: &mut Env<'local>,
    _class: JClass<'local>,
    token: jlong,
) -> JniResult<()> {
    let result = operation_control_token(token).and_then(|token| {
        operation_control_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .release(token)
    });
    if let Err(error) = result {
        throw_merman_exception(env, binding_error_text(error));
    }
    Ok(())
}

fn native_metadata_json<'local>(
    env: &mut Env<'local>,
    _class: JClass<'local>,
    id: JString<'local>,
) -> JniResult<JString<'local>> {
    let Some(id) = required_java_string(env, id, "metadataId") else {
        return Ok(JString::null());
    };
    result_to_java_string(env, android_artifact_contract().metadata_json(&id))
}

fn native_engine_new<'local>(
    env: &mut Env<'local>,
    _class: JClass<'local>,
    options_json: JString<'local>,
    icon_pack_json: JObjectArray<'local, JString<'local>>,
    icon_pack_registration_names: JObjectArray<'local, JString<'local>>,
    measurer: JObject<'local>,
) -> JniResult<jlong> {
    let Some(options_json) = optional_java_string(env, options_json, "optionsJson") else {
        return Ok(0);
    };
    let admission = BindingEngineAdmission::new(if measurer.is_null() {
        BindingEngineAdmissionMode::Concurrent
    } else {
        BindingEngineAdmissionMode::HostCallback
    });

    let result = (|| {
        #[cfg(feature = "svg")]
        let mut services = BindingEngineServices::new();
        #[cfg(feature = "svg")]
        if let Some(registry) =
            build_jni_icon_registry(env, &icon_pack_json, &icon_pack_registration_names)?
        {
            services = services.with_icon_registry(registry);
        }
        #[cfg(feature = "svg")]
        if !measurer.is_null() {
            let callback = env.new_global_ref(&measurer).map_err(jni_binding_error)?;
            let vm = env.get_java_vm().map_err(jni_binding_error)?;
            services = services.with_host_text_measurer(Arc::new(JniHostTextMeasurer::new(
                vm,
                callback,
                Arc::clone(&admission),
            )));
        }

        #[cfg(not(feature = "svg"))]
        let services = {
            if icon_pack_array_len(env, &icon_pack_json, &icon_pack_registration_names)? != 0 {
                return Err(BindingError::missing_capability(
                    "svg",
                    "icon registries require the svg capability",
                ));
            }
            if !measurer.is_null() {
                return Err(BindingError::missing_capability(
                    "svg",
                    "host text measurement requires the svg capability",
                ));
            }
            BindingEngineServices::new()
        };

        let engine = Arc::new(
            android_artifact_contract()
                .create_engine_with_services(options_json.as_bytes(), services)?,
        );
        let state = Arc::new(JniEngineState {
            engine: Mutex::new(Some(engine)),
            admission,
        });
        let publication = engine_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .register(state);
        match publication {
            Ok(handle) => Ok(handle),
            Err((error, state)) => {
                drop(state);
                Err(*error)
            }
        }
    })();
    match result {
        Ok(handle) => Ok(handle),
        Err(error) => {
            throw_merman_exception(env, binding_error_text(error));
            Ok(0)
        }
    }
}

fn native_engine_try_close<'local>(
    env: &mut Env<'local>,
    _class: JClass<'local>,
    handle: jlong,
) -> JniResult<jboolean> {
    let Some(token) = jni_token(handle) else {
        return Ok(JNI_TRUE);
    };
    let state = engine_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .acquire(token);
    let Some(state) = state else {
        return Ok(JNI_TRUE);
    };
    let close = state.admission.try_close_detaching(|| {
        let retired = engine_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retire(token);
        retired.as_ref().and_then(|state| state.detach_engine())
    });
    match close {
        Ok(()) | Err(merman_bindings_core::BindingEngineAdmissionError::Closed) => Ok(JNI_TRUE),
        Err(error) => {
            throw_merman_exception(env, binding_error_text(error.into()));
            Ok(JNI_FALSE)
        }
    }
}

fn native_engine_execute<'local>(
    env: &mut Env<'local>,
    _class: JClass<'local>,
    handle: jlong,
    operation_id: JString<'local>,
    source: JString<'local>,
    options_json: JString<'local>,
    uri: JString<'local>,
) -> JniResult<JObject<'local>> {
    native_engine_execute_impl(env, handle, operation_id, source, options_json, uri, None)
}

fn native_engine_execute_controlled<'local>(
    env: &mut Env<'local>,
    _class: JClass<'local>,
    handle: jlong,
    operation_id: JString<'local>,
    source: JString<'local>,
    options_json: JString<'local>,
    uri: JString<'local>,
    control_token: jlong,
) -> JniResult<JObject<'local>> {
    let Some(operation_control) = preflight_operation_control(env, control_token) else {
        return Ok(JObject::null());
    };
    native_engine_execute_impl(
        env,
        handle,
        operation_id,
        source,
        options_json,
        uri,
        Some(operation_control),
    )
}

fn native_engine_execute_impl<'local>(
    env: &mut Env<'local>,
    handle: jlong,
    operation_id: JString<'local>,
    source: JString<'local>,
    options_json: JString<'local>,
    uri: JString<'local>,
    operation_control: Option<OperationControl>,
) -> JniResult<JObject<'local>> {
    let Some(token) = engine_token(env, handle) else {
        return Ok(JObject::null());
    };
    let Some(operation_id) = required_java_string(env, operation_id, "operationId") else {
        return Ok(JObject::null());
    };
    let Some(source) = required_java_string(env, source, "source") else {
        return Ok(JObject::null());
    };
    let Some(options_json) = optional_java_string(env, options_json, "optionsJson") else {
        return Ok(JObject::null());
    };
    let Some(uri) = nullable_java_string(env, uri, "uri") else {
        return Ok(JObject::null());
    };
    let Some(state) = acquire_engine_state(env, token) else {
        return Ok(JObject::null());
    };
    let result = state
        .admission
        .enter_operation()
        .map_err(BindingError::from)
        .and_then(|_operation| {
            let engine = state
                .acquire_engine()
                .ok_or_else(|| BindingError::invalid_argument("Merman engine is closed"))?;
            let request = binding_operation_request(
                &operation_id,
                &source,
                &options_json,
                uri.as_deref(),
                operation_control,
            );
            engine.execute(request)
        });
    result_to_java_operation_result(env, result)
}

fn binding_operation_request<'a>(
    operation_id: &'a str,
    source: &'a str,
    options_json: &'a str,
    uri: Option<&'a str>,
    operation_control: Option<OperationControl>,
) -> BindingOperationRequest<'a> {
    let request = BindingOperationRequest::new(operation_id, source.as_bytes())
        .with_optional_uri(uri.map(str::as_bytes))
        .with_options_json(options_json.as_bytes());
    match operation_control {
        Some(operation_control) => request.with_control(operation_control),
        None => request,
    }
}

fn engine_registry() -> &'static Mutex<JniEngineRegistry> {
    ENGINE_REGISTRY.get_or_init(|| Mutex::new(JniEngineRegistry::default()))
}

fn operation_control_registry() -> &'static Mutex<JniOperationControlRegistry> {
    OPERATION_CONTROL_REGISTRY.get_or_init(|| Mutex::new(JniOperationControlRegistry::default()))
}

fn operation_control_timeout_ms(
    timeout_ms: jlong,
    has_timeout_ms: jboolean,
) -> Result<Option<u64>, BindingError> {
    match has_timeout_ms {
        JNI_FALSE => Ok(None),
        JNI_TRUE => u64::try_from(timeout_ms).map(Some).map_err(|_| {
            BindingError::invalid_argument("operation-control timeoutMs must be non-negative")
        }),
        _ => Err(BindingError::invalid_argument(
            "operation-control hasTimeoutMs must be a JNI boolean",
        )),
    }
}

fn operation_control_token(token: jlong) -> Result<u64, BindingError> {
    jni_token(token).ok_or_else(|| {
        BindingError::invalid_argument(
            "operation-control token must be a positive opaque Android token",
        )
    })
}

fn acquire_operation_control(token: jlong) -> Result<OperationControl, BindingError> {
    let token = operation_control_token(token)?;
    operation_control_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .acquire(token)
}

fn preflight_operation_control(env: &mut Env<'_>, token: jlong) -> Option<OperationControl> {
    let result = acquire_operation_control(token).and_then(|operation_control| {
        operation_control
            .checkpoint_at(OperationPhase::Admission)
            .map_err(BindingError::cancelled)?;
        Ok(operation_control)
    });
    match result {
        Ok(operation_control) => Some(operation_control),
        Err(error) => {
            throw_merman_exception(env, binding_error_text(error));
            None
        }
    }
}

fn jni_token(handle: jlong) -> Option<u64> {
    u64::try_from(handle).ok().filter(|token| *token != 0)
}

fn engine_token(env: &mut Env<'_>, handle: jlong) -> Option<u64> {
    let token = jni_token(handle);
    if token.is_none() {
        throw_merman_exception(env, "Merman engine is closed");
    }
    token
}

fn acquire_engine_state(env: &mut Env<'_>, token: u64) -> Option<Arc<JniEngineState>> {
    let state = engine_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .acquire(token);
    if state.is_none() {
        throw_merman_exception(env, "Merman engine is closed");
    }
    state
}

fn required_java_string(env: &mut Env<'_>, value: JString<'_>, name: &str) -> Option<String> {
    if value.is_null() {
        throw_merman_exception(env, format!("{name} must not be null"));
        return None;
    }
    java_string(env, value)
}

fn optional_java_string(env: &mut Env<'_>, value: JString<'_>, name: &str) -> Option<String> {
    if value.is_null() {
        return Some(String::new());
    }
    java_string(env, value).or_else(|| {
        throw_merman_exception(env, format!("{name} was not a valid Java string"));
        None
    })
}

fn nullable_java_string(
    env: &mut Env<'_>,
    value: JString<'_>,
    name: &str,
) -> Option<Option<String>> {
    if value.is_null() {
        return Some(None);
    }
    java_string(env, value).map(Some).or_else(|| {
        throw_merman_exception(env, format!("{name} was not a valid Java string"));
        None
    })
}

fn java_string(env: &mut Env<'_>, value: JString<'_>) -> Option<String> {
    match decode_java_string_strict(env, &value) {
        Ok(value) => Some(value),
        Err(JavaStringDecodeError::Jni(error)) => {
            throw_merman_exception(env, format!("failed to read Java string: {error}"));
            None
        }
        Err(JavaStringDecodeError::InvalidModifiedUtf8(error)) => {
            throw_merman_exception(env, format!("Java string was not valid Unicode: {error}"));
            None
        }
    }
}

enum JavaStringDecodeError {
    Jni(JniError),
    InvalidModifiedUtf8(simd_cesu8::DecodingError),
}

fn decode_java_string_strict(
    env: &mut Env<'_>,
    value: &JString<'_>,
) -> Result<String, JavaStringDecodeError> {
    let chars = value.mutf8_chars(env).map_err(JavaStringDecodeError::Jni)?;
    let decoded = simd_cesu8::mutf8::decode_strict(chars.to_bytes())
        .map_err(JavaStringDecodeError::InvalidModifiedUtf8)?;
    Ok(decoded.into_owned())
}

#[cfg(feature = "svg")]
struct JniIconPackInput {
    json: String,
    registration_name: Option<String>,
}

#[cfg(feature = "svg")]
struct JniIconPackReference<'local> {
    json: JString<'local>,
    json_utf8_bytes: usize,
    registration_name: Option<(JString<'local>, usize)>,
}

#[cfg(feature = "svg")]
fn build_jni_icon_registry<'local>(
    env: &mut Env<'local>,
    icon_pack_json: &JObjectArray<'local, JString<'local>>,
    icon_pack_registration_names: &JObjectArray<'local, JString<'local>>,
) -> Result<Option<BindingIconRegistry>, BindingError> {
    let json_len = icon_pack_array_len(env, icon_pack_json, icon_pack_registration_names)?;
    if json_len == 0 {
        return Ok(None);
    }

    let max_packs =
        icon_registry_limit_usize(merman_bindings_core::IconRegistryResourceLimitId::MaxPacks)?;
    if json_len > max_packs {
        return Err(BindingError::icon_registry_resource_limit(
            merman_bindings_core::IconRegistryResourceLimitId::MaxPacks,
            u64::try_from(json_len).unwrap_or(u64::MAX),
            None,
            "icon pack count exceeds the fixed registry ceiling",
        ));
    }

    use merman_bindings_core::IconRegistryResourceLimitId::{
        MaxInputBytes, MaxPackBytes, MaxPrefixBytes,
    };

    let max_pack_bytes = icon_registry_limit_usize(MaxPackBytes)?;
    let max_input_bytes = icon_registry_limit_usize(MaxInputBytes)?;
    let max_prefix_bytes = icon_registry_limit_usize(MaxPrefixBytes)?;

    // Phase one keeps only bounded local references and exact byte counts. No Java string is
    // copied or decoded until every pack and registration name passes the complete preflight.
    let mut references = Vec::with_capacity(json_len);
    let mut input_bytes = 0usize;
    for index in 0..json_len {
        let json = icon_pack_json
            .get_element(env, index)
            .map_err(|error| jni_icon_input_error("read icon-pack JSON element", error))?;
        if json.is_null() {
            return Err(BindingError::invalid_argument(format!(
                "icon-pack JSON element {index} must not be null"
            )));
        }
        let json_utf8_bytes = measure_icon_java_string_utf8(
            env,
            &json,
            max_pack_bytes,
            MaxPackBytes,
            index,
            "icon pack contains an unpaired UTF-16 surrogate",
            "icon pack bytes exceed the fixed per-pack ceiling",
        )?;
        let next_input_bytes = input_bytes.checked_add(json_utf8_bytes).ok_or_else(|| {
            BindingError::icon_registry_resource_limit(
                MaxInputBytes,
                u64::MAX,
                Some(index),
                "aggregate icon pack byte accounting overflowed",
            )
        })?;
        if next_input_bytes > max_input_bytes {
            return Err(BindingError::icon_registry_resource_limit(
                MaxInputBytes,
                u64::try_from(next_input_bytes).unwrap_or(u64::MAX),
                Some(index),
                "aggregate icon pack bytes exceed the fixed registry ceiling",
            ));
        }

        let registration_name = icon_pack_registration_names
            .get_element(env, index)
            .map_err(|error| {
                jni_icon_input_error("read icon-pack registration-name element", error)
            })?;
        let registration_name = if registration_name.is_null() {
            None
        } else {
            let utf8_bytes = measure_icon_java_string_utf8(
                env,
                &registration_name,
                max_prefix_bytes,
                MaxPrefixBytes,
                index,
                "icon registry registration name contains an unpaired UTF-16 surrogate",
                "icon registry registration name exceeds the fixed byte ceiling",
            )?;
            Some((registration_name, utf8_bytes))
        };
        references.push(JniIconPackReference {
            json,
            json_utf8_bytes,
            registration_name,
        });
        input_bytes = next_input_bytes;
    }

    // Phase two performs the bounded copies and strict MUTF-8 decoding only after the complete
    // transaction has passed the byte preflight.
    let mut inputs = Vec::with_capacity(references.len());
    for (index, reference) in references.iter().enumerate() {
        let json = decode_icon_java_string(
            env,
            &reference.json,
            reference.json_utf8_bytes,
            index,
            "read icon-pack JSON element",
            "icon pack was not valid Unicode",
        )?;
        let registration_name = match reference.registration_name.as_ref() {
            Some((value, utf8_bytes)) => Some(decode_icon_java_string(
                env,
                value,
                *utf8_bytes,
                index,
                "read icon-pack registration name",
                "icon registry registration name was not valid Unicode",
            )?),
            None => None,
        };
        inputs.push(JniIconPackInput {
            json,
            registration_name,
        });
    }

    build_icon_registry(inputs.iter().map(|input| {
        let pack = IconPack::new(input.json.as_bytes());
        match input.registration_name.as_deref() {
            Some(registration_name) => pack.with_registration_name(registration_name),
            None => pack,
        }
    }))
    .map(Some)
}

fn icon_pack_array_len<'local>(
    env: &mut Env<'local>,
    icon_pack_json: &JObjectArray<'local, JString<'local>>,
    icon_pack_registration_names: &JObjectArray<'local, JString<'local>>,
) -> Result<usize, BindingError> {
    if icon_pack_json.is_null() || icon_pack_registration_names.is_null() {
        return Err(BindingError::invalid_argument(
            "icon-pack JSON and registration-name arrays must not be null",
        ));
    }
    let json_len = icon_pack_json
        .len(env)
        .map_err(|error| jni_icon_input_error("read icon-pack JSON array", error))?;
    let registration_name_len = icon_pack_registration_names
        .len(env)
        .map_err(|error| jni_icon_input_error("read icon-pack registration-name array", error))?;
    if json_len != registration_name_len {
        return Err(BindingError::invalid_argument(format!(
            "icon-pack JSON and registration-name arrays must have the same length ({json_len} != {registration_name_len})"
        )));
    }
    Ok(json_len)
}

#[cfg(feature = "svg")]
fn decode_icon_java_string(
    env: &mut Env<'_>,
    value: &JString<'_>,
    expected_utf8_bytes: usize,
    pack_index: usize,
    context: &'static str,
    invalid_message: &'static str,
) -> Result<String, BindingError> {
    let decoded = match decode_java_string_strict(env, value) {
        Ok(decoded) => decoded,
        Err(JavaStringDecodeError::Jni(error)) => {
            return Err(jni_icon_input_error(context, error));
        }
        Err(JavaStringDecodeError::InvalidModifiedUtf8(_)) => {
            return Err(BindingError::icon_registry_invalid_utf8(
                pack_index,
                invalid_message,
            ));
        }
    };
    if decoded.len() != expected_utf8_bytes {
        return Err(BindingError::internal(format!(
            "Java UTF-8 preflight changed during {context} for icon pack {pack_index}"
        )));
    }
    Ok(decoded)
}

#[cfg(feature = "svg")]
fn measure_icon_java_string_utf8(
    env: &mut Env<'_>,
    value: &JString<'_>,
    maximum_bytes: usize,
    limit: merman_bindings_core::IconRegistryResourceLimitId,
    pack_index: usize,
    invalid_message: &'static str,
    limit_message: &'static str,
) -> Result<usize, BindingError> {
    let utf8_bytes = env
        .call_static_method(
            jni::jni_str!("io/merman/MermanJniStrings"),
            jni::jni_str!("utf8Length"),
            jni::jni_sig!((value: java.lang.String) -> jlong),
            &[JValue::Object(value)],
        )
        .and_then(|value| value.j())
        .map_err(|error| jni_icon_input_error("measure Java string UTF-8 length", error))?;
    if utf8_bytes < 0 {
        return Err(BindingError::icon_registry_invalid_utf8(
            pack_index,
            invalid_message,
        ));
    }
    let utf8_bytes = usize::try_from(utf8_bytes)
        .map_err(|_| BindingError::internal("Java UTF-8 byte length did not fit usize"))?;
    if utf8_bytes > maximum_bytes {
        return Err(BindingError::icon_registry_resource_limit(
            limit,
            u64::try_from(utf8_bytes).unwrap_or(u64::MAX),
            Some(pack_index),
            limit_message,
        ));
    }
    Ok(utf8_bytes)
}

#[cfg(feature = "svg")]
fn icon_registry_limit_usize(
    limit: merman_bindings_core::IconRegistryResourceLimitId,
) -> Result<usize, BindingError> {
    usize::try_from(limit.fixed_value()).map_err(|_| {
        BindingError::internal(format!(
            "Android cannot represent icon registry limit `{}`",
            limit.stable_id(),
        ))
    })
}

fn jni_icon_input_error(context: &str, error: JniError) -> BindingError {
    BindingError::internal(format!("failed to {context}: {error}"))
}

fn result_to_java_operation_result<'local>(
    env: &mut Env<'local>,
    result: Result<BindingOperationResult, BindingError>,
) -> JniResult<JObject<'local>> {
    match result {
        Ok(result) => match new_operation_result(env, result) {
            Ok(result) => Ok(result),
            Err(error) => {
                throw_merman_exception(
                    env,
                    format!("failed to allocate Java operation result: {error}"),
                );
                Ok(JObject::null())
            }
        },
        Err(error) => {
            throw_merman_exception(env, binding_error_text(error));
            Ok(JObject::null())
        }
    }
}

fn new_operation_result<'local>(
    env: &mut Env<'local>,
    result: BindingOperationResult,
) -> Result<JObject<'local>, String> {
    let (operation, media_type, data, metadata) = result.into_parts();
    let operation_id = env
        .new_string(operation.operation_id())
        .map_err(|error| error.to_string())?;
    let media_type = env
        .new_string(media_type)
        .map_err(|error| error.to_string())?;
    let data = env
        .byte_array_from_slice(&data)
        .map_err(|error| error.to_string())?;
    let metadata_json = std::str::from_utf8(metadata.json_bytes())
        .map_err(|error| format!("native operation metadata was not UTF-8: {error}"))?;
    let metadata_json = env
        .new_string(metadata_json)
        .map_err(|error| error.to_string())?;

    env.new_object(
        jni::jni_str!("io/merman/MermanOperationResult"),
        jni::jni_sig!("(Ljava/lang/String;Ljava/lang/String;[BLjava/lang/String;)V"),
        &[
            JValue::Object(&JObject::from(operation_id)),
            JValue::Object(&JObject::from(media_type)),
            JValue::Object(&JObject::from(data)),
            JValue::Object(&JObject::from(metadata_json)),
        ],
    )
    .map_err(|error| error.to_string())
}

#[cfg(feature = "svg")]
fn jni_binding_error(error: JniError) -> BindingError {
    BindingError::new(
        BindingStatus::InternalError,
        format!("failed to retain Android text measurer: {error}"),
    )
}

fn result_to_java_string<'local>(
    env: &mut Env<'local>,
    result: Result<Vec<u8>, BindingError>,
) -> JniResult<JString<'local>> {
    match result {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => env.new_string(text),
            Err(error) => {
                throw_merman_exception(env, format!("native metadata was not UTF-8: {error}"));
                Ok(JString::null())
            }
        },
        Err(error) => {
            throw_merman_exception(env, binding_error_text(error));
            Ok(JString::null())
        }
    }
}

fn throw_merman_exception(env: &mut Env<'_>, message: impl AsRef<str>) {
    let message = JNIString::new(message.as_ref());
    let _ = env.throw_new(jni::jni_str!("io/merman/MermanException"), &message);
    if env.exception_check() {
        return;
    }
    let _ = env.throw_new(jni::jni_str!("java/lang/RuntimeException"), &message);
}

#[cfg(feature = "svg")]
fn new_text_measure_request<'local>(
    env: &mut Env<'local>,
    request: merman_bindings_core::HostTextMeasurementRequest<'_>,
) -> JniResult<JObject<'local>> {
    let transport = merman_bindings_core::host_text_measurement_transport_fields(request);
    let style = request.style;
    let text = env.new_string(request.text)?;
    let font_family = env.new_string(style.font_family.as_deref().unwrap_or_default())?;
    let font_weight = env.new_string(
        style
            .font_weight
            .as_deref()
            .unwrap_or(JniHostTextMeasurer::DEFAULT_FONT_WEIGHT),
    )?;
    let font_style = env.new_string(
        style
            .font_style
            .as_deref()
            .unwrap_or(JniHostTextMeasurer::DEFAULT_FONT_STYLE),
    )?;
    let max_width = request.max_width.unwrap_or(0.0);
    let max_width_object = if request.max_width.is_some() {
        env.call_static_method(
            jni::jni_str!("java/lang/Double"),
            jni::jni_str!("valueOf"),
            jni::jni_sig!((v: f64) -> java.lang.Double),
            &[JValue::Double(max_width)],
        )?
        .l()?
    } else {
        JObject::null()
    };

    env.new_object(
        jni::jni_str!("io/merman/MermanTextMeasureRequest"),
        jni::jni_sig!(
            "(Ljava/lang/String;Ljava/lang/String;DLjava/lang/String;Ljava/lang/String;Ljava/lang/Double;DDDIIIII)V"
        ),
        &[
            JValue::Object(&JObject::from(text)),
            JValue::Object(&JObject::from(font_family)),
            JValue::Double(style.font_size),
            JValue::Object(&JObject::from(font_weight)),
            JValue::Object(&JObject::from(font_style)),
            JValue::Object(&max_width_object),
            JValue::Double(transport.line_height),
            JValue::Double(0.0),
            JValue::Double(0.0),
            JValue::Int(transport.wrap_mode),
            JValue::Int(transport.direction),
            JValue::Int(transport.white_space),
            JValue::Int(transport.phase),
            JValue::Int(transport.operation),
        ],
    )
}

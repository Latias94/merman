#![cfg(target_os = "android")]
#![deny(unsafe_op_in_unsafe_fn)]

//! Android's direct bindings-core transport.
//!
//! JNI is intentionally not layered over the C ABI. `JNI_OnLoad` registers this module's small,
//! typed method set, and every diagram operation goes through `BindingEngine::execute` with the
//! generated operation vocabulary.

use jni::{
    Env, EnvUnowned, JavaVM, NativeMethod,
    errors::{Result as JniResult, ThrowRuntimeExAndDefault},
    objects::{JClass, JObject, JString},
    strings::JNIString,
    sys::{JNI_ERR, JNI_VERSION_1_6, jbyteArray, jint, jlong, jstring},
};
#[cfg(feature = "svg")]
use jni::{
    errors::Error as JniError,
    objects::{Global, JValue},
};
use merman_bindings_core::{BindingEngine, BindingError, BindingOperationRequest, BindingStatus};
#[cfg(feature = "svg")]
use std::cell::Cell;
use std::{
    collections::BTreeMap,
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
    sync::{Arc, Condvar, Mutex, OnceLock},
};

const ANDROID_TRANSPORT_API_VERSION: u32 = 1;
struct JniReusableEngine {
    #[cfg(feature = "svg")]
    base: BindingEngine,
    inner: Mutex<BindingEngine>,
    coordinator: Arc<JniExecutionCoordinator>,
}

#[derive(Default)]
struct JniExecutionState {
    operation_active: bool,
    callback_active: bool,
    retired: bool,
}

#[derive(Default)]
struct JniExecutionCoordinator {
    state: Mutex<JniExecutionState>,
    ready: Condvar,
}

impl JniExecutionCoordinator {
    fn enter_operation(&self) -> Result<JniOperationGuard<'_>, BindingError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        loop {
            if state.retired {
                return Err(BindingError::new(
                    BindingStatus::InvalidArgument,
                    "Merman reusable engine is closed",
                ));
            }
            if state.callback_active {
                return Err(BindingError::reentrant_call(
                    "Merman reusable engine cannot be re-entered from a native callback",
                ));
            }
            if !state.operation_active {
                state.operation_active = true;
                return Ok(JniOperationGuard { coordinator: self });
            }
            state = self
                .ready
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }

    #[cfg(feature = "svg")]
    fn enter_callback(&self) -> JniCallbackGuard<'_> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        debug_assert!(
            state.operation_active,
            "Android host callbacks run inside an engine operation"
        );
        debug_assert!(
            !state.callback_active,
            "Android host callbacks are not recursively nested"
        );
        state.callback_active = true;
        self.ready.notify_all();
        JniCallbackGuard { coordinator: self }
    }

    fn retire(&self) -> Result<(), BindingError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.callback_active {
            return Err(BindingError::reentrant_call(
                "Merman reusable engine cannot be closed from a native callback",
            ));
        }
        if state.retired {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                "Merman reusable engine is closed",
            ));
        }
        state.retired = true;
        Ok(())
    }
}

struct JniOperationGuard<'a> {
    coordinator: &'a JniExecutionCoordinator,
}

impl Drop for JniOperationGuard<'_> {
    fn drop(&mut self) {
        let mut state = self
            .coordinator
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        debug_assert!(!state.callback_active);
        state.operation_active = false;
        self.coordinator.ready.notify_all();
    }
}

#[cfg(feature = "svg")]
struct JniCallbackGuard<'a> {
    coordinator: &'a JniExecutionCoordinator,
}

#[cfg(feature = "svg")]
impl Drop for JniCallbackGuard<'_> {
    fn drop(&mut self) {
        let mut state = self
            .coordinator
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.callback_active = false;
        self.coordinator.ready.notify_all();
    }
}

#[derive(Default)]
struct JniEngineRegistry {
    last_token: u64,
    engines: BTreeMap<u64, Arc<JniReusableEngine>>,
}

impl JniEngineRegistry {
    fn register(&mut self, engine: Arc<JniReusableEngine>) -> Result<jlong, BindingError> {
        let token = self
            .last_token
            .checked_add(1)
            .filter(|token| *token <= i64::MAX as u64)
            .ok_or_else(|| {
                BindingError::new(
                    BindingStatus::InternalError,
                    "Android reusable-engine token space is exhausted",
                )
            })?;
        self.last_token = token;
        let previous = self.engines.insert(token, engine);
        debug_assert!(previous.is_none(), "Android engine tokens are never reused");
        Ok(token as jlong)
    }

    fn acquire(&self, token: u64) -> Option<Arc<JniReusableEngine>> {
        self.engines.get(&token).map(Arc::clone)
    }

    fn retire(&mut self, token: u64) -> Option<Arc<JniReusableEngine>> {
        self.engines.remove(&token)
    }
}

static ENGINE_REGISTRY: OnceLock<Mutex<JniEngineRegistry>> = OnceLock::new();

#[cfg(feature = "svg")]
struct JniHostTextMeasurer {
    vm: JavaVM,
    callback: Global<JObject<'static>>,
    coordinator: Arc<JniExecutionCoordinator>,
}

#[cfg(feature = "svg")]
impl JniHostTextMeasurer {
    const DEFAULT_FONT_STYLE: &'static str = "normal";
    const DEFAULT_FONT_WEIGHT: &'static str = "normal";

    fn new(
        vm: JavaVM,
        callback: Global<JObject<'static>>,
        coordinator: Arc<JniExecutionCoordinator>,
    ) -> Self {
        Self {
            vm,
            callback,
            coordinator,
        }
    }

    fn call_host(
        &self,
        request: merman_bindings_core::HostTextMeasurementRequest<'_>,
    ) -> merman_bindings_core::HostMeasurementResult {
        let _callback = self.coordinator.enter_callback();
        let callback_failed = Cell::new(false);
        let result = self
            .vm
            .attach_current_thread(
                |env| -> JniResult<Option<merman_bindings_core::HostTextMeasurement>> {
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

                    Ok(Some(merman_bindings_core::host_text_measurement_from_values(
                        merman_bindings_core::HostTextMeasurementResultKind::from_external_code(
                            result_kind,
                        ),
                        merman_bindings_core::HostTextMeasurementValues {
                            width,
                            height,
                            line_count: usize::try_from(line_count).unwrap_or(0),
                            length,
                            bbox_left,
                            bbox_right,
                            raw_width: has_raw_width.then_some(raw_width),
                        },
                    )))
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
        Ok(result)
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

macro_rules! native_method {
    ($name:literal, $signature:literal, $function:ident) => {{
        // Safety: each entry is adjacent to the Kotlin declaration and uses a static JNI method
        // callback with the exact descriptor recorded in `register_native_methods`.
        unsafe {
            NativeMethod::from_raw_parts(
                jni::jni_str!($name),
                jni::jni_str!($signature),
                $function as *const () as *mut c_void,
            )
        }
    }};
}

fn register_native_methods(env: &mut Env<'_>) -> JniResult<()> {
    let engine_class = env.find_class(jni::jni_str!("io/merman/MermanEngine"))?;
    let engine_methods = [
        native_method!(
            "nativeRuntimeCatalogJson",
            "()Ljava/lang/String;",
            native_runtime_catalog_json
        ),
        native_method!(
            "nativeExecute",
            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)[B",
            native_execute
        ),
        native_method!(
            "nativeMetadataJson",
            "(Ljava/lang/String;)Ljava/lang/String;",
            native_metadata_json
        ),
    ];
    // The Kotlin declarations are static (`@JvmStatic`) and the signatures above are kept next to
    // their Rust callbacks so a load failure replaces undefined name-based lookup.
    unsafe { env.register_native_methods(engine_class, &engine_methods)? };

    let reusable_class = env.find_class(jni::jni_str!("io/merman/MermanReusableEngine"))?;
    let reusable_methods = [
        native_method!("nativeNew", "(Ljava/lang/String;)J", native_engine_new),
        native_method!("nativeFree", "(J)V", native_engine_free),
        native_method!(
            "nativeSetTextMeasurer",
            "(JLio/merman/MermanTextMeasurer;)V",
            native_engine_set_text_measurer
        ),
        native_method!(
            "nativeExecute",
            "(JLjava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)[B",
            native_engine_execute
        ),
    ];
    unsafe { env.register_native_methods(reusable_class, &reusable_methods) }
}

pub extern "system" fn native_runtime_catalog_json(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
) -> jstring {
    with_env_resolved(&mut unowned_env, |env| {
        Ok(result_to_java_string(
            env,
            merman_bindings_core::runtime_catalog_json(ANDROID_TRANSPORT_API_VERSION),
        ))
    })
}

pub extern "system" fn native_execute(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    operation_id: JString<'_>,
    source: JString<'_>,
    options_json: JObject<'_>,
    uri: JObject<'_>,
) -> jbyteArray {
    with_env_resolved(&mut unowned_env, |env| {
        let Some(operation_id) = required_java_string(env, operation_id, "operationId") else {
            return Ok(ptr::null_mut());
        };
        let Some(source) = required_java_string(env, source, "source") else {
            return Ok(ptr::null_mut());
        };
        let Some(options_json) = optional_java_string(env, options_json, "optionsJson") else {
            return Ok(ptr::null_mut());
        };
        let Some(uri) = nullable_java_string(env, uri, "uri") else {
            return Ok(ptr::null_mut());
        };
        let result = BindingEngine::from_options(options_json.as_bytes()).and_then(|engine| {
            execute_operation(
                &engine,
                &operation_id,
                source.as_bytes(),
                b"",
                uri.as_deref(),
            )
        });
        Ok(result_to_java_bytes(env, result))
    })
}

pub extern "system" fn native_metadata_json(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    id: JString<'_>,
) -> jstring {
    with_env_resolved(&mut unowned_env, |env| {
        let Some(id) = required_java_string(env, id, "metadataId") else {
            return Ok(ptr::null_mut());
        };
        Ok(result_to_java_string(env, metadata_json(&id)))
    })
}

pub extern "system" fn native_engine_new(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    options_json: JObject<'_>,
) -> jlong {
    with_env_resolved(&mut unowned_env, |env| {
        let Some(options_json) = optional_java_string(env, options_json, "optionsJson") else {
            return Ok(0);
        };
        let result = BindingEngine::from_options(options_json.as_bytes()).and_then(|engine| {
            let coordinator = Arc::new(JniExecutionCoordinator::default());
            let state = Arc::new(JniReusableEngine {
                #[cfg(feature = "svg")]
                base: engine.clone(),
                inner: Mutex::new(engine),
                coordinator,
            });
            engine_registry()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .register(state)
        });
        match result {
            Ok(handle) => Ok(handle),
            Err(error) => {
                throw_merman_exception(env, binding_error_text(error));
                Ok(0)
            }
        }
    })
}

pub extern "system" fn native_engine_free(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
) {
    with_env_resolved(&mut unowned_env, |env| {
        let Some(token) = engine_token(env, handle) else {
            return Ok(());
        };
        let Some(state) = acquire_engine(env, token) else {
            return Ok(());
        };
        if let Err(error) = state.coordinator.retire() {
            throw_merman_exception(env, binding_error_text(error));
            return Ok(());
        }
        let retired = engine_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retire(token);
        if retired.is_none() {
            throw_merman_exception(env, "Merman reusable engine is closed");
        }
        Ok(())
    })
}

pub extern "system" fn native_engine_set_text_measurer(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    measurer: JObject<'_>,
) {
    with_env_resolved(&mut unowned_env, |env| {
        let Some(token) = engine_token(env, handle) else {
            return Ok(());
        };
        let Some(state) = acquire_engine(env, token) else {
            return Ok(());
        };
        let _operation = match state.coordinator.enter_operation() {
            Ok(operation) => operation,
            Err(error) => {
                throw_merman_exception(env, binding_error_text(error));
                return Ok(());
            }
        };

        #[cfg(feature = "svg")]
        {
            let replacement = if measurer.is_null() {
                state.base.clone()
            } else {
                let callback = env.new_global_ref(&measurer)?;
                let vm = env.get_java_vm()?;
                state
                    .base
                    .clone()
                    .with_host_text_measurer(Arc::new(JniHostTextMeasurer::new(
                        vm,
                        callback,
                        Arc::clone(&state.coordinator),
                    )))
            };
            *state
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = replacement;
        }

        #[cfg(not(feature = "svg"))]
        {
            let _ = &measurer;
            throw_merman_exception(
                env,
                binding_error_text(BindingError::missing_capability(
                    "svg",
                    "host text measurement requires the svg capability",
                )),
            );
        }
        Ok(())
    })
}

pub extern "system" fn native_engine_execute(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
    operation_id: JString<'_>,
    source: JString<'_>,
    options_json: JObject<'_>,
    uri: JObject<'_>,
) -> jbyteArray {
    with_env_resolved(&mut unowned_env, |env| {
        let Some(token) = engine_token(env, handle) else {
            return Ok(ptr::null_mut());
        };
        let Some(operation_id) = required_java_string(env, operation_id, "operationId") else {
            return Ok(ptr::null_mut());
        };
        let Some(source) = required_java_string(env, source, "source") else {
            return Ok(ptr::null_mut());
        };
        let Some(options_json) = optional_java_string(env, options_json, "optionsJson") else {
            return Ok(ptr::null_mut());
        };
        let Some(uri) = nullable_java_string(env, uri, "uri") else {
            return Ok(ptr::null_mut());
        };
        let Some(state) = acquire_engine(env, token) else {
            return Ok(ptr::null_mut());
        };
        let result = state.coordinator.enter_operation().and_then(|_operation| {
            let engine = state
                .inner
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();
            execute_operation(
                &engine,
                &operation_id,
                source.as_bytes(),
                options_json.as_bytes(),
                uri.as_deref(),
            )
        });
        Ok(result_to_java_bytes(env, result))
    })
}

fn engine_registry() -> &'static Mutex<JniEngineRegistry> {
    ENGINE_REGISTRY.get_or_init(|| Mutex::new(JniEngineRegistry::default()))
}

fn engine_token(env: &mut Env<'_>, handle: jlong) -> Option<u64> {
    let token = u64::try_from(handle).ok().filter(|token| *token != 0);
    if token.is_none() {
        throw_merman_exception(env, "Merman reusable engine is closed");
    }
    token
}

fn acquire_engine(env: &mut Env<'_>, token: u64) -> Option<Arc<JniReusableEngine>> {
    let engine = engine_registry()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .acquire(token);
    if engine.is_none() {
        throw_merman_exception(env, "Merman reusable engine is closed");
    }
    engine
}

fn execute_operation(
    engine: &BindingEngine,
    operation_id: &str,
    source: &[u8],
    options_json: &[u8],
    uri: Option<&str>,
) -> Result<Vec<u8>, BindingError> {
    engine
        .execute(BindingOperationRequest {
            operation_id,
            source,
            uri: uri.map(str::as_bytes),
            options_json,
        })
        .map(|result| result.data)
}

fn metadata_json(id: &str) -> Result<Vec<u8>, BindingError> {
    match id {
        "supported-diagrams" => merman_bindings_core::supported_diagrams_json(),
        "ascii-capabilities" => merman_bindings_core::ascii_capabilities_json(),
        "diagram-family-capabilities" => merman_bindings_core::diagram_family_capabilities_json(),
        "lint-rule-catalog" => merman_bindings_core::lint_rule_catalog_json(),
        "supported-themes" => merman_bindings_core::supported_themes_json(),
        "supported-host-theme-presets" => merman_bindings_core::supported_host_theme_presets_json(),
        _ => Err(BindingError::new(
            BindingStatus::InvalidArgument,
            format!("unknown Android metadata catalog `{id}`"),
        )),
    }
}

fn with_env_resolved<T, F>(env: &mut EnvUnowned<'_>, f: F) -> T
where
    T: Default,
    F: FnOnce(&mut Env<'_>) -> JniResult<T>,
{
    env.with_env(f).resolve::<ThrowRuntimeExAndDefault>()
}

fn required_java_string(env: &mut Env<'_>, value: JString<'_>, name: &str) -> Option<String> {
    if value.is_null() {
        throw_merman_exception(env, format!("{name} must not be null"));
        return None;
    }
    java_string(env, value)
}

fn optional_java_string(env: &mut Env<'_>, value: JObject<'_>, name: &str) -> Option<String> {
    if value.is_null() {
        return Some(String::new());
    }
    let value = env.cast_local::<JString<'_>>(value).ok()?;
    java_string(env, value).or_else(|| {
        throw_merman_exception(env, format!("{name} was not a valid Java string"));
        None
    })
}

fn nullable_java_string(
    env: &mut Env<'_>,
    value: JObject<'_>,
    name: &str,
) -> Option<Option<String>> {
    if value.is_null() {
        return Some(None);
    }
    let value = env.cast_local::<JString<'_>>(value).ok()?;
    java_string(env, value).map(Some).or_else(|| {
        throw_merman_exception(env, format!("{name} was not a valid Java string"));
        None
    })
}

fn java_string(env: &mut Env<'_>, value: JString<'_>) -> Option<String> {
    match value.mutf8_chars(env) {
        Ok(value) => Some(value.to_string()),
        Err(error) => {
            throw_merman_exception(env, format!("failed to read Java string: {error}"));
            None
        }
    }
}

fn result_to_java_bytes(env: &mut Env<'_>, result: Result<Vec<u8>, BindingError>) -> jbyteArray {
    match result {
        Ok(bytes) => match env.byte_array_from_slice(&bytes) {
            Ok(array) => array.into_raw(),
            Err(error) => {
                throw_merman_exception(env, format!("failed to allocate Java byte array: {error}"));
                ptr::null_mut()
            }
        },
        Err(error) => {
            throw_merman_exception(env, binding_error_text(error));
            ptr::null_mut()
        }
    }
}

fn result_to_java_string(env: &mut Env<'_>, result: Result<Vec<u8>, BindingError>) -> jstring {
    match result {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => new_java_string(env, &text),
            Err(error) => {
                throw_merman_exception(env, format!("native metadata was not UTF-8: {error}"));
                ptr::null_mut()
            }
        },
        Err(error) => {
            throw_merman_exception(env, binding_error_text(error));
            ptr::null_mut()
        }
    }
}

fn binding_error_text(error: BindingError) -> String {
    String::from_utf8(merman_bindings_core::binding_error_payload_json_bytes(
        &error,
    ))
    .unwrap_or_else(|utf8_error| format!("native error was not UTF-8: {utf8_error}"))
}

fn new_java_string(env: &mut Env<'_>, value: &str) -> jstring {
    match env.new_string(value) {
        Ok(value) => value.into_raw(),
        Err(error) => {
            throw_merman_exception(env, format!("failed to allocate Java string: {error}"));
            ptr::null_mut()
        }
    }
}

fn throw_merman_exception(env: &mut Env<'_>, message: impl AsRef<str>) {
    let message = JNIString::new(message.as_ref());
    if env
        .throw_new(jni::jni_str!("io/merman/MermanException"), &message)
        .is_ok()
    {
        return;
    }
    env.exception_clear();
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

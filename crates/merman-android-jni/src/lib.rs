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
    objects::{JClass, JObject, JString, JValue},
    strings::JNIString,
    sys::{JNI_ERR, JNI_VERSION_1_6, jboolean, jint, jlong, jobject, jstring},
};
#[cfg(feature = "svg")]
use jni::{errors::Error as JniError, objects::Global};
use merman_bindings_core::{
    ArtifactContractSpec, BindingEngine, BindingEngineAdmission, BindingEngineAdmissionMode,
    BindingEngineServices, BindingError, BindingOperationRequest, BindingOperationResult,
    BindingStatus, BindingTransportKey, CapabilityKey, ConstructorServiceKey, OperationKey,
    RuntimePolicyExposure, TargetKey, ValidatedArtifactContract,
};
#[cfg(feature = "svg")]
use std::cell::Cell;
use std::{
    collections::BTreeMap,
    ffi::c_void,
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
    sync::{Arc, Mutex, OnceLock},
};

const ANDROID_TRANSPORT_API_VERSION: u32 = 1;

struct JniReusableEngine {
    engine: BindingEngine,
    admission: Arc<BindingEngineAdmission>,
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
const ANDROID_CONSTRUCTOR_SERVICES: &[ConstructorServiceKey] = &[
    #[cfg(feature = "svg")]
    ConstructorServiceKey::HostTextMeasurement,
];
const ANDROID_OPERATIONS: &[OperationKey] = &[
    #[cfg(feature = "analysis")]
    OperationKey::AnalysisFactsJson,
    #[cfg(feature = "analysis")]
    OperationKey::AnalysisJson,
    #[cfg(feature = "ascii")]
    OperationKey::Ascii,
    #[cfg(feature = "analysis")]
    OperationKey::DocumentAnalysisFactsJson,
    #[cfg(feature = "analysis")]
    OperationKey::DocumentAnalysisJson,
    #[cfg(feature = "jpeg")]
    OperationKey::Jpeg,
    #[cfg(feature = "svg")]
    OperationKey::LayoutJson,
    #[cfg(feature = "pdf")]
    OperationKey::Pdf,
    #[cfg(feature = "png")]
    OperationKey::Png,
    OperationKey::SemanticJson,
    #[cfg(feature = "svg")]
    OperationKey::Svg,
    #[cfg(feature = "svg")]
    OperationKey::SvgPlanJson,
    #[cfg(feature = "analysis")]
    OperationKey::ValidationJson,
];
const ANDROID_SUPPLEMENTAL_CAPABILITIES: &[CapabilityKey] = &[
    #[cfg(feature = "layout-cytoscape")]
    CapabilityKey::LayoutCytoscape,
    #[cfg(feature = "layout-elk")]
    CapabilityKey::LayoutElk,
    #[cfg(feature = "math")]
    CapabilityKey::Math,
];
const ANDROID_SYSTEM_ADAPTERS: &[CapabilityKey] = &[
    #[cfg(feature = "system-clock")]
    CapabilityKey::SystemClock,
    #[cfg(feature = "system-random")]
    CapabilityKey::SystemRandom,
    #[cfg(feature = "system-timezone")]
    CapabilityKey::SystemTimezone,
];
const ANDROID_RUNTIME_POLICY: RuntimePolicyExposure = RuntimePolicyExposure::BindingOptions;
static ARTIFACT_CONTRACT: ValidatedArtifactContract =
    ArtifactContractSpec::new(TargetKey::Native, BindingTransportKey::AndroidJni)
        .with_operations(ANDROID_OPERATIONS)
        .with_supplemental_capabilities(ANDROID_SUPPLEMENTAL_CAPABILITIES)
        .with_all_available_metadata()
        .with_constructor_services(ANDROID_CONSTRUCTOR_SERVICES)
        .with_system_adapters(ANDROID_SYSTEM_ADAPTERS)
        .with_runtime_policy_exposure(ANDROID_RUNTIME_POLICY)
        .materialize();

fn android_artifact_contract() -> &'static ValidatedArtifactContract {
    &ARTIFACT_CONTRACT
}

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
            "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Lio/merman/MermanOperationResult;",
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
        native_method!(
            "nativeNew",
            "(Ljava/lang/String;Lio/merman/MermanTextMeasurer;)J",
            native_engine_new
        ),
        native_method!("nativeTryClose", "(J)Z", native_engine_try_close),
        native_method!(
            "nativeExecute",
            "(JLjava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)Lio/merman/MermanOperationResult;",
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
            android_artifact_contract().runtime_catalog_json(ANDROID_TRANSPORT_API_VERSION),
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
) -> jobject {
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
        let result = android_artifact_contract().execute_once(
            BindingOperationRequest::new(&operation_id, source.as_bytes())
                .with_optional_uri(uri.as_deref().map(str::as_bytes))
                .with_options_json(options_json.as_bytes()),
        );
        Ok(result_to_java_operation_result(env, result))
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
        Ok(result_to_java_string(
            env,
            android_artifact_contract().metadata_json(&id),
        ))
    })
}

pub extern "system" fn native_engine_new(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    options_json: JObject<'_>,
    measurer: JObject<'_>,
) -> jlong {
    with_env_resolved(&mut unowned_env, |env| {
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
            let services = if measurer.is_null() {
                BindingEngineServices::new()
            } else {
                let callback = env.new_global_ref(&measurer).map_err(jni_binding_error)?;
                let vm = env.get_java_vm().map_err(jni_binding_error)?;
                BindingEngineServices::new().with_host_text_measurer(Arc::new(
                    JniHostTextMeasurer::new(vm, callback, Arc::clone(&admission)),
                ))
            };

            #[cfg(not(feature = "svg"))]
            let services = if measurer.is_null() {
                BindingEngineServices::new()
            } else {
                return Err(BindingError::missing_capability(
                    "svg",
                    "host text measurement requires the svg capability",
                ));
            };

            let engine = android_artifact_contract()
                .create_engine_with_services(options_json.as_bytes(), services)?;
            let state = Arc::new(JniReusableEngine { engine, admission });
            engine_registry()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .register(state)
        })();
        match result {
            Ok(handle) => Ok(handle),
            Err(error) => {
                throw_merman_exception(env, binding_error_text(error));
                Ok(0)
            }
        }
    })
}

pub extern "system" fn native_engine_try_close(
    mut unowned_env: EnvUnowned<'_>,
    _class: JClass<'_>,
    handle: jlong,
) -> jboolean {
    with_env_resolved(&mut unowned_env, |env| {
        let Some(token) = engine_token(env, handle) else {
            return Ok(false);
        };
        let Some(state) = acquire_engine(env, token) else {
            return Ok(false);
        };
        if let Err(error) = state.admission.try_close() {
            throw_merman_exception(env, binding_error_text(error.into()));
            return Ok(false);
        }
        let retired = engine_registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .retire(token);
        if retired.is_none() {
            throw_merman_exception(env, "Merman reusable engine is closed");
            return Ok(false);
        }
        Ok(true)
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
) -> jobject {
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
        let result = state
            .admission
            .enter_operation()
            .map_err(BindingError::from)
            .and_then(|_operation| {
                state.engine.execute(
                    BindingOperationRequest::new(&operation_id, source.as_bytes())
                        .with_optional_uri(uri.as_deref().map(str::as_bytes))
                        .with_options_json(options_json.as_bytes()),
                )
            });
        Ok(result_to_java_operation_result(env, result))
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

fn result_to_java_operation_result(
    env: &mut Env<'_>,
    result: Result<BindingOperationResult, BindingError>,
) -> jobject {
    match result {
        Ok(result) => match new_operation_result(env, result) {
            Ok(result) => result.into_raw(),
            Err(error) => {
                throw_merman_exception(
                    env,
                    format!("failed to allocate Java operation result: {error}"),
                );
                ptr::null_mut()
            }
        },
        Err(error) => {
            throw_merman_exception(env, binding_error_text(error));
            ptr::null_mut()
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

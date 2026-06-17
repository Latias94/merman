use jni::{
    JNIEnv, JavaVM,
    objects::{GlobalRef, JClass, JObject, JString, JValue},
    sys::{jint, jlong, jstring},
};
#[cfg(feature = "render")]
use merman_bindings_core::TextMeasurer;
use merman_bindings_core::{BindingError, BindingStatus};
use std::ptr;
#[cfg(feature = "render")]
use std::sync::Arc;

#[cfg(feature = "render")]
const TEXT_MEASURE_REQUEST_CLASS: &str = "io/merman/MermanTextMeasureRequest";

struct JniReusableEngine {
    base: merman_bindings_core::BindingEngine,
    inner: merman_bindings_core::BindingEngine,
}

#[cfg(feature = "render")]
struct JniHostTextMeasurer {
    vm: JavaVM,
    callback: GlobalRef,
    fallback: merman_bindings_core::VendoredFontMetricsTextMeasurer,
}

#[cfg(feature = "render")]
impl JniHostTextMeasurer {
    const DEFAULT_FONT_STYLE: &'static str = "normal";
    const DEFAULT_FONT_WEIGHT: &'static str = "normal";

    fn new(vm: JavaVM, callback: GlobalRef) -> Self {
        Self {
            vm,
            callback,
            fallback: merman_bindings_core::VendoredFontMetricsTextMeasurer::default(),
        }
    }

    fn call_host(
        &self,
        text: &str,
        style: &merman_bindings_core::TextStyle,
        max_width: Option<f64>,
        wrap_mode: merman_bindings_core::WrapMode,
    ) -> Option<merman_bindings_core::TextMetrics> {
        let mut env = self.vm.attach_current_thread().ok()?;
        let request = new_text_measure_request(&mut env, text, style, max_width, wrap_mode).ok()?;
        if env.exception_check().ok()? {
            let _ = env.exception_clear();
            return None;
        }

        let result = env
            .call_method(
                self.callback.as_obj(),
                "measure",
                "(Lio/merman/MermanTextMeasureRequest;)Lio/merman/MermanTextMeasureResult;",
                &[JValue::Object(&request)],
            )
            .ok()?
            .l()
            .ok()?;
        if env.exception_check().ok()? {
            let _ = env.exception_clear();
            return None;
        }
        if result.is_null() {
            return None;
        }

        let width = env.get_field(&result, "width", "D").ok()?.d().ok()?;
        let height = env.get_field(&result, "height", "D").ok()?.d().ok()?;
        let line_count = env.get_field(&result, "lineCount", "J").ok()?.j().ok()?;
        if !width.is_finite()
            || !height.is_finite()
            || width < 0.0
            || height < 0.0
            || line_count <= 0
        {
            return None;
        }

        Some(merman_bindings_core::TextMetrics {
            width,
            height,
            line_count: line_count as usize,
        })
    }

    fn measure_with_fallback(
        &self,
        text: &str,
        style: &merman_bindings_core::TextStyle,
        max_width: Option<f64>,
        wrap_mode: merman_bindings_core::WrapMode,
    ) -> merman_bindings_core::TextMetrics {
        self.call_host(text, style, max_width, wrap_mode)
            .unwrap_or_else(|| {
                self.fallback
                    .measure_wrapped(text, style, max_width, wrap_mode)
            })
    }
}

#[cfg(feature = "render")]
impl merman_bindings_core::TextMeasurer for JniHostTextMeasurer {
    fn measure(
        &self,
        text: &str,
        style: &merman_bindings_core::TextStyle,
    ) -> merman_bindings_core::TextMetrics {
        self.call_host(text, style, None, merman_bindings_core::WrapMode::SvgLike)
            .unwrap_or_else(|| self.fallback.measure(text, style))
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &merman_bindings_core::TextStyle,
        max_width: Option<f64>,
        wrap_mode: merman_bindings_core::WrapMode,
    ) -> merman_bindings_core::TextMetrics {
        self.measure_with_fallback(text, style, max_width, wrap_mode)
    }

    fn measure_wrapped_with_raw_width(
        &self,
        text: &str,
        style: &merman_bindings_core::TextStyle,
        max_width: Option<f64>,
        wrap_mode: merman_bindings_core::WrapMode,
    ) -> (merman_bindings_core::TextMetrics, Option<f64>) {
        if let Some(metrics) = self.call_host(text, style, max_width, wrap_mode) {
            let raw_width = max_width
                .and_then(|_| self.call_host(text, style, None, wrap_mode))
                .map(|raw| raw.width);
            return (metrics, raw_width);
        }
        self.fallback
            .measure_wrapped_with_raw_width(text, style, max_width, wrap_mode)
    }

    fn measure_wrapped_raw(
        &self,
        text: &str,
        style: &merman_bindings_core::TextStyle,
        max_width: Option<f64>,
        wrap_mode: merman_bindings_core::WrapMode,
    ) -> merman_bindings_core::TextMetrics {
        self.call_host(text, style, max_width, wrap_mode)
            .unwrap_or_else(|| {
                self.fallback
                    .measure_wrapped_raw(text, style, max_width, wrap_mode)
            })
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanEngine_nativeAbiVersion(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jint {
    super::merman_abi_version() as jint
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanEngine_nativePackageVersion(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    new_java_string(&mut env, env!("CARGO_PKG_VERSION"))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanEngine_nativeBufferStructSize(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jlong {
    super::merman_buffer_struct_size() as jlong
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanEngine_nativeResultStructSize(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jlong {
    super::merman_result_struct_size() as jlong
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanReusableEngine_nativeNew(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    options_json: JObject<'_>,
) -> jlong {
    let Some(options_json) = optional_java_string(&mut env, options_json, "optionsJson") else {
        return 0;
    };

    match merman_bindings_core::BindingEngine::new(options_json.as_bytes()) {
        Ok(engine) => {
            let handle = Box::new(JniReusableEngine {
                base: engine.clone(),
                inner: engine,
            });
            Box::into_raw(handle) as jlong
        }
        Err(err) => {
            throw_merman_exception(&mut env, binding_error_text(err));
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanReusableEngine_nativeFree(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
) {
    if handle == 0 {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle as *mut JniReusableEngine));
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanReusableEngine_nativeSetTextMeasurer(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
    measurer: JObject<'_>,
) {
    let Some(engine) = jni_engine_mut(&mut env, handle) else {
        return;
    };

    #[cfg(feature = "render")]
    {
        if measurer.is_null() {
            engine.inner = engine.base.clone();
            return;
        }

        let callback = match env.new_global_ref(&measurer) {
            Ok(callback) => callback,
            Err(err) => {
                throw_merman_exception(&mut env, format!("failed to retain text measurer: {err}"));
                return;
            }
        };
        let vm = match env.get_java_vm() {
            Ok(vm) => vm,
            Err(err) => {
                throw_merman_exception(&mut env, format!("failed to access Java VM: {err}"));
                return;
            }
        };
        let measurer = JniHostTextMeasurer::new(vm, callback);
        engine.inner = engine.inner.with_text_measurer(Arc::new(measurer));
    }

    #[cfg(not(feature = "render"))]
    {
        let _ = (engine, measurer);
        throw_merman_exception(
            &mut env,
            "host text measurement requires the render feature",
        );
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanReusableEngine_nativeRenderSvg(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
    source: JString<'_>,
) -> jstring {
    call_engine_binding(
        &mut env,
        handle,
        source,
        merman_bindings_core::BindingEngine::render_svg,
    )
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanReusableEngine_nativeRenderAscii(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
    source: JString<'_>,
) -> jstring {
    call_engine_binding(
        &mut env,
        handle,
        source,
        merman_bindings_core::BindingEngine::render_ascii,
    )
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanReusableEngine_nativeParseJson(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
    source: JString<'_>,
) -> jstring {
    call_engine_binding(
        &mut env,
        handle,
        source,
        merman_bindings_core::BindingEngine::parse_json,
    )
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanReusableEngine_nativeLayoutJson(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
    source: JString<'_>,
) -> jstring {
    call_engine_binding(
        &mut env,
        handle,
        source,
        merman_bindings_core::BindingEngine::layout_json,
    )
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanReusableEngine_nativeValidateJson(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    handle: jlong,
    source: JString<'_>,
) -> jstring {
    call_engine_binding(
        &mut env,
        handle,
        source,
        merman_bindings_core::BindingEngine::validate_json,
    )
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanEngine_nativeRenderSvg(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    source: JString<'_>,
    options_json: JObject<'_>,
) -> jstring {
    call_binding(
        &mut env,
        source,
        options_json,
        merman_bindings_core::render_svg,
    )
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanEngine_nativeRenderAscii(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    source: JString<'_>,
    options_json: JObject<'_>,
) -> jstring {
    call_binding(
        &mut env,
        source,
        options_json,
        merman_bindings_core::render_ascii,
    )
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanEngine_nativeParseJson(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    source: JString<'_>,
    options_json: JObject<'_>,
) -> jstring {
    call_binding(
        &mut env,
        source,
        options_json,
        merman_bindings_core::parse_json,
    )
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanEngine_nativeLayoutJson(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    source: JString<'_>,
    options_json: JObject<'_>,
) -> jstring {
    call_binding(
        &mut env,
        source,
        options_json,
        merman_bindings_core::layout_json,
    )
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanEngine_nativeValidateJson(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    source: JString<'_>,
    options_json: JObject<'_>,
) -> jstring {
    call_binding(
        &mut env,
        source,
        options_json,
        merman_bindings_core::validate_json,
    )
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanEngine_nativeSupportedDiagramsJson(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    call_metadata(&mut env, merman_bindings_core::supported_diagrams_json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanEngine_nativeAsciiSupportedDiagramsJson(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    call_metadata(
        &mut env,
        merman_bindings_core::ascii_supported_diagrams_json,
    )
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanEngine_nativeSupportedThemesJson(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    call_metadata(&mut env, merman_bindings_core::supported_themes_json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_io_merman_MermanEngine_nativeSupportedHostThemePresetsJson(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
) -> jstring {
    call_metadata(
        &mut env,
        merman_bindings_core::supported_host_theme_presets_json,
    )
}

fn call_binding<F>(
    env: &mut JNIEnv<'_>,
    source: JString<'_>,
    options_json: JObject<'_>,
    f: F,
) -> jstring
where
    F: FnOnce(&[u8], &[u8]) -> Result<Vec<u8>, BindingError>,
{
    let Some(source) = required_java_string(env, source, "source") else {
        return ptr::null_mut();
    };
    let Some(options_json) = optional_java_string(env, options_json, "optionsJson") else {
        return ptr::null_mut();
    };

    let result = super::ffi_result(|| f(source.as_bytes(), options_json.as_bytes()));
    result_to_java_string(env, result)
}

fn call_engine_binding<F>(env: &mut JNIEnv<'_>, handle: jlong, source: JString<'_>, f: F) -> jstring
where
    F: FnOnce(&merman_bindings_core::BindingEngine, &[u8]) -> Result<Vec<u8>, BindingError>,
{
    let Some(engine) = jni_engine_ref(env, handle) else {
        return ptr::null_mut();
    };
    let Some(source) = required_java_string(env, source, "source") else {
        return ptr::null_mut();
    };

    let result = super::ffi_result(|| f(&engine.inner, source.as_bytes()));
    result_to_java_string(env, result)
}

fn call_metadata<F>(env: &mut JNIEnv<'_>, f: F) -> jstring
where
    F: FnOnce() -> Result<Vec<u8>, BindingError>,
{
    let result = super::ffi_result(f);
    result_to_java_string(env, result)
}

fn jni_engine_ref<'a>(env: &mut JNIEnv<'_>, handle: jlong) -> Option<&'a JniReusableEngine> {
    if handle == 0 {
        throw_merman_exception(env, "Merman reusable engine is closed");
        return None;
    }
    Some(unsafe { &*(handle as *const JniReusableEngine) })
}

fn jni_engine_mut<'a>(env: &mut JNIEnv<'_>, handle: jlong) -> Option<&'a mut JniReusableEngine> {
    if handle == 0 {
        throw_merman_exception(env, "Merman reusable engine is closed");
        return None;
    }
    Some(unsafe { &mut *(handle as *mut JniReusableEngine) })
}

fn required_java_string(env: &mut JNIEnv<'_>, value: JString<'_>, name: &str) -> Option<String> {
    if value.is_null() {
        throw_merman_exception(env, format!("{name} must not be null"));
        return None;
    }
    java_string(env, value)
}

fn optional_java_string(env: &mut JNIEnv<'_>, value: JObject<'_>, name: &str) -> Option<String> {
    if value.is_null() {
        return Some(String::new());
    }
    let value = JString::from(value);
    java_string(env, value).or_else(|| {
        throw_merman_exception(env, format!("{name} was not a valid Java string"));
        None
    })
}

fn java_string(env: &mut JNIEnv<'_>, value: JString<'_>) -> Option<String> {
    match env.get_string(&value) {
        Ok(value) => Some(value.to_string_lossy().into_owned()),
        Err(err) => {
            throw_merman_exception(env, format!("failed to read Java string: {err}"));
            None
        }
    }
}

fn binding_error_text(err: BindingError) -> String {
    let bytes = error_payload_bytes(err);
    String::from_utf8(bytes).unwrap_or_else(|err| format!("native error was not UTF-8: {err}"))
}

fn error_payload_bytes(err: BindingError) -> Vec<u8> {
    let status = err.status();
    merman_bindings_core::error_payload_json_bytes(status, err.message())
}

fn result_to_java_string(env: &mut JNIEnv<'_>, result: super::MermanResult) -> jstring {
    let payload = take_buffer(result.data);
    let text = match String::from_utf8(payload) {
        Ok(text) => text,
        Err(err) => {
            throw_merman_exception(env, format!("native output was not UTF-8: {err}"));
            return ptr::null_mut();
        }
    };

    if result.code == BindingStatus::Ok.code() {
        new_java_string(env, &text)
    } else {
        throw_merman_exception(env, text);
        ptr::null_mut()
    }
}

fn take_buffer(buffer: super::MermanBuffer) -> Vec<u8> {
    if buffer.data.is_null() || buffer.len == 0 {
        return Vec::new();
    }
    let bytes = unsafe { std::slice::from_raw_parts(buffer.data, buffer.len).to_vec() };
    unsafe { super::merman_buffer_free(buffer) };
    bytes
}

fn new_java_string(env: &mut JNIEnv<'_>, value: &str) -> jstring {
    match env.new_string(value) {
        Ok(value) => value.into_raw(),
        Err(err) => {
            throw_merman_exception(env, format!("failed to allocate Java string: {err}"));
            ptr::null_mut()
        }
    }
}

fn throw_merman_exception(env: &mut JNIEnv<'_>, message: impl AsRef<str>) {
    let message = message.as_ref();
    if env.throw_new("io/merman/MermanException", message).is_err() {
        let _ = env.exception_clear();
        let _ = env.throw_new("java/lang/RuntimeException", message);
    }
}

#[cfg(feature = "render")]
fn new_text_measure_request<'local>(
    env: &mut JNIEnv<'local>,
    text: &str,
    style: &merman_bindings_core::TextStyle,
    max_width: Option<f64>,
    wrap_mode: merman_bindings_core::WrapMode,
) -> jni::errors::Result<JObject<'local>> {
    let text = env.new_string(text)?;
    let font_family = env.new_string(style.font_family.as_deref().unwrap_or_default())?;
    let font_weight = env.new_string(
        style
            .font_weight
            .as_deref()
            .unwrap_or(JniHostTextMeasurer::DEFAULT_FONT_WEIGHT),
    )?;
    let font_style = env.new_string(JniHostTextMeasurer::DEFAULT_FONT_STYLE)?;
    let max_width_value = max_width.unwrap_or(0.0);
    let has_max_width = max_width.is_some();
    let null_object = JObject::null();
    let max_width_object = if has_max_width {
        env.call_static_method(
            "java/lang/Double",
            "valueOf",
            "(D)Ljava/lang/Double;",
            &[JValue::Double(max_width_value)],
        )?
        .l()?
    } else {
        null_object
    };

    env.new_object(
        TEXT_MEASURE_REQUEST_CLASS,
        "(Ljava/lang/String;Ljava/lang/String;DLjava/lang/String;Ljava/lang/String;Ljava/lang/Double;DDDIII)V",
        &[
            JValue::Object(&JObject::from(text)),
            JValue::Object(&JObject::from(font_family)),
            JValue::Double(style.font_size),
            JValue::Object(&JObject::from(font_weight)),
            JValue::Object(&JObject::from(font_style)),
            JValue::Object(&max_width_object),
            JValue::Double(jni_line_height(style, wrap_mode)),
            JValue::Double(0.0),
            JValue::Double(0.0),
            JValue::Int(jni_wrap_mode(wrap_mode)),
            JValue::Int(super::MERMAN_TEXT_DIRECTION_AUTO),
            JValue::Int(jni_white_space(max_width, wrap_mode)),
        ],
    )
}

#[cfg(feature = "render")]
fn jni_wrap_mode(wrap_mode: merman_bindings_core::WrapMode) -> i32 {
    match wrap_mode {
        merman_bindings_core::WrapMode::SvgLike => super::MERMAN_WRAP_MODE_SVG_LIKE,
        merman_bindings_core::WrapMode::SvgLikeSingleRun => {
            super::MERMAN_WRAP_MODE_SVG_LIKE_SINGLE_RUN
        }
        merman_bindings_core::WrapMode::HtmlLike => super::MERMAN_WRAP_MODE_HTML_LIKE,
    }
}

#[cfg(feature = "render")]
fn jni_line_height(
    style: &merman_bindings_core::TextStyle,
    wrap_mode: merman_bindings_core::WrapMode,
) -> f64 {
    let factor = match wrap_mode {
        merman_bindings_core::WrapMode::SvgLike
        | merman_bindings_core::WrapMode::SvgLikeSingleRun => 1.1,
        merman_bindings_core::WrapMode::HtmlLike => 1.5,
    };
    style.font_size.max(1.0) * factor
}

#[cfg(feature = "render")]
fn jni_white_space(max_width: Option<f64>, wrap_mode: merman_bindings_core::WrapMode) -> i32 {
    match wrap_mode {
        merman_bindings_core::WrapMode::HtmlLike if max_width.is_some() => {
            super::MERMAN_TEXT_WHITE_SPACE_BREAK_SPACES
        }
        merman_bindings_core::WrapMode::HtmlLike => super::MERMAN_TEXT_WHITE_SPACE_NOWRAP,
        merman_bindings_core::WrapMode::SvgLike
        | merman_bindings_core::WrapMode::SvgLikeSingleRun => super::MERMAN_TEXT_WHITE_SPACE_NORMAL,
    }
}

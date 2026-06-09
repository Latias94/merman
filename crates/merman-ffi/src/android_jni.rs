use jni::{
    JNIEnv,
    objects::{JClass, JObject, JString},
    sys::{jint, jlong, jstring},
};
use merman_bindings_core::{BindingError, BindingStatus};
use std::ptr;

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

fn call_metadata<F>(env: &mut JNIEnv<'_>, f: F) -> jstring
where
    F: FnOnce() -> Result<Vec<u8>, BindingError>,
{
    let result = super::ffi_result(f);
    result_to_java_string(env, result)
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

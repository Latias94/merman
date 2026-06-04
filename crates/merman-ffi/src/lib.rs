#![deny(unsafe_op_in_unsafe_fn)]

//! C ABI exports for embedding `merman` in non-Rust hosts.
//!
//! This crate is the only place where the public FFI boundary owns unsafe code. The core
//! parser/render crates and shared binding facade remain safe Rust APIs.

use merman_bindings_core::{BindingEngine, BindingError, BindingStatus, error_payload_json_bytes};
use std::ffi::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

#[cfg(target_os = "android")]
mod android_jni;

pub const MERMAN_ABI_VERSION: u32 = 1;

const PACKAGE_VERSION: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MermanBuffer {
    pub data: *mut u8,
    pub len: usize,
}

impl MermanBuffer {
    const fn empty() -> Self {
        Self {
            data: ptr::null_mut(),
            len: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MermanResult {
    pub code: i32,
    pub data: MermanBuffer,
}

pub struct MermanEngine {
    inner: BindingEngine,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MermanEngineResult {
    pub code: i32,
    pub engine: *mut MermanEngine,
    pub data: MermanBuffer,
}

/// Return the C ABI protocol version implemented by this library.
#[unsafe(no_mangle)]
pub extern "C" fn merman_abi_version() -> u32 {
    MERMAN_ABI_VERSION
}

/// Return the `merman-ffi` crate package version as a static C string.
#[unsafe(no_mangle)]
pub extern "C" fn merman_package_version() -> *const c_char {
    PACKAGE_VERSION.as_ptr().cast()
}

/// Return the Rust-side size of `MermanBuffer`.
#[unsafe(no_mangle)]
pub extern "C" fn merman_buffer_struct_size() -> usize {
    std::mem::size_of::<MermanBuffer>()
}

/// Return the Rust-side size of `MermanResult`.
#[unsafe(no_mangle)]
pub extern "C" fn merman_result_struct_size() -> usize {
    std::mem::size_of::<MermanResult>()
}

/// Return the Rust-side size of `MermanEngineResult`.
#[unsafe(no_mangle)]
pub extern "C" fn merman_engine_result_struct_size() -> usize {
    std::mem::size_of::<MermanEngineResult>()
}

/// Create a reusable engine for repeated calls with the same options.
///
/// # Safety
///
/// - `options_json` may be null only when `options_len == 0`.
/// - Non-null pointers must be valid for reads of `options_len` bytes for the duration of the call.
/// - A returned non-null engine must be released with `merman_engine_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_engine_new(
    options_json: *const u8,
    options_len: usize,
) -> MermanEngineResult {
    ffi_engine_result(|| unsafe { engine_new_impl(options_json, options_len) })
}

/// Free an engine returned by `merman_engine_new`.
///
/// Passing null is a no-op.
///
/// # Safety
///
/// Non-null engines must have been returned by this crate and must not be freed more than once.
/// Callers must not free an engine while another thread is using it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_engine_free(engine: *mut MermanEngine) {
    if engine.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(engine));
    }
}

/// Render Mermaid source to SVG bytes using a reusable engine.
///
/// # Safety
///
/// - `engine` must be a live pointer returned by `merman_engine_new`.
/// - `source` may be null only when `source_len == 0`.
/// - Non-null source pointers must be valid for reads of `source_len` bytes.
/// - Returned non-empty buffers must be released with `merman_buffer_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_engine_render_svg(
    engine: *const MermanEngine,
    source: *const u8,
    source_len: usize,
) -> MermanResult {
    ffi_result(|| unsafe {
        let engine = engine_ref(engine)?;
        let source_bytes = raw_bytes(source, source_len, "source")?;
        engine.inner.render_svg(source_bytes)
    })
}

/// Render Mermaid source to Unicode ASCII-art text using a reusable engine.
///
/// # Safety
///
/// Safety rules are identical to `merman_engine_render_svg`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_engine_render_ascii(
    engine: *const MermanEngine,
    source: *const u8,
    source_len: usize,
) -> MermanResult {
    ffi_result(|| unsafe {
        let engine = engine_ref(engine)?;
        let source_bytes = raw_bytes(source, source_len, "source")?;
        engine.inner.render_ascii(source_bytes)
    })
}

/// Parse Mermaid source to semantic JSON bytes using a reusable engine.
///
/// # Safety
///
/// Safety rules are identical to `merman_engine_render_svg`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_engine_parse_json(
    engine: *const MermanEngine,
    source: *const u8,
    source_len: usize,
) -> MermanResult {
    ffi_result(|| unsafe {
        let engine = engine_ref(engine)?;
        let source_bytes = raw_bytes(source, source_len, "source")?;
        engine.inner.parse_json(source_bytes)
    })
}

/// Layout Mermaid source to layout JSON bytes using a reusable engine.
///
/// # Safety
///
/// Safety rules are identical to `merman_engine_render_svg`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_engine_layout_json(
    engine: *const MermanEngine,
    source: *const u8,
    source_len: usize,
) -> MermanResult {
    ffi_result(|| unsafe {
        let engine = engine_ref(engine)?;
        let source_bytes = raw_bytes(source, source_len, "source")?;
        engine.inner.layout_json(source_bytes)
    })
}

/// Validate Mermaid source using a reusable engine.
///
/// # Safety
///
/// Safety rules are identical to `merman_engine_render_svg`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_engine_validate_json(
    engine: *const MermanEngine,
    source: *const u8,
    source_len: usize,
) -> MermanResult {
    ffi_result(|| unsafe {
        let engine = engine_ref(engine)?;
        let source_bytes = raw_bytes(source, source_len, "source")?;
        engine.inner.validate_json(source_bytes)
    })
}

/// Render Mermaid source to SVG bytes.
///
/// # Safety
///
/// - `source` may be null only when `source_len == 0`.
/// - `options_json` may be null only when `options_len == 0`.
/// - Non-null pointers must be valid for reads of their paired length for the duration of the call.
/// - Returned non-empty buffers must be released with `merman_buffer_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_render_svg(
    source: *const u8,
    source_len: usize,
    options_json: *const u8,
    options_len: usize,
) -> MermanResult {
    ffi_result(|| unsafe { render_svg_impl(source, source_len, options_json, options_len) })
}

/// Render Mermaid source to Unicode ASCII-art text.
///
/// # Safety
///
/// - `source` may be null only when `source_len == 0`.
/// - `options_json` may be null only when `options_len == 0`.
/// - Non-null pointers must be valid for reads of their paired length for the duration of the call.
/// - Returned non-empty buffers must be released with `merman_buffer_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_render_ascii(
    source: *const u8,
    source_len: usize,
    options_json: *const u8,
    options_len: usize,
) -> MermanResult {
    ffi_result(|| unsafe { render_ascii_impl(source, source_len, options_json, options_len) })
}

/// Parse Mermaid source to semantic JSON bytes.
///
/// # Safety
///
/// - `source` may be null only when `source_len == 0`.
/// - `options_json` may be null only when `options_len == 0`.
/// - Non-null pointers must be valid for reads of their paired length for the duration of the call.
/// - Returned non-empty buffers must be released with `merman_buffer_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_parse_json(
    source: *const u8,
    source_len: usize,
    options_json: *const u8,
    options_len: usize,
) -> MermanResult {
    ffi_result(|| unsafe { parse_json_impl(source, source_len, options_json, options_len) })
}

/// Layout Mermaid source to layout JSON bytes.
///
/// # Safety
///
/// - `source` may be null only when `source_len == 0`.
/// - `options_json` may be null only when `options_len == 0`.
/// - Non-null pointers must be valid for reads of their paired length for the duration of the call.
/// - Returned non-empty buffers must be released with `merman_buffer_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_layout_json(
    source: *const u8,
    source_len: usize,
    options_json: *const u8,
    options_len: usize,
) -> MermanResult {
    ffi_result(|| unsafe { layout_json_impl(source, source_len, options_json, options_len) })
}

/// Validate Mermaid source and return a JSON validation payload.
///
/// # Safety
///
/// - `source` may be null only when `source_len == 0`.
/// - `options_json` may be null only when `options_len == 0`.
/// - Non-null pointers must be valid for reads of their paired length for the duration of the call.
/// - Returned non-empty buffers must be released with `merman_buffer_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_validate_json(
    source: *const u8,
    source_len: usize,
    options_json: *const u8,
    options_len: usize,
) -> MermanResult {
    ffi_result(|| unsafe { validate_json_impl(source, source_len, options_json, options_len) })
}

/// Return supported diagram type metadata as a JSON string array.
#[unsafe(no_mangle)]
pub extern "C" fn merman_supported_diagrams_json() -> MermanResult {
    ffi_result(merman_bindings_core::supported_diagrams_json)
}

/// Return ASCII-supported diagram type metadata as a JSON string array.
#[unsafe(no_mangle)]
pub extern "C" fn merman_ascii_supported_diagrams_json() -> MermanResult {
    ffi_result(merman_bindings_core::ascii_supported_diagrams_json)
}

/// Return supported theme metadata as a JSON string array.
#[unsafe(no_mangle)]
pub extern "C" fn merman_themes_json() -> MermanResult {
    ffi_result(merman_bindings_core::supported_themes_json)
}

/// Free a buffer returned by this crate.
///
/// Passing a null buffer is a no-op.
///
/// # Safety
///
/// Non-null buffers must have been returned by this crate and must not be freed more than once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_buffer_free(buffer: MermanBuffer) {
    if buffer.data.is_null() || buffer.len == 0 {
        return;
    }

    let raw = ptr::slice_from_raw_parts_mut(buffer.data, buffer.len);
    unsafe {
        drop(Box::from_raw(raw));
    }
}

fn ffi_result<F>(f: F) -> MermanResult
where
    F: FnOnce() -> Result<Vec<u8>, BindingError>,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(bytes)) => MermanResult {
            code: BindingStatus::Ok.code(),
            data: buffer_from_vec(bytes),
        },
        Ok(Err(err)) => error_result(err.status(), err.message()),
        Err(_) => error_result(BindingStatus::Panic, "panic caught at merman FFI boundary"),
    }
}

fn ffi_engine_result<F>(f: F) -> MermanEngineResult
where
    F: FnOnce() -> Result<BindingEngine, BindingError>,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(inner)) => MermanEngineResult {
            code: BindingStatus::Ok.code(),
            engine: Box::into_raw(Box::new(MermanEngine { inner })),
            data: MermanBuffer::empty(),
        },
        Ok(Err(err)) => MermanEngineResult {
            code: err.status().code(),
            engine: ptr::null_mut(),
            data: buffer_from_vec(error_payload_json_bytes(err.status(), err.message())),
        },
        Err(_) => MermanEngineResult {
            code: BindingStatus::Panic.code(),
            engine: ptr::null_mut(),
            data: buffer_from_vec(error_payload_json_bytes(
                BindingStatus::Panic,
                "panic caught at merman FFI boundary",
            )),
        },
    }
}

unsafe fn engine_new_impl(
    options_json: *const u8,
    options_len: usize,
) -> Result<BindingEngine, BindingError> {
    let options_bytes = unsafe { raw_bytes(options_json, options_len, "options_json")? };
    BindingEngine::new(options_bytes)
}

unsafe fn engine_ref<'a>(engine: *const MermanEngine) -> Result<&'a MermanEngine, BindingError> {
    if engine.is_null() {
        return Err(BindingError::new(
            BindingStatus::InvalidArgument,
            "engine pointer is null",
        ));
    }
    Ok(unsafe { &*engine })
}

unsafe fn render_svg_impl(
    source: *const u8,
    source_len: usize,
    options_json: *const u8,
    options_len: usize,
) -> Result<Vec<u8>, BindingError> {
    let source_bytes = unsafe { raw_bytes(source, source_len, "source")? };
    let options_bytes = unsafe { raw_bytes(options_json, options_len, "options_json")? };
    merman_bindings_core::render_svg(source_bytes, options_bytes)
}

unsafe fn render_ascii_impl(
    source: *const u8,
    source_len: usize,
    options_json: *const u8,
    options_len: usize,
) -> Result<Vec<u8>, BindingError> {
    let source_bytes = unsafe { raw_bytes(source, source_len, "source")? };
    let options_bytes = unsafe { raw_bytes(options_json, options_len, "options_json")? };
    merman_bindings_core::render_ascii(source_bytes, options_bytes)
}

unsafe fn parse_json_impl(
    source: *const u8,
    source_len: usize,
    options_json: *const u8,
    options_len: usize,
) -> Result<Vec<u8>, BindingError> {
    let source_bytes = unsafe { raw_bytes(source, source_len, "source")? };
    let options_bytes = unsafe { raw_bytes(options_json, options_len, "options_json")? };
    merman_bindings_core::parse_json(source_bytes, options_bytes)
}

unsafe fn layout_json_impl(
    source: *const u8,
    source_len: usize,
    options_json: *const u8,
    options_len: usize,
) -> Result<Vec<u8>, BindingError> {
    let source_bytes = unsafe { raw_bytes(source, source_len, "source")? };
    let options_bytes = unsafe { raw_bytes(options_json, options_len, "options_json")? };
    merman_bindings_core::layout_json(source_bytes, options_bytes)
}

unsafe fn validate_json_impl(
    source: *const u8,
    source_len: usize,
    options_json: *const u8,
    options_len: usize,
) -> Result<Vec<u8>, BindingError> {
    let source_bytes = unsafe { raw_bytes(source, source_len, "source")? };
    let options_bytes = unsafe { raw_bytes(options_json, options_len, "options_json")? };
    merman_bindings_core::validate_json(source_bytes, options_bytes)
}

unsafe fn raw_bytes<'a>(
    data: *const u8,
    len: usize,
    name: &'static str,
) -> Result<&'a [u8], BindingError> {
    if data.is_null() {
        if len == 0 {
            return Ok(&[]);
        }
        return Err(BindingError::new(
            BindingStatus::InvalidArgument,
            format!("{name} pointer is null but length is {len}"),
        ));
    }

    if len == 0 {
        return Ok(&[]);
    }

    Ok(unsafe { std::slice::from_raw_parts(data, len) })
}

fn buffer_from_vec(bytes: Vec<u8>) -> MermanBuffer {
    if bytes.is_empty() {
        return MermanBuffer::empty();
    }
    let mut boxed = bytes.into_boxed_slice();
    let buffer = MermanBuffer {
        data: boxed.as_mut_ptr(),
        len: boxed.len(),
    };
    std::mem::forget(boxed);
    buffer
}

fn error_result(status: BindingStatus, message: &str) -> MermanResult {
    MermanResult {
        code: status.code(),
        data: buffer_from_vec(error_payload_json_bytes(status, message)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::ffi::CStr;

    fn call_render(source: &[u8], options: &[u8]) -> MermanResult {
        unsafe {
            merman_render_svg(
                source.as_ptr(),
                source.len(),
                options.as_ptr(),
                options.len(),
            )
        }
    }

    fn call_render_ascii(source: &[u8], options: &[u8]) -> MermanResult {
        unsafe {
            merman_render_ascii(
                source.as_ptr(),
                source.len(),
                options.as_ptr(),
                options.len(),
            )
        }
    }

    fn call_parse(source: &[u8], options: &[u8]) -> MermanResult {
        unsafe {
            merman_parse_json(
                source.as_ptr(),
                source.len(),
                options.as_ptr(),
                options.len(),
            )
        }
    }

    fn call_validate(source: &[u8], options: &[u8]) -> MermanResult {
        unsafe {
            merman_validate_json(
                source.as_ptr(),
                source.len(),
                options.as_ptr(),
                options.len(),
            )
        }
    }

    fn call_layout(source: &[u8], options: &[u8]) -> MermanResult {
        unsafe {
            merman_layout_json(
                source.as_ptr(),
                source.len(),
                options.as_ptr(),
                options.len(),
            )
        }
    }

    fn call_engine(options: &[u8]) -> MermanEngineResult {
        unsafe { merman_engine_new(options.as_ptr(), options.len()) }
    }

    fn call_engine_render(engine: *const MermanEngine, source: &[u8]) -> MermanResult {
        unsafe { merman_engine_render_svg(engine, source.as_ptr(), source.len()) }
    }

    fn take_buffer(buffer: MermanBuffer) -> Vec<u8> {
        if buffer.data.is_null() || buffer.len == 0 {
            return Vec::new();
        }
        let bytes = unsafe { std::slice::from_raw_parts(buffer.data, buffer.len).to_vec() };
        unsafe { merman_buffer_free(buffer) };
        bytes
    }

    fn take_text(buffer: MermanBuffer) -> String {
        String::from_utf8(take_buffer(buffer)).expect("FFI output should be UTF-8")
    }

    fn take_error(result: MermanResult) -> Value {
        serde_json::from_str(&take_text(result.data)).expect("error payload should be JSON")
    }

    fn expect_render_feature_error(result: MermanResult) {
        assert_eq!(result.code, BindingStatus::UnsupportedFormat.code());
        let error = take_error(result);
        assert_eq!(
            error["code_name"],
            BindingStatus::UnsupportedFormat.code_name()
        );
        assert!(
            error["message"]
                .as_str()
                .unwrap()
                .contains("render feature")
        );
    }

    #[test]
    fn abi_introspection_reports_contract_values() {
        assert_eq!(merman_abi_version(), MERMAN_ABI_VERSION);
        assert_eq!(
            merman_buffer_struct_size(),
            std::mem::size_of::<MermanBuffer>()
        );
        assert_eq!(
            merman_result_struct_size(),
            std::mem::size_of::<MermanResult>()
        );

        let version = unsafe { CStr::from_ptr(merman_package_version()) };
        assert_eq!(version.to_str().unwrap(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn render_svg_returns_svg_for_flowchart() {
        let result = call_render(b"flowchart TD\nA[Hello] --> B[World]", b"");

        if cfg!(feature = "render") {
            assert_eq!(result.code, BindingStatus::Ok.code());
            let svg = take_text(result.data);
            assert!(svg.contains("<svg"));
            assert!(svg.contains("Hello"));
            assert!(svg.contains("World"));
        } else {
            expect_render_feature_error(result);
        }
    }

    #[test]
    fn render_svg_accepts_options_json() {
        let options = br#"{
            "layout": { "text_measurer": "deterministic", "viewport_width": 640, "viewport_height": 480 },
            "svg": { "diagram_id": "ffi diagram", "pipeline": "readable" }
        }"#;
        let result = call_render(b"flowchart TD\nA[Hello]", options);

        if cfg!(feature = "render") {
            assert_eq!(result.code, BindingStatus::Ok.code());
            let svg = take_text(result.data);
            assert!(svg.contains("id=\"ffi-diagram\""));
            assert!(svg.contains("data-merman-foreignobject"));
        } else {
            expect_render_feature_error(result);
        }
    }

    #[test]
    fn render_ascii_returns_text_or_feature_error() {
        let result = call_render_ascii(b"flowchart TD\nA[Hello] --> B[World]", b"");

        if cfg!(feature = "ascii") {
            assert_eq!(result.code, BindingStatus::Ok.code());
            let text = take_text(result.data);
            assert!(text.contains("Hello"));
            assert!(text.contains("World"));
        } else {
            assert_eq!(result.code, BindingStatus::UnsupportedFormat.code());
            let error = take_error(result);
            assert_eq!(
                error["code_name"],
                BindingStatus::UnsupportedFormat.code_name()
            );
        }
    }

    #[test]
    fn parse_json_returns_semantic_model() {
        let result = call_parse(b"flowchart TD\nA[Hello] --> B[World]", b"");

        if cfg!(feature = "render") {
            assert_eq!(result.code, BindingStatus::Ok.code());
            let json: Value = serde_json::from_str(&take_text(result.data)).unwrap();
            assert!(json.is_object());
            assert_eq!(
                json.get("type").and_then(Value::as_str),
                Some("flowchart-v2")
            );
            assert!(json.get("nodes").and_then(Value::as_array).is_some());
            assert!(json.get("edges").and_then(Value::as_array).is_some());
        } else {
            expect_render_feature_error(result);
        }
    }

    #[test]
    fn layout_json_returns_layouted_diagram() {
        let result = call_layout(b"flowchart TD\nA[Hello] --> B[World]", b"");

        if cfg!(feature = "render") {
            assert_eq!(result.code, BindingStatus::Ok.code());
            let json: Value = serde_json::from_str(&take_text(result.data)).unwrap();
            assert!(json.get("meta").is_some());
            assert!(json.get("layout").is_some());
        } else {
            expect_render_feature_error(result);
        }
    }

    #[test]
    fn validate_json_returns_status_payload() {
        let valid = call_validate(b"flowchart TD\nA[Hello]", b"");
        assert_eq!(valid.code, BindingStatus::Ok.code());
        let json: Value = serde_json::from_str(&take_text(valid.data)).unwrap();
        if cfg!(feature = "render") {
            assert_eq!(json["valid"], true);
            assert_eq!(json["code_name"], BindingStatus::Ok.code_name());
        } else {
            assert_eq!(json["valid"], false);
            assert_eq!(
                json["code_name"],
                BindingStatus::UnsupportedFormat.code_name()
            );
        }

        let invalid = call_validate(b"", b"");
        assert_eq!(invalid.code, BindingStatus::Ok.code());
        let json: Value = serde_json::from_str(&take_text(invalid.data)).unwrap();
        assert_eq!(json["valid"], false);
        if cfg!(feature = "render") {
            assert_eq!(json["code_name"], BindingStatus::NoDiagram.code_name());
        } else {
            assert_eq!(
                json["code_name"],
                BindingStatus::UnsupportedFormat.code_name()
            );
        }
    }

    #[test]
    fn metadata_entry_points_return_json_arrays() {
        let diagrams = merman_supported_diagrams_json();
        let ascii_diagrams = merman_ascii_supported_diagrams_json();
        let themes = merman_themes_json();

        assert_eq!(diagrams.code, BindingStatus::Ok.code());
        assert_eq!(ascii_diagrams.code, BindingStatus::Ok.code());
        assert_eq!(themes.code, BindingStatus::Ok.code());

        let diagrams: Value = serde_json::from_str(&take_text(diagrams.data)).unwrap();
        let ascii_diagrams: Value = serde_json::from_str(&take_text(ascii_diagrams.data)).unwrap();
        let themes: Value = serde_json::from_str(&take_text(themes.data)).unwrap();

        assert!(
            diagrams
                .as_array()
                .unwrap()
                .contains(&Value::String("flowchart".to_string()))
        );
        assert!(ascii_diagrams.is_array());
        assert!(
            themes
                .as_array()
                .unwrap()
                .contains(&Value::String("default".to_string()))
        );
    }

    #[test]
    fn parse_json_uses_same_error_payload() {
        let result = call_parse(&[0xff], b"");

        if cfg!(feature = "render") {
            assert_eq!(result.code, BindingStatus::Utf8Error.code());
            let error = take_error(result);
            assert_eq!(error["code_name"], BindingStatus::Utf8Error.code_name());
        } else {
            expect_render_feature_error(result);
        }
    }

    #[test]
    fn null_source_with_nonzero_len_returns_invalid_argument() {
        let result = unsafe { merman_render_svg(ptr::null(), 1, ptr::null(), 0) };

        assert_eq!(result.code, BindingStatus::InvalidArgument.code());
        let error = take_error(result);
        assert_eq!(
            error["code_name"],
            BindingStatus::InvalidArgument.code_name()
        );
    }

    #[test]
    fn invalid_source_utf8_returns_utf8_error() {
        let result = call_render(&[0xff], b"");

        if cfg!(feature = "render") {
            assert_eq!(result.code, BindingStatus::Utf8Error.code());
            let error = take_error(result);
            assert_eq!(error["code_name"], BindingStatus::Utf8Error.code_name());
        } else {
            expect_render_feature_error(result);
        }
    }

    #[test]
    fn empty_source_returns_no_diagram() {
        let result = unsafe { merman_render_svg(ptr::null(), 0, ptr::null(), 0) };

        if cfg!(feature = "render") {
            assert_eq!(result.code, BindingStatus::NoDiagram.code());
            let error = take_error(result);
            assert_eq!(error["code_name"], BindingStatus::NoDiagram.code_name());
        } else {
            expect_render_feature_error(result);
        }
    }

    #[test]
    fn invalid_options_json_returns_options_json_error() {
        let result = call_render(b"flowchart TD\nA", b"{");

        if cfg!(feature = "render") {
            assert_eq!(result.code, BindingStatus::OptionsJsonError.code());
            let error = take_error(result);
            assert_eq!(
                error["code_name"],
                BindingStatus::OptionsJsonError.code_name()
            );
        } else {
            expect_render_feature_error(result);
        }
    }

    #[test]
    fn unsupported_ratex_without_feature_returns_unsupported_format() {
        let result = call_render(
            b"flowchart TD\nA[Hello]",
            br#"{ "layout": { "math_renderer": "ratex" } }"#,
        );

        if cfg!(feature = "ratex-math") {
            assert_eq!(result.code, BindingStatus::Ok.code());
            unsafe { merman_buffer_free(result.data) };
        } else {
            assert_eq!(result.code, BindingStatus::UnsupportedFormat.code());
            let error = take_error(result);
            assert_eq!(
                error["code_name"],
                BindingStatus::UnsupportedFormat.code_name()
            );
        }
    }

    #[test]
    fn buffer_free_accepts_null_buffer() {
        unsafe { merman_buffer_free(MermanBuffer::empty()) };
    }

    #[test]
    fn ffi_result_catches_panic() {
        let result = ffi_result(|| -> Result<Vec<u8>, BindingError> { panic!("boom") });

        assert_eq!(result.code, BindingStatus::Panic.code());
        let error = take_error(result);
        assert_eq!(error["code_name"], BindingStatus::Panic.code_name());
    }

    #[test]
    fn reusable_engine_renders_with_cached_options() {
        let options = br#"{
            "layout": { "text_measurer": "deterministic" },
            "svg": { "diagram_id": "ffi engine", "pipeline": "readable" }
        }"#;
        let engine = call_engine(options);
        assert_eq!(engine.code, BindingStatus::Ok.code());
        assert!(!engine.engine.is_null());
        assert!(engine.data.data.is_null());

        let result = call_engine_render(engine.engine, b"flowchart TD\nA[Hello]");
        if cfg!(feature = "render") {
            assert_eq!(result.code, BindingStatus::Ok.code());
            let svg = take_text(result.data);
            assert!(svg.contains("id=\"ffi-engine\""));
            assert!(svg.contains("data-merman-foreignobject"));
        } else {
            expect_render_feature_error(result);
        }

        unsafe { merman_engine_free(engine.engine) };
    }

    #[test]
    fn reusable_engine_reports_invalid_options_json() {
        let engine = call_engine(b"{");

        if cfg!(any(feature = "render", feature = "ascii")) {
            assert_eq!(engine.code, BindingStatus::OptionsJsonError.code());
            assert!(engine.engine.is_null());
            let error: Value = serde_json::from_str(&take_text(engine.data)).unwrap();
            assert_eq!(
                error["code_name"],
                BindingStatus::OptionsJsonError.code_name()
            );
        } else {
            assert_eq!(engine.code, BindingStatus::Ok.code());
            unsafe { merman_engine_free(engine.engine) };
        }
    }

    #[test]
    fn reusable_engine_rejects_null_engine() {
        let result = unsafe {
            merman_engine_render_svg(
                ptr::null(),
                b"flowchart TD\nA".as_ptr(),
                b"flowchart TD\nA".len(),
            )
        };

        assert_eq!(result.code, BindingStatus::InvalidArgument.code());
        let error = take_error(result);
        assert_eq!(
            error["code_name"],
            BindingStatus::InvalidArgument.code_name()
        );
        assert!(error["message"].as_str().unwrap().contains("engine"));
    }

    #[test]
    fn engine_result_struct_size_is_reported() {
        assert_eq!(
            merman_engine_result_struct_size(),
            std::mem::size_of::<MermanEngineResult>()
        );
    }

    #[test]
    fn reusable_engine_can_render_concurrently_through_c_abi() {
        let engine = call_engine(b"");
        assert_eq!(engine.code, BindingStatus::Ok.code());
        assert!(!engine.engine.is_null());
        let engine_addr = engine.engine as usize;

        let mut handles = Vec::new();
        for _ in 0..8 {
            handles.push(std::thread::spawn(move || {
                let engine = engine_addr as *const MermanEngine;
                for _ in 0..8 {
                    let result = call_engine_render(engine, b"flowchart TD\nA[Hello] --> B[World]");
                    if cfg!(feature = "render") {
                        assert_eq!(result.code, BindingStatus::Ok.code());
                        let svg = take_text(result.data);
                        assert!(svg.contains("<svg"));
                    } else {
                        expect_render_feature_error(result);
                    }
                }
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        unsafe { merman_engine_free(engine.engine) };
    }
}

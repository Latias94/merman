#![deny(unsafe_op_in_unsafe_fn)]

//! C ABI exports for embedding `merman` in non-Rust hosts.
//!
//! This crate is the only place where the public FFI boundary owns unsafe code. The core
//! parser/render crates and shared binding facade remain safe Rust APIs.

use merman_bindings_core::{BindingError, BindingStatus, error_payload_json_bytes};
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

        assert_eq!(result.code, BindingStatus::Ok.code());
        let svg = take_text(result.data);
        assert!(svg.contains("<svg"));
        assert!(svg.contains("Hello"));
        assert!(svg.contains("World"));
    }

    #[test]
    fn render_svg_accepts_options_json() {
        let options = br#"{
            "layout": { "text_measurer": "deterministic", "viewport_width": 640, "viewport_height": 480 },
            "svg": { "diagram_id": "ffi diagram", "pipeline": "readable" }
        }"#;
        let result = call_render(b"flowchart TD\nA[Hello]", options);

        assert_eq!(result.code, BindingStatus::Ok.code());
        let svg = take_text(result.data);
        assert!(svg.contains("id=\"ffi-diagram\""));
        assert!(svg.contains("data-merman-foreignobject"));
    }

    #[test]
    fn parse_json_returns_semantic_model() {
        let result = call_parse(b"flowchart TD\nA[Hello] --> B[World]", b"");

        assert_eq!(result.code, BindingStatus::Ok.code());
        let json: Value = serde_json::from_str(&take_text(result.data)).unwrap();
        assert!(json.is_object());
        assert_eq!(
            json.get("type").and_then(Value::as_str),
            Some("flowchart-v2")
        );
        assert!(json.get("nodes").and_then(Value::as_array).is_some());
        assert!(json.get("edges").and_then(Value::as_array).is_some());
    }

    #[test]
    fn layout_json_returns_layouted_diagram() {
        let result = call_layout(b"flowchart TD\nA[Hello] --> B[World]", b"");

        assert_eq!(result.code, BindingStatus::Ok.code());
        let json: Value = serde_json::from_str(&take_text(result.data)).unwrap();
        assert!(json.get("meta").is_some());
        assert!(json.get("layout").is_some());
    }

    #[test]
    fn parse_json_uses_same_error_payload() {
        let result = call_parse(&[0xff], b"");

        assert_eq!(result.code, BindingStatus::Utf8Error.code());
        let error = take_error(result);
        assert_eq!(error["code_name"], BindingStatus::Utf8Error.code_name());
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

        assert_eq!(result.code, BindingStatus::Utf8Error.code());
        let error = take_error(result);
        assert_eq!(error["code_name"], BindingStatus::Utf8Error.code_name());
    }

    #[test]
    fn empty_source_returns_no_diagram() {
        let result = unsafe { merman_render_svg(ptr::null(), 0, ptr::null(), 0) };

        assert_eq!(result.code, BindingStatus::NoDiagram.code());
        let error = take_error(result);
        assert_eq!(error["code_name"], BindingStatus::NoDiagram.code_name());
    }

    #[test]
    fn invalid_options_json_returns_options_json_error() {
        let result = call_render(b"flowchart TD\nA", b"{");

        assert_eq!(result.code, BindingStatus::OptionsJsonError.code());
        let error = take_error(result);
        assert_eq!(
            error["code_name"],
            BindingStatus::OptionsJsonError.code_name()
        );
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
}

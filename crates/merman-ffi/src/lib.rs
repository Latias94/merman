#![deny(unsafe_op_in_unsafe_fn)]

//! C ABI exports for embedding `merman` in non-Rust hosts.
//!
//! This crate is the only place where the public FFI boundary owns unsafe code. The core
//! parser/render crates and shared binding facade remain safe Rust APIs.

#[cfg(feature = "render")]
use merman_bindings_core::TextMeasurer;
use merman_bindings_core::{BindingEngine, BindingError, BindingStatus, error_payload_json_bytes};
use std::ffi::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
#[cfg(feature = "render")]
use std::sync::Arc;

#[cfg(target_os = "android")]
mod android_jni;

pub const MERMAN_ABI_VERSION: u32 = 2;

const PACKAGE_VERSION: &[u8] = concat!(env!("CARGO_PKG_VERSION"), "\0").as_bytes();

pub const MERMAN_WRAP_MODE_SVG_LIKE: i32 = 0;
pub const MERMAN_WRAP_MODE_SVG_LIKE_SINGLE_RUN: i32 = 1;
pub const MERMAN_WRAP_MODE_HTML_LIKE: i32 = 2;

pub const MERMAN_TEXT_DIRECTION_AUTO: i32 = 0;
pub const MERMAN_TEXT_DIRECTION_LTR: i32 = 1;
pub const MERMAN_TEXT_DIRECTION_RTL: i32 = 2;

pub const MERMAN_TEXT_WHITE_SPACE_NORMAL: i32 = 0;
pub const MERMAN_TEXT_WHITE_SPACE_NOWRAP: i32 = 1;
pub const MERMAN_TEXT_WHITE_SPACE_BREAK_SPACES: i32 = 2;
pub const MERMAN_TEXT_WHITE_SPACE_PRE_WRAP: i32 = 3;

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
    #[cfg(feature = "render")]
    base: BindingEngine,
    inner: BindingEngine,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MermanEngineResult {
    pub code: i32,
    pub engine: *mut MermanEngine,
    pub data: MermanBuffer,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MermanHostTextMeasureRequest {
    pub text: *const u8,
    pub text_len: usize,
    pub font_family: *const u8,
    pub font_family_len: usize,
    pub font_size: f64,
    pub font_weight: *const u8,
    pub font_weight_len: usize,
    pub font_style: *const u8,
    pub font_style_len: usize,
    pub max_width: f64,
    pub line_height: f64,
    pub letter_spacing: f64,
    pub word_spacing: f64,
    pub wrap_mode: i32,
    pub direction: i32,
    pub white_space: i32,
    pub has_max_width: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MermanHostTextMeasureResult {
    pub handled: u8,
    pub width: f64,
    pub height: f64,
    pub line_count: usize,
}

pub type MermanHostTextMeasureCallback = unsafe extern "C" fn(
    request: MermanHostTextMeasureRequest,
    user_data: *mut std::ffi::c_void,
) -> MermanHostTextMeasureResult;

#[cfg(feature = "render")]
#[derive(Clone)]
struct FfiHostTextMeasurer {
    callback: MermanHostTextMeasureCallback,
    user_data: usize,
    fallback: merman_bindings_core::VendoredFontMetricsTextMeasurer,
}

#[cfg(feature = "render")]
impl FfiHostTextMeasurer {
    const DEFAULT_FONT_STYLE: &'static [u8] = b"normal";
    const DEFAULT_FONT_WEIGHT: &'static [u8] = b"normal";

    fn new(callback: MermanHostTextMeasureCallback, user_data: *mut std::ffi::c_void) -> Self {
        Self {
            callback,
            user_data: user_data as usize,
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
        let font_family = style.font_family.as_deref().unwrap_or_default().as_bytes();
        let font_weight = style
            .font_weight
            .as_deref()
            .map(str::as_bytes)
            .unwrap_or(Self::DEFAULT_FONT_WEIGHT);
        let font_style = Self::DEFAULT_FONT_STYLE;
        let result = unsafe {
            (self.callback)(
                MermanHostTextMeasureRequest {
                    text: text.as_ptr(),
                    text_len: text.len(),
                    font_family: font_family.as_ptr(),
                    font_family_len: font_family.len(),
                    font_size: style.font_size,
                    font_weight: font_weight.as_ptr(),
                    font_weight_len: font_weight.len(),
                    font_style: font_style.as_ptr(),
                    font_style_len: font_style.len(),
                    max_width: max_width.unwrap_or(0.0),
                    line_height: ffi_line_height(style, wrap_mode),
                    letter_spacing: 0.0,
                    word_spacing: 0.0,
                    wrap_mode: ffi_wrap_mode(wrap_mode),
                    direction: MERMAN_TEXT_DIRECTION_AUTO,
                    white_space: ffi_white_space(max_width, wrap_mode),
                    has_max_width: u8::from(max_width.is_some()),
                },
                self.user_data as *mut std::ffi::c_void,
            )
        };

        if result.handled == 0
            || !result.width.is_finite()
            || !result.height.is_finite()
            || result.width < 0.0
            || result.height < 0.0
            || result.line_count == 0
        {
            return None;
        }

        Some(merman_bindings_core::TextMetrics {
            width: result.width,
            height: result.height,
            line_count: result.line_count,
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
impl merman_bindings_core::TextMeasurer for FfiHostTextMeasurer {
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

#[cfg(feature = "render")]
fn ffi_wrap_mode(wrap_mode: merman_bindings_core::WrapMode) -> i32 {
    match wrap_mode {
        merman_bindings_core::WrapMode::SvgLike => MERMAN_WRAP_MODE_SVG_LIKE,
        merman_bindings_core::WrapMode::SvgLikeSingleRun => MERMAN_WRAP_MODE_SVG_LIKE_SINGLE_RUN,
        merman_bindings_core::WrapMode::HtmlLike => MERMAN_WRAP_MODE_HTML_LIKE,
    }
}

#[cfg(feature = "render")]
fn ffi_line_height(
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
fn ffi_white_space(max_width: Option<f64>, wrap_mode: merman_bindings_core::WrapMode) -> i32 {
    match wrap_mode {
        merman_bindings_core::WrapMode::HtmlLike if max_width.is_some() => {
            MERMAN_TEXT_WHITE_SPACE_BREAK_SPACES
        }
        merman_bindings_core::WrapMode::HtmlLike => MERMAN_TEXT_WHITE_SPACE_NOWRAP,
        merman_bindings_core::WrapMode::SvgLike
        | merman_bindings_core::WrapMode::SvgLikeSingleRun => MERMAN_TEXT_WHITE_SPACE_NORMAL,
    }
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

/// Return the Rust-side size of `MermanHostTextMeasureRequest`.
#[unsafe(no_mangle)]
pub extern "C" fn merman_host_text_measure_request_struct_size() -> usize {
    std::mem::size_of::<MermanHostTextMeasureRequest>()
}

/// Return the Rust-side size of `MermanHostTextMeasureResult`.
#[unsafe(no_mangle)]
pub extern "C" fn merman_host_text_measure_result_struct_size() -> usize {
    std::mem::size_of::<MermanHostTextMeasureResult>()
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

/// Install a host-provided text measurer on a reusable engine.
///
/// The callback is used for future layout/render calls made through this engine. Passing a null
/// callback resets the engine to the measurer configured by `merman_engine_new`.
///
/// # Safety
///
/// - `engine` must be a live pointer returned by `merman_engine_new`.
/// - `callback`, when non-null, must remain callable for as long as the engine can call it.
/// - `user_data` is never dereferenced by merman; it is passed back unchanged.
/// - Callers must not mutate an engine while another thread is using it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_engine_set_text_measure_callback(
    engine: *mut MermanEngine,
    callback: Option<MermanHostTextMeasureCallback>,
    user_data: *mut std::ffi::c_void,
) -> MermanResult {
    ffi_result(|| unsafe { engine_set_text_measure_callback_impl(engine, callback, user_data) })
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
        ffi_engine_source_call(engine, source, source_len, BindingEngine::render_svg)
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
        ffi_engine_source_call(engine, source, source_len, BindingEngine::render_ascii)
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
        ffi_engine_source_call(engine, source, source_len, BindingEngine::parse_json)
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
        ffi_engine_source_call(engine, source, source_len, BindingEngine::layout_json)
    })
}

/// Analyze Mermaid source to diagnostics JSON bytes using a reusable engine.
///
/// # Safety
///
/// Safety rules are identical to `merman_engine_render_svg`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_engine_analyze_json(
    engine: *const MermanEngine,
    source: *const u8,
    source_len: usize,
) -> MermanResult {
    ffi_result(|| unsafe {
        ffi_engine_source_call(engine, source, source_len, BindingEngine::analyze_json)
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
        ffi_engine_source_call(engine, source, source_len, BindingEngine::validate_json)
    })
}

/// Analyze Mermaid source to diagnostics JSON bytes.
///
/// # Safety
///
/// - `source` may be null only when `source_len == 0`.
/// - `options_json` may be null only when `options_len == 0`.
/// - Non-null pointers must be valid for reads of their paired length for the duration of the call.
/// - Returned non-empty buffers must be released with `merman_buffer_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_analyze_json(
    source: *const u8,
    source_len: usize,
    options_json: *const u8,
    options_len: usize,
) -> MermanResult {
    ffi_result(|| unsafe {
        ffi_source_options_call(
            source,
            source_len,
            options_json,
            options_len,
            merman_bindings_core::analyze_json,
        )
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
    ffi_result(|| unsafe {
        ffi_source_options_call(
            source,
            source_len,
            options_json,
            options_len,
            merman_bindings_core::render_svg,
        )
    })
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
    ffi_result(|| unsafe {
        ffi_source_options_call(
            source,
            source_len,
            options_json,
            options_len,
            merman_bindings_core::render_ascii,
        )
    })
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
    ffi_result(|| unsafe {
        ffi_source_options_call(
            source,
            source_len,
            options_json,
            options_len,
            merman_bindings_core::parse_json,
        )
    })
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
    ffi_result(|| unsafe {
        ffi_source_options_call(
            source,
            source_len,
            options_json,
            options_len,
            merman_bindings_core::layout_json,
        )
    })
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
    ffi_result(|| unsafe {
        ffi_source_options_call(
            source,
            source_len,
            options_json,
            options_len,
            merman_bindings_core::validate_json,
        )
    })
}

/// Return supported diagram type metadata as a JSON string array.
#[unsafe(no_mangle)]
pub extern "C" fn merman_supported_diagrams_json() -> MermanResult {
    ffi_result(merman_bindings_core::supported_diagrams_json)
}

/// Return ASCII rendering capability metadata as a JSON array.
#[unsafe(no_mangle)]
pub extern "C" fn merman_ascii_capabilities_json() -> MermanResult {
    ffi_result(merman_bindings_core::ascii_capabilities_json)
}

/// Return diagram family parser/render capability metadata as a JSON array.
#[unsafe(no_mangle)]
pub extern "C" fn merman_diagram_family_capabilities_json() -> MermanResult {
    ffi_result(merman_bindings_core::diagram_family_capabilities_json)
}

/// Return lint rule catalog metadata as a JSON array.
#[unsafe(no_mangle)]
pub extern "C" fn merman_lint_rule_catalog_json() -> MermanResult {
    ffi_result(merman_bindings_core::lint_rule_catalog_json)
}

/// Return supported theme metadata as a JSON string array.
#[unsafe(no_mangle)]
pub extern "C" fn merman_supported_themes_json() -> MermanResult {
    ffi_result(merman_bindings_core::supported_themes_json)
}

/// Return supported host/editor theme preset metadata as a JSON string array.
#[unsafe(no_mangle)]
pub extern "C" fn merman_supported_host_theme_presets_json() -> MermanResult {
    ffi_result(merman_bindings_core::supported_host_theme_presets_json)
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
            engine: Box::into_raw(Box::new(MermanEngine {
                #[cfg(feature = "render")]
                base: inner.clone(),
                inner,
            })),
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

#[cfg(feature = "render")]
unsafe fn engine_mut<'a>(engine: *mut MermanEngine) -> Result<&'a mut MermanEngine, BindingError> {
    if engine.is_null() {
        return Err(BindingError::new(
            BindingStatus::InvalidArgument,
            "engine pointer is null",
        ));
    }
    Ok(unsafe { &mut *engine })
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

unsafe fn engine_set_text_measure_callback_impl(
    engine: *mut MermanEngine,
    callback: Option<MermanHostTextMeasureCallback>,
    user_data: *mut std::ffi::c_void,
) -> Result<Vec<u8>, BindingError> {
    #[cfg(not(feature = "render"))]
    {
        let _ = (engine, callback, user_data);
        return Err(BindingError::new(
            BindingStatus::UnsupportedFormat,
            "host text measurement requires the render feature",
        ));
    }

    #[cfg(feature = "render")]
    {
        let engine = unsafe { engine_mut(engine)? };
        if let Some(callback) = callback {
            let measurer = FfiHostTextMeasurer::new(callback, user_data);
            engine.inner = engine.inner.with_text_measurer(Arc::new(measurer));
        } else {
            engine.inner = engine.base.clone();
        }
        Ok(Vec::new())
    }
}

unsafe fn ffi_engine_source_call<F>(
    engine: *const MermanEngine,
    source: *const u8,
    source_len: usize,
    f: F,
) -> Result<Vec<u8>, BindingError>
where
    F: FnOnce(&BindingEngine, &[u8]) -> Result<Vec<u8>, BindingError>,
{
    let engine = unsafe { engine_ref(engine)? };
    let source_bytes = unsafe { raw_bytes(source, source_len, "source")? };
    f(&engine.inner, source_bytes)
}

unsafe fn ffi_source_options_call<F>(
    source: *const u8,
    source_len: usize,
    options_json: *const u8,
    options_len: usize,
    f: F,
) -> Result<Vec<u8>, BindingError>
where
    F: FnOnce(&[u8], &[u8]) -> Result<Vec<u8>, BindingError>,
{
    let request = unsafe {
        FfiSourceOptionsRequest::from_raw(source, source_len, options_json, options_len)?
    };
    f(request.source, request.options_json)
}

struct FfiSourceOptionsRequest<'a> {
    source: &'a [u8],
    options_json: &'a [u8],
}

impl<'a> FfiSourceOptionsRequest<'a> {
    unsafe fn from_raw(
        source: *const u8,
        source_len: usize,
        options_json: *const u8,
        options_len: usize,
    ) -> Result<Self, BindingError> {
        let source = unsafe { raw_bytes(source, source_len, "source")? };
        let options_json = unsafe { raw_bytes(options_json, options_len, "options_json")? };
        Ok(Self {
            source,
            options_json,
        })
    }
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

    fn call_analyze(source: &[u8], options: &[u8]) -> MermanResult {
        unsafe {
            merman_analyze_json(
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

    fn foreign_object_width_before_label(svg: &str, label: &str) -> f64 {
        let label_start = svg.find(label).expect("label text");
        let before_label = &svg[..label_start];
        let width_marker = r#"<foreignObject width=""#;
        let width_start = before_label
            .rfind(width_marker)
            .map(|idx| idx + width_marker.len())
            .expect("foreignObject width marker");
        let width_end = svg[width_start..]
            .find('"')
            .map(|idx| width_start + idx)
            .expect("foreignObject width end");
        svg[width_start..width_end]
            .parse::<f64>()
            .expect("foreignObject width number")
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
        assert_eq!(
            merman_host_text_measure_request_struct_size(),
            std::mem::size_of::<MermanHostTextMeasureRequest>()
        );
        assert_eq!(
            merman_host_text_measure_result_struct_size(),
            std::mem::size_of::<MermanHostTextMeasureResult>()
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
        assert_eq!(json["valid"], true);
        assert_eq!(json["code_name"], BindingStatus::Ok.code_name());

        let invalid = call_validate(b"", b"");
        assert_eq!(invalid.code, BindingStatus::Ok.code());
        let json: Value = serde_json::from_str(&take_text(invalid.data)).unwrap();
        assert_eq!(json["valid"], false);
        assert_eq!(json["code_name"], BindingStatus::NoDiagram.code_name());
    }

    #[test]
    fn analyze_json_returns_diagnostics_payload() {
        let valid = call_analyze(b"flowchart TD\nA[Hello]", b"");
        assert_eq!(valid.code, BindingStatus::Ok.code());
        let json: Value = serde_json::from_str(&take_text(valid.data)).unwrap();
        assert_eq!(json["version"], 1);
        assert_eq!(json["valid"], true);
        assert_eq!(json["summary"]["errors"], 0);

        let invalid = call_analyze(b"", b"");
        assert_eq!(invalid.code, BindingStatus::Ok.code());
        let json: Value = serde_json::from_str(&take_text(invalid.data)).unwrap();
        assert_eq!(json["version"], 1);
        assert_eq!(json["valid"], false);
        assert_eq!(json["diagnostics"][0]["code_name"], "MERMAN_NO_DIAGRAM");
    }

    #[test]
    fn metadata_entry_points_return_json_arrays() {
        let diagrams = merman_supported_diagrams_json();
        let ascii_capabilities = merman_ascii_capabilities_json();
        let family_capabilities = merman_diagram_family_capabilities_json();
        let lint_rules = merman_lint_rule_catalog_json();
        let themes = merman_supported_themes_json();
        let host_theme_presets = merman_supported_host_theme_presets_json();

        assert_eq!(diagrams.code, BindingStatus::Ok.code());
        assert_eq!(ascii_capabilities.code, BindingStatus::Ok.code());
        assert_eq!(family_capabilities.code, BindingStatus::Ok.code());
        assert_eq!(lint_rules.code, BindingStatus::Ok.code());
        assert_eq!(themes.code, BindingStatus::Ok.code());
        assert_eq!(host_theme_presets.code, BindingStatus::Ok.code());

        let diagrams: Value = serde_json::from_str(&take_text(diagrams.data)).unwrap();
        let ascii_capabilities: Value =
            serde_json::from_str(&take_text(ascii_capabilities.data)).unwrap();
        let family_capabilities: Value =
            serde_json::from_str(&take_text(family_capabilities.data)).unwrap();
        let lint_rules: Value = serde_json::from_str(&take_text(lint_rules.data)).unwrap();
        let themes: Value = serde_json::from_str(&take_text(themes.data)).unwrap();
        let host_theme_presets: Value =
            serde_json::from_str(&take_text(host_theme_presets.data)).unwrap();

        assert!(
            diagrams
                .as_array()
                .unwrap()
                .contains(&Value::String("flowchart".to_string()))
        );
        let ascii_capabilities = ascii_capabilities.as_array().unwrap();
        if cfg!(feature = "ascii") {
            let sequence = ascii_capabilities
                .iter()
                .find(|capability| capability["diagram_type"] == "sequence")
                .expect("expected ASCII capability metadata to include sequence");
            assert_eq!(sequence["support_level"], "full");

            let gantt = ascii_capabilities
                .iter()
                .find(|capability| capability["diagram_type"] == "gantt")
                .expect("expected ASCII capability metadata to include gantt");
            assert_eq!(gantt["support_level"], "summary");

            let class = ascii_capabilities
                .iter()
                .find(|capability| capability["diagram_type"] == "class")
                .expect("expected ASCII capability metadata to include class");
            assert_eq!(class["summary_fallback"], true);
        } else {
            assert!(ascii_capabilities.is_empty());
        }
        assert!(family_capabilities.as_array().unwrap().iter().any(
            |capability| capability["diagram_type"] == "flowchart"
                && capability["metadata_id"] == "flowchart"
                && capability["has_semantic_parser"] == true
                && capability["has_render_parser"] == true
        ));
        assert!(lint_rules.as_array().unwrap().iter().any(|rule| {
            rule["id"] == "merman.authoring.config.prefer_init_directive"
                && rule["origin"] == "merman_authoring"
                && rule["evidence"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|value| value == "docs/adr/0072-lint-rule-governance.md")
        }));
        assert!(
            themes
                .as_array()
                .unwrap()
                .contains(&Value::String("default".to_string()))
        );
        assert!(host_theme_presets.is_array());
        if cfg!(feature = "render") {
            assert!(
                host_theme_presets
                    .as_array()
                    .unwrap()
                    .contains(&Value::String("one-dark".to_string()))
            );
        }
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
    fn render_resource_limit_error_uses_dedicated_status() {
        let result = call_render(
            b"flowchart TD\nA[Hello]",
            br#"{ "resources": { "max_source_bytes": 4 } }"#,
        );

        if cfg!(feature = "render") {
            assert_eq!(result.code, BindingStatus::ResourceLimitExceeded.code());
            let error = take_error(result);
            assert_eq!(
                error["code_name"],
                BindingStatus::ResourceLimitExceeded.code_name()
            );
            assert!(
                error["message"]
                    .as_str()
                    .unwrap()
                    .contains("max_source_bytes")
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
    fn ffi_source_options_request_decodes_source_and_null_options() {
        let source = b"flowchart TD\nA[Hello]";
        let request = unsafe {
            FfiSourceOptionsRequest::from_raw(source.as_ptr(), source.len(), ptr::null(), 0)
        }
        .unwrap();

        assert_eq!(request.source, source);
        assert!(request.options_json.is_empty());
    }

    #[test]
    fn ffi_engine_source_call_decodes_engine_and_source() {
        let base = BindingEngine::new(b"").unwrap();
        let engine = MermanEngine {
            #[cfg(feature = "render")]
            base: base.clone(),
            inner: base,
        };
        let source = b"flowchart TD\nA[Hello]";
        let output = unsafe {
            ffi_engine_source_call(&engine, source.as_ptr(), source.len(), |_engine, source| {
                Ok(source.to_vec())
            })
        }
        .unwrap();

        assert_eq!(output, source);
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
    fn reusable_engine_can_use_host_text_measure_callback() {
        #[derive(Default)]
        struct CallbackProbe {
            saw_condition: bool,
            saw_nowrap: bool,
            saw_break_spaces: bool,
            saw_font_style: bool,
            saw_spacing_defaults: bool,
        }

        unsafe extern "C" fn measure_condition(
            request: MermanHostTextMeasureRequest,
            user_data: *mut std::ffi::c_void,
        ) -> MermanHostTextMeasureResult {
            if user_data.is_null() {
                return MermanHostTextMeasureResult {
                    handled: 0,
                    width: 0.0,
                    height: 0.0,
                    line_count: 0,
                };
            }
            let text = unsafe { std::slice::from_raw_parts(request.text, request.text_len) };
            if text == b"Condition?" && request.wrap_mode == MERMAN_WRAP_MODE_HTML_LIKE {
                let probe = unsafe { &mut *(user_data.cast::<CallbackProbe>()) };
                probe.saw_condition = true;
                let font_style = unsafe {
                    std::slice::from_raw_parts(request.font_style, request.font_style_len)
                };
                probe.saw_font_style |= font_style == b"normal"
                    && request.direction == MERMAN_TEXT_DIRECTION_AUTO
                    && request.line_height > request.font_size;
                probe.saw_spacing_defaults |=
                    request.letter_spacing == 0.0 && request.word_spacing == 0.0;
                if request.has_max_width == 0 {
                    probe.saw_nowrap |= request.white_space == MERMAN_TEXT_WHITE_SPACE_NOWRAP;
                } else {
                    probe.saw_break_spaces |=
                        request.white_space == MERMAN_TEXT_WHITE_SPACE_BREAK_SPACES;
                }
                return MermanHostTextMeasureResult {
                    handled: 1,
                    width: 140.0,
                    height: 24.0,
                    line_count: 1,
                };
            }
            MermanHostTextMeasureResult {
                handled: 0,
                width: 0.0,
                height: 0.0,
                line_count: 0,
            }
        }

        let engine = call_engine(b"");
        assert_eq!(engine.code, BindingStatus::Ok.code());
        assert!(!engine.engine.is_null());
        let source = b"flowchart TD\nA[Start] --> B{Condition?}";

        let baseline = call_engine_render(engine.engine, source);
        if !cfg!(feature = "render") {
            expect_render_feature_error(baseline);
            unsafe { merman_engine_free(engine.engine) };
            return;
        }
        assert_eq!(baseline.code, BindingStatus::Ok.code());
        let baseline_svg = take_text(baseline.data);
        let baseline_width = foreign_object_width_before_label(&baseline_svg, "Condition?");

        let mut callback_probe = CallbackProbe::default();
        let set_result = unsafe {
            merman_engine_set_text_measure_callback(
                engine.engine,
                Some(measure_condition),
                (&mut callback_probe as *mut CallbackProbe).cast(),
            )
        };
        assert_eq!(set_result.code, BindingStatus::Ok.code());
        assert!(set_result.data.data.is_null());

        let measured = call_engine_render(engine.engine, source);
        assert_eq!(measured.code, BindingStatus::Ok.code());
        let measured_svg = take_text(measured.data);
        let measured_width = foreign_object_width_before_label(&measured_svg, "Condition?");
        assert!(
            measured_width > baseline_width + 40.0,
            "expected host callback width to affect layout; baseline={baseline_width}, measured={measured_width}"
        );
        assert!(callback_probe.saw_condition);
        assert!(callback_probe.saw_nowrap);
        assert!(callback_probe.saw_break_spaces);
        assert!(callback_probe.saw_font_style);
        assert!(callback_probe.saw_spacing_defaults);

        let reset = unsafe {
            merman_engine_set_text_measure_callback(engine.engine, None, ptr::null_mut())
        };
        assert_eq!(reset.code, BindingStatus::Ok.code());

        let reset_result = call_engine_render(engine.engine, source);
        assert_eq!(reset_result.code, BindingStatus::Ok.code());
        let reset_svg = take_text(reset_result.data);
        let reset_width = foreign_object_width_before_label(&reset_svg, "Condition?");
        assert!(
            (reset_width - baseline_width).abs() < 0.001,
            "expected null callback to restore base text measurer; baseline={baseline_width}, reset={reset_width}"
        );

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

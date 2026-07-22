#![deny(unsafe_op_in_unsafe_fn)]

//! C ABI exports for embedding `merman` in non-Rust hosts.
//!
//! This crate is the only place where the public FFI boundary owns unsafe code. The core
//! parser/render crates and shared binding facade remain safe Rust APIs.

use merman_bindings_core::{BindingEngine, BindingError, BindingStatus, error_payload_json_bytes};
use std::collections::BTreeMap;
use std::ffi::c_char;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, RwLock, TryLockError};

fn binding_engine_for_transport(options_json: &[u8]) -> Result<BindingEngine, BindingError> {
    #[cfg(all(
        feature = "system-clock",
        feature = "system-timezone",
        feature = "system-random"
    ))]
    {
        BindingEngine::try_native(options_json)
    }
    #[cfg(not(all(
        feature = "system-clock",
        feature = "system-timezone",
        feature = "system-random"
    )))]
    {
        BindingEngine::new(options_json)
    }
}

fn transport_render_svg(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    binding_engine_for_transport(options_json)?.render_svg(source)
}

fn transport_render_ascii(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    binding_engine_for_transport(options_json)?.render_ascii(source)
}

fn transport_parse_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    binding_engine_for_transport(options_json)?.parse_json(source)
}

fn transport_layout_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    binding_engine_for_transport(options_json)?.layout_json(source)
}

fn transport_analyze_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    binding_engine_for_transport(options_json)?.analyze_json(source)
}

fn transport_analyze_document_json(
    source: &[u8],
    options_json: &[u8],
    uri: &[u8],
) -> Result<Vec<u8>, BindingError> {
    binding_engine_for_transport(options_json)?.analyze_document_json(source, uri)
}

fn transport_analyze_document_facts_json(
    source: &[u8],
    options_json: &[u8],
    uri: &[u8],
) -> Result<Vec<u8>, BindingError> {
    binding_engine_for_transport(options_json)?.analyze_document_facts_json(source, uri)
}

fn transport_validate_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    binding_engine_for_transport(options_json)?.validate_json(source)
}

#[cfg(target_os = "android")]
mod android_jni;

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

pub const MERMAN_TEXT_MEASUREMENT_PHASE_LAYOUT: i32 = 0;
pub const MERMAN_TEXT_MEASUREMENT_PHASE_WRAP: i32 = 1;
pub const MERMAN_TEXT_MEASUREMENT_PHASE_SVG_BBOX: i32 = 2;
pub const MERMAN_TEXT_MEASUREMENT_PHASE_COMPUTED_LENGTH: i32 = 3;

include!("generated/text_measurement_abi.rs");
include!("generated/resource_contract.rs");

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

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MermanResourceLimitOverride {
    pub id: i32,
    pub value: usize,
}

// C receives a pointer-shaped opaque token. This type is never allocated or dereferenced.
#[repr(C)]
pub struct MermanEngine {
    _private: [u8; 0],
}

struct FfiEngineState {
    #[cfg(feature = "render")]
    base: BindingEngine,
    inner: RwLock<BindingEngine>,
}

#[derive(Default)]
struct FfiEngineRegistry {
    last_token: usize,
    engines: BTreeMap<usize, Arc<FfiEngineState>>,
}

impl FfiEngineRegistry {
    fn register(&mut self, engine: Arc<FfiEngineState>) -> Result<*mut MermanEngine, BindingError> {
        let token = self.last_token.checked_add(1).ok_or_else(|| {
            BindingError::new(
                BindingStatus::InternalError,
                "engine handle token space is exhausted",
            )
        })?;
        self.last_token = token;
        let previous = self.engines.insert(token, engine);
        debug_assert!(previous.is_none(), "engine handle tokens are never reused");
        Ok(ptr::without_provenance_mut(token))
    }

    fn acquire(&self, handle: usize) -> Option<Arc<FfiEngineState>> {
        self.engines.get(&handle).map(Arc::clone)
    }

    fn retire(&mut self, handle: usize) -> Option<Arc<FfiEngineState>> {
        self.engines.remove(&handle)
    }
}

static ENGINE_REGISTRY: OnceLock<Mutex<FfiEngineRegistry>> = OnceLock::new();

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
    pub phase: i32,
    pub operation: i32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MermanHostTextMeasureResult {
    pub handled: u8,
    pub has_raw_width: u8,
    pub result_kind: i32,
    pub width: f64,
    pub height: f64,
    pub length: f64,
    pub bbox_left: f64,
    pub bbox_right: f64,
    pub raw_width: f64,
    pub line_count: usize,
}

#[cfg(test)]
impl MermanHostTextMeasureResult {
    const fn unhandled() -> Self {
        Self {
            handled: 0,
            has_raw_width: 0,
            result_kind: MERMAN_TEXT_MEASUREMENT_RESULT_KIND_METRICS,
            width: 0.0,
            height: 0.0,
            length: 0.0,
            bbox_left: 0.0,
            bbox_right: 0.0,
            raw_width: 0.0,
            line_count: 0,
        }
    }
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
}

#[cfg(feature = "render")]
impl FfiHostTextMeasurer {
    const DEFAULT_FONT_STYLE: &'static [u8] = b"normal";
    const DEFAULT_FONT_WEIGHT: &'static [u8] = b"normal";

    fn new(callback: MermanHostTextMeasureCallback, user_data: *mut std::ffi::c_void) -> Self {
        Self {
            callback,
            user_data: user_data as usize,
        }
    }

    fn call_host(
        &self,
        request: merman_bindings_core::HostTextMeasurementRequest<'_>,
    ) -> merman_bindings_core::HostMeasurementResult {
        let text = request.text;
        let style = request.style;
        let max_width = request.max_width;
        let wrap_mode = request.wrap_mode;
        let font_family = style.font_family.as_deref().unwrap_or_default().as_bytes();
        let font_weight = style
            .font_weight
            .as_deref()
            .map(str::as_bytes)
            .unwrap_or(Self::DEFAULT_FONT_WEIGHT);
        let font_style = style
            .font_style
            .as_deref()
            .map(str::as_bytes)
            .unwrap_or(Self::DEFAULT_FONT_STYLE);
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
                    phase: ffi_measurement_phase(request.phase),
                    operation: request.operation.external_code(),
                },
                self.user_data as *mut std::ffi::c_void,
            )
        };

        if result.handled == 0 {
            return Ok(None);
        }

        Ok(Some(
            merman_bindings_core::host_text_measurement_from_values(
                merman_bindings_core::HostTextMeasurementResultKind::from_external_code(
                    result.result_kind,
                ),
                merman_bindings_core::HostTextMeasurementValues {
                    width: result.width,
                    height: result.height,
                    line_count: result.line_count,
                    length: result.length,
                    bbox_left: result.bbox_left,
                    bbox_right: result.bbox_right,
                    raw_width: (result.has_raw_width != 0).then_some(result.raw_width),
                },
            ),
        ))
    }
}

#[cfg(feature = "render")]
impl merman_bindings_core::HostTextMeasurer for FfiHostTextMeasurer {
    fn measure(
        &self,
        request: merman_bindings_core::HostTextMeasurementRequest<'_>,
    ) -> merman_bindings_core::HostMeasurementResult {
        self.call_host(request)
    }
}

#[cfg(feature = "render")]
fn ffi_measurement_phase(phase: merman_bindings_core::TextMeasurementPhase) -> i32 {
    match phase {
        merman_bindings_core::TextMeasurementPhase::Layout => MERMAN_TEXT_MEASUREMENT_PHASE_LAYOUT,
        merman_bindings_core::TextMeasurementPhase::Wrap => MERMAN_TEXT_MEASUREMENT_PHASE_WRAP,
        merman_bindings_core::TextMeasurementPhase::SvgBBox => {
            MERMAN_TEXT_MEASUREMENT_PHASE_SVG_BBOX
        }
        merman_bindings_core::TextMeasurementPhase::ComputedLength => {
            MERMAN_TEXT_MEASUREMENT_PHASE_COMPUTED_LENGTH
        }
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

/// Return the versioned runtime contract as UTF-8 JSON.
#[unsafe(no_mangle)]
pub extern "C" fn merman_runtime_contract_json() -> MermanResult {
    ffi_result(|| merman_bindings_core::runtime_contract_json(MERMAN_ABI_VERSION))
}

/// Build versioned options JSON from typed resource-profile and limit identifiers.
///
/// # Safety
///
/// `overrides` may be null only when `overrides_len == 0`; otherwise it must point to a readable
/// contiguous array of `MermanResourceLimitOverride` values for the duration of this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_resource_options_json(
    profile: i32,
    overrides: *const MermanResourceLimitOverride,
    overrides_len: usize,
) -> MermanResult {
    ffi_result(|| unsafe { resource_options_json_impl(profile, overrides, overrides_len) })
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

/// Return the Rust-side size of `MermanResourceLimitOverride`.
#[unsafe(no_mangle)]
pub extern "C" fn merman_resource_limit_override_struct_size() -> usize {
    std::mem::size_of::<MermanResourceLimitOverride>()
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
/// - A returned non-null engine handle must be released with `merman_engine_free`.
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
/// Non-null engine handles must have been returned by this crate. Calling this function consumes
/// the host handle. If a call is active, state destruction is deferred until its lease ends;
/// callers must not use the handle again after requesting release.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_engine_free(engine: *mut MermanEngine) {
    retire_engine_handle(engine);
}

/// Install a host-provided text measurer on a reusable engine.
///
/// The callback is used for future layout/render calls made through this engine. Passing a null
/// callback resets the engine to the measurer configured by `merman_engine_new`.
///
/// # Safety
///
/// - `engine` must be a live handle returned by `merman_engine_new`.
/// - `callback`, when non-null, must remain callable for as long as the engine can call it.
/// - `user_data` is never dereferenced by merman; it is passed back unchanged.
/// - Mutating the callback while any call or callback is active returns `MERMAN_INVALID_ARGUMENT`.
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

/// Analyze a host document to diagnostics JSON bytes using a reusable engine.
///
/// # Safety
///
/// - `engine` must be a live pointer returned by `merman_engine_new`.
/// - `source` and `uri` may be null only when their paired length is zero.
/// - Non-null pointers must be valid for reads of their paired length for the duration of the call.
/// - Returned non-empty buffers must be released with `merman_buffer_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_engine_analyze_document_json(
    engine: *const MermanEngine,
    source: *const u8,
    source_len: usize,
    uri: *const u8,
    uri_len: usize,
) -> MermanResult {
    ffi_result(|| unsafe {
        ffi_engine_source_uri_call(
            engine,
            source,
            source_len,
            uri,
            uri_len,
            BindingEngine::analyze_document_json,
        )
    })
}

/// Analyze a host document to syntax/facts JSON bytes using a reusable engine.
///
/// # Safety
///
/// Safety rules are identical to `merman_engine_analyze_document_json`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_engine_analyze_document_facts_json(
    engine: *const MermanEngine,
    source: *const u8,
    source_len: usize,
    uri: *const u8,
    uri_len: usize,
) -> MermanResult {
    ffi_result(|| unsafe {
        ffi_engine_source_uri_call(
            engine,
            source,
            source_len,
            uri,
            uri_len,
            BindingEngine::analyze_document_facts_json,
        )
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
            transport_analyze_json,
        )
    })
}

/// Analyze a host document to diagnostics JSON bytes.
///
/// # Safety
///
/// - `source` and `uri` may be null only when their paired length is zero.
/// - `options_json` may be null only when `options_len == 0`.
/// - Non-null pointers must be valid for reads of their paired length for the duration of the call.
/// - Returned non-empty buffers must be released with `merman_buffer_free`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_analyze_document_json(
    source: *const u8,
    source_len: usize,
    options_json: *const u8,
    options_len: usize,
    uri: *const u8,
    uri_len: usize,
) -> MermanResult {
    ffi_result(|| unsafe {
        ffi_source_options_uri_call(
            source,
            source_len,
            options_json,
            options_len,
            uri,
            uri_len,
            transport_analyze_document_json,
        )
    })
}

/// Analyze a host document to syntax/facts JSON bytes.
///
/// # Safety
///
/// Safety rules are identical to `merman_analyze_document_json`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn merman_analyze_document_facts_json(
    source: *const u8,
    source_len: usize,
    options_json: *const u8,
    options_len: usize,
    uri: *const u8,
    uri_len: usize,
) -> MermanResult {
    ffi_result(|| unsafe {
        ffi_source_options_uri_call(
            source,
            source_len,
            options_json,
            options_len,
            uri,
            uri_len,
            transport_analyze_document_facts_json,
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
            transport_render_svg,
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
            transport_render_ascii,
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
            transport_parse_json,
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
            transport_layout_json,
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
            transport_validate_json,
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

/// Return the complete diagram family capability catalog as a JSON array.
#[unsafe(no_mangle)]
pub extern "C" fn merman_diagram_family_capabilities_json() -> MermanResult {
    ffi_result(merman_bindings_core::diagram_family_capabilities_json)
}

/// Return lint rule catalog metadata as a versioned JSON response object.
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
    match catch_unwind(AssertUnwindSafe(|| f().and_then(register_engine))) {
        Ok(Ok(engine)) => MermanEngineResult {
            code: BindingStatus::Ok.code(),
            engine,
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
    binding_engine_for_transport(options_bytes)
}

fn engine_registry() -> &'static Mutex<FfiEngineRegistry> {
    ENGINE_REGISTRY.get_or_init(|| Mutex::new(FfiEngineRegistry::default()))
}

fn lock_engine_registry() -> MutexGuard<'static, FfiEngineRegistry> {
    engine_registry()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn register_engine(inner: BindingEngine) -> Result<*mut MermanEngine, BindingError> {
    let engine = Arc::new(FfiEngineState {
        #[cfg(feature = "render")]
        base: inner.clone(),
        inner: RwLock::new(inner),
    });
    lock_engine_registry().register(engine)
}

fn acquire_engine_lease(engine: *const MermanEngine) -> Result<Arc<FfiEngineState>, BindingError> {
    if engine.is_null() {
        return Err(BindingError::new(
            BindingStatus::InvalidArgument,
            "engine handle is null",
        ));
    }
    lock_engine_registry()
        .acquire(engine.addr())
        .ok_or_else(|| {
            BindingError::new(
                BindingStatus::InvalidArgument,
                "engine handle is unknown or was already freed",
            )
        })
}

fn retire_engine_handle(engine: *mut MermanEngine) {
    if engine.is_null() {
        return;
    }
    let retired = lock_engine_registry().retire(engine.addr());
    drop(retired);
}

fn with_engine_read<T>(
    engine: *const MermanEngine,
    f: impl FnOnce(&BindingEngine) -> Result<T, BindingError>,
) -> Result<T, BindingError> {
    let engine = acquire_engine_lease(engine)?;
    let inner = match engine.inner.try_read() {
        Ok(inner) => inner,
        Err(TryLockError::WouldBlock) => {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                "engine cannot be used during an active mutation",
            ));
        }
        Err(TryLockError::Poisoned(_)) => {
            return Err(BindingError::new(
                BindingStatus::InternalError,
                "engine state lock is poisoned",
            ));
        }
    };
    f(&inner)
}

unsafe fn engine_set_text_measure_callback_impl(
    engine: *mut MermanEngine,
    callback: Option<MermanHostTextMeasureCallback>,
    user_data: *mut std::ffi::c_void,
) -> Result<Vec<u8>, BindingError> {
    #[cfg(not(feature = "render"))]
    {
        let _ = (engine, callback, user_data);
        Err(BindingError::new(
            BindingStatus::UnsupportedFormat,
            "host text measurement requires the render feature",
        ))
    }

    #[cfg(feature = "render")]
    {
        let engine = acquire_engine_lease(engine)?;
        let mut inner = match engine.inner.try_write() {
            Ok(inner) => inner,
            Err(TryLockError::WouldBlock) => {
                return Err(BindingError::new(
                    BindingStatus::InvalidArgument,
                    "engine cannot be mutated during an active call",
                ));
            }
            Err(TryLockError::Poisoned(_)) => {
                return Err(BindingError::new(
                    BindingStatus::InternalError,
                    "engine state lock is poisoned",
                ));
            }
        };
        if let Some(callback) = callback {
            let measurer = FfiHostTextMeasurer::new(callback, user_data);
            *inner = inner.with_host_text_measurer(Arc::new(measurer));
        } else {
            *inner = engine.base.clone();
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
    with_engine_read(engine, |inner| {
        let source_bytes = unsafe { raw_bytes(source, source_len, "source")? };
        f(inner, source_bytes)
    })
}

unsafe fn ffi_engine_source_uri_call<F>(
    engine: *const MermanEngine,
    source: *const u8,
    source_len: usize,
    uri: *const u8,
    uri_len: usize,
    f: F,
) -> Result<Vec<u8>, BindingError>
where
    F: FnOnce(&BindingEngine, &[u8], &[u8]) -> Result<Vec<u8>, BindingError>,
{
    with_engine_read(engine, |inner| {
        let source_bytes = unsafe { raw_bytes(source, source_len, "source")? };
        let uri_bytes = unsafe { raw_bytes(uri, uri_len, "uri")? };
        f(inner, source_bytes, uri_bytes)
    })
}

#[cfg(any(feature = "render", feature = "analysis", feature = "ascii"))]
unsafe fn resource_options_json_impl(
    profile: i32,
    overrides: *const MermanResourceLimitOverride,
    overrides_len: usize,
) -> Result<Vec<u8>, BindingError> {
    let profile = profile_id(profile).ok_or_else(|| {
        BindingError::new(
            BindingStatus::InvalidArgument,
            format!("unknown resource profile code: {profile}"),
        )
    })?;
    let overrides = if overrides_len == 0 {
        &[]
    } else {
        if overrides.is_null() {
            return Err(BindingError::new(
                BindingStatus::InvalidArgument,
                "resource overrides pointer is null while overrides_len is non-zero",
            ));
        }
        unsafe { std::slice::from_raw_parts(overrides, overrides_len) }
    };
    let overrides = overrides
        .iter()
        .map(|override_| {
            let id = limit_id(override_.id).ok_or_else(|| {
                BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("unknown resource limit code: {}", override_.id),
                )
            })?;
            Ok((id, override_.value))
        })
        .collect::<Result<Vec<_>, BindingError>>()?;
    merman_bindings_core::resource_options_json(profile, &overrides)
}

#[cfg(not(any(feature = "render", feature = "analysis", feature = "ascii")))]
unsafe fn resource_options_json_impl(
    profile: i32,
    overrides: *const MermanResourceLimitOverride,
    overrides_len: usize,
) -> Result<Vec<u8>, BindingError> {
    let _ = (profile, overrides, overrides_len);
    Err(merman_bindings_core::render_resource_options_unavailable())
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

unsafe fn ffi_source_options_uri_call<F>(
    source: *const u8,
    source_len: usize,
    options_json: *const u8,
    options_len: usize,
    uri: *const u8,
    uri_len: usize,
    f: F,
) -> Result<Vec<u8>, BindingError>
where
    F: FnOnce(&[u8], &[u8], &[u8]) -> Result<Vec<u8>, BindingError>,
{
    let request = unsafe {
        FfiSourceOptionsUriRequest::from_raw(
            source,
            source_len,
            options_json,
            options_len,
            uri,
            uri_len,
        )?
    };
    f(request.source, request.options_json, request.uri)
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

struct FfiSourceOptionsUriRequest<'a> {
    source: &'a [u8],
    options_json: &'a [u8],
    uri: &'a [u8],
}

impl<'a> FfiSourceOptionsUriRequest<'a> {
    unsafe fn from_raw(
        source: *const u8,
        source_len: usize,
        options_json: *const u8,
        options_len: usize,
        uri: *const u8,
        uri_len: usize,
    ) -> Result<Self, BindingError> {
        let source = unsafe { raw_bytes(source, source_len, "source")? };
        let options_json = unsafe { raw_bytes(options_json, options_len, "options_json")? };
        let uri = unsafe { raw_bytes(uri, uri_len, "uri")? };
        Ok(Self {
            source,
            options_json,
            uri,
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
    #[cfg(feature = "analysis")]
    use merman_bindings_core::ANALYSIS_FACTS_PAYLOAD_VERSION;
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

    fn call_analyze_document(source: &[u8], options: &[u8], uri: &[u8]) -> MermanResult {
        unsafe {
            merman_analyze_document_json(
                source.as_ptr(),
                source.len(),
                options.as_ptr(),
                options.len(),
                uri.as_ptr(),
                uri.len(),
            )
        }
    }

    fn call_analyze_document_facts(source: &[u8], options: &[u8], uri: &[u8]) -> MermanResult {
        unsafe {
            merman_analyze_document_facts_json(
                source.as_ptr(),
                source.len(),
                options.as_ptr(),
                options.len(),
                uri.as_ptr(),
                uri.len(),
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

    fn call_engine_analyze_document(
        engine: *const MermanEngine,
        source: &[u8],
        uri: &[u8],
    ) -> MermanResult {
        unsafe {
            merman_engine_analyze_document_json(
                engine,
                source.as_ptr(),
                source.len(),
                uri.as_ptr(),
                uri.len(),
            )
        }
    }

    fn call_engine_analyze_document_facts(
        engine: *const MermanEngine,
        source: &[u8],
        uri: &[u8],
    ) -> MermanResult {
        unsafe {
            merman_engine_analyze_document_facts_json(
                engine,
                source.as_ptr(),
                source.len(),
                uri.as_ptr(),
                uri.len(),
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

    fn expect_analysis_feature_error(result: MermanResult) {
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
                .contains("analysis feature")
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

    #[cfg(feature = "render")]
    #[test]
    fn ffi_measurement_operation_codes_match_the_core_contract() {
        let ffi_codes = MERMAN_TEXT_MEASUREMENT_OPERATIONS;
        let core_codes = merman_bindings_core::TextMeasurementOperation::ALL
            .map(merman_bindings_core::TextMeasurementOperation::external_code);

        assert_eq!(ffi_codes, core_codes);
        assert_eq!(ffi_codes, std::array::from_fn(|code| code as i32));
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

    #[cfg(feature = "layout-cytoscape")]
    #[test]
    fn complete_ffi_build_renders_architecture() {
        let result = call_render(
            b"architecture-beta\n  service api(server)[API service]\n",
            b"",
        );

        assert_eq!(result.code, BindingStatus::Ok.code());
        let svg = take_text(result.data);
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn render_svg_accepts_options_json() {
        let options = br#"{
            "environment": { "text_measurement": "deterministic" },
            "layout": { "container_width": 640, "container_height": 480 },
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
        if cfg!(feature = "analysis") {
            assert_eq!(valid.code, BindingStatus::Ok.code());
            let json: Value = serde_json::from_str(&take_text(valid.data)).unwrap();
            assert_eq!(json["valid"], true);
            assert_eq!(json["code_name"], BindingStatus::Ok.code_name());
        } else {
            expect_analysis_feature_error(valid);
        }

        let invalid = call_validate(b"", b"");
        if cfg!(feature = "analysis") {
            assert_eq!(invalid.code, BindingStatus::Ok.code());
            let json: Value = serde_json::from_str(&take_text(invalid.data)).unwrap();
            assert_eq!(json["valid"], false);
            assert_eq!(json["code_name"], BindingStatus::NoDiagram.code_name());
        } else {
            expect_analysis_feature_error(invalid);
        }
    }

    #[test]
    fn analyze_json_returns_diagnostics_payload() {
        let valid = call_analyze(b"flowchart TD\nA[Hello]", b"");
        if cfg!(feature = "analysis") {
            assert_eq!(valid.code, BindingStatus::Ok.code());
            let json: Value = serde_json::from_str(&take_text(valid.data)).unwrap();
            assert_eq!(json["version"], 1);
            assert_eq!(json["valid"], true);
            assert_eq!(json["summary"]["errors"], 0);
        } else {
            expect_analysis_feature_error(valid);
        }

        let invalid = call_analyze(b"", b"");
        if cfg!(feature = "analysis") {
            assert_eq!(invalid.code, BindingStatus::Ok.code());
            let json: Value = serde_json::from_str(&take_text(invalid.data)).unwrap();
            assert_eq!(json["version"], 1);
            assert_eq!(json["valid"], false);
            assert_eq!(json["diagnostics"][0]["code_name"], "MERMAN_NO_DIAGRAM");
        } else {
            expect_analysis_feature_error(invalid);
        }
    }

    #[test]
    fn analyze_document_json_returns_markdown_document_payload() {
        let source = b"# Example\n\n```mermaid\nflowchart TD\nA[Hello]\n```\n";
        let result = call_analyze_document(source, b"", b"file:///tmp/example.md");

        if cfg!(feature = "analysis") {
            assert_eq!(result.code, BindingStatus::Ok.code());
            let json: Value = serde_json::from_str(&take_text(result.data)).unwrap();
            assert_eq!(json["version"], 1);
            assert_eq!(json["source"]["kind"], "markdown");
            assert_eq!(json["valid"], true);
        } else {
            expect_analysis_feature_error(result);
        }
    }

    #[test]
    fn analyze_document_facts_json_returns_host_ranges() {
        let source = b"# Example\n\n```mermaid\nflowchart TD\nA[Hello]\n```\n";
        let result = call_analyze_document_facts(source, b"", b"file:///tmp/example.md");

        if cfg!(feature = "analysis") {
            assert_eq!(result.code, BindingStatus::Ok.code());
            let json: Value = serde_json::from_str(&take_text(result.data)).unwrap();
            #[cfg(feature = "analysis")]
            assert_eq!(json["version"], ANALYSIS_FACTS_PAYLOAD_VERSION);
            assert_eq!(json["source"]["kind"], "markdown");
            assert_eq!(json["diagrams"][0]["source_id"], "mermaid-fence-1");
            assert_eq!(
                json["diagrams"][0]["syntax"]["fact_source"],
                "parser_complete"
            );
            assert!(
                json["diagrams"][0]["syntax"]["semantic_items"]
                    .as_array()
                    .is_some_and(|items| items.iter().any(|item| {
                        item["name"] == "A" && item["rename_policy"] == "flowchart_node_id"
                    }))
            );

            let unavailable = call_analyze_document_facts(
                b"```mermaid\nunknownDiagram\nPretendNode --> OtherNode\n```\n",
                b"",
                b"file:///tmp/unknown.md",
            );
            assert_eq!(unavailable.code, BindingStatus::Ok.code());
            let unavailable_json: Value =
                serde_json::from_str(&take_text(unavailable.data)).unwrap();
            #[cfg(feature = "analysis")]
            assert_eq!(unavailable_json["version"], ANALYSIS_FACTS_PAYLOAD_VERSION);
            assert_eq!(
                unavailable_json["diagrams"][0]["syntax"]["fact_source"],
                "unavailable"
            );
            assert_eq!(
                unavailable_json["diagrams"][0]["syntax"]["semantic_items"],
                serde_json::json!([])
            );
        } else {
            expect_analysis_feature_error(result);
        }
    }

    #[test]
    fn malformed_directives_never_return_ffi_panic() {
        // This first input is the minimized source from fuzz run 29806388495. The rest
        // cover the directive forms which previously reached the same editor-only panic.
        let sources: &[&[u8]] = &[
            b"\x00\x36arjav  A --> B\n%$aboxscriD\n  uchart TD\n  %%{init[A:API] Orl(> B\n",
            b"%%{unknown-directive: {\"theme\": \"dark\"}}%%\nflowchart TD\nA --> B\n",
            b"%%{init[A:API] Orl(> B\nflowchart TD\nA --> B\n",
            b"%%{initialize: {\"theme\": }}%%\nflowchart TD\nA --> B\n",
        ];
        let uri = b"file:///tmp/malformed-directive.mmd";

        for source in sources {
            let result = call_analyze_document_facts(source, b"", uri);
            assert_ne!(
                result.code,
                BindingStatus::Panic.code(),
                "one-shot document facts API returned MERMAN_PANIC for {source:?}"
            );
            let _ = take_buffer(result.data);
        }

        let engine = call_engine(b"");
        assert_eq!(engine.code, BindingStatus::Ok.code());
        assert!(!engine.engine.is_null());
        let _ = take_buffer(engine.data);

        for source in sources {
            let result = call_engine_analyze_document_facts(engine.engine, source, uri);
            assert_ne!(
                result.code,
                BindingStatus::Panic.code(),
                "reusable document facts API returned MERMAN_PANIC for {source:?}"
            );
            let _ = take_buffer(result.data);
        }

        unsafe { merman_engine_free(engine.engine) };
    }

    #[test]
    fn metadata_entry_points_return_json_contracts() {
        let diagrams = merman_supported_diagrams_json();
        let runtime_contract = merman_runtime_contract_json();
        let ascii_capabilities = merman_ascii_capabilities_json();
        let family_capabilities = merman_diagram_family_capabilities_json();
        let lint_rules = merman_lint_rule_catalog_json();
        let themes = merman_supported_themes_json();
        let host_theme_presets = merman_supported_host_theme_presets_json();

        assert_eq!(diagrams.code, BindingStatus::Ok.code());
        assert_eq!(runtime_contract.code, BindingStatus::Ok.code());
        assert_eq!(ascii_capabilities.code, BindingStatus::Ok.code());
        assert_eq!(family_capabilities.code, BindingStatus::Ok.code());
        let lint_rules_json = if cfg!(feature = "analysis") {
            assert_eq!(lint_rules.code, BindingStatus::Ok.code());
            Some(take_text(lint_rules.data))
        } else {
            expect_analysis_feature_error(lint_rules);
            None
        };
        assert_eq!(themes.code, BindingStatus::Ok.code());
        assert_eq!(host_theme_presets.code, BindingStatus::Ok.code());

        let diagrams: Value = serde_json::from_str(&take_text(diagrams.data)).unwrap();
        let runtime_contract: Value =
            serde_json::from_str(&take_text(runtime_contract.data)).unwrap();
        let ascii_capabilities: Value =
            serde_json::from_str(&take_text(ascii_capabilities.data)).unwrap();
        let family_capabilities: Value =
            serde_json::from_str(&take_text(family_capabilities.data)).unwrap();
        let themes: Value = serde_json::from_str(&take_text(themes.data)).unwrap();
        let host_theme_presets: Value =
            serde_json::from_str(&take_text(host_theme_presets.data)).unwrap();

        assert!(
            diagrams
                .as_array()
                .unwrap()
                .contains(&Value::String("flowchart".to_string()))
        );
        assert_eq!(
            runtime_contract["schema_version"],
            merman_bindings_core::RUNTIME_CONTRACT_SCHEMA_VERSION
        );
        assert_eq!(runtime_contract["abi_version"], MERMAN_ABI_VERSION);
        assert_eq!(runtime_contract["options_schema_version"], 1);
        assert!(runtime_contract["features"].get("core_host").is_none());
        assert_eq!(
            runtime_contract["features"]["system_adapter_ids"],
            serde_json::json!(merman_bindings_core::binding_capabilities().system_adapter_ids)
        );
        assert_eq!(
            runtime_contract["features"]["render"],
            cfg!(feature = "render")
        );
        if cfg!(any(
            feature = "render",
            feature = "analysis",
            feature = "ascii"
        )) {
            assert_eq!(
                runtime_contract["resources"]["general_binding_default_profile"],
                "interactive"
            );
        } else {
            assert!(runtime_contract["resources"].is_null());
        }
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
                && capability["logical_family_kind"] == "flowchart"
                && capability["metadata_id"] == "flowchart"
                && capability["render_model_kind"] == "flowchart"
                && capability["has_detector"] == true
                && capability["has_semantic_parser"] == true
                && capability["has_editor_parser"] == true
                && capability["has_combined_parser"] == true
                && capability["has_render_parser"] == true
                && capability["has_header"] == false
                && capability["config_namespace"] == "flowchart"
        ));
        if let Some(lint_rules_json) = lint_rules_json {
            let lint_rules: Value = serde_json::from_str(&lint_rules_json).unwrap();
            assert_eq!(lint_rules["version"], 1);
            let lint_rules = lint_rules["rules"].as_array().unwrap();
            assert!(lint_rules.iter().any(|rule| {
                rule["id"] == "merman.authoring.config.prefer_init_directive"
                    && rule["origin"] == "merman_authoring"
                    && rule["evidence"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|value| value == "docs/adr/0072-lint-rule-governance.md")
            }));
        }
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
            br#"{ "resources": { "limits": { "max_source_bytes": 4 } } }"#,
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
    fn typed_resource_options_builder_returns_versioned_json_and_rejects_invalid_codes() {
        let overrides = [MermanResourceLimitOverride { id: 0, value: 4096 }];
        let result =
            unsafe { merman_resource_options_json(1, overrides.as_ptr(), overrides.len()) };
        if cfg!(any(
            feature = "render",
            feature = "analysis",
            feature = "ascii"
        )) {
            assert_eq!(result.code, BindingStatus::Ok.code());
            let value: Value = serde_json::from_str(&take_text(result.data)).unwrap();
            assert_eq!(value["version"], 1);
            assert_eq!(value["resources"]["profile"], "constrained");
            assert_eq!(value["resources"]["limits"]["max_source_bytes"], 4096);

            let invalid = unsafe { merman_resource_options_json(99, std::ptr::null(), 0) };
            assert_eq!(invalid.code, BindingStatus::InvalidArgument.code());
            unsafe { merman_buffer_free(invalid.data) };
        } else {
            assert_eq!(result.code, BindingStatus::UnsupportedFormat.code());
            let error = take_error(result);
            assert_eq!(
                error["code_name"],
                BindingStatus::UnsupportedFormat.code_name()
            );
            assert_eq!(
                error["message"],
                "resource options requires at least one resource-aware operation"
            );
        }
        assert_eq!(
            merman_resource_limit_override_struct_size(),
            std::mem::size_of::<MermanResourceLimitOverride>()
        );
    }

    #[test]
    fn unsupported_ratex_without_feature_returns_unsupported_format() {
        let result = call_render(
            b"flowchart TD\nA[Hello]",
            br#"{ "environment": { "math_renderer": "ratex" } }"#,
        );

        if cfg!(feature = "math") {
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
    fn ffi_source_options_uri_request_decodes_uri() {
        let source = b"flowchart TD\nA[Hello]";
        let uri = b"file:///tmp/example.mmd";
        let request = unsafe {
            FfiSourceOptionsUriRequest::from_raw(
                source.as_ptr(),
                source.len(),
                ptr::null(),
                0,
                uri.as_ptr(),
                uri.len(),
            )
        }
        .unwrap();

        assert_eq!(request.source, source);
        assert!(request.options_json.is_empty());
        assert_eq!(request.uri, uri);
    }

    #[test]
    fn ffi_engine_source_call_decodes_engine_and_source() {
        let engine = call_engine(b"");
        assert_eq!(engine.code, BindingStatus::Ok.code());
        let source = b"flowchart TD\nA[Hello]";
        let output = unsafe {
            ffi_engine_source_call(
                engine.engine,
                source.as_ptr(),
                source.len(),
                |_engine, source| Ok(source.to_vec()),
            )
        }
        .unwrap();

        assert_eq!(output, source);
        unsafe { merman_engine_free(engine.engine) };
    }

    #[cfg(feature = "render")]
    #[test]
    fn ffi_engine_source_call_rejects_active_mutation() {
        let engine = call_engine(b"");
        assert_eq!(engine.code, BindingStatus::Ok.code());
        let lease = acquire_engine_lease(engine.engine).unwrap();
        let mutation = lease.inner.try_write().unwrap();
        let source = b"flowchart TD\nA[Hello]";

        let err = unsafe {
            ffi_engine_source_call(
                engine.engine,
                source.as_ptr(),
                source.len(),
                |_engine, _source| Ok(Vec::new()),
            )
        }
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("active mutation"));
        drop(mutation);
        drop(lease);
        unsafe { merman_engine_free(engine.engine) };
    }

    #[test]
    fn ffi_engine_source_uri_call_decodes_engine_source_and_uri() {
        let engine = call_engine(b"");
        assert_eq!(engine.code, BindingStatus::Ok.code());
        let source = b"flowchart TD\nA[Hello]";
        let uri = b"file:///tmp/example.mmd";
        let output = unsafe {
            ffi_engine_source_uri_call(
                engine.engine,
                source.as_ptr(),
                source.len(),
                uri.as_ptr(),
                uri.len(),
                |_engine, source, uri| {
                    let mut output = source.to_vec();
                    output.push(b'\n');
                    output.extend_from_slice(uri);
                    Ok(output)
                },
            )
        }
        .unwrap();

        assert_eq!(output, b"flowchart TD\nA[Hello]\nfile:///tmp/example.mmd");
        unsafe { merman_engine_free(engine.engine) };
    }

    #[test]
    fn reusable_engine_renders_with_cached_options() {
        let options = br#"{
            "environment": { "text_measurement": "deterministic" },
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
    fn reusable_engine_analyzes_documents_with_cached_options() {
        let engine = call_engine(br#"{ "analysis": { "profile": "strict" } }"#);
        assert_eq!(engine.code, BindingStatus::Ok.code());
        assert!(!engine.engine.is_null());

        let source = b"# Example\n\n```mermaid\nflowchart TD\nA[Hello]\n```\n";
        let result = call_engine_analyze_document(engine.engine, source, b"file:///tmp/example.md");

        if cfg!(feature = "analysis") {
            assert_eq!(result.code, BindingStatus::Ok.code());
            let json: Value = serde_json::from_str(&take_text(result.data)).unwrap();
            assert_eq!(json["source"]["kind"], "markdown");
        } else {
            expect_analysis_feature_error(result);
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
            operations: [bool; 19],
        }

        unsafe extern "C" fn measure_condition(
            request: MermanHostTextMeasureRequest,
            user_data: *mut std::ffi::c_void,
        ) -> MermanHostTextMeasureResult {
            if user_data.is_null() {
                return MermanHostTextMeasureResult::unhandled();
            }
            let text = unsafe { std::slice::from_raw_parts(request.text, request.text_len) };
            if text == b"Condition?" && request.wrap_mode == MERMAN_WRAP_MODE_HTML_LIKE {
                let probe = unsafe { &mut *(user_data.cast::<CallbackProbe>()) };
                if let Ok(operation) = usize::try_from(request.operation)
                    && operation < probe.operations.len()
                {
                    probe.operations[operation] = true;
                }
                probe.saw_condition = true;
                let font_style = unsafe {
                    std::slice::from_raw_parts(request.font_style, request.font_style_len)
                };
                probe.saw_font_style |= font_style == b"italic"
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
                let mut result = MermanHostTextMeasureResult::unhandled();
                result.handled = 1;
                match request.operation {
                    MERMAN_TEXT_MEASUREMENT_OPERATION_MEASURE
                    | MERMAN_TEXT_MEASUREMENT_OPERATION_WRAPPED
                    | MERMAN_TEXT_MEASUREMENT_OPERATION_MERMAID_CALCULATE_TEXT_DIMENSIONS => {
                        result.result_kind = MERMAN_TEXT_MEASUREMENT_RESULT_KIND_METRICS;
                        result.width = 140.0;
                        result.height = 24.0;
                        result.line_count = 1;
                    }
                    MERMAN_TEXT_MEASUREMENT_OPERATION_COMPUTED_LENGTH
                    | MERMAN_TEXT_MEASUREMENT_OPERATION_SIMPLE_BBOX_WIDTH
                    | MERMAN_TEXT_MEASUREMENT_OPERATION_RAW_BBOX_WIDTH
                    | MERMAN_TEXT_MEASUREMENT_OPERATION_BOUNDING_CLIENT_RECT_WIDTH
                    | MERMAN_TEXT_MEASUREMENT_OPERATION_TSPAN_BBOX_WIDTH
                    | MERMAN_TEXT_MEASUREMENT_OPERATION_WRAP_PROBE_BBOX_WIDTH
                    | MERMAN_TEXT_MEASUREMENT_OPERATION_CANVAS_MEASURE_TEXT_WIDTH => {
                        result.result_kind = MERMAN_TEXT_MEASUREMENT_RESULT_KIND_LENGTH;
                        result.length = 140.0;
                    }
                    MERMAN_TEXT_MEASUREMENT_OPERATION_TSPAN_BBOX_HEIGHT
                    | MERMAN_TEXT_MEASUREMENT_OPERATION_SIMPLE_BBOX_HEIGHT
                    | MERMAN_TEXT_MEASUREMENT_OPERATION_RAW_BBOX_HEIGHT => {
                        result.result_kind = MERMAN_TEXT_MEASUREMENT_RESULT_KIND_LENGTH;
                        result.length = 24.0;
                    }
                    MERMAN_TEXT_MEASUREMENT_OPERATION_CREATE_TEXT_BBOX_Y_OFFSET
                    | MERMAN_TEXT_MEASUREMENT_OPERATION_CREATE_TEXT_MIDDLE_BBOX_Y_OFFSET => {
                        result.result_kind = MERMAN_TEXT_MEASUREMENT_RESULT_KIND_LENGTH;
                        result.length = if request.operation
                            == MERMAN_TEXT_MEASUREMENT_OPERATION_CREATE_TEXT_MIDDLE_BBOX_Y_OFFSET
                        {
                            -2.0
                        } else {
                            -1.0
                        };
                    }
                    MERMAN_TEXT_MEASUREMENT_OPERATION_BBOX_X
                    | MERMAN_TEXT_MEASUREMENT_OPERATION_BBOX_X_WITH_ASCII_OVERHANG
                    | MERMAN_TEXT_MEASUREMENT_OPERATION_TITLE_BBOX_X => {
                        result.result_kind = MERMAN_TEXT_MEASUREMENT_RESULT_KIND_HORIZONTAL_EXTENTS;
                        result.bbox_left = 70.0;
                        result.bbox_right = 70.0;
                    }
                    MERMAN_TEXT_MEASUREMENT_OPERATION_WRAPPED_WITH_RAW_WIDTH => {
                        result.result_kind =
                            MERMAN_TEXT_MEASUREMENT_RESULT_KIND_WRAPPED_WITH_RAW_WIDTH;
                        result.width = 140.0;
                        result.height = 24.0;
                        result.raw_width = 140.0;
                        result.line_count = 1;
                        result.has_raw_width = 1;
                    }
                    _ => result.handled = 0,
                }
                return result;
            }
            MermanHostTextMeasureResult::unhandled()
        }

        let engine = call_engine(b"");
        assert_eq!(engine.code, BindingStatus::Ok.code());
        assert!(!engine.engine.is_null());
        let source = b"flowchart TD\nA[Start] --> B{Condition?}\nclassDef emphasized font-style:italic\nclass B emphasized";

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
        assert!(callback_probe.operations[MERMAN_TEXT_MEASUREMENT_OPERATION_WRAPPED as usize]);

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

    #[cfg(feature = "render")]
    #[test]
    fn ffi_callback_preserves_wrong_result_kind_for_invalid_fallback() {
        unsafe extern "C" fn wrong_kind(
            _request: MermanHostTextMeasureRequest,
            _user_data: *mut std::ffi::c_void,
        ) -> MermanHostTextMeasureResult {
            let mut result = MermanHostTextMeasureResult::unhandled();
            result.handled = 1;
            result.result_kind = MERMAN_TEXT_MEASUREMENT_RESULT_KIND_METRICS;
            result.width = 73.0;
            result.height = 12.0;
            result.line_count = 1;
            result
        }

        let host = FfiHostTextMeasurer::new(wrong_kind, std::ptr::null_mut());
        let style = merman_bindings_core::TextStyle::default();
        let result = host
            .call_host(merman_bindings_core::HostTextMeasurementRequest {
                operation: merman_bindings_core::TextMeasurementOperation::ComputedLength,
                phase: merman_bindings_core::TextMeasurementPhase::ComputedLength,
                text: "wrong-kind",
                style: &style,
                max_width: None,
                wrap_mode: merman_bindings_core::WrapMode::SvgLike,
            })
            .expect("callback transport")
            .expect("handled result");

        assert!(matches!(
            result,
            merman_bindings_core::HostTextMeasurement::Metrics(_)
        ));
    }

    #[cfg(feature = "render")]
    #[test]
    fn ffi_callback_forwards_new_measurement_operations_and_result_shapes() {
        unsafe extern "C" fn measure_new_operation(
            request: MermanHostTextMeasureRequest,
            _user_data: *mut std::ffi::c_void,
        ) -> MermanHostTextMeasureResult {
            let mut result = MermanHostTextMeasureResult::unhandled();
            result.handled = 1;
            match request.operation {
                MERMAN_TEXT_MEASUREMENT_OPERATION_MERMAID_CALCULATE_TEXT_DIMENSIONS => {
                    result.result_kind = MERMAN_TEXT_MEASUREMENT_RESULT_KIND_METRICS;
                    result.width = 81.0;
                    result.height = 17.0;
                    result.line_count = 1;
                }
                MERMAN_TEXT_MEASUREMENT_OPERATION_CANVAS_MEASURE_TEXT_WIDTH => {
                    result.result_kind = MERMAN_TEXT_MEASUREMENT_RESULT_KIND_LENGTH;
                    result.length = 93.5;
                }
                MERMAN_TEXT_MEASUREMENT_OPERATION_CREATE_TEXT_MIDDLE_BBOX_Y_OFFSET => {
                    result.result_kind = MERMAN_TEXT_MEASUREMENT_RESULT_KIND_LENGTH;
                    result.length = -3.25;
                }
                MERMAN_TEXT_MEASUREMENT_OPERATION_RAW_BBOX_HEIGHT => {
                    result.result_kind = MERMAN_TEXT_MEASUREMENT_RESULT_KIND_LENGTH;
                    result.length = 19.25;
                }
                _ => result.handled = 0,
            }
            result
        }

        let host = FfiHostTextMeasurer::new(measure_new_operation, std::ptr::null_mut());
        let style = merman_bindings_core::TextStyle::default();
        let request = |operation| merman_bindings_core::HostTextMeasurementRequest {
            operation,
            phase: merman_bindings_core::TextMeasurementPhase::Layout,
            text: "new-operation",
            style: &style,
            max_width: None,
            wrap_mode: merman_bindings_core::WrapMode::SvgLike,
        };

        let metrics = host
            .call_host(request(
                merman_bindings_core::TextMeasurementOperation::MermaidCalculateTextDimensions,
            ))
            .expect("callback transport")
            .expect("handled dimensions result");
        let merman_bindings_core::HostTextMeasurement::Metrics(metrics) = metrics else {
            panic!("Mermaid dimensions must use the metrics result shape");
        };
        assert_eq!(
            (metrics.width, metrics.height, metrics.line_count),
            (81.0, 17.0, 1)
        );

        let length = host
            .call_host(request(
                merman_bindings_core::TextMeasurementOperation::CanvasMeasureTextWidth,
            ))
            .expect("callback transport")
            .expect("handled canvas result");
        let merman_bindings_core::HostTextMeasurement::Length(length) = length else {
            panic!("canvas text width must use the length result shape");
        };
        assert_eq!(length, 93.5);

        let middle_y_offset = host
            .call_host(request(
                merman_bindings_core::TextMeasurementOperation::CreateTextMiddleBBoxYOffset,
            ))
            .expect("callback transport")
            .expect("handled createText middle y-offset result");
        let merman_bindings_core::HostTextMeasurement::Length(middle_y_offset) = middle_y_offset
        else {
            panic!("createText middle y-offset must use the length result shape");
        };
        assert_eq!(middle_y_offset, -3.25);

        let raw_height = host
            .call_host(request(
                merman_bindings_core::TextMeasurementOperation::RawBBoxHeight,
            ))
            .expect("callback transport")
            .expect("handled raw bbox height result");
        let merman_bindings_core::HostTextMeasurement::Length(raw_height) = raw_height else {
            panic!("raw bbox height must use the length result shape");
        };
        assert_eq!(raw_height, 19.25);
    }

    #[cfg(feature = "render")]
    #[test]
    fn reusable_engine_rejects_text_measure_callback_mutation_from_callback() {
        struct CallbackMutationProbe {
            engine: *mut MermanEngine,
            set_code: i32,
        }

        unsafe extern "C" fn measure_and_mutate(
            request: MermanHostTextMeasureRequest,
            user_data: *mut std::ffi::c_void,
        ) -> MermanHostTextMeasureResult {
            let probe = unsafe { &mut *(user_data.cast::<CallbackMutationProbe>()) };
            if probe.set_code == BindingStatus::Ok.code() {
                let result = unsafe {
                    merman_engine_set_text_measure_callback(probe.engine, None, ptr::null_mut())
                };
                probe.set_code = result.code;
                if !result.data.data.is_null() {
                    unsafe { merman_buffer_free(result.data) };
                }
            }
            MermanHostTextMeasureResult {
                handled: 1,
                width: (request.text_len as f64 * 8.0).max(1.0),
                height: request.line_height.max(1.0),
                line_count: 1,
                ..MermanHostTextMeasureResult::unhandled()
            }
        }

        let engine = call_engine(b"");
        assert_eq!(engine.code, BindingStatus::Ok.code());
        let mut probe = CallbackMutationProbe {
            engine: engine.engine,
            set_code: BindingStatus::Ok.code(),
        };
        let set_result = unsafe {
            merman_engine_set_text_measure_callback(
                engine.engine,
                Some(measure_and_mutate),
                (&mut probe as *mut CallbackMutationProbe).cast(),
            )
        };
        assert_eq!(set_result.code, BindingStatus::Ok.code());

        let rendered = call_engine_render(engine.engine, b"flowchart TD\nA[Measured] --> B[Done]");
        assert_eq!(rendered.code, BindingStatus::Ok.code());
        let svg = take_text(rendered.data);
        assert!(svg.contains("<svg"));
        assert_eq!(probe.set_code, BindingStatus::InvalidArgument.code());

        unsafe { merman_engine_free(engine.engine) };
    }

    #[cfg(feature = "render")]
    #[test]
    fn reusable_engine_defers_free_requested_from_text_measure_callback() {
        struct CallbackFreeProbe {
            engine: *mut MermanEngine,
            free_called: bool,
        }

        unsafe extern "C" fn measure_and_free(
            request: MermanHostTextMeasureRequest,
            user_data: *mut std::ffi::c_void,
        ) -> MermanHostTextMeasureResult {
            let probe = unsafe { &mut *(user_data.cast::<CallbackFreeProbe>()) };
            if !probe.free_called {
                probe.free_called = true;
                unsafe { merman_engine_free(probe.engine) };
            }
            MermanHostTextMeasureResult {
                handled: 1,
                width: (request.text_len as f64 * 8.0).max(1.0),
                height: request.line_height.max(1.0),
                line_count: 1,
                ..MermanHostTextMeasureResult::unhandled()
            }
        }

        let engine = call_engine(b"");
        assert_eq!(engine.code, BindingStatus::Ok.code());
        let mut probe = CallbackFreeProbe {
            engine: engine.engine,
            free_called: false,
        };
        let set_result = unsafe {
            merman_engine_set_text_measure_callback(
                engine.engine,
                Some(measure_and_free),
                (&mut probe as *mut CallbackFreeProbe).cast(),
            )
        };
        assert_eq!(set_result.code, BindingStatus::Ok.code());

        let rendered = call_engine_render(engine.engine, b"flowchart TD\nA[Measured] --> B[Done]");
        assert_eq!(rendered.code, BindingStatus::Ok.code());
        let svg = take_text(rendered.data);
        assert!(svg.contains("<svg"));
        assert!(probe.free_called);
    }

    #[test]
    fn reusable_engine_reports_invalid_options_json() {
        let engine = call_engine(b"{");

        if cfg!(any(
            feature = "analysis",
            feature = "render",
            feature = "ascii"
        )) {
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

    #[test]
    fn engine_lease_survives_free_before_first_state_access() {
        let engine = call_engine(b"");
        assert_eq!(engine.code, BindingStatus::Ok.code());
        assert!(!engine.engine.is_null());

        let lease = acquire_engine_lease(engine.engine).expect("live handle should yield a lease");
        let engine_addr = engine.engine as usize;
        std::thread::spawn(move || unsafe {
            merman_engine_free(engine_addr as *mut MermanEngine);
        })
        .join()
        .unwrap();
        assert_eq!(Arc::strong_count(&lease), 1);

        let error = match acquire_engine_lease(engine.engine) {
            Ok(_) => panic!("free must retire the handle"),
            Err(error) => error,
        };
        assert_eq!(error.status(), BindingStatus::InvalidArgument);

        let state = lease
            .inner
            .try_read()
            .expect("the pre-free lease must keep engine state alive");
        let validation = state.validate_json(b"flowchart TD\nA");
        if cfg!(feature = "analysis") {
            assert!(validation.is_ok());
        } else {
            assert_eq!(
                validation.unwrap_err().status(),
                BindingStatus::UnsupportedFormat
            );
        }
    }

    #[test]
    fn freed_engine_handle_never_rebinds_to_a_new_engine() {
        let first = call_engine(b"");
        assert_eq!(first.code, BindingStatus::Ok.code());
        assert!(!first.engine.is_null());
        let stale = first.engine;
        unsafe { merman_engine_free(stale) };

        let replacement = call_engine(b"");
        assert_eq!(replacement.code, BindingStatus::Ok.code());
        assert!(!replacement.engine.is_null());
        assert_ne!(stale.addr(), replacement.engine.addr());

        let error = match acquire_engine_lease(stale) {
            Ok(_) => panic!("a retired handle must not identify a replacement engine"),
            Err(error) => error,
        };
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(acquire_engine_lease(replacement.engine).is_ok());

        unsafe { merman_engine_free(replacement.engine) };
    }

    #[test]
    fn engine_registry_rejects_token_exhaustion_without_reuse() {
        let inner = BindingEngine::new(b"").unwrap();
        let state = Arc::new(FfiEngineState {
            #[cfg(feature = "render")]
            base: inner.clone(),
            inner: RwLock::new(inner),
        });
        let mut registry = FfiEngineRegistry {
            last_token: usize::MAX,
            engines: BTreeMap::new(),
        };

        let error = registry.register(state).unwrap_err();

        assert_eq!(error.status(), BindingStatus::InternalError);
        assert_eq!(registry.last_token, usize::MAX);
        assert!(registry.engines.is_empty());
    }
}

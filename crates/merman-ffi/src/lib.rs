#![deny(unsafe_op_in_unsafe_fn)]

//! C ABI exports for embedding `merman` in non-Rust hosts.
//!
//! This crate is the only place where the public FFI boundary owns unsafe code. The core
//! parser/render crates remain safe Rust APIs.

use merman::render::{
    DeterministicTextMeasurer, HeadlessRenderer, LayoutOptions, VendoredFontMetricsTextMeasurer,
};
use serde::{Deserialize, Serialize};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::Arc;

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

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MermanStatus {
    Ok = 0,
    InvalidArgument = 1,
    Utf8Error = 2,
    OptionsJsonError = 3,
    NoDiagram = 4,
    ParseError = 5,
    RenderError = 6,
    #[allow(dead_code)]
    UnsupportedFormat = 7,
    Panic = 8,
    InternalError = 9,
}

impl MermanStatus {
    const fn code(self) -> i32 {
        self as i32
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Ok => "MERMAN_OK",
            Self::InvalidArgument => "MERMAN_INVALID_ARGUMENT",
            Self::Utf8Error => "MERMAN_UTF8_ERROR",
            Self::OptionsJsonError => "MERMAN_OPTIONS_JSON_ERROR",
            Self::NoDiagram => "MERMAN_NO_DIAGRAM",
            Self::ParseError => "MERMAN_PARSE_ERROR",
            Self::RenderError => "MERMAN_RENDER_ERROR",
            Self::UnsupportedFormat => "MERMAN_UNSUPPORTED_FORMAT",
            Self::Panic => "MERMAN_PANIC",
            Self::InternalError => "MERMAN_INTERNAL_ERROR",
        }
    }
}

#[derive(Debug)]
struct FfiError {
    status: MermanStatus,
    message: String,
}

impl FfiError {
    fn new(status: MermanStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ErrorPayload<'a> {
    version: u32,
    ok: bool,
    code: i32,
    code_name: &'a str,
    message: &'a str,
}

#[derive(Debug, Default, Deserialize)]
struct FfiOptions {
    #[allow(dead_code)]
    version: Option<u32>,
    parse: Option<ParseOptionsJson>,
    layout: Option<LayoutOptionsJson>,
    svg: Option<SvgOptionsJson>,
}

#[derive(Debug, Default, Deserialize)]
struct ParseOptionsJson {
    suppress_errors: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct LayoutOptionsJson {
    viewport_width: Option<f64>,
    viewport_height: Option<f64>,
    text_measurer: Option<String>,
    math_renderer: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SvgOptionsJson {
    diagram_id: Option<String>,
    pipeline: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PipelineKind {
    Parity,
    Readable,
    ResvgSafe,
}

impl Default for PipelineKind {
    fn default() -> Self {
        Self::Parity
    }
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
    F: FnOnce() -> Result<Vec<u8>, FfiError>,
{
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(bytes)) => MermanResult {
            code: MermanStatus::Ok.code(),
            data: buffer_from_vec(bytes),
        },
        Ok(Err(err)) => error_result(err.status, &err.message),
        Err(_) => error_result(MermanStatus::Panic, "panic caught at merman FFI boundary"),
    }
}

unsafe fn render_svg_impl(
    source: *const u8,
    source_len: usize,
    options_json: *const u8,
    options_len: usize,
) -> Result<Vec<u8>, FfiError> {
    let source_bytes = unsafe { raw_bytes(source, source_len, "source")? };
    let options_bytes = unsafe { raw_bytes(options_json, options_len, "options_json")? };

    let source = std::str::from_utf8(source_bytes).map_err(|err| {
        FfiError::new(
            MermanStatus::Utf8Error,
            format!("invalid source UTF-8: {err}"),
        )
    })?;
    if source.trim().is_empty() {
        return Err(FfiError::new(
            MermanStatus::NoDiagram,
            "no Mermaid diagram detected",
        ));
    }

    let options = parse_options(options_bytes)?;
    let (renderer, pipeline) = build_renderer(options)?;

    let svg = match pipeline {
        PipelineKind::Parity => renderer.render_svg_sync(source),
        PipelineKind::Readable => renderer.render_svg_readable_sync(source),
        PipelineKind::ResvgSafe => renderer.render_svg_resvg_safe_sync(source),
    }
    .map_err(classify_render_error)?;

    match svg {
        Some(svg) => Ok(svg.into_bytes()),
        None => Err(FfiError::new(
            MermanStatus::NoDiagram,
            "no Mermaid diagram detected",
        )),
    }
}

unsafe fn raw_bytes<'a>(
    data: *const u8,
    len: usize,
    name: &'static str,
) -> Result<&'a [u8], FfiError> {
    if data.is_null() {
        if len == 0 {
            return Ok(&[]);
        }
        return Err(FfiError::new(
            MermanStatus::InvalidArgument,
            format!("{name} pointer is null but length is {len}"),
        ));
    }

    if len == 0 {
        return Ok(&[]);
    }

    Ok(unsafe { std::slice::from_raw_parts(data, len) })
}

fn parse_options(bytes: &[u8]) -> Result<FfiOptions, FfiError> {
    if bytes.is_empty() {
        return Ok(FfiOptions::default());
    }
    let text = std::str::from_utf8(bytes).map_err(|err| {
        FfiError::new(
            MermanStatus::Utf8Error,
            format!("invalid options_json UTF-8: {err}"),
        )
    })?;
    serde_json::from_str(text).map_err(|err| {
        FfiError::new(
            MermanStatus::OptionsJsonError,
            format!("invalid options_json: {err}"),
        )
    })
}

fn build_renderer(options: FfiOptions) -> Result<(HeadlessRenderer, PipelineKind), FfiError> {
    let mut renderer = HeadlessRenderer::new();

    if options
        .parse
        .as_ref()
        .and_then(|parse| parse.suppress_errors)
        .unwrap_or(false)
    {
        renderer = renderer.with_lenient_parsing();
    } else {
        renderer = renderer.with_strict_parsing();
    }

    let mut layout = LayoutOptions::headless_svg_defaults();
    if let Some(layout_json) = options.layout.as_ref() {
        if let Some(width) = layout_json.viewport_width {
            layout.viewport_width = finite_positive(width, "layout.viewport_width")?;
        }
        if let Some(height) = layout_json.viewport_height {
            layout.viewport_height = finite_positive(height, "layout.viewport_height")?;
        }
        if let Some(kind) = layout_json.text_measurer.as_deref() {
            match normalize_option(kind).as_str() {
                "vendored" => {
                    layout.text_measurer = Arc::new(VendoredFontMetricsTextMeasurer::default());
                }
                "deterministic" => {
                    layout.text_measurer = Arc::new(DeterministicTextMeasurer::default());
                }
                other => {
                    return Err(FfiError::new(
                        MermanStatus::InvalidArgument,
                        format!("unsupported layout.text_measurer: {other}"),
                    ));
                }
            }
        }
    }
    renderer = renderer.with_layout_options(layout);

    if let Some(math_renderer) = options
        .layout
        .as_ref()
        .and_then(|layout| layout.math_renderer.as_deref())
    {
        match normalize_option(math_renderer).as_str() {
            "none" => {}
            "ratex" => {
                #[cfg(feature = "ratex-math")]
                {
                    renderer =
                        renderer.with_math_renderer(Arc::new(merman::render::RatexMathRenderer));
                }
                #[cfg(not(feature = "ratex-math"))]
                {
                    return Err(FfiError::new(
                        MermanStatus::UnsupportedFormat,
                        "layout.math_renderer=ratex requires the ratex-math feature",
                    ));
                }
            }
            other => {
                return Err(FfiError::new(
                    MermanStatus::InvalidArgument,
                    format!("unsupported layout.math_renderer: {other}"),
                ));
            }
        }
    }

    let mut pipeline = PipelineKind::default();
    if let Some(svg) = options.svg.as_ref() {
        if let Some(diagram_id) = svg.diagram_id.as_deref() {
            renderer = renderer.with_diagram_id(diagram_id);
        }
        if let Some(raw_pipeline) = svg.pipeline.as_deref() {
            pipeline = match normalize_option(raw_pipeline).as_str() {
                "parity" => PipelineKind::Parity,
                "readable" => PipelineKind::Readable,
                "resvg-safe" | "resvg_safe" => PipelineKind::ResvgSafe,
                other => {
                    return Err(FfiError::new(
                        MermanStatus::InvalidArgument,
                        format!("unsupported svg.pipeline: {other}"),
                    ));
                }
            };
        }
    }

    Ok((renderer, pipeline))
}

fn classify_render_error(err: merman::render::HeadlessError) -> FfiError {
    match err {
        merman::render::HeadlessError::Parse(err) => {
            FfiError::new(MermanStatus::ParseError, err.to_string())
        }
        merman::render::HeadlessError::Render(err) => {
            FfiError::new(MermanStatus::RenderError, err.to_string())
        }
    }
}

fn finite_positive(value: f64, name: &'static str) -> Result<f64, FfiError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(FfiError::new(
            MermanStatus::InvalidArgument,
            format!("{name} must be a finite positive number"),
        ))
    }
}

fn normalize_option(value: &str) -> String {
    value.trim().to_ascii_lowercase()
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

fn error_result(status: MermanStatus, message: &str) -> MermanResult {
    let payload = ErrorPayload {
        version: 1,
        ok: false,
        code: status.code(),
        code_name: status.name(),
        message,
    };
    let bytes = serde_json::to_vec(&payload).unwrap_or_else(|_| {
        format!(
            r#"{{"version":1,"ok":false,"code":{},"code_name":"{}","message":"internal error payload serialization failed"}}"#,
            MermanStatus::InternalError.code(),
            MermanStatus::InternalError.name()
        )
        .into_bytes()
    });
    MermanResult {
        code: status.code(),
        data: buffer_from_vec(bytes),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

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
    fn render_svg_returns_svg_for_flowchart() {
        let result = call_render(b"flowchart TD\nA[Hello] --> B[World]", b"");

        assert_eq!(result.code, MermanStatus::Ok.code());
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

        assert_eq!(result.code, MermanStatus::Ok.code());
        let svg = take_text(result.data);
        assert!(svg.contains("id=\"ffi-diagram\""));
        assert!(svg.contains("data-merman-foreignobject"));
    }

    #[test]
    fn null_source_with_nonzero_len_returns_invalid_argument() {
        let result = unsafe { merman_render_svg(ptr::null(), 1, ptr::null(), 0) };

        assert_eq!(result.code, MermanStatus::InvalidArgument.code());
        let error = take_error(result);
        assert_eq!(error["code_name"], MermanStatus::InvalidArgument.name());
    }

    #[test]
    fn invalid_source_utf8_returns_utf8_error() {
        let result = call_render(&[0xff], b"");

        assert_eq!(result.code, MermanStatus::Utf8Error.code());
        let error = take_error(result);
        assert_eq!(error["code_name"], MermanStatus::Utf8Error.name());
    }

    #[test]
    fn empty_source_returns_no_diagram() {
        let result = unsafe { merman_render_svg(ptr::null(), 0, ptr::null(), 0) };

        assert_eq!(result.code, MermanStatus::NoDiagram.code());
        let error = take_error(result);
        assert_eq!(error["code_name"], MermanStatus::NoDiagram.name());
    }

    #[test]
    fn invalid_options_json_returns_options_json_error() {
        let result = call_render(b"flowchart TD\nA", b"{");

        assert_eq!(result.code, MermanStatus::OptionsJsonError.code());
        let error = take_error(result);
        assert_eq!(error["code_name"], MermanStatus::OptionsJsonError.name());
    }

    #[test]
    fn unsupported_ratex_without_feature_returns_unsupported_format() {
        let result = call_render(
            b"flowchart TD\nA[Hello]",
            br#"{ "layout": { "math_renderer": "ratex" } }"#,
        );

        if cfg!(feature = "ratex-math") {
            assert_eq!(result.code, MermanStatus::Ok.code());
            unsafe { merman_buffer_free(result.data) };
        } else {
            assert_eq!(result.code, MermanStatus::UnsupportedFormat.code());
            let error = take_error(result);
            assert_eq!(error["code_name"], MermanStatus::UnsupportedFormat.name());
        }
    }

    #[test]
    fn buffer_free_accepts_null_buffer() {
        unsafe { merman_buffer_free(MermanBuffer::empty()) };
    }

    #[test]
    fn ffi_result_catches_panic() {
        let result = ffi_result(|| -> Result<Vec<u8>, FfiError> { panic!("boom") });

        assert_eq!(result.code, MermanStatus::Panic.code());
        let error = take_error(result);
        assert_eq!(error["code_name"], MermanStatus::Panic.name());
    }
}

#![forbid(unsafe_code)]

//! Safe shared facade used by external binding crates.
//!
//! This crate owns options parsing, renderer setup, result-code classification, and byte payload
//! production. Unsafe transport concerns such as raw pointers and owned C buffers remain in
//! `merman-ffi`.

use merman::render::{
    DeterministicTextMeasurer, HeadlessRenderer, LayoutOptions, VendoredFontMetricsTextMeasurer,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const SUPPORTED_DIAGRAMS: &[&str] = &[
    "architecture",
    "block",
    "c4",
    "class",
    "er",
    "flowchart",
    "gantt",
    "gitgraph",
    "info",
    "journey",
    "kanban",
    "mindmap",
    "packet",
    "pie",
    "quadrantchart",
    "radar",
    "requirement",
    "sankey",
    "sequence",
    "state",
    "timeline",
    "treemap",
    "xychart",
    "zenuml",
];

pub const ASCII_SUPPORTED_DIAGRAMS: &[&str] = &["class", "er", "flowchart", "sequence", "xychart"];

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingStatus {
    Ok = 0,
    InvalidArgument = 1,
    Utf8Error = 2,
    OptionsJsonError = 3,
    NoDiagram = 4,
    ParseError = 5,
    RenderError = 6,
    UnsupportedFormat = 7,
    Panic = 8,
    InternalError = 9,
}

impl BindingStatus {
    pub const fn code(self) -> i32 {
        self as i32
    }

    pub const fn code_name(self) -> &'static str {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingError {
    status: BindingStatus,
    message: String,
}

impl BindingError {
    pub fn new(status: BindingStatus, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    pub const fn status(&self) -> BindingStatus {
        self.status
    }

    pub fn message(&self) -> &str {
        &self.message
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

#[derive(Debug, Serialize)]
struct ValidationPayload<'a> {
    valid: bool,
    error: Option<String>,
    code: i32,
    code_name: &'a str,
}

#[derive(Debug, Default, Deserialize)]
struct BindingOptions {
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

pub fn render_svg(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let source = source_text(source)?;
    let options = parse_options(options_json)?;
    let (renderer, pipeline) = build_renderer(&options)?;

    let svg = match pipeline {
        PipelineKind::Parity => renderer.render_svg_sync(source),
        PipelineKind::Readable => renderer.render_svg_readable_sync(source),
        PipelineKind::ResvgSafe => renderer.render_svg_resvg_safe_sync(source),
    }
    .map_err(classify_render_error)?;

    match svg {
        Some(svg) => Ok(svg.into_bytes()),
        None => Err(no_diagram_error()),
    }
}

pub fn supported_themes() -> &'static [&'static str] {
    merman::supported_themes()
}

pub fn supported_diagrams() -> &'static [&'static str] {
    SUPPORTED_DIAGRAMS
}

pub fn ascii_supported_diagrams() -> &'static [&'static str] {
    #[cfg(feature = "ascii")]
    {
        ASCII_SUPPORTED_DIAGRAMS
    }
    #[cfg(not(feature = "ascii"))]
    {
        &[]
    }
}

pub fn parse_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let source = source_text(source)?;
    let options = parse_options(options_json)?;
    let (renderer, _pipeline) = build_renderer(&options)?;

    let parsed = renderer
        .parse_diagram_sync(source)
        .map_err(classify_render_error)?
        .ok_or_else(no_diagram_error)?;

    serde_json::to_vec(&parsed.model).map_err(internal_json_error)
}

pub fn layout_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let source = source_text(source)?;
    let options = parse_options(options_json)?;
    let (renderer, _pipeline) = build_renderer(&options)?;

    let layouted = renderer
        .layout_diagram_sync(source)
        .map_err(classify_render_error)?
        .ok_or_else(no_diagram_error)?;

    serde_json::to_vec(&layouted).map_err(internal_json_error)
}

pub fn validate_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let payload = match parse_json(source, options_json) {
        Ok(_) => ValidationPayload {
            valid: true,
            error: None,
            code: BindingStatus::Ok.code(),
            code_name: BindingStatus::Ok.code_name(),
        },
        Err(error) => ValidationPayload {
            valid: false,
            error: Some(error.message().to_string()),
            code: error.status().code(),
            code_name: error.status().code_name(),
        },
    };
    serde_json::to_vec(&payload).map_err(internal_json_error)
}

#[cfg(feature = "ascii")]
pub fn render_ascii(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let source = source_text(source)?;
    let options = parse_options(options_json)?;

    let parse = if options
        .parse
        .as_ref()
        .and_then(|parse| parse.suppress_errors)
        .unwrap_or(false)
    {
        merman::ParseOptions::lenient()
    } else {
        merman::ParseOptions::strict()
    };

    let renderer = merman::ascii::HeadlessAsciiRenderer::new()
        .with_parse_options(parse)
        .with_ascii_options(merman::ascii::AsciiRenderOptions::unicode());

    let rendered = renderer
        .render_ascii_sync(source)
        .map_err(classify_ascii_error)?
        .ok_or_else(no_diagram_error)?;

    Ok(rendered.into_bytes())
}

pub fn supported_diagrams_json() -> Result<Vec<u8>, BindingError> {
    serde_json::to_vec(supported_diagrams()).map_err(internal_json_error)
}

pub fn ascii_supported_diagrams_json() -> Result<Vec<u8>, BindingError> {
    serde_json::to_vec(ascii_supported_diagrams()).map_err(internal_json_error)
}

pub fn supported_themes_json() -> Result<Vec<u8>, BindingError> {
    serde_json::to_vec(supported_themes()).map_err(internal_json_error)
}

pub fn error_payload_json_bytes(status: BindingStatus, message: &str) -> Vec<u8> {
    let payload = ErrorPayload {
        version: 1,
        ok: false,
        code: status.code(),
        code_name: status.code_name(),
        message,
    };
    serde_json::to_vec(&payload).unwrap_or_else(|_| {
        format!(
            r#"{{"version":1,"ok":false,"code":{},"code_name":"{}","message":"internal error payload serialization failed"}}"#,
            BindingStatus::InternalError.code(),
            BindingStatus::InternalError.code_name()
        )
        .into_bytes()
    })
}

fn parse_options(bytes: &[u8]) -> Result<BindingOptions, BindingError> {
    if bytes.is_empty() {
        return Ok(BindingOptions::default());
    }
    let text = std::str::from_utf8(bytes).map_err(|err| {
        BindingError::new(
            BindingStatus::Utf8Error,
            format!("invalid options_json UTF-8: {err}"),
        )
    })?;
    serde_json::from_str(text).map_err(|err| {
        BindingError::new(
            BindingStatus::OptionsJsonError,
            format!("invalid options_json: {err}"),
        )
    })
}

fn source_text(bytes: &[u8]) -> Result<&str, BindingError> {
    let source = std::str::from_utf8(bytes).map_err(|err| {
        BindingError::new(
            BindingStatus::Utf8Error,
            format!("invalid source UTF-8: {err}"),
        )
    })?;
    if source.trim().is_empty() {
        return Err(no_diagram_error());
    }
    Ok(source)
}

fn build_renderer(
    options: &BindingOptions,
) -> Result<(HeadlessRenderer, PipelineKind), BindingError> {
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
                    return Err(BindingError::new(
                        BindingStatus::InvalidArgument,
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
                    renderer = renderer
                        .with_math_renderer(Arc::new(merman_render::math::RatexMathRenderer));
                }
                #[cfg(not(feature = "ratex-math"))]
                {
                    return Err(BindingError::new(
                        BindingStatus::UnsupportedFormat,
                        "layout.math_renderer=ratex requires the ratex-math feature",
                    ));
                }
            }
            other => {
                return Err(BindingError::new(
                    BindingStatus::InvalidArgument,
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
                    return Err(BindingError::new(
                        BindingStatus::InvalidArgument,
                        format!("unsupported svg.pipeline: {other}"),
                    ));
                }
            };
        }
    }

    Ok((renderer, pipeline))
}

fn classify_render_error(err: merman::render::HeadlessError) -> BindingError {
    match err {
        merman::render::HeadlessError::Parse(err) => {
            BindingError::new(BindingStatus::ParseError, err.to_string())
        }
        merman::render::HeadlessError::Render(err) => {
            BindingError::new(BindingStatus::RenderError, err.to_string())
        }
    }
}

#[cfg(feature = "ascii")]
fn classify_ascii_error(err: merman::ascii::HeadlessAsciiError) -> BindingError {
    match err {
        merman::ascii::HeadlessAsciiError::Parse(err) => {
            BindingError::new(BindingStatus::ParseError, err.to_string())
        }
        merman::ascii::HeadlessAsciiError::Ascii(err) => match err {
            merman::ascii::AsciiError::InvalidOption { .. } => {
                BindingError::new(BindingStatus::InvalidArgument, err.to_string())
            }
            merman::ascii::AsciiError::UnsupportedDiagram { .. }
            | merman::ascii::AsciiError::UnsupportedFeature { .. } => {
                BindingError::new(BindingStatus::UnsupportedFormat, err.to_string())
            }
            merman::ascii::AsciiError::RenderLimitExceeded { .. } => {
                BindingError::new(BindingStatus::RenderError, err.to_string())
            }
            _ => BindingError::new(BindingStatus::RenderError, err.to_string()),
        },
    }
}

fn no_diagram_error() -> BindingError {
    BindingError::new(BindingStatus::NoDiagram, "no Mermaid diagram detected")
}

fn internal_json_error(err: serde_json::Error) -> BindingError {
    BindingError::new(
        BindingStatus::InternalError,
        format!("failed to serialize JSON output: {err}"),
    )
}

fn finite_positive(value: f64, name: &'static str) -> Result<f64, BindingError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        Err(BindingError::new(
            BindingStatus::InvalidArgument,
            format!("{name} must be a finite positive number"),
        ))
    }
}

fn normalize_option(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn render_svg_returns_svg_for_flowchart() {
        let svg =
            String::from_utf8(render_svg(b"flowchart TD\nA[Hello] --> B[World]", b"").unwrap())
                .unwrap();

        assert!(svg.contains("<svg"));
        assert!(svg.contains("Hello"));
        assert!(svg.contains("World"));
    }

    #[test]
    fn supported_themes_exposes_core_theme_surface() {
        assert_eq!(
            supported_themes(),
            &[
                "default",
                "base",
                "dark",
                "forest",
                "neutral",
                "neo",
                "neo-dark",
                "redux",
                "redux-dark",
                "redux-color",
                "redux-dark-color"
            ]
        );
    }

    #[test]
    fn supported_diagrams_exposes_binding_surface() {
        assert!(supported_diagrams().contains(&"flowchart"));
        assert!(supported_diagrams().contains(&"sequence"));
        assert!(supported_diagrams().contains(&"requirement"));
    }

    #[test]
    fn ascii_supported_diagrams_reflects_feature_surface() {
        if cfg!(feature = "ascii") {
            assert_eq!(
                ascii_supported_diagrams(),
                &["class", "er", "flowchart", "sequence", "xychart"]
            );
        } else {
            assert!(ascii_supported_diagrams().is_empty());
        }
    }

    #[test]
    fn render_svg_accepts_options_json() {
        let options = br#"{
            "layout": { "text_measurer": "deterministic", "viewport_width": 640, "viewport_height": 480 },
            "svg": { "diagram_id": "bindings core diagram", "pipeline": "readable" }
        }"#;
        let svg =
            String::from_utf8(render_svg(b"flowchart TD\nA[Hello]", options).unwrap()).unwrap();

        assert!(svg.contains("id=\"bindings-core-diagram\""));
        assert!(svg.contains("data-merman-foreignobject"));
    }

    #[test]
    fn parse_json_returns_semantic_model() {
        let json: Value = serde_json::from_slice(
            &parse_json(b"flowchart TD\nA[Hello] --> B[World]", b"").unwrap(),
        )
        .unwrap();

        assert_eq!(
            json.get("type").and_then(Value::as_str),
            Some("flowchart-v2")
        );
        assert!(json.get("nodes").and_then(Value::as_array).is_some());
        assert!(json.get("edges").and_then(Value::as_array).is_some());
    }

    #[test]
    fn layout_json_returns_layouted_diagram() {
        let json: Value = serde_json::from_slice(
            &layout_json(b"flowchart TD\nA[Hello] --> B[World]", b"").unwrap(),
        )
        .unwrap();

        assert!(json.get("meta").is_some());
        assert!(json.get("layout").is_some());
    }

    #[test]
    fn validate_json_reports_success_and_errors_without_throwing() {
        let valid: Value =
            serde_json::from_slice(&validate_json(b"flowchart TD\nA[Hello]", b"").unwrap())
                .unwrap();
        assert_eq!(valid["valid"], true);
        assert_eq!(valid["code_name"], BindingStatus::Ok.code_name());
        assert_eq!(valid.get("error"), Some(&Value::Null));

        let invalid: Value = serde_json::from_slice(&validate_json(b"", b"").unwrap()).unwrap();
        assert_eq!(invalid["valid"], false);
        assert_eq!(invalid["code_name"], BindingStatus::NoDiagram.code_name());
        assert!(
            invalid["error"]
                .as_str()
                .unwrap()
                .contains("no Mermaid diagram")
        );
    }

    #[test]
    fn metadata_json_helpers_return_arrays() {
        let diagrams: Value = serde_json::from_slice(&supported_diagrams_json().unwrap()).unwrap();
        let ascii_diagrams: Value =
            serde_json::from_slice(&ascii_supported_diagrams_json().unwrap()).unwrap();
        let themes: Value = serde_json::from_slice(&supported_themes_json().unwrap()).unwrap();

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

    #[cfg(feature = "ascii")]
    #[test]
    fn render_ascii_returns_unicode_text() {
        let text =
            String::from_utf8(render_ascii(b"flowchart TD\nA[Hello] --> B[World]", b"").unwrap())
                .unwrap();

        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn invalid_source_utf8_returns_utf8_error() {
        let err = render_svg(&[0xff], b"").unwrap_err();

        assert_eq!(err.status(), BindingStatus::Utf8Error);
        assert!(err.message().contains("invalid source UTF-8"));
    }

    #[test]
    fn invalid_options_json_returns_options_json_error() {
        let err = render_svg(b"flowchart TD\nA", b"{").unwrap_err();

        assert_eq!(err.status(), BindingStatus::OptionsJsonError);
        assert!(err.message().contains("invalid options_json"));
    }

    #[test]
    fn empty_source_returns_no_diagram() {
        let err = render_svg(b"", b"").unwrap_err();

        assert_eq!(err.status(), BindingStatus::NoDiagram);
    }

    #[test]
    fn invalid_option_value_returns_invalid_argument() {
        let err = render_svg(
            b"flowchart TD\nA",
            br#"{ "layout": { "viewport_width": -1 } }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("layout.viewport_width"));
    }

    #[test]
    fn unsupported_ratex_without_feature_returns_unsupported_format() {
        let result = render_svg(
            b"flowchart TD\nA[Hello]",
            br#"{ "layout": { "math_renderer": "ratex" } }"#,
        );

        if cfg!(feature = "ratex-math") {
            assert!(result.is_ok());
        } else {
            let err = result.unwrap_err();
            assert_eq!(err.status(), BindingStatus::UnsupportedFormat);
        }
    }

    #[test]
    fn error_payload_json_uses_public_code_names() {
        let payload = error_payload_json_bytes(BindingStatus::RenderError, "failed");
        let json: Value = serde_json::from_slice(&payload).unwrap();

        assert_eq!(json["code"], BindingStatus::RenderError.code());
        assert_eq!(json["code_name"], BindingStatus::RenderError.code_name());
        assert_eq!(json["message"], "failed");
    }
}

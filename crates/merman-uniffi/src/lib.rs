#![forbid(unsafe_code)]

//! UniFFI bindings for `merman`.
//!
//! This crate exposes an idiomatic generated-binding surface over `merman-bindings-core`. It does
//! not replace the canonical C ABI in `merman-ffi`.

use merman_bindings_core::{BindingEngine, BindingError, BindingStatus};
#[cfg(feature = "render")]
use merman_bindings_core::{TextMeasurer as CoreTextMeasurer, VendoredFontMetricsTextMeasurer};
use serde_json::Value;
use std::sync::{Arc, OnceLock, RwLock};

pub const MERMAN_UNIFFI_ABI_VERSION: u32 = 2;

static SUPPORTED_DIAGRAMS: OnceLock<Vec<String>> = OnceLock::new();
static ASCII_SUPPORTED_DIAGRAMS: OnceLock<Vec<String>> = OnceLock::new();
static SUPPORTED_THEMES: OnceLock<Vec<String>> = OnceLock::new();
static SUPPORTED_HOST_THEME_PRESETS: OnceLock<Vec<String>> = OnceLock::new();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum MermanError {
    #[error("{code_name}: {message}")]
    Binding {
        code: i32,
        code_name: String,
        message: String,
    },
}

impl MermanError {
    pub fn from_binding(error: BindingError) -> Self {
        let status = error.status();
        Self::Binding {
            code: status.code(),
            code_name: status.code_name().to_string(),
            message: error.message().to_string(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        let status = BindingStatus::InternalError;
        Self::Binding {
            code: status.code(),
            code_name: status.code_name().to_string(),
            message: message.into(),
        }
    }
}

impl From<uniffi::UnexpectedUniFFICallbackError> for MermanError {
    fn from(error: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::internal(format!("host text measurer callback failed: {error}"))
    }
}

#[derive(Debug, Default, uniffi::Object)]
pub struct MermanEngine;

#[derive(uniffi::Object)]
pub struct MermanReusableEngine {
    #[cfg(feature = "render")]
    base: BindingEngine,
    inner: RwLock<BindingEngine>,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanValidationResult {
    pub valid: bool,
    pub error: Option<String>,
    pub code: i32,
    pub code_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanDiagramFamilyCapability {
    pub diagram_type: String,
    pub metadata_id: Option<String>,
    pub has_semantic_parser: bool,
    pub has_render_parser: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MermanTextWrapMode {
    SvgLike,
    SvgLikeSingleRun,
    HtmlLike,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MermanTextDirection {
    Auto,
    Ltr,
    Rtl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MermanTextWhiteSpace {
    Normal,
    Nowrap,
    BreakSpaces,
    PreWrap,
}

#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MermanTextMeasureRequest {
    pub text: String,
    pub font_family: Option<String>,
    pub font_size: f64,
    pub font_weight: Option<String>,
    pub font_style: String,
    pub max_width: Option<f64>,
    pub line_height: f64,
    pub letter_spacing: f64,
    pub word_spacing: f64,
    pub wrap_mode: MermanTextWrapMode,
    pub direction: MermanTextDirection,
    pub white_space: MermanTextWhiteSpace,
}

#[derive(Debug, Clone, Copy, PartialEq, uniffi::Record)]
pub struct MermanTextMeasureResult {
    pub width: f64,
    pub height: f64,
    pub line_count: u64,
}

#[cfg(feature = "render")]
#[uniffi::export(with_foreign)]
pub trait MermanTextMeasurer: Send + Sync {
    fn measure(
        &self,
        request: MermanTextMeasureRequest,
    ) -> Result<Option<MermanTextMeasureResult>, MermanError>;
}

#[cfg(feature = "render")]
struct UniffiHostTextMeasurer {
    callback: Arc<dyn MermanTextMeasurer>,
    fallback: VendoredFontMetricsTextMeasurer,
}

#[cfg(feature = "render")]
impl UniffiHostTextMeasurer {
    fn new(callback: Arc<dyn MermanTextMeasurer>) -> Self {
        Self {
            callback,
            fallback: VendoredFontMetricsTextMeasurer::default(),
        }
    }

    fn call_host(
        &self,
        text: &str,
        style: &merman_bindings_core::TextStyle,
        max_width: Option<f64>,
        wrap_mode: merman_bindings_core::WrapMode,
    ) -> Option<merman_bindings_core::TextMetrics> {
        let result = self
            .callback
            .measure(MermanTextMeasureRequest {
                text: text.to_string(),
                font_family: style.font_family.clone(),
                font_size: style.font_size,
                font_weight: style.font_weight.clone(),
                font_style: "normal".to_string(),
                max_width,
                line_height: uniffi_line_height(style, wrap_mode),
                letter_spacing: 0.0,
                word_spacing: 0.0,
                wrap_mode: uniffi_wrap_mode(wrap_mode),
                direction: MermanTextDirection::Auto,
                white_space: uniffi_white_space(max_width, wrap_mode),
            })
            .ok()
            .flatten()?;

        let Ok(line_count) = usize::try_from(result.line_count) else {
            return None;
        };
        if !result.width.is_finite()
            || !result.height.is_finite()
            || result.width < 0.0
            || result.height < 0.0
            || line_count == 0
        {
            return None;
        }

        Some(merman_bindings_core::TextMetrics {
            width: result.width,
            height: result.height,
            line_count,
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
impl CoreTextMeasurer for UniffiHostTextMeasurer {
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
fn uniffi_wrap_mode(wrap_mode: merman_bindings_core::WrapMode) -> MermanTextWrapMode {
    match wrap_mode {
        merman_bindings_core::WrapMode::SvgLike => MermanTextWrapMode::SvgLike,
        merman_bindings_core::WrapMode::SvgLikeSingleRun => MermanTextWrapMode::SvgLikeSingleRun,
        merman_bindings_core::WrapMode::HtmlLike => MermanTextWrapMode::HtmlLike,
    }
}

#[cfg(feature = "render")]
fn uniffi_line_height(
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
fn uniffi_white_space(
    max_width: Option<f64>,
    wrap_mode: merman_bindings_core::WrapMode,
) -> MermanTextWhiteSpace {
    match wrap_mode {
        merman_bindings_core::WrapMode::HtmlLike if max_width.is_some() => {
            MermanTextWhiteSpace::BreakSpaces
        }
        merman_bindings_core::WrapMode::HtmlLike => MermanTextWhiteSpace::Nowrap,
        merman_bindings_core::WrapMode::SvgLike
        | merman_bindings_core::WrapMode::SvgLikeSingleRun => MermanTextWhiteSpace::Normal,
    }
}

#[uniffi::export]
impl MermanEngine {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }

    pub fn abi_version(&self) -> u32 {
        MERMAN_UNIFFI_ABI_VERSION
    }

    pub fn package_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    pub fn render_svg(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_output(merman_bindings_core::render_svg(
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
        ))
    }

    pub fn render_ascii(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_output(merman_bindings_core::render_ascii(
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
        ))
    }

    pub fn parse_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_output(merman_bindings_core::parse_json(
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
        ))
    }

    pub fn layout_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_output(merman_bindings_core::layout_json(
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
        ))
    }

    pub fn validate(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<MermanValidationResult, MermanError> {
        validation_output(merman_bindings_core::validate_json(
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
        ))
    }

    pub fn reusable_engine(
        &self,
        options_json: Option<String>,
    ) -> Result<Arc<MermanReusableEngine>, MermanError> {
        MermanReusableEngine::new(options_json)
    }

    pub fn supported_diagrams(&self) -> Vec<String> {
        cached_string_vec(
            &SUPPORTED_DIAGRAMS,
            merman_bindings_core::supported_diagrams,
        )
    }

    pub fn ascii_supported_diagrams(&self) -> Vec<String> {
        cached_string_vec(
            &ASCII_SUPPORTED_DIAGRAMS,
            merman_bindings_core::ascii_supported_diagrams,
        )
    }

    pub fn supported_themes(&self) -> Vec<String> {
        cached_string_vec(&SUPPORTED_THEMES, merman_bindings_core::supported_themes)
    }

    pub fn supported_host_theme_presets(&self) -> Vec<String> {
        cached_string_vec(
            &SUPPORTED_HOST_THEME_PRESETS,
            merman_bindings_core::supported_host_theme_presets,
        )
    }

    pub fn diagram_family_capabilities(&self) -> Vec<MermanDiagramFamilyCapability> {
        merman_bindings_core::diagram_family_capabilities()
            .into_iter()
            .map(|capability| MermanDiagramFamilyCapability {
                diagram_type: capability.diagram_type.to_string(),
                metadata_id: capability.metadata_id.map(str::to_string),
                has_semantic_parser: capability.has_semantic_parser,
                has_render_parser: capability.has_render_parser,
            })
            .collect()
    }
}

#[uniffi::export]
impl MermanReusableEngine {
    #[uniffi::constructor]
    pub fn new(options_json: Option<String>) -> Result<Arc<Self>, MermanError> {
        let inner = BindingEngine::new(options_bytes(options_json.as_deref()))
            .map_err(MermanError::from_binding)?;
        Ok(Arc::new(Self {
            #[cfg(feature = "render")]
            base: inner.clone(),
            inner: RwLock::new(inner),
        }))
    }

    pub fn render_svg(&self, source: String) -> Result<String, MermanError> {
        let inner = self.current_inner()?;
        string_output(inner.render_svg(source.as_bytes()))
    }

    pub fn render_ascii(&self, source: String) -> Result<String, MermanError> {
        let inner = self.current_inner()?;
        string_output(inner.render_ascii(source.as_bytes()))
    }

    pub fn parse_json(&self, source: String) -> Result<String, MermanError> {
        let inner = self.current_inner()?;
        string_output(inner.parse_json(source.as_bytes()))
    }

    pub fn layout_json(&self, source: String) -> Result<String, MermanError> {
        let inner = self.current_inner()?;
        string_output(inner.layout_json(source.as_bytes()))
    }

    pub fn validate(&self, source: String) -> Result<MermanValidationResult, MermanError> {
        let inner = self.current_inner()?;
        validation_output(inner.validate_json(source.as_bytes()))
    }
}

#[cfg(feature = "render")]
#[uniffi::export]
impl MermanEngine {
    pub fn reusable_engine_with_text_measurer(
        &self,
        options_json: Option<String>,
        measurer: Arc<dyn MermanTextMeasurer>,
    ) -> Result<Arc<MermanReusableEngine>, MermanError> {
        MermanReusableEngine::with_text_measurer(options_json, measurer)
    }
}

#[cfg(feature = "render")]
#[uniffi::export]
impl MermanReusableEngine {
    #[uniffi::constructor]
    pub fn with_text_measurer(
        options_json: Option<String>,
        measurer: Arc<dyn MermanTextMeasurer>,
    ) -> Result<Arc<Self>, MermanError> {
        let base = BindingEngine::new(options_bytes(options_json.as_deref()))
            .map_err(MermanError::from_binding)?;
        let inner = base.with_text_measurer(Arc::new(UniffiHostTextMeasurer::new(measurer)));
        Ok(Arc::new(Self {
            base,
            inner: RwLock::new(inner),
        }))
    }

    pub fn set_text_measurer(
        &self,
        measurer: Arc<dyn MermanTextMeasurer>,
    ) -> Result<(), MermanError> {
        self.replace_inner(
            self.base
                .with_text_measurer(Arc::new(UniffiHostTextMeasurer::new(measurer))),
        )
    }

    pub fn clear_text_measurer(&self) -> Result<(), MermanError> {
        self.replace_inner(self.base.clone())
    }
}

impl MermanReusableEngine {
    fn current_inner(&self) -> Result<BindingEngine, MermanError> {
        self.inner
            .read()
            .map(|inner| inner.clone())
            .map_err(|_| MermanError::internal("reusable engine lock poisoned"))
    }

    #[cfg(feature = "render")]
    fn replace_inner(&self, next_inner: BindingEngine) -> Result<(), MermanError> {
        let mut inner = self
            .inner
            .write()
            .map_err(|_| MermanError::internal("reusable engine lock poisoned"))?;
        *inner = next_inner;
        Ok(())
    }
}

fn options_bytes(options_json: Option<&str>) -> &[u8] {
    options_json.unwrap_or_default().as_bytes()
}

fn string_output(result: Result<Vec<u8>, BindingError>) -> Result<String, MermanError> {
    let bytes = result.map_err(MermanError::from_binding)?;
    String::from_utf8(bytes)
        .map_err(|err| MermanError::internal(format!("binding output was not UTF-8: {err}")))
}

fn validation_output(
    result: Result<Vec<u8>, BindingError>,
) -> Result<MermanValidationResult, MermanError> {
    let bytes = result.map_err(MermanError::from_binding)?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|err| MermanError::internal(format!("validation JSON decode failed: {err}")))?;
    let object = value
        .as_object()
        .ok_or_else(|| MermanError::internal("validation JSON was not an object"))?;
    let valid = object
        .get("valid")
        .and_then(Value::as_bool)
        .ok_or_else(|| MermanError::internal("validation JSON missing valid"))?;
    let code = object
        .get("code")
        .and_then(Value::as_i64)
        .ok_or_else(|| MermanError::internal("validation JSON missing code"))?;
    let code_name = object
        .get("code_name")
        .and_then(Value::as_str)
        .ok_or_else(|| MermanError::internal("validation JSON missing code_name"))?;
    let error = object
        .get("error")
        .and_then(Value::as_str)
        .map(str::to_string);

    Ok(MermanValidationResult {
        valid,
        error,
        code: code as i32,
        code_name: code_name.to_string(),
    })
}

fn string_vec(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn cached_string_vec(
    cache: &OnceLock<Vec<String>>,
    values: fn() -> &'static [&'static str],
) -> Vec<String> {
    cache.get_or_init(|| string_vec(values())).clone()
}

uniffi::setup_scaffolding!();

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    #[cfg(feature = "render")]
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn engine() -> Arc<MermanEngine> {
        MermanEngine::new()
    }

    #[cfg(feature = "render")]
    struct CountingTextMeasurer {
        calls: AtomicUsize,
    }

    #[cfg(feature = "render")]
    impl CountingTextMeasurer {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                calls: AtomicUsize::new(0),
            })
        }

        fn calls(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[cfg(feature = "render")]
    impl MermanTextMeasurer for CountingTextMeasurer {
        fn measure(
            &self,
            request: MermanTextMeasureRequest,
        ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            assert!(request.font_size.is_finite());
            assert!(request.line_height.is_finite());
            Ok(Some(MermanTextMeasureResult {
                width: (request.text.chars().count() as f64 * 9.0).max(1.0),
                height: request.line_height.max(1.0),
                line_count: 1,
            }))
        }
    }

    #[test]
    fn engine_renders_svg() {
        let svg = engine()
            .render_svg("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
            .unwrap();

        assert!(svg.contains("<svg"));
        assert!(svg.contains("Hello"));
        assert!(svg.contains("World"));
    }

    #[test]
    fn engine_exposes_versions() {
        let engine = engine();

        assert_eq!(engine.abi_version(), MERMAN_UNIFFI_ABI_VERSION);
        assert_eq!(engine.package_version(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn engine_accepts_options_json() {
        let svg = engine()
            .render_svg(
                "flowchart TD\nA[Hello]".to_string(),
                Some(
                    r#"{
                        "layout": { "text_measurer": "deterministic" },
                        "svg": { "diagram_id": "uniffi diagram", "pipeline": "readable" }
                    }"#
                    .to_string(),
                ),
            )
            .unwrap();

        assert!(svg.contains("id=\"uniffi-diagram\""));
        assert!(svg.contains("data-merman-foreignobject"));
    }

    #[test]
    fn engine_renders_ascii() {
        let text = engine()
            .render_ascii("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
            .unwrap();

        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn engine_returns_semantic_json() {
        let json: Value = serde_json::from_str(
            &engine()
                .parse_json("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
                .unwrap(),
        )
        .unwrap();

        assert_eq!(
            json.get("type").and_then(Value::as_str),
            Some("flowchart-v2")
        );
    }

    #[test]
    fn engine_returns_layout_json() {
        let json: Value = serde_json::from_str(
            &engine()
                .layout_json("flowchart TD\nA[Hello] --> B[World]".to_string(), None)
                .unwrap(),
        )
        .unwrap();

        assert!(json.get("meta").is_some());
        assert!(json.get("layout").is_some());
    }

    #[test]
    fn engine_validates_source() {
        let result = engine()
            .validate("flowchart TD\nA[Hello]".to_string(), None)
            .unwrap();

        assert!(result.valid);
        assert_eq!(result.code_name, BindingStatus::Ok.code_name());

        let result = engine().validate("".to_string(), None).unwrap();
        assert!(!result.valid);
        assert_eq!(result.code_name, BindingStatus::NoDiagram.code_name());
        assert!(result.error.unwrap().contains("no Mermaid diagram"));
    }

    #[test]
    fn engine_exposes_metadata() {
        let engine = engine();

        assert!(
            engine
                .supported_diagrams()
                .contains(&"flowchart".to_string())
        );
        let ascii_supported_diagrams = engine.ascii_supported_diagrams();
        for diagram in ["sequence", "gantt", "treeView"] {
            assert!(
                ascii_supported_diagrams.contains(&diagram.to_string()),
                "expected UniFFI ASCII metadata to include {diagram}"
            );
        }
        assert!(engine.supported_themes().contains(&"default".to_string()));
        assert!(
            engine
                .supported_host_theme_presets()
                .contains(&"one-dark".to_string())
        );
        let capabilities = engine.diagram_family_capabilities();
        assert!(
            capabilities
                .iter()
                .any(|capability| capability.diagram_type == "flowchart"
                    && capability.has_semantic_parser
                    && capability.has_render_parser)
        );
    }

    #[test]
    fn reusable_engine_reuses_options() {
        let reusable = MermanReusableEngine::new(Some(
            r#"{
                "layout": { "text_measurer": "deterministic" },
                "svg": { "diagram_id": "uniffi reusable", "pipeline": "readable" }
            }"#
            .to_string(),
        ))
        .unwrap();

        let svg = reusable
            .render_svg("flowchart TD\nA[Hello]".to_string())
            .unwrap();
        assert!(svg.contains("id=\"uniffi-reusable\""));
        assert!(svg.contains("data-merman-foreignobject"));
    }

    #[cfg(feature = "render")]
    #[test]
    fn reusable_engine_uses_host_text_measurer() {
        let measurer = CountingTextMeasurer::new();
        let reusable = MermanReusableEngine::with_text_measurer(None, measurer.clone()).unwrap();

        let svg = reusable
            .render_svg("flowchart TD\nA[Measured label] --> B[Done]".to_string())
            .unwrap();
        assert!(svg.contains("<svg"));
        assert!(measurer.calls() > 0);
    }

    #[cfg(feature = "render")]
    #[test]
    fn reusable_engine_can_set_and_clear_host_text_measurer() {
        let reusable = MermanReusableEngine::new(None).unwrap();
        let measurer = CountingTextMeasurer::new();

        reusable.set_text_measurer(measurer.clone()).unwrap();
        let svg = reusable
            .render_svg("flowchart TD\nA[Measured label] --> B[Done]".to_string())
            .unwrap();
        assert!(svg.contains("<svg"));
        let calls_after_set = measurer.calls();
        assert!(calls_after_set > 0);

        reusable.clear_text_measurer().unwrap();
        let svg = reusable
            .render_svg("flowchart TD\nA[Measured label] --> B[Done]".to_string())
            .unwrap();
        assert!(svg.contains("<svg"));
        assert_eq!(measurer.calls(), calls_after_set);
    }

    #[test]
    fn engine_error_preserves_binding_status() {
        let err = engine()
            .render_svg("flowchart TD\nA".to_string(), Some("{".to_string()))
            .unwrap_err();

        let MermanError::Binding {
            code,
            code_name,
            message,
        } = err;
        assert_eq!(code, BindingStatus::OptionsJsonError.code());
        assert_eq!(code_name, BindingStatus::OptionsJsonError.code_name());
        assert!(message.contains("invalid options_json"));
    }
}

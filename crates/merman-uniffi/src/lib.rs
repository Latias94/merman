#![forbid(unsafe_code)]

//! UniFFI bindings for `merman`.
//!
//! This crate exposes an idiomatic generated-binding surface over `merman-bindings-core`. It does
//! not replace the canonical C ABI in `merman-ffi`.

use merman_bindings_core::{BindingEngine, BindingError, BindingStatus};
#[cfg(feature = "render")]
use merman_bindings_core::{TextMeasurer as CoreTextMeasurer, VendoredFontMetricsTextMeasurer};
use serde_json::Value;
#[cfg(feature = "render")]
use std::cell::RefCell;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

pub const MERMAN_UNIFFI_ABI_VERSION: u32 = 2;

static SUPPORTED_DIAGRAMS: OnceLock<Vec<String>> = OnceLock::new();
static ASCII_CAPABILITIES: OnceLock<Vec<MermanAsciiCapability>> = OnceLock::new();
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
    #[cfg(feature = "render")]
    render_lock: Mutex<()>,
    #[cfg(feature = "render")]
    host_text_measurer: RwLock<Option<Arc<UniffiHostTextMeasurer>>>,
    inner: RwLock<BindingEngine>,
}

#[cfg(feature = "render")]
thread_local! {
    static REUSABLE_RENDER_STACK: RefCell<Vec<usize>> = RefCell::new(Vec::new());
}

#[cfg(feature = "render")]
struct ReusableRenderGuard {
    engine_id: usize,
}

#[cfg(feature = "render")]
impl Drop for ReusableRenderGuard {
    fn drop(&mut self) {
        REUSABLE_RENDER_STACK.with(|stack| {
            let mut stack = stack.borrow_mut();
            if let Some(position) = stack
                .iter()
                .rposition(|engine_id| *engine_id == self.engine_id)
            {
                stack.remove(position);
            }
        });
    }
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

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanLintRuleCatalogEntry {
    pub id: String,
    pub description: String,
    pub evidence: Vec<String>,
    pub default_severity: String,
    pub category: String,
    pub default_enabled: bool,
    pub default_profile: String,
    pub origin: String,
    pub configurable: bool,
    pub fixable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanAsciiCapabilityEvidence {
    pub kind: String,
    pub source: String,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MermanAsciiCapability {
    pub diagram_type: String,
    pub display_name: String,
    pub support_level: String,
    pub summary_fallback: bool,
    pub supported_semantics: Vec<String>,
    pub limits: Vec<String>,
    pub evidence: Vec<MermanAsciiCapabilityEvidence>,
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
    callback_error: RwLock<Option<MermanError>>,
}

#[cfg(feature = "render")]
impl UniffiHostTextMeasurer {
    fn new(callback: Arc<dyn MermanTextMeasurer>) -> Self {
        Self {
            callback,
            fallback: VendoredFontMetricsTextMeasurer::default(),
            callback_error: RwLock::new(None),
        }
    }

    fn record_callback_error(&self, error: MermanError) {
        if let Ok(mut callback_error) = self.callback_error.write()
            && callback_error.is_none()
        {
            *callback_error = Some(error);
        }
    }

    fn take_callback_error(&self) -> Result<Option<MermanError>, MermanError> {
        self.callback_error
            .write()
            .map(|mut callback_error| callback_error.take())
            .map_err(|_| MermanError::internal("host text measurer error lock poisoned"))
    }

    fn clear_callback_error(&self) -> Result<(), MermanError> {
        self.callback_error
            .write()
            .map(|mut callback_error| {
                *callback_error = None;
            })
            .map_err(|_| MermanError::internal("host text measurer error lock poisoned"))
    }

    fn call_host(
        &self,
        text: &str,
        style: &merman_bindings_core::TextStyle,
        max_width: Option<f64>,
        wrap_mode: merman_bindings_core::WrapMode,
    ) -> Option<merman_bindings_core::TextMetrics> {
        let result = match self.callback.measure(MermanTextMeasureRequest {
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
        }) {
            Ok(Some(result)) => result,
            Ok(None) => return None,
            Err(error) => {
                self.record_callback_error(error);
                return None;
            }
        };

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

fn uniffi_lint_rule(rule: merman_bindings_core::RuleCatalogEntry) -> MermanLintRuleCatalogEntry {
    MermanLintRuleCatalogEntry {
        id: rule.id.to_string(),
        description: rule.description.to_string(),
        evidence: rule
            .evidence
            .iter()
            .map(|evidence| evidence.to_string())
            .collect(),
        default_severity: rule.default_severity.as_str().to_string(),
        category: rule.category.as_str().to_string(),
        default_enabled: rule.default_enabled,
        default_profile: rule.default_profile.as_str().to_string(),
        origin: rule.origin.as_str().to_string(),
        configurable: rule.configurable,
        fixable: rule.fixable,
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

    pub fn analyze_json(
        &self,
        source: String,
        options_json: Option<String>,
    ) -> Result<String, MermanError> {
        string_output(merman_bindings_core::analyze_json(
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
        ))
    }

    pub fn analyze_document_json(
        &self,
        source: String,
        options_json: Option<String>,
        uri: String,
    ) -> Result<String, MermanError> {
        string_output(merman_bindings_core::analyze_document_json(
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
            uri.as_bytes(),
        ))
    }

    pub fn analyze_document_facts_json(
        &self,
        source: String,
        options_json: Option<String>,
        uri: String,
    ) -> Result<String, MermanError> {
        string_output(merman_bindings_core::analyze_document_facts_json(
            source.as_bytes(),
            options_bytes(options_json.as_deref()),
            uri.as_bytes(),
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

    pub fn ascii_capabilities(&self) -> Vec<MermanAsciiCapability> {
        ASCII_CAPABILITIES
            .get_or_init(|| {
                merman_bindings_core::ascii_capabilities()
                    .into_iter()
                    .map(|capability| MermanAsciiCapability {
                        diagram_type: capability.diagram_type.to_string(),
                        display_name: capability.display_name.to_string(),
                        support_level: capability.support_level.to_string(),
                        summary_fallback: capability.summary_fallback,
                        supported_semantics: capability
                            .supported_semantics
                            .iter()
                            .map(|semantic| (*semantic).to_string())
                            .collect(),
                        limits: capability
                            .limits
                            .iter()
                            .map(|limit| (*limit).to_string())
                            .collect(),
                        evidence: capability
                            .evidence
                            .into_iter()
                            .map(|evidence| MermanAsciiCapabilityEvidence {
                                kind: evidence.kind.to_string(),
                                source: evidence.source.to_string(),
                                note: evidence.note.to_string(),
                            })
                            .collect(),
                    })
                    .collect()
            })
            .clone()
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

    pub fn lint_rule_catalog(&self) -> Vec<MermanLintRuleCatalogEntry> {
        merman_bindings_core::lint_rule_catalog()
            .into_iter()
            .map(uniffi_lint_rule)
            .collect()
    }

    pub fn configurable_lint_rule_catalog(&self) -> Vec<MermanLintRuleCatalogEntry> {
        merman_bindings_core::configurable_lint_rule_catalog()
            .into_iter()
            .map(uniffi_lint_rule)
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
            #[cfg(feature = "render")]
            render_lock: Mutex::new(()),
            #[cfg(feature = "render")]
            host_text_measurer: RwLock::new(None),
            inner: RwLock::new(inner),
        }))
    }

    pub fn render_svg(&self, source: String) -> Result<String, MermanError> {
        #[cfg(feature = "render")]
        {
            return self.render_svg_with_render_lock(source);
        }
        #[cfg(not(feature = "render"))]
        {
            let inner = self.current_inner()?;
            string_output(inner.render_svg(source.as_bytes()))
        }
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
        #[cfg(feature = "render")]
        {
            return self.layout_json_with_render_lock(source);
        }
        #[cfg(not(feature = "render"))]
        {
            let inner = self.current_inner()?;
            string_output(inner.layout_json(source.as_bytes()))
        }
    }

    pub fn analyze_json(&self, source: String) -> Result<String, MermanError> {
        let inner = self.current_inner()?;
        string_output(inner.analyze_json(source.as_bytes()))
    }

    pub fn analyze_document_json(
        &self,
        source: String,
        uri: String,
    ) -> Result<String, MermanError> {
        let inner = self.current_inner()?;
        string_output(inner.analyze_document_json(source.as_bytes(), uri.as_bytes()))
    }

    pub fn analyze_document_facts_json(
        &self,
        source: String,
        uri: String,
    ) -> Result<String, MermanError> {
        let inner = self.current_inner()?;
        string_output(inner.analyze_document_facts_json(source.as_bytes(), uri.as_bytes()))
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
        let host_text_measurer = Arc::new(UniffiHostTextMeasurer::new(measurer));
        let inner = base.with_text_measurer(host_text_measurer.clone());
        Ok(Arc::new(Self {
            base,
            render_lock: Mutex::new(()),
            host_text_measurer: RwLock::new(Some(host_text_measurer)),
            inner: RwLock::new(inner),
        }))
    }

    pub fn set_text_measurer(
        &self,
        measurer: Arc<dyn MermanTextMeasurer>,
    ) -> Result<(), MermanError> {
        let host_text_measurer = Arc::new(UniffiHostTextMeasurer::new(measurer));
        self.replace_render_inner(
            self.base.with_text_measurer(host_text_measurer.clone()),
            Some(host_text_measurer),
        )
    }

    pub fn clear_text_measurer(&self) -> Result<(), MermanError> {
        self.replace_render_inner(self.base.clone(), None)
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
    fn replace_render_inner(
        &self,
        next_inner: BindingEngine,
        next_host_text_measurer: Option<Arc<UniffiHostTextMeasurer>>,
    ) -> Result<(), MermanError> {
        self.ensure_not_reentrant_render_call()?;
        let _guard = self
            .render_lock
            .lock()
            .map_err(|_| MermanError::internal("reusable engine render lock poisoned"))?;
        let mut inner = self
            .inner
            .write()
            .map_err(|_| MermanError::internal("reusable engine lock poisoned"))?;
        let mut host_text_measurer = self.host_text_measurer.write().map_err(|_| {
            MermanError::internal("reusable engine host text measurer lock poisoned")
        })?;
        *inner = next_inner;
        *host_text_measurer = next_host_text_measurer;
        Ok(())
    }

    #[cfg(feature = "render")]
    fn current_host_text_measurer(
        &self,
    ) -> Result<Option<Arc<UniffiHostTextMeasurer>>, MermanError> {
        self.host_text_measurer
            .read()
            .map(|host_text_measurer| host_text_measurer.clone())
            .map_err(|_| MermanError::internal("reusable engine host text measurer lock poisoned"))
    }

    #[cfg(feature = "render")]
    fn render_svg_with_render_lock(&self, source: String) -> Result<String, MermanError> {
        self.with_render_call_host(|inner| string_output(inner.render_svg(source.as_bytes())))
    }

    #[cfg(feature = "render")]
    fn layout_json_with_render_lock(&self, source: String) -> Result<String, MermanError> {
        self.with_render_call_host(|inner| string_output(inner.layout_json(source.as_bytes())))
    }

    #[cfg(feature = "render")]
    fn with_render_call_host<T>(
        &self,
        run: impl FnOnce(BindingEngine) -> Result<T, MermanError>,
    ) -> Result<T, MermanError> {
        let _reentry_guard = self.enter_render_call()?;
        let _guard = self
            .render_lock
            .lock()
            .map_err(|_| MermanError::internal("reusable engine render lock poisoned"))?;
        let inner = self.current_inner()?;
        let host_text_measurer = self.current_host_text_measurer()?;
        if let Some(host_text_measurer) = &host_text_measurer {
            host_text_measurer.clear_callback_error()?;
        }

        let output = run(inner);

        if let Some(host_text_measurer) = &host_text_measurer
            && let Some(error) = host_text_measurer.take_callback_error()?
        {
            return Err(error);
        }
        output
    }

    #[cfg(feature = "render")]
    fn enter_render_call(&self) -> Result<ReusableRenderGuard, MermanError> {
        let engine_id = self.render_engine_id();
        REUSABLE_RENDER_STACK.with(|stack| {
            let mut stack = stack.borrow_mut();
            if stack.contains(&engine_id) {
                return Err(reentrant_render_error());
            }
            stack.push(engine_id);
            Ok(ReusableRenderGuard { engine_id })
        })
    }

    #[cfg(feature = "render")]
    fn ensure_not_reentrant_render_call(&self) -> Result<(), MermanError> {
        let engine_id = self.render_engine_id();
        REUSABLE_RENDER_STACK.with(|stack| {
            if stack.borrow().contains(&engine_id) {
                Err(reentrant_render_error())
            } else {
                Ok(())
            }
        })
    }

    #[cfg(feature = "render")]
    fn render_engine_id(&self) -> usize {
        self as *const Self as usize
    }
}

#[cfg(feature = "render")]
fn reentrant_render_error() -> MermanError {
    MermanError::internal("reentrant reusable engine render call from host text measurer")
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
    #[cfg(feature = "render")]
    use std::sync::{Condvar, Mutex as StdMutex, mpsc};
    #[cfg(feature = "render")]
    use std::thread;
    #[cfg(feature = "render")]
    use std::time::Duration;

    fn engine() -> Arc<MermanEngine> {
        MermanEngine::new()
    }

    #[cfg(feature = "render")]
    struct CountingTextMeasurer {
        calls: AtomicUsize,
    }

    #[cfg(feature = "render")]
    struct FailingTextMeasurer {
        calls: AtomicUsize,
    }

    #[cfg(feature = "render")]
    struct MissingTextMeasurer {
        calls: AtomicUsize,
    }

    #[cfg(feature = "render")]
    struct BlockingFailingTextMeasurer {
        state: (StdMutex<BlockingFailingTextMeasurerState>, Condvar),
    }

    #[cfg(feature = "render")]
    struct ReentrantTextMeasurer {
        engine: StdMutex<Option<Arc<MermanReusableEngine>>>,
    }

    #[cfg(feature = "render")]
    struct BlockingFailingTextMeasurerState {
        entered: bool,
        released: bool,
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
    impl FailingTextMeasurer {
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
    impl MissingTextMeasurer {
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
    impl BlockingFailingTextMeasurer {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                state: (
                    StdMutex::new(BlockingFailingTextMeasurerState {
                        entered: false,
                        released: false,
                    }),
                    Condvar::new(),
                ),
            })
        }

        fn wait_until_entered(&self) {
            let (lock, cvar) = &self.state;
            let mut state = lock.lock().unwrap();
            while !state.entered {
                state = cvar.wait(state).unwrap();
            }
        }

        fn release(&self) {
            let (lock, cvar) = &self.state;
            let mut state = lock.lock().unwrap();
            state.released = true;
            cvar.notify_all();
        }
    }

    #[cfg(feature = "render")]
    impl ReentrantTextMeasurer {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                engine: StdMutex::new(None),
            })
        }

        fn set_engine(&self, engine: Arc<MermanReusableEngine>) {
            *self.engine.lock().unwrap() = Some(engine);
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

    #[cfg(feature = "render")]
    impl MermanTextMeasurer for FailingTextMeasurer {
        fn measure(
            &self,
            _request: MermanTextMeasureRequest,
        ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(MermanError::internal("test host measurer failed"))
        }
    }

    #[cfg(feature = "render")]
    impl MermanTextMeasurer for MissingTextMeasurer {
        fn measure(
            &self,
            _request: MermanTextMeasureRequest,
        ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        }
    }

    #[cfg(feature = "render")]
    impl MermanTextMeasurer for BlockingFailingTextMeasurer {
        fn measure(
            &self,
            _request: MermanTextMeasureRequest,
        ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
            let (lock, cvar) = &self.state;
            let mut state = lock.lock().unwrap();
            state.entered = true;
            cvar.notify_all();
            while !state.released {
                state = cvar.wait(state).unwrap();
            }
            Err(MermanError::internal("blocked host measurer failed"))
        }
    }

    #[cfg(feature = "render")]
    impl MermanTextMeasurer for ReentrantTextMeasurer {
        fn measure(
            &self,
            _request: MermanTextMeasureRequest,
        ) -> Result<Option<MermanTextMeasureResult>, MermanError> {
            let engine = self
                .engine
                .lock()
                .unwrap()
                .as_ref()
                .expect("reentrant measurer should have an engine")
                .clone();
            Err(engine
                .clear_text_measurer()
                .expect_err("reentrant clear should fail before taking the render lock"))
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
    fn engine_returns_analysis_json() {
        let json: Value = serde_json::from_str(
            &engine()
                .analyze_json("flowchart TD\nA[Hello]".to_string(), None)
                .unwrap(),
        )
        .unwrap();

        assert_eq!(json["version"], 1);
        assert_eq!(json["valid"], true);
    }

    #[test]
    fn engine_returns_document_analysis_json() {
        let source = "# Example\n\n```mermaid\nflowchart TD\nA[Hello]\n```\n";
        let json: Value = serde_json::from_str(
            &engine()
                .analyze_document_json(
                    source.to_string(),
                    None,
                    "file:///tmp/example.md".to_string(),
                )
                .unwrap(),
        )
        .unwrap();

        assert_eq!(json["version"], 1);
        assert_eq!(json["source"]["kind"], "markdown");
        assert_eq!(json["valid"], true);
    }

    #[test]
    fn engine_returns_document_facts_json() {
        let source = "# Example\n\n```mermaid\nflowchart TD\nA[Hello]\n```\n";
        let json: Value = serde_json::from_str(
            &engine()
                .analyze_document_facts_json(
                    source.to_string(),
                    None,
                    "file:///tmp/example.md".to_string(),
                )
                .unwrap(),
        )
        .unwrap();

        assert_eq!(json["version"], 1);
        assert_eq!(json["source"]["kind"], "markdown");
        assert_eq!(json["diagrams"][0]["source_id"], "mermaid-fence-1");
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
        let ascii_capabilities = engine.ascii_capabilities();
        let sequence = ascii_capabilities
            .iter()
            .find(|capability| capability.diagram_type == "sequence")
            .expect("expected UniFFI ASCII capabilities to include sequence");
        assert_eq!(sequence.support_level, "full");

        let gantt = ascii_capabilities
            .iter()
            .find(|capability| capability.diagram_type == "gantt")
            .expect("expected UniFFI ASCII capabilities to include gantt");
        assert_eq!(gantt.support_level, "summary");
        assert!(!gantt.summary_fallback);

        let class = ascii_capabilities
            .iter()
            .find(|capability| capability.diagram_type == "class")
            .expect("expected UniFFI ASCII capabilities to include class");
        assert_eq!(class.support_level, "partial");
        assert!(class.summary_fallback);
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
        let lint_rules = engine.lint_rule_catalog();
        assert!(lint_rules.iter().any(|rule| {
            rule.id == "merman.authoring.flowchart.explicit_direction"
                && rule.origin == "merman_authoring"
                && rule.default_profile == "recommended"
                && rule
                    .evidence
                    .contains(&"docs/adr/0072-lint-rule-governance.md".to_string())
        }));
        assert!(
            engine
                .configurable_lint_rule_catalog()
                .iter()
                .all(|rule| rule.configurable && rule.category != "internal")
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

    #[test]
    fn reusable_engine_returns_document_analysis_json() {
        let reusable = MermanReusableEngine::new(Some(
            r#"{ "analysis": { "profile": "strict" } }"#.to_string(),
        ))
        .unwrap();
        let source = "# Example\n\n```mermaid\nflowchart TD\nA[Hello]\n```\n";
        let json: Value = serde_json::from_str(
            &reusable
                .analyze_document_json(source.to_string(), "file:///tmp/example.md".to_string())
                .unwrap(),
        )
        .unwrap();

        assert_eq!(json["source"]["kind"], "markdown");
        assert_eq!(json["valid"], true);
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
    fn reusable_engine_returns_host_text_measurer_errors() {
        let measurer = FailingTextMeasurer::new();
        let reusable = MermanReusableEngine::with_text_measurer(None, measurer.clone()).unwrap();

        let err = reusable
            .render_svg("flowchart TD\nA[Measured label] --> B[Done]".to_string())
            .unwrap_err();

        let message = match err {
            MermanError::Binding { message, .. } => message,
        };
        assert!(message.contains("test host measurer failed"));
        assert!(measurer.calls() > 0);
    }

    #[cfg(feature = "render")]
    #[test]
    fn reusable_engine_returns_host_text_measurer_errors_from_layout_json() {
        let measurer = FailingTextMeasurer::new();
        let reusable = MermanReusableEngine::with_text_measurer(None, measurer.clone()).unwrap();

        let err = reusable
            .layout_json("flowchart TD\nA[Measured label] --> B[Done]".to_string())
            .unwrap_err();

        let message = match err {
            MermanError::Binding { message, .. } => message,
        };
        assert!(message.contains("test host measurer failed"));
        assert!(measurer.calls() > 0);
    }

    #[cfg(feature = "render")]
    #[test]
    fn reusable_engine_falls_back_when_host_text_measurer_returns_none() {
        let measurer = MissingTextMeasurer::new();
        let reusable = MermanReusableEngine::with_text_measurer(None, measurer.clone()).unwrap();

        let svg = reusable
            .render_svg("flowchart TD\nA[Measured label] --> B[Done]".to_string())
            .unwrap();

        assert!(svg.contains("<svg"));
        assert!(measurer.calls() > 0);
    }

    #[cfg(feature = "render")]
    #[test]
    fn reusable_engine_keeps_host_text_measurer_errors_scoped_to_inflight_render() {
        let failing_measurer = BlockingFailingTextMeasurer::new();
        let reusable =
            MermanReusableEngine::with_text_measurer(None, failing_measurer.clone()).unwrap();
        let render_engine = reusable.clone();
        let render_handle = thread::spawn(move || {
            render_engine.render_svg("flowchart TD\nA[Measured label] --> B[Done]".to_string())
        });

        failing_measurer.wait_until_entered();

        let replacement = MissingTextMeasurer::new();
        let set_engine = reusable.clone();
        let (set_done_tx, set_done_rx) = mpsc::channel();
        let set_handle = thread::spawn(move || {
            set_engine.set_text_measurer(replacement).unwrap();
            set_done_tx.send(()).unwrap();
        });

        assert!(matches!(
            set_done_rx.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));

        failing_measurer.release();

        let err = render_handle.join().unwrap().unwrap_err();
        let message = match err {
            MermanError::Binding { message, .. } => message,
        };
        assert!(message.contains("blocked host measurer failed"));

        set_handle.join().unwrap();
        set_done_rx.recv_timeout(Duration::from_secs(1)).unwrap();

        let svg = reusable
            .render_svg("flowchart TD\nA[Measured label] --> B[Done]".to_string())
            .unwrap();
        assert!(svg.contains("<svg"));
    }

    #[cfg(feature = "render")]
    #[test]
    fn reusable_engine_rejects_reentrant_host_text_measurer_calls() {
        let measurer = ReentrantTextMeasurer::new();
        let reusable = MermanReusableEngine::with_text_measurer(None, measurer.clone()).unwrap();
        measurer.set_engine(reusable.clone());

        let err = reusable
            .render_svg("flowchart TD\nA[Measured label] --> B[Done]".to_string())
            .unwrap_err();

        let message = match err {
            MermanError::Binding { message, .. } => message,
        };
        assert!(message.contains("reentrant reusable engine render call"));
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

//! Operation-scoped math preparation for browser and native SVG projections.
//!
//! Family layout owns occurrence identity and terminal style resolution. This module performs one
//! backend preparation for that resolved request. The retained XHTML payload always feeds the
//! browser/Parity writer; backends that can produce native geometry also embed that projection for
//! the ResvgSafe pipeline.

#[cfg(feature = "math")]
use std::borrow::Cow;
use std::fmt;
#[cfg(feature = "math")]
use std::fmt::Write as _;
use std::sync::Arc;

use merman_core::sanitize::{SanitizeFailure, SanitizeOutputSink};
use merman_core::{MermaidConfig, OperationPhase};

use super::MathRenderer;
use crate::resources::{
    OperationWorkError, OperationWorkMeter, RenderResourcePolicy, RenderResourceProfile,
};
use crate::text::{TextMeasurer, TextMetrics, TextStyle, WrapMode};

pub(crate) const PREPARED_MATH_CLASS_ATTRIBUTE: &str = r#"class="merman-prepared-math""#;
pub(crate) const PREPARED_MATH_NATIVE_AVAILABLE_ATTRIBUTE: &str =
    r#"data-merman-prepared-math-native="v1""#;
pub(crate) const BROWSER_ONLY_MATH_NATIVE_UNAVAILABLE_ATTRIBUTE: &str =
    r#"data-merman-math-native="unavailable""#;
pub(crate) const PREPARED_MATH_OCCURRENCE_ATTRIBUTE: &str = "data-merman-prepared-math-occurrence";
pub(crate) const PREPARED_MATH_TERMINAL_SWITCH_ATTRIBUTE: &str =
    r#"data-merman-prepared-math-switch="v1""#;
pub(crate) const PREPARED_MATH_PROJECTION_TEMPLATE_OPEN: &str =
    r#"<template data-merman-prepared-math-projection="v1">"#;

#[derive(Debug)]
struct RawPreparedMathLabel {
    html: String,
    metrics: TextMetrics,
    max_line_height_px: f64,
    max_math_height_px: Option<f64>,
    native_svg: Option<String>,
}

impl RawPreparedMathLabel {
    fn new(html: impl Into<String>, metrics: TextMetrics) -> Self {
        let max_line_height_px = metrics.height / metrics.line_count.max(1) as f64;
        Self::with_max_line_height(html, metrics, max_line_height_px, None)
    }

    fn with_max_line_height(
        html: impl Into<String>,
        metrics: TextMetrics,
        max_line_height_px: f64,
        max_math_height_px: Option<f64>,
    ) -> Self {
        Self {
            html: html.into(),
            metrics,
            max_line_height_px,
            max_math_height_px,
            native_svg: None,
        }
    }

    fn with_native_svg(
        html: impl Into<String>,
        metrics: TextMetrics,
        max_line_height_px: f64,
        max_math_height_px: f64,
        native_svg: impl Into<String>,
    ) -> Self {
        Self {
            html: html.into(),
            metrics,
            max_line_height_px,
            max_math_height_px: Some(max_math_height_px),
            native_svg: Some(native_svg.into()),
        }
    }

    fn into_parts(self) -> (String, TextMetrics, f64, Option<f64>, Option<String>) {
        (
            self.html,
            self.metrics,
            self.max_line_height_px,
            self.max_math_height_px,
            self.native_svg,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MathLabelShell {
    Flowchart,
    Sequence,
}

#[derive(Clone, Copy)]
pub(crate) struct PrepareMathLabelRequest<'a> {
    text: &'a str,
    config: &'a MermaidConfig,
    style: &'a TextStyle,
    foreground: &'a str,
    text_measurer: Option<&'a dyn TextMeasurer>,
    max_width_px: Option<f64>,
    wrap_mode: WrapMode,
    shell: MathLabelShell,
    occurrence_id: Option<&'a super::PreparedMathOccurrenceId>,
    owner_retained_bytes: usize,
}

impl<'a> PrepareMathLabelRequest<'a> {
    pub(crate) const fn flowchart(
        text: &'a str,
        config: &'a MermaidConfig,
        style: &'a TextStyle,
        foreground: &'a str,
        max_width_px: Option<f64>,
        wrap_mode: WrapMode,
    ) -> Self {
        Self {
            text,
            config,
            style,
            foreground,
            text_measurer: None,
            max_width_px,
            wrap_mode,
            shell: MathLabelShell::Flowchart,
            occurrence_id: None,
            owner_retained_bytes: 0,
        }
    }

    pub(crate) const fn with_text_measurer(mut self, text_measurer: &'a dyn TextMeasurer) -> Self {
        self.text_measurer = Some(text_measurer);
        self
    }

    pub(crate) const fn with_owner_retained_bytes(mut self, retained_bytes: usize) -> Self {
        self.owner_retained_bytes = retained_bytes;
        self
    }

    pub(crate) const fn with_occurrence_id(
        mut self,
        occurrence_id: &'a super::PreparedMathOccurrenceId,
    ) -> Self {
        self.occurrence_id = Some(occurrence_id);
        self
    }

    pub(crate) const fn sequence(
        text: &'a str,
        config: &'a MermaidConfig,
        style: &'a TextStyle,
        foreground: &'a str,
    ) -> Self {
        Self {
            text,
            config,
            style,
            foreground,
            text_measurer: None,
            max_width_px: None,
            wrap_mode: WrapMode::HtmlLike,
            shell: MathLabelShell::Sequence,
            occurrence_id: None,
            owner_retained_bytes: 0,
        }
    }
}

#[derive(Debug)]
pub(crate) struct PreparedMathLabel {
    metrics: TextMetrics,
    max_line_height_px: f64,
    max_math_height_px: Option<f64>,
    browser_xhtml: Arc<str>,
    style_assurance: super::PreparedMathStyleAssurance,
    occurrence_id: Option<super::PreparedMathOccurrenceId>,
    projection_fingerprint: Option<super::PreparedMathProjectionFingerprint>,
}

impl PreparedMathLabel {
    #[cfg(test)]
    pub(crate) fn for_test(metrics: TextMetrics) -> Self {
        Self {
            metrics,
            max_line_height_px: metrics.height,
            max_math_height_px: None,
            browser_xhtml: Arc::from("<span/>"),
            style_assurance: super::PreparedMathStyleAssurance::opaque_html("", 16.0),
            occurrence_id: None,
            projection_fingerprint: None,
        }
    }

    pub(crate) const fn metrics(&self) -> TextMetrics {
        self.metrics
    }

    pub(crate) const fn max_line_height_px(&self) -> f64 {
        self.max_line_height_px
    }

    pub(crate) const fn max_math_height_px(&self) -> Option<f64> {
        self.max_math_height_px
    }

    pub(crate) fn browser_xhtml(&self) -> &str {
        &self.browser_xhtml
    }

    pub(crate) const fn style_assurance(&self) -> &super::PreparedMathStyleAssurance {
        &self.style_assurance
    }

    pub(crate) fn expectation(
        &self,
        expected_emissions: usize,
    ) -> Option<super::PreparedMathExpectation> {
        let occurrence_id = self.occurrence_id.clone()?;
        Some(match self.projection_fingerprint {
            Some(fingerprint) => super::PreparedMathExpectation::available(
                occurrence_id,
                fingerprint,
                expected_emissions,
            ),
            None => super::PreparedMathExpectation::unavailable(occurrence_id, expected_emissions),
        })
    }

    /// Exact bytes owned by this artifact under the prepared-math accounting contract.
    pub(crate) fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .checked_add(self.browser_xhtml.len())
            .and_then(|bytes| bytes.checked_add(self.style_assurance.retained_heap_bytes()))
            .and_then(|bytes| {
                bytes.checked_add(
                    self.occurrence_id
                        .as_ref()
                        .map_or(0, |occurrence_id| occurrence_id.as_str().len()),
                )
            })
            .unwrap_or(usize::MAX)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MathPreparationUnavailable {
    BackendUnavailable,
    BackendDeclined,
    MixedContent,
    NotMath,
    UnsupportedPaint,
}

#[derive(Debug, Clone)]
pub(crate) enum MathPreparationOutcome {
    Prepared(Arc<PreparedMathLabel>),
    Unavailable(MathPreparationUnavailable),
}

impl MathPreparationOutcome {
    pub(crate) fn prepared(&self) -> Option<&PreparedMathLabel> {
        match self {
            Self::Prepared(prepared) => Some(prepared),
            Self::Unavailable(_) => None,
        }
    }

    pub(crate) const fn unavailable_reason(&self) -> Option<MathPreparationUnavailable> {
        match self {
            Self::Prepared(_) => None,
            Self::Unavailable(reason) => Some(*reason),
        }
    }
}

#[derive(Clone)]
pub(crate) enum ConfiguredMathBackend {
    #[cfg(feature = "math")]
    CompiledRatex,
    External(Arc<dyn MathRenderer + Send + Sync>),
}

impl ConfiguredMathBackend {
    #[cfg(feature = "math")]
    pub(crate) const fn compiled_ratex() -> Self {
        Self::CompiledRatex
    }

    pub(crate) fn external(renderer: Arc<dyn MathRenderer + Send + Sync>) -> Self {
        Self::External(renderer)
    }

    /// Reports whether this monolithic backend is admissible under the operation resource profile.
    ///
    /// Every currently supported backend returns a complete allocated `String`. That contract is
    /// compatible with the cooperative and trusted profiles, but it cannot enforce Merman's
    /// in-process memory ceiling for hostile input. The constrained profile therefore rejects math
    /// during capability planning, before a backend or child process is invoked. A future bounded
    /// sink backend can make this decision backend-specific without weakening that boundary.
    pub(crate) const fn supports_resource_policy(&self, policy: RenderResourcePolicy) -> bool {
        !matches!(policy.profile(), RenderResourceProfile::Constrained)
    }

    /// Compatibility view used only by legacy, non-prepared projection paths.
    pub(crate) fn renderer(&self) -> &(dyn MathRenderer + Send + Sync) {
        match self {
            #[cfg(feature = "math")]
            Self::CompiledRatex => &super::RatexMathRenderer,
            Self::External(renderer) => renderer.as_ref(),
        }
    }

    pub(crate) fn prepare(
        &self,
        request: PrepareMathLabelRequest<'_>,
        work_meter: &Arc<OperationWorkMeter>,
    ) -> Result<MathPreparationOutcome, OperationWorkError> {
        work_meter.preflight_prepared_text_retained_bytes(request.owner_retained_bytes)?;
        match self {
            #[cfg(feature = "math")]
            Self::CompiledRatex => {
                let classification = classify_math_label(request.text, Some(work_meter.as_ref()))?;
                if matches!(classification, MathLabelClassification::NotMath) {
                    return Ok(MathPreparationOutcome::Unavailable(
                        MathPreparationUnavailable::NotMath,
                    ));
                }
                if !crate::mermaid_style::is_supported_css_color_value(request.foreground) {
                    return Ok(MathPreparationOutcome::Unavailable(
                        MathPreparationUnavailable::UnsupportedPaint,
                    ));
                }
                prepare_compiled_ratex(request, classification, work_meter)
            }
            Self::External(renderer) => {
                if !charge_external_math_source(request.text, work_meter.as_ref())? {
                    return Ok(MathPreparationOutcome::Unavailable(
                        MathPreparationUnavailable::NotMath,
                    ));
                }
                if !crate::mermaid_style::is_safe_browser_css_color_value(request.foreground) {
                    return Ok(MathPreparationOutcome::Unavailable(
                        MathPreparationUnavailable::UnsupportedPaint,
                    ));
                }
                prepare_external(renderer.as_ref(), request, work_meter)
            }
        }
    }
}

impl fmt::Debug for ConfiguredMathBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "math")]
            Self::CompiledRatex => formatter.write_str("ConfiguredMathBackend::CompiledRatex"),
            Self::External(renderer) => formatter
                .debug_tuple("ConfiguredMathBackend::External")
                .field(renderer)
                .finish(),
        }
    }
}

#[cfg(feature = "math")]
enum MathLabelClassification {
    NotMath,
    Pure(Vec<ClassifiedPureMathLine>),
    Mixed(Vec<ClassifiedMathLine>),
}

#[cfg(feature = "math")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ClassifiedPureMathLine {
    Formula(String),
    Empty,
}

#[cfg(feature = "math")]
struct ClassifiedMathLine {
    segments: Vec<ClassifiedMathSegment>,
}

#[cfg(feature = "math")]
enum ClassifiedMathSegment {
    Text(String),
    Formula(String),
}

#[cfg(feature = "math")]
fn classify_math_label(
    text: &str,
    work_meter: Option<&OperationWorkMeter>,
) -> Result<MathLabelClassification, OperationWorkError> {
    if let Some(work_meter) = work_meter {
        work_meter.checkpoint(OperationPhase::Layout)?;
        work_meter.charge(1)?;
    }
    let normalized = if text.contains("\\\\") {
        Cow::Owned(text.replace("\\\\", "\\"))
    } else {
        Cow::Borrowed(text)
    };
    let mut mixed_lines = Vec::new();
    let mut saw_math = false;
    let mut all_nonempty_lines_are_formulas = true;

    for raw_line in crate::text::split_html_br_lines(&normalized) {
        if let Some(work_meter) = work_meter {
            work_meter.checkpoint(OperationPhase::Layout)?;
            work_meter.charge(1usize.saturating_add(raw_line.len() / 64))?;
        }
        let trimmed = raw_line.trim();
        let parsed = super::parse_delimited_math_line(raw_line);
        let is_pure_formula = parsed.as_ref().is_some_and(|parsed| {
            parsed.fragments.len() == 1
                && parsed.fragments[0].leading_text.trim().is_empty()
                && parsed.trailing_text.trim().is_empty()
        });
        if let Some(parsed) = parsed {
            saw_math = true;
            let mut segments =
                Vec::with_capacity(parsed.fragments.len().saturating_mul(2).saturating_add(1));
            for fragment in parsed.fragments {
                if !fragment.leading_text.is_empty() {
                    segments.push(ClassifiedMathSegment::Text(
                        fragment.leading_text.to_owned(),
                    ));
                }
                segments.push(ClassifiedMathSegment::Formula(fragment.formula.to_owned()));
            }
            if !parsed.trailing_text.is_empty() {
                segments.push(ClassifiedMathSegment::Text(parsed.trailing_text.to_owned()));
            }
            mixed_lines.push(ClassifiedMathLine { segments });
        } else {
            mixed_lines.push(ClassifiedMathLine {
                segments: (!raw_line.is_empty())
                    .then(|| ClassifiedMathSegment::Text(raw_line.to_owned()))
                    .into_iter()
                    .collect(),
            });
        }
        if trimmed.is_empty() {
            continue;
        }
        if !is_pure_formula {
            all_nonempty_lines_are_formulas = false;
        }
    }

    Ok(if !saw_math {
        MathLabelClassification::NotMath
    } else if all_nonempty_lines_are_formulas {
        let lines = mixed_lines
            .into_iter()
            .map(|line| {
                line.segments
                    .into_iter()
                    .find_map(|segment| match segment {
                        ClassifiedMathSegment::Formula(formula) => Some(formula),
                        ClassifiedMathSegment::Text(_) => None,
                    })
                    .map_or(
                        ClassifiedPureMathLine::Empty,
                        ClassifiedPureMathLine::Formula,
                    )
            })
            .collect();
        MathLabelClassification::Pure(lines)
    } else {
        MathLabelClassification::Mixed(mixed_lines)
    })
}

fn charge_external_math_source(
    text: &str,
    work_meter: &OperationWorkMeter,
) -> Result<bool, OperationWorkError> {
    work_meter.checkpoint(OperationPhase::Layout)?;
    work_meter.charge(1)?;
    let mut saw_math = false;
    for line in crate::text::split_html_br_lines(text) {
        work_meter.checkpoint(OperationPhase::Layout)?;
        work_meter.charge(1usize.saturating_add(line.len() / 64))?;
        saw_math |= super::parse_delimited_math_line(line).is_some();
    }
    Ok(saw_math)
}

fn prepare_external(
    renderer: &(dyn MathRenderer + Send + Sync),
    request: PrepareMathLabelRequest<'_>,
    work_meter: &Arc<OperationWorkMeter>,
) -> Result<MathPreparationOutcome, OperationWorkError> {
    // The compatibility backend returns an allocated String, so its first output allocation is
    // necessarily outside host admission. Family sidecars reserve the exact retained bytes before
    // keeping the result; final bounded SVG emission remains authoritative for serialized bytes.
    let metrics = match request.shell {
        MathLabelShell::Flowchart => renderer
            .measure_html_label(
                request.text,
                request.config,
                request.style,
                request.max_width_px,
                request.wrap_mode,
            )
            .or_else(|| {
                request.text_measurer.map(|measurer| {
                    measurer.measure_wrapped(
                        request.text,
                        request.style,
                        request.max_width_px,
                        request.wrap_mode,
                    )
                })
            }),
        MathLabelShell::Sequence => renderer
            .measure_sequence_html_label_with_style(request.text, request.config, request.style)
            .or_else(|| {
                request.text_measurer.map(|measurer| {
                    measurer.measure_wrapped(
                        request.text,
                        request.style,
                        None,
                        WrapMode::SvgLikeSingleRun,
                    )
                })
            }),
    };
    let html = match request.shell {
        MathLabelShell::Flowchart => renderer.render_html_label(request.text, request.config),
        MathLabelShell::Sequence => {
            renderer.render_sequence_html_label(request.text, request.config)
        }
    };
    let prepared = metrics
        .zip(html)
        .map(|(metrics, html)| RawPreparedMathLabel::new(html, metrics));
    Ok(match prepared {
        Some(prepared) => {
            let style_assurance = super::PreparedMathStyleAssurance::opaque_html(
                request.foreground,
                request.style.font_size,
            );
            match finish_external_preparation(prepared, request, style_assurance, work_meter)? {
                Some(prepared) => MathPreparationOutcome::Prepared(Arc::new(prepared)),
                None => {
                    MathPreparationOutcome::Unavailable(MathPreparationUnavailable::BackendDeclined)
                }
            }
        }
        None => MathPreparationOutcome::Unavailable(MathPreparationUnavailable::BackendDeclined),
    })
}

fn finish_external_preparation(
    prepared: RawPreparedMathLabel,
    request: PrepareMathLabelRequest<'_>,
    style_assurance: super::PreparedMathStyleAssurance,
    work_meter: &Arc<OperationWorkMeter>,
) -> Result<Option<PreparedMathLabel>, OperationWorkError> {
    let (html, metrics, max_line_height_px, max_math_height_px, native_svg) = prepared.into_parts();
    if !valid_metrics(&metrics)
        || !max_line_height_px.is_finite()
        || max_line_height_px < 0.0
        || max_line_height_px > metrics.height
        || max_math_height_px
            .is_some_and(|height| !height.is_finite() || height < 0.0 || height > metrics.height)
    {
        return Ok(None);
    }
    let Some(browser_xhtml) = browser_payload(&html, native_svg.as_deref(), request, work_meter)?
    else {
        return Ok(None);
    };
    let projection_fingerprint = native_svg
        .as_deref()
        .map(super::PreparedMathProjectionFingerprint::from_projection);
    let artifact = PreparedMathLabel {
        metrics,
        max_line_height_px,
        max_math_height_px,
        browser_xhtml: Arc::from(browser_xhtml),
        style_assurance,
        occurrence_id: request.occurrence_id.cloned(),
        projection_fingerprint,
    };
    work_meter.preflight_prepared_text_retained_bytes(
        request
            .owner_retained_bytes
            .saturating_add(artifact.retained_bytes()),
    )?;
    Ok(Some(artifact))
}

fn valid_metrics(metrics: &TextMetrics) -> bool {
    metrics.width.is_finite()
        && metrics.height.is_finite()
        && metrics.width >= 0.0
        && metrics.height >= 0.0
        && metrics.line_count > 0
}

#[derive(Debug, Clone, Copy)]
enum PreparedMathOutputError {
    Limit { attempted: usize },
    Allocation,
}

#[derive(Debug, Clone, Copy)]
struct PreparedMathSanitizeSink {
    max: usize,
}

impl PreparedMathSanitizeSink {
    const fn new(max: usize) -> Self {
        Self { max }
    }

    fn checked_len(
        &self,
        current: usize,
        additional: usize,
    ) -> Result<usize, PreparedMathOutputError> {
        let attempted = current
            .checked_add(additional)
            .ok_or(PreparedMathOutputError::Limit {
                attempted: usize::MAX,
            })?;
        if attempted > self.max {
            return Err(PreparedMathOutputError::Limit { attempted });
        }
        Ok(attempted)
    }
}

impl SanitizeOutputSink for PreparedMathSanitizeSink {
    type Error = PreparedMathOutputError;

    fn checked_output_len(&self, current: usize, additional: usize) -> Result<usize, Self::Error> {
        self.checked_len(current, additional)
    }

    fn string_with_capacity(&self, capacity: usize) -> Result<String, Self::Error> {
        self.checked_len(0, capacity)?;
        let mut output = String::new();
        output
            .try_reserve_exact(capacity)
            .map_err(|_| PreparedMathOutputError::Allocation)?;
        Ok(output)
    }

    fn output_buffer(&self, input_len: usize) -> Result<Vec<u8>, Self::Error> {
        self.checked_len(0, input_len)?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(input_len)
            .map_err(|_| PreparedMathOutputError::Allocation)?;
        Ok(output)
    }

    fn push_output_chunk(&self, output: &mut Vec<u8>, chunk: &[u8]) -> Result<(), Self::Error> {
        self.checked_len(output.len(), chunk.len())?;
        output
            .try_reserve(chunk.len())
            .map_err(|_| PreparedMathOutputError::Allocation)?;
        output.extend_from_slice(chunk);
        Ok(())
    }
}

fn push_bounded(
    output: &mut String,
    value: &str,
    sink: PreparedMathSanitizeSink,
) -> Result<(), PreparedMathOutputError> {
    sink.checked_len(output.len(), value.len())?;
    output
        .try_reserve(value.len())
        .map_err(|_| PreparedMathOutputError::Allocation)?;
    output.push_str(value);
    Ok(())
}

fn resolve_output_error(
    error: PreparedMathOutputError,
    retained_prefix: usize,
    work_meter: &OperationWorkMeter,
) -> Result<Option<String>, OperationWorkError> {
    match error {
        PreparedMathOutputError::Limit { attempted } => {
            let additional = retained_prefix.saturating_add(attempted);
            match work_meter.preflight_prepared_text_retained_bytes(additional) {
                Err(error) => Err(error),
                Ok(()) => Ok(None),
            }
        }
        PreparedMathOutputError::Allocation => Ok(None),
    }
}

fn browser_payload(
    rendered: &str,
    native_svg: Option<&str>,
    request: PrepareMathLabelRequest<'_>,
    work_meter: &OperationWorkMeter,
) -> Result<Option<String>, OperationWorkError> {
    const PREFIX_AVAILABLE: &str =
        r#"<span class="merman-prepared-math" data-merman-prepared-math-native="v1" style=""#;
    const PREFIX_UNAVAILABLE: &str = r#"<span class="merman-prepared-math" data-merman-prepared-math-native="unavailable" style=""#;
    const BODY: &str = "\">";
    const NATIVE_PREFIX: &str = r#"<template data-merman-prepared-math-projection="v1">"#;
    const NATIVE_SUFFIX: &str = "</template>";
    const SUFFIX: &str = "</span>";
    const OCCURRENCE_PREFIX: &str = r#"" data-merman-prepared-math-occurrence=""#;

    let prefix = if native_svg.is_some() {
        PREFIX_AVAILABLE
    } else {
        PREFIX_UNAVAILABLE
    };

    let retained_prefix = prepared_math_retained_prefix(request);
    preflight_raw_math_output(request, work_meter, rendered.len())?;
    let max_browser_bytes = work_meter
        .remaining_prepared_text_retained_bytes()
        .map_or(usize::MAX, |remaining| {
            remaining.saturating_sub(retained_prefix)
        });
    let sink = PreparedMathSanitizeSink::new(max_browser_bytes);

    let style = match resolved_wrapper_style(request.style, request.foreground, sink) {
        Ok(style) => style,
        Err(error) => return resolve_output_error(error, retained_prefix, work_meter),
    };
    let escaped_style = match escape_xml_attribute(&style, sink) {
        Ok(style) => style,
        Err(error) => return resolve_output_error(error, retained_prefix, work_meter),
    };
    let fixed_bytes = prefix
        .len()
        .checked_add(escaped_style.len())
        .and_then(|bytes| bytes.checked_add(BODY.len()))
        .and_then(|bytes| {
            request.occurrence_id.map_or(Some(bytes), |occurrence_id| {
                bytes
                    .checked_add(OCCURRENCE_PREFIX.len())
                    .and_then(|bytes| bytes.checked_add(occurrence_id.as_str().len()))
            })
        })
        .and_then(|bytes| {
            native_svg.map_or(Some(bytes), |native_svg| {
                bytes
                    .checked_add(NATIVE_PREFIX.len())
                    .and_then(|bytes| bytes.checked_add(native_svg.len()))
                    .and_then(|bytes| bytes.checked_add(NATIVE_SUFFIX.len()))
            })
        })
        .and_then(|bytes| bytes.checked_add(SUFFIX.len()))
        .unwrap_or(usize::MAX);
    if fixed_bytes > max_browser_bytes {
        return resolve_output_error(
            PreparedMathOutputError::Limit {
                attempted: fixed_bytes,
            },
            retained_prefix,
            work_meter,
        );
    }

    let body_sink = PreparedMathSanitizeSink::new(max_browser_bytes - fixed_bytes);
    let sanitized = match merman_core::sanitize::sanitize_text_with_sink(
        rendered,
        request.config,
        &body_sink,
    ) {
        Ok(sanitized) => sanitized,
        Err(SanitizeFailure::Output(error)) => {
            let error = match error {
                PreparedMathOutputError::Limit { attempted } => PreparedMathOutputError::Limit {
                    attempted: fixed_bytes.saturating_add(attempted),
                },
                PreparedMathOutputError::Allocation => PreparedMathOutputError::Allocation,
            };
            return resolve_output_error(error, retained_prefix, work_meter);
        }
        Err(SanitizeFailure::RejectedInput | SanitizeFailure::InvalidUtf8Output) | Err(_) => {
            return Ok(None);
        }
    };
    let rendered = match crate::xml::normalize_html_fragment_for_xhtml_bounded(
        &sanitized,
        max_browser_bytes - fixed_bytes,
    ) {
        Ok(rendered) => rendered,
        Err(crate::xml::XmlOutputError::Limit { attempted, .. }) => {
            return resolve_output_error(
                PreparedMathOutputError::Limit {
                    attempted: fixed_bytes.saturating_add(attempted),
                },
                retained_prefix,
                work_meter,
            );
        }
        Err(crate::xml::XmlOutputError::Allocation) => return Ok(None),
    };

    let final_len = fixed_bytes.saturating_add(rendered.len());
    let mut payload = match sink.string_with_capacity(final_len) {
        Ok(payload) => payload,
        Err(error) => return resolve_output_error(error, retained_prefix, work_meter),
    };
    payload.push_str(prefix);
    payload.push_str(&escaped_style);
    if let Some(occurrence_id) = request.occurrence_id {
        payload.push_str(OCCURRENCE_PREFIX);
        payload.push_str(occurrence_id.as_str());
    }
    payload.push_str(BODY);
    payload.push_str(&rendered);
    if let Some(native_svg) = native_svg {
        payload.push_str(NATIVE_PREFIX);
        payload.push_str(native_svg);
        payload.push_str(NATIVE_SUFFIX);
    }
    payload.push_str(SUFFIX);
    debug_assert_eq!(payload.len(), final_len);
    Ok(Some(payload))
}

fn prepared_math_retained_prefix(request: PrepareMathLabelRequest<'_>) -> usize {
    request
        .owner_retained_bytes
        .checked_add(std::mem::size_of::<PreparedMathLabel>())
        .and_then(|bytes| {
            bytes.checked_add(
                request
                    .occurrence_id
                    .map_or(0, |occurrence_id| occurrence_id.as_str().len()),
            )
        })
        .unwrap_or(usize::MAX)
}

fn preflight_raw_math_output(
    request: PrepareMathLabelRequest<'_>,
    work_meter: &OperationWorkMeter,
    raw_output_bytes: usize,
) -> Result<(), OperationWorkError> {
    work_meter.preflight_prepared_text_retained_bytes(
        prepared_math_retained_prefix(request).saturating_add(raw_output_bytes),
    )
}

fn resolved_wrapper_style(
    style: &TextStyle,
    foreground: &str,
    sink: PreparedMathSanitizeSink,
) -> Result<String, PreparedMathOutputError> {
    let mut declarations = sink.string_with_capacity(0)?;
    push_bounded(&mut declarations, "font-size:", sink)?;
    push_bounded(
        &mut declarations,
        &format_number(effective_font_size(style.font_size)),
        sink,
    )?;
    push_bounded(&mut declarations, "px;color:", sink)?;
    push_bounded(&mut declarations, foreground.trim(), sink)?;
    push_bounded(&mut declarations, ";", sink)?;
    if let Some(font_family) = style
        .font_family
        .as_deref()
        .filter(|value| crate::mermaid_style::is_safe_css_font_family_value(value))
    {
        push_bounded(&mut declarations, "font-family:", sink)?;
        push_bounded(&mut declarations, font_family, sink)?;
        push_bounded(&mut declarations, ";", sink)?;
    }
    if let Some(font_weight) = style
        .font_weight
        .as_deref()
        .filter(|value| crate::mermaid_style::is_supported_css_font_weight_value(value))
    {
        push_bounded(&mut declarations, "font-weight:", sink)?;
        push_bounded(&mut declarations, font_weight.trim(), sink)?;
        push_bounded(&mut declarations, ";", sink)?;
    }
    if let Some(font_style) = style
        .font_style
        .as_deref()
        .filter(|value| crate::mermaid_style::is_supported_css_font_style_value(value))
    {
        push_bounded(&mut declarations, "font-style:", sink)?;
        push_bounded(&mut declarations, font_style.trim(), sink)?;
        push_bounded(&mut declarations, ";", sink)?;
    }
    Ok(declarations)
}

fn escape_xml_attribute(
    value: &str,
    sink: PreparedMathSanitizeSink,
) -> Result<String, PreparedMathOutputError> {
    let mut escaped = sink.string_with_capacity(0)?;
    for character in value.chars() {
        let mut encoded = [0; 4];
        let value = match character {
            '&' => "&amp;",
            '"' => "&quot;",
            '<' => "&lt;",
            '>' => "&gt;",
            _ => character.encode_utf8(&mut encoded),
        };
        push_bounded(&mut escaped, value, sink)?;
    }
    Ok(escaped)
}

fn effective_font_size(requested: f64) -> f64 {
    if requested.is_finite() {
        requested.max(1.0)
    } else {
        1.0
    }
}

#[cfg(feature = "math")]
#[derive(Debug, Clone)]
pub(super) struct CompiledRatexFormula {
    pub(super) svg: String,
    pub(super) width_em: f64,
    pub(super) height_em: f64,
}

#[cfg(feature = "math")]
struct CompiledRatexLayout {
    display_list: ratex_types::DisplayList,
    width_em: f64,
    height_em: f64,
}

#[cfg(feature = "math")]
fn layout_compiled_ratex_formula(latex: &str, foreground: &str) -> Option<CompiledRatexLayout> {
    let color = ratex_types::Color::parse(foreground)?;
    let ast = ratex_parser::parse(latex).ok()?;
    let layout_options = ratex_layout::LayoutOptions::default()
        .with_style(ratex_types::MathStyle::Display)
        .with_color(color);
    let layout_box = ratex_layout::layout(&ast, &layout_options);
    let display_list = ratex_layout::to_display_list(&layout_box);
    let width_em = emitted_em_dimension(display_list.width.max(0.0));
    let height_em = emitted_em_dimension(display_list.total_height().max(0.0));
    Some(CompiledRatexLayout {
        display_list,
        width_em,
        height_em,
    })
}

#[cfg(feature = "math")]
pub(super) fn measure_compiled_ratex_formula(latex: &str, foreground: &str) -> Option<(f64, f64)> {
    let layout = layout_compiled_ratex_formula(latex, foreground)?;
    Some((layout.width_em, layout.height_em))
}

#[cfg(feature = "math")]
pub(super) fn render_compiled_ratex_formula(
    latex: &str,
    foreground: &str,
) -> Option<CompiledRatexFormula> {
    let layout = layout_compiled_ratex_formula(latex, foreground)?;
    let svg = ratex_svg::render_to_svg(
        &layout.display_list,
        &ratex_svg::SvgOptions {
            font_size: 1.0,
            padding: 0.0,
            stroke_width: 0.04,
            embed_glyphs: true,
            font_dir: String::new(),
        },
    );
    Some(CompiledRatexFormula {
        svg: svg_with_em_size(svg, layout.width_em, layout.height_em),
        width_em: layout.width_em,
        height_em: layout.height_em,
    })
}

#[cfg(feature = "math")]
fn prepare_compiled_ratex(
    request: PrepareMathLabelRequest<'_>,
    classification: MathLabelClassification,
    work_meter: &Arc<OperationWorkMeter>,
) -> Result<MathPreparationOutcome, OperationWorkError> {
    let prepared = match classification {
        MathLabelClassification::Pure(formulas) => {
            prepare_compiled_ratex_pure_html_label(formulas, request, work_meter.as_ref())?
        }
        MathLabelClassification::Mixed(lines) => {
            let Some(text_measurer) = request.text_measurer else {
                return Ok(MathPreparationOutcome::Unavailable(
                    MathPreparationUnavailable::MixedContent,
                ));
            };
            prepare_compiled_ratex_mixed_html_label(
                lines,
                request,
                text_measurer,
                work_meter.as_ref(),
            )?
        }
        MathLabelClassification::NotMath => unreachable!("handled before compiled preparation"),
    };
    let Some(prepared) = prepared else {
        return Ok(MathPreparationOutcome::Unavailable(
            MathPreparationUnavailable::BackendDeclined,
        ));
    };
    let style_assurance = super::PreparedMathStyleAssurance::compiled_ratex(
        request.foreground,
        request.style.font_size,
    );
    let Some(prepared) =
        finish_external_preparation(prepared, request, style_assurance, work_meter)?
    else {
        return Ok(MathPreparationOutcome::Unavailable(
            MathPreparationUnavailable::BackendDeclined,
        ));
    };
    Ok(MathPreparationOutcome::Prepared(Arc::new(prepared)))
}

#[cfg(feature = "math")]
fn prepare_compiled_ratex_pure_html_label(
    lines: Vec<ClassifiedPureMathLine>,
    request: PrepareMathLabelRequest<'_>,
    work_meter: &OperationWorkMeter,
) -> Result<Option<RawPreparedMathLabel>, OperationWorkError> {
    let Some(color) = ratex_types::Color::parse(request.foreground) else {
        return Ok(None);
    };
    let applied_foreground = color.to_string();
    let font_size_px = effective_font_size(request.style.font_size);
    let mut browser_html = String::new();
    let mut width_px = 0.0_f64;
    let mut height_px = 0.0_f64;
    let mut max_line_height_px = 0.0_f64;
    let mut max_math_height_px = 0.0_f64;
    let line_count = lines.len();
    let mut native_lines = Vec::with_capacity(line_count);

    for line in lines {
        match line {
            ClassifiedPureMathLine::Formula(formula) => {
                charge_formula_work(Some(work_meter), &formula)?;
                let Some(rendered) = render_compiled_ratex_formula(&formula, &applied_foreground)
                else {
                    return Ok(None);
                };
                let line_width_px = rendered.width_em * font_size_px;
                let line_height_px = rendered.height_em * font_size_px;
                let line_y_px = height_px;
                width_px = width_px.max(line_width_px);
                height_px += line_height_px;
                max_line_height_px = max_line_height_px.max(line_height_px);
                max_math_height_px = max_math_height_px.max(line_height_px);
                let Some(native_line) =
                    positioned_ratex_svg(&rendered, 0.0, line_y_px, line_width_px, line_height_px)
                else {
                    return Ok(None);
                };
                let appended_bytes = r#"<div style="display:flex;align-items:center;justify-content:center;white-space:nowrap;"></div>"#
                    .len()
                    .saturating_add(rendered.svg.len());
                preflight_raw_math_output(
                    request,
                    work_meter,
                    browser_html.len().saturating_add(appended_bytes),
                )?;
                let _ = write!(
                    &mut browser_html,
                    r#"<div style="display:flex;align-items:center;justify-content:center;white-space:nowrap;">{}</div>"#,
                    rendered.svg
                );
                native_lines.push((line_width_px, native_line));
            }
            ClassifiedPureMathLine::Empty => {
                let line_height_px = match request.shell {
                    MathLabelShell::Flowchart => {
                        crate::text::flowchart_html_line_height_px(font_size_px)
                    }
                    MathLabelShell::Sequence => {
                        crate::sequence::sequence_text_line_step_px(font_size_px)
                    }
                };
                height_px += line_height_px;
                max_line_height_px = max_line_height_px.max(line_height_px);
                let line_height = format_number(line_height_px);
                let appended_bytes = r#"<div style="display:flex;align-items:center;justify-content:center;white-space:nowrap;height:px;"></div>"#
                    .len()
                    .saturating_add(line_height.len());
                preflight_raw_math_output(
                    request,
                    work_meter,
                    browser_html.len().saturating_add(appended_bytes),
                )?;
                let _ = write!(
                    &mut browser_html,
                    r#"<div style="display:flex;align-items:center;justify-content:center;white-space:nowrap;height:{line_height}px;"></div>"#,
                );
            }
        }
    }

    let metrics = TextMetrics {
        width: width_px,
        height: height_px,
        line_count: line_count.max(1),
    };
    let native_svg = native_math_projection(width_px, height_px, native_lines);
    preflight_raw_math_output(
        request,
        work_meter,
        browser_html.len().saturating_add(native_svg.len()),
    )?;
    Ok(Some(RawPreparedMathLabel::with_native_svg(
        browser_html,
        metrics,
        max_line_height_px,
        max_math_height_px,
        native_svg,
    )))
}

#[cfg(feature = "math")]
fn positioned_ratex_svg(
    rendered: &CompiledRatexFormula,
    x_px: f64,
    y_px: f64,
    width_px: f64,
    height_px: f64,
) -> Option<String> {
    let open_end = rendered.svg.find('>')?;
    let body = rendered.svg.get(open_end + 1..)?.strip_suffix("</svg>")?;
    Some(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" x="{}" y="{}" width="{}" height="{}" viewBox="0 0 {} {}">{body}</svg>"#,
        format_number(x_px),
        format_number(y_px),
        format_number(width_px),
        format_number(height_px),
        format_number(rendered.width_em),
        format_number(rendered.height_em),
    ))
}

#[cfg(feature = "math")]
fn native_math_projection(
    width_px: f64,
    height_px: f64,
    native_lines: Vec<(f64, String)>,
) -> String {
    let mut projection = format!(
        r#"<g data-merman-prepared-math-width="{}" data-merman-prepared-math-height="{}">"#,
        format_number(width_px),
        format_number(height_px),
    );
    for (line_width_px, line_svg) in native_lines {
        let x = ((width_px - line_width_px) / 2.0).max(0.0);
        let _ = write!(
            projection,
            r#"<g transform="translate({} 0)">{line_svg}</g>"#,
            format_number(x),
        );
    }
    projection.push_str("</g>");
    projection
}

#[cfg(feature = "math")]
fn measure_compiled_mixed_prose(
    request: PrepareMathLabelRequest<'_>,
    text_measurer: &dyn TextMeasurer,
    text: &str,
    flowchart_max_width_px: Option<f64>,
) -> TextMetrics {
    let (max_width_px, wrap_mode) = match request.shell {
        MathLabelShell::Flowchart => (flowchart_max_width_px, WrapMode::HtmlLike),
        MathLabelShell::Sequence => (None, WrapMode::SvgLikeSingleRun),
    };
    text_measurer.measure_wrapped(text, request.style, max_width_px, wrap_mode)
}

#[cfg(feature = "math")]
fn prepare_compiled_ratex_mixed_html_label(
    lines: Vec<ClassifiedMathLine>,
    request: PrepareMathLabelRequest<'_>,
    text_measurer: &dyn TextMeasurer,
    work_meter: &OperationWorkMeter,
) -> Result<Option<RawPreparedMathLabel>, OperationWorkError> {
    let Some(color) = ratex_types::Color::parse(request.foreground) else {
        return Ok(None);
    };
    let applied_foreground = color.to_string();
    let font_size_px = effective_font_size(request.style.font_size);
    let mut browser_html = String::new();
    let mut width_px = 0.0_f64;
    let mut height_px = 0.0_f64;
    let mut max_line_height_px = 0.0_f64;
    let mut max_math_height_px = 0.0_f64;
    let mut line_count = 0usize;

    for line in lines {
        let has_formula = line
            .segments
            .iter()
            .any(|segment| matches!(segment, ClassifiedMathSegment::Formula(_)));
        if !has_formula {
            let text = line
                .segments
                .into_iter()
                .filter_map(|segment| match segment {
                    ClassifiedMathSegment::Text(text) => Some(text),
                    ClassifiedMathSegment::Formula(_) => None,
                })
                .collect::<String>();
            let metrics =
                measure_compiled_mixed_prose(request, text_measurer, &text, request.max_width_px);
            if !valid_metrics(&metrics) {
                return Ok(None);
            }
            width_px = width_px.max(metrics.width);
            height_px += metrics.height;
            max_line_height_px = max_line_height_px.max(metrics.height);
            line_count = line_count.saturating_add(metrics.line_count.max(1));
            preflight_raw_math_output(
                request,
                work_meter,
                browser_html
                    .len()
                    .saturating_add("<div></div>".len())
                    .saturating_add(text.len()),
            )?;
            let _ = write!(&mut browser_html, "<div>{text}</div>");
            continue;
        }

        let mut line_width_px = 0.0_f64;
        let mut line_height_px = 0.0_f64;
        const MIXED_LINE_OPEN: &str = r#"<div style="display:flex;align-items:center;justify-content:center;white-space:nowrap;">"#;
        preflight_raw_math_output(
            request,
            work_meter,
            browser_html.len().saturating_add(MIXED_LINE_OPEN.len()),
        )?;
        browser_html.push_str(MIXED_LINE_OPEN);
        for segment in line.segments {
            match segment {
                ClassifiedMathSegment::Text(text) => {
                    if !text.is_empty() {
                        let metrics =
                            measure_compiled_mixed_prose(request, text_measurer, &text, None);
                        if !valid_metrics(&metrics) {
                            return Ok(None);
                        }
                        line_width_px += metrics.width;
                        line_height_px = line_height_px.max(metrics.height);
                    }
                    preflight_raw_math_output(
                        request,
                        work_meter,
                        browser_html.len().saturating_add(text.len()),
                    )?;
                    browser_html.push_str(&text);
                }
                ClassifiedMathSegment::Formula(formula) => {
                    charge_formula_work(Some(work_meter), &formula)?;
                    let Some(rendered) =
                        render_compiled_ratex_formula(&formula, &applied_foreground)
                    else {
                        return Ok(None);
                    };
                    line_width_px += rendered.width_em * font_size_px;
                    let math_height_px = rendered.height_em * font_size_px;
                    line_height_px = line_height_px.max(math_height_px);
                    max_math_height_px = max_math_height_px.max(math_height_px);
                    preflight_raw_math_output(
                        request,
                        work_meter,
                        browser_html.len().saturating_add(rendered.svg.len()),
                    )?;
                    browser_html.push_str(&rendered.svg);
                }
            }
        }
        preflight_raw_math_output(request, work_meter, browser_html.len().saturating_add(6))?;
        browser_html.push_str("</div>");
        width_px = width_px.max(line_width_px);
        height_px += line_height_px;
        max_line_height_px = max_line_height_px.max(line_height_px);
        line_count = line_count.saturating_add(1);
    }

    let metrics = TextMetrics {
        width: width_px,
        height: height_px,
        line_count: line_count.max(1),
    };
    if !valid_metrics(&metrics) || max_line_height_px > metrics.height {
        return Ok(None);
    }
    Ok(Some(RawPreparedMathLabel::with_max_line_height(
        browser_html,
        metrics,
        max_line_height_px,
        Some(max_math_height_px),
    )))
}

#[cfg(feature = "math")]
fn charge_formula_work(
    work_meter: Option<&OperationWorkMeter>,
    formula: &str,
) -> Result<(), OperationWorkError> {
    if let Some(work_meter) = work_meter {
        work_meter.checkpoint(OperationPhase::Layout)?;
        work_meter.charge(1usize.saturating_add(formula.len() / 64))?;
    }
    Ok(())
}

#[cfg(feature = "math")]
pub(super) fn compiled_ratex_math_only_lines(text: &str) -> Option<Vec<ClassifiedPureMathLine>> {
    let MathLabelClassification::Pure(lines) = classify_math_label(text, None).ok()? else {
        return None;
    };
    Some(lines)
}

#[cfg(feature = "math")]
pub(super) fn svg_with_em_size(svg: String, width_em: f64, height_em: f64) -> String {
    let Some(open_end) = svg.find('>') else {
        return svg;
    };
    let Some(body_with_close) = svg.get(open_end + 1..) else {
        return svg;
    };
    let Some(body) = body_with_close.strip_suffix("</svg>") else {
        return svg;
    };
    let width = fmt_num(width_em);
    let height = fmt_num(height_em);
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" width="{width}em" height="{height}em">{body}</svg>"#
    )
}

#[cfg(feature = "math")]
pub(super) fn emitted_em_dimension(value: f64) -> f64 {
    fmt_num(value).parse().unwrap_or(0.0)
}

#[cfg(feature = "math")]
pub(super) fn fmt_num(n: f64) -> String {
    format_number(n)
}

fn format_number(n: f64) -> String {
    let value = format!("{n:.6}");
    let value = value.trim_end_matches('0').trim_end_matches('.');
    if value.is_empty() || value == "-" {
        "0".to_string()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::resources::{RenderResourcePolicy, ResourceLimitId};

    use super::*;

    fn meter() -> Arc<OperationWorkMeter> {
        Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ))
    }

    #[cfg(feature = "math")]
    #[test]
    fn compiled_artifact_carries_browser_and_native_projections_with_terminal_style() {
        let backend = ConfiguredMathBackend::compiled_ratex();
        let config = MermaidConfig::from_value(serde_json::json!({ "securityLevel": "loose" }));
        let style = TextStyle {
            font_size: 24.0,
            ..TextStyle::default()
        };

        let outcome = backend
            .prepare(
                PrepareMathLabelRequest::flowchart(
                    "$$x^2$$",
                    &config,
                    &style,
                    "#e5e7eb",
                    Some(200.0),
                    WrapMode::HtmlLike,
                ),
                &meter(),
            )
            .unwrap();
        let artifact = outcome.prepared().expect("prepared pure formula");

        assert!(artifact.browser_xhtml().contains("font-size:24px"));
        assert!(artifact.browser_xhtml().contains("color:#e5e7eb"));
        assert!(artifact.browser_xhtml().contains("#e5e7eb"));
        assert!(
            artifact
                .browser_xhtml()
                .contains(r#"data-merman-prepared-math-native="v1""#)
        );
        assert!(
            artifact
                .browser_xhtml()
                .contains(r#"<template data-merman-prepared-math-projection="v1">"#)
        );
        assert!(artifact.browser_xhtml().contains("<path"));
        assert!(!artifact.browser_xhtml().contains("$$"));
        assert_eq!(
            artifact.retained_bytes(),
            std::mem::size_of::<PreparedMathLabel>()
                + artifact.browser_xhtml().len()
                + "#e5e7eb".len()
        );
    }

    #[cfg(feature = "math")]
    #[test]
    fn compiled_mixed_flowchart_artifact_prepares_prose_formulas_and_metrics() {
        let backend = ConfiguredMathBackend::compiled_ratex();
        let config = MermaidConfig::from_value(serde_json::json!({ "securityLevel": "loose" }));
        let text_measurer = crate::text::DeterministicTextMeasurer::default();
        let style = TextStyle {
            font_size: 20.0,
            ..TextStyle::default()
        };

        let outcome = backend
            .prepare(
                PrepareMathLabelRequest::flowchart(
                    r"value: $$x^2$$<br>Solve: $$\textcolor{red}{y}$$",
                    &config,
                    &style,
                    "#334155",
                    Some(240.0),
                    WrapMode::HtmlLike,
                )
                .with_text_measurer(&text_measurer),
                &meter(),
            )
            .unwrap();
        let artifact = outcome
            .prepared()
            .expect("compiled mixed Flowchart browser artifact");

        assert_eq!(artifact.metrics().line_count, 2);
        assert!(artifact.metrics().width > 0.0);
        assert!(artifact.metrics().height >= artifact.max_line_height_px());
        assert!(artifact.browser_xhtml().contains("value: "));
        assert!(artifact.browser_xhtml().contains("Solve: "));
        assert!(artifact.browser_xhtml().contains("<path"));
        assert!(!artifact.browser_xhtml().contains("$$"));
        assert!(artifact.browser_xhtml().contains("color:#334155"));
        assert!(artifact.browser_xhtml().contains("rgba(255,0,0,1)"));
    }

    #[cfg(feature = "math")]
    #[test]
    fn compiled_mixed_sequence_artifact_prepares_prose_formulas_and_metrics() {
        let backend = ConfiguredMathBackend::compiled_ratex();
        let config = MermaidConfig::from_value(serde_json::json!({ "securityLevel": "loose" }));
        let text_measurer = crate::text::DeterministicTextMeasurer::default();
        let style = TextStyle {
            font_size: 18.0,
            ..TextStyle::default()
        };

        let outcome = backend
            .prepare(
                PrepareMathLabelRequest::sequence(
                    r"Solve: $$\sqrt{2+2}$$<br>Answer: $$\textcolor{red}{2}$$",
                    &config,
                    &style,
                    "#475569",
                )
                .with_text_measurer(&text_measurer),
                &meter(),
            )
            .unwrap();
        let artifact = outcome
            .prepared()
            .expect("compiled mixed Sequence browser artifact");

        assert_eq!(artifact.metrics().line_count, 2);
        assert!(artifact.metrics().width > 0.0);
        assert!(artifact.metrics().height >= artifact.max_line_height_px());
        assert!(artifact.browser_xhtml().contains("Solve: "));
        assert!(artifact.browser_xhtml().contains("Answer: "));
        assert!(artifact.browser_xhtml().contains("<path"));
        assert!(!artifact.browser_xhtml().contains("$$"));
        assert!(artifact.browser_xhtml().contains("color:#475569"));
        assert!(artifact.browser_xhtml().contains("rgba(255,0,0,1)"));
    }

    #[cfg(feature = "math")]
    #[test]
    fn compiled_mixed_sequence_prose_keeps_svg_single_run_measurement_semantics() {
        #[derive(Default)]
        struct RecordingTextMeasurer {
            modes: std::sync::Mutex<Vec<WrapMode>>,
        }

        impl TextMeasurer for RecordingTextMeasurer {
            fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
                TextMetrics {
                    width: 13.0,
                    height: 17.0,
                    line_count: 1,
                }
            }

            fn measure_wrapped(
                &self,
                _text: &str,
                _style: &TextStyle,
                _max_width: Option<f64>,
                wrap_mode: WrapMode,
            ) -> TextMetrics {
                self.modes.lock().unwrap().push(wrap_mode);
                self.measure("", &TextStyle::default())
            }
        }

        let backend = ConfiguredMathBackend::compiled_ratex();
        let config = MermaidConfig::default();
        let style = TextStyle::default();
        let text_measurer = RecordingTextMeasurer::default();

        let outcome = backend
            .prepare(
                PrepareMathLabelRequest::sequence("Solve: $$x$$", &config, &style, "#475569")
                    .with_text_measurer(&text_measurer),
                &meter(),
            )
            .unwrap();

        assert!(outcome.prepared().is_some());
        assert_eq!(
            *text_measurer.modes.lock().unwrap(),
            vec![WrapMode::SvgLikeSingleRun]
        );
    }

    #[derive(Debug, Default)]
    struct CountingExternalRenderer {
        render_calls: AtomicUsize,
        measure_calls: AtomicUsize,
    }

    impl MathRenderer for CountingExternalRenderer {
        fn render_html_label(&self, text: &str, _config: &MermaidConfig) -> Option<String> {
            self.render_calls.fetch_add(1, Ordering::SeqCst);
            Some(format!("<span>{text}</span>"))
        }

        fn measure_html_label(
            &self,
            _text: &str,
            _config: &MermaidConfig,
            _style: &TextStyle,
            _max_width_px: Option<f64>,
            _wrap_mode: WrapMode,
        ) -> Option<TextMetrics> {
            self.measure_calls.fetch_add(1, Ordering::SeqCst);
            Some(TextMetrics {
                width: 31.0,
                height: 17.0,
                line_count: 1,
            })
        }
    }

    #[test]
    fn compatibility_prepare_is_measure_first_and_writer_consumption_is_inert() {
        let renderer = Arc::new(CountingExternalRenderer::default());
        let backend = ConfiguredMathBackend::external(renderer.clone());
        let config = MermaidConfig::default();
        let style = TextStyle::default();

        let outcome = backend
            .prepare(
                PrepareMathLabelRequest::flowchart(
                    "$$x$$",
                    &config,
                    &style,
                    "#334155",
                    Some(200.0),
                    WrapMode::HtmlLike,
                ),
                &meter(),
            )
            .unwrap();
        let artifact = outcome.prepared().expect("external browser artifact");

        assert_eq!(renderer.measure_calls.load(Ordering::SeqCst), 1);
        assert_eq!(renderer.render_calls.load(Ordering::SeqCst), 1);
        let _ = artifact.metrics();
        let _ = artifact.browser_xhtml();
        assert_eq!(renderer.measure_calls.load(Ordering::SeqCst), 1);
        assert_eq!(renderer.render_calls.load(Ordering::SeqCst), 1);
    }

    #[derive(Debug)]
    struct OversizedExternalRenderer {
        render_calls: AtomicUsize,
        output_bytes: usize,
    }

    impl MathRenderer for OversizedExternalRenderer {
        fn render_html_label(&self, _text: &str, _config: &MermaidConfig) -> Option<String> {
            self.render_calls.fetch_add(1, Ordering::SeqCst);
            Some("<".repeat(self.output_bytes))
        }

        fn measure_html_label(
            &self,
            _text: &str,
            _config: &MermaidConfig,
            _style: &TextStyle,
            _max_width_px: Option<f64>,
            _wrap_mode: WrapMode,
        ) -> Option<TextMetrics> {
            Some(TextMetrics {
                width: 10.0,
                height: 10.0,
                line_count: 1,
            })
        }
    }

    #[test]
    fn opaque_output_is_rejected_before_sanitizer_or_wrapper_amplification() {
        let renderer = Arc::new(OversizedExternalRenderer {
            render_calls: AtomicUsize::new(0),
            output_bytes: 4_096,
        });
        let backend = ConfiguredMathBackend::external(renderer.clone());
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 256)
            .unwrap();
        let meter = Arc::new(OperationWorkMeter::new(policy));

        let error = backend
            .prepare(
                PrepareMathLabelRequest::flowchart(
                    "$$x$$",
                    &MermaidConfig::default(),
                    &TextStyle::default(),
                    "#334155",
                    None,
                    WrapMode::HtmlLike,
                ),
                &meter,
            )
            .unwrap_err();
        let OperationWorkError::ResourceLimitExceeded(error) = error else {
            panic!("expected prepared-math resource limit")
        };
        assert_eq!(error.limit, "max_prepared_text_retained_bytes");
        assert_eq!(error.max, 256);
        assert!(error.actual > error.max);
        assert_eq!(renderer.render_calls.load(Ordering::SeqCst), 1);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn external_browser_backend_accepts_a_safe_css_foreground() {
        let renderer = Arc::new(CountingExternalRenderer::default());
        let backend = ConfiguredMathBackend::external(renderer);
        let outcome = backend
            .prepare(
                PrepareMathLabelRequest::flowchart(
                    "$$x$$",
                    &MermaidConfig::default(),
                    &TextStyle::default(),
                    "var(--message-color, currentColor)",
                    Some(200.0),
                    WrapMode::HtmlLike,
                ),
                &meter(),
            )
            .unwrap();

        let prepared = outcome.prepared().expect("safe browser CSS foreground");
        assert!(
            prepared
                .browser_xhtml()
                .contains("color:var(--message-color, currentColor)")
        );
    }

    #[derive(Debug, Default)]
    struct DecliningMeasurementRenderer {
        render_calls: AtomicUsize,
        measure_calls: AtomicUsize,
    }

    impl MathRenderer for DecliningMeasurementRenderer {
        fn render_html_label(&self, _text: &str, _config: &MermaidConfig) -> Option<String> {
            self.render_calls.fetch_add(1, Ordering::SeqCst);
            Some("unexpected".to_string())
        }

        fn measure_html_label(
            &self,
            _text: &str,
            _config: &MermaidConfig,
            _style: &TextStyle,
            _max_width_px: Option<f64>,
            _wrap_mode: WrapMode,
        ) -> Option<TextMetrics> {
            self.measure_calls.fetch_add(1, Ordering::SeqCst);
            None
        }
    }

    #[test]
    fn render_only_external_backend_keeps_browser_math_with_fallback_metrics() {
        let renderer = Arc::new(DecliningMeasurementRenderer::default());
        let backend = ConfiguredMathBackend::external(renderer.clone());
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let outcome = backend
            .prepare(
                PrepareMathLabelRequest::flowchart(
                    "$$x$$",
                    &MermaidConfig::default(),
                    &TextStyle::default(),
                    "#334155",
                    Some(200.0),
                    WrapMode::HtmlLike,
                )
                .with_text_measurer(&measurer),
                &meter(),
            )
            .unwrap();

        let prepared = outcome.prepared().expect("render-only browser artifact");
        assert!(prepared.metrics().width > 0.0);
        assert!(prepared.browser_xhtml().contains("unexpected"));
        assert_eq!(renderer.measure_calls.load(Ordering::SeqCst), 1);
        assert_eq!(renderer.render_calls.load(Ordering::SeqCst), 1);
    }

    #[cfg(feature = "math")]
    #[test]
    fn classification_distinguishes_pure_mixed_and_unmatched_delimiters_once() {
        assert!(matches!(
            classify_math_label("$$x$$<br>$$y$$", None).unwrap(),
            MathLabelClassification::Pure(lines)
                if lines == [
                    ClassifiedPureMathLine::Formula("x".to_string()),
                    ClassifiedPureMathLine::Formula("y".to_string()),
                ]
        ));
        assert!(matches!(
            classify_math_label("value: $$x$$", None).unwrap(),
            MathLabelClassification::Mixed(_)
        ));
        assert!(matches!(
            classify_math_label("literal $$", None).unwrap(),
            MathLabelClassification::NotMath
        ));
    }

    #[cfg(feature = "math")]
    #[test]
    fn compiled_pure_math_preserves_empty_br_lines_in_terminal_geometry() {
        let backend = ConfiguredMathBackend::compiled_ratex();
        let style = TextStyle::default();
        let outcome = backend
            .prepare(
                PrepareMathLabelRequest::sequence(
                    "$$x$$<br><br>$$y$$",
                    &MermaidConfig::default(),
                    &style,
                    "#334155",
                ),
                &meter(),
            )
            .unwrap();
        let prepared = outcome.prepared().expect("compiled pure math artifact");
        let compact = backend
            .prepare(
                PrepareMathLabelRequest::sequence(
                    "$$x$$<br>$$y$$",
                    &MermaidConfig::default(),
                    &style,
                    "#334155",
                ),
                &meter(),
            )
            .unwrap();
        let compact = compact.prepared().expect("compact pure math artifact");

        assert_eq!(prepared.metrics().line_count, 3);
        assert!(prepared.metrics().height > compact.metrics().height);
        assert_eq!(
            prepared
                .browser_xhtml()
                .matches("display:flex;align-items:center")
                .count(),
            3
        );
    }
}

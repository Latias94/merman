//! Prepared SVG label rows for the Flow families.

mod occurrence;
mod terminal;

pub use occurrence::{
    FLOWCHART_RENDER_ID_RECORD_BYTES, FlowchartPreparedLabelResolution, FlowchartSvgLabelOwner,
    FlowchartSvgLabelRenderPlan, FlowchartSvgLabelSidecar, PendingFlowchartSvgLabels,
    PreparedFlowchartHtmlLabel, PreparedFlowchartMathLabel,
};
pub use terminal::{
    OptionalStyleAttr, append_native_label_color, flowchart_label_html_with_prepared_label,
    write_flowchart_svg_label_plan, write_flowchart_svg_label_plan_with_style,
    write_flowchart_svg_source_word_lines, write_flowchart_svg_text_markdown,
    write_flowchart_svg_text_markdown_wrapped,
    write_flowchart_svg_text_markdown_wrapped_with_style,
};

use std::{borrow::Cow, sync::Arc};

use merman_render_core::resources::{
    OperationWorkMeter, PreparedTextRetainedReservation, ResourceLimitExceeded,
};
use merman_render_core::text::{
    TextMeasurer, TextMetrics, TextStyle,
    inline_html::{MermaidMarkdownWordType, SvgTextEntityMode},
    literal_label::{FlowchartSvgLabelSource, FlowchartSvgWidthMode},
    measure_svg_markdown_word_lines,
    measurement::{MeasurementReuseKey, TextMeasurementOperation, measurement_reuse_key},
};

pub const FLOWCHART_PREPARED_OWNER_SLOT_BYTES: usize = 192;

#[derive(Debug)]
pub struct FlowchartNodeSourceTypography {
    style: TextStyle,
    _retained_reservation: Option<PreparedTextRetainedReservation>,
}

impl FlowchartNodeSourceTypography {
    pub fn prepare(
        style: Cow<'_, TextStyle>,
        work: Option<&Arc<OperationWorkMeter>>,
    ) -> Result<Self, ResourceLimitExceeded> {
        let retained_bytes = std::mem::size_of::<Self>()
            .saturating_add(style.font_family.as_ref().map_or(0, String::len))
            .saturating_add(style.font_weight.as_ref().map_or(0, String::len))
            .saturating_add(style.font_style.as_ref().map_or(0, String::len))
            .saturating_add(FLOWCHART_PREPARED_OWNER_SLOT_BYTES);
        let reservation = work
            .map(|work| work.reserve_prepared_text_retained_bytes(retained_bytes))
            .transpose()?;
        Ok(Self {
            style: style.into_owned(),
            _retained_reservation: reservation,
        })
    }
}

impl std::ops::Deref for FlowchartNodeSourceTypography {
    type Target = TextStyle;

    fn deref(&self) -> &Self::Target {
        &self.style
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FlowchartSvgTextStyleKey {
    font_family: Option<String>,
    font_size_bits: u64,
    font_weight: Option<String>,
    font_style: Option<String>,
}

impl FlowchartSvgTextStyleKey {
    fn new(style: &TextStyle) -> Self {
        Self {
            font_family: style.font_family.clone(),
            font_size_bits: canonical_f64_bits(style.font_size),
            font_weight: style.font_weight.clone(),
            font_style: style.font_style.clone(),
        }
    }

    fn matches(&self, style: &TextStyle) -> bool {
        self.font_family.as_deref() == style.font_family.as_deref()
            && self.font_size_bits == canonical_f64_bits(style.font_size)
            && self.font_weight.as_deref() == style.font_weight.as_deref()
            && self.font_style.as_deref() == style.font_style.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FlowchartSvgLabelBinding {
    wrap_style: FlowchartSvgTextStyleKey,
    metrics_style: FlowchartSvgTextStyleKey,
    max_width_bits: Option<u64>,
    break_long_words: bool,
    width_mode: FlowchartSvgWidthMode,
    computed_length_key: Option<MeasurementReuseKey>,
    wrapped_key: Option<MeasurementReuseKey>,
}

#[derive(Debug, Clone, Copy)]
pub struct FlowchartSvgLabelWrapBindingRequest<'a> {
    wrap_style: &'a TextStyle,
    max_width_bits: Option<u64>,
    break_long_words: bool,
    computed_length_key: Option<MeasurementReuseKey>,
}

impl<'a> FlowchartSvgLabelWrapBindingRequest<'a> {
    pub fn for_measurer(
        measurer: &dyn TextMeasurer,
        wrap_style: &'a TextStyle,
        max_width_px: Option<f64>,
        break_long_words: bool,
    ) -> Option<Self> {
        Some(Self {
            wrap_style,
            max_width_bits: normalized_label_width_bits(max_width_px),
            break_long_words,
            computed_length_key: Some(measurement_reuse_key(
                measurer,
                TextMeasurementOperation::ComputedLength,
            )?),
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct FlowchartSvgLabelBindingRequest<'a> {
    wrap_style: &'a TextStyle,
    metrics_style: &'a TextStyle,
    max_width_bits: Option<u64>,
    break_long_words: bool,
    width_mode: FlowchartSvgWidthMode,
    computed_length_key: Option<MeasurementReuseKey>,
    wrapped_key: Option<MeasurementReuseKey>,
}

impl<'a> FlowchartSvgLabelBindingRequest<'a> {
    fn for_measurer(
        measurer: &dyn TextMeasurer,
        wrap_style: &'a TextStyle,
        metrics_style: &'a TextStyle,
        max_width_px: Option<f64>,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
    ) -> Self {
        let max_width_bits = normalized_label_width_bits(max_width_px);
        let computed_length_key =
            measurement_reuse_key(measurer, TextMeasurementOperation::ComputedLength);
        let wrapped_key = computed_length_key
            .and_then(|_| measurement_reuse_key(measurer, TextMeasurementOperation::Wrapped));
        Self {
            wrap_style,
            metrics_style,
            max_width_bits,
            break_long_words,
            width_mode,
            computed_length_key,
            wrapped_key,
        }
    }

    fn is_reusable(&self) -> bool {
        self.wrapped_key.is_some()
    }

    fn into_owned(self) -> Option<FlowchartSvgLabelBinding> {
        self.is_reusable().then(|| FlowchartSvgLabelBinding {
            wrap_style: FlowchartSvgTextStyleKey::new(self.wrap_style),
            metrics_style: FlowchartSvgTextStyleKey::new(self.metrics_style),
            max_width_bits: self.max_width_bits,
            break_long_words: self.break_long_words,
            width_mode: self.width_mode,
            computed_length_key: self.computed_length_key,
            wrapped_key: self.wrapped_key,
        })
    }
}

impl FlowchartSvgLabelBinding {
    fn matches_wrapping(&self, request: &FlowchartSvgLabelWrapBindingRequest<'_>) -> bool {
        self.wrap_style.matches(request.wrap_style)
            && self.max_width_bits == request.max_width_bits
            && self.break_long_words == request.break_long_words
            && self.computed_length_key == request.computed_length_key
    }

    fn matches(&self, request: &FlowchartSvgLabelBindingRequest<'_>) -> bool {
        self.wrap_style.matches(request.wrap_style)
            && self.metrics_style.matches(request.metrics_style)
            && self.max_width_bits == request.max_width_bits
            && self.break_long_words == request.break_long_words
            && self.width_mode == request.width_mode
            && self.computed_length_key == request.computed_length_key
            && self.wrapped_key == request.wrapped_key
    }
}

fn canonical_f64_bits(value: f64) -> u64 {
    if value == 0.0 {
        0.0_f64.to_bits()
    } else if value.is_nan() {
        f64::NAN.to_bits()
    } else {
        value.to_bits()
    }
}

/// Canonicalizes optional label widths for both SVG and math artifact matching.
pub fn normalized_label_width_bits(width: Option<f64>) -> Option<u64> {
    width
        .filter(|width| width.is_finite() && *width > 0.0)
        .map(canonical_f64_bits)
}

/// A fully measured label bound to one exact style, width, wrapping mode, and built-in route.
#[derive(Debug)]
pub struct PreparedFlowchartSvgLabel {
    binding: FlowchartSvgLabelBinding,
    wrapped_lines: Vec<Vec<String>>,
    metrics: TextMetrics,
}

/// An SVG source projection with its original spelling for occurrence matching.
#[derive(Debug)]
pub struct FlowchartSvgLabelSourceEntry {
    raw_source: Box<str>,
    source: FlowchartSvgLabelSource,
}

impl FlowchartSvgLabelSourceEntry {
    pub fn new(raw_source: &str) -> Self {
        Self {
            raw_source: raw_source.into(),
            source: FlowchartSvgLabelSource::new(raw_source),
        }
    }

    pub fn matches(&self, raw_source: &str) -> bool {
        self.raw_source.as_ref() == raw_source
    }

    pub fn source(&self) -> &FlowchartSvgLabelSource {
        &self.source
    }
}

impl PreparedFlowchartSvgLabel {
    pub fn wrapped_lines(&self) -> &[Vec<String>] {
        &self.wrapped_lines
    }

    pub fn matches_wrapping(&self, binding: &FlowchartSvgLabelWrapBindingRequest<'_>) -> bool {
        self.binding.matches_wrapping(binding)
    }
}

/// The wrap and metric inputs used for one SVG row measurement.
///
/// Resolve the route before lookup. Opaque routes can still measure and reuse a source projection.
pub struct FlowchartSvgLabelMeasurementRequest<'a> {
    measurer: &'a dyn TextMeasurer,
    max_width_px: Option<f64>,
    binding: FlowchartSvgLabelBindingRequest<'a>,
}

impl<'a> FlowchartSvgLabelMeasurementRequest<'a> {
    pub fn new(
        measurer: &'a dyn TextMeasurer,
        wrap_style: &'a TextStyle,
        metrics_style: &'a TextStyle,
        max_width_px: Option<f64>,
        break_long_words: bool,
        width_mode: FlowchartSvgWidthMode,
    ) -> Self {
        Self {
            measurer,
            max_width_px,
            binding: FlowchartSvgLabelBindingRequest::for_measurer(
                measurer,
                wrap_style,
                metrics_style,
                max_width_px,
                break_long_words,
                width_mode,
            ),
        }
    }

    pub fn has_reusable_binding(&self) -> bool {
        self.binding.is_reusable()
    }

    pub fn matching_metrics(&self, prepared: &PreparedFlowchartSvgLabel) -> Option<TextMetrics> {
        (self.has_reusable_binding() && prepared.binding.matches(&self.binding))
            .then_some(prepared.metrics)
    }

    /// Measures projected rows. Use `plain_markdown` after admitting a Markdown label
    /// to the literal SVG row path.
    pub fn measure(
        &self,
        source: &FlowchartSvgLabelSource,
        plain_markdown: bool,
    ) -> MeasuredFlowchartSvgLabel<'a> {
        let wrapped_lines = source.wrapped_lines(
            self.measurer,
            self.binding.wrap_style,
            self.max_width_px,
            self.binding.break_long_words,
        );
        let metrics = if plain_markdown {
            // Plain Markdown shares the prepared SVG rows, but retains its computed-length
            // measurement and 1/64px width lattice rather than the non-Markdown bbox path.
            let markdown_lines = wrapped_lines
                .iter()
                .map(|line| {
                    line.iter()
                        .map(|word| (word.clone(), MermaidMarkdownWordType::Normal))
                        .collect()
                })
                .collect::<Vec<_>>();
            measure_svg_markdown_word_lines(
                self.measurer,
                &markdown_lines,
                self.binding.metrics_style,
                SvgTextEntityMode::CreateTextSource,
            )
        } else {
            source.metrics_from_wrapped(
                self.measurer,
                self.binding.metrics_style,
                &wrapped_lines,
                self.binding.width_mode,
            )
        };
        MeasuredFlowchartSvgLabel {
            binding: self.binding,
            wrapped_lines,
            metrics,
        }
    }
}

/// Wrapped rows and layout metrics awaiting retention for their label occurrence.
pub struct MeasuredFlowchartSvgLabel<'a> {
    binding: FlowchartSvgLabelBindingRequest<'a>,
    wrapped_lines: Vec<Vec<String>>,
    metrics: TextMetrics,
}

impl MeasuredFlowchartSvgLabel<'_> {
    pub fn metrics(&self) -> TextMetrics {
        self.metrics
    }

    pub fn into_prepared(self) -> Option<PreparedFlowchartSvgLabel> {
        Some(PreparedFlowchartSvgLabel {
            binding: self.binding.into_owned()?,
            wrapped_lines: self.wrapped_lines,
            metrics: self.metrics,
        })
    }
}

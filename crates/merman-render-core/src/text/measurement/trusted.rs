//! Sealed route authority and streaming width probes.

use super::super::deterministic::DeterministicMode;
use super::super::{HeuristicTextWidth, TextStyle};
#[cfg(feature = "resources")]
use super::{TextMeasurementOperation, TextMeasurementPhase};
#[cfg(test)]
use std::cell::Cell;
use std::cell::RefCell;
#[cfg(test)]
use std::rc::Rc;
use unicode_segmentation::UnicodeSegmentation;

pub(in crate::text) struct TrustedMeasurementQuery(());
pub(in crate::text) const TRUSTED_MEASUREMENT: TrustedMeasurementQuery =
    TrustedMeasurementQuery(());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "resources")]
pub(super) enum BuiltinTextMeasurementProfile {
    Deterministic,
    #[cfg(feature = "layout-shoal")]
    Calibrated,
}

#[cfg(feature = "resources")]
impl BuiltinTextMeasurementProfile {
    const fn mode(self) -> DeterministicMode {
        match self {
            Self::Deterministic => DeterministicMode::Compatibility,
            #[cfg(feature = "layout-shoal")]
            Self::Calibrated => DeterministicMode::Calibrated,
        }
    }
}

#[cfg(feature = "resources")]
#[doc(hidden)]
/// Equality-only identity of one prepared measurement route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeasurementReuseKey {
    profile: BuiltinTextMeasurementProfile,
    phase: TextMeasurementPhase,
    operation: TextMeasurementOperation,
}

#[cfg(feature = "resources")]
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::text) struct BuiltinTextMeasurementOperationCarrier(MeasurementReuseKey);

#[cfg(feature = "resources")]
pub fn measurement_reuse_key<M: super::super::TextMeasurer + ?Sized>(
    measurer: &M,
    operation: TextMeasurementOperation,
) -> Option<MeasurementReuseKey> {
    measurer
        .builtin_operation_carrier(TRUSTED_MEASUREMENT, operation)
        .map(|carrier| carrier.0)
}

#[cfg(feature = "resources")]
impl BuiltinTextMeasurementOperationCarrier {
    pub(super) fn new(
        profile: BuiltinTextMeasurementProfile,
        phase: TextMeasurementPhase,
        operation: TextMeasurementOperation,
    ) -> Self {
        Self(MeasurementReuseKey {
            profile,
            phase,
            operation,
        })
    }

    #[cfg(feature = "svg-parse")]
    pub(super) const fn into_inline_html(self) -> Option<InlineHtmlMeasurementCarrier> {
        match (self.0.phase, self.0.operation) {
            (TextMeasurementPhase::Wrap, TextMeasurementOperation::WrappedWithRawWidth) => {
                Some(InlineHtmlMeasurementCarrier::builtin(self.0.profile))
            }
            _ => None,
        }
    }

    #[cfg(feature = "svg-output")]
    pub(super) fn into_svg_computed_length(
        self,
        style: &TextStyle,
    ) -> Option<BuiltinSvgComputedLength> {
        match (self.0.phase, self.0.operation) {
            (TextMeasurementPhase::ComputedLength, TextMeasurementOperation::ComputedLength) => {
                Some(BuiltinSvgComputedLength::with_mode(
                    style,
                    self.0.profile.mode(),
                ))
            }
            _ => None,
        }
    }

    pub(super) fn into_html_line_width(self, style: &TextStyle) -> Option<BuiltinHtmlLineWidth> {
        match (self.0.phase, self.0.operation) {
            (TextMeasurementPhase::Wrap, TextMeasurementOperation::CanvasMeasureTextWidth) => Some(
                BuiltinHtmlLineWidth::with_mode(style, 0.0, self.0.profile.mode()),
            ),
            _ => None,
        }
    }
}

/// Private authority for one complete rich HTML measurement operation.
///
/// Only `RoutedTextMeasurer` can attach a built-in profile after resolving the operation's
/// owning phase. Arbitrary `TextMeasurer` implementations receive [`Self::opaque`], so custom and
/// host routes cannot copy or replay built-in authority through the public trait surface.
#[cfg(feature = "svg-parse")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::text) struct InlineHtmlMeasurementCarrier {
    builtin: Option<BuiltinTextMeasurementProfile>,
}

#[cfg(feature = "svg-parse")]
impl InlineHtmlMeasurementCarrier {
    pub(in crate::text) fn for_measurer<M: super::super::TextMeasurer + ?Sized>(
        measurer: &M,
    ) -> Self {
        measurer
            .builtin_operation_carrier(
                TRUSTED_MEASUREMENT,
                TextMeasurementOperation::WrappedWithRawWidth,
            )
            .and_then(BuiltinTextMeasurementOperationCarrier::into_inline_html)
            .unwrap_or_else(Self::opaque)
    }

    pub(in crate::text) const fn opaque() -> Self {
        Self { builtin: None }
    }

    pub(super) const fn builtin(profile: BuiltinTextMeasurementProfile) -> Self {
        Self {
            builtin: Some(profile),
        }
    }

    pub(in crate::text) const fn is_builtin(self) -> bool {
        self.builtin.is_some()
    }

    pub(in crate::text) fn begin_inline_html_width(
        self,
        style: &TextStyle,
    ) -> Option<BuiltinHtmlLineWidth> {
        self.builtin
            .map(|profile| BuiltinHtmlLineWidth::with_mode(style, 0.0, profile.mode()))
    }
}

#[derive(Debug, Clone)]
struct BuiltinInlineRawLineWidth {
    font_size: f64,
    model: HeuristicTextWidth,
    state: RefCell<BuiltinInlineRawLineWidthState>,
}

#[derive(Debug, Clone, Default)]
struct BuiltinInlineRawLineWidthState {
    committed_em: f64,
    pending_grapheme: String,
    pending_width_dirty: bool,
    pending_em: f64,
    #[cfg(test)]
    grapheme_input_byte_charge: Rc<Cell<usize>>,
}

/// Streaming width of a decoded, single HTML line for a qualified built-in wrapping route.
///
/// Retain only the final extendable grapheme rather than remeasure every growing candidate.
/// The input is already decoded text: literal `<br>` spellings are never interpreted as markup.
/// U+00AD retains its shaping boundary with zero advance until a discretionary break is selected.
/// Other scalar-count semantics stay unchanged for uniform-advance profiles.
#[derive(Debug)]
pub(in crate::text) struct BuiltinHtmlLineWidth {
    source: BuiltinHtmlLineWidthSource,
    spacing: Option<Box<StreamingHtmlSpacing>>,
    #[cfg(test)]
    pub(in crate::text) pushed_chunks: usize,
}

impl Clone for BuiltinHtmlLineWidth {
    fn clone(&self) -> Self {
        // Wrapping snapshots may outnumber width queries. Keep only the extendable final
        // grapheme in each snapshot instead of copying every already measured prefix.
        self.width_px();
        Self {
            source: self.source.clone(),
            spacing: self.spacing.clone(),
            #[cfg(test)]
            pushed_chunks: self.pushed_chunks,
        }
    }
}

#[derive(Debug, Clone)]
struct StreamingHtmlSpacing {
    geometry: crate::mermaid_style::HtmlTextSpacing,
    pending: RefCell<(f64, String)>,
}

impl StreamingHtmlSpacing {
    fn width_px(&self) -> f64 {
        let mut pending = self.pending.borrow_mut();
        let (total, text) = &mut *pending;
        let tail = text
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(tail, _)| tail);
        if tail != 0 {
            *total += self.geometry.advance(&text[..tail]);
            text.drain(..tail);
        }
        *total + self.geometry.advance(text)
    }
}

#[derive(Debug, Clone)]
enum BuiltinHtmlLineWidthSource {
    Heuristic(BuiltinInlineRawLineWidth),
    Uniform {
        font_size: f64,
        char_width_factor: f64,
        char_count: usize,
    },
}

impl BuiltinHtmlLineWidth {
    pub(in crate::text) fn deterministic(style: &TextStyle, char_width_factor: f64) -> Self {
        Self::with_mode(style, char_width_factor, DeterministicMode::Compatibility)
    }

    pub(in crate::text) fn with_mode(
        style: &TextStyle,
        char_width_factor: f64,
        mode: DeterministicMode,
    ) -> Self {
        if char_width_factor == 0.0 {
            let (model, size) = mode.width_model(style);
            Self::heuristic(model, size)
        } else {
            Self::with_source(BuiltinHtmlLineWidthSource::Uniform {
                font_size: style.font_size.max(0.0),
                char_width_factor,
                char_count: 0,
            })
        }
    }

    pub(in crate::text) fn heuristic(model: HeuristicTextWidth, font_size: f64) -> Self {
        Self::with_source(BuiltinHtmlLineWidthSource::Heuristic(
            BuiltinInlineRawLineWidth::new(model, font_size),
        ))
    }

    fn with_source(source: BuiltinHtmlLineWidthSource) -> Self {
        Self {
            source,
            spacing: None,
            #[cfg(test)]
            pushed_chunks: 0,
        }
    }

    pub(in crate::text) fn with_spacing(
        mut self,
        geometry: crate::mermaid_style::HtmlTextSpacing,
    ) -> Self {
        if !geometry.is_normal() {
            self.spacing = Some(Box::new(StreamingHtmlSpacing {
                geometry,
                pending: RefCell::default(),
            }));
        }
        self
    }

    pub(in crate::text) fn push_text(&mut self, text: &str) {
        #[cfg(feature = "benchmark-stages")]
        let _stage =
            merman_model::benchmark_stages::enter(merman_model::benchmark_stages::Stage::Measure);
        #[cfg(test)]
        {
            self.pushed_chunks = self.pushed_chunks.saturating_add(1);
        }
        match &mut self.source {
            BuiltinHtmlLineWidthSource::Heuristic(line) => line.push_text(text),
            BuiltinHtmlLineWidthSource::Uniform { char_count, .. } => {
                *char_count =
                    char_count.saturating_add(text.chars().filter(|ch| *ch != '\u{00ad}').count());
            }
        }
        if let Some(spacing) = &mut self.spacing {
            spacing.pending.get_mut().1.push_str(text);
        }
    }

    pub(in crate::text) fn width_px(&self) -> f64 {
        #[cfg(feature = "benchmark-stages")]
        let _stage =
            merman_model::benchmark_stages::enter(merman_model::benchmark_stages::Stage::Measure);
        let width = match &self.source {
            BuiltinHtmlLineWidthSource::Heuristic(line) => line.width_px(),
            BuiltinHtmlLineWidthSource::Uniform {
                font_size,
                char_width_factor,
                char_count,
            } => *char_count as f64 * *font_size * *char_width_factor,
        };
        // An unwrapped built-in HTML measurement starts its maximum-width fold at zero.
        (width
            + self
                .spacing
                .as_ref()
                .map_or(0.0, |spacing| spacing.width_px()))
        .max(0.0)
    }

    #[cfg(all(test, feature = "svg-parse"))]
    pub(super) fn grapheme_input_byte_charge(&self) -> usize {
        match &self.source {
            BuiltinHtmlLineWidthSource::Heuristic(line) => line.grapheme_input_byte_charge(),
            BuiltinHtmlLineWidthSource::Uniform { .. } => 0,
        }
    }

    #[cfg(all(test, feature = "svg-parse"))]
    pub(super) fn retained_grapheme_bytes(&self) -> usize {
        match &self.source {
            BuiltinHtmlLineWidthSource::Heuristic(line) => line.retained_grapheme_bytes(),
            BuiltinHtmlLineWidthSource::Uniform { .. } => 0,
        }
    }

    pub(in crate::text) fn reset(&mut self) {
        #[cfg(test)]
        {
            self.pushed_chunks = 0;
        }
        match &mut self.source {
            BuiltinHtmlLineWidthSource::Heuristic(line) => line.reset(),
            BuiltinHtmlLineWidthSource::Uniform { char_count, .. } => *char_count = 0,
        }
        if let Some(spacing) = &mut self.spacing {
            let (total, text) = spacing.pending.get_mut();
            *total = 0.0;
            text.clear();
        }
    }
}

/// Streaming `getComputedTextLength()` state for a qualified built-in SVG text route.
///
/// Flowchart's createText wrapper probes every growing word prefix. Retaining completed grapheme
/// width plus only the extendable final grapheme avoids rescanning the complete prefix while
/// preserving sequence-aware Unicode width across arbitrary chunk boundaries. Host-backed and
/// opaque custom measurers cannot construct this state, so their observable callback sequence
/// remains unchanged.
#[derive(Debug, Clone)]
#[cfg(feature = "svg-output")]
pub(in crate::text) struct BuiltinSvgComputedLength {
    line: BuiltinInlineRawLineWidth,
}

#[cfg(feature = "svg-output")]
impl BuiltinSvgComputedLength {
    pub(in crate::text) fn with_mode(style: &TextStyle, mode: DeterministicMode) -> Self {
        let (model, size) = mode.width_model(style);
        Self {
            line: BuiltinInlineRawLineWidth::new(model, size),
        }
    }

    #[cfg(test)]
    pub(in crate::text) fn deterministic(style: &TextStyle) -> Self {
        Self::with_mode(style, DeterministicMode::Compatibility)
    }

    pub(in crate::text) fn push_text(&mut self, text: &str) {
        #[cfg(feature = "benchmark-stages")]
        let _stage =
            merman_model::benchmark_stages::enter(merman_model::benchmark_stages::Stage::Measure);
        self.line.push_text(text);
    }

    pub(in crate::text) fn width_px(&self) -> f64 {
        #[cfg(feature = "benchmark-stages")]
        let _stage =
            merman_model::benchmark_stages::enter(merman_model::benchmark_stages::Stage::Measure);
        let width = self.line.width_px();
        if width.is_finite() && width >= 0.0 {
            width
        } else {
            0.0
        }
    }

    pub(in crate::text) fn reset(&mut self) {
        self.line.reset();
    }
}

impl BuiltinInlineRawLineWidth {
    fn new(model: HeuristicTextWidth, font_size: f64) -> Self {
        Self {
            font_size,
            model,
            state: RefCell::new(BuiltinInlineRawLineWidthState::default()),
        }
    }

    fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let state = self.state.get_mut();
        state.pending_grapheme.push_str(text);
        state.pending_width_dirty = true;
    }

    fn refresh_width(model: HeuristicTextWidth, state: &mut BuiltinInlineRawLineWidthState) {
        if !state.pending_width_dirty {
            return;
        }

        #[cfg(test)]
        {
            // Charge the full pending input to both boundary discovery and width estimation. This
            // is a conservative structural upper bound, not a count of CPU instructions.
            let next_charge = state
                .grapheme_input_byte_charge
                .get()
                .saturating_add(state.pending_grapheme.len().saturating_mul(2));
            state.grapheme_input_byte_charge.set(next_charge);
        }

        let last_grapheme_start = state
            .pending_grapheme
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(index, _)| index);
        if last_grapheme_start > 0 {
            model.append(
                &mut state.committed_em,
                &state.pending_grapheme[..last_grapheme_start],
            );
            state.pending_grapheme.drain(..last_grapheme_start);
        }
        state.pending_em = model.width(&state.pending_grapheme);
        state.pending_width_dirty = false;
    }

    fn width_px(&self) -> f64 {
        let mut state = self.state.borrow_mut();
        Self::refresh_width(self.model, &mut state);
        (state.committed_em + state.pending_em) * self.font_size
    }

    fn reset(&mut self) {
        let state = self.state.get_mut();
        state.committed_em = 0.0;
        state.pending_grapheme.clear();
        state.pending_width_dirty = false;
        state.pending_em = 0.0;
    }

    #[cfg(all(test, feature = "svg-parse"))]
    pub(super) fn grapheme_input_byte_charge(&self) -> usize {
        self.state.borrow().grapheme_input_byte_charge.get()
    }

    #[cfg(all(test, feature = "svg-parse"))]
    pub(super) fn retained_grapheme_bytes(&self) -> usize {
        self.state.borrow().pending_grapheme.len()
    }
}

#[cfg(all(test, feature = "svg-parse"))]
mod tests {
    use super::*;

    #[test]
    fn raw_soft_hyphens_bound_the_pending_shaping_cluster() {
        let style = TextStyle {
            font_size: 10.0,
            ..TextStyle::default()
        };
        let (model, size) = DeterministicMode::Compatibility.width_model(&style);
        let mut width = BuiltinInlineRawLineWidth::new(model, size);
        for index in 1..=8192 {
            width.push_text("👩\u{200d}\u{00ad}");
            assert_eq!(width.width_px(), index as f64 * 10.0);
            assert!(width.retained_grapheme_bytes() <= '\u{00ad}'.len_utf8());
            let snapshot = width.clone();
            assert!(snapshot.retained_grapheme_bytes() <= '\u{00ad}'.len_utf8());
        }
        width.push_text("👩");
        assert_eq!(width.width_px(), 81930.0);
    }
}

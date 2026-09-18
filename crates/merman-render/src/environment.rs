//! Operation-owned render services and deterministic policy.

use crate::diagram_theme::{
    DiagramTheme, FontCatalog, FontCatalogFingerprint, FontSourcePolicy, HostMeasurementFallback,
    HostMeasurementFallbackPolicy, ResolvedThemeAdmission, ThemeAdmissionError,
    ThemeAdmissionPolicy, ThemeHostAdmissionReport, ThemePortabilityRequirement,
    ThemeRecipeFingerprint, ThemeRecipeReport, ThemeResourceLimitExceeded, ThemeResourcePolicy,
    TrustedThemeLane, TrustedThemeLanes,
};
use crate::math::{ConfiguredMathBackend, MathRenderer};
use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
use crate::svg::IconRegistry;
#[cfg(feature = "embedded-fonts")]
use crate::text::NativeTextLayoutBackend;
use crate::text::{
    DeterministicTextMeasurer, PrepareCatalogRequest, PreparedTextLayout,
    PreparedTextLayoutBuilder, PreparedTextLayoutReport, TextLayoutBackend, TextLayoutError,
    TextLayoutFailure, TextMeasurer, TextMetrics, TextStyle, WrapMode, append_text_width_em,
    estimate_text_width_em, is_html_collapsible_ascii_whitespace,
};
use crate::{RenderCapability, RenderCapabilityPolicy};
use merman_core::__private::ThemeCompatibilityRecipe;
use merman_core::runtime::{OperationContext, OperationTiming, RuntimePolicy, RuntimePolicyError};
use merman_core::time::LocalTimeZoneProvenance;
use merman_core::{OperationControl, OperationPhase};
#[cfg(test)]
use std::cell::Cell;
use std::cell::RefCell;
use std::fmt;
use std::num::NonZeroU64;
#[cfg(test)]
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RenderEnvironmentError {
    #[error(transparent)]
    Cancelled(#[from] merman_core::OperationCancelled),
    #[error(transparent)]
    Runtime(#[from] RuntimePolicyError),
    #[error(transparent)]
    ThemeAdmission(#[from] ThemeAdmissionError),
    #[error(transparent)]
    ThemeResource(#[from] ThemeResourceLimitExceeded),
}

/// A render phase that may select a distinct complete text-measurement profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextMeasurementPhase {
    Layout,
    Wrap,
    SvgBBox,
    ComputedLength,
}

impl TextMeasurementPhase {
    pub const ALL: [Self; 4] = [
        Self::Layout,
        Self::Wrap,
        Self::SvgBBox,
        Self::ComputedLength,
    ];

    const fn index(self) -> usize {
        match self {
            Self::Layout => 0,
            Self::Wrap => 1,
            Self::SvgBBox => 2,
            Self::ComputedLength => 3,
        }
    }
}

/// Stable name for one complete [`TextMeasurer`] profile.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MeasurementProfileId(Arc<str>);

impl MeasurementProfileId {
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidMeasurementProfileIdentity> {
        let value = value.into();
        let value = value.trim();
        if value.is_empty() {
            return Err(InvalidMeasurementProfileIdentity::EmptyProfile);
        }
        Ok(Self(Arc::from(value)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MeasurementProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Observable identity for a measurer and its ordered decorator chain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextMeasurementProfileIdentity {
    profile: MeasurementProfileId,
    version: Arc<str>,
    decorators: Arc<[Arc<str>]>,
}

impl TextMeasurementProfileIdentity {
    pub fn new(
        profile: MeasurementProfileId,
        version: impl Into<String>,
    ) -> Result<Self, InvalidMeasurementProfileIdentity> {
        let version = version.into();
        let version = version.trim();
        if version.is_empty() {
            return Err(InvalidMeasurementProfileIdentity::EmptyVersion);
        }
        Ok(Self {
            profile,
            version: Arc::from(version),
            decorators: Arc::from([]),
        })
    }

    pub fn with_decorators<I, S>(
        mut self,
        decorators: I,
    ) -> Result<Self, InvalidMeasurementProfileIdentity>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut validated = Vec::new();
        for decorator in decorators {
            let decorator = decorator.into();
            let decorator = decorator.trim();
            if decorator.is_empty() {
                return Err(InvalidMeasurementProfileIdentity::EmptyDecorator);
            }
            validated.push(Arc::from(decorator));
        }
        self.decorators = validated.into();
        Ok(self)
    }

    pub fn profile(&self) -> &MeasurementProfileId {
        &self.profile
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn decorators(&self) -> &[Arc<str>] {
        &self.decorators
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InvalidMeasurementProfileIdentity {
    #[error("text measurement profile name cannot be empty")]
    EmptyProfile,
    #[error("text measurement profile version cannot be empty")]
    EmptyVersion,
    #[error("text measurement decorator identity cannot be empty")]
    EmptyDecorator,
}

/// A named, complete measurer profile. Specialized trait methods remain part of the profile.
#[derive(Clone)]
pub struct TextMeasurementProfile {
    identity: TextMeasurementProfileIdentity,
    backend: Arc<dyn TextMeasurer + Send + Sync>,
    builtin: Option<BuiltinTextMeasurementProfile>,
}

impl TextMeasurementProfile {
    pub fn new(
        identity: TextMeasurementProfileIdentity,
        backend: Arc<dyn TextMeasurer + Send + Sync>,
    ) -> Self {
        Self {
            identity,
            backend,
            builtin: None,
        }
    }

    pub fn identity(&self) -> &TextMeasurementProfileIdentity {
        &self.identity
    }

    fn new_builtin(
        identity: TextMeasurementProfileIdentity,
        backend: Arc<dyn TextMeasurer + Send + Sync>,
        builtin: BuiltinTextMeasurementProfile,
    ) -> Self {
        Self {
            identity,
            backend,
            builtin: Some(builtin),
        }
    }
}

impl fmt::Debug for TextMeasurementProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextMeasurementProfile")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BuiltinTextMeasurementProfile {
    Deterministic,
}

/// Crate-private proof that one operation resolves to a concrete built-in profile route.
///
/// The public `TextMeasurer` extension surface cannot construct or name this value. Sequence may
/// carry it between two stages of the same render operation, but a custom or host-backed measurer
/// cannot replay built-in authority to validate cached measurements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BuiltinTextMeasurementOperationCarrier {
    profile: BuiltinTextMeasurementProfile,
    phase: TextMeasurementPhase,
    operation: TextMeasurementOperation,
}

impl BuiltinTextMeasurementOperationCarrier {
    pub(crate) const fn into_inline_html(self) -> Option<InlineHtmlMeasurementCarrier> {
        match (self.phase, self.operation) {
            (TextMeasurementPhase::Wrap, TextMeasurementOperation::WrappedWithRawWidth) => {
                Some(InlineHtmlMeasurementCarrier::builtin(self.profile))
            }
            _ => None,
        }
    }

    pub(crate) fn into_svg_computed_length(
        self,
        style: &TextStyle,
    ) -> Option<BuiltinSvgComputedLength> {
        match (self.phase, self.operation) {
            (TextMeasurementPhase::ComputedLength, TextMeasurementOperation::ComputedLength) => {
                Some(BuiltinSvgComputedLength::new(style))
            }
            _ => None,
        }
    }
}

/// Private authority for one complete rich HTML measurement operation.
///
/// Only [`RoutedTextMeasurer`] can attach a built-in profile after resolving the operation's
/// owning phase. Arbitrary `TextMeasurer` implementations receive [`Self::opaque`], so custom and
/// host routes cannot copy or replay built-in authority through the public trait surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InlineHtmlMeasurementCarrier {
    builtin: Option<BuiltinTextMeasurementProfile>,
}

impl InlineHtmlMeasurementCarrier {
    pub(crate) const fn opaque() -> Self {
        Self { builtin: None }
    }

    const fn builtin(profile: BuiltinTextMeasurementProfile) -> Self {
        Self {
            builtin: Some(profile),
        }
    }

    pub(crate) const fn is_builtin(self) -> bool {
        self.builtin.is_some()
    }

    pub(crate) fn begin_inline_html_width(
        self,
        style: &TextStyle,
    ) -> Option<BuiltinInlineHtmlWidth> {
        self.builtin.map(|_| BuiltinInlineHtmlWidth::new(style))
    }
}

#[derive(Debug, Clone)]
struct BuiltinInlineRawLineWidth {
    font_size: f64,
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

/// Streaming `getComputedTextLength()` state for a qualified built-in SVG text route.
///
/// Flowchart's createText wrapper probes every growing word prefix. Retaining completed grapheme
/// width plus only the extendable final grapheme avoids rescanning the complete prefix while
/// preserving sequence-aware Unicode width across arbitrary chunk boundaries. Host-backed and
/// opaque custom measurers cannot construct this state, so their observable callback sequence
/// remains unchanged.
#[derive(Debug, Clone)]
pub(crate) struct BuiltinSvgComputedLength {
    line: BuiltinInlineRawLineWidth,
}

impl BuiltinSvgComputedLength {
    fn new(style: &TextStyle) -> Self {
        Self {
            line: BuiltinInlineRawLineWidth::new(style),
        }
    }

    pub(crate) fn deterministic(style: &TextStyle) -> Self {
        Self::new(style)
    }

    pub(crate) fn push_text(&mut self, text: &str) {
        self.line.push_text(text);
    }

    pub(crate) fn width_px(&self) -> f64 {
        let width = self.line.width_px();
        if width.is_finite() && width >= 0.0 {
            width
        } else {
            0.0
        }
    }

    pub(crate) fn reset(&mut self) {
        self.line.reset();
    }
}

impl BuiltinInlineRawLineWidth {
    fn new(style: &TextStyle) -> Self {
        Self {
            font_size: style.font_size.max(1.0),
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

    fn refresh_width(state: &mut BuiltinInlineRawLineWidthState) {
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
            append_text_width_em(
                &mut state.committed_em,
                &state.pending_grapheme[..last_grapheme_start],
            );
            state.pending_grapheme.drain(..last_grapheme_start);
        }
        state.pending_em = estimate_text_width_em(&state.pending_grapheme);
        state.pending_width_dirty = false;
    }

    fn push_char(&mut self, ch: char) {
        let mut encoded = [0_u8; 4];
        self.push_text(ch.encode_utf8(&mut encoded));
    }

    fn width_px(&self) -> f64 {
        let mut state = self.state.borrow_mut();
        Self::refresh_width(&mut state);
        (state.committed_em + state.pending_em) * self.font_size
    }

    fn compact_pending_graphemes(&mut self) {
        Self::refresh_width(self.state.get_mut());
    }

    fn reset(&mut self) {
        let state = self.state.get_mut();
        state.committed_em = 0.0;
        state.pending_grapheme.clear();
        state.pending_width_dirty = false;
        state.pending_em = 0.0;
    }

    #[cfg(test)]
    fn grapheme_input_byte_charge(&self) -> usize {
        self.state.borrow().grapheme_input_byte_charge.get()
    }

    #[cfg(test)]
    fn retained_grapheme_bytes(&self) -> usize {
        self.state.borrow().pending_grapheme.len()
    }
}

#[derive(Debug, Clone)]
struct BuiltinNormalizedTextWidth {
    line: BuiltinInlineRawLineWidth,
    max_width_px: f64,
    pending_blank_width_px: f64,
    line_index: usize,
    line_has_non_whitespace: bool,
}

impl BuiltinNormalizedTextWidth {
    fn new(line: BuiltinInlineRawLineWidth) -> Self {
        Self {
            line,
            max_width_px: 0.0,
            pending_blank_width_px: 0.0,
            line_index: 0,
            line_has_non_whitespace: false,
        }
    }

    fn push_char(&mut self, ch: char) {
        if ch == '\n' {
            self.finish_line();
            return;
        }

        if !is_html_collapsible_ascii_whitespace(ch) && !self.line_has_non_whitespace {
            // Completed whitespace-only lines cease to be trailing as soon as a later visible
            // scalar arrives. This mirrors `normalized_text_lines` trimming only the final blank
            // suffix while retaining interior whitespace-only lines.
            self.max_width_px = self.max_width_px.max(self.pending_blank_width_px);
            self.pending_blank_width_px = 0.0;
            self.line_has_non_whitespace = true;
        }
        self.line.push_char(ch);
    }

    fn push_text(&mut self, text: &str) {
        let mut start = 0usize;
        for (index, ch) in text.char_indices() {
            if ch != '\n' {
                continue;
            }
            self.push_line_segment(&text[start..index]);
            self.finish_line();
            start = index + ch.len_utf8();
        }
        self.push_line_segment(&text[start..]);
    }

    fn push_line_segment(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if !self.line_has_non_whitespace
            && text
                .chars()
                .any(|ch| !is_html_collapsible_ascii_whitespace(ch))
        {
            self.max_width_px = self.max_width_px.max(self.pending_blank_width_px);
            self.pending_blank_width_px = 0.0;
            self.line_has_non_whitespace = true;
        }
        self.line.push_text(text);
    }

    fn finish_line(&mut self) {
        let width = self.line.width_px();
        if self.line_index == 0 || self.line_has_non_whitespace {
            self.max_width_px = self.max_width_px.max(width);
        } else {
            self.pending_blank_width_px = self.pending_blank_width_px.max(width);
        }
        self.line_index = self.line_index.saturating_add(1);
        self.line_has_non_whitespace = false;
        self.line.reset();
    }

    fn finished_width_px(&self) -> f64 {
        if self.line_index == 0 || self.line_has_non_whitespace {
            self.max_width_px.max(self.line.width_px())
        } else {
            // `DeterministicTextMeasurer::normalized_text_lines` removes the trailing blank
            // suffix, but always retains the first logical line (already committed above).
            self.max_width_px
        }
    }

    #[cfg(test)]
    fn grapheme_input_byte_charge(&self) -> usize {
        self.line.grapheme_input_byte_charge()
    }

    fn compact_pending_graphemes(&mut self) {
        self.line.compact_pending_graphemes();
    }

    #[cfg(test)]
    fn retained_grapheme_bytes(&self) -> usize {
        self.line.retained_grapheme_bytes()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InlineHtmlBreakState {
    AfterLt,
    AfterB,
    AfterBr,
    AfterSlash,
}

#[derive(Debug, Clone)]
struct PendingInlineHtmlBreak {
    state: InlineHtmlBreakState,
    literal: BuiltinNormalizedTextWidth,
}

impl PendingInlineHtmlBreak {
    fn push_candidate_char(&mut self, ch: char) {
        self.literal.push_char(ch);
        // Candidate characters are ASCII and the only multi-scalar ASCII grapheme is CRLF.
        // Compact after every continuation so an incomplete `<br ...` cannot retain an
        // unbounded whitespace buffer.
        self.literal.compact_pending_graphemes();
    }
}

/// Exact streaming width state for a qualified built-in HTML measurement route.
///
/// Mermaid's pinned `createText.ts:addHtmlSpan` decodes HTML into a real span, so a valid `<br>`
/// contributes a DOM line break before `getBoundingClientRect()`. This state follows the selected
/// built-in backend's grapheme accumulation order and its existing `normalized_text_lines`
/// behavior, without retaining or rescanning the potentially unbounded whitespace in an
/// incomplete tag. It intentionally recognizes only ASCII space, tab, CR, and LF inside `<br>`;
/// the wider ECMAScript `\s` set and browser text shaping remain bounded browser residuals.
#[derive(Debug, Clone)]
pub(crate) struct BuiltinInlineHtmlWidth {
    normalized: BuiltinNormalizedTextWidth,
    pending_break: Option<PendingInlineHtmlBreak>,
    #[cfg(test)]
    speculative_clone_bytes: Rc<Cell<usize>>,
}

impl BuiltinInlineHtmlWidth {
    fn new(style: &TextStyle) -> Self {
        Self {
            normalized: BuiltinNormalizedTextWidth::new(BuiltinInlineRawLineWidth::new(style)),
            pending_break: None,
            #[cfg(test)]
            speculative_clone_bytes: Rc::new(Cell::new(0)),
        }
    }

    pub(crate) fn push_text(&mut self, text: &str) {
        if self.pending_break.is_none() && !text.contains('<') {
            self.normalized.push_text(text);
            return;
        }
        for ch in text.chars() {
            self.push_char(ch);
        }
    }

    fn push_char(&mut self, ch: char) {
        let mut current = Some(ch);
        while let Some(ch) = current.take() {
            let Some(mut pending) = self.pending_break.take() else {
                if ch == '<' {
                    // Width compaction retains only the final extendable grapheme. Cloning this
                    // bounded frontier keeps speculative `<br>` parsing linear without assuming
                    // that ASCII `<` always starts a new Unicode grapheme.
                    self.normalized.compact_pending_graphemes();
                    #[cfg(test)]
                    {
                        let next_clone_bytes = self
                            .speculative_clone_bytes
                            .get()
                            .saturating_add(self.normalized.retained_grapheme_bytes());
                        self.speculative_clone_bytes.set(next_clone_bytes);
                    }
                    let mut literal = self.normalized.clone();
                    literal.push_char(ch);
                    self.pending_break = Some(PendingInlineHtmlBreak {
                        state: InlineHtmlBreakState::AfterLt,
                        literal,
                    });
                } else {
                    self.normalized.push_char(ch);
                }
                continue;
            };

            match pending.state {
                InlineHtmlBreakState::AfterLt if matches!(ch, 'b' | 'B') => {
                    pending.push_candidate_char(ch);
                    pending.state = InlineHtmlBreakState::AfterB;
                    self.pending_break = Some(pending);
                }
                InlineHtmlBreakState::AfterB if matches!(ch, 'r' | 'R') => {
                    pending.push_candidate_char(ch);
                    pending.state = InlineHtmlBreakState::AfterBr;
                    self.pending_break = Some(pending);
                }
                InlineHtmlBreakState::AfterBr if matches!(ch, ' ' | '\t' | '\r' | '\n') => {
                    pending.push_candidate_char(ch);
                    self.pending_break = Some(pending);
                }
                InlineHtmlBreakState::AfterBr if ch == '/' => {
                    pending.push_candidate_char(ch);
                    pending.state = InlineHtmlBreakState::AfterSlash;
                    self.pending_break = Some(pending);
                }
                InlineHtmlBreakState::AfterBr | InlineHtmlBreakState::AfterSlash if ch == '>' => {
                    self.normalized.push_char('\n');
                }
                _ => {
                    self.normalized = pending.literal;
                    current = Some(ch);
                }
            }
        }
    }

    pub(crate) fn width_px(&self) -> f64 {
        self.pending_break.as_ref().map_or_else(
            || self.normalized.finished_width_px(),
            |pending| pending.literal.finished_width_px(),
        )
    }

    #[cfg(test)]
    fn grapheme_input_byte_charge(&self) -> usize {
        self.pending_break.as_ref().map_or_else(
            || self.normalized.grapheme_input_byte_charge(),
            |pending| pending.literal.grapheme_input_byte_charge(),
        )
    }

    #[cfg(test)]
    fn speculative_clone_bytes(&self) -> usize {
        self.speculative_clone_bytes.get()
    }

    #[cfg(test)]
    fn retained_grapheme_bytes(&self) -> usize {
        self.pending_break.as_ref().map_or_else(
            || self.normalized.retained_grapheme_bytes(),
            |pending| pending.literal.retained_grapheme_bytes(),
        )
    }
}

fn deterministic_profile() -> TextMeasurementProfile {
    let profile = MeasurementProfileId::new("merman.deterministic-text")
        .expect("static deterministic profile id is valid");
    let identity = TextMeasurementProfileIdentity::new(
        profile,
        concat!("merman-render@", env!("CARGO_PKG_VERSION")),
    )
    .expect("static deterministic profile version is valid");
    TextMeasurementProfile::new_builtin(
        identity,
        Arc::new(DeterministicTextMeasurer::default()),
        BuiltinTextMeasurementProfile::Deterministic,
    )
}

/// Why a configured host attempt used its named fallback profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostFallbackReason {
    Missing,
    Invalid,
    Error,
}

/// The exact [`TextMeasurer`] operation performed through a phase facade and its required host
/// result shape. Both types are generated from the independently versioned host
/// text-measurement protocol shared by every binding.
pub use crate::generated::text_measurement_abi::{
    TEXT_MEASUREMENT_PROTOCOL_VERSION, TextMeasurementOperation, TextMeasurementResultKind,
};

/// The concrete backend kind that produced one result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextMeasurementSource {
    Profile,
    Host,
}

/// Actual provenance recorded after one measurement completes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextMeasurementProvenance {
    pub phase: TextMeasurementPhase,
    pub operation: TextMeasurementOperation,
    pub source: TextMeasurementSource,
    pub identity: TextMeasurementProfileIdentity,
    pub fallback_reason: Option<HostFallbackReason>,
}

/// One distinct provenance key and its total call count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextMeasurementSummary {
    provenance: TextMeasurementProvenance,
    count: u64,
}

impl TextMeasurementSummary {
    pub fn provenance(&self) -> &TextMeasurementProvenance {
        &self.provenance
    }

    pub const fn count(&self) -> u64 {
        self.count
    }
}

/// Bounded snapshot of measurement provenance aggregated by distinct route outcome.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextMeasurementReport {
    entries: Vec<TextMeasurementSummary>,
}

impl TextMeasurementReport {
    pub fn entries(&self) -> &[TextMeasurementSummary] {
        &self.entries
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextMeasurementRouteOutcome {
    Profile,
    Host,
    Fallback(HostFallbackReason),
}

impl TextMeasurementRouteOutcome {
    const ALL: [Self; 5] = [
        Self::Profile,
        Self::Host,
        Self::Fallback(HostFallbackReason::Missing),
        Self::Fallback(HostFallbackReason::Invalid),
        Self::Fallback(HostFallbackReason::Error),
    ];

    const fn index(self) -> usize {
        match self {
            Self::Profile => 0,
            Self::Host => 1,
            Self::Fallback(HostFallbackReason::Missing) => 2,
            Self::Fallback(HostFallbackReason::Invalid) => 3,
            Self::Fallback(HostFallbackReason::Error) => 4,
        }
    }
}

#[derive(Debug)]
struct TextMeasurementRecorder {
    counts: [AtomicU64;
        TextMeasurementPhase::ALL.len()
            * TextMeasurementOperation::ALL.len()
            * TextMeasurementRouteOutcome::ALL.len()],
}

impl Default for TextMeasurementRecorder {
    fn default() -> Self {
        Self {
            counts: std::array::from_fn(|_| AtomicU64::new(0)),
        }
    }
}

impl TextMeasurementRecorder {
    const fn slot(
        phase: TextMeasurementPhase,
        operation: TextMeasurementOperation,
        outcome: TextMeasurementRouteOutcome,
    ) -> usize {
        (phase.index() * TextMeasurementOperation::ALL.len() + operation.index())
            * TextMeasurementRouteOutcome::ALL.len()
            + outcome.index()
    }

    fn record(
        &self,
        phase: TextMeasurementPhase,
        operation: TextMeasurementOperation,
        outcome: TextMeasurementRouteOutcome,
    ) {
        let counter = &self.counts[Self::slot(phase, operation, outcome)];
        let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
            Some(count.saturating_add(1))
        });
    }

    fn report(&self, policy: &TextMeasurementPolicy) -> TextMeasurementReport {
        let mut entries = Vec::new();
        for phase in TextMeasurementPhase::ALL {
            for operation in TextMeasurementOperation::ALL {
                for outcome in TextMeasurementRouteOutcome::ALL {
                    let count =
                        self.counts[Self::slot(phase, operation, outcome)].load(Ordering::Relaxed);
                    if count == 0 {
                        continue;
                    }

                    let provenance = match (&policy.routes[phase.index()], outcome) {
                        (
                            TextMeasurementRouteConfig::Profile(profile),
                            TextMeasurementRouteOutcome::Profile,
                        ) => TextMeasurementProvenance {
                            phase,
                            operation,
                            source: TextMeasurementSource::Profile,
                            identity: profile.identity.clone(),
                            fallback_reason: None,
                        },
                        (
                            TextMeasurementRouteConfig::Host { identity, .. },
                            TextMeasurementRouteOutcome::Host,
                        ) => TextMeasurementProvenance {
                            phase,
                            operation,
                            source: TextMeasurementSource::Host,
                            identity: identity.clone(),
                            fallback_reason: None,
                        },
                        (
                            TextMeasurementRouteConfig::Host { fallback, .. },
                            TextMeasurementRouteOutcome::Fallback(reason),
                        ) => TextMeasurementProvenance {
                            phase,
                            operation,
                            source: TextMeasurementSource::Profile,
                            identity: fallback.identity.clone(),
                            fallback_reason: Some(reason),
                        },
                        _ => unreachable!(
                            "recorded text measurement outcome does not match the configured route"
                        ),
                    };
                    entries.push(TextMeasurementSummary { provenance, count });
                }
            }
        }
        TextMeasurementReport { entries }
    }
}

/// A host callback failure or invalid result converted to explicit fallback provenance.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct HostTextMeasurementError {
    message: Arc<str>,
    fallback_reason: HostFallbackReason,
}

impl HostTextMeasurementError {
    /// Creates an error reported by the host callback or its transport.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: Arc::from(message.into()),
            fallback_reason: HostFallbackReason::Error,
        }
    }

    /// Creates an error for a callback value that violates the measurement contract.
    #[doc(hidden)]
    pub fn invalid_value(message: impl Into<String>) -> Self {
        Self {
            message: Arc::from(message.into()),
            fallback_reason: HostFallbackReason::Invalid,
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    fn fallback_reason(&self) -> HostFallbackReason {
        self.fallback_reason
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HostTextMeasurementRequest<'a> {
    pub operation: TextMeasurementOperation,
    pub phase: TextMeasurementPhase,
    pub text: &'a str,
    pub style: &'a TextStyle,
    pub max_width: Option<f64>,
    pub wrap_mode: WrapMode,
}

#[derive(Debug, Clone, Copy)]
pub enum HostTextMeasurement {
    Metrics(TextMetrics),
    Length(f64),
    HorizontalExtents {
        left: f64,
        right: f64,
    },
    WrappedWithRawWidth {
        metrics: TextMetrics,
        raw_width: Option<f64>,
    },
}

pub type HostMeasurementResult = Result<Option<HostTextMeasurement>, HostTextMeasurementError>;

/// Fallible, operation-aware host counterpart of [`TextMeasurer`].
///
/// Returning `Ok(None)` declines exactly the requested operation. Returning a result variant that
/// does not match `request.operation`, or an invalid value, uses the configured fallback and is
/// recorded as [`HostFallbackReason::Invalid`].
pub trait HostTextMeasurer: Send + Sync {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult;
}

#[derive(Clone)]
enum TextMeasurementRouteConfig {
    Profile(TextMeasurementProfile),
    Host {
        identity: TextMeasurementProfileIdentity,
        backend: Arc<dyn HostTextMeasurer>,
        fallback: TextMeasurementProfile,
    },
}

/// Observable configured route for one phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextMeasurementRoute {
    pub phase: TextMeasurementPhase,
    pub primary_source: TextMeasurementSource,
    pub primary: TextMeasurementProfileIdentity,
    pub fallback: Option<TextMeasurementProfileIdentity>,
}

/// Immutable routing policy for all text-measurement phases in one environment.
#[derive(Clone)]
pub struct TextMeasurementPolicy {
    routes: [TextMeasurementRouteConfig; 4],
}

impl fmt::Debug for TextMeasurementPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let routes = TextMeasurementPhase::ALL.map(|phase| self.route(phase));
        f.debug_struct("TextMeasurementPolicy")
            .field("routes", &routes)
            .finish()
    }
}

impl TextMeasurementPolicy {
    pub fn deterministic() -> Self {
        Self::uniform(deterministic_profile())
    }

    pub fn uniform(profile: TextMeasurementProfile) -> Self {
        Self {
            routes: std::array::from_fn(|_| TextMeasurementRouteConfig::Profile(profile.clone())),
        }
    }

    pub fn with_profile_for_phase(
        mut self,
        phase: TextMeasurementPhase,
        profile: TextMeasurementProfile,
    ) -> Self {
        self.routes[phase.index()] = TextMeasurementRouteConfig::Profile(profile);
        self
    }

    pub fn host_display(
        identity: TextMeasurementProfileIdentity,
        host: Arc<dyn HostTextMeasurer>,
        host_phases: impl IntoIterator<Item = TextMeasurementPhase>,
    ) -> Self {
        Self::host_display_with_fallback(identity, host, host_phases, deterministic_profile())
    }

    pub fn host_display_with_fallback(
        identity: TextMeasurementProfileIdentity,
        host: Arc<dyn HostTextMeasurer>,
        host_phases: impl IntoIterator<Item = TextMeasurementPhase>,
        fallback: TextMeasurementProfile,
    ) -> Self {
        let mut policy = Self::uniform(fallback.clone());
        for phase in host_phases {
            policy.routes[phase.index()] = TextMeasurementRouteConfig::Host {
                identity: identity.clone(),
                backend: Arc::clone(&host),
                fallback: fallback.clone(),
            };
        }
        policy
    }

    pub fn route(&self, phase: TextMeasurementPhase) -> TextMeasurementRoute {
        match &self.routes[phase.index()] {
            TextMeasurementRouteConfig::Profile(profile) => TextMeasurementRoute {
                phase,
                primary_source: TextMeasurementSource::Profile,
                primary: profile.identity.clone(),
                fallback: None,
            },
            TextMeasurementRouteConfig::Host {
                identity, fallback, ..
            } => TextMeasurementRoute {
                phase,
                primary_source: TextMeasurementSource::Host,
                primary: identity.clone(),
                fallback: Some(fallback.identity.clone()),
            },
        }
    }

    pub fn routes(&self) -> [TextMeasurementRoute; 4] {
        TextMeasurementPhase::ALL.map(|phase| self.route(phase))
    }
}

impl Default for TextMeasurementPolicy {
    fn default() -> Self {
        Self::deterministic()
    }
}

/// Session-aware facade that routes specialized operations to their named phases.
pub struct RoutedTextMeasurer<'a> {
    default_phase: TextMeasurementPhase,
    policy: &'a TextMeasurementPolicy,
    recorder: &'a TextMeasurementRecorder,
    work_meter: &'a OperationWorkMeter,
    controlled_operation_phase: Option<OperationPhase>,
}

trait CancelledTextMeasurement: Sized {
    fn cancelled() -> Self;
}

impl CancelledTextMeasurement for f64 {
    fn cancelled() -> Self {
        0.0
    }
}

impl CancelledTextMeasurement for (f64, f64) {
    fn cancelled() -> Self {
        (0.0, 0.0)
    }
}

impl CancelledTextMeasurement for TextMetrics {
    fn cancelled() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            line_count: 1,
        }
    }
}

impl CancelledTextMeasurement for (TextMetrics, Option<f64>) {
    fn cancelled() -> Self {
        (TextMetrics::cancelled(), None)
    }
}

impl RoutedTextMeasurer<'_> {
    fn controlled_cancellation_value<T: CancelledTextMeasurement>(&self) -> Option<T> {
        let phase = self.controlled_operation_phase?;
        self.work_meter
            .checkpoint(phase)
            .is_err()
            .then(T::cancelled)
    }

    fn phase_for(&self, operation: TextMeasurementOperation) -> TextMeasurementPhase {
        match operation {
            TextMeasurementOperation::ComputedLength => TextMeasurementPhase::ComputedLength,
            TextMeasurementOperation::BBoxX
            | TextMeasurementOperation::BBoxXWithAsciiOverhang
            | TextMeasurementOperation::TitleBBoxX
            | TextMeasurementOperation::SimpleBBoxWidth
            | TextMeasurementOperation::RawBBoxWidth
            | TextMeasurementOperation::RawBBoxHeight
            | TextMeasurementOperation::BoundingClientRectWidth
            | TextMeasurementOperation::TspanBBoxWidth
            | TextMeasurementOperation::TspanBBoxHeight
            | TextMeasurementOperation::CreateTextBBoxYOffset
            | TextMeasurementOperation::CreateTextMiddleBBoxYOffset
            | TextMeasurementOperation::MermaidCalculateTextDimensions
            | TextMeasurementOperation::SimpleBBoxHeight => TextMeasurementPhase::SvgBBox,
            TextMeasurementOperation::CanvasMeasureTextWidth => TextMeasurementPhase::Layout,
            TextMeasurementOperation::WrapProbeBBoxWidth => TextMeasurementPhase::Wrap,
            TextMeasurementOperation::Wrapped | TextMeasurementOperation::WrappedWithRawWidth => {
                TextMeasurementPhase::Wrap
            }
            TextMeasurementOperation::Measure => self.default_phase,
        }
    }

    pub(crate) fn builtin_operation_carrier(
        &self,
        operation: TextMeasurementOperation,
    ) -> Option<BuiltinTextMeasurementOperationCarrier> {
        let phase = self.phase_for(operation);
        match &self.policy.routes[phase.index()] {
            TextMeasurementRouteConfig::Profile(profile) => {
                profile
                    .builtin
                    .map(|profile| BuiltinTextMeasurementOperationCarrier {
                        profile,
                        phase,
                        operation,
                    })
            }
            // A host-routed operation remains observable even when its fallback is built-in. It
            // must stay opaque so callback order, failure position, and provenance cannot be
            // predicted away.
            TextMeasurementRouteConfig::Host { .. } => None,
        }
    }

    fn resolve<T: CancelledTextMeasurement>(
        &self,
        request: HostTextMeasurementRequest<'_>,
        decode_host: impl FnOnce(HostTextMeasurement) -> Option<T>,
        profile_call: impl FnOnce(&(dyn TextMeasurer + Send + Sync)) -> T,
    ) -> T {
        let phase = request.phase;
        let operation = request.operation;
        if let Some(cancelled) = self.controlled_cancellation_value() {
            return cancelled;
        }
        match &self.policy.routes[phase.index()] {
            TextMeasurementRouteConfig::Profile(profile) => {
                let value = profile_call(profile.backend.as_ref());
                self.recorder
                    .record(phase, operation, TextMeasurementRouteOutcome::Profile);
                if let Some(cancelled) = self.controlled_cancellation_value() {
                    return cancelled;
                }
                value
            }
            TextMeasurementRouteConfig::Host {
                backend, fallback, ..
            } => {
                let attempt = backend.measure(request);
                if let Some(cancelled) = self.controlled_cancellation_value() {
                    return cancelled;
                }
                let decoded = match &attempt {
                    Ok(Some(value)) if validate_host_text_measurement(&request, value).is_ok() => {
                        decode_host(*value)
                    }
                    Ok(None) | Err(_) => None,
                    Ok(Some(_)) => None,
                };
                if let Some(value) = decoded {
                    self.recorder
                        .record(phase, operation, TextMeasurementRouteOutcome::Host);
                    return value;
                }

                let reason = match attempt {
                    Ok(Some(_)) => HostFallbackReason::Invalid,
                    Ok(None) => HostFallbackReason::Missing,
                    Err(error) => error.fallback_reason(),
                };
                // `TextMeasurer` is intentionally infallible. A controlled route therefore
                // observes the canonical operation before entering another opaque backend and
                // returns a neutral value only to unwind toward the caller's fallible boundary.
                if let Some(cancelled) = self.controlled_cancellation_value() {
                    return cancelled;
                }
                let value = profile_call(fallback.backend.as_ref());
                self.recorder.record(
                    phase,
                    operation,
                    TextMeasurementRouteOutcome::Fallback(reason),
                );
                if let Some(cancelled) = self.controlled_cancellation_value() {
                    return cancelled;
                }
                value
            }
        }
    }

    fn request<'a>(
        &self,
        operation: TextMeasurementOperation,
        text: &'a str,
        style: &'a TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> HostTextMeasurementRequest<'a> {
        HostTextMeasurementRequest {
            operation,
            phase: self.phase_for(operation),
            text,
            style,
            max_width,
            wrap_mode,
        }
    }
}

impl TextMeasurer for RoutedTextMeasurer<'_> {
    fn cancellation_requested(&self) -> bool {
        self.controlled_operation_phase
            .is_some_and(|phase| self.work_meter.checkpoint(phase).is_err())
    }

    #[allow(private_interfaces)]
    fn builtin_operation_carrier(
        &self,
        operation: TextMeasurementOperation,
    ) -> Option<BuiltinTextMeasurementOperationCarrier> {
        RoutedTextMeasurer::builtin_operation_carrier(self, operation)
    }

    #[allow(private_interfaces)]
    fn begin_svg_text_computed_length(
        &self,
        style: &TextStyle,
    ) -> Option<BuiltinSvgComputedLength> {
        self.builtin_operation_carrier(TextMeasurementOperation::ComputedLength)
            .and_then(|carrier| carrier.into_svg_computed_length(style))
    }

    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.resolve(
            self.request(
                TextMeasurementOperation::Measure,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            decode_host_metrics,
            |profile| profile.measure(text, style),
        )
    }

    fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            self.request(
                TextMeasurementOperation::ComputedLength,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            decode_host_length,
            |profile| profile.measure_svg_text_computed_length_px(text, style),
        )
    }

    fn measure_svg_text_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
        self.resolve(
            self.request(
                TextMeasurementOperation::BBoxX,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            decode_host_extents,
            |profile| profile.measure_svg_text_bbox_x(text, style),
        )
    }

    fn measure_svg_text_bbox_x_with_ascii_overhang(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> (f64, f64) {
        self.resolve(
            self.request(
                TextMeasurementOperation::BBoxXWithAsciiOverhang,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            decode_host_extents,
            |profile| profile.measure_svg_text_bbox_x_with_ascii_overhang(text, style),
        )
    }

    fn measure_svg_title_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
        self.resolve(
            self.request(
                TextMeasurementOperation::TitleBBoxX,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            decode_host_extents,
            |profile| profile.measure_svg_title_bbox_x(text, style),
        )
    }

    fn measure_svg_simple_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            self.request(
                TextMeasurementOperation::SimpleBBoxWidth,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            decode_host_length,
            |profile| profile.measure_svg_simple_text_bbox_width_px(text, style),
        )
    }

    fn measure_svg_raw_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            self.request(
                TextMeasurementOperation::RawBBoxWidth,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            decode_host_length,
            |profile| profile.measure_svg_raw_text_bbox_width_px(text, style),
        )
    }

    fn measure_svg_raw_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            self.request(
                TextMeasurementOperation::RawBBoxHeight,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            decode_host_length,
            |profile| profile.measure_svg_raw_text_bbox_height_px(text, style),
        )
    }

    fn measure_svg_text_bounding_client_rect_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            self.request(
                TextMeasurementOperation::BoundingClientRectWidth,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            decode_host_length,
            |profile| profile.measure_svg_text_bounding_client_rect_width_px(text, style),
        )
    }

    fn measure_svg_tspan_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            self.request(
                TextMeasurementOperation::TspanBBoxWidth,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            decode_host_length,
            |profile| profile.measure_svg_tspan_text_bbox_width_px(text, style),
        )
    }

    fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            self.request(
                TextMeasurementOperation::TspanBBoxHeight,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            decode_host_length,
            |profile| profile.measure_svg_tspan_text_bbox_height_px(text, style),
        )
    }

    fn measure_svg_create_text_bbox_y_offset_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            self.request(
                TextMeasurementOperation::CreateTextBBoxYOffset,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            decode_host_length,
            |profile| profile.measure_svg_create_text_bbox_y_offset_px(text, style),
        )
    }

    fn measure_svg_create_text_middle_bbox_y_offset_px(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> f64 {
        self.resolve(
            self.request(
                TextMeasurementOperation::CreateTextMiddleBBoxYOffset,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            decode_host_length,
            |profile| profile.measure_svg_create_text_middle_bbox_y_offset_px(text, style),
        )
    }

    fn measure_svg_simple_text_bbox_width_for_wrap_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            self.request(
                TextMeasurementOperation::WrapProbeBBoxWidth,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            decode_host_length,
            |profile| profile.measure_svg_simple_text_bbox_width_for_wrap_px(text, style),
        )
    }

    fn measure_mermaid_calculate_text_dimensions(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> TextMetrics {
        self.resolve(
            self.request(
                TextMeasurementOperation::MermaidCalculateTextDimensions,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            decode_host_metrics,
            |profile| profile.measure_mermaid_calculate_text_dimensions(text, style),
        )
    }

    fn measure_canvas_text_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            self.request(
                TextMeasurementOperation::CanvasMeasureTextWidth,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            decode_host_length,
            |profile| profile.measure_canvas_text_width_px(text, style),
        )
    }

    fn measure_svg_simple_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            self.request(
                TextMeasurementOperation::SimpleBBoxHeight,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            decode_host_length,
            |profile| profile.measure_svg_simple_text_bbox_height_px(text, style),
        )
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.resolve(
            self.request(
                TextMeasurementOperation::Wrapped,
                text,
                style,
                max_width,
                wrap_mode,
            ),
            decode_host_metrics,
            |profile| profile.measure_wrapped(text, style, max_width, wrap_mode),
        )
    }

    fn measure_wrapped_with_raw_width(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> (TextMetrics, Option<f64>) {
        self.resolve(
            self.request(
                TextMeasurementOperation::WrappedWithRawWidth,
                text,
                style,
                max_width,
                wrap_mode,
            ),
            decode_host_wrapped_with_raw_width,
            |profile| profile.measure_wrapped_with_raw_width(text, style, max_width, wrap_mode),
        )
    }
}

fn decode_host_metrics(measurement: HostTextMeasurement) -> Option<TextMetrics> {
    match measurement {
        HostTextMeasurement::Metrics(metrics) => Some(metrics),
        _ => None,
    }
}

fn decode_host_length(measurement: HostTextMeasurement) -> Option<f64> {
    match measurement {
        HostTextMeasurement::Length(length) => Some(length),
        _ => None,
    }
}

fn decode_host_extents(measurement: HostTextMeasurement) -> Option<(f64, f64)> {
    match measurement {
        HostTextMeasurement::HorizontalExtents { left, right } => Some((left, right)),
        _ => None,
    }
}

fn decode_host_wrapped_with_raw_width(
    measurement: HostTextMeasurement,
) -> Option<(TextMetrics, Option<f64>)> {
    match measurement {
        HostTextMeasurement::WrappedWithRawWidth { metrics, raw_width } => {
            Some((metrics, raw_width))
        }
        _ => None,
    }
}

/// Checks a host callback value against the complete operation request.
///
/// The validator is the single authority for result shape and numeric bounds across direct
/// renderer hosts and every binding transport.
pub fn validate_host_text_measurement(
    request: &HostTextMeasurementRequest<'_>,
    measurement: &HostTextMeasurement,
) -> Result<(), HostTextMeasurementError> {
    let result_kind = match measurement {
        HostTextMeasurement::Metrics(_) => TextMeasurementResultKind::Metrics,
        HostTextMeasurement::Length(_) => TextMeasurementResultKind::Length,
        HostTextMeasurement::HorizontalExtents { .. } => {
            TextMeasurementResultKind::HorizontalExtents
        }
        HostTextMeasurement::WrappedWithRawWidth { .. } => {
            TextMeasurementResultKind::WrappedWithRawWidth
        }
    };
    let required_kind = request.operation.required_result_kind();
    if result_kind != required_kind {
        return Err(HostTextMeasurementError::invalid_value(format!(
            "host text measurement operation `{}` requires `{}` but returned `{}`",
            request.operation.external_name(),
            required_kind.external_name(),
            result_kind.external_name(),
        )));
    }

    let valid = match measurement {
        HostTextMeasurement::Metrics(metrics) => valid_metrics(request, metrics),
        HostTextMeasurement::Length(value) => {
            value.is_finite() && (request.operation.accepts_signed_length() || *value >= 0.0)
        }
        HostTextMeasurement::HorizontalExtents { left, right } => valid_extents(*left, *right),
        HostTextMeasurement::WrappedWithRawWidth { metrics, raw_width } => {
            valid_metrics(request, metrics)
                && raw_width.is_none_or(|value| value.is_finite() && value >= 0.0)
        }
    };
    if valid {
        Ok(())
    } else {
        Err(HostTextMeasurementError::invalid_value(format!(
            "host text measurement operation `{}` returned an invalid `{}` value",
            request.operation.external_name(),
            result_kind.external_name(),
        )))
    }
}

fn valid_metrics(request: &HostTextMeasurementRequest<'_>, metrics: &TextMetrics) -> bool {
    metrics.width.is_finite()
        && metrics.height.is_finite()
        && metrics.width >= 0.0
        && metrics.height >= 0.0
        && metrics.line_count > 0
        && metrics.line_count <= request.text.len().saturating_add(1)
}

fn valid_extents(left: f64, right: f64) -> bool {
    left.is_finite()
        && right.is_finite()
        && left >= 0.0
        && right >= 0.0
        && (left + right).is_finite()
}

#[cfg(feature = "math")]
fn default_math_backend() -> Option<ConfiguredMathBackend> {
    Some(ConfiguredMathBackend::compiled_ratex())
}

#[cfg(not(feature = "math"))]
fn default_math_backend() -> Option<ConfiguredMathBackend> {
    None
}

fn default_theme_measurement_fallbacks() -> HostMeasurementFallbackPolicy {
    HostMeasurementFallbackPolicy::new([
        HostMeasurementFallback::NativeCatalog,
        HostMeasurementFallback::AcceptHostDependent,
    ])
    .expect("static theme measurement fallback policy")
}

/// Immutable render services and the policy used to capture one operation context.
#[derive(Clone)]
pub struct RenderEnvironment {
    text_measurement: TextMeasurementPolicy,
    text_layout_backend: Option<ConfiguredTextLayoutBackend>,
    capability_policy: RenderCapabilityPolicy,
    math_backend: Option<ConfiguredMathBackend>,
    icon_registry: Option<IconRegistry>,
    runtime_policy: RuntimePolicy,
    resource_policy: RenderResourcePolicy,
    font_catalog: FontCatalog,
    font_source_policy: FontSourcePolicy,
    theme_admission_policy: ThemeAdmissionPolicy,
    theme_resource_ceiling: Arc<ThemeResourcePolicy>,
    theme_measurement_fallbacks: HostMeasurementFallbackPolicy,
    theme_portability: ThemePortabilityRequirement,
}

#[derive(Clone)]
enum ConfiguredTextLayoutBackend {
    #[cfg(feature = "embedded-fonts")]
    BuiltinNative(NativeTextLayoutBackend),
    External(Arc<dyn TextLayoutBackend>),
}

impl ConfiguredTextLayoutBackend {
    fn identity(&self) -> &crate::text::TextLayoutBackendIdentity {
        match self {
            #[cfg(feature = "embedded-fonts")]
            Self::BuiltinNative(backend) => backend.identity(),
            Self::External(backend) => backend.identity(),
        }
    }

    fn capabilities(&self) -> crate::text::TextLayoutCapabilities {
        match self {
            #[cfg(feature = "embedded-fonts")]
            Self::BuiltinNative(backend) => backend.capabilities(),
            Self::External(backend) => backend.capabilities(),
        }
    }

    fn prepare_catalog(
        &self,
        request: &PrepareCatalogRequest,
    ) -> Result<crate::text::PreparedTextLayoutResponse, TextLayoutError> {
        match self {
            #[cfg(feature = "embedded-fonts")]
            Self::BuiltinNative(backend) => backend.prepare_catalog(request),
            Self::External(backend) => backend.prepare_catalog(request),
        }
    }

    const fn is_builtin_native(&self) -> bool {
        match self {
            #[cfg(feature = "embedded-fonts")]
            Self::BuiltinNative(_) => true,
            Self::External(_) => false,
        }
    }
}

impl fmt::Debug for ConfiguredTextLayoutBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConfiguredTextLayoutBackend")
            .field("identity", self.identity())
            .field("builtin_native", &self.is_builtin_native())
            .finish()
    }
}

impl fmt::Debug for RenderEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RenderEnvironment")
            .field("text_measurement", &self.text_measurement)
            .field("text_layout_backend", &self.text_layout_backend)
            .field("capability_policy", &self.capability_policy)
            .field(
                "has_math_renderer",
                &(self.capability_policy.allows(RenderCapability::Math)
                    && self.math_backend.is_some()),
            )
            .field("has_icon_registry", &self.icon_registry.is_some())
            .field("runtime_policy", &self.runtime_policy)
            .field("resource_policy", &self.resource_policy)
            .field("font_catalog", &self.font_catalog.fingerprint())
            .field("font_source_policy", &self.font_source_policy)
            .field("theme_admission_policy", &self.theme_admission_policy)
            .field("theme_resource_ceiling", &self.theme_resource_ceiling)
            .field(
                "theme_measurement_fallbacks",
                &self.theme_measurement_fallbacks,
            )
            .field("theme_portability", &self.theme_portability)
            .finish_non_exhaustive()
    }
}

impl RenderEnvironment {
    /// Creates a target-independent environment with fixed time, UTC, and a fixed seed.
    ///
    /// When the `math` capability is compiled, the environment also installs its built-in math
    /// renderer. Builds without that capability leave the service absent so family admission can
    /// return a typed missing-capability error.
    pub fn deterministic() -> Self {
        Self {
            text_measurement: TextMeasurementPolicy::deterministic(),
            #[cfg(feature = "embedded-fonts")]
            text_layout_backend: Some(ConfiguredTextLayoutBackend::BuiltinNative(
                NativeTextLayoutBackend::default(),
            )),
            #[cfg(not(feature = "embedded-fonts"))]
            text_layout_backend: None,
            capability_policy: RenderCapabilityPolicy::unrestricted(),
            math_backend: default_math_backend(),
            icon_registry: None,
            runtime_policy: RuntimePolicy::deterministic(),
            resource_policy: RenderResourcePolicy::interactive(),
            font_catalog: FontCatalog::system_fonts(),
            font_source_policy: FontSourcePolicy::default(),
            theme_admission_policy: ThemeAdmissionPolicy::default(),
            theme_resource_ceiling: Arc::new(ThemeResourcePolicy::interactive()),
            theme_measurement_fallbacks: default_theme_measurement_fallbacks(),
            theme_portability: ThemePortabilityRequirement::BestEffort,
        }
    }

    /// Creates an environment backed by native clock, timezone, and randomness adapters.
    ///
    /// Timing remains an explicit opt-in because it adds work and observable diagnostics.
    pub fn try_native() -> Result<Self, RuntimePolicyError> {
        Ok(Self::deterministic().with_runtime_policy(RuntimePolicy::try_native()?))
    }

    pub fn with_text_measurement_policy(mut self, policy: TextMeasurementPolicy) -> Self {
        self.text_measurement = policy;
        self
    }

    /// Installs the provisional catalog preparation backend used by custom-font themes.
    ///
    /// The external backend contract remains crate-private until its C4b assurance boundary is
    /// complete. Production environments use the built-in operation-local `rustybuzz` backend.
    pub(crate) fn with_text_layout_backend(mut self, backend: Arc<dyn TextLayoutBackend>) -> Self {
        self.text_layout_backend = Some(ConfiguredTextLayoutBackend::External(backend));
        self
    }

    /// Restricts optional renderer capabilities for every operation begun by this environment.
    ///
    /// This is primarily useful to artifact owners whose public feature contract can be narrower
    /// than Cargo's resolved dependency feature union.
    pub const fn with_capability_policy(mut self, policy: RenderCapabilityPolicy) -> Self {
        self.capability_policy = policy;
        self
    }

    /// Installs the math renderer compiled into this renderer, if present.
    ///
    /// Facades use this to select the canonical compiled capability instead of duplicating Cargo
    /// feature checks in each transport layer.
    pub fn with_compiled_math_renderer(mut self) -> Self {
        self.math_backend = default_math_backend();
        self
    }

    pub(crate) fn with_math_renderer(
        mut self,
        renderer: Arc<dyn MathRenderer + Send + Sync>,
    ) -> Self {
        self.math_backend = Some(ConfiguredMathBackend::external(renderer));
        self
    }

    pub fn without_math_renderer(mut self) -> Self {
        self.math_backend = None;
        self
    }

    pub fn with_icon_registry(mut self, registry: IconRegistry) -> Self {
        self.icon_registry = Some(registry);
        self
    }

    pub fn with_runtime_policy(mut self, policy: RuntimePolicy) -> Self {
        self.runtime_policy = policy;
        self
    }

    pub fn runtime_policy(&self) -> &RuntimePolicy {
        &self.runtime_policy
    }

    pub const fn with_resource_policy(mut self, policy: RenderResourcePolicy) -> Self {
        self.resource_policy = policy;
        self
    }

    /// Retains the font catalog mode authorized for layout evidence and native export.
    ///
    /// Theme compilation supplies exact retained assets for custom catalogs. The unchanged default
    /// path uses [`FontCatalog::system_fonts`], which does not enumerate or freeze host fonts.
    pub fn with_font_catalog(mut self, catalog: FontCatalog) -> Self {
        self.font_catalog = catalog;
        self
    }

    pub const fn font_catalog(&self) -> &FontCatalog {
        &self.font_catalog
    }

    pub fn with_font_source_policy(mut self, policy: FontSourcePolicy) -> Self {
        self.font_source_policy = policy;
        self
    }

    pub const fn font_source_policy(&self) -> &FontSourcePolicy {
        &self.font_source_policy
    }

    pub fn with_theme_admission_policy(mut self, policy: ThemeAdmissionPolicy) -> Self {
        self.theme_admission_policy = policy;
        self
    }

    pub const fn theme_admission_policy(&self) -> &ThemeAdmissionPolicy {
        &self.theme_admission_policy
    }

    /// Sets the host-owned ceiling for compiled theme resources admitted by every new session.
    ///
    /// A compiled theme contributes only a prior restriction. Session creation takes the
    /// pointwise minimum, so a theme can never widen this host authority.
    pub fn with_theme_resource_ceiling(mut self, ceiling: ThemeResourcePolicy) -> Self {
        self.theme_resource_ceiling = Arc::new(ceiling);
        self
    }

    pub fn theme_resource_ceiling(&self) -> &ThemeResourcePolicy {
        &self.theme_resource_ceiling
    }

    pub fn with_theme_measurement_fallbacks(
        mut self,
        policy: HostMeasurementFallbackPolicy,
    ) -> Self {
        self.theme_measurement_fallbacks = policy;
        self
    }

    pub const fn theme_measurement_fallbacks(&self) -> &HostMeasurementFallbackPolicy {
        &self.theme_measurement_fallbacks
    }

    pub const fn with_theme_portability_requirement(
        mut self,
        requirement: ThemePortabilityRequirement,
    ) -> Self {
        self.theme_portability = requirement;
        self
    }

    pub const fn theme_portability_requirement(&self) -> ThemePortabilityRequirement {
        self.theme_portability
    }

    /// Captures time, timezone rules, random seed, and provenance exactly once.
    pub fn begin_session(&self) -> Result<RenderSession, RenderEnvironmentError> {
        self.begin_session_with_control(OperationControl::new())
    }

    /// Captures one unthemed render session using caller-owned cancellation/deadline state.
    pub fn begin_session_with_control(
        &self,
        control: OperationControl,
    ) -> Result<RenderSession, RenderEnvironmentError> {
        control.checkpoint_at(OperationPhase::Layout)?;
        let operation_context = self.runtime_policy.begin_operation()?;
        let resolved_resources = self.resolve_environment_session_resources()?;
        self.begin_session_with_resolved_theme_resources_in_context(
            resolved_resources,
            operation_context,
            control,
        )
    }

    /// Captures one operation while atomically binding the selected compiled theme.
    ///
    /// The environment owns the runtime ceiling. Theme-declared requirements and compiler-time
    /// restrictions are intersected with it before the session captures any backend evidence.
    pub fn begin_session_with_theme(
        &self,
        theme: &DiagramTheme,
    ) -> Result<RenderSession, RenderEnvironmentError> {
        let control = OperationControl::new();
        let resolved_resources = self.resolve_themed_session_resources(theme)?;
        let operation_context = self.runtime_policy.begin_operation()?;
        self.begin_session_with_resolved_theme_resources_in_context(
            resolved_resources,
            operation_context,
            control,
        )
    }

    /// Begins one unthemed render session from caller-captured operation state.
    ///
    /// This entry point deliberately does not call [`RuntimePolicy::begin_operation`]. Facades
    /// that already own the source-to-output operation use it to keep parsing and rendering on
    /// the same runtime context and cancellation/deadline control.
    pub fn begin_session_in_context(
        &self,
        operation_context: OperationContext,
        control: OperationControl,
    ) -> Result<RenderSession, RenderEnvironmentError> {
        control.checkpoint_at(OperationPhase::Layout)?;
        let resolved_resources = self.resolve_environment_session_resources()?;
        self.begin_session_with_resolved_theme_resources_in_context(
            resolved_resources,
            operation_context,
            control,
        )
    }

    /// Begins one themed render session from caller-captured operation state.
    ///
    /// The caller remains the sole owner of runtime context and cancellation while the
    /// environment atomically resolves theme admission and resources for that same operation.
    pub fn begin_session_with_theme_in_context(
        &self,
        theme: &DiagramTheme,
        operation_context: OperationContext,
        control: OperationControl,
    ) -> Result<RenderSession, RenderEnvironmentError> {
        control.checkpoint_at(OperationPhase::Layout)?;
        let resolved_resources = self.resolve_themed_session_resources(theme)?;
        self.begin_session_with_resolved_theme_resources_in_context(
            resolved_resources,
            operation_context,
            control,
        )
    }

    fn resolve_environment_session_resources(
        &self,
    ) -> Result<ResolvedSessionThemeResources, RenderEnvironmentError> {
        self.font_catalog
            .validate_retained_resources(&self.theme_resource_ceiling)?;
        Ok(ResolvedSessionThemeResources::new(
            SessionThemeResources::Environment {
                font_catalog: self.font_catalog.clone(),
                font_source_policy: self.font_source_policy.clone(),
            },
            Arc::clone(&self.theme_resource_ceiling),
        ))
    }

    fn resolve_themed_session_resources(
        &self,
        theme: &DiagramTheme,
    ) -> Result<ResolvedSessionThemeResources, RenderEnvironmentError> {
        let admission = theme.resolve_runtime_admission(
            &self.theme_admission_policy,
            &self.font_source_policy,
            &self.theme_measurement_fallbacks,
            self.theme_portability,
        )?;
        let effective_theme_resource_policy =
            if self.theme_resource_ceiling.as_ref() == theme.resource_restriction() {
                Arc::clone(&self.theme_resource_ceiling)
            } else {
                Arc::new(
                    self.theme_resource_ceiling
                        .meet(theme.resource_restriction()),
                )
            };
        theme.validate_retained_resources(&effective_theme_resource_policy)?;
        Ok(ResolvedSessionThemeResources::new(
            SessionThemeResources::Theme {
                theme: theme.clone(),
                admission,
            },
            effective_theme_resource_policy,
        ))
    }

    fn begin_session_with_resolved_theme_resources_in_context(
        &self,
        resolved_resources: ResolvedSessionThemeResources,
        operation_context: OperationContext,
        control: OperationControl,
    ) -> Result<RenderSession, RenderEnvironmentError> {
        control.checkpoint_at(OperationPhase::Layout)?;
        let theme_compatibility_recipe = resolved_resources
            .theme()
            .map(|theme| theme.parse_compatibility().recipe().clone());
        let portability_requirement = resolved_resources
            .portability_requirement()
            .unwrap_or(self.theme_portability);
        let (prepared_text_layout, text_layout_error) =
            self.prepare_text_layout(&resolved_resources, portability_requirement);
        let trusted_theme_lanes = resolved_resources
            .admission()
            .map(ResolvedThemeAdmission::trusted_lanes)
            .unwrap_or_else(|| self.theme_admission_policy.trusted_lanes())
            .clone();
        Ok(RenderSession {
            text_measurement: self.text_measurement.clone(),
            prepared_text_layout,
            text_layout_error,
            measurement_recorder: Box::default(),
            capability_policy: self.capability_policy,
            math_backend: self.math_backend.clone(),
            icon_registry: self.icon_registry.clone(),
            operation_context,
            resource_policy: self.resource_policy,
            work_meter: Arc::new(OperationWorkMeter::new_with_control(
                self.resource_policy,
                control,
            )),
            trusted_theme_lanes,
            trusted_theme_lane_usage: AtomicU64::new(0),
            theme_compatibility_recipe,
            portability_requirement,
            resolved_theme_resources: resolved_resources,
        })
    }

    fn prepare_text_layout(
        &self,
        resolved_resources: &ResolvedSessionThemeResources,
        portability: ThemePortabilityRequirement,
    ) -> (Option<PreparedTextLayout>, Option<TextLayoutError>) {
        let catalog = resolved_resources.font_catalog();
        if !catalog.requires_prepared_text_layout() {
            return (None, None);
        }

        let Some(backend) = &self.text_layout_backend else {
            return (None, Some(TextLayoutError::BackendRejected));
        };
        let request = PrepareCatalogRequest::new(
            catalog.clone(),
            resolved_resources.font_source_policy().clone(),
        );
        let fallback_policy = resolved_resources
            .measurement_fallback_policy()
            .unwrap_or(&self.theme_measurement_fallbacks);
        let mut builder = PreparedTextLayoutBuilder::new(request.clone());
        let mut deferred_host_response = None;
        let mut terminal_error = None;

        match backend.prepare_catalog(&request) {
            Ok(response) => {
                if backend.is_builtin_native() {
                    match builder.admit_portable_response(
                        backend.identity(),
                        backend.capabilities(),
                        response,
                        None,
                    ) {
                        Ok(()) => {}
                        Err(error) => {
                            builder.record_catalog_failure();
                            terminal_error = Some(error);
                        }
                    }
                } else {
                    let deferred = response.clone();
                    let mut probe = PreparedTextLayoutBuilder::new(request.clone());
                    match probe.admit_host_dependent_response(
                        backend.identity(),
                        backend.capabilities(),
                        response,
                        ThemePortabilityRequirement::BestEffort,
                    ) {
                        Ok(()) => deferred_host_response = Some(deferred),
                        Err(error) => {
                            builder.record_catalog_failure();
                            terminal_error = Some(error);
                        }
                    }
                }
            }
            Err(error) => {
                builder.record_catalog_failure();
                terminal_error = Some(error);
            }
        }

        for fallback in fallback_policy.priority() {
            match fallback {
                #[cfg(not(feature = "embedded-fonts"))]
                HostMeasurementFallback::NativeCatalog => {}
                #[cfg(feature = "embedded-fonts")]
                HostMeasurementFallback::NativeCatalog => {
                    let native = NativeTextLayoutBackend::default();
                    if builder.has_backend(native.identity()) {
                        continue;
                    }
                    match native.prepare_catalog(&request) {
                        Ok(response) => {
                            if let Err(error) = builder.admit_portable_response(
                                native.identity(),
                                native.capabilities(),
                                response,
                                Some(HostMeasurementFallback::NativeCatalog),
                            ) {
                                builder.record_catalog_failure();
                                terminal_error = Some(error);
                            }
                        }
                        Err(error) => {
                            builder.record_catalog_failure();
                            terminal_error = Some(error);
                        }
                    }
                }
                HostMeasurementFallback::AcceptHostDependent => {
                    let Some(response) = deferred_host_response.take() else {
                        continue;
                    };
                    if let Err(error) = builder.admit_host_dependent_response(
                        backend.identity(),
                        backend.capabilities(),
                        response,
                        portability,
                    ) {
                        builder.record_catalog_failure();
                        terminal_error = Some(error);
                    }
                }
            }
        }

        if builder.is_empty() {
            return (
                None,
                Some(terminal_error.unwrap_or(TextLayoutError::NoUsableFace)),
            );
        }
        match builder.build() {
            Ok(layout) => (Some(layout), None),
            Err(error) => (None, Some(error)),
        }
    }
}

impl Default for RenderEnvironment {
    fn default() -> Self {
        Self::deterministic()
    }
}

enum SessionThemeResources {
    Environment {
        font_catalog: FontCatalog,
        font_source_policy: FontSourcePolicy,
    },
    Theme {
        theme: DiagramTheme,
        admission: ResolvedThemeAdmission,
    },
}

impl SessionThemeResources {
    fn theme(&self) -> Option<&DiagramTheme> {
        match self {
            Self::Environment { .. } => None,
            Self::Theme { theme, .. } => Some(theme),
        }
    }

    fn font_catalog(&self) -> &FontCatalog {
        match self {
            Self::Environment { font_catalog, .. } => font_catalog,
            Self::Theme { theme, .. } => theme.font_catalog(),
        }
    }

    fn font_source_policy(&self) -> &FontSourcePolicy {
        match self {
            Self::Environment {
                font_source_policy, ..
            } => font_source_policy,
            Self::Theme { admission, .. } => admission.font_source_policy(),
        }
    }

    fn admission(&self) -> Option<&ResolvedThemeAdmission> {
        match self {
            Self::Environment { .. } => None,
            Self::Theme { admission, .. } => Some(admission),
        }
    }

    fn measurement_fallback_policy(&self) -> Option<&HostMeasurementFallbackPolicy> {
        self.admission()
            .map(ResolvedThemeAdmission::measurement_fallback_policy)
    }

    fn portability_requirement(&self) -> Option<ThemePortabilityRequirement> {
        self.admission()
            .map(ResolvedThemeAdmission::portability_requirement)
    }

    fn theme_recipe_report(&self) -> Option<&ThemeRecipeReport> {
        self.theme().map(DiagramTheme::report)
    }

    fn theme_host_admission_report(&self) -> Option<ThemeHostAdmissionReport> {
        self.admission().map(ResolvedThemeAdmission::report)
    }
}

/// Session-owned theme resources and the exact policy that admitted them.
///
/// Keeping these values together prevents a retained catalog or compiled theme from being paired
/// with a different host/theme policy intersection after admission.
struct ResolvedSessionThemeResources {
    resources: SessionThemeResources,
    effective_policy: Arc<ThemeResourcePolicy>,
}

impl ResolvedSessionThemeResources {
    fn new(resources: SessionThemeResources, effective_policy: Arc<ThemeResourcePolicy>) -> Self {
        Self {
            resources,
            effective_policy,
        }
    }

    fn effective_policy(&self) -> &Arc<ThemeResourcePolicy> {
        &self.effective_policy
    }

    fn theme(&self) -> Option<&DiagramTheme> {
        self.resources.theme()
    }

    fn font_catalog(&self) -> &FontCatalog {
        self.resources.font_catalog()
    }

    fn font_source_policy(&self) -> &FontSourcePolicy {
        self.resources.font_source_policy()
    }

    fn admission(&self) -> Option<&ResolvedThemeAdmission> {
        self.resources.admission()
    }

    fn measurement_fallback_policy(&self) -> Option<&HostMeasurementFallbackPolicy> {
        self.resources.measurement_fallback_policy()
    }

    fn portability_requirement(&self) -> Option<ThemePortabilityRequirement> {
        self.resources.portability_requirement()
    }

    fn theme_recipe_report(&self) -> Option<&ThemeRecipeReport> {
        self.resources.theme_recipe_report()
    }

    fn theme_host_admission_report(&self) -> Option<ThemeHostAdmissionReport> {
        self.resources.theme_host_admission_report()
    }
}

/// Opaque operation session. Family code receives only the narrow projection it needs.
pub struct RenderSession {
    text_measurement: TextMeasurementPolicy,
    prepared_text_layout: Option<PreparedTextLayout>,
    text_layout_error: Option<TextLayoutError>,
    // Keep movable family artifacts compact for bounded worker stacks.
    measurement_recorder: Box<TextMeasurementRecorder>,
    capability_policy: RenderCapabilityPolicy,
    math_backend: Option<ConfiguredMathBackend>,
    icon_registry: Option<IconRegistry>,
    operation_context: OperationContext,
    resource_policy: RenderResourcePolicy,
    work_meter: Arc<OperationWorkMeter>,
    trusted_theme_lanes: TrustedThemeLanes,
    trusted_theme_lane_usage: AtomicU64,
    theme_compatibility_recipe: Option<ThemeCompatibilityRecipe>,
    portability_requirement: ThemePortabilityRequirement,
    resolved_theme_resources: ResolvedSessionThemeResources,
}

impl RenderSession {
    pub fn text_measurer(&self, default_phase: TextMeasurementPhase) -> RoutedTextMeasurer<'_> {
        self.routed_text_measurer(default_phase, None)
    }

    pub(crate) fn controlled_text_measurer(
        &self,
        default_phase: TextMeasurementPhase,
        operation_phase: OperationPhase,
    ) -> RoutedTextMeasurer<'_> {
        self.routed_text_measurer(default_phase, Some(operation_phase))
    }

    fn routed_text_measurer(
        &self,
        default_phase: TextMeasurementPhase,
        controlled_operation_phase: Option<OperationPhase>,
    ) -> RoutedTextMeasurer<'_> {
        RoutedTextMeasurer {
            default_phase,
            policy: &self.text_measurement,
            recorder: &self.measurement_recorder,
            work_meter: self.work_meter.as_ref(),
            controlled_operation_phase,
        }
    }

    /// Returns the immutable layout session prepared against this operation's retained catalog.
    pub(crate) fn prepared_text_layout(&self) -> Option<&PreparedTextLayout> {
        self.prepared_text_layout.as_ref()
    }

    /// Returns a bounded preparation failure, if a custom catalog could not be attested.
    pub(crate) fn text_layout_error(&self) -> Option<&TextLayoutError> {
        self.text_layout_error.as_ref()
    }

    pub fn text_measurement_route(&self, phase: TextMeasurementPhase) -> TextMeasurementRoute {
        self.text_measurement.route(phase)
    }

    pub fn text_measurement_report(&self) -> TextMeasurementReport {
        self.measurement_recorder.report(&self.text_measurement)
    }

    pub fn operation_context(&self) -> &OperationContext {
        &self.operation_context
    }

    pub fn operation_timing(&self) -> Option<OperationTiming> {
        self.operation_context.timing()
    }

    pub const fn unix_millis(&self) -> i64 {
        self.operation_context.unix_millis()
    }

    pub const fn local_date(&self) -> merman_core::time::CivilDate {
        self.operation_context.today_local()
    }

    pub fn local_time_zone(&self) -> &merman_core::time::LocalTimeZone {
        self.operation_context.local_time_zone()
    }

    pub fn render_seed(&self) -> NonZeroU64 {
        self.operation_context.derive_nonzero_u64("render.root", 0)
    }

    pub const fn resource_policy(&self) -> RenderResourcePolicy {
        self.resource_policy
    }

    /// Returns the immutable host/theme resource intersection captured for this operation.
    pub(crate) fn effective_theme_resource_policy(&self) -> Arc<ThemeResourcePolicy> {
        Arc::clone(self.resolved_theme_resources.effective_policy())
    }

    pub fn theme(&self) -> Option<&DiagramTheme> {
        self.resolved_theme_resources.theme()
    }

    pub fn theme_recipe_fingerprint(&self) -> Option<ThemeRecipeFingerprint> {
        self.theme().map(DiagramTheme::recipe_fingerprint)
    }

    pub(crate) fn theme_compatibility_recipe(&self) -> Option<&ThemeCompatibilityRecipe> {
        self.theme_compatibility_recipe.as_ref()
    }

    pub fn theme_recipe_report(&self) -> Option<&ThemeRecipeReport> {
        self.resolved_theme_resources.theme_recipe_report()
    }

    pub fn theme_host_admission_report(&self) -> Option<ThemeHostAdmissionReport> {
        self.resolved_theme_resources.theme_host_admission_report()
    }

    pub fn font_catalog(&self) -> &FontCatalog {
        self.resolved_theme_resources.font_catalog()
    }

    pub fn font_source_policy(&self) -> &FontSourcePolicy {
        self.resolved_theme_resources.font_source_policy()
    }

    pub fn theme_measurement_fallback_policy(&self) -> Option<&HostMeasurementFallbackPolicy> {
        self.resolved_theme_resources.measurement_fallback_policy()
    }

    pub fn theme_portability_requirement(&self) -> Option<ThemePortabilityRequirement> {
        self.resolved_theme_resources.portability_requirement()
    }

    /// Returns the effective host/theme portability requirement frozen for this operation.
    pub const fn portability_requirement(&self) -> ThemePortabilityRequirement {
        self.portability_requirement
    }

    pub fn trusted_theme_lanes(&self) -> &TrustedThemeLanes {
        &self.trusted_theme_lanes
    }

    pub(crate) fn use_trusted_theme_lane(
        &self,
        lane: TrustedThemeLane,
    ) -> Result<(), ThemeAdmissionError> {
        if !self.trusted_theme_lanes.contains(lane) {
            return Err(ThemeAdmissionError::TrustedThemeLaneDenied(lane));
        }
        self.trusted_theme_lane_usage
            .fetch_or(lane.usage_mask(), Ordering::Relaxed);
        Ok(())
    }

    fn used_trusted_theme_lanes(&self) -> TrustedThemeLanes {
        let usage = self.trusted_theme_lane_usage.load(Ordering::Relaxed);
        TrustedThemeLanes::from_allowed(
            TrustedThemeLane::ALL
                .iter()
                .copied()
                .filter(|lane| usage & lane.usage_mask() != 0),
        )
    }

    /// Reports effective operation availability after policy and backend/service resolution.
    pub(crate) fn supports_capability(&self, capability: RenderCapability) -> bool {
        if !self.capability_policy.allows(capability) {
            return false;
        }
        match capability {
            RenderCapability::LayoutCytoscape => crate::layout_cytoscape_available(),
            RenderCapability::LayoutElk => crate::layout_elk_available(),
            RenderCapability::Math => self
                .math_backend
                .as_ref()
                .is_some_and(|backend| backend.supports_resource_policy(self.resource_policy)),
        }
    }

    pub(crate) fn work_meter(&self) -> &Arc<OperationWorkMeter> {
        &self.work_meter
    }

    /// Checks the operation-owned control at an SVG/render phase boundary.
    pub(crate) fn checkpoint(&self, phase: OperationPhase) -> crate::Result<()> {
        self.work_meter().checkpoint(phase).map_err(Into::into)
    }

    pub(crate) fn math_renderer(&self) -> Option<&(dyn MathRenderer + Send + Sync)> {
        self.math_backend().map(ConfiguredMathBackend::renderer)
    }

    /// Returns the operation-selected math backend, retaining whether it is native-capable.
    pub(crate) fn math_backend(&self) -> Option<&ConfiguredMathBackend> {
        self.supports_capability(RenderCapability::Math)
            .then_some(())
            .and(self.math_backend.as_ref())
    }

    pub fn icon_registry(&self) -> Option<&IconRegistry> {
        self.icon_registry.as_ref()
    }

    /// Freezes the observable policy and provenance accumulated so far.
    pub fn report(&self) -> RenderSessionReport {
        RenderSessionReport {
            measurement_routes: self.text_measurement.routes(),
            measurement: self.measurement_recorder.report(&self.text_measurement),
            operation_context: self.operation_context.clone(),
            local_time_zone: self
                .operation_context
                .local_time_zone()
                .provenance()
                .clone(),
            resource_policy: self.resource_policy,
            effective_theme_resource_policy: Arc::clone(
                self.resolved_theme_resources.effective_policy(),
            ),
            layout_work_units: self.work_meter.used(),
            prepared_text_retained_bytes_peak: self.work_meter.prepared_text_retained_bytes_peak(),
            theme_recipe_report: self.theme_recipe_report().cloned(),
            theme_host_admission_report: self.theme_host_admission_report(),
            portability_requirement: self.portability_requirement,
            font_catalog_fingerprint: self.font_catalog().fingerprint(),
            font_source_policy: self.font_source_policy().clone(),
            prepared_text_layout: self
                .prepared_text_layout
                .as_ref()
                .map(PreparedTextLayout::report),
            text_layout_failure: self.text_layout_error.as_ref().map(TextLayoutFailure::from),
            trusted_theme_lanes: self.trusted_theme_lanes.clone(),
            used_trusted_theme_lanes: self.used_trusted_theme_lanes(),
        }
    }
}

/// Immutable environment evidence accumulated by an operation session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderSessionReport {
    measurement_routes: [TextMeasurementRoute; 4],
    measurement: TextMeasurementReport,
    operation_context: OperationContext,
    local_time_zone: LocalTimeZoneProvenance,
    resource_policy: RenderResourcePolicy,
    effective_theme_resource_policy: Arc<ThemeResourcePolicy>,
    layout_work_units: usize,
    prepared_text_retained_bytes_peak: usize,
    theme_recipe_report: Option<ThemeRecipeReport>,
    theme_host_admission_report: Option<ThemeHostAdmissionReport>,
    portability_requirement: ThemePortabilityRequirement,
    font_catalog_fingerprint: FontCatalogFingerprint,
    font_source_policy: FontSourcePolicy,
    prepared_text_layout: Option<PreparedTextLayoutReport>,
    text_layout_failure: Option<TextLayoutFailure>,
    trusted_theme_lanes: TrustedThemeLanes,
    used_trusted_theme_lanes: TrustedThemeLanes,
}

impl RenderSessionReport {
    pub fn measurement_routes(&self) -> &[TextMeasurementRoute; 4] {
        &self.measurement_routes
    }

    pub fn measurement(&self) -> &TextMeasurementReport {
        &self.measurement
    }

    pub fn operation_context(&self) -> &OperationContext {
        &self.operation_context
    }

    pub const fn unix_millis(&self) -> i64 {
        self.operation_context.unix_millis()
    }

    pub const fn local_date(&self) -> merman_core::time::CivilDate {
        self.operation_context.today_local()
    }

    pub fn local_time_zone(&self) -> &LocalTimeZoneProvenance {
        &self.local_time_zone
    }

    pub fn render_seed(&self) -> NonZeroU64 {
        self.operation_context.derive_nonzero_u64("render.root", 0)
    }

    pub const fn resource_policy(&self) -> RenderResourcePolicy {
        self.resource_policy
    }

    pub(crate) fn effective_theme_resource_policy(&self) -> &ThemeResourcePolicy {
        &self.effective_theme_resource_policy
    }

    pub fn theme_recipe_fingerprint(&self) -> Option<ThemeRecipeFingerprint> {
        self.theme_recipe_report
            .as_ref()
            .map(ThemeRecipeReport::theme_recipe_fingerprint)
    }

    pub const fn theme_recipe_report(&self) -> Option<&ThemeRecipeReport> {
        self.theme_recipe_report.as_ref()
    }

    pub const fn theme_host_admission_report(&self) -> Option<&ThemeHostAdmissionReport> {
        self.theme_host_admission_report.as_ref()
    }

    /// Returns the effective host/theme portability requirement frozen for this operation.
    pub const fn portability_requirement(&self) -> ThemePortabilityRequirement {
        self.portability_requirement
    }

    pub const fn font_catalog_fingerprint(&self) -> FontCatalogFingerprint {
        self.font_catalog_fingerprint
    }

    pub fn prepared_text_layout(&self) -> Option<&PreparedTextLayoutReport> {
        self.prepared_text_layout.as_ref()
    }

    pub const fn text_layout_failure(&self) -> Option<TextLayoutFailure> {
        self.text_layout_failure
    }

    pub const fn font_source_policy(&self) -> &FontSourcePolicy {
        &self.font_source_policy
    }

    pub const fn trusted_theme_lanes(&self) -> &TrustedThemeLanes {
        &self.trusted_theme_lanes
    }

    pub const fn used_trusted_theme_lanes(&self) -> &TrustedThemeLanes {
        &self.used_trusted_theme_lanes
    }

    /// Returns the deterministic owner-accounted layout and geometry work consumed so far.
    ///
    /// This value is useful for resource-policy calibration. It is not elapsed time, an
    /// instruction count, or a portable latency estimate.
    pub const fn layout_work_units(&self) -> usize {
        self.layout_work_units
    }

    /// Returns the peak owner-accounted prepared-text bytes retained by the operation.
    pub const fn prepared_text_retained_bytes_peak(&self) -> usize {
        self.prepared_text_retained_bytes_peak
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::ResourceLimitId;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn inline_html_carrier<M: TextMeasurer + ?Sized>(measurer: &M) -> InlineHtmlMeasurementCarrier {
        measurer
            .builtin_operation_carrier(TextMeasurementOperation::WrappedWithRawWidth)
            .and_then(BuiltinTextMeasurementOperationCarrier::into_inline_html)
            .unwrap_or_else(InlineHtmlMeasurementCarrier::opaque)
    }

    #[test]
    fn deterministic_profile_identity_tracks_the_render_crate() {
        let profile = deterministic_profile();

        assert_eq!(
            profile.identity().profile().as_str(),
            "merman.deterministic-text"
        );
        assert_eq!(
            profile.identity().version(),
            concat!("merman-render@", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn text_measurement_operations_have_stable_external_mappings() {
        let mappings = TextMeasurementOperation::ALL
            .map(|operation| (operation.external_code(), operation.external_name()));

        assert_eq!(
            mappings,
            [
                (0, "measure"),
                (1, "computed-length"),
                (2, "bbox-x"),
                (3, "bbox-x-with-ascii-overhang"),
                (4, "title-bbox-x"),
                (5, "simple-bbox-width"),
                (6, "raw-bbox-width"),
                (7, "tspan-bbox-width"),
                (8, "tspan-bbox-height"),
                (9, "wrap-probe-bbox-width"),
                (10, "simple-bbox-height"),
                (11, "wrapped"),
                (12, "wrapped-with-raw-width"),
                (13, "bounding-client-rect-width"),
                (14, "create-text-bbox-y-offset"),
                (15, "mermaid-calculate-text-dimensions"),
                (16, "canvas-measure-text-width"),
                (17, "create-text-middle-bbox-y-offset"),
                (18, "raw-bbox-height"),
            ]
        );
    }

    #[test]
    fn deterministic_environment_projects_one_operation_context_into_the_report() {
        let runtime_policy = RuntimePolicy::deterministic()
            .with_fixed_unix_millis(1_704_067_200_000)
            .try_with_fixed_local_offset_minutes(480)
            .expect("valid fixed offset")
            .with_fixed_seed(77);
        let environment = RenderEnvironment::deterministic().with_runtime_policy(runtime_policy);

        let session = environment.begin_session().expect("render session");
        let captured = session.operation_context().clone();
        let report = session.report();

        assert_eq!(captured.unix_millis(), 1_704_067_200_000);
        assert_eq!(captured.seed(), 77);
        assert_eq!(captured.local_time_zone().fixed_offset_minutes(), Some(480));
        assert_eq!(report.operation_context(), &captured);
        assert_eq!(report.unix_millis(), captured.unix_millis());
        assert_eq!(report.operation_context().seed(), captured.seed());
        assert_eq!(
            report.render_seed(),
            captured.derive_nonzero_u64("render.root", 0)
        );
        assert_eq!(
            report.local_time_zone(),
            captured.local_time_zone().provenance()
        );
    }

    #[test]
    fn caller_captured_context_and_control_define_one_render_session() {
        let operation_context = RuntimePolicy::deterministic()
            .with_fixed_unix_millis(1_704_067_200_123)
            .with_fixed_seed(91)
            .begin_operation()
            .expect("caller operation context");
        let control = OperationControl::new();
        let session = RenderEnvironment::deterministic()
            .begin_session_in_context(operation_context.clone(), control.clone())
            .expect("caller-captured render session");

        assert_eq!(session.operation_context(), &operation_context);

        control.cancel();
        let error = session
            .checkpoint(OperationPhase::Layout)
            .expect_err("shared control should cancel the render session");
        let crate::Error::Cancelled(cancelled) = error else {
            panic!("expected structured cancellation");
        };
        assert_eq!(cancelled.phase, OperationPhase::Layout);
        assert_eq!(cancelled.reason, merman_core::CancelReason::Requested);
    }

    #[test]
    fn descriptor_drives_host_result_validation_for_every_operation() {
        let style = TextStyle::default();
        let request = |operation| HostTextMeasurementRequest {
            operation,
            phase: TextMeasurementPhase::Layout,
            text: "contract",
            style: &style,
            max_width: None,
            wrap_mode: WrapMode::SvgLike,
        };
        let valid_metrics_value = HostTextMeasurement::Metrics(metrics(10.0));
        let valid_length = HostTextMeasurement::Length(10.0);
        let negative_length = HostTextMeasurement::Length(-10.0);
        let invalid_length = HostTextMeasurement::Length(f64::NAN);
        let valid_extents = HostTextMeasurement::HorizontalExtents {
            left: 1.0,
            right: 2.0,
        };
        let valid_wrapped = HostTextMeasurement::WrappedWithRawWidth {
            metrics: metrics(10.0),
            raw_width: Some(11.0),
        };

        for operation in TextMeasurementOperation::ALL {
            let required = operation.required_result_kind();
            assert_eq!(
                validate_host_text_measurement(&request(operation), &valid_metrics_value).is_ok(),
                required == TextMeasurementResultKind::Metrics,
                "{} metrics contract",
                operation.external_name()
            );
            assert_eq!(
                validate_host_text_measurement(&request(operation), &valid_length).is_ok(),
                required == TextMeasurementResultKind::Length,
                "{} length contract",
                operation.external_name()
            );
            assert_eq!(
                validate_host_text_measurement(&request(operation), &negative_length).is_ok(),
                required == TextMeasurementResultKind::Length && operation.accepts_signed_length(),
                "{} signed-length contract",
                operation.external_name()
            );
            assert!(
                validate_host_text_measurement(&request(operation), &invalid_length).is_err(),
                "{} accepted a non-finite length",
                operation.external_name()
            );
            assert_eq!(
                validate_host_text_measurement(&request(operation), &valid_extents).is_ok(),
                required == TextMeasurementResultKind::HorizontalExtents,
                "{} extents contract",
                operation.external_name()
            );
            assert_eq!(
                validate_host_text_measurement(&request(operation), &valid_wrapped).is_ok(),
                required == TextMeasurementResultKind::WrappedWithRawWidth,
                "{} wrapped contract",
                operation.external_name()
            );
        }
    }

    #[test]
    fn checked_host_measurement_rejects_malformed_numeric_boundaries() {
        let style = TextStyle::default();
        let request = |operation, text| HostTextMeasurementRequest {
            operation,
            phase: TextMeasurementPhase::Layout,
            text,
            style: &style,
            max_width: None,
            wrap_mode: WrapMode::SvgLike,
        };
        let metrics_value = |width, height, line_count| {
            HostTextMeasurement::Metrics(TextMetrics {
                width,
                height,
                line_count,
            })
        };

        let metrics_request = request(TextMeasurementOperation::Measure, "abc");
        assert!(
            validate_host_text_measurement(&metrics_request, &metrics_value(1.0, 2.0, 4)).is_ok()
        );
        for invalid in [
            metrics_value(f64::NAN, 2.0, 1),
            metrics_value(f64::INFINITY, 2.0, 1),
            metrics_value(-1.0, 2.0, 1),
            metrics_value(1.0, f64::NAN, 1),
            metrics_value(1.0, f64::INFINITY, 1),
            metrics_value(1.0, -2.0, 1),
            metrics_value(1.0, 2.0, 0),
            metrics_value(1.0, 2.0, 5),
        ] {
            assert!(validate_host_text_measurement(&metrics_request, &invalid).is_err());
        }

        let length_request = request(TextMeasurementOperation::ComputedLength, "abc");
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
            assert!(
                validate_host_text_measurement(
                    &length_request,
                    &HostTextMeasurement::Length(value),
                )
                .is_err()
            );
        }
        for operation in [
            TextMeasurementOperation::CreateTextBBoxYOffset,
            TextMeasurementOperation::CreateTextMiddleBBoxYOffset,
        ] {
            assert!(
                validate_host_text_measurement(
                    &request(operation, "abc"),
                    &HostTextMeasurement::Length(-1.0),
                )
                .is_ok()
            );
        }

        let extents_request = request(TextMeasurementOperation::BBoxX, "abc");
        for (left, right) in [
            (f64::NAN, 1.0),
            (1.0, f64::INFINITY),
            (-1.0, 1.0),
            (1.0, -1.0),
            (f64::MAX, f64::MAX),
        ] {
            assert!(
                validate_host_text_measurement(
                    &extents_request,
                    &HostTextMeasurement::HorizontalExtents { left, right },
                )
                .is_err()
            );
        }

        let wrapped_request = request(TextMeasurementOperation::WrappedWithRawWidth, "abc");
        assert!(
            validate_host_text_measurement(
                &wrapped_request,
                &HostTextMeasurement::WrappedWithRawWidth {
                    metrics: TextMetrics {
                        width: 10.0,
                        height: 20.0,
                        line_count: 1,
                    },
                    raw_width: Some(1.0),
                },
            )
            .is_ok(),
            "raw width may be smaller than wrapped width"
        );
        for raw_width in [f64::NAN, f64::INFINITY, -1.0] {
            assert!(
                validate_host_text_measurement(
                    &wrapped_request,
                    &HostTextMeasurement::WrappedWithRawWidth {
                        metrics: metrics(10.0),
                        raw_width: Some(raw_width),
                    },
                )
                .is_err()
            );
        }
    }

    struct OperationAwareHost {
        operations: Arc<Mutex<Vec<TextMeasurementOperation>>>,
    }

    impl HostTextMeasurer for OperationAwareHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            self.operations
                .lock()
                .expect("operation probe lock")
                .push(request.operation);
            match request.operation {
                TextMeasurementOperation::ComputedLength => {
                    Ok(Some(HostTextMeasurement::Length(73.25)))
                }
                TextMeasurementOperation::BoundingClientRectWidth => {
                    Ok(Some(HostTextMeasurement::Length(91.875)))
                }
                TextMeasurementOperation::CreateTextBBoxYOffset => {
                    Ok(Some(HostTextMeasurement::Length(-1.25)))
                }
                TextMeasurementOperation::MermaidCalculateTextDimensions => {
                    Ok(Some(HostTextMeasurement::Metrics(metrics(82.5))))
                }
                TextMeasurementOperation::CanvasMeasureTextWidth => {
                    Ok(Some(HostTextMeasurement::Length(94.25)))
                }
                TextMeasurementOperation::CreateTextMiddleBBoxYOffset => {
                    Ok(Some(HostTextMeasurement::Length(-2.5)))
                }
                _ => Ok(None),
            }
        }
    }

    #[test]
    fn host_computed_length_receives_exact_operation_and_is_authoritative() {
        let operations = Arc::new(Mutex::new(Vec::new()));
        let policy = TextMeasurementPolicy::host_display_with_fallback(
            identity("test.operation-aware-host", "v1", &[]),
            Arc::new(OperationAwareHost {
                operations: Arc::clone(&operations),
            }),
            [TextMeasurementPhase::ComputedLength],
            deterministic_profile(),
        );
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(policy)
            .begin_session()
            .expect("begin render session");

        let length = session
            .text_measurer(TextMeasurementPhase::Layout)
            .measure_svg_text_computed_length_px("operation", &TextStyle::default());

        assert_eq!(length, 73.25);
        assert_eq!(
            *operations.lock().expect("operation probe lock"),
            [TextMeasurementOperation::ComputedLength]
        );
    }

    #[test]
    fn host_bounding_client_rect_width_receives_exact_operation_and_is_authoritative() {
        let operations = Arc::new(Mutex::new(Vec::new()));
        let policy = TextMeasurementPolicy::host_display_with_fallback(
            identity("test.operation-aware-host", "v1", &[]),
            Arc::new(OperationAwareHost {
                operations: Arc::clone(&operations),
            }),
            [TextMeasurementPhase::SvgBBox],
            deterministic_profile(),
        );
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(policy)
            .begin_session()
            .expect("begin render session");

        let length = session
            .text_measurer(TextMeasurementPhase::Layout)
            .measure_svg_text_bounding_client_rect_width_px("operation", &TextStyle::default());

        assert_eq!(length, 91.875);
        assert_eq!(
            *operations.lock().expect("operation probe lock"),
            [TextMeasurementOperation::BoundingClientRectWidth]
        );
    }

    #[test]
    fn host_create_text_bbox_y_offset_accepts_signed_authoritative_values() {
        let operations = Arc::new(Mutex::new(Vec::new()));
        let policy = TextMeasurementPolicy::host_display_with_fallback(
            identity("test.operation-aware-host", "v1", &[]),
            Arc::new(OperationAwareHost {
                operations: Arc::clone(&operations),
            }),
            [TextMeasurementPhase::SvgBBox],
            deterministic_profile(),
        );
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(policy)
            .begin_session()
            .expect("begin render session");

        let offset = session
            .text_measurer(TextMeasurementPhase::Layout)
            .measure_svg_create_text_bbox_y_offset_px("operation", &TextStyle::default());

        assert_eq!(offset, -1.25);
        assert_eq!(
            *operations.lock().expect("operation probe lock"),
            [TextMeasurementOperation::CreateTextBBoxYOffset]
        );
    }

    #[test]
    fn host_create_text_middle_bbox_y_offset_is_a_distinct_signed_operation() {
        let operations = Arc::new(Mutex::new(Vec::new()));
        let policy = TextMeasurementPolicy::host_display_with_fallback(
            identity("test.operation-aware-host", "v1", &[]),
            Arc::new(OperationAwareHost {
                operations: Arc::clone(&operations),
            }),
            [TextMeasurementPhase::SvgBBox],
            deterministic_profile(),
        );
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(policy)
            .begin_session()
            .expect("begin render session");

        let offset = session
            .text_measurer(TextMeasurementPhase::Layout)
            .measure_svg_create_text_middle_bbox_y_offset_px("operation", &TextStyle::default());

        assert_eq!(offset, -2.5);
        assert_eq!(
            *operations.lock().expect("operation probe lock"),
            [TextMeasurementOperation::CreateTextMiddleBBoxYOffset]
        );
    }

    #[test]
    fn host_source_specific_width_operations_are_authoritative() {
        let operations = Arc::new(Mutex::new(Vec::new()));
        let policy = TextMeasurementPolicy::host_display_with_fallback(
            identity("test.operation-aware-host", "v1", &[]),
            Arc::new(OperationAwareHost {
                operations: Arc::clone(&operations),
            }),
            [TextMeasurementPhase::SvgBBox, TextMeasurementPhase::Layout],
            deterministic_profile(),
        );
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(policy)
            .begin_session()
            .expect("begin render session");
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);

        assert_eq!(
            measurer
                .measure_mermaid_calculate_text_dimensions("operation", &TextStyle::default())
                .width,
            82.5,
        );
        assert_eq!(
            measurer.measure_canvas_text_width_px("operation", &TextStyle::default()),
            94.25
        );
        assert_eq!(
            *operations.lock().expect("operation probe lock"),
            [
                TextMeasurementOperation::MermaidCalculateTextDimensions,
                TextMeasurementOperation::CanvasMeasureTextWidth,
            ]
        );
    }

    fn identity(
        profile: &str,
        version: &str,
        decorators: &[&str],
    ) -> TextMeasurementProfileIdentity {
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new(profile).expect("valid test profile"),
            version,
        )
        .expect("valid test version")
        .with_decorators(decorators.iter().copied())
        .expect("valid test decorators")
    }

    fn metrics(width: f64) -> TextMetrics {
        TextMetrics {
            width,
            height: width + 1.0,
            line_count: 1,
        }
    }

    #[derive(Debug, Default)]
    struct SpecializedProfile;

    impl TextMeasurer for SpecializedProfile {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            metrics(1.0)
        }

        fn measure_svg_text_computed_length_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            2.0
        }

        fn measure_svg_text_bbox_x(&self, _text: &str, _style: &TextStyle) -> (f64, f64) {
            (3.0, 4.0)
        }

        fn measure_svg_text_bbox_x_with_ascii_overhang(
            &self,
            _text: &str,
            _style: &TextStyle,
        ) -> (f64, f64) {
            (5.0, 6.0)
        }

        fn measure_svg_title_bbox_x(&self, _text: &str, _style: &TextStyle) -> (f64, f64) {
            (7.0, 8.0)
        }

        fn measure_svg_simple_text_bbox_width_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            9.0
        }

        fn measure_svg_raw_text_bbox_width_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            10.0
        }

        fn measure_svg_tspan_text_bbox_width_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            10.5
        }

        fn measure_svg_tspan_text_bbox_height_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            11.5
        }

        fn measure_svg_create_text_bbox_y_offset_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            -1.25
        }

        fn measure_svg_create_text_middle_bbox_y_offset_px(
            &self,
            _text: &str,
            _style: &TextStyle,
        ) -> f64 {
            -2.5
        }

        fn measure_svg_simple_text_bbox_width_for_wrap_px(
            &self,
            _text: &str,
            _style: &TextStyle,
        ) -> f64 {
            11.0
        }

        fn measure_mermaid_calculate_text_dimensions(
            &self,
            _text: &str,
            _style: &TextStyle,
        ) -> TextMetrics {
            metrics(11.25)
        }

        fn measure_canvas_text_width_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            11.75
        }

        fn measure_svg_simple_text_bbox_height_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            12.0
        }

        fn measure_wrapped(
            &self,
            _text: &str,
            _style: &TextStyle,
            _max_width: Option<f64>,
            _wrap_mode: WrapMode,
        ) -> TextMetrics {
            metrics(13.0)
        }

        fn measure_wrapped_with_raw_width(
            &self,
            _text: &str,
            _style: &TextStyle,
            _max_width: Option<f64>,
            _wrap_mode: WrapMode,
        ) -> (TextMetrics, Option<f64>) {
            (metrics(14.0), Some(15.0))
        }
    }

    struct DecliningHost;

    impl HostTextMeasurer for DecliningHost {
        fn measure(&self, _request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            Ok(None)
        }
    }

    struct ForgedCarrierProfile;

    impl TextMeasurer for ForgedCarrierProfile {
        #[allow(private_interfaces)]
        fn builtin_operation_carrier(
            &self,
            operation: TextMeasurementOperation,
        ) -> Option<BuiltinTextMeasurementOperationCarrier> {
            Some(BuiltinTextMeasurementOperationCarrier {
                profile: BuiltinTextMeasurementProfile::Deterministic,
                phase: TextMeasurementPhase::SvgBBox,
                operation,
            })
        }

        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            metrics(1.0)
        }
    }

    #[test]
    fn private_operation_carriers_only_qualify_builtin_profile_routes() {
        assert!(
            BuiltinTextMeasurementOperationCarrier {
                profile: BuiltinTextMeasurementProfile::Deterministic,
                phase: TextMeasurementPhase::SvgBBox,
                operation: TextMeasurementOperation::WrappedWithRawWidth,
            }
            .into_inline_html()
            .is_none(),
            "the inline planner requires both the wrapped operation and its Wrap owner phase"
        );

        let builtin_session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin built-in session");
        let builtin = builtin_session.text_measurer(TextMeasurementPhase::Layout);
        let builtin_carrier = inline_html_carrier(&builtin);
        assert!(builtin_carrier.is_builtin());
        let sequence_carrier = builtin
            .builtin_operation_carrier(TextMeasurementOperation::MermaidCalculateTextDimensions)
            .expect("sequence measurement route is built-in");
        assert_eq!(sequence_carrier.phase, TextMeasurementPhase::SvgBBox);
        assert_eq!(
            sequence_carrier.operation,
            TextMeasurementOperation::MermaidCalculateTextDimensions
        );
        assert!(
            builtin
                .builtin_operation_carrier(TextMeasurementOperation::MermaidCalculateTextDimensions)
                .is_some()
        );

        let custom_profile = TextMeasurementProfile::new(
            identity("test.custom", "v1", &[]),
            Arc::new(ForgedCarrierProfile),
        );
        let custom_session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::uniform(custom_profile))
            .begin_session()
            .expect("begin custom profile session");
        let custom = custom_session.text_measurer(TextMeasurementPhase::Layout);
        assert!(!inline_html_carrier(&custom).is_builtin());
        assert!(
            custom
                .builtin_operation_carrier(TextMeasurementOperation::MermaidCalculateTextDimensions)
                .is_none()
        );

        let host_policy = TextMeasurementPolicy::host_display(
            identity("test.host", "v1", &[]),
            Arc::new(DecliningHost),
            [TextMeasurementPhase::Wrap, TextMeasurementPhase::SvgBBox],
        );
        let host_session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(host_policy)
            .begin_session()
            .expect("begin host session");
        let host = host_session.text_measurer(TextMeasurementPhase::Layout);
        assert!(
            !inline_html_carrier(&host).is_builtin(),
            "host routes remain opaque even when their fallback is deterministic"
        );
        assert!(
            host.builtin_operation_carrier(
                TextMeasurementOperation::MermaidCalculateTextDimensions
            )
            .is_none()
        );
    }

    #[test]
    fn builtin_inline_stream_matches_backend_order_and_supported_br_normalization() {
        fn assert_cases(
            backend: &dyn TextMeasurer,
            carrier: InlineHtmlMeasurementCarrier,
            style: &TextStyle,
            cases: &[(&str, &[&str])],
        ) {
            for (text, chunks) in cases {
                assert_eq!(chunks.concat(), *text);
                let expected = backend
                    .measure_wrapped(text, style, None, WrapMode::HtmlLike)
                    .width;
                let mut streamed = carrier
                    .begin_inline_html_width(style)
                    .expect("built-in carrier starts a streaming width");
                for chunk in *chunks {
                    streamed.push_text(chunk);
                }
                assert_eq!(
                    streamed.width_px().to_bits(),
                    expected.to_bits(),
                    "text={text:?}, chunks={chunks:?}"
                );
            }
        }

        let cases: &[(&str, &[&str])] = &[
            ("AVATAR office", &["A", "VAT", "AR ", "office"]),
            (
                "alpha<br   />omega",
                &["alpha<", "b", "r ", "  /", ">", "omega"],
            ),
            ("<b<br/>tail", &["<b<", "br", "/>tail"]),
            ("wide<br / >literal", &["wide<br ", "/ ", ">literal"]),
            // ECMAScript `\s` also includes form feed. The headless built-in intentionally keeps
            // that spelling literal until browser-grade HTML parsing is available.
            (
                "wide<br\u{000C}/>literal",
                &["wide<br", "\u{000C}", "/>literal"],
            ),
            (
                "wide\n                         ",
                &["wide\n", "             ", "            "],
            ),
            (" \n  ", &[" ", "\n", "  "]),
            ("i\n\u{00a0}", &["i\n", "\u{00a0}"]),
            ("A\u{301}👩‍💻مرحبا世界", &["A\u{301}", "👩‍💻", "مرحبا", "世界"]),
            ("👩‍🔬", &["👩", "\u{200d}", "🔬"]),
            ("👨‍👩‍👧‍👦", &["👨‍", "👩", "\u{200d}👧‍", "👦"]),
            ("👍🏽", &["👍", "🏽"]),
            ("🇨🇳", &["🇨", "🇳"]),
            ("1️⃣", &["1", "\u{fe0f}", "\u{20e3}"]),
            ("😀\u{fe0e}", &["😀", "\u{fe0e}"]),
            ("k中A+", &["k中", "A+"]),
        ];
        let style = TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: Some("700".to_string()),
            font_style: Some("italic".to_string()),
        };

        let deterministic_session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin deterministic session");
        let deterministic_carrier =
            inline_html_carrier(&deterministic_session.text_measurer(TextMeasurementPhase::Wrap));
        assert_cases(
            &DeterministicTextMeasurer::default(),
            deterministic_carrier,
            &style,
            cases,
        );
        let long_whitespace = " ".repeat(8_192);
        let long_break = format!("wide<br{long_whitespace}/>tail");
        let expected = DeterministicTextMeasurer::default()
            .measure_wrapped(&long_break, &style, None, WrapMode::HtmlLike)
            .width;
        let mut streamed = deterministic_carrier
            .begin_inline_html_width(&style)
            .expect("deterministic carrier starts a streaming width");
        streamed.push_text("wide<br");
        streamed.push_text(&long_whitespace);
        streamed.push_text("/>tail");
        assert_eq!(streamed.width_px().to_bits(), expected.to_bits());

        let mut unknown_font = style.clone();
        unknown_font.font_family = Some("fixture-private-font".to_string());
        assert_cases(
            &DeterministicTextMeasurer::default(),
            deterministic_carrier,
            &unknown_font,
            cases,
        );

        let combining_tail = format!("A{}", "\u{0301}".repeat(1_024));
        assert_cases(
            &DeterministicTextMeasurer::default(),
            deterministic_carrier,
            &style,
            &[(combining_tail.as_str(), &[combining_tail.as_str()])],
        );
    }

    #[test]
    fn builtin_inline_literal_lt_combining_work_is_linear_and_cached() {
        let style = TextStyle {
            font_size: 16.0,
            ..TextStyle::default()
        };
        let carrier =
            InlineHtmlMeasurementCarrier::builtin(BuiltinTextMeasurementProfile::Deterministic);

        for combining_scalars in [1_024, 2_048, 4_096] {
            let text = format!("<{}", "\u{0301}".repeat(combining_scalars));
            let expected = DeterministicTextMeasurer::default()
                .measure_wrapped(&text, &style, None, WrapMode::HtmlLike)
                .width;
            let mut streamed = carrier
                .begin_inline_html_width(&style)
                .expect("built-in carrier starts a streaming width");

            streamed.push_text(&text);
            assert_eq!(
                streamed.grapheme_input_byte_charge(),
                0,
                "appending input must not rescan a growing pending grapheme"
            );
            assert_eq!(streamed.width_px().to_bits(), expected.to_bits());
            assert_eq!(
                streamed.grapheme_input_byte_charge(),
                text.len() * 2,
                "the owning checkpoint admits two linear input-byte charges"
            );

            let input_byte_charge = streamed.grapheme_input_byte_charge();
            assert_eq!(streamed.width_px().to_bits(), expected.to_bits());
            assert_eq!(
                streamed.grapheme_input_byte_charge(),
                input_byte_charge,
                "a repeated checkpoint without new input must use the cached width"
            );
        }
    }

    #[test]
    fn builtin_inline_repeated_literal_lt_speculation_is_linear() {
        let style = TextStyle {
            font_size: 16.0,
            ..TextStyle::default()
        };
        let carrier =
            InlineHtmlMeasurementCarrier::builtin(BuiltinTextMeasurementProfile::Deterministic);

        for scalar_count in [1_024, 2_048, 4_096] {
            let text = "<".repeat(scalar_count);
            let expected = DeterministicTextMeasurer::default()
                .measure_wrapped(&text, &style, None, WrapMode::HtmlLike)
                .width;
            let mut streamed = carrier
                .begin_inline_html_width(&style)
                .expect("built-in carrier starts a streaming width");

            streamed.push_text(&text);
            assert_eq!(streamed.width_px().to_bits(), expected.to_bits());
            assert!(
                streamed.speculative_clone_bytes() <= text.len(),
                "each input byte may enter at most one retained-grapheme branch clone"
            );
            assert!(
                streamed.grapheme_input_byte_charge() <= text.len() * 4,
                "branch compaction and the final checkpoint must remain linear"
            );
        }
    }

    #[test]
    fn builtin_inline_speculative_clone_work_survives_checkpoint_rollback() {
        let style = TextStyle {
            font_size: 16.0,
            ..TextStyle::default()
        };
        let carrier =
            InlineHtmlMeasurementCarrier::builtin(BuiltinTextMeasurementProfile::Deterministic);
        let mut streamed = carrier
            .begin_inline_html_width(&style)
            .expect("built-in carrier starts a streaming width");

        streamed.push_text("A");
        let checkpoint = streamed.clone();
        let mut candidate = checkpoint.clone();
        candidate.push_text("<not-a-break");

        assert!(candidate.speculative_clone_bytes() > 0);
        assert_eq!(
            streamed.speculative_clone_bytes(),
            candidate.speculative_clone_bytes(),
            "discarded checkpoint branches still count the clone work they performed"
        );

        streamed = checkpoint;
        assert_eq!(
            streamed.speculative_clone_bytes(),
            candidate.speculative_clone_bytes(),
            "restoring a checkpoint must not roll back operation-owned work accounting"
        );
    }

    #[test]
    fn builtin_inline_incomplete_br_whitespace_keeps_a_bounded_frontier() {
        let style = TextStyle {
            font_size: 16.0,
            ..TextStyle::default()
        };
        let carrier =
            InlineHtmlMeasurementCarrier::builtin(BuiltinTextMeasurementProfile::Deterministic);

        for whitespace_repetitions in [256, 512, 1_024] {
            let text = format!("<br{}", " \t\r\n".repeat(whitespace_repetitions));
            let expected = DeterministicTextMeasurer::default()
                .measure_wrapped(&text, &style, None, WrapMode::HtmlLike)
                .width;
            let mut streamed = carrier
                .begin_inline_html_width(&style)
                .expect("built-in carrier starts a streaming width");

            streamed.push_text(&text);
            assert_eq!(streamed.width_px().to_bits(), expected.to_bits());
            assert!(streamed.retained_grapheme_bytes() <= 1);
            assert_eq!(streamed.speculative_clone_bytes(), 0);
            assert!(streamed.grapheme_input_byte_charge() <= text.len() * 4);
        }
    }

    #[test]
    fn builtin_svg_computed_length_stream_is_sequence_aware_and_reversible() {
        let backend = DeterministicTextMeasurer::default();
        let style = TextStyle {
            font_size: 16.0,
            ..TextStyle::default()
        };
        let cases: &[(&str, &[&str])] = &[
            ("👩‍🔬", &["👩", "\u{200d}", "🔬"]),
            ("👨‍👩‍👧‍👦", &["👨", "\u{200d}👩‍", "👧", "\u{200d}👦"]),
            ("👍🏽", &["👍", "🏽"]),
            ("🇨🇳", &["🇨", "🇳"]),
            ("1️⃣", &["1", "\u{fe0f}", "\u{20e3}"]),
            ("😀\u{fe0e}", &["😀", "\u{fe0e}"]),
            ("k中A+", &["k中", "A+"]),
        ];

        for (text, chunks) in cases {
            let mut streamed = BuiltinSvgComputedLength::deterministic(&style);
            let mut prefix = String::new();
            for chunk in *chunks {
                prefix.push_str(chunk);
                streamed.push_text(chunk);
                let expected = backend.measure_svg_text_computed_length_px(&prefix, &style);
                assert_eq!(
                    streamed.width_px().to_bits(),
                    expected.to_bits(),
                    "text={text:?}, prefix={prefix:?}"
                );
            }

            let checkpoint = streamed.clone();
            let expected = backend.measure_svg_text_computed_length_px(text, &style);
            assert_eq!(checkpoint.width_px().to_bits(), expected.to_bits());
            streamed.push_text("A");
            assert_ne!(
                streamed.width_px().to_bits(),
                checkpoint.width_px().to_bits()
            );
            streamed = checkpoint;
            assert_eq!(streamed.width_px().to_bits(), expected.to_bits());
            streamed.reset();
            assert_eq!(streamed.width_px(), 0.0);
        }
    }

    #[test]
    fn named_complete_profile_preserves_every_specialized_method_and_identity() {
        let profile_identity = identity(
            "test.specialized",
            "v3",
            &["fixture-map@v2", "host-adjustment@v1"],
        );
        let profile =
            TextMeasurementProfile::new(profile_identity.clone(), Arc::new(SpecializedProfile));
        let environment = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
        let session = environment.begin_session().expect("begin render session");
        let measurer = session.text_measurer(TextMeasurementPhase::SvgBBox);
        let style = TextStyle::default();

        assert_eq!(measurer.measure("x", &style).width, 1.0);
        assert_eq!(
            measurer.measure_svg_text_computed_length_px("x", &style),
            2.0
        );
        assert_eq!(measurer.measure_svg_text_bbox_x("x", &style), (3.0, 4.0));
        assert_eq!(
            measurer.measure_svg_text_bbox_x_with_ascii_overhang("x", &style),
            (5.0, 6.0)
        );
        assert_eq!(measurer.measure_svg_title_bbox_x("x", &style), (7.0, 8.0));
        assert_eq!(
            measurer.measure_svg_simple_text_bbox_width_px("x", &style),
            9.0
        );
        assert_eq!(
            measurer.measure_svg_raw_text_bbox_width_px("x", &style),
            10.0
        );
        assert_eq!(
            measurer.measure_svg_tspan_text_bbox_width_px("x", &style),
            10.5
        );
        assert_eq!(
            measurer.measure_svg_tspan_text_bbox_height_px("x", &style),
            11.5
        );
        assert_eq!(
            measurer.measure_svg_create_text_bbox_y_offset_px("x", &style),
            -1.25
        );
        assert_eq!(
            measurer.measure_svg_create_text_middle_bbox_y_offset_px("x", &style),
            -2.5
        );
        assert_eq!(
            measurer.measure_svg_simple_text_bbox_width_for_wrap_px("x", &style),
            11.0
        );
        assert_eq!(
            measurer
                .measure_mermaid_calculate_text_dimensions("x", &style)
                .width,
            11.25
        );
        assert_eq!(measurer.measure_canvas_text_width_px("x", &style), 11.75);
        assert_eq!(
            measurer.measure_svg_simple_text_bbox_height_px("x", &style),
            12.0
        );
        assert_eq!(
            measurer
                .measure_wrapped("x", &style, Some(10.0), WrapMode::HtmlLike)
                .width,
            13.0
        );
        assert_eq!(
            measurer
                .measure_wrapped_with_raw_width("x", &style, Some(10.0), WrapMode::HtmlLike,)
                .1,
            Some(15.0)
        );
        let route = session.text_measurement_route(TextMeasurementPhase::SvgBBox);
        assert_eq!(route.primary, profile_identity);
        assert_eq!(session.text_measurement_report().entries().len(), 17);
    }

    #[test]
    fn repeated_measurements_are_aggregated_into_one_bounded_summary() {
        let policy = TextMeasurementPolicy::deterministic();
        let recorder = TextMeasurementRecorder::default();

        for _ in 0..10_000 {
            recorder.record(
                TextMeasurementPhase::Layout,
                TextMeasurementOperation::Measure,
                TextMeasurementRouteOutcome::Profile,
            );
        }

        let report = recorder.report(&policy);
        assert_eq!(report.entries().len(), 1);
        assert_eq!(report.entries()[0].count(), 10_000);
        assert_eq!(
            report.entries()[0].provenance().operation,
            TextMeasurementOperation::Measure
        );
        assert_eq!(
            report.entries()[0].provenance().phase,
            TextMeasurementPhase::Layout
        );
    }

    #[derive(Clone)]
    enum HostOutcome {
        Measured(TextMetrics),
        Length(f64),
        Missing,
        Invalid,
        Error,
    }

    struct CountingHost {
        calls: Arc<AtomicUsize>,
        outcome: HostOutcome,
    }

    impl HostTextMeasurer for CountingHost {
        fn measure(&self, _request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            self.calls.fetch_add(1, Ordering::Relaxed);
            match self.outcome {
                HostOutcome::Measured(metrics) => Ok(Some(HostTextMeasurement::Metrics(metrics))),
                HostOutcome::Length(length) => Ok(Some(HostTextMeasurement::Length(length))),
                HostOutcome::Missing => Ok(None),
                HostOutcome::Invalid => Err(HostTextMeasurementError::invalid_value(
                    "invalid host value",
                )),
                HostOutcome::Error => Err(HostTextMeasurementError::new("host failed")),
            }
        }
    }

    struct CountingFallback(Arc<AtomicUsize>);

    impl TextMeasurer for CountingFallback {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            self.0.fetch_add(1, Ordering::Relaxed);
            metrics(41.0)
        }

        fn measure_svg_text_computed_length_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            self.0.fetch_add(1, Ordering::Relaxed);
            42.0
        }
    }

    fn host_policy(
        outcome: HostOutcome,
        host_calls: &Arc<AtomicUsize>,
        fallback_calls: &Arc<AtomicUsize>,
    ) -> TextMeasurementPolicy {
        let fallback = TextMeasurementProfile::new(
            identity("test.fallback", "v1", &[]),
            Arc::new(CountingFallback(Arc::clone(fallback_calls))),
        );
        TextMeasurementPolicy::host_display_with_fallback(
            identity("test.host", "v2", &["browser@stable"]),
            Arc::new(CountingHost {
                calls: Arc::clone(host_calls),
                outcome,
            }),
            [
                TextMeasurementPhase::Layout,
                TextMeasurementPhase::ComputedLength,
            ],
            fallback,
        )
    }

    struct CancellingMissingHost {
        calls: Arc<AtomicUsize>,
        control: OperationControl,
    }

    impl HostTextMeasurer for CancellingMissingHost {
        fn measure(&self, _request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.control.cancel();
            Ok(None)
        }
    }

    #[test]
    fn host_cancellation_skips_fallback_and_future_primary_calls() {
        let control = OperationControl::new();
        let host_calls = Arc::new(AtomicUsize::new(0));
        let fallback_calls = Arc::new(AtomicUsize::new(0));
        let fallback = TextMeasurementProfile::new(
            identity("test.cancelled-fallback", "v1", &[]),
            Arc::new(CountingFallback(Arc::clone(&fallback_calls))),
        );
        let policy = TextMeasurementPolicy::host_display_with_fallback(
            identity("test.cancelling-host", "v1", &[]),
            Arc::new(CancellingMissingHost {
                calls: Arc::clone(&host_calls),
                control: control.clone(),
            }),
            [TextMeasurementPhase::Layout],
            fallback,
        );
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(policy)
            .begin_session_with_control(control)
            .expect("begin controlled render session");

        let measurer =
            session.controlled_text_measurer(TextMeasurementPhase::Layout, OperationPhase::Layout);
        let measured = measurer.measure("label", &TextStyle::default());
        let measured_after_cancellation = measurer.measure("second label", &TextStyle::default());

        assert_eq!(measured.width, 0.0);
        assert_eq!(measured.height, 0.0);
        assert_eq!(measured.line_count, 1);
        assert_eq!(measured_after_cancellation.width, 0.0);
        assert_eq!(measured_after_cancellation.height, 0.0);
        assert_eq!(measured_after_cancellation.line_count, 1);
        assert_eq!(host_calls.load(Ordering::Relaxed), 1);
        assert_eq!(fallback_calls.load(Ordering::Relaxed), 0);
        assert!(matches!(
            session.checkpoint(OperationPhase::Layout),
            Err(crate::Error::Cancelled(_))
        ));
    }

    #[test]
    fn host_success_and_each_fallback_reason_are_recorded_from_actual_calls() {
        let scenarios = [
            (HostOutcome::Measured(metrics(73.0)), None, 73.0),
            (
                HostOutcome::Length(73.0),
                Some(HostFallbackReason::Invalid),
                41.0,
            ),
            (
                HostOutcome::Missing,
                Some(HostFallbackReason::Missing),
                41.0,
            ),
            (
                HostOutcome::Measured(TextMetrics {
                    width: f64::NAN,
                    height: 10.0,
                    line_count: 1,
                }),
                Some(HostFallbackReason::Invalid),
                41.0,
            ),
            (
                HostOutcome::Invalid,
                Some(HostFallbackReason::Invalid),
                41.0,
            ),
            (HostOutcome::Error, Some(HostFallbackReason::Error), 41.0),
        ];

        for (outcome, expected_reason, expected_width) in scenarios {
            let host_calls = Arc::new(AtomicUsize::new(0));
            let fallback_calls = Arc::new(AtomicUsize::new(0));
            let environment = RenderEnvironment::deterministic()
                .with_text_measurement_policy(host_policy(outcome, &host_calls, &fallback_calls));
            let session = environment.begin_session().expect("begin render session");
            let measured = session
                .text_measurer(TextMeasurementPhase::Layout)
                .measure("label", &TextStyle::default());

            assert_eq!(measured.width, expected_width);
            assert_eq!(host_calls.load(Ordering::Relaxed), 1);
            assert_eq!(
                fallback_calls.load(Ordering::Relaxed),
                usize::from(expected_reason.is_some())
            );
            let report = session.text_measurement_report();
            assert_eq!(report.entries().len(), 1);
            assert_eq!(
                report.entries()[0].provenance().fallback_reason,
                expected_reason
            );
        }
    }

    struct StyleCapturingHost {
        observed_font_style: Arc<Mutex<Option<String>>>,
    }

    impl HostTextMeasurer for StyleCapturingHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            if request.operation != TextMeasurementOperation::Wrapped {
                return Ok(None);
            }
            *self
                .observed_font_style
                .lock()
                .expect("font style probe lock") = request.style.font_style.clone();
            Ok(Some(HostTextMeasurement::Metrics(metrics(73.25))))
        }
    }

    #[test]
    fn host_success_receives_italic_style_without_fallback_adjustment() {
        let observed_font_style = Arc::new(Mutex::new(None));
        let fallback_calls = Arc::new(AtomicUsize::new(0));
        let fallback = TextMeasurementProfile::new(
            identity("test.italic-fallback", "v1", &[]),
            Arc::new(CountingFallback(Arc::clone(&fallback_calls))),
        );
        let policy = TextMeasurementPolicy::host_display_with_fallback(
            identity("test.italic-host", "v1", &[]),
            Arc::new(StyleCapturingHost {
                observed_font_style: Arc::clone(&observed_font_style),
            }),
            [TextMeasurementPhase::Wrap],
            fallback,
        );
        let environment = RenderEnvironment::deterministic().with_text_measurement_policy(policy);
        let session = environment.begin_session().expect("begin render session");
        let style = TextStyle {
            font_style: Some("italic".to_string()),
            ..TextStyle::default()
        };

        let measured = session
            .text_measurer(TextMeasurementPhase::Layout)
            .measure_wrapped("italic label", &style, Some(200.0), WrapMode::HtmlLike);

        assert_eq!(measured.width, 73.25);
        assert_eq!(
            observed_font_style
                .lock()
                .expect("font style probe lock")
                .as_deref(),
            Some("italic")
        );
        assert_eq!(fallback_calls.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn operation_specific_length_is_authoritative_and_invalid_lengths_fallback() {
        let host_calls = Arc::new(AtomicUsize::new(0));
        let fallback_calls = Arc::new(AtomicUsize::new(0));
        let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
            host_policy(HostOutcome::Length(73.0), &host_calls, &fallback_calls),
        );
        let session = environment.begin_session().expect("begin render session");

        assert_eq!(
            session
                .text_measurer(TextMeasurementPhase::Layout)
                .measure_svg_text_computed_length_px("label", &TextStyle::default()),
            73.0
        );
        assert_eq!(host_calls.load(Ordering::Relaxed), 1);
        assert_eq!(fallback_calls.load(Ordering::Relaxed), 0);
        let report = session.text_measurement_report();
        assert_eq!(
            report.entries()[0].provenance().operation,
            TextMeasurementOperation::ComputedLength
        );
        assert_eq!(
            report.entries()[0].provenance().source,
            TextMeasurementSource::Host
        );
        assert_eq!(report.entries()[0].provenance().fallback_reason, None);

        for invalid_length in [-1.0, f64::NAN] {
            let host_calls = Arc::new(AtomicUsize::new(0));
            let fallback_calls = Arc::new(AtomicUsize::new(0));
            let environment =
                RenderEnvironment::deterministic().with_text_measurement_policy(host_policy(
                    HostOutcome::Length(invalid_length),
                    &host_calls,
                    &fallback_calls,
                ));
            let session = environment.begin_session().expect("render session");

            assert_eq!(
                session
                    .text_measurer(TextMeasurementPhase::ComputedLength)
                    .measure_svg_text_computed_length_px("label", &TextStyle::default()),
                42.0
            );
            assert_eq!(host_calls.load(Ordering::Relaxed), 1);
            assert_eq!(fallback_calls.load(Ordering::Relaxed), 1);
            assert_eq!(
                session.text_measurement_report().entries()[0]
                    .provenance()
                    .fallback_reason,
                Some(HostFallbackReason::Invalid)
            );
        }
    }

    struct ExtentHost {
        calls: Arc<AtomicUsize>,
        value: (f64, f64),
    }

    impl HostTextMeasurer for ExtentHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            if request.operation != TextMeasurementOperation::BBoxX {
                return Ok(None);
            }
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(Some(HostTextMeasurement::HorizontalExtents {
                left: self.value.0,
                right: self.value.1,
            }))
        }
    }

    struct ExtentFallback(Arc<AtomicUsize>);

    impl TextMeasurer for ExtentFallback {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            metrics(1.0)
        }

        fn measure_svg_text_bbox_x(&self, _text: &str, _style: &TextStyle) -> (f64, f64) {
            self.0.fetch_add(1, Ordering::Relaxed);
            (3.0, 4.0)
        }
    }

    #[test]
    fn host_bbox_accepts_non_negative_extents_and_rejects_invalid_values() {
        for (host_value, expected, expected_source, expected_reason) in [
            ((1.5, 12.0), (1.5, 12.0), TextMeasurementSource::Host, None),
            (
                (-1.5, 12.0),
                (3.0, 4.0),
                TextMeasurementSource::Profile,
                Some(HostFallbackReason::Invalid),
            ),
            (
                (12.0, -1.5),
                (3.0, 4.0),
                TextMeasurementSource::Profile,
                Some(HostFallbackReason::Invalid),
            ),
            (
                (f64::NAN, 12.0),
                (3.0, 4.0),
                TextMeasurementSource::Profile,
                Some(HostFallbackReason::Invalid),
            ),
        ] {
            let host_calls = Arc::new(AtomicUsize::new(0));
            let fallback_calls = Arc::new(AtomicUsize::new(0));
            let fallback = TextMeasurementProfile::new(
                identity("test.extent-fallback", "v1", &[]),
                Arc::new(ExtentFallback(Arc::clone(&fallback_calls))),
            );
            let policy = TextMeasurementPolicy::host_display_with_fallback(
                identity("test.extent-host", "v1", &[]),
                Arc::new(ExtentHost {
                    calls: Arc::clone(&host_calls),
                    value: host_value,
                }),
                [TextMeasurementPhase::SvgBBox],
                fallback,
            );
            let session = RenderEnvironment::deterministic()
                .with_text_measurement_policy(policy)
                .begin_session()
                .expect("render session");

            assert_eq!(
                session
                    .text_measurer(TextMeasurementPhase::SvgBBox)
                    .measure_svg_text_bbox_x("A", &TextStyle::default()),
                expected
            );
            assert_eq!(host_calls.load(Ordering::Relaxed), 1);
            assert_eq!(
                fallback_calls.load(Ordering::Relaxed),
                usize::from(expected_reason.is_some())
            );
            let report = session.text_measurement_report();
            assert_eq!(report.entries().len(), 1);
            assert_eq!(report.entries()[0].provenance().source, expected_source);
            assert_eq!(
                report.entries()[0].provenance().fallback_reason,
                expected_reason
            );
        }
    }

    #[test]
    fn session_exposes_services_and_derives_a_nonzero_render_seed() {
        let limits = RenderResourcePolicy::trusted_native();
        let environment = RenderEnvironment::deterministic()
            .with_runtime_policy(RuntimePolicy::deterministic().with_fixed_seed(0))
            .with_math_renderer(Arc::new(crate::math::NoopMathRenderer))
            .with_icon_registry(crate::svg::IconRegistryBuilder::new().build().unwrap())
            .with_resource_policy(limits);

        let session = environment.begin_session().expect("begin render session");
        assert_eq!(session.operation_context().seed(), 0);
        assert_eq!(
            session.render_seed(),
            session
                .operation_context()
                .derive_nonzero_u64("render.root", 0)
        );
        assert_eq!(session.resource_policy(), limits);
        assert!(session.math_renderer().is_some());
        assert!(session.icon_registry().is_some());
        assert!(session.prepared_text_layout().is_none());
        assert!(session.text_layout_error().is_none());
        assert_eq!(session.report().operation_context().seed(), 0);
    }

    #[test]
    fn session_report_keeps_prepared_text_retained_peak_separate_from_layout_work() {
        let limits = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 16)
            .unwrap();
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(limits)
            .begin_session()
            .unwrap();

        let mut reservation = session
            .work_meter()
            .reserve_prepared_text_retained_bytes(12)
            .unwrap();
        reservation.reconcile_downward(7);
        let live_report = session.report();
        assert_eq!(live_report.prepared_text_retained_bytes_peak(), 12);
        assert_eq!(live_report.layout_work_units(), 0);

        drop(reservation);
        let released_report = session.report();
        assert_eq!(released_report.prepared_text_retained_bytes_peak(), 12);
        assert_eq!(released_report.layout_work_units(), 0);
    }

    #[cfg(feature = "embedded-fonts")]
    fn embedded_font_theme() -> DiagramTheme {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        let catalog =
            crate::diagram_theme::FontCatalogSpec::new([crate::diagram_theme::FontAssetSpec::new(
                "excalifont",
                bytes,
            )]);
        let spec = crate::diagram_theme::DiagramThemeSpec::new()
            .with_assets(crate::diagram_theme::ThemeAssets::default().with_font_catalog(catalog));

        crate::diagram_theme::DiagramThemeCompiler::new()
            .compile(spec)
            .expect("embedded font theme should compile")
    }

    #[cfg(feature = "embedded-fonts")]
    fn resource_rich_theme() -> DiagramTheme {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        let catalog =
            crate::diagram_theme::FontCatalogSpec::new([crate::diagram_theme::FontAssetSpec::new(
                "excalifont",
                bytes,
            )])
            .with_alias("Sketch", "Excalifont")
            .with_generic_family(
                crate::diagram_theme::GenericFontFamily::Cursive,
                "Excalifont",
            );
        let graph = crate::diagram_theme::EffectGraph::new(
            "resource-check",
            [
                crate::diagram_theme::EffectPrimitive::DropShadow {
                    input: crate::diagram_theme::EffectInput::SourceGraphic,
                    offset_x: 6.0,
                    offset_y: -5.0,
                    blur_radius: 4.0,
                    spread: 2.0,
                    color: crate::diagram_theme::ThemeColorValue::parse("#00000080")
                        .expect("valid shadow color"),
                },
                crate::diagram_theme::EffectPrimitive::GaussianBlur {
                    input: crate::diagram_theme::EffectInput::SourceGraphic,
                    std_deviation: 7.0,
                },
                crate::diagram_theme::EffectPrimitive::Turbulence {
                    input: crate::diagram_theme::EffectInput::SourceGraphic,
                    base_frequency_x: 0.02,
                    base_frequency_y: 0.03,
                    octaves: 3,
                    seed: 7,
                },
                crate::diagram_theme::EffectPrimitive::Displacement {
                    input: crate::diagram_theme::EffectInput::SourceGraphic,
                    map_input: crate::diagram_theme::EffectInput::SourceGraphic,
                    scale: 9.0,
                },
            ],
        )
        .expect("valid resource-check effect graph");
        let effects = crate::diagram_theme::DiagramEffectSet::default()
            .with_graph(graph)
            .expect("unique effect graph")
            .with_binding(
                crate::diagram_theme::EffectBinding::new(
                    crate::diagram_theme::ThemeTarget::Node,
                    "resource-check",
                )
                .expect("valid effect binding"),
            )
            .expect("unique effect binding");
        let spec = crate::diagram_theme::DiagramThemeSpec::new()
            .with_assets(crate::diagram_theme::ThemeAssets::default().with_font_catalog(catalog))
            .with_effects(effects);

        crate::diagram_theme::DiagramThemeCompiler::new()
            .with_resource_policy(
                crate::diagram_theme::ThemeResourcePolicy::unbounded_for_trusted_input(),
            )
            .compile(spec)
            .expect("resource-rich font theme should compile under the wide policy")
    }

    #[derive(Default)]
    #[cfg(feature = "embedded-fonts")]
    struct PrepareMustNotRunBackend {
        native: crate::text::NativeTextLayoutBackend,
    }

    #[cfg(feature = "embedded-fonts")]
    impl crate::text::TextLayoutBackend for PrepareMustNotRunBackend {
        fn identity(&self) -> &crate::text::TextLayoutBackendIdentity {
            self.native.identity()
        }

        fn capabilities(&self) -> crate::text::TextLayoutCapabilities {
            self.native.capabilities()
        }

        fn prepare_catalog(
            &self,
            _request: &crate::text::PrepareCatalogRequest,
        ) -> Result<crate::text::PreparedTextLayoutResponse, crate::text::TextLayoutError> {
            panic!("retained resources must be rejected before text preparation")
        }
    }

    #[cfg(feature = "embedded-fonts")]
    struct CachedPreparedBackend {
        identity: crate::text::TextLayoutBackendIdentity,
        capabilities: crate::text::TextLayoutCapabilities,
        response: crate::text::PreparedTextLayoutResponse,
    }

    #[cfg(feature = "embedded-fonts")]
    impl crate::text::TextLayoutBackend for CachedPreparedBackend {
        fn identity(&self) -> &crate::text::TextLayoutBackendIdentity {
            &self.identity
        }

        fn capabilities(&self) -> crate::text::TextLayoutCapabilities {
            self.capabilities
        }

        fn prepare_catalog(
            &self,
            _request: &crate::text::PrepareCatalogRequest,
        ) -> Result<crate::text::PreparedTextLayoutResponse, crate::text::TextLayoutError> {
            Ok(self.response.clone())
        }
    }

    #[test]
    #[cfg(feature = "embedded-fonts")]
    fn themed_session_rejects_a_font_policy_without_an_allowed_intersection() {
        let environment_theme = embedded_font_theme();
        let theme = crate::diagram_theme::DiagramThemeCompiler::new()
            .compile_preset(crate::diagram_theme::ThemePreset::OneDark)
            .expect("one-dark theme should compile");
        let environment = RenderEnvironment::deterministic()
            .with_font_catalog(environment_theme.font_catalog().clone())
            .with_font_source_policy(FontSourcePolicy::embedded_only());

        let error = match environment.begin_session_with_theme(&theme) {
            Ok(_) => panic!("embedded-only hosts must not silently re-enable system fonts"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            RenderEnvironmentError::ThemeAdmission(
                crate::diagram_theme::ThemeAdmissionError::EmptyFontSourceIntersection,
            )
        );
    }

    #[test]
    #[cfg(feature = "embedded-fonts")]
    fn themed_session_intersects_environment_and_compiled_theme_policies() {
        let theme = embedded_font_theme();
        let environment = RenderEnvironment::deterministic()
            .with_font_source_policy(FontSourcePolicy::system_then_embedded());

        let session = environment
            .begin_session_with_theme(&theme)
            .expect("embedded source remains available to the themed session");
        assert_eq!(
            session.theme_recipe_fingerprint(),
            Some(theme.recipe_fingerprint())
        );
        assert_eq!(session.theme_recipe_report(), Some(theme.report()));
        assert_eq!(
            session.font_catalog().fingerprint(),
            theme.font_catalog().fingerprint()
        );
        assert_eq!(
            session.font_source_policy().priority().collect::<Vec<_>>(),
            vec![crate::diagram_theme::FontSource::Embedded]
        );
        assert!(session.prepared_text_layout().is_some());
        assert!(session.text_layout_error().is_none());

        let report = session.report();
        assert_eq!(
            report.theme_recipe_fingerprint(),
            Some(theme.recipe_fingerprint())
        );
        assert_eq!(report.theme_recipe_report(), Some(theme.report()));
        assert_eq!(
            report.font_catalog_fingerprint(),
            theme.font_catalog().fingerprint()
        );
        assert_eq!(
            report.font_source_policy().priority().collect::<Vec<_>>(),
            vec![crate::diagram_theme::FontSource::Embedded]
        );
        assert!(report.used_trusted_theme_lanes().allowed().next().is_none());
    }

    #[test]
    fn deterministic_session_rejects_unauthorized_trusted_theme_lane() {
        let session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("deterministic session should start");
        let error = session
            .use_trusted_theme_lane(crate::diagram_theme::TrustedThemeLane::RawThemeCss)
            .expect_err("low-level deterministic sessions must deny raw CSS by default");
        assert_eq!(
            error,
            crate::diagram_theme::ThemeAdmissionError::TrustedThemeLaneDenied(
                crate::diagram_theme::TrustedThemeLane::RawThemeCss
            )
        );
    }

    #[test]
    fn themed_session_revalidates_requirements_against_environment_admission() {
        let fill = crate::diagram_theme::CanvasPaint::solid("#111827").expect("valid fill");
        let theme = crate::diagram_theme::DiagramThemeCompiler::new()
            .compile(crate::diagram_theme::DiagramThemeSpec::new().with_styles(
                crate::diagram_theme::ThemeRuleSet::default().with_rule(
                    crate::diagram_theme::ThemeRule::new(
                        crate::diagram_theme::ThemeTarget::Node,
                        crate::diagram_theme::ThemeStylePatch::default().with_fill(fill),
                    ),
                ),
            ))
            .expect("compile theme with the permissive compiler policy");
        let environment = RenderEnvironment::deterministic().with_theme_admission_policy(
            ThemeAdmissionPolicy::permissive()
                .with_allowed_capabilities([crate::diagram_theme::ThemeCapability::SemanticTokens]),
        );

        let error = match environment.begin_session_with_theme(&theme) {
            Ok(_) => panic!("the environment must revalidate inferred theme requirements"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            RenderEnvironmentError::ThemeAdmission(
                crate::diagram_theme::ThemeAdmissionError::ThemeCapabilityDenied(
                    crate::diagram_theme::ThemeCapability::SemanticRules,
                ),
            )
        );
    }

    #[test]
    fn runtime_policy_changes_host_evidence_without_changing_recipe_identity() {
        let theme = crate::diagram_theme::DiagramThemeCompiler::new()
            .compile(crate::diagram_theme::DiagramThemeSpec::new())
            .expect("compile empty recipe");
        let best_effort = RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort themed session")
            .report();
        let portable = RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin portable themed session")
            .report();

        assert_eq!(
            best_effort.theme_recipe_fingerprint(),
            Some(theme.recipe_fingerprint())
        );
        assert_eq!(
            best_effort.theme_recipe_report(),
            portable.theme_recipe_report()
        );
        assert_ne!(
            best_effort.theme_host_admission_report(),
            portable.theme_host_admission_report()
        );
        assert_eq!(
            portable
                .theme_host_admission_report()
                .expect("portable host report")
                .portability_requirement(),
            ThemePortabilityRequirement::RequirePortable
        );
    }

    #[test]
    fn unthemed_report_freezes_the_host_portability_requirement() {
        let report = RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session()
            .expect("begin unthemed portable session")
            .report();

        assert_eq!(
            report.portability_requirement(),
            ThemePortabilityRequirement::RequirePortable
        );
        assert!(report.theme_host_admission_report().is_none());
    }

    #[test]
    fn host_admission_report_matches_the_recipe_requirements_it_allowed() {
        let fill = crate::diagram_theme::CanvasPaint::solid("#111827").expect("valid fill");
        let theme = crate::diagram_theme::DiagramThemeCompiler::new()
            .compile(crate::diagram_theme::DiagramThemeSpec::new().with_styles(
                crate::diagram_theme::ThemeRuleSet::default().with_rule(
                    crate::diagram_theme::ThemeRule::new(
                        crate::diagram_theme::ThemeTarget::Node,
                        crate::diagram_theme::ThemeStylePatch::default().with_fill(fill),
                    ),
                ),
            ))
            .expect("compile semantic recipe");
        let session = RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("default host allows semantic recipe");
        let recipe = session.theme_recipe_report().expect("recipe report");
        let host = session
            .theme_host_admission_report()
            .expect("host admission report");

        assert_eq!(
            recipe.required_capabilities().collect::<Vec<_>>(),
            host.host_allowed_capabilities().collect::<Vec<_>>()
        );
        assert_eq!(
            recipe.required_text_capabilities().collect::<Vec<_>>(),
            host.host_allowed_text_capabilities().collect::<Vec<_>>()
        );
    }

    #[test]
    #[cfg(feature = "embedded-fonts")]
    fn unthemed_session_preserves_environment_font_resources() {
        let environment_theme = embedded_font_theme();
        let catalog = environment_theme.font_catalog().clone();
        let policy = FontSourcePolicy::embedded_only();
        let environment = RenderEnvironment::deterministic()
            .with_font_catalog(catalog.clone())
            .with_font_source_policy(policy.clone());

        let session = environment.begin_session().expect("begin render session");
        assert_eq!(session.theme_recipe_fingerprint(), None);
        assert_eq!(session.theme_recipe_report(), None);
        assert_eq!(session.font_catalog().fingerprint(), catalog.fingerprint());
        assert_eq!(session.font_source_policy(), &policy);
        let prepared = session
            .prepared_text_layout()
            .expect("custom environment catalog should be prepared once");
        assert_eq!(prepared.catalog_fingerprint(), catalog.fingerprint());
        assert!(session.text_layout_error().is_none());

        let metrics = session
            .text_measurer(TextMeasurementPhase::Layout)
            .measure("prepared", &TextStyle::default());
        assert!(metrics.width.is_finite() && metrics.width > 0.0);
        assert_eq!(
            session.text_measurement_report().entries()[0]
                .provenance()
                .source,
            TextMeasurementSource::Profile
        );

        let report = session.report();
        assert_eq!(report.theme_recipe_fingerprint(), None);
        assert_eq!(report.theme_recipe_report(), None);
        assert_eq!(report.font_catalog_fingerprint(), catalog.fingerprint());
        assert_eq!(report.font_source_policy(), &policy);
        assert_eq!(
            report
                .prepared_text_layout()
                .expect("report should freeze the prepared layout")
                .catalog_fingerprint(),
            catalog.fingerprint()
        );
        assert!(report.text_layout_failure().is_none());
    }

    #[test]
    #[cfg(feature = "embedded-fonts")]
    fn unthemed_session_revalidates_custom_font_catalog_before_text_preparation() {
        let catalog = embedded_font_theme().font_catalog().clone();
        let ceiling = crate::diagram_theme::ThemeResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::diagram_theme::ThemeResourceLimitId::MaxFontAssets, 0)
            .expect("valid zero-font-asset ceiling");
        let environment = RenderEnvironment::deterministic()
            .with_font_catalog(catalog)
            .with_theme_resource_ceiling(ceiling)
            .with_text_layout_backend(Arc::new(PrepareMustNotRunBackend::default()));

        let resource = match environment.begin_session() {
            Err(RenderEnvironmentError::ThemeResource(resource)) => resource,
            Ok(_) => panic!("the host ceiling must reject the retained custom font catalog"),
            Err(error) => panic!("unexpected environment error: {error:?}"),
        };
        assert_eq!(
            resource.phase,
            crate::diagram_theme::ThemeResourceLimitPhase::FontCatalog
        );
        assert_eq!(resource.limit, "max_font_assets");
        assert_eq!(resource.actual, 1);
        assert_eq!(resource.max, 0);
    }

    #[test]
    #[cfg(feature = "embedded-fonts")]
    fn cancelled_control_preempts_retained_resource_validation_and_text_preparation() {
        let catalog = embedded_font_theme().font_catalog().clone();
        let ceiling = crate::diagram_theme::ThemeResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::diagram_theme::ThemeResourceLimitId::MaxFontAssets, 0)
            .expect("valid zero-font-asset ceiling");
        let environment = RenderEnvironment::deterministic()
            .with_font_catalog(catalog)
            .with_theme_resource_ceiling(ceiling)
            .with_text_layout_backend(Arc::new(PrepareMustNotRunBackend::default()));
        let control = OperationControl::new();
        control.cancel();

        let error = match environment.begin_session_with_control(control) {
            Err(error) => error,
            Ok(_) => panic!("cancelled control must preempt session preparation"),
        };

        let RenderEnvironmentError::Cancelled(cancelled) = error else {
            panic!("expected structured cancellation, got {error:?}");
        };
        assert_eq!(cancelled.phase, OperationPhase::Layout);
        assert_eq!(cancelled.reason, merman_core::CancelReason::Requested);
    }

    #[test]
    #[cfg(feature = "embedded-fonts")]
    fn expired_deadline_preempts_themed_resource_validation_and_text_preparation() {
        let theme = resource_rich_theme();
        let ceiling = crate::diagram_theme::ThemeResourcePolicy::unbounded_for_trusted_input()
            .with_limit(crate::diagram_theme::ThemeResourceLimitId::MaxFontAssets, 0)
            .expect("valid zero-font-asset ceiling");
        let environment = RenderEnvironment::deterministic()
            .with_theme_resource_ceiling(ceiling)
            .with_text_layout_backend(Arc::new(PrepareMustNotRunBackend::default()));
        let operation_context = RuntimePolicy::deterministic()
            .begin_operation()
            .expect("deterministic operation context");
        let control = OperationControl::new().with_deadline(std::time::Duration::ZERO);

        let error = match environment.begin_session_with_theme_in_context(
            &theme,
            operation_context,
            control,
        ) {
            Err(error) => error,
            Ok(_) => panic!("expired deadline must preempt themed session preparation"),
        };

        let RenderEnvironmentError::Cancelled(cancelled) = error else {
            panic!("expected structured deadline cancellation, got {error:?}");
        };
        assert_eq!(cancelled.phase, OperationPhase::Layout);
        assert_eq!(
            cancelled.reason,
            merman_core::CancelReason::DeadlineExceeded
        );
    }

    #[test]
    fn session_theme_resources_meet_the_host_ceiling_and_compiled_restriction() {
        let limit = crate::diagram_theme::ThemeResourceLimitId::MaxEffectFilterRegionMagnitude;
        let policy_with_limit = |value| {
            crate::diagram_theme::ThemeResourcePolicy::interactive()
                .with_limit(limit, value)
                .expect("valid effect filter-region limit")
        };
        let compile_theme = |restriction| {
            crate::diagram_theme::DiagramThemeCompiler::new()
                .with_resource_policy(restriction)
                .compile(crate::diagram_theme::DiagramThemeSpec::new())
                .expect("compile empty theme with resource restriction")
        };

        let host_stricter = policy_with_limit(8);
        let theme = compile_theme(policy_with_limit(12));
        let environment =
            RenderEnvironment::deterministic().with_theme_resource_ceiling(host_stricter.clone());
        let session = environment
            .begin_session_with_theme(&theme)
            .expect("host-stricter themed session");
        assert_eq!(
            session.effective_theme_resource_policy().value(limit),
            Some(8),
            "a compiled theme must not widen the host-owned ceiling"
        );
        assert_eq!(
            environment
                .begin_session()
                .expect("unthemed host session")
                .effective_theme_resource_policy()
                .as_ref(),
            &host_stricter,
            "an unthemed session must freeze the host ceiling unchanged"
        );

        let theme_stricter = policy_with_limit(8);
        let theme = compile_theme(theme_stricter.clone());
        let session = RenderEnvironment::deterministic()
            .with_theme_resource_ceiling(policy_with_limit(12))
            .begin_session_with_theme(&theme)
            .expect("theme-stricter session");
        assert_eq!(
            session.effective_theme_resource_policy().as_ref(),
            &theme_stricter,
            "the compiler-time prior restriction must survive a looser host ceiling"
        );

        assert_eq!(
            RenderEnvironment::deterministic()
                .begin_session()
                .expect("default session")
                .effective_theme_resource_policy()
                .as_ref(),
            &crate::diagram_theme::ThemeResourcePolicy::interactive(),
            "the default host theme-resource ceiling must remain interactive"
        );
    }

    #[test]
    #[cfg(feature = "embedded-fonts")]
    fn themed_session_revalidates_retained_font_resources_before_text_preparation() {
        let theme = resource_rich_theme();
        let catalog = theme.font_catalog();
        let asset = &catalog.assets()[0];
        let compressed_bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ))
        .len();
        let table_count = catalog
            .faces()
            .iter()
            .map(crate::diagram_theme::FontFaceMetadata::table_count)
            .sum::<usize>();
        let cases = [
            (
                crate::diagram_theme::ThemeResourceLimitId::MaxFontAssetCompressedBytes,
                compressed_bytes.saturating_sub(1),
                Some(compressed_bytes),
            ),
            (
                crate::diagram_theme::ThemeResourceLimitId::MaxFontAssetDecodedBytes,
                asset.canonical_bytes().len().saturating_sub(1),
                Some(asset.canonical_bytes().len()),
            ),
            (
                crate::diagram_theme::ThemeResourceLimitId::MaxFontCatalogDecodedBytes,
                asset.canonical_bytes().len().saturating_sub(1),
                Some(asset.canonical_bytes().len()),
            ),
            (
                crate::diagram_theme::ThemeResourceLimitId::MaxFontAssets,
                0,
                Some(1),
            ),
            (
                crate::diagram_theme::ThemeResourceLimitId::MaxFontFaces,
                0,
                Some(catalog.faces().len()),
            ),
            (
                crate::diagram_theme::ThemeResourceLimitId::MaxFontTables,
                table_count.saturating_sub(1),
                Some(table_count),
            ),
            (
                crate::diagram_theme::ThemeResourceLimitId::MaxFontAliases,
                1,
                Some(2),
            ),
            (
                crate::diagram_theme::ThemeResourceLimitId::MaxFontDecodedExpansionRatio,
                1,
                None,
            ),
        ];

        for (limit, maximum, expected_actual) in cases {
            assert_theme_resource_rejected_before_text_preparation(
                &theme,
                limit,
                maximum,
                expected_actual,
            );
        }
    }

    #[test]
    #[cfg(feature = "embedded-fonts")]
    fn themed_session_revalidates_retained_effect_resources_before_text_preparation() {
        let theme = resource_rich_theme();
        let cases = [
            (
                crate::diagram_theme::ThemeResourceLimitId::MaxEffectGraphs,
                0,
                1,
            ),
            (
                crate::diagram_theme::ThemeResourceLimitId::MaxEffectPrimitivesPerGraph,
                3,
                4,
            ),
            (
                crate::diagram_theme::ThemeResourceLimitId::MaxEffectBindings,
                0,
                1,
            ),
            (
                crate::diagram_theme::ThemeResourceLimitId::MaxEffectOffsetMagnitude,
                5,
                6,
            ),
            (
                crate::diagram_theme::ThemeResourceLimitId::MaxEffectBlurMagnitude,
                6,
                7,
            ),
            (
                crate::diagram_theme::ThemeResourceLimitId::MaxEffectDisplacementScale,
                8,
                9,
            ),
            (
                crate::diagram_theme::ThemeResourceLimitId::MaxEffectTurbulenceOctaves,
                2,
                3,
            ),
        ];

        for (limit, maximum, expected_actual) in cases {
            assert_theme_resource_rejected_before_text_preparation(
                &theme,
                limit,
                maximum,
                Some(expected_actual),
            );
        }
    }

    #[cfg(feature = "embedded-fonts")]
    fn assert_theme_resource_rejected_before_text_preparation(
        theme: &DiagramTheme,
        limit: crate::diagram_theme::ThemeResourceLimitId,
        maximum: usize,
        expected_actual: Option<usize>,
    ) {
        let ceiling = crate::diagram_theme::ThemeResourcePolicy::unbounded_for_trusted_input()
            .with_limit(limit, maximum)
            .expect("valid narrow retained-resource ceiling");
        let environment = RenderEnvironment::deterministic()
            .with_theme_resource_ceiling(ceiling)
            .with_text_layout_backend(Arc::new(PrepareMustNotRunBackend::default()));

        let resource = match environment.begin_session_with_theme(theme) {
            Err(RenderEnvironmentError::ThemeResource(resource)) => resource,
            Ok(_) => panic!(
                "the narrow host must reject retained resource {}",
                limit.as_str()
            ),
            Err(error) => panic!("unexpected error for {}: {error:?}", limit.as_str()),
        };
        assert_eq!(resource.limit, limit.as_str());
        assert_eq!(resource.max, maximum);
        if let Some(expected_actual) = expected_actual {
            assert_eq!(resource.actual, expected_actual);
        } else {
            assert!(resource.actual > maximum);
        }
    }

    #[cfg(feature = "embedded-fonts")]
    fn mismatched_catalog_backend() -> (FontCatalog, CachedPreparedBackend) {
        let requested = embedded_font_theme().font_catalog().clone();
        let other_bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Xiaolai-Regular-CJK-Test.woff2"
        ));
        let other =
            crate::diagram_theme::FontCatalogSpec::new([crate::diagram_theme::FontAssetSpec::new(
                "xiaolai",
                other_bytes,
            )])
            .compile(&crate::diagram_theme::ThemeResourcePolicy::interactive())
            .expect("other fixture catalog should compile");
        let native = crate::text::NativeTextLayoutBackend::default();
        let response = native
            .prepare_catalog(&crate::text::PrepareCatalogRequest::new(
                other,
                crate::diagram_theme::FontSourcePolicy::embedded_only(),
            ))
            .expect("other catalog should prepare");
        let cached = CachedPreparedBackend {
            identity: response.backend().clone(),
            capabilities: response.capabilities(),
            response,
        };

        (requested, cached)
    }

    #[cfg(feature = "embedded-fonts")]
    fn matching_catalog_backend() -> (FontCatalog, CachedPreparedBackend) {
        let requested = embedded_font_theme().font_catalog().clone();
        let native = crate::text::NativeTextLayoutBackend::default();
        let response = native
            .prepare_catalog(&crate::text::PrepareCatalogRequest::new(
                requested.clone(),
                crate::diagram_theme::FontSourcePolicy::embedded_only(),
            ))
            .expect("fixture catalog should prepare");
        let cached = CachedPreparedBackend {
            identity: response.backend().clone(),
            capabilities: response.capabilities(),
            response,
        };

        (requested, cached)
    }

    #[test]
    #[cfg(feature = "embedded-fonts")]
    fn invalid_host_catalog_binding_falls_back_to_the_native_catalog() {
        let (requested, cached) = mismatched_catalog_backend();

        let session = RenderEnvironment::deterministic()
            .with_font_catalog(requested)
            .with_text_layout_backend(Arc::new(cached))
            .begin_session()
            .expect("runtime operation capture remains available for diagnostics");

        let layout = session
            .prepared_text_layout()
            .expect("the allowed native fallback should retain the requested catalog");
        let request = crate::text::PrepareTextRequest::new(
            "fallback",
            crate::diagram_theme::ThemeTextStyle::default().with_font_stack(
                crate::diagram_theme::FontStack::single("Excalifont")
                    .expect("fixture family is valid"),
            ),
        );
        layout
            .prepare_text(&request)
            .expect("the native fallback should prepare the label");
        let report = layout.report();
        assert!(report.face_count() > 0);
        assert_eq!(
            report.used_fallbacks(),
            &[HostMeasurementFallback::NativeCatalog]
        );
        assert_eq!(report.fallback_count(), 1);
        assert_eq!(
            report.used_font_sources(),
            &[crate::diagram_theme::FontSource::Embedded]
        );
        assert!(!report.is_host_dependent());
        assert!(session.text_layout_error().is_none());
    }

    #[test]
    #[cfg(feature = "embedded-fonts")]
    fn external_prepared_backend_is_host_dependent_only_after_actual_use() {
        let (requested, cached) = matching_catalog_backend();
        let session = RenderEnvironment::deterministic()
            .with_font_catalog(requested)
            .with_text_layout_backend(Arc::new(cached))
            .with_theme_measurement_fallbacks(
                HostMeasurementFallbackPolicy::new([HostMeasurementFallback::AcceptHostDependent])
                    .expect("single fallback is valid"),
            )
            .begin_session()
            .expect("host-dependent session should begin under best effort");
        let layout = session
            .prepared_text_layout()
            .expect("external catalog response should be retained");

        let unused = layout.report();
        assert!(unused.used_font_sources().is_empty());
        assert!(!unused.is_host_dependent());

        layout
            .prepare_text(&crate::text::PrepareTextRequest::new(
                "host",
                crate::diagram_theme::ThemeTextStyle::default().with_font_stack(
                    crate::diagram_theme::FontStack::single("Excalifont")
                        .expect("fixture family is valid"),
                ),
            ))
            .expect("external prepared backend should serve the label");
        let used = layout.report();
        assert_eq!(
            used.used_font_sources(),
            &[crate::diagram_theme::FontSource::Embedded]
        );
        assert!(used.is_host_dependent());
    }

    #[test]
    #[cfg(feature = "embedded-fonts")]
    fn portable_sessions_reject_external_prepared_backends() {
        let (requested, cached) = matching_catalog_backend();
        let session = RenderEnvironment::deterministic()
            .with_font_catalog(requested)
            .with_text_layout_backend(Arc::new(cached))
            .with_theme_measurement_fallbacks(
                HostMeasurementFallbackPolicy::new([HostMeasurementFallback::AcceptHostDependent])
                    .expect("single fallback is valid"),
            )
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session()
            .expect("runtime capture remains available for diagnostics");

        assert!(session.prepared_text_layout().is_none());
        assert_eq!(
            session.text_layout_error(),
            Some(&crate::text::TextLayoutError::HostDependentNotPortable)
        );
    }

    #[test]
    #[cfg(feature = "embedded-fonts")]
    fn invalid_host_catalog_binding_is_retained_when_fallbacks_are_disabled() {
        let (requested, cached) = mismatched_catalog_backend();
        let session = RenderEnvironment::deterministic()
            .with_font_catalog(requested)
            .with_text_layout_backend(Arc::new(cached))
            .with_theme_measurement_fallbacks(HostMeasurementFallbackPolicy::default())
            .begin_session()
            .expect("runtime operation capture remains available for diagnostics");

        assert!(session.prepared_text_layout().is_none());
        assert_eq!(
            session.text_layout_error(),
            Some(&crate::text::TextLayoutError::CatalogFingerprintMismatch)
        );
    }

    #[test]
    #[cfg(feature = "embedded-fonts")]
    fn custom_catalog_does_not_hijack_the_legacy_host_measurer() {
        let catalog = embedded_font_theme().font_catalog().clone();
        let host_calls = Arc::new(AtomicUsize::new(0));
        let fallback_calls = Arc::new(AtomicUsize::new(0));
        let policy = host_policy(
            HostOutcome::Measured(metrics(999.0)),
            &host_calls,
            &fallback_calls,
        );
        let session = RenderEnvironment::deterministic()
            .with_font_catalog(catalog)
            .with_text_measurement_policy(policy)
            .begin_session()
            .expect("begin custom catalog session");

        let measured = session
            .text_measurer(TextMeasurementPhase::Layout)
            .measure("portable", &TextStyle::default());
        assert_eq!(measured.width, 999.0);
        assert_eq!(host_calls.load(Ordering::Relaxed), 1);
        assert_eq!(fallback_calls.load(Ordering::Relaxed), 0);
        assert!(session.prepared_text_layout().is_some());
        let report = session.text_measurement_report();
        assert_eq!(report.entries().len(), 1);
        assert_eq!(
            report.entries()[0].provenance().source,
            TextMeasurementSource::Host
        );
        assert_eq!(report.entries()[0].provenance().fallback_reason, None);
    }

    #[cfg(feature = "math")]
    #[test]
    fn without_math_renderer_disables_the_compiled_default() {
        let session = RenderEnvironment::deterministic()
            .without_math_renderer()
            .begin_session()
            .expect("begin render session");

        assert!(session.math_renderer().is_none());
    }

    #[test]
    fn capability_policy_masks_installed_services_and_compiled_backends() {
        let session = RenderEnvironment::deterministic()
            .with_math_renderer(Arc::new(crate::math::NoopMathRenderer))
            .with_capability_policy(RenderCapabilityPolicy::deny_all())
            .begin_session()
            .expect("begin render session");

        assert!(!session.supports_capability(RenderCapability::LayoutCytoscape));
        assert!(!session.supports_capability(RenderCapability::LayoutElk));
        assert!(!session.supports_capability(RenderCapability::Math));
        assert!(session.math_renderer().is_none());

        let session = RenderEnvironment::deterministic()
            .with_math_renderer(Arc::new(crate::math::NoopMathRenderer))
            .with_capability_policy(
                RenderCapabilityPolicy::deny_all().with_allowed(RenderCapability::Math),
            )
            .begin_session()
            .expect("begin render session");
        assert!(session.supports_capability(RenderCapability::Math));
        assert!(session.math_renderer().is_some());
    }

    #[test]
    fn constrained_resource_profile_masks_monolithic_math_backends() {
        let constrained = RenderEnvironment::deterministic()
            .with_math_renderer(Arc::new(crate::math::NoopMathRenderer))
            .with_resource_policy(RenderResourcePolicy::constrained())
            .begin_session()
            .expect("begin constrained render session");
        assert!(!constrained.supports_capability(RenderCapability::Math));
        assert!(constrained.math_renderer().is_none());
        assert!(constrained.math_backend().is_none());

        for policy in [
            RenderResourcePolicy::interactive(),
            RenderResourcePolicy::trusted_native(),
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ] {
            let trusted = RenderEnvironment::deterministic()
                .with_math_renderer(Arc::new(crate::math::NoopMathRenderer))
                .with_resource_policy(policy)
                .begin_session()
                .expect("begin trusted render session");
            assert!(trusted.supports_capability(RenderCapability::Math));
            assert!(trusted.math_renderer().is_some());
            assert!(trusted.math_backend().is_some());
        }
    }
}

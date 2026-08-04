//! Operation-owned render services and deterministic policy.

use crate::math::MathRenderer;
use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
use crate::svg::IconRegistry;
use crate::text::{
    DeterministicTextMeasurer, FontMetricsTable, TextMeasurer, TextMetrics, TextStyle,
    VendoredFontMetricsTextMeasurer, WrapMode, estimate_line_width_px, round_to_1_64_px,
};
use merman_core::runtime::{OperationContext, OperationTiming, RuntimePolicy, RuntimePolicyError};
use merman_core::time::LocalTimeZoneProvenance;
use std::fmt;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

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
    VendoredParity,
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
        self.builtin
            .map(|profile| BuiltinInlineHtmlWidth::new(profile, style))
    }
}

#[derive(Debug, Clone)]
enum BuiltinInlineRawLineWidth {
    Vendored {
        table: &'static FontMetricsTable,
        font_size: f64,
        em: f64,
        prevprev: Option<char>,
        prev: Option<char>,
    },
    Deterministic {
        font_size: f64,
        em: f64,
    },
}

impl BuiltinInlineRawLineWidth {
    fn new(profile: BuiltinTextMeasurementProfile, style: &TextStyle) -> (Self, bool) {
        let font_size = style.font_size.max(1.0);
        if profile == BuiltinTextMeasurementProfile::VendoredParity
            && let Some(table) = VendoredFontMetricsTextMeasurer::unwrapped_html_width_table(style)
        {
            return (
                Self::Vendored {
                    table,
                    font_size,
                    em: 0.0,
                    prevprev: None,
                    prev: None,
                },
                true,
            );
        }

        (Self::Deterministic { font_size, em: 0.0 }, false)
    }

    fn push_char(&mut self, ch: char) {
        match self {
            Self::Vendored {
                table,
                em,
                prevprev,
                prev,
                ..
            } => VendoredFontMetricsTextMeasurer::accumulate_unwrapped_html_char_em(
                *table, em, prevprev, prev, ch,
            ),
            Self::Deterministic { em, .. } => {
                let mut encoded = [0_u8; 4];
                let scalar = ch.encode_utf8(&mut encoded);
                // The built-in deterministic profile uses its default heuristic. Accumulating
                // one scalar's em width at a time preserves `estimate_line_width_px`'s exact
                // left-to-right floating-point order without copying the growing line.
                *em += estimate_line_width_px(scalar, 1.0);
            }
        }
    }

    fn width_px(&self) -> f64 {
        match self {
            Self::Vendored { font_size, em, .. } | Self::Deterministic { font_size, em } => {
                *em * *font_size
            }
        }
    }

    fn reset(&mut self) {
        match self {
            Self::Vendored {
                em, prevprev, prev, ..
            } => {
                *em = 0.0;
                *prevprev = None;
                *prev = None;
            }
            Self::Deterministic { em, .. } => *em = 0.0,
        }
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

        if !ch.is_whitespace() && !self.line_has_non_whitespace {
            // Completed whitespace-only lines cease to be trailing as soon as a later visible
            // scalar arrives. This mirrors `normalized_text_lines` trimming only the final blank
            // suffix while retaining interior whitespace-only lines.
            self.max_width_px = self.max_width_px.max(self.pending_blank_width_px);
            self.pending_blank_width_px = 0.0;
            self.line_has_non_whitespace = true;
        }
        self.line.push_char(ch);
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
}

/// Exact streaming width state for a qualified built-in HTML measurement route.
///
/// The lightweight HTML parser in `text::metrics` owns tag handling and passes only decoded inline
/// text runs here. That keeps escaped literals like `&lt;br&gt;` measurable as text instead of
/// being interpreted a second time as DOM line breaks.
#[derive(Debug, Clone)]
pub(crate) struct BuiltinInlineHtmlWidth {
    normalized: BuiltinNormalizedTextWidth,
    quantize: bool,
}

impl BuiltinInlineHtmlWidth {
    fn new(profile: BuiltinTextMeasurementProfile, style: &TextStyle) -> Self {
        let (line, quantize) = BuiltinInlineRawLineWidth::new(profile, style);
        Self {
            normalized: BuiltinNormalizedTextWidth::new(line),
            quantize,
        }
    }

    pub(crate) fn push_text(&mut self, text: &str) {
        for ch in text.chars() {
            self.normalized.push_char(ch);
        }
    }

    pub(crate) fn width_px(&self) -> f64 {
        let raw_width_px = self.normalized.finished_width_px();
        if self.quantize {
            round_to_1_64_px(raw_width_px)
        } else {
            raw_width_px
        }
    }
}

fn vendored_parity_profile() -> TextMeasurementProfile {
    let profile = MeasurementProfileId::new("merman.mermaid-11.16-text-metrics")
        .expect("static vendored profile id is valid");
    let identity = TextMeasurementProfileIdentity::new(
        profile,
        concat!(
            "merman-render@",
            env!("CARGO_PKG_VERSION"),
            "/mermaid@11.16.0"
        ),
    )
    .expect("static vendored profile version is valid");
    TextMeasurementProfile::new_builtin(
        identity,
        Arc::new(VendoredFontMetricsTextMeasurer::initialized()),
        BuiltinTextMeasurementProfile::VendoredParity,
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
    pub fn parity() -> Self {
        Self::uniform(vendored_parity_profile())
    }

    pub fn deterministic() -> Self {
        let profile = MeasurementProfileId::new("merman.deterministic-text")
            .expect("static deterministic profile id is valid");
        let identity = TextMeasurementProfileIdentity::new(
            profile,
            concat!("merman-render@", env!("CARGO_PKG_VERSION")),
        )
        .expect("static deterministic profile version is valid");
        Self::uniform(TextMeasurementProfile::new_builtin(
            identity,
            Arc::new(DeterministicTextMeasurer::default()),
            BuiltinTextMeasurementProfile::Deterministic,
        ))
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
        Self::host_display_with_fallback(identity, host, host_phases, vendored_parity_profile())
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
        Self::parity()
    }
}

/// Session-aware facade that routes specialized operations to their named phases.
pub struct RoutedTextMeasurer<'a> {
    default_phase: TextMeasurementPhase,
    policy: &'a TextMeasurementPolicy,
    recorder: &'a TextMeasurementRecorder,
}

impl RoutedTextMeasurer<'_> {
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

    fn resolve<T>(
        &self,
        request: HostTextMeasurementRequest<'_>,
        decode_host: impl FnOnce(HostTextMeasurement) -> Option<T>,
        profile_call: impl FnOnce(&(dyn TextMeasurer + Send + Sync)) -> T,
    ) -> T {
        let phase = request.phase;
        let operation = request.operation;
        match &self.policy.routes[phase.index()] {
            TextMeasurementRouteConfig::Profile(profile) => {
                let value = profile_call(profile.backend.as_ref());
                self.recorder
                    .record(phase, operation, TextMeasurementRouteOutcome::Profile);
                value
            }
            TextMeasurementRouteConfig::Host {
                backend, fallback, ..
            } => {
                let attempt = backend.measure(request);
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
                let value = profile_call(fallback.backend.as_ref());
                self.recorder.record(
                    phase,
                    operation,
                    TextMeasurementRouteOutcome::Fallback(reason),
                );
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
    #[allow(private_interfaces)]
    fn builtin_operation_carrier(
        &self,
        operation: TextMeasurementOperation,
    ) -> Option<BuiltinTextMeasurementOperationCarrier> {
        RoutedTextMeasurer::builtin_operation_carrier(self, operation)
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
fn default_math_renderer() -> Option<Arc<dyn MathRenderer + Send + Sync>> {
    Some(Arc::new(crate::math::RatexMathRenderer))
}

#[cfg(not(feature = "math"))]
fn default_math_renderer() -> Option<Arc<dyn MathRenderer + Send + Sync>> {
    None
}

/// Immutable render services and the policy used to capture one operation context.
#[derive(Clone)]
pub struct RenderEnvironment {
    text_measurement: TextMeasurementPolicy,
    math_renderer: Option<Arc<dyn MathRenderer + Send + Sync>>,
    icon_registry: Option<Arc<IconRegistry>>,
    runtime_policy: RuntimePolicy,
    resource_policy: RenderResourcePolicy,
}

impl fmt::Debug for RenderEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RenderEnvironment")
            .field("text_measurement", &self.text_measurement)
            .field("has_math_renderer", &self.math_renderer.is_some())
            .field("has_icon_registry", &self.icon_registry.is_some())
            .field("runtime_policy", &self.runtime_policy)
            .field("resource_policy", &self.resource_policy)
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
            text_measurement: TextMeasurementPolicy::parity(),
            math_renderer: default_math_renderer(),
            icon_registry: None,
            runtime_policy: RuntimePolicy::deterministic(),
            resource_policy: RenderResourcePolicy::interactive(),
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

    /// Installs the math renderer compiled into this renderer, if present.
    ///
    /// Facades use this to select the canonical compiled capability instead of duplicating Cargo
    /// feature checks in each transport layer.
    pub fn with_compiled_math_renderer(mut self) -> Self {
        self.math_renderer = default_math_renderer();
        self
    }

    pub fn with_math_renderer(mut self, renderer: Arc<dyn MathRenderer + Send + Sync>) -> Self {
        self.math_renderer = Some(renderer);
        self
    }

    pub fn without_math_renderer(mut self) -> Self {
        self.math_renderer = None;
        self
    }

    pub fn with_icon_registry(mut self, registry: Arc<IconRegistry>) -> Self {
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

    /// Captures time, timezone rules, random seed, and provenance exactly once.
    pub fn begin_session(&self) -> Result<RenderSession, RuntimePolicyError> {
        let operation_context = self.runtime_policy.begin_operation()?;
        Ok(RenderSession {
            text_measurement: self.text_measurement.clone(),
            measurement_recorder: Box::default(),
            math_renderer: self.math_renderer.clone(),
            icon_registry: self.icon_registry.clone(),
            operation_context,
            resource_policy: self.resource_policy,
            work_meter: Arc::new(OperationWorkMeter::new(self.resource_policy)),
        })
    }
}

impl Default for RenderEnvironment {
    fn default() -> Self {
        Self::deterministic()
    }
}

/// Opaque operation session. Family code receives only the narrow projection it needs.
pub struct RenderSession {
    text_measurement: TextMeasurementPolicy,
    // Keep movable family artifacts compact for bounded worker stacks.
    measurement_recorder: Box<TextMeasurementRecorder>,
    math_renderer: Option<Arc<dyn MathRenderer + Send + Sync>>,
    icon_registry: Option<Arc<IconRegistry>>,
    operation_context: OperationContext,
    resource_policy: RenderResourcePolicy,
    work_meter: Arc<OperationWorkMeter>,
}

impl RenderSession {
    pub fn text_measurer(&self, default_phase: TextMeasurementPhase) -> RoutedTextMeasurer<'_> {
        RoutedTextMeasurer {
            default_phase,
            policy: &self.text_measurement,
            recorder: &self.measurement_recorder,
        }
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

    pub(crate) fn work_meter(&self) -> &Arc<OperationWorkMeter> {
        &self.work_meter
    }

    pub fn math_renderer(&self) -> Option<&(dyn MathRenderer + Send + Sync)> {
        self.math_renderer.as_deref()
    }

    pub fn icon_registry(&self) -> Option<&IconRegistry> {
        self.icon_registry.as_deref()
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::measure_html_with_inline_styles;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn inline_html_carrier<M: TextMeasurer + ?Sized>(measurer: &M) -> InlineHtmlMeasurementCarrier {
        measurer
            .builtin_operation_carrier(TextMeasurementOperation::WrappedWithRawWidth)
            .and_then(BuiltinTextMeasurementOperationCarrier::into_inline_html)
            .unwrap_or_else(InlineHtmlMeasurementCarrier::opaque)
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
            vendored_parity_profile(),
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
            vendored_parity_profile(),
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
            vendored_parity_profile(),
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
            vendored_parity_profile(),
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
            vendored_parity_profile(),
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
                profile: BuiltinTextMeasurementProfile::VendoredParity,
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
                profile: BuiltinTextMeasurementProfile::VendoredParity,
                phase: TextMeasurementPhase::SvgBBox,
                operation: TextMeasurementOperation::WrappedWithRawWidth,
            }
            .into_inline_html()
            .is_none(),
            "the inline planner requires both the wrapped operation and its Wrap owner phase"
        );

        let parity_session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin parity session");
        let parity = parity_session.text_measurer(TextMeasurementPhase::Layout);
        let parity_carrier = inline_html_carrier(&parity);
        assert!(parity_carrier.is_builtin());
        let parity_sequence_carrier = parity
            .builtin_operation_carrier(TextMeasurementOperation::MermaidCalculateTextDimensions)
            .expect("sequence measurement route is built-in");
        assert_eq!(parity_sequence_carrier.phase, TextMeasurementPhase::SvgBBox);
        assert_eq!(
            parity_sequence_carrier.operation,
            TextMeasurementOperation::MermaidCalculateTextDimensions
        );

        let deterministic_session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
            .begin_session()
            .expect("begin deterministic session");
        let deterministic = deterministic_session.text_measurer(TextMeasurementPhase::Layout);
        let deterministic_carrier = inline_html_carrier(&deterministic);
        assert!(deterministic_carrier.is_builtin());
        assert!(
            deterministic
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
            "host routes remain opaque even when their fallback is vendored"
        );
        assert!(
            host.builtin_operation_carrier(
                TextMeasurementOperation::MermaidCalculateTextDimensions
            )
            .is_none()
        );
    }

    #[test]
    fn builtin_inline_stream_matches_backend_order_for_decoded_run_text() {
        fn assert_cases(
            measurer: &dyn TextMeasurer,
            carrier: InlineHtmlMeasurementCarrier,
            style: &TextStyle,
            cases: &[(&str, &[&str])],
        ) {
            for (html, chunks) in cases {
                let expected = measure_html_with_inline_styles(
                    measurer,
                    html,
                    style,
                    None,
                    WrapMode::HtmlLike,
                )
                .width;
                let mut streamed = carrier
                    .begin_inline_html_width(style)
                    .expect("built-in carrier starts a streaming width");
                for chunk in *chunks {
                    streamed.push_text(chunk);
                }
                assert_eq!(
                    round_to_1_64_px(streamed.width_px()).to_bits(),
                    expected.to_bits(),
                    "html={html:?}, chunks={chunks:?}"
                );
            }
        }

        let cases: &[(&str, &[&str])] = &[
            ("AVATAR office", &["A", "VAT", "AR ", "office"]),
            (
                "alpha&lt;br   /&gt;omega",
                &["alpha<", "b", "r ", "  /", ">", "omega"],
            ),
            ("&lt;b&lt;br/&gt;tail", &["<b<", "br", "/>tail"]),
            ("wide&lt;br / &gt;literal", &["wide<br ", "/ ", ">literal"]),
            // Tag parsing is owned by `text::metrics`; the streaming width state measures decoded
            // inline text runs literally, including escaped spellings of `<br>`.
            (
                "wide&lt;br\u{000C}/&gt;literal",
                &["wide<br", "\u{000C}", "/>literal"],
            ),
            (
                "wide\n                         ",
                &["wide\n", "             ", "            "],
            ),
            (" \n  ", &[" ", "\n", "  "]),
            ("A\u{301}👩‍💻مرحبا世界", &["A\u{301}", "👩‍💻", "مرحبا", "世界"]),
        ];
        let style = TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: Some("700".to_string()),
            font_style: Some("italic".to_string()),
        };

        let parity_session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin parity session");
        let parity_measurer = parity_session.text_measurer(TextMeasurementPhase::Wrap);
        let parity_carrier = inline_html_carrier(&parity_measurer);
        assert_cases(&parity_measurer, parity_carrier, &style, cases);
        let long_whitespace = " ".repeat(8_192);
        let long_break = format!("wide&lt;br{long_whitespace}/&gt;tail");
        let expected = measure_html_with_inline_styles(
            &parity_measurer,
            &long_break,
            &style,
            None,
            WrapMode::HtmlLike,
        )
        .width;
        let mut streamed = parity_carrier
            .begin_inline_html_width(&style)
            .expect("parity carrier starts a streaming width");
        streamed.push_text("wide<br");
        streamed.push_text(&long_whitespace);
        streamed.push_text("/>tail");
        assert_eq!(
            round_to_1_64_px(streamed.width_px()).to_bits(),
            expected.to_bits()
        );

        let mut unknown_font = style.clone();
        unknown_font.font_family = Some("fixture-private-font".to_string());
        assert_cases(&parity_measurer, parity_carrier, &unknown_font, cases);

        let deterministic_session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
            .begin_session()
            .expect("begin deterministic session");
        let deterministic_measurer =
            deterministic_session.text_measurer(TextMeasurementPhase::Wrap);
        let deterministic_carrier = inline_html_carrier(&deterministic_measurer);
        assert_cases(
            &deterministic_measurer,
            deterministic_carrier,
            &style,
            cases,
        );
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
        let policy = TextMeasurementPolicy::parity();
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
            .with_icon_registry(Arc::new(IconRegistry::new()))
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
        assert_eq!(session.report().operation_context().seed(), 0);
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
}

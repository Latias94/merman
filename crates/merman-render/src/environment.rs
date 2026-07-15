//! Operation-owned render services and deterministic policy.

use crate::math::MathRenderer;
use crate::resources::RenderResourceLimits;
use crate::svg::IconRegistry;
use crate::text::{
    DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle,
    VendoredFontMetricsTextMeasurer, WrapMode,
};
use chrono::{DateTime, FixedOffset, NaiveDate, Utc};
use std::fmt;
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex, MutexGuard};

#[cfg(feature = "host")]
use std::sync::atomic::{AtomicU64, Ordering};

/// A render phase that may select a distinct complete text-measurement profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextMeasurementPhase {
    Layout,
    Wrap,
    SvgBBox,
    ComputedLength,
    Visibility,
}

impl TextMeasurementPhase {
    pub const ALL: [Self; 5] = [
        Self::Layout,
        Self::Wrap,
        Self::SvgBBox,
        Self::ComputedLength,
        Self::Visibility,
    ];

    const fn index(self) -> usize {
        match self {
            Self::Layout => 0,
            Self::Wrap => 1,
            Self::SvgBBox => 2,
            Self::ComputedLength => 3,
            Self::Visibility => 4,
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
}

impl TextMeasurementProfile {
    pub fn new(
        identity: TextMeasurementProfileIdentity,
        backend: Arc<dyn TextMeasurer + Send + Sync>,
    ) -> Self {
        Self { identity, backend }
    }

    pub fn identity(&self) -> &TextMeasurementProfileIdentity {
        &self.identity
    }
}

impl fmt::Debug for TextMeasurementProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextMeasurementProfile")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

fn vendored_parity_profile() -> TextMeasurementProfile {
    let profile = MeasurementProfileId::new("merman.vendored-font-metrics")
        .expect("static vendored profile id is valid");
    let identity = TextMeasurementProfileIdentity::new(
        profile,
        concat!(
            "merman-render@",
            env!("CARGO_PKG_VERSION"),
            "/mermaid@11.16.0"
        ),
    )
    .expect("static vendored profile version is valid")
    .with_decorators([
        "flowchart-text-overrides@11.12.2",
        "sequence-svg-overrides@11.16.0",
    ])
    .expect("static vendored decorators are valid");
    TextMeasurementProfile::new(
        identity,
        Arc::new(VendoredFontMetricsTextMeasurer::default()),
    )
}

/// Why a configured host attempt used its named fallback profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostFallbackReason {
    Missing,
    Invalid,
    Error,
}

/// The exact [`TextMeasurer`] operation performed through a phase facade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextMeasurementOperation {
    Measure,
    ComputedLength,
    BBoxX,
    BBoxXWithAsciiOverhang,
    TitleBBoxX,
    SimpleBBoxWidth,
    RawBBoxWidth,
    WrapProbeBBoxWidth,
    SimpleBBoxHeight,
    Wrapped,
    WrappedWithRawWidth,
    WrappedRaw,
}

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

#[derive(Debug, Default)]
struct TextMeasurementRecorder {
    entries: Mutex<Vec<TextMeasurementSummary>>,
}

impl TextMeasurementRecorder {
    fn record(&self, provenance: TextMeasurementProvenance) {
        let mut entries = lock_unpoisoned(&self.entries);
        if let Some(existing) = entries
            .iter_mut()
            .find(|entry| entry.provenance == provenance)
        {
            existing.count = existing.count.saturating_add(1);
            return;
        }
        entries.push(TextMeasurementSummary {
            provenance,
            count: 1,
        });
    }

    fn report(&self) -> TextMeasurementReport {
        TextMeasurementReport {
            entries: lock_unpoisoned(&self.entries).clone(),
        }
    }
}

fn lock_unpoisoned<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A host callback failure converted to explicit fallback provenance by the policy.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct HostTextMeasurementError {
    message: Arc<str>,
}

impl HostTextMeasurementError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: Arc::from(message.into()),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

pub type HostMeasurementResult<T> = Result<Option<T>, HostTextMeasurementError>;

/// Fallible host counterpart of the complete [`TextMeasurer`] interface.
///
/// Returning `Ok(None)` declines exactly that operation. The environment then calls the same
/// method on the configured fallback profile once and records why it did so.
pub trait HostTextMeasurer: Send + Sync {
    fn measure(
        &self,
        phase: TextMeasurementPhase,
        text: &str,
        style: &TextStyle,
    ) -> HostMeasurementResult<TextMetrics>;

    fn measure_svg_text_computed_length_px(
        &self,
        phase: TextMeasurementPhase,
        text: &str,
        style: &TextStyle,
    ) -> HostMeasurementResult<f64> {
        self.measure_svg_simple_text_bbox_width_px(phase, text, style)
    }

    fn measure_svg_text_bbox_x(
        &self,
        phase: TextMeasurementPhase,
        text: &str,
        style: &TextStyle,
    ) -> HostMeasurementResult<(f64, f64)> {
        self.measure(phase, text, style).map(|metrics| {
            metrics.map(|metrics| {
                if valid_metrics(&metrics) {
                    let half = metrics.width / 2.0;
                    (half, half)
                } else {
                    (f64::NAN, f64::NAN)
                }
            })
        })
    }

    fn measure_svg_text_bbox_x_with_ascii_overhang(
        &self,
        phase: TextMeasurementPhase,
        text: &str,
        style: &TextStyle,
    ) -> HostMeasurementResult<(f64, f64)> {
        self.measure_svg_text_bbox_x(phase, text, style)
    }

    fn measure_svg_title_bbox_x(
        &self,
        phase: TextMeasurementPhase,
        text: &str,
        style: &TextStyle,
    ) -> HostMeasurementResult<(f64, f64)> {
        self.measure_svg_text_bbox_x(phase, text, style)
    }

    fn measure_svg_simple_text_bbox_width_px(
        &self,
        phase: TextMeasurementPhase,
        text: &str,
        style: &TextStyle,
    ) -> HostMeasurementResult<f64> {
        self.measure_svg_title_bbox_x(phase, text, style)
            .map(|extents| extents.map(|(left, right)| left + right))
    }

    fn measure_svg_raw_text_bbox_width_px(
        &self,
        phase: TextMeasurementPhase,
        text: &str,
        style: &TextStyle,
    ) -> HostMeasurementResult<f64> {
        self.measure_svg_simple_text_bbox_width_px(phase, text, style)
    }

    fn measure_svg_simple_text_bbox_width_for_wrap_px(
        &self,
        phase: TextMeasurementPhase,
        text: &str,
        style: &TextStyle,
    ) -> HostMeasurementResult<f64> {
        self.measure_svg_simple_text_bbox_width_px(phase, text, style)
    }

    fn measure_svg_simple_text_bbox_height_px(
        &self,
        phase: TextMeasurementPhase,
        text: &str,
        style: &TextStyle,
    ) -> HostMeasurementResult<f64> {
        self.measure(phase, text, style).map(|metrics| {
            metrics.map(|metrics| {
                if valid_metrics(&metrics) {
                    metrics.height
                } else {
                    f64::NAN
                }
            })
        })
    }

    fn measure_wrapped(
        &self,
        _phase: TextMeasurementPhase,
        _text: &str,
        _style: &TextStyle,
        _max_width: Option<f64>,
        _wrap_mode: WrapMode,
    ) -> HostMeasurementResult<TextMetrics> {
        Ok(None)
    }

    fn measure_wrapped_with_raw_width(
        &self,
        _phase: TextMeasurementPhase,
        _text: &str,
        _style: &TextStyle,
        _max_width: Option<f64>,
        _wrap_mode: WrapMode,
    ) -> HostMeasurementResult<(TextMetrics, Option<f64>)> {
        Ok(None)
    }

    fn measure_wrapped_raw(
        &self,
        _phase: TextMeasurementPhase,
        _text: &str,
        _style: &TextStyle,
        _max_width: Option<f64>,
        _wrap_mode: WrapMode,
    ) -> HostMeasurementResult<TextMetrics> {
        Ok(None)
    }
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
    routes: [TextMeasurementRouteConfig; 5],
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
        Self::uniform(TextMeasurementProfile::new(
            identity,
            Arc::new(DeterministicTextMeasurer::default()),
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

    pub fn routes(&self) -> [TextMeasurementRoute; 5] {
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
            | TextMeasurementOperation::SimpleBBoxHeight => TextMeasurementPhase::SvgBBox,
            TextMeasurementOperation::WrapProbeBBoxWidth => TextMeasurementPhase::Wrap,
            TextMeasurementOperation::Wrapped
            | TextMeasurementOperation::WrappedWithRawWidth
            | TextMeasurementOperation::WrappedRaw => TextMeasurementPhase::Wrap,
            TextMeasurementOperation::Measure => self.default_phase,
        }
    }

    fn resolve<T>(
        &self,
        operation: TextMeasurementOperation,
        host_call: impl FnOnce(&dyn HostTextMeasurer) -> HostMeasurementResult<T>,
        profile_call: impl FnOnce(&(dyn TextMeasurer + Send + Sync)) -> T,
        valid: impl FnOnce(&T) -> bool,
    ) -> T {
        let phase = self.phase_for(operation);
        match &self.policy.routes[phase.index()] {
            TextMeasurementRouteConfig::Profile(profile) => {
                let value = profile_call(profile.backend.as_ref());
                self.recorder.record(TextMeasurementProvenance {
                    phase,
                    operation,
                    source: TextMeasurementSource::Profile,
                    identity: profile.identity.clone(),
                    fallback_reason: None,
                });
                value
            }
            TextMeasurementRouteConfig::Host {
                identity,
                backend,
                fallback,
            } => match host_call(backend.as_ref()) {
                Ok(Some(value)) if valid(&value) => {
                    self.recorder.record(TextMeasurementProvenance {
                        phase,
                        operation,
                        source: TextMeasurementSource::Host,
                        identity: identity.clone(),
                        fallback_reason: None,
                    });
                    value
                }
                attempt => {
                    let reason = match attempt {
                        Ok(Some(_)) => HostFallbackReason::Invalid,
                        Ok(None) => HostFallbackReason::Missing,
                        Err(_) => HostFallbackReason::Error,
                    };
                    let value = profile_call(fallback.backend.as_ref());
                    self.recorder.record(TextMeasurementProvenance {
                        phase,
                        operation,
                        source: TextMeasurementSource::Profile,
                        identity: fallback.identity.clone(),
                        fallback_reason: Some(reason),
                    });
                    value
                }
            },
        }
    }
}

impl TextMeasurer for RoutedTextMeasurer<'_> {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.resolve(
            TextMeasurementOperation::Measure,
            |host| {
                host.measure(
                    self.phase_for(TextMeasurementOperation::Measure),
                    text,
                    style,
                )
            },
            |profile| profile.measure(text, style),
            valid_metrics,
        )
    }

    fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            TextMeasurementOperation::ComputedLength,
            |host| {
                host.measure_svg_text_computed_length_px(
                    TextMeasurementPhase::ComputedLength,
                    text,
                    style,
                )
            },
            |profile| profile.measure_svg_text_computed_length_px(text, style),
            valid_length,
        )
    }

    fn measure_svg_text_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
        self.resolve(
            TextMeasurementOperation::BBoxX,
            |host| host.measure_svg_text_bbox_x(TextMeasurementPhase::SvgBBox, text, style),
            |profile| profile.measure_svg_text_bbox_x(text, style),
            valid_extents,
        )
    }

    fn measure_svg_text_bbox_x_with_ascii_overhang(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> (f64, f64) {
        self.resolve(
            TextMeasurementOperation::BBoxXWithAsciiOverhang,
            |host| {
                host.measure_svg_text_bbox_x_with_ascii_overhang(
                    TextMeasurementPhase::SvgBBox,
                    text,
                    style,
                )
            },
            |profile| profile.measure_svg_text_bbox_x_with_ascii_overhang(text, style),
            valid_extents,
        )
    }

    fn measure_svg_title_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
        self.resolve(
            TextMeasurementOperation::TitleBBoxX,
            |host| host.measure_svg_title_bbox_x(TextMeasurementPhase::SvgBBox, text, style),
            |profile| profile.measure_svg_title_bbox_x(text, style),
            valid_extents,
        )
    }

    fn measure_svg_simple_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            TextMeasurementOperation::SimpleBBoxWidth,
            |host| {
                host.measure_svg_simple_text_bbox_width_px(
                    TextMeasurementPhase::SvgBBox,
                    text,
                    style,
                )
            },
            |profile| profile.measure_svg_simple_text_bbox_width_px(text, style),
            valid_length,
        )
    }

    fn measure_svg_raw_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            TextMeasurementOperation::RawBBoxWidth,
            |host| {
                host.measure_svg_raw_text_bbox_width_px(TextMeasurementPhase::SvgBBox, text, style)
            },
            |profile| profile.measure_svg_raw_text_bbox_width_px(text, style),
            valid_length,
        )
    }

    fn measure_svg_simple_text_bbox_width_for_wrap_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            TextMeasurementOperation::WrapProbeBBoxWidth,
            |host| {
                host.measure_svg_simple_text_bbox_width_for_wrap_px(
                    TextMeasurementPhase::Wrap,
                    text,
                    style,
                )
            },
            |profile| profile.measure_svg_simple_text_bbox_width_for_wrap_px(text, style),
            valid_length,
        )
    }

    fn measure_mermaid_calculate_text_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            TextMeasurementOperation::WrapProbeBBoxWidth,
            |host| {
                host.measure_svg_simple_text_bbox_width_for_wrap_px(
                    TextMeasurementPhase::Wrap,
                    text,
                    style,
                )
            },
            |profile| profile.measure_mermaid_calculate_text_width_px(text, style),
            valid_length,
        )
    }

    fn measure_svg_simple_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            TextMeasurementOperation::SimpleBBoxHeight,
            |host| {
                host.measure_svg_simple_text_bbox_height_px(
                    TextMeasurementPhase::SvgBBox,
                    text,
                    style,
                )
            },
            |profile| profile.measure_svg_simple_text_bbox_height_px(text, style),
            valid_length,
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
            TextMeasurementOperation::Wrapped,
            |host| {
                host.measure_wrapped(
                    TextMeasurementPhase::Wrap,
                    text,
                    style,
                    max_width,
                    wrap_mode,
                )
            },
            |profile| profile.measure_wrapped(text, style, max_width, wrap_mode),
            valid_metrics,
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
            TextMeasurementOperation::WrappedWithRawWidth,
            |host| {
                host.measure_wrapped_with_raw_width(
                    TextMeasurementPhase::Wrap,
                    text,
                    style,
                    max_width,
                    wrap_mode,
                )
            },
            |profile| profile.measure_wrapped_with_raw_width(text, style, max_width, wrap_mode),
            valid_wrapped_with_raw_width,
        )
    }

    fn measure_wrapped_raw(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.resolve(
            TextMeasurementOperation::WrappedRaw,
            |host| {
                host.measure_wrapped_raw(
                    TextMeasurementPhase::Wrap,
                    text,
                    style,
                    max_width,
                    wrap_mode,
                )
            },
            |profile| profile.measure_wrapped_raw(text, style, max_width, wrap_mode),
            valid_metrics,
        )
    }
}

fn valid_metrics(metrics: &TextMetrics) -> bool {
    metrics.width.is_finite()
        && metrics.height.is_finite()
        && metrics.width >= 0.0
        && metrics.height >= 0.0
        && metrics.line_count > 0
}

fn valid_length(value: &f64) -> bool {
    value.is_finite() && *value >= 0.0
}

fn valid_extents(value: &(f64, f64)) -> bool {
    value.0.is_finite() && value.1.is_finite() && value.0 >= 0.0 && value.1 >= 0.0
}

fn valid_wrapped_with_raw_width(value: &(TextMetrics, Option<f64>)) -> bool {
    valid_metrics(&value.0) && value.1.as_ref().is_none_or(valid_length)
}

/// One coherent time snapshot derived from an instant and a UTC offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderTimeSnapshot {
    unix_ms: i64,
    local_date: NaiveDate,
    local_offset_minutes: i32,
}

impl RenderTimeSnapshot {
    pub fn from_unix_millis(
        unix_ms: i64,
        local_offset_minutes: i32,
    ) -> Result<Self, RenderTimeError> {
        if !(-1439..=1439).contains(&local_offset_minutes) {
            return Err(RenderTimeError::InvalidLocalOffset(local_offset_minutes));
        }
        let instant = DateTime::<Utc>::from_timestamp_millis(unix_ms)
            .ok_or(RenderTimeError::InstantOutOfRange(unix_ms))?;
        let offset = FixedOffset::east_opt(local_offset_minutes * 60)
            .ok_or(RenderTimeError::InvalidLocalOffset(local_offset_minutes))?;
        Ok(Self {
            unix_ms,
            local_date: instant.with_timezone(&offset).date_naive(),
            local_offset_minutes,
        })
    }

    pub fn unix_epoch_utc() -> Self {
        Self::from_unix_millis(0, 0).expect("Unix epoch is a valid UTC instant")
    }

    pub const fn unix_ms(self) -> i64 {
        self.unix_ms
    }

    pub const fn local_date(self) -> NaiveDate {
        self.local_date
    }

    pub const fn local_offset_minutes(self) -> i32 {
        self.local_offset_minutes
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RenderTimeError {
    #[error("local UTC offset must be between -1439 and 1439 minutes, got {0}")]
    InvalidLocalOffset(i32),
    #[error("Unix timestamp in milliseconds is outside chrono's supported range: {0}")]
    InstantOutOfRange(i64),
}

/// Supplies an instant and offset; the environment derives the date when the session begins.
pub trait RenderClock: Send + Sync {
    fn unix_millis_and_offset(&self) -> (i64, i32);
}

/// Preserves a clock's advancing instant while replacing its local offset.
#[derive(Clone)]
pub struct FixedOffsetRenderClock {
    source: Arc<dyn RenderClock>,
    local_offset_minutes: i32,
}

impl FixedOffsetRenderClock {
    pub fn new(
        source: Arc<dyn RenderClock>,
        local_offset_minutes: i32,
    ) -> Result<Self, RenderTimeError> {
        if !(-1439..=1439).contains(&local_offset_minutes) {
            return Err(RenderTimeError::InvalidLocalOffset(local_offset_minutes));
        }
        Ok(Self {
            source,
            local_offset_minutes,
        })
    }
}

impl fmt::Debug for FixedOffsetRenderClock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FixedOffsetRenderClock")
            .field("local_offset_minutes", &self.local_offset_minutes)
            .finish_non_exhaustive()
    }
}

impl RenderClock for FixedOffsetRenderClock {
    fn unix_millis_and_offset(&self) -> (i64, i32) {
        let (unix_ms, _) = self.source.unix_millis_and_offset();
        (unix_ms, self.local_offset_minutes)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedRenderClock {
    unix_ms: i64,
    local_offset_minutes: i32,
}

impl FixedRenderClock {
    pub const fn new(snapshot: RenderTimeSnapshot) -> Self {
        Self {
            unix_ms: snapshot.unix_ms,
            local_offset_minutes: snapshot.local_offset_minutes,
        }
    }
}

impl RenderClock for FixedRenderClock {
    fn unix_millis_and_offset(&self) -> (i64, i32) {
        (self.unix_ms, self.local_offset_minutes)
    }
}

#[cfg(feature = "host")]
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemRenderClock;

#[cfg(feature = "host")]
impl RenderClock for SystemRenderClock {
    fn unix_millis_and_offset(&self) -> (i64, i32) {
        let now = chrono::Local::now();
        (now.timestamp_millis(), now.offset().local_minus_utc() / 60)
    }
}

#[cfg(feature = "host")]
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemRenderSeedSource;

#[cfg(feature = "host")]
impl RenderSeedSource for SystemRenderSeedSource {
    fn next_seed(&self) -> NonZeroU64 {
        static COUNTER: AtomicU64 = AtomicU64::new(1);

        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        let mut mixed = time ^ counter.rotate_left(23);
        mixed ^= mixed >> 30;
        mixed = mixed.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed ^= mixed >> 27;
        mixed = mixed.wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^= mixed >> 31;
        NonZeroU64::new(mixed).unwrap_or(NonZeroU64::MIN)
    }
}

/// Supplies a valid ambient seed exactly when an operation session begins.
pub trait RenderSeedSource: Send + Sync {
    fn next_seed(&self) -> NonZeroU64;
}

#[derive(Debug, Clone, Copy)]
pub struct FixedRenderSeedSource(NonZeroU64);

impl FixedRenderSeedSource {
    pub const fn new(seed: NonZeroU64) -> Self {
        Self(seed)
    }
}

impl RenderSeedSource for FixedRenderSeedSource {
    fn next_seed(&self) -> NonZeroU64 {
        self.0
    }
}

/// Randomness selection before an operation begins.
#[derive(Clone)]
pub enum RenderRandomnessPolicy {
    Pinned(NonZeroU64),
    Ambient(Arc<dyn RenderSeedSource>),
}

impl RenderRandomnessPolicy {
    pub const fn parity() -> Self {
        Self::Pinned(NonZeroU64::MIN)
    }

    pub const fn pinned(seed: NonZeroU64) -> Self {
        Self::Pinned(seed)
    }

    pub fn ambient(source: Arc<dyn RenderSeedSource>) -> Self {
        Self::Ambient(source)
    }

    fn resolve(&self) -> ResolvedRenderSeed {
        match self {
            Self::Pinned(seed) => ResolvedRenderSeed {
                seed: *seed,
                origin: RenderSeedOrigin::Pinned,
            },
            Self::Ambient(source) => ResolvedRenderSeed {
                seed: source.next_seed(),
                origin: RenderSeedOrigin::Ambient,
            },
        }
    }
}

impl fmt::Debug for RenderRandomnessPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pinned(seed) => f.debug_tuple("Pinned").field(seed).finish(),
            Self::Ambient(_) => f.write_str("Ambient(..)"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderSeedOrigin {
    Pinned,
    Ambient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedRenderSeed {
    seed: NonZeroU64,
    origin: RenderSeedOrigin,
}

/// Operation-wide policy for fixture-derived root viewport overrides.
///
/// Families still compute their natural viewport. This policy only decides whether a matching
/// generated override may replace that computation; the policy is frozen before parsing starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootViewportOverridePolicy {
    ApplyGenerated,
    ComputedOnly,
}

impl RootViewportOverridePolicy {
    pub const fn applies_generated(self) -> bool {
        matches!(self, Self::ApplyGenerated)
    }
}

impl ResolvedRenderSeed {
    pub const fn seed(self) -> NonZeroU64 {
        self.seed
    }

    pub const fn origin(self) -> RenderSeedOrigin {
        self.origin
    }
}

/// Immutable adapters and policy shared by render operations.
#[derive(Clone)]
pub struct RenderEnvironment {
    text_measurement: TextMeasurementPolicy,
    math_renderer: Option<Arc<dyn MathRenderer + Send + Sync>>,
    icon_registry: Option<Arc<IconRegistry>>,
    clock: Arc<dyn RenderClock>,
    randomness: RenderRandomnessPolicy,
    resource_limits: RenderResourceLimits,
    root_viewport_overrides: RootViewportOverridePolicy,
}

impl fmt::Debug for RenderEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RenderEnvironment")
            .field("text_measurement", &self.text_measurement)
            .field("has_math_renderer", &self.math_renderer.is_some())
            .field("has_icon_registry", &self.icon_registry.is_some())
            .field("randomness", &self.randomness)
            .field("resource_limits", &self.resource_limits)
            .field("root_viewport_overrides", &self.root_viewport_overrides)
            .finish_non_exhaustive()
    }
}

impl RenderEnvironment {
    pub fn parity() -> Self {
        Self {
            text_measurement: TextMeasurementPolicy::parity(),
            math_renderer: None,
            icon_registry: None,
            clock: Arc::new(FixedRenderClock::new(RenderTimeSnapshot::unix_epoch_utc())),
            randomness: RenderRandomnessPolicy::parity(),
            resource_limits: RenderResourceLimits::interactive(),
            root_viewport_overrides: RootViewportOverridePolicy::ApplyGenerated,
        }
    }

    /// Host defaults preserve Mermaid's current-time and ambient-randomness behavior.
    #[cfg(feature = "host")]
    pub fn host() -> Self {
        Self {
            text_measurement: TextMeasurementPolicy::parity(),
            math_renderer: None,
            icon_registry: None,
            clock: Arc::new(SystemRenderClock),
            randomness: RenderRandomnessPolicy::ambient(Arc::new(SystemRenderSeedSource)),
            resource_limits: RenderResourceLimits::interactive(),
            root_viewport_overrides: RootViewportOverridePolicy::ApplyGenerated,
        }
    }

    pub fn with_text_measurement_policy(mut self, policy: TextMeasurementPolicy) -> Self {
        self.text_measurement = policy;
        self
    }

    pub fn with_math_renderer(mut self, renderer: Arc<dyn MathRenderer + Send + Sync>) -> Self {
        self.math_renderer = Some(renderer);
        self
    }

    pub fn with_icon_registry(mut self, registry: Arc<IconRegistry>) -> Self {
        self.icon_registry = Some(registry);
        self
    }

    pub fn with_clock(mut self, clock: Arc<dyn RenderClock>) -> Self {
        self.clock = clock;
        self
    }

    /// Preserves the configured clock's advancing instant while overriding its local UTC offset.
    pub fn with_fixed_local_offset_minutes(
        mut self,
        local_offset_minutes: i32,
    ) -> Result<Self, RenderTimeError> {
        self.clock = Arc::new(FixedOffsetRenderClock::new(
            self.clock,
            local_offset_minutes,
        )?);
        Ok(self)
    }

    pub fn with_time_snapshot(self, snapshot: RenderTimeSnapshot) -> Self {
        self.with_clock(Arc::new(FixedRenderClock::new(snapshot)))
    }

    pub fn with_randomness(mut self, policy: RenderRandomnessPolicy) -> Self {
        self.randomness = policy;
        self
    }

    pub const fn with_resource_limits(mut self, limits: RenderResourceLimits) -> Self {
        self.resource_limits = limits;
        self
    }

    pub const fn with_root_viewport_override_policy(
        mut self,
        policy: RootViewportOverridePolicy,
    ) -> Self {
        self.root_viewport_overrides = policy;
        self
    }

    /// Freezes time, ambient seed, and provenance collection exactly once per operation.
    pub fn begin_session(&self) -> Result<RenderSession, RenderTimeError> {
        let (unix_ms, offset) = self.clock.unix_millis_and_offset();
        Ok(RenderSession {
            text_measurement: self.text_measurement.clone(),
            measurement_recorder: TextMeasurementRecorder::default(),
            math_renderer: self.math_renderer.clone(),
            icon_registry: self.icon_registry.clone(),
            time: RenderTimeSnapshot::from_unix_millis(unix_ms, offset)?,
            seed: self.randomness.resolve(),
            resource_limits: self.resource_limits,
            root_viewport_overrides: self.root_viewport_overrides,
        })
    }
}

impl Default for RenderEnvironment {
    fn default() -> Self {
        Self::parity()
    }
}

/// Opaque operation session. Family code receives only the narrow projection it needs.
pub struct RenderSession {
    text_measurement: TextMeasurementPolicy,
    measurement_recorder: TextMeasurementRecorder,
    math_renderer: Option<Arc<dyn MathRenderer + Send + Sync>>,
    icon_registry: Option<Arc<IconRegistry>>,
    time: RenderTimeSnapshot,
    seed: ResolvedRenderSeed,
    resource_limits: RenderResourceLimits,
    root_viewport_overrides: RootViewportOverridePolicy,
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
        self.measurement_recorder.report()
    }

    pub const fn time(&self) -> RenderTimeSnapshot {
        self.time
    }

    pub const fn seed(&self) -> ResolvedRenderSeed {
        self.seed
    }

    pub const fn resource_limits(&self) -> RenderResourceLimits {
        self.resource_limits
    }

    pub const fn root_viewport_override_policy(&self) -> RootViewportOverridePolicy {
        self.root_viewport_overrides
    }

    pub fn math_renderer(&self) -> Option<&(dyn MathRenderer + Send + Sync)> {
        self.math_renderer.as_deref()
    }

    pub fn icon_registry(&self) -> Option<&IconRegistry> {
        self.icon_registry.as_deref()
    }

    /// Freezes the observable policy and provenance accumulated so far.
    pub fn report(&self) -> RenderOperationReport {
        RenderOperationReport {
            measurement_routes: self.text_measurement.routes(),
            measurement: self.measurement_recorder.report(),
            time: self.time,
            seed: self.seed,
            root_viewport_overrides: self.root_viewport_overrides,
        }
    }
}

/// Immutable post-operation evidence safe to retain after the opaque session is consumed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOperationReport {
    measurement_routes: [TextMeasurementRoute; 5],
    measurement: TextMeasurementReport,
    time: RenderTimeSnapshot,
    seed: ResolvedRenderSeed,
    root_viewport_overrides: RootViewportOverridePolicy,
}

impl RenderOperationReport {
    pub fn measurement_routes(&self) -> &[TextMeasurementRoute; 5] {
        &self.measurement_routes
    }

    pub fn measurement(&self) -> &TextMeasurementReport {
        &self.measurement
    }

    pub const fn time(&self) -> RenderTimeSnapshot {
        self.time
    }

    pub const fn seed(&self) -> ResolvedRenderSeed {
        self.seed
    }

    pub const fn root_viewport_override_policy(&self) -> RootViewportOverridePolicy {
        self.root_viewport_overrides
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

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

        fn measure_svg_simple_text_bbox_width_for_wrap_px(
            &self,
            _text: &str,
            _style: &TextStyle,
        ) -> f64 {
            11.0
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

        fn measure_wrapped_raw(
            &self,
            _text: &str,
            _style: &TextStyle,
            _max_width: Option<f64>,
            _wrap_mode: WrapMode,
        ) -> TextMetrics {
            metrics(16.0)
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
        let environment = RenderEnvironment::parity()
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
            measurer.measure_svg_simple_text_bbox_width_for_wrap_px("x", &style),
            11.0
        );
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
        assert_eq!(
            measurer
                .measure_wrapped_raw("x", &style, Some(10.0), WrapMode::HtmlLike)
                .width,
            16.0
        );

        let route = session.text_measurement_route(TextMeasurementPhase::SvgBBox);
        assert_eq!(route.primary, profile_identity);
        assert_eq!(session.text_measurement_report().entries().len(), 12);
    }

    #[test]
    fn repeated_measurements_are_aggregated_into_one_bounded_summary() {
        let session = RenderEnvironment::parity()
            .begin_session()
            .expect("begin render session");
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);
        let style = TextStyle::default();

        for _ in 0..10_000 {
            let _ = measurer.measure("same label", &style);
        }

        let report = session.text_measurement_report();
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
        Missing,
        Error,
    }

    struct CountingHost {
        calls: Arc<AtomicUsize>,
        outcome: HostOutcome,
    }

    impl HostTextMeasurer for CountingHost {
        fn measure(
            &self,
            _phase: TextMeasurementPhase,
            _text: &str,
            _style: &TextStyle,
        ) -> HostMeasurementResult<TextMetrics> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            match self.outcome {
                HostOutcome::Measured(metrics) => Ok(Some(metrics)),
                HostOutcome::Missing => Ok(None),
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
            (HostOutcome::Error, Some(HostFallbackReason::Error), 41.0),
        ];

        for (outcome, expected_reason, expected_width) in scenarios {
            let host_calls = Arc::new(AtomicUsize::new(0));
            let fallback_calls = Arc::new(AtomicUsize::new(0));
            let environment = RenderEnvironment::parity()
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

    #[test]
    fn specialized_host_operation_derives_from_the_phase_aware_measurement() {
        let host_calls = Arc::new(AtomicUsize::new(0));
        let fallback_calls = Arc::new(AtomicUsize::new(0));
        let environment = RenderEnvironment::parity().with_text_measurement_policy(host_policy(
            HostOutcome::Measured(metrics(73.0)),
            &host_calls,
            &fallback_calls,
        ));
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

        for invalid_metrics in [
            TextMetrics {
                width: -1.0,
                height: 10.0,
                line_count: 1,
            },
            TextMetrics {
                width: f64::NAN,
                height: 10.0,
                line_count: 1,
            },
            TextMetrics {
                width: 10.0,
                height: 10.0,
                line_count: 0,
            },
        ] {
            let host_calls = Arc::new(AtomicUsize::new(0));
            let fallback_calls = Arc::new(AtomicUsize::new(0));
            let environment =
                RenderEnvironment::parity().with_text_measurement_policy(host_policy(
                    HostOutcome::Measured(invalid_metrics),
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
        fn measure(
            &self,
            _phase: TextMeasurementPhase,
            _text: &str,
            _style: &TextStyle,
        ) -> HostMeasurementResult<TextMetrics> {
            Ok(None)
        }

        fn measure_svg_text_bbox_x(
            &self,
            _phase: TextMeasurementPhase,
            _text: &str,
            _style: &TextStyle,
        ) -> HostMeasurementResult<(f64, f64)> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(Some(self.value))
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
            let session = RenderEnvironment::parity()
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
    fn time_snapshot_derives_date_from_instant_and_offset() {
        let utc = RenderTimeSnapshot::from_unix_millis(0, 0).expect("UTC epoch");
        let west = RenderTimeSnapshot::from_unix_millis(0, -60).expect("west of UTC epoch");

        assert_eq!(
            utc.local_date(),
            NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()
        );
        assert_eq!(
            west.local_date(),
            NaiveDate::from_ymd_opt(1969, 12, 31).unwrap()
        );
        assert!(RenderTimeSnapshot::from_unix_millis(0, 1440).is_err());
    }

    struct CountingClock {
        calls: Arc<AtomicUsize>,
    }

    struct AdvancingClock(AtomicUsize);

    impl RenderClock for AdvancingClock {
        fn unix_millis_and_offset(&self) -> (i64, i32) {
            let tick = self.0.fetch_add(1, Ordering::Relaxed) as i64;
            (tick * 86_400_000, -60)
        }
    }

    #[test]
    fn fixed_offset_clock_preserves_advancing_instants_across_sessions() {
        let environment = RenderEnvironment::parity()
            .with_clock(Arc::new(AdvancingClock(AtomicUsize::new(0))))
            .with_fixed_local_offset_minutes(480)
            .expect("valid fixed offset");

        let first = environment.begin_session().expect("first session");
        let second = environment.begin_session().expect("second session");

        assert_eq!(first.time().unix_ms(), 0);
        assert_eq!(second.time().unix_ms(), 86_400_000);
        assert_eq!(first.time().local_offset_minutes(), 480);
        assert_eq!(second.time().local_offset_minutes(), 480);
        assert_eq!(
            first.time().local_date(),
            NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()
        );
        assert_eq!(
            second.time().local_date(),
            NaiveDate::from_ymd_opt(1970, 1, 2).unwrap()
        );
        assert!(
            RenderEnvironment::parity()
                .with_fixed_local_offset_minutes(1440)
                .is_err()
        );
    }

    impl RenderClock for CountingClock {
        fn unix_millis_and_offset(&self) -> (i64, i32) {
            self.calls.fetch_add(1, Ordering::Relaxed);
            (0, 8 * 60)
        }
    }

    struct CountingSeedSource {
        calls: Arc<AtomicUsize>,
        seed: NonZeroU64,
    }

    impl RenderSeedSource for CountingSeedSource {
        fn next_seed(&self) -> NonZeroU64 {
            self.calls.fetch_add(1, Ordering::Relaxed);
            self.seed
        }
    }

    #[test]
    fn session_freezes_clock_and_ambient_seed_once_and_exposes_narrow_services() {
        let clock_calls = Arc::new(AtomicUsize::new(0));
        let seed_calls = Arc::new(AtomicUsize::new(0));
        let limits = RenderResourceLimits::trusted_native();
        let environment = RenderEnvironment::parity()
            .with_clock(Arc::new(CountingClock {
                calls: Arc::clone(&clock_calls),
            }))
            .with_randomness(RenderRandomnessPolicy::ambient(Arc::new(
                CountingSeedSource {
                    calls: Arc::clone(&seed_calls),
                    seed: NonZeroU64::new(77).unwrap(),
                },
            )))
            .with_math_renderer(Arc::new(crate::math::NoopMathRenderer))
            .with_icon_registry(Arc::new(IconRegistry::new()))
            .with_resource_limits(limits)
            .with_root_viewport_override_policy(RootViewportOverridePolicy::ComputedOnly);

        let session = environment.begin_session().expect("begin render session");
        assert_eq!(session.time().unix_ms(), 0);
        assert_eq!(session.time().local_offset_minutes(), 8 * 60);
        assert_eq!(session.seed().seed(), NonZeroU64::new(77).unwrap());
        assert_eq!(session.seed().origin(), RenderSeedOrigin::Ambient);
        assert_eq!(session.resource_limits(), limits);
        assert_eq!(
            session.root_viewport_override_policy(),
            RootViewportOverridePolicy::ComputedOnly
        );
        assert!(session.math_renderer().is_some());
        assert!(session.icon_registry().is_some());
        assert_eq!(
            session.report().root_viewport_override_policy(),
            RootViewportOverridePolicy::ComputedOnly
        );

        assert_eq!(session.seed().seed(), NonZeroU64::new(77).unwrap());
        assert_eq!(clock_calls.load(Ordering::Relaxed), 1);
        assert_eq!(seed_calls.load(Ordering::Relaxed), 1);
        assert!(NonZeroU64::new(0).is_none());
        assert_eq!(
            RenderRandomnessPolicy::parity().resolve().seed(),
            NonZeroU64::MIN
        );
    }
}

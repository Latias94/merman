//! Named measurement policy, provenance, and operation routing.

use super::super::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle, WrapMode};
#[cfg(feature = "svg-output")]
use super::trusted::BuiltinSvgComputedLength;
use super::trusted::{
    BuiltinHtmlLineWidth, BuiltinTextMeasurementOperationCarrier, BuiltinTextMeasurementProfile,
    TrustedMeasurementQuery,
};
use super::{
    HostFallbackReason, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    TextMeasurementOperation, TextMeasurementPhase, operation_index, phase_index,
    validate_host_text_measurement,
};
use crate::resources::OperationWorkMeter;
use merman_model::OperationPhase;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

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

#[cfg(feature = "layout-shoal")]
fn calibrated_profile() -> &'static TextMeasurementProfile {
    static PROFILE: std::sync::OnceLock<TextMeasurementProfile> = std::sync::OnceLock::new();
    PROFILE.get_or_init(|| {
        let profile = MeasurementProfileId::new("merman.shoal-calibrated-text")
            .expect("static calibrated profile id is valid");
        let identity = TextMeasurementProfileIdentity::new(
            profile,
            concat!("merman-render@", env!("CARGO_PKG_VERSION")),
        )
        .expect("static calibrated profile version is valid");
        TextMeasurementProfile::new_builtin(
            identity,
            Arc::new(super::super::deterministic::CalibratedTextMeasurer),
            BuiltinTextMeasurementProfile::Calibrated,
        )
    })
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
        (phase_index(phase) * TextMeasurementOperation::ALL.len() + operation_index(operation))
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
        let _ = counter.try_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
            Some(count.saturating_add(1))
        });
    }

    fn has_host_measurements(&self) -> bool {
        TextMeasurementPhase::ALL.into_iter().any(|phase| {
            TextMeasurementOperation::ALL.into_iter().any(|operation| {
                self.counts[Self::slot(phase, operation, TextMeasurementRouteOutcome::Host)]
                    .load(Ordering::Relaxed)
                    != 0
            })
        })
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

                    let provenance = match (&policy.routes[phase_index(phase)], outcome) {
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
        self.routes[phase_index(phase)] = TextMeasurementRouteConfig::Profile(profile);
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
            policy.routes[phase_index(phase)] = TextMeasurementRouteConfig::Host {
                identity: identity.clone(),
                backend: Arc::clone(&host),
                fallback: fallback.clone(),
            };
        }
        policy
    }

    pub fn route(&self, phase: TextMeasurementPhase) -> TextMeasurementRoute {
        match &self.routes[phase_index(phase)] {
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

/// Measurement state for one render operation.
#[doc(hidden)]
pub struct TextMeasurementSession {
    policy: TextMeasurementPolicy,
    // Keep movable family artifacts compact for bounded worker stacks.
    recorder: Box<TextMeasurementRecorder>,
}

impl TextMeasurementSession {
    pub fn new(policy: TextMeasurementPolicy) -> Self {
        Self {
            policy,
            recorder: Box::default(),
        }
    }

    #[cfg(feature = "layout-shoal")]
    pub fn use_shoal_text_widths(&mut self) {
        for route in &mut self.policy.routes {
            match route {
                TextMeasurementRouteConfig::Profile(profile)
                | TextMeasurementRouteConfig::Host {
                    fallback: profile, ..
                } if profile.builtin == Some(BuiltinTextMeasurementProfile::Deterministic) => {
                    *profile = calibrated_profile().clone();
                }
                _ => {}
            }
        }
    }

    pub fn routed<'a>(
        &'a self,
        work_meter: &'a OperationWorkMeter,
        default_phase: TextMeasurementPhase,
        controlled_operation_phase: Option<OperationPhase>,
    ) -> RoutedTextMeasurer<'a> {
        RoutedTextMeasurer {
            default_phase,
            policy: &self.policy,
            recorder: &self.recorder,
            work_meter,
            controlled_operation_phase,
        }
    }

    pub fn route(&self, phase: TextMeasurementPhase) -> TextMeasurementRoute {
        self.policy.route(phase)
    }

    pub fn routes(&self) -> [TextMeasurementRoute; 4] {
        self.policy.routes()
    }

    pub fn has_host_measurements(&self) -> bool {
        self.recorder.has_host_measurements()
    }

    pub fn report(&self) -> TextMeasurementReport {
        self.recorder.report(&self.policy)
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

impl CancelledTextMeasurement for Option<f64> {
    fn cancelled() -> Self {
        None
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
            | TextMeasurementOperation::RawBBoxYOffset
            | TextMeasurementOperation::RawMiddleBBoxYOffset
            | TextMeasurementOperation::RawHangingBBoxYOffset
            | TextMeasurementOperation::BoundingClientRectWidth
            | TextMeasurementOperation::TspanBBoxWidth
            | TextMeasurementOperation::TspanBBoxHeight
            | TextMeasurementOperation::CreateTextBBoxYOffset
            | TextMeasurementOperation::CreateTextMiddleBBoxYOffset
            | TextMeasurementOperation::MermaidCalculateTextDimensions
            | TextMeasurementOperation::SimpleBBoxHeight => TextMeasurementPhase::SvgBBox,
            TextMeasurementOperation::CanvasMeasureTextWidth => TextMeasurementPhase::Layout,
            TextMeasurementOperation::WrapProbeBBoxWidth => TextMeasurementPhase::Wrap,
            TextMeasurementOperation::Wrapped
            | TextMeasurementOperation::WrappedWithRawWidth
            | TextMeasurementOperation::HtmlLineHeight => TextMeasurementPhase::Wrap,
            TextMeasurementOperation::Measure => self.default_phase,
        }
    }

    fn builtin_operation_carrier(
        &self,
        operation: TextMeasurementOperation,
    ) -> Option<BuiltinTextMeasurementOperationCarrier> {
        self.builtin_operation_carrier_for_phase(self.phase_for(operation), operation)
    }

    fn builtin_operation_carrier_for_phase(
        &self,
        phase: TextMeasurementPhase,
        operation: TextMeasurementOperation,
    ) -> Option<BuiltinTextMeasurementOperationCarrier> {
        match &self.policy.routes[phase_index(phase)] {
            TextMeasurementRouteConfig::Profile(profile) => profile.builtin.map(|profile| {
                BuiltinTextMeasurementOperationCarrier::new(profile, phase, operation)
            }),
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
        #[cfg(feature = "benchmark-stages")]
        let _stage =
            merman_model::benchmark_stages::enter(merman_model::benchmark_stages::Stage::Measure);
        let phase = request.phase;
        let operation = request.operation;
        if let Some(cancelled) = self.controlled_cancellation_value() {
            return cancelled;
        }
        match &self.policy.routes[phase_index(phase)] {
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
                let attempt = backend.measure_with_control(request, self.work_meter.control());
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
                    Err(error) => error.fallback_reason,
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
        _: TrustedMeasurementQuery,
        operation: TextMeasurementOperation,
    ) -> Option<BuiltinTextMeasurementOperationCarrier> {
        RoutedTextMeasurer::builtin_operation_carrier(self, operation)
    }

    #[cfg(feature = "svg-output")]
    #[allow(private_interfaces)]
    fn begin_svg_text_computed_length(
        &self,
        _: TrustedMeasurementQuery,
        style: &TextStyle,
    ) -> Option<BuiltinSvgComputedLength> {
        self.builtin_operation_carrier(TextMeasurementOperation::ComputedLength)
            .and_then(|carrier| carrier.into_svg_computed_length(style))
    }

    #[allow(private_interfaces)]
    fn begin_html_line_width(
        &self,
        _: TrustedMeasurementQuery,
        style: &TextStyle,
    ) -> Option<BuiltinHtmlLineWidth> {
        self.builtin_operation_carrier_for_phase(
            TextMeasurementPhase::Wrap,
            TextMeasurementOperation::CanvasMeasureTextWidth,
        )
        .and_then(|carrier| carrier.into_html_line_width(style))
    }

    fn measure_html_line_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            self.request(
                TextMeasurementOperation::HtmlLineHeight,
                text,
                style,
                None,
                WrapMode::HtmlLike,
            ),
            decode_host_length,
            |profile| profile.measure_html_line_height_px(text, style),
        )
    }

    fn measure_html_line_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        // A candidate is already decoded, single-line text. Use its raw advance without
        // normalizing it as a complete HTML label, while retaining the Wrap route and callbacks.
        self.resolve(
            HostTextMeasurementRequest {
                operation: TextMeasurementOperation::CanvasMeasureTextWidth,
                phase: TextMeasurementPhase::Wrap,
                text,
                style,
                max_width: None,
                wrap_mode: WrapMode::HtmlLike,
            },
            decode_host_length,
            |profile| profile.measure_html_line_width_px(text, style),
        )
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
                WrapMode::HtmlLike,
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
                WrapMode::HtmlLike,
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

    fn measure_svg_preserved_raw_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            self.request(
                TextMeasurementOperation::RawBBoxWidth,
                text,
                style,
                None,
                WrapMode::HtmlLike,
            ),
            decode_host_length,
            |profile| profile.measure_svg_preserved_raw_text_bbox_width_px(text, style),
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

    fn measure_svg_raw_text_bbox_y_offset_px(&self, text: &str, style: &TextStyle) -> Option<f64> {
        self.resolve(
            self.request(
                TextMeasurementOperation::RawBBoxYOffset,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            |value| decode_host_length(value).map(Some),
            |profile| profile.measure_svg_raw_text_bbox_y_offset_px(text, style),
        )
    }

    fn measure_svg_raw_text_middle_bbox_y_offset_px(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> Option<f64> {
        self.resolve(
            self.request(
                TextMeasurementOperation::RawMiddleBBoxYOffset,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            |value| decode_host_length(value).map(Some),
            |profile| profile.measure_svg_raw_text_middle_bbox_y_offset_px(text, style),
        )
    }

    fn measure_svg_raw_text_hanging_bbox_y_offset_px(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> Option<f64> {
        self.resolve(
            self.request(
                TextMeasurementOperation::RawHangingBBoxYOffset,
                text,
                style,
                None,
                WrapMode::SvgLike,
            ),
            |value| decode_host_length(value).map(Some),
            |profile| profile.measure_svg_raw_text_hanging_bbox_y_offset_px(text, style),
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

    fn measure_svg_preserved_tspan_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.resolve(
            self.request(
                TextMeasurementOperation::TspanBBoxWidth,
                text,
                style,
                None,
                WrapMode::HtmlLike,
            ),
            decode_host_length,
            |profile| profile.measure_svg_preserved_tspan_text_bbox_width_px(text, style),
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

#[cfg(all(test, feature = "svg-parse"))]
mod tests;

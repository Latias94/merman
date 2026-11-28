//! Operation-owned render services and deterministic policy.

use crate::math::{ConfiguredMathBackend, MathRenderer};
use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
use crate::svg::IconRegistry;
use crate::text::{FontCatalog, FontCatalogFingerprint, FontSourcePolicy};
use crate::theme::ThemePortabilityRequirement;

#[cfg(all(test, feature = "all-diagrams"))]
use crate::text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle, WrapMode};
use crate::{RenderCapability, RenderCapabilityPolicy};
use merman_model::runtime::{OperationContext, OperationTiming, RuntimePolicy, RuntimePolicyError};
use merman_model::time::LocalTimeZoneProvenance;
use merman_model::{OperationControl, OperationPhase};
use merman_render_core::text::measurement::TextMeasurementSession;
pub use merman_render_core::text::measurement::{
    HostFallbackReason, HostMeasurementResult, HostTextMeasurement, HostTextMeasurementError,
    HostTextMeasurementRequest, HostTextMeasurer, InvalidMeasurementProfileIdentity,
    MeasurementProfileId, RoutedTextMeasurer, TEXT_MEASUREMENT_PROTOCOL_VERSION,
    TextMeasurementOperation, TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity, TextMeasurementProvenance, TextMeasurementReport,
    TextMeasurementResultKind, TextMeasurementRoute, TextMeasurementSource, TextMeasurementSummary,
    validate_host_text_measurement,
};
use std::fmt;
use std::num::NonZeroU64;
use std::sync::Arc;
#[cfg(feature = "layout-shoal")]
use std::sync::Weak;

#[cfg(feature = "layout-shoal")]
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        reason = "Only the shared layered Shoal solver uses the scratch pool."
    )
)]
mod shoal_scratch;
#[cfg(feature = "layout-shoal")]
use shoal_scratch::ShoalScratchPool;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RenderEnvironmentError {
    #[error(transparent)]
    Cancelled(#[from] merman_model::OperationCancelled),
    #[error(transparent)]
    Runtime(#[from] RuntimePolicyError),
}

#[cfg(feature = "math")]
fn default_math_backend() -> Option<ConfiguredMathBackend> {
    Some(ConfiguredMathBackend::compiled_ratex())
}

#[cfg(not(feature = "math"))]
fn default_math_backend() -> Option<ConfiguredMathBackend> {
    None
}

/// Immutable render services and the policy used to capture one operation context.
#[derive(Clone)]
pub struct RenderEnvironment {
    text_measurement: TextMeasurementPolicy,
    capability_policy: RenderCapabilityPolicy,
    math_backend: Option<ConfiguredMathBackend>,
    icon_registry: Option<IconRegistry>,
    runtime_policy: RuntimePolicy,
    resource_policy: RenderResourcePolicy,
    #[cfg(feature = "layout-shoal")]
    shoal_scratch: Option<Arc<ShoalScratchPool>>,
    font_catalog: FontCatalog,
    font_source_policy: FontSourcePolicy,
    trusted_theme_css: bool,
    theme_portability: ThemePortabilityRequirement,
}

impl fmt::Debug for RenderEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RenderEnvironment")
            .field("text_measurement", &self.text_measurement)
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
            .field("trusted_theme_css", &self.trusted_theme_css)
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

            capability_policy: RenderCapabilityPolicy::unrestricted(),
            math_backend: default_math_backend(),
            icon_registry: None,
            runtime_policy: RuntimePolicy::deterministic(),
            resource_policy: RenderResourcePolicy::interactive(),
            #[cfg(feature = "layout-shoal")]
            shoal_scratch: None,
            font_catalog: FontCatalog::system_fonts(),
            font_source_policy: FontSourcePolicy::default(),
            trusted_theme_css: false,
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

    /// Returns the configured routes without creating an operation session.
    #[doc(hidden)]
    pub fn text_measurement_routes(&self) -> [TextMeasurementRoute; 4] {
        self.text_measurement.routes()
    }

    /// Restricts optional renderer capabilities for every operation begun by this environment.
    ///
    /// This is primarily useful to artifact owners whose public feature contract can be narrower
    /// than Cargo's resolved dependency feature union.
    pub const fn with_capability_policy(mut self, policy: RenderCapabilityPolicy) -> Self {
        self.capability_policy = policy;
        self
    }

    #[doc(hidden)]
    pub const fn capability_policy(&self) -> RenderCapabilityPolicy {
        self.capability_policy
    }

    /// Installs the math renderer compiled into this renderer, if present.
    ///
    /// Facades use this to select the canonical compiled capability instead of duplicating Cargo
    /// feature checks in each transport layer.
    pub fn with_compiled_math_renderer(mut self) -> Self {
        self.math_backend = default_math_backend();
        self
    }

    #[cfg(test)]
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

    /// Borrows the policy that will be captured by the next session.
    #[doc(hidden)]
    pub const fn resource_policy(&self) -> &RenderResourcePolicy {
        &self.resource_policy
    }

    /// Reuses row storage on independent Shoal layered layouts, including width fitting.
    ///
    /// This environment and its clones share one idle slot with at most 64 KiB of row storage.
    /// Keep an environment alive across operations to reuse it. Each operation still owns its
    /// session and resource accounting, and preview frames keep their own storage.
    #[cfg(feature = "layout-shoal")]
    pub fn with_shoal_scratch_reuse(mut self) -> Self {
        self.shoal_scratch.get_or_insert_with(Arc::default);
        self
    }

    /// Retains the font catalog mode authorized for layout evidence and native export.
    ///
    /// [`FontCatalog::system_fonts`] does not enumerate or freeze host fonts.
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

    /// Allows document-provided `themeCSS` in a trusted host environment.
    pub const fn with_trusted_theme_css(mut self, allow: bool) -> Self {
        self.trusted_theme_css = allow;
        self
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
        self.begin_session_in_context(operation_context, control)
    }

    /// Begins a render session with a compiled native theme.
    pub fn begin_session_with_native_theme(
        &self,
        theme: crate::theme::NativeTheme,
    ) -> Result<RenderSession, RenderEnvironmentError> {
        let mut session = self.begin_session()?;
        session.native_theme = Some(theme);
        Ok(session)
    }

    /// Begins a native-themed render session using caller-captured operation state.
    pub fn begin_session_with_native_theme_in_context(
        &self,
        theme: crate::theme::NativeTheme,
        operation_context: OperationContext,
        control: OperationControl,
    ) -> Result<RenderSession, RenderEnvironmentError> {
        let mut session = self.begin_session_in_context(operation_context, control)?;
        session.native_theme = Some(theme);
        Ok(session)
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
        Ok(RenderSession {
            text_measurement: TextMeasurementSession::new(self.text_measurement.clone()),
            capability_policy: self.capability_policy,
            math_backend: self.math_backend.clone(),
            icon_registry: self.icon_registry.clone(),
            operation_context,
            resource_policy: self.resource_policy,
            work_meter: Arc::new(OperationWorkMeter::new_with_control(
                self.resource_policy,
                control,
            )),
            #[cfg(feature = "layout-shoal")]
            shoal_scratch: self
                .shoal_scratch
                .as_ref()
                .map_or_else(Weak::new, Arc::downgrade),
            font_catalog: self.font_catalog.clone(),
            font_source_policy: self.font_source_policy.clone(),
            trusted_theme_css: self.trusted_theme_css,
            portability_requirement: self.theme_portability,
            native_theme: None,
            canvas_background: None,
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
    text_measurement: TextMeasurementSession,
    capability_policy: RenderCapabilityPolicy,
    math_backend: Option<ConfiguredMathBackend>,
    icon_registry: Option<IconRegistry>,
    operation_context: OperationContext,
    resource_policy: RenderResourcePolicy,
    work_meter: Arc<OperationWorkMeter>,
    // A prepared artifact can outlive its operation's environment without retaining the pool.
    #[cfg(feature = "layout-shoal")]
    #[allow(
        dead_code,
        reason = "Only the shared layered Shoal solver reads the session pool."
    )]
    shoal_scratch: Weak<ShoalScratchPool>,
    font_catalog: FontCatalog,
    font_source_policy: FontSourcePolicy,
    trusted_theme_css: bool,
    portability_requirement: ThemePortabilityRequirement,
    native_theme: Option<crate::theme::NativeTheme>,
    canvas_background: Option<crate::theme::Color>,
}

impl RenderSession {
    #[cfg(feature = "layout-shoal")]
    pub(crate) fn use_shoal_text_widths(&mut self) {
        self.text_measurement.use_shoal_text_widths();
    }

    /// Native settings selected for this operation.
    pub fn native_theme(&self) -> Option<&crate::theme::NativeTheme> {
        self.native_theme.as_ref()
    }

    /// Overrides the SVG canvas color for this session.
    pub fn with_canvas_background(mut self, color: crate::theme::Color) -> Self {
        self.canvas_background = Some(color);
        self
    }

    pub(crate) fn canvas_background(&self) -> Option<crate::theme::Color> {
        self.canvas_background
            .or_else(|| self.native_theme()?.background())
    }

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
        self.text_measurement.routed(
            self.work_meter.as_ref(),
            default_phase,
            controlled_operation_phase,
        )
    }

    pub fn text_measurement_route(&self, phase: TextMeasurementPhase) -> TextMeasurementRoute {
        self.text_measurement.route(phase)
    }

    pub(crate) fn has_host_measurements(&self) -> bool {
        self.text_measurement.has_host_measurements()
    }

    pub fn text_measurement_report(&self) -> TextMeasurementReport {
        self.text_measurement.report()
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

    pub const fn local_date(&self) -> merman_model::time::CivilDate {
        self.operation_context.today_local()
    }

    pub fn local_time_zone(&self) -> &merman_model::time::LocalTimeZone {
        self.operation_context.local_time_zone()
    }

    pub fn render_seed(&self) -> NonZeroU64 {
        self.operation_context.derive_nonzero_u64("render.root", 0)
    }

    pub const fn resource_policy(&self) -> RenderResourcePolicy {
        self.resource_policy
    }

    pub const fn font_catalog(&self) -> &FontCatalog {
        &self.font_catalog
    }

    pub const fn font_source_policy(&self) -> &FontSourcePolicy {
        &self.font_source_policy
    }

    /// Returns the host portability requirement frozen for this operation.
    pub const fn portability_requirement(&self) -> ThemePortabilityRequirement {
        self.portability_requirement
    }

    pub(crate) const fn allows_trusted_theme_css(&self) -> bool {
        self.trusted_theme_css
    }

    /// Reports effective operation availability after policy and backend/service resolution.
    pub(crate) fn supports_capability(&self, capability: RenderCapability) -> bool {
        if !self.capability_policy.allows(capability) {
            return false;
        }
        match capability {
            RenderCapability::LayoutCytoscape => crate::layout_cytoscape_available(),
            RenderCapability::LayoutElk => crate::layout_elk_available(),
            RenderCapability::LayoutDagre => crate::layout_dagre_available(),
            RenderCapability::LayoutGrid => crate::layout_grid_available(),
            RenderCapability::LayoutLayered => crate::layout_layered_available(),
            RenderCapability::LayoutShoal => crate::layout_shoal_available(),
            RenderCapability::Math => self
                .math_backend
                .as_ref()
                .is_some_and(|backend| backend.supports_resource_policy(self.resource_policy)),
            _ => false,
        }
    }

    pub(crate) fn work_meter(&self) -> &Arc<OperationWorkMeter> {
        &self.work_meter
    }

    #[cfg(feature = "layout-shoal")]
    #[allow(
        dead_code,
        reason = "Only the shared layered Shoal solver reads the session pool."
    )]
    pub(crate) fn shoal_scratch(&self) -> Option<Arc<ShoalScratchPool>> {
        self.shoal_scratch.upgrade()
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
            measurement: self.text_measurement.report(),
            operation_context: self.operation_context.clone(),
            local_time_zone: self
                .operation_context
                .local_time_zone()
                .provenance()
                .clone(),
            resource_policy: self.resource_policy,
            layout_work_units: self.work_meter.used(),
            prepared_text_retained_bytes_peak: self.work_meter.prepared_text_retained_bytes_peak(),
            portability_requirement: self.portability_requirement,
            font_catalog_fingerprint: self.font_catalog().fingerprint(),
            font_source_policy: self.font_source_policy().clone(),
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
    layout_work_units: usize,
    prepared_text_retained_bytes_peak: usize,
    portability_requirement: ThemePortabilityRequirement,
    font_catalog_fingerprint: FontCatalogFingerprint,
    font_source_policy: FontSourcePolicy,
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

    pub const fn local_date(&self) -> merman_model::time::CivilDate {
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

    /// Returns the host portability requirement frozen for this operation.
    pub const fn portability_requirement(&self) -> ThemePortabilityRequirement {
        self.portability_requirement
    }

    pub const fn font_catalog_fingerprint(&self) -> FontCatalogFingerprint {
        self.font_catalog_fingerprint
    }

    pub const fn font_source_policy(&self) -> &FontSourcePolicy {
        &self.font_source_policy
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

#[cfg(all(test, feature = "all-diagrams"))]
mod tests {
    use super::*;
    use crate::resources::ResourceLimitId;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn deterministic_profile_identity_tracks_the_render_crate() {
        let policy = TextMeasurementPolicy::deterministic();
        let profile = policy.route(TextMeasurementPhase::Layout).primary;

        assert_eq!(profile.profile().as_str(), "merman.deterministic-text");
        assert_eq!(
            profile.version(),
            concat!("merman-render@", env!("CARGO_PKG_VERSION"))
        );
    }

    #[cfg(feature = "layout-shoal")]
    #[test]
    fn shoal_calibrates_builtin_widths_and_keeps_explicit_measurement_profiles() {
        let custom = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.custom").unwrap(),
                "1",
            )
            .unwrap(),
            Arc::new(DeterministicTextMeasurer::default()),
        );
        let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
            TextMeasurementPolicy::deterministic()
                .with_profile_for_phase(TextMeasurementPhase::SvgBBox, custom),
        );
        let mut session = environment.begin_session().unwrap();
        let explicit_route = session.text_measurement_route(TextMeasurementPhase::SvgBBox);
        session.use_shoal_text_widths();
        assert_eq!(
            session.text_measurement_route(TextMeasurementPhase::SvgBBox),
            explicit_route,
        );
        assert_eq!(
            session
                .text_measurement_route(TextMeasurementPhase::Layout)
                .primary
                .profile()
                .as_str(),
            "merman.shoal-calibrated-text"
        );

        let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
        let regular = TextStyle::default();
        let bold = TextStyle {
            font_weight: Some("bolder".into()),
            ..regular.clone()
        };
        let text = "customer_account_id";
        let wide = measurer.measure_html_line_width_px(text, &regular);
        assert!(wide >= 9.58 * regular.font_size);
        let bold_width = measurer.measure_html_line_width_px(text, &bold);
        assert!((bold_width / wide - 1.06).abs() < 1e-12);
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
                (19, "raw-bbox-y-offset"),
                (20, "raw-middle-bbox-y-offset"),
                (21, "raw-hanging-bbox-y-offset"),
                (22, "html-line-height"),
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
        assert_eq!(cancelled.reason, merman_model::CancelReason::Requested);
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

    #[test]
    fn literal_html_line_height_preserves_payload_phase_and_routed_fallback() {
        struct LineHost(bool);
        impl HostTextMeasurer for LineHost {
            fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
                assert_eq!(request.operation, TextMeasurementOperation::HtmlLineHeight);
                assert_eq!(request.phase, TextMeasurementPhase::Wrap);
                assert_eq!(request.text, "A<br>B&amp;C");
                assert_eq!(request.wrap_mode, WrapMode::HtmlLike);
                assert_eq!(request.max_width, None);
                Ok(self.0.then_some(HostTextMeasurement::Length(31.5)))
            }
        }
        for handled in [false, true] {
            let policy = TextMeasurementPolicy::host_display(
                identity("test.literal-html-line-height", "v1", &[]),
                Arc::new(LineHost(handled)),
                [TextMeasurementPhase::Wrap],
            );
            let session = RenderEnvironment::deterministic()
                .with_text_measurement_policy(policy)
                .begin_session()
                .unwrap();
            let style = TextStyle {
                font_size: 16.0,
                ..TextStyle::default()
            };
            let height = session
                .text_measurer(TextMeasurementPhase::Layout)
                .measure_html_line_height_px("A<br>B&amp;C", &style);
            assert_eq!(height, if handled { 31.5 } else { 24.0 });
        }
    }

    struct OperationAwareHost {
        operations: Arc<Mutex<Vec<TextMeasurementOperation>>>,
    }

    #[test]
    fn preserved_svg_width_reuses_existing_operations_with_explicit_whitespace_mode() {
        struct PreservedTspanHost(TextMeasurementOperation);

        impl HostTextMeasurer for PreservedTspanHost {
            fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
                assert_eq!(request.operation, self.0);

                assert_eq!(
                    request.phase,
                    if self.0 == TextMeasurementOperation::ComputedLength {
                        TextMeasurementPhase::ComputedLength
                    } else {
                        TextMeasurementPhase::SvgBBox
                    }
                );
                assert_eq!(request.text, "a  b");
                assert_eq!(request.max_width, None);
                assert_eq!(request.wrap_mode, WrapMode::HtmlLike);
                Ok(Some(if self.0 == TextMeasurementOperation::BBoxX {
                    HostTextMeasurement::HorizontalExtents {
                        left: 18.0,
                        right: 19.5,
                    }
                } else {
                    HostTextMeasurement::Length(37.5)
                }))
            }
        }

        for operation in [
            TextMeasurementOperation::ComputedLength,
            TextMeasurementOperation::BBoxX,
            TextMeasurementOperation::RawBBoxWidth,
            TextMeasurementOperation::TspanBBoxWidth,
        ] {
            let policy = TextMeasurementPolicy::host_display(
                identity("test.preserved-tspan-host", "v1", &[]),
                Arc::new(PreservedTspanHost(operation)),
                [
                    TextMeasurementPhase::SvgBBox,
                    TextMeasurementPhase::ComputedLength,
                ],
            );
            let session = RenderEnvironment::deterministic()
                .with_text_measurement_policy(policy)
                .begin_session()
                .expect("begin render session");
            let measurer = session.text_measurer(TextMeasurementPhase::Layout);
            let actual = match operation {
                TextMeasurementOperation::ComputedLength => {
                    measurer.measure_svg_text_computed_length_px("a  b", &TextStyle::default())
                }
                TextMeasurementOperation::BBoxX => {
                    let (left, right) =
                        measurer.measure_svg_text_bbox_x("a  b", &TextStyle::default());
                    left + right
                }
                TextMeasurementOperation::RawBBoxWidth => measurer
                    .measure_svg_preserved_raw_text_bbox_width_px("a  b", &TextStyle::default()),
                _ => measurer
                    .measure_svg_preserved_tspan_text_bbox_width_px("a  b", &TextStyle::default()),
            };
            assert_eq!(actual, 37.5);
        }
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
        let policy = TextMeasurementPolicy::host_display(
            identity("test.operation-aware-host", "v1", &[]),
            Arc::new(OperationAwareHost {
                operations: Arc::clone(&operations),
            }),
            [TextMeasurementPhase::ComputedLength],
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
        let policy = TextMeasurementPolicy::host_display(
            identity("test.operation-aware-host", "v1", &[]),
            Arc::new(OperationAwareHost {
                operations: Arc::clone(&operations),
            }),
            [TextMeasurementPhase::SvgBBox],
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
        let policy = TextMeasurementPolicy::host_display(
            identity("test.operation-aware-host", "v1", &[]),
            Arc::new(OperationAwareHost {
                operations: Arc::clone(&operations),
            }),
            [TextMeasurementPhase::SvgBBox],
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
        let policy = TextMeasurementPolicy::host_display(
            identity("test.operation-aware-host", "v1", &[]),
            Arc::new(OperationAwareHost {
                operations: Arc::clone(&operations),
            }),
            [TextMeasurementPhase::SvgBBox],
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
        let policy = TextMeasurementPolicy::host_display(
            identity("test.operation-aware-host", "v1", &[]),
            Arc::new(OperationAwareHost {
                operations: Arc::clone(&operations),
            }),
            [TextMeasurementPhase::SvgBBox, TextMeasurementPhase::Layout],
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

    #[test]
    fn decoded_html_line_probe_keeps_wrap_canvas_host_routing_and_fallback_provenance() {
        struct DecodedLineHost {
            calls: Arc<AtomicUsize>,
            measurement: Option<HostTextMeasurement>,
        }

        impl HostTextMeasurer for DecodedLineHost {
            fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
                self.calls.fetch_add(1, Ordering::Relaxed);
                assert_eq!(
                    request.operation,
                    TextMeasurementOperation::CanvasMeasureTextWidth
                );
                assert_eq!(request.phase, TextMeasurementPhase::Wrap);
                assert_eq!(request.text, "a<br>b");
                assert_eq!(request.max_width, None);
                assert_eq!(request.wrap_mode, WrapMode::HtmlLike);
                Ok(self.measurement)
            }
        }

        let style = TextStyle::default();
        let text = "a<br>b";
        let fallback_width =
            DeterministicTextMeasurer::default().measure_html_line_width_px(text, &style);
        for (measurement, expected, source, fallback_reason) in [
            (
                Some(HostTextMeasurement::Length(73.25)),
                73.25,
                TextMeasurementSource::Host,
                None,
            ),
            (
                None,
                fallback_width,
                TextMeasurementSource::Profile,
                Some(HostFallbackReason::Missing),
            ),
            (
                Some(HostTextMeasurement::Metrics(metrics(100.0))),
                fallback_width,
                TextMeasurementSource::Profile,
                Some(HostFallbackReason::Invalid),
            ),
        ] {
            let calls = Arc::new(AtomicUsize::new(0));
            let session = RenderEnvironment::deterministic()
                .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                    identity("test.decoded-line-host", "v1", &[]),
                    Arc::new(DecodedLineHost {
                        calls: Arc::clone(&calls),
                        measurement,
                    }),
                    [TextMeasurementPhase::Wrap],
                ))
                .begin_session()
                .expect("begin decoded line host session");
            let measurer = session.text_measurer(TextMeasurementPhase::ComputedLength);
            assert_eq!(measurer.measure_html_line_width_px(text, &style), expected);
            assert_eq!(calls.load(Ordering::Relaxed), 1);
            let report = session.text_measurement_report();
            assert_eq!(report.entries().len(), 1);
            let provenance = report.entries()[0].provenance();
            assert_eq!(provenance.phase, TextMeasurementPhase::Wrap);
            assert_eq!(
                provenance.operation,
                TextMeasurementOperation::CanvasMeasureTextWidth
            );
            assert_eq!(provenance.source, source);
            assert_eq!(provenance.fallback_reason, fallback_reason);
        }

        let custom_session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(
                TextMeasurementPolicy::deterministic().with_profile_for_phase(
                    TextMeasurementPhase::Wrap,
                    TextMeasurementProfile::new(
                        identity("test.specialized-decoded-line", "v1", &[]),
                        Arc::new(SpecializedProfile),
                    ),
                ),
            )
            .begin_session()
            .expect("begin custom decoded line session");
        assert_eq!(
            custom_session
                .text_measurer(TextMeasurementPhase::Layout)
                .measure_html_line_width_px(text, &style),
            13.0,
            "a custom profile retains its complete wrapped-measurement default"
        );
    }

    #[test]
    fn decoded_html_line_cancellation_skips_fallback_and_future_wrap_probes() {
        struct CancellingDecodedLineHost(Arc<AtomicUsize>);

        impl HostTextMeasurer for CancellingDecodedLineHost {
            fn measure(&self, _request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
                panic!("decoded line requests must carry the active session control");
            }

            fn measure_with_control(
                &self,
                request: HostTextMeasurementRequest<'_>,
                control: &OperationControl,
            ) -> HostMeasurementResult {
                assert_eq!(request.phase, TextMeasurementPhase::Wrap);
                assert_eq!(
                    request.operation,
                    TextMeasurementOperation::CanvasMeasureTextWidth
                );
                self.0.fetch_add(1, Ordering::Relaxed);
                control.cancel();
                Ok(None)
            }
        }

        let control = OperationControl::new();
        let host_calls = Arc::new(AtomicUsize::new(0));
        let fallback_calls = Arc::new(AtomicUsize::new(0));
        let fallback = TextMeasurementProfile::new(
            identity("test.cancelled-html-line-fallback", "v1", &[]),
            Arc::new(CountingFallback(Arc::clone(&fallback_calls))),
        );
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::host_display_with_fallback(
                identity("test.cancelling-html-line", "v1", &[]),
                Arc::new(CancellingDecodedLineHost(Arc::clone(&host_calls))),
                [TextMeasurementPhase::Wrap],
                fallback,
            ))
            .begin_session_with_control(control)
            .expect("begin controlled decoded line session");
        let measurer = session
            .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Postprocess);
        let style = TextStyle::default();
        assert_eq!(measurer.measure_html_line_width_px("a<br>b", &style), 0.0);
        assert_eq!(
            measurer.measure_html_line_width_px("next candidate", &style),
            0.0
        );
        assert_eq!(host_calls.load(Ordering::Relaxed), 1);
        assert_eq!(fallback_calls.load(Ordering::Relaxed), 0);
        assert!(session.text_measurement_report().entries().is_empty());
        assert!(matches!(
            session.checkpoint(OperationPhase::Postprocess),
            Err(crate::Error::Cancelled(_))
        ));
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

    struct ActiveOperationHost;

    impl HostTextMeasurer for ActiveOperationHost {
        fn measure(&self, _request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            panic!("session must provide its canonical control");
        }

        fn measure_with_control(
            &self,
            _request: HostTextMeasurementRequest<'_>,
            control: &OperationControl,
        ) -> HostMeasurementResult {
            control.cancel();
            Ok(None)
        }
    }

    #[test]
    fn host_adapter_receives_the_active_session_control() {
        let control = OperationControl::new();
        let policy = TextMeasurementPolicy::host_display(
            identity("test.active-operation", "v1", &[]),
            Arc::new(ActiveOperationHost),
            TextMeasurementPhase::ALL,
        );
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(policy)
            .begin_session_with_control(control.clone())
            .expect("begin session");
        let metrics = session
            .controlled_text_measurer(TextMeasurementPhase::Layout, OperationPhase::Layout)
            .measure("label", &TextStyle::default());
        assert_eq!(metrics.width, 0.0);
        assert!(
            control
                .terminal_checkpoint_at(OperationPhase::Layout)
                .is_err()
        );
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
        assert!(!session.supports_capability(RenderCapability::LayoutDagre));
        assert!(!session.supports_capability(RenderCapability::LayoutShoal));
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

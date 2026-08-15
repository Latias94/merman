//! Canonical source-to-target rendering facade.
//!
//! `Renderer` owns long-lived defaults. Each [`RenderRequest`] owns one operation control and is
//! executed synchronously through the same internal operation runner. Target-specific layout and
//! emission remain private to their adapters.

use merman_core::{
    Engine, OperationCancelled, OperationControl, ParseOptions, resources::InputResourcePolicy,
    runtime::RuntimePolicyError,
};

use crate::operation_runner::Operation;
#[cfg(feature = "svg")]
use crate::operation_runner::OperationExecution;

#[cfg(feature = "ascii")]
use merman_ascii::{AsciiError, AsciiRenderOptions, AsciiResourcePolicy};
#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
use merman_export::ExportError;
#[cfg(feature = "svg")]
use merman_render::{
    LayoutOptions, RenderCapabilityPolicy, ResourceLimitExceeded as SvgResourceLimitExceeded,
    diagram_theme::{DiagramTheme, ThemeResourcePolicy},
    environment::{RenderEnvironment as BackendRenderEnvironment, TextMeasurementPolicy},
    math::MathRenderer,
    resources::RenderResourcePolicy,
    svg::{SvgDebugOptions, SvgPipeline, SvgRenderOptions},
};

pub use crate::operation_runner::SemanticArtifact;

/// SVG-only host services and rendering limits.
///
/// Operation time, timezone, randomness, cancellation, and deadlines deliberately do not live
/// here. [`Renderer`] owns those operation-wide concerns and injects the already captured context
/// into the private SVG session. This keeps `SvgRequest` from becoming a second operation owner.
#[cfg(feature = "svg")]
#[derive(Debug, Clone)]
pub struct SvgEnvironment {
    backend: BackendRenderEnvironment,
    text_measurement_routes: [merman_render::environment::TextMeasurementRoute; 4],
}

#[cfg(feature = "svg")]
impl SvgEnvironment {
    /// Creates the deterministic default SVG service set.
    pub fn deterministic() -> Self {
        let text_measurement = TextMeasurementPolicy::parity();
        Self {
            backend: BackendRenderEnvironment::deterministic()
                .with_text_measurement_policy(text_measurement.clone()),
            text_measurement_routes: text_measurement.routes(),
        }
    }

    pub fn with_text_measurement_policy(mut self, policy: TextMeasurementPolicy) -> Self {
        self.text_measurement_routes = policy.routes();
        self.backend = self.backend.with_text_measurement_policy(policy);
        self
    }

    pub fn with_capability_policy(mut self, policy: RenderCapabilityPolicy) -> Self {
        self.backend = self.backend.with_capability_policy(policy);
        self
    }

    pub fn with_compiled_math_renderer(mut self) -> Self {
        self.backend = self.backend.with_compiled_math_renderer();
        self
    }

    pub fn with_math_renderer(
        mut self,
        renderer: std::sync::Arc<dyn MathRenderer + Send + Sync>,
    ) -> Self {
        self.backend = self.backend.with_math_renderer(renderer);
        self
    }

    pub fn without_math_renderer(mut self) -> Self {
        self.backend = self.backend.without_math_renderer();
        self
    }

    pub fn with_icon_registry(mut self, registry: merman_render::svg::IconRegistry) -> Self {
        self.backend = self.backend.with_icon_registry(registry);
        self
    }

    pub fn with_resource_policy(mut self, policy: RenderResourcePolicy) -> Self {
        self.backend = self.backend.with_resource_policy(policy);
        self
    }

    /// Sets the host-owned ceiling for resources used while materializing a compiled theme.
    ///
    /// Session creation intersects this ceiling with the restriction captured by the theme
    /// compiler. Neither side can widen the other.
    pub fn with_theme_resource_ceiling(mut self, ceiling: ThemeResourcePolicy) -> Self {
        self.backend = self.backend.with_theme_resource_ceiling(ceiling);
        self
    }

    pub fn with_theme_admission_policy(
        mut self,
        policy: merman_render::diagram_theme::ThemeAdmissionPolicy,
    ) -> Self {
        self.backend = self.backend.with_theme_admission_policy(policy);
        self
    }

    pub fn with_theme_portability_requirement(
        mut self,
        requirement: merman_render::diagram_theme::ThemePortabilityRequirement,
    ) -> Self {
        self.backend = self.backend.with_theme_portability_requirement(requirement);
        self
    }

    /// Returns the configured text-measurement routes without creating an operation session.
    pub fn text_measurement_routes(&self) -> [merman_render::environment::TextMeasurementRoute; 4] {
        self.text_measurement_routes.clone()
    }

    fn begin_session_in_context(
        &self,
        theme: Option<&DiagramTheme>,
        context: merman_core::runtime::OperationContext,
        control: OperationControl,
    ) -> Result<merman_render::environment::RenderSession, RenderError> {
        match theme {
            Some(theme) => self
                .backend
                .begin_session_with_theme_in_context(theme, context, control)
                .map_err(RenderError::from),
            None => self
                .backend
                .begin_session_in_context(context, control)
                .map_err(RenderError::from),
        }
    }
}

#[cfg(feature = "svg")]
impl Default for SvgEnvironment {
    fn default() -> Self {
        Self::deterministic()
    }
}

/// Coarse terminal state for one bounded theme-evidence scope.
///
/// The renderer keeps mechanism keys, selectors, element receipts, and residual ledgers private.
/// Callers only need to know whether the scope was fully accounted for and how much bounded
/// evidence remained outside the portable path.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ThemeEvidenceStatus {
    NotApplicable,
    Verified,
    Residual,
    Incomplete,
}

#[cfg(feature = "svg")]
impl ThemeEvidenceStatus {
    /// Stable discovery view for the coarse evidence states exposed by the facade.
    pub const ALL: &'static [Self] = &[
        Self::NotApplicable,
        Self::Verified,
        Self::Residual,
        Self::Incomplete,
    ];
}

/// Coarse document-level projection of renderer-owned theme evidence.
///
/// This is intentionally not a mechanism ledger. It answers whether root and family work was
/// accounted for while keeping the proof implementation private and free to evolve.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThemeEvidenceSummary {
    status: ThemeEvidenceStatus,
    output_mutated: bool,
}

#[cfg(feature = "svg")]
impl ThemeEvidenceSummary {
    const fn new(status: ThemeEvidenceStatus, output_mutated: bool) -> Self {
        Self {
            status,
            output_mutated,
        }
    }

    pub const fn status(self) -> ThemeEvidenceStatus {
        self.status
    }

    pub const fn output_mutated(self) -> bool {
        self.output_mutated
    }

    pub const fn is_verified(self) -> bool {
        matches!(self.status, ThemeEvidenceStatus::Verified) && !self.output_mutated
    }

    /// Returns whether the document either required no theme evidence or verified all required
    /// evidence without a later output mutation.
    pub const fn is_satisfied(self) -> bool {
        matches!(
            self.status,
            ThemeEvidenceStatus::NotApplicable | ThemeEvidenceStatus::Verified
        ) && !self.output_mutated
    }
}

#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ThemeEvidenceScopeProjection {
    pub(crate) status: ThemeEvidenceStatus,
    pub(crate) required_count: usize,
    pub(crate) accounted_count: usize,
    pub(crate) applied_count: usize,
    pub(crate) not_applicable_count: usize,
    pub(crate) residual_count: usize,
    pub(crate) output_mutated: bool,
}

#[cfg(feature = "svg")]
impl ThemeEvidenceScopeProjection {
    pub(crate) const fn incomplete_count(self) -> usize {
        self.required_count.saturating_sub(self.accounted_count)
    }

    pub(crate) const fn is_satisfied(self) -> bool {
        matches!(
            self.status,
            ThemeEvidenceStatus::NotApplicable | ThemeEvidenceStatus::Verified
        ) && self.incomplete_count() == 0
            && self.residual_count == 0
            && !self.output_mutated
    }
}

#[cfg(all(feature = "svg", feature = "internal-theme-acceptance"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ThemeAcceptanceEvidenceSnapshot {
    root: ThemeEvidenceScopeProjection,
    family: ThemeEvidenceScopeProjection,
    source_residual_count: usize,
    compatibility_residual_count: usize,
    mermaid_compatibility_residual_count: usize,
}

/// Workspace-only projection used by the non-published theme acceptance harness.
#[cfg(all(feature = "svg", feature = "internal-theme-acceptance"))]
pub(crate) struct ThemeAcceptanceEvidenceProjection<'a> {
    pub(crate) root: ThemeEvidenceScopeProjection,
    pub(crate) family: ThemeEvidenceScopeProjection,
    pub(crate) source_residual_count: usize,
    pub(crate) compatibility_residual_count: usize,
    pub(crate) mermaid_compatibility_residual_count: usize,
    pub(crate) recipe_report: Option<&'a merman_render::diagram_theme::ThemeRecipeReport>,
    pub(crate) host_admission_report:
        Option<&'a merman_render::diagram_theme::ThemeHostAdmissionReport>,
    pub(crate) prepared_text_layout: Option<&'a merman_render::text::PreparedTextLayoutReport>,
    pub(crate) text_layout_failure: Option<merman_render::text::TextLayoutFailure>,
    pub(crate) effective_theme_resource_policy:
        &'a merman_render::diagram_theme::ThemeResourcePolicy,
}

/// Immutable evidence captured by a completed SVG operation.
///
/// The evidence is created only by the renderer after SVG emission/postprocessing succeeds. It
/// is intentionally a narrow projection: callers can inspect measurement provenance and runtime
/// identity without gaining access to SVG session services or family layout internals.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderEvidence {
    session: merman_render::environment::RenderSessionReport,
    family_id: merman_core::DiagramFamilyId,
    theme_evidence: ThemeEvidenceSummary,
    #[cfg(feature = "internal-theme-acceptance")]
    theme_acceptance: ThemeAcceptanceEvidenceSnapshot,
    #[cfg(feature = "internal-theme-acceptance")]
    native_filter_receipt: Option<merman_render::__private::NativeSvgFilterReceipt>,
}

#[cfg(feature = "svg")]
impl RenderEvidence {
    fn from_family(family: merman_render::family::FamilyRenderReport) -> Self {
        let session = family.session_report().clone();
        let family_id = family.family_id();
        let (root, family_scope, family_evidence) = theme_evidence_scopes(&family);
        let source_residual_count = family_evidence.source_residual_count();
        let compatibility_residual_count = family_evidence.compatibility_residual_count();
        let mermaid_compatibility_residual_count =
            family_evidence.mermaid_compatibility_residual_count();
        let theme_evidence = summarize_theme_evidence(
            root,
            family_scope,
            source_residual_count,
            compatibility_residual_count,
            mermaid_compatibility_residual_count,
        );
        #[cfg(feature = "internal-theme-acceptance")]
        let native_filter_receipt = merman_render::__private::family_native_filter_receipt(&family);

        Self {
            session,
            family_id,
            theme_evidence,
            #[cfg(feature = "internal-theme-acceptance")]
            theme_acceptance: ThemeAcceptanceEvidenceSnapshot {
                root,
                family: family_scope,
                source_residual_count,
                compatibility_residual_count,
                mermaid_compatibility_residual_count,
            },
            #[cfg(feature = "internal-theme-acceptance")]
            native_filter_receipt,
        }
    }

    pub fn measurement_routes(&self) -> &[merman_render::environment::TextMeasurementRoute; 4] {
        self.session.measurement_routes()
    }

    pub fn measurement(&self) -> &merman_render::environment::TextMeasurementReport {
        self.session.measurement()
    }

    pub fn operation_context(&self) -> &merman_core::runtime::OperationContext {
        self.session.operation_context()
    }

    pub const fn unix_millis(&self) -> i64 {
        self.session.unix_millis()
    }

    pub const fn local_date(&self) -> merman_core::time::CivilDate {
        self.session.local_date()
    }

    pub fn local_time_zone(&self) -> &merman_core::time::LocalTimeZoneProvenance {
        self.session.local_time_zone()
    }

    pub fn render_seed(&self) -> std::num::NonZeroU64 {
        self.session.render_seed()
    }

    pub const fn layout_work_units(&self) -> usize {
        self.session.layout_work_units()
    }

    pub const fn family_id(&self) -> merman_core::DiagramFamilyId {
        self.family_id
    }

    pub fn theme_recipe_fingerprint(
        &self,
    ) -> Option<merman_render::diagram_theme::ThemeRecipeFingerprint> {
        self.session.theme_recipe_fingerprint()
    }

    pub const fn theme_evidence(&self) -> ThemeEvidenceSummary {
        self.theme_evidence
    }

    #[cfg(feature = "internal-theme-acceptance")]
    pub(crate) fn theme_acceptance_evidence(&self) -> ThemeAcceptanceEvidenceProjection<'_> {
        ThemeAcceptanceEvidenceProjection {
            root: self.theme_acceptance.root,
            family: self.theme_acceptance.family,
            source_residual_count: self.theme_acceptance.source_residual_count,
            compatibility_residual_count: self.theme_acceptance.compatibility_residual_count,
            mermaid_compatibility_residual_count: self
                .theme_acceptance
                .mermaid_compatibility_residual_count,
            recipe_report: self.session.theme_recipe_report(),
            host_admission_report: self.session.theme_host_admission_report(),
            prepared_text_layout: self.session.prepared_text_layout(),
            text_layout_failure: self.session.text_layout_failure(),
            effective_theme_resource_policy:
                merman_render::__private::effective_theme_resource_policy(&self.session),
        }
    }

    #[cfg(feature = "internal-theme-acceptance")]
    pub(crate) fn native_filter_receipt(
        &self,
    ) -> Option<merman_render::__private::NativeSvgFilterReceipt> {
        self.native_filter_receipt
    }
}

#[cfg(feature = "svg")]
fn theme_evidence_scopes(
    report: &merman_render::family::FamilyRenderReport,
) -> (
    ThemeEvidenceScopeProjection,
    ThemeEvidenceScopeProjection,
    merman_render::__private::FamilyEvidenceSummary,
) {
    let root = merman_render::__private::root_evidence(report);
    let root_status = match root.status() {
        merman_render::diagram_theme::RootThemeVerification::NotApplicable => {
            ThemeEvidenceStatus::NotApplicable
        }
        merman_render::diagram_theme::RootThemeVerification::Verified => {
            ThemeEvidenceStatus::Verified
        }
        merman_render::diagram_theme::RootThemeVerification::Unverified => {
            ThemeEvidenceStatus::Residual
        }
        merman_render::diagram_theme::RootThemeVerification::Incomplete => {
            ThemeEvidenceStatus::Incomplete
        }
        _ => ThemeEvidenceStatus::Incomplete,
    };

    let family = merman_render::__private::family_evidence(report);
    let family_status = match family.status() {
        merman_render::__private::FamilyEvidenceStatus::NotApplicable => {
            ThemeEvidenceStatus::NotApplicable
        }
        merman_render::__private::FamilyEvidenceStatus::Verified => ThemeEvidenceStatus::Verified,
        merman_render::__private::FamilyEvidenceStatus::Unverified => ThemeEvidenceStatus::Residual,
        merman_render::__private::FamilyEvidenceStatus::Unadapted
        | merman_render::__private::FamilyEvidenceStatus::Incomplete => {
            ThemeEvidenceStatus::Incomplete
        }
    };

    (
        ThemeEvidenceScopeProjection {
            status: root_status,
            required_count: root.required_count(),
            accounted_count: root.accounted_count(),
            applied_count: root.applied_count(),
            not_applicable_count: 0,
            residual_count: root.residual_count(),
            output_mutated: root.output_mutated(),
        },
        ThemeEvidenceScopeProjection {
            status: family_status,
            required_count: family.required_count(),
            accounted_count: family.accounted_count(),
            applied_count: family.applied_count(),
            not_applicable_count: family.not_applicable_count(),
            residual_count: family.theme_residual_count(),
            output_mutated: family.output_mutated(),
        },
        family,
    )
}

#[cfg(feature = "svg")]
fn summarize_theme_evidence(
    root: ThemeEvidenceScopeProjection,
    family: ThemeEvidenceScopeProjection,
    source_residual_count: usize,
    compatibility_residual_count: usize,
    mermaid_compatibility_residual_count: usize,
) -> ThemeEvidenceSummary {
    let extra_residual_count = source_residual_count
        .saturating_add(compatibility_residual_count)
        .saturating_add(mermaid_compatibility_residual_count);
    let residual_count = root
        .residual_count
        .saturating_add(family.residual_count)
        .saturating_add(extra_residual_count);
    let output_mutated = root.output_mutated || family.output_mutated;
    let status = if root.status == ThemeEvidenceStatus::NotApplicable
        && family.status == ThemeEvidenceStatus::NotApplicable
        && residual_count == 0
        && !output_mutated
    {
        ThemeEvidenceStatus::NotApplicable
    } else if root.is_satisfied()
        && family.is_satisfied()
        && extra_residual_count == 0
        && !output_mutated
    {
        ThemeEvidenceStatus::Verified
    } else if root.status == ThemeEvidenceStatus::Incomplete
        || family.status == ThemeEvidenceStatus::Incomplete
        || root.incomplete_count() != 0
        || family.incomplete_count() != 0
    {
        ThemeEvidenceStatus::Incomplete
    } else if residual_count != 0
        || output_mutated
        || root.status == ThemeEvidenceStatus::Residual
        || family.status == ThemeEvidenceStatus::Residual
    {
        ThemeEvidenceStatus::Residual
    } else {
        ThemeEvidenceStatus::Incomplete
    };

    ThemeEvidenceSummary::new(status, output_mutated)
}

/// Successful SVG output and the evidence for the operation that produced it.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgOutput {
    svg: String,
    evidence: RenderEvidence,
}

/// Successful terminally finalized SVG output.
///
/// Unlike [`SvgOutput`], this target has passed the resvg-safe terminal validator, resource
/// closure, and prepared-text/font finalization. It remains distinct from portable admission:
/// callers must inspect the accompanying render evidence and target policy rather than treating
/// successful finalization as an unconditional portability claim.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizedSvgOutput {
    svg: merman_render::svg::ResvgCompatibleSvg,
    evidence: RenderEvidence,
}

#[cfg(feature = "svg")]
impl FinalizedSvgOutput {
    fn new(
        svg: merman_render::svg::ResvgCompatibleSvg,
        family: merman_render::family::FamilyRenderReport,
    ) -> Self {
        Self {
            svg,
            evidence: RenderEvidence::from_family(family),
        }
    }

    pub fn svg(&self) -> &str {
        self.svg.as_str()
    }

    pub fn evidence(&self) -> &RenderEvidence {
        &self.evidence
    }

    pub const fn resource_fingerprint(&self) -> merman_render::svg::SvgResourceFingerprint {
        self.svg.resource_fingerprint()
    }

    pub fn into_parts(self) -> (merman_render::svg::ResvgCompatibleSvg, RenderEvidence) {
        (self.svg, self.evidence)
    }
}

#[cfg(feature = "svg")]
impl SvgOutput {
    fn new(svg: String, family: merman_render::family::FamilyRenderReport) -> Self {
        Self {
            svg,
            evidence: RenderEvidence::from_family(family),
        }
    }

    pub fn svg(&self) -> &str {
        &self.svg
    }

    pub fn evidence(&self) -> &RenderEvidence {
        &self.evidence
    }

    pub fn into_parts(self) -> (String, RenderEvidence) {
        (self.svg, self.evidence)
    }
}

/// Successful SVG layout inspection output.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, PartialEq)]
pub struct SvgLayoutOutput {
    layout: serde_json::Value,
    gantt_time_axis: Option<merman_render::family::GanttTimeAxisDiagnostics>,
}

#[cfg(feature = "svg")]
impl SvgLayoutOutput {
    fn new(
        layout: serde_json::Value,
        gantt_time_axis: Option<merman_render::family::GanttTimeAxisDiagnostics>,
    ) -> Self {
        Self {
            layout,
            gantt_time_axis,
        }
    }

    pub fn layout(&self) -> &serde_json::Value {
        &self.layout
    }

    pub fn gantt_time_axis_diagnostics(
        &self,
    ) -> Option<merman_render::family::GanttTimeAxisDiagnostics> {
        self.gantt_time_axis
    }

    pub fn into_parts(
        self,
    ) -> (
        serde_json::Value,
        Option<merman_render::family::GanttTimeAxisDiagnostics>,
    ) {
        (self.layout, self.gantt_time_axis)
    }
}

/// Structured error returned by the canonical rendering facade.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    #[error(transparent)]
    Cancelled(#[from] OperationCancelled),
    #[error(transparent)]
    Parse(#[from] merman_core::Error),
    #[error(transparent)]
    RuntimePolicy(#[from] RuntimePolicyError),
    #[error(transparent)]
    ResourceLimitExceeded(#[from] ResourceLimitExceeded),
    #[cfg(feature = "svg")]
    #[error(transparent)]
    Svg(merman_render::Error),
    #[cfg(feature = "svg")]
    #[error(transparent)]
    SvgEnvironment(merman_render::environment::RenderEnvironmentError),
    #[cfg(feature = "ascii")]
    #[error(transparent)]
    Ascii(#[from] AsciiError),
    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    #[error(transparent)]
    Export(#[from] ExportError),
    #[error("render target is not available in this feature configuration: {0}")]
    UnsupportedTarget(&'static str),
}

/// Transport-neutral resource rejection projected by the common facade.
///
/// Target adapters retain their richer policy types internally. Hosts can classify every
/// source, layout, output, ASCII-grid, and export quota through this stable descriptor without
/// matching backend-specific errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "resource limit `{id}` exceeded during {phase}: actual={actual} maximum={maximum} cause={cause}"
)]
#[non_exhaustive]
pub struct ResourceLimitExceeded {
    pub id: &'static str,
    pub phase: &'static str,
    pub actual: u64,
    pub maximum: u64,
    pub cause: ResourceLimitCause,
}

/// Stable facade-level reason for a resource rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLimitCause {
    Ceiling,
    ArithmeticOverflow,
}

impl ResourceLimitCause {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ceiling => "ceiling",
            Self::ArithmeticOverflow => "arithmetic_overflow",
        }
    }
}

impl std::fmt::Display for ResourceLimitCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ResourceLimitExceeded {
    pub(crate) fn from_input(error: merman_core::resources::InputResourceLimitExceeded) -> Self {
        Self {
            id: error.limit,
            phase: error.phase.as_str(),
            actual: error.actual as u64,
            maximum: error.max as u64,
            cause: ResourceLimitCause::Ceiling,
        }
    }

    #[cfg(feature = "ascii")]
    fn from_operation(error: merman_core::OperationResourceLimitExceeded) -> Self {
        let phase = if error.id == merman_ascii::MAX_ASCII_GRID_CELLS_RESOURCE_LIMIT_ID {
            merman_ascii::ASCII_RESOURCE_LIMIT_DESCRIPTORS[0].phase
        } else {
            error.phase.as_str()
        };
        Self {
            id: error.id,
            phase,
            actual: error.consumed.saturating_add(error.requested),
            maximum: error.limit,
            cause: ResourceLimitCause::Ceiling,
        }
    }

    #[cfg(feature = "svg")]
    fn from_svg(error: SvgResourceLimitExceeded) -> Self {
        Self {
            id: error.limit,
            phase: error.phase.as_str(),
            actual: error.actual as u64,
            maximum: error.max as u64,
            cause: match error.cause {
                merman_render::ResourceLimitCause::Ceiling => ResourceLimitCause::Ceiling,
                merman_render::ResourceLimitCause::ArithmeticOverflow => {
                    ResourceLimitCause::ArithmeticOverflow
                }
                _ => ResourceLimitCause::Ceiling,
            },
        }
    }

    #[cfg(feature = "svg")]
    fn from_theme(error: merman_render::diagram_theme::ThemeResourceLimitExceeded) -> Self {
        Self {
            id: error.limit,
            phase: error.phase.as_str(),
            actual: u64::try_from(error.actual).unwrap_or(u64::MAX),
            maximum: u64::try_from(error.max).unwrap_or(u64::MAX),
            cause: ResourceLimitCause::Ceiling,
        }
    }

    #[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
    fn from_export(details: merman_export::ExportResourceLimitDetails) -> Self {
        Self {
            id: details.limit_id,
            phase: details.phase,
            actual: details.actual,
            maximum: details.max,
            cause: ResourceLimitCause::Ceiling,
        }
    }
}

#[cfg(feature = "svg")]
fn map_svg_error(error: merman_render::Error) -> RenderError {
    match error {
        merman_render::Error::Cancelled(cancelled) => RenderError::Cancelled(cancelled),
        merman_render::Error::ResourceLimitExceeded(resource) => {
            RenderError::from(ResourceLimitExceeded::from_svg(resource))
        }
        merman_render::Error::ThemeResourceLimitExceeded(resource) => {
            RenderError::from(ResourceLimitExceeded::from_theme(resource))
        }
        other => RenderError::Svg(other),
    }
}

#[cfg(feature = "svg")]
impl From<merman_render::Error> for RenderError {
    fn from(error: merman_render::Error) -> Self {
        map_svg_error(error)
    }
}

#[cfg(feature = "svg")]
impl From<merman_render::environment::RenderEnvironmentError> for RenderError {
    fn from(error: merman_render::environment::RenderEnvironmentError) -> Self {
        match error {
            merman_render::environment::RenderEnvironmentError::Cancelled(cancelled) => {
                Self::Cancelled(cancelled)
            }
            merman_render::environment::RenderEnvironmentError::Runtime(runtime) => {
                Self::RuntimePolicy(runtime)
            }
            merman_render::environment::RenderEnvironmentError::ThemeResource(resource) => {
                Self::ResourceLimitExceeded(ResourceLimitExceeded::from_theme(resource))
            }
            other => Self::SvgEnvironment(other),
        }
    }
}

#[cfg(feature = "ascii")]
fn map_ascii_error(error: AsciiError) -> RenderError {
    match error {
        AsciiError::Cancelled(cancelled) => RenderError::Cancelled(cancelled),
        AsciiError::ResourceLimitExceeded(resource) => {
            RenderError::from(ResourceLimitExceeded::from_operation(resource))
        }
        other => RenderError::Ascii(other),
    }
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn map_export_error(error: ExportError) -> RenderError {
    if let Some(details) = error.resource_limit_details() {
        return RenderError::from(ResourceLimitExceeded::from_export(details));
    }
    match error {
        ExportError::Cancelled(cancelled) => RenderError::Cancelled(cancelled),
        other => RenderError::Export(other),
    }
}

/// Target request for the canonical facade.
///
/// The semantic target is available in every feature configuration. SVG, ASCII, and terminal
/// export variants are feature-gated leaves of the same dispatch seam.
#[derive(Debug, Clone)]
pub enum RenderTarget {
    Semantic,
    #[cfg(feature = "svg")]
    Svg(SvgRequest),
    #[cfg(feature = "svg")]
    FinalizedSvg(SvgRequest),
    #[cfg(feature = "svg")]
    LayoutJson(SvgRequest),
    #[cfg(feature = "svg")]
    SvgPlan(SvgRequest),
    #[cfg(feature = "ascii")]
    Ascii(AsciiRequest),
    #[cfg(feature = "png")]
    Png(PngRequest),
    #[cfg(feature = "jpeg")]
    Jpeg(JpegRequest),
    #[cfg(feature = "pdf")]
    Pdf(PdfRequest),
}

#[cfg(feature = "svg")]
#[derive(Debug, Clone)]
pub struct SvgRequest {
    pub environment: SvgEnvironment,
    pub layout: LayoutOptions,
    pub options: SvgRenderOptions,
    pub debug: SvgDebugOptions,
    pub pipeline: Option<SvgPipeline>,
}

#[cfg(feature = "svg")]
impl Default for SvgRequest {
    fn default() -> Self {
        Self {
            environment: SvgEnvironment::deterministic(),
            layout: LayoutOptions::headless_svg_defaults(),
            options: SvgRenderOptions::default(),
            debug: SvgDebugOptions::default(),
            pipeline: None,
        }
    }
}

#[cfg(feature = "ascii")]
#[derive(Debug, Clone, Default)]
pub struct AsciiRequest {
    pub options: AsciiRenderOptions,
    pub resources: AsciiResourcePolicy,
}

#[cfg(feature = "png")]
#[derive(Debug, Clone)]
pub struct PngRequest {
    pub svg: SvgRequest,
    pub options: merman_export::RasterOptions,
}

#[cfg(feature = "jpeg")]
#[derive(Debug, Clone)]
pub struct JpegRequest {
    pub svg: SvgRequest,
    pub options: merman_export::RasterOptions,
}

#[cfg(feature = "pdf")]
#[derive(Debug, Clone)]
pub struct PdfRequest {
    pub svg: SvgRequest,
    pub options: merman_export::PdfOptions,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
#[derive(Debug, Clone)]
pub struct RasterOutput {
    pub bytes: Vec<u8>,
    pub plan: merman_export::RasterPlan,
    evidence: RenderEvidence,
    export_report: merman_export::RasterExportReport,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl RasterOutput {
    pub fn evidence(&self) -> &RenderEvidence {
        &self.evidence
    }

    pub const fn export_report(&self) -> merman_export::RasterExportReport {
        self.export_report
    }

    pub fn into_parts(self) -> (Vec<u8>, RenderEvidence, merman_export::RasterExportReport) {
        (self.bytes, self.evidence, self.export_report)
    }
}

#[cfg(feature = "pdf")]
#[derive(Debug, Clone)]
pub struct PdfOutput {
    pub bytes: Vec<u8>,
    pub plan: merman_export::PdfFilterImagePlan,
    evidence: RenderEvidence,
    export_report: merman_export::PdfExportReport,
}

#[cfg(feature = "pdf")]
impl PdfOutput {
    pub fn evidence(&self) -> &RenderEvidence {
        &self.evidence
    }

    pub const fn export_report(&self) -> merman_export::PdfExportReport {
        self.export_report
    }

    pub fn into_parts(self) -> (Vec<u8>, RenderEvidence, merman_export::PdfExportReport) {
        (self.bytes, self.evidence, self.export_report)
    }
}

/// One source-to-target request. The control is cloneable so a host can retain a handle and cancel
/// the synchronous worker from another task or thread.
#[derive(Debug, Clone)]
pub struct RenderRequest<'a> {
    source: &'a str,
    target: RenderTarget,
    control: OperationControl,
    parse_options: Option<ParseOptions>,
    resources: Option<InputResourcePolicy>,
    #[cfg(feature = "svg")]
    theme: Option<DiagramTheme>,
}

impl<'a> RenderRequest<'a> {
    /// Creates a typed operation request that inherits the renderer's parse and input-resource
    /// defaults until an explicit request override is applied.
    pub fn new(source: &'a str, target: RenderTarget, control: OperationControl) -> Self {
        Self {
            source,
            target,
            control,
            parse_options: None,
            resources: None,
            #[cfg(feature = "svg")]
            theme: None,
        }
    }

    pub fn semantic(source: &'a str, control: OperationControl) -> Self {
        Self::new(source, RenderTarget::Semantic, control)
    }

    #[cfg(feature = "svg")]
    pub fn svg(source: &'a str, control: OperationControl, request: SvgRequest) -> Self {
        Self::new(source, RenderTarget::Svg(request), control)
    }

    /// Requests a terminally finalized standalone SVG artifact.
    #[cfg(feature = "svg")]
    pub fn finalized_svg(source: &'a str, control: OperationControl, request: SvgRequest) -> Self {
        Self::new(source, RenderTarget::FinalizedSvg(request), control)
    }

    #[cfg(feature = "svg")]
    pub fn layout_json(source: &'a str, control: OperationControl, request: SvgRequest) -> Self {
        Self::new(source, RenderTarget::LayoutJson(request), control)
    }

    #[cfg(feature = "svg")]
    pub fn svg_plan(source: &'a str, control: OperationControl, request: SvgRequest) -> Self {
        Self::new(source, RenderTarget::SvgPlan(request), control)
    }

    #[cfg(feature = "ascii")]
    pub fn ascii(source: &'a str, control: OperationControl, request: AsciiRequest) -> Self {
        Self::new(source, RenderTarget::Ascii(request), control)
    }

    #[cfg(feature = "png")]
    pub fn png(source: &'a str, control: OperationControl, request: PngRequest) -> Self {
        Self::new(source, RenderTarget::Png(request), control)
    }

    #[cfg(feature = "jpeg")]
    pub fn jpeg(source: &'a str, control: OperationControl, request: JpegRequest) -> Self {
        Self::new(source, RenderTarget::Jpeg(request), control)
    }

    #[cfg(feature = "pdf")]
    pub fn pdf(source: &'a str, control: OperationControl, request: PdfRequest) -> Self {
        Self::new(source, RenderTarget::Pdf(request), control)
    }

    pub fn with_parse_options(mut self, parse_options: ParseOptions) -> Self {
        self.parse_options = Some(parse_options);
        self
    }

    pub fn with_resource_policy(mut self, resources: InputResourcePolicy) -> Self {
        self.resources = Some(resources);
        self
    }

    /// Binds one compiled theme to the whole parse-to-artifact operation.
    ///
    /// The same recipe supplies parse compatibility and family rendering evidence. Keeping the
    /// theme on the operation prevents a semantic artifact parsed under one recipe from being
    /// rendered under another.
    #[cfg(feature = "svg")]
    pub fn with_theme(mut self, theme: DiagramTheme) -> Self {
        self.theme = Some(theme);
        self
    }
}

/// Successful output from a canonical request.
#[derive(Debug)]
pub enum RenderOutput {
    Semantic(Option<SemanticArtifact>),
    #[cfg(feature = "svg")]
    Svg(Option<SvgOutput>),
    #[cfg(feature = "svg")]
    FinalizedSvg(Option<FinalizedSvgOutput>),
    #[cfg(feature = "svg")]
    LayoutJson(Option<SvgLayoutOutput>),
    #[cfg(feature = "svg")]
    SvgPlan(Option<merman_render::family::RenderCapabilityPlan>),
    #[cfg(feature = "ascii")]
    Ascii(Option<String>),
    #[cfg(feature = "png")]
    Png(Option<RasterOutput>),
    #[cfg(feature = "jpeg")]
    Jpeg(Option<RasterOutput>),
    #[cfg(feature = "pdf")]
    Pdf(Option<PdfOutput>),
}

/// Long-lived renderer defaults and host-independent engine configuration.
#[derive(Debug, Clone)]
pub struct Renderer {
    engine: Engine,
    parse_options: ParseOptions,
    resources: InputResourcePolicy,
}

impl Default for Renderer {
    fn default() -> Self {
        Self {
            engine: Engine::new(),
            parse_options: ParseOptions::default(),
            resources: InputResourcePolicy::default(),
        }
    }
}

impl Renderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_engine(mut self, engine: Engine) -> Self {
        self.engine = engine;
        self
    }

    pub fn with_runtime_policy(mut self, policy: merman_core::runtime::RuntimePolicy) -> Self {
        self.engine = self.engine.with_runtime_policy(policy);
        self
    }

    pub fn with_parse_options(mut self, parse_options: ParseOptions) -> Self {
        self.parse_options = parse_options;
        self
    }

    pub fn with_resource_policy(mut self, resources: InputResourcePolicy) -> Self {
        self.resources = resources;
        self
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn parse_options(&self) -> ParseOptions {
        self.parse_options
    }

    pub fn resource_policy(&self) -> &InputResourcePolicy {
        &self.resources
    }

    /// Executes one typed target request through the canonical operation runner.
    pub fn render(&self, request: RenderRequest<'_>) -> Result<RenderOutput, RenderError> {
        let RenderRequest {
            source,
            target,
            control,
            parse_options,
            resources,
            #[cfg(feature = "svg")]
            theme,
        } = request;
        #[cfg(feature = "svg")]
        let operation_engine = theme
            .as_ref()
            .map(|theme| {
                merman_render::__private::install_parse_compatibility(theme, self.engine.clone())
            })
            .unwrap_or_else(|| self.engine.clone());
        #[cfg(not(feature = "svg"))]
        let operation_engine = self.engine.clone();
        let operation = Operation::begin(
            &operation_engine,
            source,
            control,
            resources.unwrap_or(self.resources),
            #[cfg(feature = "svg")]
            theme,
        )?;
        let semantic =
            operation.parse_render_model(source, parse_options.unwrap_or(self.parse_options))?;
        let Some(semantic) = semantic else {
            return Ok(RenderOutput::empty(target));
        };
        semantic.render(target)
    }

    /// Prepares a format-neutral semantic artifact through the same runner used by `render`.
    pub fn prepare_semantic(
        &self,
        source: &str,
        control: OperationControl,
    ) -> Result<Option<SemanticArtifact>, RenderError> {
        self.prepare_semantic_with(source, control, self.parse_options, self.resources)
    }

    pub fn prepare_semantic_with(
        &self,
        source: &str,
        control: OperationControl,
        parse_options: ParseOptions,
        resources: InputResourcePolicy,
    ) -> Result<Option<SemanticArtifact>, RenderError> {
        let operation = Operation::begin(
            &self.engine,
            source,
            control,
            resources,
            #[cfg(feature = "svg")]
            None,
        )?;
        operation.parse_render_model(source, parse_options)
    }
}

impl SemanticArtifact {
    /// Returns Mermaid's compatibility semantic JSON projection without exposing family internals.
    pub fn compatibility_json(&self) -> Result<serde_json::Value, RenderError> {
        self.parsed()
            .model()
            .compatibility_json_controlled(self.parsed().metadata(), self.control())
            .map_err(RenderError::Cancelled)?
            .map_err(RenderError::Parse)
    }

    /// Consumes this operation-owned semantic artifact into one typed output target.
    pub fn render(self, target: RenderTarget) -> Result<RenderOutput, RenderError> {
        match target {
            RenderTarget::Semantic => Ok(RenderOutput::Semantic(Some(self))),
            #[cfg(feature = "svg")]
            RenderTarget::Svg(request) => render_svg_target(self, request).map(RenderOutput::Svg),
            #[cfg(feature = "svg")]
            RenderTarget::FinalizedSvg(request) => {
                render_finalized_svg_target(self, request).map(RenderOutput::FinalizedSvg)
            }
            #[cfg(feature = "svg")]
            RenderTarget::LayoutJson(request) => {
                render_layout_json_target(self, request).map(RenderOutput::LayoutJson)
            }
            #[cfg(feature = "svg")]
            RenderTarget::SvgPlan(request) => {
                render_svg_plan_target(self, request).map(RenderOutput::SvgPlan)
            }
            #[cfg(feature = "ascii")]
            RenderTarget::Ascii(request) => {
                render_ascii_target(self, request).map(RenderOutput::Ascii)
            }
            #[cfg(feature = "png")]
            RenderTarget::Png(request) => render_png_target(self, request).map(RenderOutput::Png),
            #[cfg(feature = "jpeg")]
            RenderTarget::Jpeg(request) => {
                render_jpeg_target(self, request).map(RenderOutput::Jpeg)
            }
            #[cfg(feature = "pdf")]
            RenderTarget::Pdf(request) => render_pdf_target(self, request).map(RenderOutput::Pdf),
        }
    }
}

impl RenderOutput {
    fn empty(target: RenderTarget) -> Self {
        match target {
            RenderTarget::Semantic => Self::Semantic(None),
            #[cfg(feature = "svg")]
            RenderTarget::Svg(_) => Self::Svg(None),
            #[cfg(feature = "svg")]
            RenderTarget::FinalizedSvg(_) => Self::FinalizedSvg(None),
            #[cfg(feature = "svg")]
            RenderTarget::LayoutJson(_) => Self::LayoutJson(None),
            #[cfg(feature = "svg")]
            RenderTarget::SvgPlan(_) => Self::SvgPlan(None),
            #[cfg(feature = "ascii")]
            RenderTarget::Ascii(_) => Self::Ascii(None),
            #[cfg(feature = "png")]
            RenderTarget::Png(_) => Self::Png(None),
            #[cfg(feature = "jpeg")]
            RenderTarget::Jpeg(_) => Self::Jpeg(None),
            #[cfg(feature = "pdf")]
            RenderTarget::Pdf(_) => Self::Pdf(None),
        }
    }
}

#[cfg(feature = "svg")]
fn render_svg_target(
    semantic: SemanticArtifact,
    request: SvgRequest,
) -> Result<Option<SvgOutput>, RenderError> {
    let (parsed, operation) = semantic.into_parts();
    let session = request.environment.begin_session_in_context(
        operation.theme.as_ref(),
        operation.context.clone(),
        operation.control.clone(),
    )?;
    let artifact =
        merman_render::family::prepare(parsed, &request.layout, session).map_err(map_svg_error)?;
    let rendered = artifact
        .render_svg(&request.options, &request.debug)
        .map_err(map_svg_error)?;
    let rendered = match request.pipeline.as_ref() {
        Some(pipeline) => rendered.apply_pipeline(pipeline).map_err(map_svg_error)?,
        None => rendered,
    };
    let (svg, family) = rendered.into_completion().into_output_and_report();
    Ok(Some(SvgOutput::new(svg, family)))
}

#[cfg(feature = "svg")]
fn render_layout_json_target(
    semantic: SemanticArtifact,
    request: SvgRequest,
) -> Result<Option<SvgLayoutOutput>, RenderError> {
    let (parsed, operation) = semantic.into_parts();
    let session = request.environment.begin_session_in_context(
        operation.theme.as_ref(),
        operation.context,
        operation.control,
    )?;
    let artifact =
        merman_render::family::prepare(parsed, &request.layout, session).map_err(map_svg_error)?;
    let gantt_time_axis = artifact.gantt_time_axis_diagnostics();
    let layout = artifact.layout_json().map_err(map_svg_error)?;
    Ok(Some(SvgLayoutOutput::new(layout, gantt_time_axis)))
}

#[cfg(feature = "svg")]
fn render_svg_plan_target(
    semantic: SemanticArtifact,
    request: SvgRequest,
) -> Result<Option<merman_render::family::RenderCapabilityPlan>, RenderError> {
    let (parsed, operation) = semantic.into_parts();
    let session = request.environment.begin_session_in_context(
        operation.theme.as_ref(),
        operation.context,
        operation.control,
    )?;
    merman_render::family::plan_render(&parsed, &session)
        .map(Some)
        .map_err(map_svg_error)
}

#[cfg(feature = "svg")]
fn prepare_resvg_target(
    semantic: SemanticArtifact,
    request: &SvgRequest,
) -> Result<
    Option<(
        merman_render::svg::ResvgCompatibleSvg,
        merman_render::family::FamilyRenderReport,
        OperationExecution,
    )>,
    RenderError,
> {
    let (parsed, operation) = semantic.into_parts();
    let session = request.environment.begin_session_in_context(
        operation.theme.as_ref(),
        operation.context.clone(),
        operation.control.clone(),
    )?;
    let artifact =
        merman_render::family::prepare(parsed, &request.layout, session).map_err(map_svg_error)?;
    let pipeline = request
        .pipeline
        .clone()
        .unwrap_or_else(SvgPipeline::resvg_safe)
        .into_resvg_safe();
    artifact
        .render_svg(&request.options, &request.debug)
        .map_err(map_svg_error)?
        .finalize_resvg(&pipeline)
        .map(|sealed| {
            let (svg, family) = sealed.into_completion().into_output_and_report();
            (svg, family, operation)
        })
        .map(Some)
        .map_err(map_svg_error)
}

#[cfg(feature = "svg")]
fn render_finalized_svg_target(
    semantic: SemanticArtifact,
    request: SvgRequest,
) -> Result<Option<FinalizedSvgOutput>, RenderError> {
    let Some((svg, family, _operation)) = prepare_resvg_target(semantic, &request)? else {
        unreachable!("semantic artifact always produces a finalized SVG or an error")
    };
    Ok(Some(FinalizedSvgOutput::new(svg, family)))
}

#[cfg(feature = "ascii")]
fn render_ascii_target(
    semantic: SemanticArtifact,
    request: AsciiRequest,
) -> Result<Option<String>, RenderError> {
    let (parsed, operation) = semantic.into_parts();
    merman_ascii::render_model_with_operation(
        parsed.model(),
        &request.options,
        &operation.control,
        &operation.context,
        request.resources,
    )
    .map(Some)
    .map_err(map_ascii_error)
}

#[cfg(feature = "png")]
fn render_png_target(
    semantic: SemanticArtifact,
    request: PngRequest,
) -> Result<Option<RasterOutput>, RenderError> {
    let Some((svg, family, operation)) = prepare_resvg_target(semantic, &request.svg)? else {
        unreachable!("semantic artifact always produces a sealed SVG or an error")
    };
    merman_export::prepare_raster_controlled(&svg, &request.options, operation.control)
        .and_then(merman_export::PreparedRaster::encode_png_with_report)
        .map(|(bytes, export_report)| {
            Some(RasterOutput {
                bytes,
                plan: export_report.raster(),
                evidence: RenderEvidence::from_family(family),
                export_report,
            })
        })
        .map_err(map_export_error)
}

#[cfg(feature = "jpeg")]
fn render_jpeg_target(
    semantic: SemanticArtifact,
    request: JpegRequest,
) -> Result<Option<RasterOutput>, RenderError> {
    let Some((svg, family, operation)) = prepare_resvg_target(semantic, &request.svg)? else {
        unreachable!("semantic artifact always produces a sealed SVG or an error")
    };
    merman_export::prepare_raster_controlled(&svg, &request.options, operation.control)
        .and_then(merman_export::PreparedRaster::encode_jpeg_with_report)
        .map(|(bytes, export_report)| {
            Some(RasterOutput {
                bytes,
                plan: export_report.raster(),
                evidence: RenderEvidence::from_family(family),
                export_report,
            })
        })
        .map_err(map_export_error)
}

#[cfg(feature = "pdf")]
fn render_pdf_target(
    semantic: SemanticArtifact,
    request: PdfRequest,
) -> Result<Option<PdfOutput>, RenderError> {
    let Some((svg, family, operation)) = prepare_resvg_target(semantic, &request.svg)? else {
        unreachable!("semantic artifact always produces a sealed SVG or an error")
    };
    merman_export::prepare_pdf_controlled(&svg, &request.options, operation.control)
        .and_then(merman_export::PreparedPdf::encode_with_report)
        .map(|(bytes, export_report)| {
            Some(PdfOutput {
                bytes,
                plan: export_report.filters(),
                evidence: RenderEvidence::from_family(family),
                export_report,
            })
        })
        .map_err(map_export_error)
}

#[cfg(all(test, feature = "svg"))]
mod tests {
    use super::{
        RenderError, ResourceLimitCause, ThemeEvidenceScopeProjection, ThemeEvidenceStatus,
        ThemeEvidenceSummary, summarize_theme_evidence,
    };

    #[test]
    fn theme_resource_environment_error_maps_to_resource_limit_exceeded() {
        let policy = merman_render::diagram_theme::ThemeResourcePolicy::constrained();
        let maximum = policy
            .value(merman_render::diagram_theme::ThemeResourceLimitId::MaxThemeEncodedBytes)
            .expect("constrained theme input ceiling");
        let resource = policy
            .check_theme_encoded_bytes(maximum + 1)
            .expect_err("fixture must exceed the constrained theme input ceiling");

        let error = RenderError::from(
            merman_render::environment::RenderEnvironmentError::ThemeResource(resource),
        );
        let RenderError::ResourceLimitExceeded(resource) = error else {
            panic!("theme resource environment errors must use the common resource variant");
        };
        assert_eq!(resource.id, "max_theme_encoded_bytes");
        assert_eq!(resource.phase, "theme_input");
        assert_eq!(resource.actual, (maximum + 1) as u64);
        assert_eq!(resource.maximum, maximum as u64);
        assert_eq!(resource.cause, ResourceLimitCause::Ceiling);
    }

    #[test]
    fn render_environment_cancellation_maps_to_the_common_cancelled_variant() {
        let cancelled = merman_core::OperationCancelled {
            phase: merman_core::OperationPhase::Layout,
            reason: merman_core::CancelReason::Requested,
        };

        let error = RenderError::from(
            merman_render::environment::RenderEnvironmentError::Cancelled(cancelled),
        );
        let RenderError::Cancelled(mapped) = error else {
            panic!("render-environment cancellation must use the common cancelled variant");
        };
        assert_eq!(mapped, cancelled);
    }

    #[test]
    fn terminal_theme_resource_render_error_maps_to_resource_limit_exceeded() {
        let policy = merman_render::diagram_theme::ThemeResourcePolicy::constrained();
        let maximum = policy
            .value(merman_render::diagram_theme::ThemeResourceLimitId::MaxThemeEncodedBytes)
            .expect("constrained theme input ceiling");
        let resource = policy
            .check_theme_encoded_bytes(maximum + 1)
            .expect_err("fixture must exceed the constrained theme input ceiling");

        let error = RenderError::from(merman_render::Error::ThemeResourceLimitExceeded(resource));
        let RenderError::ResourceLimitExceeded(resource) = error else {
            panic!("terminal theme resource errors must use the common resource variant");
        };
        assert_eq!(resource.id, "max_theme_encoded_bytes");
        assert_eq!(resource.phase, "theme_input");
        assert_eq!(resource.actual, (maximum + 1) as u64);
        assert_eq!(resource.maximum, maximum as u64);
        assert_eq!(resource.cause, ResourceLimitCause::Ceiling);
    }

    #[test]
    fn theme_evidence_status_exposes_the_complete_coarse_catalog() {
        assert_eq!(
            ThemeEvidenceStatus::ALL,
            &[
                ThemeEvidenceStatus::NotApplicable,
                ThemeEvidenceStatus::Verified,
                ThemeEvidenceStatus::Residual,
                ThemeEvidenceStatus::Incomplete,
            ]
        );
    }

    #[test]
    fn theme_evidence_summary_rejects_output_mutation() {
        let summary = ThemeEvidenceSummary::new(ThemeEvidenceStatus::Verified, true);

        assert!(!summary.is_verified());
        assert!(!summary.is_satisfied());
    }

    #[test]
    fn not_applicable_theme_evidence_is_satisfied_but_not_verified() {
        let summary = ThemeEvidenceSummary::new(ThemeEvidenceStatus::NotApplicable, false);

        assert!(!summary.is_verified());
        assert!(summary.is_satisfied());
    }

    #[test]
    fn incomplete_theme_evidence_takes_precedence_over_residuals() {
        let root = ThemeEvidenceScopeProjection {
            status: ThemeEvidenceStatus::Incomplete,
            required_count: 2,
            accounted_count: 1,
            applied_count: 0,
            not_applicable_count: 0,
            residual_count: 1,
            output_mutated: false,
        };
        let family = ThemeEvidenceScopeProjection {
            status: ThemeEvidenceStatus::Residual,
            required_count: 1,
            accounted_count: 1,
            applied_count: 0,
            not_applicable_count: 0,
            residual_count: 1,
            output_mutated: false,
        };

        let summary = summarize_theme_evidence(root, family, 1, 1, 1);

        assert_eq!(summary.status(), ThemeEvidenceStatus::Incomplete);
        assert!(!summary.is_satisfied());
    }
}

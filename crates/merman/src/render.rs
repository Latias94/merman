//! Canonical source-to-target rendering facade.
//!
//! `Renderer` owns long-lived defaults. Each [`RenderRequest`] owns one operation control and is
//! executed synchronously through the same internal operation runner. Target-specific layout and
//! emission remain private to their adapters.

#[cfg(feature = "svg")]
use std::sync::Arc;

use merman_core::{
    Engine, OperationCancelled, OperationControl, ParseOptions, resources::InputResourcePolicy,
    runtime::RuntimePolicyError,
};
#[cfg(feature = "svg")]
use sha2::{Digest as _, Sha256};

use crate::operation_runner::Operation;
#[cfg(feature = "svg")]
use crate::operation_runner::OperationExecution;

#[cfg(feature = "svg")]
const TARGET_EVIDENCE_DIGEST_DOMAIN: &[u8] = b"merman-target-evidence-experimental-v3";

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

    const fn portability_requirement(
        &self,
    ) -> merman_render::diagram_theme::ThemePortabilityRequirement {
        self.backend.theme_portability_requirement()
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
    /// Alpha, revision-scoped discovery view for the coarse evidence states exposed by the facade.
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

/// Artifact kind covered by one target-owned admission receipt.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RenderArtifactKind {
    Svg,
    Png,
    Jpeg,
    Pdf,
}

#[cfg(feature = "svg")]
impl RenderArtifactKind {
    /// Complete target vocabulary for this API revision, independent of enabled Cargo features.
    pub const ALL: &'static [Self] = &[Self::Svg, Self::Png, Self::Jpeg, Self::Pdf];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Pdf => "pdf",
        }
    }
}

/// Terminal portability classification produced by the target adapter itself.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TargetAdmissionStatus {
    Portable,
    HostDependent,
    Rejected,
}

#[cfg(feature = "svg")]
impl TargetAdmissionStatus {
    pub const ALL: &'static [Self] = &[Self::Portable, Self::HostDependent, Self::Rejected];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Portable => "portable",
            Self::HostDependent => "host_dependent",
            Self::Rejected => "rejected",
        }
    }

    pub const fn is_portable(self) -> bool {
        matches!(self, Self::Portable)
    }
}

/// Font-source summary observed by one concrete output target.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TargetFontSource {
    None,
    Embedded,
    System,
    Mixed,
}

#[cfg(feature = "svg")]
impl TargetFontSource {
    pub const ALL: &'static [Self] = &[Self::None, Self::Embedded, Self::System, Self::Mixed];

    pub const fn id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Embedded => "embedded",
            Self::System => "system",
            Self::Mixed => "mixed",
        }
    }
}

/// Machine-readable reason attached to document and target admission evidence.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TargetAdmissionReason {
    ThemeEvidenceIncomplete,
    PreparedTextLayoutFailed,
    HostDependentTextLayout,
    PreparedTextEvidenceInvalid,
    SvgFontsNotSelfContained,
    PreparedTextEvidenceMismatch,
    PreparedTextTerminalProofIncomplete,
    FontResolutionIncomplete,
    SystemOrHostFontDependency,
    ResourceFingerprintMismatch,
    FontCatalogFingerprintMismatch,
    TargetKindMismatch,
    NativeFilterReceiptMismatch,
    PdfNativeFilterNotLocalized,
}

#[cfg(feature = "svg")]
impl TargetAdmissionReason {
    pub const ALL: &'static [Self] = &[
        Self::ThemeEvidenceIncomplete,
        Self::PreparedTextLayoutFailed,
        Self::HostDependentTextLayout,
        Self::PreparedTextEvidenceInvalid,
        Self::SvgFontsNotSelfContained,
        Self::PreparedTextEvidenceMismatch,
        Self::PreparedTextTerminalProofIncomplete,
        Self::FontResolutionIncomplete,
        Self::SystemOrHostFontDependency,
        Self::ResourceFingerprintMismatch,
        Self::FontCatalogFingerprintMismatch,
        Self::TargetKindMismatch,
        Self::NativeFilterReceiptMismatch,
        Self::PdfNativeFilterNotLocalized,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::ThemeEvidenceIncomplete => "theme_evidence_incomplete",
            Self::PreparedTextLayoutFailed => "prepared_text_layout_failed",
            Self::HostDependentTextLayout => "host_dependent_text_layout",
            Self::PreparedTextEvidenceInvalid => "prepared_text_evidence_invalid",
            Self::SvgFontsNotSelfContained => "svg_fonts_not_self_contained",
            Self::PreparedTextEvidenceMismatch => "prepared_text_evidence_mismatch",
            Self::PreparedTextTerminalProofIncomplete => "prepared_text_terminal_proof_incomplete",
            Self::FontResolutionIncomplete => "font_resolution_incomplete",
            Self::SystemOrHostFontDependency => "system_or_host_font_dependency",
            Self::ResourceFingerprintMismatch => "resource_fingerprint_mismatch",
            Self::FontCatalogFingerprintMismatch => "font_catalog_fingerprint_mismatch",
            Self::TargetKindMismatch => "target_kind_mismatch",
            Self::NativeFilterReceiptMismatch => "native_filter_receipt_mismatch",
            Self::PdfNativeFilterNotLocalized => "pdf_native_filter_not_localized",
        }
    }
}

/// Coarse document-stage evidence frozen before any target-specific export.
///
/// This report deliberately does not use [`TargetAdmissionStatus`]. Document proof and target
/// admission are separate monotonic stages: targets may add constraints, but cannot erase a
/// document rejection or host dependency.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentPortabilityReport {
    family_id: merman_core::DiagramFamilyId,
    theme_evidence: ThemeEvidenceSummary,
    resource_fingerprint: merman_render::svg::SvgResourceFingerprint,
    font_catalog_fingerprint: merman_render::diagram_theme::FontCatalogFingerprint,
    prepared_text_evidence_valid: bool,
    rejected: bool,
    host_dependent: bool,
    reasons: Box<[TargetAdmissionReason]>,
}

#[cfg(feature = "svg")]
impl DocumentPortabilityReport {
    pub const fn family_id(&self) -> merman_core::DiagramFamilyId {
        self.family_id
    }

    pub const fn theme_evidence(&self) -> ThemeEvidenceSummary {
        self.theme_evidence
    }

    pub const fn resource_fingerprint(&self) -> merman_render::svg::SvgResourceFingerprint {
        self.resource_fingerprint
    }

    pub const fn font_catalog_fingerprint(
        &self,
    ) -> merman_render::diagram_theme::FontCatalogFingerprint {
        self.font_catalog_fingerprint
    }

    pub const fn resource_closure_complete(&self) -> bool {
        true
    }

    pub const fn prepared_text_evidence_valid(&self) -> bool {
        self.prepared_text_evidence_valid
    }

    pub const fn is_host_dependent(&self) -> bool {
        self.host_dependent
    }

    /// Returns whether the document-stage evidence is internally valid.
    ///
    /// A valid document may still depend on host fonts or measurement. Use
    /// [`Self::is_host_dependent`] and the target admission receipt when deciding whether an
    /// artifact is portable.
    pub const fn is_evidence_valid(&self) -> bool {
        self.prepared_text_evidence_valid && !self.rejected
    }

    pub fn reasons(&self) -> &[TargetAdmissionReason] {
        &self.reasons
    }
}

/// Immutable target-owned admission bound to one completed document and exact artifact bytes.
///
/// This type deliberately has no public constructor or `from_parts` API. Binary output types keep
/// the receipt next to immutable bytes; consuming `into_bytes()` is the explicit lossy escape hatch.
///
/// ```compile_fail
/// use merman::{RenderArtifactKind, TargetAdmissionReceipt, TargetAdmissionStatus};
///
/// let forged = TargetAdmissionReceipt {
///     artifact_kind: RenderArtifactKind::Svg,
///     status: TargetAdmissionStatus::Portable,
///     reasons: Box::new([]),
///     font_source: merman::TargetFontSource::Embedded,
///     resource_fingerprint: todo!(),
///     font_catalog_fingerprint: todo!(),
///     document_digest: [0; 32],
///     target_evidence_digest: [0; 32],
///     artifact_digest: [0; 32],
///     receipt_digest: [0; 32],
/// };
/// ```
#[cfg(feature = "svg")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetAdmissionReceipt {
    artifact_kind: RenderArtifactKind,
    status: TargetAdmissionStatus,
    reasons: Box<[TargetAdmissionReason]>,
    font_source: TargetFontSource,
    resource_fingerprint: merman_render::svg::SvgResourceFingerprint,
    font_catalog_fingerprint: merman_render::diagram_theme::FontCatalogFingerprint,
    document_digest: [u8; 32],
    target_evidence_digest: [u8; 32],
    artifact_digest: [u8; 32],
    receipt_digest: [u8; 32],
}

#[cfg(feature = "svg")]
impl TargetAdmissionReceipt {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn seal(
        artifact_kind: RenderArtifactKind,
        status: TargetAdmissionStatus,
        reasons: Vec<TargetAdmissionReason>,
        font_source: TargetFontSource,
        resource_fingerprint: merman_render::svg::SvgResourceFingerprint,
        font_catalog_fingerprint: merman_render::diagram_theme::FontCatalogFingerprint,
        document_digest: [u8; 32],
        target_evidence_digest: [u8; 32],
        artifact_digest: [u8; 32],
    ) -> Self {
        let reasons = reasons.into_boxed_slice();
        let receipt_digest = target_admission_receipt_digest(
            artifact_kind,
            status,
            &reasons,
            font_source,
            resource_fingerprint.as_bytes(),
            font_catalog_fingerprint.as_bytes(),
            document_digest,
            target_evidence_digest,
            artifact_digest,
        );
        Self {
            artifact_kind,
            status,
            reasons,
            font_source,
            resource_fingerprint,
            font_catalog_fingerprint,
            document_digest,
            target_evidence_digest,
            artifact_digest,
            receipt_digest,
        }
    }

    pub const fn artifact_kind(&self) -> RenderArtifactKind {
        self.artifact_kind
    }

    pub const fn status(&self) -> TargetAdmissionStatus {
        self.status
    }

    pub fn reasons(&self) -> &[TargetAdmissionReason] {
        &self.reasons
    }

    pub const fn font_source(&self) -> TargetFontSource {
        self.font_source
    }

    pub const fn resource_fingerprint(&self) -> merman_render::svg::SvgResourceFingerprint {
        self.resource_fingerprint
    }

    pub const fn font_catalog_fingerprint(
        &self,
    ) -> merman_render::diagram_theme::FontCatalogFingerprint {
        self.font_catalog_fingerprint
    }

    pub const fn document_digest(&self) -> [u8; 32] {
        self.document_digest
    }

    /// Returns the experimental identity of the target evidence encoded by this receipt.
    ///
    /// This value is suitable for correlating artifacts produced by one Merman API revision. It
    /// is not a stable cross-release cache key until the target-evidence contract is frozen.
    pub const fn target_evidence_digest(&self) -> [u8; 32] {
        self.target_evidence_digest
    }

    pub const fn artifact_digest(&self) -> [u8; 32] {
        self.artifact_digest
    }

    /// Returns the revision-scoped canonical digest of this sealed target receipt.
    ///
    /// The digest binds artifact kind, status, ordered reasons, font source, resource and font
    /// catalog fingerprints, document identity, target evidence, and artifact identity. Its
    /// encoding remains experimental and may change between Merman API revisions.
    pub const fn receipt_digest(&self) -> [u8; 32] {
        self.receipt_digest
    }
}

/// Strict-portability rejection returned when target-owned evidence is not Portable.
#[cfg(feature = "svg")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetAdmissionError {
    receipt: TargetAdmissionReceipt,
}

#[cfg(feature = "svg")]
impl TargetAdmissionError {
    fn new(receipt: TargetAdmissionReceipt) -> Self {
        Self { receipt }
    }

    pub const fn receipt(&self) -> &TargetAdmissionReceipt {
        &self.receipt
    }
}

#[cfg(feature = "svg")]
impl std::fmt::Display for TargetAdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} target requires Portable admission, got {} ({})",
            self.receipt.artifact_kind.id(),
            self.receipt.status.id(),
            self.receipt
                .reasons
                .iter()
                .map(|reason| reason.id())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

#[cfg(feature = "svg")]
impl std::error::Error for TargetAdmissionError {}

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

#[cfg(all(feature = "svg", feature = "internal-theme-acceptance"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ThemeAcceptanceEvidenceSnapshot {
    root: ThemeEvidenceScopeProjection,
    family: ThemeEvidenceScopeProjection,
    source_residual_count: usize,
    compatibility_residual_count: usize,
    mermaid_compatibility_residual_count: usize,
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
    native_filter_receipt: Option<merman_render::__private::NativeSvgFilterReceipt>,
    #[cfg(feature = "internal-theme-acceptance")]
    theme_acceptance: ThemeAcceptanceEvidenceSnapshot,
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
        let native_filter_receipt = merman_render::__private::family_native_filter_receipt(&family);
        Self {
            session,
            family_id,
            theme_evidence,
            native_filter_receipt,
            #[cfg(feature = "internal-theme-acceptance")]
            theme_acceptance: ThemeAcceptanceEvidenceSnapshot {
                root,
                family: family_scope,
                source_residual_count,
                compatibility_residual_count,
                mermaid_compatibility_residual_count,
            },
        }
    }

    const fn session(&self) -> &merman_render::environment::RenderSessionReport {
        &self.session
    }

    fn portability_requirement(&self) -> merman_render::diagram_theme::ThemePortabilityRequirement {
        self.session().portability_requirement()
    }

    fn prepared_text_layout(&self) -> Option<&merman_render::text::PreparedTextLayoutReport> {
        self.session().prepared_text_layout()
    }

    fn text_layout_failure(&self) -> Option<merman_render::text::TextLayoutFailure> {
        self.session().text_layout_failure()
    }

    pub(crate) fn native_filter_receipt(
        &self,
    ) -> Option<merman_render::__private::NativeSvgFilterReceipt> {
        self.native_filter_receipt
    }

    pub fn measurement_routes(&self) -> &[merman_render::environment::TextMeasurementRoute; 4] {
        self.session().measurement_routes()
    }

    pub fn measurement(&self) -> &merman_render::environment::TextMeasurementReport {
        self.session().measurement()
    }

    pub fn operation_context(&self) -> &merman_core::runtime::OperationContext {
        self.session().operation_context()
    }

    pub const fn unix_millis(&self) -> i64 {
        self.session().unix_millis()
    }

    pub const fn local_date(&self) -> merman_core::time::CivilDate {
        self.session().local_date()
    }

    pub fn local_time_zone(&self) -> &merman_core::time::LocalTimeZoneProvenance {
        self.session().local_time_zone()
    }

    pub fn render_seed(&self) -> std::num::NonZeroU64 {
        self.session().render_seed()
    }

    pub const fn layout_work_units(&self) -> usize {
        self.session().layout_work_units()
    }

    pub const fn family_id(&self) -> merman_core::DiagramFamilyId {
        self.family_id
    }

    pub fn theme_recipe_fingerprint(
        &self,
    ) -> Option<merman_render::diagram_theme::ThemeRecipeFingerprint> {
        self.session().theme_recipe_fingerprint()
    }

    pub const fn font_catalog_fingerprint(
        &self,
    ) -> merman_render::diagram_theme::FontCatalogFingerprint {
        self.session().font_catalog_fingerprint()
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
            recipe_report: self.session().theme_recipe_report(),
            host_admission_report: self.session().theme_host_admission_report(),
            prepared_text_layout: self.session().prepared_text_layout(),
            text_layout_failure: self.session().text_layout_failure(),
            effective_theme_resource_policy:
                merman_render::__private::effective_theme_resource_policy(self.session()),
        }
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
    admission: Option<TargetAdmissionReceipt>,
}

/// Completed graphical document retained as the single source for every terminal target.
///
/// The document owns its sealed SVG, frozen family/session evidence, and standalone-SVG target
/// admission. PNG, JPEG, and PDF projections reuse this exact object; they never re-enter parsing,
/// layout, SVG emission, or terminal finalization. Completing the document does not enforce the
/// standalone-SVG receipt: each terminal target owns its own admission decision, so a
/// host-dependent SVG cannot preempt an otherwise portable native projection.
#[cfg(feature = "svg")]
#[derive(Debug, PartialEq, Eq)]
pub struct RenderedDocument {
    svg: merman_render::svg::ResvgCompatibleSvg,
    evidence: Arc<RenderEvidence>,
    portability: DocumentPortabilityReport,
    standalone_svg_admission: TargetAdmissionReceipt,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
struct PreparedRasterTarget<'a> {
    document: &'a RenderedDocument,
    export: merman_export::PreparedRaster,
    report: merman_export::RasterExportReport,
}

/// PNG export prepared from one completed document but not yet encoded.
#[cfg(feature = "png")]
pub struct PreparedPngExport<'a> {
    inner: PreparedRasterTarget<'a>,
}

#[cfg(feature = "png")]
impl PreparedPngExport<'_> {
    pub const fn export_report(&self) -> merman_export::RasterExportReport {
        self.inner.report
    }

    pub const fn plan(&self) -> merman_export::RasterPlan {
        self.inner.report.raster()
    }

    pub fn encode(self) -> Result<RasterOutput, RenderError> {
        let PreparedRasterTarget {
            document,
            export,
            report: planned_report,
        } = self.inner;
        let (bytes, report) = export.encode_png_with_report().map_err(map_export_error)?;
        debug_assert_eq!(report, planned_report);
        document.finish_raster_export(bytes, report)
    }
}

/// JPEG export prepared from one completed document but not yet encoded.
#[cfg(feature = "jpeg")]
pub struct PreparedJpegExport<'a> {
    inner: PreparedRasterTarget<'a>,
}

#[cfg(feature = "jpeg")]
impl PreparedJpegExport<'_> {
    pub const fn export_report(&self) -> merman_export::RasterExportReport {
        self.inner.report
    }

    pub const fn plan(&self) -> merman_export::RasterPlan {
        self.inner.report.raster()
    }

    pub fn encode(self) -> Result<RasterOutput, RenderError> {
        let PreparedRasterTarget {
            document,
            export,
            report: planned_report,
        } = self.inner;
        let (bytes, report) = export.encode_jpeg_with_report().map_err(map_export_error)?;
        debug_assert_eq!(report, planned_report);
        document.finish_raster_export(bytes, report)
    }
}

/// PDF export prepared from one completed document but not yet encoded.
#[cfg(feature = "pdf")]
pub struct PreparedPdfExport<'a> {
    document: &'a RenderedDocument,
    export: merman_export::PreparedPdf,
    report: merman_export::PdfExportReport,
}

#[cfg(feature = "pdf")]
impl PreparedPdfExport<'_> {
    pub const fn export_report(&self) -> merman_export::PdfExportReport {
        self.report
    }

    pub const fn plan(&self) -> merman_export::PdfFilterImagePlan {
        self.report.filters()
    }

    pub fn encode(self) -> Result<PdfOutput, RenderError> {
        let Self {
            document,
            export,
            report: planned_report,
        } = self;
        let (bytes, report) = export.encode_with_report().map_err(map_export_error)?;
        debug_assert_eq!(report, planned_report);
        document.finish_pdf_export(bytes, report)
    }
}

#[cfg(feature = "svg")]
impl RenderedDocument {
    fn new(
        svg: merman_render::svg::ResvgCompatibleSvg,
        family: merman_render::family::FamilyRenderReport,
    ) -> Self {
        let evidence = Arc::new(RenderEvidence::from_family(family));
        let portability = document_portability_report(&svg, &evidence);
        let public_svg_digest = artifact_digest(svg.as_str().as_bytes());
        let native_svg = merman_render::__private::native_export_svg(&svg);
        let native_svg_digest = if native_svg == svg.as_str() {
            public_svg_digest
        } else {
            artifact_digest(native_svg.as_bytes())
        };
        let document_digest = document_digest(
            public_svg_digest,
            native_svg_digest,
            svg.resource_fingerprint(),
            &evidence,
            &portability,
        );
        let standalone_svg_admission = standalone_svg_admission(
            &svg,
            &evidence,
            &portability,
            document_digest,
            public_svg_digest,
        );
        Self {
            svg,
            evidence,
            portability,
            standalone_svg_admission,
        }
    }

    pub fn svg(&self) -> &str {
        self.svg.as_str()
    }

    /// Borrows the immutable terminal SVG together with its retained export resources and proofs.
    pub const fn sealed_svg(&self) -> &merman_render::svg::ResvgCompatibleSvg {
        &self.svg
    }

    pub fn evidence(&self) -> &RenderEvidence {
        &self.evidence
    }

    pub const fn portability(&self) -> &DocumentPortabilityReport {
        &self.portability
    }

    pub const fn resource_fingerprint(&self) -> merman_render::svg::SvgResourceFingerprint {
        self.svg.resource_fingerprint()
    }

    /// Returns the experimental identity of this completed document.
    ///
    /// The digest binds the sealed SVG, resources, operation context, and document-stage evidence.
    /// Its canonical encoding remains alpha until the document contract is frozen.
    pub const fn document_digest(&self) -> [u8; 32] {
        self.standalone_svg_admission.document_digest()
    }

    /// Returns the standalone-SVG target receipt without interpreting it as document admission.
    ///
    /// Callers that publish the SVG directly must apply their own required-admission policy to this
    /// receipt. Native `export_*` projections enforce the document's portability requirement using
    /// their own target-owned receipts.
    pub const fn standalone_svg_admission(&self) -> &TargetAdmissionReceipt {
        &self.standalone_svg_admission
    }

    fn into_admitted_svg_output(self) -> Result<SvgOutput, RenderError> {
        enforce_portability_requirement(&self.evidence, &self.standalone_svg_admission)?;
        let Self {
            svg,
            evidence,
            portability: _,
            standalone_svg_admission,
        } = self;
        Ok(SvgOutput {
            svg: svg.into_string(),
            evidence: (*evidence).clone(),
            admission: Some(standalone_svg_admission),
        })
    }

    #[cfg(feature = "png")]
    pub fn prepare_png_export(
        &self,
        options: &merman_export::RasterOptions,
        control: OperationControl,
    ) -> Result<PreparedPngExport<'_>, RenderError> {
        let export = merman_export::prepare_raster_controlled(&self.svg, options, control)
            .map_err(map_export_error)?;
        let report = export.report_for_output(merman_export::RasterOutputKind::Png);
        Ok(PreparedPngExport {
            inner: PreparedRasterTarget {
                document: self,
                export,
                report,
            },
        })
    }

    #[cfg(feature = "png")]
    pub fn export_png(
        &self,
        options: &merman_export::RasterOptions,
        control: OperationControl,
    ) -> Result<RasterOutput, RenderError> {
        self.prepare_png_export(options, control)?.encode()
    }

    #[cfg(feature = "jpeg")]
    pub fn prepare_jpeg_export(
        &self,
        options: &merman_export::RasterOptions,
        control: OperationControl,
    ) -> Result<PreparedJpegExport<'_>, RenderError> {
        let export = merman_export::prepare_raster_controlled(&self.svg, options, control)
            .map_err(map_export_error)?;
        let report = export.report_for_output(merman_export::RasterOutputKind::Jpeg);
        Ok(PreparedJpegExport {
            inner: PreparedRasterTarget {
                document: self,
                export,
                report,
            },
        })
    }

    #[cfg(feature = "jpeg")]
    pub fn export_jpeg(
        &self,
        options: &merman_export::RasterOptions,
        control: OperationControl,
    ) -> Result<RasterOutput, RenderError> {
        self.prepare_jpeg_export(options, control)?.encode()
    }

    #[cfg(any(feature = "png", feature = "jpeg"))]
    fn finish_raster_export(
        &self,
        bytes: Vec<u8>,
        export_report: merman_export::RasterExportReport,
    ) -> Result<RasterOutput, RenderError> {
        let artifact_kind = match export_report.output() {
            merman_export::RasterOutputKind::Png => RenderArtifactKind::Png,
            merman_export::RasterOutputKind::Jpeg => RenderArtifactKind::Jpeg,
            _ => unreachable!("prepared raster report uses a known output kind"),
        };
        let admission = native_raster_admission(
            artifact_kind,
            &bytes,
            export_report,
            &self.evidence,
            &self.portability,
            self.document_digest(),
            self.resource_fingerprint(),
        );
        enforce_portability_requirement(&self.evidence, &admission)?;
        Ok(RasterOutput {
            bytes,
            evidence: Arc::clone(&self.evidence),
            export_report,
            admission,
        })
    }

    #[cfg(feature = "pdf")]
    pub fn prepare_pdf_export(
        &self,
        options: &merman_export::PdfOptions,
        control: OperationControl,
    ) -> Result<PreparedPdfExport<'_>, RenderError> {
        let export = merman_export::prepare_pdf_controlled(&self.svg, options, control)
            .map_err(map_export_error)?;
        let report = export.report();
        Ok(PreparedPdfExport {
            document: self,
            export,
            report,
        })
    }

    #[cfg(feature = "pdf")]
    pub fn export_pdf(
        &self,
        options: &merman_export::PdfOptions,
        control: OperationControl,
    ) -> Result<PdfOutput, RenderError> {
        self.prepare_pdf_export(options, control)?.encode()
    }

    #[cfg(feature = "pdf")]
    fn finish_pdf_export(
        &self,
        bytes: Vec<u8>,
        export_report: merman_export::PdfExportReport,
    ) -> Result<PdfOutput, RenderError> {
        let admission = native_pdf_admission(
            &bytes,
            export_report,
            &self.evidence,
            &self.portability,
            self.document_digest(),
            self.resource_fingerprint(),
        );
        enforce_portability_requirement(&self.evidence, &admission)?;
        Ok(PdfOutput {
            bytes,
            evidence: Arc::clone(&self.evidence),
            export_report,
            admission,
        })
    }
}

#[cfg(feature = "svg")]
impl SvgOutput {
    fn new(svg: String, family: merman_render::family::FamilyRenderReport) -> Self {
        Self {
            svg,
            evidence: RenderEvidence::from_family(family),
            admission: None,
        }
    }

    pub fn svg(&self) -> &str {
        &self.svg
    }

    pub fn evidence(&self) -> &RenderEvidence {
        &self.evidence
    }

    /// Returns target-owned admission when this SVG request required terminal portability proof.
    ///
    /// `BestEffort` keeps the caller-selected parity/output pipeline and therefore does not claim a
    /// standalone terminal receipt. `RequirePortable` either returns an output with a Portable
    /// receipt or fails with [`RenderError::TargetAdmission`] carrying the rejected receipt.
    pub const fn admission(&self) -> Option<&TargetAdmissionReceipt> {
        self.admission.as_ref()
    }

    /// Returns SVG text and evidence, deliberately discarding any target-admission receipt.
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
    #[cfg(feature = "svg")]
    #[error(transparent)]
    TargetAdmission(#[from] TargetAdmissionError),
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

#[cfg(feature = "svg")]
fn render_digest(domain: &[u8], update: impl FnOnce(&mut Sha256)) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_digest_field(&mut hasher, domain);
    update(&mut hasher);
    hasher.finalize().into()
}

#[cfg(feature = "svg")]
fn update_digest_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}

#[cfg(feature = "svg")]
fn update_digest_bool(hasher: &mut Sha256, value: bool) {
    hasher.update([u8::from(value)]);
}

#[cfg(feature = "svg")]
fn update_digest_u64(hasher: &mut Sha256, value: u64) {
    hasher.update(value.to_le_bytes());
}

#[cfg(feature = "svg")]
fn update_digest_usize(hasher: &mut Sha256, value: usize) {
    update_digest_u64(hasher, u64::try_from(value).unwrap_or(u64::MAX));
}

#[cfg(feature = "pdf")]
fn update_digest_f32(hasher: &mut Sha256, value: f32) {
    hasher.update(value.to_bits().to_le_bytes());
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn update_digest_f64(hasher: &mut Sha256, value: f64) {
    hasher.update(value.to_bits().to_le_bytes());
}

#[cfg(feature = "svg")]
fn update_digest_optional_str(hasher: &mut Sha256, value: Option<&str>) {
    update_digest_bool(hasher, value.is_some());
    if let Some(value) = value {
        update_digest_field(hasher, value.as_bytes());
    }
}

#[cfg(feature = "svg")]
fn update_digest_sequence<I, T>(
    hasher: &mut Sha256,
    label: &[u8],
    values: I,
    mut update_item: impl FnMut(&mut Sha256, T),
) where
    I: IntoIterator<Item = T>,
    I::IntoIter: ExactSizeIterator,
{
    let values = values.into_iter();
    update_digest_field(hasher, label);
    update_digest_usize(hasher, values.len());
    for value in values {
        update_item(hasher, value);
    }
}

#[cfg(feature = "svg")]
fn artifact_digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[cfg(feature = "svg")]
fn document_digest(
    public_svg_digest: [u8; 32],
    native_svg_digest: [u8; 32],
    resource_fingerprint: merman_render::svg::SvgResourceFingerprint,
    evidence: &RenderEvidence,
    portability: &DocumentPortabilityReport,
) -> [u8; 32] {
    render_digest(b"merman-rendered-document-experimental-v4", |hasher| {
        update_digest_field(hasher, &public_svg_digest);
        update_digest_field(hasher, &native_svg_digest);
        update_digest_field(hasher, resource_fingerprint.as_bytes());
        update_digest_field(hasher, evidence.family_id().as_str().as_bytes());
        match evidence.theme_recipe_fingerprint() {
            Some(fingerprint) => {
                update_digest_field(hasher, b"theme-recipe");
                update_digest_field(hasher, fingerprint.as_bytes());
            }
            None => update_digest_field(hasher, b"no-theme-recipe"),
        }
        update_operation_context_digest(hasher, evidence);
        update_theme_summary_digest(hasher, evidence.theme_evidence());
        update_portability_requirement_digest(hasher, evidence.portability_requirement());
        update_host_admission_digest(hasher, evidence.session().theme_host_admission_report());
        update_text_measurement_digest(hasher, evidence.measurement());
        update_prepared_text_layout_digest(hasher, evidence.prepared_text_layout());
        update_digest_optional_str(
            hasher,
            evidence.text_layout_failure().map(|failure| failure.id()),
        );
        update_native_filter_digest(hasher, evidence.native_filter_receipt());
        update_digest_bool(hasher, portability.resource_closure_complete());
        update_digest_bool(hasher, portability.prepared_text_evidence_valid());
        update_digest_bool(hasher, portability.is_evidence_valid());
        update_digest_bool(hasher, portability.is_host_dependent());
        update_digest_sequence(
            hasher,
            b"portability-reasons",
            portability.reasons().iter().copied(),
            |hasher, reason| update_digest_field(hasher, reason.id().as_bytes()),
        );
    })
}

#[cfg(feature = "svg")]
fn update_operation_context_digest(hasher: &mut Sha256, evidence: &RenderEvidence) {
    let context = evidence.operation_context();
    hasher.update(context.unix_millis().to_le_bytes());
    update_digest_field(hasher, context.clock_source().id().as_bytes());
    let date = context.today_local();
    hasher.update(date.year().to_le_bytes());
    hasher.update(date.month().to_le_bytes());
    hasher.update(date.day().to_le_bytes());
    update_digest_bool(hasher, context.today_is_fixed());
    update_digest_u64(hasher, context.seed());
    update_digest_field(hasher, context.random_source().id().as_bytes());
    update_digest_bool(hasher, context.timing().is_some());
    let zone = evidence.local_time_zone();
    let zone_source = match zone.source() {
        merman_core::time::LocalTimeZoneSource::FixedOffset => "fixed-offset",
        merman_core::time::LocalTimeZoneSource::System => "system",
    };
    update_digest_field(hasher, zone_source.as_bytes());
    update_digest_field(hasher, zone.identifier().as_bytes());
    update_digest_optional_str(hasher, zone.rules_sha256());
}

#[cfg(feature = "svg")]
fn update_theme_summary_digest(hasher: &mut Sha256, summary: ThemeEvidenceSummary) {
    let status = match summary.status() {
        ThemeEvidenceStatus::NotApplicable => "not-applicable",
        ThemeEvidenceStatus::Verified => "verified",
        ThemeEvidenceStatus::Residual => "residual",
        ThemeEvidenceStatus::Incomplete => "incomplete",
    };
    update_digest_field(hasher, status.as_bytes());
    update_digest_bool(hasher, summary.output_mutated());
}

#[cfg(feature = "svg")]
fn update_portability_requirement_digest(
    hasher: &mut Sha256,
    requirement: merman_render::diagram_theme::ThemePortabilityRequirement,
) {
    let requirement = match requirement {
        merman_render::diagram_theme::ThemePortabilityRequirement::BestEffort => "best-effort",
        merman_render::diagram_theme::ThemePortabilityRequirement::RequirePortable => {
            "require-portable"
        }
        _ => "unknown",
    };
    update_digest_field(hasher, requirement.as_bytes());
}

#[cfg(feature = "svg")]
fn update_host_admission_digest(
    hasher: &mut Sha256,
    report: Option<&merman_render::diagram_theme::ThemeHostAdmissionReport>,
) {
    update_digest_bool(hasher, report.is_some());
    let Some(report) = report else {
        return;
    };
    update_digest_sequence(
        hasher,
        b"host-capabilities",
        report.host_allowed_capabilities(),
        |hasher, capability| update_digest_field(hasher, capability.id().as_bytes()),
    );
    update_digest_sequence(
        hasher,
        b"host-text-capabilities",
        report.host_allowed_text_capabilities(),
        |hasher, capability| update_digest_field(hasher, capability.id().as_bytes()),
    );
    update_digest_sequence(
        hasher,
        b"font-source-priority",
        report.font_source_policy().priority(),
        |hasher, source| update_digest_field(hasher, source.id().as_bytes()),
    );
    update_digest_sequence(
        hasher,
        b"measurement-fallback-priority",
        report.measurement_fallback_policy().priority(),
        |hasher, fallback| update_digest_field(hasher, fallback.id().as_bytes()),
    );
    update_portability_requirement_digest(hasher, report.portability_requirement());
    update_digest_sequence(
        hasher,
        b"trusted-theme-lanes",
        report.trusted_lanes().allowed(),
        |hasher, lane| update_digest_field(hasher, lane.id().as_bytes()),
    );
}

#[cfg(feature = "svg")]
fn update_text_measurement_digest(
    hasher: &mut Sha256,
    report: &merman_render::environment::TextMeasurementReport,
) {
    update_digest_sequence(
        hasher,
        b"text-measurement-provenance",
        report.entries().iter(),
        |hasher, entry| {
            let provenance = entry.provenance();
            let phase = match provenance.phase {
                merman_render::environment::TextMeasurementPhase::Layout => "layout",
                merman_render::environment::TextMeasurementPhase::Wrap => "wrap",
                merman_render::environment::TextMeasurementPhase::SvgBBox => "svg-bbox",
                merman_render::environment::TextMeasurementPhase::ComputedLength => {
                    "computed-length"
                }
            };
            update_digest_field(hasher, phase.as_bytes());
            update_digest_field(hasher, provenance.operation.external_name().as_bytes());
            let source = match provenance.source {
                merman_render::environment::TextMeasurementSource::Profile => "profile",
                merman_render::environment::TextMeasurementSource::Host => "host",
            };
            update_digest_field(hasher, source.as_bytes());
            update_digest_field(hasher, provenance.identity.profile().as_str().as_bytes());
            update_digest_field(hasher, provenance.identity.version().as_bytes());
            update_digest_sequence(
                hasher,
                b"measurement-profile-decorators",
                provenance
                    .identity
                    .decorators()
                    .iter()
                    .map(|decorator| decorator.as_ref()),
                |hasher, decorator| update_digest_field(hasher, decorator.as_bytes()),
            );
            let fallback_reason = provenance.fallback_reason.map(|reason| match reason {
                merman_render::environment::HostFallbackReason::Missing => "missing",
                merman_render::environment::HostFallbackReason::Invalid => "invalid",
                merman_render::environment::HostFallbackReason::Error => "error",
            });
            update_digest_optional_str(hasher, fallback_reason);
            update_digest_u64(hasher, entry.count());
        },
    );
}

#[cfg(feature = "svg")]
fn update_prepared_text_layout_digest(
    hasher: &mut Sha256,
    report: Option<&merman_render::text::PreparedTextLayoutReport>,
) {
    update_digest_bool(hasher, report.is_some());
    let Some(report) = report else {
        return;
    };
    update_digest_field(hasher, report.catalog_fingerprint().as_bytes());
    update_digest_usize(hasher, report.face_count());
    update_digest_sequence(
        hasher,
        b"used-font-sources",
        report.used_font_sources().iter().copied(),
        |hasher, source| update_digest_field(hasher, source.id().as_bytes()),
    );
    update_digest_sequence(
        hasher,
        b"used-measurement-fallbacks",
        report.used_fallbacks().iter().copied(),
        |hasher, fallback| update_digest_field(hasher, fallback.id().as_bytes()),
    );
    update_digest_u64(hasher, report.fallback_count());
    update_digest_u64(hasher, report.failed_attempt_count());
    update_digest_bool(hasher, report.is_host_dependent());
}

#[cfg(feature = "svg")]
fn update_native_filter_digest(
    hasher: &mut Sha256,
    receipt: Option<merman_render::__private::NativeSvgFilterReceipt>,
) {
    update_digest_bool(hasher, receipt.is_some());
    if let Some(receipt) = receipt {
        update_digest_u64(hasher, u64::from(receipt.drop_shadow_count()));
        update_digest_u64(hasher, u64::from(receipt.reference_count()));
        update_digest_field(hasher, receipt.identity_digest());
    }
}

#[cfg(feature = "svg")]
fn update_target_admission_digest(
    hasher: &mut Sha256,
    artifact_kind: RenderArtifactKind,
    status: TargetAdmissionStatus,
    reasons: &[TargetAdmissionReason],
) {
    update_digest_field(hasher, artifact_kind.id().as_bytes());
    update_digest_field(hasher, status.id().as_bytes());
    update_digest_sequence(
        hasher,
        b"target-admission-reasons",
        reasons.iter().copied(),
        |hasher, reason| update_digest_field(hasher, reason.id().as_bytes()),
    );
}

#[cfg(feature = "svg")]
#[allow(clippy::too_many_arguments)]
fn target_admission_receipt_digest(
    artifact_kind: RenderArtifactKind,
    status: TargetAdmissionStatus,
    reasons: &[TargetAdmissionReason],
    font_source: TargetFontSource,
    resource_fingerprint: &[u8; 32],
    font_catalog_fingerprint: &[u8; 32],
    document_digest: [u8; 32],
    target_evidence_digest: [u8; 32],
    artifact_digest: [u8; 32],
) -> [u8; 32] {
    render_digest(
        b"merman-target-admission-receipt-experimental-v1",
        |hasher| {
            update_target_admission_digest(hasher, artifact_kind, status, reasons);
            update_digest_field(hasher, font_source.id().as_bytes());
            update_digest_field(hasher, resource_fingerprint);
            update_digest_field(hasher, font_catalog_fingerprint);
            update_digest_field(hasher, &document_digest);
            update_digest_field(hasher, &target_evidence_digest);
            update_digest_field(hasher, &artifact_digest);
        },
    )
}

#[cfg(feature = "svg")]
fn standalone_target_evidence_digest(
    svg: &merman_render::svg::ResvgCompatibleSvg,
    evidence: &RenderEvidence,
    status: TargetAdmissionStatus,
    reasons: &[TargetAdmissionReason],
    font_source: TargetFontSource,
) -> [u8; 32] {
    render_digest(TARGET_EVIDENCE_DIGEST_DOMAIN, |hasher| {
        update_target_admission_digest(hasher, RenderArtifactKind::Svg, status, reasons);
        update_digest_field(hasher, font_source.id().as_bytes());
        let report = svg.finalization_report();
        let preset = match report.preset() {
            merman_render::svg::SvgPipelinePreset::Parity => "parity",
            merman_render::svg::SvgPipelinePreset::Readable => "readable",
            merman_render::svg::SvgPipelinePreset::ResvgSafe => "resvg-safe",
        };
        update_digest_field(hasher, preset.as_bytes());
        update_digest_sequence(
            hasher,
            b"postprocessors",
            report.postprocessor_names().iter().map(String::as_str),
            |hasher, name| update_digest_field(hasher, name.as_bytes()),
        );
        update_digest_bool(hasher, report.drop_native_duplicate_fallbacks());
        let reference_plan = report.reference_plan();
        update_digest_usize(hasher, reference_plan.expanded_elements());
        update_digest_usize(hasher, reference_plan.max_tree_depth());
        update_digest_sequence(
            hasher,
            b"raw-element-occurrences",
            reference_plan.raw_element_occurrences().iter().copied(),
            update_digest_usize,
        );
        let closure = report.resource_closure();
        update_digest_sequence(
            hasher,
            b"available-fragment-ids",
            closure.available_fragment_ids().iter().map(String::as_str),
            |hasher, id| update_digest_field(hasher, id.as_bytes()),
        );
        update_digest_sequence(
            hasher,
            b"referenced-fragment-ids",
            closure.referenced_fragment_ids().iter().map(String::as_str),
            |hasher, id| update_digest_field(hasher, id.as_bytes()),
        );
        update_digest_sequence(
            hasher,
            b"stylesheet-fragment-ids",
            closure.stylesheet_fragment_ids().iter().map(String::as_str),
            |hasher, id| update_digest_field(hasher, id.as_bytes()),
        );
        update_digest_usize(hasher, closure.inline_data_resource_count());
        update_digest_usize(hasher, report.text_element_count());
        update_digest_bool(
            hasher,
            merman_render::__private::prepared_text_evidence_valid(svg),
        );
        update_digest_bool(
            hasher,
            merman_render::__private::svg_text_fonts_are_self_contained(report),
        );
        update_native_filter_digest(hasher, evidence.native_filter_receipt());
    })
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn update_export_font_digest(hasher: &mut Sha256, fonts: merman_export::ExportFontPlan) {
    update_digest_field(hasher, fonts.catalog_fingerprint().as_bytes());
    update_digest_field(hasher, fonts.source_mode().id().as_bytes());
    update_digest_usize(hasher, fonts.loaded_embedded_face_count());
    update_digest_bool(hasher, fonts.used_embedded_fonts());
    update_digest_bool(hasher, fonts.used_system_fonts());
    update_digest_bool(hasher, fonts.family_fallback_used());
    update_digest_bool(hasher, fonts.glyph_fallback_used());
    update_digest_bool(hasher, fonts.unresolved_font_request());
    update_digest_bool(hasher, fonts.unresolved_glyph_fallback());
    update_digest_usize(hasher, fonts.prepared_label_expected_count());
    update_digest_usize(hasher, fonts.prepared_label_verified_count());
    update_digest_usize(hasher, fonts.prepared_label_mismatch_count());
    update_digest_usize(hasher, fonts.prepared_label_host_dependent_count());
    update_digest_usize(hasher, fonts.prepared_label_terminal_incomplete_count());
    update_digest_usize(hasher, fonts.unclassified_face_count());
    update_digest_usize(hasher, fonts.notdef_glyph_count());
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn update_embedded_image_digest(hasher: &mut Sha256, plan: merman_export::EmbeddedImagePlan) {
    update_digest_usize(hasher, plan.data_resources);
    update_digest_usize(hasher, plan.raster_images);
    update_digest_u64(hasher, plan.largest_data_bytes);
    update_digest_u64(hasher, plan.total_data_bytes);
    update_digest_u64(hasher, plan.largest_raster_pixels);
    update_digest_u64(hasher, plan.total_pixels);
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn update_svg_conversion_digest(hasher: &mut Sha256, plan: merman_export::SvgConversionPlan) {
    update_digest_usize(hasher, plan.tree_nodes);
    update_digest_usize(hasher, plan.max_tree_depth);
    update_digest_usize(hasher, plan.max_isolation_depth);
    update_digest_usize(hasher, plan.filtered_groups);
    update_digest_usize(hasher, plan.filter_primitives);
    update_digest_usize(hasher, plan.subroots);
    update_digest_usize(hasher, plan.nested_svg_images);
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn update_export_color_digest(hasher: &mut Sha256, color: Option<merman_export::ExportRgbaColor>) {
    update_digest_bool(hasher, color.is_some());
    if let Some(color) = color {
        hasher.update([color.red(), color.green(), color.blue(), color.alpha()]);
    }
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn raster_target_evidence_digest(
    artifact_kind: RenderArtifactKind,
    report: merman_export::RasterExportReport,
    expected_native_filter: Option<merman_render::__private::NativeSvgFilterReceipt>,
    status: TargetAdmissionStatus,
    reasons: &[TargetAdmissionReason],
) -> [u8; 32] {
    render_digest(TARGET_EVIDENCE_DIGEST_DOMAIN, |hasher| {
        update_target_admission_digest(hasher, artifact_kind, status, reasons);
        update_digest_field(hasher, report.resource_fingerprint().as_bytes());
        update_native_filter_digest(hasher, expected_native_filter);
        update_native_filter_digest(hasher, report.native_filter_receipt());
        let output = match report.output() {
            merman_export::RasterOutputKind::Png => "png",
            merman_export::RasterOutputKind::Jpeg => "jpeg",
            _ => "unknown",
        };
        update_digest_field(hasher, output.as_bytes());
        let plan = report.raster();
        update_digest_f64(hasher, plan.requested_width_px);
        update_digest_f64(hasher, plan.requested_height_px);
        update_digest_u64(hasher, u64::from(plan.width_px));
        update_digest_u64(hasher, u64::from(plan.height_px));
        update_digest_f64(hasher, plan.requested_scale);
        update_digest_f64(hasher, plan.effective_scale);
        update_digest_bool(hasher, plan.limited);
        update_embedded_image_digest(hasher, report.embedded_images());
        update_svg_conversion_digest(hasher, report.conversion());
        update_export_font_digest(hasher, report.fonts());
        update_export_color_digest(hasher, report.matte());
        update_digest_bool(hasher, report.matte_defaulted());
    })
}

#[cfg(feature = "pdf")]
fn pdf_target_evidence_digest(
    report: merman_export::PdfExportReport,
    expected_native_filter: Option<merman_render::__private::NativeSvgFilterReceipt>,
    status: TargetAdmissionStatus,
    reasons: &[TargetAdmissionReason],
) -> [u8; 32] {
    render_digest(TARGET_EVIDENCE_DIGEST_DOMAIN, |hasher| {
        update_target_admission_digest(hasher, RenderArtifactKind::Pdf, status, reasons);
        update_digest_field(hasher, report.resource_fingerprint().as_bytes());
        update_native_filter_digest(hasher, expected_native_filter);
        update_native_filter_digest(hasher, report.native_filter_receipt());
        update_digest_bool(hasher, report.native_filter_fully_localized());
        let page = report.page();
        update_digest_f32(hasher, page.page_width_pt());
        update_digest_f32(hasher, page.page_height_pt());
        update_digest_f32(hasher, page.drawing_width_pt());
        update_digest_f32(hasher, page.drawing_height_pt());
        update_digest_f32(hasher, page.offset_x_pt());
        update_digest_f32(hasher, page.offset_y_pt());
        let filters = report.filters();
        update_digest_usize(hasher, filters.filtered_groups);
        update_digest_f32(hasher, filters.requested_scale);
        update_digest_f32(hasher, filters.effective_scale);
        update_digest_u64(hasher, filters.requested_image_pixels);
        update_digest_u64(hasher, filters.effective_image_pixels);
        update_digest_bool(hasher, filters.limited);
        update_embedded_image_digest(hasher, report.embedded_images());
        update_svg_conversion_digest(hasher, report.conversion());
        update_export_font_digest(hasher, report.fonts());
        update_export_color_digest(hasher, report.page_paint());
    })
}

#[cfg(feature = "svg")]
fn classify_target_admission(rejected: bool, host_dependent: bool) -> TargetAdmissionStatus {
    if rejected {
        TargetAdmissionStatus::Rejected
    } else if host_dependent {
        TargetAdmissionStatus::HostDependent
    } else {
        TargetAdmissionStatus::Portable
    }
}

#[cfg(feature = "svg")]
fn classify_common_document_evidence(
    evidence: &RenderEvidence,
    reasons: &mut Vec<TargetAdmissionReason>,
) -> (bool, bool) {
    let mut rejected = false;
    let mut host_dependent = false;
    if !evidence.theme_evidence().is_satisfied() {
        rejected = true;
        reasons.push(TargetAdmissionReason::ThemeEvidenceIncomplete);
    }
    if evidence.text_layout_failure().is_some() {
        rejected = true;
        reasons.push(TargetAdmissionReason::PreparedTextLayoutFailed);
    }
    let prepared_text_is_host_dependent = evidence
        .prepared_text_layout()
        .is_some_and(merman_render::text::PreparedTextLayoutReport::is_host_dependent);
    let actual_measurement_is_host_dependent =
        evidence.measurement().entries().iter().any(|entry| {
            entry.count() > 0
                && entry.provenance().source
                    == merman_render::environment::TextMeasurementSource::Host
        });
    if prepared_text_is_host_dependent || actual_measurement_is_host_dependent {
        host_dependent = true;
        reasons.push(TargetAdmissionReason::HostDependentTextLayout);
    }
    (rejected, host_dependent)
}

#[cfg(feature = "svg")]
fn document_portability_report(
    svg: &merman_render::svg::ResvgCompatibleSvg,
    evidence: &RenderEvidence,
) -> DocumentPortabilityReport {
    let mut reasons = Vec::new();
    let (mut rejected, host_dependent) = classify_common_document_evidence(evidence, &mut reasons);
    let prepared_text_evidence_valid = merman_render::__private::prepared_text_evidence_valid(svg);
    if !prepared_text_evidence_valid {
        rejected = true;
        reasons.push(TargetAdmissionReason::PreparedTextEvidenceInvalid);
    }

    DocumentPortabilityReport {
        family_id: evidence.family_id(),
        theme_evidence: evidence.theme_evidence(),
        resource_fingerprint: svg.resource_fingerprint(),
        font_catalog_fingerprint: evidence.font_catalog_fingerprint(),
        prepared_text_evidence_valid,
        rejected,
        host_dependent,
        reasons: reasons.into_boxed_slice(),
    }
}

#[cfg(feature = "svg")]
fn inherit_document_portability(
    report: &DocumentPortabilityReport,
    reasons: &mut Vec<TargetAdmissionReason>,
) -> (bool, bool) {
    reasons.extend(report.reasons().iter().copied());
    (!report.is_evidence_valid(), report.is_host_dependent())
}

#[cfg(feature = "svg")]
fn prepared_text_font_source(evidence: &RenderEvidence) -> TargetFontSource {
    let Some(report) = evidence.prepared_text_layout() else {
        return TargetFontSource::None;
    };
    target_font_source(
        report.uses_font_source(merman_render::diagram_theme::FontSource::Embedded),
        report.uses_font_source(merman_render::diagram_theme::FontSource::System),
    )
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn export_font_source(fonts: merman_export::ExportFontPlan) -> TargetFontSource {
    target_font_source(fonts.used_embedded_fonts(), fonts.used_system_fonts())
}

#[cfg(feature = "svg")]
const fn target_font_source(embedded: bool, system: bool) -> TargetFontSource {
    match (embedded, system) {
        (false, false) => TargetFontSource::None,
        (true, false) => TargetFontSource::Embedded,
        (false, true) => TargetFontSource::System,
        (true, true) => TargetFontSource::Mixed,
    }
}

#[cfg(feature = "svg")]
fn standalone_svg_admission(
    svg: &merman_render::svg::ResvgCompatibleSvg,
    evidence: &RenderEvidence,
    document: &DocumentPortabilityReport,
    document_digest: [u8; 32],
    artifact_digest: [u8; 32],
) -> TargetAdmissionReceipt {
    let mut reasons = Vec::new();
    let (rejected, mut host_dependent) = inherit_document_portability(document, &mut reasons);
    if !merman_render::__private::svg_text_fonts_are_self_contained(svg.finalization_report()) {
        host_dependent = true;
        reasons.push(TargetAdmissionReason::SvgFontsNotSelfContained);
    }

    let status = classify_target_admission(rejected, host_dependent);
    let font_source = prepared_text_font_source(evidence);
    let target_evidence_digest =
        standalone_target_evidence_digest(svg, evidence, status, &reasons, font_source);
    TargetAdmissionReceipt::seal(
        RenderArtifactKind::Svg,
        status,
        reasons,
        font_source,
        svg.resource_fingerprint(),
        evidence.font_catalog_fingerprint(),
        document_digest,
        target_evidence_digest,
        artifact_digest,
    )
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
fn classify_native_export_fonts(
    fonts: merman_export::ExportFontPlan,
    reasons: &mut Vec<TargetAdmissionReason>,
) -> (bool, bool) {
    let mut rejected = false;
    let mut host_dependent = false;
    if !fonts.prepared_text_evidence_matches() {
        rejected = true;
        reasons.push(TargetAdmissionReason::PreparedTextEvidenceMismatch);
    }
    if !fonts.prepared_text_terminal_proof_complete() {
        rejected = true;
        reasons.push(TargetAdmissionReason::PreparedTextTerminalProofIncomplete);
    }
    if fonts.unresolved_font_request()
        || fonts.unresolved_glyph_fallback()
        || fonts.unclassified_face_count() != 0
        || fonts.notdef_glyph_count() != 0
    {
        rejected = true;
        reasons.push(TargetAdmissionReason::FontResolutionIncomplete);
    }
    if fonts.is_host_dependent() {
        host_dependent = true;
        reasons.push(TargetAdmissionReason::SystemOrHostFontDependency);
    }
    (rejected, host_dependent)
}

#[cfg(any(feature = "png", feature = "jpeg"))]
fn native_raster_admission(
    artifact_kind: RenderArtifactKind,
    bytes: &[u8],
    report: merman_export::RasterExportReport,
    evidence: &RenderEvidence,
    document: &DocumentPortabilityReport,
    document_digest: [u8; 32],
    expected_resource_fingerprint: merman_render::svg::SvgResourceFingerprint,
) -> TargetAdmissionReceipt {
    let mut reasons = Vec::new();
    let (mut rejected, mut host_dependent) = inherit_document_portability(document, &mut reasons);
    let (fonts_rejected, fonts_host_dependent) =
        classify_native_export_fonts(report.fonts(), &mut reasons);
    rejected |= fonts_rejected;
    host_dependent |= fonts_host_dependent;
    if report.resource_fingerprint() != expected_resource_fingerprint {
        rejected = true;
        reasons.push(TargetAdmissionReason::ResourceFingerprintMismatch);
    }
    if report.fonts().catalog_fingerprint() != evidence.font_catalog_fingerprint() {
        rejected = true;
        reasons.push(TargetAdmissionReason::FontCatalogFingerprintMismatch);
    }
    let report_matches_target = matches!(
        (artifact_kind, report.output()),
        (
            RenderArtifactKind::Png,
            merman_export::RasterOutputKind::Png
        ) | (
            RenderArtifactKind::Jpeg,
            merman_export::RasterOutputKind::Jpeg
        )
    );
    if !report_matches_target {
        rejected = true;
        reasons.push(TargetAdmissionReason::TargetKindMismatch);
    }
    if report.native_filter_receipt() != evidence.native_filter_receipt() {
        rejected = true;
        reasons.push(TargetAdmissionReason::NativeFilterReceiptMismatch);
    }
    let status = classify_target_admission(rejected, host_dependent);
    let target_evidence_digest = raster_target_evidence_digest(
        artifact_kind,
        report,
        evidence.native_filter_receipt(),
        status,
        &reasons,
    );
    TargetAdmissionReceipt::seal(
        artifact_kind,
        status,
        reasons,
        export_font_source(report.fonts()),
        report.resource_fingerprint(),
        report.fonts().catalog_fingerprint(),
        document_digest,
        target_evidence_digest,
        artifact_digest(bytes),
    )
}

#[cfg(feature = "pdf")]
fn native_pdf_admission(
    bytes: &[u8],
    report: merman_export::PdfExportReport,
    evidence: &RenderEvidence,
    document: &DocumentPortabilityReport,
    document_digest: [u8; 32],
    expected_resource_fingerprint: merman_render::svg::SvgResourceFingerprint,
) -> TargetAdmissionReceipt {
    let mut reasons = Vec::new();
    let (mut rejected, mut host_dependent) = inherit_document_portability(document, &mut reasons);
    let (fonts_rejected, fonts_host_dependent) =
        classify_native_export_fonts(report.fonts(), &mut reasons);
    rejected |= fonts_rejected;
    host_dependent |= fonts_host_dependent;
    if report.resource_fingerprint() != expected_resource_fingerprint {
        rejected = true;
        reasons.push(TargetAdmissionReason::ResourceFingerprintMismatch);
    }
    if report.fonts().catalog_fingerprint() != evidence.font_catalog_fingerprint() {
        rejected = true;
        reasons.push(TargetAdmissionReason::FontCatalogFingerprintMismatch);
    }
    let expected_native_filter = evidence.native_filter_receipt();
    if report.native_filter_receipt() != expected_native_filter {
        rejected = true;
        reasons.push(TargetAdmissionReason::NativeFilterReceiptMismatch);
    }
    if expected_native_filter.is_some() && !report.native_filter_fully_localized() {
        rejected = true;
        reasons.push(TargetAdmissionReason::PdfNativeFilterNotLocalized);
    }
    let status = classify_target_admission(rejected, host_dependent);
    let target_evidence_digest =
        pdf_target_evidence_digest(report, expected_native_filter, status, &reasons);
    TargetAdmissionReceipt::seal(
        RenderArtifactKind::Pdf,
        status,
        reasons,
        export_font_source(report.fonts()),
        report.resource_fingerprint(),
        report.fonts().catalog_fingerprint(),
        document_digest,
        target_evidence_digest,
        artifact_digest(bytes),
    )
}

#[cfg(feature = "svg")]
fn enforce_portability_requirement(
    evidence: &RenderEvidence,
    receipt: &TargetAdmissionReceipt,
) -> Result<(), RenderError> {
    if evidence.portability_requirement()
        == merman_render::diagram_theme::ThemePortabilityRequirement::RequirePortable
        && !receipt.status().is_portable()
    {
        return Err(TargetAdmissionError::new(receipt.clone()).into());
    }
    Ok(())
}

/// Target request for the canonical facade.
///
/// The semantic target is available in every feature configuration. SVG, ASCII, and terminal
/// export variants are feature-gated leaves of the same dispatch seam.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum RenderTarget {
    Semantic,
    #[cfg(feature = "svg")]
    Svg(SvgRequest),
    #[cfg(feature = "svg")]
    Document(SvgRequest),
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
    bytes: Vec<u8>,
    evidence: Arc<RenderEvidence>,
    export_report: merman_export::RasterExportReport,
    admission: TargetAdmissionReceipt,
}

#[cfg(any(feature = "png", feature = "jpeg"))]
impl RasterOutput {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn plan(&self) -> merman_export::RasterPlan {
        self.export_report.raster()
    }

    pub fn evidence(&self) -> &RenderEvidence {
        &self.evidence
    }

    pub const fn export_report(&self) -> merman_export::RasterExportReport {
        self.export_report
    }

    pub const fn admission(&self) -> &TargetAdmissionReceipt {
        &self.admission
    }

    /// Returns the encoded bytes and deliberately discards render and target-admission evidence.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(feature = "pdf")]
#[derive(Debug, Clone)]
pub struct PdfOutput {
    bytes: Vec<u8>,
    evidence: Arc<RenderEvidence>,
    export_report: merman_export::PdfExportReport,
    admission: TargetAdmissionReceipt,
}

#[cfg(feature = "pdf")]
impl PdfOutput {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn plan(&self) -> merman_export::PdfFilterImagePlan {
        self.export_report.filters()
    }

    pub fn evidence(&self) -> &RenderEvidence {
        &self.evidence
    }

    pub const fn export_report(&self) -> merman_export::PdfExportReport {
        self.export_report
    }

    pub const fn admission(&self) -> &TargetAdmissionReceipt {
        &self.admission
    }

    /// Returns the encoded bytes and deliberately discards render and target-admission evidence.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
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

    /// Requests one completed graphical document that can project every enabled native target.
    #[cfg(feature = "svg")]
    pub fn document(source: &'a str, control: OperationControl, request: SvgRequest) -> Self {
        Self::new(source, RenderTarget::Document(request), control)
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
#[non_exhaustive]
pub enum RenderOutput {
    Semantic(Option<SemanticArtifact>),
    #[cfg(feature = "svg")]
    Svg(Option<SvgOutput>),
    #[cfg(feature = "svg")]
    Document(Option<RenderedDocument>),
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
            RenderTarget::Document(request) => {
                render_document_target(self, request).map(RenderOutput::Document)
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
            RenderTarget::Document(_) => Self::Document(None),
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
    if request.environment.portability_requirement()
        == merman_render::diagram_theme::ThemePortabilityRequirement::RequirePortable
    {
        let Some((svg, family, _operation)) = prepare_resvg_target(semantic, &request)? else {
            unreachable!("semantic artifact always produces a finalized SVG or an error")
        };
        return RenderedDocument::new(svg, family)
            .into_admitted_svg_output()
            .map(Some);
    }

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
fn render_document_target(
    semantic: SemanticArtifact,
    request: SvgRequest,
) -> Result<Option<RenderedDocument>, RenderError> {
    let Some((svg, family, _operation)) = prepare_resvg_target(semantic, &request)? else {
        unreachable!("semantic artifact always produces a finalized SVG or an error")
    };
    Ok(Some(RenderedDocument::new(svg, family)))
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
    RenderedDocument::new(svg, family)
        .export_png(&request.options, operation.control)
        .map(Some)
}

#[cfg(feature = "jpeg")]
fn render_jpeg_target(
    semantic: SemanticArtifact,
    request: JpegRequest,
) -> Result<Option<RasterOutput>, RenderError> {
    let Some((svg, family, operation)) = prepare_resvg_target(semantic, &request.svg)? else {
        unreachable!("semantic artifact always produces a sealed SVG or an error")
    };
    RenderedDocument::new(svg, family)
        .export_jpeg(&request.options, operation.control)
        .map(Some)
}

#[cfg(feature = "pdf")]
fn render_pdf_target(
    semantic: SemanticArtifact,
    request: PdfRequest,
) -> Result<Option<PdfOutput>, RenderError> {
    let Some((svg, family, operation)) = prepare_resvg_target(semantic, &request.svg)? else {
        unreachable!("semantic artifact always produces a sealed SVG or an error")
    };
    RenderedDocument::new(svg, family)
        .export_pdf(&request.options, operation.control)
        .map(Some)
}

#[cfg(all(test, feature = "svg"))]
mod tests {
    use super::{
        RenderArtifactKind, RenderError, RenderOutput, RenderRequest, Renderer, ResourceLimitCause,
        TargetAdmissionReason, TargetAdmissionStatus, TargetFontSource,
        ThemeEvidenceScopeProjection, ThemeEvidenceStatus, ThemeEvidenceSummary, render_digest,
        standalone_target_evidence_digest, summarize_theme_evidence,
        target_admission_receipt_digest, update_digest_field, update_digest_sequence,
        update_native_filter_digest,
    };
    use merman_core::OperationControl;
    use merman_render::__private::{NativeSvgFilterReceipt, NativeSvgHardShadow};

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
    fn target_admission_reason_exposes_the_complete_alpha_catalog() {
        assert_eq!(
            TargetAdmissionReason::ALL,
            &[
                TargetAdmissionReason::ThemeEvidenceIncomplete,
                TargetAdmissionReason::PreparedTextLayoutFailed,
                TargetAdmissionReason::HostDependentTextLayout,
                TargetAdmissionReason::PreparedTextEvidenceInvalid,
                TargetAdmissionReason::SvgFontsNotSelfContained,
                TargetAdmissionReason::PreparedTextEvidenceMismatch,
                TargetAdmissionReason::PreparedTextTerminalProofIncomplete,
                TargetAdmissionReason::FontResolutionIncomplete,
                TargetAdmissionReason::SystemOrHostFontDependency,
                TargetAdmissionReason::ResourceFingerprintMismatch,
                TargetAdmissionReason::FontCatalogFingerprintMismatch,
                TargetAdmissionReason::TargetKindMismatch,
                TargetAdmissionReason::NativeFilterReceiptMismatch,
                TargetAdmissionReason::PdfNativeFilterNotLocalized,
            ]
        );
    }

    #[test]
    fn standalone_target_evidence_digest_binds_font_source() {
        let output = Renderer::new()
            .render(RenderRequest::document(
                "info",
                OperationControl::new(),
                super::SvgRequest::default(),
            ))
            .expect("document should render");
        let RenderOutput::Document(Some(document)) = output else {
            panic!("expected a rendered document");
        };
        let receipt = document.standalone_svg_admission();
        let digest = |font_source| {
            standalone_target_evidence_digest(
                document.sealed_svg(),
                document.evidence(),
                receipt.status(),
                receipt.reasons(),
                font_source,
            )
        };

        assert_ne!(
            digest(TargetFontSource::None),
            digest(TargetFontSource::Embedded)
        );
        assert_eq!(
            receipt.receipt_digest(),
            target_admission_receipt_digest(
                receipt.artifact_kind(),
                receipt.status(),
                receipt.reasons(),
                receipt.font_source(),
                receipt.resource_fingerprint().as_bytes(),
                receipt.font_catalog_fingerprint().as_bytes(),
                receipt.document_digest(),
                receipt.target_evidence_digest(),
                receipt.artifact_digest(),
            )
        );
    }

    #[test]
    fn native_filter_digest_distinguishes_equal_counts_with_different_blur() {
        fn receipt(std_deviation: f32) -> NativeSvgFilterReceipt {
            NativeSvgFilterReceipt::from_drop_shadows([NativeSvgHardShadow::new(
                "state-theme-effect-shadow",
                [-0.2, -0.2, 1.4, 1.4],
                [5.0, 5.0],
                [std_deviation, std_deviation],
                "#111827",
                1,
            )
            .expect("valid drop shadow")])
            .expect("valid native filter receipt")
        }

        fn digest(receipt: NativeSvgFilterReceipt) -> [u8; 32] {
            render_digest(b"native-filter-digest-test", |hasher| {
                update_native_filter_digest(hasher, Some(receipt));
            })
        }

        let sharp = receipt(0.0);
        let blurred = receipt(8.0);

        assert_eq!(sharp.drop_shadow_count(), blurred.drop_shadow_count());
        assert_eq!(sharp.reference_count(), blurred.reference_count());
        assert_ne!(digest(sharp), digest(blurred));
    }

    #[test]
    fn target_admission_receipt_digest_binds_every_canonical_field() {
        #[derive(Clone)]
        struct DigestInputs {
            artifact_kind: RenderArtifactKind,
            status: TargetAdmissionStatus,
            reasons: Vec<TargetAdmissionReason>,
            font_source: TargetFontSource,
            resource_fingerprint: [u8; 32],
            font_catalog_fingerprint: [u8; 32],
            document_digest: [u8; 32],
            target_evidence_digest: [u8; 32],
            artifact_digest: [u8; 32],
        }

        impl DigestInputs {
            fn digest(&self) -> [u8; 32] {
                target_admission_receipt_digest(
                    self.artifact_kind,
                    self.status,
                    &self.reasons,
                    self.font_source,
                    &self.resource_fingerprint,
                    &self.font_catalog_fingerprint,
                    self.document_digest,
                    self.target_evidence_digest,
                    self.artifact_digest,
                )
            }
        }

        fn assert_field_is_bound(
            baseline: &DigestInputs,
            baseline_digest: [u8; 32],
            field: &str,
            mutate: impl FnOnce(&mut DigestInputs),
        ) {
            let mut changed = baseline.clone();
            mutate(&mut changed);
            assert_ne!(
                baseline_digest,
                changed.digest(),
                "receipt digest must bind {field}"
            );
        }

        let baseline = DigestInputs {
            artifact_kind: RenderArtifactKind::Svg,
            status: TargetAdmissionStatus::Portable,
            reasons: vec![
                TargetAdmissionReason::ThemeEvidenceIncomplete,
                TargetAdmissionReason::PreparedTextLayoutFailed,
            ],
            font_source: TargetFontSource::None,
            resource_fingerprint: [1; 32],
            font_catalog_fingerprint: [2; 32],
            document_digest: [3; 32],
            target_evidence_digest: [4; 32],
            artifact_digest: [5; 32],
        };
        let baseline_digest = baseline.digest();

        assert_field_is_bound(&baseline, baseline_digest, "artifact kind", |inputs| {
            inputs.artifact_kind = RenderArtifactKind::Png;
        });
        assert_field_is_bound(&baseline, baseline_digest, "status", |inputs| {
            inputs.status = TargetAdmissionStatus::HostDependent;
        });
        assert_field_is_bound(&baseline, baseline_digest, "ordered reasons", |inputs| {
            inputs.reasons.reverse();
        });
        assert_field_is_bound(&baseline, baseline_digest, "font source", |inputs| {
            inputs.font_source = TargetFontSource::Embedded;
        });
        assert_field_is_bound(
            &baseline,
            baseline_digest,
            "resource fingerprint",
            |inputs| {
                inputs.resource_fingerprint[0] ^= 1;
            },
        );
        assert_field_is_bound(
            &baseline,
            baseline_digest,
            "font catalog fingerprint",
            |inputs| {
                inputs.font_catalog_fingerprint[0] ^= 1;
            },
        );
        assert_field_is_bound(&baseline, baseline_digest, "document digest", |inputs| {
            inputs.document_digest[0] ^= 1;
        });
        assert_field_is_bound(
            &baseline,
            baseline_digest,
            "target evidence digest",
            |inputs| {
                inputs.target_evidence_digest[0] ^= 1;
            },
        );
        assert_field_is_bound(&baseline, baseline_digest, "artifact digest", |inputs| {
            inputs.artifact_digest[0] ^= 1;
        });
    }

    #[test]
    fn canonical_digest_sequences_bind_semantic_field_boundaries() {
        fn digest(available: &[&str], referenced: &[&str], stylesheet: &[&str]) -> [u8; 32] {
            render_digest(b"digest-sequence-boundary-test", |hasher| {
                update_digest_sequence(
                    hasher,
                    b"available",
                    available.iter().copied(),
                    |hasher, value| update_digest_field(hasher, value.as_bytes()),
                );
                update_digest_sequence(
                    hasher,
                    b"referenced",
                    referenced.iter().copied(),
                    |hasher, value| update_digest_field(hasher, value.as_bytes()),
                );
                update_digest_sequence(
                    hasher,
                    b"stylesheet",
                    stylesheet.iter().copied(),
                    |hasher, value| update_digest_field(hasher, value.as_bytes()),
                );
            })
        }

        assert_ne!(digest(&["a"], &["b"], &[]), digest(&[], &["a"], &["b"]));
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

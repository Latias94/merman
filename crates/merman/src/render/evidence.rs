/// Coarse terminal state for one bounded theme-evidence scope.
///
/// The renderer keeps mechanism keys, selectors, element receipts, and residual ledgers private.
/// Callers only need to know whether the scope was fully accounted for and how much bounded
/// evidence remained outside the portable path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ThemeEvidenceStatus {
    NotApplicable,
    Verified,
    Residual,
    Incomplete,
}

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThemeEvidenceSummary {
    status: ThemeEvidenceStatus,
    output_mutated: bool,
}

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
#[cfg(feature = "internal-theme-acceptance")]
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

#[cfg(feature = "internal-theme-acceptance")]
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderEvidence {
    session: merman_render::environment::RenderSessionReport,
    family_id: merman_core::DiagramFamilyId,
    theme_evidence: ThemeEvidenceSummary,
    root_applied_capabilities: Box<[merman_render::diagram_theme::ThemeCapability]>,
    native_filter_receipt: Option<merman_render::__private::NativeSvgFilterReceipt>,
    #[cfg(feature = "internal-theme-acceptance")]
    theme_acceptance: ThemeAcceptanceEvidenceSnapshot,
}

impl RenderEvidence {
    pub(super) fn from_family(family: merman_render::family::FamilyRenderReport) -> Self {
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
        let root_applied_capabilities =
            merman_render::__private::root_applied_capabilities(&family);
        let native_filter_receipt = merman_render::__private::family_native_filter_receipt(&family);
        Self {
            session,
            family_id,
            theme_evidence,
            root_applied_capabilities,
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

    pub(super) const fn session(&self) -> &merman_render::environment::RenderSessionReport {
        &self.session
    }

    pub(super) fn portability_requirement(
        &self,
    ) -> merman_render::diagram_theme::ThemePortabilityRequirement {
        self.session().portability_requirement()
    }

    pub(super) fn prepared_text_layout(
        &self,
    ) -> Option<&merman_render::text::PreparedTextLayoutReport> {
        self.session().prepared_text_layout()
    }

    pub(super) fn text_layout_failure(&self) -> Option<merman_render::text::TextLayoutFailure> {
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

    pub(crate) fn root_applied_capabilities(
        &self,
    ) -> &[merman_render::diagram_theme::ThemeCapability] {
        &self.root_applied_capabilities
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

#[cfg(test)]
mod tests {
    use super::{
        ThemeEvidenceScopeProjection, ThemeEvidenceStatus, ThemeEvidenceSummary,
        summarize_theme_evidence,
    };

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

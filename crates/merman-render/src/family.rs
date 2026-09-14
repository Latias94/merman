mod capability;
mod direct_static_paint;
mod evidence_support;
mod inherited_font_stack;
mod parse_defaults;
mod preparation;

pub use capability::{RenderCapabilityPlan, plan_render};
pub(crate) use direct_static_paint::{
    DirectStaticPaint, DirectStaticSelectorDomain, resolve_direct_static_fill,
    resolve_direct_static_stroke,
};
pub(crate) use evidence_support::{
    DirectPaintExpectation, DirectPaintTerminalLedger, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
pub(crate) use inherited_font_stack::{
    InheritedFontStackOutcome, InheritedFontStackPlan, InheritedTextRunFacts, InheritedTextRunSpec,
    InheritedTextViewportFacts,
};
pub(crate) use parse_defaults::{FamilyPaintDefaultPaths, bind_theme_parse_defaults};

use crate::diagram_theme::{
    FamilyThemeMechanismKey, ResolvedDiagramTheme, RootThemePlan, RootThemeReport,
    SourceStyleChannel, SourceStyleOrigin, SourceStyleResidual, SourceStyleResidualReason,
    ThemeCapability, ThemePortabilityRequirement, ThemeRecipeFingerprint,
};
use crate::environment::{RenderSession, RenderSessionReport};
use crate::math::PreparedMathEvidenceLease;
use crate::model::*;
use crate::resources::ResourceLimitPhase;
use crate::svg::{
    FlowchartEdgeTraceCollector, ResvgCompatibleSvg, StandaloneSvgArtifact, SvgDebugOptions,
    SvgPipeline, SvgPipelinePreset, SvgPostprocessExecution, SvgPostprocessMetadata,
    SvgRenderOptions,
};
use crate::text::PreparedTextEvidenceLease;
use crate::wardley::WardleyDiagramLayout;
use crate::{Error, LayoutExecution, LayoutOptions, RenderCapability, Result};
use merman_core::OperationPhase;
use merman_core::diagrams;
use merman_core::models::class_diagram::ClassDiagram;
use merman_core::{
    BuiltinRenderSemantic, DiagramFamilyId, ParseMetadata, ParsedDiagramRender, RenderSemanticModel,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

/// Evaluation state of a family-local style plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum FamilyStyleEvaluation {
    /// The selected theme has no structured styles that apply to this family.
    NotApplicable,
    /// The family adapter evaluated every applicable structured style input.
    Evaluated,
    /// Structured style inputs apply, but this family has no complete adapter yet.
    Unadapted,
}

/// Verification state derived from family evaluation and its retained residuals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum FamilyStyleVerification {
    NotApplicable,
    Verified,
    Unverified,
    Unadapted,
    Incomplete,
}

/// Source channel that produced an unverified family style residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum FamilyStyleChannel {
    Shape,
    Label,
    Stylesheet,
}

impl FamilyStyleChannel {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Shape => "shape",
            Self::Label => "label",
            Self::Stylesheet => "stylesheet",
        }
    }
}

impl std::fmt::Display for FamilyStyleChannel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.id())
    }
}

/// Source origin that produced an unverified family style residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum FamilyStyleOrigin {
    AssignedClass,
    InlineStyle,
    LabelStyle,
    GeneratedClassCss,
}

impl FamilyStyleOrigin {
    pub const fn id(self) -> &'static str {
        match self {
            Self::AssignedClass => "assigned-class",
            Self::InlineStyle => "inline-style",
            Self::LabelStyle => "label-style",
            Self::GeneratedClassCss => "generated-class-css",
        }
    }
}

impl std::fmt::Display for FamilyStyleOrigin {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.id())
    }
}

/// Reason a family adapter could not prove one typed semantic mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum FamilyThemeResidualReason {
    UnsupportedPaint,
    UnsupportedTypography,
    UnsupportedGeometry,
    UnsupportedEffect,
    UnsupportedOrdinalPalette,
    OutputVisibilityFiltered,
    OutputMutation,
}

impl FamilyThemeResidualReason {
    pub const fn id(self) -> &'static str {
        match self {
            Self::UnsupportedPaint => "unsupported-paint",
            Self::UnsupportedTypography => "unsupported-typography",
            Self::UnsupportedGeometry => "unsupported-geometry",
            Self::UnsupportedEffect => "unsupported-effect",
            Self::UnsupportedOrdinalPalette => "unsupported-ordinal-palette",
            Self::OutputVisibilityFiltered => "output-visibility-filtered",
            Self::OutputMutation => "output-mutation",
        }
    }
}

impl std::fmt::Display for FamilyThemeResidualReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.id())
    }
}

/// Frozen evidence for one typed semantic mechanism not proven by the selected family adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FamilyThemeResidual {
    key: FamilyThemeMechanismKey,
    reason: FamilyThemeResidualReason,
}

impl FamilyThemeResidual {
    pub(crate) const fn key(&self) -> &FamilyThemeMechanismKey {
        &self.key
    }

    pub(crate) const fn reason(&self) -> FamilyThemeResidualReason {
        self.reason
    }
}

impl std::fmt::Display for FamilyThemeResidual {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} ({})", self.key, self.reason)
    }
}

/// Reason a source style could not be verified by the family adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum FamilyStyleResidualReason {
    InvalidDeclaration,
    UnsupportedProperty,
    InvalidValue,
    UnsupportedSurface,
}

impl FamilyStyleResidualReason {
    pub const fn id(self) -> &'static str {
        match self {
            Self::InvalidDeclaration => "invalid-declaration",
            Self::UnsupportedProperty => "unsupported-property",
            Self::InvalidValue => "invalid-value",
            Self::UnsupportedSurface => "unsupported-surface",
        }
    }
}

impl std::fmt::Display for FamilyStyleResidualReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.id())
    }
}

/// Frozen evidence for one source style the family adapter could not verify as portable.
///
/// Fields are intentionally private: callers can inspect the stable evidence but cannot forge a
/// renderer report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FamilyStyleResidual {
    raw: std::sync::Arc<str>,
    property: Option<std::sync::Arc<str>>,
    owner_id: std::sync::Arc<str>,
    class_id: Option<std::sync::Arc<str>>,
    origin: FamilyStyleOrigin,
    channel: FamilyStyleChannel,
    assignment_ordinal: Option<usize>,
    declaration_ordinal: usize,
    reason: FamilyStyleResidualReason,
}

impl FamilyStyleResidual {
    fn freeze(residual: &SourceStyleResidual) -> Self {
        let provenance = residual.provenance();
        Self {
            raw: residual.raw_arc(),
            property: residual.property_arc(),
            owner_id: provenance.owner_id_arc(),
            class_id: provenance.class_id_arc(),
            origin: provenance.origin().into(),
            channel: provenance.channel().into(),
            assignment_ordinal: provenance.assignment_ordinal(),
            declaration_ordinal: provenance.declaration_ordinal(),
            reason: residual.reason().into(),
        }
    }

    #[cfg(test)]
    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn property(&self) -> Option<&str> {
        self.property.as_deref()
    }

    #[cfg(test)]
    pub fn owner_id(&self) -> &str {
        &self.owner_id
    }

    #[cfg(test)]
    pub fn class_id(&self) -> Option<&str> {
        self.class_id.as_deref()
    }

    #[cfg(test)]
    pub const fn origin(&self) -> FamilyStyleOrigin {
        self.origin
    }

    #[cfg(test)]
    pub const fn channel(&self) -> FamilyStyleChannel {
        self.channel
    }

    #[cfg(test)]
    pub const fn assignment_ordinal(&self) -> Option<usize> {
        self.assignment_ordinal
    }

    #[cfg(test)]
    pub const fn declaration_ordinal(&self) -> usize {
        self.declaration_ordinal
    }

    #[cfg(test)]
    pub const fn reason(&self) -> FamilyStyleResidualReason {
        self.reason
    }
}

impl std::fmt::Display for FamilyStyleResidual {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} {} style on `{}`",
            self.reason, self.channel, self.owner_id
        )?;
        if let Some(property) = self.property() {
            write!(formatter, " for property `{property}`")?;
        }
        Ok(())
    }
}

impl From<SourceStyleChannel> for FamilyStyleChannel {
    fn from(channel: SourceStyleChannel) -> Self {
        match channel {
            SourceStyleChannel::Shape => Self::Shape,
            SourceStyleChannel::Label => Self::Label,
            SourceStyleChannel::Stylesheet => Self::Stylesheet,
        }
    }
}

impl From<SourceStyleOrigin> for FamilyStyleOrigin {
    fn from(origin: SourceStyleOrigin) -> Self {
        match origin {
            SourceStyleOrigin::AssignedClass => Self::AssignedClass,
            SourceStyleOrigin::InlineStyle => Self::InlineStyle,
            SourceStyleOrigin::LabelStyle => Self::LabelStyle,
            SourceStyleOrigin::GeneratedClassCss => Self::GeneratedClassCss,
        }
    }
}

impl From<SourceStyleResidualReason> for FamilyStyleResidualReason {
    fn from(reason: SourceStyleResidualReason) -> Self {
        match reason {
            SourceStyleResidualReason::InvalidDeclaration => Self::InvalidDeclaration,
            SourceStyleResidualReason::UnsupportedProperty => Self::UnsupportedProperty,
            SourceStyleResidualReason::InvalidValue => Self::InvalidValue,
            SourceStyleResidualReason::UnsupportedSurface => Self::UnsupportedSurface,
        }
    }
}

/// Frozen family-local style evidence retained through SVG postprocessing and completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FamilyStyleReport {
    family_id: DiagramFamilyId,
    evaluation: FamilyStyleEvaluation,
    output_mutated: bool,
    theme_required: Vec<FamilyThemeMechanismKey>,
    theme_applied: Vec<FamilyThemeMechanismKey>,
    #[cfg(merman_internal_theme_acceptance)]
    theme_route_cutover_facts: Vec<crate::theme_route_cutover::ThemeRouteCutoverFact>,
    #[cfg(merman_internal_theme_acceptance)]
    theme_raster_paint_binding_facts: Vec<crate::theme_raster_paint::ThemeRasterPaintBindingFact>,
    native_filter_receipt: Option<crate::__private::NativeSvgFilterReceipt>,
    #[cfg(all(merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
    architecture_text_cutover_receipt: Option<crate::__private::ArchitectureTextCutoverReceipt>,
    theme_not_applicable: Vec<FamilyThemeMechanismKey>,
    theme_residuals: Vec<FamilyThemeResidual>,
    compatibility_residual_count: usize,
    mermaid_compatibility_residual_count: usize,
    residuals: Vec<FamilyStyleResidual>,
}

impl FamilyStyleReport {
    fn freeze(plan: &ResolvedFamilyStylePlan) -> Self {
        let mut residuals = Vec::new();
        let evaluation = match &plan.payload {
            FamilyStylePayload::NotApplicable => FamilyStyleEvaluation::NotApplicable,
            FamilyStylePayload::Unadapted => FamilyStyleEvaluation::Unadapted,
            FamilyStylePayload::Evaluated => FamilyStyleEvaluation::Evaluated,
            FamilyStylePayload::State(state) => {
                for residual in state.residuals() {
                    let residual = FamilyStyleResidual::freeze(residual);
                    if !residuals.contains(&residual) {
                        residuals.push(residual);
                    }
                }
                FamilyStyleEvaluation::Evaluated
            }
        };
        for residual in &plan.source_style_residuals {
            let residual = FamilyStyleResidual::freeze(residual);
            if !residuals.contains(&residual) {
                residuals.push(residual);
            }
        }
        Self {
            family_id: plan.family_id,
            evaluation,
            output_mutated: plan.output_mutated,
            theme_required: plan.theme_evidence.required.clone(),
            theme_applied: plan.theme_evidence.applied.clone(),
            #[cfg(merman_internal_theme_acceptance)]
            theme_route_cutover_facts: plan.theme_route_cutover_facts(),
            #[cfg(merman_internal_theme_acceptance)]
            theme_raster_paint_binding_facts: plan.theme_raster_paint_binding_facts.clone(),
            native_filter_receipt: plan.native_filter_receipt,
            #[cfg(all(merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
            architecture_text_cutover_receipt: plan.architecture_text_cutover_receipt.clone(),
            theme_not_applicable: plan.theme_evidence.not_applicable.clone(),
            theme_residuals: plan.theme_evidence.residuals.clone(),
            compatibility_residual_count: plan.compatibility_residual_count,
            mermaid_compatibility_residual_count: plan.mermaid_compatibility_residual_count,
            residuals,
        }
    }

    fn invalidate_for_output_mutation(mut self) -> Self {
        self.output_mutated = true;
        for key in std::mem::take(&mut self.theme_applied) {
            if !self
                .theme_residuals
                .iter()
                .any(|residual| residual.key == key)
            {
                self.theme_residuals.push(FamilyThemeResidual {
                    key,
                    reason: FamilyThemeResidualReason::OutputMutation,
                });
            }
        }
        self.native_filter_receipt = None;
        #[cfg(merman_internal_theme_acceptance)]
        self.theme_route_cutover_facts.clear();
        #[cfg(merman_internal_theme_acceptance)]
        self.theme_raster_paint_binding_facts.clear();
        #[cfg(all(merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
        {
            self.architecture_text_cutover_receipt = None;
        }
        self
    }

    pub const fn family_id(&self) -> DiagramFamilyId {
        self.family_id
    }

    #[cfg(test)]
    pub const fn evaluation(&self) -> FamilyStyleEvaluation {
        self.evaluation
    }

    #[cfg(test)]
    pub(crate) const fn output_mutated(&self) -> bool {
        self.output_mutated
    }

    #[cfg(test)]
    pub fn residuals(&self) -> &[FamilyStyleResidual] {
        &self.residuals
    }

    /// Returns typed mechanisms the family adapter explicitly emitted or consumed.
    #[cfg(test)]
    pub fn theme_applied_mechanisms(&self) -> &[FamilyThemeMechanismKey] {
        &self.theme_applied
    }

    pub(crate) const fn native_filter_receipt(
        &self,
    ) -> Option<crate::__private::NativeSvgFilterReceipt> {
        self.native_filter_receipt
    }

    #[cfg(all(merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
    pub(crate) fn architecture_text_cutover_receipt(
        &self,
    ) -> Option<&crate::__private::ArchitectureTextCutoverReceipt> {
        self.architecture_text_cutover_receipt.as_ref()
    }

    /// Returns recipe mechanisms evaluated against this document but not selected by any rendered
    /// target, variant, or ordinal.
    #[cfg(test)]
    pub fn theme_not_applicable_mechanisms(&self) -> &[FamilyThemeMechanismKey] {
        &self.theme_not_applicable
    }

    /// Returns typed semantic mechanisms that remain outside this adapter's proof boundary.
    #[cfg(test)]
    pub fn theme_residuals(&self) -> &[FamilyThemeResidual] {
        &self.theme_residuals
    }

    /// Returns temporary family-local Mermaid compatibility contributions used by this operation.
    #[cfg(test)]
    pub const fn compatibility_residual_count(&self) -> usize {
        self.compatibility_residual_count
    }

    pub fn theme_coverage_complete(&self) -> bool {
        let required = self
            .theme_required
            .iter()
            .collect::<std::collections::BTreeSet<_>>();
        let applied = self
            .theme_applied
            .iter()
            .collect::<std::collections::BTreeSet<_>>();
        let residuals = self
            .theme_residuals
            .iter()
            .map(FamilyThemeResidual::key)
            .collect::<std::collections::BTreeSet<_>>();
        let not_applicable = self
            .theme_not_applicable
            .iter()
            .collect::<std::collections::BTreeSet<_>>();
        let accounted = applied
            .iter()
            .chain(not_applicable.iter())
            .chain(residuals.iter())
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        required.len() == self.theme_required.len()
            && applied.len() == self.theme_applied.len()
            && not_applicable.len() == self.theme_not_applicable.len()
            && residuals.len() == self.theme_residuals.len()
            && applied.is_disjoint(&residuals)
            && applied.is_disjoint(&not_applicable)
            && not_applicable.is_disjoint(&residuals)
            && accounted == required
    }

    pub fn verification(&self) -> FamilyStyleVerification {
        if self.output_mutated {
            return FamilyStyleVerification::Unverified;
        }
        if self.compatibility_residual_count != 0 || self.mermaid_compatibility_residual_count != 0
        {
            return FamilyStyleVerification::Unverified;
        }
        if !self.residuals.is_empty() {
            return FamilyStyleVerification::Unverified;
        }
        match self.evaluation {
            FamilyStyleEvaluation::NotApplicable => FamilyStyleVerification::NotApplicable,
            FamilyStyleEvaluation::Unadapted => FamilyStyleVerification::Unadapted,
            FamilyStyleEvaluation::Evaluated if !self.theme_coverage_complete() => {
                FamilyStyleVerification::Incomplete
            }
            FamilyStyleEvaluation::Evaluated
                if self.theme_residuals.is_empty() && self.residuals.is_empty() =>
            {
                FamilyStyleVerification::Verified
            }
            FamilyStyleEvaluation::Evaluated => FamilyStyleVerification::Unverified,
        }
    }

    #[cfg(any(test, merman_internal_theme_acceptance))]
    pub fn is_verified(&self) -> bool {
        self.verification() == FamilyStyleVerification::Verified
    }

    fn ensure_portable(&self) -> Result<()> {
        if self.output_mutated {
            return Err(Error::UnverifiedFamilyOutputMutation {
                family_id: self.family_id,
            });
        }
        self.ensure_compatibility_portable()?;
        match self.verification() {
            FamilyStyleVerification::NotApplicable | FamilyStyleVerification::Verified => Ok(()),
            FamilyStyleVerification::Unadapted => Err(Error::UnadaptedFamilyTheme {
                family_id: self.family_id,
            }),
            FamilyStyleVerification::Incomplete => Err(Error::IncompleteFamilyTheme {
                family_id: self.family_id,
                required_count: self.theme_required.len(),
                accounted_count: self.theme_applied.len()
                    + self.theme_not_applicable.len()
                    + self.theme_residuals.len(),
            }),
            FamilyStyleVerification::Unverified => {
                if !self.theme_residuals.is_empty() {
                    return Err(Error::UnverifiedFamilyTheme {
                        family_id: self.family_id,
                        residual_count: self.theme_residuals.len(),
                    });
                }
                debug_assert!(!self.residuals.is_empty());
                Err(Error::UnverifiedFamilyStyle {
                    family_id: self.family_id,
                    residual_count: self.residuals.len(),
                })
            }
        }
    }

    fn ensure_portable_before_terminal_evidence(&self) -> Result<()> {
        if self.output_mutated {
            return Err(Error::UnverifiedFamilyOutputMutation {
                family_id: self.family_id,
            });
        }
        self.ensure_compatibility_portable()?;
        match self.verification() {
            FamilyStyleVerification::NotApplicable
            | FamilyStyleVerification::Verified
            | FamilyStyleVerification::Incomplete
            | FamilyStyleVerification::Unadapted => Ok(()),
            FamilyStyleVerification::Unverified => {
                if !self.theme_residuals.is_empty() {
                    return Err(Error::UnverifiedFamilyTheme {
                        family_id: self.family_id,
                        residual_count: self.theme_residuals.len(),
                    });
                }
                debug_assert!(!self.residuals.is_empty());
                Err(Error::UnverifiedFamilyStyle {
                    family_id: self.family_id,
                    residual_count: self.residuals.len(),
                })
            }
        }
    }

    fn ensure_compatibility_portable(&self) -> Result<()> {
        if self.compatibility_residual_count != 0 {
            return Err(Error::LegacyFamilyThemeCompatibility {
                family_id: self.family_id,
                residual_count: self.compatibility_residual_count,
            });
        }
        if self.mermaid_compatibility_residual_count != 0 {
            return Err(Error::MermaidThemeCompatibility {
                family_id: self.family_id,
                residual_count: self.mermaid_compatibility_residual_count,
            });
        }
        Ok(())
    }
}

/// Frozen family identity and operation evidence captured after SVG work completes.
///
/// The recipe fingerprint identifies selected visual intent and retained resources only. Host
/// admission and family evaluation remain separate structured reports; the fingerprint does not
/// claim that any semantic target was applied or emitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyRenderReport {
    root_theme: RootThemeReport,
    style: FamilyStyleReport,
    session: RenderSessionReport,
}

impl FamilyRenderReport {
    fn freeze(
        root_theme: RootThemeReport,
        style: FamilyStyleReport,
        session: RenderSession,
    ) -> Self {
        Self {
            root_theme,
            style,
            session: session.report(),
        }
    }

    pub const fn family_id(&self) -> DiagramFamilyId {
        self.style.family_id()
    }

    #[cfg(any(test, merman_internal_theme_acceptance))]
    pub(crate) const fn style_report(&self) -> &FamilyStyleReport {
        &self.style
    }

    #[cfg(merman_internal_theme_acceptance)]
    pub(crate) fn theme_route_cutover_facts(
        &self,
    ) -> &[crate::theme_route_cutover::ThemeRouteCutoverFact] {
        &self.style.theme_route_cutover_facts
    }

    #[cfg(merman_internal_theme_acceptance)]
    pub(crate) fn theme_raster_paint_binding_facts(
        &self,
    ) -> &[crate::theme_raster_paint::ThemeRasterPaintBindingFact] {
        &self.style.theme_raster_paint_binding_facts
    }

    pub(crate) const fn root_theme_report(&self) -> &RootThemeReport {
        &self.root_theme
    }

    pub(crate) const fn native_filter_receipt(
        &self,
    ) -> Option<crate::__private::NativeSvgFilterReceipt> {
        self.style.native_filter_receipt()
    }

    #[cfg(all(merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
    pub(crate) fn architecture_text_cutover_receipt(
        &self,
    ) -> Option<&crate::__private::ArchitectureTextCutoverReceipt> {
        self.style.architecture_text_cutover_receipt()
    }

    pub(crate) fn evidence_summary(&self) -> crate::__private::FamilyEvidenceSummary {
        let accounted_count = self.style.theme_applied.len()
            + self.style.theme_not_applicable.len()
            + self.style.theme_residuals.len();
        crate::__private::FamilyEvidenceSummary::new(
            crate::__private::FamilyEvidenceStatus::from_verification(self.style.verification()),
            self.style.theme_required.len(),
            accounted_count,
            self.style.theme_applied.len(),
            self.style.theme_not_applicable.len(),
            self.style.theme_residuals.len(),
            self.style.residuals.len(),
            self.style.compatibility_residual_count,
            self.style.mermaid_compatibility_residual_count,
            self.style.output_mutated,
        )
    }

    pub fn theme_recipe_fingerprint(&self) -> Option<ThemeRecipeFingerprint> {
        self.session.theme_recipe_fingerprint()
    }

    pub const fn session_report(&self) -> &RenderSessionReport {
        &self.session
    }
}

struct FamilyRenderContext {
    root_theme: RootThemePlan,
    style_plan: ResolvedFamilyStylePlan,
    session: RenderSession,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FamilyThemeEvidence {
    required: Vec<FamilyThemeMechanismKey>,
    required_index: BTreeSet<FamilyThemeMechanismKey>,
    accounted_index: BTreeSet<FamilyThemeMechanismKey>,
    applied: Vec<FamilyThemeMechanismKey>,
    applied_capabilities: BTreeMap<FamilyThemeMechanismKey, BTreeSet<ThemeCapability>>,
    not_applicable: Vec<FamilyThemeMechanismKey>,
    residuals: Vec<FamilyThemeResidual>,
}

impl FamilyThemeEvidence {
    pub(crate) fn from_theme(theme: Option<&ResolvedDiagramTheme>) -> Self {
        let Some(theme) = theme else {
            return Self::default();
        };
        let mut evidence = Self::default();
        for key in theme.family_evidence_mechanism_keys() {
            if evidence.required_index.insert(key.clone()) {
                evidence.required.push(key);
            }
        }
        evidence
    }

    fn not_applicable(theme: Option<&ResolvedDiagramTheme>) -> bool {
        theme.is_none_or(|theme| theme.family_evidence_mechanism_keys().next().is_none())
    }

    #[cfg(test)]
    pub(crate) fn required_mechanisms(&self) -> &[FamilyThemeMechanismKey] {
        &self.required
    }

    pub(crate) fn mark_applied_with_capabilities(
        &mut self,
        key: FamilyThemeMechanismKey,
        capabilities: impl IntoIterator<Item = ThemeCapability>,
    ) {
        if self.required_index.contains(&key) && self.accounted_index.insert(key.clone()) {
            let capabilities = capabilities.into_iter().collect::<BTreeSet<_>>();
            self.applied.push(key.clone());
            self.applied_capabilities.insert(key, capabilities);
        }
    }

    pub(crate) fn mark_not_applicable(&mut self, key: FamilyThemeMechanismKey) {
        if self.required_index.contains(&key) && self.accounted_index.insert(key.clone()) {
            self.not_applicable.push(key);
        }
    }

    pub(crate) fn mark_residual(
        &mut self,
        key: FamilyThemeMechanismKey,
        reason: FamilyThemeResidualReason,
    ) {
        if self.required_index.contains(&key) && self.accounted_index.insert(key.clone()) {
            self.residuals.push(FamilyThemeResidual { key, reason });
        }
    }

    fn mark_output_visibility_filtered(&mut self) {
        self.move_applied_to_residual(FamilyThemeResidualReason::OutputVisibilityFiltered);
    }

    fn mark_state_output_visibility_filtered(&mut self, debug: &SvgDebugOptions) {
        if !debug.include_nodes {
            self.mark_output_visibility_filtered();
            return;
        }
        if debug.include_edges {
            return;
        }
        self.move_applied_to_residual_where(
            FamilyThemeResidualReason::OutputVisibilityFiltered,
            |key| {
                matches!(
                    key,
                    FamilyThemeMechanismKey::Rule {
                        target: crate::diagram_theme::ThemeTarget::Transition
                            | crate::diagram_theme::ThemeTarget::TransitionLabel
                            | crate::diagram_theme::ThemeTarget::TransitionMarker
                            | crate::diagram_theme::ThemeTarget::TransitionLabelBackground,
                        ..
                    } | FamilyThemeMechanismKey::OrdinalPalette {
                        target: crate::diagram_theme::ThemeTarget::Transition
                            | crate::diagram_theme::ThemeTarget::TransitionLabel
                            | crate::diagram_theme::ThemeTarget::TransitionMarker
                            | crate::diagram_theme::ThemeTarget::TransitionLabelBackground,
                    } | FamilyThemeMechanismKey::EffectBinding {
                        target: crate::diagram_theme::ThemeTarget::Transition
                            | crate::diagram_theme::ThemeTarget::TransitionLabel
                            | crate::diagram_theme::ThemeTarget::TransitionMarker
                            | crate::diagram_theme::ThemeTarget::TransitionLabelBackground,
                        ..
                    }
                )
            },
        );
    }

    fn mark_output_mutated(&mut self) {
        self.move_applied_to_residual(FamilyThemeResidualReason::OutputMutation);
    }

    fn mark_effect_emission_unverified(&mut self) {
        let effect_keys = self
            .applied_capabilities
            .iter()
            .filter(|(_, capabilities)| {
                capabilities.contains(&ThemeCapability::Shadow)
                    || capabilities.contains(&ThemeCapability::SvgFilter)
            })
            .map(|(key, _)| key.clone())
            .collect::<BTreeSet<_>>();
        self.move_applied_to_residual_where(FamilyThemeResidualReason::UnsupportedEffect, |key| {
            effect_keys.contains(key)
        });
    }

    fn move_applied_to_residual(&mut self, reason: FamilyThemeResidualReason) {
        self.move_applied_to_residual_where(reason, |_| true);
    }

    fn move_applied_to_residual_where(
        &mut self,
        reason: FamilyThemeResidualReason,
        mut should_move: impl FnMut(&FamilyThemeMechanismKey) -> bool,
    ) {
        for key in std::mem::take(&mut self.applied) {
            if !should_move(&key) {
                self.applied.push(key);
                continue;
            }
            self.applied_capabilities.remove(&key);
            if !self.residuals.iter().any(|residual| residual.key == key) {
                self.residuals.push(FamilyThemeResidual { key, reason });
            }
        }
    }

    pub(crate) fn applied_capabilities(&self) -> BTreeSet<ThemeCapability> {
        self.applied_capabilities
            .values()
            .flat_map(|capabilities| capabilities.iter().copied())
            .collect()
    }

    pub(crate) fn merge_accounted_from(&mut self, mut other: Self) {
        debug_assert_eq!(self.required, other.required);
        for key in other.applied {
            let capabilities = other.applied_capabilities.remove(&key).unwrap_or_default();
            self.mark_applied_with_capabilities(key, capabilities);
        }
        for key in other.not_applicable {
            self.mark_not_applicable(key);
        }
        for residual in other.residuals {
            self.mark_residual(residual.key, residual.reason);
        }
    }
    #[cfg(test)]
    pub(crate) fn applied(&self) -> &[FamilyThemeMechanismKey] {
        &self.applied
    }

    pub(crate) fn not_applicable_mechanisms(&self) -> &[FamilyThemeMechanismKey] {
        &self.not_applicable
    }

    pub(crate) fn residuals(&self) -> &[FamilyThemeResidual] {
        &self.residuals
    }
}

#[derive(Debug)]
enum FamilyStylePayload {
    NotApplicable,
    Unadapted,
    Evaluated,
    State(Box<crate::state::StateStylePlan>),
}

#[derive(Debug)]
pub(crate) struct ResolvedFamilyStylePlan {
    family_id: DiagramFamilyId,
    resolved_theme: Option<Box<ResolvedDiagramTheme>>,
    theme_evidence: FamilyThemeEvidence,
    source_style_residuals: Vec<SourceStyleResidual>,
    native_filter_receipt: Option<crate::__private::NativeSvgFilterReceipt>,
    #[cfg(merman_internal_theme_acceptance)]
    theme_raster_paint_binding_facts: Vec<crate::theme_raster_paint::ThemeRasterPaintBindingFact>,
    #[cfg(all(merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
    architecture_text_cutover_receipt: Option<crate::__private::ArchitectureTextCutoverReceipt>,
    output_mutated: bool,
    compatibility_residual_count: usize,
    mermaid_compatibility_residual_count: usize,
    payload: FamilyStylePayload,
}

impl ResolvedFamilyStylePlan {
    fn new(session: &RenderSession, family_id: DiagramFamilyId) -> Self {
        let resolved_theme = session
            .theme()
            .map(|theme| Box::new(theme.resolve(family_id)));
        let theme_evidence = FamilyThemeEvidence::from_theme(resolved_theme.as_deref());
        let payload = if !FamilyThemeEvidence::not_applicable(resolved_theme.as_deref()) {
            FamilyStylePayload::Unadapted
        } else {
            FamilyStylePayload::NotApplicable
        };
        Self {
            family_id,
            resolved_theme,
            theme_evidence,
            source_style_residuals: Vec::new(),
            native_filter_receipt: None,
            #[cfg(merman_internal_theme_acceptance)]
            theme_raster_paint_binding_facts: Vec::new(),
            #[cfg(all(merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
            architecture_text_cutover_receipt: None,
            output_mutated: false,
            compatibility_residual_count: 0,
            mermaid_compatibility_residual_count: 0,
            payload,
        }
    }

    #[cfg(merman_internal_theme_acceptance)]
    fn theme_route_cutover_facts(&self) -> Vec<crate::theme_route_cutover::ThemeRouteCutoverFact> {
        self.resolved_theme
            .as_deref()
            .map_or_else(Vec::new, |theme| {
                crate::theme_route_cutover::collect_theme_route_cutover_facts(
                    theme,
                    &self.theme_evidence.applied,
                )
            })
    }

    fn adapt_state(
        &mut self,
        model: &merman_core::diagrams::state::StateDiagramRenderModel,
        effective_config: &merman_core::MermaidConfig,
        title: Option<&str>,
        prepared_text_available: bool,
        effective_theme_resource_policy: Arc<crate::diagram_theme::ThemeResourcePolicy>,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> Result<()> {
        debug_assert_eq!(self.family_id, DiagramFamilyId::STATE);
        let (plan, theme_evidence) = crate::state::StateStylePlan::resolve_with_config(
            model,
            effective_config,
            self.resolved_theme.as_deref(),
            effective_theme_resource_policy,
            title,
            prepared_text_available,
            work_meter,
        )?;
        self.theme_evidence = theme_evidence;
        self.payload = FamilyStylePayload::State(Box::new(plan));
        Ok(())
    }

    fn observe_compatibility(&mut self, metadata: &merman_core::ParseMetadata) {
        let evidence = merman_core::__private::theme_parse_evidence(metadata);
        self.mermaid_compatibility_residual_count = if self.family_id == DiagramFamilyId::PACKET {
            let deferred = crate::packet::deferred_mermaid_compatibility_consumptions(
                self.resolved_theme.as_deref(),
                &metadata.effective_config,
                &evidence,
            );
            evidence
                .reconcile_mermaid_consumptions(deferred.iter())
                .remaining_field_count()
        } else {
            evidence.mermaid_residual_count()
        };
        self.compatibility_residual_count = if self.family_id == DiagramFamilyId::ERROR {
            crate::error::remaining_legacy_compatibility_residual_count(
                self.resolved_theme.as_deref(),
                &evidence,
            )
        } else {
            evidence.fallback_contribution_count()
        };
    }

    fn reconcile_packet_mermaid_compatibility(
        &mut self,
        evidence: &merman_core::__private::ThemeParseEvidence,
        consumptions: &[merman_core::__private::ThemeCompatibilityFieldConsumption],
    ) {
        debug_assert_eq!(self.family_id, DiagramFamilyId::PACKET);
        self.mermaid_compatibility_residual_count = evidence
            .reconcile_mermaid_consumptions(consumptions.iter())
            .remaining_field_count();
    }

    fn merge_flowchart_evidence(
        &mut self,
        evidence: FamilyThemeEvidence,
        source_style_residuals: Vec<SourceStyleResidual>,
    ) {
        debug_assert!(matches!(
            self.family_id,
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
        ));
        self.theme_evidence = evidence;
        self.source_style_residuals = source_style_residuals;
        self.payload = if FamilyThemeEvidence::not_applicable(self.resolved_theme.as_deref())
            && self.source_style_residuals.is_empty()
        {
            FamilyStylePayload::NotApplicable
        } else {
            FamilyStylePayload::Evaluated
        };
    }

    fn merge_sequence_evidence(&mut self, evidence: FamilyThemeEvidence) {
        debug_assert_eq!(self.family_id, DiagramFamilyId::SEQUENCE);
        self.theme_evidence = evidence;
        self.payload = if FamilyThemeEvidence::not_applicable(self.resolved_theme.as_deref()) {
            FamilyStylePayload::NotApplicable
        } else {
            FamilyStylePayload::Evaluated
        };
    }

    #[cfg(merman_internal_theme_acceptance)]
    fn record_theme_raster_paint_binding_fact(
        &mut self,
        fact: crate::theme_raster_paint::ThemeRasterPaintBindingFact,
    ) {
        debug_assert_eq!(fact.family_id(), self.family_id);
        self.theme_raster_paint_binding_facts.push(fact);
    }

    fn merge_accounted_terminal_evidence(
        &mut self,
        expected_family: DiagramFamilyId,
        evidence: FamilyThemeEvidence,
    ) {
        debug_assert_eq!(self.family_id, expected_family);
        self.theme_evidence.merge_accounted_from(evidence);
        self.payload = if FamilyThemeEvidence::not_applicable(self.resolved_theme.as_deref()) {
            FamilyStylePayload::NotApplicable
        } else {
            FamilyStylePayload::Evaluated
        };
    }

    #[cfg(all(merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
    fn record_architecture_text_cutover_receipt(
        &mut self,
        receipt: Option<crate::__private::ArchitectureTextCutoverReceipt>,
    ) {
        debug_assert_eq!(self.family_id, DiagramFamilyId::ARCHITECTURE);
        self.architecture_text_cutover_receipt = receipt;
    }

    fn reconcile_state_effect_evidence(
        &mut self,
        emitted: Option<crate::__private::NativeSvgFilterReceipt>,
    ) {
        debug_assert_eq!(self.family_id, DiagramFamilyId::STATE);
        let effect_capabilities_applied =
            self.theme_evidence
                .applied_capabilities()
                .iter()
                .any(|capability| {
                    matches!(
                        capability,
                        ThemeCapability::Shadow | ThemeCapability::SvgFilter
                    )
                });
        if !effect_capabilities_applied {
            self.native_filter_receipt = None;
            return;
        }

        let expected_application_count = self
            .state()
            .map(crate::state::StateStylePlan::expected_native_filter_application_count)
            .unwrap_or(0);
        if emitted.is_some_and(|receipt| {
            usize::try_from(receipt.hard_shadow_count()).ok() == Some(expected_application_count)
                && receipt.hard_shadow_count() == receipt.reference_count()
        }) {
            self.native_filter_receipt = emitted;
            return;
        }

        self.native_filter_receipt = None;
        self.theme_evidence.mark_effect_emission_unverified();
    }

    fn reconcile_state_terminal_evidence(&mut self) {
        debug_assert_eq!(self.family_id, DiagramFamilyId::STATE);
        let evidence = self
            .state()
            .map(crate::state::StateStylePlan::finish_theme_evidence);
        if let Some(evidence) = evidence {
            self.theme_evidence = evidence;
        }
    }

    fn observe_output_visibility(&mut self, debug: &SvgDebugOptions) {
        if self.family_id == DiagramFamilyId::STATE {
            self.theme_evidence
                .mark_state_output_visibility_filtered(debug);
        }
    }

    fn invalidate_for_output_mutation(&mut self) {
        self.output_mutated = true;
        self.native_filter_receipt = None;
        #[cfg(merman_internal_theme_acceptance)]
        self.theme_raster_paint_binding_facts.clear();
        #[cfg(all(merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
        {
            self.architecture_text_cutover_receipt = None;
        }
        self.theme_evidence.mark_output_mutated();
    }

    fn ensure_portable(&self, portability: ThemePortabilityRequirement) -> Result<()> {
        if portability != ThemePortabilityRequirement::RequirePortable {
            return Ok(());
        }

        FamilyStyleReport::freeze(self).ensure_portable()
    }

    fn ensure_portable_before_svg(&self, portability: ThemePortabilityRequirement) -> Result<()> {
        if portability != ThemePortabilityRequirement::RequirePortable {
            return Ok(());
        }
        let waits_for_svg_evidence = matches!(
            self.family_id,
            DiagramFamilyId::STATE
                | DiagramFamilyId::FLOWCHART
                | DiagramFamilyId::SWIMLANE
                | DiagramFamilyId::MINDMAP
                | DiagramFamilyId::SEQUENCE
                | DiagramFamilyId::CLASS
                | DiagramFamilyId::KANBAN
                | DiagramFamilyId::GANTT
                | DiagramFamilyId::PIE
                | DiagramFamilyId::TIMELINE
                | DiagramFamilyId::TREE_VIEW
                | DiagramFamilyId::JOURNEY
                | DiagramFamilyId::QUADRANT_CHART
                | DiagramFamilyId::XY_CHART
                | DiagramFamilyId::RADAR
                | DiagramFamilyId::TREEMAP
                | DiagramFamilyId::REQUIREMENT
                | DiagramFamilyId::PACKET
                | DiagramFamilyId::SANKEY
                | DiagramFamilyId::BLOCK
                | DiagramFamilyId::RAILROAD
                | DiagramFamilyId::INFO
                | DiagramFamilyId::ERROR
                | DiagramFamilyId::CYNEFIN
                | DiagramFamilyId::WARDLEY
                | DiagramFamilyId::GIT_GRAPH
                | DiagramFamilyId::C4
                | DiagramFamilyId::ER
                | DiagramFamilyId::VENN
                | DiagramFamilyId::ZENUML
                | DiagramFamilyId::EVENT_MODELING
                | DiagramFamilyId::ISHIKAWA
                | DiagramFamilyId::ARCHITECTURE
        ) && self
            .resolved_theme
            .as_deref()
            .is_some_and(ResolvedDiagramTheme::has_family_mechanism_routes);
        let report = FamilyStyleReport::freeze(self);
        if waits_for_svg_evidence {
            report.ensure_portable_before_terminal_evidence()
        } else {
            report.ensure_portable()
        }
    }

    pub(crate) const fn family_id(&self) -> DiagramFamilyId {
        self.family_id
    }

    pub(crate) fn resolved_theme(&self) -> Option<&ResolvedDiagramTheme> {
        self.resolved_theme.as_deref()
    }

    pub(crate) fn state(&self) -> Option<&crate::state::StateStylePlan> {
        match &self.payload {
            FamilyStylePayload::State(plan) => Some(plan),
            FamilyStylePayload::NotApplicable
            | FamilyStylePayload::Unadapted
            | FamilyStylePayload::Evaluated => None,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct FamilyExecutionView<'a> {
    family_id: DiagramFamilyId,
    session: &'a RenderSession,
    root_theme: Option<&'a RootThemePlan>,
    style_plan: Option<&'a ResolvedFamilyStylePlan>,
}

impl<'a> FamilyExecutionView<'a> {
    fn new(
        session: &'a RenderSession,
        root_theme: &'a RootThemePlan,
        style_plan: &'a ResolvedFamilyStylePlan,
    ) -> Self {
        let execution = Self {
            family_id: style_plan.family_id(),
            session,
            root_theme: Some(root_theme),
            style_plan: Some(style_plan),
        };
        debug_assert_eq!(
            execution.resolved_theme().is_some(),
            session.theme_recipe_fingerprint().is_some(),
            "family theme plan presence must match the render session"
        );
        debug_assert_eq!(
            execution
                .resolved_theme()
                .map(ResolvedDiagramTheme::family_id),
            session
                .theme_recipe_fingerprint()
                .map(|_| style_plan.family_id()),
            "family theme plan must match the planned render family"
        );
        execution
    }

    pub(crate) const fn family_id(self) -> DiagramFamilyId {
        self.family_id
    }

    pub(crate) const fn session(self) -> &'a RenderSession {
        self.session
    }

    pub(crate) const fn root_theme_plan(self) -> Option<&'a RootThemePlan> {
        self.root_theme
    }

    pub(crate) fn resolved_theme(self) -> Option<&'a ResolvedDiagramTheme> {
        match self.style_plan {
            Some(plan) => plan.resolved_theme(),
            None => None,
        }
    }

    pub(crate) const fn style_plan(self) -> Option<&'a ResolvedFamilyStylePlan> {
        self.style_plan
    }

    #[cfg(test)]
    pub(crate) fn for_test(session: &'a RenderSession, family_id: DiagramFamilyId) -> Self {
        Self {
            family_id,
            session,
            root_theme: None,
            style_plan: None,
        }
    }
}

impl FamilyRenderContext {
    fn resolve(session: RenderSession, family_id: DiagramFamilyId) -> Self {
        let root_theme = RootThemePlan::from_theme(session.theme());
        let style_plan = ResolvedFamilyStylePlan::new(&session, family_id);
        Self {
            root_theme,
            style_plan,
            session,
        }
    }

    const fn family_id(&self) -> DiagramFamilyId {
        self.style_plan.family_id()
    }

    fn resolved_theme(&self) -> Option<&ResolvedDiagramTheme> {
        self.style_plan.resolved_theme()
    }

    const fn session(&self) -> &RenderSession {
        &self.session
    }

    fn execution(&self) -> FamilyExecutionView<'_> {
        FamilyExecutionView::new(&self.session, &self.root_theme, &self.style_plan)
    }

    fn adapt_state(
        &mut self,
        model: &merman_core::diagrams::state::StateDiagramRenderModel,
        effective_config: &merman_core::MermaidConfig,
        title: Option<&str>,
    ) -> Result<()> {
        let effective_theme_resource_policy = self.session.effective_theme_resource_policy();
        self.style_plan.adapt_state(
            model,
            effective_config,
            title,
            self.session.prepared_text_layout().is_some(),
            effective_theme_resource_policy,
            self.session.work_meter().as_ref(),
        )
    }

    fn observe_compatibility(&mut self, metadata: &merman_core::ParseMetadata) {
        self.style_plan.observe_compatibility(metadata);
    }

    fn observe_output_visibility(&mut self, debug: &SvgDebugOptions) {
        self.style_plan.observe_output_visibility(debug);
    }

    fn reconcile_state_effect_evidence(
        &mut self,
        emitted: Option<crate::__private::NativeSvgFilterReceipt>,
    ) {
        self.style_plan.reconcile_state_effect_evidence(emitted);
    }

    fn reconcile_state_terminal_evidence(&mut self) {
        self.style_plan.reconcile_state_terminal_evidence();
    }

    fn invalidate_for_output_mutation(&mut self) {
        self.style_plan.invalidate_for_output_mutation();
    }

    fn merge_flowchart_evidence(
        &mut self,
        evidence: FamilyThemeEvidence,
        source_style_residuals: Vec<SourceStyleResidual>,
    ) {
        self.style_plan
            .merge_flowchart_evidence(evidence, source_style_residuals);
    }

    fn merge_sequence_evidence(&mut self, evidence: FamilyThemeEvidence) {
        self.style_plan.merge_sequence_evidence(evidence);
    }

    #[cfg(merman_internal_theme_acceptance)]
    fn record_theme_raster_paint_binding_fact(
        &mut self,
        fact: crate::theme_raster_paint::ThemeRasterPaintBindingFact,
    ) {
        self.style_plan.record_theme_raster_paint_binding_fact(fact);
    }

    fn merge_accounted_terminal_evidence(
        &mut self,
        expected_family: DiagramFamilyId,
        evidence: FamilyThemeEvidence,
    ) {
        self.style_plan
            .merge_accounted_terminal_evidence(expected_family, evidence);
    }

    fn reconcile_packet_mermaid_compatibility(
        &mut self,
        evidence: &merman_core::__private::ThemeParseEvidence,
        consumptions: &[merman_core::__private::ThemeCompatibilityFieldConsumption],
    ) {
        self.style_plan
            .reconcile_packet_mermaid_compatibility(evidence, consumptions);
    }

    fn ensure_portable(&self) -> Result<()> {
        let portability = self
            .session
            .theme_portability_requirement()
            .unwrap_or(ThemePortabilityRequirement::BestEffort);
        self.style_plan.ensure_portable(portability)
    }

    fn ensure_portable_before_svg(&self) -> Result<()> {
        let portability = self
            .session
            .theme_portability_requirement()
            .unwrap_or(ThemePortabilityRequirement::BestEffort);
        self.style_plan.ensure_portable_before_svg(portability)
    }

    fn into_session_and_style_report(self) -> (RenderSession, FamilyStyleReport) {
        let style_report = FamilyStyleReport::freeze(&self.style_plan);
        (self.session, style_report)
    }
}

#[derive(Debug)]
pub(crate) struct FamilyPair<S, L> {
    semantic: S,
    layout: L,
}

impl<S, L> FamilyPair<S, L> {
    fn new(semantic: S, layout: L) -> Self {
        Self { semantic, layout }
    }

    pub(crate) fn semantic(&self) -> &S {
        &self.semantic
    }

    pub(crate) fn layout(&self) -> &L {
        &self.layout
    }
}

impl<S: BuiltinRenderSemantic, L> FamilyPair<S, L> {
    fn compatibility_json(
        &self,
        metadata: &ParseMetadata,
    ) -> merman_core::Result<serde_json::Value> {
        self.semantic.compatibility_json(metadata)
    }
}

#[derive(Debug)]
pub(crate) struct FlowchartFamilyArtifact<L> {
    pair: FamilyPair<diagrams::flowchart::FlowchartModel, L>,
    render_context: diagrams::flowchart::FlowchartRenderContext,
    edge_style_plan: crate::svg::FlowchartEdgeStylePlan,
    edge_theme: crate::flowchart::FlowchartEdgeThemeStyle,
    svg_label_sidecar: crate::flowchart::FlowchartSvgLabelSidecar,
    theme_evidence: crate::flowchart::FlowchartThemeEvidenceRecorder,
}

impl<L> FlowchartFamilyArtifact<L> {
    pub(crate) fn pair(&self) -> &FamilyPair<diagrams::flowchart::FlowchartModel, L> {
        &self.pair
    }

    pub(crate) fn render_context(&self) -> &diagrams::flowchart::FlowchartRenderContext {
        &self.render_context
    }

    pub(crate) const fn edge_style_plan(&self) -> &crate::svg::FlowchartEdgeStylePlan {
        &self.edge_style_plan
    }

    pub(crate) const fn edge_theme(&self) -> &crate::flowchart::FlowchartEdgeThemeStyle {
        &self.edge_theme
    }

    pub(crate) fn svg_label_sidecar(&self) -> &crate::flowchart::FlowchartSvgLabelSidecar {
        &self.svg_label_sidecar
    }

    pub(crate) const fn theme_evidence(&self) -> &crate::flowchart::FlowchartThemeEvidenceRecorder {
        &self.theme_evidence
    }
}

fn flowchart_artifact_theme_evidence<L>(
    artifact: &FlowchartFamilyArtifact<L>,
    theme: Option<&ResolvedDiagramTheme>,
) -> (FamilyThemeEvidence, Vec<SourceStyleResidual>) {
    let (mut evidence, source_residuals) = artifact.theme_evidence().finish(theme);
    if let Some(base_typography) = artifact.svg_label_sidecar().base_typography() {
        evidence.merge_accounted_from(base_typography.finish_evidence(theme));
    }
    (evidence, source_residuals)
}

#[derive(Debug)]
pub(crate) struct ClassFamilyArtifact {
    pair: FamilyPair<ClassDiagram, ClassDiagramLayout>,
    relation_theme: crate::class::ClassRelationThemePlan,
    typography_theme: crate::class::ClassTextThemePlan,
    theme_evidence: crate::class::ClassThemeEvidenceRecorder,
}

#[derive(Debug)]
pub(crate) struct C4FamilyArtifact {
    pair: FamilyPair<diagrams::c4::C4DiagramRenderModel, C4DiagramLayout>,
    cluster_theme: crate::c4::C4ClusterThemePlan,
    typography_theme: crate::c4::C4TypographyThemePlan,
    text_paint: crate::c4::C4TextPaintPlan,
}

impl C4FamilyArtifact {
    pub(crate) const fn text_paint(&self) -> &crate::c4::C4TextPaintPlan {
        &self.text_paint
    }
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::c4::C4DiagramRenderModel, C4DiagramLayout> {
        &self.pair
    }

    pub(crate) const fn cluster_theme(&self) -> &crate::c4::C4ClusterThemePlan {
        &self.cluster_theme
    }

    pub(crate) const fn typography_theme(&self) -> &crate::c4::C4TypographyThemePlan {
        &self.typography_theme
    }
}

impl ClassFamilyArtifact {
    pub(crate) const fn pair(&self) -> &FamilyPair<ClassDiagram, ClassDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn relation_theme(&self) -> &crate::class::ClassRelationThemePlan {
        &self.relation_theme
    }

    pub(crate) const fn typography_theme(&self) -> &crate::class::ClassTextThemePlan {
        &self.typography_theme
    }

    pub(crate) const fn theme_evidence(&self) -> &crate::class::ClassThemeEvidenceRecorder {
        &self.theme_evidence
    }
}

#[derive(Debug)]
pub(crate) struct GanttFamilyArtifact {
    pair: FamilyPair<diagrams::gantt::GanttDiagramRenderModel, GanttDiagramLayout>,
    task_theme: crate::gantt::GanttTaskTheme,
}

#[derive(Debug)]
pub(crate) struct PieFamilyArtifact {
    pair: FamilyPair<diagrams::pie::PieDiagramRenderModel, PieDiagramLayout>,
    theme: crate::pie::PieThemePlan,
}

#[derive(Debug)]
pub(crate) struct TimelineFamilyArtifact {
    pair: FamilyPair<diagrams::timeline::TimelineDiagramRenderModel, TimelineDiagramLayout>,
    event_theme: crate::timeline::TimelineEventTheme,
    typography_theme: crate::timeline::TimelineTypographyThemePlan,
    text_paint: crate::timeline::TimelineTextPaintPlan,
}

#[derive(Debug)]
pub(crate) struct JourneyFamilyArtifact {
    pair: FamilyPair<diagrams::journey::JourneyDiagramRenderModel, JourneyDiagramLayout>,
    task_theme: crate::journey::JourneyTaskTheme,
    text_paint: crate::journey::JourneyTextPaintPlan,
    typography_theme: crate::journey::JourneyTypographyThemePlan,
}

impl JourneyFamilyArtifact {
    pub(crate) const fn text_paint(&self) -> &crate::journey::JourneyTextPaintPlan {
        &self.text_paint
    }

    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::journey::JourneyDiagramRenderModel, JourneyDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn task_theme(&self) -> &crate::journey::JourneyTaskTheme {
        &self.task_theme
    }

    pub(crate) const fn typography_theme(&self) -> &crate::journey::JourneyTypographyThemePlan {
        &self.typography_theme
    }
}

#[derive(Debug)]
pub(crate) struct QuadrantChartFamilyArtifact {
    pair:
        FamilyPair<diagrams::quadrant_chart::QuadrantChartRenderModel, QuadrantChartDiagramLayout>,
    point_theme: crate::quadrantchart::QuadrantChartPointThemePlan,
    text_paint: crate::quadrantchart::QuadrantChartPaintPlan,
}

impl QuadrantChartFamilyArtifact {
    pub(crate) const fn text_paint(&self) -> &crate::quadrantchart::QuadrantChartPaintPlan {
        &self.text_paint
    }

    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::quadrant_chart::QuadrantChartRenderModel, QuadrantChartDiagramLayout>
    {
        &self.pair
    }

    pub(crate) const fn point_theme(&self) -> &crate::quadrantchart::QuadrantChartPointThemePlan {
        &self.point_theme
    }
}

#[derive(Debug)]
pub(crate) struct XyChartFamilyArtifact {
    pair: FamilyPair<diagrams::xychart::XyChartDiagramRenderModel, XyChartDiagramLayout>,
    series_paint: crate::xychart::XyChartSeriesPaintPlan,
    typography_theme: crate::xychart::XyChartTypographyThemePlan,
    paint_theme: crate::xychart::XyChartPaintPlan,
}

#[derive(Debug)]
pub(crate) struct RadarFamilyArtifact {
    pair: FamilyPair<diagrams::radar::RadarDiagramRenderModel, RadarDiagramLayout>,
    series_paint: crate::radar::RadarSeriesPaintPlan,
    title_theme: crate::radar::RadarTitleThemePlan,
    axis_paint: crate::radar::RadarAxisPaintPlan,
    text_paint: crate::radar::RadarTextPaintPlan,
    typography_theme: crate::radar::RadarTypographyThemePlan,
}

#[derive(Debug)]
pub(crate) struct SankeyFamilyArtifact {
    pair: FamilyPair<diagrams::sankey::SankeyDiagramRenderModel, SankeyDiagramLayout>,
    node_palette: crate::sankey::SankeyNodePalettePlan,
    typography_theme: crate::sankey::SankeyTypographyThemePlan,
}

#[derive(Debug)]
pub(crate) struct VennFamilyArtifact {
    pair: FamilyPair<diagrams::venn::VennDiagramRenderModel, VennDiagramLayout>,
    title_theme: crate::venn::VennTitleThemePlan,
    typography_theme: crate::venn::VennTypographyThemePlan,
}

#[derive(Debug)]
pub(crate) struct ZenumlFamilyArtifact {
    pair:
        FamilyPair<diagrams::zenuml::ZenumlDiagramRenderModel, crate::zenuml::ZenumlDiagramLayout>,
    title_theme: crate::zenuml::ZenumlTitleThemePlan,
}

impl ZenumlFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::zenuml::ZenumlDiagramRenderModel, crate::zenuml::ZenumlDiagramLayout>
    {
        &self.pair
    }

    pub(crate) const fn title_theme(&self) -> &crate::zenuml::ZenumlTitleThemePlan {
        &self.title_theme
    }
}

#[derive(Debug)]
pub(crate) struct EventModelingFamilyArtifact {
    pair: FamilyPair<
        diagrams::eventmodeling::EventModelingDiagramRenderModel,
        EventModelingDiagramLayout,
    >,
    text_theme: crate::eventmodeling::EventModelingTextThemePlan,
}

#[derive(Debug)]
pub(crate) struct IshikawaFamilyArtifact {
    pair: FamilyPair<diagrams::ishikawa::IshikawaDiagramRenderModel, IshikawaDiagramLayout>,
    text_theme: crate::ishikawa::IshikawaTextThemePlan,
}

impl IshikawaFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::ishikawa::IshikawaDiagramRenderModel, IshikawaDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn text_theme(&self) -> &crate::ishikawa::IshikawaTextThemePlan {
        &self.text_theme
    }
}

impl EventModelingFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<
        diagrams::eventmodeling::EventModelingDiagramRenderModel,
        EventModelingDiagramLayout,
    > {
        &self.pair
    }

    pub(crate) const fn text_theme(&self) -> &crate::eventmodeling::EventModelingTextThemePlan {
        &self.text_theme
    }
}

impl VennFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::venn::VennDiagramRenderModel, VennDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn title_theme(&self) -> &crate::venn::VennTitleThemePlan {
        &self.title_theme
    }

    pub(crate) const fn typography_theme(&self) -> &crate::venn::VennTypographyThemePlan {
        &self.typography_theme
    }

    fn finish_theme_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.title_theme.finish_evidence();
        evidence.merge_accounted_from(self.typography_theme.finish_evidence());
        evidence
    }
}

#[derive(Debug)]
pub(crate) struct BlockFamilyArtifact {
    pair: FamilyPair<diagrams::block::BlockDiagramRenderModel, BlockDiagramLayout>,
    node_paint_theme: crate::block::BlockNodePaintThemePlan,
    node_label_paint_theme: crate::block::BlockNodeLabelPaintPlan,
    edge_paint_theme: crate::block::BlockEdgePaintPlan,
    marker_paint_theme: crate::block::BlockMarkerPaintPlan,
    label_background_theme: crate::block::BlockLabelBackgroundPlan,
    typography_theme: crate::block::BlockTypographyThemePlan,
}

impl BlockFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::block::BlockDiagramRenderModel, BlockDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn node_paint_theme(&self) -> &crate::block::BlockNodePaintThemePlan {
        &self.node_paint_theme
    }

    pub(crate) const fn node_label_paint_theme(&self) -> &crate::block::BlockNodeLabelPaintPlan {
        &self.node_label_paint_theme
    }

    pub(crate) const fn edge_paint_theme(&self) -> &crate::block::BlockEdgePaintPlan {
        &self.edge_paint_theme
    }

    pub(crate) const fn marker_paint_theme(&self) -> &crate::block::BlockMarkerPaintPlan {
        &self.marker_paint_theme
    }

    pub(crate) const fn label_background_theme(&self) -> &crate::block::BlockLabelBackgroundPlan {
        &self.label_background_theme
    }

    pub(crate) const fn typography_theme(&self) -> &crate::block::BlockTypographyThemePlan {
        &self.typography_theme
    }

    fn finish_theme_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.node_paint_theme.finish_evidence();
        evidence.merge_accounted_from(self.node_label_paint_theme.finish_evidence());
        evidence.merge_accounted_from(self.edge_paint_theme.finish_evidence());
        evidence.merge_accounted_from(self.marker_paint_theme.finish_evidence());
        if self.label_background_theme.requested() {
            evidence.merge_accounted_from(self.label_background_theme.finish_evidence());
        }
        evidence.merge_accounted_from(self.typography_theme.finish_evidence());
        evidence
    }
}

#[derive(Debug)]
pub(crate) struct RailroadFamilyArtifact {
    pair: FamilyPair<diagrams::railroad::RailroadDiagramRenderModel, RailroadDiagramLayout>,
    typography_theme: crate::railroad::RailroadTypographyThemePlan,
}

impl RailroadFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::railroad::RailroadDiagramRenderModel, RailroadDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn typography_theme(&self) -> &crate::railroad::RailroadTypographyThemePlan {
        &self.typography_theme
    }
}

#[derive(Debug)]
pub(crate) struct ErrorFamilyArtifact {
    pair: FamilyPair<diagrams::error_diagram::ErrorDiagramRenderModel, ErrorDiagramLayout>,
    typography_theme: crate::error::ErrorTypographyThemePlan,
}

impl ErrorFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::error_diagram::ErrorDiagramRenderModel, ErrorDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn typography_theme(&self) -> &crate::error::ErrorTypographyThemePlan {
        &self.typography_theme
    }
}

#[derive(Debug)]
pub(crate) struct InfoFamilyArtifact {
    pair: FamilyPair<diagrams::info::InfoDiagramRenderModel, InfoDiagramLayout>,
    typography_theme: crate::info::InfoTypographyThemePlan,
}

impl InfoFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::info::InfoDiagramRenderModel, InfoDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn typography_theme(&self) -> &crate::info::InfoTypographyThemePlan {
        &self.typography_theme
    }
}

#[derive(Debug)]
pub(crate) struct CynefinFamilyArtifact {
    pair: FamilyPair<diagrams::cynefin::CynefinDiagramRenderModel, CynefinDiagramLayout>,
    typography_theme: crate::cynefin::CynefinTypographyThemePlan,
}

impl CynefinFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::cynefin::CynefinDiagramRenderModel, CynefinDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn typography_theme(&self) -> &crate::cynefin::CynefinTypographyThemePlan {
        &self.typography_theme
    }
}

#[derive(Debug)]
pub(crate) struct WardleyFamilyArtifact {
    pair: FamilyPair<diagrams::wardley::WardleyDiagramRenderModel, WardleyDiagramLayout>,
    typography_theme: crate::wardley::WardleyTypographyThemePlan,
}

impl WardleyFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::wardley::WardleyDiagramRenderModel, WardleyDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn typography_theme(&self) -> &crate::wardley::WardleyTypographyThemePlan {
        &self.typography_theme
    }
}

impl SankeyFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::sankey::SankeyDiagramRenderModel, SankeyDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn node_palette(&self) -> &crate::sankey::SankeyNodePalettePlan {
        &self.node_palette
    }

    pub(crate) const fn typography_theme(&self) -> &crate::sankey::SankeyTypographyThemePlan {
        &self.typography_theme
    }

    fn finish_theme_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.node_palette.finish_evidence();
        evidence.merge_accounted_from(self.typography_theme.finish_evidence());
        evidence
    }
}

#[derive(Debug)]
pub(crate) struct GitGraphFamilyArtifact {
    pair: FamilyPair<diagrams::git_graph::GitGraphRenderModel, GitGraphDiagramLayout>,
    node_palette: crate::gitgraph::GitGraphNodePalettePlan,
    static_paint: crate::gitgraph::GitGraphStaticPaintPlan,
    typography_theme: crate::gitgraph::GitGraphTypographyThemePlan,
}

impl GitGraphFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::git_graph::GitGraphRenderModel, GitGraphDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn node_palette(&self) -> &crate::gitgraph::GitGraphNodePalettePlan {
        &self.node_palette
    }

    pub(crate) const fn typography_theme(&self) -> &crate::gitgraph::GitGraphTypographyThemePlan {
        &self.typography_theme
    }

    pub(crate) const fn static_paint(&self) -> &crate::gitgraph::GitGraphStaticPaintPlan {
        &self.static_paint
    }

    fn finish_theme_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.node_palette.finish_evidence();
        evidence.merge_accounted_from(self.node_palette.text_paint().finish_evidence());
        evidence.merge_accounted_from(self.static_paint.finish_evidence());
        evidence.merge_accounted_from(self.typography_theme.finish_evidence());
        evidence
    }
}

#[derive(Debug)]
pub(crate) struct TreemapFamilyArtifact {
    pair: FamilyPair<diagrams::treemap::TreemapDiagramRenderModel, TreemapDiagramLayout>,
    title_theme: crate::treemap::TreemapTitleThemePlan,
    typography_theme: crate::treemap::TreemapTypographyThemePlan,
}

#[derive(Debug)]
pub(crate) struct RequirementFamilyArtifact {
    pair: FamilyPair<
        diagrams::requirement::RequirementDiagramRenderModel,
        crate::requirement::RequirementPreparedArtifact,
    >,
    paint_theme: crate::requirement::RequirementPaintThemePlan,
}

#[derive(Debug)]
pub(crate) struct PacketFamilyArtifact {
    pair: FamilyPair<diagrams::packet::PacketDiagramRenderModel, PacketDiagramLayout>,
    typography_theme: crate::packet::PacketTypographyThemePlan,
}

impl PacketFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::packet::PacketDiagramRenderModel, PacketDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn typography_theme(&self) -> &crate::packet::PacketTypographyThemePlan {
        &self.typography_theme
    }
}

impl RequirementFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<
        diagrams::requirement::RequirementDiagramRenderModel,
        crate::requirement::RequirementPreparedArtifact,
    > {
        &self.pair
    }

    pub(crate) const fn paint_theme(&self) -> &crate::requirement::RequirementPaintThemePlan {
        &self.paint_theme
    }
}

impl TreemapFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::treemap::TreemapDiagramRenderModel, TreemapDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn title_theme(&self) -> &crate::treemap::TreemapTitleThemePlan {
        &self.title_theme
    }

    pub(crate) const fn typography_theme(&self) -> &crate::treemap::TreemapTypographyThemePlan {
        &self.typography_theme
    }
}

impl RadarFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::radar::RadarDiagramRenderModel, RadarDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn series_paint(&self) -> &crate::radar::RadarSeriesPaintPlan {
        &self.series_paint
    }

    pub(crate) const fn text_paint(&self) -> &crate::radar::RadarTextPaintPlan {
        &self.text_paint
    }

    pub(crate) const fn axis_paint(&self) -> &crate::radar::RadarAxisPaintPlan {
        &self.axis_paint
    }

    pub(crate) const fn title_theme(&self) -> &crate::radar::RadarTitleThemePlan {
        &self.title_theme
    }

    pub(crate) const fn typography_theme(&self) -> &crate::radar::RadarTypographyThemePlan {
        &self.typography_theme
    }

    fn finish_theme_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.series_paint.finish_evidence();
        evidence.merge_accounted_from(self.title_theme.finish_evidence(&self.text_paint));
        evidence.merge_accounted_from(self.axis_paint.finish_evidence());
        evidence.merge_accounted_from(self.text_paint.finish_evidence());
        evidence.merge_accounted_from(self.typography_theme.finish_evidence());
        evidence
    }
}

impl XyChartFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::xychart::XyChartDiagramRenderModel, XyChartDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn series_paint(&self) -> &crate::xychart::XyChartSeriesPaintPlan {
        &self.series_paint
    }

    pub(crate) const fn paint_theme(&self) -> &crate::xychart::XyChartPaintPlan {
        &self.paint_theme
    }

    pub(crate) const fn typography_theme(&self) -> &crate::xychart::XyChartTypographyThemePlan {
        &self.typography_theme
    }
}

#[derive(Debug)]
pub(crate) struct TreeViewFamilyArtifact {
    pair: FamilyPair<diagrams::tree_view::TreeViewDiagramRenderModel, TreeViewDiagramLayout>,
    theme: crate::tree_view::TreeViewThemePlan,
}

#[derive(Debug)]
pub(crate) struct MindmapFamilyArtifact {
    pair: FamilyPair<diagrams::mindmap::MindmapDiagramRenderModel, MindmapDiagramLayout>,
    node_palette: crate::mindmap::MindmapNodePalettePlan,
}

impl MindmapFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::mindmap::MindmapDiagramRenderModel, MindmapDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn node_palette(&self) -> &crate::mindmap::MindmapNodePalettePlan {
        &self.node_palette
    }
}

#[derive(Debug)]
pub(crate) struct ErFamilyArtifact {
    pair: FamilyPair<diagrams::er::ErDiagramRenderModel, ErDiagramLayout>,
    entity_theme: crate::er::ErEntityThemePlan,
}

impl ErFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::er::ErDiagramRenderModel, ErDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn entity_theme(&self) -> &crate::er::ErEntityThemePlan {
        &self.entity_theme
    }
}

impl PieFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::pie::PieDiagramRenderModel, PieDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn theme(&self) -> &crate::pie::PieThemePlan {
        &self.theme
    }
}

impl TimelineFamilyArtifact {
    pub(crate) const fn text_paint(&self) -> &crate::timeline::TimelineTextPaintPlan {
        &self.text_paint
    }

    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::timeline::TimelineDiagramRenderModel, TimelineDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn event_theme(&self) -> &crate::timeline::TimelineEventTheme {
        &self.event_theme
    }

    pub(crate) const fn typography_theme(&self) -> &crate::timeline::TimelineTypographyThemePlan {
        &self.typography_theme
    }
}

impl TreeViewFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::tree_view::TreeViewDiagramRenderModel, TreeViewDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn theme(&self) -> &crate::tree_view::TreeViewThemePlan {
        &self.theme
    }
}

impl GanttFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<diagrams::gantt::GanttDiagramRenderModel, GanttDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn task_theme(&self) -> &crate::gantt::GanttTaskTheme {
        &self.task_theme
    }
}

#[derive(Debug)]
pub(crate) struct StateFamilyArtifact {
    pair: FamilyPair<diagrams::state::StateDiagramRenderModel, StateDiagramLayout>,
    label_sidecar: crate::state::StateLabelSidecar,
    effect_evidence: crate::state::StateSvgEffectEvidenceRecorder,
}

impl StateFamilyArtifact {
    pub(crate) fn pair(
        &self,
    ) -> &FamilyPair<diagrams::state::StateDiagramRenderModel, StateDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn label_sidecar(&self) -> &crate::state::StateLabelSidecar {
        &self.label_sidecar
    }

    pub(crate) const fn effect_evidence(&self) -> &crate::state::StateSvgEffectEvidenceRecorder {
        &self.effect_evidence
    }
}

#[cfg(feature = "layout-cytoscape")]
#[derive(Debug)]
pub(crate) struct ArchitectureFamilyArtifact {
    pair: FamilyPair<
        diagrams::architecture::ArchitectureDiagramRenderModel,
        ArchitectureDiagramLayout,
    >,
    group_theme: crate::architecture::ArchitectureGroupThemePlan,
}

#[cfg(feature = "layout-cytoscape")]
impl ArchitectureFamilyArtifact {
    pub(crate) const fn pair(
        &self,
    ) -> &FamilyPair<
        diagrams::architecture::ArchitectureDiagramRenderModel,
        ArchitectureDiagramLayout,
    > {
        &self.pair
    }

    pub(crate) const fn group_theme(&self) -> &crate::architecture::ArchitectureGroupThemePlan {
        &self.group_theme
    }
}

#[derive(Debug)]
pub(crate) enum BuiltinFamilyArtifact {
    Error(Box<ErrorFamilyArtifact>),
    Mindmap(Box<MindmapFamilyArtifact>),
    State(Box<StateFamilyArtifact>),
    Sequence(
        Box<
            FamilyPair<
                diagrams::sequence::SequenceDiagramRenderModel,
                crate::sequence::SequencePreparedArtifact,
            >,
        >,
    ),
    Zenuml(Box<ZenumlFamilyArtifact>),
    Flowchart(Box<FlowchartFamilyArtifact<FlowchartLayout>>),
    Swimlane(Box<FlowchartFamilyArtifact<SwimlaneLayout>>),
    #[cfg(feature = "layout-cytoscape")]
    Architecture(Box<ArchitectureFamilyArtifact>),
    Class(Box<ClassFamilyArtifact>),
    C4(Box<C4FamilyArtifact>),
    Cynefin(Box<CynefinFamilyArtifact>),
    Wardley(Box<WardleyFamilyArtifact>),
    Railroad(Box<RailroadFamilyArtifact>),
    Kanban(
        Box<
            FamilyPair<
                diagrams::kanban::KanbanDiagramRenderModel,
                crate::kanban::KanbanPreparedArtifact,
            >,
        >,
    ),
    Gantt(Box<GanttFamilyArtifact>),
    Pie(Box<PieFamilyArtifact>),
    Packet(Box<PacketFamilyArtifact>),
    Timeline(Box<TimelineFamilyArtifact>),
    Journey(Box<JourneyFamilyArtifact>),
    Requirement(Box<RequirementFamilyArtifact>),
    Sankey(Box<SankeyFamilyArtifact>),
    Radar(Box<RadarFamilyArtifact>),
    Info(Box<InfoFamilyArtifact>),
    Treemap(Box<TreemapFamilyArtifact>),
    Block(Box<BlockFamilyArtifact>),
    Er(Box<ErFamilyArtifact>),
    QuadrantChart(Box<QuadrantChartFamilyArtifact>),
    XyChart(Box<XyChartFamilyArtifact>),
    GitGraph(Box<GitGraphFamilyArtifact>),
    TreeView(Box<TreeViewFamilyArtifact>),
    Ishikawa(Box<IshikawaFamilyArtifact>),
    EventModeling(Box<EventModelingFamilyArtifact>),
    Venn(Box<VennFamilyArtifact>),
}

#[derive(serde::Serialize)]
enum LayoutProjection<'a> {
    BlockDiagram(&'a BlockDiagramLayout),
    RequirementDiagram(&'a RequirementDiagramLayout),
    #[cfg(feature = "layout-cytoscape")]
    ArchitectureDiagram(&'a ArchitectureDiagramLayout),
    MindmapDiagram(&'a MindmapDiagramLayout),
    SankeyDiagram(&'a SankeyDiagramLayout),
    RadarDiagram(&'a RadarDiagramLayout),
    TreemapDiagram(&'a TreemapDiagramLayout),
    VennDiagram(&'a VennDiagramLayout),
    XyChartDiagram(&'a XyChartDiagramLayout),
    QuadrantChartDiagram(&'a QuadrantChartDiagramLayout),
    #[serde(rename = "FlowchartV2")]
    Flowchart(&'a FlowchartLayout),
    SwimlaneDiagram(&'a SwimlaneLayout),
    #[serde(rename = "StateDiagramV2")]
    StateDiagram(&'a StateDiagramLayout),
    #[serde(rename = "ClassDiagramV2")]
    ClassDiagram(&'a ClassDiagramLayout),
    ErDiagram(&'a ErDiagramLayout),
    SequenceDiagram(&'a SequenceDiagramLayout),
    ZenumlDiagram(&'a crate::zenuml::ZenumlDiagramLayout),
    InfoDiagram(&'a InfoDiagramLayout),
    PacketDiagram(&'a PacketDiagramLayout),
    TimelineDiagram(&'a TimelineDiagramLayout),
    PieDiagram(&'a PieDiagramLayout),
    JourneyDiagram(&'a JourneyDiagramLayout),
    KanbanDiagram(&'a KanbanDiagramLayout),
    GitGraphDiagram(&'a GitGraphDiagramLayout),
    TreeViewDiagram(&'a TreeViewDiagramLayout),
    IshikawaDiagram(&'a IshikawaDiagramLayout),
    EventModelingDiagram(&'a EventModelingDiagramLayout),
    CynefinDiagram(&'a CynefinDiagramLayout),
    WardleyDiagram(&'a WardleyDiagramLayout),
    RailroadDiagram(&'a RailroadDiagramLayout),
    GanttDiagram(&'a GanttDiagramLayout),
    C4Diagram(&'a C4DiagramLayout),
    ErrorDiagram(&'a ErrorDiagramLayout),
}

fn clone_json_value_nonrecursive(value: &serde_json::Value) -> serde_json::Value {
    let mut cloned = rustc_hash::FxHashMap::default();
    let mut stack = vec![(value, false)];

    while let Some((current, visited)) = stack.pop() {
        let current_ptr = std::ptr::from_ref(current);
        if visited {
            let value = match current {
                serde_json::Value::Null => serde_json::Value::Null,
                serde_json::Value::Bool(value) => serde_json::Value::Bool(*value),
                serde_json::Value::Number(value) => serde_json::Value::Number(value.clone()),
                serde_json::Value::String(value) => serde_json::Value::String(value.clone()),
                serde_json::Value::Array(items) => serde_json::Value::Array(
                    items
                        .iter()
                        .filter_map(|item| cloned.remove(&std::ptr::from_ref(item)))
                        .collect(),
                ),
                serde_json::Value::Object(entries) => {
                    let mut object = serde_json::Map::new();
                    for (key, child) in entries {
                        if let Some(value) = cloned.remove(&std::ptr::from_ref(child)) {
                            object.insert(key.clone(), value);
                        }
                    }
                    serde_json::Value::Object(object)
                }
            };
            cloned.insert(current_ptr, value);
            continue;
        }

        stack.push((current, true));
        match current {
            serde_json::Value::Array(items) => {
                for item in items.iter().rev() {
                    stack.push((item, false));
                }
            }
            serde_json::Value::Object(entries) => {
                for child in entries.values().rev() {
                    stack.push((child, false));
                }
            }
            serde_json::Value::Null
            | serde_json::Value::Bool(_)
            | serde_json::Value::Number(_)
            | serde_json::Value::String(_) => {}
        }
    }

    cloned
        .remove(&std::ptr::from_ref(value))
        .unwrap_or(serde_json::Value::Null)
}

impl BuiltinFamilyArtifact {
    pub fn family_id(&self) -> DiagramFamilyId {
        match self {
            Self::Error(_) => DiagramFamilyId::ERROR,
            Self::Mindmap(_) => DiagramFamilyId::MINDMAP,
            Self::State(_) => DiagramFamilyId::STATE,
            Self::Sequence(_) => DiagramFamilyId::SEQUENCE,
            Self::Zenuml(_) => DiagramFamilyId::ZENUML,
            Self::Flowchart(_) => DiagramFamilyId::FLOWCHART,
            Self::Swimlane(_) => DiagramFamilyId::SWIMLANE,
            #[cfg(feature = "layout-cytoscape")]
            Self::Architecture(_) => DiagramFamilyId::ARCHITECTURE,
            Self::Class(_) => DiagramFamilyId::CLASS,
            Self::C4(_) => DiagramFamilyId::C4,
            Self::Cynefin(_) => DiagramFamilyId::CYNEFIN,
            Self::Wardley(_) => DiagramFamilyId::WARDLEY,
            Self::Railroad(_) => DiagramFamilyId::RAILROAD,
            Self::Kanban(_) => DiagramFamilyId::KANBAN,
            Self::Gantt(_) => DiagramFamilyId::GANTT,
            Self::Pie(_) => DiagramFamilyId::PIE,
            Self::Packet(_) => DiagramFamilyId::PACKET,
            Self::Timeline(_) => DiagramFamilyId::TIMELINE,
            Self::Journey(_) => DiagramFamilyId::JOURNEY,
            Self::Requirement(_) => DiagramFamilyId::REQUIREMENT,
            Self::Sankey(_) => DiagramFamilyId::SANKEY,
            Self::Radar(_) => DiagramFamilyId::RADAR,
            Self::Info(_) => DiagramFamilyId::INFO,
            Self::Treemap(_) => DiagramFamilyId::TREEMAP,
            Self::Block(_) => DiagramFamilyId::BLOCK,
            Self::Er(_) => DiagramFamilyId::ER,
            Self::QuadrantChart(_) => DiagramFamilyId::QUADRANT_CHART,
            Self::XyChart(_) => DiagramFamilyId::XY_CHART,
            Self::GitGraph(_) => DiagramFamilyId::GIT_GRAPH,
            Self::TreeView(_) => DiagramFamilyId::TREE_VIEW,
            Self::Ishikawa(_) => DiagramFamilyId::ISHIKAWA,
            Self::EventModeling(_) => DiagramFamilyId::EVENT_MODELING,
            Self::Venn(_) => DiagramFamilyId::VENN,
        }
    }

    fn prepared_text_label_ledger(&self) -> PreparedTextEvidenceLease {
        let (entries, retained_reservations) = match self {
            Self::Flowchart(artifact) => {
                let sidecar = artifact.svg_label_sidecar();
                (
                    sidecar
                        .prepared_text_label_ledger()
                        .cloned()
                        .collect::<Vec<_>>(),
                    sidecar.take_prepared_text_retained_reservations(),
                )
            }
            Self::Swimlane(artifact) => {
                let sidecar = artifact.svg_label_sidecar();
                (
                    sidecar
                        .prepared_text_label_ledger()
                        .cloned()
                        .collect::<Vec<_>>(),
                    sidecar.take_prepared_text_retained_reservations(),
                )
            }
            Self::State(artifact) => {
                let sidecar = artifact.label_sidecar();
                (
                    sidecar
                        .prepared_text_label_ledger()
                        .cloned()
                        .collect::<Vec<_>>(),
                    sidecar.take_prepared_text_retained_reservations(),
                )
            }
            Self::Sequence(artifact) => (
                artifact.layout.prepared_text_label_ledger(),
                artifact.layout.take_prepared_text_retained_reservations(),
            ),
            _ => (Vec::new(), Vec::new()),
        };
        PreparedTextEvidenceLease::new(entries, retained_reservations)
    }

    fn prepared_math_evidence(&self) -> PreparedMathEvidenceLease {
        match self {
            Self::Flowchart(artifact) => artifact.svg_label_sidecar().prepared_math_evidence(),
            Self::Swimlane(artifact) => artifact.svg_label_sidecar().prepared_math_evidence(),
            Self::Sequence(artifact) => artifact.layout.math_sidecar().prepared_math_evidence(),
            _ => PreparedMathEvidenceLease::default(),
        }
    }

    fn merge_theme_evidence(
        &self,
        metadata: &ParseMetadata,
        context: &mut FamilyRenderContext,
    ) -> Result<()> {
        let (family_id, evidence) = match self {
            Self::Flowchart(artifact) => {
                let (evidence, residuals) =
                    flowchart_artifact_theme_evidence(artifact, context.resolved_theme());
                context.merge_flowchart_evidence(evidence, residuals);
                return Ok(());
            }
            Self::Swimlane(artifact) => {
                let (evidence, residuals) =
                    flowchart_artifact_theme_evidence(artifact, context.resolved_theme());
                context.merge_flowchart_evidence(evidence, residuals);
                return Ok(());
            }
            Self::Sequence(pair) => {
                let evidence = pair
                    .layout()
                    .theme_evidence()
                    .finish(context.resolved_theme());
                context.merge_sequence_evidence(evidence);
                #[cfg(merman_internal_theme_acceptance)]
                if let Some(fact) = pair.layout().theme_evidence().raster_paint_binding_fact() {
                    context.record_theme_raster_paint_binding_fact(fact);
                }
                return Ok(());
            }
            // State reconciles its terminal and filter receipts in the output finalizer.
            Self::State(_) => return Ok(()),
            Self::Class(artifact) => {
                let mut evidence = artifact.theme_evidence().finish(
                    context.resolved_theme(),
                    artifact.relation_theme(),
                    context.session().work_meter().as_ref(),
                )?;
                evidence.merge_accounted_from(artifact.typography_theme().finish_evidence());
                (DiagramFamilyId::CLASS, evidence)
            }
            Self::Gantt(artifact) => (
                DiagramFamilyId::GANTT,
                artifact.task_theme().finish_evidence(),
            ),
            Self::Kanban(pair) => {
                let mut evidence = pair.layout().task_theme().finish_evidence();
                evidence.merge_accounted_from(pair.layout().text_paint().finish_evidence());
                (DiagramFamilyId::KANBAN, evidence)
            }
            Self::Pie(artifact) => (DiagramFamilyId::PIE, artifact.theme().finish_evidence()),
            Self::Timeline(artifact) => {
                let mut evidence = artifact.event_theme().finish_evidence();
                evidence.merge_accounted_from(artifact.typography_theme().finish_evidence());
                evidence.merge_accounted_from(artifact.text_paint().finish_evidence());
                (DiagramFamilyId::TIMELINE, evidence)
            }
            Self::Journey(artifact) => {
                let mut evidence = artifact.task_theme().finish_evidence();
                evidence.merge_accounted_from(artifact.text_paint().finish_evidence());
                evidence.merge_accounted_from(artifact.typography_theme().finish_evidence());
                (DiagramFamilyId::JOURNEY, evidence)
            }
            Self::QuadrantChart(artifact) => {
                let mut evidence = artifact.point_theme().finish_evidence();
                evidence.merge_accounted_from(artifact.text_paint().finish_evidence());
                (DiagramFamilyId::QUADRANT_CHART, evidence)
            }
            Self::XyChart(artifact) => {
                let mut evidence = artifact.series_paint().finish_evidence();
                evidence.merge_accounted_from(artifact.typography_theme().finish_evidence());
                evidence.merge_accounted_from(artifact.paint_theme().finish_evidence());
                (DiagramFamilyId::XY_CHART, evidence)
            }
            Self::Radar(artifact) => (DiagramFamilyId::RADAR, artifact.finish_theme_evidence()),
            Self::Sankey(artifact) => (DiagramFamilyId::SANKEY, artifact.finish_theme_evidence()),
            Self::Block(artifact) => (DiagramFamilyId::BLOCK, artifact.finish_theme_evidence()),
            Self::Railroad(artifact) => (
                DiagramFamilyId::RAILROAD,
                artifact.typography_theme().finish_evidence(),
            ),
            Self::Error(artifact) => (
                DiagramFamilyId::ERROR,
                artifact.typography_theme().finish_evidence(),
            ),
            Self::Info(artifact) => (
                DiagramFamilyId::INFO,
                artifact.typography_theme().finish_evidence(),
            ),
            Self::Cynefin(artifact) => (
                DiagramFamilyId::CYNEFIN,
                artifact.typography_theme().finish_evidence(),
            ),
            Self::Wardley(artifact) => (
                DiagramFamilyId::WARDLEY,
                artifact.typography_theme().finish_evidence(),
            ),
            Self::Treemap(artifact) => {
                let mut evidence = artifact.title_theme().finish_evidence();
                evidence.merge_accounted_from(artifact.typography_theme().finish_evidence());
                (DiagramFamilyId::TREEMAP, evidence)
            }
            Self::Requirement(artifact) => (
                DiagramFamilyId::REQUIREMENT,
                artifact.paint_theme().finish_evidence(),
            ),
            Self::Packet(artifact) => (
                DiagramFamilyId::PACKET,
                artifact.typography_theme().finish_evidence(),
            ),
            Self::C4(artifact) => {
                let mut evidence = artifact.cluster_theme().finish_evidence();
                evidence.merge_accounted_from(artifact.typography_theme().finish_evidence());
                evidence.merge_accounted_from(artifact.text_paint().finish_evidence());
                (DiagramFamilyId::C4, evidence)
            }
            Self::TreeView(artifact) => (
                DiagramFamilyId::TREE_VIEW,
                artifact.theme().finish_evidence(),
            ),
            Self::Mindmap(artifact) => (
                DiagramFamilyId::MINDMAP,
                artifact.node_palette().finish_evidence(),
            ),
            Self::GitGraph(artifact) => {
                (DiagramFamilyId::GIT_GRAPH, artifact.finish_theme_evidence())
            }
            Self::Er(artifact) => (
                DiagramFamilyId::ER,
                artifact.entity_theme().finish_evidence(),
            ),
            Self::Venn(artifact) => (DiagramFamilyId::VENN, artifact.finish_theme_evidence()),
            Self::Zenuml(artifact) => (
                DiagramFamilyId::ZENUML,
                artifact.title_theme().finish_evidence(),
            ),
            Self::EventModeling(artifact) => (
                DiagramFamilyId::EVENT_MODELING,
                artifact.text_theme().finish_evidence(),
            ),
            Self::Ishikawa(artifact) => (
                DiagramFamilyId::ISHIKAWA,
                artifact.text_theme().finish_evidence(),
            ),
            #[cfg(feature = "layout-cytoscape")]
            Self::Architecture(artifact) => (
                DiagramFamilyId::ARCHITECTURE,
                artifact.group_theme().finish_evidence(),
            ),
        };
        context.merge_accounted_terminal_evidence(family_id, evidence);
        if let Self::Packet(artifact) = self {
            let evidence = merman_core::__private::theme_parse_evidence(metadata);
            let consumptions = artifact
                .typography_theme()
                .terminal_mermaid_compatibility_consumptions(&evidence);
            context.reconcile_packet_mermaid_compatibility(&evidence, &consumptions);
        }
        #[cfg(all(merman_internal_theme_acceptance, feature = "layout-cytoscape"))]
        if let Self::Architecture(artifact) = self {
            context.style_plan.record_architecture_text_cutover_receipt(
                artifact.group_theme().architecture_text_cutover_receipt(),
            );
        }
        Ok(())
    }

    fn compatibility_json(
        &self,
        metadata: &ParseMetadata,
    ) -> merman_core::Result<serde_json::Value> {
        match self {
            Self::Error(artifact) => artifact.pair().compatibility_json(metadata),
            Self::Mindmap(artifact) => artifact.pair.compatibility_json(metadata),
            Self::State(artifact) => artifact.pair.compatibility_json(metadata),
            Self::Sequence(pair) => pair.compatibility_json(metadata),
            Self::Zenuml(artifact) => artifact.pair().compatibility_json(metadata),
            Self::Flowchart(artifact) => artifact.pair.compatibility_json(metadata),
            Self::Swimlane(artifact) => artifact.pair.compatibility_json(metadata),
            #[cfg(feature = "layout-cytoscape")]
            Self::Architecture(artifact) => artifact.pair.compatibility_json(metadata),
            Self::Class(artifact) => artifact.pair.compatibility_json(metadata),
            Self::C4(artifact) => artifact.pair.compatibility_json(metadata),
            Self::Cynefin(artifact) => artifact.pair().compatibility_json(metadata),
            Self::Wardley(artifact) => artifact.pair().compatibility_json(metadata),
            Self::Railroad(artifact) => artifact.pair().compatibility_json(metadata),
            Self::Kanban(pair) => pair.compatibility_json(metadata),
            Self::Gantt(artifact) => artifact.pair.compatibility_json(metadata),
            Self::Pie(artifact) => artifact.pair.compatibility_json(metadata),
            Self::Packet(artifact) => artifact.pair.compatibility_json(metadata),
            Self::Timeline(artifact) => artifact.pair.compatibility_json(metadata),
            Self::Journey(artifact) => artifact.pair.compatibility_json(metadata),
            Self::Requirement(artifact) => artifact.pair.compatibility_json(metadata),
            Self::Sankey(artifact) => artifact.pair().compatibility_json(metadata),
            Self::Radar(artifact) => artifact.pair.compatibility_json(metadata),
            Self::Info(artifact) => artifact.pair().compatibility_json(metadata),
            Self::Treemap(artifact) => artifact.pair.compatibility_json(metadata),
            Self::Block(artifact) => artifact.pair().compatibility_json(metadata),
            Self::Er(artifact) => artifact.pair.compatibility_json(metadata),
            Self::QuadrantChart(artifact) => artifact.pair.compatibility_json(metadata),
            Self::XyChart(artifact) => artifact.pair.compatibility_json(metadata),
            Self::GitGraph(artifact) => artifact.pair.compatibility_json(metadata),
            Self::TreeView(artifact) => artifact.pair.compatibility_json(metadata),
            Self::Ishikawa(artifact) => artifact.pair().compatibility_json(metadata),
            Self::EventModeling(artifact) => artifact.pair().compatibility_json(metadata),
            Self::Venn(artifact) => artifact.pair().compatibility_json(metadata),
        }
    }

    fn layout_projection(&self) -> LayoutProjection<'_> {
        match self {
            Self::Error(artifact) => LayoutProjection::ErrorDiagram(artifact.pair().layout()),
            Self::Mindmap(artifact) => LayoutProjection::MindmapDiagram(artifact.pair.layout()),
            Self::State(artifact) => LayoutProjection::StateDiagram(artifact.pair.layout()),
            Self::Sequence(pair) => LayoutProjection::SequenceDiagram(pair.layout().layout()),
            Self::Zenuml(artifact) => LayoutProjection::ZenumlDiagram(artifact.pair().layout()),
            Self::Flowchart(artifact) => LayoutProjection::Flowchart(artifact.pair.layout()),
            Self::Swimlane(artifact) => LayoutProjection::SwimlaneDiagram(artifact.pair.layout()),
            #[cfg(feature = "layout-cytoscape")]
            Self::Architecture(artifact) => {
                LayoutProjection::ArchitectureDiagram(artifact.pair.layout())
            }
            Self::Class(artifact) => LayoutProjection::ClassDiagram(artifact.pair.layout()),
            Self::C4(artifact) => LayoutProjection::C4Diagram(artifact.pair.layout()),
            Self::Cynefin(artifact) => LayoutProjection::CynefinDiagram(artifact.pair().layout()),
            Self::Wardley(artifact) => LayoutProjection::WardleyDiagram(artifact.pair().layout()),
            Self::Railroad(artifact) => LayoutProjection::RailroadDiagram(artifact.pair().layout()),
            Self::Kanban(pair) => LayoutProjection::KanbanDiagram(pair.layout().layout()),
            Self::Gantt(artifact) => LayoutProjection::GanttDiagram(artifact.pair.layout()),
            Self::Pie(artifact) => LayoutProjection::PieDiagram(artifact.pair.layout()),
            Self::Packet(artifact) => LayoutProjection::PacketDiagram(artifact.pair.layout()),
            Self::Timeline(artifact) => LayoutProjection::TimelineDiagram(artifact.pair.layout()),
            Self::Journey(artifact) => LayoutProjection::JourneyDiagram(artifact.pair.layout()),
            Self::Requirement(artifact) => {
                LayoutProjection::RequirementDiagram(artifact.pair.layout().layout())
            }
            Self::Sankey(artifact) => LayoutProjection::SankeyDiagram(artifact.pair().layout()),
            Self::Radar(artifact) => LayoutProjection::RadarDiagram(artifact.pair.layout()),
            Self::Info(artifact) => LayoutProjection::InfoDiagram(artifact.pair().layout()),
            Self::Treemap(artifact) => LayoutProjection::TreemapDiagram(artifact.pair.layout()),
            Self::Block(artifact) => LayoutProjection::BlockDiagram(artifact.pair().layout()),
            Self::Er(artifact) => LayoutProjection::ErDiagram(artifact.pair.layout()),
            Self::QuadrantChart(artifact) => {
                LayoutProjection::QuadrantChartDiagram(artifact.pair.layout())
            }
            Self::XyChart(artifact) => LayoutProjection::XyChartDiagram(artifact.pair.layout()),
            Self::GitGraph(artifact) => LayoutProjection::GitGraphDiagram(artifact.pair.layout()),
            Self::TreeView(artifact) => LayoutProjection::TreeViewDiagram(artifact.pair.layout()),
            Self::Ishikawa(artifact) => LayoutProjection::IshikawaDiagram(artifact.pair().layout()),
            Self::EventModeling(artifact) => {
                LayoutProjection::EventModelingDiagram(artifact.pair().layout())
            }
            Self::Venn(artifact) => LayoutProjection::VennDiagram(artifact.pair().layout()),
        }
    }
}

pub struct FamilyRenderArtifact {
    metadata: ParseMetadata,
    compatibility_projection: OnceLock<std::result::Result<serde_json::Value, String>>,
    family: BuiltinFamilyArtifact,
    required_capabilities: Vec<RenderCapability>,
    context: FamilyRenderContext,
}

/// Owned projection of the Gantt time scale used by comparison tooling.
///
/// The projection deliberately hides the full family layout. Its inverse uses the same rounded
/// pixel mapping as the renderer, so coordinates that fall between representable task times return
/// `None` instead of inventing a timestamp.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GanttTimeAxisDiagnostics {
    min_ms: i64,
    max_ms: i64,
    left_x: f64,
    drawable_width: f64,
}

impl GanttTimeAxisDiagnostics {
    fn from_layout(layout: &GanttDiagramLayout) -> Option<Self> {
        let min_ms = layout.tasks.iter().map(|task| task.start_ms).min()?;
        let max_ms = layout.tasks.iter().map(|task| task.end_ms).max()?;
        if max_ms <= min_ms {
            return None;
        }

        let left_x = layout.left_padding;
        let drawable_width = (layout.width - layout.left_padding - layout.right_padding).max(1.0);
        if !left_x.is_finite() || !drawable_width.is_finite() {
            return None;
        }

        Some(Self {
            min_ms,
            max_ms,
            left_x,
            drawable_width,
        })
    }

    /// Resolves an exact rendered x coordinate back to a Unix timestamp in milliseconds.
    pub fn unix_millis_at_rendered_x(&self, target_x: f64) -> Option<i64> {
        if !target_x.is_finite() {
            return None;
        }

        let span_ms = (i128::from(self.max_ms) - i128::from(self.min_ms)) as f64;
        let scaled_x = target_x - self.left_x;
        if !span_ms.is_finite() || !scaled_x.is_finite() {
            return None;
        }

        let estimate = self.min_ms as f64 + span_ms * (scaled_x / self.drawable_width);
        if !estimate.is_finite() {
            return None;
        }

        let mut lo = estimate.round() as i64;
        let mut hi = lo;
        let mut step = 1_i64;
        for _ in 0..80 {
            if self.rendered_x(lo)? <= target_x {
                break;
            }
            hi = lo;
            lo = lo.saturating_sub(step);
            step = step.saturating_mul(2);
        }

        step = 1;
        for _ in 0..80 {
            if self.rendered_x(hi)? >= target_x {
                break;
            }
            lo = hi;
            hi = hi.saturating_add(step);
            step = step.saturating_mul(2);
        }

        let lo_x = self.rendered_x(lo)?;
        let hi_x = self.rendered_x(hi)?;
        if !(lo_x <= target_x && target_x <= hi_x) {
            return None;
        }

        while lo < hi {
            let half_distance = ((i128::from(hi) - i128::from(lo)) / 2) as i64;
            let mid = lo + half_distance;
            if self.rendered_x(mid)? < target_x {
                lo = mid.saturating_add(1);
            } else {
                hi = mid;
            }
        }

        (self.rendered_x(lo)? == target_x).then_some(lo)
    }

    fn rendered_x(&self, unix_millis: i64) -> Option<f64> {
        let offset_ms = (i128::from(unix_millis) - i128::from(self.min_ms)) as f64;
        let span_ms = (i128::from(self.max_ms) - i128::from(self.min_ms)) as f64;
        let x = self.left_x + (offset_ms / span_ms * self.drawable_width).round();
        x.is_finite().then_some(x)
    }
}

/// A completed family SVG produced by the canonical typed render operation.
///
/// Root completion evidence is private to the renderer and cannot be named by callers:
///
/// ```compile_fail
/// use merman_render::svg::RootedSvg;
/// ```
///
/// A raw string cannot be substituted for a completed family SVG:
///
/// ```compile_fail
/// use merman_render::family::RenderedFamilySvg;
///
/// let forged: RenderedFamilySvg = String::from("<svg xmlns=\"http://www.w3.org/2000/svg\"/>");
/// ```
pub struct RenderedFamilySvg {
    svg: String,
    prepared_text_svg: Option<String>,
    prepared_text_ledger: PreparedTextEvidenceLease,
    prepared_text_evidence_valid: bool,
    prepared_math_evidence: PreparedMathEvidenceLease,
    prepared_math_evidence_valid: bool,
    root_theme: RootThemeReport,
    style_report: FamilyStyleReport,
    metadata: ParseMetadata,
    required_capabilities: Vec<RenderCapability>,
    session: RenderSession,
}

/// Opaque output paired with the family report frozen after all requested SVG work.
///
/// The live [`RenderSession`] remains in pre-completion SVG types so pipelines can record their
/// measurements. Converting to this type replaces that mutable session with [`RenderSessionReport`].
/// Completion therefore exposes no route for recording more measurements:
///
/// ```compile_fail
/// use merman_render::family::FamilyRenderCompletion;
///
/// fn measure_after_completion(completion: FamilyRenderCompletion<String>) {
///     let _ = completion.session();
/// }
/// ```
///
/// An output and a report from separate renders cannot be assembled into a completion:
///
/// ```compile_fail
/// use merman_render::family::{FamilyRenderCompletion, FamilyRenderReport};
/// use merman_render::svg::ResvgCompatibleSvg;
///
/// fn rebind(output: ResvgCompatibleSvg, report: FamilyRenderReport) {
///     let _ = FamilyRenderCompletion { output, report };
/// }
/// ```
#[must_use = "completed render output and its frozen report should be consumed together"]
pub struct FamilyRenderCompletion<T> {
    output: T,
    report: FamilyRenderReport,
}

impl<T> FamilyRenderCompletion<T> {
    pub const fn output(&self) -> &T {
        &self.output
    }

    pub const fn report(&self) -> &FamilyRenderReport {
        &self.report
    }

    pub fn into_output_and_report(self) -> (T, FamilyRenderReport) {
        (self.output, self.report)
    }
}

#[cfg(merman_internal_theme_acceptance)]
impl FamilyRenderCompletion<ResvgCompatibleSvg> {
    /// Returns route receipts bound to this completion's frozen report and SVG representations.
    ///
    /// Callers cannot supply a different artifact or digest when sealing these facts.
    #[doc(hidden)]
    pub fn theme_route_cutover_receipts(&self) -> Vec<crate::__private::ThemeRouteCutoverReceipt> {
        use sha2::{Digest as _, Sha256};

        if !self.report.style_report().is_verified() {
            return Vec::new();
        }
        crate::theme_route_cutover::seal_theme_route_cutover_receipts(
            self.report.theme_route_cutover_facts(),
            Sha256::digest(self.output.as_str().as_bytes()).into(),
            Sha256::digest(self.output.native_export_svg().as_bytes()).into(),
        )
    }

    /// Returns paint bindings sealed against this completion's finalized native SVG.
    #[doc(hidden)]
    pub fn theme_raster_paint_binding_receipts(
        &self,
    ) -> Vec<crate::__private::ThemeRasterPaintBindingReceipt> {
        use sha2::{Digest as _, Sha256};

        if !self.report.style_report().is_verified() {
            return Vec::new();
        }
        crate::theme_raster_paint::seal_theme_raster_paint_binding_receipts(
            self.report.theme_raster_paint_binding_facts(),
            Sha256::digest(self.output.native_export_svg().as_bytes()).into(),
        )
    }
}

impl RenderedFamilySvg {
    pub fn svg(&self) -> &str {
        &self.svg
    }

    pub fn metadata(&self) -> &ParseMetadata {
        &self.metadata
    }

    pub fn family_id(&self) -> DiagramFamilyId {
        self.style_report.family_id()
    }

    /// Returns the optional capabilities admitted during this artifact's preparation.
    pub fn required_capabilities(&self) -> &[RenderCapability] {
        &self.required_capabilities
    }

    #[cfg(test)]
    pub(crate) const fn style_report(&self) -> &FamilyStyleReport {
        &self.style_report
    }

    #[cfg(test)]
    pub(crate) const fn root_theme_report(&self) -> &RootThemeReport {
        &self.root_theme
    }

    /// Applies an output pipeline while retaining the renderer-owned family capability.
    pub fn apply_pipeline(self, pipeline: &SvgPipeline) -> Result<Self> {
        self.apply_pipeline_with_portability(pipeline, true)
    }

    fn apply_pipeline_with_portability(
        mut self,
        pipeline: &SvgPipeline,
        enforce_portability: bool,
    ) -> Result<Self> {
        self.session.checkpoint(OperationPhase::Postprocess)?;
        let output_metadata = self.output_metadata()?;
        let preserves_prepared_text =
            self.prepared_text_evidence_valid && pipeline.preserves_prepared_text_evidence();
        let requires_prepared_math_projection = pipeline.requires_prepared_math_projection();
        let preserves_prepared_math =
            self.prepared_math_evidence_valid && pipeline.preserves_prepared_math_evidence();
        let supplies_prepared_math = self.prepared_math_evidence_valid
            && (preserves_prepared_math || requires_prepared_math_projection);
        let source_svg = if preserves_prepared_text {
            self.prepared_text_svg
                .take()
                .unwrap_or_else(|| std::mem::take(&mut self.svg))
        } else {
            self.prepared_text_svg = None;
            self.prepared_text_evidence_valid = false;
            self.prepared_text_ledger = PreparedTextEvidenceLease::default();
            std::mem::take(&mut self.svg)
        };
        let processed_svg = pipeline.process_owned_to_string_with_metadata_and_math_evidence(
            source_svg,
            &output_metadata,
            &self.session,
            supplies_prepared_math.then_some(&self.prepared_math_evidence),
        )?;
        if preserves_prepared_text && !self.prepared_text_ledger.is_empty() {
            let (public_svg, prepared_text_svg) = crate::svg::partition_prepared_text_label_ids(
                processed_svg,
                self.prepared_text_ledger.entries(),
                SvgPostprocessExecution::new(&self.session),
            )?;
            self.svg = public_svg;
            self.prepared_text_svg = prepared_text_svg;
        } else {
            self.svg = processed_svg;
        }
        if !preserves_prepared_math {
            self.prepared_math_evidence_valid = false;
        }
        self.session.work_meter().preflight_svg_byte_count(
            self.svg.len(),
            ResourceLimitPhase::SvgPostprocess,
            OperationPhase::Postprocess,
        )?;
        if !pipeline.preserves_typed_theme_evidence() {
            self.root_theme = self.root_theme.invalidate_for_output_mutation();
            self.style_report = self.style_report.invalidate_for_output_mutation();
        }
        if enforce_portability {
            ensure_root_theme_portable(
                &self.root_theme,
                self.session
                    .theme_portability_requirement()
                    .unwrap_or(ThemePortabilityRequirement::BestEffort),
            )?;
            if self.session.theme_portability_requirement()
                == Some(ThemePortabilityRequirement::RequirePortable)
            {
                self.style_report.ensure_portable()?;
            }
        }
        self.session.checkpoint(OperationPhase::Postprocess)?;
        Ok(self)
    }

    /// Finalizes the typed family output for resvg/raster consumption.
    pub fn finalize_resvg(self, pipeline: &SvgPipeline) -> Result<RenderedResvgCompatibleSvg> {
        self.finalize_resvg_with_portability(pipeline, true)
    }

    fn finalize_resvg_with_portability(
        self,
        pipeline: &SvgPipeline,
        enforce_portability: bool,
    ) -> Result<RenderedResvgCompatibleSvg> {
        self.session.checkpoint(OperationPhase::Export)?;
        let portability = self
            .session
            .theme_portability_requirement()
            .unwrap_or(ThemePortabilityRequirement::BestEffort);
        let output_metadata = self.output_metadata()?;
        let prepared_text_evidence_valid =
            self.prepared_text_evidence_valid && pipeline.preserves_prepared_text_evidence();
        let source_svg = if prepared_text_evidence_valid {
            self.prepared_text_svg.unwrap_or(self.svg)
        } else {
            self.svg
        };
        let prepared_text_ledger = if prepared_text_evidence_valid {
            self.prepared_text_ledger
        } else {
            PreparedTextEvidenceLease::default()
        };
        let prepared_math_evidence_valid =
            self.prepared_math_evidence_valid && pipeline.preserves_prepared_math_evidence();
        if !self.prepared_math_evidence.is_empty() && !prepared_math_evidence_valid {
            return Err(Error::svg_postprocess(
                "prepared-math-terminal-receipt",
                "renderer-owned prepared-math evidence was invalidated before native projection",
            ));
        }
        let prepared_math_evidence = if prepared_math_evidence_valid {
            self.prepared_math_evidence
        } else {
            PreparedMathEvidenceLease::default()
        };
        let svg = pipeline
            .process_owned_resvg_compatible_with_metadata_and_math_evidence(
                source_svg,
                &output_metadata,
                &self.session,
                prepared_math_evidence_valid.then_some(&prepared_math_evidence),
            )?
            .attach_prepared_text_evidence(
                prepared_text_ledger,
                prepared_text_evidence_valid,
                SvgPostprocessExecution::new(&self.session),
            )?
            .attach_prepared_math_evidence(prepared_math_evidence, prepared_math_evidence_valid)?;
        self.session.work_meter().preflight_svg_byte_count(
            svg.as_str().len(),
            ResourceLimitPhase::SvgPostprocess,
            OperationPhase::Export,
        )?;
        let preserves_typed_theme_evidence = pipeline.preserves_typed_theme_evidence();
        let root_theme = if preserves_typed_theme_evidence {
            self.root_theme
        } else {
            self.root_theme.invalidate_for_output_mutation()
        };
        let style_report = if preserves_typed_theme_evidence {
            self.style_report
        } else {
            self.style_report.invalidate_for_output_mutation()
        };
        if enforce_portability {
            ensure_root_theme_portable(&root_theme, portability)?;
            if portability == ThemePortabilityRequirement::RequirePortable {
                style_report.ensure_portable()?;
            }
        }
        self.session.checkpoint(OperationPhase::Export)?;
        Ok(RenderedResvgCompatibleSvg {
            svg,
            root_theme,
            style_report,
            session: self.session,
        })
    }

    /// Completes the exact artifact selected for the standalone SVG target.
    ///
    /// Resvg-safe output retains the full renderer-owned terminal capability. Other pipelines are
    /// validated in place instead of being normalized into a different artifact.
    pub fn finalize_standalone(
        self,
        pipeline: Option<&SvgPipeline>,
    ) -> Result<RenderedStandaloneSvg> {
        self.finalize_standalone_with_portability(pipeline, true)
    }

    pub(crate) fn finalize_standalone_for_target_admission(
        self,
        pipeline: Option<&SvgPipeline>,
    ) -> Result<RenderedStandaloneSvg> {
        self.finalize_standalone_with_portability(pipeline, false)
    }

    fn finalize_standalone_with_portability(
        self,
        pipeline: Option<&SvgPipeline>,
        enforce_portability: bool,
    ) -> Result<RenderedStandaloneSvg> {
        match pipeline {
            Some(pipeline) if pipeline.preset() == SvgPipelinePreset::ResvgSafe => {
                let finalized =
                    self.finalize_resvg_with_portability(pipeline, enforce_portability)?;
                Ok(finalized.into_standalone())
            }
            Some(pipeline) => self
                .apply_pipeline_with_portability(pipeline, enforce_portability)?
                .finalize_observed_standalone(pipeline),
            None => {
                let pipeline = SvgPipeline::parity();
                self.finalize_observed_standalone(&pipeline)
            }
        }
    }

    fn finalize_observed_standalone(self, pipeline: &SvgPipeline) -> Result<RenderedStandaloneSvg> {
        self.session.checkpoint(OperationPhase::Postprocess)?;
        let artifact = StandaloneSvgArtifact::observe_exact(
            self.svg,
            self.prepared_text_svg,
            self.prepared_text_ledger,
            self.prepared_text_evidence_valid,
            pipeline,
            &self.session,
        )?;
        self.session.checkpoint(OperationPhase::Postprocess)?;
        Ok(RenderedStandaloneSvg {
            artifact,
            root_theme: self.root_theme,
            style_report: self.style_report,
            session: self.session,
        })
    }

    fn output_metadata(&self) -> Result<SvgPostprocessMetadata> {
        let metadata = SvgPostprocessMetadata::from_svg_with_execution(
            &self.svg,
            SvgPostprocessExecution::new(&self.session),
        )?;
        Ok(metadata
            .with_family_id(self.family_id())
            .with_diagram_type(self.metadata.diagram_type.clone())
            .with_optional_diagram_title(self.metadata.title.clone()))
    }

    pub fn into_completion(self) -> FamilyRenderCompletion<String> {
        let Self {
            svg,
            prepared_text_svg: _,
            prepared_text_ledger: _,
            prepared_text_evidence_valid: _,
            prepared_math_evidence: _,
            prepared_math_evidence_valid: _,
            root_theme,
            style_report,
            session,
            metadata: _,
            required_capabilities: _,
        } = self;
        FamilyRenderCompletion {
            output: svg,
            report: FamilyRenderReport::freeze(root_theme, style_report, session),
        }
    }
}

/// Renderer-owned family output after the terminal resvg compatibility finalizer.
pub struct RenderedResvgCompatibleSvg {
    svg: ResvgCompatibleSvg,
    root_theme: RootThemeReport,
    style_report: FamilyStyleReport,
    session: RenderSession,
}

impl RenderedResvgCompatibleSvg {
    pub fn svg(&self) -> &ResvgCompatibleSvg {
        &self.svg
    }

    pub const fn family_id(&self) -> DiagramFamilyId {
        self.style_report.family_id()
    }

    #[cfg(test)]
    pub(crate) const fn style_report(&self) -> &FamilyStyleReport {
        &self.style_report
    }

    #[cfg(test)]
    pub(crate) const fn root_theme_report(&self) -> &RootThemeReport {
        &self.root_theme
    }

    pub fn into_completion(self) -> FamilyRenderCompletion<ResvgCompatibleSvg> {
        FamilyRenderCompletion {
            output: self.svg,
            report: FamilyRenderReport::freeze(self.root_theme, self.style_report, self.session),
        }
    }

    fn into_standalone(self) -> RenderedStandaloneSvg {
        RenderedStandaloneSvg {
            artifact: StandaloneSvgArtifact::from_resvg_compatible(self.svg),
            root_theme: self.root_theme,
            style_report: self.style_report,
            session: self.session,
        }
    }
}

/// Renderer-owned family output paired with the exact standalone SVG terminal artifact.
pub struct RenderedStandaloneSvg {
    artifact: StandaloneSvgArtifact,
    root_theme: RootThemeReport,
    style_report: FamilyStyleReport,
    session: RenderSession,
}

impl RenderedStandaloneSvg {
    pub fn artifact(&self) -> &StandaloneSvgArtifact {
        &self.artifact
    }

    pub const fn family_id(&self) -> DiagramFamilyId {
        self.style_report.family_id()
    }

    pub fn into_completion(self) -> FamilyRenderCompletion<StandaloneSvgArtifact> {
        FamilyRenderCompletion {
            output: self.artifact,
            report: FamilyRenderReport::freeze(self.root_theme, self.style_report, self.session),
        }
    }
}

impl FamilyRenderArtifact {
    fn new(
        metadata: ParseMetadata,
        family: BuiltinFamilyArtifact,
        context: FamilyRenderContext,
    ) -> Result<Self> {
        let actual_family = family.family_id();
        let expected_family = context.family_id();
        if actual_family != expected_family {
            return Err(Error::InvalidModel {
                message: format!(
                    "planned render family {expected_family} produced {actual_family} artifact"
                ),
            });
        }

        Ok(Self {
            metadata,
            compatibility_projection: OnceLock::new(),
            family,
            required_capabilities: Vec::new(),
            context,
        })
    }

    pub fn metadata(&self) -> &ParseMetadata {
        &self.metadata
    }

    pub fn family_id(&self) -> DiagramFamilyId {
        self.family.family_id()
    }

    pub fn gantt_time_axis_diagnostics(&self) -> Option<GanttTimeAxisDiagnostics> {
        let BuiltinFamilyArtifact::Gantt(artifact) = &self.family else {
            return None;
        };
        GanttTimeAxisDiagnostics::from_layout(artifact.pair().layout())
    }

    /// Builds the public layout projection from a compatibility JSON value produced by the
    /// operation's semantic artifact.
    ///
    /// The workspace facade computes compatibility JSON while it still owns the caller's
    /// [`merman_core::OperationControl`]. Keeping that projection outside this renderer artifact
    /// prevents a second, uncontrolled family projection and ensures semantic projection failures
    /// retain the core error category at the facade boundary. The value is intentionally supplied
    /// by the caller rather than revalidated here: it is paired with this artifact by the
    /// canonical parse operation.
    #[doc(hidden)]
    pub fn layout_json_with_compatibility_json(
        &self,
        semantic: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.context.session().checkpoint(OperationPhase::Emit)?;
        self.assemble_layout_json(semantic)
    }

    pub fn layout_json(&self) -> Result<serde_json::Value> {
        self.context.session().checkpoint(OperationPhase::Emit)?;
        let semantic = self
            .compatibility_projection
            .get_or_init(|| {
                self.family
                    .compatibility_json(&self.metadata)
                    .map_err(|error| {
                        format!(
                            "failed to project {} compatibility JSON: {error}",
                            self.family.family_id()
                        )
                    })
            })
            .as_ref()
            .map_err(|message| Error::InvalidModel {
                message: message.clone(),
            })
            .map(clone_json_value_nonrecursive)?;
        self.assemble_layout_json(semantic)
    }

    fn assemble_layout_json(&self, semantic: serde_json::Value) -> Result<serde_json::Value> {
        let layout = serde_json::to_value(self.family.layout_projection())?;

        let mut metadata = serde_json::Map::new();
        metadata.insert(
            "diagram_type".to_string(),
            serde_json::Value::String(self.metadata.diagram_type.clone()),
        );
        metadata.insert(
            "title".to_string(),
            self.metadata
                .title
                .as_ref()
                .map_or(serde_json::Value::Null, |title| {
                    serde_json::Value::String(title.clone())
                }),
        );
        metadata.insert(
            "config".to_string(),
            clone_json_value_nonrecursive(self.metadata.config.as_value()),
        );
        metadata.insert(
            "effective_config".to_string(),
            clone_json_value_nonrecursive(self.metadata.effective_config.as_value()),
        );

        let mut projection = serde_json::Map::new();
        projection.insert("meta".to_string(), serde_json::Value::Object(metadata));
        projection.insert("semantic".to_string(), semantic);
        projection.insert("layout".to_string(), layout);
        self.context.session().checkpoint(OperationPhase::Emit)?;
        Ok(serde_json::Value::Object(projection))
    }

    pub fn render_svg(
        self,
        options: &SvgRenderOptions,
        debug: &SvgDebugOptions,
    ) -> Result<RenderedFamilySvg> {
        self.context.session().checkpoint(OperationPhase::Emit)?;
        let trace_stage = debug.flowchart_edge_trace().map(|(edge_id, destination)| {
            let staging = FlowchartEdgeTraceCollector::default();
            let staged_debug = debug
                .clone()
                .with_flowchart_edge_trace(edge_id.to_owned(), staging.clone());
            (destination.clone(), staging, staged_debug)
        });
        let render_debug = trace_stage
            .as_ref()
            .map_or(debug, |(_, _, staged_debug)| staged_debug);
        let rendered = render_family_artifact_svg(&self, options, render_debug)?;
        admit_rendered_svg_output(self.context.session(), rendered.as_str())?;
        self.context.session().checkpoint(OperationPhase::Emit)?;
        let state_filter_receipt = match &self.family {
            BuiltinFamilyArtifact::State(artifact) => Some(artifact.effect_evidence().finish()),
            _ => None,
        };
        let prepared_text_ledger = self.family.prepared_text_label_ledger();
        let prepared_math_evidence = self.family.prepared_math_evidence();
        let Self {
            metadata,
            compatibility_projection: _,
            family,
            required_capabilities,
            mut context,
        } = self;
        family.merge_theme_evidence(&metadata, &mut context)?;
        if context.family_id() == DiagramFamilyId::STATE {
            context.reconcile_state_terminal_evidence();
        }
        context.observe_output_visibility(debug);
        if let Some(emitted) = state_filter_receipt {
            context.reconcile_state_effect_evidence(emitted);
        }
        let (tokenized_svg, root_theme, preserves_typed_theme_evidence) = rendered.into_parts();
        if !preserves_typed_theme_evidence {
            context.invalidate_for_output_mutation();
        }
        context.ensure_portable()?;
        let (svg, prepared_text_svg) = crate::svg::partition_prepared_text_label_ids(
            tokenized_svg,
            prepared_text_ledger.entries(),
            SvgPostprocessExecution::new(context.session()),
        )?;
        let (session, style_report) = context.into_session_and_style_report();
        ensure_root_theme_portable(
            &root_theme,
            session
                .theme_portability_requirement()
                .unwrap_or(ThemePortabilityRequirement::BestEffort),
        )?;
        session.checkpoint(OperationPhase::Emit)?;

        if let Some((destination, staging, _)) = trace_stage {
            for trace in staging.drain() {
                destination.record(trace);
            }
        }

        Ok(RenderedFamilySvg {
            svg,
            prepared_text_svg,
            prepared_text_ledger,
            prepared_text_evidence_valid: true,
            prepared_math_evidence,
            prepared_math_evidence_valid: true,
            root_theme,
            style_report,
            metadata,
            required_capabilities,
            session,
        })
    }
}

fn admit_rendered_svg_output(session: &RenderSession, svg: &str) -> Result<()> {
    // Termination wins over the final output ceiling when both become observable during emit.
    session.checkpoint(OperationPhase::Emit)?;
    session.work_meter().preflight_svg_byte_count(
        svg.len(),
        ResourceLimitPhase::SvgOutput,
        OperationPhase::Emit,
    )?;
    Ok(())
}

#[inline(never)]
fn render_family_artifact_svg(
    artifact: &FamilyRenderArtifact,
    request: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<crate::svg::RootThemeAppliedSvg> {
    let options = crate::svg::normalize_svg_render_options(request, artifact.context.session())?;
    let execution = artifact.context.execution();
    #[cfg(feature = "layout-cytoscape")]
    if let BuiltinFamilyArtifact::Architecture(architecture) = &artifact.family {
        return crate::svg::render_architecture_family_artifact(
            architecture,
            &artifact.metadata.effective_config,
            execution,
            &options,
            debug,
        );
    }
    crate::svg::render_builtin_family_artifact(
        &artifact.family,
        &artifact.metadata,
        execution,
        &options,
        debug,
    )
}

fn ensure_root_theme_portable(
    report: &RootThemeReport,
    portability: ThemePortabilityRequirement,
) -> Result<()> {
    if portability != ThemePortabilityRequirement::RequirePortable {
        return Ok(());
    }
    let verification = report.verification();
    if matches!(
        verification,
        crate::diagram_theme::RootThemeVerification::NotApplicable
            | crate::diagram_theme::RootThemeVerification::Verified
    ) {
        return Ok(());
    }
    Err(Error::RejectedRootTheme {
        verification,
        residual_count: report.residuals().len(),
    })
}

/// Prepares one family-owned typed semantic model for layout and SVG rendering.
///
/// Compatibility JSON is deliberately not accepted by this interface:
///
/// ```compile_fail
/// use merman_render::{LayoutOptions, environment::RenderEnvironment};
///
/// let session = RenderEnvironment::deterministic().begin_session().unwrap();
/// let raw_json = serde_json::json!({ "type": "flowchart-v2" });
/// let _ = merman_render::family::prepare(raw_json, &LayoutOptions::default(), session);
/// ```
///
/// Family semantic/layout pairing is private and therefore cannot be assembled independently:
///
/// ```compile_fail
/// use merman_render::family::FamilyPair;
/// ```
pub fn prepare(
    parsed: ParsedDiagramRender,
    options: &LayoutOptions,
    session: RenderSession,
) -> Result<FamilyRenderArtifact> {
    session.checkpoint(OperationPhase::Layout)?;
    if let Some(error) = session.text_layout_error().cloned() {
        return Err(error.into());
    }
    let plan = plan_render(&parsed, &session)?;
    plan.ensure_available()?;
    let required_capabilities = plan.required_capabilities().to_vec();
    let expected_family = plan.family_id();
    let context = FamilyRenderContext::resolve(session, expected_family);
    // The heterogeneous router has one generic layout call per family. Keep its debug-build
    // caller slots out of the Class Dagre call chain, whose own phase frames are already deep.
    let mut artifact = if expected_family == DiagramFamilyId::CLASS {
        preparation::prepare_class_render(parsed, options, context)
    } else {
        preparation::prepare_non_class_render(parsed, options, context)
    }?;
    artifact.required_capabilities = required_capabilities;
    artifact
        .context
        .session()
        .checkpoint(OperationPhase::Layout)?;
    Ok(artifact)
}

#[cfg(test)]
mod tests;

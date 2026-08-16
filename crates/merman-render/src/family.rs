mod capability;
mod evidence_support;

pub use capability::{RenderCapabilityPlan, plan_render};
pub(crate) use evidence_support::{
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};

use crate::diagram_theme::{
    FamilyThemeMechanismKey, ResolvedDiagramTheme, RootThemePlan, RootThemeReport,
    SourceStyleChannel, SourceStyleOrigin, SourceStyleResidual, SourceStyleResidualReason,
    ThemeCapability, ThemePortabilityRequirement, ThemeRecipeFingerprint,
};
use crate::environment::{RenderSession, RenderSessionReport};
use crate::model::*;
use crate::resources::ResourceLimitPhase;
use crate::svg::{
    ResvgCompatibleSvg, StandaloneSvgArtifact, SvgDebugOptions, SvgPipeline, SvgPipelinePreset,
    SvgPostprocessMetadata, SvgRenderOptions,
};
use crate::text::PreparedTextEvidenceLease;
use crate::wardley::WardleyDiagramLayout;
use crate::{Error, LayoutExecution, LayoutOptions, Result};
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

    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn property(&self) -> Option<&str> {
        self.property.as_deref()
    }

    pub fn owner_id(&self) -> &str {
        &self.owner_id
    }

    pub fn class_id(&self) -> Option<&str> {
        self.class_id.as_deref()
    }

    pub const fn origin(&self) -> FamilyStyleOrigin {
        self.origin
    }

    pub const fn channel(&self) -> FamilyStyleChannel {
        self.channel
    }

    pub const fn assignment_ordinal(&self) -> Option<usize> {
        self.assignment_ordinal
    }

    pub const fn declaration_ordinal(&self) -> usize {
        self.declaration_ordinal
    }

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
    native_filter_receipt: Option<crate::__private::NativeSvgFilterReceipt>,
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
            native_filter_receipt: plan.native_filter_receipt,
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
        self
    }

    pub const fn family_id(&self) -> DiagramFamilyId {
        self.family_id
    }

    pub const fn evaluation(&self) -> FamilyStyleEvaluation {
        self.evaluation
    }

    #[cfg(test)]
    pub(crate) const fn output_mutated(&self) -> bool {
        self.output_mutated
    }

    pub fn residuals(&self) -> &[FamilyStyleResidual] {
        &self.residuals
    }

    /// Returns every family-scoped typed mechanism selected by the recipe.
    pub fn theme_required_mechanisms(&self) -> &[FamilyThemeMechanismKey] {
        &self.theme_required
    }

    /// Returns typed mechanisms the family adapter explicitly emitted or consumed.
    pub fn theme_applied_mechanisms(&self) -> &[FamilyThemeMechanismKey] {
        &self.theme_applied
    }

    pub(crate) const fn native_filter_receipt(
        &self,
    ) -> Option<crate::__private::NativeSvgFilterReceipt> {
        self.native_filter_receipt
    }

    /// Returns recipe mechanisms evaluated against this document but not selected by any rendered
    /// target, variant, or ordinal.
    pub fn theme_not_applicable_mechanisms(&self) -> &[FamilyThemeMechanismKey] {
        &self.theme_not_applicable
    }

    /// Returns typed semantic mechanisms that remain outside this adapter's proof boundary.
    pub fn theme_residuals(&self) -> &[FamilyThemeResidual] {
        &self.theme_residuals
    }

    /// Returns temporary family-local Mermaid compatibility contributions used by this operation.
    pub const fn compatibility_residual_count(&self) -> usize {
        self.compatibility_residual_count
    }

    /// Returns explicit Mermaid compatibility fields that survived the final config cascade.
    #[doc(hidden)]
    pub const fn mermaid_compatibility_residual_count(&self) -> usize {
        self.mermaid_compatibility_residual_count
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

    pub(crate) const fn style_report(&self) -> &FamilyStyleReport {
        &self.style
    }

    pub(crate) const fn root_theme_report(&self) -> &RootThemeReport {
        &self.root_theme
    }

    pub(crate) const fn native_filter_receipt(
        &self,
    ) -> Option<crate::__private::NativeSvgFilterReceipt> {
        self.style.native_filter_receipt()
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
        Self {
            required: theme.family_mechanism_keys(),
            applied: Vec::new(),
            applied_capabilities: BTreeMap::new(),
            not_applicable: Vec::new(),
            residuals: Vec::new(),
        }
    }

    fn not_applicable(theme: Option<&ResolvedDiagramTheme>) -> bool {
        theme.is_none_or(|theme| theme.family_mechanism_keys().is_empty())
    }

    pub(crate) fn mark_applied(&mut self, key: FamilyThemeMechanismKey) {
        self.mark_applied_with_capabilities(key, []);
    }

    pub(crate) fn mark_applied_with_capabilities(
        &mut self,
        key: FamilyThemeMechanismKey,
        capabilities: impl IntoIterator<Item = ThemeCapability>,
    ) {
        if self.required.contains(&key) && !self.is_accounted(&key) {
            let capabilities = capabilities.into_iter().collect::<BTreeSet<_>>();
            self.applied.push(key.clone());
            self.applied_capabilities.insert(key, capabilities);
        }
    }

    pub(crate) fn mark_not_applicable(&mut self, key: FamilyThemeMechanismKey) {
        if self.required.contains(&key) && !self.is_accounted(&key) {
            self.not_applicable.push(key);
        }
    }

    pub(crate) fn mark_residual(
        &mut self,
        key: FamilyThemeMechanismKey,
        reason: FamilyThemeResidualReason,
    ) {
        if self.required.contains(&key) && !self.is_accounted(&key) {
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

    fn is_accounted(&self, key: &FamilyThemeMechanismKey) -> bool {
        self.applied.contains(key)
            || self.not_applicable.contains(key)
            || self.residuals.iter().any(|residual| residual.key == *key)
    }

    pub(crate) fn applied_capabilities(&self) -> BTreeSet<ThemeCapability> {
        self.applied_capabilities
            .values()
            .flat_map(|capabilities| capabilities.iter().copied())
            .collect()
    }

    fn merge_accounted_from(&mut self, mut other: Self) {
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

    #[cfg(test)]
    pub(crate) fn not_applicable_mechanisms(&self) -> &[FamilyThemeMechanismKey] {
        &self.not_applicable
    }

    #[cfg(test)]
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
            output_mutated: false,
            compatibility_residual_count: 0,
            mermaid_compatibility_residual_count: 0,
            payload,
        }
    }

    fn adapt_state(
        &mut self,
        model: &merman_core::diagrams::state::StateDiagramRenderModel,
        effective_config: &serde_json::Value,
        title: Option<&str>,
        prepared_text_available: bool,
        effective_theme_resource_policy: Arc<crate::diagram_theme::ThemeResourcePolicy>,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> Result<()> {
        debug_assert_eq!(self.family_id, DiagramFamilyId::STATE);
        let (plan, theme_evidence) = crate::state::StateStylePlan::resolve_with_evidence(
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
        self.mermaid_compatibility_residual_count = evidence.mermaid_residual_count();
        self.compatibility_residual_count = evidence.fallback_contribution_count();
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

    fn observe_output_visibility(&mut self, debug: &SvgDebugOptions) {
        if self.family_id == DiagramFamilyId::STATE {
            self.theme_evidence
                .mark_state_output_visibility_filtered(debug);
        }
    }

    fn invalidate_for_output_mutation(&mut self) {
        self.output_mutated = true;
        self.native_filter_receipt = None;
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
            DiagramFamilyId::FLOWCHART
                | DiagramFamilyId::SWIMLANE
                | DiagramFamilyId::SEQUENCE
                | DiagramFamilyId::CLASS
                | DiagramFamilyId::GANTT
        ) && self
            .resolved_theme
            .as_deref()
            .is_some_and(ResolvedDiagramTheme::has_family_mechanism_routes);
        let report = FamilyStyleReport::freeze(self);
        if waits_for_svg_evidence {
            report.ensure_compatibility_portable()
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
        effective_config: &serde_json::Value,
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

    fn merge_accounted_terminal_evidence(
        &mut self,
        expected_family: DiagramFamilyId,
        evidence: FamilyThemeEvidence,
    ) {
        self.style_plan
            .merge_accounted_terminal_evidence(expected_family, evidence);
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
    label_sources: diagrams::flowchart::FlowchartRenderLabelSources,
    edge_style_plan: crate::svg::FlowchartEdgeStylePlan,
    edge_theme: crate::flowchart::FlowchartEdgeThemeStyle,
    svg_label_sidecar: crate::flowchart::FlowchartSvgLabelSidecar,
    theme_evidence: crate::flowchart::FlowchartThemeEvidenceRecorder,
}

impl<L> FlowchartFamilyArtifact<L> {
    pub(crate) fn pair(&self) -> &FamilyPair<diagrams::flowchart::FlowchartModel, L> {
        &self.pair
    }

    pub(crate) fn label_sources(&self) -> &diagrams::flowchart::FlowchartRenderLabelSources {
        &self.label_sources
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

#[derive(Debug)]
pub(crate) struct ClassFamilyArtifact {
    pair: FamilyPair<ClassDiagram, ClassDiagramLayout>,
    relation_theme: crate::class::ClassRelationThemePlan,
    theme_evidence: crate::class::ClassThemeEvidenceRecorder,
}

impl ClassFamilyArtifact {
    pub(crate) const fn pair(&self) -> &FamilyPair<ClassDiagram, ClassDiagramLayout> {
        &self.pair
    }

    pub(crate) const fn relation_theme(&self) -> &crate::class::ClassRelationThemePlan {
        &self.relation_theme
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

#[derive(Debug)]
pub(crate) enum BuiltinFamilyArtifact {
    Error(Box<FamilyPair<diagrams::error_diagram::ErrorDiagramRenderModel, ErrorDiagramLayout>>),
    Mindmap(Box<FamilyPair<diagrams::mindmap::MindmapDiagramRenderModel, MindmapDiagramLayout>>),
    State(Box<StateFamilyArtifact>),
    Sequence(
        Box<
            FamilyPair<
                diagrams::sequence::SequenceDiagramRenderModel,
                crate::sequence::SequencePreparedArtifact,
            >,
        >,
    ),
    Zenuml(
        Box<
            FamilyPair<
                diagrams::zenuml::ZenumlDiagramRenderModel,
                crate::zenuml::ZenumlDiagramLayout,
            >,
        >,
    ),
    Flowchart(Box<FlowchartFamilyArtifact<FlowchartLayout>>),
    Swimlane(Box<FlowchartFamilyArtifact<SwimlaneLayout>>),
    #[cfg(feature = "layout-cytoscape")]
    Architecture(
        Box<
            FamilyPair<
                diagrams::architecture::ArchitectureDiagramRenderModel,
                ArchitectureDiagramLayout,
            >,
        >,
    ),
    Class(Box<ClassFamilyArtifact>),
    C4(Box<FamilyPair<diagrams::c4::C4DiagramRenderModel, C4DiagramLayout>>),
    Cynefin(Box<FamilyPair<diagrams::cynefin::CynefinDiagramRenderModel, CynefinDiagramLayout>>),
    Wardley(Box<FamilyPair<diagrams::wardley::WardleyDiagramRenderModel, WardleyDiagramLayout>>),
    Railroad(
        Box<FamilyPair<diagrams::railroad::RailroadDiagramRenderModel, RailroadDiagramLayout>>,
    ),
    Kanban(
        Box<
            FamilyPair<
                diagrams::kanban::KanbanDiagramRenderModel,
                crate::kanban::KanbanPreparedArtifact,
            >,
        >,
    ),
    Gantt(Box<GanttFamilyArtifact>),
    Pie(Box<FamilyPair<diagrams::pie::PieDiagramRenderModel, PieDiagramLayout>>),
    Packet(Box<FamilyPair<diagrams::packet::PacketDiagramRenderModel, PacketDiagramLayout>>),
    Timeline(
        Box<FamilyPair<diagrams::timeline::TimelineDiagramRenderModel, TimelineDiagramLayout>>,
    ),
    Journey(Box<FamilyPair<diagrams::journey::JourneyDiagramRenderModel, JourneyDiagramLayout>>),
    Requirement(
        Box<
            FamilyPair<
                diagrams::requirement::RequirementDiagramRenderModel,
                crate::requirement::RequirementPreparedArtifact,
            >,
        >,
    ),
    Sankey(Box<FamilyPair<diagrams::sankey::SankeyDiagramRenderModel, SankeyDiagramLayout>>),
    Radar(Box<FamilyPair<diagrams::radar::RadarDiagramRenderModel, RadarDiagramLayout>>),
    Info(Box<FamilyPair<diagrams::info::InfoDiagramRenderModel, InfoDiagramLayout>>),
    Treemap(Box<FamilyPair<diagrams::treemap::TreemapDiagramRenderModel, TreemapDiagramLayout>>),
    Block(Box<FamilyPair<diagrams::block::BlockDiagramRenderModel, BlockDiagramLayout>>),
    Er(Box<FamilyPair<diagrams::er::ErDiagramRenderModel, ErDiagramLayout>>),
    QuadrantChart(
        Box<
            FamilyPair<
                diagrams::quadrant_chart::QuadrantChartRenderModel,
                QuadrantChartDiagramLayout,
            >,
        >,
    ),
    XyChart(Box<FamilyPair<diagrams::xychart::XyChartDiagramRenderModel, XyChartDiagramLayout>>),
    GitGraph(Box<FamilyPair<diagrams::git_graph::GitGraphRenderModel, GitGraphDiagramLayout>>),
    TreeView(
        Box<FamilyPair<diagrams::tree_view::TreeViewDiagramRenderModel, TreeViewDiagramLayout>>,
    ),
    Ishikawa(
        Box<FamilyPair<diagrams::ishikawa::IshikawaDiagramRenderModel, IshikawaDiagramLayout>>,
    ),
    EventModeling(
        Box<
            FamilyPair<
                diagrams::eventmodeling::EventModelingDiagramRenderModel,
                EventModelingDiagramLayout,
            >,
        >,
    ),
    Venn(Box<FamilyPair<diagrams::venn::VennDiagramRenderModel, VennDiagramLayout>>),
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

    fn flowchart_theme_evidence(
        &self,
        theme: Option<&ResolvedDiagramTheme>,
    ) -> Option<(FamilyThemeEvidence, Vec<SourceStyleResidual>)> {
        match self {
            Self::Flowchart(artifact) => Some(artifact.theme_evidence().finish(theme)),
            Self::Swimlane(artifact) => Some(artifact.theme_evidence().finish(theme)),
            _ => None,
        }
    }

    fn sequence_theme_evidence(
        &self,
        theme: Option<&ResolvedDiagramTheme>,
    ) -> Option<FamilyThemeEvidence> {
        match self {
            Self::Sequence(pair) => Some(pair.layout().theme_evidence().finish(theme)),
            _ => None,
        }
    }

    fn class_theme_evidence(
        &self,
        theme: Option<&ResolvedDiagramTheme>,
    ) -> Option<FamilyThemeEvidence> {
        match self {
            Self::Class(artifact) => Some(
                artifact
                    .theme_evidence()
                    .finish(theme, artifact.relation_theme()),
            ),
            _ => None,
        }
    }

    fn gantt_theme_evidence(&self) -> Option<FamilyThemeEvidence> {
        match self {
            Self::Gantt(artifact) => Some(artifact.task_theme().finish_evidence()),
            _ => None,
        }
    }

    fn compatibility_json(
        &self,
        metadata: &ParseMetadata,
    ) -> merman_core::Result<serde_json::Value> {
        match self {
            Self::Error(pair) => pair.compatibility_json(metadata),
            Self::Mindmap(pair) => pair.compatibility_json(metadata),
            Self::State(artifact) => artifact.pair.compatibility_json(metadata),
            Self::Sequence(pair) => pair.compatibility_json(metadata),
            Self::Zenuml(pair) => pair.compatibility_json(metadata),
            Self::Flowchart(artifact) => artifact.pair.compatibility_json(metadata),
            Self::Swimlane(artifact) => artifact.pair.compatibility_json(metadata),
            #[cfg(feature = "layout-cytoscape")]
            Self::Architecture(pair) => pair.compatibility_json(metadata),
            Self::Class(artifact) => artifact.pair.compatibility_json(metadata),
            Self::C4(pair) => pair.compatibility_json(metadata),
            Self::Cynefin(pair) => pair.compatibility_json(metadata),
            Self::Wardley(pair) => pair.compatibility_json(metadata),
            Self::Railroad(pair) => pair.compatibility_json(metadata),
            Self::Kanban(pair) => pair.compatibility_json(metadata),
            Self::Gantt(artifact) => artifact.pair.compatibility_json(metadata),
            Self::Pie(pair) => pair.compatibility_json(metadata),
            Self::Packet(pair) => pair.compatibility_json(metadata),
            Self::Timeline(pair) => pair.compatibility_json(metadata),
            Self::Journey(pair) => pair.compatibility_json(metadata),
            Self::Requirement(pair) => pair.compatibility_json(metadata),
            Self::Sankey(pair) => pair.compatibility_json(metadata),
            Self::Radar(pair) => pair.compatibility_json(metadata),
            Self::Info(pair) => pair.compatibility_json(metadata),
            Self::Treemap(pair) => pair.compatibility_json(metadata),
            Self::Block(pair) => pair.compatibility_json(metadata),
            Self::Er(pair) => pair.compatibility_json(metadata),
            Self::QuadrantChart(pair) => pair.compatibility_json(metadata),
            Self::XyChart(pair) => pair.compatibility_json(metadata),
            Self::GitGraph(pair) => pair.compatibility_json(metadata),
            Self::TreeView(pair) => pair.compatibility_json(metadata),
            Self::Ishikawa(pair) => pair.compatibility_json(metadata),
            Self::EventModeling(pair) => pair.compatibility_json(metadata),
            Self::Venn(pair) => pair.compatibility_json(metadata),
        }
    }

    fn layout_projection(&self) -> LayoutProjection<'_> {
        match self {
            Self::Error(pair) => LayoutProjection::ErrorDiagram(pair.layout()),
            Self::Mindmap(pair) => LayoutProjection::MindmapDiagram(pair.layout()),
            Self::State(artifact) => LayoutProjection::StateDiagram(artifact.pair.layout()),
            Self::Sequence(pair) => LayoutProjection::SequenceDiagram(pair.layout().layout()),
            Self::Zenuml(pair) => LayoutProjection::ZenumlDiagram(pair.layout()),
            Self::Flowchart(artifact) => LayoutProjection::Flowchart(artifact.pair.layout()),
            Self::Swimlane(artifact) => LayoutProjection::SwimlaneDiagram(artifact.pair.layout()),
            #[cfg(feature = "layout-cytoscape")]
            Self::Architecture(pair) => LayoutProjection::ArchitectureDiagram(pair.layout()),
            Self::Class(artifact) => LayoutProjection::ClassDiagram(artifact.pair.layout()),
            Self::C4(pair) => LayoutProjection::C4Diagram(pair.layout()),
            Self::Cynefin(pair) => LayoutProjection::CynefinDiagram(pair.layout()),
            Self::Wardley(pair) => LayoutProjection::WardleyDiagram(pair.layout()),
            Self::Railroad(pair) => LayoutProjection::RailroadDiagram(pair.layout()),
            Self::Kanban(pair) => LayoutProjection::KanbanDiagram(pair.layout().layout()),
            Self::Gantt(artifact) => LayoutProjection::GanttDiagram(artifact.pair.layout()),
            Self::Pie(pair) => LayoutProjection::PieDiagram(pair.layout()),
            Self::Packet(pair) => LayoutProjection::PacketDiagram(pair.layout()),
            Self::Timeline(pair) => LayoutProjection::TimelineDiagram(pair.layout()),
            Self::Journey(pair) => LayoutProjection::JourneyDiagram(pair.layout()),
            Self::Requirement(pair) => LayoutProjection::RequirementDiagram(pair.layout().layout()),
            Self::Sankey(pair) => LayoutProjection::SankeyDiagram(pair.layout()),
            Self::Radar(pair) => LayoutProjection::RadarDiagram(pair.layout()),
            Self::Info(pair) => LayoutProjection::InfoDiagram(pair.layout()),
            Self::Treemap(pair) => LayoutProjection::TreemapDiagram(pair.layout()),
            Self::Block(pair) => LayoutProjection::BlockDiagram(pair.layout()),
            Self::Er(pair) => LayoutProjection::ErDiagram(pair.layout()),
            Self::QuadrantChart(pair) => LayoutProjection::QuadrantChartDiagram(pair.layout()),
            Self::XyChart(pair) => LayoutProjection::XyChartDiagram(pair.layout()),
            Self::GitGraph(pair) => LayoutProjection::GitGraphDiagram(pair.layout()),
            Self::TreeView(pair) => LayoutProjection::TreeViewDiagram(pair.layout()),
            Self::Ishikawa(pair) => LayoutProjection::IshikawaDiagram(pair.layout()),
            Self::EventModeling(pair) => LayoutProjection::EventModelingDiagram(pair.layout()),
            Self::Venn(pair) => LayoutProjection::VennDiagram(pair.layout()),
        }
    }
}

pub struct FamilyRenderArtifact {
    metadata: ParseMetadata,
    compatibility_projection: OnceLock<std::result::Result<serde_json::Value, String>>,
    family: BuiltinFamilyArtifact,
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
    root_theme: RootThemeReport,
    style_report: FamilyStyleReport,
    metadata: ParseMetadata,
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

    pub(crate) const fn style_report(&self) -> &FamilyStyleReport {
        &self.style_report
    }

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
        let output_metadata = self.output_metadata();
        let preserves_prepared_text =
            self.prepared_text_evidence_valid && pipeline.preserves_prepared_text_evidence();
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
        let processed_svg = pipeline.process_owned_to_string_with_metadata(
            source_svg,
            &output_metadata,
            &self.session,
        )?;
        if preserves_prepared_text && !self.prepared_text_ledger.is_empty() {
            let (public_svg, prepared_text_svg) = crate::svg::partition_prepared_text_label_ids(
                processed_svg,
                self.prepared_text_ledger.entries(),
            )?;
            self.svg = public_svg;
            self.prepared_text_svg = prepared_text_svg;
        } else {
            self.svg = processed_svg;
        }
        self.session
            .resource_policy()
            .check_svg_bytes(&self.svg, ResourceLimitPhase::SvgPostprocess)?;
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
        let output_metadata = self.output_metadata();
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
        let svg = pipeline
            .process_owned_resvg_compatible_with_metadata(
                source_svg,
                &output_metadata,
                &self.session,
            )?
            .attach_prepared_text_evidence(
                prepared_text_ledger,
                prepared_text_evidence_valid,
                self.session.resource_policy(),
            )?;
        self.session
            .resource_policy()
            .check_svg_bytes(svg.as_str(), ResourceLimitPhase::SvgPostprocess)?;
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
        match pipeline {
            Some(pipeline) if pipeline.preset() == SvgPipelinePreset::ResvgSafe => {
                let finalized = self.finalize_resvg_with_portability(pipeline, false)?;
                Ok(finalized.into_standalone())
            }
            Some(pipeline) => self
                .apply_pipeline_with_portability(pipeline, false)?
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

    fn output_metadata(&self) -> SvgPostprocessMetadata {
        SvgPostprocessMetadata::from_svg(&self.svg)
            .with_family_id(self.family_id())
            .with_diagram_type(self.metadata.diagram_type.clone())
            .with_optional_diagram_title(self.metadata.title.clone())
    }

    pub fn into_completion(self) -> FamilyRenderCompletion<String> {
        let Self {
            svg,
            prepared_text_svg: _,
            prepared_text_ledger: _,
            prepared_text_evidence_valid: _,
            root_theme,
            style_report,
            session,
            metadata: _,
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

    pub(crate) const fn style_report(&self) -> &FamilyStyleReport {
        &self.style_report
    }

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
            })?;
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
        projection.insert(
            "semantic".to_string(),
            clone_json_value_nonrecursive(semantic),
        );
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
        let rendered = render_family_artifact_svg(&self, options, debug)?;
        self.context
            .session()
            .resource_policy()
            .check_svg_bytes(rendered.as_str(), ResourceLimitPhase::SvgOutput)?;
        self.context.session().checkpoint(OperationPhase::Emit)?;
        let flowchart_theme_evidence = self
            .family
            .flowchart_theme_evidence(self.context.resolved_theme());
        let sequence_theme_evidence = self
            .family
            .sequence_theme_evidence(self.context.resolved_theme());
        let class_theme_evidence = self
            .family
            .class_theme_evidence(self.context.resolved_theme());
        let gantt_theme_evidence = self.family.gantt_theme_evidence();
        let state_filter_receipt = match &self.family {
            BuiltinFamilyArtifact::State(artifact) => Some(artifact.effect_evidence().finish()),
            _ => None,
        };
        let prepared_text_ledger = self.family.prepared_text_label_ledger();
        let Self {
            metadata,
            compatibility_projection: _,
            family: _,
            mut context,
        } = self;
        if let Some((evidence, source_style_residuals)) = flowchart_theme_evidence {
            context.merge_flowchart_evidence(evidence, source_style_residuals);
        }
        if let Some(evidence) = sequence_theme_evidence {
            context.merge_sequence_evidence(evidence);
        }
        if let Some(evidence) = class_theme_evidence {
            context.merge_accounted_terminal_evidence(DiagramFamilyId::CLASS, evidence);
        }
        if let Some(evidence) = gantt_theme_evidence {
            context.merge_accounted_terminal_evidence(DiagramFamilyId::GANTT, evidence);
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
        )?;
        let (session, style_report) = context.into_session_and_style_report();
        ensure_root_theme_portable(
            &root_theme,
            session
                .theme_portability_requirement()
                .unwrap_or(ThemePortabilityRequirement::BestEffort),
        )?;
        session.checkpoint(OperationPhase::Emit)?;

        Ok(RenderedFamilySvg {
            svg,
            prepared_text_svg,
            prepared_text_ledger,
            prepared_text_evidence_valid: true,
            root_theme,
            style_report,
            metadata,
            session,
        })
    }
}

#[inline(never)]
fn render_family_artifact_svg(
    artifact: &FamilyRenderArtifact,
    request: &SvgRenderOptions,
    debug: &SvgDebugOptions,
) -> Result<crate::svg::RootThemeAppliedSvg> {
    let options = request.normalized();
    let execution = artifact.context.execution();
    #[cfg(feature = "layout-cytoscape")]
    if let BuiltinFamilyArtifact::Architecture(pair) = &artifact.family {
        return crate::svg::render_architecture_family_artifact(
            pair,
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

#[inline(never)]
fn prepare_pair<S, L>(
    semantic: S,
    layout: impl FnOnce(&S) -> Result<L>,
) -> Result<Box<FamilyPair<S, L>>> {
    let layout = layout(&semantic)?;
    Ok(Box::new(FamilyPair::new(semantic, layout)))
}

fn prepare_flowchart_artifact<L>(
    semantic: diagrams::flowchart::FlowchartModel,
    label_sources: diagrams::flowchart::FlowchartRenderLabelSources,
    prepared_text_layout: Option<&crate::text::PreparedTextLayout>,
    resolved_theme: Option<&ResolvedDiagramTheme>,
    typography_config_ownership: crate::flowchart::FlowchartTypographyConfigOwnership,
    work_meter: Arc<crate::resources::OperationWorkMeter>,
    edge_style_plan: crate::svg::FlowchartEdgeStylePlan,
    layout: impl FnOnce(
        &diagrams::flowchart::FlowchartModel,
        &diagrams::flowchart::FlowchartRenderLabelSources,
        &crate::flowchart::FlowchartSvgLabelSidecarBuilder,
        &crate::svg::FlowchartEdgeStylePlan,
    ) -> Result<L>,
) -> Result<Box<FlowchartFamilyArtifact<L>>> {
    let edge_theme =
        crate::flowchart::FlowchartEdgeThemeStyle::resolve(resolved_theme, work_meter.as_ref())?;
    let svg_label_sidecar = crate::flowchart::FlowchartSvgLabelSidecarBuilder::new_with_work_meter(
        prepared_text_layout,
        resolved_theme,
        work_meter,
    )
    .with_typography_config_ownership(typography_config_ownership)
    .with_edge_label_padding(edge_theme.edge_label_padding());
    let layout = layout(
        &semantic,
        &label_sources,
        &svg_label_sidecar,
        &edge_style_plan,
    )?;
    let svg_label_sidecar = svg_label_sidecar.finish();
    if let Some(error) = svg_label_sidecar.prepared_resource_error().cloned() {
        return Err(error.into());
    }
    if let Some(error) = svg_label_sidecar.prepared_error().cloned() {
        return Err(error.into());
    }
    Ok(Box::new(FlowchartFamilyArtifact {
        pair: FamilyPair::new(semantic, layout),
        label_sources,
        edge_style_plan,
        edge_theme,
        svg_label_sidecar,
        theme_evidence: crate::flowchart::FlowchartThemeEvidenceRecorder::default(),
    }))
}

#[inline(never)]
fn prepare_class_family(
    model: ClassDiagram,
    meta: &ParseMetadata,
    diagram_type: &str,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    let relation_count = model.relations.len();
    let node_count = model.classes.len();
    let relation_theme = crate::class::ClassRelationThemePlan::resolve(
        execution.resolved_theme(),
        relation_count,
        node_count,
        execution.work_meter_ref(),
    )?;
    let layout = crate::layout_class_typed_by_engine(
        diagram_type,
        &model,
        &meta.effective_config,
        execution,
    )?;
    Ok(BuiltinFamilyArtifact::Class(Box::new(
        ClassFamilyArtifact {
            pair: FamilyPair::new(model, layout),
            relation_theme,
            theme_evidence: crate::class::ClassThemeEvidenceRecorder::new(
                relation_count,
                node_count,
            ),
        },
    )))
}

#[inline(never)]
fn prepare_class_render(
    parsed: ParsedDiagramRender,
    options: &LayoutOptions,
    mut context: FamilyRenderContext,
) -> Result<FamilyRenderArtifact> {
    let (meta, model) = parsed.into_parts();
    let RenderSemanticModel::Class(model) = model else {
        unreachable!("Class render dispatch requires a Class semantic model")
    };
    context.observe_compatibility(&meta);
    context.ensure_portable_before_svg()?;
    let diagram_type = meta.diagram_type.as_str();
    let execution = LayoutExecution::new(options, context.execution());
    let family = prepare_class_family(model, &meta, diagram_type, &execution)?;
    FamilyRenderArtifact::new(meta, family, context)
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
    let expected_family = plan.family_id();
    let context = FamilyRenderContext::resolve(session, expected_family);
    // The heterogeneous router has one generic layout call per family. Keep its debug-build
    // caller slots out of the Class Dagre call chain, whose own phase frames are already deep.
    let artifact = if expected_family == DiagramFamilyId::CLASS {
        prepare_class_render(parsed, options, context)
    } else {
        prepare_non_class_render(parsed, options, context)
    }?;
    artifact
        .context
        .session()
        .checkpoint(OperationPhase::Layout)?;
    Ok(artifact)
}

#[inline(never)]
fn prepare_non_class_render(
    parsed: ParsedDiagramRender,
    options: &LayoutOptions,
    mut context: FamilyRenderContext,
) -> Result<FamilyRenderArtifact> {
    let (meta, model, render_context) = parsed.into_render_parts();
    let flowchart_label_sources = render_context.into_flowchart_label_sources();
    let diagram_type = meta.diagram_type.as_str();
    let effective_config = meta.effective_config.as_value();
    let title = meta.title.as_deref();
    context.observe_compatibility(&meta);
    if let RenderSemanticModel::State(model) = &model {
        context.adapt_state(model, effective_config, title)?;
    }
    context.ensure_portable_before_svg()?;
    let execution = LayoutExecution::new(options, context.execution());
    let family = match model {
        RenderSemanticModel::Error(model) => {
            BuiltinFamilyArtifact::Error(prepare_pair(model, |model| {
                crate::error::layout_error_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Mindmap(model) => {
            BuiltinFamilyArtifact::Mindmap(prepare_pair(model, |model| {
                crate::mindmap::layout_mindmap_diagram_typed_with_work_meter(
                    model,
                    &meta.effective_config,
                    execution.text_measurer(),
                    execution.math_renderer(),
                    execution.work_meter(),
                )
            })?)
        }
        RenderSemanticModel::State(model) => {
            let label_sidecar = crate::state::StateLabelSidecarBuilder::new_with_work_meter(
                execution.prepared_text_layout(),
                execution.work_meter(),
            );
            if title.is_some_and(|title| !title.trim().is_empty()) {
                label_sidecar.reject_unsupported("state_diagram_title_bbox_y");
            }
            let layout = crate::state::layout_state_diagram_typed_with_work_meter(
                &model,
                effective_config,
                execution
                    .state_style_plan()
                    .expect("State family layout requires an adapted style plan"),
                execution.text_measurer(),
                Some(&label_sidecar),
                execution.work_meter(),
            )?;
            let label_sidecar = label_sidecar.finish();
            if let Some(error) = label_sidecar.prepared_resource_error().cloned() {
                return Err(error.into());
            }
            if let Some(error) = label_sidecar.prepared_error().cloned() {
                return Err(error.into());
            }
            BuiltinFamilyArtifact::State(Box::new(StateFamilyArtifact {
                pair: FamilyPair::new(model, layout),
                label_sidecar,
                effect_evidence: crate::state::StateSvgEffectEvidenceRecorder::default(),
            }))
        }
        RenderSemanticModel::Sequence(model) => {
            BuiltinFamilyArtifact::Sequence(prepare_pair(model, |model| {
                crate::sequence::prepare_sequence_diagram_typed_with_title_and_work_meter(
                    model,
                    title,
                    &meta.effective_config,
                    execution.resolved_theme(),
                    execution.prepared_text_layout(),
                    execution.text_measurer(),
                    execution.math_renderer(),
                    execution.work_meter(),
                )
            })?)
        }
        RenderSemanticModel::Zenuml(model) => {
            BuiltinFamilyArtifact::Zenuml(prepare_pair(model, |model| {
                crate::zenuml::layout_zenuml_diagram_typed(model, execution.text_measurer())
            })?)
        }
        RenderSemanticModel::Flowchart(model) => match execution.family_id() {
            DiagramFamilyId::SWIMLANE => {
                let edge_style_plan = crate::svg::FlowchartEdgeStylePlan::prepare_for_model(
                    &model,
                    &meta.effective_config,
                    true,
                    execution.work_meter_ref(),
                )?;
                BuiltinFamilyArtifact::Swimlane(prepare_flowchart_artifact(
                    model,
                    flowchart_label_sources,
                    execution.prepared_text_layout(),
                    execution.resolved_theme(),
                    crate::flowchart::flowchart_typography_config_ownership(&meta.effective_config),
                    execution.work_meter(),
                    edge_style_plan,
                    |model, label_sources, svg_label_sidecar, edge_style_plan| {
                        crate::swimlane::layout_swimlane_typed_with_work_meter_and_svg_label_sidecar(
                            model,
                            label_sources,
                            &meta.effective_config,
                            execution.text_measurer(),
                            execution.math_renderer(),
                            Some(svg_label_sidecar),
                            edge_style_plan,
                            execution.work_meter(),
                        )
                    },
                )?)
            }
            DiagramFamilyId::FLOWCHART => {
                let edge_style_plan = crate::svg::FlowchartEdgeStylePlan::prepare_for_model(
                    &model,
                    &meta.effective_config,
                    false,
                    execution.work_meter_ref(),
                )?;
                BuiltinFamilyArtifact::Flowchart(prepare_flowchart_artifact(
                    model,
                    flowchart_label_sources,
                    execution.prepared_text_layout(),
                    execution.resolved_theme(),
                    crate::flowchart::flowchart_typography_config_ownership(&meta.effective_config),
                    execution.work_meter(),
                    edge_style_plan,
                    |model, label_sources, svg_label_sidecar, edge_style_plan| {
                        crate::layout_flowchart_typed_with_render_labels_and_svg_label_sidecar_by_engine(
                            diagram_type,
                            model,
                            label_sources,
                            &meta.effective_config,
                            &execution,
                            Some(svg_label_sidecar),
                            edge_style_plan,
                        )
                    },
                )?)
            }
            planned_family => {
                return Err(Error::InvalidModel {
                    message: format!(
                        "planned render family {planned_family} cannot consume a Flowchart model"
                    ),
                });
            }
        },
        #[cfg(feature = "layout-cytoscape")]
        RenderSemanticModel::Architecture(model) => {
            BuiltinFamilyArtifact::Architecture(prepare_pair(model, |model| {
                crate::architecture::layout_architecture_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                    execution.operation_seed(),
                    execution.work_meter().as_ref(),
                )
            })?)
        }
        #[cfg(not(feature = "layout-cytoscape"))]
        RenderSemanticModel::Architecture(_) => {
            return Err(Error::MissingCapability {
                capability: crate::RenderCapability::LayoutCytoscape,
                diagram_type: diagram_type.to_string(),
            });
        }
        RenderSemanticModel::Class(_) => {
            unreachable!("Class models use the stack-bounded family dispatch path")
        }
        RenderSemanticModel::C4(model) => {
            BuiltinFamilyArtifact::C4(prepare_pair(model, |model| {
                crate::c4::layout_c4_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                    execution.container_width,
                    execution.container_height,
                    execution.screen_available_width,
                )
            })?)
        }
        RenderSemanticModel::Cynefin(model) => {
            BuiltinFamilyArtifact::Cynefin(prepare_pair(model, |model| {
                crate::cynefin::layout_cynefin_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Wardley(model) => {
            BuiltinFamilyArtifact::Wardley(prepare_pair(model, |model| {
                crate::wardley::layout_wardley_diagram_typed(
                    model,
                    title,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Railroad(model) => {
            BuiltinFamilyArtifact::Railroad(prepare_pair(model, |model| {
                crate::railroad::layout_railroad_diagram_typed_for_type(
                    model,
                    diagram_type,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Kanban(model) => {
            BuiltinFamilyArtifact::Kanban(prepare_pair(model, |model| {
                crate::kanban::prepare_kanban_diagram_typed_with_work_meter(
                    model,
                    &meta.effective_config,
                    execution.text_measurer(),
                    execution.work_meter_ref(),
                )
            })?)
        }
        RenderSemanticModel::Gantt(model) => {
            let task_theme = crate::gantt::GanttTaskTheme::resolve(
                execution.resolved_theme(),
                &model.tasks,
                execution.work_meter_ref(),
            )?;
            let layout = crate::gantt::layout_gantt_diagram_typed(
                &model,
                title,
                effective_config,
                &task_theme,
                execution.text_measurer(),
                execution.container_width,
                execution.local_time_zone(),
            )?;
            BuiltinFamilyArtifact::Gantt(Box::new(GanttFamilyArtifact {
                pair: FamilyPair::new(model, layout),
                task_theme,
            }))
        }
        RenderSemanticModel::Pie(model) => {
            BuiltinFamilyArtifact::Pie(prepare_pair(model, |model| {
                crate::pie::layout_pie_diagram_typed(
                    model,
                    title,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Packet(model) => {
            BuiltinFamilyArtifact::Packet(prepare_pair(model, |model| {
                crate::packet::layout_packet_diagram_typed(
                    model,
                    title,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Timeline(model) => {
            BuiltinFamilyArtifact::Timeline(prepare_pair(model, |model| {
                crate::timeline::layout_timeline_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Journey(model) => {
            BuiltinFamilyArtifact::Journey(prepare_pair(model, |model| {
                crate::journey::layout_journey_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Requirement(model) => {
            BuiltinFamilyArtifact::Requirement(prepare_pair(model, |model| {
                crate::requirement::layout_requirement_diagram_typed_with_work_meter(
                    model,
                    effective_config,
                    execution.text_measurer(),
                    execution.work_meter_ref(),
                )
            })?)
        }
        RenderSemanticModel::Sankey(model) => {
            BuiltinFamilyArtifact::Sankey(prepare_pair(model, |model| {
                crate::sankey::layout_sankey_diagram_typed_with_work_meter(
                    model,
                    effective_config,
                    execution.text_measurer(),
                    execution.work_meter_ref(),
                )
            })?)
        }
        RenderSemanticModel::Radar(model) => {
            BuiltinFamilyArtifact::Radar(prepare_pair(model, |model| {
                crate::radar::layout_radar_diagram_typed_with_work_meter(
                    model,
                    effective_config,
                    execution.text_measurer(),
                    execution.work_meter_ref(),
                )
            })?)
        }
        RenderSemanticModel::Info(model) => {
            BuiltinFamilyArtifact::Info(prepare_pair(model, |model| {
                crate::info::layout_info_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Treemap(model) => {
            BuiltinFamilyArtifact::Treemap(prepare_pair(model, |model| {
                crate::treemap::layout_treemap_diagram_typed(
                    model,
                    title,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Block(model) => {
            BuiltinFamilyArtifact::Block(prepare_pair(model, |model| {
                crate::block::layout_block_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Er(model) => {
            #[cfg(feature = "layout-elk")]
            {
                BuiltinFamilyArtifact::Er(prepare_pair(model, |model| {
                    crate::er::layout_er_diagram_typed_with_elk_operation_seed(
                        model,
                        effective_config,
                        execution.text_measurer(),
                        execution.elk_operation_seed(),
                        execution.work_meter(),
                    )
                })?)
            }
            #[cfg(not(feature = "layout-elk"))]
            BuiltinFamilyArtifact::Er(prepare_pair(model, |model| {
                crate::er::layout_er_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                    execution.work_meter(),
                )
            })?)
        }
        RenderSemanticModel::QuadrantChart(model) => {
            BuiltinFamilyArtifact::QuadrantChart(prepare_pair(model, |model| {
                crate::quadrantchart::layout_quadrantchart_diagram_typed(
                    model,
                    title,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::XyChart(model) => {
            BuiltinFamilyArtifact::XyChart(prepare_pair(model, |model| {
                crate::xychart::layout_xychart_diagram_typed(
                    model,
                    title,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::GitGraph(model) => {
            BuiltinFamilyArtifact::GitGraph(prepare_pair(model, |model| {
                crate::gitgraph::layout_gitgraph_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::TreeView(model) => {
            BuiltinFamilyArtifact::TreeView(prepare_pair(model, |model| {
                crate::tree_view::layout_tree_view_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Ishikawa(model) => {
            BuiltinFamilyArtifact::Ishikawa(prepare_pair(model, |model| {
                crate::ishikawa::layout_ishikawa_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::EventModeling(model) => {
            BuiltinFamilyArtifact::EventModeling(prepare_pair(model, |model| {
                crate::eventmodeling::layout_eventmodeling_diagram_typed(
                    model,
                    effective_config,
                    execution.text_measurer(),
                )
            })?)
        }
        RenderSemanticModel::Venn(model) => {
            BuiltinFamilyArtifact::Venn(prepare_pair(model, |model| {
                crate::venn::layout_venn_diagram_typed_with_work_meter(
                    model,
                    title,
                    effective_config,
                    execution.work_meter_ref(),
                )
            })?)
        }
        RenderSemanticModel::CustomJson(_) => {
            unreachable!("custom JSON models return before built-in family dispatch")
        }
    };
    FamilyRenderArtifact::new(meta, family, context)
}

#[cfg(test)]
mod tests;

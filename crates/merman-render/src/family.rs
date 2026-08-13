use crate::diagram_theme::{
    FamilyThemeMechanismKey, ResolvedDiagramTheme, RootThemePlan, RootThemeReport,
    SourceStyleChannel, SourceStyleOrigin, SourceStyleResidual, SourceStyleResidualReason,
    ThemeCapability, ThemePortabilityRequirement, ThemeRecipeFingerprint,
};
use crate::environment::{RenderSession, RenderSessionReport};
use crate::model::*;
use crate::resources::ResourceLimitPhase;
use crate::svg::{
    ResvgCompatibleSvg, SvgDebugOptions, SvgPipeline, SvgPostprocessMetadata, SvgRenderOptions,
};
use crate::text::PreparedTextEvidenceLease;
use crate::wardley::WardleyDiagramLayout;
use crate::{Error, LayoutExecution, LayoutOptions, RenderCapability, Result};
use merman_core::diagrams;
use merman_core::models::class_diagram::ClassDiagram;
use merman_core::{BuiltinRenderSemantic, ParseMetadata, ParsedDiagramRender, RenderSemanticModel};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

pub use crate::render_family::RenderFamilyKind;

/// Capabilities required by one parsed typed render operation before layout starts.
///
/// Requirements come from the canonically paired semantic model and effective Mermaid config;
/// availability comes from the compiled layout backends and the operation's render session.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderCapabilityPlan {
    diagram_type: String,
    family_kind: RenderFamilyKind,
    required: Vec<RenderCapability>,
    missing: Vec<RenderCapability>,
}

impl RenderCapabilityPlan {
    /// Returns the detected Mermaid diagram type used by render dispatch.
    pub fn diagram_type(&self) -> &str {
        &self.diagram_type
    }

    /// Returns the authoritative typed render family selected before layout starts.
    pub const fn family_kind(&self) -> RenderFamilyKind {
        self.family_kind
    }

    /// Returns every optional capability this operation requires.
    pub fn required_capabilities(&self) -> &[RenderCapability] {
        &self.required
    }

    /// Returns the required capabilities unavailable in the planned render session.
    pub fn missing_capabilities(&self) -> &[RenderCapability] {
        &self.missing
    }

    /// Iterates over stable semantic IDs for every required capability.
    pub fn required_capability_ids(&self) -> impl ExactSizeIterator<Item = &'static str> + '_ {
        self.required.iter().copied().map(RenderCapability::id)
    }

    /// Iterates over stable semantic IDs for every missing capability.
    pub fn missing_capability_ids(&self) -> impl ExactSizeIterator<Item = &'static str> + '_ {
        self.missing.iter().copied().map(RenderCapability::id)
    }

    /// Reports whether the planned render session satisfies every requirement.
    pub fn is_ready(&self) -> bool {
        self.missing.is_empty()
    }

    fn ensure_available(&self) -> Result<()> {
        let Some(capability) = self.missing.first().copied() else {
            return Ok(());
        };
        Err(Error::MissingCapability {
            capability,
            diagram_type: self.diagram_type.clone(),
        })
    }
}

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
    raw: String,
    property: Option<String>,
    owner_id: String,
    class_id: Option<String>,
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
            raw: residual.raw().to_string(),
            property: residual.property().map(str::to_string),
            owner_id: provenance.owner_id().to_string(),
            class_id: provenance.class_id().map(str::to_string),
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
    family_kind: RenderFamilyKind,
    evaluation: FamilyStyleEvaluation,
    output_mutated: bool,
    theme_required: Vec<FamilyThemeMechanismKey>,
    theme_applied: Vec<FamilyThemeMechanismKey>,
    applied_capabilities: BTreeSet<ThemeCapability>,
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
            family_kind: plan.family_kind,
            evaluation,
            output_mutated: plan.output_mutated,
            theme_required: plan.theme_evidence.required.clone(),
            theme_applied: plan.theme_evidence.applied.clone(),
            applied_capabilities: plan.theme_evidence.applied_capabilities(),
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
        self.applied_capabilities.clear();
        self.native_filter_receipt = None;
        self
    }

    pub const fn family_kind(&self) -> RenderFamilyKind {
        self.family_kind
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

    pub(crate) fn applied_capabilities(
        &self,
    ) -> impl ExactSizeIterator<Item = ThemeCapability> + '_ {
        self.applied_capabilities.iter().copied()
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
                family_kind: self.family_kind,
            });
        }
        self.ensure_compatibility_portable()?;
        match self.verification() {
            FamilyStyleVerification::NotApplicable | FamilyStyleVerification::Verified => Ok(()),
            FamilyStyleVerification::Unadapted => Err(Error::UnadaptedFamilyTheme {
                family_kind: self.family_kind,
            }),
            FamilyStyleVerification::Incomplete => Err(Error::IncompleteFamilyTheme {
                family_kind: self.family_kind,
                required_count: self.theme_required.len(),
                accounted_count: self.theme_applied.len()
                    + self.theme_not_applicable.len()
                    + self.theme_residuals.len(),
            }),
            FamilyStyleVerification::Unverified => {
                if !self.theme_residuals.is_empty() {
                    return Err(Error::UnverifiedFamilyTheme {
                        family_kind: self.family_kind,
                        residual_count: self.theme_residuals.len(),
                    });
                }
                debug_assert!(!self.residuals.is_empty());
                Err(Error::UnverifiedFamilyStyle {
                    family_kind: self.family_kind,
                    residual_count: self.residuals.len(),
                })
            }
        }
    }

    fn ensure_compatibility_portable(&self) -> Result<()> {
        if self.compatibility_residual_count != 0 {
            return Err(Error::LegacyFamilyThemeCompatibility {
                family_kind: self.family_kind,
                residual_count: self.compatibility_residual_count,
            });
        }
        if self.mermaid_compatibility_residual_count != 0 {
            return Err(Error::MermaidThemeCompatibility {
                family_kind: self.family_kind,
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
    prepared_text_ledger: PreparedTextEvidenceLease,
}

impl FamilyRenderReport {
    fn freeze(
        root_theme: RootThemeReport,
        style: FamilyStyleReport,
        session: RenderSession,
        prepared_text_ledger: PreparedTextEvidenceLease,
    ) -> Self {
        Self {
            root_theme,
            style,
            session: session.report(),
            prepared_text_ledger,
        }
    }

    pub const fn family_kind(&self) -> RenderFamilyKind {
        self.style.family_kind()
    }

    pub(crate) const fn style_report(&self) -> &FamilyStyleReport {
        &self.style
    }

    pub const fn root_theme_report(&self) -> &RootThemeReport {
        &self.root_theme
    }

    /// Returns the frozen family style report carried by this completion.
    pub(crate) const fn family_style_report(&self) -> &FamilyStyleReport {
        &self.style
    }

    /// Returns source-style residuals recorded by the family adapter.
    pub(crate) fn family_style_residuals(&self) -> &[FamilyStyleResidual] {
        self.style.residuals()
    }

    pub(crate) fn applied_theme_capabilities(
        &self,
    ) -> impl ExactSizeIterator<Item = ThemeCapability> + '_ {
        self.style.applied_capabilities()
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
            self.style.family_kind,
            crate::__private::FamilyEvidenceStatus::from_verification(self.style.verification()),
            self.style.theme_required.len(),
            accounted_count,
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

    pub(crate) fn prepared_text_label_ledger(
        &self,
    ) -> &[crate::text::PreparedTextLabelLedgerEntry] {
        self.prepared_text_ledger.entries()
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
    family_kind: RenderFamilyKind,
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
    fn new(session: &RenderSession, family_kind: RenderFamilyKind) -> Self {
        let resolved_theme = session
            .theme()
            .map(|theme| Box::new(theme.resolve(family_kind)));
        let theme_evidence = FamilyThemeEvidence::from_theme(resolved_theme.as_deref());
        let payload = if !FamilyThemeEvidence::not_applicable(resolved_theme.as_deref()) {
            FamilyStylePayload::Unadapted
        } else {
            FamilyStylePayload::NotApplicable
        };
        Self {
            family_kind,
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
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> Result<()> {
        debug_assert_eq!(self.family_kind, RenderFamilyKind::State);
        let (plan, theme_evidence) = crate::state::StateStylePlan::resolve_with_evidence(
            model,
            effective_config,
            self.resolved_theme.as_deref(),
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
            self.family_kind,
            RenderFamilyKind::Flowchart | RenderFamilyKind::Swimlane
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
        debug_assert_eq!(self.family_kind, RenderFamilyKind::Sequence);
        self.theme_evidence = evidence;
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
        debug_assert_eq!(self.family_kind, RenderFamilyKind::State);
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
        if self.family_kind == RenderFamilyKind::State {
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
            self.family_kind,
            RenderFamilyKind::Flowchart | RenderFamilyKind::Swimlane | RenderFamilyKind::Sequence
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

    pub(crate) const fn family_kind(&self) -> RenderFamilyKind {
        self.family_kind
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
    family_kind: RenderFamilyKind,
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
            family_kind: style_plan.family_kind(),
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
            execution.resolved_theme().map(ResolvedDiagramTheme::family),
            session
                .theme_recipe_fingerprint()
                .map(|_| style_plan.family_kind()),
            "family theme plan must match the planned render family"
        );
        execution
    }

    pub(crate) const fn family_kind(self) -> RenderFamilyKind {
        self.family_kind
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
    pub(crate) fn for_test(session: &'a RenderSession, family_kind: RenderFamilyKind) -> Self {
        Self {
            family_kind,
            session,
            root_theme: None,
            style_plan: None,
        }
    }
}

impl FamilyRenderContext {
    fn resolve(session: RenderSession, family_kind: RenderFamilyKind) -> Self {
        let root_theme = RootThemePlan::from_theme(session.theme());
        let style_plan = ResolvedFamilyStylePlan::new(&session, family_kind);
        Self {
            root_theme,
            style_plan,
            session,
        }
    }

    const fn family_kind(&self) -> RenderFamilyKind {
        self.style_plan.family_kind()
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
        self.style_plan.adapt_state(
            model,
            effective_config,
            title,
            self.session.prepared_text_layout().is_some(),
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

    pub(crate) fn svg_label_sidecar(&self) -> &crate::flowchart::FlowchartSvgLabelSidecar {
        &self.svg_label_sidecar
    }

    pub(crate) const fn theme_evidence(&self) -> &crate::flowchart::FlowchartThemeEvidenceRecorder {
        &self.theme_evidence
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
    Class(Box<FamilyPair<ClassDiagram, ClassDiagramLayout>>),
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
    Gantt(Box<FamilyPair<diagrams::gantt::GanttDiagramRenderModel, GanttDiagramLayout>>),
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
    pub fn kind(&self) -> RenderFamilyKind {
        match self {
            Self::Error(_) => RenderFamilyKind::Error,
            Self::Mindmap(_) => RenderFamilyKind::Mindmap,
            Self::State(_) => RenderFamilyKind::State,
            Self::Sequence(_) => RenderFamilyKind::Sequence,
            Self::Zenuml(_) => RenderFamilyKind::Zenuml,
            Self::Flowchart(_) => RenderFamilyKind::Flowchart,
            Self::Swimlane(_) => RenderFamilyKind::Swimlane,
            #[cfg(feature = "layout-cytoscape")]
            Self::Architecture(_) => RenderFamilyKind::Architecture,
            Self::Class(_) => RenderFamilyKind::Class,
            Self::C4(_) => RenderFamilyKind::C4,
            Self::Cynefin(_) => RenderFamilyKind::Cynefin,
            Self::Wardley(_) => RenderFamilyKind::Wardley,
            Self::Railroad(_) => RenderFamilyKind::Railroad,
            Self::Kanban(_) => RenderFamilyKind::Kanban,
            Self::Gantt(_) => RenderFamilyKind::Gantt,
            Self::Pie(_) => RenderFamilyKind::Pie,
            Self::Packet(_) => RenderFamilyKind::Packet,
            Self::Timeline(_) => RenderFamilyKind::Timeline,
            Self::Journey(_) => RenderFamilyKind::Journey,
            Self::Requirement(_) => RenderFamilyKind::Requirement,
            Self::Sankey(_) => RenderFamilyKind::Sankey,
            Self::Radar(_) => RenderFamilyKind::Radar,
            Self::Info(_) => RenderFamilyKind::Info,
            Self::Treemap(_) => RenderFamilyKind::Treemap,
            Self::Block(_) => RenderFamilyKind::Block,
            Self::Er(_) => RenderFamilyKind::Er,
            Self::QuadrantChart(_) => RenderFamilyKind::QuadrantChart,
            Self::XyChart(_) => RenderFamilyKind::XyChart,
            Self::GitGraph(_) => RenderFamilyKind::GitGraph,
            Self::TreeView(_) => RenderFamilyKind::TreeView,
            Self::Ishikawa(_) => RenderFamilyKind::Ishikawa,
            Self::EventModeling(_) => RenderFamilyKind::EventModeling,
            Self::Venn(_) => RenderFamilyKind::Venn,
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
            Self::Class(pair) => pair.compatibility_json(metadata),
            Self::C4(pair) => pair.compatibility_json(metadata),
            Self::Cynefin(pair) => pair.compatibility_json(metadata),
            Self::Wardley(pair) => pair.compatibility_json(metadata),
            Self::Railroad(pair) => pair.compatibility_json(metadata),
            Self::Kanban(pair) => pair.compatibility_json(metadata),
            Self::Gantt(pair) => pair.compatibility_json(metadata),
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
            Self::Class(pair) => LayoutProjection::ClassDiagram(pair.layout()),
            Self::C4(pair) => LayoutProjection::C4Diagram(pair.layout()),
            Self::Cynefin(pair) => LayoutProjection::CynefinDiagram(pair.layout()),
            Self::Wardley(pair) => LayoutProjection::WardleyDiagram(pair.layout()),
            Self::Railroad(pair) => LayoutProjection::RailroadDiagram(pair.layout()),
            Self::Kanban(pair) => LayoutProjection::KanbanDiagram(pair.layout().layout()),
            Self::Gantt(pair) => LayoutProjection::GanttDiagram(pair.layout()),
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

    pub fn family_kind(&self) -> RenderFamilyKind {
        self.style_report.family_kind()
    }

    pub(crate) const fn style_report(&self) -> &FamilyStyleReport {
        &self.style_report
    }

    pub const fn root_theme_report(&self) -> &RootThemeReport {
        &self.root_theme
    }

    /// Applies an output pipeline while retaining the renderer-owned family capability.
    pub fn apply_pipeline(mut self, pipeline: &SvgPipeline) -> Result<Self> {
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
        Ok(self)
    }

    /// Finalizes the typed family output for resvg/raster consumption.
    pub fn finalize_resvg(self, pipeline: &SvgPipeline) -> Result<RenderedResvgCompatibleSvg> {
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
        let svg = pipeline
            .process_owned_resvg_compatible_with_metadata(
                source_svg,
                &output_metadata,
                &self.session,
            )?
            .attach_prepared_text_evidence(
                self.prepared_text_ledger.clone(),
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
        ensure_root_theme_portable(&root_theme, portability)?;
        if portability == ThemePortabilityRequirement::RequirePortable {
            style_report.ensure_portable()?;
        }
        let prepared_text_ledger = if prepared_text_evidence_valid {
            self.prepared_text_ledger
        } else {
            PreparedTextEvidenceLease::default()
        };
        Ok(RenderedResvgCompatibleSvg {
            svg,
            root_theme,
            style_report,
            session: self.session,
            prepared_text_ledger,
        })
    }

    fn output_metadata(&self) -> SvgPostprocessMetadata {
        SvgPostprocessMetadata::from_svg(&self.svg)
            .with_family_kind(self.family_kind())
            .with_diagram_type(self.metadata.diagram_type.clone())
            .with_optional_diagram_title(self.metadata.title.clone())
    }

    pub fn into_completion(self) -> FamilyRenderCompletion<String> {
        let Self {
            svg,
            prepared_text_svg: _,
            prepared_text_ledger,
            prepared_text_evidence_valid: _,
            root_theme,
            style_report,
            session,
            metadata: _,
        } = self;
        FamilyRenderCompletion {
            output: svg,
            report: FamilyRenderReport::freeze(
                root_theme,
                style_report,
                session,
                prepared_text_ledger,
            ),
        }
    }
}

/// Renderer-owned family output after the terminal resvg compatibility finalizer.
pub struct RenderedResvgCompatibleSvg {
    svg: ResvgCompatibleSvg,
    root_theme: RootThemeReport,
    style_report: FamilyStyleReport,
    session: RenderSession,
    prepared_text_ledger: PreparedTextEvidenceLease,
}

impl RenderedResvgCompatibleSvg {
    pub fn svg(&self) -> &ResvgCompatibleSvg {
        &self.svg
    }

    pub const fn family_kind(&self) -> RenderFamilyKind {
        self.style_report.family_kind()
    }

    pub(crate) const fn style_report(&self) -> &FamilyStyleReport {
        &self.style_report
    }

    pub const fn root_theme_report(&self) -> &RootThemeReport {
        &self.root_theme
    }

    pub fn into_completion(self) -> FamilyRenderCompletion<ResvgCompatibleSvg> {
        FamilyRenderCompletion {
            output: self.svg,
            report: FamilyRenderReport::freeze(
                self.root_theme,
                self.style_report,
                self.session,
                self.prepared_text_ledger,
            ),
        }
    }
}

impl FamilyRenderArtifact {
    fn new(
        metadata: ParseMetadata,
        family: BuiltinFamilyArtifact,
        context: FamilyRenderContext,
    ) -> Result<Self> {
        let actual_family = family.kind();
        let expected_family = context.family_kind();
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

    pub fn family_kind(&self) -> RenderFamilyKind {
        self.family.kind()
    }

    pub fn gantt_time_axis_diagnostics(&self) -> Option<GanttTimeAxisDiagnostics> {
        let BuiltinFamilyArtifact::Gantt(pair) = &self.family else {
            return None;
        };
        GanttTimeAxisDiagnostics::from_layout(pair.layout())
    }

    pub fn layout_json(&self) -> Result<serde_json::Value> {
        let semantic = self
            .compatibility_projection
            .get_or_init(|| {
                self.family
                    .compatibility_json(&self.metadata)
                    .map_err(|error| {
                        format!(
                            "failed to project {} compatibility JSON: {error}",
                            self.family.kind()
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
        Ok(serde_json::Value::Object(projection))
    }

    pub fn render_svg(
        self,
        options: &SvgRenderOptions,
        debug: &SvgDebugOptions,
    ) -> Result<RenderedFamilySvg> {
        let rendered = render_family_artifact_svg(&self, options, debug)?;
        self.context
            .session()
            .resource_policy()
            .check_svg_bytes(rendered.as_str(), ResourceLimitPhase::SvgOutput)?;
        let flowchart_theme_evidence = self
            .family
            .flowchart_theme_evidence(self.context.resolved_theme());
        let sequence_theme_evidence = self
            .family
            .sequence_theme_evidence(self.context.resolved_theme());
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
        first_residual: report.residuals().first().cloned(),
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
    layout: impl FnOnce(
        &diagrams::flowchart::FlowchartModel,
        &diagrams::flowchart::FlowchartRenderLabelSources,
        &crate::flowchart::FlowchartSvgLabelSidecarBuilder,
    ) -> Result<L>,
) -> Result<Box<FlowchartFamilyArtifact<L>>> {
    let svg_label_sidecar = crate::flowchart::FlowchartSvgLabelSidecarBuilder::new_with_work_meter(
        prepared_text_layout,
        resolved_theme,
        work_meter,
    )
    .with_typography_config_ownership(typography_config_ownership);
    let layout = layout(&semantic, &label_sources, &svg_label_sidecar)?;
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
        svg_label_sidecar,
        theme_evidence: crate::flowchart::FlowchartThemeEvidenceRecorder::default(),
    }))
}

fn semantic_flowchart_requires_math(model: &diagrams::flowchart::FlowchartModel) -> bool {
    model
        .nodes
        .iter()
        .filter_map(|node| node.label.as_deref())
        .chain(model.edges.iter().filter_map(|edge| edge.label.as_deref()))
        .chain(
            model
                .subgraphs
                .iter()
                .map(|subgraph| subgraph.title.as_str()),
        )
        .any(crate::math::contains_delimited_math)
}

fn sequence_requires_math(model: &diagrams::sequence::SequenceDiagramRenderModel) -> bool {
    model
        .actors
        .values()
        .map(|actor| actor.description.as_str())
        .chain(model.messages.iter().map(|message| message.message_text()))
        .chain(model.notes.iter().map(|note| note.message.as_str()))
        .any(crate::math::contains_delimited_math)
}

fn mindmap_requires_math(model: &diagrams::mindmap::MindmapDiagramRenderModel) -> bool {
    model
        .nodes
        .iter()
        .map(|node| node.label.as_str())
        .any(crate::math::contains_delimited_math)
}

fn parsed_render_requires_math(parsed: &ParsedDiagramRender) -> bool {
    match parsed.model() {
        RenderSemanticModel::Class(model) => crate::class::class_requires_math(model),
        RenderSemanticModel::Flowchart(model) => {
            parsed.flowchart_render_label_sources().map_or_else(
                || semantic_flowchart_requires_math(model),
                |label_sources| {
                    crate::flowchart::FlowchartRenderModelRef::new(model, label_sources)
                        .requires_math()
                },
            )
        }
        RenderSemanticModel::Mindmap(model) => mindmap_requires_math(model),
        RenderSemanticModel::Sequence(model) => sequence_requires_math(model),
        _ => false,
    }
}

fn capability_is_available(capability: RenderCapability, session: &RenderSession) -> bool {
    session.supports_capability(capability)
}

fn required_capabilities(parsed: &ParsedDiagramRender) -> Vec<RenderCapability> {
    let mut required = Vec::with_capacity(2);
    let meta = parsed.metadata();
    let model = parsed.model();
    let effective_config = &meta.effective_config;
    match model {
        RenderSemanticModel::Architecture(_) => {
            required.push(RenderCapability::LayoutCytoscape);
        }
        RenderSemanticModel::Mindmap(_)
            if !crate::mindmap::uses_tidy_tree_layout(effective_config.as_value()) =>
        {
            required.push(RenderCapability::LayoutCytoscape);
        }
        RenderSemanticModel::Flowchart(_) | RenderSemanticModel::Class(_)
            if crate::uses_elk_layout(effective_config) =>
        {
            required.push(RenderCapability::LayoutElk);
        }
        RenderSemanticModel::Er(_) if crate::er::uses_elk_layout(effective_config.as_value()) => {
            required.push(RenderCapability::LayoutElk);
        }
        _ => {}
    }

    if parsed_render_requires_math(parsed) {
        required.push(RenderCapability::Math);
    }
    required
}

fn detect_render_family(parsed: &ParsedDiagramRender) -> Option<RenderFamilyKind> {
    let effective_config = &parsed.metadata().effective_config;
    Some(match parsed.model() {
        RenderSemanticModel::Error(_) => RenderFamilyKind::Error,
        RenderSemanticModel::Mindmap(_) => RenderFamilyKind::Mindmap,
        RenderSemanticModel::State(_) => RenderFamilyKind::State,
        RenderSemanticModel::Sequence(_) => RenderFamilyKind::Sequence,
        RenderSemanticModel::Zenuml(_) => RenderFamilyKind::Zenuml,
        RenderSemanticModel::Flowchart(_)
            if effective_config.get_str("layout") == Some("swimlane") =>
        {
            RenderFamilyKind::Swimlane
        }
        RenderSemanticModel::Flowchart(_) => RenderFamilyKind::Flowchart,
        RenderSemanticModel::Architecture(_) => RenderFamilyKind::Architecture,
        RenderSemanticModel::Class(_) => RenderFamilyKind::Class,
        RenderSemanticModel::C4(_) => RenderFamilyKind::C4,
        RenderSemanticModel::Cynefin(_) => RenderFamilyKind::Cynefin,
        RenderSemanticModel::Wardley(_) => RenderFamilyKind::Wardley,
        RenderSemanticModel::Railroad(_) => RenderFamilyKind::Railroad,
        RenderSemanticModel::Kanban(_) => RenderFamilyKind::Kanban,
        RenderSemanticModel::Gantt(_) => RenderFamilyKind::Gantt,
        RenderSemanticModel::Pie(_) => RenderFamilyKind::Pie,
        RenderSemanticModel::Packet(_) => RenderFamilyKind::Packet,
        RenderSemanticModel::Timeline(_) => RenderFamilyKind::Timeline,
        RenderSemanticModel::Journey(_) => RenderFamilyKind::Journey,
        RenderSemanticModel::Requirement(_) => RenderFamilyKind::Requirement,
        RenderSemanticModel::Sankey(_) => RenderFamilyKind::Sankey,
        RenderSemanticModel::Radar(_) => RenderFamilyKind::Radar,
        RenderSemanticModel::Info(_) => RenderFamilyKind::Info,
        RenderSemanticModel::Treemap(_) => RenderFamilyKind::Treemap,
        RenderSemanticModel::Block(_) => RenderFamilyKind::Block,
        RenderSemanticModel::Er(_) => RenderFamilyKind::Er,
        RenderSemanticModel::QuadrantChart(_) => RenderFamilyKind::QuadrantChart,
        RenderSemanticModel::XyChart(_) => RenderFamilyKind::XyChart,
        RenderSemanticModel::GitGraph(_) => RenderFamilyKind::GitGraph,
        RenderSemanticModel::TreeView(_) => RenderFamilyKind::TreeView,
        RenderSemanticModel::Ishikawa(_) => RenderFamilyKind::Ishikawa,
        RenderSemanticModel::EventModeling(_) => RenderFamilyKind::EventModeling,
        RenderSemanticModel::Venn(_) => RenderFamilyKind::Venn,
        RenderSemanticModel::CustomJson(_) => return None,
    })
}

fn validate_render_input(
    parsed: &ParsedDiagramRender,
    session: &RenderSession,
) -> Result<RenderFamilyKind> {
    let meta = parsed.metadata();
    let model = parsed.model();
    if !merman_core::__private::theme_parse_evidence(meta)
        .matches_recipe(session.theme_compatibility_recipe())
    {
        return Err(Error::ThemeParseBindingMismatch);
    }
    let diagram_type = meta.diagram_type.as_str();
    if let RenderSemanticModel::CustomJson(custom) = model {
        return Err(Error::NonRenderableCustomModel {
            diagram_type: meta.diagram_type.clone(),
            model_name: custom.model_name().to_string(),
            provenance: custom.provenance(),
        });
    }

    if !model.supports_diagram_type(diagram_type) {
        return Err(Error::InvalidModel {
            message: format!(
                "unexpected render model variant {} for diagram type: {diagram_type}",
                model.kind()
            ),
        });
    }

    session.resource_policy().check_parsed_render(parsed)?;
    detect_render_family(parsed).ok_or_else(|| Error::InvalidModel {
        message: format!(
            "render model variant {} has no built-in family for diagram type: {diagram_type}",
            model.kind()
        ),
    })
}

/// Plans capability admission for a canonically paired typed render model without running layout.
pub fn plan_render(
    parsed: &ParsedDiagramRender,
    session: &RenderSession,
) -> Result<RenderCapabilityPlan> {
    let meta = parsed.metadata();
    let family_kind = validate_render_input(parsed, session)?;
    let required = required_capabilities(parsed);
    let missing = required
        .iter()
        .copied()
        .filter(|capability| !capability_is_available(*capability, session))
        .collect();
    Ok(RenderCapabilityPlan {
        diagram_type: meta.diagram_type.clone(),
        family_kind,
        required,
        missing,
    })
}

#[inline(never)]
fn prepare_class_family(
    model: ClassDiagram,
    meta: &ParseMetadata,
    diagram_type: &str,
    execution: &LayoutExecution<'_>,
) -> Result<BuiltinFamilyArtifact> {
    Ok(BuiltinFamilyArtifact::Class(prepare_pair(
        model,
        |model| {
            crate::layout_class_typed_by_engine(
                diagram_type,
                model,
                &meta.effective_config,
                execution,
            )
        },
    )?))
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
    context.ensure_portable()?;
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
    if let Some(error) = session.text_layout_error().cloned() {
        return Err(error.into());
    }
    let plan = plan_render(&parsed, &session)?;
    plan.ensure_available()?;
    let expected_family = plan.family_kind();
    let context = FamilyRenderContext::resolve(session, expected_family);
    // The heterogeneous router has one generic layout call per family. Keep its debug-build
    // caller slots out of the Class Dagre call chain, whose own phase frames are already deep.
    if expected_family == RenderFamilyKind::Class {
        prepare_class_render(parsed, options, context)
    } else {
        prepare_non_class_render(parsed, options, context)
    }
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
                    effective_config,
                    execution.text_measurer(),
                    execution.math_renderer(),
                    execution.work_meter_ref(),
                )
            })?)
        }
        RenderSemanticModel::Zenuml(model) => {
            BuiltinFamilyArtifact::Zenuml(prepare_pair(model, |model| {
                crate::zenuml::layout_zenuml_diagram_typed(model, execution.text_measurer())
            })?)
        }
        RenderSemanticModel::Flowchart(model) => match execution.family_kind() {
            RenderFamilyKind::Swimlane => {
                BuiltinFamilyArtifact::Swimlane(prepare_flowchart_artifact(
                    model,
                    flowchart_label_sources,
                    execution.prepared_text_layout(),
                    execution.resolved_theme(),
                    crate::flowchart::flowchart_typography_config_ownership(&meta.effective_config),
                    execution.work_meter(),
                    |model, label_sources, svg_label_sidecar| {
                        crate::swimlane::layout_swimlane_typed_with_work_meter_and_svg_label_sidecar(
                            model,
                            label_sources,
                            &meta.effective_config,
                            execution.text_measurer(),
                            execution.math_renderer(),
                            Some(svg_label_sidecar),
                            execution.work_meter(),
                        )
                    },
                )?)
            }
            RenderFamilyKind::Flowchart => {
                BuiltinFamilyArtifact::Flowchart(prepare_flowchart_artifact(
                    model,
                    flowchart_label_sources,
                    execution.prepared_text_layout(),
                    execution.resolved_theme(),
                    crate::flowchart::flowchart_typography_config_ownership(&meta.effective_config),
                    execution.work_meter(),
                    |model, label_sources, svg_label_sidecar| {
                        crate::layout_flowchart_typed_with_render_labels_and_svg_label_sidecar_by_engine(
                            diagram_type,
                            model,
                            label_sources,
                            &meta.effective_config,
                            &execution,
                            Some(svg_label_sidecar),
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
                capability: RenderCapability::LayoutCytoscape,
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
            BuiltinFamilyArtifact::Gantt(prepare_pair(model, |model| {
                crate::gantt::layout_gantt_diagram_typed(
                    model,
                    title,
                    effective_config,
                    execution.text_measurer(),
                    execution.container_width,
                    execution.local_time_zone(),
                )
            })?)
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
mod tests {
    use super::*;
    use crate::diagram_theme::{
        BlendMode, CanvasLayer, CanvasPaint, CanvasSpec, DiagramTheme, DiagramThemeCompiler,
        DiagramThemeSpec, FontStack, GradientStop, LinearGradient, MermaidThemeCompatibility,
        OrdinalPalette, OrdinalSelector, RootThemeEvaluation, RootThemeMechanismKey,
        RootThemeVerification, Specified, TextStylePatch, ThemeCapability, ThemeColorValue,
        ThemeGeometryPatch, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle,
        TypographySpec,
    };
    #[cfg(feature = "layout-cytoscape")]
    use std::sync::Arc;
    use std::sync::Mutex;

    use crate::environment::{
        HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
        TextMeasurementOperation, TextMeasurementPhase, TextMeasurementResultKind,
    };
    #[cfg(feature = "layout-cytoscape")]
    use crate::environment::{
        MeasurementProfileId, TextMeasurementPolicy, TextMeasurementProfileIdentity,
    };
    use crate::svg::SvgPipelinePreset;
    use crate::text::{TextMetrics, WrapMode};
    use merman_core::__private::{
        ThemeCompatibilityPlan, ThemeFamilyCompatibilityOverlayBuilder,
        install_theme_compatibility, theme_parse_evidence,
    };
    use merman_core::{
        CustomJsonProvenance, CustomJsonRenderModel, Engine, MermaidConfig, ParseOptions,
    };
    use serde_json::{Value, json};

    fn custom_semantic_parser(
        _code: &str,
        meta: &ParseMetadata,
        control: &merman_core::ParseControl,
    ) -> merman_core::ParseControlResult<merman_core::Result<Value>> {
        control.checkpoint()?;
        Ok(Ok(
            json!({ "type": meta.diagram_type, "owner": "semantic" }),
        ))
    }

    fn custom_render_parser(
        _code: &str,
        _meta: &ParseMetadata,
    ) -> merman_core::Result<CustomJsonRenderModel> {
        Ok(CustomJsonRenderModel::new(
            "custom-flowchart",
            json!({ "owner": "render" }),
        ))
    }

    fn session() -> RenderSession {
        crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap()
    }

    fn flowchart_node_theme(style: ThemeStylePatch) -> DiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(ThemeTarget::Node, style)
                            .for_family(RenderFamilyKind::Flowchart),
                    ),
                ),
            )
            .expect("compile Flowchart Node theme")
    }

    fn flowchart_node_stroke_width_theme(
        family: RenderFamilyKind,
        stroke_width: f32,
    ) -> DiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_stroke_width(stroke_width)
                                .expect("valid Flowchart stroke width"),
                        )
                        .for_family(family),
                    ),
                ),
            )
            .expect("compile Flowchart Node stroke-width theme")
    }

    fn flowchart_node_stroke_dasharray_theme(
        family: RenderFamilyKind,
        stroke_dasharray: impl IntoIterator<Item = f32>,
    ) -> DiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_stroke_dasharray(stroke_dasharray)
                                .expect("valid Flowchart stroke dasharray"),
                        )
                        .for_family(family),
                    ),
                ),
            )
            .expect("compile Flowchart Node stroke-dasharray theme")
    }

    fn flowchart_node_radius_theme(family: RenderFamilyKind, radius: f32) -> DiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch {
                                geometry: ThemeGeometryPatch {
                                    radius: Specified::Value(radius),
                                },
                                ..ThemeStylePatch::default()
                            },
                        )
                        .for_family(family),
                    ),
                ),
            )
            .expect("compile Flowchart Node radius theme")
    }

    fn flowchart_node_label_font_stack_theme(family: RenderFamilyKind) -> DiagramTheme {
        flowchart_node_label_font_stack_theme_with_base(family, None)
    }

    fn flowchart_node_label_font_stack_theme_with_base(
        family: RenderFamilyKind,
        base_font_stack: Option<FontStack>,
    ) -> DiagramTheme {
        flowchart_node_label_typography_theme(family, base_font_stack, None, None)
    }

    fn flowchart_node_label_font_stack_and_size_theme_with_base(
        family: RenderFamilyKind,
        base_font_size_px: Option<f32>,
    ) -> DiagramTheme {
        flowchart_node_label_typography_theme(family, None, Some(26.0), base_font_size_px)
    }

    fn flowchart_node_label_font_size_theme_without_catalog(
        family: RenderFamilyKind,
    ) -> DiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::NodeLabel,
                            ThemeStylePatch {
                                typography: TextStylePatch {
                                    font_size_px: Specified::Value(26.0),
                                    ..TextStylePatch::default()
                                },
                                ..ThemeStylePatch::default()
                            },
                        )
                        .for_family(family),
                    ),
                ),
            )
            .expect("compile Flowchart NodeLabel font-size theme without a font catalog")
    }

    fn flowchart_node_label_typography_theme(
        family: RenderFamilyKind,
        base_font_stack: Option<FontStack>,
        node_label_font_size_px: Option<f32>,
        base_font_size_px: Option<f32>,
    ) -> DiagramTheme {
        let latin = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        let cjk = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Xiaolai-Regular-CJK-Test.woff2"
        ));
        let mut styles = ThemeRuleSet::default().with_rule(
            ThemeRule::new(
                ThemeTarget::NodeLabel,
                ThemeStylePatch {
                    typography: TextStylePatch {
                        font_stack: Specified::Value(
                            FontStack::single("Excalifont").expect("valid fixture font stack"),
                        ),
                        ..TextStylePatch::default()
                    },
                    ..ThemeStylePatch::default()
                },
            )
            .for_family(family),
        );
        if let Some(font_size_px) = node_label_font_size_px {
            styles = styles.with_rule(
                ThemeRule::new(
                    ThemeTarget::NodeLabel,
                    ThemeStylePatch {
                        typography: TextStylePatch {
                            font_size_px: Specified::Value(font_size_px),
                            ..TextStylePatch::default()
                        },
                        ..ThemeStylePatch::default()
                    },
                )
                .for_family(family),
            );
        }
        let mut spec = DiagramThemeSpec::new().with_styles(styles).with_assets(
            crate::diagram_theme::ThemeAssets::default().with_font_catalog(
                crate::diagram_theme::FontCatalogSpec::new([
                    crate::diagram_theme::FontAssetSpec::new("excalifont", latin),
                    crate::diagram_theme::FontAssetSpec::new("xiaolai", cjk),
                ]),
            ),
        );
        if base_font_stack.is_some() || base_font_size_px.is_some() {
            let mut base = ThemeTextStyle::default();
            if let Some(base_font_stack) = base_font_stack {
                base = base.with_font_stack(base_font_stack);
            }
            if let Some(base_font_size_px) = base_font_size_px {
                base = base
                    .with_font_size_px(base_font_size_px)
                    .expect("valid base font size");
            }
            spec = spec.with_typography(TypographySpec::default().with_default(base));
        }
        DiagramThemeCompiler::new()
            .compile(spec)
            .expect("compile Flowchart NodeLabel font-stack theme")
    }

    fn flowchart_node_shape_style(svg: &str, node_id: &str) -> String {
        let document = roxmltree::Document::parse(svg).expect("valid Flowchart SVG");
        let node_id_marker = format!("-flowchart-{node_id}-");
        let wrapper = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node
                        .attribute("id")
                        .is_some_and(|id| id.contains(&node_id_marker))
            })
            .unwrap_or_else(|| panic!("Flowchart node wrapper for {node_id}"));
        wrapper
            .descendants()
            .find(|node| {
                node.is_element()
                    && node.attribute("class").is_some_and(|class| {
                        class
                            .split_ascii_whitespace()
                            .any(|part| part == "label-container")
                    })
            })
            .and_then(|node| node.attribute("style"))
            .unwrap_or_else(|| panic!("Flowchart node shape style for {node_id}"))
            .to_string()
    }

    fn flowchart_node_shape_attribute(svg: &str, node_id: &str, attribute: &str) -> Option<String> {
        let document = roxmltree::Document::parse(svg).expect("valid Flowchart SVG");
        let node_id_marker = format!("-flowchart-{node_id}-");
        let wrapper = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node
                        .attribute("id")
                        .is_some_and(|id| id.contains(&node_id_marker))
            })
            .unwrap_or_else(|| panic!("Flowchart node wrapper for {node_id}"));
        wrapper
            .descendants()
            .find(|node| {
                node.is_element()
                    && node.attribute("class").is_some_and(|class| {
                        class
                            .split_ascii_whitespace()
                            .any(|part| part == "label-container")
                    })
            })
            .and_then(|node| node.attribute(attribute))
            .map(str::to_string)
    }

    fn flowchart_node_label_style(svg: &str, node_id: &str) -> String {
        let document = roxmltree::Document::parse(svg).expect("valid Flowchart SVG");
        let node_id_marker = format!("-flowchart-{node_id}-");
        let wrapper = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node
                        .attribute("id")
                        .is_some_and(|id| id.contains(&node_id_marker))
            })
            .unwrap_or_else(|| panic!("Flowchart node wrapper for {node_id}"));
        wrapper
            .descendants()
            .find(|node| {
                node.is_element()
                    && node.attribute("class").is_some_and(|class| {
                        class.split_ascii_whitespace().any(|part| part == "label")
                    })
            })
            .and_then(|node| node.attribute("style"))
            .unwrap_or_else(|| panic!("Flowchart node label style for {node_id}"))
            .to_string()
    }

    fn family_report(
        evaluation: FamilyStyleEvaluation,
        required: Vec<FamilyThemeMechanismKey>,
        applied: Vec<FamilyThemeMechanismKey>,
        theme_residuals: Vec<FamilyThemeResidual>,
    ) -> FamilyStyleReport {
        FamilyStyleReport {
            family_kind: RenderFamilyKind::State,
            evaluation,
            output_mutated: false,
            theme_required: required,
            theme_applied: applied,
            applied_capabilities: BTreeSet::new(),
            native_filter_receipt: None,
            theme_not_applicable: Vec::new(),
            theme_residuals,
            compatibility_residual_count: 0,
            mermaid_compatibility_residual_count: 0,
            residuals: Vec::new(),
        }
    }

    #[test]
    fn family_theme_verification_state_matrix_is_fail_closed() {
        let key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::State,
        };
        let residual = FamilyThemeResidual {
            key: key.clone(),
            reason: FamilyThemeResidualReason::UnsupportedPaint,
        };

        assert_eq!(
            family_report(FamilyStyleEvaluation::NotApplicable, vec![], vec![], vec![])
                .verification(),
            FamilyStyleVerification::NotApplicable
        );
        assert_eq!(
            family_report(
                FamilyStyleEvaluation::Unadapted,
                vec![key.clone()],
                vec![],
                vec![residual.clone()],
            )
            .verification(),
            FamilyStyleVerification::Unadapted
        );
        assert_eq!(
            family_report(
                FamilyStyleEvaluation::Evaluated,
                vec![key.clone()],
                vec![key.clone()],
                vec![],
            )
            .verification(),
            FamilyStyleVerification::Verified
        );
        assert_eq!(
            family_report(
                FamilyStyleEvaluation::Evaluated,
                vec![key.clone()],
                vec![],
                vec![residual],
            )
            .verification(),
            FamilyStyleVerification::Unverified
        );
        assert_eq!(
            family_report(
                FamilyStyleEvaluation::Evaluated,
                vec![key.clone()],
                vec![],
                vec![],
            )
            .verification(),
            FamilyStyleVerification::Incomplete
        );
        assert_eq!(
            family_report(
                FamilyStyleEvaluation::Evaluated,
                vec![key.clone(), key.clone()],
                vec![key.clone()],
                vec![],
            )
            .verification(),
            FamilyStyleVerification::Incomplete
        );

        let error = family_report(FamilyStyleEvaluation::Evaluated, vec![key], vec![], vec![])
            .ensure_portable()
            .expect_err("strict portability must reject incomplete family evidence");
        assert_eq!(
            error.incomplete_family_theme(),
            Some((RenderFamilyKind::State, 1, 0))
        );

        let mut mermaid_compatibility =
            family_report(FamilyStyleEvaluation::NotApplicable, vec![], vec![], vec![]);
        mermaid_compatibility.mermaid_compatibility_residual_count = 1;
        assert_eq!(
            mermaid_compatibility.verification(),
            FamilyStyleVerification::Unverified
        );
        assert!(matches!(
            mermaid_compatibility.ensure_portable(),
            Err(Error::MermaidThemeCompatibility {
                family_kind: RenderFamilyKind::State,
                residual_count: 1,
            })
        ));
    }

    #[test]
    fn require_portable_rejects_explicit_mermaid_compatibility_in_the_low_level_api() {
        let compatibility = MermaidThemeCompatibility::default()
            .with_variable("primaryColor", "#ef4444")
            .expect("valid Mermaid compatibility value");
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_mermaid_compatibility(compatibility))
            .expect("compile explicit Mermaid compatibility theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\nReady --> Done\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("State source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable session");

        let error = match prepare(parsed, &LayoutOptions::default(), session) {
            Ok(_) => {
                panic!("strict low-level rendering must reject explicit Mermaid compatibility")
            }
            Err(error) => error,
        };
        assert!(matches!(
            error,
            Error::MermaidThemeCompatibility {
                family_kind: RenderFamilyKind::State,
                residual_count: 1,
            }
        ));
    }

    #[test]
    fn unknown_fallback_contributions_are_always_portability_residuals() {
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new())
            .expect("compile empty typed theme");
        let mut overlay =
            ThemeFamilyCompatibilityOverlayBuilder::new("state", "test.compatibility.");
        overlay
            .try_push(
                "unknown-fallback",
                MermaidConfig::from_value(json!({
                    "state": {"titleTopMargin": 77}
                })),
            )
            .expect("bounded fallback contribution");
        let overlay = overlay.finish();
        let plan = ThemeCompatibilityPlan::try_new(
            *theme.recipe_fingerprint().as_bytes(),
            MermaidConfig::empty_object(),
            move |family, _control| Ok((family == "state").then(|| overlay.clone())),
        )
        .expect("bounded compatibility plan");
        let parsed = install_theme_compatibility(Engine::new(), &plan)
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\nReady --> Done\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("State source should produce a render model");
        assert_eq!(
            parsed.metadata().effective_config.as_value()["state"]["titleTopMargin"],
            json!(77)
        );
        assert_eq!(
            theme_parse_evidence(parsed.metadata()).fallback_contribution_count(),
            1
        );
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable session");

        let error = match prepare(parsed, &LayoutOptions::default(), session) {
            Ok(_) => panic!("unknown fallback config must remain an explicit residual"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            Error::LegacyFamilyThemeCompatibility {
                family_kind: RenderFamilyKind::State,
                residual_count: 1,
                ..
            }
        ));
    }

    struct RejectingTextLayoutBackend {
        identity: crate::text::TextLayoutBackendIdentity,
    }

    impl crate::text::TextLayoutBackend for RejectingTextLayoutBackend {
        fn identity(&self) -> &crate::text::TextLayoutBackendIdentity {
            &self.identity
        }

        fn capabilities(&self) -> crate::text::TextLayoutCapabilities {
            crate::text::TextLayoutCapabilities::native()
        }

        fn prepare_catalog(
            &self,
            _request: &crate::text::PrepareCatalogRequest,
        ) -> std::result::Result<
            crate::text::PreparedTextLayoutResponse,
            crate::text::TextLayoutError,
        > {
            Err(crate::text::TextLayoutError::BackendRejected)
        }
    }

    #[test]
    fn custom_catalog_preparation_failure_uses_the_native_catalog_fallback() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        let catalog =
            crate::diagram_theme::FontCatalogSpec::new([crate::diagram_theme::FontAssetSpec::new(
                "excalifont",
                bytes,
            )])
            .compile(&crate::diagram_theme::ThemeResourcePolicy::interactive())
            .expect("fixture catalog should compile");
        let backend = RejectingTextLayoutBackend {
            identity: crate::text::TextLayoutBackendIdentity::new("test.rejecting", "v1")
                .expect("test backend identity"),
        };
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_font_catalog(catalog)
            .with_text_layout_backend(std::sync::Arc::new(backend))
            .begin_session()
            .expect("runtime session should still capture preparation evidence");
        assert_eq!(session.text_layout_error(), None);
        let session_report = session.report();
        let prepared_report = session_report
            .prepared_text_layout()
            .expect("native fallback should prepare the custom catalog");
        assert!(prepared_report.face_count() > 0);
        assert_eq!(prepared_report.failed_attempt_count(), 1);

        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync("info", ParseOptions::strict())
            .expect("parse info")
            .expect("info render model");
        prepare(parsed, &LayoutOptions::default(), session)
            .expect("native fallback should allow layout to continue");
    }

    #[test]
    fn capability_plan_reports_the_authoritative_render_family() {
        let configured_swimlane = Engine::new().with_site_config(
            merman_core::MermaidConfig::from_value(json!({ "layout": "swimlane" })),
        );
        let cases = [
            (
                Engine::new(),
                "flowchart TD\nA --> B\n",
                RenderFamilyKind::Flowchart,
            ),
            (
                Engine::new(),
                "swimlane-beta LR\nA --> B\n",
                RenderFamilyKind::Swimlane,
            ),
            (
                configured_swimlane,
                "flowchart LR\nA --> B\n",
                RenderFamilyKind::Swimlane,
            ),
        ];

        for (engine, source, expected_family) in cases {
            let parsed = engine
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("family fixture should produce a render model");
            let plan = plan_render(&parsed, &session()).unwrap();

            assert_eq!(plan.family_kind(), expected_family, "{source}");
        }
    }

    #[test]
    fn family_theme_plan_tracks_the_authoritative_family() {
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new())
            .expect("compile test theme");
        let configured_swimlane = Engine::new().with_site_config(
            merman_core::MermaidConfig::from_value(json!({ "layout": "swimlane" })),
        );
        let cases = [
            (
                Engine::new(),
                "flowchart TD\nA --> B\n",
                RenderFamilyKind::Flowchart,
            ),
            (
                configured_swimlane,
                "flowchart LR\nA --> B\n",
                RenderFamilyKind::Swimlane,
            ),
        ];

        for (engine, source, expected_family) in cases {
            let parsed = theme
                .install_parse_compatibility(engine)
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("family fixture should produce a render model");
            let session = crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin themed render session");
            let artifact = prepare(parsed, &LayoutOptions::default(), session).unwrap();
            let resolved_theme = artifact
                .context
                .resolved_theme()
                .expect("themed artifact should retain a resolved family plan");

            assert_eq!(artifact.family_kind(), expected_family, "{source}");
            assert_eq!(artifact.context.family_kind(), expected_family, "{source}");
            assert_eq!(resolved_theme.family(), expected_family, "{source}");
            assert_eq!(
                artifact.context.session().theme_recipe_fingerprint(),
                Some(theme.recipe_fingerprint()),
                "{source}"
            );
        }
    }

    #[test]
    fn family_render_report_freezes_after_pipeline_and_terminal_svg() {
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new())
            .expect("compile test theme");
        let engine = Engine::new().with_site_config(merman_core::MermaidConfig::from_value(
            json!({ "layout": "swimlane" }),
        ));
        let parsed = theme
            .install_parse_compatibility(engine)
            .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
            .unwrap()
            .expect("flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin themed render session");
        let artifact = prepare(parsed, &LayoutOptions::default(), session).unwrap();
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render themed family SVG");
        assert_eq!(rendered.family_kind(), RenderFamilyKind::Swimlane);
        assert_eq!(
            rendered.root_theme_report().evaluation(),
            RootThemeEvaluation::NotApplicable
        );
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::NotApplicable
        );
        assert_eq!(
            rendered.style_report().evaluation(),
            FamilyStyleEvaluation::NotApplicable
        );
        assert_eq!(
            rendered.session.theme_recipe_fingerprint(),
            Some(theme.recipe_fingerprint())
        );

        let rendered = rendered
            .apply_pipeline(&SvgPipeline::parity())
            .expect("apply themed family SVG pipeline");
        assert_eq!(rendered.family_kind(), RenderFamilyKind::Swimlane);
        assert_eq!(
            rendered.session.theme_recipe_fingerprint(),
            Some(theme.recipe_fingerprint())
        );

        let finalized = rendered
            .finalize_resvg(&SvgPipeline::resvg_safe())
            .expect("finalize themed family SVG");
        assert_eq!(finalized.family_kind(), RenderFamilyKind::Swimlane);
        assert!(!finalized.style_report().is_verified());
        assert_eq!(
            finalized.session.theme_recipe_fingerprint(),
            Some(theme.recipe_fingerprint())
        );

        let completion = finalized.into_completion();
        assert_eq!(
            completion.output().finalization_report().preset(),
            SvgPipelinePreset::ResvgSafe
        );
        assert_eq!(
            completion.report().family_kind(),
            RenderFamilyKind::Swimlane
        );
        assert_eq!(
            completion.report().theme_recipe_fingerprint(),
            Some(theme.recipe_fingerprint())
        );
        assert_eq!(
            completion
                .report()
                .session_report()
                .theme_recipe_fingerprint(),
            Some(theme.recipe_fingerprint())
        );
        assert_eq!(
            completion.report().style_report(),
            &FamilyStyleReport {
                family_kind: RenderFamilyKind::Swimlane,
                evaluation: FamilyStyleEvaluation::NotApplicable,
                output_mutated: false,
                theme_required: Vec::new(),
                theme_applied: Vec::new(),
                applied_capabilities: BTreeSet::new(),
                native_filter_receipt: None,
                theme_not_applicable: Vec::new(),
                theme_residuals: Vec::new(),
                compatibility_residual_count: 0,
                mermaid_compatibility_residual_count: 0,
                residuals: Vec::new(),
            }
        );
        assert_eq!(
            completion.report().root_theme_report().evaluation(),
            RootThemeEvaluation::NotApplicable
        );
    }

    #[test]
    fn solid_root_layer_is_applied_by_terminal_svg_completion() {
        let layer = CanvasLayer::new(CanvasPaint::solid("#ef4444").expect("valid layer paint"))
            .with_opacity(0.5)
            .expect("valid layer opacity")
            .with_offset(4.0, -2.0)
            .expect("valid layer offset")
            .with_blend_mode(BlendMode::Multiply);
        let canvas = CanvasSpec::default()
            .with_layer(layer)
            .expect("bounded canvas layer");
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_canvas(canvas))
            .expect("compile layered canvas theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort themed session");
        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("root theme should not stop layout")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render themed SVG");

        assert_eq!(
            rendered.root_theme_report().evaluation(),
            RootThemeEvaluation::Evaluated
        );
        assert_eq!(
            rendered.root_theme_report().verification(),
            RootThemeVerification::Verified
        );
        assert!(
            rendered
                .root_theme_report()
                .applied_mechanisms()
                .contains(&RootThemeMechanismKey::CanvasLayer { index: 0 })
        );
        assert!(rendered.root_theme_report().residuals().is_empty());
        assert!(rendered.svg().contains(
            r#"data-merman-theme-canvas-layer="0" aria-hidden="true" pointer-events="none""#
        ));
        assert!(rendered.svg().contains(r#"opacity="0.5""#));
        assert!(rendered.svg().contains(r#"transform="translate(4 -2)""#));
        assert!(
            rendered
                .svg()
                .contains(r#"style="mix-blend-mode:multiply""#)
        );
        assert!(rendered.svg().contains(r##"fill="#ef4444""##));

        let completion = rendered
            .finalize_resvg(&SvgPipeline::resvg_safe())
            .expect("finalize themed SVG")
            .into_completion();
        let root_report = completion.report().root_theme_report();
        assert!(root_report.residuals().is_empty());
        assert_eq!(root_report.verification(), RootThemeVerification::Verified);
        assert!(root_report.coverage_complete());
    }

    #[test]
    fn solid_root_base_is_applied_by_terminal_svg_completion() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_canvas(CanvasSpec::solid("#111827").expect("valid canvas base")),
            )
            .expect("compile solid canvas theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin themed session");
        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("solid canvas should not stop layout")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render themed SVG");

        assert_eq!(
            rendered.root_theme_report().verification(),
            RootThemeVerification::Verified
        );
        assert!(
            rendered
                .root_theme_report()
                .applied_mechanisms()
                .contains(&RootThemeMechanismKey::CanvasBase)
        );
        assert!(
            rendered
                .svg()
                .contains(r#"class="merman-theme-canvas-base" data-merman-theme-canvas="base""#)
        );
        assert!(rendered.svg().contains(r##"fill="#111827""##));
    }

    #[test]
    fn explicit_transparent_root_base_clears_the_mermaid_white_background() {
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_canvas(CanvasSpec::transparent()))
            .expect("compile transparent canvas theme");
        assert!(
            theme
                .report()
                .requires_capability(ThemeCapability::TransparentPaint)
        );
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin transparent themed session");
        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("transparent canvas should not stop layout")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render transparent themed SVG");

        assert_eq!(
            rendered.root_theme_report().verification(),
            RootThemeVerification::Verified
        );
        assert!(rendered.svg().contains("background-color: transparent;"));
        assert!(!rendered.svg().contains("background-color: white;"));
        assert!(
            rendered
                .svg()
                .contains(r#"data-merman-theme-canvas="base""#)
        );
        assert!(rendered.svg().contains(r#"fill="none""#));
    }

    #[test]
    fn root_theme_evidence_is_invalidated_by_an_untrusted_svg_postprocessor() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_canvas(CanvasSpec::solid("#111827").expect("valid canvas paint")),
            )
            .expect("compile root canvas theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin themed render session");
        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("prepare themed family")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render themed SVG")
            .apply_pipeline(
                &SvgPipeline::parity()
                    .with_postprocessor(crate::svg::RootBackgroundPostprocessor::new("white")),
            )
            .expect("best-effort output may retain a downgraded report");

        assert_eq!(
            rendered.root_theme_report().verification(),
            RootThemeVerification::Unverified
        );
        assert_eq!(
            rendered.root_theme_report().residuals()[0].reason(),
            crate::diagram_theme::RootThemeResidualReason::OutputMutation
        );
    }

    #[test]
    fn strict_root_theme_is_rechecked_after_svg_postprocessing() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_canvas(CanvasSpec::solid("#111827").expect("valid canvas paint")),
            )
            .expect("compile root canvas theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict themed render session");
        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("prepare themed family")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("initial root consumer proves the canvas");
        let error = match rendered.apply_pipeline(
            &SvgPipeline::parity()
                .with_postprocessor(crate::svg::RootBackgroundPostprocessor::new("white")),
        ) {
            Ok(_) => panic!("strict portability must be rechecked after postprocessing"),
            Err(error) => error,
        };

        assert!(error.rejected_root_theme().is_some(), "{error}");
    }

    #[test]
    fn family_theme_evidence_is_invalidated_by_untrusted_svg_postprocessing() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let render = |portability| {
            let parsed = theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(
                    "flowchart LR\nA[Alpha]\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .expect("Flowchart source should produce a render model");
            let session = crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(portability)
                .begin_session_with_theme(&theme)
                .expect("begin themed render session");
            prepare(parsed, &LayoutOptions::default(), session)
                .expect("prepare themed Flowchart")
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .expect("initial Node consumer proves typed paint")
        };
        let pipeline = || {
            SvgPipeline::parity()
                .with_postprocessor(crate::svg::RootBackgroundPostprocessor::new("white"))
        };

        let rendered = render(ThemePortabilityRequirement::BestEffort)
            .apply_pipeline(&pipeline())
            .expect("best-effort output retains downgraded family evidence");
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Unverified
        );
        assert!(rendered.style_report().output_mutated());
        assert_eq!(
            rendered.style_report().theme_residuals(),
            &[FamilyThemeResidual {
                key: FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Node,
                },
                reason: FamilyThemeResidualReason::OutputMutation,
            }]
        );

        let apply_error = match render(ThemePortabilityRequirement::RequirePortable)
            .apply_pipeline(&pipeline())
        {
            Ok(_) => panic!("strict draft output must reject invalidated family evidence"),
            Err(error) => error,
        };
        assert!(matches!(
            apply_error,
            Error::UnverifiedFamilyOutputMutation {
                family_kind: RenderFamilyKind::Flowchart
            }
        ));

        let finalize_error = match render(ThemePortabilityRequirement::RequirePortable)
            .finalize_resvg(&pipeline().into_resvg_safe())
        {
            Ok(_) => panic!("strict finalized output must reject invalidated family evidence"),
            Err(error) => error,
        };
        assert!(matches!(
            finalize_error,
            Error::UnverifiedFamilyOutputMutation {
                family_kind: RenderFamilyKind::Flowchart
            }
        ));
    }

    #[test]
    fn unsupported_root_gradient_residual_survives_terminal_svg_completion() {
        let gradient = LinearGradient::new(
            90.0,
            [
                GradientStop::new(0.0, ThemeColorValue::parse("#0f172a").unwrap()).unwrap(),
                GradientStop::new(1.0, ThemeColorValue::parse("#22d3ee").unwrap()).unwrap(),
            ],
        )
        .unwrap();
        let canvas = CanvasSpec::default()
            .with_layer(CanvasLayer::new(CanvasPaint::LinearGradient(gradient)))
            .expect("bounded gradient canvas layer");
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_canvas(canvas))
            .expect("compile layered canvas theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort themed session");
        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("best-effort root residual should not stop layout")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render best-effort SVG");

        let report = rendered.root_theme_report();
        assert_eq!(report.evaluation(), RootThemeEvaluation::Evaluated);
        assert_eq!(report.verification(), RootThemeVerification::Unverified);
        assert_eq!(report.residuals().len(), 1);
        assert_eq!(
            report.residuals()[0].key(),
            &RootThemeMechanismKey::CanvasLayer { index: 0 }
        );
        assert!(!rendered.svg().contains("data-merman-theme-canvas-layer"));
    }

    #[test]
    fn root_theme_evidence_preserves_distinct_layers_with_equal_capabilities() {
        let canvas = CanvasSpec::default()
            .with_layer(CanvasLayer::new(
                CanvasPaint::solid("#ef4444").expect("valid first layer paint"),
            ))
            .expect("bounded first canvas layer")
            .with_layer(CanvasLayer::new(
                CanvasPaint::solid("#2563eb").expect("valid second layer paint"),
            ))
            .expect("bounded second canvas layer");
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_canvas(canvas))
            .expect("compile layered canvas theme");
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin best-effort themed session");
        let plan = RootThemePlan::from_theme(session.theme());
        let mut application = plan.begin_svg_application();
        assert!(application.mark_applied(
            &RootThemeMechanismKey::CanvasLayer { index: 0 },
            [ThemeCapability::LayeredCanvas, ThemeCapability::SolidPaint],
        ));
        assert!(application.mark_applied(
            &RootThemeMechanismKey::CanvasLayer { index: 1 },
            [ThemeCapability::LayeredCanvas, ThemeCapability::SolidPaint],
        ));
        let report = application.finish();

        assert_eq!(
            report.required_mechanisms(),
            &[
                RootThemeMechanismKey::CanvasLayer { index: 0 },
                RootThemeMechanismKey::CanvasLayer { index: 1 },
            ]
        );
        assert!(report.residuals().is_empty());
        assert_eq!(
            report.applied_mechanisms(),
            &[
                RootThemeMechanismKey::CanvasLayer { index: 0 },
                RootThemeMechanismKey::CanvasLayer { index: 1 },
            ]
        );
        assert!(report.coverage_complete());
    }

    #[test]
    fn root_layer_evidence_keeps_opacity_and_offset_capabilities_on_the_layer_key() {
        let layer = CanvasLayer::new(CanvasPaint::solid("#ef4444").expect("valid layer paint"))
            .with_opacity(0.5)
            .expect("valid layer opacity")
            .with_offset(4.0, -2.0)
            .expect("valid layer offset");
        let canvas = CanvasSpec::default()
            .with_layer(layer)
            .expect("bounded canvas layer");
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_canvas(canvas))
            .expect("compile layered canvas theme");
        let report = RootThemePlan::from_theme(Some(&theme))
            .begin_svg_application()
            .finish();
        let mechanism = &report.mechanisms()[0];

        assert!(
            mechanism
                .required_capabilities()
                .any(|capability| capability == ThemeCapability::Opacity)
        );
        assert!(
            mechanism
                .required_capabilities()
                .any(|capability| capability == ThemeCapability::CanvasLayerPlacement)
        );
        assert!(
            mechanism
                .residual_capabilities()
                .any(|capability| capability == ThemeCapability::CanvasLayerPlacement)
        );
    }

    #[test]
    fn require_portable_accepts_a_root_layer_proved_by_the_svg_consumer() {
        let canvas = CanvasSpec::transparent()
            .with_layer(CanvasLayer::new(
                CanvasPaint::solid("#ef4444").expect("valid layer paint"),
            ))
            .expect("bounded canvas layer");
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_canvas(canvas))
            .expect("compile layered canvas theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable session");

        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("strict root theme should reach the real SVG consumer")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("proved root layer should satisfy strict portability");

        assert_eq!(
            rendered.root_theme_report().verification(),
            RootThemeVerification::Verified
        );
    }

    #[test]
    fn require_portable_rejects_an_unsupported_root_theme_after_svg_consumption() {
        let gradient = LinearGradient::new(
            90.0,
            [
                GradientStop::new(0.0, ThemeColorValue::parse("#0f172a").unwrap()).unwrap(),
                GradientStop::new(1.0, ThemeColorValue::parse("#22d3ee").unwrap()).unwrap(),
            ],
        )
        .unwrap();
        let canvas = CanvasSpec::transparent()
            .with_layer(CanvasLayer::new(CanvasPaint::LinearGradient(gradient)))
            .expect("bounded gradient layer");
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_canvas(canvas))
            .expect("compile gradient theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable session");
        let artifact = prepare(parsed, &LayoutOptions::default(), session)
            .expect("strict root theme should reach the real SVG consumer");

        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("strict portability must reject an unsupported root gradient"),
                Err(error) => error,
            };
        let (verification, residual_count, first_residual) = error
            .rejected_root_theme()
            .expect("evaluated root theme rejection");
        assert_eq!(verification, RootThemeVerification::Unverified);
        assert_eq!(residual_count, 1);
        assert_eq!(
            first_residual.expect("gradient residual").key(),
            &RootThemeMechanismKey::CanvasLayer { index: 0 }
        );
    }

    #[test]
    fn require_portable_accepts_flowchart_typed_node_paint_after_svg_emission() {
        let fill = CanvasPaint::solid("#ef4444").expect("valid node fill");
        let stroke = CanvasPaint::solid("#2563eb").expect("valid node stroke");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_fill(fill)
                                .with_stroke(stroke),
                        )
                        .for_family(RenderFamilyKind::Flowchart),
                    ),
                ),
            )
            .expect("compile strict Flowchart theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable render session");

        let artifact = prepare(parsed, &LayoutOptions::default(), session)
            .expect("typed Flowchart paint must reach SVG emission");
        let (preparation_evidence, source_residuals) = artifact
            .family
            .flowchart_theme_evidence(artifact.context.resolved_theme())
            .expect("Flowchart artifact evidence");
        assert!(source_residuals.is_empty());
        assert!(preparation_evidence.applied().is_empty());

        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("emitted typed Flowchart paint must satisfy strict portability");

        assert!(rendered.svg().contains("fill:#ef4444 !important"));
        assert!(rendered.svg().contains("stroke:#2563eb !important"));
        let report = rendered.style_report();
        assert_eq!(report.evaluation(), FamilyStyleEvaluation::Evaluated);
        assert_eq!(report.verification(), FamilyStyleVerification::Verified);
        assert_eq!(report.compatibility_residual_count(), 0);
        assert_eq!(
            report.theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
    }

    #[test]
    fn require_portable_accepts_swimlane_typed_node_paint_after_svg_emission() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::Transparent)
                                .with_stroke(CanvasPaint::Transparent),
                        )
                        .for_family(RenderFamilyKind::Swimlane),
                    ),
                ),
            )
            .expect("compile strict Swimlane theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA --> B\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Swimlane source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable render session");

        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("typed Swimlane paint must reach SVG emission")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("emitted typed Swimlane paint must satisfy strict portability");

        assert_eq!(rendered.family_kind(), RenderFamilyKind::Swimlane);
        assert!(
            rendered
                .svg()
                .contains("fill:none !important;stroke:none !important")
        );
        let report = rendered.style_report();
        assert_eq!(report.evaluation(), FamilyStyleEvaluation::Evaluated);
        assert_eq!(report.verification(), FamilyStyleVerification::Verified);
        assert_eq!(report.compatibility_residual_count(), 0);
        assert_eq!(
            report.theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
    }

    #[test]
    fn flowchart_source_paint_override_does_not_claim_typed_application() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap())
                                .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                        )
                        .for_family(RenderFamilyKind::Flowchart),
                    ),
                ),
            )
            .expect("compile Flowchart source precedence theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nclassDef source fill:#22c55e,stroke:#111827\nA[Alpha]:::source\nB[Beta]\nstyle B fill:#f59e0b,stroke:#334155\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable render session");

        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("source-overridden typed paint remains evaluable")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("source-overridden typed paint is not a portability residual");

        assert!(rendered.svg().contains("fill:#22c55e !important"));
        assert!(rendered.svg().contains("stroke:#111827 !important"));
        assert!(rendered.svg().contains("fill:#f59e0b !important"));
        assert!(rendered.svg().contains("stroke:#334155 !important"));
        assert!(!rendered.svg().contains("fill:#ef4444 !important"));
        assert!(!rendered.svg().contains("stroke:#2563eb !important"));
        let report = rendered.style_report();
        assert_eq!(report.verification(), FamilyStyleVerification::Verified);
        assert!(report.theme_applied_mechanisms().is_empty());
        assert_eq!(
            report.theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
    }

    #[test]
    fn flowchart_single_source_paint_override_keeps_other_typed_channel_applied() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap())
                                .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                        )
                        .for_family(RenderFamilyKind::Flowchart),
                    ),
                ),
            )
            .expect("compile Flowchart source precedence theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nstyle A fill:#22c55e\nA[Alpha]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable session");

        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("single source override remains evaluable")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("the surviving typed stroke should satisfy strict portability");

        assert!(rendered.svg().contains("fill:#22c55e !important"));
        assert!(rendered.svg().contains("stroke:#2563eb !important"));
        assert!(!rendered.svg().contains("fill:#ef4444 !important"));
        assert!(rendered.style_report().theme_applied_mechanisms().contains(
            &FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }
        ));
    }

    #[test]
    fn flowchart_source_paint_identity_and_value_admission_control_precedence() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let render = |source: &str| {
            let parsed = theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model");
            let session = crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict portable render session");
            prepare(parsed, &LayoutOptions::default(), session)
                .expect("source paint precedence is evaluated during SVG emission")
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .expect("admitted source precedence must remain portable")
        };

        let overridden = render("flowchart LR\nstyle A FILL:#22c55e\nA[Alpha]\n");
        assert!(overridden.svg().contains("FILL:#22c55e !important"));
        assert!(!overridden.svg().contains("fill:#ef4444 !important"));
        assert!(
            overridden
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert_eq!(
            overridden.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );

        let dynamic_source = "flowchart LR\nstyle A --paint:#22c55e,fill:var(--paint)\nA[Alpha]\n";
        let parse_dynamic = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(dynamic_source, ParseOptions::strict())
                .unwrap()
                .expect("dynamic source paint should produce a render model")
        };
        let dynamic = prepare(
            parse_dynamic(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort session"),
        )
        .expect("prepare dynamic source paint")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output must preserve dynamic source paint");
        assert!(dynamic.svg().contains("fill:var(--paint) !important"));
        assert!(!dynamic.svg().contains("fill:#ef4444 !important"));
        assert!(dynamic.style_report().theme_residuals().is_empty());
        assert_eq!(dynamic.style_report().residuals().len(), 1);
        assert_eq!(dynamic.style_report().residuals()[0].owner_id(), "A");
        assert_eq!(
            dynamic.style_report().residuals()[0].property(),
            Some("fill")
        );
        assert_eq!(
            dynamic.style_report().residuals()[0].reason(),
            FamilyStyleResidualReason::InvalidValue
        );

        let strict_session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict session");
        let artifact = prepare(parse_dynamic(), &LayoutOptions::default(), strict_session)
            .expect("dynamic source verification waits for SVG emission");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("unverified dynamic source paint must fail strict portability"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_style(),
            Some((
                RenderFamilyKind::Flowchart,
                dynamic.style_report().residuals().len(),
            ))
        );
    }

    #[test]
    fn flowchart_explicit_mermaid_config_owns_typed_node_paint_precedence() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );

        for explicit_fill in ["#22c55e", "#ef4444"] {
            let engine = theme.install_parse_compatibility(Engine::new().with_site_config(
                MermaidConfig::from_value(json!({
                    "themeVariables": {"mainBkg": explicit_fill}
                })),
            ));
            let parsed = engine
                .parse_diagram_for_render_model_sync(
                    "flowchart LR\nA[Alpha]\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .expect("Flowchart source should produce a render model");
            let session = crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict render session");

            let rendered = prepare(parsed, &LayoutOptions::default(), session)
                .expect("prepare explicit Mermaid config")
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .expect("explicit Mermaid config should supersede typed Node paint");

            assert!(rendered.svg().contains(explicit_fill), "{}", rendered.svg());
            assert!(
                rendered
                    .style_report()
                    .theme_applied_mechanisms()
                    .is_empty()
            );
            assert_eq!(
                rendered.style_report().theme_not_applicable_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Node,
                }]
            );
            assert!(rendered.style_report().theme_residuals().is_empty());
        }
    }

    #[test]
    fn require_portable_accepts_flowchart_and_swimlane_node_stroke_width_after_emission() {
        for (family, source) in [
            (RenderFamilyKind::Flowchart, "flowchart LR\nA[Alpha]\n"),
            (
                RenderFamilyKind::Swimlane,
                "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA[Alpha]\n",
            ),
        ] {
            let theme = flowchart_node_stroke_width_theme(family, 2.5);
            let parsed = theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model");
            let rendered = prepare(
                parsed,
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .begin_session_with_theme(&theme)
                    .expect("begin strict stroke-width session"),
            )
            .expect("prepare typed Node stroke width")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("classic Process writer should prove typed Node stroke width");

            assert_eq!(rendered.family_kind(), family);
            assert!(
                flowchart_node_shape_style(rendered.svg(), "A")
                    .contains("stroke-width:2.5px !important")
            );
            assert_eq!(
                rendered.style_report().verification(),
                FamilyStyleVerification::Verified
            );
            assert_eq!(
                rendered.style_report().theme_applied_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Node,
                }]
            );
            assert!(rendered.style_report().theme_residuals().is_empty());
        }
    }

    #[test]
    fn flowchart_source_and_explicit_config_own_node_stroke_width_precedence() {
        let theme = flowchart_node_stroke_width_theme(RenderFamilyKind::Flowchart, 2.5);
        let render = |engine: Engine, source: &str| {
            let parsed = theme
                .install_parse_compatibility(engine)
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model");
            prepare(
                parsed,
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .begin_session_with_theme(&theme)
                    .expect("begin strict precedence session"),
            )
            .expect("prepare Flowchart stroke-width precedence")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("explicit Mermaid owner should supersede typed Node stroke width")
        };

        let source = render(
            Engine::new(),
            "flowchart LR\nstyle A stroke-width:4px\nA[Alpha]\n",
        );
        assert!(
            flowchart_node_shape_style(source.svg(), "A").contains("stroke-width:4px !important")
        );
        assert!(!source.svg().contains("stroke-width:2.5px !important"));

        let configured = render(
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "themeVariables": {"strokeWidth": 4}
            }))),
            "flowchart LR\nA[Alpha]\n",
        );
        assert!(configured.svg().contains("stroke-width:4px;"));
        assert!(!configured.svg().contains("stroke-width:2.5px !important"));

        for rendered in [&source, &configured] {
            assert!(
                rendered
                    .style_report()
                    .theme_applied_mechanisms()
                    .is_empty()
            );
            assert_eq!(
                rendered.style_report().theme_not_applicable_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Node,
                }]
            );
            assert!(rendered.style_report().theme_residuals().is_empty());
        }
    }

    #[test]
    fn unverified_flowchart_node_surfaces_keep_typed_stroke_width_fail_closed() {
        let theme = flowchart_node_stroke_width_theme(RenderFamilyKind::Flowchart, 2.5);
        let cases = [
            r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
flowchart LR
A[Alpha]
"#,
            "flowchart LR\nA@{ shape: choice }\n",
        ];

        for source in cases {
            let parse = || {
                theme
                    .install_parse_compatibility(Engine::new())
                    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                    .unwrap()
                    .expect("Flowchart source should produce a render model")
            };
            let rendered = prepare(
                parse(),
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .begin_session_with_theme(&theme)
                    .expect("begin best-effort stroke-width session"),
            )
            .expect("prepare unverified stroke-width surface")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("best-effort output should retain geometry evidence");
            assert_eq!(rendered.style_report().theme_residuals().len(), 1);
            assert_eq!(
                rendered.style_report().theme_residuals()[0].reason(),
                FamilyThemeResidualReason::UnsupportedGeometry
            );

            let artifact = prepare(
                parse(),
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .begin_session_with_theme(&theme)
                    .expect("begin strict stroke-width session"),
            )
            .expect("writer support is decided during SVG emission");
            let error = match artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            {
                Ok(_) => panic!("unverified Node stroke width must remain fail-closed"),
                Err(error) => error,
            };
            assert_eq!(
                error.unverified_family_theme(),
                Some((RenderFamilyKind::Flowchart, 1))
            );
        }
    }

    #[test]
    fn require_portable_accepts_flowchart_and_swimlane_node_stroke_dasharray_after_emission() {
        for (family, source) in [
            (RenderFamilyKind::Flowchart, "flowchart LR\nA[Alpha]\n"),
            (
                RenderFamilyKind::Swimlane,
                "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA[Alpha]\n",
            ),
        ] {
            let theme = flowchart_node_stroke_dasharray_theme(family, [4.0, 2.0]);
            let parsed = theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model");
            let rendered = prepare(
                parsed,
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .begin_session_with_theme(&theme)
                    .expect("begin strict stroke-dasharray session"),
            )
            .expect("prepare typed Node stroke dasharray")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("classic Process writer should prove typed Node stroke dasharray");

            assert_eq!(rendered.family_kind(), family);
            assert!(
                flowchart_node_shape_style(rendered.svg(), "A")
                    .contains("stroke-dasharray:4 2 !important")
            );
            assert_eq!(
                rendered.style_report().verification(),
                FamilyStyleVerification::Verified
            );
            assert_eq!(
                rendered.style_report().theme_applied_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Node,
                }]
            );
            assert_eq!(
                rendered
                    .style_report()
                    .applied_capabilities()
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from([ThemeCapability::DashStyling])
            );
            assert!(rendered.style_report().theme_residuals().is_empty());
        }
    }

    #[test]
    fn flowchart_source_owns_node_stroke_dasharray_precedence() {
        let theme = flowchart_node_stroke_dasharray_theme(RenderFamilyKind::Flowchart, [4.0, 2.0]);
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nstyle A stroke-dasharray:8 3\nA[Alpha]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict stroke-dasharray precedence session"),
        )
        .expect("prepare Flowchart stroke-dasharray precedence")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("source style should supersede typed Node stroke dasharray");

        let shape_style = flowchart_node_shape_style(rendered.svg(), "A");
        assert!(shape_style.contains("stroke-dasharray:8 3 !important"));
        assert!(!shape_style.contains("stroke-dasharray:4 2 !important"));
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }

    #[test]
    fn unverified_flowchart_node_surfaces_keep_typed_stroke_dasharray_fail_closed() {
        let theme = flowchart_node_stroke_dasharray_theme(RenderFamilyKind::Flowchart, [4.0, 2.0]);
        let cases = [
            r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
flowchart LR
A[Alpha]
"#,
            "flowchart LR\nA@{ shape: choice }\n",
        ];

        for source in cases {
            let parse = || {
                theme
                    .install_parse_compatibility(Engine::new())
                    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                    .unwrap()
                    .expect("Flowchart source should produce a render model")
            };
            let rendered = prepare(
                parse(),
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .begin_session_with_theme(&theme)
                    .expect("begin best-effort stroke-dasharray session"),
            )
            .expect("prepare unverified stroke-dasharray surface")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("best-effort output should retain dash evidence");
            assert_eq!(rendered.style_report().theme_residuals().len(), 1);
            assert_eq!(
                rendered.style_report().theme_residuals()[0].reason(),
                FamilyThemeResidualReason::UnsupportedGeometry
            );

            let artifact = prepare(
                parse(),
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .begin_session_with_theme(&theme)
                    .expect("begin strict stroke-dasharray session"),
            )
            .expect("writer support is decided during SVG emission");
            let error = match artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            {
                Ok(_) => panic!("unverified Node stroke dasharray must remain fail-closed"),
                Err(error) => error,
            };
            assert_eq!(
                error.unverified_family_theme(),
                Some((RenderFamilyKind::Flowchart, 1))
            );
        }
    }

    #[test]
    fn require_portable_accepts_flowchart_and_swimlane_node_radius_after_emission() {
        for (family, source) in [
            (RenderFamilyKind::Flowchart, "flowchart LR\nA[Alpha]\n"),
            (
                RenderFamilyKind::Swimlane,
                "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA[Alpha]\n",
            ),
        ] {
            let theme = flowchart_node_radius_theme(family, 8.0);
            let parsed = theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model");
            let rendered = prepare(
                parsed,
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .begin_session_with_theme(&theme)
                    .expect("begin strict radius session"),
            )
            .expect("prepare typed Node radius")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("classic Process writer should prove typed Node radius");

            assert_eq!(rendered.family_kind(), family);
            assert_eq!(
                flowchart_node_shape_attribute(rendered.svg(), "A", "rx").as_deref(),
                Some("8")
            );
            assert_eq!(
                flowchart_node_shape_attribute(rendered.svg(), "A", "ry").as_deref(),
                Some("8")
            );
            assert_eq!(
                rendered.style_report().verification(),
                FamilyStyleVerification::Verified
            );
            assert_eq!(
                rendered.style_report().theme_applied_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Node,
                }]
            );
            assert_eq!(
                rendered
                    .style_report()
                    .applied_capabilities()
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from([ThemeCapability::RoundedGeometry])
            );
            assert!(rendered.style_report().theme_residuals().is_empty());
        }
    }

    #[test]
    fn flowchart_source_and_explicit_config_own_node_radius_precedence() {
        let theme = flowchart_node_radius_theme(RenderFamilyKind::Flowchart, 8.0);
        let render = |engine: Engine, source: &str| {
            let parsed = theme
                .install_parse_compatibility(engine)
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model");
            prepare(
                parsed,
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .begin_session_with_theme(&theme)
                    .expect("begin strict radius precedence session"),
            )
            .expect("prepare Flowchart radius precedence")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("explicit Mermaid owner should supersede typed Node radius")
        };

        let source = render(
            Engine::new(),
            "flowchart LR\nstyle A rx:4px,ry:6px\nA[Alpha]\n",
        );
        let source_style = flowchart_node_shape_style(source.svg(), "A");
        assert!(source_style.contains("rx:4px !important"));
        assert!(source_style.contains("ry:6px !important"));
        assert_eq!(
            flowchart_node_shape_attribute(source.svg(), "A", "rx"),
            None
        );
        assert_eq!(
            flowchart_node_shape_attribute(source.svg(), "A", "ry"),
            None
        );

        let configured = render(
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "look": "neo",
                "themeVariables": {"radius": 4}
            }))),
            "flowchart LR\nA[Alpha]\n",
        );
        assert_eq!(
            flowchart_node_shape_attribute(configured.svg(), "A", "rx").as_deref(),
            Some("4")
        );
        assert_eq!(
            flowchart_node_shape_attribute(configured.svg(), "A", "ry").as_deref(),
            Some("4")
        );

        for rendered in [&source, &configured] {
            assert!(
                rendered
                    .style_report()
                    .theme_applied_mechanisms()
                    .is_empty()
            );
            assert_eq!(
                rendered.style_report().theme_not_applicable_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Node,
                }]
            );
            assert!(rendered.style_report().theme_residuals().is_empty());
        }
    }

    #[test]
    fn unverified_flowchart_node_surfaces_keep_typed_radius_fail_closed() {
        let theme = flowchart_node_radius_theme(RenderFamilyKind::Flowchart, 8.0);
        let cases = [
            r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
flowchart LR
A[Alpha]
"#,
            "flowchart LR\nA@{ shape: choice }\n",
        ];

        for source in cases {
            let parse = || {
                theme
                    .install_parse_compatibility(Engine::new())
                    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                    .unwrap()
                    .expect("Flowchart source should produce a render model")
            };
            let rendered = prepare(
                parse(),
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .begin_session_with_theme(&theme)
                    .expect("begin best-effort radius session"),
            )
            .expect("prepare unverified radius surface")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("best-effort output should retain geometry evidence");
            assert_eq!(rendered.style_report().theme_residuals().len(), 1);
            assert_eq!(
                rendered.style_report().theme_residuals()[0].reason(),
                FamilyThemeResidualReason::UnsupportedGeometry
            );

            let artifact = prepare(
                parse(),
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .begin_session_with_theme(&theme)
                    .expect("begin strict radius session"),
            )
            .expect("writer support is decided during SVG emission");
            let error = match artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            {
                Ok(_) => panic!("unverified Node radius must remain fail-closed"),
                Err(error) => error,
            };
            assert_eq!(
                error.unverified_family_theme(),
                Some((RenderFamilyKind::Flowchart, 1))
            );
        }
    }

    #[test]
    fn classic_flowchart_without_diagram_theme_does_not_emit_radius_attributes() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync("flowchart LR\nA[Alpha]\n", ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let rendered = prepare(parsed, &LayoutOptions::default(), session())
            .expect("prepare default Flowchart")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render default Flowchart");

        assert_eq!(
            flowchart_node_shape_attribute(rendered.svg(), "A", "rx"),
            None
        );
        assert_eq!(
            flowchart_node_shape_attribute(rendered.svg(), "A", "ry"),
            None
        );
    }

    #[test]
    fn require_portable_accepts_flowchart_and_swimlane_node_label_typography_after_emission() {
        for (family, source) in [
            (
                RenderFamilyKind::Flowchart,
                r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A[Alpha]
"#,
            ),
            (
                RenderFamilyKind::Swimlane,
                r#"---
config:
  layout: swimlane
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart TD
A[Alpha]
"#,
            ),
        ] {
            let theme = flowchart_node_label_font_stack_and_size_theme_with_base(family, None);
            let parsed = theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model");
            let rendered = prepare(
                parsed,
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .begin_session_with_theme(&theme)
                    .expect("begin strict NodeLabel typography session"),
            )
            .expect("prepare typed NodeLabel typography")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("prepared SVG writer should prove typed NodeLabel typography");

            assert_eq!(rendered.family_kind(), family);
            let label_style = flowchart_node_label_style(rendered.svg(), "A");
            assert!(
                label_style.contains("font-family:\"Excalifont\" !important"),
                "{label_style}"
            );
            assert!(
                label_style.contains("font-size:26px !important"),
                "{label_style}"
            );
            assert_eq!(
                rendered.style_report().verification(),
                FamilyStyleVerification::Verified
            );
            assert_eq!(
                rendered.style_report().theme_applied_mechanisms(),
                &[
                    FamilyThemeMechanismKey::Rule {
                        index: 0,
                        target: ThemeTarget::NodeLabel,
                    },
                    FamilyThemeMechanismKey::Rule {
                        index: 1,
                        target: ThemeTarget::NodeLabel,
                    },
                ]
            );
            assert_eq!(
                rendered
                    .style_report()
                    .applied_capabilities()
                    .collect::<BTreeSet<_>>(),
                BTreeSet::from([ThemeCapability::Typography])
            );
            assert!(rendered.style_report().theme_residuals().is_empty());
        }
    }

    #[test]
    fn typed_node_label_font_stack_outranks_legacy_base_typography() {
        for (family, source) in [
            (
                RenderFamilyKind::Flowchart,
                r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A[Alpha]
"#,
            ),
            (
                RenderFamilyKind::Swimlane,
                r#"---
config:
  layout: swimlane
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart TD
A[Alpha]
"#,
            ),
        ] {
            let theme = flowchart_node_label_font_stack_theme_with_base(
                family,
                Some(FontStack::single("Xiaolai SC").expect("valid base font stack")),
            );
            let parsed = theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model");
            let rendered = prepare(
                parsed,
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .begin_session_with_theme(&theme)
                    .expect("begin mixed typography session"),
            )
            .expect("typed NodeLabel rule should outrank legacy base typography")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render mixed typography Flowchart");

            let label_style = flowchart_node_label_style(rendered.svg(), "A");
            assert!(
                label_style.contains("font-family:\"Excalifont\" !important"),
                "{label_style}"
            );
            assert!(!label_style.contains("Xiaolai"), "{label_style}");
            assert!(rendered.style_report().theme_applied_mechanisms().contains(
                &FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::NodeLabel,
                }
            ));
            assert!(
                !rendered
                    .style_report()
                    .theme_not_applicable_mechanisms()
                    .contains(&FamilyThemeMechanismKey::Rule {
                        index: 0,
                        target: ThemeTarget::NodeLabel,
                    })
            );
        }
    }

    #[test]
    fn typed_node_label_font_size_outranks_legacy_base_typography() {
        for (family, source) in [
            (
                RenderFamilyKind::Flowchart,
                r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A[Alpha]
"#,
            ),
            (
                RenderFamilyKind::Swimlane,
                r#"---
config:
  layout: swimlane
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart TD
A[Alpha]
"#,
            ),
        ] {
            let theme =
                flowchart_node_label_font_stack_and_size_theme_with_base(family, Some(19.0));
            let parsed = theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model");
            let rendered = prepare(
                parsed,
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .begin_session_with_theme(&theme)
                    .expect("begin mixed font-size session"),
            )
            .expect("typed NodeLabel font size should outrank legacy base typography")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render mixed font-size Flowchart");

            let label_style = flowchart_node_label_style(rendered.svg(), "A");
            assert!(
                label_style.contains("font-size:26px !important"),
                "{label_style}"
            );
            assert!(!label_style.contains("font-size:19px"), "{label_style}");
            assert!(rendered.style_report().theme_applied_mechanisms().contains(
                &FamilyThemeMechanismKey::Rule {
                    index: 1,
                    target: ThemeTarget::NodeLabel,
                }
            ));
        }
    }

    #[test]
    fn flowchart_source_and_explicit_config_own_node_label_font_stack_precedence() {
        let theme = flowchart_node_label_font_stack_theme(RenderFamilyKind::Flowchart);
        let render = |engine: Engine, source: &str| {
            let parsed = theme
                .install_parse_compatibility(engine)
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model");
            prepare(
                parsed,
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .begin_session_with_theme(&theme)
                    .expect("begin strict NodeLabel font-stack precedence session"),
            )
            .expect("prepare Flowchart NodeLabel font-stack precedence")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("source or Mermaid config should supersede typed NodeLabel font stack")
        };

        let source = render(
            Engine::new(),
            r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
classDef local font-family:Xiaolai SC
A[测试]:::local
"#,
        );
        assert!(
            flowchart_node_label_style(source.svg(), "A")
                .contains("font-family:\"Xiaolai SC\" !important")
        );

        let configured = render(
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "htmlLabels": false,
                "flowchart": {"htmlLabels": false},
                "themeVariables": {"fontFamily": "Xiaolai SC"}
            }))),
            "flowchart LR\nA[测试]\n",
        );
        assert!(
            flowchart_node_label_style(configured.svg(), "A")
                .contains("font-family:\"Xiaolai SC\" !important")
        );

        for rendered in [&source, &configured] {
            assert!(
                rendered
                    .style_report()
                    .theme_applied_mechanisms()
                    .is_empty()
            );
            assert_eq!(
                rendered.style_report().theme_not_applicable_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::NodeLabel,
                }]
            );
            assert!(rendered.style_report().theme_residuals().is_empty());
        }
    }

    #[test]
    fn flowchart_source_and_explicit_config_own_only_node_label_font_size_precedence() {
        let theme = flowchart_node_label_font_stack_and_size_theme_with_base(
            RenderFamilyKind::Flowchart,
            None,
        );
        let render = |engine: Engine, source: &str| {
            let parsed = theme
                .install_parse_compatibility(engine)
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model");
            prepare(
                parsed,
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .begin_session_with_theme(&theme)
                    .expect("begin strict NodeLabel font-size precedence session"),
            )
            .expect("prepare Flowchart NodeLabel font-size precedence")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("source or Mermaid config should supersede typed NodeLabel font size")
        };

        let source = render(
            Engine::new(),
            r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
classDef local font-size:22px
A[Alpha]:::local
"#,
        );
        assert!(
            flowchart_node_label_style(source.svg(), "A").contains("font-size:22px !important")
        );

        let configured = render(
            Engine::new().with_site_config(MermaidConfig::from_value(json!({
                "htmlLabels": false,
                "flowchart": {"htmlLabels": false},
                "themeVariables": {"fontSize": "22px"}
            }))),
            "flowchart LR\nA[Alpha]\n",
        );
        assert!(
            flowchart_node_label_style(configured.svg(), "A").contains("font-size:22px !important")
        );

        for rendered in [&source, &configured] {
            assert_eq!(
                rendered.style_report().theme_applied_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::NodeLabel,
                }]
            );
            assert_eq!(
                rendered.style_report().theme_not_applicable_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 1,
                    target: ThemeTarget::NodeLabel,
                }]
            );
            assert!(rendered.style_report().theme_residuals().is_empty());
        }
    }

    #[test]
    fn flowchart_node_label_typography_rejects_unprepared_label_modes() {
        let theme = flowchart_node_label_font_stack_and_size_theme_with_base(
            RenderFamilyKind::Flowchart,
            None,
        );
        let cases = [
            "flowchart LR\nA[Alpha]\n",
            r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A["`**Alpha**`"]
"#,
            r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A@{ shape: icon, label: "Plain" }
style A font-family:Excalifont
"#,
        ];

        for source in cases {
            let parsed = theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model");
            let result = prepare(
                parsed,
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .begin_session_with_theme(&theme)
                    .expect("begin strict unprepared NodeLabel session"),
            );

            assert!(matches!(
                result,
                Err(Error::TextLayout(
                    crate::text::TextLayoutFailure::UnsupportedLabelMode
                ))
            ));
        }
    }

    #[test]
    fn flowchart_node_label_font_size_without_prepared_text_is_residual() {
        let theme =
            flowchart_node_label_font_size_theme_without_catalog(RenderFamilyKind::Flowchart);
        let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A[Alpha]
"#;
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };

        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort NodeLabel font-size session"),
        )
        .expect("prepare best-effort NodeLabel font-size Flowchart")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render best-effort NodeLabel font-size SVG");

        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Unverified
        );
        assert_eq!(rendered.style_report().theme_residuals().len(), 1);
        assert_eq!(
            rendered.style_report().theme_residuals()[0].key(),
            &FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::NodeLabel,
            }
        );
        assert_eq!(
            rendered.style_report().theme_residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedTypography
        );

        let strict_result = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict NodeLabel font-size session"),
        )
        .expect("prepare strict NodeLabel font-size Flowchart")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
        let strict_error = match strict_result {
            Ok(_) => panic!("strict output must reject unproven NodeLabel font size"),
            Err(error) => error,
        };
        assert!(matches!(
            strict_error,
            Error::UnverifiedFamilyTheme {
                family_kind: RenderFamilyKind::Flowchart,
                residual_count: 1,
            }
        ));
    }

    #[test]
    fn flowchart_node_without_label_makes_typed_typography_not_applicable() {
        let theme = flowchart_node_label_font_stack_and_size_theme_with_base(
            RenderFamilyKind::Flowchart,
            None,
        );
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A@{ shape: start }
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict no-label NodeLabel session"),
        )
        .expect("prepare no-label Flowchart node")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("no-label node should not require NodeLabel typography");

        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[
                FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::NodeLabel,
                },
                FamilyThemeMechanismKey::Rule {
                    index: 1,
                    target: ThemeTarget::NodeLabel,
                },
            ]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }

    #[test]
    fn flowchart_escaped_font_size_is_not_a_node_label_override() {
        let theme = flowchart_node_label_font_stack_and_size_theme_with_base(
            RenderFamilyKind::Flowchart,
            None,
        );
        let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
classDef local f\6f nt-size:22px
A[Alpha]:::local
"#;
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };

        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort escaped font-size session"),
        )
        .expect("prepare escaped font-size Flowchart")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should preserve escaped source CSS");

        let label_style = flowchart_node_label_style(rendered.svg(), "A");
        assert!(
            label_style.contains("font-family:\"Excalifont\" !important"),
            "{label_style}"
        );
        assert!(label_style.contains("font-size:26px !important"));
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "A"
                && residual.property() == Some("font-size")
                && residual.channel() == FamilyStyleChannel::Shape
                && residual.reason() == FamilyStyleResidualReason::UnsupportedProperty
        }));
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[
                FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::NodeLabel,
                },
                FamilyThemeMechanismKey::Rule {
                    index: 1,
                    target: ThemeTarget::NodeLabel,
                },
            ],
            "svg={} report={:?}",
            rendered.svg(),
            rendered.style_report(),
        );

        let strict_result = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict escaped font-size session"),
        )
        .expect("escaped source evidence is completed during SVG emission")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
        let strict_error = match strict_result {
            Ok(_) => panic!("escaped shape CSS must remain fail-closed"),
            Err(error) => error,
        };
        assert_eq!(
            strict_error.unverified_family_style(),
            Some((
                RenderFamilyKind::Flowchart,
                rendered.style_report().residuals().len(),
            ))
        );
    }

    #[test]
    fn flowchart_uppercase_font_size_is_not_a_node_label_override() {
        let theme = flowchart_node_label_font_stack_and_size_theme_with_base(
            RenderFamilyKind::Flowchart,
            None,
        );
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
classDef local FONT-SIZE:22px
A[Alpha]:::local
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort uppercase font-size session"),
        )
        .expect("prepare uppercase font-size Flowchart")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should preserve uppercase source CSS");

        let label_style = flowchart_node_label_style(rendered.svg(), "A");
        assert!(label_style.contains("font-size:26px !important"));
        assert!(!label_style.contains("font-size:22px"), "{label_style}");
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "A"
                && residual.property() == Some("font-size")
                && residual.channel() == FamilyStyleChannel::Shape
                && residual.reason() == FamilyStyleResidualReason::UnsupportedProperty
        }));
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[
                FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::NodeLabel,
                },
                FamilyThemeMechanismKey::Rule {
                    index: 1,
                    target: ThemeTarget::NodeLabel,
                },
            ]
        );
    }

    #[test]
    fn flowchart_entity_authored_unicode_space_consumes_typed_node_label_typography() {
        let theme = flowchart_node_label_font_stack_and_size_theme_with_base(
            RenderFamilyKind::Flowchart,
            None,
        );
        let source = "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart LR\nA[\"&nbsp;\"]\n";
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict visible Unicode-space session"),
        )
        .expect("prepare visible Unicode-space Flowchart")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("prepared writer should prove visible Unicode-space typography");

        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[
                FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::NodeLabel,
                },
                FamilyThemeMechanismKey::Rule {
                    index: 1,
                    target: ThemeTarget::NodeLabel,
                },
            ],
            "svg={} report={:?}",
            rendered.svg(),
            rendered.style_report(),
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
        assert!(
            flowchart_node_label_style(rendered.svg(), "A").contains("font-size:26px !important")
        );
    }

    #[test]
    fn flowchart_explicit_primary_color_owns_derived_main_background() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let engine = theme.install_parse_compatibility(Engine::new().with_site_config(
            MermaidConfig::from_value(json!({
                "theme": "base",
                "themeVariables": {"primaryColor": "#123456"}
            })),
        ));
        let parsed = engine
            .parse_diagram_for_render_model_sync("flowchart LR\nA[Alpha]\n", ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict render session");

        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("prepare derived Mermaid config")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("derived Mermaid main background should supersede typed Node paint");

        assert!(rendered.svg().contains("#123456"), "{}", rendered.svg());
        assert!(!rendered.svg().contains("fill:#ef4444 !important"));
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }

    #[test]
    fn flowchart_theme_recipe_mermaid_config_owns_typed_node_paint_precedence() {
        let compatibility = MermaidThemeCompatibility::default()
            .with_variable("mainBkg", "#22c55e")
            .expect("valid Mermaid compatibility color");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_styles(
                        ThemeRuleSet::default().with_rule(
                            ThemeRule::new(
                                ThemeTarget::Node,
                                ThemeStylePatch::default()
                                    .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                            )
                            .for_family(RenderFamilyKind::Flowchart),
                        ),
                    )
                    .with_mermaid_compatibility(compatibility),
            )
            .expect("compile Flowchart theme recipe");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync("flowchart LR\nA[Alpha]\n", ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin render session");

        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("prepare recipe compatibility precedence")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("best-effort compatibility output");

        assert!(rendered.svg().contains("#22c55e"), "{}", rendered.svg());
        assert!(!rendered.svg().contains("fill:#ef4444 !important"));
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
    }

    #[test]
    fn flowchart_source_paint_residual_is_independent_from_theme_channel() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
        );
        let source = "flowchart LR\nstyle A --paint:#22c55e,fill:var(--paint)\nA[Alpha]\n";
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };

        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort session"),
        )
        .expect("prepare dynamic source paint")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output must preserve dynamic source paint");

        assert!(rendered.svg().contains("fill:var(--paint) !important"));
        assert!(rendered.svg().contains("stroke:#2563eb !important"));
        assert!(rendered.style_report().theme_applied_mechanisms().contains(
            &FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }
        ));
        assert!(rendered.style_report().theme_residuals().is_empty());
        assert_eq!(rendered.style_report().residuals().len(), 1);
        assert_eq!(rendered.style_report().residuals()[0].owner_id(), "A");
        assert_eq!(
            rendered.style_report().residuals()[0].property(),
            Some("fill")
        );

        let artifact = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict session"),
        )
        .expect("source residual is discovered during SVG emission");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("independent source residual must fail strict portability"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_style(),
            Some((
                RenderFamilyKind::Flowchart,
                rendered.style_report().residuals().len(),
            ))
        );
    }

    #[test]
    fn flowchart_unverified_node_surfaces_remain_fail_closed_after_svg_emission() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("rgb(239 68 68)").unwrap()),
        );
        let cases = [
            r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
flowchart LR
A[Alpha]
"#,
            "flowchart LR\nA@{ shape: choice }\n",
        ];

        for source in cases {
            let parse = || {
                theme
                    .install_parse_compatibility(Engine::new())
                    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                    .unwrap()
                    .expect("Flowchart source should produce a render model")
            };
            let rendered = prepare(
                parse(),
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .begin_session_with_theme(&theme)
                    .expect("begin best-effort render session"),
            )
            .expect("prepare unverified node surface")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("best-effort node surface should retain evidence");
            assert_eq!(
                rendered.style_report().theme_residuals(),
                &[FamilyThemeResidual {
                    key: FamilyThemeMechanismKey::Rule {
                        index: 0,
                        target: ThemeTarget::Node,
                    },
                    reason: FamilyThemeResidualReason::UnsupportedPaint,
                }]
            );

            let session = crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict portable render session");
            let artifact = prepare(parse(), &LayoutOptions::default(), session)
                .expect("concrete paint support must be decided during SVG emission");

            let error = match artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            {
                Ok(_) => panic!("unverified node surface must remain fail-closed"),
                Err(error) => error,
            };
            assert_eq!(
                error.unverified_family_theme(),
                Some((
                    RenderFamilyKind::Flowchart,
                    rendered.style_report().theme_residuals().len(),
                ))
            );
        }
    }

    #[test]
    fn flowchart_start_node_proves_direct_typed_paint_emission() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nA@{ shape: start }\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Flowchart start source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict render session"),
        )
        .expect("prepare Flowchart start")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("direct start paint should satisfy strict portability");

        assert!(rendered
            .svg()
            .contains("class=\"state-start\" r=\"7\" width=\"14\" height=\"14\" style=\"fill:#ef4444 !important\""));
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Verified
        );
    }

    #[test]
    fn flowchart_common_style_node_preserves_best_effort_paint_and_source_precedence() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let render = |source: &str, portability| {
            let parsed = theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart choice source should produce a render model");
            let session = crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(portability)
                .begin_session_with_theme(&theme)
                .expect("begin Flowchart choice render session");
            prepare(parsed, &LayoutOptions::default(), session)
                .expect("prepare Flowchart choice")
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        };

        let themed = render(
            "flowchart LR\nA@{ shape: choice }\n",
            ThemePortabilityRequirement::BestEffort,
        )
        .expect("best-effort choice output should preserve typed paint");
        assert!(themed.svg().contains("fill:#ef4444 !important"));
        assert!(themed.style_report().residuals().is_empty());
        assert_eq!(themed.style_report().theme_residuals().len(), 1);
        assert_eq!(
            themed.style_report().theme_residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedPaint
        );

        let sourced = render(
            "flowchart LR\nstyle A fill:#22c55e\nA@{ shape: choice }\n",
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("verified static source paint should supersede typed choice paint");
        assert!(sourced.svg().contains("fill:#22c55e !important"));
        assert!(!sourced.svg().contains("fill:#ef4444 !important"));
        assert!(sourced.style_report().residuals().is_empty());
        assert!(sourced.style_report().theme_residuals().is_empty());
        assert_eq!(
            sourced.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
    }

    #[test]
    fn flowchart_unemitted_source_paint_cannot_hide_theme_residual() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        for source in [
            "flowchart LR\nstyle A fill:#22c55e\nA@{ shape: start }\n",
            "flowchart LR\nstyle A fill:#22c55e\nA@{ shape: icon, label: \"Plain\" }\n",
        ] {
            let parse = || {
                theme
                    .install_parse_compatibility(Engine::new())
                    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                    .unwrap()
                    .expect("Flowchart source should produce a render model")
            };

            let rendered = prepare(
                parse(),
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .begin_session_with_theme(&theme)
                    .expect("begin best-effort render session"),
            )
            .expect("prepare Flowchart source")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("best-effort output should preserve Mermaid emission semantics");

            assert!(!rendered.svg().contains("fill:#22c55e !important"));
            assert!(!rendered.svg().contains("fill:#ef4444 !important"));
            assert_eq!(rendered.style_report().residuals().len(), 1);
            assert_eq!(
                rendered.style_report().residuals()[0].reason(),
                FamilyStyleResidualReason::UnsupportedSurface
            );
            assert_eq!(rendered.style_report().theme_residuals().len(), 1);
            assert_eq!(
                rendered.style_report().theme_residuals()[0].reason(),
                FamilyThemeResidualReason::UnsupportedPaint
            );

            let artifact = prepare(
                parse(),
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .begin_session_with_theme(&theme)
                    .expect("begin strict render session"),
            )
            .expect("strict verification must wait for SVG emission");
            let error = match artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            {
                Ok(_) => panic!("unemitted paint must fail strict portability"),
                Err(error) => error,
            };
            assert_eq!(
                error.unverified_family_theme(),
                Some((
                    RenderFamilyKind::Flowchart,
                    rendered.style_report().theme_residuals().len(),
                ))
            );
        }
    }

    #[test]
    fn flowchart_generated_default_class_css_is_a_real_start_paint_surface() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nclassDef default fill:#22c55e\nA@{ shape: start }\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict render session");

        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("generated default class paint is decided during SVG emission")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("static generated class paint should remain portable");

        assert!(rendered.svg().contains("fill:rgb(34, 197, 94)!important"));
        assert!(!rendered.svg().contains("fill:#ef4444 !important"));
        assert!(rendered.style_report().residuals().is_empty());
        assert!(rendered.style_report().theme_residuals().is_empty());
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
    }

    #[test]
    fn flowchart_start_uses_the_generated_css_winner_that_reaches_its_wrapper() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nclassDef default fill:#22c55e\nclassDef explicit fill:#f97316\nA@{ shape: start }\nclass A explicit\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict render session");

        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("generated CSS winner is decided during SVG emission")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("the reachable static default class remains portable");

        assert!(rendered.svg().contains("fill:rgb(34, 197, 94)!important"));
        assert!(rendered.svg().contains("fill:rgb(249, 115, 22)!important"));
        assert!(!rendered.svg().contains("fill:#ef4444 !important"));
        assert!(rendered.style_report().residuals().is_empty());
        assert!(rendered.style_report().theme_residuals().is_empty());
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
    }

    #[test]
    fn flowchart_anchor_dynamic_generated_css_is_fail_closed() {
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new())
            .expect("compile empty typed theme");
        let source = "flowchart LR\nclassDef default opacity:var(--alpha)\nA@{ shape: anchor }\n";
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };

        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort session"),
        )
        .expect("prepare generated CSS fixture")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should preserve generated CSS");

        assert!(
            rendered.style_report().residuals().iter().any(|residual| {
                residual.owner_id() == "default"
                    && residual.property() == Some("opacity")
                    && residual.reason() == FamilyStyleResidualReason::InvalidValue
            }),
            "residuals={:?}",
            rendered.style_report().residuals()
        );

        let artifact = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict session"),
        )
        .expect("generated CSS verification waits for emission");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("dynamic generated CSS must fail strict portability"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_style(),
            Some((
                RenderFamilyKind::Flowchart,
                rendered.style_report().residuals().len(),
            ))
        );
    }

    #[test]
    fn flowchart_generated_css_emits_important_once() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nclassDef default fill:#22c55e !important\nA@{ shape: start }\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let rendered = prepare(parsed, &LayoutOptions::default(), session())
            .expect("prepare important fixture")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render important fixture");

        assert!(rendered.svg().contains("fill:rgb(34, 197, 94)!important"));
        assert!(!rendered.svg().contains("important!important"));
    }

    #[test]
    fn flowchart_direct_class_style_outranks_generated_class_css() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let source = "flowchart LR\nclassDef z fill:var(--paint)\nclassDef default fill:#22c55e\nA[Alpha]:::z\nstyle A --paint:#f97316\n";
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };

        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort render session"),
        )
        .expect("prepare class precedence fixture")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should retain the direct class winner");

        assert!(rendered.svg().contains("fill:var(--paint) !important"));
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "A"
                && residual.property() == Some("fill")
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }));
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );

        let artifact = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict render session"),
        )
        .expect("source precedence is decided during SVG emission");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("dynamic direct class paint must fail strict portability"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_style(),
            Some((
                RenderFamilyKind::Flowchart,
                rendered.style_report().residuals().len(),
            ))
        );
    }

    #[test]
    fn flowchart_hand_drawn_keeps_dynamic_source_paint_in_the_render_transport() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "%%{init: {\"look\": \"handDrawn\", \"handDrawnSeed\": 7}}%%\nflowchart LR\nstyle A --paint:#22c55e,fill:var(--paint)\nA[Alpha]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin render session");

        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("prepare hand-drawn Flowchart")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("best-effort output should retain the dynamic paint transport");

        assert!(rendered.svg().contains("stroke=\"var(--paint)\""));
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "A"
                && residual.property() == Some("fill")
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }));
    }

    #[test]
    fn flowchart_raw_theme_css_invalidates_family_theme_evidence() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let source = "%%{init: {\"themeCSS\": \".node rect { fill: #22c55e; }\"}}%%\nflowchart LR\nA[Alpha]\n";
        let environment = || {
            crate::environment::RenderEnvironment::deterministic().with_theme_admission_policy(
                crate::diagram_theme::ThemeAdmissionPolicy::permissive().with_trusted_lanes(
                    crate::diagram_theme::TrustedThemeLanes::from_allowed([
                        crate::diagram_theme::TrustedThemeLane::RawThemeCss,
                    ]),
                ),
            )
        };
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new().with_site_config(
                    MermaidConfig::from_value(json!({
                        "secure": [
                            "secure",
                            "securityLevel",
                            "startOnLoad",
                            "maxTextSize",
                            "suppressErrorRendering",
                            "maxEdges"
                        ]
                    })),
                ))
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };

        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            environment()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort render session"),
        )
        .expect("prepare raw theme CSS")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output may retain raw theme CSS");
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert_eq!(rendered.style_report().theme_residuals().len(), 1);
        assert_eq!(
            rendered.style_report().theme_residuals()[0].reason(),
            FamilyThemeResidualReason::OutputMutation
        );
        assert!(rendered.style_report().output_mutated());

        let artifact = prepare(
            parse(),
            &LayoutOptions::default(),
            environment()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict render session"),
        )
        .expect("raw theme CSS is evaluated after SVG emission");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("raw theme CSS must invalidate strict family evidence"),
                Err(error) => error,
            };
        assert!(matches!(
            error,
            Error::UnverifiedFamilyOutputMutation {
                family_kind: RenderFamilyKind::Flowchart
            }
        ));
    }

    #[test]
    fn raw_theme_css_rejects_strict_output_when_typed_rules_are_not_applicable() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let source = "%%{init: {\"themeCSS\": \".node rect { fill: #f97316; }\"}}%%\nflowchart LR\nA[Alpha]\n";
        let engine = theme.install_parse_compatibility(Engine::new().with_site_config(
            MermaidConfig::from_value(json!({
                "secure": [
                    "secure",
                    "securityLevel",
                    "startOnLoad",
                    "maxTextSize",
                    "suppressErrorRendering",
                    "maxEdges"
                ],
                "themeVariables": {"mainBkg": "#22c55e"}
            })),
        ));
        let parsed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_admission_policy(
                crate::diagram_theme::ThemeAdmissionPolicy::permissive().with_trusted_lanes(
                    crate::diagram_theme::TrustedThemeLanes::from_allowed([
                        crate::diagram_theme::TrustedThemeLane::RawThemeCss,
                    ]),
                ),
            )
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict render session");
        let artifact = prepare(parsed, &LayoutOptions::default(), session)
            .expect("raw theme CSS mutation is observed after SVG emission");

        let error = match artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        {
            Ok(_) => panic!("raw theme CSS must reject strict output independently of rule state"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            Error::UnverifiedFamilyOutputMutation {
                family_kind: RenderFamilyKind::Flowchart
            }
        ));
    }

    #[test]
    fn flowchart_emitted_dynamic_shape_property_is_a_source_residual() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let source = "flowchart LR\nstyle A stroke-width:var(--width)\nA[Alpha]\n";
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };

        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort render session"),
        )
        .expect("prepare dynamic source property")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should preserve dynamic source property");

        assert!(
            rendered
                .svg()
                .contains("stroke-width:var(--width) !important")
        );
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "A"
                && residual.property() == Some("stroke-width")
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }));

        let artifact = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict render session"),
        )
        .expect("source residual is discovered during SVG emission");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("dynamic source property must fail strict portability"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_style(),
            Some((
                RenderFamilyKind::Flowchart,
                rendered.style_report().residuals().len(),
            ))
        );
    }

    #[test]
    fn flowchart_edge_dynamic_shape_style_is_a_source_residual() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let source = "flowchart LR\nA --> B\nlinkStyle 0 stroke:var(--marker),stroke:#111827,stroke-width:var(--width),opacity:var(--alpha),filter:url(#alpha)\n";
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };

        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort render session"),
        )
        .expect("prepare dynamic edge style")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should preserve dynamic edge style");

        assert!(rendered.svg().contains("stroke-width:var(--width)"));
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "L_A_B_0"
                && residual.property() == Some("stroke-width")
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }));
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "L_A_B_0"
                && residual.property() == Some("opacity")
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }));
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "L_A_B_0" && residual.raw().contains("filter:url(#alpha)")
        }));
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "L_A_B_0" && residual.raw().contains("stroke:var(--marker)")
        }));

        let artifact = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict render session"),
        )
        .expect("strict verification must wait for edge emission");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("dynamic edge style must fail strict portability"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_style().map(|(family, _)| family),
            Some(RenderFamilyKind::Flowchart)
        );
    }

    #[test]
    fn flowchart_assigned_class_marker_style_is_a_source_residual() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let source =
            "flowchart LR\nA edge@--> B\nclassDef dynamic stroke:var(--edge)\nclass edge dynamic\n";
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };

        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort render session"),
        )
        .expect("prepare assigned-class marker style")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should preserve assigned-class marker style");

        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "edge"
                && residual.class_id() == Some("dynamic")
                && residual.origin() == FamilyStyleOrigin::AssignedClass
                && residual.property() == Some("stroke")
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }));

        let artifact = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict render session"),
        )
        .expect("strict verification must wait for marker emission");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("dynamic assigned-class marker must fail strict portability"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_style().map(|(family, _)| family),
            Some(RenderFamilyKind::Flowchart)
        );
    }

    #[test]
    fn flowchart_edge_label_invalid_typography_is_a_source_residual() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let source = "flowchart LR\nA -->|label| B\nlinkStyle 0 font-weight:banana\n";
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };

        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort render session"),
        )
        .expect("prepare dynamic edge label style")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should preserve dynamic edge label style");

        assert!(rendered.svg().contains("font-weight:banana"));
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "L_A_B_0"
                && residual.property() == Some("font-weight")
                && residual.channel() == FamilyStyleChannel::Label
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }));

        let artifact = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict render session"),
        )
        .expect("strict verification must wait for edge label emission");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("dynamic edge label style must fail strict portability"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_style().map(|(family, _)| family),
            Some(RenderFamilyKind::Flowchart)
        );
    }

    #[test]
    fn flowchart_edge_sanitized_xhtml_and_empty_label_styles_are_source_residuals() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let source = r#"---
config:
  securityLevel: loose
  htmlLabels: true
  flowchart:
    htmlLabels: true
---
flowchart LR
A -->|"<span class='host-label' style='color:var(--accent)'>Label</span>"| B
B --> C
linkStyle 1 font-weight:banana
"#;
        let rendered = prepare(
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model"),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort render session"),
        )
        .expect("prepare HTML edge labels")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should preserve sanitized XHTML styles");

        assert!(rendered.svg().contains("class='host-label'"));
        assert!(rendered.svg().contains("style='color:var(--accent)'"));
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "L_A_B_0"
                && residual.property() == Some("color")
                && residual.origin() == FamilyStyleOrigin::LabelStyle
                && residual.channel() == FamilyStyleChannel::Label
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }));
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "L_A_B_0"
                && residual.property() == Some("class")
                && residual.origin() == FamilyStyleOrigin::LabelStyle
                && residual.channel() == FamilyStyleChannel::Label
                && residual.reason() == FamilyStyleResidualReason::UnsupportedProperty
        }));
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "L_B_C_0"
                && residual.property() == Some("font-weight")
                && residual.channel() == FamilyStyleChannel::Label
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }));
    }

    #[test]
    fn flowchart_cluster_dynamic_shape_style_is_a_source_residual() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let source = "flowchart TD\nsubgraph S[Service]\nA\nend\nstyle S stroke-width:var(--width),transform:translateX(1px)\n";
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };

        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort render session"),
        )
        .expect("prepare dynamic cluster style")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should preserve dynamic cluster style");

        assert!(rendered.svg().contains("stroke-width:var(--width)"));
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "S"
                && residual.property() == Some("stroke-width")
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }));
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "S"
                && residual.property() == Some("transform")
                && residual.reason() == FamilyStyleResidualReason::UnsupportedProperty
        }));

        let artifact = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict render session"),
        )
        .expect("strict verification must wait for cluster emission");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("dynamic cluster style must fail strict portability"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_style().map(|(family, _)| family),
            Some(RenderFamilyKind::Flowchart)
        );
    }

    #[test]
    fn flowchart_cluster_sanitized_xhtml_and_empty_title_styles_are_source_residuals() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let source = r#"---
config:
  securityLevel: loose
  htmlLabels: true
  flowchart:
    htmlLabels: true
---
flowchart TD
subgraph Rich["<span class='host-title'>Service</span>"]
A
end
subgraph Empty[" "]
B
end
style Empty font-weight:banana
"#;
        let rendered = prepare(
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model"),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort render session"),
        )
        .expect("prepare HTML cluster titles")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should preserve sanitized cluster XHTML");

        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "Rich"
                && residual.property() == Some("class")
                && residual.origin() == FamilyStyleOrigin::LabelStyle
                && residual.reason() == FamilyStyleResidualReason::UnsupportedProperty
        }));
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "Empty"
                && residual.property() == Some("font-weight")
                && residual.channel() == FamilyStyleChannel::Label
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }));
    }

    #[test]
    fn swimlane_cluster_dynamic_shape_style_is_a_source_residual() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(RenderFamilyKind::Swimlane),
                    ),
                ),
            )
            .expect("compile Swimlane Node theme");
        let source = "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nsubgraph Lane[Lane]\nA\nend\nstyle Lane opacity:var(--alpha),stroke-width:var(--width)\n";
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Swimlane source should produce a render model")
        };

        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort render session"),
        )
        .expect("prepare dynamic Swimlane style")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should preserve dynamic Swimlane style");

        assert_eq!(rendered.family_kind(), RenderFamilyKind::Swimlane);
        assert!(rendered.svg().contains("opacity:var(--alpha)"));
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "Lane"
                && residual.property() == Some("opacity")
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }));
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "Lane"
                && residual.property() == Some("stroke-width")
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }));

        let artifact = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict render session"),
        )
        .expect("strict verification must wait for Swimlane emission");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("dynamic Swimlane style must fail strict portability"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_style().map(|(family, _)| family),
            Some(RenderFamilyKind::Swimlane)
        );
    }

    #[test]
    fn swimlane_sanitized_xhtml_and_empty_title_styles_are_source_residuals() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(RenderFamilyKind::Swimlane),
                    ),
                ),
            )
            .expect("compile Swimlane Node theme");
        let source = r#"---
config:
  layout: swimlane
  securityLevel: loose
  htmlLabels: true
  flowchart:
    htmlLabels: true
---
flowchart TD
subgraph Rich["<span style='color:var(--lane-title)'>Lane</span>"]
A
end
subgraph Empty[" "]
B
end
style Empty font-weight:banana
"#;
        let rendered = prepare(
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Swimlane source should produce a render model"),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort render session"),
        )
        .expect("prepare HTML Swimlane titles")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should preserve sanitized Swimlane XHTML");

        assert_eq!(rendered.family_kind(), RenderFamilyKind::Swimlane);
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "Rich"
                && residual.property() == Some("color")
                && residual.origin() == FamilyStyleOrigin::LabelStyle
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }));
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "Empty"
                && residual.property() == Some("font-weight")
                && residual.channel() == FamilyStyleChannel::Label
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }));
    }

    #[test]
    fn swimlane_empty_edge_label_still_records_emitted_shape_style() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(RenderFamilyKind::Swimlane),
                    ),
                ),
            )
            .expect("compile Swimlane Node theme");
        let source = "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA -->| | B\nlinkStyle 0 opacity:var(--alpha)\n";
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Swimlane source should produce a render model");

        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort render session"),
        )
        .expect("prepare empty Swimlane edge label")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should preserve emitted edge label style");

        assert_eq!(rendered.family_kind(), RenderFamilyKind::Swimlane);
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.property() == Some("opacity")
                && residual.channel() == FamilyStyleChannel::Shape
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }));
    }

    #[test]
    fn flowchart_dynamic_label_typography_is_a_source_residual() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let source = "flowchart LR\nclassDef default --size:40px,font-size:var(--size)\nA[Alpha]\n";
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };

        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort session"),
        )
        .expect("prepare dynamic label fixture")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should preserve label CSS");

        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "A"
                && residual.property() == Some("font-size")
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }));

        let artifact = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict session"),
        )
        .expect("dynamic label verification waits for emission");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("dynamic label typography must fail strict portability"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_style(),
            Some((
                RenderFamilyKind::Flowchart,
                rendered.style_report().residuals().len(),
            ))
        );
    }

    #[test]
    fn flowchart_icon_label_typography_is_emitted_and_residualized() {
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new())
            .expect("compile empty theme session");
        let source = "flowchart LR\nclassDef default --size:40px,font-size:var(--size)\nA@{ icon: \"missing:icon\", label: \"Alpha\" }\n";
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };

        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort session"),
        )
        .expect("prepare icon label fixture")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort output should preserve icon label CSS");

        assert!(rendered.svg().contains("font-size:var(--size) !important"));
        assert!(rendered.style_report().residuals().iter().any(|residual| {
            residual.owner_id() == "A"
                && residual.property() == Some("font-size")
                && residual.reason() == FamilyStyleResidualReason::InvalidValue
        }));

        let artifact = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict session"),
        )
        .expect("dynamic icon label verification waits for emission");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("dynamic icon label typography must fail strict portability"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_style(),
            Some((
                RenderFamilyKind::Flowchart,
                rendered.style_report().residuals().len(),
            ))
        );
    }

    #[test]
    fn flowchart_empty_subgraph_consumes_typed_node_paint() {
        let theme = flowchart_node_theme(
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ef4444").unwrap()),
        );
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "flowchart TD\nsubgraph Empty\nend\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("empty subgraph source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable render session");

        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("empty subgraph theme must reach SVG emission")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("emitted empty subgraph paint must satisfy strict portability");

        assert!(rendered.svg().contains("fill:#ef4444 !important"));
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
    }

    #[test]
    fn flowchart_icon_without_asset_retains_unemitted_fill_as_residual() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(RenderFamilyKind::Flowchart),
                    ),
                ),
            )
            .expect("compile Flowchart icon theme");
        let source = "flowchart LR\nI@{ shape: icon, label: \"Plain\" }\n";
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart icon source should produce a render model")
        };

        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort render session"),
        )
        .expect("prepare Flowchart icon")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render best-effort Flowchart icon");

        assert!(!rendered.svg().contains("#ef4444"));
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert_eq!(
            rendered.style_report().theme_residuals(),
            &[FamilyThemeResidual {
                key: FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Node,
                },
                reason: FamilyThemeResidualReason::UnsupportedPaint,
            }]
        );

        let strict_session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict render session");
        let artifact = prepare(parse(), &LayoutOptions::default(), strict_session)
            .expect("strict verification must wait for icon emission");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("unemitted icon fill must fail strict portability"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_theme(),
            Some((
                RenderFamilyKind::Flowchart,
                rendered.style_report().theme_residuals().len(),
            ))
        );
    }

    #[test]
    fn require_portable_rejects_flowchart_unsupported_styles_after_svg_emission() {
        let gradient = LinearGradient::new(
            90.0,
            [
                GradientStop::new(0.0, ThemeColorValue::parse("#0f172a").unwrap()).unwrap(),
                GradientStop::new(1.0, ThemeColorValue::parse("#22d3ee").unwrap()).unwrap(),
            ],
        )
        .unwrap();
        let cases = [
            (
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::LinearGradient(gradient))
                    .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                FamilyThemeResidualReason::UnsupportedPaint,
                "flowchart LR\nA --> B\n",
            ),
            (
                ThemeStylePatch {
                    geometry: crate::diagram_theme::ThemeGeometryPatch {
                        radius: crate::diagram_theme::Specified::Value(8.0),
                    },
                    ..ThemeStylePatch::default()
                },
                FamilyThemeResidualReason::UnsupportedGeometry,
                "flowchart LR\nA@{ shape: choice }\n",
            ),
        ];

        for (style, expected_reason, source) in cases {
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(
                        ThemeRuleSet::default().with_rule(
                            ThemeRule::new(ThemeTarget::Node, style)
                                .for_family(RenderFamilyKind::Flowchart),
                        ),
                    ),
                )
                .expect("compile unsupported Flowchart theme");
            let parse = || {
                theme
                    .install_parse_compatibility(Engine::new())
                    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                    .unwrap()
                    .expect("Flowchart source should produce a render model")
            };
            let rendered = prepare(
                parse(),
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .begin_session_with_theme(&theme)
                    .expect("begin best-effort render session"),
            )
            .expect("prepare unsupported Flowchart theme")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("best-effort Flowchart output should retain residual evidence");
            assert_eq!(rendered.style_report().theme_residuals().len(), 1);
            assert_eq!(
                rendered.style_report().theme_residuals()[0].reason(),
                expected_reason
            );

            let session = crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict portable render session");
            let artifact = prepare(parse(), &LayoutOptions::default(), session)
                .expect("Flowchart theme verification must wait for SVG emission");

            let error = match artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            {
                Ok(_) => panic!("unsupported Flowchart style must remain fail-closed"),
                Err(error) => error,
            };
            assert_eq!(
                error.unverified_family_theme(),
                Some((
                    RenderFamilyKind::Flowchart,
                    rendered.style_report().theme_residuals().len(),
                ))
            );
        }
    }

    #[test]
    fn require_portable_accepts_flowchart_node_ordinal_palette_after_svg_emission() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_ordinal_palette(
                        ThemeTarget::Node,
                        OrdinalPalette::new([
                            ThemeColorValue::parse("#ef4444").unwrap(),
                            ThemeColorValue::parse("#2563eb").unwrap(),
                        ])
                        .unwrap(),
                    ),
                ),
            )
            .expect("compile Flowchart ordinal palette theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nA[Alpha] --> B[Beta]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict portable render session"),
        )
        .expect("prepare Flowchart ordinal palette")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("emitted ordinal palette must satisfy strict portability");

        assert!(
            flowchart_node_shape_style(rendered.svg(), "A").contains("fill:#ef4444 !important")
        );
        assert!(
            flowchart_node_shape_style(rendered.svg(), "B").contains("fill:#2563eb !important")
        );
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Verified
        );
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::Node,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }

    #[test]
    fn require_portable_accepts_swimlane_node_ordinal_palette_after_svg_emission() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_ordinal_palette(
                        ThemeTarget::Node,
                        OrdinalPalette::new([
                            ThemeColorValue::parse("#ef4444").unwrap(),
                            ThemeColorValue::parse("#2563eb").unwrap(),
                        ])
                        .unwrap(),
                    ),
                ),
            )
            .expect("compile Swimlane ordinal palette theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA[Alpha] --> B[Beta]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Swimlane source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict portable render session"),
        )
        .expect("prepare Swimlane ordinal palette")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("emitted Swimlane ordinal palette must satisfy strict portability");

        assert_eq!(rendered.family_kind(), RenderFamilyKind::Swimlane);
        assert!(
            flowchart_node_shape_style(rendered.svg(), "A").contains("fill:#ef4444 !important")
        );
        assert!(
            flowchart_node_shape_style(rendered.svg(), "B").contains("fill:#2563eb !important")
        );
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::Node,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }

    #[test]
    fn require_portable_rejects_flowchart_ordinal_rule_when_a_node_matches() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .with_ordinal(OrdinalSelector::exact(2).unwrap())
                        .for_family(RenderFamilyKind::Flowchart),
                    ),
                ),
            )
            .expect("compile ordinal Flowchart rule");
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(
                    "flowchart LR\nA[Alpha] --> B[Beta]\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .expect("Flowchart source should produce a render model")
        };
        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort render session"),
        )
        .expect("prepare ordinal Flowchart rule")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort ordinal rule should retain residual evidence");
        assert_eq!(
            rendered.style_report().theme_residuals(),
            &[FamilyThemeResidual {
                key: FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Node,
                },
                reason: FamilyThemeResidualReason::UnsupportedPaint,
            }]
        );

        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable render session");
        let artifact = prepare(parse(), &LayoutOptions::default(), session)
            .expect("ordinal verification must wait for SVG emission");

        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("matching unsupported ordinal rule must fail closed"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_theme(),
            Some((
                RenderFamilyKind::Flowchart,
                rendered.style_report().theme_residuals().len(),
            ))
        );
    }

    #[test]
    fn require_portable_still_rejects_flowchart_legacy_edge_paint_before_svg() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Edge,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(RenderFamilyKind::Flowchart),
                    ),
                ),
            )
            .expect("compile legacy Flowchart edge theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable render session");

        let error = match prepare(parsed, &LayoutOptions::default(), session) {
            Ok(_) => panic!("legacy Flowchart edge paint must fail before layout"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            Error::LegacyFamilyThemeCompatibility {
                family_kind: RenderFamilyKind::Flowchart,
                residual_count: 1..,
            }
        ));
    }

    #[test]
    fn require_portable_accepts_sequence_actor_fill_after_svg_emission() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Actor,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(RenderFamilyKind::Sequence),
                    ),
                ),
            )
            .expect("compile Sequence actor theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Sequence source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict portable Sequence session"),
        )
        .expect("Sequence actor theme should prepare")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("Sequence actor fill should be proven by the SVG writer");

        assert!(
            rendered.svg().contains("fill:#ef4444"),
            "Sequence SVG should contain the typed actor fill"
        );
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Verified
        );
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Actor,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }

    #[test]
    fn require_portable_accepts_sequence_actor_stroke_after_svg_emission() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Actor,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                        )
                        .for_family(RenderFamilyKind::Sequence),
                    ),
                ),
            )
            .expect("compile Sequence actor stroke theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Sequence source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict portable Sequence session"),
        )
        .expect("Sequence actor stroke theme should prepare")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("Sequence actor stroke should be proven by the SVG writer");

        assert!(
            rendered.svg().contains("stroke:#2563eb"),
            "Sequence SVG should contain the typed actor stroke"
        );
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Verified
        );
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Actor,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }

    #[test]
    fn sequence_actor_stroke_explicit_mermaid_border_owns_precedence() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Actor,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                        )
                        .for_family(RenderFamilyKind::Sequence),
                    ),
                ),
            )
            .expect("compile Sequence actor stroke theme");
        let engine = theme.install_parse_compatibility(Engine::new().with_site_config(
            MermaidConfig::from_value(json!({
                "themeVariables": {"actorBorder": "#22c55e"}
            })),
        ));
        let parsed = engine
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Sequence source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict portable Sequence session"),
        )
        .expect("explicit Mermaid actor stroke should remain evaluable")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("explicit Mermaid actor stroke should satisfy strict portability");

        assert!(rendered.svg().contains("#22c55e"), "{}", rendered.svg());
        assert!(!rendered.svg().contains("#2563eb"), "{}", rendered.svg());
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Actor,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }

    #[test]
    fn sequence_actor_stroke_with_unhandled_shapes_fails_closed() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Actor,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                        )
                        .for_family(RenderFamilyKind::Sequence),
                    ),
                ),
            )
            .expect("compile Sequence actor stroke theme");

        for source in [
            "sequenceDiagram\nparticipant Alice@{\"type\":\"control\"}\nparticipant Bob\nAlice->>Bob: Hello\n",
            "sequenceDiagram\nparticipant Alice@{\"type\":\"database\"}\nparticipant Bob\nAlice->>Bob: Hello\n",
            "sequenceDiagram\nparticipant Alice\nparticipant Bob\nproperties Alice: {\"class\":\"custom\"}\nAlice->>Bob: Hello\n",
        ] {
            let parsed = theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Sequence source should produce a render model");
            let strict = crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict portable Sequence session");
            let artifact = prepare(parsed, &LayoutOptions::default(), strict)
                .expect("strict verification must wait for terminal SVG evidence");
            let error = match artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            {
                Ok(_) => panic!("unhandled actor stroke shape must not be signed as portable"),
                Err(error) => error,
            };
            assert_eq!(
                error.unverified_family_theme(),
                Some((RenderFamilyKind::Sequence, 1))
            );
        }
    }

    #[test]
    fn sequence_actor_stroke_tracks_every_css_covered_shape() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Actor,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                        )
                        .for_family(RenderFamilyKind::Sequence),
                    ),
                ),
            )
            .expect("compile Sequence actor stroke theme");

        for actor_type in ["actor", "boundary", "entity", "collections", "queue"] {
            let source = format!(
                "sequenceDiagram\nparticipant Alice@{{\"type\":\"{actor_type}\"}}\nparticipant Bob\nAlice->>Bob: Hello\n"
            );
            let parsed = theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .expect("Sequence source should produce a render model");
            let rendered = prepare(
                parsed,
                &LayoutOptions::default(),
                crate::environment::RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .begin_session_with_theme(&theme)
                    .expect("begin strict portable Sequence session"),
            )
            .expect("strict verification must wait for terminal SVG evidence")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap_or_else(|error| {
                panic!("{actor_type} should consume the typed actor stroke: {error}")
            });

            assert!(
                rendered.svg().contains("stroke:#2563eb"),
                "{actor_type} SVG should contain the typed actor stroke: {}",
                rendered.svg()
            );
            assert_eq!(
                rendered.style_report().theme_applied_mechanisms(),
                &[FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Actor,
                }],
                "{actor_type} should produce writer-owned Actor evidence"
            );
            assert!(rendered.style_report().theme_residuals().is_empty());
        }
    }

    #[test]
    fn sequence_actor_stroke_transparent_paint_is_signed_only_for_covered_shapes() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Actor,
                            ThemeStylePatch::default().with_stroke(CanvasPaint::Transparent),
                        )
                        .for_family(RenderFamilyKind::Sequence),
                    ),
                ),
            )
            .expect("compile transparent Sequence actor stroke theme");
        let parse = |source: &str| {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Sequence source should produce a render model")
        };
        let strict = || {
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict portable Sequence session")
        };

        let rendered = prepare(
            parse(
                "sequenceDiagram\nparticipant Alice@{\"type\":\"queue\"}\nparticipant Bob\nAlice->>Bob: Hello\n",
            ),
            &LayoutOptions::default(),
            strict(),
        )
        .expect("transparent actor stroke should wait for terminal SVG evidence")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("queue should consume the transparent typed actor stroke");
        assert!(rendered.svg().contains("stroke:transparent"));
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Actor,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());

        let artifact = prepare(
            parse(
                "sequenceDiagram\nparticipant Alice@{\"type\":\"control\"}\nparticipant Bob\nAlice->>Bob: Hello\n",
            ),
            &LayoutOptions::default(),
            strict(),
        )
        .expect("control stroke verification must wait for terminal SVG evidence");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("control marker must not be signed as transparent typed stroke"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_theme(),
            Some((RenderFamilyKind::Sequence, 1))
        );
    }

    #[test]
    fn sequence_actor_stroke_does_not_recolor_autonumber_carrier_or_message_lines() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Actor,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                        )
                        .for_family(RenderFamilyKind::Sequence),
                    ),
                ),
            )
            .expect("compile Sequence actor stroke theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nautonumber\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("autonumbered Sequence source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict portable Sequence session"),
        )
        .expect("actor stroke should wait for terminal SVG evidence")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("autonumbered Sequence should retain isolated Actor stroke evidence");
        let svg = rendered.svg();

        assert!(svg.contains("stroke:#2563eb"), "{svg}");
        assert!(
            svg.contains(r#"class="messageLine0""#),
            "fixture must contain a non-Actor message line: {svg}"
        );
        assert!(
            svg.contains(r#"stroke-width="0" marker-start="url(#merman-sequencenumber)""#),
            "fixture must contain the unclassified autonumber carrier: {svg}"
        );
        let typed_rules_start = svg
            .find("#merman .actor{stroke:#2563eb;}")
            .expect("typed Actor stroke rule should be emitted");
        let typed_rules_end = svg[typed_rules_start..]
            .find("#merman g rect.rect")
            .map(|offset| typed_rules_start + offset)
            .expect("typed Actor stroke rules should precede the next legacy rule");
        let typed_rules = &svg[typed_rules_start..typed_rules_end];
        assert!(!typed_rules.contains("#merman line"), "{typed_rules}");
        assert!(!typed_rules.contains("messageLine"), "{typed_rules}");
        assert!(!typed_rules.contains("sequencenumber"), "{typed_rules}");
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Actor,
            }]
        );
    }

    #[test]
    fn sequence_explicit_actor_fill_owns_precedence_over_typed_actor_fill() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Actor,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(RenderFamilyKind::Sequence),
                    ),
                ),
            )
            .expect("compile Sequence actor theme");
        let engine = theme.install_parse_compatibility(Engine::new().with_site_config(
            MermaidConfig::from_value(json!({
                "themeVariables": {"actorBkg": "#22c55e"}
            })),
        ));
        let parsed = engine
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Sequence source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict portable Sequence session"),
        )
        .expect("explicit Mermaid actor fill should remain evaluable")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("explicit Mermaid actor fill should satisfy strict portability");

        assert!(rendered.svg().contains("#22c55e"), "{}", rendered.svg());
        assert!(!rendered.svg().contains("#ef4444"), "{}", rendered.svg());
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Actor,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }

    #[test]
    fn sequence_unsupported_actor_gradient_fails_closed_after_svg_emission() {
        let gradient = LinearGradient::new(
            90.0,
            [
                GradientStop::new(0.0, ThemeColorValue::parse("#000000").unwrap()).unwrap(),
                GradientStop::new(1.0, ThemeColorValue::parse("#ffffff").unwrap()).unwrap(),
            ],
        )
        .unwrap();
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Actor,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::LinearGradient(gradient)),
                        )
                        .for_family(RenderFamilyKind::Sequence),
                    ),
                ),
            )
            .expect("compile unsupported Sequence actor gradient");
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(
                    "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .expect("Sequence source should produce a render model")
        };
        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort Sequence session"),
        )
        .expect("best-effort preparation should retain unsupported evidence")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort Sequence output should remain renderable");
        assert_eq!(
            rendered.style_report().theme_residuals(),
            &[FamilyThemeResidual {
                key: FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Actor,
                },
                reason: FamilyThemeResidualReason::UnsupportedPaint,
            }]
        );

        let strict = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict Sequence session");
        let artifact = prepare(parse(), &LayoutOptions::default(), strict)
            .expect("strict verification waits for terminal SVG evidence");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("unsupported Sequence actor gradient must fail closed"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_theme(),
            Some((RenderFamilyKind::Sequence, 1))
        );

        let configured_parse = || {
            theme
                .install_parse_compatibility(Engine::new().with_site_config(
                    MermaidConfig::from_value(json!({
                        "themeVariables": {"actorBkg": "#22c55e"}
                    })),
                ))
                .parse_diagram_for_render_model_sync(
                    "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .expect("configured Sequence source should produce a render model")
        };
        let configured = prepare(
            configured_parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin configured strict Sequence session"),
        )
        .expect("explicit actor fill should suppress the unsupported theme gradient")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("explicit actor fill should satisfy strict portability");
        assert!(configured.svg().contains("#22c55e"), "{}", configured.svg());
        assert!(configured.style_report().theme_residuals().is_empty());
        assert_eq!(
            configured.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Actor,
            }]
        );
    }

    #[test]
    fn sequence_actor_fill_with_custom_class_does_not_claim_typed_consumption() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Actor,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(RenderFamilyKind::Sequence),
                    ),
                ),
            )
            .expect("compile Sequence actor theme");
        let source = r#"sequenceDiagram
participant Alice
participant Bob
properties Alice: {"class":"custom"}
Alice->>Bob: Hello
"#;
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Sequence source should produce a render model")
        };
        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort Sequence session"),
        )
        .expect("prepare Sequence actor theme")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort Sequence output should remain renderable");

        assert!(
            rendered.svg().contains("fill:#ef4444"),
            "{}",
            rendered.svg()
        );
        assert!(
            rendered.svg().contains("fill=\"#EDF2AE\""),
            "custom actor fill must remain source-owned: {}",
            rendered.svg()
        );
        assert_eq!(
            rendered.style_report().theme_residuals(),
            &[FamilyThemeResidual {
                key: FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Actor,
                },
                reason: FamilyThemeResidualReason::UnsupportedPaint,
            }]
        );
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );

        let strict = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict Sequence session");
        let artifact = prepare(parse(), &LayoutOptions::default(), strict)
            .expect("strict verification must wait for terminal SVG evidence");
        let error =
            match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default()) {
                Ok(_) => panic!("custom actor class must not be signed as typed portable fill"),
                Err(error) => error,
            };
        assert_eq!(
            error.unverified_family_theme(),
            Some((RenderFamilyKind::Sequence, 1))
        );
    }

    #[test]
    fn sequence_actor_fill_tracks_actor_man_css_and_inline_precedence() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Actor,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(RenderFamilyKind::Sequence),
                    ),
                ),
            )
            .expect("compile Sequence actor theme");

        for (actor_type, should_be_portable) in [
            ("actor", true),
            ("boundary", true),
            ("entity", true),
            ("collections", true),
            ("queue", true),
            ("database", true),
            ("control", false),
        ] {
            let source = format!(
                "sequenceDiagram\nparticipant Alice@{{\"type\":\"{actor_type}\"}}\nparticipant Bob\nAlice->>Bob: Hello\n"
            );
            let parsed = theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap()
                .expect("Sequence source should produce a render model");
            let strict = crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict Sequence session");
            let artifact = prepare(parsed, &LayoutOptions::default(), strict)
                .expect("strict verification must wait for terminal SVG evidence");
            let result =
                artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());

            if should_be_portable {
                let rendered = result.expect("actor-man CSS-covered fill should be portable");
                assert!(rendered.svg().contains("fill:#ef4444"));
                assert_eq!(
                    rendered.style_report().theme_applied_mechanisms(),
                    &[FamilyThemeMechanismKey::Rule {
                        index: 0,
                        target: ThemeTarget::Actor,
                    }]
                );
                assert!(rendered.style_report().theme_residuals().is_empty());
            } else {
                let error = match result {
                    Ok(_) => panic!("control marker fill must not be signed as typed"),
                    Err(error) => error,
                };
                assert_eq!(
                    error.unverified_family_theme(),
                    Some((RenderFamilyKind::Sequence, 1))
                );
            }
        }
    }

    #[test]
    fn sequence_actor_fill_accounts_for_rules_without_matching_actors() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Actor,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .with_ordinal(OrdinalSelector::exact(3).unwrap())
                        .for_family(RenderFamilyKind::Sequence),
                    ),
                ),
            )
            .expect("compile unmatched ordinal Sequence actor theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Sequence source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict Sequence session"),
        )
        .expect("prepare unmatched ordinal Sequence actor rule")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("unmatched ordinal Sequence actor rule should be not applicable");

        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Actor,
            }]
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
    }

    #[test]
    fn sequence_actor_ordinal_evidence_charges_once_per_actual_actor() {
        let ordinal = OrdinalSelector::cycle(1, 0).unwrap();
        let mut radius = ThemeStylePatch::default();
        radius.geometry.radius = Specified::Value(6.0);
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::Actor,
                                ThemeStylePatch::default()
                                    .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                            )
                            .with_ordinal(ordinal)
                            .for_family(RenderFamilyKind::Sequence),
                        )
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::Actor,
                                ThemeStylePatch::default()
                                    .with_stroke(CanvasPaint::solid("#2563eb").unwrap()),
                            )
                            .with_ordinal(ordinal)
                            .for_family(RenderFamilyKind::Sequence),
                        )
                        .with_rule(
                            ThemeRule::new(ThemeTarget::Actor, radius)
                                .with_ordinal(ordinal)
                                .for_family(RenderFamilyKind::Sequence),
                        ),
                ),
            )
            .expect("compile ordinal Sequence actor theme");
        let source =
            "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n";
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Sequence source should produce a render model")
        };
        let environment_with_limit = |limit| {
            crate::environment::RenderEnvironment::deterministic()
                .with_resource_policy(
                    crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                        .with_limit(
                            crate::resources::ResourceLimitId::MaxLayoutWorkUnits,
                            limit,
                        )
                        .unwrap(),
                )
                .begin_session_with_theme(&theme)
                .expect("begin bounded Sequence session")
        };

        let unbounded = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin unbounded Sequence session"),
        )
        .expect("prepare unbounded Sequence artifact");
        let layout_work_units = unbounded.context.session.work_meter().used();
        drop(unbounded);

        // Three ordinal candidates are resolved once for each of the two actual actors. The
        // three facets must share those two resolutions instead of each rescanning both actors.
        let evidence_work_units = 3 * 2;
        let exact_limit = layout_work_units + evidence_work_units;
        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            environment_with_limit(exact_limit),
        )
        .expect("exact limit must admit Sequence layout")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("one ordinal resolution per actual actor must fit the exact limit");
        assert_eq!(rendered.session.work_meter().used(), exact_limit);
        assert_eq!(
            rendered.style_report().theme_residuals(),
            &[
                FamilyThemeResidual {
                    key: FamilyThemeMechanismKey::Rule {
                        index: 0,
                        target: ThemeTarget::Actor,
                    },
                    reason: FamilyThemeResidualReason::UnsupportedPaint,
                },
                FamilyThemeResidual {
                    key: FamilyThemeMechanismKey::Rule {
                        index: 1,
                        target: ThemeTarget::Actor,
                    },
                    reason: FamilyThemeResidualReason::UnsupportedPaint,
                },
                FamilyThemeResidual {
                    key: FamilyThemeMechanismKey::Rule {
                        index: 2,
                        target: ThemeTarget::Actor,
                    },
                    reason: FamilyThemeResidualReason::UnsupportedGeometry,
                },
            ]
        );

        let short_limit = exact_limit - 1;
        let artifact = prepare(
            parse(),
            &LayoutOptions::default(),
            environment_with_limit(short_limit),
        )
        .expect("short limit must still admit Sequence layout");
        let error = match artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        {
            Ok(_) => panic!("ordinal evidence must fail closed when its work exceeds the limit"),
            Err(error) => error,
        };
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected max_layout_work_units resource rejection")
        };
        assert_eq!(limit.limit, "max_layout_work_units");
        assert_eq!(limit.max, short_limit);
        assert_eq!(limit.actual, exact_limit);
    }

    #[test]
    fn sequence_actor_fill_winner_accounts_for_superseded_and_unsupported_facets() {
        let mut winning_style =
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#2563eb").unwrap());
        winning_style.geometry.radius = Specified::Value(6.0);
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::Actor,
                                ThemeStylePatch::default()
                                    .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                            )
                            .for_family(RenderFamilyKind::Sequence),
                        )
                        .with_rule(
                            ThemeRule::new(ThemeTarget::Actor, winning_style)
                                .for_family(RenderFamilyKind::Sequence),
                        ),
                ),
            )
            .expect("compile competing Sequence actor rules");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Sequence source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort Sequence session"),
        )
        .expect("prepare competing Sequence actor rules")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render competing Sequence actor rules");

        assert!(rendered.svg().contains("fill:#2563eb"));
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .is_empty()
        );
        assert_eq!(
            rendered.style_report().theme_not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Actor,
            }]
        );
        assert_eq!(
            rendered.style_report().theme_residuals(),
            &[FamilyThemeResidual {
                key: FamilyThemeMechanismKey::Rule {
                    index: 1,
                    target: ThemeTarget::Actor,
                },
                reason: FamilyThemeResidualReason::UnsupportedGeometry,
            }]
        );
    }

    #[test]
    fn state_style_residual_survives_terminal_svg_completion() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\nclassDef broken font-size:not-a-size\n[*] --> Ready:::broken\nReady --> [*]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("State source should produce a render model");
        let rendered = prepare(parsed, &LayoutOptions::default(), session())
            .expect("best-effort State preparation")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render State SVG");

        assert_eq!(rendered.family_kind(), RenderFamilyKind::State);
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Unverified
        );
        let residual = rendered
            .style_report()
            .residuals()
            .first()
            .cloned()
            .expect("invalid font-size residual");
        assert_eq!(residual.property(), Some("font-size"));
        assert_eq!(residual.owner_id(), "Ready");
        assert_eq!(residual.class_id(), Some("broken"));
        assert_eq!(residual.origin(), FamilyStyleOrigin::AssignedClass);
        assert_eq!(residual.channel(), FamilyStyleChannel::Label);
        assert_eq!(residual.reason(), FamilyStyleResidualReason::InvalidValue);

        let completion = rendered
            .finalize_resvg(&SvgPipeline::resvg_safe())
            .expect("finalize State SVG")
            .into_completion();
        assert_eq!(
            completion.report().style_report().residuals(),
            std::slice::from_ref(&residual)
        );
    }

    #[test]
    fn state_structured_gradient_is_a_semantic_residual_not_verified_source_css() {
        let gradient = LinearGradient::new(
            90.0,
            [
                GradientStop::new(
                    0.0,
                    ThemeColorValue::parse("#0f172a").expect("valid first stop"),
                )
                .expect("valid first stop"),
                GradientStop::new(
                    1.0,
                    ThemeColorValue::parse("#22d3ee").expect("valid second stop"),
                )
                .expect("valid second stop"),
            ],
        )
        .expect("valid gradient");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::State,
                        ThemeStylePatch::default().with_fill(CanvasPaint::LinearGradient(gradient)),
                    ),
                )),
            )
            .expect("compile gradient theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\n[*] --> Ready\nReady --> [*]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("State source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin themed session"),
        )
        .expect("best-effort State preparation")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render State SVG");

        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Unverified
        );
        let residual = rendered
            .style_report()
            .theme_residuals()
            .first()
            .expect("gradient must retain semantic residual");
        assert_eq!(
            residual.reason(),
            FamilyThemeResidualReason::UnsupportedPaint
        );
        assert!(rendered.style_report().residuals().is_empty());
    }

    #[test]
    fn state_ordinal_palette_is_applied_by_an_observed_state_binding() {
        let palette = OrdinalPalette::new([
            ThemeColorValue::parse("#0f172a").expect("valid first palette color"),
            ThemeColorValue::parse("#22d3ee").expect("valid second palette color"),
        ])
        .expect("valid ordinal palette");
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::State, palette),
            ))
            .expect("compile ordinal State theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\n[*] --> Ready\nReady --> [*]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("State source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin themed session"),
        )
        .expect("best-effort State preparation")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render State SVG");

        let key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::State,
        };
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Verified
        );
        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .contains(&key)
        );
        assert!(
            !rendered
                .style_report()
                .theme_residuals()
                .iter()
                .any(|residual| residual.key() == &key)
        );
        assert!(rendered.svg().contains("#0f172a"));
    }

    #[test]
    fn state_debug_visibility_filters_downgrade_applied_theme_evidence() {
        let node_theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::State,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ef4444").expect("valid State fill")),
                    )),
                ),
            )
            .expect("compile State node theme");
        let node_parsed = node_theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\nReady --> Done\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("State source should produce a render model");
        let mut node_debug = SvgDebugOptions::default();
        node_debug.include_nodes = false;
        let node_rendered = prepare(
            node_parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&node_theme)
                .expect("begin themed session"),
        )
        .expect("prepare State node theme")
        .render_svg(&SvgRenderOptions::default(), &node_debug)
        .expect("render filtered State SVG");

        let node_key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::State,
        };
        assert!(!node_rendered.svg().contains("#ef4444"));
        assert_eq!(
            node_rendered.style_report().verification(),
            FamilyStyleVerification::Unverified
        );
        assert!(
            !node_rendered
                .style_report()
                .theme_applied_mechanisms()
                .contains(&node_key)
        );
        assert!(
            node_rendered
                .style_report()
                .theme_residuals()
                .iter()
                .any(|residual| {
                    residual.key() == &node_key
                        && residual.reason() == FamilyThemeResidualReason::OutputVisibilityFiltered
                })
        );

        let edge_theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Transition,
                        ThemeStylePatch::default().with_stroke(
                            CanvasPaint::solid("#ec4899").expect("valid transition stroke"),
                        ),
                    ),
                )),
            )
            .expect("compile State transition theme");
        let edge_parsed = edge_theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\nReady --> Done\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("State source should produce a render model");
        let mut edge_debug = SvgDebugOptions::default();
        edge_debug.include_edges = false;
        let edge_rendered = prepare(
            edge_parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&edge_theme)
                .expect("begin themed session"),
        )
        .expect("prepare State transition theme")
        .render_svg(&SvgRenderOptions::default(), &edge_debug)
        .expect("render filtered State SVG");

        let edge_key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Transition,
        };
        assert!(!edge_rendered.svg().contains("#ec4899"));
        assert!(
            edge_rendered
                .style_report()
                .theme_residuals()
                .iter()
                .any(|residual| {
                    residual.key() == &edge_key
                        && residual.reason() == FamilyThemeResidualReason::OutputVisibilityFiltered
                })
        );
    }

    #[test]
    fn require_portable_rechecks_state_theme_after_debug_visibility_filter() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::State,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#ef4444").expect("valid State fill")),
                    )),
                ),
            )
            .expect("compile strict State theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\nReady --> Done\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("State source should produce a render model");
        let artifact = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .expect("begin strict themed session"),
        )
        .expect("complete strict State preparation before output filtering");
        let mut debug = SvgDebugOptions::default();
        debug.include_nodes = false;

        let error = match artifact.render_svg(&SvgRenderOptions::default(), &debug) {
            Ok(_) => panic!("filtered State output must not retain portable family evidence"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            Error::UnverifiedFamilyTheme {
                family_kind: RenderFamilyKind::State,
                residual_count: 1,
            }
        ));
    }

    #[test]
    fn state_family_typography_is_shared_by_layout_and_terminal_svg() {
        let typography = ThemeTextStyle::default()
            .with_font_size_px(26.0)
            .expect("valid State font size");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_typography(TypographySpec::default().with_default(typography)),
            )
            .expect("compile State typography theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "---\ntitle: Architecture\n---\nstateDiagram-v2\n[*] --> Ready\nReady --> Done\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("State source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin themed session"),
        )
        .expect("State typography preparation")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render State SVG");

        assert!(
            rendered
                .style_report()
                .theme_applied_mechanisms()
                .contains(&FamilyThemeMechanismKey::Typography)
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
        assert!(rendered.svg().contains("font-size:26px"));
        assert!(
            rendered.svg().contains("font-size:26px !important"),
            "node/title inline emission must match the measured typography: {}",
            rendered.svg()
        );
        assert!(
            rendered
                .svg()
                .contains("statediagramTitleText{text-anchor:middle;font-family:"),
            "title CSS must use the computed State text style: {}",
            rendered.svg()
        );
    }

    #[test]
    fn state_spacing_without_prepared_text_is_suppressed_and_reported() {
        let typography = ThemeTextStyle::default()
            .with_letter_spacing_px(10.0)
            .expect("valid State letter spacing")
            .with_word_spacing_px(6.0)
            .expect("valid State word spacing");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_typography(TypographySpec::default().with_default(typography)),
            )
            .expect("compile State spacing theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\nReady --> Done\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("State source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin themed session without a custom font catalog"),
        )
        .expect("best-effort State spacing preparation")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render State SVG");

        assert!(!rendered.svg().contains("letter-spacing"));
        assert!(!rendered.svg().contains("word-spacing"));
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Unverified
        );
        assert!(
            rendered
                .style_report()
                .theme_residuals()
                .iter()
                .any(|residual| {
                    residual.key() == &FamilyThemeMechanismKey::Typography
                        && residual.reason() == FamilyThemeResidualReason::UnsupportedTypography
                })
        );
    }

    #[test]
    fn state_rule_without_a_matching_document_target_is_not_applicable() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::Note,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#fef3c7").expect("valid note fill")),
                    )),
                ),
            )
            .expect("compile Note theme");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\n[*] --> Ready\nReady --> [*]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("State source should produce a render model");
        let rendered = prepare(
            parsed,
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin themed session"),
        )
        .expect("best-effort State preparation")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render State SVG");

        let key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Note,
        };
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Verified
        );
        assert!(
            rendered
                .style_report()
                .theme_not_applicable_mechanisms()
                .contains(&key)
        );
        assert!(
            !rendered
                .style_report()
                .theme_applied_mechanisms()
                .contains(&key)
        );
        assert!(rendered.style_report().theme_residuals().is_empty());
    }

    #[test]
    fn require_portable_rejects_state_semantic_residual_before_layout() {
        let gradient = LinearGradient::new(
            90.0,
            [
                GradientStop::new(
                    0.0,
                    ThemeColorValue::parse("#0f172a").expect("valid first stop"),
                )
                .expect("valid first stop"),
                GradientStop::new(
                    1.0,
                    ThemeColorValue::parse("#22d3ee").expect("valid second stop"),
                )
                .expect("valid second stop"),
            ],
        )
        .expect("valid gradient");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::State,
                        ThemeStylePatch::default().with_fill(CanvasPaint::LinearGradient(gradient)),
                    ),
                )),
            )
            .expect("compile gradient theme");
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(
                    "stateDiagram-v2\n[*] --> Ready\nReady --> [*]\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .expect("State source should produce a render model")
        };
        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort State session"),
        )
        .expect("prepare State semantic residual")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort State output should retain semantic residual evidence");
        assert_eq!(
            rendered.style_report().theme_residuals(),
            &[FamilyThemeResidual {
                key: FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::State,
                },
                reason: FamilyThemeResidualReason::UnsupportedPaint,
            }]
        );

        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable render session");

        let error = match prepare(parse(), &LayoutOptions::default(), session) {
            Ok(_) => panic!("strict portability must reject a State semantic residual"),
            Err(error) => error,
        };
        assert_eq!(
            error.unverified_family_theme(),
            Some((
                RenderFamilyKind::State,
                rendered.style_report().theme_residuals().len(),
            ))
        );
    }

    #[test]
    fn require_portable_rejects_state_style_residual_before_layout() {
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new())
            .expect("compile portable theme recipe");
        let parse = || {
            theme
                .install_parse_compatibility(Engine::new())
                .parse_diagram_for_render_model_sync(
                    "stateDiagram-v2\nclassDef broken font-size:not-a-size\n[*] --> Ready:::broken\nReady --> [*]\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .expect("State source should produce a render model")
        };
        let rendered = prepare(
            parse(),
            &LayoutOptions::default(),
            crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("begin best-effort State session"),
        )
        .expect("prepare State source-style residual")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("best-effort State output should retain source-style residual evidence");
        assert_eq!(rendered.style_report().residuals().len(), 1);
        let residual = &rendered.style_report().residuals()[0];
        assert_eq!(residual.owner_id(), "Ready");
        assert_eq!(residual.property(), Some("font-size"));
        assert_eq!(residual.reason(), FamilyStyleResidualReason::InvalidValue);

        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable render session");

        let error = match prepare(parse(), &LayoutOptions::default(), session) {
            Ok(_) => panic!("strict portability must reject an unverified State style"),
            Err(error) => error,
        };
        assert_eq!(
            error.unverified_family_style(),
            Some((
                RenderFamilyKind::State,
                rendered.style_report().residuals().len(),
            ))
        );
    }

    #[test]
    fn parsed_theme_binding_must_match_render_session_theme() {
        let compiler = DiagramThemeCompiler::new();
        let parsed_theme = compiler
            .compile_preset(crate::diagram_theme::ThemePreset::EditorDark)
            .expect("parse theme should compile");
        let render_theme = compiler
            .compile_preset(crate::diagram_theme::ThemePreset::EditorLight)
            .expect("render theme should compile");
        let parsed = parsed_theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\n[*] --> Ready\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("State source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&render_theme)
            .expect("render session should start");

        let error = match prepare(parsed, &LayoutOptions::default(), session) {
            Ok(_) => panic!("a parsed artifact must not cross theme sessions"),
            Err(error) => error,
        };
        assert!(matches!(error, Error::ThemeParseBindingMismatch));
    }

    #[test]
    fn recipe_identity_cannot_be_paired_with_a_different_compatibility_config() {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(crate::diagram_theme::ThemePreset::EditorDark)
            .expect("theme should compile");
        for compatibility in [
            merman_core::MermaidConfig::empty_object(),
            merman_core::MermaidConfig::from_value(json!({
                "theme": "base",
                "darkMode": false,
                "themeVariables": {"darkMode": false}
            })),
        ] {
            let plan = ThemeCompatibilityPlan::try_new(
                *theme.recipe_fingerprint().as_bytes(),
                compatibility,
                |_family, _control| Ok(None),
            )
            .expect("bounded compatibility config");
            let parsed = install_theme_compatibility(Engine::new(), &plan)
                .parse_diagram_for_render_model_sync(
                    "stateDiagram-v2\n[*] --> Ready\n",
                    ParseOptions::strict(),
                )
                .unwrap()
                .expect("State source should produce a render model");
            let session = crate::environment::RenderEnvironment::deterministic()
                .begin_session_with_theme(&theme)
                .expect("render session should start");

            let error = match prepare(parsed, &LayoutOptions::default(), session) {
                Ok(_) => panic!("compatibility config is part of the parse binding"),
                Err(error) => error,
            };
            assert!(matches!(error, Error::ThemeParseBindingMismatch));
        }
    }

    #[test]
    fn themed_and_unthemed_parse_session_transitions_are_fail_closed() {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(crate::diagram_theme::ThemePreset::EditorDark)
            .expect("theme should compile");

        let themed_parse = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\n[*] --> Ready\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("themed State source should produce a render model");
        let unthemed_session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("unthemed render session should start");
        assert!(matches!(
            prepare(themed_parse, &LayoutOptions::default(), unthemed_session),
            Err(Error::ThemeParseBindingMismatch)
        ));

        let unthemed_parse = Engine::new()
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\n[*] --> Ready\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("unthemed State source should produce a render model");
        let themed_session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("themed render session should start");
        assert!(matches!(
            prepare(unthemed_parse, &LayoutOptions::default(), themed_session),
            Err(Error::ThemeParseBindingMismatch)
        ));

        let unthemed_parse = Engine::new()
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\n[*] --> Ready\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("unthemed State source should produce a render model");
        let unthemed_session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("unthemed render session should start");
        prepare(unthemed_parse, &LayoutOptions::default(), unthemed_session)
            .expect("unthemed parses remain valid in unthemed sessions");
    }

    #[test]
    fn equivalent_theme_instances_retain_selected_family_typed_evidence() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default()
                        .with_fill(CanvasPaint::solid("#ef4444").expect("valid node fill")),
                )
                .for_family(RenderFamilyKind::Flowchart),
            ),
        );
        let compiler = DiagramThemeCompiler::new();
        let parse_theme = compiler
            .compile(spec.clone())
            .expect("parse theme should compile");
        let render_theme = compiler
            .compile(spec)
            .expect("equivalent render theme should compile");
        assert_eq!(
            parse_theme.recipe_fingerprint(),
            render_theme.recipe_fingerprint()
        );

        let parsed = parse_theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&render_theme)
            .expect("equivalent render session should start");

        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .expect("equivalent theme should retain the typed route")
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("equivalent theme should retain emitted typed evidence");
        assert!(rendered.svg().contains("fill:#ef4444 !important"));
        assert_eq!(
            rendered.style_report().verification(),
            FamilyStyleVerification::Verified
        );
        assert_eq!(
            rendered.style_report().theme_applied_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
    }

    #[test]
    fn planned_family_drives_flowchart_router_before_layout() {
        let engine = Engine::new();
        let options = LayoutOptions::default();

        let flowchart = engine
            .parse_diagram_for_render_model_sync("flowchart TD\nA --> B\n", ParseOptions::strict())
            .unwrap()
            .expect("flowchart source should produce a render model");
        let swimlane = prepare_non_class_render(
            flowchart,
            &options,
            FamilyRenderContext::resolve(session(), RenderFamilyKind::Swimlane),
        )
        .expect("planned Swimlane family should drive layout");
        assert_eq!(swimlane.family_kind(), RenderFamilyKind::Swimlane);

        let configured_swimlane = engine
            .parse_diagram_for_render_model_sync(
                "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA --> B\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("configured Swimlane source should produce a render model");
        let flowchart = prepare_non_class_render(
            configured_swimlane,
            &options,
            FamilyRenderContext::resolve(session(), RenderFamilyKind::Flowchart),
        )
        .expect("planned Flowchart family should drive layout");
        assert_eq!(flowchart.family_kind(), RenderFamilyKind::Flowchart);
    }

    #[test]
    fn flowchart_router_rejects_an_incompatible_planned_family() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync("flowchart TD\nA --> B\n", ParseOptions::strict())
            .unwrap()
            .expect("flowchart source should produce a render model");

        let error = match prepare_non_class_render(
            parsed,
            &LayoutOptions::default(),
            FamilyRenderContext::resolve(session(), RenderFamilyKind::State),
        ) {
            Ok(_) => panic!("State cannot consume a Flowchart semantic model"),
            Err(error) => error,
        };
        let Error::InvalidModel { message } = error else {
            panic!("expected invalid model error")
        };
        assert!(message.contains("state"));
        assert!(message.contains("Flowchart"));
    }

    #[test]
    fn family_artifact_rejects_planned_family_drift() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync("flowchart TD\nA --> B\n", ParseOptions::strict())
            .unwrap()
            .expect("flowchart source should produce a render model");
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let FamilyRenderArtifact {
            metadata, family, ..
        } = artifact;

        let error = match FamilyRenderArtifact::new(
            metadata,
            family,
            FamilyRenderContext::resolve(session(), RenderFamilyKind::State),
        ) {
            Ok(_) => panic!("family drift must be rejected"),
            Err(error) => error,
        };
        let Error::InvalidModel { message } = error else {
            panic!("expected invalid model error")
        };
        assert!(message.contains("state"));
        assert!(message.contains("flowchart"));
    }

    #[test]
    fn unthemed_family_completion_has_no_theme_identity() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync("flowchart TD\nA --> B\n", ParseOptions::strict())
            .unwrap()
            .expect("flowchart source should produce a render model");
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        assert!(artifact.context.resolved_theme().is_none());

        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render unthemed family SVG");
        assert_eq!(rendered.family_kind(), RenderFamilyKind::Flowchart);

        let completion = rendered.into_completion();
        assert_eq!(
            completion.report().family_kind(),
            RenderFamilyKind::Flowchart
        );
        assert_eq!(completion.report().theme_recipe_fingerprint(), None);
        assert_eq!(
            completion
                .report()
                .session_report()
                .theme_recipe_fingerprint(),
            None
        );
    }

    fn text_measurement_call_count(session: &RenderSession) -> u64 {
        session
            .text_measurement_report()
            .entries()
            .iter()
            .map(crate::environment::TextMeasurementSummary::count)
            .sum()
    }

    #[derive(Debug, Clone, Copy)]
    enum SidecarHostOutcome {
        Success,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct SidecarHostRequest {
        ordinal: usize,
        phase: TextMeasurementPhase,
        operation: TextMeasurementOperation,
        result_kind: TextMeasurementResultKind,
        text: String,
        font_size_bits: u64,
        max_width_bits: Option<u64>,
        wrap_mode: WrapMode,
    }

    struct SidecarRecordingHost {
        outcome: SidecarHostOutcome,
        requests: Mutex<Vec<SidecarHostRequest>>,
    }

    impl SidecarRecordingHost {
        fn new(outcome: SidecarHostOutcome) -> Self {
            Self {
                outcome,
                requests: Mutex::new(Vec::new()),
            }
        }

        fn snapshot(&self) -> Vec<SidecarHostRequest> {
            self.requests.lock().expect("host request trace").clone()
        }
    }

    impl HostTextMeasurer for SidecarRecordingHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            let ordinal = {
                let mut requests = self.requests.lock().expect("host request trace");
                let ordinal = requests.len();
                requests.push(SidecarHostRequest {
                    ordinal,
                    phase: request.phase,
                    operation: request.operation,
                    result_kind: request.operation.required_result_kind(),
                    text: request.text.to_string(),
                    font_size_bits: request.style.font_size.to_bits(),
                    max_width_bits: request.max_width.map(f64::to_bits),
                    wrap_mode: request.wrap_mode,
                });
                ordinal
            };

            match self.outcome {
                SidecarHostOutcome::Success => Ok(Some(sidecar_host_measurement(request, ordinal))),
            }
        }
    }

    fn sidecar_host_measurement(
        request: HostTextMeasurementRequest<'_>,
        ordinal: usize,
    ) -> HostTextMeasurement {
        let state_delta = (ordinal % 7) as f64 / 32.0;
        let raw_width = request
            .text
            .lines()
            .map(|line| line.chars().count() as f64 * 8.0)
            .fold(0.0_f64, f64::max)
            + state_delta;
        let max_width = request
            .max_width
            .filter(|width| width.is_finite() && *width > 0.0);
        let line_count = max_width
            .map(|width| (raw_width / width).ceil() as usize)
            .unwrap_or(1)
            .max(request.text.lines().count())
            .max(1)
            .min(request.text.len().saturating_add(1));
        let metrics = TextMetrics {
            width: max_width.map_or(raw_width, |width| raw_width.min(width)),
            height: line_count as f64 * 20.0 + state_delta,
            line_count,
        };

        match request.operation.required_result_kind() {
            TextMeasurementResultKind::Metrics => HostTextMeasurement::Metrics(metrics),
            TextMeasurementResultKind::Length => {
                let length = match request.operation {
                    TextMeasurementOperation::RawBBoxHeight
                    | TextMeasurementOperation::SimpleBBoxHeight
                    | TextMeasurementOperation::TspanBBoxHeight => metrics.height,
                    TextMeasurementOperation::CreateTextBBoxYOffset
                    | TextMeasurementOperation::CreateTextMiddleBBoxYOffset => 0.0,
                    _ => raw_width,
                };
                HostTextMeasurement::Length(length)
            }
            TextMeasurementResultKind::HorizontalExtents => {
                HostTextMeasurement::HorizontalExtents {
                    left: raw_width / 2.0,
                    right: raw_width / 2.0,
                }
            }
            TextMeasurementResultKind::WrappedWithRawWidth => {
                HostTextMeasurement::WrappedWithRawWidth {
                    metrics,
                    raw_width: Some(raw_width),
                }
            }
        }
    }

    #[cfg(feature = "layout-cytoscape")]
    fn prepare_mindmap_with_host_limit(
        max_layout_work_units: Option<usize>,
    ) -> (Result<FamilyRenderArtifact>, Arc<SidecarRecordingHost>) {
        let source = "mindmap\n  Root\n    First child\n    Second child\n";
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse mindmap")
            .expect("detect mindmap");
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.mindmap-cose-budget").expect("profile id"),
            "1",
        )
        .expect("profile identity");
        let host = Arc::new(SidecarRecordingHost::new(SidecarHostOutcome::Success));
        let mut environment = crate::environment::RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                identity,
                host.clone(),
                TextMeasurementPhase::ALL,
            ));
        if let Some(limit) = max_layout_work_units {
            let policy = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(crate::resources::ResourceLimitId::MaxLayoutWorkUnits, limit)
                .expect("layout work limit");
            environment = environment.with_resource_policy(policy);
        }
        let session = environment.begin_session().expect("render session");
        (prepare(parsed, &LayoutOptions::default(), session), host)
    }

    #[test]
    fn public_flowchart_preparation_enables_prepared_svg_label_reuse() {
        let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A -->|control label| B
"#;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse flowchart")
            .expect("detect flowchart");
        let artifact = prepare(parsed, &LayoutOptions::default(), session())
            .expect("prepare public flowchart artifact");
        let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
            panic!("expected Flowchart family artifact");
        };

        assert!(
            flowchart
                .svg_label_sidecar()
                .node_owner("A", false)
                .is_some(),
            "the public Flowchart preparation path must build the label sidecar"
        );
    }

    #[test]
    fn prepared_self_loop_edge_label_keeps_its_semantic_owner_through_family_dispatch() {
        let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A ordinary-edge@-->|ordinary owner sentinel| B
A self-loop-edge@-->|self loop semantic owner keeps wrapped label rows through the logical render id alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron| A
"#;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse flowchart")
            .expect("detect flowchart");
        let artifact = prepare(parsed, &LayoutOptions::default(), session())
            .expect("prepare flowchart family artifact");

        let rendered_svg = {
            let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
                panic!("expected Flowchart family artifact");
            };
            let model = crate::flowchart::FlowchartRenderModelRef::new(
                flowchart.pair().semantic(),
                flowchart.label_sources(),
            );
            let edge = model.edges.get(1).expect("self-loop edge");
            assert_eq!(edge.id, "self-loop-edge");
            let label = model
                .edge_label_for_render(edge)
                .expect("self-loop edge label");
            let owner = flowchart
                .svg_label_sidecar()
                .edge_owner(edge.id.as_str(), false)
                .expect("semantic self-loop owner");
            assert_eq!(owner, crate::flowchart::FlowchartSvgLabelOwner::Edge(1));
            assert_eq!(
                flowchart
                    .svg_label_sidecar()
                    .edge_owner("A-cyclic-special-mid", false),
                None
            );
            assert!(
                flowchart
                    .pair()
                    .layout()
                    .edges
                    .iter()
                    .any(|edge| edge.id == "self-loop-edge")
            );

            let config = crate::flowchart::FlowchartConfigView::new(
                artifact.metadata.effective_config.as_value(),
            );
            let font_family = config.font_family();
            let render_style = config.render_text_style(&font_family, config.render_font_size());
            let edge_width = config.layout_settings().edge_label_wrapping_width;
            let render_measurer = artifact
                .context
                .session
                .text_measurer(crate::environment::TextMeasurementPhase::SvgBBox);
            let calls_before = text_measurement_call_count(&artifact.context.session);
            let plan = crate::flowchart::FlowchartSvgLabelRenderPlan::new(
                Some(flowchart.svg_label_sidecar()),
                Some(owner),
                label,
                &render_measurer,
                &render_style,
                Some(edge_width),
                true,
                crate::flowchart::FlowchartSvgWidthMode::Bbox,
            );
            assert!(matches!(
                &plan,
                crate::flowchart::FlowchartSvgLabelRenderPlan::Prepared { .. }
            ));
            let wrapped = plan.wrapped_lines();
            assert!(matches!(&wrapped, std::borrow::Cow::Borrowed(_)));
            assert!(wrapped.len() >= 2, "{wrapped:?}");
            assert_eq!(
                text_measurement_call_count(&artifact.context.session),
                calls_before,
                "a prepared self-loop label must not invoke the SVG measurer again"
            );
            drop(wrapped);
            drop(plan);

            let hits_before_render = flowchart.svg_label_sidecar().prepared_hit_count(owner);
            let svg = render_family_artifact_svg(
                &artifact,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
            )
            .expect("render self-loop SVG");
            assert!(
                flowchart.svg_label_sidecar().prepared_hit_count(owner) > hits_before_render,
                "the real Flowchart SVG renderer must consume the prepared self-loop label"
            );
            svg
        };

        assert!(!rendered_svg.contains("cyclic-special"), "{}", rendered_svg);
        let document = roxmltree::Document::parse(&rendered_svg).expect("valid self-loop SVG");
        let logical_path = document.descendants().any(|node| {
            node.has_tag_name("path") && node.attribute("data-id") == Some("self-loop-edge")
        });
        assert!(logical_path, "{rendered_svg}");
        let label_group = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g") && node.attribute("data-id") == Some("self-loop-edge")
            })
            .expect("logical self-loop label group");
        let text = label_group
            .descendants()
            .filter_map(|node| node.text().filter(|_| node.is_text()))
            .collect::<String>();
        assert!(text.contains("self loop semantic owner"), "{text:?}");
        let row_count = label_group
            .descendants()
            .filter(|node| {
                node.has_tag_name("tspan")
                    && node.attribute("class").is_some_and(|class| {
                        class
                            .split_ascii_whitespace()
                            .any(|part| part == "text-outer-tspan")
                    })
            })
            .count();
        assert!(row_count >= 2, "rows={row_count}: {rendered_svg}");
    }

    #[test]
    fn prepared_swimlane_edge_label_is_consumed_by_the_real_svg_renderer() {
        let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
    wrappingWidth: 96
---
swimlane-beta LR
A styled@-->|swimlane semantic owner keeps wrapped label rows through the generated labelRect| B
linkStyle default font-size:24px,font-weight:bold
linkStyle 0 font-size:12px,font-style:italic
"#;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse Swimlane")
            .expect("detect Swimlane");
        let artifact = prepare(parsed, &LayoutOptions::default(), session())
            .expect("prepare Swimlane family artifact");

        let rendered_svg = {
            let BuiltinFamilyArtifact::Swimlane(swimlane) = &artifact.family else {
                panic!("expected Swimlane family artifact");
            };
            let model = crate::flowchart::FlowchartRenderModelRef::new(
                swimlane.pair().semantic(),
                swimlane.label_sources(),
            );
            let edge = model.edges.first().expect("styled Swimlane edge");
            assert_eq!(edge.id, "styled");
            let label = model
                .edge_label_for_render(edge)
                .expect("Swimlane edge label");
            let owner = swimlane
                .svg_label_sidecar()
                .edge_owner(edge.id.as_str(), true)
                .expect("semantic Swimlane edge owner");
            assert_eq!(
                owner,
                crate::flowchart::FlowchartSvgLabelOwner::SwimlaneEdgeLabel(0)
            );
            assert_eq!(
                swimlane.pair().layout().edges[0].label_node_id.as_deref(),
                Some("edge-label-A-B-styled")
            );

            let config = crate::flowchart::FlowchartConfigView::new(
                artifact.metadata.effective_config.as_value(),
            );
            let font_family = config.font_family();
            let base_style = config.render_text_style(&font_family, config.render_font_size());
            let default_edge_styles = model
                .edge_defaults
                .as_ref()
                .map_or(&[][..], |defaults| defaults.style.as_slice());
            let label_style = crate::flowchart::flowchart_swimlane_label_rect_text_style(
                &base_style,
                default_edge_styles,
                &edge.style,
            );
            let render_measurer = artifact
                .context
                .session
                .text_measurer(crate::environment::TextMeasurementPhase::SvgBBox);
            let plan = crate::flowchart::FlowchartSvgLabelRenderPlan::new(
                Some(swimlane.svg_label_sidecar()),
                Some(owner),
                label,
                &render_measurer,
                label_style.as_ref(),
                Some(config.render_wrapping_width()),
                true,
                crate::flowchart::FlowchartSvgWidthMode::Bbox,
            );
            assert!(matches!(
                &plan,
                crate::flowchart::FlowchartSvgLabelRenderPlan::Prepared { .. }
            ));
            let wrapped = plan.wrapped_lines();
            assert!(matches!(&wrapped, std::borrow::Cow::Borrowed(_)));
            assert!(wrapped.len() >= 2, "{wrapped:?}");
            drop(wrapped);
            drop(plan);

            let hits_before_render = swimlane.svg_label_sidecar().prepared_hit_count(owner);
            let svg = render_family_artifact_svg(
                &artifact,
                &SvgRenderOptions::default(),
                &SvgDebugOptions::default(),
            )
            .expect("render Swimlane SVG");
            assert!(
                swimlane.svg_label_sidecar().prepared_hit_count(owner) > hits_before_render,
                "the real Swimlane SVG renderer must consume the prepared labelRect"
            );
            svg
        };

        let document = roxmltree::Document::parse(&rendered_svg).expect("valid Swimlane SVG");
        let label_group = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g") && node.attribute("id") == Some("edge-label-A-B-styled")
            })
            .expect("generated labelRect group");
        let visible = label_group
            .descendants()
            .filter_map(|node| node.text().filter(|_| node.is_text()))
            .flat_map(str::chars)
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>();
        assert_eq!(
            visible,
            "swimlanesemanticownerkeepswrappedlabelrowsthroughthegeneratedlabelRect"
        );
    }

    fn prepare_with_model_item_limit(
        source: &str,
        max_model_items: usize,
    ) -> Result<FamilyRenderArtifact> {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(
                        crate::resources::ResourceLimitId::MaxModelItems,
                        max_model_items,
                    )
                    .unwrap(),
            )
            .begin_session()
            .unwrap();
        prepare(parsed, &LayoutOptions::default(), session)
    }

    fn prepare_with_layout_work_limit(
        source: &str,
        max_layout_work_units: usize,
    ) -> Result<FamilyRenderArtifact> {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(
                        crate::resources::ResourceLimitId::MaxLayoutWorkUnits,
                        max_layout_work_units,
                    )
                    .unwrap(),
            )
            .begin_session()
            .unwrap();
        prepare(parsed, &LayoutOptions::default(), session)
    }

    fn prepare_with_prepared_text_retained_limit(
        source: &str,
        max_prepared_text_retained_bytes: Option<usize>,
    ) -> Result<FamilyRenderArtifact> {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        let typography = ThemeTextStyle::default()
            .with_font_stack(crate::diagram_theme::FontStack::single("Excalifont").unwrap());
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_typography(TypographySpec::default().with_default(typography))
                    .with_assets(
                        crate::diagram_theme::ThemeAssets::default().with_font_catalog(
                            crate::diagram_theme::FontCatalogSpec::new([
                                crate::diagram_theme::FontAssetSpec::new("excalifont", bytes),
                            ]),
                        ),
                    ),
            )
            .expect("prepared-text budget fixture theme should compile");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("source should produce a render model");
        let mut policy = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input();
        if let Some(maximum) = max_prepared_text_retained_bytes {
            policy = policy
                .with_limit(
                    crate::resources::ResourceLimitId::MaxPreparedTextRetainedBytes,
                    maximum,
                )
                .unwrap();
        }
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session_with_theme(&theme)
            .unwrap();
        prepare(parsed, &LayoutOptions::default(), session)
    }

    fn prepare_with_unbounded_layout_work(source: &str) -> Result<FamilyRenderArtifact> {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            )
            .begin_session()
            .unwrap();
        prepare(parsed, &LayoutOptions::default(), session)
    }

    fn assert_model_item_limit(error: Error, actual: usize, max: usize) {
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected max_model_items resource limit error")
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_model_items");
        assert_eq!(limit.actual, actual);
        assert_eq!(limit.max, max);
    }

    #[test]
    fn session_report_accounts_for_every_metered_layout_family() {
        let cases = vec![
            (
                "classDiagram\nclass A\nclass B\nA --> B\n",
                RenderFamilyKind::Class,
            ),
            (
                "stateDiagram-v2\n[*] --> Idle\nIdle --> Active\n",
                RenderFamilyKind::State,
            ),
            (
                "erDiagram\nCUSTOMER ||--o{ ORDER : places\n",
                RenderFamilyKind::Er,
            ),
            (
                "---\nconfig:\n  layout: tidy-tree\n---\nmindmap\n  Root\n    First child\n    Second child\n",
                RenderFamilyKind::Mindmap,
            ),
            (
                "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: hello\n",
                RenderFamilyKind::Sequence,
            ),
            (
                "kanban\n  todo[Todo]\n    task[Task]\n",
                RenderFamilyKind::Kanban,
            ),
            (
                "requirementDiagram\nrequirement req1 {\n  id: 1\n  text: Login\n  risk: high\n}\n",
                RenderFamilyKind::Requirement,
            ),
            ("sankey-beta\nA,B,10\n", RenderFamilyKind::Sankey),
            (
                "radar-beta\naxis A,B,C\ncurve score{1,2,3}\n",
                RenderFamilyKind::Radar,
            ),
            (
                "venn-beta\nset A[\"Core\"]:20\nset B[\"Editor\"]:14\nunion A,B[\"Shared\"]:4\n",
                RenderFamilyKind::Venn,
            ),
        ];

        #[cfg(feature = "layout-cytoscape")]
        let cases = {
            let mut cases = cases;
            cases.push((
                "mindmap\n  Root\n    First child\n    Second child\n",
                RenderFamilyKind::Mindmap,
            ));
            cases
        };

        for (source, expected_family) in cases {
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(source, ParseOptions::default())
                .unwrap()
                .expect("the layout-work fixture should produce a render model");
            let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();

            assert_eq!(artifact.family_kind(), expected_family);
            assert!(
                artifact.context.session().report().layout_work_units() > 0,
                "{expected_family} must contribute layout work to the session report"
            );
        }
    }

    #[test]
    fn prepared_text_retained_budget_has_an_exact_family_boundary() {
        let cases = [
            ("stateDiagram-v2\nReady --> Done\n", RenderFamilyKind::State),
            (
                "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart LR\nA -->|portable label| B\n",
                RenderFamilyKind::Flowchart,
            ),
        ];

        for (source, expected_family) in cases {
            let unbounded = prepare_with_prepared_text_retained_limit(source, None).unwrap();
            assert_eq!(unbounded.family_kind(), expected_family);
            let exact = unbounded
                .context
                .session
                .report()
                .prepared_text_retained_bytes_peak();
            assert!(exact > 1, "{expected_family} must retain prepared text");
            drop(unbounded);

            let bounded = prepare_with_prepared_text_retained_limit(source, Some(exact)).unwrap();
            assert_eq!(
                bounded
                    .context
                    .session
                    .report()
                    .prepared_text_retained_bytes_peak(),
                exact
            );
            drop(bounded);

            let error = match prepare_with_prepared_text_retained_limit(source, Some(exact - 1)) {
                Ok(_) => panic!("{expected_family} exact minus one unexpectedly succeeded"),
                Err(error) => error,
            };
            let Error::ResourceLimitExceeded(limit) = error else {
                panic!("expected {expected_family} prepared-text retained-byte rejection")
            };
            assert_eq!(limit.limit, "max_prepared_text_retained_bytes");
            assert_eq!(limit.max, exact - 1);
            assert_eq!(limit.actual, exact);
        }
    }

    #[test]
    fn invalidated_resvg_output_releases_prepared_text_evidence_and_reservations() {
        let artifact = prepare_with_prepared_text_retained_limit(
            "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart LR\nA -->|portable label| B\n",
            None,
        )
        .unwrap();
        let work_meter = Arc::clone(artifact.context.session.work_meter());
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        assert!(work_meter.prepared_text_retained_bytes() > 0);

        let finalized = rendered
            .finalize_resvg(
                &SvgPipeline::resvg_safe()
                    .with_postprocessor(crate::svg::RootBackgroundPostprocessor::new("white")),
            )
            .unwrap();

        assert!(!finalized.svg().prepared_text_evidence_valid());
        assert!(finalized.svg().prepared_text_label_ledger().is_empty());
        assert_eq!(work_meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn sealed_resvg_output_shares_the_prepared_text_reservation_lease() {
        let artifact = prepare_with_prepared_text_retained_limit(
            "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart LR\nA -->|portable label| B\n",
            None,
        )
        .unwrap();
        let work_meter = Arc::clone(artifact.context.session.work_meter());
        let finalized = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap()
            .finalize_resvg(&SvgPipeline::resvg_safe())
            .unwrap();
        assert!(finalized.svg().prepared_text_evidence_valid());
        assert!(!finalized.svg().prepared_text_label_ledger().is_empty());
        assert!(work_meter.prepared_text_retained_bytes() > 0);

        let (output, report) = finalized.into_completion().into_output_and_report();
        let output_clone = output.clone();
        drop(report);
        assert!(work_meter.prepared_text_retained_bytes() > 0);
        drop(output);
        assert!(work_meter.prepared_text_retained_bytes() > 0);
        drop(output_clone);
        assert_eq!(work_meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn dagre_family_work_budgets_are_exact_and_preserve_layout_output() {
        let cases = [
            (
                "classDiagram\nnamespace Outer {\n  class A\n  class B\n}\nA --> B\n",
                RenderFamilyKind::Class,
            ),
            (
                "stateDiagram-v2\nstate Parent {\n  [*] --> Idle\n  Idle --> Active\n}\nParent --> Outside\n",
                RenderFamilyKind::State,
            ),
            (
                "erDiagram\nNODE {\n  string id\n}\nNODE ||--o{ NODE : leads\n",
                RenderFamilyKind::Er,
            ),
        ];

        for (source, expected_family) in cases {
            let unbounded = prepare_with_unbounded_layout_work(source).unwrap();
            assert_eq!(unbounded.family_kind(), expected_family);
            let exact = unbounded.context.session.report().layout_work_units();
            assert!(exact > 0, "{expected_family} must report layout work");
            let expected_layout = unbounded.layout_json().unwrap();

            let bounded = prepare_with_layout_work_limit(source, exact).unwrap();
            assert_eq!(bounded.context.session.report().layout_work_units(), exact);
            assert_eq!(bounded.layout_json().unwrap(), expected_layout);

            let error = match prepare_with_layout_work_limit(source, exact - 1) {
                Ok(_) => panic!("{expected_family} exact minus one unexpectedly succeeded"),
                Err(error) => error,
            };
            let Error::ResourceLimitExceeded(limit) = error else {
                panic!("expected {expected_family} layout work rejection")
            };
            assert_eq!(limit.limit, "max_layout_work_units");
            assert_eq!(limit.max, exact - 1);
        }
    }

    #[cfg(feature = "layout-cytoscape")]
    #[test]
    fn default_mindmap_cose_reports_kernel_work_and_has_an_exact_resource_boundary() {
        let (unbounded_result, unbounded_host) = prepare_mindmap_with_host_limit(None);
        let unbounded = unbounded_result.expect("unbounded COSE mindmap");
        let exact = unbounded.context.session.report().layout_work_units();
        assert!(
            exact > 78,
            "kernel work must exceed the 3-node adapter estimate"
        );
        let unbounded_layout = unbounded.layout_json().expect("unbounded layout json");
        let unbounded_trace = unbounded_host.snapshot();
        assert!(!unbounded_trace.is_empty());

        let (exact_result, exact_host) = prepare_mindmap_with_host_limit(Some(exact));
        let exact_artifact = exact_result.expect("exact COSE budget");
        assert_eq!(
            exact_artifact.context.session.report().layout_work_units(),
            exact
        );
        assert_eq!(
            exact_artifact.layout_json().expect("exact layout json"),
            unbounded_layout
        );
        assert_eq!(exact_host.snapshot(), unbounded_trace);

        let (short_result, _short_host) = prepare_mindmap_with_host_limit(Some(exact - 1));
        let error = match short_result {
            Ok(_) => panic!("exact minus one must reject COSE work"),
            Err(error) => error,
        };
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected max_layout_work_units rejection")
        };
        assert_eq!(limit.limit, "max_layout_work_units");
        assert_eq!(limit.max, exact - 1);

        let (early_result, early_host) = prepare_mindmap_with_host_limit(Some(1));
        assert!(matches!(early_result, Err(Error::ResourceLimitExceeded(_))));
        assert!(
            early_host.snapshot().is_empty(),
            "adapter admission must reject before the first host measurement"
        );
    }

    #[test]
    fn requirement_layout_projection_excludes_operation_prepared_labels() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"requirementDiagram
requirement req1 {
  id: 1
  text: User logs in
  risk: high
}
element system {
  type: service
}
system - satisfies -> req1
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Requirement source should produce a render model");
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();
        let projection = artifact.layout_json().unwrap();
        let layout = &projection["layout"]["RequirementDiagram"];
        let fields = layout
            .as_object()
            .expect("Requirement layout projection should remain an object");

        assert_eq!(artifact.family_kind(), RenderFamilyKind::Requirement);
        assert!(fields.contains_key("nodes"));
        assert!(fields.contains_key("edges"));
        assert!(fields.contains_key("bounds"));
        assert!(!fields.contains_key("labels"));
        assert!(!layout.to_string().contains("display_text"));
        let serialized_projection = projection.to_string();
        assert!(!serialized_projection.contains("max_width_px"));
        assert!(!serialized_projection.contains("keep_centered"));
        assert!(!serialized_projection.contains("divider_y_offset"));
        assert!(
            serde_json::from_value::<RequirementDiagramLayout>(layout.clone()).is_ok(),
            "prepared labels must not alter the public Requirement layout schema"
        );
    }

    #[test]
    fn flowchart_family_renderer_reuses_prepared_labels_by_semantic_owner() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
    wrappingWidth: 96
---
flowchart LR
subgraph S[Service title]
  A[Node label]
end
subgraph E[Empty title]
end
A labeled@-->|edge semantic owner wraps alpha beta gamma delta epsilon| B[Second node]
"#,
                ParseOptions::strict(),
            )
            .expect("parse Flowchart")
            .expect("detect Flowchart");
        let artifact = prepare(parsed, &LayoutOptions::default(), session())
            .expect("prepare Flowchart family artifact");

        let owners = {
            let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
                panic!("expected Flowchart family artifact");
            };
            let model = crate::flowchart::FlowchartRenderModelRef::new(
                flowchart.pair().semantic(),
                flowchart.label_sources(),
            );
            let node_index = model
                .nodes
                .iter()
                .position(|node| node.id == "A")
                .expect("semantic node A");
            let empty_subgraph_index = model
                .subgraphs
                .iter()
                .position(|subgraph| subgraph.id == "E")
                .expect("semantic empty subgraph E");
            let cluster_index = model
                .subgraphs
                .iter()
                .position(|subgraph| subgraph.id == "S")
                .expect("semantic cluster S");
            let edge_index = model
                .edges
                .iter()
                .position(|edge| edge.id == "labeled")
                .expect("semantic labeled edge");
            let sidecar = flowchart.svg_label_sidecar();

            let owners = [
                sidecar.node_owner("A", false).expect("node owner"),
                sidecar
                    .node_owner("E", false)
                    .expect("empty subgraph owner"),
                sidecar.edge_owner("labeled", false).expect("edge owner"),
                sidecar
                    .subgraph_title_owner("S")
                    .expect("cluster title owner"),
            ];
            assert_eq!(
                owners,
                [
                    crate::flowchart::FlowchartSvgLabelOwner::Node(node_index),
                    crate::flowchart::FlowchartSvgLabelOwner::EmptySubgraphNode(
                        empty_subgraph_index,
                    ),
                    crate::flowchart::FlowchartSvgLabelOwner::Edge(edge_index),
                    crate::flowchart::FlowchartSvgLabelOwner::SubgraphTitle(cluster_index),
                ]
            );
            owners
        };

        let (prepared_hits_before, source_plans_before) = {
            let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
                unreachable!();
            };
            let sidecar = flowchart.svg_label_sidecar();
            (
                owners.map(|owner| sidecar.prepared_hit_count(owner)),
                owners.map(|owner| sidecar.source_plan_count(owner)),
            )
        };

        let svg = render_family_artifact_svg(
            &artifact,
            &SvgRenderOptions::default(),
            &SvgDebugOptions::default(),
        )
        .expect("render Flowchart SVG");

        let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
            unreachable!();
        };
        let sidecar = flowchart.svg_label_sidecar();
        for (index, owner) in owners.into_iter().enumerate() {
            let prepared_hits = sidecar.prepared_hit_count(owner);
            let source_plans = sidecar.source_plan_count(owner);
            assert!(
                prepared_hits > prepared_hits_before[index],
                "real Flowchart SVG emission must consume {owner:?}; prepared_hits={prepared_hits}, source_plans={source_plans}"
            );
            assert_eq!(
                source_plans, source_plans_before[index],
                "eligible owner {owner:?} must not fall back to render-time preparation"
            );
        }
        let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
        let visible_text = document
            .descendants()
            .filter_map(|node| node.text().filter(|_| node.is_text()))
            .flat_map(str::chars)
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        assert!(visible_text.contains("Servicetitle"), "{visible_text}");
        assert!(visible_text.contains("Emptytitle"), "{visible_text}");
        assert!(visible_text.contains("edgesemanticowner"), "{visible_text}");
    }

    #[test]
    fn custom_catalog_swimlane_group_title_fails_closed_until_markdown_is_prepared() {
        let bytes = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        let typography = ThemeTextStyle::default()
            .with_font_stack(crate::diagram_theme::FontStack::single("Excalifont").unwrap());
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_typography(TypographySpec::default().with_default(typography))
                    .with_assets(
                        crate::diagram_theme::ThemeAssets::default().with_font_catalog(
                            crate::diagram_theme::FontCatalogSpec::new([
                                crate::diagram_theme::FontAssetSpec::new("excalifont", bytes),
                            ]),
                        ),
                    ),
            )
            .expect("fixture theme should compile");
        let parsed = theme
            .install_parse_compatibility(Engine::new())
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  layout: swimlane
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
subgraph Lane[Portable lane]
  A[Node]
end
"#,
                ParseOptions::strict(),
            )
            .expect("parse Swimlane")
            .expect("detect Swimlane");
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin themed session");

        assert!(matches!(
            prepare(parsed, &LayoutOptions::default(), session),
            Err(Error::TextLayout(
                crate::text::TextLayoutFailure::UnsupportedLabelMode
            ))
        ));
    }

    #[test]
    fn flowchart_special_shape_intersections_reuse_layout_label_metrics() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
    wrappingWidth: 96
---
flowchart LR
P[plain]
S([stadium label])
H{{hexagon label}}
D@{ shape: doc, label: "document label", labelType: "string" }
P --> S
P --> H
P --> D
S --> P
H --> P
D --> P
"#,
                ParseOptions::strict(),
            )
            .expect("parse Flowchart")
            .expect("detect Flowchart");
        let artifact = prepare(parsed, &LayoutOptions::default(), session())
            .expect("prepare Flowchart family artifact");
        let operation_count =
            |artifact: &FamilyRenderArtifact,
             operation: crate::environment::TextMeasurementOperation| {
                artifact
                    .context
                    .session()
                    .text_measurement_report()
                    .entries()
                    .iter()
                    .filter(|entry| entry.provenance().operation == operation)
                    .map(|entry| entry.count())
                    .sum::<u64>()
            };
        let operation_counts = |artifact: &FamilyRenderArtifact| {
            [
                operation_count(
                    artifact,
                    crate::environment::TextMeasurementOperation::Wrapped,
                ),
                operation_count(
                    artifact,
                    crate::environment::TextMeasurementOperation::ComputedLength,
                ),
            ]
        };
        let operations_before = operation_counts(&artifact);
        let (owners, source_plans_before) = {
            let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
                panic!("expected Flowchart family artifact");
            };
            let node_ids = flowchart
                .pair()
                .semantic()
                .nodes
                .iter()
                .map(|node| node.id.as_str())
                .collect::<Vec<_>>();
            let sidecar = flowchart.svg_label_sidecar();
            let owners = ["S", "H", "D"].map(|id| {
                sidecar.node_owner(id, false).unwrap_or_else(|| {
                    panic!("missing semantic label owner for {id}; nodes={node_ids:?}")
                })
            });
            (owners, owners.map(|owner| sidecar.source_plan_count(owner)))
        };

        let svg = render_family_artifact_svg(
            &artifact,
            &SvgRenderOptions::default(),
            &SvgDebugOptions::default(),
        )
        .expect("render Flowchart SVG");

        assert_eq!(
            operation_counts(&artifact),
            operations_before,
            "special-shape edge intersections must consume layout label metrics"
        );
        let BuiltinFamilyArtifact::Flowchart(flowchart) = &artifact.family else {
            unreachable!();
        };
        for (index, owner) in owners.into_iter().enumerate() {
            assert_eq!(
                flowchart.svg_label_sidecar().source_plan_count(owner),
                source_plans_before[index],
                "special-shape label {owner:?} must reuse its prepared SVG plan"
            );
        }
        let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
        let visible_text = document
            .descendants()
            .filter_map(|node| node.text().filter(|_| node.is_text()))
            .flat_map(str::chars)
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        assert!(visible_text.contains("stadiumlabel"), "{visible_text}");
        assert!(visible_text.contains("hexagonlabel"), "{visible_text}");
        assert!(visible_text.contains("documentlabel"), "{visible_text}");
    }

    #[test]
    fn swimlane_family_renderer_reuses_the_original_semantic_edge_owner() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
    wrappingWidth: 96
---
swimlane-beta LR
A --> C
A styled@-->|swimlane semantic owner wraps alpha beta gamma delta epsilon| B
"#,
                ParseOptions::strict(),
            )
            .expect("parse Swimlane")
            .expect("detect Swimlane");
        let artifact = prepare(parsed, &LayoutOptions::default(), session())
            .expect("prepare Swimlane family artifact");

        let (owner, hits_before, source_plans_before) = {
            let BuiltinFamilyArtifact::Swimlane(swimlane) = &artifact.family else {
                panic!("expected Swimlane family artifact");
            };
            let model = crate::flowchart::FlowchartRenderModelRef::new(
                swimlane.pair().semantic(),
                swimlane.label_sources(),
            );
            let edge_index = model
                .edges
                .iter()
                .position(|edge| edge.id == "styled")
                .expect("semantic styled edge");
            let owner = swimlane
                .svg_label_sidecar()
                .edge_owner("styled", true)
                .expect("Swimlane edge-label owner");
            assert_eq!(
                owner,
                crate::flowchart::FlowchartSvgLabelOwner::SwimlaneEdgeLabel(edge_index)
            );
            assert_eq!(
                swimlane
                    .pair()
                    .layout()
                    .edges
                    .iter()
                    .find(|edge| edge.id == "styled")
                    .and_then(|edge| edge.label_node_id.as_deref()),
                Some("edge-label-A-B-styled")
            );
            (
                owner,
                swimlane.svg_label_sidecar().prepared_hit_count(owner),
                swimlane.svg_label_sidecar().source_plan_count(owner),
            )
        };

        let svg = render_family_artifact_svg(
            &artifact,
            &SvgRenderOptions::default(),
            &SvgDebugOptions::default(),
        )
        .expect("render Swimlane SVG");

        let BuiltinFamilyArtifact::Swimlane(swimlane) = &artifact.family else {
            unreachable!();
        };
        assert!(
            swimlane.svg_label_sidecar().prepared_hit_count(owner) > hits_before,
            "real Swimlane SVG emission must consume the prepared labelRect"
        );
        assert_eq!(
            swimlane.svg_label_sidecar().source_plan_count(owner),
            source_plans_before
        );
        let document = roxmltree::Document::parse(&svg).expect("valid Swimlane SVG");
        let visible_text = document
            .descendants()
            .filter_map(|node| node.text().filter(|_| node.is_text()))
            .flat_map(str::chars)
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        assert!(
            visible_text.contains("swimlanesemanticowner"),
            "{visible_text}"
        );
    }

    #[test]
    fn flowchart_family_layout_projections_exclude_operation_prepared_labels() {
        for (source, variant) in [
            ("flowchart LR\nA -->|Flowchart label| B\n", "FlowchartV2"),
            (
                "swimlane-beta LR\nA -->|Swimlane label| B\n",
                "SwimlaneDiagram",
            ),
        ] {
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .expect("parse Flowchart family")
                .expect("detect Flowchart family");
            let artifact = prepare(parsed, &LayoutOptions::default(), session())
                .expect("prepare Flowchart family artifact");
            let projection = artifact.layout_json().expect("project public layout JSON");
            let layout = &projection["layout"][variant];
            let serialized = layout.to_string();

            for internal_field in [
                "wrapped_lines",
                "plain_text",
                "binding",
                "render_ids",
                "svg_label_sidecar",
            ] {
                assert!(
                    !serialized.contains(internal_field),
                    "{variant} leaked operation-local field {internal_field}: {serialized}"
                );
            }

            match variant {
                "FlowchartV2" => assert!(
                    serde_json::from_value::<FlowchartLayout>(layout.clone()).is_ok(),
                    "prepared labels must not alter the public Flowchart layout schema"
                ),
                "SwimlaneDiagram" => assert!(
                    serde_json::from_value::<SwimlaneLayout>(layout.clone()).is_ok(),
                    "prepared labels must not alter the public Swimlane layout schema"
                ),
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn dagre_flowchart_node_limit_accepts_boundary_and_rejects_one_beyond() {
        let source = "flowchart TD\nA --> B";
        let artifact = prepare_with_model_item_limit(source, 3).unwrap();
        assert_eq!(artifact.family_kind(), RenderFamilyKind::Flowchart);

        let error = match prepare_with_model_item_limit(source, 2) {
            Err(error) => error,
            Ok(_) => panic!("flowchart above the node limit unexpectedly rendered"),
        };
        assert_model_item_limit(error, 3, 2);
    }

    #[test]
    fn swimlane_node_limit_accepts_boundary_and_rejects_one_beyond() {
        let source = "swimlane-beta LR\nA --> B";
        let artifact = prepare_with_model_item_limit(source, 3).unwrap();
        assert_eq!(artifact.family_kind(), RenderFamilyKind::Swimlane);

        let error = match prepare_with_model_item_limit(source, 2) {
            Err(error) => error,
            Ok(_) => panic!("swimlane above the node limit unexpectedly rendered"),
        };
        assert_model_item_limit(error, 3, 2);
    }

    #[test]
    fn swimlane_rejects_pairwise_routing_work_before_layout() {
        let source = "swimlane-beta LR\nA --> B\nB --> C";
        let artifact = prepare_with_layout_work_limit(source, 1_000).unwrap();
        assert_eq!(artifact.family_kind(), RenderFamilyKind::Swimlane);

        let error = match prepare_with_layout_work_limit(source, 1) {
            Err(error) => error,
            Ok(_) => panic!("swimlane above the layout work limit unexpectedly rendered"),
        };
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected max_layout_work_units resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_layout_work_units");
        assert!(limit.actual > limit.max);
        assert_eq!(limit.max, 1);
    }

    #[test]
    fn mindmap_node_limit_is_checked_before_layout_allocation_or_backend_dispatch() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "mindmap\n  Root\n    First child\n    Second child\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("mindmap source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(crate::resources::ResourceLimitId::MaxModelItems, 4)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();

        let error = match prepare(parsed, &LayoutOptions::default(), session) {
            Err(error) => error,
            Ok(_) => panic!("mindmap above the node limit unexpectedly reached layout"),
        };
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected max_model_items resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_model_items");
        assert_eq!(limit.actual, 5);
        assert_eq!(limit.max, 4);
    }

    #[test]
    fn flowchart_math_capability_uses_parser_owned_render_spelling() {
        for source in [
            "flowchart TD\nA[\"#36;#36;node#36;#36;\"]\n",
            "flowchart TD\nA -->|#36;#36;edge#36;#36;| B\n",
            "flowchart TD\nsubgraph S[\"#36;#36;group#36;#36;\"]\nA\nend\n",
        ] {
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Flowchart source should produce a render model");
            let session = crate::environment::RenderEnvironment::deterministic()
                .without_math_renderer()
                .begin_session()
                .unwrap();

            let plan = plan_render(&parsed, &session).unwrap();
            assert!(
                !plan
                    .required_capabilities()
                    .contains(&RenderCapability::Math),
                "encoded dollar entities remain ordinary createText input: {source}"
            );
            prepare(parsed, &LayoutOptions::default(), session)
                .expect("encoded dollar entities must not require a Math renderer");
        }

        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart TD\nA[\"$$x$$\"]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .without_math_renderer()
            .begin_session()
            .unwrap();
        let plan = plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.required_capabilities(), &[RenderCapability::Math]);
        assert_eq!(plan.missing_capabilities(), &[RenderCapability::Math]);
    }

    #[test]
    fn mindmap_math_label_requires_the_math_capability() {
        let source = r#"---
config:
  layout: tidy-tree
---
mindmap
  root[Root]
    formula["$$x^2$$"]
"#;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("mindmap source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .without_math_renderer()
            .begin_session()
            .unwrap();

        let plan = plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.required_capabilities(), &[RenderCapability::Math]);
        assert_eq!(plan.missing_capabilities(), &[RenderCapability::Math]);
        assert!(!plan.is_ready());

        let error = match prepare(parsed, &LayoutOptions::default(), session) {
            Err(error) => error,
            Ok(_) => panic!("mindmap math label unexpectedly rendered without a math backend"),
        };
        assert!(matches!(
            error,
            Error::MissingCapability {
                capability: RenderCapability::Math,
                ref diagram_type,
            } if diagram_type == "mindmap"
        ));
    }

    #[derive(Debug)]
    struct MindmapMathRenderer;

    impl crate::math::MathRenderer for MindmapMathRenderer {
        fn render_html_label(
            &self,
            text: &str,
            _config: &merman_core::MermaidConfig,
        ) -> Option<String> {
            text.contains("$$")
                .then(|| "<strong>rendered-mindmap-math</strong>".to_string())
        }

        fn measure_html_label(
            &self,
            text: &str,
            _config: &merman_core::MermaidConfig,
            _style: &crate::text::TextStyle,
            _max_width_px: Option<f64>,
            _wrap_mode: crate::text::WrapMode,
        ) -> Option<crate::text::TextMetrics> {
            text.contains("$$").then_some(crate::text::TextMetrics {
                width: 96.0,
                height: 24.0,
                line_count: 1,
            })
        }
    }

    #[test]
    fn mindmap_math_label_is_consumed_by_the_math_renderer() {
        let source = r#"---
config:
  layout: tidy-tree
---
mindmap
  root[Root]
    formula["$$x^2$$"]
"#;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("mindmap source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_math_renderer(std::sync::Arc::new(MindmapMathRenderer))
            .begin_session()
            .unwrap();

        let plan = plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.required_capabilities(), &[RenderCapability::Math]);
        assert!(plan.missing_capabilities().is_empty());
        assert!(plan.is_ready());

        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        assert!(rendered.svg().contains("rendered-mindmap-math"));
        assert!(!rendered.svg().contains("$$x^2$$"));
    }

    #[test]
    fn class_math_label_requires_the_math_capability() {
        let source = r#"classDiagram
class Formula["$$x^2$$"]
"#;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Class source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .without_math_renderer()
            .begin_session()
            .unwrap();

        let plan = plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.required_capabilities(), &[RenderCapability::Math]);
        assert_eq!(plan.missing_capabilities(), &[RenderCapability::Math]);
        assert!(!plan.is_ready());

        let error = match prepare(parsed, &LayoutOptions::default(), session) {
            Err(error) => error,
            Ok(_) => panic!("Class math label unexpectedly rendered without a math backend"),
        };
        assert!(matches!(
            error,
            Error::MissingCapability {
                capability: RenderCapability::Math,
                ref diagram_type,
            } if diagram_type == "class"
        ));
    }

    #[derive(Debug)]
    struct ClassMathRenderer;

    impl crate::math::MathRenderer for ClassMathRenderer {
        fn render_html_label(
            &self,
            text: &str,
            _config: &merman_core::MermaidConfig,
        ) -> Option<String> {
            text.contains("$$")
                .then(|| "<div>rendered-class-math</div>".to_string())
        }

        fn measure_html_label(
            &self,
            text: &str,
            _config: &merman_core::MermaidConfig,
            _style: &crate::text::TextStyle,
            _max_width_px: Option<f64>,
            _wrap_mode: crate::text::WrapMode,
        ) -> Option<crate::text::TextMetrics> {
            text.contains("$$").then_some(crate::text::TextMetrics {
                width: 96.0,
                height: 24.0,
                line_count: 1,
            })
        }
    }

    #[test]
    fn class_math_label_is_consumed_by_the_math_renderer() {
        let source = r#"classDiagram
class Formula["$$x^2$$"]
"#;
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Class source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_math_renderer(std::sync::Arc::new(ClassMathRenderer))
            .begin_session()
            .unwrap();

        let plan = plan_render(&parsed, &session).unwrap();
        assert_eq!(plan.required_capabilities(), &[RenderCapability::Math]);
        assert!(plan.missing_capabilities().is_empty());
        assert!(plan.is_ready());

        let rendered = prepare(parsed, &LayoutOptions::default(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        assert!(rendered.svg().contains("rendered-class-math"));
        assert!(!rendered.svg().contains("$$x^2$$"));
    }

    fn render_class_math(source: &str) -> String {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .expect("Class source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_math_renderer(std::sync::Arc::new(ClassMathRenderer))
            .begin_session()
            .unwrap();
        prepare(parsed, &LayoutOptions::default(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap()
            .svg()
            .to_string()
    }

    #[test]
    fn class_math_label_forces_html_rendering_when_html_labels_are_disabled() {
        let svg = render_class_math(
            r#"---
config:
  htmlLabels: false
---
classDiagram
class Formula["$$x^2$$"]
"#,
        );

        assert!(svg.contains("rendered-class-math"));
        assert!(!svg.contains("$$x^2$$"));
    }

    #[test]
    fn class_relation_terminal_and_note_math_labels_use_the_math_renderer() {
        let svg = render_class_math(
            r#"classDiagram
class Formula
class Result
Formula "$$one$$" --> "$$many$$" Result : $$edge$$
note for Formula "$$note$$"
"#,
        );

        assert_eq!(svg.matches("rendered-class-math").count(), 4);
        assert!(!svg.contains("$$"));
        assert!(!svg.contains("<p><div>"));
    }

    #[test]
    fn class_annotation_and_interface_math_labels_require_and_use_math() {
        for source in [
            r#"classDiagram
class Formula <<$$annotation$$>>
"#,
            r#"classDiagram
class Formula
$$interface$$ ()-- Formula
"#,
        ] {
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .expect("Class source should produce a render model");
            let session = crate::environment::RenderEnvironment::deterministic()
                .without_math_renderer()
                .begin_session()
                .unwrap();

            let plan = plan_render(&parsed, &session).unwrap();
            assert_eq!(plan.required_capabilities(), &[RenderCapability::Math]);
            assert_eq!(plan.missing_capabilities(), &[RenderCapability::Math]);

            let svg = render_class_math(source);
            assert!(svg.contains("rendered-class-math"));
            assert!(!svg.contains("$$"));
            assert!(!svg.contains("<p><div>"));
        }
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn elk_flowchart_node_limit_accepts_boundary_and_rejects_one_beyond() {
        let source = "flowchart-elk TD\nA --> B";
        let artifact = prepare_with_model_item_limit(source, 3).unwrap();
        assert_eq!(artifact.family_kind(), RenderFamilyKind::Flowchart);

        let error = match prepare_with_model_item_limit(source, 2) {
            Err(error) => error,
            Ok(_) => panic!("ELK flowchart above the node limit unexpectedly rendered"),
        };
        assert_model_item_limit(error, 3, 2);
    }

    #[test]
    fn custom_semantic_json_is_explicitly_non_renderable() {
        let mut engine = Engine::new();
        engine
            .diagram_registry_mut()
            .insert("customDiagram", custom_semantic_parser);
        let parsed = engine
            .parse_diagram_for_render_model_with_type_sync(
                "customDiagram",
                "customDiagram\npayload",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        let error = match prepare(parsed, &LayoutOptions::default(), session()) {
            Err(error) => error,
            Ok(_) => panic!("custom JSON unexpectedly produced a built-in artifact"),
        };
        let Error::NonRenderableCustomModel {
            diagram_type,
            model_name,
            provenance,
        } = error
        else {
            panic!("expected explicit custom-model capability error")
        };
        assert_eq!(diagram_type, "customDiagram");
        assert_eq!(model_name, "customDiagram");
        assert_eq!(provenance, CustomJsonProvenance::SemanticRegistryOverlay);
    }

    #[test]
    fn custom_render_overlay_cannot_masquerade_as_a_builtin_family() {
        let mut engine = Engine::new();
        engine
            .render_diagram_registry_mut()
            .insert("flowchart-v2", custom_render_parser);
        let parsed = engine
            .parse_diagram_for_render_model_with_type_sync(
                "flowchart-v2",
                "flowchart TD\nA --> B",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        let error = match prepare(parsed, &LayoutOptions::default(), session()) {
            Err(error) => error,
            Ok(_) => panic!("custom JSON unexpectedly produced a built-in artifact"),
        };
        let Error::NonRenderableCustomModel {
            diagram_type,
            model_name,
            provenance,
        } = error
        else {
            panic!("expected explicit custom-model capability error")
        };
        assert_eq!(diagram_type, "flowchart-v2");
        assert_eq!(model_name, "custom-flowchart");
        assert_eq!(provenance, CustomJsonProvenance::RenderRegistryOverlay);
    }

    #[test]
    fn gantt_time_axis_diagnostics_invert_rendered_x_without_exposing_layout() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  gantt:
    useWidth: 130
    leftPadding: 10
    rightPadding: 20
---
gantt
dateFormat x
section Delivery
First: first,-1,1ms
Second: second,after first,2ms
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();

        assert_eq!(artifact.family_kind(), RenderFamilyKind::Gantt);
        let diagnostics = artifact
            .gantt_time_axis_diagnostics()
            .expect("Gantt tasks should expose time-axis diagnostics");
        assert_eq!(diagnostics.unix_millis_at_rendered_x(10.0), Some(-1));
        assert_eq!(diagnostics.unix_millis_at_rendered_x(43.0), Some(0));
        assert_eq!(diagnostics.unix_millis_at_rendered_x(77.0), Some(1));
        assert_eq!(diagnostics.unix_millis_at_rendered_x(110.0), Some(2));
        assert_eq!(diagnostics.unix_millis_at_rendered_x(44.0), None);
        assert_eq!(diagnostics.unix_millis_at_rendered_x(f64::NAN), None);

        artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        assert_eq!(diagnostics.unix_millis_at_rendered_x(77.0), Some(1));
    }

    #[test]
    fn suppressed_parse_failure_uses_the_typed_error_artifact_and_renderer() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync("flowchart TD\nA -->", ParseOptions::lenient())
            .unwrap()
            .unwrap();
        let artifact = prepare(parsed, &LayoutOptions::default(), session()).unwrap();

        assert_eq!(artifact.family_kind(), RenderFamilyKind::Error);
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        assert!(rendered.svg().contains("Syntax error in text"));
    }
}

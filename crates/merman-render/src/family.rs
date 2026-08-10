use crate::diagram_theme::{
    FamilyThemeMechanismKey, ResolvedDiagramTheme, RootThemePlan, RootThemeReport,
    SourceStyleChannel, SourceStyleOrigin, SourceStyleResidual, SourceStyleResidualReason,
    ThemePortabilityRequirement, ThemeRecipeFingerprint,
};
use crate::environment::{RenderSession, RenderSessionReport};
use crate::model::*;
use crate::resources::ResourceLimitPhase;
use crate::svg::{
    ResvgCompatibleSvg, SvgDebugOptions, SvgPipeline, SvgPostprocessMetadata, SvgRenderOptions,
};
use crate::wardley::WardleyDiagramLayout;
use crate::{Error, LayoutExecution, LayoutOptions, RenderCapability, Result};
use merman_core::diagrams;
use merman_core::models::class_diagram::ClassDiagram;
use merman_core::{BuiltinRenderSemantic, ParseMetadata, ParsedDiagramRender, RenderSemanticModel};
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
#[non_exhaustive]
pub enum FamilyStyleEvaluation {
    /// The selected theme has no structured styles that apply to this family.
    NotApplicable,
    /// The family adapter evaluated every applicable structured style input.
    Evaluated,
    /// Structured style inputs apply, but this family has no complete adapter yet.
    Unadapted,
}

/// Verification state derived from family evaluation and its retained residuals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FamilyStyleVerification {
    NotApplicable,
    Verified,
    Unverified,
    Unadapted,
    Incomplete,
}

/// Source channel that produced an unverified family style residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FamilyStyleChannel {
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
#[non_exhaustive]
pub enum FamilyStyleOrigin {
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
#[non_exhaustive]
pub enum FamilyThemeResidualReason {
    UnsupportedPaint,
    UnsupportedTypography,
    UnsupportedGeometry,
    UnsupportedEffect,
    UnsupportedOrdinalPalette,
}

impl FamilyThemeResidualReason {
    pub const fn id(self) -> &'static str {
        match self {
            Self::UnsupportedPaint => "unsupported-paint",
            Self::UnsupportedTypography => "unsupported-typography",
            Self::UnsupportedGeometry => "unsupported-geometry",
            Self::UnsupportedEffect => "unsupported-effect",
            Self::UnsupportedOrdinalPalette => "unsupported-ordinal-palette",
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
pub struct FamilyThemeResidual {
    key: FamilyThemeMechanismKey,
    reason: FamilyThemeResidualReason,
}

impl FamilyThemeResidual {
    pub const fn key(&self) -> &FamilyThemeMechanismKey {
        &self.key
    }

    pub const fn reason(&self) -> FamilyThemeResidualReason {
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
#[non_exhaustive]
pub enum FamilyStyleResidualReason {
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
pub struct FamilyStyleResidual {
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
pub struct FamilyStyleReport {
    family_kind: RenderFamilyKind,
    evaluation: FamilyStyleEvaluation,
    theme_required: Vec<FamilyThemeMechanismKey>,
    theme_applied: Vec<FamilyThemeMechanismKey>,
    theme_not_applicable: Vec<FamilyThemeMechanismKey>,
    theme_residuals: Vec<FamilyThemeResidual>,
    residuals: Vec<FamilyStyleResidual>,
}

impl FamilyStyleReport {
    fn freeze(plan: &ResolvedFamilyStylePlan) -> Self {
        let mut residuals = Vec::new();
        let evaluation = match &plan.payload {
            FamilyStylePayload::NotApplicable => FamilyStyleEvaluation::NotApplicable,
            FamilyStylePayload::Unadapted => FamilyStyleEvaluation::Unadapted,
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
        Self {
            family_kind: plan.family_kind,
            evaluation,
            theme_required: plan.theme_evidence.required.clone(),
            theme_applied: plan.theme_evidence.applied.clone(),
            theme_not_applicable: plan.theme_evidence.not_applicable.clone(),
            theme_residuals: plan.theme_evidence.residuals.clone(),
            residuals,
        }
    }

    pub const fn family_kind(&self) -> RenderFamilyKind {
        self.family_kind
    }

    pub const fn evaluation(&self) -> FamilyStyleEvaluation {
        self.evaluation
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

    /// Returns recipe mechanisms evaluated against this document but not selected by any rendered
    /// target, variant, or ordinal.
    pub fn theme_not_applicable_mechanisms(&self) -> &[FamilyThemeMechanismKey] {
        &self.theme_not_applicable
    }

    /// Returns typed semantic mechanisms that remain outside this adapter's proof boundary.
    pub fn theme_residuals(&self) -> &[FamilyThemeResidual] {
        &self.theme_residuals
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
                if let Some(first_residual) = self.theme_residuals.first().cloned() {
                    return Err(Error::UnverifiedFamilyTheme {
                        family_kind: self.family_kind,
                        residual_count: self.theme_residuals.len(),
                        first_residual,
                    });
                }
                let first_residual = self
                    .residuals
                    .first()
                    .cloned()
                    .expect("unverified family source styles retain a residual");
                Err(Error::UnverifiedFamilyStyle {
                    family_kind: self.family_kind,
                    residual_count: self.residuals.len(),
                    first_residual,
                })
            }
        }
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
    prepared_text_ledger: Arc<[crate::text::PreparedTextLabelLedgerEntry]>,
}

impl FamilyRenderReport {
    fn freeze(
        root_theme: RootThemeReport,
        style: FamilyStyleReport,
        session: RenderSession,
        prepared_text_ledger: Arc<[crate::text::PreparedTextLabelLedgerEntry]>,
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

    pub const fn style_report(&self) -> &FamilyStyleReport {
        &self.style
    }

    pub const fn root_theme_report(&self) -> &RootThemeReport {
        &self.root_theme
    }

    /// Returns the frozen family style report carried by this completion.
    pub const fn family_style_report(&self) -> &FamilyStyleReport {
        &self.style
    }

    /// Returns source-style residuals recorded by the family adapter.
    pub fn family_style_residuals(&self) -> &[FamilyStyleResidual] {
        self.style.residuals()
    }

    pub fn theme_recipe_fingerprint(&self) -> Option<ThemeRecipeFingerprint> {
        self.session.theme_recipe_fingerprint()
    }

    pub const fn session_report(&self) -> &RenderSessionReport {
        &self.session
    }

    /// Returns prepared labels actually consumed by the family SVG emitter.
    #[doc(hidden)]
    pub fn prepared_text_label_ledger(&self) -> &[crate::text::PreparedTextLabelLedgerEntry] {
        &self.prepared_text_ledger
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
            not_applicable: Vec::new(),
            residuals: Vec::new(),
        }
    }

    fn not_applicable(theme: Option<&ResolvedDiagramTheme>) -> bool {
        theme.is_none_or(|theme| theme.family_mechanism_keys().is_empty())
    }

    pub(crate) fn mark_applied(&mut self, key: FamilyThemeMechanismKey) {
        if self.required.contains(&key) && !self.is_accounted(&key) {
            self.applied.push(key);
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

    fn is_accounted(&self, key: &FamilyThemeMechanismKey) -> bool {
        self.applied.contains(key)
            || self.not_applicable.contains(key)
            || self.residuals.iter().any(|residual| residual.key == *key)
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
    State(Box<crate::state::StateStylePlan>),
}

#[derive(Debug)]
pub(crate) struct ResolvedFamilyStylePlan {
    family_kind: RenderFamilyKind,
    resolved_theme: Option<Box<ResolvedDiagramTheme>>,
    theme_evidence: FamilyThemeEvidence,
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
            payload,
        }
    }

    fn adapt_state(
        &mut self,
        model: &merman_core::diagrams::state::StateDiagramRenderModel,
        effective_config: &serde_json::Value,
        title: Option<&str>,
    ) -> Result<()> {
        debug_assert_eq!(self.family_kind, RenderFamilyKind::State);
        let (plan, theme_evidence) = crate::state::StateStylePlan::resolve_with_evidence(
            model,
            effective_config,
            self.resolved_theme.as_deref(),
            title,
        );
        self.theme_evidence = theme_evidence;
        self.payload = FamilyStylePayload::State(Box::new(plan));
        Ok(())
    }

    fn ensure_portable(&self, portability: ThemePortabilityRequirement) -> Result<()> {
        if portability != ThemePortabilityRequirement::RequirePortable {
            return Ok(());
        }

        FamilyStyleReport::freeze(self).ensure_portable()
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
            FamilyStylePayload::NotApplicable | FamilyStylePayload::Unadapted => None,
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
        self.style_plan.adapt_state(model, effective_config, title)
    }

    fn ensure_portable(&self) -> Result<()> {
        let portability = self
            .session
            .theme_portability_requirement()
            .unwrap_or(ThemePortabilityRequirement::BestEffort);
        self.style_plan.ensure_portable(portability)
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
}

#[derive(Debug)]
pub(crate) struct StateFamilyArtifact {
    pair: FamilyPair<diagrams::state::StateDiagramRenderModel, StateDiagramLayout>,
    label_sidecar: crate::state::StateLabelSidecar,
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

    fn prepared_text_label_ledger(&self) -> Arc<[crate::text::PreparedTextLabelLedgerEntry]> {
        let entries = match self {
            Self::Flowchart(artifact) => artifact
                .svg_label_sidecar()
                .prepared_text_label_ledger()
                .cloned()
                .collect::<Vec<_>>(),
            Self::Swimlane(artifact) => artifact
                .svg_label_sidecar()
                .prepared_text_label_ledger()
                .cloned()
                .collect::<Vec<_>>(),
            Self::State(artifact) => artifact
                .label_sidecar()
                .prepared_text_label_ledger()
                .cloned()
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        Arc::from(entries)
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
    prepared_text_ledger: Arc<[crate::text::PreparedTextLabelLedgerEntry]>,
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

    pub const fn style_report(&self) -> &FamilyStyleReport {
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
                &self.prepared_text_ledger,
            )?;
            self.svg = public_svg;
            self.prepared_text_svg = prepared_text_svg;
        } else {
            self.svg = processed_svg;
        }
        self.session
            .resource_policy()
            .check_svg_bytes(&self.svg, ResourceLimitPhase::SvgPostprocess)?;
        if !pipeline.preserves_typed_root_theme() {
            self.root_theme = self.root_theme.invalidate_for_output_mutation();
        }
        ensure_root_theme_portable(
            &self.root_theme,
            self.session
                .theme_portability_requirement()
                .unwrap_or(ThemePortabilityRequirement::BestEffort),
        )?;
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
                Arc::clone(&self.prepared_text_ledger),
                prepared_text_evidence_valid,
            )?;
        self.session
            .resource_policy()
            .check_svg_bytes(svg.as_str(), ResourceLimitPhase::SvgPostprocess)?;
        let root_theme = if pipeline.preserves_typed_root_theme() {
            self.root_theme
        } else {
            self.root_theme.invalidate_for_output_mutation()
        };
        ensure_root_theme_portable(&root_theme, portability)?;
        Ok(RenderedResvgCompatibleSvg {
            svg,
            root_theme,
            style_report: self.style_report,
            session: self.session,
            prepared_text_ledger: self.prepared_text_ledger,
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
    prepared_text_ledger: Arc<[crate::text::PreparedTextLabelLedgerEntry]>,
}

impl RenderedResvgCompatibleSvg {
    pub fn svg(&self) -> &ResvgCompatibleSvg {
        &self.svg
    }

    pub const fn family_kind(&self) -> RenderFamilyKind {
        self.style_report.family_kind()
    }

    pub const fn style_report(&self) -> &FamilyStyleReport {
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
        let prepared_text_ledger = self.family.prepared_text_label_ledger();
        let Self {
            metadata,
            compatibility_projection: _,
            family: _,
            context,
        } = self;
        let (tokenized_svg, root_theme) = rendered.into_parts();
        let (svg, prepared_text_svg) =
            crate::svg::partition_prepared_text_label_ids(tokenized_svg, &prepared_text_ledger)?;
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
    layout: impl FnOnce(
        &diagrams::flowchart::FlowchartModel,
        &diagrams::flowchart::FlowchartRenderLabelSources,
        &crate::flowchart::FlowchartSvgLabelSidecarBuilder,
    ) -> Result<L>,
) -> Result<Box<FlowchartFamilyArtifact<L>>> {
    let svg_label_sidecar = crate::flowchart::FlowchartSvgLabelSidecarBuilder::new(
        prepared_text_layout,
        resolved_theme,
    );
    let layout = layout(&semantic, &label_sources, &svg_label_sidecar)?;
    let svg_label_sidecar = svg_label_sidecar.finish();
    if let Some(error) = svg_label_sidecar.prepared_error().cloned() {
        return Err(error.into());
    }
    Ok(Box::new(FlowchartFamilyArtifact {
        pair: FamilyPair::new(semantic, layout),
        label_sources,
        svg_label_sidecar,
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
    context: FamilyRenderContext,
) -> Result<FamilyRenderArtifact> {
    let (meta, model) = parsed.into_parts();
    let RenderSemanticModel::Class(model) = model else {
        unreachable!("Class render dispatch requires a Class semantic model")
    };
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
    if let RenderSemanticModel::State(model) = &model {
        context.adapt_state(model, effective_config, title)?;
    }
    context.ensure_portable()?;
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
            let label_sidecar = crate::state::StateLabelSidecarBuilder::new(
                execution.prepared_text_layout(),
                execution.resolved_theme(),
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
            if let Some(error) = label_sidecar.prepared_error().cloned() {
                return Err(error.into());
            }
            BuiltinFamilyArtifact::State(Box::new(StateFamilyArtifact {
                pair: FamilyPair::new(model, layout),
                label_sidecar,
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
        BlendMode, CanvasLayer, CanvasPaint, CanvasSpec, DiagramThemeCompiler, DiagramThemeSpec,
        GradientStop, LinearGradient, OrdinalPalette, RootThemeEvaluation, RootThemeMechanismKey,
        RootThemeVerification, ThemeCapability, ThemeColorValue, ThemeRule, ThemeRuleSet,
        ThemeStylePatch, ThemeTarget, ThemeTextStyle, TypographySpec,
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
    use merman_core::{CustomJsonProvenance, CustomJsonRenderModel, Engine, ParseOptions};
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

    fn family_report(
        evaluation: FamilyStyleEvaluation,
        required: Vec<FamilyThemeMechanismKey>,
        applied: Vec<FamilyThemeMechanismKey>,
        theme_residuals: Vec<FamilyThemeResidual>,
    ) -> FamilyStyleReport {
        FamilyStyleReport {
            family_kind: RenderFamilyKind::State,
            evaluation,
            theme_required: required,
            theme_applied: applied,
            theme_not_applicable: Vec::new(),
            theme_residuals,
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
        assert_eq!(prepared_report.backend().name(), "merman.native-rustybuzz");
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
            let parsed = engine
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
        let parsed = engine
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
                theme_required: Vec::new(),
                theme_applied: Vec::new(),
                theme_not_applicable: Vec::new(),
                theme_residuals: Vec::new(),
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
        let parsed = Engine::new()
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
        let parsed = Engine::new()
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
        let parsed = Engine::new()
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
        let parsed = Engine::new()
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
        let parsed = Engine::new()
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
        let parsed = Engine::new()
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
        let parsed = Engine::new()
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
        let parsed = Engine::new()
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
    fn environment_portability_ceiling_rejects_an_unadapted_family() {
        let fill = CanvasPaint::solid("#ef4444").expect("valid node fill");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default().with_fill(fill),
                        )
                        .for_family(RenderFamilyKind::Flowchart),
                    ),
                ),
            )
            .expect("compile strict Flowchart theme");
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync("flowchart LR\nA --> B\n", ParseOptions::strict())
            .unwrap()
            .expect("Flowchart source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable render session");

        let error = match prepare(parsed, &LayoutOptions::default(), session) {
            Ok(_) => panic!("strict portability must reject an unadapted Flowchart theme"),
            Err(error) => error,
        };
        assert_eq!(
            error.unadapted_family_theme(),
            Some(RenderFamilyKind::Flowchart)
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
        let parsed = Engine::new()
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
        let parsed = Engine::new()
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
        let parsed = Engine::new()
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
        let parsed = Engine::new()
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
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\n[*] --> Ready\nReady --> [*]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("State source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable render session");

        let error = match prepare(parsed, &LayoutOptions::default(), session) {
            Ok(_) => panic!("strict portability must reject a State semantic residual"),
            Err(error) => error,
        };
        let (family_kind, residual_count, residual) = error
            .unverified_family_theme()
            .expect("typed family theme portability error");
        assert_eq!(family_kind, RenderFamilyKind::State);
        assert_eq!(residual_count, 1);
        assert_eq!(
            residual.reason(),
            FamilyThemeResidualReason::UnsupportedPaint
        );
    }

    #[test]
    fn require_portable_rejects_state_style_residual_before_layout() {
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new())
            .expect("compile portable theme recipe");
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "stateDiagram-v2\nclassDef broken font-size:not-a-size\n[*] --> Ready:::broken\nReady --> [*]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("State source should produce a render model");
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict portable render session");

        let error = match prepare(parsed, &LayoutOptions::default(), session) {
            Ok(_) => panic!("strict portability must reject an unverified State style"),
            Err(error) => error,
        };
        let (family_kind, residual_count, residual) = error
            .unverified_family_style()
            .expect("structured family style portability error");
        assert_eq!(family_kind, RenderFamilyKind::State);
        assert_eq!(residual_count, 1);
        assert_eq!(residual.owner_id(), "Ready");
        assert_eq!(residual.property(), Some("font-size"));
        assert_eq!(residual.reason(), FamilyStyleResidualReason::InvalidValue);
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

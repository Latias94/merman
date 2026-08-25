use std::collections::BTreeSet;

use super::admission::ThemeCapability;
use super::mechanisms::{
    collect_effect_graph_capabilities, collect_style_patch_capabilities, paint_capabilities,
};
use super::{CanvasSpec, DiagramTheme, DiagramThemeSpec, Specified, ThemeTarget};

/// Evaluation state for mechanisms that apply outside a selected diagram family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub(crate) enum RootThemeEvaluation {
    NotApplicable,
    Evaluated,
}

/// Recipe-local identity of one applicable root mechanism.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub(crate) enum RootThemeMechanismKey {
    CanvasBase,
    CanvasLayer {
        index: usize,
    },
    CanvasGeometry,
    CanvasRule {
        index: usize,
    },
    CanvasOrdinalPalette,
    EffectBinding {
        target: ThemeTarget,
        effect_id: String,
    },
}

/// Recipe-local identity of one family-scoped semantic mechanism.
///
/// This remains renderer-private until family adapters preserve facet-level identity end to end.
/// The current rule-level shape is sufficient for internal accounting, but is not a stable public
/// contract because one rule can mix supported and unsupported facets.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum FamilyThemeMechanismKey {
    Typography,
    Rule {
        index: usize,
        target: ThemeTarget,
    },
    OrdinalPalette {
        target: ThemeTarget,
    },
    EffectBinding {
        target: ThemeTarget,
        effect_id: String,
    },
}

impl std::fmt::Display for FamilyThemeMechanismKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Typography => formatter.write_str("typography"),
            Self::Rule { index, target } => write!(formatter, "rule[{index}:{}]", target.id()),
            Self::OrdinalPalette { target } => {
                write!(formatter, "ordinal-palette[{}]", target.id())
            }
            Self::EffectBinding { target, effect_id } => {
                write!(formatter, "effect-binding[{}={effect_id}]", target.id())
            }
        }
    }
}

impl std::fmt::Display for RootThemeMechanismKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CanvasBase => formatter.write_str("canvas-base"),
            Self::CanvasLayer { index } => write!(formatter, "canvas-layer[{index}]"),
            Self::CanvasGeometry => formatter.write_str("canvas-geometry"),
            Self::CanvasRule { index } => write!(formatter, "canvas-rule[{index}]"),
            Self::CanvasOrdinalPalette => formatter.write_str("canvas-ordinal-palette"),
            Self::EffectBinding { target, effect_id } => {
                write!(formatter, "effect-binding[{}={effect_id}]", target.id())
            }
        }
    }
}

/// Why a root-level mechanism remains outside the renderer's proof boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub(crate) enum RootThemeResidualReason {
    NoConsumer,
    OutputMutation,
}

impl RootThemeResidualReason {
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoConsumer => "no-consumer",
            Self::OutputMutation => "output-mutation",
        }
    }
}

impl std::fmt::Display for RootThemeResidualReason {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.id())
    }
}

/// Immutable evidence for one unapplied root-level theme mechanism.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RootThemeResidual {
    key: RootThemeMechanismKey,
    capabilities: BTreeSet<ThemeCapability>,
    reason: RootThemeResidualReason,
}

impl RootThemeResidual {
    pub const fn key(&self) -> &RootThemeMechanismKey {
        &self.key
    }

    pub const fn reason(&self) -> RootThemeResidualReason {
        self.reason
    }
}

impl std::fmt::Display for RootThemeResidual {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} requires ", self.key)?;
        for (index, capability) in self.capabilities.iter().enumerate() {
            if index != 0 {
                formatter.write_str(", ")?;
            }
            write!(formatter, "{capability}")?;
        }
        write!(formatter, " ({})", self.reason)
    }
}

/// Capability-level evidence for one root mechanism.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RootThemeMechanismEvidence {
    key: RootThemeMechanismKey,
    required_capabilities: BTreeSet<ThemeCapability>,
    applied_capabilities: BTreeSet<ThemeCapability>,
    residual_capabilities: BTreeSet<ThemeCapability>,
    residual_reason: Option<RootThemeResidualReason>,
}

impl RootThemeMechanismEvidence {
    pub const fn key(&self) -> &RootThemeMechanismKey {
        &self.key
    }

    pub fn required_capabilities(&self) -> impl ExactSizeIterator<Item = ThemeCapability> + '_ {
        self.required_capabilities.iter().copied()
    }

    pub fn applied_capabilities(&self) -> impl ExactSizeIterator<Item = ThemeCapability> + '_ {
        self.applied_capabilities.iter().copied()
    }

    pub fn residual_capabilities(&self) -> impl ExactSizeIterator<Item = ThemeCapability> + '_ {
        self.residual_capabilities.iter().copied()
    }

    fn coverage_complete(&self) -> bool {
        self.has_residual() == self.residual_reason.is_some()
            && self
                .applied_capabilities
                .is_disjoint(&self.residual_capabilities)
            && self
                .applied_capabilities
                .union(&self.residual_capabilities)
                .eq(&self.required_capabilities)
    }

    fn has_residual(&self) -> bool {
        !self.residual_capabilities.is_empty()
    }
}

/// Frozen root-level theme evidence. Positive verification requires explicit per-mechanism
/// coverage; an empty residual list alone can never manufacture success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RootThemeReport {
    evaluation: RootThemeEvaluation,
    mechanisms: Vec<RootThemeMechanismEvidence>,
    required_mechanisms: Vec<RootThemeMechanismKey>,
    applied_mechanisms: Vec<RootThemeMechanismKey>,
    required_capabilities: BTreeSet<ThemeCapability>,
    applied_capabilities: BTreeSet<ThemeCapability>,
    residuals: Vec<RootThemeResidual>,
}

/// Recipe-local root work retained until the operation-owned SVG root has completed assembly.
///
/// A plan is deliberately not a report: parsing a theme can identify applicable mechanisms, but
/// only the root consumer may freeze applied or residual evidence.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct RootThemePlan {
    requirements: Vec<RootThemeRequirement>,
    canvas: CanvasSpec,
}

impl RootThemePlan {
    pub(crate) fn from_theme(theme: Option<&DiagramTheme>) -> Self {
        let Some(theme) = theme else {
            return Self::default();
        };
        Self {
            requirements: root_theme_requirements(theme.spec()),
            canvas: theme.spec().canvas().clone(),
        }
    }

    pub(crate) const fn canvas(&self) -> &CanvasSpec {
        &self.canvas
    }

    pub(crate) fn begin_svg_application(&self) -> RootThemeApplication {
        RootThemeApplication {
            mechanisms: self
                .requirements
                .iter()
                .cloned()
                .map(RootThemeRequirement::into_unadapted_evidence)
                .collect(),
        }
    }
}

/// Mutable, operation-local evidence builder owned by the actual SVG root consumer.
#[derive(Debug)]
pub(crate) struct RootThemeApplication {
    mechanisms: Vec<RootThemeMechanismEvidence>,
}

impl RootThemeApplication {
    /// Records the capabilities that the root consumer emitted for one mechanism.
    ///
    /// A consumer must report its actual emission rather than obtaining blanket credit for every
    /// capability required by the recipe. Any required capability omitted from `applied` remains a
    /// residual and keeps strict portability fail-closed.
    pub(crate) fn mark_applied(
        &mut self,
        key: &RootThemeMechanismKey,
        applied: impl IntoIterator<Item = ThemeCapability>,
    ) -> bool {
        let Some(mechanism) = self
            .mechanisms
            .iter_mut()
            .find(|mechanism| mechanism.key() == key)
        else {
            return false;
        };

        let applied = applied.into_iter().collect::<BTreeSet<_>>();
        if !applied.is_subset(&mechanism.required_capabilities) {
            return false;
        }
        mechanism.applied_capabilities = applied;
        mechanism.residual_capabilities = mechanism
            .required_capabilities
            .difference(&mechanism.applied_capabilities)
            .copied()
            .collect();
        mechanism.residual_reason = (!mechanism.residual_capabilities.is_empty())
            .then_some(RootThemeResidualReason::NoConsumer);
        true
    }

    pub(crate) fn finish(self) -> RootThemeReport {
        if self.mechanisms.is_empty() {
            RootThemeReport::not_applicable()
        } else {
            RootThemeReport::freeze(RootThemeEvaluation::Evaluated, self.mechanisms)
        }
    }
}

impl RootThemeReport {
    fn not_applicable() -> Self {
        Self::freeze(RootThemeEvaluation::NotApplicable, Vec::new())
    }

    fn freeze(
        evaluation: RootThemeEvaluation,
        mechanisms: Vec<RootThemeMechanismEvidence>,
    ) -> Self {
        let required_mechanisms = mechanisms
            .iter()
            .map(|mechanism| mechanism.key.clone())
            .collect();
        let applied_mechanisms = mechanisms
            .iter()
            .filter(|mechanism| {
                !mechanism.has_residual()
                    && mechanism.applied_capabilities == mechanism.required_capabilities
            })
            .map(|mechanism| mechanism.key.clone())
            .collect();
        let required_capabilities = mechanisms
            .iter()
            .flat_map(|mechanism| mechanism.required_capabilities.iter().copied())
            .collect();
        let applied_capabilities = mechanisms
            .iter()
            .flat_map(|mechanism| mechanism.applied_capabilities.iter().copied())
            .collect();
        let residuals = mechanisms
            .iter()
            .filter_map(|mechanism| {
                let reason = mechanism.residual_reason?;
                (!mechanism.residual_capabilities.is_empty()).then(|| RootThemeResidual {
                    key: mechanism.key.clone(),
                    capabilities: mechanism.residual_capabilities.clone(),
                    reason,
                })
            })
            .collect();
        Self {
            evaluation,
            mechanisms,
            required_mechanisms,
            applied_mechanisms,
            required_capabilities,
            applied_capabilities,
            residuals,
        }
    }

    pub const fn evaluation(&self) -> RootThemeEvaluation {
        self.evaluation
    }

    pub fn required_mechanisms(&self) -> &[RootThemeMechanismKey] {
        &self.required_mechanisms
    }

    pub fn mechanisms(&self) -> &[RootThemeMechanismEvidence] {
        &self.mechanisms
    }

    pub fn applied_mechanisms(&self) -> &[RootThemeMechanismKey] {
        &self.applied_mechanisms
    }

    pub fn required_capabilities(&self) -> impl ExactSizeIterator<Item = ThemeCapability> + '_ {
        self.required_capabilities.iter().copied()
    }

    pub fn applied_capabilities(&self) -> impl ExactSizeIterator<Item = ThemeCapability> + '_ {
        self.applied_capabilities.iter().copied()
    }

    pub fn residuals(&self) -> &[RootThemeResidual] {
        &self.residuals
    }

    pub fn coverage_complete(&self) -> bool {
        let required = self.required_mechanisms.iter().collect::<BTreeSet<_>>();
        let evidence_keys = self
            .mechanisms
            .iter()
            .map(RootThemeMechanismEvidence::key)
            .collect::<BTreeSet<_>>();
        required.len() == self.required_mechanisms.len()
            && evidence_keys.len() == self.mechanisms.len()
            && evidence_keys == required
            && self
                .mechanisms
                .iter()
                .all(RootThemeMechanismEvidence::coverage_complete)
    }

    pub fn verification(&self) -> RootThemeVerification {
        match self.evaluation {
            RootThemeEvaluation::NotApplicable => RootThemeVerification::NotApplicable,
            RootThemeEvaluation::Evaluated => {
                let coverage_complete = self.coverage_complete();
                if !coverage_complete {
                    RootThemeVerification::Incomplete
                } else if self.mechanisms.iter().all(|mechanism| {
                    !mechanism.has_residual()
                        && mechanism.applied_capabilities == mechanism.required_capabilities
                }) && self.residuals.is_empty()
                {
                    RootThemeVerification::Verified
                } else {
                    RootThemeVerification::Unverified
                }
            }
        }
    }

    pub fn is_verified(&self) -> bool {
        self.verification() == RootThemeVerification::Verified
    }

    /// Invalidates positive root evidence after an untrusted SVG transformation.
    ///
    /// The transformation may have changed a typed canvas/effect node without exposing a
    /// machine-checkable proof of preservation. Keeping the original `Verified` state would make
    /// downstream document admission claim more than the final SVG proves.
    pub(crate) fn invalidate_for_output_mutation(self) -> Self {
        if self.evaluation == RootThemeEvaluation::NotApplicable {
            return self;
        }
        let mechanisms = self
            .mechanisms
            .into_iter()
            .map(|mut mechanism| {
                mechanism.applied_capabilities.clear();
                mechanism.residual_capabilities = mechanism.required_capabilities.clone();
                mechanism.residual_reason = (!mechanism.residual_capabilities.is_empty())
                    .then_some(RootThemeResidualReason::OutputMutation);
                mechanism
            })
            .collect();
        Self::freeze(RootThemeEvaluation::Evaluated, mechanisms)
    }
}

/// Terminal interpretation of root-level theme evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RootThemeVerification {
    NotApplicable,
    Verified,
    Unverified,
    Incomplete,
}

impl RootThemeVerification {
    pub const ALL: &'static [Self] = &[
        Self::NotApplicable,
        Self::Verified,
        Self::Unverified,
        Self::Incomplete,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RootThemeRequirement {
    key: RootThemeMechanismKey,
    capabilities: BTreeSet<ThemeCapability>,
}

impl RootThemeRequirement {
    fn new(
        key: RootThemeMechanismKey,
        capabilities: impl IntoIterator<Item = ThemeCapability>,
    ) -> Self {
        Self {
            key,
            capabilities: capabilities.into_iter().collect(),
        }
    }

    pub(crate) fn capabilities(&self) -> impl Iterator<Item = ThemeCapability> + '_ {
        self.capabilities.iter().copied()
    }

    fn into_unadapted_evidence(self) -> RootThemeMechanismEvidence {
        RootThemeMechanismEvidence {
            key: self.key,
            required_capabilities: self.capabilities.clone(),
            applied_capabilities: BTreeSet::new(),
            residual_capabilities: self.capabilities,
            residual_reason: Some(RootThemeResidualReason::NoConsumer),
        }
    }
}

pub(crate) fn root_theme_requirements(spec: &DiagramThemeSpec) -> Vec<RootThemeRequirement> {
    let mut requirements = Vec::new();
    if spec.canvas().has_explicit_base() {
        let capabilities = paint_capabilities(spec.canvas().base()).collect::<BTreeSet<_>>();
        if !capabilities.is_empty() {
            requirements.push(RootThemeRequirement::new(
                RootThemeMechanismKey::CanvasBase,
                capabilities,
            ));
        }
    }
    for (index, layer) in spec.canvas().layers().iter().enumerate() {
        let mut capabilities = BTreeSet::from([ThemeCapability::LayeredCanvas]);
        capabilities.extend(paint_capabilities(layer.paint()));
        if layer.opacity() != 1.0 {
            capabilities.insert(ThemeCapability::Opacity);
        }
        if layer.offset() != (0.0, 0.0) {
            capabilities.insert(ThemeCapability::CanvasLayerPlacement);
        }
        if !matches!(layer.blend_mode(), super::BlendMode::Normal) {
            capabilities.insert(ThemeCapability::BlendMode);
        }
        requirements.push(RootThemeRequirement::new(
            RootThemeMechanismKey::CanvasLayer { index },
            capabilities,
        ));
    }
    if spec.canvas().bleed() != super::InsetsPx::ZERO {
        requirements.push(RootThemeRequirement::new(
            RootThemeMechanismKey::CanvasGeometry,
            [ThemeCapability::CanvasBleed],
        ));
    }

    for (index, rule) in spec.styles().rules().iter().enumerate() {
        if rule.target() != ThemeTarget::Canvas {
            continue;
        }
        let mut capabilities = BTreeSet::from([
            ThemeCapability::SemanticTokens,
            ThemeCapability::SemanticRules,
        ]);
        collect_style_patch_capabilities(rule.style(), &mut capabilities);
        if let Specified::Value(effect_id) = &rule.style().effects.effect {
            let graph = spec
                .effects()
                .graph(effect_id)
                .expect("validated canvas effect reference must resolve");
            collect_effect_graph_capabilities(graph, &mut capabilities);
        }
        requirements.push(RootThemeRequirement::new(
            RootThemeMechanismKey::CanvasRule { index },
            capabilities,
        ));
    }
    if spec
        .styles()
        .ordinal_palettes()
        .iter()
        .any(|(target, _)| *target == ThemeTarget::Canvas)
    {
        requirements.push(RootThemeRequirement::new(
            RootThemeMechanismKey::CanvasOrdinalPalette,
            [ThemeCapability::OrdinalPalette],
        ));
    }
    for binding in spec
        .effects()
        .bindings()
        .iter()
        .filter(|binding| binding.target() == ThemeTarget::Canvas)
    {
        let graph = spec
            .effects()
            .graph(binding.effect_id())
            .expect("validated canvas effect binding must resolve");
        let mut capabilities = BTreeSet::new();
        collect_effect_graph_capabilities(graph, &mut capabilities);
        requirements.push(RootThemeRequirement::new(
            RootThemeMechanismKey::EffectBinding {
                target: binding.target(),
                effect_id: binding.effect_id().to_string(),
            },
            capabilities,
        ));
    }
    requirements
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(
        key: RootThemeMechanismKey,
        required: &[ThemeCapability],
        applied: &[ThemeCapability],
        residual: &[ThemeCapability],
        residual_reason: Option<RootThemeResidualReason>,
    ) -> RootThemeMechanismEvidence {
        RootThemeMechanismEvidence {
            key,
            required_capabilities: required.iter().copied().collect(),
            applied_capabilities: applied.iter().copied().collect(),
            residual_capabilities: residual.iter().copied().collect(),
            residual_reason,
        }
    }

    #[test]
    fn root_verification_state_matrix_is_fail_closed() {
        let capability = ThemeCapability::SolidPaint;
        let applied = evidence(
            RootThemeMechanismKey::CanvasBase,
            &[capability],
            &[capability],
            &[],
            None,
        );
        let residual = evidence(
            RootThemeMechanismKey::CanvasBase,
            &[capability],
            &[],
            &[capability],
            Some(RootThemeResidualReason::NoConsumer),
        );
        let incomplete = evidence(
            RootThemeMechanismKey::CanvasBase,
            &[capability],
            &[],
            &[],
            None,
        );

        assert_eq!(
            RootThemeReport::freeze(RootThemeEvaluation::NotApplicable, Vec::new()).verification(),
            RootThemeVerification::NotApplicable
        );
        assert_eq!(
            RootThemeReport::freeze(RootThemeEvaluation::Evaluated, vec![applied.clone()])
                .verification(),
            RootThemeVerification::Verified
        );
        assert_eq!(
            RootThemeReport::freeze(RootThemeEvaluation::Evaluated, vec![residual]).verification(),
            RootThemeVerification::Unverified
        );
        assert_eq!(
            RootThemeReport::freeze(RootThemeEvaluation::Evaluated, vec![incomplete])
                .verification(),
            RootThemeVerification::Incomplete
        );
        assert_eq!(
            RootThemeReport::freeze(
                RootThemeEvaluation::Evaluated,
                vec![applied.clone(), applied],
            )
            .verification(),
            RootThemeVerification::Incomplete
        );
    }

    #[test]
    fn root_evidence_requires_a_reason_exactly_when_capabilities_are_residual() {
        let capability = ThemeCapability::SolidPaint;
        let missing_reason = evidence(
            RootThemeMechanismKey::CanvasBase,
            &[capability],
            &[],
            &[capability],
            None,
        );
        let spurious_reason = evidence(
            RootThemeMechanismKey::CanvasBase,
            &[capability],
            &[capability],
            &[],
            Some(RootThemeResidualReason::NoConsumer),
        );

        for mechanism in [missing_reason, spurious_reason] {
            let report = RootThemeReport::freeze(RootThemeEvaluation::Evaluated, vec![mechanism]);
            assert_eq!(report.verification(), RootThemeVerification::Incomplete);
        }
    }
}

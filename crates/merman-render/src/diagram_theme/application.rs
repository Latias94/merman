use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

use super::admission::ThemeCapability;
use super::mechanisms::{
    collect_effect_graph_capabilities, collect_style_patch_capabilities, paint_capabilities,
};
use super::{
    CanvasSpec, DiagramTheme, DiagramThemeSpec, Specified, ThemeTarget, ThemeTypographyProperty,
};

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
    Typography(ThemeTypographyProperty),
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
            Self::Typography(property) => write!(formatter, "typography[{}]", property.id()),
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

/// Requested and emitted capabilities for one root mechanism.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RootThemeMechanismOutcome {
    key: RootThemeMechanismKey,
    required_capabilities: BTreeSet<ThemeCapability>,
    applied_capabilities: BTreeSet<ThemeCapability>,
}

/// Frozen root outcomes recorded by the SVG writer, with terminal counts and capability union.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RootThemeReport {
    mechanisms: Vec<RootThemeMechanismOutcome>,
    applied_count: usize,
    residual_count: usize,
    applied_capabilities: BTreeSet<ThemeCapability>,
    output_mutated: bool,
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
    pub(crate) fn from_spec(spec: &DiagramThemeSpec) -> Self {
        Self {
            requirements: root_theme_requirements(spec),
            canvas: spec.canvas().clone(),
        }
    }

    pub(crate) fn from_theme(theme: Option<&DiagramTheme>) -> Arc<Self> {
        static EMPTY: OnceLock<Arc<RootThemePlan>> = OnceLock::new();
        match theme {
            Some(theme) => Arc::clone(theme.root_plan()),
            None => Arc::clone(EMPTY.get_or_init(|| Arc::new(Self::default()))),
        }
    }

    pub(crate) fn required_capabilities(&self) -> impl Iterator<Item = ThemeCapability> + '_ {
        self.requirements
            .iter()
            .flat_map(RootThemeRequirement::capabilities)
    }

    pub(crate) const fn canvas(&self) -> &CanvasSpec {
        &self.canvas
    }

    /// A family backdrop must not cover an explicit root canvas unless authored config owns it.
    pub(crate) fn replaces_default_family_background(
        &self,
        config: &merman_core::MermaidConfig,
        family_background_path: &str,
    ) -> bool {
        (self.canvas.has_explicit_base() || !self.canvas.layers().is_empty())
            && !merman_core::__private::config_paint_overrides_typed_default(
                config,
                "themeVariables.background",
            )
            && !merman_core::__private::config_paint_overrides_typed_default(
                config,
                family_background_path,
            )
    }

    pub(crate) fn begin_svg_application(&self) -> RootThemeApplication {
        RootThemeApplication {
            mechanisms: self
                .requirements
                .iter()
                .cloned()
                .map(RootThemeRequirement::into_unapplied_outcome)
                .collect(),
        }
    }
}

/// Mutable, operation-local evidence builder owned by the actual SVG root consumer.
#[derive(Debug)]
pub(crate) struct RootThemeApplication {
    mechanisms: Vec<RootThemeMechanismOutcome>,
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
            .find(|mechanism| &mechanism.key == key)
        else {
            return false;
        };

        let applied = applied.into_iter().collect::<BTreeSet<_>>();
        if !applied.is_subset(&mechanism.required_capabilities) {
            return false;
        }
        mechanism.applied_capabilities = applied;
        true
    }

    pub(crate) fn finish(self) -> RootThemeReport {
        let mut applied_count = 0;
        let mut applied_capabilities = BTreeSet::new();
        for mechanism in &self.mechanisms {
            if mechanism.applied_capabilities == mechanism.required_capabilities {
                applied_count += 1;
            }
            applied_capabilities.extend(mechanism.applied_capabilities.iter().copied());
        }
        RootThemeReport {
            residual_count: self.mechanisms.len() - applied_count,
            mechanisms: self.mechanisms,
            applied_count,
            applied_capabilities,
            output_mutated: false,
        }
    }
}

impl RootThemeReport {
    pub const fn required_count(&self) -> usize {
        self.mechanisms.len()
    }

    pub const fn applied_count(&self) -> usize {
        self.applied_count
    }

    pub const fn residual_count(&self) -> usize {
        self.residual_count
    }

    pub const fn output_mutated(&self) -> bool {
        self.output_mutated
    }

    pub fn applied_capabilities(&self) -> impl ExactSizeIterator<Item = ThemeCapability> + '_ {
        self.applied_capabilities.iter().copied()
    }

    pub fn residuals(
        &self,
    ) -> impl Iterator<Item = (&RootThemeMechanismKey, RootThemeResidualReason)> + '_ {
        let reason = if self.output_mutated {
            RootThemeResidualReason::OutputMutation
        } else {
            RootThemeResidualReason::NoConsumer
        };
        self.mechanisms
            .iter()
            .filter(|mechanism| {
                self.output_mutated
                    || mechanism.applied_capabilities != mechanism.required_capabilities
            })
            .map(move |mechanism| (&mechanism.key, reason))
    }

    pub const fn verification(&self) -> RootThemeVerification {
        if self.mechanisms.is_empty() {
            RootThemeVerification::NotApplicable
        } else if self.residual_count == 0 {
            RootThemeVerification::Verified
        } else {
            RootThemeVerification::Unverified
        }
    }

    /// Invalidates positive root evidence after an untrusted SVG transformation.
    ///
    /// The transformation may have changed a typed canvas/effect node without exposing a
    /// machine-checkable proof of preservation. Keeping the original `Verified` state would make
    /// downstream document admission claim more than the final SVG proves.
    pub(crate) fn invalidate_for_output_mutation(mut self) -> Self {
        if self.mechanisms.is_empty() {
            return self;
        }
        for mechanism in &mut self.mechanisms {
            mechanism.applied_capabilities.clear();
        }
        self.applied_count = 0;
        self.residual_count = self.mechanisms.len();
        self.applied_capabilities.clear();
        self.output_mutated = true;
        self
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

    fn into_unapplied_outcome(self) -> RootThemeMechanismOutcome {
        RootThemeMechanismOutcome {
            key: self.key,
            required_capabilities: self.capabilities,
            applied_capabilities: BTreeSet::new(),
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

    fn solid_layer() -> super::super::CanvasLayer {
        super::super::CanvasLayer::new(super::super::CanvasPaint::solid("#123456").unwrap())
    }

    #[test]
    fn compiled_root_plan_is_shared_but_application_evidence_is_operation_local() {
        let theme = super::super::DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_canvas(CanvasSpec::solid("#123456").unwrap()))
            .unwrap();
        let first = RootThemePlan::from_theme(Some(&theme));
        let second = RootThemePlan::from_theme(Some(&theme));
        assert!(Arc::ptr_eq(&first, &second));
        assert!(Arc::ptr_eq(&first, theme.root_plan()));
        assert!(Arc::ptr_eq(
            &RootThemePlan::from_theme(None),
            &RootThemePlan::from_theme(None)
        ));
        let mut applied = first.begin_svg_application();
        assert!(applied.mark_applied(
            &RootThemeMechanismKey::CanvasBase,
            [ThemeCapability::SolidPaint]
        ));
        assert_eq!(
            applied.finish().verification(),
            RootThemeVerification::Verified
        );
        assert_eq!(
            second.begin_svg_application().finish().verification(),
            RootThemeVerification::Unverified
        );
    }

    #[test]
    fn root_application_rejects_unknown_keys_and_capabilities_without_losing_partial_emission() {
        let canvas = CanvasSpec::default().with_layer(solid_layer()).unwrap();
        let plan = RootThemePlan::from_spec(&DiagramThemeSpec::new().with_canvas(canvas));
        let mut application = plan.begin_svg_application();
        assert!(!application.mark_applied(
            &RootThemeMechanismKey::CanvasBase,
            [ThemeCapability::SolidPaint],
        ));
        let key = RootThemeMechanismKey::CanvasLayer { index: 0 };
        assert!(application.mark_applied(&key, [ThemeCapability::SolidPaint]));
        assert!(!application.mark_applied(
            &key,
            [ThemeCapability::SolidPaint, ThemeCapability::SvgFilter],
        ));
        let report = application.finish();

        assert_eq!(report.verification(), RootThemeVerification::Unverified);
        assert_eq!(report.required_count(), 1);
        assert_eq!(report.applied_count(), 0);
        assert_eq!(report.residual_count(), 1);
        assert_eq!(
            report.applied_capabilities().collect::<Vec<_>>(),
            [ThemeCapability::SolidPaint]
        );
        assert_eq!(
            report.residuals().collect::<Vec<_>>(),
            [(&key, RootThemeResidualReason::NoConsumer)]
        );
        assert!(!report.output_mutated());
    }

    #[test]
    fn repeated_root_marks_replace_previous_emission() {
        let canvas = CanvasSpec::default().with_layer(solid_layer()).unwrap();
        let plan = RootThemePlan::from_spec(&DiagramThemeSpec::new().with_canvas(canvas));
        let key = RootThemeMechanismKey::CanvasLayer { index: 0 };
        let full = [ThemeCapability::LayeredCanvas, ThemeCapability::SolidPaint];
        for applied in [Vec::new(), vec![ThemeCapability::SolidPaint], full.to_vec()] {
            let mut application = plan.begin_svg_application();
            assert!(application.mark_applied(&key, full));
            assert!(application.mark_applied(&key, applied.iter().copied()));
            let report = application.finish();
            let complete = applied.len() == full.len();

            assert_eq!(report.required_count(), 1);
            assert_eq!(report.applied_count(), usize::from(complete));
            assert_eq!(report.residual_count(), usize::from(!complete));
            assert_eq!(
                report.applied_capabilities().collect::<BTreeSet<_>>(),
                applied.into_iter().collect()
            );
            assert_eq!(
                report.verification(),
                if complete {
                    RootThemeVerification::Verified
                } else {
                    RootThemeVerification::Unverified
                }
            );
        }
    }

    #[test]
    fn output_mutation_invalidates_every_root_mechanism_in_recipe_order() {
        let canvas = CanvasSpec::solid("#654321")
            .unwrap()
            .with_layer(solid_layer())
            .unwrap()
            .with_layer(solid_layer())
            .unwrap();
        let plan = RootThemePlan::from_spec(&DiagramThemeSpec::new().with_canvas(canvas));
        let mut application = plan.begin_svg_application();
        assert!(application.mark_applied(
            &RootThemeMechanismKey::CanvasBase,
            [ThemeCapability::SolidPaint],
        ));
        assert!(application.mark_applied(
            &RootThemeMechanismKey::CanvasLayer { index: 0 },
            [ThemeCapability::SolidPaint],
        ));
        let report = application.finish();
        assert_eq!(report.applied_count(), 1);
        assert_eq!(report.residual_count(), 2);
        let keys = [
            RootThemeMechanismKey::CanvasBase,
            RootThemeMechanismKey::CanvasLayer { index: 0 },
            RootThemeMechanismKey::CanvasLayer { index: 1 },
        ];
        assert_eq!(
            report.residuals().map(|(key, _)| key).collect::<Vec<_>>(),
            [&keys[1], &keys[2]]
        );

        let invalidated = report.invalidate_for_output_mutation();
        assert_eq!(
            invalidated.verification(),
            RootThemeVerification::Unverified
        );
        assert_eq!(invalidated.required_count(), 3);
        assert_eq!(invalidated.applied_count(), 0);
        assert_eq!(invalidated.residual_count(), 3);
        assert!(invalidated.applied_capabilities().next().is_none());
        assert!(invalidated.output_mutated());
        assert_eq!(
            invalidated.residuals().collect::<Vec<_>>(),
            keys.iter()
                .map(|key| (key, RootThemeResidualReason::OutputMutation))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            invalidated.clone().invalidate_for_output_mutation(),
            invalidated
        );

        let empty = RootThemePlan::default().begin_svg_application().finish();
        assert_eq!(empty.verification(), RootThemeVerification::NotApplicable);
        assert_eq!(empty.required_count(), 0);
        assert_eq!(empty.applied_count(), 0);
        assert_eq!(empty.residual_count(), 0);
        assert!(empty.residuals().next().is_none());
        assert!(!empty.output_mutated());
        assert_eq!(empty.clone().invalidate_for_output_mutation(), empty);
    }
}

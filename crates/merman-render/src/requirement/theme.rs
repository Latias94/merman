use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;
use merman_core::diagrams::requirement::RequirementDiagramRenderModel;

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemePaintKind, FamilyThemeRuleFacet, ResolvedDiagramTheme, ResolvedStyleProperty,
    ResolvedThemeStyle, Specified, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::OperationWorkMeter;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedFill {
    rule_index: usize,
    css: String,
}

/// Requirement box fill resolved once for semantic nodes and shared by SVG emission and evidence.
#[derive(Debug)]
pub(crate) struct RequirementFillThemePlan {
    node_indices: BTreeMap<String, usize>,
    expectations: Vec<Option<ExpectedFill>>,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, BTreeSet<ThemeCapability>>,
    terminal_receipt: OnceLock<RequirementFillThemeReceipt>,
}

impl RequirementFillThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        model: &RequirementDiagramRenderModel,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let mut node_indices = BTreeMap::new();
        for node_id in model
            .requirements
            .iter()
            .map(|node| node.name.as_str())
            .chain(model.elements.iter().map(|node| node.name.as_str()))
            .filter(|node_id| *node_id != "__proto__")
        {
            let next_index = node_indices.len();
            node_indices
                .entry(node_id.to_string())
                .or_insert(next_index);
        }
        let node_count = node_indices.len();
        let expectations = vec![None; node_count];
        let Some(theme) = theme else {
            return Ok(Self {
                node_indices,
                expectations,
                evidence: FamilyThemeEvidence::default(),
                pending: BTreeMap::new(),
                terminal_receipt: OnceLock::new(),
            });
        };

        let mermaid_owns_fill = mermaid_owns_requirement_fill(effective_config);
        let mut expectations = expectations;
        let mut winner_properties = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut expected_capabilities =
            BTreeMap::<(usize, ResolvedStyleProperty), ThemeCapability>::new();

        for node_index in 0..node_count {
            let style = theme.style_with_work_meter(
                ThemeTarget::Requirement,
                ThemeVariant::Default,
                Some(node_index + 1),
                work_meter,
            )?;
            for (property, origin) in style.winner_rule_properties() {
                winner_properties.insert((origin.rule_index(), property));
            }
            if let Some(expected) = typed_fill_expectation(theme, &style, mermaid_owns_fill) {
                expected_capabilities.insert(
                    (expected.rule_index, ResolvedStyleProperty::Fill),
                    paint_capability_from_css(&expected.css),
                );
                expectations[node_index] = Some(expected);
            }
        }

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<usize, RequirementFillRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Requirement,
                    facet,
                    ..
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    let property = resolved_style_property_for_facet(facet);
                    if !winner_properties.contains(&(rule_index, property)) {
                        continue;
                    }
                    observation.applicable = true;
                    match (route.disposition(), facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeRuleFacet::Fill(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) => {
                            if let Some(capability) =
                                expected_capabilities.get(&(rule_index, property))
                            {
                                observation.capabilities.insert(*capability);
                            } else {
                                observation.suppressed = true;
                            }
                        }
                        (FamilyThemeDisposition::Unsupported, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::TypedAdapter, _)
                        | (FamilyThemeDisposition::LegacyCompatibility, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Requirement,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if node_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Requirement,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if node_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() != FamilyThemeDisposition::LegacyCompatibility {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    }
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        let mut pending = BTreeMap::new();
        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Requirement,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Mixed rules remain fail-closed until every winning facet has a terminal owner.
            } else if !observation.capabilities.is_empty() {
                pending.insert(key, observation.capabilities);
            } else if observation.suppressed {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            node_indices,
            expectations,
            evidence,
            pending,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn index_for_node_id(&self, node_id: &str) -> Option<usize> {
        self.node_indices.get(node_id).copied()
    }

    pub(crate) fn typed_fill(
        &self,
        node_index: usize,
        source_owns_fill: bool,
    ) -> Option<(usize, &str)> {
        (!source_owns_fill)
            .then(|| self.expectations.get(node_index)?.as_ref())
            .flatten()
            .map(|expected| (expected.rule_index, expected.css.as_str()))
    }

    pub(crate) fn begin_terminal_receipt(&self) -> RequirementFillThemeReceipt {
        RequirementFillThemeReceipt::new(self.expectations.clone())
    }

    pub(crate) fn record_terminal(&self, receipt: RequirementFillThemeReceipt) -> bool {
        receipt.proves_complete() && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(receipt) = self.terminal_receipt.get() else {
            return evidence;
        };
        for (key, capabilities) in &self.pending {
            let rule_index = match key {
                FamilyThemeMechanismKey::Rule { index, .. } => *index,
                FamilyThemeMechanismKey::Typography
                | FamilyThemeMechanismKey::OrdinalPalette { .. }
                | FamilyThemeMechanismKey::EffectBinding { .. } => continue,
            };
            if receipt.proves_rule(rule_index) {
                evidence.mark_applied_with_capabilities(key.clone(), capabilities.iter().copied());
            } else if receipt.proves_complete() && !receipt.has_effective_rule(rule_index) {
                evidence.mark_not_applicable(key.clone());
            }
        }
        evidence
    }
}

fn typed_fill_expectation(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    mermaid_owns: bool,
) -> Option<ExpectedFill> {
    if mermaid_owns {
        return None;
    }
    let origin = style.fill_resolution().winner()?;
    let facet = FamilyThemeRuleFacet::fill(style.fill_resolution().specified())?;
    if theme.rule_facet_disposition(origin.rule_index(), facet)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    let css = match style.fill_resolution().specified() {
        Specified::Value(CanvasPaint::Transparent) => "transparent".to_string(),
        Specified::Value(CanvasPaint::Solid(color)) => color.as_css(),
        Specified::Unspecified
        | Specified::Clear
        | Specified::Value(
            CanvasPaint::LinearGradient(_)
            | CanvasPaint::RadialGradient(_)
            | CanvasPaint::Pattern(_),
        ) => return None,
    };
    Some(ExpectedFill {
        rule_index: origin.rule_index(),
        css,
    })
}

fn mermaid_owns_requirement_fill(config: &MermaidConfig) -> bool {
    merman_core::__private::config_path_overrides_typed_default(
        config,
        "themeVariables.requirementBackground",
    ) || matches!(
        config.get_str("theme"),
        Some("redux-color" | "redux-dark-color")
    ) || (merman_core::__private::config_path_overrides_typed_default(
        config,
        "themeVariables.borderColorArray",
    ) && merman_core::__private::config_path_overrides_typed_default(
        config,
        "themeVariables.bkgColorArray",
    ))
}

#[derive(Debug, Default)]
struct RequirementFillRuleObservation {
    applicable: bool,
    incomplete: bool,
    suppressed: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeSet<ThemeCapability>,
}

/// Writer-owned proof that every semantic Requirement node reached its canonical fill path.
#[derive(Debug, Clone)]
pub(crate) struct RequirementFillThemeReceipt {
    expectations: Vec<Option<ExpectedFill>>,
    checkpointed_nodes: Vec<bool>,
    attributes_match: bool,
    effective_by_rule: BTreeMap<usize, usize>,
    emitted_by_rule: BTreeMap<usize, usize>,
}

impl RequirementFillThemeReceipt {
    fn new(expectations: Vec<Option<ExpectedFill>>) -> Self {
        Self {
            checkpointed_nodes: vec![false; expectations.len()],
            expectations,
            attributes_match: true,
            effective_by_rule: BTreeMap::new(),
            emitted_by_rule: BTreeMap::new(),
        }
    }

    pub(crate) fn record_checkpointed_node(
        &mut self,
        node_index: usize,
        source_owns_fill: bool,
        emitted_fill: Option<(usize, &str)>,
    ) {
        let Some(checkpointed) = self.checkpointed_nodes.get_mut(node_index) else {
            self.attributes_match = false;
            return;
        };
        if *checkpointed {
            self.attributes_match = false;
            return;
        }
        *checkpointed = true;
        let expected = self.expectations.get(node_index).and_then(Option::as_ref);
        self.attributes_match &= record_fill_checkpoint(
            expected,
            source_owns_fill,
            emitted_fill,
            &mut self.effective_by_rule,
            &mut self.emitted_by_rule,
        );
    }

    fn proves_complete(&self) -> bool {
        self.attributes_match && self.checkpointed_nodes.iter().all(|entry| *entry)
    }

    fn has_effective_rule(&self, rule_index: usize) -> bool {
        self.effective_by_rule
            .get(&rule_index)
            .copied()
            .unwrap_or(0)
            != 0
    }

    fn proves_rule(&self, rule_index: usize) -> bool {
        self.proves_complete() && self.emitted_by_rule.get(&rule_index).copied().unwrap_or(0) != 0
    }
}

fn record_fill_checkpoint(
    expected: Option<&ExpectedFill>,
    source_owns_fill: bool,
    emitted_fill: Option<(usize, &str)>,
    effective_by_rule: &mut BTreeMap<usize, usize>,
    emitted_by_rule: &mut BTreeMap<usize, usize>,
) -> bool {
    if source_owns_fill {
        return emitted_fill.is_none();
    }
    match (expected, emitted_fill) {
        (None, None) => true,
        (Some(expected), Some((rule_index, css))) => {
            *effective_by_rule.entry(expected.rule_index).or_default() += 1;
            let matches = expected.rule_index == rule_index && expected.css == css;
            if matches {
                *emitted_by_rule.entry(rule_index).or_default() += 1;
            }
            matches
        }
        (Some(expected), None) => {
            *effective_by_rule.entry(expected.rule_index).or_default() += 1;
            false
        }
        (None, Some(_)) => false,
    }
}

fn paint_capability_from_css(css: &str) -> ThemeCapability {
    if css == "transparent" {
        ThemeCapability::TransparentPaint
    } else {
        ThemeCapability::SolidPaint
    }
}

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeStyle,
    Specified, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::ErEntity;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedPaint {
    rule_index: usize,
    css: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct EntityExpectation {
    fill: Option<ExpectedPaint>,
    stroke: Option<ExpectedPaint>,
}

/// ER entity paint resolved once for the semantic entity set and shared by layout, SVG, and
/// terminal theme evidence.
#[derive(Debug)]
pub(crate) struct ErEntityThemePlan {
    entity_indices: BTreeMap<String, usize>,
    expectations: Vec<EntityExpectation>,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, BTreeSet<ThemeCapability>>,
    terminal_receipt: OnceLock<ErEntityThemeReceipt>,
}

impl ErEntityThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::MermaidConfig,
        entities: &std::collections::BTreeMap<String, ErEntity>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let entity_indices = entities
            .values()
            .enumerate()
            .map(|(index, entity)| (entity.id.clone(), index))
            .collect::<BTreeMap<_, _>>();
        let entity_count = entities.len();
        let expectations = vec![EntityExpectation::default(); entity_count];
        let Some(theme) = theme else {
            return Ok(Self {
                entity_indices,
                expectations,
                evidence: FamilyThemeEvidence::default(),
                pending: BTreeMap::new(),
                terminal_receipt: OnceLock::new(),
            });
        };

        let mermaid_owns_fill = mermaid_owns_entity_fill(effective_config);
        let mermaid_owns_stroke = mermaid_owns_entity_stroke(effective_config);
        let mut expectations = expectations;
        let mut winner_properties = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut expected_capabilities =
            BTreeMap::<(usize, ResolvedStyleProperty), ThemeCapability>::new();

        for (entity_index, _) in entities.values().enumerate() {
            let style = theme.style_with_work_meter(
                ThemeTarget::Entity,
                ThemeVariant::Default,
                Some(entity_index + 1),
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
                expectations[entity_index].fill = Some(expected);
            }
            if let Some(expected) = typed_stroke_expectation(theme, &style, mermaid_owns_stroke) {
                expected_capabilities.insert(
                    (expected.rule_index, ResolvedStyleProperty::Stroke),
                    paint_capability_from_css(&expected.css),
                );
                expectations[entity_index].stroke = Some(expected);
            }
        }

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<usize, EntityRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Entity,
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
                        )
                        | (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeRuleFacet::Stroke(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) => {
                            if let Some(capability) = expected_capabilities
                                .get(&(rule_index, resolved_style_property_for_facet(facet)))
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
                        (FamilyThemeDisposition::TypedAdapter, _) => {
                            observation.incomplete = true;
                        }
                        (FamilyThemeDisposition::LegacyCompatibility, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Entity,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if entity_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Entity,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if entity_count == 0 {
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
                target: ThemeTarget::Entity,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Strict completion remains fail-closed until the writer proves every winner.
            } else if !observation.capabilities.is_empty() {
                pending.insert(key, observation.capabilities);
            } else if observation.suppressed {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            entity_indices,
            expectations,
            evidence,
            pending,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn index_for_entity_id(&self, entity_id: &str) -> Option<usize> {
        self.entity_indices.get(entity_id).copied()
    }

    pub(crate) fn typed_fill(
        &self,
        entity_index: usize,
        source_owns_fill: bool,
    ) -> Option<(usize, &str)> {
        (!source_owns_fill)
            .then(|| self.expectations.get(entity_index)?.fill.as_ref())
            .flatten()
            .map(|expected| (expected.rule_index, expected.css.as_str()))
    }

    pub(crate) fn typed_stroke(
        &self,
        entity_index: usize,
        source_owns_stroke: bool,
    ) -> Option<(usize, &str)> {
        (!source_owns_stroke)
            .then(|| self.expectations.get(entity_index)?.stroke.as_ref())
            .flatten()
            .map(|expected| (expected.rule_index, expected.css.as_str()))
    }

    pub(crate) fn begin_terminal_receipt(&self) -> ErEntityThemeReceipt {
        ErEntityThemeReceipt::new(self.expectations.clone())
    }

    pub(crate) fn record_terminal(&self, receipt: ErEntityThemeReceipt) -> bool {
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
) -> Option<ExpectedPaint> {
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
        Specified::Value(crate::diagram_theme::CanvasPaint::Transparent) => {
            "transparent".to_string()
        }
        Specified::Value(crate::diagram_theme::CanvasPaint::Solid(color)) => color.as_css(),
        Specified::Unspecified
        | Specified::Clear
        | Specified::Value(crate::diagram_theme::CanvasPaint::LinearGradient(_))
        | Specified::Value(crate::diagram_theme::CanvasPaint::RadialGradient(_))
        | Specified::Value(crate::diagram_theme::CanvasPaint::Pattern(_)) => return None,
    };
    Some(ExpectedPaint {
        rule_index: origin.rule_index(),
        css,
    })
}

fn typed_stroke_expectation(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    mermaid_owns: bool,
) -> Option<ExpectedPaint> {
    if mermaid_owns {
        return None;
    }
    let origin = style.stroke_resolution().winner()?;
    let facet = FamilyThemeRuleFacet::stroke(style.stroke_resolution().specified())?;
    if theme.rule_facet_disposition(origin.rule_index(), facet)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    let css = match style.stroke_resolution().specified() {
        Specified::Value(crate::diagram_theme::CanvasPaint::Transparent) => "transparent".into(),
        Specified::Value(crate::diagram_theme::CanvasPaint::Solid(color)) => color.as_css(),
        Specified::Unspecified
        | Specified::Clear
        | Specified::Value(crate::diagram_theme::CanvasPaint::LinearGradient(_))
        | Specified::Value(crate::diagram_theme::CanvasPaint::RadialGradient(_))
        | Specified::Value(crate::diagram_theme::CanvasPaint::Pattern(_)) => return None,
    };
    Some(ExpectedPaint {
        rule_index: origin.rule_index(),
        css,
    })
}

fn mermaid_owns_entity_fill(config: &merman_core::MermaidConfig) -> bool {
    merman_core::__private::config_path_overrides_typed_default(config, "themeVariables.mainBkg")
        || matches!(
            config.get_str("theme"),
            Some("redux-color" | "redux-dark-color")
        )
}

fn mermaid_owns_entity_stroke(config: &merman_core::MermaidConfig) -> bool {
    merman_core::__private::config_path_overrides_typed_default(config, "themeVariables.nodeBorder")
        || matches!(
            config.get_str("theme"),
            Some("redux-color" | "redux-dark-color")
        )
}

#[derive(Debug, Default)]
struct EntityRuleObservation {
    applicable: bool,
    incomplete: bool,
    suppressed: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeSet<ThemeCapability>,
}

/// Writer-owned proof that every semantic ER entity reached its canonical SVG checkpoint and
/// that typed paint values survived source-style precedence at the final writer owner.
#[derive(Debug, Clone)]
pub(crate) struct ErEntityThemeReceipt {
    expectations: Vec<EntityExpectation>,
    checkpointed_entities: Vec<bool>,
    attributes_match: bool,
    effective_by_rule: BTreeMap<usize, usize>,
    emitted_by_rule: BTreeMap<usize, usize>,
}

impl ErEntityThemeReceipt {
    fn new(expectations: Vec<EntityExpectation>) -> Self {
        Self {
            checkpointed_entities: vec![false; expectations.len()],
            expectations,
            attributes_match: true,
            effective_by_rule: BTreeMap::new(),
            emitted_by_rule: BTreeMap::new(),
        }
    }

    pub(crate) fn record_checkpointed_entity(
        &mut self,
        entity_index: usize,
        source_owns_fill: bool,
        source_owns_stroke: bool,
        emitted_fill: Option<(usize, &str)>,
        emitted_stroke: Option<(usize, &str)>,
    ) {
        let Some(checkpointed) = self.checkpointed_entities.get_mut(entity_index) else {
            self.attributes_match = false;
            return;
        };
        if *checkpointed {
            self.attributes_match = false;
            return;
        }
        *checkpointed = true;
        let expectation = self
            .expectations
            .get(entity_index)
            .cloned()
            .unwrap_or_default();
        self.attributes_match &= record_paint_checkpoint(
            expectation.fill.as_ref(),
            source_owns_fill,
            emitted_fill,
            &mut self.effective_by_rule,
            &mut self.emitted_by_rule,
        );
        self.attributes_match &= record_paint_checkpoint(
            expectation.stroke.as_ref(),
            source_owns_stroke,
            emitted_stroke,
            &mut self.effective_by_rule,
            &mut self.emitted_by_rule,
        );
    }

    fn proves_complete(&self) -> bool {
        self.attributes_match && self.checkpointed_entities.iter().all(|entry| *entry)
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

fn record_paint_checkpoint(
    expected: Option<&ExpectedPaint>,
    source_owns: bool,
    emitted: Option<(usize, &str)>,
    effective_by_rule: &mut BTreeMap<usize, usize>,
    emitted_by_rule: &mut BTreeMap<usize, usize>,
) -> bool {
    if source_owns {
        return emitted.is_none();
    }
    match (expected, emitted) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_receipt_requires_every_terminal_checkpoint_and_exact_values() {
        let mut receipt = ErEntityThemeReceipt::new(vec![EntityExpectation {
            fill: Some(ExpectedPaint {
                rule_index: 2,
                css: "#123456".to_string(),
            }),
            stroke: None,
        }]);
        receipt.record_checkpointed_entity(0, false, false, None, None);
        assert!(!receipt.proves_complete());

        let mut receipt = ErEntityThemeReceipt::new(vec![EntityExpectation {
            fill: Some(ExpectedPaint {
                rule_index: 2,
                css: "#123456".to_string(),
            }),
            stroke: None,
        }]);
        receipt.record_checkpointed_entity(0, false, false, Some((2, "#123456")), None);
        assert!(receipt.proves_complete());
        assert!(receipt.has_effective_rule(2));
        assert!(receipt.proves_rule(2));
    }

    #[test]
    fn source_owned_paint_is_not_an_effective_typed_route() {
        let mut receipt = ErEntityThemeReceipt::new(vec![EntityExpectation {
            fill: Some(ExpectedPaint {
                rule_index: 2,
                css: "#123456".to_string(),
            }),
            stroke: None,
        }]);
        receipt.record_checkpointed_entity(0, true, false, None, None);

        assert!(receipt.proves_complete());
        assert!(!receipt.has_effective_rule(2));
        assert!(!receipt.proves_rule(2));
    }

    #[test]
    fn entity_receipt_rejects_duplicate_and_wrong_paint_checkpoint() {
        let mut receipt = ErEntityThemeReceipt::new(vec![EntityExpectation {
            fill: Some(ExpectedPaint {
                rule_index: 2,
                css: "#123456".to_string(),
            }),
            stroke: None,
        }]);
        receipt.record_checkpointed_entity(0, false, false, Some((2, "#abcdef")), None);
        receipt.record_checkpointed_entity(0, false, false, Some((2, "#123456")), None);
        assert!(!receipt.proves_complete());
    }
}

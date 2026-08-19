use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty,
    ResolvedThemeStyle, Specified, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::ErEntity;

mod terminal;

use terminal::{EntityExpectation, ExpectedPaint};
pub(crate) use terminal::{ErEntityThemeReceipt, ErRelationTerminalExpectation};

/// ER terminal paint resolved once for the semantic model and shared by layout, SVG, and terminal
/// theme evidence.
#[derive(Debug)]
pub(crate) struct ErEntityThemePlan {
    entity_indices: BTreeMap<String, usize>,
    expectations: Vec<EntityExpectation>,
    relation_stroke: Option<ExpectedPaint>,
    relation_stroke_source_owned: bool,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, BTreeSet<ThemeCapability>>,
    terminal_receipt: OnceLock<ErEntityThemeReceipt>,
}

impl ErEntityThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::MermaidConfig,
        entities: &std::collections::BTreeMap<String, ErEntity>,
        relation_count: usize,
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
                relation_stroke: None,
                relation_stroke_source_owned: false,
                evidence: FamilyThemeEvidence::default(),
                pending: BTreeMap::new(),
                terminal_receipt: OnceLock::new(),
            });
        };

        let mermaid_owns_fill = mermaid_owns_entity_fill(effective_config);
        let mermaid_owns_stroke = mermaid_owns_entity_stroke(effective_config);
        let relation_stroke_source_owned = mermaid_owns_relation_stroke(effective_config);
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

        let relation_style = theme.style_with_work_meter(
            ThemeTarget::Relation,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let relation_static_winners = relation_style
            .winner_rule_properties()
            .into_iter()
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect::<BTreeSet<_>>();
        let relation_stroke =
            typed_relation_stroke_expectation(theme, &relation_style, relation_stroke_source_owned);
        if let Some(expected) = &relation_stroke {
            expected_capabilities.insert(
                (expected.rule_index, ResolvedStyleProperty::Stroke),
                paint_capability_from_css(&expected.css),
            );
        }
        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<(usize, ThemeTarget), ErRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: target @ (ThemeTarget::Entity | ThemeTarget::Relation),
                    selector,
                    facet,
                } => {
                    let occurrence_count = match target {
                        ThemeTarget::Entity => entity_count,
                        ThemeTarget::Relation => relation_count,
                        _ => unreachable!("guarded ER theme target"),
                    };
                    let observation = observations.entry((rule_index, target)).or_default();
                    if !selector.ordinal_domain_intersects_occurrence_count(occurrence_count) {
                        continue;
                    }
                    if target == ThemeTarget::Relation
                        && selector != (FamilyThemeSelectorShape::Static { variant: None })
                    {
                        observation.applicable = true;
                        match route.disposition() {
                            FamilyThemeDisposition::Unsupported => {
                                observation
                                    .residual
                                    .get_or_insert(unsupported_residual_for_facet(facet));
                            }
                            FamilyThemeDisposition::TypedAdapter
                            | FamilyThemeDisposition::LegacyCompatibility => {
                                observation.incomplete = true;
                            }
                        }
                        continue;
                    }
                    let property = resolved_style_property_for_facet(facet);
                    let route_won = match (target, selector) {
                        (ThemeTarget::Entity, _) => {
                            winner_properties.contains(&(rule_index, property))
                        }
                        (
                            ThemeTarget::Relation,
                            FamilyThemeSelectorShape::Static { variant: None },
                        ) => relation_static_winners.contains(&(rule_index, property)),
                        (ThemeTarget::Relation, _) => false,
                        _ => unreachable!("guarded ER theme target"),
                    };
                    if !route_won {
                        continue;
                    }
                    observation.applicable = true;
                    if target == ThemeTarget::Relation
                        && property == ResolvedStyleProperty::Stroke
                        && relation_stroke_source_owned
                    {
                        observation.suppressed = true;
                        continue;
                    }
                    match (route.disposition(), target, selector, facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            ThemeTarget::Entity,
                            _,
                            FamilyThemeRuleFacet::Fill(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        )
                        | (
                            FamilyThemeDisposition::TypedAdapter,
                            ThemeTarget::Entity,
                            _,
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
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            ThemeTarget::Relation,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Stroke(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) if relation_stroke
                            .as_ref()
                            .is_some_and(|stroke| stroke.rule_index == rule_index) =>
                        {
                            observation.capabilities.insert(paint_capability_from_css(
                                &relation_stroke
                                    .as_ref()
                                    .expect("guarded relation stroke")
                                    .css,
                            ));
                        }
                        (FamilyThemeDisposition::Unsupported, _, _, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::TypedAdapter, _, _, _) => {
                            observation.incomplete = true;
                        }
                        (FamilyThemeDisposition::LegacyCompatibility, _, _, _) => {
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
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Relation,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if relation_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Relation,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if relation_count == 0 {
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
        for ((rule_index, target), observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target,
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
            relation_stroke,
            relation_stroke_source_owned,
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

    pub(crate) fn typed_relation_stroke(&self) -> Option<(usize, &str)> {
        self.relation_stroke
            .as_ref()
            .map(|expected| (expected.rule_index, expected.css.as_str()))
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        relation_terminals: Vec<ErRelationTerminalExpectation>,
    ) -> ErEntityThemeReceipt {
        ErEntityThemeReceipt::new(self.expectations.clone()).with_relation_terminals(
            self.relation_stroke.clone(),
            self.relation_stroke_source_owned,
            relation_terminals,
        )
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

fn typed_relation_stroke_expectation(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    mermaid_owns: bool,
) -> Option<ExpectedPaint> {
    if mermaid_owns {
        return None;
    }
    let origin = style.stroke_resolution().winner()?;
    let rule = theme
        .family_rules()
        .find_map(|(index, rule)| (index == origin.rule_index()).then_some(rule))?;
    let facet = FamilyThemeRuleFacet::stroke(style.stroke_resolution().specified())?;
    if rule.target() != ThemeTarget::Relation
        || rule.variant().is_some()
        || rule.ordinal().is_some()
        || theme.rule_facet_disposition(origin.rule_index(), facet)
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

fn mermaid_owns_relation_stroke(config: &merman_core::MermaidConfig) -> bool {
    merman_core::__private::config_path_overrides_typed_default(config, "themeVariables.lineColor")
}

#[derive(Debug, Default)]
struct ErRuleObservation {
    applicable: bool,
    incomplete: bool,
    suppressed: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeSet<ThemeCapability>,
}

fn paint_capability_from_css(css: &str) -> ThemeCapability {
    if css == "transparent" {
        ThemeCapability::TransparentPaint
    } else {
        ThemeCapability::SolidPaint
    }
}

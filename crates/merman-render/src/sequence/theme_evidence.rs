use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemePaintKind,
    FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedThemeStyle,
    ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason};

#[derive(Debug, Clone, Copy, Default)]
struct SequenceThemeEvidenceState {
    actor_count: usize,
    actor_fill_emitted: bool,
    actor_fill_unhandled: bool,
    actor_fill_overridden: bool,
    actor_stroke_emitted: bool,
    actor_stroke_unhandled: bool,
    actor_stroke_overridden: bool,
}

#[derive(Debug, Default)]
struct SequenceRuleObservation {
    applicable: bool,
    incomplete: bool,
    capabilities: BTreeSet<ThemeCapability>,
    residual: Option<FamilyThemeResidualReason>,
}

/// Records actual Sequence actor emission so program routes alone cannot manufacture evidence.
#[derive(Debug, Default)]
pub(crate) struct SequenceThemeEvidenceRecorder {
    state: Mutex<SequenceThemeEvidenceState>,
}

impl SequenceThemeEvidenceRecorder {
    pub(crate) fn record_actor_emission(
        &self,
        actor_count: usize,
        actor_fill_emitted: bool,
        actor_fill_unhandled: bool,
        actor_fill_overridden: bool,
        actor_stroke_emitted: bool,
        actor_stroke_unhandled: bool,
        actor_stroke_overridden: bool,
    ) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.actor_count = state.actor_count.max(actor_count);
        state.actor_fill_emitted |= actor_count != 0 && actor_fill_emitted;
        state.actor_fill_unhandled |= actor_count != 0 && actor_fill_unhandled;
        state.actor_fill_overridden |= actor_count != 0 && actor_fill_overridden;
        state.actor_stroke_emitted |= actor_count != 0 && actor_stroke_emitted;
        state.actor_stroke_unhandled |= actor_count != 0 && actor_stroke_unhandled;
        state.actor_stroke_overridden |= actor_count != 0 && actor_stroke_overridden;
    }

    pub(crate) fn finish(&self, theme: Option<&ResolvedDiagramTheme>) -> FamilyThemeEvidence {
        let mut evidence = FamilyThemeEvidence::from_theme(theme);
        let state = *self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(theme) = theme else {
            return evidence;
        };

        let default_actor_style = theme.style(ThemeTarget::Actor, ThemeVariant::Default, None);
        let mut rules = BTreeMap::<usize, SequenceRuleObservation>::new();

        // Seed every Actor rule before checking winners. This keeps superseded rules, unmatched
        // ordinal selectors, and empty actor models in the evidence ledger so the final pass can
        // classify them as explicitly not applicable instead of leaving required keys unaccounted.
        for route in theme.family_mechanism_routes().iter().copied() {
            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Actor,
                ..
            } = route.mechanism()
            {
                rules.entry(rule_index).or_default();
            }
        }

        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::BaseTypography(_) => {
                    let key = theme.family_mechanism_key(route);
                    match route.disposition() {
                        FamilyThemeDisposition::Unsupported => {
                            if state.actor_count == 0 {
                                evidence.mark_not_applicable(key);
                            } else {
                                evidence.mark_residual(
                                    key,
                                    FamilyThemeResidualReason::UnsupportedTypography,
                                );
                            }
                        }
                        FamilyThemeDisposition::TypedAdapter => {
                            // No Sequence base-typography route is direct yet. Leaving a future
                            // route unaccounted fails closed instead of guessing consumption.
                        }
                        FamilyThemeDisposition::LegacyCompatibility => {}
                    }
                }
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Actor,
                    selector,
                    facet,
                } => {
                    let observation = rules.entry(rule_index).or_default();
                    if !route_matches_actual_actor(
                        theme,
                        rule_index,
                        selector,
                        facet,
                        state.actor_count,
                    ) {
                        continue;
                    }
                    observation.applicable = true;
                    if state.actor_fill_overridden && matches!(facet, FamilyThemeRuleFacet::Fill(_))
                    {
                        continue;
                    }
                    if state.actor_stroke_overridden
                        && matches!(facet, FamilyThemeRuleFacet::Stroke(_))
                    {
                        continue;
                    }
                    match route.disposition() {
                        FamilyThemeDisposition::TypedAdapter
                            if matches!(
                                facet,
                                FamilyThemeRuleFacet::Fill(
                                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                                )
                            ) =>
                        {
                            if state.actor_fill_unhandled {
                                observation
                                    .residual
                                    .get_or_insert(FamilyThemeResidualReason::UnsupportedPaint);
                                continue;
                            }
                            if !state.actor_fill_emitted {
                                continue;
                            }
                            match default_actor_style.fill() {
                                Some(CanvasPaint::Transparent) => {
                                    observation
                                        .capabilities
                                        .insert(ThemeCapability::TransparentPaint);
                                }
                                Some(CanvasPaint::Solid(_)) => {
                                    observation.capabilities.insert(ThemeCapability::SolidPaint);
                                }
                                _ => {
                                    observation
                                        .residual
                                        .get_or_insert(FamilyThemeResidualReason::UnsupportedPaint);
                                }
                            }
                        }
                        FamilyThemeDisposition::TypedAdapter
                            if matches!(
                                facet,
                                FamilyThemeRuleFacet::Stroke(
                                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                                )
                            ) =>
                        {
                            if state.actor_stroke_unhandled {
                                observation
                                    .residual
                                    .get_or_insert(FamilyThemeResidualReason::UnsupportedPaint);
                                continue;
                            }
                            if !state.actor_stroke_emitted {
                                continue;
                            }
                            match default_actor_style.stroke() {
                                Some(CanvasPaint::Transparent) => {
                                    observation
                                        .capabilities
                                        .insert(ThemeCapability::TransparentPaint);
                                }
                                Some(CanvasPaint::Solid(_)) => {
                                    observation.capabilities.insert(ThemeCapability::SolidPaint);
                                }
                                _ => {
                                    observation
                                        .residual
                                        .get_or_insert(FamilyThemeResidualReason::UnsupportedPaint);
                                }
                            }
                        }
                        FamilyThemeDisposition::TypedAdapter => observation.incomplete = true,
                        FamilyThemeDisposition::Unsupported => {
                            observation
                                .residual
                                .get_or_insert(unsupported_reason_for_facet(facet));
                        }
                        FamilyThemeDisposition::LegacyCompatibility => {}
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Actor,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if state.actor_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if matches!(route.disposition(), FamilyThemeDisposition::Unsupported) {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Actor,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if state.actor_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if !matches!(
                        route.disposition(),
                        FamilyThemeDisposition::LegacyCompatibility
                    ) {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    }
                }
                FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {
                    // This narrow adapter owns only Actor evidence. Other Sequence targets stay
                    // unaccounted unless the compatibility lane rejects them first.
                }
            }
        }

        for (rule_index, observation) in rules {
            let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Actor,
            };
            if state.actor_count == 0 || !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // A future direct facet reached the actor but has no terminal receipt yet.
            } else if observation.capabilities.is_empty() {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_applied_with_capabilities(key, observation.capabilities);
            }
        }
        evidence
    }
}

fn route_matches_actual_actor(
    theme: &ResolvedDiagramTheme,
    rule_index: usize,
    selector: FamilyThemeSelectorShape,
    facet: FamilyThemeRuleFacet,
    actor_count: usize,
) -> bool {
    if actor_count == 0 {
        return false;
    }
    match selector {
        FamilyThemeSelectorShape::Static {
            variant: None | Some(ThemeVariant::Default),
        } => {
            facet_winner(
                &theme.style(ThemeTarget::Actor, ThemeVariant::Default, None),
                facet,
            ) == Some(rule_index)
        }
        FamilyThemeSelectorShape::Ordinal {
            variant: None | Some(ThemeVariant::Default),
            selector,
        } => (1..=actor_count).any(|ordinal| {
            selector.matches(ordinal)
                && facet_winner(
                    &theme.style(ThemeTarget::Actor, ThemeVariant::Default, Some(ordinal)),
                    facet,
                ) == Some(rule_index)
        }),
        FamilyThemeSelectorShape::Static { .. } | FamilyThemeSelectorShape::Ordinal { .. } => false,
    }
}

fn facet_winner(style: &ResolvedThemeStyle, facet: FamilyThemeRuleFacet) -> Option<usize> {
    let winner = match facet {
        FamilyThemeRuleFacet::Fill(_) => style.fill_resolution().winner(),
        FamilyThemeRuleFacet::Stroke(_) => style.stroke_resolution().winner(),
        FamilyThemeRuleFacet::StrokeWidth => style.stroke_width_resolution().winner(),
        FamilyThemeRuleFacet::StrokeDasharray => style.stroke_dasharray_resolution().winner(),
        FamilyThemeRuleFacet::StrokeLinecap => style.stroke_linecap_resolution().winner(),
        FamilyThemeRuleFacet::StrokeLinejoin => style.stroke_linejoin_resolution().winner(),
        FamilyThemeRuleFacet::Opacity => style.opacity_resolution().winner(),
        FamilyThemeRuleFacet::FillOpacity => style.fill_opacity_resolution().winner(),
        FamilyThemeRuleFacet::StrokeOpacity => style.stroke_opacity_resolution().winner(),
        FamilyThemeRuleFacet::Radius => style.radius_resolution().winner(),
        FamilyThemeRuleFacet::Padding => style.padding_resolution().winner(),
        FamilyThemeRuleFacet::Typography(property) => {
            style.typography_resolution().winner(property)
        }
        FamilyThemeRuleFacet::Effect => style.effect_resolution().winner(),
    };
    winner.map(|origin| origin.rule_index())
}

fn unsupported_reason_for_facet(facet: FamilyThemeRuleFacet) -> FamilyThemeResidualReason {
    match facet {
        FamilyThemeRuleFacet::Typography(_) => FamilyThemeResidualReason::UnsupportedTypography,
        FamilyThemeRuleFacet::Effect => FamilyThemeResidualReason::UnsupportedEffect,
        FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_) => {
            FamilyThemeResidualReason::UnsupportedPaint
        }
        FamilyThemeRuleFacet::StrokeWidth
        | FamilyThemeRuleFacet::StrokeDasharray
        | FamilyThemeRuleFacet::StrokeLinecap
        | FamilyThemeRuleFacet::StrokeLinejoin
        | FamilyThemeRuleFacet::Opacity
        | FamilyThemeRuleFacet::FillOpacity
        | FamilyThemeRuleFacet::StrokeOpacity
        | FamilyThemeRuleFacet::Radius
        | FamilyThemeRuleFacet::Padding => FamilyThemeResidualReason::UnsupportedGeometry,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
        ThemeStylePatch,
    };
    use crate::render_family::RenderFamilyKind;

    #[test]
    fn actor_rule_is_not_applicable_when_terminal_model_has_no_actors() {
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
        let resolved = theme.resolve(RenderFamilyKind::Sequence);
        let evidence = SequenceThemeEvidenceRecorder::default().finish(Some(&resolved));

        assert_eq!(
            evidence.not_applicable_mechanisms(),
            [crate::diagram_theme::FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Actor,
            }]
        );
        assert!(evidence.applied().is_empty());
        assert!(evidence.residuals().is_empty());
    }
}

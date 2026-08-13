use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemePaintKind, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeStyle,
    ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason};

#[derive(Debug, Clone, Default)]
struct SequenceThemeEvidenceState {
    actor_count: usize,
    actor_fill_emitted: bool,
    actor_fill_unhandled: bool,
    actor_fill_overridden: bool,
    actor_stroke_emitted: bool,
    actor_stroke_unhandled: bool,
    actor_stroke_overridden: bool,
    actor_style_receipt: SequenceActorThemeReceipt,
}

/// Winner facts produced by the terminal Sequence Actor writer.
///
/// Static and ordinal winners remain separate because a static rule is resolved against the
/// family default while ordinal selectors are evaluated against each concrete one-based actor
/// ordinal. The writer resolves each ordinal exactly once and records every facet winner from
/// that result; evidence finalization only consumes these facts and performs no selector scan.
#[derive(Debug, Clone, Default)]
pub(crate) struct SequenceActorThemeReceipt {
    static_winners: BTreeSet<(usize, ResolvedStyleProperty)>,
    ordinal_winners: BTreeSet<(usize, ResolvedStyleProperty)>,
}

impl SequenceActorThemeReceipt {
    pub(crate) fn record_static_style(&mut self, style: &ResolvedThemeStyle) {
        record_style_winners(&mut self.static_winners, style);
    }

    pub(crate) fn record_ordinal_style(&mut self, style: &ResolvedThemeStyle) {
        record_style_winners(&mut self.ordinal_winners, style);
    }

    fn merge(&mut self, other: Self) {
        self.static_winners.extend(other.static_winners);
        self.ordinal_winners.extend(other.ordinal_winners);
    }

    fn route_won(
        &self,
        rule_index: usize,
        selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        let property = style_property_for_facet(facet);
        match selector {
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default),
            } => self.static_winners.contains(&(rule_index, property)),
            FamilyThemeSelectorShape::Ordinal {
                variant: None | Some(ThemeVariant::Default),
                ..
            } => self.ordinal_winners.contains(&(rule_index, property)),
            FamilyThemeSelectorShape::Static { .. } | FamilyThemeSelectorShape::Ordinal { .. } => {
                false
            }
        }
    }
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
        actor_style_receipt: SequenceActorThemeReceipt,
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
        state.actor_style_receipt.merge(actor_style_receipt);
    }

    pub(crate) fn finish(&self, theme: Option<&ResolvedDiagramTheme>) -> FamilyThemeEvidence {
        let mut evidence = FamilyThemeEvidence::from_theme(theme);
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let Some(theme) = theme else {
            return evidence;
        };

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
                    if state.actor_count == 0
                        || !state
                            .actor_style_receipt
                            .route_won(rule_index, selector, facet)
                    {
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
                            match facet {
                                FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Transparent) => {
                                    observation
                                        .capabilities
                                        .insert(ThemeCapability::TransparentPaint);
                                }
                                FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid) => {
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
                            match facet {
                                FamilyThemeRuleFacet::Stroke(FamilyThemePaintKind::Transparent) => {
                                    observation
                                        .capabilities
                                        .insert(ThemeCapability::TransparentPaint);
                                }
                                FamilyThemeRuleFacet::Stroke(FamilyThemePaintKind::Solid) => {
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

fn record_style_winners(
    winners: &mut BTreeSet<(usize, ResolvedStyleProperty)>,
    style: &ResolvedThemeStyle,
) {
    winners.extend(
        style
            .winner_rule_properties()
            .into_iter()
            .map(|(property, origin)| (origin.rule_index(), property)),
    );
}

const fn style_property_for_facet(facet: FamilyThemeRuleFacet) -> ResolvedStyleProperty {
    match facet {
        FamilyThemeRuleFacet::Fill(_) => ResolvedStyleProperty::Fill,
        FamilyThemeRuleFacet::Stroke(_) => ResolvedStyleProperty::Stroke,
        FamilyThemeRuleFacet::StrokeWidth => ResolvedStyleProperty::StrokeWidth,
        FamilyThemeRuleFacet::StrokeDasharray => ResolvedStyleProperty::StrokeDasharray,
        FamilyThemeRuleFacet::StrokeLinecap => ResolvedStyleProperty::StrokeLinecap,
        FamilyThemeRuleFacet::StrokeLinejoin => ResolvedStyleProperty::StrokeLinejoin,
        FamilyThemeRuleFacet::Opacity => ResolvedStyleProperty::Opacity,
        FamilyThemeRuleFacet::FillOpacity => ResolvedStyleProperty::FillOpacity,
        FamilyThemeRuleFacet::StrokeOpacity => ResolvedStyleProperty::StrokeOpacity,
        FamilyThemeRuleFacet::Radius => ResolvedStyleProperty::Radius,
        FamilyThemeRuleFacet::Padding => ResolvedStyleProperty::Padding,
        FamilyThemeRuleFacet::Typography(property) => ResolvedStyleProperty::Typography(property),
        FamilyThemeRuleFacet::Effect => ResolvedStyleProperty::Effect,
    }
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
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
        ThemeStylePatch,
    };

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
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .expect("compile Sequence actor theme");
        let resolved = theme.resolve(DiagramFamilyId::SEQUENCE);
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

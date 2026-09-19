use std::collections::BTreeMap;
use std::sync::Mutex;

use super::observation::{
    SequenceRuleObservation, observe_control_rule, observe_lifeline_rule, observe_message_rule,
    observe_sequence_number_rule, observe_static_rect_rule, observe_typography_rule,
};
use super::receipts::{
    SequenceActorThemeReceipt, SequenceControlThemeEmission, SequenceControlThemeState,
    SequenceLifelineThemeEmission, SequenceMessageThemeEmission, SequenceNumberLabelThemeEmission,
    SequenceNumberLabelThemeState, SequenceStaticRectThemeEmission, SequenceStaticRectThemeState,
    SequenceThemeEvidenceState, SequenceTypographyThemeReceipt,
};
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemePaintKind, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    unsupported_residual_for_facet as unsupported_reason_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

/// Records actual Sequence writer emission so program routes alone cannot manufacture evidence.
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

    pub(crate) fn record_note_emission(&self, emission: SequenceStaticRectThemeEmission) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.note.merge(emission);
    }

    pub(crate) fn record_lifeline_emission(&self, emission: SequenceLifelineThemeEmission) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.lifeline.merge(emission);
    }

    pub(crate) fn record_message_emission(&self, emission: SequenceMessageThemeEmission) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.message.merge(emission);
    }

    pub(crate) fn record_sequence_number_emission(
        &self,
        emission: SequenceNumberLabelThemeEmission,
    ) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.sequence_number.merge(emission);
    }

    pub(crate) fn record_control_emission(
        &self,
        target: ThemeTarget,
        emission: SequenceControlThemeEmission,
    ) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if target == ThemeTarget::Loop {
            state.loop_frame.merge(emission);
        } else {
            state.keyword_background.merge(emission);
        }
    }

    pub(crate) fn record_activation_emission(&self, emission: SequenceStaticRectThemeEmission) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.activation.merge(emission);
    }

    pub(crate) fn record_typography_emission(&self, receipt: SequenceTypographyThemeReceipt) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        debug_assert!(
            state.typography.is_none(),
            "Sequence role typography is sealed once per terminal SVG"
        );
        state.typography = Some(receipt);
    }

    /// Reconcile unsupported text requests only after the final SVG has completed.
    pub(crate) fn record_unsupported_text_emission(
        &self,
        theme: Option<&ResolvedDiagramTheme>,
        title_present: bool,
        work: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(());
        };
        if !theme.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Text | ThemeTarget::Title,
                    ..
                } | FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Text | ThemeTarget::Title
                } | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Text | ThemeTarget::Title,
                    ..
                }
            )
        }) {
            return Ok(());
        }

        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(typography) = state.typography.as_ref() else {
            return Ok(());
        };
        let title_count = usize::from(title_present);
        let text_count = typography
            .role_label_candidate_count()
            .saturating_add(state.sequence_number.receipt.text_candidates)
            .saturating_add(title_count);
        let number = &state.sequence_number;
        let number_fill_covered = number.receipt.text_candidates == 0
            || (number.fill_emitted
                && !number.fill_overridden
                && number
                    .receipt
                    .proves_typed_fill(number.receipt.stylesheet_fill.as_deref()));
        let text_fill_covered =
            title_count == 0 && typography.role_fills_cover_all_labels() && number_fill_covered;
        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[
                UnsupportedTerminalDomain::direct(
                    ThemeTarget::Text,
                    TerminalVariantDomain::uniform(text_count, ThemeVariant::Default),
                )
                .with_fill_fully_overridden(text_fill_covered),
                UnsupportedTerminalDomain::direct(
                    ThemeTarget::Title,
                    TerminalVariantDomain::uniform(title_count, ThemeVariant::Default),
                ),
            ],
            work,
        )?;
        state.unsupported_text = Some(evidence);
        Ok(())
    }

    #[cfg(merman_internal_theme_acceptance)]
    pub(crate) fn raster_paint_binding_fact(
        &self,
    ) -> Option<crate::theme_raster_paint::ThemeRasterPaintBindingFact> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !state.lifeline.paint_emitted || state.lifeline.paint_overridden {
            return None;
        }
        let selected_property = state.lifeline.selected_property?;
        crate::theme_raster_paint::ThemeRasterPaintBindingFact::new(
            crate::DiagramFamilyId::SEQUENCE,
            ThemeTarget::Lifeline,
            state
                .lifeline
                .receipt
                .raster_paint_selector(selected_property)?,
            state.lifeline.receipt.raster_paint_terminals()?,
        )
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

        let mut actor_rules = BTreeMap::<usize, SequenceRuleObservation>::new();
        let mut lifeline_rules = BTreeMap::<usize, SequenceRuleObservation>::new();
        let mut message_rules = BTreeMap::<usize, SequenceRuleObservation>::new();
        let mut sequence_number_rules = BTreeMap::<usize, SequenceRuleObservation>::new();
        let mut loop_rules =
            BTreeMap::<ThemeTarget, BTreeMap<usize, SequenceRuleObservation>>::new();
        let mut note_rules = BTreeMap::<usize, SequenceRuleObservation>::new();
        let mut activation_rules = BTreeMap::<usize, SequenceRuleObservation>::new();
        let mut typography_rules =
            BTreeMap::<ThemeTarget, BTreeMap<usize, SequenceRuleObservation>>::new();
        let base_typography_routes = theme
            .family_mechanism_routes()
            .iter()
            .copied()
            .filter_map(|route| match route.mechanism() {
                FamilyThemeMechanism::BaseTypography(property) => {
                    Some((property, route.disposition()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        // Seed every Actor rule and every static rule for directly owned surfaces before
        // checking winners, including rules with only unsupported facets.
        // This keeps superseded rules, unmatched ordinal selectors, and empty terminal models in
        // the ledger so the final pass can classify them as explicitly not applicable instead of
        // leaving required keys unaccounted.
        for route in theme.family_mechanism_routes().iter().copied() {
            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Actor,
                ..
            } = route.mechanism()
            {
                actor_rules.entry(rule_index).or_default();
            }
            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Lifeline,
                selector:
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default),
                    },
                ..
            } = route.mechanism()
            {
                lifeline_rules.entry(rule_index).or_default();
            }
            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Message,
                selector:
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default),
                    },
                ..
            } = route.mechanism()
            {
                message_rules.entry(rule_index).or_default();
            }
            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::SequenceNumberLabel,
                ..
            } = route.mechanism()
            {
                sequence_number_rules.entry(rule_index).or_default();
            }
            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: target @ (ThemeTarget::Loop | ThemeTarget::LoopLabelBackground),
                selector:
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default),
                    },
                ..
            } = route.mechanism()
            {
                loop_rules
                    .entry(target)
                    .or_default()
                    .entry(rule_index)
                    .or_default();
            }
            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Note,
                selector:
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default),
                    },
                ..
            } = route.mechanism()
            {
                note_rules.entry(rule_index).or_default();
            }
            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Activation,
                selector:
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default),
                    },
                ..
            } = route.mechanism()
            {
                activation_rules.entry(rule_index).or_default();
            }
            if let FamilyThemeMechanism::RuleFacet {
                rule_index, target, ..
            } = route.mechanism()
                && crate::sequence::SequenceTypographyRole::from_target(target).is_some()
            {
                typography_rules
                    .entry(target)
                    .or_default()
                    .entry(rule_index)
                    .or_default();
            }
        }

        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::BaseTypography(_) => {}
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target:
                        target @ (ThemeTarget::ActorLabel
                        | ThemeTarget::MessageLabel
                        | ThemeTarget::NoteLabel
                        | ThemeTarget::LoopLabel),
                    selector,
                    facet,
                } => {
                    let Some(role) = crate::sequence::SequenceTypographyRole::from_target(target)
                    else {
                        continue;
                    };
                    let Some(receipt) = state.typography.as_ref().map(|receipt| receipt.role(role))
                    else {
                        continue;
                    };
                    let Some(observation) = typography_rules
                        .get_mut(&target)
                        .and_then(|rules| rules.get_mut(&rule_index))
                    else {
                        continue;
                    };
                    observe_typography_rule(
                        receipt,
                        observation,
                        route.disposition(),
                        rule_index,
                        selector,
                        facet,
                    );
                }
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Lifeline,
                    selector,
                    facet,
                } if matches!(
                    selector,
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default)
                    }
                ) =>
                {
                    let Some(observation) = lifeline_rules.get_mut(&rule_index) else {
                        continue;
                    };
                    observe_lifeline_rule(
                        &state.lifeline,
                        observation,
                        route.disposition(),
                        rule_index,
                        selector,
                        facet,
                    );
                }
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Message,
                    selector,
                    facet,
                } if matches!(
                    selector,
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default)
                    }
                ) =>
                {
                    let Some(observation) = message_rules.get_mut(&rule_index) else {
                        continue;
                    };
                    observe_message_rule(
                        &state.message,
                        observation,
                        route.disposition(),
                        rule_index,
                        selector,
                        facet,
                    );
                }
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::SequenceNumberLabel,
                    selector,
                    facet,
                } => {
                    let observation = sequence_number_rules.entry(rule_index).or_default();
                    observe_sequence_number_rule(
                        &state.sequence_number,
                        observation,
                        route.disposition(),
                        rule_index,
                        selector,
                        facet,
                    );
                }
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: target @ (ThemeTarget::Loop | ThemeTarget::LoopLabelBackground),
                    selector,
                    facet,
                } if matches!(
                    selector,
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default)
                    }
                ) =>
                {
                    let Some(observation) = loop_rules
                        .get_mut(&target)
                        .and_then(|rules| rules.get_mut(&rule_index))
                    else {
                        continue;
                    };
                    observe_control_rule(
                        if target == ThemeTarget::Loop {
                            &state.loop_frame
                        } else {
                            &state.keyword_background
                        },
                        observation,
                        route.disposition(),
                        rule_index,
                        selector,
                        facet,
                    );
                }
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Actor,
                    selector,
                    facet,
                } => {
                    let observation = actor_rules.entry(rule_index).or_default();
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
                    if state.actor_style_receipt.stroke_width_overridden
                        && facet == FamilyThemeRuleFacet::StrokeWidth
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
                        FamilyThemeDisposition::TypedAdapter
                            if matches!(
                                facet,
                                FamilyThemeRuleFacet::StrokeWidth | FamilyThemeRuleFacet::Radius
                            ) =>
                        {
                            if state.actor_style_receipt.geometry_unhandled.get() {
                                observation
                                    .residual
                                    .get_or_insert(FamilyThemeResidualReason::UnsupportedGeometry);
                            } else if !state.actor_style_receipt.geometry_complete() {
                                observation.incomplete = true;
                            } else {
                                observation.capabilities.insert(
                                    if facet == FamilyThemeRuleFacet::StrokeWidth {
                                        ThemeCapability::BorderStyling
                                    } else {
                                        ThemeCapability::RoundedGeometry
                                    },
                                );
                            }
                        }
                        FamilyThemeDisposition::TypedAdapter
                            if facet == FamilyThemeRuleFacet::Effect =>
                        {
                            let receipt = &state.actor_style_receipt;
                            if receipt.effect_unhandled || receipt.geometry_unhandled.get() {
                                observation
                                    .residual
                                    .get_or_insert(FamilyThemeResidualReason::UnsupportedEffect);
                            } else if !receipt.effect_complete() {
                                observation.incomplete = true;
                            } else {
                                observation
                                    .capabilities
                                    .insert(ThemeCapability::SemanticRules);
                                if !receipt.effect_cleared {
                                    observation.capabilities.extend([
                                        ThemeCapability::Shadow,
                                        ThemeCapability::SvgFilter,
                                    ]);
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
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: target @ (ThemeTarget::Note | ThemeTarget::Activation),
                    selector,
                    facet,
                } if matches!(
                    selector,
                    FamilyThemeSelectorShape::Static {
                        variant: None | Some(ThemeVariant::Default)
                    }
                ) =>
                {
                    let (surface, observation) = match target {
                        ThemeTarget::Note => {
                            (&state.note, note_rules.entry(rule_index).or_default())
                        }
                        ThemeTarget::Activation => (
                            &state.activation,
                            activation_rules.entry(rule_index).or_default(),
                        ),
                        _ => unreachable!("guarded by the static rectangle target pattern"),
                    };
                    observe_static_rect_rule(
                        surface,
                        observation,
                        route.disposition(),
                        rule_index,
                        selector,
                        facet,
                    );
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
                    target:
                        target @ (ThemeTarget::ActorLabel
                        | ThemeTarget::NoteLabel
                        | ThemeTarget::LoopLabel),
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if let Some(typography) = &state.typography {
                        let receipt = typography.role(
                            crate::sequence::SequenceTypographyRole::from_target(target)
                                .expect("matched text effect target"),
                        );
                        if receipt.label_candidate_count() == 0
                            || (receipt.effect_requested && !receipt.effect_binding_used)
                        {
                            evidence.mark_not_applicable(key);
                        } else if receipt.effect_unhandled {
                            evidence
                                .mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                        } else if receipt.effect_complete()
                            && receipt.effect_applications.get() == 0
                        {
                            evidence.mark_not_applicable(key);
                        } else if receipt.effect_complete() {
                            evidence.mark_applied_with_capabilities(
                                key,
                                [ThemeCapability::Shadow, ThemeCapability::SvgFilter],
                            );
                        }
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Actor,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    let receipt = &state.actor_style_receipt;
                    if state.actor_count == 0
                        || (receipt.effect_requested && !receipt.effect_binding_used)
                    {
                        evidence.mark_not_applicable(key);
                    } else if receipt.effect_unhandled || receipt.geometry_unhandled.get() {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    } else if receipt.effect_complete() {
                        evidence.mark_applied_with_capabilities(
                            key,
                            [ThemeCapability::Shadow, ThemeCapability::SvgFilter],
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: target @ (ThemeTarget::Loop | ThemeTarget::LoopLabelBackground),
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    let receipt = if target == ThemeTarget::Loop {
                        &state.loop_frame.receipt
                    } else {
                        &state.keyword_background.receipt
                    };
                    if receipt.surface_candidates.get() == 0
                        || (receipt.effect_requested && !receipt.effect_binding_used)
                    {
                        evidence.mark_not_applicable(key);
                    } else if receipt.effect_unhandled.get() {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    } else if receipt.effect_complete() {
                        evidence.mark_applied_with_capabilities(
                            key,
                            [ThemeCapability::Shadow, ThemeCapability::SvgFilter],
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Lifeline,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    let receipt = &state.lifeline.receipt;
                    if receipt.candidate_count() == 0
                        || (receipt.effect_requested && !receipt.effect_binding_used)
                    {
                        evidence.mark_not_applicable(key);
                    } else if receipt.effect_unhandled {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    } else if receipt.effect_complete() {
                        evidence.mark_applied_with_capabilities(
                            key,
                            [ThemeCapability::Shadow, ThemeCapability::SvgFilter],
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Message,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    let receipt = &state.message.receipt;
                    if receipt.candidate_count() == 0
                        || (receipt.effect_requested && !receipt.effect_binding_used)
                    {
                        evidence.mark_not_applicable(key);
                    } else if receipt.effect_unhandled {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    } else if receipt.effect_complete() {
                        evidence.mark_applied_with_capabilities(
                            key,
                            [ThemeCapability::Shadow, ThemeCapability::SvgFilter],
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: target @ (ThemeTarget::Note | ThemeTarget::Activation),
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    let surface = if target == ThemeTarget::Note {
                        &state.note
                    } else {
                        &state.activation
                    };
                    let receipt = &surface.receipt;
                    if surface.surface_count == 0
                        || (receipt.effect_requested && !receipt.effect_binding_used)
                    {
                        evidence.mark_not_applicable(key);
                    } else if receipt.effect_unhandled {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    } else if receipt.effect_complete(surface.surface_count) {
                        evidence.mark_applied_with_capabilities(
                            key,
                            [ThemeCapability::Shadow, ThemeCapability::SvgFilter],
                        );
                    }
                }
                FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {
                    // This narrow adapter owns Actor, unqualified static Lifeline paint, Message
                    // stroke, SequenceNumberLabel fill, Loop surface paint, and Note/Activation
                    // paint evidence. Message fill is observed only to account for its
                    // participation in the shared signalColor projection. Other Sequence targets
                    // and selector classes remain unaccounted unless compatibility rejects them.
                }
            }
        }

        for (rule_index, observation) in actor_rules {
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
        finish_line_rules(
            &mut evidence,
            ThemeTarget::Lifeline,
            state.lifeline.receipt.candidate_count(),
            lifeline_rules,
        );
        finish_line_rules(
            &mut evidence,
            ThemeTarget::Message,
            state.message.receipt.candidate_count(),
            message_rules,
        );
        finish_sequence_number_rules(&mut evidence, &state.sequence_number, sequence_number_rules);
        for (target, rules) in loop_rules {
            finish_control_rules(
                &mut evidence,
                target,
                if target == ThemeTarget::Loop {
                    &state.loop_frame
                } else {
                    &state.keyword_background
                },
                rules,
            );
        }
        finish_static_rect_rules(&mut evidence, ThemeTarget::Note, &state.note, note_rules);
        finish_static_rect_rules(
            &mut evidence,
            ThemeTarget::Activation,
            &state.activation,
            activation_rules,
        );
        finish_base_typography(
            &mut evidence,
            state.typography.as_ref(),
            &base_typography_routes,
        );
        finish_typography_rules(&mut evidence, state.typography.as_ref(), typography_rules);
        if let Some(unsupported_text) = state.unsupported_text {
            evidence.merge_accounted_from(unsupported_text);
        }
        evidence
    }
}

fn finish_base_typography(
    evidence: &mut FamilyThemeEvidence,
    receipt: Option<&SequenceTypographyThemeReceipt>,
    routes: &[(ThemeTypographyProperty, FamilyThemeDisposition)],
) {
    if routes.is_empty() {
        return;
    }
    let Some(receipt) = receipt else {
        return;
    };
    for (property, disposition) in routes {
        let key = crate::diagram_theme::FamilyThemeMechanismKey::Typography(*property);
        let relevant_occurrences = match property {
            ThemeTypographyProperty::FontStack | ThemeTypographyProperty::FontSize => {
                receipt.base_property_relevant_occurrences(*property)
            }
            ThemeTypographyProperty::FontWeight
            | ThemeTypographyProperty::FontStyle
            | ThemeTypographyProperty::LineHeight
            | ThemeTypographyProperty::LetterSpacing
            | ThemeTypographyProperty::WordSpacing
            | ThemeTypographyProperty::Transform
            | ThemeTypographyProperty::Decoration
            | ThemeTypographyProperty::TextAlign
            | ThemeTypographyProperty::WhiteSpace
            | ThemeTypographyProperty::Wrap => receipt.base_relevant_occurrences(),
        };
        match disposition {
            FamilyThemeDisposition::LegacyCompatibility => {}
            FamilyThemeDisposition::Unsupported => {
                if relevant_occurrences == 0 {
                    evidence.mark_not_applicable(key);
                } else {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                }
            }
            FamilyThemeDisposition::TypedAdapter => {
                if relevant_occurrences == 0 || !receipt.base.has_typed_property(*property) {
                    evidence.mark_not_applicable(key);
                } else if !receipt.base.terminal_svg_observed()
                    || receipt.base_property_incomplete(*property)
                {
                    // The route reached a visible surface, but the final SVG has not provided
                    // enough independent facts to promote it.
                } else if receipt.base_property_applied(*property)
                    && receipt.base.stylesheet_verified()
                {
                    evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
                } else {
                    // A visible typed route whose terminal evidence is missing or mismatched is
                    // not "not applicable". Keep it residual so strict portability cannot be
                    // satisfied by a false ownership claim.
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                }
            }
        }
    }
}

fn finish_typography_rules(
    evidence: &mut FamilyThemeEvidence,
    receipt: Option<&SequenceTypographyThemeReceipt>,
    rules_by_target: BTreeMap<ThemeTarget, BTreeMap<usize, SequenceRuleObservation>>,
) {
    for (target, rules) in rules_by_target {
        let role = crate::sequence::SequenceTypographyRole::from_target(target)
            .expect("Sequence typography rules are keyed only by role targets");
        let candidate_count =
            receipt.map_or(0, |receipt| receipt.role(role).label_candidate_count());
        for (rule_index, observation) in rules {
            let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target,
            };
            if candidate_count == 0 || !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // The rule reached a terminal label candidate without a complete writer seal.
            } else if observation.capabilities.is_empty() {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_applied_with_capabilities(key, observation.capabilities);
            }
        }
    }
}

fn finish_line_rules(
    evidence: &mut FamilyThemeEvidence,
    target: ThemeTarget,
    candidate_count: usize,
    rules: BTreeMap<usize, SequenceRuleObservation>,
) {
    for (rule_index, observation) in rules {
        let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule {
            index: rule_index,
            target,
        };
        if candidate_count == 0 || !observation.applicable {
            evidence.mark_not_applicable(key);
        } else if let Some(reason) = observation.residual {
            evidence.mark_residual(key, reason);
        } else if observation.incomplete {
            // A selected line style reached a candidate without a complete terminal writer seal.
        } else if observation.capabilities.is_empty() {
            evidence.mark_not_applicable(key);
        } else {
            evidence.mark_applied_with_capabilities(key, observation.capabilities);
        }
    }
}

fn finish_sequence_number_rules(
    evidence: &mut FamilyThemeEvidence,
    sequence_number: &SequenceNumberLabelThemeState,
    rules: BTreeMap<usize, SequenceRuleObservation>,
) {
    for (rule_index, observation) in rules {
        let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule {
            index: rule_index,
            target: ThemeTarget::SequenceNumberLabel,
        };
        if sequence_number.receipt.text_candidates == 0 || !observation.applicable {
            evidence.mark_not_applicable(key);
        } else if let Some(reason) = observation.residual {
            evidence.mark_residual(key, reason);
        } else if observation.incomplete {
            // A SequenceNumberLabel winner reached text without a complete stylesheet/text seal.
        } else if observation.capabilities.is_empty() {
            evidence.mark_not_applicable(key);
        } else {
            evidence.mark_applied_with_capabilities(key, observation.capabilities);
        }
    }
}

fn finish_control_rules(
    evidence: &mut FamilyThemeEvidence,
    target: ThemeTarget,
    surface: &SequenceControlThemeState,
    rules: BTreeMap<usize, SequenceRuleObservation>,
) {
    for (rule_index, observation) in rules {
        let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule {
            index: rule_index,
            target,
        };
        if surface.receipt.surface_candidates.get() == 0 || !observation.applicable {
            evidence.mark_not_applicable(key);
        } else if let Some(reason) = observation.residual {
            evidence.mark_residual(key, reason);
        } else if observation.incomplete {
            // A Loop winner reached a label-box candidate without a complete CSS/DOM seal.
        } else if observation.capabilities.is_empty() {
            evidence.mark_not_applicable(key);
        } else {
            evidence.mark_applied_with_capabilities(key, observation.capabilities);
        }
    }
}

fn finish_static_rect_rules(
    evidence: &mut FamilyThemeEvidence,
    target: ThemeTarget,
    surface: &SequenceStaticRectThemeState,
    rules: BTreeMap<usize, SequenceRuleObservation>,
) {
    for (rule_index, observation) in rules {
        let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule {
            index: rule_index,
            target,
        };
        if surface.surface_count == 0 || !observation.applicable {
            evidence.mark_not_applicable(key);
        } else if let Some(reason) = observation.residual {
            evidence.mark_residual(key, reason);
        } else if observation.incomplete {
            // A direct facet reached a rectangle but the terminal writer did not seal it.
        } else if observation.capabilities.is_empty() {
            evidence.mark_not_applicable(key);
        } else {
            evidence.mark_applied_with_capabilities(key, observation.capabilities);
        }
    }
}

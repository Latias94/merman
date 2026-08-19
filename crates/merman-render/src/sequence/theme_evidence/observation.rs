use std::collections::BTreeSet;

use super::receipts::{
    SequenceLifelineThemeState, SequenceLoopThemeState, SequenceMessageThemeState,
    SequenceNumberLabelThemeState, SequenceRoleTypographyReceipt, SequenceStaticRectThemeState,
};
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemePaintKind, FamilyThemeRuleFacet, FamilyThemeSelectorShape,
    ThemeCapability, ThemeVariant,
};
use crate::family::{
    FamilyThemeResidualReason, resolved_style_property_for_facet as style_property_for_facet,
    unsupported_residual_for_facet as unsupported_reason_for_facet,
};

#[derive(Debug, Default)]
pub(super) struct SequenceRuleObservation {
    pub(super) applicable: bool,
    pub(super) incomplete: bool,
    pub(super) capabilities: BTreeSet<ThemeCapability>,
    pub(super) residual: Option<FamilyThemeResidualReason>,
}

pub(super) fn observe_loop_rule(
    surface: &SequenceLoopThemeState,
    observation: &mut SequenceRuleObservation,
    disposition: FamilyThemeDisposition,
    rule_index: usize,
    selector: FamilyThemeSelectorShape,
    facet: FamilyThemeRuleFacet,
) {
    if surface.receipt.surface_candidates.get() == 0
        || !surface.receipt.route_won(rule_index, selector, facet)
    {
        return;
    }
    observation.applicable = true;
    match facet {
        FamilyThemeRuleFacet::Fill(_) if surface.fill_overridden => return,
        FamilyThemeRuleFacet::Stroke(_) if surface.stroke_overridden => return,
        _ => {}
    }
    match (disposition, facet) {
        (
            FamilyThemeDisposition::TypedAdapter,
            FamilyThemeRuleFacet::Fill(
                kind @ (FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid),
            ),
        ) => {
            if !surface.fill_emitted {
                observation.incomplete = true;
                return;
            }
            observation.capabilities.insert(capability_for_paint(kind));
        }
        (
            FamilyThemeDisposition::TypedAdapter,
            FamilyThemeRuleFacet::Stroke(
                kind @ (FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid),
            ),
        ) => {
            if !surface.stroke_emitted {
                observation.incomplete = true;
                return;
            }
            observation.capabilities.insert(capability_for_paint(kind));
        }
        (FamilyThemeDisposition::TypedAdapter, _) => observation.incomplete = true,
        (FamilyThemeDisposition::Unsupported, facet) => {
            observation
                .residual
                .get_or_insert(unsupported_reason_for_facet(facet));
        }
        (FamilyThemeDisposition::LegacyCompatibility, _) => {}
    }
}

pub(super) fn observe_static_rect_rule(
    surface: &SequenceStaticRectThemeState,
    observation: &mut SequenceRuleObservation,
    disposition: FamilyThemeDisposition,
    rule_index: usize,
    selector: FamilyThemeSelectorShape,
    facet: FamilyThemeRuleFacet,
) {
    if surface.surface_count == 0 || !surface.receipt.route_won(rule_index, selector, facet) {
        return;
    }
    observation.applicable = true;
    match facet {
        FamilyThemeRuleFacet::Fill(_) if surface.fill_overridden => return,
        FamilyThemeRuleFacet::Stroke(_) if surface.stroke_overridden => return,
        _ => {}
    }
    match (disposition, facet) {
        (
            FamilyThemeDisposition::TypedAdapter,
            FamilyThemeRuleFacet::Fill(
                kind @ (FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid),
            ),
        ) => {
            if !surface.fill_emitted {
                observation.incomplete = true;
                return;
            }
            observation.capabilities.insert(capability_for_paint(kind));
        }
        (
            FamilyThemeDisposition::TypedAdapter,
            FamilyThemeRuleFacet::Stroke(
                kind @ (FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid),
            ),
        ) => {
            if !surface.stroke_emitted {
                observation.incomplete = true;
                return;
            }
            observation.capabilities.insert(capability_for_paint(kind));
        }
        (FamilyThemeDisposition::TypedAdapter, _) => observation.incomplete = true,
        (FamilyThemeDisposition::Unsupported, facet) => {
            observation
                .residual
                .get_or_insert(unsupported_reason_for_facet(facet));
        }
        (FamilyThemeDisposition::LegacyCompatibility, _) => {}
    }
}

pub(super) fn observe_message_rule(
    message: &SequenceMessageThemeState,
    observation: &mut SequenceRuleObservation,
    disposition: FamilyThemeDisposition,
    rule_index: usize,
    selector: FamilyThemeSelectorShape,
    facet: FamilyThemeRuleFacet,
) {
    if message.receipt.candidate_count() == 0
        || !message.receipt.route_won(rule_index, selector, facet)
    {
        return;
    }
    observation.applicable = true;
    if message.stroke_overridden
        && matches!(
            facet,
            FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_)
        )
    {
        return;
    }
    match (disposition, facet) {
        (
            FamilyThemeDisposition::TypedAdapter,
            FamilyThemeRuleFacet::Stroke(
                kind @ (FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid),
            ),
        ) => {
            if !message.stroke_emitted {
                observation.incomplete = true;
                return;
            }
            observation.capabilities.insert(capability_for_paint(kind));
        }
        (FamilyThemeDisposition::TypedAdapter, _) => observation.incomplete = true,
        (FamilyThemeDisposition::Unsupported, facet) => {
            observation
                .residual
                .get_or_insert(unsupported_reason_for_facet(facet));
        }
        (FamilyThemeDisposition::LegacyCompatibility, _) => {}
    }
}

pub(super) fn observe_sequence_number_rule(
    sequence_number: &SequenceNumberLabelThemeState,
    observation: &mut SequenceRuleObservation,
    disposition: FamilyThemeDisposition,
    rule_index: usize,
    selector: FamilyThemeSelectorShape,
    facet: FamilyThemeRuleFacet,
) {
    if sequence_number.receipt.text_candidates == 0
        || !sequence_number
            .receipt
            .route_applies(rule_index, selector, facet)
    {
        return;
    }
    observation.applicable = true;
    if sequence_number.fill_overridden && matches!(facet, FamilyThemeRuleFacet::Fill(_)) {
        return;
    }
    match (disposition, facet) {
        (
            FamilyThemeDisposition::TypedAdapter,
            FamilyThemeRuleFacet::Fill(
                kind @ (FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid),
            ),
        ) => {
            if !sequence_number.fill_emitted {
                observation.incomplete = true;
                return;
            }
            observation.capabilities.insert(capability_for_paint(kind));
        }
        (FamilyThemeDisposition::TypedAdapter, _) => observation.incomplete = true,
        (FamilyThemeDisposition::Unsupported, facet) => {
            observation
                .residual
                .get_or_insert(unsupported_reason_for_facet(facet));
        }
        (FamilyThemeDisposition::LegacyCompatibility, _) => {}
    }
}

pub(super) fn observe_lifeline_rule(
    lifeline: &SequenceLifelineThemeState,
    observation: &mut SequenceRuleObservation,
    disposition: FamilyThemeDisposition,
    rule_index: usize,
    selector: FamilyThemeSelectorShape,
    facet: FamilyThemeRuleFacet,
) {
    if lifeline.receipt.candidate_count() == 0
        || !lifeline.receipt.route_won(rule_index, selector, facet)
    {
        return;
    }
    observation.applicable = true;
    if matches!(
        facet,
        FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_)
    ) {
        if lifeline.paint_overridden {
            return;
        }
        if lifeline.selected_property != Some(style_property_for_facet(facet)) {
            return;
        }
    }
    match (disposition, facet) {
        (
            FamilyThemeDisposition::TypedAdapter,
            FamilyThemeRuleFacet::Fill(
                kind @ (FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid),
            )
            | FamilyThemeRuleFacet::Stroke(
                kind @ (FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid),
            ),
        ) => {
            if !lifeline.paint_emitted {
                observation.incomplete = true;
                return;
            }
            observation.capabilities.insert(capability_for_paint(kind));
        }
        (FamilyThemeDisposition::TypedAdapter, FamilyThemeRuleFacet::StrokeWidth) => {
            if !lifeline.stroke_width_emitted {
                observation.incomplete = true;
                return;
            }
            observation
                .capabilities
                .insert(ThemeCapability::BorderStyling);
        }
        (FamilyThemeDisposition::TypedAdapter, _) => observation.incomplete = true,
        (FamilyThemeDisposition::Unsupported, facet) => {
            observation
                .residual
                .get_or_insert(unsupported_reason_for_facet(facet));
        }
        (FamilyThemeDisposition::LegacyCompatibility, _) => {}
    }
}

pub(super) fn observe_typography_rule(
    role: &SequenceRoleTypographyReceipt,
    observation: &mut SequenceRuleObservation,
    disposition: FamilyThemeDisposition,
    rule_index: usize,
    selector: FamilyThemeSelectorShape,
    facet: FamilyThemeRuleFacet,
) {
    if role.label_candidate_count() == 0 {
        return;
    }
    if !role.route_won(rule_index, selector, facet) {
        if matches!(
            (disposition, selector),
            (
                FamilyThemeDisposition::Unsupported,
                FamilyThemeSelectorShape::Ordinal {
                    variant: None | Some(ThemeVariant::Default),
                    ..
                }
            )
        ) && selector.ordinal_domain_intersects_occurrence_count(role.label_candidate_count())
        {
            observation.applicable = true;
            observation
                .residual
                .get_or_insert(unsupported_reason_for_facet(facet));
        }
        return;
    }
    observation.applicable = true;
    if let FamilyThemeRuleFacet::Typography(property) = facet
        && role.config_overrides.contains(&property)
    {
        return;
    }
    match (disposition, facet) {
        (FamilyThemeDisposition::TypedAdapter, FamilyThemeRuleFacet::Typography(_)) => {
            if role.complete() {
                observation.capabilities.insert(ThemeCapability::Typography);
            } else {
                observation.incomplete = true;
            }
        }
        (
            FamilyThemeDisposition::TypedAdapter,
            FamilyThemeRuleFacet::Fill(
                kind @ (FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid),
            ),
        ) => {
            let fill = role.fill_summary();
            if fill.incomplete {
                observation.incomplete = true;
            } else if fill.typed_applied {
                observation.capabilities.insert(capability_for_paint(kind));
            }
        }
        (FamilyThemeDisposition::TypedAdapter, _) => observation.incomplete = true,
        (FamilyThemeDisposition::Unsupported, facet) => {
            observation
                .residual
                .get_or_insert(unsupported_reason_for_facet(facet));
        }
        (FamilyThemeDisposition::LegacyCompatibility, _) => {}
    }
}

fn capability_for_paint(kind: FamilyThemePaintKind) -> ThemeCapability {
    match kind {
        FamilyThemePaintKind::Transparent => ThemeCapability::TransparentPaint,
        FamilyThemePaintKind::Solid => ThemeCapability::SolidPaint,
        FamilyThemePaintKind::Clear
        | FamilyThemePaintKind::LinearGradient
        | FamilyThemePaintKind::RadialGradient
        | FamilyThemePaintKind::Pattern => {
            unreachable!("only typed scalar paints reach direct Sequence paint writers")
        }
    }
}

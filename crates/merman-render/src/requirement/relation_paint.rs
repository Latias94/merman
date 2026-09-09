use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;
use merman_core::diagrams::requirement::RequirementDiagramRenderModel;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeEffect,
    Specified, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectPaintExpectation, DirectPaintTerminalLedger, DirectStaticSelectorDomain,
    FamilyThemeEvidence, FamilyThemeResidualReason, resolve_direct_static_fill,
    resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

pub(crate) const REQUIREMENT_RELATION_PAINT_PATH: &str = "themeVariables.relationColor";

/// One static cascade lane shared by Requirement paths and both marker definitions.
#[derive(Debug)]
pub(crate) struct RequirementRelationPaintPlan {
    assignment: Option<(ResolvedStyleProperty, DirectPaintExpectation)>,
    pending: bool,
    terminal_receipt: OnceLock<RequirementRelationPaintReceipt>,
}

impl RequirementRelationPaintPlan {
    pub(crate) fn resolve(
        theme: &ResolvedDiagramTheme,
        config: &MermaidConfig,
        model: &RequirementDiagramRenderModel,
        evidence: &mut FamilyThemeEvidence,
        work_meter: &OperationWorkMeter,
    ) -> Result<Option<Self>, OperationWorkError> {
        let routes = theme.family_mechanism_routes();
        if !routes.iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Relation,
                    ..
                } | FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Relation
                } | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Relation,
                    ..
                }
            )
        }) {
            return Ok(None);
        }
        let source_owned = merman_core::__private::config_path_overrides_typed_default(
            config,
            REQUIREMENT_RELATION_PAINT_PATH,
        );
        let style = theme.style_with_work_meter(
            ThemeTarget::Relation,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        // Any specified stroke, including Clear and unsupported paint, suppresses fill fallback.
        let fill_fallback = matches!(
            style.stroke_resolution().specified(),
            Specified::Unspecified
        );
        let property = if fill_fallback {
            ResolvedStyleProperty::Fill
        } else {
            ResolvedStyleProperty::Stroke
        };
        let assignment = (!source_owned && !model.relationships.is_empty())
            .then(|| {
                if fill_fallback {
                    resolve_direct_static_fill(
                        theme,
                        &style,
                        &[ThemeTarget::Relation],
                        DirectStaticSelectorDomain::Default,
                    )
                } else {
                    resolve_direct_static_stroke(
                        theme,
                        &style,
                        &[ThemeTarget::Relation],
                        DirectStaticSelectorDomain::Default,
                    )
                }
            })
            .flatten()
            .map(|paint| (property, DirectPaintExpectation::from_paint(paint)));

        let resolves_per_ordinal = routes.iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Relation,
                    selector: FamilyThemeSelectorShape::Ordinal { .. },
                    ..
                } | FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Relation
                } | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Relation,
                    ..
                }
            )
        });
        let mut winners = BTreeSet::new();
        let mut palette_applicable = false;
        let mut effect_bindings = BTreeSet::new();
        let mut observe_style = |style: &crate::diagram_theme::ResolvedThemeStyle, ordinal| {
            let fill_fallback = matches!(
                style.stroke_resolution().specified(),
                Specified::Unspecified
            );
            for (property, origin) in style.winner_rule_properties() {
                if property == ResolvedStyleProperty::Fill && (!fill_fallback || source_owned)
                    || property == ResolvedStyleProperty::Stroke && source_owned
                {
                    continue;
                }
                winners.insert((origin.rule_index(), property));
            }
            palette_applicable |= !source_owned
                && fill_fallback
                && matches!(style.fill_resolution().specified(), Specified::Unspecified)
                && theme.series_color(ThemeTarget::Relation, ordinal).is_some();
            if let Some(ResolvedThemeEffect::Binding { binding, .. }) =
                theme.resolve_effect(ThemeTarget::Relation, style.effect_resolution())
            {
                effect_bindings.insert(binding.effect_id().to_owned());
            }
        };
        if resolves_per_ordinal {
            for ordinal in 1..=model.relationships.len() {
                let style = theme.style_with_work_meter(
                    ThemeTarget::Relation,
                    ThemeVariant::Default,
                    Some(ordinal),
                    work_meter,
                )?;
                observe_style(&style, ordinal);
            }
        } else if !model.relationships.is_empty() {
            observe_style(&style, 1);
        }

        let mut observations = BTreeMap::new();
        for route in routes.iter().copied() {
            work_meter.charge(1)?;
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Relation,
                    facet,
                    ..
                } => {
                    let observation = observations.entry(rule_index).or_insert((false, None));
                    let property = resolved_style_property_for_facet(facet);
                    if !winners.contains(&(rule_index, property)) {
                        continue;
                    }
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter
                        && assignment
                            .as_ref()
                            .is_some_and(|(candidate_property, paint)| {
                                *candidate_property == property && paint.rule_index() == rule_index
                            })
                    {
                        observation.0 = true;
                    } else {
                        observation
                            .1
                            .get_or_insert(unsupported_residual_for_facet(facet));
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Relation,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if palette_applicable {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    } else {
                        evidence.mark_not_applicable(key);
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Relation,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    let applicable = matches!(
                        &key,
                        FamilyThemeMechanismKey::EffectBinding { effect_id, .. }
                            if effect_bindings.contains(effect_id)
                    );
                    if applicable {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    } else {
                        evidence.mark_not_applicable(key);
                    }
                }
                _ => {}
            }
        }

        let mut pending = false;
        for (rule_index, (applied, residual)) in observations {
            let key = relation_rule_key(rule_index);
            if let Some(reason) = residual {
                evidence.mark_residual(key, reason);
            } else if applied {
                pending = true;
            } else {
                evidence.mark_not_applicable(key);
            }
        }
        Ok(Some(Self {
            assignment,
            pending,
            terminal_receipt: OnceLock::new(),
        }))
    }

    pub(crate) fn typed_color(&self) -> Option<(usize, &str)> {
        self.assignment
            .as_ref()
            .map(|(_, paint)| (paint.rule_index(), paint.css()))
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        expected_path_count: usize,
    ) -> Option<RequirementRelationPaintReceipt> {
        let (property, paint) = self.assignment.as_ref()?;
        Some(RequirementRelationPaintReceipt {
            expected: paint.clone(),
            property: *property,
            expected_path_count,
            emitted_path_count: 0,
            css_emitted: false,
            markers_emitted: [false; 2],
            values_match: true,
            ledger: DirectPaintTerminalLedger::default(),
        })
    }

    pub(crate) fn record_terminal(&self, receipt: RequirementRelationPaintReceipt) -> bool {
        self.assignment.as_ref().is_some_and(|(property, paint)| {
            *property == receipt.property && *paint == receipt.expected
        }) && receipt.proves_complete()
            && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        if !self.pending {
            return;
        }
        let (property, paint) = self
            .assignment
            .as_ref()
            .expect("pending static relation paint");
        let key = relation_rule_key(paint.rule_index());
        if self.terminal_receipt.get().is_some_and(|receipt| {
            receipt
                .ledger
                .proves_property(receipt.proves_complete(), paint.rule_index(), *property)
        }) {
            evidence.mark_applied_with_capabilities(key, [paint.capability()]);
        } else {
            evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedPaint);
        }
    }
}

fn relation_rule_key(index: usize) -> FamilyThemeMechanismKey {
    FamilyThemeMechanismKey::Rule {
        index,
        target: ThemeTarget::Relation,
    }
}

/// Constant-size terminal identity checks; the ledger only stores the one static property.
#[derive(Debug)]
pub(crate) struct RequirementRelationPaintReceipt {
    expected: DirectPaintExpectation,
    property: ResolvedStyleProperty,
    expected_path_count: usize,
    emitted_path_count: usize,
    css_emitted: bool,
    markers_emitted: [bool; 2],
    values_match: bool,
    ledger: DirectPaintTerminalLedger,
}

impl RequirementRelationPaintReceipt {
    pub(crate) fn record_css(&mut self, emitted: Option<(usize, &str)>) {
        self.values_match &= !self.css_emitted;
        self.css_emitted = true;
        self.values_match &=
            self.ledger
                .record(Some(&self.expected), false, emitted, self.property);
    }

    pub(crate) fn record_marker(&mut self, index: usize) {
        let Some(emitted) = self.markers_emitted.get_mut(index) else {
            self.values_match = false;
            return;
        };
        self.values_match &= !*emitted;
        *emitted = true;
    }

    pub(crate) fn record_path(&mut self, index: usize) {
        self.values_match &= index == self.emitted_path_count && index < self.expected_path_count;
        self.emitted_path_count = self.emitted_path_count.saturating_add(1);
    }

    fn proves_complete(&self) -> bool {
        self.values_match
            && self.css_emitted
            && self.markers_emitted.iter().all(|emitted| *emitted)
            && self.expected_path_count != 0
            && self.expected_path_count == self.emitted_path_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::ThemeCapability;

    fn plan() -> RequirementRelationPaintPlan {
        RequirementRelationPaintPlan {
            assignment: Some((
                ResolvedStyleProperty::Stroke,
                DirectPaintExpectation::new(3, "#123456", ThemeCapability::SolidPaint),
            )),
            pending: true,
            terminal_receipt: OnceLock::new(),
        }
    }

    #[test]
    fn relation_receipt_requires_css_both_markers_and_every_path() {
        for missing in 0..5 {
            let plan = plan();
            let mut receipt = plan.begin_terminal_receipt(2).unwrap();
            if missing != 0 {
                receipt.record_css(Some((3, "#123456")));
            }
            for marker in 0..2 {
                if missing != marker + 1 {
                    receipt.record_marker(marker);
                }
            }
            for path in 0..2 {
                if missing != path + 3 {
                    receipt.record_path(path);
                }
            }
            assert!(!plan.record_terminal(receipt), "missing terminal {missing}");
        }
        let plan = plan();
        let mut receipt = plan.begin_terminal_receipt(2).unwrap();
        receipt.record_css(Some((3, "#123456")));
        receipt.record_marker(0);
        receipt.record_marker(1);
        receipt.record_path(0);
        receipt.record_path(1);
        assert!(plan.record_terminal(receipt));
    }

    #[test]
    fn relation_receipt_rejects_wrong_values_duplicates_and_empty_paths() {
        for case in 0..5 {
            let plan = plan();
            let mut receipt = plan.begin_terminal_receipt(usize::from(case != 4)).unwrap();
            receipt.record_css(Some((3, if case == 0 { "#abcdef" } else { "#123456" })));
            receipt.record_marker(0);
            receipt.record_marker(1);
            if case != 4 {
                receipt.record_path(0);
            }
            match case {
                1 => receipt.record_css(Some((3, "#123456"))),
                2 => receipt.record_marker(0),
                3 => receipt.record_path(0),
                _ => {}
            }
            assert!(
                !plan.record_terminal(receipt),
                "invalid terminal case {case}"
            );
        }
    }
}

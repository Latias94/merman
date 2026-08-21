use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, ResolvedDiagramTheme, ResolvedStyleProperty, ThemeCapability,
    ThemeTarget, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::{ClassRelationThemePlan, ClassRelationThemeReceipt};

/// Terminal Class theme ledger. The SVG writer may seal it only after the completed root exists.
#[derive(Debug)]
pub(crate) struct ClassThemeEvidenceRecorder {
    expected_relation_paths: usize,
    node_count: usize,
    cluster_label_count: usize,
    table_group_lengths: Vec<usize>,
    terminal_receipt: OnceLock<ClassRelationThemeReceipt>,
}

impl ClassThemeEvidenceRecorder {
    pub(crate) fn new(
        expected_relation_paths: usize,
        node_count: usize,
        cluster_label_count: usize,
        table_group_lengths: Vec<usize>,
    ) -> Self {
        Self {
            expected_relation_paths,
            node_count,
            cluster_label_count,
            table_group_lengths,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn record_terminal(&self, receipt: ClassRelationThemeReceipt) -> bool {
        if receipt.expected_relation_count() != self.expected_relation_paths
            || receipt.expected_node_count() < self.node_count
            || !receipt.proves_complete()
        {
            return false;
        }
        self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish(
        &self,
        theme: Option<&ResolvedDiagramTheme>,
        plan: &ClassRelationThemePlan,
        work_meter: &OperationWorkMeter,
    ) -> Result<FamilyThemeEvidence, OperationWorkError> {
        let mut evidence = FamilyThemeEvidence::from_theme(theme);
        let Some(theme) = theme else {
            return Ok(evidence);
        };
        let receipt = self.terminal_receipt.get();
        let has_relations = self.expected_relation_paths != 0;
        let visible_node_count = receipt
            .map(ClassRelationThemeReceipt::expected_node_count)
            .unwrap_or(self.node_count);
        let mut rules = BTreeMap::<(usize, ThemeTarget), ClassRuleObservation>::new();

        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target,
                    selector,
                    facet,
                } if matches!(
                    target,
                    ThemeTarget::Edge | ThemeTarget::Node | ThemeTarget::NodeLabel
                ) =>
                {
                    let observation = rules.entry((rule_index, target)).or_default();
                    let property = resolved_style_property_for_facet(facet);
                    let route_applies = match target {
                        ThemeTarget::Edge => {
                            has_relations && plan.route_won(rule_index, target, selector, facet)
                        }
                        ThemeTarget::Node | ThemeTarget::NodeLabel => {
                            visible_node_count != 0
                                && if matches!(
                                    (target, property),
                                    (ThemeTarget::Node, ResolvedStyleProperty::Fill)
                                        | (ThemeTarget::Node, ResolvedStyleProperty::Stroke)
                                        | (ThemeTarget::NodeLabel, ResolvedStyleProperty::Fill)
                                ) {
                                    receipt.is_some_and(|receipt| {
                                        receipt.has_node_paint_winner_rule(
                                            rule_index, target, property,
                                        )
                                    })
                                } else {
                                    plan.node_route_won(rule_index, target, selector, facet)
                                }
                        }
                        _ => unreachable!("guarded Class evidence target"),
                    };
                    if !route_applies {
                        continue;
                    }
                    observation.applicable = true;
                    if matches!(target, ThemeTarget::Node | ThemeTarget::NodeLabel) {
                        match (route.disposition(), facet) {
                            (
                                FamilyThemeDisposition::TypedAdapter,
                                FamilyThemeRuleFacet::Fill(
                                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                                ),
                            ) if matches!(target, ThemeTarget::Node | ThemeTarget::NodeLabel) => {
                                observe_terminal_node_paint(
                                    observation,
                                    receipt,
                                    rule_index,
                                    target,
                                    ResolvedStyleProperty::Fill,
                                    facet,
                                );
                            }
                            (
                                FamilyThemeDisposition::TypedAdapter,
                                FamilyThemeRuleFacet::Stroke(
                                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                                ),
                            ) if target == ThemeTarget::Node => {
                                observe_terminal_node_paint(
                                    observation,
                                    receipt,
                                    rule_index,
                                    target,
                                    ResolvedStyleProperty::Stroke,
                                    facet,
                                );
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
                        continue;
                    }
                    match (route.disposition(), facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeRuleFacet::Stroke(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) => {
                            if let Some((winner, css)) = plan.typed_stroke() {
                                if winner != rule_index {
                                    observation.incomplete = true;
                                } else if receipt
                                    .is_some_and(|receipt| receipt.proves_typed_stroke(rule_index))
                                {
                                    observation.capabilities.insert(if css == "transparent" {
                                        ThemeCapability::TransparentPaint
                                    } else {
                                        ThemeCapability::SolidPaint
                                    });
                                } else {
                                    observation.incomplete = true;
                                }
                            } else {
                                observation.incomplete = true;
                            }
                        }
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeRuleFacet::StrokeWidth,
                        ) => {
                            if plan.stroke_width_is_clear_for(rule_index) {
                                observation.residual =
                                    Some(FamilyThemeResidualReason::UnsupportedGeometry);
                            } else if let Some((winner, _)) = plan.typed_stroke_width_emission() {
                                if winner != rule_index {
                                    observation.incomplete = true;
                                } else if receipt
                                    .is_some_and(ClassRelationThemeReceipt::proves_typed_width)
                                {
                                    observation
                                        .capabilities
                                        .insert(ThemeCapability::BorderStyling);
                                } else {
                                    observation.incomplete = true;
                                }
                            } else {
                                observation.incomplete = true;
                            }
                        }
                        (FamilyThemeDisposition::TypedAdapter, _) => {
                            observation.incomplete = true;
                        }
                        (FamilyThemeDisposition::Unsupported, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::LegacyCompatibility, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        for ((rule_index, target), observation) in rules {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Leaving the mechanism unaccounted makes strict completion fail closed.
            } else if !observation.capabilities.is_empty() {
                evidence.mark_applied_with_capabilities(key, observation.capabilities);
            } else if observation.suppressed {
                evidence.mark_not_applicable(key);
            }
        }

        let mut unsupported_domains = Vec::with_capacity(4);
        if let Some(marker_count) =
            receipt.and_then(ClassRelationThemeReceipt::visible_marker_occurrence_count)
        {
            unsupported_domains.push(UnsupportedTerminalDomain::direct(
                ThemeTarget::Marker,
                TerminalVariantDomain::uniform(marker_count, ThemeVariant::Default),
            ));
        }
        unsupported_domains.push(UnsupportedTerminalDomain::direct(
            ThemeTarget::ClusterLabel,
            TerminalVariantDomain::uniform(self.cluster_label_count, ThemeVariant::Default),
        ));
        unsupported_domains.push(UnsupportedTerminalDomain::direct(
            ThemeTarget::Table,
            TerminalVariantDomain::grouped_alternating(
                &self.table_group_lengths,
                ThemeVariant::Odd,
                ThemeVariant::Even,
            ),
        ));
        unsupported_domains.push(UnsupportedTerminalDomain::fallbacks_only(
            ThemeTarget::Edge,
            TerminalVariantDomain::uniform(self.expected_relation_paths, ThemeVariant::Default),
        ));
        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &unsupported_domains,
            work_meter,
        )?;

        Ok(evidence)
    }
}

#[derive(Debug, Default)]
struct ClassRuleObservation {
    applicable: bool,
    incomplete: bool,
    suppressed: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeSet<ThemeCapability>,
}

fn observe_terminal_node_paint(
    observation: &mut ClassRuleObservation,
    receipt: Option<&ClassRelationThemeReceipt>,
    rule_index: usize,
    target: ThemeTarget,
    property: ResolvedStyleProperty,
    facet: FamilyThemeRuleFacet,
) {
    let Some(receipt) = receipt else {
        observation.incomplete = true;
        return;
    };
    if receipt.proves_typed_node_paint(rule_index, target, property) {
        observation.capabilities.insert(match facet {
            FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Transparent)
            | FamilyThemeRuleFacet::Stroke(FamilyThemePaintKind::Transparent) => {
                ThemeCapability::TransparentPaint
            }
            FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid)
            | FamilyThemeRuleFacet::Stroke(FamilyThemePaintKind::Solid) => {
                ThemeCapability::SolidPaint
            }
            _ => unreachable!("guarded Class direct node paint"),
        });
    } else if receipt.proves_complete()
        && !receipt.has_effective_node_paint_rule(rule_index, target, property)
    {
        observation.suppressed = true;
    } else {
        observation.incomplete = true;
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
    use crate::resources::RenderResourcePolicy;

    fn work_meter() -> OperationWorkMeter {
        OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input())
    }

    fn resolved_fill(target: ThemeTarget, variant: Option<ThemeVariant>) -> ResolvedDiagramTheme {
        let mut rule = ThemeRule::new(
            target,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        )
        .for_family(DiagramFamilyId::CLASS);
        if let Some(variant) = variant {
            rule = rule.with_variant(variant);
        }
        DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
            .expect("compile Class terminal fixture")
            .resolve(DiagramFamilyId::CLASS)
    }

    #[test]
    fn terminal_recorder_rejects_wrong_counts_and_duplicate_seals() {
        let complete_receipt = |count| {
            let relations = (0..count)
                .map(|index| super::super::ClassRelationTerminalExpectation::new(index, None, None))
                .collect::<Vec<_>>();
            let mut receipt = ClassRelationThemeReceipt::new(relations, Vec::new(), None, false);
            for index in 0..count {
                receipt.record_relation(index, None, None, None, ";;;", None, false);
            }
            receipt
        };
        let recorder = ClassThemeEvidenceRecorder::new(2, 0, 0, Vec::new());
        assert!(!recorder.record_terminal(complete_receipt(1)));
        assert!(recorder.record_terminal(complete_receipt(2)));
        assert!(!recorder.record_terminal(complete_receipt(2)));
    }

    #[test]
    fn visible_relation_marker_keeps_unsupported_paint_as_a_residual() {
        let theme = resolved_fill(ThemeTarget::Marker, None);
        let recorder = ClassThemeEvidenceRecorder::new(1, 0, 0, Vec::new());
        let mut receipt = ClassRelationThemeReceipt::new(
            vec![super::super::ClassRelationTerminalExpectation::new(
                0,
                Some("extensionStart"),
                None,
            )],
            vec![super::super::ClassMarkerTerminalExpectation::new(
                "extensionStart",
                false,
            )],
            None,
            false,
        );
        receipt.record_marker("extensionStart", false, None, None);
        receipt.record_relation(0, Some("extensionStart"), None, None, ";;;", None, false);
        assert!(recorder.record_terminal(receipt));

        let evidence = recorder
            .finish(
                Some(&theme),
                &ClassRelationThemePlan::default(),
                &work_meter(),
            )
            .expect("reconcile visible Class marker evidence");
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].key(),
            &FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Marker,
            }
        );
    }

    #[test]
    fn table_odd_even_variants_restart_for_each_member_group() {
        let odd_theme = resolved_fill(ThemeTarget::Table, Some(ThemeVariant::Odd));
        let odd_recorder = ClassThemeEvidenceRecorder::new(0, 0, 0, vec![1, 1]);
        let odd_evidence = odd_recorder
            .finish(
                Some(&odd_theme),
                &ClassRelationThemePlan::default(),
                &work_meter(),
            )
            .expect("reconcile Class odd table evidence");
        assert_eq!(odd_evidence.residuals().len(), 1);

        let even_theme = resolved_fill(ThemeTarget::Table, Some(ThemeVariant::Even));
        let even_recorder = ClassThemeEvidenceRecorder::new(0, 0, 0, vec![1, 1]);
        let even_evidence = even_recorder
            .finish(
                Some(&even_theme),
                &ClassRelationThemePlan::default(),
                &work_meter(),
            )
            .expect("reconcile Class even table evidence");
        assert!(even_evidence.residuals().is_empty());
        assert_eq!(
            even_evidence.not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Table,
            }]
        );
    }
}

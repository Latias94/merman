use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty,
    ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::{ClassRelationThemePlan, ClassThemeOutcomeSummary, ClassThemeOutcomes};

/// Theme outcomes recorded by successful Class writers.
#[derive(Debug)]
pub(crate) struct ClassThemeEvidenceRecorder {
    expected_relation_paths: usize,
    expected_note_paths: usize,
    node_count: usize,
    cluster_label_count: usize,
    table_group_lengths: Vec<usize>,
    requested: bool,
    outcomes: OnceLock<ClassThemeOutcomeSummary>,
}

impl ClassThemeEvidenceRecorder {
    pub(crate) fn new(
        theme: Option<&ResolvedDiagramTheme>,
        expected_relation_paths: usize,
        expected_note_paths: usize,
        node_count: usize,
        cluster_label_count: usize,
        table_group_lengths: Vec<usize>,
    ) -> Self {
        Self {
            expected_relation_paths,
            expected_note_paths,
            node_count,
            cluster_label_count,
            table_group_lengths,
            requested: theme.is_some(),
            outcomes: OnceLock::new(),
        }
    }

    pub(crate) fn record_outcomes(&self, outcomes: ClassThemeOutcomes) {
        if self.requested {
            let _ = self.outcomes.set(outcomes.into_summary());
        }
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
        let receipt = self.outcomes.get();
        let receipt_summary = receipt;
        let has_relations = self.expected_relation_paths != 0;
        let visible_node_count = receipt
            .map(ClassThemeOutcomeSummary::node_count)
            .unwrap_or(self.node_count);
        let mut rules = BTreeMap::<(usize, ThemeTarget), ClassRuleObservation>::new();
        let background_count =
            receipt.and_then(ClassThemeOutcomeSummary::edge_label_background_count);
        let mut background_winners = BTreeSet::new();
        if let Some(count) = background_count.filter(|count| *count > 0) {
            let has_ordinals = theme.family_rules().any(|(_, rule)| {
                rule.target() == ThemeTarget::EdgeLabelBackground && rule.ordinal().is_some()
            });
            for ordinal in 1..=if has_ordinals { count } else { 1 } {
                let style = theme.style_with_work_meter(
                    ThemeTarget::EdgeLabelBackground,
                    ThemeVariant::Default,
                    has_ordinals.then_some(ordinal),
                    work_meter,
                )?;
                background_winners.extend(style.winner_rule_properties().filter_map(
                    |(property, origin)| {
                        (!(property == ResolvedStyleProperty::Fill
                            && plan.mermaid_owns_edge_label_background))
                            .then_some((origin.rule_index(), property))
                    },
                ));
            }
        }

        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target,
                    selector,
                    facet,
                } if matches!(
                    target,
                    ThemeTarget::Edge
                        | ThemeTarget::Node
                        | ThemeTarget::NodeLabel
                        | ThemeTarget::Cluster
                        | ThemeTarget::Title
                        | ThemeTarget::EdgeLabelBackground
                ) =>
                {
                    let observation = rules.entry((rule_index, target)).or_default();
                    let property = resolved_style_property_for_facet(facet);
                    let route_applies = match target {
                        ThemeTarget::EdgeLabelBackground => {
                            if background_count.is_none() {
                                observation.applicable = true;
                                observation.incomplete = true;
                                continue;
                            }
                            background_winners.contains(&(rule_index, property))
                        }
                        ThemeTarget::Edge => {
                            let has_static_note_paint = self.expected_note_paths != 0
                                && matches!(selector, FamilyThemeSelectorShape::Static { .. })
                                && matches!(
                                    facet,
                                    FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_)
                                );
                            (has_relations || has_static_note_paint)
                                && plan.route_won(rule_index, target, selector, facet)
                        }
                        ThemeTarget::Cluster | ThemeTarget::Title => {
                            self.cluster_label_count != 0
                                && if target == ThemeTarget::Title {
                                    plan.namespace_title_plan
                                        .route_won(rule_index, selector, facet)
                                } else {
                                    plan.cluster_plan.route_won(rule_index, selector, facet)
                                }
                        }
                        ThemeTarget::Node | ThemeTarget::NodeLabel => {
                            visible_node_count != 0
                                && if matches!(
                                    (target, property),
                                    (
                                        ThemeTarget::Node,
                                        ResolvedStyleProperty::Fill | ResolvedStyleProperty::Stroke
                                    ) | (ThemeTarget::NodeLabel, ResolvedStyleProperty::Fill)
                                ) {
                                    receipt_summary.is_some_and(|summary| {
                                        summary.selected_node_paint(rule_index, target, property)
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
                    if matches!(
                        target,
                        ThemeTarget::Cluster
                            | ThemeTarget::Title
                            | ThemeTarget::EdgeLabelBackground
                    ) {
                        match (route.disposition(), facet) {
                            (
                                FamilyThemeDisposition::TypedAdapter,
                                FamilyThemeRuleFacet::Fill(
                                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                                )
                                | FamilyThemeRuleFacet::Stroke(
                                    FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                                ),
                            ) => {
                                if receipt.is_some_and(|receipt| {
                                    if target == ThemeTarget::EdgeLabelBackground {
                                        property == ResolvedStyleProperty::Fill
                                            && receipt.applied_edge_label_background(rule_index)
                                    } else if target == ThemeTarget::Title {
                                        property == ResolvedStyleProperty::Fill
                                            && receipt.applied_namespace_title(rule_index)
                                    } else {
                                        receipt.applied_cluster_paint(rule_index, property)
                                    }
                                }) {
                                    observation.capabilities.insert(match facet {
                                        FamilyThemeRuleFacet::Fill(
                                            FamilyThemePaintKind::Transparent,
                                        )
                                        | FamilyThemeRuleFacet::Stroke(
                                            FamilyThemePaintKind::Transparent,
                                        ) => ThemeCapability::TransparentPaint,
                                        _ => ThemeCapability::SolidPaint,
                                    });
                                } else {
                                    observation.incomplete = true;
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
                        }
                        continue;
                    }
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
                                    receipt_summary,
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
                                    receipt_summary,
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
                            (FamilyThemeDisposition::TypedAdapter, _) => {
                                observation.incomplete = true;
                            }
                        }
                        continue;
                    }
                    match (route.disposition(), facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeRuleFacet::Fill(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            )
                            | FamilyThemeRuleFacet::Stroke(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) => {
                            if let Some((winner, css)) = plan.typed_stroke() {
                                if winner != rule_index {
                                    observation.incomplete = true;
                                } else if receipt.is_some_and(|receipt| {
                                    receipt.applied_stroke(rule_index, property)
                                }) {
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
                            if plan.relation_width().is_clear_for(rule_index) {
                                observation.residual =
                                    Some(FamilyThemeResidualReason::UnsupportedGeometry);
                            } else if let Some((winner, _)) = plan.relation_width().typed_emission()
                            {
                                if winner != rule_index {
                                    observation.incomplete = true;
                                } else if receipt
                                    .is_some_and(ClassThemeOutcomeSummary::applied_width)
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

        let mut unsupported_domains = Vec::with_capacity(8);
        unsupported_domains.push(
            UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Cluster,
                TerminalVariantDomain::uniform(self.cluster_label_count, ThemeVariant::Default),
            )
            .with_fill_fully_overridden(plan.cluster_plan.fill.config_owned()),
        );
        if let Some(count) = receipt.and_then(ClassThemeOutcomeSummary::edge_label_background_count)
        {
            unsupported_domains.push(
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::EdgeLabelBackground,
                    TerminalVariantDomain::uniform(count, ThemeVariant::Default),
                )
                .with_fill_fully_overridden(plan.mermaid_owns_edge_label_background),
            );
        }
        unsupported_domains.push(UnsupportedTerminalDomain::fallbacks_only(
            ThemeTarget::Node,
            TerminalVariantDomain::uniform(visible_node_count, ThemeVariant::Default),
        ));
        unsupported_domains.push(UnsupportedTerminalDomain::fallbacks_only(
            ThemeTarget::NodeLabel,
            TerminalVariantDomain::uniform(visible_node_count, ThemeVariant::Default),
        ));
        if let Some(marker_count) =
            receipt_summary.map(ClassThemeOutcomeSummary::visible_marker_occurrence_count)
        {
            unsupported_domains.push(UnsupportedTerminalDomain::direct(
                ThemeTarget::Marker,
                TerminalVariantDomain::uniform(marker_count, ThemeVariant::Default),
            ));
        }
        unsupported_domains.push(
            UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Title,
                TerminalVariantDomain::uniform(self.cluster_label_count, ThemeVariant::Default),
            )
            .with_fill_fully_overridden(plan.namespace_title_plan.fill.config_owned()),
        );
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
    receipt: Option<&ClassThemeOutcomeSummary>,
    rule_index: usize,
    target: ThemeTarget,
    property: ResolvedStyleProperty,
    facet: FamilyThemeRuleFacet,
) {
    let Some(receipt) = receipt else {
        observation.incomplete = true;
        return;
    };
    if receipt.applied_node_paint(rule_index, target, property) {
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
    } else if !receipt.has_node_paint(rule_index, target, property) {
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
    fn table_odd_even_variants_restart_for_each_member_group() {
        let odd_theme = resolved_fill(ThemeTarget::Table, Some(ThemeVariant::Odd));
        let odd_recorder =
            ClassThemeEvidenceRecorder::new(Some(&odd_theme), 0, 0, 0, 0, vec![1, 1]);
        let odd_evidence = odd_recorder
            .finish(
                Some(&odd_theme),
                &ClassRelationThemePlan::default(),
                &work_meter(),
            )
            .expect("reconcile Class odd table evidence");
        assert_eq!(odd_evidence.residuals().len(), 1);

        let even_theme = resolved_fill(ThemeTarget::Table, Some(ThemeVariant::Even));
        let even_recorder =
            ClassThemeEvidenceRecorder::new(Some(&even_theme), 0, 0, 0, 0, vec![1, 1]);
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

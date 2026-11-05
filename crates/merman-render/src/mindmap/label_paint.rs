use std::collections::{BTreeMap, BTreeSet};

use merman_model::families::mindmap::MindmapDiagramRenderModel;
use merman_model::presentation::PresentationConfig;

use crate::diagram_theme::{
    FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme, ResolvedStyleProperty,
    ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectPaintExpectation, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    FamilyThemeResidualReason, TerminalVariantDomain, UnsupportedTerminalDomain,
    reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolved_style_property_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

const TARGETS: [ThemeTarget; 2] = [ThemeTarget::Text, ThemeTarget::NodeLabel];

/// Label colors share author order while retaining each real node's CSS/source fill owner.
#[derive(Debug, Default)]
pub(super) struct MindmapLabelPaintPlan {
    fills: Vec<Option<DirectPaintExpectation>>,
    pending: BTreeMap<FamilyThemeMechanismKey, ThemeCapability>,
    evidence: FamilyThemeOutcomes,
    fallback: FamilyThemeOutcomes,
}

impl MindmapLabelPaintPlan {
    pub(super) fn resolve(
        theme: &ResolvedDiagramTheme,
        config: &PresentationConfig,
        model: &MindmapDiagramRenderModel,
        binding: &super::css_binding::MindmapCssBinding,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self::default();
        if !theme
            .family_mechanism_routes()
            .iter()
            .any(|route| match route.mechanism() {
                FamilyThemeMechanism::RuleFacet { target, .. }
                | FamilyThemeMechanism::OrdinalPalette { target }
                | FamilyThemeMechanism::EffectBinding { target, .. } => TARGETS.contains(&target),
                FamilyThemeMechanism::BaseTypography(_) => false,
            })
        {
            return Ok(plan);
        }
        let source_owned = model
            .nodes
            .iter()
            .map(|node| node.label.trim().is_empty() || binding.label_fill_owned(config, node))
            .collect::<Vec<_>>();
        let occurrences = model
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, node)| !node.label.trim().is_empty())
            .map(|(index, _)| (index + 1, ThemeVariant::Default))
            .collect::<Vec<_>>();
        reconcile_unsupported_terminal_domains(
            theme,
            &mut plan.fallback,
            &[UnsupportedTerminalDomain::textual(
                ThemeTarget::NodeLabel,
                &TARGETS,
                TerminalVariantDomain::indexed(&occurrences),
            )
            .with_source_owned_fill(&source_owned)],
            work,
        )?;
        let mut winners = BTreeSet::new();
        for (index, node) in model.nodes.iter().enumerate() {
            work.charge(1)?;
            if node.label.trim().is_empty() {
                plan.fills.push(None);
                continue;
            }
            let style = theme.text_style_with_work_meter(
                ThemeTarget::NodeLabel,
                ThemeVariant::Default,
                Some(index + 1),
                work,
            )?;
            winners.extend(
                style
                    .winner_rule_properties()
                    .filter(|(property, _)| {
                        *property != ResolvedStyleProperty::Fill || !source_owned[index]
                    })
                    .map(|(property, origin)| (origin.rule_index(), property)),
            );
            let fill = (!source_owned[index])
                .then(|| {
                    resolve_direct_static_fill(
                        theme,
                        &style,
                        &TARGETS,
                        DirectStaticSelectorDomain::Default,
                    )
                })
                .flatten()
                .map(DirectPaintExpectation::from_paint);
            plan.fills.push(fill);
        }
        let mut rules = BTreeSet::new();
        let mut incomplete = BTreeSet::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target,
                facet,
                ..
            } = route.mechanism()
            else {
                continue;
            };
            if !TARGETS.contains(&target) {
                continue;
            }
            let key = theme.family_mechanism_key(route);
            rules.insert(key.clone());
            let property = resolved_style_property_for_facet(facet);
            if !winners.contains(&(rule_index, property)) {
                continue;
            }
            let fill = (property == ResolvedStyleProperty::Fill)
                .then(|| {
                    plan.fills
                        .iter()
                        .flatten()
                        .find(|fill| fill.rule_index() == rule_index)
                })
                .flatten();
            if let Some(fill) = fill {
                plan.pending.insert(key, fill.capability());
            } else {
                incomplete.insert(key);
            }
        }
        for key in rules {
            if incomplete.contains(&key) {
                plan.pending.remove(&key);
            } else if !plan.pending.contains_key(&key) {
                plan.evidence.mark_not_applicable(key);
            }
        }
        Ok(plan)
    }

    pub(super) fn fill(&self, index: usize) -> Option<&DirectPaintExpectation> {
        self.fills.get(index)?.as_ref()
    }

    pub(super) fn begin_receipt(&self) -> MindmapLabelPaintReceipt {
        MindmapLabelPaintReceipt {
            css: vec![false; self.fills.len()],
            nodes: vec![false; self.fills.len()],
        }
    }

    pub(super) fn record_css(
        &self,
        receipt: &mut MindmapLabelPaintReceipt,
        index: usize,
        fill: &str,
    ) {
        if let Some(expected) = self.fill(index)
            && let Some(seen) = receipt.css.get_mut(index)
        {
            *seen = expected.css() == fill;
        }
    }

    pub(super) fn record_node(&self, receipt: &mut MindmapLabelPaintReceipt, index: usize) {
        if let Some(seen) = receipt.nodes.get_mut(index) {
            *seen = true;
        }
    }

    pub(super) fn record_evidence(
        &self,
        evidence: &mut FamilyThemeEvidence,
        receipt: Option<&MindmapLabelPaintReceipt>,
    ) {
        // Residual siblings win over partial application; fallback N/A never hides a typed fill.
        for residual in self.fallback.residuals() {
            evidence.mark_residual(residual.key().clone(), residual.reason());
        }
        for (key, capability) in &self.pending {
            let FamilyThemeMechanismKey::Rule {
                index: rule_index, ..
            } = key
            else {
                continue;
            };
            let complete = receipt.is_some_and(|receipt| {
                self.fills
                    .iter()
                    .enumerate()
                    .filter(|(_, fill)| {
                        fill.as_ref()
                            .is_some_and(|fill| fill.rule_index() == *rule_index)
                    })
                    .all(|(index, _)| {
                        receipt.css.get(index) == Some(&true)
                            && receipt.nodes.get(index) == Some(&true)
                    })
            });
            if complete {
                evidence.mark_applied_with_capabilities(key.clone(), [*capability]);
            } else {
                evidence.mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
            }
        }
        evidence.merge_borrowed(&self.evidence);
        for key in self.fallback.not_applicable_mechanisms() {
            evidence.mark_not_applicable(key.clone());
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct MindmapLabelPaintReceipt {
    css: Vec<bool>,
    nodes: Vec<bool>,
}

use std::collections::{BTreeMap, BTreeSet};

use merman_model::families::mindmap::{MindmapDiagramRenderModel, MindmapDiagramRenderNode};
use merman_model::presentation::PresentationConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ThemeCapability,
    ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectPaintExpectation, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    FamilyThemeResidualReason, resolve_direct_static_fill, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

pub(in crate::mindmap) fn node_variant(node: &MindmapDiagramRenderNode) -> ThemeVariant {
    if node
        .css_classes
        .split_whitespace()
        .any(|class| class == "section-root")
    {
        ThemeVariant::Primary
    } else {
        ThemeVariant::Default
    }
}

/// The primary root is outside the branch palette and retains its own source fill owner.
#[derive(Debug, Default)]
pub(super) struct MindmapRootPaintPlan {
    fills: Vec<Option<DirectPaintExpectation>>,
    pending: BTreeMap<FamilyThemeMechanismKey, ThemeCapability>,
    evidence: FamilyThemeOutcomes,
}

impl MindmapRootPaintPlan {
    pub(super) fn resolve(
        theme: &ResolvedDiagramTheme,
        config: &PresentationConfig,
        model: &MindmapDiagramRenderModel,
        binding: &super::super::css_binding::MindmapCssBinding,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self::default();
        let mut winners = BTreeSet::new();
        for (index, node) in model.nodes.iter().enumerate() {
            work.charge(1)?;
            let fill = if node_variant(node) == ThemeVariant::Primary {
                let fill_owned = binding.root_fill_owned(config, node);
                let style = theme.style_with_work_meter(
                    ThemeTarget::Node,
                    ThemeVariant::Primary,
                    Some(index + 1),
                    work,
                )?;
                winners.extend(
                    style
                        .winner_rule_properties()
                        .filter(|(property, _)| {
                            *property != ResolvedStyleProperty::Fill || !fill_owned
                        })
                        .map(|(property, origin)| (origin.rule_index(), property)),
                );
                // Unqualified Node.fill keeps its existing mainBkg bridge. Only an explicit
                // primary rule opts the root into this separate body role.
                let primary = style
                    .fill_resolution()
                    .winner()
                    .is_some_and(|origin| origin.variant() == Some(ThemeVariant::Primary));
                if primary && !fill_owned {
                    resolve_direct_static_fill(
                        theme,
                        &style,
                        &[ThemeTarget::Node],
                        DirectStaticSelectorDomain::State(ThemeVariant::Primary),
                    )
                    .map(DirectPaintExpectation::from_paint)
                } else {
                    None
                }
            } else {
                None
            };
            plan.fills.push(fill);
        }
        let mut observations = BTreeMap::new();
        for route in theme.family_mechanism_routes() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Node,
                selector,
                facet,
            } = route.mechanism()
            else {
                continue;
            };
            if !matches!(
                selector,
                FamilyThemeSelectorShape::Static {
                    variant: Some(ThemeVariant::Primary)
                } | FamilyThemeSelectorShape::Ordinal {
                    variant: Some(ThemeVariant::Primary),
                    ..
                }
            ) {
                continue;
            }
            let key = theme.family_mechanism_key(*route);
            let observation = observations.entry(key).or_insert((None, None));
            if !winners.contains(&(rule_index, resolved_style_property_for_facet(facet))) {
                continue;
            }
            let fill = plan
                .fills
                .iter()
                .flatten()
                .find(|fill| fill.rule_index() == rule_index);
            if route.disposition() == FamilyThemeDisposition::TypedAdapter
                && matches!(facet, FamilyThemeRuleFacet::Fill(_))
                && let Some(fill) = fill
            {
                observation.0 = Some(fill.capability());
            } else {
                observation
                    .1
                    .get_or_insert(unsupported_residual_for_facet(facet));
            }
        }
        for (key, (capability, residual)) in observations {
            if let Some(reason) = residual {
                plan.evidence.mark_residual(key, reason);
            } else if let Some(capability) = capability {
                plan.pending.insert(key, capability);
            } else {
                plan.evidence.mark_not_applicable(key);
            }
        }
        Ok(plan)
    }

    pub(super) fn fill(&self, index: usize) -> Option<&str> {
        self.fills
            .get(index)?
            .as_ref()
            .map(DirectPaintExpectation::css)
    }

    pub(super) fn begin_receipt(&self) -> MindmapRootPaintReceipt {
        MindmapRootPaintReceipt {
            css: vec![false; self.fills.len()],
            nodes: vec![false; self.fills.len()],
        }
    }

    pub(super) fn record_css(
        &self,
        receipt: &mut MindmapRootPaintReceipt,
        index: usize,
        fill: &str,
    ) {
        if let Some(seen) = receipt.css.get_mut(index) {
            *seen = self.fill(index) == Some(fill);
        }
    }

    pub(super) fn record_node(&self, receipt: &mut MindmapRootPaintReceipt, index: usize) {
        if let Some(seen) = receipt.nodes.get_mut(index) {
            *seen = true;
        }
    }

    pub(super) fn record_evidence(
        &self,
        evidence: &mut FamilyThemeEvidence,
        receipt: Option<&MindmapRootPaintReceipt>,
    ) {
        evidence.merge_borrowed(&self.evidence);
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
    }
}

#[derive(Debug, Default)]
pub(super) struct MindmapRootPaintReceipt {
    css: Vec<bool>,
    nodes: Vec<bool>,
}

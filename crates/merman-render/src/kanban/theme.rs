use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    ResolvedDiagramTheme, Specified, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

/// Mermaid 11.16 assigns `rx = ry = 5` directly to every Kanban item. Kanban source metadata and
/// site configuration expose no separate radius owner, so `Task.radius` can replace this value
/// without competing with a source-owned style channel. Section geometry remains independent.
pub(super) const MERMAID_TASK_RADIUS_PX: f64 = 5.0;

/// Kanban task geometry and evidence resolved once for concrete item occurrences.
#[derive(Debug)]
pub(crate) struct KanbanTaskTheme {
    item_radii: Vec<f64>,
    evidence: FamilyThemeEvidence,
    pending_radius_key: Option<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<()>,
}

impl KanbanTaskTheme {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        item_count: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::baseline(item_count));
        };

        let mut item_radii = Vec::with_capacity(item_count);
        let mut winner_properties = BTreeSet::new();
        for item_index in 0..item_count {
            let style = theme.style_with_work_meter(
                ThemeTarget::Task,
                ThemeVariant::Default,
                Some(item_index + 1),
                work_meter,
            )?;
            winner_properties.extend(
                style
                    .winner_rule_properties()
                    .into_iter()
                    .map(|(property, origin)| (origin.rule_index(), property)),
            );
            item_radii.push(typed_radius_px(theme, &style));
        }

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<usize, KanbanTaskRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Task,
                    facet,
                    ..
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    if !winner_properties
                        .contains(&(rule_index, resolved_style_property_for_facet(facet)))
                    {
                        continue;
                    }
                    observation.applicable = true;
                    match (route.disposition(), facet) {
                        (FamilyThemeDisposition::TypedAdapter, FamilyThemeRuleFacet::Radius) => {
                            observation.radius_pending = true;
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
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Task,
                } if item_count == 0 => {
                    evidence.mark_not_applicable(theme.family_mechanism_key(route));
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Task,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if item_count == 0 {
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

        let mut pending_radius_key = None;
        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Task,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // A mixed rule cannot be signed Applied until every winning facet is accounted.
            } else if observation.radius_pending {
                debug_assert!(pending_radius_key.is_none());
                pending_radius_key = Some(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            item_radii,
            evidence,
            pending_radius_key,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline(item_count: usize) -> Self {
        Self {
            item_radii: vec![MERMAID_TASK_RADIUS_PX; item_count],
            evidence: FamilyThemeEvidence::default(),
            pending_radius_key: None,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn radius_px(&self, item_index: usize) -> Option<f64> {
        self.item_radii.get(item_index).copied()
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<KanbanTaskRadiusThemeReceipt> {
        self.pending_radius_key
            .as_ref()
            .map(|_| KanbanTaskRadiusThemeReceipt::new(self.item_radii.len()))
    }

    pub(crate) fn record_terminal(&self, receipt: KanbanTaskRadiusThemeReceipt) -> bool {
        self.pending_radius_key.is_some()
            && receipt.proves(self.item_radii.len())
            && self.terminal_receipt.set(()).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if let Some(key) = self.pending_radius_key.clone()
            && self.terminal_receipt.get().is_some()
        {
            evidence.mark_applied_with_capabilities(key, [ThemeCapability::RoundedGeometry]);
        }
        evidence
    }
}

fn typed_radius_px(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> f64 {
    let Some(origin) = style.radius_resolution().winner() else {
        return MERMAID_TASK_RADIUS_PX;
    };
    if theme.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::Radius)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return MERMAID_TASK_RADIUS_PX;
    }
    match style.radius_resolution().specified() {
        Specified::Value(value) => f64::from(*value),
        Specified::Unspecified | Specified::Clear => MERMAID_TASK_RADIUS_PX,
    }
}

/// Writer-owned proof that every Kanban item emitted its canonical SVG radius attributes.
#[derive(Debug)]
pub(crate) struct KanbanTaskRadiusThemeReceipt {
    checkpointed_items: Vec<bool>,
    attributes_match: bool,
}

impl KanbanTaskRadiusThemeReceipt {
    fn new(item_count: usize) -> Self {
        Self {
            checkpointed_items: vec![false; item_count],
            attributes_match: true,
        }
    }

    pub(crate) fn record_checkpointed_item(&mut self, item_index: usize, attributes_match: bool) {
        let Some(checkpointed) = self.checkpointed_items.get_mut(item_index) else {
            self.attributes_match = false;
            return;
        };
        if *checkpointed {
            self.attributes_match = false;
            return;
        }
        *checkpointed = true;
        self.attributes_match &= attributes_match;
    }

    fn proves(&self, expected_item_count: usize) -> bool {
        self.checkpointed_items.len() == expected_item_count
            && self.attributes_match
            && self
                .checkpointed_items
                .iter()
                .all(|checkpointed| *checkpointed)
    }
}

#[derive(Debug, Default)]
struct KanbanTaskRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    radius_pending: bool,
}

#[cfg(test)]
mod tests {
    use super::KanbanTaskRadiusThemeReceipt;

    #[test]
    fn task_radius_receipt_requires_each_terminal_item_once() {
        let mut incomplete = KanbanTaskRadiusThemeReceipt::new(2);
        incomplete.record_checkpointed_item(0, true);
        assert!(!incomplete.proves(2));

        let mut duplicate = KanbanTaskRadiusThemeReceipt::new(1);
        duplicate.record_checkpointed_item(0, true);
        duplicate.record_checkpointed_item(0, true);
        assert!(!duplicate.proves(1));

        let mut mismatch = KanbanTaskRadiusThemeReceipt::new(1);
        mismatch.record_checkpointed_item(0, false);
        assert!(!mismatch.proves(1));
    }
}

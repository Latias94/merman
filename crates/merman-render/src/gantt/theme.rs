use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::diagrams::gantt::GanttRenderTask;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    ResolvedDiagramTheme, Specified, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

const MERMAID_TASK_RADIUS_PX: f64 = 3.0;

/// Gantt task geometry and evidence resolved once for the concrete semantic task occurrences.
#[derive(Debug)]
pub(crate) struct GanttTaskTheme {
    task_radii: Vec<f64>,
    evidence: FamilyThemeEvidence,
    pending_radius_key: Option<FamilyThemeMechanismKey>,
    layout_occurrences: OnceLock<Box<[usize]>>,
    terminal_receipt: OnceLock<GanttTaskRadiusThemeReceipt>,
}

impl GanttTaskTheme {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        tasks: &[GanttRenderTask],
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::baseline(tasks.len()));
        };

        let mut task_radii = Vec::with_capacity(tasks.len());
        let mut winner_properties = BTreeSet::new();
        let mut emitted_winner_properties = BTreeSet::new();
        for (task_index, task) in tasks.iter().enumerate() {
            let variants = task_theme_variants(task);
            let mut primary_radius = MERMAID_TASK_RADIUS_PX;
            for (variant_index, variant) in variants.into_iter().flatten().enumerate() {
                let style = theme.style_with_work_meter(
                    ThemeTarget::Task,
                    variant,
                    Some(task_index + 1),
                    work_meter,
                )?;
                for (property, origin) in style.winner_rule_properties() {
                    let winner = (origin.rule_index(), property);
                    winner_properties.insert(winner);
                    if variant_index == 0 {
                        emitted_winner_properties.insert(winner);
                    }
                }
                if variant_index == 0 {
                    primary_radius = typed_radius_px(theme, &style);
                }
            }
            task_radii.push(primary_radius);
        }

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<usize, GanttTaskRuleObservation>::new();
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
                            observation.radius_pending = emitted_winner_properties
                                .contains(&(rule_index, resolved_style_property_for_facet(facet)));
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
                } => {
                    let key = theme.family_mechanism_key(route);
                    if tasks.is_empty() {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Task,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if tasks.is_empty() {
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
                // Leaving the mechanism unaccounted makes strict completion fail closed.
            } else if observation.radius_pending {
                debug_assert!(pending_radius_key.is_none());
                pending_radius_key = Some(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            task_radii,
            evidence,
            pending_radius_key,
            layout_occurrences: OnceLock::new(),
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline(task_count: usize) -> Self {
        Self {
            task_radii: vec![MERMAID_TASK_RADIUS_PX; task_count],
            evidence: FamilyThemeEvidence::default(),
            pending_radius_key: None,
            layout_occurrences: OnceLock::new(),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn task_count(&self) -> usize {
        self.task_radii.len()
    }

    pub(crate) fn radius_px(&self, task_index: usize) -> Option<f64> {
        self.task_radii.get(task_index).copied()
    }

    pub(crate) fn bind_layout_occurrences(&self, layout_occurrences: Vec<usize>) -> bool {
        if layout_occurrences.len() != self.task_count() {
            return false;
        }
        let mut seen = vec![false; self.task_count()];
        for &semantic_index in &layout_occurrences {
            let Some(entry) = seen.get_mut(semantic_index) else {
                return false;
            };
            if *entry {
                return false;
            }
            *entry = true;
        }
        seen.into_iter().all(|entry| entry)
            && self
                .layout_occurrences
                .set(layout_occurrences.into_boxed_slice())
                .is_ok()
    }

    pub(crate) fn radius_px_for_layout_task(&self, layout_index: usize) -> Option<f64> {
        let semantic_index = *self.layout_occurrences.get()?.get(layout_index)?;
        self.radius_px(semantic_index)
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<GanttTaskRadiusThemeReceipt> {
        self.pending_radius_key
            .as_ref()
            .map(|_| GanttTaskRadiusThemeReceipt::new(self.task_count()))
    }

    pub(crate) fn record_terminal(&self, receipt: GanttTaskRadiusThemeReceipt) -> bool {
        self.pending_radius_key.is_some()
            && receipt.proves(self.task_count())
            && self.terminal_receipt.set(receipt).is_ok()
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

fn task_theme_variants(task: &GanttRenderTask) -> [Option<ThemeVariant>; 3] {
    let mut variants = [None; 3];
    let mut next = 0;
    for (present, variant) in [
        (task.active, ThemeVariant::Active),
        (task.done, ThemeVariant::Success),
        (task.crit, ThemeVariant::Error),
    ] {
        if present {
            variants[next] = Some(variant);
            next += 1;
        }
    }
    if next == 0 {
        variants[0] = Some(ThemeVariant::Default);
    }
    variants
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

/// Writer-owned proof that every semantic task bar emitted its canonical SVG radius attributes.
#[derive(Debug)]
pub(crate) struct GanttTaskRadiusThemeReceipt {
    checkpointed_tasks: Vec<bool>,
    attributes_match: bool,
}

impl GanttTaskRadiusThemeReceipt {
    fn new(task_count: usize) -> Self {
        Self {
            checkpointed_tasks: vec![false; task_count],
            attributes_match: true,
        }
    }

    pub(crate) fn record_checkpointed_task(&mut self, task_index: usize, attributes_match: bool) {
        let Some(checkpointed) = self.checkpointed_tasks.get_mut(task_index) else {
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

    fn proves(&self, expected_task_count: usize) -> bool {
        self.checkpointed_tasks.len() == expected_task_count
            && self.attributes_match
            && self
                .checkpointed_tasks
                .iter()
                .all(|checkpointed| *checkpointed)
    }
}

#[derive(Debug, Default)]
struct GanttTaskRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    radius_pending: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, ThemeGeometryPatch, ThemeRule, ThemeRuleSet,
        ThemeStylePatch,
    };
    use crate::resources::RenderResourcePolicy;

    fn radius_style(radius: f32) -> ThemeStylePatch {
        ThemeStylePatch {
            geometry: ThemeGeometryPatch {
                radius: Specified::Value(radius),
            },
            ..ThemeStylePatch::default()
        }
    }

    #[test]
    fn task_radius_receipt_requires_each_terminal_attribute_once() {
        let mut receipt = GanttTaskRadiusThemeReceipt::new(2);
        receipt.record_checkpointed_task(0, true);
        assert!(!receipt.proves(2));
        receipt.record_checkpointed_task(1, false);
        assert!(!receipt.proves(2));

        let mut duplicate = GanttTaskRadiusThemeReceipt::new(1);
        duplicate.record_checkpointed_task(0, true);
        duplicate.record_checkpointed_task(0, true);
        assert!(!duplicate.proves(1));
    }

    #[test]
    fn terminal_receipt_rejects_wrong_counts_and_duplicate_seals() {
        let key = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Task,
        };
        let theme = GanttTaskTheme {
            task_radii: vec![3.0, 3.0],
            evidence: FamilyThemeEvidence::default(),
            pending_radius_key: Some(key),
            layout_occurrences: OnceLock::new(),
            terminal_receipt: OnceLock::new(),
        };
        let mut wrong_count = GanttTaskRadiusThemeReceipt::new(1);
        wrong_count.record_checkpointed_task(0, true);
        assert!(!theme.record_terminal(wrong_count));

        let mut receipt = GanttTaskRadiusThemeReceipt::new(2);
        receipt.record_checkpointed_task(0, true);
        receipt.record_checkpointed_task(1, true);
        assert!(theme.record_terminal(receipt));

        let mut duplicate = GanttTaskRadiusThemeReceipt::new(2);
        duplicate.record_checkpointed_task(0, true);
        duplicate.record_checkpointed_task(1, true);
        assert!(!theme.record_terminal(duplicate));
    }

    #[test]
    fn layout_binding_maps_render_order_back_to_semantic_occurrences() {
        let theme = GanttTaskTheme {
            task_radii: vec![7.0, 3.0],
            evidence: FamilyThemeEvidence::default(),
            pending_radius_key: None,
            layout_occurrences: OnceLock::new(),
            terminal_receipt: OnceLock::new(),
        };

        assert!(theme.bind_layout_occurrences(vec![1, 0]));
        assert_eq!(theme.radius_px_for_layout_task(0), Some(3.0));
        assert_eq!(theme.radius_px_for_layout_task(1), Some(7.0));
        assert!(!theme.bind_layout_occurrences(vec![0, 1]));

        let invalid = GanttTaskTheme::baseline(2);
        assert!(!invalid.bind_layout_occurrences(vec![0, 0]));
        assert!(!invalid.bind_layout_occurrences(vec![0, 2]));
    }

    #[test]
    fn secondary_variant_winner_cannot_claim_terminal_radius_evidence() {
        let resolved = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(ThemeTarget::Task, radius_style(7.0)))
                        .with_rule(
                            ThemeRule::new(ThemeTarget::Task, radius_style(11.0))
                                .with_variant(ThemeVariant::Active),
                        ),
                ),
            )
            .expect("compile Gantt multi-state theme")
            .resolve(crate::DiagramFamilyId::GANTT);
        let task = GanttRenderTask {
            active: true,
            crit: true,
            ..GanttRenderTask::default()
        };
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());

        let task_theme = GanttTaskTheme::resolve(Some(&resolved), &[task], &work_meter)
            .expect("resolve Gantt multi-state theme");
        assert_eq!(task_theme.radius_px(0), Some(MERMAID_TASK_RADIUS_PX));

        let evidence = task_theme.finish_evidence();
        let unqualified = FamilyThemeMechanismKey::Rule {
            index: 0,
            target: ThemeTarget::Task,
        };
        let active = FamilyThemeMechanismKey::Rule {
            index: 1,
            target: ThemeTarget::Task,
        };
        assert!(!evidence.applied().contains(&unqualified));
        assert!(evidence.not_applicable_mechanisms().contains(&unqualified));
        assert!(
            evidence
                .residuals()
                .iter()
                .any(|residual| residual.key() == &active)
        );
    }
}

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;
use merman_core::diagrams::gantt::GanttRenderTask;

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemePaintKind, FamilyThemeRuleFacet, ResolvedDiagramTheme, ResolvedStyleProperty,
    ResolvedThemeStyle, Specified, ThemeCapability, ThemeTarget,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::OperationWorkMeter;

mod task_bar;

use task_bar::GanttTaskBarState;

const MERMAID_TASK_RADIUS_PX: f64 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GanttTaskFillOwner {
    Mermaid,
    Typed {
        rule_index: usize,
        capability: ThemeCapability,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GanttTaskFillExpectation {
    css: Box<str>,
    owner: GanttTaskFillOwner,
}

impl GanttTaskFillExpectation {
    const fn typed_rule_index(&self) -> Option<usize> {
        match self.owner {
            GanttTaskFillOwner::Mermaid => None,
            GanttTaskFillOwner::Typed { rule_index, .. } => Some(rule_index),
        }
    }

    const fn typed_capability(&self) -> Option<ThemeCapability> {
        match self.owner {
            GanttTaskFillOwner::Mermaid => None,
            GanttTaskFillOwner::Typed { capability, .. } => Some(capability),
        }
    }
}

#[derive(Debug, Clone)]
struct GanttTaskTerminalExpectation {
    semantic_id: Box<str>,
    state: GanttTaskBarState,
    radius_px: f64,
    radius_rule_index: Option<usize>,
    fill: Option<GanttTaskFillExpectation>,
}

impl GanttTaskTerminalExpectation {
    fn baseline(task: &GanttRenderTask) -> Self {
        Self {
            semantic_id: task.id.clone().into_boxed_str(),
            state: GanttTaskBarState::from_task(task),
            radius_px: MERMAID_TASK_RADIUS_PX,
            radius_rule_index: None,
            fill: None,
        }
    }
}

/// Gantt task geometry, paint, and terminal evidence resolved once for semantic occurrences.
#[derive(Debug)]
pub(crate) struct GanttTaskTheme {
    tasks: Box<[GanttTaskTerminalExpectation]>,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, GanttTaskPendingEvidence>,
    layout_occurrences: OnceLock<Box<[usize]>>,
    terminal_receipt: OnceLock<GanttTaskThemeReceipt>,
}

impl GanttTaskTheme {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        tasks: &[GanttRenderTask],
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let Some(theme) = theme else {
            return Ok(Self::baseline(tasks));
        };

        let mut task_expectations = tasks
            .iter()
            .map(GanttTaskTerminalExpectation::baseline)
            .collect::<Vec<_>>();
        let mut winner_properties = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut radius_rules = BTreeSet::new();
        let mut typed_fill_capabilities = BTreeMap::<usize, ThemeCapability>::new();
        let mut source_owned_fill_rules = BTreeSet::new();

        for (task_index, expectation) in task_expectations.iter_mut().enumerate() {
            let style = theme.style_with_work_meter(
                ThemeTarget::Task,
                expectation.state.theme_variant(),
                Some(task_index + 1),
                work_meter,
            )?;
            for (property, origin) in style.winner_rule_properties() {
                winner_properties.insert((origin.rule_index(), property));
            }

            let (radius_px, radius_rule_index) = typed_radius(theme, &style);
            expectation.radius_px = radius_px;
            expectation.radius_rule_index = radius_rule_index;
            if let Some(rule_index) = radius_rule_index {
                radius_rules.insert(rule_index);
            }

            expectation.fill =
                typed_fill_expectation(theme, effective_config, expectation.state, &style)?;
            if let Some(fill) = &expectation.fill {
                if let (Some(rule_index), Some(capability)) =
                    (fill.typed_rule_index(), fill.typed_capability())
                {
                    typed_fill_capabilities.insert(rule_index, capability);
                } else if let Some(origin) = style.fill_resolution().winner() {
                    source_owned_fill_rules.insert(origin.rule_index());
                }
            }
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
                    let property = resolved_style_property_for_facet(facet);
                    if !winner_properties.contains(&(rule_index, property)) {
                        continue;
                    }
                    observation.applicable = true;
                    match (route.disposition(), facet) {
                        (FamilyThemeDisposition::TypedAdapter, FamilyThemeRuleFacet::Radius) => {
                            if radius_rules.contains(&rule_index) {
                                observation.pending.radius = true;
                                observation
                                    .pending
                                    .capabilities
                                    .insert(ThemeCapability::RoundedGeometry);
                            } else {
                                observation.incomplete = true;
                            }
                        }
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeRuleFacet::Fill(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) => {
                            if let Some(capability) = typed_fill_capabilities.get(&rule_index) {
                                observation.pending.fill = true;
                                observation.pending.capabilities.insert(*capability);
                            } else if source_owned_fill_rules.contains(&rule_index) {
                                observation.suppressed = true;
                            } else {
                                observation.incomplete = true;
                            }
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

        let mut pending = BTreeMap::new();
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
                // Mixed rules remain fail-closed until every winning facet has one terminal owner.
            } else if observation.pending.requires_terminal_proof() {
                pending.insert(key, observation.pending);
            } else if observation.suppressed {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            tasks: task_expectations.into_boxed_slice(),
            evidence,
            pending,
            layout_occurrences: OnceLock::new(),
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline(tasks: &[GanttRenderTask]) -> Self {
        Self {
            tasks: tasks
                .iter()
                .map(GanttTaskTerminalExpectation::baseline)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            evidence: FamilyThemeEvidence::default(),
            pending: BTreeMap::new(),
            layout_occurrences: OnceLock::new(),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn task_count(&self) -> usize {
        self.tasks.len()
    }

    pub(crate) fn radius_px(&self, task_index: usize) -> Option<f64> {
        self.tasks.get(task_index).map(|task| task.radius_px)
    }

    pub(crate) fn bar_state_class_for_semantic_task(
        &self,
        task_index: usize,
    ) -> Option<&'static str> {
        self.tasks
            .get(task_index)
            .map(|task| task.state.bar_class_prefix())
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

    pub(crate) fn terminal_fill_for_layout_task(&self, layout_index: usize) -> Option<&str> {
        self.layout_task(layout_index)
            .and_then(|task| task.fill.as_ref())
            .map(|fill| fill.css.as_ref())
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<GanttTaskThemeReceipt> {
        let requires_receipt =
            !self.pending.is_empty() || self.tasks.iter().any(|task| task.fill.is_some());
        requires_receipt.then(|| {
            let Some(layout_occurrences) = self.layout_occurrences.get() else {
                return GanttTaskThemeReceipt::invalid(self.task_count());
            };
            let expectations = layout_occurrences
                .iter()
                .filter_map(|semantic_index| self.tasks.get(*semantic_index).cloned())
                .collect::<Vec<_>>();
            if expectations.len() != self.task_count() {
                GanttTaskThemeReceipt::invalid(self.task_count())
            } else {
                GanttTaskThemeReceipt::new(expectations)
            }
        })
    }

    pub(crate) fn record_terminal(&self, receipt: GanttTaskThemeReceipt) -> bool {
        receipt.proves_complete() && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(receipt) = self.terminal_receipt.get() else {
            return evidence;
        };
        for (key, pending) in &self.pending {
            let rule_index = match key {
                FamilyThemeMechanismKey::Rule { index, .. } => *index,
                FamilyThemeMechanismKey::Typography
                | FamilyThemeMechanismKey::OrdinalPalette { .. }
                | FamilyThemeMechanismKey::EffectBinding { .. } => continue,
            };
            if receipt.proves_rule(rule_index, pending) {
                evidence.mark_applied_with_capabilities(
                    key.clone(),
                    pending.capabilities.iter().copied(),
                );
            }
        }
        evidence
    }

    fn layout_task(&self, layout_index: usize) -> Option<&GanttTaskTerminalExpectation> {
        let semantic_index = *self.layout_occurrences.get()?.get(layout_index)?;
        self.tasks.get(semantic_index)
    }
}

fn typed_radius(theme: &ResolvedDiagramTheme, style: &ResolvedThemeStyle) -> (f64, Option<usize>) {
    let Some(origin) = style.radius_resolution().winner() else {
        return (MERMAID_TASK_RADIUS_PX, None);
    };
    if theme.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::Radius)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return (MERMAID_TASK_RADIUS_PX, None);
    }
    let radius = match style.radius_resolution().specified() {
        Specified::Value(value) => f64::from(*value),
        Specified::Unspecified | Specified::Clear => MERMAID_TASK_RADIUS_PX,
    };
    (radius, Some(origin.rule_index()))
}

fn typed_fill_expectation(
    theme: &ResolvedDiagramTheme,
    effective_config: &MermaidConfig,
    state: GanttTaskBarState,
    style: &ResolvedThemeStyle,
) -> crate::Result<Option<GanttTaskFillExpectation>> {
    let Some(origin) = style.fill_resolution().winner() else {
        return Ok(None);
    };
    let Some(facet) = FamilyThemeRuleFacet::fill(style.fill_resolution().specified()) else {
        return Ok(None);
    };
    if theme.rule_facet_disposition(origin.rule_index(), facet)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return Ok(None);
    }

    let (typed_css, capability) = match style.fill_resolution().specified() {
        Specified::Value(CanvasPaint::Transparent) => {
            ("transparent".to_string(), ThemeCapability::TransparentPaint)
        }
        Specified::Value(CanvasPaint::Solid(color)) => {
            (color.as_css(), ThemeCapability::SolidPaint)
        }
        Specified::Unspecified
        | Specified::Clear
        | Specified::Value(
            CanvasPaint::LinearGradient(_)
            | CanvasPaint::RadialGradient(_)
            | CanvasPaint::Pattern(_),
        ) => return Ok(None),
    };

    let final_fill_path = state.final_fill_path();
    let owner = if merman_core::__private::config_path_overrides_typed_default(
        effective_config,
        final_fill_path,
    ) {
        GanttTaskFillOwner::Mermaid
    } else {
        GanttTaskFillOwner::Typed {
            rule_index: origin.rule_index(),
            capability,
        }
    };
    let css = match owner {
        GanttTaskFillOwner::Mermaid => effective_config
            .get_str(final_fill_path)
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!(
                    "Gantt terminal fill owner `{final_fill_path}` had no effective value"
                ),
            })?
            .to_string(),
        GanttTaskFillOwner::Typed { .. } => typed_css,
    };

    Ok(Some(GanttTaskFillExpectation {
        css: css.into_boxed_str(),
        owner,
    }))
}

#[derive(Debug, Default)]
struct GanttTaskRuleObservation {
    applicable: bool,
    incomplete: bool,
    suppressed: bool,
    residual: Option<FamilyThemeResidualReason>,
    pending: GanttTaskPendingEvidence,
}

#[derive(Debug, Default)]
struct GanttTaskPendingEvidence {
    radius: bool,
    fill: bool,
    capabilities: BTreeSet<ThemeCapability>,
}

impl GanttTaskPendingEvidence {
    const fn requires_terminal_proof(&self) -> bool {
        self.radius || self.fill
    }
}

/// Writer-owned proof that every real Gantt task rect reached its canonical terminal state.
#[derive(Debug)]
pub(crate) struct GanttTaskThemeReceipt {
    expectations: Box<[GanttTaskTerminalExpectation]>,
    checkpointed_tasks: Vec<bool>,
    terminals_match: bool,
    terminal_ids: BTreeSet<Box<str>>,
    radius_rules: BTreeSet<usize>,
    fill_rules: BTreeSet<usize>,
}

impl GanttTaskThemeReceipt {
    fn new(expectations: Vec<GanttTaskTerminalExpectation>) -> Self {
        Self {
            checkpointed_tasks: vec![false; expectations.len()],
            expectations: expectations.into_boxed_slice(),
            terminals_match: true,
            terminal_ids: BTreeSet::new(),
            radius_rules: BTreeSet::new(),
            fill_rules: BTreeSet::new(),
        }
    }

    fn invalid(expected_task_count: usize) -> Self {
        Self {
            expectations: Vec::new().into_boxed_slice(),
            checkpointed_tasks: vec![false; expected_task_count],
            terminals_match: false,
            terminal_ids: BTreeSet::new(),
            radius_rules: BTreeSet::new(),
            fill_rules: BTreeSet::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_checkpointed_task(
        &mut self,
        layout_index: usize,
        diagram_id: &str,
        semantic_id: &str,
        terminal_id: &str,
        section_suffix: &str,
        terminal_class: &str,
        emitted_radius_x: f64,
        emitted_radius_y: f64,
        emitted_fill: Option<&str>,
    ) {
        let Some(checkpointed) = self.checkpointed_tasks.get_mut(layout_index) else {
            self.terminals_match = false;
            return;
        };
        if *checkpointed {
            self.terminals_match = false;
            return;
        }
        *checkpointed = true;

        let Some(expected) = self.expectations.get(layout_index) else {
            self.terminals_match = false;
            return;
        };
        let expected_terminal_id = if diagram_id.is_empty() {
            expected.semantic_id.to_string()
        } else {
            format!("{diagram_id}-{}", expected.semantic_id)
        };
        let expected_state_class = format!("{}{section_suffix}", expected.state.bar_class_prefix());
        let class_matches = terminal_class.split_ascii_whitespace().next() == Some("task")
            && terminal_class
                .split_ascii_whitespace()
                .any(|class| class == expected_state_class.as_str());
        let fill_matches = emitted_fill == expected.fill.as_ref().map(|fill| fill.css.as_ref());
        let radius_matches =
            emitted_radius_x == expected.radius_px && emitted_radius_y == expected.radius_px;
        let terminal_matches = expected.semantic_id.as_ref() == semantic_id
            && expected_terminal_id == terminal_id
            && class_matches
            && fill_matches
            && radius_matches
            && self.terminal_ids.insert(terminal_id.into());
        self.terminals_match &= terminal_matches;

        if terminal_matches {
            if let Some(rule_index) = expected.radius_rule_index {
                self.radius_rules.insert(rule_index);
            }
            if let Some(rule_index) = expected
                .fill
                .as_ref()
                .and_then(GanttTaskFillExpectation::typed_rule_index)
            {
                self.fill_rules.insert(rule_index);
            }
        }
    }

    fn proves_complete(&self) -> bool {
        self.terminals_match
            && self.expectations.len() == self.checkpointed_tasks.len()
            && self.terminal_ids.len() == self.expectations.len()
            && self
                .checkpointed_tasks
                .iter()
                .all(|checkpointed| *checkpointed)
    }

    fn proves_rule(&self, rule_index: usize, pending: &GanttTaskPendingEvidence) -> bool {
        self.proves_complete()
            && (!pending.radius || self.radius_rules.contains(&rule_index))
            && (!pending.fill || self.fill_rules.contains(&rule_index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, ThemeGeometryPatch, ThemeRule, ThemeRuleSet,
        ThemeStylePatch, ThemeVariant,
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
    fn task_bar_state_has_one_precedence_authority() {
        for (task, expected) in [
            (GanttRenderTask::default(), GanttTaskBarState::Default),
            (
                GanttRenderTask {
                    crit: true,
                    ..GanttRenderTask::default()
                },
                GanttTaskBarState::Crit,
            ),
            (
                GanttRenderTask {
                    done: true,
                    crit: true,
                    ..GanttRenderTask::default()
                },
                GanttTaskBarState::DoneCrit,
            ),
            (
                GanttRenderTask {
                    active: true,
                    done: true,
                    crit: true,
                    ..GanttRenderTask::default()
                },
                GanttTaskBarState::ActiveCrit,
            ),
        ] {
            assert_eq!(GanttTaskBarState::from_task(&task), expected);
        }
    }

    #[test]
    fn terminal_receipt_rejects_missing_duplicate_and_wrong_terminals() {
        let expectation = GanttTaskTerminalExpectation {
            semantic_id: "task-a".into(),
            state: GanttTaskBarState::Active,
            radius_px: 7.0,
            radius_rule_index: Some(0),
            fill: Some(GanttTaskFillExpectation {
                css: "#123456".into(),
                owner: GanttTaskFillOwner::Typed {
                    rule_index: 0,
                    capability: ThemeCapability::SolidPaint,
                },
            }),
        };
        let mut missing = GanttTaskThemeReceipt::new(vec![expectation.clone()]);
        assert!(!missing.proves_complete());

        let mut wrong = GanttTaskThemeReceipt::new(vec![expectation.clone()]);
        wrong.record_checkpointed_task(
            0,
            "gantt",
            "task-a",
            "gantt-task-a",
            "0",
            "task active0",
            7.0,
            7.0,
            Some("#abcdef"),
        );
        assert!(!wrong.proves_complete());

        let mut duplicate = GanttTaskThemeReceipt::new(vec![expectation]);
        for _ in 0..2 {
            duplicate.record_checkpointed_task(
                0,
                "gantt",
                "task-a",
                "gantt-task-a",
                "0",
                "task active0",
                7.0,
                7.0,
                Some("#123456"),
            );
        }
        assert!(!duplicate.proves_complete());
    }

    #[test]
    fn layout_binding_maps_render_order_back_to_semantic_occurrences() {
        let tasks = [
            GanttRenderTask {
                id: "active".to_string(),
                active: true,
                ..GanttRenderTask::default()
            },
            GanttRenderTask {
                id: "done".to_string(),
                done: true,
                ..GanttRenderTask::default()
            },
        ];
        let theme = GanttTaskTheme::baseline(&tasks);

        assert!(theme.bind_layout_occurrences(vec![1, 0]));
        assert!(!theme.bind_layout_occurrences(vec![0, 1]));

        let invalid = GanttTaskTheme::baseline(&tasks);
        assert!(!invalid.bind_layout_occurrences(vec![0, 0]));
        assert!(!invalid.bind_layout_occurrences(vec![0, 2]));
    }

    #[test]
    fn secondary_state_winner_cannot_claim_terminal_radius_evidence() {
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
            id: "task".to_string(),
            active: true,
            crit: true,
            ..GanttRenderTask::default()
        };
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());

        let task_theme = GanttTaskTheme::resolve(
            Some(&resolved),
            &MermaidConfig::default(),
            &[task],
            &work_meter,
        )
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

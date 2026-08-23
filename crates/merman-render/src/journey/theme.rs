use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, Specified,
    ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    resolve_direct_static_fill, resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::model::JourneyTaskLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

const MERMAID_TASK_RADIUS_PX: f64 = 3.0;

#[derive(Debug)]
pub(crate) struct JourneyTaskTheme {
    tasks: Box<[JourneyTaskTerminalExpectation]>,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, JourneyTaskPendingEvidence>,
    terminal_receipt: OnceLock<JourneyTaskRadiusThemeReceipt>,
}

#[derive(Debug, Clone)]
struct JourneyTaskTerminalExpectation {
    radius: JourneyTaskRadius,
    radius_rule_index: Option<usize>,
    fill: Option<JourneyTaskPaint>,
    stroke: Option<JourneyTaskPaint>,
}

impl JourneyTaskTerminalExpectation {
    fn baseline() -> Self {
        Self {
            radius: JourneyTaskRadius::baseline(),
            radius_rule_index: None,
            fill: None,
            stroke: None,
        }
    }
}

#[derive(Debug, Clone)]
struct JourneyTaskRadius {
    token: Box<str>,
}

#[derive(Debug, Clone)]
struct JourneyTaskPaint {
    css: Box<str>,
    rule_index: usize,
    capability: ThemeCapability,
}

impl JourneyTaskTheme {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        tasks: &[JourneyTaskLayout],
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::baseline(tasks));
        };

        let task_count = tasks.len();
        let static_style = theme.style_with_work_meter(
            ThemeTarget::JourneyTask,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let static_winners = static_style
            .winner_rule_properties()
            .into_iter()
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect::<BTreeSet<_>>();

        let mut task_expectations = tasks
            .iter()
            .map(|_| JourneyTaskTerminalExpectation::baseline())
            .collect::<Vec<_>>();
        let mut occurrence_winners = BTreeSet::new();
        let mut occurrence_radius_winners = BTreeMap::<usize, usize>::new();
        let mut occurrence_fill_winners = BTreeMap::<usize, usize>::new();
        let mut occurrence_stroke_winners = BTreeMap::<usize, usize>::new();
        let mut radius_rules = BTreeSet::new();
        let mut typed_fill_rules = BTreeSet::new();
        let mut source_owned_fill_rules = BTreeSet::new();
        let mut typed_stroke_rules = BTreeSet::new();

        for (task_index, (task, expectation)) in
            tasks.iter().zip(task_expectations.iter_mut()).enumerate()
        {
            let style = theme.style_with_work_meter(
                ThemeTarget::JourneyTask,
                ThemeVariant::Default,
                Some(task_index + 1),
                work_meter,
            )?;
            for (property, origin) in style.winner_rule_properties() {
                let key = (origin.rule_index(), property);
                occurrence_winners.insert(key);
                match property {
                    ResolvedStyleProperty::Radius => {
                        *occurrence_radius_winners
                            .entry(origin.rule_index())
                            .or_default() += 1;
                    }
                    ResolvedStyleProperty::Fill => {
                        *occurrence_fill_winners
                            .entry(origin.rule_index())
                            .or_default() += 1;
                    }
                    ResolvedStyleProperty::Stroke => {
                        *occurrence_stroke_winners
                            .entry(origin.rule_index())
                            .or_default() += 1;
                    }
                    _ => {}
                }
            }

            if let Some((radius, rule_index)) = typed_radius(theme, &style) {
                radius_rules.insert(rule_index);
                expectation.radius_rule_index = Some(rule_index);
                expectation.radius = radius;
            }

            if let Some(fill) = resolve_direct_static_fill(
                theme,
                &style,
                &[ThemeTarget::JourneyTask],
                DirectStaticSelectorDomain::Unqualified,
            ) {
                if journey_fill_source_owned(effective_config, task.num) {
                    source_owned_fill_rules.insert(fill.rule_index());
                } else {
                    typed_fill_rules.insert(fill.rule_index());
                    expectation.fill = Some(JourneyTaskPaint {
                        css: fill.css().to_owned().into_boxed_str(),
                        rule_index: fill.rule_index(),
                        capability: fill.capability(),
                    });
                }
            }

            if let Some(stroke) = resolve_direct_static_stroke(
                theme,
                &style,
                &[ThemeTarget::JourneyTask],
                DirectStaticSelectorDomain::Unqualified,
            ) {
                typed_stroke_rules.insert(stroke.rule_index());
                expectation.stroke = Some(JourneyTaskPaint {
                    css: stroke.css().to_owned().into_boxed_str(),
                    rule_index: stroke.rule_index(),
                    capability: stroke.capability(),
                });
            }
        }

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<usize, JourneyTaskRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::JourneyTask,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    let property = resolved_style_property_for_facet(facet);
                    if !selector.ordinal_domain_intersects_occurrence_count(task_count) {
                        continue;
                    }
                    let qualified_variant = matches!(
                        selector,
                        FamilyThemeSelectorShape::Static { variant: Some(_) }
                            | FamilyThemeSelectorShape::Ordinal {
                                variant: Some(_),
                                ..
                            }
                    );
                    let route_won = match selector {
                        FamilyThemeSelectorShape::Static { variant: None } => {
                            static_winners.contains(&(rule_index, property))
                        }
                        FamilyThemeSelectorShape::Static { variant: Some(_) }
                        | FamilyThemeSelectorShape::Ordinal { .. } => {
                            occurrence_winners.contains(&(rule_index, property))
                        }
                    };
                    if !route_won && !qualified_variant {
                        continue;
                    }

                    observation.applicable = true;
                    match (route.disposition(), selector, facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Radius,
                        ) => {
                            if occurrence_radius_winners.get(&rule_index).copied()
                                == Some(task_count)
                                && radius_rules.contains(&rule_index)
                            {
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
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Fill(_),
                        ) => {
                            let wins = occurrence_fill_winners.get(&rule_index).copied();
                            let typed = typed_fill_rules.contains(&rule_index);
                            let source_owned = source_owned_fill_rules.contains(&rule_index);
                            if wins == Some(task_count) && (typed || source_owned) {
                                if typed {
                                    observation.pending.fill = true;
                                    observation.pending.capabilities.insert(
                                        task_expectations
                                            .iter()
                                            .filter_map(|task| task.fill.as_ref())
                                            .find(|paint| paint.rule_index == rule_index)
                                            .map_or(ThemeCapability::SolidPaint, |paint| {
                                                paint.capability
                                            }),
                                    );
                                }
                                if source_owned && !typed {
                                    observation.suppressed = true;
                                }
                            } else {
                                observation.incomplete = true;
                            }
                        }
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Stroke(_),
                        ) => {
                            if occurrence_stroke_winners.get(&rule_index).copied()
                                == Some(task_count)
                                && typed_stroke_rules.contains(&rule_index)
                            {
                                observation.pending.stroke = true;
                                observation.pending.capabilities.insert(
                                    task_expectations
                                        .iter()
                                        .filter_map(|task| task.stroke.as_ref())
                                        .find(|paint| paint.rule_index == rule_index)
                                        .map_or(ThemeCapability::SolidPaint, |paint| {
                                            paint.capability
                                        }),
                                );
                            } else {
                                observation.incomplete = true;
                            }
                        }
                        (FamilyThemeDisposition::Unsupported, _, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::TypedAdapter, _, _)
                        | (FamilyThemeDisposition::LegacyCompatibility, _, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::JourneyTask,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if task_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::JourneyTask,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if task_count == 0 {
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
                target: ThemeTarget::JourneyTask,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Mixed or partially shadowed rules remain fail-closed until every winning facet
                // has one terminal owner.
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
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline(tasks: &[JourneyTaskLayout]) -> Self {
        Self {
            tasks: tasks
                .iter()
                .map(|_| JourneyTaskTerminalExpectation::baseline())
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            evidence: FamilyThemeEvidence::default(),
            pending: BTreeMap::new(),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) const fn task_count(&self) -> usize {
        self.tasks.len()
    }

    pub(crate) fn terminal_radius_for_task(&self, task_index: usize) -> &str {
        self.tasks
            .get(task_index)
            .map_or("3", |task| task.radius.token.as_ref())
    }

    pub(crate) fn terminal_fill_for_task(&self, task_index: usize) -> Option<(usize, &str)> {
        self.tasks
            .get(task_index)
            .and_then(|task| task.fill.as_ref())
            .map(|paint| (paint.rule_index, paint.css.as_ref()))
    }

    pub(crate) fn terminal_stroke_for_task(&self, task_index: usize) -> Option<(usize, &str)> {
        self.tasks
            .get(task_index)
            .and_then(|task| task.stroke.as_ref())
            .map(|paint| (paint.rule_index, paint.css.as_ref()))
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<JourneyTaskRadiusThemeReceipt> {
        let requires_receipt = !self.pending.is_empty()
            || self
                .tasks
                .iter()
                .any(|task| task.fill.is_some() || task.stroke.is_some());
        requires_receipt
            .then(|| JourneyTaskRadiusThemeReceipt::from_expectations(self.tasks.clone()))
    }

    pub(crate) fn record_terminal(&self, receipt: JourneyTaskRadiusThemeReceipt) -> bool {
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
}

impl JourneyTaskRadius {
    fn baseline() -> Self {
        Self {
            token: MERMAID_TASK_RADIUS_PX.to_string().into_boxed_str(),
        }
    }
}

fn typed_radius(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> Option<(JourneyTaskRadius, usize)> {
    let origin = style.radius_resolution().winner()?;
    if theme.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::Radius)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    match style.radius_resolution().specified() {
        Specified::Value(value) => Some((
            JourneyTaskRadius {
                token: value.to_string().into_boxed_str(),
            },
            origin.rule_index(),
        )),
        Specified::Clear => Some((JourneyTaskRadius::baseline(), origin.rule_index())),
        Specified::Unspecified => None,
    }
}

fn journey_fill_source_owned(config: &MermaidConfig, task_num: i64) -> bool {
    let path = format!("themeVariables.fillType{}", task_num.max(0));
    merman_core::__private::config_path_overrides_typed_default(config, &path)
}

/// Writer-owned proof that every Journey task emitted its resolved terminal state exactly once.
#[derive(Debug)]
pub(crate) struct JourneyTaskRadiusThemeReceipt {
    expectations: Box<[JourneyTaskTerminalExpectation]>,
    checkpointed_tasks: Vec<bool>,
    terminals_match: bool,
    radius_rules: BTreeSet<usize>,
    fill_rules: BTreeSet<usize>,
    stroke_rules: BTreeSet<usize>,
}

impl JourneyTaskRadiusThemeReceipt {
    fn new(expected_task_count: usize, expected_radius_token: Box<str>) -> Self {
        let expectations = (0..expected_task_count)
            .map(|_| JourneyTaskTerminalExpectation {
                radius: JourneyTaskRadius {
                    token: expected_radius_token.clone(),
                },
                radius_rule_index: None,
                fill: None,
                stroke: None,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self::from_expectations(expectations)
    }

    fn from_expectations(expectations: Box<[JourneyTaskTerminalExpectation]>) -> Self {
        Self {
            checkpointed_tasks: vec![false; expectations.len()],
            expectations,
            terminals_match: true,
            radius_rules: BTreeSet::new(),
            fill_rules: BTreeSet::new(),
            stroke_rules: BTreeSet::new(),
        }
    }

    pub(crate) fn record_checkpointed_task(
        &mut self,
        task_index: usize,
        emitted_rx_token: &str,
        emitted_ry_token: &str,
    ) {
        self.record_checkpointed_task_with_paint(
            task_index,
            emitted_rx_token,
            emitted_ry_token,
            None,
            None,
        );
    }

    pub(crate) fn record_checkpointed_task_with_paint(
        &mut self,
        task_index: usize,
        emitted_rx_token: &str,
        emitted_ry_token: &str,
        emitted_fill: Option<(usize, &str)>,
        emitted_stroke: Option<(usize, &str)>,
    ) {
        let Some(checkpointed) = self.checkpointed_tasks.get_mut(task_index) else {
            self.terminals_match = false;
            return;
        };
        if *checkpointed {
            self.terminals_match = false;
            return;
        }
        *checkpointed = true;

        let Some(expected) = self.expectations.get(task_index) else {
            self.terminals_match = false;
            return;
        };
        let radius_matches = emitted_rx_token == expected.radius.token.as_ref()
            && emitted_ry_token == expected.radius.token.as_ref();
        let expected_fill = expected
            .fill
            .as_ref()
            .map(|paint| (paint.rule_index, paint.css.as_ref()));
        let expected_stroke = expected
            .stroke
            .as_ref()
            .map(|paint| (paint.rule_index, paint.css.as_ref()));
        let terminal_matches =
            radius_matches && emitted_fill == expected_fill && emitted_stroke == expected_stroke;
        self.terminals_match &= terminal_matches;

        if terminal_matches {
            if let Some(rule_index) = expected.radius_rule_index {
                self.radius_rules.insert(rule_index);
            }
            if let Some(rule_index) = expected.fill.as_ref().map(|paint| paint.rule_index) {
                self.fill_rules.insert(rule_index);
            }
            if let Some(rule_index) = expected.stroke.as_ref().map(|paint| paint.rule_index) {
                self.stroke_rules.insert(rule_index);
            }
        }
    }

    fn proves_complete(&self) -> bool {
        self.terminals_match
            && self
                .checkpointed_tasks
                .iter()
                .all(|checkpointed| *checkpointed)
            && self.expectations.len() == self.checkpointed_tasks.len()
    }

    fn proves(&self, expected_task_count: usize) -> bool {
        self.proves_complete() && self.expectations.len() == expected_task_count
    }

    fn proves_rule(&self, rule_index: usize, pending: &JourneyTaskPendingEvidence) -> bool {
        self.proves_complete()
            && (!pending.radius || self.radius_rules.contains(&rule_index))
            && (!pending.fill || self.fill_rules.contains(&rule_index))
            && (!pending.stroke || self.stroke_rules.contains(&rule_index))
    }
}

#[derive(Debug, Default)]
struct JourneyTaskRuleObservation {
    applicable: bool,
    incomplete: bool,
    suppressed: bool,
    residual: Option<FamilyThemeResidualReason>,
    pending: JourneyTaskPendingEvidence,
}

#[derive(Debug, Default)]
struct JourneyTaskPendingEvidence {
    radius: bool,
    fill: bool,
    stroke: bool,
    capabilities: BTreeSet<ThemeCapability>,
}

impl JourneyTaskPendingEvidence {
    const fn requires_terminal_proof(&self) -> bool {
        self.radius || self.fill || self.stroke
    }
}

#[cfg(test)]
mod tests {
    use super::JourneyTaskRadiusThemeReceipt;

    #[test]
    fn task_radius_receipt_requires_ordered_matching_terminal_rects() {
        let mut complete = JourneyTaskRadiusThemeReceipt::new(1, "9".into());
        complete.record_checkpointed_task(0, "9", "9");
        assert!(complete.proves(1));

        let mut incomplete = JourneyTaskRadiusThemeReceipt::new(2, "9".into());
        incomplete.record_checkpointed_task(0, "9", "9");
        assert!(!incomplete.proves(2));

        let mut mismatch = JourneyTaskRadiusThemeReceipt::new(1, "9".into());
        mismatch.record_checkpointed_task(0, "9", "3");
        assert!(!mismatch.proves(1));
    }
}

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
    TerminalVariantDomain, UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolve_direct_static_fill, resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::model::JourneyTaskLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

const MERMAID_TASK_RADIUS_PX: f64 = 3.0;

#[derive(Debug)]
pub(crate) struct JourneyTaskTheme {
    tasks: Box<[JourneyTaskTerminalExpectation]>,
    sections: Box<[JourneySectionTerminalExpectation]>,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, JourneyTaskPendingEvidence>,
    palette_key: Option<FamilyThemeMechanismKey>,
    palette_surface_owned: bool,
    terminal_receipt: OnceLock<JourneyTaskThemeReceipt>,
}

#[derive(Debug, Clone)]
struct JourneyTaskTerminalExpectation {
    radius: JourneyTaskRadius,
    radius_rule_index: Option<usize>,
    fill: Option<JourneyTaskPaint>,
    stroke: Option<JourneyTaskPaint>,
    palette: Option<JourneyTaskPalettePaint>,
}

impl JourneyTaskTerminalExpectation {
    fn baseline() -> Self {
        Self {
            radius: JourneyTaskRadius::baseline(),
            radius_rule_index: None,
            fill: None,
            stroke: None,
            palette: None,
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

#[derive(Debug, Clone)]
struct JourneyTaskPalettePaint {
    css: Box<str>,
    capability: Option<ThemeCapability>,
}

impl JourneyTaskPalettePaint {
    fn typed(css: impl Into<Box<str>>, capability: ThemeCapability) -> Self {
        Self {
            css: css.into(),
            capability: Some(capability),
        }
    }

    fn source_owned(css: impl Into<Box<str>>) -> Self {
        Self {
            css: css.into(),
            capability: None,
        }
    }
}

#[derive(Debug, Clone)]
struct JourneySectionTerminalExpectation {
    palette: Option<JourneyTaskPalettePaint>,
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
        let palette_key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::JourneyTask,
        };
        let palette_disposition = theme.ordinal_palette_disposition(ThemeTarget::JourneyTask);
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
        let mut palette_surface_owned = false;
        let mut missing_palette_slot = false;

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

            if palette_disposition == Some(FamilyThemeDisposition::TypedAdapter)
                && style.fill_resolution().winner().is_none()
            {
                if journey_fill_source_owned(effective_config, task.num) {
                    if let Some(source_css) =
                        journey_palette_source_owned(effective_config, task.num)
                    {
                        expectation.palette =
                            Some(JourneyTaskPalettePaint::source_owned(source_css));
                        palette_surface_owned = true;
                    } else {
                        missing_palette_slot = true;
                    }
                } else if let Some(color) =
                    theme.series_color(ThemeTarget::JourneyTask, task.num.max(0) as usize + 1)
                {
                    expectation.palette = Some(JourneyTaskPalettePaint::typed(
                        color.as_css(),
                        if color.is_transparent() {
                            ThemeCapability::TransparentPaint
                        } else {
                            ThemeCapability::SolidPaint
                        },
                    ));
                    palette_surface_owned = true;
                } else {
                    missing_palette_slot = true;
                }
            }
        }

        let (sections, missing_section_palette_slot) =
            journey_section_expectations(theme, effective_config, tasks);
        missing_palette_slot |= missing_section_palette_slot;
        palette_surface_owned |= sections.iter().any(|section| section.palette.is_some());

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
                    match route.disposition() {
                        FamilyThemeDisposition::TypedAdapter if task_count == 0 => {
                            evidence.mark_not_applicable(key);
                        }
                        FamilyThemeDisposition::TypedAdapter if missing_palette_slot => {
                            evidence.mark_residual(
                                key,
                                FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                            );
                        }
                        FamilyThemeDisposition::TypedAdapter if palette_surface_owned => {}
                        FamilyThemeDisposition::TypedAdapter => {
                            evidence.mark_not_applicable(key);
                        }
                        FamilyThemeDisposition::Unsupported => {
                            evidence.mark_residual(
                                key,
                                FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                            );
                        }
                        FamilyThemeDisposition::LegacyCompatibility => {}
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::JourneyTask,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if task_count == 0 {
                        evidence.mark_not_applicable(key);
                    }
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        // Unsupported effect bindings must be reconciled against the final per-task winner so an
        // explicit effect rule or clear does not leave a shadowed binding as a false residual.
        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::JourneyTask,
                TerminalVariantDomain::uniform(task_count, ThemeVariant::Default),
            )],
            work_meter,
        )?;

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
            sections: sections.into_boxed_slice(),
            evidence,
            pending,
            palette_key: (palette_surface_owned && !missing_palette_slot).then_some(palette_key),
            // Keep the historical CSS fallback available when the typed palette is incomplete.
            // The route remains residual in that case, but permissive rendering should not lose
            // the unclaimed fillType slots merely because one direct slot was unavailable.
            palette_surface_owned: palette_surface_owned && !missing_palette_slot,
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
            sections: Vec::new().into_boxed_slice(),
            evidence: FamilyThemeEvidence::default(),
            pending: BTreeMap::new(),
            palette_key: None,
            palette_surface_owned: false,
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

    pub(crate) fn terminal_palette_fill_for_task(&self, task_index: usize) -> Option<&str> {
        self.tasks
            .get(task_index)
            .and_then(|task| task.palette.as_ref())
            .map(|paint| paint.css.as_ref())
    }

    pub(crate) fn terminal_fill_css_for_task(&self, task_index: usize) -> Option<&str> {
        self.terminal_fill_for_task(task_index)
            .map(|(_, css)| css)
            .or_else(|| self.terminal_palette_fill_for_task(task_index))
    }

    pub(crate) fn terminal_palette_fill_for_section(&self, section_index: usize) -> Option<&str> {
        self.sections
            .get(section_index)
            .and_then(|section| section.palette.as_ref())
            .map(|paint| paint.css.as_ref())
    }

    pub(crate) const fn palette_surface_owned(&self) -> bool {
        self.palette_surface_owned
    }

    pub(crate) fn terminal_stroke_for_task(&self, task_index: usize) -> Option<(usize, &str)> {
        self.tasks
            .get(task_index)
            .and_then(|task| task.stroke.as_ref())
            .map(|paint| (paint.rule_index, paint.css.as_ref()))
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<JourneyTaskThemeReceipt> {
        let requires_receipt = !self.pending.is_empty()
            || self
                .tasks
                .iter()
                .any(|task| task.fill.is_some() || task.stroke.is_some() || task.palette.is_some())
            || self
                .sections
                .iter()
                .any(|section| section.palette.is_some());
        requires_receipt.then(|| {
            JourneyTaskThemeReceipt::from_expectations(self.tasks.clone(), self.sections.clone())
        })
    }

    pub(crate) fn record_terminal(&self, receipt: JourneyTaskThemeReceipt) -> bool {
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
                FamilyThemeMechanismKey::Typography(_)
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
        if let Some(key) = self.palette_key.clone() {
            if receipt.palette_capabilities.is_empty() {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_applied_with_capabilities(
                    key,
                    receipt.palette_capabilities.iter().copied(),
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

fn journey_palette_source_owned(config: &MermaidConfig, task_num: i64) -> Option<Box<str>> {
    let path = format!("themeVariables.fillType{}", task_num.max(0));
    merman_core::__private::config_path_overrides_typed_default(config, &path)
        .then(|| {
            config
                .get_str(&path)
                .map(str::to_owned)
                .map(String::into_boxed_str)
        })
        .flatten()
}

fn journey_section_expectations(
    theme: &ResolvedDiagramTheme,
    effective_config: &MermaidConfig,
    tasks: &[JourneyTaskLayout],
) -> (Vec<JourneySectionTerminalExpectation>, bool) {
    let mut sections = Vec::new();
    let mut missing_palette_slot = false;
    let mut section_start = 0usize;
    while section_start < tasks.len() {
        let section_name = tasks[section_start].section.as_str();
        let mut section_end = section_start + 1;
        while section_end < tasks.len() && tasks[section_end].section == section_name {
            section_end += 1;
        }

        let section_num = tasks[section_start].num;
        let palette = if journey_fill_source_owned(effective_config, section_num) {
            journey_palette_source_owned(effective_config, section_num)
                .map(JourneyTaskPalettePaint::source_owned)
        } else {
            theme
                .series_color(ThemeTarget::JourneyTask, section_num.max(0) as usize + 1)
                .map(|color| {
                    JourneyTaskPalettePaint::typed(
                        color.as_css(),
                        if color.is_transparent() {
                            ThemeCapability::TransparentPaint
                        } else {
                            ThemeCapability::SolidPaint
                        },
                    )
                })
        };
        if palette.is_none() {
            missing_palette_slot = true;
        }
        sections.push(JourneySectionTerminalExpectation { palette });
        section_start = section_end;
    }
    (sections, missing_palette_slot)
}

/// Writer-owned proof that every Journey task emitted its resolved terminal state exactly once.
#[derive(Debug)]
pub(crate) struct JourneyTaskThemeReceipt {
    expectations: Box<[JourneyTaskTerminalExpectation]>,
    section_expectations: Box<[JourneySectionTerminalExpectation]>,
    checkpointed_tasks: Vec<bool>,
    checkpointed_sections: Vec<bool>,
    terminals_match: bool,
    radius_rules: BTreeSet<usize>,
    fill_rules: BTreeSet<usize>,
    stroke_rules: BTreeSet<usize>,
    palette_capabilities: BTreeSet<ThemeCapability>,
}

impl JourneyTaskThemeReceipt {
    fn from_expectations(
        expectations: Box<[JourneyTaskTerminalExpectation]>,
        section_expectations: Box<[JourneySectionTerminalExpectation]>,
    ) -> Self {
        Self {
            checkpointed_tasks: vec![false; expectations.len()],
            checkpointed_sections: vec![false; section_expectations.len()],
            expectations,
            section_expectations,
            terminals_match: true,
            radius_rules: BTreeSet::new(),
            fill_rules: BTreeSet::new(),
            stroke_rules: BTreeSet::new(),
            palette_capabilities: BTreeSet::new(),
        }
    }

    pub(crate) fn record_checkpointed_task_with_terminal_paint(
        &mut self,
        task_index: usize,
        emitted_rx_token: &str,
        emitted_ry_token: &str,
        emitted_fill: Option<&str>,
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
            .map(|paint| paint.css.as_ref())
            .or_else(|| expected.palette.as_ref().map(|paint| paint.css.as_ref()));
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
            if let Some(capability) = expected.palette.as_ref().and_then(|paint| paint.capability) {
                self.palette_capabilities.insert(capability);
            }
        }
    }

    pub(crate) fn record_checkpointed_section(
        &mut self,
        section_index: usize,
        emitted_fill: Option<&str>,
    ) {
        let Some(checkpointed) = self.checkpointed_sections.get_mut(section_index) else {
            self.terminals_match = false;
            return;
        };
        if *checkpointed {
            self.terminals_match = false;
            return;
        }
        *checkpointed = true;

        let Some(expected) = self.section_expectations.get(section_index) else {
            self.terminals_match = false;
            return;
        };
        let expected_fill = expected.palette.as_ref().map(|paint| paint.css.as_ref());
        let terminal_matches = emitted_fill == expected_fill;
        self.terminals_match &= terminal_matches;
        if terminal_matches {
            if let Some(capability) = expected.palette.as_ref().and_then(|paint| paint.capability) {
                self.palette_capabilities.insert(capability);
            }
        }
    }

    fn proves_complete(&self) -> bool {
        self.terminals_match
            && self
                .checkpointed_tasks
                .iter()
                .all(|checkpointed| *checkpointed)
            && self
                .checkpointed_sections
                .iter()
                .all(|checkpointed| *checkpointed)
            && self.expectations.len() == self.checkpointed_tasks.len()
            && self.section_expectations.len() == self.checkpointed_sections.len()
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
    use super::{JourneyTaskTerminalExpectation, JourneyTaskTheme, JourneyTaskThemeReceipt};
    use crate::diagram_theme::{
        DiagramEffectSet, DiagramThemeCompiler, DiagramThemeSpec, EffectBinding, EffectGraph,
        EffectInput, EffectPrimitive, FamilyThemeMechanismKey, ThemeRule, ThemeRuleSet,
        ThemeStylePatch, ThemeTarget,
    };
    use crate::model::{JourneyMouthKind, JourneyTaskLayout};
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
    use merman_core::MermaidConfig;

    fn one_task() -> JourneyTaskLayout {
        JourneyTaskLayout {
            index: 0,
            section: "Delivery".to_string(),
            task: "Ship safely".to_string(),
            score: 5,
            x: 0.0,
            y: 0.0,
            width: 150.0,
            height: 50.0,
            fill: "#ccc".to_string(),
            num: 0,
            people: Vec::new(),
            actor_circles: Vec::new(),
            line_id: "task0".to_string(),
            line_x1: 0.0,
            line_y1: 0.0,
            line_x2: 0.0,
            line_y2: 0.0,
            face_cx: 0.0,
            face_cy: Some(0.0),
            mouth: JourneyMouthKind::Smile,
        }
    }

    #[test]
    fn task_receipt_requires_ordered_matching_terminals() {
        let receipt = |task_count| {
            JourneyTaskThemeReceipt::from_expectations(
                (0..task_count)
                    .map(|_| JourneyTaskTerminalExpectation::baseline())
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                Vec::new().into_boxed_slice(),
            )
        };

        let mut complete = receipt(1);
        complete.record_checkpointed_task_with_terminal_paint(0, "3", "3", None, None);
        assert!(complete.proves_complete());

        let mut incomplete = receipt(2);
        incomplete.record_checkpointed_task_with_terminal_paint(0, "3", "3", None, None);
        assert!(!incomplete.proves_complete());

        let mut mismatch = receipt(1);
        mismatch.record_checkpointed_task_with_terminal_paint(0, "3", "9", None, None);
        assert!(!mismatch.proves_complete());
    }

    #[test]
    fn explicit_journey_effect_rule_shadows_unsupported_binding() {
        let effects = DiagramEffectSet::default()
            .with_graph(
                EffectGraph::new(
                    "bound",
                    [EffectPrimitive::GaussianBlur {
                        input: EffectInput::SourceGraphic,
                        std_deviation: 1.0,
                    }],
                )
                .expect("valid bound Journey effect"),
            )
            .expect("unique bound Journey effect")
            .with_graph(
                EffectGraph::new(
                    "explicit",
                    [EffectPrimitive::GaussianBlur {
                        input: EffectInput::SourceGraphic,
                        std_deviation: 2.0,
                    }],
                )
                .expect("valid explicit Journey effect"),
            )
            .expect("unique explicit Journey effect")
            .with_binding(
                EffectBinding::new(ThemeTarget::JourneyTask, "bound")
                    .expect("valid Journey effect binding"),
            )
            .expect("unique Journey effect binding");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_styles(
                        ThemeRuleSet::default().with_rule(
                            ThemeRule::new(
                                ThemeTarget::JourneyTask,
                                ThemeStylePatch::default()
                                    .with_effect("explicit")
                                    .expect("valid explicit Journey effect rule"),
                            )
                            .for_family(crate::DiagramFamilyId::JOURNEY),
                        ),
                    )
                    .with_effects(effects),
            )
            .expect("compile Journey effect shadow fixture")
            .resolve(crate::DiagramFamilyId::JOURNEY);
        let task_theme = JourneyTaskTheme::resolve(
            Some(&theme),
            &MermaidConfig::empty_object(),
            &[one_task()],
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .expect("resolve Journey effect shadow fixture");
        let evidence = task_theme.finish_evidence();
        let binding_key = FamilyThemeMechanismKey::EffectBinding {
            target: ThemeTarget::JourneyTask,
            effect_id: "bound".to_string(),
        };
        assert_eq!(evidence.not_applicable_mechanisms(), &[binding_key]);
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            crate::family::FamilyThemeResidualReason::UnsupportedEffect
        );
    }
}

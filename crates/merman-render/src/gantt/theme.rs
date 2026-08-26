use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;
use merman_core::diagrams::gantt::GanttRenderTask;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeStyle,
    Specified, ThemeCapability, ThemeTarget, ThemeTypographyProperty,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    InheritedFontStackOutcome, TerminalVariantDomain, UnsupportedTerminalDomain,
    reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolve_direct_static_stroke, resolved_style_property_for_facet,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GanttTaskStrokeOwner {
    Mermaid,
    Typed {
        rule_index: usize,
        capability: ThemeCapability,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GanttTaskStrokeExpectation {
    css: Box<str>,
    owner: GanttTaskStrokeOwner,
}

impl GanttTaskStrokeExpectation {
    const fn typed_rule_index(&self) -> Option<usize> {
        match self.owner {
            GanttTaskStrokeOwner::Mermaid => None,
            GanttTaskStrokeOwner::Typed { rule_index, .. } => Some(rule_index),
        }
    }

    const fn typed_capability(&self) -> Option<ThemeCapability> {
        match self.owner {
            GanttTaskStrokeOwner::Mermaid => None,
            GanttTaskStrokeOwner::Typed { capability, .. } => Some(capability),
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
    stroke: Option<GanttTaskStrokeExpectation>,
}

impl GanttTaskTerminalExpectation {
    fn baseline(task: &GanttRenderTask) -> Self {
        Self {
            semantic_id: task.id.clone().into_boxed_str(),
            state: GanttTaskBarState::from_task(task),
            radius_px: MERMAID_TASK_RADIUS_PX,
            radius_rule_index: None,
            fill: None,
            stroke: None,
        }
    }
}

/// Gantt task geometry, paint, and terminal evidence resolved once for semantic occurrences.
#[derive(Debug)]
pub(crate) struct GanttTaskTheme {
    tasks: Box<[GanttTaskTerminalExpectation]>,
    font_family_css: Box<str>,
    typography_outcome: InheritedFontStackOutcome,
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
        let (font_family_css, typography_outcome) =
            resolve_gantt_font_stack(theme, effective_config);
        let Some(theme) = theme else {
            let mut baseline = Self::baseline(tasks);
            baseline.font_family_css = font_family_css;
            baseline.typography_outcome = typography_outcome;
            return Ok(baseline);
        };

        let mut task_expectations = tasks
            .iter()
            .map(GanttTaskTerminalExpectation::baseline)
            .collect::<Vec<_>>();
        let mut winner_properties = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut radius_rules = BTreeSet::new();
        let mut typed_fill_capabilities = BTreeMap::<usize, ThemeCapability>::new();
        let mut source_owned_fill_rules = BTreeSet::new();
        let mut typed_stroke_capabilities = BTreeMap::<usize, ThemeCapability>::new();
        let mut source_owned_stroke_rules = BTreeSet::new();

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
            expectation.stroke =
                typed_stroke_expectation(theme, effective_config, expectation.state, &style)?;
            if let Some(stroke) = &expectation.stroke {
                if let (Some(rule_index), Some(capability)) =
                    (stroke.typed_rule_index(), stroke.typed_capability())
                {
                    typed_stroke_capabilities.insert(rule_index, capability);
                } else if let Some(origin) = style.stroke_resolution().winner() {
                    source_owned_stroke_rules.insert(origin.rule_index());
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
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeRuleFacet::Stroke(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) => {
                            if let Some(capability) = typed_stroke_capabilities.get(&rule_index) {
                                observation.pending.stroke = true;
                                observation.pending.capabilities.insert(*capability);
                            } else if source_owned_stroke_rules.contains(&rule_index) {
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
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Task,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if tasks.is_empty() {
                        evidence.mark_not_applicable(key);
                    }
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        let task_variants = task_expectations
            .iter()
            .map(|task| task.state.theme_variant())
            .collect::<Vec<_>>();
        let source_owned_fills = tasks
            .iter()
            .map(|task| {
                merman_core::__private::config_path_overrides_typed_default(
                    effective_config,
                    GanttTaskBarState::from_task(task).final_fill_path(),
                )
            })
            .collect::<Vec<_>>();
        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Task,
                TerminalVariantDomain::per_occurrence(&task_variants),
            )
            .with_source_owned_fill(&source_owned_fills)],
            work_meter,
        )?;

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
            font_family_css,
            typography_outcome,
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
            font_family_css: crate::config::MERMAID_DEFAULT_FONT_FAMILY_CSS.into(),
            typography_outcome: InheritedFontStackOutcome::Inactive,
            evidence: FamilyThemeEvidence::default(),
            pending: BTreeMap::new(),
            layout_occurrences: OnceLock::new(),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn task_count(&self) -> usize {
        self.tasks.len()
    }

    pub(crate) fn font_family_css(&self) -> &str {
        &self.font_family_css
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

    pub(crate) fn terminal_stroke_for_layout_task(&self, layout_index: usize) -> Option<&str> {
        self.layout_task(layout_index)
            .and_then(|task| task.stroke.as_ref())
            .map(|stroke| stroke.css.as_ref())
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<GanttTaskThemeReceipt> {
        let requires_receipt = !self.pending.is_empty()
            || self
                .tasks
                .iter()
                .any(|task| task.fill.is_some() || task.stroke.is_some())
            || self.typography_outcome == InheritedFontStackOutcome::Typed;
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
                GanttTaskThemeReceipt::new_with_typography(
                    expectations,
                    self.font_family_css.as_ref(),
                    self.typography_outcome == InheritedFontStackOutcome::Typed,
                )
            }
        })
    }

    pub(crate) fn record_terminal(&self, receipt: GanttTaskThemeReceipt) -> bool {
        receipt.proves_complete() && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        for (key, pending) in &self.pending {
            let Some(receipt) = self.terminal_receipt.get() else {
                break;
            };
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
        let typography_key = FamilyThemeMechanismKey::Typography;
        match self.typography_outcome {
            InheritedFontStackOutcome::Typed => {
                if self
                    .terminal_receipt
                    .get()
                    .is_some_and(GanttTaskThemeReceipt::proves_typography)
                {
                    evidence.mark_applied_with_capabilities(
                        typography_key,
                        [ThemeCapability::Typography],
                    );
                } else {
                    evidence.mark_residual(
                        typography_key,
                        FamilyThemeResidualReason::UnsupportedTypography,
                    );
                }
            }
            InheritedFontStackOutcome::ConfigOwned => evidence.mark_not_applicable(typography_key),
            InheritedFontStackOutcome::Unsupported => evidence.mark_residual(
                typography_key,
                FamilyThemeResidualReason::UnsupportedTypography,
            ),
            InheritedFontStackOutcome::Inactive => {}
        }
        evidence
    }

    fn layout_task(&self, layout_index: usize) -> Option<&GanttTaskTerminalExpectation> {
        let semantic_index = *self.layout_occurrences.get()?.get(layout_index)?;
        self.tasks.get(semantic_index)
    }
}

fn gantt_config_font_family_css(config: &MermaidConfig) -> String {
    let value = config
        .as_value()
        .get("gantt")
        .and_then(|gantt| gantt.get("fontFamily"))
        .and_then(serde_json::Value::as_str)
        .map(crate::config::normalize_css_font_family)
        .filter(|value| !value.is_empty());
    value.unwrap_or_else(|| crate::config::config_font_family_css(config.as_value()))
}

fn gantt_config_owns_font_stack(config: &MermaidConfig) -> bool {
    [
        "gantt.fontFamily",
        "themeVariables.fontFamily",
        "fontFamily",
    ]
    .into_iter()
    .any(|path| merman_core::__private::config_path_overrides_typed_default(config, path))
}

fn resolve_gantt_font_stack(
    theme: Option<&ResolvedDiagramTheme>,
    effective_config: &MermaidConfig,
) -> (Box<str>, InheritedFontStackOutcome) {
    let configured_font = gantt_config_font_family_css(effective_config);
    let Some(theme) = theme else {
        return (
            configured_font.into_boxed_str(),
            InheritedFontStackOutcome::Inactive,
        );
    };

    let mut typed_font_stack = false;
    let mut unsupported_typography = false;
    for route in theme.family_mechanism_routes().iter().copied() {
        match route.mechanism() {
            FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
                if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
            {
                typed_font_stack = true;
            }
            FamilyThemeMechanism::BaseTypography(_)
                if route.disposition() == FamilyThemeDisposition::Unsupported =>
            {
                unsupported_typography = true;
            }
            FamilyThemeMechanism::BaseTypography(_)
            | FamilyThemeMechanism::RuleFacet { .. }
            | FamilyThemeMechanism::OrdinalPalette { .. }
            | FamilyThemeMechanism::EffectBinding { .. } => {}
        }
    }

    let config_owned = gantt_config_owns_font_stack(effective_config);
    let font_family = if typed_font_stack && !config_owned && !unsupported_typography {
        theme.typography().font_stack().as_css()
    } else {
        configured_font
    };
    let outcome = if unsupported_typography {
        InheritedFontStackOutcome::Unsupported
    } else if typed_font_stack && config_owned {
        InheritedFontStackOutcome::ConfigOwned
    } else if typed_font_stack {
        InheritedFontStackOutcome::Typed
    } else {
        InheritedFontStackOutcome::Inactive
    };
    (font_family.into(), outcome)
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
    let Some(typed_fill) = resolve_direct_static_fill(
        theme,
        style,
        &[ThemeTarget::Task],
        DirectStaticSelectorDomain::State(state.theme_variant()),
    ) else {
        return Ok(None);
    };
    let (typed_css, rule_index, capability) = typed_fill.into_parts();

    let final_fill_path = state.final_fill_path();
    let owner = if merman_core::__private::config_path_overrides_typed_default(
        effective_config,
        final_fill_path,
    ) {
        GanttTaskFillOwner::Mermaid
    } else {
        GanttTaskFillOwner::Typed {
            rule_index,
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
            .into(),
        GanttTaskFillOwner::Typed { .. } => typed_css,
    };

    Ok(Some(GanttTaskFillExpectation { css, owner }))
}

fn typed_stroke_expectation(
    theme: &ResolvedDiagramTheme,
    effective_config: &MermaidConfig,
    state: GanttTaskBarState,
    style: &ResolvedThemeStyle,
) -> crate::Result<Option<GanttTaskStrokeExpectation>> {
    let Some(typed_stroke) = resolve_direct_static_stroke(
        theme,
        style,
        &[ThemeTarget::Task],
        DirectStaticSelectorDomain::State(state.theme_variant()),
    ) else {
        return Ok(None);
    };
    let (typed_css, rule_index, capability) = typed_stroke.into_parts();

    let final_stroke_path = state.final_stroke_path();
    let owner = if merman_core::__private::config_path_overrides_typed_default(
        effective_config,
        final_stroke_path,
    ) {
        GanttTaskStrokeOwner::Mermaid
    } else {
        GanttTaskStrokeOwner::Typed {
            rule_index,
            capability,
        }
    };
    let css = match owner {
        GanttTaskStrokeOwner::Mermaid => effective_config
            .get_str(final_stroke_path)
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!(
                    "Gantt terminal stroke owner `{final_stroke_path}` had no effective value"
                ),
            })?
            .into(),
        GanttTaskStrokeOwner::Typed { .. } => typed_css,
    };

    Ok(Some(GanttTaskStrokeExpectation { css, owner }))
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
    stroke: bool,
    capabilities: BTreeSet<ThemeCapability>,
}

impl GanttTaskPendingEvidence {
    const fn requires_terminal_proof(&self) -> bool {
        self.radius || self.fill || self.stroke
    }
}

/// Writer-owned proof that every real Gantt task rect reached its canonical terminal state.
#[derive(Debug)]
pub(crate) struct GanttTaskThemeReceipt {
    expectations: Box<[GanttTaskTerminalExpectation]>,
    checkpointed_tasks: Vec<bool>,
    terminals_match: bool,
    expected_font_family_css: Option<Box<str>>,
    typography_required: bool,
    typography_css_recorded: bool,
    typography_css_matches: bool,
    typography_text_count: usize,
    terminal_ids: BTreeSet<Box<str>>,
    radius_rules: BTreeSet<usize>,
    fill_rules: BTreeSet<usize>,
    stroke_rules: BTreeSet<usize>,
}

impl GanttTaskThemeReceipt {
    #[cfg(test)]
    fn new(expectations: Vec<GanttTaskTerminalExpectation>) -> Self {
        Self::new_with_typography(expectations, "", false)
    }

    fn new_with_typography(
        expectations: Vec<GanttTaskTerminalExpectation>,
        expected_font_family_css: &str,
        typography_required: bool,
    ) -> Self {
        Self {
            checkpointed_tasks: vec![false; expectations.len()],
            expectations: expectations.into_boxed_slice(),
            terminals_match: true,
            expected_font_family_css: typography_required.then(|| expected_font_family_css.into()),
            typography_required,
            typography_css_recorded: false,
            typography_css_matches: true,
            typography_text_count: 0,
            terminal_ids: BTreeSet::new(),
            radius_rules: BTreeSet::new(),
            fill_rules: BTreeSet::new(),
            stroke_rules: BTreeSet::new(),
        }
    }

    fn invalid(expected_task_count: usize) -> Self {
        Self {
            expectations: Vec::new().into_boxed_slice(),
            checkpointed_tasks: vec![false; expected_task_count],
            terminals_match: false,
            expected_font_family_css: None,
            typography_required: false,
            typography_css_recorded: false,
            typography_css_matches: false,
            typography_text_count: 0,
            terminal_ids: BTreeSet::new(),
            radius_rules: BTreeSet::new(),
            fill_rules: BTreeSet::new(),
            stroke_rules: BTreeSet::new(),
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
        emitted_stroke: Option<&str>,
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
        let stroke_matches =
            emitted_stroke == expected.stroke.as_ref().map(|stroke| stroke.css.as_ref());
        let radius_matches =
            emitted_radius_x == expected.radius_px && emitted_radius_y == expected.radius_px;
        let terminal_matches = expected.semantic_id.as_ref() == semantic_id
            && expected_terminal_id == terminal_id
            && class_matches
            && fill_matches
            && stroke_matches
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
            if let Some(rule_index) = expected
                .stroke
                .as_ref()
                .and_then(GanttTaskStrokeExpectation::typed_rule_index)
            {
                self.stroke_rules.insert(rule_index);
            }
        }
    }

    pub(crate) fn record_typography_css(&mut self, emitted_font_family_css: &str) {
        if !self.typography_required || self.typography_css_recorded {
            self.typography_css_matches = false;
            return;
        }
        self.typography_css_recorded = true;
        self.typography_css_matches = self
            .expected_font_family_css
            .as_deref()
            .is_some_and(|expected| expected == emitted_font_family_css);
    }

    pub(crate) fn record_typography_text(&mut self, text: &str) {
        if self.typography_required && !text.trim().is_empty() {
            self.typography_text_count = self.typography_text_count.saturating_add(1);
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
            && (!pending.stroke || self.stroke_rules.contains(&rule_index))
    }

    fn proves_typography(&self) -> bool {
        !self.typography_required
            || (self.typography_css_recorded
                && self.typography_css_matches
                && self.typography_text_count != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, FamilyThemeMechanismKey, FontStack,
        OrdinalPalette, ThemeColorValue, ThemeGeometryPatch, ThemeRule, ThemeRuleSet,
        ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
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

    fn font_stack_spec(name: &str) -> DiagramThemeSpec {
        DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(
                crate::DiagramFamilyId::GANTT,
                ThemeTextStyle::default()
                    .with_font_stack(FontStack::single(name).expect("valid font stack")),
            ),
        )
    }

    #[test]
    fn typed_font_stack_is_shared_by_gantt_measurement_and_terminal_plan() {
        let theme = DiagramThemeCompiler::new()
            .compile(font_stack_spec("Inter"))
            .expect("compile Gantt typography theme")
            .resolve(crate::DiagramFamilyId::GANTT);
        let task_theme = GanttTaskTheme::resolve(
            Some(&theme),
            &MermaidConfig::default(),
            &[GanttRenderTask::default()],
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .expect("resolve Gantt typography theme");

        assert_eq!(task_theme.font_family_css(), "Inter");
        assert_eq!(
            task_theme.typography_outcome,
            InheritedFontStackOutcome::Typed
        );
        assert!(task_theme.bind_layout_occurrences(vec![0]));
        let mut receipt = task_theme
            .begin_terminal_receipt()
            .expect("bound typography receipt");
        receipt.record_typography_css("Inter");
        receipt.record_typography_text("visible task label");
        receipt.record_checkpointed_task(0, "", "", "", "0", "task task0", 3.0, 3.0, None, None);
        assert!(receipt.proves_typography());
        assert!(task_theme.record_terminal(receipt));
        assert!(task_theme.finish_evidence().residuals().is_empty());
    }

    #[test]
    fn unsupported_gantt_font_sibling_fails_closed() {
        let mixed_typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Inter").expect("valid font stack"))
            .with_font_weight(700)
            .expect("valid font weight");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_typography(
                    TypographySpec::default()
                        .with_family_style(crate::DiagramFamilyId::GANTT, mixed_typography),
                ),
            )
            .expect("compile mixed Gantt typography theme")
            .resolve(crate::DiagramFamilyId::GANTT);
        let task_theme = GanttTaskTheme::resolve(
            Some(&theme),
            &MermaidConfig::default(),
            &[GanttRenderTask::default()],
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .expect("resolve mixed Gantt typography");

        assert_eq!(
            task_theme.typography_outcome,
            InheritedFontStackOutcome::Unsupported
        );
        assert!(
            task_theme
                .finish_evidence()
                .residuals()
                .iter()
                .any(|residual| {
                    residual.key() == &FamilyThemeMechanismKey::Typography
                        && residual.reason() == FamilyThemeResidualReason::UnsupportedTypography
                })
        );
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
    fn typed_task_fill_shadows_unsupported_ordinal_palette_per_state() {
        let palette = OrdinalPalette::new([
            ThemeColorValue::parse("#123456").expect("valid Gantt palette color")
        ])
        .expect("non-empty Gantt palette");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::Task,
                                ThemeStylePatch::default().with_fill(
                                    CanvasPaint::solid("#abcdef").expect("valid Gantt task fill"),
                                ),
                            )
                            .for_family(crate::DiagramFamilyId::GANTT),
                        )
                        .with_ordinal_palette(ThemeTarget::Task, palette),
                ),
            )
            .expect("compile Gantt palette shadow fixture")
            .resolve(crate::DiagramFamilyId::GANTT);
        let task = GanttRenderTask::default();
        let task_theme = GanttTaskTheme::resolve(
            Some(&theme),
            &MermaidConfig::default(),
            &[task],
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .expect("resolve Gantt palette shadow fixture");

        assert!(task_theme.evidence.not_applicable_mechanisms().contains(
            &FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::Task,
            }
        ));
        assert!(task_theme.evidence.residuals().is_empty());
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
            stroke: Some(GanttTaskStrokeExpectation {
                css: "#654321".into(),
                owner: GanttTaskStrokeOwner::Typed {
                    rule_index: 1,
                    capability: ThemeCapability::SolidPaint,
                },
            }),
        };
        let missing = GanttTaskThemeReceipt::new(vec![expectation.clone()]);
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
            Some("#654321"),
        );
        assert!(!wrong.proves_complete());

        let mut wrong_stroke = GanttTaskThemeReceipt::new(vec![expectation.clone()]);
        wrong_stroke.record_checkpointed_task(
            0,
            "gantt",
            "task-a",
            "gantt-task-a",
            "0",
            "task active0",
            7.0,
            7.0,
            Some("#123456"),
            Some("#abcdef"),
        );
        assert!(!wrong_stroke.proves_complete());

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
                Some("#654321"),
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

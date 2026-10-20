use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, Specified, ThemeCapability, ThemeTarget,
    ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    resolve_direct_static_fill, resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::model::TimelineDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

const TIMELINE_PALETTE_SLOT_COUNT: usize = crate::timeline::config::MAX_THEME_COLOR_LIMIT;

#[derive(Debug, Clone)]
struct TimelineEventRadius {
    token: Box<str>,
    value_px: f64,
}

impl TimelineEventRadius {
    fn value(value: f32, node: &crate::model::TimelineNodeLayout) -> Self {
        let max_radius = node.width.max(1.0).min(node.height.max(1.0)) / 2.0;
        let normalized_value = if value == 0.0 { 0.0 } else { value };
        let authored_value_px = f64::from(normalized_value);
        let effective_value_px = authored_value_px.min(max_radius);
        let token = if authored_value_px > max_radius {
            effective_value_px.to_string()
        } else {
            normalized_value.to_string()
        };
        let value_px = token
            .parse::<f64>()
            .expect("Timeline event radius token must be a canonical finite number");
        Self {
            token: token.into_boxed_str(),
            value_px,
        }
    }
}

#[derive(Debug, Clone)]
struct TimelineEventDecision {
    fill: Option<TimelineEventPaint>,
    radius: Option<TimelineEventRadius>,
    radius_rule_index: Option<usize>,
    opacity_token: Option<Box<str>>,
    opacity_rule_index: Option<usize>,
}

#[derive(Debug, Clone)]
struct TimelineEventPaint {
    css: Box<str>,
    rule_index: usize,
    capability: ThemeCapability,
}

#[derive(Debug)]
struct TimelineStrokePlan {
    paint: TimelineEventPaint,
    shape_stroke: bool,
}

#[derive(Debug)]
struct TimelinePalettePaint {
    css: Box<str>,
    capability: Option<ThemeCapability>,
}

impl TimelinePalettePaint {
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

impl TimelineEventDecision {
    fn baseline() -> Self {
        Self {
            fill: None,
            radius: None,
            radius_rule_index: None,
            opacity_token: None,
            opacity_rule_index: None,
        }
    }
}

/// Timeline event paint/geometry and evidence resolved in the exact terminal draw order.
#[derive(Debug)]
pub(crate) struct TimelineEventTheme {
    events: Box<[TimelineEventDecision]>,
    stroke: Option<TimelineStrokePlan>,
    palette_slots: [Option<TimelinePalettePaint>; TIMELINE_PALETTE_SLOT_COUNT],
    palette_line_strokes: [Option<Box<str>>; TIMELINE_PALETTE_SLOT_COUNT],
    palette_key: Option<FamilyThemeMechanismKey>,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, TimelineEventPendingEvidence>,
    terminal_outcome: OnceLock<TimelineEventThemeOutcome>,
}

impl TimelineEventTheme {
    pub(crate) fn resolve_with_binding(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        binding: &super::TimelineCssBinding,
        layout: &TimelineDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::baseline());
        };
        if !theme.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::TimelineEvent,
                    ..
                } | FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::TimelineEvent
                } | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::TimelineEvent,
                    ..
                }
            )
        }) {
            return Ok(Self::baseline());
        }
        let event_nodes = timeline_event_nodes(layout);
        let event_count = event_nodes.clone().count();

        let palette_key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::TimelineEvent,
        };
        let palette_disposition = theme.ordinal_palette_disposition(ThemeTarget::TimelineEvent);
        let has_palette_nodes = timeline_palette_nodes(layout).next().is_some();
        let active_palette_slot_limit = binding.sections.len().min(TIMELINE_PALETTE_SLOT_COUNT);
        let mut palette_slots: [Option<TimelinePalettePaint>; TIMELINE_PALETTE_SLOT_COUNT] =
            std::array::from_fn(|_| None);
        let mut palette_line_strokes: [Option<Box<str>>; TIMELINE_PALETTE_SLOT_COUNT] =
            std::array::from_fn(|_| None);
        // Layout already applies Mermaid's modulo ring before it emits section classes. A valid
        // class therefore always maps into the active limit; only malformed classes or missing
        // colors are residuals here.
        let mut missing_palette_slot = false;
        let palette_enabled = palette_disposition == Some(FamilyThemeDisposition::TypedAdapter)
            && !binding.is_redux_theme
            && !binding.use_neo_gradient;
        if palette_enabled {
            let mut used_slots = [false; TIMELINE_PALETTE_SLOT_COUNT];
            for node in timeline_palette_nodes(layout) {
                work_meter.charge(1)?;
                if let Some(slot) = timeline_section_slot(&node.section_class)
                    .filter(|slot| *slot < active_palette_slot_limit)
                {
                    used_slots[slot] = true;
                } else {
                    missing_palette_slot = true;
                }
            }
            for (slot, used) in used_slots.into_iter().enumerate() {
                if !used {
                    continue;
                }
                let color_scale_path = format!("themeVariables.cScale{slot}");
                let paint = if merman_core::__private::config_paint_overrides_typed_default(
                    effective_config,
                    &color_scale_path,
                ) {
                    binding
                        .source_scale(slot)
                        .map(|color| TimelinePalettePaint::source_owned(color.to_owned()))
                } else {
                    theme
                        .series_color(ThemeTarget::TimelineEvent, slot + 1)
                        .map(|color| {
                            TimelinePalettePaint::typed(
                                color.as_css(),
                                if color.is_transparent() {
                                    ThemeCapability::TransparentPaint
                                } else {
                                    ThemeCapability::SolidPaint
                                },
                            )
                        })
                };
                if let Some(paint) = paint {
                    let line_stroke =
                        timeline_classic_line_stroke(effective_config, binding, slot, &paint.css);
                    missing_palette_slot |= line_stroke.is_none();
                    palette_line_strokes[slot] = line_stroke;
                    palette_slots[slot] = Some(paint);
                } else {
                    missing_palette_slot = true;
                }
            }
        }

        let has_event_rules = theme
            .family_rules()
            .any(|(_, rule)| rule.target() == ThemeTarget::TimelineEvent);
        let mut events = if has_event_rules {
            vec![TimelineEventDecision::baseline(); event_count]
        } else {
            Vec::new()
        };
        let mut winner_counts =
            BTreeMap::<(usize, crate::diagram_theme::ResolvedStyleProperty), usize>::new();
        let mut source_owned_fill_rule_occurrences = BTreeMap::<usize, usize>::new();
        let static_style = theme.style_with_work_meter(
            ThemeTarget::TimelineEvent,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let source_owns_event_stroke = merman_core::__private::config_paint_overrides_typed_default(
            effective_config,
            "themeVariables.nodeBorder",
        );
        // nodeBorder is a shared Redux terminal: labels and the activity axis consume it even
        // when there are no events. Color and Neo shapes have independent paint owners.
        let stroke_domain_active =
            binding.is_redux_theme && active_palette_slot_limit != 0 && !source_owns_event_stroke;
        let stroke = if stroke_domain_active {
            resolve_direct_static_stroke(
                theme,
                &static_style,
                &[ThemeTarget::TimelineEvent],
                DirectStaticSelectorDomain::Default,
            )
            .map(|paint| {
                let (css, rule_index, capability) = paint.into_parts();
                TimelineStrokePlan {
                    paint: TimelineEventPaint {
                        css,
                        rule_index,
                        capability,
                    },
                    shape_stroke: !binding.is_color_theme && !binding.use_neo_gradient,
                }
            })
        } else {
            None
        };
        let has_ordinal_event_rules = theme.family_rules().any(|(_, rule)| {
            rule.target() == ThemeTarget::TimelineEvent && rule.ordinal().is_some()
        });
        if event_count != 0 && !has_ordinal_event_rules {
            let style = &static_style;
            collect_winners(style, event_count, &mut winner_counts);
            for (event, node) in events.iter_mut().zip(event_nodes.clone()) {
                apply_event_style(
                    theme,
                    style,
                    event,
                    node,
                    timeline_event_fill_source_owned(effective_config, binding, node),
                    &mut source_owned_fill_rule_occurrences,
                );
            }
        } else {
            for (event_index, (event, node)) in events.iter_mut().zip(event_nodes).enumerate() {
                let style = theme.style_with_work_meter(
                    ThemeTarget::TimelineEvent,
                    ThemeVariant::Default,
                    Some(event_index + 1),
                    work_meter,
                )?;
                collect_winners(&style, 1, &mut winner_counts);
                apply_event_style(
                    theme,
                    &style,
                    event,
                    node,
                    timeline_event_fill_source_owned(effective_config, binding, node),
                    &mut source_owned_fill_rule_occurrences,
                );
            }
        }

        let mut evidence = FamilyThemeEvidence::outcomes();
        let mut observations = BTreeMap::<usize, TimelineEventRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::TimelineEvent,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    let property = resolved_style_property_for_facet(facet);
                    let is_stroke = matches!(facet, FamilyThemeRuleFacet::Stroke(_));
                    if is_stroke && !stroke_domain_active {
                        // A non-consuming facet cannot seal the whole rule: its fill, geometry,
                        // or unsupported siblings still need their own winner accounting.
                        continue;
                    }
                    let is_static_stroke =
                        is_stroke && matches!(selector, FamilyThemeSelectorShape::Static { .. });
                    if !is_static_stroke
                        && !selector.ordinal_domain_intersects_occurrence_count(event_count)
                    {
                        continue;
                    }
                    let route_won = if is_static_stroke {
                        static_style
                            .stroke_resolution()
                            .winner()
                            .is_some_and(|winner| winner.rule_index() == rule_index)
                    } else {
                        winner_counts.contains_key(&(rule_index, property))
                    };
                    if !route_won {
                        continue;
                    }
                    observation.applicable = true;
                    match (route.disposition(), facet) {
                        (FamilyThemeDisposition::TypedAdapter, FamilyThemeRuleFacet::Radius) => {
                            let property = resolved_style_property_for_facet(facet);
                            if winner_counts.get(&(rule_index, property)).copied()
                                == Some(
                                    events
                                        .iter()
                                        .filter(|event| event.radius_rule_index == Some(rule_index))
                                        .count(),
                                )
                            {
                                observation.radius_pending = true;
                            } else {
                                observation.incomplete = true;
                            }
                        }
                        (FamilyThemeDisposition::TypedAdapter, FamilyThemeRuleFacet::Opacity) => {
                            let property = resolved_style_property_for_facet(facet);
                            if winner_counts.get(&(rule_index, property)).copied()
                                == Some(
                                    events
                                        .iter()
                                        .filter(|event| {
                                            event.opacity_rule_index == Some(rule_index)
                                        })
                                        .count(),
                                )
                            {
                                observation.opacity_pending = true;
                            } else {
                                observation.incomplete = true;
                            }
                        }
                        (FamilyThemeDisposition::TypedAdapter, FamilyThemeRuleFacet::Fill(_)) => {
                            let property = resolved_style_property_for_facet(facet);
                            let wins = winner_counts.get(&(rule_index, property)).copied();
                            let typed_count = events
                                .iter()
                                .filter(|event| {
                                    event.fill.as_ref().map(|fill| fill.rule_index)
                                        == Some(rule_index)
                                })
                                .count();
                            let source_owned_count = source_owned_fill_rule_occurrences
                                .get(&rule_index)
                                .copied()
                                .unwrap_or_default();
                            if wins == Some(typed_count.saturating_add(source_owned_count)) {
                                if typed_count != 0 {
                                    observation.fill_pending = true;
                                }
                            } else {
                                observation.incomplete = true;
                            }
                        }
                        (FamilyThemeDisposition::TypedAdapter, FamilyThemeRuleFacet::Stroke(_)) => {
                            if stroke
                                .as_ref()
                                .is_some_and(|stroke| stroke.paint.rule_index == rule_index)
                            {
                                observation.stroke_pending = true;
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
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::TimelineEvent,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if !has_palette_nodes {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    } else if !palette_enabled {
                        evidence.mark_not_applicable(key);
                    } else if missing_palette_slot {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::TimelineEvent,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if event_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else {
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
                target: ThemeTarget::TimelineEvent,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // A mixed rule cannot be signed Applied until every winning facet is accounted.
            } else if observation.radius_pending
                || observation.opacity_pending
                || observation.fill_pending
                || observation.stroke_pending
            {
                let mut capabilities = BTreeSet::new();
                if observation.radius_pending {
                    capabilities.insert(ThemeCapability::RoundedGeometry);
                }
                if observation.opacity_pending {
                    capabilities.insert(ThemeCapability::Opacity);
                }
                if observation.fill_pending {
                    capabilities.extend(events.iter().filter_map(|event| {
                        event
                            .fill
                            .as_ref()
                            .filter(|fill| fill.rule_index == rule_index)
                            .map(|fill| fill.capability)
                    }));
                }
                if observation.stroke_pending {
                    capabilities.extend(stroke.as_ref().map(|stroke| stroke.paint.capability));
                }
                pending.insert(
                    key,
                    TimelineEventPendingEvidence {
                        radius: observation.radius_pending,
                        opacity: observation.opacity_pending,
                        fill: observation.fill_pending,
                        stroke: observation.stroke_pending,
                        capabilities,
                    },
                );
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            events: events.into_boxed_slice(),
            stroke,
            palette_slots,
            palette_line_strokes,
            palette_key: (palette_enabled && !missing_palette_slot && has_palette_nodes)
                .then_some(palette_key),
            evidence,
            pending,
            terminal_outcome: OnceLock::new(),
        })
    }

    pub(crate) fn baseline() -> Self {
        Self {
            events: Box::new([]),
            stroke: None,
            palette_slots: std::array::from_fn(|_| None),
            palette_line_strokes: std::array::from_fn(|_| None),
            palette_key: None,
            evidence: FamilyThemeEvidence::outcomes(),
            pending: BTreeMap::new(),
            terminal_outcome: OnceLock::new(),
        }
    }

    pub(crate) fn opacity_token_for_event(&self, event_index: usize) -> Option<&str> {
        self.events
            .get(event_index)
            .and_then(|event| event.opacity_token.as_deref())
    }

    pub(crate) fn radius_for_event(&self, event_index: usize) -> Option<(&str, f64)> {
        self.events
            .get(event_index)
            .and_then(|event| event.radius.as_ref())
            .map(|radius| (radius.token.as_ref(), radius.value_px))
    }

    pub(crate) fn fill_for_event(&self, event_index: usize) -> Option<&str> {
        self.events
            .get(event_index)
            .and_then(|event| event.fill.as_ref())
            .map(|fill| fill.css.as_ref())
    }

    pub(crate) fn stroke_for_redux(&self) -> Option<&str> {
        self.stroke.as_ref().map(|stroke| stroke.paint.css.as_ref())
    }

    pub(crate) fn palette_fill_for_slot(&self, slot: usize) -> Option<&str> {
        self.palette_slots
            .get(slot)
            .and_then(|paint| paint.as_ref())
            .map(|paint| paint.css.as_ref())
    }

    pub(crate) fn palette_line_stroke_for_slot(&self, slot: usize) -> Option<&str> {
        self.palette_line_strokes
            .get(slot)
            .and_then(Option::as_deref)
    }

    pub(crate) fn palette_slot_for_section(&self, section_class: &str) -> Option<usize> {
        timeline_section_slot(section_class)
    }

    pub(crate) fn shape_stroke_for_redux(&self) -> Option<&str> {
        self.stroke
            .as_ref()
            .filter(|stroke| stroke.shape_stroke)
            .map(|stroke| stroke.paint.css.as_ref())
    }

    pub(crate) fn begin_emission(&self) -> Option<TimelineEventThemeOutcome> {
        (!self.pending.is_empty() || self.palette_key.is_some() || self.stroke.is_some())
            .then(TimelineEventThemeOutcome::default)
    }

    pub(crate) fn record_emission(&self, outcome: TimelineEventThemeOutcome) {
        let _ = self.terminal_outcome.set(outcome);
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(outcome) = self.terminal_outcome.get() else {
            return evidence;
        };
        for (key, pending) in &self.pending {
            let rule_index = match key {
                FamilyThemeMechanismKey::Rule { index, .. } => *index,
                FamilyThemeMechanismKey::Typography(_)
                | FamilyThemeMechanismKey::OrdinalPalette { .. }
                | FamilyThemeMechanismKey::EffectBinding { .. } => continue,
            };
            if outcome.rule_was_written(self, rule_index, pending) {
                evidence.mark_applied_with_capabilities(
                    key.clone(),
                    pending.capabilities.iter().copied(),
                );
            }
        }
        if let Some(key) = self.palette_key.clone() {
            if outcome.palette_capabilities.is_empty() {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_applied_with_capabilities(
                    key,
                    outcome.palette_capabilities.iter().copied(),
                );
            }
        }
        evidence
    }
}

fn timeline_event_nodes(
    layout: &TimelineDiagramLayout,
) -> impl Clone + Iterator<Item = &crate::model::TimelineNodeLayout> {
    layout
        .sections
        .iter()
        .flat_map(|section| section.tasks.iter())
        .chain(layout.orphan_tasks.iter())
        .flat_map(|task| task.events.iter())
}

fn timeline_event_fill_source_owned(
    config: &MermaidConfig,
    binding: &super::TimelineCssBinding,
    event: &crate::model::TimelineNodeLayout,
) -> bool {
    let Some(slot) = timeline_section_slot(&event.section_class) else {
        return false;
    };
    if binding.use_neo_gradient {
        return merman_core::__private::config_paint_overrides_typed_default(
            config,
            "themeVariables.mainBkg",
        );
    }
    if !binding.is_redux_theme {
        return merman_core::__private::config_paint_overrides_typed_default(
            config,
            &format!("themeVariables.cScale{slot}"),
        );
    }

    if binding.is_color_theme && !binding.is_dark_theme {
        if binding.has_source_border_slot(slot) {
            return merman_core::__private::config_paint_overrides_typed_default(
                config,
                "themeVariables.borderColorArray",
            );
        }
        // A color theme owns an event fill only when the section has an actual
        // border-color slot. An empty or short array cannot provide a fill
        // terminal, so it must not suppress the typed event paint via the
        // unrelated node-border token.
        return false;
    }

    merman_core::__private::config_paint_overrides_typed_default(config, "themeVariables.mainBkg")
}

fn timeline_classic_line_stroke(
    config: &MermaidConfig,
    binding: &super::TimelineCssBinding,
    slot: usize,
    fill: &str,
) -> Option<Box<str>> {
    if binding.is_redux_theme {
        return None;
    }
    let inverse_path = format!("themeVariables.cScaleInv{slot}");
    if merman_core::__private::config_paint_overrides_typed_default(config, &inverse_path) {
        return binding
            .source_inverse(slot)
            .map(str::to_owned)
            .map(Into::into);
    }
    merman_core::theme_color::invert(fill).ok().map(Into::into)
}

fn timeline_section_slot(section_class: &str) -> Option<usize> {
    let suffix = section_class.strip_prefix("section-")?;
    let section = suffix.parse::<i64>().ok()?;
    let slot = section.checked_add(1)?;
    usize::try_from(slot)
        .ok()
        .filter(|slot| *slot < TIMELINE_PALETTE_SLOT_COUNT)
}

fn timeline_palette_nodes(
    layout: &TimelineDiagramLayout,
) -> impl Iterator<Item = &crate::model::TimelineNodeLayout> {
    layout
        .sections
        .iter()
        .flat_map(|section| {
            std::iter::once(&section.node).chain(
                section
                    .tasks
                    .iter()
                    .flat_map(|task| std::iter::once(&task.node).chain(task.events.iter())),
            )
        })
        .chain(
            layout
                .orphan_tasks
                .iter()
                .flat_map(|task| std::iter::once(&task.node).chain(task.events.iter())),
        )
}

fn collect_winners(
    style: &crate::diagram_theme::ResolvedThemeStyle,
    occurrence_count: usize,
    winner_counts: &mut BTreeMap<(usize, crate::diagram_theme::ResolvedStyleProperty), usize>,
) {
    for (property, origin) in style.winner_rule_properties() {
        let key = (origin.rule_index(), property);
        *winner_counts.entry(key).or_default() += occurrence_count;
    }
}

fn apply_event_style(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
    event: &mut TimelineEventDecision,
    node: &crate::model::TimelineNodeLayout,
    source_owns_event_fill: bool,
    source_owned_fill_rule_occurrences: &mut BTreeMap<usize, usize>,
) {
    if let Some(fill) = resolve_direct_static_fill(
        theme,
        style,
        &[ThemeTarget::TimelineEvent],
        DirectStaticSelectorDomain::Default,
    ) {
        let (css, rule_index, capability) = fill.into_parts();
        if source_owns_event_fill {
            *source_owned_fill_rule_occurrences
                .entry(rule_index)
                .or_default() += 1;
        } else {
            event.fill = Some(TimelineEventPaint {
                css,
                rule_index,
                capability,
            });
        }
    }
    if let Some(origin) = style.radius_resolution().winner()
        && theme.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::Radius)
            == Some(FamilyThemeDisposition::TypedAdapter)
    {
        match style.radius_resolution().specified() {
            Specified::Value(value) => {
                event.radius = Some(TimelineEventRadius::value(*value, node));
                event.radius_rule_index = Some(origin.rule_index());
            }
            Specified::Clear => {
                event.radius = None;
                event.radius_rule_index = Some(origin.rule_index());
            }
            Specified::Unspecified => {}
        }
    }
    if let Some(origin) = style.opacity_resolution().winner()
        && theme.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::Opacity)
            == Some(FamilyThemeDisposition::TypedAdapter)
    {
        event.opacity_token = match style.opacity_resolution().specified() {
            Specified::Value(value) => Some(value.to_string().into_boxed_str()),
            Specified::Unspecified | Specified::Clear => None,
        };
        event.opacity_rule_index = Some(origin.rule_index());
    }
}

#[derive(Debug, Default)]
struct TimelineEventRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    radius_pending: bool,
    opacity_pending: bool,
    fill_pending: bool,
    stroke_pending: bool,
}

#[derive(Debug, Default)]
struct TimelineEventPendingEvidence {
    radius: bool,
    opacity: bool,
    fill: bool,
    stroke: bool,
    capabilities: BTreeSet<ThemeCapability>,
}

#[derive(Debug, Default)]
pub(crate) struct TimelineEventThemeOutcome {
    radius_rules: BTreeSet<usize>,
    opacity_rules: BTreeSet<usize>,
    fill_rules: BTreeSet<usize>,
    stroke_css_written: bool,
    stroke_line_written: bool,
    palette_capabilities: BTreeSet<ThemeCapability>,
}

impl TimelineEventThemeOutcome {
    pub(crate) fn record_event(&mut self, theme: &TimelineEventTheme, event_index: usize) {
        let Some(event) = theme.events.get(event_index) else {
            return;
        };
        if let Some(rule_index) = event.radius_rule_index {
            self.radius_rules.insert(rule_index);
        }
        if let Some(rule_index) = event.opacity_rule_index {
            self.opacity_rules.insert(rule_index);
        }
        if let Some(fill) = &event.fill {
            self.fill_rules.insert(fill.rule_index);
        }
    }

    pub(crate) fn record_palette_node(
        &mut self,
        theme: &TimelineEventTheme,
        slot: Option<usize>,
        palette_fill_written: bool,
    ) {
        if palette_fill_written
            && let Some(capability) = slot
                .and_then(|slot| theme.palette_slots.get(slot))
                .and_then(Option::as_ref)
                .and_then(|paint| paint.capability)
        {
            self.palette_capabilities.insert(capability);
        }
    }

    pub(crate) fn record_stroke_css(&mut self) {
        self.stroke_css_written = true;
    }

    pub(crate) fn record_stroke_line(&mut self) {
        self.stroke_line_written = true;
    }

    fn rule_was_written(
        &self,
        theme: &TimelineEventTheme,
        rule_index: usize,
        pending: &TimelineEventPendingEvidence,
    ) -> bool {
        (!pending.radius || self.radius_rules.contains(&rule_index))
            && (!pending.opacity || self.opacity_rules.contains(&rule_index))
            && (!pending.fill || self.fill_rules.contains(&rule_index))
            && (!pending.stroke
                || self.stroke_css_written
                    && self.stroke_line_written
                    && theme
                        .stroke
                        .as_ref()
                        .is_some_and(|stroke| stroke.paint.rule_index == rule_index))
    }
}

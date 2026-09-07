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
    resolve_direct_static_fill, resolved_style_property_for_facet, unsupported_residual_for_facet,
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
struct TimelineEventTerminalExpectation {
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
struct TimelinePaletteNodeExpectation {
    slot: usize,
    fill: Option<TimelinePalettePaint>,
    classic_line_stroke: Option<Box<str>>,
}

impl TimelineEventTerminalExpectation {
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
    events: Box<[TimelineEventTerminalExpectation]>,
    palette_slots: [Option<TimelinePalettePaint>; TIMELINE_PALETTE_SLOT_COUNT],
    palette_line_strokes: [Option<Box<str>>; TIMELINE_PALETTE_SLOT_COUNT],
    palette_nodes: Box<[TimelinePaletteNodeExpectation]>,
    palette_key: Option<FamilyThemeMechanismKey>,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, TimelineEventPendingEvidence>,
    terminal_receipt: OnceLock<TimelineEventThemeReceipt>,
}

impl TimelineEventTheme {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        layout: &TimelineDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::baseline());
        };
        let event_nodes = timeline_event_nodes(layout);
        let event_count = event_nodes.len();

        let palette_key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::TimelineEvent,
        };
        let palette_disposition = theme.ordinal_palette_disposition(ThemeTarget::TimelineEvent);
        let palette_node_occurrences = timeline_palette_node_occurrences(layout);
        let palette_node_slots = palette_node_occurrences
            .iter()
            .map(|occurrence| timeline_section_slot(&occurrence.node.section_class))
            .collect::<Vec<_>>();
        let active_palette_slot_limit =
            super::timeline_theme_color_limit(effective_config.as_value())
                .min(TIMELINE_PALETTE_SLOT_COUNT);
        let mut palette_slots: [Option<TimelinePalettePaint>; TIMELINE_PALETTE_SLOT_COUNT] =
            std::array::from_fn(|_| None);
        let mut palette_line_strokes: [Option<Box<str>>; TIMELINE_PALETTE_SLOT_COUNT] =
            std::array::from_fn(|_| None);
        // Layout already applies Mermaid's modulo ring before it emits section classes. A valid
        // class therefore always maps into the active limit; only malformed classes or missing
        // colors are residuals here.
        let mut missing_palette_slot = palette_node_slots.iter().any(Option::is_none);
        let palette_enabled = palette_disposition == Some(FamilyThemeDisposition::TypedAdapter)
            && !timeline_is_redux_theme(effective_config);
        if palette_enabled {
            let mut used_slots = BTreeSet::new();
            for slot in palette_node_slots
                .iter()
                .flatten()
                .copied()
                .filter(|slot| *slot < active_palette_slot_limit)
            {
                used_slots.insert(slot);
            }
            for slot in used_slots {
                let color_scale_path = format!("themeVariables.cScale{slot}");
                let paint = if merman_core::__private::config_path_overrides_typed_default(
                    effective_config,
                    &color_scale_path,
                ) {
                    effective_config
                        .get_str(&color_scale_path)
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
                    palette_slots[slot] = Some(paint);
                } else {
                    missing_palette_slot = true;
                }
            }
        }

        let mut events = vec![TimelineEventTerminalExpectation::baseline(); event_count];
        let mut winner_counts =
            BTreeMap::<(usize, crate::diagram_theme::ResolvedStyleProperty), usize>::new();
        let mut source_owned_fill_rule_occurrences = BTreeMap::<usize, usize>::new();
        let has_ordinal_event_rules = theme.family_rules().any(|(_, rule)| {
            rule.target() == ThemeTarget::TimelineEvent && rule.ordinal().is_some()
        });
        if event_count != 0 && !has_ordinal_event_rules {
            let style = theme.style_with_work_meter(
                ThemeTarget::TimelineEvent,
                ThemeVariant::Default,
                None,
                work_meter,
            )?;
            collect_winners(&style, event_count, &mut winner_counts);
            for (event, node) in events.iter_mut().zip(event_nodes.iter().copied()) {
                apply_event_style(
                    theme,
                    &style,
                    event,
                    node,
                    timeline_event_fill_source_owned(effective_config, node),
                    &mut source_owned_fill_rule_occurrences,
                );
            }
        } else {
            for (event_index, event) in events.iter_mut().enumerate() {
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
                    event_nodes[event_index],
                    timeline_event_fill_source_owned(effective_config, event_nodes[event_index]),
                    &mut source_owned_fill_rule_occurrences,
                );
            }
        }

        let mut palette_nodes = Vec::new();
        if palette_enabled {
            for (occurrence, slot) in palette_node_occurrences
                .iter()
                .zip(palette_node_slots.iter().copied())
            {
                let Some(slot) = slot.filter(|slot| *slot < active_palette_slot_limit) else {
                    missing_palette_slot = true;
                    continue;
                };
                let Some(fill) = palette_slots[slot].clone() else {
                    missing_palette_slot = true;
                    continue;
                };
                let line_stroke =
                    timeline_classic_line_stroke(effective_config, slot, fill.css.as_ref());
                if line_stroke.is_none() {
                    missing_palette_slot = true;
                }
                palette_line_strokes[slot] = line_stroke.clone();
                let direct_fill_wins = occurrence
                    .event_index
                    .and_then(|event_index| events.get(event_index))
                    .is_some_and(|event| event.fill.is_some());
                palette_nodes.push(TimelinePaletteNodeExpectation {
                    slot,
                    fill: (!direct_fill_wins).then_some(fill),
                    classic_line_stroke: line_stroke,
                });
            }
        }

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
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
                    if !selector.ordinal_domain_intersects_occurrence_count(event_count) {
                        continue;
                    }
                    let route_won = winner_counts
                        .contains_key(&(rule_index, resolved_style_property_for_facet(facet)));
                    let qualified_variant = matches!(
                        selector,
                        FamilyThemeSelectorShape::Static { variant: Some(_) }
                            | FamilyThemeSelectorShape::Ordinal {
                                variant: Some(_),
                                ..
                            }
                    );
                    if !route_won && !qualified_variant {
                        continue;
                    }
                    observation.applicable = true;
                    match (route.disposition(), facet) {
                        (FamilyThemeDisposition::TypedAdapter, FamilyThemeRuleFacet::Radius) => {
                            let property = resolved_style_property_for_facet(facet);
                            if winner_counts.get(&(rule_index, property)).copied()
                                == Some(event_count)
                                && events
                                    .iter()
                                    .all(|event| event.radius_rule_index == Some(rule_index))
                            {
                                observation.radius_pending = true;
                            } else {
                                observation.incomplete = true;
                            }
                        }
                        (FamilyThemeDisposition::TypedAdapter, FamilyThemeRuleFacet::Opacity) => {
                            let property = resolved_style_property_for_facet(facet);
                            if winner_counts.get(&(rule_index, property)).copied()
                                == Some(event_count)
                                && events
                                    .iter()
                                    .all(|event| event.opacity_rule_index == Some(rule_index))
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
                            if wins == Some(event_count)
                                && typed_count.saturating_add(source_owned_count) == event_count
                            {
                                if typed_count != 0 {
                                    observation.fill_pending = true;
                                }
                                if source_owned_count == event_count {
                                    observation.suppressed = true;
                                }
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
                    target: ThemeTarget::TimelineEvent,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if palette_node_slots.is_empty() {
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
                pending.insert(
                    key,
                    TimelineEventPendingEvidence {
                        radius: observation.radius_pending,
                        opacity: observation.opacity_pending,
                        fill: observation.fill_pending,
                        capabilities,
                    },
                );
            } else if observation.suppressed {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        let palette_nodes_are_nonempty = !palette_nodes.is_empty();
        Ok(Self {
            events: events.into_boxed_slice(),
            palette_slots,
            palette_line_strokes,
            palette_nodes: palette_nodes.into_boxed_slice(),
            palette_key: (palette_enabled && !missing_palette_slot && palette_nodes_are_nonempty)
                .then_some(palette_key),
            evidence,
            pending,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline() -> Self {
        Self {
            events: Box::new([]),
            palette_slots: std::array::from_fn(|_| None),
            palette_line_strokes: std::array::from_fn(|_| None),
            palette_nodes: Box::new([]),
            palette_key: None,
            evidence: FamilyThemeEvidence::default(),
            pending: BTreeMap::new(),
            terminal_receipt: OnceLock::new(),
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

    pub(crate) fn begin_terminal_receipt(
        &self,
        is_redux_theme: bool,
    ) -> Option<TimelineEventThemeReceipt> {
        (!self.pending.is_empty() || self.palette_key.is_some()).then(|| {
            TimelineEventThemeReceipt::from_expectations_with_baseline_and_palette(
                self.events.clone(),
                (!is_redux_theme).then_some(super::MERMAID_EVENT_RADIUS_TOKEN),
                self.palette_nodes.clone(),
            )
        })
    }

    pub(crate) fn record_terminal(&self, receipt: TimelineEventThemeReceipt) -> bool {
        receipt.proves(self.events.len()) && self.terminal_receipt.set(receipt).is_ok()
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

fn timeline_event_nodes(layout: &TimelineDiagramLayout) -> Vec<&crate::model::TimelineNodeLayout> {
    layout
        .sections
        .iter()
        .flat_map(|section| section.tasks.iter())
        .chain(layout.orphan_tasks.iter())
        .flat_map(|task| task.events.iter())
        .collect()
}

#[derive(Clone, Copy)]
struct TimelinePaletteNodeOccurrence<'a> {
    node: &'a crate::model::TimelineNodeLayout,
    event_index: Option<usize>,
}

fn timeline_is_redux_theme(config: &MermaidConfig) -> bool {
    config
        .get_str("theme")
        .is_some_and(|theme| theme.contains("redux"))
}

fn timeline_event_fill_source_owned(
    config: &MermaidConfig,
    event: &crate::model::TimelineNodeLayout,
) -> bool {
    let Some(slot) = timeline_section_slot(&event.section_class) else {
        return false;
    };
    let theme = config.get_str("theme").unwrap_or_default();
    if !theme.contains("redux") {
        return merman_core::__private::config_path_overrides_typed_default(
            config,
            &format!("themeVariables.cScale{slot}"),
        );
    }

    if theme.contains("color") && !theme.contains("dark") {
        let border_colors = config
            .as_value()
            .get("themeVariables")
            .and_then(|variables| variables.get("borderColorArray"))
            .and_then(serde_json::Value::as_array);
        if border_colors
            .and_then(|colors| colors.get(slot))
            .and_then(serde_json::Value::as_str)
            .is_some()
        {
            return merman_core::__private::config_path_overrides_typed_default(
                config,
                "themeVariables.borderColorArray",
            );
        }
        return merman_core::__private::config_path_overrides_typed_default(
            config,
            "themeVariables.nodeBorder",
        );
    }

    merman_core::__private::config_path_overrides_typed_default(config, "themeVariables.mainBkg")
}

fn timeline_classic_line_stroke(
    config: &MermaidConfig,
    slot: usize,
    fill: &str,
) -> Option<Box<str>> {
    if timeline_is_redux_theme(config) {
        return None;
    }
    let inverse_path = format!("themeVariables.cScaleInv{slot}");
    if merman_core::__private::config_path_overrides_typed_default(config, &inverse_path) {
        return config
            .get_str(&inverse_path)
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

fn timeline_palette_node_occurrences(
    layout: &TimelineDiagramLayout,
) -> Vec<TimelinePaletteNodeOccurrence<'_>> {
    let mut nodes = Vec::new();
    let mut event_index = 0usize;
    for section in &layout.sections {
        nodes.push(TimelinePaletteNodeOccurrence {
            node: &section.node,
            event_index: None,
        });
        for task in &section.tasks {
            nodes.push(TimelinePaletteNodeOccurrence {
                node: &task.node,
                event_index: None,
            });
            for event in &task.events {
                nodes.push(TimelinePaletteNodeOccurrence {
                    node: event,
                    event_index: Some(event_index),
                });
                event_index += 1;
            }
        }
    }
    for task in &layout.orphan_tasks {
        nodes.push(TimelinePaletteNodeOccurrence {
            node: &task.node,
            event_index: None,
        });
        for event in &task.events {
            nodes.push(TimelinePaletteNodeOccurrence {
                node: event,
                event_index: Some(event_index),
            });
            event_index += 1;
        }
    }
    nodes
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
    event: &mut TimelineEventTerminalExpectation,
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
    if let Some(origin) = style.radius_resolution().winner() {
        if theme.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::Radius)
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
    }
    if let Some(origin) = style.opacity_resolution().winner() {
        if theme.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::Opacity)
            == Some(FamilyThemeDisposition::TypedAdapter)
        {
            event.opacity_token = match style.opacity_resolution().specified() {
                Specified::Value(value) => Some(value.to_string().into_boxed_str()),
                Specified::Unspecified | Specified::Clear => None,
            };
            event.opacity_rule_index = Some(origin.rule_index());
        }
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
    suppressed: bool,
}

#[derive(Debug, Default)]
struct TimelineEventPendingEvidence {
    radius: bool,
    opacity: bool,
    fill: bool,
    capabilities: BTreeSet<ThemeCapability>,
}

/// Writer-owned proof that every Timeline event emitted its canonical terminal state.
#[derive(Debug)]
pub(crate) struct TimelineEventThemeReceipt {
    expectations: Box<[TimelineEventTerminalExpectation]>,
    baseline_radius_token: Option<&'static str>,
    palette_expectations: Box<[TimelinePaletteNodeExpectation]>,
    next_event_index: usize,
    next_palette_index: usize,
    palette_nodes_seen: usize,
    attributes_match: bool,
    palette_values_match: bool,
    radius_rules: BTreeSet<usize>,
    opacity_rules: BTreeSet<usize>,
    fill_rules: BTreeSet<usize>,
    palette_capabilities: BTreeSet<ThemeCapability>,
}

impl TimelineEventThemeReceipt {
    #[cfg(test)]
    fn new(event_count: usize) -> Self {
        Self::from_expectations(
            vec![TimelineEventTerminalExpectation::baseline(); event_count].into_boxed_slice(),
        )
    }

    #[cfg(test)]
    fn from_expectations(
        expectations: Box<[TimelineEventTerminalExpectation]>,
    ) -> TimelineEventThemeReceipt {
        Self::from_expectations_with_baseline(expectations, None)
    }

    #[cfg(test)]
    fn from_expectations_with_baseline(
        expectations: Box<[TimelineEventTerminalExpectation]>,
        baseline_radius_token: Option<&'static str>,
    ) -> TimelineEventThemeReceipt {
        Self {
            expectations,
            baseline_radius_token,
            palette_expectations: Box::new([]),
            next_event_index: 0,
            next_palette_index: 0,
            palette_nodes_seen: 0,
            attributes_match: true,
            palette_values_match: true,
            radius_rules: BTreeSet::new(),
            opacity_rules: BTreeSet::new(),
            fill_rules: BTreeSet::new(),
            palette_capabilities: BTreeSet::new(),
        }
    }

    fn from_expectations_with_baseline_and_palette(
        expectations: Box<[TimelineEventTerminalExpectation]>,
        baseline_radius_token: Option<&'static str>,
        palette_expectations: Box<[TimelinePaletteNodeExpectation]>,
    ) -> TimelineEventThemeReceipt {
        Self {
            expectations,
            baseline_radius_token,
            palette_expectations,
            next_event_index: 0,
            next_palette_index: 0,
            palette_nodes_seen: 0,
            attributes_match: true,
            palette_values_match: true,
            radius_rules: BTreeSet::new(),
            opacity_rules: BTreeSet::new(),
            fill_rules: BTreeSet::new(),
            palette_capabilities: BTreeSet::new(),
        }
    }

    pub(crate) fn record_checkpointed_event(
        &mut self,
        event_index: usize,
        emitted_opacity_token: Option<&str>,
        emitted_opacity_matches: bool,
        emitted_radius_token: Option<&str>,
        emitted_radius_geometry_matches: bool,
        emitted_fill: Option<&str>,
        emitted_fill_matches: bool,
    ) {
        if event_index != self.next_event_index || event_index >= self.expectations.len() {
            self.attributes_match = false;
            return;
        }

        let (
            opacity_matches,
            radius_matches,
            fill_matches,
            opacity_rule_index,
            radius_rule_index,
            fill_rule_index,
        ) = {
            let expected = &self.expectations[event_index];
            let expected_opacity_token = expected.opacity_token.as_deref();
            let expected_radius_token = expected
                .radius
                .as_ref()
                .map(|radius| radius.token.as_ref())
                .or(self.baseline_radius_token);
            (
                emitted_opacity_matches && emitted_opacity_token == expected_opacity_token,
                emitted_radius_geometry_matches
                    && expected_radius_token.map_or(emitted_radius_token.is_none(), |token| {
                        emitted_radius_token == Some(token)
                    }),
                emitted_fill_matches
                    && emitted_fill == expected.fill.as_ref().map(|fill| fill.css.as_ref()),
                expected.opacity_rule_index,
                expected.radius_rule_index,
                expected.fill.as_ref().map(|fill| fill.rule_index),
            )
        };
        self.next_event_index += 1;
        self.attributes_match &= opacity_matches && radius_matches && fill_matches;
        if opacity_matches {
            if let Some(rule_index) = opacity_rule_index {
                self.opacity_rules.insert(rule_index);
            }
        }
        if radius_matches {
            if let Some(rule_index) = radius_rule_index {
                self.radius_rules.insert(rule_index);
            }
        }
        if fill_matches {
            if let Some(rule_index) = fill_rule_index {
                self.fill_rules.insert(rule_index);
            }
        }
    }

    fn proves(&self, expected_event_count: usize) -> bool {
        self.expectations.len() == expected_event_count
            && self.next_event_index == expected_event_count
            && self.attributes_match
            && self.next_palette_index == self.palette_expectations.len()
            && self.palette_nodes_seen == self.palette_expectations.len()
            && self.palette_values_match
    }

    pub(crate) fn record_palette_node(
        &mut self,
        slot: Option<usize>,
        emitted_fill: Option<&str>,
        emitted_line_stroke: Option<&str>,
    ) {
        if self.palette_expectations.is_empty() {
            return;
        }
        self.palette_nodes_seen += 1;
        let Some(slot) = slot else {
            self.palette_values_match = false;
            return;
        };
        let Some(expected) = self.palette_expectations.get(self.next_palette_index) else {
            self.palette_values_match = false;
            return;
        };
        self.next_palette_index += 1;
        let fill_matches = slot == expected.slot
            && emitted_fill == expected.fill.as_ref().map(|fill| fill.css.as_ref());
        let line_matches = emitted_line_stroke == expected.classic_line_stroke.as_deref();
        self.palette_values_match &= fill_matches && line_matches;
        if fill_matches {
            if let Some(capability) = expected.fill.as_ref().and_then(|fill| fill.capability) {
                self.palette_capabilities.insert(capability);
            }
        }
    }

    fn proves_rule(&self, rule_index: usize, pending: &TimelineEventPendingEvidence) -> bool {
        (!pending.radius || self.radius_rules.contains(&rule_index))
            && (!pending.opacity || self.opacity_rules.contains(&rule_index))
            && (!pending.fill || self.fill_rules.contains(&rule_index))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TimelineEventPaint, TimelineEventPendingEvidence, TimelineEventRadius,
        TimelineEventTerminalExpectation, TimelineEventThemeReceipt,
        TimelinePaletteNodeExpectation, TimelinePalettePaint,
    };
    use crate::diagram_theme::ThemeCapability;

    #[test]
    fn timeline_event_receipt_requires_each_terminal_event_once() {
        let mut complete = TimelineEventThemeReceipt::new(1);
        complete.record_checkpointed_event(0, None, true, None, true, None, true);
        assert!(complete.proves(1));

        let mut incomplete = TimelineEventThemeReceipt::new(2);
        incomplete.record_checkpointed_event(0, None, true, None, true, None, true);
        assert!(!incomplete.proves(2));

        let mut duplicate = TimelineEventThemeReceipt::new(1);
        duplicate.record_checkpointed_event(0, None, true, None, true, None, true);
        duplicate.record_checkpointed_event(0, None, true, None, true, None, true);
        assert!(!duplicate.proves(1));

        let mut out_of_order = TimelineEventThemeReceipt::new(2);
        out_of_order.record_checkpointed_event(1, None, true, None, true, None, true);
        out_of_order.record_checkpointed_event(0, None, true, None, true, None, true);
        assert!(!out_of_order.proves(2));

        let mut mismatch = TimelineEventThemeReceipt::new(1);
        mismatch.record_checkpointed_event(0, Some("1"), true, None, true, None, true);
        assert!(!mismatch.proves(1));
    }

    #[test]
    fn timeline_event_receipt_tracks_matching_radius_rule() {
        let expectations = vec![TimelineEventTerminalExpectation {
            fill: None,
            radius: Some(TimelineEventRadius {
                token: "12".into(),
                value_px: 12.0,
            }),
            radius_rule_index: Some(7),
            opacity_token: None,
            opacity_rule_index: None,
        }]
        .into_boxed_slice();
        let mut receipt = TimelineEventThemeReceipt::from_expectations(expectations);
        receipt.record_checkpointed_event(0, None, true, Some("12"), true, None, true);

        assert!(receipt.proves(1));
        assert!(receipt.radius_rules.contains(&7));
    }

    #[test]
    fn timeline_event_receipt_requires_matching_radius_and_opacity_tokens() {
        let expectations = vec![TimelineEventTerminalExpectation {
            fill: None,
            radius: Some(TimelineEventRadius {
                token: "12".into(),
                value_px: 12.0,
            }),
            radius_rule_index: Some(7),
            opacity_token: Some("0.5".into()),
            opacity_rule_index: Some(7),
        }]
        .into_boxed_slice();
        let mut receipt = TimelineEventThemeReceipt::from_expectations(expectations.clone());
        receipt.record_checkpointed_event(0, Some("0.5"), true, Some("12"), true, None, true);

        assert!(receipt.proves(1));
        assert!(receipt.proves_rule(
            7,
            &TimelineEventPendingEvidence {
                radius: true,
                opacity: true,
                fill: false,
                capabilities: std::collections::BTreeSet::new(),
            }
        ));

        let mut mismatch = TimelineEventThemeReceipt::from_expectations(expectations);
        mismatch.record_checkpointed_event(0, Some("0.5"), true, Some("13"), true, None, true);
        assert!(!mismatch.proves(1));

        let expectations = vec![TimelineEventTerminalExpectation {
            fill: None,
            radius: None,
            radius_rule_index: None,
            opacity_token: Some("0.5".into()),
            opacity_rule_index: Some(7),
        }]
        .into_boxed_slice();
        let mut unobserved_opacity = TimelineEventThemeReceipt::from_expectations(expectations);
        unobserved_opacity.record_checkpointed_event(0, Some("0.5"), false, None, true, None, true);
        assert!(!unobserved_opacity.proves(1));
    }

    #[test]
    fn timeline_event_receipt_requires_matching_direct_fill() {
        let expectations = vec![TimelineEventTerminalExpectation {
            fill: Some(TimelineEventPaint {
                css: "#123456".into(),
                rule_index: 9,
                capability: ThemeCapability::SolidPaint,
            }),
            radius: None,
            radius_rule_index: None,
            opacity_token: None,
            opacity_rule_index: None,
        }]
        .into_boxed_slice();
        let pending = TimelineEventPendingEvidence {
            radius: false,
            opacity: false,
            fill: true,
            capabilities: [ThemeCapability::SolidPaint].into_iter().collect(),
        };

        let mut complete = TimelineEventThemeReceipt::from_expectations(expectations.clone());
        complete.record_checkpointed_event(0, None, true, None, true, Some("#123456"), true);
        assert!(complete.proves(1));
        assert!(complete.proves_rule(9, &pending));

        let mut wrong_value = TimelineEventThemeReceipt::from_expectations(expectations.clone());
        wrong_value.record_checkpointed_event(0, None, true, None, true, Some("#654321"), true);
        assert!(!wrong_value.proves(1));

        let mut unobserved = TimelineEventThemeReceipt::from_expectations(expectations);
        unobserved.record_checkpointed_event(0, None, true, None, true, Some("#123456"), false);
        assert!(!unobserved.proves(1));
    }

    #[test]
    fn timeline_event_receipt_distinguishes_classic_and_redux_baselines() {
        let expectations = vec![TimelineEventTerminalExpectation::baseline()].into_boxed_slice();
        let mut classic = TimelineEventThemeReceipt::from_expectations_with_baseline(
            expectations.clone(),
            Some(crate::timeline::MERMAID_EVENT_RADIUS_TOKEN),
        );
        classic.record_checkpointed_event(0, None, true, None, true, None, true);
        assert!(!classic.proves(1));

        let mut redux =
            TimelineEventThemeReceipt::from_expectations_with_baseline(expectations, None);
        redux.record_checkpointed_event(0, None, true, None, true, None, true);
        assert!(redux.proves(1));

        let mut malformed = TimelineEventThemeReceipt::new(1);
        malformed.record_checkpointed_event(0, None, true, None, false, None, true);
        assert!(!malformed.proves(1));
    }

    #[test]
    fn timeline_palette_receipt_rejects_wrong_order_values_and_line_shape() {
        let palette_expectations = vec![
            TimelinePaletteNodeExpectation {
                slot: 0,
                fill: Some(TimelinePalettePaint::typed(
                    "#123456",
                    ThemeCapability::SolidPaint,
                )),
                classic_line_stroke: Some("#edcba9".into()),
            },
            TimelinePaletteNodeExpectation {
                slot: 1,
                fill: Some(TimelinePalettePaint::typed(
                    "#654321",
                    ThemeCapability::SolidPaint,
                )),
                classic_line_stroke: Some("#9abcde".into()),
            },
        ]
        .into_boxed_slice();

        let mut wrong_order =
            TimelineEventThemeReceipt::from_expectations_with_baseline_and_palette(
                Box::new([]),
                None,
                palette_expectations.clone(),
            );
        wrong_order.record_palette_node(Some(1), Some("#654321"), Some("#9abcde"));
        wrong_order.record_palette_node(Some(0), Some("#123456"), Some("#edcba9"));
        assert!(!wrong_order.proves(0));

        let mut wrong_value =
            TimelineEventThemeReceipt::from_expectations_with_baseline_and_palette(
                Box::new([]),
                None,
                palette_expectations.clone(),
            );
        wrong_value.record_palette_node(Some(0), Some("#badbad"), Some("#edcba9"));
        wrong_value.record_palette_node(Some(1), Some("#654321"), Some("#9abcde"));
        assert!(!wrong_value.proves(0));

        let mut wrong_line = TimelineEventThemeReceipt::from_expectations_with_baseline_and_palette(
            Box::new([]),
            None,
            palette_expectations,
        );
        wrong_line.record_palette_node(Some(0), Some("#123456"), None);
        wrong_line.record_palette_node(Some(1), Some("#654321"), Some("#9abcde"));
        assert!(!wrong_line.proves(0));

        let mut complete = TimelineEventThemeReceipt::from_expectations_with_baseline_and_palette(
            Box::new([]),
            None,
            vec![
                TimelinePaletteNodeExpectation {
                    slot: 0,
                    fill: Some(TimelinePalettePaint::typed(
                        "#123456",
                        ThemeCapability::SolidPaint,
                    )),
                    classic_line_stroke: Some("#edcba9".into()),
                },
                TimelinePaletteNodeExpectation {
                    slot: 1,
                    fill: Some(TimelinePalettePaint::typed(
                        "#654321",
                        ThemeCapability::SolidPaint,
                    )),
                    classic_line_stroke: Some("#9abcde".into()),
                },
            ]
            .into_boxed_slice(),
        );
        complete.record_palette_node(Some(0), Some("#123456"), Some("#edcba9"));
        complete.record_palette_node(Some(1), Some("#654321"), Some("#9abcde"));
        assert!(complete.proves(0));

        let mut missing_last =
            TimelineEventThemeReceipt::from_expectations_with_baseline_and_palette(
                Box::new([]),
                None,
                complete.palette_expectations.clone(),
            );
        missing_last.record_palette_node(Some(0), Some("#123456"), Some("#edcba9"));
        assert!(!missing_last.proves(0));

        let mut extra = TimelineEventThemeReceipt::from_expectations_with_baseline_and_palette(
            Box::new([]),
            None,
            complete.palette_expectations.clone(),
        );
        extra.record_palette_node(Some(0), Some("#123456"), Some("#edcba9"));
        extra.record_palette_node(Some(1), Some("#654321"), Some("#9abcde"));
        extra.record_palette_node(Some(1), Some("#654321"), Some("#9abcde"));
        assert!(!extra.proves(0));

        let mut malformed = TimelineEventThemeReceipt::from_expectations_with_baseline_and_palette(
            Box::new([]),
            None,
            complete.palette_expectations,
        );
        malformed.record_palette_node(None, Some("#123456"), Some("#edcba9"));
        malformed.record_palette_node(Some(0), Some("#123456"), Some("#edcba9"));
        malformed.record_palette_node(Some(1), Some("#654321"), Some("#9abcde"));
        assert!(!malformed.proves(0));
    }
}

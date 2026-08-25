use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;
use merman_core::theme_color::{darken, lighten};

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    ResolvedDiagramTheme, Specified, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    TerminalVariantDomain, UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolve_direct_static_fill, resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

/// Mermaid 11.16 assigns `rx = ry = 5` directly to every Kanban item. Kanban source metadata and
/// site configuration expose no separate radius owner, so `Task.radius` can replace this value
/// without competing with a source-owned style channel. Section geometry remains independent.
pub(super) const MERMAID_TASK_RADIUS_PX: f64 = 5.0;
const MERMAID_TASK_PALETTE_SLOT_COUNT: usize = 12;

/// Kanban task geometry and evidence resolved once for concrete item occurrences.
#[derive(Debug)]
pub(crate) struct KanbanTaskTheme {
    items: Box<[KanbanTaskResolvedItem]>,
    evidence: FamilyThemeEvidence,
    pending_radius_key: Option<FamilyThemeMechanismKey>,
    pending_fill_keys: BTreeSet<FamilyThemeMechanismKey>,
    pending_stroke_keys: BTreeSet<FamilyThemeMechanismKey>,
    palette_key: Option<FamilyThemeMechanismKey>,
    pending_label_keys: BTreeSet<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<KanbanTaskThemeReceipt>,
}

#[derive(Debug)]
struct KanbanTaskResolvedItem {
    occurrence: KanbanTaskOccurrence,
    radius_px: f64,
    typed_fill: Option<KanbanTaskTypedFill>,
    typed_stroke: Option<KanbanTaskTypedStroke>,
    palette_fill: Option<KanbanTaskPaletteFill>,
    label_foreground: Option<KanbanTaskLabelForeground>,
}

#[derive(Debug)]
struct KanbanTaskPaletteFill {
    css: String,
    capability: ThemeCapability,
}

#[derive(Debug)]
struct KanbanTaskTypedFill {
    css: String,
    capability: ThemeCapability,
    rule_index: usize,
}

#[derive(Debug)]
struct KanbanTaskTypedStroke {
    css: String,
    capability: ThemeCapability,
    rule_index: usize,
}

#[derive(Debug)]
struct KanbanTaskLabelForeground {
    css: String,
    capability: ThemeCapability,
    rule_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum KanbanTaskLabelRole {
    Title,
    Ticket,
    Assigned,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct KanbanTaskLabelRoles(u8);

impl KanbanTaskLabelRoles {
    fn bit(role: KanbanTaskLabelRole) -> u8 {
        match role {
            KanbanTaskLabelRole::Title => 1 << 0,
            KanbanTaskLabelRole::Ticket => 1 << 1,
            KanbanTaskLabelRole::Assigned => 1 << 2,
        }
    }

    fn is_empty(self) -> bool {
        self.0 == 0
    }

    fn contains(self, role: KanbanTaskLabelRole) -> bool {
        self.0 & Self::bit(role) != 0
    }

    fn insert(&mut self, role: KanbanTaskLabelRole) -> bool {
        let bit = Self::bit(role);
        let inserted = self.0 & bit == 0;
        self.0 |= bit;
        inserted
    }
}

impl FromIterator<KanbanTaskLabelRole> for KanbanTaskLabelRoles {
    fn from_iter<T: IntoIterator<Item = KanbanTaskLabelRole>>(roles: T) -> Self {
        let mut collected = Self::default();
        for role in roles {
            collected.insert(role);
        }
        collected
    }
}

#[derive(Debug, Clone)]
pub(crate) struct KanbanTaskOccurrence {
    semantic_id: Box<str>,
    visible_labels: KanbanTaskLabelRoles,
    inheriting_labels: KanbanTaskLabelRoles,
}

impl KanbanTaskOccurrence {
    pub(crate) fn new(
        semantic_id: impl Into<Box<str>>,
        visible_labels: impl IntoIterator<Item = KanbanTaskLabelRole>,
    ) -> Self {
        let visible_labels = visible_labels.into_iter().collect();
        Self {
            semantic_id: semantic_id.into(),
            visible_labels,
            inheriting_labels: visible_labels,
        }
    }

    pub(crate) fn with_inheriting_labels(
        mut self,
        labels: impl IntoIterator<Item = KanbanTaskLabelRole>,
    ) -> Self {
        self.inheriting_labels = labels.into_iter().collect();
        debug_assert!(self.inheriting_labels.0 & !self.visible_labels.0 == 0);
        self
    }

    fn has_visible_labels(&self) -> bool {
        !self.visible_labels.is_empty()
    }

    fn has_inheriting_labels(&self) -> bool {
        !self.inheriting_labels.is_empty()
    }
}

impl KanbanTaskTheme {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        occurrences: Vec<KanbanTaskOccurrence>,
        effective_config: &MermaidConfig,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let item_count = occurrences.len();
        let Some(theme) = theme else {
            return Ok(Self::baseline_occurrences(occurrences));
        };

        let palette_disposition = theme.ordinal_palette_disposition(ThemeTarget::Task);
        let source_owned_card_fill = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.background",
        );
        let source_owned_card_stroke = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.nodeBorder",
        );
        // Query only the terminal path consumed by the Kanban writer. Core provenance may carry
        // derived ownership into `textColor`; upstream inputs such as `primaryTextColor` are not
        // enumerated here as a second ownership authority.
        let source_owned_label = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.textColor",
        );
        let mut items = Vec::with_capacity(item_count);
        let mut typed_fill_rule_occurrences = BTreeMap::<usize, usize>::new();
        let mut typed_stroke_rule_occurrences = BTreeMap::<usize, usize>::new();
        let mut label_rule_visible = BTreeSet::new();
        let mut label_rule_shadowed = BTreeSet::new();
        let mut winner_properties = BTreeSet::new();
        for (item_index, occurrence) in occurrences.into_iter().enumerate() {
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
            let radius_px = typed_radius_px(theme, &style);
            let typed_fill = if source_owned_card_fill {
                None
            } else {
                typed_task_fill(theme, &style)
            };
            if let Some(fill) = typed_fill.as_ref() {
                *typed_fill_rule_occurrences
                    .entry(fill.rule_index)
                    .or_default() += 1;
            }
            let typed_stroke = if source_owned_card_stroke {
                None
            } else {
                typed_task_stroke(theme, &style)
            };
            if let Some(stroke) = typed_stroke.as_ref() {
                *typed_stroke_rule_occurrences
                    .entry(stroke.rule_index)
                    .or_default() += 1;
            }
            let palette_fill = if !source_owned_card_fill
                && palette_disposition == Some(FamilyThemeDisposition::TypedAdapter)
                && matches!(style.fill_resolution().specified(), Specified::Unspecified)
            {
                let palette_ordinal = item_index % MERMAID_TASK_PALETTE_SLOT_COUNT + 1;
                match theme.series_color(ThemeTarget::Task, palette_ordinal) {
                    Some(color) => Some(KanbanTaskPaletteFill {
                        css: color.as_css(),
                        capability: if color.is_transparent() {
                            ThemeCapability::TransparentPaint
                        } else {
                            ThemeCapability::SolidPaint
                        },
                    }),
                    None => None,
                }
            } else {
                None
            };

            let label_style = theme.style_with_work_meter(
                ThemeTarget::TaskLabel,
                ThemeVariant::Default,
                Some(item_index + 1),
                work_meter,
            )?;
            let has_visible_labels = occurrence.has_visible_labels();
            if has_visible_labels {
                winner_properties.extend(
                    label_style
                        .winner_rule_properties()
                        .into_iter()
                        .map(|(property, origin)| (origin.rule_index(), property)),
                );
            }
            let label_candidate = if has_visible_labels && !source_owned_label {
                typed_label_foreground(theme, &label_style)
            } else {
                None
            };
            let label_foreground = if occurrence.has_inheriting_labels() {
                label_candidate
            } else {
                if let Some(label) = label_candidate.as_ref() {
                    label_rule_shadowed.insert(label.rule_index);
                }
                None
            };
            if let Some(label) = label_foreground.as_ref() {
                label_rule_visible.insert(label.rule_index);
            }
            debug_assert!(has_visible_labels || label_foreground.is_none());
            items.push(KanbanTaskResolvedItem {
                occurrence,
                radius_px,
                typed_fill,
                typed_stroke,
                palette_fill,
                label_foreground,
            });
        }

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<(usize, ThemeTarget), KanbanTaskRuleObservation>::new();
        let mut palette_key = None;
        let mut pending_fill_keys = BTreeSet::new();
        let mut pending_stroke_keys = BTreeSet::new();
        let mut pending_label_keys = BTreeSet::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: target @ (ThemeTarget::Task | ThemeTarget::TaskLabel),
                    facet,
                    ..
                } => {
                    let observation = observations.entry((rule_index, target)).or_default();
                    if !winner_properties
                        .contains(&(rule_index, resolved_style_property_for_facet(facet)))
                    {
                        continue;
                    }
                    observation.applicable = true;
                    match (target, route.disposition(), facet) {
                        (
                            ThemeTarget::TaskLabel,
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeRuleFacet::Fill(
                                crate::diagram_theme::FamilyThemePaintKind::Transparent
                                | crate::diagram_theme::FamilyThemePaintKind::Solid,
                            ),
                        ) => {
                            if source_owned_label {
                                observation.suppressed = true;
                            } else if label_rule_visible.contains(&rule_index) {
                                observation.label_pending = true;
                            } else if label_rule_shadowed.contains(&rule_index) {
                                observation.suppressed = true;
                            } else {
                                observation.residual =
                                    Some(FamilyThemeResidualReason::UnsupportedPaint);
                            }
                        }
                        (
                            ThemeTarget::Task,
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeRuleFacet::Fill(
                                crate::diagram_theme::FamilyThemePaintKind::Transparent
                                | crate::diagram_theme::FamilyThemePaintKind::Solid,
                            ),
                        ) => {
                            if source_owned_card_fill {
                                observation.suppressed = true;
                            } else {
                                match typed_fill_rule_occurrences.get(&rule_index).copied() {
                                    Some(count) if count == item_count => {
                                        observation.fill_pending = true;
                                    }
                                    Some(_) | None => {
                                        observation.residual =
                                            Some(FamilyThemeResidualReason::UnsupportedPaint);
                                    }
                                }
                            }
                        }
                        (
                            ThemeTarget::Task,
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeRuleFacet::Stroke(
                                crate::diagram_theme::FamilyThemePaintKind::Transparent
                                | crate::diagram_theme::FamilyThemePaintKind::Solid,
                            ),
                        ) => {
                            if source_owned_card_stroke {
                                observation.suppressed = true;
                            } else {
                                match typed_stroke_rule_occurrences.get(&rule_index).copied() {
                                    Some(count) if count == item_count => {
                                        observation.stroke_pending = true;
                                    }
                                    Some(_) | None => {
                                        observation.residual =
                                            Some(FamilyThemeResidualReason::UnsupportedPaint);
                                    }
                                }
                            }
                        }
                        (
                            ThemeTarget::Task,
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeRuleFacet::Radius,
                        ) => {
                            observation.radius_pending = true;
                        }
                        (_, FamilyThemeDisposition::Unsupported, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (_, FamilyThemeDisposition::TypedAdapter, _)
                        | (_, FamilyThemeDisposition::LegacyCompatibility, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Task,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if item_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else {
                        match route.disposition() {
                            FamilyThemeDisposition::TypedAdapter if source_owned_card_fill => {
                                evidence.mark_not_applicable(key);
                            }
                            FamilyThemeDisposition::TypedAdapter => palette_key = Some(key),
                            FamilyThemeDisposition::Unsupported => evidence.mark_residual(
                                key,
                                FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                            ),
                            FamilyThemeDisposition::LegacyCompatibility => {}
                        }
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Task,
                    ..
                } => {
                    // Effect bindings are selected by the final terminal winner. Reconcile
                    // them after all route metadata is collected so an explicit rule or Clear
                    // can suppress the fallback binding without a false residual.
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Task,
                TerminalVariantDomain::uniform(item_count, ThemeVariant::Default),
            )],
            work_meter,
        )?;

        let mut pending_radius_key = None;
        for ((rule_index, target), observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // A mixed rule cannot be signed Applied until every winning facet is accounted.
            } else if observation.suppressed {
                evidence.mark_not_applicable(key);
            } else {
                let mut pending = false;
                if observation.radius_pending {
                    debug_assert!(pending_radius_key.is_none());
                    pending_radius_key = Some(key.clone());
                    pending = true;
                }
                if observation.fill_pending {
                    pending_fill_keys.insert(key.clone());
                    pending = true;
                }
                if observation.stroke_pending {
                    pending_stroke_keys.insert(key.clone());
                    pending = true;
                }
                if observation.label_pending {
                    pending_label_keys.insert(key.clone());
                    pending = true;
                }
                if !pending {
                    evidence.mark_not_applicable(key);
                }
            }
        }

        Ok(Self {
            items: items.into_boxed_slice(),
            evidence,
            pending_radius_key,
            pending_fill_keys,
            pending_stroke_keys,
            palette_key,
            pending_label_keys,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline(item_count: usize) -> Self {
        Self::baseline_occurrences(
            (0..item_count)
                .map(|index| KanbanTaskOccurrence::new(format!("item-{index}"), []))
                .collect(),
        )
    }

    fn baseline_occurrences(occurrences: Vec<KanbanTaskOccurrence>) -> Self {
        Self {
            items: occurrences
                .into_iter()
                .map(|occurrence| KanbanTaskResolvedItem {
                    occurrence,
                    radius_px: MERMAID_TASK_RADIUS_PX,
                    typed_fill: None,
                    typed_stroke: None,
                    palette_fill: None,
                    label_foreground: None,
                })
                .collect(),
            evidence: FamilyThemeEvidence::default(),
            pending_radius_key: None,
            pending_fill_keys: BTreeSet::new(),
            pending_stroke_keys: BTreeSet::new(),
            palette_key: None,
            pending_label_keys: BTreeSet::new(),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn radius_px(&self, item_index: usize) -> Option<f64> {
        self.items.get(item_index).map(|item| item.radius_px)
    }

    pub(crate) fn terminal_decisions(
        &self,
        config: &MermaidConfig,
    ) -> crate::Result<Vec<KanbanTaskTerminalDecision>> {
        let dark_mode = config
            .get_bool("darkMode")
            .or_else(|| config.get_bool("themeVariables.darkMode"))
            .unwrap_or(false);
        self.items
            .iter()
            .map(|item| Self::terminal_decision(item, dark_mode))
            .collect()
    }

    fn terminal_decision(
        item: &KanbanTaskResolvedItem,
        dark_mode: bool,
    ) -> crate::Result<KanbanTaskTerminalDecision> {
        let (fill_css, fill_capability, fill_rule_index) =
            if let Some(fill) = item.typed_fill.as_ref() {
                (
                    Some(fill.css.clone()),
                    Some(fill.capability),
                    Some(fill.rule_index),
                )
            } else {
                let fill = item.palette_fill.as_ref();
                let fill_css = match fill {
                    Some(fill) if dark_mode => Some(darken(&fill.css, 10.0)?),
                    Some(fill) => Some(lighten(&fill.css, 10.0)?),
                    None => None,
                };
                (fill_css, fill.map(|fill| fill.capability), None)
            };
        let stroke = item.typed_stroke.as_ref();
        let label = item.label_foreground.as_ref();
        Ok(KanbanTaskTerminalDecision {
            fill_css,
            fill_capability,
            fill_rule_index,
            stroke_css: stroke.map(|stroke| stroke.css.clone()),
            stroke_capability: stroke.map(|stroke| stroke.capability),
            stroke_rule_index: stroke.map(|stroke| stroke.rule_index),
            label_css: label.map(|label| label.css.clone()),
            label_capability: label.map(|label| label.capability),
            label_rule_index: label.map(|label| label.rule_index),
        })
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        decisions: &[KanbanTaskTerminalDecision],
    ) -> Option<KanbanTaskThemeReceipt> {
        (self.pending_radius_key.is_some()
            || !self.pending_fill_keys.is_empty()
            || !self.pending_stroke_keys.is_empty()
            || self.palette_key.is_some()
            || !self.pending_label_keys.is_empty())
        .then(|| KanbanTaskThemeReceipt::new(&self.items, decisions))
    }

    pub(crate) fn record_terminal(&self, receipt: KanbanTaskThemeReceipt) -> bool {
        (self.pending_radius_key.is_some()
            || !self.pending_fill_keys.is_empty()
            || !self.pending_stroke_keys.is_empty()
            || self.palette_key.is_some()
            || !self.pending_label_keys.is_empty())
            && receipt.proves(self.items.len())
            && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if let Some(receipt) = self.terminal_receipt.get() {
            if let Some(key) = self.pending_radius_key.clone() {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::RoundedGeometry]);
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
            for key in &self.pending_fill_keys {
                let FamilyThemeMechanismKey::Rule { index, .. } = key else {
                    unreachable!("Kanban task fill evidence must be rule-backed")
                };
                if receipt.proves_fill_rule(*index) {
                    evidence.mark_applied_with_capabilities(
                        key.clone(),
                        receipt.typed_fill_capabilities[index].iter().copied(),
                    );
                } else {
                    evidence
                        .mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                }
            }
            for key in &self.pending_stroke_keys {
                let FamilyThemeMechanismKey::Rule { index, .. } = key else {
                    unreachable!("Kanban task stroke evidence must be rule-backed")
                };
                if receipt.proves_stroke_rule(*index) {
                    evidence.mark_applied_with_capabilities(
                        key.clone(),
                        receipt.typed_stroke_capabilities[index].iter().copied(),
                    );
                } else {
                    evidence
                        .mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                }
            }
            for key in &self.pending_label_keys {
                let FamilyThemeMechanismKey::Rule { index, .. } = key else {
                    unreachable!("Kanban task-label evidence must be rule-backed")
                };
                if receipt.proves_label_rule(*index) {
                    evidence.mark_applied_with_capabilities(
                        key.clone(),
                        receipt.label_capabilities[index].iter().copied(),
                    );
                } else {
                    evidence
                        .mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                }
            }
        } else {
            for key in &self.pending_fill_keys {
                evidence.mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
            }
            for key in &self.pending_stroke_keys {
                evidence.mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
            }
            if let Some(key) = self.palette_key.clone() {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette);
            }
            for key in &self.pending_label_keys {
                evidence.mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
            }
        }
        evidence
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KanbanTaskTerminalDecision {
    fill_css: Option<String>,
    fill_capability: Option<ThemeCapability>,
    fill_rule_index: Option<usize>,
    stroke_css: Option<String>,
    stroke_capability: Option<ThemeCapability>,
    stroke_rule_index: Option<usize>,
    label_css: Option<String>,
    label_capability: Option<ThemeCapability>,
    label_rule_index: Option<usize>,
}

impl KanbanTaskTerminalDecision {
    pub(crate) fn fill_css(&self) -> Option<&str> {
        self.fill_css.as_deref()
    }

    pub(crate) fn label_css(&self) -> Option<&str> {
        self.label_css.as_deref()
    }

    pub(crate) fn stroke_css(&self) -> Option<&str> {
        self.stroke_css.as_deref()
    }
}

fn typed_label_foreground(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> Option<KanbanTaskLabelForeground> {
    let origin = style.fill_resolution().winner()?;
    let facet = FamilyThemeRuleFacet::fill(style.fill_resolution().specified())?;
    if theme.rule_facet_disposition(origin.rule_index(), facet)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    match style.fill_resolution().specified() {
        Specified::Value(crate::diagram_theme::CanvasPaint::Transparent) => {
            Some(KanbanTaskLabelForeground {
                css: "transparent".to_string(),
                capability: ThemeCapability::TransparentPaint,
                rule_index: origin.rule_index(),
            })
        }
        Specified::Value(crate::diagram_theme::CanvasPaint::Solid(color)) => {
            Some(KanbanTaskLabelForeground {
                css: color.as_css(),
                capability: ThemeCapability::SolidPaint,
                rule_index: origin.rule_index(),
            })
        }
        Specified::Unspecified
        | Specified::Clear
        | Specified::Value(
            crate::diagram_theme::CanvasPaint::LinearGradient(_)
            | crate::diagram_theme::CanvasPaint::RadialGradient(_)
            | crate::diagram_theme::CanvasPaint::Pattern(_),
        ) => None,
    }
}

fn typed_task_fill(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> Option<KanbanTaskTypedFill> {
    let fill = resolve_direct_static_fill(
        theme,
        style,
        &[ThemeTarget::Task],
        DirectStaticSelectorDomain::Unqualified,
    )?;
    let (css, rule_index, capability) = fill.into_parts();
    Some(KanbanTaskTypedFill {
        css: css.into(),
        capability,
        rule_index,
    })
}

fn typed_task_stroke(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> Option<KanbanTaskTypedStroke> {
    let stroke = resolve_direct_static_stroke(
        theme,
        style,
        &[ThemeTarget::Task],
        DirectStaticSelectorDomain::Unqualified,
    )?;
    let (css, rule_index, capability) = stroke.into_parts();
    Some(KanbanTaskTypedStroke {
        css: css.into(),
        capability,
        rule_index,
    })
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

/// Writer-owned proof that every Kanban item emitted its exact terminal surface values.
#[derive(Debug)]
pub(crate) struct KanbanTaskThemeReceipt {
    items: Vec<KanbanTaskReceiptItem>,
    schema_valid: bool,
    palette_capabilities: BTreeSet<ThemeCapability>,
    typed_fill_capabilities: BTreeMap<usize, BTreeSet<ThemeCapability>>,
    typed_stroke_capabilities: BTreeMap<usize, BTreeSet<ThemeCapability>>,
    label_capabilities: BTreeMap<usize, BTreeSet<ThemeCapability>>,
    unverified_label_rules: BTreeSet<usize>,
}

#[derive(Debug)]
struct KanbanTaskReceiptItem {
    semantic_id: Box<str>,
    rect_checkpointed: bool,
    expected_labels: KanbanTaskLabelRoles,
    emitted_labels: KanbanTaskLabelRoles,
}

impl KanbanTaskThemeReceipt {
    fn new(items: &[KanbanTaskResolvedItem], decisions: &[KanbanTaskTerminalDecision]) -> Self {
        let schema_valid = items.len() == decisions.len();
        Self {
            items: items
                .iter()
                .zip(decisions)
                .map(|(item, decision)| KanbanTaskReceiptItem {
                    semantic_id: item.occurrence.semantic_id.clone(),
                    rect_checkpointed: false,
                    expected_labels: decision
                        .label_css()
                        .map(|_| item.occurrence.inheriting_labels)
                        .unwrap_or_default(),
                    emitted_labels: KanbanTaskLabelRoles::default(),
                })
                .collect(),
            schema_valid,
            palette_capabilities: BTreeSet::new(),
            typed_fill_capabilities: BTreeMap::new(),
            typed_stroke_capabilities: BTreeMap::new(),
            label_capabilities: BTreeMap::new(),
            unverified_label_rules: BTreeSet::new(),
        }
    }

    pub(crate) fn record_checkpointed_item(
        &mut self,
        item_index: usize,
        semantic_id: &str,
        attributes_match: bool,
        terminal_decision: &KanbanTaskTerminalDecision,
    ) {
        let Some(item) = self.items.get_mut(item_index) else {
            self.schema_valid = false;
            return;
        };
        if item.rect_checkpointed {
            self.schema_valid = false;
            return;
        }
        item.rect_checkpointed = true;
        self.schema_valid &= item.semantic_id.as_ref() == semantic_id && attributes_match;
        if let Some(fill_capability) = terminal_decision.fill_capability {
            if let Some(rule_index) = terminal_decision.fill_rule_index {
                self.typed_fill_capabilities
                    .entry(rule_index)
                    .or_default()
                    .insert(fill_capability);
            } else {
                self.palette_capabilities.insert(fill_capability);
            }
        }
        if let Some(stroke_capability) = terminal_decision.stroke_capability {
            if let Some(rule_index) = terminal_decision.stroke_rule_index {
                self.typed_stroke_capabilities
                    .entry(rule_index)
                    .or_default()
                    .insert(stroke_capability);
            }
        }
    }

    pub(crate) fn record_label(
        &mut self,
        item_index: usize,
        semantic_id: &str,
        role: KanbanTaskLabelRole,
        actual_group_style: &str,
        actual_div_style: &str,
        visible_run_count: usize,
        inherited_color_run_count: usize,
        decision: &KanbanTaskTerminalDecision,
    ) {
        let Some(item) = self.items.get_mut(item_index) else {
            self.schema_valid = false;
            return;
        };
        let (Some(expected_css), Some(capability), Some(rule_index)) = (
            decision.label_css.as_deref(),
            decision.label_capability,
            decision.label_rule_index,
        ) else {
            self.schema_valid = false;
            return;
        };
        self.schema_valid &= item.semantic_id.as_ref() == semantic_id;
        self.schema_valid &= item.expected_labels.contains(role);
        self.schema_valid &= item.emitted_labels.insert(role);
        self.schema_valid &= visible_run_count > 0;
        self.schema_valid &=
            inherited_color_run_count > 0 && inherited_color_run_count <= visible_run_count;
        if inherited_color_run_count != visible_run_count {
            self.unverified_label_rules.insert(rule_index);
        }
        self.schema_valid &=
            terminal_style_property(actual_group_style, "color") == Some(expected_css);
        self.schema_valid &=
            terminal_style_property(actual_group_style, "fill") == Some(expected_css);
        self.schema_valid &=
            terminal_style_property(actual_div_style, "color") == Some(expected_css);
        self.label_capabilities
            .entry(rule_index)
            .or_default()
            .insert(capability);
    }

    fn proves_label_rule(&self, rule_index: usize) -> bool {
        self.schema_valid
            && !self.unverified_label_rules.contains(&rule_index)
            && self
                .label_capabilities
                .get(&rule_index)
                .is_some_and(|capabilities| !capabilities.is_empty())
    }

    fn proves_fill_rule(&self, rule_index: usize) -> bool {
        self.schema_valid
            && self
                .typed_fill_capabilities
                .get(&rule_index)
                .is_some_and(|capabilities| !capabilities.is_empty())
    }

    fn proves_stroke_rule(&self, rule_index: usize) -> bool {
        self.schema_valid
            && self
                .typed_stroke_capabilities
                .get(&rule_index)
                .is_some_and(|capabilities| !capabilities.is_empty())
    }

    fn terminal_occurrences_complete(&self) -> bool {
        self.items
            .iter()
            .all(|item| item.rect_checkpointed && item.expected_labels == item.emitted_labels)
    }

    fn proves(&self, expected_item_count: usize) -> bool {
        self.items.len() == expected_item_count
            && self.schema_valid
            && self.terminal_occurrences_complete()
    }
}

fn terminal_style_property<'a>(style: &'a str, property: &str) -> Option<&'a str> {
    style
        .split(';')
        .filter_map(crate::mermaid_style::parse_style_declaration)
        .filter(|declaration| declaration.property() == property)
        .map(|declaration| declaration.value())
        .next_back()
}

#[derive(Debug, Default)]
struct KanbanTaskRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    radius_pending: bool,
    fill_pending: bool,
    stroke_pending: bool,
    label_pending: bool,
    suppressed: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        KanbanTaskLabelRole, KanbanTaskOccurrence, KanbanTaskResolvedItem,
        KanbanTaskTerminalDecision, KanbanTaskTheme, KanbanTaskThemeReceipt,
        MERMAID_TASK_RADIUS_PX,
    };
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramEffectSet, DiagramThemeCompiler, DiagramThemeSpec, EffectBinding, EffectGraph,
        EffectInput, EffectPrimitive, FamilyThemeMechanismKey, ThemeCapability, ThemeRule,
        ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    };
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
    use merman_core::MermaidConfig;

    fn label_decision() -> KanbanTaskTerminalDecision {
        KanbanTaskTerminalDecision {
            fill_css: None,
            fill_capability: None,
            fill_rule_index: None,
            stroke_css: None,
            stroke_capability: None,
            stroke_rule_index: None,
            label_css: Some("#000000".to_string()),
            label_capability: Some(ThemeCapability::SolidPaint),
            label_rule_index: Some(7),
        }
    }

    fn palette_decision() -> KanbanTaskTerminalDecision {
        KanbanTaskTerminalDecision {
            fill_css: Some("#ffffff".to_string()),
            fill_capability: Some(ThemeCapability::SolidPaint),
            fill_rule_index: None,
            stroke_css: None,
            stroke_capability: None,
            stroke_rule_index: None,
            label_css: None,
            label_capability: None,
            label_rule_index: None,
        }
    }

    fn resolved_item(occurrence: KanbanTaskOccurrence) -> KanbanTaskResolvedItem {
        KanbanTaskResolvedItem {
            occurrence,
            radius_px: MERMAID_TASK_RADIUS_PX,
            typed_fill: None,
            typed_stroke: None,
            palette_fill: None,
            label_foreground: None,
        }
    }

    #[test]
    fn task_theme_receipt_requires_each_terminal_occurrence_once() {
        let item = resolved_item(KanbanTaskOccurrence::new(
            "task",
            [KanbanTaskLabelRole::Title, KanbanTaskLabelRole::Ticket],
        ));
        let decision = label_decision();

        let mut incomplete = KanbanTaskThemeReceipt::new(
            std::slice::from_ref(&item),
            std::slice::from_ref(&decision),
        );
        incomplete.record_checkpointed_item(0, "task", true, &decision);
        incomplete.record_label(
            0,
            "task",
            KanbanTaskLabelRole::Title,
            "color:#000000;fill:#000000;text-align:left !important",
            "color: #000000; text-align:center",
            1,
            1,
            &decision,
        );
        assert!(!incomplete.proves(1));

        let mut mismatched = KanbanTaskThemeReceipt::new(
            std::slice::from_ref(&item),
            std::slice::from_ref(&decision),
        );
        mismatched.record_checkpointed_item(0, "task", true, &decision);
        for role in [KanbanTaskLabelRole::Title, KanbanTaskLabelRole::Ticket] {
            mismatched.record_label(
                0,
                "task",
                role,
                "color:#ffffff;fill:#000000;text-align:left !important",
                "color: #000000; text-align:center",
                1,
                1,
                &decision,
            );
        }
        assert!(!mismatched.proves(1));

        let mut complete = KanbanTaskThemeReceipt::new(&[item], std::slice::from_ref(&decision));
        complete.record_checkpointed_item(0, "task", true, &decision);
        for role in [KanbanTaskLabelRole::Title, KanbanTaskLabelRole::Ticket] {
            complete.record_label(
                0,
                "task",
                role,
                "color:#000000;fill:#000000;text-align:left !important",
                "color: #000000; text-align:center",
                1,
                1,
                &decision,
            );
        }
        assert!(complete.proves(1));
        assert!(complete.proves_label_rule(7));

        complete.record_label(
            0,
            "task",
            KanbanTaskLabelRole::Title,
            "color:#000000;fill:#000000;text-align:left !important",
            "color: #000000; text-align:center",
            1,
            1,
            &decision,
        );
        assert!(!complete.proves(1));
    }

    #[test]
    fn task_theme_receipt_does_not_require_labels_for_a_palette_only_route() {
        let item = resolved_item(KanbanTaskOccurrence::new(
            "task",
            [KanbanTaskLabelRole::Title],
        ));
        let decision = palette_decision();
        let mut receipt = KanbanTaskThemeReceipt::new(&[item], std::slice::from_ref(&decision));

        receipt.record_checkpointed_item(0, "task", true, &decision);

        assert!(receipt.proves(1));
        assert_eq!(
            receipt.palette_capabilities,
            [ThemeCapability::SolidPaint].into_iter().collect()
        );
        assert!(!receipt.proves_label_rule(7));
    }

    #[test]
    fn task_effect_binding_yields_to_an_explicit_effect_winner() {
        let bound_effect_id = "kanban-task-bound";
        let explicit_effect_id = "kanban-task-explicit";
        let effects = DiagramEffectSet::default()
            .with_graph(
                EffectGraph::new(
                    bound_effect_id,
                    [EffectPrimitive::GaussianBlur {
                        input: EffectInput::SourceGraphic,
                        std_deviation: 1.0,
                    }],
                )
                .expect("valid bound Kanban effect graph"),
            )
            .expect("unique bound Kanban effect graph")
            .with_graph(
                EffectGraph::new(
                    explicit_effect_id,
                    [EffectPrimitive::GaussianBlur {
                        input: EffectInput::SourceGraphic,
                        std_deviation: 2.0,
                    }],
                )
                .expect("valid explicit Kanban effect graph"),
            )
            .expect("unique explicit Kanban effect graph")
            .with_binding(
                EffectBinding::new(ThemeTarget::Task, bound_effect_id)
                    .expect("valid Kanban task effect binding"),
            )
            .expect("unique Kanban task effect binding");
        let explicit_rule = ThemeRule::new(
            ThemeTarget::Task,
            ThemeStylePatch::default()
                .with_effect(explicit_effect_id)
                .expect("valid explicit Kanban task effect"),
        )
        .for_family(DiagramFamilyId::KANBAN);
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_styles(ThemeRuleSet::default().with_rule(explicit_rule))
                    .with_effects(effects),
            )
            .expect("compile Kanban effect evidence fixture")
            .resolve(DiagramFamilyId::KANBAN);
        let binding_key = FamilyThemeMechanismKey::EffectBinding {
            target: ThemeTarget::Task,
            effect_id: bound_effect_id.to_string(),
        };

        let task_theme = KanbanTaskTheme::resolve(
            Some(&theme),
            vec![KanbanTaskOccurrence::new("task", [])],
            &MermaidConfig::default(),
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .expect("resolve Kanban task effect evidence fixture");
        let evidence = task_theme.finish_evidence();

        assert_eq!(
            evidence.not_applicable_mechanisms(),
            std::slice::from_ref(&binding_key)
        );
        assert!(
            evidence
                .residuals()
                .iter()
                .all(|residual| residual.key() != &binding_key)
        );
    }
}

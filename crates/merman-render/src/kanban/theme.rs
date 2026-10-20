use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;
use merman_core::theme_color::{darken, lighten};

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    ResolvedDiagramTheme, Specified, ThemeCapability, ThemeTarget, ThemeTypographyProperty,
    ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    InheritedFontStackOutcome, InheritedFontStackPlan, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

/// Mermaid 11.16 assigns `rx = ry = 5` directly to every Kanban item. Kanban source metadata and
/// site configuration expose no separate radius owner, so `Task.radius` can replace this value
/// without competing with a source-owned style channel. Section geometry remains independent.
pub(super) const MERMAID_TASK_RADIUS_PX: f64 = 5.0;
const MERMAID_TASK_PALETTE_SLOT_COUNT: usize = 12;
const MERMAID_BASE_FONT_SIZE_PX: f64 = 16.0;

fn positive_font_size(value: Option<f64>) -> Option<f64> {
    value.filter(|value| value.is_finite() && *value > 0.0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KanbanFontSizeOutcome {
    Inactive,
    Typed,
    ConfigOwned,
    UnverifiedConfig,
}

/// Resolves Kanban's inherited typography once for measurement, layout, CSS, and evidence.
#[derive(Debug)]
pub(crate) struct KanbanTypographyPlan {
    inherited_font_stack: InheritedFontStackPlan,
    font_size_css: Option<Box<str>>,
    font_size_px: f64,
    typed_font_size_requested: bool,
    font_size_outcome: KanbanFontSizeOutcome,
}

impl KanbanTypographyPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let typed_font_size_requested = theme.is_some_and(|theme| {
            theme.family_mechanism_routes().iter().any(|route| {
                route.mechanism()
                    == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                    && route.disposition() == FamilyThemeDisposition::TypedAdapter
            })
        });
        let config_owns_font_size = typed_font_size_requested
            && merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "themeVariables.fontSize",
            );
        let configured_theme_font_size = config_owns_font_size
            .then(|| {
                positive_font_size(crate::config::config_f64_css_px(
                    effective_config.as_value(),
                    &["themeVariables", "fontSize"],
                ))
            })
            .flatten();
        let fallback_font_size_px = || {
            positive_font_size(crate::config::config_theme_or_root_font_size_px_opt(
                effective_config.as_value(),
            ))
            .unwrap_or(MERMAID_BASE_FONT_SIZE_PX)
            .max(1.0)
        };
        let fallback_font_size_css = || {
            positive_font_size(
                crate::config::config_theme_font_size_css_or_root_number_px_opt(
                    effective_config.as_value(),
                ),
            )
            .unwrap_or(MERMAID_BASE_FONT_SIZE_PX)
            .max(1.0)
        };
        let resolved_css_requested =
            inherited_font_stack.typed_font_stack_requested() || typed_font_size_requested;
        let font_size_outcome = match () {
            _ if !typed_font_size_requested => KanbanFontSizeOutcome::Inactive,
            _ if !config_owns_font_size => KanbanFontSizeOutcome::Typed,
            _ if configured_theme_font_size.is_some() => KanbanFontSizeOutcome::ConfigOwned,
            _ => KanbanFontSizeOutcome::UnverifiedConfig,
        };
        let (font_size_px, font_size_css) = match (theme, font_size_outcome) {
            (Some(theme), KanbanFontSizeOutcome::Typed) => {
                let font_size_px = theme.typography().font_size_px().max(1.0);
                (
                    f64::from(font_size_px),
                    Some(format!("{font_size_px}px").into_boxed_str()),
                )
            }
            (_, KanbanFontSizeOutcome::ConfigOwned) => {
                let font_size_px = configured_theme_font_size
                    .expect("validated Kanban theme font-size ownership")
                    .max(1.0);
                (
                    font_size_px,
                    Some(format!("{font_size_px}px").into_boxed_str()),
                )
            }
            _ => (
                fallback_font_size_px(),
                resolved_css_requested
                    .then(|| format!("{}px", fallback_font_size_css()).into_boxed_str()),
            ),
        };

        Self {
            inherited_font_stack,
            font_size_css,
            font_size_px,
            typed_font_size_requested,
            font_size_outcome,
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) const fn font_size_px(&self) -> f64 {
        self.font_size_px
    }

    pub(crate) fn typography_requested(&self) -> bool {
        self.inherited_font_stack.typography_requested() || self.typed_font_size_requested
    }
}

/// Kanban task geometry and evidence resolved once for concrete item occurrences.
#[derive(Debug)]
pub(crate) struct KanbanTaskTheme {
    item_count: usize,
    items: Option<Box<[KanbanTaskResolvedItem]>>,
    typography: KanbanTypographyPlan,
    evidence: FamilyThemeEvidence,
    pending_radius_key: Option<FamilyThemeMechanismKey>,
    pending_fill_keys: BTreeSet<FamilyThemeMechanismKey>,
    pending_stroke_keys: BTreeSet<FamilyThemeMechanismKey>,
    palette_key: Option<FamilyThemeMechanismKey>,
    pending_label_keys: BTreeSet<FamilyThemeMechanismKey>,
    terminal_outcome: OnceLock<KanbanTaskThemeOutcome>,
}

/// Inherited typography ownership aggregated from the labels actually emitted by Kanban.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KanbanTypographyFacts {
    visible_run_count: usize,
    inherited_font_family_run_count: usize,
    unverified_font_family_run_count: usize,
    inherited_font_size_run_count: usize,
    unverified_font_size_run_count: usize,
    parse_valid: bool,
}

/// Local style facts carried by one checkpointed Kanban text terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct KanbanTypographyTerminalFacts {
    pub(crate) parse_valid: bool,
    pub(crate) visible_run_count: usize,
    pub(crate) inherited_font_family_run_count: usize,
    pub(crate) unverified_font_family_run_count: usize,
    pub(crate) inherited_font_size_run_count: usize,
    pub(crate) unverified_font_size_run_count: usize,
}

impl KanbanTypographyTerminalFacts {
    pub(crate) fn from_visible_style_facts(facts: &crate::text::VisibleTextStyleFacts) -> Self {
        Self {
            parse_valid: facts.parse_valid(),
            visible_run_count: facts.visible_run_count(),
            inherited_font_family_run_count: facts.inherited_font_family_run_count(),
            unverified_font_family_run_count: facts.unverified_font_family_run_count(),
            inherited_font_size_run_count: facts.inherited_font_size_run_count(),
            unverified_font_size_run_count: facts.unverified_font_size_run_count(),
        }
    }
}

impl Default for KanbanTypographyFacts {
    fn default() -> Self {
        Self {
            visible_run_count: 0,
            inherited_font_family_run_count: 0,
            unverified_font_family_run_count: 0,
            inherited_font_size_run_count: 0,
            unverified_font_size_run_count: 0,
            parse_valid: true,
        }
    }
}

impl KanbanTypographyFacts {
    fn record_terminal(&mut self, facts: KanbanTypographyTerminalFacts) {
        self.visible_run_count = self
            .visible_run_count
            .saturating_add(facts.visible_run_count);
        self.inherited_font_family_run_count = self
            .inherited_font_family_run_count
            .saturating_add(facts.inherited_font_family_run_count);
        self.unverified_font_family_run_count = self
            .unverified_font_family_run_count
            .saturating_add(facts.unverified_font_family_run_count);
        self.inherited_font_size_run_count = self
            .inherited_font_size_run_count
            .saturating_add(facts.inherited_font_size_run_count);
        self.unverified_font_size_run_count = self
            .unverified_font_size_run_count
            .saturating_add(facts.unverified_font_size_run_count);
        self.parse_valid &= facts.parse_valid;
    }
}

#[derive(Debug)]
struct KanbanTaskResolvedItem {
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

#[derive(Debug, Clone, Copy)]
pub(crate) struct KanbanTaskOccurrence {
    visible_labels: bool,
    inheriting_labels: bool,
}

impl KanbanTaskOccurrence {
    pub(crate) fn new(visible_labels: bool, inheriting_labels: bool) -> Self {
        Self {
            visible_labels,
            inheriting_labels,
        }
    }

    fn has_visible_labels(&self) -> bool {
        self.visible_labels
    }

    fn has_inheriting_labels(&self) -> bool {
        self.inheriting_labels
    }
}

impl KanbanTaskTheme {
    pub(crate) fn resolve_with_typography(
        theme: &ResolvedDiagramTheme,
        occurrences: Vec<KanbanTaskOccurrence>,
        effective_config: &MermaidConfig,
        typography: KanbanTypographyPlan,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let item_count = occurrences.len();

        let palette_disposition = theme.ordinal_palette_disposition(ThemeTarget::Task);
        let source_owned_card_fill = merman_core::__private::config_paint_overrides_typed_default(
            effective_config,
            "themeVariables.background",
        );
        let source_owned_card_stroke = merman_core::__private::config_paint_overrides_typed_default(
            effective_config,
            "themeVariables.nodeBorder",
        );
        // Query only the terminal path consumed by the Kanban writer. Core provenance may carry
        // derived ownership into `textColor`; upstream inputs such as `primaryTextColor` are not
        // enumerated here as a second ownership authority.
        let source_owned_label = merman_core::__private::config_paint_overrides_typed_default(
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
                theme
                    .series_color(ThemeTarget::Task, palette_ordinal)
                    .map(|color| KanbanTaskPaletteFill {
                        css: color.as_css(),
                        capability: if color.is_transparent() {
                            ThemeCapability::TransparentPaint
                        } else {
                            ThemeCapability::SolidPaint
                        },
                    })
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
                    if matches!((target, facet),
                        (ThemeTarget::Task, FamilyThemeRuleFacet::Fill(_)) if source_owned_card_fill)
                        || matches!((target, facet),
                            (ThemeTarget::Task, FamilyThemeRuleFacet::Stroke(_)) if source_owned_card_stroke)
                        || matches!((target, facet),
                            (ThemeTarget::TaskLabel, FamilyThemeRuleFacet::Fill(_)) if source_owned_label)
                    {
                        continue;
                    }
                    match (target, route.disposition(), facet) {
                        (
                            ThemeTarget::TaskLabel,
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeRuleFacet::Fill(
                                crate::diagram_theme::FamilyThemePaintKind::Transparent
                                | crate::diagram_theme::FamilyThemePaintKind::Solid,
                            ),
                        ) => {
                            if label_rule_visible.contains(&rule_index) {
                                observation.label_pending = true;
                            } else if label_rule_shadowed.contains(&rule_index) {
                                continue;
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
                        ) => match typed_fill_rule_occurrences.get(&rule_index).copied() {
                            Some(count) if count == item_count => {
                                observation.fill_pending = true;
                            }
                            Some(_) | None => {
                                observation.residual =
                                    Some(FamilyThemeResidualReason::UnsupportedPaint);
                            }
                        },
                        (
                            ThemeTarget::Task,
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeRuleFacet::Stroke(
                                crate::diagram_theme::FamilyThemePaintKind::Transparent
                                | crate::diagram_theme::FamilyThemePaintKind::Solid,
                            ),
                        ) => match typed_stroke_rule_occurrences.get(&rule_index).copied() {
                            Some(count) if count == item_count => {
                                observation.stroke_pending = true;
                            }
                            Some(_) | None => {
                                observation.residual =
                                    Some(FamilyThemeResidualReason::UnsupportedPaint);
                            }
                        },
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
                        (_, FamilyThemeDisposition::TypedAdapter, _) => {
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
            &[
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::Task,
                    TerminalVariantDomain::uniform(item_count, ThemeVariant::Default),
                ),
                // Kanban emits column and card labels, but no independent diagram title.
                // Frontmatter and literal node labels do not create a Title occurrence.
                UnsupportedTerminalDomain::direct(
                    ThemeTarget::Title,
                    TerminalVariantDomain::uniform(0, ThemeVariant::Default),
                ),
            ],
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
            item_count,
            items: Some(items.into_boxed_slice()),
            typography,
            evidence,
            pending_radius_key,
            pending_fill_keys,
            pending_stroke_keys,
            palette_key,
            pending_label_keys,
            terminal_outcome: OnceLock::new(),
        })
    }

    pub(super) fn baseline(item_count: usize, typography: KanbanTypographyPlan) -> Self {
        Self {
            item_count,
            items: None,
            typography,
            evidence: FamilyThemeEvidence::default(),
            pending_radius_key: None,
            pending_fill_keys: BTreeSet::new(),
            pending_stroke_keys: BTreeSet::new(),
            palette_key: None,
            pending_label_keys: BTreeSet::new(),
            terminal_outcome: OnceLock::new(),
        }
    }

    pub(crate) const fn item_count(&self) -> usize {
        self.item_count
    }

    pub(crate) fn radius_px(&self, item_index: usize) -> Option<f64> {
        match self.items.as_deref() {
            Some(items) => items.get(item_index).map(|item| item.radius_px),
            None => (item_index < self.item_count).then_some(MERMAID_TASK_RADIUS_PX),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.typography.font_family_css()
    }

    #[cfg(test)]
    pub(crate) fn typography_font_size_px(&self) -> f64 {
        self.typography.font_size_px()
    }

    pub(crate) fn resolved_typography_for_css(&self) -> Option<(&str, &str)> {
        self.typography
            .font_size_css
            .as_deref()
            .map(|size| (self.font_family_css(), size))
    }

    pub(crate) fn owns_label_foreground(&self, item_index: usize) -> bool {
        self.items
            .as_deref()
            .and_then(|items| items.get(item_index))
            .is_some_and(|item| item.label_foreground.is_some())
    }

    pub(super) fn bind_terminal_paints(&mut self, dark_mode: bool) -> crate::Result<()> {
        let Some(items) = self.items.as_deref_mut() else {
            return Ok(());
        };
        for item in items {
            if item.typed_fill.is_none()
                && let Some(fill) = item.palette_fill.as_mut()
            {
                fill.css = if dark_mode {
                    darken(&fill.css, 10.0)?
                } else {
                    lighten(&fill.css, 10.0)?
                };
            }
        }
        Ok(())
    }

    pub(crate) fn terminal_decision(&self, item_index: usize) -> KanbanTaskTerminalDecision<'_> {
        let Some(items) = self.items.as_deref() else {
            return KanbanTaskTerminalDecision::default();
        };
        let item = &items[item_index];
        let (fill_css, fill_capability, fill_rule_index) =
            if let Some(fill) = item.typed_fill.as_ref() {
                (
                    Some(fill.css.as_str()),
                    Some(fill.capability),
                    Some(fill.rule_index),
                )
            } else {
                let fill = item.palette_fill.as_ref();
                (
                    fill.map(|fill| fill.css.as_str()),
                    fill.map(|fill| fill.capability),
                    None,
                )
            };
        let stroke = item.typed_stroke.as_ref();
        let label = item.label_foreground.as_ref();
        KanbanTaskTerminalDecision {
            fill_css,
            fill_capability,
            fill_rule_index,
            stroke_css: stroke.map(|stroke| stroke.css.as_str()),
            stroke_capability: stroke.map(|stroke| stroke.capability),
            stroke_rule_index: stroke.map(|stroke| stroke.rule_index),
            label_css: label.map(|label| label.css.as_str()),
            label_capability: label.map(|label| label.capability),
            label_rule_index: label.map(|label| label.rule_index),
        }
    }

    pub(crate) fn begin_terminal_outcome(&self) -> Option<KanbanTaskThemeOutcome> {
        self.items.as_ref()?;
        (self.pending_radius_key.is_some()
            || !self.pending_fill_keys.is_empty()
            || !self.pending_stroke_keys.is_empty()
            || self.palette_key.is_some()
            || !self.pending_label_keys.is_empty()
            || self.typography.typography_requested())
        .then(|| KanbanTaskThemeOutcome {
            typography_requested: self.typography.typography_requested(),
            ..KanbanTaskThemeOutcome::default()
        })
    }

    pub(crate) fn record_terminal(&self, outcome: KanbanTaskThemeOutcome) {
        let _ = self.terminal_outcome.set(outcome);
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        self.typography
            .inherited_font_stack
            .mark_unsupported_typography_evidence(
                &mut evidence,
                self.terminal_outcome.get().is_none_or(
                    KanbanTaskThemeOutcome::typography_has_visible_or_unverified_terminals,
                ),
            );
        if let Some(outcome) = self.terminal_outcome.get() {
            if let Some(key) = self.pending_radius_key.clone() {
                if outcome.radius_written {
                    evidence
                        .mark_applied_with_capabilities(key, [ThemeCapability::RoundedGeometry]);
                } else {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedGeometry);
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
            for key in &self.pending_fill_keys {
                let FamilyThemeMechanismKey::Rule { index, .. } = key else {
                    unreachable!("Kanban task fill evidence must be rule-backed")
                };
                if outcome.fill_applied(*index) {
                    evidence.mark_applied_with_capabilities(
                        key.clone(),
                        outcome.typed_fill_capabilities[index].iter().copied(),
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
                if outcome.stroke_applied(*index) {
                    evidence.mark_applied_with_capabilities(
                        key.clone(),
                        outcome.typed_stroke_capabilities[index].iter().copied(),
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
                if outcome.label_applied(*index) {
                    evidence.mark_applied_with_capabilities(
                        key.clone(),
                        outcome.label_capabilities[index].iter().copied(),
                    );
                } else {
                    evidence
                        .mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                }
            }
        } else {
            if let Some(key) = self.pending_radius_key.clone() {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedGeometry);
            }
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

        if self
            .typography
            .inherited_font_stack
            .typed_font_stack_requested()
        {
            let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
            match self.typography.inherited_font_stack.outcome() {
                InheritedFontStackOutcome::Typed => match self.terminal_outcome.get() {
                    Some(outcome) if outcome.typography_is_not_applicable() => {
                        evidence.mark_not_applicable(key)
                    }
                    Some(outcome) if outcome.font_family_applied() => {
                        evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography])
                    }
                    Some(_) | None => evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography),
                },
                InheritedFontStackOutcome::ConfigOwned => evidence.mark_not_applicable(key),
                InheritedFontStackOutcome::Unsupported => {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography)
                }
                InheritedFontStackOutcome::Inactive => {}
            }
        }

        if self.typography.typed_font_size_requested {
            let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontSize);
            match self.typography.font_size_outcome {
                KanbanFontSizeOutcome::Typed => match self.terminal_outcome.get() {
                    Some(outcome) if outcome.typography_is_not_applicable() => {
                        evidence.mark_not_applicable(key)
                    }
                    Some(outcome) if outcome.font_size_applied() => {
                        evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography])
                    }
                    Some(_) | None => evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography),
                },
                KanbanFontSizeOutcome::ConfigOwned => evidence.mark_not_applicable(key),
                KanbanFontSizeOutcome::UnverifiedConfig => match self.terminal_outcome.get() {
                    Some(outcome) if outcome.typography_is_not_applicable() => {
                        evidence.mark_not_applicable(key)
                    }
                    Some(_) | None => evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography),
                },
                KanbanFontSizeOutcome::Inactive => {}
            }
        }
        evidence
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct KanbanTaskTerminalDecision<'a> {
    fill_css: Option<&'a str>,
    fill_capability: Option<ThemeCapability>,
    fill_rule_index: Option<usize>,
    stroke_css: Option<&'a str>,
    stroke_capability: Option<ThemeCapability>,
    stroke_rule_index: Option<usize>,
    label_css: Option<&'a str>,
    label_capability: Option<ThemeCapability>,
    label_rule_index: Option<usize>,
}

impl KanbanTaskTerminalDecision<'_> {
    pub(crate) fn fill_css(&self) -> Option<&str> {
        self.fill_css
    }

    pub(crate) fn label_css(&self) -> Option<&str> {
        self.label_css
    }

    pub(crate) fn stroke_css(&self) -> Option<&str> {
        self.stroke_css
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

/// Actual capabilities and inherited text ownership observed by the Kanban writers.
#[derive(Debug, Default)]
pub(crate) struct KanbanTaskThemeOutcome {
    radius_written: bool,
    typography_requested: bool,
    typography: KanbanTypographyFacts,
    typography_css_written: bool,
    palette_capabilities: BTreeSet<ThemeCapability>,
    typed_fill_capabilities: BTreeMap<usize, BTreeSet<ThemeCapability>>,
    typed_stroke_capabilities: BTreeMap<usize, BTreeSet<ThemeCapability>>,
    label_capabilities: BTreeMap<usize, BTreeSet<ThemeCapability>>,
    unverified_label_rules: BTreeSet<usize>,
}

impl KanbanTaskThemeOutcome {
    pub(crate) fn record_typography_css(&mut self) {
        self.typography_css_written = true;
    }

    pub(crate) fn record_typography_text(&mut self, facts: KanbanTypographyTerminalFacts) {
        if self.typography_requested {
            self.typography.record_terminal(facts);
        }
    }

    pub(crate) fn record_item(&mut self, decision: &KanbanTaskTerminalDecision<'_>) {
        self.radius_written = true;
        if let Some(capability) = decision.fill_capability {
            if let Some(rule_index) = decision.fill_rule_index {
                self.typed_fill_capabilities
                    .entry(rule_index)
                    .or_default()
                    .insert(capability);
            } else {
                self.palette_capabilities.insert(capability);
            }
        }
        if let (Some(capability), Some(rule_index)) =
            (decision.stroke_capability, decision.stroke_rule_index)
        {
            self.typed_stroke_capabilities
                .entry(rule_index)
                .or_default()
                .insert(capability);
        }
    }

    pub(crate) fn record_label(
        &mut self,
        visible_run_count: usize,
        inherited_color_run_count: usize,
        decision: &KanbanTaskTerminalDecision<'_>,
    ) {
        if let (Some(capability), Some(rule_index)) =
            (decision.label_capability, decision.label_rule_index)
        {
            if inherited_color_run_count != visible_run_count {
                self.unverified_label_rules.insert(rule_index);
            }
            self.label_capabilities
                .entry(rule_index)
                .or_default()
                .insert(capability);
        }
    }

    fn label_applied(&self, rule_index: usize) -> bool {
        !self.unverified_label_rules.contains(&rule_index)
            && self.label_capabilities.contains_key(&rule_index)
    }

    fn fill_applied(&self, rule_index: usize) -> bool {
        self.typed_fill_capabilities.contains_key(&rule_index)
    }

    fn stroke_applied(&self, rule_index: usize) -> bool {
        self.typed_stroke_capabilities.contains_key(&rule_index)
    }

    fn typography_is_not_applicable(&self) -> bool {
        self.typography.parse_valid && self.typography.visible_run_count == 0
    }

    fn typography_has_visible_or_unverified_terminals(&self) -> bool {
        !self.typography_is_not_applicable()
    }

    fn font_family_applied(&self) -> bool {
        self.typography_css_written
            && self.typography.parse_valid
            && self.typography.inherited_font_family_run_count == self.typography.visible_run_count
            && self.typography.unverified_font_family_run_count == 0
    }

    fn font_size_applied(&self) -> bool {
        self.typography_css_written
            && self.typography.parse_valid
            && self.typography.inherited_font_size_run_count == self.typography.visible_run_count
            && self.typography.unverified_font_size_run_count == 0
    }
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
}

#[cfg(test)]
mod tests {
    use super::{
        KanbanTaskOccurrence, KanbanTaskTheme, KanbanTypographyPlan, MERMAID_TASK_RADIUS_PX,
    };
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramEffectSet, DiagramThemeCompiler, DiagramThemeSpec, EffectBinding, EffectGraph,
        EffectInput, EffectPrimitive, FamilyThemeMechanismKey, ThemeRule, ThemeRuleSet,
        ThemeStylePatch, ThemeTarget,
    };
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
    use merman_core::MermaidConfig;

    #[test]
    fn baseline_keeps_only_item_count_and_skips_terminal_evidence() {
        let config = MermaidConfig::default();
        for item_count in [0, 1, 4096] {
            let theme =
                KanbanTaskTheme::baseline(item_count, KanbanTypographyPlan::resolve(None, &config));
            assert_eq!(theme.item_count(), item_count);
            assert!(theme.items.is_none());
            assert_eq!(theme.radius_px(item_count), None);
            assert_eq!(theme.radius_px(usize::MAX), None);
            if item_count > 0 {
                assert_eq!(theme.radius_px(0), Some(MERMAID_TASK_RADIUS_PX));
                assert_eq!(
                    theme.radius_px(item_count - 1),
                    Some(MERMAID_TASK_RADIUS_PX)
                );
            }
            assert!(theme.begin_terminal_outcome().is_none());
        }
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

        let config = MermaidConfig::default();
        let task_theme = KanbanTaskTheme::resolve_with_typography(
            &theme,
            vec![KanbanTaskOccurrence::new(false, false)],
            &config,
            KanbanTypographyPlan::resolve(Some(&theme), &config),
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

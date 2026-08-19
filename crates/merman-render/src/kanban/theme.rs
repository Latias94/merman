use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;
use merman_core::theme_color::{darken, lighten};

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
const MERMAID_TASK_PALETTE_SLOT_COUNT: usize = 12;

/// Kanban task geometry and evidence resolved once for concrete item occurrences.
#[derive(Debug)]
pub(crate) struct KanbanTaskTheme {
    item_radii: Vec<f64>,
    item_surfaces: Vec<Option<KanbanTaskSurface>>,
    occurrences: Box<[KanbanTaskOccurrence]>,
    evidence: FamilyThemeEvidence,
    pending_radius_key: Option<FamilyThemeMechanismKey>,
    palette_key: Option<FamilyThemeMechanismKey>,
    pending_label_keys: BTreeSet<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<KanbanTaskThemeReceipt>,
}

#[derive(Debug)]
struct KanbanTaskSurface {
    fill: KanbanTaskPaletteFill,
    label: Option<KanbanTaskLabelForeground>,
}

#[derive(Debug)]
struct KanbanTaskPaletteFill {
    css: String,
    capability: ThemeCapability,
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
}

impl KanbanTaskOccurrence {
    pub(crate) fn new(
        semantic_id: impl Into<Box<str>>,
        visible_labels: impl IntoIterator<Item = KanbanTaskLabelRole>,
    ) -> Self {
        Self {
            semantic_id: semantic_id.into(),
            visible_labels: visible_labels.into_iter().collect(),
        }
    }

    fn has_visible_labels(&self) -> bool {
        !self.visible_labels.is_empty()
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
        let source_owned_palette = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.background",
        );
        // The generated family bridge is a fallback overlay and does not claim typed-default
        // ownership. Explicit site/source or authored Mermaid compatibility still owns the whole
        // visible task surface and suppresses the paired direct writer.
        let source_owned_label = [
            "themeVariables.textColor",
            "themeVariables.primaryTextColor",
        ]
        .into_iter()
        .any(|path| {
            merman_core::__private::config_path_overrides_typed_default(effective_config, path)
        });
        let mut item_radii = Vec::with_capacity(item_count);
        let mut item_surfaces = Vec::with_capacity(item_count);
        let mut label_rule_visible = BTreeSet::new();
        let mut unpaired_palette = false;
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
            let palette_fill = if !source_owned_palette
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
            let has_visible_labels = occurrences[item_index].has_visible_labels();
            if has_visible_labels {
                winner_properties.extend(
                    label_style
                        .winner_rule_properties()
                        .into_iter()
                        .map(|(property, origin)| (origin.rule_index(), property)),
                );
            }
            let label_foreground = if has_visible_labels && !source_owned_label {
                typed_label_foreground(theme, &label_style)
            } else {
                None
            };
            if let Some(label) = label_foreground.as_ref() {
                label_rule_visible.insert(label.rule_index);
            }

            let surface = match (palette_fill, label_foreground, has_visible_labels) {
                (Some(fill), Some(label), true) => Some(KanbanTaskSurface {
                    fill,
                    label: Some(label),
                }),
                (Some(fill), None, false) => Some(KanbanTaskSurface { fill, label: None }),
                (None, None, false) => None,
                (Some(_), None, true) | (None, Some(_), true) => {
                    unpaired_palette = true;
                    None
                }
                (None, None, true) => None,
                (None, Some(_), false) | (Some(_), Some(_), false) => {
                    unreachable!("invisible Kanban labels cannot resolve a typed foreground")
                }
            };
            item_surfaces.push(surface);
        }
        if unpaired_palette {
            item_surfaces.iter_mut().for_each(|surface| *surface = None);
        }

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<(usize, ThemeTarget), KanbanTaskRuleObservation>::new();
        let mut palette_key = None;
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
                            if source_owned_label || source_owned_palette {
                                observation.suppressed = true;
                            } else if !unpaired_palette && label_rule_visible.contains(&rule_index)
                            {
                                observation.label_pending = true;
                            } else {
                                observation.residual =
                                    Some(FamilyThemeResidualReason::UnsupportedPaint);
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
                            FamilyThemeDisposition::TypedAdapter
                                if source_owned_palette || source_owned_label =>
                            {
                                evidence.mark_not_applicable(key);
                            }
                            FamilyThemeDisposition::TypedAdapter if unpaired_palette => {
                                evidence.mark_residual(
                                    key,
                                    FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                                );
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
            } else if observation.radius_pending {
                debug_assert!(pending_radius_key.is_none());
                pending_radius_key = Some(key);
            } else if observation.label_pending {
                pending_label_keys.insert(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            item_radii,
            item_surfaces,
            occurrences: occurrences.into_boxed_slice(),
            evidence,
            pending_radius_key,
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
        let item_count = occurrences.len();
        Self {
            item_radii: vec![MERMAID_TASK_RADIUS_PX; item_count],
            item_surfaces: (0..item_count).map(|_| None).collect(),
            occurrences: occurrences.into_boxed_slice(),
            evidence: FamilyThemeEvidence::default(),
            pending_radius_key: None,
            palette_key: None,
            pending_label_keys: BTreeSet::new(),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn radius_px(&self, item_index: usize) -> Option<f64> {
        self.item_radii.get(item_index).copied()
    }

    pub(crate) fn palette_terminal_decisions(
        &self,
        config: &MermaidConfig,
    ) -> crate::Result<Vec<KanbanTaskPaletteTerminalDecision>> {
        if self.palette_key.is_none() {
            return Ok(vec![
                KanbanTaskPaletteTerminalDecision::NotApplicable;
                self.item_surfaces.len()
            ]);
        }
        let dark_mode = config
            .get_bool("darkMode")
            .or_else(|| config.get_bool("themeVariables.darkMode"))
            .unwrap_or(false);
        (0..self.item_surfaces.len())
            .map(|item_index| self.palette_terminal_decision(item_index, dark_mode))
            .collect()
    }

    fn palette_terminal_decision(
        &self,
        item_index: usize,
        dark_mode: bool,
    ) -> crate::Result<KanbanTaskPaletteTerminalDecision> {
        let Some(surface) = self.item_surfaces.get(item_index).and_then(Option::as_ref) else {
            return Ok(KanbanTaskPaletteTerminalDecision::NotApplicable);
        };
        let css = if dark_mode {
            darken(&surface.fill.css, 10.0)?
        } else {
            lighten(&surface.fill.css, 10.0)?
        };
        Ok(KanbanTaskPaletteTerminalDecision::Applied {
            fill_css: css,
            fill_capability: surface.fill.capability,
            label_css: surface.label.as_ref().map(|label| label.css.clone()),
            label_capability: surface.label.as_ref().map(|label| label.capability),
            label_rule_index: surface.label.as_ref().map(|label| label.rule_index),
        })
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        decisions: &[KanbanTaskPaletteTerminalDecision],
    ) -> Option<KanbanTaskThemeReceipt> {
        (self.pending_radius_key.is_some()
            || self.palette_key.is_some()
            || !self.pending_label_keys.is_empty())
        .then(|| KanbanTaskThemeReceipt::new(&self.occurrences, decisions))
    }

    pub(crate) fn record_terminal(&self, receipt: KanbanTaskThemeReceipt) -> bool {
        (self.pending_radius_key.is_some()
            || self.palette_key.is_some()
            || !self.pending_label_keys.is_empty())
            && receipt.proves(self.item_radii.len())
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
pub(crate) enum KanbanTaskPaletteTerminalDecision {
    Applied {
        fill_css: String,
        fill_capability: ThemeCapability,
        label_css: Option<String>,
        label_capability: Option<ThemeCapability>,
        label_rule_index: Option<usize>,
    },
    NotApplicable,
}

impl KanbanTaskPaletteTerminalDecision {
    pub(crate) fn fill_css(&self) -> Option<&str> {
        match self {
            Self::Applied { fill_css, .. } => Some(fill_css),
            Self::NotApplicable => None,
        }
    }

    pub(crate) fn label_css(&self) -> Option<&str> {
        match self {
            Self::Applied { label_css, .. } => label_css.as_deref(),
            Self::NotApplicable => None,
        }
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
    label_capabilities: BTreeMap<usize, BTreeSet<ThemeCapability>>,
}

#[derive(Debug)]
struct KanbanTaskReceiptItem {
    semantic_id: Box<str>,
    rect_checkpointed: bool,
    expected_labels: KanbanTaskLabelRoles,
    emitted_labels: KanbanTaskLabelRoles,
}

impl KanbanTaskThemeReceipt {
    fn new(
        occurrences: &[KanbanTaskOccurrence],
        decisions: &[KanbanTaskPaletteTerminalDecision],
    ) -> Self {
        let schema_valid = occurrences.len() == decisions.len();
        Self {
            items: occurrences
                .iter()
                .zip(decisions)
                .map(|(occurrence, decision)| KanbanTaskReceiptItem {
                    semantic_id: occurrence.semantic_id.clone(),
                    rect_checkpointed: false,
                    expected_labels: decision
                        .label_css()
                        .map(|_| occurrence.visible_labels)
                        .unwrap_or_default(),
                    emitted_labels: KanbanTaskLabelRoles::default(),
                })
                .collect(),
            schema_valid,
            palette_capabilities: BTreeSet::new(),
            label_capabilities: BTreeMap::new(),
        }
    }

    pub(crate) fn record_checkpointed_item(
        &mut self,
        item_index: usize,
        semantic_id: &str,
        attributes_match: bool,
        palette_decision: &KanbanTaskPaletteTerminalDecision,
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
        if let KanbanTaskPaletteTerminalDecision::Applied {
            fill_capability, ..
        } = palette_decision
        {
            self.palette_capabilities.insert(*fill_capability);
        }
    }

    pub(crate) fn record_label(
        &mut self,
        item_index: usize,
        semantic_id: &str,
        role: KanbanTaskLabelRole,
        actual_group_style: &str,
        actual_div_style: &str,
        decision: &KanbanTaskPaletteTerminalDecision,
    ) {
        let Some(item) = self.items.get_mut(item_index) else {
            self.schema_valid = false;
            return;
        };
        let KanbanTaskPaletteTerminalDecision::Applied {
            label_css: Some(expected_css),
            label_capability: Some(capability),
            label_rule_index: Some(rule_index),
            ..
        } = decision
        else {
            self.schema_valid = false;
            return;
        };
        self.schema_valid &= item.semantic_id.as_ref() == semantic_id;
        self.schema_valid &= item.expected_labels.contains(role);
        self.schema_valid &= item.emitted_labels.insert(role);
        self.schema_valid &=
            terminal_style_property(actual_group_style, "color") == Some(expected_css.as_str());
        self.schema_valid &=
            terminal_style_property(actual_group_style, "fill") == Some(expected_css.as_str());
        self.schema_valid &=
            terminal_style_property(actual_div_style, "color") == Some(expected_css.as_str());
        self.label_capabilities
            .entry(*rule_index)
            .or_default()
            .insert(*capability);
    }

    fn proves_label_rule(&self, rule_index: usize) -> bool {
        self.schema_valid
            && self
                .label_capabilities
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
    label_pending: bool,
    suppressed: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        KanbanTaskLabelRole, KanbanTaskOccurrence, KanbanTaskPaletteTerminalDecision,
        KanbanTaskThemeReceipt,
    };
    use crate::diagram_theme::ThemeCapability;

    fn label_decision() -> KanbanTaskPaletteTerminalDecision {
        KanbanTaskPaletteTerminalDecision::Applied {
            fill_css: "#ffffff".to_string(),
            fill_capability: ThemeCapability::SolidPaint,
            label_css: Some("#000000".to_string()),
            label_capability: Some(ThemeCapability::SolidPaint),
            label_rule_index: Some(7),
        }
    }

    #[test]
    fn task_theme_receipt_requires_each_terminal_occurrence_once() {
        let occurrence = KanbanTaskOccurrence::new(
            "task",
            [KanbanTaskLabelRole::Title, KanbanTaskLabelRole::Ticket],
        );
        let decision = label_decision();

        let mut incomplete = KanbanTaskThemeReceipt::new(
            std::slice::from_ref(&occurrence),
            std::slice::from_ref(&decision),
        );
        incomplete.record_checkpointed_item(0, "task", true, &decision);
        incomplete.record_label(
            0,
            "task",
            KanbanTaskLabelRole::Title,
            "color:#000000;fill:#000000;text-align:left !important",
            "color: #000000; text-align:center",
            &decision,
        );
        assert!(!incomplete.proves(1));

        let mut mismatched = KanbanTaskThemeReceipt::new(
            std::slice::from_ref(&occurrence),
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
                &decision,
            );
        }
        assert!(!mismatched.proves(1));

        let mut complete =
            KanbanTaskThemeReceipt::new(&[occurrence], std::slice::from_ref(&decision));
        complete.record_checkpointed_item(0, "task", true, &decision);
        for role in [KanbanTaskLabelRole::Title, KanbanTaskLabelRole::Ticket] {
            complete.record_label(
                0,
                "task",
                role,
                "color:#000000;fill:#000000;text-align:left !important",
                "color: #000000; text-align:center",
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
            &decision,
        );
        assert!(!complete.proves(1));
    }
}

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
    item_palette_fills: Vec<Option<KanbanTaskPaletteFill>>,
    evidence: FamilyThemeEvidence,
    pending_radius_key: Option<FamilyThemeMechanismKey>,
    palette_key: Option<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<KanbanTaskThemeReceipt>,
}

#[derive(Debug)]
struct KanbanTaskPaletteFill {
    css: String,
    capability: ThemeCapability,
}

impl KanbanTaskTheme {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        item_count: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::baseline(item_count));
        };

        let palette_disposition = theme.ordinal_palette_disposition(ThemeTarget::Task);
        let mut item_radii = Vec::with_capacity(item_count);
        let mut item_palette_fills = Vec::with_capacity(item_count);
        let mut missing_palette_color = false;
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
            let palette_fill = if palette_disposition == Some(FamilyThemeDisposition::TypedAdapter)
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
                    None => {
                        missing_palette_color = true;
                        None
                    }
                }
            } else {
                None
            };
            item_palette_fills.push(palette_fill);
        }

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<usize, KanbanTaskRuleObservation>::new();
        let mut palette_key = None;
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
                            observation.radius_pending = true;
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
                    if item_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else {
                        match route.disposition() {
                            FamilyThemeDisposition::TypedAdapter if missing_palette_color => {
                                evidence.mark_residual(
                                    key,
                                    FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                                );
                                item_palette_fills.iter_mut().for_each(|fill| *fill = None);
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
                // A mixed rule cannot be signed Applied until every winning facet is accounted.
            } else if observation.radius_pending {
                debug_assert!(pending_radius_key.is_none());
                pending_radius_key = Some(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            item_radii,
            item_palette_fills,
            evidence,
            pending_radius_key,
            palette_key,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline(item_count: usize) -> Self {
        Self {
            item_radii: vec![MERMAID_TASK_RADIUS_PX; item_count],
            item_palette_fills: (0..item_count).map(|_| None).collect(),
            evidence: FamilyThemeEvidence::default(),
            pending_radius_key: None,
            palette_key: None,
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
                self.item_palette_fills.len()
            ]);
        }
        // `cScaleN` and `gitN` own section and root surfaces, not task cards. Only an explicit or
        // surviving compatibility `background` value can outrank the typed terminal rectangle.
        let mermaid_owns_fill = merman_core::__private::config_path_overrides_typed_default(
            config,
            "themeVariables.background",
        );
        let dark_mode = config
            .get_bool("darkMode")
            .or_else(|| config.get_bool("themeVariables.darkMode"))
            .unwrap_or(false);
        (0..self.item_palette_fills.len())
            .map(|item_index| {
                self.palette_terminal_decision(item_index, mermaid_owns_fill, dark_mode)
            })
            .collect()
    }

    fn palette_terminal_decision(
        &self,
        item_index: usize,
        mermaid_owns_fill: bool,
        dark_mode: bool,
    ) -> crate::Result<KanbanTaskPaletteTerminalDecision> {
        let Some(fill) = self
            .item_palette_fills
            .get(item_index)
            .and_then(Option::as_ref)
        else {
            return Ok(KanbanTaskPaletteTerminalDecision::NotApplicable);
        };
        if mermaid_owns_fill {
            return Ok(KanbanTaskPaletteTerminalDecision::NotApplicable);
        }
        let css = if dark_mode {
            darken(&fill.css, 10.0)?
        } else {
            lighten(&fill.css, 10.0)?
        };
        Ok(KanbanTaskPaletteTerminalDecision::Applied {
            css,
            capability: fill.capability,
        })
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<KanbanTaskThemeReceipt> {
        (self.pending_radius_key.is_some() || self.palette_key.is_some())
            .then(|| KanbanTaskThemeReceipt::new(self.item_radii.len()))
    }

    pub(crate) fn record_terminal(&self, receipt: KanbanTaskThemeReceipt) -> bool {
        (self.pending_radius_key.is_some() || self.palette_key.is_some())
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
        } else if let Some(key) = self.palette_key.clone() {
            evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette);
        }
        evidence
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KanbanTaskPaletteTerminalDecision {
    Applied {
        css: String,
        capability: ThemeCapability,
    },
    NotApplicable,
}

impl KanbanTaskPaletteTerminalDecision {
    pub(crate) fn fill_css(&self) -> Option<&str> {
        match self {
            Self::Applied { css, .. } => Some(css),
            Self::NotApplicable => None,
        }
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

/// Writer-owned proof that every Kanban item emitted its canonical SVG theme attributes.
#[derive(Debug)]
pub(crate) struct KanbanTaskThemeReceipt {
    checkpointed_items: Vec<bool>,
    attributes_match: bool,
    palette_capabilities: BTreeSet<ThemeCapability>,
}

impl KanbanTaskThemeReceipt {
    fn new(item_count: usize) -> Self {
        Self {
            checkpointed_items: vec![false; item_count],
            attributes_match: true,
            palette_capabilities: BTreeSet::new(),
        }
    }

    pub(crate) fn record_checkpointed_item(
        &mut self,
        item_index: usize,
        attributes_match: bool,
        palette_decision: &KanbanTaskPaletteTerminalDecision,
    ) {
        let Some(checkpointed) = self.checkpointed_items.get_mut(item_index) else {
            self.attributes_match = false;
            return;
        };
        if *checkpointed {
            self.attributes_match = false;
            return;
        }
        *checkpointed = true;
        self.attributes_match &= attributes_match;
        if let KanbanTaskPaletteTerminalDecision::Applied { capability, .. } = palette_decision {
            self.palette_capabilities.insert(*capability);
        }
    }

    fn proves(&self, expected_item_count: usize) -> bool {
        self.checkpointed_items.len() == expected_item_count
            && self.attributes_match
            && self
                .checkpointed_items
                .iter()
                .all(|checkpointed| *checkpointed)
    }
}

#[derive(Debug, Default)]
struct KanbanTaskRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    radius_pending: bool,
}

#[cfg(test)]
mod tests {
    use super::{KanbanTaskPaletteTerminalDecision, KanbanTaskThemeReceipt};

    #[test]
    fn task_theme_receipt_requires_each_terminal_item_once() {
        let not_applicable = KanbanTaskPaletteTerminalDecision::NotApplicable;
        let mut incomplete = KanbanTaskThemeReceipt::new(2);
        incomplete.record_checkpointed_item(0, true, &not_applicable);
        assert!(!incomplete.proves(2));

        let mut duplicate = KanbanTaskThemeReceipt::new(1);
        duplicate.record_checkpointed_item(0, true, &not_applicable);
        duplicate.record_checkpointed_item(0, true, &not_applicable);
        assert!(!duplicate.proves(1));

        let mut mismatch = KanbanTaskThemeReceipt::new(1);
        mismatch.record_checkpointed_item(0, false, &not_applicable);
        assert!(!mismatch.proves(1));
    }
}

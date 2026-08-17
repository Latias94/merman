use std::collections::BTreeSet;
use std::sync::OnceLock;

use merman_core::MermaidConfig;
use merman_core::diagrams::radar::RadarDiagramRenderModel;

use crate::config::value_at;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanismKey, ResolvedDiagramTheme, ThemeCapability,
    ThemeTarget, ThemeVariant,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason};
use crate::resources::{OperationWorkError, OperationWorkMeter};
use crate::theme::MermaidThemeAdapter;

const RADAR_PALETTE_SLOT_COUNT: usize = 12;

const RADAR_COLOR_SCALE_PATHS: [&str; RADAR_PALETTE_SLOT_COUNT] = [
    "themeVariables.cScale0",
    "themeVariables.cScale1",
    "themeVariables.cScale2",
    "themeVariables.cScale3",
    "themeVariables.cScale4",
    "themeVariables.cScale5",
    "themeVariables.cScale6",
    "themeVariables.cScale7",
    "themeVariables.cScale8",
    "themeVariables.cScale9",
    "themeVariables.cScale10",
    "themeVariables.cScale11",
];

/// Resolves Radar curve and legend paint once for terminal CSS and family evidence.
#[derive(Debug)]
pub(crate) struct RadarSeriesPaintPlan {
    colors: [String; RADAR_PALETTE_SLOT_COUNT],
    typed_capabilities: [Option<ThemeCapability>; RADAR_PALETTE_SLOT_COUNT],
    curve_count: usize,
    show_legend: bool,
    evidence: FamilyThemeEvidence,
    palette_key: Option<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<()>,
}

impl RadarSeriesPaintPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        model: &RadarDiagramRenderModel,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self::baseline(effective_config, model);
        let Some(theme) = theme else {
            return Ok(plan);
        };

        plan.evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let Some(disposition) = theme.ordinal_palette_disposition(ThemeTarget::ChartSeries) else {
            return Ok(plan);
        };
        let key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::ChartSeries,
        };
        if model.curves.is_empty() {
            plan.evidence.mark_not_applicable(key);
            return Ok(plan);
        }

        match disposition {
            FamilyThemeDisposition::TypedAdapter => {
                if model.curves.len() > RADAR_PALETTE_SLOT_COUNT
                    || !uses_fixed_mermaid_palette_surface(effective_config)
                {
                    plan.evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette);
                    return Ok(plan);
                }

                for curve_index in 0..model.curves.len() {
                    let style = theme.style_with_work_meter(
                        ThemeTarget::ChartSeries,
                        ThemeVariant::Default,
                        Some(curve_index + 1),
                        work_meter,
                    )?;
                    if style.fill_resolution().winner().is_some()
                        || style.stroke_resolution().winner().is_some()
                        || merman_core::__private::config_path_overrides_typed_default(
                            effective_config,
                            RADAR_COLOR_SCALE_PATHS[curve_index],
                        )
                    {
                        continue;
                    }

                    let color = theme
                        .series_color(ThemeTarget::ChartSeries, curve_index + 1)
                        .expect("compiled ordinal palettes are non-empty and one-based");
                    plan.colors[curve_index] = color.as_css();
                    plan.typed_capabilities[curve_index] = Some(if color.is_transparent() {
                        ThemeCapability::TransparentPaint
                    } else {
                        ThemeCapability::SolidPaint
                    });
                }
                plan.palette_key = Some(key);
            }
            FamilyThemeDisposition::Unsupported => plan
                .evidence
                .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette),
            FamilyThemeDisposition::LegacyCompatibility => {}
        }

        Ok(plan)
    }

    pub(crate) fn baseline(
        effective_config: &MermaidConfig,
        model: &RadarDiagramRenderModel,
    ) -> Self {
        let colors = MermaidThemeAdapter::new(effective_config.as_value())
            .radar()
            .series_colors;
        debug_assert_eq!(colors.len(), RADAR_PALETTE_SLOT_COUNT);
        Self {
            colors: std::array::from_fn(|index| colors[index].clone()),
            typed_capabilities: [None; RADAR_PALETTE_SLOT_COUNT],
            curve_count: model.curves.len(),
            show_legend: model.options.show_legend,
            evidence: FamilyThemeEvidence::default(),
            palette_key: None,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn colors(&self) -> &[String] {
        &self.colors
    }

    pub(crate) const fn curve_count(&self) -> usize {
        self.curve_count
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<RadarSeriesPaintReceipt> {
        self.palette_key
            .as_ref()
            .map(|_| RadarSeriesPaintReceipt::new())
    }

    pub(crate) fn record_terminal(&self, receipt: RadarSeriesPaintReceipt) -> bool {
        self.palette_key.is_some() && receipt.proves(self) && self.terminal_receipt.set(()).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(key) = self.palette_key.clone() else {
            return evidence;
        };
        match self.terminal_receipt.get() {
            Some(()) => {
                let capabilities = self
                    .typed_capabilities
                    .iter()
                    .take(self.curve_count)
                    .flatten()
                    .copied()
                    .collect::<BTreeSet<_>>();
                if capabilities.is_empty() {
                    evidence.mark_not_applicable(key);
                } else {
                    evidence.mark_applied_with_capabilities(key, capabilities);
                }
            }
            None => {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette)
            }
        }
        evidence
    }
}

fn uses_fixed_mermaid_palette_surface(effective_config: &MermaidConfig) -> bool {
    value_at(
        effective_config.as_value(),
        &["themeVariables", "THEME_COLOR_LIMIT"],
    )
    .is_none_or(|value| value.as_f64() == Some(RADAR_PALETTE_SLOT_COUNT as f64))
}

/// Writer-owned proof that the complete visible Radar series surface reached terminal SVG.
#[derive(Debug)]
pub(crate) struct RadarSeriesPaintReceipt {
    next_rule_index: usize,
    next_curve_index: usize,
    next_legend_index: usize,
    values_match: bool,
}

impl RadarSeriesPaintReceipt {
    fn new() -> Self {
        Self {
            next_rule_index: 0,
            next_curve_index: 0,
            next_legend_index: 0,
            values_match: true,
        }
    }

    pub(crate) fn record_series_rule(
        &mut self,
        plan: &RadarSeriesPaintPlan,
        rule_index: usize,
        emitted_color: &str,
    ) {
        if rule_index != self.next_rule_index || rule_index >= plan.curve_count {
            self.values_match = false;
            return;
        }
        self.next_rule_index += 1;
        self.values_match &= plan
            .colors
            .get(rule_index)
            .is_some_and(|color| color == emitted_color);
    }

    pub(crate) fn record_curve(
        &mut self,
        plan: &RadarSeriesPaintPlan,
        curve_index: usize,
        emitted_class_index: i64,
    ) {
        if curve_index != self.next_curve_index || curve_index >= plan.curve_count {
            self.values_match = false;
            return;
        }
        self.next_curve_index += 1;
        self.values_match &= usize::try_from(emitted_class_index).ok() == Some(curve_index);
    }

    pub(crate) fn record_legend(
        &mut self,
        plan: &RadarSeriesPaintPlan,
        legend_index: usize,
        emitted_class_index: i64,
    ) {
        let expected_legend_count = if plan.show_legend {
            plan.curve_count
        } else {
            0
        };
        if legend_index != self.next_legend_index || legend_index >= expected_legend_count {
            self.values_match = false;
            return;
        }
        self.next_legend_index += 1;
        self.values_match &= usize::try_from(emitted_class_index).ok() == Some(legend_index);
    }

    fn proves(&self, plan: &RadarSeriesPaintPlan) -> bool {
        self.values_match
            && self.next_rule_index == plan.curve_count
            && self.next_curve_index == plan.curve_count
            && self.next_legend_index
                == if plan.show_legend {
                    plan.curve_count
                } else {
                    0
                }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_curve_plan() -> RadarSeriesPaintPlan {
        RadarSeriesPaintPlan {
            colors: std::array::from_fn(|_| "#123456".to_string()),
            typed_capabilities: [Some(ThemeCapability::SolidPaint); RADAR_PALETTE_SLOT_COUNT],
            curve_count: 1,
            show_legend: true,
            evidence: FamilyThemeEvidence::default(),
            palette_key: Some(FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::ChartSeries,
            }),
            terminal_receipt: OnceLock::new(),
        }
    }

    #[test]
    fn series_paint_receipt_rejects_missing_or_mismatched_terminal_checkpoints() {
        let plan = one_curve_plan();
        let mut incomplete = RadarSeriesPaintReceipt::new();
        incomplete.record_series_rule(&plan, 0, "#123456");
        incomplete.record_curve(&plan, 0, 0);
        assert!(!incomplete.proves(&plan));

        let mut mismatch = RadarSeriesPaintReceipt::new();
        mismatch.record_series_rule(&plan, 0, "#abcdef");
        mismatch.record_curve(&plan, 0, 0);
        mismatch.record_legend(&plan, 0, 0);
        assert!(!mismatch.proves(&plan));
    }
}

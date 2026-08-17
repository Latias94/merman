use std::collections::BTreeSet;
use std::sync::OnceLock;

use merman_core::MermaidConfig;
use merman_core::diagrams::xychart::{XyChartDiagramRenderModel, XyChartPlotType};

use crate::chart_palette::plot_color_from_palette;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanismKey, ResolvedDiagramTheme, ThemeCapability,
    ThemeTarget, ThemeVariant,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason};
use crate::resources::{OperationWorkError, OperationWorkMeter};
use crate::theme::MermaidThemeAdapter;

#[derive(Debug)]
struct XyChartSeriesPaint {
    plot_type: XyChartPlotType,
    fill_css: String,
    stroke_css: String,
    typed_fill_capability: Option<ThemeCapability>,
    typed_stroke_capability: Option<ThemeCapability>,
    expected_mark_count: usize,
    expected_point_label_count: usize,
}

/// Resolves XY Chart series paint once for layout, terminal SVG emission, and evidence.
#[derive(Debug)]
pub(crate) struct XyChartSeriesPaintPlan {
    paints: Vec<XyChartSeriesPaint>,
    evidence: FamilyThemeEvidence,
    palette_key: Option<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<()>,
}

impl XyChartSeriesPaintPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        model: &XyChartDiagramRenderModel,
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
        let visible_plot_count = plan
            .paints
            .iter()
            .filter(|paint| paint.expected_mark_count != 0)
            .count();
        if visible_plot_count == 0 {
            plan.evidence.mark_not_applicable(key);
            return Ok(plan);
        }

        match disposition {
            FamilyThemeDisposition::TypedAdapter => {
                if merman_core::__private::config_path_overrides_typed_default(
                    effective_config,
                    "themeVariables.xyChart.plotColorPalette",
                ) {
                    plan.evidence.mark_not_applicable(key);
                    return Ok(plan);
                }

                for (plot_index, paint) in plan.paints.iter_mut().enumerate() {
                    if paint.expected_mark_count == 0 {
                        continue;
                    }
                    let style = theme.style_with_work_meter(
                        ThemeTarget::ChartSeries,
                        ThemeVariant::Default,
                        Some(plot_index + 1),
                        work_meter,
                    )?;
                    let color = theme
                        .series_color(ThemeTarget::ChartSeries, plot_index + 1)
                        .expect("compiled ordinal palettes are non-empty and one-based");
                    let css = color.as_css();
                    let capability = if color.is_transparent() {
                        ThemeCapability::TransparentPaint
                    } else {
                        ThemeCapability::SolidPaint
                    };

                    // Explicit static or ordinal rule winners outrank the ordinal-palette fallback
                    // independently for the fill and stroke channels.
                    if style.fill_resolution().winner().is_none() {
                        paint.fill_css.clone_from(&css);
                        paint.typed_fill_capability = Some(capability);
                    }
                    if style.stroke_resolution().winner().is_none() {
                        paint.stroke_css = css;
                        paint.typed_stroke_capability = Some(capability);
                    }
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

    fn baseline(effective_config: &MermaidConfig, model: &XyChartDiagramRenderModel) -> Self {
        let palette = MermaidThemeAdapter::new(effective_config.as_value())
            .xychart()
            .plot_color_palette;
        let paints = model
            .plots
            .iter()
            .enumerate()
            .map(|(plot_index, plot)| {
                let css = plot_color_from_palette(&palette, plot_index);
                XyChartSeriesPaint {
                    plot_type: plot.plot_type,
                    fill_css: css.clone(),
                    stroke_css: css,
                    typed_fill_capability: None,
                    typed_stroke_capability: None,
                    expected_mark_count: match plot.plot_type {
                        XyChartPlotType::Bar => plot.data.len(),
                        XyChartPlotType::Line => usize::from(!plot.data.is_empty()),
                    },
                    expected_point_label_count: if plot.plot_type == XyChartPlotType::Line
                        && !plot.data.is_empty()
                    {
                        plot.point_labels
                            .iter()
                            .take(plot.data.len())
                            .filter(|label| !label.is_empty())
                            .count()
                    } else {
                        0
                    },
                }
            })
            .collect();
        Self {
            paints,
            evidence: FamilyThemeEvidence::default(),
            palette_key: None,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn plot_count(&self) -> usize {
        self.paints.len()
    }

    pub(crate) fn fill_css(&self, plot_index: usize) -> Option<&str> {
        self.paints
            .get(plot_index)
            .map(|paint| paint.fill_css.as_str())
    }

    pub(crate) fn stroke_css(&self, plot_index: usize) -> Option<&str> {
        self.paints
            .get(plot_index)
            .map(|paint| paint.stroke_css.as_str())
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<XyChartSeriesPaintReceipt> {
        self.palette_key
            .as_ref()
            .map(|_| XyChartSeriesPaintReceipt::new(&self.paints))
    }

    pub(crate) fn record_terminal(&self, receipt: XyChartSeriesPaintReceipt) -> bool {
        self.palette_key.is_some()
            && receipt.proves(&self.paints)
            && self.terminal_receipt.set(()).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(key) = self.palette_key.clone() else {
            return evidence;
        };
        match self.terminal_receipt.get() {
            Some(()) => {
                let capabilities = self.applied_capabilities();
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

    fn applied_capabilities(&self) -> BTreeSet<ThemeCapability> {
        let mut capabilities = BTreeSet::new();
        for paint in &self.paints {
            let fill_is_visible = match paint.plot_type {
                XyChartPlotType::Bar => paint.expected_mark_count != 0,
                XyChartPlotType::Line => paint.expected_point_label_count != 0,
            };
            if fill_is_visible {
                capabilities.extend(paint.typed_fill_capability);
            }
            if paint.expected_mark_count != 0 {
                capabilities.extend(paint.typed_stroke_capability);
            }
        }
        capabilities
    }
}

#[derive(Debug)]
struct XyChartPlotPaintReceipt {
    next_mark_index: usize,
    next_label_index: usize,
}

/// Writer-owned proof that every visible plot emitted its canonical paint attributes once.
#[derive(Debug)]
pub(crate) struct XyChartSeriesPaintReceipt {
    plots: Vec<XyChartPlotPaintReceipt>,
    attributes_match: bool,
}

impl XyChartSeriesPaintReceipt {
    fn new(paints: &[XyChartSeriesPaint]) -> Self {
        Self {
            plots: paints
                .iter()
                .map(|_| XyChartPlotPaintReceipt {
                    next_mark_index: 0,
                    next_label_index: 0,
                })
                .collect(),
            attributes_match: true,
        }
    }

    pub(crate) fn record_bar_mark(
        &mut self,
        plan: &XyChartSeriesPaintPlan,
        plot_index: usize,
        mark_index: usize,
        emitted_fill: Option<(&str, &str)>,
        emitted_stroke: Option<(&str, &str)>,
    ) {
        let Some((receipt, paint)) = self
            .plots
            .get_mut(plot_index)
            .zip(plan.paints.get(plot_index))
        else {
            self.attributes_match = false;
            return;
        };
        if paint.plot_type != XyChartPlotType::Bar || mark_index != receipt.next_mark_index {
            self.attributes_match = false;
            return;
        }
        receipt.next_mark_index = receipt.next_mark_index.saturating_add(1);
        self.attributes_match &= receipt.next_mark_index <= paint.expected_mark_count
            && attribute_matches(emitted_fill, "fill", &paint.fill_css)
            && attribute_matches(emitted_stroke, "stroke", &paint.stroke_css);
    }

    pub(crate) fn record_line_mark(
        &mut self,
        plan: &XyChartSeriesPaintPlan,
        plot_index: usize,
        mark_index: usize,
        emitted_stroke: Option<(&str, &str)>,
    ) {
        let Some((receipt, paint)) = self
            .plots
            .get_mut(plot_index)
            .zip(plan.paints.get(plot_index))
        else {
            self.attributes_match = false;
            return;
        };
        if paint.plot_type != XyChartPlotType::Line || mark_index != receipt.next_mark_index {
            self.attributes_match = false;
            return;
        }
        receipt.next_mark_index = receipt.next_mark_index.saturating_add(1);
        self.attributes_match &= receipt.next_mark_index <= paint.expected_mark_count
            && attribute_matches(emitted_stroke, "stroke", &paint.stroke_css);
    }

    pub(crate) fn record_line_label(
        &mut self,
        plan: &XyChartSeriesPaintPlan,
        plot_index: usize,
        label_index: usize,
        emitted_fill: Option<(&str, &str)>,
    ) {
        let Some((receipt, paint)) = self
            .plots
            .get_mut(plot_index)
            .zip(plan.paints.get(plot_index))
        else {
            self.attributes_match = false;
            return;
        };
        if paint.plot_type != XyChartPlotType::Line || label_index != receipt.next_label_index {
            self.attributes_match = false;
            return;
        }
        receipt.next_label_index = receipt.next_label_index.saturating_add(1);
        self.attributes_match &= receipt.next_label_index <= paint.expected_point_label_count
            && attribute_matches(emitted_fill, "fill", &paint.fill_css);
    }

    fn proves(&self, paints: &[XyChartSeriesPaint]) -> bool {
        self.plots.len() == paints.len()
            && self.attributes_match
            && self.plots.iter().zip(paints).all(|(receipt, paint)| {
                receipt.next_mark_index == paint.expected_mark_count
                    && receipt.next_label_index == paint.expected_point_label_count
            })
    }
}

fn attribute_matches(
    emitted: Option<(&str, &str)>,
    expected_name: &str,
    expected_value: &str,
) -> bool {
    emitted.is_some_and(|(name, value)| name == expected_name && value == expected_value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_bar_plan() -> XyChartSeriesPaintPlan {
        XyChartSeriesPaintPlan {
            paints: vec![XyChartSeriesPaint {
                plot_type: XyChartPlotType::Bar,
                fill_css: "#123456".to_string(),
                stroke_css: "#123456".to_string(),
                typed_fill_capability: Some(ThemeCapability::SolidPaint),
                typed_stroke_capability: Some(ThemeCapability::SolidPaint),
                expected_mark_count: 1,
                expected_point_label_count: 0,
            }],
            evidence: FamilyThemeEvidence::default(),
            palette_key: Some(FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::ChartSeries,
            }),
            terminal_receipt: OnceLock::new(),
        }
    }

    #[test]
    fn series_paint_receipt_rejects_a_missing_emitted_attribute() {
        let plan = one_bar_plan();
        let mut receipt = XyChartSeriesPaintReceipt::new(&plan.paints);

        receipt.record_bar_mark(&plan, 0, 0, None, Some(("stroke", "#123456")));

        assert!(!receipt.proves(&plan.paints));
    }
}

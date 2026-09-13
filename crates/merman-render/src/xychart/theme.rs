use std::collections::BTreeSet;
use std::sync::{Arc, OnceLock};

use merman_core::MermaidConfig;
use merman_core::diagrams::xychart::{XyChartDiagramRenderModel, XyChartPlotType};

use crate::chart_palette::plot_color_from_palette;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    ResolvedDiagramTheme, ThemeCapability, ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    InheritedFontStackOutcome, InheritedFontStackPlan, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};
use crate::theme::MermaidThemeAdapter;

/// Final XY Chart base typography shared by layout, stylesheet emission, and evidence.
#[derive(Debug)]
pub(crate) struct XyChartTypographyThemePlan {
    inherited_font_stack: InheritedFontStackPlan,
    evidence: FamilyThemeEvidence,
    terminal_receipt: OnceLock<XyChartTypographyTerminalSeal>,
}

#[derive(Debug, Clone, Copy)]
struct XyChartTypographyTerminalSeal {
    emitted_visible_text_count: usize,
    css_seen: bool,
    font_family_matches: bool,
}

/// Writer-owned proof for the final XY Chart font-family CSS and visible text stream.
#[derive(Debug)]
pub(crate) struct XyChartTypographyTerminalReceipt {
    expected_font_family: Box<str>,
    emitted_visible_text_count: usize,
    css_seen: bool,
    font_family_matches: bool,
}

impl XyChartTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        Self {
            inherited_font_stack: InheritedFontStackPlan::resolve_property_local(
                theme,
                effective_config,
            ),
            evidence: FamilyThemeEvidence::from_theme(theme),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<XyChartTypographyTerminalReceipt> {
        self.inherited_font_stack
            .typography_requested()
            .then(|| XyChartTypographyTerminalReceipt {
                expected_font_family: self.font_family_css().into(),
                emitted_visible_text_count: 0,
                css_seen: false,
                font_family_matches: true,
            })
    }

    pub(crate) fn record_terminal(&self, receipt: XyChartTypographyTerminalReceipt) -> bool {
        self.terminal_receipt
            .set(XyChartTypographyTerminalSeal {
                emitted_visible_text_count: receipt.emitted_visible_text_count,
                css_seen: receipt.css_seen,
                font_family_matches: receipt.font_family_matches,
            })
            .is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if !self.inherited_font_stack.typography_requested() {
            return evidence;
        }

        let Some(receipt) = self.terminal_receipt.get() else {
            self.inherited_font_stack
                .mark_unsupported_typography_evidence(&mut evidence, true);
            if self.inherited_font_stack.typed_font_stack_requested() {
                evidence.mark_residual(
                    FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack),
                    FamilyThemeResidualReason::UnsupportedTypography,
                );
            }
            return evidence;
        };

        let has_visible_text = receipt.emitted_visible_text_count != 0;
        self.inherited_font_stack
            .mark_unsupported_typography_evidence(&mut evidence, has_visible_text);
        if !has_visible_text {
            if self.inherited_font_stack.typed_font_stack_requested() {
                evidence.mark_not_applicable(FamilyThemeMechanismKey::Typography(
                    ThemeTypographyProperty::FontStack,
                ));
            }
            return evidence;
        }

        if self.inherited_font_stack.typed_font_stack_requested() {
            let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
            match self.inherited_font_stack.outcome() {
                InheritedFontStackOutcome::Typed
                    if self.inherited_font_stack.typed_font_stack_active()
                        && receipt.css_seen
                        && receipt.font_family_matches =>
                {
                    evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
                }
                InheritedFontStackOutcome::ConfigOwned => evidence.mark_not_applicable(key),
                InheritedFontStackOutcome::Typed | InheritedFontStackOutcome::Unsupported => {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography)
                }
                InheritedFontStackOutcome::Inactive => {}
            }
        }
        evidence
    }
}

impl XyChartTypographyTerminalReceipt {
    pub(crate) fn record_css(&mut self, font_family: &str) {
        if self.css_seen {
            self.font_family_matches = false;
            return;
        }
        self.css_seen = true;
        self.font_family_matches = font_family == self.expected_font_family.as_ref();
    }

    pub(crate) fn record_visible_text(&mut self) {
        self.emitted_visible_text_count = self.emitted_visible_text_count.saturating_add(1);
    }
}

/// Direct title paint with evidence deferred until the final SVG document is complete.
#[derive(Debug, Default)]
pub(crate) struct XyChartTitleThemePlan {
    terminal: Option<Arc<(Box<str>, Box<str>)>>,
    pending: Option<(FamilyThemeMechanismKey, ThemeCapability)>,
    evidence: FamilyThemeEvidence,
    terminal_receipt: OnceLock<()>,
}

impl XyChartTitleThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &MermaidConfig,
        layout: &mut crate::model::XyChartDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let Some(theme) = theme else {
            return Ok(Self::default());
        };
        let mut plan = Self {
            evidence: FamilyThemeEvidence::from_theme(Some(theme)),
            ..Self::default()
        };
        if !theme.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Title,
                    ..
                } | FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Title
                } | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Title,
                    ..
                }
            )
        }) {
            return Ok(plan);
        }
        let title = layout
            .drawables
            .iter_mut()
            .find_map(|drawable| match drawable {
                crate::model::XyChartDrawableElem::Text { group_texts, data }
                    if group_texts.len() == 1 && group_texts[0] == "chart-title" =>
                {
                    data.first_mut()
                }
                _ => None,
            });
        let count = usize::from(
            title
                .as_ref()
                .is_some_and(|title| !title.text.trim().is_empty()),
        );
        let config_owned = merman_core::__private::config_path_overrides_typed_default(
            config,
            "themeVariables.xyChart.titleColor",
        );
        // Text fallback and Title rules keep their original author order. Only Title is
        // direct here; generic Text still has other legacy terminals to migrate.
        let style = theme.text_style_with_work_meter(
            ThemeTarget::Title,
            ThemeVariant::Default,
            Some(1),
            work_meter,
        )?;
        let fill = (count != 0 && !config_owned)
            .then(|| {
                resolve_direct_static_fill(
                    theme,
                    &style,
                    &[ThemeTarget::Title],
                    DirectStaticSelectorDomain::Default,
                )
            })
            .flatten();
        let mut winning_rule = None;
        if let (Some(fill), Some(title)) = (fill, title) {
            let (css, index, capability) = fill.into_parts();
            title.fill = css.to_string();
            plan.terminal = Some(Arc::new((title.text.clone().into_boxed_str(), css)));
            winning_rule = Some((index, capability));
        }
        let mut observations =
            std::collections::BTreeMap::<usize, XyChartTitleRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            let FamilyThemeMechanism::RuleFacet {
                target: ThemeTarget::Title,
                rule_index,
                selector,
                facet,
            } = route.mechanism()
            else {
                continue;
            };
            let observation = observations.entry(rule_index).or_default();
            if !selector.ordinal_domain_intersects_occurrence_count(count)
                || !style.winner_rule_properties().any(|(property, origin)| {
                    property == resolved_style_property_for_facet(facet)
                        && origin.rule_index() == rule_index
                })
                || (config_owned && matches!(facet, FamilyThemeRuleFacet::Fill(_)))
            {
                continue;
            }
            observation.applicable = true;
            if route.disposition() == FamilyThemeDisposition::TypedAdapter
                && matches!(facet, FamilyThemeRuleFacet::Fill(_))
                && winning_rule.is_some_and(|(index, _)| index == rule_index)
            {
                observation.fill_pending = true;
            } else {
                observation
                    .residual
                    .get_or_insert(unsupported_residual_for_facet(facet));
            }
        }
        for (index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index,
                target: ThemeTarget::Title,
            };
            if !observation.applicable {
                plan.evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                plan.evidence.mark_residual(key, reason);
            } else if observation.fill_pending
                && let Some((_, capability)) = winning_rule
            {
                plan.pending = Some((key, capability));
            }
        }
        reconcile_unsupported_terminal_domains(
            theme,
            &mut plan.evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Title,
                TerminalVariantDomain::uniform(count, ThemeVariant::Default),
            )
            .with_source_owned_fill(&[config_owned])],
            work_meter,
        )?;
        Ok(plan)
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        escape_xml: impl Fn(&str) -> String,
    ) -> Option<XyChartTitleThemeReceipt> {
        self.terminal
            .as_ref()
            .map(|terminal| XyChartTitleThemeReceipt {
                owner: Arc::clone(terminal),
                expected_text: escape_xml(&terminal.0),
                expected_fill: escape_xml(&terminal.1),
                count: 0,
                matches: true,
            })
    }

    pub(crate) fn record_terminal(&self, receipt: XyChartTitleThemeReceipt) -> bool {
        self.terminal
            .as_ref()
            .is_some_and(|terminal| Arc::ptr_eq(terminal, &receipt.owner))
            && receipt.count == 1
            && receipt.matches
            && self.terminal_receipt.set(()).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if self.terminal_receipt.get().is_some()
            && let Some((key, capability)) = &self.pending
        {
            evidence.mark_applied_with_capabilities(key.clone(), [*capability]);
        }
        evidence
    }
}

#[derive(Default)]
struct XyChartTitleRuleObservation {
    applicable: bool,
    fill_pending: bool,
    residual: Option<FamilyThemeResidualReason>,
}

#[derive(Debug)]
pub(crate) struct XyChartTitleThemeReceipt {
    owner: Arc<(Box<str>, Box<str>)>,
    expected_text: String,
    expected_fill: String,
    count: usize,
    matches: bool,
}

impl XyChartTitleThemeReceipt {
    pub(crate) fn record_text(&mut self, text: Option<&str>, fill: Option<&str>) {
        self.count = self.count.saturating_add(1);
        self.matches &=
            text == Some(self.expected_text.as_str()) && fill == Some(self.expected_fill.as_str());
    }
}

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

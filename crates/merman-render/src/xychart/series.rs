use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use merman_core::MermaidConfig;
use merman_core::diagrams::xychart::{XyChartDiagramRenderModel, XyChartPlotType};

use crate::chart_palette::plot_color_from_palette;
use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeEffect,
    Specified, SvgShadowEffect, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

#[derive(Debug, Default)]
struct RuleObservation {
    capabilities: BTreeSet<ThemeCapability>,
    residual: Option<FamilyThemeResidualReason>,
}

#[derive(Debug)]
struct SeriesPaint {
    plot_type: XyChartPlotType,
    fill: String,
    stroke: String,
    point_label_fill: String,
    stroke_width: f64,
    opacity: Option<f32>,
    fill_opacity: Option<f32>,
    stroke_opacity: Option<f32>,
    effect: Option<Arc<SvgShadowEffect>>,
    uses_effect_binding: bool,
    rules: BTreeMap<usize, RuleObservation>,
    palette_capabilities: BTreeSet<ThemeCapability>,
    expected_marks: usize,
    expected_labels: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, ThemeColorValue, ThemeRuleSet,
    };

    #[test]
    fn authored_palette_owns_color_even_when_it_matches_the_fallback() {
        let model: XyChartDiagramRenderModel = serde_json::from_value(serde_json::json!({
            "xAxis": { "type": "band", "categories": ["A"] },
            "yAxis": { "type": "linear", "min": 0, "max": 10 },
            "plots": [{ "type": "bar", "values": [4], "data": [["A", 4]] }]
        }))
        .unwrap();
        let palette = OrdinalPalette::new([ThemeColorValue::parse("#123456").unwrap()]).unwrap();
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::ChartSeries, palette),
            ))
            .unwrap()
            .resolve(crate::DiagramFamilyId::XY_CHART);
        let meter = OperationWorkMeter::new(crate::resources::RenderResourcePolicy::interactive());
        let source = "xychart\n  x-axis [A]\n  y-axis 0 --> 10\n  bar [4]\n";
        let baseline_config = merman_core::Engine::new()
            .parse_metadata_sync(source)
            .unwrap()
            .effective_config;
        let fallback =
            super::super::css_binding::XyChartCssBinding::resolve(baseline_config.as_value())
                .plot_color_palette[0]
                .clone();
        let baseline =
            XyChartSeriesPaintPlan::resolve(Some(&theme), &baseline_config, &model, &meter)
                .unwrap();
        assert_eq!(baseline.fill_css(0), Some("#123456"));
        for token in [fallback.as_str(), "var(--series)", " , "] {
            let config = merman_core::Engine::new()
                .with_site_config(MermaidConfig::from_value(serde_json::json!({
                    "themeVariables": { "xyChart": { "plotColorPalette": token } }
                })))
                .parse_metadata_sync(source)
                .unwrap()
                .effective_config;
            assert!(merman_core::__private::config_path_overrides_typed_default(
                &config,
                "themeVariables.xyChart.plotColorPalette",
            ));
            let bound_css =
                super::super::css_binding::XyChartCssBinding::resolve(config.as_value());
            let plan =
                XyChartSeriesPaintPlan::resolve(Some(&theme), &config, &model, &meter).unwrap();
            let expected = bound_css.plot_color_palette[0].as_str();
            if token == fallback {
                assert_eq!(expected, fallback);
            } else if token == "var(--series)" {
                assert_eq!(expected, token);
            }
            assert_eq!(plan.fill_css(0), Some(expected));
            assert_eq!(plan.stroke_css(0), Some(expected));
        }
    }
}

/// Resolves geometry paint without allowing mark alpha or fill rules to recolor point labels.
#[derive(Debug)]
pub(crate) struct XyChartSeriesPaintPlan {
    css: super::css_binding::XyChartCssBinding,
    chart_config: super::ChartConfig,
    paints: Vec<SeriesPaint>,
    evidence: FamilyThemeEvidence,
    rules: BTreeSet<usize>,
    palette: Option<FamilyThemeDisposition>,
    effect_binding: Option<FamilyThemeMechanismKey>,
    legend_plots: OnceLock<[Vec<usize>; 2]>,
    terminal_receipt: OnceLock<()>,
}

impl XyChartSeriesPaintPlan {
    pub(super) fn css_binding(&self) -> &super::css_binding::XyChartCssBinding {
        &self.css
    }

    pub(super) fn chart_config(&self) -> &super::ChartConfig {
        &self.chart_config
    }

    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &MermaidConfig,
        model: &XyChartDiagramRenderModel,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let css = super::css_binding::XyChartCssBinding::resolve(config.as_value());
        let palette = &css.plot_color_palette;
        let mut plan = Self {
            paints: model
                .plots
                .iter()
                .enumerate()
                .map(|(index, plot)| {
                    let color = plot_color_from_palette(palette, index);
                    SeriesPaint {
                        plot_type: plot.plot_type,
                        fill: if plot.plot_type == XyChartPlotType::Bar {
                            color.clone()
                        } else {
                            "none".to_owned()
                        },
                        stroke: color.clone(),
                        point_label_fill: color,
                        stroke_width: if plot.plot_type == XyChartPlotType::Bar {
                            0.0
                        } else {
                            2.0
                        },
                        opacity: None,
                        fill_opacity: None,
                        stroke_opacity: None,
                        effect: None,
                        uses_effect_binding: false,
                        rules: BTreeMap::new(),
                        palette_capabilities: BTreeSet::new(),
                        expected_marks: if plot.plot_type == XyChartPlotType::Bar {
                            plot.data.len()
                        } else {
                            usize::from(!plot.data.is_empty())
                        },
                        expected_labels: if plot.plot_type == XyChartPlotType::Line {
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
                .collect(),
            evidence: FamilyThemeEvidence::from_theme(theme),
            rules: BTreeSet::new(),
            palette: None,
            effect_binding: None,
            legend_plots: OnceLock::new(),
            terminal_receipt: OnceLock::new(),
            chart_config: super::parse_chart_config(config.as_value(), model),
            css,
        };
        let Some(theme) = theme else {
            return Ok(plan);
        };
        let routes = theme
            .family_mechanism_routes()
            .iter()
            .copied()
            .filter(|route| {
                matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::ChartSeries,
                        ..
                    }
                )
            })
            .collect::<Vec<_>>();
        let mut kind_rules = [false; 2];
        for route in &routes {
            if let FamilyThemeMechanism::RuleFacet {
                rule_index,
                selector,
                ..
            } = route.mechanism()
            {
                plan.rules.insert(rule_index);
                match selector {
                    FamilyThemeSelectorShape::Static { variant }
                    | FamilyThemeSelectorShape::Ordinal { variant, .. } => match variant {
                        Some(ThemeVariant::Bar) => {
                            kind_rules[plot_kind(XyChartPlotType::Bar)] = true
                        }
                        Some(ThemeVariant::Line) => {
                            kind_rules[plot_kind(XyChartPlotType::Line)] = true
                        }
                        _ => {}
                    },
                }
            }
        }
        plan.effect_binding = theme.family_mechanism_routes().iter().find_map(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::ChartSeries,
                    ..
                }
            )
            .then(|| theme.family_mechanism_key(*route))
        });
        plan.palette = theme.ordinal_palette_disposition(ThemeTarget::ChartSeries);
        if routes.is_empty() && plan.palette.is_none() && plan.effect_binding.is_none() {
            return Ok(plan);
        }
        let source_colors = merman_core::__private::config_path_overrides_typed_default(
            config,
            "themeVariables.xyChart.plotColorPalette",
        );
        let mut effects: BTreeMap<String, Option<Arc<SvgShadowEffect>>> = BTreeMap::new();
        for (index, paint) in plan.paints.iter_mut().enumerate() {
            let mut style = theme.style_with_work_meter(
                ThemeTarget::ChartSeries,
                ThemeVariant::Default,
                Some(index + 1),
                work_meter,
            )?;
            if kind_rules[plot_kind(paint.plot_type)] {
                let variant = match paint.plot_type {
                    XyChartPlotType::Bar => ThemeVariant::Bar,
                    XyChartPlotType::Line => ThemeVariant::Line,
                };
                // Default and the concrete kind share author order, not selector specificity.
                style.merge_from(&theme.style_with_work_meter(
                    ThemeTarget::ChartSeries,
                    variant,
                    Some(index + 1),
                    work_meter,
                )?);
            }
            let mut consumed = BTreeMap::new();
            if !source_colors {
                for (property, resolved, css) in [
                    (
                        ResolvedStyleProperty::Fill,
                        style.fill_resolution(),
                        &mut paint.fill,
                    ),
                    (
                        ResolvedStyleProperty::Stroke,
                        style.stroke_resolution(),
                        &mut paint.stroke,
                    ),
                ] {
                    match resolved.specified() {
                        Specified::Clear => {
                            consumed.insert(property, ThemeCapability::SemanticRules);
                        }
                        Specified::Value(CanvasPaint::Solid(color)) => {
                            *css = color.as_css();
                            consumed.insert(
                                property,
                                if color.is_transparent() {
                                    ThemeCapability::TransparentPaint
                                } else {
                                    ThemeCapability::SolidPaint
                                },
                            );
                        }
                        Specified::Value(CanvasPaint::Transparent) => {
                            *css = "#00000000".to_owned();
                            consumed.insert(property, ThemeCapability::TransparentPaint);
                        }
                        _ => {}
                    }
                }
                if plan.palette == Some(FamilyThemeDisposition::TypedAdapter) {
                    let color = theme
                        .series_color(ThemeTarget::ChartSeries, index + 1)
                        .expect("compiled palette is non-empty");
                    let css = color.as_css();
                    let capability = if color.is_transparent() {
                        ThemeCapability::TransparentPaint
                    } else {
                        ThemeCapability::SolidPaint
                    };
                    if paint.plot_type == XyChartPlotType::Bar
                        && style.fill_resolution().winner().is_none()
                    {
                        paint.fill.clone_from(&css);
                        paint.palette_capabilities.insert(capability);
                    }
                    if style.stroke_resolution().winner().is_none() {
                        paint.stroke.clone_from(&css);
                        paint.palette_capabilities.insert(capability);
                    }
                    // Point-label paint has always been a palette surface, independently of mark fill.
                    paint.point_label_fill = css;
                    if paint.expected_labels != 0 {
                        paint.palette_capabilities.insert(capability);
                    }
                }
            }
            if !matches!(
                style.stroke_width_resolution().specified(),
                Specified::Unspecified
            ) {
                paint.stroke_width = style
                    .stroke_width()
                    .map(f64::from)
                    .unwrap_or(paint.stroke_width);
                consumed.insert(
                    ResolvedStyleProperty::StrokeWidth,
                    ThemeCapability::BorderStyling,
                );
            }
            for (property, resolved, destination) in [
                (
                    ResolvedStyleProperty::Opacity,
                    style.opacity_resolution(),
                    &mut paint.opacity,
                ),
                (
                    ResolvedStyleProperty::FillOpacity,
                    style.fill_opacity_resolution(),
                    &mut paint.fill_opacity,
                ),
                (
                    ResolvedStyleProperty::StrokeOpacity,
                    style.stroke_opacity_resolution(),
                    &mut paint.stroke_opacity,
                ),
            ] {
                if !matches!(resolved.specified(), Specified::Unspecified) {
                    *destination = resolved.value().copied();
                    consumed.insert(property, ThemeCapability::Opacity);
                }
            }
            match theme.resolve_effect(ThemeTarget::ChartSeries, style.effect_resolution()) {
                Some(ResolvedThemeEffect::ClearedByRule) => {
                    consumed.insert(
                        ResolvedStyleProperty::Effect,
                        ThemeCapability::SemanticRules,
                    );
                }
                Some(ResolvedThemeEffect::Rule { graph }) => {
                    paint.effect = graph.and_then(|graph| {
                        effects
                            .entry(graph.id().to_owned())
                            .or_insert_with(|| SvgShadowEffect::from_graph(graph).map(Arc::new))
                            .clone()
                    });
                    if paint.effect.is_some() {
                        consumed.insert(ResolvedStyleProperty::Effect, ThemeCapability::Shadow);
                    }
                }
                Some(ResolvedThemeEffect::Binding { graph, .. }) => {
                    paint.uses_effect_binding = true;
                    paint.effect = graph.and_then(|graph| {
                        effects
                            .entry(graph.id().to_owned())
                            .or_insert_with(|| SvgShadowEffect::from_graph(graph).map(Arc::new))
                            .clone()
                    });
                }
                None => {}
            }
            work_meter.charge(routes.len())?;
            for route in &routes {
                let FamilyThemeMechanism::RuleFacet {
                    rule_index, facet, ..
                } = route.mechanism()
                else {
                    continue;
                };
                let property = resolved_style_property_for_facet(facet);
                if source_colors
                    && matches!(
                        property,
                        ResolvedStyleProperty::Fill | ResolvedStyleProperty::Stroke
                    )
                {
                    continue;
                }
                if !style.winner_rule_properties().any(|(candidate, origin)| {
                    candidate == property && origin.rule_index() == rule_index
                }) {
                    continue;
                }
                let observation = paint.rules.entry(rule_index).or_default();
                match (route.disposition(), consumed.get(&property)) {
                    (FamilyThemeDisposition::TypedAdapter, Some(capability)) => {
                        observation.capabilities.insert(*capability);
                        if *capability == ThemeCapability::Shadow {
                            observation.capabilities.insert(ThemeCapability::SvgFilter);
                        }
                    }
                    _ => {
                        observation
                            .residual
                            .get_or_insert(unsupported_residual_for_facet(facet));
                    }
                }
            }
        }
        Ok(plan)
    }

    pub(crate) fn has_effect(&self) -> bool {
        self.paints.iter().any(|paint| paint.effect.is_some())
    }

    pub(crate) fn effect(&self, index: usize) -> Option<&Arc<SvgShadowEffect>> {
        self.paints.get(index)?.effect.as_ref()
    }

    pub(crate) fn plot_count(&self) -> usize {
        self.paints.len()
    }
    pub(crate) fn fill_css(&self, index: usize) -> Option<&str> {
        self.paints.get(index).map(|paint| paint.fill.as_str())
    }
    pub(crate) fn stroke_css(&self, index: usize) -> Option<&str> {
        self.paints.get(index).map(|paint| paint.stroke.as_str())
    }
    pub(crate) fn point_label_fill_css(&self, index: usize) -> Option<&str> {
        self.paints
            .get(index)
            .map(|paint| paint.point_label_fill.as_str())
    }
    pub(crate) fn stroke_width(&self, index: usize) -> Option<f64> {
        self.paints.get(index).map(|paint| paint.stroke_width)
    }

    /// Layout knows which titled series actually fit; the writer cannot authorize missing markers.
    pub(crate) fn record_legend_layout(&self, plots: impl IntoIterator<Item = usize>) -> bool {
        if self.rules.is_empty() && self.palette.is_none() && self.effect_binding.is_none() {
            return true;
        }
        let mut markers = [Vec::new(), Vec::new()];
        for index in plots {
            markers[plot_kind(self.paints[index].plot_type)].push(index);
        }
        self.legend_plots.set(markers).is_ok()
    }

    pub(crate) fn legend_plot(&self, plot_type: XyChartPlotType, marker: usize) -> Option<usize> {
        self.legend_plots.get()?[plot_kind(plot_type)]
            .get(marker)
            .copied()
    }

    fn has_legend(&self, index: usize) -> bool {
        self.legend_plots.get().is_some_and(|plots| {
            plots[plot_kind(self.paints[index].plot_type)]
                .binary_search(&index)
                .is_ok()
        })
    }

    pub(crate) fn opacity_attributes(
        &self,
        index: usize,
    ) -> impl Iterator<Item = (&'static str, String)> {
        let paint = &self.paints[index];
        [
            ("opacity", paint.opacity),
            ("fill-opacity", paint.fill_opacity),
            ("stroke-opacity", paint.stroke_opacity),
        ]
        .into_iter()
        .filter_map(|(name, value)| value.map(|value| (name, value.to_string())))
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<XyChartSeriesPaintReceipt> {
        (!self.rules.is_empty() || self.palette.is_some() || self.effect_binding.is_some()).then(
            || XyChartSeriesPaintReceipt {
                plots: self.paints.iter().map(|_| PlotReceipt::default()).collect(),
                attributes_match: true,
            },
        )
    }

    pub(crate) fn record_terminal(&self, receipt: XyChartSeriesPaintReceipt) -> bool {
        receipt.proves(self) && self.terminal_receipt.set(()).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let mut observations: BTreeMap<usize, RuleObservation> = self
            .rules
            .iter()
            .map(|index| (*index, RuleObservation::default()))
            .collect();
        let mut palette_capabilities = BTreeSet::new();
        let mut has_terminal = false;
        for (index, paint) in self.paints.iter().enumerate() {
            if paint.expected_marks == 0 && !self.has_legend(index) {
                continue;
            }
            has_terminal = true;
            palette_capabilities.extend(&paint.palette_capabilities);
            for (rule, actual) in &paint.rules {
                let total = observations.get_mut(rule).expect("registered series rule");
                total.capabilities.extend(&actual.capabilities);
                if total.residual.is_none() {
                    total.residual = actual.residual;
                }
            }
        }
        for (index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index,
                target: ThemeTarget::ChartSeries,
            };
            if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.capabilities.is_empty() {
                evidence.mark_not_applicable(key);
            } else if self.terminal_receipt.get().is_some() {
                evidence.mark_applied_with_capabilities(key, observation.capabilities);
            }
        }
        if let Some(disposition) = self.palette {
            let key = FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::ChartSeries,
            };
            if !has_terminal
                || (disposition == FamilyThemeDisposition::TypedAdapter
                    && palette_capabilities.is_empty())
            {
                evidence.mark_not_applicable(key);
            } else if disposition == FamilyThemeDisposition::TypedAdapter
                && self.terminal_receipt.get().is_some()
            {
                evidence.mark_applied_with_capabilities(key, palette_capabilities);
            } else {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette);
            }
        }
        if let Some(key) = &self.effect_binding {
            let mut used = false;
            let mut unsupported = false;
            for (index, paint) in self.paints.iter().enumerate() {
                if paint.uses_effect_binding
                    && (paint.expected_marks != 0 || self.has_legend(index))
                {
                    used = true;
                    unsupported |= paint.effect.is_none();
                }
            }
            if !used {
                evidence.mark_not_applicable(key.clone());
            } else if unsupported {
                evidence.mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedEffect);
            } else if self.terminal_receipt.get().is_some() {
                evidence.mark_applied_with_capabilities(
                    key.clone(),
                    [ThemeCapability::Shadow, ThemeCapability::SvgFilter],
                );
            }
        }
        evidence
    }
}

fn plot_kind(plot_type: XyChartPlotType) -> usize {
    match plot_type {
        XyChartPlotType::Bar => 0,
        XyChartPlotType::Line => 1,
    }
}

fn number(value: f64) -> String {
    crate::number_format::canonical_number(value).to_string()
}

#[derive(Debug, Default)]
struct PlotReceipt {
    marks: usize,
    labels: usize,
    legend: bool,
}

/// Checks emitted mark and visible legend attributes after the writer output checkpoint.
#[derive(Debug)]
pub(crate) struct XyChartSeriesPaintReceipt {
    plots: Vec<PlotReceipt>,
    attributes_match: bool,
}

impl XyChartSeriesPaintReceipt {
    pub(crate) fn record_mark<'a>(
        &mut self,
        plan: &XyChartSeriesPaintPlan,
        plot: usize,
        mark: usize,
        plot_type: XyChartPlotType,
        legend: bool,
        attribute: impl Fn(&str) -> Option<&'a str>,
    ) {
        let Some((receipt, paint)) = self.plots.get_mut(plot).zip(plan.paints.get(plot)) else {
            self.attributes_match = false;
            return;
        };
        if paint.plot_type != plot_type {
            self.attributes_match = false;
            return;
        }
        if legend {
            self.attributes_match &= !receipt.legend && plan.has_legend(plot);
            receipt.legend = true;
        } else {
            self.attributes_match &= receipt.marks == mark && mark < paint.expected_marks;
            receipt.marks = receipt.marks.saturating_add(1);
        }
        self.attributes_match &= attribute("fill") == Some(paint.fill.as_str())
            && attribute("stroke") == Some(paint.stroke.as_str())
            && attribute("stroke-width") == Some(number(paint.stroke_width).as_str());
        for (name, expected) in [
            ("opacity", paint.opacity),
            ("fill-opacity", paint.fill_opacity),
            ("stroke-opacity", paint.stroke_opacity),
        ] {
            let expected = expected.map(|value| value.to_string());
            self.attributes_match &= attribute(name) == expected.as_deref();
        }
    }

    pub(crate) fn record_line_label(
        &mut self,
        plan: &XyChartSeriesPaintPlan,
        plot: usize,
        label: usize,
        fill: Option<&str>,
    ) {
        let Some((receipt, paint)) = self.plots.get_mut(plot).zip(plan.paints.get(plot)) else {
            self.attributes_match = false;
            return;
        };
        self.attributes_match &= paint.plot_type == XyChartPlotType::Line
            && receipt.labels == label
            && label < paint.expected_labels
            && fill == Some(paint.point_label_fill.as_str());
        receipt.labels = receipt.labels.saturating_add(1);
    }

    fn proves(&self, plan: &XyChartSeriesPaintPlan) -> bool {
        self.attributes_match
            && self.plots.len() == plan.paints.len()
            && self
                .plots
                .iter()
                .zip(&plan.paints)
                .enumerate()
                .all(|(index, (receipt, paint))| {
                    receipt.marks == paint.expected_marks
                        && receipt.labels == paint.expected_labels
                        && receipt.legend == plan.has_legend(index)
                })
    }
}

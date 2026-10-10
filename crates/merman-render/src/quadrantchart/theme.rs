use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use merman_core::MermaidConfig;
use merman_core::diagrams::quadrant_chart::{
    QuadrantChartPointModel, QuadrantChartRenderModel, QuadrantChartStyles,
};

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemePaintKind, FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme,
    ResolvedStyleProperty, Specified, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackOutcome,
    InheritedFontStackPlan, TerminalVariantDomain, UnsupportedTerminalDomain,
    reconcile_unsupported_terminal_domains, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::model::QuadrantChartDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

mod css_binding;
use css_binding::QuadrantChartCssBinding;

/// Typed geometry and paint for Quadrant Chart point marks and their terminal evidence.
#[derive(Debug)]
pub(crate) struct QuadrantChartPointThemePlan {
    css: QuadrantChartCssBinding,
    common_css: crate::svg::PreparedCommonCss,
    terminal_points: Box<[QuadrantChartPointBinding]>,
    points: Box<[QuadrantChartPointThemeExpectation]>,
    inherited_font_stack: InheritedFontStackPlan,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, QuadrantChartPointPendingEvidence>,
    terminal_receipt: OnceLock<QuadrantChartPointThemeReceipt>,
}

#[derive(Debug)]
pub(crate) struct QuadrantChartPointBinding {
    pub(crate) radius: f64,
    pub(crate) fill: String,
    pub(crate) stroke_color: String,
    pub(crate) stroke_width: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct QuadrantChartTextOccurrences {
    quadrants: usize,
    points: usize,
    axis_labels: usize,
    titles: usize,
}

impl QuadrantChartTextOccurrences {
    fn from_layout(layout: &QuadrantChartDiagramLayout) -> Self {
        Self {
            quadrants: layout.quadrants.len(),
            points: layout.points.len(),
            axis_labels: layout.axis_labels.len(),
            titles: usize::from(layout.title.is_some()),
        }
    }
}

#[derive(Debug, Clone, Default)]
struct QuadrantChartPointThemeExpectation {
    radius: Option<QuadrantChartPointRadius>,
    fill: Option<QuadrantChartPointFillExpectation>,
}

#[derive(Debug, Clone)]
struct QuadrantChartPointRadius {
    value_px: f64,
    token: Arc<str>,
    rule_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuadrantChartPointFillOwner {
    Source,
    Config,
    Typed { capability: ThemeCapability },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QuadrantChartPointFillExpectation {
    css: Arc<str>,
    rule_index: usize,
    owner: QuadrantChartPointFillOwner,
}

#[derive(Debug, Clone)]
struct QuadrantChartTypedPointFill {
    css: Arc<str>,
    rule_index: usize,
    capability: ThemeCapability,
}

impl QuadrantChartPointFillExpectation {
    const fn typed_rule_index(&self) -> Option<usize> {
        match self.owner {
            QuadrantChartPointFillOwner::Typed { .. } => Some(self.rule_index),
            QuadrantChartPointFillOwner::Source | QuadrantChartPointFillOwner::Config => None,
        }
    }

    const fn typed_capability(&self) -> Option<ThemeCapability> {
        match self.owner {
            QuadrantChartPointFillOwner::Typed { capability } => Some(capability),
            QuadrantChartPointFillOwner::Source | QuadrantChartPointFillOwner::Config => None,
        }
    }
}

impl QuadrantChartPointThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        model: &QuadrantChartRenderModel,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let point_count = model.points.len();
        work_meter.charge(point_count)?;
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let css = QuadrantChartCssBinding::resolve(effective_config.as_value());
        let common_css = crate::svg::PreparedCommonCss::new(
            effective_config.as_value(),
            Some(inherited_font_stack.font_family_css()),
        );
        let Some(theme) = theme else {
            let points =
                vec![QuadrantChartPointThemeExpectation::default(); point_count].into_boxed_slice();
            let terminal_points = bind_points(model, effective_config, &css, &points);
            return Ok(Self {
                css,
                common_css,
                terminal_points,
                points,
                inherited_font_stack,
                evidence: FamilyThemeEvidence::default(),
                pending: BTreeMap::new(),
                terminal_receipt: OnceLock::new(),
            });
        };

        let config_owns_radius = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "quadrantChart.pointRadius",
        );
        let config_owns_fill = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.quadrantPointFill",
        );
        let mermaid_point_radius_px =
            super::QuadrantChartConfigView::new(effective_config.as_value())
                .layout_settings()
                .point_radius;
        let point_class_styles = model
            .points
            .iter()
            .map(|point| super::point_class_styles(model, point))
            .collect::<Vec<_>>();
        let radius_has_higher_priority_owner = model
            .points
            .iter()
            .zip(point_class_styles.iter().copied())
            .map(|(point, class_styles)| {
                config_owns_radius || point_has_source_radius(point, class_styles)
            })
            .collect::<Vec<_>>();
        let unowned_point_count = radius_has_higher_priority_owner
            .iter()
            .filter(|owned| !**owned)
            .count();

        let mut has_ordinal_series_rules = false;
        let mut static_series_rules = BTreeSet::new();
        for (rule_index, rule) in theme.family_rules() {
            if rule.target() != ThemeTarget::ChartSeries {
                continue;
            }
            has_ordinal_series_rules |= rule.ordinal().is_some();
            if rule.variant().is_none() && rule.ordinal().is_none() {
                static_series_rules.insert(rule_index);
            }
        }
        let mut points = vec![QuadrantChartPointThemeExpectation::default(); point_count];
        let mut occurrence_winners = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut unowned_radius_winners = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut typed_radius_rules = BTreeSet::<usize>::new();
        let mut typed_fill_capabilities = BTreeMap::<usize, ThemeCapability>::new();
        let mut suppressed_fill_rules = BTreeSet::<usize>::new();

        if point_count != 0 && has_ordinal_series_rules {
            for (point_index, radius_has_higher_priority_owner) in
                radius_has_higher_priority_owner.iter().copied().enumerate()
            {
                let style = theme.style_with_work_meter(
                    ThemeTarget::ChartSeries,
                    ThemeVariant::Default,
                    Some(point_index + 1),
                    work_meter,
                )?;
                let winners = winner_properties(&style);
                occurrence_winners.extend(winners.iter().copied());
                if !radius_has_higher_priority_owner {
                    unowned_radius_winners.extend(winners);
                    if let Some(radius) =
                        typed_radius_override(theme, &style, mermaid_point_radius_px)
                    {
                        typed_radius_rules.insert(radius.rule_index);
                        points[point_index].radius = Some(radius);
                    }
                }
                let typed_fill = typed_fill_candidate(theme, &style, &static_series_rules);
                record_fill_expectation(
                    &mut points[point_index].fill,
                    point_fill_expectation(
                        typed_fill.as_ref(),
                        &model.points[point_index],
                        point_class_styles[point_index],
                        config_owns_fill,
                        &css.quadrant_point_fill,
                    ),
                    &mut typed_fill_capabilities,
                    &mut suppressed_fill_rules,
                );
            }
        } else if point_count != 0 {
            let style = theme.style_with_work_meter(
                ThemeTarget::ChartSeries,
                ThemeVariant::Default,
                None,
                work_meter,
            )?;
            let winners = winner_properties(&style);
            occurrence_winners.extend(winners.iter().copied());
            let typed_fill = typed_fill_candidate(theme, &style, &static_series_rules);
            if unowned_point_count != 0 {
                unowned_radius_winners.extend(winners);
                let radius_override = typed_radius_override(theme, &style, mermaid_point_radius_px);
                if let Some(radius) = &radius_override {
                    typed_radius_rules.insert(radius.rule_index);
                }
                for (point_index, radius_has_higher_priority_owner) in
                    radius_has_higher_priority_owner.iter().copied().enumerate()
                {
                    if !radius_has_higher_priority_owner {
                        points[point_index].radius = radius_override.clone();
                    }
                }
            }
            for (point_index, expectation) in points.iter_mut().enumerate() {
                record_fill_expectation(
                    &mut expectation.fill,
                    point_fill_expectation(
                        typed_fill.as_ref(),
                        &model.points[point_index],
                        point_class_styles[point_index],
                        config_owns_fill,
                        &css.quadrant_point_fill,
                    ),
                    &mut typed_fill_capabilities,
                    &mut suppressed_fill_rules,
                );
            }
        }

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let source_owned_fill = model
            .points
            .iter()
            .zip(point_class_styles.iter().copied())
            .map(|(point, class_styles)| {
                config_owns_fill || super::point_source_fill(point, class_styles).is_some()
            })
            .collect::<Vec<_>>();
        let mut observations = BTreeMap::<usize, QuadrantChartSeriesRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::ChartSeries,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    let property = resolved_style_property_for_facet(facet);
                    if !selector.ordinal_domain_intersects_occurrence_count(point_count) {
                        continue;
                    }
                    let radius_facet = facet == FamilyThemeRuleFacet::Radius;
                    let applicable_point_count = if radius_facet {
                        unowned_point_count
                    } else {
                        point_count
                    };
                    let route_won = if radius_facet {
                        unowned_radius_winners.contains(&(rule_index, property))
                    } else {
                        occurrence_winners.contains(&(rule_index, property))
                    };
                    let qualified_variant = matches!(
                        selector,
                        FamilyThemeSelectorShape::Static { variant: Some(_) }
                            | FamilyThemeSelectorShape::Ordinal {
                                variant: Some(_),
                                ..
                            }
                    );
                    if applicable_point_count == 0 || (!route_won && !qualified_variant) {
                        continue;
                    }

                    observation.applicable = true;
                    match (route.disposition(), selector, facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Radius,
                        ) if route_won => {
                            if typed_radius_rules.contains(&rule_index) {
                                observation.pending.radius = true;
                                observation
                                    .pending
                                    .capabilities
                                    .insert(ThemeCapability::RoundedGeometry);
                            } else {
                                observation.incomplete = true;
                            }
                        }
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Fill(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) if route_won => {
                            if let Some(capability) = typed_fill_capabilities.get(&rule_index) {
                                observation.pending.fill = true;
                                observation.pending.capabilities.insert(*capability);
                            } else if !suppressed_fill_rules.contains(&rule_index) {
                                observation.incomplete = true;
                            }
                        }
                        (FamilyThemeDisposition::Unsupported, _, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::TypedAdapter, _, _)
                        | (FamilyThemeDisposition::LegacyCompatibility, _, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::ChartSeries,
                } => {
                    // Reconciled below from each point's final fill winner.
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::ChartSeries,
                    ..
                } => {
                    // Reconciled below from each point's final style.
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
                ThemeTarget::ChartSeries,
                TerminalVariantDomain::uniform(point_count, ThemeVariant::Default),
            )
            .with_source_owned_fill(&source_owned_fill)],
            work_meter,
        )?;

        let mut pending = BTreeMap::new();
        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::ChartSeries,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Mixed rules stay unaccounted until every winning facet has a terminal owner.
            } else if observation.pending.requires_terminal_proof() {
                pending.insert(key, observation.pending);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        let terminal_points = bind_points(model, effective_config, &css, &points);
        Ok(Self {
            css,
            common_css,
            terminal_points,
            points: points.into_boxed_slice(),
            inherited_font_stack,
            evidence,
            pending,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn css(&self) -> &QuadrantChartCssBinding {
        &self.css
    }

    pub(crate) fn point_binding(&self, index: usize) -> &QuadrantChartPointBinding {
        &self.terminal_points[index]
    }

    pub(crate) fn common_css(&self) -> &crate::svg::PreparedCommonCss {
        &self.common_css
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn point_count(&self) -> usize {
        self.points.len()
    }

    pub(crate) fn radius_override_px(&self, point_index: usize) -> Option<f64> {
        self.points
            .get(point_index)
            .and_then(|point| point.radius.as_ref())
            .map(|radius| radius.value_px)
    }

    pub(crate) fn radius_override_token(&self, point_index: usize) -> Option<&str> {
        self.points
            .get(point_index)
            .and_then(|point| point.radius.as_ref())
            .map(|radius| radius.token.as_ref())
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        layout: &QuadrantChartDiagramLayout,
    ) -> Option<QuadrantChartPointThemeReceipt> {
        let requires_receipt = !self.pending.is_empty()
            || self.points.iter().any(|point| point.fill.is_some())
            || self.inherited_font_stack.typography_requested();
        requires_receipt.then(|| {
            QuadrantChartPointThemeReceipt::new(
                self.points.clone(),
                QuadrantChartTextOccurrences::from_layout(layout),
                self.inherited_font_stack.typography_requested(),
                self.font_family_css(),
            )
        })
    }

    pub(crate) fn record_terminal(&self, receipt: QuadrantChartPointThemeReceipt) -> bool {
        receipt.proves_complete() && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(receipt) = self.terminal_receipt.get() else {
            if self.inherited_font_stack.typography_requested() {
                self.inherited_font_stack
                    .mark_unsupported_typography_evidence(&mut evidence, true);
                if self.inherited_font_stack.typed_font_stack_requested() {
                    evidence.mark_residual(
                        FamilyThemeMechanismKey::Typography(
                            crate::diagram_theme::ThemeTypographyProperty::FontStack,
                        ),
                        FamilyThemeResidualReason::UnsupportedTypography,
                    );
                }
            }
            return evidence;
        };
        self.inherited_font_stack
            .mark_unsupported_typography_evidence(&mut evidence, receipt.has_visible_text());
        if self.inherited_font_stack.typed_font_stack_requested() {
            let key = FamilyThemeMechanismKey::Typography(
                crate::diagram_theme::ThemeTypographyProperty::FontStack,
            );
            if !receipt.has_visible_text() {
                evidence.mark_not_applicable(key);
            } else if receipt.proves_font_stack()
                && self.inherited_font_stack.typed_font_stack_active()
            {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            } else {
                match self.inherited_font_stack.outcome() {
                    InheritedFontStackOutcome::ConfigOwned if receipt.proves_font_stack() => {
                        evidence.mark_not_applicable(key)
                    }
                    InheritedFontStackOutcome::Typed
                    | InheritedFontStackOutcome::ConfigOwned
                    | InheritedFontStackOutcome::Unsupported => evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography),
                    InheritedFontStackOutcome::Inactive => {}
                }
            }
        }
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
        evidence
    }
}

fn point_has_source_radius(
    point: &QuadrantChartPointModel,
    class_styles: Option<&QuadrantChartStyles>,
) -> bool {
    point.styles.radius.is_some()
        || class_styles.is_some_and(|class_style| class_style.radius.is_some())
}

fn winner_properties(
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> BTreeSet<(usize, ResolvedStyleProperty)> {
    style
        .winner_rule_properties()
        .map(|(property, origin)| (origin.rule_index(), property))
        .collect()
}

fn record_fill_expectation(
    slot: &mut Option<QuadrantChartPointFillExpectation>,
    fill: Option<QuadrantChartPointFillExpectation>,
    typed_fill_capabilities: &mut BTreeMap<usize, ThemeCapability>,
    suppressed_fill_rules: &mut BTreeSet<usize>,
) {
    let Some(fill) = fill else {
        return;
    };
    if let Some(capability) = fill.typed_capability() {
        typed_fill_capabilities.insert(fill.rule_index, capability);
    } else {
        suppressed_fill_rules.insert(fill.rule_index);
    }
    *slot = Some(fill);
}

fn typed_radius_override(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
    mermaid_point_radius_px: f64,
) -> Option<QuadrantChartPointRadius> {
    let origin = style.radius_resolution().winner()?;
    if theme.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::Radius)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    match style.radius_resolution().specified() {
        Specified::Value(value) => Some(QuadrantChartPointRadius {
            value_px: f64::from(*value),
            token: Arc::from(value.to_string()),
            rule_index: origin.rule_index(),
        }),
        Specified::Clear => Some(QuadrantChartPointRadius {
            value_px: mermaid_point_radius_px,
            token: Arc::from(mermaid_point_radius_px.to_string()),
            rule_index: origin.rule_index(),
        }),
        Specified::Unspecified => None,
    }
}

fn typed_fill_candidate(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
    static_series_rules: &BTreeSet<usize>,
) -> Option<QuadrantChartTypedPointFill> {
    let origin = style.fill_resolution().winner()?;
    let facet = FamilyThemeRuleFacet::fill(style.fill_resolution().specified())?;
    if !static_series_rules.contains(&origin.rule_index())
        || theme.rule_facet_disposition(origin.rule_index(), facet)
            != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }

    let (css, capability) = match style.fill_resolution().specified() {
        Specified::Value(CanvasPaint::Transparent) => {
            (Arc::from("transparent"), ThemeCapability::TransparentPaint)
        }
        Specified::Value(CanvasPaint::Solid(color)) => {
            (Arc::from(color.as_css()), ThemeCapability::SolidPaint)
        }
        Specified::Unspecified
        | Specified::Clear
        | Specified::Value(
            CanvasPaint::LinearGradient(_)
            | CanvasPaint::RadialGradient(_)
            | CanvasPaint::Pattern(_),
        ) => return None,
    };

    Some(QuadrantChartTypedPointFill {
        css,
        rule_index: origin.rule_index(),
        capability,
    })
}

fn bind_points(
    model: &QuadrantChartRenderModel,
    config: &MermaidConfig,
    css: &QuadrantChartCssBinding,
    expectations: &[QuadrantChartPointThemeExpectation],
) -> Box<[QuadrantChartPointBinding]> {
    let default_radius = super::QuadrantChartConfigView::new(config.as_value())
        .layout_settings()
        .point_radius;
    model
        .points
        .iter()
        .zip(expectations)
        .map(|(point, expectation)| {
            let class = super::point_class_styles(model, point);
            QuadrantChartPointBinding {
                radius: point
                    .styles
                    .radius
                    .map(|value| value as f64)
                    .or_else(|| class.and_then(|style| style.radius.map(|value| value as f64)))
                    .or_else(|| expectation.radius.as_ref().map(|radius| radius.value_px))
                    .unwrap_or(default_radius),
                fill: super::point_source_fill(point, class)
                    .or_else(|| expectation.fill.as_ref().map(|fill| fill.css.as_ref()))
                    .unwrap_or(&css.quadrant_point_fill)
                    .to_owned(),
                stroke_color: point
                    .styles
                    .stroke_color
                    .as_deref()
                    .or_else(|| class.and_then(|style| style.stroke_color.as_deref()))
                    .unwrap_or(&css.quadrant_point_fill)
                    .to_owned(),
                stroke_width: point
                    .styles
                    .stroke_width
                    .as_deref()
                    .or_else(|| class.and_then(|style| style.stroke_width.as_deref()))
                    .unwrap_or("0px")
                    .to_owned(),
            }
        })
        .collect()
}

fn point_fill_expectation(
    typed_fill: Option<&QuadrantChartTypedPointFill>,
    point: &QuadrantChartPointModel,
    class_styles: Option<&QuadrantChartStyles>,
    config_owns_fill: bool,
    mermaid_point_fill_css: &str,
) -> Option<QuadrantChartPointFillExpectation> {
    let typed_fill = typed_fill?;
    let (css, owner) = if let Some(source_fill) = super::point_source_fill(point, class_styles) {
        (Arc::from(source_fill), QuadrantChartPointFillOwner::Source)
    } else if config_owns_fill {
        (
            Arc::from(mermaid_point_fill_css),
            QuadrantChartPointFillOwner::Config,
        )
    } else {
        (
            typed_fill.css.clone(),
            QuadrantChartPointFillOwner::Typed {
                capability: typed_fill.capability,
            },
        )
    };
    Some(QuadrantChartPointFillExpectation {
        css,
        rule_index: typed_fill.rule_index,
        owner,
    })
}

/// Writer-owned proof that themed point circles and inherited typography reached the terminal SVG.
#[derive(Debug)]
pub(crate) struct QuadrantChartPointThemeReceipt {
    expectations: Box<[QuadrantChartPointThemeExpectation]>,
    next_point_index: usize,
    attributes_match: bool,
    expected_text: QuadrantChartTextOccurrences,
    emitted_text: QuadrantChartTextOccurrences,
    visible_text_count: usize,
    expected_font_family_css: Box<str>,
    typography_requested: bool,
    css_emitted: bool,
    css_emission_unique: bool,
    css_font_family_matches: bool,
    radius_rules: BTreeSet<usize>,
    fill_rules: BTreeSet<usize>,
}

impl QuadrantChartPointThemeReceipt {
    fn new(
        expectations: Box<[QuadrantChartPointThemeExpectation]>,
        expected_text: QuadrantChartTextOccurrences,
        typography_requested: bool,
        expected_font_family_css: &str,
    ) -> Self {
        Self {
            expectations,
            next_point_index: 0,
            attributes_match: true,
            expected_text,
            emitted_text: QuadrantChartTextOccurrences::default(),
            visible_text_count: 0,
            expected_font_family_css: expected_font_family_css.into(),
            typography_requested,
            css_emitted: false,
            css_emission_unique: true,
            css_font_family_matches: false,
            radius_rules: BTreeSet::new(),
            fill_rules: BTreeSet::new(),
        }
    }

    pub(crate) fn record_css_emission(
        &mut self,
        root_font_family_css: &str,
        inherited_font_family_css: &str,
        root_variable_font_family_css: &str,
    ) {
        if !self.typography_requested {
            return;
        }
        if self.css_emitted {
            self.css_emission_unique = false;
            return;
        }
        let expected = self.expected_font_family_css.as_ref();
        self.css_emitted = true;
        self.css_font_family_matches = root_font_family_css == expected
            && inherited_font_family_css == expected
            && root_variable_font_family_css == expected;
    }

    fn record_text(&mut self, role: QuadrantChartTextRole, text: &str) {
        match role {
            QuadrantChartTextRole::Quadrant => {
                self.emitted_text.quadrants = self.emitted_text.quadrants.saturating_add(1);
            }
            QuadrantChartTextRole::Point => {
                self.emitted_text.points = self.emitted_text.points.saturating_add(1);
            }
            QuadrantChartTextRole::AxisLabel => {
                self.emitted_text.axis_labels = self.emitted_text.axis_labels.saturating_add(1);
            }
            QuadrantChartTextRole::Title => {
                self.emitted_text.titles = self.emitted_text.titles.saturating_add(1);
            }
        }
        if !text.trim().is_empty() {
            self.visible_text_count = self.visible_text_count.saturating_add(1);
        }
    }

    pub(crate) fn record_quadrant_text(&mut self, text: &str) {
        self.record_text(QuadrantChartTextRole::Quadrant, text);
    }

    pub(crate) fn record_point_text(&mut self, text: &str) {
        self.record_text(QuadrantChartTextRole::Point, text);
    }

    pub(crate) fn record_axis_label(&mut self, text: &str) {
        self.record_text(QuadrantChartTextRole::AxisLabel, text);
    }

    pub(crate) fn record_title_text(&mut self, text: &str) {
        self.record_text(QuadrantChartTextRole::Title, text);
    }

    pub(crate) fn record_checkpointed_point(
        &mut self,
        point_index: usize,
        emitted_radius_token: Option<&str>,
        emitted_fill: &str,
    ) {
        if point_index != self.next_point_index {
            self.attributes_match = false;
            return;
        }
        self.next_point_index = self.next_point_index.saturating_add(1);
        let Some(expected) = self.expectations.get(point_index) else {
            self.attributes_match = false;
            return;
        };
        let radius_matches =
            emitted_radius_token == expected.radius.as_ref().map(|radius| radius.token.as_ref());
        let fill_matches = expected
            .fill
            .as_ref()
            .is_none_or(|fill| emitted_fill == fill.css.as_ref());
        let terminal_matches = radius_matches && fill_matches;
        self.attributes_match &= terminal_matches;

        if terminal_matches {
            if let Some(radius) = &expected.radius {
                self.radius_rules.insert(radius.rule_index);
            }
            if let Some(rule_index) = expected
                .fill
                .as_ref()
                .and_then(QuadrantChartPointFillExpectation::typed_rule_index)
            {
                self.fill_rules.insert(rule_index);
            }
        }
    }

    fn proves_complete(&self) -> bool {
        self.next_point_index == self.expectations.len()
            && self.attributes_match
            && self.proves_typography()
    }

    fn proves_typography(&self) -> bool {
        !self.typography_requested
            || (self.css_emission_unique
                && self.css_font_family_matches
                && self.emitted_text == self.expected_text)
    }

    fn proves_font_stack(&self) -> bool {
        self.proves_typography()
    }

    fn has_visible_text(&self) -> bool {
        self.visible_text_count != 0
    }

    fn proves_rule(&self, rule_index: usize, pending: &QuadrantChartPointPendingEvidence) -> bool {
        self.proves_complete()
            && (!pending.radius || self.radius_rules.contains(&rule_index))
            && (!pending.fill || self.fill_rules.contains(&rule_index))
    }
}

#[derive(Debug, Clone, Copy)]
enum QuadrantChartTextRole {
    Quadrant,
    Point,
    AxisLabel,
    Title,
}

#[derive(Debug, Default)]
struct QuadrantChartSeriesRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    pending: QuadrantChartPointPendingEvidence,
}

#[derive(Debug, Default)]
struct QuadrantChartPointPendingEvidence {
    radius: bool,
    fill: bool,
    capabilities: BTreeSet<ThemeCapability>,
}

impl QuadrantChartPointPendingEvidence {
    const fn requires_terminal_proof(&self) -> bool {
        self.radius || self.fill
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_binding_preserves_unverified_source_and_class_priority() {
        let model: QuadrantChartRenderModel = serde_json::from_value(serde_json::json!({
            "title": null, "accTitle": null, "accDescr": null,
            "quadrants": { "quadrant1Text": "", "quadrant2Text": "", "quadrant3Text": "", "quadrant4Text": "" },
            "axes": { "xAxisLeftText": "", "xAxisRightText": "", "yAxisBottomText": "", "yAxisTopText": "" },
            "points": [{ "text": "point", "x": 0.5, "y": 0.5,
                "className": "custom", "styles": { "color": "var(--point)" } }],
            "classes": { "custom": { "color": "#ffffff", "radius": 7,
                "strokeColor": "currentColor", "strokeWidth": "0.25em" } }
        })).unwrap();
        let config = MermaidConfig::empty_object();
        let css = QuadrantChartCssBinding::resolve(config.as_value());
        let points = bind_points(
            &model,
            &config,
            &css,
            &[QuadrantChartPointThemeExpectation {
                radius: Some(QuadrantChartPointRadius {
                    value_px: 12.0,
                    token: Arc::from("12"),
                    rule_index: 0,
                }),
                fill: Some(QuadrantChartPointFillExpectation {
                    css: Arc::from("#ff0000"),
                    rule_index: 0,
                    owner: QuadrantChartPointFillOwner::Typed {
                        capability: ThemeCapability::SolidPaint,
                    },
                }),
            }],
        );
        assert_eq!(points[0].fill, "var(--point)");
        assert_eq!(points[0].radius, 7.0);
        assert_eq!(points[0].stroke_color, "currentColor");
        assert_eq!(points[0].stroke_width, "0.25em");
    }

    fn receipt(typography_requested: bool) -> QuadrantChartPointThemeReceipt {
        QuadrantChartPointThemeReceipt::new(
            vec![QuadrantChartPointThemeExpectation::default()].into_boxed_slice(),
            QuadrantChartTextOccurrences {
                quadrants: 1,
                points: 1,
                axis_labels: 1,
                titles: 1,
            },
            typography_requested,
            if typography_requested {
                "QuadrantSans"
            } else {
                ""
            },
        )
    }

    fn record_complete_text_pass(receipt: &mut QuadrantChartPointThemeReceipt) {
        receipt.record_quadrant_text("Q1");
        receipt.record_checkpointed_point(0, None, "#123456");
        receipt.record_point_text("Point");
        receipt.record_axis_label("Low");
        receipt.record_title_text("Title");
    }

    #[test]
    fn typography_receipt_requires_matching_css_and_each_text_role() {
        let mut complete = receipt(true);
        complete.record_css_emission("QuadrantSans", "QuadrantSans", "QuadrantSans");
        record_complete_text_pass(&mut complete);
        assert!(complete.proves_complete());

        let mut missing_axis = receipt(true);
        missing_axis.record_css_emission("QuadrantSans", "QuadrantSans", "QuadrantSans");
        missing_axis.record_quadrant_text("Q1");
        missing_axis.record_checkpointed_point(0, None, "#123456");
        missing_axis.record_point_text("Point");
        missing_axis.record_title_text("Title");
        assert!(!missing_axis.proves_complete());

        let mut wrong_css = receipt(true);
        wrong_css.record_css_emission("Wrong", "QuadrantSans", "QuadrantSans");
        record_complete_text_pass(&mut wrong_css);
        assert!(!wrong_css.proves_complete());

        let mut duplicate_css = receipt(true);
        duplicate_css.record_css_emission("QuadrantSans", "QuadrantSans", "QuadrantSans");
        duplicate_css.record_css_emission("QuadrantSans", "QuadrantSans", "QuadrantSans");
        record_complete_text_pass(&mut duplicate_css);
        assert!(!duplicate_css.proves_complete());
    }

    #[test]
    fn point_receipt_does_not_require_unrelated_text_when_typography_is_not_requested() {
        let mut point_only = receipt(false);
        point_only.record_checkpointed_point(0, None, "#123456");
        assert!(point_only.proves_complete());
    }
}

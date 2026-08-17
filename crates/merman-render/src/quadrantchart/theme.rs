use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;
use merman_core::diagrams::quadrant_chart::{QuadrantChartPointModel, QuadrantChartRenderModel};

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, Specified,
    ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

/// Typed radius overrides for Quadrant Chart point marks and their terminal evidence.
#[derive(Debug)]
pub(crate) struct QuadrantChartPointThemePlan {
    radius_overrides: Vec<Option<QuadrantChartPointRadius>>,
    evidence: FamilyThemeEvidence,
    pending_radius_key: Option<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<()>,
}

#[derive(Debug, Clone)]
struct QuadrantChartPointRadius {
    value_px: f64,
    token: Box<str>,
}

impl QuadrantChartPointThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        model: &QuadrantChartRenderModel,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let point_count = model.points.len();
        let Some(theme) = theme else {
            return Ok(Self::baseline(point_count));
        };

        let config_owns_radius = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "quadrantChart.pointRadius",
        );
        let mermaid_point_radius_px =
            super::QuadrantChartConfigView::new(effective_config.as_value())
                .layout_settings()
                .point_radius;
        let radius_has_higher_priority_owner = model
            .points
            .iter()
            .map(|point| config_owns_radius || point_has_source_radius(model, point))
            .collect::<Vec<_>>();
        let unowned_point_count = radius_has_higher_priority_owner
            .iter()
            .filter(|owned| !**owned)
            .count();

        let has_ordinal_series_rules = theme
            .family_rules()
            .any(|(_, rule)| rule.target() == ThemeTarget::ChartSeries && rule.ordinal().is_some());
        let mut radius_overrides = vec![None; point_count];
        let mut occurrence_winners = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut unowned_radius_winners = BTreeSet::<(usize, ResolvedStyleProperty)>::new();

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
                    radius_overrides[point_index] =
                        typed_radius_override(theme, &style, mermaid_point_radius_px);
                }
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
            if unowned_point_count != 0 {
                unowned_radius_winners.extend(winners);
                let radius_override = typed_radius_override(theme, &style, mermaid_point_radius_px);
                for (point_index, radius_has_higher_priority_owner) in
                    radius_has_higher_priority_owner.iter().copied().enumerate()
                {
                    if !radius_has_higher_priority_owner {
                        radius_overrides[point_index] = radius_override.clone();
                    }
                }
            }
        }

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
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
                            observation.radius_pending = true;
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
                    let key = theme.family_mechanism_key(route);
                    if point_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::ChartSeries,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if point_count == 0 {
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
                target: ThemeTarget::ChartSeries,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Mixed rules stay unaccounted until every winning facet has a terminal owner.
            } else if observation.radius_pending {
                debug_assert!(pending_radius_key.is_none());
                pending_radius_key = Some(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            radius_overrides,
            evidence,
            pending_radius_key,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline(point_count: usize) -> Self {
        Self {
            radius_overrides: vec![None; point_count],
            evidence: FamilyThemeEvidence::default(),
            pending_radius_key: None,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn point_count(&self) -> usize {
        self.radius_overrides.len()
    }

    pub(crate) fn radius_override_px(&self, point_index: usize) -> Option<f64> {
        self.radius_overrides
            .get(point_index)
            .and_then(|radius| radius.as_ref())
            .map(|radius| radius.value_px)
    }

    pub(crate) fn radius_override_token(&self, point_index: usize) -> Option<&str> {
        self.radius_overrides
            .get(point_index)
            .and_then(|radius| radius.as_ref())
            .map(|radius| radius.token.as_ref())
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<QuadrantChartPointRadiusThemeReceipt> {
        self.pending_radius_key
            .as_ref()
            .map(|_| QuadrantChartPointRadiusThemeReceipt::new(self.point_count()))
    }

    pub(crate) fn record_terminal(&self, receipt: QuadrantChartPointRadiusThemeReceipt) -> bool {
        self.pending_radius_key.is_some()
            && receipt.proves(self.point_count())
            && self.terminal_receipt.set(()).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if let Some(key) = self.pending_radius_key.clone()
            && self.terminal_receipt.get().is_some()
        {
            evidence.mark_applied_with_capabilities(key, [ThemeCapability::RoundedGeometry]);
        }
        evidence
    }
}

fn point_has_source_radius(
    model: &QuadrantChartRenderModel,
    point: &QuadrantChartPointModel,
) -> bool {
    point.styles.radius.is_some()
        || point
            .class_name
            .as_deref()
            .and_then(|class_name| model.classes.get(class_name))
            .is_some_and(|class_style| class_style.radius.is_some())
}

fn winner_properties(
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> BTreeSet<(usize, ResolvedStyleProperty)> {
    style
        .winner_rule_properties()
        .into_iter()
        .map(|(property, origin)| (origin.rule_index(), property))
        .collect()
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
            token: value.to_string().into_boxed_str(),
        }),
        Specified::Clear => Some(QuadrantChartPointRadius {
            value_px: mermaid_point_radius_px,
            token: mermaid_point_radius_px.to_string().into_boxed_str(),
        }),
        Specified::Unspecified => None,
    }
}

/// Writer-owned proof that all point circles reached the terminal SVG in semantic order.
#[derive(Debug)]
pub(crate) struct QuadrantChartPointRadiusThemeReceipt {
    expected_point_count: usize,
    next_point_index: usize,
    attributes_match: bool,
}

impl QuadrantChartPointRadiusThemeReceipt {
    fn new(expected_point_count: usize) -> Self {
        Self {
            expected_point_count,
            next_point_index: 0,
            attributes_match: true,
        }
    }

    pub(crate) fn record_checkpointed_point(
        &mut self,
        point_index: usize,
        emitted_radius_token: Option<&str>,
        expected_radius_token: Option<&str>,
    ) {
        if point_index != self.next_point_index {
            self.attributes_match = false;
            return;
        }
        self.next_point_index = self.next_point_index.saturating_add(1);
        self.attributes_match &= emitted_radius_token == expected_radius_token;
    }

    fn proves(&self, expected_point_count: usize) -> bool {
        self.expected_point_count == expected_point_count
            && self.next_point_index == expected_point_count
            && self.attributes_match
    }
}

#[derive(Debug, Default)]
struct QuadrantChartSeriesRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    radius_pending: bool,
}

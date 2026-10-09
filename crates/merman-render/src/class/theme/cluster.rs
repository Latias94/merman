use std::collections::{BTreeMap, BTreeSet};

use crate::diagram_theme::{
    FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty,
    ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, resolve_direct_static_fill, resolve_direct_static_stroke,
    resolved_style_property_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::paint::ClassPaintBinding;
use super::terminal::ExpectedPaint;

/// Namespace rectangles share static paint; ordinal rules are accounted as unsupported requests.
#[derive(Debug, Clone, Default)]
pub(super) struct ClassClusterThemePlan {
    pub(super) fill: ClassPaintBinding,
    pub(super) stroke: ClassPaintBinding,
    terminal_style: String,
    static_winners: BTreeMap<ResolvedStyleProperty, usize>,
    ordinal_winners: BTreeSet<(usize, ResolvedStyleProperty)>,
}

impl ClassClusterThemePlan {
    pub(super) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &merman_core::MermaidConfig,
        cluster_count: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self {
            fill: ClassPaintBinding::compatibility(
                config,
                &["themeVariables.clusterBkg"],
                crate::config::config_string(config.as_value(), &["themeVariables", "clusterBkg"])
                    .unwrap_or_else(|| "#ffffde".into()),
            ),
            stroke: ClassPaintBinding::compatibility(
                config,
                &["themeVariables.clusterBorder"],
                crate::config::config_string(
                    config.as_value(),
                    &["themeVariables", "clusterBorder"],
                )
                .unwrap_or_else(|| "#aaaa33".into()),
            ),
            ..Self::default()
        };
        let Some(theme) = theme else { return Ok(plan) };
        let style = theme.style_with_work_meter(
            ThemeTarget::Cluster,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        plan.static_winners = style
            .winner_rule_properties()
            .filter(|(property, _)| !plan.property_overridden(*property))
            .map(|(property, origin)| (property, origin.rule_index()))
            .collect();
        {
            let candidate = resolve_direct_static_fill(
                theme,
                &style,
                &[ThemeTarget::Cluster],
                DirectStaticSelectorDomain::Default,
            )
            .map(|paint| {
                let (css, rule_index, _) = paint.into_parts();
                ExpectedPaint {
                    target: ThemeTarget::Cluster,
                    rule_index,
                    css: css.into_string(),
                }
            });
            plan.fill.lower(&style, false, candidate);
        }
        {
            let candidate = resolve_direct_static_stroke(
                theme,
                &style,
                &[ThemeTarget::Cluster],
                DirectStaticSelectorDomain::Default,
            )
            .map(|paint| {
                let (css, rule_index, _) = paint.into_parts();
                ExpectedPaint {
                    target: ThemeTarget::Cluster,
                    rule_index,
                    css: css.into_string(),
                }
            });
            plan.stroke.lower(&style, true, candidate);
        }
        for (property, paint_binding) in [("fill", &plan.fill), ("stroke", &plan.stroke)] {
            if paint_binding.typed().is_some() {
                plan.terminal_style.push_str(property);
                plan.terminal_style.push(':');
                plan.terminal_style.push_str(paint_binding.css());
                plan.terminal_style.push_str(" !important;");
            }
        }
        if theme
            .family_rules()
            .any(|(_, rule)| rule.target() == ThemeTarget::Cluster && rule.ordinal().is_some())
        {
            for ordinal in 1..=cluster_count {
                let style = theme.style_with_work_meter(
                    ThemeTarget::Cluster,
                    ThemeVariant::Default,
                    Some(ordinal),
                    work_meter,
                )?;
                let winners = style
                    .winner_rule_properties()
                    .filter(|(property, _)| !plan.property_overridden(*property))
                    .map(|(property, origin)| (origin.rule_index(), property))
                    .collect::<Vec<_>>();
                plan.ordinal_winners.extend(winners);
            }
        }
        Ok(plan)
    }

    pub(super) fn terminal_style(&self) -> &str {
        &self.terminal_style
    }

    pub(super) fn route_won(
        &self,
        rule_index: usize,
        selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        let property = resolved_style_property_for_facet(facet);
        match selector {
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default),
            } => self.static_winners.get(&property).copied() == Some(rule_index),
            FamilyThemeSelectorShape::Ordinal {
                variant: None | Some(ThemeVariant::Default),
                ..
            } => self.ordinal_winners.contains(&(rule_index, property)),
            FamilyThemeSelectorShape::Static { .. } | FamilyThemeSelectorShape::Ordinal { .. } => {
                false
            }
        }
    }

    fn property_overridden(&self, property: ResolvedStyleProperty) -> bool {
        match property {
            ResolvedStyleProperty::Fill => self.fill.config_owned(),
            ResolvedStyleProperty::Stroke => self.stroke.config_owned(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, OrdinalSelector, ThemeRule,
        ThemeRuleSet, ThemeStylePatch,
    };
    use crate::resources::RenderResourcePolicy;

    fn resolve(rules: ThemeRuleSet, count: usize) -> ClassClusterThemePlan {
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap()
            .resolve(DiagramFamilyId::CLASS);
        ClassClusterThemePlan::resolve(
            Some(&theme),
            &merman_core::MermaidConfig::default(),
            count,
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .unwrap()
    }

    fn rule(patch: ThemeStylePatch) -> ThemeRule {
        ThemeRule::new(ThemeTarget::Cluster, patch).for_family(DiagramFamilyId::CLASS)
    }

    #[test]
    fn cluster_static_plan_keeps_fill_and_stroke_winners_independent() {
        let plan = resolve(
            ThemeRuleSet::default()
                .with_rule(rule(
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                ))
                .with_rule(
                    rule(ThemeStylePatch::default().with_stroke(CanvasPaint::Transparent))
                        .with_variant(ThemeVariant::Default),
                ),
            2,
        );
        assert_eq!(
            plan.terminal_style(),
            "fill:#123456 !important;stroke:transparent !important;"
        );
        assert_eq!(plan.fill.typed().unwrap().rule_index, 0);
        assert_eq!(plan.stroke.typed().unwrap().rule_index, 1);
    }

    #[test]
    fn cluster_unsupported_ordinal_keeps_static_fallback_without_inventing_a_terminal() {
        let plan = resolve(
            ThemeRuleSet::default()
                .with_rule(rule(
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                ))
                .with_rule(
                    rule(ThemeStylePatch::default().with_fill(CanvasPaint::Transparent))
                        .with_ordinal(OrdinalSelector::exact(2).unwrap()),
                )
                .with_rule(
                    rule(ThemeStylePatch::default().with_stroke(CanvasPaint::Transparent))
                        .with_ordinal(OrdinalSelector::exact(3).unwrap()),
                ),
            2,
        );
        assert_eq!(plan.terminal_style(), "fill:#123456 !important;");
        assert!(
            plan.ordinal_winners
                .contains(&(1, ResolvedStyleProperty::Fill))
        );
        assert!(
            !plan
                .ordinal_winners
                .contains(&(2, ResolvedStyleProperty::Stroke))
        );
    }
}

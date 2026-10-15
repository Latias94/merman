use std::collections::{BTreeMap, BTreeSet};

use crate::diagram_theme::{
    FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty,
    ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, resolve_direct_static_fill, resolved_style_property_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::paint::ClassPaintBinding;
use super::terminal::ExpectedPaint;

/// Class's historical Title channel belongs to visible namespace labels.
/// Generic Text participates in author order and retains its own evidence key.
#[derive(Debug, Clone, Default)]
pub(super) struct ClassNamespaceTitleThemePlan {
    pub(super) fill: ClassPaintBinding,
    terminal_style: String,
    static_winners: BTreeMap<ResolvedStyleProperty, usize>,
    ordinal_winners: BTreeSet<(usize, ResolvedStyleProperty)>,
}

impl ClassNamespaceTitleThemePlan {
    pub(super) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &merman_core::MermaidConfig,
        count: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self {
            fill: ClassPaintBinding::compatibility(
                config,
                &["themeVariables.titleColor"],
                crate::config::config_string(config.as_value(), &["themeVariables", "titleColor"])
                    .unwrap_or_else(|| "#333".into()),
            ),
            ..Self::default()
        };
        let Some(theme) = theme else { return Ok(plan) };
        let style = theme.text_style_with_work_meter(
            ThemeTarget::Title,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        plan.static_winners = style
            .winner_rule_properties()
            .filter(|(property, _)| {
                *property != ResolvedStyleProperty::Fill || !plan.fill.config_owned()
            })
            .map(|(property, origin)| (property, origin.rule_index()))
            .collect();
        {
            let candidate = resolve_direct_static_fill(
                theme,
                &style,
                &[ThemeTarget::Title, ThemeTarget::Text],
                DirectStaticSelectorDomain::Default,
            )
            .map(|paint| {
                let (css, rule_index, _) = paint.into_parts();
                ExpectedPaint {
                    target: style
                        .fill_resolution()
                        .winner()
                        .expect("resolved fill has an origin")
                        .target(),
                    rule_index,
                    css: css.into_string(),
                }
            });
            plan.fill.lower(&style, false, candidate);
        }
        if plan.fill.typed().is_some() {
            plan.terminal_style = format!(
                "color:{} !important;fill:{} !important;",
                plan.fill.css(),
                plan.fill.css()
            );
        }
        if theme.family_rules().any(|(_, rule)| {
            matches!(rule.target(), ThemeTarget::Text | ThemeTarget::Title)
                && rule.ordinal().is_some()
        }) {
            for ordinal in 1..=count {
                let style = theme.text_style_with_work_meter(
                    ThemeTarget::Title,
                    ThemeVariant::Default,
                    Some(ordinal),
                    work_meter,
                )?;
                let winners = style
                    .winner_rule_properties()
                    .filter(|(property, _)| {
                        *property != ResolvedStyleProperty::Fill || !plan.fill.config_owned()
                    })
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
            _ => false,
        }
    }
}

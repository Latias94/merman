use std::collections::{BTreeMap, BTreeSet};

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeRuleFacet, FamilyThemeSelectorShape,
    ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeStyle, Specified, ThemeTarget,
    ThemeVariant,
};
use crate::family::resolved_style_property_for_facet;
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::terminal::{ClassNodeTerminalExpectation, ExpectedPaint};

#[derive(Debug, Clone, Default)]
struct ClassNodeTerminalPaintPlan {
    fill_winner: Option<usize>,
    stroke_winner: Option<usize>,
    label_fill_winner: Option<usize>,
    fill: Option<ExpectedPaint>,
    stroke: Option<ExpectedPaint>,
    label_fill: Option<ExpectedPaint>,
}

impl ClassNodeTerminalPaintPlan {
    fn expectation(&self, id: String) -> ClassNodeTerminalExpectation {
        ClassNodeTerminalExpectation::new(id).with_paints(
            self.fill_winner,
            self.stroke_winner,
            self.label_fill_winner,
            self.fill.clone(),
            self.stroke.clone(),
            self.label_fill.clone(),
        )
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct ClassNodeThemePlan {
    resolved_theme: Option<ResolvedDiagramTheme>,
    mermaid_owns_fill: bool,
    mermaid_owns_stroke: bool,
    mermaid_owns_label_fill: bool,
    static_winner_rules: BTreeMap<(ThemeTarget, ResolvedStyleProperty), usize>,
    ordinal_winner_rules: BTreeSet<(ThemeTarget, usize, ResolvedStyleProperty)>,
    ordinal_paints: Box<[ClassNodeTerminalPaintPlan]>,
}

impl ClassNodeThemePlan {
    pub(super) fn from_config(effective_config: &merman_core::MermaidConfig) -> Self {
        let mermaid_owns_fill = ["themeVariables.mainBkg", "themeVariables.primaryColor"]
            .into_iter()
            .any(|path| {
                merman_core::__private::config_path_overrides_typed_default(effective_config, path)
            });
        let mermaid_owns_stroke = [
            "themeVariables.nodeBorder",
            "themeVariables.primaryBorderColor",
        ]
        .into_iter()
        .any(|path| {
            merman_core::__private::config_path_overrides_typed_default(effective_config, path)
        });
        let mermaid_owns_label_fill = [
            "themeVariables.classText",
            "themeVariables.primaryTextColor",
            "themeVariables.textColor",
        ]
        .into_iter()
        .any(|path| {
            merman_core::__private::config_path_overrides_typed_default(effective_config, path)
        });
        Self {
            mermaid_owns_fill,
            mermaid_owns_stroke,
            mermaid_owns_label_fill,
            ..Self::default()
        }
    }

    pub(super) fn resolve_static(
        &mut self,
        theme: &ResolvedDiagramTheme,
        work_meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let node_style = theme.style_with_work_meter(
            ThemeTarget::Node,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let node_label_style = theme.text_style_with_work_meter(
            ThemeTarget::NodeLabel,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let mut static_winner_rules = node_style
            .winner_rule_properties()
            .map(|(property, origin)| ((ThemeTarget::Node, property), origin.rule_index()))
            .collect::<BTreeMap<_, _>>();
        static_winner_rules.extend(
            node_label_style
                .winner_rule_properties()
                .map(|(property, origin)| {
                    ((ThemeTarget::NodeLabel, property), origin.rule_index())
                }),
        );
        self.resolved_theme = Some(theme.clone());
        self.static_winner_rules = static_winner_rules;
        Ok(())
    }

    pub(super) fn resolve_ordinals(
        &mut self,
        theme: &ResolvedDiagramTheme,
        node_count: usize,
        has_ordinal_rules: bool,
        work_meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        if !has_ordinal_rules {
            return Ok(());
        }

        let mut ordinal_paints = Vec::with_capacity(node_count);
        for ordinal in 1..=node_count {
            let node_style = theme.style_with_work_meter(
                ThemeTarget::Node,
                ThemeVariant::Default,
                Some(ordinal),
                work_meter,
            )?;
            let node_label_style = theme.text_style_with_work_meter(
                ThemeTarget::NodeLabel,
                ThemeVariant::Default,
                Some(ordinal),
                work_meter,
            )?;
            for (target, ordinal_style) in [
                (ThemeTarget::Node, &node_style),
                (ThemeTarget::NodeLabel, &node_label_style),
            ] {
                self.ordinal_winner_rules.extend(
                    ordinal_style
                        .winner_rule_properties()
                        .map(|(property, origin)| (target, origin.rule_index(), property)),
                );
            }
            ordinal_paints.push(self.terminal_paints(theme, &node_style, &node_label_style));
        }
        self.ordinal_paints = ordinal_paints.into_boxed_slice();
        Ok(())
    }

    pub(super) fn resolve_expectations(
        &self,
        node_ids: impl IntoIterator<Item = String>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Vec<ClassNodeTerminalExpectation>, OperationWorkError> {
        node_ids
            .into_iter()
            .enumerate()
            .map(|(index, id)| self.resolve_expectation(id, index + 1, work_meter))
            .collect()
    }

    pub(super) fn route_won(
        &self,
        rule_index: usize,
        target: ThemeTarget,
        selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        if !matches!(target, ThemeTarget::Node | ThemeTarget::NodeLabel) {
            return false;
        }
        if target == ThemeTarget::Node
            && ((matches!(facet, FamilyThemeRuleFacet::Fill(_)) && self.mermaid_owns_fill)
                || (matches!(facet, FamilyThemeRuleFacet::Stroke(_)) && self.mermaid_owns_stroke))
        {
            return false;
        }
        if target == ThemeTarget::NodeLabel
            && matches!(facet, FamilyThemeRuleFacet::Fill(_))
            && self.mermaid_owns_label_fill
        {
            return false;
        }
        let property = resolved_style_property_for_facet(facet);
        match selector {
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default),
            } => self.static_winner_rules.get(&(target, property)).copied() == Some(rule_index),
            FamilyThemeSelectorShape::Ordinal {
                variant: None | Some(ThemeVariant::Default),
                ..
            } => self
                .ordinal_winner_rules
                .contains(&(target, rule_index, property)),
            FamilyThemeSelectorShape::Static { .. } | FamilyThemeSelectorShape::Ordinal { .. } => {
                false
            }
        }
    }

    fn resolve_expectation(
        &self,
        id: String,
        ordinal: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<ClassNodeTerminalExpectation, OperationWorkError> {
        if let Some(paints) = self.ordinal_paints.get(ordinal.saturating_sub(1)) {
            return Ok(paints.expectation(id));
        }
        let Some(theme) = self.resolved_theme.as_ref() else {
            return Ok(ClassNodeTerminalExpectation::new(id));
        };
        let node_style = theme.style_with_work_meter(
            ThemeTarget::Node,
            ThemeVariant::Default,
            Some(ordinal),
            work_meter,
        )?;
        let node_label_style = theme.text_style_with_work_meter(
            ThemeTarget::NodeLabel,
            ThemeVariant::Default,
            Some(ordinal),
            work_meter,
        )?;
        Ok(self
            .terminal_paints(theme, &node_style, &node_label_style)
            .expectation(id))
    }

    fn terminal_paints(
        &self,
        theme: &ResolvedDiagramTheme,
        node_style: &ResolvedThemeStyle,
        node_label_style: &ResolvedThemeStyle,
    ) -> ClassNodeTerminalPaintPlan {
        ClassNodeTerminalPaintPlan {
            fill_winner: paint_winner_rule(
                node_style,
                ResolvedStyleProperty::Fill,
                self.mermaid_owns_fill,
            ),
            stroke_winner: paint_winner_rule(
                node_style,
                ResolvedStyleProperty::Stroke,
                self.mermaid_owns_stroke,
            ),
            label_fill_winner: paint_winner_rule(
                node_label_style,
                ResolvedStyleProperty::Fill,
                self.mermaid_owns_label_fill,
            ),
            fill: typed_paint_expectation(
                theme,
                node_style,
                ResolvedStyleProperty::Fill,
                self.mermaid_owns_fill,
            ),
            stroke: typed_paint_expectation(
                theme,
                node_style,
                ResolvedStyleProperty::Stroke,
                self.mermaid_owns_stroke,
            ),
            label_fill: typed_paint_expectation(
                theme,
                node_label_style,
                ResolvedStyleProperty::Fill,
                self.mermaid_owns_label_fill,
            ),
        }
    }
}

fn paint_winner_rule(
    style: &ResolvedThemeStyle,
    property: ResolvedStyleProperty,
    mermaid_owns: bool,
) -> Option<usize> {
    if mermaid_owns {
        return None;
    }
    match property {
        ResolvedStyleProperty::Fill => style.fill_resolution().winner(),
        ResolvedStyleProperty::Stroke => style.stroke_resolution().winner(),
        _ => None,
    }
    .map(|origin| origin.rule_index())
}

fn typed_paint_expectation(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    property: ResolvedStyleProperty,
    mermaid_owns: bool,
) -> Option<ExpectedPaint> {
    if mermaid_owns {
        return None;
    }
    let resolution = match property {
        ResolvedStyleProperty::Fill => style.fill_resolution(),
        ResolvedStyleProperty::Stroke => style.stroke_resolution(),
        _ => return None,
    };
    let origin = resolution.winner()?;
    let facet = match property {
        ResolvedStyleProperty::Fill => FamilyThemeRuleFacet::fill(resolution.specified())?,
        ResolvedStyleProperty::Stroke => FamilyThemeRuleFacet::stroke(resolution.specified())?,
        _ => return None,
    };
    let has_direct_route = theme.rule_facet_disposition(origin.rule_index(), facet)
        == Some(FamilyThemeDisposition::TypedAdapter);
    if !has_direct_route {
        return None;
    }
    let css = match resolution.specified() {
        Specified::Value(CanvasPaint::Transparent) => "transparent".into(),
        Specified::Value(CanvasPaint::Solid(color)) => color.as_css(),
        Specified::Unspecified
        | Specified::Clear
        | Specified::Value(CanvasPaint::LinearGradient(_))
        | Specified::Value(CanvasPaint::RadialGradient(_))
        | Specified::Value(CanvasPaint::Pattern(_)) => return None,
    };
    Some(ExpectedPaint {
        target: origin.target(),
        rule_index: origin.rule_index(),
        css,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::class::theme::ClassRelationThemePlan;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, OrdinalSelector, ThemeRule, ThemeRuleSet,
        ThemeStylePatch,
    };
    use crate::resources::RenderResourcePolicy;

    fn work_meter() -> OperationWorkMeter {
        OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input())
    }

    #[test]
    fn ordinal_node_paint_cache_preserves_per_occurrence_direct_winners() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::Node,
                                ThemeStylePatch::default()
                                    .with_fill(CanvasPaint::solid("#123456").unwrap()),
                            )
                            .for_family(DiagramFamilyId::CLASS),
                        )
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::Node,
                                ThemeStylePatch::default()
                                    .with_fill(CanvasPaint::solid("#654321").unwrap()),
                            )
                            .for_family(DiagramFamilyId::CLASS)
                            .with_ordinal(OrdinalSelector::exact(1).unwrap()),
                        ),
                ),
            )
            .expect("compile ordinal Class node theme")
            .resolve(DiagramFamilyId::CLASS);
        let meter = work_meter();
        let plan = ClassRelationThemePlan::resolve(
            Some(&theme),
            &merman_core::MermaidConfig::default(),
            0,
            2,
            &meter,
        )
        .expect("resolve Class ordinal node plan");
        let work_after_plan = meter.used();

        let expectations = plan
            .resolve_node_expectations(["Alpha".to_string(), "Beta".to_string()], &meter)
            .expect("bind cached Class ordinal node paints");

        assert_eq!(meter.used(), work_after_plan);
        assert_eq!(expectations[0].typed_fill(false), None);
        assert_eq!(expectations[1].typed_fill(false), Some((0, "#123456")));
    }

    #[test]
    fn generic_text_ordinal_is_charged_once_before_node_binding() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(
                            ThemeTarget::NodeLabel,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#123456").unwrap()),
                        ))
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::Text,
                                ThemeStylePatch::default()
                                    .with_fill(CanvasPaint::solid("#654321").unwrap()),
                            )
                            .with_ordinal(OrdinalSelector::exact(1).unwrap()),
                        ),
                ),
            )
            .unwrap()
            .resolve(DiagramFamilyId::CLASS);
        let meter = work_meter();
        let config = merman_core::MermaidConfig::default();
        let plan = ClassRelationThemePlan::resolve(Some(&theme), &config, 0, 2, &meter).unwrap();
        let work_after_plan = meter.used();
        let expectations = plan
            .resolve_node_expectations(["Alpha".to_string(), "Beta".to_string()], &meter)
            .unwrap();
        assert_eq!(meter.used(), work_after_plan);
        assert_eq!(expectations[0].typed_label_fill(false), None);
        assert_eq!(
            expectations[1].typed_label_fill(false),
            Some((0, "#123456"))
        );
        assert!(work_after_plan > 1);
        let limited = OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(
                    crate::resources::ResourceLimitId::MaxLayoutWorkUnits,
                    work_after_plan - 1,
                )
                .unwrap(),
        );
        assert!(ClassRelationThemePlan::resolve(Some(&theme), &config, 0, 2, &limited).is_err());
    }
}

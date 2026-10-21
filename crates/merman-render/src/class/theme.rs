use std::collections::{BTreeMap, BTreeSet};

use crate::diagram_theme::{
    FamilyThemeMechanism, FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme,
    ResolvedStyleProperty, Specified, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, resolve_direct_static_fill, resolve_direct_static_stroke,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

mod cluster;
mod css_binding;
mod evidence;
mod namespace_title;
mod node;
mod outcomes;
mod paint;
mod relation_width;
mod text;

use cluster::ClassClusterThemePlan;
pub(crate) use css_binding::ClassCssThemeBinding;
pub(crate) use evidence::ClassThemeEvidenceRecorder;
use namespace_title::ClassNamespaceTitleThemePlan;
pub(crate) use node::ClassNodePaintBinding;
use node::ClassNodeThemePlan;
pub(crate) use outcomes::ClassThemeOutcomeSummary;
pub(crate) use outcomes::{
    ClassMarkerPaintBinding, ClassNodePaintOutcomes, ClassRelationPaintOutcome, ClassThemeOutcomes,
};
pub(crate) use relation_width::ClassRelationWidthBinding;
pub(crate) use text::{
    ClassNodeLabelStyleFacts, ClassTextOutcomes, ClassTextPaint, ClassTextTerminalFacts,
    ClassTextThemePlan,
};

/// Prepared final Class relation paint winner shared by bounds, SVG emission, and evidence.
/// Node resolution stays behind the same crate-private handle for existing renderer callers.
#[derive(Debug, Clone, Default)]
pub(crate) struct ClassRelationThemePlan {
    theme_requested: bool,
    stroke_binding: paint::ClassPaintBinding,
    stroke_width: ClassRelationWidthBinding,
    static_winner_rules: BTreeMap<ResolvedStyleProperty, usize>,
    ordinal_winner_rules: BTreeSet<(usize, ResolvedStyleProperty)>,
    node_plan: ClassNodeThemePlan,
    cluster_plan: ClassClusterThemePlan,
    namespace_title_plan: ClassNamespaceTitleThemePlan,
    track_edge_label_backgrounds: bool,
    mermaid_owns_edge_label_background: bool,
    edge_label_background: Option<outcomes::ClassSelectedPaint>,
}

impl ClassRelationThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::presentation::PresentationConfig,
        relation_count: usize,
        node_count: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut stroke_width = ClassRelationWidthBinding::compatibility(effective_config);
        let mermaid_owns_stroke_width = stroke_width.config_owned();
        let mut stroke_binding = paint::ClassPaintBinding::compatibility(
            effective_config,
            &["themeVariables.lineColor"],
            crate::config::config_string(
                effective_config.as_value(),
                &["themeVariables", "lineColor"],
            )
            .unwrap_or_else(|| "#333333".into()),
        );
        let mermaid_owns_stroke = stroke_binding.config_owned();
        let mut node_plan = ClassNodeThemePlan::from_config(effective_config);
        let Some(theme) = theme else {
            return Ok(Self {
                stroke_binding,
                stroke_width,
                node_plan,
                ..Self::default()
            });
        };
        let track_edge_label_backgrounds = theme.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::EdgeLabelBackground,
                    ..
                } | FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::EdgeLabelBackground
                } | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::EdgeLabelBackground,
                    ..
                }
            )
        });
        let mermaid_owns_edge_label_background =
            ["themeVariables.mainBkg", "themeVariables.primaryColor"]
                .into_iter()
                .any(|path| {
                    merman_core::__private::config_paint_overrides_typed_default(
                        effective_config,
                        path,
                    )
                });
        let edge_label_background = if mermaid_owns_edge_label_background {
            None
        } else {
            let style = theme.style_with_work_meter(
                ThemeTarget::EdgeLabelBackground,
                ThemeVariant::Default,
                None,
                work_meter,
            )?;
            resolve_direct_static_fill(
                theme,
                &style,
                &[ThemeTarget::EdgeLabelBackground],
                DirectStaticSelectorDomain::Default,
            )
            .map(|paint| {
                let (css, rule_index, _) = paint.into_parts();
                outcomes::ClassSelectedPaint {
                    target: ThemeTarget::EdgeLabelBackground,
                    rule_index,
                    css: css.into_string(),
                }
            })
        };
        let style = theme.style_with_work_meter(
            ThemeTarget::Edge,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let static_winner_rules = style
            .winner_rule_properties()
            .filter(|(property, _)| {
                (!matches!(
                    property,
                    ResolvedStyleProperty::Fill | ResolvedStyleProperty::Stroke
                ) || !mermaid_owns_stroke)
                    && (*property != ResolvedStyleProperty::StrokeWidth
                        || !mermaid_owns_stroke_width)
                    && (*property != ResolvedStyleProperty::Fill
                        || matches!(
                            style.stroke_resolution().specified(),
                            Specified::Unspecified
                        ))
            })
            .map(|(property, origin)| (property, origin.rule_index()))
            .collect::<BTreeMap<_, _>>();
        node_plan.resolve_static(theme, work_meter)?;
        let ordinal_rule_targets = theme
            .family_rules()
            .filter_map(|(_, rule)| rule.ordinal().is_some().then_some(rule.target()))
            .collect::<BTreeSet<_>>();
        let mut ordinal_winner_rules = BTreeSet::new();
        if ordinal_rule_targets.contains(&ThemeTarget::Edge) {
            for ordinal in 1..=relation_count {
                let ordinal_style = theme.style_with_work_meter(
                    ThemeTarget::Edge,
                    ThemeVariant::Default,
                    Some(ordinal),
                    work_meter,
                )?;
                for (property, origin) in ordinal_style.winner_rule_properties() {
                    if (matches!(
                        property,
                        ResolvedStyleProperty::Fill | ResolvedStyleProperty::Stroke
                    ) && mermaid_owns_stroke)
                        || (property == ResolvedStyleProperty::StrokeWidth
                            && mermaid_owns_stroke_width)
                    {
                        continue;
                    }
                    if property != ResolvedStyleProperty::Fill
                        || matches!(
                            ordinal_style.stroke_resolution().specified(),
                            Specified::Unspecified
                        )
                    {
                        ordinal_winner_rules.insert((origin.rule_index(), property));
                    }
                }
            }
        }
        node_plan.resolve_ordinals(
            theme,
            node_count,
            ordinal_rule_targets.contains(&ThemeTarget::Node)
                || ordinal_rule_targets.contains(&ThemeTarget::NodeLabel)
                || ordinal_rule_targets.contains(&ThemeTarget::Text),
            work_meter,
        )?;
        let stroke_is_specified = !matches!(
            style.stroke_resolution().specified(),
            Specified::Unspecified
        );
        let candidate = if mermaid_owns_stroke {
            None
        } else if stroke_is_specified {
            resolve_direct_static_stroke(
                theme,
                &style,
                &[ThemeTarget::Edge],
                DirectStaticSelectorDomain::Default,
            )
        } else {
            resolve_direct_static_fill(
                theme,
                &style,
                &[ThemeTarget::Edge],
                DirectStaticSelectorDomain::Default,
            )
        }
        .map(|paint| {
            let (css, rule_index, _) = paint.into_parts();
            outcomes::ClassSelectedPaint {
                target: ThemeTarget::Edge,
                rule_index,
                css: css.into_string(),
            }
        });
        stroke_binding.lower(&style, stroke_is_specified, candidate);
        stroke_width.lower(theme, &style);
        Ok(Self {
            theme_requested: true,
            stroke_binding,
            stroke_width,
            static_winner_rules,
            ordinal_winner_rules,
            node_plan,
            cluster_plan: ClassClusterThemePlan::default(),
            namespace_title_plan: ClassNamespaceTitleThemePlan::default(),
            track_edge_label_backgrounds,
            mermaid_owns_edge_label_background,
            edge_label_background,
        })
    }

    pub(crate) fn edge_label_background(&self) -> Option<(usize, &str)> {
        self.edge_label_background
            .as_ref()
            .map(|paint| (paint.rule_index, paint.css.as_str()))
    }

    pub(crate) fn with_cluster_domain(
        mut self,
        theme: Option<&ResolvedDiagramTheme>,
        config: &merman_core::presentation::PresentationConfig,
        cluster_count: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        self.cluster_plan =
            ClassClusterThemePlan::resolve(theme, config, cluster_count, work_meter)?;
        self.namespace_title_plan =
            ClassNamespaceTitleThemePlan::resolve(theme, config, cluster_count, work_meter)?;
        Ok(self)
    }

    pub(crate) fn namespace_title_fill(&self) -> Option<(usize, &str)> {
        self.namespace_title_plan
            .fill
            .typed()
            .map(|paint| (paint.rule_index, paint.css.as_str()))
    }

    pub(crate) fn namespace_title_terminal(&self) -> Option<(usize, &str)> {
        self.namespace_title_plan
            .fill
            .typed()
            .map(|paint| (paint.rule_index, self.namespace_title_plan.terminal_style()))
    }

    pub(crate) fn cluster_terminal_style(&self) -> &str {
        self.cluster_plan.terminal_style()
    }

    pub(crate) fn cluster_css_defaults(&self) -> (&str, &str) {
        (
            self.cluster_plan.fill.compatibility_css(),
            self.cluster_plan.stroke.compatibility_css(),
        )
    }

    pub(crate) fn relation_css_default(&self) -> &str {
        self.stroke_binding.compatibility_css()
    }

    pub(crate) fn namespace_title_css_default(&self) -> &str {
        self.namespace_title_plan.fill.compatibility_css()
    }

    pub(crate) fn cluster_paint_rule_indices(&self) -> (Option<usize>, Option<usize>) {
        (
            self.cluster_plan.fill.typed().map(|paint| paint.rule_index),
            self.cluster_plan
                .stroke
                .typed()
                .map(|paint| paint.rule_index),
        )
    }

    pub(crate) fn relation_width(&self) -> &ClassRelationWidthBinding {
        &self.stroke_width
    }

    pub(crate) fn typed_stroke(&self) -> Option<(usize, &str)> {
        self.stroke_binding
            .typed()
            .map(|expected| (expected.rule_index, expected.css.as_str()))
    }

    pub(crate) fn begin_outcomes(
        &self,
        markers: Vec<ClassMarkerPaintBinding>,
    ) -> ClassThemeOutcomes {
        ClassThemeOutcomes::new(
            self.theme_requested,
            markers,
            self.stroke_binding.stroke_property(),
            self.track_edge_label_backgrounds,
        )
    }

    pub(crate) fn node_paint(&self, ordinal: usize) -> &ClassNodePaintBinding {
        self.node_plan.paint(ordinal)
    }

    fn route_won(
        &self,
        rule_index: usize,
        target: ThemeTarget,
        selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        if target != ThemeTarget::Edge {
            return false;
        }
        let property = crate::family::resolved_style_property_for_facet(facet);
        match selector {
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default),
            } => self.static_winner_rules.get(&property).copied() == Some(rule_index),
            FamilyThemeSelectorShape::Ordinal {
                variant: None | Some(ThemeVariant::Default),
                ..
            } => self.ordinal_winner_rules.contains(&(rule_index, property)),
            FamilyThemeSelectorShape::Static { .. } | FamilyThemeSelectorShape::Ordinal { .. } => {
                false
            }
        }
    }

    fn node_route_won(
        &self,
        rule_index: usize,
        target: ThemeTarget,
        selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        self.node_plan
            .route_won(rule_index, target, selector, facet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relation_stroke_binding_retains_the_fallback_property_and_clear_owner() {
        use crate::diagram_theme::{
            CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
            ThemeStylePatch,
        };
        for clear_stroke in [false, true] {
            let mut patch =
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
            if clear_stroke {
                patch.stroke.paint = Specified::Clear;
            }
            let theme = DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Edge, patch)),
                ))
                .unwrap()
                .resolve(crate::DiagramFamilyId::CLASS);
            let meter =
                OperationWorkMeter::new(crate::resources::RenderResourcePolicy::interactive());
            let plan = ClassRelationThemePlan::resolve(
                Some(&theme),
                &merman_core::presentation::PresentationConfig::empty_object(),
                1,
                0,
                &meter,
            )
            .unwrap();
            if clear_stroke {
                assert!(plan.typed_stroke().is_none());
                assert!(plan.stroke_binding.stroke_property().is_none());
                assert_eq!(plan.stroke_binding.action_rule(), Some(0));
            } else {
                assert_eq!(plan.typed_stroke(), Some((0, "#123456")));
                assert_eq!(
                    plan.stroke_binding.stroke_property(),
                    Some(ResolvedStyleProperty::Fill)
                );
            }
            assert_eq!(plan.relation_css_default(), "#333333");
        }
    }

    #[test]
    fn authored_relation_color_preserves_origin_even_for_equal_tokens() {
        use crate::diagram_theme::{
            CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
            ThemeStylePatch,
        };
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#333333").unwrap()),
                    )),
                ),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::CLASS);
        let meter = OperationWorkMeter::new(crate::resources::RenderResourcePolicy::interactive());
        for token in ["#333333", "#123456"] {
            let config = merman_core::Engine::new()
                .with_site_config(merman_core::presentation::PresentationConfig::from_value(
                    serde_json::json!({"themeVariables":{"lineColor":token}}),
                ))
                .parse_metadata_sync("classDiagram\nA --> B\n")
                .unwrap()
                .effective_config;
            let plan =
                ClassRelationThemePlan::resolve(Some(&theme), &config, 1, 0, &meter).unwrap();
            assert!(plan.stroke_binding.config_owned());
            assert_eq!(plan.stroke_binding.css(), token);
            assert_eq!(plan.relation_css_default(), token);
            assert_eq!(plan.typed_stroke(), None);
            assert_eq!(plan.stroke_binding.stroke_property(), None);
        }
        // Host colors enter the core theme derivation before Class binding. Opaque CSS is
        // therefore rejected here even though the renderer preserves already prepared tokens.
        assert!(
            merman_core::Engine::new()
                .with_site_config(merman_core::presentation::PresentationConfig::from_value(
                    serde_json::json!({"themeVariables":{"lineColor":"var(--edge)"}}),
                ))
                .parse_metadata_sync("classDiagram\nA --> B\n")
                .is_err()
        );
    }
}

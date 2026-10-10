use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeRuleFacet, ResolvedDiagramTheme, ResolvedThemeStyle,
    Specified,
};

#[derive(Debug, Clone, Default)]
enum WidthAction {
    #[default]
    Inherit,
    Clear {
        rule_index: usize,
    },
    Set {
        rule_index: usize,
        value: f32,
    },
}

/// Raw stylesheet spelling and geometry interpretation share one property-local owner.
/// An authored opaque token suppresses typed paint even without a measurable width.
#[derive(Debug, Clone, Default)]
pub(crate) struct ClassRelationWidthBinding {
    compatibility_css: String,
    config_origin: Option<&'static str>,
    paint_width: Option<f32>,
    action: WidthAction,
    terminal_css: Option<String>,
}

impl ClassRelationWidthBinding {
    pub(super) fn compatibility(config: &merman_core::MermaidConfig) -> Self {
        const PATH: &str = "themeVariables.strokeWidth";
        let config_origin =
            merman_core::__private::config_path_overrides_typed_default(config, PATH)
                .then_some(PATH);
        let compatibility_css = crate::config::config_css_number_or_string(
            config.as_value(),
            &["themeVariables", "strokeWidth"],
        )
        .unwrap_or_else(|| "1".to_owned());
        let paint_width = config_origin.and_then(|_| {
            crate::config::config_f64_css_px(config.as_value(), &["themeVariables", "strokeWidth"])
                .filter(|value| value.is_finite() && *value >= 0.0 && *value <= f32::MAX as f64)
                .map(|value| value as f32)
        });
        Self {
            compatibility_css,
            config_origin,
            paint_width,
            ..Self::default()
        }
    }

    pub(super) fn config_owned(&self) -> bool {
        self.config_origin.is_some()
    }

    pub(super) fn lower(&mut self, theme: &ResolvedDiagramTheme, style: &ResolvedThemeStyle) {
        if self.config_owned() {
            return;
        }
        let resolution = style.stroke_width_resolution();
        let Some(origin) = resolution.winner() else {
            return;
        };
        let rule_index = origin.rule_index();
        if theme.rule_facet_disposition(rule_index, FamilyThemeRuleFacet::StrokeWidth)
            != Some(FamilyThemeDisposition::TypedAdapter)
        {
            return;
        }
        match resolution.specified() {
            Specified::Unspecified => {}
            Specified::Clear => {
                self.action = WidthAction::Clear { rule_index };
            }
            Specified::Value(value) => {
                self.action = WidthAction::Set {
                    rule_index,
                    value: *value,
                };
                self.paint_width = Some(*value);
                self.terminal_css = Some(format!(
                    "{}px",
                    crate::number_format::canonical_number(f64::from(*value))
                ));
            }
        }
    }

    pub(crate) fn compatibility_css(&self) -> &str {
        &self.compatibility_css
    }
    pub(crate) fn paint_width(&self) -> Option<f32> {
        self.paint_width
    }
    pub(crate) fn terminal_css(&self) -> Option<&str> {
        self.terminal_css.as_deref()
    }
    pub(super) fn typed_emission(&self) -> Option<(usize, f32)> {
        match self.action {
            WidthAction::Set { rule_index, value } => Some((rule_index, value)),
            _ => None,
        }
    }
    pub(super) fn is_clear_for(&self, index: usize) -> bool {
        matches!(self.action, WidthAction::Clear { rule_index } if rule_index == index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet, ThemeStylePatch,
        ThemeTarget, ThemeVariant,
    };
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};

    #[test]
    fn authored_opaque_width_blocks_typed_without_fabricating_geometry() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default().with_stroke_width(6.0).unwrap(),
                    ),
                )),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::CLASS);
        let meter = OperationWorkMeter::new(RenderResourcePolicy::interactive());
        let style = theme
            .style_with_work_meter(ThemeTarget::Edge, ThemeVariant::Default, None, &meter)
            .unwrap();
        for (token, expected_width) in [("calculated", None), ("0.5em", None), ("2px", Some(2.0))] {
            let config = merman_core::Engine::new()
                .with_site_config(merman_core::MermaidConfig::from_value(serde_json::json!({
                    "themeVariables": {"strokeWidth": token}
                })))
                .parse_metadata_sync("classDiagram\nA --> B\n")
                .unwrap()
                .effective_config;
            let mut binding = ClassRelationWidthBinding::compatibility(&config);
            assert!(binding.config_owned());
            binding.lower(&theme, &style);
            assert_eq!(binding.compatibility_css(), token);
            assert_eq!(binding.paint_width(), expected_width);
            assert_eq!(binding.terminal_css(), None);
            assert_eq!(binding.typed_emission(), None);
        }
    }

    #[test]
    fn clear_width_keeps_the_raw_fallback_without_claiming_typed_geometry() {
        let mut patch = ThemeStylePatch::default();
        patch.stroke.width = Specified::Clear;
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Edge, patch)),
            ))
            .unwrap()
            .resolve(crate::DiagramFamilyId::CLASS);
        let meter = OperationWorkMeter::new(RenderResourcePolicy::interactive());
        let style = theme
            .style_with_work_meter(ThemeTarget::Edge, ThemeVariant::Default, None, &meter)
            .unwrap();
        let mut binding =
            ClassRelationWidthBinding::compatibility(&merman_core::MermaidConfig::empty_object());
        binding.lower(&theme, &style);
        assert!(binding.is_clear_for(0));
        assert_eq!(binding.compatibility_css(), "1");
        assert_eq!(binding.paint_width(), None);
        assert_eq!(binding.terminal_css(), None);
    }
}

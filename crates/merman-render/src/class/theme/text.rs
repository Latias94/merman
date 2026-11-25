use crate::text::TextStyle;
use merman_model::presentation::PresentationConfig;

#[derive(Debug)]
pub(crate) struct ClassTextThemePlan {
    css_binding: super::ClassCssThemeBinding,
    common_css: crate::svg::PreparedCommonCss,
    layout_font_family_css: String,
}

impl ClassTextThemePlan {
    pub(crate) fn resolve(
        theme: Option<&crate::theme::FamilyTheme>,
        config: &PresentationConfig,
    ) -> Self {
        let values = config.as_value();
        let owns = |paths: &[&str]| {
            paths
                .iter()
                .any(|path| config.explicit_config_owns_path(path))
        };
        let native_family = theme.filter(|_| !owns(&["fontFamily", "themeVariables.fontFamily"]));
        let (family, layout_font_family_css) = if let Some(theme) = native_family {
            let typography = &theme.typography;
            let family = crate::theme::font_family_css(typography.stack(typography.body_family));
            (family.clone(), family)
        } else {
            (
                crate::config::config_font_family_css(config),
                super::super::config::ClassConfigView::new(values).layout_font_family_css(),
            )
        };
        let size = theme
            .filter(|_| !owns(&["fontSize", "themeVariables.fontSize"]))
            .map_or_else(
                || {
                    crate::config::config_css_number_or_string(
                        values,
                        &["themeVariables", "fontSize"],
                    )
                    .unwrap_or_else(|| "16px".to_owned())
                },
                |theme| format!("{}px", theme.typography.base_size_px),
            );
        let (css_binding, common_css) = match theme {
            Some(theme) => {
                let css = super::ClassCssThemeBinding::from_native(theme);
                // Feed the shared SVG rules only the native defaults they consume.
                let paints = serde_json::json!({"themeVariables": {
                    "textColor": css.class_text,
                    "nodeBorder": css.node_border,
                    "lineColor": theme.roles.stroke_connector.first_color().to_string(),
                    "strokeWidth": theme.form.stroke_width,
                }});
                let mut common =
                    crate::svg::PreparedCommonCss::with_resolved_typography(&paints, family, size);
                if native_family.is_none() {
                    common = common.with_configured_root_font_family(values);
                }
                (css, common)
            }
            None => (
                super::ClassCssThemeBinding::resolve(values),
                crate::svg::PreparedCommonCss::bind_normalized(values, &family, &size, false),
            ),
        };
        Self {
            css_binding,
            common_css,
            layout_font_family_css,
        }
    }

    pub(crate) fn css_binding(&self) -> &super::ClassCssThemeBinding {
        &self.css_binding
    }

    pub(crate) fn common_css(&self) -> &crate::svg::PreparedCommonCss {
        &self.common_css
    }

    pub(crate) fn apply_layout_text_styles(
        &self,
        text_style: &mut TextStyle,
        html_calc_text_style: &mut TextStyle,
    ) {
        text_style.font_family = Some(self.common_css.font_family().to_owned());
        html_calc_text_style.font_family = Some(self.layout_font_family_css.clone());
    }
}

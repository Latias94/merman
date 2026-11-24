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
        let font = crate::config::config_font_family_css(config);
        let values = config.as_value();
        let size =
            crate::config::config_css_number_or_string(values, &["themeVariables", "fontSize"])
                .unwrap_or_else(|| "16px".to_owned());
        let mut plan = Self {
            css_binding: super::ClassCssThemeBinding::resolve(values),
            common_css: crate::svg::PreparedCommonCss::bind_normalized(values, &font, &size, false),
            layout_font_family_css: super::super::config::ClassConfigView::new(values)
                .layout_font_family_css(),
        };
        if let Some(theme) = theme {
            plan.apply_native(theme, config, &size);
        }
        plan
    }

    fn apply_native(
        &mut self,
        theme: &crate::theme::FamilyTheme,
        config: &PresentationConfig,
        configured_size: &str,
    ) {
        let roles = &theme.roles;
        let typography = &theme.typography;
        let owns = |paths: &[&str]| {
            paths
                .iter()
                .any(|path| config.explicit_config_owns_path(path))
        };
        let native_family = !owns(&["fontFamily", "themeVariables.fontFamily"]);
        let family = if native_family {
            let family = crate::theme::font_family_css(typography.stack(typography.body_family));
            self.layout_font_family_css = family.clone();
            family
        } else {
            self.common_css.font_family().to_owned()
        };
        let native_size = format!("{}px", typography.base_size_px);
        let size = if owns(&["fontSize", "themeVariables.fontSize"]) {
            configured_size
        } else {
            &native_size
        };
        self.common_css = crate::svg::PreparedCommonCss::bind_normalized(
            config.as_value(),
            &family,
            size,
            native_family,
        );

        let css = &mut self.css_binding;
        if !owns(&["themeVariables.mainBkg", "themeVariables.primaryColor"]) {
            css.main_bkg = roles.surface_node.first_color().to_string();
            css.node_default_fill.clone_from(&css.main_bkg);
        }
        if !owns(&[
            "themeVariables.nodeBorder",
            "themeVariables.primaryBorderColor",
        ]) {
            css.node_default_stroke = roles.stroke_border.first_color().to_string();
            css.node_border.clone_from(&css.node_default_stroke);
        }
        if !owns(&[
            "themeVariables.classText",
            "themeVariables.primaryTextColor",
            "themeVariables.textColor",
        ]) {
            css.class_text = roles.text_primary.to_string();
        }
        if !owns(&["themeVariables.noteTextColor"]) {
            css.note_text = roles.text_callout.to_string();
        }
        if !owns(&["themeVariables.textColor"]) {
            css.text_color = roles.text_title.to_string();
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

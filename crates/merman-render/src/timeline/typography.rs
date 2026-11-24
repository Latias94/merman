use merman_model::presentation::PresentationConfig;

use crate::config::config_font_family_css;
use crate::svg::PreparedCommonCss;
use crate::text::TextStyle;
use crate::theme::{FamilyTheme, font_family_css};

use super::TimelineNativeStyle;

#[derive(Debug)]
pub(crate) struct TimelineStyle {
    native: Option<TimelineNativeStyle>,
    css_binding: super::TimelineCssBinding,
    common_css: PreparedCommonCss,
    layout_settings: super::config::TimelineLayoutSettings,
    used_font_size_px: Option<f64>,
}

impl TimelineStyle {
    pub(crate) fn resolve(config: &PresentationConfig, native: Option<&FamilyTheme>) -> Self {
        let config_view = super::TimelineConfigView::new(config.as_value());
        let font_size = config_view.configured_font_size().unwrap_or_default();
        let font_family = config_font_family_css(config);
        let layout_settings = config_view.layout_settings_with_resolved_typography(
            Some(&font_family),
            Some(font_size.used_px.unwrap_or(16.0)),
        );
        let style = Self {
            native: None,
            css_binding: super::TimelineCssBinding::resolve(config.as_value()),
            common_css: PreparedCommonCss::with_resolved_typography(
                config.as_value(),
                font_family,
                font_size.css,
            ),
            layout_settings,
            used_font_size_px: font_size.used_px,
        };
        match native {
            Some(theme) => style.with_native(theme, config),
            None => style,
        }
    }

    fn with_native(mut self, theme: &FamilyTheme, config: &PresentationConfig) -> Self {
        let typography = &theme.typography;
        let source_family = ["themeVariables.fontFamily", "fontFamily"]
            .into_iter()
            .filter(|path| config.explicit_config_owns_path(path))
            .find_map(|path| {
                config
                    .get_str(path)
                    .and_then(crate::config::normalized_config_font_family)
            });
        let body_family = source_family
            .clone()
            .unwrap_or_else(|| font_family_css(typography.stack(typography.body_family)));
        let title_family = source_family
            .unwrap_or_else(|| font_family_css(typography.stack(typography.title_family)));
        let has_config_size = config.explicit_config_owns_path("themeVariables.fontSize");
        let font_size = if has_config_size {
            self.layout_settings.text_style.font_size
        } else {
            typography.base_size_px
        };
        let font_size_css = if has_config_size {
            super::TimelineConfigView::new(config.as_value())
                .configured_font_size()
                .map_or_else(|| format!("{font_size}px"), |size| size.css)
        } else {
            self.used_font_size_px = Some(font_size);
            format!("{font_size}px")
        };
        let label_weight = typography.label_weight.to_string();
        let configured_weight = self
            .css_binding
            .section_font_weight()
            .filter(|_| config.explicit_config_owns_path("themeVariables.fontWeight"));
        let node_weight = configured_weight.unwrap_or(&label_weight).to_owned();
        let layout_settings = &mut self.layout_settings;
        for style in [
            &mut layout_settings.text_style,
            &mut layout_settings.node_text_style,
        ] {
            style.font_family = Some(body_family.clone());
            style.font_size = font_size;
        }
        layout_settings.text_style.font_weight = Some(label_weight.clone());
        if configured_weight.is_none() {
            layout_settings.node_text_style.font_weight = Some(label_weight);
        }
        if !config.explicit_config_owns_path("fontSize") {
            layout_settings.layout_font_size = font_size;
        }
        layout_settings.title_style = Some(TextStyle {
            font_family: Some(title_family),
            font_size: font_size * 1.9375,
            font_weight: Some(typography.title_weight.to_string()),
            font_style: None,
        });
        let native = TimelineNativeStyle::new(theme, config, &self.css_binding, node_weight);
        self.common_css = crate::svg::PreparedCommonCss::with_resolved_typography(
            config.as_value(),
            body_family,
            font_size_css,
        )
        .with_text_color(&native.text);
        self.native = Some(native);
        self
    }

    pub(crate) fn native(&self) -> Option<&TimelineNativeStyle> {
        self.native.as_ref()
    }

    pub(crate) fn css_binding(&self) -> &super::TimelineCssBinding {
        &self.css_binding
    }

    pub(crate) fn common_css(&self) -> &crate::svg::PreparedCommonCss {
        &self.common_css
    }

    pub(crate) fn layout_settings(&self) -> &super::config::TimelineLayoutSettings {
        &self.layout_settings
    }

    pub(crate) const fn used_font_size_px(&self) -> Option<f64> {
        self.used_font_size_px
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_layout_preserves_independent_root_font_offset() {
        let config = crate::test_support::presentation_config(serde_json::json!({
            "theme": "redux", "fontSize": "18px",
            "themeVariables": {"fontSize": "24px", "fontWeight": 900, "textColor": "var(--text)"}
        }));
        let plan = TimelineStyle::resolve(&config, None);
        assert_eq!(plan.layout_settings().text_style.font_size, 24.0);
        assert_eq!(plan.layout_settings().node_text_style.font_size, 24.0);
        assert_eq!(plan.layout_settings().text_style.font_weight, None);
        assert_eq!(
            plan.layout_settings()
                .node_text_style
                .font_weight
                .as_deref(),
            plan.css_binding().section_font_weight()
        );
        assert_eq!(plan.layout_settings().layout_font_size, 18.0);
        assert_eq!(plan.common_css().text_color(), "var(--text)");
        assert_eq!(
            plan.layout_settings().text_style.font_family.as_deref(),
            Some(plan.common_css().font_family())
        );
        assert_eq!(
            plan.layout_settings().node_text_style.font_family,
            plan.layout_settings().text_style.font_family
        );
        for size in [0.0, 0.5] {
            let config = crate::test_support::presentation_config(serde_json::json!({
                "fontSize": "18px", "themeVariables": {"fontSize": format!("{size}px")}
            }));
            let plan = TimelineStyle::resolve(&config, None);
            assert_eq!(plan.layout_settings().text_style.font_size, size);
            assert_eq!(plan.layout_settings().node_text_style.font_size, size);
            assert_eq!(plan.layout_settings().layout_font_size, 18.0);
        }
    }
}

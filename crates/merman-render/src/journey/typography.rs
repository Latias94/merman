use merman_model::presentation::PresentationConfig;

use crate::config::{
    config_f64_css_absolute_px, config_font_family_css,
    config_theme_font_size_css_or_root_number_px_opt,
};
use crate::text::TextStyle;
use crate::theme::{FamilyTheme, font_family_css};

use super::JourneyNativeStyle;

#[derive(Debug)]
pub(crate) struct JourneyStyle {
    native: Option<JourneyNativeStyle>,
    css: super::JourneyCssBinding,
    font_size_px: f64,
}

impl JourneyStyle {
    pub(crate) fn resolve(config: &PresentationConfig, native: Option<&FamilyTheme>) -> Self {
        let font_size_px = config_theme_font_size_css_or_root_number_px_opt(config.as_value())
            .filter(|size| *size >= 0.0)
            .unwrap_or(16.0)
            .max(0.0);
        let css = super::JourneyCssBinding::new(
            config.as_value(),
            &config_font_family_css(config),
            &format!("{font_size_px}px"),
        );
        let style = Self {
            native: None,
            css,
            font_size_px,
        };
        match native {
            Some(theme) => style.with_native(theme, config),
            None => style,
        }
    }

    fn with_native(mut self, theme: &FamilyTheme, config: &PresentationConfig) -> Self {
        let typography = &theme.typography;
        let owns = |path| config.explicit_config_owns_path(path);
        let source_family = ["themeVariables.fontFamily", "fontFamily"]
            .into_iter()
            .filter(|path| owns(path))
            .find_map(|path| {
                config
                    .get_str(path)
                    .and_then(crate::config::normalized_config_font_family)
            });
        let family = source_family
            .clone()
            .unwrap_or_else(|| font_family_css(typography.stack(typography.body_family)));
        self.font_size_px = [
            (
                "themeVariables.fontSize",
                &["themeVariables", "fontSize"][..],
            ),
            ("fontSize", &["fontSize"][..]),
        ]
        .into_iter()
        .filter(|(path, _)| owns(path))
        .find_map(|(_, path)| config_f64_css_absolute_px(config.as_value(), path))
        .filter(|size| *size >= 0.0)
        .unwrap_or(typography.base_size_px);
        let mut css = super::JourneyCssBinding::new(
            config.as_value(),
            &family,
            &format!("{}px", self.font_size_px),
        );
        let native = JourneyNativeStyle::new(theme, config);
        css = css.with_text_color(&native.text);
        let render = &mut css.render;
        if !owns("journey.taskFontFamily") {
            render.task_text_style.font_family = Some(family);
        }
        if !owns("journey.taskFontSize") {
            render.task_text_style.font_size = self.font_size_px;
        }
        if !owns("journey.titleFontFamily") {
            render.title_font_family = source_family
                .unwrap_or_else(|| font_family_css(typography.stack(typography.title_family)));
        }
        if !owns("journey.titleFontSize") {
            render.title_font_size = format!("{}px", self.font_size_px * 1.9375);
        }
        if !owns("journey.titleColor") {
            render.title_color = config
                .get_str("themeVariables.titleColor")
                .filter(|_| owns("themeVariables.titleColor"))
                .map_or_else(|| theme.roles.text_title.to_string(), str::to_owned);
        }
        self.css = css;
        self.native = Some(native);
        self
    }

    pub(crate) fn native(&self) -> Option<&JourneyNativeStyle> {
        self.native.as_ref()
    }

    pub(super) fn native_title_style(&self) -> Option<TextStyle> {
        let native = self.native()?;
        let render = &self.css.render;
        let size = crate::mermaid_style::resolve_mermaid_font_size_px(
            &render.title_font_size,
            crate::mermaid_style::CssFontSizeContext::uniform(self.font_size_px),
        )
        .unwrap_or(self.font_size_px * 1.9375);
        Some(TextStyle {
            font_family: Some(render.title_font_family.clone()),
            font_size: size,
            font_weight: Some(native.title_weight.to_string()),
            font_style: None,
        })
    }

    pub(crate) fn css_binding(&self) -> &super::JourneyCssBinding {
        &self.css
    }

    pub(crate) const fn font_size_px(&self) -> f64 {
        self.font_size_px
    }
}

use merman_model::presentation::PresentationConfig;

use merman_render_core::config::{
    config_font_family_css, config_theme_font_size_css_or_root_number_px_opt,
};
use merman_render_core::mermaid_style::font_family_css;
use merman_render_core::svg_css::PreparedCommonCss;
use merman_render_core::text::TextStyle;
use merman_theme_contract::v2::FamilyTheme;

use super::config::{JourneyConfigView, JourneyLayoutSettings, JourneyRenderSettings};
use super::{JourneyCssBinding, JourneyNativeStyle};

#[derive(Debug)]
enum JourneyAppearance {
    Mermaid(JourneyCssBinding),
    Native(JourneyNativeStyle),
}

/// Resolved Journey layout and SVG presentation.
#[derive(Debug)]
pub struct JourneyStyle {
    appearance: JourneyAppearance,
    common_css: PreparedCommonCss,
    layout_settings: JourneyLayoutSettings,
    render_settings: JourneyRenderSettings,
    font_size_px: f64,
}

impl JourneyStyle {
    pub(crate) fn resolve(config: &PresentationConfig, native: Option<&FamilyTheme>) -> Self {
        match native {
            Some(theme) => Self::resolve_native(config, theme),
            None => Self::resolve_mermaid(config),
        }
    }

    fn resolve_mermaid(config: &PresentationConfig) -> Self {
        let config_view = JourneyConfigView::new(config.as_value());
        let font_size_px = config_theme_font_size_css_or_root_number_px_opt(config.as_value())
            .filter(|size| *size >= 0.0)
            .unwrap_or(16.0)
            .max(0.0);
        Self {
            appearance: JourneyAppearance::Mermaid(JourneyCssBinding::new(config.as_value())),
            common_css: PreparedCommonCss::with_resolved_typography(
                config.as_value(),
                config_font_family_css(config),
                format!("{font_size_px}px"),
            ),
            layout_settings: config_view.layout_settings(None),
            render_settings: config_view.render_settings(),
            font_size_px,
        }
    }

    fn resolve_native(config: &PresentationConfig, theme: &FamilyTheme) -> Self {
        let config_view = JourneyConfigView::new(config.as_value());
        let typography = &theme.typography;
        let family = font_family_css(typography.stack(typography.body_family));
        let font_size_px = typography.base_size_px;
        let native = JourneyNativeStyle::new(theme, config);
        let layout_settings = config_view.layout_settings(Some(&native));
        let render_settings = config_view.native_render_settings(theme, &family, font_size_px);
        let common_css = PreparedCommonCss::with_resolved_typography(
            config.as_value(),
            family,
            format!("{font_size_px}px"),
        )
        .with_text_color(&native.text);
        Self {
            appearance: JourneyAppearance::Native(native),
            common_css,
            layout_settings,
            render_settings,
            font_size_px,
        }
    }

    pub(crate) fn native(&self) -> Option<&JourneyNativeStyle> {
        match &self.appearance {
            JourneyAppearance::Native(native) => Some(native),
            JourneyAppearance::Mermaid(_) => None,
        }
    }

    pub(super) fn native_title_style(&self) -> Option<TextStyle> {
        let native = self.native()?;
        let render = &self.render_settings;
        let size = merman_render_core::mermaid_style::resolve_mermaid_font_size_px(
            &render.title_font_size,
            merman_render_core::mermaid_style::CssFontSizeContext::uniform(self.font_size_px),
        )
        .unwrap_or(self.font_size_px * 1.9375);
        Some(TextStyle {
            font_family: Some(render.title_font_family.clone()),
            font_size: size,
            font_weight: Some(native.title_weight.to_string()),
            font_style: None,
        })
    }

    pub(crate) fn css_binding(&self) -> Option<&JourneyCssBinding> {
        match &self.appearance {
            JourneyAppearance::Mermaid(css) => Some(css),
            JourneyAppearance::Native(_) => None,
        }
    }

    pub(crate) const fn common_css(&self) -> &PreparedCommonCss {
        &self.common_css
    }

    pub(super) const fn layout_settings(&self) -> &JourneyLayoutSettings {
        &self.layout_settings
    }

    pub(crate) const fn render_settings(&self) -> &JourneyRenderSettings {
        &self.render_settings
    }

    pub(crate) fn text_color(&self) -> &str {
        match &self.appearance {
            JourneyAppearance::Mermaid(css) => &css.text_color,
            JourneyAppearance::Native(native) => &native.text,
        }
    }

    pub(super) fn legend_text_style(&self) -> TextStyle {
        TextStyle {
            font_family: Some(self.common_css.font_family().to_owned()),
            font_size: self.font_size_px,
            font_weight: self.native().map(|style| style.label_weight.to_string()),
            font_style: None,
        }
    }
}

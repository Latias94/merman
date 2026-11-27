use merman_model::presentation::PresentationConfig;
use merman_render_core::config::ConfigLookup;
use merman_render_core::mermaid_style::font_family_css;
use merman_render_core::text::TextStyle;

use super::{FlowchartConfigView, FlowchartLayoutSettings};

/// Shared family text defaults for layout, HTML measurement and SVG.
#[derive(Debug, Clone)]
pub struct FlowchartBaseTypographyPlan {
    font_family_css: String,
    font_size_px: f64,
    html_font_size_px: f64,
    native_weight: Option<u16>,
}

#[derive(Debug)]
pub struct FlowchartBaseTypographyStyles {
    pub font_family: String,
    pub font_size: f64,
    pub text_style: TextStyle,
    pub html_label_text_style: TextStyle,
}

impl FlowchartBaseTypographyPlan {
    pub fn resolve(effective_config: &PresentationConfig) -> Self {
        let config = FlowchartConfigView::new(effective_config);
        let font_family_css = config
            .font_family_source(Some(effective_config))
            .map_or_else(|| config.font_family(), |(font, _)| font);
        let font_size_px = config.render_font_size();
        let text_style = config.render_text_style(&font_family_css, font_size_px);
        Self {
            font_family_css,
            font_size_px,
            html_font_size_px: config
                .html_label_measurement_base_style(&text_style)
                .font_size,
            native_weight: None,
        }
    }

    pub fn native(typography: &merman_theme_contract::v2::Typography) -> Self {
        Self {
            font_family_css: font_family_css(typography.stack(typography.body_family)),
            font_size_px: typography.base_size_px,
            html_font_size_px: typography.base_size_px,
            native_weight: Some(typography.label_weight),
        }
    }

    pub(super) fn layout_settings(
        &self,
        effective_config: &(impl ConfigLookup + ?Sized),
    ) -> FlowchartLayoutSettings {
        let config = FlowchartConfigView::new(effective_config);
        let mut settings = config.layout_settings();
        self.apply_to_styles(
            &mut settings.text_style,
            &mut settings.html_label_text_style,
        );
        settings
    }

    pub fn render_styles(
        &self,
        effective_config: &(impl ConfigLookup + ?Sized),
    ) -> FlowchartBaseTypographyStyles {
        let config = FlowchartConfigView::new(effective_config);
        let mut text_style = config.render_text_style(&self.font_family_css, self.font_size_px);
        let mut html_label_text_style = config.html_label_measurement_base_style(&text_style);
        self.apply_to_styles(&mut text_style, &mut html_label_text_style);
        FlowchartBaseTypographyStyles {
            font_family: self.font_family_css.to_string(),
            font_size: self.font_size_px,
            text_style,
            html_label_text_style,
        }
    }

    fn apply_to_styles(&self, text_style: &mut TextStyle, html_label_text_style: &mut TextStyle) {
        text_style.font_family = Some(self.font_family_css.to_string());
        text_style.font_size = self.font_size_px;
        html_label_text_style.font_family = Some(self.font_family_css.to_string());
        html_label_text_style.font_size = self.html_font_size_px;
        if let Some(weight) = self.native_weight {
            text_style.font_weight = Some(weight.to_string());
            html_label_text_style
                .font_weight
                .clone_from(&text_style.font_weight);
        }
    }
}

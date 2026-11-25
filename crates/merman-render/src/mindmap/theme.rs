use merman_model::presentation::PresentationConfig;

use crate::text::TextStyle;

use super::{MindmapCssBinding, MindmapNativeStyle};

#[derive(Debug)]
pub(crate) enum MindmapAppearance {
    Mermaid(Box<MindmapCssBinding>),
    Native(MindmapNativeStyle),
}

impl MindmapAppearance {
    pub(crate) fn text_style(&self) -> &TextStyle {
        match self {
            Self::Mermaid(binding) => &binding.label_style,
            Self::Native(style) => &style.label,
        }
    }
}

const MINDMAP_THEME_COLOR_LIMIT_DEFAULT: usize = 12;

pub(super) fn mindmap_color_scale_css(config: &PresentationConfig, slot: usize) -> String {
    config
        .get_str(&format!("themeVariables.cScale{slot}"))
        .map(str::to_string)
        .unwrap_or_else(|| {
            crate::svg::render_theme::default_c_scale(slot % MINDMAP_THEME_COLOR_LIMIT_DEFAULT)
                .to_owned()
        })
}

pub(super) fn mindmap_theme_color_limit(config: &PresentationConfig) -> usize {
    crate::config::config_f64(config.as_value(), &["themeVariables", "THEME_COLOR_LIMIT"])
        .map(|value| value.round() as i64)
        .filter(|value| *value > 0)
        .unwrap_or(MINDMAP_THEME_COLOR_LIMIT_DEFAULT as i64)
        .clamp(1, 64) as usize
}

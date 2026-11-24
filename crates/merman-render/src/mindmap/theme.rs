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
const MINDMAP_DEFAULT_COLOR_SCALE: [&str; MINDMAP_THEME_COLOR_LIMIT_DEFAULT] = [
    "hsl(240, 100%, 76.2745098039%)",
    "hsl(60, 100%, 73.5294117647%)",
    "hsl(80, 100%, 76.2745098039%)",
    "hsl(270, 100%, 76.2745098039%)",
    "hsl(300, 100%, 76.2745098039%)",
    "hsl(330, 100%, 76.2745098039%)",
    "hsl(0, 100%, 76.2745098039%)",
    "hsl(30, 100%, 76.2745098039%)",
    "hsl(90, 100%, 76.2745098039%)",
    "hsl(150, 100%, 76.2745098039%)",
    "hsl(180, 100%, 76.2745098039%)",
    "hsl(210, 100%, 76.2745098039%)",
];

pub(super) fn mindmap_color_scale_css(config: &PresentationConfig, slot: usize) -> String {
    config
        .get_str(&format!("themeVariables.cScale{slot}"))
        .map(str::to_string)
        .unwrap_or_else(|| {
            MINDMAP_DEFAULT_COLOR_SCALE[slot % MINDMAP_DEFAULT_COLOR_SCALE.len()].to_string()
        })
}

pub(super) fn mindmap_theme_color_limit(config: &PresentationConfig) -> usize {
    crate::config::config_f64(config.as_value(), &["themeVariables", "THEME_COLOR_LIMIT"])
        .map(|value| value.round() as i64)
        .filter(|value| *value > 0)
        .unwrap_or(MINDMAP_THEME_COLOR_LIMIT_DEFAULT as i64)
        .clamp(1, 64) as usize
}

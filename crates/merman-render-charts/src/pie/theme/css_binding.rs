use merman_render_core::config::{
    ConfigLookup, config_css_number_or_string, config_theme_font_size_css_or_root_number_px,
    selected_config_string,
};
use merman_render_core::mermaid_style::{
    CssFontSizeContext, css_font_size_is_zero, css_font_size_override, parse_style_declaration,
    resolve_explicit_css_absolute_font_size_px,
};
use merman_render_core::text::TextStyle;

#[derive(Debug, Clone, Copy)]
pub enum PieTextRole {
    Title,
    Section,
    Legend,
}

impl PieTextRole {
    pub const ALL: [Self; 3] = [Self::Title, Self::Section, Self::Legend];

    pub const fn selector(self) -> &'static str {
        match self {
            Self::Title => super::super::PIE_TITLE_CLASS,
            Self::Section => "slice",
            Self::Legend => "legend text",
        }
    }
}

/// A prepared stylesheet token retains Mermaid's authored spelling, including browser-only CSS.
#[derive(Debug)]
pub struct PieMermaidCss {
    inherited_font_size_px: f64,
    pub slice_stroke: String,
    pub slice_stroke_width: String,
    pub slice_opacity: String,
    pub outer_stroke: String,
    pub outer_stroke_width: String,
    pub outer_stroke_width_number: f64,
    text_fills: [String; 3],
    text_sizes: [PieTextSize; 3],
}

fn text_fill(config: &impl ConfigLookup, role: PieTextRole) -> String {
    let (paths, fallback): (&[&'static str], &str) = match role {
        PieTextRole::Title => (
            &[
                "themeVariables.pieTitleTextColor",
                "themeVariables.taskTextDarkColor",
            ],
            "black",
        ),
        PieTextRole::Section => (
            &[
                "themeVariables.pieSectionTextColor",
                "themeVariables.textColor",
            ],
            "#333",
        ),
        PieTextRole::Legend => (
            &[
                "themeVariables.pieLegendTextColor",
                "themeVariables.taskTextDarkColor",
            ],
            "black",
        ),
    };
    selected_config_string(config, paths)
        .map_or(fallback, |(_, css)| css)
        .to_owned()
}

#[derive(Debug)]
struct PieTextSize {
    css: String,
    measurement_px: f64,
}

impl PieTextSize {
    fn new(css: String, inherited_px: f64) -> Self {
        let measurement_px = Self::resolve(&css, inherited_px).unwrap_or(inherited_px);
        Self {
            css,
            measurement_px,
        }
    }

    fn resolve(css: &str, inherited_px: f64) -> Option<f64> {
        let declaration_list = format!("font-size:{css}");
        let declaration = parse_style_declaration(&declaration_list)?;
        if css_font_size_override(&declaration_list).is_none() {
            return Some(inherited_px);
        }
        if let Some(size) = resolve_explicit_css_absolute_font_size_px(css) {
            return Some(size);
        }
        if css_font_size_is_zero(css) {
            return Some(0.0);
        }
        if declaration.analysis().depends_on_root_font_size() {
            return None;
        }
        declaration.resolve_font_size_px(CssFontSizeContext::uniform(inherited_px))
    }
}

impl PieMermaidCss {
    pub(super) fn new(effective_config: &impl ConfigLookup) -> Self {
        let inherited_font_size_px =
            config_theme_font_size_css_or_root_number_px(effective_config, 16.0);
        let value = |key: &str, fallback: &str| {
            config_css_number_or_string(effective_config, &["themeVariables", key])
                .unwrap_or_else(|| fallback.to_owned())
        };
        Self {
            inherited_font_size_px,
            slice_stroke: value("pieStrokeColor", "black"),
            slice_stroke_width: value("pieStrokeWidth", "2px"),
            slice_opacity: value("pieOpacity", "0.7"),
            outer_stroke: value("pieOuterStrokeColor", "black"),
            outer_stroke_width: value("pieOuterStrokeWidth", "2px"),
            outer_stroke_width_number: super::super::config::pie_outer_stroke_width(
                effective_config,
            ),
            text_fills: PieTextRole::ALL.map(|role| text_fill(effective_config, role)),
            text_sizes: [
                ("pieTitleTextSize", "25px"),
                ("pieSectionTextSize", "17px"),
                ("pieLegendTextSize", "17px"),
            ]
            .map(|(key, fallback)| PieTextSize::new(value(key, fallback), inherited_font_size_px)),
        }
    }

    /// Base size shared by measurement and the host's classic stylesheet.
    pub fn inherited_font_size_px(&self) -> f64 {
        self.inherited_font_size_px
    }

    pub fn text_size(&self, role: PieTextRole) -> &str {
        &self.text_sizes[role as usize].css
    }

    pub fn text_fill(&self, role: PieTextRole) -> &str {
        &self.text_fills[role as usize]
    }

    pub(in crate::pie) fn legend_measurement_style(&self, font_family: &str) -> TextStyle {
        self.measurement_style(PieTextRole::Legend, font_family)
    }

    pub(in crate::pie) fn title_measurement_style(&self, font_family: &str) -> TextStyle {
        self.measurement_style(PieTextRole::Title, font_family)
    }

    fn measurement_style(&self, role: PieTextRole, font_family: &str) -> TextStyle {
        TextStyle {
            font_family: Some(font_family.to_owned()),
            font_size: self.text_sizes[role as usize].measurement_px,
            ..Default::default()
        }
    }
}

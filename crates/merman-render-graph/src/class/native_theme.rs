use merman_model::presentation::PresentationConfig;
use merman_theme_contract::v2::{EdgeLabelBackground, EdgeStyle};

use merman_render_core::mermaid_style::font_family_css;
use merman_render_core::text::TextStyle;
use merman_theme_contract::v2::{Color, FamilyTheme};

#[derive(Debug)]
pub(crate) struct ClassNativeStyle {
    pub(crate) code_style: TextStyle,
    pub(crate) title_style: TextStyle,
    pub(crate) emphasis: String,
    pub(crate) on_emphasis: String,
    pub(crate) guide: String,
    pub(crate) width: String,
    pub(crate) radius: f64,
    pub(crate) edge_label_bg: EdgeLabelBackground,
    pub(crate) container: Color,
}

impl ClassNativeStyle {
    pub(crate) fn resolve(
        theme: &FamilyTheme,
        config: &PresentationConfig,
        body_style: &TextStyle,
    ) -> Self {
        let typography = &theme.typography;
        let roles = &theme.roles;
        let has_configured_family = ["fontFamily", "themeVariables.fontFamily"]
            .into_iter()
            .any(|path| config.explicit_config_owns_path(path));
        Self {
            code_style: TextStyle {
                font_family: Some(font_family_css(typography.stack(typography.code_family))),
                ..body_style.clone()
            },
            title_style: TextStyle {
                font_family: if has_configured_family {
                    body_style.font_family.clone()
                } else {
                    Some(font_family_css(typography.stack(typography.title_family)))
                },
                font_weight: Some(typography.title_weight.to_string()),
                ..body_style.clone()
            },
            emphasis: roles.surface_emphasis.first_color().to_string(),
            on_emphasis: roles.text_on_emphasis.to_string(),
            guide: roles.stroke_guide.first_color().to_string(),
            width: theme.form.stroke_width.to_string(),
            radius: theme.form.radius,
            edge_label_bg: theme.form.edge_label_bg,
            container: roles.surface_container.first_color(),
        }
    }
}

pub(super) fn edge_label_fill(theme: &FamilyTheme, background: Option<Color>) -> Color {
    match theme.form.edge_label_bg {
        EdgeLabelBackground::Transparent => Color::TRANSPARENT,
        EdgeLabelBackground::Pill => theme.roles.surface_node.first_color(),
        EdgeLabelBackground::Auto | EdgeLabelBackground::Canvas => {
            background.unwrap_or_else(|| theme.roles.canvas.first_color())
        }
    }
}

pub(super) fn edge_curve(style: EdgeStyle) -> &'static str {
    match style {
        EdgeStyle::Curved => "basis",
        EdgeStyle::Orthogonal => "step",
        EdgeStyle::Straight | EdgeStyle::Sketch => "linear",
    }
}

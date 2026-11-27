use merman_model::presentation::PresentationConfig;
use merman_render_core::mermaid_style::font_family_css;
use merman_theme_contract::v2::{Color, EdgeLabelBackground, EdgeStyle, FamilyTheme};

#[derive(Debug)]
pub struct ErNativeStyle {
    pub(super) body_family: Option<String>,
    pub(super) code_family: String,
    pub title_family: Option<String>,
    pub(super) font_size: Option<f64>,
    pub title_weight: String,
    pub(super) heading_ink: String,
    pub guide: String,
    pub radius: f64,
    pub border_width: f64,
    pub connector_width: f64,
    pub edge_style: EdgeStyle,
    pub(super) label_background: String,
}

impl ErNativeStyle {
    pub fn resolve(
        theme: &FamilyTheme,
        config: &PresentationConfig,
        canvas: Option<Color>,
    ) -> Self {
        let typography = &theme.typography;
        let roles = &theme.roles;
        let owns = |paths: &[&str]| {
            paths
                .iter()
                .any(|path| config.explicit_config_owns_path(path))
        };
        let family = |role| font_family_css(typography.stack(role));
        let theme_family_active = !owns(&["fontFamily", "themeVariables.fontFamily"]);
        let label_background = match theme.form.edge_label_bg {
            EdgeLabelBackground::Transparent => Color::TRANSPARENT,
            EdgeLabelBackground::Pill => roles.surface_node.first_color(),
            EdgeLabelBackground::Auto | EdgeLabelBackground::Canvas => {
                canvas.unwrap_or_else(|| roles.canvas.first_color())
            }
        }
        .to_string();
        Self {
            body_family: theme_family_active.then(|| family(typography.body_family)),
            code_family: family(typography.code_family),
            title_family: theme_family_active.then(|| family(typography.title_family)),
            font_size: (!owns(&["fontSize", "themeVariables.fontSize", "er.fontSize"]))
                .then_some(typography.base_size_px),
            title_weight: typography.title_weight.to_string(),
            heading_ink: roles.text_on_emphasis.to_string(),
            guide: roles.stroke_guide.first_color().to_string(),
            radius: theme.form.radius,
            border_width: theme.form.stroke_width,
            connector_width: theme.form.connector_width,
            edge_style: theme.form.edge_style,
            label_background,
        }
    }

    pub(super) fn apply_layout(&self, settings: &mut super::ErLayoutSettings) {
        settings.code_family = Some(self.code_family.clone());
        settings.calculation_style = settings.label_style.clone();
    }
}

use merman_model::presentation::PresentationConfig;
#[cfg(feature = "layout-shoal")]
use merman_theme_contract::v2::{Color, EdgeLabelBackground};
use merman_theme_contract::v2::{EdgeStyle, Elevation, MarkerStyle};

use merman_render_core::mermaid_style::font_family_css;
use merman_render_core::text::TextStyle;
use merman_theme_contract::v2::FamilyTheme;

#[derive(Debug)]
pub struct ArchitectureNativeStyle {
    pub layout_text: TextStyle,
    pub(crate) node_fill: String,
    pub(crate) group_fill: String,
    pub(crate) node_stroke: String,
    pub(crate) text_ink: String,
    pub(crate) icon_ink: String,
    pub(crate) stroke_width: f64,
    pub radius: f64,
    pub group_radius: f64,
    pub(crate) elevation: Elevation,
    pub(crate) edge_style: EdgeStyle,
    pub marker_style: MarkerStyle,
    #[cfg(feature = "layout-shoal")]
    pub(crate) edge_label_bg: EdgeLabelBackground,
    #[cfg(feature = "layout-shoal")]
    pub(crate) edge_label_fill: Color,
}

impl ArchitectureNativeStyle {
    pub fn edge_style(&self) -> EdgeStyle {
        self.edge_style
    }

    pub(super) fn resolve(theme: &FamilyTheme, config: &PresentationConfig) -> Self {
        let typography = &theme.typography;
        let roles = &theme.roles;
        let body_family = ["themeVariables.fontFamily", "fontFamily"]
            .into_iter()
            .filter_map(|path| authored(config, &[path]))
            .find_map(merman_render_core::config::normalized_config_font_family)
            .unwrap_or_else(|| font_family_css(typography.stack(typography.body_family)));
        let size = config
            .explicit_config_owns_path("architecture.fontSize")
            .then(|| {
                merman_render_core::config::config_f64(
                    config.as_value(),
                    &["architecture", "fontSize"],
                )
            })
            .flatten()
            .filter(|size| size.is_finite())
            .map_or(typography.base_size_px, |size| size.max(1.0));
        Self {
            layout_text: TextStyle {
                font_family: Some(body_family),
                font_size: size,
                ..TextStyle::default()
            },
            node_fill: authored(
                config,
                &["themeVariables.mainBkg", "themeVariables.primaryColor"],
            )
            .map_or_else(
                || roles.surface_node.first_color().to_string(),
                str::to_owned,
            ),
            group_fill: roles.surface_container.first_color().to_string(),
            node_stroke: authored(
                config,
                &[
                    "themeVariables.nodeBorder",
                    "themeVariables.primaryBorderColor",
                ],
            )
            .map_or_else(
                || roles.stroke_border.first_color().to_string(),
                str::to_owned,
            ),
            text_ink: authored(config, &["themeVariables.textColor"])
                .map_or_else(|| roles.text_primary.to_string(), str::to_owned),
            icon_ink: roles.text_title.to_string(),
            stroke_width: theme.form.stroke_width,
            radius: theme.form.radius,
            group_radius: theme.form.container_radius,
            elevation: theme.elevation.clone(),
            edge_style: theme.form.edge_style,
            marker_style: theme.form.marker_style,
            #[cfg(feature = "layout-shoal")]
            edge_label_bg: theme.form.edge_label_bg,
            #[cfg(feature = "layout-shoal")]
            edge_label_fill: match theme.form.edge_label_bg {
                EdgeLabelBackground::Auto | EdgeLabelBackground::Canvas => {
                    roles.canvas.first_color()
                }
                EdgeLabelBackground::Pill => roles.surface_node.first_color(),
                EdgeLabelBackground::Transparent => Color::TRANSPARENT,
            },
        }
    }
}

pub(super) fn authored<'a>(config: &'a PresentationConfig, paths: &[&str]) -> Option<&'a str> {
    paths
        .iter()
        .filter(|path| config.explicit_config_owns_path(path))
        .find_map(|path| config.get_str(path))
}

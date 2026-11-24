use merman_theme_contract::v2::{EdgeLabelBackground, Elevation, Form};

use crate::DiagramFamilyId;
use crate::theme::{Color, FamilyTheme, FamilyThemeAdapter, ThemeCapabilityFlags, font_family_css};

use super::BlockCssThemeBinding;

pub(crate) struct BlockThemeAdapter;

impl FamilyThemeAdapter for BlockThemeAdapter {
    const FAMILY: DiagramFamilyId = DiagramFamilyId::BLOCK;
    const CAPS: ThemeCapabilityFlags = ThemeCapabilityFlags::CANVAS
        .union(ThemeCapabilityFlags::NODE_SURFACE)
        .union(ThemeCapabilityFlags::CONTAINER)
        .union(ThemeCapabilityFlags::CONNECTOR)
        .union(ThemeCapabilityFlags::ELEVATION)
        .union(ThemeCapabilityFlags::RADIUS)
        .union(ThemeCapabilityFlags::EDGE_STYLE)
        .union(ThemeCapabilityFlags::MARKER_STYLE)
        .union(ThemeCapabilityFlags::TYPOGRAPHY);
}

#[derive(Debug)]
pub(crate) struct BlockNativeStyle {
    pub(crate) form: Form,
    pub(crate) elevation: Elevation,
}

impl BlockNativeStyle {
    pub(super) fn resolve(
        theme: &FamilyTheme,
        background: Option<Color>,
        config: &serde_json::Value,
    ) -> (Self, BlockCssThemeBinding) {
        let roles = &theme.roles;
        let canvas = background.unwrap_or_else(|| roles.canvas.first_color());
        let typography = &theme.typography;
        let text_color = roles.text_primary.to_string();
        let border = roles.stroke_border.first_color().to_string();
        let connector = roles.stroke_connector.first_color().to_string();
        let edge_label_background = match theme.form.edge_label_bg {
            EdgeLabelBackground::Auto | EdgeLabelBackground::Canvas => canvas,
            EdgeLabelBackground::Pill => roles.surface_container.first_color(),
            EdgeLabelBackground::Transparent => Color::TRANSPARENT,
        }
        .to_string();
        (
            Self {
                form: theme.form,
                elevation: theme.elevation.clone(),
            },
            BlockCssThemeBinding {
                class_definitions: Vec::new(),
                look_defs: None,
                node_text_color: text_color.clone(),
                text_color,
                title_color: roles.text_title.to_string(),
                main_bkg: roles.surface_node.first_color().to_string(),
                node_border: border.clone(),
                line_color: connector.clone(),
                arrowhead_color: connector,
                stroke_width: theme.form.stroke_width.to_string(),
                edge_label_background,
                cluster_bkg: roles.surface_container.first_color().to_string(),
                cluster_border: border,
                html_labels: crate::config::config_effective_html_labels(config),
                palette_look: String::new(),
                palette: Vec::new(),
                root_font_family: font_family_css(typography.stack(typography.body_family)),
                edge_stroke_width_px: Some(theme.form.connector_width),
                look: config
                    .get("look")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("classic")
                    .to_owned(),
                hand_drawn_seed: config
                    .get("handDrawnSeed")
                    .and_then(serde_json::Value::as_f64),
            },
        )
    }
}

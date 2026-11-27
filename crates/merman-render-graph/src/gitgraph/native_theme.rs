use merman_render_core::mermaid_style::font_family_css;
use merman_theme_contract::v2::{Color, DataPalette, FamilyTheme};

#[derive(Debug)]
pub(super) struct GitGraphNativeStyle {
    pub(super) data: DataPalette,
    pub(super) node: Color,
    pub(super) callout: Color,
    pub(super) border: Color,
    pub(super) primary: Color,
    pub(super) muted: Color,
    pub(super) title: Color,
    pub(super) callout_text: Color,
    pub(super) connector_width: f64,
    pub(super) stroke_width: f64,
    pub(super) body_font: String,
    pub(super) label_weight: String,
    pub(super) title_font: String,
    pub(super) title_weight: String,
}

impl GitGraphNativeStyle {
    pub(super) fn resolve(theme: &FamilyTheme) -> Self {
        let roles = &theme.roles;
        let typography = &theme.typography;
        Self {
            data: theme.data.clone(),
            node: roles.surface_node.first_color(),
            callout: roles.surface_callout.first_color(),
            border: roles.stroke_border.first_color(),
            primary: roles.text_primary,
            muted: roles.text_muted,
            title: roles.text_title,
            callout_text: roles.text_callout,
            connector_width: theme.form.connector_width,
            stroke_width: theme.form.stroke_width,
            body_font: font_family_css(typography.stack(typography.body_family)),
            label_weight: typography.label_weight.to_string(),
            title_font: font_family_css(typography.stack(typography.title_family)),
            title_weight: typography.title_weight.to_string(),
        }
    }

    pub(super) fn slot(&self, branch: i64) -> usize {
        branch.rem_euclid(self.data.ink.len() as i64) as usize
    }
}

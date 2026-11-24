use merman_theme_contract::v2::DataPalette;

use crate::theme::{Color, FamilyTheme, font_family_css};

#[derive(Debug)]
pub(crate) struct GitGraphNativeStyle {
    pub(crate) data: DataPalette,
    pub(crate) node: Color,
    pub(crate) callout: Color,
    pub(crate) border: Color,
    pub(crate) primary: Color,
    pub(crate) muted: Color,
    pub(crate) title: Color,
    pub(crate) callout_text: Color,
    pub(crate) connector_width: f64,
    pub(crate) stroke_width: f64,
    pub(crate) body_font: String,
    pub(crate) label_weight: String,
    pub(crate) title_font: String,
    pub(crate) title_weight: String,
}

impl GitGraphNativeStyle {
    pub(crate) fn resolve(theme: &FamilyTheme) -> Self {
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

    pub(crate) fn slot(&self, branch: i64) -> usize {
        branch.rem_euclid(self.data.ink.len() as i64) as usize
    }
}

use merman_theme_contract::v2::DataPalette;

use crate::text::TextStyle;
use crate::theme::{Color, FamilyTheme, FamilyThemeAdapter, ThemeCapabilityFlags, font_family_css};

pub(crate) struct MindmapThemeAdapter;

impl FamilyThemeAdapter for MindmapThemeAdapter {
    const FAMILY: crate::DiagramFamilyId = crate::DiagramFamilyId::MINDMAP;
    const CAPS: ThemeCapabilityFlags = ThemeCapabilityFlags::CANVAS
        .union(ThemeCapabilityFlags::NODE_SURFACE)
        .union(ThemeCapabilityFlags::EMPHASIS)
        .union(ThemeCapabilityFlags::CONNECTOR)
        .union(ThemeCapabilityFlags::DATA_INK)
        .union(ThemeCapabilityFlags::DATA_FILL)
        .union(ThemeCapabilityFlags::RADIUS)
        .union(ThemeCapabilityFlags::TYPOGRAPHY);
}

#[derive(Debug)]
pub(crate) struct MindmapNativeStyle {
    pub(crate) label: TextStyle,
    pub(crate) data: DataPalette,
    pub(crate) node: Color,
    pub(crate) emphasis: Color,
    pub(crate) primary: Color,
    pub(crate) on_emphasis: Color,
    pub(crate) border: Color,
    pub(crate) connector: Color,
    pub(crate) radius: f64,
    pub(crate) stroke_width: f64,
    pub(crate) connector_width: f64,
}

impl MindmapNativeStyle {
    pub(crate) fn resolve(theme: &FamilyTheme) -> Self {
        let typography = &theme.typography;
        let roles = &theme.roles;
        Self {
            label: TextStyle {
                font_family: Some(font_family_css(typography.stack(typography.body_family))),
                font_size: typography.base_size_px,
                font_weight: Some(typography.label_weight.to_string()),
                font_style: None,
            },
            data: theme.data.clone(),
            node: roles.surface_node.first_color(),
            emphasis: roles.surface_emphasis.first_color(),
            primary: roles.text_primary,
            on_emphasis: roles.text_on_emphasis,
            border: roles.stroke_border.first_color(),
            connector: roles.stroke_connector.first_color(),
            radius: theme.form.radius,
            stroke_width: theme.form.stroke_width,
            connector_width: theme.form.connector_width,
        }
    }

    pub(crate) fn branch_slot(&self, section: Option<i32>) -> Option<usize> {
        usize::try_from(section?)
            .ok()
            .map(|section| section % self.data.fill.len())
    }
}

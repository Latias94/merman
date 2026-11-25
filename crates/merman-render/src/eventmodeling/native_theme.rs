use merman_theme_contract::v2::{DataPalette, Form};

use crate::text::TextStyle;
use crate::theme::{Color, FamilyTheme, font_family_css};

use super::EventModelingCssBinding;

#[derive(Debug)]
pub(crate) struct EventModelingNativeStyle {
    pub(crate) label: TextStyle,
    pub(crate) code: TextStyle,
    pub(crate) heading: TextStyle,
    pub(crate) form: Form,
    data: DataPalette,
    primary: Color,
    node: Color,
    border: Color,
    status_info: Color,
    status_danger: Color,
}

impl EventModelingNativeStyle {
    pub(crate) fn resolve(theme: &FamilyTheme, binding: &mut EventModelingCssBinding) -> Self {
        let roles = &theme.roles;
        let border = roles.stroke_border.first_color();
        binding.text_color = roles.text_primary.to_string();
        binding.swimlane_background_fill = roles.surface_container.first_color().to_string();
        binding.swimlane_background_stroke = border.to_string();
        binding.relation_stroke = roles.stroke_connector.first_color().to_string();
        binding.arrowhead_fill.clone_from(&binding.relation_stroke);

        let typography = &theme.typography;
        let style = |role, weight: u16| TextStyle {
            font_family: Some(font_family_css(typography.stack(role))),
            font_size: typography.base_size_px,
            font_weight: Some(weight.to_string()),
            font_style: None,
        };
        Self {
            label: style(typography.body_family, typography.label_weight),
            code: style(typography.code_family, typography.label_weight),
            heading: style(typography.title_family, typography.title_weight),
            form: theme.form,
            data: theme.data.clone(),
            primary: roles.text_primary,
            node: roles.surface_node.first_color(),
            border,
            status_info: roles.status_info,
            status_danger: roles.status_danger,
        }
    }

    fn data_slot(entity_type: &str) -> Option<usize> {
        match entity_type {
            "cmd" | "command" => Some(0),
            "evt" | "event" => Some(1),
            "rmo" | "readmodel" => Some(2),
            "ui" => Some(3),
            _ => None,
        }
    }

    pub(super) fn box_paint(&self, entity_type: &str) -> (Color, Color) {
        if let Some(slot) = Self::data_slot(entity_type) {
            return (self.data.fill(slot), self.border);
        }
        let stroke = match entity_type {
            "pcr" | "processor" => self.status_info,
            _ => self.status_danger,
        };
        (self.node, stroke)
    }

    pub(crate) fn box_ink(&self, entity_type: &str) -> Color {
        Self::data_slot(entity_type).map_or(self.primary, |slot| self.data.fill_ink(slot))
    }

    pub(super) fn write_stylesheet(
        &self,
        out: &mut impl std::fmt::Write,
        id: impl Copy + std::fmt::Display,
    ) {
        let primary = self.primary;
        let _ = write!(out, "#{id} {{ fill: {primary}; color: {primary}; }}");
        for (selector, text) in [
            (".em-swimlane text", &self.heading),
            (".em-box span", &self.label),
            (".em-box code", &self.code),
        ] {
            let family = text.font_family.as_deref().unwrap_or_default();
            let weight = text.font_weight.as_deref().unwrap_or_default();
            let size = text.font_size;
            let _ = write!(
                out,
                "#{id} {selector} {{ font-family: {family}; font-size: {size}px; font-weight: {weight}; }}"
            );
        }
        let _ = write!(out, "#{id} .em-box b {{ font-weight: inherit; }}");
    }
}

use merman_theme_contract::v2::{DataPalette, Elevation, Form};

use crate::model::KanbanItemLayout;
use crate::text::TextStyle;
use crate::theme::{Color, FamilyTheme, font_family_css};

#[derive(Debug)]
pub(crate) struct KanbanNativeStyle {
    pub(crate) body: TextStyle,
    pub(crate) heading: TextStyle,
    pub(crate) form: Form,
    pub(crate) elevation: Elevation,
    pub(crate) data: DataPalette,
    pub(crate) container: Color,
    pub(crate) node: Color,
    pub(crate) border: Color,
    pub(crate) primary: Color,
    pub(crate) title: Color,
    priority: [Color; 4],
}

impl KanbanNativeStyle {
    pub(super) fn resolve(theme: &FamilyTheme) -> Self {
        let typography = &theme.typography;
        let roles = &theme.roles;
        let text = |family, weight: u16| TextStyle {
            font_family: Some(font_family_css(typography.stack(family))),
            font_size: typography.base_size_px,
            font_weight: Some(weight.to_string()),
            font_style: None,
        };
        Self {
            body: text(typography.body_family, typography.label_weight),
            heading: text(typography.title_family, typography.title_weight),
            form: theme.form,
            elevation: theme.elevation.clone(),
            data: theme.data.clone(),
            container: roles.surface_container.first_color(),
            node: roles.surface_node.first_color(),
            border: roles.stroke_border.first_color(),
            primary: roles.text_primary,
            title: roles.text_title,
            priority: [
                roles.status_danger,
                roles.status_warn,
                roles.status_info,
                roles.status_success,
            ],
        }
    }

    pub(crate) fn priority(&self, priority: &str) -> Option<Color> {
        match priority.trim() {
            "Very High" => Some(self.priority[0]),
            "High" => Some(self.priority[1]),
            "Low" => Some(self.priority[2]),
            "Very Low" => Some(self.priority[3]),
            _ => None,
        }
    }
}

pub(crate) fn task_body(item: &KanbanItemLayout) -> kurbo::Rect {
    kurbo::Rect::from_center_size((item.center_x, item.center_y), (item.width, item.height))
}

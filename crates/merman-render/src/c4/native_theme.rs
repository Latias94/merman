use merman_model::presentation::PresentationConfig;
use merman_theme_contract::v2::{EdgeStyle, MarkerStyle};

use crate::theme::{FamilyTheme, FamilyThemeAdapter, ThemeCapabilityFlags, font_family_css};

pub(crate) struct C4ThemeAdapter;

impl FamilyThemeAdapter for C4ThemeAdapter {
    const FAMILY: crate::DiagramFamilyId = crate::DiagramFamilyId::C4;
    const CAPS: ThemeCapabilityFlags = ThemeCapabilityFlags::CANVAS
        .union(ThemeCapabilityFlags::NODE_SURFACE)
        .union(ThemeCapabilityFlags::CONTAINER)
        .union(ThemeCapabilityFlags::DATA_FILL)
        .union(ThemeCapabilityFlags::CONNECTOR)
        .union(ThemeCapabilityFlags::RADIUS)
        .union(ThemeCapabilityFlags::EDGE_STYLE)
        .union(ThemeCapabilityFlags::MARKER_STYLE)
        .union(ThemeCapabilityFlags::TYPOGRAPHY);
}

#[derive(Debug)]
pub(crate) struct C4NativeStyle {
    pub(crate) radius: f64,
    pub(crate) shape_width: f64,
    pub(crate) connector_width: f64,
    pub(crate) edge_style: EdgeStyle,
    pub(crate) marker_style: MarkerStyle,
    pub(crate) title_ink: String,
    pub(crate) title_family: String,
    pub(crate) title_weight: u16,
    pub(crate) text_ink: String,
}

impl C4NativeStyle {
    pub(super) fn resolve(theme: &FamilyTheme, title_family: String) -> Self {
        Self {
            radius: theme.form.radius,
            shape_width: theme.form.stroke_width,
            connector_width: theme.form.connector_width,
            edge_style: theme.form.edge_style,
            marker_style: theme.form.marker_style,
            title_ink: theme.roles.text_title.to_string(),
            title_family,
            title_weight: theme.typography.title_weight,
            text_ink: theme.roles.text_primary.to_string(),
        }
    }
}

pub(super) fn element_colors(
    name: &str,
    theme: &FamilyTheme,
    config: &PresentationConfig,
) -> super::typography::C4ElementColors {
    let slot = |kind: &str| match kind {
        "person" => Some(0),
        "system" => Some(1),
        "container" => Some(2),
        "component" => Some(3),
        _ => None,
    };
    let data_slot = slot(name.split('_').next().unwrap_or_default());
    let (fill, ink) = data_slot.map_or_else(
        || {
            (
                theme.roles.surface_node.first_color(),
                theme.roles.text_primary,
            )
        },
        |index| (theme.data.fill(index), theme.data.fill_ink(index)),
    );
    let color = |key| {
        let path = format!("c4.{name}_{key}_color");
        config
            .get_str(&path)
            .filter(|_| config.explicit_config_owns_path(&path))
    };
    super::typography::C4ElementColors {
        background: color("bg").map_or_else(|| fill.to_string(), str::to_owned),
        border: color("border").map_or_else(
            || theme.roles.stroke_border.first_color().to_string(),
            str::to_owned,
        ),
        text: color("font").map_or_else(|| ink.to_string(), str::to_owned),
    }
}

pub(super) fn authored_font_family(config: &PresentationConfig) -> Option<String> {
    ["themeVariables.fontFamily", "fontFamily"]
        .into_iter()
        .filter(|path| config.explicit_config_owns_path(path))
        .filter_map(|path| config.get_str(path))
        .find_map(crate::config::normalized_config_font_family)
}

pub(super) fn font_stack(theme: &FamilyTheme, role: merman_theme_contract::v2::FontRole) -> String {
    font_family_css(theme.typography.stack(role))
}

use merman_theme_contract::v2::{DataPalette, Density, EdgeStyle, Elevation, Form};

use crate::theme::{Color, FamilyTheme};

use super::{FlowchartLayoutSettings, FlowchartShape};

#[derive(Debug)]
pub(crate) struct FlowchartNativeStyle {
    pub(crate) form: Form,
    pub(crate) elevation: Elevation,
    pub(crate) canvas: Color,
    pub(crate) node: Color,
    pub(crate) border: Color,
    pub(crate) container: Color,
    pub(crate) emphasis: Color,
    pub(crate) on_emphasis: Color,
    pub(crate) guide: Color,
    pub(super) callout: Color,
    on_callout: Color,
    data: DataPalette,
    node_ordinal_palette: bool,
}

impl FlowchartNativeStyle {
    pub(super) fn resolve(theme: &FamilyTheme, background: Option<Color>) -> Self {
        let roles = &theme.roles;
        Self {
            form: theme.form,
            elevation: theme.elevation.clone(),
            canvas: background.unwrap_or_else(|| roles.canvas.first_color()),
            node: roles.surface_node.first_color(),
            border: roles.stroke_border.first_color(),
            container: roles.surface_container.first_color(),
            emphasis: roles.surface_emphasis.first_color(),
            on_emphasis: roles.text_on_emphasis,
            guide: roles.stroke_guide.first_color(),
            callout: roles.surface_callout.first_color(),
            on_callout: roles.text_callout,
            data: theme.data.clone(),
            node_ordinal_palette: theme.node_ordinal_palette,
        }
    }

    pub(crate) fn node_colors(
        &self,
        shape: Option<FlowchartShape>,
        ordinal: Option<usize>,
        classes: &[String],
    ) -> Option<(Color, Color)> {
        let kind = classes
            .iter()
            .find_map(|class| class.strip_prefix("af-kind-"));
        let slot = match kind {
            Some("tool") => Some(0),
            Some("task") => Some(1),
            Some("input") => Some(2),
            Some("action") => Some(3),
            Some("connector") => Some(4),
            Some("decision") => return Some((self.emphasis, self.on_emphasis)),
            Some("refdoc") => return Some((self.callout, self.on_callout)),
            _ => None,
        };
        if let Some(slot) = slot {
            return Some((self.data.fill(slot), self.data.fill_ink(slot)));
        }
        match shape {
            Some(
                FlowchartShape::Stadium
                | FlowchartShape::Circle
                | FlowchartShape::DoubleCircle
                | FlowchartShape::Diamond
                | FlowchartShape::Choice
                | FlowchartShape::FramedCircle,
            ) => Some((self.emphasis, self.on_emphasis)),
            Some(FlowchartShape::Note) => Some((self.callout, self.on_callout)),
            Some(FlowchartShape::Anchor | FlowchartShape::Text | FlowchartShape::FilledCircle)
            | None => None,
            _ => ordinal
                .filter(|_| self.node_ordinal_palette)
                .map(|ordinal| {
                    let index = ordinal.saturating_sub(1);
                    (self.data.fill(index), self.data.fill_ink(index))
                }),
        }
    }

    pub(crate) fn has_depth(&self) -> bool {
        !matches!(self.elevation, Elevation::Flat)
    }
}

pub(crate) fn default_curve(
    edge_style: EdgeStyle,
    model: &super::FlowchartModel,
    config: &merman_model::presentation::PresentationConfig,
) -> Option<&'static str> {
    if config.explicit_config_owns_path("flowchart.curve")
        || model
            .edge_defaults
            .as_ref()
            .is_some_and(|defaults| defaults.interpolate.is_some())
    {
        return None;
    }
    Some(match edge_style {
        EdgeStyle::Curved => "basis",
        EdgeStyle::Orthogonal => "step",
        EdgeStyle::Straight | EdgeStyle::Sketch => "linear",
    })
}

pub(crate) fn density_scales(density: Density) -> (f64, f64) {
    match density {
        Density::Compact => (0.85, 0.85),
        Density::Balanced => (1.0, 1.0),
        Density::Airy => (1.25, 1.2),
    }
}

pub(crate) fn apply_density(settings: &mut FlowchartLayoutSettings, density: Density) {
    let (padding, spacing) = density_scales(density);
    settings.node_padding *= padding;
    settings.nodesep *= spacing;
    settings.ranksep *= spacing;
}

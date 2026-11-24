use merman_theme_contract::v2::{DataPalette, Density, EdgeStyle, Elevation, Form};

use crate::DiagramFamilyId;
use crate::theme::{Color, FamilyTheme, FamilyThemeAdapter, ThemeCapabilityFlags};

use super::{FlowchartCompatibilityBinding, FlowchartLayoutSettings, FlowchartShape};

const CAPS: ThemeCapabilityFlags = ThemeCapabilityFlags::CANVAS
    .union(ThemeCapabilityFlags::NODE_SURFACE)
    .union(ThemeCapabilityFlags::CONTAINER)
    .union(ThemeCapabilityFlags::CALLOUT)
    .union(ThemeCapabilityFlags::EMPHASIS)
    .union(ThemeCapabilityFlags::CONNECTOR)
    .union(ThemeCapabilityFlags::DATA_FILL)
    .union(ThemeCapabilityFlags::ELEVATION)
    .union(ThemeCapabilityFlags::RADIUS)
    .union(ThemeCapabilityFlags::MARKER_STYLE)
    .union(ThemeCapabilityFlags::TYPOGRAPHY);

pub(crate) struct FlowchartThemeAdapter;
pub(crate) struct AgentflowThemeAdapter;
pub(crate) struct SwimlaneThemeAdapter;

impl FamilyThemeAdapter for FlowchartThemeAdapter {
    const FAMILY: DiagramFamilyId = DiagramFamilyId::FLOWCHART;
    const CAPS: ThemeCapabilityFlags = CAPS.union(ThemeCapabilityFlags::EDGE_STYLE);
}

impl FamilyThemeAdapter for AgentflowThemeAdapter {
    const FAMILY: DiagramFamilyId = DiagramFamilyId::AGENTFLOW;
    const CAPS: ThemeCapabilityFlags = FlowchartThemeAdapter::CAPS;
}

impl FamilyThemeAdapter for SwimlaneThemeAdapter {
    const FAMILY: DiagramFamilyId = DiagramFamilyId::SWIMLANE;
    const CAPS: ThemeCapabilityFlags = CAPS;
}

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
    callout: Color,
    on_callout: Color,
    data: DataPalette,
    node_ordinal_palette: bool,
}

impl FlowchartNativeStyle {
    pub(crate) fn resolve(
        theme: &FamilyTheme,
        background: Option<Color>,
        defaults: &mut FlowchartCompatibilityBinding,
    ) -> Self {
        let roles = &theme.roles;
        let canvas = background.unwrap_or_else(|| roles.canvas.first_color());
        let container = roles.surface_container.first_color();
        let emphasis = roles.surface_emphasis.first_color();
        let callout = roles.surface_callout.first_color();
        defaults.text_color = roles.text_primary.to_string();
        defaults.node_text_color = defaults.text_color.clone();
        defaults.title_color = roles.text_title.to_string();
        defaults.line_color = roles.stroke_connector.first_color().to_string();
        defaults.arrowhead_color = defaults.line_color.clone();
        defaults.node_border = roles.stroke_border.first_color().to_string();
        defaults.main_bkg = roles.surface_node.first_color().to_string();
        defaults.cluster_bkg = container.to_string();
        defaults.cluster_border = defaults.node_border.clone();
        defaults.border2 = defaults.cluster_border.clone();
        defaults.agentflow_container_stroke = defaults.cluster_border.clone();
        defaults.collapsed_agentflow_stroke = defaults.cluster_border.clone();
        defaults.tertiary = emphasis.to_string();
        defaults.note_fill = callout.to_string();
        defaults.note_stroke = defaults.node_border.clone();
        defaults.state_inner_fill = defaults.arrowhead_color.clone();
        defaults.error_bkg = roles.status_danger.to_string();
        defaults.error_text = roles.text_callout.to_string();
        defaults.node_stroke_width = theme.form.stroke_width as f32;
        defaults.stroke_width = theme.form.stroke_width.to_string();
        defaults
            .neo
            .bind_native_node_stroke(&defaults.node_border, &defaults.stroke_width);
        defaults.connector_stroke_width = Some(theme.form.connector_width as f32);
        defaults.edge_label_background = "#00000000".to_owned();
        defaults.drop_shadow = "none".to_owned();
        defaults.node_corner_radius = theme.form.radius;
        defaults.rounded_rect_radius = serde_json::json!(theme.form.radius);
        defaults.rounded_rect_radius_numeric = Some(theme.form.radius);
        defaults.rounded_rect_radius_truthy = theme.form.radius > 0.0;
        defaults.root_font_family =
            crate::theme::font_family_css(theme.typography.stack(theme.typography.body_family));
        defaults.root_font_weight = Some(theme.typography.label_weight);
        Self {
            form: theme.form.clone(),
            elevation: theme.elevation.clone(),
            canvas,
            node: roles.surface_node.first_color(),
            border: roles.stroke_border.first_color(),
            container,
            emphasis,
            on_emphasis: roles.text_on_emphasis,
            guide: roles.stroke_guide.first_color(),
            callout,
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

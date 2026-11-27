//! Flowchart render configuration preparation.

use merman_model::{families::flowchart::FlowchartModel, presentation::PresentationConfig};
use merman_render_core::text::{TextStyle, WrapMode};

use super::{
    FlowchartBaseTypographyPlan, FlowchartBaseTypographyStyles, FlowchartConfigView, density_scales,
};
use crate::theme::{FlowchartCompatibilityBinding, FlowchartNativeStyle, default_curve};

#[derive(Debug)]
pub struct FlowchartRenderConfig {
    pub font_family: String,
    pub font_size: f64,
    pub wrapping_width: f64,
    pub node_html_labels: bool,
    pub edge_html_labels: bool,
    pub swimlane_title_html_labels: bool,
    pub node_wrap_mode: WrapMode,
    pub edge_wrap_mode: WrapMode,
    pub diagram_padding: f64,
    pub use_max_width: bool,
    pub title_top_margin: f64,
    pub node_padding: f64,
    pub text_style: TextStyle,
    pub html_label_text_style: TextStyle,
    pub default_edge_interpolate: String,
    pub default_edge_style: Vec<String>,
    pub node_border_color: String,
    pub node_stroke_width: f32,
    pub node_stroke_width_config_override: bool,
    pub cluster_fill_color: String,
    pub cluster_stroke_color: String,
    pub node_corner_radius_config_override: bool,
    pub edge_corner_radius: f64,
}

impl FlowchartRenderConfig {
    pub fn apply_native(
        &mut self,
        native: &FlowchartNativeStyle,
        model: &FlowchartModel,
        config: &PresentationConfig,
        uses_elk: bool,
    ) {
        self.node_padding *= density_scales(native.form.density).0;
        self.cluster_fill_color = native.container.to_string();
        if !uses_elk && let Some(curve) = default_curve(native.form.edge_style, model, config) {
            self.default_edge_interpolate = curve.to_owned();
        }
        if native.form.radius > 0.0 {
            self.edge_corner_radius = native.form.radius.clamp(6.0, 12.0);
        }
    }
}

pub fn prepare_flowchart_render_config(
    model: &FlowchartModel,
    effective_config: &PresentationConfig,
    compatibility: &FlowchartCompatibilityBinding,
    is_elk_layout: bool,
    base_typography: Option<&FlowchartBaseTypographyPlan>,
) -> FlowchartRenderConfig {
    let config = FlowchartConfigView::new(effective_config);
    let typography = base_typography.map_or_else(
        || {
            let font_family = config.font_family();
            let font_size = config.render_font_size();
            let text_style = config.render_text_style(&font_family, font_size);
            let html_label_text_style = config.html_label_measurement_base_style(&text_style);
            FlowchartBaseTypographyStyles {
                font_family,
                font_size,
                text_style,
                html_label_text_style,
            }
        },
        |plan| plan.render_styles(effective_config),
    );
    let FlowchartBaseTypographyStyles {
        font_family,
        font_size,
        text_style,
        html_label_text_style,
    } = typography;
    let wrapping_width = config.render_wrapping_width();
    let node_html_labels = config.node_html_labels();
    let edge_html_labels = config.effective_html_labels();
    let swimlane_title_html_labels = config.swimlane_title_html_labels();
    let node_wrap_mode = config.node_wrap_mode();
    let edge_wrap_mode = config.edge_wrap_mode();
    let diagram_padding = config.render_diagram_padding();
    let use_max_width = config.render_use_max_width();
    let title_top_margin = config.render_title_top_margin();
    let node_padding = config.render_node_padding();
    let default_edge_interpolate = config
        .effective_edge_interpolation(model, is_elk_layout)
        .to_owned();
    let default_edge_style = model
        .edge_defaults
        .as_ref()
        .map(|d| d.style.clone())
        .unwrap_or_default();

    let node_border_color = compatibility.node_border.clone();
    let node_stroke_width = compatibility.node_stroke_width;
    let node_stroke_width_config_override = compatibility.stroke_width_configured
        && effective_config.explicit_config_owns_path("themeVariables.strokeWidth");
    let agentflow = model.keyword == "agentflow-beta";
    let cluster_fill_color = if agentflow {
        "none".to_owned()
    } else {
        compatibility.cluster_bkg.clone()
    };
    let cluster_stroke_color = if agentflow {
        compatibility.agentflow_container_stroke.clone()
    } else {
        compatibility.cluster_border.clone()
    };
    let node_corner_radius_config_override = compatibility.node_corner_radius_configured
        && effective_config.explicit_config_owns_path("themeVariables.radius");
    let edge_corner_radius = 5.0;

    FlowchartRenderConfig {
        font_family,
        font_size,
        wrapping_width,
        node_html_labels,
        edge_html_labels,
        swimlane_title_html_labels,
        node_wrap_mode,
        edge_wrap_mode,
        diagram_padding,
        use_max_width,
        title_top_margin,
        node_padding,
        text_style,
        html_label_text_style,
        default_edge_interpolate,
        default_edge_style,
        node_border_color,
        node_stroke_width,
        node_stroke_width_config_override,
        cluster_fill_color,
        cluster_stroke_color,
        node_corner_radius_config_override,
        edge_corner_radius,
    }
}

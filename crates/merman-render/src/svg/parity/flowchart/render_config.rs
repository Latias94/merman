//! Flowchart render configuration preparation.

use crate::flowchart::FlowchartConfigView;
use crate::text::{TextStyle, WrapMode};

pub(in crate::svg::parity::flowchart) struct FlowchartRenderConfig {
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
    pub node_fill_color: String,
    pub node_stroke_width: f32,
    pub node_typography_config_ownership: crate::flowchart::FlowchartTypographyConfigOwnership,
    pub node_label_fill_config_override: bool,
    pub node_border_config_override: bool,
    pub node_fill_config_override: bool,
    pub node_stroke_width_config_override: bool,
    pub edge_stroke_config_override: bool,
    pub cluster_fill_color: String,
    pub cluster_stroke_color: String,
    pub cluster_fill_config_override: bool,
    pub cluster_stroke_config_override: bool,
    pub node_corner_radius: f64,
    pub node_corner_radius_config_override: bool,
    pub edge_corner_radius: f64,
    pub edge_label_padding: crate::flowchart::FlowchartEdgeLabelPadding,
    pub compact_edge_corners: bool,
}

pub(in crate::svg::parity::flowchart) fn prepare_flowchart_render_config(
    model: &crate::flowchart::FlowchartModel,
    effective_config: &merman_core::MermaidConfig,
    compatibility: &crate::flowchart::FlowchartCompatibilityBinding,
    is_elk_layout: bool,
    base_typography: Option<&crate::flowchart::FlowchartBaseTypographyPlan>,
    edge_label_padding: crate::flowchart::FlowchartEdgeLabelPadding,
) -> FlowchartRenderConfig {
    let effective_config_value = effective_config.as_value();
    let config = FlowchartConfigView::new(effective_config_value);
    let typography = base_typography.map_or_else(
        || {
            let font_family = config.font_family();
            let font_size = config.render_font_size();
            let text_style = config.render_text_style(&font_family, font_size);
            let html_label_text_style = config.html_label_measurement_base_style(&text_style);
            crate::flowchart::FlowchartBaseTypographyStyles {
                font_family,
                font_size,
                text_style,
                html_label_text_style,
            }
        },
        |plan| plan.render_styles(effective_config_value),
    );
    let crate::flowchart::FlowchartBaseTypographyStyles {
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
    let cfg_curve = if is_elk_layout {
        Some("rounded".to_string())
    } else {
        config.render_curve()
    };
    let default_edge_interpolate = model
        .edge_defaults
        .as_ref()
        .and_then(|d| d.interpolate.as_deref())
        .or(cfg_curve.as_deref())
        .unwrap_or("basis")
        .to_string();
    let default_edge_style = model
        .edge_defaults
        .as_ref()
        .map(|d| d.style.clone())
        .unwrap_or_default();

    let node_border_color = compatibility.node_border.clone();
    let node_fill_color = compatibility.main_bkg.clone();
    let node_stroke_width = compatibility.node_stroke_width;
    let node_typography_config_ownership =
        crate::flowchart::flowchart_typography_config_ownership(effective_config);
    let node_label_fill_config_override =
        flowchart_node_label_fill_config_override(effective_config);
    let node_border_config_override = merman_core::__private::config_path_overrides_typed_default(
        effective_config,
        "themeVariables.nodeBorder",
    );
    let node_fill_config_override = merman_core::__private::config_path_overrides_typed_default(
        effective_config,
        "themeVariables.mainBkg",
    );
    let node_stroke_width_config_override =
        merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.strokeWidth",
        );
    let edge_stroke_config_override = merman_core::__private::config_path_overrides_typed_default(
        effective_config,
        "themeVariables.lineColor",
    );
    let cluster_fill_color = compatibility.cluster_bkg.clone();
    let cluster_stroke_color = compatibility.cluster_border.clone();
    let cluster_fill_config_override = merman_core::__private::config_path_overrides_typed_default(
        effective_config,
        "themeVariables.clusterBkg",
    );
    let cluster_stroke_config_override =
        merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.clusterBorder",
        );
    let node_corner_radius = compatibility.node_corner_radius;
    let node_corner_radius_config_override =
        merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.radius",
        );
    let edge_corner_radius = 5.0;
    let compact_edge_corners = false;

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
        node_fill_color,
        node_stroke_width,
        node_typography_config_ownership,
        node_label_fill_config_override,
        node_border_config_override,
        node_fill_config_override,
        node_stroke_width_config_override,
        edge_stroke_config_override,
        cluster_fill_color,
        cluster_stroke_color,
        cluster_fill_config_override,
        cluster_stroke_config_override,
        node_corner_radius,
        node_corner_radius_config_override,
        edge_corner_radius,
        edge_label_padding,
        compact_edge_corners,
    }
}

pub(crate) fn flowchart_node_label_fill_config_override(
    effective_config: &merman_core::MermaidConfig,
) -> bool {
    const NODE_TEXT_COLOR: &str = "themeVariables.nodeTextColor";
    const PRIMARY_TEXT_COLOR: &str = "themeVariables.primaryTextColor";
    const TEXT_COLOR: &str = "themeVariables.textColor";

    // Mermaid's Flowchart stylesheet resolves this terminal as
    // `nodeTextColor || textColor`. `primaryTextColor` only owns the terminal when the
    // materialized `nodeTextColor` is its direct derived value.
    let theme_variables = effective_config
        .as_value()
        .get("themeVariables")
        .and_then(serde_json::Value::as_object);
    let node_text_color = theme_variables
        .and_then(|variables| variables.get("nodeTextColor"))
        .and_then(serde_json::Value::as_str)
        .filter(|color| !color.is_empty());

    let owns =
        |path| merman_core::__private::config_path_overrides_typed_default(effective_config, path);
    let Some(node_text_color) = node_text_color else {
        return owns(TEXT_COLOR);
    };
    if owns(NODE_TEXT_COLOR) {
        return true;
    }

    let theme = effective_config
        .get_str("theme")
        .and_then(|name| merman_core::MermaidThemeId::parse(name).ok())
        .unwrap_or_default();
    if !matches!(
        theme,
        merman_core::MermaidThemeId::Default | merman_core::MermaidThemeId::Base
    ) {
        return false;
    }

    theme_variables
        .and_then(|variables| variables.get("primaryTextColor"))
        .and_then(serde_json::Value::as_str)
        .filter(|primary_text_color| *primary_text_color == node_text_color)
        .is_some_and(|_| owns(PRIMARY_TEXT_COLOR))
}

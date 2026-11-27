use merman_render_flow::layout_config as config;
pub(crate) use merman_render_flow::theme as native_theme;
use merman_render_flow::{class_style, style};
#[cfg(test)]
mod bpmn_layout_tests;
#[cfg(feature = "layout-dagre")]
pub(crate) mod dagre;
#[cfg(feature = "layout-elk")]
pub mod elk;
use crate::graph_label as label;
#[cfg(feature = "layout-shoal")]
use merman_render_flow::measured;
#[cfg(feature = "layout-shoal")]
pub(crate) mod shoal;

pub(crate) use merman_model::families::flowchart::{FlowchartModel, FlowchartRenderContext};

pub(crate) use class_style::FlowchartClassDefinitions;
#[cfg(feature = "diagram-flowchart")]
pub(crate) use class_style::FlowchartLabelDom;
pub(crate) use merman_render_flow::projection::{
    FlowchartEdgeKey, FlowchartEdgeOwners, FlowchartEdgeTransportPlan, FlowchartRenderModelRef,
    ProjectedFlowchartEdge, project_flowchart_edge,
};
pub(crate) use native_theme::FlowchartPreparedTheme;

#[cfg(feature = "layout-shoal")]
pub(crate) use config::FlowchartLayoutSettings;
pub(crate) use config::{FlowchartBaseTypographyPlan, FlowchartConfigView};
#[cfg(all(test, feature = "all-diagrams"))]
pub(crate) use label::FlowchartSvgWidthMode;
#[cfg(any(feature = "diagram-flowchart", feature = "layout-shoal", test))]
pub(crate) use label::flowchart_label_is_empty_for_render;
#[cfg(test)]
pub(crate) use label::{FlowchartLabelMetricsRequest, flowchart_label_metrics_for_layout};
#[cfg(test)]
pub(crate) use merman_render_core::text::{
    flowchart_wrap_svg_source_word_lines,
    literal_label::flowchart_non_markdown_svg_source_word_lines,
};
#[cfg(all(test, feature = "layout-elk"))]
pub(crate) use merman_render_flow::node_geometry::flowchart_node_label_min_width;
#[cfg(any(feature = "layout-shoal", all(test, feature = "diagram-flowchart")))]
pub(crate) use merman_render_flow::node_geometry::flowchart_node_render_dimensions;
#[cfg(feature = "layout-shoal")]
pub(crate) use merman_render_flow::node_geometry::{
    resolve_flowchart_shape, validate_flowchart_node_asset_dimensions,
};
#[cfg(any(feature = "diagram-flowchart", feature = "layout-shoal", test))]
pub(crate) use merman_render_flow::shapes::FlowchartShape;
#[cfg(test)]
pub(crate) use merman_render_flow::svg_label::FlowchartPreparedLabelResolution;
#[cfg(any(feature = "layout-shoal", test))]
pub(crate) use merman_render_flow::svg_label::FlowchartSvgLabelOwner;
#[cfg(all(test, feature = "all-diagrams"))]
pub(crate) use merman_render_flow::svg_label::FlowchartSvgLabelRenderPlan;
pub(crate) use merman_render_flow::svg_label::{
    FlowchartSvgLabelSidecar, FlowchartSvgLabelSidecarBuilder,
};
#[cfg(all(
    test,
    feature = "diagram-flowchart",
    feature = "diagram-agentflow",
    feature = "layout-shoal"
))]
pub(crate) use style::flowchart_effective_text_style_for_node_classes;
#[cfg(all(test, feature = "all-diagrams"))]
pub(crate) use style::flowchart_swimlane_label_rect_text_style;
#[cfg(feature = "diagram-flowchart")]
pub(crate) use style::{
    flowchart_effective_node_class_names, flowchart_insert_style_by_spelling,
    flowchart_split_mermaid_style_decls,
};

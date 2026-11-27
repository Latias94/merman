pub(crate) use merman_render_flow::bpmn;
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
#[cfg(any(feature = "layout-elk", feature = "layout-shoal"))]
mod measured;
#[cfg(feature = "layout-shoal")]
pub(crate) mod shoal;

pub(crate) use merman_model::families::flowchart::{
    FlowEdge, FlowNode, FlowSubgraph, FlowchartModel, FlowchartRenderContext,
};

pub(crate) use class_style::{
    FlowchartClassDefinitions, FlowchartLabelDom, FlowchartStructuralLabelTypography,
};
pub(crate) use merman_render_flow::lane_title::zero_width_lane_title_rows;
pub(crate) use merman_render_flow::projection::{
    FlowchartEdgeKey, FlowchartEdgeOwners, FlowchartEdgeTransportPlan, FlowchartRenderModelRef,
    ProjectedFlowchartEdge, project_flowchart_edge,
};
pub(crate) use native_theme::{FlowchartCompatibilityBinding, FlowchartPreparedTheme};

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) use config::FlowchartLayoutSettings;
pub(crate) use config::{FlowchartBaseTypographyPlan, FlowchartConfigView};
pub(crate) use label::{
    FlowchartLabelMetricsRequest, FlowchartSvgWidthMode, flowchart_label_is_empty_for_render,
    flowchart_label_metrics_for_layout, flowchart_label_plain_text_for_layout,
    flowchart_label_text_is_empty_for_mode,
};
#[cfg(test)]
pub(crate) use merman_render_core::text::{
    flowchart_wrap_svg_source_word_lines,
    literal_label::flowchart_non_markdown_svg_source_word_lines,
};
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) use merman_render_flow::node_geometry::NodeLayoutDimensionsRequest;
#[cfg(feature = "layout-elk")]
pub(crate) use merman_render_flow::node_geometry::validate_flowchart_model_shapes;
pub(crate) use merman_render_flow::node_geometry::{
    CROSSED_CIRCLE_RADIUS, DelayGeometry, DisplayGeometry, DoubleCircleGeometry,
    FlowchartRectangleGeometry, HexagonGeometry, ImageSquareGeometry, ImageSquareSizing,
    LeanGeometry, LeanKind, OddGeometry, PreparedFlowchartNodeGeometry, StadiumGeometry,
    flowchart_brace_content_dimensions, flowchart_node_label_min_width,
    flowchart_node_render_dimensions, flowchart_node_svg_width_mode,
    flowchart_stacked_document_geometry, resolve_flowchart_shape,
    validate_flowchart_node_asset_dimensions,
};
pub(crate) use merman_render_flow::projection::flowchart_dagre_merges_self_loop_segments;
pub(crate) use merman_render_flow::shapes::{FlowchartShape, bang_geometry, cloud_geometry};
#[cfg(any(feature = "layout-elk", feature = "layout-shoal"))]
pub(crate) use merman_render_flow::svg_label::measure_flowchart_svg_label_for_layout_with_metrics_and_source_style;
pub(crate) use merman_render_flow::svg_label::{
    FlowchartPreparedLabelResolution, FlowchartSvgLabelOwner, FlowchartSvgLabelRenderPlan,
    FlowchartSvgLabelSidecar, FlowchartSvgLabelSidecarBuilder,
    measure_flowchart_svg_label_for_layout_with_source_style,
};
#[cfg(any(feature = "layout-elk", feature = "layout-shoal"))]
pub(crate) use style::flowchart_apply_html_node_class_box_metrics;
#[cfg(all(
    test,
    feature = "diagram-flowchart",
    feature = "diagram-agentflow",
    feature = "layout-shoal"
))]
pub(crate) use style::flowchart_effective_text_style_for_node_classes;
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) use style::flowchart_node_layout_style;
#[cfg(all(test, feature = "all-diagrams"))]
pub(crate) use style::flowchart_swimlane_label_rect_text_style;
pub(crate) use style::{
    flowchart_effective_node_class_names,
    flowchart_effective_text_style_for_classes_with_provenance, flowchart_insert_style_by_spelling,
    flowchart_split_mermaid_style_decls,
};

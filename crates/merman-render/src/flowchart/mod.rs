pub(crate) use merman_render_flow::bpmn;
use merman_render_flow::layout_config as config;
pub(crate) use merman_render_flow::theme as native_theme;
use merman_render_flow::{class_style, style};
#[cfg(test)]
mod bpmn_layout_tests;
#[cfg(feature = "layout-dagre")]
pub(crate) mod dagre;
mod edge_occurrence;
#[cfg(feature = "layout-elk")]
pub mod elk;
use crate::graph_label as label;
mod image;
#[cfg(any(feature = "layout-elk", feature = "layout-shoal"))]
mod measured;
mod node;
mod self_loop;
mod shape_validation;
#[cfg(feature = "layout-shoal")]
pub(crate) mod shoal;
mod svg_label_artifact;

pub(crate) use merman_model::families::flowchart::{
    FlowEdge, FlowNode, FlowSubgraph, FlowchartModel, FlowchartRenderContext,
};

pub(crate) use class_style::{
    FlowchartClassDefinitions, FlowchartLabelDom, FlowchartStructuralLabelTypography,
};
pub(crate) use edge_occurrence::FlowchartEdgeOwners;
pub(crate) use merman_render_flow::lane_title::zero_width_lane_title_rows;
pub(crate) use merman_render_flow::projection::{
    FlowchartEdgeKey, FlowchartEdgeTransportPlan, FlowchartRenderModelRef, ProjectedFlowchartEdge,
    project_flowchart_edge,
};
pub(crate) use native_theme::{FlowchartCompatibilityBinding, FlowchartPreparedTheme};

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) use config::FlowchartLayoutSettings;
pub(crate) use config::{
    FLOWCHART_FIXED_LABEL_WRAP_WIDTH, FlowchartBaseTypographyPlan, FlowchartConfigView,
};
pub(crate) use image::{ImageSquareGeometry, ImageSquareSizing};
#[cfg(not(test))]
pub(crate) use label::flowchart_non_markdown_svg_source_word_lines;
pub(crate) use label::{
    FlowchartLabelMetricsRequest, FlowchartSvgWidthMode, flowchart_label_is_empty_for_render,
    flowchart_label_metrics_for_layout, flowchart_label_plain_text_for_layout,
    flowchart_label_text_is_empty_for_mode, flowchart_node_label_min_width,
    flowchart_node_svg_width_mode,
};
#[cfg(all(
    test,
    any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    )
))]
pub(crate) use label::{
    flowchart_non_markdown_svg_source_word_lines, flowchart_wrap_svg_source_word_lines,
};
pub(crate) use merman_render_flow::shapes::{
    FlowchartShape, OrganicShapeGeometry, RelativeArc, bang_geometry, cloud_geometry,
    is_flowchart_process_shape,
};
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) use node::NodeLayoutDimensionsRequest;
pub(crate) use node::{
    CROSSED_CIRCLE_RADIUS, DelayGeometry, DisplayGeometry, DoubleCircleGeometry, HexagonGeometry,
    LeanGeometry, LeanKind, OddGeometry, StadiumGeometry, flowchart_brace_content_dimensions,
    flowchart_node_render_dimensions, flowchart_stacked_document_geometry,
};
pub(crate) use node::{FlowchartRectangleGeometry, PreparedFlowchartNodeGeometry};
pub(crate) use self_loop::{
    flowchart_dagre_merges_self_loop_segments, flowchart_self_loop_helper_edges,
};
#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
pub(crate) use shape_validation::validate_flowchart_model_shapes;
pub(crate) use shape_validation::{
    resolve_flowchart_shape, validate_flowchart_node_asset_dimensions,
};
#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-shoal"
))]
pub(crate) use style::flowchart_apply_html_node_class_box_metrics;
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) use style::flowchart_node_layout_style;
#[cfg(all(test, feature = "all-diagrams"))]
pub(crate) use style::flowchart_swimlane_label_rect_text_style;
pub(crate) use style::{
    FlowchartTerminalForeground, FlowchartTerminalForegroundProvenance,
    FlowchartTextStyleResolution, flowchart_effective_node_class_names,
    flowchart_effective_text_style_for_classes,
    flowchart_effective_text_style_for_classes_with_provenance,
    flowchart_effective_text_style_for_node_classes, flowchart_insert_style_by_spelling,
    flowchart_is_source_spelled_label_style_key, flowchart_split_mermaid_style_decls,
};
#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-shoal"
))]
pub(crate) use svg_label_artifact::measure_flowchart_svg_label_for_layout_with_metrics_and_source_style;
pub(crate) use svg_label_artifact::{
    FlowchartNodeSourceTypography, FlowchartPreparedLabelResolution, FlowchartSvgLabelOwner,
    FlowchartSvgLabelRenderPlan, FlowchartSvgLabelSidecar, FlowchartSvgLabelSidecarBuilder,
    measure_flowchart_svg_label_for_layout_with_source_style,
};

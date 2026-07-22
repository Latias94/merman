mod config;
#[cfg(feature = "layout-elk")]
pub mod elk;
mod label;
mod layout;
mod node;
mod self_loop;
mod shapes;
mod style;

pub(crate) use merman_core::diagrams::flowchart::{
    FlowEdge, FlowNode, FlowSubgraph, FlowchartModel,
};

pub use layout::layout_flowchart_typed;

pub(crate) use config::{FlowchartConfigView, FlowchartLayoutSettings};
pub(crate) use label::{
    FlowchartLabelMetricsRequest, flowchart_decode_label_escapes,
    flowchart_label_metrics_for_layout, flowchart_label_plain_text_for_layout,
    flowchart_normalize_plain_multiline_label_for_html,
};
pub(crate) use node::{
    NodeLayoutDimensionsRequest, flowchart_node_render_dimensions, node_layout_dimensions,
};
pub(crate) use self_loop::flowchart_self_loop_helper_edges;
pub(crate) use shapes::{
    FlowchartShape, OrganicShapeGeometry, RelativeArc, bang_geometry, cloud_geometry,
    is_flowchart_process_shape, validate_flowchart_model_shapes,
};
pub(crate) use style::{
    flowchart_apply_html_node_class_box_metrics, flowchart_effective_node_class_names,
    flowchart_effective_text_style_for_classes, flowchart_effective_text_style_for_node_classes,
    flowchart_split_mermaid_style_decls,
};

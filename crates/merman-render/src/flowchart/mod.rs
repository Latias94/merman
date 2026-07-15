mod config;
#[cfg(feature = "elk-layout")]
pub mod elk;
mod label;
mod layout;
mod node;
mod self_loop;
mod style;

pub(crate) use merman_core::diagrams::flowchart::{
    FlowEdge, FlowNode, FlowSubgraph, FlowchartModel,
};

pub use layout::layout_flowchart_typed;

pub(crate) use config::FlowchartConfigView;
pub(crate) use label::{
    FlowchartLabelMetricsRequest, flowchart_decode_label_escapes,
    flowchart_label_metrics_for_layout, flowchart_label_plain_text_for_layout,
    flowchart_normalize_plain_multiline_label_for_html,
    flowchart_whole_label_font_style_requests_italic,
};
pub(crate) use node::flowchart_node_render_dimensions;
pub(crate) use self_loop::flowchart_self_loop_helper_edges;
pub(crate) use style::{
    flowchart_effective_font_style_for_classes, flowchart_effective_font_style_for_node_classes,
    flowchart_effective_node_class_names, flowchart_effective_text_style_for_classes,
    flowchart_effective_text_style_for_node_classes, flowchart_node_has_span_css_height_parity,
    flowchart_split_mermaid_style_decls,
};

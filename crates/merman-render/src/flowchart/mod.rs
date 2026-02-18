#![allow(clippy::too_many_arguments)]

mod label;
mod layout;
mod node;
mod style;

pub(crate) type FlowchartV2Model = merman_core::diagrams::flowchart::FlowchartV2Model;
pub(crate) type FlowNode = merman_core::diagrams::flowchart::FlowNode;
pub(crate) type FlowEdge = merman_core::diagrams::flowchart::FlowEdge;
pub(crate) type FlowSubgraph = merman_core::diagrams::flowchart::FlowSubgraph;

pub use layout::{layout_flowchart_v2, layout_flowchart_v2_typed};

pub(crate) use label::{flowchart_label_metrics_for_layout, flowchart_label_plain_text_for_layout};
pub(crate) use node::flowchart_node_render_dimensions;
pub(crate) use style::flowchart_effective_text_style_for_classes;

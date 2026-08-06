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
    FlowEdge, FlowNode, FlowSubgraph, FlowchartModel, FlowchartRenderLabelSources,
};
use std::ops::Deref;

#[derive(Debug, Clone, Copy)]
pub(crate) struct FlowchartRenderModelRef<'a> {
    semantic: &'a FlowchartModel,
    label_sources: &'a FlowchartRenderLabelSources,
}

impl<'a> FlowchartRenderModelRef<'a> {
    pub(crate) const fn new(
        semantic: &'a FlowchartModel,
        label_sources: &'a FlowchartRenderLabelSources,
    ) -> Self {
        Self {
            semantic,
            label_sources,
        }
    }

    pub(crate) fn node_label_for_render<'b>(&'b self, node: &'b FlowNode) -> Option<&'b str> {
        self.label_sources.node_label_for_render(node)
    }

    pub(crate) fn edge_label_for_render<'b>(&'b self, edge: &'b FlowEdge) -> Option<&'b str> {
        self.label_sources.edge_label_for_render(edge)
    }

    pub(crate) fn subgraph_title_for_render<'b>(&'b self, subgraph: &'b FlowSubgraph) -> &'b str {
        self.label_sources.subgraph_title_for_render(subgraph)
    }

    pub(crate) fn requires_math(&self) -> bool {
        self.nodes
            .iter()
            .filter_map(|node| self.node_label_for_render(node))
            .chain(
                self.edges
                    .iter()
                    .filter_map(|edge| self.edge_label_for_render(edge)),
            )
            .chain(
                self.subgraphs
                    .iter()
                    .map(|subgraph| self.subgraph_title_for_render(subgraph)),
            )
            .any(crate::math::contains_delimited_math)
    }
}

impl Deref for FlowchartRenderModelRef<'_> {
    type Target = FlowchartModel;

    fn deref(&self) -> &Self::Target {
        self.semantic
    }
}

pub(crate) use layout::layout_flowchart_typed_with_render_labels_and_work_meter;

pub(crate) use config::{FlowchartConfigView, FlowchartLayoutSettings};
pub(crate) use label::{
    FlowchartLabelMetricsRequest, FlowchartSvgWidthMode, PreparedFlowchartSvgLabel,
    flowchart_label_is_empty_for_render, flowchart_label_metrics_for_layout,
    flowchart_label_plain_text_for_layout, flowchart_label_text_is_empty_for_mode,
    flowchart_non_markdown_label_for_html, flowchart_non_markdown_svg_source_word_lines,
    flowchart_svg_source_word_lines_plain_text, flowchart_trim_html_collapsible_whitespace,
    flowchart_wrap_svg_source_word_lines,
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

mod base_typography;
mod compatibility;
mod config;
mod edge_label_padding;
mod edge_occurrence;
#[cfg(feature = "layout-elk")]
pub mod elk;
use crate::graph_label as label;
mod layout;
mod node;
mod self_loop;
mod shapes;
mod style;
mod svg_label_artifact;
mod theme_evidence;

pub(crate) use merman_core::diagrams::flowchart::{
    FlowEdge, FlowNode, FlowSubgraph, FlowchartModel, FlowchartRenderContext,
};
use std::ops::Deref;

pub(crate) use compatibility::{
    FlowchartCompatibilityBinding, FlowchartEffectEligibility, FlowchartPreparedTheme,
};
pub(crate) use edge_label_padding::FlowchartEdgeLabelPadding;
pub(crate) use edge_occurrence::{
    FlowchartEdgeKey, FlowchartEdgeOwners, FlowchartEdgeTransportPlan,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct FlowchartRenderModelRef<'a> {
    semantic: &'a FlowchartModel,
    render_context: &'a FlowchartRenderContext,
}

impl<'a> FlowchartRenderModelRef<'a> {
    pub(crate) const fn new(
        semantic: &'a FlowchartModel,
        render_context: &'a FlowchartRenderContext,
    ) -> Self {
        Self {
            semantic,
            render_context,
        }
    }

    pub(crate) fn node_label_for_render<'b>(&'b self, node: &'b FlowNode) -> Option<&'b str> {
        self.render_context.node_label_for_render(node)
    }

    pub(crate) fn edge_label_for_render<'b>(
        &'b self,
        semantic_index: usize,
        edge: &'b FlowEdge,
    ) -> Option<&'b str> {
        self.render_context
            .edge_label_for_render(semantic_index, edge)
    }

    pub(crate) fn subgraph_title_for_render<'b>(
        &'b self,
        declaration_ordinal: usize,
        subgraph: &'b FlowSubgraph,
    ) -> &'b str {
        self.render_context
            .subgraph_title_for_render(declaration_ordinal, subgraph)
    }

    pub(crate) fn effective_subgraph_css<'b>(
        &'b self,
        declaration_ordinal: usize,
        subgraph: &'b FlowSubgraph,
    ) -> (&'b [String], &'b [String]) {
        self.render_context
            .effective_subgraph_css(declaration_ordinal, subgraph)
    }

    pub(crate) fn is_subgraph_collapsed(&self, id: &str) -> bool {
        self.render_context.is_subgraph_collapsed(id)
    }

    pub(crate) fn collapsed_replacement(&self, id: &str) -> Option<&str> {
        self.render_context.collapsed_replacement(id)
    }

    pub(crate) fn subgraph_color_ordinal(&self, id: &str) -> Option<usize> {
        self.render_context.subgraph_color_ordinal(id)
    }

    pub(crate) fn requires_math(&self) -> bool {
        self.nodes
            .iter()
            .filter_map(|node| self.node_label_for_render(node))
            .chain(
                self.edges
                    .iter()
                    .enumerate()
                    .filter_map(|(index, edge)| self.edge_label_for_render(index, edge)),
            )
            .chain(
                self.subgraphs
                    .iter()
                    .enumerate()
                    .map(|(ordinal, subgraph)| self.subgraph_title_for_render(ordinal, subgraph)),
            )
            .any(crate::math::contains_delimited_math)
    }
}

/// Projects a logical Flowchart edge through Mermaid's collapsed-subgraph view. The typed model
/// remains lossless; render/layout callers use this helper to hide fully internal edges and route
/// boundary edges to the visible collapsed node.
pub(crate) fn project_flowchart_edge_endpoints<'a>(
    edge: &'a FlowEdge,
    render_context: &'a FlowchartRenderContext,
) -> Option<(&'a str, &'a str)> {
    let from = render_context
        .collapsed_replacement(edge.from.as_str())
        .unwrap_or(edge.from.as_str());
    let to = render_context
        .collapsed_replacement(edge.to.as_str())
        .unwrap_or(edge.to.as_str());
    let changed = from != edge.from || to != edge.to;
    if from == to && changed {
        return None;
    }
    Some((from, to))
}

pub(crate) fn project_flowchart_edge(
    edge: &FlowEdge,
    render_context: &FlowchartRenderContext,
) -> Option<FlowEdge> {
    let (from, to) = project_flowchart_edge_endpoints(edge, render_context)?;
    let changed = from != edge.from || to != edge.to;
    if !changed {
        return Some(edge.clone());
    }
    let mut projected = edge.clone();
    projected.from = from.to_string();
    projected.to = to.to_string();
    Some(projected)
}

impl Deref for FlowchartRenderModelRef<'_> {
    type Target = FlowchartModel;

    fn deref(&self) -> &Self::Target {
        self.semantic
    }
}

pub(crate) use layout::layout_flowchart_typed_with_render_labels_and_svg_label_sidecar_and_work_meter;

pub(crate) use base_typography::{
    FlowchartBaseTypographyLabelEmission, FlowchartBaseTypographyPlan,
    FlowchartBaseTypographyStyles,
};
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub(crate) use config::FlowchartLayoutSettings;
pub(crate) use config::{
    FLOWCHART_FIXED_LABEL_WRAP_WIDTH, FlowchartConfigView, FlowchartTypographyConfigOwnership,
    flowchart_typography_config_ownership,
};
#[cfg(not(test))]
pub(crate) use label::flowchart_non_markdown_svg_source_word_lines;
pub(crate) use label::{
    FlowchartLabelMetricsRequest, FlowchartSvgWidthMode, flowchart_label_is_empty_for_render,
    flowchart_label_metrics_for_layout, flowchart_label_plain_text_for_layout,
    flowchart_label_text_is_empty_for_mode, flowchart_node_label_min_width,
    flowchart_node_svg_width_mode, flowchart_non_markdown_label_for_html,
    flowchart_trim_html_collapsible_whitespace,
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
pub(crate) use node::{
    CROSSED_CIRCLE_RADIUS, DelayGeometry, DisplayGeometry, DoubleCircleGeometry, HexagonGeometry,
    ImageSquareGeometry, LeanGeometry, LeanKind, OddGeometry, StadiumGeometry,
    flowchart_brace_content_dimensions, flowchart_node_render_dimensions,
    flowchart_stacked_document_geometry,
};
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) use node::{NodeLayoutDimensionsRequest, node_layout_dimensions};
pub(crate) use self_loop::flowchart_self_loop_helper_edges;
pub(crate) use shapes::{
    FlowchartShape, OrganicShapeGeometry, RelativeArc, bang_geometry, cloud_geometry,
    is_flowchart_process_shape, validate_flowchart_model_shapes,
};
#[cfg(test)]
pub(crate) use style::flowchart_swimlane_label_rect_text_style;
pub(crate) use style::{
    FlowchartTerminalForeground, FlowchartTerminalForegroundProvenance,
    FlowchartTextStyleResolution, flowchart_apply_html_node_class_box_metrics,
    flowchart_apply_text_style_decl, flowchart_effective_node_class_names,
    flowchart_effective_text_style_for_classes,
    flowchart_effective_text_style_for_classes_with_provenance,
    flowchart_effective_text_style_for_node_classes,
    flowchart_effective_text_style_for_node_classes_with_provenance,
    flowchart_is_source_spelled_label_style_key, flowchart_split_mermaid_style_decls,
};
pub(crate) use svg_label_artifact::{
    FlowchartLabelTypographyOverrides, FlowchartPreparedMathResolution, FlowchartSvgLabelOwner,
    FlowchartSvgLabelRenderPlan, FlowchartSvgLabelSidecar, FlowchartSvgLabelSidecarBuilder,
    measure_flowchart_svg_label_for_layout_with_metrics_style_and_typography_overrides,
    measure_flowchart_svg_label_for_layout_with_typography_overrides,
};
pub(crate) use theme_evidence::{
    FlowchartClusterThemeEmission, FlowchartClusterThemePlan, FlowchartClusterThemeStyle,
    FlowchartEdgeLabelThemeEmission, FlowchartEdgeThemeEmission, FlowchartEdgeThemeStyle,
    FlowchartFacetPrecedence, FlowchartNodeThemeEmission, FlowchartNodeThemeStyle,
    FlowchartRadiusEmission, FlowchartShapeFacetEmissionReceipt, FlowchartSourceFacetStatus,
    FlowchartThemeEvidenceRecorder, FlowchartThemeFacetEmission,
};

mod text_paint;
pub(crate) use text_paint::{
    FlowchartTextPaintChannel, FlowchartTextPaintFacts, FlowchartTextPaintPlan,
};
mod text_surface_paint;
pub(crate) use text_surface_paint::FlowchartTextSurfacePaintPlan;

mod label_background;
pub(crate) use label_background::FlowchartLabelBackgroundPlan;

mod label_weight;
pub(crate) use label_weight::FlowchartLabelWeights;

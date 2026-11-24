mod base_typography;
pub(crate) mod bpmn;
mod class_style;
mod compatibility;
mod config;
pub(crate) mod dagre;
mod edge_label_padding;
mod edge_occurrence;
#[cfg(feature = "layout-elk")]
pub mod elk;
use crate::graph_label as label;
mod image;
mod lane_title;
#[cfg(any(feature = "layout-elk", feature = "layout-shoal"))]
mod measured;
mod node;
mod self_loop;
mod shapes;
#[cfg(feature = "layout-shoal")]
pub(crate) mod shoal;
mod style;
mod svg_label_artifact;
mod theme_evidence;

pub(crate) use merman_model::families::flowchart::{
    FlowEdge, FlowNode, FlowSubgraph, FlowchartModel, FlowchartRenderContext,
};
use std::borrow::Cow;
use std::ops::Deref;

pub(crate) use class_style::{
    FlowchartClassDefStyles, FlowchartClassDefinitions, FlowchartLabelDom,
    FlowchartStructuralLabelTypography,
};
pub(crate) use compatibility::{
    FlowchartCompatibilityBinding, FlowchartEffectEligibility, FlowchartPreparedTheme,
};
pub(crate) use edge_label_padding::FlowchartEdgeLabelPadding;
pub(crate) use edge_occurrence::{
    FlowchartEdgeKey, FlowchartEdgeOwners, FlowchartEdgeTransportPlan,
};
pub(crate) use lane_title::zero_width_lane_title_rows;

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

    pub(crate) fn edge_label_for_occurrence<'b>(
        &'b self,
        key: FlowchartEdgeKey,
        edge: &'b FlowEdge,
    ) -> Option<&'b str> {
        match key.self_loop_segment_order() {
            None | Some(1) => self.edge_label_for_render(key.semantic_index(), edge),
            Some(_) => None,
        }
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

    /// Parent relations in the graph sent to layout, in FlowDB insertion order.
    pub(crate) fn visible_memberships(self) -> impl Iterator<Item = (&'a str, &'a str)> {
        self.semantic
            .subgraphs
            .iter()
            .rev()
            .filter(move |group| {
                self.collapsed_replacement(&group.id).is_none()
                    && !self.is_subgraph_collapsed(&group.id)
            })
            .flat_map(|group| {
                group
                    .nodes
                    .iter()
                    .map(move |child| (child.as_str(), group.id.as_str()))
            })
    }

    pub(crate) fn projected_edges(
        self,
    ) -> impl Iterator<Item = (FlowchartEdgeKey, ProjectedFlowchartEdge<'a>)> + Clone {
        self.semantic
            .edges
            .iter()
            .enumerate()
            .filter_map(move |(index, edge)| {
                project_flowchart_edge(edge, self.render_context)
                    .map(|edge| (FlowchartEdgeKey::new(index), edge))
            })
    }

    pub(crate) fn requires_math(&self) -> bool {
        self.nodes.iter().any(|node| {
            self.node_label_for_render(node)
                .is_some_and(crate::math::contains_delimited_math)
                && self.collapsed_replacement(&node.id).is_none()
                && !self.is_subgraph_collapsed(&node.id)
        }) || self.edges.iter().enumerate().any(|(index, edge)| {
            self.edge_label_for_render(index, edge)
                .is_some_and(crate::math::contains_delimited_math)
                && project_flowchart_edge(edge, self.render_context).is_some()
        }) || self.subgraphs.iter().enumerate().any(|(index, subgraph)| {
            crate::math::contains_delimited_math(self.subgraph_title_for_render(index, subgraph))
                && self.collapsed_replacement(&subgraph.id).is_none()
        })
    }
}

pub(crate) struct ProjectedFlowchartEdge<'a> {
    pub(crate) semantic: &'a FlowEdge,
    pub(crate) from: &'a str,
    pub(crate) to: &'a str,
}

impl<'a> ProjectedFlowchartEdge<'a> {
    pub(crate) fn into_render_edge(self) -> Cow<'a, FlowEdge> {
        if self.from == self.semantic.from && self.to == self.semantic.to {
            return Cow::Borrowed(self.semantic);
        }

        let mut edge = self.semantic.clone();
        self.from.clone_into(&mut edge.from);
        self.to.clone_into(&mut edge.to);
        Cow::Owned(edge)
    }
}

/// Keeps the semantic edge intact while redirecting boundary edges to visible collapsed nodes.
/// Edges that collapse onto one endpoint are hidden, while unchanged self-loops survive.
pub(crate) fn project_flowchart_edge<'a>(
    edge: &'a FlowEdge,
    render_context: &'a FlowchartRenderContext,
) -> Option<ProjectedFlowchartEdge<'a>> {
    let from = render_context
        .collapsed_replacement(edge.from.as_str())
        .unwrap_or(edge.from.as_str());
    let to = render_context
        .collapsed_replacement(edge.to.as_str())
        .unwrap_or(edge.to.as_str());
    if from == to && (from != edge.from || to != edge.to) {
        return None;
    }
    Some(ProjectedFlowchartEdge {
        semantic: edge,
        from,
        to,
    })
}

impl Deref for FlowchartRenderModelRef<'_> {
    type Target = FlowchartModel;

    fn deref(&self) -> &Self::Target {
        self.semantic
    }
}

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
pub(crate) use shapes::{
    FlowchartShape, OrganicShapeGeometry, RelativeArc, bang_geometry, cloud_geometry,
    is_flowchart_process_shape, validate_flowchart_model_shapes,
    validate_flowchart_node_asset_dimensions,
};
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) use style::flowchart_node_layout_style;
#[cfg(test)]
pub(crate) use style::flowchart_swimlane_label_rect_text_style;
pub(crate) use style::{
    FlowchartRectangleCssSize, FlowchartTerminalForeground, FlowchartTerminalForegroundProvenance,
    FlowchartTextStyleResolution, flowchart_apply_html_node_class_box_metrics,
    flowchart_effective_node_class_names, flowchart_effective_text_style_for_classes,
    flowchart_effective_text_style_for_classes_with_provenance,
    flowchart_effective_text_style_for_node_classes, flowchart_insert_style_by_spelling,
    flowchart_is_source_spelled_label_style_key, flowchart_split_mermaid_style_decls,
};
pub(crate) use svg_label_artifact::{
    FlowchartNodeSourceTypography, FlowchartPreparedLabelResolution, FlowchartSvgLabelOwner,
    FlowchartSvgLabelRenderPlan, FlowchartSvgLabelSidecar, FlowchartSvgLabelSidecarBuilder,
    measure_flowchart_svg_label_for_layout_with_metrics_and_source_style,
    measure_flowchart_svg_label_for_layout_with_source_style,
};
pub(crate) use theme_evidence::{
    FlowchartClusterThemeEmission, FlowchartClusterThemeStyle, FlowchartEdgeLabelThemeEmission,
    FlowchartEdgeThemeEmission, FlowchartEdgeThemeStyle, FlowchartFacetPrecedence,
    FlowchartNodeThemeEmission, FlowchartNodeThemeStyle, FlowchartRadiusEmission,
    FlowchartShapeFacetEmissionReceipt, FlowchartSourceFacetStatus, FlowchartThemeEvidenceRecorder,
    FlowchartThemeFacetEmission,
};

mod text_paint;
pub(crate) use text_paint::{
    FlowchartTextPaintChannel, FlowchartTextPaintFacts, FlowchartTextPaintPlan,
};
mod text_surface_paint;
pub(crate) use text_surface_paint::FlowchartTextSurfacePaintPlan;

mod label_typography;
pub(crate) use label_typography::FlowchartLabelTypography;

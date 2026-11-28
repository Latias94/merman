//! Paint the prepared Flow graph in traversal order.

use crate::graph::{FlowchartClusterPass, FlowchartGraph, FlowchartGraphTimings, StepBadges};

use super::super::edge_fill::FlowchartEdgeFillPlan;
use super::super::paint::{FlowchartEdgeLabels, prepare_rough_edge_paths};
use super::super::render_input::FlowchartRenderEdgeRef;
use super::super::*;
use super::cluster::render_flowchart_cluster_pass;
use super::edge_label::render_swimlane_edge_label_node;
use super::node::render_flowchart_node_depth;
use crate::markers::FlowchartMarkerEmissionPlan;
use merman_render_core::svg_output::timing::RenderTiming;

pub(crate) struct FlowchartRootPaint<'a, 'data> {
    pub ctx: &'a FlowchartRenderCtx<'data>,
    pub timing: RenderTiming,
    pub details: &'a mut FlowchartRenderDetails,
    pub node_label_div_style: String,
    pub edge_cache:
        &'a mut FxHashMap<crate::projection::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    pub hierarchy_plan: &'a FlowchartHierarchyPlan<'data>,
    pub marker_plan: &'a FlowchartMarkerEmissionPlan<'a>,
    pub(in crate::svg) edge_fills: &'a FlowchartEdgeFillPlan,
    pub(in crate::svg) label_cutouts: &'a FxHashSet<crate::projection::FlowchartEdgeKey>,
    pub edge_labels: &'a mut FlowchartEdgeLabels,
    pub step_badges: &'a StepBadges<'a>,
    pub depth_filters: merman_render_core::theme_svg::elevation::DepthFilters,
}

impl FlowchartRootPaint<'_, '_> {
    pub(in crate::svg) fn write_graph(
        &mut self,
        out: &mut impl SvgOutput,
        graph_timings: &mut FlowchartGraphTimings,
    ) -> Result<()> {
        let ctx = self.ctx;
        let hierarchy = self.hierarchy_plan;
        let timing = self.timing;
        FlowchartGraph {
            hierarchy,
            model: ctx.model,
            work_meter: ctx.work_meter,
            markers: self.marker_plan,
            step_badges: self.step_badges,
            struck_group_titles: ctx.layout.route_contract.struck_group_titles(),
            subgraphs_by_id: &ctx.subgraphs_by_id,
            subgraph_ids_with_children: &ctx.subgraph_ids_with_children,
            recursive_clusters: &ctx.recursive_clusters,
            swimlane_label_edges: &ctx.swimlane_edge_label_edges_by_node_id,
            swimlane: ctx.swimlane_direction.is_some(),
            has_depth: ctx.native.is_some_and(|style| style.has_depth()),
            edge_html_labels: ctx.edge_html_labels,
            dagre_merges_self_loop_segments:
                crate::projection::flowchart_dagre_merges_self_loop_segments(&ctx.model.keyword),
        }
        .write(out, self, timing, graph_timings)
    }

    pub(crate) fn write_cluster(
        &self,
        out: &mut impl SvgOutput,
        cluster: &LayoutCluster,
        (x, y): (f64, f64),
        pass: FlowchartClusterPass,
    ) -> Result<()> {
        render_flowchart_cluster_pass(out, self.ctx, cluster, x, y, pass)
    }

    pub(crate) fn prepare_edges(&mut self, edges: &[FlowchartRenderEdgeRef<'_>]) -> Result<()> {
        prepare_rough_edge_paths(self.ctx, edges, self.edge_cache)?;
        if self.ctx.hand_drawn_seed.seed().may_use_math_random() {
            self.edge_labels
                .update(self.ctx, self.hierarchy_plan, self.edge_cache, edges)?;
        }
        Ok(())
    }

    pub(crate) fn write_edge(
        &self,
        out: &mut impl SvgOutput,
        edge: FlowchartRenderEdgeRef<'_>,
        (x, y): (f64, f64),
        scratch: &mut FlowchartEdgeDataPointsScratch,
    ) -> Result<()> {
        render_flowchart_edge_path(
            out,
            self.ctx,
            self.marker_plan,
            self.edge_fills,
            self.label_cutouts.contains(&edge.key),
            edge,
            x,
            y,
            scratch,
            self.edge_cache,
        )
    }

    pub(crate) fn write_edge_label(
        &self,
        out: &mut impl SvgOutput,
        edge: FlowchartRenderEdgeRef<'_>,
        (x, y): (f64, f64),
    ) -> Result<()> {
        render_flowchart_edge_label(
            out,
            self.ctx,
            edge,
            x,
            y,
            self.hierarchy_plan,
            self.edge_labels,
        )
    }

    #[cfg(feature = "layout-grid")]
    pub(crate) fn paints_grid_edge_label_nodes(&self) -> bool {
        self.ctx.layout.route_contract.grid_paint().is_some()
    }

    #[cfg(feature = "layout-grid")]
    pub(crate) fn write_grid_edge_label_nodes(
        &self,
        out: &mut impl SvgOutput,
        edges: &[FlowchartRenderEdgeRef<'_>],
    ) -> Result<()> {
        super::grid_label::write_grid_edge_label_nodes(out, self.ctx, edges, self.hierarchy_plan)
    }

    pub(crate) fn write_swimlane_edge_label(
        &self,
        out: &mut impl SvgOutput,
        id: &str,
        edge: FlowchartRenderEdgeRef<'_>,
        origin: (f64, f64),
    ) -> Result<()> {
        render_swimlane_edge_label_node(
            out,
            self.ctx,
            id,
            edge,
            origin,
            self.hierarchy_plan,
            self.edge_labels,
        )
    }

    pub(crate) fn write_node(
        &mut self,
        out: &mut impl SvgOutput,
        id: &str,
        (x, y): (f64, f64),
        depth: bool,
    ) -> Result<()> {
        if depth {
            render_flowchart_node_depth(out, self.ctx, id, x, y, &mut self.depth_filters)
        } else {
            render_flowchart_node(
                out,
                self.ctx,
                id,
                x,
                y,
                self.timing,
                self.details,
                &mut self.node_label_div_style,
            )
        }
    }
}

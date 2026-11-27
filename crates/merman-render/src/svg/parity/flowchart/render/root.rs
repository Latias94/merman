//! Flow graph paint adapter for facade-owned services.

use merman_render_flow::graph::{
    FlowchartClusterPass, FlowchartGraph, FlowchartGraphPaint, FlowchartGraphTimings,
};

use super::super::defs::{FlowchartDefs, FlowchartMarkerEmissionPlan};
use super::super::edge_fill::FlowchartEdgeFillPlan;
use super::super::paint::{FlowchartEdgeLabels, prepare_rough_edge_paths};
use super::super::render_input::FlowchartRenderEdgeRef;
use super::super::*;
use super::cluster::render_flowchart_cluster_pass;
use super::edge_label::render_swimlane_edge_label_node;
use super::node::render_flowchart_node_depth;
use crate::svg::parity::timing::RenderTiming;

pub(in crate::svg::parity::flowchart) struct FlowchartRootPaint<'a, 'data> {
    pub ctx: &'a FlowchartRenderCtx<'data>,
    pub defs: FlowchartDefs<'a>,
    pub timing: RenderTiming,
    pub details: &'a mut FlowchartRenderDetails,
    pub node_label_div_style: String,
    pub edge_cache:
        &'a mut FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    pub hierarchy_plan: &'a FlowchartHierarchyPlan<'data>,
    pub marker_plan: &'a FlowchartMarkerEmissionPlan,
    pub edge_fills: &'a FlowchartEdgeFillPlan,
    pub edge_labels: &'a mut FlowchartEdgeLabels,
    pub step_badges: &'a super::super::step::StepBadges,
}

impl FlowchartRootPaint<'_, '_> {
    pub(in crate::svg::parity::flowchart) fn write_graph(
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
            subgraphs_by_id: &ctx.subgraphs_by_id,
            subgraph_ids_with_children: &ctx.subgraph_ids_with_children,
            recursive_clusters: &ctx.recursive_clusters,
            swimlane_label_edges: &ctx.swimlane_edge_label_edges_by_node_id,
            swimlane: ctx.swimlane_direction.is_some(),
            has_depth: ctx.native.is_some_and(|style| style.has_depth()),
            edge_html_labels: ctx.edge_html_labels,
            dagre_merges_self_loop_segments:
                crate::flowchart::flowchart_dagre_merges_self_loop_segments(&ctx.model.keyword),
        }
        .write(out, self, timing, graph_timings)
    }
}

impl<S: SvgOutput> FlowchartGraphPaint<S> for FlowchartRootPaint<'_, '_> {
    type EdgeScratch = FlowchartEdgeDataPointsScratch;

    fn write_base_markers(&self, out: &mut S) -> Result<()> {
        self.defs.push_base_markers(out)
    }

    fn write_extra_markers(&self, out: &mut S) -> Result<()> {
        self.defs.push_extra_markers(out)
    }

    fn group_title_is_struck(&self, id: &str) -> bool {
        self.ctx.layout.route_contract.group_title_is_struck(id)
    }

    fn write_cluster(
        &self,
        out: &mut S,
        cluster: &LayoutCluster,
        (x, y): (f64, f64),
        pass: FlowchartClusterPass,
    ) -> Result<()> {
        render_flowchart_cluster_pass(out, self.ctx, cluster, x, y, pass)
    }

    fn prepare_edges(&mut self, edges: &[FlowchartRenderEdgeRef<'_>]) -> Result<()> {
        prepare_rough_edge_paths(self.ctx, edges, self.edge_cache)?;
        if self.ctx.hand_drawn_seed.seed().may_use_math_random() {
            self.edge_labels
                .update(self.ctx, self.hierarchy_plan, self.edge_cache, edges)?;
        }
        Ok(())
    }

    fn write_edge_casings(&self, out: &mut S, edges: &[FlowchartRenderEdgeRef<'_>]) -> Result<()> {
        self.edge_fills
            .write_casings(out, self.ctx, self.marker_plan, edges, self.edge_cache)
    }

    fn write_edge(
        &self,
        out: &mut S,
        edge: FlowchartRenderEdgeRef<'_>,
        (x, y): (f64, f64),
        scratch: &mut Self::EdgeScratch,
    ) -> Result<()> {
        render_flowchart_edge_path(
            out,
            self.ctx,
            self.marker_plan,
            self.edge_fills.ink(edge.key),
            edge,
            x,
            y,
            scratch,
            self.edge_cache,
        )
    }

    fn write_edge_label(
        &self,
        out: &mut S,
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

    fn write_swimlane_edge_label(
        &self,
        out: &mut S,
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

    fn write_node(&mut self, out: &mut S, id: &str, (x, y): (f64, f64), depth: bool) -> Result<()> {
        if depth {
            render_flowchart_node_depth(out, self.ctx, id, x, y)
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

    fn write_step_badges(&self, out: &mut S, root: Option<&str>) -> Result<()> {
        self.step_badges.write_root(out, self.ctx, root)
    }
}

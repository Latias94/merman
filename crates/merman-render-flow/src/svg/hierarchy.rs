//! Bind Flow hierarchy preparation to the facade's admitted render inputs.

pub(super) use crate::graph::FlowchartHierarchyPlan;
use crate::graph::{FlowchartHierarchyInputs, FlowchartNodeRootSchedule};
pub(super) use crate::hierarchy::{apply_solved_group_parents, flowchart_node_dom_indices};

use super::*;

pub(super) fn prepare_flowchart_hierarchy<'a>(
    ctx: &'a FlowchartRenderCtx<'a>,
    node_schedule: &'a FlowchartNodeRootSchedule,
) -> merman_render_core::Result<FlowchartHierarchyPlan<'a>> {
    FlowchartHierarchyPlan::prepare(
        FlowchartHierarchyInputs {
            uses_flat_adapter_dom: ctx.layout.route_contract.dom().is_flat(),
            node_schedule,
            edge_order: &ctx.edge_order,
            subgraphs_by_id: &ctx.subgraphs_by_id,
            subgraph_indices_by_id: &ctx.subgraph_indices_by_id,
            subgraph_ids_with_children: &ctx.subgraph_ids_with_children,
            recursive_clusters: &ctx.recursive_clusters,
            parent: &ctx.parent,
            layout_clusters_by_id: &ctx.layout_clusters_by_id,
            swimlane_lanes_by_id: &ctx.swimlane_lanes_by_id,
            tx: ctx.tx,
            ty: ctx.ty,
            work_meter: ctx.work_meter,
        },
        ctx.prepared_nodes.cluster_ids(),
    )
}

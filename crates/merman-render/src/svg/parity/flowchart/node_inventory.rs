//! Owned node root order prepared once and shared with hierarchy emission.

use super::hierarchy::{
    FlowchartDomCandidateBuckets, FlowchartEffectiveParentIndex, flowchart_dom_order_for_root,
    flowchart_elk_dom_order,
};
use super::*;

pub(super) struct FlowchartNodeInventoryInput<'a> {
    pub(super) nodes_by_id: &'a FxHashMap<&'a str, &'a crate::flowchart::FlowNode>,
    pub(super) subgraphs_by_id: &'a FxHashMap<&'a str, &'a crate::flowchart::FlowSubgraph>,
    pub(super) subgraph_indices_by_id: &'a FxHashMap<&'a str, usize>,
    pub(super) recursive_clusters: &'a FxHashSet<&'a str>,
    pub(super) parent: &'a FxHashMap<&'a str, &'a str>,
    pub(super) layout_nodes_by_id: &'a FxHashMap<&'a str, &'a LayoutNode>,
    pub(super) layout_clusters_by_id: &'a FxHashMap<&'a str, &'a LayoutCluster>,
    pub(super) dom_node_order_by_root: &'a std::collections::HashMap<String, Vec<String>>,
    pub(super) node_dom_index: &'a FxHashMap<&'a str, usize>,
    pub(super) uses_elk_adapter_dom: bool,
    subgraph_ids_with_children: &'a FxHashSet<&'a str>,
    pub(super) work_meter: &'a crate::resources::OperationWorkMeter,
    checkpoint: FlowchartEmitCheckpoint<'a>,
}

impl<'a> FlowchartNodeInventoryInput<'a> {
    pub(super) fn from_render_context(ctx: &'a FlowchartRenderCtx<'a>) -> Self {
        Self {
            nodes_by_id: &ctx.nodes_by_id,
            subgraphs_by_id: &ctx.subgraphs_by_id,
            subgraph_indices_by_id: &ctx.subgraph_indices_by_id,
            recursive_clusters: &ctx.recursive_clusters,
            parent: &ctx.parent,
            layout_nodes_by_id: &ctx.layout_nodes_by_id,
            layout_clusters_by_id: &ctx.layout_clusters_by_id,
            dom_node_order_by_root: ctx.dom_node_order_by_root,
            node_dom_index: &ctx.node_dom_index,
            uses_elk_adapter_dom: ctx.uses_elk_adapter_dom,
            subgraph_ids_with_children: &ctx.subgraph_ids_with_children,
            work_meter: ctx.work_meter,
            checkpoint: ctx.emit,
        }
    }

    pub(super) fn subgraph_has_children(&self, id: &str) -> bool {
        self.subgraph_ids_with_children.contains(id)
    }
}

pub(super) struct FlowchartNodeRootSchedule {
    top: Vec<String>,
    nested: Vec<(String, Vec<String>)>,
    effective_parents: FlowchartEffectiveParentIndex,
}

impl FlowchartNodeRootSchedule {
    pub(super) fn prepare(ctx: &FlowchartNodeInventoryInput<'_>) -> crate::Result<Self> {
        let effective_parents = FlowchartEffectiveParentIndex::prepare(
            ctx.parent,
            |id| ctx.subgraphs_by_id.contains_key(id) && !ctx.recursive_clusters.contains(id),
            ctx.work_meter,
        )?;
        let (top, nested) = {
            let mut candidates = FlowchartDomCandidateBuckets::prepare(ctx, &effective_parents)?;
            if ctx.uses_elk_adapter_dom {
                let top = flowchart_elk_dom_order(ctx, candidates.take(None))?;
                (top.into_iter().map(str::to_owned).collect(), Vec::new())
            } else {
                ctx.checkpoint.checkpoint()?;
                ctx.work_meter.charge(1)?;
                let top = flowchart_dom_order_for_root(ctx, None, candidates.take(None))?;
                let mut pending = Vec::new();
                let push_roots = |order: &[&str], pending: &mut Vec<String>| {
                    pending.extend(
                        order
                            .iter()
                            .rev()
                            .filter(|id| {
                                ctx.subgraphs_by_id.contains_key(**id)
                                    && ctx.subgraph_has_children(id)
                                    && ctx.recursive_clusters.contains(**id)
                            })
                            .map(|id| (*id).to_owned()),
                    );
                };
                push_roots(&top, &mut pending);
                let mut nested = Vec::new();
                let mut visited = FxHashSet::default();
                while let Some(root) = pending.pop() {
                    ctx.checkpoint.checkpoint()?;
                    ctx.work_meter.charge(1)?;
                    let root_id = ctx
                        .recursive_clusters
                        .get(root.as_str())
                        .copied()
                        .ok_or_else(|| crate::Error::InvalidModel {
                            message: format!("missing prepared Flowchart recursive root `{root}`"),
                        })?;
                    if !visited.insert(root_id) {
                        return Err(crate::Error::InvalidModel {
                            message: format!(
                                "Flowchart hierarchy visits extracted root `{root}` more than once"
                            ),
                        });
                    }
                    let order = flowchart_dom_order_for_root(
                        ctx,
                        Some(root_id),
                        candidates.take(Some(root_id)),
                    )?;
                    push_roots(&order, &mut pending);
                    nested.push((root, order.into_iter().map(str::to_owned).collect()));
                }
                (top.into_iter().map(str::to_owned).collect(), nested)
            }
        };
        Ok(Self {
            top,
            nested,
            effective_parents,
        })
    }

    pub(super) fn top(&self) -> &[String] {
        &self.top
    }
    pub(super) fn nested(&self) -> &[(String, Vec<String>)] {
        &self.nested
    }
    pub(super) fn effective_parents(&self) -> &FlowchartEffectiveParentIndex {
        &self.effective_parents
    }
}

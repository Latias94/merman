//! Flowchart hierarchy helpers (clusters, LCA, edge selection).

pub(super) use merman_render_flow::graph::FlowchartHierarchyPlan;
use merman_render_flow::graph::{
    FlowchartHierarchyInputs, FlowchartNodeRootSchedule, flowchart_checked_sort_work,
};
pub(super) use merman_render_flow::hierarchy::FlowchartEffectiveParentIndex;

use super::node_inventory::FlowchartNodeInventoryInput;
use super::*;

pub(super) fn apply_solved_group_parents<'a>(
    layout: &'a FlowchartLayout,
    parents: &mut FxHashMap<&'a str, &'a str>,
    clusters: &mut FxHashSet<&'a str>,
) {
    if let Some(solved_parents) = layout.route_contract.group_parents() {
        for (group, parent) in layout.clusters.iter().zip(solved_parents) {
            let id = group.id.as_str();
            clusters.insert(id);
            if let Some(index) = parent {
                parents.insert(id, &layout.clusters[*index as usize].id);
            } else {
                parents.remove(id);
            }
        }
    }
}

pub(super) fn flowchart_node_dom_indices<'a>(
    model: &'a crate::flowchart::FlowchartModel,
) -> FxHashMap<&'a str, usize> {
    if !model.vertex_calls.is_empty() {
        let mut out: FxHashMap<&'a str, usize> = FxHashMap::default();
        out.reserve(model.vertex_calls.len());
        for (vertex_counter, id) in model.vertex_calls.iter().enumerate() {
            let id: &'a str = id.as_str();
            let _ = out.entry(id).or_insert(vertex_counter);
        }
        return out;
    }

    let mut out: FxHashMap<&'a str, usize> = FxHashMap::default();
    out.reserve(model.edges.len().saturating_mul(2) + model.nodes.len());
    let mut vertex_counter: usize = 0;

    // Mermaid FlowDB assigns `domId` when a vertex is first created, but increments the internal
    // `vertexCounter` on every `addVertex(...)` call (even for repeated references). This means the
    // domId suffix depends on the full "first-use" order + repeat uses.
    fn touch<'a>(id: &'a str, out: &mut FxHashMap<&'a str, usize>, c: &mut usize) {
        let _ = out.entry(id).or_insert(*c);
        *c += 1;
    }

    for e in &model.edges {
        touch(e.from.as_str(), &mut out, &mut vertex_counter);
        touch(e.to.as_str(), &mut out, &mut vertex_counter);
    }

    for n in &model.nodes {
        touch(n.id.as_str(), &mut out, &mut vertex_counter);
    }

    out
}

#[derive(Clone, Copy)]
struct FlowchartClusterOrderKey<'a> {
    id: &'a str,
    subgraph_index: Option<usize>,
    left: f64,
    top: f64,
}

fn flowchart_cluster_order_key<'a>(
    ctx: &'a FlowchartNodeInventoryInput<'a>,
    id: &'a str,
) -> FlowchartClusterOrderKey<'a> {
    let (left, top) = ctx
        .layout_clusters_by_id
        .get(id)
        .map(|cluster| cluster.top_left())
        .unwrap_or((0.0, 0.0));
    FlowchartClusterOrderKey {
        id,
        subgraph_index: ctx.subgraph_indices_by_id.get(id).copied(),
        left,
        top,
    }
}

fn flowchart_sort_cluster_order_keys<'a>(
    mut keyed: Vec<FlowchartClusterOrderKey<'a>>,
) -> Vec<&'a str> {
    keyed.sort_by(|a, b| {
        if let (Some(ai), Some(bi)) = (a.subgraph_index, b.subgraph_index) {
            // Mirror Mermaid's Dagre graph registration behavior: sibling cluster roots tend to
            // appear in reverse subgraph definition order.
            bi.cmp(&ai)
                .then_with(|| a.left.total_cmp(&b.left))
                .then_with(|| a.top.total_cmp(&b.top))
                .then_with(|| a.id.cmp(b.id))
        } else {
            a.left
                .total_cmp(&b.left)
                .then_with(|| a.top.total_cmp(&b.top))
                .then_with(|| a.id.cmp(b.id))
        }
    });
    keyed.into_iter().map(|entry| entry.id).collect()
}

#[derive(Clone, Copy)]
struct FlowchartNodeOrderKey<'a> {
    id: &'a str,
    explicit_dom_index: usize,
    node_dom_index: usize,
    x: f64,
    y: f64,
    nesting_depth: usize,
    nearest_cluster_id: Option<&'a str>,
    directional_primary: f64,
    directional_secondary: f64,
}

fn flowchart_node_hierarchy_key_with<'a, E>(
    ctx: &'a FlowchartNodeInventoryInput<'a>,
    id: &str,
    parent_cluster: Option<&str>,
    mut visit_parent: impl FnMut() -> std::result::Result<(), E>,
) -> std::result::Result<(usize, Option<&'a str>), E> {
    let mut nesting_depth = 0usize;
    let mut nearest_cluster_id = None;
    let mut current = ctx.parent.get(id).copied();
    while let Some(parent) = current {
        visit_parent()?;
        let (counts_toward_depth, is_nearest_candidate) = if parent_cluster.is_some() {
            let is_subgraph = ctx.subgraphs_by_id.contains_key(parent);
            (
                is_subgraph,
                is_subgraph && ctx.subgraph_has_children(parent),
            )
        } else {
            let is_recursive = ctx.recursive_clusters.contains(parent);
            (is_recursive, is_recursive)
        };
        if counts_toward_depth {
            nesting_depth = nesting_depth.saturating_add(1);
        }
        if nearest_cluster_id.is_none() && is_nearest_candidate {
            nearest_cluster_id = Some(parent);
        }
        current = ctx.parent.get(parent).copied();
    }
    Ok((nesting_depth, nearest_cluster_id))
}

fn flowchart_node_hierarchy_key_metered<'a>(
    ctx: &'a FlowchartNodeInventoryInput<'a>,
    id: &str,
    parent_cluster: Option<&str>,
) -> crate::Result<(usize, Option<&'a str>)> {
    flowchart_node_hierarchy_key_with(ctx, id, parent_cluster, || {
        ctx.work_meter.charge(1)?;
        Ok::<(), crate::Error>(())
    })
}

fn flowchart_directional_sort_key(primary_dir: &str, x: f64, y: f64) -> (f64, f64) {
    match primary_dir {
        "BT" => (-y, x),
        "LR" => (x, y),
        "RL" => (-x, y),
        _ => (y, x), // TB (default)
    }
}

fn flowchart_explicit_dom_indices<'a>(
    ctx: &'a FlowchartNodeInventoryInput<'a>,
    parent_cluster: Option<&str>,
) -> Option<FxHashMap<&'a str, usize>> {
    ctx.dom_node_order_by_root
        .get(parent_cluster.unwrap_or(""))
        .map(|ids| {
            let mut indices = FxHashMap::default();
            indices.reserve(ids.len());
            for (index, id) in ids.iter().enumerate() {
                indices.insert(id.as_str(), index);
            }
            indices
        })
}

fn flowchart_node_order_key<'a>(
    ctx: &'a FlowchartNodeInventoryInput<'a>,
    explicit_dom_indices: Option<&FxHashMap<&str, usize>>,
    id: &'a str,
    nesting_depth: usize,
    nearest_cluster_id: Option<&'a str>,
) -> FlowchartNodeOrderKey<'a> {
    let (x, y) = ctx
        .layout_nodes_by_id
        .get(id)
        .map(|node| node.position())
        .unwrap_or((0.0, 0.0));
    let direction = nearest_cluster_id
        .and_then(|cluster_id| ctx.layout_clusters_by_id.get(cluster_id))
        .map(|cluster| cluster.direction())
        .unwrap_or("TB");
    let (directional_primary, directional_secondary) =
        flowchart_directional_sort_key(direction, x, y);
    FlowchartNodeOrderKey {
        id,
        explicit_dom_index: explicit_dom_indices
            .and_then(|indices| indices.get(id).copied())
            .unwrap_or(usize::MAX),
        node_dom_index: ctx.node_dom_index.get(id).copied().unwrap_or(usize::MAX),
        x,
        y,
        nesting_depth,
        nearest_cluster_id,
        directional_primary,
        directional_secondary,
    }
}

fn flowchart_sort_node_order_keys<'a>(mut keyed: Vec<FlowchartNodeOrderKey<'a>>) -> Vec<&'a str> {
    keyed.sort_by(|a, b| {
        a.explicit_dom_index
            .cmp(&b.explicit_dom_index)
            .then_with(|| b.nesting_depth.cmp(&a.nesting_depth))
            .then_with(|| {
                if a.nesting_depth == 0 && b.nesting_depth == 0 {
                    // For nodes not nested in any subgraph, upstream Mermaid keeps the graph
                    // insertion order as the primary key, then uses position to stabilize ties.
                    a.node_dom_index
                        .cmp(&b.node_dom_index)
                        .then_with(|| a.y.total_cmp(&b.y))
                        .then_with(|| a.x.total_cmp(&b.x))
                } else {
                    // For nodes that are nested in subgraphs, upstream Mermaid's DOM ordering is
                    // closer to “flow direction” ordering within the nearest cluster.
                    if a.nearest_cluster_id == b.nearest_cluster_id {
                        a.directional_primary
                            .total_cmp(&b.directional_primary)
                            .then_with(|| {
                                a.directional_secondary.total_cmp(&b.directional_secondary)
                            })
                            .then_with(|| a.node_dom_index.cmp(&b.node_dom_index))
                    } else {
                        // Different clusters at the same nesting depth: keep insertion order stable.
                        a.node_dom_index
                            .cmp(&b.node_dom_index)
                            .then_with(|| a.y.total_cmp(&b.y))
                            .then_with(|| a.x.total_cmp(&b.x))
                    }
                }
            })
            .then_with(|| a.id.cmp(b.id))
    });
    keyed.into_iter().map(|entry| entry.id).collect()
}

enum FlowchartRootNodeOrder<'a> {
    ModelOrder {
        ids: Vec<&'a str>,
        nodes: &'a [crate::flowchart::FlowNode],
    },
    SortKeys(Vec<FlowchartNodeOrderKey<'a>>),
}

impl<'a> FlowchartRootNodeOrder<'a> {
    fn len(&self) -> usize {
        match self {
            Self::ModelOrder { ids, .. } => ids.len(),
            Self::SortKeys(keys) => keys.len(),
        }
    }

    fn into_dom_order(self) -> Vec<&'a str> {
        match self {
            Self::ModelOrder { mut ids, nodes } => {
                for (id, node) in ids.iter_mut().zip(nodes) {
                    *id = node.id.as_str();
                }
                ids
            }
            Self::SortKeys(keys) => flowchart_sort_node_order_keys(keys),
        }
    }
}

fn flowchart_has_complete_model_node_order(
    ctx: &FlowchartNodeInventoryInput<'_>,
    candidate_count: usize,
) -> bool {
    if !ctx.parent.is_empty()
        || !ctx.subgraphs_by_id.is_empty()
        || ctx.nodes_by_id.len() != candidate_count
        || ctx.model_nodes.len() != candidate_count
    {
        return false;
    }

    let mut previous = None;
    ctx.model_nodes.iter().all(|node| {
        ctx.node_dom_index
            .get(node.id.as_str())
            .is_some_and(|&ordinal| {
                previous
                    .replace(ordinal)
                    .is_none_or(|previous| previous < ordinal)
            })
    })
}

#[derive(Default)]
pub(super) struct FlowchartDomCandidates<'a> {
    nodes: Vec<&'a str>,
    clusters: Vec<&'a str>,
}

#[derive(Default)]
pub(super) struct FlowchartDomCandidateBuckets<'a> {
    by_root: FxHashMap<Option<&'a str>, FlowchartDomCandidates<'a>>,
}

impl<'a> FlowchartDomCandidateBuckets<'a> {
    pub(super) fn prepare(
        ctx: &'a FlowchartNodeInventoryInput<'a>,
        parent_index: &'a FlowchartEffectiveParentIndex,
    ) -> crate::Result<Self> {
        let scan_work = ctx
            .nodes_by_id
            .len()
            .checked_add(ctx.subgraphs_by_id.len())
            .ok_or_else(|| ctx.work_meter.arithmetic_overflow())?;
        ctx.work_meter.charge(scan_work)?;

        let mut buckets = Self::default();
        for id in ctx.nodes_by_id.keys().copied() {
            if ctx.subgraph_has_children(id) {
                continue;
            }
            buckets.push_node(parent_index.parent(id), id);
        }
        for id in ctx.subgraphs_by_id.keys().copied() {
            let root = parent_index.parent(id);
            if ctx.subgraph_has_children(id) {
                if ctx.recursive_clusters.contains(id) {
                    buckets.push_cluster(root, id);
                }
            } else {
                buckets.push_node(root, id);
            }
        }
        Ok(buckets)
    }

    fn push_node(&mut self, root: Option<&'a str>, id: &'a str) {
        self.by_root.entry(root).or_default().nodes.push(id);
    }

    fn push_cluster(&mut self, root: Option<&'a str>, id: &'a str) {
        self.by_root.entry(root).or_default().clusters.push(id);
    }

    pub(super) fn take(&mut self, root: Option<&'a str>) -> FlowchartDomCandidates<'a> {
        self.by_root.remove(&root).unwrap_or_default()
    }
}

fn flowchart_prepare_root_children_nodes_metered<'a>(
    ctx: &'a FlowchartNodeInventoryInput<'a>,
    parent_cluster: Option<&str>,
    ids: Vec<&'a str>,
) -> crate::Result<FlowchartRootNodeOrder<'a>> {
    let explicit_dom_order = ctx.dom_node_order_by_root.get(parent_cluster.unwrap_or(""));
    ctx.work_meter
        .charge(explicit_dom_order.map_or(0, |order| order.len()))?;
    let explicit_dom_indices = flowchart_explicit_dom_indices(ctx, parent_cluster);

    ctx.work_meter.charge(ids.len())?;
    if parent_cluster.is_none()
        && explicit_dom_order.is_none_or(Vec::is_empty)
        && flowchart_has_complete_model_node_order(ctx, ids.len())
    {
        return Ok(FlowchartRootNodeOrder::ModelOrder {
            ids,
            nodes: ctx.model_nodes,
        });
    }

    let mut keyed = Vec::with_capacity(ids.len());
    for id in ids {
        let (nesting_depth, nearest_cluster_id) =
            flowchart_node_hierarchy_key_metered(ctx, id, parent_cluster)?;
        keyed.push(flowchart_node_order_key(
            ctx,
            explicit_dom_indices.as_ref(),
            id,
            nesting_depth,
            nearest_cluster_id,
        ));
    }
    Ok(FlowchartRootNodeOrder::SortKeys(keyed))
}

fn flowchart_prepare_root_children_clusters_metered<'a>(
    ctx: &'a FlowchartNodeInventoryInput<'a>,
    ids: Vec<&'a str>,
) -> crate::Result<Vec<FlowchartClusterOrderKey<'a>>> {
    ctx.work_meter.charge(ids.len())?;
    Ok(ids
        .into_iter()
        .map(|id| flowchart_cluster_order_key(ctx, id))
        .collect())
}

pub(super) fn flowchart_dom_order_for_root<'a>(
    ctx: &'a FlowchartNodeInventoryInput<'a>,
    cluster_id: Option<&str>,
    candidates: FlowchartDomCandidates<'a>,
) -> crate::Result<Vec<&'a str>> {
    let explicit_dom_order = ctx.dom_node_order_by_root.get(cluster_id.unwrap_or(""));
    let mut dom_order = if let Some(ids) = explicit_dom_order {
        let work = ids
            .len()
            .checked_mul(2)
            .ok_or_else(|| ctx.work_meter.arithmetic_overflow())?;
        ctx.work_meter.charge(work)?;
        ids.iter().map(String::as_str).collect()
    } else {
        Vec::new()
    };

    if !dom_order.is_empty() {
        // Some upstream flowchart-v2 configurations only register non-recursive clusters with
        // external edges. Those clusters do not emit a node DOM element, so the raw order would
        // otherwise produce an empty `.nodes` group.
        let emits_anything = dom_order.iter().any(|id| {
            if ctx.subgraphs_by_id.contains_key(id) && ctx.subgraph_has_children(id) {
                return ctx.recursive_clusters.contains(id);
            }
            true
        });
        if !emits_anything {
            dom_order.clear();
        }
    }

    if dom_order.is_empty() {
        let node_order =
            flowchart_prepare_root_children_nodes_metered(ctx, cluster_id, candidates.nodes)?;
        let cluster_keys =
            flowchart_prepare_root_children_clusters_metered(ctx, candidates.clusters)?;

        let sort_work = flowchart_checked_sort_work(node_order.len(), ctx.work_meter)?
            .checked_add(flowchart_checked_sort_work(
                cluster_keys.len(),
                ctx.work_meter,
            )?)
            .ok_or_else(|| ctx.work_meter.arithmetic_overflow())?;
        ctx.work_meter.charge(sort_work)?;

        dom_order = node_order.into_dom_order();
        dom_order.extend(flowchart_sort_cluster_order_keys(cluster_keys));
    }
    Ok(dom_order)
}

pub(super) fn flowchart_elk_dom_order<'a>(
    ctx: &'a FlowchartNodeInventoryInput<'a>,
    candidates: FlowchartDomCandidates<'a>,
) -> crate::Result<Vec<&'a str>> {
    if let Some(ids) = ctx.dom_node_order_by_root.get("")
        && !ids.is_empty()
    {
        ctx.work_meter.charge(ids.len())?;
        return Ok(ids.iter().map(String::as_str).collect());
    }
    flowchart_dom_order_for_root(ctx, None, candidates)
}

pub(super) fn prepare_flowchart_hierarchy<'a>(
    ctx: &'a FlowchartRenderCtx<'a>,
    node_schedule: &'a FlowchartNodeRootSchedule,
) -> crate::Result<FlowchartHierarchyPlan<'a>> {
    FlowchartHierarchyPlan::prepare(
        FlowchartHierarchyInputs {
            uses_elk_adapter_dom: ctx.uses_elk_adapter_dom,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dom_candidate_buckets_partition_each_item_once_by_prepared_root() {
        let mut buckets = FlowchartDomCandidateBuckets::default();
        buckets.push_node(None, "top-node");
        buckets.push_cluster(None, "outer");
        buckets.push_node(Some("outer"), "nested-node");
        buckets.push_cluster(Some("outer"), "inner");

        let top = buckets.take(None);
        assert_eq!(top.nodes, ["top-node"]);
        assert_eq!(top.clusters, ["outer"]);

        let outer = buckets.take(Some("outer"));
        assert_eq!(outer.nodes, ["nested-node"]);
        assert_eq!(outer.clusters, ["inner"]);

        let already_taken = buckets.take(Some("outer"));
        assert!(already_taken.nodes.is_empty());
        assert!(already_taken.clusters.is_empty());
    }
}

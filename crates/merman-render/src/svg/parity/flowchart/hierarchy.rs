//! Flowchart hierarchy helpers (clusters, LCA, edge selection).

use merman_render_flow::hierarchy::FlowchartAncestryIndex;
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

pub(in crate::svg::parity::flowchart) fn flowchart_checked_sort_work(
    item_count: usize,
    work_meter: &crate::resources::OperationWorkMeter,
) -> crate::Result<usize> {
    if item_count <= 1 {
        return Ok(0);
    }
    item_count
        .checked_mul(item_count.ilog2() as usize + 1)
        .ok_or_else(|| work_meter.arithmetic_overflow().into())
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

#[derive(Clone, Copy)]
struct FlowchartRenderedClusterOrderKey<'a> {
    cluster: &'a LayoutCluster,
    subgraph_index: Option<usize>,
    top_y: f64,
    top_x: f64,
}

impl<'a> FlowchartRenderedClusterOrderKey<'a> {
    fn prepare(ctx: &FlowchartRenderCtx<'_>, cluster: &'a LayoutCluster) -> Self {
        Self {
            cluster,
            subgraph_index: ctx.subgraph_indices_by_id.get(cluster.id.as_str()).copied(),
            top_y: cluster.y - cluster.height / 2.0,
            top_x: cluster.x - cluster.width / 2.0,
        }
    }
}

fn flowchart_sort_rendered_clusters<'a>(
    ctx: &FlowchartRenderCtx<'_>,
    ancestry: &FlowchartAncestryIndex<'_>,
    clusters: Vec<&'a LayoutCluster>,
) -> crate::Result<Vec<&'a LayoutCluster>> {
    let sort_work = flowchart_checked_sort_work(clusters.len(), ctx.work_meter)?
        .checked_add(clusters.len())
        .ok_or_else(|| ctx.work_meter.arithmetic_overflow())?;
    ctx.work_meter.charge(sort_work)?;

    let mut keyed = clusters
        .into_iter()
        .map(|cluster| FlowchartRenderedClusterOrderKey::prepare(ctx, cluster))
        .collect::<Vec<_>>();
    keyed.sort_by(|a, b| {
        let a_id = a.cluster.id.as_str();
        let b_id = b.cluster.id.as_str();
        if a_id != b_id {
            if ancestry.is_strict_ancestor(a_id, b_id) {
                return std::cmp::Ordering::Less;
            }
            if ancestry.is_strict_ancestor(b_id, a_id) {
                return std::cmp::Ordering::Greater;
            }
        }

        if let (Some(ai), Some(bi)) = (a.subgraph_index, b.subgraph_index) {
            bi.cmp(&ai)
                .then_with(|| b.top_y.total_cmp(&a.top_y))
                .then_with(|| b.top_x.total_cmp(&a.top_x))
                .then_with(|| a_id.cmp(b_id))
        } else {
            b.top_y
                .total_cmp(&a.top_y)
                .then_with(|| b.top_x.total_cmp(&a.top_x))
                .then_with(|| a_id.cmp(b_id))
        }
    });
    Ok(keyed.into_iter().map(|entry| entry.cluster).collect())
}

#[derive(Default)]
struct FlowchartRenderedClusterBuckets<'a> {
    by_root: FxHashMap<Option<&'a str>, Vec<&'a LayoutCluster>>,
}

impl<'a> FlowchartRenderedClusterBuckets<'a> {
    fn prepare(
        ctx: &'a FlowchartRenderCtx<'a>,
        parent_index: &'a FlowchartEffectiveParentIndex,
        ancestry: &FlowchartAncestryIndex<'a>,
    ) -> crate::Result<Self> {
        let mut buckets = Self::default();
        for id in ctx.prepared_nodes.cluster_ids() {
            ctx.work_meter.charge(1)?;
            let cluster = ctx.layout_clusters_by_id.get(id).copied().ok_or_else(|| {
                crate::Error::InvalidModel {
                    message: format!("missing prepared Flowchart cluster geometry `{id}`"),
                }
            })?;
            let root = if ctx.subgraphs_by_id.contains_key(id) {
                if ctx.recursive_clusters.contains(id) {
                    Some(cluster.id.as_str())
                } else {
                    parent_index.parent(cluster.id.as_str())
                }
            } else {
                ctx.swimlane_lanes_by_id
                    .get(id)
                    .and_then(|lane| lane.parent_id.as_deref())
            };
            buckets.by_root.entry(root).or_default().push(cluster);
        }

        for clusters in buckets.by_root.values_mut() {
            let unsorted = std::mem::take(clusters);
            *clusters = flowchart_sort_rendered_clusters(ctx, ancestry, unsorted)?;
        }
        Ok(buckets)
    }

    fn prepare_elk(ctx: &'a FlowchartRenderCtx<'a>) -> crate::Result<Self> {
        let mut clusters = Vec::new();
        for id in ctx.prepared_nodes.cluster_ids() {
            ctx.work_meter.charge(1)?;
            clusters.push(ctx.layout_clusters_by_id.get(id).copied().ok_or_else(|| {
                crate::Error::InvalidModel {
                    message: format!("missing prepared Flowchart cluster geometry `{id}`"),
                }
            })?);
        }

        let mut by_root = FxHashMap::default();
        by_root.insert(None, clusters);
        Ok(Self { by_root })
    }

    fn take(&mut self, root: Option<&'a str>) -> Vec<&'a LayoutCluster> {
        self.by_root.remove(&root).unwrap_or_default()
    }
}

fn flowchart_prepare_root_offsets<'a>(
    ctx: &'a FlowchartRenderCtx<'a>,
    parent_index: &'a FlowchartEffectiveParentIndex,
) -> crate::Result<(FxHashMap<&'a str, FlowchartRootOffsets>, f64)> {
    const ROOT_MARGIN_PX: f64 = 8.0;

    ctx.work_meter.charge(ctx.subgraphs_by_id.len())?;
    let mut empty_sibling_count_by_root: FxHashMap<Option<&'a str>, usize> = FxHashMap::default();
    let mut empty_subgraphs = FxHashSet::default();
    for id in ctx.subgraphs_by_id.keys().copied() {
        if ctx.subgraph_has_children(id) || !ctx.layout_clusters_by_id.contains_key(id) {
            continue;
        }
        empty_subgraphs.insert(id);
        let count = empty_sibling_count_by_root
            .entry(parent_index.parent(id))
            .or_default();
        *count = count
            .checked_add(1)
            .ok_or_else(|| ctx.work_meter.arithmetic_overflow())?;
    }

    ctx.work_meter.charge(ctx.recursive_clusters.len())?;
    let mut offsets_by_root = FxHashMap::default();
    offsets_by_root.reserve(ctx.recursive_clusters.len());
    let mut extra_root_y = 0.0f64;
    for cid in ctx.recursive_clusters.iter().copied() {
        let Some(cluster) = ctx.layout_clusters_by_id.get(cid).copied() else {
            continue;
        };
        let empty_siblings = empty_sibling_count_by_root
            .get(&parent_index.parent(cid))
            .copied()
            .unwrap_or(0)
            .saturating_sub(usize::from(empty_subgraphs.contains(cid)));

        let abs_left = if cluster.diff > 0.0 {
            (cluster.x + cluster.diff - cluster.width / 2.0) + ctx.tx
        } else {
            (cluster.x - cluster.width / 2.0) + ctx.tx - ROOT_MARGIN_PX
        };
        let title_total_margin = (cluster.title_margin_top + cluster.title_margin_bottom).max(0.0);
        let title_y_shift = title_total_margin / 2.0;
        let base_top = (cluster.y - cluster.height / 2.0) + ctx.ty - ROOT_MARGIN_PX;
        let extra_transform_y = if empty_siblings > 0 {
            cluster.offset_y.max(0.0) * 2.0
        } else {
            0.0
        };
        extra_root_y = extra_root_y.max(extra_transform_y);
        offsets_by_root.insert(
            cid,
            FlowchartRootOffsets {
                origin_x: abs_left,
                origin_y: base_top + title_y_shift,
                abs_top_transform: base_top + extra_transform_y,
            },
        );
    }
    Ok((offsets_by_root, extra_root_y))
}

pub(in crate::svg::parity::flowchart) struct FlowchartRootEmission<'a> {
    edges: Vec<super::render_input::FlowchartRenderEdgeRef<'a>>,
    dom_order: Vec<&'a str>,
    clusters: Vec<&'a LayoutCluster>,
}

impl<'a> FlowchartRootEmission<'a> {
    pub(in crate::svg::parity::flowchart) fn edges(
        &self,
    ) -> &[super::render_input::FlowchartRenderEdgeRef<'a>] {
        &self.edges
    }

    pub(in crate::svg::parity::flowchart) fn dom_order(&self) -> &[&'a str] {
        &self.dom_order
    }

    pub(in crate::svg::parity::flowchart) fn clusters(&self) -> &[&'a LayoutCluster] {
        &self.clusters
    }
}

pub(in crate::svg::parity::flowchart) struct FlowchartHierarchyPlan<'a> {
    top_root: FlowchartRootEmission<'a>,
    nested_roots: FxHashMap<&'a str, FlowchartRootEmission<'a>>,
    ordered_edges: Option<Vec<super::render_input::FlowchartRenderEdgeRef<'a>>>,
    edge_root_by_key: FxHashMap<crate::flowchart::FlowchartEdgeKey, Option<&'a str>>,
    effective_parents: &'a FlowchartEffectiveParentIndex,
    root_offsets: FxHashMap<&'a str, FlowchartRootOffsets>,
    extra_recursive_root_y: f64,
}

impl<'a> FlowchartHierarchyPlan<'a> {
    pub(in crate::svg::parity::flowchart) fn prepare(
        ctx: &'a FlowchartRenderCtx<'a>,
        node_schedule: &'a super::node_inventory::FlowchartNodeRootSchedule,
    ) -> crate::Result<Self> {
        let physical_ancestry = FlowchartAncestryIndex::prepare(
            &ctx.subgraphs_by_id,
            &ctx.layout_clusters_by_id,
            &ctx.parent,
            ctx.work_meter,
        )?;
        let effective_parents = node_schedule.effective_parents();
        let (root_offsets, extra_recursive_root_y) =
            flowchart_prepare_root_offsets(ctx, effective_parents)?;

        if ctx.uses_elk_adapter_dom {
            ctx.work_meter.charge(ctx.edge_order.len())?;
            let edges = ctx.edge_order.clone();
            let mut edge_root_by_key = FxHashMap::default();
            edge_root_by_key.reserve(edges.len());
            for edge in edges.iter().copied() {
                if edge_root_by_key.insert(edge.key, None).is_some() {
                    return Err(flowchart_duplicate_edge_key_error(edge.key));
                }
            }
            let dom_order = node_schedule
                .top()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            let mut cluster_buckets = FlowchartRenderedClusterBuckets::prepare_elk(ctx)?;
            let clusters = cluster_buckets.take(None);
            let retained_refs = edges
                .len()
                .checked_add(dom_order.len())
                .and_then(|count| count.checked_add(clusters.len()))
                .ok_or_else(|| ctx.work_meter.arithmetic_overflow())?;
            ctx.work_meter.charge(retained_refs)?;
            return Ok(Self {
                ordered_edges: None,
                top_root: FlowchartRootEmission {
                    edges,
                    dom_order,
                    clusters,
                },
                nested_roots: FxHashMap::default(),
                edge_root_by_key,
                effective_parents,
                root_offsets,
                extra_recursive_root_y,
            });
        }

        let mut cluster_buckets =
            FlowchartRenderedClusterBuckets::prepare(ctx, effective_parents, &physical_ancestry)?;
        let mut top_edges = Vec::new();
        let mut edges_by_nested_root: FxHashMap<
            &'a str,
            Vec<super::render_input::FlowchartRenderEdgeRef<'a>>,
        > = FxHashMap::default();
        let mut edge_root_by_key = FxHashMap::default();
        edge_root_by_key.reserve(ctx.edge_order.len());
        for edge in ctx.edge_order.iter().copied() {
            ctx.work_meter.charge(1)?;
            let root = flowchart_lca_with_work_meter(
                ctx,
                edge.edge.from.as_str(),
                edge.edge.to.as_str(),
                effective_parents,
                ctx.work_meter,
            )?;
            if edge_root_by_key.insert(edge.key, root).is_some() {
                return Err(flowchart_duplicate_edge_key_error(edge.key));
            }
            match root {
                Some(root) => edges_by_nested_root.entry(root).or_default().push(edge),
                None => top_edges.push(edge),
            }
        }

        let top_dom_order = node_schedule
            .top()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let top_clusters = cluster_buckets.take(None);
        let top_retained_refs = top_edges
            .len()
            .checked_add(top_dom_order.len())
            .and_then(|count| count.checked_add(top_clusters.len()))
            .ok_or_else(|| ctx.work_meter.arithmetic_overflow())?;
        ctx.work_meter.charge(top_retained_refs)?;
        let top_root = FlowchartRootEmission {
            edges: top_edges,
            dom_order: top_dom_order,
            clusters: top_clusters,
        };

        let mut nested_roots = FxHashMap::default();
        let mut ordered_edges = None;
        for (cluster_id, scheduled_order) in node_schedule.nested() {
            let cluster_id = cluster_id.as_str();
            let edges = edges_by_nested_root.remove(cluster_id).unwrap_or_default();
            let dom_order = scheduled_order
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            let clusters = cluster_buckets.take(Some(cluster_id));
            let retained_refs = edges
                .len()
                .checked_add(dom_order.len())
                .and_then(|count| count.checked_add(clusters.len()))
                .ok_or_else(|| ctx.work_meter.arithmetic_overflow())?;
            ctx.work_meter.charge(retained_refs)?;
            if !edges.is_empty() {
                let order = ordered_edges.get_or_insert_with(|| {
                    let mut order = Vec::with_capacity(ctx.edge_order.len());
                    order.extend_from_slice(top_root.edges());
                    order
                });
                order.extend_from_slice(&edges);
            }
            if nested_roots
                .insert(
                    cluster_id,
                    FlowchartRootEmission {
                        edges,
                        dom_order,
                        clusters,
                    },
                )
                .is_some()
            {
                return Err(crate::Error::InvalidModel {
                    message: format!(
                        "Flowchart hierarchy visits extracted root `{cluster_id}` more than once"
                    ),
                });
            }
        }

        let covered_edges = ordered_edges
            .as_ref()
            .map_or(top_root.edges.len(), Vec::len);
        if !edges_by_nested_root.is_empty() || covered_edges != ctx.edge_order.len() {
            return Err(crate::Error::InvalidModel {
                message: "Flowchart hierarchy does not cover every prepared render item"
                    .to_string(),
            });
        }
        Ok(Self {
            top_root,
            nested_roots,
            ordered_edges,
            edge_root_by_key,
            effective_parents,
            root_offsets,
            extra_recursive_root_y,
        })
    }

    pub(in crate::svg::parity::flowchart) fn root(
        &self,
        cluster_id: Option<&str>,
    ) -> crate::Result<&FlowchartRootEmission<'a>> {
        match cluster_id {
            None => Ok(&self.top_root),
            Some(cluster_id) => {
                self.nested_roots
                    .get(cluster_id)
                    .ok_or_else(|| crate::Error::InvalidModel {
                        message: format!(
                            "missing prepared Flowchart root emission for `{cluster_id}`"
                        ),
                    })
            }
        }
    }

    pub(in crate::svg::parity::flowchart) fn ordered_edges(
        &self,
    ) -> &[super::render_input::FlowchartRenderEdgeRef<'a>] {
        self.ordered_edges
            .as_deref()
            .unwrap_or(self.top_root.edges())
    }

    pub(in crate::svg::parity::flowchart) fn painted_clusters(
        &self,
    ) -> std::borrow::Cow<'_, [&'a LayoutCluster]> {
        if self.nested_roots.is_empty() {
            return std::borrow::Cow::Borrowed(self.top_root.clusters());
        }
        let mut ordered = Vec::new();
        let mut stack = vec![&self.top_root];
        while let Some(root) = stack.pop() {
            ordered.extend_from_slice(root.clusters());
            for &id in root.dom_order().iter().rev() {
                if let Some(nested) = self.nested_roots.get(id) {
                    stack.push(nested);
                }
            }
        }
        std::borrow::Cow::Owned(ordered)
    }

    pub(in crate::svg::parity::flowchart) fn rendered_node_ids(
        &self,
    ) -> impl Iterator<Item = &'a str> + '_ {
        self.top_root
            .dom_order
            .iter()
            .chain(
                self.nested_roots
                    .values()
                    .flat_map(|root| root.dom_order.iter()),
            )
            .copied()
    }

    pub(in crate::svg::parity::flowchart) fn effective_parent(&self, id: &str) -> Option<&'a str> {
        self.effective_parents.parent(id)
    }

    pub(in crate::svg::parity::flowchart) fn edge_root(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
    ) -> crate::Result<Option<&'a str>> {
        self.edge_root_by_key
            .get(&key)
            .copied()
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!(
                    "missing prepared Flowchart hierarchy root for edge occurrence {}",
                    key.semantic_index()
                ),
            })
    }

    pub(in crate::svg::parity::flowchart) fn root_offsets(
        &self,
        cluster_id: &str,
    ) -> Option<FlowchartRootOffsets> {
        self.root_offsets.get(cluster_id).copied()
    }

    pub(in crate::svg::parity::flowchart) fn extra_recursive_root_y(&self) -> f64 {
        self.extra_recursive_root_y
    }
}

fn flowchart_duplicate_edge_key_error(key: crate::flowchart::FlowchartEdgeKey) -> crate::Error {
    crate::Error::InvalidModel {
        message: format!(
            "Flowchart hierarchy contains duplicate edge occurrence {}",
            key.semantic_index()
        ),
    }
}

fn flowchart_lca_with_work_meter<'a>(
    ctx: &'a FlowchartRenderCtx<'a>,
    a: &'a str,
    b: &'a str,
    parent_index: &'a FlowchartEffectiveParentIndex,
    work_meter: &crate::resources::OperationWorkMeter,
) -> crate::Result<Option<&'a str>> {
    fn first_render_root<'a>(
        ctx: &'a FlowchartRenderCtx<'a>,
        parent_index: &'a FlowchartEffectiveParentIndex,
        id: &'a str,
        include_cluster_endpoint: bool,
    ) -> Option<&'a str> {
        if include_cluster_endpoint && ctx.recursive_clusters.contains(id) {
            Some(id)
        } else {
            parent_index.parent(id)
        }
    }

    let include_cluster_endpoints = a != b;
    let left = first_render_root(ctx, parent_index, a, include_cluster_endpoints);
    let right = first_render_root(ctx, parent_index, b, include_cluster_endpoints);
    parent_index.common_root(left, right, work_meter)
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

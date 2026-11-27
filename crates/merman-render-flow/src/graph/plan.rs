//! Root-local frames retaining the traversal order prepared for SVG emission.

use merman_model::{OperationPhase, families::flowchart::FlowSubgraph};
use merman_render_core::{
    Error, LayoutCluster, Result, SwimlaneLaneLayout, resources::OperationWorkMeter,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::hierarchy::{FlowchartAncestryIndex, FlowchartEffectiveParentIndex};
use crate::projection::{FlowchartEdgeKey, FlowchartRenderEdgeRef};

/// A checked DOM order for the layout's flat or extracted roots.
#[derive(Debug)]
pub struct FlowchartNodeRootOrder {
    top: Vec<String>,
    nested: Vec<(String, Vec<String>)>,
}

impl FlowchartNodeRootOrder {
    pub fn flat(top: Vec<&str>) -> Self {
        Self {
            top: top.into_iter().map(str::to_owned).collect(),
            nested: Vec::new(),
        }
    }

    /// Visit extracted roots in DOM order. The caller supplies each root's layout order.
    pub fn prepare_nested<'a>(
        subgraphs_by_id: &FxHashMap<&'a str, &FlowSubgraph>,
        subgraph_ids_with_children: &FxHashSet<&str>,
        recursive_clusters: &FxHashSet<&'a str>,
        work_meter: &OperationWorkMeter,
        mut dom_order_for_root: impl FnMut(Option<&'a str>) -> Result<Vec<&'a str>>,
    ) -> Result<Self> {
        work_meter.checkpoint(OperationPhase::Layout)?;
        work_meter.charge(1)?;
        let top = dom_order_for_root(None)?;
        let mut pending = Vec::new();
        let push_roots = |order: &[&str], pending: &mut Vec<String>| {
            pending.extend(
                order
                    .iter()
                    .rev()
                    .filter(|id| {
                        subgraphs_by_id.contains_key(**id)
                            && subgraph_ids_with_children.contains(**id)
                            && recursive_clusters.contains(**id)
                    })
                    .map(|id| (*id).to_owned()),
            );
        };
        push_roots(&top, &mut pending);
        let mut nested = Vec::new();
        let mut visited = FxHashSet::default();
        while let Some(root) = pending.pop() {
            work_meter.checkpoint(OperationPhase::Layout)?;
            work_meter.charge(1)?;
            let root_id = recursive_clusters
                .get(root.as_str())
                .copied()
                .ok_or_else(|| Error::InvalidModel {
                    message: format!("missing prepared Flowchart recursive root `{root}`"),
                })?;
            if !visited.insert(root_id) {
                return Err(Error::InvalidModel {
                    message: format!(
                        "Flowchart hierarchy visits extracted root `{root}` more than once"
                    ),
                });
            }
            let order = dom_order_for_root(Some(root_id))?;
            push_roots(&order, &mut pending);
            nested.push((root, order.into_iter().map(str::to_owned).collect()));
        }
        Ok(Self {
            top: top.into_iter().map(str::to_owned).collect(),
            nested,
        })
    }
}

/// DOM node order and render parents shared by label admission and graph emission.
#[derive(Debug)]
pub struct FlowchartNodeRootSchedule {
    order: FlowchartNodeRootOrder,
    effective_parents: FlowchartEffectiveParentIndex,
}

impl FlowchartNodeRootSchedule {
    pub fn new(
        effective_parents: FlowchartEffectiveParentIndex,
        order: FlowchartNodeRootOrder,
    ) -> Self {
        Self {
            order,
            effective_parents,
        }
    }

    pub fn top(&self) -> &[String] {
        &self.order.top
    }

    pub fn nested(&self) -> &[(String, Vec<String>)] {
        &self.order.nested
    }

    pub fn effective_parents(&self) -> &FlowchartEffectiveParentIndex {
        &self.effective_parents
    }
}

/// The existing node schedule and admitted, occurrence-keyed render inputs.
pub struct FlowchartHierarchyInputs<'a> {
    pub uses_flat_adapter_dom: bool,
    pub node_schedule: &'a FlowchartNodeRootSchedule,
    pub edge_order: &'a [FlowchartRenderEdgeRef<'a>],
    pub subgraphs_by_id: &'a FxHashMap<&'a str, &'a FlowSubgraph>,
    pub subgraph_indices_by_id: &'a FxHashMap<&'a str, usize>,
    pub subgraph_ids_with_children: &'a FxHashSet<&'a str>,
    pub recursive_clusters: &'a FxHashSet<&'a str>,
    pub parent: &'a FxHashMap<&'a str, &'a str>,
    pub layout_clusters_by_id: &'a FxHashMap<&'a str, &'a LayoutCluster>,
    pub swimlane_lanes_by_id: &'a FxHashMap<&'a str, &'a SwimlaneLaneLayout>,
    pub tx: f64,
    pub ty: f64,
    pub work_meter: &'a OperationWorkMeter,
}

impl FlowchartHierarchyInputs<'_> {
    fn subgraph_has_children(&self, id: &str) -> bool {
        self.subgraph_ids_with_children.contains(id)
    }
}

/// Absolute content and paint origins of a nested root.
#[derive(Debug, Clone, Copy)]
pub struct FlowchartRootOffsets {
    pub origin_x: f64,
    pub origin_y: f64,
    pub abs_top_transform: f64,
}

pub fn flowchart_checked_sort_work(
    item_count: usize,
    work_meter: &OperationWorkMeter,
) -> Result<usize> {
    if item_count <= 1 {
        return Ok(0);
    }
    item_count
        .checked_mul(item_count.ilog2() as usize + 1)
        .ok_or_else(|| work_meter.arithmetic_overflow().into())
}

#[derive(Clone, Copy)]
struct FlowchartRenderedClusterOrderKey<'a> {
    cluster: &'a LayoutCluster,
    subgraph_index: Option<usize>,
    top_y: f64,
    top_x: f64,
}

impl<'a> FlowchartRenderedClusterOrderKey<'a> {
    fn prepare(ctx: &FlowchartHierarchyInputs<'_>, cluster: &'a LayoutCluster) -> Self {
        Self {
            cluster,
            subgraph_index: ctx.subgraph_indices_by_id.get(cluster.id.as_str()).copied(),
            top_y: cluster.y - cluster.height / 2.0,
            top_x: cluster.x - cluster.width / 2.0,
        }
    }
}

fn flowchart_sort_rendered_clusters<'a>(
    ctx: &FlowchartHierarchyInputs<'_>,
    ancestry: &FlowchartAncestryIndex<'_>,
    clusters: Vec<&'a LayoutCluster>,
) -> Result<Vec<&'a LayoutCluster>> {
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
        ctx: &FlowchartHierarchyInputs<'a>,
        parent_index: &'a FlowchartEffectiveParentIndex,
        ancestry: &FlowchartAncestryIndex<'a>,
        cluster_ids: impl Iterator<Item = &'a str>,
    ) -> Result<Self> {
        let mut buckets = Self::default();
        for id in cluster_ids {
            ctx.work_meter.charge(1)?;
            let cluster =
                ctx.layout_clusters_by_id
                    .get(id)
                    .copied()
                    .ok_or_else(|| Error::InvalidModel {
                        message: format!("missing prepared Flowchart cluster geometry `{id}`"),
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

    fn prepare_flat(
        ctx: &FlowchartHierarchyInputs<'a>,
        cluster_ids: impl Iterator<Item = &'a str>,
    ) -> Result<Self> {
        let mut clusters = Vec::new();
        for id in cluster_ids {
            ctx.work_meter.charge(1)?;
            clusters.push(ctx.layout_clusters_by_id.get(id).copied().ok_or_else(|| {
                Error::InvalidModel {
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
    ctx: &FlowchartHierarchyInputs<'a>,
    parent_index: &'a FlowchartEffectiveParentIndex,
) -> Result<(FxHashMap<&'a str, FlowchartRootOffsets>, f64)> {
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

pub struct FlowchartRootEmission<'a> {
    edges: Vec<FlowchartRenderEdgeRef<'a>>,
    dom_order: Vec<&'a str>,
    clusters: Vec<&'a LayoutCluster>,
}

impl<'a> FlowchartRootEmission<'a> {
    pub fn edges(&self) -> &[FlowchartRenderEdgeRef<'a>] {
        &self.edges
    }

    pub fn dom_order(&self) -> &[&'a str] {
        &self.dom_order
    }

    pub fn clusters(&self) -> &[&'a LayoutCluster] {
        &self.clusters
    }
}

pub struct FlowchartHierarchyPlan<'a> {
    pub(super) uses_flat_adapter_dom: bool,
    top_root: FlowchartRootEmission<'a>,
    nested_roots: FxHashMap<&'a str, FlowchartRootEmission<'a>>,
    ordered_edges: Option<Vec<FlowchartRenderEdgeRef<'a>>>,
    edge_root_by_key: FxHashMap<FlowchartEdgeKey, Option<&'a str>>,
    effective_parents: &'a FlowchartEffectiveParentIndex,
    root_offsets: FxHashMap<&'a str, FlowchartRootOffsets>,
    extra_recursive_root_y: f64,
}

impl<'a> FlowchartHierarchyPlan<'a> {
    pub fn prepare(
        ctx: FlowchartHierarchyInputs<'a>,
        cluster_ids: impl Iterator<Item = &'a str>,
    ) -> Result<Self> {
        let physical_ancestry = FlowchartAncestryIndex::prepare(
            ctx.subgraphs_by_id,
            ctx.layout_clusters_by_id,
            ctx.parent,
            ctx.work_meter,
        )?;
        let effective_parents = ctx.node_schedule.effective_parents();
        let (root_offsets, extra_recursive_root_y) =
            flowchart_prepare_root_offsets(&ctx, effective_parents)?;

        if ctx.uses_flat_adapter_dom {
            ctx.work_meter.charge(ctx.edge_order.len())?;
            let edges = ctx.edge_order.to_vec();
            let mut edge_root_by_key = FxHashMap::default();
            edge_root_by_key.reserve(edges.len());
            for edge in edges.iter().copied() {
                if edge_root_by_key.insert(edge.key, None).is_some() {
                    return Err(flowchart_duplicate_edge_key_error(edge.key));
                }
            }
            let dom_order = ctx
                .node_schedule
                .top()
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>();
            let mut cluster_buckets =
                FlowchartRenderedClusterBuckets::prepare_flat(&ctx, cluster_ids)?;
            let clusters = cluster_buckets.take(None);
            let retained_refs = edges
                .len()
                .checked_add(dom_order.len())
                .and_then(|count| count.checked_add(clusters.len()))
                .ok_or_else(|| ctx.work_meter.arithmetic_overflow())?;
            ctx.work_meter.charge(retained_refs)?;
            return Ok(Self {
                uses_flat_adapter_dom: true,
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

        let mut cluster_buckets = FlowchartRenderedClusterBuckets::prepare(
            &ctx,
            effective_parents,
            &physical_ancestry,
            cluster_ids,
        )?;
        let mut top_edges = Vec::new();
        let mut edges_by_nested_root: FxHashMap<&'a str, Vec<FlowchartRenderEdgeRef<'a>>> =
            FxHashMap::default();
        let mut edge_root_by_key = FxHashMap::default();
        edge_root_by_key.reserve(ctx.edge_order.len());
        for edge in ctx.edge_order.iter().copied() {
            ctx.work_meter.charge(1)?;
            let root = flowchart_lca_with_work_meter(
                &ctx,
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

        let top_dom_order = ctx
            .node_schedule
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
        for (cluster_id, scheduled_order) in ctx.node_schedule.nested() {
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
                return Err(Error::InvalidModel {
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
            return Err(Error::InvalidModel {
                message: "Flowchart hierarchy does not cover every prepared render item"
                    .to_string(),
            });
        }
        Ok(Self {
            uses_flat_adapter_dom: false,
            top_root,
            nested_roots,
            ordered_edges,
            edge_root_by_key,
            effective_parents,
            root_offsets,
            extra_recursive_root_y,
        })
    }

    pub fn root(&self, cluster_id: Option<&str>) -> Result<&FlowchartRootEmission<'a>> {
        match cluster_id {
            None => Ok(&self.top_root),
            Some(cluster_id) => {
                self.nested_roots
                    .get(cluster_id)
                    .ok_or_else(|| Error::InvalidModel {
                        message: format!(
                            "missing prepared Flowchart root emission for `{cluster_id}`"
                        ),
                    })
            }
        }
    }

    pub fn ordered_edges(&self) -> &[FlowchartRenderEdgeRef<'a>] {
        self.ordered_edges
            .as_deref()
            .unwrap_or(self.top_root.edges())
    }

    pub fn painted_clusters(&self) -> std::borrow::Cow<'_, [&'a LayoutCluster]> {
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

    pub fn rendered_node_ids(&self) -> impl Iterator<Item = &'a str> + '_ {
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

    pub fn effective_parent(&self, id: &str) -> Option<&'a str> {
        self.effective_parents.parent(id)
    }

    pub fn edge_root(&self, key: FlowchartEdgeKey) -> Result<Option<&'a str>> {
        self.edge_root_by_key
            .get(&key)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!(
                    "missing prepared Flowchart hierarchy root for edge occurrence {}",
                    key.semantic_index()
                ),
            })
    }

    pub fn root_offsets(&self, cluster_id: &str) -> Option<FlowchartRootOffsets> {
        self.root_offsets.get(cluster_id).copied()
    }

    pub fn extra_recursive_root_y(&self) -> f64 {
        self.extra_recursive_root_y
    }
}

fn flowchart_duplicate_edge_key_error(key: FlowchartEdgeKey) -> Error {
    Error::InvalidModel {
        message: format!(
            "Flowchart hierarchy contains duplicate edge occurrence {}",
            key.semantic_index()
        ),
    }
}

fn flowchart_lca_with_work_meter<'a>(
    ctx: &FlowchartHierarchyInputs<'a>,
    a: &'a str,
    b: &'a str,
    parent_index: &'a FlowchartEffectiveParentIndex,
    work_meter: &OperationWorkMeter,
) -> Result<Option<&'a str>> {
    fn first_render_root<'a>(
        ctx: &FlowchartHierarchyInputs<'a>,
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

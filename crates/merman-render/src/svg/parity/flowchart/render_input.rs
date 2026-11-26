//! Flowchart render-time edge and helper-node preparation.

use std::borrow::Cow;
use std::collections::{BTreeSet, HashMap};

use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Debug)]
pub(in crate::svg::parity::flowchart) struct FlowchartRenderEdge<'a> {
    pub(in crate::svg::parity::flowchart) key: crate::flowchart::FlowchartEdgeKey,
    pub(in crate::svg::parity::flowchart) edge: Cow<'a, crate::flowchart::FlowEdge>,
}

impl FlowchartRenderEdge<'_> {
    pub(in crate::svg::parity::flowchart) fn as_ref(&self) -> FlowchartRenderEdgeRef<'_> {
        FlowchartRenderEdgeRef {
            key: self.key,
            edge: self.edge.as_ref(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(in crate::svg::parity) struct FlowchartRenderEdgeRef<'a> {
    pub(in crate::svg::parity::flowchart) key: crate::flowchart::FlowchartEdgeKey,
    pub(in crate::svg::parity::flowchart) edge: &'a crate::flowchart::FlowEdge,
}

impl std::ops::Deref for FlowchartRenderEdgeRef<'_> {
    type Target = crate::flowchart::FlowEdge;

    fn deref(&self) -> &Self::Target {
        self.edge
    }
}

pub(in crate::svg::parity::flowchart) struct FlowchartRenderInputs<'a> {
    pub render_edges: Vec<FlowchartRenderEdge<'a>>,
    pub extra_nodes: Vec<crate::flowchart::FlowNode>,
}

pub(super) fn flowchart_helper_node_ids(
    model: &crate::flowchart::FlowchartModel,
    render_context: &crate::flowchart::FlowchartRenderContext,
    owners: &crate::flowchart::FlowchartEdgeOwners,
    uses_elk: bool,
) -> BTreeSet<String> {
    if uses_elk {
        return BTreeSet::new();
    }
    owners
        .iter()
        .filter_map(|owner| model.edges.get(owner.semantic_index()))
        .filter_map(|edge| crate::flowchart::project_flowchart_edge(edge, render_context))
        .filter(|edge| edge.from == edge.to)
        .flat_map(|edge| {
            let from = edge.from;
            [
                format!("{from}---{from}---1"),
                format!("{from}---{from}---2"),
            ]
        })
        .collect()
}

pub(in crate::svg::parity::flowchart) fn prepare_flowchart_render_inputs<'a>(
    model: &'a crate::flowchart::FlowchartModel,
    render_context: &'a crate::flowchart::FlowchartRenderContext,
    layout_edge_owners: &crate::flowchart::FlowchartEdgeOwners,
    owner_occurrences: Option<HashMap<usize, u8>>,
    use_semantic_edges: bool,
) -> FlowchartRenderInputs<'a> {
    let merge_self_loops = use_semantic_edges
        || crate::flowchart::flowchart_dagre_merges_self_loop_segments(&model.keyword);
    let renderable_self_loop_nodes: FxHashSet<&str> = if merge_self_loops {
        FxHashSet::default()
    } else {
        layout_edge_owners
            .iter()
            .filter_map(|owner| model.edges.get(owner.semantic_index()))
            .filter_map(|edge| crate::flowchart::project_flowchart_edge(edge, render_context))
            .filter(|edge| edge.from == edge.to)
            .map(|edge| edge.from)
            .collect()
    };
    let semantic_edges = model
        .edges
        .iter()
        .enumerate()
        .filter_map(|(semantic_index, edge)| {
            let projected = crate::flowchart::project_flowchart_edge(edge, render_context)?;
            // Earlier overwritten helper triples still establish Graphlib's insertion slot.
            // Their later surviving owner replaces the complete value at that same slot.
            if owner_occurrences
                .as_ref()
                .is_some_and(|owners| !owners.contains_key(&semantic_index))
                && (merge_self_loops
                    || projected.from != projected.to
                    || !renderable_self_loop_nodes.contains(projected.from))
            {
                return None;
            }
            Some(FlowchartRenderEdge {
                key: crate::flowchart::FlowchartEdgeKey::new(semantic_index),
                edge: projected.into_render_edge(),
            })
        });
    if use_semantic_edges {
        return FlowchartRenderInputs {
            render_edges: semantic_edges.collect(),
            extra_nodes: Vec::new(),
        };
    }

    // Dagre always inserts helper nodes. Only the source's selected diagram types merge their
    // three segments back into a logical self-loop; Agentflow paints the helper edges directly.
    let self_loop_label_node_ids =
        flowchart_helper_node_ids(model, render_context, layout_edge_owners, false);
    let mut render_edges: Vec<FlowchartRenderEdge<'a>> = Vec::new();
    let mut helper_indexes: FxHashMap<String, usize> = FxHashMap::default();
    for edge in semantic_edges {
        if !merge_self_loops && edge.edge.from == edge.edge.to {
            let mut helpers = crate::flowchart::flowchart_self_loop_helper_edges(&edge.edge);
            helpers.edge_mid.label = render_context
                .edge_label_for_render(edge.key.semantic_index(), &edge.edge)
                .map(str::to_owned);
            // Graphlib overwrites an existing helper triple for parallel self-loops while
            // retaining its first insertion position. Keep each segment's semantic owner too.
            for (order, helper) in [helpers.edge1, helpers.edge_mid, helpers.edge2]
                .into_iter()
                .enumerate()
            {
                let key = crate::flowchart::FlowchartEdgeKey::self_loop_segment(
                    edge.key.semantic_index(),
                    order as u8,
                );
                if let Some(&index) = helper_indexes.get(helper.id.as_str()) {
                    render_edges[index] = FlowchartRenderEdge {
                        key,
                        edge: Cow::Owned(helper),
                    };
                } else {
                    helper_indexes.insert(helper.id.clone(), render_edges.len());
                    render_edges.push(FlowchartRenderEdge {
                        key,
                        edge: Cow::Owned(helper),
                    });
                }
            }
        } else {
            render_edges.push(edge);
        }
    }

    // Mermaid's `adjustClustersAndEdges(graph)` rewrites edges that connect directly to cluster
    // nodes by removing and re-adding them (after swapping endpoints to anchor nodes). This has a
    // visible side-effect: those edges end up later in `graph.edges()` insertion order, so the
    // DOM emitted under `.edgePaths` / `.edgeLabels` matches that stable partition.
    let cluster_ids_with_children: FxHashSet<&str> = model
        .subgraphs
        .iter()
        .filter(|sg| !sg.nodes.is_empty())
        .filter(|sg| !render_context.is_subgraph_collapsed(sg.id.as_str()))
        .filter(|sg| {
            render_context
                .collapsed_replacement(sg.id.as_str())
                .is_none()
        })
        .map(|sg| sg.id.as_str())
        .collect();
    if !cluster_ids_with_children.is_empty() && render_edges.len() >= 2 {
        let mut normal: Vec<FlowchartRenderEdge<'a>> = Vec::with_capacity(render_edges.len());
        let mut cluster: Vec<FlowchartRenderEdge<'a>> = Vec::new();
        for e in render_edges {
            let edge = e.edge.as_ref();
            if cluster_ids_with_children.contains(edge.from.as_str())
                || cluster_ids_with_children.contains(edge.to.as_str())
            {
                cluster.push(e);
            } else {
                normal.push(e);
            }
        }
        normal.extend(cluster);
        render_edges = normal;
    }

    // `getEdgesToRender` first emits every regular Graphlib edge, then appends merged self-loop
    // groups in first-seen order. Preserve that 11.16 DOM ordering independently of model order.
    if merge_self_loops && render_edges.len() >= 2 {
        let mut regular = Vec::with_capacity(render_edges.len());
        let mut self_loops = Vec::new();
        for edge in render_edges {
            if edge.edge.from == edge.edge.to {
                self_loops.push(edge);
            } else {
                regular.push(edge);
            }
        }
        regular.extend(self_loops);
        render_edges = regular;
    }

    let mut extra_nodes: Vec<crate::flowchart::FlowNode> =
        Vec::with_capacity(self_loop_label_node_ids.len());
    for id in &self_loop_label_node_ids {
        extra_nodes.push(crate::flowchart::FlowNode {
            id: id.clone(),
            provenance: Default::default(),
            label: Some(String::new()),
            label_type: None,
            layout_shape: None,
            shape: None,
            icon: None,
            form: None,
            pos: None,
            img: None,
            constraint: None,
            asset_width: None,
            asset_height: None,
            classes: Vec::new(),
            styles: Vec::new(),
            have_callback: false,
            link: None,
            link_target: None,
        });
    }

    FlowchartRenderInputs {
        render_edges,
        extra_nodes,
    }
}

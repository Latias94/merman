//! Owned node root order prepared once and shared with hierarchy emission.

use super::hierarchy::{
    FlowchartDomCandidateBuckets, FlowchartEffectiveParentIndex, flowchart_dom_order_for_root,
    flowchart_elk_dom_order,
};
use super::*;

#[derive(Clone, Copy)]
pub(crate) enum FlowchartNodeLayoutView<'a> {
    Flowchart(&'a FlowchartLayout),
    Swimlane(&'a crate::model::SwimlaneLayout),
}

#[derive(Clone, Copy)]
pub(super) enum NodeGeometry<'a> {
    Flowchart(&'a LayoutNode),
    Swimlane(&'a crate::model::SwimlaneNodeLayout),
}
impl NodeGeometry<'_> {
    pub(super) fn position(self) -> (f64, f64) {
        match self {
            Self::Flowchart(n) => (n.x, n.y),
            Self::Swimlane(n) => (n.x, n.y),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum ClusterGeometry<'a> {
    Flowchart(&'a LayoutCluster),
    Swimlane(
        &'a crate::model::SwimlaneLaneLayout,
        crate::model::SwimlaneDirection,
    ),
}
impl<'a> ClusterGeometry<'a> {
    pub(super) fn top_left(self) -> (f64, f64) {
        match self {
            Self::Flowchart(c) => (c.x - c.width / 2.0, c.y - c.height / 2.0),
            Self::Swimlane(c, dir) => {
                let geometry = super::swimlane::swimlane_terminal_geometry(c, dir);
                (c.x - geometry.width / 2.0, c.y - geometry.height / 2.0)
            }
        }
    }
    pub(super) fn direction(self) -> &'a str {
        match self {
            Self::Flowchart(c) => &c.effective_dir,
            Self::Swimlane(_, dir) => dir.as_str(),
        }
    }
}

pub(super) struct FlowchartNodeInventoryInput<'a> {
    pub(super) nodes_by_id: FxHashMap<&'a str, Option<&'a crate::flowchart::FlowNode>>,
    pub(super) subgraphs_by_id: FxHashMap<&'a str, &'a crate::flowchart::FlowSubgraph>,
    pub(super) subgraph_indices_by_id: FxHashMap<&'a str, usize>,
    pub(super) recursive_clusters: FxHashSet<&'a str>,
    pub(super) parent: FxHashMap<&'a str, &'a str>,
    pub(super) layout_nodes_by_id: FxHashMap<&'a str, NodeGeometry<'a>>,
    pub(super) layout_clusters_by_id: FxHashMap<&'a str, ClusterGeometry<'a>>,
    pub(super) dom_node_order_by_root:
        std::borrow::Cow<'a, std::collections::HashMap<String, Vec<String>>>,
    pub(super) node_dom_index: FxHashMap<&'a str, usize>,
    pub(super) uses_elk_adapter_dom: bool,
    subgraph_ids_with_children: FxHashSet<&'a str>,
    pub(super) work_meter: &'a crate::resources::OperationWorkMeter,
}

impl<'a> FlowchartNodeInventoryInput<'a> {
    fn prepare(
        model: &'a crate::flowchart::FlowchartModel,
        render_context: &'a crate::flowchart::FlowchartRenderContext,
        layout: FlowchartNodeLayoutView<'a>,
        helper_ids: &'a std::collections::BTreeSet<String>,
        work_meter: &'a crate::resources::OperationWorkMeter,
    ) -> Self {
        let mut nodes_by_id = model
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), Some(node)))
            .collect::<FxHashMap<_, _>>();
        for id in helper_ids {
            nodes_by_id.entry(id.as_str()).or_insert(None);
        }
        let mut subgraphs_by_id = FxHashMap::default();
        let mut subgraph_indices_by_id = FxHashMap::default();
        let mut subgraph_ids_with_children = FxHashSet::default();
        let mut parent = FxHashMap::default();
        for (index, subgraph) in model.subgraphs.iter().enumerate() {
            let id = subgraph.id.as_str();
            if let std::collections::hash_map::Entry::Vacant(entry) = subgraphs_by_id.entry(id) {
                entry.insert(subgraph);
                subgraph_indices_by_id.insert(id, index);
            }
            if !subgraph.nodes.is_empty()
                && !render_context.is_subgraph_collapsed(id)
                && render_context.collapsed_replacement(id).is_none()
            {
                subgraph_ids_with_children.insert(id);
            }
        }
        for subgraph in model.subgraphs.iter().rev() {
            for child in &subgraph.nodes {
                parent.insert(child.as_str(), subgraph.id.as_str());
            }
        }
        for id in helper_ids {
            if let Some((base, _)) = id.split_once("---")
                && let Some(owner) = parent.get(base).copied()
            {
                parent.insert(id, owner);
            }
        }
        let (
            layout_nodes_by_id,
            layout_clusters_by_id,
            dom_node_order_by_root,
            recursive_clusters,
            uses_elk_adapter_dom,
        ) = match layout {
            FlowchartNodeLayoutView::Flowchart(layout) => (
                layout
                    .nodes
                    .iter()
                    .map(|node| (node.id.as_str(), NodeGeometry::Flowchart(node)))
                    .collect(),
                layout
                    .clusters
                    .iter()
                    .map(|cluster| (cluster.id.as_str(), ClusterGeometry::Flowchart(cluster)))
                    .collect(),
                std::borrow::Cow::Borrowed(&layout.dom_node_order_by_root),
                layout
                    .dom_node_order_by_root
                    .keys()
                    .filter(|id| !id.is_empty())
                    .map(String::as_str)
                    .collect(),
                layout.uses_elk_adapter_dom,
            ),
            FlowchartNodeLayoutView::Swimlane(layout) => {
                let order = model
                    .nodes
                    .iter()
                    .map(|node| node.id.clone())
                    .chain(
                        layout
                            .nodes
                            .iter()
                            .filter(|node| node.is_edge_label)
                            .map(|node| node.id.clone()),
                    )
                    .collect();
                (
                    layout
                        .nodes
                        .iter()
                        .filter(|node| !node.is_edge_label)
                        .map(|node| (node.id.as_str(), NodeGeometry::Swimlane(node)))
                        .collect(),
                    layout
                        .lanes
                        .iter()
                        .map(|lane| {
                            (
                                lane.id.as_str(),
                                ClusterGeometry::Swimlane(lane, layout.direction),
                            )
                        })
                        .collect(),
                    std::borrow::Cow::Owned(std::collections::HashMap::from([(
                        String::new(),
                        order,
                    )])),
                    FxHashSet::default(),
                    false,
                )
            }
        };
        let mut node_dom_index = super::hierarchy::flowchart_node_dom_indices(model);
        for node in &model.nodes {
            if let Some(index) = render_context.node_dom_index(&node.id) {
                node_dom_index.insert(&node.id, index);
            }
        }
        Self {
            nodes_by_id,
            subgraphs_by_id,
            subgraph_indices_by_id,
            recursive_clusters,
            parent,
            layout_nodes_by_id,
            layout_clusters_by_id,
            dom_node_order_by_root,
            node_dom_index,
            uses_elk_adapter_dom,
            subgraph_ids_with_children,
            work_meter,
        }
    }

    pub(super) fn subgraph_has_children(&self, id: &str) -> bool {
        self.subgraph_ids_with_children.contains(id)
    }
}

#[derive(Debug)]
pub(crate) struct FlowchartNodeRootSchedule {
    top: Vec<String>,
    nested: Vec<(String, Vec<String>)>,
    effective_parents: FlowchartEffectiveParentIndex,
}

#[derive(Debug)]
pub(crate) struct FlowchartPreparedNodes {
    schedule: FlowchartNodeRootSchedule,
    terminals: FxHashMap<String, super::node_effect::PreparedNodeTerminal>,
}

impl FlowchartPreparedNodes {
    pub(crate) fn prepare(
        model: &crate::flowchart::FlowchartModel,
        render_context: &crate::flowchart::FlowchartRenderContext,
        layout: FlowchartNodeLayoutView<'_>,
        theme: Option<&crate::diagram_theme::ResolvedDiagramTheme>,
        config: &merman_core::MermaidConfig,
        prepared_theme: &crate::flowchart::FlowchartPreparedTheme,
        work: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Self> {
        let (owners, uses_elk) = match layout {
            FlowchartNodeLayoutView::Flowchart(layout) => {
                (&layout.edge_owners, layout.uses_elk_adapter_dom)
            }
            FlowchartNodeLayoutView::Swimlane(layout) => (&layout.edge_owners, false),
        };
        let helper_ids =
            super::render_input::flowchart_helper_node_ids(model, render_context, owners, uses_elk);
        let input =
            FlowchartNodeInventoryInput::prepare(model, render_context, layout, &helper_ids, work);
        let schedule = FlowchartNodeRootSchedule::prepare(&input)?;
        let ordinals = super::svg_emit::flowchart_node_theme_ordinals(
            &model.nodes,
            &model.subgraphs,
            uses_elk,
        );
        let render_model = crate::flowchart::FlowchartRenderModelRef::new(model, render_context);
        let selection_inputs =
            super::node_effect::NodeSelectionInputs::new(config, &prepared_theme.compatibility);
        let mut terminals = FxHashMap::default();
        for id in schedule
            .top
            .iter()
            .chain(schedule.nested.iter().flat_map(|(_, order)| order.iter()))
        {
            work.checkpoint(merman_core::OperationPhase::Layout)?;
            work.charge(1)?;
            if !input.layout_nodes_by_id.contains_key(id.as_str()) {
                continue;
            }
            let source_styles = if let Some(subgraph) = input.subgraphs_by_id.get(id.as_str())
                && (render_context.is_subgraph_collapsed(id) || !input.subgraph_has_children(id))
            {
                let Some(index) = input.subgraph_indices_by_id.get(id.as_str()).copied() else {
                    continue;
                };
                render_model.effective_subgraph_css(index, subgraph)
            } else if let Some(node) = input.nodes_by_id.get(id.as_str()) {
                node.map_or((&[][..], &[][..]), |node| {
                    (node.classes.as_slice(), node.styles.as_slice())
                })
            } else {
                continue;
            };
            let terminal = super::node_effect::PreparedNodeTerminal::prepare(
                &model.class_defs,
                source_styles,
                ordinals.get(id.as_str()).copied(),
                theme,
                &selection_inputs,
                work,
            )?;
            if terminals.insert(id.clone(), terminal).is_some() {
                return Err(crate::Error::InvalidModel {
                    message: format!("Flowchart emits themed node `{id}` more than once"),
                });
            }
        }
        Ok(Self {
            schedule,
            terminals,
        })
    }

    pub(super) fn schedule(&self) -> &FlowchartNodeRootSchedule {
        &self.schedule
    }
    pub(super) fn node(&self, id: &str) -> Option<&super::node_effect::PreparedNodeTerminal> {
        self.terminals.get(id)
    }
}

impl FlowchartNodeRootSchedule {
    pub(super) fn prepare(ctx: &FlowchartNodeInventoryInput<'_>) -> crate::Result<Self> {
        let effective_parents = FlowchartEffectiveParentIndex::prepare(
            &ctx.parent,
            |id| ctx.subgraphs_by_id.contains_key(id) && !ctx.recursive_clusters.contains(id),
            ctx.work_meter,
        )?;
        let (top, nested) = {
            let mut candidates = FlowchartDomCandidateBuckets::prepare(ctx, &effective_parents)?;
            if ctx.uses_elk_adapter_dom {
                let top = flowchart_elk_dom_order(ctx, candidates.take(None))?;
                (top.into_iter().map(str::to_owned).collect(), Vec::new())
            } else {
                ctx.work_meter
                    .checkpoint(merman_core::OperationPhase::Layout)?;
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
                    ctx.work_meter
                        .checkpoint(merman_core::OperationPhase::Layout)?;
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

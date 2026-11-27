//! Retained node and container terminals and their metered SVG document order.

use merman_model::{
    OperationPhase,
    families::flowchart::{FlowNode, FlowSubgraph, FlowchartModel, FlowchartRenderContext},
    presentation::PresentationConfig,
};
use merman_render_core::{
    Error, LayoutCluster, LayoutNode, Result, SwimlaneDirection, SwimlaneLaneLayout,
    SwimlaneNodeLayout,
    resources::{OperationWorkMeter, PreparedTextRetainedReservation},
    text::WrapMode,
};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    class_style::{
        FlowchartClassDefinitions, FlowchartLabelDom, FlowchartStructuralLabelTypography,
    },
    cluster_shell::swimlane_terminal_geometry,
    graph::{FlowchartNodeRootOrder, FlowchartNodeRootSchedule},
    hierarchy::{
        FlowchartEffectiveParentIndex, apply_solved_group_parents, flowchart_node_dom_indices,
    },
    layout_config::{FlowchartBaseTypographyPlan, FlowchartConfigView, FlowchartRenderConfig},
    projection::{FlowchartEdgeKey, FlowchartRenderModelRef, flowchart_helper_node_ids},
    style::{
        compiled::{
            FlowchartCompiledStyles, FlowchartNodeClassStyles, FlowchartPreparedClassStyles,
            flowchart_compile_prepared_styles,
        },
        flowchart_effective_text_style_for_classes,
        flowchart_effective_text_style_for_node_classes,
        node::{NodeSelectionInputs, PreparedNodeTerminal},
    },
    svg_label::{FlowchartNodeSourceTypography, FlowchartSvgLabelSidecar},
    theme::FlowchartPreparedTheme,
};
use order::{FlowchartDomCandidateBuckets, flowchart_dom_order_for_root, flowchart_elk_dom_order};

mod order;

/// The admitted layout geometry and route facts used to plan Flow SVG nodes.
#[derive(Clone, Copy)]
pub enum FlowchartNodeLayoutView<'a> {
    Flowchart {
        nodes: &'a [LayoutNode],
        clusters: &'a [LayoutCluster],
        dom_node_order_by_root: &'a std::collections::HashMap<String, Vec<String>>,
        edge_owners: &'a [FlowchartEdgeKey],
        solved_parents: Option<&'a [Option<u32>]>,
        uses_elk_adapter_dom: bool,
        uses_projected_edges: bool,
    },
    Swimlane {
        nodes: &'a [SwimlaneNodeLayout],
        lanes: &'a [SwimlaneLaneLayout],
        edge_owners: &'a [FlowchartEdgeKey],
        direction: SwimlaneDirection,
        canonical: bool,
    },
}

impl FlowchartNodeLayoutView<'_> {
    pub const fn uses_elk_adapter_dom(self) -> bool {
        matches!(
            self,
            Self::Flowchart {
                uses_elk_adapter_dom: true,
                ..
            }
        )
    }

    pub const fn is_swimlane(self) -> bool {
        matches!(self, Self::Swimlane { .. })
    }
}

#[derive(Clone, Copy)]
pub(super) enum NodeGeometry<'a> {
    Flowchart(&'a LayoutNode),
    Swimlane(&'a SwimlaneNodeLayout),
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
    Swimlane(&'a SwimlaneLaneLayout, SwimlaneDirection, bool),
}
impl<'a> ClusterGeometry<'a> {
    pub(super) fn top_left(self) -> (f64, f64) {
        match self {
            Self::Flowchart(c) => (c.x - c.width / 2.0, c.y - c.height / 2.0),
            Self::Swimlane(c, dir, canonical) => {
                let geometry = swimlane_terminal_geometry(c, dir, canonical);
                (c.x - geometry.width / 2.0, c.y - geometry.height / 2.0)
            }
        }
    }
    pub(super) fn direction(self) -> &'a str {
        match self {
            Self::Flowchart(c) => &c.effective_dir,
            Self::Swimlane(c, dir, true) => c.requested_dir.as_deref().unwrap_or(dir.as_str()),
            Self::Swimlane(_, dir, false) => dir.as_str(),
        }
    }
}

pub(super) struct FlowchartNodeInventoryInput<'a> {
    pub(super) model_nodes: &'a [FlowNode],
    pub(super) nodes_by_id: FxHashMap<&'a str, Option<&'a FlowNode>>,
    pub(super) subgraphs_by_id: FxHashMap<&'a str, &'a FlowSubgraph>,
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
    pub(super) work_meter: &'a OperationWorkMeter,
}

impl<'a> FlowchartNodeInventoryInput<'a> {
    fn prepare(
        model: &'a FlowchartModel,
        render_context: &'a FlowchartRenderContext,
        layout: FlowchartNodeLayoutView<'a>,
        helper_ids: &'a std::collections::BTreeSet<String>,
        work_meter: &'a OperationWorkMeter,
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
        let mut parent: FxHashMap<_, _> = FlowchartRenderModelRef::new(model, render_context)
            .visible_memberships()
            .collect();
        if let FlowchartNodeLayoutView::Flowchart {
            clusters,
            solved_parents,
            ..
        } = layout
        {
            apply_solved_group_parents(
                clusters,
                solved_parents,
                &mut parent,
                &mut subgraph_ids_with_children,
            );
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
            FlowchartNodeLayoutView::Flowchart {
                nodes,
                clusters,
                dom_node_order_by_root,
                uses_elk_adapter_dom,
                ..
            } => (
                nodes
                    .iter()
                    .map(|node| (node.id.as_str(), NodeGeometry::Flowchart(node)))
                    .collect(),
                clusters
                    .iter()
                    .map(|cluster| (cluster.id.as_str(), ClusterGeometry::Flowchart(cluster)))
                    .collect(),
                std::borrow::Cow::Borrowed(dom_node_order_by_root),
                dom_node_order_by_root
                    .keys()
                    .filter(|id| !id.is_empty())
                    .map(String::as_str)
                    .collect(),
                uses_elk_adapter_dom,
            ),
            FlowchartNodeLayoutView::Swimlane {
                nodes,
                lanes,
                direction,
                canonical,
                ..
            } => {
                if canonical {
                    for lane in lanes {
                        subgraph_ids_with_children.insert(lane.id.as_str());
                        if let Some(owner) = &lane.parent_id {
                            parent.insert(&lane.id, owner);
                        } else {
                            parent.remove(lane.id.as_str());
                        }
                    }
                    for node in nodes {
                        if let Some(owner) = &node.parent_id {
                            parent.insert(&node.id, owner);
                        }
                    }
                }
                // FlowDB emits visible collapsed groups before authored vertices.
                let order = nodes
                    .iter()
                    .filter(|node| {
                        !node.is_edge_label && render_context.is_subgraph_collapsed(&node.id)
                    })
                    .map(|node| node.id.clone())
                    .chain(
                        model
                            .nodes
                            .iter()
                            .filter(|node| {
                                !render_context.is_subgraph_collapsed(&node.id)
                                    && render_context.collapsed_replacement(&node.id).is_none()
                            })
                            .map(|node| node.id.clone()),
                    )
                    .chain(
                        nodes
                            .iter()
                            .filter(|node| node.is_edge_label)
                            .map(|node| node.id.clone()),
                    )
                    .collect();
                (
                    nodes
                        .iter()
                        .filter(|node| !node.is_edge_label)
                        .map(|node| (node.id.as_str(), NodeGeometry::Swimlane(node)))
                        .collect(),
                    lanes
                        .iter()
                        .map(|lane| {
                            (
                                lane.id.as_str(),
                                ClusterGeometry::Swimlane(lane, direction, canonical),
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
        let mut node_dom_index = flowchart_node_dom_indices(model);
        for node in &model.nodes {
            if let Some(index) = render_context.node_dom_index(&node.id) {
                node_dom_index.insert(&node.id, index);
            }
        }
        Self {
            model_nodes: &model.nodes,
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
pub struct FlowchartPreparedNodes {
    schedule: FlowchartNodeRootSchedule,
    terminals: FxHashMap<String, PreparedNodeTerminal>,
    cluster_ids: Vec<String>,
    clusters: FxHashMap<String, FlowchartPreparedCluster>,
    _cluster_retained_reservation: PreparedTextRetainedReservation,
}

#[cfg(all(feature = "test-support", feature = "diagram-flowchart"))]
thread_local! {
    static PAINT_TABLE_ENTRIES_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[derive(Debug)]
pub struct FlowchartPreparedCluster {
    pub compiled: FlowchartCompiledStyles,
    pub rect_style: String,
    pub fill: String,
    pub stroke: String,
    pub title_text_style: std::sync::Arc<FlowchartNodeSourceTypography>,
}

impl FlowchartPreparedNodes {
    #[cfg(all(feature = "test-support", feature = "diagram-flowchart"))]
    pub fn paint_table_entries_built() -> usize {
        PAINT_TABLE_ENTRIES_BUILT.get()
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Node terminals bind existing layout and label artifacts to the operation theme"
    )]
    pub fn prepare(
        model: &FlowchartModel,
        render_context: &FlowchartRenderContext,
        class_defs: &FlowchartClassDefinitions<'_>,
        layout: FlowchartNodeLayoutView<'_>,
        svg_labels: &FlowchartSvgLabelSidecar,
        config: &PresentationConfig,
        prepared_theme: &FlowchartPreparedTheme,
        render_config: &FlowchartRenderConfig,
        work: &std::sync::Arc<OperationWorkMeter>,
        prepare_paint: bool,
    ) -> Result<Self> {
        let swimlane = layout.is_swimlane();
        let typography = svg_labels.base_typography().map_or_else(
            || FlowchartBaseTypographyPlan::resolve(config).render_styles(config),
            |plan| plan.render_styles(config),
        );
        let source_base = if FlowchartConfigView::new(config).node_wrap_mode() == WrapMode::HtmlLike
        {
            &typography.html_label_text_style
        } else {
            &typography.text_style
        };
        let (owners, use_projected_edges) = match layout {
            FlowchartNodeLayoutView::Flowchart {
                edge_owners,
                uses_projected_edges,
                ..
            } => (edge_owners, uses_projected_edges),
            FlowchartNodeLayoutView::Swimlane {
                edge_owners,
                canonical,
                ..
            } => (edge_owners, canonical),
        };
        let helper_ids =
            flowchart_helper_node_ids(model, render_context, owners, use_projected_edges);
        let input =
            FlowchartNodeInventoryInput::prepare(model, render_context, layout, &helper_ids, work);
        let schedule = prepare_flowchart_node_root_schedule(&input)?;
        let ordinals = flowchart_node_theme_ordinals(
            &model.nodes,
            &model.subgraphs,
            render_context,
            use_projected_edges,
        );
        let render_model = FlowchartRenderModelRef::new(model, render_context);
        let selection_inputs = NodeSelectionInputs::new(
            &prepared_theme.compatibility,
            render_config,
            prepared_theme.native.as_ref(),
        );
        let mut node_classes = FlowchartNodeClassStyles::new(class_defs.source());
        let mut terminals = FxHashMap::default();
        let mut admitted_nodes = FxHashSet::default();
        // Any typography missing from the label sidecar still reserves retained bytes.
        // Hold those reservations through cluster admission on both paths.
        let mut layout_typography = Vec::new();
        for id in schedule
            .top()
            .iter()
            .chain(schedule.nested().iter().flat_map(|(_, order)| order.iter()))
        {
            work.checkpoint(OperationPhase::Layout)?;
            work.charge(1)?;
            if !input.layout_nodes_by_id.contains_key(id.as_str()) {
                continue;
            }
            let (source_styles, terminal_shape, label_source) = if let Some(subgraph) =
                input.subgraphs_by_id.get(id.as_str())
                && (render_context.is_subgraph_collapsed(id) || !input.subgraph_has_children(id))
            {
                let Some(index) = input.subgraph_indices_by_id.get(id.as_str()).copied() else {
                    continue;
                };
                let shape = if render_context.is_subgraph_collapsed(id) {
                    "collapsedGroup"
                } else {
                    "squareRect"
                };
                (
                    render_model.effective_subgraph_css(index, subgraph),
                    Some(shape),
                    (
                        render_model.subgraph_title_for_render(index, subgraph),
                        subgraph.label_type.as_deref().unwrap_or("text"),
                    ),
                )
            } else if let Some(node) = input.nodes_by_id.get(id.as_str()) {
                (
                    node.map_or((&[][..], &[][..]), |node| {
                        (node.classes.as_slice(), node.styles.as_slice())
                    }),
                    node.as_ref().map(|node| {
                        // Match resolve_node_render_info, including the direct-model fallback.
                        node.layout_shape.as_deref().unwrap_or("squareRect")
                    }),
                    node.map_or((id.as_str(), "text"), |node| {
                        (
                            render_model
                                .node_label_for_render(node)
                                .unwrap_or(node.id.as_str()),
                            node.label_type.as_deref().unwrap_or("text"),
                        )
                    }),
                )
            } else {
                continue;
            };
            let label_dom = (
                prepared_theme.compatibility.html_labels,
                FlowchartLabelDom::node(
                    terminal_shape.unwrap_or("squareRect"),
                    label_source.0,
                    label_source.1,
                    render_config.node_html_labels,
                    prepared_theme.compatibility.look.is_hand_drawn(),
                ),
            );
            let structural = FlowchartStructuralLabelTypography::prepare(
                class_defs,
                label_dom.0,
                label_dom.1,
                |units| work.charge(units).map_err(Into::into),
            )?;
            let include_default_class = !render_context.is_subgraph_collapsed(id);
            let (classes, styles) = source_styles;
            let source_text_style = match svg_labels
                .node_owner(id, swimlane)
                .and_then(|owner| svg_labels.node_source_style(owner))
            {
                Some(style) => std::borrow::Cow::Borrowed(style),
                None => {
                    let parent =
                        structural.apply_to_parent(std::borrow::Cow::Borrowed(source_base));
                    let mut style = if include_default_class {
                        flowchart_effective_text_style_for_node_classes
                    } else {
                        flowchart_effective_text_style_for_classes
                    }(
                        parent.as_ref(), &model.class_defs, classes, styles
                    );
                    structural.apply_to_content(&mut style);
                    std::borrow::Cow::Owned(std::sync::Arc::new(
                        FlowchartNodeSourceTypography::prepare(
                            std::borrow::Cow::Owned(style.into_owned()),
                            Some(work),
                        )?,
                    ))
                }
            };
            let source_text_style = if prepare_paint {
                Some(source_text_style.into_owned())
            } else {
                if let std::borrow::Cow::Owned(style) = source_text_style {
                    layout_typography.push(style);
                }
                None
            };
            let occurrence_inputs =
                if model.keyword == "agentflow-beta" && render_context.is_subgraph_collapsed(id) {
                    selection_inputs.for_collapsed_agentflow()
                } else {
                    selection_inputs
                };
            let terminal = PreparedNodeTerminal::prepare(
                class_defs,
                &mut node_classes,
                (classes, styles, include_default_class),
                terminal_shape,
                &structural,
                source_text_style,
                ordinals.get(id.as_str()).copied(),
                &occurrence_inputs,
                work,
            )?;
            let duplicate = if let Some(terminal) = terminal {
                #[cfg(all(feature = "test-support", feature = "diagram-flowchart"))]
                PAINT_TABLE_ENTRIES_BUILT.set(PAINT_TABLE_ENTRIES_BUILT.get() + 1);
                terminals.insert(id.clone(), terminal).is_some()
            } else {
                !admitted_nodes.insert(id.as_str())
            };
            if duplicate {
                return Err(Error::InvalidModel {
                    message: format!("Flowchart emits themed node `{id}` more than once"),
                });
            }
        }
        drop(node_classes);
        let cluster_ids = prepared_cluster_ids(model, layout, &input, &schedule)?;
        let render_model = FlowchartRenderModelRef::new(model, render_context);
        let class_styles = FlowchartPreparedClassStyles::prepare(
            &model.class_defs,
            cluster_ids.iter().flat_map(|id| {
                input
                    .subgraphs_by_id
                    .get(id.as_str())
                    .zip(input.subgraph_indices_by_id.get(id.as_str()))
                    .map(|(subgraph, index)| {
                        render_model.effective_subgraph_css(*index, subgraph).0
                    })
                    .unwrap_or_default()
                    .iter()
                    .map(String::as_str)
            }),
            Some(work),
        )?;
        let mut clusters = FxHashMap::default();
        let mut cluster_retained_bytes = cluster_ids.iter().fold(0usize, |bytes, id| {
            bytes
                .saturating_add(std::mem::size_of::<FlowchartPreparedCluster>())
                .saturating_add(std::mem::size_of::<String>() * 2)
                .saturating_add(id.len().saturating_mul(2))
        });
        let mut remaining = cluster_ids
            .iter()
            .map(String::as_str)
            .collect::<FxHashSet<_>>();
        let top_lane_ids = match layout {
            FlowchartNodeLayoutView::Swimlane { lanes, .. } => lanes
                .iter()
                .filter(|lane| lane.parent_id.is_none())
                .map(|lane| lane.id.as_str())
                .collect::<FxHashSet<_>>(),
            FlowchartNodeLayoutView::Flowchart { .. } => FxHashSet::default(),
        };
        let lane_ids = match layout {
            FlowchartNodeLayoutView::Swimlane { lanes, .. } => lanes
                .iter()
                .map(|lane| lane.id.as_str())
                .collect::<Vec<_>>(),
            FlowchartNodeLayoutView::Flowchart { .. } => Vec::new(),
        };
        for id in model
            .subgraphs
            .iter()
            .map(|subgraph| subgraph.id.as_str())
            .chain(lane_ids)
        {
            work.charge(1)?;
            if !remaining.remove(id) {
                continue;
            }
            let source = input
                .subgraphs_by_id
                .get(id)
                .zip(input.subgraph_indices_by_id.get(id))
                .map(|(subgraph, index)| render_model.effective_subgraph_css(*index, subgraph))
                .unwrap_or_default();
            let compiled = flowchart_compile_prepared_styles(
                &class_styles,
                source.0,
                source.1,
                &[],
                Some(work),
            )?;
            let fill = (model.keyword == "agentflow-beta")
                .then_some(compiled.fill.as_deref())
                .flatten()
                .unwrap_or(&render_config.cluster_fill_color)
                .to_owned();
            let stroke = render_config.cluster_stroke_color.clone();
            let rect_style = compiled.node_style.trim().to_owned();
            let lane = top_lane_ids.contains(id);
            let owner = if lane {
                svg_labels.swimlane_group_title_owner(id)
            } else {
                svg_labels.subgraph_title_owner(id)
            };
            let title_text_style = match owner
                .and_then(|owner| svg_labels.node_source_style(owner))
                .cloned()
            {
                Some(style) => style,
                None => {
                    let base = if !lane && render_config.edge_html_labels {
                        &render_config.html_label_text_style
                    } else {
                        &render_config.text_style
                    };
                    let style = flowchart_effective_text_style_for_classes(
                        base,
                        &model.class_defs,
                        source.0,
                        source.1,
                    )
                    .into_owned();
                    std::sync::Arc::new(FlowchartNodeSourceTypography::prepare(
                        std::borrow::Cow::Owned(style),
                        Some(work),
                    )?)
                }
            };
            cluster_retained_bytes = cluster_retained_bytes
                .saturating_add(rect_style.len())
                .saturating_add(fill.len())
                .saturating_add(stroke.len());
            if prepare_paint {
                #[cfg(all(feature = "test-support", feature = "diagram-flowchart"))]
                PAINT_TABLE_ENTRIES_BUILT.set(PAINT_TABLE_ENTRIES_BUILT.get() + 1);
                clusters.insert(
                    id.to_owned(),
                    FlowchartPreparedCluster {
                        compiled,
                        rect_style,
                        fill,
                        stroke,
                        title_text_style,
                    },
                );
            } else {
                layout_typography.push(title_text_style);
            }
        }
        if !remaining.is_empty() {
            return Err(Error::InvalidModel {
                message: "Flowchart hierarchy emits clusters outside the semantic inventory".into(),
            });
        }
        Ok(Self {
            schedule,
            terminals,
            cluster_ids,
            clusters,
            _cluster_retained_reservation: work
                .reserve_prepared_text_retained_bytes(cluster_retained_bytes)?,
        })
    }

    pub fn schedule(&self) -> &FlowchartNodeRootSchedule {
        &self.schedule
    }
    pub fn node(&self, id: &str) -> Option<&PreparedNodeTerminal> {
        self.terminals.get(id)
    }
    pub fn cluster_ids(&self) -> impl Iterator<Item = &str> {
        self.cluster_ids.iter().map(String::as_str)
    }
    pub fn cluster(&self, id: &str) -> Result<&FlowchartPreparedCluster> {
        self.clusters.get(id).ok_or_else(|| Error::InvalidModel {
            message: format!("missing prepared Flowchart cluster `{id}`"),
        })
    }
}

fn prepared_cluster_ids(
    model: &FlowchartModel,
    layout: FlowchartNodeLayoutView<'_>,
    input: &FlowchartNodeInventoryInput<'_>,
    schedule: &FlowchartNodeRootSchedule,
) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    let scheduled_roots = schedule
        .nested()
        .iter()
        .map(|(id, _)| id.as_str())
        .collect::<FxHashSet<_>>();
    if input.uses_elk_adapter_dom {
        ids.extend(
            input
                .dom_node_order_by_root
                .get("")
                .into_iter()
                .flatten()
                .filter(|id| {
                    input.subgraphs_by_id.contains_key(id.as_str())
                        && input.layout_clusters_by_id.contains_key(id.as_str())
                })
                .cloned(),
        );
        if ids.is_empty() {
            let mut seen = FxHashSet::default();
            ids.extend(
                model
                    .subgraphs
                    .iter()
                    .filter(|subgraph| {
                        seen.insert(subgraph.id.as_str())
                            && input
                                .layout_clusters_by_id
                                .contains_key(subgraph.id.as_str())
                    })
                    .map(|subgraph| subgraph.id.clone()),
            );
        }
    } else if let FlowchartNodeLayoutView::Swimlane { lanes, .. } = layout {
        ids.extend(lanes.iter().map(|lane| lane.id.clone()));
    } else {
        for id in input.subgraphs_by_id.keys().copied() {
            if !input.subgraph_has_children(id) || !input.layout_clusters_by_id.contains_key(id) {
                continue;
            }
            let root = if input.recursive_clusters.contains(id) {
                Some(id)
            } else {
                schedule.effective_parents().parent(id)
            };
            if root.is_none_or(|root| scheduled_roots.contains(root)) {
                ids.push(id.to_owned());
            }
        }
    }
    let mut seen = FxHashSet::default();
    for id in &ids {
        input.work_meter.charge(1)?;
        if !seen.insert(id.as_str()) {
            return Err(Error::InvalidModel {
                message: format!("Flowchart hierarchy emits cluster `{id}` more than once"),
            });
        }
    }
    Ok(ids)
}

fn prepare_flowchart_node_root_schedule(
    ctx: &FlowchartNodeInventoryInput<'_>,
) -> Result<FlowchartNodeRootSchedule> {
    let effective_parents = FlowchartEffectiveParentIndex::prepare(
        &ctx.parent,
        |id| {
            (ctx.subgraphs_by_id.contains_key(id) || ctx.layout_clusters_by_id.contains_key(id))
                && !ctx.recursive_clusters.contains(id)
        },
        ctx.work_meter,
    )?;
    let mut candidates = FlowchartDomCandidateBuckets::prepare(ctx, &effective_parents)?;
    let order = if ctx.uses_elk_adapter_dom {
        FlowchartNodeRootOrder::flat(flowchart_elk_dom_order(ctx, candidates.take(None))?)
    } else {
        FlowchartNodeRootOrder::prepare_nested(
            &ctx.subgraphs_by_id,
            &ctx.subgraph_ids_with_children,
            &ctx.recursive_clusters,
            ctx.work_meter,
            |root| flowchart_dom_order_for_root(ctx, root, candidates.take(root)),
        )?
    };
    Ok(FlowchartNodeRootSchedule::new(effective_parents, order))
}

fn flowchart_node_theme_ordinals<'a>(
    nodes: &'a [FlowNode],
    subgraphs: &'a [FlowSubgraph],
    render_context: &FlowchartRenderContext,
    uses_elk_adapter_dom: bool,
) -> FxHashMap<&'a str, usize> {
    let subgraph_ids: FxHashSet<&str> = subgraphs
        .iter()
        .map(|subgraph| subgraph.id.as_str())
        .collect();
    let mut ordinals = FxHashMap::default();
    let mut next_ordinal = 1usize;

    // Theme ordinals describe surfaces that reach the Node emitter, not Graphlib/ELK vertices.
    // ELK materializes every subgraph as a node in the semantic model, but renders it as a
    // cluster. Collapsed groups are nodes in either backend; hidden descendants own no slot.
    for node in nodes {
        if subgraph_ids.contains(node.id.as_str())
            || render_context
                .collapsed_replacement(node.id.as_str())
                .is_some()
        {
            continue;
        }
        if let std::collections::hash_map::Entry::Vacant(entry) = ordinals.entry(node.id.as_str()) {
            entry.insert(next_ordinal);
            next_ordinal += 1;
        }
    }

    for subgraph in subgraphs.iter().filter(|subgraph| {
        render_context.collapsed_replacement(&subgraph.id).is_none()
            && (render_context.is_subgraph_collapsed(&subgraph.id)
                || !uses_elk_adapter_dom && subgraph.nodes.is_empty())
    }) {
        if let std::collections::hash_map::Entry::Vacant(entry) =
            ordinals.entry(subgraph.id.as_str())
        {
            entry.insert(next_ordinal);
            next_ordinal += 1;
        }
    }

    ordinals
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str) -> FlowNode {
        FlowNode {
            id: id.to_string(),
            provenance: Default::default(),
            label: None,
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
            link: None,
            link_target: None,
            have_callback: false,
        }
    }

    fn subgraph(id: &str, nodes: &[&str]) -> FlowSubgraph {
        FlowSubgraph {
            id: id.to_string(),
            title: id.to_string(),
            dir: None,
            has_explicit_dir: false,
            label_type: None,
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: nodes.iter().map(|node| (*node).to_string()).collect(),
            metadata: Default::default(),
        }
    }

    #[test]
    fn node_theme_ordinals_follow_actual_node_emitter_surfaces() {
        let nodes = vec![node("A"), node("group"), node("A"), node("empty")];
        let subgraphs = vec![subgraph("group", &["A"]), subgraph("empty", &[])];

        let context = FlowchartRenderContext::default();
        let dagre = flowchart_node_theme_ordinals(&nodes, &subgraphs, &context, false);
        assert_eq!(dagre.len(), 2);
        assert_eq!(dagre.get("A"), Some(&1));
        assert_eq!(dagre.get("empty"), Some(&2));
        assert!(!dagre.contains_key("group"));

        let elk = flowchart_node_theme_ordinals(&nodes, &subgraphs, &context, true);
        assert_eq!(elk.len(), 1);
        assert_eq!(elk.get("A"), Some(&1));
        assert!(!elk.contains_key("group"));
        assert!(!elk.contains_key("empty"));
    }
}

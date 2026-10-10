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
    cluster_ids: Vec<String>,
    clusters: FxHashMap<String, FlowchartPreparedCluster>,
    _cluster_retained_reservation: crate::resources::PreparedTextRetainedReservation,
}

#[derive(Debug)]
pub(super) struct FlowchartPreparedCluster {
    pub(super) theme: crate::flowchart::FlowchartClusterThemeStyle,
    pub(super) compiled: FlowchartCompiledStyles,
    pub(super) rect_style: String,
    pub(super) fill: String,
    pub(super) stroke: String,
    pub(super) typed_fill: Option<String>,
    pub(super) typed_stroke_width: Option<f32>,
    pub(super) fill_precedence: crate::flowchart::FlowchartFacetPrecedence,
    pub(super) stroke_precedence: crate::flowchart::FlowchartFacetPrecedence,
    pub(super) stroke_width_precedence: crate::flowchart::FlowchartFacetPrecedence,
    pub(super) title_text_style: std::sync::Arc<crate::flowchart::FlowchartNodeSourceTypography>,
}

impl FlowchartPreparedNodes {
    #[allow(
        clippy::too_many_arguments,
        reason = "Node terminals bind existing layout and label artifacts to the operation theme"
    )]
    pub(crate) fn prepare(
        model: &crate::flowchart::FlowchartModel,
        render_context: &crate::flowchart::FlowchartRenderContext,
        layout: FlowchartNodeLayoutView<'_>,
        svg_labels: &crate::flowchart::FlowchartSvgLabelSidecar,
        theme: Option<&crate::diagram_theme::ResolvedDiagramTheme>,
        config: &merman_core::MermaidConfig,
        prepared_theme: &crate::flowchart::FlowchartPreparedTheme,
        render_config: &super::render_config::FlowchartRenderConfig,
        work: &std::sync::Arc<crate::resources::OperationWorkMeter>,
    ) -> crate::Result<Self> {
        let swimlane = matches!(layout, FlowchartNodeLayoutView::Swimlane(_));
        let typography = svg_labels.base_typography().map_or_else(
            || {
                crate::flowchart::FlowchartBaseTypographyPlan::resolve(theme, config)
                    .render_styles(config.as_value())
            },
            |plan| plan.render_styles(config.as_value()),
        );
        let source_base = if crate::flowchart::FlowchartConfigView::new(config.as_value())
            .node_wrap_mode()
            == crate::text::WrapMode::HtmlLike
        {
            &typography.html_label_text_style
        } else {
            &typography.text_style
        };
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
        let selection_inputs = super::node_effect::NodeSelectionInputs::new(
            &prepared_theme.compatibility,
            render_config,
        );
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
            let source_text_style = match svg_labels
                .node_owner(id, swimlane)
                .and_then(|owner| svg_labels.node_source_style(owner))
                .cloned()
            {
                Some(style) => style,
                None => {
                    let style = crate::flowchart::flowchart_effective_text_style_for_node_classes(
                        source_base,
                        &model.class_defs,
                        source_styles.0,
                        source_styles.1,
                    )
                    .into_owned();
                    std::sync::Arc::new(
                        crate::flowchart::FlowchartNodeSourceTypography::from_style(style, work)?,
                    )
                }
            };
            let terminal = super::node_effect::PreparedNodeTerminal::prepare(
                &model.class_defs,
                source_styles,
                source_text_style,
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
        let cluster_ids = prepared_cluster_ids(model, layout, &input, &schedule)?;
        let render_model = crate::flowchart::FlowchartRenderModelRef::new(model, render_context);
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
            FlowchartNodeLayoutView::Swimlane(layout) => layout
                .lanes
                .iter()
                .filter(|lane| lane.parent_id.is_none())
                .map(|lane| lane.id.as_str())
                .collect::<FxHashSet<_>>(),
            FlowchartNodeLayoutView::Flowchart(_) => FxHashSet::default(),
        };
        let lane_ids = match layout {
            FlowchartNodeLayoutView::Swimlane(layout) => layout
                .lanes
                .iter()
                .map(|lane| lane.id.as_str())
                .collect::<Vec<_>>(),
            FlowchartNodeLayoutView::Flowchart(_) => Vec::new(),
        };
        let mut ordinal = 1usize;
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
            let theme =
                crate::flowchart::FlowchartClusterThemeStyle::resolve(theme, Some(ordinal), work)?;
            ordinal = ordinal
                .checked_add(1)
                .ok_or_else(|| work.arithmetic_overflow())?;
            let fill_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
                compiled.source_fill_status(),
                render_config.cluster_fill_config_override,
            );
            let stroke_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
                compiled.source_stroke_status(),
                render_config.cluster_stroke_config_override,
            );
            let stroke_width_precedence = crate::flowchart::FlowchartFacetPrecedence::new(
                compiled.source_stroke_width_status(),
                false,
            );
            let typed_fill = theme.fill_value(fill_precedence, true).map(str::to_owned);
            let fill = typed_fill
                .as_deref()
                .unwrap_or(&render_config.cluster_fill_color)
                .to_owned();
            let stroke = theme
                .stroke_value(stroke_precedence, true)
                .unwrap_or(&render_config.cluster_stroke_color)
                .to_owned();
            let typed_stroke_width = theme.stroke_width_value(stroke_width_precedence);
            let mut rect_style = compiled.node_style.trim().to_owned();
            theme.append_inline_style(
                &mut rect_style,
                fill_precedence,
                stroke_precedence,
                stroke_width_precedence,
            );
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
                    let style = crate::flowchart::flowchart_effective_text_style_for_classes(
                        base,
                        &model.class_defs,
                        source.0,
                        source.1,
                    )
                    .into_owned();
                    std::sync::Arc::new(
                        crate::flowchart::FlowchartNodeSourceTypography::from_style(style, work)?,
                    )
                }
            };
            cluster_retained_bytes = cluster_retained_bytes
                .saturating_add(rect_style.len())
                .saturating_add(fill.len())
                .saturating_add(stroke.len())
                .saturating_add(typed_fill.as_ref().map_or(0, String::len));
            clusters.insert(
                id.to_owned(),
                FlowchartPreparedCluster {
                    theme,
                    compiled,
                    rect_style,
                    fill,
                    stroke,
                    typed_fill,
                    typed_stroke_width,
                    fill_precedence,
                    stroke_precedence,
                    stroke_width_precedence,
                    title_text_style,
                },
            );
        }
        if !remaining.is_empty() {
            return Err(crate::Error::InvalidModel {
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

    pub(super) fn schedule(&self) -> &FlowchartNodeRootSchedule {
        &self.schedule
    }
    pub(super) fn node(&self, id: &str) -> Option<&super::node_effect::PreparedNodeTerminal> {
        self.terminals.get(id)
    }
    pub(super) fn cluster_ids(&self) -> impl Iterator<Item = &str> {
        self.cluster_ids.iter().map(String::as_str)
    }
    pub(super) fn cluster(&self, id: &str) -> crate::Result<&FlowchartPreparedCluster> {
        self.clusters
            .get(id)
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!("missing prepared Flowchart cluster `{id}`"),
            })
    }
}

fn prepared_cluster_ids(
    model: &crate::flowchart::FlowchartModel,
    layout: FlowchartNodeLayoutView<'_>,
    input: &FlowchartNodeInventoryInput<'_>,
    schedule: &FlowchartNodeRootSchedule,
) -> crate::Result<Vec<String>> {
    let mut ids = Vec::new();
    let scheduled_roots = schedule
        .nested
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
    } else {
        for id in input.subgraphs_by_id.keys().copied() {
            if !input.subgraph_has_children(id) || !input.layout_clusters_by_id.contains_key(id) {
                continue;
            }
            let root = if input.recursive_clusters.contains(id) {
                Some(id)
            } else {
                schedule.effective_parents.parent(id)
            };
            if root.is_none_or(|root| scheduled_roots.contains(root)) {
                ids.push(id.to_owned());
            }
        }
        if let FlowchartNodeLayoutView::Swimlane(layout) = layout {
            ids.extend(
                layout
                    .lanes
                    .iter()
                    .filter(|lane| {
                        !input.subgraphs_by_id.contains_key(lane.id.as_str())
                            && input.layout_clusters_by_id.contains_key(lane.id.as_str())
                            && lane
                                .parent_id
                                .as_deref()
                                .is_none_or(|root| scheduled_roots.contains(root))
                    })
                    .map(|lane| lane.id.clone()),
            );
        }
    }
    let mut seen = FxHashSet::default();
    for id in &ids {
        input.work_meter.charge(1)?;
        if !seen.insert(id.as_str()) {
            return Err(crate::Error::InvalidModel {
                message: format!("Flowchart hierarchy emits cluster `{id}` more than once"),
            });
        }
    }
    Ok(ids)
}

#[cfg(test)]
mod source_typography_tests {
    use super::*;

    #[test]
    fn cluster_terminal_uses_first_source_owner_and_releases_retained_paint() {
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
        let parsed = merman_core::Engine::new()
            .parse_diagram_for_render_model_sync(
                "---\nconfig:\n  layout: dagre\n  htmlLabels: false\n---\nflowchart LR\nclassDef first fill:#112233,font-size:19px\nclassDef last fill:#aabbcc,font-size:27px\nsubgraph Shared[First]\nA\nend\nclass Shared first\nsubgraph Shared[Last]\nB\nend\n",
                merman_core::ParseOptions::default(),
            )
            .unwrap()
            .unwrap();
        let render_context = parsed.flowchart_render_context().unwrap().clone();
        let (metadata, semantic) = parsed.into_parts();
        let merman_core::RenderSemanticModel::Flowchart(model) = semantic else {
            panic!("Flowchart model");
        };
        assert_eq!(model.subgraphs.len(), 1);
        assert_eq!(model.subgraphs[0].title, "First");
        assert_eq!(
            render_context
                .effective_subgraph_css(0, &model.subgraphs[0])
                .0,
            ["first"]
        );
        let work = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let edge_style = crate::svg::FlowchartEdgeStylePlan::prepare_for_model(
            &model,
            &metadata.effective_config,
            false,
            &work,
        )
        .unwrap();
        let mut layout = crate::flowchart::layout_flowchart_typed_with_render_labels_and_svg_label_sidecar_and_work_meter(
            &model, &render_context, &metadata.effective_config,
            &crate::text::DeterministicTextMeasurer::default(), None, None, &edge_style, work.clone(),
        ).unwrap();
        let theme = crate::flowchart::FlowchartPreparedTheme::resolve(
            None,
            &metadata.effective_config,
            false,
            &work,
        )
        .unwrap();
        let sidecar = crate::flowchart::FlowchartSvgLabelSidecar::default();
        let render_config = super::render_config::prepare_flowchart_render_config(
            &model,
            &metadata.effective_config,
            &theme.compatibility,
            layout.uses_elk_adapter_dom,
            sidecar.base_typography(),
            sidecar.edge_label_padding(),
        );
        let before = work.prepared_text_retained_bytes();
        let prepared = FlowchartPreparedNodes::prepare(
            &model,
            &render_context,
            FlowchartNodeLayoutView::Flowchart(&layout),
            &sidecar,
            None,
            &metadata.effective_config,
            &theme,
            &render_config,
            &work,
        )
        .unwrap();
        assert_eq!(prepared.cluster_ids().collect::<Vec<_>>(), ["Shared"]);
        let terminal = prepared.cluster("Shared").unwrap();
        assert!(terminal.compiled.node_style.contains("#112233"));
        assert!(!terminal.compiled.node_style.contains("#aabbcc"));
        assert_eq!(terminal.title_text_style.font_size, 19.0);
        assert!(prepared._cluster_retained_reservation.retained_bytes() > 0);
        assert!(work.prepared_text_retained_bytes() > before);
        drop(prepared);
        assert_eq!(work.prepared_text_retained_bytes(), before);

        layout.uses_elk_adapter_dom = true;
        layout
            .dom_node_order_by_root
            .insert(String::new(), vec!["Shared".into(), "Shared".into()]);
        let helpers = std::collections::BTreeSet::new();
        let input = FlowchartNodeInventoryInput::prepare(
            &model,
            &render_context,
            FlowchartNodeLayoutView::Flowchart(&layout),
            &helpers,
            &work,
        );
        let schedule = FlowchartNodeRootSchedule::prepare(&input).unwrap();
        let error = prepared_cluster_ids(
            &model,
            FlowchartNodeLayoutView::Flowchart(&layout),
            &input,
            &schedule,
        )
        .unwrap_err();
        assert!(
            matches!(error, crate::Error::InvalidModel { message } if message.contains("cluster `Shared` more than once"))
        );
    }

    #[test]
    fn disabled_svg_label_preparation_still_enforces_retained_font_limit() {
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
        let parsed = merman_core::Engine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nA[Label]\nstyle A font-family:CallerFont,font-size:23px",
                merman_core::ParseOptions::default(),
            )
            .unwrap()
            .unwrap();
        let render_context = parsed.flowchart_render_context().unwrap().clone();
        let (metadata, semantic) = parsed.into_parts();
        let merman_core::RenderSemanticModel::Flowchart(model) = semantic else {
            panic!("Flowchart model");
        };
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let layout_work = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let edge_style = crate::svg::FlowchartEdgeStylePlan::prepare_for_model(
            &model,
            &metadata.effective_config,
            false,
            &layout_work,
        )
        .unwrap();
        let layout = crate::flowchart::layout_flowchart_typed_with_render_labels_and_svg_label_sidecar_and_work_meter(
            &model,
            &render_context,
            &metadata.effective_config,
            &measurer,
            None,
            None,
            &edge_style,
            layout_work,
        )
        .unwrap();
        let work = std::sync::Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 1)
                .unwrap(),
        ));
        let theme = crate::flowchart::FlowchartPreparedTheme::resolve(
            None,
            &metadata.effective_config,
            false,
            &work,
        )
        .unwrap();
        let sidecar = crate::flowchart::FlowchartSvgLabelSidecar::default();
        let render_config = super::render_config::prepare_flowchart_render_config(
            &model,
            &metadata.effective_config,
            &theme.compatibility,
            layout.uses_elk_adapter_dom,
            sidecar.base_typography(),
            sidecar.edge_label_padding(),
        );
        let result = FlowchartPreparedNodes::prepare(
            &model,
            &render_context,
            FlowchartNodeLayoutView::Flowchart(&layout),
            &sidecar,
            None,
            &metadata.effective_config,
            &theme,
            &render_config,
            &work,
        );
        assert!(
            matches!(result, Err(crate::Error::ResourceLimitExceeded(ref error))
            if error.limit == "max_prepared_text_retained_bytes")
        );
        assert_eq!(work.prepared_text_retained_bytes(), 0);
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

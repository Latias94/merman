use super::super::timing::RenderTiming;
use super::context::{ClassEmitCheckpoint, ClassRenderDetails};
use super::groups::{
    ClassSplitEdgeGroupsRenderContext, ClassSplitEdgeGroupsRenderState,
    render_class_split_edge_groups, render_class_split_edge_labels, render_class_split_edge_paths,
};
use super::interface::{
    ClassInterfaceRenderContext, ClassInterfaceRenderState, render_class_interface_node,
};
use super::namespace::{
    ClassNamespaceClusterGroupContext, class_namespace_root_offset,
    render_class_namespace_cluster_group, render_class_namespace_clusters_in_root,
};
use super::node::{
    ClassHtmlNodeBodyContext, ClassNodeBasicContainerContext, ClassNodeRenderPosition,
    ClassNodeRenderState, ClassNodeShellContext, ClassSvgNodeBodyContext,
    render_class_html_node_body, render_class_node_basic_container, render_class_node_shell_open,
    render_class_svg_node_body,
};
use super::note::{ClassNoteRenderContext, ClassNoteRenderState, render_class_note_node};
use super::*;
use super::{ClassSvgInterface, ClassSvgNode, ClassSvgNote};
use crate::class::ClassRenderConfig;
use crate::model::{Bounds, ClassDiagramLayout, ClassRenderItem, ClassRenderRootId, LayoutEdge};
use crate::{Error, Result};
use rustc_hash::FxHashMap;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy)]
struct ClassNodeRootOffsets {
    namespace_root_dx: f64,
    namespace_root_dy: f64,
    in_namespace_root: bool,
}

#[derive(Default)]
struct ClassNodeRenderOutcome {
    theme_emission: Option<crate::class::ClassNodeTerminalEmission>,
    typography: crate::class::ClassTextTerminalFacts,
}

pub(super) struct ClassNodesRenderState<'a, O: SvgOutput> {
    pub(super) out: &'a mut O,
    pub(super) content_bounds: &'a mut Option<Bounds>,
    pub(super) detail: &'a mut ClassRenderDetails,
}

pub(super) struct ClassNodesRenderContext<'a> {
    pub(super) layout: &'a ClassDiagramLayout,
    pub(super) class_nodes_by_id: &'a FxHashMap<&'a str, &'a ClassSvgNode>,
    pub(super) class_color_indices: &'a FxHashMap<&'a str, usize>,
    pub(super) note_by_id: &'a FxHashMap<&'a str, &'a ClassSvgNote>,
    pub(super) iface_by_id: &'a FxHashMap<&'a str, &'a ClassSvgInterface>,
    pub(super) settings: &'a ClassRenderConfig,
    pub(super) hand_drawn_seed: &'a roughr::core::RoughRandomness,
    pub(super) diagram_id: SvgDiagramId<'a>,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) mermaid_config: &'a merman_core::MermaidConfig,
    pub(super) math_renderer: Option<&'a (dyn crate::math::MathRenderer + Send + Sync)>,
    pub(super) node_visual_plan: &'a crate::class::ClassNodeVisualPlan,
    pub(super) typography_theme: &'a crate::class::ClassTextThemePlan,
    pub(super) content_tx: f64,
    pub(super) content_ty: f64,
    pub(super) timing: RenderTiming,
    pub(super) emit: ClassEmitCheckpoint<'a>,
}

pub(super) fn render_class_render_tree<O: SvgOutput>(
    state: ClassNodesRenderState<'_, O>,
    ctx: &ClassNodesRenderContext<'_>,
    edge_ctx: &ClassSplitEdgeGroupsRenderContext<'_>,
    theme_receipt: &mut crate::class::ClassRelationThemeReceipt,
    typography_receipt: &mut Option<crate::class::ClassTextThemeReceipt>,
) -> Result<()> {
    let ClassNodesRenderState {
        out,
        content_bounds,
        detail,
    } = state;
    let layout_nodes_by_id = ctx
        .layout
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<FxHashMap<_, _>>();
    let clusters_by_id = ctx
        .layout
        .clusters
        .iter()
        .map(|cluster| (cluster.id.as_str(), cluster))
        .collect::<HashMap<_, _>>();
    let edges_by_id = ctx
        .layout
        .edges
        .iter()
        .map(|edge| (edge.id.as_str(), edge))
        .collect::<HashMap<_, _>>();
    validate_class_render_tree(ctx, &layout_nodes_by_id, &clusters_by_id, &edges_by_id)?;

    enum RenderFrame<'a> {
        Enter {
            root_id: ClassRenderRootId,
            parent_origin: (f64, f64),
        },
        Node {
            id: &'a str,
            origin: (f64, f64),
            in_namespace_root: bool,
        },
        Close {
            in_namespace_root: bool,
        },
    }

    let mut stack = vec![RenderFrame::Enter {
        root_id: ctx.layout.render_tree.top,
        parent_origin: (0.0, 0.0),
    }];
    while let Some(frame) = stack.pop() {
        let (outcome, typography_node_id) = match frame {
            RenderFrame::Enter {
                root_id,
                parent_origin,
            } => {
                let root = ctx
                    .layout
                    .render_tree
                    .roots
                    .get(root_id.0)
                    .expect("validated Class render root id");
                let namespace_id = root.namespace_id.as_deref();
                let origin = namespace_id
                    .map(|id| {
                        clusters_by_id
                            .get(id)
                            .copied()
                            .expect("validated Class namespace root cluster")
                    })
                    .map(class_namespace_root_offset)
                    .unwrap_or((0.0, 0.0));
                let in_namespace_root = namespace_id.is_some();

                if let Some(namespace_id) = namespace_id {
                    let _ = write!(
                        out,
                        r#"<g class="root" transform="translate({}, {})">"#,
                        fmt(origin.0 - parent_origin.0),
                        fmt(origin.1 - parent_origin.1)
                    );
                    out.checkpoint()?;
                    render_class_namespace_clusters_in_root(
                        out,
                        content_bounds,
                        &clusters_by_id,
                        &root
                            .cluster_ids
                            .iter()
                            .map(String::as_str)
                            .collect::<Vec<_>>(),
                        ClassNamespaceClusterGroupContext {
                            relation_theme: edge_ctx.relation_theme,
                            diagram_id: ctx.diagram_id,
                            content_tx: ctx.content_tx,
                            content_ty: ctx.content_ty,
                            bounds_dx: 0.0,
                            bounds_dy: 0.0,
                            use_html_labels: ctx.settings.edge_use_html_labels,
                            look: ctx.settings.look.as_str(),
                            mermaid_config: Some(ctx.mermaid_config),
                            math_renderer: ctx.math_renderer,
                            timing: ctx.timing,
                            emit: ctx.emit,
                        },
                        namespace_id,
                        origin.0,
                        origin.1,
                        theme_receipt,
                        typography_receipt,
                    )?;
                } else {
                    let clusters = root
                        .cluster_ids
                        .iter()
                        .map(|id| {
                            clusters_by_id
                                .get(id.as_str())
                                .copied()
                                .expect("validated Class render cluster")
                                .clone()
                        })
                        .collect::<Vec<_>>();
                    detail.clusters += render_class_namespace_cluster_group(
                        out,
                        content_bounds,
                        &clusters,
                        ClassNamespaceClusterGroupContext {
                            relation_theme: edge_ctx.relation_theme,
                            diagram_id: ctx.diagram_id,
                            content_tx: ctx.content_tx,
                            content_ty: ctx.content_ty,
                            bounds_dx: 0.0,
                            bounds_dy: 0.0,
                            use_html_labels: ctx.settings.edge_use_html_labels,
                            look: ctx.settings.look.as_str(),
                            mermaid_config: Some(ctx.mermaid_config),
                            math_renderer: ctx.math_renderer,
                            timing: ctx.timing,
                            emit: ctx.emit,
                        },
                        theme_receipt,
                        typography_receipt,
                    )?;
                }

                let edges = root
                    .edge_ids
                    .iter()
                    .map(|id| {
                        edges_by_id
                            .get(id.as_str())
                            .copied()
                            .expect("validated Class render edge")
                            .clone()
                    })
                    .collect::<Vec<_>>();
                render_class_split_edges_for_namespace(
                    out,
                    content_bounds,
                    detail,
                    edge_ctx,
                    theme_receipt,
                    &edges,
                    origin.0,
                    origin.1,
                    in_namespace_root,
                    typography_receipt,
                )?;
                out.push_str(r#"<g class="nodes">"#);

                stack.push(RenderFrame::Close { in_namespace_root });
                for item in root.items.iter().rev() {
                    match item {
                        ClassRenderItem::Node(id) => stack.push(RenderFrame::Node {
                            id,
                            origin,
                            in_namespace_root,
                        }),
                        ClassRenderItem::Subgraph(child) => stack.push(RenderFrame::Enter {
                            root_id: *child,
                            parent_origin: origin,
                        }),
                    }
                }
                (ClassNodeRenderOutcome::default(), None)
            }
            RenderFrame::Node {
                id,
                origin,
                in_namespace_root,
            } => (
                render_class_node_id(
                    ClassNodesRenderState {
                        out,
                        content_bounds,
                        detail,
                    },
                    ctx,
                    &layout_nodes_by_id,
                    id,
                    ClassNodeRootOffsets {
                        namespace_root_dx: origin.0,
                        namespace_root_dy: origin.1,
                        in_namespace_root,
                    },
                )?,
                Some(id),
            ),
            RenderFrame::Close { in_namespace_root } => {
                out.push_str("</g>");
                if in_namespace_root {
                    out.push_str("</g>");
                }
                (ClassNodeRenderOutcome::default(), None)
            }
        };
        out.checkpoint()?;
        if let (Some(receipt), Some(id)) = (typography_receipt.as_mut(), typography_node_id) {
            receipt.record_node(id, outcome.typography);
        }
        if let Some(emission) = outcome.theme_emission {
            theme_receipt.record_node(emission);
        }
    }
    Ok(())
}

pub(super) fn render_class_elk_adapter_dom<O: SvgOutput>(
    state: ClassNodesRenderState<'_, O>,
    ctx: &ClassNodesRenderContext<'_>,
    edge_ctx: &ClassSplitEdgeGroupsRenderContext<'_>,
    theme_receipt: &mut crate::class::ClassRelationThemeReceipt,
    typography_receipt: &mut Option<crate::class::ClassTextThemeReceipt>,
) -> Result<()> {
    let ClassNodesRenderState {
        out,
        content_bounds,
        detail,
    } = state;
    let layout_nodes_by_id = ctx
        .layout
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<FxHashMap<_, _>>();
    let clusters_by_id = ctx
        .layout
        .clusters
        .iter()
        .map(|cluster| (cluster.id.as_str(), cluster))
        .collect::<HashMap<_, _>>();
    // Consume the prepared paint routes, including terminal straightening and label updates.
    let edges_by_id = edge_ctx
        .edges
        .iter()
        .map(|edge| (edge.id.as_str(), edge))
        .collect::<HashMap<_, _>>();
    validate_class_render_tree(ctx, &layout_nodes_by_id, &clusters_by_id, &edges_by_id)?;

    let root = ctx
        .layout
        .render_tree
        .roots
        .get(ctx.layout.render_tree.top.0)
        .expect("validated Class ELK render root");
    if root.namespace_id.is_some()
        || root
            .items
            .iter()
            .any(|item| matches!(item, ClassRenderItem::Subgraph(_)))
    {
        return Err(Error::InvalidModel {
            message: "Class ELK adapter requires one flat render root".to_string(),
        });
    }

    // Mermaid 12 paints clusters before paths so namespace fills cannot cover relations.
    detail.clusters += render_class_namespace_cluster_group(
        out,
        content_bounds,
        &ctx.layout.clusters,
        ClassNamespaceClusterGroupContext {
            relation_theme: edge_ctx.relation_theme,
            diagram_id: ctx.diagram_id,
            content_tx: ctx.content_tx,
            content_ty: ctx.content_ty,
            bounds_dx: 0.0,
            bounds_dy: 0.0,
            use_html_labels: ctx.settings.edge_use_html_labels,
            look: ctx.settings.look.as_str(),
            mermaid_config: Some(ctx.mermaid_config),
            math_renderer: ctx.math_renderer,
            timing: ctx.timing,
            emit: ctx.emit,
        },
        theme_receipt,
        typography_receipt,
    )?;

    let edge_label_centers = render_class_split_edge_paths(
        out,
        content_bounds,
        detail,
        theme_receipt,
        edge_ctx,
        0.0,
        0.0,
    )?;
    render_class_split_edge_labels(
        out,
        content_bounds,
        detail,
        theme_receipt,
        typography_receipt,
        edge_ctx,
        0.0,
        0.0,
        &edge_label_centers,
    )?;

    out.push_str(r#"<g class="nodes">"#);
    out.checkpoint()?;
    for item in &root.items {
        let ClassRenderItem::Node(id) = item else {
            unreachable!("Class ELK render root was validated as flat")
        };
        let outcome = render_class_node_id(
            ClassNodesRenderState {
                out,
                content_bounds,
                detail,
            },
            ctx,
            &layout_nodes_by_id,
            id,
            ClassNodeRootOffsets {
                namespace_root_dx: 0.0,
                namespace_root_dy: 0.0,
                in_namespace_root: false,
            },
        )?;
        out.checkpoint()?;
        if let Some(receipt) = typography_receipt.as_mut() {
            receipt.record_node(id, outcome.typography);
        }
        if let Some(emission) = outcome.theme_emission {
            theme_receipt.record_node(emission);
        }
    }
    out.push_str("</g>");
    out.checkpoint()?;
    Ok(())
}

fn validate_class_render_tree(
    ctx: &ClassNodesRenderContext<'_>,
    layout_nodes_by_id: &FxHashMap<&str, &crate::model::LayoutNode>,
    clusters_by_id: &HashMap<&str, &crate::model::LayoutCluster>,
    edges_by_id: &HashMap<&str, &LayoutEdge>,
) -> Result<()> {
    let tree = &ctx.layout.render_tree;
    if tree.roots.is_empty() || tree.top.0 >= tree.roots.len() {
        return Err(Error::InvalidModel {
            message: format!(
                "invalid Class render tree top {} for {} roots",
                tree.top.0,
                tree.roots.len()
            ),
        });
    }
    if layout_nodes_by_id.len() != ctx.layout.nodes.len()
        || clusters_by_id.len() != ctx.layout.clusters.len()
        || edges_by_id.len() != ctx.layout.edges.len()
    {
        return Err(Error::InvalidModel {
            message: "duplicate identifiers in Class layout artifact".to_string(),
        });
    }

    enum ValidationFrame {
        Enter(ClassRenderRootId),
        Exit(ClassRenderRootId),
    }

    let mut root_state = vec![0_u8; tree.roots.len()];
    let mut owned_nodes = HashSet::new();
    let mut owned_clusters = HashSet::new();
    let mut owned_edges = HashSet::new();
    let mut stack = vec![ValidationFrame::Enter(tree.top)];
    while let Some(frame) = stack.pop() {
        match frame {
            ValidationFrame::Enter(root_id) => {
                let Some(root) = tree.roots.get(root_id.0) else {
                    return Err(Error::InvalidModel {
                        message: format!("missing Class render root {}", root_id.0),
                    });
                };
                match root_state[root_id.0] {
                    1 => {
                        return Err(Error::InvalidModel {
                            message: format!("cycle in Class render tree at root {}", root_id.0),
                        });
                    }
                    2 => {
                        return Err(Error::InvalidModel {
                            message: format!("Class render root {} has multiple owners", root_id.0),
                        });
                    }
                    _ => {}
                }
                root_state[root_id.0] = 1;
                stack.push(ValidationFrame::Exit(root_id));

                if let Some(namespace_id) = root.namespace_id.as_deref()
                    && !clusters_by_id.contains_key(namespace_id)
                {
                    return Err(Error::InvalidModel {
                        message: format!(
                            "Class render root {} references missing namespace cluster {namespace_id}",
                            root_id.0
                        ),
                    });
                }
                for cluster_id in &root.cluster_ids {
                    if !clusters_by_id.contains_key(cluster_id.as_str()) {
                        return Err(Error::InvalidModel {
                            message: format!(
                                "Class render root {} references missing cluster {cluster_id}",
                                root_id.0
                            ),
                        });
                    }
                    if !owned_clusters.insert(cluster_id.as_str()) {
                        return Err(Error::InvalidModel {
                            message: format!(
                                "Class cluster {cluster_id} has multiple render owners"
                            ),
                        });
                    }
                }
                for edge_id in &root.edge_ids {
                    if !edges_by_id.contains_key(edge_id.as_str()) {
                        return Err(Error::InvalidModel {
                            message: format!(
                                "Class render root {} references missing edge {edge_id}",
                                root_id.0
                            ),
                        });
                    }
                    if !owned_edges.insert(edge_id.as_str()) {
                        return Err(Error::InvalidModel {
                            message: format!("Class edge {edge_id} has multiple render owners"),
                        });
                    }
                }
                for item in root.items.iter().rev() {
                    match item {
                        ClassRenderItem::Node(node_id) => {
                            let Some(node) = layout_nodes_by_id.get(node_id.as_str()) else {
                                return Err(Error::InvalidModel {
                                    message: format!(
                                        "Class render root {} references missing node {node_id}",
                                        root_id.0
                                    ),
                                });
                            };
                            if node.is_cluster {
                                return Err(Error::InvalidModel {
                                    message: format!(
                                        "Class render item {node_id} is a cluster, not a leaf node"
                                    ),
                                });
                            }
                            if !ctx.class_nodes_by_id.contains_key(node_id.as_str())
                                && !ctx.note_by_id.contains_key(node_id.as_str())
                                && !ctx.iface_by_id.contains_key(node_id.as_str())
                            {
                                return Err(Error::InvalidModel {
                                    message: format!(
                                        "Class render node {node_id} has no semantic node payload"
                                    ),
                                });
                            }
                            if !owned_nodes.insert(node_id.as_str()) {
                                return Err(Error::InvalidModel {
                                    message: format!(
                                        "Class node {node_id} has multiple render owners"
                                    ),
                                });
                            }
                        }
                        ClassRenderItem::Subgraph(child_id) => {
                            if child_id.0 >= tree.roots.len() {
                                return Err(Error::InvalidModel {
                                    message: format!(
                                        "Class render root {} references missing child root {}",
                                        root_id.0, child_id.0
                                    ),
                                });
                            }
                            stack.push(ValidationFrame::Enter(*child_id));
                        }
                    }
                }
            }
            ValidationFrame::Exit(root_id) => root_state[root_id.0] = 2,
        }
    }

    if let Some(unattached) = root_state.iter().position(|state| *state == 0) {
        return Err(Error::InvalidModel {
            message: format!("unattached Class render root {unattached}"),
        });
    }
    for node in &ctx.layout.nodes {
        if !node.is_cluster && !owned_nodes.contains(node.id.as_str()) {
            return Err(Error::InvalidModel {
                message: format!("Class node {} has no render owner", node.id),
            });
        }
    }
    for cluster in &ctx.layout.clusters {
        if !owned_clusters.contains(cluster.id.as_str()) {
            return Err(Error::InvalidModel {
                message: format!("Class cluster {} has no render owner", cluster.id),
            });
        }
    }
    for edge in &ctx.layout.edges {
        if !owned_edges.contains(edge.id.as_str()) {
            return Err(Error::InvalidModel {
                message: format!("Class edge {} has no render owner", edge.id),
            });
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn render_class_split_edges_for_namespace<O: SvgOutput>(
    out: &mut O,
    content_bounds: &mut Option<Bounds>,
    detail: &mut ClassRenderDetails,
    edge_ctx: &ClassSplitEdgeGroupsRenderContext<'_>,
    theme_receipt: &mut crate::class::ClassRelationThemeReceipt,
    edges: &[LayoutEdge],
    root_dx: f64,
    root_dy: f64,
    in_namespace_root: bool,
    typography_receipt: &mut Option<crate::class::ClassTextThemeReceipt>,
) -> Result<()> {
    let local_ctx = ClassSplitEdgeGroupsRenderContext {
        edges,
        missing_section_points: edge_ctx.missing_section_points,
        work_meter: edge_ctx.work_meter,
        line_hop_paths: edge_ctx.line_hop_paths,
        relations_by_id: edge_ctx.relations_by_id,
        relation_index_by_id: edge_ctx.relation_index_by_id,
        diagram_marker_class: edge_ctx.diagram_marker_class,
        diagram_id: edge_ctx.diagram_id,
        content_tx: if in_namespace_root {
            // The recursive Class root's x origin already folds in Dagre's fixed graph margin.
            // Adding the top-level content translation again moves only the edge route 8px to the
            // right of nodes rendered in the same local coordinate frame.
            -root_dx
        } else {
            edge_ctx.content_tx
        },
        content_ty: if in_namespace_root {
            edge_ctx.content_ty - root_dy
        } else {
            edge_ctx.content_ty
        },
        edge_use_html_labels: edge_ctx.edge_use_html_labels,
        text_measurer: edge_ctx.text_measurer,
        terminal_text_style: edge_ctx.terminal_text_style,
        mermaid_config: edge_ctx.mermaid_config,
        math_renderer: edge_ctx.math_renderer,
        look: edge_ctx.look,
        hand_drawn_seed: edge_ctx.hand_drawn_seed.clone(),
        timing: edge_ctx.timing,
        uses_elk_adapter_dom: edge_ctx.uses_elk_adapter_dom,
        edge_paths_class: edge_ctx.edge_paths_class,
        relation_theme: edge_ctx.relation_theme,
        text_paint: edge_ctx.text_paint,
        emit: edge_ctx.emit,
    };
    render_class_split_edge_groups(
        out,
        ClassSplitEdgeGroupsRenderState {
            content_bounds,
            detail,
            theme_receipt,
            typography_receipt,
        },
        &local_ctx,
        if in_namespace_root { root_dx } else { 0.0 },
        if in_namespace_root { root_dy } else { 0.0 },
    )
}

fn render_class_node_id<O: SvgOutput>(
    state: ClassNodesRenderState<'_, O>,
    ctx: &ClassNodesRenderContext<'_>,
    layout_nodes_by_id: &FxHashMap<&str, &crate::model::LayoutNode>,
    id: &str,
    offsets: ClassNodeRootOffsets,
) -> Result<ClassNodeRenderOutcome> {
    let ClassNodesRenderState {
        out,
        content_bounds,
        detail,
    } = state;
    let settings = ctx.settings;

    let n = layout_nodes_by_id
        .get(id)
        .copied()
        .expect("validated Class render node id");

    let node_tx = if offsets.in_namespace_root {
        n.x - offsets.namespace_root_dx
    } else {
        n.x + ctx.content_tx
    };
    let node_ty = if offsets.in_namespace_root {
        n.y + ctx.content_ty - offsets.namespace_root_dy
    } else {
        n.y + ctx.content_ty
    };
    let node_bounds_tx = node_tx + offsets.namespace_root_dx;
    let node_bounds_ty = node_ty + offsets.namespace_root_dy;
    let position = ClassNodeRenderPosition {
        node_tx,
        node_ty,
        node_bounds_tx,
        node_bounds_ty,
    };

    if let Some(note) = ctx.note_by_id.get(n.id.as_str()).copied() {
        let stats = render_class_note_node(
            ClassNoteRenderState {
                out,
                content_bounds,
            },
            note,
            n,
            position,
            &ClassNoteRenderContext {
                diagram_id: ctx.diagram_id,
                measurer: ctx.measurer,
                text_style: settings.text_style(),
                line_height: settings.line_height,
                use_html_labels: settings.diagram_use_html_labels
                    || crate::math::contains_delimited_math(&note.text),
                mermaid_config: ctx.mermaid_config,
                text_paint: ctx.typography_theme.note_paint(),
                css_binding: ctx.typography_theme.css_binding(),
                math_renderer: ctx.math_renderer,
                look: settings.look.as_str(),
                hand_drawn_seed: ctx.hand_drawn_seed.clone(),
                timing: ctx.timing,
                emit: ctx.emit,
            },
        )?;
        detail.notes_sanitize += stats.notes_sanitize;
        detail.path_bounds += stats.path_bounds;
        detail.path_bounds_calls += stats.path_bounds_calls;
        return Ok(ClassNodeRenderOutcome {
            theme_emission: None,
            typography: stats.typography,
        });
    }

    if let Some(iface) = ctx.iface_by_id.get(n.id.as_str()).copied() {
        let binding = ctx.node_visual_plan.interface(n.id.as_str());
        let result = render_class_interface_node(
            ClassInterfaceRenderState {
                out,
                content_bounds,
            },
            iface,
            n,
            position,
            &ClassInterfaceRenderContext {
                diagram_id: ctx.diagram_id,
                measurer: ctx.measurer,
                text_style: settings.text_style(),
                use_html_labels: settings.diagram_use_html_labels,
                wrapping_width: settings.interface_wrapping_width,
                look: settings.look.as_str(),
                mermaid_config: Some(ctx.mermaid_config),
                math_renderer: ctx.math_renderer,
                visual_binding: binding,
                emit: ctx.emit,
            },
        )?;
        return Ok(ClassNodeRenderOutcome {
            theme_emission: Some(result.theme_emission),
            typography: result.typography,
        });
    }

    let node = ctx
        .class_nodes_by_id
        .get(n.id.as_str())
        .copied()
        .expect("validated Class semantic node payload");

    let binding = ctx.node_visual_plan.node(n.id.as_str());
    let node_label_plan = ctx
        .layout
        .class_label_plans_by_id
        .get(n.id.as_str())
        .map(|plan| plan.as_ref());
    let node_fill = binding.fill.as_str();
    let node_stroke = binding.stroke.as_str();
    let html_node_label_style_attr = &binding.html_label_style;
    let svg_node_label_style_attr = &binding.svg_label_style;
    let node_fill_style_attr = &binding.fill_style;
    let node_stroke_style_attr = &binding.stroke_style;
    let node_stroke_width = binding.stroke_width.as_str();
    let node_stroke_dasharray = binding.stroke_dasharray.as_str();

    let node_link_open = render_class_node_shell_open(
        out,
        node,
        position,
        &ClassNodeShellContext {
            diagram_id: ctx.diagram_id,
            emit: ctx.emit,
            look: settings.look.as_str(),
            security_level_loose: settings.security_level_loose,
            color_index: ctx.class_color_indices.get(n.id.as_str()).copied(),
            palette_size: ctx.typography_theme.css_binding().palette.len(),
        },
    )?;
    let basic_container = render_class_node_basic_container(
        ClassNodeRenderState {
            out,
            content_bounds,
        },
        node,
        n,
        position,
        &ClassNodeBasicContainerContext {
            diagram_id: ctx.diagram_id,
            node_fill_style_attr: node_fill_style_attr.as_str(),
            node_stroke_style_attr: node_stroke_style_attr.as_str(),
            node_fill,
            node_stroke,
            node_stroke_width,
            node_stroke_dasharray,
            look: settings.look.as_str(),
            hand_drawn_seed: ctx.hand_drawn_seed.clone(),
            timing: ctx.timing,
        },
    );
    detail.path_bounds += basic_container.stats.path_bounds;
    detail.path_bounds_calls += basic_container.stats.path_bounds_calls;

    let use_html_labels =
        settings.diagram_use_html_labels || crate::class::class_node_requires_math(node);
    if use_html_labels {
        let html_stats = render_class_html_node_body(
            ClassNodeRenderState {
                out,
                content_bounds,
            },
            position,
            node,
            basic_container.geometry,
            node_label_plan,
            &ClassHtmlNodeBodyContext {
                measurer: ctx.measurer,
                text_style: settings.text_style(),
                html_calc_text_style: settings.html_calc_text_style(),
                line_height: settings.line_height,
                class_padding: settings.class_padding,
                hide_empty_members_box: settings.hide_empty_members_box(),
                node_stroke_style_attr: node_stroke_style_attr.as_str(),
                node_label_style_attr: html_node_label_style_attr.as_str(),
                node_label_fill: binding.label_fill.as_deref(),
                node_stroke,
                node_stroke_width,
                node_stroke_dasharray,
                look: settings.look.as_str(),
                mermaid_config: Some(ctx.mermaid_config),
                use_gradient: ctx.typography_theme.css_binding().look_defs.uses_gradient(),
                math_renderer: ctx.math_renderer,
                timing: ctx.timing,
            },
        );
        detail.path_bounds += html_stats.path_bounds;
        detail.path_bounds_calls += html_stats.path_bounds_calls;
    } else {
        let svg_stats = render_class_svg_node_body(
            ClassNodeRenderState {
                out,
                content_bounds,
            },
            position,
            node,
            basic_container.geometry,
            &ClassSvgNodeBodyContext {
                measurer: ctx.measurer,
                text_style: settings.text_style(),
                wrap_probe_font_size: settings.wrap_probe_font_size(),
                class_padding: settings.class_padding,
                hide_empty_members_box: settings.hide_empty_members_box(),
                node_stroke_style_attr: node_stroke_style_attr.as_str(),
                node_label_style_attr: svg_node_label_style_attr.as_str(),
                node_stroke,
                node_stroke_width,
                node_stroke_dasharray,
                look: settings.look.as_str(),
                use_gradient: ctx.typography_theme.css_binding().look_defs.uses_gradient(),
                timing: ctx.timing,
            },
        );
        detail.path_bounds += svg_stats.path_bounds;
        detail.path_bounds_calls += svg_stats.path_bounds_calls;
    }

    out.push_str("</g>");
    if node_link_open {
        out.push_str("</a>");
    }
    Ok(ClassNodeRenderOutcome {
        theme_emission: Some(binding.emission.clone()),
        typography: binding.typography,
    })
}

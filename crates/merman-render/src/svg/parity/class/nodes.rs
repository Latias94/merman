use super::context::ClassRenderDetails;
use super::interface::{
    ClassInterfaceRenderContext, ClassInterfaceRenderState, render_class_interface_node,
};
use super::label::class_apply_inline_styles;
use super::namespace::{
    ClassNamespaceSubgraphState, ClassNodeRenderOrder, build_class_node_render_order,
    close_class_namespace_subgraph, transition_class_namespace_subgraph,
};
use super::node::{
    ClassHtmlNodeBodyContext, ClassNodeBasicContainerContext, ClassNodeRenderPosition,
    ClassNodeRenderState, ClassSvgNodeBodyContext, render_class_html_node_body,
    render_class_node_basic_container, render_class_node_shell_open, render_class_svg_node_body,
};
use super::note::{ClassNoteRenderContext, ClassNoteRenderState, render_class_note_node};
use super::settings::ClassRenderSettings;
use super::*;
use super::{ClassSvgInterface, ClassSvgModel, ClassSvgNode, ClassSvgNote};
use crate::model::{Bounds, ClassDiagramV2Layout};
use rustc_hash::FxHashMap;

pub(super) struct ClassNodesRenderState<'a> {
    pub(super) out: &'a mut String,
    pub(super) content_bounds: &'a mut Option<Bounds>,
    pub(super) detail: &'a mut ClassRenderDetails,
    pub(super) sanitize_config: &'a mut Option<merman_core::MermaidConfig>,
    pub(super) borrowed_sanitize_config: Option<&'a merman_core::MermaidConfig>,
}

pub(super) struct ClassNodesRenderContext<'a> {
    pub(super) layout: &'a ClassDiagramV2Layout,
    pub(super) model: &'a ClassSvgModel,
    pub(super) class_nodes_by_id: &'a FxHashMap<&'a str, &'a ClassSvgNode>,
    pub(super) note_by_id: &'a FxHashMap<&'a str, &'a ClassSvgNote>,
    pub(super) iface_by_id: &'a FxHashMap<&'a str, &'a ClassSvgInterface>,
    pub(super) settings: &'a ClassRenderSettings,
    pub(super) effective_config: &'a serde_json::Value,
    pub(super) diagram_id: &'a str,
    pub(super) measurer: &'a dyn TextMeasurer,
    pub(super) content_tx: f64,
    pub(super) content_ty: f64,
    pub(super) timing_enabled: bool,
    pub(super) wrap_nodes_root: bool,
    pub(super) single_namespace_id: Option<&'a str>,
    pub(super) render_namespaces_as_subgraphs: bool,
    pub(super) nodes_root_dx: f64,
    pub(super) nodes_root_dy: f64,
}

pub(super) fn render_class_nodes(
    state: ClassNodesRenderState<'_>,
    ctx: &ClassNodesRenderContext<'_>,
) {
    let ClassNodesRenderState {
        out,
        content_bounds,
        detail,
        sanitize_config,
        borrowed_sanitize_config,
    } = state;
    let settings = ctx.settings;

    let ClassNodeRenderOrder {
        layout_nodes_by_id,
        ordered_ids,
        namespace_key_set,
        clusters_by_id,
    } = build_class_node_render_order(
        ctx.layout,
        ctx.model,
        ctx.class_nodes_by_id,
        ctx.wrap_nodes_root,
        ctx.single_namespace_id,
        ctx.render_namespaces_as_subgraphs,
    );

    let mut inner_nodes_group_open = ctx.wrap_nodes_root;
    let mut namespace_subgraph_state = ClassNamespaceSubgraphState::default();
    for id in ordered_ids {
        if ctx.wrap_nodes_root && inner_nodes_group_open {
            let parent = ctx
                .class_nodes_by_id
                .get(id)
                .and_then(|n| n.parent.as_deref());
            let should_be_inner = ctx.single_namespace_id.is_some_and(|ns| parent == Some(ns));
            if !should_be_inner {
                // Close the nested wrapper, then continue emitting remaining nodes at the outer level.
                out.push_str("</g>"); // inner nodes
                out.push_str("</g>"); // inner root
                inner_nodes_group_open = false;
            }
        }

        if ctx.render_namespaces_as_subgraphs {
            let parent = ctx
                .class_nodes_by_id
                .get(id)
                .and_then(|n| n.parent.as_deref());
            let parent = parent.filter(|p| namespace_key_set.contains(p));
            transition_class_namespace_subgraph(
                out,
                content_bounds,
                &mut namespace_subgraph_state,
                parent,
                &clusters_by_id,
            );
        }

        let (active_nodes_root_dx, active_nodes_root_dy) =
            if ctx.wrap_nodes_root && inner_nodes_group_open {
                (ctx.nodes_root_dx, ctx.nodes_root_dy)
            } else {
                (0.0, 0.0)
            };
        let (active_namespace_root_dx, active_namespace_root_dy) =
            namespace_subgraph_state.root_offset.unwrap_or((0.0, 0.0));

        let Some(n) = layout_nodes_by_id.get(id).copied() else {
            continue;
        };

        let in_namespace_root = ctx.render_namespaces_as_subgraphs
            && namespace_subgraph_state.active_subgraph.is_some();
        let node_tx = if in_namespace_root {
            n.x - active_namespace_root_dx
        } else {
            n.x + ctx.content_tx
        };
        let node_ty = if in_namespace_root {
            n.y + ctx.content_ty - active_namespace_root_dy
        } else {
            n.y + ctx.content_ty
        };
        let node_bounds_tx = node_tx + active_namespace_root_dx + active_nodes_root_dx;
        let node_bounds_ty = node_ty + active_namespace_root_dy + active_nodes_root_dy;
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
                    sanitize_config,
                    borrowed_sanitize_config,
                },
                note,
                n,
                position,
                &ClassNoteRenderContext {
                    diagram_id: ctx.diagram_id,
                    effective_config: ctx.effective_config,
                    measurer: ctx.measurer,
                    text_style: &settings.text_style,
                    line_height: settings.line_height,
                    use_html_labels: settings.diagram_use_html_labels,
                    timing_enabled: ctx.timing_enabled,
                },
            );
            detail.notes_sanitize += stats.notes_sanitize;
            detail.path_bounds += stats.path_bounds;
            detail.path_bounds_calls += stats.path_bounds_calls;
            continue;
        }

        if let Some(iface) = ctx.iface_by_id.get(n.id.as_str()).copied() {
            render_class_interface_node(
                ClassInterfaceRenderState {
                    out,
                    content_bounds,
                },
                iface,
                n,
                position,
                &ClassInterfaceRenderContext {
                    measurer: ctx.measurer,
                    text_style: &settings.text_style,
                    line_height: settings.line_height,
                },
            );
            continue;
        }

        let Some(node) = ctx.class_nodes_by_id.get(n.id.as_str()).copied() else {
            continue;
        };

        let node_inline_styles = class_apply_inline_styles(node);
        let node_style_attr = node_inline_styles.style_attr.as_str();
        let node_fill = node_inline_styles
            .fill
            .unwrap_or(settings.default_node_fill.as_str());
        let node_stroke = node_inline_styles
            .stroke
            .unwrap_or(settings.default_node_stroke.as_str());
        let node_stroke_width = node_inline_styles
            .stroke_width
            .unwrap_or("1.3")
            .trim_end_matches("px")
            .trim();
        let node_stroke_dasharray = node_inline_styles.stroke_dasharray.unwrap_or("0 0");

        let node_link_open = render_class_node_shell_open(out, node, position);
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
                node_style_attr,
                node_fill,
                node_stroke,
                node_stroke_width,
                node_stroke_dasharray,
                timing_enabled: ctx.timing_enabled,
            },
        );
        detail.path_bounds += basic_container.stats.path_bounds;
        detail.path_bounds_calls += basic_container.stats.path_bounds_calls;

        if settings.diagram_use_html_labels {
            let html_stats = render_class_html_node_body(
                ClassNodeRenderState {
                    out,
                    content_bounds,
                },
                position,
                node,
                basic_container.geometry,
                ctx.layout
                    .class_row_metrics_by_id
                    .get(n.id.as_str())
                    .map(|rows| rows.as_ref()),
                &ClassHtmlNodeBodyContext {
                    measurer: ctx.measurer,
                    text_style: &settings.text_style,
                    html_calc_text_style: &settings.html_calc_text_style,
                    line_height: settings.line_height,
                    class_padding: settings.class_padding,
                    hide_empty_members_box: settings.hide_empty_members_box,
                    node_style_attr,
                    node_stroke,
                    node_stroke_width,
                    node_stroke_dasharray,
                    timing_enabled: ctx.timing_enabled,
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
                    text_style: &settings.text_style,
                    font_size: settings.font_size,
                    wrap_probe_font_size: settings.wrap_probe_font_size,
                    class_padding: settings.class_padding,
                    hide_empty_members_box: settings.hide_empty_members_box,
                    node_style_attr,
                    node_stroke,
                    node_stroke_width,
                    node_stroke_dasharray,
                    timing_enabled: ctx.timing_enabled,
                },
            );
            detail.path_bounds += svg_stats.path_bounds;
            detail.path_bounds_calls += svg_stats.path_bounds_calls;
        }

        out.push_str("</g>");
        if node_link_open {
            out.push_str("</a>");
        }
    }

    if ctx.render_namespaces_as_subgraphs {
        close_class_namespace_subgraph(out, &mut namespace_subgraph_state);
    }

    if inner_nodes_group_open {
        out.push_str("</g>"); // inner nodes
        out.push_str("</g>"); // inner root
    }
}

#![allow(clippy::too_many_arguments)]

use super::super::timing::{RenderTimings, TimingGuard, render_timing_enabled};
use super::edge::{
    ClassEdgeGroupsRenderContext, ClassEdgeGroupsRenderState, render_class_edge_groups,
};
use super::interface::{
    ClassInterfaceRenderContext, ClassInterfaceRenderState, render_class_interface_node,
};
use super::namespace::{
    ClassNamespaceClusterGroupContext, ClassNamespaceRenderMode, ClassNamespaceSubgraphState,
    ClassNodeRenderOrder, build_class_node_render_order, class_namespace_render_mode,
    close_class_namespace_subgraph, render_class_namespace_cluster_group,
    transition_class_namespace_subgraph,
};
use super::node::{
    ClassHtmlNodeBodyContext, ClassNodeBasicContainerContext, ClassNodeRenderPosition,
    ClassNodeRenderState, ClassSvgNodeBodyContext, render_class_html_node_body,
    render_class_node_basic_container, render_class_node_shell_open, render_class_svg_node_body,
};
use super::note::{ClassNoteRenderContext, ClassNoteRenderState, render_class_note_node};
use super::root::{CLASS_GRAPH_MARGIN_PX, write_class_svg_root_open};
use super::viewbox::{ClassViewBoxContext, class_viewbox_attrs};
use super::*;

pub(super) fn render_class_diagram_v2_svg_impl(
    layout: &ClassDiagramV2Layout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: ClassSvgModel = crate::json::from_value_ref(semantic)?;
    render_class_diagram_v2_svg_model_impl(
        layout,
        &model,
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}

pub(super) fn render_class_diagram_v2_svg_model_impl(
    layout: &ClassDiagramV2Layout,
    model: &ClassSvgModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let timing_enabled = render_timing_enabled();
    let total_start = timing_enabled.then(std::time::Instant::now);
    let mut timings = RenderTimings::default();

    let mut detail = ClassRenderDetails::default();

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let aria_roledescription = options.aria_roledescription.as_deref().unwrap_or("class");
    let mut sanitize_config: Option<merman_core::MermaidConfig> = None;

    let build_ctx_guard = timing_enabled.then(|| TimingGuard::new(&mut timings.build_ctx));

    let diagram_use_html_labels = config_bool(effective_config, &["htmlLabels"]).unwrap_or(true);
    let edge_use_html_labels = config_bool(effective_config, &["flowchart", "htmlLabels"])
        .or_else(|| config_bool(effective_config, &["htmlLabels"]))
        .unwrap_or(true);
    let font_size = if diagram_use_html_labels {
        // Mermaid class diagram labels are rendered via HTML `<foreignObject>`. Mermaid CLI
        // baselines show that those HTML labels do not reliably inherit the surrounding SVG-root
        // `font-size` rules, so they effectively render at the browser default (16px) even when
        // users override `fontSize` / `themeVariables.fontSize`.
        16.0
    } else {
        // Mermaid injects `themeVariables.fontSize` into CSS as `font-size: ${fontSize};` without
        // forcing a unit. A unitless `font-size: 24` is invalid CSS and gets ignored (falling back
        // to the browser default 16px), while a value like `"24px"` works and *does* influence
        // wrapping/sizing (see:
        // `fixtures/upstream-svgs/class/stress_class_svg_font_size_precedence_025.svg` and
        // `fixtures/upstream-svgs/class/stress_class_svg_font_size_px_string_precedence_026.svg`).
        theme_font_size_px_string_only(effective_config).unwrap_or(16.0)
    }
    .max(1.0);
    let wrap_probe_font_size = config_f64(effective_config, &["fontSize"])
        .unwrap_or(16.0)
        .max(1.0);
    let html_calc_text_style = crate::class::class_html_calculate_text_style(effective_config);
    let line_height = font_size * 1.5;
    // Mermaid defaults `config.class.padding` to 12 (used for node sizing, not SVG viewport padding).
    let _class_padding = effective_config
        .get("class")
        .and_then(|v| v.get("padding"))
        .and_then(|v| v.as_f64())
        .unwrap_or(12.0)
        .max(0.0);
    let text_style = TextStyle {
        font_family: config_string(effective_config, &["fontFamily"])
            .or_else(|| config_string(effective_config, &["themeVariables", "fontFamily"]))
            .or_else(|| Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string())),
        font_size,
        font_weight: None,
    };

    // Mermaid uses `setupGraphViewbox(..., conf.diagramPadding)` (v2) / `setupViewPortForSVG(..., 8)` (v3),
    // both of which expand the root viewBox/max-width by 2 * padding around the rendered content bbox.
    //
    // Keep the config lookup compatible with Mermaid's classRenderer-v2 quirk that reads `flowchart ?? class`.
    let conf = effective_config
        .get("flowchart")
        .or_else(|| effective_config.get("class"))
        .unwrap_or(effective_config);
    let viewport_padding = config_f64(conf, &["diagramPadding"])
        .unwrap_or(8.0)
        .max(0.0);
    // Mermaid's class renderer uses Dagre with fixed `marginx/marginy=8`, then calls
    // `setupGraphViewbox(svg, padding=conf.diagramPadding)` which computes the final SVG viewBox
    // from `svg.getBBox()`.
    //
    // Our headless layout output is margin-free, so re-introduce Dagre's margin at render time to
    // match upstream SVG coordinates and viewport sizing.
    let content_tx = CLASS_GRAPH_MARGIN_PX;
    let content_ty = CLASS_GRAPH_MARGIN_PX;

    let hide_empty_members_box =
        config_bool(effective_config, &["class", "hideEmptyMembersBox"]).unwrap_or(false);

    // Theme-derived defaults. Mermaid's class renderer applies `themeVariables.*` values to node
    // fills/strokes when no explicit `style` overrides exist on the node.
    let default_node_fill = config_string(effective_config, &["themeVariables", "primaryColor"])
        .unwrap_or_else(|| "#ECECFF".to_string());
    let default_node_stroke =
        config_string(effective_config, &["themeVariables", "primaryBorderColor"])
            .unwrap_or_else(|| "#9370DB".to_string());

    // Mermaid derives the final viewport using `svg.getBBox()` (after rendering). We don't have a
    // browser DOM, so approximate the effective bbox by accumulating bounds for the elements we
    // emit (using the exact same `d` strings we output for paths).
    let mut content_bounds: Option<Bounds> = None;

    let render_guard = timing_enabled.then(|| TimingGuard::new(&mut timings.render_svg));
    let estimated_svg_bytes = 2048usize
        + model.classes.len().saturating_mul(512)
        + model.relations.len().saturating_mul(384)
        + model.notes.len().saturating_mul(256)
        + model.namespaces.len().saturating_mul(128);
    let mut out = String::with_capacity(estimated_svg_bytes);
    let root_open = write_class_svg_root_open(&mut out, model, diagram_id, aria_roledescription);

    // Mermaid emits a single `<style>` element with diagram-scoped CSS.
    out.push_str("<style></style>");

    // Mermaid wraps diagram content (defs + root) in a single `<g>` element.
    out.push_str("<g>");
    class_markers(&mut out, diagram_id, aria_roledescription);

    let ClassRenderLookups {
        class_nodes_by_id,
        relations_by_id,
        relation_index_by_id,
        note_by_id,
        iface_by_id,
    } = ClassRenderLookups::new(model);

    out.push_str(r#"<g class="root">"#);

    // Mermaid sometimes emits a nested dagre-d3 `root` wrapper (translated by -8px on the x-axis).
    // In that mode, the outer `clusters/edgePaths/edgeLabels` groups are empty placeholders, and
    // all cluster + edge rendering happens inside the nested wrapper under `<g class="nodes">`.
    //
    // This affects DOM parity for namespace-heavy diagrams. See upstream fixtures:
    // - `upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_add_classes_namespaces_039`
    // - `upstream_docs_classdiagram_define_namespace_035`
    // - `upstream_cypress_classdiagram_v2_spec_renders_a_class_diagram_with_nested_namespaces_and_relationships_035`
    let viewbox_override_min_xy =
        crate::generated::class_root_overrides_11_12_2::lookup_class_root_viewport_override(
            diagram_id,
        )
        .and_then(|(vb, _)| parse_viewbox_min_xy(vb));
    let ClassNamespaceRenderMode {
        single_namespace_id,
        wrap_nodes_root,
        nodes_root_dx,
        nodes_root_dy,
        render_namespaces_as_subgraphs,
    } = class_namespace_render_mode(
        model,
        &class_nodes_by_id,
        viewbox_override_min_xy,
        CLASS_GRAPH_MARGIN_PX,
    );

    drop(build_ctx_guard);

    let marker_url_prefix = {
        let mut out = String::new();
        let _ = write!(&mut out, "{}", escape_attr_display(diagram_id));
        out.push('_');
        let _ = write!(&mut out, "{}", escape_attr_display(aria_roledescription));
        out.push('-');
        out
    };

    let mut render_clusters_edges_and_labels =
        |out: &mut String,
         content_bounds: &mut Option<Bounds>,
         bounds_dx: f64,
         bounds_dy: f64,
         emit_clusters: bool| {
            if emit_clusters {
                detail.clusters += render_class_namespace_cluster_group(
                    out,
                    content_bounds,
                    &layout.clusters,
                    ClassNamespaceClusterGroupContext {
                        content_tx,
                        content_ty,
                        bounds_dx,
                        bounds_dy,
                        timing_enabled,
                    },
                );
            }

            render_class_edge_groups(
                ClassEdgeGroupsRenderState {
                    out,
                    content_bounds,
                    detail: &mut detail,
                },
                &ClassEdgeGroupsRenderContext {
                    edges: &layout.edges,
                    relations_by_id: &relations_by_id,
                    relation_index_by_id: &relation_index_by_id,
                    marker_url_prefix: &marker_url_prefix,
                    content_tx,
                    content_ty,
                    bounds_dx,
                    bounds_dy,
                    edge_use_html_labels,
                    timing_enabled,
                },
            );
        };

    if wrap_nodes_root {
        out.push_str(r#"<g class="clusters"/><g class="edgePaths"/><g class="edgeLabels"/>"#);
    } else if render_namespaces_as_subgraphs {
        out.push_str(r#"<g class="clusters"/>"#);
        render_clusters_edges_and_labels(&mut out, &mut content_bounds, 0.0, 0.0, false);
    } else {
        render_clusters_edges_and_labels(&mut out, &mut content_bounds, 0.0, 0.0, true);
    }

    // Nodes.
    let nodes_start = timing_enabled.then(std::time::Instant::now);
    out.push_str(r#"<g class="nodes">"#);

    if wrap_nodes_root {
        let _ = write!(
            &mut out,
            r#"<g class="root" transform="translate({}, {})">"#,
            fmt(nodes_root_dx),
            fmt(nodes_root_dy)
        );
        render_clusters_edges_and_labels(
            &mut out,
            &mut content_bounds,
            nodes_root_dx,
            nodes_root_dy,
            true,
        );
        out.push_str(r#"<g class="nodes">"#);
    }

    let ClassNodeRenderOrder {
        layout_nodes_by_id,
        ordered_ids,
        namespace_key_set,
        clusters_by_id,
    } = build_class_node_render_order(
        layout,
        model,
        &class_nodes_by_id,
        wrap_nodes_root,
        single_namespace_id,
        render_namespaces_as_subgraphs,
    );

    let mut inner_nodes_group_open = wrap_nodes_root;
    let mut namespace_subgraph_state = ClassNamespaceSubgraphState::default();
    for id in ordered_ids {
        if wrap_nodes_root && inner_nodes_group_open {
            let parent = class_nodes_by_id.get(id).and_then(|n| n.parent.as_deref());
            let should_be_inner = single_namespace_id.is_some_and(|ns| parent == Some(ns));
            if !should_be_inner {
                // Close the nested wrapper, then continue emitting remaining nodes at the outer level.
                out.push_str("</g>"); // inner nodes
                out.push_str("</g>"); // inner root
                inner_nodes_group_open = false;
            }
        }

        if render_namespaces_as_subgraphs {
            let parent = class_nodes_by_id.get(id).and_then(|n| n.parent.as_deref());
            let parent = parent.filter(|p| namespace_key_set.contains(p));
            transition_class_namespace_subgraph(
                &mut out,
                &mut content_bounds,
                &mut namespace_subgraph_state,
                parent,
                &clusters_by_id,
            );
        }

        let (active_nodes_root_dx, active_nodes_root_dy) =
            if wrap_nodes_root && inner_nodes_group_open {
                (nodes_root_dx, nodes_root_dy)
            } else {
                (0.0, 0.0)
            };
        let (active_namespace_root_dx, active_namespace_root_dy) =
            namespace_subgraph_state.root_offset.unwrap_or((0.0, 0.0));

        let Some(n) = layout_nodes_by_id.get(id).copied() else {
            continue;
        };

        let in_namespace_root =
            render_namespaces_as_subgraphs && namespace_subgraph_state.active_subgraph.is_some();
        let node_tx = if in_namespace_root {
            n.x - active_namespace_root_dx
        } else {
            n.x + content_tx
        };
        let node_ty = if in_namespace_root {
            n.y + content_ty - active_namespace_root_dy
        } else {
            n.y + content_ty
        };
        let node_bounds_tx = node_tx + active_namespace_root_dx + active_nodes_root_dx;
        let node_bounds_ty = node_ty + active_namespace_root_dy + active_nodes_root_dy;

        if let Some(note) = note_by_id.get(n.id.as_str()).copied() {
            let stats = render_class_note_node(
                ClassNoteRenderState {
                    out: &mut out,
                    content_bounds: &mut content_bounds,
                    sanitize_config: &mut sanitize_config,
                },
                note,
                n,
                ClassNodeRenderPosition {
                    node_tx,
                    node_ty,
                    node_bounds_tx,
                    node_bounds_ty,
                },
                &ClassNoteRenderContext {
                    diagram_id,
                    effective_config,
                    measurer,
                    text_style: &text_style,
                    line_height,
                    use_html_labels: diagram_use_html_labels,
                    timing_enabled,
                },
            );
            detail.notes_sanitize += stats.notes_sanitize;
            detail.path_bounds += stats.path_bounds;
            detail.path_bounds_calls += stats.path_bounds_calls;
            continue;
        }

        if let Some(iface) = iface_by_id.get(n.id.as_str()).copied() {
            render_class_interface_node(
                ClassInterfaceRenderState {
                    out: &mut out,
                    content_bounds: &mut content_bounds,
                },
                iface,
                n,
                ClassNodeRenderPosition {
                    node_tx,
                    node_ty,
                    node_bounds_tx,
                    node_bounds_ty,
                },
                &ClassInterfaceRenderContext {
                    measurer,
                    text_style: &text_style,
                    line_height,
                },
            );
            continue;
        }

        let Some(node) = class_nodes_by_id.get(n.id.as_str()).copied() else {
            continue;
        };

        let node_inline_styles = class_apply_inline_styles(node);
        let node_style_attr = node_inline_styles.style_attr.as_str();
        let node_fill = node_inline_styles
            .fill
            .unwrap_or(default_node_fill.as_str());
        let node_stroke = node_inline_styles
            .stroke
            .unwrap_or(default_node_stroke.as_str());
        let node_stroke_width = node_inline_styles
            .stroke_width
            .unwrap_or("1.3")
            .trim_end_matches("px")
            .trim();
        let node_stroke_dasharray = node_inline_styles.stroke_dasharray.unwrap_or("0 0");

        let node_link_open = render_class_node_shell_open(
            &mut out,
            node,
            ClassNodeRenderPosition {
                node_tx,
                node_ty,
                node_bounds_tx,
                node_bounds_ty,
            },
        );
        let basic_container = render_class_node_basic_container(
            ClassNodeRenderState {
                out: &mut out,
                content_bounds: &mut content_bounds,
            },
            node,
            n,
            ClassNodeRenderPosition {
                node_tx,
                node_ty,
                node_bounds_tx,
                node_bounds_ty,
            },
            &ClassNodeBasicContainerContext {
                diagram_id,
                node_style_attr,
                node_fill,
                node_stroke,
                node_stroke_width,
                node_stroke_dasharray,
                timing_enabled,
            },
        );
        detail.path_bounds += basic_container.stats.path_bounds;
        detail.path_bounds_calls += basic_container.stats.path_bounds_calls;

        if diagram_use_html_labels {
            let html_stats = render_class_html_node_body(
                ClassNodeRenderState {
                    out: &mut out,
                    content_bounds: &mut content_bounds,
                },
                ClassNodeRenderPosition {
                    node_tx,
                    node_ty,
                    node_bounds_tx,
                    node_bounds_ty,
                },
                node,
                basic_container.geometry,
                layout
                    .class_row_metrics_by_id
                    .get(n.id.as_str())
                    .map(|rows| rows.as_ref()),
                &ClassHtmlNodeBodyContext {
                    measurer,
                    text_style: &text_style,
                    html_calc_text_style: &html_calc_text_style,
                    line_height,
                    class_padding: _class_padding,
                    hide_empty_members_box,
                    node_style_attr,
                    node_stroke,
                    node_stroke_width,
                    node_stroke_dasharray,
                    timing_enabled,
                },
            );
            detail.path_bounds += html_stats.path_bounds;
            detail.path_bounds_calls += html_stats.path_bounds_calls;
        } else {
            let svg_stats = render_class_svg_node_body(
                ClassNodeRenderState {
                    out: &mut out,
                    content_bounds: &mut content_bounds,
                },
                ClassNodeRenderPosition {
                    node_tx,
                    node_ty,
                    node_bounds_tx,
                    node_bounds_ty,
                },
                node,
                basic_container.geometry,
                &ClassSvgNodeBodyContext {
                    measurer,
                    text_style: &text_style,
                    font_size,
                    wrap_probe_font_size,
                    class_padding: _class_padding,
                    hide_empty_members_box,
                    node_style_attr,
                    node_stroke,
                    node_stroke_width,
                    node_stroke_dasharray,
                    timing_enabled,
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

    if render_namespaces_as_subgraphs {
        close_class_namespace_subgraph(&mut out, &mut namespace_subgraph_state);
    }

    if inner_nodes_group_open {
        out.push_str("</g>"); // inner nodes
        out.push_str("</g>"); // inner root
    }
    out.push_str("</g>"); // outer nodes
    out.push_str("</g>"); // root
    out.push_str("</g>"); // wrapper
    if let Some(s) = nodes_start {
        detail.nodes += s.elapsed();
    }

    drop(render_guard);
    let viewbox_guard = timing_enabled.then(|| TimingGuard::new(&mut timings.viewbox));

    let viewbox_attrs = class_viewbox_attrs(ClassViewBoxContext {
        diagram_id,
        model,
        content_bounds,
        viewport_padding,
        diagram_title,
        has_acc_title: root_open.has_acc_title,
        has_acc_descr: root_open.has_acc_descr,
    });

    // Mermaid renders the diagram title as a direct child of `<svg>` (outside the wrapper `<g>`),
    // centered in the root viewport.
    if let Some(title) = viewbox_attrs.title.as_ref() {
        let _ = write!(
            &mut out,
            r#"<text text-anchor="middle" x="{}" y="{}" class="classDiagramTitleText">{}</text>"#,
            fmt(title.x),
            fmt(title.y),
            escape_xml_display(title.text)
        );
    }

    drop(viewbox_guard);
    let finalize_guard = timing_enabled.then(|| TimingGuard::new(&mut timings.finalize_svg));

    // Avoid a full-string scan + allocation for placeholder replacement by patching the initial
    // `<svg ...>` attributes in-place.
    out.replace_range(
        root_open.viewbox_placeholder_range,
        viewbox_attrs.view_box_attr.as_str(),
    );
    out.replace_range(
        root_open.max_width_placeholder_range,
        viewbox_attrs.max_w_attr.as_str(),
    );

    out.push_str("</svg>");
    drop(finalize_guard);

    if let Some(s) = total_start {
        timings.total = s.elapsed();
        emit_class_render_timing(&timings, &detail, layout);
    }
    Ok(out)
}

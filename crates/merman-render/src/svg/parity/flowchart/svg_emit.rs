use super::defs::prepare_flowchart_defs;
use super::document::{FlowchartSvgDocumentRequest, prepare_flowchart_svg_document};
use super::render_config::{FlowchartRenderConfig, prepare_flowchart_render_config};
use super::render_input::{FlowchartRenderInputs, prepare_flowchart_render_inputs};
use super::viewbox::{
    FlowchartRenderedBoundsRequest, FlowchartViewboxBounds, FlowchartViewboxBoundsRequest,
    prepare_flowchart_rendered_bounds, prepare_flowchart_viewbox_bounds,
};
use super::*;

pub(in crate::svg::parity) fn render_flowchart_svg_artifact(
    artifact: &crate::family::FlowchartFamilyArtifact<FlowchartLayout>,
    metadata: &merman_core::ParseMetadata,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    render_flowchart_svg_model(
        FlowchartSvgModelRequest {
            layout: artifact.pair().layout(),
            swimlane_layout: None,
            model: artifact.pair().semantic(),
            render_context: artifact.render_context(),
            effective_config: &metadata.effective_config,
            diagram_type: metadata.diagram_type.as_str(),
            diagram_title: metadata.title.as_deref(),
            presentation_policy: artifact.policy(),
            svg_label_sidecar: artifact.svg_label_sidecar(),
        },
        options,
    )
}

pub(super) struct FlowchartSvgModelRequest<'a> {
    pub(super) layout: &'a FlowchartLayout,
    pub(super) swimlane_layout: Option<&'a crate::model::SwimlaneLayout>,
    pub(super) model: &'a crate::flowchart::FlowchartModel,
    pub(super) render_context: &'a crate::flowchart::FlowchartRenderContext,
    pub(super) effective_config: &'a merman_core::MermaidConfig,
    pub(super) diagram_type: &'a str,
    pub(super) diagram_title: Option<&'a str>,
    pub(super) presentation_policy: Option<crate::presentation::FlowchartPresentationPolicy>,
    pub(super) svg_label_sidecar: &'a crate::flowchart::FlowchartSvgLabelSidecar,
}

pub(super) fn render_flowchart_svg_model(
    request: FlowchartSvgModelRequest<'_>,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let FlowchartSvgModelRequest {
        layout,
        swimlane_layout,
        model,
        render_context,
        effective_config,
        diagram_type,
        diagram_title,
        presentation_policy,
        svg_label_sidecar,
    } = request;
    let render_model = crate::flowchart::FlowchartRenderModelRef::new(model, render_context);
    let model = &render_model;
    if model
        .nodes
        .iter()
        .any(|node| node.layout_shape.as_deref() == Some("ellipse"))
    {
        return Err(crate::Error::InvalidModel {
            message: "No such shape: ellipse. Please check your syntax.".to_string(),
        });
    }

    let render_timing = options.timing();
    let measurer = options.text_measurer();
    let mut timings = timing::RenderTimings::default();
    let total_timer = render_timing.start();

    let effective_config_value = effective_config.as_value();
    let hand_drawn_seed = options.rough_randomness(
        effective_config_value
            .get("handDrawnSeed")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(options.seed() as f64),
        "render.flowchart.roughjs",
    );

    let diagram_id = options.diagram_id_or("merman");
    let _g_build_ctx = render_timing.section(&mut timings.build_ctx);

    let FlowchartRenderInputs {
        mut render_edges,
        extra_nodes,
    } = prepare_flowchart_render_inputs(model, layout.uses_elk_adapter_dom);
    if let Some(swimlane_layout) = swimlane_layout {
        super::swimlane::apply_swimlane_edge_curves(&mut render_edges, swimlane_layout);
    }

    let FlowchartRenderConfig {
        font_family,
        font_size,
        wrapping_width,
        node_html_labels,
        edge_html_labels,
        swimlane_title_html_labels,
        node_wrap_mode,
        edge_wrap_mode,
        diagram_padding,
        use_max_width,
        title_top_margin,
        node_padding,
        text_style,
        html_label_text_style,
        default_edge_interpolate,
        default_edge_style,
        node_border_color,
        node_fill_color,
        node_corner_radius,
        edge_corner_radius,
        edge_label_padding,
        compact_edge_corners,
    } = prepare_flowchart_render_config(
        model,
        effective_config_value,
        diagram_type,
        presentation_policy,
    );

    let mut nodes_by_id: FxHashMap<&str, &crate::flowchart::FlowNode> =
        FxHashMap::with_capacity_and_hasher(
            model.nodes.len() + extra_nodes.len(),
            Default::default(),
        );
    for n in &model.nodes {
        nodes_by_id.insert(n.id.as_str(), n);
    }
    for n in &extra_nodes {
        let _ = nodes_by_id.entry(n.id.as_str()).or_insert(n);
    }

    // Source-ported ELK should preserve Mermaid's edge emission order, not the layout engine's
    // internal reordering. `render_edges` already reflects the source-backed ordering rules.
    let edge_order: Vec<&str> = render_edges
        .iter()
        .map(|e| e.as_ref().id.as_str())
        .collect();
    let mut edges_by_id: FxHashMap<&str, &crate::flowchart::FlowEdge> =
        FxHashMap::with_capacity_and_hasher(render_edges.len(), Default::default());
    for e in &render_edges {
        let edge = e.as_ref();
        edges_by_id.insert(edge.id.as_str(), edge);
    }

    let swimlane_direction = swimlane_layout.map(|layout| layout.direction);
    let swimlane_lanes_by_id: FxHashMap<&str, &crate::model::SwimlaneLaneLayout> = swimlane_layout
        .into_iter()
        .flat_map(|layout| layout.lanes.iter())
        .map(|lane| (lane.id.as_str(), lane))
        .collect();
    let swimlane_edge_label_edges_by_node_id: FxHashMap<&str, &crate::flowchart::FlowEdge> =
        swimlane_layout
            .into_iter()
            .flat_map(|layout| layout.edges.iter())
            .filter_map(|layout_edge| {
                let label_node_id = layout_edge.label_node_id.as_deref()?;
                let edge = edges_by_id.get(layout_edge.id.as_str()).copied()?;
                Some((label_node_id, edge))
            })
            .collect();
    let mut subgraph_order: Vec<&str> = Vec::with_capacity(model.subgraphs.len());
    let mut subgraphs_by_id: FxHashMap<&str, &crate::flowchart::FlowSubgraph> =
        FxHashMap::with_capacity_and_hasher(model.subgraphs.len(), Default::default());
    let mut subgraph_indices_by_id: FxHashMap<&str, usize> =
        FxHashMap::with_capacity_and_hasher(model.subgraphs.len(), Default::default());
    let mut subgraph_ids_with_children: FxHashSet<&str> = FxHashSet::default();
    for (subgraph_index, sg) in model.subgraphs.iter().enumerate() {
        let id = sg.id.as_str();
        if let std::collections::hash_map::Entry::Vacant(entry) = subgraphs_by_id.entry(id) {
            entry.insert(sg);
            subgraph_indices_by_id.insert(id, subgraph_index);
            subgraph_order.push(id);
        }
        if !sg.nodes.is_empty() {
            subgraph_ids_with_children.insert(id);
        }
    }

    let mut parent: FxHashMap<&str, &str> = FxHashMap::default();
    for sg in model.subgraphs.iter().rev() {
        let sg_id = sg.id.as_str();
        for child in &sg.nodes {
            parent.insert(child.as_str(), sg_id);
        }
    }
    for n in &extra_nodes {
        let id = n.id.as_str();
        let Some((base, _)) = id.split_once("---") else {
            continue;
        };
        if let Some(&p) = parent.get(base) {
            parent.insert(id, p);
        }
    }

    // Layout extraction is the source of truth for recursive cluster roots. Recomputing this from
    // semantic edges loses Mermaid 11.16's explicit-direction extraction branch and can make every
    // node inside an extracted cluster disappear from the SVG DOM.
    let recursive_clusters: FxHashSet<&str> = layout
        .dom_node_order_by_root
        .keys()
        .filter(|id| !id.is_empty())
        .map(String::as_str)
        .collect();

    let mut layout_nodes_by_id: FxHashMap<&str, &LayoutNode> =
        FxHashMap::with_capacity_and_hasher(layout.nodes.len(), Default::default());
    for n in &layout.nodes {
        layout_nodes_by_id.insert(n.id.as_str(), n);
    }

    let mut layout_edges_by_id: FxHashMap<&str, &crate::model::LayoutEdge> =
        FxHashMap::with_capacity_and_hasher(layout.edges.len(), Default::default());
    for e in &layout.edges {
        layout_edges_by_id.insert(e.id.as_str(), e);
    }

    let mut layout_clusters_by_id: FxHashMap<&str, &LayoutCluster> =
        FxHashMap::with_capacity_and_hasher(layout.clusters.len(), Default::default());
    for c in &layout.clusters {
        layout_clusters_by_id.insert(c.id.as_str(), c);
    }

    // Mermaid flowchart-v2 does not translate the root `.root` group; node/edge coordinates are
    // already in the Dagre coordinate space (including Dagre's fixed `marginx/marginy=8`).
    // `diagramPadding` is applied only when computing the final SVG viewBox.
    let tx = 0.0;
    let ty = 0.0;

    let node_dom_index = flowchart_node_dom_indices(model);

    let flowchart_edge_trace = options.debug.flowchart_edge_trace();
    let icon_registry = options.icon_registry();
    let icon_scope_prefix = icon_registry
        .map(|_| {
            crate::svg::icon_registry::IconIdScopePrefix::from_parts(
                &[diagram_id.semantic_str(), "-flowchart-icon-"],
                options.work_meter(),
            )
        })
        .transpose()?;
    let ctx = FlowchartRenderCtx {
        model,
        diagram_id,
        diagram_type,
        tx,
        ty,
        measurer,
        config: effective_config,
        hand_drawn_seed,
        work_meter: options.work_meter(),
        math_renderer: options.math_renderer(),
        svg_label_sidecar: Some(svg_label_sidecar),
        icon_registry,
        icon_scope_prefix,
        security_level_loose: effective_config.get_str("securityLevel") == Some("loose"),
        node_html_labels,
        edge_html_labels,
        swimlane_title_html_labels,
        uses_elk_adapter_dom: layout.uses_elk_adapter_dom,
        class_defs: &model.class_defs,
        node_border_color,
        node_fill_color,
        node_corner_radius,
        edge_corner_radius,
        edge_label_padding,
        compact_edge_corners,
        default_edge_interpolate,
        default_edge_style,
        trace_edge_id: flowchart_edge_trace.map(|(edge_id, _)| edge_id),
        trace_collector: flowchart_edge_trace.map(|(_, collector)| collector),
        subgraph_order,
        edge_order,
        nodes_by_id,
        edges_by_id,
        subgraphs_by_id,
        subgraph_indices_by_id,
        subgraph_ids_with_children,
        tooltips: &model.tooltips,
        recursive_clusters,
        parent,
        layout_nodes_by_id,
        layout_edges_by_id,
        layout_clusters_by_id,
        swimlane_direction,
        swimlane_lanes_by_id,
        swimlane_edge_label_edges_by_node_id,
        dom_node_order_by_root: &layout.dom_node_order_by_root,
        node_dom_index,
        node_padding,
        wrapping_width,
        node_wrap_mode,
        edge_wrap_mode,
        text_style,
        html_label_text_style,
    };

    let mut edge_path_cache: FxHashMap<&str, FlowchartEdgePathCacheEntry> =
        FxHashMap::with_capacity_and_hasher(render_edges.len(), Default::default());

    let subgraph_title_y_shift = crate::flowchart::FlowchartConfigView::new(effective_config_value)
        .render_subgraph_title_y_shift();

    fn self_loop_label_base_node_id(id: &str) -> Option<&str> {
        let mut parts = id.split("---");
        let a = parts.next()?;
        let b = parts.next()?;
        let n = parts.next()?;
        if parts.next().is_some() {
            return None;
        }
        if a != b {
            return None;
        }
        if n != "1" && n != "2" {
            return None;
        }
        Some(a)
    }

    drop(_g_build_ctx);

    let mut detail = FlowchartRenderDetails::default();
    let mut viewbox_edge_curve_bounds = std::time::Duration::ZERO;
    let _g_viewbox = render_timing.section(&mut timings.viewbox);

    let effective_parent_for_id = |id: &str| -> Option<&str> {
        let mut cur = ctx.parent.get(id).copied();
        if cur.is_none()
            && let Some(base) = self_loop_label_base_node_id(id)
        {
            cur = ctx.parent.get(base).copied();
        }
        while let Some(p) = cur {
            if ctx.subgraphs_by_id.contains_key(p) && !ctx.recursive_clusters.contains(p) {
                cur = ctx.parent.get(p).copied();
                continue;
            }
            return Some(p);
        }
        None
    };

    let bounds = prepare_flowchart_rendered_bounds(
        FlowchartRenderedBoundsRequest {
            ctx: &ctx,
            layout,
            subgraph_title_y_shift,
        },
        &effective_parent_for_id,
    );
    let FlowchartViewboxBounds {
        diagram_title,
        title_anchor_x,
        bbox_min_x,
        bbox_min_y,
        bbox_max_x,
        bbox_max_y,
    } = prepare_flowchart_viewbox_bounds(
        FlowchartViewboxBoundsRequest {
            ctx: &ctx,
            render_edges: &render_edges,
            base_bounds: bounds,
            diagram_title,
            font_family: &font_family,
            title_top_margin,
            timing: render_timing,
            viewbox_edge_curve_bounds: &mut viewbox_edge_curve_bounds,
            detail: &mut detail,
            edge_path_cache: &mut edge_path_cache,
        },
        &effective_parent_for_id,
    )?;

    let document = prepare_flowchart_svg_document(FlowchartSvgDocumentRequest {
        family_kind: if swimlane_layout.is_some() {
            crate::family::RenderFamilyKind::Swimlane
        } else {
            crate::family::RenderFamilyKind::Flowchart
        },
        diagram_id,
        diagram_type,
        model,
        use_max_width,
        diagram_padding,
        bbox_min_x,
        bbox_min_y,
        bbox_max_x,
        bbox_max_y,
    });

    drop(_g_viewbox);
    let _g_render_svg = render_timing.section(&mut timings.render_svg);

    let mut css = flowchart_css(
        diagram_id,
        effective_config_value,
        &font_family,
        font_size,
        &model.class_defs,
    )?;
    if swimlane_layout.is_some() {
        css.push_str(&super::swimlane::swimlane_css(diagram_id, effective_config));
    }

    let estimated_svg_bytes = 2048usize
        + css.len()
        + layout.nodes.len().saturating_mul(256)
        + render_edges.len().saturating_mul(256)
        + layout.clusters.len().saturating_mul(128);
    let mut out = String::with_capacity(estimated_svg_bytes);

    let root_document = document.push_root_open(&mut out)?;
    document.push_accessibility_metadata(&mut out);
    out.push_str("<style>");
    out.push_str(&css);
    out.push_str("</style>");

    let defs = prepare_flowchart_defs(diagram_id, diagram_type, &ctx);

    let mut root_session = FlowchartRootRenderSession {
        timing: render_timing,
        details: &mut detail,
        edge_cache: &mut edge_path_cache,
    };
    if layout.uses_elk_adapter_dom {
        out.push_str("<g>");
        defs.push_base_markers(&mut out);
        defs.push_extra_markers(&mut out);
        out.push_str("</g>");
        push_flowchart_shadow_defs(&mut out, diagram_id, effective_config_value);
        render_flowchart_elk_root_groups(&mut out, &ctx, &mut root_session)?;
    } else {
        push_flowchart_shadow_defs(&mut out, diagram_id, effective_config_value);
        out.push_str("<g>");
        defs.push_base_markers(&mut out);
        render_flowchart_root(&mut out, &ctx, None, 0.0, 0.0, &mut root_session)?;

        defs.push_extra_markers(&mut out);
        out.push_str("</g>");
    }
    push_flowchart_gradient(&mut out, diagram_id, effective_config_value);
    if let Some(title) = diagram_title.as_deref() {
        let title_x = title_anchor_x;
        let title_y = -title_top_margin;
        let _ = write!(
            &mut out,
            r#"<text text-anchor="middle" x="{}" y="{}" class="flowchartTitleText">{}</text>"#,
            fmt(title_x),
            fmt(title_y),
            escape_xml(title)
        );
    }
    out.push_str("</svg>\n");

    drop(_g_render_svg);
    timings.total = total_timer
        .map(merman_core::runtime::OperationTimer::elapsed)
        .unwrap_or_default();
    if render_timing.is_enabled() {
        eprintln!(
            "[render-timing] diagram=flowchart-v2 total={:?} deserialize={:?} build_ctx={:?} viewbox={:?} viewbox_edge_curve_bounds={:?} viewbox_edge_curve_lca={:?} viewbox_edge_curve_offsets={:?} viewbox_edge_curve_geom={:?} viewbox_edge_curve_bbox_union={:?} viewbox_edge_curve_geom_calls={} viewbox_edge_curve_geom_skipped_bounds={} render_svg={:?} finalize={:?} root_calls={} clusters={:?} edges_select={:?} edge_paths={:?} edge_labels={:?} dom_order={:?} nodes={:?} node_style_compile={:?} node_roughjs={:?} node_roughjs_calls={} node_label_html={:?} node_label_html_calls={} nested_roots={:?}",
            timings.total,
            timings.deserialize_model,
            timings.build_ctx,
            timings.viewbox,
            viewbox_edge_curve_bounds,
            detail.viewbox_edge_curve_lca,
            detail.viewbox_edge_curve_offsets,
            detail.viewbox_edge_curve_geom,
            detail.viewbox_edge_curve_bbox_union,
            detail.viewbox_edge_curve_geom_calls,
            detail.viewbox_edge_curve_geom_skipped_bounds,
            timings.render_svg,
            timings.finalize_svg,
            detail.root_calls,
            detail.clusters,
            detail.edges_select,
            detail.edge_paths,
            detail.edge_labels,
            detail.dom_order,
            detail.nodes,
            detail.node_style_compile,
            detail.node_roughjs,
            detail.node_roughjs_calls,
            detail.node_label_html,
            detail.node_label_html_calls,
            detail.nested_roots,
        );
    }
    root_document.complete(out)
}

fn push_flowchart_shadow_defs(
    out: &mut String,
    diagram_id: SvgDiagramId<'_>,
    effective_config_value: &serde_json::Value,
) {
    let flood_color = effective_config_value
        .get("theme")
        .and_then(|v| v.as_str())
        .filter(|theme| theme.contains("dark"))
        .map(|_| "#FFFFFF")
        .unwrap_or("#000000");
    let _ = write!(
        out,
        r#"<defs><filter id="{}-drop-shadow" height="130%" width="130%"><feDropShadow dx="4" dy="4" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs><defs><filter id="{}-drop-shadow-small" height="150%" width="150%"><feDropShadow dx="2" dy="2" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs>"#,
        diagram_id, flood_color, diagram_id, flood_color
    );
}

fn push_flowchart_gradient(
    out: &mut String,
    diagram_id: SvgDiagramId<'_>,
    effective_config_value: &serde_json::Value,
) {
    if !config_bool(effective_config_value, &["themeVariables", "useGradient"]).unwrap_or(false) {
        return;
    }

    let gradient_start =
        config_string(effective_config_value, &["themeVariables", "gradientStart"])
            .or_else(|| {
                config_string(
                    effective_config_value,
                    &["themeVariables", "primaryBorderColor"],
                )
            })
            .unwrap_or_else(|| "#9370DB".to_string());
    let gradient_stop = config_string(effective_config_value, &["themeVariables", "gradientStop"])
        .or_else(|| {
            config_string(
                effective_config_value,
                &["themeVariables", "secondaryBorderColor"],
            )
        })
        .unwrap_or_else(|| gradient_start.clone());

    let gradient_start = escape_xml(&gradient_start);
    let gradient_stop = escape_xml(&gradient_stop);
    let _ = write!(
        out,
        r#"<linearGradient id="{}-gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%"><stop offset="0%" stop-color="{}" stop-opacity="1"/><stop offset="100%" stop-color="{}" stop-opacity="1"/></linearGradient>"#,
        diagram_id,
        gradient_start.as_str(),
        gradient_stop.as_str()
    );
}

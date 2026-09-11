use super::defs::{FlowchartMarkerEmissionPlan, prepare_flowchart_defs};
use super::document::{FlowchartSvgDocumentRequest, prepare_flowchart_svg_document};
use super::document_ids::{FlowchartDocumentIdRequest, FlowchartDocumentIds};
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
            svg_label_sidecar: artifact.svg_label_sidecar(),
            theme_evidence: artifact.theme_evidence(),
            edge_style_plan: artifact.edge_style_plan(),
            edge_theme: artifact.edge_theme(),
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
    pub(super) svg_label_sidecar: &'a crate::flowchart::FlowchartSvgLabelSidecar,
    pub(super) theme_evidence: &'a crate::flowchart::FlowchartThemeEvidenceRecorder,
    pub(super) edge_style_plan: &'a FlowchartEdgeStylePlan,
    pub(super) edge_theme: &'a crate::flowchart::FlowchartEdgeThemeStyle,
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
        svg_label_sidecar,
        theme_evidence,
        edge_style_plan,
        edge_theme,
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
    layout.edge_owners.validate(&layout.edges, &model.edges)?;

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
    let diagram_id_value = diagram_id.semantic_str();
    let _g_build_ctx = render_timing.section(&mut timings.build_ctx);

    let FlowchartRenderInputs {
        mut render_edges,
        extra_nodes,
    } = prepare_flowchart_render_inputs(
        model,
        render_context,
        &layout.edge_owners,
        layout.uses_elk_adapter_dom,
    );
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
        node_stroke_width,
        node_typography_config_ownership,
        node_label_fill_config_override,
        node_border_config_override,
        node_fill_config_override,
        node_stroke_width_config_override,
        edge_stroke_config_override,
        cluster_fill_color,
        cluster_stroke_color,
        cluster_fill_config_override,
        cluster_stroke_config_override,
        node_corner_radius,
        node_corner_radius_config_override,
        edge_corner_radius,
        edge_label_padding,
        compact_edge_corners,
    } = prepare_flowchart_render_config(
        model,
        effective_config,
        diagram_type,
        svg_label_sidecar.base_typography(),
        svg_label_sidecar.edge_label_padding(),
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
    let edge_order: Vec<super::render_input::FlowchartRenderEdgeRef<'_>> =
        render_edges.iter().map(|edge| edge.as_ref()).collect();
    let mut edges_by_key: FxHashMap<
        crate::flowchart::FlowchartEdgeKey,
        super::render_input::FlowchartRenderEdgeRef<'_>,
    > = FxHashMap::with_capacity_and_hasher(render_edges.len(), Default::default());
    for e in &render_edges {
        let edge = e.as_ref();
        edges_by_key.insert(edge.key, edge);
    }

    let swimlane_direction = swimlane_layout.map(|layout| layout.direction);
    let swimlane_lanes_by_id: FxHashMap<&str, &crate::model::SwimlaneLaneLayout> = swimlane_layout
        .into_iter()
        .flat_map(|layout| layout.lanes.iter())
        .map(|lane| (lane.id.as_str(), lane))
        .collect();
    let swimlane_lane_order = swimlane_layout
        .into_iter()
        .flat_map(|layout| layout.lanes.iter())
        .map(|lane| lane.id.as_str())
        .collect::<Vec<_>>();
    let swimlane_edge_label_edges_by_node_id: FxHashMap<
        &str,
        super::render_input::FlowchartRenderEdgeRef<'_>,
    > = swimlane_layout
        .into_iter()
        .flat_map(|layout| layout.edges.iter().zip(layout.edge_owners.iter()))
        .filter_map(|(layout_edge, owner)| {
            let label_node_id = layout_edge.label_node_id.as_deref()?;
            let edge = edges_by_key.get(&owner).copied()?;
            Some((label_node_id, edge))
        })
        .collect();
    let mut subgraph_order: Vec<&str> = Vec::with_capacity(model.subgraphs.len());
    let mut subgraphs_by_id: FxHashMap<&str, &crate::flowchart::FlowSubgraph> =
        FxHashMap::with_capacity_and_hasher(model.subgraphs.len(), Default::default());
    let mut subgraph_index_by_id: FxHashMap<&str, usize> =
        FxHashMap::with_capacity_and_hasher(model.subgraphs.len(), Default::default());
    let mut subgraph_ids_with_children: FxHashSet<&str> = FxHashSet::default();
    for (subgraph_index, sg) in model.subgraphs.iter().enumerate() {
        let id = sg.id.as_str();
        if let std::collections::hash_map::Entry::Vacant(entry) = subgraphs_by_id.entry(id) {
            entry.insert(sg);
            subgraph_index_by_id.insert(id, subgraph_index);
            subgraph_order.push(id);
        }
        if !sg.nodes.is_empty()
            && !render_context.is_subgraph_collapsed(id)
            && render_context.collapsed_replacement(id).is_none()
        {
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

    let mut layout_edges_by_key: FxHashMap<
        crate::flowchart::FlowchartEdgeKey,
        &crate::model::LayoutEdge,
    > = FxHashMap::with_capacity_and_hasher(layout.edges.len(), Default::default());
    for (key, edge) in layout.edge_owners.iter().zip(&layout.edges) {
        if layout_edges_by_key.insert(key, edge).is_some() {
            return Err(crate::Error::InvalidModel {
                message: format!(
                    "Flowchart layout contains multiple edges for semantic owner {}",
                    key.semantic_index()
                ),
            });
        }
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
    let document_ids = FlowchartDocumentIds::prepare(FlowchartDocumentIdRequest {
        diagram_id: diagram_id_value,
        edge_order: &edge_order,
        node_dom_index: &node_dom_index,
        subgraph_index_by_id: &subgraph_index_by_id,
        layout_nodes: &layout.nodes,
        swimlane_nodes: swimlane_layout.map(|layout| layout.nodes.as_slice()),
        swimlane_lanes: swimlane_layout.map(|layout| layout.lanes.as_slice()),
    });
    let node_theme_ordinals =
        flowchart_node_theme_ordinals(&model.nodes, &model.subgraphs, layout.uses_elk_adapter_dom);
    let flowchart_edge_trace = options.debug.flowchart_edge_trace();
    let checkpoint_emit = || options.checkpoint_emit();
    let title_paint = crate::flowchart::FlowchartTitlePaintPlan::resolve(
        options.resolved_theme(),
        effective_config,
        options.work_meter(),
    )?;
    let ctx = FlowchartRenderCtx {
        title_paint: &title_paint,
        model,
        diagram_id,
        diagram_type,
        tx,
        ty,
        measurer,
        config: effective_config,
        hand_drawn_seed,
        work_meter: options.work_meter(),
        resolved_theme: options.resolved_theme(),
        theme_evidence,
        emit: FlowchartEmitCheckpoint::new(&checkpoint_emit),
        math_renderer: options.math_renderer(),
        svg_label_sidecar: Some(svg_label_sidecar),
        icon_registry: options.icon_registry(),
        security_level_loose: effective_config.get_str("securityLevel") == Some("loose"),
        node_html_labels,
        edge_html_labels,
        swimlane_title_html_labels,
        uses_elk_adapter_dom: layout.uses_elk_adapter_dom,
        class_defs: &model.class_defs,
        edge_style_plan,
        document_ids: &document_ids,
        node_border_color,
        node_fill_color,
        node_stroke_width,
        node_typography_config_ownership,
        node_label_fill_config_override,
        node_border_config_override,
        node_fill_config_override,
        node_stroke_width_config_override,
        edge_stroke_config_override,
        cluster_fill_color,
        cluster_stroke_color,
        cluster_fill_config_override,
        cluster_stroke_config_override,
        node_corner_radius,
        node_corner_radius_config_override,
        edge_corner_radius,
        edge_label_padding,
        compact_edge_corners,
        default_edge_interpolate,
        default_edge_style,
        edge_theme,
        trace_edge_id: flowchart_edge_trace.map(|(edge_id, _)| edge_id),
        trace_collector: flowchart_edge_trace.map(|(_, collector)| collector),
        subgraph_order,
        edge_order,
        nodes_by_id,
        subgraphs_by_id,
        subgraph_indices_by_id: subgraph_index_by_id,
        subgraph_ids_with_children,
        tooltips: &model.tooltips,
        recursive_clusters,
        parent,
        layout_nodes_by_id,
        layout_edges_by_key,
        layout_clusters_by_id,
        swimlane_direction,
        swimlane_lanes_by_id,
        swimlane_lane_order,
        swimlane_edge_label_edges_by_node_id,
        dom_node_order_by_root: &layout.dom_node_order_by_root,
        node_dom_index,
        node_theme_ordinals,
        node_padding,
        wrapping_width,
        node_wrap_mode,
        edge_wrap_mode,
        text_style,
        html_label_text_style,
    };

    let hierarchy_plan = FlowchartHierarchyPlan::prepare(&ctx)?;
    let cluster_theme_plan = crate::flowchart::FlowchartClusterThemePlan::prepare(
        ctx.resolved_theme,
        ctx.subgraph_order
            .iter()
            .copied()
            .chain(ctx.swimlane_lane_order.iter().copied()),
        hierarchy_plan.rendered_cluster_ids(),
        ctx.work_meter,
    )?;
    title_paint.begin_terminal_emission(hierarchy_plan.rendered_cluster_ids(), ctx.work_meter)?;
    let marker_plan = FlowchartMarkerEmissionPlan::prepare(&ctx, &hierarchy_plan)?;

    let mut edge_path_cache: FxHashMap<
        crate::flowchart::FlowchartEdgeKey,
        FlowchartEdgePathCacheEntry,
    > = FxHashMap::with_capacity_and_hasher(render_edges.len(), Default::default());

    let subgraph_title_y_shift = crate::flowchart::FlowchartConfigView::new(effective_config_value)
        .render_subgraph_title_y_shift();

    drop(_g_build_ctx);

    let mut detail = FlowchartRenderDetails::default();
    let mut viewbox_edge_curve_bounds = std::time::Duration::ZERO;
    let _g_viewbox = render_timing.section(&mut timings.viewbox);

    let bounds = prepare_flowchart_rendered_bounds(
        FlowchartRenderedBoundsRequest {
            ctx: &ctx,
            layout,
            subgraph_title_y_shift,
        },
        &hierarchy_plan,
    )?;
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
        &hierarchy_plan,
    )?;

    let document = prepare_flowchart_svg_document(FlowchartSvgDocumentRequest {
        family_id: if swimlane_layout.is_some() {
            crate::DiagramFamilyId::SWIMLANE
        } else {
            crate::DiagramFamilyId::FLOWCHART
        },
        diagram_id,
        diagram_type,
        model,
        document_ids: &document_ids,
        use_max_width,
        diagram_padding,
        bbox_min_x,
        bbox_min_y,
        bbox_max_x,
        bbox_max_y,
    });

    drop(_g_viewbox);
    let _g_render_svg = render_timing.section(&mut timings.render_svg);

    let mut out = BoundedSvgOutput::new(options.work_meter());

    if let Some(base_typography) = svg_label_sidecar
        .base_typography()
        .filter(|plan| plan.requires_terminal_evidence())
    {
        base_typography.begin_terminal_emission();
    }

    let root_document = document.push_root_open(&mut out)?;
    document.push_accessibility_metadata(&mut out);
    out.push_str("<style>");
    out.checkpoint()?;
    write_flowchart_css(
        &mut out,
        diagram_id,
        document_ids.drop_shadow(),
        document_ids.drop_shadow_small(),
        effective_config_value,
        &font_family,
        font_size,
        &model.class_defs,
        Some(&title_paint),
    )?;
    if swimlane_layout.is_some() {
        super::swimlane::write_swimlane_css(&mut out, diagram_id);
    }
    out.push_str("</style>");
    out.checkpoint()?;
    if let Some(base_typography) = svg_label_sidecar
        .base_typography()
        .filter(|plan| plan.requires_terminal_evidence())
    {
        base_typography.record_stylesheet_emission(&font_family, font_size);
    }

    let defs = prepare_flowchart_defs(
        document_ids.marker_scope(),
        diagram_type,
        &ctx,
        &marker_plan,
    );

    let mut root_session = FlowchartRootRenderSession {
        timing: render_timing,
        details: &mut detail,
        edge_cache: &mut edge_path_cache,
        hierarchy_plan: &hierarchy_plan,
        cluster_theme_plan: &cluster_theme_plan,
        marker_plan: &marker_plan,
    };
    if layout.uses_elk_adapter_dom {
        out.push_str("<g>");
        defs.push_base_markers(&mut out)?;
        out.checkpoint()?;
        defs.push_extra_markers(&mut out)?;
        out.checkpoint()?;
        render_flowchart_elk_root_groups(&mut out, &ctx, &mut root_session)?;
        out.push_str("</g>");
        // The shadow filters are siblings of Mermaid's marker/root wrapper,
        // rather than children of the wrapper that owns the painted graph.
        push_flowchart_shadow_defs(&mut out, &document_ids, effective_config_value);
        out.checkpoint()?;
    } else {
        push_flowchart_shadow_defs(&mut out, &document_ids, effective_config_value);
        out.checkpoint()?;
        out.push_str("<g>");
        defs.push_base_markers(&mut out)?;
        render_flowchart_root(&mut out, &ctx, None, 0.0, 0.0, &mut root_session)?;

        defs.push_extra_markers(&mut out)?;
        out.push_str("</g>");
        out.checkpoint()?;
    }
    push_flowchart_gradient(&mut out, &document_ids, effective_config_value);
    out.checkpoint()?;
    if let Some(title) = diagram_title.as_deref() {
        let title_x = title_anchor_x;
        let title_y = -title_top_margin;
        let _ = write!(
            &mut out,
            r#"<text text-anchor="middle" x="{}" y="{}" class="flowchartTitleText">{}</text>"#,
            fmt(title_x),
            fmt(title_y),
            escape_xml_display(title)
        );
        if !title.trim().is_empty() {
            ctx.record_base_typography_label_emission(
                crate::flowchart::FlowchartBaseTypographyLabelEmission::diagram_title(),
            );
        }
    }
    out.push_str("</svg>\n");
    out.checkpoint()?;

    drop(_g_render_svg);
    timings.total = total_timer
        .map(merman_core::runtime::OperationTimer::elapsed)
        .unwrap_or_default();
    if render_timing.is_enabled() {
        eprintln!(
            "[render-timing] diagram=flowchart-v2 total={:?} deserialize={:?} build_ctx={:?} viewbox={:?} viewbox_edge_curve_bounds={:?} viewbox_edge_root_lookup={:?} viewbox_edge_curve_offsets={:?} viewbox_edge_curve_geom={:?} viewbox_edge_curve_bbox_union={:?} viewbox_edge_curve_geom_calls={} viewbox_edge_curve_geom_skipped_bounds={} render_svg={:?} finalize={:?} root_calls={} clusters={:?} edges_select={:?} edge_paths={:?} edge_labels={:?} dom_order={:?} nodes={:?} node_style_compile={:?} node_roughjs={:?} node_roughjs_calls={} node_label_html={:?} node_label_html_calls={} nested_roots={:?}",
            timings.total,
            timings.deserialize_model,
            timings.build_ctx,
            timings.viewbox,
            viewbox_edge_curve_bounds,
            detail.viewbox_edge_root_lookup,
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
    let rooted = root_document.complete(out.finish()?)?;
    if title_paint.requested() {
        theme_evidence.record_title_evidence(title_paint.finish_evidence());
    }
    Ok(rooted)
}

fn flowchart_node_theme_ordinals<'a>(
    nodes: &'a [crate::flowchart::FlowNode],
    subgraphs: &'a [crate::flowchart::FlowSubgraph],
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
    // cluster. Dagre renders only empty subgraphs through the ordinary Node surface.
    for node in nodes {
        if subgraph_ids.contains(node.id.as_str()) {
            continue;
        }
        if let std::collections::hash_map::Entry::Vacant(entry) = ordinals.entry(node.id.as_str()) {
            entry.insert(next_ordinal);
            next_ordinal += 1;
        }
    }

    if !uses_elk_adapter_dom {
        for subgraph in subgraphs
            .iter()
            .filter(|subgraph| subgraph.nodes.is_empty())
        {
            if let std::collections::hash_map::Entry::Vacant(entry) =
                ordinals.entry(subgraph.id.as_str())
            {
                entry.insert(next_ordinal);
                next_ordinal += 1;
            }
        }
    }

    ordinals
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str) -> crate::flowchart::FlowNode {
        crate::flowchart::FlowNode {
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

    fn subgraph(id: &str, nodes: &[&str]) -> crate::flowchart::FlowSubgraph {
        crate::flowchart::FlowSubgraph {
            id: id.to_string(),
            title: id.to_string(),
            dir: None,
            has_explicit_dir: false,
            label_type: None,
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: nodes.iter().map(|node| (*node).to_string()).collect(),
        }
    }

    #[test]
    fn node_theme_ordinals_follow_actual_node_emitter_surfaces() {
        let nodes = vec![node("A"), node("group"), node("A"), node("empty")];
        let subgraphs = vec![subgraph("group", &["A"]), subgraph("empty", &[])];

        let dagre = flowchart_node_theme_ordinals(&nodes, &subgraphs, false);
        assert_eq!(dagre.len(), 2);
        assert_eq!(dagre.get("A"), Some(&1));
        assert_eq!(dagre.get("empty"), Some(&2));
        assert!(!dagre.contains_key("group"));

        let elk = flowchart_node_theme_ordinals(&nodes, &subgraphs, true);
        assert_eq!(elk.len(), 1);
        assert_eq!(elk.get("A"), Some(&1));
        assert!(!elk.contains_key("group"));
        assert!(!elk.contains_key("empty"));
    }
}

fn push_flowchart_shadow_defs(
    out: &mut impl crate::svg::parity::SvgOutput,
    document_ids: &FlowchartDocumentIds<'_>,
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
        r#"<defs><filter id="{}" height="130%" width="130%"><feDropShadow dx="4" dy="4" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs><defs><filter id="{}" height="150%" width="150%"><feDropShadow dx="2" dy="2" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs>"#,
        document_ids.drop_shadow(),
        flood_color,
        document_ids.drop_shadow_small(),
        flood_color
    );
}

fn push_flowchart_gradient(
    out: &mut impl crate::svg::parity::SvgOutput,
    document_ids: &FlowchartDocumentIds<'_>,
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
        r#"<linearGradient id="{}" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%"><stop offset="0%" stop-color="{}" stop-opacity="1"/><stop offset="100%" stop-color="{}" stop-opacity="1"/></linearGradient>"#,
        document_ids.root_gradient(),
        gradient_start.as_str(),
        gradient_stop.as_str()
    );
}

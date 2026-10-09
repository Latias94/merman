use super::defs::{FlowchartMarkerEmissionPlan, prepare_flowchart_defs};
use super::document::{FlowchartSvgDocumentRequest, prepare_flowchart_svg_document};
use super::document_ids::{FlowchartDocumentIdRequest, FlowchartDocumentIds};
use super::render_config::FlowchartRenderConfig;
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
    edge_paint_geometry: Option<&mut Vec<crate::model::EdgePaintGeometry>>,
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
            effect_evidence: artifact.effect_evidence(),
            expected_effect_applications: artifact.expected_effect_applications(),
            edge_style_plan: artifact.edge_style_plan(),
            edge_theme: artifact.edge_theme(),
            prepared_theme: artifact.prepared_theme(),
            prepared_nodes: artifact.prepared_nodes(),
            render_config: artifact.render_config(),
        },
        options,
        edge_paint_geometry,
    )
}

pub(super) struct FlowchartSvgModelRequest<'a> {
    pub(super) render_config: &'a FlowchartRenderConfig,
    pub(super) prepared_nodes: &'a super::node_inventory::FlowchartPreparedNodes,
    pub(super) prepared_theme: &'a crate::flowchart::FlowchartPreparedTheme,
    pub(super) layout: &'a FlowchartLayout,
    pub(super) swimlane_layout: Option<&'a crate::model::SwimlaneLayout>,
    pub(super) model: &'a crate::flowchart::FlowchartModel,
    pub(super) render_context: &'a crate::flowchart::FlowchartRenderContext,
    pub(super) effective_config: &'a merman_core::MermaidConfig,
    pub(super) diagram_type: &'a str,
    pub(super) diagram_title: Option<&'a str>,
    pub(super) svg_label_sidecar: &'a crate::flowchart::FlowchartSvgLabelSidecar,
    pub(super) effect_evidence: &'a crate::diagram_theme::SvgShadowEvidenceRecorder,
    pub(super) expected_effect_applications: &'a std::cell::Cell<usize>,
    pub(super) theme_evidence: &'a crate::flowchart::FlowchartThemeEvidenceRecorder,
    pub(super) edge_style_plan: &'a FlowchartEdgeStylePlan,
    pub(super) edge_theme: &'a crate::flowchart::FlowchartEdgeThemeStyle,
}

pub(super) fn render_flowchart_svg_model(
    request: FlowchartSvgModelRequest<'_>,
    options: &SvgExecution<'_>,
    edge_paint_geometry: Option<&mut Vec<crate::model::EdgePaintGeometry>>,
) -> Result<root_svg::RootedSvg> {
    let FlowchartSvgModelRequest {
        render_config,
        prepared_nodes,
        prepared_theme,
        layout,
        swimlane_layout,
        model,
        render_context,
        effective_config,
        diagram_type,
        diagram_title,
        svg_label_sidecar,
        theme_evidence,
        effect_evidence,
        expected_effect_applications,
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
        prepared_theme
            .compatibility
            .hand_drawn_seed
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

    let font_family = &render_config.font_family;
    let font_size = render_config.font_size;
    let wrapping_width = render_config.wrapping_width;
    let node_html_labels = render_config.node_html_labels;
    let edge_html_labels = render_config.edge_html_labels;
    let swimlane_title_html_labels = render_config.swimlane_title_html_labels;
    let node_wrap_mode = render_config.node_wrap_mode;
    let edge_wrap_mode = render_config.edge_wrap_mode;
    let diagram_padding = render_config.diagram_padding;
    let use_max_width = render_config.use_max_width;
    let title_top_margin = render_config.title_top_margin;
    let node_padding = render_config.node_padding;
    let text_style = &render_config.text_style;
    let html_label_text_style = &render_config.html_label_text_style;
    let default_edge_interpolate = &render_config.default_edge_interpolate;
    let default_edge_style = &render_config.default_edge_style;
    let node_border_color = &render_config.node_border_color;
    let node_stroke_width = render_config.node_stroke_width;
    let node_typography_config_ownership = render_config.node_typography_config_ownership;
    let node_label_fill_config_override = render_config.node_label_fill_config_override;
    let node_border_config_override = render_config.node_border_config_override;
    let node_fill_config_override = render_config.node_fill_config_override;
    let node_stroke_width_config_override = render_config.node_stroke_width_config_override;
    let edge_stroke_config_override = render_config.edge_stroke_config_override;
    let node_corner_radius_config_override = render_config.node_corner_radius_config_override;
    let edge_corner_radius = render_config.edge_corner_radius;
    let edge_label_padding = render_config.edge_label_padding;
    let compact_edge_corners = render_config.compact_edge_corners;

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

    let mut node_dom_index = flowchart_node_dom_indices(model);
    for node in &model.nodes {
        if let Some(index) = render_context.node_dom_index(&node.id) {
            node_dom_index.insert(node.id.as_str(), index);
        }
    }
    let document_ids = FlowchartDocumentIds::prepare(FlowchartDocumentIdRequest {
        diagram_id: diagram_id_value,
        edge_order: &edge_order,
        node_dom_index: &node_dom_index,
        subgraph_index_by_id: &subgraph_index_by_id,
        layout_nodes: &layout.nodes,
        swimlane_nodes: swimlane_layout.map(|layout| layout.nodes.as_slice()),
        swimlane_lanes: swimlane_layout.map(|layout| layout.lanes.as_slice()),
    });
    let flowchart_edge_trace = options.debug.flowchart_edge_trace();
    let checkpoint_emit = || options.checkpoint_emit();
    let text_surface_paint = &prepared_theme.text_surface;
    let ctx = FlowchartRenderCtx {
        prepared_nodes,
        edges_by_key: edge_order
            .iter()
            .map(|edge| (edge.key, edge.edge))
            .collect(),
        label_effects: std::cell::OnceCell::new(),
        node_effects: std::cell::OnceCell::new(),
        edge_effects: std::cell::OnceCell::new(),
        effect_evidence,
        text_surface_paint,
        model,
        diagram_id,
        diagram_type,
        tx,
        ty,
        measurer,
        config: effective_config,
        compatibility: &prepared_theme.compatibility,
        effect_eligibility: &prepared_theme.effects,
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
        node_stroke_width,
        node_typography_config_ownership,
        node_label_fill_config_override,
        node_border_config_override,
        node_fill_config_override,
        node_stroke_width_config_override,
        edge_stroke_config_override,
        node_corner_radius_config_override,
        edge_corner_radius,
        edge_label_padding,
        compact_edge_corners,
        default_edge_interpolate,
        default_edge_style,
        edge_theme,
        trace_edge_id: flowchart_edge_trace.map(|(edge_id, _)| edge_id),
        trace_collector: flowchart_edge_trace.map(|(_, collector)| collector),
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
        swimlane_edge_label_edges_by_node_id,
        node_dom_index,
        node_padding,
        wrapping_width,
        node_wrap_mode,
        edge_wrap_mode,
        text_style,
        html_label_text_style,
    };

    let hierarchy_plan = FlowchartHierarchyPlan::prepare(&ctx, prepared_nodes.schedule())?;
    text_surface_paint
        .begin_terminal_emission(hierarchy_plan.rendered_cluster_ids(), ctx.work_meter)?;
    let marker_plan = FlowchartMarkerEmissionPlan::prepare(&ctx, &hierarchy_plan)?;
    let theme_resource_policy = options.theme_resource_policy();
    let node_effects =
        node_effect::FlowchartNodeEffects::prepare(&ctx, &hierarchy_plan, &theme_resource_policy)?;
    expected_effect_applications.set(node_effects.expected_applications());
    ctx.node_effects
        .set(node_effects)
        .expect("node effects are prepared once");

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
        source_paint_bounds,
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
            font_family,
            title_top_margin,
            timing: render_timing,
            theme_resource_policy: &theme_resource_policy,
            marker_plan: &marker_plan,
            viewbox_edge_curve_bounds: &mut viewbox_edge_curve_bounds,
            detail: &mut detail,
            edge_path_cache: &mut edge_path_cache,
        },
        &hierarchy_plan,
    )?;

    expected_effect_applications.set(
        expected_effect_applications.get()
            + ctx
                .label_effects
                .get()
                .map_or(0, |effects| effects.expected_applications())
            + ctx
                .edge_effects
                .get()
                .map_or(0, |effects| effects.expected_applications()),
    );

    // `data_points` is written only by `finish_edge_route` inside the viewbox pass and is
    // read-only afterwards, so this is the exact point list the emitted `data-points`
    // attribute carries — before any emit-phase mutation of the cache.
    let clipped_points: Option<
        FxHashMap<crate::flowchart::FlowchartEdgeKey, Vec<crate::model::LayoutPoint>>,
    > = edge_paint_geometry.as_ref().map(|_| {
        let mut map = FxHashMap::default();
        for edge in &render_edges {
            let Some(entry) = edge_path_cache.get(&edge.key) else {
                continue;
            };
            map.insert(
                edge.key,
                entry
                    .geom
                    .data_points
                    .iter()
                    .map(|point| crate::model::LayoutPoint {
                        x: point.x + entry.origin_x - ctx.tx,
                        y: point.y + entry.origin_y - ctx.ty,
                    })
                    .collect(),
            );
        }
        map
    });
    let document = prepare_flowchart_svg_document(FlowchartSvgDocumentRequest {
        family_id: if swimlane_layout.is_some() {
            crate::DiagramFamilyId::SWIMLANE
        } else if diagram_type == "agentflow" {
            crate::DiagramFamilyId::AGENTFLOW
        } else {
            crate::DiagramFamilyId::FLOWCHART
        },
        diagram_id,
        diagram_type,
        model,
        document_ids: &document_ids,
        use_max_width,
        diagram_padding,
        source_paint_bounds,
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
        diagram_type,
        document_ids.drop_shadow(),
        document_ids.drop_shadow_small(),
        document_ids.root_gradient(),
        &prepared_theme.compatibility,
        font_family,
        font_size,
        &model.class_defs,
        Some(text_surface_paint),
    )?;
    if diagram_type == "agentflow" {
        super::agentflow::write_css(&mut out, diagram_id, &prepared_theme.compatibility)?;
    }
    text_surface_paint.generic_text.record_stylesheet_emission();
    text_surface_paint.background.record_stylesheet();
    if swimlane_layout.is_some() {
        super::swimlane::write_swimlane_css(&mut out, diagram_id);
    }
    out.push_str("</style>");
    out.checkpoint()?;
    if let Some(base_typography) = svg_label_sidecar
        .base_typography()
        .filter(|plan| plan.requires_terminal_evidence())
    {
        base_typography.record_stylesheet_emission(font_family, font_size);
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
        marker_plan: &marker_plan,
        edge_label_positions: edge_paint_geometry.as_ref().map(|_| FxHashMap::default()),
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
        push_flowchart_shadow_defs(
            &mut out,
            &document_ids,
            &prepared_theme.compatibility.look_defs,
        );
        out.checkpoint()?;
    } else {
        push_flowchart_shadow_defs(
            &mut out,
            &document_ids,
            &prepared_theme.compatibility.look_defs,
        );
        out.checkpoint()?;
        out.push_str("<g>");
        defs.push_base_markers(&mut out)?;
        render_flowchart_root(&mut out, &ctx, None, 0.0, 0.0, &mut root_session)?;

        defs.push_extra_markers(&mut out)?;
        out.push_str("</g>");
        out.checkpoint()?;
    }
    push_flowchart_gradient(
        &mut out,
        &document_ids,
        &prepared_theme.compatibility.look_defs,
    );
    out.checkpoint()?;
    if let Some(title) = diagram_title.as_deref() {
        let title_x = title_anchor_x;
        let title_y = -title_top_margin;
        let _ = write!(
            &mut out,
            r#"<text text-anchor="middle" x="{}" y="{}" class="{}">{}</text>"#,
            fmt(title_x),
            fmt(title_y),
            title_css_class(diagram_type),
            escape_xml(title)
        );
        text_surface_paint.generic_text.record_label(
            crate::flowchart::FlowchartTextPaintChannel::DiagramTitle,
            crate::flowchart::FlowchartTextPaintFacts::from_visible(
                &crate::text::VisibleTextStyleFacts::plain_text(title),
            ),
            crate::flowchart::FlowchartSourceFacetStatus::Absent,
            ctx.work_meter,
        )?;
        if !title.trim().is_empty() {
            ctx.record_base_typography_label_emission(
                crate::flowchart::FlowchartBaseTypographyLabelEmission::diagram_title(),
            );
        }
    }
    out.push_str("</svg>\n");
    out.checkpoint()?;

    let edge_label_positions = root_session.edge_label_positions.take();
    drop(root_session);
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
    if let Some(out) = edge_paint_geometry {
        out.extend(render_edges.iter().map(|edge| {
            crate::model::EdgePaintGeometry {
                id: edge.as_ref().id.clone(),
                points: clipped_points
                    .as_ref()
                    .and_then(|map| map.get(&edge.key).cloned()),
                label_position: edge_label_positions
                    .as_ref()
                    .and_then(|map| map.get(&edge.key).cloned()),
            }
        }));
    }
    if let Some(evidence) = marker_plan.finish_theme_evidence(ctx.resolved_theme, ctx.work_meter)? {
        theme_evidence.record_marker_evidence(evidence);
    }
    if text_surface_paint.requested() {
        theme_evidence.record_title_evidence(text_surface_paint.finish_evidence());
    }
    Ok(rooted)
}

pub(super) fn flowchart_node_theme_ordinals<'a>(
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
            metadata: Default::default(),
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
    prepared: &crate::svg::PreparedLookDefs,
) {
    let flood_color = prepared.flood_color();
    let offset = super::super::look_defs::NEO_SHADOW_OFFSET_PX;
    let _ = write!(
        out,
        r#"<defs><filter id="{}" height="130%" width="130%"><feDropShadow dx="{offset}" dy="{offset}" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs><defs><filter id="{}" height="150%" width="150%"><feDropShadow dx="2" dy="2" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs>"#,
        document_ids.drop_shadow(),
        flood_color,
        document_ids.drop_shadow_small(),
        flood_color
    );
}

fn push_flowchart_gradient(
    out: &mut impl crate::svg::parity::SvgOutput,
    document_ids: &FlowchartDocumentIds<'_>,
    prepared: &crate::svg::PreparedLookDefs,
) {
    let Some((gradient_start, gradient_stop)) = prepared.gradient() else {
        return;
    };

    let gradient_start = escape_xml(gradient_start);
    let gradient_stop = escape_xml(gradient_stop);
    let _ = write!(
        out,
        r#"<linearGradient id="{}" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%"><stop offset="0%" stop-color="{}" stop-opacity="1"/><stop offset="100%" stop-color="{}" stop-opacity="1"/></linearGradient>"#,
        document_ids.root_gradient(),
        gradient_start.as_str(),
        gradient_stop.as_str()
    );
}
#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::environment::RenderEnvironment;
    use crate::model::{FlowchartLayout, LayoutNode};
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};
    use crate::svg::{SvgDebugOptions, SvgRenderOptions};
    use merman_core::{Engine, ParseOptions, RenderSemanticModel};

    #[test]
    fn diagram_id_terminal_precedes_later_flowchart_node_emission_error() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart TD\nA@{ img: \"https://example.invalid/a.svg\", label: \"A\" }\n",
                ParseOptions::strict(),
            )
            .expect("parse succeeds")
            .expect("detects Flowchart");
        let render_context = parsed
            .flowchart_render_context()
            .expect("Flowchart render context")
            .clone();
        let (metadata, semantic) = parsed.into_parts();
        let RenderSemanticModel::Flowchart(mut model) = semantic else {
            panic!("expected Flowchart model");
        };
        let node = model.nodes.first_mut().expect("fixture node");
        assert_eq!(node.layout_shape.as_deref(), Some("imageSquare"));
        node.img = None;

        let layout = FlowchartLayout {
            nodes: vec![LayoutNode {
                id: node.id.clone(),
                x: 40.0,
                y: 30.0,
                width: 80.0,
                height: 60.0,
                is_cluster: false,
                label_width: Some(10.0),
                label_height: Some(10.0),
            }],
            edges: Vec::new(),
            edge_owners: Default::default(),
            clusters: Vec::new(),
            bounds: None,
            dom_node_order_by_root: std::collections::HashMap::from([(
                String::new(),
                vec![node.id.clone()],
            )]),
            uses_elk_adapter_dom: false,
        };
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, 1)
            .expect("valid SVG byte limit");
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session()
            .expect("begin render session");
        let request = SvgRenderOptions {
            diagram_id: Some("terminal".to_string()),
            ..SvgRenderOptions::default()
        };
        let debug = SvgDebugOptions::default();
        let execution = SvgExecution::unthemed_for_test(
            &request,
            &debug,
            &session,
            crate::DiagramFamilyId::FLOWCHART,
        )
        .expect("SVG execution");
        let sidecar = crate::flowchart::FlowchartSvgLabelSidecar::default();
        let prepared_theme = crate::flowchart::FlowchartPreparedTheme::resolve(
            None,
            &metadata.effective_config,
            super::render_config::flowchart_node_label_fill_config_override(
                &metadata.effective_config,
            ),
            execution.work_meter(),
        )
        .expect("prepared theme");
        let prepared_nodes = super::node_inventory::FlowchartPreparedNodes::prepare(
            &model,
            &render_context,
            super::node_inventory::FlowchartNodeLayoutView::Flowchart(&layout),
            &sidecar,
            None,
            &metadata.effective_config,
            &prepared_theme,
            &super::render_config::prepare_flowchart_render_config(
                &model,
                &metadata.effective_config,
                &prepared_theme.compatibility,
                layout.uses_elk_adapter_dom,
                sidecar.base_typography(),
                sidecar.edge_label_padding(),
            ),
            session.work_meter(),
        )
        .expect("prepared nodes");

        let error = render_flowchart_svg_model(
            FlowchartSvgModelRequest {
                render_config: &super::render_config::prepare_flowchart_render_config(
                    &model,
                    &metadata.effective_config,
                    &prepared_theme.compatibility,
                    layout.uses_elk_adapter_dom,
                    sidecar.base_typography(),
                    sidecar.edge_label_padding(),
                ),
                prepared_theme: &prepared_theme,
                prepared_nodes: &prepared_nodes,
                layout: &layout,
                swimlane_layout: None,
                model: &model,
                render_context: &render_context,
                effective_config: &metadata.effective_config,
                diagram_type: metadata.diagram_type.as_str(),
                diagram_title: metadata.title.as_deref(),
                theme_evidence: &Default::default(),
                effect_evidence: &Default::default(),
                expected_effect_applications: &Default::default(),
                edge_theme: &Default::default(),
                edge_style_plan: &FlowchartEdgeStylePlan::prepare_for_model(
                    &model,
                    &metadata.effective_config,
                    false,
                    execution.work_meter(),
                )
                .expect("edge style plan")
                .with_resolved_stroke_widths(
                    &model,
                    &Default::default(),
                    &metadata.effective_config,
                    &prepared_theme.compatibility,
                    execution.work_meter(),
                )
                .expect("prepared stroke widths"),
                svg_label_sidecar: &sidecar,
            },
            &execution,
            None,
        )
        .expect_err("diagram-ID rejection must stop before the invalid image node is emitted");

        let crate::Error::ResourceLimitExceeded(details) = error else {
            panic!("expected SVG byte rejection, got {error}");
        };
        assert_eq!(details.limit, ResourceLimitId::MaxSvgBytes.as_str());
    }

    #[test]
    fn elk_terminal_straightening_reaches_svg_and_preserves_ports() {
        use crate::model::{LayoutEdge, LayoutPoint};
        use base64::Engine as _;

        // Pinned Mermaid geometry.spec.ts terminal-jog case, with measured rectangles whose
        // boundaries coincide with its ports. The layout is fixed to isolate SVG postprocessing.
        let render = |enabled: bool, trace: bool| {
            let parsed = Engine::new()
                .parse_diagram_for_render_model_sync(
                    &format!("---\nconfig:\n  elk:\n    straightenEdges: {enabled}\n---\nflowchart LR\nA --> B"),
                    ParseOptions::strict(),
                ).unwrap().unwrap();
            let render_context = parsed.flowchart_render_context().unwrap().clone();
            let (metadata, semantic) = parsed.into_parts();
            let RenderSemanticModel::Flowchart(model) = semantic else {
                panic!("Flowchart");
            };
            let edge_id = model.edges[0].id.clone();
            let layout = FlowchartLayout {
                nodes: [("A", 153.0, 116.25), ("B", 400.0, 320.0)]
                    .into_iter()
                    .map(|(id, x, y)| LayoutNode {
                        id: id.into(),
                        x,
                        y,
                        width: 80.0,
                        height: 40.0,
                        is_cluster: false,
                        label_width: Some(10.0),
                        label_height: Some(10.0),
                    })
                    .collect(),
                edges: vec![LayoutEdge {
                    id: edge_id.clone(),
                    from: "A".into(),
                    to: "B".into(),
                    from_cluster: None,
                    to_cluster: None,
                    points: [
                        (193.0, 116.25),
                        (218.0, 116.25),
                        (218.0, 119.5),
                        (400.0, 119.5),
                        (400.0, 300.0),
                    ]
                    .into_iter()
                    .map(|(x, y)| LayoutPoint { x, y })
                    .collect(),
                    label: None,
                    start_label_left: None,
                    start_label_right: None,
                    end_label_left: None,
                    end_label_right: None,
                    start_marker: None,
                    end_marker: None,
                    stroke_dasharray: None,
                }],
                edge_owners: crate::flowchart::FlowchartEdgeOwners::from_semantic_indices([0]),
                clusters: Vec::new(),
                bounds: None,
                dom_node_order_by_root: std::collections::HashMap::from([(
                    String::new(),
                    vec!["A".into(), "B".into()],
                )]),
                uses_elk_adapter_dom: true,
            };
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let request = SvgRenderOptions {
                diagram_id: Some("terminal-jog".into()),
                ..SvgRenderOptions::default()
            };
            let debug = if trace {
                SvgDebugOptions::default().with_flowchart_edge_trace(
                    edge_id,
                    crate::svg::FlowchartEdgeTraceCollector::default(),
                )
            } else {
                SvgDebugOptions::default()
            };
            let execution = SvgExecution::unthemed_for_test(
                &request,
                &debug,
                &session,
                crate::DiagramFamilyId::FLOWCHART,
            )
            .unwrap();
            let sidecar = crate::flowchart::FlowchartSvgLabelSidecar::default();
            let prepared_theme = crate::flowchart::FlowchartPreparedTheme::resolve(
                None,
                &metadata.effective_config,
                super::render_config::flowchart_node_label_fill_config_override(
                    &metadata.effective_config,
                ),
                execution.work_meter(),
            )
            .expect("prepared theme");
            let prepared_nodes = super::node_inventory::FlowchartPreparedNodes::prepare(
                &model,
                &render_context,
                super::node_inventory::FlowchartNodeLayoutView::Flowchart(&layout),
                &sidecar,
                None,
                &metadata.effective_config,
                &prepared_theme,
                &super::render_config::prepare_flowchart_render_config(
                    &model,
                    &metadata.effective_config,
                    &prepared_theme.compatibility,
                    layout.uses_elk_adapter_dom,
                    sidecar.base_typography(),
                    sidecar.edge_label_padding(),
                ),
                session.work_meter(),
            )
            .expect("prepared nodes");
            render_flowchart_svg_model(
                FlowchartSvgModelRequest {
                    render_config: &super::render_config::prepare_flowchart_render_config(
                        &model,
                        &metadata.effective_config,
                        &prepared_theme.compatibility,
                        layout.uses_elk_adapter_dom,
                        sidecar.base_typography(),
                        sidecar.edge_label_padding(),
                    ),
                    prepared_theme: &prepared_theme,
                    prepared_nodes: &prepared_nodes,
                    layout: &layout,
                    swimlane_layout: None,
                    model: &model,
                    render_context: &render_context,
                    effective_config: &metadata.effective_config,
                    diagram_type: metadata.diagram_type.as_str(),
                    diagram_title: None,
                    theme_evidence: &Default::default(),
                    effect_evidence: &Default::default(),
                    expected_effect_applications: &Default::default(),
                    edge_theme: &Default::default(),
                    edge_style_plan: &FlowchartEdgeStylePlan::prepare_for_model(
                        &model,
                        &metadata.effective_config,
                        false,
                        execution.work_meter(),
                    )
                    .expect("edge style plan")
                    .with_resolved_stroke_widths(
                        &model,
                        &Default::default(),
                        &metadata.effective_config,
                        &prepared_theme.compatibility,
                        execution.work_meter(),
                    )
                    .expect("prepared stroke widths"),
                    svg_label_sidecar: &sidecar,
                },
                &execution,
                None,
            )
            .unwrap()
            .to_string()
        };
        let route = |svg: &str| {
            let doc = roxmltree::Document::parse(svg).unwrap();
            let path = doc
                .descendants()
                .find(|n| n.attribute("data-points").is_some())
                .unwrap();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(path.attribute("data-points").unwrap())
                .unwrap();
            let points: Vec<LayoutPoint> = serde_json::from_slice(&bytes).unwrap();
            (
                path.attribute("d").unwrap().to_owned(),
                points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(),
            )
        };
        let off = render(false, false);
        let on = render(true, false);
        let (old_path, old_points) = route(&off);
        let (new_path, new_points) = route(&on);
        assert_eq!(old_points.len(), 5);
        assert_eq!(new_points.len(), 3);
        assert_eq!(old_points.first(), new_points.first());
        assert_eq!(old_points.last(), new_points.last());
        assert_eq!(new_points[0].1, new_points[1].1);
        assert_ne!(old_path, new_path);
        assert_eq!(
            on,
            render(true, true),
            "diagnostics must preserve processed geometry"
        );
    }
}

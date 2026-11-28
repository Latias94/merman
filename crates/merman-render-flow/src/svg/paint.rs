use super::render_input::{
    FlowchartRenderEdge, FlowchartRenderEdgeRef, FlowchartRenderInputs,
    prepare_flowchart_render_inputs,
};
use super::svg_emit::FlowchartSvgModelRequest;
use super::*;
use merman_model::OperationPhase;

#[cfg(feature = "canonical-edge-paint")]
use merman_render_core::rough_svg::constrained_rough_edge_path;

#[cfg(feature = "layout-shoal")]
use super::swimlane::label_seating as swimlane_label_seating;

pub fn with_flowchart_paint_context<T>(
    request: FlowchartSvgModelRequest<'_>,
    services: &FlowchartSvgServices<'_>,
    consume: impl FnOnce(
        &FlowchartRenderCtx<'_>,
        &[FlowchartRenderEdge<'_>],
        &FlowchartHierarchyPlan<'_>,
    ) -> Result<T>,
) -> Result<T> {
    with_flowchart_paint_context_in_phase(request, services, OperationPhase::Layout, consume)
}

pub(super) fn with_flowchart_paint_context_in_phase<T>(
    request: FlowchartSvgModelRequest<'_>,
    services: &FlowchartSvgServices<'_>,
    work_phase: OperationPhase,
    consume: impl FnOnce(
        &FlowchartRenderCtx<'_>,
        &[FlowchartRenderEdge<'_>],
        &FlowchartHierarchyPlan<'_>,
    ) -> Result<T>,
) -> Result<T> {
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
        diagram_title: _,
        svg_label_sidecar,
        edge_style_plan,
    } = request;
    let mut render_model = crate::projection::FlowchartRenderModelRef::new(model, render_context);
    if layout.route_contract.is_canonical_axis() {
        render_model = render_model.with_shoal_svg_text(
            render_config.node_html_labels,
            render_config.edge_html_labels,
        );
    }
    let model = &render_model;
    if model.nodes.iter().any(|node| {
        node.layout_shape.as_deref() == Some("ellipse")
            && model.collapsed_replacement(&node.id).is_none()
            && !model.is_subgraph_collapsed(&node.id)
    }) {
        return Err(merman_render_core::Error::InvalidModel {
            message: "No such shape: ellipse. Please check your syntax.".to_string(),
        });
    }
    #[cfg(feature = "layout-shoal")]
    let svg_edges = layout.route_contract.flowchart_svg_edges();
    let validate_layout_edges = || layout.edge_owners.validate(layout.edges, &model.edges);
    #[cfg(feature = "layout-shoal")]
    let owner_occurrences = match svg_edges {
        Some(edges) => layout
            .edge_owners
            .validate_projected(edges.len(), &model.edges)?,
        None => validate_layout_edges()?,
    };
    #[cfg(not(feature = "layout-shoal"))]
    let owner_occurrences = validate_layout_edges()?;
    #[cfg(feature = "layout-layered")]
    if let FlowchartSvgRoute::Layered(layered) = layout.route_contract
        && layered.edges.len() != layout.edge_owners.len()
    {
        return Err(Error::InvalidModel {
            message: "Flowchart Layered routes do not match the prepared edge owners".to_owned(),
        });
    }

    let hand_drawn_seed = merman_render_core::rough_svg::operation_randomness(
        services.operation,
        prepared_theme
            .compatibility
            .hand_drawn_seed
            .unwrap_or(services.render_seed as f64),
        "render.flowchart.roughjs",
    );

    let diagram_id = services.diagram_id;

    let FlowchartRenderInputs {
        mut render_edges,
        extra_nodes,
    } = prepare_flowchart_render_inputs(
        model,
        render_context,
        layout.edge_owners,
        owner_occurrences,
        layout.route_contract.uses_projected_edges(),
    );
    if let Some(swimlane_layout) = swimlane_layout {
        super::swimlane::apply_swimlane_edge_curves(&mut render_edges, swimlane_layout);
    }

    let wrapping_width = render_config.wrapping_width;
    let node_html_labels = render_config.node_html_labels;
    let edge_html_labels = render_config.edge_html_labels;
    let swimlane_title_html_labels = render_config.swimlane_title_html_labels;
    let node_wrap_mode = render_config.node_wrap_mode;
    let edge_wrap_mode = render_config.edge_wrap_mode;
    let node_padding = render_config.node_padding;
    let text_style = &render_config.text_style;
    let html_label_text_style = &render_config.html_label_text_style;
    let default_edge_interpolate = &render_config.default_edge_interpolate;
    let default_edge_style = &render_config.default_edge_style;
    let node_border_color = &render_config.node_border_color;
    let node_stroke_width = render_config.node_stroke_width;
    let edge_corner_radius = render_config.edge_corner_radius;

    let mut nodes_by_id: FxHashMap<&str, &merman_model::families::flowchart::FlowNode> =
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
    let edge_order: Vec<FlowchartRenderEdgeRef<'_>> =
        render_edges.iter().map(|edge| edge.as_ref()).collect();
    let mut edges_by_key =
        FxHashMap::with_capacity_and_hasher(edge_order.len(), Default::default());
    for edge in &edge_order {
        if edges_by_key.insert(edge.key, edge.edge).is_some() {
            return Err(merman_render_core::Error::InvalidModel {
                message: format!("duplicate Flowchart render edge occurrence {:?}", edge.key),
            });
        }
    }
    if edges_by_key.len() != layout.edge_owners.len()
        || layout
            .edge_owners
            .iter()
            .any(|key| !edges_by_key.contains_key(&key))
    {
        return Err(merman_render_core::Error::InvalidModel {
            message: "Flowchart render occurrences do not match the prepared layout".to_string(),
        });
    }

    let swimlane_direction = swimlane_layout.map(|layout| layout.direction);
    let swimlane_lanes_by_id: FxHashMap<&str, &merman_render_core::SwimlaneLaneLayout> =
        swimlane_layout
            .into_iter()
            .flat_map(|layout| layout.lanes.iter())
            .map(|lane| (lane.id.as_str(), lane))
            .collect();
    let swimlane_edge_label_edges_by_node_id: FxHashMap<&str, FlowchartRenderEdgeRef<'_>> =
        swimlane_layout
            .into_iter()
            .flat_map(|layout| layout.edges.iter().zip(layout.edge_owners.iter()))
            .filter_map(|(layout_edge, owner)| {
                let label_node_id = layout_edge.label_node_id.as_deref()?;
                let edge = edges_by_key.get(&owner).copied()?;
                Some((label_node_id, FlowchartRenderEdgeRef { key: owner, edge }))
            })
            .collect();
    let mut subgraphs_by_id: FxHashMap<&str, &merman_model::families::flowchart::FlowSubgraph> =
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

    let mut parent: FxHashMap<&str, &str> = model.visible_memberships().collect();
    apply_solved_group_parents(
        layout.clusters,
        layout.route_contract.group_parents(),
        &mut parent,
        &mut subgraph_ids_with_children,
    );
    if let Some(swimlane) = swimlane_layout.filter(|layout| layout.has_canonical_routes) {
        for node in swimlane.nodes {
            if let Some(owner) = &node.parent_id {
                parent.insert(&node.id, owner);
            }
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
    for n in layout.nodes {
        layout_nodes_by_id.insert(n.id.as_str(), n);
    }

    let mut layout_edges_by_key =
        FxHashMap::with_capacity_and_hasher(layout.edge_owners.len(), Default::default());
    layout_edges_by_key.extend(FlowchartLayoutEdgeRef::in_layout_order(layout));

    let mut layout_clusters_by_id: FxHashMap<&str, &LayoutCluster> =
        FxHashMap::with_capacity_and_hasher(layout.clusters.len(), Default::default());
    for c in layout.clusters {
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
    let flowchart_edge_trace = services.edge_trace;
    let mut palette = super::agentflow::ContainerPalette::new(effective_config);
    if prepared_theme.native.is_some() {
        palette = palette.without_theme_colors();
    }
    let mut ctx = FlowchartRenderCtx {
        prepared_nodes,
        edges_by_key,
        model,
        diagram_id,
        diagram_type,
        tx,
        ty,
        measurer: services.measurer,
        config: effective_config,
        compatibility: &prepared_theme.compatibility,
        native: prepared_theme.native.as_ref(),
        canvas: render::cluster::resolve_canvas_color(effective_config, services),
        container_label_layers: std::cell::OnceCell::new(),
        palette,
        hand_drawn_seed,
        work_meter: services.work_meter,
        work_phase,
        math_renderer: services.math_renderer,
        svg_label_sidecar: Some(svg_label_sidecar),
        icon_registry: services.icon_registry,
        security_level_loose: effective_config.get_str("securityLevel") == Some("loose"),
        node_html_labels,
        edge_html_labels,
        swimlane_title_html_labels,
        uses_elk_adapter_dom: layout.route_contract.is_elk_parity(),
        class_defs: &model.class_defs,
        edge_style_plan,
        steps: crate::edge_svg::FlowchartSteps::new(model, effective_config),
        document_ids: std::cell::OnceCell::new(),
        layout,
        swimlane_layout,
        node_border_color,
        node_stroke_width,
        edge_corner_radius,
        default_edge_interpolate,
        default_edge_style,
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
        elk_cluster_geometry: FxHashMap::default(),
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

    ctx.elk_cluster_geometry = super::cluster_geometry::prepare_elk_cluster_geometry(&ctx)?;
    let hierarchy_plan = prepare_flowchart_hierarchy(&ctx, prepared_nodes.schedule())?;
    consume(&ctx, &render_edges, &hierarchy_plan)
}

#[inline]
pub(super) fn charged_edge_route_request<'a>(
    ctx: &'a FlowchartRenderCtx<'a>,
    edge: FlowchartRenderEdgeRef<'a>,
    hierarchy: &FlowchartHierarchyPlan<'_>,
) -> Result<(FlowchartEdgePathGeomRequest<'a>, FlowchartRootOffsets)> {
    let root = hierarchy.edge_root(edge.key)?.unwrap_or("");
    let offsets = hierarchy
        .root_offsets(root)
        .unwrap_or(FlowchartRootOffsets {
            origin_x: 0.0,
            origin_y: 0.0,
            abs_top_transform: 0.0,
        });
    let request = FlowchartEdgePathGeomRequest {
        ctx,
        key: edge.key,
        edge: edge.edge,
        origin_x: offsets.origin_x,
        origin_y: offsets.origin_y,
        trace_enabled: false,
    };
    if let Some(layout_edge) = ctx.layout_edges_by_key.get(&edge.key) {
        ctx.work_meter
            .charge_work(layout_edge.points().len().max(2), edge_paint_phase(ctx))?;
    }
    Ok((request, offsets))
}

/// Prepares one painted shaft under the shared per-edge work debit.
///
/// The caller owns the batch debit and final checkpoint. A missing shaft requires compact
/// rendering to refuse the request.
#[cfg(all(
    feature = "diagram-flowchart",
    feature = "layout-dagre",
    any(feature = "native-compact", feature = "test-support")
))]
pub fn prepare_dagre_linear_edge_route(
    ctx: &FlowchartRenderCtx<'_>,
    edge: FlowchartRenderEdgeRef<'_>,
    hierarchy: &FlowchartHierarchyPlan<'_>,
    scratch: &mut FlowchartEdgeDataPointsScratch,
) -> Result<Option<edge_geom::LinearPaintedRoute>> {
    let (request, _) = charged_edge_route_request(ctx, edge, hierarchy)?;
    Ok(edge_geom::dagre_linear_painted_route(request, scratch))
}

/// Prepare the ELK batch through terminal straightening and label separation, before curve
/// formatting and line hops. The caller owns the batch debit and final checkpoint.
pub(super) fn prepare_elk_clipped_routes<'a>(
    ctx: &'a FlowchartRenderCtx<'a>,
    render_edges: &'a [FlowchartRenderEdge<'_>],
    hierarchy: &FlowchartHierarchyPlan<'_>,
    scratch: &mut FlowchartEdgeDataPointsScratch,
) -> Result<
    impl Iterator<
        Item = (
            FlowchartEdgePathGeomRequest<'a>,
            FlowchartRootOffsets,
            edge_geom::ClippedEdgeRoute,
        ),
    > + use<'a>,
> {
    let mut routes = Vec::new();
    let mut routed_edges = Vec::new();
    for edge in render_edges {
        let edge = edge.as_ref();
        let (request, offsets) = charged_edge_route_request(ctx, edge, hierarchy)?;
        if let Some(route) = edge_geom::prepare_edge_route(request, scratch) {
            routes.push(route);
            routed_edges.push((edge, offsets));
        }
    }
    if ctx
        .config
        .get("elk")
        .and_then(|value| value.get("straightenEdges"))
        .and_then(serde_json::Value::as_bool)
        != Some(false)
    {
        edge_geom::straighten_edge_terminals(&mut routes, ctx.work_meter, ctx.work_phase)?;
    }
    edge_geom::separate_edge_labels(
        &mut routes,
        routed_edges.iter().map(|(edge, _)| {
            (
                edge.edge.from.as_str(),
                edge.edge.to.as_str(),
                ctx.model
                    .edge_label_for_occurrence(edge.key, edge.edge)
                    .is_some_and(|label| !label.is_empty()),
            )
        }),
        ctx.work_meter,
        ctx.work_phase,
    )?;
    Ok(routed_edges
        .into_iter()
        .zip(routes)
        .map(move |((edge, offsets), route)| {
            let request = FlowchartEdgePathGeomRequest {
                ctx,
                key: edge.key,
                edge: edge.edge,
                origin_x: offsets.origin_x,
                origin_y: offsets.origin_y,
                trace_enabled: false,
            };
            (request, offsets, route)
        }))
}

pub(super) fn edge_paint_phase(ctx: &FlowchartRenderCtx<'_>) -> OperationPhase {
    match ctx.layout.route_contract {
        #[cfg(feature = "layout-grid")]
        FlowchartSvgRoute::Grid(_) => OperationPhase::Emit,
        #[cfg(feature = "layout-layered")]
        FlowchartSvgRoute::Layered(_) => OperationPhase::Emit,
        _ => ctx.work_phase,
    }
}

pub fn prepare_edge_routes(
    ctx: &FlowchartRenderCtx<'_>,
    render_edges: &[FlowchartRenderEdge<'_>],
    hierarchy: &FlowchartHierarchyPlan<'_>,
    measure_neo_length: bool,
) -> Result<FxHashMap<crate::projection::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>> {
    let phase = edge_paint_phase(ctx);
    ctx.work_meter.charge_work(render_edges.len(), phase)?;
    let mut cache = FxHashMap::with_capacity_and_hasher(render_edges.len(), Default::default());
    let mut scratch = FlowchartEdgeDataPointsScratch::default();
    if ctx.uses_elk_adapter_dom {
        for (request, offsets, route) in
            prepare_elk_clipped_routes(ctx, render_edges, hierarchy, &mut scratch)?
        {
            if let Some(geom) = edge_geom::finish_edge_route(request, route, &mut scratch, false) {
                cache.insert(
                    request.key,
                    FlowchartEdgePathCacheEntry {
                        origin_x: offsets.origin_x,
                        origin_y: offsets.origin_y,
                        abs_top_transform: offsets.abs_top_transform,
                        geom,
                    },
                );
            }
        }
    } else {
        for edge in render_edges {
            let (request, offsets) = charged_edge_route_request(ctx, edge.as_ref(), hierarchy)?;
            if let Some(geom) =
                flowchart_compute_edge_path_geom(request, &mut scratch, measure_neo_length)?
            {
                cache.insert(
                    request.key,
                    FlowchartEdgePathCacheEntry {
                        origin_x: offsets.origin_x,
                        origin_y: offsets.origin_y,
                        abs_top_transform: offsets.abs_top_transform,
                        geom,
                    },
                );
            }
        }
    }
    #[cfg(feature = "layout-grid")]
    if let Some(settings) = ctx.layout.route_contract.grid_paint() {
        edge_geom::apply_grid_line_hops(&mut cache, render_edges, settings, ctx.work_meter)?;
        ctx.work_meter.checkpoint(phase)?;
        return Ok(cache);
    }
    #[cfg(feature = "layout-layered")]
    if let FlowchartSvgRoute::Layered(routes) = ctx.layout.route_contract {
        edge_geom::apply_layered_line_hops(ctx, &mut cache, render_edges, routes.routing)?;
        ctx.work_meter.checkpoint(phase)?;
        return Ok(cache);
    }
    let preserve_dash_mask_length = |edge: &FlowchartRenderEdge<'_>| {
        ctx.compatibility.look.is_neo()
            && flowchart_neo_mask_uses_dash_pairs(edge.edge.as_ref())
            && ctx
                .edge_style_plan
                .animation_for(edge.key)
                .is_ok_and(|animation| !animation.is_active())
    };
    let legacy_hops = matches!(ctx.layout.route_contract, FlowchartSvgRoute::Swimlane)
        || ctx.uses_elk_adapter_dom;
    #[cfg(feature = "layout-shoal")]
    if ctx.swimlane_direction.is_some() && ctx.layout.route_contract.is_canonical_axis() {
        super::swimlane::apply_canonical_line_hops(
            &mut cache,
            render_edges,
            ctx,
            preserve_dash_mask_length,
        )?;
        ctx.work_meter
            .checkpoint(merman_model::OperationPhase::Layout)?;
        return Ok(cache);
    }
    if legacy_hops {
        super::swimlane::apply_line_hops_to_edge_geometries(
            &mut cache,
            render_edges,
            ctx.config,
            ctx.work_meter,
            ctx.uses_elk_adapter_dom,
            preserve_dash_mask_length,
        )?;
    }
    ctx.work_meter
        .checkpoint(merman_model::OperationPhase::Layout)?;
    Ok(cache)
}

pub(super) fn paints_rough_edges_first(
    ctx: &FlowchartRenderCtx<'_>,
    adaptive_edges: bool,
    cutout_labels: bool,
) -> bool {
    #[cfg(feature = "layout-shoal")]
    if swimlane_label_seating::applies(ctx) {
        return true;
    }
    adaptive_edges
        || cutout_labels
        || uses_theme_sketch(ctx)
        || !ctx.hand_drawn_seed.seed().may_use_math_random()
}

pub(super) fn prepare_rough_edge_paths(
    ctx: &FlowchartRenderCtx<'_>,
    edges: &[super::render_input::FlowchartRenderEdgeRef<'_>],
    cache: &mut FxHashMap<crate::projection::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
) -> Result<()> {
    let hand_drawn = ctx.compatibility.look.is_hand_drawn();
    let theme_sketch = uses_theme_sketch(ctx);
    if !hand_drawn && !theme_sketch {
        return Ok(());
    }
    for edge in edges {
        if !hand_drawn && has_authored_curve(ctx, edge.key) {
            continue;
        }
        let Some(entry) = cache.get_mut(&edge.key) else {
            continue;
        };
        if entry.geom.line_hop_applied || entry.geom.emitted_d_for_label.is_some() {
            continue;
        }
        ctx.work_meter.charge(entry.geom.d.len().max(1))?;
        #[cfg(feature = "canonical-edge-paint")]
        if matches!(
            ctx.layout.route_contract,
            FlowchartSvgRoute::CanonicalAxis(_)
        ) {
            #[cfg(feature = "layout-shoal")]
            if theme_sketch {
                let (path, shaft) =
                    merman_render_core::rough_svg::constrained_rough_edge_with_geometry(
                        &entry.geom.d,
                        flowchart_marker_metrics(edge.edge),
                        ctx.work_meter,
                        &ctx.hand_drawn_seed,
                    )?;
                let bounds = kurbo::Shape::bounding_box(&shaft);
                entry.geom.pb = Some(path_bounds::SvgPathBounds {
                    min_x: bounds.x0,
                    min_y: bounds.y0,
                    max_x: bounds.x1,
                    max_y: bounds.y1,
                });
                entry.geom.emitted_d_for_label = Some(path);
                continue;
            }
            entry.geom.emitted_d_for_label = Some(constrained_rough_edge_path(
                &entry.geom.d,
                flowchart_marker_metrics(edge.edge),
                ctx.work_meter,
                &ctx.hand_drawn_seed,
            )?);
            continue;
        }
        entry.geom.emitted_d_for_label =
            render::node::roughjs::roughjs_hand_drawn_stroke_path_for_svg_path(
                &entry.geom.d,
                0.3,
                ctx.work_meter,
                &ctx.hand_drawn_seed,
            );
    }
    Ok(())
}

pub(super) fn uses_theme_sketch(ctx: &FlowchartRenderCtx<'_>) -> bool {
    use merman_theme_contract::v2::EdgeStyle;

    ctx.layout.route_contract.is_canonical_axis()
        && ctx.native.is_some_and(|native| {
            native.form.edge_style == EdgeStyle::Sketch
                && crate::theme::default_curve(native.form.edge_style, ctx.model, ctx.config)
                    .is_some()
        })
}

pub(super) fn has_authored_curve(
    ctx: &FlowchartRenderCtx<'_>,
    key: crate::projection::FlowchartEdgeKey,
) -> bool {
    ctx.model
        .edges
        .get(key.semantic_index())
        .is_some_and(|edge| {
            edge.interpolate
                .as_ref()
                .is_some_and(|curve| !curve.is_empty())
        })
}

#[derive(Default)]
pub struct FlowchartEdgeLabels {
    positions: FxHashMap<crate::projection::FlowchartEdgeKey, merman_render_core::LayoutPoint>,
    synthetic: FxHashSet<crate::projection::FlowchartEdgeKey>,
}

impl FlowchartEdgeLabels {
    pub fn prepare(
        ctx: &FlowchartRenderCtx<'_>,
        hierarchy: &FlowchartHierarchyPlan<'_>,
        cache: &FxHashMap<crate::projection::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    ) -> Result<Self> {
        let mut result = Self {
            positions: FxHashMap::default(),
            synthetic: hierarchy
                .rendered_node_ids()
                .filter_map(|id| {
                    ctx.swimlane_edge_label_edges_by_node_id
                        .get(id)
                        .map(|edge| edge.key)
                })
                .collect(),
        };
        result.update(ctx, hierarchy, cache, &ctx.edge_order)?;
        #[cfg(feature = "layout-shoal")]
        if swimlane_label_seating::applies(ctx) && !result.positions.is_empty() {
            swimlane_label_seating::reseat(ctx, cache, &mut result.positions)?;
        }
        Ok(result)
    }

    pub(super) fn update(
        &mut self,
        ctx: &FlowchartRenderCtx<'_>,
        hierarchy: &FlowchartHierarchyPlan<'_>,
        cache: &FxHashMap<crate::projection::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
        edges: &[super::render_input::FlowchartRenderEdgeRef<'_>],
    ) -> Result<()> {
        ctx.work_meter.charge(edges.len())?;
        for edge in edges {
            let text = ctx
                .model
                .edge_label_for_occurrence(edge.key, edge.edge)
                .unwrap_or_default();
            if ctx.swimlane_direction.is_some() && !self.synthetic.contains(&edge.key) {
                continue;
            }
            // Keep the resolved canonical seat during subsequent root-local paint updates.
            if ctx.swimlane_direction.is_some()
                && ctx.layout.route_contract.is_canonical_axis()
                && self.positions.contains_key(&edge.key)
            {
                continue;
            }
            if ctx.uses_elk_adapter_dom
                && crate::svg::label::flowchart_label_is_empty_for_render(text)
            {
                continue;
            }
            let Some(&layout_edge) = ctx.layout_edges_by_key.get(&edge.key) else {
                continue;
            };
            let root = hierarchy.edge_root(edge.key)?.unwrap_or("");
            let offsets = hierarchy
                .root_offsets(root)
                .unwrap_or(FlowchartRootOffsets {
                    origin_x: 0.0,
                    origin_y: 0.0,
                    abs_top_transform: 0.0,
                });
            let (origin_x, origin_y) = if ctx.uses_elk_adapter_dom {
                (0.0, 0.0)
            } else {
                (offsets.origin_x, offsets.origin_y)
            };
            let html = if ctx.swimlane_direction.is_some() {
                ctx.node_html_labels
            } else {
                ctx.edge_html_labels
            };
            let label_type = if ctx.swimlane_direction.is_some() {
                "text"
            } else {
                edge.label_type.as_deref().unwrap_or("text")
            };
            let plain = flowchart_label_plain_text(text, label_type, html);
            let empty = crate::svg::label::flowchart_label_text_is_empty_for_mode(&plain, html);
            let position = if let Some(label) = layout_edge.label() {
                if ctx.swimlane_direction.is_none() && !html && empty && text.is_empty() {
                    continue;
                }
                render::resolve_flowchart_edge_label_position(
                    ctx,
                    edge.key,
                    layout_edge,
                    label,
                    origin_x,
                    origin_y,
                    cache,
                    false,
                )
            } else {
                if ctx.swimlane_direction.is_some() || empty {
                    continue;
                }
                fallback_label_position(ctx, layout_edge.points(), origin_x, origin_y)
            };
            self.positions.insert(
                edge.key,
                merman_render_core::LayoutPoint {
                    x: position.x + origin_x - ctx.tx,
                    y: position.y + origin_y - ctx.ty,
                },
            );
        }
        Ok(())
    }

    pub(super) fn local_position(
        &self,
        key: crate::projection::FlowchartEdgeKey,
        ctx: &FlowchartRenderCtx<'_>,
        origin_x: f64,
        origin_y: f64,
    ) -> Option<merman_render_core::LayoutPoint> {
        self.positions
            .get(&key)
            .map(|point| merman_render_core::LayoutPoint {
                x: point.x + ctx.tx - origin_x,
                y: point.y + ctx.ty - origin_y,
            })
    }

    pub(super) fn position(
        &self,
        key: crate::projection::FlowchartEdgeKey,
    ) -> Option<merman_render_core::LayoutPoint> {
        self.positions.get(&key).cloned()
    }
}

fn fallback_label_position(
    ctx: &FlowchartRenderCtx<'_>,
    points: &[merman_render_core::LayoutPoint],
    origin_x: f64,
    origin_y: f64,
) -> merman_render_core::LayoutPoint {
    let points: Vec<_> = points
        .iter()
        .map(|point| merman_render_core::LayoutPoint {
            x: point.x + ctx.tx - origin_x,
            y: point.y + ctx.ty - origin_y,
        })
        .collect();
    merman_render_core::svg_edge_parity::edge_label_geometry::position_edge_label(
        merman_render_core::LayoutPoint {
            x: ctx.tx - origin_x,
            y: ctx.ty - origin_y,
        },
        None,
        &points,
        "",
        true,
    )
}

pub fn prepare_query_rough_paths(
    ctx: &FlowchartRenderCtx<'_>,
    hierarchy: &FlowchartHierarchyPlan<'_>,
    routes: &mut FxHashMap<crate::projection::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    labels: &FlowchartEdgeLabels,
    services: &FlowchartSvgServices<'_>,
) -> Result<bool> {
    if !ctx.compatibility.look.is_hand_drawn() && !uses_theme_sketch(ctx) {
        return Ok(false);
    }
    let mut needed: FxHashSet<_> = ctx
        .edge_order
        .iter()
        .filter_map(|edge| {
            let route = routes.get(&edge.key)?;
            (labels.position(edge.key).is_some()
                && !route.geom.line_hop_applied
                && !route.geom.label_path_was_explicitly_updated
                && !route.geom.label_path_points.is_empty())
            .then_some(edge.key)
        })
        .collect();
    if needed.is_empty() {
        return Ok(false);
    }
    if !ctx.hand_drawn_seed.seed().may_use_math_random() {
        for edge in hierarchy
            .ordered_edges()
            .iter()
            .filter(|edge| needed.contains(&edge.key))
        {
            prepare_rough_edge_paths(ctx, std::slice::from_ref(edge), routes)?;
        }
        return Ok(true);
    }

    if paints_rough_edges_first(
        ctx,
        super::edge_fill::FlowchartEdgeFillPlan::applies(ctx),
        super::edge_label_cutout::applies(ctx, services),
    ) {
        for edge in hierarchy.ordered_edges() {
            prepare_rough_edge_paths(ctx, std::slice::from_ref(edge), routes)?;
            needed.remove(&edge.key);
            if needed.is_empty() {
                break;
            }
        }
        return Ok(true);
    }

    // Only earlier emitted geometry can advance the fallback stream used by a requested label.
    // Visit the same root-local clusters, edges and DOM nodes as SVG and stop at the last need.
    let mut frames = vec![(None, 0.0, 0.0, false, 0usize)];
    while let Some((root_id, origin_x, origin_y, initialized, next_node)) = frames.pop() {
        ctx.work_meter.charge(1)?;
        let root = hierarchy.root(root_id)?;
        if !initialized {
            for cluster in root.clusters() {
                render::advance_flowchart_cluster_rough_geometry(ctx, cluster, origin_x, origin_y)?;
            }
            for edge in root.edges() {
                prepare_rough_edge_paths(ctx, std::slice::from_ref(edge), routes)?;
                needed.remove(&edge.key);
                if needed.is_empty() {
                    return Ok(true);
                }
            }
        }
        if ctx.uses_elk_adapter_dom {
            return Ok(true);
        }
        let Some(&id) = root.dom_order().get(next_node) else {
            continue;
        };
        frames.push((root_id, origin_x, origin_y, true, next_node + 1));
        if ctx.swimlane_edge_label_edges_by_node_id.contains_key(id) {
            continue;
        }
        if ctx.subgraphs_by_id.contains_key(id)
            && ctx.subgraph_has_children(id)
            && !ctx.is_subgraph_collapsed(id)
        {
            if ctx.recursive_clusters.contains(id) {
                let offsets = hierarchy.root_offsets(id);
                frames.push((
                    Some(id),
                    offsets.map_or(origin_x, |offsets| offsets.origin_x),
                    offsets.map_or(origin_y, |offsets| offsets.origin_y),
                    false,
                    0,
                ));
            }
        } else {
            render::node::advance_rough_geometry(ctx, id)?;
        }
    }
    Ok(true)
}

pub fn edge_geometry(
    request: FlowchartSvgModelRequest<'_>,
    services: &FlowchartSvgServices<'_>,
) -> Result<Vec<merman_render_core::EdgePaintGeometry>> {
    with_flowchart_paint_context(request, services, |ctx, edges, hierarchy| {
        let mut routes = prepare_edge_routes(ctx, edges, hierarchy, false)?;
        #[cfg(feature = "layout-shoal")]
        if swimlane_label_seating::applies(ctx) {
            prepare_rough_edge_paths(ctx, hierarchy.ordered_edges(), &mut routes)?;
        }
        let mut labels = FlowchartEdgeLabels::prepare(ctx, hierarchy, &routes)?;
        if prepare_query_rough_paths(ctx, hierarchy, &mut routes, &labels, services)? {
            labels.update(ctx, hierarchy, &routes, &ctx.edge_order)?;
        }
        let mut result = Vec::with_capacity(edges.len());
        for edge in edges {
            let points = routes
                .get(&edge.key)
                .map(|entry| {
                    ctx.work_meter.charge(entry.geom.data_points.len())?;
                    Ok::<_, merman_render_core::Error>(
                        entry
                            .geom
                            .data_points
                            .iter()
                            .map(|point| merman_render_core::LayoutPoint {
                                x: point.x + entry.origin_x - ctx.tx,
                                y: point.y + entry.origin_y - ctx.ty,
                            })
                            .collect(),
                    )
                })
                .transpose()?;
            ctx.work_meter
                .checkpoint(merman_model::OperationPhase::Emit)?;
            result.push(merman_render_core::EdgePaintGeometry {
                id: edge.edge.id.clone(),
                points,
                label_position: labels.position(edge.key),
            });
        }
        Ok(result)
    })
}

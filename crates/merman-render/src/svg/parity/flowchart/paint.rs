use super::render_input::{
    FlowchartRenderEdge, FlowchartRenderInputs, prepare_flowchart_render_inputs,
};
use super::svg_emit::FlowchartSvgModelRequest;
use super::*;

pub(super) fn with_flowchart_paint_context<T>(
    request: FlowchartSvgModelRequest<'_>,
    options: &SvgExecution<'_>,
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
        theme_evidence,
        effect_evidence,
        expected_effect_applications: _,
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

    let measurer = options.text_measurer();

    let hand_drawn_seed = options.rough_randomness(
        prepared_theme
            .compatibility
            .hand_drawn_seed
            .unwrap_or(options.seed() as f64),
        "render.flowchart.roughjs",
    );

    let diagram_id = options.diagram_id_or("merman");

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
        if edges_by_key.insert(edge.key, edge).is_some() {
            return Err(crate::Error::InvalidModel {
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
        return Err(crate::Error::InvalidModel {
            message: "Flowchart render occurrences do not match the prepared layout".to_string(),
        });
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
                message: format!("Flowchart layout contains duplicate edge occurrence {key:?}"),
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
        document_ids: std::cell::OnceCell::new(),
        layout,
        swimlane_layout,
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
    consume(&ctx, &render_edges, &hierarchy_plan)
}

pub(super) fn prepare_edge_routes(
    ctx: &FlowchartRenderCtx<'_>,
    render_edges: &[FlowchartRenderEdge<'_>],
    hierarchy: &FlowchartHierarchyPlan<'_>,
) -> Result<FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>> {
    ctx.work_meter.charge(render_edges.len())?;
    let mut cache = FxHashMap::with_capacity_and_hasher(render_edges.len(), Default::default());
    let mut scratch = FlowchartEdgeDataPointsScratch::default();
    let mut routes = Vec::new();
    let mut routed_edges = Vec::new();
    for edge in render_edges {
        let edge = edge.as_ref();
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
            ctx.work_meter.charge(layout_edge.points.len().max(2))?;
        }
        if ctx.uses_elk_adapter_dom {
            if let Some(route) = edge_geom::prepare_edge_route(request, &mut scratch) {
                routes.push(route);
                routed_edges.push((edge, offsets));
            }
        } else if let Some(geom) = flowchart_compute_edge_path_geom(request, &mut scratch) {
            cache.insert(
                edge.key,
                FlowchartEdgePathCacheEntry {
                    origin_x: offsets.origin_x,
                    origin_y: offsets.origin_y,
                    abs_top_transform: offsets.abs_top_transform,
                    geom,
                },
            );
        }
    }
    if ctx.uses_elk_adapter_dom {
        if ctx
            .config
            .as_value()
            .get("elk")
            .and_then(|value| value.get("straightenEdges"))
            .and_then(serde_json::Value::as_bool)
            != Some(false)
        {
            edge_geom::straighten_edge_terminals(&mut routes, ctx.work_meter)?;
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
        )?;
        for ((edge, offsets), route) in routed_edges.into_iter().zip(routes) {
            if let Some(geom) = edge_geom::finish_edge_route(
                FlowchartEdgePathGeomRequest {
                    ctx,
                    key: edge.key,
                    edge: edge.edge,
                    origin_x: offsets.origin_x,
                    origin_y: offsets.origin_y,
                    trace_enabled: false,
                },
                route,
                &mut scratch,
            ) {
                cache.insert(
                    edge.key,
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
    if ctx.swimlane_direction.is_some() || ctx.uses_elk_adapter_dom {
        super::swimlane::apply_line_hops_to_edge_geometries(
            &mut cache,
            render_edges,
            ctx.config,
            ctx.work_meter,
            ctx.uses_elk_adapter_dom,
        )?;
    }
    ctx.work_meter
        .checkpoint(merman_core::OperationPhase::Layout)?;
    Ok(cache)
}

pub(super) fn prepare_rough_edge_paths(
    ctx: &FlowchartRenderCtx<'_>,
    edges: &[super::render_input::FlowchartRenderEdgeRef<'_>],
    cache: &mut FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
) -> Result<()> {
    if !ctx.compatibility.look.is_hand_drawn() {
        return Ok(());
    }
    for edge in edges {
        let Some(entry) = cache.get_mut(&edge.key) else {
            continue;
        };
        if entry.geom.line_hop_applied || entry.geom.emitted_d_for_label.is_some() {
            continue;
        }
        ctx.work_meter.charge(entry.geom.d.len().max(1))?;
        entry.geom.emitted_d_for_label =
            render::node::roughjs::roughjs_hand_drawn_stroke_path_for_svg_path(
                &entry.geom.d,
                0.3,
                &ctx.hand_drawn_seed,
            );
    }
    Ok(())
}

#[derive(Default)]
pub(in crate::svg::parity) struct FlowchartEdgeLabels {
    positions: FxHashMap<crate::flowchart::FlowchartEdgeKey, crate::model::LayoutPoint>,
    synthetic: FxHashSet<crate::flowchart::FlowchartEdgeKey>,
}

impl FlowchartEdgeLabels {
    pub(super) fn prepare(
        ctx: &FlowchartRenderCtx<'_>,
        hierarchy: &FlowchartHierarchyPlan<'_>,
        cache: &FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
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
        Ok(result)
    }

    pub(super) fn update(
        &mut self,
        ctx: &FlowchartRenderCtx<'_>,
        hierarchy: &FlowchartHierarchyPlan<'_>,
        cache: &FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
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
            if ctx.uses_elk_adapter_dom
                && crate::flowchart::flowchart_label_is_empty_for_render(text)
            {
                continue;
            }
            let Some(layout_edge) = ctx.layout_edges_by_key.get(&edge.key) else {
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
            let empty = crate::flowchart::flowchart_label_text_is_empty_for_mode(&plain, html);
            let position = if let Some(label) = layout_edge.label.as_ref() {
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
                fallback_label_position(ctx, layout_edge, origin_x, origin_y)
            };
            self.positions.insert(
                edge.key,
                crate::model::LayoutPoint {
                    x: position.x + origin_x - ctx.tx,
                    y: position.y + origin_y - ctx.ty,
                },
            );
        }
        Ok(())
    }

    pub(super) fn local_position(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
        ctx: &FlowchartRenderCtx<'_>,
        origin_x: f64,
        origin_y: f64,
    ) -> Option<crate::model::LayoutPoint> {
        self.positions
            .get(&key)
            .map(|point| crate::model::LayoutPoint {
                x: point.x + ctx.tx - origin_x,
                y: point.y + ctx.ty - origin_y,
            })
    }

    fn position(
        &self,
        key: crate::flowchart::FlowchartEdgeKey,
    ) -> Option<crate::model::LayoutPoint> {
        self.positions.get(&key).cloned()
    }
}

fn fallback_label_position(
    ctx: &FlowchartRenderCtx<'_>,
    edge: &crate::model::LayoutEdge,
    origin_x: f64,
    origin_y: f64,
) -> crate::model::LayoutPoint {
    let points: Vec<_> = edge
        .points
        .iter()
        .map(|point| crate::model::LayoutPoint {
            x: point.x + ctx.tx - origin_x,
            y: point.y + ctx.ty - origin_y,
        })
        .collect();
    crate::svg::parity::edge_label_geometry::position_edge_label(
        crate::model::LayoutPoint {
            x: ctx.tx - origin_x,
            y: ctx.ty - origin_y,
        },
        None,
        &points,
        "",
        true,
    )
}

fn prepare_query_rough_paths(
    ctx: &FlowchartRenderCtx<'_>,
    hierarchy: &FlowchartHierarchyPlan<'_>,
    routes: &mut FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    labels: &FlowchartEdgeLabels,
) -> Result<bool> {
    if !ctx.compatibility.look.is_hand_drawn() {
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

pub(super) fn edge_geometry(
    request: FlowchartSvgModelRequest<'_>,
    options: &SvgExecution<'_>,
) -> Result<Vec<crate::model::EdgePaintGeometry>> {
    with_flowchart_paint_context(request, options, |ctx, edges, hierarchy| {
        let mut routes = prepare_edge_routes(ctx, edges, hierarchy)?;
        let mut labels = FlowchartEdgeLabels::prepare(ctx, hierarchy, &routes)?;
        if prepare_query_rough_paths(ctx, hierarchy, &mut routes, &labels)? {
            labels.update(ctx, hierarchy, &routes, &ctx.edge_order)?;
        }
        let mut result = Vec::with_capacity(edges.len());
        for edge in edges {
            let points = routes
                .get(&edge.key)
                .map(|entry| {
                    ctx.work_meter.charge(entry.geom.data_points.len())?;
                    Ok::<_, crate::Error>(
                        entry
                            .geom
                            .data_points
                            .iter()
                            .map(|point| crate::model::LayoutPoint {
                                x: point.x + entry.origin_x - ctx.tx,
                                y: point.y + entry.origin_y - ctx.ty,
                            })
                            .collect(),
                    )
                })
                .transpose()?;
            ctx.work_meter
                .checkpoint(merman_core::OperationPhase::Emit)?;
            result.push(crate::model::EdgePaintGeometry {
                id: edge.edge.id.clone(),
                points,
                label_position: labels.position(edge.key),
            });
        }
        Ok(result)
    })
}

#[cfg(all(test, feature = "diagram-flowchart"))]
mod tests {
    use super::*;

    fn with_source<T>(
        source: &str,
        tiny_svg_budget: bool,
        consume: impl FnOnce(FlowchartSvgModelRequest<'_>, &SvgExecution<'_>) -> Result<T>,
    ) -> T {
        let parsed = merman_core::Engine::new()
            .parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let environment = crate::environment::RenderEnvironment::deterministic();
        let merman_core::RenderSemanticModel::Flowchart(model) = parsed.model() else {
            panic!("Flowchart model")
        };
        let context = parsed.flowchart_render_context().unwrap();
        let layout_session = environment.clone().begin_session().unwrap();
        let layout_style = FlowchartEdgeStylePlan::prepare_for_model(
            model,
            &parsed.metadata().effective_config,
            false,
            layout_session.work_meter(),
        )
        .unwrap();
        let layout = crate::flowchart::layout_flowchart_typed_with_render_labels_and_svg_label_sidecar_and_work_meter(model, context, &parsed.metadata().effective_config, &layout_session.text_measurer(crate::environment::TextMeasurementPhase::Layout), None, None, &layout_style, std::sync::Arc::clone(layout_session.work_meter())).unwrap();
        let mut resources = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input();
        if tiny_svg_budget {
            resources = resources
                .with_limit(crate::resources::ResourceLimitId::MaxSvgBytes, 1)
                .unwrap();
        }
        let session = environment
            .with_resource_policy(resources)
            .begin_session()
            .unwrap();
        let options = SvgRenderOptions::default();
        let debug = SvgDebugOptions::default();
        let execution = SvgExecution::unthemed_for_test(
            &options,
            &debug,
            &session,
            crate::DiagramFamilyId::FLOWCHART,
        )
        .unwrap();
        let sidecar = crate::flowchart::FlowchartSvgLabelSidecar::default();
        let theme = crate::flowchart::FlowchartPreparedTheme::resolve(
            None,
            &parsed.metadata().effective_config,
            super::super::render_config::flowchart_node_label_fill_config_override(
                &parsed.metadata().effective_config,
            ),
            execution.work_meter(),
        )
        .unwrap();
        let edge_theme =
            crate::flowchart::FlowchartEdgeThemeStyle::resolve(None, execution.work_meter())
                .unwrap();
        let config = super::super::render_config::prepare_flowchart_render_config(
            model,
            &parsed.metadata().effective_config,
            &theme.compatibility,
            layout.uses_elk_adapter_dom,
            None,
            sidecar.edge_label_padding(),
        );
        let style = FlowchartEdgeStylePlan::prepare_for_model(
            model,
            &parsed.metadata().effective_config,
            layout.uses_elk_adapter_dom,
            execution.work_meter(),
        )
        .unwrap()
        .with_resolved_stroke_widths(
            model,
            &edge_theme,
            &parsed.metadata().effective_config,
            &theme.compatibility,
            execution.work_meter(),
        )
        .unwrap()
        .with_typography_work_meter(std::sync::Arc::clone(session.work_meter()));
        style
            .bind_terminal_typography(&config.text_style, false)
            .unwrap();
        let nodes = super::super::node_inventory::FlowchartPreparedNodes::prepare(
            model,
            context,
            super::super::node_inventory::FlowchartNodeLayoutView::Flowchart(&layout),
            &sidecar,
            None,
            &parsed.metadata().effective_config,
            &theme,
            &config,
            session.work_meter(),
        )
        .unwrap();
        consume(
            FlowchartSvgModelRequest {
                render_config: &config,
                prepared_nodes: &nodes,
                prepared_theme: &theme,
                layout: &layout,
                swimlane_layout: None,
                model,
                render_context: context,
                effective_config: &parsed.metadata().effective_config,
                diagram_type: &parsed.metadata().diagram_type,
                diagram_title: None,
                svg_label_sidecar: &sidecar,
                effect_evidence: &Default::default(),
                expected_effect_applications: &Default::default(),
                theme_evidence: &Default::default(),
                edge_style_plan: &style,
                edge_theme: &edge_theme,
            },
            &execution,
        )
        .unwrap()
    }

    #[test]
    fn geometry_frontier_preserves_the_actual_rough_paths_after_nested_prefixes() {
        for seed in [-1_i64, 4_294_967_296, 7] {
            let source = format!(
                "---\nconfig:\n  layout: dagre\n  htmlLabels: false\n  look: handDrawn\n  handDrawnSeed: {seed}\n---\nflowchart LR\nsubgraph Group\n direction TB\n A((Circle)) -->|inner| B[End]\nend\nX -->|outer| Y\nY --> Group"
            );
            let svg = with_source(&source, false, |request, execution| {
                super::super::svg_emit::render_flowchart_svg_model(request, execution)
                    .map(|svg| svg.as_str().to_owned())
            });
            let document = crate::xml_dom::Document::parse(&svg).unwrap();
            let actual: FxHashMap<_, _> = document
                .descendants()
                .filter(|node| node.has_tag_name("path") && node.attribute("data-points").is_some())
                .map(|node| {
                    (
                        node.attribute("data-id").unwrap().to_owned(),
                        node.attribute("d").unwrap().to_owned(),
                    )
                })
                .collect();
            with_source(&source, true, |request, execution| {
                with_flowchart_paint_context(request, execution, |ctx, edges, hierarchy| {
                    let mut routes = prepare_edge_routes(ctx, edges, hierarchy)?;
                    let labels = FlowchartEdgeLabels::prepare(ctx, hierarchy, &routes)?;
                    assert!(prepare_query_rough_paths(
                        ctx,
                        hierarchy,
                        &mut routes,
                        &labels
                    )?);
                    let mut compared = 0;
                    for edge in edges {
                        let Some(rough) = routes
                            .get(&edge.key)
                            .and_then(|entry| entry.geom.emitted_d_for_label.as_ref())
                        else {
                            continue;
                        };
                        assert_eq!(
                            rough, &actual[&edge.edge.id],
                            "seed {seed}, edge {}",
                            edge.edge.id
                        );
                        compared += 1;
                    }
                    assert!(compared >= 2);
                    assert_eq!(ctx.work_meter.projected_svg_bytes(), 0);
                    assert!(ctx.document_ids.get().is_none());
                    Ok(())
                })
            });
        }
    }

    #[test]
    fn geometry_without_positioned_labels_does_not_prepare_rough_paths() {
        let source = "---\nconfig:\n  layout: dagre\n  htmlLabels: false\n  look: handDrawn\n  handDrawnSeed: -1\n---\nflowchart LR\nA --> B";
        with_source(source, true, |request, execution| {
            with_flowchart_paint_context(request, execution, |ctx, edges, hierarchy| {
                let mut routes = prepare_edge_routes(ctx, edges, hierarchy)?;
                let labels = FlowchartEdgeLabels::prepare(ctx, hierarchy, &routes)?;
                let before = ctx.work_meter.used();
                assert!(!prepare_query_rough_paths(
                    ctx,
                    hierarchy,
                    &mut routes,
                    &labels
                )?);
                assert_eq!(before, ctx.work_meter.used());
                assert!(
                    routes
                        .values()
                        .all(|entry| entry.geom.emitted_d_for_label.is_none())
                );
                Ok(())
            })
        });
    }

    #[test]
    fn geometry_preserves_the_elk_missing_section_path_and_label() {
        with_source(
            "flowchart LR\nA((Circle)) -->|label| B{Diamond}",
            false,
            |request, execution| {
                let mut layout = request.layout.clone();
                layout.uses_elk_adapter_dom = true;
                layout.edges[0].points.clear();
                let request = FlowchartSvgModelRequest {
                    layout: &layout,
                    ..request
                };
                let geometry = edge_geometry(request, execution)?;
                let svg = super::super::svg_emit::render_flowchart_svg_model(request, execution)?;
                let document = crate::xml_dom::Document::parse(svg.as_str()).unwrap();
                let path = document
                    .descendants()
                    .find(|node| node.attribute("data-points").is_some())
                    .unwrap();
                use base64::Engine as _;
                let points: Vec<crate::model::LayoutPoint> = serde_json::from_slice(
                    &base64::engine::general_purpose::STANDARD
                        .decode(path.attribute("data-points").unwrap())
                        .unwrap(),
                )
                .unwrap();
                let query = geometry[0]
                    .points
                    .as_ref()
                    .expect("missing section receives endpoint geometry");
                assert_eq!(query.len(), points.len());
                for (query, actual) in query.iter().zip(points.iter()) {
                    assert_eq!((query.x, query.y), (actual.x, actual.y));
                }
                let label = document
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("g")
                            && node.attribute("class") == Some("edgeLabel")
                            && node.attribute("transform").is_some()
                    })
                    .unwrap();
                let (x, y) = label
                    .attribute("transform")
                    .unwrap()
                    .strip_prefix("translate(")
                    .unwrap()
                    .strip_suffix(')')
                    .unwrap()
                    .split_once(',')
                    .unwrap();
                let position = geometry[0].label_position.as_ref().unwrap();
                assert!((position.x - x.trim().parse::<f64>().unwrap()).abs() < 1e-5);
                assert!((position.y - y.trim().parse::<f64>().unwrap()).abs() < 1e-5);
                Ok(())
            },
        );
    }
}

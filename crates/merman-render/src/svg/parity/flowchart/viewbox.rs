//! Flowchart final viewBox/content-bounds preparation.

use merman_render_flow::stylesheet::TITLE_FONT_SIZE_PX;
use rustc_hash::FxHashMap;

use super::super::timing::RenderTiming;
use super::cluster_geometry::painted_cluster_frame;
use super::viewbox_node_bounds::include_flowchart_node_rendered_bounds;
use super::*;

pub(in crate::svg::parity::flowchart) struct FlowchartRenderedBoundsRequest<'view, 'data> {
    pub ctx: &'view FlowchartRenderCtx<'data>,
    pub layout: &'view FlowchartLayout,
    pub subgraph_title_y_shift: f64,
}

pub(in crate::svg::parity::flowchart) struct FlowchartViewboxBoundsRequest<
    'borrow,
    'view,
    'data,
    'title,
> {
    pub ctx: &'view FlowchartRenderCtx<'data>,
    pub render_edges: &'view [super::render_input::FlowchartRenderEdge<'data>],
    pub base_bounds: Bounds,
    pub diagram_title: Option<&'title str>,
    pub font_family: &'borrow str,
    pub title_top_margin: f64,
    pub timing: RenderTiming,
    pub viewbox_edge_curve_bounds: &'borrow mut std::time::Duration,
    pub detail: &'borrow mut FlowchartRenderDetails,
}

pub(in crate::svg::parity::flowchart) struct FlowchartViewboxBounds {
    pub diagram_title: Option<String>,
    pub title_anchor_x: f64,
    pub source_paint_bounds: Option<Bounds>,
    pub bbox_min_x: f64,
    pub bbox_min_y: f64,
    pub bbox_max_x: f64,
    pub bbox_max_y: f64,
    pub edge_path_cache: FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
}

pub(in crate::svg::parity::flowchart) fn prepare_flowchart_rendered_bounds<'data>(
    request: FlowchartRenderedBoundsRequest<'_, 'data>,
    hierarchy_plan: &FlowchartHierarchyPlan<'data>,
) -> Result<Bounds> {
    let FlowchartRenderedBoundsRequest {
        ctx,
        layout,
        subgraph_title_y_shift,
    } = request;
    let y_offset_for_root = |root: Option<&str>| -> f64 {
        if root.is_some() && subgraph_title_y_shift.abs() >= 1e-9 {
            -subgraph_title_y_shift
        } else {
            0.0
        }
    };

    // Mermaid's flowchart-v2 renderer draws the self-loop helper nodes (`labelRect`) as
    // `<g class="label edgeLabel" transform="translate(x, y)">` with a `0.1 x 0.1` rect anchored
    // at the translated origin (top-left). Dagre's `x/y` still represent a node center, but the
    // rendered DOM bbox that drives `setupViewPortForSVG(svg, diagramPadding)` is top-left based.
    // Account for that when approximating the final `svg.getBBox()`.
    let mut b: Option<Bounds> = None;
    let mut include_rect = |min_x: f64, min_y: f64, max_x: f64, max_y: f64| {
        if let Some(ref mut cur) = b {
            cur.min_x = cur.min_x.min(min_x);
            cur.min_y = cur.min_y.min(min_y);
            cur.max_x = cur.max_x.max(max_x);
            cur.max_y = cur.max_y.max(max_y);
        } else {
            b = Some(Bounds {
                min_x,
                min_y,
                max_x,
                max_y,
            });
        }
    };

    for c in &layout.clusters {
        let root = if ctx.recursive_clusters.contains(c.id.as_str()) {
            Some(c.id.as_str())
        } else {
            hierarchy_plan.effective_parent(&c.id)
        };
        let y_off = y_offset_for_root(root);
        let frame = painted_cluster_frame(ctx, c).bounds();
        if !frame.is_zero_area() {
            include_rect(frame.x0, frame.y0 + y_off, frame.x1, frame.y1 + y_off);
        }

        if let Some(geometry) = ctx.elk_cluster_geometry.get(c.id.as_str()) {
            let lhw = geometry.label.width / 2.0;
            let label_top = frame.y0 + y_off + c.title_margin_top.max(0.0);
            include_rect(
                c.x - lhw,
                label_top,
                c.x + lhw,
                label_top + geometry.label.height,
            );
        } else {
            let lhw = c.title_label.width / 2.0;
            let lhh = c.title_label.height / 2.0;
            include_rect(
                c.title_label.x - lhw,
                c.title_label.y + y_off - lhh,
                c.title_label.x + lhw,
                c.title_label.y + y_off + lhh,
            );
        }
    }

    let effective_parent_for_id = |id: &str| hierarchy_plan.effective_parent(id);
    include_flowchart_node_rendered_bounds(
        ctx,
        &layout.nodes,
        subgraph_title_y_shift,
        &effective_parent_for_id,
        &mut include_rect,
    );

    if let Some(native) = ctx.native.filter(|style| style.has_depth()) {
        use kurbo::Shape;

        for node in &layout.nodes {
            if let Some(silhouette) = super::render::node::native_silhouette(ctx, node) {
                let y = node.y + y_offset_for_root(hierarchy_plan.effective_parent(&node.id));
                let bounds = crate::theme::elevation_svg::bounds(
                    &native.elevation,
                    silhouette.bounding_box(),
                );
                include_rect(
                    node.x + bounds.x0,
                    y + bounds.y0,
                    node.x + bounds.x1,
                    y + bounds.y1,
                );
            }
        }
    }

    for (key, edge) in FlowchartLayoutEdgeRef::in_layout_order(layout) {
        let materialized = edge.materialized();
        let root = hierarchy_plan.edge_root(key)?;
        let y_off = y_offset_for_root(root);
        let missing_section_label =
            edge_geom::missing_section_label_position(ctx, key, edge.points(), ctx.tx, ctx.ty);
        for (label_index, lbl) in [
            edge.label(),
            materialized.and_then(|edge| edge.start_label_left.as_ref()),
            materialized.and_then(|edge| edge.start_label_right.as_ref()),
            materialized.and_then(|edge| edge.end_label_left.as_ref()),
            materialized.and_then(|edge| edge.end_label_right.as_ref()),
        ]
        .into_iter()
        .enumerate()
        .filter_map(|(index, label)| label.map(|label| (index, label)))
        {
            // ELK main labels may move after clipping. Include their final painted position
            // below, after terminal straightening and paired-label separation.
            if ctx.uses_elk_adapter_dom && label_index == 0 {
                continue;
            }
            let label_width = lbl.width;
            let hw = label_width / 2.0;
            let label_height = lbl.height;
            let hh = label_height / 2.0;
            let (x, y) = if label_index == 0
                && let Some(point) = &missing_section_label
            {
                (point.x, point.y)
            } else {
                (lbl.x, lbl.y)
            };
            include_rect(x - hw, y + y_off - hh, x + hw, y + y_off + hh);
        }
    }

    Ok(b.unwrap_or({
        if layout.nodes.is_empty()
            && ctx.layout_edges_by_key.is_empty()
            && layout.clusters.is_empty()
        {
            Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 0.0,
                max_y: 0.0,
            }
        } else {
            Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 100.0,
                max_y: 100.0,
            }
        }
    }))
}

pub(in crate::svg::parity::flowchart) fn prepare_flowchart_viewbox_bounds<'data>(
    request: FlowchartViewboxBoundsRequest<'_, '_, 'data, '_>,
    hierarchy_plan: &FlowchartHierarchyPlan<'data>,
) -> Result<FlowchartViewboxBounds> {
    let FlowchartViewboxBoundsRequest {
        ctx,
        render_edges,
        base_bounds,
        diagram_title,
        font_family,
        title_top_margin,
        timing,
        viewbox_edge_curve_bounds,
        detail,
    } = request;

    // Mermaid computes the final viewport using `svg.getBBox()` after inserting the title, then
    // applies `setupViewPortForSVG(svg, diagramPadding)`.
    let diagram_title = diagram_title
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned);

    let mut bbox_min_x = base_bounds.min_x + ctx.tx;
    let mut bbox_min_y = base_bounds.min_y + ctx.ty;
    let mut bbox_max_x = base_bounds.max_x + ctx.tx;
    let mut bbox_max_y = base_bounds.max_y + ctx.ty;

    bbox_max_y += hierarchy_plan.extra_recursive_root_y();

    // Mermaid derives the final viewport using `svg.getBBox()` (after rendering). For flowcharts
    // this includes the actual curve geometry generated by D3 (which can extend beyond the routed
    // polyline points). Union a tight bbox over each rendered edge path into the base bbox.
    let edge_path_cache = {
        let _g = timing.section(viewbox_edge_curve_bounds);
        let edge_path_cache =
            super::paint::prepare_edge_routes(ctx, render_edges, hierarchy_plan, true)?;
        detail.viewbox_edge_curve_geom_calls += edge_path_cache.len() as u32;
        for entry in edge_path_cache.values() {
            if entry.geom.bounds_skipped_for_viewbox {
                detail.viewbox_edge_curve_geom_skipped_bounds += 1;
            }
            if let Some(pb) = entry.geom.pb {
                bbox_min_x = bbox_min_x.min(pb.min_x + entry.origin_x);
                bbox_min_y = bbox_min_y.min(pb.min_y + entry.abs_top_transform);
                bbox_max_x = bbox_max_x.max(pb.max_x + entry.origin_x);
                bbox_max_y = bbox_max_y.max(pb.max_y + entry.abs_top_transform);
            }
        }
        edge_path_cache
    };

    if ctx.uses_elk_adapter_dom {
        ctx.work_meter.charge(render_edges.len())?;
        for edge in render_edges {
            let key = edge.as_ref().key;
            let Some(&layout_edge) = ctx.layout_edges_by_key.get(&key) else {
                continue;
            };
            let Some(label) = layout_edge.label() else {
                continue;
            };
            let anchor = render::resolve_flowchart_edge_label_position(
                ctx,
                key,
                layout_edge,
                label,
                0.0,
                0.0,
                &edge_path_cache,
                false,
            );
            let half_width = label.width / 2.0;
            let half_height = label.height / 2.0;
            bbox_min_x = bbox_min_x.min(anchor.x - half_width);
            bbox_min_y = bbox_min_y.min(anchor.y - half_height);
            bbox_max_x = bbox_max_x.max(anchor.x + half_width);
            bbox_max_y = bbox_max_y.max(anchor.y + half_height);
        }
    }

    // Mermaid centers the title using the pre-title `getBBox()` of the rendered root group.
    let title_anchor_x = (bbox_min_x + bbox_max_x) / 2.0;

    if let Some(title) = diagram_title.as_deref() {
        let compatibility_style;
        let title_style = match ctx.native {
            Some(native) => &native.title_style,
            None => {
                compatibility_style = TextStyle {
                    font_family: Some(font_family.to_string()),
                    font_size: TITLE_FONT_SIZE_PX,
                    font_weight: None,
                    font_style: None,
                };
                &compatibility_style
            }
        };
        let (title_left, title_right) = ctx.measurer.measure_svg_title_bbox_x(title, title_style);
        let baseline_y = -title_top_margin;
        let (ascent, descent) = crate::text::svg_text_vertical_extents_px(
            ctx.measurer,
            title,
            title_style,
            crate::text::svg_title_bbox_vertical_extents_px(title_style),
        );

        bbox_min_x = bbox_min_x.min(title_anchor_x - title_left);
        bbox_max_x = bbox_max_x.max(title_anchor_x + title_right);
        bbox_min_y = bbox_min_y.min(baseline_y - ascent);
        bbox_max_y = bbox_max_y.max(baseline_y + descent);
    }

    let source_paint_bounds = builtin_neo_paint_bounds(ctx, hierarchy_plan)?;

    Ok(FlowchartViewboxBounds {
        diagram_title,
        title_anchor_x,
        source_paint_bounds,
        bbox_min_x,
        bbox_min_y,
        bbox_max_x,
        bbox_max_y,
        edge_path_cache,
    })
}

// Union known source paint after diagram padding, so ordinary padded outputs retain their
// existing viewport. Font-dependent title ink remains owned by the host measurement seam.
fn builtin_neo_paint_bounds(
    ctx: &FlowchartRenderCtx<'_>,
    hierarchy: &FlowchartHierarchyPlan<'_>,
) -> Result<Option<Bounds>> {
    if !ctx.compatibility.look.is_neo() {
        return Ok(None);
    }
    let title_shift =
        crate::flowchart::FlowchartConfigView::new(ctx.config).render_subgraph_title_y_shift();
    let y_offset = |id: &str, recursive: bool| {
        if recursive || hierarchy.effective_parent(id).is_some() {
            -title_shift
        } else {
            0.0
        }
    };
    let mut bounds: Option<Bounds> = None;
    let mut include = |min_x: f64, min_y: f64, max_x: f64, max_y: f64| {
        if let Some(bounds) = &mut bounds {
            bounds.min_x = bounds.min_x.min(min_x);
            bounds.min_y = bounds.min_y.min(min_y);
            bounds.max_x = bounds.max_x.max(max_x);
            bounds.max_y = bounds.max_y.max(max_y);
        } else {
            bounds = Some(Bounds {
                min_x,
                min_y,
                max_x,
                max_y,
            });
        }
    };
    let builtin_shadow = ctx.compatibility.drop_shadow == "url(#drop-shadow)";
    let shadow_offset = merman_render_core::theme_svg::NEO_SHADOW_OFFSET_PX;
    for cluster in ctx.layout_clusters_by_id.values() {
        ctx.work_meter.charge(1)?;
        let frame = painted_cluster_frame(ctx, cluster).bounds();
        if frame.is_zero_area() {
            continue;
        }
        let offset = y_offset(
            &cluster.id,
            ctx.recursive_clusters.contains(cluster.id.as_str()),
        );
        let shadow = if builtin_shadow
            && !super::swimlane::is_lane_or_pool_cluster(ctx, &cluster.id)
            && ctx
                .prepared_nodes
                .cluster(&cluster.id)
                .is_ok_and(|terminal| !terminal.compiled.has_source_filter())
        {
            shadow_offset
        } else {
            0.0
        };
        include(
            frame.x0 - 0.5,
            frame.y0 + offset - 0.5,
            frame.x1 + 0.5 + shadow,
            frame.y1 + offset + 0.5 + shadow,
        );
    }
    if !builtin_shadow {
        return Ok(bounds);
    }
    for id in hierarchy.rendered_node_ids() {
        ctx.work_meter.charge(1)?;
        let Some(node) = ctx.layout_nodes_by_id.get(id) else {
            continue;
        };
        if node.is_cluster && ctx.layout_clusters_by_id.contains_key(id) {
            continue;
        }
        let Some(info) = super::render::node::helpers::resolve_node_render_info(ctx, id) else {
            continue;
        };
        if !matches!(
            crate::flowchart::resolve_flowchart_shape(info.shape)?,
            crate::flowchart::FlowchartShape::Process
                | crate::flowchart::FlowchartShape::RoundedRectangle
        ) {
            continue;
        }
        let Some(prepared) = ctx.prepared_nodes.node(id) else {
            continue;
        };
        let source = &prepared.source;
        if source.has_source_filter() {
            continue;
        }
        let stroke = f64::from(
            source
                .admitted_stroke_width_value()
                .unwrap_or(ctx.node_stroke_width),
        ) / 2.0;
        let y = node.y + y_offset(id, node.is_cluster && ctx.recursive_clusters.contains(id));
        let shape = ctx.rectangle_geometry(node).shape;
        if shape.width() == 0.0 || shape.height() == 0.0 {
            continue;
        }
        include(
            node.x + shape.x0 - stroke,
            y + shape.y0 - stroke,
            node.x + shape.x1 + stroke + shadow_offset,
            y + shape.y1 + stroke + shadow_offset,
        );
    }
    Ok(bounds)
}

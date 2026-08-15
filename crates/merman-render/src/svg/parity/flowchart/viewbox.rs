//! Flowchart final viewBox/content-bounds preparation.

use rustc_hash::FxHashMap;

use super::super::timing::RenderTiming;
use super::viewbox_node_bounds::include_flowchart_node_rendered_bounds;
use super::*;

const TITLE_FONT_SIZE_PX: f64 = 18.0;

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
    pub edge_path_cache:
        &'borrow mut FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
}

pub(in crate::svg::parity::flowchart) struct FlowchartViewboxBounds {
    pub diagram_title: Option<String>,
    pub title_anchor_x: f64,
    pub bbox_min_x: f64,
    pub bbox_min_y: f64,
    pub bbox_max_x: f64,
    pub bbox_max_y: f64,
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
        let hw = c.width / 2.0;
        let hh = c.height / 2.0;
        include_rect(c.x - hw, c.y + y_off - hh, c.x + hw, c.y + y_off + hh);

        let lhw = c.title_label.width / 2.0;
        let lhh = c.title_label.height / 2.0;
        include_rect(
            c.title_label.x - lhw,
            c.title_label.y + y_off - lhh,
            c.title_label.x + lhw,
            c.title_label.y + y_off + lhh,
        );
    }

    let effective_parent_for_id = |id: &str| hierarchy_plan.effective_parent(id);
    include_flowchart_node_rendered_bounds(
        ctx,
        &layout.nodes,
        subgraph_title_y_shift,
        &effective_parent_for_id,
        &mut include_rect,
    );

    for (key, e) in layout.edge_owners.iter().zip(&layout.edges) {
        let root = hierarchy_plan.edge_root(key)?;
        let y_off = y_offset_for_root(root);
        for lbl in [
            e.label.as_ref(),
            e.start_label_left.as_ref(),
            e.start_label_right.as_ref(),
            e.end_label_left.as_ref(),
            e.end_label_right.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            let label_width = lbl.width;
            let hw = label_width / 2.0;
            let label_height = lbl.height;
            let hh = label_height / 2.0;
            let svg_label_y_offset = if ctx.edge_html_labels { 0.0 } else { 1.0 };
            include_rect(
                lbl.x - hw,
                lbl.y + y_off - hh - svg_label_y_offset,
                lbl.x + hw,
                lbl.y + y_off + hh - svg_label_y_offset,
            );
        }
    }

    Ok(b.unwrap_or({
        if layout.nodes.is_empty() && layout.edges.is_empty() && layout.clusters.is_empty() {
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
        edge_path_cache,
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
    // polyline points). Headlessly, approximate that by unioning a tight bbox over each rendered
    // edge path `d` into our base bbox.
    {
        let edge_bounds_base = (bbox_min_x, bbox_min_y, bbox_max_x, bbox_max_y);
        let _g = timing.section(viewbox_edge_curve_bounds);
        let mut scratch = FlowchartEdgeDataPointsScratch::default();
        for e in render_edges {
            let edge_ref = e.as_ref();
            let key = edge_ref.key;
            let edge = edge_ref.edge;
            let root_id = {
                let _g = detail_guard(timing, &mut detail.viewbox_edge_root_lookup);
                hierarchy_plan.edge_root(key)?.unwrap_or("")
            };
            let off = {
                let _g = detail_guard(timing, &mut detail.viewbox_edge_curve_offsets);
                hierarchy_plan
                    .root_offsets(root_id)
                    .unwrap_or(FlowchartRootOffsets {
                        origin_x: 0.0,
                        origin_y: 0.0,
                        abs_top_transform: 0.0,
                    })
            };

            let Some(geom) = ({
                detail.viewbox_edge_curve_geom_calls += 1;
                let _g = detail_guard(timing, &mut detail.viewbox_edge_curve_geom);
                flowchart_compute_edge_path_geom(
                    FlowchartEdgePathGeomRequest {
                        ctx,
                        key,
                        edge,
                        origin_x: off.origin_x,
                        origin_y: off.origin_y,
                        trace_enabled: false,
                        collapse_degenerate_subgraph_route: hierarchy_plan
                            .is_degenerate_subgraph_descendant_edge(ctx, edge),
                    },
                    &mut scratch,
                )
            }) else {
                continue;
            };
            if geom.bounds_skipped_for_viewbox {
                detail.viewbox_edge_curve_geom_skipped_bounds += 1;
            }

            {
                let _g = detail_guard(timing, &mut detail.viewbox_edge_curve_bbox_union);
                if let Some(pb) = geom.pb {
                    bbox_min_x = bbox_min_x.min(pb.min_x + off.origin_x);
                    bbox_min_y = bbox_min_y.min(pb.min_y + off.abs_top_transform);
                    bbox_max_x = bbox_max_x.max(pb.max_x + off.origin_x);
                    bbox_max_y = bbox_max_y.max(pb.max_y + off.abs_top_transform);
                }

                edge_path_cache.insert(
                    key,
                    FlowchartEdgePathCacheEntry {
                        origin_x: off.origin_x,
                        origin_y: off.origin_y,
                        abs_top_transform: off.abs_top_transform,
                        geom,
                    },
                );
            }
        }

        if ctx.swimlane_direction.is_some() {
            super::swimlane::apply_line_hops_to_edge_geometries(
                edge_path_cache,
                render_edges,
                ctx.config,
                ctx.work_meter,
            )?;

            // Line hops are a render-time replacement of the original path. Rebuild edge bounds
            // from the same post-processed geometry that SVG emission consumes, while retaining
            // node, cluster, and label bounds as the base.
            (bbox_min_x, bbox_min_y, bbox_max_x, bbox_max_y) = edge_bounds_base;
            for cache_entry in edge_path_cache.values() {
                let _g = detail_guard(timing, &mut detail.viewbox_edge_curve_bbox_union);
                if let Some(pb) = cache_entry.geom.pb {
                    bbox_min_x = bbox_min_x.min(pb.min_x + cache_entry.origin_x);
                    bbox_min_y = bbox_min_y.min(pb.min_y + cache_entry.abs_top_transform);
                    bbox_max_x = bbox_max_x.max(pb.max_x + cache_entry.origin_x);
                    bbox_max_y = bbox_max_y.max(pb.max_y + cache_entry.abs_top_transform);
                }
            }
        }
    }

    // Mermaid centers the title using the pre-title `getBBox()` of the rendered root group.
    let title_anchor_x = (bbox_min_x + bbox_max_x) / 2.0;

    if let Some(title) = diagram_title.as_deref() {
        let title_style = TextStyle {
            font_family: Some(font_family.to_string()),
            font_size: TITLE_FONT_SIZE_PX,
            font_weight: None,
            font_style: None,
        };
        let (title_left, title_right) = ctx.measurer.measure_svg_title_bbox_x(title, &title_style);
        let baseline_y = -title_top_margin;
        let (ascent, descent) = crate::text::svg_title_bbox_vertical_extents_px(&title_style);

        bbox_min_x = bbox_min_x.min(title_anchor_x - title_left);
        bbox_max_x = bbox_max_x.max(title_anchor_x + title_right);
        bbox_min_y = bbox_min_y.min(baseline_y - ascent);
        bbox_max_y = bbox_max_y.max(baseline_y + descent);
    }

    Ok(FlowchartViewboxBounds {
        diagram_title,
        title_anchor_x,
        bbox_min_x,
        bbox_min_y,
        bbox_max_x,
        bbox_max_y,
    })
}

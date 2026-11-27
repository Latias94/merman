//! Flowchart edge path geometry computation.

use super::*;

pub(in crate::svg::parity::flowchart) fn prepare_edge_route(
    request: FlowchartEdgePathGeomRequest<'_>,
    scratch: &mut FlowchartEdgeDataPointsScratch,
) -> Option<ClippedEdgeRoute> {
    let FlowchartEdgePathGeomRequest {
        ctx,
        key,
        edge,
        origin_x,
        origin_y,
        trace_enabled: _,
    } = request;

    let le = ctx.layout_edges_by_key.get(&key)?.materialized()?;
    let mut label = ctx.uses_elk_adapter_dom.then(|| le.label.clone()).flatten();
    if let Some(label) = label.as_mut() {
        label.x += ctx.tx - origin_x;
        label.y += ctx.ty - origin_y;
    }
    if ctx.uses_elk_adapter_dom && le.points.is_empty() {
        // Mermaid 12 renders providers without sections as clipped straight lines. Keep the
        // raw layout empty so a real two-point section still follows the routed-edge path.
        let points = missing_section_points(ctx, edge, origin_x, origin_y)?;
        if let Some(label) = label.as_mut()
            && let Some(midpoint) =
                missing_section_label_position(ctx, key, &le.points, origin_x, origin_y)
        {
            label.x = midpoint.x;
            label.y = midpoint.y;
        }
        return Some(ClippedEdgeRoute {
            base_points: points.clone(),
            points,
            origin_x,
            origin_y,
            label,
        });
    }
    if le.points.len() < 2 {
        return None;
    }

    let base_points = &mut scratch.tmp_points_a;
    base_points.clear();
    base_points.extend(le.points.iter().map(|p| crate::model::LayoutPoint {
        x: p.x + ctx.tx - origin_x,
        y: p.y + ctx.ty - origin_y,
    }));
    let is_elk_layout = ctx.uses_elk_adapter_dom;
    let is_swimlane_layout = ctx.swimlane_direction.is_some();
    // Swimlane endpoint duplicates pin ports and must survive until clipping.
    if !is_elk_layout && !is_swimlane_layout {
        dedup_consecutive_points(base_points);
    }
    let points_after_intersect = &mut scratch.tmp_points_b;
    points_after_intersect.clone_from(base_points);

    if is_swimlane_layout {
        let layout_from = le.from.as_str();
        let layout_to = le.to.as_str();
        let layout_shape = |id: &str| {
            if ctx.is_subgraph_collapsed(id) {
                Some("collapsedGroup")
            } else {
                ctx.nodes_by_id
                    .get(id)
                    .and_then(|node| node.layout_shape.as_deref())
            }
        };
        if let (Some(tail), Some(head)) = (
            boundary_for_node(ctx, layout_from, origin_x, origin_y),
            boundary_for_node(ctx, layout_to, origin_x, origin_y),
        ) {
            super::super::swimlane::clip_swimlane_endpoints(
                base_points,
                |point| {
                    intersect_for_layout_shape(
                        ctx,
                        layout_from,
                        &tail,
                        layout_shape(layout_from),
                        point,
                    )
                },
                |point| {
                    intersect_for_layout_shape(
                        ctx,
                        layout_to,
                        &head,
                        layout_shape(layout_to),
                        point,
                    )
                },
                super::arrow_types_for_edge(edge.edge_type.as_deref()).1,
                points_after_intersect,
            );
        }
    } else if is_elk_layout {
        apply_flowchart_elk_endpoint_cutter(
            ctx,
            edge,
            origin_x,
            origin_y,
            base_points,
            points_after_intersect,
        );
    } else if base_points.len() >= 3 {
        // The semantic edge keeps its original source/target, while explicit-direction cluster
        // extraction can rebind the Graphlib layout edge to a surviving cluster node. Mermaid
        // passes those graph endpoints to `insertEdge` for shape lookup and clipping.
        let layout_from = le.from.as_str();
        let layout_to = le.to.as_str();
        let tail_shape = if ctx.is_subgraph_collapsed(layout_from) {
            Some("collapsedGroup")
        } else {
            ctx.nodes_by_id
                .get(layout_from)
                .and_then(|n| n.layout_shape.as_deref())
        };
        let head_shape = if ctx.is_subgraph_collapsed(layout_to) {
            Some("collapsedGroup")
        } else {
            ctx.nodes_by_id
                .get(layout_to)
                .and_then(|n| n.layout_shape.as_deref())
        };
        if let (Some(tail), Some(head)) = (
            boundary_for_node(ctx, layout_from, origin_x, origin_y),
            boundary_for_node(ctx, layout_to, origin_x, origin_y),
        ) {
            let interior = &base_points[1..base_points.len() - 1];
            if !interior.is_empty() {
                let mut start = base_points[0].clone();
                let mut end = base_points[base_points.len() - 1].clone();

                let eps = 1e-4;
                let start_is_center =
                    (start.x - tail.x).abs() < eps && (start.y - tail.y).abs() < eps;
                let end_is_center = (end.x - head.x).abs() < eps && (end.y - head.y).abs() < eps;
                let is_compact_self_loop = edge.from == edge.to && le.from == le.to;

                if is_compact_self_loop
                    || start_is_center
                    || force_intersect_for_layout_shape(tail_shape)
                {
                    start = intersect_for_layout_shape(
                        ctx,
                        layout_from,
                        &tail,
                        tail_shape,
                        &interior[0],
                    );
                }

                if is_compact_self_loop
                    || end_is_center
                    || force_intersect_for_layout_shape(head_shape)
                {
                    end = intersect_for_layout_shape(
                        ctx,
                        layout_to,
                        &head,
                        head_shape,
                        &interior[interior.len() - 1],
                    );
                }

                points_after_intersect.clear();
                points_after_intersect.reserve(interior.len() + 2);
                points_after_intersect.push(start);
                points_after_intersect.extend(interior.iter().cloned());
                points_after_intersect.push(end);
            }
        }
    }

    Some(ClippedEdgeRoute {
        base_points: std::mem::take(&mut scratch.tmp_points_a),
        points: std::mem::take(&mut scratch.tmp_points_b),
        origin_x,
        origin_y,
        label,
    })
}

struct PreparedEdgePaint<'a> {
    line_data: Vec<crate::model::LayoutPoint>,
    interpolate: &'a str,
    points_after_intersect_for_trace: Option<Vec<crate::model::LayoutPoint>>,
    original_label_path_points: Vec<crate::model::LayoutPoint>,
    label_path_points: Vec<crate::model::LayoutPoint>,
    label: Option<crate::model::LayoutLabel>,
    is_cluster_edge: bool,
}

fn prepare_edge_paint<'a>(
    request: FlowchartEdgePathGeomRequest<'a>,
    le: &crate::model::LayoutEdge,
    route: ClippedEdgeRoute,
    scratch: &mut FlowchartEdgeDataPointsScratch,
) -> PreparedEdgePaint<'a> {
    let FlowchartEdgePathGeomRequest {
        ctx,
        key,
        edge,
        origin_x,
        origin_y,
        trace_enabled,
    } = request;
    let is_elk_layout = ctx.uses_elk_adapter_dom;
    let clip_clusters = !is_elk_layout;
    let ClippedEdgeRoute {
        base_points,
        points,
        label,
        ..
    } = route;
    let missing_section = is_elk_layout && le.points.is_empty();
    scratch.tmp_points_a = base_points;
    scratch.tmp_points_b = points;
    let base_points = &scratch.tmp_points_a;
    let points_after_intersect = &scratch.tmp_points_b;

    scratch.tmp_points_c.clear();
    // ELK groups were clipped by sanitizeElkEdgePoints. The local cluster metadata is
    // not Dagre's toCluster/fromCluster rewrite; clipping again would discard that route.
    if clip_clusters && let Some(tc) = le.to_cluster.as_deref() {
        if let Some(boundary) = boundary_for_cluster(ctx, tc, origin_x, origin_y) {
            cut_path_at_intersect_into(base_points, &boundary, &mut scratch.tmp_points_c);
        } else {
            scratch
                .tmp_points_c
                .extend_from_slice(points_after_intersect);
        }
    } else {
        scratch
            .tmp_points_c
            .extend_from_slice(points_after_intersect);
    }
    if clip_clusters
        && let Some(fc) = le.from_cluster.as_deref()
        && let Some(boundary) = boundary_for_cluster(ctx, fc, origin_x, origin_y)
    {
        scratch.tmp_points_rev.clear();
        scratch
            .tmp_points_rev
            .extend_from_slice(&scratch.tmp_points_c);
        scratch.tmp_points_rev.reverse();

        cut_path_at_intersect_into(
            &scratch.tmp_points_rev,
            &boundary,
            &mut scratch.tmp_points_c,
        );
        scratch.tmp_points_c.reverse();
    }
    let points_for_render: &mut Vec<crate::model::LayoutPoint> = &mut scratch.tmp_points_c;

    // Mermaid sets `data-points` as `btoa(JSON.stringify(points))` *before* any cluster clipping
    // (`cutPathAtIntersect`). Keep that exact ordering for strict DOM parity.
    let points_after_intersect_for_trace = trace_enabled.then(|| scratch.tmp_points_b.clone());

    let interpolate = if missing_section {
        "linear"
    } else if is_elk_layout {
        "rounded"
    } else {
        edge.interpolate
            .as_deref()
            .unwrap_or(ctx.default_edge_interpolate.as_str())
    };
    let is_rounded = interpolate == "rounded";
    let is_basis = !matches!(
        interpolate,
        "linear"
            | "natural"
            | "bumpX"
            | "bumpY"
            | "catmullRom"
            | "step"
            | "stepAfter"
            | "stepBefore"
            | "cardinal"
            | "monotoneX"
            | "monotoneY"
            | "rounded"
    );

    let is_cluster_edge = clip_clusters && (le.to_cluster.is_some() || le.from_cluster.is_some());
    // `positionEdgeLabel` consumes the polyline held by `points`; `fixCorners`, marker offsets,
    // and the D3 curve generator operate on the separate `lineData` copy below.
    let (original_label_path_points, label_path_points) = if ctx
        .model
        .edge_label_for_occurrence(key, edge)
        .is_some_and(|label| !label.is_empty())
    {
        let original = if is_elk_layout {
            points_after_intersect
        } else {
            base_points
        };
        (original.clone(), points_for_render.clone())
    } else {
        (Vec::new(), Vec::new())
    };

    if is_basis && is_cluster_edge {
        maybe_remove_redundant_cluster_run_point(points_for_render);
    }

    if clip_clusters && points_for_render.len() == 1 {
        // Avoid emitting a degenerate `M x,y` path for clipped cluster-adjacent edges.
        points_for_render.clear();
        points_for_render.extend(le.points.iter().map(|point| crate::model::LayoutPoint {
            x: point.x + ctx.tx - origin_x,
            y: point.y + ctx.ty - origin_y,
        }));
    }

    let line_data = {
        let mut line_data: Vec<crate::model::LayoutPoint> = points_for_render
            .iter()
            .filter(|p| !p.y.is_nan())
            .cloned()
            .collect();

        // Match Mermaid `fixCorners` in `rendering-elements/edges.js`: insert small offset points to
        // round orthogonal corners before feeding into D3's line generator. The `rounded` curve uses
        // its own rounded-corner generator and skips this pre-processing upstream.
        if !is_rounded {
            maybe_fix_corners(&mut line_data);
        }

        // Mermaid shortens edge paths so markers don't render on top of the line (see
        // `packages/mermaid/src/utils/lineWithOffset.ts`).

        if is_rounded {
            rounded_line_with_marker_offsets_for_edge_type(&line_data, edge.edge_type.as_deref())
        } else {
            line_with_offset_for_edge_type(&line_data, edge.edge_type.as_deref())
        }
    };
    PreparedEdgePaint {
        line_data,
        interpolate,
        points_after_intersect_for_trace,
        original_label_path_points,
        label_path_points,
        label,
        is_cluster_edge,
    }
}

pub(in crate::svg::parity::flowchart) fn finish_edge_route(
    request: FlowchartEdgePathGeomRequest<'_>,
    route: ClippedEdgeRoute,
    scratch: &mut FlowchartEdgeDataPointsScratch,
) -> Option<FlowchartEdgePathGeom> {
    let ctx = request.ctx;
    let le = ctx.layout_edges_by_key.get(&request.key)?.materialized()?;
    let PreparedEdgePaint {
        line_data,
        interpolate,
        points_after_intersect_for_trace,
        original_label_path_points,
        label_path_points,
        label,
        is_cluster_edge,
    } = prepare_edge_paint(request, le, route, scratch);
    let geometry = curve_path_geometry(&line_data, interpolate, ctx.edge_corner_radius);
    drop(line_data);

    let points_for_data_points = &scratch.tmp_points_b;
    if request.trace_enabled {
        record_flowchart_edge_trace(FlowchartEdgeTraceInput {
            ctx,
            edge: request.edge,
            layout_edge: Some(le),
            origin_x: request.origin_x,
            origin_y: request.origin_y,
            base_points: &scratch.tmp_points_a,
            points_after_intersect_for_trace: points_after_intersect_for_trace.as_deref(),
            points_for_render: &scratch.tmp_points_c,
            points_for_data_points,
        });
    }

    Some(FlowchartEdgePathGeom {
        d: geometry.d,
        pb: geometry.bounds,
        neo_length: None,
        data_points: points_for_data_points.clone(),
        pre_hop_dash_mask_length: None,
        line_hop_applied: false,
        original_label_path_points,
        label_path_points,
        label_anchor: label.map(|label| crate::model::LayoutPoint {
            x: label.x,
            y: label.y,
        }),
        label_path_was_explicitly_updated: is_cluster_edge,
        emitted_d_for_label: None,
        bounds_skipped_for_viewbox: false,
    })
}

/// The painted linear shaft in root-local coordinates, rounded as in the parity path.
#[cfg(all(
    feature = "diagram-flowchart",
    feature = "layout-dagre",
    any(feature = "native-compact", test)
))]
pub(in crate::svg::parity::flowchart) struct DagreLinearPaintedRoute {
    pub(in crate::svg::parity::flowchart) points: Vec<crate::model::LayoutPoint>,
    pub(in crate::svg::parity::flowchart) bounds: path_bounds::SvgPathBounds,
}

#[cfg(all(
    feature = "diagram-flowchart",
    feature = "layout-dagre",
    any(feature = "native-compact", test)
))]
pub(in crate::svg::parity::flowchart) fn dagre_linear_painted_route(
    request: FlowchartEdgePathGeomRequest<'_>,
    scratch: &mut FlowchartEdgeDataPointsScratch,
) -> Option<DagreLinearPaintedRoute> {
    let ctx = request.ctx;
    if !matches!(
        ctx.layout.route_contract,
        crate::graph_layout::RouteContract::DagreParity
    ) || ctx.uses_elk_adapter_dom
        || ctx.swimlane_direction.is_some()
        || request.trace_enabled
        || !ctx.model.subgraphs.is_empty()
        || request.edge.from == request.edge.to
        || request.key.self_loop_segment_order().is_some()
        || ctx
            .model
            .edge_label_for_occurrence(request.key, request.edge)
            .is_some_and(|label| !label.is_empty())
    {
        return None;
    }
    let route = prepare_edge_route(request, scratch)?;
    let le = ctx.layout_edges_by_key.get(&request.key)?.materialized()?;
    if le.from_cluster.is_some() || le.to_cluster.is_some() {
        return None;
    }
    let prepared = prepare_edge_paint(request, le, route, scratch);
    if prepared.interpolate != "linear" {
        return None;
    }
    DagreLinearPaintedRoute::from_line_data(prepared.line_data)
}

#[cfg(all(
    feature = "diagram-flowchart",
    feature = "layout-dagre",
    any(feature = "native-compact", test)
))]
impl DagreLinearPaintedRoute {
    fn from_line_data(mut points: Vec<crate::model::LayoutPoint>) -> Option<Self> {
        let (first, rest) = points.split_first_mut()?;
        if rest.is_empty() {
            return None;
        }
        round_linear_point(first)?;
        let mut bounds = path_bounds::SvgPathBounds::at(first.x, first.y);
        for point in rest {
            round_linear_point(point)?;
            bounds.include_point(point.x, point.y);
        }
        Some(Self { points, bounds })
    }
}

#[cfg(all(
    feature = "diagram-flowchart",
    feature = "layout-dagre",
    any(feature = "native-compact", test)
))]
fn round_linear_point(point: &mut crate::model::LayoutPoint) -> Option<()> {
    fn coordinate(value: f64) -> Option<f64> {
        if !value.is_finite() {
            return None;
        }
        let thousandths = crate::svg::parity::util::path_coordinate_thousandths(value);
        (thousandths.unsigned_abs() <= (1_u64 << f64::MANTISSA_DIGITS))
            .then(|| thousandths as f64 / 1000.0)
    }

    point.x = coordinate(point.x)?;
    point.y = coordinate(point.y)?;
    Some(())
}

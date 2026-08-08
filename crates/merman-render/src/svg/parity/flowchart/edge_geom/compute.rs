//! Flowchart edge path geometry computation.

use super::*;

pub(super) fn flowchart_compute_edge_path_geom(
    request: FlowchartEdgePathGeomRequest<'_>,
    scratch: &mut FlowchartEdgeDataPointsScratch,
) -> Option<FlowchartEdgePathGeom> {
    let FlowchartEdgePathGeomRequest {
        ctx,
        edge,
        origin_x,
        origin_y,
        trace_enabled,
    } = request;

    let le = ctx.layout_edges_by_id.get(edge.id.as_str())?;
    if le.points.len() < 2 {
        return None;
    }

    scratch.local_points.clear();
    scratch.local_points.reserve(le.points.len());
    for p in &le.points {
        scratch.local_points.push(crate::model::LayoutPoint {
            x: p.x + ctx.tx - origin_x,
            y: p.y + ctx.ty - origin_y,
        });
    }
    let local_points = scratch.local_points.as_slice();

    use super::{
        FlowchartEdgeTraceInput, align_elk_endpoint_adapters_to_route,
        apply_flowchart_elk_endpoint_cutter, boundary_for_cluster, boundary_for_node,
        collapse_short_terminal_marker_stub, curve_path_d_and_bounds, cut_path_at_intersect_into,
        dedup_consecutive_points_into, force_intersect_for_layout_shape,
        intersect_for_layout_shape, is_rounded_intersect_shift_shape,
        line_with_offset_for_edge_type, maybe_collapse_degenerate_subgraph_edge_route,
        maybe_fix_corners, maybe_remove_redundant_cluster_run_point, record_flowchart_edge_trace,
        rounded_line_with_marker_offsets_for_edge_type,
    };

    let is_elk_layout = ctx.diagram_type == "flowchart-elk"
        || ctx
            .config
            .get_str("layout")
            .is_some_and(|layout| layout.eq_ignore_ascii_case("elk"));
    dedup_consecutive_points_into(local_points, &mut scratch.tmp_points_a);
    let base_points: &mut Vec<crate::model::LayoutPoint> = &mut scratch.tmp_points_a;

    scratch.tmp_points_b.clear();
    scratch.tmp_points_b.extend_from_slice(base_points);
    let points_after_intersect: &mut Vec<crate::model::LayoutPoint> = &mut scratch.tmp_points_b;

    let mut elk_endpoint_adapters = super::ElkEndpointAdapterCorners::default();
    if is_elk_layout {
        elk_endpoint_adapters = apply_flowchart_elk_endpoint_cutter(
            ctx,
            edge,
            origin_x,
            origin_y,
            base_points,
            points_after_intersect,
        );
        if ctx.compact_edge_corners {
            align_elk_endpoint_adapters_to_route(
                ctx,
                edge,
                origin_x,
                origin_y,
                &mut elk_endpoint_adapters,
                points_after_intersect,
            );
        }
    } else if base_points.len() >= 3 {
        // The semantic edge keeps its original source/target, while explicit-direction cluster
        // extraction can rebind the Graphlib layout edge to a surviving cluster node. Mermaid
        // passes those graph endpoints to `insertEdge` for shape lookup and clipping.
        let layout_from = le.from.as_str();
        let layout_to = le.to.as_str();
        let tail_shape = ctx
            .nodes_by_id
            .get(layout_from)
            .and_then(|n| n.layout_shape.as_deref());
        let head_shape = ctx
            .nodes_by_id
            .get(layout_to)
            .and_then(|n| n.layout_shape.as_deref());
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
                    if is_rounded_intersect_shift_shape(tail_shape) {
                        start.x += 0.5;
                        start.y += 0.5;
                    }
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
                    if is_rounded_intersect_shift_shape(head_shape) {
                        end.x += 0.5;
                        end.y += 0.5;
                    }
                }

                points_after_intersect.clear();
                points_after_intersect.reserve(interior.len() + 2);
                points_after_intersect.push(start);
                points_after_intersect.extend(interior.iter().cloned());
                points_after_intersect.push(end);
            }
        }
    }

    scratch.tmp_points_c.clear();
    if let Some(tc) = le.to_cluster.as_deref() {
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
    if let Some(fc) = le.from_cluster.as_deref()
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
    let points_for_data_points = &scratch.tmp_points_b;

    let interpolate = if is_elk_layout {
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

    let is_cluster_edge = le.to_cluster.is_some() || le.from_cluster.is_some();
    // `positionEdgeLabel` consumes the polyline held by `points`; `fixCorners`, marker offsets,
    // and the D3 curve generator operate on the separate `lineData` copy below.
    let label_path_points = if ctx
        .model
        .edge_label_for_render(edge)
        .is_some_and(|label| !label.is_empty())
    {
        points_for_render.clone()
    } else {
        Vec::new()
    };

    if is_basis && is_cluster_edge {
        maybe_remove_redundant_cluster_run_point(points_for_render);
    }

    if points_for_render.len() == 1 {
        // Avoid emitting a degenerate `M x,y` path for clipped cluster-adjacent edges.
        points_for_render.clear();
        points_for_render.extend(scratch.local_points.iter().cloned());
    }

    // D3's `curveBasis` emits only a straight `M ... L ...` when there are exactly two points.
    // Mermaid's Dagre pipeline typically provides at least one intermediate point even for
    // straight-looking edges, resulting in `C` segments in the SVG `d`. To keep our output closer
    // to Mermaid's command sequence, re-insert a midpoint when our route collapses to two points
    // after clipping (but keep cluster-adjacent edges as-is: Mermaid uses straight segments there).
    if is_basis && points_for_render.len() == 2 && interpolate != "linear" && !is_cluster_edge {
        let a = &points_for_render[0];
        let b = &points_for_render[1];
        points_for_render.insert(
            1,
            crate::model::LayoutPoint {
                x: (a.x + b.x) / 2.0,
                y: (a.y + b.y) / 2.0,
            },
        );
    }

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

    let collapsed_terminal_stub = if is_rounded && ctx.compact_edge_corners {
        collapse_short_terminal_marker_stub(&mut line_data, edge.edge_type.as_deref())
    } else {
        false
    };

    let mut rounded_corner_mask = vec![true; line_data.len()];
    if is_rounded && ctx.compact_edge_corners && is_elk_layout && line_data.len() > 2 {
        if elk_endpoint_adapters.source && le.from_cluster.is_none() {
            rounded_corner_mask[1] = false;
        }
        if elk_endpoint_adapters.target && le.to_cluster.is_none() && !collapsed_terminal_stub {
            let target_anchor = line_data.len() - 2;
            rounded_corner_mask[target_anchor] = false;
        }
    }
    let rounded_corner_mask =
        (is_rounded && ctx.compact_edge_corners).then_some(rounded_corner_mask);

    let mut line_data = if is_rounded {
        rounded_line_with_marker_offsets_for_edge_type(&line_data, edge.edge_type.as_deref())
    } else {
        line_with_offset_for_edge_type(&line_data, edge.edge_type.as_deref())
    };
    maybe_collapse_degenerate_subgraph_edge_route(
        ctx,
        edge,
        points_for_data_points,
        &mut line_data,
    );

    let (d, raw_pb, skipped_bounds_for_viewbox) = curve_path_d_and_bounds(
        &line_data,
        interpolate,
        ctx.edge_corner_radius,
        ctx.compact_edge_corners,
        rounded_corner_mask.as_deref(),
    );
    let pb = svg_path_bounds_from_d(&d).or(raw_pb);
    let path_length = svg_path_length_from_d(&d);

    if trace_enabled {
        record_flowchart_edge_trace(FlowchartEdgeTraceInput {
            ctx,
            edge,
            layout_edge: le,
            origin_x,
            origin_y,
            base_points,
            points_after_intersect_for_trace: points_after_intersect_for_trace.as_deref(),
            points_for_render,
            points_for_data_points,
        });
    }

    scratch.json.clear();
    json_stringify_points_into(
        &mut scratch.json,
        points_for_data_points.as_slice(),
        &mut scratch.ryu,
    );
    let mut data_points_b64 =
        String::with_capacity(base64::encoded_len(scratch.json.len(), true).unwrap_or_default());
    base64::engine::general_purpose::STANDARD
        .encode_string(scratch.json.as_bytes(), &mut data_points_b64);

    Some(FlowchartEdgePathGeom {
        d,
        pb,
        data_points: points_for_data_points.clone(),
        data_points_b64,
        original_path_length: path_length,
        path_length,
        line_hop_applied: false,
        label_path_points,
        label_path_was_explicitly_updated: is_cluster_edge,
        emitted_d_for_label: None,
        bounds_skipped_for_viewbox: skipped_bounds_for_viewbox,
    })
}

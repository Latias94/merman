//! Flowchart edge path renderer.

use super::super::*;
use merman_render_flow::edge_svg::{FlowchartEdgeSvgEmission, FlowchartNeoEdgeMaskPlan};
use merman_render_flow::markers::FlowchartMarkerEmissionPlan;

#[allow(
    clippy::too_many_arguments,
    reason = "The streaming edge writer borrows independent prepared plans, shared scratch storage, and the geometry cache without allocating an aggregate context."
)]
pub(in crate::svg::parity::flowchart) fn render_flowchart_edge_path(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    marker_plan: &FlowchartMarkerEmissionPlan<'_>,
    adaptive_ink: Option<&'static str>,
    edge_ref: super::super::render_input::FlowchartRenderEdgeRef<'_>,
    origin_x: f64,
    origin_y: f64,
    scratch: &mut FlowchartEdgeDataPointsScratch,
    edge_cache: &FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
) -> crate::Result<()> {
    let key = edge_ref.key;
    let edge = edge_ref.edge;
    let edge_dom_id = ctx
        .document_ids()
        .edge(key)
        .ok_or_else(|| crate::Error::InvalidModel {
            message: format!(
                "missing prepared Flowchart DOM id for edge occurrence {}",
                key.semantic_index()
            ),
        })?;
    let trace_enabled = ctx.trace_edge_id.is_some_and(|id| id == edge.id.as_str());
    let data_look = ctx.compatibility.look;
    let hand_drawn = data_look.is_hand_drawn();
    let emitted_styles = ctx.edge_style_plan.edge_for(key)?;
    let animation = ctx.edge_style_plan.animation_for(key)?;
    let cached_geom = edge_cache
        .get(&key)
        .filter(|c| (c.origin_x - origin_x).abs() <= 1e-9 && (c.origin_y - origin_y).abs() <= 1e-9)
        .map(|c| &c.geom);

    // Trace collection recomputes the geometry before graph-wide postprocessing for diagnostics, but the emitted SVG
    // must still consume the post-processed cache. Enabling diagnostics must not alter rendering.
    let owned_geom = if cached_geom.is_none() || trace_enabled {
        if let Some(layout_edge) = ctx.layout_edges_by_key.get(&key) {
            ctx.work_meter.charge(layout_edge.points().len().max(2))?;
        }
        flowchart_compute_edge_path_geom(
            FlowchartEdgePathGeomRequest {
                ctx,
                key,
                edge,
                origin_x,
                origin_y,
                trace_enabled,
            },
            scratch,
            true,
        )?
    } else {
        None
    };
    let geom = if let Some(g) = cached_geom {
        g
    } else {
        let Some(g) = owned_geom.as_ref() else {
            return Ok(());
        };
        g
    };
    let d = geom.d.as_str();
    let data_points_b64 = scratch.data_points.encode(&geom.data_points);
    let neo_edge_mask = if data_look.is_neo()
        && !animation.is_active()
        && let Some(path_length) = flowchart_neo_edge_path_length(geom, edge)
    {
        Some(FlowchartNeoEdgeMaskPlan::prepare(
            path_length,
            edge,
            geom.line_hop_applied,
            ctx.work_meter,
        )?)
    } else {
        None
    };
    let d = geom.emitted_d_for_label.as_deref().unwrap_or(d);

    scratch.edge_class_attr.clear();
    css::write_flowchart_edge_class_attr(&mut scratch.edge_class_attr, edge, animation);
    if hand_drawn {
        scratch.edge_class_attr.push_str(" transition");
    }
    scratch.edge_marker_attrs.clear();
    marker_plan.push_edge_marker_attributes_for(&mut scratch.edge_marker_attrs, key)?;
    let source_style_svg_bytes = ctx.edge_style_plan.edge_source_style_svg_bytes_for(key)?;
    let path = FlowchartEdgeSvgEmission {
        edge_dom_id,
        edge,
        d,
        data_points_b64,
        data_look,
        hand_drawn,
        class_declarations: emitted_styles.emitted_edge_class_declarations(),
        class_attr: &scratch.edge_class_attr,
        marker_attrs: &scratch.edge_marker_attrs,
        default_edge_style: ctx.default_edge_style,
        adaptive_ink,
        neo_edge_mask,
        line_hop_applied: geom.line_hop_applied,
    };
    ctx.checkpoint_emit()?;
    path.append_to(out, source_style_svg_bytes, ctx.work_meter)?;
    super::super::step::write_edge_marks(out, ctx, key, edge, &geom.d)?;
    Ok(())
}

fn flowchart_neo_edge_path_length(
    geom: &FlowchartEdgePathGeom,
    edge: &crate::flowchart::FlowEdge,
) -> Option<f64> {
    if geom.line_hop_applied && flowchart_neo_mask_uses_dash_pairs(edge) {
        geom.pre_hop_dash_mask_length
    } else {
        geom.centerline_length()
    }
}

//! Adapter for Flowchart edge geometry and SVG paint.

use super::super::*;
use crate::edge_svg::{FlowchartEdgeSvgContext, FlowchartEdgeSvgGeometry};
use crate::markers::FlowchartMarkerEmissionPlan;

#[allow(
    clippy::too_many_arguments,
    reason = "The streaming edge writer borrows independent prepared plans, shared scratch storage, and the geometry cache without allocating an aggregate context."
)]
pub(in crate::svg) fn render_flowchart_edge_path(
    out: &mut impl crate::svg::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    marker_plan: &FlowchartMarkerEmissionPlan<'_>,
    fills: &super::super::edge_fill::FlowchartEdgeFillPlan,
    label_cutout: bool,
    edge_ref: super::super::render_input::FlowchartRenderEdgeRef<'_>,
    origin_x: f64,
    origin_y: f64,
    scratch: &mut FlowchartEdgeDataPointsScratch,
    edge_cache: &FxHashMap<crate::projection::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
) -> merman_render_core::Result<()> {
    let key = edge_ref.key;
    let edge = edge_ref.edge;
    let terminal = FlowchartEdgeSvgContext {
        document_ids: ctx.document_ids(),
        marker_plan,
        style_plan: ctx.edge_style_plan,
        compatibility: ctx.compatibility,
        steps: &ctx.steps,
        default_edge_style: ctx.default_edge_style,
        adaptive_ink: fills.ink(key),
        surface_layers: fills.layers(key),
        label_cutout,
        canonical: ctx.layout.route_contract.is_canonical_axis(),
        work_meter: ctx.work_meter,
    }
    .bind(edge_ref)?;
    let trace_enabled = ctx.trace_edge_id.is_some_and(|id| id == edge.id.as_str());
    let cached_geom = edge_cache
        .get(&key)
        .filter(|c| (c.origin_x - origin_x).abs() <= 1e-9 && (c.origin_y - origin_y).abs() <= 1e-9)
        .map(|c| &c.geom);

    // Trace collection recomputes the geometry before graph-wide postprocessing for diagnostics, but the emitted SVG
    // must still consume the post-processed cache. Enabling diagnostics must not alter rendering.
    let owned_geom = if cached_geom.is_none() || trace_enabled {
        if let Some(layout_edge) = ctx.layout_edges_by_key.get(&key) {
            ctx.work_meter.charge_work(
                layout_edge.points().len().max(2),
                super::super::paint::edge_paint_phase(ctx),
            )?;
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
    #[cfg(any(feature = "layout-grid", feature = "layout-layered"))]
    let after_paint_d = geom.after_paint_d.as_deref();
    #[cfg(not(any(feature = "layout-grid", feature = "layout-layered")))]
    let after_paint_d = None;
    terminal.append_with_route_overrides(
        out,
        FlowchartEdgeSvgGeometry {
            d: &geom.d,
            emitted_d_for_label: geom.emitted_d_for_label.as_deref(),
            data_points: &geom.data_points,
            neo_length: geom.neo_length,
            pre_hop_dash_mask_length: geom.pre_hop_dash_mask_length,
            line_hop_applied: geom.line_hop_applied,
        },
        after_paint_d,
        #[cfg(feature = "layout-layered")]
        geom.axes.as_deref(),
        &mut scratch.terminal,
    )
}

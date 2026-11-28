//! Bind Flowchart marker inputs from the prepared edge styles and layout paint.

use crate::markers::{FlowchartEdgeMarkers, FlowchartMarkerEmissionPlan};
use merman_theme_contract::v2::MarkerStyle;

use super::edge_fill::FlowchartEdgeFillPlan;
use super::{FlowchartHierarchyPlan, FlowchartRenderCtx, Result};
use crate::projection::FlowchartEdgeKey;

#[cfg(all(
    feature = "native-compact",
    feature = "diagram-flowchart",
    any(
        feature = "layout-dagre",
        feature = "layout-elk",
        feature = "layout-shoal"
    )
))]
pub(super) fn compact_classic_point_end(
    color: merman_render_core::paint_scene::SceneColor,
) -> merman_render_core::paint_scene::SceneMarker<'static> {
    use merman_render_core::paint_scene::{
        SceneBox, SceneMarker, SceneMarkerOrient, SceneMarkerUnits, ScenePaint, ScenePathCommand,
        SceneStroke,
    };

    use crate::markers::{CLASSIC_POINT_END_VIEW_BOX, flowchart_marker_shape_spec};

    use crate::markers::FlowchartMarkerBase;

    const WIDTH: f64 = CLASSIC_POINT_END_VIEW_BOX[0];
    const HEIGHT: f64 = CLASSIC_POINT_END_VIEW_BOX[1];
    const COMMANDS: [ScenePathCommand; 4] = [
        ScenePathCommand::MoveTo([0.0, 0.0]),
        ScenePathCommand::LineTo([WIDTH, HEIGHT / 2.0]),
        ScenePathCommand::LineTo([0.0, HEIGHT]),
        ScenePathCommand::Close,
    ];
    let spec = flowchart_marker_shape_spec(FlowchartMarkerBase::PointEnd, false);
    SceneMarker {
        view_box: Some(SceneBox {
            x: 0.0,
            y: 0.0,
            width: spec.viewport.view_box[0],
            height: spec.viewport.view_box[1],
        }),
        reference: spec.viewport.reference,
        width: spec.viewport.size[0],
        height: spec.viewport.size[1],
        units: SceneMarkerUnits::UserSpaceOnUse,
        orient: SceneMarkerOrient::Auto,
        round_stroke: false,
        commands: &COMMANDS,
        paint: ScenePaint {
            fill: Some(color),
            stroke: Some(SceneStroke {
                color,
                width: 1.0,
                dash: None,
            }),
        },
    }
}

#[cfg(any(feature = "canonical-edge-paint", test))]
pub(super) fn measure_marker(
    base: Option<crate::markers::FlowchartMarkerBase>,
    setback: Option<f64>,
    margin: bool,
) -> merman_render_core::route_types::Marker {
    use crate::markers::flowchart_marker_shape_spec;

    let Some(base) = base else {
        return Default::default();
    };
    let viewport = flowchart_marker_shape_spec(base, margin).viewport;
    merman_render_core::route_types::Marker {
        trim: setback.unwrap_or_default(),
        depth: viewport.size[0].max(viewport.rotation_radius()),
        width: viewport.size[1],
    }
}

pub(super) fn prepare_flowchart_markers<'a>(
    ctx: &'a FlowchartRenderCtx<'_>,
    hierarchy: &FlowchartHierarchyPlan<'_>,
    fill_plan: &FlowchartEdgeFillPlan,
) -> Result<FlowchartMarkerEmissionPlan<'a>> {
    let look = ctx.compatibility.look;
    let hand_drawn = look.is_hand_drawn();
    let neo = look.is_neo();
    let marker_style = ctx
        .native
        .map_or(MarkerStyle::Classic, |native| native.form.marker_style);
    let edges = hierarchy.ordered_edges().iter().copied().map(|edge| {
        bind_marker_path_sources_for(ctx, hierarchy, edge.key)?;
        let source_color = ctx
            .edge_style_plan
            .edge_for(edge.key)?
            .edge_marker_color(hand_drawn);
        let margin = ctx.edge_style_plan.marker_uses_margin_for(edge.key, neo)?;
        Ok(FlowchartEdgeMarkers {
            edge,
            source_color,
            adaptive_ink: fill_plan.ink(edge.key),
            surfaces: fill_plan.layers(edge.key),
            margin,
        })
    });
    FlowchartMarkerEmissionPlan::prepare(
        ctx.document_ids().scope(),
        ctx.diagram_type,
        ctx.security_level_loose,
        marker_style,
        ctx.work_meter,
        edges,
    )
}

pub(super) fn bind_marker_path_sources_for(
    ctx: &FlowchartRenderCtx<'_>,
    hierarchy: &FlowchartHierarchyPlan<'_>,
    key: FlowchartEdgeKey,
) -> Result<()> {
    ctx.edge_style_plan.bind_path_sources_for(
        key,
        ctx.compatibility.html_labels,
        ctx.uses_elk_adapter_dom || ctx.swimlane_direction.is_some(),
        hierarchy.edge_root(key)?.is_some(),
        ctx.work_meter,
    )
}

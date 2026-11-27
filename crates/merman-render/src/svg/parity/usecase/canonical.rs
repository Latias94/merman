use super::*;
use kurbo::{Rect, Vec2};
use merman_model::OperationPhase;
use merman_render_core::{
    mermaid_style::visit_parsed_style_declarations, resources::ResourceLimitPhase,
    rough_svg::constrained_rough_edge_path, svg_bounds::svg_path_bounds_from_d,
};
use merman_render_graph::usecase::{UsecaseEdgePlan, UsecasePaintedEdge};
use roughr::core::{RoughJsSeed, RoughMathRandom, RoughRandomness};
use serde_json::Value;

fn stroke_width(plan: &UsecaseEdgePlan, default_width: f64) -> f64 {
    let mut width = None;
    visit_parsed_style_declarations(&plan.source_style, |declaration| {
        if declaration.property() == "stroke-width" {
            width = declaration.svg_number_or_px();
        }
    });
    width.unwrap_or(default_width).max(0.0)
}

pub(super) fn prepare_edges(
    prepared: &UsecasePreparedArtifact,
    config: &Value,
    bounds: &mut Bounds,
    options: &SvgExecution<'_>,
) -> Result<Option<Vec<UsecasePaintedEdge>>> {
    let Some(routes) = prepared.canonical_routes() else {
        return Ok(None);
    };
    let measured = prepared.prepared.measured();
    let style = measured.canonical_route_style(config);
    let randomness = prepared.prepared.rough_edge_seed().map(|seed| {
        let randomness = options.rough_randomness(seed, "usecase-edge");
        let seed = randomness.seed().number();
        if seed == 0.0 {
            randomness
        } else {
            RoughRandomness::new(RoughJsSeed::new(0.0), RoughMathRandom::new(seed.to_bits()))
        }
    });
    let meter = options.work_meter();
    meter.charge(measured.edges.len())?;
    let mut painted = Vec::with_capacity(measured.edges.len());
    let mut path_bytes = 0usize;
    for (edge, plan) in prepared.layout().edges.iter().zip(&measured.edges) {
        options.checkpoint_emit()?;
        let width = stroke_width(plan, measured.connector_width());
        meter.charge(edge.points.len())?;
        let geometry = routes.paint(edge, Vec2::ZERO, width / 2.0, style, meter)?;
        let (path, path_bounds) = if let Some(randomness) = &randomness {
            meter.charge(geometry.path.d.len())?;
            let path =
                constrained_rough_edge_path(&geometry.path.d, geometry.markers, meter, randomness)?;
            let bounds = svg_path_bounds_from_d(&path).ok_or_else(|| Error::InvalidModel {
                message: format!("Usecase edge {} has no hand-drawn path bounds", edge.id),
            })?;
            (
                path,
                Rect::new(bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y),
            )
        } else {
            let bounds = geometry.path.bounds;
            (
                geometry.path.d,
                Rect::new(bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y),
            )
        };
        // These serialized paths are retained together until the root viewport is known.
        path_bytes = path_bytes.checked_add(path.len()).ok_or_else(|| {
            meter.terminate_svg_byte_count_overflow(
                ResourceLimitPhase::SvgOutput,
                OperationPhase::Emit,
            )
        })?;
        meter.preflight_svg_byte_count(
            path_bytes,
            ResourceLimitPhase::SvgOutput,
            OperationPhase::Emit,
        )?;
        let outset = 2.0 * width;
        let mut paint_bounds = path_bounds.inflate(outset, outset);
        let markers = [true, false].map(|start| measured.marker_for_edge(plan, start));
        let [start_ref, start_lead, end_lead, end_ref] = &geometry.marker_points;
        for (marker, reference, lead) in [
            (markers[0].as_ref(), start_ref, start_lead),
            (markers[1].as_ref(), end_ref, end_lead),
        ] {
            if let Some(marker) = marker {
                paint_bounds = paint_bounds.union(marker.paint_bounds(reference, lead));
            }
        }
        bounds.min_x = bounds.min_x.min(paint_bounds.x0);
        bounds.min_y = bounds.min_y.min(paint_bounds.y0);
        bounds.max_x = bounds.max_x.max(paint_bounds.x1);
        bounds.max_y = bounds.max_y.max(paint_bounds.y1);
        painted.push(UsecasePaintedEdge {
            path,
            points: geometry.data_points,
            markers,
        });
    }
    Ok(Some(painted))
}

use super::*;
use crate::usecase::{UsecaseEdgePlan, UsecaseMarker, canonical_route_style};
use kurbo::{Rect, Vec2};
use merman_render_core::svg_bounds::svg_path_bounds_from_d;
use merman_render_graph::usecase::UsecasePaintedEdge;
use roughr::core::{RoughJsSeed, RoughMathRandom, RoughRandomness};
use serde_json::Value;

fn stroke_width(plan: &UsecaseEdgePlan) -> f64 {
    plan.source_style
        .split(';')
        .filter_map(crate::mermaid_style::parse_style_declaration)
        .rfind(|declaration| declaration.property() == "stroke-width")
        .and_then(|declaration| declaration.svg_number_or_px())
        .unwrap_or(1.0)
        .max(0.0)
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
    let neo = config.get("look").and_then(Value::as_str) == Some("neo");
    let style = canonical_route_style(config);
    let randomness = prepared.prepared.hand_drawn_seed().map(|seed| {
        let randomness = options.rough_randomness(seed, "usecase-edge");
        let seed = randomness.seed().number();
        if seed == 0.0 {
            randomness
        } else {
            RoughRandomness::new(RoughJsSeed::new(0.0), RoughMathRandom::new(seed.to_bits()))
        }
    });
    options
        .work_meter()
        .charge(prepared.prepared.edges().len())?;
    let mut painted = Vec::with_capacity(prepared.prepared.edges().len());
    for (edge, plan) in prepared
        .layout()
        .edges
        .iter()
        .zip(prepared.prepared.edges())
    {
        let width = stroke_width(plan);
        options.checkpoint_emit()?;
        options.work_meter().charge(edge.points.len())?;
        let geometry = routes.paint(edge, Vec2::ZERO, width / 2.0, style, options.work_meter())?;
        let (path, path_bounds) = if let Some(randomness) = &randomness {
            options.work_meter().charge(geometry.path.d.len())?;
            let path = super::super::canonical_rough::constrained_rough_edge_path(
                &geometry.path.d,
                geometry.markers,
                options.work_meter(),
                randomness,
            )?;
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
        let outset = 2.0 * width;
        let mut paint_bounds = path_bounds.inflate(outset, outset);
        let markers = [true, false].map(|start| UsecaseMarker::for_edge(plan, start, neo));
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

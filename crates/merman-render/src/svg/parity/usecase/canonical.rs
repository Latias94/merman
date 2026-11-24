use super::*;
use crate::model::LayoutPoint;
use crate::usecase::{UsecaseEdgePlan, UsecaseMarker, canonical_route_style};
use kurbo::{Rect, Vec2};
use roughr::core::{RoughJsSeed, RoughMathRandom, RoughRandomness};
use serde_json::Value;

pub(super) struct PaintedEdge {
    pub(super) path: String,
    pub(super) points: Vec<LayoutPoint>,
    pub(super) markers: [Option<UsecaseMarker>; 2],
}

fn stroke_width(prepared: &UsecasePreparedArtifact, plan: &UsecaseEdgePlan) -> f64 {
    let source_width = plan
        .source_style
        .split(';')
        .filter_map(crate::mermaid_style::parse_style_declaration)
        .rfind(|declaration| declaration.property() == "stroke-width")
        .and_then(|declaration| declaration.svg_number_or_px());
    source_width
        .or_else(|| {
            prepared
                .theme_plan
                .edge(&plan.id)
                .and_then(|style| style.stroke_width)
        })
        .unwrap_or(1.0)
        .max(0.0)
}

pub(super) fn prepare_edges(
    prepared: &UsecasePreparedArtifact,
    config: &Value,
    bounds: &mut Bounds,
    options: &SvgExecution<'_>,
) -> Result<Option<Vec<PaintedEdge>>> {
    let Some(routes) = prepared.canonical_routes() else {
        return Ok(None);
    };
    let neo = config.get("look").and_then(Value::as_str) == Some("neo");
    let style = canonical_route_style(config);
    let randomness = (prepared.css_binding.look == "handDrawn").then(|| {
        let randomness =
            options.rough_randomness(prepared.css_binding.hand_drawn_seed, "usecase-edge");
        let seed = randomness.seed().number();
        if seed == 0.0 {
            randomness
        } else {
            RoughRandomness::new(RoughJsSeed::new(0.0), RoughMathRandom::new(seed.to_bits()))
        }
    });
    options.work_meter().charge(prepared.edges.len())?;
    let mut painted = Vec::with_capacity(prepared.edges.len());
    for (edge, plan) in prepared.layout.edges.iter().zip(&prepared.edges) {
        let width = stroke_width(prepared, plan);
        options.checkpoint_emit()?;
        options.work_meter().charge(edge.points.len())?;
        let geometry = routes.paint(edge, Vec2::ZERO, width / 2.0, style)?;
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
        let markers = [true, false]
            .map(|start| UsecaseMarker::for_edge(plan, start, neo, &prepared.theme_plan));
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
        painted.push(PaintedEdge {
            path,
            points: geometry.data_points,
            markers,
        });
    }
    Ok(Some(painted))
}

pub(super) fn write_marker(
    out: &mut String,
    diagram_id: SvgDiagramId<'_>,
    marker: &UsecaseMarker,
    start: bool,
    index: usize,
    style: Option<&UsecaseThemeStyle>,
    receipt: &UsecaseThemeReceipt,
) -> String {
    let name = marker.name();
    let side = if start { "Start" } else { "End" };
    let id = format!("{diagram_id}_usecase-{name}{side}-shoal-{index}");
    let (width, height) = marker.dimensions();
    let _ = write!(
        out,
        r#"<defs><marker id="{id}" class="marker {name} usecase" refX="{}" refY="{}" markerWidth="{}" markerHeight="{}" markerUnits="userSpaceOnUse" orient="auto" overflow="visible">"#,
        fmt(marker.ref_x),
        fmt(marker.ref_y()),
        fmt(width),
        fmt(height)
    );
    if let Some(radius) = marker.radius() {
        let _ = write!(
            out,
            r#"<circle cx="{}" cy="{}" r="{}""#,
            fmt(radius),
            fmt(radius),
            fmt(radius)
        );
    } else {
        let _ = write!(out, r#"<path d="{}""#, marker.path().to_svg());
    }
    let css = style.map(|style| style.css(false)).unwrap_or_default();
    let _ = write!(
        out,
        r#" style="stroke-width:{};stroke-dasharray:0;stroke-linecap:round;stroke-linejoin:round;{}"/></marker></defs>"#,
        fmt(marker.stroke_width),
        escape_attr(&css)
    );
    if let Some(style) = style {
        style.record_emitted(receipt);
    }
    format!("url(#{id})")
}

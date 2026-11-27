//! Flowchart v2 lean and trapezoid polygon shapes.

use std::fmt::Write as _;

use merman_render_core::{
    number_format::canonical_number as fmt_display,
    rough_svg::{
        closed_path_d_from_points as path_from_points,
        paths::{
            RoughGeometryTarget, roughjs_hachure_paths_for_svg_path,
            roughjs_prepare_hachure_paths_for_svg_path,
        },
    },
    svg_output::SvgOutput,
};

use super::{NodeBodyGeometry, NodeBodySvg, OptionalStyleAttr, escape_attr};

const FLOWCHART_POLYGON_HAND_DRAWN_ROUGHNESS: f32 = 0.7;
const FLOWCHART_POLYGON_HAND_DRAWN_FILL_WEIGHT: f32 = 1.5;
const FLOWCHART_POLYGON_HAND_DRAWN_HACHURE_GAP: f32 = 1.5;

pub fn write_lean(
    out: &mut impl SvgOutput,
    mut body: NodeBodySvg<'_>,
    pts: &[(f64, f64); 4],
    tx: f64,
    ty: f64,
) {
    if body.geometry.hand_drawn {
        let path_data = path_from_points(pts);
        if let Some((fill_d, stroke_d)) = body.timed_rough(|geometry| {
            roughjs_hachure_paths_for_svg_path(
                &path_data,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                FLOWCHART_POLYGON_HAND_DRAWN_FILL_WEIGHT,
                FLOWCHART_POLYGON_HAND_DRAWN_HACHURE_GAP,
                FLOWCHART_POLYGON_HAND_DRAWN_ROUGHNESS,
                geometry.work_meter,
                geometry.randomness,
            )
        }) {
            let _ = write!(
                out,
                r#"<g transform="translate({},{})" style="{}"><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g>"#,
                fmt_display(tx),
                fmt_display(ty),
                escape_attr(body.rough_group_style),
                escape_attr(&fill_d),
                escape_attr(body.fill_color),
                fmt_display(FLOWCHART_POLYGON_HAND_DRAWN_FILL_WEIGHT as f64),
                escape_attr(&stroke_d),
                escape_attr(body.stroke_color),
                fmt_display(body.geometry.stroke_width as f64),
                escape_attr(body.geometry.stroke_dasharray),
            );
            return;
        }
    }

    let mut points_attr = String::new();
    for (idx, (px, py)) in pts.iter().copied().enumerate() {
        if idx > 0 {
            points_attr.push(' ');
        }
        let _ = write!(&mut points_attr, "{},{}", fmt_display(px), fmt_display(py));
    }
    let _ = write!(
        out,
        r#"<polygon points="{}" class="label-container" transform="translate({},{})"{} />"#,
        points_attr,
        fmt_display(tx),
        fmt_display(ty),
        OptionalStyleAttr(body.style)
    );
}

/// Advances the rough stream for the caller's hand-drawn lean or trapezoid geometry.
pub fn advance_lean_hand_drawn(geometry: NodeBodyGeometry<'_>, points: &[(f64, f64); 4]) {
    let path = path_from_points(points);
    let _ = roughjs_prepare_hachure_paths_for_svg_path(
        RoughGeometryTarget::Query,
        &path,
        geometry.stroke_width,
        geometry.stroke_dasharray,
        FLOWCHART_POLYGON_HAND_DRAWN_FILL_WEIGHT,
        FLOWCHART_POLYGON_HAND_DRAWN_HACHURE_GAP,
        FLOWCHART_POLYGON_HAND_DRAWN_ROUGHNESS,
        geometry.work_meter,
        geometry.randomness,
    );
}

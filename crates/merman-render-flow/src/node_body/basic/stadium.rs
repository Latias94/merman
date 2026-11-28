//! Flowchart v2 stadium shape.

use merman_render_core::{
    rough_svg::{
        closed_path_d_from_points as path_from_points,
        paths::{RoughGeometryTarget, prepare_hachure_svg_path},
    },
    svg_output::SvgOutput,
};

use crate::node_geometry::StadiumGeometry;

use super::super::{NodeBodyGeometry, NodeBodySvg, escape_attr, fmt, prepare_node_path};

const FLOWCHART_STADIUM_HAND_DRAWN_ROUGHNESS: f32 = 0.7;
const FLOWCHART_STADIUM_HAND_DRAWN_FILL_WEIGHT: f32 = 1.5;
const FLOWCHART_STADIUM_HAND_DRAWN_HACHURE_GAP: f32 = 1.5;

fn silhouette(geometry: &NodeBodyGeometry<'_>, stadium: &StadiumGeometry) -> String {
    #[cfg(feature = "layout-shoal")]
    if geometry.compact_rough && geometry.hand_drawn {
        let radius = stadium.height / 2.0;
        let cap = (stadium.width / 2.0 - radius).max(0.0);
        return format!(
            "M{left} {top}H{right}A{r} {r} 0 0 1 {right} {r}H{left}A{r} {r} 0 0 1 {left} {top}Z",
            left = fmt(-cap),
            right = fmt(cap),
            top = fmt(-radius),
            r = fmt(radius),
        );
    }
    let _ = geometry;
    path_from_points(&stadium.outline())
}

pub fn write_stadium(
    out: &mut impl SvgOutput,
    mut body: NodeBodySvg<'_>,
    stadium: StadiumGeometry,
) {
    // Mermaid samples this outline from the measured label. Layout retains the theoretical size.
    let path_data = silhouette(&body.geometry, &stadium);
    let w = stadium.width;
    let h = stadium.height;
    let radius = h / 2.0;
    if body.geometry.hand_drawn {
        if let Some((fill_d, stroke_d)) = body.timed_rough(|geometry| {
            prepare_hachure_svg_path(
                RoughGeometryTarget::Svg,
                &path_data,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                geometry.hachure(
                    FLOWCHART_STADIUM_HAND_DRAWN_FILL_WEIGHT,
                    FLOWCHART_STADIUM_HAND_DRAWN_HACHURE_GAP,
                    FLOWCHART_STADIUM_HAND_DRAWN_ROUGHNESS,
                ),
                geometry.work_meter,
                geometry.randomness,
            )
            .and_then(|paths| geometry.write_rough_paths(paths))
        }) {
            if body.rough_fill_opacity.is_some() {
                let _ = write!(
                    out,
                    r#"<g class="basic label-container outer-path" style="{}">"#,
                    escape_attr(body.hand_drawn_group_style()),
                );
            } else {
                out.push_str(r#"<g class="basic label-container outer-path">"#);
            }
            body.write_hand_drawn_path_pair(
                out,
                &path_data,
                &fill_d,
                &stroke_d,
                f64::from(FLOWCHART_STADIUM_HAND_DRAWN_FILL_WEIGHT),
            );
            out.push_str("</g>");
            return;
        }
    } else if let Some((fill_d, stroke_d)) = body.solid_path_pair(&path_data) {
        out.push_str(r#"<g class="basic label-container outer-path">"#);
        let _ = write!(
            out,
            r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
            escape_attr(&fill_d),
            escape_attr(body.fill_color),
            escape_attr(body.style)
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
            escape_attr(&stroke_d),
            escape_attr(body.stroke_color),
            fmt(body.geometry.stroke_width as f64),
            escape_attr(body.geometry.stroke_dasharray),
            escape_attr(body.style)
        );
        out.push_str("</g>");
        return;
    }

    let _ = write!(
        out,
        r#"<rect class="basic label-container" style="{}" x="{}" y="{}" width="{}" height="{}" rx="{}" ry="{}"/>"#,
        escape_attr(body.style),
        fmt(-w / 2.0),
        fmt(-h / 2.0),
        fmt(w),
        fmt(h),
        fmt(radius),
        fmt(radius)
    );
}

pub fn advance_stadium_rough_geometry(geometry: NodeBodyGeometry<'_>, stadium: StadiumGeometry) {
    let path_data = silhouette(&geometry, &stadium);
    let target = RoughGeometryTarget::Query;
    if geometry.hand_drawn {
        let _ = prepare_hachure_svg_path(
            target,
            &path_data,
            geometry.stroke_width,
            geometry.stroke_dasharray,
            geometry.hachure(
                FLOWCHART_STADIUM_HAND_DRAWN_FILL_WEIGHT,
                FLOWCHART_STADIUM_HAND_DRAWN_HACHURE_GAP,
                FLOWCHART_STADIUM_HAND_DRAWN_ROUGHNESS,
            ),
            geometry.work_meter,
            geometry.randomness,
        );
    } else {
        let _ = prepare_node_path(&geometry, &path_data, target);
    }
}

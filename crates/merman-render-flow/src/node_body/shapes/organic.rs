//! Mermaid 11.16 bang and cloud path renderers.

use std::fmt::Write as _;

use merman_render_core::{
    number_format::canonical_number as fmt,
    rough_svg::paths::{
        RoughGeometryTarget, roughjs_hachure_paths_for_svg_path,
        roughjs_prepare_hachure_paths_for_svg_path,
    },
    svg_output::SvgOutput,
};

use crate::shapes::{OrganicShapeGeometry, RelativeArc, bang_geometry, cloud_geometry};

use super::super::{NodeBodyGeometry, NodeBodySvg, escape_attr};

const HAND_DRAWN_ROUGHNESS: f32 = 0.7;
const HAND_DRAWN_FILL_WEIGHT: f32 = 1.5;
const HAND_DRAWN_HACHURE_GAP: f32 = 1.5;

fn path_data(arcs: &[RelativeArc]) -> String {
    let mut path = String::from("M0 0\n");
    for arc in arcs {
        let _ = writeln!(
            path,
            "a{},{} {} {},{} {},{}",
            fmt(arc.rx),
            fmt(arc.ry),
            fmt(arc.x_axis_rotation_deg),
            u8::from(arc.large_arc),
            u8::from(arc.sweep),
            fmt(arc.dx),
            fmt(arc.dy),
        );
    }
    path.push_str("H0 V0 Z");
    path
}

fn render_organic_shape(
    out: &mut impl SvgOutput,
    mut svg: NodeBodySvg<'_>,
    geometry: &OrganicShapeGeometry,
) {
    let path = path_data(&geometry.arcs);
    if svg.geometry.hand_drawn
        && let Some((fill_d, stroke_d)) = svg.timed_rough(|geometry| {
            roughjs_hachure_paths_for_svg_path(
                &path,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                HAND_DRAWN_FILL_WEIGHT,
                HAND_DRAWN_HACHURE_GAP,
                HAND_DRAWN_ROUGHNESS,
                geometry.work_meter,
                geometry.randomness,
            )
        })
    {
        let _ = write!(
            out,
            r#"<g class="basic label-container" style="{}" transform="translate({},{})">"#,
            escape_attr(svg.hand_drawn_group_style()),
            fmt(geometry.translate_x),
            fmt(geometry.translate_y),
        );
        svg.write_hand_drawn_path_pair(
            out,
            &path,
            &fill_d,
            &stroke_d,
            f64::from(HAND_DRAWN_FILL_WEIGHT),
        );
        out.push_str("</g>");
        return;
    }

    let _ = write!(
        out,
        r#"<path class="basic label-container" style="{}" d="{}" transform="translate({},{})"/>"#,
        escape_attr(svg.style),
        escape_attr(&path),
        fmt(geometry.translate_x),
        fmt(geometry.translate_y),
    );
}

pub(super) fn write(
    out: &mut impl SvgOutput,
    svg: NodeBodySvg<'_>,
    cloud: bool,
    label_size: kurbo::Size,
    padding: f64,
    html_labels: bool,
    label_dx: &mut f64,
) {
    let geometry = organic_geometry(cloud, label_size, padding);
    if !html_labels {
        *label_dx = -label_size.width / 2.0;
    }
    render_organic_shape(out, svg, &geometry);
}

pub(super) fn advance_rough_geometry(
    geometry: NodeBodyGeometry<'_>,
    cloud: bool,
    label_size: kurbo::Size,
    padding: f64,
) {
    if !geometry.hand_drawn {
        return;
    }
    let outline = organic_geometry(cloud, label_size, padding);
    let path = path_data(&outline.arcs);
    let _ = roughjs_prepare_hachure_paths_for_svg_path(
        RoughGeometryTarget::Query,
        &path,
        geometry.stroke_width,
        geometry.stroke_dasharray,
        HAND_DRAWN_FILL_WEIGHT,
        HAND_DRAWN_HACHURE_GAP,
        HAND_DRAWN_ROUGHNESS,
        geometry.work_meter,
        geometry.randomness,
    );
}

fn organic_geometry(cloud: bool, label_size: kurbo::Size, padding: f64) -> OrganicShapeGeometry {
    if cloud {
        cloud_geometry(label_size.width, label_size.height, padding)
    } else {
        bang_geometry(label_size.width, label_size.height, padding)
    }
}

//! Flowchart v2 hexagon shape.

use merman_render_core::{
    rough_svg::{
        closed_path_d_from_points as path_from_points,
        paths::{RoughGeometryTarget, prepare_hachure_svg_path},
    },
    svg_output::SvgOutput,
};

use crate::node_geometry::HexagonGeometry;

use super::super::{NodeBodyGeometry, NodeBodySvg, OptionalStyleAttr, escape_attr, fmt};

const FLOWCHART_HEXAGON_HAND_DRAWN_ROUGHNESS: f32 = 0.7;
const FLOWCHART_HEXAGON_HAND_DRAWN_FILL_WEIGHT: f32 = 1.5;
const FLOWCHART_HEXAGON_HAND_DRAWN_HACHURE_GAP: f32 = 1.5;

pub fn write_hexagon(out: &mut impl SvgOutput, mut body: NodeBodySvg<'_>, neo: bool) {
    let (geometry, w, h, path_data) = prepare_hexagon_geometry(&body.geometry, neo);
    let rough_paths = if body.geometry.hand_drawn {
        body.timed_rough(|geometry| {
            prepare_hachure_svg_path(
                RoughGeometryTarget::Svg,
                &path_data,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                geometry.hachure(
                    FLOWCHART_HEXAGON_HAND_DRAWN_FILL_WEIGHT,
                    FLOWCHART_HEXAGON_HAND_DRAWN_HACHURE_GAP,
                    FLOWCHART_HEXAGON_HAND_DRAWN_ROUGHNESS,
                ),
                geometry.work_meter,
                geometry.randomness,
            )
            .and_then(|paths| geometry.write_rough_paths(paths))
        })
    } else {
        None
    };

    if let Some((fill_d, stroke_d)) = rough_paths {
        let _ = write!(
            out,
            r#"<g transform="translate({},{})" style="{}">"#,
            fmt(-w / 2.0),
            fmt(h / 2.0),
            escape_attr(body.hand_drawn_group_style())
        );
        body.write_hachure(
            out,
            &path_data,
            &fill_d,
            f64::from(FLOWCHART_HEXAGON_HAND_DRAWN_FILL_WEIGHT),
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
            escape_attr(&stroke_d),
            escape_attr(body.stroke_color),
            fmt(body.geometry.stroke_width as f64),
            escape_attr(body.geometry.stroke_dasharray),
            escape_attr(&body.grouped_child_style())
        );
        out.push_str("</g>");
    } else {
        let _ = write!(
            out,
            r#"<polygon points="{},{} {},{} {},{} {},{} {},{} {},{}" class="label-container" transform="translate({},{})"{} />"#,
            fmt(geometry.points[0].0),
            fmt(geometry.points[0].1),
            fmt(geometry.points[1].0),
            fmt(geometry.points[1].1),
            fmt(geometry.points[2].0),
            fmt(geometry.points[2].1),
            fmt(geometry.points[3].0),
            fmt(geometry.points[3].1),
            fmt(geometry.points[4].0),
            fmt(geometry.points[4].1),
            fmt(geometry.points[5].0),
            fmt(geometry.points[5].1),
            fmt(-w / 2.0),
            fmt(h / 2.0),
            OptionalStyleAttr(body.style)
        );
    }
}

fn prepare_hexagon_geometry(
    geometry: &NodeBodyGeometry<'_>,
    neo: bool,
) -> (HexagonGeometry, f64, f64, String) {
    let geometry = HexagonGeometry::from_bounds(geometry.width, geometry.height, neo);
    let w = geometry.width;
    let h = geometry.height;
    let path_data = path_from_points(&geometry.points);

    (geometry, w, h, path_data)
}

pub fn advance_hexagon_rough_geometry(geometry: NodeBodyGeometry<'_>, neo: bool) {
    if !geometry.hand_drawn {
        return;
    }
    let (_, _, _, path_data) = prepare_hexagon_geometry(&geometry, neo);
    let target = RoughGeometryTarget::Query;
    let _ = prepare_hachure_svg_path(
        target,
        &path_data,
        geometry.stroke_width,
        geometry.stroke_dasharray,
        geometry.hachure(
            FLOWCHART_HEXAGON_HAND_DRAWN_FILL_WEIGHT,
            FLOWCHART_HEXAGON_HAND_DRAWN_HACHURE_GAP,
            FLOWCHART_HEXAGON_HAND_DRAWN_ROUGHNESS,
        ),
        geometry.work_meter,
        geometry.randomness,
    );
}

//! Flowchart v2 odd shape.

use merman_render_core::{
    rough_svg::{closed_path_d_from_points as path_from_points, paths::RoughGeometryTarget},
    svg_output::SvgOutput,
};

use crate::node_geometry::OddGeometry;

use super::super::{
    NodeBodyGeometry, NodeBodySvg, OptionalStyleAttr, escape_attr, fmt, prepare_node_path,
};

pub fn write_odd(out: &mut impl SvgOutput, mut body: NodeBodySvg<'_>, label_dx: &mut f64) {
    let (geometry, dx, path_data) = prepare_odd_geometry(&body.geometry, label_dx);
    if let Some((fill_d, stroke_d)) = body.hand_drawn_path_pair(&path_data) {
        let _ = write!(
            out,
            r#"<g class="basic label-container outer-path" transform="translate({},0)" style="{}">"#,
            fmt(dx),
            escape_attr(body.rough_group_style),
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
            escape_attr(&fill_d),
            escape_attr(body.fill_color),
            escape_attr(&stroke_d),
            escape_attr(body.stroke_color),
            fmt(body.geometry.stroke_width as f64),
            escape_attr(body.geometry.stroke_dasharray),
        );
        out.push_str("</g>");
    } else if body.geometry.hand_drawn {
        let _ = write!(
            out,
            r#"<path d="{}" class="label-container outer-path" transform="translate({},0)" style="{}"/>"#,
            escape_attr(&path_data),
            fmt(dx),
            escape_attr(body.style),
        );
    } else if let Some((fill_d, stroke_d)) = body.solid_path_pair(&path_data) {
        let _ = write!(
            out,
            r#"<g class="basic label-container outer-path" transform="translate({},0)">"#,
            fmt(dx)
        );
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
    } else {
        let _ = write!(
            out,
            r#"<polygon points="{},{} {},{} {},{} {},{} {},{}" class="label-container outer-path" transform="translate({},{})"{} />"#,
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
            fmt(dx),
            fmt(0.0),
            OptionalStyleAttr(body.style)
        );
    }
}

fn prepare_odd_geometry(
    geometry: &NodeBodyGeometry<'_>,
    label_dx: &mut f64,
) -> (OddGeometry, f64, String) {
    let geometry = OddGeometry::from_bounds(geometry.width, geometry.height);
    let dx = geometry.shift_x;
    *label_dx = dx;
    let path_data = path_from_points(&geometry.points);

    (geometry, dx, path_data)
}

pub fn advance_odd_rough_geometry(geometry: NodeBodyGeometry<'_>, label_dx: &mut f64) {
    let (_, _, path_data) = prepare_odd_geometry(&geometry, label_dx);
    let _ = prepare_node_path(&geometry, &path_data, RoughGeometryTarget::Query);
}

use super::super::UsecaseMarker;
use super::{SvgId, fmt};
use merman_render_core::LayoutPoint;
use std::fmt::Write as _;

pub struct UsecasePaintedEdge {
    pub path: String,
    pub points: Vec<LayoutPoint>,
    pub markers: [Option<UsecaseMarker>; 2],
}

pub(super) fn write_marker(
    out: &mut String,
    diagram_id: SvgId<'_>,
    marker: &UsecaseMarker,
    start: bool,
    index: usize,
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
    let _ = write!(
        out,
        r#" style="stroke-width:{};stroke-dasharray:0;stroke-linecap:round;stroke-linejoin:round;"/></marker></defs>"#,
        fmt(marker.stroke_width)
    );
    format!("url(#{id})")
}

//! Flowchart v2 hourglass and notched rectangle bodies.

use std::fmt::Write as _;

use merman_render_core::{number_format::canonical_number as fmt, svg_output::SvgOutput};

use super::{
    NodeBodyGeometry, NodeBodySvg, OptionalStyleAttr, OuterPathBody, OuterPathTranslation,
};

pub fn write_hourglass(out: &mut impl SvgOutput, body: NodeBodySvg<'_>) {
    hourglass_body(&body.geometry).write(out, body);
}

pub fn write_notched_rectangle(out: &mut impl SvgOutput, width: f64, height: f64, style: &str) {
    let w = width.max(1.0);
    let h = height.max(1.0);
    let notch = 12.0;
    let pts: Vec<(f64, f64)> = vec![
        (notch, -h),
        (w, -h),
        (w, 0.0),
        (0.0, 0.0),
        (0.0, -h + notch),
        (notch, -h),
    ];
    let mut points_attr = String::new();
    for (idx, (px, py)) in pts.iter().copied().enumerate() {
        if idx > 0 {
            points_attr.push(' ');
        }
        let _ = write!(&mut points_attr, "{},{}", fmt(px), fmt(py));
    }
    let _ = write!(
        out,
        r#"<polygon points="{}" class="label-container" transform="translate({},{})"{} />"#,
        points_attr,
        fmt(-w / 2.0),
        fmt(h / 2.0),
        OptionalStyleAttr(style),
    );
}

fn hourglass_body(geometry: &NodeBodyGeometry<'_>) -> OuterPathBody {
    let w = geometry.width.max(30.0);
    let h = geometry.height.max(30.0);
    let pts: Vec<(f64, f64)> = vec![(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)];
    OuterPathBody::from_points(&pts, OuterPathTranslation::Xy(-w / 2.0, -h / 2.0))
}

pub fn advance_hourglass(geometry: NodeBodyGeometry<'_>) {
    hourglass_body(&geometry).advance_rough_geometry(geometry);
}

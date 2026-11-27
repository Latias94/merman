use super::super::{
    SvgDiagramId, SvgOutput, escape_attr_display, fmt, root_svg, scoped_svg_id, scoped_svg_url,
};
use crate::c4::C4ShoalRelation;
use crate::{Error, Result};
use base64::{display::Base64Display, engine::general_purpose::STANDARD};
use kurbo::Rect;

pub(super) fn open_shaft(
    out: &mut impl SvgOutput,
    route: &C4ShoalRelation,
    stroke: &str,
    stroke_width: f64,
    diagram_id: SvgDiagramId<'_>,
    viewport: root_svg::DiagramBounds,
    points: &mut Vec<u8>,
) -> Result<()> {
    let cutout = (!route.shaft_cutouts.is_empty()).then(|| {
        let id = format!("{}-label-cutout", route.id);
        let mut outer = Rect::new(
            viewport.min_x,
            viewport.min_y,
            viewport.min_x + viewport.width,
            viewport.min_y + viewport.height,
        );
        for &gap in &route.shaft_cutouts {
            outer = outer.union(gap);
        }
        // Knock out only this shaft. The boundary or canvas already behind it shows through.
        outer = outer.inflate(1.0, 1.0);
        let _ = write!(
            out,
            r#"<defs><clipPath id="{}"><path clip-rule="evenodd" d="M{} {}H{}V{}H{}Z"#,
            escape_attr_display(scoped_svg_id(diagram_id, &id)),
            fmt(outer.x0),
            fmt(outer.y0),
            fmt(outer.x1),
            fmt(outer.y1),
            fmt(outer.x0),
        );
        for &gap in &route.shaft_cutouts {
            let _ = write!(
                out,
                "M{} {}V{}H{}V{}Z",
                fmt(gap.x0),
                fmt(gap.y0),
                fmt(gap.y1),
                fmt(gap.x1),
                fmt(gap.y0),
            );
        }
        out.push_str(r#""/></clipPath></defs>"#);
        id
    });
    if cutout.is_some() {
        out.checkpoint()?;
    }
    points.clear();
    serde_json::to_writer(&mut *points, &route.points).map_err(|error| Error::InvalidModel {
        message: format!("c4: failed to encode canonical relation points: {error}"),
    })?;
    let points = Base64Display::new(points, &STANDARD);
    let _ = write!(
        out,
        r#"<path data-et="edge" data-id="{}" data-points="{points}" fill="none" stroke-width="{}" stroke="{}" d="{}""#,
        escape_attr_display(&route.id),
        fmt(stroke_width),
        escape_attr_display(stroke),
        escape_attr_display(&route.path),
    );
    if let Some(id) = cutout {
        let _ = write!(
            out,
            r#" clip-path="{}""#,
            escape_attr_display(scoped_svg_url(diagram_id, &id))
        );
    }
    out.checkpoint()
}

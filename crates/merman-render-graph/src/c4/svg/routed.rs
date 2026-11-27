use super::super::C4RoutedRelation;
use base64::{display::Base64Display, engine::general_purpose::STANDARD};
use kurbo::Rect;
use merman_render_core::{
    Error, Result,
    number_format::canonical_number as fmt,
    root_svg,
    svg_css::{SvgId, scoped_svg_id, scoped_svg_url},
    svg_output::SvgOutput,
    xml::escape_attr_display,
};

pub(super) fn open_shaft(
    out: &mut impl SvgOutput,
    route: &C4RoutedRelation,
    stroke: &str,
    stroke_width: f64,
    diagram_id: SvgId<'_>,
    clip: Option<(&str, root_svg::DiagramBounds)>,
    points: &mut Vec<u8>,
) -> Result<()> {
    let clip = clip.zip(route.shaft_cutout);
    if let Some(((id, viewport), gap)) = clip {
        let outer = Rect::new(
            viewport.min_x,
            viewport.min_y,
            viewport.min_x + viewport.width,
            viewport.min_y + viewport.height,
        )
        .union(gap)
        .inflate(1.0, 1.0);
        // The relation's text blocks share one ink box, so the hole never toggles in an overlap.
        let _ = write!(
            out,
            r#"<defs><clipPath id="{}"><path clip-rule="evenodd" d="M{} {}H{}V{}H{}ZM{} {}V{}H{}V{}Z"/></clipPath></defs>"#,
            escape_attr_display(scoped_svg_id(diagram_id, id)),
            fmt(outer.x0),
            fmt(outer.y0),
            fmt(outer.x1),
            fmt(outer.y1),
            fmt(outer.x0),
            fmt(gap.x0),
            fmt(gap.y0),
            fmt(gap.y1),
            fmt(gap.x1),
            fmt(gap.y0),
        );
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
    if let Some(((id, _), _)) = clip {
        let _ = write!(
            out,
            r#" clip-path="{}""#,
            escape_attr_display(scoped_svg_url(diagram_id, id))
        );
    }
    out.checkpoint()
}

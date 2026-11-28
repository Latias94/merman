use super::super::{
    C4RelLayout, C4RoutedRelation,
    routed_text::relation_label_footprint,
    theme::{literal_color, readable_opaque_background},
};
use base64::{display::Base64Display, engine::general_purpose::STANDARD};
use kurbo::{Rect, Vec2};
use merman_render_core::{
    Error, LayoutPoint, Result,
    number_format::canonical_number as fmt,
    root_svg,
    svg_css::{SvgId, scoped_svg_id, scoped_svg_url},
    svg_output::SvgOutput,
    xml::escape_attr_display,
};
use merman_theme_contract::v2::Color;

pub(super) struct ShaftPaint<'a> {
    pub(super) stroke: &'a str,
    pub(super) width: f64,
    pub(super) outline: Option<Color>,
}

pub(super) fn open_shaft(
    out: &mut impl SvgOutput,
    route: &C4RoutedRelation,
    paint: ShaftPaint<'_>,
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
    let clip_id = clip.map(|((id, _), _)| id);
    let width = if let Some(outline) = paint.outline {
        // Keep both paints inside the width that layout reserved, with the same caption gap.
        let _ = write!(
            out,
            r#"<path class="c4-relation-outline" pointer-events="none" fill="none" stroke-width="{}" stroke="{outline}" d="{}""#,
            fmt(paint.width),
            escape_attr_display(&route.path),
        );
        write_shaft_clip(out, diagram_id, clip_id);
        out.push_str("/>");
        out.checkpoint()?;
        paint.width - (paint.width / 2.0).min(0.65)
    } else {
        paint.width
    };
    points.clear();
    serde_json::to_writer(&mut *points, &route.points).map_err(|error| Error::InvalidModel {
        message: format!("c4: failed to encode canonical relation points: {error}"),
    })?;
    let points = Base64Display::new(points, &STANDARD);
    let _ = write!(
        out,
        r#"<path data-et="edge" data-id="{}" data-points="{points}" fill="none" stroke-width="{}" stroke="{}" d="{}""#,
        escape_attr_display(&route.id),
        fmt(width),
        escape_attr_display(paint.stroke),
        escape_attr_display(&route.path),
    );
    write_shaft_clip(out, diagram_id, clip_id);
    out.checkpoint()
}

fn write_shaft_clip(out: &mut impl SvgOutput, diagram_id: SvgId<'_>, id: Option<&str>) {
    if let Some(id) = id {
        let _ = write!(
            out,
            r#" clip-path="{}""#,
            escape_attr_display(scoped_svg_url(diagram_id, id))
        );
    }
}

pub(super) fn open_label(
    out: &mut impl SvgOutput,
    relation: &C4RelLayout,
    route: &C4RoutedRelation,
    origin: &LayoutPoint,
    background: Option<Color>,
    ink: &str,
) -> Result<()> {
    let _ = write!(
        out,
        r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate({}, {})""#,
        escape_attr_display(&route.id),
        fmt(origin.x),
        fmt(origin.y),
    );
    if let Some(background) = background
        && let Some(bounds) = route
            .shaft_cutout
            .map(|gap| gap - Vec2::new(origin.x, origin.y))
            .or_else(|| relation_label_footprint(relation, route.label_reference.as_deref()))
    {
        let ink = literal_color(ink);
        let fill = readable_opaque_background(background, ink);
        // A browser's substitute font can extend beyond the measured backplate.
        // Its glyphs keep the same backdrop color at their outline.
        if ink.is_some_and(|ink| ink.channels()[3] == u8::MAX) {
            let _ = write!(
                out,
                r#" stroke="{fill}" stroke-width="1.5" stroke-linejoin="round" paint-order="stroke""#,
            );
        }
        let _ = write!(
            out,
            r#"><rect class="c4-relation-label-background" x="{}" y="{}" width="{}" height="{}" rx="4" fill="{fill}" stroke="none"/>"#,
            fmt(bounds.x0),
            fmt(bounds.y0),
            fmt(bounds.width()),
            fmt(bounds.height()),
        );
    } else {
        out.push('>');
    }
    out.checkpoint()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relation_outline_shares_the_existing_caption_cutout() {
        let route = C4RoutedRelation {
            id: "c4-rel-0".into(),
            path: "M0 15H30".into(),
            points: vec![
                LayoutPoint { x: 0.0, y: 15.0 },
                LayoutPoint { x: 30.0, y: 15.0 },
            ],
            label_origin: Some(LayoutPoint { x: 15.0, y: 15.0 }),
            label_reference: None,
            shaft_cutout: Some(Rect::new(10.0, 12.0, 20.0, 18.0)),
        };
        let mut svg = "<svg>".to_owned();
        open_shaft(
            &mut svg,
            &route,
            ShaftPaint {
                stroke: "#94A3B8",
                width: 1.5,
                outline: Some(Color::rgb(20, 27, 37)),
            },
            SvgId::new("c4").unwrap(),
            Some((
                "caption",
                root_svg::DiagramBounds::from_view_box(0.0, 0.0, 30.0, 30.0),
            )),
            &mut Vec::new(),
        )
        .unwrap();
        svg.push_str("/></svg>");
        let document = roxmltree::Document::parse(&svg).unwrap();
        let clip = document
            .descendants()
            .find(|node| node.has_tag_name("clipPath"))
            .unwrap();
        assert_eq!(clip.attribute("id"), Some("c4-caption"));
        let hole = clip.first_element_child().unwrap();
        assert_eq!(hole.attribute("clip-rule"), Some("evenodd"));
        assert!(hole.attribute("d").unwrap().ends_with("M10 12V18H20V12Z"));
        let routes: Vec<_> = document
            .root_element()
            .children()
            .filter(|node| node.has_tag_name("path"))
            .collect();
        assert_eq!(routes.len(), 2);
        for path in routes {
            assert_eq!(path.attribute("clip-path"), Some("url(#c4-caption)"));
            assert_eq!(path.attribute("d"), Some(route.path.as_str()));
        }
    }
}

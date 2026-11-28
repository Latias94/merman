//! Rectangular cutouts in the coordinate space of an SVG shaft.

use std::fmt;

use kurbo::Rect;

use crate::{
    Result, number_format::canonical_number as fmt_number, resources::OperationWorkMeter,
    svg_output::SvgOutput, xml::escape_attr_display,
};

/// Tests a resolved label paint without assuming a backdrop for unresolved CSS values.
pub fn is_transparent_paint(paint: &str) -> bool {
    let paint = paint.trim();
    paint.eq_ignore_ascii_case("none")
        || paint.eq_ignore_ascii_case("transparent")
        || merman_model::css_color::parse(paint).is_some_and(|color| color.rgba.alpha == 0.0)
}

/// Writes a clip path inside the caller's definitions. The frame includes all of the paint
/// bounds (including attached markers) and the holes, with the requested outward margin.
/// Overlapping holes subtract their union, and their input order does not affect the output.
pub fn write_definition(
    out: &mut impl SvgOutput,
    id: impl fmt::Display,
    paint_bounds: Rect,
    outset: f64,
    holes: &[Rect],
    work_meter: &OperationWorkMeter,
) -> Result<()> {
    let has_area = |rect: &Rect| rect.is_finite() && rect.x0 < rect.x1 && rect.y0 < rect.y1;
    if holes.len() > 1 {
        work_meter
            .charge_emit_work(holes.len().saturating_mul(holes.len().ilog2() as usize + 1))?;
    }
    let gaps = holes.iter().copied().filter(has_area);
    let frame = gaps
        .clone()
        .fold(paint_bounds, |bounds, gap| bounds.union(gap))
        .inflate(outset, outset);
    let _ = write!(
        out,
        r#"<clipPath id="{}"><path clip-rule="evenodd" d=""#,
        escape_attr_display(id),
    );
    write_contour(out, frame);
    if holes.len() <= 1 {
        for gap in gaps {
            write_contour(out, gap);
        }
    } else {
        write_union(out, gaps.collect(), work_meter)?;
    }
    out.push_str(r#""/></clipPath>"#);
    out.checkpoint()
}

/// Writes a clip path in its own definitions element.
pub fn write_defs(
    out: &mut impl SvgOutput,
    id: impl fmt::Display,
    paint_bounds: Rect,
    outset: f64,
    holes: &[Rect],
    work_meter: &OperationWorkMeter,
) -> Result<()> {
    out.push_str("<defs>");
    write_definition(out, id, paint_bounds, outset, holes, work_meter)?;
    out.push_str("</defs>");
    out.checkpoint()
}

/// Writes a reference to a renderer-owned, fragment-safe cutout ID.
pub fn write_reference(out: &mut impl fmt::Write, id: impl fmt::Display) {
    let _ = write!(out, r#" clip-path="url(#{})""#, escape_attr_display(id));
}

fn write_contour(out: &mut impl SvgOutput, rect: Rect) {
    let (left, top, right, bottom) = (
        fmt_number(rect.x0),
        fmt_number(rect.y0),
        fmt_number(rect.x1),
        fmt_number(rect.y1),
    );
    let _ = write!(out, "M{left} {top}H{right}V{bottom}H{left}Z");
}

fn write_union(
    out: &mut impl SvgOutput,
    mut gaps: Vec<Rect>,
    work: &OperationWorkMeter,
) -> Result<()> {
    // Tile the union with disjoint rectangles. An overlap must never toggle the even-odd clip.
    let mut rows: Vec<_> = gaps.iter().flat_map(|gap| [gap.y0, gap.y1]).collect();
    rows.sort_unstable_by(f64::total_cmp);
    rows.dedup();
    gaps.sort_unstable_by(|a, b| a.x0.total_cmp(&b.x0));
    for row in rows.windows(2) {
        work.charge_emit_work(gaps.len())?;
        let (top, bottom) = (row[0], row[1]);
        let mut spans = gaps.iter().filter(|gap| gap.y0 < bottom && gap.y1 > top);
        let Some(first) = spans.next() else {
            continue;
        };
        let (mut left, mut right) = (first.x0, first.x1);
        for gap in spans {
            if gap.x0 > right {
                write_contour(out, Rect::new(left, top, right, bottom));
                left = gap.x0;
            }
            right = right.max(gap.x1);
        }
        write_contour(out, Rect::new(left, top, right, bottom));
        out.checkpoint()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::RenderResourcePolicy;
    use kurbo::{BezPath, Point, Shape};

    #[test]
    fn overlapping_cutouts_subtract_the_union_in_deterministic_order() {
        let mut holes = [
            Rect::new(5.0, 5.0, 20.0, 18.0),
            Rect::new(14.0, 10.0, 27.0, 22.0),
            Rect::new(35.0, 7.0, 38.0, 14.0),
        ];
        let render = |holes: &[Rect]| {
            let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
            let mut out = String::new();
            write_defs(
                &mut out,
                "cutout",
                Rect::new(0.0, 0.0, 30.0, 30.0),
                1.0,
                holes,
                &work,
            )
            .unwrap();
            out
        };
        let svg = render(&holes);
        holes.reverse();
        assert_eq!(render(&holes), svg);

        let doc = roxmltree::Document::parse(&svg).unwrap();
        let clip = doc.root_element().first_element_child().unwrap();
        let path = clip.first_element_child().unwrap();
        assert_eq!(path.attribute("clip-rule"), Some("evenodd"));
        let frame = BezPath::from_svg(path.attribute("d").unwrap()).unwrap();
        for point in [(8.0, 8.0), (16.0, 12.0), (22.0, 20.0), (36.0, 10.0)] {
            assert_eq!(frame.winding(Point::from(point)) % 2, 0, "{point:?}");
        }
        for point in [(0.0, 2.0), (23.0, 8.0), (29.0, 12.0), (37.0, 22.0)] {
            assert_ne!(frame.winding(Point::from(point)) % 2, 0, "{point:?}");
        }
    }
}

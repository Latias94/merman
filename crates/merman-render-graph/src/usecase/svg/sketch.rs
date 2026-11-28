use super::*;
use kurbo::PathEl;
use merman_render_core::{
    mermaid_style::visit_parsed_style_declarations,
    rough_svg::{
        RoughRandomness, ops_to_svg_path_d,
        paths::{
            RoughGeometryTarget, roughjs_prepare_hand_drawn_stroke_path_for_svg_path,
            svg_path_bounds_from_rough_ops,
        },
    },
};
use std::cell::Cell;

pub(super) struct UsecaseSketch<'a> {
    pub(super) work_meter: &'a OperationWorkMeter,
    pub(super) randomness: RoughRandomness,
    pub(super) bounds: Cell<Option<Rect>>,
}

pub(super) enum ShapeStyle<'a> {
    Local(&'a str),
    Inherited(&'a str),
}

impl UsecaseSketch<'_> {
    pub(super) fn write_shape(
        &self,
        out: &mut impl SvgOutput,
        class: &str,
        style: ShapeStyle<'_>,
        outline: &str,
        translation: Vec2,
        filled: bool,
    ) -> Result<()> {
        let meter = self.work_meter;
        // Keep a solid backing and sketch only the perimeter. Work scales with the outline's
        // segments even for a boundary around a large compound diagram.
        let operations = roughjs_prepare_hand_drawn_stroke_path_for_svg_path(
            RoughGeometryTarget::Svg,
            outline,
            0.7,
            meter,
            &self.randomness,
        );
        out.checkpoint()?;
        let operations = operations.ok_or_else(|| Error::InvalidModel {
            message: "failed to construct Usecase hand-drawn outline".into(),
        })?;
        meter.charge_emit_work(operations.ops.len())?;
        let (style, write_style) = match style {
            ShapeStyle::Local(style) => (style, true),
            ShapeStyle::Inherited(style) => (style, false),
        };
        if let Some(bounds) = svg_path_bounds_from_rough_ops(&operations) {
            let mut outset = 1.0;
            visit_parsed_style_declarations(style, |declaration| {
                if declaration.property() == "stroke-width"
                    && let Some(width) = declaration.svg_number_or_px()
                {
                    outset = width.max(0.0) / 2.0;
                }
            });
            let bounds = Rect::new(bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y)
                .inflate(outset, outset)
                + translation;
            self.bounds.set(Some(
                self.bounds.get().map_or(bounds, |old| old.union(bounds)),
            ));
        }
        let _ = write!(
            out,
            r#"<g class="{} usecase-sketch-shape""#,
            escape_attr(class),
        );
        if write_style {
            let _ = write!(out, r#" style="{}""#, escape_attr(style));
        }
        out.push('>');
        if filled {
            let _ = write!(out, r#"<path d="{outline}" stroke="none"/>"#);
        }
        let stroke = ops_to_svg_path_d(&operations);
        let _ = write!(
            out,
            r#"<path d="{stroke}" fill="none" stroke-linecap="round" stroke-linejoin="round"/></g>"#,
        );
        out.checkpoint()
    }
}

pub(super) fn outline_path(elements: impl IntoIterator<Item = PathEl>) -> String {
    let mut path = String::new();
    for element in elements {
        let _ = match element {
            PathEl::MoveTo(point) => write!(path, "M{} {}", fmt(point.x), fmt(point.y)),
            PathEl::LineTo(point) => write!(path, "L{} {}", fmt(point.x), fmt(point.y)),
            PathEl::QuadTo(control, end) => write!(
                path,
                "Q{} {} {} {}",
                fmt(control.x),
                fmt(control.y),
                fmt(end.x),
                fmt(end.y),
            ),
            PathEl::CurveTo(first, second, end) => write!(
                path,
                "C{} {} {} {} {} {}",
                fmt(first.x),
                fmt(first.y),
                fmt(second.x),
                fmt(second.y),
                fmt(end.x),
                fmt(end.y),
            ),
            PathEl::ClosePath => path.write_char('Z'),
        };
    }
    path
}

pub(super) fn ellipse_path(y: f64, rx: f64, ry: f64) -> String {
    let (y, left, right, rx, ry) = (fmt(y), fmt(-rx), fmt(rx), fmt(rx), fmt(ry));
    format!("M{right} {y}A{rx} {ry} 0 1 0 {left} {y}A{rx} {ry} 0 1 0 {right} {y}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_uses_canonical_svg_coordinates() {
        let outline = outline_path([
            PathEl::MoveTo((-0.0, 2.0 + 1e-8).into()),
            PathEl::LineTo((3.0 - 1e-8, 2.5).into()),
            PathEl::QuadTo((4.0, 3.0).into(), (5.0, 4.0).into()),
            PathEl::CurveTo((5.0, 5.0).into(), (4.25, 6.0).into(), (1e-10, 6.0).into()),
            PathEl::ClosePath,
        ]);
        assert_eq!(outline, "M0 2L3 2.5Q4 3 5 4C5 5 4.25 6 0 6Z");
    }
}

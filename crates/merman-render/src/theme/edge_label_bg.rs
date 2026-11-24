use std::fmt::{self, Write};

use super::{Color, canvas_svg::write_color};
use crate::number_format::canonical_number;

/// Paints a label's measured box in the edge coordinate space.
pub(crate) fn write_backplate(
    out: &mut impl Write,
    bounds: kurbo::Rect,
    fill: Color,
    pill_stroke: Option<Color>,
) -> fmt::Result {
    if fill.channels()[3] == 0 || bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return Ok(());
    }
    write!(
        out,
        r#"<rect class="merman-edge-label-background" x="{}" y="{}" width="{}" height="{}" "#,
        canonical_number(bounds.x0),
        canonical_number(bounds.y0),
        canonical_number(bounds.width()),
        canonical_number(bounds.height()),
    )?;
    write_color(out, "fill", fill, 1.0)?;
    if let Some(stroke) = pill_stroke {
        write!(
            out,
            r#" rx="{}" stroke-width="0.75" "#,
            canonical_number(bounds.height() / 2.0)
        )?;
        write_color(out, "stroke", stroke, 1.0)?;
    }
    out.write_str("/>")
}

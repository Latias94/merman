use std::fmt::Write;

use super::{Color, canvas_svg::write_color};

/// Assigns a repeatable print pattern to a categorical fill. Slot zero remains solid.
pub(crate) fn data_pattern(
    out: &mut impl Write,
    id: &str,
    ordinal: usize,
    fill: Color,
    ink: Color,
) {
    let id = crate::svg::escape_attr(id);
    let _ = write!(
        out,
        r#"<pattern id="{id}" width="8" height="8" patternUnits="userSpaceOnUse"><rect width="8" height="8" "#,
    );
    let _ = write_color(out, "fill", fill, 1.0);
    let _ = out.write_str("/>");
    match ordinal % 6 {
        0 => {}
        3 => {
            let _ = out.write_str(r#"<circle cx="4" cy="4" r="1" "#);
            let _ = write_color(out, "fill", ink, 0.5);
            let _ = out.write_str("/>");
        }
        slot => {
            let path = match slot {
                1 => "M-2 2L2 -2M0 8L8 0M6 10L10 6",
                2 => "M-2 6L2 10M0 0L8 8M6 -2L10 2",
                4 => "M0 4H8",
                _ => "M0 4H8M4 0V8",
            };
            let _ = write!(out, r#"<path d="{path}" fill="none" "#);
            let _ = write_color(out, "stroke", ink, 0.5);
            let _ = out.write_str("/>");
        }
    }
    let _ = out.write_str("</pattern>");
}

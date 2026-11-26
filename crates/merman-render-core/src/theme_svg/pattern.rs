use std::fmt::{self, Write};

use merman_theme_contract::v2::Color;

use super::write_color;

/// Assigns a repeatable print pattern to a categorical fill. Slot zero remains solid.
pub fn data_pattern(
    out: &mut impl Write,
    id: &str,
    ordinal: usize,
    fill: Color,
    ink: Color,
) -> fmt::Result {
    let id = crate::xml::escape_attr(id);
    write!(
        out,
        r#"<pattern id="{id}" width="8" height="8" patternUnits="userSpaceOnUse"><rect width="8" height="8" "#,
    )?;
    write_color(out, "fill", fill, 1.0)?;
    out.write_str("/>")?;
    match ordinal % 6 {
        0 => {}
        3 => {
            out.write_str(r#"<circle cx="4" cy="4" r="1" "#)?;
            write_color(out, "fill", ink, 0.5)?;
            out.write_str("/>")?;
        }
        slot => {
            let path = match slot {
                1 => "M-2 2L2 -2M0 8L8 0M6 10L10 6",
                2 => "M-2 6L2 10M0 0L8 8M6 -2L10 2",
                4 => "M0 4H8",
                _ => "M0 4H8M4 0V8",
            };
            write!(out, r#"<path d="{path}" fill="none" "#)?;
            write_color(out, "stroke", ink, 0.5)?;
            out.write_str("/>")?;
        }
    }
    out.write_str("</pattern>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct RejectAfterOneWrite(usize);

    impl Write for RejectAfterOneWrite {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            self.0 += 1;
            if self.0 > 1 {
                return Err(fmt::Error);
            }
            Ok(())
        }
    }

    #[test]
    fn data_pattern_stops_at_the_first_sink_error() {
        let mut out = RejectAfterOneWrite::default();
        let color = Color::rgb(0, 0, 0);
        assert!(data_pattern(&mut out, "pie-data-3", 3, color, color).is_err());
        assert_eq!(out.0, 2);
    }
}

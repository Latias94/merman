use std::fmt::{self, Write};

use merman_theme_contract::v2::{Color, EdgeLabelBackground};

use super::canvas::write_color;
use crate::number_format::canonical_number;

/// Resolves a literal SVG paint to the color visible behind a label.
pub fn literal_fill(paint: &str) -> Option<Color> {
    let paint = paint.trim();
    if paint.eq_ignore_ascii_case("none") || paint.eq_ignore_ascii_case("transparent") {
        return Some(Color::TRANSPARENT);
    }
    let color = paint.parse::<svgtypes::Color>().ok()?;
    Some(Color::rgb(color.red, color.green, color.blue).composite(
        Color::TRANSPARENT,
        f64::from(color.alpha) / f64::from(u8::MAX),
    ))
}

pub fn literal_opacity(value: &str) -> Option<f64> {
    let value = value.trim();
    let (number, scale) = value
        .strip_suffix('%')
        .map_or((value, 1.0), |percent| (percent, 0.01));
    let opacity = number.parse::<f64>().ok()? * scale;
    opacity.is_finite().then(|| opacity.clamp(0.0, 1.0))
}

/// Returns a high-contrast foreground hex color when `default_ink` fails WCAG AA (4.5:1)
/// against an opaque `backdrop` (for example, a dark or light styled container fill).
pub fn adaptive_text_ink(backdrop: Color, default_ink: Color) -> Option<&'static str> {
    if backdrop.channels()[3] != u8::MAX || default_ink.contrast(backdrop) >= 4.5 {
        return None;
    }
    const LIGHT_INK: Color = Color::rgb(255, 255, 255);
    const DARK_INK: Color = Color::rgb(15, 23, 42);
    Some(
        if LIGHT_INK.contrast(backdrop) >= DARK_INK.contrast(backdrop) {
            "#FFFFFF"
        } else {
            "#0F172A"
        },
    )
}

/// Paints a label's measured box in the edge coordinate space.
pub fn write_backplate(
    out: &mut impl Write,
    label_bounds: kurbo::Rect,
    mode: EdgeLabelBackground,
    fill: Color,
    border: Color,
) -> fmt::Result {
    let pill = mode == EdgeLabelBackground::Pill;
    // Auto samples an already painted surface. Repainting a translucent sample changes it.
    if mode == EdgeLabelBackground::Transparent
        || (mode == EdgeLabelBackground::Auto && fill.channels()[3] != u8::MAX)
        || (fill.channels()[3] == 0 && (!pill || border.channels()[3] == 0))
        || label_bounds.width() <= 0.0
        || label_bounds.height() <= 0.0
    {
        return Ok(());
    }
    let bounds = label_bounds.inflate(4.0, 2.0);
    let radius = canonical_number(if pill { bounds.height() / 2.0 } else { 4.0 });
    write!(
        out,
        r#"<rect class="merman-edge-label-background" x="{}" y="{}" width="{}" height="{}" rx="{radius}" ry="{radius}" "#,
        canonical_number(bounds.x0),
        canonical_number(bounds.y0),
        canonical_number(bounds.width()),
        canonical_number(bounds.height()),
    )?;
    write_color(out, "fill", fill, 1.0)?;
    if pill {
        out.write_str(r#" stroke-width="0.75" "#)?;
        write_color(out, "stroke", border, 1.0)?;
    }
    out.write_str("/>")
}

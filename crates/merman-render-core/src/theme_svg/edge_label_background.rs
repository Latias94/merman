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
    if paint.eq_ignore_ascii_case("rebeccapurple") {
        return Some(Color::rgb(0x66, 0x33, 0x99));
    }
    if let Ok(color) = paint.parse::<svgtypes::Color>() {
        return Some(Color::rgb(color.red, color.green, color.blue).composite(
            Color::TRANSPARENT,
            f64::from(color.alpha) / f64::from(u8::MAX),
        ));
    }
    parse_css_color4_rgb(paint)
}

fn parse_css_color4_rgb(paint: &str) -> Option<Color> {
    let open = paint.find('(')?;
    let name = &paint[..open];
    if !name.eq_ignore_ascii_case("rgb") && !name.eq_ignore_ascii_case("rgba") {
        return None;
    }
    let body = paint.get(open + 1..paint.len().checked_sub(1)?)?;
    if !paint.ends_with(')') {
        return None;
    }
    let body = body.trim();
    if body.is_empty() {
        return None;
    }

    let (rgb_tokens, alpha_str) = if body.contains(',') {
        if body.contains('/') {
            return None;
        }
        let mut parts = body.split(',').map(str::trim);
        let r = parts.next()?;
        let g = parts.next()?;
        let b = parts.next()?;
        let a = parts.next();
        if parts.next().is_some() {
            return None;
        }
        ([r, g, b], a)
    } else {
        let (channels_part, alpha_part) = match body.split_once('/') {
            Some((channels, alpha)) => {
                if alpha.contains('/') {
                    return None;
                }
                let alpha = alpha.trim();
                if alpha.is_empty() {
                    return None;
                }
                (channels.trim(), Some(alpha))
            }
            None => (body, None),
        };
        let mut parts = channels_part.split_ascii_whitespace();
        let r = parts.next()?;
        let g = parts.next()?;
        let b = parts.next()?;
        if parts.next().is_some() {
            return None;
        }
        ([r, g, b], alpha_part)
    };

    let r = parse_rgb_channel(rgb_tokens[0])?;
    let g = parse_rgb_channel(rgb_tokens[1])?;
    let b = parse_rgb_channel(rgb_tokens[2])?;
    let alpha = match alpha_str {
        Some(raw) => literal_opacity(raw)?,
        None => 1.0,
    };
    Some(Color::rgb(r, g, b).composite(Color::TRANSPARENT, alpha))
}

fn parse_rgb_channel(token: &str) -> Option<u8> {
    if let Some(percent) = token.strip_suffix('%') {
        let value = percent.trim().parse::<f64>().ok()?;
        value
            .is_finite()
            .then(|| (value * 255.0 / 100.0).round().clamp(0.0, 255.0) as u8)
    } else {
        let value = token.parse::<f64>().ok()?;
        value
            .is_finite()
            .then(|| value.round().clamp(0.0, 255.0) as u8)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_fill_supports_rebeccapurple_and_css_color_4_percent_alpha() {
        assert_eq!(
            literal_fill("rebeccapurple"),
            Some(Color::rgb(0x66, 0x33, 0x99))
        );
        assert_eq!(
            literal_fill("ReBeCcApUrPlE"),
            Some(Color::rgb(0x66, 0x33, 0x99))
        );
        assert_eq!(
            literal_fill("rgba(255, 0, 128, 50%)"),
            Some(Color::rgb(255, 0, 128).composite(Color::TRANSPARENT, 0.5))
        );
        assert_eq!(
            literal_fill("rgb(255, 0, 128, 100%)"),
            Some(Color::rgb(255, 0, 128))
        );
        assert_eq!(
            literal_fill("rgb(255 0 128 / 50%)"),
            Some(Color::rgb(255, 0, 128).composite(Color::TRANSPARENT, 0.5))
        );
        assert_eq!(
            literal_fill("rgba(100% 0% 50% / 25%)"),
            Some(Color::rgb(255, 0, 128).composite(Color::TRANSPARENT, 0.25))
        );
        assert_eq!(literal_fill("rgb(10 20 30 / 0%)"), Some(Color::TRANSPARENT));
        assert_eq!(literal_fill("rgb(10, 20, 30 / 50%)"), None);
        assert_eq!(literal_fill("rgb(10 20 30 / 50% / 10%)"), None);
        assert_eq!(literal_fill("rgb(10 20)"), None);
    }
}

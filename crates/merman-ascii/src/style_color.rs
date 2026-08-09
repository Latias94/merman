use crate::color::AsciiRgb;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CssColor {
    Rgb(AsciiRgb),
    Transparent,
}

pub(crate) fn parse_css_color(value: &str) -> Option<AsciiRgb> {
    match parse_css_color_value(value)? {
        CssColor::Rgb(color) => Some(color),
        CssColor::Transparent => None,
    }
}

pub(crate) fn parse_css_color_value(value: &str) -> Option<CssColor> {
    let value = value.trim().trim_end_matches(';').trim();
    if value.eq_ignore_ascii_case("transparent") || value.eq_ignore_ascii_case("none") {
        return Some(CssColor::Transparent);
    }
    if let Some(hex) = value.strip_prefix('#') {
        return parse_hex_color(hex).map(CssColor::Rgb);
    }
    parse_rgb_function(value)
        .or_else(|| parse_hsl_function(value))
        .or_else(|| parse_named_color(value).map(CssColor::Rgb))
}

pub(crate) fn parse_border_color(value: &str) -> Option<AsciiRgb> {
    parse_css_color(value).or_else(|| value.split_whitespace().rev().find_map(parse_css_color))
}

fn parse_hex_color(hex: &str) -> Option<AsciiRgb> {
    match hex.len() {
        3 => {
            let r = parse_hex_digit(hex.as_bytes()[0])?;
            let g = parse_hex_digit(hex.as_bytes()[1])?;
            let b = parse_hex_digit(hex.as_bytes()[2])?;
            Some(AsciiRgb::new(r * 17, g * 17, b * 17))
        }
        6 => {
            let rgb = u32::from_str_radix(hex, 16).ok()?;
            Some(AsciiRgb::from_hex24(rgb))
        }
        _ => None,
    }
}

fn parse_hex_digit(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}

fn parse_rgb_function(value: &str) -> Option<CssColor> {
    let (inner, min_components) = if let Some(inner) = strip_ascii_case_prefix(value, "rgb(") {
        (inner.strip_suffix(')')?, 3)
    } else if let Some(inner) = strip_ascii_case_prefix(value, "rgba(") {
        (inner.strip_suffix(')')?, 4)
    } else {
        return None;
    };

    let mut components = inner
        .split([',', ' '])
        .filter(|part| !part.trim().is_empty() && part.trim() != "/")
        .map(str::trim);

    let r = parse_rgb_component(components.next()?)?;
    let g = parse_rgb_component(components.next()?)?;
    let b = parse_rgb_component(components.next()?)?;
    let alpha_component = components.next();
    if min_components == 4 && alpha_component.is_none() {
        return None;
    }
    let alpha = match alpha_component {
        Some(value) => Some(parse_alpha(value)?),
        None => None,
    };
    match alpha {
        Some(0) => Some(CssColor::Transparent),
        Some(255) | None => Some(CssColor::Rgb(AsciiRgb::new(r, g, b))),
        Some(_) => None,
    }
}

fn parse_hsl_function(value: &str) -> Option<CssColor> {
    let (inner, min_components) = if let Some(inner) = strip_ascii_case_prefix(value, "hsl(") {
        (inner.strip_suffix(')')?, 3)
    } else if let Some(inner) = strip_ascii_case_prefix(value, "hsla(") {
        (inner.strip_suffix(')')?, 4)
    } else {
        return None;
    };

    let mut components = inner
        .split([',', ' '])
        .filter(|part| !part.trim().is_empty() && part.trim() != "/")
        .map(str::trim);

    let hue = parse_hue(components.next()?)?;
    let saturation = parse_percentage(components.next()?)?;
    let lightness = parse_percentage(components.next()?)?;
    let alpha_component = components.next();
    if min_components == 4 && alpha_component.is_none() {
        return None;
    }
    let alpha = match alpha_component {
        Some(value) => Some(parse_alpha(value)?),
        None => None,
    };
    match alpha {
        Some(0) => Some(CssColor::Transparent),
        Some(255) | None => Some(CssColor::Rgb(hsl_to_rgb(hue, saturation, lightness))),
        Some(_) => None,
    }
}

fn parse_hue(value: &str) -> Option<f32> {
    let value = value.trim();
    let degrees = if let Some(degrees) = value.strip_suffix("deg") {
        degrees.trim().parse::<f32>().ok()?
    } else if let Some(turns) = value.strip_suffix("turn") {
        turns.trim().parse::<f32>().ok()? * 360.0
    } else if let Some(radians) = value.strip_suffix("rad") {
        radians.trim().parse::<f32>().ok()? * 180.0 / std::f32::consts::PI
    } else {
        value.parse::<f32>().ok()?
    };
    degrees.is_finite().then_some(degrees.rem_euclid(360.0))
}

fn parse_percentage(value: &str) -> Option<f32> {
    let percent = value.strip_suffix('%')?.trim().parse::<f32>().ok()?;
    if !(0.0..=100.0).contains(&percent) {
        return None;
    }
    Some(percent / 100.0)
}

fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> AsciiRgb {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let hue_sector = hue / 60.0;
    let x = chroma * (1.0 - (hue_sector.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = if hue_sector < 1.0 {
        (chroma, x, 0.0)
    } else if hue_sector < 2.0 {
        (x, chroma, 0.0)
    } else if hue_sector < 3.0 {
        (0.0, chroma, x)
    } else if hue_sector < 4.0 {
        (0.0, x, chroma)
    } else if hue_sector < 5.0 {
        (x, 0.0, chroma)
    } else {
        (chroma, 0.0, x)
    };
    let m = lightness - chroma / 2.0;
    AsciiRgb::new(
        rgb_float_to_u8(r1 + m),
        rgb_float_to_u8(g1 + m),
        rgb_float_to_u8(b1 + m),
    )
}

fn rgb_float_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn parse_rgb_component(value: &str) -> Option<u8> {
    if value.ends_with('%') {
        return None;
    }
    value.parse::<u8>().ok()
}

fn parse_alpha(value: &str) -> Option<u8> {
    if let Some(percent) = value.strip_suffix('%') {
        let percent = percent.parse::<f32>().ok()?;
        if !(0.0..=100.0).contains(&percent) {
            return None;
        }
        return Some((percent * 255.0 / 100.0).round() as u8);
    }

    let alpha = value.parse::<f32>().ok()?;
    if !(0.0..=1.0).contains(&alpha) {
        return None;
    }
    Some((alpha * 255.0).round() as u8)
}

fn parse_named_color(value: &str) -> Option<AsciiRgb> {
    const NAMED_COLORS: [(&str, u32); 15] = [
        ("black", 0x000000),
        ("white", 0xffffff),
        ("red", 0xff0000),
        ("green", 0x008000),
        ("blue", 0x0000ff),
        ("yellow", 0xffff00),
        ("cyan", 0x00ffff),
        ("aqua", 0x00ffff),
        ("magenta", 0xff00ff),
        ("fuchsia", 0xff00ff),
        ("gray", 0x808080),
        ("grey", 0x808080),
        ("orange", 0xffa500),
        ("purple", 0x800080),
        ("lime", 0x00ff00),
    ];

    NAMED_COLORS
        .iter()
        .find(|(name, _)| value.eq_ignore_ascii_case(name))
        .map(|(_, rgb)| AsciiRgb::from_hex24(*rgb))
}

fn strip_ascii_case_prefix<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    let candidate = value.get(..prefix.len())?;
    candidate
        .eq_ignore_ascii_case(prefix)
        .then(|| &value[prefix.len()..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_named_and_opaque_rgb_colors() {
        assert_eq!(
            parse_css_color("#abc"),
            Some(AsciiRgb::from_hex24(0xaabbcc))
        );
        assert_eq!(
            parse_css_color("#112233"),
            Some(AsciiRgb::from_hex24(0x112233))
        );
        assert_eq!(
            parse_css_color("green"),
            Some(AsciiRgb::from_hex24(0x008000))
        );
        assert_eq!(
            parse_css_color("rgb(1, 2, 3)"),
            Some(AsciiRgb::new(1, 2, 3))
        );
        assert_eq!(
            parse_css_color("rgba(1, 2, 3, 1)"),
            Some(AsciiRgb::new(1, 2, 3))
        );
        assert_eq!(
            parse_css_color("hsl(120, 100%, 25%)"),
            Some(AsciiRgb::from_hex24(0x008000))
        );
        assert_eq!(
            parse_css_color("hsla(240, 100%, 50%, 1)"),
            Some(AsciiRgb::from_hex24(0x0000ff))
        );
        assert_eq!(
            parse_css_color("RGB(1, 2, 3)"),
            Some(AsciiRgb::new(1, 2, 3))
        );
    }

    #[test]
    fn treats_transparent_and_alpha_colors_as_non_drawable() {
        assert_eq!(
            parse_css_color_value("transparent"),
            Some(CssColor::Transparent)
        );
        assert_eq!(
            parse_css_color_value("rgba(1, 2, 3, 0)"),
            Some(CssColor::Transparent)
        );
        assert_eq!(
            parse_css_color_value("hsla(120, 100%, 25%, 0)"),
            Some(CssColor::Transparent)
        );
        assert_eq!(parse_css_color("rgba(1, 2, 3, 0.5)"), None);
        assert_eq!(parse_css_color("hsla(120, 100%, 25%, 0.5)"), None);
    }
}

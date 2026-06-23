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
    parse_rgb_function(value).or_else(|| parse_named_color(value).map(CssColor::Rgb))
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
    let lower = value.to_ascii_lowercase();
    let (prefix, min_components) = if lower.starts_with("rgb(") {
        ("rgb(", 3)
    } else if lower.starts_with("rgba(") {
        ("rgba(", 4)
    } else {
        return None;
    };
    if !value.ends_with(')') {
        return None;
    }

    let inner = &value[prefix.len()..value.len() - 1];
    let components = inner
        .split([',', ' '])
        .filter(|part| !part.trim().is_empty() && part.trim() != "/")
        .map(str::trim)
        .collect::<Vec<_>>();
    if components.len() < min_components {
        return None;
    }

    let r = parse_rgb_component(components[0])?;
    let g = parse_rgb_component(components[1])?;
    let b = parse_rgb_component(components[2])?;
    let alpha = match components.get(3) {
        Some(value) => Some(parse_alpha(value)?),
        None => None,
    };
    match alpha {
        Some(0) => Some(CssColor::Transparent),
        Some(255) | None => Some(CssColor::Rgb(AsciiRgb::new(r, g, b))),
        Some(_) => None,
    }
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
    match value.to_ascii_lowercase().as_str() {
        "black" => Some(AsciiRgb::from_hex24(0x000000)),
        "white" => Some(AsciiRgb::from_hex24(0xffffff)),
        "red" => Some(AsciiRgb::from_hex24(0xff0000)),
        "green" => Some(AsciiRgb::from_hex24(0x008000)),
        "blue" => Some(AsciiRgb::from_hex24(0x0000ff)),
        "yellow" => Some(AsciiRgb::from_hex24(0xffff00)),
        "cyan" | "aqua" => Some(AsciiRgb::from_hex24(0x00ffff)),
        "magenta" | "fuchsia" => Some(AsciiRgb::from_hex24(0xff00ff)),
        "gray" | "grey" => Some(AsciiRgb::from_hex24(0x808080)),
        "orange" => Some(AsciiRgb::from_hex24(0xffa500)),
        "purple" => Some(AsciiRgb::from_hex24(0x800080)),
        "lime" => Some(AsciiRgb::from_hex24(0x00ff00)),
        _ => None,
    }
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
        assert_eq!(parse_css_color("rgba(1, 2, 3, 0.5)"), None);
    }
}

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
    let rgba = merman_model::css_color::parse(value)?.rgba;
    match rgba.alpha {
        0.0 => Some(CssColor::Transparent),
        1.0 => {
            let [r, g, b, _] = rgba.to_rgba8();
            Some(CssColor::Rgb(AsciiRgb::new(r, g, b)))
        }
        _ => None,
    }
}

pub(crate) fn parse_border_color(value: &str) -> Option<AsciiRgb> {
    parse_css_color(value).or_else(|| value.split_whitespace().rev().find_map(parse_css_color))
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

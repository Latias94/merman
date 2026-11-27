//! Literal CSS color syntax shared by terminal and SVG paint consumers.
//!
//! Parsing follows the color formats accepted by Mermaid's pinned Khroma implementation.
//! Theme mutation and CSS serialization remain in `merman-markup`.

/// The syntax family which produced a parsed color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSourceFormat {
    Hex,
    Rgb,
    Hsl,
    Keyword,
    ConstructedRgb,
}

/// RGBA evidence in Khroma's native units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RgbaChannels {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

impl RgbaChannels {
    /// Quantizes CSS channels to straight 8-bit sRGB and alpha.
    pub fn to_rgba8(self) -> [u8; 4] {
        [self.red, self.green, self.blue, self.alpha * 255.0]
            .map(|channel| channel.round().clamp(0.0, 255.0) as u8)
    }
}

/// HSLA evidence in Khroma's native units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HslaChannels {
    pub hue_degrees: f64,
    pub saturation_percent: f64,
    pub lightness_percent: f64,
    pub alpha: f64,
}

/// Parsed color channels and the source spelling used by Mermaid theme serialization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParsedColor<'a> {
    pub preserved_color: &'a str,
    pub source_format: ColorSourceFormat,
    pub rgba: RgbaChannels,
    pub hsla: HslaChannels,
}

pub fn parse(input: &str) -> Option<ParsedColor<'_>> {
    parse_hex(input)
        .or_else(|| parse_rgb(input))
        .or_else(|| parse_hsl(input))
        .or_else(|| parse_keyword(input))
}

fn parse_hex(input: &str) -> Option<ParsedColor<'_>> {
    let hex = input.strip_prefix('#')?;
    if !matches!(hex.len(), 3 | 4 | 6 | 8) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    parse_hex_with_evidence(input, ColorSourceFormat::Hex)
}

fn parse_hex_with_evidence(
    preserved_color: &str,
    source_format: ColorSourceFormat,
) -> Option<ParsedColor<'_>> {
    let hex = preserved_color.strip_prefix('#')?;
    let short = hex.len() <= 4;
    let has_alpha = matches!(hex.len(), 4 | 8);
    let component = |index: usize| -> Option<u8> {
        if short {
            let value = u8::from_str_radix(&hex[index..index + 1], 16).ok()?;
            Some(value * 17)
        } else {
            u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).ok()
        }
    };
    let rgba = RgbaChannels {
        red: f64::from(component(0)?),
        green: f64::from(component(1)?),
        blue: f64::from(component(2)?),
        alpha: if has_alpha {
            f64::from(component(3)?) / 255.0
        } else {
            1.0
        },
    };
    Some(ParsedColor {
        preserved_color,
        source_format,
        rgba,
        hsla: rgb_to_hsl(rgba),
    })
}

fn parse_rgb(input: &str) -> Option<ParsedColor<'_>> {
    let body = function_body(input, "rgb", "rgba")?;
    let mut cursor = ComponentCursor::new(body);
    let red = cursor.number(false, false)?;
    let red_percent = cursor.take_char('%');
    let separator = cursor.component_separator()?;
    let green = cursor.number(false, false)?;
    let green_percent = cursor.take_char('%');
    if cursor.component_separator()? != separator {
        return None;
    }
    let blue = cursor.number(false, false)?;
    let blue_percent = cursor.take_char('%');
    let alpha = if cursor.alpha_separator(separator) {
        let value = cursor.number(true, false)?;
        let percent = cursor.take_char('%');
        Some(if percent { value / 100.0 } else { value })
    } else {
        None
    };
    cursor.finish()?;

    let rgba = RgbaChannels {
        red: clamp_rgb(if red_percent {
            (red / 100.0) * 255.0
        } else {
            red
        }),
        green: clamp_rgb(if green_percent {
            (green / 100.0) * 255.0
        } else {
            green
        }),
        blue: clamp_rgb(if blue_percent {
            (blue / 100.0) * 255.0
        } else {
            blue
        }),
        alpha: clamp_alpha(alpha.unwrap_or(1.0)),
    };
    Some(ParsedColor {
        preserved_color: input,
        source_format: ColorSourceFormat::Rgb,
        rgba,
        hsla: rgb_to_hsl(rgba),
    })
}

fn parse_hsl(input: &str) -> Option<ParsedColor<'_>> {
    let body = function_body(input, "hsl", "hsla")?;
    let mut cursor = ComponentCursor::new(body);
    let mut hue = cursor.number(false, true)?;
    match cursor.hue_unit() {
        Some(HueUnit::Grad) => hue *= 0.9,
        Some(HueUnit::Rad) => hue = hue * 180.0 / std::f64::consts::PI,
        Some(HueUnit::Turn) => hue *= 360.0,
        Some(HueUnit::Degrees) | None => {}
    }
    let separator = cursor.component_separator()?;
    let saturation = cursor.number(false, true)?;
    if !cursor.take_char('%') {
        return None;
    }
    if cursor.component_separator()? != separator {
        return None;
    }
    let lightness = cursor.number(false, true)?;
    if !cursor.take_char('%') {
        return None;
    }
    let alpha = if cursor.alpha_separator(separator) {
        let value = cursor.number(true, true)?;
        let percent = cursor.take_char('%');
        Some(if percent { value / 100.0 } else { value })
    } else {
        None
    };
    cursor.finish()?;

    let hsla = HslaChannels {
        hue_degrees: clamp_hue(hue),
        saturation_percent: clamp_percent(saturation),
        lightness_percent: clamp_percent(lightness),
        alpha: clamp_alpha(alpha.unwrap_or(1.0)),
    };
    Some(ParsedColor {
        preserved_color: input,
        source_format: ColorSourceFormat::Hsl,
        rgba: hsl_to_rgb(hsla),
        hsla,
    })
}

fn parse_keyword(input: &str) -> Option<ParsedColor<'_>> {
    let canonical = keyword_hex(&input.to_ascii_lowercase())?;
    parse_hex_with_evidence(canonical, ColorSourceFormat::Keyword)
}

fn function_body<'a>(input: &'a str, short: &str, long: &str) -> Option<&'a str> {
    let open = input.find('(')?;
    let name = &input[..open];
    if !name.eq_ignore_ascii_case(short) && !name.eq_ignore_ascii_case(long) {
        return None;
    }
    input
        .get(open + 1..input.len().checked_sub(1)?)
        .filter(|_| input.ends_with(')'))
}

struct ComponentCursor<'a> {
    source: &'a str,
    offset: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ComponentSeparator {
    Comma,
    Space,
}

#[derive(Debug, Clone, Copy)]
enum HueUnit {
    Degrees,
    Grad,
    Rad,
    Turn,
}

impl<'a> ComponentCursor<'a> {
    fn new(source: &'a str) -> Self {
        let mut cursor = Self { source, offset: 0 };
        cursor.skip_whitespace();
        cursor
    }

    fn number(&mut self, allow_plus: bool, allow_negative_exponent: bool) -> Option<f64> {
        let start = self.offset;
        let _ = self.take_char('-') || (allow_plus && self.take_char('+'));

        let integer_digits = self.take_digits();
        let fractional_digits = if self.take_char('.') {
            let digits = self.take_digits();
            if digits == 0 {
                return None;
            }
            digits
        } else {
            0
        };
        if integer_digits == 0 && fractional_digits == 0 {
            return None;
        }

        let exponent_start = self.offset;
        if self.take_char('e') || self.take_char('E') {
            if allow_negative_exponent {
                self.take_char('-');
            }
            if self.take_digits() == 0 {
                self.offset = exponent_start;
            }
        }
        self.source[start..self.offset].parse().ok()
    }

    fn component_separator(&mut self) -> Option<ComponentSeparator> {
        let before_whitespace = self.offset;
        self.skip_whitespace();
        let had_whitespace = self.offset > before_whitespace;
        if self.take_char(',') {
            self.skip_whitespace();
            Some(ComponentSeparator::Comma)
        } else if had_whitespace {
            Some(ComponentSeparator::Space)
        } else {
            None
        }
    }

    fn alpha_separator(&mut self, separator: ComponentSeparator) -> bool {
        let start = self.offset;
        self.skip_whitespace();
        let delimiter = match separator {
            ComponentSeparator::Comma => ',',
            ComponentSeparator::Space => '/',
        };
        if self.take_char(delimiter) {
            self.skip_whitespace();
            true
        } else {
            self.offset = start;
            false
        }
    }

    fn finish(&mut self) -> Option<()> {
        self.skip_whitespace();
        (self.offset == self.source.len()).then_some(())
    }

    fn take_digits(&mut self) -> usize {
        let start = self.offset;
        while self
            .source
            .as_bytes()
            .get(self.offset)
            .is_some_and(u8::is_ascii_digit)
        {
            self.offset += 1;
        }
        self.offset - start
    }

    fn take_char(&mut self, expected: char) -> bool {
        if self.source[self.offset..].starts_with(expected) {
            self.offset += expected.len_utf8();
            true
        } else {
            false
        }
    }

    fn hue_unit(&mut self) -> Option<HueUnit> {
        for (expected, unit) in [
            ("grad", HueUnit::Grad),
            ("turn", HueUnit::Turn),
            ("rad", HueUnit::Rad),
            ("deg", HueUnit::Degrees),
        ] {
            let Some(candidate) = self.source.get(self.offset..self.offset + expected.len()) else {
                continue;
            };
            if !candidate.eq_ignore_ascii_case(expected) {
                continue;
            }
            self.offset += expected.len();
            return Some(unit);
        }
        None
    }

    fn skip_whitespace(&mut self) {
        while let Some(character) = self.source[self.offset..].chars().next() {
            if !character.is_whitespace() {
                break;
            }
            self.offset += character.len_utf8();
        }
    }
}

pub fn rgb_to_hsl(rgba: RgbaChannels) -> HslaChannels {
    let red = rgba.red / 255.0;
    let green = rgba.green / 255.0;
    let blue = rgba.blue / 255.0;
    let max = red.max(green).max(blue);
    let min = red.min(green).min(blue);
    let lightness = (max + min) / 2.0;
    if max == min {
        return HslaChannels {
            hue_degrees: 0.0,
            saturation_percent: 0.0,
            lightness_percent: lightness * 100.0,
            alpha: rgba.alpha,
        };
    }
    let delta = max - min;
    let saturation = if lightness > 0.5 {
        delta / (2.0 - max - min)
    } else {
        delta / (max + min)
    };
    let hue = if max == red {
        ((green - blue) / delta + if green < blue { 6.0 } else { 0.0 }) * 60.0
    } else if max == green {
        ((blue - red) / delta + 2.0) * 60.0
    } else {
        ((red - green) / delta + 4.0) * 60.0
    };
    HslaChannels {
        hue_degrees: hue,
        saturation_percent: saturation * 100.0,
        lightness_percent: lightness * 100.0,
        alpha: rgba.alpha,
    }
}

pub fn hsl_to_rgb(hsla: HslaChannels) -> RgbaChannels {
    if hsla.saturation_percent == 0.0 {
        let channel = hsla.lightness_percent * 2.55;
        return RgbaChannels {
            red: channel,
            green: channel,
            blue: channel,
            alpha: hsla.alpha,
        };
    }
    let hue = clamp_hue(hsla.hue_degrees) / 360.0;
    let saturation = hsla.saturation_percent / 100.0;
    let lightness = hsla.lightness_percent / 100.0;
    let q = if lightness < 0.5 {
        lightness * (1.0 + saturation)
    } else {
        lightness + saturation - lightness * saturation
    };
    let p = 2.0 * lightness - q;
    RgbaChannels {
        red: hue_to_rgb(p, q, hue + 1.0 / 3.0) * 255.0,
        green: hue_to_rgb(p, q, hue) * 255.0,
        blue: hue_to_rgb(p, q, hue - 1.0 / 3.0) * 255.0,
        alpha: hsla.alpha,
    }
}

fn hue_to_rgb(p: f64, q: f64, mut hue: f64) -> f64 {
    if hue < 0.0 {
        hue += 1.0;
    }
    if hue > 1.0 {
        hue -= 1.0;
    }
    if hue < 1.0 / 6.0 {
        p + (q - p) * 6.0 * hue
    } else if hue < 0.5 {
        q
    } else if hue < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - hue) * 6.0
    } else {
        p
    }
}

pub fn clamp_rgb(value: f64) -> f64 {
    value.clamp(0.0, 255.0)
}

pub fn clamp_hue(value: f64) -> f64 {
    if (0.0..360.0).contains(&value) {
        value
    } else {
        value.rem_euclid(360.0)
    }
}

pub fn clamp_percent(value: f64) -> f64 {
    value.clamp(0.0, 100.0)
}

pub fn clamp_alpha(value: f64) -> f64 {
    value.clamp(0.0, 1.0)
}

fn keyword_hex(keyword: &str) -> Option<&'static str> {
    Some(match keyword {
        "aliceblue" => "#f0f8ff",
        "antiquewhite" => "#faebd7",
        "aqua" | "cyan" => "#00ffff",
        "aquamarine" => "#7fffd4",
        "azure" => "#f0ffff",
        "beige" => "#f5f5dc",
        "bisque" => "#ffe4c4",
        "black" => "#000000",
        "blanchedalmond" => "#ffebcd",
        "blue" => "#0000ff",
        "blueviolet" => "#8a2be2",
        "brown" => "#a52a2a",
        "burlywood" => "#deb887",
        "cadetblue" => "#5f9ea0",
        "chartreuse" => "#7fff00",
        "chocolate" => "#d2691e",
        "coral" => "#ff7f50",
        "cornflowerblue" => "#6495ed",
        "cornsilk" => "#fff8dc",
        "crimson" => "#dc143c",
        "darkblue" => "#00008b",
        "darkcyan" => "#008b8b",
        "darkgoldenrod" => "#b8860b",
        "darkgray" | "darkgrey" => "#a9a9a9",
        "darkgreen" => "#006400",
        "darkkhaki" => "#bdb76b",
        "darkmagenta" => "#8b008b",
        "darkolivegreen" => "#556b2f",
        "darkorange" => "#ff8c00",
        "darkorchid" => "#9932cc",
        "darkred" => "#8b0000",
        "darksalmon" => "#e9967a",
        "darkseagreen" => "#8fbc8f",
        "darkslateblue" => "#483d8b",
        "darkslategray" | "darkslategrey" => "#2f4f4f",
        "darkturquoise" => "#00ced1",
        "darkviolet" => "#9400d3",
        "deeppink" => "#ff1493",
        "deepskyblue" => "#00bfff",
        "dimgray" | "dimgrey" => "#696969",
        "dodgerblue" => "#1e90ff",
        "firebrick" => "#b22222",
        "floralwhite" => "#fffaf0",
        "forestgreen" => "#228b22",
        "fuchsia" | "magenta" => "#ff00ff",
        "gainsboro" => "#dcdcdc",
        "ghostwhite" => "#f8f8ff",
        "gold" => "#ffd700",
        "goldenrod" => "#daa520",
        "gray" | "grey" => "#808080",
        "green" => "#008000",
        "greenyellow" => "#adff2f",
        "honeydew" => "#f0fff0",
        "hotpink" => "#ff69b4",
        "indianred" => "#cd5c5c",
        "indigo" => "#4b0082",
        "ivory" => "#fffff0",
        "khaki" => "#f0e68c",
        "lavender" => "#e6e6fa",
        "lavenderblush" => "#fff0f5",
        "lawngreen" => "#7cfc00",
        "lemonchiffon" => "#fffacd",
        "lightblue" => "#add8e6",
        "lightcoral" => "#f08080",
        "lightcyan" => "#e0ffff",
        "lightgoldenrodyellow" => "#fafad2",
        "lightgray" | "lightgrey" => "#d3d3d3",
        "lightgreen" => "#90ee90",
        "lightpink" => "#ffb6c1",
        "lightsalmon" => "#ffa07a",
        "lightseagreen" => "#20b2aa",
        "lightskyblue" => "#87cefa",
        "lightslategray" | "lightslategrey" => "#778899",
        "lightsteelblue" => "#b0c4de",
        "lightyellow" => "#ffffe0",
        "lime" => "#00ff00",
        "limegreen" => "#32cd32",
        "linen" => "#faf0e6",
        "maroon" => "#800000",
        "mediumaquamarine" => "#66cdaa",
        "mediumblue" => "#0000cd",
        "mediumorchid" => "#ba55d3",
        "mediumpurple" => "#9370db",
        "mediumseagreen" => "#3cb371",
        "mediumslateblue" => "#7b68ee",
        "mediumspringgreen" => "#00fa9a",
        "mediumturquoise" => "#48d1cc",
        "mediumvioletred" => "#c71585",
        "midnightblue" => "#191970",
        "mintcream" => "#f5fffa",
        "mistyrose" => "#ffe4e1",
        "moccasin" => "#ffe4b5",
        "navajowhite" => "#ffdead",
        "navy" => "#000080",
        "oldlace" => "#fdf5e6",
        "olive" => "#808000",
        "olivedrab" => "#6b8e23",
        "orange" => "#ffa500",
        "orangered" => "#ff4500",
        "orchid" => "#da70d6",
        "palegoldenrod" => "#eee8aa",
        "palegreen" => "#98fb98",
        "paleturquoise" => "#afeeee",
        "palevioletred" => "#db7093",
        "papayawhip" => "#ffefd5",
        "peachpuff" => "#ffdab9",
        "peru" => "#cd853f",
        "pink" => "#ffc0cb",
        "plum" => "#dda0dd",
        "powderblue" => "#b0e0e6",
        "purple" => "#800080",
        "rebeccapurple" => "#663399",
        "red" => "#ff0000",
        "rosybrown" => "#bc8f8f",
        "royalblue" => "#4169e1",
        "saddlebrown" => "#8b4513",
        "salmon" => "#fa8072",
        "sandybrown" => "#f4a460",
        "seagreen" => "#2e8b57",
        "seashell" => "#fff5ee",
        "sienna" => "#a0522d",
        "silver" => "#c0c0c0",
        "skyblue" => "#87ceeb",
        "slateblue" => "#6a5acd",
        "slategray" | "slategrey" => "#708090",
        "snow" => "#fffafa",
        "springgreen" => "#00ff7f",
        "tan" => "#d2b48c",
        "teal" => "#008080",
        "thistle" => "#d8bfd8",
        "transparent" => "#00000000",
        "turquoise" => "#40e0d0",
        "violet" => "#ee82ee",
        "wheat" => "#f5deb3",
        "white" => "#ffffff",
        "whitesmoke" => "#f5f5f5",
        "yellow" => "#ffff00",
        "yellowgreen" => "#9acd32",
        _ => return None,
    })
}

use std::fmt;
#[cfg(any(feature = "compile", test))]
use std::ops::RangeInclusive;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

const CHANNEL_MAX: f64 = u8::MAX as f64;
#[cfg(any(feature = "compile", test))]
const GAMUT_SEARCH_STEPS: usize = 18;
#[cfg(any(feature = "compile", test))]
const LINEAR_RGB_GAMUT: RangeInclusive<f64> = -1e-4..=1.0001;
const LUMINANCE_WEIGHTS: [f64; 3] = [0.2126, 0.7152, 0.0722];
const SRGB_LINEAR_CUTOFF: f64 = 0.04045;

/// An 8-bit sRGB color with straight alpha.
///
/// Display and Serde use uppercase `#RRGGBB` for opaque colors and `#RRGGBBAA`
/// for colors with alpha below 255.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color([u8; 4]);

/// A color string outside the supported hexadecimal formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("expected a color in #RRGGBB or #RRGGBBAA format")]
pub struct InvalidColor;

impl Color {
    /// Transparent black.
    pub const TRANSPARENT: Self = Self([0, 0, 0, 0]);

    /// Creates an opaque sRGB color.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b, u8::MAX])
    }

    /// Returns the red, green, blue, and alpha channels in that order.
    pub const fn channels(self) -> [u8; 4] {
        self.0
    }

    /// Parses `#RRGGBB` or `#RRGGBBAA` with case-insensitive ASCII hex digits.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidColor`] for any other input.
    pub fn parse(input: &str) -> Result<Self, InvalidColor> {
        let hex = input.strip_prefix('#').ok_or(InvalidColor)?;
        if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(InvalidColor);
        }

        let bytes = u32::from_str_radix(hex, 16)
            .map_err(|_| InvalidColor)?
            .to_be_bytes();
        Ok(if hex.len() == 6 {
            let [_, r, g, b] = bytes;
            Self::rgb(r, g, b)
        } else {
            Self(bytes)
        })
    }

    /// Returns the WCAG 2.1 contrast ratio between opaque colors.
    ///
    /// Composite translucent colors over their backgrounds before measuring contrast.
    pub fn contrast(self, other: Self) -> f64 {
        let first = self.luminance();
        let second = other.luminance();
        luminance_ratio(first.max(second), first.min(second))
    }

    /// Composites this foreground color over `background` using straight alpha in sRGB.
    ///
    /// The foreground alpha is multiplied by `opacity` clamped to `0.0..=1.0`.
    pub fn composite(self, background: Self, opacity: f64) -> Self {
        let [r, g, b, a] = self.0;
        let [bg_r, bg_g, bg_b, bg_a] = background.0;
        let foreground_alpha = normalize_channel(a) * opacity.clamp(0.0, 1.0);
        let background_weight = normalize_channel(bg_a) * (1.0 - foreground_alpha);
        let alpha = foreground_alpha + background_weight;
        if alpha == 0.0 {
            return Self::TRANSPARENT;
        }

        let blend = |foreground, background| {
            let weighted = normalize_channel(foreground) * foreground_alpha
                + normalize_channel(background) * background_weight;
            quantize_channel(weighted / alpha)
        };
        Self([
            blend(r, bg_r),
            blend(g, bg_g),
            blend(b, bg_b),
            quantize_channel(alpha),
        ])
    }

    /// Returns the WCAG relative luminance of the RGB channels.
    pub(super) fn luminance(self) -> f64 {
        dot(LUMINANCE_WEIGHTS, self.linear_rgb())
    }

    /// Returns OKLCH lightness, chroma, and hue in degrees from the RGB channels.
    #[cfg(any(feature = "compile", test))]
    pub(super) fn oklch(self) -> [f64; 3] {
        let [lightness, a, b] = self.oklab();
        [
            lightness,
            a.hypot(b),
            b.atan2(a).to_degrees().rem_euclid(360.0),
        ]
    }

    /// Converts OKLCH to opaque sRGB by reducing chroma into gamut.
    #[cfg(any(feature = "compile", test))]
    pub(super) fn from_oklch(lightness: f64, chroma: f64, hue: f64) -> Self {
        let lightness = lightness.clamp(0.0, 1.0);
        let (sin_hue, cos_hue) = hue.to_radians().sin_cos();
        let (mut lower, mut upper) = (0.0, chroma.max(0.0));
        let mut best = oklab_to_linear_rgb([lightness, 0.0, 0.0]);
        for _ in 0..GAMUT_SEARCH_STEPS {
            let mid = (lower + upper) * 0.5;
            let candidate = oklab_to_linear_rgb([lightness, mid * cos_hue, mid * sin_hue]);
            if candidate
                .iter()
                .all(|channel| LINEAR_RGB_GAMUT.contains(channel))
            {
                best = candidate;
                lower = mid;
            } else {
                upper = mid;
            }
        }
        Self::from_linear_rgb(best)
    }

    /// Interpolates the RGB channels in OKLab and returns an opaque sRGB color.
    #[cfg(any(feature = "compile", test))]
    pub(super) fn mix_oklab(self, other: Self, amount: f64) -> Self {
        let [lightness, a, b] = self.oklab();
        let [other_lightness, other_a, other_b] = other.oklab();
        let lerp = |start, end| start + (end - start) * amount;
        Self::from_linear_rgb(oklab_to_linear_rgb([
            lerp(lightness, other_lightness),
            lerp(a, other_a),
            lerp(b, other_b),
        ]))
    }

    fn linear_rgb(self) -> [f64; 3] {
        let [r, g, b, _] = self.0;
        [r, g, b].map(|channel| srgb_to_linear(normalize_channel(channel)))
    }

    #[cfg(any(feature = "compile", test))]
    fn from_linear_rgb(rgb: [f64; 3]) -> Self {
        let [r, g, b] = rgb.map(|channel| quantize_channel(linear_to_srgb(channel)));
        Self::rgb(r, g, b)
    }

    #[cfg(any(feature = "compile", test))]
    fn oklab(self) -> [f64; 3] {
        let [r, g, b] = self.linear_rgb();
        // Björn Ottosson's linear sRGB to OKLab transform.
        let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
        let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
        let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
        [
            0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
            1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
            0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
        ]
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [r, g, b, a] = self.0;
        if a == u8::MAX {
            write!(f, "#{r:02X}{g:02X}{b:02X}")
        } else {
            write!(f, "#{r:02X}{g:02X}{b:02X}{a:02X}")
        }
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ColorVisitor;

        impl de::Visitor<'_> for ColorVisitor {
            type Value = Color;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a #RRGGBB or #RRGGBBAA color string")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Color, E> {
                Color::parse(value).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(ColorVisitor)
    }
}

pub(super) fn normalize_channel(channel: u8) -> f64 {
    f64::from(channel) / CHANNEL_MAX
}

fn quantize_channel(channel: f64) -> u8 {
    // Match Python's round at half-integer channel values.
    (channel * CHANNEL_MAX).round_ties_even() as u8
}

fn srgb_to_linear(channel: f64) -> f64 {
    if channel <= SRGB_LINEAR_CUTOFF {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

pub(super) fn dot([ax, ay, az]: [f64; 3], [bx, by, bz]: [f64; 3]) -> f64 {
    ax * bx + ay * by + az * bz
}

pub(super) fn relative_luminance(rgb: [f64; 3]) -> f64 {
    dot(LUMINANCE_WEIGHTS, rgb.map(srgb_to_linear))
}

pub(super) fn luminance_gradient(rgb: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|index| {
        let channel = rgb[index];
        let slope = if channel <= SRGB_LINEAR_CUTOFF {
            1.0 / 12.92
        } else {
            (2.4 / 1.055) * ((channel + 0.055) / 1.055).powf(1.4)
        };
        LUMINANCE_WEIGHTS[index] * slope
    })
}

pub(super) fn luminance_ratio(first: f64, second: f64) -> f64 {
    (first + 0.05) / (second + 0.05)
}

#[cfg(any(feature = "compile", test))]
fn linear_to_srgb(channel: f64) -> f64 {
    let channel = channel.clamp(0.0, 1.0);
    if channel <= 0.0031308 {
        12.92 * channel
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(any(feature = "compile", test))]
fn oklab_to_linear_rgb([lightness, a, b]: [f64; 3]) -> [f64; 3] {
    let l = (lightness + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m = (lightness - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s = (lightness - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    [
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    ]
}

#[cfg(test)]
mod tests {
    use super::{Color, InvalidColor};

    #[test]
    fn parse_rejects_non_hex_syntax_at_valid_lengths() {
        for input in ["#+12345", "#12é45", "#12345 ", "1234567"] {
            assert_eq!(Color::parse(input), Err(InvalidColor));
        }
    }

    #[test]
    fn oklab_interpolation_and_oklch_gamut_mapping_match_reference() {
        let red = Color::rgb(255, 0, 0);
        let blue = Color::rgb(0, 0, 255);
        assert_eq!(red.mix_oklab(blue, 0.5), Color::rgb(140, 83, 162));
        assert_eq!(Color::from_oklch(0.65, 0.4, 250.0), Color::rgb(0, 146, 249));
    }

    #[test]
    fn composite_uses_both_alphas_and_rounds_ties_to_even() {
        let red = Color::parse("#FF000080").unwrap();
        let blue = Color::parse("#0000FF80").unwrap();
        assert_eq!(red.composite(blue, 0.5).channels(), [102, 0, 153, 160]);
        assert_eq!(
            Color::rgb(1, 3, 5).composite(Color::rgb(0, 0, 0), 0.5),
            Color::rgb(0, 2, 2)
        );
    }
}

//! Source-backed color operations used by Mermaid themes and renderers.
//!
//! The behavior in this module mirrors Khroma 2.1.0, the color package pinned by Mermaid
//! 11.16.1. Parsing is intentionally implemented as a small structured parser rather than a
//! regular expression so every accepted component and separator has an explicit owner.

use merman_model::css_color::{
    self, clamp_alpha, clamp_hue, clamp_percent, clamp_rgb, hsl_to_rgb, rgb_to_hsl,
};
pub use merman_model::css_color::{ColorSourceFormat, HslaChannels, RgbaChannels};
use ryu_js::Buffer;
use std::error::Error as StdError;
use std::fmt;

/// A Khroma-compatible color operation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ColorError {
    /// The input is not one of the color formats accepted by Khroma 2.1.0.
    UnsupportedFormat { input: String },
    /// A single adjustment attempted to modify RGB and HSL channels together.
    MixedColorSpaces,
}

impl fmt::Display for ColorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFormat { input } => {
                write!(formatter, "Unsupported color format: \"{input}\"")
            }
            Self::MixedColorSpaces => {
                formatter.write_str("Cannot change both RGB and HSL channels at the same time")
            }
        }
    }
}

impl StdError for ColorError {}

/// A typed color channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorChannel {
    Red,
    Green,
    Blue,
    Hue,
    Saturation,
    Lightness,
    Alpha,
}

/// A typed equivalent of the channel object passed to `khroma.adjust`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ColorAdjustment {
    pub red: Option<f64>,
    pub green: Option<f64>,
    pub blue: Option<f64>,
    pub hue: Option<f64>,
    pub saturation: Option<f64>,
    pub lightness: Option<f64>,
    pub alpha: Option<f64>,
}

impl ColorAdjustment {
    pub const fn rgb(red: f64, green: f64, blue: f64) -> Self {
        Self {
            red: Some(red),
            green: Some(green),
            blue: Some(blue),
            hue: None,
            saturation: None,
            lightness: None,
            alpha: None,
        }
    }

    pub const fn hsl(hue: f64, saturation: f64, lightness: f64) -> Self {
        Self {
            red: None,
            green: None,
            blue: None,
            hue: Some(hue),
            saturation: Some(saturation),
            lightness: Some(lightness),
            alpha: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColorSpace {
    Rgb,
    Hsl,
}

/// Parsed Khroma color state, including source and mutation evidence.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeColor {
    raw_input: Option<String>,
    preserved_color: Option<String>,
    source_format: ColorSourceFormat,
    source_space: ColorSpace,
    mutation_space: Option<ColorSpace>,
    rgba: RgbaChannels,
    hsla: HslaChannels,
    changed: bool,
}

impl ThemeColor {
    /// Parse a color accepted by Khroma 2.1.0.
    pub fn parse(input: &str) -> Result<Self, ColorError> {
        let color = css_color::parse(input).ok_or_else(|| ColorError::UnsupportedFormat {
            input: input.to_string(),
        })?;
        Ok(Self {
            raw_input: Some(input.to_string()),
            preserved_color: Some(color.preserved_color.to_string()),
            source_format: color.source_format,
            source_space: if color.source_format == ColorSourceFormat::Hsl {
                ColorSpace::Hsl
            } else {
                ColorSpace::Rgb
            },
            mutation_space: None,
            rgba: color.rgba,
            hsla: color.hsla,
            changed: false,
        })
    }

    /// Original caller input. Constructed colors have no raw input.
    pub fn raw(&self) -> Option<&str> {
        self.raw_input.as_deref()
    }

    pub const fn source_format(&self) -> ColorSourceFormat {
        self.source_format
    }

    pub const fn rgba_channels(&self) -> RgbaChannels {
        self.rgba
    }

    pub const fn hsla_channels(&self) -> HslaChannels {
        self.hsla
    }

    pub const fn changed(&self) -> bool {
        self.changed
    }

    /// Read a channel rounded the same way as `khroma.channel`.
    pub fn channel(&self, channel: ColorChannel) -> f64 {
        round_1e10(self.channel_unrounded(channel))
    }

    /// Serialize according to Khroma's raw-preservation and changed-space rules.
    pub fn stringify(&self) -> String {
        if !self.changed
            && let Some(color) = &self.preserved_color
        {
            return color.clone();
        }

        let output_space = self.mutation_space.unwrap_or(self.source_space);
        if output_space == ColorSpace::Hsl {
            return stringify_hsl(self.hsla);
        }

        if self.rgba.alpha < 1.0
            || !is_integer(self.rgba.red)
            || !is_integer(self.rgba.green)
            || !is_integer(self.rgba.blue)
        {
            stringify_rgb(self.rgba)
        } else {
            stringify_hex(self.rgba)
        }
    }

    fn constructed_rgba(rgba: RgbaChannels) -> Self {
        let hsla = rgb_to_hsl(rgba);
        Self {
            raw_input: None,
            preserved_color: None,
            source_format: ColorSourceFormat::ConstructedRgb,
            source_space: ColorSpace::Rgb,
            mutation_space: None,
            rgba,
            hsla,
            changed: false,
        }
    }

    fn channel_unrounded(&self, channel: ColorChannel) -> f64 {
        match channel {
            ColorChannel::Red => self.rgba.red,
            ColorChannel::Green => self.rgba.green,
            ColorChannel::Blue => self.rgba.blue,
            ColorChannel::Hue => self.hsla.hue_degrees,
            ColorChannel::Saturation => self.hsla.saturation_percent,
            ColorChannel::Lightness => self.hsla.lightness_percent,
            ColorChannel::Alpha => self.rgba.alpha,
        }
    }

    fn set_channel(&mut self, channel: ColorChannel, value: f64) -> Result<(), ColorError> {
        let value = clamp_channel(channel, value);
        match channel {
            ColorChannel::Red | ColorChannel::Green | ColorChannel::Blue => {
                self.select_mutation_space(ColorSpace::Rgb)?;
                match channel {
                    ColorChannel::Red => self.rgba.red = value,
                    ColorChannel::Green => self.rgba.green = value,
                    ColorChannel::Blue => self.rgba.blue = value,
                    _ => unreachable!(),
                }
                self.hsla = rgb_to_hsl(self.rgba);
            }
            ColorChannel::Hue | ColorChannel::Saturation | ColorChannel::Lightness => {
                self.select_mutation_space(ColorSpace::Hsl)?;
                match channel {
                    ColorChannel::Hue => self.hsla.hue_degrees = value,
                    ColorChannel::Saturation => self.hsla.saturation_percent = value,
                    ColorChannel::Lightness => self.hsla.lightness_percent = value,
                    _ => unreachable!(),
                }
                self.rgba = hsl_to_rgb(self.hsla);
            }
            ColorChannel::Alpha => {
                self.rgba.alpha = value;
                self.hsla.alpha = value;
            }
        }
        self.changed = true;
        Ok(())
    }

    fn adjust_channel(&mut self, channel: ColorChannel, amount: f64) -> Result<(), ColorError> {
        let current = self.channel_unrounded(channel);
        let next = clamp_channel(channel, current + amount);
        if current != next {
            self.set_channel(channel, next)?;
        }
        Ok(())
    }

    fn select_mutation_space(&mut self, space: ColorSpace) -> Result<(), ColorError> {
        if let Some(selected) = self.mutation_space
            && selected != space
        {
            return Err(ColorError::MixedColorSpaces);
        }
        self.mutation_space = Some(space);
        Ok(())
    }
}

/// Return one rounded channel from a parsed color.
pub fn channel(input: &str, color_channel: ColorChannel) -> Result<f64, ColorError> {
    Ok(ThemeColor::parse(input)?.channel(color_channel))
}

/// Construct and stringify an RGBA color using Khroma's clamp and format selection rules.
pub fn rgba(red: f64, green: f64, blue: f64, alpha: f64) -> Result<String, ColorError> {
    let color = ThemeColor::constructed_rgba(RgbaChannels {
        red: clamp_channel(ColorChannel::Red, red),
        green: clamp_channel(ColorChannel::Green, green),
        blue: clamp_channel(ColorChannel::Blue, blue),
        alpha: clamp_channel(ColorChannel::Alpha, alpha),
    });
    Ok(color.stringify())
}

/// Replace a color's alpha channel, equivalent to Khroma's color overload of `rgba`.
pub fn with_alpha(input: &str, alpha: f64) -> Result<String, ColorError> {
    let mut color = ThemeColor::parse(input)?;
    color.set_channel(ColorChannel::Alpha, alpha)?;
    Ok(color.stringify())
}

/// Apply typed channel deltas with Khroma's `adjust` semantics.
pub fn adjust(input: &str, adjustment: ColorAdjustment) -> Result<String, ColorError> {
    // Khroma parses before it inspects the adjustment object, so an invalid color wins over a
    // later mixed-space setter failure.
    let mut color = ThemeColor::parse(input)?;
    let rgb_changed = [adjustment.red, adjustment.green, adjustment.blue]
        .into_iter()
        .flatten()
        .any(is_truthy_number);
    let hsl_changed = [adjustment.hue, adjustment.saturation, adjustment.lightness]
        .into_iter()
        .flatten()
        .any(is_truthy_number);
    if rgb_changed && hsl_changed {
        return Err(ColorError::MixedColorSpaces);
    }

    if rgb_changed {
        let original = color.rgba;
        if let Some(amount) = adjustment.red.filter(|value| is_truthy_number(*value)) {
            color.set_channel(ColorChannel::Red, original.red + amount)?;
        }
        if let Some(amount) = adjustment.green.filter(|value| is_truthy_number(*value)) {
            color.set_channel(ColorChannel::Green, original.green + amount)?;
        }
        if let Some(amount) = adjustment.blue.filter(|value| is_truthy_number(*value)) {
            color.set_channel(ColorChannel::Blue, original.blue + amount)?;
        }
    } else if hsl_changed {
        let original = color.hsla;
        if let Some(amount) = adjustment.hue.filter(|value| is_truthy_number(*value)) {
            color.set_channel(ColorChannel::Hue, original.hue_degrees + amount)?;
        }
        if let Some(amount) = adjustment
            .saturation
            .filter(|value| is_truthy_number(*value))
        {
            color.set_channel(
                ColorChannel::Saturation,
                original.saturation_percent + amount,
            )?;
        }
        if let Some(amount) = adjustment
            .lightness
            .filter(|value| is_truthy_number(*value))
        {
            color.set_channel(ColorChannel::Lightness, original.lightness_percent + amount)?;
        }
    }
    if let Some(amount) = adjustment.alpha.filter(|value| is_truthy_number(*value)) {
        let original_alpha = color.rgba.alpha;
        color.set_channel(ColorChannel::Alpha, original_alpha + amount)?;
    }
    Ok(color.stringify())
}

pub fn lighten(input: &str, amount: f64) -> Result<String, ColorError> {
    let mut color = ThemeColor::parse(input)?;
    color.adjust_channel(ColorChannel::Lightness, amount)?;
    Ok(color.stringify())
}

pub fn darken(input: &str, amount: f64) -> Result<String, ColorError> {
    lighten(input, -amount)
}

pub fn transparentize(input: &str, amount: f64) -> Result<String, ColorError> {
    let mut color = ThemeColor::parse(input)?;
    color.adjust_channel(ColorChannel::Alpha, -amount)?;
    Ok(color.stringify())
}

/// Invert a color at Khroma's default 100 percent weight.
pub fn invert(input: &str) -> Result<String, ColorError> {
    invert_weighted(input, 100.0)
}

/// Invert a color with Khroma's Sass-compatible weighted mixing behavior.
pub fn invert_weighted(input: &str, weight: f64) -> Result<String, ColorError> {
    let original = ThemeColor::parse(input)?.rgba;
    let inverse = RgbaChannels {
        red: 255.0 - original.red,
        green: 255.0 - original.green,
        blue: 255.0 - original.blue,
        alpha: original.alpha,
    };
    let weight_scale = weight / 100.0;
    let weight_normalized = weight_scale * 2.0 - 1.0;
    let alpha_delta = inverse.alpha - original.alpha;
    let combined = if weight_normalized * alpha_delta == -1.0 {
        weight_normalized
    } else {
        (weight_normalized + alpha_delta) / (1.0 + weight_normalized * alpha_delta)
    };
    let weight_inverse = (combined + 1.0) / 2.0;
    let weight_original = 1.0 - weight_inverse;
    rgba(
        inverse.red * weight_inverse + original.red * weight_original,
        inverse.green * weight_inverse + original.green * weight_original,
        inverse.blue * weight_inverse + original.blue * weight_original,
        inverse.alpha * weight_scale + original.alpha * (1.0 - weight_scale),
    )
}

pub fn is_dark(input: &str) -> Result<bool, ColorError> {
    let rgba = ThemeColor::parse(input)?.rgba;
    let luminance = 0.2126 * to_linear(rgba.red)
        + 0.7152 * to_linear(rgba.green)
        + 0.0722 * to_linear(rgba.blue);
    Ok(round_1e10(luminance) < 0.5)
}

fn stringify_hex(rgba: RgbaChannels) -> String {
    let red = rounded_color_byte(rgba.red);
    let green = rounded_color_byte(rgba.green);
    let blue = rounded_color_byte(rgba.blue);
    if rgba.alpha < 1.0 {
        format!(
            "#{red:02x}{green:02x}{blue:02x}{:02x}",
            rounded_color_byte(rgba.alpha * 255.0)
        )
    } else {
        format!("#{red:02x}{green:02x}{blue:02x}")
    }
}

fn stringify_rgb(rgba: RgbaChannels) -> String {
    if rgba.alpha < 1.0 {
        format!(
            "rgba({}, {}, {}, {})",
            format_js_rounded(rgba.red),
            format_js_rounded(rgba.green),
            format_js_rounded(rgba.blue),
            format_js_rounded(rgba.alpha)
        )
    } else {
        format!(
            "rgb({}, {}, {})",
            format_js_rounded(rgba.red),
            format_js_rounded(rgba.green),
            format_js_rounded(rgba.blue)
        )
    }
}

fn stringify_hsl(hsla: HslaChannels) -> String {
    if hsla.alpha < 1.0 {
        format!(
            "hsla({}, {}%, {}%, {})",
            format_js_rounded(hsla.hue_degrees),
            format_js_rounded(hsla.saturation_percent),
            format_js_rounded(hsla.lightness_percent),
            JsNumber(hsla.alpha)
        )
    } else {
        format!(
            "hsl({}, {}%, {}%)",
            format_js_rounded(hsla.hue_degrees),
            format_js_rounded(hsla.saturation_percent),
            format_js_rounded(hsla.lightness_percent)
        )
    }
}

fn rounded_color_byte(value: f64) -> u8 {
    js_round(value).clamp(0.0, 255.0) as u8
}

fn round_1e10(value: f64) -> f64 {
    js_round(value * 10_000_000_000.0) / 10_000_000_000.0
}

fn js_round(value: f64) -> f64 {
    if !value.is_finite() || value.fract() == 0.0 {
        return value;
    }
    (value + 0.5).floor()
}

fn format_js_rounded(value: f64) -> JsNumber {
    JsNumber(round_1e10(value))
}

struct JsNumber(f64);

impl fmt::Display for JsNumber {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = self.0;
        let special = if value.is_nan() {
            Some("NaN")
        } else if value == f64::INFINITY {
            Some("Infinity")
        } else if value == f64::NEG_INFINITY {
            Some("-Infinity")
        } else if value == 0.0 {
            Some("0")
        } else {
            None
        };
        if let Some(special) = special {
            return formatter.write_str(special);
        }
        formatter.write_str(Buffer::new().format_finite(value))
    }
}

fn clamp_channel(channel: ColorChannel, value: f64) -> f64 {
    match channel {
        ColorChannel::Red | ColorChannel::Green | ColorChannel::Blue => clamp_rgb(value),
        ColorChannel::Hue => clamp_hue(value),
        ColorChannel::Saturation | ColorChannel::Lightness => clamp_percent(value),
        ColorChannel::Alpha => clamp_alpha(value),
    }
}

fn is_integer(value: f64) -> bool {
    value.is_finite() && value.fract() == 0.0
}

fn is_truthy_number(value: f64) -> bool {
    value != 0.0 && !value.is_nan()
}

fn to_linear(channel: f64) -> f64 {
    let normalized = channel / 255.0;
    if channel > 0.03928 {
        ((normalized + 0.055) / 1.055).powf(2.4)
    } else {
        normalized / 12.92
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_khroma_color_syntax_without_normalizing_raw_values() {
        let cases = [
            ("#abc", ColorSourceFormat::Hex, [170.0, 187.0, 204.0, 1.0]),
            (
                "#abcd",
                ColorSourceFormat::Hex,
                [170.0, 187.0, 204.0, 221.0 / 255.0],
            ),
            (
                "#10203040",
                ColorSourceFormat::Hex,
                [16.0, 32.0, 48.0, 64.0 / 255.0],
            ),
            (
                "rgb(10, 20, 30)",
                ColorSourceFormat::Rgb,
                [10.0, 20.0, 30.0, 1.0],
            ),
            (
                "RGB(10% 20% 30% / 40%)",
                ColorSourceFormat::Rgb,
                [25.5, 51.0, 76.5, 0.4],
            ),
            ("rgb( 1,2,3)", ColorSourceFormat::Rgb, [1.0, 2.0, 3.0, 1.0]),
        ];
        for (input, format, expected) in cases {
            let color = ThemeColor::parse(input).unwrap();
            assert_eq!(color.raw(), Some(input));
            assert_eq!(color.source_format(), format);
            let actual = color.rgba_channels();
            assert_eq!(
                [actual.red, actual.green, actual.blue, actual.alpha],
                expected
            );
            assert_eq!(color.stringify(), input);
        }
    }

    #[test]
    fn parses_hsl_angles_alpha_and_keywords_with_khroma_evidence() {
        let turn = ThemeColor::parse("hsla(.5turn 50% 25% / .75)").unwrap();
        assert_eq!(turn.source_format(), ColorSourceFormat::Hsl);
        assert_eq!(
            turn.hsla_channels(),
            HslaChannels {
                hue_degrees: 180.0,
                saturation_percent: 50.0,
                lightness_percent: 25.0,
                alpha: 0.75,
            }
        );
        assert_eq!(turn.stringify(), "hsla(.5turn 50% 25% / .75)");
        assert_eq!(
            ThemeColor::parse("hsl( 1,2%,3%)").unwrap().stringify(),
            "hsl( 1,2%,3%)"
        );
        assert_eq!(
            channel("hsl(1turn 50% 50%)", ColorChannel::Hue).unwrap(),
            0.0
        );
        assert_eq!(
            channel("hsl(1TURN 50% 50%)", ColorChannel::Hue).unwrap(),
            1.0
        );
        assert_eq!(
            channel("hsl(100grad 50% 50%)", ColorChannel::Hue).unwrap(),
            90.0
        );
        assert_eq!(
            channel("hsl(100GRAD 50% 50%)", ColorChannel::Hue).unwrap(),
            100.0
        );

        let keyword = ThemeColor::parse("ReD").unwrap();
        assert_eq!(keyword.raw(), Some("ReD"));
        assert_eq!(keyword.source_format(), ColorSourceFormat::Keyword);
        assert_eq!(keyword.stringify(), "#ff0000");
        assert_eq!(
            ThemeColor::parse("transparent").unwrap().stringify(),
            "#00000000"
        );
    }

    #[test]
    fn rejects_non_khroma_forms() {
        for input in [
            " #fff",
            "#12",
            "rgb(1 2)",
            "rgb(1 2 3 0.5)",
            "hsl(1 2 3)",
            "unknown",
        ] {
            assert!(
                matches!(
                    ThemeColor::parse(input),
                    Err(ColorError::UnsupportedFormat { .. })
                ),
                "{input}"
            );
        }
    }

    #[test]
    fn matches_khroma_operation_oracle_matrix() {
        assert_eq!(
            lighten("#123456", 10.0).unwrap(),
            "hsl(210, 65.3846153846%, 30.3921568627%)"
        );
        assert_eq!(
            darken("hsl(20, 30%, 40%)", 50.0).unwrap(),
            "hsl(20, 30%, 0%)"
        );
        assert_eq!(
            adjust("#102030", ColorAdjustment::rgb(5.0, 10.0, 15.0)).unwrap(),
            "#152a3f"
        );
        assert_eq!(
            adjust("rgb(18 52 86 / .5)", ColorAdjustment::hsl(-120.0, 0.0, 5.0)).unwrap(),
            "hsla(90, 65.3846153846%, 25.3921568627%, 0.5)"
        );
        assert_eq!(invert("#123456").unwrap(), "#edcba9");
        assert_eq!(invert("hsl(0, 100%, 50%)").unwrap(), "#00ffff");
        assert!(is_dark("#123456").unwrap());
        assert!(!is_dark("white").unwrap());
        assert_eq!(
            transparentize("rgb(1, 2, 3)", 0.25).unwrap(),
            "rgba(1, 2, 3, 0.75)"
        );
    }

    #[test]
    fn channel_and_rgba_match_khroma_rounding_and_clamping() {
        assert_eq!(channel("#abcdef", ColorChannel::Hue).unwrap(), 210.0);
        assert_eq!(
            channel("rgb(10% 20% 30%)", ColorChannel::Red).unwrap(),
            25.5
        );
        assert_eq!(rgba(255.0, 255.0, 255.0, 70.0).unwrap(), "#ffffff");
        assert_eq!(rgba(255.0, 255.0, 255.0, 50.0).unwrap(), "#ffffff");
        assert_eq!(
            rgba(1.25, 2.5, 3.75, 0.5).unwrap(),
            "rgba(1.25, 2.5, 3.75, 0.5)"
        );
        assert_eq!(
            with_alpha("hsl(20, 30%, 40%)", 0.25).unwrap(),
            "hsla(20, 30%, 40%, 0.25)"
        );
        assert_eq!(
            with_alpha("hsl(20, 30%, 40%)", 1.0 / 3.0).unwrap(),
            "hsla(20, 30%, 40%, 0.3333333333333333)"
        );
        assert_eq!(
            rgba(1.0 / 3.0, 0.0, 0.0, 0.5).unwrap(),
            "rgba(0.3333333333, 0, 0, 0.5)"
        );
        assert_eq!(with_alpha("#fff", 1.0).unwrap(), "#ffffff");
    }

    #[test]
    fn preserves_changed_space_and_nan_behavior() {
        assert_eq!(lighten("#fff", 0.0).unwrap(), "#fff");
        assert_eq!(
            lighten("#ECECFF", f64::NAN).unwrap(),
            "hsl(240, 100%, NaN%)"
        );
        assert_eq!(
            adjust("red", ColorAdjustment::default()).unwrap(),
            "#ff0000"
        );
        assert_eq!(
            adjust(
                "#fff",
                ColorAdjustment {
                    red: Some(1.0),
                    ..ColorAdjustment::default()
                }
            )
            .unwrap(),
            "#ffffff"
        );
        assert_eq!(
            lighten("hsl(20, 30%, 100%)", 10.0).unwrap(),
            "hsl(20, 30%, 100%)"
        );
        let mixed = ColorAdjustment {
            red: Some(1.0),
            hue: Some(1.0),
            ..ColorAdjustment::default()
        };
        assert_eq!(adjust("#fff", mixed), Err(ColorError::MixedColorSpaces));
        assert!(matches!(
            adjust("nope", mixed),
            Err(ColorError::UnsupportedFormat { .. })
        ));
        assert_eq!(
            ColorError::UnsupportedFormat {
                input: "nope".to_string()
            }
            .to_string(),
            "Unsupported color format: \"nope\""
        );
        assert_eq!(
            ColorError::MixedColorSpaces.to_string(),
            "Cannot change both RGB and HSL channels at the same time"
        );
    }
}

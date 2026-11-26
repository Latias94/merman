//! Shared SVG paint emission for resolved diagram themes.

use merman_markup::theme_color::{ColorChannel, ColorSourceFormat, ThemeColor};

use crate::number_format::canonical_number;

pub mod canvas;
#[cfg(feature = "theme-label-background")]
pub mod edge_label_background;
#[cfg(feature = "theme-elevation")]
pub mod elevation;
pub mod marker;
pub mod pattern;

pub use canvas::{write_canvas, write_color};
pub use pattern::data_pattern;

/// Mermaid Neo emits a zero-blur shadow translated by this distance on each axis.
pub const NEO_SHADOW_OFFSET_PX: f64 = 4.0;

pub fn cssom_color_value(value: &str) -> String {
    let value = value.trim();
    let Ok(color) = ThemeColor::parse(value) else {
        // Preserve CSS variables and browser-supported syntaxes outside Khroma's parser surface.
        return value.to_string();
    };

    if color.source_format() == ColorSourceFormat::Keyword {
        return value.to_ascii_lowercase();
    }

    let red = color.channel(ColorChannel::Red).round() as i64;
    let green = color.channel(ColorChannel::Green).round() as i64;
    let blue = color.channel(ColorChannel::Blue).round() as i64;
    let alpha = color.channel(ColorChannel::Alpha);
    if alpha < 1.0 {
        let alpha = (alpha * 1000.0).round() / 1000.0;
        format!("rgba({red}, {green}, {blue}, {})", canonical_number(alpha))
    } else {
        format!("rgb({red}, {green}, {blue})")
    }
}

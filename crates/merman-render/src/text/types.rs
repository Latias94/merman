//! Shared text measurement types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WrapMode {
    #[default]
    SvgLike,
    /// SVG `<text>` behaves as a single shaping run (no whitespace-to-`<tspan>` tokenization).
    ///
    /// Mermaid uses this behavior in some diagrams (e.g. sequence message labels), where the
    /// resulting `getBBox()` width differs measurably from per-word `<tspan>` tokenization.
    SvgLikeSingleRun,
    HtmlLike,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextStyle {
    pub font_family: Option<String>,
    pub font_size: f64,
    pub font_weight: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_style: Option<String>,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: None,
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TextMetrics {
    pub width: f64,
    pub height: f64,
    pub line_count: usize,
}

/// One normal-height line box measured as a unit by a single text provider.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NormalLineMetrics {
    pub line_height: f64,
    /// Alphabetic baseline relative to the top of the line box.
    pub baseline_offset: f64,
}

impl NormalLineMetrics {
    pub(crate) fn deterministic(font_size: f64, line_height_factor: f64) -> Self {
        // A font-agnostic em box: 0.8em ascent, 0.2em descent, with symmetric leading.
        // This is the compatibility profile, not a measurement of CSS `normal` in any font.
        let em = font_size.max(1.0);
        let factor = if line_height_factor == 0.0 {
            1.2
        } else {
            line_height_factor
        };
        let line_height = em * factor;
        Self {
            line_height,
            baseline_offset: em * 0.8 + (line_height - em) / 2.0,
        }
    }
}

//! Measurement of Mermaid's body-attached calculateTextDimensions probe.

use super::{TextMeasurer, TextStyle};

pub const MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX: f64 = 200.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MermaidTextDimensions {
    pub width: i64,
    pub height: i64,
    pub line_height: i64,
}

fn try_measure_mermaid_text_dimensions_for_family<E>(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
    checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
) -> std::result::Result<MermaidTextDimensions, E> {
    if text.is_empty() {
        return Ok(MermaidTextDimensions::default());
    }

    let mut dimensions = MermaidTextDimensions::default();
    for line in super::split_mermaid_common_br_lines(text) {
        checkpoint()?;
        let measured_line = if line.is_empty() { "\u{200b}" } else { line };
        checkpoint()?;
        let measured = measurer.measure_mermaid_calculate_text_dimensions(measured_line, style);
        checkpoint()?;
        let width = measured.width.max(0.0).round() as i64;
        let height = measured.height.max(0.0).round() as i64;
        dimensions.width = dimensions.width.max(width);
        dimensions.height += height;
        dimensions.line_height = dimensions.line_height.max(height);
    }
    Ok(dimensions)
}

fn try_measure_mermaid_text_dimensions<E>(
    measurer: &dyn TextMeasurer,
    text: &str,
    configured_style: &TextStyle,
    checkpoint: &mut impl FnMut() -> std::result::Result<(), E>,
) -> std::result::Result<MermaidTextDimensions, E> {
    let mut sans_style = configured_style.clone();
    sans_style.font_family = Some("sans-serif".to_string());

    let sans =
        try_measure_mermaid_text_dimensions_for_family(measurer, text, &sans_style, checkpoint)?;
    let configured = try_measure_mermaid_text_dimensions_for_family(
        measurer,
        text,
        configured_style,
        checkpoint,
    )?;

    if sans.width > configured.width
        && sans.height > configured.height
        && sans.line_height > configured.line_height
    {
        Ok(sans)
    } else {
        Ok(configured)
    }
}

/// Mirrors Mermaid's shared `calculateTextDimensions` utility.
pub fn measure_mermaid_text_dimensions(
    measurer: &dyn TextMeasurer,
    text: &str,
    configured_style: &TextStyle,
) -> MermaidTextDimensions {
    let mut checkpoint = || Ok::<(), std::convert::Infallible>(());
    match try_measure_mermaid_text_dimensions(measurer, text, configured_style, &mut checkpoint) {
        Ok(dimensions) => dimensions,
        Err(error) => match error {},
    }
}

#[cfg(feature = "svg-output")]
pub fn measure_mermaid_text_dimensions_with_checkpoint(
    measurer: &dyn TextMeasurer,
    text: &str,
    configured_style: &TextStyle,
    mut checkpoint: impl FnMut() -> crate::Result<()>,
) -> crate::Result<MermaidTextDimensions> {
    try_measure_mermaid_text_dimensions(measurer, text, configured_style, &mut checkpoint)
}

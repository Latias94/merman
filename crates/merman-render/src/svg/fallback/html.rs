use crate::text::html_wrap::{NormalizedHtmlText, PlannedHtmlLine, plan_html_lines};
use crate::text::{HtmlWhiteSpace, TextMeasurer, TextStyle};
use std::borrow::Cow;

use super::css::{extract_style_property_with_checkpoints, parse_css_px_value};
use crate::svg::pipeline::{
    checkpoint_loop, extract_exact_double_quoted_attr_with_checkpoints, find_with_checkpoints,
};

#[derive(Clone, Debug)]
pub(super) enum HtmlTextFragment {
    Text {
        text: String,
        white_space: HtmlWhiteSpace,
        // Common ancestry depth and mode at the boundary with the previous text leaf.
        boundary_white_space: Option<(usize, HtmlWhiteSpace)>,
    },
    HardBreak,
}

fn replace_with_checkpoint<E>(
    input: &str,
    needle: &str,
    replacement: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<String, E> {
    if needle.is_empty() {
        checkpoint()?;
        return Ok(input.to_string());
    }
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0usize;
    while let Some(relative) = find_with_checkpoints(&input[cursor..], needle, checkpoint)? {
        let start = cursor + relative;
        output.push_str(&input[cursor..start]);
        output.push_str(replacement);
        cursor = start + needle.len();
    }
    output.push_str(&input[cursor..]);
    checkpoint()?;
    Ok(output)
}

fn decode_mermaid_entity_placeholders<'a, E>(
    text: &'a str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Cow<'a, str>, E> {
    if !text.contains('ﬂ') && !text.contains('¶') {
        return Ok(Cow::Borrowed(text));
    }

    let restored = replace_with_checkpoint(text, "ﬂ°°", "&#", checkpoint)?;
    let restored = replace_with_checkpoint(&restored, "ﬂ°", "&", checkpoint)?;
    replace_with_checkpoint(&restored, "¶ß", ";", checkpoint).map(Cow::Owned)
}

fn decode_html_entities<E>(
    text: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<String, E> {
    checkpoint()?;
    let mut current = text.to_string();
    for _ in 0..3 {
        checkpoint()?;
        if !current.contains('&') && !current.contains('ﬂ') && !current.contains('¶') {
            break;
        }
        // Mermaid's placeholders are not HTML entities. Restore that wrapper first,
        // then let the shared HTML entity decoder handle the browser-facing syntax.
        let restored = decode_mermaid_entity_placeholders(&current, checkpoint)?;
        checkpoint()?;
        let next = decode_html_entities_streaming(restored.as_ref(), checkpoint)?;
        if next == current {
            break;
        }
        current = next;
    }
    Ok(current)
}

fn decode_html_entities_streaming<E>(
    text: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<String, E> {
    use merman_core::entities::{
        DecodedHtmlFragment, visit_decoded_html_entity_fragments_with_checkpoints,
    };

    let mut output = String::with_capacity(text.len());
    visit_decoded_html_entity_fragments_with_checkpoints(text, &mut *checkpoint, |fragment| {
        match fragment {
            DecodedHtmlFragment::Borrowed(value) => output.push_str(value),
            DecodedHtmlFragment::Scalar(value) => output.push(value),
        }
        Ok(())
    })?;
    Ok(output)
}

fn normalize_html_source_newlines<E>(
    text: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<String, E> {
    let mut result = String::with_capacity(text.len());
    let mut previous_cr = false;
    for (index, ch) in text.chars().enumerate() {
        checkpoint_loop(index, checkpoint)?;
        if ch == '\n' && previous_cr {
            previous_cr = false;
            continue;
        }
        previous_cr = ch == '\r';
        result.push(if previous_cr { '\n' } else { ch });
    }
    checkpoint()?;
    Ok(result)
}

pub(super) fn normalized_foreign_object_text<E>(
    fragments: &[HtmlTextFragment],
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<NormalizedHtmlText, E> {
    let mut normalized = NormalizedHtmlText::default();
    for (index, fragment) in fragments.iter().enumerate() {
        checkpoint_loop(index, checkpoint)?;
        match fragment {
            HtmlTextFragment::HardBreak => normalized.hard_break(),
            HtmlTextFragment::Text {
                text,
                white_space,
                boundary_white_space,
            } => {
                if let Some((depth, mode)) = boundary_white_space {
                    normalized.inline_boundary(*depth, *mode);
                }
                // HTML parsing normalizes literal source CR before resolving character references.
                let text = normalize_html_source_newlines(text, checkpoint)?;
                // Mermaid's literal escape is an explicit break independently of CSS mode.
                for (part_index, part) in text.split("\\n").enumerate() {
                    if part_index > 0 {
                        normalized.hard_break();
                    }
                    let decoded = decode_html_entities(part, checkpoint)?;
                    normalized.append(&decoded, *white_space, checkpoint)?;
                }
            }
        }
    }
    checkpoint()?;
    Ok(normalized)
}

pub(super) fn wrap_html_text<E>(
    text: &NormalizedHtmlText,
    max_width: Option<f64>,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<PlannedHtmlLine>, E> {
    plan_html_lines(
        text,
        max_width,
        measurer.begin_html_line_width(style),
        &mut |text| Ok(measurer.measure_html_line_width_px(text, style)),
        checkpoint,
    )
}

pub(super) fn extract_inline_html_style_property<E>(
    html: &str,
    property: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<String>, E> {
    let Some(style) = extract_exact_double_quoted_attr_with_checkpoints(html, "style", checkpoint)?
    else {
        return Ok(None);
    };
    extract_style_property_with_checkpoints(style, property, checkpoint)
}

pub(super) fn foreign_object_html_wrapping<E>(
    tag: &str,
    inner: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<f64>, E> {
    // Width belongs to the label container; wrapping permissions belong to each text run.
    let css_width = extract_inline_html_style_property(inner, "width", checkpoint)?
        .and_then(|value| parse_css_px_value(&value));
    let max_width = extract_inline_html_style_property(inner, "max-width", checkpoint)?
        .and_then(|value| parse_css_px_value(&value));
    let attr_width = extract_exact_double_quoted_attr_with_checkpoints(tag, "width", checkpoint)?
        .and_then(|value| value.parse::<f64>().ok());
    let width = match (css_width, max_width) {
        (Some(width), Some(maximum)) => Some(width.min(maximum)),
        _ => css_width.or(max_width).or(attr_width),
    }
    .filter(|width| width.is_finite() && *width > 0.0);
    checkpoint()?;
    Ok(width)
}

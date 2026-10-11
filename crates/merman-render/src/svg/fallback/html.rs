use crate::text::{
    TextMeasurer, TextStyle, WrapMode, html_break_spaces_segments_with_checkpoints,
    is_html_collapsible_ascii_whitespace,
};
use std::borrow::Cow;
use std::collections::VecDeque;

use super::css::{extract_style_property_with_checkpoints, parse_css_px_value};
use crate::svg::pipeline::{
    checkpoint_loop, extract_exact_double_quoted_attr_with_checkpoints, find_with_checkpoints,
};

fn strip_html_tags<E>(
    s: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<String, E> {
    let mut out = String::new();
    let mut in_tag = false;
    for (index, ch) in s.chars().enumerate() {
        checkpoint_loop(index, checkpoint)?;
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    checkpoint()?;
    Ok(out)
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

pub(super) fn htmlish_to_text_lines<E>(
    html: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<String>, E> {
    // Mermaid foreignObject labels often look like:
    //   <div class="label">Line 1<br/>Line 2</div>
    // We treat `<br>` as line breaks and strip remaining tags.
    checkpoint()?;
    let normalized = replace_with_checkpoint(html, "<br/>", "\n", checkpoint)?;
    let normalized = replace_with_checkpoint(&normalized, "<br />", "\n", checkpoint)?;
    let normalized = replace_with_checkpoint(&normalized, "<br>", "\n", checkpoint)?;
    let normalized = replace_with_checkpoint(&normalized, "</br>", "\n", checkpoint)?;
    let normalized = replace_with_checkpoint(&normalized, "\\n", "\n", checkpoint)?;
    let stripped = strip_html_tags(&normalized, checkpoint)?;
    let text = decode_html_entities(&stripped, checkpoint)?;

    let mut lines = Vec::new();
    for (index, line) in text.lines().enumerate() {
        checkpoint_loop(index, checkpoint)?;
        let line = line.trim();
        if !line.is_empty() {
            lines.push(line.to_string());
        }
    }
    checkpoint()?;
    Ok(lines)
}

fn line_width_html_px<E>(
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    text: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<f64, E> {
    checkpoint()?;
    let width = measurer
        .measure_wrapped(text, style, None, WrapMode::HtmlLike)
        .width;
    checkpoint()?;
    Ok(width)
}

fn wrap_html_line_to_width<E>(
    line: &str,
    max_width_px: f64,
    white_space: HtmlWhiteSpace,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<String>, E> {
    let preserve_spaces = white_space.preserves_spaces();
    let mut collapsed = String::new();
    let line = if preserve_spaces {
        line
    } else {
        let mut pending_space = false;
        for (index, ch) in line.chars().enumerate() {
            checkpoint_loop(index, checkpoint)?;
            if is_html_collapsible_ascii_whitespace(ch) {
                pending_space = !collapsed.is_empty();
            } else {
                if pending_space {
                    collapsed.push(' ');
                }
                collapsed.push(ch);
                pending_space = false;
            }
        }
        collapsed.as_str()
    };
    if !max_width_px.is_finite() || max_width_px <= 0.0 {
        return Ok(vec![line.to_string()]);
    }
    if line_width_html_px(measurer, style, line, checkpoint)? <= max_width_px {
        return Ok(vec![line.to_string()]);
    }

    checkpoint()?;
    let mut tokens = VecDeque::from(html_break_spaces_segments_with_checkpoints(
        line, checkpoint,
    )?);
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut iteration = 0usize;

    while let Some(tok) = tokens.pop_front() {
        checkpoint_loop(iteration, checkpoint)?;
        iteration = iteration.saturating_add(1);
        if !preserve_spaces && cur.is_empty() && tok == " " {
            continue;
        }
        if white_space == HtmlWhiteSpace::PreWrap && tok == " " {
            cur.push(' ');
            continue;
        }
        let candidate = format!("{cur}{tok}");
        let measured_candidate = if white_space == HtmlWhiteSpace::BreakSpaces {
            candidate.as_str()
        } else {
            candidate.trim_end_matches(' ')
        };
        if line_width_html_px(measurer, style, measured_candidate, checkpoint)? <= max_width_px {
            cur = candidate;
            continue;
        }

        if !cur.is_empty() {
            if !preserve_spaces {
                cur.truncate(cur.trim_end_matches(' ').len());
            }
            out.push(std::mem::take(&mut cur));
            tokens.push_front(tok);
            continue;
        }

        // Keep a pre-wrap segment pending so its entire trailing space run can hang together.
        if white_space == HtmlWhiteSpace::PreWrap {
            cur = candidate;
            continue;
        }
        // Browser wrapping preserves an unbreakable segment's min-content overflow.
        out.push(if preserve_spaces {
            tok.to_string()
        } else {
            tok.trim_end_matches(' ').to_string()
        });
    }

    if !cur.is_empty() {
        if !preserve_spaces {
            cur.truncate(cur.trim_end_matches(' ').len());
        }
        out.push(cur);
    }

    if out.is_empty() {
        Ok(vec![line.to_string()])
    } else {
        Ok(out)
    }
}

pub(super) fn wrap_html_lines_to_width<E>(
    lines: Vec<String>,
    wrapping: HtmlTextWrapping,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<String>, E> {
    let Some(max_width_px) = wrapping.max_width_px.filter(|w| w.is_finite() && *w > 0.0) else {
        checkpoint()?;
        return Ok(lines);
    };

    let mut wrapped = Vec::new();
    for (index, line) in lines.into_iter().enumerate() {
        checkpoint_loop(index, checkpoint)?;
        wrapped.extend(wrap_html_line_to_width(
            &line,
            max_width_px,
            wrapping.white_space,
            measurer,
            style,
            checkpoint,
        )?);
    }
    checkpoint()?;
    Ok(wrapped)
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum HtmlWhiteSpace {
    Normal,
    PreWrap,
    BreakSpaces,
}

impl HtmlWhiteSpace {
    pub(super) fn preserves_spaces(self) -> bool {
        self != Self::Normal
    }
}

#[derive(Clone, Copy)]
pub(super) struct HtmlTextWrapping {
    pub max_width_px: Option<f64>,
    pub white_space: HtmlWhiteSpace,
}

pub(super) fn html_line_alignment_offset<E>(
    line: &str,
    white_space: HtmlWhiteSpace,
    anchor: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<f64, E> {
    if white_space != HtmlWhiteSpace::PreWrap || anchor == "start" || !line.ends_with(' ') {
        return Ok(0.0);
    }
    // Hanging spaces remain selectable, but do not participate in visible text alignment.
    let hanging_width = line_width_html_px(measurer, style, line, checkpoint)?
        - line_width_html_px(measurer, style, line.trim_end_matches(' '), checkpoint)?;
    Ok(hanging_width * if anchor == "middle" { 0.5 } else { 1.0 })
}

pub(super) fn foreign_object_html_wrapping<E>(
    tag: &str,
    inner: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<HtmlTextWrapping, E> {
    let css_white_space = extract_inline_html_style_property(inner, "white-space", checkpoint)?
        .map(|value| value.trim().to_ascii_lowercase());
    let white_space = match css_white_space.as_deref() {
        Some("break-spaces") => HtmlWhiteSpace::BreakSpaces,
        Some("pre-wrap" | "pre") => HtmlWhiteSpace::PreWrap,
        _ => HtmlWhiteSpace::Normal,
    };
    if matches!(css_white_space.as_deref(), Some("nowrap" | "pre")) {
        return Ok(HtmlTextWrapping {
            max_width_px: None,
            white_space,
        });
    }

    let wrap_is_explicit = matches!(
        css_white_space.as_deref(),
        Some("break-spaces" | "normal" | "pre-wrap" | "pre-line")
    );
    if css_white_space.is_some() && !wrap_is_explicit {
        return Ok(HtmlTextWrapping {
            max_width_px: None,
            white_space,
        });
    }

    let css_width = extract_inline_html_style_property(inner, "width", checkpoint)?
        .and_then(|value| parse_css_px_value(&value));
    let max_width = extract_inline_html_style_property(inner, "max-width", checkpoint)?
        .and_then(|value| parse_css_px_value(&value));
    let attr_width = extract_exact_double_quoted_attr_with_checkpoints(tag, "width", checkpoint)?
        .and_then(|value| value.parse::<f64>().ok());

    let width = css_width.or(max_width).or(attr_width).filter(|width| {
        *width > 0.0 && (wrap_is_explicit || css_width.is_some() || max_width.is_some())
    });
    checkpoint()?;
    Ok(HtmlTextWrapping {
        max_width_px: width,
        white_space,
    })
}

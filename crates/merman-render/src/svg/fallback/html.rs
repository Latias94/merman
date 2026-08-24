use crate::text::{TextMeasurer, TextStyle, WrapMode};
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

fn split_line_to_words_with_checkpoints<E>(
    line: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<VecDeque<String>, E> {
    let mut tokens = VecDeque::new();
    let mut part_start = 0usize;
    for (iteration, (offset, character)) in line.char_indices().enumerate() {
        checkpoint_loop(iteration, checkpoint)?;
        if character != ' ' {
            continue;
        }
        if part_start < offset {
            tokens.push_back(line[part_start..offset].to_string());
        }
        tokens.push_back(" ".to_string());
        part_start = offset + character.len_utf8();
    }
    if part_start < line.len() {
        tokens.push_back(line[part_start..].to_string());
    }
    while tokens.back().is_some_and(|token| token == " ") {
        tokens.pop_back();
    }
    checkpoint()?;
    Ok(tokens)
}

fn wrap_html_line_to_width<E>(
    line: &str,
    max_width_px: f64,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<String>, E> {
    if !max_width_px.is_finite() || max_width_px <= 0.0 {
        return Ok(vec![line.to_string()]);
    }
    if line_width_html_px(measurer, style, line, checkpoint)? <= max_width_px {
        return Ok(vec![line.to_string()]);
    }

    checkpoint()?;
    let mut tokens = split_line_to_words_with_checkpoints(line, checkpoint)?;
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut iteration = 0usize;

    while let Some(tok) = tokens.pop_front() {
        checkpoint_loop(iteration, checkpoint)?;
        iteration = iteration.saturating_add(1);
        if cur.is_empty() && tok == " " {
            continue;
        }

        let candidate = format!("{cur}{tok}");
        let candidate_trimmed = candidate.trim_end();
        if line_width_html_px(measurer, style, candidate_trimmed, checkpoint)? <= max_width_px {
            cur = candidate;
            continue;
        }

        if !cur.trim().is_empty() {
            out.push(cur.trim_end().to_string());
            cur.clear();
            tokens.push_front(tok);
            continue;
        }

        if tok == " " {
            continue;
        }

        // HTML labels do not use `word-break: break-all`; preserve long tokens as readable units.
        out.push(tok);
    }

    if !cur.trim().is_empty() {
        out.push(cur.trim_end().to_string());
    }

    if out.is_empty() {
        Ok(vec![line.to_string()])
    } else {
        Ok(out)
    }
}

pub(super) fn wrap_html_lines_to_width<E>(
    lines: Vec<String>,
    max_width_px: Option<f64>,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<String>, E> {
    let Some(max_width_px) = max_width_px.filter(|w| w.is_finite() && *w > 0.0) else {
        checkpoint()?;
        return Ok(lines);
    };

    let mut wrapped = Vec::new();
    for (index, line) in lines.into_iter().enumerate() {
        checkpoint_loop(index, checkpoint)?;
        wrapped.extend(wrap_html_line_to_width(
            &line,
            max_width_px,
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

pub(super) fn foreign_object_html_soft_wrap_width<E>(
    tag: &str,
    inner: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<f64>, E> {
    let white_space = extract_inline_html_style_property(inner, "white-space", checkpoint)?
        .map(|value| value.trim().to_ascii_lowercase());
    if matches!(white_space.as_deref(), Some("nowrap" | "pre")) {
        return Ok(None);
    }

    let wrap_is_explicit = matches!(
        white_space.as_deref(),
        Some("break-spaces" | "normal" | "pre-wrap" | "pre-line")
    );
    if white_space.is_some() && !wrap_is_explicit {
        return Ok(None);
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
    Ok(width)
}

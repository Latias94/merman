use crate::text::{DeterministicTextMeasurer, TextMeasurer, TextStyle, WrapMode};
use std::borrow::Cow;
use std::collections::VecDeque;

use super::attr::{parse_attr_f64, parse_attr_str};
use super::css::{extract_style_property, parse_css_px_value};

fn strip_html_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn decode_mermaid_entity_placeholders(text: &str) -> Cow<'_, str> {
    if !text.contains('ﬂ') && !text.contains('¶') {
        return Cow::Borrowed(text);
    }

    Cow::Owned(
        text.replace("ﬂ°°", "&#")
            .replace("ﬂ°", "&")
            .replace("¶ß", ";"),
    )
}

fn decode_html_entities(text: &str) -> String {
    let mut current = text.to_string();
    for _ in 0..3 {
        if !current.contains('&') && !current.contains('ﬂ') && !current.contains('¶') {
            break;
        }
        // Mermaid's placeholders are not HTML entities. Restore that wrapper first,
        // then let the shared HTML entity decoder handle the browser-facing syntax.
        let restored = decode_mermaid_entity_placeholders(&current);
        let next =
            merman_core::entities::decode_html_entities_to_unicode(restored.as_ref()).into_owned();
        if next == current {
            break;
        }
        current = next;
    }
    current
}

pub(super) fn htmlish_to_text_lines(html: &str) -> Vec<String> {
    // Mermaid foreignObject labels often look like:
    //   <div class="label">Line 1<br/>Line 2</div>
    // We treat `<br>` as line breaks and strip remaining tags.
    let mut normalized = html.replace("<br/>", "\n");
    normalized = normalized.replace("<br />", "\n");
    normalized = normalized.replace("<br>", "\n");
    normalized = normalized.replace("</br>", "\n");
    normalized = normalized.replace("\\n", "\n");
    let text = decode_html_entities(&strip_html_tags(&normalized));

    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}

fn line_width_html_px(measurer: &dyn TextMeasurer, style: &TextStyle, text: &str) -> f64 {
    measurer
        .measure_wrapped_raw(text, style, None, WrapMode::HtmlLike)
        .width
}

fn wrap_html_line_to_width(
    line: &str,
    max_width_px: f64,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
) -> Vec<String> {
    const EPS_PX: f64 = 0.125;
    if !max_width_px.is_finite()
        || max_width_px <= 0.0
        || line_width_html_px(measurer, style, line) <= max_width_px + EPS_PX
    {
        return vec![line.to_string()];
    }

    let mut tokens = VecDeque::from(DeterministicTextMeasurer::split_line_to_words(line));
    let mut out = Vec::new();
    let mut cur = String::new();

    while let Some(tok) = tokens.pop_front() {
        if cur.is_empty() && tok == " " {
            continue;
        }

        let candidate = format!("{cur}{tok}");
        let candidate_trimmed = candidate.trim_end();
        if line_width_html_px(measurer, style, candidate_trimmed) <= max_width_px + EPS_PX {
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
        vec![line.to_string()]
    } else {
        out
    }
}

pub(super) fn wrap_html_lines_to_width(
    lines: Vec<String>,
    max_width_px: Option<f64>,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
) -> Vec<String> {
    let Some(max_width_px) = max_width_px.filter(|w| w.is_finite() && *w > 0.0) else {
        return lines;
    };

    lines
        .into_iter()
        .flat_map(|line| wrap_html_line_to_width(&line, max_width_px, measurer, style))
        .collect()
}

pub(super) fn extract_inline_html_style_property(html: &str, property: &str) -> Option<String> {
    parse_attr_str(html, "style").and_then(|style| extract_style_property(style, property))
}

pub(super) fn extract_inline_html_color(html: &str) -> Option<String> {
    extract_inline_html_style_property(html, "color")
}

pub(super) fn parse_css_px(value: &str, fallback: f64) -> f64 {
    parse_css_px_value(value).unwrap_or(fallback)
}

pub(super) fn foreign_object_html_soft_wrap_width(tag: &str, inner: &str) -> Option<f64> {
    let white_space = extract_inline_html_style_property(inner, "white-space")
        .map(|value| value.trim().to_ascii_lowercase());
    if matches!(white_space.as_deref(), Some("nowrap" | "pre")) {
        return None;
    }

    let wrap_is_explicit = matches!(
        white_space.as_deref(),
        Some("break-spaces" | "normal" | "pre-wrap" | "pre-line")
    );
    if white_space.is_some() && !wrap_is_explicit {
        return None;
    }

    let css_width = extract_inline_html_style_property(inner, "width")
        .and_then(|value| parse_css_px_value(&value));
    let max_width = extract_inline_html_style_property(inner, "max-width")
        .and_then(|value| parse_css_px_value(&value));

    css_width
        .or(max_width)
        .or_else(|| parse_attr_f64(tag, "width"))
        .filter(|width| {
            *width > 0.0 && (wrap_is_explicit || css_width.is_some() || max_width.is_some())
        })
}

use std::borrow::Cow;

use crate::entities::decode_mermaid_entities_for_render_text;
use crate::svg::scanner::find_tag_end;
use crate::text::{
    TextMeasurer, TextMetrics, TextStyle, WrapMode, html_has_soft_break_opportunity,
    measure_html_with_inline_styles, mermaid_markdown_to_xhtml_label_fragment,
};

#[derive(Debug, Clone)]
pub(crate) struct StateLabelMeasurement {
    pub(crate) metrics: TextMetrics,
    pub(crate) uses_html_wrapping_table: bool,
}

impl StateLabelMeasurement {
    pub(crate) fn plain_svg_metrics(
        text: &str,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
    ) -> TextMetrics {
        measure_state_svg_plain_label(text, measurer, style)
    }
}

fn state_label_xhtml(text: &str) -> String {
    let decoded = decode_mermaid_entities_for_render_text(text);
    mermaid_markdown_to_xhtml_label_fragment(decoded.as_ref(), true)
}

/// Inspect adopted author HTML only when it exists. Plain and generated Markdown styling
/// continues to use the prepared writer decisions without a completed-SVG observation.
pub(crate) fn state_label_source_style_facts(
    text: &str,
) -> Option<crate::text::VisibleTextStyleFacts> {
    let decoded = decode_mermaid_entities_for_render_text(text);
    if !decoded.contains('<') {
        return None;
    }
    Some(crate::text::VisibleTextStyleFacts::from_xhtml_fragment(
        &state_label_xhtml(text),
    ))
}

fn simple_xhtml_text(fragment: &str) -> Option<Cow<'_, str>> {
    let inner = fragment.strip_prefix("<p>")?.strip_suffix("</p>")?;
    if inner.contains('<') || inner.contains('>') {
        return None;
    }
    Some(merman_model::text::decode_html_entities_to_unicode(inner))
}

pub(crate) fn state_markdown_label_plain_text(text: &str) -> Option<Cow<'_, str>> {
    let decoded = merman_model::text::restore_mermaid_entity_spelling(text);
    let source = crate::text::split_html_br_lines(decoded.as_ref()).join("\n");
    let lines = crate::text::mermaid_markdown_to_lines(&source, true);
    if lines.len() != 1
        || lines[0]
            .iter()
            .any(|(_, kind)| *kind != crate::text::MermaidMarkdownWordType::Normal)
    {
        return None;
    }
    Some(Cow::Owned(
        lines[0]
            .iter()
            .map(|(word, _)| crate::entities::decode_svg_text_content_entities(word))
            .collect::<Vec<_>>()
            .join(" "),
    ))
}

pub(crate) fn state_value_to_label_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(parts) => parts
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn escape_xml_attribute(value: &str) -> String {
    let decoded = merman_model::text::decode_html_entities_to_unicode(value);
    let mut out = String::with_capacity(decoded.len());
    for ch in decoded.chars() {
        if !crate::xml::is_xml_1_0_char(ch) {
            continue;
        }
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '"' => out.push_str("&quot;"),
            '\r' => out.push_str("&#13;"),
            '\n' => out.push_str("&#10;"),
            _ => out.push(ch),
        }
    }
    out
}

fn parse_img_attributes(tag: &str) -> Vec<(String, String)> {
    fn is_name_byte(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'-' | b'.')
    }

    let bytes = tag.as_bytes();
    let mut attributes = Vec::new();
    let mut i = tag.find(char::is_whitespace).unwrap_or(tag.len());
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i].is_ascii_whitespace() || bytes[i] == b'/') {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] == b'>' {
            break;
        }

        let name_start = i;
        while i < bytes.len() && is_name_byte(bytes[i]) {
            i += 1;
        }
        if name_start == i {
            i += 1;
            continue;
        }
        let name = tag[name_start..i].to_ascii_lowercase();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }

        let mut value = String::new();
        if i < bytes.len() && bytes[i] == b'=' {
            i += 1;
            while i < bytes.len() && bytes[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < bytes.len() && matches!(bytes[i], b'\'' | b'"') {
                let quote = bytes[i];
                i += 1;
                let value_start = i;
                while i < bytes.len() && bytes[i] != quote {
                    i += 1;
                }
                value.push_str(&tag[value_start..i]);
                if i < bytes.len() {
                    i += 1;
                }
            } else {
                let value_start = i;
                while i < bytes.len()
                    && !bytes[i].is_ascii_whitespace()
                    && !matches!(bytes[i], b'/' | b'>')
                {
                    i += 1;
                }
                value.push_str(&tag[value_start..i]);
            }
        }
        attributes.push((name, value));
    }
    attributes
}

fn normalized_image_style(existing: Option<&str>, image_only: bool, font_size: f64) -> String {
    let mut declarations = existing
        .into_iter()
        .flat_map(|style| style.split(';'))
        .filter_map(|declaration| {
            let declaration = declaration.trim();
            let (name, value) = declaration.split_once(':')?;
            let name = name.trim().to_ascii_lowercase();
            if matches!(
                name.as_str(),
                "display" | "flex-direction" | "width" | "min-width" | "max-width"
            ) {
                return None;
            }
            Some((name, value.trim().to_string()))
        })
        .collect::<Vec<_>>();
    declarations.push(("display".to_string(), "flex".to_string()));
    declarations.push(("flex-direction".to_string(), "column".to_string()));
    if image_only {
        let width = font_size.max(1.0) * 5.0;
        let width = if width.fract().abs() < 1e-9 {
            format!("{width:.0}px")
        } else {
            format!("{width}px")
        };
        declarations.push(("min-width".to_string(), width.clone()));
        declarations.push(("max-width".to_string(), width));
    } else {
        declarations.push(("width".to_string(), "100%".to_string()));
    }

    declarations
        .into_iter()
        .map(|(name, value)| format!("{name}: {value};"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn label_contains_only_img_tags(raw: &str) -> bool {
    let lower = raw.to_ascii_lowercase();
    let mut cursor = 0usize;
    let mut visible = String::new();
    while let Some(relative) = lower[cursor..].find("<img") {
        let start = cursor + relative;
        visible.push_str(&raw[cursor..start]);
        let Some(end) = find_tag_end(raw, start + 4) else {
            visible.push_str(&raw[start..]);
            return visible.trim().is_empty();
        };
        cursor = end + 1;
    }
    visible.push_str(&raw[cursor..]);
    visible.trim().is_empty()
}

fn normalize_img_tags(fragment: &str, image_style: Option<(&str, f64)>) -> String {
    if !fragment.to_ascii_lowercase().contains("<img") {
        return fragment.to_string();
    }

    let image_only = image_style.is_some_and(|(raw, _)| label_contains_only_img_tags(raw));
    let lower = fragment.to_ascii_lowercase();
    let mut out = String::with_capacity(fragment.len() + 64);
    let mut cursor = 0usize;
    while let Some(relative) = lower[cursor..].find("<img") {
        let start = cursor + relative;
        out.push_str(&fragment[cursor..start]);
        let Some(end) = find_tag_end(fragment, start + 4) else {
            out.push_str("&lt;");
            cursor = start + 1;
            continue;
        };
        let tag = &fragment[start + 1..=end];
        let mut attributes = parse_img_attributes(tag);
        if let Some((_, font_size)) = image_style {
            let existing = attributes
                .iter()
                .find(|(name, _)| name == "style")
                .map(|(_, value)| value.as_str());
            let style = normalized_image_style(existing, image_only, font_size);
            attributes.retain(|(name, _)| name != "style");
            attributes.push(("style".to_string(), style));
        }

        out.push_str("<img");
        for (name, value) in attributes {
            out.push(' ');
            out.push_str(&name);
            out.push_str("=\"");
            out.push_str(&escape_xml_attribute(&value));
            out.push('"');
        }
        out.push_str(" />");
        cursor = end + 1;
    }
    out.push_str(&fragment[cursor..]);
    out
}

pub(crate) fn state_node_label_xhtml(text: &str, font_size: f64) -> String {
    normalize_img_tags(&state_label_xhtml(text), Some((text, font_size)))
}

pub(crate) fn state_edge_label_xhtml(text: &str) -> String {
    normalize_img_tags(&state_label_xhtml(text), None)
}

/// Prepare SVG words from label source before applying createText's entity projection.
pub(super) fn state_svg_markdown_word_lines(
    text: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_width: Option<f64>,
    wrap_mode: WrapMode,
) -> Vec<Vec<(String, crate::text::MermaidMarkdownWordType)>> {
    let decoded = merman_model::text::restore_mermaid_entity_spelling(text);
    let source = crate::text::split_html_br_lines(decoded.as_ref()).join("\n");
    crate::text::mermaid_markdown_to_wrapped_word_lines(
        measurer, &source, style, max_width, wrap_mode,
    )
}

/// Mirror nonMarkdownToLines: split source breaks and words before decoding SVG entities.
pub(super) fn state_svg_plain_lines(text: &str) -> Vec<String> {
    let decoded = merman_model::text::restore_mermaid_entity_spelling(text);
    let source = decoded.replace("\\n", "\n");
    source
        .split('\n')
        .flat_map(crate::text::split_html_br_lines)
        .map(|line| {
            crate::text::non_markdown_svg_words(line)
                .map(crate::entities::decode_svg_text_content_entities)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

/// Measure retained SVG words with literal tspan operations, never a label-source API.
pub(super) fn measure_state_svg_word_lines(
    lines: &[Vec<(String, crate::text::MermaidMarkdownWordType)>],
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
) -> TextMetrics {
    use crate::text::{svg_markdown_word_style, trim_html_collapsible_ascii_whitespace};

    let mut width = 0.0_f64;
    let mut ink: Option<(f64, f64)> = None;
    for (row_index, line) in lines.iter().enumerate() {
        let mut row_width = 0.0;
        let mut run = String::new();
        let mut run_kind = None;
        for (word_index, (word, kind)) in line.iter().enumerate() {
            let visible = crate::entities::decode_svg_text_content_entities(word);
            let text = if word_index == 0 {
                visible
            } else {
                Cow::Owned(format!(" {visible}"))
            };
            if run_kind.is_some_and(|previous| previous != *kind) {
                let face = svg_markdown_word_style(style, run_kind.unwrap());
                row_width += measurer.measure_svg_text_computed_length_px(&run, &face);
                run.clear();
            }
            run_kind = Some(*kind);
            run.push_str(&text);
            if trim_html_collapsible_ascii_whitespace(&text).is_empty() {
                continue;
            }
            let face = svg_markdown_word_style(style, *kind);
            let top = measurer.measure_svg_create_text_bbox_y_offset_px(&text, &face)
                + row_index as f64 * style.font_size * 1.1;
            let height = measurer.measure_svg_tspan_text_bbox_height_px(&text, &face);
            if height > 0.0 {
                let bottom = top + height;
                ink = Some(ink.map_or((top, bottom), |(min, max)| (min.min(top), max.max(bottom))));
            }
        }
        if let Some(kind) = run_kind {
            let face = svg_markdown_word_style(style, kind);
            row_width += measurer.measure_svg_text_computed_length_px(&run, &face);
        }
        width = width.max(row_width);
    }
    TextMetrics {
        width: if lines
            .iter()
            .flatten()
            .all(|(_, kind)| *kind == crate::text::MermaidMarkdownWordType::Normal)
        {
            width
        } else {
            crate::text::round_to_1_64_px(width)
        },
        height: ink.map_or(0.0, |(top, bottom)| (bottom - top).max(0.0)),
        line_count: lines.len(),
    }
}

pub(super) fn measure_state_svg_plain_label(
    text: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
) -> TextMetrics {
    // Plain rows are already terminal text; do not apply even the SVG entity projection twice.
    let lines = state_svg_plain_lines(text);
    let style =
        crate::text::svg_markdown_word_style(style, crate::text::MermaidMarkdownWordType::Normal);
    let style = style.as_ref();
    let mut width = 0.0_f64;
    let mut ink: Option<(f64, f64)> = None;
    for (index, line) in lines.iter().enumerate() {
        width = width.max(measurer.measure_svg_text_computed_length_px(line, style));
        let height = measurer.measure_svg_tspan_text_bbox_height_px(line, style);
        if height > 0.0 {
            let top = measurer.measure_svg_create_text_bbox_y_offset_px(line, style)
                + index as f64 * style.font_size * 1.1;
            let bottom = top + height;
            ink = Some(ink.map_or((top, bottom), |(min, max)| (min.min(top), max.max(bottom))));
        }
    }
    TextMetrics {
        width,
        height: ink.map_or(0.0, |(top, bottom)| (bottom - top).max(0.0)),
        line_count: lines.len(),
    }
}

pub(crate) fn measure_state_markdown_label(
    text: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_width: Option<f64>,
    wrap_mode: WrapMode,
) -> StateLabelMeasurement {
    if wrap_mode != WrapMode::HtmlLike {
        let lines = state_svg_markdown_word_lines(text, measurer, style, max_width, wrap_mode);
        return StateLabelMeasurement {
            metrics: measure_state_svg_word_lines(&lines, measurer, style),
            uses_html_wrapping_table: false,
        };
    }

    let max_width = max_width.filter(|width| width.is_finite() && *width > 0.0);
    let fragment = state_label_xhtml(text);
    // HTML terminals retain the structured path when a literal tag spelling could be reparsed.
    let simple_text = simple_xhtml_text(&fragment).filter(|plain| !plain.contains('<'));
    let measure = |width: Option<f64>| match simple_text.as_deref() {
        Some(plain) => {
            let natural = measurer.measure_wrapped(plain, style, None, WrapMode::HtmlLike);
            let needs_breakable_wrap = width.is_some_and(|max_width| {
                natural.width > max_width && html_has_soft_break_opportunity(plain)
            });
            if needs_breakable_wrap {
                measure_html_with_inline_styles(
                    measurer,
                    &fragment,
                    style,
                    width,
                    WrapMode::HtmlLike,
                )
            } else {
                natural
            }
        }
        None => {
            measure_html_with_inline_styles(measurer, &fragment, style, width, WrapMode::HtmlLike)
        }
    };

    let natural = measure(None);
    let mut metrics = measure(max_width);
    let uses_html_wrapping_table = wrap_mode == WrapMode::HtmlLike
        && max_width.is_some_and(|width| natural.width >= width - 1e-9);

    if let Some(max_width) = max_width
        && metrics.width > max_width + 1e-9
    {
        let expanded_width = metrics.width;
        let reflowed = measure(Some(expanded_width));
        metrics.width = metrics.width.max(reflowed.width);
        metrics.height = reflowed.height;
        metrics.line_count = reflowed.line_count;
    }

    StateLabelMeasurement {
        metrics,
        uses_html_wrapping_table,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::DeterministicTextMeasurer;
    use serde_json::json;

    struct LiteralSvgMeasurer;

    impl TextMeasurer for LiteralSvgMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            panic!("prepared SVG terminals must use literal tspan operations");
        }

        fn measure_svg_text_computed_length_px(&self, text: &str, _style: &TextStyle) -> f64 {
            text.chars().count() as f64 * 10.0
        }

        fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, _style: &TextStyle) -> f64 {
            if text.is_empty() { 0.0 } else { 19.0 }
        }
    }

    #[test]
    fn svg_terminal_measurement_never_reenters_label_source_normalization() {
        let measurer = LiteralSvgMeasurer;
        let style = TextStyle::default();
        for (source, visible) in [
            ("A&lt;br&gt;B", "A<br>B"),
            ("A#lt;br#gt;B", "A<br>B"),
            ("**A&lt;br&gt;B**", "A<br>B"),
            ("A&amp;lt;br&amp;gt;B", "A&lt;br&gt;B"),
            ("&nbsp;", "&nbsp;"),
            ("A&shy;B", "A&shy;B"),
            ("A#nbsp;B", "A&nbsp;B"),
        ] {
            let measurement = measure_state_markdown_label(
                source,
                &measurer,
                &style,
                Some(1000.0),
                WrapMode::SvgLike,
            );
            assert_eq!(measurement.metrics.line_count, 1, "{source}");
            assert_eq!(measurement.metrics.height, 19.0, "{source}");
            assert_eq!(
                measurement.metrics.width,
                visible.chars().count() as f64 * 10.0,
                "{source}"
            );
        }
        let plain = measure_state_svg_plain_label("A&lt;br&gt;B", &measurer, &style);
        assert_eq!(
            (plain.line_count, plain.width, plain.height),
            (1, 60.0, 19.0)
        );
        let actual_break = measure_state_markdown_label(
            "A<br/>B",
            &measurer,
            &style,
            Some(1000.0),
            WrapMode::SvgLike,
        );
        assert_eq!(actual_break.metrics.line_count, 2);
        assert!((actual_break.metrics.height - 36.6).abs() < 1e-9);
    }

    #[test]
    fn array_label_text_preserves_every_source_line() {
        assert_eq!(
            state_value_to_label_text(&json!(["first", "second", "third"])),
            "first\nsecond\nthird"
        );
    }

    #[test]
    fn node_xhtml_normalizes_sanitized_images_like_mermaid_label_helper() {
        let html = state_node_label_xhtml(
            "<a href='https://example.com'><code>note</code></a><br/>\n<img src=x>",
            16.0,
        );

        assert!(html.contains(
            r#"<img src="x" style="display: flex; flex-direction: column; width: 100%;" />"#
        ));
        crate::xml_dom::Document::parse(&format!("<root>{html}</root>"))
            .expect("State label fragment must be valid XHTML");
    }

    #[test]
    fn elk_edge_breaks_follow_mermaid_regex_before_markdown() {
        assert_eq!(
            state_edge_label_xhtml(&crate::text::mermaid_html_breaks_to_newlines(
                "Hello<br/>- l1<br/>- l2"
            )),
            "<p>Hello</p>- l1 - l2"
        );
    }

    #[test]
    fn edge_xhtml_keeps_trailing_underscores_literal() {
        assert_eq!(
            state_edge_label_xhtml("Transition1_____"),
            "<p>Transition1_____</p>"
        );
    }

    #[test]
    fn simple_html_cjk_label_uses_unicode_soft_break_opportunities() {
        let measurer = DeterministicTextMeasurer::default();
        let style = TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };
        let max_width = measurer
            .measure_wrapped("负责", &style, None, WrapMode::HtmlLike)
            .width;

        let measurement = measure_state_markdown_label(
            "负责人审批",
            &measurer,
            &style,
            Some(max_width),
            WrapMode::HtmlLike,
        );

        assert_eq!(measurement.metrics.line_count, 3, "{measurement:?}");
        assert!(measurement.metrics.width <= max_width, "{measurement:?}");
    }
}

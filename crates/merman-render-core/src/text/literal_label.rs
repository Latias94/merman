//! Mermaid createText projection of literal graph labels.

use super::{
    TextMeasurer, TextStyle, flowchart_wrap_svg_source_word_lines, is_ecmascript_whitespace,
    trim_html_collapsible_ascii_whitespace,
};
use std::borrow::Cow;

fn normalize_flowchart_svg_line_breaks(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut pos = 0usize;
    while pos < input.len() {
        let rest = &input[pos..];
        if rest.starts_with("\\n") {
            out.push('\n');
            pos += 2;
            continue;
        }

        if rest.starts_with('<') && rest.len() >= 3 {
            let mut cursor = 1usize;
            if rest[cursor..].starts_with('/') {
                cursor += 1;
            }
            let mut chars = rest[cursor..].char_indices();
            if let Some((_, b)) = chars.next()
                && let Some((r_offset, r)) = chars.next()
                && b.eq_ignore_ascii_case(&'b')
                && r.eq_ignore_ascii_case(&'r')
            {
                let mut end = cursor + r_offset + r.len_utf8();
                while end < rest.len() {
                    let ch = rest[end..]
                        .chars()
                        .next()
                        .expect("position before input end has a character");
                    if !is_ecmascript_whitespace(ch) {
                        break;
                    }
                    end += ch.len_utf8();
                }
                if rest[end..].starts_with('/') {
                    end += 1;
                }
                if rest[end..].starts_with('>') {
                    out.push('\n');
                    pos += end + 1;
                    continue;
                }
            }
        }

        let ch = rest
            .chars()
            .next()
            .expect("position before input end has a character");
        out.push(ch);
        pos += ch.len_utf8();
    }
    out
}

/// Preserves Mermaid `nonMarkdownToLines()` word provenance for SVG labels.
///
/// Tokenization intentionally runs before the sanitizer entities are decoded. A literal
/// `<span class="a b">` is therefore one source word, while an entity-authored
/// `&lt;span class="a b"&gt;` keeps the ordinary whitespace boundaries it had in the source.
pub fn flowchart_non_markdown_svg_source_word_lines(label: &str) -> Vec<Vec<String>> {
    let normalized = label.replace("\r\n", "\n");
    let normalized = flowchart_decode_label_escapes(&normalized);
    let normalized = normalize_flowchart_svg_line_breaks(&normalized);

    normalized
        .split('\n')
        .map(|line| {
            crate::text::non_markdown_svg_words(line)
                .map(str::to_string)
                .collect()
        })
        .collect()
}

fn flowchart_svg_source_word_lines_plain_text(lines: &[Vec<String>]) -> String {
    let capacity = lines
        .iter()
        .flatten()
        .map(String::len)
        .sum::<usize>()
        .saturating_add(
            lines
                .iter()
                .map(|line| line.len().saturating_sub(1))
                .sum::<usize>(),
        )
        .saturating_add(lines.len().saturating_sub(1));
    let mut out = String::with_capacity(capacity);
    for (line_index, line) in lines.iter().enumerate() {
        if line_index > 0 {
            out.push('\n');
        }
        for (word_index, word) in line.iter().enumerate() {
            if word_index > 0 {
                out.push(' ');
            }
            let visible = crate::entities::decode_svg_text_content_entities(word);
            out.push_str(visible.as_ref());
        }
    }
    out
}

pub fn flowchart_non_markdown_svg_plain_text(label: &str) -> String {
    let word_lines = flowchart_non_markdown_svg_source_word_lines(label);
    flowchart_svg_source_word_lines_plain_text(&word_lines)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FlowchartSvgWidthMode {
    Bbox,
    ComputedLength,
}

/// Pure source projection of Mermaid's non-Markdown SVG createText payload.
///
/// The source rows retain authored entity and tag boundaries while `plain_text` is the exact
/// visible projection used for emptiness and metrics. This state is independent of style, width,
/// and text-measurement routing; measured preparation lives in the private Flowchart sidecar.
#[derive(Debug, Clone)]
pub struct FlowchartSvgLabelSource {
    source_lines: Vec<Vec<String>>,
    plain_text: String,
}

/// Heap storage held by the SVG word rows after their producer returns.
#[doc(hidden)]
pub fn flowchart_svg_label_rows_heap_bytes(lines: &Vec<Vec<String>>) -> usize {
    let rows = lines
        .capacity()
        .saturating_mul(std::mem::size_of::<Vec<String>>());
    lines.iter().fold(rows, |bytes, line| {
        let words = line
            .capacity()
            .saturating_mul(std::mem::size_of::<String>());
        line.iter()
            .fold(bytes.saturating_add(words), |bytes, word| {
                bytes.saturating_add(word.capacity())
            })
    })
}

fn flowchart_source_plain_projection(source_lines: &[Vec<String>]) -> String {
    let mut plain_text = String::new();
    for (line_index, line) in source_lines.iter().enumerate() {
        if line_index > 0 {
            plain_text.push('\n');
        }
        for (word_index, source_word) in line.iter().enumerate() {
            if word_index > 0 {
                plain_text.push(' ');
            }
            let mut rest = source_word.as_str();
            while !rest.is_empty() {
                if let Some(entity_len) = authored_entity_len(rest) {
                    let entity = &rest[..entity_len];
                    plain_text.push_str(match entity {
                        "&lt;" => "<",
                        "&gt;" => ">",
                        "&amp;" => "&",
                        _ => entity,
                    });
                    rest = &rest[entity_len..];
                } else {
                    let character = rest.chars().next().expect("nonempty source word");
                    plain_text.push(character);
                    rest = &rest[character.len_utf8()..];
                }
            }
        }
    }
    plain_text
}

fn authored_entity_len(text: &str) -> Option<usize> {
    const MAX_AUTHORED_ENTITY_BYTES: usize = 64;

    let bytes = text.as_bytes();
    if bytes.first() != Some(&b'&') {
        return None;
    }
    let scan = &bytes[..bytes.len().min(MAX_AUTHORED_ENTITY_BYTES)];
    let end = scan.iter().position(|byte| *byte == b';')?.checked_add(1)?;
    let body = text.get(1..end - 1)?;
    (!body.is_empty() && body.chars().all(|character| !character.is_whitespace())).then_some(end)
}

impl FlowchartSvgLabelSource {
    pub fn new(source: &str) -> Self {
        let source_lines = flowchart_non_markdown_svg_source_word_lines(source);
        let plain_text = flowchart_source_plain_projection(&source_lines);
        Self {
            source_lines,
            plain_text,
        }
    }

    pub fn plain_text(&self) -> &str {
        &self.plain_text
    }

    #[doc(hidden)]
    pub fn retained_heap_bytes(&self) -> usize {
        flowchart_svg_label_rows_heap_bytes(&self.source_lines)
            .saturating_add(self.plain_text.capacity())
    }

    pub fn wrapped_lines(
        &self,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        max_width_px: Option<f64>,
        break_long_words: bool,
    ) -> Vec<Vec<String>> {
        flowchart_wrap_svg_source_word_lines(
            measurer,
            &self.source_lines,
            style,
            max_width_px,
            break_long_words,
        )
    }

    pub fn metrics(
        &self,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        max_width_px: Option<f64>,
        width_mode: FlowchartSvgWidthMode,
    ) -> crate::text::TextMetrics {
        if measurer.cancellation_requested() {
            return crate::text::TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 0,
            };
        }
        let wrapped = self.wrapped_lines(measurer, style, max_width_px, true);
        self.metrics_from_wrapped(measurer, style, &wrapped, width_mode)
    }

    pub fn metrics_from_wrapped(
        &self,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        wrapped: &[Vec<String>],
        width_mode: FlowchartSvgWidthMode,
    ) -> crate::text::TextMetrics {
        if measurer.cancellation_requested() {
            return crate::text::TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 0,
            };
        }
        if trim_html_collapsible_ascii_whitespace(&self.plain_text).is_empty() {
            return crate::text::TextMetrics {
                width: 0.0,
                height: 0.0,
                line_count: 0,
            };
        }

        let mut width = 0.0_f64;
        let mut ink: Option<(f64, f64)> = None;
        for (row_index, row) in wrapped.iter().enumerate() {
            let visible = flowchart_svg_source_word_lines_plain_text(std::slice::from_ref(row));
            if trim_html_collapsible_ascii_whitespace(&visible).is_empty() {
                continue;
            }
            let row_width = match width_mode {
                FlowchartSvgWidthMode::Bbox => {
                    measurer.measure_svg_tspan_text_bbox_width_px(&visible, style)
                }
                FlowchartSvgWidthMode::ComputedLength => {
                    measurer.measure_svg_text_computed_length_px(&visible, style)
                }
            };
            width = width.max(row_width);
            let top = measurer.measure_svg_create_text_bbox_y_offset_px(&visible, style)
                + row_index as f64 * style.font_size * 1.1;
            let bottom = top + measurer.measure_svg_tspan_text_bbox_height_px(&visible, style);
            ink = Some(ink.map_or((top, bottom), |(min, max)| (min.min(top), max.max(bottom))));
        }
        crate::text::TextMetrics {
            width,
            height: ink.map_or(0.0, |(top, bottom)| (bottom - top).max(0.0)),
            line_count: wrapped.len(),
        }
    }
}

pub fn flowchart_decode_label_escapes(label: &str) -> Cow<'_, str> {
    if !label.contains('\\') {
        return Cow::Borrowed(label);
    }

    let mut out = String::with_capacity(label.len());
    let mut chars = label.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }

        match chars.peek().copied() {
            Some('\\') => {
                out.push('\\');
                chars.next();
            }
            Some(':') => {
                out.push(':');
                chars.next();
            }
            _ => out.push('\\'),
        }
    }
    Cow::Owned(out)
}

pub fn flowchart_non_markdown_label_for_html<'a>(label: &'a str, label_type: &str) -> Cow<'a, str> {
    if !label.contains(['\\', '\n']) {
        return Cow::Borrowed(if label_type == "string" {
            trim_html_collapsible_ascii_whitespace(label)
        } else {
            label
        });
    }
    let normalized = label.replace("\r\n", "\n");
    let label = flowchart_decode_label_escapes(&normalized);
    let label = if label_type == "string" {
        trim_html_collapsible_ascii_whitespace(&label)
    } else {
        &label
    };
    let label = label.trim_end_matches('\n');
    if !label.contains("\\n") && !label.contains('\n') {
        return Cow::Owned(label.to_owned());
    }

    let mut out = String::with_capacity(label.len());
    let mut offset = 0usize;
    while offset < label.len() {
        let rest = &label[offset..];
        if rest.starts_with("\\n") {
            out.push_str("<br />");
            offset += 2;
            continue;
        }
        let ch = rest
            .chars()
            .next()
            .expect("offset before label end has a character");
        if ch == '\n' {
            out.push_str("<br />");
        } else {
            out.push(ch);
        }
        offset += ch.len_utf8();
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_entity_scan_is_bounded_to_sixty_four_bytes() {
        let longest_admitted = format!("&{};", "a".repeat(62));
        let first_rejected = format!("&{};", "a".repeat(63));
        let amplified = format!("{};", "&".repeat(16_384));

        assert_eq!(authored_entity_len(&longest_admitted), Some(64));
        assert_eq!(authored_entity_len(&first_rejected), None);
        assert_eq!(authored_entity_len(&amplified), None);
        assert_eq!(
            FlowchartSvgLabelSource::new(&amplified).plain_text(),
            amplified
        );
    }

    #[test]
    fn authored_entity_scan_preserves_unicode_entity_semantics() {
        assert_eq!(authored_entity_len("&é;"), Some(4));
        assert_eq!(authored_entity_len("&\u{2003};"), None);

        let split_at_scan_boundary = format!("&{};", "é".repeat(32));
        assert_eq!(authored_entity_len(&split_at_scan_boundary), None);
        assert_eq!(
            FlowchartSvgLabelSource::new(&split_at_scan_boundary).plain_text(),
            split_at_scan_boundary
        );
    }

    #[test]
    fn source_plain_projection_preserves_unknown_entities_and_source_breaks() {
        let source = FlowchartSvgLabelSource::new("alpha &amp; &bad&amp; &unknown;\nbeta");
        assert_eq!(source.plain_text(), "alpha & &bad&amp; &unknown;\nbeta");
    }

    #[test]
    fn svg_non_markdown_words_preserve_source_tag_provenance() {
        let raw = flowchart_non_markdown_svg_source_word_lines("<span class='foo bar'>X</span>");
        assert_eq!(
            raw,
            vec![vec![
                "<span class='foo bar'>".to_string(),
                "X".to_string(),
                "</span>".to_string(),
            ]],
        );
        assert_eq!(
            flowchart_svg_source_word_lines_plain_text(&raw),
            "<span class='foo bar'> X </span>"
        );

        let encoded = flowchart_non_markdown_svg_source_word_lines(
            "&lt;span class='foo bar'&gt;X&lt;/span&gt;",
        );
        assert_eq!(
            encoded,
            vec![vec![
                "&lt;span".to_string(),
                "class='foo".to_string(),
                "bar'&gt;X&lt;/span&gt;".to_string(),
            ]],
        );
        assert_eq!(
            flowchart_svg_source_word_lines_plain_text(&encoded),
            "<span class='foo bar'>X</span>"
        );

        let decoded_angle = flowchart_non_markdown_svg_source_word_lines("&lt;Less&lt;");
        assert_eq!(decoded_angle, vec![vec!["&lt;Less&lt;".to_string()]]);
        assert_eq!(
            flowchart_svg_source_word_lines_plain_text(&decoded_angle),
            "<Less<"
        );

        let short_non_ascii = flowchart_non_markdown_svg_source_word_lines("<é");
        assert_eq!(short_non_ascii, vec![vec!["é".to_string()]]);
    }

    #[test]
    fn non_markdown_html_preserves_internal_line_boundary_whitespace() {
        assert_eq!(
            flowchart_non_markdown_label_for_html("A\n  B  \nC", "text"),
            "A<br />  B  <br />C"
        );
        assert_eq!(
            flowchart_non_markdown_label_for_html("  A\n  B  \nC  ", "string"),
            "A<br />  B  <br />C"
        );
    }
}

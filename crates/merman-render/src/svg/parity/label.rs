//! Shared Mermaid `createText` SVG label emission.

use super::*;
use crate::svg::parity::util::escape_xml_raw_into;
use crate::text::PreparedTextLabelId;

#[derive(Clone, Copy)]
enum SvgTextEntityMode {
    DecodedModel,
    CreateTextSource,
}

fn write_svg_text_inner_word(
    out: &mut impl crate::svg::parity::SvgOutput,
    word_index: usize,
    word: &str,
    entity_mode: SvgTextEntityMode,
    reset_font_style: bool,
) {
    out.push_str("<tspan");
    if reset_font_style {
        out.push_str(r#" font-style="normal""#);
    }
    out.push_str(r#" class="text-inner-tspan""#);
    if reset_font_style {
        out.push_str(r#" font-weight="normal""#);
    }
    out.push('>');
    if word_index > 0 {
        out.push(' ');
    }
    write_svg_text_word(out, word, entity_mode);
    out.push_str("</tspan>");
}

pub(in crate::svg::parity) fn write_svg_text_centered(
    out: &mut impl crate::svg::parity::SvgOutput,
    text: &str,
    include_style: bool,
) {
    write_svg_text_impl(
        out,
        text,
        include_style,
        true,
        true,
        SvgTextEntityMode::DecodedModel,
    );
}

pub(in crate::svg::parity) fn write_svg_text_centered_from_create_text_source(
    out: &mut impl crate::svg::parity::SvgOutput,
    text: &str,
    include_style: bool,
) {
    write_svg_text_impl(
        out,
        text,
        include_style,
        true,
        true,
        SvgTextEntityMode::CreateTextSource,
    );
}

pub(in crate::svg::parity) fn write_svg_text_source_word_lines(
    out: &mut impl crate::svg::parity::SvgOutput,
    lines: &[Vec<String>],
    include_style: bool,
    center_text: bool,
) {
    write_svg_text_source_word_lines_impl(
        out,
        lines,
        include_style.then_some(""),
        center_text,
        None,
        true,
        1.1,
    );
}

pub(in crate::svg::parity) fn write_svg_text_source_word_lines_with_style(
    out: &mut impl crate::svg::parity::SvgOutput,
    lines: &[Vec<String>],
    style: &str,
    center_text: bool,
) {
    write_svg_text_source_word_lines_impl(out, lines, Some(style), center_text, None, true, 1.1);
}

pub(in crate::svg::parity) fn write_prepared_svg_text_source_word_lines(
    out: &mut impl crate::svg::parity::SvgOutput,
    lines: &[Vec<String>],
    include_style: bool,
    center_text: bool,
    label_id: PreparedTextLabelId,
) {
    write_svg_text_source_word_lines_impl(
        out,
        lines,
        include_style.then_some(""),
        center_text,
        Some(label_id),
        true,
        1.1,
    );
}

pub(in crate::svg::parity) fn write_prepared_svg_text_source_word_lines_with_style(
    out: &mut impl crate::svg::parity::SvgOutput,
    lines: &[Vec<String>],
    style: &str,
    center_text: bool,
    label_id: PreparedTextLabelId,
    inherit_font_style: bool,
    line_height_em: f64,
) {
    write_svg_text_source_word_lines_impl(
        out,
        lines,
        Some(style),
        center_text,
        Some(label_id),
        !inherit_font_style,
        line_height_em,
    );
}

fn write_svg_text_source_word_lines_impl(
    out: &mut impl crate::svg::parity::SvgOutput,
    lines: &[Vec<String>],
    style: Option<&str>,
    center_text: bool,
    label_id: Option<PreparedTextLabelId>,
    reset_inner_font_style: bool,
    line_height_em: f64,
) {
    open_svg_text(out, style, center_text, label_id);

    if lines.len() == 1 && lines[0].is_empty() {
        write_empty_tspan(out, center_text, true, line_height_em);
        out.push_str("</text>");
        return;
    }

    for (line_index, words) in lines.iter().enumerate() {
        open_tspan(out, line_index, center_text, true, line_height_em);
        for (word_index, word) in words.iter().enumerate() {
            write_svg_text_inner_word(
                out,
                word_index,
                word,
                SvgTextEntityMode::CreateTextSource,
                reset_inner_font_style,
            );
        }
        out.push_str("</tspan>");
    }

    out.push_str("</text>");
}

fn open_svg_text(
    out: &mut impl crate::svg::parity::SvgOutput,
    style: Option<&str>,
    center_text: bool,
    label_id: Option<PreparedTextLabelId>,
) {
    out.push_str(r#"<text y="-10.1""#);
    if let Some(style) = style {
        let _ = write!(out, r#" style="{}""#, escape_attr_display(style));
    }
    if center_text {
        out.push_str(r#" text-anchor="middle""#);
    }
    if let Some(label_id) = label_id {
        let _ = write!(out, r#" id="{label_id}""#);
    }
    out.push('>');
}

fn outer_tspan_class(include_row_class: bool) -> &'static str {
    if include_row_class {
        "row text-outer-tspan"
    } else {
        "text-outer-tspan"
    }
}

fn normalized_em(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

fn write_empty_tspan(
    out: &mut impl crate::svg::parity::SvgOutput,
    center_text: bool,
    include_row_class: bool,
    line_height_em: f64,
) {
    let outer_class = outer_tspan_class(include_row_class);
    let initial_y_em = 1.0 - line_height_em;
    if center_text {
        let _ = write!(
            out,
            r#"<tspan class="{}" x="0" y="{}em" dy="{}em" text-anchor="middle"/>"#,
            outer_class,
            fmt_display(normalized_em(initial_y_em)),
            fmt_display(normalized_em(line_height_em)),
        );
    } else {
        let _ = write!(
            out,
            r#"<tspan class="{}" x="0" y="{}em" dy="{}em"/>"#,
            outer_class,
            fmt_display(normalized_em(initial_y_em)),
            fmt_display(normalized_em(line_height_em)),
        );
    }
}

fn open_tspan(
    out: &mut impl crate::svg::parity::SvgOutput,
    index: usize,
    center_text: bool,
    include_row_class: bool,
    line_height_em: f64,
) {
    let text_anchor = if center_text {
        r#" text-anchor="middle""#
    } else {
        ""
    };
    let outer_class = outer_tspan_class(include_row_class);
    if index == 0 {
        let initial_y_em = 1.0 - line_height_em;
        let _ = write!(
            out,
            r#"<tspan class="{}" x="0" y="{}em" dy="{}em"{}>"#,
            outer_class,
            fmt_display(normalized_em(initial_y_em)),
            fmt_display(normalized_em(line_height_em)),
            text_anchor,
        );
    } else {
        let y_em = 1.0 + (index as f64 - 1.0) * line_height_em;
        let _ = write!(
            out,
            r#"<tspan class="{}" x="0" y="{}em" dy="{}em"{}>"#,
            outer_class,
            fmt_display(normalized_em(y_em)),
            fmt_display(normalized_em(line_height_em)),
            text_anchor,
        );
    }
}

fn write_svg_text_impl(
    out: &mut impl crate::svg::parity::SvgOutput,
    text: &str,
    include_style: bool,
    center_text: bool,
    include_row_class: bool,
    entity_mode: SvgTextEntityMode,
) {
    open_svg_text(out, include_style.then_some(""), center_text, None);

    let lines = crate::text::DeterministicTextMeasurer::normalized_text_lines_for_wrap_mode(
        text,
        crate::text::WrapMode::SvgLike,
    );
    if lines.len() == 1 && lines[0].is_empty() {
        write_empty_tspan(out, center_text, include_row_class, 1.1);
        out.push_str("</text>");
        return;
    }

    for (index, line) in lines.iter().enumerate() {
        open_tspan(out, index, center_text, include_row_class, 1.1);
        for (word_index, word) in crate::text::non_markdown_svg_words(line).enumerate() {
            write_svg_text_inner_word(out, word_index, word, entity_mode, true);
        }
        out.push_str("</tspan>");
    }

    out.push_str("</text>");
}

fn write_svg_text_word(
    out: &mut impl crate::svg::parity::SvgOutput,
    word: &str,
    entity_mode: SvgTextEntityMode,
) {
    match entity_mode {
        SvgTextEntityMode::DecodedModel => escape_xml_into(out, word),
        SvgTextEntityMode::CreateTextSource => {
            let visible = crate::entities::decode_svg_text_content_entities(word);
            escape_xml_raw_into(out, visible.as_ref());
        }
    }
}

fn normalized_markdown_label(markdown: &str) -> &str {
    markdown
        .strip_prefix('`')
        .and_then(|value| value.strip_suffix('`'))
        .unwrap_or(markdown)
}

fn markdown_to_svg_word_lines(markdown: &str) -> Vec<Vec<(String, bool, bool)>> {
    crate::text::mermaid_markdown_to_lines(markdown, true)
        .into_iter()
        .map(|line| {
            line.into_iter()
                .map(|(word, kind)| {
                    let is_strong = kind == crate::text::MermaidMarkdownWordType::Strong;
                    let is_em = kind == crate::text::MermaidMarkdownWordType::Em;
                    (word, is_strong, is_em)
                })
                .collect()
        })
        .collect()
}

fn markdown_to_wrapped_svg_word_lines(
    measurer: &dyn crate::text::TextMeasurer,
    markdown: &str,
    style: &crate::text::TextStyle,
    max_width_px: Option<f64>,
) -> Vec<Vec<(String, bool, bool)>> {
    crate::text::mermaid_markdown_to_wrapped_word_lines(
        measurer,
        markdown,
        style,
        max_width_px,
        crate::text::WrapMode::SvgLike,
    )
    .into_iter()
    .map(|line| {
        line.into_iter()
            .map(|(word, kind)| {
                let is_strong = kind == crate::text::MermaidMarkdownWordType::Strong;
                let is_em = kind == crate::text::MermaidMarkdownWordType::Em;
                (word, is_strong, is_em)
            })
            .collect()
    })
    .collect()
}

fn write_svg_text_markdown_lines(
    out: &mut impl crate::svg::parity::SvgOutput,
    lines: &[Vec<(String, bool, bool)>],
    include_style: bool,
    center_text: bool,
    include_row_class: bool,
    entity_mode: SvgTextEntityMode,
) {
    open_svg_text(out, include_style.then_some(""), center_text, None);

    if lines.len() == 1 && lines[0].is_empty() {
        write_empty_tspan(out, center_text, include_row_class, 1.1);
        out.push_str("</text>");
        return;
    }

    for (index, words) in lines.iter().enumerate() {
        open_tspan(out, index, center_text, include_row_class, 1.1);

        for (word_index, (word, is_strong, is_em)) in words.iter().enumerate() {
            let font_style = if *is_em { "italic" } else { "normal" };
            let font_weight = if *is_strong { "bold" } else { "normal" };
            let _ = write!(
                out,
                r#"<tspan font-style="{}" class="text-inner-tspan" font-weight="{}">"#,
                font_style, font_weight
            );
            if word_index == 0 {
                write_svg_text_word(out, word, entity_mode);
            } else {
                out.push(' ');
                write_svg_text_word(out, word, entity_mode);
            }
            out.push_str("</tspan>");
        }

        out.push_str("</tspan>");
    }

    out.push_str("</text>");
}

pub(in crate::svg::parity) fn write_svg_text_markdown(
    out: &mut impl crate::svg::parity::SvgOutput,
    markdown: &str,
    include_style: bool,
) {
    let lines = markdown_to_svg_word_lines(normalized_markdown_label(markdown));
    write_svg_text_markdown_lines(
        out,
        &lines,
        include_style,
        false,
        true,
        SvgTextEntityMode::DecodedModel,
    );
}

pub(in crate::svg::parity) fn write_svg_text_markdown_centered(
    out: &mut impl crate::svg::parity::SvgOutput,
    markdown: &str,
    include_style: bool,
) {
    let lines = markdown_to_svg_word_lines(normalized_markdown_label(markdown));
    write_svg_text_markdown_lines(
        out,
        &lines,
        include_style,
        true,
        true,
        SvgTextEntityMode::DecodedModel,
    );
}

pub(in crate::svg::parity) fn write_svg_text_markdown_from_create_text_source(
    out: &mut impl crate::svg::parity::SvgOutput,
    markdown: &str,
    include_style: bool,
) {
    let lines = markdown_to_svg_word_lines(normalized_markdown_label(markdown));
    write_svg_text_markdown_lines(
        out,
        &lines,
        include_style,
        false,
        true,
        SvgTextEntityMode::CreateTextSource,
    );
}

pub(in crate::svg::parity) fn write_svg_text_markdown_wrapped_centered_from_create_text_source(
    out: &mut impl crate::svg::parity::SvgOutput,
    markdown: &str,
    include_style: bool,
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
    max_width_px: Option<f64>,
) {
    let lines = markdown_to_wrapped_svg_word_lines(
        measurer,
        normalized_markdown_label(markdown),
        style,
        max_width_px,
    );
    write_svg_text_markdown_lines(
        out,
        &lines,
        include_style,
        true,
        true,
        SvgTextEntityMode::CreateTextSource,
    );
}

pub(in crate::svg::parity) fn write_svg_text_markdown_wrapped_from_create_text_source(
    out: &mut impl crate::svg::parity::SvgOutput,
    markdown: &str,
    include_style: bool,
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
    max_width_px: Option<f64>,
) {
    let lines = markdown_to_wrapped_svg_word_lines(
        measurer,
        normalized_markdown_label(markdown),
        style,
        max_width_px,
    );
    write_svg_text_markdown_lines(
        out,
        &lines,
        include_style,
        false,
        true,
        SvgTextEntityMode::CreateTextSource,
    );
}

#[cfg(test)]
mod tests {
    use super::{
        write_prepared_svg_text_source_word_lines_with_style, write_svg_text_centered,
        write_svg_text_source_word_lines, write_svg_text_source_word_lines_with_style,
    };

    #[test]
    fn svg_text_style_preserves_hex_color_declarations_as_attribute_data() {
        let mut svg = String::new();
        write_svg_text_source_word_lines_with_style(
            &mut svg,
            &[vec!["label".to_string()]],
            "color:#112233;fill:#112233",
            true,
        );

        assert!(
            svg.contains(r#"style="color:#112233;fill:#112233""#),
            "{svg}"
        );
    }

    #[test]
    fn non_markdown_svg_text_uses_ecmascript_whitespace_boundaries() {
        let expected = [
            r#"<tspan font-style="normal" class="text-inner-tspan" font-weight="normal">A</tspan>"#,
            r#"<tspan font-style="normal" class="text-inner-tspan" font-weight="normal"> B</tspan>"#,
        ];

        for text in ["A\tB", "A\u{00A0}B", "A\u{2000}B"] {
            let mut out = String::new();
            write_svg_text_centered(&mut out, text, false);
            for fragment in expected {
                assert!(out.contains(fragment), "{text:?}: {out}");
            }
        }
    }

    #[test]
    fn create_text_source_word_lines_preserve_entity_and_tag_provenance() {
        let raw = vec![vec![
            "<span class='foo bar'>".to_string(),
            "X".to_string(),
            "</span>".to_string(),
        ]];
        let mut raw_svg = String::new();
        write_svg_text_source_word_lines(&mut raw_svg, &raw, false, true);
        assert!(
            raw_svg.contains(">&lt;span class=&#39;foo bar&#39;></tspan>"),
            "{raw_svg}"
        );
        assert_eq!(raw_svg.matches("text-inner-tspan").count(), 3, "{raw_svg}");

        let encoded = vec![vec![
            "&lt;span".to_string(),
            "class='foo".to_string(),
            "bar'&gt;X&lt;/span&gt;".to_string(),
        ]];
        let mut encoded_svg = String::new();
        write_svg_text_source_word_lines(&mut encoded_svg, &encoded, false, true);
        assert!(encoded_svg.contains(">&lt;span</tspan>"), "{encoded_svg}");
        assert!(
            encoded_svg.contains("> class=&#39;foo</tspan>"),
            "{encoded_svg}"
        );
        assert_eq!(
            encoded_svg.matches("text-inner-tspan").count(),
            3,
            "{encoded_svg}"
        );

        let mut angle_svg = String::new();
        write_svg_text_source_word_lines(
            &mut angle_svg,
            &[vec!["&lt;Less&lt;".to_string()]],
            false,
            true,
        );
        assert!(angle_svg.contains(">&lt;Less&lt;</tspan>"), "{angle_svg}");
    }

    #[test]
    fn create_text_source_word_lines_preserve_explicit_empty_rows() {
        let mut svg = String::new();
        write_svg_text_source_word_lines(
            &mut svg,
            &[Vec::new(), Vec::new(), Vec::new()],
            true,
            true,
        );

        assert_eq!(svg.matches(r#"class="row text-outer-tspan""#).count(), 3);
    }

    #[test]
    fn prepared_svg_rows_use_the_admitted_line_height() {
        let mut svg = String::new();
        write_prepared_svg_text_source_word_lines_with_style(
            &mut svg,
            &[
                vec!["alpha".to_string()],
                vec!["beta".to_string()],
                vec!["gamma".to_string()],
            ],
            "line-height:1.4 !important",
            false,
            crate::text::PreparedTextLabelId::new(
                crate::text::PreparedTextLabelFamily::Flowchart,
                1,
            ),
            true,
            1.4,
        );

        assert!(svg.contains(r#"y="-0.4em" dy="1.4em""#), "{svg}");
        assert!(svg.contains(r#"y="1em" dy="1.4em""#), "{svg}");
        assert!(svg.contains(r#"y="2.4em" dy="1.4em""#), "{svg}");
    }

    #[test]
    fn non_markdown_svg_text_tokenizes_tags_anywhere_in_the_line() {
        let mut out = String::new();
        write_svg_text_centered(&mut out, "x <strong>A B</strong> y", false);

        let expected = ["x", " &lt;strong>", " A", " B", " &lt;/strong>", " y"];
        let mut cursor = 0usize;
        for word in expected {
            let next = out[cursor..]
                .find(&format!(">{word}</tspan>"))
                .unwrap_or_else(|| panic!("missing {word:?}: {out}"));
            cursor += next + word.len();
        }
    }
}

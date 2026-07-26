//! Shared Mermaid `createText` SVG label emission.

use super::*;

pub(in crate::svg::parity) fn write_svg_text(out: &mut String, text: &str, include_style: bool) {
    write_svg_text_impl(out, text, include_style, false, true);
}

pub(in crate::svg::parity) fn write_svg_text_centered(
    out: &mut String,
    text: &str,
    include_style: bool,
) {
    write_svg_text_impl(out, text, include_style, true, true);
}

fn open_svg_text(out: &mut String, include_style: bool, center_text: bool) {
    match (include_style, center_text) {
        (true, true) => out.push_str(r#"<text y="-10.1" style="" text-anchor="middle">"#),
        (true, false) => out.push_str(r#"<text y="-10.1" style="">"#),
        (false, true) => out.push_str(r#"<text y="-10.1" text-anchor="middle">"#),
        (false, false) => out.push_str(r#"<text y="-10.1">"#),
    }
}

fn outer_tspan_class(include_row_class: bool) -> &'static str {
    if include_row_class {
        "row text-outer-tspan"
    } else {
        "text-outer-tspan"
    }
}

fn write_empty_tspan(out: &mut String, center_text: bool, include_row_class: bool) {
    let outer_class = outer_tspan_class(include_row_class);
    if center_text {
        let _ = write!(
            out,
            r#"<tspan class="{}" x="0" y="-0.1em" dy="1.1em" text-anchor="middle"/>"#,
            outer_class
        );
    } else {
        let _ = write!(
            out,
            r#"<tspan class="{}" x="0" y="-0.1em" dy="1.1em"/>"#,
            outer_class
        );
    }
}

fn open_tspan(out: &mut String, index: usize, center_text: bool, include_row_class: bool) {
    let text_anchor = if center_text {
        r#" text-anchor="middle""#
    } else {
        ""
    };
    let outer_class = outer_tspan_class(include_row_class);
    if index == 0 {
        let _ = write!(
            out,
            r#"<tspan class="{}" x="0" y="-0.1em" dy="1.1em"{}>"#,
            outer_class, text_anchor
        );
    } else {
        let y_em = if index == 1 {
            "1em".to_string()
        } else {
            format!("{:.1}em", 1.0 + (index as f64 - 1.0) * 1.1)
        };
        let _ = write!(
            out,
            r#"<tspan class="{}" x="0" y="{}" dy="1.1em"{}>"#,
            outer_class, y_em, text_anchor
        );
    }
}

fn write_svg_text_impl(
    out: &mut String,
    text: &str,
    include_style: bool,
    center_text: bool,
    include_row_class: bool,
) {
    open_svg_text(out, include_style, center_text);

    let lines = crate::text::DeterministicTextMeasurer::normalized_text_lines(text);
    if lines.len() == 1 && lines[0].is_empty() {
        write_empty_tspan(out, center_text, include_row_class);
        out.push_str("</text>");
        return;
    }

    fn split_mermaid_escaped_tag_tokens(line: &str) -> Option<Vec<String>> {
        let line = line.trim_end();
        if !line.starts_with('<') || !line.ends_with('>') {
            return None;
        }
        let open_end = line.find('>')?;
        let open_tag = &line[..=open_end];
        if open_tag.starts_with("</") {
            return None;
        }
        let tag_name = open_tag
            .trim_start_matches('<')
            .trim_end_matches('>')
            .split_whitespace()
            .next()
            .filter(|name| !name.is_empty())?;
        let close_tag = format!("</{tag_name}>");
        if !line.ends_with(&close_tag) {
            return None;
        }
        let inner = &line[open_end + 1..line.len() - close_tag.len()];
        Some(vec![
            open_tag.to_string(),
            inner.trim().to_string(),
            close_tag,
        ])
    }

    for (index, line) in lines.iter().enumerate() {
        open_tspan(out, index, center_text, include_row_class);
        let words = split_mermaid_escaped_tag_tokens(line).unwrap_or_else(|| {
            line.split_whitespace()
                .filter(|word| !word.is_empty())
                .map(str::to_string)
                .collect()
        });
        for (word_index, word) in words.iter().enumerate() {
            let _ = write!(
                out,
                r#"<tspan font-style="normal" class="text-inner-tspan" font-weight="normal">"#
            );
            if word_index == 0 {
                escape_xml_into(out, word);
            } else {
                out.push(' ');
                escape_xml_into(out, word);
            }
            out.push_str("</tspan>");
        }
        out.push_str("</tspan>");
    }

    out.push_str("</text>");
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
    out: &mut String,
    lines: &[Vec<(String, bool, bool)>],
    include_style: bool,
    center_text: bool,
    include_row_class: bool,
) {
    open_svg_text(out, include_style, center_text);

    if lines.len() == 1 && lines[0].is_empty() {
        write_empty_tspan(out, center_text, include_row_class);
        out.push_str("</text>");
        return;
    }

    for (index, words) in lines.iter().enumerate() {
        open_tspan(out, index, center_text, include_row_class);

        for (word_index, (word, is_strong, is_em)) in words.iter().enumerate() {
            let font_style = if *is_em { "italic" } else { "normal" };
            let font_weight = if *is_strong { "bold" } else { "normal" };
            let _ = write!(
                out,
                r#"<tspan font-style="{}" class="text-inner-tspan" font-weight="{}">"#,
                font_style, font_weight
            );
            if word_index == 0 {
                escape_xml_into(out, word);
            } else {
                out.push(' ');
                escape_xml_into(out, word);
            }
            out.push_str("</tspan>");
        }

        out.push_str("</tspan>");
    }

    out.push_str("</text>");
}

pub(in crate::svg::parity) fn write_svg_text_markdown(
    out: &mut String,
    markdown: &str,
    include_style: bool,
) {
    let lines = markdown_to_svg_word_lines(normalized_markdown_label(markdown));
    write_svg_text_markdown_lines(out, &lines, include_style, false, true);
}

pub(in crate::svg::parity) fn write_svg_text_markdown_centered(
    out: &mut String,
    markdown: &str,
    include_style: bool,
) {
    let lines = markdown_to_svg_word_lines(normalized_markdown_label(markdown));
    write_svg_text_markdown_lines(out, &lines, include_style, true, true);
}

pub(in crate::svg::parity) fn write_svg_text_markdown_wrapped_centered(
    out: &mut String,
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
    write_svg_text_markdown_lines(out, &lines, include_style, true, true);
}

pub(in crate::svg::parity) fn write_svg_text_markdown_wrapped(
    out: &mut String,
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
    write_svg_text_markdown_lines(out, &lines, include_style, false, true);
}

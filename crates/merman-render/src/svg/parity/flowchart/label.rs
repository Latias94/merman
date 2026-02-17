//! Flowchart label rendering helpers (HTML/SVG text).

use super::*;

pub(in crate::svg::parity) fn flowchart_label_html(
    label: &str,
    label_type: &str,
    config: &merman_core::MermaidConfig,
) -> String {
    if label.trim().is_empty() {
        return String::new();
    }

    fn xhtml_fix_fragment(input: &str) -> String {
        // `foreignObject` content lives in an XML document, so:
        // - void tags must be self-closed (`<br />`, not `<br>`)
        // - stray `<` / `>` in text must be entity-escaped (`&lt;`, `&gt;`)
        //
        // Mermaid's SVG baselines follow these rules.
        let input = input
            .replace("<br>", "<br />")
            .replace("<br/>", "<br />")
            .replace("<br >", "<br />");

        fn is_xhtml_void_tag(name: &str) -> bool {
            matches!(
                name,
                "br" | "img"
                    | "hr"
                    | "input"
                    | "meta"
                    | "link"
                    | "source"
                    | "area"
                    | "base"
                    | "col"
                    | "embed"
                    | "param"
                    | "track"
                    | "wbr"
            )
        }

        fn xhtml_self_close_void_tag(tag: &str) -> String {
            if !tag.ends_with('>') {
                return tag.to_string();
            }
            let mut inner = tag[..tag.len() - 1].to_string();
            while inner.ends_with(|c: char| c.is_whitespace()) {
                inner.pop();
            }
            if inner.ends_with('/') {
                // Normalize to `<tag ... />` (space before `/`) to match upstream SVG baselines.
                while inner.ends_with('/') {
                    inner.pop();
                }
                while inner.ends_with(|c: char| c.is_whitespace()) {
                    inner.pop();
                }
                inner.push_str(" /");
                inner.push('>');
                return inner;
            }
            inner.push_str(" /");
            inner.push('>');
            inner
        }

        let mut out = String::with_capacity(input.len());
        let mut chars = input.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '<' => {
                    let next = chars.peek().copied();
                    if !matches!(
                        next,
                        Some(n) if n.is_ascii_alphabetic() || matches!(n, '/' | '!' | '?')
                    ) {
                        out.push_str("&lt;");
                        continue;
                    }

                    let mut tag = String::from("<");
                    let mut saw_end = false;
                    for c in chars.by_ref() {
                        tag.push(c);
                        if c == '>' {
                            saw_end = true;
                            break;
                        }
                    }
                    if !saw_end {
                        out.push_str("&lt;");
                        out.push_str(&tag[1..]);
                        continue;
                    }

                    let tag_trim = tag.trim();
                    let inner = tag_trim
                        .trim_start_matches('<')
                        .trim_end_matches('>')
                        .trim();
                    let is_closing = inner.starts_with('/');
                    let name = inner
                        .trim_start_matches('/')
                        .trim_end_matches('/')
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    if !is_closing && is_xhtml_void_tag(&name) {
                        out.push_str(&xhtml_self_close_void_tag(tag_trim));
                    } else {
                        out.push_str(tag_trim);
                    }
                }
                '>' => out.push_str("&gt;"),
                '&' => {
                    // Preserve entities already encoded by the sanitizer.
                    let mut tail = String::new();
                    let mut ok = false;
                    for _ in 0..32 {
                        match chars.peek().copied() {
                            Some(';') => {
                                chars.next();
                                tail.push(';');
                                ok = true;
                                break;
                            }
                            Some(c)
                                if c.is_ascii_alphanumeric() || matches!(c, '#' | 'x' | 'X') =>
                            {
                                chars.next();
                                tail.push(c);
                            }
                            _ => break,
                        }
                    }
                    if ok {
                        out.push('&');
                        out.push_str(&tail);
                    } else {
                        out.push_str("&amp;");
                        out.push_str(&tail);
                    }
                }
                _ => out.push(ch),
            }
        }

        out
    }

    fn normalize_flowchart_img_tags(input: &str, fixed_width: bool) -> String {
        // Mermaid flowchart-v2 adds inline styles to `<img>` tags inside HTML labels to constrain
        // their layout. The SVG baseline uses XHTML, so we also self-close the tags later.
        if !input.to_ascii_lowercase().contains("<img") {
            return input.to_string();
        }

        let style = if fixed_width {
            "display: flex; flex-direction: column; min-width: 80px; max-width: 80px;"
        } else {
            "display: flex; flex-direction: column; width: 100%;"
        };

        fn extract_img_src(tag: &str) -> Option<String> {
            let lower = tag.to_ascii_lowercase();
            let idx = lower.find("src=")?;
            let rest = &tag[idx + 4..];
            let rest = rest.trim_start();
            let quote = rest.chars().next()?;
            if quote != '"' && quote != '\'' {
                return None;
            }
            let mut val = String::new();
            let mut it = rest.chars();
            let _ = it.next(); // consume quote
            for ch in it {
                if ch == quote {
                    break;
                }
                val.push(ch);
            }
            let val = val.trim().to_string();
            if val.is_empty() { None } else { Some(val) }
        }

        let mut out = String::with_capacity(input.len());
        let bytes = input.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'<' && i + 3 < bytes.len() {
                let rest = &input[i..];
                let rest_lower = rest.to_ascii_lowercase();
                if rest_lower.starts_with("<img") {
                    let Some(rel_end) = rest.find('>') else {
                        out.push_str(rest);
                        break;
                    };
                    let tag = &rest[..=rel_end];
                    let src = extract_img_src(tag);
                    out.push_str("<img");
                    if let Some(src) = src {
                        let _ = write!(out, r#" src="{}""#, escape_attr(&src));
                    }
                    out.push_str(r#" style=""#);
                    out.push_str(style);
                    out.push('"');
                    out.push('>');
                    i += rel_end + 1;
                    continue;
                }
            }
            let ch = input[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
        out
    }

    fn is_single_img_label(label: &str) -> bool {
        let t = label.trim();
        let lower = t.to_ascii_lowercase();
        if !lower.starts_with("<img") {
            return false;
        }
        let Some(end) = t.find('>') else {
            return false;
        };
        t[end + 1..].trim().is_empty()
    }

    match label_type {
        "markdown" => {
            let mut html_out = String::new();
            let parser = pulldown_cmark::Parser::new_ext(
                label,
                pulldown_cmark::Options::ENABLE_TABLES
                    | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
                    | pulldown_cmark::Options::ENABLE_TASKLISTS,
            )
            .map(|ev| match ev {
                pulldown_cmark::Event::SoftBreak => pulldown_cmark::Event::HardBreak,
                other => other,
            });
            pulldown_cmark::html::push_html(&mut html_out, parser);
            let html_out = html_out.trim().to_string();
            let html_out = crate::text::replace_fontawesome_icons(&html_out);
            xhtml_fix_fragment(&merman_core::sanitize::sanitize_text(&html_out, config))
        }
        _ => {
            let label = if label.contains("\r\n") {
                label.replace("\r\n", "\n")
            } else {
                label.to_string()
            };
            let label = if label_type == "string" {
                label.trim().to_string()
            } else {
                label
            };
            let label = label.trim_end_matches('\n');
            let wants_p = crate::text::mermaid_markdown_wants_paragraph_wrap(label);

            // Fast path for the overwhelmingly common case: plain text labels (no HTML, no
            // entities, no Mermaid icon syntax). In upstream Mermaid, these go through
            // `sanitizeText(...)` but the output is unchanged; skipping the HTML sanitizer here is
            // a large win in flowcharts with many nodes.
            if !label.contains('<')
                && !label.contains('>')
                && !label.contains('&')
                && !label.contains(":fa-")
            {
                let inner = if wants_p {
                    if label.contains('\n') {
                        label.replace('\n', "<br />")
                    } else {
                        label.to_string()
                    }
                } else {
                    label.to_string()
                };
                if wants_p {
                    return format!("<p>{inner}</p>");
                }
                return inner;
            }

            let label = if wants_p {
                label.replace('\n', "<br />")
            } else {
                label.to_string()
            };
            let fixed_img_width = is_single_img_label(&label);
            let label = normalize_flowchart_img_tags(&label, fixed_img_width);
            let wrapped = if fixed_img_width || !wants_p {
                label
            } else {
                format!("<p>{}</p>", label)
            };
            let wrapped = if wrapped.contains(":fa-") {
                crate::text::replace_fontawesome_icons(&wrapped)
            } else {
                wrapped
            };
            xhtml_fix_fragment(&merman_core::sanitize::sanitize_text(&wrapped, config))
        }
    }
}

pub(in crate::svg::parity) fn flowchart_label_plain_text(
    label: &str,
    label_type: &str,
    html_labels: bool,
) -> String {
    crate::flowchart::flowchart_label_plain_text_for_layout(label, label_type, html_labels)
}

pub(in crate::svg::parity) fn write_flowchart_svg_text(
    out: &mut String,
    text: &str,
    include_style: bool,
) {
    // Mirrors Mermaid's SVG text structure when `flowchart.htmlLabels=false`.
    if include_style {
        out.push_str(r#"<text y="-10.1" style="">"#);
    } else {
        out.push_str(r#"<text y="-10.1">"#);
    }

    let lines = crate::text::DeterministicTextMeasurer::normalized_text_lines(text);
    if lines.len() == 1 && lines[0].is_empty() {
        out.push_str(r#"<tspan class="text-outer-tspan" x="0" y="-0.1em" dy="1.1em"/>"#);
        out.push_str("</text>");
        return;
    }

    fn split_mermaid_escaped_tag_tokens(line: &str) -> Option<Vec<String>> {
        // Mermaid’s SVG text renderer tokenizes a simple HTML-tag wrapper even when htmlLabels are
        // disabled, resulting in 3 inner <tspan> runs like:
        //   `<strong>Haiya</strong>` -> `<strong>` + ` Haiya` + ` </strong>`
        // (all still rendered as escaped text).
        let line = line.trim_end();
        if !line.starts_with('<') || !line.ends_with('>') {
            return None;
        }
        let open_end = line.find('>')?;
        let open_tag = &line[..=open_end];
        if open_tag.starts_with("</") {
            return None;
        }
        let open_inner = open_tag.trim_start_matches('<').trim_end_matches('>');
        let tag_name = open_inner
            .split_whitespace()
            .next()
            .filter(|s| !s.is_empty())?;
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

    for (idx, line) in lines.iter().enumerate() {
        if idx == 0 {
            out.push_str(r#"<tspan class="text-outer-tspan" x="0" y="-0.1em" dy="1.1em">"#);
        } else {
            // Mermaid sets an absolute `y` for each subsequent line, then uses `dy="1.1em"` as
            // the line-height increment. This yields `y="1em"` for the 2nd line and `y="2.1em"`
            // for the 3rd line, etc.
            let y_em = if idx == 1 {
                "1em".to_string()
            } else {
                format!("{:.1}em", 1.0 + (idx as f64 - 1.0) * 1.1)
            };
            let _ = write!(
                out,
                r#"<tspan class="text-outer-tspan" x="0" y="{}" dy="1.1em">"#,
                y_em
            );
        }
        let words: Vec<String> = split_mermaid_escaped_tag_tokens(line).unwrap_or_else(|| {
            line.split_whitespace()
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect()
        });
        for (word_idx, word) in words.iter().enumerate() {
            out.push_str(
                r#"<tspan font-style="normal" class="text-inner-tspan" font-weight="normal">"#,
            );
            if word_idx == 0 {
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

fn markdown_to_svg_word_lines(markdown: &str) -> Vec<Vec<(String, bool, bool)>> {
    // Mirrors Mermaid's `markdownToLines(...)` + `createFormattedText(...)` behavior at a high
    // level for the subset used in Mermaid@11.12.2 flowchart baselines:
    // - words are split on whitespace
    // - each word carries `strong`/`em` style based on the active Markdown span
    // - line breaks come from hard/soft breaks and explicit `\n` in text
    let mut lines: Vec<Vec<(String, bool, bool)>> = vec![Vec::new()];

    let mut strong_depth: usize = 0;
    let mut em_depth: usize = 0;

    let mut curr = String::new();
    let mut curr_strong = false;
    let mut curr_em = false;

    let flush = |lines: &mut Vec<Vec<(String, bool, bool)>>,
                 curr: &mut String,
                 curr_strong: &mut bool,
                 curr_em: &mut bool| {
        if !curr.is_empty() {
            lines
                .last_mut()
                .unwrap_or_else(|| unreachable!("lines always has at least one entry"))
                .push((std::mem::take(curr), *curr_strong, *curr_em));
        }
        *curr_strong = false;
        *curr_em = false;
    };

    let parser = pulldown_cmark::Parser::new_ext(
        markdown,
        pulldown_cmark::Options::ENABLE_TABLES
            | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
            | pulldown_cmark::Options::ENABLE_TASKLISTS,
    )
    .map(|ev| match ev {
        pulldown_cmark::Event::SoftBreak => pulldown_cmark::Event::HardBreak,
        other => other,
    });

    for ev in parser {
        match ev {
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Strong) => {
                strong_depth += 1;
            }
            pulldown_cmark::Event::Start(pulldown_cmark::Tag::Emphasis) => {
                em_depth += 1;
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Strong) => {
                strong_depth = strong_depth.saturating_sub(1);
            }
            pulldown_cmark::Event::End(pulldown_cmark::TagEnd::Emphasis) => {
                em_depth = em_depth.saturating_sub(1);
            }
            pulldown_cmark::Event::HardBreak => {
                flush(&mut lines, &mut curr, &mut curr_strong, &mut curr_em);
                lines.push(Vec::new());
            }
            pulldown_cmark::Event::Text(t) | pulldown_cmark::Event::Code(t) => {
                for ch in t.chars() {
                    if ch == '\n' {
                        flush(&mut lines, &mut curr, &mut curr_strong, &mut curr_em);
                        lines.push(Vec::new());
                        continue;
                    }
                    if ch.is_whitespace() {
                        flush(&mut lines, &mut curr, &mut curr_strong, &mut curr_em);
                        continue;
                    }

                    let want_strong = strong_depth > 0;
                    let want_em = em_depth > 0;
                    if curr.is_empty() {
                        curr_strong = want_strong;
                        curr_em = want_em;
                    } else if curr_strong != want_strong || curr_em != want_em {
                        flush(&mut lines, &mut curr, &mut curr_strong, &mut curr_em);
                        curr_strong = want_strong;
                        curr_em = want_em;
                    }
                    curr.push(ch);
                }
            }
            pulldown_cmark::Event::Html(t) => {
                // Mermaid's SVG-label markdown path keeps raw inline HTML tokens as literal text.
                // Treat them as plain text here (whitespace-separated).
                for ch in t.chars() {
                    if ch.is_whitespace() {
                        flush(&mut lines, &mut curr, &mut curr_strong, &mut curr_em);
                        continue;
                    }
                    if curr.is_empty() {
                        curr_strong = strong_depth > 0;
                        curr_em = em_depth > 0;
                    }
                    curr.push(ch);
                }
            }
            _ => {}
        }
    }

    flush(&mut lines, &mut curr, &mut curr_strong, &mut curr_em);
    while lines.last().is_some_and(|l| l.is_empty()) && lines.len() > 1 {
        lines.pop();
    }
    lines
}

pub(in crate::svg::parity) fn write_flowchart_svg_text_markdown(
    out: &mut String,
    markdown: &str,
    include_style: bool,
) {
    if include_style {
        out.push_str(r#"<text y="-10.1" style="">"#);
    } else {
        out.push_str(r#"<text y="-10.1">"#);
    }

    let lines = markdown_to_svg_word_lines(markdown);
    if lines.len() == 1 && lines[0].is_empty() {
        out.push_str(r#"<tspan class="text-outer-tspan" x="0" y="-0.1em" dy="1.1em"/>"#);
        out.push_str("</text>");
        return;
    }

    for (idx, words) in lines.iter().enumerate() {
        if idx == 0 {
            out.push_str(r#"<tspan class="text-outer-tspan" x="0" y="-0.1em" dy="1.1em">"#);
        } else {
            let y_em = if idx == 1 {
                "1em".to_string()
            } else {
                format!("{:.1}em", 1.0 + (idx as f64 - 1.0) * 1.1)
            };
            let _ = write!(
                out,
                r#"<tspan class="text-outer-tspan" x="0" y="{}" dy="1.1em">"#,
                y_em
            );
        }

        for (word_idx, (word, is_strong, is_em)) in words.iter().enumerate() {
            let font_style = if *is_em { "italic" } else { "normal" };
            let font_weight = if *is_strong { "bold" } else { "normal" };
            let _ = write!(
                out,
                r#"<tspan font-style="{}" class="text-inner-tspan" font-weight="{}">"#,
                font_style, font_weight
            );
            if word_idx == 0 {
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

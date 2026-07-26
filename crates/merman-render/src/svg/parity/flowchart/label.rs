//! Flowchart label rendering helpers (HTML/SVG text).

use super::*;

pub(in crate::svg::parity) fn flowchart_label_html(
    label: &str,
    label_type: &str,
    config: &merman_core::MermaidConfig,
    math_renderer: Option<&(dyn crate::math::MathRenderer + Send + Sync)>,
) -> String {
    flowchart_label_html_impl(label, label_type, config, math_renderer)
}

fn flowchart_label_html_impl(
    label: &str,
    label_type: &str,
    config: &merman_core::MermaidConfig,
    math_renderer: Option<&(dyn crate::math::MathRenderer + Send + Sync)>,
) -> String {
    if label.trim().is_empty() {
        return String::new();
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
            let Some(ch) = input[i..].chars().next() else {
                break;
            };
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

    fn trim_markdown_trailing_newlines(
        input: std::borrow::Cow<'_, str>,
    ) -> std::borrow::Cow<'_, str> {
        if input.ends_with('\n') {
            std::borrow::Cow::Owned(input.trim_end_matches('\n').to_string())
        } else {
            input
        }
    }

    fn replace_non_markdown_html_line_breaks(input: &str) -> String {
        if input.contains("\\n") || input.contains('\n') {
            input.replace("\\n", "<br />").replace('\n', "<br />")
        } else {
            input.to_string()
        }
    }

    if let Some(html) = crate::math::render_math_html_label(label, config, math_renderer) {
        return html;
    }

    fn mermaid_markdown_to_html_minimal(
        markdown: &str,
        markdown_auto_wrap: bool,
        wants_p: bool,
    ) -> String {
        if !wants_p {
            return markdown.replace("\r\n", "\n");
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Ty {
            Strong,
            Em,
        }

        fn is_punctuation(ch: char) -> bool {
            !ch.is_whitespace() && !ch.is_alphanumeric()
        }

        fn mermaid_delim_can_open_close(
            ch: char,
            prev: Option<char>,
            next: Option<char>,
        ) -> (bool, bool) {
            let prev_is_ws = prev.is_none_or(|c| c.is_whitespace());
            let next_is_ws = next.is_none_or(|c| c.is_whitespace());
            let prev_is_punct = prev.is_some_and(is_punctuation);
            let next_is_punct = next.is_some_and(is_punctuation);

            let left_flanking = !next_is_ws && (!next_is_punct || prev_is_ws || prev_is_punct);
            let right_flanking = !prev_is_ws && (!prev_is_punct || next_is_ws || next_is_punct);

            if ch == '_' {
                let can_open = left_flanking && (!right_flanking || prev_is_ws || prev_is_punct);
                let can_close = right_flanking && (!left_flanking || next_is_ws || next_is_punct);
                (can_open, can_close)
            } else {
                (left_flanking, right_flanking)
            }
        }

        let s = markdown.replace("\r\n", "\n");
        let chars: Vec<char> = s.chars().collect();

        fn open_tag(ty: Ty) -> &'static str {
            match ty {
                Ty::Strong => "<strong>",
                Ty::Em => "<em>",
            }
        }

        fn close_tag(ty: Ty) -> &'static str {
            match ty {
                Ty::Strong => "</strong>",
                Ty::Em => "</em>",
            }
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        struct Delim {
            ty: Ty,
            ch: char,
            run_len: usize,
            token_index: usize,
        }

        let mut tokens: Vec<String> = Vec::with_capacity(16);
        tokens.push("<p>".to_string());

        let mut text_buf = String::new();
        let flush_text = |tokens: &mut Vec<String>, text_buf: &mut String| {
            if !text_buf.is_empty() {
                tokens.push(std::mem::take(text_buf));
            }
        };

        let mut stack: Vec<Delim> = Vec::new();

        let mut i = 0usize;
        while i < chars.len() {
            let ch = chars[i];

            if ch == '\n' {
                let mut j = i;
                while j < chars.len() && chars[j] == '\n' {
                    j += 1;
                }
                let newline_count = j - i;

                if newline_count >= 2 {
                    while text_buf.ends_with(' ') {
                        text_buf.pop();
                    }
                    flush_text(&mut tokens, &mut text_buf);
                    tokens.push("</p><p>".to_string());
                    i = j;
                    while i < chars.len() && chars[i] == ' ' {
                        i += 1;
                    }
                    continue;
                }

                flush_text(&mut tokens, &mut text_buf);
                tokens.push("<br/>".to_string());
                i += 1;
                while i < chars.len() && chars[i] == ' ' {
                    i += 1;
                }
                continue;
            }

            if ch == '<'
                && let Some(end_rel) = chars[i..].iter().position(|c| *c == '>')
            {
                let end = i + end_rel;
                flush_text(&mut tokens, &mut text_buf);
                let mut tag = String::new();
                for c in &chars[i..=end] {
                    tag.push(*c);
                }
                tokens.push(tag);
                i = end + 1;
                continue;
            }

            if ch == '*' || ch == '_' {
                let run_len = if i + 1 < chars.len() && chars[i + 1] == ch {
                    2
                } else {
                    1
                };
                let want = if run_len == 2 { Ty::Strong } else { Ty::Em };
                let prev = if i > 0 { Some(chars[i - 1]) } else { None };
                let next = if i + run_len < chars.len() {
                    Some(chars[i + run_len])
                } else {
                    None
                };
                let (can_open, can_close) = mermaid_delim_can_open_close(ch, prev, next);

                flush_text(&mut tokens, &mut text_buf);

                let delim_text: String = std::iter::repeat_n(ch, run_len).collect();

                if can_close
                    && stack
                        .last()
                        .is_some_and(|d| d.ty == want && d.ch == ch && d.run_len == run_len)
                    && let Some(opener) = stack.pop()
                {
                    tokens[opener.token_index] = open_tag(want).to_string();
                    tokens.push(close_tag(want).to_string());
                    i += run_len;
                    continue;
                }
                if can_open {
                    let token_index = tokens.len();
                    tokens.push(delim_text);
                    stack.push(Delim {
                        ty: want,
                        ch,
                        run_len,
                        token_index,
                    });
                    i += run_len;
                    continue;
                }

                tokens.push(delim_text);
                i += run_len;
                continue;
            }

            if ch == ' ' && !markdown_auto_wrap {
                text_buf.push_str("&nbsp;");
            } else {
                text_buf.push(ch);
            }
            i += 1;
        }

        while text_buf.ends_with(' ') {
            text_buf.pop();
        }
        flush_text(&mut tokens, &mut text_buf);
        tokens.push("</p>".to_string());
        tokens.concat()
    }

    match label_type {
        "markdown" => {
            let decoded = decode_mermaid_entities_for_render_text(label);
            let decoded = if decoded.contains("\\\\") {
                std::borrow::Cow::Owned(decoded.replace("\\\\", "\\"))
            } else {
                decoded
            };
            let decoded = trim_markdown_trailing_newlines(decoded);
            let markdown_auto_wrap = config
                .as_value()
                .get("markdownAutoWrap")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true);
            let html_out = if crate::text::mermaid_markdown_contains_raw_blocks(decoded.as_ref()) {
                crate::text::mermaid_markdown_to_html_label_fragment(
                    decoded.as_ref(),
                    markdown_auto_wrap,
                )
            } else {
                let wants_p = crate::text::mermaid_markdown_wants_paragraph_wrap(decoded.as_ref());
                mermaid_markdown_to_html_minimal(decoded.as_ref(), markdown_auto_wrap, wants_p)
            };
            let html_out = html_out.trim().to_string();
            let html_out = crate::text::replace_fontawesome_icons(&html_out);
            crate::xml::normalize_html_fragment_for_xhtml(&merman_core::sanitize::sanitize_text(
                &html_out, config,
            ))
        }
        _ => {
            let label = if label.contains("\r\n") {
                label.replace("\r\n", "\n")
            } else {
                label.to_string()
            };
            let label = crate::flowchart::flowchart_decode_label_escapes(&label);
            let label = if label_type == "string" {
                label.trim().to_string()
            } else {
                label
            };
            let label = label.trim_end_matches('\n');
            let label = crate::flowchart::flowchart_normalize_plain_multiline_label_for_html(label);
            let label = label.as_ref();

            // Fast path for the overwhelmingly common case: plain text labels (no HTML, no
            // entities, no Mermaid icon syntax). In upstream Mermaid, these go through
            // `sanitizeText(...)` but the output is unchanged; skipping the HTML sanitizer here is
            // a large win in flowcharts with many nodes.
            if !label.contains('<')
                && !label.contains('>')
                && !label.contains('&')
                && !label.contains(":fa-")
            {
                let inner = replace_non_markdown_html_line_breaks(label);
                return format!("<p>{inner}</p>");
            }

            // Mermaid's nonMarkdownToHTML() wraps every non-empty label in one paragraph.
            // Markdown block classification must not leak into this branch.
            let label = replace_non_markdown_html_line_breaks(label);
            let fixed_img_width = is_single_img_label(&label);
            let label = normalize_flowchart_img_tags(&label, fixed_img_width);
            let wrapped = format!("<p>{}</p>", label);
            let wrapped = if wrapped.contains(":fa-") {
                crate::text::replace_fontawesome_icons(&wrapped)
            } else {
                wrapped
            };
            crate::xml::normalize_html_fragment_for_xhtml(&merman_core::sanitize::sanitize_text(
                &wrapped, config,
            ))
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
    crate::svg::parity::label::write_svg_text(out, text, include_style);
}

pub(in crate::svg::parity) fn write_flowchart_svg_text_centered(
    out: &mut String,
    text: &str,
    include_style: bool,
) {
    crate::svg::parity::label::write_svg_text_centered(out, text, include_style);
}

pub(in crate::svg::parity) fn write_flowchart_svg_text_markdown(
    out: &mut String,
    markdown: &str,
    include_style: bool,
) {
    crate::svg::parity::label::write_svg_text_markdown(out, markdown, include_style);
}

pub(in crate::svg::parity) fn write_flowchart_svg_text_markdown_wrapped_centered(
    out: &mut String,
    markdown: &str,
    include_style: bool,
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
    max_width_px: Option<f64>,
) {
    crate::svg::parity::label::write_svg_text_markdown_wrapped_centered(
        out,
        markdown,
        include_style,
        measurer,
        style,
        max_width_px,
    );
}

pub(in crate::svg::parity) fn write_flowchart_svg_text_markdown_wrapped(
    out: &mut String,
    markdown: &str,
    include_style: bool,
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
    max_width_px: Option<f64>,
) {
    crate::svg::parity::label::write_svg_text_markdown_wrapped(
        out,
        markdown,
        include_style,
        measurer,
        style,
        max_width_px,
    );
}

//! Mermaid HTML/XHTML label fragment helpers.

use super::{
    is_ecmascript_whitespace, is_html_collapsible_ascii_whitespace,
    trim_html_collapsible_ascii_whitespace,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MermaidMarkdownBlockKind {
    Paragraph,
    Html,
    IndentedCode,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MermaidMarkdownBlock {
    kind: MermaidMarkdownBlockKind,
    range: std::ops::Range<usize>,
}

fn is_unformatted_ascii_text(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b' ')
}

fn mermaid_markdown_block_options() -> pulldown_cmark::Options {
    pulldown_cmark::Options::ENABLE_TABLES
        | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
        | pulldown_cmark::Options::ENABLE_TASKLISTS
}

fn marked_16_3_accepts_html_block(raw: &str) -> bool {
    let first_line = raw.lines().next().unwrap_or(raw);
    let first_line = first_line.strip_prefix("   ").unwrap_or(first_line);
    let first_line = first_line.trim_start_matches(' ');

    // Marked's block HTML grammar accepts complete tags on the opening line plus the CommonMark
    // special forms below. Pulldown-cmark also classifies an incomplete opener such as `<div` as
    // HtmlBlock; pinned marked 16.3.0 keeps that input in a paragraph.
    first_line.contains('>')
        || first_line.starts_with("<!--")
        || first_line.starts_with("<?")
        || first_line.starts_with("<![CDATA[")
        || first_line
            .strip_prefix("<!")
            .and_then(|rest| rest.chars().next())
            .is_some_and(|ch| ch.is_ascii_uppercase())
}

fn mermaid_markdown_top_level_blocks(markdown: &str) -> Vec<MermaidMarkdownBlock> {
    use pulldown_cmark::{CodeBlockKind, Event, Tag};

    let mut blocks = Vec::new();
    let mut depth = 0usize;
    let parser = pulldown_cmark::Parser::new_ext(markdown, mermaid_markdown_block_options());
    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(tag) => {
                if depth == 0 {
                    let kind = match tag {
                        Tag::Paragraph => MermaidMarkdownBlockKind::Paragraph,
                        Tag::HtmlBlock
                            if marked_16_3_accepts_html_block(&markdown[range.clone()]) =>
                        {
                            MermaidMarkdownBlockKind::Html
                        }
                        Tag::HtmlBlock => MermaidMarkdownBlockKind::Paragraph,
                        Tag::CodeBlock(CodeBlockKind::Indented) => {
                            MermaidMarkdownBlockKind::IndentedCode
                        }
                        _ => MermaidMarkdownBlockKind::Unsupported,
                    };
                    let range = if kind == MermaidMarkdownBlockKind::IndentedCode {
                        let line_start = markdown[..range.start]
                            .rfind('\n')
                            .map_or(0, |newline| newline + 1);
                        // Marked keeps an EOF whitespace-only continuation in the raw indented
                        // code token. That spelling is observable after Mermaid switches the HTML
                        // label to `white-space: break-spaces` (for example, Mindmap delimiter
                        // indentation can own a final line box).
                        let range_end = if markdown[range.end..]
                            .chars()
                            .all(is_html_collapsible_ascii_whitespace)
                        {
                            markdown.len()
                        } else {
                            range.end
                        };
                        line_start..range_end
                    } else {
                        range
                    };
                    blocks.push(MermaidMarkdownBlock { kind, range });
                }
                depth += 1;
            }
            Event::End(_) => depth = depth.saturating_sub(1),
            Event::Rule if depth == 0 => blocks.push(MermaidMarkdownBlock {
                kind: MermaidMarkdownBlockKind::Unsupported,
                range,
            }),
            Event::Html(_) if depth == 0 => blocks.push(MermaidMarkdownBlock {
                kind: MermaidMarkdownBlockKind::Html,
                range,
            }),
            _ => {}
        }
    }
    blocks
}

pub(crate) fn mermaid_markdown_contains_raw_blocks(markdown: &str) -> bool {
    let markdown = markdown.replace("\r\n", "\n");
    mermaid_markdown_top_level_blocks(&markdown)
        .iter()
        .any(|block| block.kind != MermaidMarkdownBlockKind::Paragraph)
}

fn mermaid_markdown_paragraph_to_fragment(
    label: &str,
    markdown_auto_wrap: bool,
    xhtml: bool,
) -> String {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Ty {
        Strong,
        Em,
    }

    fn is_punctuation(ch: char) -> bool {
        !is_ecmascript_whitespace(ch) && !ch.is_alphanumeric()
    }

    fn mermaid_delim_can_open_close(
        ch: char,
        prev: Option<char>,
        next: Option<char>,
    ) -> (bool, bool) {
        let prev_is_ws = prev.is_none_or(is_ecmascript_whitespace);
        let next_is_ws = next.is_none_or(is_ecmascript_whitespace);
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

    let s = label.replace("\r\n", "\n");
    let chars: Vec<char> = s.chars().collect();
    let mut tokens: Vec<String> = Vec::with_capacity(16);
    tokens.push("<p>".to_string());

    let mut text_buf = String::new();
    let flush_text = |tokens: &mut Vec<String>, text_buf: &mut String| {
        if text_buf.is_empty() {
            return;
        }
        let raw = std::mem::take(text_buf);
        tokens.push(if xhtml {
            escape_xml_text_preserving_entities(&raw)
        } else {
            raw
        });
    };

    let mut stack: Vec<Delim> = Vec::new();
    let mut in_code_span = false;
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];

        if ch == '\n' && in_code_span {
            text_buf.push(ch);
            i += 1;
            continue;
        }

        if ch == '\n' {
            while text_buf.ends_with(' ') {
                text_buf.pop();
            }
            flush_text(&mut tokens, &mut text_buf);
            tokens.push("<br/>".to_string());
            i += 1;
            while i < chars.len() && chars[i] == ' ' {
                i += 1;
            }
            continue;
        }

        if ch == '`' {
            text_buf.push(ch);
            in_code_span = !in_code_span;
            i += 1;
            continue;
        }

        if in_code_span {
            if ch == ' ' && !markdown_auto_wrap {
                text_buf.push_str("&nbsp;");
            } else {
                text_buf.push(ch);
            }
            i += 1;
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
            if tag.eq_ignore_ascii_case("<br>")
                || tag.eq_ignore_ascii_case("<br/>")
                || tag.eq_ignore_ascii_case("<br />")
                || tag.eq_ignore_ascii_case("</br>")
                || tag.eq_ignore_ascii_case("</br/>")
                || tag.eq_ignore_ascii_case("</br />")
                || tag.eq_ignore_ascii_case("</br >")
            {
                tokens.push(if xhtml { "<br/>" } else { "<br />" }.to_string());
            } else {
                tokens.push(tag);
            }
            i = end + 1;
            continue;
        }

        if (ch == '*' || ch == '_')
            && i + 2 < chars.len()
            && chars[i + 1] == ch
            && chars[i + 2] == ch
        {
            let prev = if i > 0 { Some(chars[i - 1]) } else { None };
            let next = chars.get(i + 3).copied();
            let (can_open, can_close) = mermaid_delim_can_open_close(ch, prev, next);
            flush_text(&mut tokens, &mut text_buf);

            let closes_triple = stack.len() >= 2
                && stack.last().is_some_and(|delimiter| {
                    delimiter.ty == Ty::Strong && delimiter.ch == ch && delimiter.run_len == 3
                })
                && stack.get(stack.len() - 2).is_some_and(|delimiter| {
                    delimiter.ty == Ty::Em && delimiter.ch == ch && delimiter.run_len == 3
                });
            if can_close && closes_triple {
                let strong = stack.pop().expect("triple strong delimiter");
                let em = stack.pop().expect("triple emphasis delimiter");
                tokens[em.token_index] = open_tag(Ty::Em).to_string();
                tokens[strong.token_index] = open_tag(Ty::Strong).to_string();
                tokens.push(close_tag(Ty::Strong).to_string());
                tokens.push(close_tag(Ty::Em).to_string());
                i += 3;
                continue;
            }
            if can_open {
                let em_token_index = tokens.len();
                tokens.push(ch.to_string());
                stack.push(Delim {
                    ty: Ty::Em,
                    ch,
                    run_len: 3,
                    token_index: em_token_index,
                });
                let strong_token_index = tokens.len();
                tokens.push(std::iter::repeat_n(ch, 2).collect());
                stack.push(Delim {
                    ty: Ty::Strong,
                    ch,
                    run_len: 3,
                    token_index: strong_token_index,
                });
                i += 3;
                continue;
            }

            tokens.push(std::iter::repeat_n(ch, 3).collect());
            i += 3;
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
            if ch == '*' && can_close {
                if run_len == 1
                    && stack
                        .last()
                        .is_some_and(|d| d.ty == Ty::Strong && d.ch == '*' && d.run_len == 2)
                    && let Some(opener) = stack.pop()
                {
                    tokens[opener.token_index] = format!("*{}", open_tag(Ty::Em));
                    tokens.push(close_tag(Ty::Em).to_string());
                    i += 1;
                    continue;
                }
                if run_len == 2
                    && stack
                        .last()
                        .is_some_and(|d| d.ty == Ty::Em && d.ch == '*' && d.run_len == 1)
                    && let Some(opener) = stack.pop()
                {
                    tokens[opener.token_index] = open_tag(Ty::Em).to_string();
                    tokens.push(close_tag(Ty::Em).to_string());
                    tokens.push("*".to_string());
                    i += 2;
                    continue;
                }
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

fn mermaid_collapse_raw_html_label_text(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    let mut pending_space = false;
    for ch in markdown.chars() {
        if is_html_collapsible_ascii_whitespace(ch) {
            pending_space = true;
            continue;
        }
        if pending_space && !out.is_empty() {
            out.push(' ');
        }
        pending_space = false;
        out.push(ch);
    }
    trim_html_collapsible_ascii_whitespace(&out).to_string()
}

fn mermaid_markdown_to_label_fragment(
    markdown: &str,
    markdown_auto_wrap: bool,
    xhtml: bool,
) -> String {
    let markdown = markdown.replace("\r\n", "\n");
    if markdown.is_empty() {
        return String::new();
    }
    // This grammar cannot open any enabled Markdown construct and needs no HTML/XML escaping.
    if markdown_auto_wrap && is_unformatted_ascii_text(&markdown) {
        let mut out = String::with_capacity(markdown.len() + "<p></p>".len());
        out.push_str("<p>");
        out.push_str(&markdown);
        out.push_str("</p>");
        return out;
    }

    let mut out = String::new();
    for block in mermaid_markdown_top_level_blocks(&markdown) {
        let raw = &markdown[block.range];
        match block.kind {
            MermaidMarkdownBlockKind::Paragraph => {
                out.push_str(&mermaid_markdown_paragraph_to_fragment(
                    raw.trim_end_matches(['\r', '\n']),
                    markdown_auto_wrap,
                    xhtml,
                ));
            }
            MermaidMarkdownBlockKind::Html => out.push_str(raw.trim_end_matches(['\r', '\n'])),
            MermaidMarkdownBlockKind::IndentedCode => {
                let raw = raw.trim_end_matches(['\r', '\n']);
                if xhtml {
                    out.push_str(&escape_xml_text_preserving_entities(raw));
                } else {
                    out.push_str(raw);
                }
            }
            MermaidMarkdownBlockKind::Unsupported => {
                let raw = mermaid_collapse_raw_html_label_text(raw);
                if xhtml {
                    out.push_str(&escape_xml_text_preserving_entities(&raw));
                } else {
                    out.push_str(&raw);
                }
            }
        }
    }

    out
}

/// Approximate the final browser DOM fragment that Mermaid HTML labels produce for Markdown.
pub(crate) fn mermaid_markdown_to_html_label_fragment(
    markdown: &str,
    markdown_auto_wrap: bool,
) -> String {
    mermaid_markdown_to_label_fragment(markdown, markdown_auto_wrap, false)
}
fn escape_xml_text_preserving_entities(raw: &str) -> String {
    fn escape_xml_segment(out: &mut String, raw: &str) {
        for ch in raw.chars() {
            if !crate::xml::is_xml_1_0_char(ch) {
                continue;
            }
            match ch {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                _ => out.push(ch),
            }
        }
    }

    let raw = crate::xml::normalize_html_entities_for_xml(raw);
    let raw = raw.as_ref();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0usize;
    while let Some(rel) = raw[i..].find('&') {
        let amp = i + rel;
        escape_xml_segment(&mut out, &raw[i..amp]);
        let tail = &raw[amp + 1..];
        if let Some(semi_rel) = tail.find(';') {
            let semi = amp + 1 + semi_rel;
            let entity = &raw[amp + 1..semi];
            if crate::xml::is_valid_xml_entity_reference(entity) {
                out.push('&');
                out.push_str(entity);
                out.push(';');
                i = semi + 1;
                continue;
            }
        }
        out.push_str("&amp;");
        i = amp + 1;
    }
    escape_xml_segment(&mut out, &raw[i..]);
    out
}

/// XHTML-safe projection for direct `<foreignObject>` insertion.
pub(crate) fn mermaid_markdown_to_xhtml_label_fragment(
    markdown: &str,
    markdown_auto_wrap: bool,
) -> String {
    mermaid_markdown_to_label_fragment(markdown, markdown_auto_wrap, true)
}

/// Returns the rendered text when an XHTML label fragment is structurally plain.
///
/// Mermaid's HTML-label path always wraps ordinary inline text in one `<p>`. Parsing the
/// generated fragment lets callers preserve the host measurer's native precision for that common
/// case without guessing from Markdown punctuation. Any nested element, additional root node, or
/// malformed fragment remains on the structured HTML path.
pub(crate) fn mermaid_xhtml_label_plain_text(fragment: &str) -> Option<String> {
    let wrapped = format!("<merman-fragment>{fragment}</merman-fragment>");
    let document = roxmltree::Document::parse(&wrapped).ok()?;
    let root = document.root_element();

    let mut children = root.children();
    let paragraph = children.next()?;
    if children.next().is_some()
        || !paragraph.is_element()
        || paragraph.tag_name().name() != "p"
        || paragraph.children().any(|child| !child.is_text())
    {
        return None;
    }

    Some(
        paragraph
            .children()
            .filter_map(|child| child.text())
            .collect(),
    )
}

pub(crate) fn mermaid_xhtml_label_text_content(fragment: &str) -> Option<String> {
    let wrapped = format!("<merman-fragment>{fragment}</merman-fragment>");
    let document = roxmltree::Document::parse(&wrapped).ok()?;
    Some(
        document
            .root_element()
            .descendants()
            .filter_map(|node| node.text().filter(|_| node.is_text()))
            .collect(),
    )
}

/// Whether Mermaid's upstream `markdownToHTML()` would wrap the first top-level token in
/// `<p>...</p>` when `htmlLabels=true`.
///
/// Mermaid 11.16 uses `marked@16.3.0` and only explicitly formats a small subset of
/// token types (`paragraph`, `strong`, `em`, `text`, `html`, `escape`). For unsupported *block*
/// tokens (e.g. ordered/unordered lists, headings, fenced code blocks), Mermaid falls back to
/// emitting the raw Markdown without a surrounding `<p>` wrapper.
///
/// This function projects pulldown-cmark's CommonMark/GFM block parser down to only the top-level
/// paragraph decision. Inline rendering remains owned by the pinned Marked-compatible interpreter;
/// its ambiguous delimiter-run path may use pulldown-cmark events while preserving source slices.
pub(crate) fn mermaid_markdown_wants_paragraph_wrap(markdown: &str) -> bool {
    let markdown = markdown.replace("\r\n", "\n");
    mermaid_markdown_top_level_blocks(&markdown)
        .first()
        .is_some_and(|block| block.kind == MermaidMarkdownBlockKind::Paragraph)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unformatted_ascii_projection_is_an_exact_paragraph() {
        for text in [
            "Root",
            "Long history",
            "Release 12",
            "ASCII 123",
            "Two  spaces",
        ] {
            assert!(is_unformatted_ascii_text(text));
            assert_eq!(
                mermaid_markdown_to_html_label_fragment(text, true),
                format!("<p>{text}</p>")
            );
            assert_eq!(
                mermaid_markdown_to_xhtml_label_fragment(text, true),
                format!("<p>{text}</p>")
            );
        }

        for text in [
            "",
            " leading",
            "trailing ",
            "two\nlines",
            "# heading",
            "Release 1.2",
            "alpha/beta",
            "it's ready",
            "中文标签",
            "<strong>html</strong>",
            "Fish & Chips",
            "fa:fa-user",
            "$x$",
        ] {
            assert!(!is_unformatted_ascii_text(text));
        }
    }

    #[test]
    fn block_projection_matches_pinned_marked_16_3_0_top_level_tokens() {
        // Mermaid 11.16 pins marked 16.3.0. These cases were captured from marked.lexer():
        // heading, table, html, blockquote, list, code, code, hr, and paragraph respectively.
        for markdown in [
            "Heading\n=======",
            "a | b\n--|--\n1 | 2",
            "<div>alpha</div>",
            "> quoted\n> next",
            "- first\n- second",
            "    first line\n    second line",
            "```js\nlet x = 1;\n```",
            "***",
        ] {
            assert!(
                mermaid_markdown_contains_raw_blocks(markdown),
                "expected a non-paragraph top-level token for {markdown:?}"
            );
            assert!(
                !mermaid_markdown_wants_paragraph_wrap(markdown),
                "expected no paragraph wrapper for {markdown:?}"
            );
        }

        assert!(!mermaid_markdown_contains_raw_blocks("alpha\nbeta"));
        assert!(mermaid_markdown_wants_paragraph_wrap("alpha\nbeta"));

        // Marked keeps these malformed block starters as ordinary paragraphs.
        for markdown in [
            "<div",
            "Heading\n==x==",
            "a|b\n--|x\n1|2",
            "1.not a list",
            "[unterminated",
        ] {
            assert!(
                !mermaid_markdown_contains_raw_blocks(markdown),
                "expected a paragraph token for {markdown:?}"
            );
            assert!(
                mermaid_markdown_wants_paragraph_wrap(markdown),
                "expected a paragraph wrapper for {markdown:?}"
            );
        }

        assert_eq!(
            mermaid_markdown_to_html_label_fragment("Heading\n=======", true),
            "Heading ======="
        );
        assert_eq!(
            mermaid_markdown_to_html_label_fragment("a | b\n--|--\n1 | 2", true),
            "a | b --|-- 1 | 2"
        );
        assert_eq!(
            mermaid_markdown_to_html_label_fragment("<div>alpha</div>", true),
            "<div>alpha</div>"
        );
    }

    #[test]
    fn html_label_fragment_collapses_mixed_list_blocks_like_browser_dom() {
        let input = "Hello\n  - l1\n  - l2";
        assert!(mermaid_markdown_contains_raw_blocks(input));
        assert_eq!(
            mermaid_markdown_to_html_label_fragment(input, true),
            "<p>Hello</p>- l1 - l2"
        );
    }

    #[test]
    fn xhtml_label_requires_atx_heading_whitespace() {
        assert_eq!(
            mermaid_markdown_to_xhtml_label_fragment("#int protectedMarmoset", true),
            "<p>#int protectedMarmoset</p>"
        );
        assert_eq!(
            mermaid_markdown_to_xhtml_label_fragment("# heading", true),
            "# heading"
        );
    }

    #[test]
    fn xhtml_label_fragment_preserves_inline_br_listish_continuations() {
        let input = "Hello<br/>- l1<br/>- l2";
        assert_eq!(
            mermaid_markdown_to_xhtml_label_fragment(input, true),
            "<p>Hello<br/>- l1<br/>- l2</p>"
        );
    }

    #[test]
    fn xhtml_label_fragment_normalizes_raw_br_variants() {
        let input = "Hello<br>world";
        assert_eq!(
            mermaid_markdown_to_xhtml_label_fragment(input, true),
            "<p>Hello<br/>world</p>"
        );
    }

    #[test]
    fn html_label_fragment_preserves_inline_code_literals() {
        let input = "inline: `**not bold**`";
        assert_eq!(
            mermaid_markdown_to_html_label_fragment(input, true),
            "<p>inline: `**not bold**`</p>"
        );
    }

    #[test]
    fn html_label_fragment_preserves_marked_indented_code_tokens() {
        let input = "    first line\n    second line";
        assert_eq!(mermaid_markdown_to_html_label_fragment(input, true), input);

        let trailing_indentation = "    first line\n    second line\n  ";
        assert_eq!(
            mermaid_markdown_to_html_label_fragment(trailing_indentation, true),
            trailing_indentation
        );
    }

    #[test]
    fn html_label_fragment_preserves_newlines_inside_code_spans() {
        let input = "`first line\nsecond line`";
        assert_eq!(
            mermaid_markdown_to_html_label_fragment(input, true),
            "<p>`first line\nsecond line`</p>"
        );
        assert_eq!(
            mermaid_markdown_to_xhtml_label_fragment(input, true),
            "<p>`first line\nsecond line`</p>"
        );
    }

    #[test]
    fn xhtml_label_fragment_preserves_marked_indented_code_tokens() {
        let input = "    first line\n    second line";
        assert_eq!(mermaid_markdown_to_xhtml_label_fragment(input, true), input);
    }

    #[test]
    fn xhtml_label_fragment_preserves_inline_code_literals() {
        let input = "inline: `**not bold**`";
        assert_eq!(
            mermaid_markdown_to_xhtml_label_fragment(input, true),
            "<p>inline: `**not bold**`</p>"
        );
    }

    #[test]
    fn html_label_fragment_reinterprets_partial_star_strong_like_mermaid() {
        let input = "+inline: **bold*";
        assert_eq!(
            mermaid_markdown_to_html_label_fragment(input, true),
            "<p>+inline: *<em>bold</em></p>"
        );
    }

    #[test]
    fn xhtml_label_fragment_reinterprets_partial_star_strong_like_mermaid() {
        let input = "+inline: **bold*";
        assert_eq!(
            mermaid_markdown_to_xhtml_label_fragment(input, true),
            "<p>+inline: *<em>bold</em></p>"
        );
    }

    #[test]
    fn xhtml_label_fragment_nests_triple_emphasis_like_marked() {
        assert_eq!(
            mermaid_markdown_to_xhtml_label_fragment("***The license #***", true),
            "<p><em><strong>The license #</strong></em></p>"
        );
        assert_eq!(
            mermaid_markdown_to_html_label_fragment("___The license #___", true),
            "<p><em><strong>The license #</strong></em></p>"
        );
    }

    #[test]
    fn xhtml_plain_text_is_decided_from_the_rendered_structure() {
        assert_eq!(
            mermaid_xhtml_label_plain_text("<p>Generic&lt;T&gt; driver_license</p>").as_deref(),
            Some("Generic<T> driver_license")
        );
        assert_eq!(
            mermaid_xhtml_label_plain_text("<p><a href='/'><code>Entity</code></a></p>"),
            None
        );
        assert_eq!(
            mermaid_xhtml_label_plain_text("<p>first<br/>second</p>"),
            None
        );
        assert_eq!(
            mermaid_xhtml_label_plain_text("<p>first</p><p>second</p>"),
            None
        );
        assert_eq!(
            mermaid_xhtml_label_text_content("<p><em>Result&lt;<strong>T</strong>&gt;</em></p>")
                .as_deref(),
            Some("Result<T>")
        );
    }
}

use super::TitleKind;

pub(super) fn title_kind_str(kind: &TitleKind) -> &'static str {
    match kind {
        TitleKind::Text => "text",
        TitleKind::String => "string",
        TitleKind::Markdown => "markdown",
    }
}

pub(super) fn unquote(s: &str) -> String {
    let bytes = s.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
        return s[1..s.len() - 1].to_string();
    }
    s.to_string()
}

pub(super) fn is_ecmascript_trim_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{0009}'
            | '\u{000a}'
            | '\u{000b}'
            | '\u{000c}'
            | '\u{000d}'
            | '\u{0020}'
            | '\u{00a0}'
            | '\u{1680}'
            | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

pub(super) fn trim_flowdb_label_text(text: &str) -> &str {
    // Mermaid FlowDB calls JavaScript String.trim() on parser-produced text before sanitizing or
    // decoding HTML entities. Keep the exact ECMAScript trim set: Rust's str::trim() differs for
    // U+0085 and U+FEFF, and decoding first would erase the distinction between direct NBSP and
    // an `&nbsp;` entity.
    text.trim_matches(is_ecmascript_trim_char)
}

pub(super) fn parse_label_text(raw: &str) -> (String, TitleKind) {
    // Jison classifies TEXT / STR / MD_STR before FlowDB trims the parser payload. Do not trim
    // `raw` before deciding whether the whole token is a quoted string: surrounding whitespace
    // produces a mixed token sequence upstream and must not be reinterpreted as STR here.
    let quoted = raw.starts_with('"') && raw.ends_with('"');
    let unquoted = if quoted {
        // Mermaid flowchart quoted labels are treated as raw text with surrounding quotes stripped.
        // Do not interpret backslash escapes here: fixtures rely on sequences like `\\n`, `\\t`,
        // `\\nabla`, and Windows paths (e.g. `C:\\Temp\\...`) being preserved verbatim.
        unquote(raw)
    } else {
        raw.to_string()
    };

    if quoted {
        // Mermaid's MD_STR token is only reachable from the double-quoted string state. A bare
        // backtick is ordinary TEXT and must remain visible in the label.
        let (no_backticks, is_markdown) = strip_wrapping_backticks(&unquoted);
        if is_markdown {
            return (
                trim_flowdb_label_text(&no_backticks).to_string(),
                TitleKind::Markdown,
            );
        }
        return (
            trim_flowdb_label_text(&unquoted).to_string(),
            TitleKind::String,
        );
    }
    (
        trim_flowdb_label_text(&unquoted).to_string(),
        TitleKind::Text,
    )
}

pub(super) fn parse_edge_label_text(raw: &str) -> (String, TitleKind) {
    let quoted = raw.starts_with('"') && raw.ends_with('"');

    if quoted {
        return parse_label_text(raw);
    }

    // Mermaid flowchart edge labels only enter Markdown-string mode via the lexer's `MD_STR`
    // token, i.e. a double-quoted string whose payload is wrapped in backticks:
    //   -- "`edge **label**`" -->
    //
    // Bare pipe labels like `-->|`edge **label**`|` keep the backticks literally and stay `text`.
    (trim_flowdb_label_text(raw).to_string(), TitleKind::Text)
}

pub(super) fn strip_wrapping_backticks(s: &str) -> (String, bool) {
    if s.len() >= 2 && s.starts_with('`') && s.ends_with('`') {
        return (s[1..s.len() - 1].to_string(), true);
    }
    (s.to_string(), false)
}

#[cfg(test)]
mod tests {
    use super::{parse_edge_label_text, parse_label_text};
    use crate::diagrams::flowchart::TitleKind;

    #[test]
    fn parse_label_text_keeps_backslashes_in_string_labels() {
        let (text, kind) = parse_label_text(r#""Path: C:\\Temp\\merman\\out.svg (Windows-style)""#);
        assert_eq!(kind, TitleKind::String);
        assert_eq!(text, r#"Path: C:\\Temp\\merman\\out.svg (Windows-style)"#);
    }

    #[test]
    fn parse_label_text_does_not_treat_tex_commands_as_escapes() {
        let (text, kind) = parse_label_text(r#""$$\nabla\therefore\alpha$$""#);
        assert_eq!(kind, TitleKind::String);
        assert_eq!(text, r#"$$\nabla\therefore\alpha$$"#);
    }

    #[test]
    fn parse_label_text_keeps_single_quotes_as_text() {
        let (text, kind) = parse_label_text("'Literal quotes'");
        assert_eq!(kind, TitleKind::Text);
        assert_eq!(text, "'Literal quotes'");
    }

    #[test]
    fn parse_label_text_keeps_bare_backticks_as_text() {
        let (text, kind) = parse_label_text("bare `tick` text");
        assert_eq!(kind, TitleKind::Text);
        assert_eq!(text, "bare `tick` text");
    }

    #[test]
    fn parse_edge_label_text_keeps_unquoted_backticks_literal() {
        let (text, kind) =
            parse_edge_label_text(r#"`This is **bold** </br>and <strong>strong</strong>`"#);
        assert_eq!(kind, TitleKind::Text);
        assert_eq!(
            text,
            r#"`This is **bold** </br>and <strong>strong</strong>`"#
        );
    }

    #[test]
    fn parse_edge_label_text_keeps_unquoted_partial_markdown_literal() {
        let (text, kind) = parse_edge_label_text(r#"`**bold*`"#);
        assert_eq!(kind, TitleKind::Text);
        assert_eq!(text, r#"`**bold*`"#);
    }

    #[test]
    fn parse_edge_label_text_supports_quoted_markdown_strings() {
        let (text, kind) = parse_edge_label_text(r#""`Bold **edge label**`""#);
        assert_eq!(kind, TitleKind::Markdown);
        assert_eq!(text, r#"Bold **edge label**"#);
    }

    #[test]
    fn flowdb_label_trim_preserves_entity_provenance_for_nodes_and_edges() {
        let cases = [
            ("\u{00a0}A\u{00a0}", "A", TitleKind::Text),
            ("\"\u{00a0}A\u{00a0}\"", "A", TitleKind::String),
            ("\"`\u{00a0}A\u{00a0}`\"", "A", TitleKind::Markdown),
            (
                "\"\u{00a0}&nbsp;A&nbsp;\u{00a0}\"",
                "&nbsp;A&nbsp;",
                TitleKind::String,
            ),
            (
                "\"`\u{00a0}&nbsp;A&nbsp;\u{00a0}`\"",
                "&nbsp;A&nbsp;",
                TitleKind::Markdown,
            ),
            ("\"\u{feff}A\u{feff}\"", "A", TitleKind::String),
            (
                "\"\u{0085}A\u{0085}\"",
                "\u{0085}A\u{0085}",
                TitleKind::String,
            ),
            ("\" `A` \"", "`A`", TitleKind::String),
            ("\"\u{00a0}`A`\u{00a0}\"", "`A`", TitleKind::String),
            (
                "\"\u{0085}`A`\u{0085}\"",
                "\u{0085}`A`\u{0085}",
                TitleKind::String,
            ),
            ("\"` A `\"", "A", TitleKind::Markdown),
        ];

        for (raw, expected, expected_kind) in cases {
            let (node_text, node_kind) = parse_label_text(raw);
            assert_eq!(node_text, expected, "node label {raw:?}");
            assert_eq!(node_kind, expected_kind, "node label {raw:?}");

            let (edge_text, edge_kind) = parse_edge_label_text(raw);
            assert_eq!(edge_text, expected, "edge label {raw:?}");
            assert_eq!(edge_kind, expected_kind, "edge label {raw:?}");
        }
    }
}

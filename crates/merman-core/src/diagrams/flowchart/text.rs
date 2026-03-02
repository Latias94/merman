use super::TitleKind;

pub(super) fn title_kind_str(kind: &TitleKind) -> &'static str {
    match kind {
        TitleKind::Text => "text",
        TitleKind::String => "string",
        TitleKind::Markdown => "markdown",
    }
}

pub(super) fn unescape_flowchart_string(s: &str) -> String {
    // Mermaid's flowchart string labels behave like a lightweight escape layer:
    // - `\\` => `\`
    // - `\"` => `"`
    // - `\'` => `'`
    // - `\n`, `\r`, `\t` => newline/carriage return/tab
    //
    // Keep unknown escapes as-is (preserve the backslash) to avoid surprising data loss.
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(ch) = it.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let Some(next) = it.next() else {
            out.push('\\');
            break;
        };
        match next {
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            '\'' => out.push('\''),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            other => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    out
}

pub(super) fn unquote(s: &str) -> String {
    let bytes = s.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
        return s[1..s.len() - 1].to_string();
    }
    if bytes.len() >= 2 && bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'' {
        return s[1..s.len() - 1].to_string();
    }
    s.to_string()
}

pub(super) fn parse_label_text(raw: &str) -> (String, TitleKind) {
    let trimmed = raw.trim();
    let quoted = (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''));
    let unquoted = if quoted {
        unescape_flowchart_string(&unquote(trimmed))
    } else {
        trimmed.to_string()
    };

    let (no_backticks, is_markdown) = strip_wrapping_backticks(unquoted.trim());
    if is_markdown {
        return (no_backticks, TitleKind::Markdown);
    }
    if quoted {
        return (unquoted, TitleKind::String);
    }
    (unquoted, TitleKind::Text)
}

pub(super) fn strip_wrapping_backticks(s: &str) -> (String, bool) {
    let trimmed = s.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('`') && trimmed.ends_with('`') {
        return (trimmed[1..trimmed.len() - 1].to_string(), true);
    }
    (trimmed.to_string(), false)
}

#[cfg(test)]
mod tests {
    use super::{parse_label_text, unescape_flowchart_string};
    use crate::diagrams::flowchart::TitleKind;

    #[test]
    fn unescape_flowchart_string_unescapes_common_sequences() {
        assert_eq!(
            unescape_flowchart_string(r#"C:\\Temp\\merman\\out.svg"#),
            r#"C:\Temp\merman\out.svg"#
        );
        assert_eq!(unescape_flowchart_string(r#"\"hi\""#), r#""hi""#);
        assert_eq!(unescape_flowchart_string(r#"\'hi\'"#), r#"'hi'"#);
        assert_eq!(unescape_flowchart_string(r#"\n"#), "\n");
        assert_eq!(unescape_flowchart_string(r#"\t"#), "\t");
        assert_eq!(unescape_flowchart_string("Model – label"), "Model – label");
    }

    #[test]
    fn parse_label_text_unescapes_string_labels() {
        let (text, kind) = parse_label_text(r#""Path: C:\\Temp\\merman\\out.svg (Windows-style)""#);
        assert_eq!(kind, TitleKind::String);
        assert_eq!(text, r#"Path: C:\Temp\merman\out.svg (Windows-style)"#);
    }
}

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
        unquote(trimmed)
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

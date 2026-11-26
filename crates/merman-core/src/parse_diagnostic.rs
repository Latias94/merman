#![cfg(any(
    feature = "diagram-class",
    feature = "diagram-er",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-sequence",
    feature = "diagram-state"
))]

pub(crate) fn unexpected_eof_message<'a>(expected: impl Iterator<Item = &'a str>) -> String {
    with_expectations("unexpected end of input".to_string(), expected)
}

#[cfg(any(
    feature = "diagram-class",
    feature = "diagram-er",
    feature = "diagram-sequence",
    feature = "diagram-state"
))]
pub(crate) fn unexpected_token_message<'a>(
    token: &impl std::fmt::Debug,
    expected: impl Iterator<Item = &'a str>,
) -> String {
    let debug = format!("{token:?}");
    let variant = debug
        .split_once('(')
        .map_or(debug.as_str(), |(name, _)| name);
    unexpected_token_name_message(variant, expected)
}

pub(crate) fn unexpected_token_name_message<'a>(
    variant: &str,
    expected: impl Iterator<Item = &'a str>,
) -> String {
    with_expectations(
        format!("unexpected {}", format_found_token(variant)),
        expected,
    )
}

fn with_expectations<'a>(mut message: String, expected: impl Iterator<Item = &'a str>) -> String {
    let expected = expected.map(humanize_expected_token).collect::<Vec<_>>();
    if !expected.is_empty() {
        message.push_str("; expected ");
        message.push_str(&expected.join(", "));
    }
    message
}

fn humanize_expected_token(token: &str) -> String {
    match token {
        "Id" => "node identifier".to_string(),
        "EdgeLabel" => "edge label".to_string(),
        "Direction" => "diagram direction".to_string(),
        "AlphaNumToken" => "identifier".to_string(),
        "Text" | "NoteText" | "Descr" | "RestOfLine" => "text".to_string(),
        "StringLit" => "string literal".to_string(),
        other => humanize_token_name(other),
    }
}

fn format_found_token(variant: &str) -> String {
    match variant {
        "Sep" | "Newline" => "statement separator".to_string(),
        "StyleSep" => "style separator".to_string(),
        "Amp" => "`&`".to_string(),
        "Comma" => "`,`".to_string(),
        "Plus" => "`+`".to_string(),
        "Minus" => "`-`".to_string(),
        "Arrow" => "edge operator".to_string(),
        "SignalType" => "message operator".to_string(),
        "Id" | "Actor" | "StyledId" => "identifier".to_string(),
        "Direction" | "DirectionStmt" => "diagram direction".to_string(),
        "EdgeLabel" | "Text" | "NoteText" | "Descr" | "RestOfLine" => "text".to_string(),
        "NodeLabel" | "StateDescr" | "CompositState" => "node label".to_string(),
        "StringLit" => "string literal".to_string(),
        "Num" => "number".to_string(),
        other => humanize_token_name(other),
    }
}

fn humanize_token_name(token: &str) -> String {
    let token = token.strip_prefix("Kw").unwrap_or(token);
    let mut out = String::new();
    let mut previous_is_lowercase = false;

    for ch in token.chars() {
        if ch == '_' || ch == '-' {
            if !out.ends_with(' ') {
                out.push(' ');
            }
            previous_is_lowercase = false;
            continue;
        }

        if ch.is_ascii_uppercase() && previous_is_lowercase && !out.ends_with(' ') {
            out.push(' ');
        }

        if ch.is_ascii_digit() && !out.ends_with(' ') && !out.is_empty() {
            out.push(' ');
        }

        out.push(ch.to_ascii_lowercase());
        previous_is_lowercase = ch.is_ascii_lowercase();
    }

    out.trim().to_string()
}

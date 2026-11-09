pub(super) use merman_model::text::is_ecmascript_whitespace;

use super::super::{
    LINETYPE_BIDIRECTIONAL_DOTTED, LINETYPE_BIDIRECTIONAL_SOLID, LINETYPE_DOTTED,
    LINETYPE_DOTTED_CROSS, LINETYPE_DOTTED_OPEN, LINETYPE_DOTTED_POINT, LINETYPE_SOLID,
    LINETYPE_SOLID_ARROW_BOTTOM_REVERSE, LINETYPE_SOLID_ARROW_BOTTOM_REVERSE_DOTTED,
    LINETYPE_SOLID_ARROW_TOP_REVERSE, LINETYPE_SOLID_ARROW_TOP_REVERSE_DOTTED,
    LINETYPE_SOLID_BOTTOM, LINETYPE_SOLID_BOTTOM_DOTTED, LINETYPE_SOLID_CROSS, LINETYPE_SOLID_OPEN,
    LINETYPE_SOLID_POINT, LINETYPE_SOLID_TOP, LINETYPE_SOLID_TOP_DOTTED,
    LINETYPE_STICK_ARROW_BOTTOM_REVERSE, LINETYPE_STICK_ARROW_BOTTOM_REVERSE_DOTTED,
    LINETYPE_STICK_ARROW_TOP_REVERSE, LINETYPE_STICK_ARROW_TOP_REVERSE_DOTTED,
    LINETYPE_STICK_BOTTOM, LINETYPE_STICK_BOTTOM_DOTTED, LINETYPE_STICK_TOP,
    LINETYPE_STICK_TOP_DOTTED,
};

pub(super) const HALF_ARROW_TYPES: [(&str, i32); 16] = [
    ("--|\\", LINETYPE_SOLID_TOP_DOTTED),
    ("--|/", LINETYPE_SOLID_BOTTOM_DOTTED),
    ("--\\\\", LINETYPE_STICK_TOP_DOTTED),
    ("--//", LINETYPE_STICK_BOTTOM_DOTTED),
    ("/|--", LINETYPE_SOLID_ARROW_TOP_REVERSE_DOTTED),
    ("\\|--", LINETYPE_SOLID_ARROW_BOTTOM_REVERSE_DOTTED),
    ("//--", LINETYPE_STICK_ARROW_TOP_REVERSE_DOTTED),
    ("\\\\--", LINETYPE_STICK_ARROW_BOTTOM_REVERSE_DOTTED),
    ("-|\\", LINETYPE_SOLID_TOP),
    ("-|/", LINETYPE_SOLID_BOTTOM),
    ("-\\\\", LINETYPE_STICK_TOP),
    ("-//", LINETYPE_STICK_BOTTOM),
    ("/|-", LINETYPE_SOLID_ARROW_TOP_REVERSE),
    ("\\|-", LINETYPE_SOLID_ARROW_BOTTOM_REVERSE),
    ("//-", LINETYPE_STICK_ARROW_TOP_REVERSE),
    ("\\\\-", LINETYPE_STICK_ARROW_BOTTOM_REVERSE),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ActorBoundary {
    Declaration,
    StatementEnd,
    Text,
}

pub(super) struct ActorScan<'input> {
    pub text: &'input str,
    pub scan_end: usize,
    pub token_end: usize,
    pub config_allowed: bool,
}

pub(super) fn scan_actor(
    input: &str,
    start: usize,
    boundary: ActorBoundary,
) -> Option<ActorScan<'_>> {
    let uses_id_rules = matches!(
        boundary,
        ActorBoundary::Declaration | ActorBoundary::StatementEnd
    );
    let end = if uses_id_rules {
        let mut end = start;
        for ch in input[start..].chars() {
            if matches!(ch, '\n' | ';' | ',' | ':' | '<' | '>' | '@')
                || (boundary == ActorBoundary::Declaration
                    && is_ecmascript_whitespace(ch)
                    && declaration_alias_starts_at(input, start, end))
            {
                break;
            }
            end += ch.len_utf8();
        }
        end
    } else {
        scan_initial_actor_end(input, start)?
    };

    let mut token_end = end;
    while token_end > start {
        let ch = input[start..token_end].chars().next_back()?;
        if !is_ecmascript_whitespace(ch) {
            break;
        }
        token_end -= ch.len_utf8();
    }
    if token_end == start {
        return None;
    }

    let text = trim_start_ecmascript(&input[start..token_end]);
    Some(ActorScan {
        text,
        scan_end: end,
        token_end,
        config_allowed: boundary == ActorBoundary::Declaration
            && declaration_id_allows_config(text),
    })
}

fn scan_initial_actor_end(input: &str, start: usize) -> Option<usize> {
    // The pinned ACTOR rule has a restrictive nonempty head followed by wider chunks. Slashes
    // and parentheses are legal inside a continuation, where only a new chunk checks arrows.
    let head = &input[start..];
    let head_len = head
        .find(|ch| {
            matches!(
                ch,
                '/' | '\\' | '+' | '(' | ')' | '<' | '-' | '>' | ':' | '\n' | ',' | ';'
            )
        })
        .unwrap_or(head.len());
    if head_len == 0 {
        return None;
    }

    let mut end = start + head_len;
    loop {
        let rest = &input[end..];
        if signal_type_at(input, end).is_some()
            || ["--", "-/", "-\\", "()"]
                .iter()
                .any(|prefix| rest.starts_with(prefix))
        {
            break;
        }
        let hyphen_end = end + rest.bytes().take_while(|byte| *byte == b'-').count();
        let tail = &input[hyphen_end..];
        let tail_len = tail
            .find(['+', '<', '-', '>', ':', '\n', ',', ';'])
            .unwrap_or(tail.len());
        if tail_len == 0 {
            break;
        }
        end = hyphen_end + tail_len;
    }
    Some(end)
}

pub(super) fn signal_type_at(input: &str, position: usize) -> Option<(usize, i32)> {
    let rest = input.get(position..)?;
    half_arrow_type(rest)
        .or_else(|| {
            rest.starts_with("<<-->>")
                .then_some((6, LINETYPE_BIDIRECTIONAL_DOTTED))
        })
        .or_else(|| {
            rest.starts_with("<<->>")
                .then_some((5, LINETYPE_BIDIRECTIONAL_SOLID))
        })
        .or_else(|| rest.starts_with("-->>").then_some((4, LINETYPE_DOTTED)))
        .or_else(|| rest.starts_with("->>").then_some((3, LINETYPE_SOLID)))
        .or_else(|| rest.starts_with("-->").then_some((3, LINETYPE_DOTTED_OPEN)))
        .or_else(|| rest.starts_with("->").then_some((2, LINETYPE_SOLID_OPEN)))
        .or_else(|| {
            rest.starts_with("--x")
                .then_some((3, LINETYPE_DOTTED_CROSS))
        })
        .or_else(|| rest.starts_with("-x").then_some((2, LINETYPE_SOLID_CROSS)))
        .or_else(|| {
            rest.starts_with("--)")
                .then_some((3, LINETYPE_DOTTED_POINT))
        })
        .or_else(|| rest.starts_with("-)").then_some((2, LINETYPE_SOLID_POINT)))
}

pub(super) fn config_followed_by_alias(input: &str, position: usize) -> bool {
    let mut alias_start = position;
    let mut saw_whitespace = false;
    while let Some(ch) = input[alias_start..].chars().next() {
        if ch == '\n' || !is_ecmascript_whitespace(ch) {
            break;
        }
        saw_whitespace = true;
        alias_start += ch.len_utf8();
    }
    saw_whitespace
        && starts_with_ci_at(input, alias_start, "as")
        && char_at_is_ecmascript_whitespace(input, alias_start + "as".len())
}

pub(super) fn trim_start_ecmascript(value: &str) -> &str {
    value.trim_start_matches(is_ecmascript_whitespace)
}

pub(super) fn trim_ecmascript(value: &str) -> &str {
    value.trim_matches(is_ecmascript_whitespace)
}

fn half_arrow_type(rest: &str) -> Option<(usize, i32)> {
    HALF_ARROW_TYPES
        .iter()
        .find_map(|(arrow, ty)| rest.starts_with(arrow).then_some((arrow.len(), *ty)))
}

fn declaration_id_allows_config(actor_id: &str) -> bool {
    !actor_id.is_empty()
        && actor_id.chars().all(|ch| {
            !is_ecmascript_whitespace(ch) && !matches!(ch, '<' | '>' | ':' | ',' | ';' | '@')
        })
}

pub(super) fn central_suffix_precedes_signal(input: &str, position: usize) -> bool {
    if !input[position..].starts_with("()") {
        return false;
    }
    let mut signal_start = position + 2;
    while let Some(ch) = input[signal_start..].chars().next() {
        if ch == '\n' || !is_ecmascript_whitespace(ch) {
            break;
        }
        signal_start += ch.len_utf8();
    }
    signal_type_at(input, signal_start).is_some()
}

fn declaration_alias_starts_at(input: &str, actor_start: usize, whitespace_start: usize) -> bool {
    if input[actor_start..whitespace_start]
        .chars()
        .any(is_ecmascript_whitespace)
    {
        return false;
    }

    let mut alias_start = whitespace_start;
    while let Some(ch) = input[alias_start..].chars().next() {
        if !is_ecmascript_whitespace(ch) || ch == '\n' {
            break;
        }
        alias_start += ch.len_utf8();
    }
    starts_with_ci_at(input, alias_start, "as")
        && char_at_is_ecmascript_whitespace(input, alias_start + 2)
}

fn char_at_is_ecmascript_whitespace(input: &str, position: usize) -> bool {
    input
        .get(position..)
        .and_then(|rest| rest.chars().next())
        .is_some_and(is_ecmascript_whitespace)
}

fn starts_with_ci_at(input: &str, position: usize, expected: &str) -> bool {
    let Some(actual) = input
        .as_bytes()
        .get(position..position.saturating_add(expected.len()))
    else {
        return false;
    };
    actual.eq_ignore_ascii_case(expected.as_bytes())
}

pub(crate) fn strip_line_ending(segment: &str) -> &str {
    let segment = segment.strip_suffix('\n').unwrap_or(segment);
    segment.strip_suffix('\r').unwrap_or(segment)
}

pub(crate) fn starts_with_case_insensitive(haystack: &str, needle: &str) -> bool {
    if haystack.len() < needle.len() {
        return false;
    }
    haystack
        .as_bytes()
        .iter()
        .take(needle.len())
        .copied()
        .map(|b| b.to_ascii_lowercase())
        .eq(needle
            .as_bytes()
            .iter()
            .copied()
            .map(|b| b.to_ascii_lowercase()))
}

pub(crate) fn split_indent_by<F>(line: &str, mut is_indent_char: F) -> (usize, &str)
where
    F: FnMut(char) -> bool,
{
    let mut indent_chars = 0usize;
    let mut byte_idx = line.len();
    for (idx, ch) in line.char_indices() {
        if is_indent_char(ch) {
            indent_chars += 1;
            continue;
        }
        byte_idx = idx;
        break;
    }
    if indent_chars == 0 {
        byte_idx = 0;
    } else if byte_idx == line.len() {
        byte_idx = line.len();
    }
    (indent_chars, &line[byte_idx..])
}

pub(crate) fn split_indent(line: &str) -> (usize, &str) {
    split_indent_by(line, char::is_whitespace)
}

pub(crate) fn leading_whitespace_len(s: &str) -> usize {
    s.chars()
        .take_while(|ch| ch.is_whitespace())
        .map(char::len_utf8)
        .sum()
}

pub(crate) fn split_statement_suffix_hash_or_semi(s: &str) -> &str {
    let mut end = s.len();
    for (i, c) in s.char_indices() {
        if c == '#' || c == ';' {
            end = i;
            break;
        }
    }
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::{
        leading_whitespace_len, split_indent, split_indent_by, split_statement_suffix_hash_or_semi,
        starts_with_case_insensitive, strip_line_ending,
    };

    #[test]
    fn strip_line_ending_removes_lf_and_crlf() {
        assert_eq!(strip_line_ending("line\n"), "line");
        assert_eq!(strip_line_ending("line\r\n"), "line");
        assert_eq!(strip_line_ending("line"), "line");
    }

    #[test]
    fn starts_with_case_insensitive_handles_ascii_prefixes() {
        assert!(starts_with_case_insensitive("MindMap", "mindmap"));
        assert!(!starts_with_case_insensitive("diagram", "mindmap"));
    }

    #[test]
    fn split_indent_counts_leading_whitespace() {
        let (indent, rest) = split_indent(" \troot");
        assert_eq!(indent, 2);
        assert_eq!(rest, "root");
    }

    #[test]
    fn split_indent_by_honors_custom_predicate() {
        let (indent, rest) = split_indent_by(" \troot", |ch| ch == ' ' || ch == '\t');
        assert_eq!(indent, 2);
        assert_eq!(rest, "root");
    }

    #[test]
    fn leading_whitespace_len_tracks_utf8_width() {
        assert_eq!(leading_whitespace_len(" \troot"), 2);
    }

    #[test]
    fn split_statement_suffix_hash_or_semi_stops_before_comment_markers() {
        assert_eq!(split_statement_suffix_hash_or_semi("task # note"), "task ");
        assert_eq!(split_statement_suffix_hash_or_semi("task; note"), "task");
        assert_eq!(split_statement_suffix_hash_or_semi("task"), "task");
    }
}

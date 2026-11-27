#[cfg(feature = "diagram-c4")]
pub(crate) use merman_core_preprocess::scan::LineCursor;
#[cfg(test)]
pub(crate) use merman_core_preprocess::scan::physical_line_at;
#[cfg(test)]
pub(crate) use merman_core_preprocess::scan::split_statement_suffix_hash_or_semi;
#[cfg(any(test, feature = "diagram-kanban"))]
pub(crate) use merman_core_preprocess::scan::starts_with_case_insensitive;
#[cfg(feature = "diagram-kanban")]
pub(crate) use merman_core_preprocess::scan::strip_line_ending;
pub(crate) use merman_model::text::is_ecmascript_whitespace;

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

#[cfg(test)]
mod tests {
    use super::{
        leading_whitespace_len, physical_line_at, split_indent, split_indent_by,
        split_statement_suffix_hash_or_semi, starts_with_case_insensitive,
    };

    #[test]
    fn physical_line_at_tracks_next_offset_and_strips_endings() {
        let source = "first\r\nsecond\r";
        let second_start = "first\r\n".len();

        assert_eq!(physical_line_at(source, 0), ("first", second_start));
        assert_eq!(
            physical_line_at(source, second_start),
            ("second", source.len())
        );
        assert_eq!(physical_line_at(source, source.len()), ("", source.len()));
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

#[cfg(test)]
pub(crate) use merman_core_preprocess::scan::physical_line_at;
#[cfg(test)]
pub(crate) use merman_core_preprocess::scan::split_statement_suffix_hash_or_semi;
#[cfg(test)]
pub(crate) use merman_core_preprocess::scan::starts_with_case_insensitive;
pub(crate) use merman_model::text::is_ecmascript_whitespace;

#[cfg(test)]
mod tests {
    use super::{
        physical_line_at, split_statement_suffix_hash_or_semi, starts_with_case_insensitive,
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
    fn split_statement_suffix_hash_or_semi_stops_before_comment_markers() {
        assert_eq!(split_statement_suffix_hash_or_semi("task # note"), "task ");
        assert_eq!(split_statement_suffix_hash_or_semi("task; note"), "task");
        assert_eq!(split_statement_suffix_hash_or_semi("task"), "task");
    }
}

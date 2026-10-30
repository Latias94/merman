/// Returns whether a character belongs to JavaScript's ECMAScript whitespace set.
///
/// Mermaid's Jison lexers use `\s`, whose behavior differs from Rust's `char::is_whitespace`
/// for U+FEFF and a few control characters. Keep this predicate source-compatible with the
/// JavaScript lexer while advancing UTF-8 cursors by complete characters.
pub fn is_ecmascript_whitespace(ch: char) -> bool {
    matches!(
        ch,
        '\u{0009}'
            | '\u{000A}'
            | '\u{000B}'
            | '\u{000C}'
            | '\u{000D}'
            | '\u{0020}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::is_ecmascript_whitespace;

    #[test]
    fn javascript_whitespace_preserves_bom_and_control_character_differences() {
        for ch in [
            ' ', '\t', '\r', '\n', '\u{00a0}', '\u{2028}', '\u{2029}', '\u{feff}',
        ] {
            assert!(is_ecmascript_whitespace(ch), "{ch:?}");
        }
        for ch in [
            'a', '\u{001c}', '\u{001f}', '\u{0085}', '\u{180e}', '\u{200b}',
        ] {
            assert!(!is_ecmascript_whitespace(ch), "{ch:?}");
        }
        assert!(!'\u{feff}'.is_whitespace());
        assert!('\u{0085}'.is_whitespace());
    }
}

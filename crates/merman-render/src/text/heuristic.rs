//! Heuristic text width estimation helpers.

pub(crate) fn estimate_char_width_em(ch: char) -> f64 {
    if matches!(ch, ' ' | '\u{00A0}') {
        return 0.33;
    }
    if ch == '\t' {
        return 0.66;
    }
    if ch == '_' || ch == '-' {
        return 0.33;
    }
    if matches!(ch, '.' | ',' | ':' | ';') {
        return 0.28;
    }
    if matches!(ch, '(' | ')' | '[' | ']' | '{' | '}' | '/') {
        return 0.33;
    }
    if matches!(ch, '+' | '*' | '=' | '\\' | '^' | '|' | '~') {
        return 0.45;
    }
    if ch.is_ascii_digit() {
        return 0.56;
    }
    if ch.is_ascii_uppercase() {
        return match ch {
            'I' => 0.30,
            'W' => 0.85,
            _ => 0.60,
        };
    }
    if ch.is_ascii_lowercase() {
        return match ch {
            'i' | 'l' => 0.28,
            'm' | 'w' => 0.78,
            'k' | 'y' => 0.55,
            _ => 0.43,
        };
    }
    if !ch.is_ascii() {
        return match unicode_width::UnicodeWidthChar::width(ch) {
            Some(0) => 0.0,
            Some(2..) => 1.0,
            _ => 0.60,
        };
    }

    // Remaining ASCII punctuation and controls use a conservative single-column estimate.
    0.60
}

#[cfg(test)]
mod tests {
    use super::estimate_char_width_em;

    fn estimate_line_width_px(line: &str, font_size: f64) -> f64 {
        line.chars().map(estimate_char_width_em).sum::<f64>() * font_size
    }

    #[test]
    fn non_breaking_space_uses_the_regular_space_advance() {
        assert_eq!(
            estimate_line_width_px("A\u{00A0}B", 16.0),
            estimate_line_width_px("A B", 16.0),
        );
    }

    #[test]
    fn unicode_display_width_distinguishes_wide_and_zero_width_scalars() {
        assert_eq!(estimate_line_width_px("中文测试", 16.0), 64.0);
        assert_eq!(
            estimate_line_width_px("e\u{0301}", 16.0),
            estimate_line_width_px("e", 16.0),
        );
        assert_eq!(estimate_line_width_px("🙂", 16.0), 16.0);
        assert_eq!(estimate_line_width_px("é", 16.0), 9.6);
    }
}

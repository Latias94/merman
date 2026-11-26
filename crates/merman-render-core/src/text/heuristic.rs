//! Heuristic text width estimation helpers.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const fn compatibility_ascii_width(ch: u8) -> f64 {
    match ch {
        b' ' | b'_' | b'-' | b'(' | b')' | b'[' | b']' | b'{' | b'}' | b'/' => 0.33,
        b'\t' => 0.66,
        b'.' | b',' | b':' | b';' | b'i' | b'l' => 0.28,
        b'+' | b'*' | b'=' | b'\\' | b'^' | b'|' | b'~' => 0.45,
        b'0'..=b'9' => 0.56,
        b'I' => 0.30,
        b'W' => 0.85,
        b'm' | b'w' => 0.78,
        b'k' | b'y' => 0.55,
        b'a'..=b'z' => 0.43,
        // Remaining ASCII punctuation and controls use a conservative single-column estimate.
        _ => 0.60,
    }
}

const fn calibrated_ascii_width(ch: u8) -> f64 {
    // Sans-serif advances in em, grouped by the Trebuchet MS and Arial fallback ranges.
    match ch {
        b'i' | b'l' => 0.29,
        b'f' | b'j' | b'r' | b't' => 0.39,
        b's' => 0.50,
        b'm' | b'M' => 0.84,
        b'w' => 0.76,
        b'a'..=b'z' => 0.55,
        b'_' | b'J' | b'L' => 0.56,
        b'I' => 0.29,
        b'W' => 0.94,
        b'G' | b'O' | b'Q' => 0.77,
        b'A'..=b'Z' => 0.69,
        _ => compatibility_ascii_width(ch),
    }
}

fn estimate_char_width_em(ch: char) -> f64 {
    match ch {
        // Keep the source control as a shaping boundary without an unselected glyph advance.
        '\u{00ad}' => 0.0,
        '\u{00a0}' => 0.33,
        _ if ch.is_ascii() => COMPATIBILITY_ASCII[ch as usize],
        _ => unicode_char_width(ch),
    }
}

fn calibrated_char_width_em(ch: char) -> f64 {
    if ch.is_ascii() {
        CALIBRATED_ASCII[ch as usize]
    } else {
        estimate_char_width_em(ch)
    }
}

fn unicode_char_width(ch: char) -> f64 {
    match UnicodeWidthChar::width(ch) {
        Some(0) => 0.0,
        Some(2..) => 1.0,
        _ => 0.60,
    }
}

fn estimate_grapheme_width_em(grapheme: &str, advance: impl Fn(char) -> f64) -> f64 {
    let mut chars = grapheme.chars();
    let Some(first) = chars.next() else {
        return 0.0;
    };
    if chars.next().is_none() {
        return advance(first);
    }

    // Preserve the proportional ASCII model for the only multi-scalar ASCII grapheme (CRLF).
    // Non-ASCII clusters use the sequence-aware width API so grapheme-local ZWJ emoji,
    // modifiers, flags, keycaps, and presentation selectors are not charged once per scalar.
    if grapheme.is_ascii() {
        return grapheme.chars().map(advance).sum();
    }

    let sequence_columns = UnicodeWidthStr::width(grapheme);
    match sequence_columns {
        0 => 0.0,
        columns => {
            let mut visible = grapheme
                .chars()
                .filter(|ch| UnicodeWidthChar::width(*ch).is_some_and(|width| width > 0));
            let Some(base) = visible.next() else {
                return 0.0;
            };
            if visible.next().is_none() && UnicodeWidthChar::width(base) == Some(columns) {
                // Keep combining-mark sequences aligned with their single visible base. This
                // applies the selected proportional estimate to decomposed text.
                advance(base)
            } else if columns == 1 {
                0.60
            } else {
                columns as f64 * 0.5
            }
        }
    }
}

pub fn is_monospace_font_family(font_family: Option<&str>) -> bool {
    let Some(raw) = font_family else {
        return false;
    };
    let trimmed = raw.trim().trim_end_matches(';');
    if trimmed.is_empty() {
        return false;
    }
    for entry in trimmed.split(',') {
        let candidate = entry.trim();
        if candidate.is_empty() {
            continue;
        }
        // Quoted entries are family names, not CSS generic family keywords.
        if (candidate.starts_with('"') && candidate.ends_with('"'))
            || (candidate.starts_with('\'') && candidate.ends_with('\''))
        {
            continue;
        }
        if candidate.eq_ignore_ascii_case("monospace")
            || candidate.eq_ignore_ascii_case("ui-monospace")
        {
            return true;
        }
        if candidate.eq_ignore_ascii_case("sans-serif")
            || candidate.eq_ignore_ascii_case("serif")
            || candidate.eq_ignore_ascii_case("cursive")
            || candidate.eq_ignore_ascii_case("fantasy")
            || candidate.eq_ignore_ascii_case("system-ui")
        {
            return false;
        }
    }
    false
}

fn estimate_monospace_char_width_em(ch: char) -> f64 {
    match ch {
        '\u{00ad}' => 0.0,
        _ if ch.is_ascii() => MONOSPACE_ASCII[ch as usize],
        _ => unicode_char_width(ch),
    }
}

fn estimate_monospace_grapheme_width_em(grapheme: &str) -> f64 {
    let mut chars = grapheme.chars();
    let Some(first) = chars.next() else {
        return 0.0;
    };
    if chars.next().is_none() {
        return estimate_monospace_char_width_em(first);
    }
    if grapheme.is_ascii() {
        return grapheme.chars().map(estimate_monospace_char_width_em).sum();
    }
    match UnicodeWidthStr::width(grapheme) {
        0 => 0.0,
        1 => 0.60,
        columns => columns as f64 * 0.5,
    }
}

/// Built-in width estimates for a decoded line or line fragment.
#[derive(Debug, Clone, Copy)]
pub enum HeuristicTextWidth {
    Compatibility,
    Calibrated,
    Monospace,
}

const COMPATIBILITY_ASCII: [f64; 128] = HeuristicTextWidth::Compatibility.ascii_advances();
const CALIBRATED_ASCII: [f64; 128] = HeuristicTextWidth::Calibrated.ascii_advances();
const MONOSPACE_ASCII: [f64; 128] = HeuristicTextWidth::Monospace.ascii_advances();

impl HeuristicTextWidth {
    const fn ascii_advances(self) -> [f64; 128] {
        let mut widths = [0.0; 128];
        let mut byte = 0;
        while byte < widths.len() {
            widths[byte] = match self {
                Self::Compatibility => compatibility_ascii_width(byte as u8),
                Self::Calibrated => calibrated_ascii_width(byte as u8),
                Self::Monospace if byte == b'\t' as usize => 1.20,
                Self::Monospace => 0.60,
            };
            byte += 1;
        }
        widths
    }

    pub fn append(self, width_em: &mut f64, text: &str) {
        if text.is_ascii() {
            let advances = match self {
                Self::Compatibility => &COMPATIBILITY_ASCII,
                Self::Calibrated => &CALIBRATED_ASCII,
                Self::Monospace => &MONOSPACE_ASCII,
            };
            let mut bytes = text.bytes().peekable();
            while let Some(byte) = bytes.next() {
                let mut advance = advances[usize::from(byte)];
                // Keep the same addition order for the only multi-byte ASCII grapheme.
                if byte == b'\r' && bytes.next_if_eq(&b'\n').is_some() {
                    advance += advances[usize::from(b'\n')];
                }
                *width_em += advance;
            }
            return;
        }
        match self {
            Self::Compatibility => {
                for grapheme in text.graphemes(true) {
                    *width_em += estimate_grapheme_width_em(grapheme, estimate_char_width_em);
                }
            }
            Self::Calibrated => {
                for grapheme in text.graphemes(true) {
                    *width_em += estimate_grapheme_width_em(grapheme, calibrated_char_width_em);
                }
            }
            Self::Monospace => {
                for grapheme in text.graphemes(true) {
                    *width_em += estimate_monospace_grapheme_width_em(grapheme);
                }
            }
        }
    }

    pub fn width(self, text: &str) -> f64 {
        let mut width_em = 0.0;
        self.append(&mut width_em, text);
        width_em
    }
}

pub fn append_text_width_em(width_em: &mut f64, text: &str) {
    HeuristicTextWidth::Compatibility.append(width_em, text);
}

pub fn estimate_text_width_em(text: &str) -> f64 {
    HeuristicTextWidth::Compatibility.width(text)
}

pub fn append_monospace_text_width_em(width_em: &mut f64, text: &str) {
    HeuristicTextWidth::Monospace.append(width_em, text);
}

pub fn estimate_monospace_text_width_em(text: &str) -> f64 {
    HeuristicTextWidth::Monospace.width(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_width_appends_in_grapheme_order() {
        let ascii = String::from_utf8((0..128).collect()).unwrap();
        let text = format!("{ascii}\r\n{ascii}\r\n\r\n\n\r");
        for model in [
            HeuristicTextWidth::Compatibility,
            HeuristicTextWidth::Calibrated,
            HeuristicTextWidth::Monospace,
        ] {
            for start in [0.0, 0.137, 63.0, 512.0, 1.0e16] {
                let mut actual = start;
                model.append(&mut actual, &text);
                let expected = text.graphemes(true).fold(start, |sum, grapheme| {
                    sum + match model {
                        HeuristicTextWidth::Compatibility => {
                            estimate_grapheme_width_em(grapheme, estimate_char_width_em)
                        }
                        HeuristicTextWidth::Calibrated => {
                            estimate_grapheme_width_em(grapheme, calibrated_char_width_em)
                        }
                        HeuristicTextWidth::Monospace => {
                            estimate_monospace_grapheme_width_em(grapheme)
                        }
                    }
                });
                assert_eq!(actual.to_bits(), expected.to_bits(), "{model:?}");
            }
        }
    }

    #[test]
    fn calibrated_sans_serif_covers_typical_fallback_advances() {
        let width = |text| HeuristicTextWidth::Calibrated.width(text);
        assert!(width("customer_account_id") >= 9.58);
        assert!(width("DELIVERY_ADDRESS") >= 10.32);
        assert!(width("timeout_exceeded") >= 8.35);
        assert!(width("+applyDiscountCode(String promo) void") >= 17.76);
        assert_eq!(width("e\u{0301}"), width("e"));
        assert_eq!(width("👩‍🔬"), width("🙂"));
    }

    fn estimate_line_width_px(line: &str, font_size: f64) -> f64 {
        estimate_text_width_em(line) * font_size
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

    #[test]
    fn unicode_display_width_collapses_emoji_sequences_to_one_cluster() {
        for sequence in ["👩‍🔬", "👨‍👩‍👧‍👦", "👍🏽", "🇨🇳", "1️⃣"]
        {
            assert_eq!(
                estimate_line_width_px(sequence, 16.0),
                16.0,
                "sequence={sequence:?}"
            );
        }
    }

    #[test]
    fn unicode_presentation_selectors_adjust_the_sequence_display_width() {
        // U+26A0 defaults to text presentation, while VS16 selects its standardized emoji form.
        assert_eq!(estimate_line_width_px("⚠", 16.0), 9.6);
        assert_eq!(estimate_line_width_px("⚠️", 16.0), 16.0);

        // U+2648 defaults to emoji presentation, while VS15 selects its standardized text form.
        assert_eq!(estimate_line_width_px("♈", 16.0), 16.0);
        assert_eq!(estimate_line_width_px("♈\u{fe0e}", 16.0), 9.6);
    }

    #[test]
    fn monospace_font_stack_detection_and_uniform_cell_width() {
        use super::{estimate_monospace_text_width_em, is_monospace_font_family};

        assert!(is_monospace_font_family(Some("monospace")));
        assert!(is_monospace_font_family(Some(
            r#""JetBrains Mono", "Cascadia Mono", "Consolas", "Noto Sans Mono CJK SC", monospace"#
        )));
        assert!(is_monospace_font_family(Some(
            r#""courier new", courier, monospace;"#
        )));
        assert!(is_monospace_font_family(Some("ui-monospace")));
        assert!(!is_monospace_font_family(None));
        assert!(!is_monospace_font_family(Some(
            r#""Inter", "Helvetica Neue", sans-serif"#
        )));
        assert!(!is_monospace_font_family(Some(
            r#""monospace", sans-serif"#
        )));

        // 14 ASCII chars at 14px -> 14 * 0.60 * 14 = 117.6px (matches native JetBrains Mono / Menlo).
        assert!((estimate_monospace_text_width_em("+String field1") * 14.0 - 117.6).abs() < 1e-9);
        assert!((estimate_monospace_text_width_em("+method1()") * 14.0 - 84.0).abs() < 1e-9);
        assert!((estimate_monospace_text_width_em("中文A") * 14.0 - (2.6 * 14.0)).abs() < 1e-9);
    }
}

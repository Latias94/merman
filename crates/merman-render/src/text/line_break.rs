//! Unicode line-breaking primitives for browser-like HTML labels.

/// Returns the atomic line-box segments for Mermaid's wrapped HTML labels.
///
/// `unicode_linebreak` supplies the UAX #14 soft and mandatory opportunities. Chromium's CSS
/// line breaker suppresses the library's soft opportunity immediately after `/`; this keeps path
/// and URI components together while retaining opportunities after characters such as `-` and
/// `?`. CSS `white-space: break-spaces` additionally permits a break after every preserved U+0020,
/// so runs of spaces are divided without discarding the spaces themselves.
pub(crate) fn html_break_spaces_segments(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return vec![text];
    }

    let mut segments = Vec::new();
    let mut segment_start = 0usize;
    for (segment_end, opportunity) in unicode_linebreak::linebreaks(text) {
        if opportunity == unicode_linebreak::BreakOpportunity::Allowed
            && text[..segment_end].ends_with('/')
        {
            continue;
        }
        push_break_spaces_segments(text, segment_start, segment_end, &mut segments);
        segment_start = segment_end;
    }

    // UAX #14 emits a mandatory opportunity at the end of non-empty text. Keep the remainder as
    // a defensive fallback in case the dependency changes that iterator contract.
    if segment_start < text.len() {
        push_break_spaces_segments(text, segment_start, text.len(), &mut segments);
    }

    if segments.is_empty() {
        vec![text]
    } else {
        segments
    }
}

/// Reports whether browser-like `white-space: break-spaces` layout has an internal soft break.
///
/// Deriving this from the same atomic segments used by wrapping and min-content sizing keeps
/// callers from guessing breakability from ASCII whitespace alone. A single segment means that
/// shrinking the containing block cannot reflow the text without an additional CSS breaking rule.
pub(crate) fn html_has_soft_break_opportunity(text: &str) -> bool {
    html_break_spaces_segments(text).len() > 1
}

fn push_break_spaces_segments<'a>(
    text: &'a str,
    start: usize,
    end: usize,
    segments: &mut Vec<&'a str>,
) {
    if start >= end {
        return;
    }

    let mut part_start = start;
    for (offset, ch) in text[start..end].char_indices() {
        if ch != ' ' {
            continue;
        }
        let part_end = start + offset + ch.len_utf8();
        segments.push(&text[part_start..part_end]);
        part_start = part_end;
    }
    if part_start < end {
        segments.push(&text[part_start..end]);
    }
}

#[cfg(test)]
mod tests {
    use super::{html_break_spaces_segments, html_has_soft_break_opportunity};

    #[test]
    fn follows_browser_line_breaking_for_prose_cjk_and_url_boundaries() {
        assert_eq!(html_break_spaces_segments("alpha-beta"), ["alpha-", "beta"]);
        assert_eq!(
            html_break_spaces_segments("负责人审批"),
            ["负", "责", "人", "审", "批"]
        );
        assert_eq!(
            html_break_spaces_segments("https://x.test/(alpha)/z"),
            ["https://x.test/(alpha)/z"]
        );
        assert_eq!(
            html_break_spaces_segments(
                "https://example.com/api/v1/some(very-long)/resource-name?query=foo_bar&baz=qux"
            ),
            [
                "https://example.com/api/v1/some(very-",
                "long)/resource-",
                "name?",
                "query=foo_bar&baz=qux"
            ]
        );
    }

    #[test]
    fn reports_soft_breaks_from_the_same_browser_segments() {
        assert!(!html_has_soft_break_opportunity(""));
        assert!(!html_has_soft_break_opportunity("unbroken"));
        assert!(!html_has_soft_break_opportunity("https://x.test/(alpha)/z"));
        assert!(html_has_soft_break_opportunity("alpha beta"));
        assert!(html_has_soft_break_opportunity("alpha-beta"));
        assert!(html_has_soft_break_opportunity("负责人审批"));
        assert!(html_has_soft_break_opportunity(
            "https://example.com/api/v1/some(very-long)/resource-name?query=foo_bar&baz=qux"
        ));
    }

    #[test]
    fn break_spaces_preserves_each_space_before_its_break() {
        assert_eq!(html_break_spaces_segments("a  b"), ["a ", " ", "b"]);
        assert_eq!(html_break_spaces_segments("  "), [" ", " "]);
    }
}

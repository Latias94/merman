//! Unicode line-breaking primitives for browser-like HTML labels.

/// Replaces tags matched by Mermaid's `common.lineBreakRegex` (`/<\/?br\s*\/?>/gi`).
/// ELK applies this before Markdown parsing, including for State and Class edge labels.
pub(crate) fn mermaid_html_breaks_to_newlines(text: &str) -> std::borrow::Cow<'_, str> {
    let mut output: Option<String> = None;
    let mut copied_until = 0;
    for (start, _) in text.match_indices('<') {
        let rest = &text[start + 1..];
        let rest = rest.strip_prefix('/').unwrap_or(rest);
        if !rest
            .get(..2)
            .is_some_and(|name| name.eq_ignore_ascii_case("br"))
        {
            continue;
        }
        let rest = rest[2..].trim_start_matches(super::is_ecmascript_whitespace);
        let rest = rest.strip_prefix('/').unwrap_or(rest);
        let Some(after_tag) = rest.strip_prefix('>') else {
            continue;
        };
        let output = output.get_or_insert_with(|| String::with_capacity(text.len()));
        output.push_str(&text[copied_until..start]);
        output.push('\n');
        copied_until = text.len() - after_tag.len();
    }
    match output {
        Some(mut output) => {
            output.push_str(&text[copied_until..]);
            std::borrow::Cow::Owned(output)
        }
        None => std::borrow::Cow::Borrowed(text),
    }
}

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
    use super::{
        html_break_spaces_segments, html_has_soft_break_opportunity,
        mermaid_html_breaks_to_newlines,
    };

    #[test]
    fn mermaid_html_breaks_match_source_tags_and_javascript_whitespace() {
        assert_eq!(
            mermaid_html_breaks_to_newlines("a<BR />b</br>c<br\u{feff}/>d"),
            "a\nb\nc\nd"
        );
        let unchanged = "a<br class=x>b<br\u{0085}>c<br / >d<break>e\\n";
        assert!(
            matches!(mermaid_html_breaks_to_newlines(unchanged), std::borrow::Cow::Borrowed(value) if value == unchanged)
        );
        assert_eq!(
            mermaid_html_breaks_to_newlines("<broken<br/>中文</BR >"),
            "<broken\n中文\n"
        );
    }

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

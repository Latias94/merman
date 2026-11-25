//! Borrowed source tokens for Timeline syntax.

use merman_model::families::timeline::TimelineDirection;
use merman_model::text::is_ecmascript_whitespace;
use merman_model::{OperationControl, OperationControlResult, SourceSpan};

#[derive(Debug, Clone, Copy)]
pub struct TimelineSource<'source> {
    pub text: &'source str,
    pub start: usize,
}

impl<'source> TimelineSource<'source> {
    pub fn new(text: &'source str, start: usize) -> Self {
        Self { text, start }
    }

    pub fn end(self) -> usize {
        self.start + self.text.len()
    }

    pub fn span(self) -> SourceSpan {
        SourceSpan::new(self.start, self.end())
    }

    fn subslice(self, start: usize, end: usize) -> Self {
        debug_assert!(start <= end);
        debug_assert!(end <= self.text.len());
        Self::new(&self.text[start..end], self.start + start)
    }

    pub fn trim_start(self) -> Self {
        let leading =
            self.text.len() - self.text.trim_start_matches(is_ecmascript_whitespace).len();
        self.subslice(leading, self.text.len())
    }

    fn trim_end(self) -> Self {
        let text = self.text.trim_end_matches(is_ecmascript_whitespace);
        self.subslice(0, text.len())
    }

    pub fn trim(self) -> Self {
        self.trim_start().trim_end()
    }
}

pub struct ParsedAccDescrBlock<'source> {
    pub keyword: SourceSpan,
    pub closing: Option<SourceSpan>,
    pub value: TimelineSource<'source>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineEventIssue {
    InvalidToken(SourceSpan),
    MissingWhitespace(usize),
    ExpectedText(usize),
}

pub struct ParsedTimelineEvents<'source> {
    pub events: Vec<TimelineSource<'source>>,
    pub issue: Option<TimelineEventIssue>,
}

#[derive(Debug, Clone, Copy)]
pub enum TimelinePeriodSuffix<'source> {
    None,
    HashComment,
    Events(TimelineSource<'source>),
}

#[derive(Debug, Clone, Copy)]
pub struct ParsedTimelinePeriod<'source> {
    pub period: TimelineSource<'source>,
    pub task: TimelineSource<'source>,
    pub suffix: TimelinePeriodSuffix<'source>,
}

fn parse_keyword_prefix<'source>(
    source: TimelineSource<'source>,
    keyword: &str,
) -> Option<(TimelineSource<'source>, TimelineSource<'source>)> {
    if !source
        .text
        .get(..keyword.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(keyword))
    {
        return None;
    }
    let after_keyword = source.subslice(keyword.len(), source.text.len());
    if after_keyword
        .text
        .starts_with(|ch: char| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    Some((source.subslice(0, keyword.len()), after_keyword))
}

pub fn starts_timeline_comment(text: &str) -> bool {
    if text.starts_with('%') && !text.starts_with("%{") {
        return true;
    }

    // The selected Jison lexer also skips [^\}]\%\%[^\n]* before other
    // tokens. Its JavaScript regexp consumes one UTF-16 code unit, so an
    // astral character before %% remains part of a period token.
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|ch| ch != '}' && ch.len_utf16() == 1)
        && chars.as_str().starts_with("%%")
}

pub fn parse_timeline_header(
    source: TimelineSource<'_>,
) -> Option<(TimelineDirection, TimelineSource<'_>)> {
    let (_, after_keyword) = parse_keyword_prefix(source, "timeline")?;
    let direction_text = after_keyword.text.trim_start_matches([' ', '\t']);
    if direction_text.len() < after_keyword.text.len() {
        let direction_source = after_keyword.subslice(
            after_keyword.text.len() - direction_text.len(),
            after_keyword.text.len(),
        );
        for (keyword, direction) in [
            ("LR", TimelineDirection::LeftToRight),
            ("TD", TimelineDirection::TopDown),
        ] {
            if let Some((_, remainder)) = parse_keyword_prefix(direction_source, keyword) {
                return Some((direction, remainder));
            }
        }
    }
    Some((TimelineDirection::LeftToRight, after_keyword))
}

fn scan_timeline_while(
    source: &str,
    start: usize,
    mut accepts: impl FnMut(char) -> bool,
    control: &OperationControl,
) -> OperationControlResult<usize> {
    for (count, (relative, ch)) in source[start..].char_indices().enumerate() {
        if count.is_multiple_of(128) {
            control.checkpoint()?;
        }
        if !accepts(ch) {
            return Ok(start + relative);
        }
    }
    control.checkpoint()?;
    Ok(source.len())
}

pub fn parse_space_directive<'source>(
    source: TimelineSource<'source>,
    keyword: &str,
    control: &OperationControl,
) -> OperationControlResult<Option<TimelineSource<'source>>> {
    let Some((_, after_keyword)) = parse_keyword_prefix(source, keyword) else {
        return Ok(None);
    };
    let Some(whitespace) = after_keyword.text.chars().next() else {
        return Ok(None);
    };
    if !is_ecmascript_whitespace(whitespace) {
        return Ok(None);
    }
    let value_start = whitespace.len_utf8();
    let value_end = scan_timeline_while(
        after_keyword.text,
        value_start,
        |ch| ch != '\n' && (keyword != "section" || ch != ':'),
        control,
    )?;
    let mut value = after_keyword.subslice(value_start, value_end);
    if (value_end == after_keyword.text.len()
        || after_keyword.text.as_bytes().get(value_end) == Some(&b'\n'))
        && let Some(text) = value.text.strip_suffix('\r')
    {
        value = TimelineSource::new(text, value.start);
    }
    Ok((!value.text.is_empty()).then_some(value))
}

fn parse_accessibility_delimiter(
    source: TimelineSource<'_>,
    keyword: &str,
    delimiter: u8,
    control: &OperationControl,
) -> OperationControlResult<Option<usize>> {
    if parse_keyword_prefix(source, keyword).is_none() {
        return Ok(None);
    }
    let end = scan_timeline_while(
        source.text,
        keyword.len(),
        is_ecmascript_whitespace,
        control,
    )?;
    Ok((source.text.as_bytes().get(end) == Some(&delimiter)).then_some(end))
}

pub fn parse_colon_directive<'source>(
    source: TimelineSource<'source>,
    keyword: &str,
    control: &OperationControl,
) -> OperationControlResult<Option<TimelineSource<'source>>> {
    let Some(colon) = parse_accessibility_delimiter(source, keyword, b':', control)? else {
        return Ok(None);
    };
    let value_start =
        scan_timeline_while(source.text, colon + 1, is_ecmascript_whitespace, control)?;
    let value_end = scan_timeline_while(source.text, value_start, |ch| ch != '\n', control)?;
    Ok(Some(source.subslice(value_start, value_end).trim()))
}

/// Reads a description from a source tail and reports the absolute resume offset.
pub fn parse_acc_descr_block<'source>(
    source: TimelineSource<'source>,
    resume_at: impl FnOnce(usize),
    control: &OperationControl,
) -> OperationControlResult<Option<ParsedAccDescrBlock<'source>>> {
    control.checkpoint()?;
    let Some(open) = parse_accessibility_delimiter(source, "accDescr", b'{', control)? else {
        return Ok(None);
    };
    let start = source.start;
    let content_start = open + 1;
    let content_end = scan_timeline_while(source.text, content_start, |ch| ch != '}', control)?;
    let closing = (content_end < source.text.len()).then_some(SourceSpan::new(
        start + content_end,
        start + content_end + 1,
    ));
    resume_at(closing.map_or(source.end(), |span| span.end));
    Ok(Some(ParsedAccDescrBlock {
        keyword: SourceSpan::new(start, start + "accDescr".len()),
        closing,
        value: source.subslice(content_start, content_end).trim(),
    }))
}

fn discard_timeline_event_line(
    resume_at: &mut impl FnMut(usize),
    source: TimelineSource<'_>,
    control: &OperationControl,
) -> OperationControlResult<()> {
    let end = scan_timeline_while(source.text, 0, |ch| ch != '\n', control)?;
    resume_at(source.start + end);
    Ok(())
}

/// Reads events from a source tail, reporting each resume offset as it is reached.
pub fn parse_timeline_events<'source>(
    mut source: TimelineSource<'source>,
    mut resume_at: impl FnMut(usize),
    control: &OperationControl,
) -> OperationControlResult<ParsedTimelineEvents<'source>> {
    let mut events = Vec::new();

    while !source.text.is_empty() {
        control.checkpoint()?;
        if starts_timeline_comment(source.text) {
            break;
        }
        if !source.text.starts_with(':') {
            discard_timeline_event_line(&mut resume_at, source, control)?;
            return Ok(ParsedTimelineEvents {
                events,
                issue: Some(TimelineEventIssue::InvalidToken(source.span())),
            });
        }
        let after_colon = source.subslice(1, source.text.len());
        let Some(whitespace) = after_colon
            .text
            .chars()
            .next()
            .filter(|ch| is_ecmascript_whitespace(*ch))
        else {
            discard_timeline_event_line(&mut resume_at, source, control)?;
            return Ok(ParsedTimelineEvents {
                events,
                issue: Some(TimelineEventIssue::MissingWhitespace(after_colon.start)),
            });
        };
        let separator_len = if after_colon.text.starts_with("\r\n") {
            2
        } else {
            whitespace.len_utf8()
        };
        let body = after_colon.subslice(separator_len, after_colon.text.len());
        let mut boundary = body.text.len();
        let mut continues = false;
        for (count, (offset, ch)) in body.text.char_indices().enumerate() {
            if count.is_multiple_of(128) {
                control.checkpoint()?;
            }
            if ch == '\n' {
                boundary = offset;
                break;
            }
            if ch == ':'
                && body.text[offset + 1..]
                    .chars()
                    .next()
                    .is_some_and(is_ecmascript_whitespace)
            {
                boundary = offset;
                continues = true;
                break;
            }
        }
        let mut value = body.subslice(0, boundary);
        if !continues && let Some(text) = value.text.strip_suffix('\r') {
            value = TimelineSource::new(text, value.start);
        }
        if value.text.is_empty() {
            discard_timeline_event_line(&mut resume_at, source, control)?;
            let issue = if matches!(whitespace, '\r' | '\n') {
                TimelineEventIssue::MissingWhitespace(after_colon.start)
            } else {
                TimelineEventIssue::ExpectedText(value.start)
            };
            return Ok(ParsedTimelineEvents {
                events,
                issue: Some(issue),
            });
        }
        resume_at(value.end());
        events.push(value);
        if !continues {
            break;
        }
        source = body.subslice(boundary, body.text.len());
    }

    Ok(ParsedTimelineEvents {
        events,
        issue: None,
    })
}

pub fn parse_timeline_period(line: TimelineSource<'_>) -> ParsedTimelinePeriod<'_> {
    let boundary = line
        .text
        .char_indices()
        .find(|(_, ch)| matches!(ch, ':' | '#'));
    let (period, suffix) = match boundary {
        Some((offset, ':')) => (
            line.subslice(0, offset),
            TimelinePeriodSuffix::Events(line.subslice(offset, line.text.len())),
        ),
        Some((offset, '#')) => (line.subslice(0, offset), TimelinePeriodSuffix::HashComment),
        Some(_) => unreachable!("Timeline period boundary is closed"),
        None => (line, TimelinePeriodSuffix::None),
    };
    ParsedTimelinePeriod {
        period,
        task: period.trim(),
        suffix,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_raw_keyword_scan_preserves_the_one_character_boundary() {
        let control = OperationControl::new();
        for source in ["title\r\nValue", "title \nValue", "title\n\nValue", "title"] {
            assert!(
                parse_space_directive(TimelineSource::new(source, 0), "title", &control)
                    .unwrap()
                    .is_none(),
                "{source:?}"
            );
        }
        let title_source = "title\nValue:literal;#retained";
        let title = parse_space_directive(TimelineSource::new(title_source, 7), "title", &control)
            .unwrap()
            .unwrap();
        assert_eq!(title.text, "Value:literal;#retained");
        assert_eq!(title.span(), SourceSpan::new(13, 7 + title_source.len()));

        let section_source = "section\nValue: Event";
        let section =
            parse_space_directive(TimelineSource::new(section_source, 0), "section", &control)
                .unwrap()
                .unwrap();
        assert_eq!(section.text, "Value");
        assert_eq!(section.end(), "section\nValue".len());
    }

    #[test]
    fn timeline_multiline_token_scans_observe_cancellation() {
        for (source, keyword) in [
            (format!("title\n{}", "\u{754c}".repeat(4096)), "title"),
            (format!("section\n{}", "\u{754c}".repeat(4096)), "section"),
        ] {
            let control = OperationControl::new();
            control.cancel_after_checkpoints(2);
            assert!(
                parse_space_directive(TimelineSource::new(&source, 0), keyword, &control).is_err()
            );
        }
        let scalar = format!("accTitle{}:Value", "\u{feff}\n".repeat(2048));
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);
        assert!(
            parse_colon_directive(TimelineSource::new(&scalar, 0), "accTitle", &control).is_err()
        );

        let block = format!("accDescr{{{}}}", "\u{85}".repeat(4096));
        let control = OperationControl::new();
        control.cancel_after_checkpoints(3);
        assert!(parse_acc_descr_block(TimelineSource::new(&block, 0), |_| {}, &control).is_err());

        let event = format!(":\n{}", "\u{754c}".repeat(4096));
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);
        assert!(parse_timeline_events(TimelineSource::new(&event, 0), |_| {}, &control).is_err());
    }

    #[test]
    fn timeline_event_resumes_and_checkpoints_keep_original_byte_boundaries() {
        let payload = "界".repeat(129);
        let source = format!(":\r\n{payload}: 続き\r\n");
        let start = "timeline\r\nP ".len();
        let first = SourceSpan::new(start + ":\r\n".len(), start + ":\r\n".len() + payload.len());
        let second = SourceSpan::new(first.end + ": ".len(), start + source.len() - "\r\n".len());

        for allowance in 0..=5 {
            let control = OperationControl::new();
            control.cancel_after_checkpoints(allowance);
            let mut resumes = Vec::new();
            let parsed = parse_timeline_events(
                TimelineSource::new(&source, start),
                |offset| resumes.push(offset),
                &control,
            );
            if allowance == 5 {
                let parsed = parsed.unwrap();
                assert_eq!(parsed.issue, None);
                assert_eq!(parsed.events.len(), 2);
                assert_eq!(parsed.events[0].text, payload);
                assert_eq!(parsed.events[0].span(), first);
                assert_eq!(parsed.events[1].text, "続き");
                assert_eq!(parsed.events[1].span(), second);
                assert_eq!(resumes, [first.end, second.end]);
                assert!(control.checkpoint().is_err());
            } else {
                assert!(parsed.is_err());
                if allowance >= 3 {
                    assert_eq!(resumes, [first.end]);
                } else {
                    assert!(resumes.is_empty());
                }
            }
        }
    }

    #[test]
    fn timeline_malformed_event_keeps_insertions_and_skips_through_the_same_newline() {
        let start = 7;
        for (source, issue) in [
            (
                ":bad\r\nNext",
                TimelineEventIssue::MissingWhitespace(start + 1),
            ),
            (": \r\nNext", TimelineEventIssue::ExpectedText(start + 2)),
            (
                ":\r\n\r\nNext",
                TimelineEventIssue::MissingWhitespace(start + 1),
            ),
        ] {
            let control = OperationControl::new();
            let mut resume = None;
            let parsed = parse_timeline_events(
                TimelineSource::new(source, start),
                |offset| resume = Some(offset),
                &control,
            )
            .unwrap();
            assert!(parsed.events.is_empty());
            assert_eq!(parsed.issue, Some(issue));
            assert_eq!(resume, source.find('\n').map(|offset| start + offset));
        }
    }
}

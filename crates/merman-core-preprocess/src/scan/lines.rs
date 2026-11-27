pub fn strip_line_ending(segment: &str) -> &str {
    let segment = segment.strip_suffix('\n').unwrap_or(segment);
    segment.strip_suffix('\r').unwrap_or(segment)
}

pub struct LineCursor<'a> {
    source: &'a str,
    segments: std::str::SplitInclusive<'a, char>,
    remainder: Option<LineRemainder>,
    current_segment: Option<(usize, usize)>,
    offset: usize,
    #[cfg(test)]
    scanned_bytes: usize,
}

struct LineRemainder {
    start: usize,
    end: usize,
    advance_offset: bool,
}

impl<'a> LineCursor<'a> {
    pub fn new(code: &'a str) -> Self {
        Self {
            source: code,
            segments: code.split_inclusive('\n'),
            remainder: None,
            current_segment: None,
            offset: 0,
            #[cfg(test)]
            scanned_bytes: 0,
        }
    }

    pub fn next_line(&mut self) -> Option<(&'a str, usize)> {
        if let Some(remainder) = self.remainder.take() {
            if remainder.advance_offset {
                self.offset = remainder.end;
            }
            return Some((
                strip_line_ending(&self.source[remainder.start..remainder.end]),
                remainder.start,
            ));
        }
        let segment = self.segments.next()?;
        let line_start = self.offset;
        self.offset += segment.len();
        self.current_segment = Some((line_start, self.offset));
        #[cfg(test)]
        {
            self.scanned_bytes += segment.len();
        }
        Some((strip_line_ending(segment), line_start))
    }

    /// Replays the unconsumed suffix of the current physical line before advancing to the next.
    ///
    /// Stateful lexers resume their default mode immediately after a closing delimiter. Manual
    /// line parsers use this operation to preserve the same source-backed behavior without
    /// rescanning or manufacturing a synthetic line.
    pub fn resume_same_line_at(&mut self, start: usize) {
        debug_assert!(self.remainder.is_none());
        debug_assert!(start <= self.source.len());
        debug_assert!(self.source.is_char_boundary(start));
        let end = if let Some((segment_start, segment_end)) = self.current_segment
            && start >= segment_start
            && start < segment_end
        {
            segment_end - usize::from(self.source.as_bytes().get(segment_end - 1) == Some(&b'\n'))
        } else {
            let end = self.source[start..]
                .find('\n')
                .map_or(self.source.len(), |relative| start + relative);
            #[cfg(test)]
            {
                self.scanned_bytes += end - start;
            }
            end
        };
        self.remainder = Some(LineRemainder {
            start,
            end,
            advance_offset: false,
        });
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn source(&self) -> &'a str {
        self.source
    }

    pub fn resume_at(&mut self, start: usize) {
        debug_assert!(start <= self.source.len());
        debug_assert!(self.source.is_char_boundary(start));
        self.remainder = None;
        self.offset = start;
        if let Some((segment_start, segment_end)) = self.current_segment
            && start >= segment_start
            && start < segment_end
        {
            self.remainder = Some(LineRemainder {
                start,
                end: segment_end,
                advance_offset: true,
            });
            return;
        }
        if self.current_segment.is_some_and(|(_, end)| start == end) {
            return;
        }
        self.segments = self.source[start..].split_inclusive('\n');
        self.current_segment = None;
    }
}

#[cfg(test)]
mod tests {
    use super::{LineCursor, strip_line_ending};

    #[test]
    fn strip_line_ending_removes_lf_and_crlf() {
        assert_eq!(strip_line_ending("line\n"), "line");
        assert_eq!(strip_line_ending("line\r\n"), "line");
        assert_eq!(strip_line_ending("line"), "line");
    }

    #[test]
    fn line_cursor_tracks_utf8_byte_offsets_and_strips_endings() {
        let mut cursor = LineCursor::new("alpha\r\n\u{03b2}eta");

        assert_eq!(cursor.next_line(), Some(("alpha", 0)));
        assert_eq!(cursor.offset(), "alpha\r\n".len());
        assert_eq!(cursor.next_line(), Some(("\u{03b2}eta", "alpha\r\n".len())));
        assert_eq!(cursor.offset(), "alpha\r\n\u{03b2}eta".len());
        assert_eq!(cursor.next_line(), None);
    }

    #[test]
    fn line_cursor_replays_a_utf8_same_line_remainder_before_the_next_line() {
        let source = "first } \u{03b2}eta\r\nnext";
        let mut cursor = LineCursor::new(source);

        assert_eq!(cursor.next_line(), Some(("first } \u{03b2}eta", 0)));
        let remainder_start = source.find('\u{03b2}').unwrap();
        cursor.resume_same_line_at(remainder_start);

        assert_eq!(cursor.next_line(), Some(("\u{03b2}eta", remainder_start)));
        assert_eq!(
            cursor.next_line(),
            Some(("next", "first } \u{03b2}eta\r\n".len()))
        );
        assert_eq!(cursor.next_line(), None);
    }

    #[test]
    fn line_cursor_resumes_across_lines_without_replaying_consumed_segments() {
        let source = "title\r\n\u{03b2} value;task\r\nnext\n";
        let mut cursor = LineCursor::new(source);
        assert_eq!(cursor.source(), source);
        assert_eq!(cursor.next_line(), Some(("title", 0)));
        let start = source.find(";task").unwrap();
        cursor.resume_at(start);
        assert_eq!(cursor.offset(), start);
        assert_eq!(cursor.next_line(), Some((";task", start)));
        assert_eq!(cursor.offset(), source.find("next").unwrap());
        assert_eq!(
            cursor.next_line(),
            Some(("next", source.find("next").unwrap()))
        );
        assert_eq!(cursor.next_line(), None);
    }

    #[test]
    fn line_cursor_resume_clears_pending_input_and_handles_line_boundaries_and_eof() {
        let source = "first } tail\n\u{03b2}eta\nlast";
        let mut cursor = LineCursor::new(source);
        cursor.next_line();
        cursor.resume_same_line_at(source.find("tail").unwrap());
        let start = source.find('\u{03b2}').unwrap();
        cursor.resume_at(start);
        assert_eq!(cursor.next_line(), Some(("\u{03b2}eta", start)));
        let newline = source.find("\nlast").unwrap();
        cursor.resume_at(newline);
        assert_eq!(cursor.next_line(), Some(("", newline)));
        assert_eq!(cursor.next_line(), Some(("last", newline + 1)));
        cursor.resume_at(source.len());
        assert_eq!(cursor.offset(), source.len());
        assert_eq!(cursor.next_line(), None);
    }

    #[test]
    fn repeated_same_line_resumption_reuses_the_original_newline_search() {
        for ending in ["\n", "\r\n", ""] {
            let source = format!("{}{}", " accDescr{界}".repeat(4096), ending);
            let mut cursor = LineCursor::new(&source);
            cursor.next_line().unwrap();
            let scanned = cursor.scanned_bytes;
            for (start, _) in source.match_indices(" accDescr") {
                cursor.resume_at(start);
                assert_eq!(cursor.offset(), start);
                let (_, actual) = cursor.next_line().unwrap();
                assert_eq!(actual, start);
                assert_eq!(cursor.offset(), source.len());
                cursor.resume_same_line_at(start + 1);
                assert_eq!(cursor.next_line().unwrap().1, start + 1);
                assert_eq!(cursor.offset(), source.len());
            }
            assert_eq!(cursor.scanned_bytes, scanned);
            assert_eq!(scanned, source.len());
            cursor.resume_at(source.len());
            assert_eq!(cursor.next_line(), None);
        }
    }
}

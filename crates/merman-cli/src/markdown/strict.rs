use super::{MarkdownChart, MarkdownChartLimitExceeded, MarkdownFenceLocation, admit_chart};

const LANGUAGE: &str = "mermaid";

// Pinned source: @mermaid-js/mermaid-cli@11.16.0, src/index.js:746.
// /^[^\S\n]*[`:]{3}(?:mermaid)([^\S\n]*\r?\n([\s\S]*?))[`:]{3}[^\S\n]*$/gm
pub(super) fn scan<'source>(
    source: &'source str,
    max_charts: Option<u64>,
) -> Result<Vec<MarkdownChart<'source>>, MarkdownChartLimitExceeded> {
    let mut charts = Vec::new();
    let mut search_start = 0;
    let mut search_line = 1;

    while search_start < source.len() {
        let Some(opening) = find_opening(source, search_start, search_line) else {
            break;
        };
        let Some((closing_start, match_end)) = find_closing_marker(source, opening.body_start)
        else {
            // A strict closing marker is independent of its opening marker. If
            // none exists after this opening, no later opening can close either.
            break;
        };

        let location = MarkdownFenceLocation {
            line: opening.line,
            column: opening.column,
        };
        admit_chart(charts.len(), max_charts, location)?;
        charts.push(MarkdownChart::new(
            source,
            opening.match_start..match_end,
            opening.body_start..closing_start,
            location,
        ));

        let next_search = next_anchor_after(source, match_end);
        search_line += logical_position(&source[search_start..next_search]).0;
        search_start = next_search;
    }

    Ok(charts)
}

#[derive(Debug, Clone, Copy)]
struct Opening {
    match_start: usize,
    body_start: usize,
    line: usize,
    column: usize,
}

fn find_opening(source: &str, mut anchor: usize, anchor_line: usize) -> Option<Opening> {
    let mut line = anchor_line;
    while anchor < source.len() {
        let line_feed = source[anchor..].find('\n').map(|at| anchor + at)?;
        let segment = &source[anchor..line_feed];
        if let Some((match_offset, marker_offset)) = strict_opening_in_segment(segment) {
            let marker = anchor + marker_offset;
            let (line_delta, column) = logical_position(&source[anchor..marker]);
            return Some(Opening {
                match_start: anchor + match_offset,
                body_start: line_feed + 1,
                line: line + line_delta,
                column,
            });
        }

        let next_anchor = line_feed + 1;
        line += logical_position(&source[anchor..next_anchor]).0;
        anchor = next_anchor;
    }
    None
}

fn strict_opening_in_segment(segment: &str) -> Option<(usize, usize)> {
    let token_end = trim_ecmascript_whitespace_end(segment);
    let token_start = token_end.checked_sub(3 + LANGUAGE.len())?;
    let token = segment.as_bytes().get(token_start..token_end)?;
    if !token[..3]
        .iter()
        .all(|marker| matches!(marker, b'`' | b':'))
        || token.get(3..) != Some(LANGUAGE.as_bytes())
    {
        return None;
    }

    let prefix = &segment[..token_start];
    let Some((last_non_whitespace, _)) = prefix
        .char_indices()
        .rev()
        .find(|(_, ch)| !is_ecmascript_whitespace_except_lf(*ch))
    else {
        return Some((0, token_start));
    };
    let after_non_whitespace =
        last_non_whitespace + prefix[last_non_whitespace..].chars().next()?.len_utf8();
    let match_offset = prefix[after_non_whitespace..]
        .char_indices()
        .find_map(|(offset, ch)| {
            is_ecmascript_line_terminator(ch)
                .then_some(after_non_whitespace + offset + ch.len_utf8())
        })?;
    Some((match_offset, token_start))
}

fn find_closing_marker(source: &str, body_start: usize) -> Option<(usize, usize)> {
    let mut segment_start = body_start;
    loop {
        let terminator = next_line_terminator(source, segment_start);
        let segment_end = terminator.map_or(source.len(), |(at, _)| at);
        let segment = &source[segment_start..segment_end];
        let content_end = trim_ecmascript_whitespace_end(segment);
        if let Some(marker_offset) = content_end.checked_sub(3)
            && segment
                .as_bytes()
                .get(marker_offset..content_end)
                .is_some_and(|markers| markers.iter().all(|marker| matches!(marker, b'`' | b':')))
        {
            let closing_start = segment_start + marker_offset;
            let match_end = greedy_trailing_match_end(source, closing_start + 3);
            return Some((closing_start, match_end));
        }

        let Some((_, terminator_len)) = terminator else {
            return None;
        };
        segment_start = segment_end + terminator_len;
        if segment_start > source.len() {
            return None;
        }
    }
}

fn greedy_trailing_match_end(source: &str, mut cursor: usize) -> usize {
    let mut last_anchor = None;
    while cursor < source.len() {
        let ch = source[cursor..]
            .chars()
            .next()
            .expect("cursor remains on a character boundary");
        if ch == '\n' {
            return cursor;
        }
        if !is_ecmascript_whitespace_except_lf(ch) {
            return last_anchor.unwrap_or(cursor);
        }
        if is_ecmascript_line_terminator(ch) {
            last_anchor = Some(cursor);
        }
        cursor += ch.len_utf8();
    }
    cursor
}

fn next_anchor_after(source: &str, match_end: usize) -> usize {
    next_line_terminator(source, match_end).map_or(source.len(), |(at, len)| at + len)
}

fn next_line_terminator(source: &str, start: usize) -> Option<(usize, usize)> {
    source[start..].char_indices().find_map(|(offset, ch)| {
        is_ecmascript_line_terminator(ch).then_some((start + offset, ch.len_utf8()))
    })
}

fn trim_ecmascript_whitespace_end(value: &str) -> usize {
    value
        .char_indices()
        .rev()
        .find_map(|(index, ch)| {
            (!is_ecmascript_whitespace_except_lf(ch)).then_some(index + ch.len_utf8())
        })
        .unwrap_or(0)
}

fn logical_position(value: &str) -> (usize, usize) {
    let mut breaks = 0;
    let mut column = 1;
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                breaks += 1;
                column = 1;
            }
            '\n' | '\u{2028}' | '\u{2029}' => {
                breaks += 1;
                column = 1;
            }
            _ => column += 1,
        }
    }
    (breaks, column)
}

fn is_ecmascript_line_terminator(ch: char) -> bool {
    matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

fn is_ecmascript_whitespace_except_lf(ch: char) -> bool {
    matches!(
        ch,
        '\u{0009}'
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
    use super::*;

    #[test]
    fn snapshots_the_upstream_regex_quirks() {
        let source = concat!(
            "`::mermaid\r\n",
            "flowchart LR\r\n",
            "Body-->Mixed\r\n",
            "prefix:`::  \r\n",
            "```Mermaid\n",
            "flowchart LR\n",
            "Ignored-->Case\n",
            "```\n",
            "~~~mermaid\n",
            "flowchart LR\n",
            "Ignored-->Tilde\n",
            "~~~\n",
        );

        let charts = scan(source, None).expect("scan");

        assert_eq!(charts.len(), 1);
        assert_eq!(
            charts[0].definition(),
            "flowchart LR\r\nBody-->Mixed\r\nprefix:"
        );
        assert_eq!(
            charts[0].location(),
            MarkdownFenceLocation { line: 1, column: 1 }
        );
        assert_eq!(
            &source[charts[0].source_span()],
            "`::mermaid\r\nflowchart LR\r\nBody-->Mixed\r\nprefix:`::  \r"
        );
    }

    #[test]
    fn ignores_long_openings_and_unclosed_fences_but_borrows_crlf_body() {
        let source = concat!(
            "````mermaid\n",
            "ignored\n",
            "````\n",
            "```mermaid\r\n",
            "flowchart LR\r\n",
            "A-->B\r\n",
            "```\r\n",
            "```mermaid\n",
            "unclosed\n",
        );

        let charts = scan(source, None).expect("scan");

        assert_eq!(charts.len(), 1);
        assert_eq!(charts[0].definition(), "flowchart LR\r\nA-->B\r\n");
        assert_eq!(&source[charts[0].definition_span()], charts[0].definition());
    }

    #[test]
    fn follows_ecmascript_multiline_anchors_without_accepting_bare_cr_openers() {
        let source = concat!(
            "noise\r```mermaid\n",
            "A\n",
            "```\n",
            "noise\u{2028}```mermaid\n",
            "B```\u{2028}tail\n",
            "```mermaid\rC\r```",
        );

        let charts = scan(source, None).expect("scan");

        assert_eq!(charts.len(), 2);
        assert_eq!(charts[0].definition(), "A\n");
        assert_eq!(
            charts[0].location(),
            MarkdownFenceLocation { line: 2, column: 1 }
        );
        assert_eq!(charts[1].definition(), "B");
        assert_eq!(
            charts[1].location(),
            MarkdownFenceLocation { line: 6, column: 1 }
        );
    }

    #[test]
    fn non_ascii_text_after_markers_is_rejected_without_slicing_panics() {
        let source = "```💡💡\nnot mermaid\n```\n";

        assert!(scan(source, None).expect("scan").is_empty());
    }

    #[test]
    fn locks_the_remaining_opening_and_closing_divergences() {
        for (source, expected_body) in [
            ("    ```mermaid \t\nA\n```\n", "A\n"),
            ("\t:::mermaid\nB\n``:\n", "B\n"),
            ("```mermaid\nC```\n", "C"),
            ("```mermaid\nD\n````\n", "D\n`"),
            ("````text\n```mermaid\nE\n```\n````\n", "E\n"),
        ] {
            let charts = scan(source, None).expect("scan accepted fixture");
            assert_eq!(charts.len(), 1, "{source:?}");
            assert_eq!(charts[0].definition(), expected_body, "{source:?}");
        }

        for source in [
            "```mermaid title=x\nA\n```\n",
            "``` mermaid\nA\n```\n",
            "```Mermaid\nA\n```\n",
            "~~~mermaid\nA\n~~~\n",
            "````mermaid\nA\n````\n",
            "```mermaid\nA\n",
            "```mermaid\nA\n```suffix\n",
        ] {
            assert!(
                scan(source, None)
                    .expect("scan rejected fixture")
                    .is_empty(),
                "{source:?}"
            );
        }
    }
}

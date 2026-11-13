//! Lightweight line-oriented fence location for Markdown and MDX editor buffers.
//!
//! This is an editor-host contract, not a CommonMark parser or an MDX transform. Both hosts
//! accept backtick, tilde and colon fences with zero to three ASCII spaces of indentation.
//! The case-insensitive `mermaid` language must end at whitespace or the end of the line.
//! Other languages and bare fences hide nested examples. Closers need the same marker, at
//! least the opening length and only trailing whitespace. Unfinished fences extend to EOF.
//!
//! Outside fences, literal `<!--` opens a non-nesting HTML comment through the next `-->`.
//! Every transition on a physical line is observed, but a line that starts inside a comment
//! cannot open a fence. Fence bodies and info strings never change host comment state.
//! Locations retain the original UTF-8 bytes, including LF, CRLF and bare CR line endings.
//! No semantic capture, source map, CommonMark tree or JavaScript/JSX parsing is required.

use crate::{AnalysisCancellationToken, AnalysisCancelled};
use std::ops::{ControlFlow, Range};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FenceMarker {
    Backtick,
    Tilde,
    Colon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FenceDelimiter {
    marker: FenceMarker,
    len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FenceDelimiterSpans {
    pub opening: Range<usize>,
    pub closing: Option<Range<usize>>,
}

impl FenceDelimiter {
    pub const fn new(marker: FenceMarker, len: usize) -> Self {
        Self { marker, len }
    }

    pub const fn marker(self) -> FenceMarker {
        self.marker
    }

    pub const fn marker_len(self) -> usize {
        self.len
    }

    const fn marker_byte(self) -> u8 {
        match self.marker {
            FenceMarker::Backtick => b'`',
            FenceMarker::Tilde => b'~',
            FenceMarker::Colon => b':',
        }
    }
}

/// Original byte ranges for one active Mermaid fence, including unfinished and empty bodies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorHostFence {
    pub fence: Range<usize>,
    pub body: Range<usize>,
    pub delimiter: FenceDelimiter,
    pub delimiter_spans: FenceDelimiterSpans,
}

/// Locates active Mermaid fences without parsing their bodies or constructing semantic facts.
pub fn locate_editor_host_fences_cancellable(
    text: &str,
    cancellation: &AnalysisCancellationToken,
) -> Result<Vec<EditorHostFence>, AnalysisCancelled> {
    let mut fences = Vec::new();
    match visit_editor_host_fences(
        text,
        cancellation,
        |_| ControlFlow::<std::convert::Infallible>::Continue(()),
        |fence| fences.push(fence),
    )? {
        ControlFlow::Continue(()) => Ok(fences),
        ControlFlow::Break(never) => match never {},
    }
}

struct ActiveFence {
    start: usize,
    body_start: usize,
    opening_marker: Range<usize>,
    opening: FenceOpening,
}

pub(crate) fn visit_editor_host_fences<B>(
    text: &str,
    cancellation: &AnalysisCancellationToken,
    mut visit_opening: impl FnMut(Range<usize>) -> ControlFlow<B>,
    mut visit: impl FnMut(EditorHostFence),
) -> Result<ControlFlow<B>, AnalysisCancelled> {
    let mut cursor = 0;
    let mut in_comment = false;
    let mut active: Option<ActiveFence> = None;
    while cursor < text.len() {
        cancellation.checkpoint()?;
        let line_end = next_line_end_cancellable(text, cursor, cancellation)?;
        let line = trim_line_ending(&text[cursor..line_end]);
        if let Some(fence) = &active {
            if let Some(closing) =
                matching_closing_fence_marker(line, fence.opening.delimiter, cancellation)?
            {
                if fence.opening.is_mermaid {
                    visit(EditorHostFence {
                        fence: fence.start..line_end,
                        body: fence.body_start..cursor,
                        delimiter: fence.opening.delimiter,
                        delimiter_spans: FenceDelimiterSpans {
                            opening: fence.opening_marker.clone(),
                            closing: Some(cursor + closing.start..cursor + closing.end),
                        },
                    });
                }
                active = None;
            }
        } else if in_comment {
            in_comment = comment_state_after_line(line, in_comment, cancellation)?;
        } else if let Some(opening) = fence_opening(line, cancellation)? {
            let marker_start = cursor + opening.marker_offset;
            let opening_marker = marker_start..marker_start + opening.delimiter.marker_len();
            if opening.is_mermaid {
                if let ControlFlow::Break(value) = visit_opening(opening_marker.clone()) {
                    return Ok(ControlFlow::Break(value));
                }
            }
            active = Some(ActiveFence {
                start: cursor,
                body_start: line_end,
                opening_marker,
                opening,
            });
        } else {
            in_comment = comment_state_after_line(line, in_comment, cancellation)?;
        }
        cursor = line_end;
    }
    if let Some(fence) = active.filter(|fence| fence.opening.is_mermaid) {
        cancellation.checkpoint()?;
        visit(EditorHostFence {
            fence: fence.start..text.len(),
            body: fence.body_start..text.len(),
            delimiter: fence.opening.delimiter,
            delimiter_spans: FenceDelimiterSpans {
                opening: fence.opening_marker,
                closing: None,
            },
        });
    }
    cancellation.checkpoint()?;
    Ok(ControlFlow::Continue(()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FenceOpening {
    delimiter: FenceDelimiter,
    is_mermaid: bool,
    marker_offset: usize,
}

const FENCE_SCAN_CHECKPOINT_BYTES: usize = 4 * 1024;

fn comment_state_after_line(
    line: &str,
    mut in_comment: bool,
    cancellation: &AnalysisCancellationToken,
) -> Result<bool, AnalysisCancelled> {
    let bytes = line.as_bytes();
    let mut cursor = 0;
    let mut next_checkpoint = 0;
    while cursor < bytes.len() {
        if cursor >= next_checkpoint {
            cancellation.checkpoint()?;
            next_checkpoint = cursor.saturating_add(FENCE_SCAN_CHECKPOINT_BYTES);
        }
        let marker: &[u8] = if in_comment { b"-->" } else { b"<!--" };
        if bytes[cursor..].starts_with(marker) {
            in_comment = !in_comment;
            cursor += marker.len();
        } else {
            cursor += 1;
        }
    }
    cancellation.checkpoint()?;
    Ok(in_comment)
}

fn fence_opening(
    line: &str,
    cancellation: &AnalysisCancellationToken,
) -> Result<Option<FenceOpening>, AnalysisCancelled> {
    cancellation.checkpoint()?;
    let Some(trimmed) = trim_fence_indent(line) else {
        return Ok(None);
    };
    let marker_offset = line.len() - trimmed.len();
    let Some(first) = trimmed.as_bytes().first().copied() else {
        return Ok(None);
    };
    let marker = match first {
        b'`' => FenceMarker::Backtick,
        b'~' => FenceMarker::Tilde,
        b':' => FenceMarker::Colon,
        _ => return Ok(None),
    };
    let len = repeated_marker_len_cancellable(trimmed.as_bytes(), first, cancellation)?;
    if len < 3 {
        return Ok(None);
    }
    let rest = trim_start_whitespace_cancellable(&trimmed[len..], cancellation)?;
    let language = "mermaid";
    let is_mermaid = rest
        .as_bytes()
        .get(..language.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(language.as_bytes()))
        && rest[language.len()..]
            .chars()
            .next()
            .is_none_or(char::is_whitespace);
    cancellation.checkpoint()?;
    Ok(Some(FenceOpening {
        delimiter: FenceDelimiter::new(marker, len),
        is_mermaid,
        marker_offset,
    }))
}

fn matching_closing_fence_marker(
    line: &str,
    delimiter: FenceDelimiter,
    cancellation: &AnalysisCancellationToken,
) -> Result<Option<Range<usize>>, AnalysisCancelled> {
    cancellation.checkpoint()?;
    let Some(trimmed) = trim_fence_indent(line) else {
        return Ok(None);
    };
    let marker_offset = line.len() - trimmed.len();
    let len =
        repeated_marker_len_cancellable(trimmed.as_bytes(), delimiter.marker_byte(), cancellation)?;
    if len < delimiter.marker_len() {
        return Ok(None);
    }
    Ok(all_whitespace_cancellable(&trimmed[len..], cancellation)?
        .then_some(marker_offset..marker_offset + len))
}

fn trim_fence_indent(line: &str) -> Option<&str> {
    let mut spaces = 0;
    for (index, byte) in line.bytes().enumerate() {
        match byte {
            b' ' if spaces < 3 => spaces += 1,
            b' ' | b'\t' => return None,
            _ => return Some(&line[index..]),
        }
    }
    Some("")
}

fn repeated_marker_len_cancellable(
    bytes: &[u8],
    marker: u8,
    cancellation: &AnalysisCancellationToken,
) -> Result<usize, AnalysisCancelled> {
    let mut index = 0usize;
    let mut next_checkpoint = 0usize;
    while bytes.get(index) == Some(&marker) {
        if index >= next_checkpoint {
            cancellation.checkpoint()?;
            next_checkpoint = index.saturating_add(FENCE_SCAN_CHECKPOINT_BYTES);
        }
        index += 1;
    }
    cancellation.checkpoint()?;
    Ok(index)
}

fn trim_start_whitespace_cancellable<'a>(
    text: &'a str,
    cancellation: &AnalysisCancellationToken,
) -> Result<&'a str, AnalysisCancelled> {
    let mut trimmed_start = 0usize;
    let mut next_checkpoint = 0usize;
    for (offset, character) in text.char_indices() {
        if offset >= next_checkpoint {
            cancellation.checkpoint()?;
            next_checkpoint = offset.saturating_add(FENCE_SCAN_CHECKPOINT_BYTES);
        }
        if !character.is_whitespace() {
            return Ok(&text[offset..]);
        }
        trimmed_start = offset + character.len_utf8();
    }
    cancellation.checkpoint()?;
    Ok(&text[trimmed_start..])
}

fn all_whitespace_cancellable(
    text: &str,
    cancellation: &AnalysisCancellationToken,
) -> Result<bool, AnalysisCancelled> {
    let mut next_checkpoint = 0usize;
    for (offset, character) in text.char_indices() {
        if offset >= next_checkpoint {
            cancellation.checkpoint()?;
            next_checkpoint = offset.saturating_add(FENCE_SCAN_CHECKPOINT_BYTES);
        }
        if !character.is_whitespace() {
            return Ok(false);
        }
    }
    cancellation.checkpoint()?;
    Ok(true)
}

fn trim_line_ending(line: &str) -> &str {
    line.strip_suffix("\r\n")
        .or_else(|| line.strip_suffix('\n'))
        .or_else(|| line.strip_suffix('\r'))
        .unwrap_or(line)
}

fn next_line_end_cancellable(
    source: &str,
    start: usize,
    cancellation: &AnalysisCancellationToken,
) -> Result<usize, AnalysisCancelled> {
    let bytes = source.as_bytes();
    let mut index = start;
    let mut next_checkpoint = start;
    while index < bytes.len() {
        if index >= next_checkpoint {
            cancellation.checkpoint()?;
            next_checkpoint = index.saturating_add(FENCE_SCAN_CHECKPOINT_BYTES);
        }
        match bytes[index] {
            b'\n' => return Ok(index + 1),
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => return Ok(index + 2),
            b'\r' => return Ok(index + 1),
            _ => index += 1,
        }
    }
    Ok(source.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_fence_opening_marker_scan_observes_scheduled_cancellation() {
        let line = format!("{}mermaid", "`".repeat(32 * 1024));
        let cancellation = AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(2);
        assert!(matches!(
            fence_opening(&line, &cancellation),
            Err(AnalysisCancelled)
        ));
        assert!(cancellation.is_cancelled());
    }

    #[test]
    fn markdown_fence_opening_info_indent_scan_observes_scheduled_cancellation() {
        let line = format!("```{}mermaid", " ".repeat(32 * 1024));
        let cancellation = AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(4);
        assert!(matches!(
            fence_opening(&line, &cancellation),
            Err(AnalysisCancelled)
        ));
        assert!(cancellation.is_cancelled());
    }

    #[test]
    fn markdown_fence_closing_tail_scan_observes_scheduled_cancellation() {
        let line = format!("```{}", " ".repeat(32 * 1024));
        let cancellation = AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(4);
        assert!(matches!(
            matching_closing_fence_marker(
                &line,
                FenceDelimiter::new(FenceMarker::Backtick, 3),
                &cancellation,
            ),
            Err(AnalysisCancelled)
        ));
        assert!(cancellation.is_cancelled());
    }

    #[test]
    fn host_comment_scan_observes_scheduled_cancellation() {
        let line = format!("{}<!--", "😀".repeat(32 * 1024));
        let cancellation = AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(2);
        assert_eq!(
            comment_state_after_line(&line, false, &cancellation),
            Err(AnalysisCancelled)
        );
    }
}

//! Byte-level directive boundaries and quote normalization, without config interpretation.

use crate::scan::controlled::{
    ControlledScanCheckpoints, find_ascii_pattern_controlled, find_line_break_controlled,
    push_str_controlled, trim_whitespace_bounds_controlled,
};
use merman_model::{OperationControl, OperationControlResult};
use std::borrow::Cow;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectiveRecoveryMode {
    /// An unmatched opener consumes to the end of the input.
    Strict,
    /// Stop an incomplete directive at the next line break or opener, whichever comes first.
    RecoverLine,
}

/// One directive's lexical bounds in the exact UTF-8 input passed to the scanner.
#[derive(Debug)]
pub struct DirectiveBlock {
    /// The trimmed body, excluding both delimiters.
    pub raw: Range<usize>,
    pub complete: bool,
    /// The full directive, including its opening delimiter and any matched closing delimiter.
    pub range: Range<usize>,
}

/// Finds directive blocks without inspecting their keywords, quotes, or arguments.
pub fn directive_blocks_controlled(
    input: &str,
    directive_recovery: DirectiveRecoveryMode,
    control: &OperationControl,
) -> OperationControlResult<Vec<DirectiveBlock>> {
    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    let mut blocks = Vec::new();
    let mut pos = 0;
    while let Some(start) = find_ascii_pattern_controlled(input, pos, b"%%{", &mut checkpoints)? {
        let content_start = start + 3;
        let close = find_ascii_pattern_controlled(input, content_start, b"}%%", &mut checkpoints)?;
        let next_open = if directive_recovery == DirectiveRecoveryMode::RecoverLine {
            find_ascii_pattern_controlled(input, content_start, b"%%{", &mut checkpoints)?
        } else {
            None
        };

        let content_end = match directive_recovery {
            DirectiveRecoveryMode::Strict => close,
            DirectiveRecoveryMode::RecoverLine => {
                close.filter(|close| next_open.is_none_or(|open| *close < open))
            }
        };
        if let Some(content_end) = content_end {
            let end = content_end + 3;
            let raw = &input[content_start..content_end];
            let (trimmed_start, trimmed_end) =
                trim_whitespace_bounds_controlled(raw, &mut checkpoints)?;
            blocks.push(DirectiveBlock {
                raw: content_start + trimmed_start..content_start + trimmed_end,
                complete: true,
                range: start..end,
            });
            pos = end;
            continue;
        }

        let end = match directive_recovery {
            // Mermaid's optional closing marker makes an unterminated directive consume to EOF.
            DirectiveRecoveryMode::Strict => input.len(),
            DirectiveRecoveryMode::RecoverLine => {
                let line_end = find_line_break_controlled(input, content_start, &mut checkpoints)?
                    .unwrap_or(input.len());
                next_open.map_or(line_end, |open| line_end.min(open))
            }
        };
        let raw = &input[content_start..end];
        let (trimmed_start, trimmed_end) =
            trim_whitespace_bounds_controlled(raw, &mut checkpoints)?;
        blocks.push(DirectiveBlock {
            raw: content_start + trimmed_start..content_start + trimmed_end,
            complete: false,
            range: start..end,
        });
        pos = end;
    }

    checkpoints.finish()?;
    Ok(blocks)
}

/// Replaces each ASCII apostrophe with a double quote without changing any byte offsets.
///
/// This is Mermaid's lexical normalization, not a quote-aware parse or validation.
pub fn normalize_directive_quotes_controlled<'a>(
    raw: &'a str,
    control: &OperationControl,
) -> OperationControlResult<Cow<'a, str>> {
    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    let mut first_quote = None;
    for (index, byte) in raw.as_bytes().iter().copied().enumerate() {
        checkpoints.scanned(1)?;
        if byte == b'\'' {
            first_quote = Some(index);
            break;
        }
    }
    let Some(first_quote) = first_quote else {
        checkpoints.finish()?;
        return Ok(Cow::Borrowed(raw));
    };

    let mut normalized = String::with_capacity(raw.len());
    let mut cursor = 0usize;
    for (relative, byte) in raw.as_bytes()[first_quote..].iter().copied().enumerate() {
        checkpoints.scanned(1)?;
        if byte != b'\'' {
            continue;
        }
        let quote = first_quote + relative;
        push_str_controlled(&mut normalized, &raw[cursor..quote], &mut checkpoints)?;
        normalized.push('"');
        cursor = quote + 1;
    }
    push_str_controlled(&mut normalized, &raw[cursor..], &mut checkpoints)?;
    checkpoints.finish()?;
    Ok(Cow::Owned(normalized))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::controlled::CONTROLLED_SCAN_CHECKPOINT_BYTES;

    #[test]
    fn directive_blocks_preserve_trimmed_utf8_ranges() {
        let input = "前 %%{ \u{2003}init: {'clé': '🦀'}\r\n }%% %%{\t\u{2003}\n}%%";
        let blocks = directive_blocks_controlled(
            input,
            DirectiveRecoveryMode::Strict,
            &OperationControl::new(),
        )
        .unwrap();
        let [init, blank] = blocks.as_slice() else {
            panic!("expected two complete directives");
        };
        assert!(init.complete && blank.complete);
        assert_eq!(init.raw.start, input.find("init").unwrap());
        assert_eq!(&input[init.raw.clone()], "init: {'clé': '🦀'}");
        assert_eq!(
            &input[init.range.clone()],
            "%%{ \u{2003}init: {'clé': '🦀'}\r\n }%%"
        );
        assert_eq!(blank.raw, blank.range.end - 3..blank.range.end - 3);
        assert_eq!(&input[blank.range.clone()], "%%{\t\u{2003}\n}%%");
    }

    #[test]
    fn directive_blocks_keep_strict_and_line_recovery_boundaries() {
        let control = OperationControl::new();
        for line_break in ["\r", "\n", "\r\n"] {
            let input = format!("%%{{ init: 'é'{line_break}%%{{wrap}}%%{line_break}%%{{tail");
            let strict =
                directive_blocks_controlled(&input, DirectiveRecoveryMode::Strict, &control)
                    .unwrap();
            assert_eq!(strict.len(), 2);
            assert!(strict[0].complete);
            assert_eq!(
                &input[strict[0].raw.clone()],
                format!("init: 'é'{line_break}%%{{wrap")
            );
            assert!(!strict[1].complete);
            assert_eq!(strict[1].range.end, input.len());

            let recovery =
                directive_blocks_controlled(&input, DirectiveRecoveryMode::RecoverLine, &control)
                    .unwrap();
            assert_eq!(recovery.len(), 3);
            assert!(!recovery[0].complete);
            assert_eq!(&input[recovery[0].range.clone()], "%%{ init: 'é'");
            assert!(recovery[1].complete);
            assert_eq!(&input[recovery[1].raw.clone()], "wrap");
            assert!(!recovery[2].complete);
            assert_eq!(&input[recovery[2].raw.clone()], "tail");
        }

        let input = "%%{%%{wrap}%%";
        let blocks =
            directive_blocks_controlled(input, DirectiveRecoveryMode::RecoverLine, &control)
                .unwrap();
        assert_eq!(blocks.len(), 2);
        assert!(!blocks[0].complete);
        assert_eq!(blocks[0].raw, 3..3);
        assert_eq!(blocks[0].range.end, blocks[1].range.start);
        assert!(blocks[1].complete);
    }

    #[test]
    fn directive_byte_scans_preserve_checkpoint_counts() {
        let body = "é".repeat(CONTROLLED_SCAN_CHECKPOINT_BYTES / 2);
        let input = format!("%%{{{body}}}%%");
        for (mode, count) in [
            (DirectiveRecoveryMode::Strict, 3),
            (DirectiveRecoveryMode::RecoverLine, 4),
        ] {
            assert_checkpoints(count, |control| {
                directive_blocks_controlled(&input, mode, control)
            });
        }
        assert_checkpoints(3, |control| {
            let normalized = normalize_directive_quotes_controlled(&body, control)?;
            assert!(matches!(normalized, Cow::Borrowed(_)));
            Ok(normalized)
        });
        let quoted = format!("{body}'");
        assert_checkpoints(4, |control| {
            let normalized = normalize_directive_quotes_controlled(&quoted, control)?;
            assert!(matches!(normalized, Cow::Owned(_)));
            assert_eq!(normalized.strip_suffix('"'), Some(body.as_str()));
            Ok(normalized)
        });
    }

    fn assert_checkpoints<T>(
        count: usize,
        scan: impl Fn(&OperationControl) -> OperationControlResult<T>,
    ) {
        for successful in 0..count {
            let control = OperationControl::new();
            control.cancel_after_checkpoints(successful);
            assert!(
                scan(&control).is_err(),
                "must cancel after {successful} checkpoints"
            );
        }
        let control = OperationControl::new();
        control.cancel_after_checkpoints(count);
        assert!(scan(&control).is_ok());
        assert!(control.checkpoint().is_err());
    }
}

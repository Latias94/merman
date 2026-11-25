use crate::{OperationControl, OperationControlResult};

pub(super) const CONTROLLED_SCAN_CHECKPOINT_BYTES: usize = 4 * 1024;

pub(super) struct ControlledScanCheckpoints<'a> {
    control: &'a OperationControl,
    bytes_since_checkpoint: usize,
}

impl<'a> ControlledScanCheckpoints<'a> {
    pub(super) fn new(control: &'a OperationControl) -> OperationControlResult<Self> {
        control.checkpoint()?;
        Ok(Self {
            control,
            bytes_since_checkpoint: 0,
        })
    }

    pub(super) fn scanned(&mut self, bytes: usize) -> OperationControlResult<()> {
        self.bytes_since_checkpoint = self.bytes_since_checkpoint.saturating_add(bytes);
        while self.bytes_since_checkpoint >= CONTROLLED_SCAN_CHECKPOINT_BYTES {
            self.control.checkpoint()?;
            self.bytes_since_checkpoint -= CONTROLLED_SCAN_CHECKPOINT_BYTES;
        }
        Ok(())
    }

    pub(super) fn finish(&self) -> OperationControlResult<()> {
        self.control.checkpoint()
    }
}

pub(super) fn find_newline_controlled(
    input: &str,
    start: usize,
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<Option<usize>> {
    for (offset, byte) in input.as_bytes()[start..].iter().enumerate() {
        checkpoints.scanned(1)?;
        if *byte == b'\n' {
            return Ok(Some(start + offset));
        }
    }
    Ok(None)
}

pub(super) fn find_ascii_pattern_controlled(
    input: &str,
    start: usize,
    pattern: &[u8],
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<Option<usize>> {
    find_ascii_pattern_in_range_controlled(input, start, input.len(), pattern, checkpoints)
}

pub(super) fn find_ascii_pattern_in_range_controlled(
    input: &str,
    start: usize,
    end: usize,
    pattern: &[u8],
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<Option<usize>> {
    let end = end.min(input.len());
    if pattern.is_empty() || start > end || pattern.len() > end.saturating_sub(start) {
        return Ok(None);
    }
    let bytes = input.as_bytes();
    let last_start = end - pattern.len();
    for cursor in start..=last_start {
        checkpoints.scanned(1)?;
        if &bytes[cursor..cursor + pattern.len()] == pattern {
            return Ok(Some(cursor));
        }
    }
    Ok(None)
}

pub(super) fn find_line_break_controlled(
    input: &str,
    start: usize,
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<Option<usize>> {
    for (offset, byte) in input.as_bytes()[start..].iter().copied().enumerate() {
        checkpoints.scanned(1)?;
        if matches!(byte, b'\r' | b'\n') {
            return Ok(Some(start + offset));
        }
    }
    Ok(None)
}

pub(super) fn is_js_regex_whitespace(ch: char) -> bool {
    if ('\u{2000}'..='\u{200A}').contains(&ch) {
        return true;
    }

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
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    )
}

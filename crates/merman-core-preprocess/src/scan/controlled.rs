//! Cancellation-aware byte scanning for source preprocessing.

use merman_model::{OperationControl, OperationControlResult};

pub const CONTROLLED_SCAN_CHECKPOINT_BYTES: usize = 4 * 1024;

pub struct ControlledScanCheckpoints<'a> {
    control: &'a OperationControl,
    bytes_since_checkpoint: usize,
}

impl<'a> ControlledScanCheckpoints<'a> {
    pub fn new(control: &'a OperationControl) -> OperationControlResult<Self> {
        control.checkpoint()?;
        Ok(Self {
            control,
            bytes_since_checkpoint: 0,
        })
    }

    pub fn scanned(&mut self, bytes: usize) -> OperationControlResult<()> {
        self.bytes_since_checkpoint = self.bytes_since_checkpoint.saturating_add(bytes);
        while self.bytes_since_checkpoint >= CONTROLLED_SCAN_CHECKPOINT_BYTES {
            self.control.checkpoint()?;
            self.bytes_since_checkpoint -= CONTROLLED_SCAN_CHECKPOINT_BYTES;
        }
        Ok(())
    }

    pub fn finish(&self) -> OperationControlResult<()> {
        self.control.checkpoint()
    }
}

pub fn find_newline_controlled(
    input: &str,
    start: usize,
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<Option<usize>> {
    for (offset, byte) in input
        .as_bytes()
        .get(start..)
        .unwrap_or_default()
        .iter()
        .enumerate()
    {
        checkpoints.scanned(1)?;
        if *byte == b'\n' {
            return Ok(Some(start + offset));
        }
    }
    Ok(None)
}

pub fn find_ascii_pattern_controlled(
    input: &str,
    start: usize,
    pattern: &[u8],
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<Option<usize>> {
    find_ascii_pattern_in_range_controlled(input, start, input.len(), pattern, checkpoints)
}

pub fn find_ascii_pattern_in_range_controlled(
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

pub fn find_line_break_controlled(
    input: &str,
    start: usize,
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<Option<usize>> {
    for (offset, byte) in input
        .as_bytes()
        .get(start..)
        .unwrap_or_default()
        .iter()
        .copied()
        .enumerate()
    {
        checkpoints.scanned(1)?;
        if matches!(byte, b'\r' | b'\n') {
            return Ok(Some(start + offset));
        }
    }
    Ok(None)
}

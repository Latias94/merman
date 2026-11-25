//! Controlled depth scans for source configuration.

use crate::{OperationControl, OperationControlResult};
use merman_core_preprocess::scan::controlled::{
    ControlledScanCheckpoints, find_newline_controlled, trim_start_whitespace_controlled,
    trim_whitespace_bounds_controlled,
};

pub(super) const MAX_CONFIG_NESTING_DEPTH: usize = crate::MAX_DIAGRAM_NESTING_DEPTH;

#[cfg(test)]
pub(super) fn config_nesting_exceeds_limit(text: &str) -> bool {
    let control = OperationControl::new();
    config_nesting_exceeds_limit_controlled(text, &control)
        .expect("a private parse control cannot be cancelled")
}

pub(super) fn config_nesting_exceeds_limit_controlled(
    text: &str,
    control: &OperationControl,
) -> OperationControlResult<bool> {
    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    if max_flow_collection_depth_controlled::<false>(text, &mut checkpoints)?
        > MAX_CONFIG_NESTING_DEPTH
    {
        checkpoints.finish()?;
        return Ok(true);
    }
    let exceeds =
        max_yaml_indent_depth_controlled(text, &mut checkpoints)? > MAX_CONFIG_NESTING_DEPTH;
    checkpoints.finish()?;
    Ok(exceeds)
}

pub(super) fn max_flow_collection_depth_controlled<const JSON5_COMMENTS: bool>(
    text: &str,
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<usize> {
    let mut max_depth = 0usize;
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;

    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        checkpoints.scanned(ch.len_utf8())?;
        if let Some(q) = quote {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == q {
                quote = None;
            }
            continue;
        }

        match ch {
            '"' | '\'' => quote = Some(ch),
            '/' if JSON5_COMMENTS && chars.next_if_eq(&'/').is_some() => {
                checkpoints.scanned(1)?;
                for ch in chars.by_ref() {
                    checkpoints.scanned(ch.len_utf8())?;
                    if json5::char::is_json5_line_terminator(ch) {
                        break;
                    }
                }
            }
            '/' if JSON5_COMMENTS && chars.next_if_eq(&'*').is_some() => {
                checkpoints.scanned(1)?;
                let mut previous_star = false;
                for ch in chars.by_ref() {
                    checkpoints.scanned(ch.len_utf8())?;
                    if previous_star && ch == '/' {
                        break;
                    }
                    previous_star = ch == '*';
                }
            }
            '{' | '[' => {
                depth = depth.saturating_add(1);
                max_depth = max_depth.max(depth);
            }
            '}' | ']' => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }

    Ok(max_depth)
}

pub(super) fn max_yaml_indent_depth_controlled(
    text: &str,
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<usize> {
    let mut indents = Vec::<usize>::new();
    let mut max_depth = 0usize;
    let mut line_start = 0usize;

    while line_start < text.len() {
        let newline = find_newline_controlled(text, line_start, checkpoints)?;
        let line_end = newline.unwrap_or(text.len());
        let line_end =
            if newline.is_some() && line_end > line_start && text.as_bytes()[line_end - 1] == b'\r'
            {
                checkpoints.scanned(1)?;
                line_end - 1
            } else {
                line_end
            };
        let line = &text[line_start..line_end];
        let (trimmed_start, trimmed_end) = trim_whitespace_bounds_controlled(line, checkpoints)?;
        let trimmed = &line[trimmed_start..trimmed_end];
        if !trimmed.is_empty() && !trimmed.starts_with('#') {
            let indent = leading_ascii_space_count_controlled(line, checkpoints)?;
            while indents.last().is_some_and(|prev| indent <= *prev) {
                checkpoints.scanned(1)?;
                indents.pop();
            }
            indents.push(indent);
            let inline_sequence_depth =
                yaml_inline_sequence_indicator_count_controlled(trimmed, checkpoints)?;
            max_depth = max_depth.max(indents.len() + inline_sequence_depth.saturating_sub(1));
        }

        let Some(newline) = newline else {
            break;
        };
        line_start = newline + 1;
    }

    Ok(max_depth)
}

fn leading_ascii_space_count_controlled(
    text: &str,
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<usize> {
    let mut count = 0usize;
    for byte in text.bytes() {
        checkpoints.scanned(1)?;
        if byte != b' ' {
            break;
        }
        count += 1;
    }
    Ok(count)
}

fn yaml_inline_sequence_indicator_count_controlled(
    mut text: &str,
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<usize> {
    let mut count = 0usize;
    loop {
        let Some(after_dash) = text.strip_prefix('-') else {
            return Ok(count);
        };
        checkpoints.scanned(1)?;
        if let Some(ch) = after_dash.chars().next() {
            checkpoints.scanned(ch.len_utf8())?;
            if !ch.is_whitespace() {
                return Ok(count);
            }
        }
        count += 1;
        let trimmed_start = trim_start_whitespace_controlled(after_dash, checkpoints)?;
        text = &after_dash[trimmed_start..];
    }
}

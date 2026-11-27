//! Locates Mermaid frontmatter and decodes its YAML fields.

use crate::scan::controlled::{
    ControlledScanCheckpoints, find_line_break_controlled, push_str_controlled,
};
use merman_model::{OperationControl, OperationControlResult};
#[cfg(feature = "yaml")]
use serde_json::{Map, Value};
use std::borrow::Cow;

#[cfg(feature = "yaml")]
mod config;
#[cfg(feature = "yaml")]
pub use config::{
    DecodedFrontmatter, FrontmatterFields, decode_frontmatter_controlled,
    frontmatter_config_namespaces, frontmatter_config_routes, parse_frontmatter_yaml_fields,
    parse_frontmatter_yaml_fields_controlled,
};
#[cfg(feature = "yaml-editor")]
pub use config::{
    decode_frontmatter_with_evidence_controlled, parse_frontmatter_insert_order_json,
    read_ordered_frontmatter_controlled,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrontmatterByteSpan {
    pub start: usize,
    pub end: usize,
}

/// Source-backed frontmatter bounds found without materializing a dedented body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrontmatterBlockLocation<'a> {
    pub full: FrontmatterByteSpan,
    pub body: FrontmatterByteSpan,
    pub indent: &'a str,
    pub stripped: &'a str,
}

#[derive(Debug, Clone)]
pub struct FrontmatterBlock<'a> {
    pub full: FrontmatterByteSpan,
    pub body: FrontmatterByteSpan,
    pub indent: &'a str,
    pub dedented_body: Cow<'a, str>,
    pub stripped: &'a str,
}

/// Splits an optional frontmatter block using a private, non-cancellable parse control.
pub fn split_frontmatter_block(input: &str) -> Option<FrontmatterBlock<'_>> {
    let control = OperationControl::new();
    split_frontmatter_block_controlled(input, &control)
        .expect("a private parse control cannot be cancelled")
}

/// Parses frontmatter fields with caller-owned nesting and materialization budgets.
#[cfg(feature = "yaml")]
pub fn parse_frontmatter_yaml_fields_bounded_controlled(
    input: &str,
    max_input_bytes: usize,
    max_nesting_depth: usize,
    max_materialized_bytes: usize,
    control: &OperationControl,
) -> OperationControlResult<Result<Map<String, Value>, String>> {
    let parsed = crate::yaml::parse_yaml_value_with_limits_controlled(
        input,
        max_input_bytes,
        max_nesting_depth,
        max_materialized_bytes,
        control,
    )?;
    Ok(frontmatter_fields_from_yaml_value(parsed))
}

/// Applies the frontmatter object rule to a raw or source-captured YAML value.
#[cfg(feature = "yaml")]
fn frontmatter_fields_from_yaml_value(
    parsed: Result<Value, String>,
) -> Result<Map<String, Value>, String> {
    match parsed? {
        Value::Object(map) => Ok(map),
        other => {
            merman_model::json::drop_value_nonrecursive(other);
            Ok(Map::new())
        }
    }
}

/// Locates an optional frontmatter block without allocating a dedented body.
pub fn locate_frontmatter_block_controlled<'a>(
    input: &'a str,
    control: &OperationControl,
) -> OperationControlResult<Option<FrontmatterBlockLocation<'a>>> {
    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    let indent_end = frontmatter_indent_end_controlled(input, &mut checkpoints)?;
    let indent = &input[..indent_end];
    if !input[indent_end..].starts_with("---") {
        checkpoints.finish()?;
        return Ok(None);
    }

    // Match Mermaid's greedy opening whitespace and lazy body without regex backtracking.
    let mut opening_ends = Vec::new();
    for (offset, ch) in input[indent_end + 3..].char_indices() {
        checkpoints.scanned(ch.len_utf8())?;
        if !is_frontmatter_whitespace(ch) {
            break;
        }
        if matches!(ch, '\n' | '\r') {
            opening_ends.push(indent_end + 3 + offset);
        }
    }
    let Some(&first_opening_end) = opening_ends.first() else {
        checkpoints.finish()?;
        return Ok(None);
    };
    let earliest_body_start = first_opening_end + 1;
    let mut last_closing_fence = None;
    for (offset, ch) in input[earliest_body_start..].char_indices() {
        checkpoints.scanned(ch.len_utf8())?;
        let position = earliest_body_start + offset;
        if matches!(ch, '\n' | '\r')
            && frontmatter_closing_end_controlled(input, position, indent, &mut checkpoints)?
                .is_some()
        {
            last_closing_fence = Some(position);
        }
    }
    let Some(last_closing_fence) = last_closing_fence else {
        checkpoints.finish()?;
        return Ok(None);
    };
    let body_start = opening_ends
        .into_iter()
        .rfind(|&end| end < last_closing_fence)
        .unwrap_or(first_opening_end)
        + 1;
    for (offset, ch) in input[body_start..=last_closing_fence].char_indices() {
        checkpoints.scanned(ch.len_utf8())?;
        if !matches!(ch, '\n' | '\r') {
            continue;
        }
        let body_end = body_start + offset;
        if let Some(end) =
            frontmatter_closing_end_controlled(input, body_end, indent, &mut checkpoints)?
        {
            checkpoints.finish()?;
            return Ok(Some(FrontmatterBlockLocation {
                full: FrontmatterByteSpan { start: 0, end },
                body: FrontmatterByteSpan {
                    start: body_start,
                    end: body_end,
                },
                indent,
                stripped: &input[end..],
            }));
        }
    }
    checkpoints.finish()?;
    Ok(None)
}

/// Splits an optional frontmatter block while observing cooperative cancellation.
pub fn split_frontmatter_block_controlled<'a>(
    input: &'a str,
    control: &OperationControl,
) -> OperationControlResult<Option<FrontmatterBlock<'a>>> {
    let Some(location) = locate_frontmatter_block_controlled(input, control)? else {
        return Ok(None);
    };
    let body = &input[location.body.start..location.body.end];
    let dedented_body = dedent_frontmatter_body_controlled(body, location.indent, control)?;
    control.checkpoint()?;
    Ok(Some(FrontmatterBlock {
        full: location.full,
        body: location.body,
        indent: location.indent,
        dedented_body,
        stripped: location.stripped,
    }))
}

pub fn find_physical_line_ending_controlled(
    input: &str,
    start: usize,
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<Option<(usize, usize)>> {
    let Some(offset) = find_line_break_controlled(input, start, checkpoints)? else {
        return Ok(None);
    };
    let bytes = input.as_bytes();
    let has_crlf = bytes[offset] == b'\r' && bytes.get(offset + 1) == Some(&b'\n');
    if has_crlf {
        checkpoints.scanned(1)?;
    }
    Ok(Some((offset, offset + 1 + usize::from(has_crlf))))
}

fn frontmatter_indent_end_controlled(
    line: &str,
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<usize> {
    let mut end = 0usize;
    for (idx, ch) in line.char_indices() {
        checkpoints.scanned(ch.len_utf8())?;
        if ch == '\n' || ch == '\r' || !is_frontmatter_whitespace(ch) {
            break;
        }
        end = idx + ch.len_utf8();
    }
    Ok(end)
}

fn is_frontmatter_whitespace(ch: char) -> bool {
    (ch.is_whitespace() && ch != '\u{0085}') || ch == '\u{feff}'
}

fn frontmatter_closing_end_controlled(
    input: &str,
    preceding_line_break: usize,
    indent: &str,
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<Option<usize>> {
    let fence_start = preceding_line_break + 1;
    if !frontmatter_has_prefix_controlled(&input[fence_start..], indent, checkpoints)? {
        return Ok(None);
    }
    let fence_end = fence_start + indent.len();
    if !input[fence_end..].starts_with("---") {
        return Ok(None);
    }
    let whitespace_start = fence_end + 3;
    let mut end = None;
    for (offset, ch) in input[whitespace_start..].char_indices() {
        checkpoints.scanned(ch.len_utf8())?;
        if !is_frontmatter_whitespace(ch) {
            break;
        }
        if matches!(ch, '\n' | '\r') {
            end = Some(whitespace_start + offset + 1);
        }
    }
    Ok(end)
}

pub fn frontmatter_has_prefix_controlled(
    text: &str,
    prefix: &str,
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<bool> {
    if text.len() < prefix.len() {
        return Ok(false);
    }
    for (actual, expected) in text.as_bytes()[..prefix.len()]
        .iter()
        .zip(prefix.as_bytes())
    {
        checkpoints.scanned(1)?;
        if actual != expected {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn dedent_frontmatter_body_controlled<'a>(
    body: &'a str,
    indent: &str,
    control: &OperationControl,
) -> OperationControlResult<Cow<'a, str>> {
    if indent.is_empty() {
        control.checkpoint()?;
        return Ok(Cow::Borrowed(body));
    }

    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    let mut out = String::with_capacity(body.len());
    let mut line_start = 0usize;
    loop {
        let line_ending = find_physical_line_ending_controlled(body, line_start, &mut checkpoints)?;
        let (line_end, next_line_start) = line_ending.unwrap_or((body.len(), body.len()));
        let line = &body[line_start..line_end];
        let content = if frontmatter_has_prefix_controlled(line, indent, &mut checkpoints)? {
            &line[indent.len()..]
        } else {
            line
        };
        push_str_controlled(&mut out, content, &mut checkpoints)?;
        if line_end == body.len() {
            break;
        }
        push_str_controlled(&mut out, &body[line_end..next_line_start], &mut checkpoints)?;
        if next_line_start == body.len() {
            break;
        }
        line_start = next_line_start;
    }
    checkpoints.finish()?;
    Ok(Cow::Owned(out))
}

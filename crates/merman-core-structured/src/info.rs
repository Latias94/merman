//! Info syntax and exact terminal spans for semantic and editor projection.

use merman_core_preprocess::scan::langium::{LangiumCommonParse, parse_langium_common};
use merman_model::{OperationControl, OperationControlResult, SourceSpan};

pub enum InfoStatement {
    Common(LangiumCommonParse),
    ShowInfo {
        #[cfg(feature = "editor-facts")]
        span: SourceSpan,
        consumed: usize,
    },
    Empty {
        next_offset: usize,
    },
    Unexpected {
        ch: char,
        skipped: usize,
        bad_offset: usize,
        #[cfg(feature = "editor-facts")]
        span: SourceSpan,
        next_offset: usize,
    },
}

/// Reads the next statement at a UTF-8 byte boundary before the end of the source.
pub fn parse_info_statement(
    code: &str,
    offset: usize,
    allow_show_info: bool,
) -> Option<InfoStatement> {
    let rest = code.get(offset..)?;
    if rest.is_empty() {
        return None;
    }
    if let Some(parsed) = parse_langium_common(code, offset) {
        return Some(InfoStatement::Common(parsed));
    }

    let (line, next_offset) = physical_line(rest, offset);
    let visible = strip_inline_comment(line);
    let trimmed = visible.trim();
    if trimmed.is_empty() {
        return Some(InfoStatement::Empty { next_offset });
    }

    let leading = visible.len() - visible.trim_start().len();
    if allow_show_info && let Some(token_len) = keyword_token_len(visible.trim_start(), "showInfo")
    {
        #[cfg(feature = "editor-facts")]
        let start = offset + leading;
        return Some(InfoStatement::ShowInfo {
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(start, start + token_len),
            consumed: leading + token_len,
        });
    }

    let bad_offset = offset + visible.find(trimmed).unwrap_or_default();
    Some(InfoStatement::Unexpected {
        ch: trimmed.chars().next().unwrap_or('?'),
        skipped: trimmed.chars().count(),
        bad_offset,
        #[cfg(feature = "editor-facts")]
        span: SourceSpan::new(bad_offset, bad_offset + trimmed.len()),
        next_offset,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoHeader {
    Empty,
    ExpectedInfo(SourceSpan),
    Body { offset: usize },
}

pub fn info_body_start_controlled(
    code: &str,
    control: &OperationControl,
) -> OperationControlResult<InfoHeader> {
    control.checkpoint()?;
    let mut offset = 0usize;
    while offset < code.len() {
        control.checkpoint()?;
        let (line, next_offset) = physical_line(&code[offset..], offset);
        let visible = strip_inline_comment(line);
        let trimmed = visible.trim_start();
        if trimmed.trim().is_empty() {
            offset = next_offset;
            continue;
        }
        let leading = visible.len() - trimmed.len();
        let header_start = offset + leading;
        let Some(header_len) = keyword_token_len(trimmed, "info") else {
            let token_len = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
            return Ok(InfoHeader::ExpectedInfo(SourceSpan::new(
                header_start,
                header_start + token_len,
            )));
        };
        return Ok(InfoHeader::Body {
            offset: header_start + header_len,
        });
    }
    Ok(InfoHeader::Empty)
}

fn keyword_token_len(input: &str, keyword: &str) -> Option<usize> {
    let rest = input.strip_prefix(keyword)?;
    (rest.is_empty()
        || rest.starts_with("%%")
        || rest.chars().next().is_some_and(char::is_whitespace))
    .then_some(keyword.len())
}

fn physical_line(rest: &str, offset: usize) -> (&str, usize) {
    if let Some(newline) = rest.find('\n') {
        let line = rest[..newline]
            .strip_suffix('\r')
            .unwrap_or(&rest[..newline]);
        (line, offset + newline + 1)
    } else {
        (rest, offset + rest.len())
    }
}

fn strip_inline_comment(line: &str) -> &str {
    match line.find("%%") {
        Some(idx) => &line[..idx],
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_reject_offsets_outside_utf8_boundaries() {
        assert!(parse_info_statement("info\né", 6, false).is_none());
        assert!(parse_info_statement("info", usize::MAX, false).is_none());
    }

    #[test]
    fn statements_distinguish_empty_lines_from_end_of_input() {
        assert!(parse_info_statement("", 0, false).is_none());
        assert!(parse_info_statement("info", 4, false).is_none());
        assert!(matches!(
            parse_info_statement("info\n\n", 5, false),
            Some(InfoStatement::Empty { next_offset: 6 })
        ));
    }

    #[test]
    fn empty_header_observes_cancellation() {
        let control = OperationControl::new();
        control.cancel();
        assert!(info_body_start_controlled("", &control).is_err());
    }
}

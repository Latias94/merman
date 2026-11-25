//! Physical-line syntax for Ishikawa headers, effects, and causes.

use merman_model::{OperationControl, OperationControlResult, SourceSpan};

#[derive(Debug)]
pub struct IshikawaSyntaxNode {
    pub raw_level: usize,
    pub text: String,
    #[cfg(feature = "editor-facts")]
    pub span: SourceSpan,
    #[cfg(feature = "editor-facts")]
    pub selection: SourceSpan,
}

#[derive(Debug)]
pub enum IshikawaSyntaxError {
    MissingHeader,
    InvalidHeader(SourceSpan),
    InvalidNode(SourceSpan),
}

#[derive(Debug)]
pub struct IshikawaSyntax {
    pub nodes: Vec<IshikawaSyntaxNode>,
    pub first_error: Option<IshikawaSyntaxError>,
}

/// Scans physical lines, retaining the first error and any later nodes for recovery.
///
/// Spans are UTF-8 byte offsets in `code`. The scan checkpoints before each physical
/// line, including trivia. The caller brackets the scan and any editor projection with
/// the operation's entry and successful-completion checkpoints.
pub fn scan_ishikawa_lines_controlled(
    code: &str,
    control: &OperationControl,
) -> OperationControlResult<IshikawaSyntax> {
    let mut nodes = Vec::new();
    let mut offset = 0usize;
    let mut header_seen = false;
    let mut first_error = None;

    for segment in code.split_inclusive('\n') {
        control.checkpoint()?;
        let line_start = offset;
        offset += segment.len();
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let line = line.strip_suffix('\r').unwrap_or(line);
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("%%") {
            continue;
        }

        if !header_seen {
            match parse_header_line(line, line_start) {
                Ok(root) => {
                    header_seen = true;
                    if let Some(root) = root {
                        nodes.push(root);
                    }
                }
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
            continue;
        }

        match parse_node_line(line, line_start) {
            Ok(node) => nodes.push(node),
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }

    if !header_seen {
        first_error.get_or_insert(IshikawaSyntaxError::MissingHeader);
    }

    Ok(IshikawaSyntax { nodes, first_error })
}

fn parse_header_line(
    line: &str,
    line_start: usize,
) -> Result<Option<IshikawaSyntaxNode>, IshikawaSyntaxError> {
    let trimmed_start = line.len().saturating_sub(line.trim_start().len());
    let trimmed = &line[trimmed_start..];
    for header in ["ishikawa-beta", "ishikawa"] {
        if !trimmed
            .get(..header.len())
            .is_some_and(|actual| actual.eq_ignore_ascii_case(header))
        {
            continue;
        }
        let rest = &trimmed[header.len()..];
        if rest
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            continue;
        }
        let text = rest.trim();
        if text.is_empty() {
            return Ok(None);
        }
        #[cfg(feature = "editor-facts")]
        let rel = rest.len().saturating_sub(rest.trim_start().len());
        #[cfg(feature = "editor-facts")]
        let start = line_start + trimmed_start + header.len() + rel;
        #[cfg(feature = "editor-facts")]
        let end = start + text.len();
        return Ok(Some(IshikawaSyntaxNode {
            raw_level: 0,
            text: text.to_string(),
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(line_start, line_start + line.len()),
            #[cfg(feature = "editor-facts")]
            selection: SourceSpan::new(start, end),
        }));
    }

    Err(IshikawaSyntaxError::InvalidHeader(SourceSpan::new(
        line_start,
        line_start + line.len(),
    )))
}

fn parse_node_line(
    line: &str,
    line_start: usize,
) -> Result<IshikawaSyntaxNode, IshikawaSyntaxError> {
    let indent = line
        .chars()
        .take_while(|ch| matches!(ch, ' ' | '\t'))
        .count();
    let body = &line[indent..];
    let text = body.trim();
    if text.is_empty() {
        return Err(IshikawaSyntaxError::InvalidNode(SourceSpan::new(
            line_start,
            line_start + line.len(),
        )));
    }

    #[cfg(feature = "editor-facts")]
    let rel = body.len().saturating_sub(body.trim_start().len());
    #[cfg(feature = "editor-facts")]
    let start = line_start + indent + rel;
    #[cfg(feature = "editor-facts")]
    let end = start + text.len();
    Ok(IshikawaSyntaxNode {
        raw_level: indent,
        text: text.to_string(),
        #[cfg(feature = "editor-facts")]
        span: SourceSpan::new(line_start, line_start + line.len()),
        #[cfg(feature = "editor-facts")]
        selection: SourceSpan::new(start, end),
    })
}

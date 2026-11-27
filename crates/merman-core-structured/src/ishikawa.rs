//! Ishikawa physical-line syntax and flat cause-tree construction.

use merman_model::families::ishikawa::{IshikawaDiagramModel, IshikawaNode};
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

/// Builds the flat cause tree from accepted nodes in source order.
///
/// The caller supplies labels after applying its configuration-dependent sanitization.
pub fn build_ishikawa_model_controlled(
    nodes: Vec<IshikawaSyntaxNode>,
    control: &OperationControl,
) -> OperationControlResult<IshikawaDiagramModel> {
    let mut iter = nodes.into_iter();
    let Some(root) = iter.next() else {
        return Ok(IshikawaDiagramModel::default());
    };
    let mut arena = vec![IshikawaNode {
        text: root.text,
        children: Vec::new(),
    }];
    let mut stack = vec![(0usize, 0usize)];
    let mut base_level = None;
    for flat in iter {
        control.checkpoint()?;
        let base = *base_level.get_or_insert(flat.raw_level);
        let level = flat.raw_level.saturating_sub(base) + 1;
        while stack.len() > 1
            && stack
                .last()
                .is_some_and(|(_, top_level)| *top_level >= level)
        {
            control.checkpoint()?;
            stack.pop();
        }
        let parent = stack.last().map(|(id, _)| *id).unwrap_or(0);
        let id = arena.len();
        arena.push(IshikawaNode {
            text: flat.text,
            children: Vec::new(),
        });
        arena[parent].children.push(id);
        stack.push((id, level));
    }
    control.checkpoint()?;
    Ok(IshikawaDiagramModel {
        title: Some(arena[0].text.clone()),
        root: Some(0),
        nodes: arena,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_model::OperationPhase;

    #[test]
    fn ishikawa_builder_preserves_dedent_checkpoints_and_child_order() {
        let source = concat!(
            "ishikawa Problem\n",
            "  A\n",
            "    A detail\n",
            "      A subdetail\n",
            "  B\n",
        );
        let build = |allowed_checkpoints| {
            let control = OperationControl::new().for_phase(OperationPhase::Semantic);
            let syntax = scan_ishikawa_lines_controlled(source, &control).unwrap();
            assert!(syntax.first_error.is_none());
            control.cancel_after_checkpoints(allowed_checkpoints);
            build_ishikawa_model_controlled(syntax.nodes, &control)
        };

        // Four causes, three stack pops, and the final completion checkpoint.
        assert_eq!(build(7).unwrap_err().phase, OperationPhase::Semantic);
        let model = build(8).unwrap();
        assert_eq!(model.root, Some(0));
        assert_eq!(model.title.as_deref(), Some("Problem"));
        let labels: Vec<_> = model.nodes.iter().map(|node| node.text.as_str()).collect();
        assert_eq!(labels, ["Problem", "A", "A detail", "A subdetail", "B"]);
        assert_eq!(model.nodes[0].children, [1, 4]);
        assert_eq!(model.nodes[1].children, [2]);
        assert_eq!(model.nodes[2].children, [3]);
    }
}

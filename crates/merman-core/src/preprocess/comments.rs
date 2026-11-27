use super::PreprocessedSource;
#[cfg(feature = "editor-facts")]
use super::editor::editor_evidence_line_end;
#[cfg(feature = "editor-facts")]
use crate::{EditorExpectedSyntaxKind, SourceSpan};
use crate::{OperationControl, OperationControlResult};
use merman_core_preprocess::text::mermaid_comment_edits_controlled;
#[cfg(feature = "editor-facts")]
use std::ops::Range;

pub(super) fn remove_mermaid_comments_controlled(
    source: &mut PreprocessedSource,
    #[cfg(feature = "editor-facts")] capture_editor_evidence: bool,
    control: &OperationControl,
) -> OperationControlResult<()> {
    #[cfg(feature = "editor-facts")]
    let mut comments = Vec::new();
    #[cfg(feature = "editor-facts")]
    let edits = {
        let text = source.text();
        let mut observe = |line: Range<usize>| {
            comments.push(SourceSpan::new(
                line.start,
                editor_evidence_line_end(text, line.end),
            ));
        };
        mermaid_comment_edits_controlled(
            text,
            capture_editor_evidence.then_some(&mut observe as &mut dyn FnMut(Range<usize>)),
            control,
        )?
    };
    #[cfg(not(feature = "editor-facts"))]
    let edits = mermaid_comment_edits_controlled(source.text(), None, control)?;
    #[cfg(feature = "editor-facts")]
    for (index, expected_span) in comments.into_iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        source.record_global_expected_syntax(EditorExpectedSyntaxKind::Directive, expected_span);
    }
    source.apply_edits(edits, control)
}

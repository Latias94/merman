use super::PreprocessedSource;
#[cfg(feature = "editor-facts")]
use crate::source_span::line_content_end;
#[cfg(feature = "editor-facts")]
use crate::{EditorExpectedSyntaxKind, SourceSpan};
use crate::{OperationControl, OperationControlResult};
use merman_core_preprocess::SourceEdit;
use merman_core_preprocess::scan::controlled::{
    ControlledScanCheckpoints, find_newline_controlled,
};
use merman_core_preprocess::text::{
    PreprocessMarkers, normalize_crlf_controlled,
    normalize_html_tag_attributes_like_upstream_controlled,
};
use merman_model::text::is_ecmascript_whitespace;

pub(super) fn cleanup_text_controlled(
    source: &mut PreprocessedSource,
    markers: PreprocessMarkers,
    control: &OperationControl,
) -> OperationControlResult<()> {
    control.checkpoint()?;
    strip_leading_utf8_bom(source, control)?;
    if markers.contains(PreprocessMarkers::CARRIAGE_RETURN) {
        normalize_crlf_controlled(source.mapped_mut(), control)?;
    }

    // Mermaid performs this HTML attribute rewrite as part of preprocessing.
    if markers.contains(PreprocessMarkers::TAG_OPEN) {
        normalize_html_tag_attributes_like_upstream_controlled(source.mapped_mut(), control)?;
    }
    control.checkpoint()
}

fn strip_leading_utf8_bom(
    source: &mut PreprocessedSource,
    control: &OperationControl,
) -> OperationControlResult<()> {
    if source.text().starts_with('\u{feff}') {
        source.set_first_line_column_offset(1);
        source.apply_edits(vec![SourceEdit::delete(0..'\u{feff}'.len_utf8())], control)?;
    }
    Ok(())
}

pub(super) fn remove_mermaid_comments_controlled(
    source: &mut PreprocessedSource,
    #[cfg(feature = "editor-facts")] capture_editor_evidence: bool,
    control: &OperationControl,
) -> OperationControlResult<()> {
    let text = source.text();
    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    let mut edits = Vec::new();
    #[cfg(feature = "editor-facts")]
    let mut comments = Vec::new();
    #[cfg(feature = "editor-facts")]
    let mut line_start = 0usize;
    let mut whitespace_start = 0usize;
    let mut cursor = 0usize;
    let mut at_line_start = true;
    while cursor < text.len() {
        let ch = text[cursor..]
            .chars()
            .next()
            .expect("comment scan position must contain a character");
        checkpoints.scanned(ch.len_utf8())?;
        if at_line_start && is_ecmascript_whitespace(ch) {
            cursor += ch.len_utf8();
            #[cfg(feature = "editor-facts")]
            if matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
                line_start = cursor;
            }
            continue;
        }
        if at_line_start && let Some(after_marker) = text[cursor..].strip_prefix("%%") {
            let has_comment_body = after_marker.chars().next().is_some_and(|ch| ch != '\n');
            if !after_marker.starts_with('{') && has_comment_body {
                let newline = find_newline_controlled(text, cursor + 2, &mut checkpoints)?;
                let line_end = newline.map_or(text.len(), |newline| newline + 1);
                #[cfg(feature = "editor-facts")]
                if capture_editor_evidence {
                    comments.push(SourceSpan::new(
                        line_start,
                        editor_evidence_line_end(text, line_end),
                    ));
                }
                edits.push(SourceEdit::delete(whitespace_start..line_end));
                cursor = line_end;
                #[cfg(feature = "editor-facts")]
                {
                    line_start = cursor;
                }
                whitespace_start = cursor;
                continue;
            }
        }
        at_line_start = false;
        cursor += ch.len_utf8();
        if matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}') {
            #[cfg(feature = "editor-facts")]
            {
                line_start = cursor;
            }
            whitespace_start = cursor;
            at_line_start = true;
        }
    }
    checkpoints.finish()?;
    #[cfg(feature = "editor-facts")]
    for (index, expected_span) in comments.into_iter().enumerate() {
        if index.is_multiple_of(128) {
            control.checkpoint()?;
        }
        source.record_global_expected_syntax(EditorExpectedSyntaxKind::Directive, expected_span);
    }
    source.apply_edits(edits, control)
}

pub(super) fn trim_source_start_controlled(
    source: &mut PreprocessedSource,
    control: &OperationControl,
) -> OperationControlResult<()> {
    source.set_first_line_column_offset(0);
    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    let mut leading_whitespace = 0usize;
    for ch in source.text().chars() {
        checkpoints.scanned(ch.len_utf8())?;
        if !is_ecmascript_whitespace(ch) {
            break;
        }
        leading_whitespace += ch.len_utf8();
    }
    checkpoints.finish()?;
    if leading_whitespace > 0 {
        source.apply_edits(vec![SourceEdit::delete(0..leading_whitespace)], control)?;
    }
    control.checkpoint()
}

#[cfg(feature = "editor-facts")]
pub(super) fn editor_evidence_line_end(source: &str, end: usize) -> usize {
    let end = end.min(source.len());
    let newline_start = end
        .checked_sub(1)
        .filter(|index| source.as_bytes().get(*index) == Some(&b'\n'))
        .unwrap_or(end);
    line_content_end(source, newline_start)
}

#[cfg(test)]
pub(super) fn normalize_crlf(source: &mut PreprocessedSource) {
    normalize_crlf_controlled(source.mapped_mut(), &OperationControl::new())
        .expect("a private parse control cannot be cancelled");
}

#[cfg(test)]
pub(super) fn normalize_html_tag_attributes_like_upstream(source: &mut PreprocessedSource) {
    normalize_html_tag_attributes_like_upstream_controlled(
        source.mapped_mut(),
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled");
}

pub(super) fn encode_mermaid_entities_like_upstream_controlled(
    source: &mut PreprocessedSource,
    control: &OperationControl,
) -> OperationControlResult<()> {
    merman_core_preprocess::text::encode_mermaid_entities_like_upstream_controlled(
        source.mapped_mut(),
        control,
    )
}

#[cfg(test)]
pub(super) fn encode_mermaid_entities_like_upstream(source: &mut PreprocessedSource) {
    encode_mermaid_entities_like_upstream_controlled(source, &OperationControl::new())
        .expect("a private parse control cannot be cancelled");
}

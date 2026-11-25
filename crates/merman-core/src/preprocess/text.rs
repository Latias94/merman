use super::PreprocessedSource;
use super::scan::{
    CONTROLLED_SCAN_CHECKPOINT_BYTES, ControlledScanCheckpoints, find_ascii_pattern_controlled,
    find_ascii_pattern_in_range_controlled, find_newline_controlled, is_js_regex_whitespace,
};
#[cfg(feature = "editor-facts")]
use crate::source_span::line_content_end;
#[cfg(feature = "editor-facts")]
use crate::{EditorExpectedSyntaxKind, SourceSpan};
use crate::{OperationControl, OperationControlResult};
use merman_core_preprocess::{ReplacementMapping, SourceEdit};

#[derive(Clone, Copy)]
pub(super) struct PreprocessMarkers(u8);

impl PreprocessMarkers {
    const CARRIAGE_RETURN: u8 = 1 << 0;
    const TAG_OPEN: u8 = 1 << 1;
    pub(super) const PERCENT: u8 = 1 << 2;
    const HASH: u8 = 1 << 3;
    const SEMICOLON: u8 = 1 << 4;
    pub(super) const ENTITY: u8 = Self::HASH | Self::SEMICOLON;
    const ALL: u8 = Self::CARRIAGE_RETURN | Self::TAG_OPEN | Self::PERCENT | Self::ENTITY;
    const MASKS: [u8; 256] = {
        let mut masks = [0; 256];
        masks[b'\r' as usize] = Self::CARRIAGE_RETURN;
        masks[b'<' as usize] = Self::TAG_OPEN;
        masks[b'%' as usize] = Self::PERCENT;
        masks[b'#' as usize] = Self::HASH;
        masks[b';' as usize] = Self::SEMICOLON;
        masks
    };

    pub(super) fn scan(input: &str, control: &OperationControl) -> OperationControlResult<Self> {
        control.checkpoint()?;
        let mut found = 0;
        for chunk in input.as_bytes().chunks(CONTROLLED_SCAN_CHECKPOINT_BYTES) {
            for &byte in chunk {
                found |= Self::MASKS[usize::from(byte)];
            }
            control.checkpoint()?;
            if found == Self::ALL {
                break;
            }
        }
        Ok(Self(found))
    }

    pub(super) fn contains(self, markers: u8) -> bool {
        self.0 & markers == markers
    }
}

pub(super) fn cleanup_text_controlled(
    source: &mut PreprocessedSource,
    markers: PreprocessMarkers,
    control: &OperationControl,
) -> OperationControlResult<()> {
    control.checkpoint()?;
    strip_leading_utf8_bom(source, control)?;
    if markers.contains(PreprocessMarkers::CARRIAGE_RETURN) {
        normalize_crlf_controlled(source, control)?;
    }

    // Mermaid performs this HTML attribute rewrite as part of preprocessing.
    if markers.contains(PreprocessMarkers::TAG_OPEN) {
        normalize_html_tag_attributes_like_upstream_controlled(source, control)?;
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
        if at_line_start && is_js_regex_whitespace(ch) {
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
        if !is_js_regex_whitespace(ch) {
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

fn normalize_crlf_controlled(
    source: &mut PreprocessedSource,
    control: &OperationControl,
) -> OperationControlResult<()> {
    let bytes = source.text().as_bytes();
    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    let mut edits = Vec::new();
    let mut cursor = 0usize;
    while cursor < bytes.len() {
        checkpoints.scanned(1)?;
        if bytes[cursor] == b'\r' {
            let end = cursor + usize::from(bytes.get(cursor + 1) == Some(&b'\n')) + 1;
            edits.push(SourceEdit::replace(
                cursor..end,
                "\n",
                if end - cursor == 1 {
                    ReplacementMapping::ExactBytes
                } else {
                    ReplacementMapping::Boundaries
                },
            ));
            cursor = end;
        } else {
            cursor += 1;
        }
    }
    checkpoints.finish()?;
    source.apply_edits(edits, control)?;
    control.checkpoint()
}

#[cfg(test)]
pub(super) fn normalize_crlf(source: &mut PreprocessedSource) {
    normalize_crlf_controlled(source, &OperationControl::new())
        .expect("a private parse control cannot be cancelled");
}

fn normalize_html_tag_attributes_like_upstream_controlled(
    source: &mut PreprocessedSource,
    control: &OperationControl,
) -> OperationControlResult<()> {
    let text = source.text();
    let bytes = text.as_bytes();
    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    let mut probe = 0usize;
    let mut edits = Vec::new();

    while let Some(start) = find_ascii_pattern_controlled(text, probe, b"<", &mut checkpoints)? {
        let tag_start = start + 1;
        if tag_start >= bytes.len() || !is_mermaid_js_word_byte(bytes[tag_start]) {
            probe = tag_start;
            continue;
        }

        let mut tag_end = tag_start + 1;
        while tag_end < bytes.len() && is_mermaid_js_word_byte(bytes[tag_end]) {
            checkpoints.scanned(1)?;
            tag_end += 1;
        }

        let Some(end) = find_ascii_pattern_controlled(text, tag_end, b">", &mut checkpoints)?
        else {
            // No later `<` can form a closed tag when the remaining suffix has no `>`.
            // Advancing by one here made malformed input rescan the same suffix O(n^2).
            break;
        };

        html_attribute_quote_edits_controlled(text, tag_end, end, &mut edits, &mut checkpoints)?;

        probe = end + 1;
    }
    checkpoints.finish()?;
    source.apply_edits(edits, control)?;
    control.checkpoint()
}

#[cfg(test)]
pub(super) fn normalize_html_tag_attributes_like_upstream(source: &mut PreprocessedSource) {
    normalize_html_tag_attributes_like_upstream_controlled(source, &OperationControl::new())
        .expect("a private parse control cannot be cancelled");
}

fn html_attribute_quote_edits_controlled(
    text: &str,
    attributes_start: usize,
    attributes_end: usize,
    edits: &mut Vec<SourceEdit>,
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<()> {
    let mut probe = attributes_start;
    while let Some(start) =
        find_ascii_pattern_in_range_controlled(text, probe, attributes_end, b"=\"", checkpoints)?
    {
        let value_start = start + 2;
        let Some(end) = find_ascii_pattern_in_range_controlled(
            text,
            value_start,
            attributes_end,
            b"\"",
            checkpoints,
        )?
        else {
            probe = value_start;
            continue;
        };

        let opening_quote = start + 1;
        let closing_quote = end;
        edits.push(SourceEdit::replace(
            opening_quote..opening_quote + 1,
            "'",
            ReplacementMapping::ExactBytes,
        ));
        edits.push(SourceEdit::replace(
            closing_quote..closing_quote + 1,
            "'",
            ReplacementMapping::ExactBytes,
        ));

        probe = end + 1;
    }
    Ok(())
}

pub(super) fn encode_mermaid_entities_like_upstream_controlled(
    source: &mut PreprocessedSource,
    control: &OperationControl,
) -> OperationControlResult<()> {
    // Mirrors Mermaid `encodeEntities` (Mermaid@11.12.2):
    //
    // 1) Protect `style...:#...;` and `classDef...:#...;` so color hex fragments are not mistaken
    //    as entities by the `/#\\w+;/g` pass.
    // 2) Encode `#<name>;` and `#<number>;` sequences into placeholders that do not contain `#`/`;`.
    strip_hex_style_semicolons_like_upstream_controlled(source, "style", control)?;
    strip_hex_style_semicolons_like_upstream_controlled(source, "classDef", control)?;
    encode_entity_placeholders_like_upstream_controlled(source, control)?;
    control.checkpoint()
}

#[cfg(test)]
pub(super) fn encode_mermaid_entities_like_upstream(source: &mut PreprocessedSource) {
    encode_mermaid_entities_like_upstream_controlled(source, &OperationControl::new())
        .expect("a private parse control cannot be cancelled");
}

fn encode_entity_placeholders_like_upstream_controlled(
    source: &mut PreprocessedSource,
    control: &OperationControl,
) -> OperationControlResult<()> {
    let text = source.text();
    let bytes = text.as_bytes();
    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    let mut cursor = 0usize;
    let mut edits = Vec::new();

    while let Some(start) = find_ascii_pattern_controlled(text, cursor, b"#", &mut checkpoints)? {
        let mut end = start + 1;
        while end < bytes.len() && is_mermaid_js_word_byte(bytes[end]) {
            checkpoints.scanned(1)?;
            end += 1;
        }

        if end > start + 1 && bytes.get(end) == Some(&b';') {
            let inner = &text[start + 1..end];
            let mut all_digits = true;
            for byte in inner.bytes() {
                checkpoints.scanned(1)?;
                if !byte.is_ascii_digit() {
                    all_digits = false;
                    break;
                }
            }
            let prefix = if all_digits { "ﬂ°°" } else { "ﬂ°" };
            edits.push(SourceEdit::replace(
                start..start + 1,
                prefix,
                ReplacementMapping::Boundaries,
            ));
            edits.push(SourceEdit::replace(
                end..end + 1,
                "¶ß",
                ReplacementMapping::Boundaries,
            ));
            cursor = end + 1;
        } else {
            cursor = start + 1;
        }
    }
    checkpoints.finish()?;
    source.apply_edits(edits, control)?;
    control.checkpoint()
}

fn is_mermaid_js_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn strip_hex_style_semicolons_like_upstream_controlled(
    source: &mut PreprocessedSource,
    keyword: &str,
    control: &OperationControl,
) -> OperationControlResult<()> {
    let text = source.text();
    let mut checkpoints = ControlledScanCheckpoints::new(control)?;
    let mut edits = Vec::new();
    let mut line_start = 0usize;

    loop {
        let newline = find_newline_controlled(text, line_start, &mut checkpoints)?;
        let line_end = newline.unwrap_or(text.len());
        collect_hex_style_semicolon_edits_controlled(
            text,
            line_start,
            line_end,
            keyword,
            &mut edits,
            &mut checkpoints,
        )?;
        let Some(newline) = newline else {
            break;
        };
        line_start = newline + 1;
    }

    checkpoints.finish()?;
    source.apply_edits(edits, control)?;
    control.checkpoint()
}

fn collect_hex_style_semicolon_edits_controlled(
    text: &str,
    line_start: usize,
    line_end: usize,
    keyword: &str,
    edits: &mut Vec<SourceEdit>,
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<()> {
    let mut cursor = line_start;
    while let Some(semicolon) =
        find_hex_style_match_controlled(text, line_start, line_end, keyword, cursor, checkpoints)?
    {
        edits.push(SourceEdit::delete(semicolon..semicolon + 1));
        cursor = semicolon + 1;
    }
    Ok(())
}

fn find_hex_style_match_controlled(
    text: &str,
    line_start: usize,
    line_end: usize,
    keyword: &str,
    search_start: usize,
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<Option<usize>> {
    let Some(start) = find_ascii_pattern_in_range_controlled(
        text,
        search_start.max(line_start),
        line_end,
        keyword.as_bytes(),
        checkpoints,
    )?
    else {
        return Ok(None);
    };
    find_hex_style_match_end_controlled(text, start + keyword.len(), line_end, checkpoints)
}

fn find_hex_style_match_end_controlled(
    text: &str,
    search_start: usize,
    line_end: usize,
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<Option<usize>> {
    let mut probe = search_start;
    while let Some(colon) =
        find_ascii_pattern_in_range_controlled(text, probe, line_end, b":", checkpoints)?
    {
        let mut hash = None;
        let mut token_end = colon + 1;
        for (rel, ch) in text[colon + 1..line_end].char_indices() {
            checkpoints.scanned(ch.len_utf8())?;
            if ch.is_whitespace() {
                token_end = colon + 1 + rel;
                break;
            }
            token_end = colon + 1 + rel + ch.len_utf8();
            if ch == '#' {
                hash = Some(colon + 1 + rel);
                break;
            }
        }

        if let Some(hash) = hash {
            return rfind_ascii_byte_in_range_controlled(
                text,
                hash + 1,
                line_end,
                b';',
                checkpoints,
            );
        }

        // Skip the complete value token. Re-probing at every byte after a colon makes a
        // long colon-heavy line quadratic when no hexadecimal color is present.
        probe = token_end.max(colon + 1);
    }
    Ok(None)
}

fn rfind_ascii_byte_in_range_controlled(
    text: &str,
    start: usize,
    end: usize,
    needle: u8,
    checkpoints: &mut ControlledScanCheckpoints<'_>,
) -> OperationControlResult<Option<usize>> {
    for offset in (start..end.min(text.len())).rev() {
        checkpoints.scanned(1)?;
        if text.as_bytes()[offset] == needle {
            return Ok(Some(offset));
        }
    }
    Ok(None)
}

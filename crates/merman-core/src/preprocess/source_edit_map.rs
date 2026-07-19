use crate::{EditorLexeme, EditorLexemeKind, SourceSpan};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ReplacementMapping {
    ExactBytes,
    Boundaries,
}

#[derive(Debug, Clone)]
pub(super) struct SourceEdit {
    range: Range<usize>,
    replacement: String,
    mapping: ReplacementMapping,
}

impl SourceEdit {
    pub(super) fn delete(range: Range<usize>) -> Self {
        Self {
            range,
            replacement: String::new(),
            mapping: ReplacementMapping::Boundaries,
        }
    }

    pub(super) fn replace(
        range: Range<usize>,
        replacement: impl Into<String>,
        mapping: ReplacementMapping,
    ) -> Self {
        Self {
            range,
            replacement: replacement.into(),
            mapping,
        }
    }
}

/// Parser text together with an exact, composable map back to the caller's source.
#[derive(Debug, Clone)]
pub struct PreprocessedSource {
    text: String,
    edit_map: SourceEditMap,
    global_lexemes: Vec<EditorLexeme>,
}

impl PreprocessedSource {
    pub fn new(source: &str) -> Self {
        Self {
            text: source.to_string(),
            edit_map: SourceEditMap::identity(source.len()),
            global_lexemes: Vec::new(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn try_map_span(&self, span: SourceSpan) -> Option<SourceSpan> {
        if !self.text.is_char_boundary(span.start) || !self.text.is_char_boundary(span.end) {
            return None;
        }
        self.edit_map.try_map_span(span)
    }

    pub(crate) fn global_lexemes(&self) -> &[EditorLexeme] {
        &self.global_lexemes
    }

    pub(super) fn record_global_lexeme(&mut self, kind: EditorLexemeKind, span: SourceSpan) {
        let Some(span) = self.try_map_span(span) else {
            return;
        };
        if span.start < span.end {
            self.global_lexemes.push(EditorLexeme::global(kind, span));
        }
    }

    pub fn into_text(self) -> String {
        self.text
    }

    pub(super) fn apply_edits(&mut self, mut edits: Vec<SourceEdit>) {
        if edits.is_empty() {
            return;
        }
        edits.sort_by_key(|edit| (edit.range.start, edit.range.end));
        assert_valid_edits(&self.text, &edits);

        let old_text = std::mem::take(&mut self.text);
        let old_map = std::mem::replace(&mut self.edit_map, SourceEditMap::identity(0));
        let replacement_bytes = edits
            .iter()
            .map(|edit| edit.replacement.len())
            .sum::<usize>();
        let removed_bytes = edits
            .iter()
            .map(|edit| edit.range.end - edit.range.start)
            .sum::<usize>();
        let mut text = String::with_capacity(
            old_text
                .len()
                .saturating_sub(removed_bytes)
                .saturating_add(replacement_bytes),
        );
        let mut builder = EditMapBuilder::new(old_map.original_len);
        let mut cursor = 0usize;

        for edit in edits {
            text.push_str(&old_text[cursor..edit.range.start]);
            builder.copy_from(&old_map, cursor..edit.range.start);

            if edit.replacement.is_empty() {
                builder.delete_from(&old_map, edit.range.clone());
            } else {
                text.push_str(&edit.replacement);
                builder.replace_from(
                    &old_map,
                    edit.range.clone(),
                    edit.replacement.len(),
                    edit.mapping,
                );
            }
            cursor = edit.range.end;
        }

        text.push_str(&old_text[cursor..]);
        builder.copy_from(&old_map, cursor..old_text.len());
        let edit_map = builder.finish(text.len());
        debug_assert!(edit_map.is_well_formed());
        self.text = text;
        self.edit_map = edit_map;
    }
}

#[derive(Debug, Clone)]
struct SourceEditMap {
    original_len: usize,
    output_len: usize,
    segments: Vec<EditMapSegment>,
    gaps: Vec<EditMapGap>,
}

impl SourceEditMap {
    fn identity(source_len: usize) -> Self {
        let segments = (source_len > 0)
            .then_some(EditMapSegment {
                output: 0..source_len,
                original: 0..source_len,
                mapping: SegmentMapping::ExactBytes,
            })
            .into_iter()
            .collect();
        Self {
            original_len: source_len,
            output_len: source_len,
            segments,
            gaps: Vec::new(),
        }
    }

    /// Maps a parser-input span to the smallest exact span in the original source.
    ///
    /// A span that crosses deleted text or ends inside a length-changing replacement is not exact
    /// and returns `None`. Adjacent facts remain independently mappable.
    fn try_map_span(&self, span: SourceSpan) -> Option<SourceSpan> {
        if span.start > span.end || span.end > self.output_len {
            return None;
        }
        if self.has_unmapped_overlap(span.start, span.end)
            || (span.start < span.end && self.has_gap_inside(span.start, span.end))
        {
            return None;
        }

        let start = self.original_at_start(span.start)?;
        let end = if span.start == span.end {
            start
        } else {
            self.original_at_end(span.end)?
        };
        (start <= end).then(|| SourceSpan::new(start, end))
    }

    fn original_at_start(&self, offset: usize) -> Option<usize> {
        if offset > self.output_len {
            return None;
        }
        if let Some(gap) = self.gap_at(offset) {
            return Some(gap.original_right);
        }
        if offset == self.output_len {
            return match self.segments.last() {
                Some(segment) => segment.original_at_end(offset),
                None => Some(self.original_len),
            };
        }
        self.segment_to_right(offset)?.original_at_start(offset)
    }

    fn original_at_end(&self, offset: usize) -> Option<usize> {
        if offset > self.output_len {
            return None;
        }
        if let Some(gap) = self.gap_at(offset) {
            return Some(gap.original_left);
        }
        if offset == 0 {
            return match self.segments.first() {
                Some(segment) => segment.original_at_start(offset),
                None => Some(0),
            };
        }
        self.segment_to_left(offset)?.original_at_end(offset)
    }

    fn segment_to_right(&self, offset: usize) -> Option<&EditMapSegment> {
        let index = self
            .segments
            .partition_point(|segment| segment.output.end <= offset);
        self.segments
            .get(index)
            .filter(|segment| segment.output.start <= offset && offset < segment.output.end)
    }

    fn segment_to_left(&self, offset: usize) -> Option<&EditMapSegment> {
        let index = self
            .segments
            .partition_point(|segment| segment.output.start < offset)
            .checked_sub(1)?;
        self.segments
            .get(index)
            .filter(|segment| segment.output.start < offset && offset <= segment.output.end)
    }

    fn gap_at(&self, offset: usize) -> Option<&EditMapGap> {
        self.gaps
            .binary_search_by_key(&offset, |gap| gap.output_offset)
            .ok()
            .and_then(|index| self.gaps.get(index))
    }

    fn has_gap_inside(&self, start: usize, end: usize) -> bool {
        let first = self.gaps.partition_point(|gap| gap.output_offset <= start);
        self.gaps
            .get(first)
            .is_some_and(|gap| gap.output_offset < end)
    }

    fn has_unmapped_overlap(&self, start: usize, end: usize) -> bool {
        self.segments.iter().any(|segment| {
            segment.mapping == SegmentMapping::Unmapped
                && if start == end {
                    segment.output.start < start && start < segment.output.end
                } else {
                    segment.output.start < end && start < segment.output.end
                }
        })
    }

    fn original_anchor(&self, offset: usize) -> usize {
        if let Some(gap) = self.gap_at(offset) {
            return gap.original_right;
        }
        if offset == self.output_len {
            return self.original_len;
        }
        self.segment_to_right(offset)
            .map_or(self.original_len, |segment| segment.original.start)
    }

    fn is_exact_bytes_span(&self, range: Range<usize>) -> bool {
        if range.start > range.end || range.end > self.output_len {
            return false;
        }
        if range.start == range.end {
            return self.original_at_start(range.start).is_some();
        }
        if self.has_gap_inside(range.start, range.end) {
            return false;
        }

        let mut cursor = range.start;
        while cursor < range.end {
            let Some(segment) = self.segment_to_right(cursor) else {
                return false;
            };
            if segment.mapping != SegmentMapping::ExactBytes {
                return false;
            }
            cursor = segment.output.end.min(range.end);
        }
        self.try_map_span(SourceSpan::new(range.start, range.end))
            .is_some_and(|mapped| mapped.end - mapped.start == range.end - range.start)
    }

    fn is_well_formed(&self) -> bool {
        self.segments.windows(2).all(|pair| {
            pair[0].output.end <= pair[1].output.start
                && pair[0].original.end <= pair[1].original.start
        }) && self
            .segments
            .iter()
            .all(|segment| segment.output.start < segment.output.end)
            && self
                .gaps
                .windows(2)
                .all(|pair| pair[0].output_offset < pair[1].output_offset)
            && self
                .segments
                .last()
                .is_none_or(|segment| segment.output.end <= self.output_len)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditMapSegment {
    output: Range<usize>,
    original: Range<usize>,
    mapping: SegmentMapping,
}

impl EditMapSegment {
    fn original_at_start(&self, offset: usize) -> Option<usize> {
        match self.mapping {
            SegmentMapping::ExactBytes => self
                .original
                .start
                .checked_add(offset.checked_sub(self.output.start)?),
            SegmentMapping::Boundaries if offset == self.output.start => Some(self.original.start),
            SegmentMapping::Boundaries if offset == self.output.end => Some(self.original.end),
            SegmentMapping::Boundaries => None,
            SegmentMapping::Unmapped => None,
        }
    }

    fn original_at_end(&self, offset: usize) -> Option<usize> {
        self.original_at_start(offset)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentMapping {
    ExactBytes,
    Boundaries,
    Unmapped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EditMapGap {
    output_offset: usize,
    original_left: usize,
    original_right: usize,
}

struct EditMapBuilder {
    original_len: usize,
    output_len: usize,
    segments: Vec<EditMapSegment>,
    gaps: Vec<EditMapGap>,
}

impl EditMapBuilder {
    fn new(original_len: usize) -> Self {
        Self {
            original_len,
            output_len: 0,
            segments: Vec::new(),
            gaps: Vec::new(),
        }
    }

    fn copy_from(&mut self, old: &SourceEditMap, range: Range<usize>) {
        if range.start >= range.end {
            return;
        }
        let output_base = self.output_len;
        for segment in &old.segments {
            let start = segment.output.start.max(range.start);
            let end = segment.output.end.min(range.end);
            if start >= end {
                continue;
            }

            let (original, mapping) = match segment.mapping {
                SegmentMapping::ExactBytes => {
                    let original_start = segment.original.start + (start - segment.output.start);
                    (
                        original_start..original_start + (end - start),
                        SegmentMapping::ExactBytes,
                    )
                }
                SegmentMapping::Boundaries
                    if start == segment.output.start && end == segment.output.end =>
                {
                    (segment.original.clone(), SegmentMapping::Boundaries)
                }
                SegmentMapping::Boundaries | SegmentMapping::Unmapped => {
                    let anchor = segment.original.start;
                    (anchor..anchor, SegmentMapping::Unmapped)
                }
            };
            self.push_segment(EditMapSegment {
                output: output_base + (start - range.start)..output_base + (end - range.start),
                original,
                mapping,
            });
        }
        for gap in old
            .gaps
            .iter()
            .filter(|gap| range.start <= gap.output_offset && gap.output_offset <= range.end)
        {
            self.push_gap(EditMapGap {
                output_offset: output_base + (gap.output_offset - range.start),
                original_left: gap.original_left,
                original_right: gap.original_right,
            });
        }
        self.output_len += range.end - range.start;
    }

    fn delete_from(&mut self, old: &SourceEditMap, range: Range<usize>) {
        let left = old
            .original_at_end(range.start)
            .unwrap_or_else(|| old.original_anchor(range.start));
        let right = old
            .original_at_start(range.end)
            .unwrap_or_else(|| old.original_anchor(range.end));
        self.push_gap(EditMapGap {
            output_offset: self.output_len,
            original_left: left,
            original_right: right,
        });
    }

    fn replace_from(
        &mut self,
        old: &SourceEditMap,
        range: Range<usize>,
        replacement_len: usize,
        mapping: ReplacementMapping,
    ) {
        let original_start = old.original_at_start(range.start);
        let original_end = old.original_at_end(range.end);
        let endpoints_are_mappable = original_start.is_some() && original_end.is_some();
        let original_start = original_start.unwrap_or_else(|| old.original_anchor(range.start));
        let original_end = original_end.unwrap_or(original_start);
        let exact = mapping == ReplacementMapping::ExactBytes
            && endpoints_are_mappable
            && replacement_len == range.end - range.start
            && old.is_exact_bytes_span(range.clone())
            && original_end.checked_sub(original_start) == Some(replacement_len);
        self.push_segment(EditMapSegment {
            output: self.output_len..self.output_len + replacement_len,
            original: original_start..original_end,
            mapping: if !endpoints_are_mappable {
                SegmentMapping::Unmapped
            } else if exact {
                SegmentMapping::ExactBytes
            } else {
                SegmentMapping::Boundaries
            },
        });
        self.output_len += replacement_len;
    }

    fn push_segment(&mut self, segment: EditMapSegment) {
        if let Some(last) = self.segments.last_mut()
            && last.mapping == SegmentMapping::ExactBytes
            && segment.mapping == SegmentMapping::ExactBytes
            && last.output.end == segment.output.start
            && last.original.end == segment.original.start
        {
            last.output.end = segment.output.end;
            last.original.end = segment.original.end;
            return;
        }
        self.segments.push(segment);
    }

    fn push_gap(&mut self, gap: EditMapGap) {
        if let Some(last) = self.gaps.last_mut()
            && last.output_offset == gap.output_offset
        {
            last.original_left = last.original_left.min(gap.original_left);
            last.original_right = last.original_right.max(gap.original_right);
            return;
        }
        self.gaps.push(gap);
    }

    fn finish(mut self, output_len: usize) -> SourceEditMap {
        debug_assert_eq!(self.output_len, output_len);
        self.gaps.sort_by_key(|gap| gap.output_offset);
        let mut gaps: Vec<EditMapGap> = Vec::with_capacity(self.gaps.len());
        for gap in self.gaps {
            if let Some(last) = gaps.last_mut()
                && last.output_offset == gap.output_offset
            {
                last.original_left = last.original_left.min(gap.original_left);
                last.original_right = last.original_right.max(gap.original_right);
            } else {
                gaps.push(gap);
            }
        }
        SourceEditMap {
            original_len: self.original_len,
            output_len,
            segments: self.segments,
            gaps,
        }
    }
}

fn assert_valid_edits(source: &str, edits: &[SourceEdit]) {
    let mut cursor = 0usize;
    for edit in edits {
        assert!(edit.range.start <= edit.range.end, "reversed source edit");
        assert!(edit.range.start >= cursor, "overlapping source edits");
        assert!(edit.range.end <= source.len(), "source edit out of bounds");
        assert!(source.is_char_boundary(edit.range.start));
        assert!(source.is_char_boundary(edit.range.end));
        cursor = edit.range.end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composes_deletions_replacements_and_unicode_without_global_degradation() {
        let original = "前A\r\nREMOVE中#quot;后";
        let mut source = PreprocessedSource::new(original);
        let cr = source.text().find('\r').unwrap();
        source.apply_edits(vec![SourceEdit::replace(
            cr..cr + 2,
            "\n",
            ReplacementMapping::Boundaries,
        )]);
        let remove = source.text().find("REMOVE").unwrap();
        source.apply_edits(vec![SourceEdit::delete(remove..remove + "REMOVE".len())]);
        let hash = source.text().find('#').unwrap();
        source.apply_edits(vec![
            SourceEdit::replace(hash..hash + 1, "ﬂ°", ReplacementMapping::Boundaries),
            SourceEdit::replace(
                hash + "#quot".len()..hash + "#quot;".len(),
                "¶ß",
                ReplacementMapping::Boundaries,
            ),
        ]);

        assert_eq!(source.text(), "前A\n中ﬂ°quot¶ß后");
        let middle = source.text().find('中').unwrap();
        let mapped = source
            .try_map_span(SourceSpan::new(middle, middle + '中'.len_utf8()))
            .unwrap();
        assert_eq!(&original[mapped.start..mapped.end], "中");

        let entity = source.text().find("ﬂ°quot¶ß").unwrap();
        let mapped = source
            .try_map_span(SourceSpan::new(entity, entity + "ﬂ°quot¶ß".len()))
            .unwrap();
        assert_eq!(&original[mapped.start..mapped.end], "#quot;");

        let crossing_deletion = source.text().find('A').unwrap();
        let after_deletion = source.text().find('中').unwrap() + '中'.len_utf8();
        assert_eq!(
            source.try_map_span(SourceSpan::new(crossing_deletion, after_deletion)),
            None
        );
    }

    #[test]
    fn boundary_replacements_only_reject_locally_ambiguous_offsets() {
        let mut source = PreprocessedSource::new("A#x;B");
        source.apply_edits(vec![
            SourceEdit::replace(1..2, "ﬂ°", ReplacementMapping::Boundaries),
            SourceEdit::replace(3..4, "¶ß", ReplacementMapping::Boundaries),
        ]);

        let b = source.text().find('B').unwrap();
        assert_eq!(
            source.try_map_span(SourceSpan::new(b, b + 1)),
            Some(SourceSpan::new(4, 5))
        );
        assert_eq!(source.try_map_span(SourceSpan::new(2, 2)), None);

        let identity = PreprocessedSource::new("A😀B");
        assert_eq!(identity.try_map_span(SourceSpan::new(2, 2)), None);
    }

    #[test]
    fn composing_an_edit_inside_generated_text_marks_only_that_region_unmappable() {
        let mut source = PreprocessedSource::new("A#x;B");
        source.apply_edits(vec![SourceEdit::replace(
            1..2,
            "ﬂ°",
            ReplacementMapping::Boundaries,
        )]);
        let degree = source.text().find('°').unwrap();
        source.apply_edits(vec![SourceEdit::delete(degree..degree + '°'.len_utf8())]);

        let generated = source.text().find('ﬂ').unwrap();
        assert_eq!(
            source.try_map_span(SourceSpan::new(generated, generated + 'ﬂ'.len_utf8())),
            None
        );
        let b = source.text().find('B').unwrap();
        assert_eq!(
            source.try_map_span(SourceSpan::new(b, b + 1)),
            Some(SourceSpan::new(4, 5))
        );
    }
}

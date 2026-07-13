use crate::{
    EditorSemanticFacts, EditorSpanCoordinateSpace, Engine, Error, MermaidConfig, ParseMetadata,
    ParseOptions, Result, SourceSpan, common_db, diagram, diagrams::error_diagram, family,
    preprocess_diagram, preprocess_diagram_with_known_type, runtime, sanitize, theme,
};
use diagram::{
    DiagramWarningFact, ParsedDiagram, ParsedDiagramRender, ParsedDiagramWithEditorFacts,
    ParsedEditorFacts, RenderSemanticModel,
};

#[derive(Debug, Clone, Copy)]
pub(crate) enum ParseSource<'a> {
    Detect,
    KnownType(&'a str),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ParseTiming {
    None,
    Json,
    Render,
}

pub(crate) struct ParsePipeline<'a> {
    engine: &'a Engine,
    text: &'a str,
    options: ParseOptions,
    source: ParseSource<'a>,
}

struct EditorParseSourceMap<'a> {
    original: &'a str,
    parser_input: &'a str,
    remap: EditorSourceRemap,
}

enum EditorSourceRemap {
    None,
    Offset(usize),
    Segmented {
        parser_offset: usize,
        source_map: SourceMapSegments,
    },
    ParserInputCoordinates,
}

#[derive(Debug, Clone, Copy)]
struct SourceMapSegment {
    parser_start: usize,
    original_start: usize,
}

#[derive(Debug, Clone, Copy)]
struct SourceMapGap {
    parser_offset: usize,
    original_left: usize,
    original_right: usize,
}

struct SourceMapSegments {
    parser_len: usize,
    segments: Vec<SourceMapSegment>,
    gaps: Vec<SourceMapGap>,
}

const WARNING_FACT_REMAP_CONTEXT_EXPANSIONS: [usize; 6] = [1, 4, 8, 16, 32, 64];

fn registry_uses_baseline_semantic_parser(
    registry: &diagram::DiagramRegistry,
    diagram_type: &str,
) -> bool {
    let Some(registered) = registry.get(diagram_type) else {
        return false;
    };
    let Some(baseline) = family::semantic_parser_facts(registry.profile())
        .iter()
        .find(|fact| fact.id == diagram_type)
    else {
        return false;
    };
    std::ptr::fn_addr_eq(registered, baseline.parser)
}

impl<'a> EditorParseSourceMap<'a> {
    fn new(original: &'a str, preprocessed: &'a str) -> Self {
        if preprocessed == original {
            return Self {
                original,
                parser_input: original,
                remap: EditorSourceRemap::None,
            };
        }

        if preprocessed.is_empty() {
            return Self {
                original,
                parser_input: preprocessed,
                remap: EditorSourceRemap::Offset(original.len()),
            };
        }

        if let Some(source_map) = deletion_only_source_map(original, preprocessed) {
            return Self {
                original,
                parser_input: preprocessed,
                remap: EditorSourceRemap::Segmented {
                    parser_offset: 0,
                    source_map,
                },
            };
        }

        if let Some(offset) = original.rfind(preprocessed) {
            return Self {
                original,
                parser_input: preprocessed,
                remap: EditorSourceRemap::Offset(offset),
            };
        }

        if original.contains('\r') {
            let (normalized, source_map) = normalize_original_with_source_map(original);
            if let Some(normalized_offset) = normalized.rfind(preprocessed) {
                return Self {
                    original,
                    parser_input: preprocessed,
                    remap: EditorSourceRemap::Segmented {
                        parser_offset: normalized_offset,
                        source_map,
                    },
                };
            }
        }

        Self {
            original,
            parser_input: preprocessed,
            remap: EditorSourceRemap::ParserInputCoordinates,
        }
    }

    fn parser_input(&self) -> &'a str {
        self.parser_input
    }

    fn remap_facts(&self, facts: &mut EditorSemanticFacts) {
        match self.remap {
            EditorSourceRemap::None | EditorSourceRemap::Offset(0) => {
                facts.span_coordinate_space = EditorSpanCoordinateSpace::OriginalSource;
                return;
            }
            EditorSourceRemap::ParserInputCoordinates => {
                facts.span_coordinate_space = EditorSpanCoordinateSpace::ParserInput;
                return;
            }
            EditorSourceRemap::Offset(_) | EditorSourceRemap::Segmented { .. } => {}
        }

        if !self.facts_are_fully_remappable(facts) {
            facts.span_coordinate_space = EditorSpanCoordinateSpace::ParserInput;
            return;
        }
        facts.span_coordinate_space = EditorSpanCoordinateSpace::OriginalSource;

        for symbol in &mut facts.symbols {
            symbol.span = self
                .try_remap_source_span(symbol.span)
                .expect("fact spans were prevalidated");
            symbol.selection = self
                .try_remap_source_span(symbol.selection)
                .expect("fact spans were prevalidated");
        }
        for diagnostic in &mut facts.diagnostics {
            diagnostic.span = diagnostic.span.map(|span| {
                self.try_remap_source_span(span)
                    .expect("fact spans were prevalidated")
            });
        }
        for expected in &mut facts.expected_syntax {
            expected.span = self
                .try_remap_source_span(expected.span)
                .expect("fact spans were prevalidated");
        }
    }

    fn facts_are_fully_remappable(&self, facts: &EditorSemanticFacts) -> bool {
        facts.symbols.iter().all(|symbol| {
            self.try_remap_source_span(symbol.span).is_some()
                && self.try_remap_source_span(symbol.selection).is_some()
        }) && facts.diagnostics.iter().all(|diagnostic| {
            diagnostic
                .span
                .is_none_or(|span| self.try_remap_source_span(span).is_some())
        }) && facts
            .expected_syntax
            .iter()
            .all(|expected| self.try_remap_source_span(expected.span).is_some())
    }

    fn remap_parse_error(&self, err: Error) -> Error {
        match err {
            Error::DiagramParse {
                diagram_type,
                diagnostic,
            } => Error::DiagramParse {
                diagram_type,
                diagnostic: self.remap_parse_diagnostic(diagnostic),
            },
            err => err,
        }
    }

    fn remap_parse_diagnostic(&self, diagnostic: crate::ParseDiagnostic) -> crate::ParseDiagnostic {
        let Some(span) = diagnostic.span() else {
            return diagnostic;
        };
        match self.try_remap_source_span(span) {
            Some(remapped) => diagnostic.map_span(|_| remapped),
            None => diagnostic.without_span(),
        }
    }

    fn try_remap_source_span(&self, span: SourceSpan) -> Option<SourceSpan> {
        if let EditorSourceRemap::Segmented {
            parser_offset,
            source_map,
        } = &self.remap
        {
            if span.start > span.end || span.end > self.parser_input.len() {
                return None;
            }
            let start = parser_offset.checked_add(span.start)?;
            let end = parser_offset.checked_add(span.end)?;
            return source_map.try_remap_span(SourceSpan::new(start, end));
        }
        let start = self.try_remap_offset(span.start)?;
        let end = self.try_remap_offset(span.end)?;
        (start <= end).then(|| SourceSpan::new(start, end))
    }

    fn try_remap_warning_source_span(&self, span: SourceSpan) -> Option<SourceSpan> {
        self.try_remap_source_span(span)
            .or_else(|| self.try_remap_span_by_unique_fragment(span))
    }

    fn try_remap_span_by_unique_fragment(&self, span: SourceSpan) -> Option<SourceSpan> {
        if span.start >= span.end {
            return None;
        }
        if let Some(mapped) = self.try_remap_span_with_unique_context(span, span.start, span.end) {
            return Some(mapped);
        }

        // Some warning facts are produced after family-local masking, so the raw span text can
        // also appear in frontmatter or config. Grow bounded context until the source fragment is
        // unique, then translate only the original span within that fragment.
        for extra_after in WARNING_FACT_REMAP_CONTEXT_EXPANSIONS {
            let context_end = span
                .end
                .saturating_add(extra_after)
                .min(self.parser_input.len());
            if let Some(mapped) =
                self.try_remap_span_with_unique_context(span, span.start, context_end)
            {
                return Some(mapped);
            }
        }

        for extra_before in WARNING_FACT_REMAP_CONTEXT_EXPANSIONS {
            let context_start = span.start.saturating_sub(extra_before);
            if let Some(mapped) =
                self.try_remap_span_with_unique_context(span, context_start, span.end)
            {
                return Some(mapped);
            }
        }

        None
    }

    fn try_remap_span_with_unique_context(
        &self,
        span: SourceSpan,
        context_start: usize,
        context_end: usize,
    ) -> Option<SourceSpan> {
        let fragment = self.parser_input.get(context_start..context_end)?;
        if fragment.is_empty() {
            return None;
        }

        let mut matches = self.original.match_indices(fragment);
        let (match_start, _) = matches.next()?;
        if matches.next().is_some() {
            return None;
        }

        let remapped_start = match_start.checked_add(span.start.checked_sub(context_start)?)?;
        let remapped_end = remapped_start.checked_add(span.end.checked_sub(span.start)?)?;
        (remapped_end <= self.original.len()).then(|| SourceSpan::new(remapped_start, remapped_end))
    }

    fn try_remap_offset(&self, offset: usize) -> Option<usize> {
        if offset > self.parser_input.len() {
            return None;
        }
        match &self.remap {
            EditorSourceRemap::None => Some(offset),
            EditorSourceRemap::Offset(base) => base.checked_add(offset),
            EditorSourceRemap::ParserInputCoordinates => None,
            EditorSourceRemap::Segmented {
                parser_offset,
                source_map,
            } => source_map.original_at_right(parser_offset.checked_add(offset)?),
        }
    }
}

impl SourceMapSegments {
    fn try_remap_span(&self, span: SourceSpan) -> Option<SourceSpan> {
        if span.start > span.end || span.end > self.parser_len {
            return None;
        }
        if span.start < span.end && self.has_gap_between(span.start, span.end) {
            return None;
        }

        let start = self.original_at_right(span.start)?;
        let end = if span.start == span.end {
            start
        } else {
            self.original_at_left(span.end)?
        };
        (start <= end).then(|| SourceSpan::new(start, end))
    }

    fn original_at_right(&self, offset: usize) -> Option<usize> {
        if offset > self.parser_len {
            return None;
        }
        if let Some(gap) = self.gap_at(offset) {
            return Some(gap.original_right);
        }
        self.original_at_segment(offset)
    }

    fn original_at_left(&self, offset: usize) -> Option<usize> {
        if offset > self.parser_len {
            return None;
        }
        if let Some(gap) = self.gap_at(offset) {
            return Some(gap.original_left);
        }
        self.original_at_segment(offset)
    }

    fn original_at_segment(&self, offset: usize) -> Option<usize> {
        let index = self
            .segments
            .partition_point(|segment| segment.parser_start <= offset)
            .checked_sub(1)?;
        let segment = self.segments.get(index)?;
        segment
            .original_start
            .checked_add(offset.checked_sub(segment.parser_start)?)
    }

    fn gap_at(&self, offset: usize) -> Option<&SourceMapGap> {
        self.gaps
            .binary_search_by_key(&offset, |gap| gap.parser_offset)
            .ok()
            .and_then(|index| self.gaps.get(index))
    }

    fn has_gap_between(&self, start: usize, end: usize) -> bool {
        let first_after_start = self.gaps.partition_point(|gap| gap.parser_offset <= start);
        self.gaps
            .get(first_after_start)
            .is_some_and(|gap| gap.parser_offset < end)
    }

    fn remove_ranges(&mut self, ranges: &[(usize, usize)]) {
        if ranges.is_empty() {
            return;
        }

        let mut removed_before = 0;
        let deletions: Vec<SourceMapDeletion> = ranges
            .iter()
            .map(|&(start, end)| {
                debug_assert!(start < end);
                debug_assert!(end <= self.parser_len);
                let deletion = SourceMapDeletion {
                    start,
                    end,
                    removed_before,
                };
                removed_before += end - start;
                deletion
            })
            .collect();

        let deletion_boundaries: Vec<(usize, usize)> = deletions
            .iter()
            .map(|deletion| {
                (
                    self.original_at_left(deletion.start)
                        .expect("deletion start should be in the source map"),
                    self.original_at_right(deletion.end)
                        .expect("deletion end should be in the source map"),
                )
            })
            .collect();

        let mut segments = Vec::with_capacity(self.segments.len() + deletions.len());
        for segment in &self.segments {
            if let Some(parser_start) = remap_retained_offset(segment.parser_start, &deletions) {
                segments.push(SourceMapSegment {
                    parser_start,
                    original_start: segment.original_start,
                });
            }
        }
        for (deletion, &(_, original_right)) in deletions.iter().zip(&deletion_boundaries) {
            segments.push(SourceMapSegment {
                parser_start: deletion.start - deletion.removed_before,
                original_start: original_right,
            });
        }
        let segments = canonicalize_source_map_segments(segments);

        let mut gaps = Vec::with_capacity(self.gaps.len() + deletions.len());
        for gap in &self.gaps {
            if let Some(parser_offset) = remap_retained_offset(gap.parser_offset, &deletions) {
                gaps.push(SourceMapGap {
                    parser_offset,
                    original_left: gap.original_left,
                    original_right: gap.original_right,
                });
            }
        }
        for (deletion, &(original_left, original_right)) in
            deletions.iter().zip(&deletion_boundaries)
        {
            gaps.push(SourceMapGap {
                parser_offset: deletion.start - deletion.removed_before,
                original_left,
                original_right,
            });
        }
        self.segments = segments;
        self.gaps = canonicalize_source_map_gaps(gaps);
        self.parser_len -= removed_before;

        debug_assert_eq!(
            self.segments.first().map(|segment| segment.parser_start),
            Some(0)
        );
        debug_assert!(
            self.segments
                .windows(2)
                .all(|pair| pair[0].parser_start < pair[1].parser_start)
        );
        debug_assert!(
            self.gaps
                .windows(2)
                .all(|pair| pair[0].parser_offset < pair[1].parser_offset)
        );
    }
}

#[derive(Clone, Copy)]
struct SourceMapDeletion {
    start: usize,
    end: usize,
    removed_before: usize,
}

fn remap_retained_offset(offset: usize, deletions: &[SourceMapDeletion]) -> Option<usize> {
    let insertion = deletions.partition_point(|deletion| deletion.start <= offset);
    let Some(deletion) = insertion
        .checked_sub(1)
        .and_then(|index| deletions.get(index))
    else {
        return Some(offset);
    };
    if offset <= deletion.end {
        return None;
    }
    offset.checked_sub(deletion.removed_before + deletion.end - deletion.start)
}

fn canonicalize_source_map_segments(mut segments: Vec<SourceMapSegment>) -> Vec<SourceMapSegment> {
    segments.sort_by_key(|segment| segment.parser_start);

    let mut grouped: Vec<SourceMapSegment> = Vec::with_capacity(segments.len());
    for segment in segments {
        if let Some(previous) = grouped.last_mut()
            && previous.parser_start == segment.parser_start
        {
            previous.original_start = previous.original_start.max(segment.original_start);
        } else {
            grouped.push(segment);
        }
    }

    let mut canonical: Vec<SourceMapSegment> = Vec::with_capacity(grouped.len());
    for segment in grouped {
        let is_redundant = canonical.last().is_some_and(|previous| {
            previous
                .original_start
                .checked_add(segment.parser_start - previous.parser_start)
                == Some(segment.original_start)
        });
        if !is_redundant {
            canonical.push(segment);
        }
    }
    canonical
}

fn canonicalize_source_map_gaps(mut gaps: Vec<SourceMapGap>) -> Vec<SourceMapGap> {
    gaps.sort_by_key(|gap| gap.parser_offset);
    let mut canonical: Vec<SourceMapGap> = Vec::with_capacity(gaps.len());
    for gap in gaps {
        if let Some(previous) = canonical.last_mut()
            && previous.parser_offset == gap.parser_offset
        {
            previous.original_left = previous.original_left.min(gap.original_left);
            previous.original_right = previous.original_right.max(gap.original_right);
        } else if gap.original_left != gap.original_right {
            canonical.push(gap);
        }
    }
    canonical
}

fn push_source_map_segment(
    segments: &mut Vec<SourceMapSegment>,
    parser_start: usize,
    original_start: usize,
) {
    let Some(previous) = segments.last_mut() else {
        segments.push(SourceMapSegment {
            parser_start,
            original_start,
        });
        return;
    };
    if previous.parser_start == parser_start {
        previous.original_start = original_start;
        return;
    }
    let expected = previous
        .original_start
        .checked_add(parser_start - previous.parser_start);
    if expected != Some(original_start) {
        segments.push(SourceMapSegment {
            parser_start,
            original_start,
        });
    }
}

struct DeletionMappedText {
    text: String,
    source_map: SourceMapSegments,
}

impl DeletionMappedText {
    fn from_original(original: &str) -> Self {
        let (text, source_map) = normalize_original_with_source_map(original);
        Self { text, source_map }
    }

    fn remove_ranges(&mut self, ranges: &[(usize, usize)]) {
        let mut sorted: Vec<(usize, usize)> = ranges
            .iter()
            .copied()
            .filter(|(start, end)| start < end)
            .collect();
        sorted.sort_unstable_by_key(|&(start, end)| (start, end));

        let mut merged: Vec<(usize, usize)> = Vec::with_capacity(sorted.len());
        for (start, end) in sorted {
            debug_assert!(self.text.is_char_boundary(start));
            debug_assert!(self.text.is_char_boundary(end));
            if let Some((_, previous_end)) = merged.last_mut()
                && start <= *previous_end
            {
                *previous_end = (*previous_end).max(end);
            } else {
                merged.push((start, end));
            }
        }
        if merged.is_empty() {
            return;
        }

        let old_text = std::mem::take(&mut self.text);
        let removed_bytes: usize = merged.iter().map(|(start, end)| end - start).sum();
        let mut text = String::with_capacity(old_text.len() - removed_bytes);
        let mut cursor = 0;

        for &(start, end) in &merged {
            if cursor < start {
                text.push_str(&old_text[cursor..start]);
            }
            cursor = end;
        }
        if cursor < old_text.len() {
            text.push_str(&old_text[cursor..]);
        }

        self.source_map.remove_ranges(&merged);
        self.text = text;
        debug_assert_eq!(self.source_map.parser_len, self.text.len());
    }

    fn remove_frontmatter(&mut self) {
        if let Some(end) =
            crate::preprocess::split_frontmatter_block(&self.text).map(|block| block.full.end)
        {
            self.remove_ranges(&[(0, end)]);
        }
    }

    fn remove_directives(&mut self) {
        let mut ranges = Vec::new();
        let mut cursor = 0;
        while let Some(relative_start) = self.text[cursor..].find("%%{") {
            let start = cursor + relative_start;
            let after_start = start + 3;
            let Some(relative_end) = self.text[after_start..].find("}%%") else {
                ranges.push((start, self.text.len()));
                break;
            };
            let end = after_start + relative_end + 3;
            ranges.push((start, end));
            cursor = end;
        }
        self.remove_ranges(&ranges);
    }

    fn remove_full_line_comments(&mut self) {
        let mut ranges = Vec::new();
        let mut offset = 0;
        for line in self.text.split_inclusive('\n') {
            let trimmed = line.trim_start();
            if let Some(after_marker) = trimmed.strip_prefix("%%") {
                let has_comment_body = after_marker.chars().next().is_some_and(|ch| ch != '\n');
                if !after_marker.starts_with('{') && has_comment_body {
                    ranges.push((offset, offset + line.len()));
                }
            }
            offset += line.len();
        }
        self.remove_ranges(&ranges);

        let leading = self.text.len() - self.text.trim_start().len();
        self.remove_ranges(&[(0, leading)]);
    }
}

fn deletion_only_source_map(original: &str, preprocessed: &str) -> Option<SourceMapSegments> {
    let mut mapped = DeletionMappedText::from_original(original);
    mapped.remove_frontmatter();
    mapped.remove_directives();
    mapped.remove_full_line_comments();
    (mapped.text == preprocessed).then_some(mapped.source_map)
}

fn normalize_original_with_source_map(original: &str) -> (String, SourceMapSegments) {
    let mut normalized = String::with_capacity(original.len());
    let mut segments = vec![SourceMapSegment {
        parser_start: 0,
        original_start: 0,
    }];
    let bytes = original.as_bytes();
    let mut offset = 0;

    while offset < bytes.len() {
        if bytes[offset] == b'\r' {
            normalized.push('\n');
            offset += if bytes.get(offset + 1) == Some(&b'\n') {
                2
            } else {
                1
            };
            push_source_map_segment(&mut segments, normalized.len(), offset);
            continue;
        }

        let ch = original[offset..]
            .chars()
            .next()
            .expect("offset should be at a UTF-8 character boundary");
        normalized.push(ch);
        offset += ch.len_utf8();
    }

    let parser_len = normalized.len();
    (
        normalized,
        SourceMapSegments {
            parser_len,
            segments,
            gaps: Vec::new(),
        },
    )
}

#[cfg(test)]
mod editor_parse_source_map_tests {
    use super::{EditorParseSourceMap, EditorSourceRemap};
    use crate::{
        EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
        EditorSemanticSymbol, EditorSpanCoordinateSpace, Error, ParseDiagnosticSpanKind,
        SourceSpan,
    };

    #[test]
    fn parser_input_coordinate_parse_error_drops_span() {
        let map = EditorParseSourceMap::new(
            "flowchart TD\nA%% removed comment %%-->B\n",
            "flowchart TD\nA-->B\n",
        );
        let error = map.remap_parse_error(Error::diagram_parse_exact(
            "flowchart-v2",
            "bad parser input span",
            SourceSpan::new(13, 14),
        ));

        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected parse diagnostic");
        };
        assert_eq!(diagnostic.span(), None);
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Fallback);
    }

    #[test]
    fn normalized_parse_error_remaps_valid_crlf_span() {
        let original = "flowchart TD\r\nA-->B\r\n";
        let preprocessed = "flowchart TD\nA-->B\n";
        let target = preprocessed.find('B').expect("parser input target");
        let map = EditorParseSourceMap::new(original, preprocessed);
        let error = map.remap_parse_error(Error::diagram_parse_exact(
            "flowchart-v2",
            "bad parser input span",
            SourceSpan::new(target, target + 1),
        ));

        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected parse diagnostic");
        };
        let span = diagnostic.span().expect("remapped span");
        assert_eq!(&original[span.start..span.end], "B");
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
    }

    #[test]
    fn normalized_parse_error_drops_out_of_bounds_span() {
        let original = "flowchart TD\r\nA-->B\r\n";
        let preprocessed = "flowchart TD\nA-->B\n";
        let map = EditorParseSourceMap::new(original, preprocessed);
        let error = map.remap_parse_error(Error::diagram_parse_exact(
            "flowchart-v2",
            "bad parser input span",
            SourceSpan::new(preprocessed.len() + 1, preprocessed.len() + 2),
        ));

        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected parse diagnostic");
        };
        assert_eq!(diagnostic.span(), None);
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Fallback);
    }

    #[test]
    fn discontinuous_map_tracks_crlf_directives_and_removed_comment_lines() {
        let original = concat!(
            "flowchart TD\r\n",
            "A-->B\r\n",
            "%%{init: {\"theme\": \"default\"}}%%\r\n",
            "%% removed comment\r\n",
            "B-->C\r\n",
        );
        let preprocessed = "flowchart TD\nA-->B\n\nB-->C\n";
        let target = preprocessed.rfind('C').unwrap();
        let map = EditorParseSourceMap::new(original, preprocessed);
        let span = map
            .try_remap_source_span(SourceSpan::new(target, target + 1))
            .expect("discontinuous parser span should remap");

        assert_eq!(&original[span.start..span.end], "C");
        assert_eq!(span.start, original.rfind('C').unwrap());
    }

    #[test]
    fn discontinuous_map_storage_scales_with_edits_not_input_bytes() {
        let body = "A".repeat(128 * 1024);
        let original = format!("flowchart TD\n{body}\n%% removed comment\nB\n");
        let preprocessed = format!("flowchart TD\n{body}\nB\n");
        let map = EditorParseSourceMap::new(&original, &preprocessed);
        let storage_units = match &map.remap {
            EditorSourceRemap::Segmented { source_map, .. } => {
                source_map.segments.len() + source_map.gaps.len()
            }
            EditorSourceRemap::None
            | EditorSourceRemap::Offset(_)
            | EditorSourceRemap::ParserInputCoordinates => 0,
        };

        assert!(
            storage_units <= 16,
            "one deletion should use sparse mapping storage, got {storage_units} units"
        );
    }

    #[test]
    fn discontinuous_map_sends_eof_insertions_past_removed_trailing_comments() {
        let original = "flowchart TD\nA-->B\n%% trailing comment\n";
        let preprocessed = "flowchart TD\nA-->B\n";
        let map = EditorParseSourceMap::new(original, preprocessed);
        let eof = preprocessed.len();
        let span = map
            .try_remap_source_span(SourceSpan::new(eof, eof))
            .expect("parser EOF should remap after the removed suffix");

        assert_eq!(span, SourceSpan::new(original.len(), original.len()));
    }

    #[test]
    fn discontinuous_map_rejects_spans_that_cross_a_removed_segment() {
        let original = "architecture-beta\nservice be%%{wrap}%%fore\n";
        let preprocessed = "architecture-beta\nservice before\n";
        let map = EditorParseSourceMap::new(original, preprocessed);
        let start = preprocessed.find("before").unwrap();
        let span = SourceSpan::new(start, start + "before".len());

        assert_eq!(map.try_remap_source_span(span), None);
    }

    #[test]
    fn discontinuous_map_uses_directional_gap_boundaries() {
        let directive = "%%{wrap}%%";
        let original = format!("architecture-beta\nservice be{directive}fore\n");
        let preprocessed = "architecture-beta\nservice before\n";
        let map = EditorParseSourceMap::new(&original, preprocessed);
        let gap = preprocessed.find("before").unwrap() + "be".len();
        let original_gap_left = original.find(directive).unwrap();
        let original_gap_right = original_gap_left + directive.len();

        let prefix = map
            .try_remap_source_span(SourceSpan::new(gap - "be".len(), gap))
            .expect("span ending at the gap should use its left boundary");
        assert_eq!(prefix.end, original_gap_left);

        let suffix = map
            .try_remap_source_span(SourceSpan::new(gap, gap + "fore".len()))
            .expect("span starting at the gap should use its right boundary");
        assert_eq!(suffix.start, original_gap_right);

        let insertion = map
            .try_remap_source_span(SourceSpan::new(gap, gap))
            .expect("zero-width span should use the gap's right boundary");
        assert_eq!(
            insertion,
            SourceSpan::new(original_gap_right, original_gap_right)
        );
    }

    #[test]
    fn fact_remap_stays_in_parser_coordinates_when_any_span_is_unmappable() {
        let original = "architecture-beta\nservice be%%{wrap}%%fore\n";
        let preprocessed = "architecture-beta\nservice before\n";
        let map = EditorParseSourceMap::new(original, preprocessed);
        let start = preprocessed.find("before").unwrap();
        let span = SourceSpan::new(start, start + "before".len());
        let mut facts = EditorSemanticFacts::new();
        facts.push_symbol(EditorSemanticSymbol::new(
            "before",
            None,
            EditorSemanticKind::Variable,
            span,
            span,
        ));
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::NodeIdentifier,
            SourceSpan::new(preprocessed.len() + 1, preprocessed.len() + 1),
        ));

        map.remap_facts(&mut facts);

        assert_eq!(
            facts.span_coordinate_space,
            EditorSpanCoordinateSpace::ParserInput
        );
        assert_eq!(facts.symbols[0].selection, span);
    }
}

impl<'a> ParsePipeline<'a> {
    pub(crate) fn detect(engine: &'a Engine, text: &'a str, options: ParseOptions) -> Self {
        Self {
            engine,
            text,
            options,
            source: ParseSource::Detect,
        }
    }

    pub(crate) fn known_type(
        engine: &'a Engine,
        diagram_type: &'a str,
        text: &'a str,
        options: ParseOptions,
    ) -> Self {
        Self {
            engine,
            text,
            options,
            source: ParseSource::KnownType(diagram_type),
        }
    }

    pub(crate) fn metadata(&self) -> Result<Option<ParseMetadata>> {
        Ok(self.preprocess()?.map(|(_, meta)| meta))
    }

    pub(crate) fn parse_json(&self, timing: ParseTiming) -> Result<Option<ParsedDiagram>> {
        self.parse_model(
            timing,
            |pipeline, code, meta| {
                diagram::parse_or_unsupported(
                    &pipeline.engine.diagram_registry,
                    &meta.diagram_type,
                    code,
                    meta,
                )
            },
            common_db::apply_common_db_sanitization,
            error_diagram::suppressed_error_diagram,
            |meta, model| ParsedDiagram { meta, model },
            Self::remap_value_warning_facts,
            |_| None,
        )
    }

    pub(crate) fn parse_json_with_editor_facts(
        &self,
        timing: ParseTiming,
    ) -> Result<Option<ParsedDiagramWithEditorFacts>> {
        let timing_enabled = timing.is_enabled();
        let total_start = runtime::timing_start(timing_enabled);
        let preprocess_start = runtime::timing_start(timing_enabled);
        let directive_prefixes = editor_directive_prefixes(self.text);
        let Some((code, meta)) = self.preprocess()? else {
            return Ok(None);
        };
        let source_map = EditorParseSourceMap::new(self.text, &code);
        let editor_input = source_map.parser_input();
        let preprocess = preprocess_start.map(runtime::timing_elapsed);
        let uses_baseline_parser = registry_uses_baseline_semantic_parser(
            &self.engine.diagram_registry,
            &meta.diagram_type,
        );

        let parse_start = runtime::timing_start(timing_enabled);
        let parsed = match meta.diagram_type.as_str() {
            "flowchart-v2" | "flowchart" | "flowchart-elk" | "swimlane" if uses_baseline_parser => {
                let parse_res = self.with_fixed_time(|| {
                    crate::diagrams::flowchart::parse_flowchart_json_and_editor_facts(
                        editor_input,
                        &meta,
                    )
                });
                let parse = parse_start.map(runtime::timing_elapsed);
                let (mut model, facts) = match parse_res {
                    Ok(parsed) => parsed,
                    Err(err) => {
                        if !self.options.suppress_errors {
                            return Err(source_map.remap_parse_error(err));
                        }

                        timing.log_suppressed_error(
                            total_start,
                            preprocess,
                            parse,
                            self.text.len(),
                        );
                        return Ok(Some(ParsedDiagramWithEditorFacts {
                            diagram: error_diagram::suppressed_error_diagram(&meta),
                            editor_facts: ParsedEditorFacts::Unavailable,
                        }));
                    }
                };
                let sanitize_start = runtime::timing_start(timing_enabled);
                common_db::apply_common_db_sanitization(&mut model, &meta.effective_config);
                let sanitize = sanitize_start.map(runtime::timing_elapsed);
                Self::remap_value_warning_facts(&mut model, &source_map);
                timing.log_success(ParseTimingSuccess {
                    total_start,
                    meta: &meta,
                    model_kind: None,
                    preprocess,
                    parse,
                    sanitize,
                    input_bytes: self.text.len(),
                });
                let facts =
                    self.finish_editor_semantic_facts(facts, &source_map, directive_prefixes);
                return Ok(Some(ParsedDiagramWithEditorFacts {
                    diagram: ParsedDiagram { meta, model },
                    editor_facts: ParsedEditorFacts::Available(facts),
                }));
            }
            "architecture" if uses_baseline_parser => {
                let parse_res = self.with_fixed_time(|| {
                    crate::diagrams::architecture::parse_architecture_json_and_editor_facts(
                        editor_input,
                        &meta,
                    )
                });
                let parse = parse_start.map(runtime::timing_elapsed);
                let (mut model, facts) = match parse_res {
                    Ok(parsed) => parsed,
                    Err(err) => {
                        if !self.options.suppress_errors {
                            return Err(source_map.remap_parse_error(err));
                        }

                        timing.log_suppressed_error(
                            total_start,
                            preprocess,
                            parse,
                            self.text.len(),
                        );
                        return Ok(Some(ParsedDiagramWithEditorFacts {
                            diagram: error_diagram::suppressed_error_diagram(&meta),
                            editor_facts: ParsedEditorFacts::Unavailable,
                        }));
                    }
                };
                let sanitize_start = runtime::timing_start(timing_enabled);
                common_db::apply_common_db_sanitization(&mut model, &meta.effective_config);
                let sanitize = sanitize_start.map(runtime::timing_elapsed);
                Self::remap_value_warning_facts(&mut model, &source_map);
                timing.log_success(ParseTimingSuccess {
                    total_start,
                    meta: &meta,
                    model_kind: None,
                    preprocess,
                    parse,
                    sanitize,
                    input_bytes: self.text.len(),
                });
                let facts =
                    self.finish_editor_semantic_facts(facts, &source_map, directive_prefixes);
                return Ok(Some(ParsedDiagramWithEditorFacts {
                    diagram: ParsedDiagram { meta, model },
                    editor_facts: ParsedEditorFacts::Available(facts),
                }));
            }
            _ => {
                let parse_res = self.with_fixed_time(|| {
                    diagram::parse_or_unsupported(
                        &self.engine.diagram_registry,
                        &meta.diagram_type,
                        editor_input,
                        &meta,
                    )
                });
                let parse = parse_start.map(runtime::timing_elapsed);
                let mut model = match parse_res {
                    Ok(model) => model,
                    Err(err) => {
                        if !self.options.suppress_errors {
                            return Err(source_map.remap_parse_error(err));
                        }

                        timing.log_suppressed_error(
                            total_start,
                            preprocess,
                            parse,
                            self.text.len(),
                        );
                        return Ok(Some(ParsedDiagramWithEditorFacts {
                            diagram: error_diagram::suppressed_error_diagram(&meta),
                            editor_facts: ParsedEditorFacts::Unavailable,
                        }));
                    }
                };
                let sanitize_start = runtime::timing_start(timing_enabled);
                common_db::apply_common_db_sanitization(&mut model, &meta.effective_config);
                let sanitize = sanitize_start.map(runtime::timing_elapsed);
                timing.log_success(ParseTimingSuccess {
                    total_start,
                    meta: &meta,
                    model_kind: None,
                    preprocess,
                    parse,
                    sanitize,
                    input_bytes: self.text.len(),
                });
                ParsedDiagram { meta, model }
            }
        };

        let mut parsed = parsed;
        Self::remap_value_warning_facts(&mut parsed.model, &source_map);

        let editor_facts = if uses_baseline_parser {
            match self.parse_editor_semantic_facts_from_preprocessed(
                editor_input,
                &parsed.meta,
                &source_map,
                directive_prefixes,
            ) {
                Ok(Some(facts)) => ParsedEditorFacts::Available(facts),
                Ok(None) => ParsedEditorFacts::Unavailable,
                Err(error) => ParsedEditorFacts::Error(error),
            }
        } else {
            ParsedEditorFacts::Unavailable
        };
        Ok(Some(ParsedDiagramWithEditorFacts {
            diagram: parsed,
            editor_facts,
        }))
    }

    pub(crate) fn parse_render_model(&self) -> Result<Option<ParsedDiagramRender>> {
        self.parse_model(
            ParseTiming::Render,
            Self::parse_render_semantic_model,
            RenderSemanticModel::sanitize_common_db_fields,
            error_diagram::suppressed_error_render_diagram,
            |meta, model| ParsedDiagramRender { meta, model },
            |model, source_map| {
                model.remap_warning_fact_spans(|fact| {
                    Self::remap_warning_fact_spans(fact, source_map);
                });
            },
            |model| Some(model.kind()),
        )
    }

    pub(crate) fn parse_editor_semantic_facts(&self) -> Result<Option<EditorSemanticFacts>> {
        let mut directive_prefixes = editor_directive_prefixes(self.text);
        let Some((code, meta)) = self.preprocess()? else {
            return Ok(None);
        };
        if self
            .engine
            .diagram_registry
            .get(&meta.diagram_type)
            .is_some()
            && !registry_uses_baseline_semantic_parser(
                &self.engine.diagram_registry,
                &meta.diagram_type,
            )
        {
            return Ok(None);
        }
        let source_map = EditorParseSourceMap::new(self.text, &code);
        self.parse_editor_semantic_facts_from_preprocessed(
            source_map.parser_input(),
            &meta,
            &source_map,
            std::mem::take(&mut directive_prefixes),
        )
    }

    fn parse_editor_semantic_facts_from_preprocessed(
        &self,
        editor_input: &str,
        meta: &ParseMetadata,
        source_map: &EditorParseSourceMap<'_>,
        directive_prefixes: Vec<String>,
    ) -> Result<Option<EditorSemanticFacts>> {
        let registry_profile = self.engine.diagram_registry.profile();
        if !family::diagram_type_supported_in_profile(registry_profile, meta.diagram_type.as_str())
        {
            return Err(Error::UnsupportedDiagram {
                diagram_type: meta.diagram_type.clone(),
            });
        }

        let facts = match meta.diagram_type.as_str() {
            "flowchart-v2" | "flowchart" | "flowchart-elk" | "swimlane" => {
                crate::diagrams::flowchart::parse_flowchart_editor_facts(editor_input, &meta)?
            }
            "sequence" => {
                crate::diagrams::sequence::parse_sequence_editor_facts(editor_input, &meta)
            }
            "state" | "stateDiagram" => {
                crate::diagrams::state::parse_state_editor_facts(editor_input, &meta)
            }
            "class" | "classDiagram" => {
                crate::diagrams::class::parse_class_editor_facts(editor_input, &meta)
            }
            "er" | "erDiagram" => crate::diagrams::er::parse_er_editor_facts(editor_input, &meta),
            "mindmap" => crate::diagrams::mindmap::parse_mindmap_editor_facts(editor_input, &meta),
            "gantt" => crate::diagrams::gantt::parse_gantt_editor_facts(editor_input, &meta),
            "architecture" => {
                crate::diagrams::architecture::parse_architecture_editor_facts(editor_input, &meta)
            }
            "block" => crate::diagrams::block::parse_block_editor_facts(editor_input, &meta),
            "c4" => crate::diagrams::c4::parse_c4_editor_facts(editor_input, &meta),
            "cynefin" => crate::diagrams::cynefin::parse_cynefin_editor_facts(editor_input, &meta),
            "gitGraph" => {
                crate::diagrams::git_graph::parse_git_graph_editor_facts(editor_input, &meta)
            }
            "kanban" => crate::diagrams::kanban::parse_kanban_editor_facts(editor_input, &meta),
            "ishikawa" => {
                crate::diagrams::ishikawa::parse_ishikawa_editor_facts(editor_input, &meta)
            }
            "journey" => crate::diagrams::journey::parse_journey_editor_facts(editor_input, &meta),
            "info" => crate::diagrams::info::parse_info_editor_facts(editor_input, &meta),
            "timeline" => {
                crate::diagrams::timeline::parse_timeline_editor_facts(editor_input, &meta)
            }
            "pie" => crate::diagrams::pie::parse_pie_editor_facts(editor_input, &meta),
            "packet" => crate::diagrams::packet::parse_packet_editor_facts(editor_input, &meta),
            "sankey" => crate::diagrams::sankey::parse_sankey_editor_facts(editor_input, &meta),
            "treeView" => {
                crate::diagrams::tree_view::parse_tree_view_editor_facts(editor_input, &meta)
            }
            "eventmodeling" => crate::diagrams::eventmodeling::parse_eventmodeling_editor_facts(
                editor_input,
                &meta,
            ),
            "quadrantChart" => crate::diagrams::quadrant_chart::parse_quadrant_chart_editor_facts(
                editor_input,
                &meta,
            ),
            "railroad" => {
                crate::diagrams::railroad::parse_railroad_editor_facts(editor_input, &meta)
            }
            "railroadEbnf" => {
                crate::diagrams::railroad::parse_railroad_ebnf_editor_facts(editor_input, &meta)
            }
            "railroadAbnf" => {
                crate::diagrams::railroad::parse_railroad_abnf_editor_facts(editor_input, &meta)
            }
            "railroadPeg" => {
                crate::diagrams::railroad::parse_railroad_peg_editor_facts(editor_input, &meta)
            }
            "radar" => crate::diagrams::radar::parse_radar_editor_facts(editor_input, &meta),
            "treemap" => crate::diagrams::treemap::parse_treemap_editor_facts(editor_input, &meta),
            "requirement" => {
                crate::diagrams::requirement::parse_requirement_editor_facts(editor_input, &meta)
            }
            "venn" => crate::diagrams::venn::parse_venn_editor_facts(editor_input, &meta),
            "xychart" => crate::diagrams::xychart::parse_xychart_editor_facts(editor_input, &meta),
            "zenuml" => crate::diagrams::zenuml::parse_zenuml_editor_facts(editor_input, &meta),
            _ => return Ok(None),
        };

        Ok(Some(self.finish_editor_semantic_facts(
            facts,
            source_map,
            directive_prefixes,
        )))
    }

    fn finish_editor_semantic_facts(
        &self,
        facts: EditorSemanticFacts,
        source_map: &EditorParseSourceMap<'_>,
        mut directive_prefixes: Vec<String>,
    ) -> EditorSemanticFacts {
        let EditorSemanticFacts {
            completeness,
            span_coordinate_space: _,
            symbols,
            directive_prefixes: family_directive_prefixes,
            diagnostics,
            expected_syntax,
        } = facts;
        directive_prefixes.extend(family_directive_prefixes);
        let mut facts = EditorSemanticFacts {
            completeness,
            span_coordinate_space: EditorSpanCoordinateSpace::OriginalSource,
            symbols,
            directive_prefixes: Vec::new(),
            diagnostics,
            expected_syntax,
        };
        source_map.remap_facts(&mut facts);
        for prefix in directive_prefixes {
            facts.push_directive_prefix(prefix);
        }
        facts
    }

    fn parse_model<T, O>(
        &self,
        timing: ParseTiming,
        parse: impl FnOnce(&Self, &str, &ParseMetadata) -> Result<T>,
        sanitize: impl FnOnce(&mut T, &MermaidConfig),
        suppressed: impl FnOnce(&ParseMetadata) -> O,
        finish: impl FnOnce(ParseMetadata, T) -> O,
        postprocess: impl FnOnce(&mut T, &EditorParseSourceMap<'_>),
        model_kind: impl FnOnce(&T) -> Option<&'static str>,
    ) -> Result<Option<O>> {
        let timing_enabled = timing.is_enabled();
        let total_start = runtime::timing_start(timing_enabled);

        let preprocess_start = runtime::timing_start(timing_enabled);
        let Some((code, meta)) = self.preprocess()? else {
            return Ok(None);
        };
        let source_map = EditorParseSourceMap::new(self.text, &code);
        let preprocess = preprocess_start.map(runtime::timing_elapsed);

        let parse_start = runtime::timing_start(timing_enabled);
        let parse_res = self.with_fixed_time(|| parse(self, source_map.parser_input(), &meta));
        let parse = parse_start.map(runtime::timing_elapsed);

        let mut model = match parse_res {
            Ok(model) => model,
            Err(err) => {
                if !self.options.suppress_errors {
                    return Err(source_map.remap_parse_error(err));
                }

                timing.log_suppressed_error(total_start, preprocess, parse, self.text.len());
                return Ok(Some(suppressed(&meta)));
            }
        };

        let sanitize_start = runtime::timing_start(timing_enabled);
        sanitize(&mut model, &meta.effective_config);
        let sanitize = sanitize_start.map(runtime::timing_elapsed);
        postprocess(&mut model, &source_map);

        timing.log_success(ParseTimingSuccess {
            total_start,
            meta: &meta,
            model_kind: model_kind(&model),
            preprocess,
            parse,
            sanitize,
            input_bytes: self.text.len(),
        });

        Ok(Some(finish(meta, model)))
    }

    fn remap_value_warning_facts(
        model: &mut serde_json::Value,
        source_map: &EditorParseSourceMap<'_>,
    ) {
        let Some(warning_facts_value) = model.get_mut("warningFacts") else {
            return;
        };
        let Ok(mut warning_facts) =
            serde_json::from_value::<Vec<DiagramWarningFact>>(warning_facts_value.clone())
        else {
            return;
        };

        for fact in &mut warning_facts {
            Self::remap_warning_fact_spans(fact, source_map);
        }

        *warning_facts_value = serde_json::json!(warning_facts);
    }

    fn remap_warning_fact_spans(
        fact: &mut DiagramWarningFact,
        source_map: &EditorParseSourceMap<'_>,
    ) {
        let source_span = fact.span;
        let remapped_span =
            source_span.and_then(|span| source_map.try_remap_warning_source_span(span));
        fact.span = remapped_span;
        fact.fix_span = match (fact.fix_span, source_span, remapped_span) {
            (Some(fix_span), Some(source_span), Some(remapped_span))
                if fix_span.start == fix_span.end && fix_span.start == source_span.end =>
            {
                Some(SourceSpan::new(remapped_span.end, remapped_span.end))
            }
            (Some(fix_span), _, _) => source_map.try_remap_warning_source_span(fix_span),
            (None, _, _) => None,
        };
    }

    fn parse_render_semantic_model(
        &self,
        code: &str,
        meta: &ParseMetadata,
    ) -> Result<RenderSemanticModel> {
        if let Some(parser) = self.engine.render_diagram_registry.get(&meta.diagram_type) {
            return parser(code, meta);
        }

        let registry_profile = self.engine.render_diagram_registry.profile();
        debug_assert_eq!(self.engine.diagram_registry.profile(), registry_profile);
        if !family::permits_json_render_fallback(registry_profile, &meta.diagram_type) {
            return Err(Error::diagram_parse_fallback(
                meta.diagram_type.clone(),
                format!(
                    "built-in diagram type `{}` is missing a typed render parser; JSON render fallback is reserved for error and custom diagram adapters",
                    meta.diagram_type
                ),
            ));
        }

        diagram::parse_or_unsupported(
            &self.engine.diagram_registry,
            &meta.diagram_type,
            code,
            meta,
        )
        .map(RenderSemanticModel::Json)
    }

    fn preprocess(&self) -> Result<Option<(String, ParseMetadata)>> {
        match self.source {
            ParseSource::Detect => self.preprocess_and_detect(),
            ParseSource::KnownType(diagram_type) => self.preprocess_and_assume_type(diagram_type),
        }
    }

    fn preprocess_and_detect(&self) -> Result<Option<(String, ParseMetadata)>> {
        let pre = preprocess_diagram(self.text, &self.engine.registry)?;
        if pre.code.trim_start().starts_with("---") {
            return Err(Error::MalformedFrontMatter);
        }

        let has_config_overrides = !pre.config.is_empty_object();
        let mut effective_config = self.effective_config_before_detect(&pre.config);
        let cached_effective_config = (!has_config_overrides).then(|| effective_config.clone());

        let diagram_type = match self
            .engine
            .registry
            .detect_type_precleaned(&pre.code, &mut effective_config)
        {
            Ok(diagram_type) => diagram_type.to_string(),
            Err(err) => {
                if self.options.suppress_errors {
                    return Ok(None);
                }
                return Err(err);
            }
        };
        family::apply_diagram_type_config_defaults(
            &diagram_type,
            &pre.config,
            &mut effective_config,
        );
        if has_config_overrides {
            theme::apply_theme_defaults(&mut effective_config);
        } else if cached_effective_config
            .as_ref()
            .is_some_and(|cached| effective_config.ptr_eq(cached))
        {
            effective_config = self.engine.default_effective_config();
        } else {
            theme::apply_theme_defaults(&mut effective_config);
        }

        let title = sanitized_title(pre.title.as_deref(), &effective_config);

        Ok(Some((
            pre.code,
            ParseMetadata {
                diagram_type,
                config: pre.config,
                effective_config,
                title,
            },
        )))
    }

    fn preprocess_and_assume_type(
        &self,
        diagram_type: &str,
    ) -> Result<Option<(String, ParseMetadata)>> {
        let pre = preprocess_diagram_with_known_type(
            self.text,
            &self.engine.registry,
            Some(diagram_type),
        )?;
        if pre.code.trim_start().starts_with("---") {
            return Err(Error::MalformedFrontMatter);
        }

        let has_config_overrides = !pre.config.is_empty_object();
        let mut effective_config = self.effective_config_before_detect(&pre.config);
        let cached_effective_config = (!has_config_overrides).then(|| effective_config.clone());
        family::apply_known_type_detector_side_effects(diagram_type, &mut effective_config);
        family::apply_diagram_type_config_defaults(
            diagram_type,
            &pre.config,
            &mut effective_config,
        );
        if has_config_overrides {
            theme::apply_theme_defaults(&mut effective_config);
        } else if cached_effective_config
            .as_ref()
            .is_some_and(|cached| effective_config.ptr_eq(cached))
        {
            effective_config = self.engine.default_effective_config();
        } else {
            theme::apply_theme_defaults(&mut effective_config);
        }

        let title = sanitized_title(pre.title.as_deref(), &effective_config);

        Ok(Some((
            pre.code,
            ParseMetadata {
                diagram_type: diagram_type.to_string(),
                config: pre.config,
                effective_config,
                title,
            },
        )))
    }

    fn with_fixed_time<R>(&self, f: impl FnOnce() -> R) -> R {
        runtime::with_fixed_today_local(self.engine.fixed_today_local, || {
            runtime::with_fixed_local_offset_minutes(self.engine.fixed_local_offset_minutes, f)
        })
    }

    fn effective_config_before_detect(&self, overrides: &MermaidConfig) -> MermaidConfig {
        if overrides.is_empty_object() {
            return self.engine.site_config.clone();
        }

        let mut effective_config = self.engine.site_config.clone();
        let effective_overrides = effective_config.secure_filtered_overrides(overrides);
        effective_config.deep_merge(effective_overrides.as_value());
        effective_config
    }
}

impl ParseTiming {
    fn is_enabled(self) -> bool {
        self != Self::None && Engine::parse_timing_enabled()
    }

    fn log_suppressed_error(
        self,
        total_start: Option<runtime::TimingInstant>,
        preprocess: Option<runtime::TimingDuration>,
        parse: Option<runtime::TimingDuration>,
        input_bytes: usize,
    ) {
        let Some(start) = total_start else {
            return;
        };

        match self {
            Self::None => {}
            Self::Json => {
                eprintln!(
                    "[parse-timing] diagram=error total={:?} preprocess={:?} parse={:?} sanitize={:?} input_bytes={}",
                    runtime::timing_elapsed(start),
                    preprocess.unwrap_or_default(),
                    parse.unwrap_or_default(),
                    runtime::timing_zero_duration(),
                    input_bytes,
                );
            }
            Self::Render => {
                eprintln!(
                    "[parse-render-timing] diagram=error model=json total={:?} preprocess={:?} parse={:?} sanitize={:?} input_bytes={}",
                    runtime::timing_elapsed(start),
                    preprocess.unwrap_or_default(),
                    parse.unwrap_or_default(),
                    runtime::timing_zero_duration(),
                    input_bytes,
                );
            }
        }
    }

    fn log_success(self, success: ParseTimingSuccess<'_>) {
        let Some(start) = success.total_start else {
            return;
        };

        match self {
            Self::None => {}
            Self::Json => {
                eprintln!(
                    "[parse-timing] diagram={} total={:?} preprocess={:?} parse={:?} sanitize={:?} input_bytes={}",
                    success.meta.diagram_type,
                    runtime::timing_elapsed(start),
                    success.preprocess.unwrap_or_default(),
                    success.parse.unwrap_or_default(),
                    success.sanitize.unwrap_or_default(),
                    success.input_bytes,
                );
            }
            Self::Render => {
                eprintln!(
                    "[parse-render-timing] diagram={} model={} total={:?} preprocess={:?} parse={:?} sanitize={:?} input_bytes={}",
                    success.meta.diagram_type,
                    success.model_kind.unwrap_or("unknown"),
                    runtime::timing_elapsed(start),
                    success.preprocess.unwrap_or_default(),
                    success.parse.unwrap_or_default(),
                    success.sanitize.unwrap_or_default(),
                    success.input_bytes,
                );
            }
        }
    }
}

struct ParseTimingSuccess<'a> {
    total_start: Option<runtime::TimingInstant>,
    meta: &'a ParseMetadata,
    model_kind: Option<&'static str>,
    preprocess: Option<runtime::TimingDuration>,
    parse: Option<runtime::TimingDuration>,
    sanitize: Option<runtime::TimingDuration>,
    input_bytes: usize,
}

fn editor_directive_prefixes(text: &str) -> Vec<String> {
    let mut prefixes = Vec::new();
    for line in text.lines() {
        if let Some(prefix) = editor_directive_prefix(line) {
            let prefix = prefix.to_string();
            if !prefixes.contains(&prefix) {
                prefixes.push(prefix);
            }
        }
    }
    prefixes
}

fn editor_directive_prefix(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();

    if let Some(rest) = trimmed.strip_prefix("%%{") {
        let name = rest
            .split(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '}'))
            .next()
            .filter(|name| !name.is_empty())?;

        return matches!(name, "init" | "initialize" | "wrap").then_some(match name {
            "init" => "init",
            "initialize" => "initialize",
            "wrap" => "wrap",
            _ => unreachable!(),
        });
    }

    if trimmed.starts_with(":::") {
        return Some(":::");
    }

    None
}

fn sanitized_title(title: Option<&str>, effective_config: &MermaidConfig) -> Option<String> {
    title
        .map(|title| sanitize::sanitize_text(title, effective_config))
        .filter(|title| !title.is_empty())
}

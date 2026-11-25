
use super::*;
use std::fmt::Write as _;

fn new_source(source: &str) -> MappedSource {
    MappedSource::new_controlled(source, false, &OperationControl::new()).unwrap()
}

#[test]
fn composes_deletions_replacements_and_unicode_without_global_degradation() {
    let original = "前A\r\nREMOVE中#quot;后";
    let mut source = new_source(original);
    let cr = source.text().find('\r').unwrap();
    source.apply_edits_uncontrolled(vec![SourceEdit::replace(
        cr..cr + 2,
        "\n",
        ReplacementMapping::Boundaries,
    )]);
    let remove = source.text().find("REMOVE").unwrap();
    source.apply_edits_uncontrolled(vec![SourceEdit::delete(remove..remove + "REMOVE".len())]);
    let hash = source.text().find('#').unwrap();
    source.apply_edits_uncontrolled(vec![
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
fn enclosing_mapping_preserves_provenance_across_deleted_indentation() {
    let original = "  first\n  second\n";
    let mut source = new_source(original);
    let second_indent = original.find("  second").expect("second-line indentation");
    source.apply_edits_uncontrolled(vec![
        SourceEdit::delete(0..2),
        SourceEdit::delete(second_indent..second_indent + 2),
    ]);

    let span = SourceSpan::new(0, "first\nsecond".len());
    assert_eq!(source.try_map_span(span), None);
    let enclosing = source
        .try_map_enclosing_span(span)
        .expect("provenance boundaries remain mappable");
    assert_eq!(&original[enclosing.start..enclosing.end], "first\n  second");
}

#[test]
fn boundary_replacements_only_reject_locally_ambiguous_offsets() {
    let mut source = new_source("A#x;B");
    source.apply_edits_uncontrolled(vec![
        SourceEdit::replace(1..2, "ﬂ°", ReplacementMapping::Boundaries),
        SourceEdit::replace(3..4, "¶ß", ReplacementMapping::Boundaries),
    ]);
    assert_eq!(source.text(), "Aﬂ°x¶ßB");

    let b = source.text().find('B').unwrap();
    assert_eq!(
        source.try_map_span(SourceSpan::new(b, b + 1)),
        Some(SourceSpan::new(4, 5))
    );
    assert_eq!(source.try_map_span(SourceSpan::new(2, 2)), None);

    let identity = new_source("A😀B");
    assert_eq!(identity.try_map_span(SourceSpan::new(2, 2)), None);
}

#[test]
fn controlled_text_rebuild_cancellation_preserves_the_previous_source_atomically() {
    let original = format!(
        "{}TAIL",
        "a".repeat(CONTROLLED_EDIT_REBUILD_CHECKPOINT_BYTES * 2)
    );
    let mut source = new_source(&original);
    let before = source.clone();
    let tail = source.text().len() - "TAIL".len();
    let control = OperationControl::new();
    control.cancel_after_checkpoints(1);

    let result = source.apply_edits(
        vec![SourceEdit::replace(
            tail..tail + "TAIL".len(),
            "tail",
            ReplacementMapping::ExactBytes,
        )],
        &control,
    );

    assert!(result.is_err());
    assert!(control.is_cancelled());
    assert_source_state_eq(&source, &before);
}

#[test]
fn controlled_mapping_rebuild_cancellation_preserves_the_previous_source_atomically() {
    let original = "x".repeat(CONTROLLED_EDIT_REBUILD_CHECKPOINT_ITEMS * 2);
    let mut source =
        MappedSource::new_controlled(&original, true, &OperationControl::new()).unwrap();
    source.apply_edits_uncontrolled(
        (0..original.len())
            .map(|offset| {
                SourceEdit::replace(offset..offset + 1, "x", ReplacementMapping::Boundaries)
            })
            .collect(),
    );
    let map = source.resolved_edit_map().unwrap();
    assert!(map.segments.len() > CONTROLLED_EDIT_REBUILD_CHECKPOINT_ITEMS);
    assert!(source.text().len() < CONTROLLED_EDIT_REBUILD_CHECKPOINT_BYTES);

    let before = source.clone();
    let control = OperationControl::new();
    control.cancel_after_checkpoints(1);
    let last_byte = source.text().len() - 1;
    let result = source.apply_edits(vec![SourceEdit::delete(last_byte..last_byte + 1)], &control);

    assert!(result.is_err());
    assert!(control.is_cancelled());
    assert_source_state_eq(&source, &before);
}

#[test]
fn deferred_map_waits_for_spans_and_can_retry_cancelled_composition() {
    let removed = "discard\r\n".repeat(CONTROLLED_EDIT_REBUILD_CHECKPOINT_ITEMS * 2);
    let original = format!("{removed}前😀\r\nTAIL");
    let mut source = new_source(&original);
    source.apply_edits_uncontrolled(crlf_normalization_edits(source.text()));
    source.apply_edits_uncontrolled(vec![SourceEdit::delete(
        0..source.text().find('前').unwrap(),
    )]);
    assert_eq!(source.text(), "前😀\nTAIL");
    let SourceMapping::Deferred(map) = &source.mapping else {
        panic!("ordinary parsing records edits for deferred composition");
    };

    assert!(map.composed.get().is_none());
    let len = "前😀".len();
    let span = SourceSpan::new(0, len);
    let expected = SourceSpan::new(removed.len(), removed.len() + len);
    let cancelled = OperationControl::new();
    cancelled.cancel_after_checkpoints(1);
    assert!(source.prepare_span_mapping_controlled(&cancelled).is_err());
    assert!(map.composed.get().is_none());

    source
        .prepare_span_mapping_controlled(&OperationControl::new())
        .unwrap();
    assert_eq!(source.try_map_span(span), Some(expected));
    assert!(map.composed.get().is_some());
}

#[test]
fn same_offset_insertions_and_adjacent_deletions_coalesce_gaps_in_order() {
    let mut source = new_source("aXbYc");
    source.apply_edits_uncontrolled(vec![SourceEdit::delete(1..2)]);

    source.apply_edits_uncontrolled(vec![
        SourceEdit::replace(1..1, "<", ReplacementMapping::Boundaries),
        SourceEdit::replace(1..1, ">", ReplacementMapping::Boundaries),
        SourceEdit::delete(1..2),
        SourceEdit::delete(2..3),
    ]);

    assert_eq!(source.text(), "a<>c");
    let map = source.resolved_edit_map().unwrap();
    assert!(map.is_well_formed());
    assert_eq!(map.gaps.len(), 2);

    let c = source.text().find('c').unwrap();
    assert_eq!(
        source.try_map_span(SourceSpan::new(c, c + 1)),
        Some(SourceSpan::new(4, 5))
    );
    assert_eq!(source.try_map_span(SourceSpan::new(0, c + 1)), None);
}

#[test]
fn composing_an_edit_inside_generated_text_marks_only_that_region_unmappable() {
    let mut source = new_source("A#x;B");
    source.apply_edits_uncontrolled(vec![SourceEdit::replace(
        1..2,
        "ﬂ°",
        ReplacementMapping::Boundaries,
    )]);
    let degree = source.text().find('°').unwrap();
    source.apply_edits_uncontrolled(vec![SourceEdit::replace(
        degree..degree + '°'.len_utf8(),
        "!",
        ReplacementMapping::ExactBytes,
    )]);
    let map = source.resolved_edit_map().unwrap();
    assert!(map.unmapped_ranges_match_segments());
    assert!(!map.unmapped_output_ranges.is_empty());

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

#[test]
fn crlf_comment_batches_scan_old_map_once_and_preserve_offsets() {
    const MIN_INPUT_BYTES: usize = 1024 * 1024;

    let mut original = String::with_capacity(MIN_INPUT_BYTES + 256);
    let mut pair_count = 0usize;
    while original.len() < MIN_INPUT_BYTES {
        let group = if pair_count.is_multiple_of(2) {
            "even"
        } else {
            "odd"
        };
        write!(
            original,
            "KEEP-{pair_count:05} payload\r\n%% {group}-comment-{pair_count:05} payload\r\n"
        )
        .unwrap();
        pair_count += 1;
    }
    original.push_str("TAIL\r\n");
    assert!(original.len() >= MIN_INPUT_BYTES);

    let mut source =
        MappedSource::new_controlled(&original, true, &OperationControl::new()).unwrap();
    source.apply_edits_uncontrolled(crlf_normalization_edits(source.text()));
    assert!(!source.text().contains('\r'));
    assert!(
        source
            .resolved_edit_map()
            .unwrap()
            .unmapped_output_ranges
            .is_empty(),
        "CRLF boundary mappings must not make every fact lookup scan every line"
    );

    let even_comments = comment_line_deletions(source.text(), "%% even-comment-");
    source.apply_edits_uncontrolled(even_comments);

    let old_map = source.resolved_edit_map().unwrap();
    let old_segment_count = old_map.segments.len();
    let old_gap_count = old_map.gaps.len();
    let odd_comments = comment_line_deletions(source.text(), "%% odd-comment-");
    let odd_comment_count = odd_comments.len();
    assert!(odd_comment_count > 8_000);
    assert!(old_gap_count > 8_000);

    source.apply_edits_uncontrolled(odd_comments);

    let scans = source.resolved_edit_map().unwrap().scan_stats;
    assert_eq!(scans.copy_ranges, odd_comment_count + 1);
    assert!(
        scans.copy_segment_steps <= old_segment_count + scans.copy_ranges,
        "copying revisited old segments: {scans:?} for {old_segment_count} segments"
    );
    assert!(
        scans.copy_gap_steps <= old_gap_count + scans.copy_ranges,
        "copying revisited old gaps: {scans:?} for {old_gap_count} gaps"
    );
    assert!(
        scans.lookup_segment_advances <= old_segment_count,
        "endpoint lookup revisited old segments: {scans:?}"
    );
    assert!(
        scans.lookup_gap_advances <= old_gap_count,
        "endpoint lookup revisited old gaps: {scans:?}"
    );

    assert!(!source.text().contains("%%"));
    for pair_index in [0, pair_count / 2, pair_count - 1] {
        let literal = format!("KEEP-{pair_index:05}");
        assert_literal_mapping(&source, &original, &literal, &literal);
    }

    let first_keep = "KEEP-00000 payload";
    let output_newline = source.text().find(first_keep).unwrap() + first_keep.len();
    let original_newline = original.find(first_keep).unwrap() + first_keep.len();
    assert_eq!(
        source.try_map_span(SourceSpan::new(output_newline, output_newline + 1)),
        Some(SourceSpan::new(original_newline, original_newline + 2))
    );

    let second_keep = source.text().find("KEEP-00001").unwrap();
    assert_eq!(
        source.try_map_span(SourceSpan::new(output_newline, second_keep + 1)),
        None,
        "a span crossing a removed comment must remain unmappable"
    );
}

#[test]
fn seeded_edit_compositions_preserve_exact_monotonic_source_ranges() {
    const SEED: u64 = 0x4d45_524d_414e_5538;
    const CASES: usize = 64;
    let original = "前|KEEP_A|COPY|DELETE|\r\n|#quot;|<span title=\"值😀\">|WIDE|KEEP_B|后";
    let mut rng = DeterministicRng::new(SEED);

    for case in 0..CASES {
        let mut source = new_source(original);
        let mut eager =
            MappedSource::new_controlled(original, true, &OperationControl::new()).unwrap();
        for transformation in shuffled_transformations(&mut rng) {
            apply_transformation(&mut source, transformation);
            apply_transformation(&mut eager, transformation);
            assert_source_state_eq(&source, &eager);
            assert_mapping_invariants(&source, original, case, transformation);
        }

        assert_literal_mapping(&source, original, "KEEP_A", "KEEP_A");
        assert_literal_mapping(&source, original, "copy", "COPY");
        assert_literal_mapping(&source, original, "\n", "\r\n");
        assert_literal_mapping(
            &source,
            original,
            "title=&quot;值😀&quot;",
            "title=\"值😀\"",
        );
        assert_literal_mapping(&source, original, "长值😀", "WIDE");
        assert_literal_mapping(&source, original, "KEEP_B", "KEEP_B");

        let crossing_start = source.text().find("copy").unwrap();
        let crossing_end = source.text().find('\n').unwrap() + 1;
        assert_eq!(
            source.try_map_span(SourceSpan::new(crossing_start, crossing_end)),
            None,
            "case {case}: a span crossing deleted source must stay locally unmappable"
        );

        if case % 2 == 0 {
            assert_literal_mapping(&source, original, "ﬂ°quot¶ß", "#quot;");
        } else {
            let degree = source.text().find('°').unwrap();
            let edits = vec![SourceEdit::delete(degree..degree + '°'.len_utf8())];
            source.apply_edits_uncontrolled(edits.clone());
            eager.apply_edits_uncontrolled(edits);
            assert_source_state_eq(&source, &eager);
            assert_mapping_invariants(
                &source,
                original,
                case,
                SeededTransformation::GeneratedInteriorDeletion,
            );
            let generated = source.text().find("ﬂquot¶ß").unwrap();
            assert_eq!(
                source.try_map_span(SourceSpan::new(generated, generated + "ﬂquot¶ß".len(),)),
                None,
                "case {case}: editing generated bytes must not invent an original range"
            );
            assert_literal_mapping(&source, original, "KEEP_B", "KEEP_B");
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum SeededTransformation {
    ExactByteReplacement,
    Deletion,
    CrLfNormalization,
    EntityEncoding,
    QuoteExpansion,
    BoundaryReplacement,
    GeneratedInteriorDeletion,
}

struct DeterministicRng(u64);

impl DeterministicRng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn index(&mut self, upper: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 as usize) % upper
    }
}

fn shuffled_transformations(rng: &mut DeterministicRng) -> [SeededTransformation; 6] {
    let mut transformations = [
        SeededTransformation::ExactByteReplacement,
        SeededTransformation::Deletion,
        SeededTransformation::CrLfNormalization,
        SeededTransformation::EntityEncoding,
        SeededTransformation::QuoteExpansion,
        SeededTransformation::BoundaryReplacement,
    ];
    for index in (1..transformations.len()).rev() {
        transformations.swap(index, rng.index(index + 1));
    }
    transformations
}

fn apply_transformation(source: &mut MappedSource, transformation: SeededTransformation) {
    match transformation {
        SeededTransformation::ExactByteReplacement => {
            replace_once(source, "COPY", "copy", ReplacementMapping::ExactBytes);
        }
        SeededTransformation::Deletion => {
            let start = source.text().find("DELETE|").unwrap();
            source
                .apply_edits_uncontrolled(vec![SourceEdit::delete(start..start + "DELETE|".len())]);
        }
        SeededTransformation::CrLfNormalization => {
            replace_once(source, "\r\n", "\n", ReplacementMapping::Boundaries);
        }
        SeededTransformation::EntityEncoding => {
            let start = source.text().find("#quot;").unwrap();
            source.apply_edits_uncontrolled(vec![
                SourceEdit::replace(start..start + 1, "ﬂ°", ReplacementMapping::Boundaries),
                SourceEdit::replace(
                    start + "#quot".len()..start + "#quot;".len(),
                    "¶ß",
                    ReplacementMapping::Boundaries,
                ),
            ]);
        }
        SeededTransformation::QuoteExpansion => {
            let edits = source
                .text()
                .match_indices('"')
                .map(|(start, _)| {
                    SourceEdit::replace(start..start + 1, "&quot;", ReplacementMapping::Boundaries)
                })
                .collect();
            source.apply_edits_uncontrolled(edits);
        }
        SeededTransformation::BoundaryReplacement => {
            replace_once(source, "WIDE", "长值😀", ReplacementMapping::Boundaries);
        }
        SeededTransformation::GeneratedInteriorDeletion => unreachable!(),
    }
}

fn replace_once(
    source: &mut MappedSource,
    needle: &str,
    replacement: &'static str,
    mapping: ReplacementMapping,
) {
    let start = source.text().find(needle).unwrap();
    source.apply_edits_uncontrolled(vec![SourceEdit::replace(
        start..start + needle.len(),
        replacement,
        mapping,
    )]);
}

fn crlf_normalization_edits(text: &str) -> Vec<SourceEdit> {
    text.match_indices("\r\n")
        .map(|(start, _)| {
            SourceEdit::replace(start..start + 2, "\n", ReplacementMapping::Boundaries)
        })
        .collect()
}

fn comment_line_deletions(text: &str, prefix: &str) -> Vec<SourceEdit> {
    let mut line_start = 0usize;
    text.split_inclusive('\n')
        .filter_map(|line| {
            let range = line_start..line_start + line.len();
            line_start = range.end;
            line.starts_with(prefix).then(|| SourceEdit::delete(range))
        })
        .collect()
}

fn assert_literal_mapping(
    source: &MappedSource,
    original: &str,
    output_literal: &str,
    original_literal: &str,
) {
    let output_start = source.text().find(output_literal).unwrap();
    let original_start = original.find(original_literal).unwrap();
    assert_eq!(
        source.try_map_span(SourceSpan::new(
            output_start,
            output_start + output_literal.len(),
        )),
        Some(SourceSpan::new(
            original_start,
            original_start + original_literal.len(),
        )),
        "{output_literal:?} must map exactly to {original_literal:?}"
    );
}

fn assert_source_state_eq(actual: &MappedSource, expected: &MappedSource) {
    assert_eq!(actual.text, expected.text);
    assert_eq!(actual.resolved_edit_map(), expected.resolved_edit_map());
}

fn assert_mapping_invariants(
    source: &MappedSource,
    original: &str,
    case: usize,
    transformation: SeededTransformation,
) {
    let map = source.resolved_edit_map().unwrap();
    assert!(
        map.is_well_formed(),
        "case {case} after {transformation:?}: malformed composed edit map"
    );
    assert_eq!(map.original_len, original.len());
    assert_eq!(map.output_len, source.text().len());

    let mut boundaries = source
        .text()
        .char_indices()
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    boundaries.push(source.text().len());

    let mut previous_original_offset = None;
    for &offset in &boundaries {
        if let Some(mapped) = source.try_map_span(SourceSpan::new(offset, offset)) {
            assert_eq!(mapped.start, mapped.end);
            assert!(mapped.end <= original.len());
            assert!(original.is_char_boundary(mapped.start));
            if let Some(previous) = previous_original_offset {
                assert!(
                    previous <= mapped.start,
                    "case {case} after {transformation:?}: output boundary mapping regressed"
                );
            }
            previous_original_offset = Some(mapped.start);
        }
    }

    for (start_index, &start) in boundaries.iter().enumerate() {
        for &end in &boundaries[start_index..] {
            let Some(mapped) = source.try_map_span(SourceSpan::new(start, end)) else {
                continue;
            };
            assert!(mapped.start <= mapped.end);
            assert!(mapped.end <= original.len());
            assert!(original.is_char_boundary(mapped.start));
            assert!(original.is_char_boundary(mapped.end));
        }
    }

    assert_eq!(
        source.try_map_span(SourceSpan::new(0, source.text().len() + 1)),
        None
    );
    let emoji = source.text().find('😀').unwrap();
    assert_eq!(
        source.try_map_span(SourceSpan::new(emoji + 1, emoji + 1)),
        None
    );
}

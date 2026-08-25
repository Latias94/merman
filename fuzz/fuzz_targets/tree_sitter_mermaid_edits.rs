#![no_main]

use libfuzzer_sys::fuzz_target;
use tree_sitter::{InputEdit, Language, Node, Parser, Point};
use tree_sitter_mermaid::LANGUAGE;

const MAX_INPUT_BYTES: usize = 256 * 1024;
const MAX_SOURCE_BYTES: usize = 64 * 1024;
const MAX_EDITS: usize = 32;
const OPERATION_BYTES: usize = 8;
const REGRESSION_SEED_PREFIX: &[u8] = b"seed\n";
const REGRESSION_OPERATIONS: &[u8] = &[
    0, 0, 0, 1, b' ', 0, 0, 0, 5, 0, 1, 0, 0, 0, 0, 0, 2, 0, 0, 3, 0xe7, 0xbb, 0x88, 0, 0xff, 0, 2,
    3, 0xe2, 0x94, 0x82, 0,
];

fn parser() -> Parser {
    let language: Language = LANGUAGE.into();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .expect("generated Mermaid language must load");
    parser
}

fn point_at(source: &[u8], byte: usize) -> Point {
    let mut point = Point { row: 0, column: 0 };
    for &value in &source[..byte] {
        if value == b'\n' {
            point.row += 1;
            point.column = 0;
        } else {
            point.column += 1;
        }
    }
    point
}

fn assert_equivalent(left: Node<'_>, right: Node<'_>) {
    let mut pending = vec![(left, right)];
    while let Some((left, right)) = pending.pop() {
        assert_eq!(left.kind(), right.kind());
        assert_eq!(left.start_byte(), right.start_byte());
        assert_eq!(left.end_byte(), right.end_byte());
        assert_eq!(left.start_position(), right.start_position());
        assert_eq!(left.end_position(), right.end_position());
        assert_eq!(left.is_named(), right.is_named());
        assert_eq!(left.is_error(), right.is_error());
        assert_eq!(left.is_missing(), right.is_missing());
        // Tree-sitter may attach skipped trivia to different descendants inside the same ERROR
        // range when an invalid editing intermediate is reparsed incrementally. The ERROR node
        // itself remains the stable recovery boundary; outside it, keep the full child/field/range
        // comparison so scanner or reuse drift cannot be hidden.
        if left.is_error() {
            continue;
        }
        assert_eq!(left.child_count(), right.child_count());
        for index in 0..left.child_count() {
            let index = u32::try_from(index).expect("child index fits u32");
            assert_eq!(
                left.field_name_for_child(index),
                right.field_name_for_child(index),
                "field name diverged under {}",
                left.kind()
            );
            pending.push((
                left.child(index).expect("left child"),
                right.child(index).expect("right child"),
            ));
        }
    }
}

fn split_input(data: &[u8]) -> Option<(Vec<u8>, &[u8])> {
    if let Some(source) = data.strip_prefix(REGRESSION_SEED_PREFIX) {
        return (source.len() <= MAX_SOURCE_BYTES)
            .then(|| (source.to_vec(), REGRESSION_OPERATIONS));
    }
    if data.len() < 2 {
        return None;
    }
    let declared = usize::from(u16::from_le_bytes([data[0], data[1]]));
    let source_length = declared.min(MAX_SOURCE_BYTES).min(data.len() - 2);
    Some((
        data[2..2 + source_length].to_vec(),
        &data[2 + source_length..],
    ))
}

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }
    let Some((mut source, operations)) = split_input(data) else {
        return;
    };

    let mut incremental_parser = parser();
    let mut old_tree = incremental_parser
        .parse(&source, None)
        .expect("initial parse");
    let (operation_chunks, _) = operations.as_chunks::<OPERATION_BYTES>();
    for operation in operation_chunks.iter().take(MAX_EDITS) {
        let position_seed = usize::from(u16::from_le_bytes([operation[0], operation[1]]));
        let start = position_seed % (source.len() + 1);
        let delete = usize::from(operation[2]).min(source.len() - start);
        let replacement_length = usize::from(operation[3]) % 5;
        let replacement = &operation[4..4 + replacement_length];
        let end = start + delete;

        let start_position = point_at(&source, start);
        let old_end_position = point_at(&source, end);
        source.splice(start..end, replacement.iter().copied());
        let edit = InputEdit {
            start_byte: start,
            old_end_byte: end,
            new_end_byte: start + replacement.len(),
            start_position,
            old_end_position,
            new_end_position: point_at(&source, start + replacement.len()),
        };
        old_tree.edit(&edit);
        let incremental = incremental_parser
            .parse(&source, Some(&old_tree))
            .expect("incremental parse");
        let mut fresh_parser = parser();
        let fresh = fresh_parser.parse(&source, None).expect("fresh parse");
        assert_equivalent(incremental.root_node(), fresh.root_node());
        old_tree = incremental;
    }
});

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

fn edit_source(source: &mut Vec<u8>, start: usize, end: usize, replacement: &[u8]) -> InputEdit {
    let start_position = point_at(source, start);
    let old_end_position = point_at(source, end);
    source.splice(start..end, replacement.iter().copied());
    InputEdit {
        start_byte: start,
        old_end_byte: end,
        new_end_byte: start + replacement.len(),
        start_position,
        old_end_position,
        new_end_position: point_at(source, start + replacement.len()),
    }
}

// Align with Tree-sitter v0.27.1: crates/cli/src/fuzz.rs and crates/cli/src/fuzz/corpus_test.rs.
fn assert_geometry(root: Node<'_>, source: &[u8]) {
    let mut line_offsets = vec![0];
    for (index, byte) in source.iter().enumerate() {
        if *byte == b'\n' {
            line_offsets.push(index + 1);
        }
    }

    let mut pending = vec![root];
    let mut cursor = root.walk();
    while let Some(node) = pending.pop() {
        assert!(node.start_byte() <= node.end_byte(), "reversed node range");
        assert!(node.end_byte() <= source.len(), "node extends past source");
        for (byte, point) in [
            (node.start_byte(), node.start_position()),
            (node.end_byte(), node.end_position()),
        ] {
            let line_start = *line_offsets.get(point.row).expect("point row exists");
            assert!(byte >= line_start, "point precedes its line");
            assert_eq!(point.column, byte - line_start, "point column matches byte");
            if let Some(next_line) = line_offsets.get(point.row + 1) {
                assert!(byte < *next_line, "point crosses a line ending");
            }
        }

        let mut previous_end = node.start_byte();
        let mut previous_point = node.start_position();
        let mut children = 0;
        let mut named_children = 0;
        // Cursor iteration visits each edge once, including wide recovery trees.
        for child in node.children(&mut cursor) {
            assert!(child.start_byte() >= previous_end, "overlapping children");
            assert!(child.start_position() >= previous_point);
            assert!(child.end_byte() <= node.end_byte(), "child outside parent");
            assert!(child.end_position() <= node.end_position());
            previous_end = child.end_byte();
            previous_point = child.end_position();
            children += 1;
            named_children += usize::from(child.is_named());
            pending.push(child);
        }
        assert_eq!(children, node.child_count());
        assert_eq!(named_children, node.named_child_count());
    }
}

fn assert_equivalent(left: Node<'_>, right: Node<'_>, source: &[u8]) {
    assert_geometry(left, source);
    assert_geometry(right, source);
    assert_eq!(
        left.has_error(),
        right.has_error(),
        "parse validity diverged"
    );
    // Recovery can choose a different CST. Reused recovery tokens can also retain leading
    // extras after the document becomes valid, so cross-tree ranges are not an invariant.
    // Check geometry for every node and compare valid CST structure, as in upstream fuzzing.
    if left.has_error() {
        return;
    }
    let mut pending = vec![(left, right)];
    let mut left_cursor = left.walk();
    let mut right_cursor = right.walk();
    while let Some((left, right)) = pending.pop() {
        assert_eq!(left.kind(), right.kind());
        assert_eq!(left.is_named(), right.is_named());
        assert_eq!(left.is_extra(), right.is_extra());
        assert_eq!(left.is_error(), right.is_error());
        assert_eq!(left.is_missing(), right.is_missing());
        assert_eq!(left.has_error(), right.has_error());
        assert_eq!(left.child_count(), right.child_count());
        left_cursor.reset(left);
        right_cursor.reset(right);
        let mut left_has_child = left_cursor.goto_first_child();
        let mut right_has_child = right_cursor.goto_first_child();
        loop {
            assert_eq!(left_has_child, right_has_child);
            if !left_has_child {
                break;
            }
            assert_eq!(
                left_cursor.field_name(),
                right_cursor.field_name(),
                "field name diverged under {}",
                left.kind()
            );
            pending.push((left_cursor.node(), right_cursor.node()));
            left_has_child = left_cursor.goto_next_sibling();
            right_has_child = right_cursor.goto_next_sibling();
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

pub fn assert_edit_sequence(data: &[u8]) {
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
    assert_geometry(old_tree.root_node(), &source);
    let initial_source = source.clone();
    let initial_tree = old_tree.clone();
    let mut undo = Vec::new();
    let (operation_chunks, _) = operations.as_chunks::<OPERATION_BYTES>();
    for operation in operation_chunks.iter().take(MAX_EDITS) {
        let position_seed = usize::from(u16::from_le_bytes([operation[0], operation[1]]));
        let start = position_seed % (source.len() + 1);
        let delete = usize::from(operation[2]).min(source.len() - start);
        let replacement_length = usize::from(operation[3]) % 5;
        let replacement = &operation[4..4 + replacement_length];
        undo.push((
            start,
            start + replacement.len(),
            source[start..start + delete].to_vec(),
        ));
        let edit = edit_source(&mut source, start, start + delete, replacement);
        old_tree.edit(&edit);
        let incremental = incremental_parser
            .parse(&source, Some(&old_tree))
            .expect("incremental parse");
        // Tolerant grammar nodes can represent malformed input without has_error().
        // As in upstream fuzzing, edited intermediates need consistent geometry;
        // compare CST output only after undoing all edits to the original input.
        assert_geometry(incremental.root_node(), &source);
        old_tree = incremental;
    }

    if undo.is_empty() {
        return;
    }
    while let Some((start, end, replacement)) = undo.pop() {
        old_tree.edit(&edit_source(&mut source, start, end, &replacement));
    }
    assert_eq!(source, initial_source, "inverse edits must restore source");
    let restored = incremental_parser
        .parse(&source, Some(&old_tree))
        .expect("restored incremental parse");
    assert_geometry(restored.root_node(), &source);
    if !initial_tree.root_node().has_error() {
        assert_equivalent(restored.root_node(), initial_tree.root_node(), &source);
    }
}

#[cfg(test)]
#[path = "tree_sitter_edits_tests.rs"]
mod tests;

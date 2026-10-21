use super::*;

use std::panic::{AssertUnwindSafe, catch_unwind};
use tree_sitter::Tree;

fn edited_pair(
    initial: &[u8],
    start: usize,
    end: usize,
    replacement: &[u8],
) -> (Vec<u8>, Tree, Tree) {
    let mut source = initial.to_vec();
    let mut incremental_parser = parser();
    let mut old = incremental_parser
        .parse(&source, None)
        .expect("initial tree");
    old.edit(&edit_source(&mut source, start, end, replacement));
    let incremental = incremental_parser
        .parse(&source, Some(&old))
        .expect("incremental tree");
    let fresh = parser().parse(&source, None).expect("fresh tree");
    (source, incremental, fresh)
}

fn keyword(tree: &Tree) -> Node<'_> {
    let root = tree.root_node();
    (0..root.child_count())
        .filter_map(|index| root.child(index).unwrap().child_by_field_name("header"))
        .find_map(|header| header.child_by_field_name("keyword"))
        .expect("diagram keyword")
}

fn assert_strict_ranges(left: Node<'_>, right: Node<'_>) {
    let mut pending = vec![(left, right)];
    while let Some((left, right)) = pending.pop() {
        assert_eq!(left.range(), right.range(), "{}", left.kind());
        assert_eq!(left.child_count(), right.child_count());
        for index in 0..left.child_count() {
            pending.push((left.child(index).unwrap(), right.child(index).unwrap()));
        }
    }
}

fn assert_rejected(left: Node<'_>, right: Node<'_>, source: &[u8]) {
    assert!(
        catch_unwind(AssertUnwindSafe(|| assert_equivalent(left, right, source))).is_err(),
        "the comparison must reject this difference"
    );
}

#[test]
fn real_header_recovery_range_differences_are_accepted() {
    for initial in [b"C mindmap\n".as_slice(), b"C sequenceDiagram\n"] {
        let (source, incremental, fresh) = edited_pair(initial, 0, 0, b" ");
        assert!(incremental.root_node().has_error());
        assert!(fresh.root_node().has_error());
        assert_eq!(keyword(&incremental).start_byte(), 3);
        assert_eq!(keyword(&fresh).start_byte(), 2);
        assert_equivalent(incremental.root_node(), fresh.root_node(), &source);
        assert_equivalent(fresh.root_node(), incremental.root_node(), &source);
    }
}

#[test]
fn no_newline_error_keyword_regression_is_accepted() {
    let (source, operations) = split_input(include_bytes!(
        "../seeds/tree-sitter-edits/08_error_keyword_trivia.bin"
    ))
    .unwrap();
    assert_eq!(source, b"d mindmap");
    assert_eq!(operations, &[0; 8]);
    let (source, incremental, fresh) = edited_pair(&source, 0, 0, b"");
    let left_error = incremental.root_node().child(0).unwrap();
    let right_error = fresh.root_node().child(0).unwrap();
    assert!(left_error.is_error() && right_error.is_error());
    assert_eq!(left_error.range(), right_error.range());
    assert_eq!(left_error.child(0).unwrap().byte_range(), 2..9);
    assert_eq!(right_error.child(0).unwrap().byte_range(), 1..9);
    assert_equivalent(incremental.root_node(), fresh.root_node(), &source);
    assert_edit_sequence(include_bytes!(
        "../seeds/tree-sitter-edits/08_error_keyword_trivia.bin"
    ));
}

#[test]
fn third_edit_recovery_checks_both_trees_without_requiring_identical_ranges() {
    for (seed, expected) in [
        (
            include_bytes!("../seeds/tree-sitter-edits/07_original_header_crash.bin").as_slice(),
            (28, 27),
        ),
        (
            include_bytes!("../seeds/tree-sitter-edits/05_third_edit_header.txt").as_slice(),
            (13, 12),
        ),
    ] {
        let (mut source, operations) = split_input(seed).unwrap();
        let mut incremental_parser = parser();
        let mut old = incremental_parser.parse(&source, None).unwrap();
        for (index, operation) in operations.as_chunks::<OPERATION_BYTES>().0[..3]
            .iter()
            .enumerate()
        {
            let start = usize::from(u16::from_le_bytes([operation[0], operation[1]]));
            let end = start + usize::from(operation[2]);
            let replacement = &operation[4..4 + usize::from(operation[3])];
            old.edit(&edit_source(&mut source, start, end, replacement));
            let incremental = incremental_parser.parse(&source, Some(&old)).unwrap();
            let fresh = parser().parse(&source, None).unwrap();
            if index == 2 {
                assert_eq!(
                    (
                        keyword(&incremental).start_byte(),
                        keyword(&fresh).start_byte()
                    ),
                    expected
                );
            }
            assert_equivalent(incremental.root_node(), fresh.root_node(), &source);
            old = incremental;
        }
        assert_edit_sequence(seed);
    }
}

#[test]
fn invalid_tree_ranges_are_rejected_even_when_both_error_trees_match() {
    let source = b"d mindmap";
    let mut altered = parser().parse(source, None).unwrap();
    let mut changed = source.to_vec();
    altered.edit(&edit_source(&mut changed, 2, 2, b" "));
    assert!(altered.root_node().has_error());
    assert!(altered.root_node().end_byte() > source.len());
    assert_rejected(altered.root_node(), altered.root_node(), source);
}

#[test]
fn invalid_point_rows_and_columns_are_rejected_in_error_trees() {
    let source = b"d mindmap";
    for new_end_position in [Point::new(1, 0), Point::new(0, 1)] {
        let mut altered = parser().parse(source, None).unwrap();
        altered.edit(&InputEdit {
            start_byte: 0,
            old_end_byte: 0,
            new_end_byte: 0,
            start_position: Point::new(0, 0),
            old_end_position: Point::new(0, 0),
            new_end_position,
        });
        assert!(altered.root_node().has_error());
        assert_rejected(altered.root_node(), altered.root_node(), source);
    }
}

#[test]
fn points_cannot_use_a_previous_line_with_an_oversized_column() {
    let source = b"d mindmap\n";
    let mut altered = parser().parse(source, None).unwrap();
    let mut changed = source.to_vec();
    altered.edit(&edit_source(&mut changed, 9, 10, b" "));
    assert_eq!(altered.root_node().end_byte(), source.len());
    assert_eq!(altered.root_node().end_position(), Point::new(0, 10));
    assert_rejected(altered.root_node(), altered.root_node(), source);
}

#[test]
fn byte_points_support_utf8_crlf_and_invalid_utf8() {
    for source in [
        "mindmap\r\n根\r\n  子\r\n".as_bytes(),
        b"sequenceDiagram\r\nA->>B: Hi\r\n",
        b"C\xff mindmap\n\0\x80",
        b"",
    ] {
        let tree = parser().parse(source, None).unwrap();
        assert_geometry(tree.root_node(), source);
        assert_equivalent(tree.root_node(), tree.root_node(), source);
    }
}

#[test]
fn validity_disagreement_is_rejected_in_both_directions() {
    let source = b"mindmap\nRoot\n";
    let valid = parser().parse(source, None).unwrap();
    let invalid = parser().parse(b"xindmap\nRoot\n", None).unwrap();
    assert!(!valid.root_node().has_error());
    assert!(invalid.root_node().has_error());
    assert_geometry(invalid.root_node(), source);
    assert_rejected(valid.root_node(), invalid.root_node(), source);
    assert_rejected(invalid.root_node(), valid.root_node(), source);
}

#[test]
fn valid_body_structure_is_still_compared_strictly() {
    for (source, other_source) in [
        (
            b"mindmap\nRoot\n  Child\n".as_slice(),
            b"mindmap\nRoot\n       \n".as_slice(),
        ),
        (
            b"flowchart LR\nA[x]\n".as_slice(),
            b"flowchart LR\nA(x)\n".as_slice(),
        ),
    ] {
        let left = parser().parse(source, None).unwrap();
        let right = parser().parse(other_source, None).unwrap();
        assert!(!left.root_node().has_error());
        assert!(!right.root_node().has_error());
        assert_geometry(left.root_node(), source);
        assert_geometry(right.root_node(), source);
        assert_eq!(left.root_node().range(), right.root_node().range());
        assert_rejected(left.root_node(), right.root_node(), source);
    }
}

#[test]
fn recovered_valid_tokens_can_retain_leading_extras() {
    let initial = b"C sequenceDiagram\nA->>B: Hi\n";
    let (source, incremental, fresh) = edited_pair(initial, 0, 1, b"\n");
    assert!(!incremental.root_node().has_error());
    assert!(!fresh.root_node().has_error());
    assert_eq!(keyword(&incremental).byte_range(), 1..17);
    assert_eq!(keyword(&fresh).byte_range(), 2..17);
    assert_equivalent(incremental.root_node(), fresh.root_node(), &source);
    assert_edit_sequence(include_bytes!(
        "../seeds/tree-sitter-edits/09_recovered_valid_sequence.bin"
    ));
}

#[test]
fn restoring_valid_documents_restores_exact_cst_ranges() {
    for initial in [
        b"C mindmap\nRoot\n  Child\n".as_slice(),
        b"C sequenceDiagram\nAlice->>Bob: Hello\n",
    ] {
        let (mut source, mut incremental, fresh) = edited_pair(initial, 0, 0, b" ");
        assert_equivalent(incremental.root_node(), fresh.root_node(), &source);
        incremental.edit(&edit_source(&mut source, 0, 3, b""));
        let restored = parser().parse(&source, Some(&incremental)).unwrap();
        let fresh = parser().parse(&source, None).unwrap();
        assert!(!restored.root_node().has_error());
        assert!(!fresh.root_node().has_error());
        assert_equivalent(restored.root_node(), fresh.root_node(), &source);
        assert_strict_ranges(restored.root_node(), fresh.root_node());
    }
}

#[test]
fn all_seed_files_replay_the_actual_fuzz_contract() {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("seeds/tree-sitter-edits");
    for entry in std::fs::read_dir(directory).expect("seed directory") {
        let path = entry.expect("seed entry").path();
        if path.is_file() {
            eprintln!("replaying {}", path.display());
            assert_edit_sequence(&std::fs::read(path).expect("seed bytes"));
        }
    }
}

#[test]
fn valid_to_valid_edits_are_undone_before_comparing_cst() {
    let initial = b"sequenceDiagram\nAlice->>Bob: Hello\n";
    let (source, incremental, fresh) = edited_pair(initial, 32, 33, b"!");
    assert!(!incremental.root_node().has_error());
    assert!(!fresh.root_node().has_error());
    assert_ne!(source, initial);
    let mut data = u16::try_from(initial.len()).unwrap().to_le_bytes().to_vec();
    data.extend_from_slice(initial);
    data.extend_from_slice(&[32, 0, 1, 1, b'!', 0, 0, 0]);
    assert_edit_sequence(&data);
}

#[test]
fn binary_operations_cover_valid_invalid_valid_transitions() {
    let initial = b"sequenceDiagram\nAlice->>Bob: Hello\n";
    let mut data = u16::try_from(initial.len()).unwrap().to_le_bytes().to_vec();
    data.extend_from_slice(initial);
    data.extend_from_slice(&[0, 0, 1, 0, 0, 0, 0, 0]);
    assert_edit_sequence(&data);
    data.extend_from_slice(&[0, 0, 0, 1, b's', 0, 0, 0]);
    assert_edit_sequence(&data);
}

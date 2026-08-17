use std::{cell::Cell, ops::ControlFlow};

use tree_sitter::{InputEdit, Language, ParseOptions, Parser, Point};
use tree_sitter_mermaid::LANGUAGE;

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    kind: String,
    error: bool,
    missing: bool,
    start_byte: usize,
    end_byte: usize,
    start_position: Point,
    end_position: Point,
    field: Option<String>,
    children: Vec<Snapshot>,
}

fn new_parser() -> Parser {
    let language: Language = LANGUAGE.into();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .expect("generated Mermaid language must load");
    parser
}

fn snapshot(node: tree_sitter::Node<'_>, field: Option<&str>) -> Snapshot {
    let mut cursor = node.walk();
    let children = node
        .children(&mut cursor)
        .enumerate()
        .filter(|(_, child)| child.is_named())
        .map(|(index, child)| {
            let index = u32::try_from(index).expect("child index fits u32");
            snapshot(child, node.field_name_for_child(index))
        })
        .collect();
    Snapshot {
        kind: node.kind().to_owned(),
        error: node.is_error(),
        missing: node.is_missing(),
        start_byte: node.start_byte(),
        end_byte: node.end_byte(),
        start_position: node.start_position(),
        end_position: node.end_position(),
        field: field.map(str::to_owned),
        children,
    }
}

fn point_at(source: &[u8], byte: usize) -> Point {
    let mut point = Point::new(0, 0);
    for value in &source[..byte] {
        if *value == b'\n' {
            point.row += 1;
            point.column = 0;
        } else {
            point.column += 1;
        }
    }
    point
}

fn replace_and_compare(source: &[u8], start: usize, end: usize, replacement: &[u8]) {
    let mut parser = new_parser();
    let mut old_tree = parser
        .parse(source, None)
        .expect("initial parse must succeed");
    let mut edited = source.to_vec();
    edited.splice(start..end, replacement.iter().copied());

    old_tree.edit(&InputEdit {
        start_byte: start,
        old_end_byte: end,
        new_end_byte: start + replacement.len(),
        start_position: point_at(source, start),
        old_end_position: point_at(source, end),
        new_end_position: point_at(&edited, start + replacement.len()),
    });

    let incremental = parser
        .parse(&edited, Some(&old_tree))
        .expect("incremental parse must produce a tree");
    let fresh = new_parser()
        .parse(&edited, None)
        .expect("fresh parse must produce a tree");
    assert_eq!(
        snapshot(incremental.root_node(), None),
        snapshot(fresh.root_node(), None),
        "incremental and fresh trees diverged after edit at byte {start}"
    );
}

#[test]
fn family_header_replacement_matches_a_fresh_parse() {
    let source = b"flowchart TD\nA --> B\n";
    replace_and_compare(source, 0, b"flowchart TD".len(), b"sequenceDiagram");

    let source = b"mindmap\nRoot\n  Child\n";
    replace_and_compare(source, 0, b"mindmap".len(), b"treeView-beta");
}

#[test]
fn ordinary_and_scanner_edits_match_fresh_parses() {
    let flowchart = b"flowchart TD\nA[Start] --> B[Finish]\n";
    let start = flowchart
        .windows(b"Finish".len())
        .position(|window| window == b"Finish")
        .expect("fixture contains the edited label");
    replace_and_compare(flowchart, start, start + b"Finish".len(), "结束".as_bytes());

    let mindmap = b"mindmap\nRoot\n    Branch\n      Leaf\n";
    let indentation = mindmap
        .windows(b"    Branch".len())
        .position(|window| window == b"    Branch")
        .expect("fixture contains the edited indentation");
    replace_and_compare(mindmap, indentation, indentation + 4, b"  ");
}

#[test]
fn cancellation_after_scanner_state_keeps_the_parser_reusable() {
    let mut source = b"mindmap\nRoot\n".to_vec();
    for index in 0..2_048 {
        source.extend_from_slice(format!("  Branch {index}\n    Leaf {index}\n").as_bytes());
    }
    let cancellation_offset = source.len() / 4;

    let mut parser = new_parser();
    let mut read = |offset: usize, _position: Point| source.get(offset..).unwrap_or_default();
    let cancel = Cell::new(true);
    let cancelled_at = Cell::new(0_usize);
    let mut progress = |state: &tree_sitter::ParseState| {
        if cancel.get() && state.current_byte_offset() >= cancellation_offset {
            cancelled_at.set(state.current_byte_offset());
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    };
    let mut options = ParseOptions::new().progress_callback(&mut progress);

    assert!(
        parser
            .parse_with_options(&mut read, None, Some(options.reborrow()))
            .is_none(),
        "the first parse must be cancelled"
    );
    assert!(cancelled_at.get() >= cancellation_offset);

    cancel.set(false);
    let resumed = parser
        .parse_with_options(&mut read, None, Some(options.reborrow()))
        .expect("a cancelled parser must remain reusable");
    let fresh = new_parser()
        .parse(&source, None)
        .expect("fresh parse must produce a tree");
    assert_eq!(
        snapshot(resumed.root_node(), None),
        snapshot(fresh.root_node(), None)
    );
}

#[test]
fn invalid_utf8_is_bounded_and_repeatable() {
    let source = b"mindmap\nRoot\n\xff\xff\xff\n";
    let mut parser = new_parser();
    let first = parser
        .parse(source, None)
        .expect("parse must return a tree");
    let second = parser
        .parse(source, None)
        .expect("parser must remain reusable");
    assert_eq!(
        snapshot(first.root_node(), None),
        snapshot(second.root_node(), None)
    );
    assert!(first.root_node().end_byte() <= source.len());
}

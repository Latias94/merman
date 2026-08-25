#![no_main]

use libfuzzer_sys::fuzz_target;
use tree_sitter::{Language, Node, Parser};
use tree_sitter_mermaid::LANGUAGE;

const MAX_INPUT_BYTES: usize = 256 * 1024;

fn parser() -> Parser {
    let language: Language = LANGUAGE.into();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .expect("generated Mermaid language must load");
    parser
}

fn assert_equivalent(left: Node<'_>, right: Node<'_>) {
    let mut pending = vec![(left, right)];
    while let Some((left, right)) = pending.pop() {
        assert_eq!(left.kind(), right.kind());
        assert_eq!(left.start_byte(), right.start_byte());
        assert_eq!(left.end_byte(), right.end_byte());
        assert_eq!(left.start_position(), right.start_position());
        assert_eq!(left.end_position(), right.end_position());
        assert_eq!(left.is_error(), right.is_error());
        assert_eq!(left.is_missing(), right.is_missing());
        assert_eq!(left.named_child_count(), right.named_child_count());
        for index in 0..left.named_child_count() {
            let index = u32::try_from(index).expect("named child index fits u32");
            pending.push((
                left.named_child(index).expect("left named child"),
                right.named_child(index).expect("right named child"),
            ));
        }
    }
}

fn assert_spans(root: Node<'_>, input_length: usize) {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        assert!(node.start_byte() <= node.end_byte());
        assert!(node.end_byte() <= input_length);
        for index in 0..node.named_child_count() {
            let index = u32::try_from(index).expect("named child index fits u32");
            let child = node.named_child(index).expect("named child");
            assert!(child.start_byte() >= node.start_byte());
            assert!(child.end_byte() <= node.end_byte());
            pending.push(child);
        }
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_INPUT_BYTES {
        return;
    }
    let mut parser = parser();
    let first = parser.parse(data, None).expect("fresh parse");
    parser.reset();
    let second = parser.parse(data, None).expect("repeat fresh parse");
    assert_spans(first.root_node(), data.len());
    assert_equivalent(first.root_node(), second.root_node());
});

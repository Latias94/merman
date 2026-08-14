use tree_sitter::{Language, Node, Parser};

use tree_sitter_mermaid::LANGUAGE;

fn parser() -> Parser {
    let language: Language = LANGUAGE.into();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .expect("generated Mermaid language must load");
    parser
}

fn assert_bounded(root: Node<'_>, source_length: usize) {
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        assert!(node.start_byte() <= node.end_byte());
        assert!(node.end_byte() <= source_length);
        let mut cursor = node.walk();
        pending.extend(node.children(&mut cursor));
    }
}

#[test]
fn outer_inputs_distinguish_valid_preamble_from_unknown_headers() {
    let valid = [
        b"".as_slice(),
        b"\xef\xbb\xbfflowchart TD\nA --> B\n".as_slice(),
        b"---\ntitle: Sample\n---\n%%{init: {}}%%\n%% comment\nflowchart TD\nA --> B\n".as_slice(),
        b"architecture-beta\n".as_slice(),
    ];
    for source in valid {
        let mut parser = parser();
        let tree = parser
            .parse(source, None)
            .expect("parse must return a tree");
        assert!(!tree.root_node().has_error(), "{source:?}");
        assert_bounded(tree.root_node(), source.len());
    }

    let source = b"notAMermaidFamily\nbody\n";
    let mut parser = parser();
    let tree = parser
        .parse(source, None)
        .expect("parse must return a tree");
    assert!(tree.root_node().has_error());
    assert!(!tree.root_node().to_sexp().contains("_diagram"));
    assert_bounded(tree.root_node(), source.len());
}

#[test]
fn comment_frontmatter_and_directive_only_documents_are_valid_outer_inputs() {
    let cases = [
        b"%% comment only".as_slice(),
        b"%% first\r\n%% second\r\n".as_slice(),
        "---\r\ntitle: 仅元数据\r\n---".as_bytes(),
        b"%%{init: {\"theme\": \"dark\"}}%%".as_slice(),
        b"---\ntitle: Combined\n---\n%%{config: {}}%%\n%% trailing comment\n".as_slice(),
    ];

    for source in cases {
        let mut parser = parser();
        let tree = parser
            .parse(source, None)
            .expect("preamble-only input must return a tree");
        let root = tree.root_node();
        assert!(!root.has_error(), "{source:?}");
        assert!(!root.to_sexp().contains("_diagram"));
        assert_bounded(root, source.len());
    }
}

#[test]
fn unclosed_family_constructs_recover_inside_the_selected_root() {
    let cases = [
        (
            b"eventmodeling\ndata Payload `json` {\n  {\"id\": 1}\n".as_slice(),
            "event_modeling_diagram",
        ),
        (
            b"sankey\n\"Source\nline,Target,1\n".as_slice(),
            "sankey_diagram",
        ),
        (
            b"zenuml\nif (ready) {\n  Alice.method()\n".as_slice(),
            "zenuml_diagram",
        ),
    ];

    for (source, expected_root) in cases {
        let mut parser = parser();
        let tree = parser
            .parse(source, None)
            .expect("parse must return a tree");
        let root = tree.root_node();
        assert!(root.has_error(), "{expected_root} must expose recovery");
        assert!(root.to_sexp().contains(expected_root));
        assert_bounded(root, source.len());
    }
}

#[test]
fn deep_indentation_and_long_lines_remain_bounded() {
    let mut deep = b"mindmap\nRoot\n".to_vec();
    for depth in 1..=300 {
        deep.extend(std::iter::repeat_n(b' ', depth));
        deep.extend_from_slice(b"Node\n");
    }

    let mut long = b"flowchart TD\nA[".to_vec();
    long.extend(std::iter::repeat_n(b'x', 256 * 1024));
    long.extend_from_slice(b"\n");

    for (source, expected_root) in [
        (deep.as_slice(), "mindmap_diagram"),
        (long.as_slice(), "flowchart_diagram"),
    ] {
        let mut parser = parser();
        let tree = parser
            .parse(source, None)
            .expect("parse must return a tree");
        let root = tree.root_node();
        assert!(root.to_sexp().contains(expected_root));
        assert_bounded(root, source.len());
    }
}

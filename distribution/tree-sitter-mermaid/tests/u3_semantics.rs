use tree_sitter::{Language, Node, Parser, Tree};

use tree_sitter_mermaid::LANGUAGE;

fn parse(source: &str) -> Tree {
    let language: Language = LANGUAGE.into();
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .expect("generated Mermaid language must load");
    parser
        .parse(source, None)
        .expect("parse must return a tree")
}

fn count_kind(node: Node<'_>, expected: &str) -> usize {
    let mut count = usize::from(node.kind() == expected);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        count += count_kind(child, expected);
    }
    count
}

fn assert_clean(source: &str) -> Tree {
    let tree = parse(source);
    assert!(
        !tree.root_node().has_error(),
        "unexpected error for {source:?}: {}",
        tree.root_node().to_sexp()
    );
    tree
}

fn assert_error(source: &str) {
    let tree = parse(source);
    assert!(
        tree.root_node().has_error(),
        "expected an error for {source:?}: {}",
        tree.root_node().to_sexp()
    );
}

#[test]
fn architecture_boundaries_and_reserved_identifiers_remain_explicit() {
    assert_clean("architecture-beta  ");
    let adjacent = assert_clean("architecture-beta%% comment\nservice api(server)[API]\n");
    assert_eq!(
        count_kind(adjacent.root_node(), "architecture_service_statement"),
        1
    );

    for reserved in ["align", "row", "column"] {
        let tree = assert_clean(&format!(
            "architecture-beta\nservice {reserved}(server)[Service]\n"
        ));
        assert_eq!(
            count_kind(
                tree.root_node(),
                "architecture_malformed_reserved_identifier"
            ),
            1,
            "{reserved}"
        );
    }

    assert_error("architecture-beta\nservice api( server )[ \"API\" ]\n");
}

#[test]
fn langium_family_recovery_stops_before_the_next_physical_line() {
    let cynefin = assert_clean(
        "cynefin-beta\ncomplex \"Before\"\ncomplex -->\nclear \"After\"\nchaotic --> complex\n",
    );
    assert_eq!(
        count_kind(
            cynefin.root_node(),
            "cynefin_incomplete_transition_statement"
        ),
        1
    );
    assert_eq!(count_kind(cynefin.root_node(), "cynefin_domain_block"), 2);

    let cynefin_quote =
        assert_clean("cynefin-beta\ncomplex \"broken\nclear \"After\"\nchaotic --> complex\n");
    assert_eq!(
        count_kind(cynefin_quote.root_node(), "langium_unclosed_string"),
        1
    );
    assert_eq!(
        count_kind(cynefin_quote.root_node(), "cynefin_domain_block"),
        2
    );
    assert_error("cynefin-beta\ncomplex --> clear chaotic --> complex\n");

    let info = assert_clean("info\ntitle Before\naccDescr {\nbroken\naccTitle: After\n");
    assert_eq!(
        count_kind(info.root_node(), "langium_unclosed_acc_descr_block"),
        1
    );
    assert_eq!(count_kind(info.root_node(), "info_recovery_statement"), 1);
    assert_eq!(
        count_kind(info.root_node(), "langium_acc_title_statement"),
        1
    );
    assert_error("info\naccDescr {} title Next\n");
}

#[test]
fn git_graph_keywords_clauses_and_unclosed_strings_recover_locally() {
    for source in [
        "gitGraph\nbranchmain\n",
        "gitGraph\nmergefeature\n",
        "gitGraph\ncommitid:\"x\"\n",
    ] {
        let tree = assert_clean(source);
        assert_eq!(
            count_kind(tree.root_node(), "git_graph_malformed_statement"),
            1,
            "{source:?}"
        );
    }

    let clause = assert_clean("gitGraph\ncommit id:\"2\" oops:\"ignored\"\nbranch after\n");
    assert_eq!(
        count_kind(clause.root_node(), "git_graph_malformed_clause"),
        1
    );
    assert_eq!(
        count_kind(clause.root_node(), "git_graph_branch_statement"),
        1
    );

    let quote = assert_clean("gitGraph\ncommit msg:\"oops\nbranch main\n");
    assert_eq!(count_kind(quote.root_node(), "langium_unclosed_string"), 1);
    assert_eq!(
        count_kind(quote.root_node(), "git_graph_branch_statement"),
        1
    );

    let directive = assert_clean("gitGraph\ncommit %%{init: {}}%%\n");
    assert_eq!(count_kind(directive.root_node(), "directive"), 1);
}

#[test]
fn packet_and_pie_require_line_boundaries_without_losing_siblings() {
    let packet = assert_clean("packet\n0: \"bad\n1: \"ok\"\n");
    assert_eq!(count_kind(packet.root_node(), "langium_unclosed_string"), 1);
    assert_eq!(count_kind(packet.root_node(), "packet_block_statement"), 2);
    assert_error("packet\n0:\"A\"1:\"B\"\n");

    let pie = assert_clean("pie\n\"bad\n\"ok\": 1\n");
    assert_eq!(
        count_kind(pie.root_node(), "pie_unclosed_section_statement"),
        1
    );
    assert_eq!(count_kind(pie.root_node(), "pie_section"), 1);
    assert_error("pie\n\"A\":1\"B\":2\n");
}

#[test]
fn radar_boundaries_comments_and_unclosed_labels_are_local() {
    assert_clean("radar-beta  ");
    let quote = assert_clean("radar-beta\naxis A[\"broken\naxis B[\"good\"]\ncurve C{1}\n");
    assert_eq!(
        count_kind(quote.root_node(), "radar_unclosed_quoted_string"),
        1
    );
    assert_eq!(count_kind(quote.root_node(), "radar_axis_statement"), 2);
    assert_eq!(count_kind(quote.root_node(), "radar_curve_statement"), 1);

    let glued = assert_clean("radar-beta\naxisA\n");
    assert_eq!(
        count_kind(glued.root_node(), "radar_malformed_statement"),
        1
    );
    assert_eq!(count_kind(glued.root_node(), "radar_axis_statement"), 0);
    assert_error("radar-beta\naxis \u{901f}\u{5ea6}\n");

    let comments = assert_clean("radar-beta\ntitle Hello %% note\ncurve C{\n1,\n%% nested\n2\n}\n");
    assert_eq!(count_kind(comments.root_node(), "comment"), 2);
}

#[test]
fn wardley_line_contract_and_recovery_keep_following_statements() {
    assert_clean("wardley-beta  ");
    assert_error("wardley-beta\nsize [100,100] component A [0.1,0.2]\n");

    let quote = assert_clean(
        "wardley-beta\ncomponent \"broken [0.1,0.2]\nnote \"good\" [0.3,0.4]\ncomponent B [0.2,0.3]\n",
    );
    assert_eq!(
        count_kind(quote.root_node(), "wardley_unclosed_quoted_string"),
        1
    );
    assert_eq!(count_kind(quote.root_node(), "wardley_note_statement"), 1);
    assert_eq!(
        count_kind(quote.root_node(), "wardley_component_statement"),
        1
    );

    for source in [
        "wardley-beta\n\"A\" B\n",
        "wardley-beta\nA \"B\"\n",
        "wardley-beta\naccDescr\n{\ntext\n}\n",
        "wardley-beta\ncomponent A [0.1,0.2] label [- 10, 5]\n",
    ] {
        assert_clean(source);
    }

    let unicode = assert_clean("wardley-beta\ncomponent \u{6570}\u{636e}\u{5e93} [0.1,0.2]\n");
    assert_eq!(
        count_kind(
            unicode.root_node(),
            "wardley_incomplete_component_statement"
        ),
        1
    );
    assert_eq!(
        count_kind(unicode.root_node(), "wardley_component_statement"),
        0
    );

    assert_error("wardley-beta\npipeline A {\n\n}\n");
}

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

fn assert_no_generic_fallback(node: Node<'_>) {
    const FORBIDDEN: &[&str] = &[
        "catch_all_body",
        "raw_line",
        "unknown_statement",
        "unstructured_body",
        "unstructured_statement",
    ];

    assert!(
        !FORBIDDEN.contains(&node.kind()),
        "generic fallback {} survived in {}",
        node.kind(),
        node.to_sexp()
    );
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        assert_no_generic_fallback(child);
    }
}

fn assert_structured(source: &str, root: &str, required: &[&str]) -> Tree {
    let tree = parse(source);
    assert!(
        !tree.root_node().has_error(),
        "unexpected error for {source:?}: {}",
        tree.root_node().to_sexp()
    );
    assert_eq!(count_kind(tree.root_node(), root), 1, "{source:?}");
    for kind in required {
        assert!(
            count_kind(tree.root_node(), kind) > 0,
            "missing {kind} for {source:?}: {}",
            tree.root_node().to_sexp()
        );
    }
    assert_no_generic_fallback(tree.root_node());
    tree
}

#[test]
fn railroad_dialects_expose_dialect_owned_expression_structures() {
    assert_structured(
        "railroad-beta\nroot = sequence(terminal(\"start\"), optional(nonterminal('item')), choice(terminal(\"a\"), terminal(\"b\")), oneOrMore(special(\"tail\"))) ;\n",
        "railroad_diagram",
        &[
            "railroad_rule",
            "railroad_sequence",
            "railroad_optional",
            "railroad_choice",
            "railroad_repetition",
            "railroad_terminal",
            "railroad_reference",
        ],
    );
    assert_structured(
        "railroad-abnf-beta\nscheme = ALPHA *( ALPHA / DIGIT / \"+\" ) ;\nrule = 1*2\"hello\" / [ other-rule ] / %x41 ;\n",
        "railroad_abnf_diagram",
        &[
            "railroad_abnf_rule",
            "railroad_abnf_alternation",
            "railroad_abnf_concatenation",
            "railroad_abnf_repeat",
            "railroad_abnf_optional_group",
            "railroad_abnf_numeric_value",
        ],
    );
    assert_structured(
        "railroad-ebnf-beta\nexpression = term , ( \"+\" term | \"-\" term )* ;\nterm ::= [ sign ] , { digit | \"_\" } ;\nspecial = ? Unicode 描述 ? ;\n",
        "railroad_ebnf_diagram",
        &[
            "railroad_ebnf_rule",
            "railroad_ebnf_choice",
            "railroad_ebnf_sequence",
            "railroad_ebnf_group",
            "railroad_ebnf_optional_group",
            "railroad_ebnf_repetition_group",
            "railroad_ebnf_special_sequence",
        ],
    );
    assert_structured(
        "railroad-peg-beta\nExpression <- Term ((\"+\" / \"-\") Term)* ;\nTerm <- &\"a\" !\"b\" . other? item+ tail* ;\n",
        "railroad_peg_diagram",
        &[
            "railroad_peg_rule",
            "railroad_peg_ordered_choice",
            "railroad_peg_sequence",
            "railroad_peg_prefix_operator",
            "railroad_peg_suffix_operator",
            "railroad_peg_group",
            "railroad_peg_any",
        ],
    );
}

#[test]
fn railroad_recovery_keeps_following_rules_as_siblings() {
    let cases = [
        (
            "railroad-beta\nbefore = terminal(\"a\") ;\nbroken = ;\nafter = terminal(\"b\") ;\n",
            "railroad_diagram",
            "railroad_rule",
            "railroad_incomplete_rule",
        ),
        (
            "railroad-abnf-beta\nbefore = \"a\" ;\nbroken = ;\nafter = %d48.49 ;\n",
            "railroad_abnf_diagram",
            "railroad_abnf_rule",
            "railroad_abnf_incomplete_rule",
        ),
        (
            "railroad-ebnf-beta\nbefore = \"a\" ;\nbroken ::= ;\nafter = other? ;\n",
            "railroad_ebnf_diagram",
            "railroad_ebnf_rule",
            "railroad_ebnf_incomplete_rule",
        ),
        (
            "railroad-peg-beta\nbefore <- \"a\" ;\nbroken <- ;\nafter <- . ;\n",
            "railroad_peg_diagram",
            "railroad_peg_rule",
            "railroad_peg_incomplete_rule",
        ),
    ];

    for (source, root, rule, recovery) in cases {
        let tree = assert_structured(source, root, &[rule, recovery]);
        assert_eq!(count_kind(tree.root_node(), rule), 2, "{source:?}");
        assert_eq!(count_kind(tree.root_node(), recovery), 1, "{source:?}");
    }
}

#[test]
fn zenuml_exposes_participants_messages_control_blocks_and_expressions() {
    assert_structured(
        "zenuml\ntitle Order 流程\n@Starter(用户)\n@Actor <<boundary>> 用户 as \"Customer\" #4f81bd\nconst Order order = new Order(id, count = 2)\na = A.SyncMessage()\nSomeType result = API.fetch()\nif (ready && count >= 2) {\n  用户->API.create(order, timeout = 250ms)\n} else {\n  API->Queue: accepted\n  Queue-->用户: done\n}\ntry {\n  API.work()\n} catch (Failure error) {\n  API->用户: failed\n} finally {\n  API.cleanup()\n}\n",
        "zenuml_diagram",
        &[
            "zenuml_starter_declaration",
            "zenuml_participant_declaration",
            "zenuml_creation_statement",
            "zenuml_assignment",
            "zenuml_if_statement",
            "zenuml_binary_expression",
            "zenuml_sync_message_statement",
            "zenuml_async_message_statement",
            "zenuml_reply_message_statement",
            "zenuml_try_statement",
            "zenuml_catch_clause",
            "zenuml_finally_clause",
        ],
    );
}

#[test]
fn zenuml_recovery_is_local_and_preserves_following_messages() {
    let tree = assert_structured(
        "zenuml\nBefore->\nnew Order(id, count =\nAlice.call(\"unfinished\nAfter->Before: sibling survives\n",
        "zenuml_diagram",
        &[
            "zenuml_incomplete_message_statement",
            "zenuml_creation_statement",
            "zenuml_unclosed_argument_list",
            "zenuml_unclosed_string",
            "zenuml_async_message_statement",
        ],
    );
    assert_eq!(
        count_kind(tree.root_node(), "zenuml_async_message_statement"),
        1
    );
}

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
fn stateful_and_multiline_families_expose_family_owned_structures() {
    assert_structured(
        "eventmodeling\nentity Sales.Order\ntf 01 cmd Sales.Update ->> 007 [[payload]] `json`{ready: true}\ndata payload `json` {\n  { \"ready\": true }\n}\n",
        "event_modeling_diagram",
        &[
            "event_entity_statement",
            "event_frame_statement",
            "event_inline_data",
            "event_data_statement",
            "event_data_block",
        ],
    );
    assert_structured(
        "kanban\n  todo[Todo]\n    docs[\"Write docs\"]@{ assigned: 'Alice B.', ticket: MM-42 }\n    ::icon(fa fa-book)\n    :::urgent large\n",
        "kanban_diagram",
        &[
            "kanban_item_statement",
            "kanban_metadata",
            "kanban_icon_statement",
            "kanban_class_statement",
        ],
    );
    assert_structured(
        "mindmap\n  root((Map))\n    child[\"`**Bold** and\nUnicode`\"]\n    ::icon(fa fa-book)\n    :::urgent large\n",
        "mindmap_diagram",
        &[
            "mindmap_node_statement",
            "mindmap_circle_shape",
            "mindmap_icon_statement",
            "mindmap_class_statement",
        ],
    );
    assert_structured(
        "sankey-beta\n\"Source, one\",\"Target \"\"quoted\"\"\",\"12\n.5\"\nplain,target,3.25\n",
        "sankey_diagram",
        &[
            "sankey_record",
            "sankey_quoted_field",
            "sankey_escaped_quote",
        ],
    );
    assert_structured(
        "sequenceDiagram\nactor U as User\nparticipant API@{ \"type\": \"boundary\" } as Checkout API\nU->>+API: start\nalt accepted\n  Note over U,API: processing\nelse rejected\n  API-->>-U: complete\nend\n",
        "sequence_diagram",
        &[
            "sequence_participant_declaration",
            "sequence_message_statement",
            "sequence_alt_block",
            "sequence_note_statement",
        ],
    );
    assert_structured(
        "treeView-beta\ntitle Project files\nroot/\n  App.tsx icon(logos:react) :::highlight ## main component\n│   └── README.md\n",
        "tree_view_diagram",
        &[
            "tree_view_node_statement",
            "tree_view_icon_annotation",
            "tree_view_class_annotation",
            "tree_view_description_annotation",
            "tree_view_box_statement",
        ],
    );
    assert_structured(
        "treemap\ntitle Product revenue\n\"Products\":::root\n  \"Phones\": 50:::important\nclassDef important fill:red;\n",
        "treemap_diagram",
        &[
            "treemap_section",
            "treemap_leaf",
            "treemap_number",
            "treemap_class_definition",
        ],
    );
    assert_structured(
        "venn-beta\nset A[\"Frontend\"]:10\nset B\nunion A,B[Shared]:4\n  text \"Implicit\"[\"display\"]\nstyle A,B fill:#ffcc00,stroke-width:2px\n",
        "venn_diagram",
        &[
            "venn_set_statement",
            "venn_union_statement",
            "venn_intersection_expression",
            "venn_indented_text_statement",
            "venn_style_statement",
        ],
    );
}

#[test]
fn localized_recovery_preserves_following_stateful_siblings() {
    let event = assert_structured(
        "eventmodeling\ntf 01 cmd Before\ntf nope broken payload\ntf 02 evt After ->> 01\n",
        "event_modeling_diagram",
        &["event_recovered_statement", "event_frame_statement"],
    );
    assert_eq!(count_kind(event.root_node(), "event_frame_statement"), 2);

    let kanban = assert_structured(
        "kanban\n  before[Before]\n    broken[unterminated\n    after[After]@{ assigned: bob }\n  final[Final]\n",
        "kanban_diagram",
        &["kanban_incomplete_item_statement", "kanban_item_statement"],
    );
    assert_eq!(count_kind(kanban.root_node(), "kanban_item_statement"), 3);

    let mindmap = assert_structured(
        "mindmap\n  before[Before]\n    broken[\"unterminated\n    after[After]\n  final\n",
        "mindmap_diagram",
        &[
            "mindmap_incomplete_node_statement",
            "mindmap_node_statement",
        ],
    );
    assert_eq!(count_kind(mindmap.root_node(), "mindmap_node_statement"), 3);

    let sankey = assert_structured(
        "sankey\nBefore,Middle,1\nTruncated,Only\nbroken row\n\"Unclosed,target,2\nAfter,End,2\n",
        "sankey_diagram",
        &[
            "sankey_incomplete_record",
            "sankey_malformed_record",
            "sankey_unclosed_record",
            "sankey_record",
        ],
    );
    assert_eq!(count_kind(sankey.root_node(), "sankey_record"), 2);

    let sequence = assert_structured(
        "sequenceDiagram\nparticipant Before\nBefore->>\nunsupported editor fragment\nparticipant After\nAfter-->>Before: still parsed\n",
        "sequence_diagram",
        &[
            "sequence_incomplete_message_statement",
            "sequence_malformed_statement",
            "sequence_message_statement",
        ],
    );
    assert_eq!(
        count_kind(sequence.root_node(), "sequence_participant_declaration"),
        2
    );

    let tree_view = assert_structured(
        "treeView-beta\nbefore.txt\n  └──\nafter.txt :::\nfinal.txt icon(\n\"Unclosed label\nsibling.txt\n",
        "tree_view_diagram",
        &[
            "tree_view_incomplete_box_statement",
            "tree_view_incomplete_class_annotation",
            "tree_view_incomplete_icon_annotation",
            "tree_view_unclosed_name",
            "tree_view_node_statement",
        ],
    );
    assert_eq!(
        count_kind(tree_view.root_node(), "tree_view_node_statement"),
        5
    );

    let treemap = assert_structured(
        "treemap-beta\n\"Root\"\n  \"Missing value\":\n  \"Invalid value\": nope\n  \"Unclosed section\n  \"Sibling\": 2\n",
        "treemap_diagram",
        &[
            "treemap_incomplete_leaf",
            "treemap_malformed_leaf",
            "treemap_unclosed_name",
            "treemap_leaf",
        ],
    );
    assert_eq!(count_kind(treemap.root_node(), "treemap_item_statement"), 5);

    let venn = assert_structured(
        "venn-beta\nset Before[ok]\nunion Before,\nbroken payload\nset Label[\"unclosed\nset After\nstyle After fill:#fff\n",
        "venn_diagram",
        &[
            "venn_incomplete_union_statement",
            "venn_malformed_statement",
            "venn_unclosed_quoted_label",
            "venn_set_statement",
            "venn_style_statement",
        ],
    );
    assert_eq!(count_kind(venn.root_node(), "venn_set_statement"), 3);
}

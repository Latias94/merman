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
    tree
}

#[test]
fn graph_families_expose_family_owned_structures() {
    assert_structured(
        "block\ncolumns 3\nblock:group[\"Group\"]:2\n  A[\"Alpha\"] B[\"Beta\"]\nend\nA --> B\nclassDef active fill:#f96\nclass A active\n",
        "block_diagram",
        &[
            "block_composite_statement",
            "block_edge_statement",
            "block_class_definition_statement",
            "block_class_assignment_statement",
        ],
    );
    assert_structured(
        "C4Context\nBoundary(system, \"System\") {\n  Person(user, \"User\")\n  System(app, \"App\")\n}\nRel(user, app, \"Uses\")\nUpdateElementStyle(app, $bgColor=\"blue\")\n",
        "c4_diagram",
        &[
            "c4_boundary_statement",
            "c4_entity_declaration",
            "c4_relationship_statement",
            "c4_style_update_statement",
        ],
    );
    assert_structured(
        "classDiagram\nnamespace domain {\n  class Customer {\n    +String name\n  }\n}\nCustomer \"1\" o-- \"many\" Order : places\n",
        "class_diagram",
        &[
            "class_namespace_declaration",
            "class_declaration",
            "class_member",
            "class_relationship",
        ],
    );
    assert_structured(
        "erDiagram\nCUSTOMER[\"Customer\"] {\n  string id PK\n  }\nORDER {\n  uuid id PK\n  }\nCUSTOMER ||--o{ ORDER : places\n",
        "entity_relationship_diagram",
        &[
            "er_entity_alias",
            "er_attribute_block",
            "er_attribute",
            "er_relationship",
        ],
    );
    assert_structured(
        "flowchart TD\nsubgraph lane[Lane]\n  A[Alpha] -->|ships| B[Beta]\nend\nclassDef active fill:#f96\nclass A active\n",
        "flowchart_diagram",
        &[
            "flow_subgraph",
            "flow_edge_statement",
            "flow_edge_label",
            "flow_class_definition_statement",
            "flow_class_assignment_statement",
        ],
    );
    assert_structured(
        "stateDiagram-v2\n[*] --> Active\nstate Active {\n  Active --> Done: finish\n}\nDone --> [*]\n",
        "state_diagram",
        &[
            "state_composite_declaration",
            "state_transition_statement",
            "state_marker",
            "state_description_text",
        ],
    );
    assert_structured(
        "swimlane-beta LR\nsubgraph sales[Sales]\n  lead -->|ships| quote\nend\n",
        "swimlane_diagram",
        &[
            "swimlane_subgraph",
            "swimlane_edge_statement",
            "swimlane_edge_label",
            "swimlane_vertex",
        ],
    );
}

#[test]
fn line_local_recovery_preserves_following_graph_siblings() {
    let block = assert_structured(
        "block-beta\nbefore[\"Before\"]\nbefore -->\nafter[\"After\"]\nbroken[\"unterminated\nlast[\"Last\"]\n",
        "block_diagram",
        &[
            "block_incomplete_edge_statement",
            "block_incomplete_shape",
            "block_node_statement",
        ],
    );
    assert_eq!(count_kind(block.root_node(), "block_node_statement"), 4);

    let c4 = assert_structured(
        "C4Context\nPerson(before, \"Before\")\nRel(before,\nSystem(after, \"After\")\nPerson(last, \"Last\")\n",
        "c4_diagram",
        &["c4_incomplete_statement", "c4_entity_declaration"],
    );
    assert_eq!(count_kind(c4.root_node(), "c4_entity_declaration"), 3);

    assert_structured(
        "classDiagram\nclass A\nA <|--\nnote for A \"unfinished\nclass B\nB --> A : follows\n",
        "class_diagram",
        &[
            "class_incomplete_relationship",
            "class_unclosed_string",
            "class_relationship",
        ],
    );
    assert_structured(
        "erDiagram\nA ||--o{\nB\nC ||--|| D : \"unfinished\nE ||--|| F : follows\n",
        "entity_relationship_diagram",
        &[
            "er_incomplete_relationship",
            "er_unclosed_quoted_text",
            "er_relationship",
        ],
    );
    assert_structured(
        "flowchart TD\nbefore --> after\nbroken -->\nleft ==> right\n",
        "flowchart_diagram",
        &["flow_incomplete_edge_statement", "flow_edge_statement"],
    );
    assert_structured(
        "stateDiagram-v2\nBefore --> After\nBroken -->\nLast --> [*]\n",
        "state_diagram",
        &[
            "state_incomplete_transition_statement",
            "state_transition_statement",
        ],
    );
    assert_structured(
        "swimlane-beta\nstart\n  --> finish\nbroken -->\nleft -. dotted .-> right\n",
        "swimlane_diagram",
        &[
            "swimlane_incomplete_edge_statement",
            "swimlane_edge_statement",
        ],
    );
}

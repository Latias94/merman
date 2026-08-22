use super::*;

#[test]
fn flowchart_parser_top_down_branch_merge_preserves_dagre_rank_and_connectivity() {
    let rendered = render_flowchart(
        concat!(
            "flowchart TD\n",
            "    A[Start] --> B{Condition?}\n",
            "    B -->|Yes| C[Execute]\n",
            "    B -->|No| D[End]\n",
            "    C --> D\n",
        ),
        &AsciiRenderOptions::unicode(),
    )
    .unwrap();

    for expected in ["Start", "Condition?", "Yes", "No", "Execute", "End"] {
        assert!(
            rendered.contains(expected),
            "branch merge should keep {expected:?} visible:\n{rendered}"
        );
    }
    let line_index = |needle: &str| first_line_index_containing(&rendered, needle);
    assert!(line_index("Start") < line_index("Condition?"), "{rendered}");
    assert!(
        line_index("Condition?") < line_index("Execute"),
        "{rendered}"
    );
    assert!(line_index("Execute") < line_index("End"), "{rendered}");
    assert!(
        rendered.matches('▼').count() >= 4,
        "all four semantic edges should retain target markers:\n{rendered}"
    );
}

#[test]
fn flowchart_parser_top_down_skip_edge_preserves_dagre_chain_ranks() {
    let rendered = render_flowchart(
        concat!(
            "flowchart TD\n",
            "  A --> C\n",
            "  A --> B\n",
            "  B --> C\n",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("same-rank top-down skip edge should route");

    let line_index = |needle: &str| first_line_index_containing(&rendered, needle);
    assert!(line_index("A") < line_index("B"), "{rendered}");
    assert!(line_index("B") < line_index("C"), "{rendered}");
    assert_eq!(
        rendered
            .chars()
            .filter(|ch| matches!(ch, '>' | '<' | 'v' | '^'))
            .count(),
        3,
        "the two chain edges and skip edge should all retain target markers:\n{rendered}"
    );
    assert_rectangular_char_grid(&rendered);
}

#[test]
fn flowchart_parser_top_down_skip_edge_routes_for_every_declaration_order() {
    // Dagre ranking owns topology before terminal routing. Equivalent edge declaration orders must
    // therefore keep the semantic A -> B -> C ranks even though route paint order can still vary.
    for input in [
        "flowchart TD\n  A --> B\n  B --> C\n  A --> C\n",
        "flowchart TD\n  A --> B\n  A --> C\n  B --> C\n",
        "flowchart TD\n  B --> C\n  A --> B\n  A --> C\n",
        "flowchart TD\n  B --> C\n  A --> C\n  A --> B\n",
        "flowchart TD\n  A --> C\n  A --> B\n  B --> C\n",
        "flowchart TD\n  A --> C\n  B --> C\n  A --> B\n",
    ] {
        let rendered = render_flowchart(input, &AsciiRenderOptions::ascii())
            .unwrap_or_else(|err| panic!("top-down skip edge should route for {input:?}: {err}"));

        for node in ["A", "B", "C"] {
            assert!(
                rendered.contains(node),
                "node {node} should stay visible for {input:?}:\n{rendered}"
            );
        }
        let a_line = first_line_index_containing(&rendered, "| A |");
        let b_line = first_line_index_containing(&rendered, "| B |");
        let c_line = first_line_index_containing(&rendered, "| C |");
        assert!(
            a_line < b_line && b_line < c_line,
            "Dagre ranks must stay invariant for {input:?}:\n{rendered}"
        );
        assert_rectangular_char_grid(&rendered);
    }
}

#[test]
fn flowchart_parser_ranks_explicitly_declared_reverse_order_chain_by_connectivity() {
    let rendered = render_flowchart(
        "flowchart TD\n  C\n  B\n  A\n  A --> B\n  B --> C\n",
        &AsciiRenderOptions::ascii(),
    )
    .expect("reverse-declared chain should render through Dagre rank semantics");

    let a_line = first_line_index_containing(&rendered, "| A |");
    let b_line = first_line_index_containing(&rendered, "| B |");
    let c_line = first_line_index_containing(&rendered, "| C |");
    assert!(
        a_line < b_line && b_line < c_line,
        "connectivity must own rank independently of node declaration order:\n{rendered}"
    );
}

#[test]
fn flowchart_parser_top_down_skip_edges_retain_every_terminal_marker() {
    for input in [
        "flowchart TD\n  A --> B\n  B --> C\n  A --> C\n",
        "flowchart TD\n  A --> B\n  A --> C\n  B --> C\n",
        "flowchart TD\n  B --> C\n  A --> B\n  A --> C\n",
        "flowchart TD\n  B --> C\n  A --> C\n  A --> B\n",
        "flowchart TD\n  A --> C\n  A --> B\n  B --> C\n",
        "flowchart TD\n  A --> C\n  B --> C\n  A --> B\n",
    ] {
        let rendered = render_flowchart(input, &AsciiRenderOptions::ascii()).unwrap();
        let arrow_count = rendered
            .chars()
            .filter(|ch| matches!(ch, '>' | '<' | 'v' | '^'))
            .count();

        assert_eq!(
            arrow_count, 3,
            "all three directed edges should retain one terminal marker for {input:?}:\n{rendered}"
        );
    }
}

#[test]
fn flowchart_parser_preserves_bidirectional_point_markers_in_both_charsets() {
    let ascii = render_flowchart("flowchart LR\nA <--> B", &AsciiRenderOptions::ascii()).unwrap();
    let unicode =
        render_flowchart("flowchart LR\nA <--> B", &AsciiRenderOptions::unicode()).unwrap();

    assert_eq!(ascii.matches('<').count(), 1, "{ascii}");
    assert_eq!(ascii.matches('>').count(), 1, "{ascii}");
    assert_eq!(unicode.matches('◄').count(), 1, "{unicode}");
    assert_eq!(unicode.matches('►').count(), 1, "{unicode}");
}

#[test]
fn flowchart_parser_preserves_mixed_circle_and_cross_markers() {
    let ascii = render_flowchart("flowchart LR\nA o--x B", &AsciiRenderOptions::ascii()).unwrap();
    let unicode =
        render_flowchart("flowchart LR\nA o--x B", &AsciiRenderOptions::unicode()).unwrap();

    assert_eq!(ascii.matches('o').count(), 1, "{ascii}");
    assert_eq!(ascii.matches('x').count(), 1, "{ascii}");
    assert_eq!(unicode.matches('○').count(), 1, "{unicode}");
    assert_eq!(unicode.matches('×').count(), 1, "{unicode}");
}

#[test]
fn flowchart_parser_preserves_labeled_mixed_markers_without_label_overlap() {
    let input = "flowchart LR\nA o-- label --x B";
    let ascii = render_flowchart(input, &AsciiRenderOptions::ascii()).unwrap();
    let unicode = render_flowchart(input, &AsciiRenderOptions::unicode()).unwrap();

    assert_eq!(ascii.matches('o').count(), 1, "{ascii}");
    assert_eq!(ascii.matches('x').count(), 1, "{ascii}");
    assert!(ascii.contains("label"), "{ascii}");
    assert_eq!(unicode.matches('○').count(), 1, "{unicode}");
    assert_eq!(unicode.matches('×').count(), 1, "{unicode}");
    assert!(unicode.contains("label"), "{unicode}");
}

#[test]
fn flowchart_parser_top_down_double_point_edge_keeps_both_directions() {
    let rendered =
        render_flowchart("flowchart TD\nA <--> B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(rendered.matches('^').count(), 1, "{rendered}");
    assert_eq!(rendered.matches('v').count(), 1, "{rendered}");
}

#[test]
fn flowchart_parser_bt_skip_edge_preserves_dagre_chain_after_vertical_flip() {
    let rendered = render_flowchart(
        "flowchart BT\n  A --> C\n  A --> B\n  B --> C\n",
        &AsciiRenderOptions::ascii(),
    )
    .expect("same-rank bottom-up skip edge should route");

    let line_index = |needle: &str| first_line_index_containing(&rendered, needle);
    assert!(line_index("C") < line_index("B"), "{rendered}");
    assert!(line_index("B") < line_index("A"), "{rendered}");
    assert_eq!(
        rendered
            .chars()
            .filter(|ch| matches!(ch, '>' | '<' | 'v' | '^'))
            .count(),
        3,
        "the two chain edges and skip edge should survive the BT mirror:\n{rendered}"
    );
    assert_rectangular_char_grid(&rendered);
}

#[test]
fn flowchart_parser_top_down_skip_edge_keeps_unicode_connectivity() {
    let rendered = render_flowchart(
        "flowchart TD\n  A --> C\n  A --> B\n  B --> C\n",
        &AsciiRenderOptions::unicode(),
    )
    .expect("same-rank top-down skip edge should route in unicode");

    let line_index = |needle: &str| first_line_index_containing(&rendered, needle);
    assert!(line_index("A") < line_index("B"), "{rendered}");
    assert!(line_index("B") < line_index("C"), "{rendered}");
    assert_eq!(
        rendered
            .chars()
            .filter(|ch| matches!(ch, '▼' | '◄' | '►' | '▲'))
            .count(),
        3,
        "unicode skip and chain edges should keep all target markers:\n{rendered}"
    );
}

#[test]
fn flowchart_parser_top_down_skip_edge_label_preserves_arrow() {
    let rendered = render_flowchart(
        "flowchart TD\n  A --> C\n  A --> B\n  B -->|back| C\n",
        &AsciiRenderOptions::unicode(),
    )
    .expect("labeled same-rank top-down edge should route in unicode");

    assert!(rendered.contains("back"), "{rendered}");
    assert_eq!(
        rendered
            .chars()
            .filter(|ch| matches!(ch, '▼' | '◄' | '►' | '▲'))
            .count(),
        3,
        "the labeled chain edge and skip edge should keep all target markers:\n{rendered}"
    );
}

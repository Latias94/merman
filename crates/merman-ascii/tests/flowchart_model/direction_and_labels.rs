use super::*;

#[test]
fn flowchart_parser_lr_chain_matches_upstream_ascii_golden() {
    let rendered = render_flowchart("flowchart LR\nA --> B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(rendered, fixture_expected("ascii", "two_nodes_linked.txt"));
}

#[test]
fn flowchart_graph_alias_lr_chain_matches_upstream_ascii_golden() {
    let rendered = render_flowchart("graph LR\nA --> B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(rendered, fixture_expected("ascii", "two_nodes_linked.txt"));
}

#[test]
fn flowchart_parser_lr_chain_matches_upstream_unicode_golden() {
    let rendered =
        render_flowchart("flowchart LR\nA --> B", &AsciiRenderOptions::unicode()).unwrap();

    assert_eq!(
        rendered,
        fixture_expected("extended-chars", "two_nodes_linked.txt")
    );
}

#[test]
fn flowchart_parser_tb_chain_matches_upstream_ascii_golden() {
    let rendered = render_flowchart(
        "flowchart TB\nA --> B\nB --> C",
        &AsciiRenderOptions::ascii(),
    )
    .unwrap();

    assert_eq!(
        rendered,
        fixture_expected("ascii", "flowchart_tb_simple.txt")
    );
}

#[test]
fn flowchart_parser_bt_root_direction_renders_with_vertical_flip() {
    let rendered = render_flowchart("flowchart BT\nA --> B", &AsciiRenderOptions::ascii())
        .expect("BT flowchart direction should render as a vertical flip of TD");

    assert_eq!(
        rendered,
        concat!(
            "+---+\n", "|   |\n", "| B |\n", "|   |\n", "+---+\n", "  ^  \n", "  |  \n", "  |  \n",
            "  |  \n", "  |  \n", "+---+\n", "|   |\n", "| A |\n", "|   |\n", "+---+\n",
        )
    );
}

#[test]
fn flowchart_parser_bt_multiline_edge_label_preserves_authored_line_order() {
    let rendered = render_flowchart(
        "flowchart BT\nA -->|first<br>second| B",
        &AsciiRenderOptions::ascii(),
    )
    .expect("BT multiline edge labels should render without reversing their text");

    let first = first_line_index_containing(&rendered, "first");
    let second = first_line_index_containing(&rendered, "second");
    assert_eq!(
        second,
        first + 1,
        "vertical mirroring should preserve authored label row order:\n{rendered}"
    );
}

#[test]
fn flowchart_parser_rl_root_direction_renders_with_horizontal_mirror() {
    let rendered = render_flowchart("flowchart RL\nA --> B", &AsciiRenderOptions::ascii())
        .expect("RL flowchart direction should render as a horizontal mirror of LR");

    assert_eq!(
        rendered,
        concat!(
            "+---+     +---+\n",
            "|   |     |   |\n",
            "| B |<----| A |\n",
            "|   |     |   |\n",
            "+---+     +---+\n",
        )
    );
}

#[test]
fn flowchart_parser_rl_multi_character_node_labels_stay_readable() {
    let rendered = render_flowchart(
        "flowchart RL\nLongerName1 --> LongerName2",
        &AsciiRenderOptions::ascii(),
    )
    .unwrap();

    assert_eq!(
        rendered,
        concat!(
            "+-------------+     +-------------+\n",
            "|             |     |             |\n",
            "| LongerName2 |<----| LongerName1 |\n",
            "|             |     |             |\n",
            "+-------------+     +-------------+\n",
        )
    );
}

#[test]
fn flowchart_parser_rl_cjk_node_labels_reserve_display_cells() {
    let rendered = render_flowchart(
        "flowchart RL\nA[中A] --> B[终B]",
        &AsciiRenderOptions::ascii(),
    )
    .unwrap();

    assert_eq!(
        rendered,
        concat!(
            "+-----+     +-----+\n",
            "|     |     |     |\n",
            "| 终B |<----| 中A |\n",
            "|     |     |     |\n",
            "+-----+     +-----+\n",
        )
    );
}

#[test]
fn flowchart_parser_multibyte_reference_labels_render_readably() {
    let cases = [
        (
            "graph LR\nA[Café]-->|résumé|B[Über]",
            ["Café", "résumé", "Über"],
        ),
        (
            "graph LR\nA[Γειά]-->|ετικέτα|B[Κόσμος]",
            ["Γειά", "ετικέτα", "Κόσμος"],
        ),
        (
            "graph LR\nA[Привет]-->|метка|B[Мир]",
            ["Привет", "метка", "Мир"],
        ),
    ];

    for (input, expected_labels) in cases {
        let rendered = render_flowchart(input, &AsciiRenderOptions::ascii()).unwrap();

        for label in expected_labels {
            assert!(
                rendered.contains(label),
                "missing {label:?} in rendered fixture:\n{rendered}"
            );
        }
        assert_rectangular_char_grid(&rendered);
    }
}

#[test]
fn flowchart_parser_cjk_and_emoji_labels_reserve_terminal_cells() {
    let rendered = render_flowchart(
        "flowchart LR\nA[开始] -->|处理🚀| B[完成]",
        &AsciiRenderOptions::ascii(),
    )
    .unwrap();

    for expected in ["开始", "处理🚀", "完成"] {
        assert!(
            rendered.contains(expected),
            "wide label {expected:?} should stay visible:\n{rendered}"
        );
    }
    assert_rectangular_terminal_grid(&rendered);
}

#[test]
fn flowchart_parser_rl_edge_labels_stay_readable() {
    let rendered = render_flowchart(
        "flowchart RL\nA -- hello --> B",
        &AsciiRenderOptions::ascii(),
    )
    .unwrap();

    assert_eq!(
        rendered,
        concat!(
            "+---+       +---+\n",
            "|   |       |   |\n",
            "| B |<hello-| A |\n",
            "|   |       |   |\n",
            "+---+       +---+\n",
        )
    );
}

#[test]
fn flowchart_parser_rl_chain_mirrors_unicode_connectors() {
    let rendered = render_flowchart("flowchart RL\nA --> B", &AsciiRenderOptions::unicode())
        .expect("RL flowchart direction should mirror Unicode connectors and arrowheads");

    assert_eq!(
        rendered,
        concat!(
            "┌───┐     ┌───┐\n",
            "│   │     │   │\n",
            "│ B │◄────┤ A │\n",
            "│   │     │   │\n",
            "└───┘     └───┘\n",
        )
    );
}

#[test]
fn flowchart_parser_lr_edge_label_renders_on_edge_line() {
    let rendered = render_flowchart(
        "flowchart LR\nA -- hello --> B",
        &AsciiRenderOptions::ascii(),
    )
    .unwrap();

    assert_eq!(
        rendered,
        concat!(
            "+---+       +---+\n",
            "|   |       |   |\n",
            "| A |-hello>| B |\n",
            "|   |       |   |\n",
            "+---+       +---+\n",
        )
    );
}

#[test]
fn flowchart_parser_tb_edge_label_renders_between_nodes() {
    let rendered =
        render_flowchart("flowchart TB\nA -- yes --> B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(
        rendered,
        concat!(
            "+-----+\n",
            "|     |\n",
            "|  A  |\n",
            "|     |\n",
            "+-----+\n",
            "   |   \n",
            "   |   \n",
            "  yes  \n",
            "   |   \n",
            "   v   \n",
            "+-----+\n",
            "|     |\n",
            "|  B  |\n",
            "|     |\n",
            "+-----+\n",
        )
    );
}

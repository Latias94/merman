use super::*;

fn rendered_terminal_width(rendered: &str) -> usize {
    rendered
        .lines()
        .map(terminal_test_width)
        .max()
        .unwrap_or_default()
}

fn assert_issue_53_semantics(rendered: &str) {
    for expected in [
        "browser / agent",
        "*.docs.mysampleapp.net",
        "Route53 subzone",
        "delegated to mysampleapps account",
        "CloudFront distribution",
        "wildcard cert",
        "Host-to-prefix CF Function",
        "WAF VPN",
        "allowlist, OAC to S3",
        "upload app",
        "Lambda, Python",
        "GET / form",
        "POST /upload",
        "POST /api/deploy",
        "IAM",
        "boto3",
        "S3 bucket mysampleapp",
        "sites/subdomain/",
        "DynamoDB reservations",
        "reads",
        "writes",
        "reserve / check owner",
        "CreateInvalidation",
    ] {
        assert!(
            rendered.contains(expected),
            "missing {expected:?} in wrapped Issue #53 fixture:\n{rendered}"
        );
    }
}

#[test]
fn flowchart_issue_53_long_node_labels_wrap_before_layout() {
    let input = local_semantic_input("flowchart/issue_53_long_node_labels.mmd");

    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        let rendered = render_flowchart(&input, &options).expect("Issue #53 should render");

        assert_issue_53_semantics(&rendered);
        assert!(
            rendered_terminal_width(&rendered) <= 100,
            "default terminal wrapping should keep Issue #53 readable:\n{rendered}"
        );
        assert_rectangular_terminal_grid(&rendered);
    }
}

#[test]
fn flowchart_node_label_wrap_width_changes_the_pre_layout_plan() {
    let input = "flowchart LR\nA[\"Alpha Beta Gamma Delta Epsilon\"] --> B[Done]";
    let narrow = render_flowchart(
        input,
        &AsciiRenderOptions::ascii().with_flowchart_node_label_wrap_width(10),
    )
    .expect("custom narrow node-label wrapping should render");
    let wide = render_flowchart(
        input,
        &AsciiRenderOptions::ascii().with_flowchart_node_label_wrap_width(40),
    )
    .expect("custom wide node-label wrapping should render");

    for expected in ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Done"] {
        assert!(narrow.contains(expected), "missing {expected:?}:\n{narrow}");
        assert!(wide.contains(expected), "missing {expected:?}:\n{wide}");
    }
    assert!(
        rendered_terminal_width(&narrow) < rendered_terminal_width(&wide),
        "a narrower pre-layout plan should produce a narrower graph:\nnarrow:\n{narrow}\nwide:\n{wide}"
    );
    assert!(
        narrow.lines().count() > wide.lines().count(),
        "a narrower node should gain label rows before routing"
    );
}

#[test]
fn flowchart_node_label_wrapping_preserves_authored_breaks_and_graphemes() {
    let input = "flowchart TD\nA[\"é team 👩‍💻<br>超長標籤 abcdefghijklmnop\"] --> B[Done]";
    let options = AsciiRenderOptions::unicode().with_flowchart_node_label_wrap_width(8);
    let rendered = render_flowchart(input, &options).expect("wrapped Unicode label should render");

    for expected in [
        "é",
        "team",
        "👩‍💻",
        "超長標籤",
        "abcdefgh",
        "ijklmnop",
        "Done",
    ] {
        assert!(
            rendered.contains(expected),
            "missing complete grapheme/text fragment {expected:?}:\n{rendered}"
        );
    }
    let emoji_row = first_line_index_containing(&rendered, "👩‍💻");
    let cjk_row = first_line_index_containing(&rendered, "超長");
    assert!(
        cjk_row > emoji_row,
        "the authored <br> must remain a hard break:\n{rendered}"
    );
    assert_rectangular_terminal_grid(&rendered);
}

#[test]
fn flowchart_wrapped_node_labels_respect_exact_grid_limit() {
    let input = "flowchart TD\nA[\"Alpha Beta Gamma Delta Epsilon\"] --> B[Done]";
    let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
    let base_options = AsciiRenderOptions::ascii()
        .with_flowchart_node_label_wrap_width(8)
        .with_resource_policy(unbounded);
    let measured = render_flowchart(input, &base_options).expect("unbounded graph should render");
    let exact_cells = rendered_terminal_width(&measured) * measured.lines().count();

    let exact_policy = unbounded
        .with_limit(AsciiResourceLimitId::MaxGridCells, exact_cells)
        .expect("exact grid limit should be valid");
    render_flowchart(input, &base_options.with_resource_policy(exact_policy))
        .expect("exact wrapped-node grid limit should render");

    let below_policy = unbounded
        .with_limit(AsciiResourceLimitId::MaxGridCells, exact_cells - 1)
        .expect("max-minus-one grid limit should be valid");
    let error = render_flowchart(input, &base_options.with_resource_policy(below_policy))
        .expect_err("max-minus-one wrapped-node grid limit should fail");
    let AsciiError::ResourceLimitExceeded(details) = error else {
        panic!("expected a grid resource error, got {error:?}");
    };
    assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
    assert_eq!(details.actual, exact_cells);
    assert_eq!(details.max, exact_cells - 1);
}

#[test]
fn flowchart_node_label_wrapping_renders_for_shapes_and_root_directions() {
    for direction in ["TD", "LR", "RL", "BT"] {
        let input = format!(
            "flowchart {direction}\nA@{{ shape: stadium, label: \"Alpha Beta Gamma Delta\" }} --> B[Done]"
        );
        let rendered = render_flowchart(
            &input,
            &AsciiRenderOptions::unicode().with_flowchart_node_label_wrap_width(8),
        )
        .unwrap_or_else(|error| panic!("{direction} wrapped stadium should render: {error}"));

        for expected in ["Alpha", "Beta", "Gamma", "Delta", "Done"] {
            assert!(
                rendered.contains(expected),
                "missing {expected:?} for {direction}:\n{rendered}"
            );
        }
        assert_rectangular_terminal_grid(&rendered);
    }
}

#[test]
fn flowchart_node_label_plans_check_aggregate_text_limits() {
    let input = "flowchart TD\nA[Alpha]\nB[Bravo]";
    let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);

    for limit in [
        AsciiResourceLimitId::MaxDocumentCells,
        AsciiResourceLimitId::MaxOutputBytes,
    ] {
        let policy = unbounded
            .with_limit(limit, 9)
            .expect("aggregate text limit should be valid");
        let error = render_flowchart(
            input,
            &AsciiRenderOptions::ascii().with_resource_policy(policy),
        )
        .expect_err("two five-cell labels should exceed an aggregate limit of nine");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected an aggregate text resource error, got {error:?}");
        };
        assert_eq!(details.limit, limit);
        assert_eq!(details.actual, 10);
        assert_eq!(details.max, 9);
    }
}

#[test]
fn flowchart_node_label_wrapping_uses_the_selected_width_profile() {
    let input = "flowchart TD\nA[\"A·B C\"]";
    let unicode = render_flowchart(
        input,
        &AsciiRenderOptions::unicode().with_flowchart_node_label_wrap_width(5),
    )
    .expect("Unicode-width label should render");
    let cjk = render_flowchart(
        input,
        &AsciiRenderOptions::unicode()
            .with_terminal_width_profile(merman_ascii::TerminalWidthProfile::Cjk)
            .with_flowchart_node_label_wrap_width(5),
    )
    .expect("CJK-width label should render");

    assert!(unicode.contains("A·B C"), "{unicode}");
    assert!(cjk.contains("A·B"), "{cjk}");
    assert!(cjk.contains('C'), "{cjk}");
    assert!(
        cjk.lines().count() > unicode.lines().count(),
        "the ambiguous-width character should force an extra CJK label row:\nunicode:\n{unicode}\ncjk:\n{cjk}"
    );
}

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

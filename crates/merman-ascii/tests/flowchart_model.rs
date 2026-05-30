use merman_ascii::{AsciiRenderOptions, render_model};
use merman_core::{Engine, ParseOptions};
use std::path::Path;

fn render_flowchart(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("flowchart should parse")
        .expect("flowchart should be detected");

    render_model(&parsed.model, options)
}

fn fixture_expected(directory: &str, name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/testdata/mermaid-ascii")
        .join(directory)
        .join(name);
    let content = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
        .replace("\r\n", "\n");
    let (_, expected) = content
        .split_once("\n---\n")
        .unwrap_or_else(|| panic!("fixture missing separator: {}", path.display()));
    expected.to_string()
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

#[test]
fn flowchart_parser_simple_subgraph_renders_group_box() {
    let rendered = render_flowchart(
        "flowchart TB\nsubgraph one\nA --> B\nend",
        &AsciiRenderOptions::ascii(),
    )
    .unwrap();

    assert_eq!(
        rendered,
        fixture_expected("ascii", "graph_tb_direction.txt")
    );
}

#[test]
fn flowchart_parser_circle_shape_renders_as_round_terminal_shape() {
    let rendered =
        render_flowchart("flowchart LR\nA((A)) --> B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(
        rendered,
        "/---\\     +---+\n|   |     |   |\n| A |---->| B |\n|   |     |   |\n\\---/     +---+\n"
    );
}

#[test]
fn flowchart_parser_diamond_shape_renders_as_decision_terminal_shape() {
    let rendered =
        render_flowchart("flowchart LR\nA{A} --> B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(
        rendered,
        "/---\\     +---+\n/   \\     |   |\n< A >---->| B |\n\\   /     |   |\n\\---/     +---+\n"
    );
}

#[test]
fn flowchart_parser_subroutine_and_cylinder_shapes_render_terminal_approximations() {
    let rendered = render_flowchart(
        "flowchart LR\nA[[Sub]] --> B[(DB)]",
        &AsciiRenderOptions::ascii(),
    )
    .unwrap();

    assert_eq!(
        rendered,
        concat!(
            "+-------+     /------\\\n",
            "| |   | |     |------|\n",
            "| |Sub| |---->|  DB  |\n",
            "| |   | |     |      |\n",
            "+-------+     \\------/\n",
        )
    );
}

#[test]
fn flowchart_parser_dotted_edges_render_with_dotted_line() {
    let rendered =
        render_flowchart("flowchart LR\nA -.-> B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(
        rendered,
        "+---+     +---+\n|   |     |   |\n| A |....>| B |\n|   |     |   |\n+---+     +---+\n"
    );
}

#[test]
fn flowchart_parser_thick_edges_render_with_heavy_ascii_line() {
    let rendered = render_flowchart("flowchart LR\nA ==> B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(
        rendered,
        "+---+     +---+\n|   |     |   |\n| A |====>| B |\n|   |     |   |\n+---+     +---+\n"
    );
}

#[test]
fn flowchart_parser_thick_edges_render_with_heavy_unicode_line() {
    let rendered =
        render_flowchart("flowchart LR\nA ==> B", &AsciiRenderOptions::unicode()).unwrap();

    assert_eq!(
        rendered,
        "┌───┐     ┌───┐\n│   │     │   │\n│ A ├━━━━►│ B │\n│   │     │   │\n└───┘     └───┘\n"
    );
}

#[test]
fn flowchart_parser_thick_top_down_edges_render_with_heavy_ascii_line() {
    let rendered = render_flowchart("flowchart TB\nA ==> B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(
        rendered,
        concat!(
            "+---+\n", "|   |\n", "| A |\n", "|   |\n", "+---+\n", "  #  \n", "  #  \n", "  #  \n",
            "  #  \n", "  v  \n", "+---+\n", "|   |\n", "| B |\n", "|   |\n", "+---+\n",
        )
    );
}

#[test]
fn flowchart_parser_open_edges_render_without_arrowhead() {
    let rendered = render_flowchart("flowchart LR\nA --- B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(
        rendered,
        "+---+     +---+\n|   |     |   |\n| A |-----| B |\n|   |     |   |\n+---+     +---+\n"
    );
}

#[test]
fn flowchart_parser_edge_length_modifiers_add_spacing() {
    let rendered =
        render_flowchart("flowchart LR\nA ----> B", &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(
        rendered,
        "+---+         +---+\n|   |         |   |\n| A |-------->| B |\n|   |         |   |\n+---+         +---+\n"
    );
}

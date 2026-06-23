use merman_ascii::{
    AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiRenderOptions, AsciiRgb, render_model,
};
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

fn strip_ansi(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for escaped in chars.by_ref() {
                if escaped == 'm' {
                    break;
                }
            }
            continue;
        }
        output.push(ch);
    }
    output
}

fn strip_html_spans(input: &str) -> String {
    let mut output = String::new();
    let mut index = 0;
    while index < input.len() {
        let rest = &input[index..];
        if rest.starts_with("<span ") {
            index += rest.find('>').expect("span start tag should be closed") + 1;
            continue;
        }
        if rest.starts_with("</span>") {
            index += "</span>".len();
            continue;
        }
        let ch = rest
            .chars()
            .next()
            .expect("index should be on a char boundary");
        if let Some(entity) = rest.strip_prefix("&gt;") {
            output.push('>');
            index += rest.len() - entity.len();
        } else if let Some(entity) = rest.strip_prefix("&lt;") {
            output.push('<');
            index += rest.len() - entity.len();
        } else if let Some(entity) = rest.strip_prefix("&amp;") {
            output.push('&');
            index += rest.len() - entity.len();
        } else {
            output.push(ch);
            index += ch.len_utf8();
        }
    }
    output
}

#[test]
fn flowchart_color_truecolor_emits_semantic_roles_without_changing_plain_text() {
    let theme = AsciiColorTheme::default_light()
        .with_role(AsciiColorRole::NodeBorder, AsciiRgb::new(1, 1, 1))
        .with_role(AsciiColorRole::Text, AsciiRgb::new(2, 2, 2))
        .with_role(AsciiColorRole::EdgeLine, AsciiRgb::new(3, 3, 3))
        .with_role(AsciiColorRole::EdgeArrow, AsciiRgb::new(4, 4, 4))
        .with_role(AsciiColorRole::EdgeLabel, AsciiRgb::new(5, 5, 5));
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::TrueColor)
        .with_color_theme(theme);

    let rendered = render_flowchart("flowchart LR\nA -- yes --> B", &options).unwrap();

    assert_eq!(
        strip_ansi(&rendered),
        concat!(
            "+---+     +---+\n",
            "|   |     |   |\n",
            "| A |-yes>| B |\n",
            "|   |     |   |\n",
            "+---+     +---+\n",
        )
    );
    for expected_code in [
        "\u{1b}[38;2;1;1;1m",
        "\u{1b}[38;2;2;2;2m",
        "\u{1b}[38;2;3;3;3m",
        "\u{1b}[38;2;4;4;4m",
        "\u{1b}[38;2;5;5;5m",
    ] {
        assert!(
            rendered.contains(expected_code),
            "missing {expected_code:?} in {rendered:?}"
        );
    }
}

#[test]
fn flowchart_color_html_wraps_subgraph_roles_without_changing_plain_text() {
    let theme = AsciiColorTheme::default_light()
        .with_role(AsciiColorRole::GroupBorder, AsciiRgb::from_hex24(0x101010))
        .with_role(AsciiColorRole::MutedText, AsciiRgb::from_hex24(0x202020))
        .with_role(AsciiColorRole::NodeBorder, AsciiRgb::from_hex24(0x303030))
        .with_role(AsciiColorRole::EdgeLine, AsciiRgb::from_hex24(0x404040))
        .with_role(AsciiColorRole::EdgeArrow, AsciiRgb::from_hex24(0x505050))
        .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0x606060));
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::Html)
        .with_color_theme(theme);

    let rendered = render_flowchart("flowchart TB\nsubgraph one\nA --> B\nend", &options).unwrap();

    assert_eq!(
        strip_html_spans(&rendered),
        fixture_expected("ascii", "graph_tb_direction.txt")
    );
    for expected_fragment in [
        "<span style=\"color:#101010\">+-------+</span>",
        "<span style=\"color:#202020\">one</span>",
        "<span style=\"color:#303030\">+---+</span>",
        "<span style=\"color:#404040\">|</span>",
        "<span style=\"color:#505050\">v</span>",
        "<span style=\"color:#606060\">A</span>",
        "<span style=\"color:#606060\">B</span>",
    ] {
        assert!(
            rendered.contains(expected_fragment),
            "missing {expected_fragment:?} in {rendered:?}"
        );
    }
}

#[test]
fn flowchart_color_truecolor_preserves_roles_after_horizontal_mirror() {
    let theme = AsciiColorTheme::default_light()
        .with_role(AsciiColorRole::NodeBorder, AsciiRgb::new(7, 7, 7))
        .with_role(AsciiColorRole::Text, AsciiRgb::new(8, 8, 8))
        .with_role(AsciiColorRole::EdgeLine, AsciiRgb::new(9, 9, 9))
        .with_role(AsciiColorRole::EdgeArrow, AsciiRgb::new(10, 10, 10));
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::TrueColor)
        .with_color_theme(theme);

    let rendered = render_flowchart("flowchart RL\nA --> B", &options).unwrap();

    assert_eq!(
        strip_ansi(&rendered),
        concat!(
            "+---+     +---+\n",
            "|   |     |   |\n",
            "| B |<----| A |\n",
            "|   |     |   |\n",
            "+---+     +---+\n",
        )
    );
    for expected_code in [
        "\u{1b}[38;2;7;7;7m",
        "\u{1b}[38;2;8;8;8m",
        "\u{1b}[38;2;9;9;9m",
        "\u{1b}[38;2;10;10;10m",
    ] {
        assert!(
            rendered.contains(expected_code),
            "missing {expected_code:?} in {rendered:?}"
        );
    }
}

#[test]
fn flowchart_style_color_truecolor_maps_classdef_and_inline_node_foreground_without_plain_text_changes()
 {
    let input = concat!(
        "flowchart LR\n",
        "  A[Alpha]:::hot --> B[Beta]\n",
        "  classDef hot color:#112233,stroke:#445566,fill:#ffeecc\n",
        "  style B color:#778899,stroke:#aabbcc,fill:#001122\n",
    );
    let options = AsciiRenderOptions::ascii().with_color_mode(AsciiColorMode::TrueColor);

    let rendered = render_flowchart(input, &options).unwrap();
    let plain = render_flowchart(input, &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(strip_ansi(&rendered), plain);
    for expected_code in [
        "\u{1b}[38;2;17;34;51m",
        "\u{1b}[38;2;68;85;102m",
        "\u{1b}[38;2;119;136;153m",
        "\u{1b}[38;2;170;187;204m",
    ] {
        assert!(
            rendered.contains(expected_code),
            "missing {expected_code:?} in {rendered:?}"
        );
    }
    for ignored_fill_code in ["\u{1b}[38;2;255;238;204m", "\u{1b}[38;2;0;17;34m"] {
        assert!(
            !rendered.contains(ignored_fill_code),
            "fill/background style should not be emitted as foreground in {rendered:?}"
        );
    }
}

#[test]
fn flowchart_style_color_html_maps_linkstyle_edge_and_label_foreground_without_plain_text_changes()
{
    let input = concat!(
        "flowchart LR\n",
        "  A[Alpha] -->|go| B[Beta]\n",
        "  linkStyle 0 stroke:#123456,color:#654321\n",
    );
    let options = AsciiRenderOptions::ascii().with_color_mode(AsciiColorMode::Html);

    let rendered = render_flowchart(input, &options).unwrap();
    let plain = render_flowchart(input, &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(strip_html_spans(&rendered), plain);
    assert!(
        rendered.contains("<span style=\"color:#123456\">-</span>")
            || rendered.contains("<span style=\"color:#123456\">&gt;</span>"),
        "missing styled edge line or arrow in {rendered:?}"
    );
    assert!(
        rendered.contains("<span style=\"color:#654321\">go</span>"),
        "missing styled edge label in {rendered:?}"
    );
}

#[test]
fn flowchart_style_color_truecolor_maps_class_statement_to_node_and_subgraph_foreground_without_plain_text_changes()
 {
    let input = concat!(
        "flowchart TB\n",
        "  subgraph sg [Group]\n",
        "    A[Alpha]\n",
        "  end\n",
        "  classDef warm color:#010203,stroke:#040506\n",
        "  class sg warm\n",
        "  class A warm\n",
    );
    let options = AsciiRenderOptions::ascii().with_color_mode(AsciiColorMode::TrueColor);

    let rendered = render_flowchart(input, &options).unwrap();
    let plain = render_flowchart(input, &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(strip_ansi(&rendered), plain);
    for expected_code in ["\u{1b}[38;2;1;2;3m", "\u{1b}[38;2;4;5;6m"] {
        assert!(
            rendered.contains(expected_code),
            "missing {expected_code:?} in {rendered:?}"
        );
    }
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
fn flowchart_parser_top_down_branch_merge_uses_connected_unicode_bend_corner() {
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

    assert!(
        rendered.contains("├────────┐"),
        "top-down right/down branch should use a connected top-right bend: {rendered}"
    );
    assert!(
        !rendered.contains("├────────└"),
        "top-down right/down branch must not use a disconnected bottom-left bend: {rendered}"
    );
    assert!(
        rendered.contains("│  Execute   ├────►│ End │"),
        "top-down same-rank merge edge should be rendered instead of being dropped: {rendered}"
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
fn flowchart_parser_multiline_subgraph_title_renders_centered_rows() {
    let rendered = render_flowchart(
        "flowchart TB\nsubgraph cluster [Line<br>Two]\nA\nend",
        &AsciiRenderOptions::ascii(),
    )
    .expect("subgraph titles with Mermaid break syntax should render as multiline title rows");

    assert_eq!(
        rendered,
        concat!(
            "+-------+\n",
            "| Line  |\n",
            "|       |\n",
            "|  Two  |\n",
            "|       |\n",
            "|       |\n",
            "| +---+ |\n",
            "| |   | |\n",
            "| | A | |\n",
            "| |   | |\n",
            "| +---+ |\n",
            "|       |\n",
            "+-------+\n",
        )
    );
}

#[test]
fn flowchart_parser_long_subgraph_title_wraps_to_multiple_rows() {
    let rendered = render_flowchart(
        "flowchart LR\nsubgraph cluster [Wrap this title nicely]\nA --> B\nend",
        &AsciiRenderOptions::ascii(),
    )
    .expect("long subgraph titles should wrap inside the existing group box");

    assert_eq!(
        rendered,
        concat!(
            "+-----------------+\n",
            "| Wrap this title |\n",
            "|                 |\n",
            "|     nicely      |\n",
            "|                 |\n",
            "|                 |\n",
            "| +---+     +---+ |\n",
            "| |   |     |   | |\n",
            "| | A |---->| B | |\n",
            "| |   |     |   | |\n",
            "| +---+     +---+ |\n",
            "|                 |\n",
            "+-----------------+\n",
        )
    );
}

#[test]
fn render_model_subgraph_direction_override_renders_local_left_right_layout_without_cross_boundary_edges()
 {
    let model = merman_core::diagrams::flowchart::FlowchartV2Model {
        acc_descr: None,
        acc_title: None,
        class_defs: Default::default(),
        direction: Some("TD".to_string()),
        edge_defaults: None,
        vertex_calls: Vec::new(),
        nodes: vec![
            merman_core::diagrams::flowchart::FlowNode {
                id: "A".to_string(),
                label: Some("A".to_string()),
                label_type: None,
                layout_shape: None,
                icon: None,
                form: None,
                pos: None,
                img: None,
                constraint: None,
                asset_width: None,
                asset_height: None,
                classes: Vec::new(),
                styles: Vec::new(),
                link: None,
                link_target: None,
                have_callback: false,
            },
            merman_core::diagrams::flowchart::FlowNode {
                id: "B".to_string(),
                label: Some("B".to_string()),
                label_type: None,
                layout_shape: None,
                icon: None,
                form: None,
                pos: None,
                img: None,
                constraint: None,
                asset_width: None,
                asset_height: None,
                classes: Vec::new(),
                styles: Vec::new(),
                link: None,
                link_target: None,
                have_callback: false,
            },
        ],
        edges: vec![merman_core::diagrams::flowchart::FlowEdge {
            id: "L-A-B".to_string(),
            from: "A".to_string(),
            to: "B".to_string(),
            label: None,
            label_type: None,
            edge_type: Some("arrow_point".to_string()),
            stroke: Some("normal".to_string()),
            interpolate: None,
            classes: Vec::new(),
            style: Vec::new(),
            animate: None,
            animation: None,
            length: 1,
        }],
        subgraphs: vec![merman_core::diagrams::flowchart::FlowSubgraph {
            id: "one".to_string(),
            title: "LR Group".to_string(),
            dir: Some("LR".to_string()),
            label_type: None,
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: vec!["A".to_string(), "B".to_string()],
        }],
        tooltips: Default::default(),
    };
    let rendered = render_model(
        &merman_core::RenderSemanticModel::Flowchart(model),
        &AsciiRenderOptions::ascii(),
    )
    .expect("subgraph direction override should render a local LR layout inside a TD graph");

    assert_eq!(
        rendered,
        concat!(
            "+-----------------+\n",
            "|    LR Group     |\n",
            "|                 |\n",
            "|                 |\n",
            "| +---+     +---+ |\n",
            "| |   |     |   | |\n",
            "| | A |---->| B | |\n",
            "| |   |     |   | |\n",
            "| +---+     +---+ |\n",
            "|                 |\n",
            "+-----------------+\n",
        )
    );
}

#[test]
fn flowchart_parser_subgraph_direction_override_with_cross_boundary_edges_records_boundary_aware_baseline()
 {
    let rendered = render_flowchart(
        concat!(
            "flowchart TD\n",
            "subgraph one [LR Group]\n",
            "    direction LR\n",
            "    A --> B\n",
            "end\n",
            "X --> A\n",
            "B --> Y\n",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect(
        "cross-boundary mixed-direction subgraph should render through the boundary-aware seam",
    );

    assert_eq!(
        rendered,
        concat!(
            "+-----------------+        \n",
            "|    LR Group     |        \n",
            "|                 |        \n",
            "|                 |        \n",
            "| +---+     +---+ |   +---+\n",
            "| |   |     |   | |   |   |\n",
            "| | A |---->| B |+|   | X |\n",
            "| |   |     |   |||   |   |\n",
            "| +---+     +---+||   +---+\n",
            "|   ^            ||     |  \n",
            "+---+------------+------+  \n",
            "                 |         \n",
            "                 |         \n",
            "                 |         \n",
            "  +---+          |         \n",
            "  |   |          |         \n",
            "  | Y |<---------+         \n",
            "  |   |                    \n",
            "  +---+                    \n",
        )
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

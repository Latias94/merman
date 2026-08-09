use super::*;

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
    for expected_background_code in ["\u{1b}[48;2;255;238;204m", "\u{1b}[48;2;0;17;34m"] {
        assert!(
            rendered.contains(expected_background_code),
            "missing background {expected_background_code:?} in {rendered:?}"
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
fn flowchart_style_color_html_maps_node_fill_background_without_plain_text_changes() {
    let input = concat!(
        "flowchart LR\n",
        "  A[Alpha]:::hot\n",
        "  classDef hot color:#112233,stroke:#445566,fill:#ffeecc\n",
    );
    let options = AsciiRenderOptions::ascii().with_color_mode(AsciiColorMode::Html);

    let rendered = render_flowchart(input, &options).unwrap();
    let plain = render_flowchart(input, &AsciiRenderOptions::ascii()).unwrap();

    assert_eq!(strip_html_spans(&rendered), plain);
    assert!(
        rendered.contains("background-color:#ffeecc"),
        "missing node fill background in {rendered:?}"
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

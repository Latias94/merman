use merman_ascii::{AsciiRenderOptions, render_model};
use merman_core::{Engine, ParseOptions};

fn render_xychart(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("xychart should parse")
        .expect("xychart should be detected");

    render_model(&parsed.model, options)
}

#[test]
fn xychart_parser_vertical_bar_renders_ascii_chart_with_titles_and_axes() {
    let rendered = render_xychart(
        r#"xychart
title "Sales"
x-axis "Month" [Jan, Feb, Mar]
y-axis "Revenue" 0 --> 10
bar [2, 5, 8]
"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("xychart should render");

    assert_eq!(
        rendered,
        concat!(
            "Sales\n",
            "y: Revenue\n",
            "10 |\n",
            " 8 |        ###\n",
            " 6 |    ### ###\n",
            " 4 |    ### ###\n",
            " 2 |### ### ###\n",
            " 0 +-----------\n",
            "    Jan Feb Mar\n",
            "x: Month\n",
        )
    );
}

#[test]
fn xychart_parser_line_plot_renders_ascii_stair_step_line() {
    let rendered = render_xychart(
        r#"xychart
x-axis [A, B, C]
y-axis 0 --> 10
line [1, 5, 9]
"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("xychart line plot should render");

    assert_eq!(
        rendered,
        concat!(
            "10 |       ***\n",
            " 8 |       *\n",
            " 6 |   *****\n",
            " 4 |   *\n",
            " 2 | ***\n",
            " 0 +-----------\n",
            "     A   B   C\n",
        )
    );
}

#[test]
fn xychart_parser_mixed_bar_and_line_overlay_in_series_order() {
    let rendered = render_xychart(
        r#"xychart
x-axis [A, B]
y-axis 0 --> 10
bar [2, 8]
line [8, 2]
"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("mixed xychart should render");

    assert_eq!(
        rendered,
        concat!(
            "10 |\n",
            " 8 | ***###\n",
            " 6 |   *###\n",
            " 4 |   *###\n",
            " 2 |###***#\n",
            " 0 +-------\n",
            "     A   B\n",
        )
    );
}

#[test]
fn xychart_parser_horizontal_bar_renders_ascii_value_axis() {
    let rendered = render_xychart(
        r#"xychart horizontal
x-axis [A, B]
y-axis 0 --> 10
bar [4, 8]
"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("horizontal xychart should render");

    assert_eq!(
        rendered,
        concat!(
            "A |####       4\n",
            "B |########   8\n",
            "  +----------\n",
            "   0       10\n",
        )
    );
}

#[test]
fn xychart_parser_vertical_bar_renders_unicode_chart_chars() {
    let rendered = render_xychart(
        r#"xychart
x-axis [A, B]
y-axis 0 --> 5
bar [2, 5]
"#,
        &AsciiRenderOptions::unicode(),
    )
    .expect("unicode xychart should render");

    assert_eq!(
        rendered,
        concat!(
            "5 │    ███\n",
            "4 │    ███\n",
            "3 │    ███\n",
            "2 │███ ███\n",
            "1 │███ ███\n",
            "0 ┼───────\n",
            "    A   B\n",
        )
    );
}

#[test]
fn xychart_parser_infers_numeric_x_labels_when_x_axis_is_omitted() {
    let rendered = render_xychart(
        r#"xychart
y-axis 0 --> 10
bar [5]
"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("xychart with inferred x axis should render");

    assert_eq!(
        rendered,
        concat!(
            "10 |\n",
            " 8 |\n",
            " 6 |###\n",
            " 4 |###\n",
            " 2 |###\n",
            " 0 +---\n",
            "     1\n",
        )
    );
}

#[test]
fn xychart_parser_header_only_renders_empty_text() {
    let rendered = render_xychart("xychart", &AsciiRenderOptions::ascii())
        .expect("empty xychart should render");

    assert_eq!(rendered, "");
}

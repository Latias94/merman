use merman_ascii::{
    AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiError, AsciiRenderOptions, AsciiRgb,
    render_model,
};
use merman_core::{Engine, ParseOptions};
use std::path::Path;

fn render_xychart(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("xychart should parse")
        .expect("xychart should be detected");

    render_model(&parsed.model, options)
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
        output.push(ch);
        index += ch.len_utf8();
    }
    output
}

fn cjk_test_width(input: &str) -> usize {
    input
        .chars()
        .map(|ch| if ch.is_ascii() { 1 } else { 2 })
        .sum()
}

#[test]
fn xychart_color_truecolor_emits_axis_text_and_series_roles_without_changing_plain_text() {
    let theme = AsciiColorTheme::default_light()
        .with_role(AsciiColorRole::Text, AsciiRgb::new(1, 1, 1))
        .with_role(AsciiColorRole::ChartAxis, AsciiRgb::new(2, 2, 2))
        .with_role(AsciiColorRole::ChartSeries(0), AsciiRgb::new(3, 3, 3));
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::TrueColor)
        .with_color_theme(theme);

    let rendered = render_xychart(
        r#"xychart
title "Sales"
x-axis "Month" [Jan, Feb, Mar]
y-axis "Revenue" 0 --> 10
bar [2, 5, 8]
"#,
        &options,
    )
    .expect("xychart should render");

    assert_eq!(
        strip_ansi(&rendered),
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
    for expected_code in [
        "\u{1b}[38;2;1;1;1m",
        "\u{1b}[38;2;2;2;2m",
        "\u{1b}[38;2;3;3;3m",
    ] {
        assert!(
            rendered.contains(expected_code),
            "missing {expected_code:?} in {rendered:?}"
        );
    }
}

#[test]
fn xychart_color_html_wraps_bar_and_line_series_roles_without_changing_plain_text() {
    let theme = AsciiColorTheme::default_light()
        .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0x101010))
        .with_role(AsciiColorRole::ChartAxis, AsciiRgb::from_hex24(0x202020))
        .with_role(
            AsciiColorRole::ChartSeries(0),
            AsciiRgb::from_hex24(0x303030),
        )
        .with_role(
            AsciiColorRole::ChartSeries(1),
            AsciiRgb::from_hex24(0x404040),
        );
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::Html)
        .with_color_theme(theme);

    let rendered = render_xychart(
        r#"xychart
x-axis [A, B]
y-axis 0 --> 10
bar [2, 8]
line [8, 2]
"#,
        &options,
    )
    .expect("mixed xychart should render");

    assert_eq!(
        strip_html_spans(&rendered),
        concat!(
            "# Bar 1  * Line 1\n",
            "10 |\n",
            " 8 | ***###\n",
            " 6 |   *###\n",
            " 4 |   *###\n",
            " 2 |###***#\n",
            " 0 +-------\n",
            "     A   B\n",
        )
    );
    for expected_fragment in [
        "<span style=\"color:#303030\">#</span>",
        "<span style=\"color:#404040\">*</span>",
        "<span style=\"color:#202020\">|</span>",
        "<span style=\"color:#202020\">+-------</span>",
        "<span style=\"color:#303030\">###</span>",
        "<span style=\"color:#404040\">***</span>",
    ] {
        assert!(
            rendered.contains(expected_fragment),
            "missing {expected_fragment:?} in {rendered:?}"
        );
    }
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
            "# Bar 1  * Line 1\n",
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
fn xychart_parser_multiple_same_type_series_render_legend_labels_by_type_order() {
    let rendered = render_xychart(
        r#"xychart
x-axis [A, B]
y-axis 0 --> 10
bar [2, 8]
bar [5, 6]
line [8, 2]
line [4, 4]
"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("multi-series xychart should render");

    let legend = rendered
        .lines()
        .next()
        .expect("multi-series chart should render a legend line");

    assert_eq!(legend, "# Bar 1  # Bar 2  * Line 1  * Line 2");
}

#[test]
fn xychart_parser_uses_series_titles_in_legend_when_available() {
    let rendered = render_xychart(
        r#"xychart
x-axis [A, B]
y-axis 0 --> 10
bar "Revenue" [2, 8]
line "Forecast" [8, 2]
"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("xychart should render with series titles");

    let legend = rendered
        .lines()
        .next()
        .expect("xychart with series titles should render a legend line");

    assert_eq!(legend, "# Revenue  * Forecast");
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
fn xychart_plot_area_options_scale_vertical_chart() {
    let options = AsciiRenderOptions::ascii()
        .with_xychart_vertical_plot_height(4)
        .with_xychart_category_band_width(4);

    let rendered = render_xychart(
        r#"xychart
x-axis [Jan, Feb]
y-axis 0 --> 8
bar [4, 8]
"#,
        &options,
    )
    .expect("xychart should render with custom vertical plot area");

    assert_eq!(
        rendered,
        concat!(
            "8 |     ####\n",
            "6 |     ####\n",
            "4 |#### ####\n",
            "2 |#### ####\n",
            "0 +---------\n",
            "   Jan  Feb\n",
        )
    );
}

#[test]
fn xychart_plot_area_options_scale_horizontal_chart() {
    let options = AsciiRenderOptions::ascii().with_xychart_horizontal_plot_width(5);

    let rendered = render_xychart(
        r#"xychart horizontal
x-axis [A, B]
y-axis 0 --> 10
bar [4, 8]
"#,
        &options,
    )
    .expect("xychart should render with custom horizontal plot area");

    assert_eq!(
        rendered,
        concat!("A |##    4\n", "B |####  8\n", "  +-----\n", "   0  10\n",)
    );
}

#[test]
fn xychart_plot_area_respects_max_grid_cells() {
    let mut options = AsciiRenderOptions::ascii();
    options.max_grid_cells = 3;

    let err = render_xychart(
        r#"xychart
x-axis [A, B]
y-axis 0 --> 10
bar [4, 8]
"#,
        &options,
    )
    .expect_err("xychart plot area should respect max_grid_cells");

    assert_eq!(
        err,
        AsciiError::RenderLimitExceeded {
            actual: 35,
            limit: 3,
        }
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
fn xychart_parser_vertical_categories_respect_display_width_for_cjk() {
    let rendered = render_xychart(
        r#"xychart
x-axis [中, B]
y-axis 0 --> 5
bar [2, 5]
"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("CJK xychart categories should render");

    let axis_line = rendered
        .lines()
        .find(|line| line.contains('+'))
        .expect("axis line should render");
    let category_line = rendered
        .lines()
        .find(|line| line.contains('中'))
        .expect("CJK category should render");

    assert!(
        cjk_test_width(category_line) <= cjk_test_width(axis_line),
        "category labels must stay inside the plot width:\n{rendered}"
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

#[test]
fn xychart_local_semantic_fixture_covers_small_mixed_plot() {
    let input = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/testdata/local-semantic/xychart/mixed_small.mmd"),
    )
    .expect("local semantic xychart fixture must be readable");

    let rendered = render_xychart(&input, &AsciiRenderOptions::ascii())
        .expect("local semantic xychart fixture should render");

    for expected in ["Ops", "A", "B", "C"] {
        assert!(
            rendered.contains(expected),
            "local semantic xychart fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        rendered.lines().count() >= 5,
        "local semantic xychart fixture should produce a multi-line layout:\n{rendered}"
    );
}

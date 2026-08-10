use merman_ascii::{
    AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiError, AsciiRenderOptions,
    AsciiResourceLimitId, AsciiRgb, render_model, render_xychart as render_typed_xychart,
};
use merman_core::diagrams::xychart::{
    XyChartAxisDisplayPolicy, XyChartAxisRenderModel, XyChartDiagramRenderModel,
    XyChartDisplayPolicy, XyChartPlotRenderModel, XyChartPlotType,
};
use merman_core::{Engine, ParseOptions};
use std::path::Path;

fn render_xychart(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("xychart should parse")
        .expect("xychart should be detected");

    render_model(parsed.model(), options)
}

fn read_local_semantic_fixture(path: &str) -> String {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/testdata/local-semantic")
        .join(path);
    std::fs::read_to_string(&fixture_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", fixture_path.display()))
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

fn first_line_index_containing(rendered: &str, needle: &str) -> usize {
    rendered
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("missing {needle:?} in rendered fixture:\n{rendered}"))
}

fn cjk_test_width(input: &str) -> usize {
    input
        .chars()
        .map(|ch| if ch.is_ascii() { 1 } else { 2 })
        .sum()
}

fn hidden_axis_policy() -> XyChartAxisDisplayPolicy {
    XyChartAxisDisplayPolicy {
        show_label: false,
        show_title: false,
        show_tick: false,
        show_axis_line: false,
    }
}

fn typed_xychart_model(
    orientation: &str,
    x_axis: XyChartAxisRenderModel,
    y_min: f64,
    y_max: f64,
    plots: Vec<XyChartPlotRenderModel>,
) -> XyChartDiagramRenderModel {
    XyChartDiagramRenderModel {
        orientation: orientation.to_string(),
        title: None,
        acc_title: None,
        acc_descr: None,
        x_axis,
        y_axis: XyChartAxisRenderModel::Linear {
            title: String::new(),
            min: Some(y_min),
            max: Some(y_max),
        },
        plots,
        display: XyChartDisplayPolicy {
            show_title: false,
            show_data_label: false,
            show_data_label_outside_bar: false,
            x_axis: hidden_axis_policy(),
            y_axis: hidden_axis_policy(),
        },
    }
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
            "10 +\n",
            " 8 +        ###\n",
            " 6 +    ### ###\n",
            " 4 +    ### ###\n",
            " 2 +### ### ###\n",
            " 0 +-+---+---+-\n",
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
            "values: # Bar 1: A=2, B=8\n",
            "values: * Line 1: A=8, B=2\n",
            "10 +\n",
            " 8 + *-+###\n",
            " 6 +   |###\n",
            " 4 +   |###\n",
            " 2 +###+-*#\n",
            " 0 +-+---+-\n",
            "     A   B\n",
        )
    );
    for expected_fragment in [
        "<span style=\"color:#303030\">#</span>",
        "<span style=\"color:#404040\">*</span>",
        "<span style=\"color:#202020\">+</span>",
        "<span style=\"color:#303030\">###</span>",
        "<span style=\"color:#404040\">*-+</span>",
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
            "10 +\n",
            " 8 +        ###\n",
            " 6 +    ### ###\n",
            " 4 +    ### ###\n",
            " 2 +### ### ###\n",
            " 0 +-+---+---+-\n",
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
            "10 +       +-*\n",
            " 8 +       |\n",
            " 6 +   +-*-+\n",
            " 4 +   |\n",
            " 2 + *-+\n",
            " 0 +-+---+---+-\n",
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
            "values: # Bar 1: A=2, B=8\n",
            "values: * Line 1: A=8, B=2\n",
            "10 +\n",
            " 8 + *-+###\n",
            " 6 +   |###\n",
            " 4 +   |###\n",
            " 2 +###+-*#\n",
            " 0 +-+---+-\n",
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
            "A +####\n",
            "B +########\n",
            "  ++--------+\n",
            "   0       10\n",
        )
    );
}

#[test]
fn xychart_parser_horizontal_bar_shows_data_labels_when_enabled() {
    let rendered = render_xychart(
        r#"%%{init: {"xyChart": {"showDataLabel": true}}}%%
xychart horizontal
x-axis [A, B]
y-axis 0 --> 10
bar [4, 8]
"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("horizontal xychart with data labels should render");

    assert_eq!(
        rendered,
        concat!(
            "A +###4\n",
            "B +#######8\n",
            "  ++--------+\n",
            "   0       10\n",
        )
    );
}

#[test]
fn xychart_parser_horizontal_bar_can_place_data_labels_outside_bars() {
    let rendered = render_xychart(
        r#"%%{init: {"xyChart": {"showDataLabel": true, "showDataLabelOutsideBar": true}}}%%
xychart horizontal
x-axis [A, B]
y-axis 0 --> 10
bar [4, 8]
"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("horizontal xychart with outside data labels should render");

    assert_eq!(
        rendered,
        concat!(
            "A +####       4\n",
            "B +########   8\n",
            "  ++--------+\n",
            "   0       10\n",
        )
    );
}

#[test]
fn xychart_parser_horizontal_line_uses_terminal_value_disclosure() {
    let rendered = render_xychart(
        r#"%%{init: {"xyChart": {"showDataLabel": true}}}%%
xychart horizontal
x-axis [A, B]
y-axis 0 --> 10
line [4, 8]
"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("horizontal xychart line plot should render with terminal value disclosure");

    assert_eq!(
        rendered,
        concat!(
            "values: * Line 1: A=4, B=8\n",
            "A +   *-+\n",
            "B +     +-*\n",
            "  ++--------+\n",
            "   0       10\n",
        )
    );
}

#[test]
fn xychart_parser_multiseries_data_labels_use_terminal_value_disclosure() {
    let rendered = render_xychart(
        r#"%%{init: {"xyChart": {"showDataLabel": true}}}%%
xychart
x-axis [A, B]
y-axis 0 --> 10
bar "Revenue" [2, 8]
line "Forecast" [8, 2]
"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("multi-series xychart should render with terminal value disclosure");

    assert_eq!(
        rendered,
        concat!(
            "# Revenue  * Forecast\n",
            "values: # Revenue: A=2, B=8\n",
            "values: * Forecast: A=8, B=2\n",
            "10 +\n",
            " 8 + *-+###\n",
            " 6 +   |###\n",
            " 4 +   |###\n",
            " 2 +###+-*#\n",
            " 0 +-+---+-\n",
            "     A   B\n",
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
            "8 +     ####\n",
            "6 +     ####\n",
            "4 +#### ####\n",
            "2 +#### ####\n",
            "0 +--+----+-\n",
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
        concat!("A +##\n", "B +####\n", "  ++---+\n", "   0  10\n",)
    );
}

#[test]
fn xychart_parser_vertical_bar_shows_data_labels_when_enabled() {
    let rendered = render_xychart(
        r#"%%{init: {"xyChart": {"showDataLabel": true}}}%%
xychart
x-axis [A, B]
y-axis 0 --> 10
bar [4, 8]
"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("vertical xychart with data labels should render");

    assert_eq!(
        rendered,
        concat!(
            "10 +\n",
            " 8 +     8\n",
            " 6 +    ###\n",
            " 4 + 4  ###\n",
            " 2 +### ###\n",
            " 0 +-+---+-\n",
            "     A   B\n",
        )
    );
}

#[test]
fn xychart_parser_vertical_bar_can_place_data_labels_outside_bars() {
    let rendered = render_xychart(
        r#"%%{init: {"xyChart": {"showDataLabel": true, "showDataLabelOutsideBar": true}}}%%
xychart
x-axis [A, B]
y-axis 0 --> 10
bar [4, 8]
"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("vertical xychart with outside data labels should render");

    let mut lines = rendered.lines();
    assert_eq!(lines.next(), Some("     4   8"));
    assert_eq!(
        rendered,
        concat!(
            "     4   8\n",
            "10 +\n",
            " 8 +    ###\n",
            " 6 +    ###\n",
            " 4 +### ###\n",
            " 2 +### ###\n",
            " 0 +-+---+-\n",
            "     A   B\n",
        )
    );
}

#[test]
fn xychart_parser_respects_title_and_axis_visibility_config() {
    let rendered = render_xychart(
        r#"%%{init: {"xyChart": {"showTitle": false, "xAxis": {"showLabel": false, "showTitle": false, "showTick": false, "showAxisLine": false}, "yAxis": {"showLabel": false, "showTitle": false, "showTick": false, "showAxisLine": false}}}}%%
xychart
title "Sales"
x-axis "Month" [A, B]
y-axis "Revenue" 0 --> 10
bar [4, 8]
"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("xychart with hidden titles and axes should render");

    assert!(rendered.contains("###"));
    for hidden in ["Sales", "Month", "Revenue", "A", "B", "|", "+", "-"] {
        assert!(
            !rendered.contains(hidden),
            "hidden token {hidden:?} should not be rendered:\n{rendered}"
        );
    }
}

#[test]
fn xychart_plot_area_respects_typed_grid_limit() {
    let options = AsciiRenderOptions::ascii()
        .with_resource_limit(AsciiResourceLimitId::MaxGridCells, 3)
        .expect("valid grid limit");

    let err = render_xychart(
        r#"xychart
x-axis [A, B]
y-axis 0 --> 10
bar [4, 8]
"#,
        &options,
    )
    .expect_err("xychart plot area should respect max_ascii_grid_cells");

    let AsciiError::ResourceLimitExceeded(details) = err else {
        panic!("expected a typed resource-limit error, got {err:?}");
    };
    assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
    assert_eq!(details.actual, 35);
    assert_eq!(details.max, 3);
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
            "5 ┤    ███\n",
            "4 ┤    ███\n",
            "3 ┤    ███\n",
            "2 ┤███ ███\n",
            "1 ┤███ ███\n",
            "0 ┼─┬───┬─\n",
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
            "10 +\n",
            " 8 +\n",
            " 6 +###\n",
            " 4 +###\n",
            " 2 +###\n",
            " 0 +-+-\n",
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
    let input = read_local_semantic_fixture("xychart/mixed_small.mmd");

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

#[test]
fn xychart_local_semantic_fixture_covers_horizontal_mixed_plot_with_cjk_labels() {
    let input = read_local_semantic_fixture("xychart/horizontal_mixed_cjk.mmd");

    let rendered = render_xychart(&input, &AsciiRenderOptions::ascii())
        .expect("local semantic xychart fixture should render");

    for expected in ["营收", "北区", "南区", "东区", "分数", "Bar 1", "Line 1"] {
        assert!(
            rendered.contains(expected),
            "local semantic xychart fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        first_line_index_containing(&rendered, "营收")
            < first_line_index_containing(&rendered, "y: 分数"),
        "title should render above the axis title:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .find(|line| line.contains("Bar 1") && line.contains("Line 1"))
            .is_some_and(|line| line.find("Bar 1") < line.find("Line 1")),
        "legend should preserve series order on the same row:\n{rendered}"
    );
    let category_row = |category: &str| {
        rendered
            .lines()
            .position(|line| line.starts_with(category))
            .unwrap_or_else(|| panic!("missing category row {category:?}:\n{rendered}"))
    };
    assert!(
        category_row("北区") < category_row("南区") && category_row("南区") < category_row("东区"),
        "CJK category labels should keep their row order:\n{rendered}"
    );
    assert!(
        rendered.lines().count() >= 6,
        "local semantic xychart fixture should produce a multi-line layout:\n{rendered}"
    );
}

#[test]
fn xychart_imported_fixture_matrix_remains_smoke_green() {
    let fixture_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/xychart");
    let mut fixtures = std::fs::read_dir(&fixture_dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", fixture_dir.display()))
        .map(|entry| {
            entry
                .expect("fixture directory entry should be readable")
                .path()
        })
        .filter(|path| path.extension().is_some_and(|extension| extension == "mmd"))
        .collect::<Vec<_>>();
    fixtures.sort();
    assert_eq!(
        fixtures.len(),
        73,
        "the pinned XYChart fixture inventory changed; update the semantic gate intentionally"
    );

    let mut failures = Vec::new();
    for fixture in fixtures {
        let input = std::fs::read_to_string(&fixture)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", fixture.display()));
        if let Err(error) = render_xychart(&input, &AsciiRenderOptions::ascii()) {
            failures.push(format!(
                "{}: {error}",
                fixture
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("<non-utf8 fixture>")
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "imported XYChart fixtures failed ASCII smoke rendering:\n{}",
        failures.join("\n")
    );
}

#[test]
fn xychart_typed_data_is_the_coordinate_and_value_source_of_truth() {
    let mut model = typed_xychart_model(
        "vertical",
        XyChartAxisRenderModel::Linear {
            title: String::new(),
            min: Some(0.0),
            max: Some(10.0),
        },
        -3.0,
        1.0,
        vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Line,
            title: Some("precision".to_string()),
            values: vec![999.0, 998.0, 997.0],
            data: vec![
                ("0".to_string(), Some(0.001)),
                ("1".to_string(), Some(0.75)),
                ("10".to_string(), Some(-2.5)),
            ],
            point_labels: vec!["tiny".to_string(), "ratio".to_string(), "loss".to_string()],
        }],
    );
    model.display.show_data_label = true;

    let rendered = render_typed_xychart(&model, &AsciiRenderOptions::ascii())
        .expect("typed XYChart data should render");

    for expected in ["0=0.001 [tiny]", "1=0.75 [ratio]", "10=-2.5 [loss]"] {
        assert!(
            rendered.contains(expected),
            "typed sample {expected:?} should survive exact disclosure:\n{rendered}"
        );
    }
    assert!(
        !rendered.contains("999") && !rendered.contains("998") && !rendered.contains("997"),
        "legacy values must not override typed data samples:\n{rendered}"
    );
}

#[test]
fn xychart_linear_x_coordinates_control_terminal_point_spacing() {
    let make_model = |second_x: &str| {
        typed_xychart_model(
            "vertical",
            XyChartAxisRenderModel::Linear {
                title: String::new(),
                min: Some(0.0),
                max: Some(10.0),
            },
            0.0,
            1.0,
            vec![XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Line,
                title: None,
                values: vec![0.5, 0.5],
                data: vec![
                    ("0".to_string(), Some(0.5)),
                    (second_x.to_string(), Some(0.5)),
                ],
                point_labels: Vec::new(),
            }],
        )
    };
    let options = AsciiRenderOptions::ascii()
        .with_xychart_vertical_plot_height(3)
        .with_xychart_category_band_width(3);

    let near = render_typed_xychart(&make_model("2"), &options)
        .expect("near linear samples should render");
    let far = render_typed_xychart(&make_model("10"), &options)
        .expect("far linear samples should render");
    let near_width = near.lines().map(str::len).max().unwrap_or_default();
    let far_width = far.lines().map(str::len).max().unwrap_or_default();

    assert!(
        far_width > near_width,
        "typed x coordinates should change point spacing (near={near:?}, far={far:?})"
    );
}

#[test]
fn xychart_horizontal_line_series_draw_connected_paths() {
    let model = typed_xychart_model(
        "horizontal",
        XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A".to_string(), "B".to_string(), "C".to_string()],
        },
        0.0,
        10.0,
        vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Line,
            title: None,
            values: vec![2.0, 8.0, 4.0],
            data: vec![
                ("A".to_string(), Some(2.0)),
                ("B".to_string(), Some(8.0)),
                ("C".to_string(), Some(4.0)),
            ],
            point_labels: Vec::new(),
        }],
    );

    let rendered = render_typed_xychart(&model, &AsciiRenderOptions::ascii())
        .expect("horizontal line series should render");
    let painted_cells = rendered
        .chars()
        .filter(|ch| matches!(ch, '*' | '-' | '|' | '+'))
        .count();

    assert!(
        painted_cells > 3,
        "three samples should be joined by path cells, not emitted as isolated points:\n{rendered}"
    );
    assert!(
        rendered.contains('-') && rendered.contains('+'),
        "horizontal line topology should expose segments and bends:\n{rendered}"
    );
}

#[test]
fn xychart_horizontal_linear_axis_labels_use_authored_sample_coordinates() {
    let mut model = typed_xychart_model(
        "horizontal",
        XyChartAxisRenderModel::Linear {
            title: String::new(),
            min: Some(0.0),
            max: Some(10.0),
        },
        0.0,
        10.0,
        vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Line,
            title: None,
            values: vec![2.0, 5.0, 8.0],
            data: vec![
                ("0".to_string(), Some(2.0)),
                ("4".to_string(), Some(5.0)),
                ("10".to_string(), Some(8.0)),
            ],
            point_labels: Vec::new(),
        }],
    );
    model.display.x_axis = XyChartAxisDisplayPolicy::default();

    let rendered = render_typed_xychart(&model, &AsciiRenderOptions::ascii())
        .expect("horizontal linear samples should retain their authored x labels");
    let labels = rendered
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect::<Vec<_>>();

    assert!(
        labels.contains(&"4"),
        "the x=4 sample was mislabeled:\n{rendered}"
    );
    assert!(
        !labels.contains(&"5"),
        "a generated midpoint tick must not replace the authored x=4 coordinate:\n{rendered}"
    );
    assert!(
        !rendered.contains("values:"),
        "a simple lossless horizontal plot should not need disclosure:\n{rendered}"
    );
}

#[test]
fn xychart_empty_band_domain_discloses_authored_sample_coordinates() {
    for orientation in ["vertical", "horizontal"] {
        let render = |x: &str| {
            let model = typed_xychart_model(
                orientation,
                XyChartAxisRenderModel::Band {
                    title: String::new(),
                    categories: Vec::new(),
                },
                0.0,
                10.0,
                vec![XyChartPlotRenderModel {
                    plot_type: XyChartPlotType::Line,
                    title: None,
                    values: vec![5.0],
                    data: vec![(x.to_string(), Some(5.0))],
                    point_labels: Vec::new(),
                }],
            );
            render_typed_xychart(&model, &AsciiRenderOptions::ascii())
                .unwrap_or_else(|error| panic!("{orientation} empty Band should render: {error}"))
        };

        let alpha = render("alpha");
        let beta = render("beta");
        assert!(
            alpha.contains("alpha=5"),
            "{orientation} output must disclose the authored x coordinate:\n{alpha}"
        );
        assert!(
            beta.contains("beta=5"),
            "{orientation} output must disclose the authored x coordinate:\n{beta}"
        );
        assert_ne!(
            alpha, beta,
            "distinct authored x coordinates must not collapse to the same output"
        );
    }
}

#[test]
fn xychart_horizontal_unequal_linear_series_share_exact_sample_labels() {
    let mut model = typed_xychart_model(
        "horizontal",
        XyChartAxisRenderModel::Linear {
            title: String::new(),
            min: Some(0.0),
            max: Some(10.0),
        },
        0.0,
        10.0,
        vec![
            XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Line,
                title: Some("Sparse".to_string()),
                values: vec![1.0, 9.0],
                data: vec![("0".to_string(), Some(1.0)), ("10".to_string(), Some(9.0))],
                point_labels: Vec::new(),
            },
            XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Line,
                title: Some("Dense".to_string()),
                values: vec![2.0, 4.0, 6.0, 8.0],
                data: vec![
                    ("0".to_string(), Some(2.0)),
                    ("3".to_string(), Some(4.0)),
                    ("7".to_string(), Some(6.0)),
                    ("10".to_string(), Some(8.0)),
                ],
                point_labels: Vec::new(),
            },
        ],
    );
    model.display.x_axis = XyChartAxisDisplayPolicy::default();

    let rendered = render_typed_xychart(&model, &AsciiRenderOptions::ascii())
        .expect("unequal linear series should share an exact horizontal axis");
    let first_tokens = rendered
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect::<Vec<_>>();

    for expected in ["0", "3", "7", "10"] {
        assert!(
            first_tokens.contains(&expected),
            "missing authored x={expected} row label:\n{rendered}"
        );
    }
    for expected in [
        "values: * Sparse: 0=1, 10=9",
        "values: * Dense: 0=2, 3=4, 7=6, 10=8",
    ] {
        assert!(rendered.contains(expected), "{rendered}");
    }
}

#[test]
fn xychart_unicode_line_topology_resolves_rounded_corners() {
    let model = typed_xychart_model(
        "vertical",
        XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A".to_string(), "B".to_string()],
        },
        0.0,
        10.0,
        vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Line,
            title: None,
            values: vec![2.0, 8.0],
            data: vec![("A".to_string(), Some(2.0)), ("B".to_string(), Some(8.0))],
            point_labels: Vec::new(),
        }],
    );

    let rendered = render_typed_xychart(&model, &AsciiRenderOptions::unicode())
        .expect("Unicode line topology should render");

    for glyph in ['╭', '╯', '─', '│', '●'] {
        assert!(
            rendered.contains(glyph),
            "Unicode line topology should contain {glyph:?}:\n{rendered}"
        );
    }
}

#[test]
fn xychart_line_topology_temporary_extent_is_budgeted() {
    let model = typed_xychart_model(
        "vertical",
        XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A".to_string(), "B".to_string()],
        },
        0.0,
        10.0,
        vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Line,
            title: None,
            values: vec![2.0, 8.0],
            data: Vec::new(),
            point_labels: Vec::new(),
        }],
    );
    let options = AsciiRenderOptions::ascii()
        .with_xychart_vertical_plot_height(3)
        .with_xychart_category_band_width(3)
        .with_resource_limit(AsciiResourceLimitId::MaxGridCells, 41)
        .expect("valid grid limit");

    let error = render_typed_xychart(&model, &options)
        .expect_err("plot cells plus the line-topology mask must exceed 41 cells");
    let AsciiError::ResourceLimitExceeded(details) = error else {
        panic!("expected resource-limit error, got {error:?}");
    };
    assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
    assert_eq!(details.actual, 42);
    assert_eq!(details.max, 41);
}

#[test]
fn xychart_horizontal_grouped_bars_get_independent_rows() {
    let model = typed_xychart_model(
        "horizontal",
        XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A".to_string(), "B".to_string()],
        },
        0.0,
        10.0,
        vec![
            XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Bar,
                title: Some("first".to_string()),
                values: vec![2.0, 8.0],
                data: vec![("A".to_string(), Some(2.0)), ("B".to_string(), Some(8.0))],
                point_labels: Vec::new(),
            },
            XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Bar,
                title: Some("second".to_string()),
                values: vec![7.0, 3.0],
                data: vec![("A".to_string(), Some(7.0)), ("B".to_string(), Some(3.0))],
                point_labels: Vec::new(),
            },
        ],
    );

    let rendered = render_typed_xychart(&model, &AsciiRenderOptions::ascii())
        .expect("grouped horizontal bars should render");
    let bar_rows = rendered
        .lines()
        .filter(|line| !line.is_empty() && line.chars().all(|ch| ch == '#'))
        .count();

    assert_eq!(
        bar_rows, 4,
        "two bar series across two categories need four recoverable lanes:\n{rendered}"
    );
}

#[test]
fn xychart_grouped_horizontal_extent_is_budgeted_before_allocation() {
    let model = typed_xychart_model(
        "horizontal",
        XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A".to_string(), "B".to_string()],
        },
        0.0,
        10.0,
        vec![
            XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Bar,
                title: None,
                values: vec![2.0, 8.0],
                data: Vec::new(),
                point_labels: Vec::new(),
            },
            XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Bar,
                title: None,
                values: vec![7.0, 3.0],
                data: Vec::new(),
                point_labels: Vec::new(),
            },
        ],
    );
    let options = AsciiRenderOptions::ascii()
        .with_xychart_horizontal_plot_width(10)
        .with_resource_limit(AsciiResourceLimitId::MaxGridCells, 39)
        .expect("valid grid limit");

    let error = render_typed_xychart(&model, &options)
        .expect_err("the four-by-ten grouped plot must exceed 39 grid cells");
    let AsciiError::ResourceLimitExceeded(details) = error else {
        panic!("expected resource-limit error, got {error:?}");
    };
    assert_eq!(details.limit, AsciiResourceLimitId::MaxGridCells);
    assert_eq!(details.actual, 40);
    assert_eq!(details.max, 39);
}

#[test]
fn xychart_vertical_grouped_bars_use_distinct_category_lanes() {
    let model = typed_xychart_model(
        "vertical",
        XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A".to_string()],
        },
        0.0,
        10.0,
        vec![
            XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Bar,
                title: Some("full".to_string()),
                values: vec![10.0],
                data: vec![("A".to_string(), Some(10.0))],
                point_labels: Vec::new(),
            },
            XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Bar,
                title: Some("half".to_string()),
                values: vec![5.0],
                data: vec![("A".to_string(), Some(5.0))],
                point_labels: Vec::new(),
            },
        ],
    );
    let options = AsciiRenderOptions::ascii()
        .with_xychart_vertical_plot_height(4)
        .with_xychart_category_band_width(4);

    let rendered =
        render_typed_xychart(&model, &options).expect("grouped vertical bars should render");
    let plot_rows = rendered
        .lines()
        .filter(|line| !line.is_empty() && line.chars().all(|ch| ch == '#'))
        .collect::<Vec<_>>();

    assert_eq!(
        plot_rows,
        vec!["##", "##", "####", "####"],
        "bar series should occupy independent lanes inside the category band:\n{rendered}"
    );
}

#[test]
fn xychart_tiny_axis_ticks_remain_distinct_and_scale_aware() {
    let rendered = render_xychart(
        r#"xychart
x-axis [A, B]
y-axis 0.001 --> 0.005
line [0.001, 0.005]
"#,
        &AsciiRenderOptions::ascii(),
    )
    .expect("tiny XYChart range should render");

    assert!(
        rendered.contains("0.005"),
        "missing exact upper tick:\n{rendered}"
    );
    assert!(
        rendered.contains("0.001"),
        "missing exact lower tick:\n{rendered}"
    );
    let visible_ticks = rendered
        .lines()
        .take(5)
        .filter_map(|line| line.split_whitespace().next())
        .collect::<Vec<_>>();
    assert_eq!(
        visible_ticks.len(),
        5,
        "compact value axis should retain five tick rows:\n{rendered}"
    );
    assert!(
        visible_ticks.windows(2).all(|pair| pair[0] != pair[1]),
        "scale-aware formatting must not collapse adjacent ticks:\n{rendered}"
    );
}

#[test]
fn xychart_degenerate_range_keeps_data_visible() {
    let model = typed_xychart_model(
        "vertical",
        XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A".to_string()],
        },
        5.0,
        5.0,
        vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Bar,
            title: None,
            values: vec![5.0],
            data: vec![("A".to_string(), Some(5.0))],
            point_labels: Vec::new(),
        }],
    );

    let options = AsciiRenderOptions::ascii().with_xychart_vertical_plot_height(4);
    let rendered = render_typed_xychart(&model, &options)
        .expect("degenerate XYChart range should render deterministically");

    assert_eq!(
        rendered.lines().filter(|line| line.contains('#')).count(),
        2,
        "a degenerate linear scale should place its bar at the domain midpoint:\n{rendered}"
    );
}

#[test]
fn xychart_reversed_value_range_preserves_authored_axis_direction() {
    let mut model = typed_xychart_model(
        "vertical",
        XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A".to_string(), "B".to_string()],
        },
        10.0,
        0.0,
        vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Line,
            title: None,
            values: vec![10.0, 0.0],
            data: vec![("A".to_string(), Some(10.0)), ("B".to_string(), Some(0.0))],
            point_labels: Vec::new(),
        }],
    );
    model.display.x_axis = XyChartAxisDisplayPolicy::default();
    model.display.y_axis = XyChartAxisDisplayPolicy::default();

    let rendered = render_typed_xychart(&model, &AsciiRenderOptions::ascii())
        .expect("reversed XYChart range should render");
    let lines = rendered.lines().collect::<Vec<_>>();

    assert!(
        lines
            .first()
            .is_some_and(|line| line.trim_start().starts_with("0 +")),
        "the authored `0` endpoint should remain at the top of a reversed axis:\n{rendered}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.trim_start().starts_with("10 +")),
        "the authored `10` endpoint should remain on the baseline:\n{rendered}"
    );
}

#[test]
fn xychart_orphan_point_labels_and_clipped_values_are_disclosed() {
    let model = typed_xychart_model(
        "vertical",
        XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A".to_string()],
        },
        0.0,
        1.0,
        vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Line,
            title: None,
            values: vec![2.0],
            data: vec![("A".to_string(), Some(2.0))],
            point_labels: vec!["peak".to_string(), "detached".to_string()],
        }],
    );

    let rendered = render_typed_xychart(&model, &AsciiRenderOptions::ascii())
        .expect("clipped and orphan XYChart semantics should render through disclosure");

    assert!(
        rendered.contains("A=2 (clipped) [peak]"),
        "clipped value and anchored point label should remain exact:\n{rendered}"
    );
    assert!(
        rendered.contains("orphan-label=detached"),
        "orphan point labels must not disappear silently:\n{rendered}"
    );
}

#[test]
fn xychart_duplicate_categories_keep_source_order_and_disclosure_identity() {
    let model = typed_xychart_model(
        "vertical",
        XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["B".to_string(), "A".to_string(), "A".to_string()],
        },
        0.0,
        3.0,
        vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Line,
            title: None,
            values: vec![0.0, 1.0, 2.0],
            data: vec![
                ("B".to_string(), Some(0.0)),
                ("A".to_string(), Some(1.0)),
                ("A".to_string(), Some(2.0)),
            ],
            point_labels: Vec::new(),
        }],
    );

    let rendered = render_typed_xychart(&model, &AsciiRenderOptions::ascii())
        .expect("duplicate categories should render deterministically");

    assert!(
        rendered.contains("B=0, A[1]=1, A[2]=2"),
        "duplicate categories need stable occurrence identities:\n{rendered}"
    );
    let plotted_points = rendered
        .lines()
        .filter(|line| !line.starts_with("values:"))
        .map(|line| line.matches('*').count())
        .sum::<usize>();
    assert_eq!(
        plotted_points, 3,
        "duplicate category samples must retain separate source-order slots:\n{rendered}"
    );
}

#[test]
fn xychart_missing_samples_break_paths_and_remain_explicit() {
    for orientation in ["vertical", "horizontal"] {
        let model = typed_xychart_model(
            orientation,
            XyChartAxisRenderModel::Band {
                title: String::new(),
                categories: vec!["A".to_string(), "B".to_string(), "C".to_string()],
            },
            0.0,
            10.0,
            vec![XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Line,
                title: None,
                values: vec![2.0, 8.0],
                data: vec![
                    ("A".to_string(), Some(2.0)),
                    ("B".to_string(), None),
                    ("C".to_string(), Some(8.0)),
                ],
                point_labels: Vec::new(),
            }],
        );

        let rendered = render_typed_xychart(&model, &AsciiRenderOptions::ascii())
            .unwrap_or_else(|error| panic!("{orientation} sparse line should render: {error}"));
        assert!(
            rendered.contains("B=n/a"),
            "{orientation} sparse line must disclose the missing sample:\n{rendered}"
        );
        let plot = rendered
            .lines()
            .filter(|line| !line.starts_with("values:"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            plot.matches('*').count(),
            2,
            "{orientation} sparse line should retain both defined points:\n{rendered}"
        );
        assert!(
            !plot.chars().any(|ch| matches!(ch, '-' | '|' | '+')),
            "{orientation} sparse line must not bridge across a missing sample:\n{rendered}"
        );
    }
}

#[test]
fn xychart_quantized_line_collision_triggers_exact_disclosure() {
    let model = typed_xychart_model(
        "vertical",
        XyChartAxisRenderModel::Linear {
            title: String::new(),
            min: Some(0.0),
            max: Some(100.0),
        },
        0.0,
        10.0,
        vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Line,
            title: None,
            values: vec![5.0, 5.0],
            data: vec![("0".to_string(), Some(5.0)), ("0.1".to_string(), Some(5.0))],
            point_labels: Vec::new(),
        }],
    );

    let rendered = render_typed_xychart(&model, &AsciiRenderOptions::ascii())
        .expect("quantized line collision should render through disclosure");

    assert!(
        rendered.contains("0=5, 0.1=5"),
        "colliding typed coordinates must remain exact:\n{rendered}"
    );
    let plotted_points = rendered
        .lines()
        .filter(|line| !line.starts_with("values:"))
        .map(|line| line.matches('*').count())
        .sum::<usize>();
    assert_eq!(
        plotted_points, 1,
        "the probe should exercise one quantized terminal cell:\n{rendered}"
    );
}

#[test]
fn xychart_dense_linear_bar_overlap_triggers_exact_disclosure() {
    let model = typed_xychart_model(
        "vertical",
        XyChartAxisRenderModel::Linear {
            title: String::new(),
            min: Some(0.0),
            max: Some(100.0),
        },
        0.0,
        10.0,
        vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Bar,
            title: None,
            values: vec![5.0, 8.0],
            data: vec![("0".to_string(), Some(5.0)), ("20".to_string(), Some(8.0))],
            point_labels: Vec::new(),
        }],
    );

    let rendered = render_typed_xychart(&model, &AsciiRenderOptions::ascii())
        .expect("dense linear bars should render through disclosure");

    assert!(
        rendered.contains("0=5, 20=8"),
        "overlapping bar identities must remain exact:\n{rendered}"
    );
}

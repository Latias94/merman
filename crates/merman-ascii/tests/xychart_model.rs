mod support;

use merman_ascii::{
    AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiError, AsciiRenderOptions,
    AsciiResourceLimitId, AsciiResourcePolicy, AsciiRgb,
};
use merman_core::diagrams::xychart::{
    XyChartAxisDisplayPolicy, XyChartAxisRenderModel, XyChartDiagramRenderModel,
    XyChartDisplayPolicy, XyChartPlotRenderModel, XyChartPlotType,
};
use merman_core::{Engine, OperationControl, ParseOptions, RenderSemanticModel};
use std::path::Path;
use support::{render_controlled_model, render_model_with_resources};

fn render_xychart(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    render_xychart_with_resources(input, options, AsciiResourcePolicy::default())
}

fn render_xychart_with_resources(
    input: &str,
    options: &AsciiRenderOptions,
    resources: AsciiResourcePolicy,
) -> merman_ascii::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("xychart should parse")
        .expect("xychart should be detected");

    render_model_with_resources(parsed.model(), options, resources)
}

fn render_typed_xychart(
    model: &XyChartDiagramRenderModel,
    options: &AsciiRenderOptions,
) -> merman_ascii::Result<String> {
    render_typed_xychart_with_resources(model, options, AsciiResourcePolicy::default())
}

fn render_typed_xychart_with_resources(
    model: &XyChartDiagramRenderModel,
    options: &AsciiRenderOptions,
    resources: AsciiResourcePolicy,
) -> merman_ascii::Result<String> {
    render_model_with_resources(
        &RenderSemanticModel::XyChart(model.clone()),
        options,
        resources,
    )
}

fn render_xychart_with_grid_limit(
    input: &str,
    options: &AsciiRenderOptions,
    max_grid_cells: usize,
) -> merman_ascii::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("xychart should parse")
        .expect("xychart should be detected");
    let control = OperationControl::new();
    let context = Engine::new()
        .begin_operation()
        .expect("deterministic operation context should be available");
    let resources = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxGridCells, max_grid_cells)
        .expect("valid grid limit");

    render_controlled_model(parsed.model(), options, &control, &context, resources)
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
        let mut decoded_entity = false;
        for (entity, decoded) in [
            ("&quot;", '"'),
            ("&#39;", '\''),
            ("&gt;", '>'),
            ("&lt;", '<'),
            ("&amp;", '&'),
        ] {
            if rest.starts_with(entity) {
                output.push(decoded);
                index += entity.len();
                decoded_entity = true;
                break;
            }
        }
        if decoded_entity {
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
            "titleDisplay: chart(bytes=5)=\"Sales\"\n",
            "titleDisplay: yAxis(bytes=7)=\"Revenue\"\n",
            "10 +\n",
            " 8 +        ###\n",
            " 6 +    ### ###\n",
            " 4 +    ### ###\n",
            " 2 +### ### ###\n",
            " 0 +-+---+---+-\n",
            "    Jan Feb Mar\n",
            "titleDisplay: xAxis(bytes=5)=\"Month\"\n",
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
            "xDomain: band categories=[bytes=1=\"A\", bytes=1=\"B\"]\n",
            "values: # series=0 type=bar title=none samples=[{index=0 x(bytes=1)=\"A\" value=2 pointLabel=none clipped=false}, {index=1 x(bytes=1)=\"B\" value=8 pointLabel=none clipped=false}] orphanPointLabels=[]\n",
            "values: * series=1 type=line title=none samples=[{index=0 x(bytes=1)=\"A\" value=8 pointLabel=none clipped=false}, {index=1 x(bytes=1)=\"B\" value=2 pointLabel=none clipped=false}] orphanPointLabels=[]\n",
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
            "titleDisplay: chart(bytes=5)=\"Sales\"\n",
            "titleDisplay: yAxis(bytes=7)=\"Revenue\"\n",
            "10 +\n",
            " 8 +        ###\n",
            " 6 +    ### ###\n",
            " 4 +    ### ###\n",
            " 2 +### ### ###\n",
            " 0 +-+---+---+-\n",
            "    Jan Feb Mar\n",
            "titleDisplay: xAxis(bytes=5)=\"Month\"\n",
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
            "xDomain: band categories=[bytes=1=\"A\", bytes=1=\"B\", bytes=1=\"C\"]\n",
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
            "xDomain: band categories=[bytes=1=\"A\", bytes=1=\"B\"]\n",
            "values: # series=0 type=bar title=none samples=[{index=0 x(bytes=1)=\"A\" value=2 pointLabel=none clipped=false}, {index=1 x(bytes=1)=\"B\" value=8 pointLabel=none clipped=false}] orphanPointLabels=[]\n",
            "values: * series=1 type=line title=none samples=[{index=0 x(bytes=1)=\"A\" value=8 pointLabel=none clipped=false}, {index=1 x(bytes=1)=\"B\" value=2 pointLabel=none clipped=false}] orphanPointLabels=[]\n",
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
            "values: * series=0 type=line title=none samples=[{index=0 x(bytes=1)=\"A\" value=4 pointLabel=none clipped=false}, {index=1 x(bytes=1)=\"B\" value=8 pointLabel=none clipped=false}] orphanPointLabels=[]\n",
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
            "xDomain: band categories=[bytes=1=\"A\", bytes=1=\"B\"]\n",
            "values: # series=0 type=bar title(bytes=7)=\"Revenue\" samples=[{index=0 x(bytes=1)=\"A\" value=2 pointLabel=none clipped=false}, {index=1 x(bytes=1)=\"B\" value=8 pointLabel=none clipped=false}] orphanPointLabels=[]\n",
            "values: * series=1 type=line title(bytes=8)=\"Forecast\" samples=[{index=0 x(bytes=1)=\"A\" value=8 pointLabel=none clipped=false}, {index=1 x(bytes=1)=\"B\" value=2 pointLabel=none clipped=false}] orphanPointLabels=[]\n",
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
            "xDomain: band categories=[bytes=3=\"Jan\", bytes=3=\"Feb\"]\n",
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
            "xDomain: band categories=[bytes=1=\"A\", bytes=1=\"B\"]\n",
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
    assert_eq!(
        lines.next(),
        Some("xDomain: band categories=[bytes=1=\"A\", bytes=1=\"B\"]")
    );
    assert_eq!(lines.next(), Some("     4   8"));
    assert_eq!(
        rendered,
        concat!(
            "xDomain: band categories=[bytes=1=\"A\", bytes=1=\"B\"]\n",
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
fn xychart_plot_area_respects_resource_limits() {
    let options = AsciiRenderOptions::ascii();

    let err = render_xychart_with_grid_limit(
        r#"xychart
x-axis [A, B]
y-axis 0 --> 10
bar [4, 8]
"#,
        &options,
        3,
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
            "xDomain: band categories=[bytes=1=\"A\", bytes=1=\"B\"]\n",
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
        .find(|line| !line.starts_with("xDomain:") && line.contains('中'))
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
fn xychart_parser_header_only_preserves_empty_chart_state() {
    let rendered = render_xychart("xychart", &AsciiRenderOptions::ascii())
        .expect("empty xychart should render");

    assert_eq!(
        rendered,
        concat!(
            "xychart: empty\n",
            "orientation: vertical\n",
            "xAxis: band title(bytes=0)=\"\" categories=[]\n",
            "yAxis: linear title(bytes=0)=\"\" min=none max=none\n",
            "display: showTitle=true showDataLabel=false showDataLabelOutsideBar=false xAxis={showLabel=true showTitle=true showTick=true showAxisLine=true} yAxis={showLabel=true showTitle=true showTick=true showAxisLine=true}\n",
            "plots: []",
        )
    );
}

#[test]
fn xychart_parser_empty_chart_preserves_title_and_axis_semantics() {
    let rendered = render_xychart(
        concat!(
            "xychart horizontal\n",
            "title Lost\n",
            "x-axis [alpha, beta]\n",
            "y-axis Score 0 --> 10\n",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("empty authored xychart should render");

    assert_eq!(
        rendered,
        concat!(
            "xychart: empty\n",
            "orientation: horizontal\n",
            "title(bytes=4)=\"Lost\"\n",
            "xAxis: band title(bytes=0)=\"\" categories=[bytes=5 \"alpha\", bytes=4 \"beta\"]\n",
            "yAxis: linear title(bytes=5)=\"Score\" min=0 max=10\n",
            "display: showTitle=true showDataLabel=false showDataLabelOutsideBar=false xAxis={showLabel=true showTitle=true showTick=true showAxisLine=true} yAxis={showLabel=true showTitle=true showTick=true showAxisLine=true}\n",
            "plots: []",
        )
    );
}

#[test]
fn xychart_empty_plot_preserves_typed_series_metadata() {
    let model = XyChartDiagramRenderModel {
        orientation: "vertical".to_string(),
        title: Some("No samples".to_string()),
        acc_title: None,
        acc_descr: None,
        x_axis: XyChartAxisRenderModel::Band {
            title: "Category".to_string(),
            categories: Vec::new(),
        },
        y_axis: XyChartAxisRenderModel::Linear {
            title: "Score".to_string(),
            min: Some(0.0),
            max: Some(10.0),
        },
        plots: vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Line,
            title: Some("Forecast".to_string()),
            values: Vec::new(),
            data: Vec::new(),
            point_labels: vec!["orphan".to_string()],
        }],
        display: XyChartDisplayPolicy::default(),
    };

    let rendered = render_typed_xychart(&model, &AsciiRenderOptions::ascii())
        .expect("zero-slot typed XYChart should retain its authored fields");

    assert_eq!(
        rendered,
        concat!(
            "xychart: empty\n",
            "orientation: vertical\n",
            "title(bytes=10)=\"No samples\"\n",
            "xAxis: band title(bytes=8)=\"Category\" categories=[]\n",
            "yAxis: linear title(bytes=5)=\"Score\" min=0 max=10\n",
            "display: showTitle=true showDataLabel=false showDataLabelOutsideBar=false xAxis={showLabel=true showTitle=true showTick=true showAxisLine=true} yAxis={showLabel=true showTitle=true showTick=true showAxisLine=true}\n",
            "plots: count=1\n",
            "plot: index=0 type=line title(bytes=8)=\"Forecast\" values=[] data=[] pointLabels=[bytes=6 \"orphan\"]",
        )
    );
}

#[test]
fn xychart_empty_projection_obeys_exact_output_byte_budget() {
    let input = concat!(
        "xychart\n",
        "title 空\n",
        "x-axis [alpha, beta]\n",
        "y-axis Score 0 --> 10\n",
    );
    let rendered = render_xychart(input, &AsciiRenderOptions::ascii())
        .expect("empty XYChart should render under the default budget");
    let exact = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxOutputBytes, rendered.len())
        .expect("valid exact output-byte limit");

    assert_eq!(
        render_xychart_with_resources(input, &AsciiRenderOptions::ascii(), exact)
            .expect("exact empty-chart byte budget should render"),
        rendered
    );

    let below = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxOutputBytes, rendered.len() - 1)
        .expect("valid N-1 output-byte limit");
    let error = render_xychart_with_resources(input, &AsciiRenderOptions::ascii(), below)
        .expect_err("N-1 empty-chart byte budget should reject the final document");
    let AsciiError::ResourceLimitExceeded(details) = error else {
        panic!("expected output-byte resource error, got {error:?}");
    };
    assert_eq!(details.limit, AsciiResourceLimitId::MaxOutputBytes);
    assert_eq!(details.actual, rendered.len());
    assert_eq!(details.max, rendered.len() - 1);
}

#[test]
fn xychart_empty_projection_obeys_exact_document_cell_budget() {
    let input = concat!(
        "xychart horizontal\n",
        "title Lost\n",
        "x-axis [alpha, beta]\n",
        "y-axis Score 0 --> 10\n",
    );
    let rendered = render_xychart(input, &AsciiRenderOptions::ascii())
        .expect("empty XYChart should render under the default budget");
    let exact_cells = rendered.lines().map(str::len).sum::<usize>();
    let exact = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells)
        .expect("valid exact document-cell limit");

    assert_eq!(
        render_xychart_with_resources(input, &AsciiRenderOptions::ascii(), exact)
            .expect("exact empty-chart cell budget should render"),
        rendered
    );

    let below = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxDocumentCells, exact_cells - 1)
        .expect("valid N-1 document-cell limit");
    let error = render_xychart_with_resources(input, &AsciiRenderOptions::ascii(), below)
        .expect_err("N-1 empty-chart cell budget should reject the document");
    let AsciiError::ResourceLimitExceeded(details) = error else {
        panic!("expected document-cell resource error, got {error:?}");
    };
    assert_eq!(details.limit, AsciiResourceLimitId::MaxDocumentCells);
    assert_eq!(details.actual, exact_cells);
    assert_eq!(details.max, exact_cells - 1);
}

#[test]
fn xychart_empty_projection_enforces_authored_grapheme_budget() {
    let grapheme = "👩‍💻";
    let model = XyChartDiagramRenderModel {
        orientation: "vertical".to_string(),
        title: Some(grapheme.to_string()),
        acc_title: None,
        acc_descr: None,
        x_axis: XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: Vec::new(),
        },
        y_axis: XyChartAxisRenderModel::Linear {
            title: String::new(),
            min: None,
            max: None,
        },
        plots: Vec::new(),
        display: XyChartDisplayPolicy::default(),
    };
    let exact = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxGraphemeBytes, grapheme.len())
        .expect("valid exact grapheme-byte limit");

    assert!(
        render_typed_xychart_with_resources(&model, &AsciiRenderOptions::ascii(), exact)
            .expect("exact empty-chart grapheme budget should render")
            .contains(grapheme)
    );

    let below = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxGraphemeBytes, grapheme.len() - 1)
        .expect("valid N-1 grapheme-byte limit");
    let error = render_typed_xychart_with_resources(&model, &AsciiRenderOptions::ascii(), below)
        .expect_err("N-1 empty-chart grapheme budget should reject authored text");
    let AsciiError::ResourceLimitExceeded(details) = error else {
        panic!("expected grapheme resource error, got {error:?}");
    };
    assert_eq!(details.limit, AsciiResourceLimitId::MaxGraphemeBytes);
    assert_eq!(details.actual, grapheme.len());
    assert_eq!(details.max, grapheme.len() - 1);
}

#[test]
fn xychart_empty_projection_html_escapes_authored_fields() {
    let model = XyChartDiagramRenderModel {
        orientation: "vertical".to_string(),
        title: Some("<empty & safe>".to_string()),
        acc_title: None,
        acc_descr: None,
        x_axis: XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: Vec::new(),
        },
        y_axis: XyChartAxisRenderModel::Linear {
            title: String::new(),
            min: None,
            max: None,
        },
        plots: Vec::new(),
        display: XyChartDisplayPolicy::default(),
    };
    let rendered = render_typed_xychart(
        &model,
        &AsciiRenderOptions::ascii().with_color_mode(AsciiColorMode::Html),
    )
    .expect("empty XYChart HTML output should render");

    assert!(rendered.contains("title(bytes=14)=&quot;&lt;empty &amp; safe&gt;&quot;"));
    assert!(!rendered.contains("<empty"));
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
        first_line_index_containing(&rendered, r#"titleDisplay: chart(bytes=6)="营收""#,)
            < first_line_index_containing(&rendered, r#"titleDisplay: yAxis(bytes=6)="分数""#,),
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

    for expected in [
        "x(bytes=1)=\"0\" value=0.001 pointLabel(bytes=4)=\"tiny\"",
        "x(bytes=1)=\"1\" value=0.75 pointLabel(bytes=5)=\"ratio\"",
        "x(bytes=2)=\"10\" value=-2.5 pointLabel(bytes=4)=\"loss\"",
    ] {
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
            alpha.contains("x(bytes=5)=\"alpha\" value=5"),
            "{orientation} output must disclose the authored x coordinate:\n{alpha}"
        );
        assert!(
            beta.contains("x(bytes=4)=\"beta\" value=5"),
            "{orientation} output must disclose the authored x coordinate:\n{beta}"
        );
        assert_ne!(
            alpha, beta,
            "distinct authored x coordinates must not collapse to the same output"
        );
    }
}

#[test]
fn xychart_exact_disclosure_is_injective_for_authored_delimiters() {
    let split_fields = typed_xychart_model(
        "vertical",
        XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A".to_string(), "B".to_string()],
        },
        0.0,
        2.0,
        vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Bar,
            title: None,
            values: vec![0.0, 2.0],
            data: vec![("A".to_string(), Some(0.0)), ("B".to_string(), Some(2.0))],
            point_labels: vec![String::new(), "p".to_string()],
        }],
    );
    let authored_delimiters = typed_xychart_model(
        "vertical",
        XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["unused".to_string(), "A=0, B".to_string()],
        },
        0.0,
        2.0,
        vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Bar,
            title: None,
            values: vec![2.0],
            data: vec![("A=0, B".to_string(), Some(2.0))],
            point_labels: vec!["p".to_string()],
        }],
    );

    let split_fields = render_typed_xychart(&split_fields, &AsciiRenderOptions::ascii())
        .expect("split XYChart fields should render");
    let authored_delimiters =
        render_typed_xychart(&authored_delimiters, &AsciiRenderOptions::ascii())
            .expect("authored XYChart delimiters should render");

    assert_ne!(
        split_fields, authored_delimiters,
        "authored delimiters must not forge disclosure field ownership"
    );
}

#[test]
fn xychart_exact_disclosure_quotes_control_and_field_delimiters() {
    let model = typed_xychart_model(
        "vertical",
        XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A\nB".to_string()],
        },
        0.0,
        2.0,
        vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Line,
            title: Some("series=\"one\"\\two".to_string()),
            values: vec![2.0],
            data: vec![("A\nB".to_string(), Some(2.0))],
            point_labels: vec!["point=\"peak\"\\tail".to_string()],
        }],
    );

    let rendered = render_typed_xychart(&model, &AsciiRenderOptions::ascii())
        .expect("quoted XYChart disclosure should render");

    assert!(
        rendered.contains(r#"title(bytes=16)="series=\"one\"\\two""#),
        "series title was not injectively quoted:\n{rendered}"
    );
    assert!(
        rendered.contains(r#"x(bytes=3)="A\nB""#),
        "structural newline was not escaped inside the framed x field:\n{rendered}"
    );
    assert!(
        rendered.contains(r#"pointLabel(bytes=17)="point=\"peak\"\\tail""#),
        "point label was not injectively quoted:\n{rendered}"
    );
}

#[test]
fn xychart_direct_model_title_owners_prevent_chart_axis_spoofing() {
    let render = |chart_title: Option<&str>, y_title: &str, show_chart: bool, show_y: bool| {
        let mut model = typed_xychart_model(
            "vertical",
            XyChartAxisRenderModel::Band {
                title: String::new(),
                categories: vec!["A".to_string()],
            },
            0.0,
            10.0,
            vec![XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Line,
                title: None,
                values: vec![5.0],
                data: vec![("A".to_string(), Some(5.0))],
                point_labels: Vec::new(),
            }],
        );
        model.title = chart_title.map(str::to_string);
        model.y_axis = XyChartAxisRenderModel::Linear {
            title: y_title.to_string(),
            min: Some(0.0),
            max: Some(10.0),
        };
        model.display.show_title = show_chart;
        model.display.y_axis.show_title = show_y;
        render_typed_xychart(&model, &AsciiRenderOptions::ascii())
            .expect("owned title disclosure should render")
    };

    let chart_owned = render(Some("y: Y"), "", true, false);
    let y_axis_owned = render(None, "Y", false, true);

    assert_ne!(
        chart_owned, y_axis_owned,
        "a chart title must not impersonate a renderer-owned y-axis title row"
    );
    assert!(
        chart_owned
            .lines()
            .any(|line| line == r#"titleDisplay: chart(bytes=4)="y: Y""#),
        "chart title lost its owner or UTF-8 byte frame:\n{chart_owned}"
    );
    assert!(
        y_axis_owned
            .lines()
            .any(|line| line == r#"titleDisplay: yAxis(bytes=1)="Y""#),
        "y-axis title lost its owner or UTF-8 byte frame:\n{y_axis_owned}"
    );
    assert!(
        !chart_owned.lines().any(|line| line == "y: Y"),
        "authored chart text must never be emitted as an unframed owned row:\n{chart_owned}"
    );

    let authored_owned_row = render(Some(r#"titleDisplay: yAxis(bytes=1)="Y""#), "", true, false);
    assert!(
        authored_owned_row
            .contains(r#"titleDisplay: chart(bytes=32)="titleDisplay: yAxis(bytes=1)=\"Y\"""#),
        "renderer-like title text must remain a chart-owned payload without prefix filtering:\n\
         {authored_owned_row}"
    );
}

#[test]
fn xychart_parser_title_owners_prevent_chart_axis_spoofing() {
    let chart_owned = render_xychart(
        concat!(
            "xychart\n",
            "title \"y: Y\"\n",
            "x-axis [A]\n",
            "y-axis 0 --> 10\n",
            "line [5]\n",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("parser chart-title fixture should render");
    let y_axis_owned = render_xychart(
        concat!(
            "xychart\n",
            "x-axis [A]\n",
            "y-axis Y 0 --> 10\n",
            "line [5]\n",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("parser y-axis-title fixture should render");

    assert_ne!(
        chart_owned, y_axis_owned,
        "parser-produced chart and y-axis titles must retain distinct owners"
    );
    assert!(
        chart_owned.contains(r#"titleDisplay: chart(bytes=4)="y: Y""#),
        "parser chart title lost its owner frame:\n{chart_owned}"
    );
    assert!(
        y_axis_owned.contains(r#"titleDisplay: yAxis(bytes=1)="Y""#),
        "parser y-axis title lost its owner frame:\n{y_axis_owned}"
    );
}

#[test]
fn xychart_direct_model_title_frames_preserve_trim_and_esc() {
    let render = |chart_title: &str, x_title: &str, y_title: &str| {
        let mut model = typed_xychart_model(
            "vertical",
            XyChartAxisRenderModel::Band {
                title: x_title.to_string(),
                categories: vec!["A".to_string()],
            },
            0.0,
            10.0,
            vec![XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Line,
                title: None,
                values: vec![5.0],
                data: vec![("A".to_string(), Some(5.0))],
                point_labels: Vec::new(),
            }],
        );
        model.title = Some(chart_title.to_string());
        model.y_axis = XyChartAxisRenderModel::Linear {
            title: y_title.to_string(),
            min: Some(0.0),
            max: Some(10.0),
        };
        model.display.show_title = true;
        model.display.x_axis.show_title = true;
        model.display.y_axis.show_title = true;
        render_typed_xychart(&model, &AsciiRenderOptions::ascii())
            .expect("owned title frames should render")
    };

    let control_and_left_trim = render("\u{1b}", " X", "Y ");
    let authored_escape_and_right_trim = render("\\u{1B}", "X ", " Y");

    assert_ne!(
        control_and_left_trim, authored_escape_and_right_trim,
        "distinct authored titles must not collapse after terminal escaping"
    );
    for expected in [
        r#"titleDisplay: chart(bytes=1)="\u{1B}""#,
        r#"titleDisplay: xAxis(bytes=2)=" X""#,
        r#"titleDisplay: yAxis(bytes=2)="Y ""#,
    ] {
        assert!(
            control_and_left_trim.contains(expected),
            "missing exact owned title frame {expected:?}:\n{control_and_left_trim}"
        );
    }
    for expected in [
        r#"titleDisplay: chart(bytes=6)="\\u{1B}""#,
        r#"titleDisplay: xAxis(bytes=2)="X ""#,
        r#"titleDisplay: yAxis(bytes=2)=" Y""#,
    ] {
        assert!(
            authored_escape_and_right_trim.contains(expected),
            "missing exact authored escape frame {expected:?}:\n{authored_escape_and_right_trim}"
        );
    }
}

#[test]
fn xychart_direct_model_title_frames_distinguish_lf_from_literal_escape() {
    let render = |chart_title: &str| {
        let mut model = typed_xychart_model(
            "vertical",
            XyChartAxisRenderModel::Band {
                title: String::new(),
                categories: vec!["A".to_string()],
            },
            0.0,
            10.0,
            vec![XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Line,
                title: None,
                values: vec![5.0],
                data: vec![("A".to_string(), Some(5.0))],
                point_labels: Vec::new(),
            }],
        );
        model.title = Some(chart_title.to_string());
        model.display.show_title = true;
        model.display.x_axis.show_title = false;
        model.display.y_axis.show_title = false;
        render_typed_xychart(&model, &AsciiRenderOptions::ascii())
            .expect("owned title fixture should render")
    };

    let structural_lf = render("A\nB");
    let authored_escape = render(r"A\nB");

    assert_ne!(
        structural_lf, authored_escape,
        "a structural line break must not collapse with authored escape text"
    );
    assert!(
        structural_lf.contains(r#"titleDisplay: chart(bytes=3)="A\nB""#),
        "structural LF must retain its source byte length:\n{structural_lf}"
    );
    assert!(
        authored_escape.contains(r#"titleDisplay: chart(bytes=4)="A\\nB""#),
        "literal escape text must remain distinguishable from LF:\n{authored_escape}"
    );
}

#[test]
fn xychart_single_series_title_remains_visible() {
    let render = |title: Option<&str>| {
        let model = typed_xychart_model(
            "vertical",
            XyChartAxisRenderModel::Band {
                title: String::new(),
                categories: vec!["A".to_string()],
            },
            0.0,
            10.0,
            vec![XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Line,
                title: title.map(str::to_string),
                values: vec![5.0],
                data: vec![("A".to_string(), Some(5.0))],
                point_labels: Vec::new(),
            }],
        );
        render_typed_xychart(&model, &AsciiRenderOptions::ascii())
            .expect("single-series XYChart should render")
    };

    let titled = render(Some("Revenue"));
    let untitled = render(None);

    assert!(
        titled.contains("Revenue"),
        "missing series title:\n{titled}"
    );
    assert_ne!(
        titled, untitled,
        "an authored series title must not collapse into an untitled series"
    );
}

#[test]
fn xychart_rejects_unknown_direct_model_orientation() {
    let model = typed_xychart_model(
        "diagonal",
        XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A".to_string()],
        },
        0.0,
        10.0,
        vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Line,
            title: None,
            values: vec![5.0],
            data: vec![("A".to_string(), Some(5.0))],
            point_labels: Vec::new(),
        }],
    );

    let error = render_typed_xychart(&model, &AsciiRenderOptions::ascii())
        .expect_err("unknown direct-model orientations must not become vertical charts");
    assert!(matches!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "xychart",
            feature: "chart orientation"
        }
    ));
}

#[test]
fn xychart_rejects_direct_model_band_y_axis() {
    let mut model = typed_xychart_model(
        "vertical",
        XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A".to_string()],
        },
        0.0,
        10.0,
        vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Line,
            title: None,
            values: vec![5.0],
            data: vec![("A".to_string(), Some(5.0))],
            point_labels: Vec::new(),
        }],
    );
    model.y_axis = XyChartAxisRenderModel::Band {
        title: "invalid".to_string(),
        categories: vec!["low".to_string(), "high".to_string()],
    };

    let error = render_typed_xychart(&model, &AsciiRenderOptions::ascii())
        .expect_err("Mermaid-invalid Band y-axes must not be silently linearized");
    assert!(matches!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "xychart",
            feature: "band y-axis"
        }
    ));
}

#[test]
fn xychart_accessibility_metadata_is_intentionally_omitted() {
    let model = typed_xychart_model(
        "vertical",
        XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: vec!["A".to_string()],
        },
        0.0,
        10.0,
        vec![XyChartPlotRenderModel {
            plot_type: XyChartPlotType::Line,
            title: None,
            values: vec![5.0],
            data: vec![("A".to_string(), Some(5.0))],
            point_labels: Vec::new(),
        }],
    );
    let mut with_accessibility_metadata = model.clone();
    with_accessibility_metadata.acc_title = Some("Screen reader title".to_string());
    with_accessibility_metadata.acc_descr = Some("Browser-only chart description".to_string());

    let baseline = render_typed_xychart(&model, &AsciiRenderOptions::ascii())
        .expect("baseline XYChart should render");
    let with_accessibility_metadata =
        render_typed_xychart(&with_accessibility_metadata, &AsciiRenderOptions::ascii())
            .expect("accessibility metadata should not block terminal rendering");

    assert_eq!(
        baseline, with_accessibility_metadata,
        "browser accessibility metadata is an intentional terminal omission"
    );
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
        "title(bytes=6)=\"Sparse\" samples=[{index=0 x(bytes=1)=\"0\" value=1",
        "title(bytes=5)=\"Dense\" samples=[{index=0 x(bytes=1)=\"0\" value=2",
        "index=1 x(bytes=1)=\"3\" value=4",
        "index=2 x(bytes=1)=\"7\" value=6",
        "index=3 x(bytes=2)=\"10\" value=8",
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
        .with_xychart_category_band_width(3);
    let resources = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxGridCells, 41)
        .expect("valid grid limit");

    let error = render_typed_xychart_with_resources(&model, &options, resources)
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
    let options = AsciiRenderOptions::ascii().with_xychart_horizontal_plot_width(10);
    let resources = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxGridCells, 39)
        .expect("valid grid limit");

    let error = render_typed_xychart_with_resources(&model, &options, resources)
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
    let top_endpoint = lines
        .iter()
        .position(|line| line.trim_start().starts_with("0 +"));
    let baseline_endpoint = lines
        .iter()
        .position(|line| line.trim_start().starts_with("10 +"));

    assert!(
        matches!((top_endpoint, baseline_endpoint), (Some(top), Some(bottom)) if top < bottom),
        "the authored `0` endpoint should remain at the top of a reversed axis:\n{rendered}"
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
        rendered.contains("x(bytes=1)=\"A\" value=2 pointLabel(bytes=4)=\"peak\" clipped=true"),
        "clipped value and anchored point label should remain exact:\n{rendered}"
    );
    assert!(
        rendered.contains("orphanPointLabels=[bytes=8=\"detached\"]"),
        "orphan point labels must not disappear silently:\n{rendered}"
    );
}

#[test]
fn xychart_point_labels_preserve_authored_whitespace_and_presence() {
    let render = |point_labels: Vec<&str>| {
        let model = typed_xychart_model(
            "vertical",
            XyChartAxisRenderModel::Band {
                title: String::new(),
                categories: vec!["A".to_string()],
            },
            0.0,
            10.0,
            vec![XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Line,
                title: None,
                values: vec![5.0],
                data: vec![("A".to_string(), Some(5.0))],
                point_labels: point_labels.into_iter().map(str::to_string).collect(),
            }],
        );

        render_typed_xychart(&model, &AsciiRenderOptions::ascii())
            .expect("authored point-label whitespace should render")
    };

    let leading = render(vec![" peak"]);
    let trailing = render(vec!["peak "]);
    let whitespace = render(vec![" "]);
    let absent = render(Vec::new());
    let orphan_whitespace = render(vec!["peak", " "]);

    assert!(
        leading.contains(r#"pointLabel(bytes=5)=" peak""#),
        "{leading}"
    );
    assert!(
        trailing.contains(r#"pointLabel(bytes=5)="peak ""#),
        "{trailing}"
    );
    assert!(
        whitespace.contains(r#"pointLabel(bytes=1)=" ""#),
        "{whitespace}"
    );
    assert!(
        orphan_whitespace.contains(r#"orphanPointLabels=[bytes=1=" "]"#),
        "{orphan_whitespace}"
    );
    assert_ne!(leading, trailing);
    assert_ne!(whitespace, absent);
}

#[test]
fn xychart_nonempty_domain_discloses_empty_series_title_and_type() {
    let render = |plot_type, title: &str| {
        let model = typed_xychart_model(
            "vertical",
            XyChartAxisRenderModel::Band {
                title: String::new(),
                categories: vec!["A".to_string()],
            },
            0.0,
            10.0,
            vec![XyChartPlotRenderModel {
                plot_type,
                title: Some(title.to_string()),
                values: Vec::new(),
                data: Vec::new(),
                point_labels: Vec::new(),
            }],
        );

        render_typed_xychart(&model, &AsciiRenderOptions::ascii())
            .expect("empty series metadata should render")
    };

    let line = render(XyChartPlotType::Line, "Forecast");
    let bar = render(XyChartPlotType::Bar, "Forecast");
    let actual = render(XyChartPlotType::Line, "Actual");

    assert!(
        line.contains(
            r#"values: * series=0 type=line title(bytes=8)="Forecast" samples=[] orphanPointLabels=[]"#
        ),
        "{line}"
    );
    assert!(
        bar.contains(
            r#"values: # series=0 type=bar title(bytes=8)="Forecast" samples=[] orphanPointLabels=[]"#
        ),
        "{bar}"
    );
    assert_ne!(line, bar);
    assert_ne!(line, actual);
}

#[test]
fn xychart_overwide_band_category_discloses_the_complete_axis_domain() {
    let render = |unused_category: &str| {
        let model = typed_xychart_model(
            "vertical",
            XyChartAxisRenderModel::Band {
                title: String::new(),
                categories: vec!["A".to_string(), unused_category.to_string()],
            },
            0.0,
            10.0,
            vec![
                XyChartPlotRenderModel {
                    plot_type: XyChartPlotType::Line,
                    title: None,
                    values: vec![5.0],
                    data: vec![("A".to_string(), Some(5.0))],
                    point_labels: Vec::new(),
                },
                XyChartPlotRenderModel {
                    plot_type: XyChartPlotType::Line,
                    title: None,
                    values: Vec::new(),
                    data: Vec::new(),
                    point_labels: Vec::new(),
                },
            ],
        );
        let options = AsciiRenderOptions::ascii().with_xychart_category_band_width(3);

        render_typed_xychart(&model, &options)
            .expect("overwide Band categories should render through exact disclosure")
    };

    let alphabetic = render("abcdef");
    let alternate = render("abcxyz");

    assert!(
        alphabetic.contains(r#"xDomain: band categories=[bytes=1="A", bytes=6="abcdef"]"#),
        "{alphabetic}"
    );
    assert!(
        alternate.contains(r#"xDomain: band categories=[bytes=1="A", bytes=6="abcxyz"]"#),
        "{alternate}"
    );
    assert_ne!(alphabetic, alternate);
}

#[test]
fn xychart_centered_band_projection_discloses_colliding_categories() {
    for (orientation, second) in [("vertical", " A "), ("horizontal", "  A")] {
        let mut model = typed_xychart_model(
            orientation,
            XyChartAxisRenderModel::Band {
                title: String::new(),
                categories: vec!["A".to_string(), second.to_string()],
            },
            0.0,
            10.0,
            vec![XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Bar,
                title: None,
                values: vec![5.0, 8.0],
                data: vec![
                    ("A".to_string(), Some(5.0)),
                    (second.to_string(), Some(8.0)),
                ],
                point_labels: Vec::new(),
            }],
        );
        model.display.x_axis.show_label = true;
        let options = AsciiRenderOptions::ascii().with_xychart_category_band_width(3);

        let rendered = render_typed_xychart(&model, &options)
            .expect("projected Band labels should render through exact disclosure");
        let expected = format!(
            r#"xDomain: band categories=[bytes=1="A", bytes={}="{second}"]"#,
            second.len()
        );

        assert!(
            rendered.contains(&expected),
            "{orientation} categories whose final projection loses identity must remain explicit:\n{rendered}"
        );
        assert!(
            !rendered.lines().any(|line| line.starts_with("values: ")),
            "domain-only disclosure should not add a redundant values row:\n{rendered}"
        );
    }
}

#[test]
fn xychart_trailing_band_projection_discloses_trimmed_category() {
    for orientation in ["vertical", "horizontal"] {
        let mut model = typed_xychart_model(
            orientation,
            XyChartAxisRenderModel::Band {
                title: String::new(),
                categories: vec!["A  ".to_string()],
            },
            0.0,
            10.0,
            vec![XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Bar,
                title: None,
                values: vec![5.0],
                data: vec![("A  ".to_string(), Some(5.0))],
                point_labels: Vec::new(),
            }],
        );
        model.display.x_axis.show_label = true;

        let rendered = render_typed_xychart(&model, &AsciiRenderOptions::ascii())
            .expect("trailing Band category should render through framed disclosure");
        assert!(
            rendered.contains(r#"xDomain: band categories=[bytes=3="A  "]"#),
            "a category erased by the final row trim must retain its source identity:\n{rendered}"
        );
    }
}

#[test]
fn xychart_band_domain_discloses_terminal_normalization_identity() {
    let render = |category: &str| {
        let model = typed_xychart_model(
            "horizontal",
            XyChartAxisRenderModel::Band {
                title: String::new(),
                categories: vec![category.to_string()],
            },
            0.0,
            10.0,
            vec![XyChartPlotRenderModel {
                plot_type: XyChartPlotType::Bar,
                title: None,
                values: vec![5.0],
                data: vec![(category.to_string(), Some(5.0))],
                point_labels: Vec::new(),
            }],
        );

        render_typed_xychart(&model, &AsciiRenderOptions::ascii())
            .expect("Band category normalization should remain injective")
    };

    let control = render("\u{1b}");
    let authored_escape = render(r"\u{1B}");

    assert!(
        control.contains(r#"xDomain: band categories=[bytes=1="\u{1B}"]"#),
        "the normalized control must retain its authored byte identity:\n{control}"
    );
    assert!(
        !authored_escape.contains("xDomain: band categories="),
        "unchanged printable text should not force redundant domain disclosure:\n{authored_escape}"
    );
    assert_ne!(control, authored_escape);
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
        rendered.contains("index=0 x(bytes=1)=\"B\" value=0")
            && rendered.contains("index=1 x(bytes=1)=\"A\" value=1")
            && rendered.contains("index=2 x(bytes=1)=\"A\" value=2"),
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
            rendered.contains("x(bytes=1)=\"B\" value=none"),
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
        rendered.contains("index=0 x(bytes=1)=\"0\" value=5")
            && rendered.contains("index=1 x(bytes=3)=\"0.1\" value=5"),
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
        rendered.contains("index=0 x(bytes=1)=\"0\" value=5")
            && rendered.contains("index=1 x(bytes=2)=\"20\" value=8"),
        "overlapping bar identities must remain exact:\n{rendered}"
    );
}

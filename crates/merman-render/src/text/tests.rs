use super::*;

#[test]
fn html_br_trims_trailing_space_before_break_for_flowchart_labels() {
    let plain =
        crate::flowchart::flowchart_label_plain_text_for_layout("Hexagon <br> end", "text", true);
    assert_eq!(plain, "Hexagon\nend");

    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let m = measurer.measure_wrapped(&plain, &style, Some(200.0), WrapMode::HtmlLike);
    assert_eq!(m.width, 60.984375);
    assert_eq!(m.height, 48.0);
}

#[test]
fn markdown_strong_width_matches_flowchart_table() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let regular_html = measurer.measure_wrapped("Two", &style, Some(200.0), WrapMode::HtmlLike);
    assert_eq!(regular_html.width, 27.578125);

    let strong_html = measure_markdown_with_flowchart_bold_deltas(
        &measurer,
        "**Two**",
        &style,
        Some(200.0),
        WrapMode::HtmlLike,
    );
    assert_eq!(strong_html.width, 30.109375);

    let regular_svg = measurer.measure_wrapped("Two", &style, Some(200.0), WrapMode::SvgLike);
    assert_eq!(regular_svg.width, 28.984375);

    let strong_svg = measure_markdown_with_flowchart_bold_deltas(
        &measurer,
        "**Two**",
        &style,
        Some(200.0),
        WrapMode::SvgLike,
    );
    // The whole point of `measure_markdown_with_flowchart_bold_deltas` is that `**...**` uses
    // the flowchart bold delta table consistently across wrap modes.
    assert_eq!(
        strong_svg.width - regular_svg.width,
        strong_html.width - regular_html.width
    );
    assert_eq!(strong_svg.width, 31.515625);
}

#[test]
fn flowchart_html_unwrapped_width_matches_upstream_at_30px() {
    // Mermaid upstream fixture:
    // fixtures/upstream-svgs/flowchart/upstream_flowchart_v2_bigger_font_from_classes_spec.svg
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 30.0,
        font_weight: None,
    };

    let m = measurer.measure_wrapped("I am a circle", &style, None, WrapMode::HtmlLike);
    assert_eq!(m.width, 167.03125);
    assert_eq!(m.height, 45.0);
    assert_eq!(m.line_count, 1);
}

#[test]
fn flowchart_html_fontawesome_icon_width_matches_upstream() {
    // Mermaid upstream fixture:
    // fixtures/upstream-svgs/flowchart/upstream_flowchart_v2_icons_in_edge_labels_spec.svg
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let html = "<p><i class=\"fa fa-car\"></i> Car</p>";
    let m = measure_html_with_flowchart_bold_deltas(
        &measurer,
        html,
        &style,
        Some(200.0),
        WrapMode::HtmlLike,
    );
    assert_eq!(m.width, 45.015625);
    assert_eq!(m.height, 24.0);
    assert_eq!(m.line_count, 1);
}

#[test]
fn flowchart_label_metrics_for_layout_fontawesome_matches_upstream() {
    // Mermaid upstream fixture:
    // fixtures/upstream-svgs/flowchart/upstream_flowchart_v2_icons_in_edge_labels_spec.svg
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let m = crate::flowchart::flowchart_label_metrics_for_layout(
        &measurer,
        "fa:fa-car Car",
        "text",
        &style,
        Some(200.0),
        WrapMode::HtmlLike,
    );
    assert_eq!(m.width, 45.015625);
    assert_eq!(m.height, 24.0);
    assert_eq!(m.line_count, 1);
}

#[test]
fn sequence_svg_overrides_keep_literal_br_with_backslash_t_single_line() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif;".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    // Mermaid `lineBreakRegex` should not treat this as a `<br>` break because `\\t` is a
    // literal backslash + `t`, not whitespace.
    let text = "multiline<br \\t/>text";
    let m = measurer.measure_wrapped(text, &style, None, WrapMode::SvgLikeSingleRun);
    assert_eq!(m.line_count, 1);
    assert_eq!(m.width, 132.0);
}

#[test]
fn wrap_label_like_mermaid_does_not_split_escaped_br() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif;".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let lines =
        wrap_label_like_mermaid_lines("multiline<br>using #lt;br#gt;", &measurer, &style, 10_000.0);
    assert_eq!(
        lines,
        vec!["multiline".to_string(), "using #lt;br#gt;".to_string()],
        "wrapLabel should short-circuit when explicit `<br>` breaks are present, and must not treat escaped `#lt;br#gt;` as a break"
    );
}

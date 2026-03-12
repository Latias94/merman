use super::*;
use crate::flowchart::flowchart_label_metrics_for_layout;
use merman_core::{Engine, ParseOptions};
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

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
    // Mermaid's SVG cluster-title probe for `` `**Two**` `` lands on the same total width as the
    // HTML-label measurement, even though the regular SVG token baseline is wider.
    assert_eq!(strong_svg.width, strong_html.width);
    assert_eq!(strong_svg.width - regular_svg.width, 1.125);
}

#[test]
fn generated_flowchart_markdown_override_paths_cover_repeat_offenders() {
    assert_eq!(
        crate::generated::flowchart_text_overrides_11_12_2::
            lookup_flowchart_markdown_bold_word_delta_em(WrapMode::SvgLike, "Two"),
        Some(9.0 / 128.0)
    );
    assert_eq!(
        crate::generated::flowchart_text_overrides_11_12_2::
            lookup_flowchart_markdown_italic_word_delta_em(WrapMode::SvgLike, "Child"),
        Some(172.0 / 2048.0)
    );
    assert_eq!(
        crate::generated::flowchart_text_overrides_11_12_2::
            lookup_flowchart_markdown_italic_word_delta_em(WrapMode::HtmlLike, "Markdown"),
        Some(83.0 / 1024.0)
    );
    assert_eq!(
        crate::generated::flowchart_text_overrides_11_12_2::
            lookup_flowchart_markdown_bold_word_extra_delta_em(WrapMode::SvgLike, "dog"),
        -7.0 / 16384.0
    );
    assert_eq!(
        crate::generated::flowchart_text_overrides_11_12_2::
            lookup_flowchart_markdown_bold_char_extra_delta_em(WrapMode::SvgLike, "a", 'a'),
        1.0 / 1024.0
    );
    assert_eq!(
        crate::generated::flowchart_text_overrides_11_12_2::
            lookup_flowchart_markdown_bold_char_extra_delta_em(WrapMode::HtmlLike, "a", 'a'),
        0.0
    );
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
    let cfg = merman_core::MermaidConfig::default();

    let m = crate::flowchart::flowchart_label_metrics_for_layout(
        &measurer,
        "fa:fa-car Car",
        "text",
        &style,
        Some(200.0),
        WrapMode::HtmlLike,
        &cfg,
        None,
    );
    assert_eq!(m.width, 45.015625);
    assert_eq!(m.height, 24.0);
    assert_eq!(m.line_count, 1);
}

#[test]
fn courier_html_flowchart_label_width_matches_upstream() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("courier".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let node = measurer.measure_wrapped("Christmas", &style, Some(200.0), WrapMode::HtmlLike);
    assert_eq!(node.width, 86.421875);
    assert_eq!(node.height, 24.0);

    let edge = measurer.measure_wrapped("Get money", &style, Some(200.0), WrapMode::HtmlLike);
    assert_eq!(edge.width, 86.421875);
    assert_eq!(edge.height, 24.0);
}

#[test]
fn default_font_flowchart_html_width_overrides_match_upstream() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let edge_a = measurer.measure_wrapped("A to B", &style, Some(200.0), WrapMode::HtmlLike);
    assert_eq!(edge_a.width, 42.1875);
    assert_eq!(edge_a.height, 24.0);

    let edge_b = measurer.measure_wrapped("B to C", &style, Some(200.0), WrapMode::HtmlLike);
    assert_eq!(edge_b.width, 43.203125);
    assert_eq!(edge_b.height, 24.0);

    let node = measurer.measure_wrapped("A: (Edge Text)", &style, Some(200.0), WrapMode::HtmlLike);
    assert_eq!(node.width, 101.046875);
    assert_eq!(node.height, 24.0);

    let cluster = measurer.measure_wrapped("Inner B", &style, Some(200.0), WrapMode::HtmlLike);
    assert_eq!(cluster.width, 50.765625);
    assert_eq!(cluster.height, 24.0);

    let edge = measurer.measure_wrapped(
        "very long edge label",
        &style,
        Some(200.0),
        WrapMode::HtmlLike,
    );
    assert_eq!(edge.width, 145.09375);
    assert_eq!(edge.height, 24.0);

    let post = measurer.measure_wrapped("post", &style, Some(200.0), WrapMode::HtmlLike);
    assert_eq!(post.width, 30.328125);
    assert_eq!(post.height, 24.0);

    let dense_cluster =
        measurer.measure_wrapped("Dense Cluster", &style, Some(200.0), WrapMode::HtmlLike);
    assert_eq!(dense_cluster.width, 98.109375);
    assert_eq!(dense_cluster.height, 24.0);

    let outside2 = measurer.measure_wrapped("outside2", &style, Some(200.0), WrapMode::HtmlLike);
    assert_eq!(outside2.width, 60.75);
    assert_eq!(outside2.height, 24.0);

    for level in ["Level 1", "Level 2", "Level 3", "Level 4"] {
        let metrics = measurer.measure_wrapped(level, &style, Some(200.0), WrapMode::HtmlLike);
        assert_eq!(metrics.width, 51.328125, "{level}");
        assert_eq!(metrics.height, 24.0, "{level}");
    }

    let subgraph_title =
        measurer.measure_wrapped("Subgraph Title", &style, Some(200.0), WrapMode::HtmlLike);
    assert_eq!(subgraph_title.width, 103.171875);
    assert_eq!(subgraph_title.height, 24.0);

    let edge_label =
        measurer.measure_wrapped("Edge Label", &style, Some(200.0), WrapMode::HtmlLike);
    assert_eq!(edge_label.width, 77.9375);
    assert_eq!(edge_label.height, 24.0);

    let node_label_b =
        measurer.measure_wrapped("Node Label B", &style, Some(200.0), WrapMode::HtmlLike);
    assert_eq!(node_label_b.width, 94.0);
    assert_eq!(node_label_b.height, 24.0);

    let custom = measurer.measure_wrapped("custom", &style, Some(200.0), WrapMode::HtmlLike);
    assert_eq!(custom.width, 51.359375);
    assert_eq!(custom.height, 24.0);
}

#[test]
fn flowchart_html_wrapped_measurement_does_not_leak_other_diagram_overrides() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let wrapped = measurer.measure_wrapped("plain", &style, Some(200.0), WrapMode::HtmlLike);
    let unwrapped = measurer.measure_wrapped("plain", &style, None, WrapMode::HtmlLike);
    assert_eq!(wrapped.width, 35.34375);
    assert_eq!(wrapped.height, 24.0);
    assert_eq!(unwrapped.width, 144.359375);
}

#[test]
fn flowchart_svg_cluster_title_precise_width_matches_upstream_wrapped_text() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let width = measure_flowchart_svg_like_precise_width_px(
        &measurer,
        "A very long cluster title with punctuation: (a/b/c)",
        &style,
        Some(200.0),
    );
    assert_eq!(width, 186.90625);
}

#[test]
fn flowchart_svg_cluster_title_precise_width_matches_upstream_single_line_text() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let width = measure_flowchart_svg_like_precise_width_px(
        &measurer,
        "Subgraph Title",
        &style,
        Some(200.0),
    );
    assert_eq!(width, 103.1875);
}

#[test]
fn flowchart_svg_cluster_title_precise_width_matches_upstream_one() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let width = measure_flowchart_svg_like_precise_width_px(&measurer, "One", &style, Some(200.0));
    assert_eq!(width, 28.25);
}

#[test]
fn flowchart_svg_edge_label_width_matches_upstream_single_line_text() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let metrics = measurer.measure_wrapped("Edge Label", &style, Some(200.0), WrapMode::SvgLike);
    assert_eq!(metrics.width, 77.9375);
    assert_eq!(metrics.height, 19.0);
}

#[test]
fn flowchart_svg_node_label_width_overrides_match_repeat_offenders() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let node_label = measurer.measure_wrapped("Node Label", &style, Some(200.0), WrapMode::SvgLike);
    assert_eq!(node_label.width, 80.125);

    let node_label_b =
        measurer.measure_wrapped("Node Label B", &style, Some(200.0), WrapMode::SvgLike);
    assert_eq!(node_label_b.width, 94.0);

    let cfg = merman_core::MermaidConfig::default();
    let b = flowchart_label_metrics_for_layout(
        &measurer,
        "b",
        "text",
        &style,
        Some(200.0),
        WrapMode::SvgLike,
        &cfg,
        None,
    );
    assert_eq!(b.width, 8.921875);
}

#[test]
fn courier_svg_edge_label_width_matches_upstream() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("courier".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let metrics = measurer.measure_wrapped("Get money", &style, Some(200.0), WrapMode::SvgLike);
    assert_eq!(metrics.width, 86.421875);
    assert_eq!(metrics.height, 18.0);
    assert_eq!(metrics.line_count, 1);
}

#[test]
fn flowchart_svg_edge_label_background_y_matches_upstream_fonts() {
    let trebuchet = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };
    let courier = TextStyle {
        font_family: Some("courier".to_string()),
        font_size: 16.0,
        font_weight: None,
    };
    let courier_stack = TextStyle {
        font_family: Some("\"Courier New\", courier, monospace;".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    assert_eq!(flowchart_svg_edge_label_background_y_px(&trebuchet), -1.0);
    assert_eq!(flowchart_svg_edge_label_background_y_px(&courier), 0.0);
    assert_eq!(
        flowchart_svg_edge_label_background_y_px(&courier_stack),
        0.0
    );
}

#[test]
fn svg_title_bbox_vertical_extents_use_courier_profile_for_courier_stacks() {
    let trebuchet = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 18.0,
        font_weight: None,
    };
    let courier = TextStyle {
        font_family: Some("courier".to_string()),
        font_size: 18.0,
        font_weight: None,
    };
    let courier_stack = TextStyle {
        font_family: Some("\"Courier New\", courier, monospace;".to_string()),
        font_size: 18.0,
        font_weight: None,
    };

    assert_eq!(
        svg_title_bbox_vertical_extents_px(&courier_stack),
        svg_title_bbox_vertical_extents_px(&courier)
    );
    assert_ne!(
        svg_title_bbox_vertical_extents_px(&courier_stack),
        svg_title_bbox_vertical_extents_px(&trebuchet)
    );
}

#[test]
fn generated_timeline_svg_override_paths_cover_long_word_bbox() {
    assert_eq!(
        crate::generated::timeline_text_overrides_11_12_2::
            lookup_timeline_svg_bbox_x_with_ascii_overhang_px(
                "\"trebuchet ms\", verdana, arial, sans-serif",
                16.0,
                "SupercalifragilisticexpialidociousSupercalifragilisticexpialidocious",
            ),
        Some((235.3203125, 235.3203125))
    );
    assert_eq!(
        crate::generated::timeline_text_overrides_11_12_2::
            lookup_timeline_svg_bbox_x_with_ascii_overhang_px(
                "",
                16.0,
                "SupercalifragilisticexpialidociousSupercalifragilisticexpialidocious",
            ),
        Some((235.3203125, 235.3203125))
    );
    assert_eq!(
        crate::generated::timeline_text_overrides_11_12_2::
            lookup_timeline_svg_bbox_x_with_ascii_overhang_px("courier", 16.0, "Line 2"),
        None
    );
}

#[test]
fn timeline_long_word_wrap_keeps_upstream_activity_line_extent() {
    let path = workspace_root()
        .join("fixtures")
        .join("timeline")
        .join("upstream_long_word_wrap.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let engine = Engine::new();
    let parsed = futures::executor::block_on(engine.parse_diagram(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");
    let out = crate::layout_parsed(&parsed, &crate::LayoutOptions::default()).expect("layout ok");
    let crate::model::LayoutDiagram::TimelineDiagram(layout) = out.layout else {
        panic!("expected TimelineDiagram layout");
    };

    let actual = layout.activity_line.x2;
    assert!(
        (actual - 920.640625).abs() < 0.0001,
        "expected long-word timeline activity line extent to stay aligned with upstream, got {actual}"
    );
}

#[test]
fn default_font_extra_html_override_table_keeps_special_characters_stable() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let metrics = measurer.measure_wrapped("special characters", &style, None, WrapMode::HtmlLike);
    assert_eq!(metrics.width, 129.9375);
    assert_eq!(metrics.height, 24.0);
}

#[test]
fn generated_flowchart_html_override_paths_cover_promoted_leftovers() {
    assert_eq!(
        crate::generated::flowchart_text_overrides_11_12_2::lookup_flowchart_html_width_px(
            FLOWCHART_DEFAULT_FONT_KEY,
            16.0,
            "special characters",
        ),
        Some(129.9375)
    );
    assert_eq!(
        crate::generated::flowchart_text_overrides_11_12_2::lookup_flowchart_html_width_px(
            "courier",
            16.0,
            "special characters",
        ),
        None
    );
    assert_eq!(
        crate::generated::flowchart_text_overrides_11_12_2::lookup_flowchart_html_width_px(
            FLOWCHART_DEFAULT_FONT_KEY,
            16.0,
            "Block 1",
        ),
        None
    );
    assert_eq!(
        crate::generated::flowchart_text_overrides_11_12_2::lookup_flowchart_html_width_px(
            FLOWCHART_DEFAULT_FONT_KEY,
            16.0,
            "Line 2",
        ),
        Some(43.34375)
    );
}

#[test]
fn generated_html_override_paths_cover_pruned_block_and_flowchart_literals() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let block = measurer.measure_wrapped("Block 1", &style, None, WrapMode::HtmlLike);
    assert_eq!(block.width, 51.5625);

    let flowchart = measurer.measure_wrapped("Circle shape", &style, None, WrapMode::HtmlLike);
    assert_eq!(flowchart.width, 87.8125);
}

#[test]
fn generated_flowchart_svg_override_paths_cover_pruned_literals() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    assert_eq!(
        crate::generated::flowchart_text_overrides_11_12_2::lookup_flowchart_svg_bbox_x_px(
            FLOWCHART_DEFAULT_FONT_KEY,
            16.0,
            "End",
        ),
        Some((13.1171875, 13.1171875))
    );
    let end = measurer.measure_wrapped("End", &style, Some(200.0), WrapMode::SvgLike);
    assert_eq!(end.width, 26.234375);

    let edge_label = measurer.measure_wrapped("edge label", &style, Some(200.0), WrapMode::SvgLike);
    assert_eq!(edge_label.width, 74.71875);
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

#[test]
fn markdown_underscore_delimiters_match_mermaid() {
    use MermaidMarkdownWordType::*;

    assert_eq!(
        mermaid_markdown_to_lines("`a__b`", true),
        vec![vec![("a__b".to_string(), Normal)]]
    );
    assert_eq!(
        mermaid_markdown_to_lines("`_a_b_`", true),
        vec![vec![("a_b".to_string(), Em)]]
    );
    assert_eq!(
        mermaid_markdown_to_lines("`_a__b_`", true),
        vec![vec![("a__b".to_string(), Em)]]
    );
    assert_eq!(
        mermaid_markdown_to_lines("`__a__`", true),
        vec![vec![("a".to_string(), Strong)]]
    );
}

#[test]
fn markdown_inline_code_suppresses_emphasis_delimiters() {
    use MermaidMarkdownWordType::*;

    // Mermaid CLI baselines (class diagram HTML labels) preserve backticks and do not interpret
    // `**...**` inside them as strong/emphasis.
    assert_eq!(
        mermaid_markdown_to_lines("inline: `**not bold**`", true),
        vec![vec![
            ("inline:".to_string(), Normal),
            ("`**not".to_string(), Normal),
            ("bold**`".to_string(), Normal),
        ]]
    );
}

#[test]
fn flowchart_label_metrics_for_layout_measures_markdown_inline_html_like_mermaid() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };
    let cfg = merman_core::MermaidConfig::default();
    let markdown = "This is **bold** </br>and <strong>strong</strong>";
    assert!(mermaid_markdown_contains_html_tags(markdown));

    let html = mermaid_markdown_to_html_label_fragment(markdown, true);
    let html_metrics = measure_html_with_flowchart_bold_deltas(
        &measurer,
        &html,
        &style,
        Some(200.0),
        WrapMode::HtmlLike,
    );
    assert_eq!(html_metrics.width, 82.125);
    assert_eq!(html_metrics.height, 48.0);
    assert_eq!(html_metrics.line_count, 2);

    let metrics = crate::flowchart::flowchart_label_metrics_for_layout(
        &measurer,
        markdown,
        "markdown",
        &style,
        Some(200.0),
        WrapMode::HtmlLike,
        &cfg,
        None,
    );
    assert_eq!(metrics.width, 82.125);
    assert_eq!(metrics.height, 48.0);
    assert_eq!(metrics.line_count, 2);
}

#[test]
fn markdown_svg_wrapping_keeps_raw_html_tags_literal_but_wraps_like_mermaid() {
    use MermaidMarkdownWordType::*;

    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let lines = mermaid_markdown_to_wrapped_word_lines(
        &measurer,
        "This is **bold** </br>and <strong>strong</strong>",
        &style,
        Some(200.0),
        WrapMode::SvgLike,
    );
    assert_eq!(
        lines,
        vec![
            vec![
                ("This".to_string(), Normal),
                ("is".to_string(), Normal),
                ("bold".to_string(), Strong),
            ],
            vec![
                ("and".to_string(), Normal),
                ("<strong>".to_string(), Normal),
                ("strong".to_string(), Normal),
            ],
            vec![("</strong>".to_string(), Normal)],
        ]
    );
}

#[test]
fn markdown_html_label_fragment_collapses_mixed_list_blocks_like_browser_dom() {
    let input = "Hello\n  - l1\n  - l2";
    assert!(mermaid_markdown_contains_raw_blocks(input));
    assert_eq!(
        mermaid_markdown_to_html_label_fragment(input, true),
        "<p>Hello</p>- l1 - l2"
    );
}

#[test]
fn markdown_xhtml_label_fragment_preserves_inline_br_listish_continuations() {
    let input = "Hello<br/>- l1<br/>- l2";
    assert_eq!(
        mermaid_markdown_to_xhtml_label_fragment(input, true),
        "<p>Hello<br/>- l1<br/>- l2</p>"
    );
}

#[test]
fn markdown_xhtml_label_fragment_normalizes_raw_br_variants() {
    let input = "Hello<br>world";
    assert_eq!(
        mermaid_markdown_to_xhtml_label_fragment(input, true),
        "<p>Hello<br/>world</p>"
    );
}

#[test]
fn markdown_html_label_fragment_preserves_inline_code_literals() {
    let input = "inline: `**not bold**`";
    assert_eq!(
        mermaid_markdown_to_html_label_fragment(input, true),
        "<p>inline: `**not bold**`</p>"
    );
}

#[test]
fn markdown_xhtml_label_fragment_preserves_inline_code_literals() {
    let input = "inline: `**not bold**`";
    assert_eq!(
        mermaid_markdown_to_xhtml_label_fragment(input, true),
        "<p>inline: `**not bold**`</p>"
    );
}

#[test]
fn markdown_html_label_fragment_reinterprets_partial_star_strong_like_mermaid() {
    let input = "+inline: **bold*";
    assert_eq!(
        mermaid_markdown_to_html_label_fragment(input, true),
        "<p>+inline: *<em>bold</em></p>"
    );
}

#[test]
fn markdown_xhtml_label_fragment_reinterprets_partial_star_strong_like_mermaid() {
    let input = "+inline: **bold*";
    assert_eq!(
        mermaid_markdown_to_xhtml_label_fragment(input, true),
        "<p>+inline: *<em>bold</em></p>"
    );
}

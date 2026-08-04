use super::*;
use crate::flowchart::flowchart_label_metrics_for_layout;

fn assert_finite_positive_metrics(metrics: TextMetrics) {
    assert!(
        metrics.width.is_finite() && metrics.width > 0.0,
        "{metrics:?}"
    );
    assert!(
        metrics.height.is_finite() && metrics.height > 0.0,
        "{metrics:?}"
    );
    assert!(metrics.line_count > 0, "{metrics:?}");
}

fn assert_same_metrics(actual: TextMetrics, expected: TextMetrics) {
    assert_eq!(actual.width, expected.width);
    assert_eq!(actual.height, expected.height);
    assert_eq!(actual.line_count, expected.line_count);
}

fn approximate_svg_vertical_profiles(
    bbox_y_em: f64,
    bbox_height_em: f64,
    pair_union_max_delta_px: f64,
) -> [super::font_metrics_data::SvgVerticalProfileSetData;
    super::font_metrics_data::SvgVerticalDomShapeData::COUNT] {
    std::array::from_fn(
        |_| super::font_metrics_data::SvgVerticalProfileSetData::Approximate {
            bbox_y_em,
            bbox_height_em,
            pair_union_max_delta_px,
        },
    )
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
        font_style: None,
    };

    let m = measurer.measure_wrapped(&plain, &style, Some(200.0), WrapMode::HtmlLike);
    let first_line = measurer.measure_wrapped("Hexagon", &style, None, WrapMode::HtmlLike);
    let second_line = measurer.measure_wrapped("end", &style, None, WrapMode::HtmlLike);
    assert_eq!(m.line_count, 2);
    assert_eq!(m.width, first_line.width.max(second_line.width));
    assert!(m.height > first_line.height.max(second_line.height));
}

#[test]
fn flowchart_html_text_extraction_preserves_bare_comparison_symbols() {
    let plain = crate::flowchart::flowchart_label_plain_text_for_layout(
        "标题 Unicode — 測試 &amp; &lt; &gt; and x < y > z",
        "text",
        true,
    );
    assert_eq!(plain, "标题 Unicode — 測試 & < > and x < y > z");
}

#[test]
fn flowchart_html_text_extraction_preserves_nbsp_as_non_breaking_space() {
    let plain =
        crate::flowchart::flowchart_label_plain_text_for_layout("left&nbsp;right", "text", true);
    assert_eq!(plain, "left\u{00A0}right");
}

#[test]
fn flowchart_html_unicode_entities_use_finite_fallback_metrics() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let cfg = merman_core::MermaidConfig::default();

    let metrics = crate::flowchart::flowchart_label_metrics_for_layout(
        crate::flowchart::FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: "标题 Unicode — 測試 & < >",
            label_type: "text",
            style: &style,
            max_width_px: Some(200.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &cfg,
            math_renderer: None,
        },
    );
    assert_finite_positive_metrics(metrics);
    assert_eq!(metrics.line_count, 1);

    let plain_cjk = measurer.measure_wrapped("负责人审批", &style, Some(200.0), WrapMode::HtmlLike);
    let single_cjk = measurer.measure_wrapped("负", &style, Some(200.0), WrapMode::HtmlLike);
    assert_finite_positive_metrics(plain_cjk);
    assert!(plain_cjk.width > single_cjk.width);
}

#[test]
fn flowchart_html_unicode_blocks_produce_finite_metrics() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };

    for text in [
        "emoji: 😀😅👍",
        "rtl: שלום-עולם",
        "中文 / 日本語 / 한글",
        "Path: C:\\Temp\\synthetic\\out.svg (Windows-style)",
    ] {
        assert_finite_positive_metrics(measurer.measure_wrapped(
            text,
            &style,
            Some(200.0),
            WrapMode::HtmlLike,
        ));
    }
}

#[test]
fn typst_relevant_font_intent_keeps_measurement_finite_without_host_font_assets() {
    let payloads = [
        "unknown font family",
        "CJK: 负责人审批",
        "emoji: 😀😅👍",
        "mixed: Source Sans 3 / 測試 / 🚀",
    ];
    let styles = [
        TextStyle {
            font_family: Some("TypstOnlyFont, Arial, sans-serif".to_string()),
            font_size: 13.0,
            font_weight: None,
            font_style: None,
        },
        TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        },
    ];
    let vendored = VendoredFontMetricsTextMeasurer::default();
    let deterministic = DeterministicTextMeasurer::default();

    for style in &styles {
        for payload in payloads {
            for metrics in [
                vendored.measure_wrapped(payload, style, Some(200.0), WrapMode::HtmlLike),
                deterministic.measure_wrapped(payload, style, Some(200.0), WrapMode::HtmlLike),
            ] {
                assert!(
                    metrics.width.is_finite() && metrics.width >= 0.0,
                    "{metrics:?}"
                );
                assert!(
                    metrics.height.is_finite() && metrics.height >= 0.0,
                    "{metrics:?}"
                );
            }
        }
    }
}

#[test]
fn markdown_strong_uses_operation_specific_bold_metrics() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };

    let regular_html = measurer.measure_wrapped("omega", &style, Some(200.0), WrapMode::HtmlLike);
    let strong_html = measure_markdown_with_inline_styles(
        &measurer,
        "**omega**",
        &style,
        Some(200.0),
        WrapMode::HtmlLike,
    );
    let regular_svg = measurer.measure_wrapped("omega", &style, Some(200.0), WrapMode::SvgLike);
    let strong_svg = measure_markdown_with_inline_styles(
        &measurer,
        "**omega**",
        &style,
        Some(200.0),
        WrapMode::SvgLike,
    );

    assert!(strong_html.width > regular_html.width);
    assert!(strong_svg.width > regular_svg.width);
    assert_ne!(strong_svg.height, strong_html.height);
}

#[test]
fn html_inline_styles_delegate_to_the_matching_font_variant() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let regular = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let bold_italic = TextStyle {
        font_weight: Some("700".to_string()),
        font_style: Some("italic".to_string()),
        ..regular.clone()
    };

    let actual = measure_html_with_inline_styles(
        &measurer,
        "<strong><em>Moving</em></strong>",
        &regular,
        None,
        WrapMode::HtmlLike,
    );
    let expected = measurer.measure_wrapped("Moving", &bold_italic, None, WrapMode::HtmlLike);

    assert_same_metrics(actual, expected);

    let bold = TextStyle {
        font_weight: Some("700".to_string()),
        ..regular.clone()
    };
    let italic = TextStyle {
        font_style: Some("italic".to_string()),
        ..regular.clone()
    };
    let mixed = measure_html_with_inline_styles(
        &measurer,
        "plain<strong>Bold</strong><em>Italic</em>",
        &regular,
        None,
        WrapMode::HtmlLike,
    );
    let mixed_expected = measurer
        .measure_wrapped("plain", &regular, None, WrapMode::HtmlLike)
        .width
        + measurer
            .measure_wrapped("Bold", &bold, None, WrapMode::HtmlLike)
            .width
        + measurer
            .measure_wrapped("Italic", &italic, None, WrapMode::HtmlLike)
            .width;
    assert_eq!(mixed.width, mixed_expected);
}

#[test]
fn html_wrapping_uses_browser_line_break_opportunities() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let width = |text: &str| {
        measurer
            .measure_wrapped(text, &style, None, WrapMode::HtmlLike)
            .width
    };

    let hyphen_width = width("alpha-").max(width("beta"));
    let hyphenated =
        measurer.measure_wrapped("alpha-beta", &style, Some(hyphen_width), WrapMode::HtmlLike);
    assert_eq!(hyphenated.line_count, 2, "{hyphenated:?}");

    let cjk_width = width("负责");
    let cjk = measurer.measure_wrapped("负责人审批", &style, Some(cjk_width), WrapMode::HtmlLike);
    assert_eq!(cjk.line_count, 3, "{cjk:?}");
    assert!(cjk.width <= cjk_width, "{cjk:?}");

    let path_width = ["prefix/", "(alpha)/", "suffix"]
        .into_iter()
        .map(width)
        .fold(0.0_f64, f64::max);
    let parenthesized_path = measurer.measure_wrapped(
        "prefix/(alpha)/suffix",
        &style,
        Some(path_width),
        WrapMode::HtmlLike,
    );
    assert_eq!(
        parenthesized_path.line_count, 1,
        "Chromium keeps this parenthesized path at its min-content width: {parenthesized_path:?}"
    );
    assert!(
        parenthesized_path.width > path_width,
        "{parenthesized_path:?}"
    );

    let long_url = "https://example.com/api/v1/some(very-long)/resource-name?query=foo_bar&baz=qux";
    let long_url_width = [
        "https://example.com/api/v1/some(very-long)/resource-name?",
        "query=foo_bar&baz=qux",
    ]
    .into_iter()
    .map(width)
    .fold(0.0_f64, f64::max);
    let wrapped_url =
        measurer.measure_wrapped(long_url, &style, Some(long_url_width), WrapMode::HtmlLike);
    assert_eq!(wrapped_url.line_count, 2, "{wrapped_url:?}");
}

#[test]
fn html_min_content_width_uses_browser_line_break_segments() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let width = |text: &str| {
        measurer
            .measure_wrapped(text, &style, None, WrapMode::HtmlLike)
            .width
    };

    for (text, segments) in [
        ("https://x.test/(alpha)/z", vec!["https://x.test/(alpha)/z"]),
        (
            "https://example.com/api/v1/some(very-long)/resource-name?query=foo_bar&baz=qux",
            vec![
                "https://example.com/api/v1/some(very-",
                "long)/resource-",
                "name?",
                "query=foo_bar&baz=qux",
            ],
        ),
        ("负责人审批", vec!["负", "责", "人", "审", "批"]),
    ] {
        let expected_min_content = segments.into_iter().map(width).fold(0.0_f64, f64::max);
        let actual = measurer.measure_wrapped(text, &style, Some(1.0), WrapMode::HtmlLike);

        assert_eq!(actual.width, expected_min_content, "{text}: {actual:?}");
    }
}

#[test]
fn html_styled_runs_preserve_unicode_line_break_opportunities() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let regular = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let bold = TextStyle {
        font_weight: Some("700".to_string()),
        ..regular.clone()
    };
    let regular_width = measurer
        .measure_wrapped("alpha-beta", &regular, None, WrapMode::HtmlLike)
        .width;
    let bold_width = measurer
        .measure_wrapped("alpha-beta", &bold, None, WrapMode::HtmlLike)
        .width;
    assert!(bold_width > regular_width);
    let wrapping_width = (regular_width + bold_width) / 2.0;

    let actual = measure_html_with_inline_styles(
        &measurer,
        "<strong>alpha-beta</strong>",
        &regular,
        Some(wrapping_width),
        WrapMode::HtmlLike,
    );

    assert_eq!(actual.line_count, 2, "{actual:?}");
    assert_eq!(actual.width, round_to_1_64_px(wrapping_width), "{actual:?}");
}

#[test]
fn html_break_spaces_preserves_trailing_spaces() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };

    let actual =
        measure_html_with_inline_styles(&measurer, "alpha ", &style, None, WrapMode::HtmlLike);
    let expected = measurer.measure_wrapped("alpha ", &style, None, WrapMode::HtmlLike);

    assert_same_metrics(actual, expected);
}

#[test]
fn markdown_inline_styles_delegate_to_operation_specific_font_variants() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let regular = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let bold = TextStyle {
        font_weight: Some("700".to_string()),
        ..regular.clone()
    };
    let italic = TextStyle {
        font_style: Some("italic".to_string()),
        ..regular.clone()
    };

    let italic_actual = measure_markdown_with_inline_styles(
        &measurer,
        "*Moving*",
        &regular,
        None,
        WrapMode::HtmlLike,
    );
    let italic_expected = measurer.measure_wrapped("Moving", &italic, None, WrapMode::HtmlLike);
    assert_same_metrics(italic_actual, italic_expected);

    let bold_actual = measure_markdown_with_inline_styles(
        &measurer,
        "**Two**",
        &regular,
        None,
        WrapMode::SvgLike,
    );
    let bold_expected = measurer.measure_svg_text_computed_length_px("Two", &bold);
    assert_eq!(bold_actual.width, bold_expected);

    let mixed = measure_markdown_with_inline_styles(
        &measurer,
        "plain **Bold** *Italic*",
        &regular,
        None,
        WrapMode::HtmlLike,
    );
    let mixed_expected = measurer
        .measure_wrapped("plain ", &regular, None, WrapMode::HtmlLike)
        .width
        + measurer
            .measure_wrapped("Bold", &bold, None, WrapMode::HtmlLike)
            .width
        + measurer
            .measure_wrapped(" ", &regular, None, WrapMode::HtmlLike)
            .width
        + measurer
            .measure_wrapped("Italic", &italic, None, WrapMode::HtmlLike)
            .width;
    assert_eq!(mixed.width, mixed_expected);
}

#[test]
fn flowchart_html_unwrapped_measurement_scales_with_font_size() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style_15 = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 15.0,
        font_weight: None,
        font_style: None,
    };
    let style_30 = TextStyle {
        font_size: 30.0,
        ..style_15.clone()
    };

    let small =
        measurer.measure_wrapped("synthetic scale probe", &style_15, None, WrapMode::HtmlLike);
    let large =
        measurer.measure_wrapped("synthetic scale probe", &style_30, None, WrapMode::HtmlLike);
    assert_eq!(small.line_count, 1);
    assert_eq!(large.line_count, 1);
    assert!((large.width / small.width - 2.0).abs() < 0.01);
    assert!((large.height / small.height - 2.0).abs() < 0.01);
}

#[test]
fn flowchart_html_fontawesome_icon_width_uses_nominal_boundary() {
    // Model standard FontAwesome icons using Mermaid 11.15's inline FA box width instead of
    // the browser's per-icon glyph advance.
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };

    let html = "<p><i class=\"fa fa-car\"></i> Car</p>";
    let m =
        measure_html_with_inline_styles(&measurer, html, &style, Some(200.0), WrapMode::HtmlLike);
    let plain = measure_html_with_inline_styles(
        &measurer,
        "<p>Car</p>",
        &style,
        Some(200.0),
        WrapMode::HtmlLike,
    );
    assert_finite_positive_metrics(m);
    assert!(m.width > plain.width);
    assert_eq!(m.height, plain.height);
    assert_eq!(m.line_count, 1);
}

#[test]
fn flowchart_html_fontawesome_custom_pack_icon_width_uses_nominal_boundary() {
    // Mermaid 11.15 keeps the inline icon box width even for the documented custom-pack example.
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };

    let html = "<p><i class=\"fab fa-truck-bold\"></i> a custom icon</p>";
    let m =
        measure_html_with_inline_styles(&measurer, html, &style, Some(200.0), WrapMode::HtmlLike);
    let plain = measure_html_with_inline_styles(
        &measurer,
        "<p>a custom icon</p>",
        &style,
        Some(200.0),
        WrapMode::HtmlLike,
    );
    assert_finite_positive_metrics(m);
    assert!(m.width > plain.width);
    assert_eq!(m.height, plain.height);
    assert_eq!(m.line_count, 1);
}

#[test]
fn fontawesome_icon_substitution_matches_mermaid_source_boundaries() {
    assert_eq!(
        replace_fontawesome_icons("This is an icon: fa:fa-user and fab:fa-github"),
        r#"This is an icon: <i class="fa fa-user"></i> and <i class="fab fa-github"></i>"#
    );
    assert_eq!(
        replace_fontawesome_icons("Icons galore: fa:fa-arrow-right, fak:fa-truck, fas:fa-home"),
        r#"Icons galore: <i class="fa fa-arrow-right"></i>, <i class="fak fa-truck"></i>, <i class="fas fa-home"></i>"#
    );
    assert_eq!(
        replace_fontawesome_icons(
            "Here is a long icon: fak:fa-truck-driving-long-winding-road in use"
        ),
        r#"Here is a long icon: <i class="fak fa-truck-driving-long-winding-road"></i> in use"#
    );
    assert_eq!(
        replace_fontawesome_icons("no icons: faa:fa-user fa:fa- fa:fa-éclair"),
        "no icons: faa:fa-user fa:fa- fa:fa-éclair"
    );
    assert_eq!(
        replace_fontawesome_icons("prefix can match inside text: xfa:fa-user!"),
        r#"prefix can match inside text: x<i class="fa fa-user"></i>!"#
    );
}

#[test]
fn flowchart_label_metrics_for_layout_fontawesome_uses_nominal_boundary() {
    // Non-markdown Flowchart icon labels should use the same HTML fragment measurement path as
    // emitted `<foreignObject>` content, with the same Mermaid 11.15 icon width boundary.
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let cfg = merman_core::MermaidConfig::default();

    let actual = crate::flowchart::flowchart_label_metrics_for_layout(
        crate::flowchart::FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: "fa:fa-car Car",
            label_type: "text",
            style: &style,
            max_width_px: Some(200.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &cfg,
            math_renderer: None,
        },
    );
    let html = format!("<p>{}</p>", replace_fontawesome_icons("fa:fa-car Car"));
    let expected =
        measure_html_with_inline_styles(&measurer, &html, &style, Some(200.0), WrapMode::HtmlLike);
    assert_same_metrics(actual, expected);
}

#[test]
fn flowchart_label_metrics_plain_text_uses_dom_text_operation() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let cfg = merman_core::MermaidConfig::default();

    let actual = crate::flowchart::flowchart_label_metrics_for_layout(
        crate::flowchart::FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: "synthetic",
            label_type: "text",
            style: &style,
            max_width_px: Some(200.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &cfg,
            math_renderer: None,
        },
    );
    let expected = measurer.measure_wrapped("synthetic", &style, Some(200.0), WrapMode::HtmlLike);
    assert_same_metrics(actual, expected);
}

#[test]
fn flowchart_label_metrics_for_layout_fontawesome_icon_only_lines_preserve_breaks() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let cfg = merman_core::MermaidConfig::default();

    let twitter = crate::flowchart::flowchart_label_metrics_for_layout(
        crate::flowchart::FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: "fa:fa-twitter<br/>for peace",
            label_type: "text",
            style: &style,
            max_width_px: Some(200.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &cfg,
            math_renderer: None,
        },
    );
    assert_finite_positive_metrics(twitter);
    assert_eq!(twitter.line_count, 2);

    let camera = crate::flowchart::flowchart_label_metrics_for_layout(
        crate::flowchart::FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: "fa:fa-camera-retro<br/>capture<br/>moments",
            label_type: "text",
            style: &style,
            max_width_px: Some(200.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &cfg,
            math_renderer: None,
        },
    );
    assert_finite_positive_metrics(camera);
    assert_eq!(camera.line_count, 3);
    assert!(camera.height > twitter.height);
}

#[test]
fn flowchart_label_metrics_for_layout_fontawesome_wraps_unbreakable_icon_runs() {
    // Mermaid upstream fixture:
    // fixtures/upstream-svgs/flowchart/upstream_cypress_flowchart_handdrawn_spec_fhd7_should_render_a_flowchart_full_of_icons_007.svg
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let cfg = merman_core::MermaidConfig::default();

    let database = crate::flowchart::flowchart_label_metrics_for_layout(
        crate::flowchart::FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: r"fa:fa-database [DBServer\SharedDbInstance]",
            label_type: "text",
            style: &style,
            max_width_px: Some(200.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &cfg,
            math_renderer: None,
        },
    );
    assert!(database.width > 200.0);
    assert_eq!(database.line_count, 2);

    let support_db = crate::flowchart::flowchart_label_metrics_for_layout(
        crate::flowchart::FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: r"fa:fa-circle [DBServer\SharedDbInstance].[SupportDb]",
            label_type: "text",
            style: &style,
            max_width_px: Some(200.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &cfg,
            math_renderer: None,
        },
    );
    assert!(support_db.width > 200.0);
    assert_eq!(support_db.line_count, 3);
    assert!(support_db.height > database.height);
}

#[test]
fn default_font_html_advance_is_monotonic_for_appended_text() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };

    let metrics = [
        "synthetic",
        "synthetic label",
        "synthetic label with punctuation: []{}",
    ]
    .map(|text| measurer.measure_wrapped(text, &style, None, WrapMode::HtmlLike));

    for metrics in metrics {
        assert_finite_positive_metrics(metrics);
        assert_eq!(metrics.line_count, 1);
    }
    assert!(metrics.windows(2).all(|pair| pair[1].width > pair[0].width));
}

#[test]
fn default_font_repeated_glyph_runs_have_monotonic_advance() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };

    let widths = ["s", "ss", "sss", "ssss", "sssss"].map(|text| {
        let metrics = measurer.measure_wrapped(text, &style, None, WrapMode::HtmlLike);
        assert_finite_positive_metrics(metrics);
        metrics.width
    });
    assert!(
        widths.windows(2).all(|pair| pair[1] > pair[0]),
        "appending a visible glyph must increase advance: {widths:?}"
    );

    let mixed = ["ttts", "tttss", "tttsss"].map(|text| {
        measurer
            .measure_wrapped(text, &style, None, WrapMode::HtmlLike)
            .width
    });
    assert!(mixed.windows(2).all(|pair| pair[1] > pair[0]));
}

#[test]
fn flowchart_multiline_html_label_uses_widest_measured_line() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let cfg = merman_core::MermaidConfig::default();
    let lines = [
        "short run",
        "a substantially wider synthetic run",
        "middle run",
    ];
    let raw_label = lines.join("<br/>");

    let metrics = crate::flowchart::flowchart_label_metrics_for_layout(
        crate::flowchart::FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: &raw_label,
            label_type: "text",
            style: &style,
            max_width_px: None,
            wrap_mode: WrapMode::HtmlLike,
            config: &cfg,
            math_renderer: None,
        },
    );
    let widest_line = lines
        .map(|line| {
            measurer
                .measure_wrapped(line, &style, None, WrapMode::HtmlLike)
                .width
        })
        .into_iter()
        .fold(0.0, f64::max);

    assert_eq!(metrics.line_count, lines.len());
    assert_eq!(metrics.width, widest_line);
    assert!(metrics.height > style.font_size);
}

#[test]
fn default_font_ascii_punctuation_uses_canonical_profile_entries() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };

    let open_brace = measurer.measure_wrapped("{", &style, None, WrapMode::HtmlLike);
    let close_brace = measurer.measure_wrapped("}", &style, None, WrapMode::HtmlLike);
    let table = crate::generated::mermaid_font_metrics_11_16_0::lookup_font_metrics(
        FLOWCHART_DEFAULT_FONT_KEY,
        FontMetricsVariant::Regular,
    )
    .expect("default regular font profile");
    assert!(
        table
            .entries
            .binary_search_by_key(&'{', |entry| entry.0)
            .is_ok()
    );
    assert!(
        table
            .entries
            .binary_search_by_key(&'}', |entry| entry.0)
            .is_ok()
    );
    assert_finite_positive_metrics(open_brace);
    assert_finite_positive_metrics(close_brace);

    let bracketed = measurer.measure_wrapped("[x] {y} (z)", &style, None, WrapMode::HtmlLike);
    assert_finite_positive_metrics(bracketed);
    assert!(bracketed.width > open_brace.width + close_brace.width);
}

#[test]
fn default_font_nbsp_uses_its_canonical_profile_entry() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };

    let regular_space = measurer.measure_wrapped("A B", &style, None, WrapMode::HtmlLike);
    let non_breaking_space =
        measurer.measure_wrapped("A\u{00A0}B", &style, None, WrapMode::HtmlLike);

    let table = crate::generated::mermaid_font_metrics_11_16_0::lookup_font_metrics(
        FLOWCHART_DEFAULT_FONT_KEY,
        FontMetricsVariant::Regular,
    )
    .expect("default regular font profile");
    assert!(
        table
            .entries
            .binary_search_by_key(&'\u{00A0}', |entry| entry.0)
            .is_ok()
    );
    assert_finite_positive_metrics(regular_space);
    assert_finite_positive_metrics(non_breaking_space);
    assert_eq!(non_breaking_space.line_count, regular_space.line_count);
}

#[test]
fn default_font_v_comma_pair_uses_profile_advance() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };

    let v = measurer.measure_wrapped("v", &style, None, WrapMode::HtmlLike);
    let comma = measurer.measure_wrapped(",", &style, None, WrapMode::HtmlLike);
    let pair = measurer.measure_wrapped("v,", &style, None, WrapMode::HtmlLike);
    assert_finite_positive_metrics(pair);
    assert!(pair.width <= v.width + comma.width);
    assert!(pair.width > v.width.max(comma.width));
}

#[test]
fn c1_controls_use_each_profiles_generic_missing_glyph_advance() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let default_style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let courier_style = TextStyle {
        font_family: Some("courier".to_string()),
        ..default_style.clone()
    };

    let default_widths = ['\u{80}', '\u{89}', '\u{8f}', '\u{9f}'].map(|control| {
        let metrics = measurer.measure_wrapped(
            &control.to_string(),
            &default_style,
            None,
            WrapMode::HtmlLike,
        );
        assert_finite_positive_metrics(metrics);
        metrics.width
    });
    let courier_widths = ['\u{80}', '\u{89}', '\u{8f}', '\u{9f}'].map(|control| {
        let metrics = measurer.measure_wrapped(
            &control.to_string(),
            &courier_style,
            None,
            WrapMode::HtmlLike,
        );
        assert_finite_positive_metrics(metrics);
        metrics.width
    });

    assert!(default_widths.windows(2).all(|pair| pair[0] == pair[1]));
    assert!(courier_widths.windows(2).all(|pair| pair[0] == pair[1]));
    assert_ne!(default_widths[0], courier_widths[0]);
}

#[test]
fn html_measurement_ignores_inactive_wrap_limit() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let text = "untrained inactive wrap probe";

    let unwrapped = measurer.measure_wrapped(text, &style, None, WrapMode::HtmlLike);
    let wrapped = measurer.measure_wrapped(
        text,
        &style,
        Some(unwrapped.width + style.font_size),
        WrapMode::HtmlLike,
    );
    assert_same_metrics(wrapped, unwrapped);
}

#[test]
fn font_metrics_variant_tracks_css_weight_and_style() {
    let variant = |font_weight: Option<&str>, font_style: Option<&str>| {
        FontMetricsVariant::from_style(&TextStyle {
            font_family: None,
            font_size: 16.0,
            font_weight: font_weight.map(str::to_string),
            font_style: font_style.map(str::to_string),
        })
    };

    assert_eq!(variant(None, None), FontMetricsVariant::Regular);
    assert_eq!(variant(Some("600"), None), FontMetricsVariant::Bold);
    assert_eq!(
        variant(None, Some("oblique 12deg")),
        FontMetricsVariant::Italic
    );
    assert_eq!(
        variant(Some("bold"), Some("italic")),
        FontMetricsVariant::BoldItalic
    );
}

#[test]
fn font_metrics_lookup_prefers_exact_variant_and_falls_back_to_regular() {
    let regular = crate::generated::mermaid_font_metrics_11_16_0::lookup_font_metrics(
        FLOWCHART_DEFAULT_FONT_KEY,
        FontMetricsVariant::Regular,
    )
    .expect("default regular font profile");
    let bold = crate::generated::mermaid_font_metrics_11_16_0::lookup_font_metrics(
        FLOWCHART_DEFAULT_FONT_KEY,
        FontMetricsVariant::Bold,
    )
    .expect("default bold font profile");

    assert_eq!(regular.variant, FontMetricsVariant::Regular);
    assert_eq!(bold.variant, FontMetricsVariant::Bold);

    let regular_only = [*regular];
    let fallback = FontMetricsTable::lookup(
        &regular_only,
        FLOWCHART_DEFAULT_FONT_KEY,
        FontMetricsVariant::BoldItalic,
    )
    .expect("missing variants must use the regular profile");
    assert_eq!(fallback.variant, FontMetricsVariant::Regular);
    assert!(
        FontMetricsTable::lookup(&regular_only, "missing-font", FontMetricsVariant::Regular,)
            .is_none()
    );
}

#[test]
fn compact_font_metrics_rejects_palettes_that_exceed_u8_indices() {
    use super::font_metrics_data::{
        FontMetricsTableData, FontMetricsVariantData, encode_font_metrics_profile,
    };

    let entries = (0_u32..257)
        .map(|index| {
            (
                char::from_u32(index).expect("valid scalar"),
                f64::from_bits(0x3ff0_0000_0000_0000 + u64::from(index)),
            )
        })
        .collect::<Vec<_>>();
    let table = FontMetricsTableData {
        font_key: "palette-limit".to_string(),
        variant: FontMetricsVariantData::Regular,
        default_em: entries[0].1,
        entries,
        kern_pairs: Vec::new(),
        space_trigrams: Vec::new(),
        trigrams: Vec::new(),
        svg_scale: 1.0,
        svg_bbox_overhang_left_default_em: 1.0,
        svg_bbox_overhang_right_default_em: 1.0,
        svg_bbox_overhang_left: Vec::new(),
        svg_bbox_overhang_right: Vec::new(),
        svg_vertical_glyphs: Vec::new(),
        svg_vertical_profiles: approximate_svg_vertical_profiles(-0.9, 1.1, 0.0),
    };

    let error = encode_font_metrics_profile(&[table]).expect_err("257-value palette must fail");
    assert_eq!(
        error.to_string(),
        "font metrics profile error at byte 0: metric palette exceeds u8 index capacity"
    );
}

#[test]
fn compact_font_metrics_accepts_all_256_u8_palette_indices() {
    use super::font_metrics_data::{
        FontMetricsTableData, FontMetricsVariantData, decode_font_metrics_profile,
        encode_font_metrics_profile,
    };

    let entries = (0_u32..=u32::from(u8::MAX))
        .map(|index| {
            (
                char::from_u32(index).expect("valid scalar"),
                f64::from_bits(0x3ff0_0000_0000_0000 + u64::from(index)),
            )
        })
        .collect::<Vec<_>>();
    let first_value = entries[0].1;
    let table = FontMetricsTableData {
        font_key: "palette-full".to_string(),
        variant: FontMetricsVariantData::Regular,
        default_em: first_value,
        entries,
        kern_pairs: Vec::new(),
        space_trigrams: Vec::new(),
        trigrams: Vec::new(),
        svg_scale: first_value,
        svg_bbox_overhang_left_default_em: first_value,
        svg_bbox_overhang_right_default_em: first_value,
        svg_bbox_overhang_left: Vec::new(),
        svg_bbox_overhang_right: Vec::new(),
        svg_vertical_glyphs: Vec::new(),
        svg_vertical_profiles: approximate_svg_vertical_profiles(first_value, first_value, 0.0),
    };

    let encoded =
        encode_font_metrics_profile(std::slice::from_ref(&table)).expect("256-value palette");
    let key_length = usize::from(u16::from_le_bytes([encoded[10], encoded[11]]));
    let palette_count_offset = 12 + key_length + 1;
    let palette_count = u32::from_le_bytes(
        encoded[palette_count_offset..palette_count_offset + 4]
            .try_into()
            .expect("palette count"),
    );
    assert_eq!(palette_count, 256);

    let entries_count_offset = palette_count_offset + 4 + 256 * 8 + 4;
    let entries_offset = entries_count_offset + 2;
    let last_entry_index_offset = entries_offset + usize::from(u8::MAX) * 5 + 4;
    assert_eq!(encoded[last_entry_index_offset], u8::MAX);

    let decoded = decode_font_metrics_profile(&encoded).expect("decode full palette");
    assert_eq!(decoded[0].entries.len(), 256);
    assert_eq!(
        decoded[0].entries[usize::from(u8::MAX)].1.to_bits(),
        table.entries[usize::from(u8::MAX)].1.to_bits()
    );
}

#[test]
fn compact_font_metrics_round_trip_preserves_fact_bits_and_variant_fallback() {
    use super::font_metrics_data::{
        FontMetricsTableData, FontMetricsVariantData, decode_font_metrics_profile,
        decode_font_metrics_tables, encode_font_metrics_profile,
    };

    fn table(font_key: &str, variant: FontMetricsVariantData, salt: u64) -> FontMetricsTableData {
        let value = |bits| f64::from_bits(0x3fd0_0000_0000_0000_u64 + bits + salt);
        FontMetricsTableData {
            font_key: font_key.to_string(),
            variant,
            default_em: value(1),
            entries: vec![(' ', value(2)), ('~', value(3)), ('\u{00a0}', value(4))],
            kern_pairs: vec![(33, 126, value(5)), (126, 33, -0.0)],
            space_trigrams: vec![(33, 126, value(6))],
            trigrams: vec![(33, 64, 126, value(7))],
            svg_scale: value(8),
            svg_bbox_overhang_left_default_em: value(9),
            svg_bbox_overhang_right_default_em: value(10),
            svg_bbox_overhang_left: vec![('!', value(11))],
            svg_bbox_overhang_right: vec![('~', value(12))],
            svg_vertical_glyphs: vec![' ', 'ß'],
            svg_vertical_profiles: approximate_svg_vertical_profiles(
                value(13),
                value(14),
                value(15),
            ),
        }
    }

    fn fact_bits(table: &FontMetricsTableData) -> Vec<u64> {
        std::iter::once(table.default_em)
            .chain(table.entries.iter().map(|entry| entry.1))
            .chain(table.kern_pairs.iter().map(|entry| entry.2))
            .chain(table.space_trigrams.iter().map(|entry| entry.2))
            .chain(table.trigrams.iter().map(|entry| entry.3))
            .chain(std::iter::once(table.svg_scale))
            .chain(std::iter::once(table.svg_bbox_overhang_left_default_em))
            .chain(std::iter::once(table.svg_bbox_overhang_right_default_em))
            .chain(table.svg_bbox_overhang_left.iter().map(|entry| entry.1))
            .chain(table.svg_bbox_overhang_right.iter().map(|entry| entry.1))
            .map(f64::to_bits)
            .collect()
    }

    let variants = [
        FontMetricsVariantData::Regular,
        FontMetricsVariantData::Bold,
        FontMetricsVariantData::Italic,
        FontMetricsVariantData::BoldItalic,
    ];
    let mut source = variants
        .into_iter()
        .enumerate()
        .map(|(index, variant)| table("probe", variant, index as u64 * 32))
        .collect::<Vec<_>>();
    source.push(table("regular-only", FontMetricsVariantData::Regular, 256));

    let encoded = encode_font_metrics_profile(&source).expect("encode compact profile");
    let decoded = decode_font_metrics_profile(&encoded).expect("decode compact profile");
    assert_eq!(decoded.len(), source.len());
    for (actual, expected) in decoded.iter().zip(&source) {
        assert_eq!(actual.font_key, expected.font_key);
        assert_eq!(actual.variant, expected.variant);
        assert_eq!(fact_bits(actual), fact_bits(expected));
        assert_eq!(
            actual
                .entries
                .iter()
                .map(|entry| entry.0)
                .collect::<Vec<_>>(),
            expected
                .entries
                .iter()
                .map(|entry| entry.0)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            actual
                .kern_pairs
                .iter()
                .map(|entry| (entry.0, entry.1))
                .collect::<Vec<_>>(),
            expected
                .kern_pairs
                .iter()
                .map(|entry| (entry.0, entry.1))
                .collect::<Vec<_>>()
        );
        assert_eq!(actual.svg_vertical_glyphs, expected.svg_vertical_glyphs);
        assert_eq!(actual.svg_vertical_profiles, expected.svg_vertical_profiles);
    }

    let runtime = decode_font_metrics_tables(&encoded).expect("decode runtime tables");
    let fallback =
        FontMetricsTable::lookup(runtime, "regular-only", FontMetricsVariant::BoldItalic)
            .expect("missing variant falls back to regular");
    assert_eq!(fallback.variant, FontMetricsVariant::Regular);
    assert_eq!(fallback.kern_pairs[1].2.to_bits(), (-0.0_f64).to_bits());
}

#[test]
fn generated_font_metrics_keep_all_fonts_and_variants() {
    for (module, font_keys) in [
        (
            crate::generated::mermaid_font_metrics_11_16_0::lookup_font_metrics
                as fn(&str, FontMetricsVariant) -> Option<&'static FontMetricsTable>,
            &[
                "courier",
                "helveticaneue,helvetica,sans-serif",
                "sans-serif",
                "trebuchetms,verdana,arial,sans-serif",
            ][..],
        ),
        (
            crate::generated::mermaid_calculate_text_dimensions_font_metrics_11_16_0::lookup_exact_font_metrics,
            &["mermaid-calculate-text-dimensions-cssom-fallback"][..],
        ),
    ] {
        for font_key in font_keys {
            for variant in [
                FontMetricsVariant::Regular,
                FontMetricsVariant::Bold,
                FontMetricsVariant::Italic,
                FontMetricsVariant::BoldItalic,
            ] {
                let table = module(font_key, variant).expect("generated font variant");
                assert_eq!(table.font_key, *font_key);
                assert_eq!(table.variant, variant);
                assert_eq!(table.entries.len(), 100);
                assert_eq!(table.svg_vertical_glyphs.len(), 100);
                assert_eq!(
                    table.svg_vertical_profiles.len(),
                    SvgVerticalDomShape::COUNT
                );
            }
        }
    }
}

#[test]
fn generated_font_metric_blobs_have_the_exact_canonical_catalog() {
    use super::font_metrics_data::{FontMetricsVariantData, decode_font_metrics_profile};

    let variants = [
        FontMetricsVariantData::Regular,
        FontMetricsVariantData::Bold,
        FontMetricsVariantData::Italic,
        FontMetricsVariantData::BoldItalic,
    ];
    let main_expected = [
        "courier",
        "helveticaneue,helvetica,sans-serif",
        "sans-serif",
        "trebuchetms,verdana,arial,sans-serif",
    ]
    .into_iter()
    .flat_map(|font_key| variants.map(|variant| (font_key, variant)))
    .collect::<Vec<_>>();
    let calculate_text_dimensions_expected = variants
        .map(|variant| ("mermaid-calculate-text-dimensions-cssom-fallback", variant))
        .into_iter()
        .collect::<Vec<_>>();

    for (bytes, expected) in [
        (
            include_bytes!("../generated/mermaid_font_metrics_11_16_0.bin").as_slice(),
            main_expected,
        ),
        (
            include_bytes!(
                "../generated/mermaid_calculate_text_dimensions_font_metrics_11_16_0.bin"
            )
            .as_slice(),
            calculate_text_dimensions_expected,
        ),
    ] {
        let tables = decode_font_metrics_profile(bytes).expect("valid generated profile");
        let identities = tables
            .iter()
            .map(|table| (table.font_key.as_str(), table.variant))
            .collect::<Vec<_>>();
        assert_eq!(identities, expected);

        let canonical_chars = (b' '..=b'~')
            .map(char::from)
            .chain(std::iter::once('\u{00a0}'))
            .chain(['ﬂ', '°', '¶', 'ß'])
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let vertical_glyphs = (b' '..=b'~')
            .map(char::from)
            .chain(['ﬂ', '°', '¶', 'ß'])
            .chain(std::iter::once('\u{200b}'))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        for table in &tables {
            assert_eq!(
                table
                    .entries
                    .iter()
                    .map(|entry| entry.0)
                    .collect::<Vec<_>>(),
                canonical_chars
            );
            for (left, right, _) in table.kern_pairs.iter().chain(&table.space_trigrams) {
                assert!((33..=126).contains(left));
                assert!((33..=126).contains(right));
            }
            for (character, _) in table
                .svg_bbox_overhang_left
                .iter()
                .chain(&table.svg_bbox_overhang_right)
            {
                assert!(canonical_chars.contains(character));
            }
            assert_eq!(table.svg_vertical_glyphs, vertical_glyphs);
            assert_eq!(
                table.svg_vertical_profiles.len(),
                super::font_metrics_data::SvgVerticalDomShapeData::COUNT
            );
        }
    }
}

#[test]
fn calculate_text_dimensions_uses_the_body_attached_svg_profile() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif;".to_string()),
        font_size: 16.0,
        font_weight: Some("400".to_string()),
        font_style: None,
    };

    let metrics = measurer.measure_mermaid_calculate_text_dimensions(
        "This is a longer message that should be wrapped by Mermaid's default behavior",
        &style,
    );
    let selected = measure_mermaid_text_dimensions(
        &measurer,
        "This is a longer message that should be wrapped by Mermaid's default behavior",
        &style,
    );

    assert_eq!(metrics.width.round(), 510.0, "{metrics:?}");
    assert_eq!(metrics.height.round(), 17.0, "{metrics:?}");
    assert_eq!(metrics.line_count, 1);
    assert_eq!(selected.width, 510, "{selected:?}");
    assert_eq!(selected.height, 17, "{selected:?}");
    assert_eq!(selected.line_height, 17, "{selected:?}");

    let literal_text = r"multiline<br \t/>text";
    let literal_direct = measurer.measure_mermaid_calculate_text_dimensions(literal_text, &style);
    let literal_br = measure_mermaid_text_dimensions(&measurer, literal_text, &style);
    assert_eq!(
        literal_br.width, 131,
        "direct={literal_direct:?} selected={literal_br:?}"
    );
}

#[test]
fn calculate_text_dimensions_collapses_svg_tspan_ascii_whitespace() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif;".to_string()),
        font_size: 16.0,
        font_weight: Some("400".to_string()),
        font_style: None,
    };

    let single = measurer.measure_mermaid_calculate_text_dimensions("A B", &style);
    let repeated = measurer.measure_mermaid_calculate_text_dimensions("  A  B  ", &style);
    let ascii_controls = measurer.measure_mermaid_calculate_text_dimensions("\tA\n\r B\t", &style);
    let non_breaking =
        measurer.measure_mermaid_calculate_text_dimensions("A\u{00a0}\u{00a0}B", &style);

    assert_eq!(repeated.width.to_bits(), single.width.to_bits());
    assert_eq!(repeated.height.to_bits(), single.height.to_bits());
    assert_eq!(ascii_controls.width.to_bits(), single.width.to_bits());
    assert_eq!(ascii_controls.height.to_bits(), single.height.to_bits());
    assert!(non_breaking.width > single.width);
}

#[test]
fn vendored_create_text_bbox_y_operations_use_exact_profile_facts() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: Some("400".to_string()),
        font_style: None,
    };

    assert_eq!(
        measurer.measure_svg_create_text_bbox_y_offset_px("API gateway", &style),
        1.0
    );
    assert_eq!(
        measurer.measure_svg_create_text_middle_bbox_y_offset_px("API gateway", &style),
        5.1875
    );

    let unsupported = TextStyle {
        font_family: Some("unknown-font-without-a-profile".to_string()),
        ..style
    };
    assert_eq!(
        measurer.measure_svg_create_text_bbox_y_offset_px("API gateway", &unsupported),
        0.0
    );
    assert_eq!(
        measurer.measure_svg_create_text_middle_bbox_y_offset_px("API gateway", &unsupported),
        0.0
    );
}

#[test]
fn flowchart_html_font_variants_use_measured_profiles() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let regular = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let italic = TextStyle {
        font_style: Some("italic".to_string()),
        ..regular.clone()
    };
    let bold = TextStyle {
        font_weight: Some("700".to_string()),
        ..regular.clone()
    };
    let bold_italic = TextStyle {
        font_weight: Some("bold".to_string()),
        font_style: Some("oblique".to_string()),
        ..regular.clone()
    };

    let measure =
        |style| measurer.measure_wrapped("Merman 012345", style, None, WrapMode::HtmlLike);
    let regular_metrics = measure(&regular);
    let bold_metrics = measure(&bold);
    let italic_metrics = measure(&italic);
    let bold_italic_metrics = measure(&bold_italic);
    for metrics in [
        regular_metrics,
        bold_metrics,
        italic_metrics,
        bold_italic_metrics,
    ] {
        assert_finite_positive_metrics(metrics);
    }
    assert!(bold_metrics.width >= regular_metrics.width);
    assert!(bold_italic_metrics.width >= italic_metrics.width);
    assert_ne!(italic_metrics.width, regular_metrics.width);
}

#[test]
fn svg_wrapped_width_tracks_a_bounded_emitted_line() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let text = "A synthetic cluster title with punctuation: (q/r/s)";
    let unwrapped = measurer.measure_wrapped(text, &style, None, WrapMode::SvgLike);
    let metrics =
        measurer.measure_wrapped(text, &style, Some(unwrapped.width / 2.0), WrapMode::SvgLike);
    assert_finite_positive_metrics(metrics);
    assert!(metrics.line_count > 1);
    assert!(metrics.width < unwrapped.width);
    assert!(metrics.height > unwrapped.height);
}

#[test]
fn flowchart_html_punctuation_wraps_at_spaces() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let title = "Synthetic punctuation (q/r/s) + dashes - and spaces";
    let unwrapped = measurer.measure_wrapped(title, &style, None, WrapMode::HtmlLike);
    let limit = unwrapped.width / 2.0;
    let metrics = measurer.measure_wrapped(title, &style, Some(limit), WrapMode::HtmlLike);
    assert_finite_positive_metrics(metrics);
    assert!(metrics.line_count > 1);
    assert!(
        metrics.width <= limit + 1.0 / 64.0,
        "DOM width may differ from the wrap limit by at most one 1/64px lattice step: {metrics:?}, limit={limit}"
    );
    assert!(metrics.height > unwrapped.height);
}

#[test]
fn svg_and_html_text_operations_expose_distinct_bbox_metrics() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let text = "synthetic operation probe";

    let html = measurer.measure_wrapped(text, &style, None, WrapMode::HtmlLike);
    let svg = measurer.measure_wrapped(text, &style, None, WrapMode::SvgLike);
    assert_finite_positive_metrics(html);
    assert_finite_positive_metrics(svg);
    assert_ne!(svg.width, html.width);
    assert_eq!(svg.line_count, html.line_count);
    assert!(svg.height < html.height);
}

#[test]
fn flowchart_svg_layout_metrics_follow_the_shared_text_operation() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let text = "synthetic node alpha";

    let direct = measurer.measure_wrapped(text, &style, Some(200.0), WrapMode::SvgLike);
    let extended =
        measurer.measure_wrapped("synthetic node alpha beta", &style, None, WrapMode::SvgLike);
    assert!(extended.width > direct.width);

    let cfg = merman_core::MermaidConfig::default();
    let layout =
        flowchart_label_metrics_for_layout(crate::flowchart::FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: text,
            label_type: "text",
            style: &style,
            max_width_px: Some(200.0),
            wrap_mode: WrapMode::SvgLike,
            config: &cfg,
            math_renderer: None,
        });
    assert_same_metrics(layout, direct);
}

#[test]
fn courier_svg_and_html_operations_keep_operation_specific_heights() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("courier".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let text = "synthetic";

    let svg = measurer.measure_wrapped(text, &style, None, WrapMode::SvgLike);
    let html = measurer.measure_wrapped(text, &style, None, WrapMode::HtmlLike);
    assert_finite_positive_metrics(svg);
    assert_finite_positive_metrics(html);
    assert_eq!(svg.line_count, 1);
    assert_eq!(html.line_count, 1);
    assert!(svg.height < html.height);
}

#[test]
fn courier_html_dotted_identifier_overflows_without_dot_wrapping() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("courier".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let text = "Synthetic.Namespace.UnbrokenIdentifier";
    let unwrapped = measurer.measure_wrapped(text, &style, None, WrapMode::HtmlLike);
    let limit = unwrapped.width / 2.0;

    let metrics = measurer.measure_wrapped(text, &style, Some(limit), WrapMode::HtmlLike);
    assert_eq!(metrics.line_count, 1);
    assert_eq!(metrics.width, unwrapped.width);
    assert!(metrics.width > limit);
}

#[test]
fn default_font_html_hyphenated_compound_wraps_at_dynamic_limit() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let text = "Synthetic prose before half-rounded-compound suffix";
    let unwrapped = measurer.measure_wrapped(text, &style, None, WrapMode::HtmlLike);
    let limit = unwrapped.width / 2.0;

    let metrics = measurer.measure_wrapped(text, &style, Some(limit), WrapMode::HtmlLike);
    assert!(metrics.width <= limit);
    assert!(metrics.height > unwrapped.height);
    assert!(metrics.line_count > 1);
}

#[test]
fn flowchart_svg_edge_label_background_y_selects_font_profile() {
    let trebuchet = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let courier = TextStyle {
        font_family: Some("courier".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let courier_stack = TextStyle {
        font_family: Some("\"Courier New\", courier, monospace;".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
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
        font_style: None,
    };
    let courier = TextStyle {
        font_family: Some("courier".to_string()),
        font_size: 18.0,
        font_weight: None,
        font_style: None,
    };
    let courier_stack = TextStyle {
        font_family: Some("\"Courier New\", courier, monospace;".to_string()),
        font_size: 18.0,
        font_weight: None,
        font_style: None,
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
fn flowchart_title_bbox_uses_symmetric_shared_advance() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 18.0,
        font_weight: None,
        font_style: None,
    };
    let text = "synthetic title probe";

    let (left, right) = measurer.measure_svg_title_bbox_x(text, &style);
    let bbox_width = measurer.measure_svg_simple_text_bbox_width_px(text, &style);
    assert!(left.is_finite() && left > 0.0);
    assert_eq!(left, right);
    assert!(bbox_width.is_finite() && bbox_width >= left + right);
}

#[test]
fn svg_single_run_keeps_literal_br_with_backslash_t_on_one_line() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif;".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };

    // Mermaid `lineBreakRegex` should not treat this as a `<br>` break because `\\t` is a
    // literal backslash + `t`, not whitespace.
    let text = "multiline<br \\t/>text";
    assert_eq!(split_html_br_lines(text), vec![text]);

    let literal = measurer.measure_wrapped(text, &style, None, WrapMode::SvgLikeSingleRun);
    let without_literal_marker =
        measurer.measure_wrapped("multilinetext", &style, None, WrapMode::SvgLikeSingleRun);
    assert_eq!(literal.line_count, 1);
    assert!(literal.width.is_finite() && literal.width > without_literal_marker.width);
}

#[test]
fn vendored_svg_bbox_operations_scale_generalized_font_facts() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style_16 = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif;".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let style_32 = TextStyle {
        font_size: 32.0,
        ..style_16.clone()
    };
    let text = "synthetic-sequence-probe-omega-42";

    for (width_16, width_32) in [
        (
            measurer.measure_svg_simple_text_bbox_width_px(text, &style_16),
            measurer.measure_svg_simple_text_bbox_width_px(text, &style_32),
        ),
        (
            measurer.measure_svg_raw_text_bbox_width_px(text, &style_16),
            measurer.measure_svg_raw_text_bbox_width_px(text, &style_32),
        ),
        (
            measurer.measure_svg_tspan_text_bbox_width_px(text, &style_16),
            measurer.measure_svg_tspan_text_bbox_width_px(text, &style_32),
        ),
    ] {
        assert!(width_16.is_finite() && width_16 > 0.0);
        assert!(width_32.is_finite() && width_32 > width_16);
        assert!(
            (width_32 / width_16 - 2.0).abs() < 0.01,
            "font-backed SVG measurement should scale with font size: {width_16} -> {width_32}"
        );
    }
}

#[test]
fn wrap_label_like_mermaid_respects_generalized_probe_thresholds() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif;".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let text = "This is a longer message that should be wrapped by Mermaid's default behavior";

    let probe = measurer.measure_svg_simple_text_bbox_width_for_wrap_px(text, &style);
    assert!(probe.is_finite() && probe > 0.0);
    assert_eq!(
        wrap_label_like_mermaid_lines(text, &measurer, &style, probe + 1.0),
        vec![text.to_string()],
        "a threshold above the measured candidate must preserve the line"
    );

    let wrapped = wrap_label_like_mermaid_lines(text, &measurer, &style, probe / 2.0);
    assert!(
        wrapped.len() > 1,
        "a threshold below the measured candidate must use the normal Mermaid wrapLabel flow"
    );
}

#[test]
fn wrap_label_like_mermaid_does_not_split_escaped_br() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif;".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
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
fn flowchart_label_metrics_for_layout_measures_markdown_inline_html_like_mermaid() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let cfg = merman_core::MermaidConfig::default();
    let markdown = "This is **bold** </br>and <strong>strong</strong>";
    assert!(mermaid_markdown_contains_html_tags(markdown));

    let html = mermaid_markdown_to_html_label_fragment(markdown, true);
    let html_metrics =
        measure_html_with_inline_styles(&measurer, &html, &style, Some(200.0), WrapMode::HtmlLike);
    assert_finite_positive_metrics(html_metrics);
    assert_eq!(html_metrics.line_count, 2);

    let metrics = crate::flowchart::flowchart_label_metrics_for_layout(
        crate::flowchart::FlowchartLabelMetricsRequest {
            measurer: &measurer,
            raw_label: markdown,
            label_type: "markdown",
            style: &style,
            max_width_px: Some(200.0),
            wrap_mode: WrapMode::HtmlLike,
            config: &cfg,
            math_renderer: None,
        },
    );
    assert_same_metrics(metrics, html_metrics);
}

#[test]
fn html_code_elements_use_the_browser_monospace_font() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let html = r#"<p><a href="https://mermaid.js.org/"><code>note about mermaid</code></a></p>"#;

    let actual = measure_html_with_inline_styles(&measurer, html, &style, None, WrapMode::HtmlLike);
    let mut code_style = style.clone();
    code_style.font_family = Some("monospace".to_string());
    let expected =
        measurer.measure_wrapped("note about mermaid", &code_style, None, WrapMode::HtmlLike);
    let ordinary = measurer.measure_wrapped("note about mermaid", &style, None, WrapMode::HtmlLike);

    assert_same_metrics(actual, expected);
    assert_ne!(actual.width, ordinary.width);
}

#[test]
fn flowchart_html_markdown_metrics_preserve_paragraph_break_height() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let cfg = merman_core::MermaidConfig::default();
    let measure = |markdown: &str| {
        crate::flowchart::flowchart_label_metrics_for_layout(
            crate::flowchart::FlowchartLabelMetricsRequest {
                measurer: &measurer,
                raw_label: markdown,
                label_type: "markdown",
                style: &style,
                max_width_px: None,
                wrap_mode: WrapMode::HtmlLike,
                config: &cfg,
                math_renderer: None,
            },
        )
    };

    let single_paragraph = measure("Synthetic first sentence.\nSynthetic second sentence.");
    let two_paragraphs = measure("Synthetic first sentence.\n\nSynthetic second sentence.");
    assert_finite_positive_metrics(single_paragraph);
    assert_finite_positive_metrics(two_paragraphs);
    assert!(two_paragraphs.line_count > single_paragraph.line_count);
    assert!(two_paragraphs.height > single_paragraph.height);
}

#[test]
fn markdown_svg_wrapping_keeps_raw_html_tags_literal_but_wraps_like_mermaid() {
    use MermaidMarkdownWordType::*;

    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
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

#[cfg(feature = "diagram-er")]
use crate::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, TextMeasurementOperation, TextMeasurementPolicy,
    TextMeasurementProfileIdentity,
};
use crate::environment::{RenderEnvironment, TextMeasurementPhase};
#[cfg(feature = "markdown-labels")]
use crate::text::measure_markdown_with_inline_styles;
use crate::text::{
    DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle, WrapMode,
    measure_html_with_inline_styles,
};
#[cfg(feature = "markdown-labels")]
use merman_render_core::text::inline_html::style_requests_bold_font_weight;
use merman_render_core::text::measure_xhtml_label_fragment_with_terminal_fonts;
use std::cell::RefCell;
#[cfg(feature = "diagram-er")]
use std::sync::{Arc, Mutex};

type RecordedMeasurementCall = (String, Option<String>, Option<String>, Option<String>);

#[test]
fn zero_font_runs_keep_builtin_wrapping_equal_to_visible_siblings() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
    let style = TextStyle::default();
    let visible = "Alpha beta gamma delta epsilon zeta";
    let hidden = "invisible ".repeat(16);
    for width in [48.0, 96.0, 1000.0] {
        let measure = |html: &str| {
            measure_xhtml_label_fragment_with_terminal_fonts(
                &measurer,
                html,
                &style,
                Some(width),
                16.0,
            )
        };
        // Keep both controls on the mixed-font path; the legacy uniform-font path uses
        // a distinct width quantization policy that is unrelated to zero glyph advance.
        let visible = format!("<span style='font-size:18px'>{visible}</span>");
        let expected = measure(&format!("<p>{visible}</p>"));
        let actual = measure(&format!(
            "<p><span style='font-size:0px'>{hidden}</span>{visible}</p>"
        ));
        assert_eq!(actual.width, expected.width, "width={width}");
        assert_eq!(actual.height, expected.height, "width={width}");
        assert_eq!(actual.line_count, expected.line_count, "width={width}");
    }
    let zero = measure_xhtml_label_fragment_with_terminal_fonts(
        &measurer,
        "<p><span style='font-size:0px'>invisible</span></p>",
        &style,
        Some(1.0),
        16.0,
    );
    assert_eq!(zero.width, 0.0);
    assert_eq!(zero.height, 24.0);
    assert_eq!(zero.line_count, 1);
}

#[test]
fn inline_ancestor_struts_follow_text_ranges_through_wrapping_and_breaks() {
    struct FontSizedMeasurer;
    impl TextMeasurer for FontSizedMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            assert!(!text.contains('\u{fffc}'), "icons are fixed-advance boxes");
            TextMetrics {
                width: text.chars().count() as f64 * style.font_size,
                height: style.font_size * 1.5,
                line_count: 1,
            }
        }
    }
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let builtin = session.text_measurer(TextMeasurementPhase::Wrap);
    let opaque = FontSizedMeasurer;
    let style = TextStyle {
        font_size: 10.0,
        ..TextStyle::default()
    };
    for measurer in [&builtin as &dyn TextMeasurer, &opaque] {
        for (html, max_width, height, lines) in [
            (
                "<p><span style='font-size:64px'><span style='font-size:0px'>Hidden</span></span>Visible</p>",
                None,
                96.0,
                1,
            ),
            (
                "<p><span style='font-size:64px'><span style='font-size:0px'>Hidden</span></span></p>",
                None,
                96.0,
                1,
            ),
            (
                "<p><span style='font-size:64px'><span style='font-size:10px'>AA</span></span><span style='font-size:10px'> BBBB</span></p>",
                Some(25.0),
                111.0,
                2,
            ),
            (
                "<p><span style='font-size:64px'><span style='font-size:0px'>Hidden<br/>More</span></span><br/>Visible</p>",
                Some(1000.0),
                207.0,
                3,
            ),
            (
                "<p><span style='font-size:64px'><span style='font-size:0px'><br/><br/></span></span></p>",
                None,
                192.0,
                2,
            ),
            (
                "<p><span style='font-size:64px'><span style='font-size:0px'><i class='fa fa-car'></i></span></span>Visible</p>",
                Some(1000.0),
                96.0,
                1,
            ),
        ] {
            let measured = measure_xhtml_label_fragment_with_terminal_fonts(
                measurer, html, &style, max_width, 16.0,
            );
            assert_eq!(measured.height, height, "{html}: {measured:?}");
            assert_eq!(measured.line_count, lines, "{html}: {measured:?}");
        }
        for html in [
            "<p><span style='font-size:128px'></span>Visible</p>",
            "<p><img style='font-size:128px'/>Visible</p>",
            "<p><span style='font-size:128px'><img/></span>Visible</p>",
            "<p><span style='font-size:128px'><span style='font-size:0px'> </span></span>Visible</p>",
            "<p>Visible<span style='font-size:128px'><span style='font-size:10px'> </span></span></p>",
        ] {
            for max_width in [None, Some(1000.0)] {
                let measured = measure_xhtml_label_fragment_with_terminal_fonts(
                    measurer, html, &style, max_width, 16.0,
                );
                assert_eq!(measured.height, 15.0, "{html}: {measured:?}");
                assert_eq!(measured.line_count, 1, "{html}: {measured:?}");
            }
        }
        let zero = TextStyle {
            font_size: 0.0,
            ..style.clone()
        };
        for max_width in [None, Some(1000.0)] {
            let measured = measure_xhtml_label_fragment_with_terminal_fonts(
                measurer,
                "<p><span style='font-size:64px'><span style='font-size:0px'>Hidden</span></span><br/><span style='font-size:10px'>Visible</span></p>",
                &zero,
                max_width,
                16.0,
            );
            assert_eq!(measured.height, 111.0);
            assert_eq!(measured.line_count, 2);
        }
    }
}

#[test]
fn inline_ancestor_struts_do_not_split_opaque_font_requests() {
    #[derive(Default)]
    struct Probe(RefCell<Vec<(String, f64)>>);
    impl TextMeasurer for Probe {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.0.borrow_mut().push((text.to_owned(), style.font_size));
            TextMetrics {
                width: text.chars().count() as f64 * style.font_size,
                height: style.font_size * 1.5,
                line_count: 1,
            }
        }
    }
    let style = TextStyle {
        font_size: 10.0,
        ..TextStyle::default()
    };
    for max_width in [None, Some(25.0), Some(1000.0)] {
        let control = Probe::default();
        let expected = measure_xhtml_label_fragment_with_terminal_fonts(
            &control,
            "<p><span style='font-size:10px'>AA BBBB</span></p>",
            &style,
            max_width,
            16.0,
        );
        let probe = Probe::default();
        let measured = measure_xhtml_label_fragment_with_terminal_fonts(
            &probe,
            "<p><span style='font-size:64px'><span style='font-size:10px'>AA</span></span><span style='font-size:10px'> BBBB</span></p>",
            &style,
            max_width,
            16.0,
        );
        assert_eq!(*probe.0.borrow(), *control.0.borrow());
        assert_eq!(measured.width, expected.width);
        assert_eq!(measured.line_count, expected.line_count);
        assert_eq!(measured.height - expected.height, 81.0);
    }
}

#[derive(Default)]
struct RecordingMeasurer {
    calls: RefCell<Vec<RecordedMeasurementCall>>,
}

impl TextMeasurer for RecordingMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.measure_wrapped(text, style, None, WrapMode::HtmlLike)
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        _max_width: Option<f64>,
        _wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.calls.borrow_mut().push((
            text.to_string(),
            style.font_weight.clone(),
            style.font_style.clone(),
            style.font_family.clone(),
        ));
        TextMetrics {
            width: text.len() as f64,
            height: style.font_size,
            line_count: 1,
        }
    }
}

#[test]
fn html_emphasis_resolves_each_parent_before_measuring_terminal_fonts() {
    // WHATWG's default strong/b weight is bolder; CSS Fonts computes it against the parent.
    // These expectations also match Chromium computed styles, including nested emphasis.
    for (parent_weight, strong, nested) in [
        (100, 400, 700),
        (300, 400, 700),
        (400, 700, 900),
        (500, 700, 900),
        (600, 900, 900),
        (700, 900, 900),
        (900, 900, 900),
    ] {
        let base = TextStyle {
            font_size: 16.0,
            font_weight: Some(parent_weight.to_string()),
            font_style: Some("oblique".to_owned()),
            ..TextStyle::default()
        };
        for (html, weight, font_style) in [
            ("<strong>A</strong>", strong, "oblique"),
            ("<b><strong>A</strong></b>", nested, "oblique"),
            ("<em>A</em>", parent_weight, "italic"),
            ("<i>A</i>", parent_weight, "italic"),
            (
                "<strong style='font-weight:inherit'>A</strong>",
                parent_weight,
                "oblique",
            ),
            ("<b style='font-weight:normal'>A</b>", 400, "oblique"),
            ("<b style='font-weight:initial'>A</b>", 400, "oblique"),
            (
                "<em style='font-style:inherit'>A</em>",
                parent_weight,
                "oblique",
            ),
            (
                "<em style='font-style:initial'>A</em>",
                parent_weight,
                "normal",
            ),
            (
                "<b style='font-weight:500!important;font-weight:900'>A</b>",
                500,
                "oblique",
            ),
        ] {
            for max_width in [None, Some(1000.0)] {
                for prepared in [false, true] {
                    let measurer = RecordingMeasurer::default();
                    if prepared {
                        measure_xhtml_label_fragment_with_terminal_fonts(
                            &measurer, html, &base, max_width, 16.0,
                        );
                    } else {
                        measure_html_with_inline_styles(
                            &measurer,
                            html,
                            &base,
                            max_width,
                            WrapMode::HtmlLike,
                        );
                    }
                    let calls = measurer.calls.borrow();
                    let (_, actual_weight, actual_style, _) =
                        calls.last().expect("terminal font measurement");
                    assert_eq!(
                        actual_weight.as_deref(),
                        Some(weight.to_string().as_str()),
                        "{parent_weight}/{html}/{max_width:?}/prepared={prepared}: {calls:?}"
                    );
                    assert_eq!(actual_style.as_deref(), Some(font_style), "{html}");
                }
            }
        }
    }
}

#[test]
fn html_phrasing_defaults_and_author_overrides_use_the_same_font_stack() {
    let base = TextStyle {
        font_family: Some("serif".to_owned()),
        font_style: Some("oblique".to_owned()),
        ..TextStyle::default()
    };
    for (tag, property, expected, override_value) in [
        ("cite", "font-style", "italic", "normal"),
        ("dfn", "font-style", "italic", "normal"),
        ("var", "font-style", "italic", "normal"),
        ("code", "font-family", "monospace", "serif"),
        ("kbd", "font-family", "monospace", "serif"),
        ("samp", "font-family", "monospace", "serif"),
        ("tt", "font-family", "monospace", "serif"),
    ] {
        for override_default in [false, true] {
            let attr = if override_default {
                format!(" style='{property}:{override_value}!important'")
            } else {
                String::new()
            };
            let measurer = RecordingMeasurer::default();
            measure_html_with_inline_styles(
                &measurer,
                &format!("<{tag}{attr}>A</{tag}>"),
                &base,
                None,
                WrapMode::HtmlLike,
            );
            let calls = measurer.calls.borrow();
            let (_, _, actual_style, actual_family) = calls.last().unwrap();
            let actual = if property == "font-style" {
                actual_style
            } else {
                actual_family
            };
            assert_eq!(
                actual.as_deref(),
                Some(if override_default {
                    override_value
                } else {
                    expected
                }),
                "{tag}/{attr}"
            );
        }
    }
}

#[test]
fn html_terminal_fonts_restore_the_parent_after_nested_and_void_elements() {
    let measurer = RecordingMeasurer::default();
    let base = TextStyle {
        font_size: 16.0,
        font_weight: Some("400".to_owned()),
        ..TextStyle::default()
    };
    measure_html_with_inline_styles(
        &measurer,
        "<strong>A<strong>B</strong>C</strong>D<img style='font-weight:100'>E",
        &base,
        None,
        WrapMode::HtmlLike,
    );
    let calls = measurer.calls.borrow();
    for (text, expected) in [
        ("A", "700"),
        ("B", "900"),
        ("C", "700"),
        ("D", "400"),
        ("E", "400"),
    ] {
        assert!(
            calls
                .iter()
                .any(|(actual, weight, ..)| actual == text && weight.as_deref() == Some(expected)),
            "{text} must use {expected}: {calls:?}"
        );
    }
}

#[test]
fn prepared_xhtml_terminal_fonts_keep_inline_sizes_empty_rows_and_wrapped_heights() {
    struct FontSizedMeasurer;
    impl TextMeasurer for FontSizedMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            assert!(
                !text.contains('\u{fffc}'),
                "inline boxes are not glyph probes"
            );
            TextMetrics {
                width: text.chars().count() as f64 * style.font_size,
                height: style.font_size * 1.5,
                line_count: 1,
            }
        }
    }
    let style = TextStyle {
        font_size: 10.0,
        ..TextStyle::default()
    };
    for (html, expected_width, expected_height, expected_lines) in [
        ("<p><big>AA</big></p>", 24.0, 18.0, 1),
        ("<p><small>A</small></p>", 10.0 / 1.2, 15.0, 1),
        ("<p><big>A</big><br/>B</p>", 12.0, 33.0, 2),
        ("<p><big>A<br/><br/>B</big></p>", 12.0, 54.0, 3),
        ("<p>A<big><br/></big>B</p>", 10.0, 33.0, 2),
        ("<p><big><br/><br/></big></p>", 0.0, 36.0, 2),
        ("<p>A<br/><br/><img/></p>", 10.0, 30.0, 2),
        (
            "<p style='font-size:10px'>A<br/><br/><img/></p>",
            10.0,
            30.0,
            2,
        ),
        (
            "<p><big><i class='fa fa-car'></i>A</big></p>",
            27.0,
            18.0,
            1,
        ),
        (
            "<p><big style='font-size:inherit'>A</big></p>",
            10.0,
            15.0,
            1,
        ),
        (
            "<p><span style='font-size:2rem'>A</span></p>",
            32.0,
            48.0,
            1,
        ),
        (
            "<p>A<span style='font-size:100px'> </span></p>",
            10.0,
            15.0,
            1,
        ),
    ] {
        for max_width in [None, Some(1000.0)] {
            let measured = measure_xhtml_label_fragment_with_terminal_fonts(
                &FontSizedMeasurer,
                html,
                &style,
                max_width,
                16.0,
            );
            assert!(
                (measured.width - expected_width).abs() < 1.0 / 64.0,
                "{html}: {measured:?}"
            );
            assert!(
                (measured.height - expected_height).abs() < 1e-9,
                "{html}: {measured:?}"
            );
            assert_eq!(measured.line_count, expected_lines, "{html}");
        }
    }
    let wrapped = measure_xhtml_label_fragment_with_terminal_fonts(
        &FontSizedMeasurer,
        "<p><big>A A</big></p>",
        &style,
        Some(15.0),
        16.0,
    );
    assert_eq!(wrapped.line_count, 2);
    assert_eq!(wrapped.height, 36.0);

    let wrapped_icon = measure_xhtml_label_fragment_with_terminal_fonts(
        &FontSizedMeasurer,
        "<p><big><i class='fa fa-car'></i>A</big></p>",
        &style,
        Some(15.0),
        16.0,
    );
    assert_eq!(wrapped_icon.width, 15.0);
    assert_eq!(wrapped_icon.line_count, 2);
    assert_eq!(wrapped_icon.height, 36.0);
    let plain_break = measure_xhtml_label_fragment_with_terminal_fonts(
        &FontSizedMeasurer,
        "<p>A A A<br/>B</p>",
        &style,
        Some(15.0),
        16.0,
    );
    let enlarged_break = measure_xhtml_label_fragment_with_terminal_fonts(
        &FontSizedMeasurer,
        "<p>A A A<big><br/></big>B</p>",
        &style,
        Some(15.0),
        16.0,
    );
    assert_eq!(plain_break.line_count, enlarged_break.line_count);
    assert_eq!(enlarged_break.height - plain_break.height, 3.0);
}

#[test]
fn terminal_font_line_boxes_keep_the_hosts_fractional_height() {
    struct FractionalLineHeight;
    impl TextMeasurer for FractionalLineHeight {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: text.len() as f64 * style.font_size,
                height: 62.203125,
                line_count: 1,
            }
        }
    }
    let style = TextStyle {
        font_size: 24.0,
        ..TextStyle::default()
    };
    for max_width in [None, Some(10000.0)] {
        let metrics = measure_xhtml_label_fragment_with_terminal_fonts(
            &FractionalLineHeight,
            "<p><big><big><big>Account</big></big></big></p>",
            &style,
            max_width,
            16.0,
        );
        assert_eq!(metrics.height, 62.203125);
    }
}

#[cfg(feature = "markdown-labels")]
#[derive(Default)]
struct SoftHyphenMeasurer {
    calls: RefCell<Vec<RecordedMeasurementCall>>,
    kern_hyphen: bool,
}

#[cfg(feature = "markdown-labels")]
impl TextMeasurer for SoftHyphenMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.measure_wrapped(text, style, None, WrapMode::HtmlLike)
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        _max_width: Option<f64>,
        _wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.calls.borrow_mut().push((
            text.to_string(),
            style.font_weight.clone(),
            style.font_style.clone(),
            style.font_family.clone(),
        ));
        let advance = if style_requests_bold_font_weight(style) {
            20.0
        } else {
            10.0
        };
        let kerning = if self.kern_hyphen && text.len() > 1 && text.ends_with('-') {
            5.0
        } else {
            0.0
        };
        TextMetrics {
            width: text.chars().filter(|ch| *ch != '\u{00ad}').count() as f64 * advance - kerning,
            height: style.font_size,
            line_count: 1,
        }
    }
}

#[cfg(feature = "markdown-labels")]
#[test]
fn svg_markdown_resets_parent_weight_without_changing_html_inheritance() {
    let measurer = SoftHyphenMeasurer::default();
    let style = TextStyle {
        font_weight: Some("700".to_owned()),
        font_style: Some("italic".to_owned()),
        ..TextStyle::default()
    };
    let svg = measure_markdown_with_inline_styles(
        &measurer,
        "plain **bold**",
        &style,
        None,
        WrapMode::SvgLike,
    );
    assert_eq!(svg.width, 150.0);
    let calls = measurer.calls.borrow();
    assert!(
        calls.iter().any(|(text, weight, italic, _)| {
            text == "plain"
                && weight.as_deref() == Some("normal")
                && italic.as_deref() == Some("normal")
        }),
        "{calls:?}"
    );
    drop(calls);
    let html = measure_markdown_with_inline_styles(
        &measurer,
        "plain **bold**",
        &style,
        None,
        WrapMode::HtmlLike,
    );
    assert_eq!(html.width, 200.0);
}

#[cfg(feature = "markdown-labels")]
#[test]
fn html_markdown_measurement_keeps_code_span_literals() {
    let metrics = measure_markdown_with_inline_styles(
        &SoftHyphenMeasurer::default(),
        "inline: `**not bold**`",
        &TextStyle::default(),
        None,
        WrapMode::HtmlLike,
    );
    assert_eq!(metrics.width, 220.0);
}

#[cfg(feature = "diagram-er")]
#[test]
fn font_aware_xhtml_preserves_unstyled_opaque_requests() {
    let legacy = RecordingMeasurer::default();
    let terminal = RecordingMeasurer::default();
    let style = TextStyle::default();
    let html = "<p>aa <strong>BB</strong> cc</p>";
    let expected =
        measure_html_with_inline_styles(&legacy, html, &style, Some(4.0), WrapMode::HtmlLike);
    let actual =
        measure_xhtml_label_fragment_with_terminal_fonts(&terminal, html, &style, Some(4.0), 16.0);
    assert_eq!(terminal.calls.into_inner(), legacy.calls.into_inner());
    assert_eq!(actual.width, expected.width);
    assert_eq!(actual.height, expected.height);
    assert_eq!(actual.line_count, expected.line_count);
}

#[cfg(feature = "diagram-er")]
#[test]
fn wrapped_terminal_fonts_keep_builtin_and_opaque_line_selection() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let builtin = session.text_measurer(TextMeasurementPhase::Wrap);
    let opaque = DeterministicTextMeasurer::default();
    let style = TextStyle {
        font_size: 14.0,
        ..TextStyle::default()
    };
    for html in [
        "<p><span style='font-size:64px'>A B C D E</span></p>",
        "<p><span style='font-size:64px'>A</span> B C D E F G H I J K L M N O P</p>",
        concat!(
            "<p><span style='font-size:64px;font-family:monospace;font-weight:700'>",
            "AB&shy;CD EF</span> <span style='font-size:28px;font-style:italic'>GH IJ</span></p>",
        ),
    ] {
        for width in [40.0, 100.0, 1000.0] {
            let expected = measure_xhtml_label_fragment_with_terminal_fonts(
                &opaque,
                html,
                &style,
                Some(width),
                16.0,
            );
            let actual = measure_xhtml_label_fragment_with_terminal_fonts(
                &builtin,
                html,
                &style,
                Some(width),
                16.0,
            );
            assert_eq!(actual.width, expected.width, "{html} at {width}");
            assert_eq!(actual.height, expected.height, "{html} at {width}");
            assert_eq!(actual.line_count, expected.line_count, "{html} at {width}");
        }
    }
    let mixed = measure_xhtml_label_fragment_with_terminal_fonts(
        &builtin,
        "<p><span style='font-size:64px'>A</span> B C D E F G H I J K L M N O P</p>",
        &style,
        Some(100.0),
        16.0,
    );
    assert!(mixed.line_count > 1);
    assert_eq!(mixed.height, 96.0 + (mixed.line_count - 1) as f64 * 21.0);
}

#[cfg(feature = "diagram-er")]
#[test]
fn wrapped_terminal_fonts_preserve_natural_width_activation_and_min_content() {
    let measurer = DeterministicTextMeasurer {
        char_width_factor: 1.0,
        ..DeterministicTextMeasurer::default()
    };
    let style = TextStyle {
        font_size: 14.0,
        ..TextStyle::default()
    };
    let html = "<p><span style='font-size:64px'>A   B</span></p>";
    let fits = measure_xhtml_label_fragment_with_terminal_fonts(
        &measurer,
        html,
        &style,
        Some(193.0),
        16.0,
    );
    assert_eq!((fits.width, fits.height, fits.line_count), (192.0, 96.0, 1));
    let activated = measure_xhtml_label_fragment_with_terminal_fonts(
        &measurer,
        html,
        &style,
        Some(192.0),
        16.0,
    );
    assert_eq!(
        (activated.width, activated.height, activated.line_count),
        (192.0, 192.0, 2),
        "equality activates break-spaces and retains the authored spaces",
    );
    let min_content = measure_xhtml_label_fragment_with_terminal_fonts(
        &measurer,
        "<p><span style='font-size:64px'>ABCDE</span></p>",
        &style,
        Some(100.0),
        16.0,
    );
    assert_eq!(
        (
            min_content.width,
            min_content.height,
            min_content.line_count
        ),
        (320.0, 96.0, 1),
        "an unbreakable styled word expands min-content instead of clipping its viewport",
    );
}

#[cfg(feature = "diagram-er")]
#[test]
fn wrapped_terminal_fonts_route_inherited_fonts_and_selected_heights_to_the_host() {
    type FontCall = (TextMeasurementOperation, String, TextStyle, Option<f64>);
    struct FontHost(Arc<Mutex<Vec<FontCall>>>);

    impl HostTextMeasurer for FontHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            self.0.lock().unwrap().push((
                request.operation,
                request.text.to_owned(),
                request.style.clone(),
                request.max_width,
            ));
            let width = request.text.chars().count() as f64 * request.style.font_size;
            Ok(Some(match request.operation {
                TextMeasurementOperation::CanvasMeasureTextWidth => {
                    HostTextMeasurement::Length(width)
                }
                TextMeasurementOperation::HtmlLineHeight => {
                    HostTextMeasurement::Length(request.style.font_size * 2.0)
                }
                _ => HostTextMeasurement::Metrics(TextMetrics {
                    width,
                    height: request.style.font_size * 2.0,
                    line_count: 1,
                }),
            }))
        }
    }

    let calls = Arc::new(Mutex::new(Vec::new()));
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.wrapped-terminal-font-host").unwrap(),
        "v1",
    )
    .unwrap();
    let policy = TextMeasurementPolicy::host_display(
        identity,
        Arc::new(FontHost(Arc::clone(&calls))),
        [TextMeasurementPhase::Wrap],
    );
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(policy)
        .begin_session()
        .unwrap();
    let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
    let style = TextStyle {
        font_size: 14.0,
        ..TextStyle::default()
    };
    let html = concat!(
        "<p><span style='font-size:64px;font-family:monospace;font-weight:700;font-style:italic'>",
        "<strong style='font-size:inherit;font-family:inherit;font-weight:inherit;font-style:inherit'>",
        "A</strong></span><br/>B<br/>C</p>",
    );
    let metrics = measure_xhtml_label_fragment_with_terminal_fonts(
        &measurer,
        html,
        &style,
        Some(100.0),
        16.0,
    );
    assert_eq!(
        (metrics.width, metrics.height, metrics.line_count),
        (64.0, 184.0, 3)
    );
    let calls = calls.lock().unwrap();
    assert_eq!(calls[0].0, TextMeasurementOperation::Wrapped);
    assert_eq!(calls[0].1, "A\nB\nC");
    assert_eq!(calls[0].2.font_size, 14.0);
    assert_eq!(calls[0].3, Some(100.0));
    for operation in [
        TextMeasurementOperation::CanvasMeasureTextWidth,
        TextMeasurementOperation::HtmlLineHeight,
    ] {
        let a_calls = calls
            .iter()
            .filter(|call| call.0 == operation && call.1 == "A")
            .collect::<Vec<_>>();
        assert!(!a_calls.is_empty());
        for call in a_calls {
            assert_eq!(call.2.font_size, 64.0);
            assert_eq!(call.2.font_family.as_deref(), Some("monospace"));
            assert_eq!(call.2.font_weight.as_deref(), Some("700"));
            assert_eq!(call.2.font_style.as_deref(), Some("italic"));
        }
    }
    for text in ["B", "C"] {
        let line_calls = calls
            .iter()
            .filter(|call| call.0 == TextMeasurementOperation::HtmlLineHeight && call.1 == text)
            .collect::<Vec<_>>();
        assert!(!line_calls.is_empty());
        assert!(line_calls.iter().all(|call| call.2.font_size == 14.0));
    }
}

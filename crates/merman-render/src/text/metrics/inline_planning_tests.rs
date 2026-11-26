use super::*;
use crate::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use crate::text::DeterministicTextMeasurer;
use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex};

fn inline_html_carrier<M: TextMeasurer + ?Sized>(measurer: &M) -> InlineHtmlMeasurementCarrier {
    measurer
        .builtin_operation_carrier(TextMeasurementOperation::WrappedWithRawWidth)
        .and_then(BuiltinTextMeasurementOperationCarrier::into_inline_html)
        .unwrap_or_else(InlineHtmlMeasurementCarrier::opaque)
}

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

#[test]
fn inline_ancestor_ranges_keep_soft_hyphen_and_streaming_font_groups() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
    let style = TextStyle {
        font_size: 10.0,
        ..TextStyle::default()
    };
    let font = Arc::new(style.clone());
    for text in ["AA BBBB", "AA\u{00ad}BBBB"] {
        let mut run = InlineTextRun {
            text: text.to_owned(),
            terminal_style: Some(Arc::clone(&font)),
            ..Default::default()
        };
        run.ancestor_struts.push(InlineAncestorStrut {
            start: 0,
            end: 2,
            font_size: 64.0,
        });
        let control = InlineTextRun {
            ancestor_struts: Vec::new(),
            ..run.clone()
        };
        for width in [15.0, 25.0, 1000.0] {
            let mut reference_stats = InlineHtmlPlanningStats::default();
            let reference = measure_inline_html_line_layout_with_carrier_and_stats(
                &measurer,
                inline_html_carrier(&measurer),
                std::slice::from_ref(&control),
                &style,
                Some(width),
                &mut reference_stats,
            );
            let mut stats = InlineHtmlPlanningStats::default();
            let measured = measure_inline_html_line_layout_with_carrier_and_stats(
                &measurer,
                inline_html_carrier(&measurer),
                std::slice::from_ref(&run),
                &style,
                Some(width),
                &mut stats,
            );
            assert_eq!(measured.line_count, reference.line_count);
            assert_eq!(measured.wrapped_width, reference.wrapped_width);
            assert_eq!(measured.height - reference.height, 81.0);
            assert_eq!(
                stats.builtin_stream_bytes,
                reference_stats.builtin_stream_bytes
            );
            assert_eq!(stats.backend_requests, reference_stats.backend_requests);
            assert_eq!(
                stats.backend_request_bytes,
                reference_stats.backend_request_bytes
            );
            assert_eq!(
                stats.builtin_checkpoint_copies,
                reference_stats.builtin_checkpoint_copies
            );
            assert_eq!(stats.measurement_payload_copy_bytes, 0);
        }
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
fn terminal_font_heights_measure_selected_line_groups_without_repeating_whole_runs() {
    #[derive(Default)]
    struct HeightProbe(RefCell<Vec<String>>);
    impl TextMeasurer for HeightProbe {
        fn measure(&self, text: &str, _style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: text.chars().filter(|ch| *ch != '\u{00ad}').count() as f64,
                height: 24.0,
                line_count: 1,
            }
        }

        fn measure_html_line_height_px(&self, text: &str, _style: &TextStyle) -> f64 {
            self.0.borrow_mut().push(text.to_string());
            24.0
        }
    }
    let style = TextStyle {
        font_size: 16.0,
        ..TextStyle::default()
    };
    for words in [3, 256, 4096] {
        let text = vec!["x"; words].join(" ");
        let probe = HeightProbe::default();
        let metrics = measure_xhtml_label_fragment_with_terminal_fonts(
            &probe,
            &format!("<p><span style='font-size:16px'>{text}</span></p>"),
            &style,
            Some(1_000_000.0),
            16.0,
        );
        assert_eq!(metrics.height, 24.0);
        assert_eq!(metrics.line_count, 1);
        let calls = probe.0.borrow();
        assert_eq!(calls.as_slice(), [text.as_str()]);
        assert_eq!(calls.iter().map(String::len).sum::<usize>(), text.len());
    }
    for (text, budget, expected) in [
        ("x x x x", 4.0, &["x x ", "x x"][..]),
        ("ab\u{00ad}cd", 3.0, &["ab-", "cd"][..]),
    ] {
        let probe = HeightProbe::default();
        let runs = [InlineTextRun {
            text: text.to_string(),
            terminal_style: Some(Arc::new(style.clone())),
            ..Default::default()
        }];
        let layout = measure_inline_html_line_layout(&probe, &runs, &style, Some(budget));
        assert_eq!(layout.line_count, 2);
        assert_eq!(layout.height, 48.0);
        assert_eq!(probe.0.borrow().as_slice(), expected);
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

struct ScriptedStatefulMeasurer {
    widths: Vec<f64>,
    next: Cell<usize>,
    calls: RefCell<Vec<String>>,
}

struct RecordingHostMeasurer {
    calls: Arc<Mutex<Vec<(TextMeasurementOperation, String)>>>,
}

impl HostTextMeasurer for RecordingHostMeasurer {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        self.calls
            .lock()
            .expect("host call log lock")
            .push((request.operation, request.text.to_string()));
        if request.operation == TextMeasurementOperation::CanvasMeasureTextWidth {
            Ok(Some(HostTextMeasurement::Length(request.text.len() as f64)))
        } else {
            Ok(Some(HostTextMeasurement::Metrics(TextMetrics {
                width: request.text.len() as f64,
                height: request.style.font_size,
                line_count: 1,
            })))
        }
    }
}

impl ScriptedStatefulMeasurer {
    fn new(widths: Vec<f64>) -> Self {
        Self {
            widths,
            next: Cell::new(0),
            calls: RefCell::new(Vec::new()),
        }
    }
}

impl TextMeasurer for ScriptedStatefulMeasurer {
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
        let index = self.next.get();
        self.next.set(index + 1);
        self.calls.borrow_mut().push(text.to_string());
        TextMetrics {
            width: self.widths[index],
            height: style.font_size,
            line_count: 1,
        }
    }
}

fn run(text: impl Into<String>, variant: usize) -> InlineTextRun {
    InlineTextRun {
        text: text.into(),
        bold: variant == 1,
        italic: variant == 2,
        code: variant == 3,
        terminal_style: None,
        replaced_width_px: None,
        ancestor_struts: Vec::new(),
    }
}

fn fixed_byte_runs(
    source_bytes: usize,
    run_count: usize,
    break_count: usize,
) -> Vec<InlineTextRun> {
    assert!(source_bytes > 0);
    assert!((1..=source_bytes).contains(&run_count));
    assert!(break_count < source_bytes);

    let mut text = vec![b'a'; source_bytes];
    for index in 0..break_count {
        let offset = (index + 1) * source_bytes / (break_count + 1);
        text[offset] = b' ';
    }

    let mut runs = Vec::with_capacity(run_count);
    let mut start = 0usize;
    for index in 0..run_count {
        let end = (index + 1) * source_bytes / run_count;
        runs.push(run(
            String::from_utf8(text[start..end].to_vec()).expect("ASCII workload"),
            index % 4,
        ));
        start = end;
    }
    runs
}

fn fragment_text(source: &str, fragments: &[InlineRunFragment]) -> String {
    let mut text = String::new();
    for fragment in fragments {
        text.push_str(&source[fragment.start..fragment.end]);
    }
    text
}

#[test]
fn indexed_break_walk_preserves_unicode_and_cross_run_fragments() {
    let runs = vec![
        run("A\u{301} ", 0),
        run("👩‍💻 ", 1),
        run("مرحبا ", 2),
        run("世界", 3),
    ];
    let expected_text = runs.iter().map(|run| run.text.as_str()).collect::<String>();
    let expected_segments = html_break_spaces_segments(&expected_text);
    let mut stats = InlineHtmlPlanningStats::default();
    let plan = index_inline_run_breaks(&runs, &mut stats);
    let actual_segments = plan
        .segments
        .iter()
        .map(|segment| {
            fragment_text(
                &plan.source,
                &plan.fragments[segment.fragment_start..segment.fragment_end],
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        actual_segments,
        expected_segments
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
    );
    assert_eq!(stats.source_bytes, expected_text.len());
    assert_eq!(stats.break_segments, actual_segments.len());
    assert!(stats.fragment_refs < runs.len() + actual_segments.len());
    assert_eq!(stats.run_visits, runs.len() * 2 + stats.fragment_refs);
}

#[derive(Default)]
struct SoftHyphenMeasurer {
    calls: RefCell<Vec<RecordedMeasurementCall>>,
    kern_hyphen: bool,
}

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

#[test]
fn soft_hyphen_inline_min_content_includes_the_selected_glyph() {
    let measurer = SoftHyphenMeasurer::default();
    let style = TextStyle::default();
    let runs = vec![run("alpha\u{00ad}beta", 0)];
    let layout = measure_inline_html_line_layout(&measurer, &runs, &style, Some(50.0));
    assert_eq!(layout.natural_width, 90.0);
    assert_eq!(layout.min_content_width, 60.0);
    assert_eq!(layout.wrapped_width, 60.0);
    assert_eq!(layout.line_count, 2);

    let unbroken = measure_inline_html_line_layout(&measurer, &runs, &style, None);
    assert_eq!(unbroken.wrapped_width, 90.0);
    assert_eq!(unbroken.line_count, 1);
    let terminal =
        measure_inline_html_line_layout(&measurer, &[run("alpha\u{00ad}", 0)], &style, Some(50.0));
    assert_eq!(terminal.natural_width, 50.0);
    assert_eq!(terminal.min_content_width, 50.0);
    assert_eq!(terminal.line_count, 1);
}

#[test]
fn soft_hyphen_inline_glyph_inherits_its_run_and_parent_style() {
    let measurer = SoftHyphenMeasurer::default();
    let mut style = TextStyle {
        font_family: Some("inherited-family".to_string()),
        ..TextStyle::default()
    };
    let runs = vec![run("al", 0), run("pha\u{00ad}", 1), run("beta", 2)];
    let layout = measure_inline_html_line_layout(&measurer, &runs, &style, Some(100.0));
    assert_eq!(layout.natural_width, 120.0);
    assert_eq!(layout.min_content_width, 100.0);
    assert_eq!(layout.wrapped_width, 100.0);
    assert_eq!(layout.line_count, 2);
    assert!(measurer.calls.borrow().contains(&(
        "pha-".to_string(),
        Some("700".to_string()),
        None,
        Some("inherited-family".to_string()),
    )));

    style.font_weight = Some("700".to_string());
    let layout = measure_inline_html_line_layout(
        &measurer,
        &[run("alpha\u{00ad}beta", 0)],
        &style,
        Some(120.0),
    );
    assert_eq!(layout.min_content_width, 120.0);
    assert_eq!(layout.wrapped_width, 120.0);
    assert_eq!(layout.line_count, 2);
}

#[test]
fn soft_hyphen_inline_rewinds_to_an_earlier_fitting_break() {
    let measurer = SoftHyphenMeasurer::default();
    let layout = measure_inline_html_line_layout(
        &measurer,
        &[run("x alpha\u{00ad}beta", 0)],
        &TextStyle::default(),
        Some(70.0),
    );
    assert_eq!(layout.natural_width, 110.0);
    assert_eq!(layout.min_content_width, 60.0);
    assert_eq!(layout.wrapped_width, 60.0);
    assert_eq!(layout.line_count, 3);
}

#[test]
fn soft_hyphen_inline_callback_shapes_the_complete_visible_style_group() {
    let measurer = SoftHyphenMeasurer {
        kern_hyphen: true,
        ..Default::default()
    };
    let layout = measure_inline_html_line_layout(
        &measurer,
        &[run("al", 0), run("pha\u{00ad}beta", 0)],
        &TextStyle::default(),
        Some(55.0),
    );
    assert_eq!(layout.min_content_width, 55.0);
    assert_eq!(layout.wrapped_width, 55.0);
    assert_eq!(layout.line_count, 2);
    let calls = measurer.calls.borrow();
    assert!(calls.iter().any(|call| call.0 == "alpha-"));
    assert!(calls.iter().all(|call| call.0 != "-"));
}

#[test]
fn soft_hyphen_inline_complete_word_fit_does_not_select_a_wider_hyphen() {
    struct WideHyphenMeasurer;
    impl TextMeasurer for WideHyphenMeasurer {
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
            TextMetrics {
                width: text
                    .chars()
                    .map(|ch| match ch {
                        '\u{00ad}' => 0.0,
                        '-' => 30.0,
                        _ => 10.0,
                    })
                    .sum(),
                height: style.font_size,
                line_count: 1,
            }
        }
    }
    let layout = measure_inline_html_line_layout(
        &WideHyphenMeasurer,
        &[run("alpha\u{00ad}b", 0)],
        &TextStyle::default(),
        Some(60.0),
    );
    assert_eq!(layout.natural_width, 60.0);
    assert_eq!(layout.wrapped_width, 60.0);
    assert_eq!(layout.line_count, 1);
}

#[test]
fn soft_hyphen_inline_html_entity_preserves_conditional_styled_measurement() {
    let measurer = SoftHyphenMeasurer::default();
    let layout = measure_html_with_inline_styles(
        &measurer,
        "<strong>alpha&shy;</strong>beta",
        &TextStyle::default(),
        Some(120.0),
        WrapMode::HtmlLike,
    );
    assert_eq!(layout.width, 120.0);
    assert_eq!(layout.line_count, 2);
    assert!(
        measurer
            .calls
            .borrow()
            .iter()
            .any(|call| { call.0 == "alpha-" && call.1.as_deref() == Some("700") })
    );
}

#[test]
fn soft_hyphen_inline_stream_preserves_combining_boundaries_in_one_chunk() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
    let carrier = inline_html_carrier(&measurer);
    let style = TextStyle {
        font_size: 10.0,
        ..TextStyle::default()
    };
    let runs = [run("", 1)];
    let mut profiles = BuiltinInlineWidthProfiles::new(carrier, &runs);
    let mut stats = InlineHtmlPlanningStats::default();
    let mut group = BuiltinInlineGroupWidth::new(&mut profiles, 0, &runs, &style, &mut stats);
    let text = format!("a{}", "\u{00ad}\u{0301}".repeat(8192));
    group.push_text(&text, &mut stats);
    assert_eq!(group.width.pushed_chunks, 1);
    assert_eq!(group.width_px(), 4.3);

    let mut stats = InlineHtmlPlanningStats::default();
    let runs = vec![run(format!("W {text}"), 1)];
    let layout = measure_inline_html_line_layout_with_carrier_and_stats(
        &measurer,
        carrier,
        &runs,
        &style,
        Some(15.0),
        &mut stats,
    );
    assert_eq!(layout.line_count, 2);
    assert!((layout.natural_width - 16.1).abs() < 1.0e-12);
    assert_eq!(stats.source_bytes, runs[0].text.len());
    assert!(
        stats.builtin_stream_bytes <= stats.source_bytes * 3,
        "stats={stats:?}"
    );
}

#[test]
fn soft_hyphen_inline_builtin_and_opaque_routes_choose_the_same_lines() {
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("deterministic render session");
    let builtin = session.text_measurer(TextMeasurementPhase::Wrap);
    let deterministic = DeterministicTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    for runs in [
        vec![run("alpha\u{00ad}beta", 0)],
        vec![run("x ", 2), run("alpha\u{00ad}", 1), run("beta", 0)],
        vec![run("ab\u{00ad}cd\u{00ad}ef\u{00ad}gh", 0)],
    ] {
        for max_width in [20.0, 32.0, 48.0] {
            let expected =
                measure_inline_html_line_layout(&deterministic, &runs, &style, Some(max_width));
            let actual = measure_inline_html_line_layout_with_carrier(
                &builtin,
                inline_html_carrier(&builtin),
                &runs,
                &style,
                Some(max_width),
            );
            if expected.natural_width > max_width {
                assert_layout_eq(actual, expected);
            } else {
                assert_eq!(actual.line_count, expected.line_count);
                assert_eq!(actual.wrapped_width, expected.wrapped_width);
            }
        }
    }
}

#[test]
fn opaque_measurer_observes_the_legacy_request_order() {
    let measurer = RecordingMeasurer::default();
    let style = TextStyle {
        font_family: Some("sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let runs = vec![run("aa ", 0), run("BB", 1), run(" cc", 0)];
    let mut stats = InlineHtmlPlanningStats::default();

    let layout =
        measure_inline_html_line_layout_with_stats(&measurer, &runs, &style, Some(4.0), &mut stats);

    let calls = measurer.calls.into_inner();
    let regular = |text: &str| (text.to_string(), None, None, Some("sans-serif".to_string()));
    let bold = |text: &str| {
        (
            text.to_string(),
            Some("700".to_string()),
            None,
            Some("sans-serif".to_string()),
        )
    };
    assert_eq!(
        calls,
        vec![
            regular("aa "),
            bold("BB"),
            regular(" cc"),
            regular("aa "),
            bold("BB"),
            regular(" "),
            regular("cc"),
            regular("aa "),
            regular("aa "),
            bold("BB"),
            regular(" "),
            regular("aa "),
            bold("BB"),
            regular(" cc"),
            bold("BB"),
            regular(" "),
            regular("cc"),
        ]
    );
    assert_eq!(layout.line_count, 3);
    assert_eq!(layout.natural_width, 8.0);
    assert_eq!(layout.min_content_width, 3.0);
    assert_eq!(layout.wrapped_width, 3.0);
    assert_eq!(stats.backend_requests, calls.len());
    assert_eq!(
        stats.backend_request_bytes,
        calls.iter().map(|(text, ..)| text.len()).sum::<usize>()
    );
}

#[test]
fn opaque_same_style_fragments_preserve_payload_order_without_recopying() {
    let measurer = RecordingMeasurer::default();
    let style = TextStyle {
        font_family: Some("sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let runs = vec![run("aa ", 0), run("bb ", 0), run("cc", 0)];
    let mut stats = InlineHtmlPlanningStats::default();

    let layout =
        measure_inline_html_line_layout_with_stats(&measurer, &runs, &style, Some(4.0), &mut stats);

    assert_eq!(
        measurer
            .calls
            .into_inner()
            .into_iter()
            .map(|(text, ..)| text)
            .collect::<Vec<_>>(),
        vec![
            "aa ", "bb ", "cc", "aa ", "bb ", "cc", "aa ", "aa bb ", "aa ", "bb cc", "bb ", "cc",
        ]
    );
    assert_eq!(layout.line_count, 3);
    assert_eq!(stats.measurement_payload_copy_bytes, 0);
}

#[test]
fn opaque_public_html_operation_preserves_the_full_request_order() {
    let measurer = RecordingMeasurer::default();
    let style = TextStyle {
        font_family: Some("sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };

    let metrics = measure_html_with_inline_styles(
        &measurer,
        "aa <strong>BB</strong> cc",
        &style,
        Some(4.0),
        WrapMode::HtmlLike,
    );

    let calls = measurer.calls.into_inner();
    let regular = |text: &str| (text.to_string(), None, None, Some("sans-serif".to_string()));
    let bold = |text: &str| {
        (
            text.to_string(),
            Some("700".to_string()),
            None,
            Some("sans-serif".to_string()),
        )
    };
    let layout_pass = vec![
        regular("aa "),
        bold("BB"),
        regular(" cc"),
        regular("aa "),
        bold("BB"),
        regular(" "),
        regular("cc"),
        regular("aa "),
        regular("aa "),
        bold("BB"),
        regular(" "),
        regular("aa "),
        bold("BB"),
        regular(" cc"),
        bold("BB"),
        regular(" "),
        regular("cc"),
    ];
    let mut expected = vec![
        regular("aa BB cc"),
        regular("aa "),
        bold("BB"),
        regular(" cc"),
    ];
    // Mermaid remeasures after switching from nowrap to break-spaces at the width boundary.
    expected.extend(layout_pass.iter().cloned());
    expected.extend(layout_pass);
    assert_eq!(calls, expected);
    assert_eq!(metrics.width, 4.0);
    assert_eq!(metrics.line_count, 3);
}

#[test]
fn opaque_public_html_operation_preserves_failure_position() {
    struct PanickingMeasurer {
        panic_at: usize,
        calls: RefCell<Vec<String>>,
    }

    impl TextMeasurer for PanickingMeasurer {
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
            let call_index = self.calls.borrow().len() + 1;
            self.calls.borrow_mut().push(text.to_string());
            assert_ne!(call_index, self.panic_at, "scripted measurement failure");
            TextMetrics {
                width: text.len() as f64,
                height: style.font_size,
                line_count: 1,
            }
        }
    }

    let measurer = PanickingMeasurer {
        panic_at: 5,
        calls: RefCell::new(Vec::new()),
    };
    let style = TextStyle {
        font_family: Some("sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        measure_html_with_inline_styles(
            &measurer,
            "aa <strong>BB</strong> cc",
            &style,
            Some(4.0),
            WrapMode::HtmlLike,
        )
    }));

    assert!(result.is_err());
    assert_eq!(
        measurer.calls.into_inner(),
        vec!["aa BB cc", "aa ", "BB", " cc", "aa "]
    );
}

#[test]
fn opaque_stateful_measurer_keeps_fit_and_overflow_call_semantics() {
    let measurer = ScriptedStatefulMeasurer::new(vec![
        10.0, 10.0, 10.0, // natural width
        3.0, 2.0, 1.0, 2.0, // min-content segments
        3.0, // first candidate
        3.0, 2.0, 1.0, // second candidate fits
        20.0, 2.0, 4.0, // third candidate overflows
        3.0, 2.0, 1.0, // committed checkpoint
        2.0, // final segment
    ]);
    let style = TextStyle {
        font_family: Some("sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let runs = vec![run("aa ", 0), run("BB", 1), run(" cc", 0)];
    let mut stats = InlineHtmlPlanningStats::default();

    let layout = measure_inline_html_line_layout_with_stats(
        &measurer,
        &runs,
        &style,
        Some(25.0),
        &mut stats,
    );

    assert_eq!(
        measurer.calls.into_inner(),
        vec![
            "aa ", "BB", " cc", "aa ", "BB", " ", "cc", "aa ", "aa ", "BB", " ", "aa ", "BB",
            " cc", "aa ", "BB", " ", "cc",
        ]
    );
    assert_eq!(layout.natural_width, 30.0);
    assert_eq!(layout.min_content_width, 3.0);
    assert_eq!(layout.wrapped_width, 6.0);
    assert_eq!(layout.line_count, 2);
    assert_eq!(stats.backend_requests, 18);
}

#[test]
fn host_route_keeps_the_full_opaque_html_operation_trace() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.inline-recording-host")
            .expect("valid measurement profile id"),
        "v1",
    )
    .expect("valid measurement profile identity");
    let policy = TextMeasurementPolicy::host_display(
        identity,
        Arc::new(RecordingHostMeasurer {
            calls: Arc::clone(&calls),
        }),
        [TextMeasurementPhase::Wrap],
    );
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(policy)
        .begin_session()
        .expect("begin host render session");
    let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
    let style = TextStyle {
        font_family: Some("sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let metrics = measure_html_with_inline_styles(
        &measurer,
        "aa <strong>BB</strong> cc",
        &style,
        Some(4.0),
        WrapMode::HtmlLike,
    );

    let calls = calls.lock().expect("host call log lock").clone();
    assert_eq!(calls[0].0, TextMeasurementOperation::Wrapped);
    assert!(
        calls[1..].iter().all(|(operation, _)| {
            *operation == TextMeasurementOperation::CanvasMeasureTextWidth
        })
    );
    assert_eq!(
        calls
            .iter()
            .map(|(_, text)| text.as_str())
            .collect::<Vec<_>>(),
        {
            let layout_pass = [
                "aa ", "BB", " cc", "aa ", "BB", " ", "cc", "aa ", "aa ", "BB", " ", "aa ", "BB",
                " cc", "BB", " ", "cc",
            ];
            let mut expected = vec!["aa BB cc", "aa ", "BB", " cc"];
            expected.extend(layout_pass);
            expected.extend(layout_pass);
            expected
        }
    );
    assert_eq!(metrics.width, 4.0);
    assert_eq!(metrics.line_count, 3);
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

fn assert_layout_eq(left: InlineHtmlLineLayout, right: InlineHtmlLineLayout) {
    assert!((left.natural_width - right.natural_width).abs() <= 1.0e-9);
    assert!((left.wrapped_width - right.wrapped_width).abs() <= 1.0e-9);
    assert!((left.min_content_width - right.min_content_width).abs() <= 1.0e-9);
    assert_eq!(left.line_count, right.line_count);
}

fn assert_layout_bits_eq(left: InlineHtmlLineLayout, right: InlineHtmlLineLayout, max_width: f64) {
    assert_eq!(
        left.natural_width.to_bits(),
        right.natural_width.to_bits(),
        "natural width mismatch at max_width={max_width}: left={left:?}, right={right:?}"
    );
    assert_eq!(
        left.wrapped_width.to_bits(),
        right.wrapped_width.to_bits(),
        "wrapped width mismatch at max_width={max_width}: left={left:?}, right={right:?}"
    );
    if right.natural_width > max_width {
        assert_eq!(
            left.min_content_width.to_bits(),
            right.min_content_width.to_bits()
        );
    }
    assert_eq!(
        left.line_count, right.line_count,
        "line-count mismatch at max_width={max_width}: left={left:?}, right={right:?}"
    );
}

fn next_down(value: f64) -> f64 {
    debug_assert!(value.is_finite() && value > 0.0);
    f64::from_bits(value.to_bits() - 1)
}

fn next_up(value: f64) -> f64 {
    debug_assert!(value.is_finite() && value >= 0.0);
    f64::from_bits(value.to_bits() + 1)
}

#[test]
fn qualified_builtin_routes_preserve_ulp_wrap_boundaries_and_backend_sum_order() {
    fn assert_backend(
        backend: &dyn TextMeasurer,
        routed: &dyn TextMeasurer,
        carrier: InlineHtmlMeasurementCarrier,
        style: &TextStyle,
    ) {
        let runs = vec![run("AV ", 0), run("To ", 1), run("fi", 0)];
        let bold_style = inline_text_style(style, true, false, false);
        let boundary = backend
            .measure_wrapped("AV ", style, None, WrapMode::HtmlLike)
            .width
            + backend
                .measure_wrapped("To ", &bold_style, None, WrapMode::HtmlLike)
                .width;

        for max_width in [next_down(boundary), boundary, next_up(boundary)] {
            let expected = measure_inline_html_line_layout(backend, &runs, style, Some(max_width));
            let actual = measure_inline_html_line_layout_with_carrier(
                routed,
                carrier,
                &runs,
                style,
                Some(max_width),
            );
            assert_layout_bits_eq(actual, expected, max_width);
        }

        let same_style_runs = vec![run("A", 0), run("V ", 0), run("office ", 0), run("fi", 0)];
        for max_width in [32.0, 64.0, 96.0] {
            let expected =
                measure_inline_html_line_layout(backend, &same_style_runs, style, Some(max_width));
            let actual = measure_inline_html_line_layout_with_carrier(
                routed,
                carrier,
                &same_style_runs,
                style,
                Some(max_width),
            );
            assert_layout_bits_eq(actual, expected, max_width);
        }

        let escaped_break_runs = vec![run("wide<br   />tail end", 0)];
        for max_width in [24.0, 64.0, 128.0] {
            let expected = measure_inline_html_line_layout(
                backend,
                &escaped_break_runs,
                style,
                Some(max_width),
            );
            let actual = measure_inline_html_line_layout_with_carrier(
                routed,
                carrier,
                &escaped_break_runs,
                style,
                Some(max_width),
            );
            assert_layout_bits_eq(actual, expected, max_width);

            let html = "wide&lt;br   /&gt;tail end";
            let expected = measure_html_with_inline_styles(
                backend,
                html,
                style,
                Some(max_width),
                WrapMode::HtmlLike,
            );
            let actual = measure_html_with_inline_styles(
                routed,
                html,
                style,
                Some(max_width),
                WrapMode::HtmlLike,
            );
            assert_eq!(actual.width.to_bits(), expected.width.to_bits());
            assert_eq!(actual.height.to_bits(), expected.height.to_bits());
            assert_eq!(actual.line_count, expected.line_count);
        }
    }

    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let builtin_session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("deterministic render session");
    let builtin = builtin_session.text_measurer(TextMeasurementPhase::Wrap);
    let builtin_carrier = inline_html_carrier(&builtin);
    assert_backend(
        &DeterministicTextMeasurer::default(),
        &builtin,
        builtin_carrier,
        &style,
    );
    let mut unknown_font_style = style.clone();
    unknown_font_style.font_family = Some("fixture-private-font".to_string());
    assert_backend(
        &DeterministicTextMeasurer::default(),
        &builtin,
        builtin_carrier,
        &unknown_font_style,
    );

    let deterministic_session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .expect("deterministic text session");
    let deterministic = deterministic_session.text_measurer(TextMeasurementPhase::Wrap);
    assert_backend(
        &DeterministicTextMeasurer::default(),
        &deterministic,
        inline_html_carrier(&deterministic),
        &style,
    );
}

#[test]
fn qualified_builtin_stream_reports_only_actual_backend_requests() {
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("deterministic render session");
    let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
    let style = TextStyle {
        font_family: Some("sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let html = "alpha <strong>BETA </strong>omega";

    let _ =
        measure_html_with_inline_styles(&measurer, html, &style, Some(48.0), WrapMode::HtmlLike);

    let report = session.text_measurement_report();
    assert_eq!(report.entries().len(), 1);
    let summary = &report.entries()[0];
    assert_eq!(
        summary.provenance().operation,
        TextMeasurementOperation::CanvasMeasureTextWidth
    );
    assert!(
        (3..=6).contains(&summary.count()),
        "each of the three visible runs should require at most one fit and one overflow probe"
    );
}

#[test]
fn qualified_builtin_fit_path_skips_break_indexing_and_stream_replay() {
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("deterministic render session");
    let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
    let style = TextStyle {
        font_family: Some("sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let runs = vec![run("alpha beta ", 0), run("gamma delta", 1)];
    let natural_width =
        measure_inline_runs_width_px(&measurer, &runs, &style, WrapMode::HtmlLike, false);

    for max_width in [None, Some(natural_width), Some(natural_width + 1.0)] {
        let mut stats = InlineHtmlPlanningStats::default();
        let layout = measure_inline_html_line_layout_with_carrier_and_stats(
            &measurer,
            inline_html_carrier(&measurer),
            &runs,
            &style,
            max_width,
            &mut stats,
        );

        assert_eq!(layout.natural_width.to_bits(), natural_width.to_bits());
        assert_eq!(layout.wrapped_width.to_bits(), natural_width.to_bits());
        assert_eq!(layout.line_count, 1);
        assert_eq!(stats.backend_requests, runs.len());
        assert_eq!(stats.break_segments, 0);
        assert_eq!(stats.fragment_refs, 0);
        assert_eq!(stats.builtin_stream_bytes, 0);
        assert_eq!(stats.builtin_checkpoint_copies, 0);
    }
}

#[test]
fn qualified_builtin_routes_match_opaque_layout_for_unicode_and_rollback() {
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let runs = vec![
        run("A\u{301} alpha ", 0),
        run("👩‍💻 beta ", 1),
        run("مرحبا gamma ", 2),
        run("世界 delta", 3),
    ];
    let html =
        "A\u{301} alpha <strong>👩‍💻 beta </strong><em>مرحبا gamma </em><code>世界 delta</code>";

    let builtin_session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("deterministic render session");
    let builtin = builtin_session.text_measurer(TextMeasurementPhase::Wrap);
    let builtin_expected = measure_inline_html_line_layout(
        &DeterministicTextMeasurer::default(),
        &runs,
        &style,
        Some(96.0),
    );
    let builtin_actual = measure_inline_html_line_layout_with_carrier(
        &builtin,
        inline_html_carrier(&builtin),
        &runs,
        &style,
        Some(96.0),
    );
    assert_layout_eq(builtin_actual, builtin_expected);
    let builtin_actual =
        measure_html_with_inline_styles(&builtin, html, &style, Some(96.0), WrapMode::HtmlLike);
    let builtin_expected = measure_html_with_inline_styles(
        &DeterministicTextMeasurer::default(),
        html,
        &style,
        Some(96.0),
        WrapMode::HtmlLike,
    );
    assert_eq!(builtin_actual.width, builtin_expected.width);
    assert_eq!(builtin_actual.height, builtin_expected.height);
    assert_eq!(builtin_actual.line_count, builtin_expected.line_count);

    let mut unknown_font_style = style.clone();
    unknown_font_style.font_family = Some("fixture-private-font".to_string());
    let fallback_expected = measure_inline_html_line_layout(
        &DeterministicTextMeasurer::default(),
        &runs,
        &unknown_font_style,
        Some(96.0),
    );
    let fallback_actual = measure_inline_html_line_layout_with_carrier(
        &builtin,
        inline_html_carrier(&builtin),
        &runs,
        &unknown_font_style,
        Some(96.0),
    );
    assert_layout_eq(fallback_actual, fallback_expected);
    let fallback_actual = measure_html_with_inline_styles(
        &builtin,
        html,
        &unknown_font_style,
        Some(96.0),
        WrapMode::HtmlLike,
    );
    let fallback_expected = measure_html_with_inline_styles(
        &DeterministicTextMeasurer::default(),
        html,
        &unknown_font_style,
        Some(96.0),
        WrapMode::HtmlLike,
    );
    assert_eq!(fallback_actual.width, fallback_expected.width);
    assert_eq!(fallback_actual.height, fallback_expected.height);
    assert_eq!(fallback_actual.line_count, fallback_expected.line_count);

    let deterministic_session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .expect("deterministic text session");
    let deterministic = deterministic_session.text_measurer(TextMeasurementPhase::Wrap);
    let deterministic_expected = measure_inline_html_line_layout(
        &DeterministicTextMeasurer::default(),
        &runs,
        &style,
        Some(96.0),
    );
    let deterministic_actual = measure_inline_html_line_layout_with_carrier(
        &deterministic,
        inline_html_carrier(&deterministic),
        &runs,
        &style,
        Some(96.0),
    );
    assert_layout_eq(deterministic_actual, deterministic_expected);
    let deterministic_actual = measure_html_with_inline_styles(
        &deterministic,
        html,
        &style,
        Some(96.0),
        WrapMode::HtmlLike,
    );
    let deterministic_expected = measure_html_with_inline_styles(
        &DeterministicTextMeasurer::default(),
        html,
        &style,
        Some(96.0),
        WrapMode::HtmlLike,
    );
    assert_eq!(deterministic_actual.width, deterministic_expected.width);
    assert_eq!(deterministic_actual.height, deterministic_expected.height);
    assert_eq!(
        deterministic_actual.line_count,
        deterministic_expected.line_count
    );
}

#[test]
fn rich_html_path_handles_many_styles_breaks_entities_and_unicode() {
    let measurer = DeterministicTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let mut html = String::new();
    for index in 0..1024 {
        let (open, close) = match index % 3 {
            0 => ("<strong>", "</strong>"),
            1 => ("<em>", "</em>"),
            _ => ("<code>", "</code>"),
        };
        html.push_str(open);
        html.push_str("a ");
        html.push_str(close);
    }
    html.push_str("&amp;\r\n<br>👩‍💻 مرحبا 世界");

    let metrics =
        measure_html_with_inline_styles(&measurer, &html, &style, Some(96.0), WrapMode::HtmlLike);

    assert!(metrics.width.is_finite() && metrics.width > 0.0);
    assert!(metrics.height.is_finite() && metrics.height > 0.0);
    assert!(metrics.line_count > 2);
}

#[test]
fn inline_planner_work_is_additive_for_fixed_byte_r_by_k_matrix() {
    const SOURCE_BYTES: usize = 4096;
    let style = TextStyle {
        font_family: Some("sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };

    for run_count in [1, 32, 256] {
        for break_count in [0, 31, 255] {
            let runs = fixed_byte_runs(SOURCE_BYTES, run_count, break_count);
            let measurer = RecordingMeasurer::default();
            let mut stats = InlineHtmlPlanningStats::default();
            let _ = measure_inline_html_line_layout_with_stats(
                &measurer,
                &runs,
                &style,
                Some(32.0),
                &mut stats,
            );

            let segment_count = break_count + 1;
            let fragment_bound = run_count + break_count;
            assert_eq!(stats.source_bytes, SOURCE_BYTES);
            assert_eq!(stats.break_segments, segment_count);
            assert!(stats.fragment_refs <= fragment_bound);
            assert_eq!(stats.fragment_capacity_refs, fragment_bound);
            assert_eq!(stats.run_visits, run_count * 2 + stats.fragment_refs);
            assert!(stats.line_plan_updates <= segment_count * 2);
            assert_eq!(stats.line_plan_bytes, std::mem::size_of::<InlineLinePlan>());

            let temporary_byte_bound = SOURCE_BYTES
                + segment_count * std::mem::size_of::<&str>()
                + fragment_bound * std::mem::size_of::<InlineRunFragment>()
                + segment_count * std::mem::size_of::<InlineBreakSegment>()
                + fragment_bound * std::mem::size_of::<usize>()
                + std::mem::size_of::<InlineLinePlan>();
            assert!(stats.logical_temporary_bytes() <= temporary_byte_bound);
            assert_eq!(stats.backend_requests, measurer.calls.borrow().len());
            assert_eq!(
                stats.backend_request_bytes,
                measurer
                    .calls
                    .borrow()
                    .iter()
                    .map(|(text, ..)| text.len())
                    .sum::<usize>()
            );
        }
    }
}

#[test]
fn opaque_same_style_active_line_work_is_linear_in_fragments_and_requests() {
    const SOURCE_BYTES: usize = 4096;
    let style = TextStyle {
        font_family: Some("sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };

    for run_count in [32, 128, 256] {
        for segment_count in [32, 64, 128] {
            let mut runs = fixed_byte_runs(SOURCE_BYTES, run_count, segment_count - 1);
            for run in &mut runs {
                run.bold = false;
                run.italic = false;
                run.code = false;
            }
            let measurer = RecordingMeasurer::default();
            let mut stats = InlineHtmlPlanningStats::default();
            let _ = measure_inline_html_line_layout_with_stats(
                &measurer,
                &runs,
                &style,
                Some(32.0),
                &mut stats,
            );

            let fragment_requests = stats.backend_requests - run_count;
            assert_eq!(
                stats.fragment_measure_visits,
                stats.fragment_refs + fragment_requests,
                "r={run_count}, k={segment_count}, stats={stats:?}"
            );
            assert_eq!(stats.measurement_payload_copy_bytes, 0);
        }
    }
}

#[test]
fn opaque_same_style_utf8_fragments_borrow_source_across_rollback() {
    let measurer = RecordingMeasurer::default();
    let style = TextStyle {
        font_family: Some("sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let runs = vec![run("A\u{301} ", 0), run("👩‍💻 ", 0), run("世界", 0)];
    let mut stats = InlineHtmlPlanningStats::default();

    let layout =
        measure_inline_html_line_layout_with_stats(&measurer, &runs, &style, Some(8.0), &mut stats);

    assert!(layout.line_count > 1);
    assert_eq!(stats.measurement_payload_copy_bytes, 0);
}

#[test]
fn builtin_terminal_line_height_excludes_collapsed_tails_and_keeps_visible_boxes() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let builtin = session.text_measurer(TextMeasurementPhase::Wrap);
    let opaque = DeterministicTextMeasurer::default();
    let base = TextStyle {
        font_size: 16.0,
        ..Default::default()
    };
    for (html, expected_height) in [
        ("A<span style='font-size:100px'> </span>", 24.0),
        ("<span style='font-size:100px'>&nbsp;</span>", 150.0),
        (
            "<span style='font-size:100px'><i class='fa fa-car'></i></span>",
            150.0,
        ),
    ] {
        for max_width in [None, Some(1000.0)] {
            let expected = measure_html_with_inline_styles(
                &opaque,
                html,
                &base,
                max_width,
                WrapMode::HtmlLike,
            );
            let actual = measure_html_with_inline_styles(
                &builtin,
                html,
                &base,
                max_width,
                WrapMode::HtmlLike,
            );
            assert_eq!(actual.height, expected_height, "{html}/{max_width:?}");
            assert_eq!(actual.width, expected.width, "{html}/{max_width:?}");
            assert_eq!(actual.height, expected.height, "{html}/{max_width:?}");
            assert_eq!(
                actual.line_count, expected.line_count,
                "{html}/{max_width:?}"
            );
        }
    }
}

#[test]
fn builtin_terminal_fonts_keep_linear_stream_work_near_the_natural_width() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
    let base = TextStyle::default();
    for words in [64, 512, 8192] {
        for run_count in [1, 16] {
            let mut runs = (0..run_count)
                .map(|index| InlineTextRun {
                    text: "x ".repeat(words / run_count),
                    terminal_style: Some(Arc::new(TextStyle {
                        font_size: if index % 2 == 0 { 16.0 } else { 22.0 },
                        font_weight: Some("900".to_owned()),
                        ..base.clone()
                    })),
                    ..Default::default()
                })
                .collect::<Vec<_>>();
            runs.last_mut().unwrap().text.pop();
            let bytes: usize = runs.iter().map(|run| run.text.len()).sum();
            let natural =
                measure_inline_runs_width_px(&measurer, &runs, &base, WrapMode::HtmlLike, false);
            let mut stats = InlineHtmlPlanningStats::default();
            let layout = measure_inline_html_line_layout_with_carrier_and_stats(
                &measurer,
                inline_html_carrier(&measurer),
                &runs,
                &base,
                Some(natural - 0.01),
                &mut stats,
            );
            assert_eq!(layout.line_count, 2, "{words}/{run_count}");
            assert_eq!(stats.backend_requests, run_count);
            assert_eq!(stats.backend_request_bytes, bytes);
            assert!(
                stats.builtin_stream_bytes <= 3 * bytes,
                "{words}/{run_count}: {stats:?}"
            );
            assert!(stats.builtin_profile_initializations <= run_count);
            assert_eq!(
                stats.builtin_profile_cache_bytes,
                std::mem::size_of::<BuiltinInlineWidthProfiles>()
                    + run_count * std::mem::size_of::<Option<BuiltinHtmlLineWidth>>()
            );
        }
    }

    // Retain a small same-font reference to expose the growing-prefix cost of the old route.
    for words in [32, 64, 128] {
        let runs = [InlineTextRun {
            text: vec!["x"; words].join(" "),
            terminal_style: Some(Arc::new(base.clone())),
            ..Default::default()
        }];
        let natural = measurer.measure_html_line_width_px(&runs[0].text, &base);
        let mut stats = InlineHtmlPlanningStats::default();
        measure_inline_html_line_layout_with_stats(
            &measurer,
            &runs,
            &base,
            Some(natural - 0.01),
            &mut stats,
        );
        assert!(
            stats.backend_request_bytes >= words * words,
            "{words}: {stats:?}"
        );
    }
}

#[test]
fn builtin_terminal_font_streaming_matches_the_opaque_width_and_height_reference() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
    let base = TextStyle::default();
    let large = Arc::new(TextStyle {
        font_size: 24.0,
        font_weight: Some("900".to_owned()),
        ..base.clone()
    });
    let small = Arc::new(TextStyle {
        font_size: 12.0,
        font_style: Some("italic".to_owned()),
        ..base.clone()
    });
    for text in [
        "alpha beta gamma",
        "alpha\u{00ad}beta gamma",
        "A\u{301} 👩‍💻 世界\u{200b}مرحبا",
    ] {
        for with_icon in [false, true] {
            let mut runs = vec![InlineTextRun {
                text: "alpha ".to_owned(),
                terminal_style: Some(Arc::clone(&large)),
                ..Default::default()
            }];
            if with_icon {
                runs.push(InlineTextRun {
                    text: "\u{fffc}".to_owned(),
                    terminal_style: Some(Arc::clone(&large)),
                    replaced_width_px: Some(30.0),
                    ..Default::default()
                });
            }
            runs.push(InlineTextRun {
                text: text.to_owned(),
                terminal_style: Some(Arc::clone(&small)),
                ..Default::default()
            });
            for max_width in [None, Some(10.0), Some(48.0), Some(120.0), Some(10000.0)] {
                let expected = measure_inline_html_line_layout(&measurer, &runs, &base, max_width);
                let actual = measure_inline_html_line_layout_with_carrier(
                    &measurer,
                    inline_html_carrier(&measurer),
                    &runs,
                    &base,
                    max_width,
                );
                assert!((actual.natural_width - expected.natural_width).abs() < 1e-9);
                assert!(
                    (actual.wrapped_width - expected.wrapped_width).abs() < 1e-9,
                    "{text}/{with_icon}/{max_width:?}: {actual:?} != {expected:?}"
                );
                assert_eq!(actual.line_count, expected.line_count);
                assert_eq!(actual.height, expected.height);
                assert_eq!(actual.last_line_height, expected.last_line_height);
            }
        }
    }
}

#[test]
fn qualified_builtin_active_line_work_is_linear_for_r_by_k_matrix() {
    const SOURCE_BYTES: usize = 4096;
    let style = TextStyle {
        font_family: Some("sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("deterministic render session");
    let measurer = session.text_measurer(TextMeasurementPhase::Wrap);

    for run_count in [1, 32, 128] {
        for segment_count in [16, 32, 64, 128] {
            let runs = fixed_byte_runs(SOURCE_BYTES, run_count, segment_count - 1);
            let natural_width =
                measure_inline_runs_width_px(&measurer, &runs, &style, WrapMode::HtmlLike, false);
            let mut stats = InlineHtmlPlanningStats::default();
            let layout = measure_inline_html_line_layout_with_carrier_and_stats(
                &measurer,
                inline_html_carrier(&measurer),
                &runs,
                &style,
                Some(natural_width - 1.0e-6),
                &mut stats,
            );

            let fragment_bound = run_count + segment_count - 1;
            assert_eq!(stats.source_bytes, SOURCE_BYTES);
            assert_eq!(stats.break_segments, segment_count);
            assert!(stats.fragment_refs <= fragment_bound);
            assert!(stats.fragment_measure_visits >= stats.fragment_refs * 2);
            assert!(stats.fragment_measure_visits <= stats.fragment_refs * 3);
            assert!(stats.line_plan_updates <= segment_count * 2);
            assert_eq!(
                stats.line_plan_bytes,
                std::mem::size_of::<BuiltinInlineLineWidth>()
            );
            assert_eq!(stats.measurement_payload_copy_bytes, 0);
            assert_eq!(stats.backend_requests, run_count);
            assert_eq!(stats.backend_request_bytes, SOURCE_BYTES);
            assert!(stats.builtin_stream_bytes >= SOURCE_BYTES * 2);
            assert!(
                stats.builtin_stream_bytes <= SOURCE_BYTES * 3,
                "r={run_count}, k={segment_count}, stats={stats:?}"
            );
            assert!(stats.builtin_stream_scalars <= stats.builtin_stream_bytes);
            assert_eq!(stats.builtin_profile_initializations, run_count.min(4));
            assert_eq!(
                stats.builtin_profile_cache_bytes,
                std::mem::size_of::<BuiltinInlineWidthProfiles>()
            );
            assert!(stats.builtin_group_state_copies <= stats.fragment_measure_visits);
            assert_eq!(stats.builtin_checkpoint_copies, segment_count);
            let temporary_byte_bound = SOURCE_BYTES
                + segment_count * std::mem::size_of::<&str>()
                + fragment_bound * std::mem::size_of::<InlineRunFragment>()
                + segment_count * std::mem::size_of::<InlineBreakSegment>()
                + std::mem::size_of::<BuiltinInlineLineWidth>()
                + std::mem::size_of::<BuiltinInlineWidthProfiles>();
            assert!(stats.logical_temporary_bytes() <= temporary_byte_bound);
            assert_eq!(layout.line_count, 2);
        }
    }
}

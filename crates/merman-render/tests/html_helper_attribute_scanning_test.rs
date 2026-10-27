use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman_render::text::{
    TextMeasurer, TextMetrics, TextStyle, WrapMode, measure_html_with_inline_styles,
};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct RecordingMeasurer {
    calls: Mutex<Vec<(String, TextStyle)>>,
}

fn literal_metrics(text: &str, style: &TextStyle) -> TextMetrics {
    let weight = if matches!(style.font_weight.as_deref(), Some("700" | "bold")) {
        2.0
    } else {
        1.0
    };
    let slant = if style.font_style.as_deref() == Some("italic") {
        1.5
    } else {
        1.0
    };
    let family = if style.font_family.as_deref() == Some("monospace") {
        1.25
    } else {
        1.0
    };
    let line_count = text.split('\n').count();
    TextMetrics {
        width: text
            .split('\n')
            .map(|line| line.chars().count() as f64)
            .fold(0.0, f64::max)
            * style.font_size
            / 2.0
            * weight
            * slant
            * family,
        height: line_count as f64 * style.font_size * 1.5,
        line_count,
    }
}

impl TextMeasurer for RecordingMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.calls
            .lock()
            .unwrap()
            .push((text.to_owned(), style.clone()));
        literal_metrics(text, style)
    }
}

fn assert_plain_html(html: &str, visible: &str) {
    let style = TextStyle::default();
    let expected = literal_metrics(visible, &style);
    for mode in [
        WrapMode::HtmlLike,
        WrapMode::SvgLike,
        WrapMode::SvgLikeSingleRun,
    ] {
        let measurer = RecordingMeasurer::default();
        let actual = measure_html_with_inline_styles(&measurer, html, &style, None, mode);
        let calls = measurer.calls.lock().unwrap();
        assert_eq!(
            calls.first().map(|(text, _)| text.as_str()),
            Some(visible),
            "visible payload for {html:?}, {mode:?}: {calls:?}",
        );
        assert_eq!(actual.width, expected.width, "{html:?}, {mode:?}");
        assert_eq!(actual.height, expected.height, "{html:?}, {mode:?}");
        assert_eq!(actual.line_count, expected.line_count, "{html:?}, {mode:?}");
        assert!(
            calls.iter().all(|(_, face)| {
                face.font_weight.is_none()
                    && face.font_style.is_none()
                    && face.font_family.is_none()
            }),
            "attribute/comment/entity text must not introduce a face: {html:?}, {calls:?}",
        );
    }
}

#[test]
fn double_quoted_attribute_greater_than_is_not_visible_text() {
    assert_plain_html(r#"<span title="a > b">X</span>"#, "X");
}

#[test]
fn single_quoted_attribute_greater_than_is_not_visible_text() {
    assert_plain_html("<span title='a > b'>X</span>", "X");
}

#[test]
fn attribute_quotes_allow_whitespace_and_the_other_quote_character() {
    for html in [
        r#"<span title = "a > b" data-note = 'c > d'>X</span>"#,
        r#"<span title="a ' > b">X</span>"#,
        r#"<span title='a " > b'>X</span>"#,
        r#"<span title="a > b"data-note='c > d'>X</span>"#,
        "<SPAN\ttitle=\n'a > b'\r\n data-note=\"c > d\">X</SPAN>",
    ] {
        assert_plain_html(html, "X");
    }
}

#[test]
fn unquoted_greater_than_ends_the_tag() {
    for html in [
        "<span title=a>b>X</span>",
        r#"<span data-note=="a>b>X</span>"#,
    ] {
        assert_plain_html(html, "b>X");
    }
}

#[test]
fn quotes_inside_unquoted_values_or_attribute_names_do_not_open_a_string() {
    for html in [
        r#"<span title=a"b>X</span>"#,
        "<span title=a'b>X</span>",
        r#"<span data-note"flag>X</span>"#,
        "<span data-note'flag>X</span>",
    ] {
        assert_plain_html(html, "X");
    }
}

#[test]
fn backslash_does_not_escape_an_html_attribute_quote() {
    assert_plain_html(r#"<span title="a\">X</span>"#, "X");
    assert_plain_html(r"<span title='a\'>X</span>", "X");
}

#[test]
fn markup_inside_a_quoted_attribute_cannot_add_text_faces_or_rows() {
    assert_plain_html(
        r#"A<span title="<br><strong>hidden</strong>" data-note='</span>>'>X</span>B"#,
        "AXB",
    );
}

#[test]
fn comments_keep_greater_than_quotes_and_markup_out_of_measurement() {
    for html in [
        "A<!-- hidden > text -->B",
        "A<!-- ' > <br><strong>hidden</strong> -->B",
        "A<!-- \" > &lt;em&gt;hidden&lt;/em&gt; -->B",
        "A<!-->B",
        "A<!--->B",
        "A<!-- hidden --!>B",
    ] {
        assert_plain_html(html, "AB");
    }
}

#[test]
fn unterminated_comments_hide_the_remaining_input() {
    assert_plain_html("A<!-- hidden > <strong>hidden", "A");
}

#[test]
fn bogus_comments_end_at_greater_than_even_after_a_quote() {
    for html in [
        "A<!note title=' >B",
        "A<?note title=\" >B",
        "A</3>B",
        "A</ >B",
    ] {
        assert_plain_html(html, "AB");
    }
}

#[test]
fn entity_decoding_does_not_change_attribute_or_markup_boundaries() {
    assert_plain_html(
        r#"<span title="a &quot; > &gt; b">&lt;strong&gt;X&lt;/strong&gt;&amp;gt;&#62;&#x3e;&nbsp;</span>"#,
        "<strong>X</strong>&gt;>>\u{00a0}",
    );
    assert_plain_html(
        "<span title='a &#39; > b'>&amp;lt;b&amp;gt;X</span>",
        "&lt;b&gt;X",
    );
}

#[test]
fn non_tag_less_than_characters_remain_visible() {
    for visible in ["A < B > C", "A <3 > B", "X <", "X </"] {
        assert_plain_html(visible, visible);
    }
}

#[test]
fn incomplete_tags_do_not_leak_unfinished_attributes() {
    for html in [
        r#"A<span title="hidden > text"#,
        "A<span title='hidden > text",
        "A<span title=hidden",
        r#"A<strong title="hidden > text"#,
        "A<br title='hidden > text",
    ] {
        assert_plain_html(html, "A");
    }
}

#[test]
fn quoted_attributes_preserve_nested_inline_faces() {
    let measurer = RecordingMeasurer::default();
    let actual = measure_html_with_inline_styles(
        &measurer,
        r#"A<strong title="a > b">B<em title='c > d'>C</em></strong><code title="e > f">D</code>E"#,
        &TextStyle::default(),
        None,
        WrapMode::HtmlLike,
    );
    let calls = measurer.calls.lock().unwrap();
    assert_eq!(calls[0].0, "ABCDE", "{calls:?}");
    for (text, bold, italic, code) in [
        ("A", false, false, false),
        ("B", true, false, false),
        ("C", true, true, false),
        ("D", false, false, true),
        ("E", false, false, false),
    ] {
        assert!(
            calls.iter().any(|(payload, face)| {
                payload == text
                    && (face.font_weight.as_deref() == Some("700")) == bold
                    && (face.font_style.as_deref() == Some("italic")) == italic
                    && (face.font_family.as_deref() == Some("monospace")) == code
            }),
            "missing visible face for {text}: {calls:?}",
        );
    }
    assert_eq!(actual.width, 66.0);
    assert_eq!(actual.height, 24.0);
    assert_eq!(actual.line_count, 1);
}

#[test]
fn quoted_attributes_on_real_breaks_preserve_only_the_explicit_rows() {
    assert_plain_html(r#"<span title="a > b">A</span><br title='c > d'>B"#, "A\nB");
}

#[test]
fn quoted_attributes_preserve_fontawesome_icon_advance() {
    let measurer = RecordingMeasurer::default();
    let actual = measure_html_with_inline_styles(
        &measurer,
        r#"<i title="a > b" class="fas fa-heart"></i>X"#,
        &TextStyle::default(),
        None,
        WrapMode::HtmlLike,
    );
    let calls = measurer.calls.lock().unwrap();
    assert_eq!(calls[0].0, "X", "{calls:?}");
    assert!(calls.iter().all(|(text, face)| {
        text == "X" && face.font_weight.is_none() && face.font_style.is_none()
    }));
    assert_eq!(actual.width, 28.0);
    assert_eq!(actual.height, 24.0);
    assert_eq!(actual.line_count, 1);
}

#[derive(Default)]
struct RecordingHost {
    calls: Mutex<Vec<(TextMeasurementOperation, String)>>,
}

impl HostTextMeasurer for RecordingHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        self.calls
            .lock()
            .unwrap()
            .push((request.operation, request.text.to_owned()));
        let metrics = literal_metrics(request.text, request.style);
        match request.operation {
            TextMeasurementOperation::Wrapped => Ok(Some(HostTextMeasurement::Metrics(metrics))),
            TextMeasurementOperation::CanvasMeasureTextWidth => {
                Ok(Some(HostTextMeasurement::Length(metrics.width)))
            }
            _ => Ok(None),
        }
    }
}

#[test]
fn public_html_helper_routes_only_visible_text_to_the_host() {
    let host = Arc::new(RecordingHost::default());
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.html-helper-attribute-scanning").unwrap(),
        "v1",
    )
    .unwrap();
    let policy =
        TextMeasurementPolicy::host_display(identity, host.clone(), [TextMeasurementPhase::Wrap]);
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(policy)
        .begin_session()
        .unwrap();
    let measurer = session.text_measurer(TextMeasurementPhase::Wrap);
    let actual = measure_html_with_inline_styles(
        &measurer,
        r#"<span title="a > b">A</span><!-- hidden > text --><strong title='c > d'>B</strong>"#,
        &TextStyle::default(),
        Some(100.0),
        WrapMode::HtmlLike,
    );
    let calls = host.calls.lock().unwrap();
    assert_eq!(
        calls.first(),
        Some(&(TextMeasurementOperation::Wrapped, "AB".to_owned())),
        "{calls:?}",
    );
    assert!(calls[1..].iter().all(|(operation, text)| {
        *operation == TextMeasurementOperation::CanvasMeasureTextWidth
            && matches!(text.as_str(), "A" | "B")
    }));
    assert!(calls.iter().any(|(_, text)| text == "A"), "{calls:?}");
    assert!(calls.iter().any(|(_, text)| text == "B"), "{calls:?}");
    assert_eq!(actual.width, 24.0);
    assert_eq!(actual.height, 24.0);
    assert_eq!(actual.line_count, 1);
}

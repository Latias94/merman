use super::super::HostMeasurementResult;
use super::super::test_support::TestMeasurementSession;
use super::super::trusted::{InlineHtmlMeasurementCarrier, TRUSTED_MEASUREMENT};
use super::*;
use crate::text::estimate_text_width_em;

#[test]
fn repeated_measurements_are_aggregated_into_one_bounded_summary() {
    let policy = TextMeasurementPolicy::deterministic();
    let recorder = TextMeasurementRecorder::default();

    for _ in 0..10_000 {
        recorder.record(
            TextMeasurementPhase::Layout,
            TextMeasurementOperation::Measure,
            TextMeasurementRouteOutcome::Profile,
        );
    }

    let report = recorder.report(&policy);
    assert_eq!(report.entries().len(), 1);
    assert_eq!(report.entries()[0].count(), 10_000);
    assert_eq!(
        report.entries()[0].provenance().operation,
        TextMeasurementOperation::Measure
    );
    assert_eq!(
        report.entries()[0].provenance().phase,
        TextMeasurementPhase::Layout
    );
}

fn identity(profile: &str, version: &str, decorators: &[&str]) -> TextMeasurementProfileIdentity {
    TextMeasurementProfileIdentity::new(MeasurementProfileId::new(profile).unwrap(), version)
        .unwrap()
        .with_decorators(decorators.iter().copied())
        .unwrap()
}

struct DecliningHost;

impl HostTextMeasurer for DecliningHost {
    fn measure(&self, _request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        Ok(None)
    }
}

struct ForgedCarrierProfile;

impl TextMeasurer for ForgedCarrierProfile {
    #[allow(private_interfaces)]
    fn builtin_operation_carrier(
        &self,
        _: TrustedMeasurementQuery,
        operation: TextMeasurementOperation,
    ) -> Option<BuiltinTextMeasurementOperationCarrier> {
        Some(BuiltinTextMeasurementOperationCarrier::new(
            BuiltinTextMeasurementProfile::Deterministic,
            TextMeasurementPhase::SvgBBox,
            operation,
        ))
    }

    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        DeterministicTextMeasurer::default().measure(text, style)
    }
}

#[test]
fn private_operation_carriers_only_qualify_builtin_profile_routes() {
    assert!(
        BuiltinTextMeasurementOperationCarrier::new(
            BuiltinTextMeasurementProfile::Deterministic,
            TextMeasurementPhase::SvgBBox,
            TextMeasurementOperation::WrappedWithRawWidth
        )
        .into_inline_html()
        .is_none(),
        "the inline planner requires both the wrapped operation and its Wrap owner phase"
    );

    let builtin_session = TestMeasurementSession::deterministic();
    let builtin = builtin_session.text_measurer(TextMeasurementPhase::Layout);
    let builtin_carrier = InlineHtmlMeasurementCarrier::for_measurer(&builtin);
    assert!(builtin_carrier.is_builtin());
    let sequence_carrier = builtin
        .builtin_operation_carrier(TextMeasurementOperation::MermaidCalculateTextDimensions)
        .expect("sequence measurement route is built-in");
    assert_eq!(
        sequence_carrier,
        BuiltinTextMeasurementOperationCarrier::new(
            BuiltinTextMeasurementProfile::Deterministic,
            TextMeasurementPhase::SvgBBox,
            TextMeasurementOperation::MermaidCalculateTextDimensions,
        )
    );
    assert!(
        builtin
            .builtin_operation_carrier(TextMeasurementOperation::MermaidCalculateTextDimensions)
            .is_some()
    );

    let custom_profile = TextMeasurementProfile::new(
        deterministic_profile().identity().clone(),
        Arc::new(ForgedCarrierProfile),
    );
    let custom_session =
        TestMeasurementSession::new(TextMeasurementPolicy::uniform(custom_profile));
    let custom = custom_session.text_measurer(TextMeasurementPhase::Layout);
    assert!(!InlineHtmlMeasurementCarrier::for_measurer(&custom).is_builtin());
    assert!(
        custom
            .builtin_operation_carrier(TextMeasurementOperation::MermaidCalculateTextDimensions)
            .is_none()
    );

    let host_policy = TextMeasurementPolicy::host_display(
        identity("test.host", "v1", &[]),
        Arc::new(DecliningHost),
        [TextMeasurementPhase::Wrap, TextMeasurementPhase::SvgBBox],
    );
    let host_session = TestMeasurementSession::new(host_policy);
    let host = host_session.text_measurer(TextMeasurementPhase::Layout);
    assert!(
        !InlineHtmlMeasurementCarrier::for_measurer(&host).is_builtin(),
        "host routes remain opaque even when their fallback is deterministic"
    );
    assert!(
        host.builtin_operation_carrier(TextMeasurementOperation::MermaidCalculateTextDimensions)
            .is_none()
    );
}

#[test]
fn builtin_inline_stream_preserves_decoded_candidates_and_backend_order() {
    fn assert_cases(
        backend: &dyn TextMeasurer,
        carrier: InlineHtmlMeasurementCarrier,
        style: &TextStyle,
        cases: &[(&str, &[&str])],
    ) {
        for (text, chunks) in cases {
            assert_eq!(chunks.concat(), *text);
            let expected = backend.measure_html_line_width_px(text, style);
            let mut streamed = carrier
                .begin_inline_html_width(style)
                .expect("built-in carrier starts a streaming width");
            for chunk in *chunks {
                streamed.push_text(chunk);
            }
            assert_eq!(
                streamed.width_px().to_bits(),
                expected.to_bits(),
                "text={text:?}, chunks={chunks:?}"
            );
        }
    }

    let cases: &[(&str, &[&str])] = &[
        ("AVATAR office", &["A", "VAT", "AR ", "office"]),
        (
            "alpha<br   />omega",
            &["alpha<", "b", "r ", "  /", ">", "omega"],
        ),
        ("<b<br/>tail", &["<b<", "br", "/>tail"]),
        ("wide<br / >literal", &["wide<br ", "/ ", ">literal"]),
        // ECMAScript `\s` also includes form feed. The headless built-in intentionally keeps
        // that spelling literal until browser-grade HTML parsing is available.
        (
            "wide<br\u{000C}/>literal",
            &["wide<br", "\u{000C}", "/>literal"],
        ),
        (
            "wide\n                         ",
            &["wide\n", "             ", "            "],
        ),
        (" \n  ", &[" ", "\n", "  "]),
        ("i\n\u{00a0}", &["i\n", "\u{00a0}"]),
        ("A\u{301}👩‍💻مرحبا世界", &["A\u{301}", "👩‍💻", "مرحبا", "世界"]),
        ("👩‍🔬", &["👩", "\u{200d}", "🔬"]),
        ("👨‍👩‍👧‍👦", &["👨‍", "👩", "\u{200d}👧‍", "👦"]),
        ("👍🏽", &["👍", "🏽"]),
        ("🇨🇳", &["🇨", "🇳"]),
        ("1️⃣", &["1", "\u{fe0f}", "\u{20e3}"]),
        ("😀\u{fe0e}", &["😀", "\u{fe0e}"]),
        ("k中A+", &["k中", "A+"]),
        (" A  B ", &[" ", "A ", " B", " "]),
        ("A\tB", &["A", "\t", "B"]),
        ("A\nB", &["A", "\n", "B"]),
        ("A<br/>B", &["A<", "br", "/>", "B"]),
    ];
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: Some("700".to_string()),
        font_style: Some("italic".to_string()),
    };

    let deterministic_session = TestMeasurementSession::deterministic();
    let deterministic_carrier = InlineHtmlMeasurementCarrier::for_measurer(
        &deterministic_session.text_measurer(TextMeasurementPhase::Wrap),
    );
    assert_cases(
        &DeterministicTextMeasurer::default(),
        deterministic_carrier,
        &style,
        cases,
    );
    let long_whitespace = " ".repeat(8_192);
    let long_break = format!("wide<br{long_whitespace}/>tail");
    let expected =
        DeterministicTextMeasurer::default().measure_html_line_width_px(&long_break, &style);
    let mut streamed = deterministic_carrier
        .begin_inline_html_width(&style)
        .expect("deterministic carrier starts a streaming width");
    streamed.push_text("wide<br");
    streamed.push_text(&long_whitespace);
    streamed.push_text("/>tail");
    assert_eq!(streamed.width_px().to_bits(), expected.to_bits());

    let mut unknown_font = style.clone();
    unknown_font.font_family = Some("fixture-private-font".to_string());
    assert_cases(
        &DeterministicTextMeasurer::default(),
        deterministic_carrier,
        &unknown_font,
        cases,
    );

    let combining_tail = format!("A{}", "\u{0301}".repeat(1_024));
    assert_cases(
        &DeterministicTextMeasurer::default(),
        deterministic_carrier,
        &style,
        &[(combining_tail.as_str(), &[combining_tail.as_str()])],
    );
}

#[test]
fn builtin_inline_literal_lt_combining_work_is_linear_and_cached() {
    let style = TextStyle {
        font_size: 16.0,
        ..TextStyle::default()
    };
    let carrier =
        InlineHtmlMeasurementCarrier::builtin(BuiltinTextMeasurementProfile::Deterministic);

    for combining_scalars in [1_024, 2_048, 4_096] {
        let text = format!("<{}", "\u{0301}".repeat(combining_scalars));
        let expected =
            DeterministicTextMeasurer::default().measure_html_line_width_px(&text, &style);
        let mut streamed = carrier
            .begin_inline_html_width(&style)
            .expect("built-in carrier starts a streaming width");

        streamed.push_text(&text);
        assert_eq!(
            streamed.grapheme_input_byte_charge(),
            0,
            "appending input must not rescan a growing pending grapheme"
        );
        assert_eq!(streamed.width_px().to_bits(), expected.to_bits());
        assert_eq!(
            streamed.grapheme_input_byte_charge(),
            text.len() * 2,
            "the owning checkpoint admits two linear input-byte charges"
        );

        let input_byte_charge = streamed.grapheme_input_byte_charge();
        assert_eq!(streamed.width_px().to_bits(), expected.to_bits());
        assert_eq!(
            streamed.grapheme_input_byte_charge(),
            input_byte_charge,
            "a repeated checkpoint without new input must use the cached width"
        );
    }
}

#[test]
fn builtin_inline_decoded_literals_have_linear_width_work() {
    let style = TextStyle::default();
    let carrier =
        InlineHtmlMeasurementCarrier::builtin(BuiltinTextMeasurementProfile::Deterministic);
    for scalar_count in [1_024, 2_048, 4_096] {
        let text = "<".repeat(scalar_count);
        let expected =
            DeterministicTextMeasurer::default().measure_html_line_width_px(&text, &style);
        let mut streamed = carrier.begin_inline_html_width(&style).unwrap();
        streamed.push_text(&text);
        assert_eq!(streamed.width_px().to_bits(), expected.to_bits());
        assert_eq!(streamed.grapheme_input_byte_charge(), text.len() * 2);
        assert!(streamed.retained_grapheme_bytes() <= 1);
    }
}

#[test]
fn builtin_inline_decoded_width_work_survives_checkpoint_rollback() {
    let style = TextStyle::default();
    let carrier =
        InlineHtmlMeasurementCarrier::builtin(BuiltinTextMeasurementProfile::Deterministic);
    let mut streamed = carrier.begin_inline_html_width(&style).unwrap();
    streamed.push_text("A");
    let baseline_width = streamed.width_px();
    let checkpoint = streamed.clone();
    let mut candidate = checkpoint.clone();
    candidate.push_text("<not-a-break");
    candidate.width_px();
    let charged = candidate.grapheme_input_byte_charge();
    assert!(charged > 2);
    assert_eq!(streamed.grapheme_input_byte_charge(), charged);
    streamed = checkpoint;
    assert_eq!(streamed.width_px().to_bits(), baseline_width.to_bits());
    assert_eq!(streamed.grapheme_input_byte_charge(), charged);
}

#[test]
fn builtin_inline_decoded_br_whitespace_keeps_a_bounded_frontier() {
    let style = TextStyle::default();
    let carrier =
        InlineHtmlMeasurementCarrier::builtin(BuiltinTextMeasurementProfile::Deterministic);
    for whitespace_repetitions in [256, 512, 1_024] {
        let text = format!("<br{}", " \t\r\n".repeat(whitespace_repetitions));
        let expected =
            DeterministicTextMeasurer::default().measure_html_line_width_px(&text, &style);
        let mut streamed = carrier.begin_inline_html_width(&style).unwrap();
        streamed.push_text(&text);
        assert_eq!(streamed.width_px().to_bits(), expected.to_bits());
        assert!(streamed.retained_grapheme_bytes() <= 2);
        assert_eq!(streamed.grapheme_input_byte_charge(), text.len() * 2);
    }
}

#[test]
fn builtin_svg_computed_length_stream_is_sequence_aware_and_reversible() {
    let backend = DeterministicTextMeasurer::default();
    let style = TextStyle {
        font_size: 16.0,
        ..TextStyle::default()
    };
    let cases: &[(&str, &[&str])] = &[
        ("👩‍🔬", &["👩", "\u{200d}", "🔬"]),
        ("👨‍👩‍👧‍👦", &["👨", "\u{200d}👩‍", "👧", "\u{200d}👦"]),
        ("👍🏽", &["👍", "🏽"]),
        ("🇨🇳", &["🇨", "🇳"]),
        ("1️⃣", &["1", "\u{fe0f}", "\u{20e3}"]),
        ("😀\u{fe0e}", &["😀", "\u{fe0e}"]),
        ("k中A+", &["k中", "A+"]),
    ];

    for (text, chunks) in cases {
        let mut streamed = BuiltinSvgComputedLength::deterministic(&style);
        let mut prefix = String::new();
        for chunk in *chunks {
            prefix.push_str(chunk);
            streamed.push_text(chunk);
            let expected = backend.measure_svg_text_computed_length_px(&prefix, &style);
            assert_eq!(
                streamed.width_px().to_bits(),
                expected.to_bits(),
                "text={text:?}, prefix={prefix:?}"
            );
        }

        let checkpoint = streamed.clone();
        let expected = backend.measure_svg_text_computed_length_px(text, &style);
        assert_eq!(checkpoint.width_px().to_bits(), expected.to_bits());
        streamed.push_text("A");
        assert_ne!(
            streamed.width_px().to_bits(),
            checkpoint.width_px().to_bits()
        );
        streamed = checkpoint;
        assert_eq!(streamed.width_px().to_bits(), expected.to_bits());
        streamed.reset();
        assert_eq!(streamed.width_px(), 0.0);
    }
}

#[test]
fn builtin_html_line_stream_preserves_decoded_unicode_width_and_rollback() {
    let style = TextStyle {
        font_size: 16.0,
        ..TextStyle::default()
    };
    let cases: &[(&str, &[&str])] = &[
        ("AVATAR office", &["A", "VAT", "AR ", "office"]),
        ("A\u{301}مرحبا世界", &["A", "\u{301}", "مرحبا", "世", "界"]),
        ("👩‍🔬", &["👩", "\u{200d}", "🔬"]),
        ("👨‍👩‍👧‍👦", &["👨", "\u{200d}👩‍", "👧", "\u{200d}👦"]),
        ("👍🏽", &["👍", "🏽"]),
        ("🇨🇳", &["🇨", "🇳"]),
        ("1️⃣", &["1", "\u{fe0f}", "\u{20e3}"]),
        ("😀\u{fe0e}", &["😀", "\u{fe0e}"]),
        ("alphabeta\u{200b}", &["alpha", "beta", "\u{200b}"]),
        (
            " a  b\t\u{00a0}\u{2002}",
            &[" a", "  b", "\t", "\u{00a0}", "\u{2002}"],
        ),
    ];

    for char_width_factor in [0.0, 0.6] {
        let backend = DeterministicTextMeasurer {
            char_width_factor,
            ..DeterministicTextMeasurer::default()
        };
        for (text, chunks) in cases {
            assert_eq!(chunks.concat(), *text);
            let mut streamed = BuiltinHtmlLineWidth::deterministic(&style, char_width_factor);
            let mut prefix = String::new();
            for chunk in *chunks {
                prefix.push_str(chunk);
                streamed.push_text(chunk);
                let expected = backend
                    .measure_wrapped(&prefix, &style, None, WrapMode::HtmlLike)
                    .width;
                assert_eq!(
                    streamed.width_px().to_bits(),
                    expected.to_bits(),
                    "factor={char_width_factor}, text={text:?}, prefix={prefix:?}"
                );
            }

            let checkpoint = streamed.clone();
            streamed.push_text("A");
            assert!(streamed.width_px() > checkpoint.width_px());
            streamed = checkpoint;
            let expected = backend
                .measure_wrapped(text, &style, None, WrapMode::HtmlLike)
                .width;
            assert_eq!(streamed.width_px().to_bits(), expected.to_bits());
            streamed.reset();
            assert_eq!(streamed.width_px(), 0.0);
            streamed.push_text("A");
            assert_eq!(
                streamed.width_px().to_bits(),
                backend
                    .measure_wrapped("A", &style, None, WrapMode::HtmlLike)
                    .width
                    .to_bits()
            );
        }
    }

    // HTML has already been decoded before a line reaches this accumulator. A label such
    // as `&lt;br&gt;` must keep its literal spelling rather than introduce another line.
    let mut literal = BuiltinHtmlLineWidth::deterministic(&style, 0.0);
    literal.push_text("a<br>b");
    assert_eq!(
        literal.width_px().to_bits(),
        (estimate_text_width_em("a<br>b") * style.font_size).to_bits()
    );
    assert_ne!(
        literal.width_px().to_bits(),
        DeterministicTextMeasurer::default()
            .measure_wrapped("a<br>b", &style, None, WrapMode::HtmlLike)
            .width
            .to_bits()
    );
}

#[test]
fn builtin_html_line_stream_uses_wrap_canvas_route_and_keeps_opaque_routes_observable() {
    let style = TextStyle::default();
    for (phase, operation) in [
        (
            TextMeasurementPhase::Layout,
            TextMeasurementOperation::CanvasMeasureTextWidth,
        ),
        (
            TextMeasurementPhase::Wrap,
            TextMeasurementOperation::Wrapped,
        ),
        (
            TextMeasurementPhase::Wrap,
            TextMeasurementOperation::ComputedLength,
        ),
        (
            TextMeasurementPhase::Wrap,
            TextMeasurementOperation::WrappedWithRawWidth,
        ),
    ] {
        assert!(
            BuiltinTextMeasurementOperationCarrier::new(
                BuiltinTextMeasurementProfile::Deterministic,
                phase,
                operation
            )
            .into_html_line_width(&style)
            .is_none()
        );
    }

    let builtin_session = TestMeasurementSession::deterministic();
    for default_phase in TextMeasurementPhase::ALL {
        assert!(
            builtin_session
                .text_measurer(default_phase)
                .begin_html_line_width(TRUSTED_MEASUREMENT, &style)
                .is_some(),
            "decoded HTML candidates always belong to the Wrap phase"
        );
    }

    // Even a deterministic backend remains opaque when admitted as an ordinary custom
    // profile; concrete operation authority comes from the route, not its backend type.
    let custom = TextMeasurementProfile::new(
        identity("test.custom-deterministic", "v1", &[]),
        Arc::new(DeterministicTextMeasurer::default()),
    );
    let custom_wrap_session = TestMeasurementSession::new(
        TextMeasurementPolicy::deterministic()
            .with_profile_for_phase(TextMeasurementPhase::Wrap, custom.clone()),
    );
    let custom_wrap = custom_wrap_session.text_measurer(TextMeasurementPhase::ComputedLength);
    assert!(
        custom_wrap
            .begin_html_line_width(TRUSTED_MEASUREMENT, &style)
            .is_none()
    );
    assert!(
        custom_wrap
            .begin_svg_text_computed_length(TRUSTED_MEASUREMENT, &style)
            .is_some()
    );

    let custom_layout_session = TestMeasurementSession::new(
        TextMeasurementPolicy::deterministic()
            .with_profile_for_phase(TextMeasurementPhase::Layout, custom.clone()),
    );
    assert!(
        custom_layout_session
            .text_measurer(TextMeasurementPhase::Layout)
            .begin_html_line_width(TRUSTED_MEASUREMENT, &style)
            .is_some(),
        "the ordinary Canvas Layout route does not own decoded HTML candidates"
    );

    let custom_computed_session = TestMeasurementSession::new(
        TextMeasurementPolicy::deterministic()
            .with_profile_for_phase(TextMeasurementPhase::ComputedLength, custom),
    );
    let custom_computed = custom_computed_session.text_measurer(TextMeasurementPhase::Layout);
    assert!(
        custom_computed
            .begin_html_line_width(TRUSTED_MEASUREMENT, &style)
            .is_some()
    );
    assert!(
        custom_computed
            .begin_svg_text_computed_length(TRUSTED_MEASUREMENT, &style)
            .is_none()
    );

    let host_session = TestMeasurementSession::new(TextMeasurementPolicy::host_display(
        identity("test.host", "v1", &[]),
        Arc::new(DecliningHost),
        [TextMeasurementPhase::Wrap],
    ));
    assert!(
        host_session
            .text_measurer(TextMeasurementPhase::ComputedLength)
            .begin_html_line_width(TRUSTED_MEASUREMENT, &style)
            .is_none(),
        "a built-in fallback cannot bypass host measurement attempts"
    );
    assert!(host_session.text_measurement_report().entries().is_empty());

    let callback = DeterministicTextMeasurer::default()
        .with_width_callback(|text, _style| text.chars().count() as f64);
    assert!(
        callback
            .begin_html_line_width(TRUSTED_MEASUREMENT, &style)
            .is_none()
    );
    assert!(
        ForgedCarrierProfile
            .begin_html_line_width(TRUSTED_MEASUREMENT, &style)
            .is_none()
    );
}

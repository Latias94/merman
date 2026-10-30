//! A downstream consumer of the published 0.9.2 text-measurement API.
//!
//! Keep the exhaustive match and array type fixed throughout 0.9.x. This integration test
//! compiles against public exports, so new variants or a changed array length fail compilation.

use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfileIdentity, TextMeasurementSource,
};
use merman_render::text::{TextMeasurer, TextMetrics, TextStyle, WrapMode};
use std::sync::Arc;

const PUBLISHED_OPERATIONS: [TextMeasurementOperation; 22] = TextMeasurementOperation::ALL;

fn published_operation_code(operation: TextMeasurementOperation) -> i32 {
    match operation {
        TextMeasurementOperation::Measure => 0,
        TextMeasurementOperation::ComputedLength => 1,
        TextMeasurementOperation::BBoxX => 2,
        TextMeasurementOperation::BBoxXWithAsciiOverhang => 3,
        TextMeasurementOperation::TitleBBoxX => 4,
        TextMeasurementOperation::SimpleBBoxWidth => 5,
        TextMeasurementOperation::RawBBoxWidth => 6,
        TextMeasurementOperation::TspanBBoxWidth => 7,
        TextMeasurementOperation::TspanBBoxHeight => 8,
        TextMeasurementOperation::WrapProbeBBoxWidth => 9,
        TextMeasurementOperation::SimpleBBoxHeight => 10,
        TextMeasurementOperation::Wrapped => 11,
        TextMeasurementOperation::WrappedWithRawWidth => 12,
        TextMeasurementOperation::BoundingClientRectWidth => 13,
        TextMeasurementOperation::CreateTextBBoxYOffset => 14,
        TextMeasurementOperation::MermaidCalculateTextDimensions => 15,
        TextMeasurementOperation::CanvasMeasureTextWidth => 16,
        TextMeasurementOperation::CreateTextMiddleBBoxYOffset => 17,
        TextMeasurementOperation::RawBBoxHeight => 18,
        TextMeasurementOperation::RawBBoxYOffset => 19,
        TextMeasurementOperation::RawMiddleBBoxYOffset => 20,
        TextMeasurementOperation::RawHangingBBoxYOffset => 21,
    }
}

#[test]
fn published_operations_remain_exhaustive_and_keep_their_codes() {
    for (expected, operation) in (0_i32..22).zip(PUBLISHED_OPERATIONS) {
        assert_eq!(published_operation_code(operation), expected);
        assert_eq!(operation.external_code(), expected);
        assert_eq!(
            TextMeasurementOperation::from_external_code(expected),
            Some(operation)
        );
    }
    assert_eq!(TextMeasurementOperation::from_external_code(22), None);
    assert_eq!(
        TextMeasurementOperation::from_external_name("html-line-height"),
        None
    );
}

struct PublishedHost;

impl HostTextMeasurer for PublishedHost {
    // Implement only the callback required in 0.9.2; the session uses its default control hook.
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        if request.operation != TextMeasurementOperation::Wrapped {
            return Ok(None);
        }

        assert_eq!(request.phase, TextMeasurementPhase::Wrap);
        assert_eq!(request.text, "legacy label");
        assert_eq!(request.style.font_size, 16.0);
        assert_eq!(request.style.font_family.as_deref(), Some("sans-serif"));
        assert_eq!(request.max_width, None);
        assert_eq!(request.wrap_mode, WrapMode::HtmlLike);

        Ok(Some(HostTextMeasurement::Metrics(TextMetrics {
            width: 73.125,
            height: 31.25,
            line_count: 1,
        })))
    }
}

#[test]
fn published_host_remains_object_safe_and_preserves_routed_measurements() {
    let host: Arc<dyn HostTextMeasurer> = Arc::new(PublishedHost);
    let style = TextStyle {
        font_family: Some("sans-serif".to_owned()),
        font_size: 16.0,
        font_weight: None,
        font_style: None,
    };
    let request = HostTextMeasurementRequest {
        operation: TextMeasurementOperation::Wrapped,
        phase: TextMeasurementPhase::Wrap,
        text: "legacy label",
        style: &style,
        max_width: None,
        wrap_mode: WrapMode::HtmlLike,
    };
    let measured = host
        .measure(request)
        .expect("published callback succeeds")
        .expect("published host handles wrapped HTML");
    let HostTextMeasurement::Metrics(metrics) = measured else {
        panic!("wrapped HTML must return metrics");
    };
    assert_eq!(
        (metrics.width, metrics.height, metrics.line_count),
        (73.125, 31.25, 1)
    );
    assert!(
        host.measure(HostTextMeasurementRequest {
            operation: TextMeasurementOperation::ComputedLength,
            phase: TextMeasurementPhase::ComputedLength,
            ..request
        })
        .expect("unsupported operation is declined without an error")
        .is_none()
    );

    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.published-09-host").expect("valid profile ID"),
        "1",
    )
    .expect("valid profile identity");
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::host_display(
            identity.clone(),
            host,
            [TextMeasurementPhase::Wrap],
        ))
        .begin_session()
        .expect("begin measurement session");
    let routed = session
        .text_measurer(TextMeasurementPhase::Layout)
        .measure_wrapped("legacy label", &style, None, WrapMode::HtmlLike);
    assert_eq!(
        (routed.width, routed.height, routed.line_count),
        (73.125, 31.25, 1)
    );

    let report = session.text_measurement_report();
    assert_eq!(report.entries().len(), 1);
    let entry = &report.entries()[0];
    assert_eq!(entry.count(), 1);
    let provenance = entry.provenance();
    assert_eq!(provenance.phase, TextMeasurementPhase::Wrap);
    assert_eq!(provenance.operation, TextMeasurementOperation::Wrapped);
    assert_eq!(provenance.source, TextMeasurementSource::Host);
    assert_eq!(provenance.identity, identity);
    assert_eq!(provenance.fallback_reason, None);
}

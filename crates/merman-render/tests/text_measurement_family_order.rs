#![cfg(any(feature = "diagram-ishikawa", feature = "diagram-xychart"))]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::text::WrapMode;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct Call {
    phase: TextMeasurementPhase,
    operation: TextMeasurementOperation,
    mode: WrapMode,
    max_width: Option<f64>,
    text: String,
}

struct RecordingHost {
    missing_middle_offset: Option<&'static str>,
    calls: Mutex<Vec<Call>>,
}

impl HostTextMeasurer for RecordingHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        use HostTextMeasurement::{HorizontalExtents, Length};
        use TextMeasurementOperation as Op;

        self.calls.lock().unwrap().push(Call {
            phase: request.phase,
            operation: request.operation,
            mode: request.wrap_mode,
            max_width: request.max_width,
            text: request.text.to_owned(),
        });
        Ok(match request.operation {
            Op::ComputedLength | Op::RawBBoxWidth | Op::BoundingClientRectWidth => {
                Some(Length(16.0))
            }
            Op::BBoxXWithAsciiOverhang => Some(HorizontalExtents {
                left: 8.25,
                right: 8.0,
            }),
            Op::RawMiddleBBoxYOffset if self.missing_middle_offset != Some(request.text) => {
                Some(Length(-8.0))
            }
            Op::RawBBoxHeight => Some(Length(10.0)),
            _ => None,
        })
    }
}

fn layout_trace(source: &str, missing_middle_offset: Option<&'static str>) -> Vec<Call> {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let host = Arc::new(RecordingHost {
        missing_middle_offset,
        calls: Mutex::new(Vec::new()),
    });
    let policy = TextMeasurementPolicy::host_display(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.family-probe-order").unwrap(),
            "1",
        )
        .unwrap(),
        host.clone(),
        TextMeasurementPhase::ALL,
    );
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(policy)
        .begin_session()
        .unwrap();
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
    std::mem::take(&mut *host.calls.lock().unwrap())
}

impl Call {
    fn unwrapped_key(
        &self,
    ) -> (
        TextMeasurementPhase,
        TextMeasurementOperation,
        WrapMode,
        &str,
    ) {
        assert_eq!(self.max_width, None, "{self:?}");
        (self.phase, self.operation, self.mode, &self.text)
    }
}

#[cfg(feature = "diagram-ishikawa")]
#[test]
fn ishikawa_checks_signed_offsets_before_heights_and_short_circuits() {
    use TextMeasurementOperation as Op;
    use TextMeasurementPhase::{ComputedLength, SvgBBox};
    use WrapMode::{HtmlLike, SvgLike};

    let source = "ishikawa-beta\n    Result\n    one<br/>two\n";
    for missing in [None, Some("one")] {
        let trace = layout_trace(source, missing);
        let head: Vec<_> = trace
            .iter()
            .filter(|call| call.text == "Result")
            .map(Call::unwrapped_key)
            .collect();
        assert_eq!(
            head,
            [
                (SvgBBox, Op::RawBBoxWidth, SvgLike, "Result"),
                (SvgBBox, Op::RawMiddleBBoxYOffset, SvgLike, "Result"),
                (SvgBBox, Op::RawBBoxHeight, SvgLike, "Result"),
            ],
            "{trace:?}"
        );

        let body: Vec<_> = trace
            .iter()
            .filter(|call| matches!(call.text.as_str(), "one" | "two"))
            .map(Call::unwrapped_key)
            .collect();
        let mut expected = vec![
            (ComputedLength, Op::ComputedLength, HtmlLike, "one"),
            (SvgBBox, Op::BBoxXWithAsciiOverhang, SvgLike, "one"),
            (ComputedLength, Op::ComputedLength, HtmlLike, "two"),
            (SvgBBox, Op::BBoxXWithAsciiOverhang, SvgLike, "two"),
            (SvgBBox, Op::RawMiddleBBoxYOffset, SvgLike, "one"),
        ];
        if missing.is_none() {
            expected.extend([
                (SvgBBox, Op::RawBBoxHeight, SvgLike, "one"),
                (SvgBBox, Op::RawMiddleBBoxYOffset, SvgLike, "two"),
                (SvgBBox, Op::RawBBoxHeight, SvgLike, "two"),
            ]);
        }
        assert_eq!(body, expected, "{trace:?}");
    }
}

#[cfg(feature = "diagram-xychart")]
#[test]
fn xychart_measures_literal_client_width_before_raw_height() {
    use TextMeasurementOperation as Op;
    use TextMeasurementPhase::SvgBBox;
    use WrapMode::SvgLike;

    let source =
        "xychart\ntitle \"LayoutOnly<br/>Title\"\nx-axis [A, B]\ny-axis 0 --> 2\nbar [1, 2]";
    let trace = layout_trace(source, None);
    let title = trace
        .iter()
        .filter(|call| call.text.starts_with("LayoutOnly"))
        .map(Call::unwrapped_key)
        .collect::<Vec<_>>();
    let (_, _, _, literal) = *title.first().expect("chart title reached measurement");
    assert!(literal.starts_with("LayoutOnly<br"), "{literal:?}");
    assert!(literal.ends_with(">Title"), "{literal:?}");
    assert_eq!(
        title,
        [
            (SvgBBox, Op::BoundingClientRectWidth, SvgLike, literal),
            (SvgBBox, Op::RawBBoxHeight, SvgLike, literal),
        ],
        "{trace:?}"
    );
}

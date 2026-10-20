#![cfg(all(
    feature = "svg",
    feature = "diagram-flowchart",
    feature = "diagram-gantt"
))]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use merman::{
    OperationControl, PrepareSourceRequest, PreparedSource, RenderOutput, RenderTarget, Renderer,
    SvgLayoutOutput, SvgRequest,
};

fn layout(renderer: &Renderer, prepared: &PreparedSource, request: SvgRequest) -> SvgLayoutOutput {
    let RenderOutput::LayoutJson(Some(output)) = renderer
        .render_prepared(
            prepared,
            RenderTarget::LayoutJson(request),
            OperationControl::new(),
        )
        .expect("prepared layout succeeds")
    else {
        panic!("expected the layout output");
    };
    output
}

#[test]
fn the_same_gantt_snapshot_applies_each_requested_container_width() {
    let renderer = Renderer::new();
    let prepared = renderer
        .prepare_source(PrepareSourceRequest::new(
            "gantt\ndateFormat YYYY-MM-DD\nsection Delivery\nTask: task,2026-01-01,2d",
            OperationControl::new(),
        ))
        .unwrap()
        .unwrap();

    for width in [480.0, 960.0, 480.0] {
        let mut request = SvgRequest::default();
        request.layout.container_width = width;
        let output = layout(&renderer, &prepared, request);
        assert_eq!(
            output.layout()["layout"]["GanttDiagram"]["width"].as_f64(),
            Some(width)
        );
    }
}

#[derive(Debug)]
struct FontMetrics {
    width: f64,
    calls: Arc<AtomicUsize>,
}

impl merman::svg::HostTextMeasurer for FontMetrics {
    fn measure(
        &self,
        request: merman::svg::HostTextMeasurementRequest<'_>,
    ) -> merman::svg::HostMeasurementResult {
        use merman::svg::{
            HostTextMeasurement, TextMeasurementOperation, TextMeasurementResultKind, TextMetrics,
        };

        self.calls.fetch_add(1, Ordering::SeqCst);
        let metrics = TextMetrics {
            width: self.width,
            height: 20.0,
            line_count: 1,
        };
        Ok(Some(match request.operation.required_result_kind() {
            TextMeasurementResultKind::Metrics => HostTextMeasurement::Metrics(metrics),
            TextMeasurementResultKind::Length => {
                HostTextMeasurement::Length(match request.operation {
                    TextMeasurementOperation::RawBBoxHeight
                    | TextMeasurementOperation::SimpleBBoxHeight
                    | TextMeasurementOperation::TspanBBoxHeight => metrics.height,
                    TextMeasurementOperation::CreateTextBBoxYOffset
                    | TextMeasurementOperation::CreateTextMiddleBBoxYOffset => 0.0,
                    _ => self.width,
                })
            }
            TextMeasurementResultKind::HorizontalExtents => {
                HostTextMeasurement::HorizontalExtents {
                    left: self.width / 2.0,
                    right: self.width / 2.0,
                }
            }
            TextMeasurementResultKind::WrappedWithRawWidth => {
                HostTextMeasurement::WrappedWithRawWidth {
                    metrics,
                    raw_width: Some(self.width),
                }
            }
        }))
    }
}

#[test]
fn the_same_snapshot_uses_each_outputs_actual_host_font_metrics() {
    let renderer = Renderer::new();
    let prepared = renderer
        .prepare_source(PrepareSourceRequest::new(
            "---\nconfig:\n  layout: dagre\n  htmlLabels: false\n---\nflowchart TD\nA[Retained]",
            OperationControl::new(),
        ))
        .unwrap()
        .unwrap();
    let identity = merman::svg::TextMeasurementProfileIdentity::new(
        merman::svg::MeasurementProfileId::new("test.prepared-redraw-fonts").unwrap(),
        "1",
    )
    .unwrap();
    let mut widths = Vec::new();

    for width in [40.0, 160.0, 40.0] {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut request = SvgRequest::default();
        request.environment = merman::SvgEnvironment::deterministic().with_text_measurement_policy(
            merman::svg::TextMeasurementPolicy::host_display(
                identity.clone(),
                Arc::new(FontMetrics {
                    width,
                    calls: Arc::clone(&calls),
                }),
                merman::svg::TextMeasurementPhase::ALL,
            ),
        );
        let output = layout(&renderer, &prepared, request);
        assert!(calls.load(Ordering::SeqCst) > 0);
        let node = output.layout()["layout"]["FlowchartV2"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == "A")
            .unwrap();
        widths.push(node["width"].as_f64().unwrap());
    }

    assert!(widths[1] > widths[0]);
    assert_eq!(widths[2], widths[0]);
}

use std::sync::{Arc, Mutex};

use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    HostFallbackReason, HostMeasurementResult, HostTextMeasurement, HostTextMeasurementError,
    HostTextMeasurementRequest, HostTextMeasurer, MeasurementProfileId, RenderEnvironment,
    TextMeasurementOperation, TextMeasurementPhase, TextMeasurementPolicy,
    TextMeasurementProfileIdentity, TextMeasurementResultKind, TextMeasurementSource,
};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMetrics, WrapMode};

#[derive(Debug, Clone, Copy)]
enum HostOutcome {
    Success,
    Missing,
    Invalid,
    Error,
}

impl HostOutcome {
    fn name(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Missing => "missing",
            Self::Invalid => "invalid",
            Self::Error => "error",
        }
    }

    fn fallback_reason(self) -> Option<HostFallbackReason> {
        match self {
            Self::Success => None,
            Self::Missing => Some(HostFallbackReason::Missing),
            Self::Invalid => Some(HostFallbackReason::Invalid),
            Self::Error => Some(HostFallbackReason::Error),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct RecordedRequest {
    operation: TextMeasurementOperation,
    phase: TextMeasurementPhase,
    text: String,
    font_size_bits: u64,
    max_width_bits: Option<u64>,
    wrap_mode: WrapMode,
}

struct RecordingFlowchartHost {
    outcome: HostOutcome,
    requests: Mutex<Vec<RecordedRequest>>,
}

impl RecordingFlowchartHost {
    fn new(outcome: HostOutcome) -> Self {
        Self {
            outcome,
            requests: Mutex::new(Vec::new()),
        }
    }

    fn snapshot(&self) -> Vec<RecordedRequest> {
        self.requests.lock().expect("request trace").clone()
    }
}

impl HostTextMeasurer for RecordingFlowchartHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        self.requests
            .lock()
            .expect("request trace")
            .push(RecordedRequest {
                operation: request.operation,
                phase: request.phase,
                text: request.text.to_string(),
                font_size_bits: request.style.font_size.to_bits(),
                max_width_bits: request.max_width.map(f64::to_bits),
                wrap_mode: request.wrap_mode,
            });

        match self.outcome {
            HostOutcome::Missing => Ok(None),
            HostOutcome::Invalid => Ok(Some(HostTextMeasurement::Length(f64::NAN))),
            HostOutcome::Error => Err(HostTextMeasurementError::new("recorded host failure")),
            HostOutcome::Success => Ok(Some(valid_measurement(request))),
        }
    }
}

fn valid_measurement(request: HostTextMeasurementRequest<'_>) -> HostTextMeasurement {
    let raw_width = request
        .text
        .lines()
        .map(|line| line.chars().count() as f64 * 8.0)
        .fold(0.0_f64, f64::max);
    let max_width = request
        .max_width
        .filter(|width| width.is_finite() && *width > 0.0);
    let line_count = max_width
        .map(|width| (raw_width / width).ceil() as usize)
        .unwrap_or(1)
        .max(request.text.lines().count())
        .max(1)
        .min(request.text.len().saturating_add(1));
    let metrics = TextMetrics {
        width: max_width.map_or(raw_width, |width| raw_width.min(width)),
        height: line_count as f64 * 20.0,
        line_count,
    };

    match request.operation.required_result_kind() {
        TextMeasurementResultKind::Metrics => HostTextMeasurement::Metrics(metrics),
        TextMeasurementResultKind::Length => {
            let length = match request.operation {
                TextMeasurementOperation::RawBBoxHeight
                | TextMeasurementOperation::SimpleBBoxHeight
                | TextMeasurementOperation::TspanBBoxHeight => metrics.height,
                TextMeasurementOperation::CreateTextBBoxYOffset
                | TextMeasurementOperation::CreateTextMiddleBBoxYOffset => 0.0,
                _ => raw_width,
            };
            HostTextMeasurement::Length(length)
        }
        TextMeasurementResultKind::HorizontalExtents => HostTextMeasurement::HorizontalExtents {
            left: raw_width / 2.0,
            right: raw_width / 2.0,
        },
        TextMeasurementResultKind::WrappedWithRawWidth => {
            HostTextMeasurement::WrappedWithRawWidth {
                metrics,
                raw_width: Some(raw_width),
            }
        }
    }
}

fn render_flowchart_with_host(outcome: HostOutcome) {
    let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
    wrappingWidth: 96
---
flowchart LR
  subgraph S["service words"]
    A["alpha beta<br/>gamma"]
  end
  A -->|edge label words| B["delta epsilon"]
"#;
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse flowchart")
        .expect("detect flowchart");
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new(format!("test.flowchart-sidecar-{}", outcome.name()))
            .expect("profile id"),
        "1",
    )
    .expect("profile identity");
    let host = Arc::new(RecordingFlowchartHost::new(outcome));
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::host_display(identity, host.clone(), TextMeasurementPhase::ALL),
    );
    let session = environment.begin_session().expect("render session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare flowchart artifact");
    let layout_requests = host.snapshot();
    assert!(
        !layout_requests.is_empty(),
        "layout must exercise the configured host for {}",
        outcome.name()
    );

    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render flowchart SVG");
    let all_requests = host.snapshot();
    let render_requests = &all_requests[layout_requests.len()..];
    assert!(
        !render_requests.is_empty(),
        "the sidecar must not suppress {} host callbacks during SVG emission",
        outcome.name()
    );
    assert!(
        render_requests.iter().any(|request| {
            request.operation == TextMeasurementOperation::ComputedLength
                && (request.text.contains("alpha") || request.text.contains("delta"))
        }),
        "non-Markdown SVG wrapping must preserve the host computed-length trace for {}: {render_requests:#?}",
        outcome.name()
    );

    let (_, _, _, session) = rendered.into_parts();
    let report = session.text_measurement_report();
    assert_eq!(
        report
            .entries()
            .iter()
            .map(|entry| entry.count())
            .sum::<u64>(),
        all_requests.len() as u64,
        "the measurement report must account for every {} callback",
        outcome.name()
    );
    for entry in report.entries() {
        let provenance = entry.provenance();
        match outcome.fallback_reason() {
            None => {
                assert_eq!(provenance.source, TextMeasurementSource::Host);
                assert_eq!(provenance.fallback_reason, None);
            }
            Some(reason) => {
                assert_eq!(provenance.source, TextMeasurementSource::Profile);
                assert_eq!(provenance.fallback_reason, Some(reason));
            }
        }
    }
}

#[test]
fn host_and_fallback_callback_traces_survive_flowchart_prepare_and_render() {
    for outcome in [
        HostOutcome::Success,
        HostOutcome::Missing,
        HostOutcome::Invalid,
        HostOutcome::Error,
    ] {
        render_flowchart_with_host(outcome);
    }
}

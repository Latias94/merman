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
    Mixed,
}

impl HostOutcome {
    fn name(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Missing => "missing",
            Self::Invalid => "invalid",
            Self::Error => "error",
            Self::Mixed => "mixed",
        }
    }

    fn fallback_reason(self, operation: TextMeasurementOperation) -> Option<HostFallbackReason> {
        match self {
            Self::Success => None,
            Self::Missing => Some(HostFallbackReason::Missing),
            Self::Invalid => Some(HostFallbackReason::Invalid),
            Self::Error => Some(HostFallbackReason::Error),
            Self::Mixed if operation == TextMeasurementOperation::ComputedLength => None,
            Self::Mixed => Some(HostFallbackReason::Missing),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct RecordedRequest {
    ordinal: usize,
    operation: TextMeasurementOperation,
    result_kind: TextMeasurementResultKind,
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
        let ordinal = {
            let mut requests = self.requests.lock().expect("request trace");
            let ordinal = requests.len();
            requests.push(RecordedRequest {
                ordinal,
                operation: request.operation,
                result_kind: request.operation.required_result_kind(),
                phase: request.phase,
                text: request.text.to_string(),
                font_size_bits: request.style.font_size.to_bits(),
                max_width_bits: request.max_width.map(f64::to_bits),
                wrap_mode: request.wrap_mode,
            });
            ordinal
        };

        match self.outcome {
            HostOutcome::Missing => Ok(None),
            HostOutcome::Invalid => Ok(Some(HostTextMeasurement::Length(f64::NAN))),
            HostOutcome::Error => Err(HostTextMeasurementError::new("recorded host failure")),
            HostOutcome::Success => Ok(Some(valid_measurement(request, ordinal))),
            HostOutcome::Mixed if request.operation == TextMeasurementOperation::ComputedLength => {
                Ok(Some(valid_measurement(request, ordinal)))
            }
            HostOutcome::Mixed => Ok(None),
        }
    }
}

fn valid_measurement(
    request: HostTextMeasurementRequest<'_>,
    ordinal: usize,
) -> HostTextMeasurement {
    // Make callback order observable without materially changing the fixture geometry. If a
    // cache ever suppresses or reorders host requests, every subsequent result will diverge.
    let state_delta = (ordinal % 7) as f64 / 32.0;
    let raw_width = request
        .text
        .lines()
        .map(|line| line.chars().count() as f64 * 8.0)
        .fold(0.0_f64, f64::max)
        + state_delta;
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
        height: line_count as f64 * 20.0 + state_delta,
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

fn render_flowchart_with_host(
    case_name: &str,
    source: &str,
    expected_text_fragment: &str,
    expected_operation: Option<TextMeasurementOperation>,
    outcome: HostOutcome,
) -> Vec<RecordedRequest> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse flowchart")
        .expect("detect flowchart");
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new(format!(
            "test.flowchart-sidecar-{case_name}-{}",
            outcome.name()
        ))
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
        "{case_name} layout must exercise the configured host for {}",
        outcome.name()
    );

    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render flowchart SVG");
    let all_requests = host.snapshot();
    let render_requests = &all_requests[layout_requests.len()..];
    assert!(
        !render_requests.is_empty(),
        "the {case_name} sidecar must not suppress {} host callbacks during SVG emission",
        outcome.name()
    );
    for (ordinal, request) in all_requests.iter().enumerate() {
        assert_eq!(request.ordinal, ordinal);
        assert_eq!(
            request.result_kind,
            request.operation.required_result_kind()
        );
    }
    assert!(
        render_requests.iter().any(|request| {
            request.text.contains(expected_text_fragment)
                && expected_operation.is_none_or(|operation| request.operation == operation)
        }),
        "{case_name} must preserve the relevant host trace for {}: {render_requests:#?}",
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
        "the measurement report must account for every {case_name} {} callback",
        outcome.name()
    );
    for entry in report.entries() {
        let provenance = entry.provenance();
        match outcome.fallback_reason(provenance.operation) {
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

    all_requests
}

const DAGRE_SOURCE: &str = r#"---
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

#[test]
fn empty_subgraph_node_keeps_mermaid_bbox_measurement_after_svg_wrapping() {
    let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
    wrappingWidth: 96
---
flowchart LR
subgraph Empty["alpha beta gamma delta epsilon zeta eta theta"]
end
"#;

    for outcome in [HostOutcome::Success, HostOutcome::Missing] {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse empty subgraph")
            .expect("detect flowchart");
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new(format!("test.flowchart-empty-subgraph-{}", outcome.name()))
                .expect("profile id"),
            "1",
        )
        .expect("profile identity");
        let host = Arc::new(RecordingFlowchartHost::new(outcome));
        let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
            TextMeasurementPolicy::host_display(identity, host.clone(), TextMeasurementPhase::ALL),
        );
        let session = environment.begin_session().expect("render session");
        let _artifact = family::prepare(parsed, &LayoutOptions::default(), session)
            .expect("prepare empty subgraph");
        let requests = host.snapshot();
        let final_title_request = requests
            .iter()
            .rfind(|request| request.text.contains("alpha"))
            .expect("empty subgraph title measurement");

        assert_eq!(
            final_title_request.operation,
            TextMeasurementOperation::Wrapped,
            "Mermaid wraps the empty-subgraph node with flowchart.wrappingWidth, then sizes the final SVG text through getBBox(): {requests:#?}"
        );
        assert!(
            final_title_request.text.contains('\n'),
            "the configured width must wrap the title before the final bbox measurement: {requests:#?}"
        );
    }
}

#[test]
fn host_and_fallback_callback_traces_survive_flowchart_prepare_and_render() {
    for outcome in [
        HostOutcome::Success,
        HostOutcome::Missing,
        HostOutcome::Invalid,
        HostOutcome::Error,
        HostOutcome::Mixed,
    ] {
        let first_trace = render_flowchart_with_host(
            "dagre",
            DAGRE_SOURCE,
            "alpha",
            Some(TextMeasurementOperation::ComputedLength),
            outcome,
        );
        let second_trace = render_flowchart_with_host(
            "dagre",
            DAGRE_SOURCE,
            "alpha",
            Some(TextMeasurementOperation::ComputedLength),
            outcome,
        );
        assert_eq!(
            second_trace,
            first_trace,
            "the complete {} host callback trace must remain deterministic",
            outcome.name()
        );
    }
}

#[test]
fn host_trace_survives_self_loop_and_swimlane_family_paths() {
    let cases = [
        (
            "dagre-self-loop",
            r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A -->|self loop label words| A
"#,
            "self loop label",
        ),
        (
            "swimlane",
            r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
swimlane-beta LR
A -->|swimlane edge label words| B
"#,
            "swimlane edge label",
        ),
    ];

    for (case_name, source, label) in cases {
        let first_trace =
            render_flowchart_with_host(case_name, source, label, None, HostOutcome::Success);
        let second_trace =
            render_flowchart_with_host(case_name, source, label, None, HostOutcome::Success);
        assert_eq!(second_trace, first_trace, "complete {case_name} trace");
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn host_trace_survives_elk_family_path() {
    let source = r#"---
config:
  layout: elk
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
subgraph Group["ELK group label words"]
  A[ELK node label words]
end
A -->|ELK edge label words| B
"#;
    let first_trace =
        render_flowchart_with_host("elk", source, "ELK node label", None, HostOutcome::Success);
    let second_trace =
        render_flowchart_with_host("elk", source, "ELK node label", None, HostOutcome::Success);
    assert_eq!(second_trace, first_trace, "complete ELK trace");
}

#[cfg(feature = "math")]
#[test]
fn host_trace_survives_svg_math_like_source() {
    let source = r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A["math $$x$$ label words"] --> B
"#;
    let first_trace = render_flowchart_with_host(
        "svg-math",
        source,
        "math $$x$$ label",
        Some(TextMeasurementOperation::ComputedLength),
        HostOutcome::Success,
    );
    let second_trace = render_flowchart_with_host(
        "svg-math",
        source,
        "math $$x$$ label",
        Some(TextMeasurementOperation::ComputedLength),
        HostOutcome::Success,
    );
    assert_eq!(second_trace, first_trace, "complete SVG math-like trace");
}

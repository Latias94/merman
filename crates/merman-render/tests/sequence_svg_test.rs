mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions, ParsedDiagramRender, RenderSemanticModel};
use merman_render::LayoutOptions;
use merman_render::environment::{
    HostFallbackReason, HostMeasurementResult, HostTextMeasurement, HostTextMeasurementError,
    HostTextMeasurementRequest, HostTextMeasurer, MeasurementProfileId, RenderEnvironment,
    TextMeasurementOperation, TextMeasurementPhase, TextMeasurementPolicy,
    TextMeasurementProfileIdentity, TextMeasurementReport, TextMeasurementRoute,
    TextMeasurementSource,
};
use merman_render::family;
use merman_render::model::{LayoutEdge, SequenceDiagramLayout};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{TextMetrics, WrapMode};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordedSequenceMeasurement {
    text: String,
    operation: TextMeasurementOperation,
    phase: TextMeasurementPhase,
    font_family: Option<String>,
    font_size_bits: u64,
    font_weight: Option<String>,
    font_style: Option<String>,
    max_width_bits: Option<u64>,
    wrap_mode: WrapMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecordedSequenceOutcome {
    Host,
    Missing,
    Error,
}

#[derive(Debug, Clone)]
struct RecordedSequenceExchange {
    request: RecordedSequenceMeasurement,
    outcome: RecordedSequenceOutcome,
}

#[derive(Debug, Clone, Copy, Default)]
enum SequenceHostResponse {
    #[default]
    Missing,
    StatefulMetrics,
    Error,
}

#[derive(Default)]
struct RecordingSequenceHost {
    exchanges: Mutex<Vec<RecordedSequenceExchange>>,
    response: SequenceHostResponse,
    response_index: AtomicUsize,
}

impl RecordingSequenceHost {
    fn new(response: SequenceHostResponse) -> Self {
        Self {
            response,
            ..Self::default()
        }
    }

    fn snapshot(&self) -> Vec<RecordedSequenceExchange> {
        self.exchanges
            .lock()
            .expect("Sequence host exchanges lock")
            .clone()
    }
}

impl HostTextMeasurer for RecordingSequenceHost {
    fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        let recorded = RecordedSequenceMeasurement {
            text: request.text.to_string(),
            operation: request.operation,
            phase: request.phase,
            font_family: request.style.font_family.clone(),
            font_size_bits: request.style.font_size.to_bits(),
            font_weight: request.style.font_weight.clone(),
            font_style: request.style.font_style.clone(),
            max_width_bits: request.max_width.map(f64::to_bits),
            wrap_mode: request.wrap_mode,
        };
        let result = match self.response {
            SequenceHostResponse::StatefulMetrics
                if request.operation
                    == TextMeasurementOperation::MermaidCalculateTextDimensions
                    && request.text.starts_with("probe-") =>
            {
                let response_index = self.response_index.fetch_add(1, Ordering::Relaxed);
                Ok(Some(HostTextMeasurement::Metrics(TextMetrics {
                    width: 320.0 + response_index as f64,
                    height: 24.0,
                    line_count: 1,
                })))
            }
            SequenceHostResponse::Error => Err(HostTextMeasurementError::new(
                "recorded Sequence host failure",
            )),
            _ => Ok(None),
        };
        let outcome = match &result {
            Ok(Some(_)) => RecordedSequenceOutcome::Host,
            Ok(None) => RecordedSequenceOutcome::Missing,
            Err(_) => RecordedSequenceOutcome::Error,
        };
        self.exchanges
            .lock()
            .expect("Sequence host exchanges lock")
            .push(RecordedSequenceExchange {
                request: recorded,
                outcome,
            });
        result
    }
}

struct SequenceRenderObservation {
    svg: String,
    report: TextMeasurementReport,
}

struct SequenceHostObservation {
    render: SequenceRenderObservation,
    requests: Vec<RecordedSequenceMeasurement>,
    outcomes: Vec<RecordedSequenceOutcome>,
    routes: [TextMeasurementRoute; 4],
    stateful_response_count: usize,
}

type SequenceMeasurementReportKey = (
    TextMeasurementPhase,
    TextMeasurementOperation,
    TextMeasurementSource,
    TextMeasurementProfileIdentity,
    Option<HostFallbackReason>,
);

fn render_sequence_with_environment(
    source: &str,
    environment: &RenderEnvironment,
) -> SequenceRenderObservation {
    let session = environment.begin_session().expect("begin Sequence session");
    let parsed = parse_sequence_for_render(&Engine::new(), source);
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare Sequence artifact");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Sequence artifact");
    let (svg, _, _, session) = rendered.into_parts();
    SequenceRenderObservation {
        svg,
        report: session.text_measurement_report(),
    }
}

fn render_sequence_with_host_environment(
    source: &str,
    response: SequenceHostResponse,
    profile_id: &str,
    environment: RenderEnvironment,
) -> SequenceHostObservation {
    let host = Arc::new(RecordingSequenceHost::new(response));
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new(profile_id).expect("valid profile id"),
        "1",
    )
    .expect("valid profile identity");
    let policy =
        TextMeasurementPolicy::host_display(identity, host.clone(), TextMeasurementPhase::ALL);
    let routes = TextMeasurementPhase::ALL.map(|phase| policy.route(phase));
    let environment = environment.with_text_measurement_policy(policy);
    let render = render_sequence_with_environment(source, &environment);
    let exchanges = host.snapshot();
    let mut requests = Vec::with_capacity(exchanges.len());
    let mut outcomes = Vec::with_capacity(exchanges.len());
    for exchange in exchanges {
        requests.push(exchange.request);
        outcomes.push(exchange.outcome);
    }
    let observation = SequenceHostObservation {
        render,
        requests,
        outcomes,
        routes,
        stateful_response_count: host.response_index.load(Ordering::Relaxed),
    };
    assert_sequence_host_report_matches_trace(&observation);
    observation
}

fn assert_sequence_host_report_matches_trace(observation: &SequenceHostObservation) {
    assert_eq!(observation.requests.len(), observation.outcomes.len());

    let mut expected: HashMap<SequenceMeasurementReportKey, u64> = HashMap::new();
    for (request, outcome) in observation.requests.iter().zip(&observation.outcomes) {
        let route = observation
            .routes
            .iter()
            .find(|route| route.phase == request.phase)
            .expect("measurement route for recorded Sequence phase");
        assert_eq!(route.primary_source, TextMeasurementSource::Host);
        let (source, identity, fallback_reason) = match outcome {
            RecordedSequenceOutcome::Host => {
                (TextMeasurementSource::Host, route.primary.clone(), None)
            }
            RecordedSequenceOutcome::Missing => (
                TextMeasurementSource::Profile,
                route
                    .fallback
                    .clone()
                    .expect("host measurement route fallback identity"),
                Some(HostFallbackReason::Missing),
            ),
            RecordedSequenceOutcome::Error => (
                TextMeasurementSource::Profile,
                route
                    .fallback
                    .clone()
                    .expect("host measurement route fallback identity"),
                Some(HostFallbackReason::Error),
            ),
        };
        *expected
            .entry((
                request.phase,
                request.operation,
                source,
                identity,
                fallback_reason,
            ))
            .or_insert(0) += 1;
    }

    let mut actual: HashMap<SequenceMeasurementReportKey, u64> = HashMap::new();
    for entry in observation.render.report.entries() {
        let provenance = entry.provenance();
        *actual
            .entry((
                provenance.phase,
                provenance.operation,
                provenance.source,
                provenance.identity.clone(),
                provenance.fallback_reason,
            ))
            .or_insert(0) += entry.count();
    }

    assert_eq!(
        actual, expected,
        "the Sequence measurement report must reconcile every traced phase, operation, source, identity, fallback reason, and count"
    );
}

fn normalized_measurement_counts(
    report: &TextMeasurementReport,
) -> HashMap<(TextMeasurementPhase, TextMeasurementOperation), u64> {
    let mut counts = HashMap::new();
    for entry in report.entries() {
        let provenance = entry.provenance();
        *counts
            .entry((provenance.phase, provenance.operation))
            .or_insert(0) += entry.count();
    }
    counts
}

fn measurement_operation_count(
    report: &TextMeasurementReport,
    operation: TextMeasurementOperation,
) -> u64 {
    report
        .entries()
        .iter()
        .filter(|entry| entry.provenance().operation == operation)
        .map(|entry| entry.count())
        .sum()
}

fn assert_sequence_probe_message_trace(requests: &[RecordedSequenceMeasurement]) {
    let message_requests = requests
        .iter()
        .filter(|request| {
            request.text.starts_with("probe-")
                && request.operation == TextMeasurementOperation::MermaidCalculateTextDimensions
        })
        .collect::<Vec<_>>();
    let expected_probe_pairs = [
        // Actor-spacing scan.
        "probe-loop-message",
        "probe-alt-message",
        "probe-opt-message",
        "probe-rect-message",
        // Layout-time control-block bounds.
        "probe-loop-message",
        "probe-alt-message",
        "probe-opt-message",
        // Main message horizontal and vertical geometry.
        "probe-loop-message",
        "probe-loop-message",
        "probe-alt-message",
        "probe-alt-message",
        "probe-opt-message",
        "probe-opt-message",
        "probe-rect-message",
        "probe-rect-message",
        // SVG control-block width reconstruction.
        "probe-loop-message",
        "probe-alt-message",
        "probe-opt-message",
    ];
    assert_eq!(
        message_requests.len(),
        expected_probe_pairs.len() * 2,
        "host routes must retain every actor-spacing, block-bound, geometry, and SVG reconstruction callback"
    );
    let configured_family = message_requests[1].font_family.as_deref();
    assert_ne!(configured_family, Some("sans-serif"));
    for (pair, expected_text) in message_requests.chunks_exact(2).zip(expected_probe_pairs) {
        assert_eq!(pair[0].text, expected_text);
        assert_eq!(pair[1].text, expected_text);
        assert_eq!(
            pair[0].operation,
            TextMeasurementOperation::MermaidCalculateTextDimensions
        );
        assert_eq!(
            pair[1].operation,
            TextMeasurementOperation::MermaidCalculateTextDimensions
        );
        assert_eq!(pair[0].phase, TextMeasurementPhase::SvgBBox);
        assert_eq!(pair[1].phase, TextMeasurementPhase::SvgBBox);
        assert_eq!(pair[0].font_family.as_deref(), Some("sans-serif"));
        assert_eq!(pair[1].font_family.as_deref(), configured_family);
        assert_eq!(pair[0].font_size_bits, pair[1].font_size_bits);
        assert_eq!(pair[0].font_weight, pair[1].font_weight);
        assert_eq!(pair[0].font_style, pair[1].font_style);
        assert_eq!(pair[0].max_width_bits, None);
        assert_eq!(pair[1].max_width_bits, None);
        assert_eq!(pair[0].wrap_mode, WrapMode::SvgLike);
        assert_eq!(pair[1].wrap_mode, WrapMode::SvgLike);
    }
}

fn assert_sequence_shape_does_not_reuse_ordinary_message_metrics(
    case_name: &str,
    source: &str,
    expected_request_fragment: &str,
    builtin_environment: RenderEnvironment,
    host_environment: RenderEnvironment,
) {
    let builtin = render_sequence_with_environment(source, &builtin_environment);
    let host = render_sequence_with_host_environment(
        source,
        SequenceHostResponse::Missing,
        &format!("test.sequence-sidecar-non-reused-{case_name}"),
        host_environment,
    );

    assert_eq!(
        host.render.svg, builtin.svg,
        "the {case_name} host fallback must preserve built-in Sequence geometry"
    );
    assert_eq!(
        normalized_measurement_counts(&host.render.report),
        normalized_measurement_counts(&builtin.report),
        "the {case_name} path must not reuse ordinary-message sidecar metrics"
    );
    assert!(
        host.requests
            .iter()
            .any(|request| request.text.contains(expected_request_fragment)),
        "the {case_name} fixture must exercise the intended label through the host trace"
    );
    assert_eq!(
        host.render
            .report
            .entries()
            .iter()
            .map(|entry| entry.count())
            .sum::<u64>(),
        host.requests.len() as u64,
        "the {case_name} report must account for every host callback"
    );
}

fn render_prepared_sequence_after_release(
    source: &'static str,
    environment: RenderEnvironment,
    prepared: Sender<()>,
    release: Receiver<()>,
) -> SequenceRenderObservation {
    let session = environment.begin_session().expect("begin Sequence session");
    let parsed = parse_sequence_for_render(&Engine::new(), source);
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare Sequence artifact");

    prepared
        .send(())
        .expect("report prepared Sequence artifact");
    release.recv().expect("release prepared Sequence artifact");

    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Sequence artifact");
    let (svg, _, _, session) = rendered.into_parts();
    SequenceRenderObservation {
        svg,
        report: session.text_measurement_report(),
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn parse_sequence_for_render(engine: &Engine, text: &str) -> ParsedDiagramRender {
    engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
}

fn layout_sequence_from_environment(
    text: &str,
    environment: &RenderEnvironment,
) -> SequenceDiagramLayout {
    let parsed = parse_sequence_for_render(&Engine::new(), text);
    let session = environment.begin_session().unwrap();
    let artifact =
        family::prepare(parsed, &LayoutOptions::default(), session).expect("typed Sequence layout");
    let projection = artifact.layout_json().expect("Sequence layout projection");
    serde_json::from_value(projection["layout"]["SequenceDiagram"].clone())
        .expect("Sequence layout")
}

fn extract_self_closing_tags<'a>(s: &'a str, tag_name: &str) -> Vec<&'a str> {
    let needle = format!("<{tag_name}");
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(pos) = s[i..].find(&needle) {
        let start = i + pos;
        let Some(end_rel) = s[start..].find("/>") else {
            break;
        };
        let end = start + end_rel + 2;
        out.push(&s[start..end]);
        i = end;
    }
    out
}

fn extract_paired_tags<'a>(s: &'a str, tag_name: &str) -> Vec<&'a str> {
    let needle = format!("<{tag_name}");
    let closing = format!("</{tag_name}>");
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(pos) = s[i..].find(&needle) {
        let start = i + pos;
        let Some(end_rel) = s[start..].find(&closing) else {
            break;
        };
        let end = start + end_rel + closing.len();
        out.push(&s[start..end]);
        i = end;
    }
    out
}

fn text_rows_by_class(svg: &str, class_name: &str) -> Vec<String> {
    let document = roxmltree::Document::parse(svg).expect("valid Sequence SVG");
    document
        .descendants()
        .filter(|node| {
            node.is_element()
                && node.tag_name().name() == "text"
                && node.attribute("class").is_some_and(|classes| {
                    classes.split_whitespace().any(|class| class == class_name)
                })
        })
        .map(|node| {
            node.descendants()
                .filter(|descendant| descendant.is_text())
                .filter_map(|descendant| descendant.text())
                .collect::<String>()
        })
        .collect()
}

fn attr_f64(tag: &str, name: &str) -> Option<f64> {
    let needle = format!(r#"{name}=""#);
    let i = tag.find(&needle)? + needle.len();
    let rest = &tag[i..];
    let end = rest.find('"')?;
    rest[..end].parse::<f64>().ok()
}

fn root_view_box_and_max_width(svg: &str) -> ([f64; 4], f64) {
    let document = roxmltree::Document::parse(svg).expect("valid Sequence SVG");
    let root = document.root_element();
    assert_eq!(root.tag_name().name(), "svg", "expected SVG root element");

    let values = root
        .attribute("viewBox")
        .expect("Sequence root viewBox")
        .split_whitespace()
        .map(|part| part.parse::<f64>().expect("numeric viewBox component"))
        .collect::<Vec<_>>();
    let view_box: [f64; 4] = values
        .try_into()
        .unwrap_or_else(|values: Vec<f64>| panic!("expected four viewBox values: {values:?}"));

    let max_width = root
        .attribute("style")
        .expect("Sequence root style")
        .split(';')
        .map(str::trim)
        .find_map(|declaration| declaration.strip_prefix("max-width:"))
        .map(str::trim)
        .and_then(|value| value.strip_suffix("px"))
        .and_then(|value| value.parse::<f64>().ok())
        .expect("numeric Sequence root max-width");

    (view_box, max_width)
}

fn sequence_number_x(svg: &str, number: &str) -> f64 {
    extract_paired_tags(svg, "text")
        .into_iter()
        .find(|tag| {
            tag.contains(r#"class="sequenceNumber""#) && tag.ends_with(&format!(">{number}</text>"))
        })
        .and_then(|tag| attr_f64(tag, "x"))
        .unwrap_or_else(|| panic!("missing sequence number {number}: {svg}"))
}

fn render_sequence_svg_from_fixture(fixture: &str) -> String {
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join(fixture);
    let text = std::fs::read_to_string(&path).expect("fixture");
    render_sequence_svg_from_text(&text)
}

fn render_sequence_svg_from_fixture_with_options(
    fixture: &str,
    options: &SvgRenderOptions,
) -> String {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join(fixture);
    let text = std::fs::read_to_string(&path).expect("fixture");
    let parsed = parse_sequence_for_render(&Engine::new(), &text);
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Sequence artifact");

    artifact
        .render_svg(options, &SvgDebugOptions::default())
        .expect("render Sequence artifact")
        .svg()
        .to_string()
}

fn sequence_layout_json_from_fixture(fixture: &str) -> serde_json::Value {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join(fixture);
    let text = std::fs::read_to_string(&path).expect("fixture");
    let parsed = parse_sequence_for_render(&Engine::new(), &text);

    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Sequence artifact")
        .layout_json()
        .expect("project Sequence layout JSON")
}

fn render_sequence_svg_from_text(text: &str) -> String {
    let engine = Engine::new();
    render_sequence_svg_from_text_with_engine(engine, text)
}

fn render_sequence_svg_from_text_with_engine(engine: Engine, text: &str) -> String {
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .unwrap();
    let parsed = parse_sequence_for_render(&engine, text);
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare Sequence artifact");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Sequence artifact")
        .svg()
        .to_string()
}

#[test]
fn sequence_actor_links_follow_mermaid_security_level() {
    let strict = render_sequence_svg_from_text(
        r#"sequenceDiagram
participant Alice
link Alice: Docs @ https://example.test/docs
link Alice: Script @ javascript:alert(1)
"#,
    );
    assert!(
        strict.contains(r#"xlink:href="https://example.test/docs""#),
        "{strict}"
    );
    assert!(
        strict.contains("<a><text") && !strict.contains("javascript:alert(1)"),
        "{strict}"
    );

    let loose = render_sequence_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "loose"
        }))),
        r#"sequenceDiagram
participant Alice
link Alice: Script @ javascript:alert(1)
"#,
    );
    assert!(
        loose.contains(r#"xlink:href="about:blank""#)
            && loose.contains(r#"target="_blank""#)
            && !loose.to_ascii_lowercase().contains("javascript:"),
        "{loose}"
    );
}

fn render_sequence_svg_with_theme_variables(
    text: &str,
    theme_variables: serde_json::Value,
) -> String {
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .unwrap();
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": theme_variables,
    })));
    let parsed = parse_sequence_for_render(&engine, text);
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare Sequence artifact");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Sequence artifact")
        .svg()
        .to_string()
}

fn layout_sequence_from_text(text: &str) -> SequenceDiagramLayout {
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic());
    layout_sequence_from_environment(text, &environment)
}

#[test]
fn sequence_builtin_route_reuses_message_bound_metrics_within_one_operation() {
    for message_count in [0usize, 1, 16, 64] {
        for repeated_text in [false, true] {
            let mut source = String::from("sequenceDiagram\nparticipant A\nparticipant B\n");
            for message_index in 0..message_count {
                let label = if repeated_text {
                    "operation-scoped-message-bound-metrics".to_string()
                } else {
                    format!("operation-scoped-message-bound-metrics-{message_index}")
                };
                source.push_str(&format!("A->>B: {label}\n"));
            }

            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let parsed = parse_sequence_for_render(&Engine::new(), &source);
            let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
                .expect("prepare Sequence artifact");
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .expect("render Sequence artifact");
            let (_, _, _, session) = rendered.into_parts();

            let dimension_calls: u64 = session
                .text_measurement_report()
                .entries()
                .iter()
                .filter(|entry| {
                    entry.provenance().operation
                        == TextMeasurementOperation::MermaidCalculateTextDimensions
                })
                .map(|entry| entry.count())
                .sum();

            assert_eq!(
                dimension_calls,
                4 + 2 * message_count as u64,
                "two configured/sans-serif actor probes and one probe pair per message should remain for message_count={message_count}, repeated_text={repeated_text}; later horizontal and vertical probes must reuse the operation-owned result"
            );
        }
    }
}

#[test]
fn sequence_builtin_route_reuses_self_and_multiline_message_bounds() {
    let dimension_calls = |source: &str| {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let parsed = parse_sequence_for_render(&Engine::new(), source);
        let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
            .expect("prepare Sequence artifact");
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render Sequence artifact");
        let (_, _, _, session) = rendered.into_parts();

        session
            .text_measurement_report()
            .entries()
            .iter()
            .filter(|entry| {
                entry.provenance().operation
                    == TextMeasurementOperation::MermaidCalculateTextDimensions
            })
            .map(|entry| entry.count())
            .sum::<u64>()
    };

    assert_eq!(
        dimension_calls("sequenceDiagram\nparticipant A\nparticipant B\nA->>A: self-message\n"),
        6,
        "self-message horizontal, vertical, and root-bound consumers must reuse one probe pair"
    );
    assert_eq!(
        dimension_calls(
            "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: first-line<br/>second-line\n"
        ),
        8,
        "each explicit line keeps one configured/sans-serif pair while later consumers reuse the combined bounds"
    );
}

const SEQUENCE_BLOCK_MESSAGE_TRACE_SOURCE: &str = r#"sequenceDiagram
participant A
participant B
loop loop-control
  A->>B: probe-loop-message
end
alt alt-control
  A->>B: probe-alt-message
else alt-empty
end
opt opt-control
  A->>B: probe-opt-message
end
rect rgb(240,240,240)
  A->>B: probe-rect-message
end
"#;

#[test]
fn sequence_builtin_route_reuses_control_block_message_metrics_through_svg_emission() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let parsed = parse_sequence_for_render(&Engine::new(), SEQUENCE_BLOCK_MESSAGE_TRACE_SOURCE);
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare Sequence artifact");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Sequence artifact");
    let (_, _, _, session) = rendered.into_parts();

    let dimension_calls = session
        .text_measurement_report()
        .entries()
        .iter()
        .filter(|entry| {
            entry.provenance().operation == TextMeasurementOperation::MermaidCalculateTextDimensions
        })
        .map(|entry| entry.count())
        .sum::<u64>();

    assert_eq!(
        dimension_calls, 36,
        "the fixed Mermaid control-block corpus should retain title probes while reusing the four messages' built-in bounds; dropping the sidecar adds six duplicate message probes"
    );
}

#[test]
fn sequence_host_route_preserves_message_measurement_callback_sequence() {
    const LABEL: &str = "operation-scoped-message-bound-metrics";
    let source = format!("sequenceDiagram\nparticipant A\nparticipant B\nA->>B: {LABEL}\n");
    let host = render_sequence_with_host_environment(
        &source,
        SequenceHostResponse::Missing,
        "test.sequence-host-observability",
        RenderEnvironment::deterministic(),
    );
    let message_requests = host
        .requests
        .iter()
        .filter(|request| {
            request.text == LABEL
                && request.operation == TextMeasurementOperation::MermaidCalculateTextDimensions
        })
        .collect::<Vec<_>>();
    assert_eq!(
        message_requests.len(),
        6,
        "host/fallback routes must retain actor-spacing, horizontal, and vertical configured/sans-serif probes"
    );
    assert!(message_requests.iter().all(|request| {
        request.phase == TextMeasurementPhase::SvgBBox
            && request.max_width_bits.is_none()
            && request.wrap_mode == WrapMode::SvgLike
    }));
    let configured_family = message_requests[1].font_family.as_deref();
    assert_ne!(configured_family, Some("sans-serif"));
    for pair in message_requests.chunks_exact(2) {
        assert_eq!(pair[0].font_family.as_deref(), Some("sans-serif"));
        assert_eq!(pair[1].font_family.as_deref(), configured_family);
        assert_eq!(pair[0].font_size_bits, pair[1].font_size_bits);
        assert_eq!(pair[0].font_weight, pair[1].font_weight);
        assert_eq!(pair[0].font_style, pair[1].font_style);
    }

    let parity = render_sequence_with_environment(&source, &RenderEnvironment::deterministic());
    assert_eq!(host.render.svg, parity.svg);
}

#[test]
fn sequence_stateful_host_results_affect_geometry_without_changing_the_callback_trace() {
    let missing = render_sequence_with_host_environment(
        SEQUENCE_BLOCK_MESSAGE_TRACE_SOURCE,
        SequenceHostResponse::Missing,
        "test.sequence-stateful-control-missing",
        RenderEnvironment::deterministic(),
    );
    let first = render_sequence_with_host_environment(
        SEQUENCE_BLOCK_MESSAGE_TRACE_SOURCE,
        SequenceHostResponse::StatefulMetrics,
        "test.sequence-stateful-control-first",
        RenderEnvironment::deterministic(),
    );
    let second = render_sequence_with_host_environment(
        SEQUENCE_BLOCK_MESSAGE_TRACE_SOURCE,
        SequenceHostResponse::StatefulMetrics,
        "test.sequence-stateful-control-second",
        RenderEnvironment::deterministic(),
    );

    assert_sequence_probe_message_trace(&first.requests);
    assert_eq!(first.requests, missing.requests);
    assert_eq!(second.requests, first.requests);
    assert_eq!(second.render.svg, first.render.svg);
    assert_ne!(
        first.render.svg, missing.render.svg,
        "large stateful message metrics must materially affect Sequence geometry"
    );
    assert_eq!(
        first.stateful_response_count, 36,
        "every candidate-relevant probe callback must consume a fresh host result"
    );
    let host_dimension_calls = first
        .render
        .report
        .entries()
        .iter()
        .filter(|entry| {
            let provenance = entry.provenance();
            provenance.operation == TextMeasurementOperation::MermaidCalculateTextDimensions
                && provenance.source == TextMeasurementSource::Host
                && provenance.fallback_reason.is_none()
        })
        .map(|entry| entry.count())
        .sum::<u64>();
    assert_eq!(host_dimension_calls, 36);
    assert_eq!(
        first
            .render
            .report
            .entries()
            .iter()
            .map(|entry| entry.count())
            .sum::<u64>(),
        first.requests.len() as u64
    );
    assert!(first.render.report.entries().iter().all(|entry| {
        let provenance = entry.provenance();
        (provenance.source == TextMeasurementSource::Host
            && provenance.operation == TextMeasurementOperation::MermaidCalculateTextDimensions
            && provenance.fallback_reason.is_none())
            || (provenance.source == TextMeasurementSource::Profile
                && provenance.fallback_reason == Some(HostFallbackReason::Missing))
    }));
}

#[test]
fn sequence_host_route_preserves_control_block_message_callback_trace() {
    let host = render_sequence_with_host_environment(
        SEQUENCE_BLOCK_MESSAGE_TRACE_SOURCE,
        SequenceHostResponse::Missing,
        "test.sequence-block-host-observability",
        RenderEnvironment::deterministic(),
    );
    assert_sequence_probe_message_trace(&host.requests);

    let parity = render_sequence_with_environment(
        SEQUENCE_BLOCK_MESSAGE_TRACE_SOURCE,
        &RenderEnvironment::deterministic(),
    );
    assert_eq!(host.render.svg, parity.svg);
}

#[test]
fn sequence_host_error_preserves_control_block_trace_svg_and_fallback_provenance() {
    let missing = render_sequence_with_host_environment(
        SEQUENCE_BLOCK_MESSAGE_TRACE_SOURCE,
        SequenceHostResponse::Missing,
        "test.sequence-block-host-missing",
        RenderEnvironment::deterministic(),
    );
    let error = render_sequence_with_host_environment(
        SEQUENCE_BLOCK_MESSAGE_TRACE_SOURCE,
        SequenceHostResponse::Error,
        "test.sequence-block-host-error",
        RenderEnvironment::deterministic(),
    );

    assert_sequence_probe_message_trace(&error.requests);
    assert_eq!(error.requests, missing.requests);
    assert_eq!(error.render.svg, missing.render.svg);
    assert_eq!(
        error
            .render
            .report
            .entries()
            .iter()
            .map(|entry| entry.count())
            .sum::<u64>(),
        error.requests.len() as u64
    );
    assert!(error.render.report.entries().iter().all(|entry| {
        let provenance = entry.provenance();
        provenance.source == TextMeasurementSource::Profile
            && provenance.fallback_reason == Some(HostFallbackReason::Error)
    }));
}

#[test]
fn sequence_wrapped_messages_and_notes_do_not_reuse_ordinary_message_metrics() {
    const ELIGIBLE_SOURCE: &str = r#"sequenceDiagram
participant A
participant B
A->>B: eligible-sidecar-control
"#;
    let eligible_builtin =
        render_sequence_with_environment(ELIGIBLE_SOURCE, &RenderEnvironment::deterministic());
    let eligible_host = render_sequence_with_host_environment(
        ELIGIBLE_SOURCE,
        SequenceHostResponse::Missing,
        "test.sequence-sidecar-eligible-control",
        RenderEnvironment::deterministic(),
    );
    assert_eq!(eligible_host.render.svg, eligible_builtin.svg);
    assert!(
        measurement_operation_count(
            &eligible_host.render.report,
            TextMeasurementOperation::MermaidCalculateTextDimensions,
        ) > measurement_operation_count(
            &eligible_builtin.report,
            TextMeasurementOperation::MermaidCalculateTextDimensions,
        ),
        "the control must prove that the comparison detects eligible built-in sidecar reuse"
    );

    const WRAPPED_SOURCE: &str = r#"sequenceDiagram
participant A
participant B
A->>B: wrap: wrapped-sidecar-sentinel alpha beta gamma delta epsilon zeta
"#;
    assert_sequence_shape_does_not_reuse_ordinary_message_metrics(
        "wrapped-message",
        WRAPPED_SOURCE,
        "wrapped-sidecar-sentinel",
        RenderEnvironment::deterministic(),
        RenderEnvironment::deterministic(),
    );

    const NOTE_SOURCE: &str = r#"sequenceDiagram
participant A
participant B
Note over A,B: note-sidecar-sentinel
"#;
    assert_sequence_shape_does_not_reuse_ordinary_message_metrics(
        "note",
        NOTE_SOURCE,
        "note-sidecar-sentinel",
        RenderEnvironment::deterministic(),
        RenderEnvironment::deterministic(),
    );
}

#[cfg(feature = "math")]
#[test]
fn sequence_math_messages_do_not_reuse_ordinary_message_metrics() {
    const SOURCE: &str = r#"sequenceDiagram
participant A
participant B
A->>B: math-sidecar-sentinel $$x^2 + y^2$$ tail
"#;
    assert_sequence_shape_does_not_reuse_ordinary_message_metrics(
        "math-message",
        SOURCE,
        "math-sidecar-sentinel",
        RenderEnvironment::deterministic()
            .with_math_renderer(Arc::new(merman_render::math::RatexMathRenderer)),
        RenderEnvironment::deterministic()
            .with_math_renderer(Arc::new(merman_render::math::RatexMathRenderer)),
    );
}

#[test]
fn sequence_message_metric_sidecars_are_isolated_while_prepared_artifact_lifetimes_overlap() {
    const FIRST_SOURCE: &str = r#"sequenceDiagram
participant A
participant B
loop first-control-title
  A->>B: concurrent-first-sidecar-message
end
"#;
    const SECOND_SOURCE: &str = r#"sequenceDiagram
participant X
participant Y
loop second-control-title-with-a-different-width
  X->>Y: concurrent-second-sidecar-message-with-a-much-longer-width
end
"#;

    let environment = RenderEnvironment::deterministic();
    let first_sequential = render_sequence_with_environment(FIRST_SOURCE, &environment);
    let second_sequential = render_sequence_with_environment(SECOND_SOURCE, &environment);

    let (first_prepared_tx, first_prepared_rx) = mpsc::channel();
    let (first_release_tx, first_release_rx) = mpsc::channel();
    let (second_prepared_tx, second_prepared_rx) = mpsc::channel();
    let (second_release_tx, second_release_rx) = mpsc::channel();
    let first_handle = {
        let environment = environment.clone();
        std::thread::spawn(move || {
            render_prepared_sequence_after_release(
                FIRST_SOURCE,
                environment,
                first_prepared_tx,
                first_release_rx,
            )
        })
    };
    let second_handle = {
        let environment = environment.clone();
        std::thread::spawn(move || {
            render_prepared_sequence_after_release(
                SECOND_SOURCE,
                environment,
                second_prepared_tx,
                second_release_rx,
            )
        })
    };

    let first_prepared = first_prepared_rx.recv();
    let second_prepared = second_prepared_rx.recv();

    // Release both waiters even when either prepare path unwound and disconnected its channel.
    let _ = first_release_tx.send(());
    let _ = second_release_tx.send(());

    let first_joined = first_handle.join();
    let second_joined = second_handle.join();
    first_prepared.expect("first Sequence artifact reached the prepared state");
    second_prepared.expect("second Sequence artifact reached the prepared state");
    let first_concurrent = first_joined.expect("first Sequence render thread");
    let second_concurrent = second_joined.expect("second Sequence render thread");

    assert_eq!(first_concurrent.svg, first_sequential.svg);
    assert_eq!(second_concurrent.svg, second_sequential.svg);
    assert_eq!(
        normalized_measurement_counts(&first_concurrent.report),
        normalized_measurement_counts(&first_sequential.report)
    );
    assert_eq!(
        normalized_measurement_counts(&second_concurrent.report),
        normalized_measurement_counts(&second_sequential.report)
    );
    assert!(
        first_concurrent
            .svg
            .contains("concurrent-first-sidecar-message")
            && !first_concurrent
                .svg
                .contains("concurrent-second-sidecar-message")
    );
    assert!(
        second_concurrent
            .svg
            .contains("concurrent-second-sidecar-message")
            && !second_concurrent
                .svg
                .contains("concurrent-first-sidecar-message")
    );
}

#[test]
fn sequence_autonumber_anchors_to_current_activation_bounds_like_mermaid_11_15() {
    let svg = render_sequence_svg_from_text(
        r#"sequenceDiagram
    autonumber
    participant C as Client
    participant S as Server
    participant D as Database
    participant Q as Message Queue

    C->>+S: Submit Order
    S->>D: Save Order
    D-->>S: Confirm
    S->>Q: Send Notification
    S-->>-C: Return Order ID

    Note over Q: Async Processing
    Q->>S: Consume Message
    S->>C: Push Notification"#,
    );

    let activation = extract_self_closing_tags(&svg, "rect")
        .into_iter()
        .find(|tag| tag.contains(r#"class="activation0""#))
        .unwrap_or_else(|| panic!("missing activation rect: {svg}"));
    let activation_left = attr_f64(activation, "x").expect("activation x");
    let activation_width = attr_f64(activation, "width").expect("activation width");
    let activation_right = activation_left + activation_width;

    let n2 = sequence_number_x(&svg, "2");
    let n4 = sequence_number_x(&svg, "4");
    let n5 = sequence_number_x(&svg, "5");

    assert!(
        (n2 - (activation_left + 1.0)).abs() <= 0.0001,
        "expected message 2 number to sit inside the left activation bound, got {n2} for activation {activation}"
    );
    assert!(
        (n4 - (activation_left + 1.0)).abs() <= 0.0001,
        "expected message 4 number to sit inside the left activation bound, got {n4} for activation {activation}"
    );
    assert!(
        (n5 - (activation_right - 1.0)).abs() <= 0.0001,
        "expected message 5 number to sit inside the right activation bound, got {n5} for activation {activation}"
    );
}

#[test]
fn sequence_layout_nested_activation_bounds_include_full_stack_like_mermaid_11_15() {
    let layout = layout_sequence_from_text(
        r#"sequenceDiagram
    participant C as Caller
    participant A as Active

    C->>+A: Open outer
    A->>+A: Open inner
    C->>A: Call nested
    A-->>-A: Close inner
    C->>A: Call outer"#,
    );

    let a_center = layout
        .nodes
        .iter()
        .find(|node| node.id == "actor-top-A")
        .map(|node| node.x)
        .expect("actor A center");
    let c_to_a_edges: Vec<&LayoutEdge> = layout
        .edges
        .iter()
        .filter(|edge| edge.from == "C" && edge.to == "A")
        .collect();
    assert_eq!(c_to_a_edges.len(), 3, "expected three C->A messages");

    let nested_call = c_to_a_edges[1];
    let outer_call = c_to_a_edges[2];
    let expected_left_target = a_center - 5.0 - 3.0;

    assert!(
        (nested_call.points[1].x - expected_left_target).abs() <= 0.0001,
        "expected nested activation target to use the full activation stack left bound, got {} with A center {a_center}",
        nested_call.points[1].x
    );
    assert!(
        (outer_call.points[1].x - expected_left_target).abs() <= 0.0001,
        "expected remaining outer activation target to keep the same left bound, got {} with A center {a_center}",
        outer_call.points[1].x
    );
}

#[test]
fn sequence_control_structure_label_box_uses_configured_width() {
    let svg = render_sequence_svg_from_text(
        r#"---
config:
  sequence:
    labelBoxWidth: 96
---
sequenceDiagram
    Alice->>Bob: Start
    loop Retry
        Alice->>Bob: Again
    end
    alt Accepted
        Alice->>Bob: Continue
    else Rejected
        Bob-->>Alice: Stop
    end
    critical Establish connection
        Alice->>Bob: Connect
    option Retry later
        Bob-->>Alice: Retry
    end
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Sequence SVG");
    let control_structures = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g") && node.attribute("data-et") == Some("control-structure")
        })
        .collect::<Vec<_>>();
    assert_eq!(control_structures.len(), 3);

    for control_structure in control_structures {
        let polygon = control_structure
            .descendants()
            .find(|node| {
                node.has_tag_name("polygon")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "labelBox")
                    })
            })
            .expect("control-structure label box");
        let points = polygon
            .attribute("points")
            .expect("label box points")
            .split_whitespace()
            .map(|point| {
                point
                    .split_once(',')
                    .map(|(x, y)| (x.parse::<f64>().unwrap(), y.parse::<f64>().unwrap()))
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert!((points[1].0 - points[0].0 - 96.0).abs() <= f64::EPSILON);

        let label = control_structure
            .descendants()
            .find(|node| {
                node.has_tag_name("text")
                    && node.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|class| class == "labelText")
                    })
            })
            .expect("control-structure label text");
        let label_x = label
            .attribute("x")
            .expect("label x")
            .parse::<f64>()
            .unwrap();
        assert!((label_x - (points[0].0 + 48.0).round()).abs() <= f64::EPSILON);
    }
}

#[test]
fn sequence_representative_roots_are_finite_and_scale_with_fixture_complexity() {
    let cases = [
        "activation_explicit.mmd",
        "stress_sequence_batch5_many_participants_spacing_050.mmd",
        "zed_pr_57644_sequence.mmd",
    ];
    let mut roots = Vec::new();

    for fixture in cases {
        let svg =
            render_sequence_svg_from_fixture_with_options(fixture, &SvgRenderOptions::default());
        let (view_box, max_width) = root_view_box_and_max_width(&svg);

        assert!(
            view_box.into_iter().all(f64::is_finite),
            "expected finite root geometry for {fixture}: {view_box:?}"
        );
        assert!(
            view_box[2] > 0.0 && view_box[3] > 0.0 && max_width.is_finite(),
            "expected positive root extent for {fixture}: viewBox={view_box:?}, max-width={max_width}"
        );
        assert!(
            (max_width - view_box[2]).abs() <= 1e-6,
            "root max-width must track viewBox width for {fixture}: viewBox={view_box:?}, max-width={max_width}"
        );

        roots.push((view_box[2], view_box[3]));
    }

    let activation = roots[0];
    let many_participants = roots[1];
    let long_conversation = roots[2];
    assert!(
        many_participants.0 > long_conversation.0 && long_conversation.0 > activation.0,
        "participant count should drive representative root widths: {roots:?}"
    );
    assert!(
        long_conversation.1 > many_participants.1 && many_participants.1 > activation.1,
        "message depth should drive representative root heights: {roots:?}"
    );
}

#[test]
fn sequence_block_root_width_replays_upstream_bounds_insert_lifecycle() {
    for (fixture, expected_min_x, expected_width) in [
        ("stress_create_destroy_inside_alt_030.mmd", -50.0, 734.0),
        ("stress_critical_break_007.mmd", -50.0, 650.0),
    ] {
        let svg = render_sequence_svg_from_fixture(fixture);
        let (view_box, max_width) = root_view_box_and_max_width(&svg);
        assert_eq!(
            view_box[0], expected_min_x,
            "unexpected root x for {fixture}"
        );
        assert_eq!(
            view_box[2], expected_width,
            "unexpected width for {fixture}"
        );
        assert_eq!(max_width, expected_width);
    }
}

#[test]
fn sequence_actor_lifecycle_adjustment_survives_block_close() {
    let fixture = "upstream_cypress_sequencediagram_spec_should_render_a_sequence_diagram_with_actor_creation_and_destruc_010.mmd";
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join(fixture);
    let text = std::fs::read_to_string(path).expect("fixture");
    let layout = layout_sequence_from_environment(&text, &RenderEnvironment::deterministic());
    let actor = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("missing lifecycle actor {id}"))
    };

    let alice_top = actor("actor-top-Alice");
    let bob_top = actor("actor-top-Bob");
    let john_top = actor("actor-top-John");
    let alice_bottom = actor("actor-bottom-Alice");
    let bob_bottom = actor("actor-bottom-Bob");
    let john_bottom = actor("actor-bottom-John");

    assert!(
        john_top.y > alice_top.y.max(bob_top.y),
        "created actor must begin below the initially declared actors"
    );
    assert!(
        john_bottom.y < alice_bottom.y.min(bob_bottom.y),
        "destroyed actor must end before ordinary footer actors"
    );

    let lifeline = layout
        .edges
        .iter()
        .find(|edge| edge.id == "lifeline-John")
        .expect("John lifecycle edge");
    assert_eq!(lifeline.from, john_top.id);
    assert_eq!(lifeline.to, john_bottom.id);
    let lifeline_start = lifeline.points.first().expect("lifeline start").y;
    let lifeline_end = lifeline.points.last().expect("lifeline end").y;
    let creation_boundary = john_top.y + john_top.height / 2.0;
    let destruction_boundary = john_bottom.y - john_bottom.height / 2.0;

    assert!(
        (lifeline_start - creation_boundary).abs() <= 1e-6
            && (lifeline_end - destruction_boundary).abs() <= 1e-6
            && lifeline_start < lifeline_end,
        "John's lifeline must remain bounded by its create/destroy actors after block closure"
    );
}

#[test]
fn sequence_font_size_precedence_matches_fresh_mermaid_11_16_root() {
    let svg = render_sequence_svg_from_fixture_with_options(
        "stress_sequence_font_size_precedence_090.mmd",
        &SvgRenderOptions::default(),
    );
    let root = svg.split_once('>').expect("SVG root").0;
    let note = extract_self_closing_tags(&svg, "rect")
        .into_iter()
        .find(|tag| tag.contains(r#"class="note""#))
        .expect("expected note rectangle");

    assert_eq!(attr_f64(note, "height"), Some(31.0));
    assert!(
        root.contains(r#"style="max-width: 550px; background-color: white;""#)
            && root.contains(r#"viewBox="-50 -10 550 244""#),
        "expected fresh Mermaid 11.16 font-size root geometry: {root}"
    );
}

#[test]
fn sequence_reverse_message_align_uses_the_normalized_message_interval() {
    let source = "sequenceDiagram\nparticipant A\nparticipant B\nB->>A: reverse\n";
    let wrap_padding = 17.0;
    let render_with_align = |align: &str| {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "sequence": { "messageAlign": align, "wrapPadding": wrap_padding }
        })));
        render_sequence_svg_from_text_with_engine(engine, source)
    };
    let message_position = |svg: &str| {
        let document = roxmltree::Document::parse(svg).expect("valid Sequence SVG");
        let text = document
            .descendants()
            .find(|node| {
                node.has_tag_name("text")
                    && node.attribute("class").is_some_and(|classes| {
                        classes
                            .split_whitespace()
                            .any(|class| class == "messageText")
                    })
            })
            .expect("message text");
        let line = document
            .descendants()
            .find(|node| {
                node.has_tag_name("line")
                    && node.attribute("class").is_some_and(|classes| {
                        classes
                            .split_whitespace()
                            .any(|class| class.starts_with("messageLine"))
                    })
                    && node.attribute("data-et") == Some("message")
            })
            .expect("message line");
        let endpoint = |name: &str| {
            line.attribute(name)
                .unwrap_or_else(|| panic!("message {name}"))
                .parse::<f64>()
                .unwrap_or_else(|_| panic!("numeric message {name}"))
        };
        let x1 = endpoint("x1");
        let x2 = endpoint("x2");
        (
            text.attribute("x")
                .expect("message x")
                .parse::<f64>()
                .expect("numeric message x"),
            text.attribute("text-anchor")
                .expect("message anchor")
                .to_string(),
            x1.min(x2),
            x1.max(x2),
        )
    };

    let left_svg = render_with_align("left");
    let right_svg = render_with_align("right");
    let (left_x, left_anchor, left_edge, _) = message_position(&left_svg);
    let (right_x, right_anchor, _, right_edge) = message_position(&right_svg);

    assert_eq!(left_anchor, "start");
    assert_eq!(right_anchor, "end");
    assert!((left_x - (left_edge + wrap_padding)).abs() < f64::EPSILON);
    assert!((right_x - (right_edge - wrap_padding)).abs() < f64::EPSILON);
}

#[test]
fn sequence_parity_wraps_message_candidates_with_calculate_text_width_bbox() {
    let svg = render_sequence_svg_from_fixture_with_options(
        "stress_br_in_messages_notes_011.mmd",
        &SvgRenderOptions::default(),
    );
    let wrapped_message =
        "This is a longer message that should be wrapped by Mermaid&#39;s default behavior";
    let message_lines = extract_paired_tags(&svg, "text")
        .into_iter()
        .filter(|tag| tag.contains(r#"class="messageText""#))
        .collect::<Vec<_>>();

    assert_eq!(
        message_lines.len(),
        5,
        "Mermaid 11.16 keeps the wrapped message on one line: {message_lines:#?}"
    );
    assert!(
        message_lines
            .iter()
            .any(|line| line.contains(wrapped_message)),
        "expected the complete wrapped message in one SVG text node: {message_lines:#?}"
    );
}

#[test]
fn sequence_calculate_text_dimensions_wraps_long_notes_to_six_rows() {
    let expected_rows = [
        "Extremely utterly long",
        "line of longness which",
        "had previously",
        "overflown the actor box",
        "as it is much longer",
        "than what it should be",
    ];
    for fixture in [
        "upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026.mmd",
        "upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_notes_left_of_control_019.mmd",
    ] {
        let svg =
            render_sequence_svg_from_fixture_with_options(fixture, &SvgRenderOptions::default());
        let note_rows = text_rows_by_class(&svg, "noteText");
        let note_rect = extract_self_closing_tags(&svg, "rect")
            .into_iter()
            .find(|tag| tag.contains(r#"class="note""#))
            .expect("wrapped note rectangle");

        assert_eq!(note_rows, expected_rows, "unexpected rows for {fixture}");
        assert_eq!(
            attr_f64(note_rect, "width"),
            Some(173.0),
            "unexpected note width for {fixture}"
        );
    }
}

#[test]
fn sequence_calculate_text_dimensions_keeps_first_wrapped_message_on_two_rows() {
    let fixture =
        "upstream_cypress_sequencediagram_spec_should_render_with_wrapping_enabled_048.mmd";
    let svg = render_sequence_svg_from_fixture_with_options(fixture, &SvgRenderOptions::default());
    let message_rows = text_rows_by_class(&svg, "messageText");

    assert_eq!(
        message_rows.len(),
        10,
        "unexpected message rows: {message_rows:#?}"
    );
    assert_eq!(
        &message_rows[..2],
        [
            "Hello John, how are you today?",
            "I'm feeling quite verbose today."
        ],
        "the first wrapped message must stay on two rows"
    );
}

#[test]
fn sequence_fallback_wraps_block_candidates_without_losing_text() {
    let svg = render_sequence_svg_from_fixture_with_options(
        "upstream_critical_without_options_spec.mmd",
        &SvgRenderOptions::default(),
    );
    let loop_lines = text_rows_by_class(&svg, "loopText");

    assert_eq!(
        loop_lines.len(),
        2,
        "the configured critical-title cap must wrap into two rows: {loop_lines:#?}"
    );
    assert_eq!(loop_lines.join(" "), "[Establish a connection to the DB]");
}

#[test]
fn sequence_nested_opt_wraps_from_source_block_width_like_mermaid_11_16() {
    let fixture = "upstream_cypress_sequencediagram_spec_should_render_a_single_and_nested_opt_with_long_test_overflowing_037.mmd";
    let svg = render_sequence_svg_from_fixture_with_options(fixture, &SvgRenderOptions::default());
    let group_start = svg
        .find(r#"<g data-et="control-structure" data-id="i17">"#)
        .unwrap_or_else(|| panic!("missing nested opt control group: {svg}"));
    let group_tail = &svg[group_start..];
    let group_end = group_tail
        .find("</g>")
        .unwrap_or_else(|| panic!("unterminated nested opt control group: {group_tail}"));
    let loop_lines: Vec<&str> = extract_paired_tags(&group_tail[..group_end], "text")
        .into_iter()
        .filter(|tag| tag.contains(r#"class="loopText""#))
        .collect();

    assert_eq!(
        loop_lines.len(),
        3,
        "nested opt title should use three Mermaid 11.16 lines"
    );
    for (line, expected) in loop_lines.iter().zip([
        "[this is a nested opt",
        "with a long title that",
        "will overflow]",
    ]) {
        assert!(
            line.contains(&format!(">{expected}</tspan>")),
            "unexpected nested opt title line: {line}"
        );
    }
}

#[test]
fn sequence_layout_json_preserves_family_wire_shape() {
    for fixture in [
        "upstream_cypress_sequencediagram_spec_should_render_a_single_and_nested_opt_with_long_test_overflowing_037.mmd",
        "upstream_alt_multiple_elses_spec.mmd",
        "upstream_par_multiple_ands_spec.mmd",
        "upstream_critical_with_options_spec.mmd",
    ] {
        let layout_json = sequence_layout_json_from_fixture(fixture);
        assert_eq!(
            layout_json.pointer("/meta/diagram_type"),
            Some(&serde_json::Value::String("sequence".to_string())),
            "unexpected Sequence metadata projection for {fixture}"
        );
        assert_eq!(
            layout_json.pointer("/semantic/type"),
            Some(&serde_json::Value::String("sequence".to_string())),
            "unexpected Sequence semantic projection for {fixture}"
        );
        assert!(
            layout_json
                .pointer("/semantic/messages")
                .is_some_and(serde_json::Value::is_array),
            "Sequence semantic messages must remain an array for {fixture}: {layout_json}"
        );
        let layout = layout_json
            .pointer("/layout/SequenceDiagram")
            .and_then(serde_json::Value::as_object)
            .unwrap_or_else(|| {
                panic!("missing SequenceDiagram layout projection for {fixture}: {layout_json}")
            });
        assert!(
            ["nodes", "edges", "clusters"]
                .into_iter()
                .all(|key| layout.get(key).is_some_and(serde_json::Value::is_array)),
            "Sequence layout collections must remain arrays for {fixture}: {layout_json}"
        );
        assert!(
            layout
                .get("bounds")
                .is_some_and(serde_json::Value::is_object),
            "Sequence layout bounds must remain an object for {fixture}: {layout_json}"
        );
    }
}

#[test]
fn sequence_bracketed_block_titles_receive_the_renderer_bracket_pair() {
    let svg = render_sequence_svg_from_text(
        r#"sequenceDiagram
    par [Action 1]
        Alice->>Bob: First
    and [Action 2]
        Bob-->>Alice: Second
    end"#,
    );

    assert!(
        svg.contains(">[[Action 1]]</tspan>"),
        "expected the par title to retain its source brackets and receive renderer brackets: {svg}"
    );
    assert!(
        svg.contains(">[[Action 2]]</text>"),
        "expected the and title to retain its source brackets and receive renderer brackets: {svg}"
    );
}

#[test]
fn sequence_autonumber_renders_decimal_sequence_numbers() {
    let svg = render_sequence_svg_from_text(
        r#"sequenceDiagram
autonumber 10.01 .01
Alice->>Bob:Hello
Bob-->>Alice:Back
Bob->>Alice:Again"#,
    );

    assert!(
        svg.contains(r#"font-size="9px" text-anchor="middle" class="sequenceNumber">10.01</text>"#),
        "expected first decimal sequence number in SVG"
    );
    assert!(
        svg.contains(r#"font-size="9px" text-anchor="middle" class="sequenceNumber">10.02</text>"#),
        "expected second decimal sequence number rounded to hundredths"
    );
    assert!(
        svg.contains(r#"font-size="9px" text-anchor="middle" class="sequenceNumber">10.03</text>"#),
        "expected third decimal sequence number rounded to hundredths"
    );
    assert!(
        !svg.contains("10.019999"),
        "expected decimal sequence numbers to avoid floating point artifacts"
    );
}

#[test]
fn sequence_svg_honors_mermaid_11_15_theme_css_options() {
    let svg = render_sequence_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"themeVariables": {"actorBorder": "#220000", "actorBkg": "#330000", "actorTextColor": "#fafafa", "actorLineColor": "#444444", "signalColor": "#555555", "signalTextColor": "#777777", "labelBoxBorderColor": "#888888", "labelBoxBkgColor": "#999999", "labelTextColor": "#aaaaaa", "loopTextColor": "#bbbbbb", "noteBorderColor": "#cccccc", "noteBkgColor": "#dddddd", "noteTextColor": "#eeeeee", "noteFontWeight": 600, "activationBkgColor": "#010203", "activationBorderColor": "#040506", "nodeBorder": "#070809"}}}%%
sequenceDiagram
autonumber
participant Alice
participant Bob
Alice->>Bob: Hello
activate Bob
Note over Alice,Bob: Readable note
loop Retry
Alice-->>Bob: Again
end"##,
    );

    assert!(
        svg.contains(r#".actor{stroke:#220000;fill:#330000;"#),
        "expected actor theme variables in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#"text.actor>tspan{fill:#fafafa;stroke:none;}"#),
        "expected actor text theme color in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#".actor-line{stroke:#444444;}"#),
        "expected actor lifeline theme color in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#".messageLine0{stroke-width:1.5;stroke-dasharray:none;stroke:#555555;}"#),
        "expected signal color in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#".messageText{fill:#777777;stroke:none;}"#),
        "expected signal text color in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#".labelBox{stroke:#888888;fill:#999999;filter:none;}"#),
        "expected label box theme colors in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#".labelText,#merman .labelText>tspan{fill:#aaaaaa;stroke:none;}"#),
        "expected label text theme color in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#".loopText,#merman .loopText>tspan{fill:#bbbbbb;stroke:none;}"#),
        "expected loop text theme color in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#".sectionTitle,#merman .sectionTitle>tspan{fill:#bbbbbb;stroke:none;}"#),
        "expected section title theme color in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#".note{stroke:#cccccc;fill:#dddddd;}"#),
        "expected note theme colors in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(
            r#".noteText,#merman .noteText>tspan{fill:#eeeeee;stroke:none;font-weight:600;}"#
        ),
        "expected note text theme color and weight in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#".activation0{fill:#010203;stroke:#040506;}"#),
        "expected activation theme colors in Sequence CSS: {svg}"
    );
    assert!(
        svg.contains(r#"g rect.rect{filter:"#) && svg.contains(r#"stroke:#070809;"#),
        "expected Sequence rect node border theme color in CSS: {svg}"
    );
}

#[test]
fn sequence_note_width_expands_for_literal_br_backslash_t_with_fallback_profile() {
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join("html_br_variants_and_wrap.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let layout = layout_sequence_from_environment(&text, &RenderEnvironment::deterministic());

    let note = layout
        .nodes
        .iter()
        .find(|n| n.id == "note-7")
        .expect("expected note-7 layout node");

    // Mermaid's text-dimension probe treats the escaped `<br \t/>` as literal single-run text,
    // then adds the normal note padding. The reusable fallback profile must preserve that semantic
    // expansion without encoding the fixture's browser-specific width.
    assert!(
        note.width > 150.0 && note.width.is_finite(),
        "expected literal escaped <br> note to expand beyond the default width, got {}",
        note.width
    );
}

#[test]
fn sequence_alt_multiple_elses_separators_touch_frame_edges() {
    let svg = render_sequence_svg_from_fixture("upstream_alt_multiple_elses_spec.mmd");

    let line_tags = extract_self_closing_tags(&svg, "line");
    let loop_lines: Vec<&str> = line_tags
        .into_iter()
        .filter(|t| t.contains(r#"class="loopLine""#))
        .collect();

    let dashed_separators: Vec<&str> = loop_lines
        .iter()
        .copied()
        .filter(|t| t.contains("stroke-dasharray: 3, 3"))
        .collect();
    assert_eq!(
        dashed_separators.len(),
        2,
        "expected 2 dashed separators for 3 alt sections"
    );

    let y0 = attr_f64(dashed_separators[0], "y1").expect("sep y1");
    let y1 = attr_f64(dashed_separators[1], "y1").expect("sep y1");
    assert!(
        y0 < y1,
        "expected section separators to increase monotonically, got {y0} then {y1}"
    );

    let mut frame_min_x = f64::INFINITY;
    let mut frame_max_x = f64::NEG_INFINITY;
    for t in &loop_lines {
        if t.contains("style=") {
            continue;
        }
        let (Some(x1), Some(x2)) = (attr_f64(t, "x1"), attr_f64(t, "x2")) else {
            continue;
        };
        if (x1 - x2).abs() <= 0.0001 {
            frame_min_x = frame_min_x.min(x1);
            frame_max_x = frame_max_x.max(x1);
        }
    }
    assert!(frame_min_x.is_finite() && frame_max_x.is_finite());

    for sep in dashed_separators {
        let x1 = attr_f64(sep, "x1").expect("sep x1");
        let x2 = attr_f64(sep, "x2").expect("sep x2");
        assert!(
            x1 <= frame_min_x + 0.0001,
            "expected separator x1 ({x1}) to touch frame left edge ({frame_min_x})"
        );
        assert!(
            x2 >= frame_max_x - 0.0001,
            "expected separator x2 ({x2}) to touch frame right edge ({frame_max_x})"
        );
    }
}

#[test]
fn sequence_zed_59651_nested_frame_headers_follow_the_preceding_note() {
    let svg = render_sequence_svg_from_text(
        r#"sequenceDiagram
    participant S as Server
    participant C as Client

    Note over S: ① Initialize connection
    loop for each request
        alt request is valid
            S->>C: process normally
        else
            S-->>C: return error
        end
    end
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Sequence SVG");

    let note = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "rect"
                && node.attribute("class") == Some("note")
        })
        .expect("issue fixture note");
    let note_bottom = note
        .attribute("y")
        .expect("note y")
        .parse::<f64>()
        .expect("numeric note y")
        + note
            .attribute("height")
            .expect("note height")
            .parse::<f64>()
            .expect("numeric note height");

    let control_group = |title: &str| {
        document
            .descendants()
            .find(|node| {
                node.is_element()
                    && node.tag_name().name() == "g"
                    && node.attribute("data-et") == Some("control-structure")
                    && node
                        .descendants()
                        .filter(|descendant| descendant.is_text())
                        .filter_map(|descendant| descendant.text())
                        .any(|text| text.contains(title))
            })
            .unwrap_or_else(|| panic!("control structure containing {title:?}"))
    };
    let frame_bounds = |group: roxmltree::Node<'_, '_>| {
        let mut horizontal_ys = group
            .children()
            .filter(|node| {
                node.is_element()
                    && node.tag_name().name() == "line"
                    && node.attribute("class") == Some("loopLine")
                    && node.attribute("style").is_none()
            })
            .filter_map(|node| {
                let y1 = node.attribute("y1")?.parse::<f64>().ok()?;
                let y2 = node.attribute("y2")?.parse::<f64>().ok()?;
                ((y1 - y2).abs() < 0.0001).then_some(y1)
            });
        let top = horizontal_ys.next().expect("frame top");
        let bottom = horizontal_ys.next().expect("frame bottom");
        (top.min(bottom), top.max(bottom))
    };
    let title_y = |group: roxmltree::Node<'_, '_>| {
        group
            .descendants()
            .find(|node| {
                node.is_element()
                    && node.tag_name().name() == "text"
                    && node.attribute("class") == Some("loopText")
            })
            .and_then(|node| node.attribute("y"))
            .and_then(|value| value.parse::<f64>().ok())
            .expect("numeric loop title y")
    };

    let outer_loop = control_group("for each request");
    let inner_alt = control_group("request is valid");
    let (outer_top, outer_bottom) = frame_bounds(outer_loop);
    let (inner_top, inner_bottom) = frame_bounds(inner_alt);
    let outer_title_y = title_y(outer_loop);
    let inner_title_y = title_y(inner_alt);
    const LABEL_BOX_HEIGHT: f64 = 20.0;

    assert!(
        note_bottom < outer_top && outer_top < inner_top,
        "expected note.bottom < outer loop.top < inner alt.top, got \
         {note_bottom} < {outer_top} < {inner_top}: {svg}"
    );
    assert!(
        outer_top < outer_title_y
            && outer_top + LABEL_BOX_HEIGHT < inner_top
            && outer_title_y + LABEL_BOX_HEIGHT < inner_top
            && inner_top < inner_title_y
            && inner_title_y < inner_bottom
            && inner_bottom < outer_bottom,
        "expected nested frames and labels to occupy distinct vertical bands: {svg}"
    );

    let separator_y = inner_alt
        .children()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "line"
                && node.attribute("style") == Some("stroke-dasharray: 3, 3;")
        })
        .and_then(|node| node.attribute("y1"))
        .and_then(|value| value.parse::<f64>().ok())
        .expect("numeric alt separator y");
    assert!(
        inner_top + LABEL_BOX_HEIGHT < separator_y
            && inner_title_y + LABEL_BOX_HEIGHT < separator_y
            && separator_y < inner_bottom,
        "expected the alt section separator to advance below its title and stay inside the frame"
    );
}

#[test]
fn sequence_issue_86_nested_loop_and_alt_labels_keep_separate_vertical_positions() {
    let svg = render_sequence_svg_from_text(
        r#"sequenceDiagram
    participant A
    participant B
    loop Outer loop
        alt Inner branch
            A->>B: Message
        end
    end
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Sequence SVG");

    let title_y = |label: &str| {
        document
            .descendants()
            .find(|node| {
                node.is_element()
                    && node.tag_name().name() == "text"
                    && node.attribute("class") == Some("loopText")
                    && node
                        .descendants()
                        .filter(|descendant| descendant.is_text())
                        .filter_map(|descendant| descendant.text())
                        .any(|text| text.contains(label))
            })
            .and_then(|node| node.attribute("y"))
            .and_then(|value| value.parse::<f64>().ok())
            .unwrap_or_else(|| panic!("missing numeric loop title {label:?}: {svg}"))
    };

    let outer_y = title_y("[Outer loop]");
    let inner_y = title_y("[Inner branch]");
    assert!(
        inner_y - outer_y >= 20.0,
        "nested loop and alt labels must occupy separate vertical bands, got outer y={outer_y}, inner y={inner_y}: {svg}"
    );
}

#[test]
fn sequence_rect_block_is_root_level_before_actors() {
    let svg = render_sequence_svg_from_fixture("upstream_rect_block_spec.mmd");

    let fill_pos = svg
        .find(r#"fill="rgb(200, 255, 200)""#)
        .expect("expected rect fill to match directive payload");
    let rect_pos = svg[..fill_pos]
        .rfind("<rect")
        .expect("expected rect tag for fill");
    let rect_end_rel = svg[rect_pos..]
        .find("/>")
        .expect("expected self-closing rect tag");
    let rect_tag = &svg[rect_pos..(rect_pos + rect_end_rel + 2)];
    assert!(rect_tag.contains(r#"class="rect""#), "expected rect class");

    let actor_pos = svg
        .find(r#"class="actor actor-bottom""#)
        .expect("expected bottom actors");
    assert!(
        rect_pos < actor_pos,
        "expected rect blocks to be emitted before actor groups"
    );
}

#[test]
fn sequence_bare_rect_uses_resolved_theme_fill_and_explicit_override() {
    let bare_rect = r#"sequenceDiagram
participant A
participant B
rect
A->>B: Hello
end"#;

    let rect_fill = render_sequence_svg_with_theme_variables(
        bare_rect,
        serde_json::json!({
            "rectBkgColor": "#112233",
            "actorBkg": "#445566"
        }),
    );
    assert!(
        extract_self_closing_tags(&rect_fill, "rect")
            .into_iter()
            .any(|tag| tag.contains(r#"class="rect""#) && tag.contains(r##"fill="#112233""##)),
        "rectBkgColor should be the first bare rect fallback: {rect_fill}"
    );

    let explicit_fill = render_sequence_svg_with_theme_variables(
        &bare_rect.replacen("rect\n", "rect rgb(1, 2, 3)\n", 1),
        serde_json::json!({ "rectBkgColor": "#112233" }),
    );
    assert!(
        extract_self_closing_tags(&explicit_fill, "rect")
            .into_iter()
            .any(|tag| tag.contains(r#"class="rect""#) && tag.contains(r#"fill="rgb(1, 2, 3)""#)),
        "an explicit rect color should override theme fallbacks: {explicit_fill}"
    );
}

#[test]
fn sequence_nested_rect_blocks_render_in_start_order() {
    let svg = render_sequence_svg_from_fixture("upstream_nested_rect_blocks_spec.mmd");

    let outer = svg
        .find(r#"fill="rgb(200, 255, 200)""#)
        .expect("expected outer rect fill");
    let inner = svg
        .find(r#"fill="rgb(0, 0, 0)""#)
        .expect("expected inner rect fill");
    assert!(
        outer < inner,
        "expected nested rect blocks to be emitted in start order"
    );
}

#[test]
fn sequence_notes_render_inline_with_block_frames() {
    let svg = render_sequence_svg_from_fixture("stress_end_in_labels_025.mmd");

    let loop_pos = svg
        .find("[health(end)check]")
        .expect("expected loop frame label");
    let note_pos = svg.find(r#"class="note""#).expect("expected note group");
    let alt_pos = svg
        .find("[should continue]")
        .expect("expected alt frame label");

    assert!(
        loop_pos < note_pos,
        "expected completed loop frame to render before the later note"
    );
    assert!(
        note_pos < alt_pos,
        "expected note to render before its enclosing alt frame closes"
    );
}

#[test]
fn sequence_notes_expand_viewbox_left_for_leftof_notes() {
    let svg = render_sequence_svg_from_fixture("notes_placements.mmd");
    assert!(
        svg.contains(r#"viewBox="-150 -10"#),
        "expected viewBox min_x to expand for left-of notes"
    );
    assert!(
        svg.contains(r#"max-width: 750px"#),
        "expected max-width to reflect expanded viewBox width"
    );
}

#[test]
#[ignore = "documented Sequence root-width residual: deterministic local 570px vs Mermaid 11.16 upstream 567px"]
fn sequence_long_leftof_notes_keep_mermaid_11_16_root_width() {
    for fixture in [
        "upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026.mmd",
        "upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_notes_left_of_control_019.mmd",
    ] {
        let svg = render_sequence_svg_from_fixture(fixture);
        assert!(
            svg.contains(r#"max-width: 567px"#),
            "expected long left-of note fixture {fixture} to keep Mermaid 11.16 root width"
        );
    }
}

#[test]
fn sequence_long_leftof_notes_drop_the_stale_width_slack() {
    for fixture in [
        "upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026.mmd",
        "upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_notes_left_of_control_019.mmd",
    ] {
        let path = workspace_root()
            .join("fixtures")
            .join("sequence")
            .join(fixture);
        let text = std::fs::read_to_string(&path).expect("fixture");
        let environment = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::deterministic());
        let layout = layout_sequence_from_environment(&text, &environment);
        let note = layout
            .nodes
            .iter()
            .find(|n| n.id == "note-1")
            .expect("expected note-1 layout node");
        assert_eq!(
            note.width, 155.0,
            "expected long left-of note fixture {fixture} to use the source-backed wrapped width"
        );
    }
}

#[test]
fn sequence_frontmatter_title_expands_layout_root_y() {
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join("upstream_html_demos_sequence_sequence_diagram_demos_002.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let parsed = parse_sequence_for_render(&Engine::new(), &text);
    assert_eq!(
        parsed.metadata().title.as_deref(),
        Some("With forced menus")
    );
    let RenderSemanticModel::Sequence(model) = parsed.model() else {
        panic!("expected Sequence render model");
    };
    assert!(
        model.title.is_none(),
        "frontmatter title should stay in parse metadata, not the sequence semantic title"
    );

    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic());
    let layout = layout_sequence_from_environment(&text, &environment);
    let bounds = layout.bounds.as_ref().expect("sequence root bounds");
    assert_eq!(bounds.min_y, -50.0);
}

#[test]
fn sequence_message_font_size_override_matches_mermaid_cli_baselines() {
    // Mermaid CLI (mmdc) currently does not reflect `sequence.messageFontSize` overrides in the
    // emitted SVG; it sticks to the global `fontSize` defaults. Keep our Stage B output aligned
    // with the upstream baselines under `fixtures/upstream-svgs/sequence`.
    let svg = render_sequence_svg_from_fixture(
        "upstream_cypress_sequencediagram_spec_should_render_different_message_fonts_when_configured_011.mmd",
    );
    assert!(
        svg.contains("font-size: 16px"),
        "expected message/actor text to use the global fontSize (16px) like Mermaid CLI baselines"
    );
    assert!(
        !svg.contains("font-size: 18px"),
        "expected sequence.messageFontSize (18px) to not affect SVG output under the pinned upstream baselines"
    );
}

#[test]
fn sequence_central_connection_rtl_layout_matches_fixture_golden_spacing() {
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join(
            "upstream_cypress_sequencediagram_v2_spec_should_render_central_connection_with_normal_arrows_right_to_lef_033.mmd",
        );
    let text = std::fs::read_to_string(&path).expect("fixture");

    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic());
    let layout = layout_sequence_from_environment(&text, &environment);

    let actor_center = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .map(|node| node.x)
            .unwrap_or_else(|| panic!("missing node {id}"))
    };

    assert_eq!(actor_center("actor-top-Alice"), 75.0);
    assert_eq!(actor_center("actor-top-Bob"), 443.0);
    assert_eq!(actor_center("actor-top-Charlie"), 820.0);

    let edge = layout
        .edges
        .iter()
        .find(|edge| edge.id == "msg-1")
        .expect("expected first central-connection edge");
    assert_eq!(edge.points.len(), 2);
    assert_eq!(edge.points[0].x, 442.0);
    assert_eq!(edge.points[1].x, 83.0);
}

#[test]
fn sequence_central_connection_rtl_svg_uses_layout_actor_centers() {
    let fixture = "upstream_cypress_sequencediagram_v2_spec_should_render_central_connection_with_normal_arrows_right_to_lef_033.mmd";
    let svg = render_sequence_svg_from_fixture(fixture);

    assert!(
        svg.contains(r#"<text x="443" y="32.5""#),
        "expected Bob top actor center from layout to be preserved in SVG: {svg}"
    );
    assert!(
        svg.contains(r#"<text x="820" y="32.5""#),
        "expected Charlie top actor center from layout to be preserved in SVG: {svg}"
    );
    assert!(
        extract_self_closing_tags(&svg, "line")
            .into_iter()
            .any(|tag| {
                tag.contains(r#"x1="442""#)
                    && tag.contains(r#"x2="83""#)
                    && tag.contains(r#"class="messageLine"#)
            }),
        "expected first message x positions to stay near layout/golden spacing: {svg}"
    );
}

#[cfg(feature = "math")]
#[test]
fn sequence_svg_renders_ratex_math_message_and_note_end_to_end() {
    let text = r#"sequenceDiagram
participant A
participant B
A->>B: $$x^2$$
Note right of B: $$x^2$$
"#;
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .with_math_renderer(Arc::new(merman_render::math::RatexMathRenderer));
    let session = environment.begin_session().unwrap();
    let parsed = parse_sequence_for_render(&Engine::new(), text);
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare Sequence artifact");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Sequence artifact");
    let svg = rendered.svg();

    assert!(
        svg.contains(r#"width="0.97153em""#),
        "expected RaTeX inline SVG sizing in sequence labels: {svg}"
    );
    assert!(
        svg.contains(r#"<div style="width: fit-content;""#),
        "expected Sequence math labels to use the KaTeX foreignObject shell: {svg}"
    );
    assert!(
        svg.contains("<path"),
        "expected RaTeX glyph paths in sequence SVG: {svg}"
    );
    assert!(
        !svg.contains("$$x^2$$"),
        "expected math source delimiters to be replaced by rendered SVG: {svg}"
    );
}

#[cfg(feature = "math")]
#[test]
fn sequence_docs_math_fixture_renders_supported_ratex_formulas() {
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join("upstream_docs_math_sequence_002.mmd");
    let text = std::fs::read_to_string(&path).expect("fixture");

    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .with_math_renderer(Arc::new(merman_render::math::RatexMathRenderer));
    let session = environment.begin_session().unwrap();
    let parsed = parse_sequence_for_render(&Engine::new(), &text);
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare Sequence artifact");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Sequence artifact");
    let svg = rendered.svg();

    let inline_formula_count = svg
        .matches(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 "#)
        .count();
    assert!(
        inline_formula_count >= 7,
        "expected participant, message, and note math labels to render through RaTeX: {svg}"
    );
    assert!(
        !svg.contains(r#"Solve: $$\sqrt{2+2}$$"#) && !svg.contains(r#"Answer: $$2$$"#),
        "expected mixed sequence message formulas to replace source delimiters: {svg}"
    );
}

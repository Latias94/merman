mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions, ParsedDiagramRender, RenderSemanticModel};
use merman_render::DiagramFamilyId;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, InsetsPx, OrdinalSelector,
    Specified, TextStylePatch as ThemeTextStylePatch, ThemePortabilityRequirement, ThemeRule,
    ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
};

use merman_render::environment::{
    HostFallbackReason, HostMeasurementResult, HostTextMeasurement, HostTextMeasurementError,
    HostTextMeasurementRequest, HostTextMeasurer, MeasurementProfileId, RenderEnvironment,
    TextMeasurementOperation, TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfile,
    TextMeasurementProfileIdentity, TextMeasurementReport, TextMeasurementRoute,
    TextMeasurementSource,
};
use merman_render::family;
use merman_render::model::{LayoutEdge, SequenceDiagramLayout};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, WrapMode};
use merman_render::{
    Error, LayoutOptions, RenderResourcePolicy, ResourceLimitCause, ResourceLimitId,
    ResourceLimitPhase,
};
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
    WeightSensitiveMetrics,
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
            SequenceHostResponse::WeightSensitiveMetrics if request.text.starts_with("probe-") => {
                let advance = if request.style.font_weight.as_deref() == Some("700") {
                    20.0
                } else {
                    8.0
                };
                Ok(Some(HostTextMeasurement::Metrics(TextMetrics {
                    width: request.text.len() as f64 * advance,
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
    let (svg, family_report) = rendered.into_completion().into_output_and_report();
    SequenceRenderObservation {
        svg,
        report: family_report.session_report().measurement().clone(),
    }
}

fn try_render_sequence_svg_with_resource_policy(
    source: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Sequence resource-bound session");
    let parsed = parse_sequence_for_render(&Engine::new(), source);
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn sequence_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"sequenceDiagram
participant A as Alice
participant B as Bob
Note over A,B: bounded family-level note
A->>B: hello
"#;
    let baseline = try_render_sequence_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Sequence baseline");
    let exact_bytes = baseline.len();
    assert!(
        exact_bytes > 1,
        "Sequence fixture must emit a non-empty SVG"
    );

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Sequence SVG byte ceiling");
    let exact = try_render_sequence_svg_with_resource_policy(source, exact_policy)
        .expect("the exact Sequence family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Sequence SVG byte ceiling");
    let error = try_render_sequence_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the Sequence family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Sequence MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.max, below_exact);
    assert!(limit.actual > limit.max);
    assert!(limit.explicit_overrides.iter().any(|resource_override| {
        resource_override.id == ResourceLimitId::MaxSvgBytes
            && resource_override.value == below_exact
    }));
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
    let (svg, family_report) = rendered.into_completion().into_output_and_report();
    SequenceRenderObservation {
        svg,
        report: family_report.session_report().measurement().clone(),
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

fn sequence_number_label_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::SequenceNumberLabel,
                        ThemeStylePatch::default().with_fill(fill),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence number label theme")
}

fn sequence_role_paint_theme(fill: CanvasPaint, stroke: CanvasPaint) -> DiagramTheme {
    let role_fill = |target| {
        ThemeRule::new(target, ThemeStylePatch::default().with_fill(fill.clone()))
            .for_family(DiagramFamilyId::SEQUENCE)
    };
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(role_fill(ThemeTarget::ActorLabel))
                    .with_rule(role_fill(ThemeTarget::MessageLabel))
                    .with_rule(role_fill(ThemeTarget::NoteLabel))
                    .with_rule(role_fill(ThemeTarget::LoopLabel))
                    .with_rule(role_fill(ThemeTarget::LoopLabelBackground))
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::LoopLabelBackground,
                            ThemeStylePatch::default().with_stroke(stroke),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
            ),
        )
        .expect("compile Sequence role paint theme")
}

fn inline_style_value<'a>(style: &'a str, property: &str) -> Option<&'a str> {
    style.split(';').find_map(|declaration| {
        let (name, value) = declaration.split_once(':')?;
        if !name.trim().eq_ignore_ascii_case(property) {
            return None;
        }
        let value = value.trim();
        let value = value.strip_suffix("!important").unwrap_or(value).trim();
        Some(
            value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .unwrap_or(value),
        )
    })
}

fn css_property_values_for_exact_selector<'a>(
    css: &'a str,
    selector: &str,
    property: &str,
) -> Vec<&'a str> {
    let mut values = Vec::new();
    for rule in css.split('}') {
        let Some((selectors, declarations)) = rule.rsplit_once('{') else {
            continue;
        };
        if !selectors
            .split(',')
            .any(|candidate| candidate.trim() == selector)
        {
            continue;
        }
        for declaration in declarations.split(';') {
            let Some((name, value)) = declaration.split_once(':') else {
                continue;
            };
            if name.trim() == property {
                values.push(value.trim());
            }
        }
    }
    values
}

fn assert_sequence_role_text_style(
    document: &roxmltree::Document<'_>,
    class_name: &str,
    text_fragment: &str,
    family: &str,
    size: &str,
    weight: &str,
    style: &str,
) {
    let text = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_ascii_whitespace()
                        .any(|class| class == class_name)
                })
                && node
                    .descendants()
                    .filter(|descendant| descendant.is_text())
                    .filter_map(|descendant| descendant.text())
                    .any(|text| text.contains(text_fragment))
        })
        .unwrap_or_else(|| panic!("missing {class_name} text containing {text_fragment:?}"));
    let inline = text.attribute("style").expect("role text inline style");
    assert_eq!(inline_style_value(inline, "font-family"), Some(family));
    assert_eq!(inline_style_value(inline, "font-size"), Some(size));
    assert_eq!(inline_style_value(inline, "font-weight"), Some(weight));
    assert_eq!(inline_style_value(inline, "font-style"), Some(style));
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
    render_sequence_svg_from_text_with_options(text, &SvgRenderOptions::default())
}

fn render_sequence_svg_from_text_with_options(text: &str, options: &SvgRenderOptions) -> String {
    let engine = Engine::new();
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .unwrap();
    let parsed = parse_sequence_for_render(&engine, text);
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare Sequence artifact");

    artifact
        .render_svg(options, &SvgDebugOptions::default())
        .expect("render Sequence artifact")
        .svg()
        .to_string()
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

fn sequence_control_frame_x(svg: &str, label: &str) -> (f64, f64) {
    let document = roxmltree::Document::parse(svg).expect("valid Sequence SVG");
    let group = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node.attribute("data-et") == Some("control-structure")
                && node
                    .descendants()
                    .filter(|descendant| descendant.is_text())
                    .filter_map(|descendant| descendant.text())
                    .any(|text| text.contains(label))
        })
        .unwrap_or_else(|| panic!("missing control structure {label:?}: {svg}"));

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    for line in group.children().filter(|node| {
        node.is_element()
            && node.tag_name().name() == "line"
            && node.attribute("class") == Some("loopLine")
            && node.attribute("style").is_none()
    }) {
        let x1 = line
            .attribute("x1")
            .and_then(|value| value.parse::<f64>().ok())
            .expect("numeric frame x1");
        let x2 = line
            .attribute("x2")
            .and_then(|value| value.parse::<f64>().ok())
            .expect("numeric frame x2");
        min_x = min_x.min(x1).min(x2);
        max_x = max_x.max(x1).max(x2);
    }
    assert!(
        min_x.is_finite() && max_x.is_finite(),
        "control structure {label:?} has no frame lines: {svg}"
    );
    (min_x, max_x)
}

#[test]
fn sequence_root_id_is_safe_for_direct_css_selectors() {
    for raw_id in ["a.b", "a:b"] {
        let svg = render_sequence_svg_from_text_with_options(
            "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: hello\n",
            &SvgRenderOptions {
                diagram_id: Some(raw_id.to_string()),
                ..SvgRenderOptions::default()
            },
        );

        assert!(svg.contains(r#"id="a-b""#), "{raw_id}: {svg}");
        assert!(svg.contains("<style>#a-b{"), "{raw_id}: {svg}");
        assert!(!svg.contains(&format!("#{raw_id}{{")), "{raw_id}: {svg}");
    }
}

#[test]
fn sequence_large_diagram_id_preflight_counts_dynamic_message_references() {
    fn render_with_limit(text: &str, diagram_id: &str, maximum: usize) -> Result<String, Error> {
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, maximum)
            .unwrap();
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
            .begin_session()
            .unwrap();
        let parsed = parse_sequence_for_render(&Engine::new(), text);
        let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
            .expect("prepare Sequence artifact");
        artifact
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some(diagram_id.to_string()),
                    ..SvgRenderOptions::default()
                },
                &SvgDebugOptions::default(),
            )
            .map(|rendered| rendered.svg().to_string())
    }

    let diagram_id = "diagram".repeat(128);
    let base = "sequenceDiagram\nparticipant A\nparticipant B\n";
    let options = SvgRenderOptions {
        diagram_id: Some(diagram_id.clone()),
        ..SvgRenderOptions::default()
    };
    let base_svg = render_sequence_svg_from_text_with_options(base, &options);
    let base_occurrences = base_svg.matches(&diagram_id).count();
    assert!(
        base_occurrences > 0,
        "base Sequence SVG must use the diagram ID"
    );

    let message_count = 96usize;
    let mut messages = String::from(base);
    messages.push_str("autonumber\n");
    for _ in 0..message_count {
        messages.push_str("A->>B: hello\n");
    }
    let fanout_ceiling = base_occurrences * diagram_id.len();
    let Error::ResourceLimitExceeded(fanout) =
        render_with_limit(&messages, &diagram_id, fanout_ceiling)
            .expect_err("message ID fanout must exceed the base-only contribution")
    else {
        panic!("expected dynamic diagram-ID projection error");
    };
    assert_eq!(fanout.cause, ResourceLimitCause::Ceiling);
    assert_eq!(fanout.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(fanout.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert!(
        fanout.actual > fanout_ceiling,
        "message references must increase the projected SVG bytes beyond the base-only ceiling: {fanout:?}"
    );
    assert_eq!(fanout.max, fanout_ceiling);

    let full_svg = render_sequence_svg_from_text_with_options(&messages, &options);
    let exact_bytes = full_svg.len();
    let exact = render_with_limit(&messages, &diagram_id, exact_bytes)
        .expect("exact final SVG byte ceiling must succeed");
    assert_eq!(exact, full_svg);

    let Error::ResourceLimitExceeded(n_minus_one) = render_with_limit(
        &messages,
        &diagram_id,
        exact_bytes.checked_sub(1).expect("non-empty Sequence SVG"),
    )
    .expect_err("N-1 final SVG byte ceiling must fail") else {
        panic!("expected N-1 SVG byte projection error");
    };
    assert_eq!(n_minus_one.cause, ResourceLimitCause::Ceiling);
    assert_eq!(n_minus_one.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(n_minus_one.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(n_minus_one.actual, exact_bytes);
    assert_eq!(n_minus_one.max, exact_bytes - 1);
}

#[test]
fn sequence_half_arrows_use_the_mermaid_11_16_marker_and_line_class_table() {
    let svg = render_sequence_svg_from_text(
        r#"sequenceDiagram
participant A
participant B
A-|\B: solid top
A-|/B: solid bottom
A-\\B: stick top
A-//B: stick bottom
A--|\B: dotted solid top
A--|/B: dotted solid bottom
A--\\B: dotted stick top
A--//B: dotted stick bottom
A/|-B: reverse solid top
A\|-B: reverse solid bottom
A//-B: reverse stick top
A\\-B: reverse stick bottom
A/|--B: dotted reverse solid top
A\|--B: dotted reverse solid bottom
A//--B: dotted reverse stick top
A\\--B: dotted reverse stick bottom
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Sequence half-arrow SVG");
    let expected = [
        (0, "messageLine0", None, Some("solidTopArrowHead")),
        (1, "messageLine0", None, Some("solidBottomArrowHead")),
        (2, "messageLine0", None, Some("stickTopArrowHead")),
        (3, "messageLine0", None, Some("stickBottomArrowHead")),
        (4, "messageLine1", None, Some("solidTopArrowHead")),
        (5, "messageLine1", None, Some("solidBottomArrowHead")),
        (6, "messageLine1", None, Some("stickTopArrowHead")),
        (7, "messageLine1", None, Some("stickBottomArrowHead")),
        (8, "messageLine0", Some("solidBottomArrowHead"), None),
        (9, "messageLine0", Some("solidTopArrowHead"), None),
        (10, "messageLine0", Some("stickBottomArrowHead"), None),
        (11, "messageLine0", Some("stickTopArrowHead"), None),
        (12, "messageLine1", Some("solidBottomArrowHead"), None),
        (13, "messageLine1", Some("solidTopArrowHead"), None),
        (14, "messageLine1", Some("stickBottomArrowHead"), None),
        (15, "messageLine1", Some("stickTopArrowHead"), None),
    ];

    for (id, expected_class, expected_start, expected_end) in expected {
        let data_id = format!("i{id}");
        let message = document
            .descendants()
            .find(|node| {
                node.is_element()
                    && node.attribute("data-et") == Some("message")
                    && node.attribute("data-id") == Some(data_id.as_str())
            })
            .unwrap_or_else(|| panic!("missing Sequence message {data_id}: {svg}"));
        assert_eq!(
            message.attribute("class"),
            Some(expected_class),
            "{data_id}"
        );
        assert_eq!(
            message.attribute("marker-start"),
            expected_start
                .map(|marker| format!("url(#merman-{marker})"))
                .as_deref(),
            "{data_id} marker-start"
        );
        assert_eq!(
            message.attribute("marker-end"),
            expected_end
                .map(|marker| format!("url(#merman-{marker})"))
                .as_deref(),
            "{data_id} marker-end"
        );
        assert_eq!(
            message
                .attribute("style")
                .is_some_and(|style| style.contains("stroke-dasharray: 3, 3")),
            expected_class == "messageLine1",
            "{data_id} dash style"
        );
    }
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
            let (_, family_report) = rendered.into_completion().into_output_and_report();

            let dimension_calls: u64 = family_report
                .session_report()
                .measurement()
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
        let (_, family_report) = rendered.into_completion().into_output_and_report();

        family_report
            .session_report()
            .measurement()
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
    let (_, family_report) = rendered.into_completion().into_output_and_report();

    let dimension_calls = family_report
        .session_report()
        .measurement()
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
        RenderEnvironment::deterministic().with_compiled_math_renderer(),
        RenderEnvironment::deterministic().with_compiled_math_renderer(),
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
fn sequence_layout_nested_activation_bounds_include_full_stack_with_neo_marker_spacing() {
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
    // Mermaid 12 applies Neo spacing before shortening the line for its arrowhead.
    let activation_half_width = 5.0;
    let neo_marker_spacing = 3.0;
    let arrowhead_shortening = 3.0;
    let expected_left_target =
        a_center - activation_half_width - neo_marker_spacing - arrowhead_shortening;

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
fn sequence_control_labels_follow_look_height_margin_and_font() {
    let source = r#"sequenceDiagram
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
"#;
    // Mermaid 12 drawLoop adds Neo height before applying the zero-height fallback.
    for (look, configured_height, margin, expected_height) in [
        ("neo", 20.0, 5.0, 35.0),
        ("classic", 20.0, 5.0, 20.0),
        ("neo", 42.0, 0.0, 57.0),
        ("classic", 42.0, -5.0, 42.0),
        ("neo", 0.0, 5.5, 15.0),
        ("classic", 0.0, 5.5, 20.0),
        ("neo", -15.0, 5.0, 20.0),
        ("neo", -5.0, 5.0, 10.0),
        ("classic", -5.0, 5.0, -5.0),
    ] {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "look": look,
            "fontSize": 22,
            "sequence": {
                "labelBoxHeight": configured_height,
                "labelBoxWidth": 96,
                "boxTextMargin": margin
            }
        })));
        let svg = render_sequence_svg_from_text_with_engine(engine, source);
        let document = roxmltree::Document::parse(&svg).expect("valid Sequence SVG");
        let controls = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("g") && node.attribute("data-et") == Some("control-structure")
            })
            .collect::<Vec<_>>();
        assert_eq!(controls.len(), 3);
        for control in controls {
            let polygon = control
                .descendants()
                .find(|node| {
                    node.has_tag_name("polygon") && node.attribute("class") == Some("labelBox")
                })
                .expect("control label box");
            let points = polygon
                .attribute("points")
                .expect("label box points")
                .split_whitespace()
                .map(|point| {
                    let (x, y) = point.split_once(',').expect("coordinate pair");
                    (x.parse::<f64>().unwrap(), y.parse::<f64>().unwrap())
                })
                .collect::<Vec<_>>();
            assert_eq!(points.len(), 5);
            assert!(
                (points[3].1 - points[0].1 - expected_height).abs() <= 1e-6,
                "{look} control label height: configured={configured_height}, points={points:?}"
            );
            assert!((points[2].1 - points[3].1 + 7.0).abs() <= 1e-6);
            assert!((points[1].0 - points[0].0 - 96.0).abs() <= 1e-6);
            assert!((points[2].0 - points[3].0 - 8.4).abs() <= 1e-6);
            let label = control
                .descendants()
                .find(|node| {
                    node.has_tag_name("text") && node.attribute("class") == Some("labelText")
                })
                .expect("control label text");
            let label_y = label
                .attribute("y")
                .expect("label y")
                .parse::<f64>()
                .unwrap();
            let center_y = points[0].1 + expected_height / 2.0;
            let expected_y = if margin > 0.0 {
                (center_y + margin / 2.0 + 0.5).floor()
            } else {
                center_y
            };
            assert!(
                (label_y - expected_y).abs() <= 1e-6,
                "{look} label baseline"
            );
            assert_eq!(
                inline_style_value(label.attribute("style").expect("label style"), "font-size"),
                Some("22px")
            );
        }
    }
}

#[test]
fn sequence_control_titles_use_resolved_message_font_weight() {
    let source = r#"sequenceDiagram
    alt Accepted
        A->>B: Continue
    else Rejected
        B-->>A: Stop
    end
"#;
    for (config, expected_weight) in [
        (
            serde_json::json!({"sequence": {"messageFontWeight": 700}}),
            Some("700"),
        ),
        (
            serde_json::json!({"sequence": {"messageFontWeight": "bold"}}),
            Some("bold"),
        ),
        (
            serde_json::json!({"fontWeight": 500, "sequence": {"messageFontWeight": 700}}),
            Some("500"),
        ),
        (
            serde_json::json!({"fontWeight": "600", "sequence": {"messageFontWeight": 700}}),
            Some("600"),
        ),
        (
            serde_json::json!({"fontWeight": 0, "sequence": {"messageFontWeight": 700}}),
            Some("700"),
        ),
        (
            serde_json::json!({"sequence": {"messageFontWeight": "700; font-style: italic"}}),
            None,
        ),
    ] {
        let svg = render_sequence_svg_from_text_with_engine(
            Engine::new().with_site_config(MermaidConfig::from_value(config.clone())),
            source,
        );
        let document = roxmltree::Document::parse(&svg).expect("valid Sequence SVG");
        let titles = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("text")
                    && matches!(
                        node.attribute("class"),
                        Some("labelText" | "loopText" | "sectionTitle")
                    )
            })
            .collect::<Vec<_>>();
        assert_eq!(titles.len(), 3, "control keyword, title and section");
        for title in titles {
            let style = title.attribute("style").expect("title style");
            assert_eq!(
                inline_style_value(style, "font-weight").unwrap_or("400"),
                expected_weight.unwrap_or("400"),
                "{config}: {style}"
            );
            assert_eq!(
                inline_style_value(style, "font-style").unwrap_or("normal"),
                "normal",
                "invalid CSS weight must not inject an italic declaration"
            );
        }
    }
}

#[test]
fn sequence_representative_roots_are_finite_and_scale_with_fixture_complexity() {
    let cases = vec![
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
fn sequence_diagram_title_expands_the_root_for_typed_base_font_size() {
    let font_size = 256.0_f32;
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(
                    DiagramFamilyId::SEQUENCE,
                    ThemeTextStyle::default()
                        .with_font_size_px(font_size)
                        .expect("valid large Sequence base font size"),
                ),
            ),
        )
        .expect("compile large Sequence base typography");
    let engine = merman_render::__private::install_parse_compatibility(&theme, Engine::new());
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin portable Sequence title session");
    let artifact = family::prepare(
        parse_sequence_for_render(
            &engine,
            "sequenceDiagram\ntitle Large Diagram Title\nparticipant A\nparticipant B\nA->>B: hello\n",
        ),
        &LayoutOptions::default(),
        session,
    )
    .expect("prepare large Sequence title");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render large Sequence title");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Sequence title SVG");
    let title = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.attribute("class").is_none()
                && node.text() == Some("Large Diagram Title")
        })
        .expect("Sequence diagram title");
    let title_y = title
        .attribute("y")
        .expect("Sequence title y")
        .parse::<f64>()
        .expect("numeric Sequence title y");
    let (view_box, _) = root_view_box_and_max_width(rendered.svg());
    assert!(
        view_box[1] <= title_y - f64::from(font_size) * 0.8,
        "typed base font size must expand the root above the title baseline: viewBox={view_box:?}, title={title:?}"
    );
}

#[test]
fn sequence_classic_block_root_width_replays_upstream_bounds_insert_lifecycle() {
    for (fixture, expected_min_x, expected_width) in [
        ("stress_create_destroy_inside_alt_030.mmd", -50.0, 734.0),
        ("stress_critical_break_007.mmd", -50.0, 650.0),
    ] {
        let text = std::fs::read_to_string(
            workspace_root()
                .join("fixtures")
                .join("sequence")
                .join(fixture),
        )
        .expect("fixture");
        // Keep this bounds-insertion regression independent of Neo marker spacing.
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "sequence": { "look": "classic" }
        })));
        let svg = render_sequence_svg_from_text_with_engine(engine, &text);
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
fn sequence_svg_uses_resolved_add_message_lifecycle_ownership() {
    let layout = layout_sequence_from_environment(
        concat!(
            "sequenceDiagram\n",
            "participant A\n",
            "participant C\n",
            "create participant B\n",
            "destroy A\n",
            "loop pending\n",
            "Note over C: pending\n",
            "end\n",
            "autonumber\n",
            "activate C\n",
            "deactivate C\n",
            "C->>B: create\n",
            "A--xC: destroy\n",
        ),
        &RenderEnvironment::deterministic(),
    );
    let actor = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("missing lifecycle actor {id}"))
    };

    assert!(
        actor("actor-top-B").y > actor("actor-top-A").y.max(actor("actor-top-C").y),
        "the create signal after intervening records must own B's top actor"
    );
    assert!(
        actor("actor-bottom-A").y < actor("actor-bottom-B").y.min(actor("actor-bottom-C").y),
        "the signal following the shared create anchor must own A's destruction"
    );
}

#[test]
fn sequence_svg_supersedes_consecutive_same_kind_lifecycle_declarations() {
    let created = layout_sequence_from_environment(
        concat!(
            "sequenceDiagram\n",
            "participant A\n",
            "create participant B\n",
            "create participant C\n",
            "A->>C: create\n",
        ),
        &RenderEnvironment::deterministic(),
    );
    let created_actor = |id: &str| {
        created
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("missing created actor {id}"))
    };
    assert_eq!(
        created_actor("actor-top-B").y,
        created_actor("actor-top-A").y
    );
    assert!(created_actor("actor-top-C").y > created_actor("actor-top-A").y);

    let destroyed = layout_sequence_from_environment(
        concat!(
            "sequenceDiagram\n",
            "participant A\n",
            "participant B\n",
            "destroy A\n",
            "destroy B\n",
            "A--xB: destroy\n",
        ),
        &RenderEnvironment::deterministic(),
    );
    let destroyed_actor = |id: &str| {
        destroyed
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("missing destroyed actor {id}"))
    };
    assert!(
        destroyed_actor("actor-bottom-B").y < destroyed_actor("actor-bottom-A").y,
        "only the latest pending destroy declaration should shorten its actor lifeline"
    );
}

#[test]
fn sequence_root_font_size_wins_for_emitted_text_style() {
    let svg = render_sequence_svg_from_fixture_with_options(
        "stress_sequence_font_size_precedence_090.mmd",
        &SvgRenderOptions::default(),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Sequence SVG");
    let styled_text = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("text")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_whitespace()
                        .any(|class| matches!(class, "actor" | "messageText" | "noteText"))
                })
        })
        .collect::<Vec<_>>();

    assert!(!styled_text.is_empty());
    for text in styled_text {
        let style = text.attribute("style").expect("Sequence text inline style");
        assert_eq!(
            inline_style_value(style, "font-size"),
            Some("10px"),
            "the root fontSize must win for emitted Sequence text: {text:?}"
        );
    }
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
fn sequence_wrap_directives_preserve_text_and_nowrap_boundary() {
    let svg = render_sequence_svg_from_fixture_with_options(
        "stress_br_in_messages_notes_011.mmd",
        &SvgRenderOptions::default(),
    );
    let message_rows = text_rows_by_class(&svg, "messageText");
    let wrapped_start = message_rows
        .iter()
        .position(|row| row.starts_with("This is a longer message"))
        .expect("wrapped message start");
    let nowrap_start = message_rows
        .iter()
        .position(|row| row.starts_with("This message should not wrap"))
        .expect("nowrap message");

    assert_eq!(
        message_rows[wrapped_start..nowrap_start].join(" "),
        "This is a longer message that should be wrapped by Mermaid's default behavior"
    );
    assert_eq!(
        &message_rows[nowrap_start..],
        ["This message should not wrap even if it is long long long long long"]
    );
}

#[test]
fn sequence_long_notes_wrap_consistently_without_losing_text() {
    let expected_text = "Extremely utterly long line of longness which had previously overflown the actor box as it is much longer than what it should be";
    let mut reference_rows: Option<Vec<String>> = None;
    let mut reference_width: Option<f64> = None;
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

        assert!(
            note_rows.len() > 1,
            "wrap-enabled long note must use multiple rows for {fixture}: {note_rows:?}"
        );
        assert_eq!(note_rows.join(" "), expected_text, "text loss in {fixture}");

        let width = attr_f64(note_rect, "width").expect("note width");
        assert!(
            width.is_finite() && width > 0.0,
            "invalid width for {fixture}"
        );
        if let Some(reference_rows) = &reference_rows {
            assert_eq!(
                &note_rows, reference_rows,
                "equivalent Sequence variants must wrap identically"
            );
            assert_eq!(
                width.to_bits(),
                reference_width.expect("reference width").to_bits(),
                "equivalent Sequence variants must use identical deterministic widths"
            );
        } else {
            reference_rows = Some(note_rows);
            reference_width = Some(width);
        }
    }
}

#[test]
fn sequence_wrap_true_splits_the_first_message_without_losing_text() {
    let fixture =
        "upstream_cypress_sequencediagram_spec_should_render_with_wrapping_enabled_048.mmd";
    let svg = render_sequence_svg_from_fixture_with_options(fixture, &SvgRenderOptions::default());
    let message_rows = text_rows_by_class(&svg, "messageText");

    let expected = "Hello John, how are you today? I'm feeling quite verbose today.";
    let row_count = (1..=message_rows.len())
        .find(|&end| message_rows[..end].join(" ") == expected)
        .expect("first message must be reconstructable from leading rows");
    assert!(
        row_count > 1,
        "wrap=true must split the first long message: {message_rows:#?}"
    );
}

#[test]
fn sequence_critical_wrap_responds_to_controlled_width_boundaries() {
    let source = std::fs::read_to_string(
        workspace_root()
            .join("fixtures")
            .join("sequence")
            .join("upstream_critical_without_options_spec.mmd"),
    )
    .expect("critical fixture");
    let measurer =
        DeterministicTextMeasurer::default().with_width_callback(|text, style| match text {
            "[Establish a" => 90.0,
            "[Establish a connection" => 160.0,
            "connection to the" => 125.0,
            "connection to the DB]" => 170.0,
            "DB]" => 26.0,
            _ => {
                DeterministicTextMeasurer::default()
                    .measure(text, style)
                    .width
            }
        });
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.sequence-critical-browser-bounds").unwrap(),
            "fixture",
        )
        .unwrap(),
        measurer,
    );
    let environment = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile));
    let svg = render_sequence_with_environment(&source, &environment).svg;
    let lines = text_rows_by_class(&svg, "loopText");
    assert_eq!(lines, vec!["[Establish a", "connection to the", "DB]"]);
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
fn sequence_classic_nested_opt_wraps_from_source_block_width() {
    let fixture = "upstream_cypress_sequencediagram_spec_should_render_a_single_and_nested_opt_with_long_test_overflowing_037.mmd";
    let text = std::fs::read_to_string(
        workspace_root()
            .join("fixtures")
            .join("sequence")
            .join(fixture),
    )
    .expect("fixture");
    // Classic retains the source block width used by this deterministic wrapping oracle.
    // Neo shortens message bounds before calculateLoopBounds derives that width.
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "sequence": { "look": "classic" }
    })));
    let svg = render_sequence_svg_from_text_with_engine(engine, &text);
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
        "classic nested opt title should use three deterministic lines"
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
fn sequence_autonumber_off_preserves_and_advances_hidden_state() {
    let svg = render_sequence_svg_from_text(
        r#"sequenceDiagram
participant A
participant B
autonumber 10 5
A->>B: Visible first
autonumber off
A->>B: Hidden first
B-->>A: Hidden second
autonumber
A->>B: Visible resumed"#,
    );

    assert_eq!(
        text_rows_by_class(&svg, "sequenceNumber"),
        vec!["10".to_string(), "25".to_string()],
        "reenabling autonumber should preserve its step and include hidden signals in the counter"
    );
}

#[test]
fn sequence_open_line_types_render_without_svg_endpoint_markers() {
    let svg = render_sequence_svg_from_text(
        r#"sequenceDiagram
participant A
participant B
A->B: Headless solid
A-->B: Headless dotted
A->>B: Filled"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Sequence SVG");
    let message = |id: &str| {
        document
            .descendants()
            .find(|node| node.is_element() && node.attribute("data-id") == Some(id))
            .unwrap_or_else(|| panic!("missing Sequence message {id}: {svg}"))
    };

    let solid_headless = message("i0");
    assert_eq!(solid_headless.attribute("class"), Some("messageLine0"));
    assert_eq!(solid_headless.attribute("marker-start"), None);
    assert_eq!(solid_headless.attribute("marker-end"), None);

    let dotted_headless = message("i1");
    assert_eq!(dotted_headless.attribute("class"), Some("messageLine1"));
    assert_eq!(dotted_headless.attribute("marker-start"), None);
    assert_eq!(dotted_headless.attribute("marker-end"), None);
    assert!(
        dotted_headless
            .attribute("style")
            .is_some_and(|style| style.contains("stroke-dasharray: 3, 3")),
        "dotted headless signal should preserve its stroke style: {svg}"
    );

    let filled = message("i2");
    assert_eq!(filled.attribute("marker-start"), None);
    assert!(
        filled
            .attribute("marker-end")
            .is_some_and(|marker| marker.contains("-arrowhead)")),
        "filled signal should retain its target marker: {svg}"
    );
}

#[test]
fn sequence_headless_strokes_follow_mermaid_neo_endpoint_spacing() {
    for (look, dotted_offset) in [("classic", 0.0), ("neo", 3.0)] {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "look": look
        })));
        let svg = render_sequence_svg_from_text_with_engine(
            engine,
            r#"sequenceDiagram
participant A
participant B
A->B: Headless solid
A-->B: Headless dotted
B->A: Headless solid left
B-->A: Headless dotted left"#,
        );
        let document = roxmltree::Document::parse(&svg).expect("valid Sequence SVG");
        let endpoints = |id: &str| {
            let message = document
                .descendants()
                .find(|node| node.is_element() && node.attribute("data-id") == Some(id))
                .unwrap_or_else(|| panic!("missing Sequence message {id}: {svg}"));
            assert!(message.attribute("marker-start").is_none());
            assert!(message.attribute("marker-end").is_none());
            let coordinate = |name: &str| {
                message
                    .attribute(name)
                    .unwrap_or_else(|| panic!("missing {name} for Sequence message {id}: {svg}"))
                    .parse::<f64>()
                    .unwrap_or_else(|_| panic!("invalid {name} for Sequence message {id}: {svg}"))
            };
            (coordinate("x1"), coordinate("x2"))
        };

        // buildMessageModel exempts only SOLID_OPEN from the Neo target offset.
        for (solid_id, dotted_id, target_offset) in
            [("i0", "i1", -dotted_offset), ("i2", "i3", dotted_offset)]
        {
            let solid = endpoints(solid_id);
            let dotted = endpoints(dotted_id);
            assert_eq!(solid.0, dotted.0, "{look} headless start");
            assert_eq!(solid.1 + target_offset, dotted.1, "{look} headless target");
        }
    }
}

#[test]
fn sequence_classic_svg_honors_theme_css_options() {
    let svg = render_sequence_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"sequence": {"look": "classic", "noteFontWeight": 700}, "themeVariables": {"actorBorder": "#220000", "actorBkg": "#330000", "actorTextColor": "#fafafa", "actorLineColor": "#444444", "signalColor": "#555555", "signalTextColor": "#777777", "labelBoxBorderColor": "#888888", "labelBoxBkgColor": "#999999", "labelTextColor": "#aaaaaa", "loopTextColor": "#bbbbbb", "noteBorderColor": "#cccccc", "noteBkgColor": "#dddddd", "noteTextColor": "#eeeeee", "noteFontWeight": 600, "activationBkgColor": "#010203", "activationBorderColor": "#040506", "nodeBorder": "#070809"}}}%%
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
        svg.contains(r#".messageLine0{stroke-width:1.5;stroke-dasharray:none;}"#)
            && svg.contains(r#".messageLine0,#merman .messageLine1{stroke:#555555;}"#),
        "expected signal color in the single final Sequence writer rule: {svg}"
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
        svg.contains(r#".noteText,#merman .noteText>tspan{fill:#eeeeee;stroke:none;}"#),
        "expected note text theme color without a tspan weight override in Sequence CSS: {svg}"
    );
    let document = roxmltree::Document::parse(&svg).expect("Sequence SVG");
    let note_text = document
        .descendants()
        .find(|node| node.has_tag_name("text") && node.attribute("class") == Some("noteText"))
        .expect("note text");
    assert_eq!(
        inline_style_value(note_text.attribute("style").unwrap(), "font-weight"),
        Some("700"),
        "the configured note weight must reach the text that note tspans inherit"
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
fn sequence_role_text_and_loop_surface_paints_seal_terminal_css_and_dom() {
    let source = r#"sequenceDiagram
participant Alice
participant Bob
Alice->>Bob: Message Role
Note over Alice,Bob: Note Role
loop Loop Role
Bob-->>Alice: Reply Role
end"#;

    for (case, fill, stroke, fill_css, stroke_css) in [
        (
            "solid",
            CanvasPaint::solid("#fef3c7").expect("valid Sequence role fill"),
            CanvasPaint::solid("#c084fc").expect("valid Sequence Loop stroke"),
            "#fef3c7",
            "#c084fc",
        ),
        (
            "transparent",
            CanvasPaint::Transparent,
            CanvasPaint::Transparent,
            "transparent",
            "transparent",
        ),
    ] {
        let theme = sequence_role_paint_theme(fill, stroke);
        let parsed = merman_render::__private::install_parse_compatibility(
            &theme,
            classic_sequence_engine(),
        )
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap_or_else(|error| panic!("parse {case} Sequence role paint source: {error}"))
        .unwrap_or_else(|| panic!("detect {case} Sequence role paint source"));
        let session = RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .unwrap_or_else(|error| panic!("begin {case} Sequence role paint session: {error}"));
        let diagram_id = format!("sequence-role-paint-{case}");
        let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
            .unwrap_or_else(|error| panic!("prepare {case} Sequence role paint: {error}"))
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some(diagram_id.clone()),
                    ..SvgRenderOptions::default()
                },
                &SvgDebugOptions::default(),
            )
            .unwrap_or_else(|error| panic!("render {case} Sequence role paint: {error}"));
        let document = roxmltree::Document::parse(rendered.svg())
            .unwrap_or_else(|error| panic!("parse {case} Sequence role paint SVG: {error}"));
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("Sequence role paint stylesheet");

        for terminal_rule in [
            format!(
                "#{diagram_id} text.actor,#{diagram_id} text.actor>tspan,#{diagram_id} text.text,#{diagram_id} text.text>tspan{{fill:{fill_css};}}"
            ),
            format!(
                "#{diagram_id} .messageText,#{diagram_id} .messageText>tspan{{fill:{fill_css};}}"
            ),
            format!("#{diagram_id} .noteText,#{diagram_id} .noteText>tspan{{fill:{fill_css};}}"),
            format!(
                "#{diagram_id} .loopText,#{diagram_id} .loopText>tspan,#{diagram_id} .sectionTitle,#{diagram_id} .sectionTitle>tspan,#{diagram_id} .labelText,#{diagram_id} .labelText>tspan{{fill:{fill_css};}}"
            ),
            format!("#{diagram_id} .labelBox{{stroke:{stroke_css};fill:{fill_css};filter:none;}}"),
        ] {
            assert!(
                css.contains(&terminal_rule),
                "missing {case} Sequence terminal rule {terminal_rule}: {css}"
            );
        }

        for (class_name, text_fragment) in [
            ("actor", "Alice"),
            ("messageText", "Message Role"),
            ("noteText", "Note Role"),
            ("loopText", "Loop Role"),
            ("labelText", "loop"),
        ] {
            assert!(
                document.descendants().any(|node| {
                    node.has_tag_name("text")
                        && node.attribute("class").is_some_and(|classes| {
                            classes
                                .split_ascii_whitespace()
                                .any(|class| class == class_name)
                        })
                        && node.descendants().any(|descendant| {
                            descendant
                                .text()
                                .is_some_and(|text| text.contains(text_fragment))
                        })
                }),
                "missing {case} Sequence {class_name} occurrence for {text_fragment:?}"
            );
        }
        assert_eq!(
            document
                .descendants()
                .filter(|node| {
                    node.attribute("class").is_some_and(|classes| {
                        classes
                            .split_ascii_whitespace()
                            .any(|class| class == "labelBox")
                    })
                })
                .count(),
            1,
            "the Loop receipt must bind the one emitted labelBox"
        );
        drop(document);

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 6, "{case}");
        assert_eq!(evidence.accounted_count(), 6, "{case}");
        assert_eq!(evidence.applied_count(), 6, "{case}");
        assert_eq!(evidence.not_applicable_count(), 0, "{case}");
        assert_eq!(evidence.theme_residual_count(), 0, "{case}");
    }
}

#[test]
fn sequence_role_paints_respect_explicit_mermaid_owners_from_site_and_source() {
    const SOURCE: &str = r#"sequenceDiagram
autonumber
participant Alice
participant Bob
Alice->>Bob: Message Role
Note over Alice,Bob: Note Role
loop Loop Role
Bob-->>Alice: Reply Role
end"#;
    const MERMAID_COLOR: &str = "#fedcba";
    const TYPED_COLOR: &str = "#123456";
    let cases = [
        (
            "actor-label-fill",
            ThemeTarget::ActorLabel,
            "actorTextColor",
            "fill",
            "text.actor>tspan",
            "text",
            "actor",
            Some("Alice"),
            true,
            false,
        ),
        (
            "message-label-fill",
            ThemeTarget::MessageLabel,
            "signalTextColor",
            "fill",
            ".messageText",
            "text",
            "messageText",
            Some("Message Role"),
            false,
            false,
        ),
        (
            "note-label-fill",
            ThemeTarget::NoteLabel,
            "noteTextColor",
            "fill",
            ".noteText",
            "text",
            "noteText",
            Some("Note Role"),
            false,
            false,
        ),
        (
            "loop-control-label-fill",
            ThemeTarget::LoopLabel,
            "labelTextColor",
            "fill",
            ".labelText",
            "text",
            "labelText",
            Some("loop"),
            false,
            false,
        ),
        (
            "loop-title-label-fill",
            ThemeTarget::LoopLabel,
            "loopTextColor",
            "fill",
            ".loopText",
            "text",
            "loopText",
            Some("Loop Role"),
            false,
            false,
        ),
        (
            "loop-fill",
            ThemeTarget::LoopLabelBackground,
            "labelBoxBkgColor",
            "fill",
            ".labelBox",
            "polygon",
            "labelBox",
            None,
            false,
            false,
        ),
        (
            "loop-stroke",
            ThemeTarget::LoopLabelBackground,
            "labelBoxBorderColor",
            "stroke",
            ".labelBox",
            "polygon",
            "labelBox",
            None,
            false,
            true,
        ),
        (
            "sequence-number-label-fill",
            ThemeTarget::SequenceNumberLabel,
            "sequenceNumberColor",
            "fill",
            ".sequenceNumber",
            "text",
            "sequenceNumber",
            Some("1"),
            false,
            false,
        ),
    ];

    for (
        case,
        target,
        config_key,
        property,
        selector_suffix,
        dom_tag,
        dom_class,
        text_fragment,
        requires_tspan,
        stroke,
    ) in cases
    {
        for origin in ["site", "source"] {
            let patch = if stroke {
                ThemeStylePatch::default().with_stroke(
                    CanvasPaint::solid(TYPED_COLOR).expect("valid configured-owner stroke"),
                )
            } else {
                ThemeStylePatch::default().with_fill(
                    CanvasPaint::solid(TYPED_COLOR).expect("valid configured-owner fill"),
                )
            };
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                        ThemeRule::new(target, patch).for_family(DiagramFamilyId::SEQUENCE),
                    )),
                )
                .unwrap_or_else(|error| {
                    panic!("compile {origin} Sequence {case} owner test: {error}")
                });
            let config = serde_json::json!({
                "themeVariables": {(config_key): MERMAID_COLOR},
            });
            let (engine, source) = if origin == "site" {
                (
                    Engine::new().with_site_config(MermaidConfig::from_value(config)),
                    SOURCE.to_string(),
                )
            } else {
                (
                    legacy_init_theme_compat_engine(),
                    format!("%%{{init: {config}}}%%\n{SOURCE}"),
                )
            };
            let engine = merman_render::__private::install_parse_compatibility(&theme, engine);
            let parsed = engine
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap_or_else(|error| {
                    panic!("parse {origin} Sequence {case} owner source: {error}")
                })
                .unwrap_or_else(|| panic!("detect {origin} Sequence {case} owner source"));
            let session = RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .unwrap_or_else(|error| {
                    panic!("begin {origin} Sequence {case} owner session: {error}")
                });
            let diagram_id = format!("sequence-{origin}-{case}");
            let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
                .unwrap_or_else(|error| {
                    panic!("prepare {origin} Sequence {case} owner theme: {error}")
                })
                .render_svg(
                    &SvgRenderOptions {
                        diagram_id: Some(diagram_id.clone()),
                        ..SvgRenderOptions::default()
                    },
                    &SvgDebugOptions::default(),
                )
                .unwrap_or_else(|error| {
                    panic!("render {origin} Sequence {case} owner theme: {error}")
                });
            let document = roxmltree::Document::parse(rendered.svg()).unwrap_or_else(|error| {
                panic!("parse {origin} Sequence {case} owner SVG: {error}")
            });
            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .and_then(|node| node.text())
                .unwrap_or_else(|| panic!("missing {origin} Sequence {case} stylesheet"));
            let selector = format!("#{diagram_id} {selector_suffix}");
            let values = css_property_values_for_exact_selector(css, &selector, property);
            assert_eq!(
                values,
                [MERMAID_COLOR],
                "{origin} {case} must leave the explicit Mermaid owner as the only exact {selector} {property} terminal declaration: {css}"
            );

            let target_node = document.descendants().find(|node| {
                node.has_tag_name(dom_tag)
                    && node.attribute("class").is_some_and(|classes| {
                        classes
                            .split_ascii_whitespace()
                            .any(|class| class == dom_class)
                    })
                    && text_fragment.is_none_or(|fragment| {
                        node.descendants().any(|descendant| {
                            descendant
                                .text()
                                .is_some_and(|text| text.contains(fragment))
                        })
                    })
            });
            let target_node = target_node.unwrap_or_else(|| {
                panic!("missing {origin} Sequence {case} terminal DOM target {dom_tag}.{dom_class}")
            });
            if requires_tspan {
                assert!(
                    target_node
                        .children()
                        .any(|child| child.has_tag_name("tspan")),
                    "{origin} {case} terminal selector must bind an emitted actor tspan"
                );
            }
            drop(document);

            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            let partially_owned_role = target == ThemeTarget::LoopLabel
                && matches!(config_key, "labelTextColor" | "loopTextColor");
            assert_eq!(evidence.required_count(), 1, "{origin} {case}");
            assert_eq!(evidence.accounted_count(), 1, "{origin} {case}");
            assert_eq!(
                evidence.applied_count(),
                usize::from(partially_owned_role),
                "{origin} {case}"
            );
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(!partially_owned_role),
                "{origin} {case}"
            );
            assert_eq!(evidence.theme_residual_count(), 0, "{origin} {case}");
        }
    }
}

#[test]
fn sequence_actor_label_fill_ownership_is_terminal_surface_local() {
    const SOURCE: &str = r#"sequenceDiagram
box Team
participant Alice
participant Bob
end
Alice->>Bob: Message
"#;
    const MERMAID_COLOR: &str = "#fedcba";
    const TYPED_COLOR: &str = "#123456";

    for (case, actor_owned, box_owned) in [
        ("actor-only", true, false),
        ("box-only", false, true),
        ("both", true, true),
    ] {
        for origin in ["site", "source"] {
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(
                        ThemeRuleSet::default().with_rule(
                            ThemeRule::new(
                                ThemeTarget::ActorLabel,
                                ThemeStylePatch::default().with_fill(
                                    CanvasPaint::solid(TYPED_COLOR)
                                        .expect("valid Sequence ActorLabel fill"),
                                ),
                            )
                            .for_family(DiagramFamilyId::SEQUENCE),
                        ),
                    ),
                )
                .unwrap_or_else(|error| {
                    panic!("compile {origin} Sequence ActorLabel {case} theme: {error}")
                });
            let mut theme_variables = serde_json::Map::new();
            if actor_owned {
                theme_variables.insert(
                    "actorTextColor".to_string(),
                    serde_json::Value::String(MERMAID_COLOR.to_string()),
                );
            }
            if box_owned {
                theme_variables.insert(
                    "textColor".to_string(),
                    serde_json::Value::String(MERMAID_COLOR.to_string()),
                );
            }
            let config = serde_json::json!({"themeVariables": theme_variables});
            let (engine, source) = if origin == "site" {
                (
                    Engine::new().with_site_config(MermaidConfig::from_value(config)),
                    SOURCE.to_string(),
                )
            } else {
                (
                    legacy_init_theme_compat_engine(),
                    format!("%%{{init: {config}}}%%\n{SOURCE}"),
                )
            };
            let parsed = merman_render::__private::install_parse_compatibility(&theme, engine)
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap_or_else(|error| {
                    panic!("parse {origin} Sequence ActorLabel {case} source: {error}")
                })
                .unwrap_or_else(|| panic!("detect {origin} Sequence ActorLabel {case} source"));
            let session = RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .unwrap_or_else(|error| {
                    panic!("begin {origin} Sequence ActorLabel {case} session: {error}")
                });
            let diagram_id = format!("sequence-actor-label-{origin}-{case}");
            let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
                .unwrap_or_else(|error| {
                    panic!("prepare {origin} Sequence ActorLabel {case}: {error}")
                })
                .render_svg(
                    &SvgRenderOptions {
                        diagram_id: Some(diagram_id.clone()),
                        ..SvgRenderOptions::default()
                    },
                    &SvgDebugOptions::default(),
                )
                .unwrap_or_else(|error| {
                    panic!("render {origin} Sequence ActorLabel {case}: {error}")
                });
            let document = roxmltree::Document::parse(rendered.svg()).unwrap_or_else(|error| {
                panic!("parse {origin} Sequence ActorLabel {case} SVG: {error}")
            });
            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .and_then(|node| node.text())
                .expect("Sequence ActorLabel stylesheet");

            let participant_selector = format!("#{diagram_id} text.actor>tspan");
            let participant_values =
                css_property_values_for_exact_selector(css, &participant_selector, "fill");
            assert_eq!(
                participant_values.last().copied(),
                Some(if actor_owned {
                    MERMAID_COLOR
                } else {
                    TYPED_COLOR
                }),
                "{origin} {case} participant terminal owner: {css}"
            );

            let box_selector = format!("#{diagram_id} text.text");
            let box_values = css_property_values_for_exact_selector(css, &box_selector, "fill");
            if box_owned {
                assert!(
                    !box_values.contains(&TYPED_COLOR),
                    "{origin} {case} box title must not receive typed fill: {css}"
                );
                assert_eq!(
                    css_property_values_for_exact_selector(css, &format!("#{diagram_id}"), "fill")
                        .last()
                        .copied(),
                    Some(MERMAID_COLOR),
                    "{origin} {case} box title must inherit the explicit root text owner"
                );
            } else {
                assert_eq!(
                    box_values.last().copied(),
                    Some(TYPED_COLOR),
                    "{origin} {case} box title must retain the typed fill: {css}"
                );
            }

            for (class_name, text_fragment) in [("actor", "Alice"), ("text", "Team")] {
                assert!(
                    document.descendants().any(|node| {
                        node.has_tag_name("text")
                            && node.attribute("class").is_some_and(|classes| {
                                classes
                                    .split_ascii_whitespace()
                                    .any(|class| class == class_name)
                            })
                            && node.descendants().any(|descendant| {
                                descendant
                                    .text()
                                    .is_some_and(|text| text.contains(text_fragment))
                            })
                    }),
                    "missing {origin} {case} Sequence {class_name} occurrence"
                );
            }
            drop(document);

            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1, "{origin} {case}");
            assert_eq!(evidence.accounted_count(), 1, "{origin} {case}");
            assert_eq!(
                evidence.applied_count(),
                usize::from(!(actor_owned && box_owned)),
                "{origin} {case}"
            );
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(actor_owned && box_owned),
                "{origin} {case}"
            );
            assert_eq!(evidence.theme_residual_count(), 0, "{origin} {case}");
        }
    }
}

#[test]
fn sequence_loop_label_fill_ownership_is_terminal_surface_local() {
    const SOURCE: &str = r#"sequenceDiagram
participant Alice
participant Bob
alt Primary
Alice->>Bob: Message
else Secondary
Bob-->>Alice: Reply
end
"#;
    const MERMAID_COLOR: &str = "#fedcba";
    const TYPED_COLOR: &str = "#123456";

    for (case, keyword_owned, title_owned) in [
        ("keyword-only", true, false),
        ("titles-only", false, true),
        ("both", true, true),
    ] {
        for origin in ["site", "source"] {
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(
                        ThemeRuleSet::default().with_rule(
                            ThemeRule::new(
                                ThemeTarget::LoopLabel,
                                ThemeStylePatch::default().with_fill(
                                    CanvasPaint::solid(TYPED_COLOR)
                                        .expect("valid Sequence LoopLabel fill"),
                                ),
                            )
                            .for_family(DiagramFamilyId::SEQUENCE),
                        ),
                    ),
                )
                .unwrap_or_else(|error| {
                    panic!("compile {origin} Sequence LoopLabel {case} theme: {error}")
                });
            let mut theme_variables = serde_json::Map::new();
            if keyword_owned {
                theme_variables.insert(
                    "labelTextColor".to_string(),
                    serde_json::Value::String(MERMAID_COLOR.to_string()),
                );
            }
            if title_owned {
                theme_variables.insert(
                    "loopTextColor".to_string(),
                    serde_json::Value::String(MERMAID_COLOR.to_string()),
                );
            }
            let config = serde_json::json!({"themeVariables": theme_variables});
            let (engine, source) = if origin == "site" {
                (
                    Engine::new().with_site_config(MermaidConfig::from_value(config)),
                    SOURCE.to_string(),
                )
            } else {
                (
                    legacy_init_theme_compat_engine(),
                    format!("%%{{init: {config}}}%%\n{SOURCE}"),
                )
            };
            let parsed = merman_render::__private::install_parse_compatibility(&theme, engine)
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap_or_else(|error| {
                    panic!("parse {origin} Sequence LoopLabel {case} source: {error}")
                })
                .unwrap_or_else(|| panic!("detect {origin} Sequence LoopLabel {case} source"));
            let session = RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .unwrap_or_else(|error| {
                    panic!("begin {origin} Sequence LoopLabel {case} session: {error}")
                });
            let diagram_id = format!("sequence-loop-label-{origin}-{case}");
            let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
                .unwrap_or_else(|error| {
                    panic!("prepare {origin} Sequence LoopLabel {case}: {error}")
                })
                .render_svg(
                    &SvgRenderOptions {
                        diagram_id: Some(diagram_id.clone()),
                        ..SvgRenderOptions::default()
                    },
                    &SvgDebugOptions::default(),
                )
                .unwrap_or_else(|error| {
                    panic!("render {origin} Sequence LoopLabel {case}: {error}")
                });
            let document = roxmltree::Document::parse(rendered.svg()).unwrap_or_else(|error| {
                panic!("parse {origin} Sequence LoopLabel {case} SVG: {error}")
            });
            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .and_then(|node| node.text())
                .expect("Sequence LoopLabel stylesheet");

            for (selector_suffix, owned) in [
                (".labelText", keyword_owned),
                (".loopText", title_owned),
                (".sectionTitle", title_owned),
            ] {
                let selector = format!("#{diagram_id} {selector_suffix}");
                let values = css_property_values_for_exact_selector(css, &selector, "fill");
                assert_eq!(
                    values.last().copied(),
                    Some(if owned { MERMAID_COLOR } else { TYPED_COLOR }),
                    "{origin} {case} {selector_suffix} terminal owner: {css}"
                );
            }

            for (class_name, text_fragment) in [
                ("labelText", "alt"),
                ("loopText", "Primary"),
                ("sectionTitle", "Secondary"),
            ] {
                assert!(
                    document.descendants().any(|node| {
                        node.has_tag_name("text")
                            && node.attribute("class").is_some_and(|classes| {
                                classes
                                    .split_ascii_whitespace()
                                    .any(|class| class == class_name)
                            })
                            && node.descendants().any(|descendant| {
                                descendant
                                    .text()
                                    .is_some_and(|text| text.contains(text_fragment))
                            })
                    }),
                    "missing {origin} {case} Sequence {class_name} occurrence"
                );
            }
            drop(document);

            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1, "{origin} {case}");
            assert_eq!(evidence.accounted_count(), 1, "{origin} {case}");
            assert_eq!(
                evidence.applied_count(),
                usize::from(!(keyword_owned && title_owned)),
                "{origin} {case}"
            );
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(keyword_owned && title_owned),
                "{origin} {case}"
            );
            assert_eq!(evidence.theme_residual_count(), 0, "{origin} {case}");
        }
    }
}

#[test]
fn sequence_role_paints_yield_to_derived_mermaid_surface_owners_from_site_and_source() {
    const SOURCE: &str = r#"sequenceDiagram
autonumber
box Team
participant Alice
participant Bob
end
Alice->>Bob: Message Role
activate Bob
Bob-->>Alice: Active Reply
deactivate Bob
Note over Alice,Bob: Note Role
loop Loop Role
Bob-->>Alice: Reply Role
end"#;
    const MERMAID_COLOR: &str = "#22c55e";
    const TYPED_COLOR: &str = "#123456";

    let mut cases = vec![
        ("default", ThemeTarget::ActorLabel, "textColor", false, true),
        (
            "default",
            ThemeTarget::MessageLabel,
            "textColor",
            false,
            true,
        ),
        (
            "default",
            ThemeTarget::NoteLabel,
            "actorTextColor",
            false,
            true,
        ),
        (
            "default",
            ThemeTarget::LoopLabel,
            "actorTextColor",
            false,
            true,
        ),
        (
            "base",
            ThemeTarget::LoopLabelBackground,
            "mainBkg",
            false,
            true,
        ),
        (
            "base",
            ThemeTarget::Activation,
            "secondaryColor",
            true,
            true,
        ),
        (
            "base",
            ThemeTarget::SequenceNumberLabel,
            "lineColor",
            false,
            true,
        ),
        (
            "dark",
            ThemeTarget::LoopLabelBackground,
            "border1",
            true,
            true,
        ),
        ("forest", ThemeTarget::Actor, "mainBkg", false, true),
        ("forest", ThemeTarget::Lifeline, "mainBkg", true, true),
        (
            "forest",
            ThemeTarget::LoopLabelBackground,
            "mainBkg",
            false,
            true,
        ),
        (
            "forest",
            ThemeTarget::LoopLabel,
            "actorTextColor",
            false,
            true,
        ),
        ("neutral", ThemeTarget::Actor, "mainBkg", false, true),
        ("neutral", ThemeTarget::Lifeline, "border1", true, true),
        (
            "neutral",
            ThemeTarget::LoopLabelBackground,
            "mainBkg",
            false,
            true,
        ),
        ("default", ThemeTarget::Lifeline, "actorBorder", true, false),
        ("dark", ThemeTarget::Lifeline, "actorBorder", true, false),
        (
            "default",
            ThemeTarget::LoopLabelBackground,
            "actorBkg",
            false,
            false,
        ),
        (
            "dark",
            ThemeTarget::LoopLabelBackground,
            "actorBkg",
            false,
            false,
        ),
    ];
    for (theme_id, primary_owns_activation, primary_border_owns_loop, text_owns_message) in [
        ("neo", true, true, true),
        ("neo-dark", false, true, true),
        ("redux", true, false, true),
        ("redux-dark", false, false, false),
        ("redux-color", true, false, true),
        ("redux-dark-color", false, false, false),
    ] {
        cases.extend([
            (
                theme_id,
                ThemeTarget::LoopLabelBackground,
                "mainBkg",
                false,
                true,
            ),
            (
                theme_id,
                ThemeTarget::Activation,
                "secondaryColor",
                false,
                true,
            ),
            (
                theme_id,
                ThemeTarget::Activation,
                "secondaryColor",
                true,
                true,
            ),
            (
                theme_id,
                ThemeTarget::Activation,
                "primaryColor",
                false,
                primary_owns_activation,
            ),
            (
                theme_id,
                ThemeTarget::LoopLabel,
                "primaryTextColor",
                false,
                true,
            ),
            (
                theme_id,
                ThemeTarget::MessageLabel,
                "textColor",
                false,
                true,
            ),
            (
                theme_id,
                ThemeTarget::Message,
                "textColor",
                true,
                text_owns_message,
            ),
            (
                theme_id,
                ThemeTarget::LoopLabelBackground,
                "primaryBorderColor",
                true,
                primary_border_owns_loop,
            ),
            (
                theme_id,
                ThemeTarget::SequenceNumberLabel,
                "lineColor",
                false,
                true,
            ),
            (
                theme_id,
                ThemeTarget::SequenceNumberLabel,
                "background",
                false,
                true,
            ),
            (
                theme_id,
                ThemeTarget::LoopLabelBackground,
                "primaryColor",
                false,
                false,
            ),
            (
                theme_id,
                ThemeTarget::LoopLabelBackground,
                "nodeBkg",
                false,
                false,
            ),
        ]);
    }

    for (theme_id, target, config_key, stroke, mermaid_owns_terminal) in cases {
        for origin in ["site", "source"] {
            let partially_owned_terminal = theme_id == "default"
                && target == ThemeTarget::ActorLabel
                && config_key == "textColor";
            let terminal_key = match target {
                ThemeTarget::Actor => "actorBkg",
                ThemeTarget::ActorLabel => "actorTextColor",
                ThemeTarget::Lifeline => "actorLineColor",
                ThemeTarget::MessageLabel => "signalTextColor",
                ThemeTarget::NoteLabel => "noteTextColor",
                ThemeTarget::LoopLabel => "loopTextColor",
                ThemeTarget::LoopLabelBackground if stroke => "labelBoxBorderColor",
                ThemeTarget::LoopLabelBackground => "labelBoxBkgColor",
                ThemeTarget::Activation if stroke => "activationBorderColor",
                ThemeTarget::Activation => "activationBkgColor",
                ThemeTarget::Message => "signalColor",
                ThemeTarget::SequenceNumberLabel => "sequenceNumberColor",
                _ => panic!("unexpected Sequence terminal target: {target:?}"),
            };
            let patch = if stroke {
                ThemeStylePatch::default().with_stroke(
                    CanvasPaint::solid(TYPED_COLOR).expect("valid typed Sequence stroke"),
                )
            } else {
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid(TYPED_COLOR).expect("valid typed Sequence fill"))
            };
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                        ThemeRule::new(target, patch).for_family(DiagramFamilyId::SEQUENCE),
                    )),
                )
                .unwrap_or_else(|error| {
                    panic!("compile {origin} {theme_id} {config_key} Sequence theme: {error}")
                });
            let config = serde_json::json!({
                "theme": theme_id,
                "themeVariables": {(config_key): MERMAID_COLOR},
            });
            let (engine, source) = if origin == "site" {
                (
                    Engine::new().with_site_config(MermaidConfig::from_value(config)),
                    SOURCE.to_string(),
                )
            } else {
                (
                    legacy_init_theme_compat_engine(),
                    format!("%%{{init: {config}}}%%\n{SOURCE}"),
                )
            };
            let parsed = merman_render::__private::install_parse_compatibility(&theme, engine)
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap_or_else(|error| {
                    panic!("parse {origin} {theme_id} {config_key} Sequence source: {error}")
                })
                .unwrap_or_else(|| {
                    panic!("detect {origin} {theme_id} {config_key} Sequence source")
                });
            let expected_mermaid_color = parsed
                .metadata()
                .effective_config
                .as_value()["themeVariables"][terminal_key]
                .as_str()
                .unwrap_or_else(|| {
                    panic!(
                        "{origin} {theme_id} {config_key} did not resolve terminal themeVariables.{terminal_key}"
                    )
                })
                .to_owned();
            let session = RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .unwrap_or_else(|error| {
                    panic!("begin strict {origin} {theme_id} {config_key} session: {error}")
                });
            let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
                .unwrap_or_else(|error| {
                    panic!("prepare {origin} {theme_id} {config_key} Sequence theme: {error}")
                })
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap_or_else(|error| {
                    panic!("render strict {origin} {theme_id} {config_key} Sequence theme: {error}")
                });

            if mermaid_owns_terminal {
                assert!(
                    rendered.svg().contains(&expected_mermaid_color),
                    "{origin} {theme_id} {config_key} must preserve effective themeVariables.{terminal_key}={expected_mermaid_color}: {}",
                    rendered.svg()
                );
                if partially_owned_terminal {
                    assert!(
                        rendered.svg().contains(TYPED_COLOR),
                        "typed paint must remain on the unowned participant surface for {origin} {theme_id} {config_key}: {}",
                        rendered.svg()
                    );
                } else {
                    assert!(
                        !rendered.svg().contains(TYPED_COLOR),
                        "typed paint must yield to {origin} {theme_id} {config_key}: {}",
                        rendered.svg()
                    );
                }
            } else {
                assert!(
                    rendered.svg().contains(TYPED_COLOR),
                    "typed paint must retain the unowned {origin} {theme_id} {config_key} route: {}",
                    rendered.svg()
                );
            }
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(
                evidence.required_count(),
                1,
                "{origin} {theme_id} {config_key}"
            );
            assert_eq!(
                evidence.accounted_count(),
                1,
                "{origin} {theme_id} {config_key}"
            );
            assert_eq!(
                evidence.applied_count(),
                usize::from(!mermaid_owns_terminal || partially_owned_terminal),
                "{origin} {theme_id} {config_key}"
            );
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(mermaid_owns_terminal && !partially_owned_terminal),
                "{origin} {theme_id} {config_key}"
            );
            assert_eq!(
                evidence.theme_residual_count(),
                0,
                "{origin} {theme_id} {config_key}"
            );
        }
    }
}

#[test]
fn sequence_role_paints_yield_to_neutral_site_text_derivations() {
    const SOURCE: &str = r#"sequenceDiagram
participant Alice
participant Bob
Alice->>Bob: Message Role
loop Loop Role
Bob-->>Alice: Reply Role
end"#;
    const MERMAID_COLOR: &str = "#22c55e";
    const TYPED_COLOR: &str = "#123456";

    for (case, target, terminal_key, stroke) in [
        ("loop-label", ThemeTarget::LoopLabel, "loopTextColor", false),
        ("message", ThemeTarget::Message, "signalColor", true),
    ] {
        let patch = if stroke {
            ThemeStylePatch::default().with_stroke(
                CanvasPaint::solid(TYPED_COLOR).expect("valid typed Neutral Sequence stroke"),
            )
        } else {
            ThemeStylePatch::default().with_fill(
                CanvasPaint::solid(TYPED_COLOR).expect("valid typed Neutral Sequence fill"),
            )
        };
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(target, patch).for_family(DiagramFamilyId::SEQUENCE),
                )),
            )
            .unwrap_or_else(|error| panic!("compile Neutral site text {case} theme: {error}"));
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "theme": "neutral",
            "themeVariables": { "text": MERMAID_COLOR },
        })));
        let parsed = merman_render::__private::install_parse_compatibility(&theme, engine)
            .parse_diagram_for_render_model_sync(SOURCE, ParseOptions::strict())
            .unwrap_or_else(|error| panic!("parse Neutral site text {case} source: {error}"))
            .unwrap_or_else(|| panic!("detect Neutral site text {case} source"));
        let terminal_path = format!("themeVariables.{terminal_key}");
        assert_eq!(
            parsed.metadata().effective_config.get_str(&terminal_path),
            Some(MERMAID_COLOR),
            "Neutral site text has incorrect derived {terminal_key} value"
        );
        assert!(
            merman_core::__private::config_path_overrides_typed_default(
                &parsed.metadata().effective_config,
                &terminal_path,
            ),
            "Neutral site text must own derived {terminal_key}"
        );

        let session = RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .unwrap_or_else(|error| panic!("begin strict Neutral site text {case}: {error}"));
        let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
            .unwrap_or_else(|error| panic!("prepare Neutral site text {case}: {error}"))
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap_or_else(|error| panic!("render strict Neutral site text {case}: {error}"));
        assert!(
            rendered.svg().contains(MERMAID_COLOR),
            "Neutral site text must reach the {case} terminal: {}",
            rendered.svg()
        );
        assert!(
            !rendered.svg().contains(TYPED_COLOR),
            "typed {case} paint must yield to Neutral site text: {}",
            rendered.svg()
        );

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1, "{case}");
        assert_eq!(evidence.accounted_count(), 1, "{case}");
        assert_eq!(evidence.applied_count(), 0, "{case}");
        assert_eq!(evidence.not_applicable_count(), 1, "{case}");
        assert_eq!(evidence.theme_residual_count(), 0, "{case}");
    }
}

#[cfg(feature = "math")]
#[test]
fn sequence_math_role_paint_is_sealed_by_exact_prepared_terminal_occurrences() {
    for (case, target, source, expected_math_occurrences) in [
        (
            "participant",
            ThemeTarget::ActorLabel,
            "sequenceDiagram\nparticipant A as $$x^2$$\nparticipant B\nA->>B: ok\n",
            2,
        ),
        (
            "actor-man",
            ThemeTarget::ActorLabel,
            "sequenceDiagram\nactor A as $$x^2$$\nparticipant B\nA->>B: ok\n",
            2,
        ),
        (
            "message",
            ThemeTarget::MessageLabel,
            "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: $$x^2$$\n",
            1,
        ),
        (
            "note",
            ThemeTarget::NoteLabel,
            "sequenceDiagram\nparticipant A\nparticipant B\nNote over A,B: $$x^2$$\n",
            1,
        ),
        (
            "control-primary-and-section",
            ThemeTarget::LoopLabel,
            "sequenceDiagram\nparticipant A\nparticipant B\nalt $$x^2$$\nA->>B: ok\nelse $$y^2$$\nB-->>A: no\nend\n",
            2,
        ),
    ] {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            target,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#22d3ee")
                                    .expect("valid Sequence math role fill"),
                            ),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
            )
            .unwrap_or_else(|error| panic!("compile Sequence {case} math theme: {error}"));
        let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap_or_else(|error| panic!("parse Sequence {case} math source: {error}"))
            .unwrap_or_else(|| panic!("detect Sequence {case} math source"));
        let session = RenderEnvironment::deterministic()
            .with_compiled_math_renderer()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .unwrap_or_else(|error| panic!("begin strict Sequence {case} math session: {error}"));
        let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
            .unwrap_or_else(|error| panic!("prepare Sequence {case} math: {error}"))
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap_or_else(|error| panic!("render strict Sequence {case} math: {error}"));
        assert_eq!(
            rendered
                .svg()
                .matches("class=\"merman-prepared-math\"")
                .count(),
            expected_math_occurrences,
            "{case}: {}",
            rendered.svg()
        );
        let document = roxmltree::Document::parse(rendered.svg())
            .unwrap_or_else(|error| panic!("parse Sequence {case} math SVG: {error}"));
        for literal_fallback in document
            .descendants()
            .filter(|node| node.is_text() && node.text().is_some_and(|text| text.contains("$$")))
        {
            assert!(
                literal_fallback.ancestors().any(|ancestor| {
                    ancestor.has_tag_name("switch")
                        && ancestor.attribute("data-merman-prepared-math-switch") == Some("v1")
                }),
                "{case}: raw math text may only survive inside the renderer-owned switch fallback: {}",
                rendered.svg()
            );
        }
        drop(document);

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1, "{case}");
        assert_eq!(evidence.accounted_count(), 1, "{case}");
        assert_eq!(evidence.applied_count(), 1, "{case}");
        assert_eq!(evidence.theme_residual_count(), 0, "{case}");
    }
}

#[test]
fn sequence_explicit_config_typography_reaches_measurement_and_terminal_writers() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "fontFamily": "Courier New",
        "fontSize": 23,
        "fontWeight": 700,
        "themeVariables": {"fontFamily": "Excalifont"},
    })));
    let source = r#"sequenceDiagram
autonumber
box Config Team
participant Alice as Config Actor
participant Bob
end
Alice->>Bob: Config Message
Note over Alice,Bob: Config Note
loop Config Loop
Bob-->>Alice: Reply
end"#;
    let measurement_parsed = parse_sequence_for_render(&engine, source);
    let host = Arc::new(RecordingSequenceHost::new(SequenceHostResponse::Missing));
    let measurement_identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("sequence-config-typography").expect("valid profile id"),
        "1",
    )
    .expect("valid measurement profile identity");
    let measurement_policy = TextMeasurementPolicy::host_display(
        measurement_identity,
        host.clone(),
        TextMeasurementPhase::ALL,
    );
    let measurement_session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(measurement_policy)
        .begin_session()
        .expect("begin configured Sequence measurement session");
    let _measurement_artifact = family::prepare(
        measurement_parsed,
        &LayoutOptions::default(),
        measurement_session,
    )
    .expect("prepare configured Sequence measurement artifact");
    let requests = host.snapshot();
    for text_fragment in [
        "Config Team",
        "Config Actor",
        "Config Message",
        "Config Note",
        "Config Loop",
    ] {
        assert!(
            requests.iter().any(|exchange| {
                let request = &exchange.request;
                request.text.contains(text_fragment)
                    && request.font_family.as_deref() == Some("Courier New")
                    && request.font_size_bits == f64::to_bits(23.0)
                    && request.font_weight.as_deref() == Some("700")
            }),
            "layout measurement must consume explicit config typography for {text_fragment:?}"
        );
    }

    let rendered = family::prepare(
        parse_sequence_for_render(&engine, source),
        &LayoutOptions::default(),
        RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin configured Sequence render session"),
    )
    .expect("prepare configured Sequence render")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("render configured Sequence SVG");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid config-only SVG");
    for (class_name, text_fragment) in [
        ("text", "Config Team"),
        ("actor-box", "Config Actor"),
        ("messageText", "Config Message"),
        ("noteText", "Config Note"),
        ("loopText", "Config Loop"),
    ] {
        assert_sequence_role_text_style(
            &document,
            class_name,
            text_fragment,
            "Courier New",
            "23px",
            "700",
            "normal",
        );
    }

    let sequence_number = document
        .descendants()
        .find(|node| node.has_tag_name("text") && node.attribute("class") == Some("sequenceNumber"))
        .expect("configured autonumber terminal label");
    let inline = sequence_number
        .attribute("style")
        .expect("configured autonumber role typography style");
    assert_eq!(
        inline_style_value(inline, "font-family"),
        Some("Courier New")
    );
    assert_eq!(inline_style_value(inline, "font-weight"), Some("700"));
    assert_eq!(inline_style_value(inline, "font-style"), Some("normal"));
    assert_eq!(sequence_number.attribute("font-size"), Some("12px"));
}

#[test]
fn sequence_cssom_rejected_root_font_keeps_measurement_and_terminal_ownership_separate() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {"fontFamily": "Excalifont"},
    })));
    let source = r#"sequenceDiagram
participant Alice as CSSOM Actor
participant Bob
Alice->>Bob: CSSOM Message
Note over Alice,Bob: CSSOM Note
loop CSSOM Loop
Bob-->>Alice: Reply
end"#;
    let host = Arc::new(RecordingSequenceHost::new(SequenceHostResponse::Missing));
    let measurement_identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("sequence-cssom-font-owner").expect("valid profile id"),
        "1",
    )
    .expect("valid measurement profile identity");
    let measurement_policy = TextMeasurementPolicy::host_display(
        measurement_identity,
        host.clone(),
        TextMeasurementPhase::ALL,
    );
    let measurement_session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(measurement_policy)
        .begin_session()
        .expect("begin Sequence CSSOM font measurement session");
    let _measurement_artifact = family::prepare(
        parse_sequence_for_render(&engine, source),
        &LayoutOptions::default(),
        measurement_session,
    )
    .expect("prepare Sequence CSSOM font measurement artifact");
    let requests = host.snapshot();

    for text_fragment in ["CSSOM Actor", "CSSOM Message", "CSSOM Note", "CSSOM Loop"] {
        let matching = requests
            .iter()
            .filter(|exchange| {
                exchange.request.text.contains(text_fragment)
                    && exchange.request.operation
                        == TextMeasurementOperation::MermaidCalculateTextDimensions
            })
            .collect::<Vec<_>>();
        assert!(
            !matching.is_empty(),
            "expected layout measurement requests for {text_fragment:?}"
        );
        let configured = matching
            .iter()
            .filter(|exchange| exchange.request.font_family.as_deref() != Some("sans-serif"))
            .collect::<Vec<_>>();
        assert!(
            !configured.is_empty()
                && configured.iter().all(|exchange| {
                    exchange.request.font_family.as_deref()
                        == Some("\"trebuchet ms\", verdana, arial, sans-serif;")
                }),
            "Mermaid calculateTextDimensions must preserve the source declaration and its CSSOM fallback for {text_fragment:?}: {matching:#?}"
        );
    }

    for (text_fragment, operations) in [
        (
            "CSSOM Message",
            [
                TextMeasurementOperation::RawBBoxWidth,
                TextMeasurementOperation::RawBBoxHeight,
            ],
        ),
        (
            "CSSOM Note",
            [
                TextMeasurementOperation::TspanBBoxWidth,
                TextMeasurementOperation::TspanBBoxHeight,
            ],
        ),
    ] {
        for operation in operations {
            let matching = requests
                .iter()
                .filter(|exchange| {
                    exchange.request.text.contains(text_fragment)
                        && exchange.request.operation == operation
                })
                .collect::<Vec<_>>();
            assert!(
                !matching.is_empty()
                    && matching.iter().all(|exchange| {
                        exchange.request.font_family.as_deref() == Some("Excalifont")
                    }),
                "terminal {operation:?} requests must use the CSSOM-effective inherited font for {text_fragment:?}: {matching:#?}"
            );
        }
    }
}

#[test]
fn sequence_theme_variable_note_weight_does_not_override_typed_note_weight() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::NoteLabel,
                        ThemeStylePatch {
                            typography: ThemeTextStylePatch {
                                font_weight: Specified::Value(700),
                                ..ThemeTextStylePatch::default()
                            },
                            ..ThemeStylePatch::default()
                        },
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence NoteLabel weight theme");
    let source = r#"%%{init: {"themeVariables": {"noteFontWeight": 600}}}%%
sequenceDiagram
participant Alice
participant Bob
Note over Alice,Bob: Config Note Weight"#;
    let engine = merman_render::__private::install_parse_compatibility(
        &theme,
        legacy_init_theme_compat_engine(),
    );
    let host = Arc::new(RecordingSequenceHost::new(SequenceHostResponse::Missing));
    let measurement_identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("sequence-note-theme-variable-weight").expect("valid profile id"),
        "1",
    )
    .expect("valid measurement profile identity");
    let measurement_policy = TextMeasurementPolicy::host_display(
        measurement_identity,
        host.clone(),
        TextMeasurementPhase::ALL,
    );
    let measurement_parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse measured Sequence note source")
        .expect("detect measured Sequence note source");
    assert_eq!(
        measurement_parsed.metadata().effective_config.as_value()["themeVariables"]["noteFontWeight"],
        serde_json::json!(600),
        "the explicit legacy init value must survive the configured Mermaid secure allowlist"
    );
    assert!(
        merman_core::__private::config_path_overrides_typed_default(
            &measurement_parsed.metadata().effective_config,
            "themeVariables.noteFontWeight",
        ),
        "the legacy init value remains authored config even though Sequence no longer consumes it"
    );
    let measurement_session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(measurement_policy)
        .begin_session_with_theme(&theme)
        .expect("begin measured Sequence note session");
    let _measurement_artifact = family::prepare(
        measurement_parsed,
        &LayoutOptions::default(),
        measurement_session,
    )
    .expect("prepare measured Sequence note");
    assert!(
        host.snapshot().iter().any(|exchange| {
            exchange.request.text.contains("Config Note Weight")
                && exchange.request.font_weight.as_deref() == Some("700")
        }),
        "themeVariables.noteFontWeight must not override typed NoteLabel layout measurement"
    );

    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse configured Sequence note source")
        .expect("detect configured Sequence note source");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin configured Sequence note session");
    let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare configured Sequence note")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render configured Sequence note");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid configured note SVG");
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Sequence stylesheet");
    let selector = "#merman .noteText,#merman .noteText>tspan{";
    let role_rule = css
        .rfind(selector)
        .and_then(|start| css[start + selector.len()..].split_once('}'))
        .map(|(declarations, _)| declarations)
        .expect("post-legacy NoteLabel typography rule");
    assert!(
        role_rule.contains("font-weight:700"),
        "typed NoteLabel weight must reach the note and its child tspans: {role_rule}"
    );

    let note = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.attribute("class") == Some("noteText")
                && node
                    .descendants()
                    .filter_map(|descendant| descendant.text())
                    .any(|text| text.contains("Config Note Weight"))
        })
        .expect("terminal configured note label");
    assert_eq!(
        note.attribute("style")
            .and_then(|style| inline_style_value(style, "font-weight")),
        Some("700")
    );
}

#[test]
fn sequence_typed_message_css_escapes_the_diagram_id_selector() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Message,
                        ThemeStylePatch::default().with_stroke(
                            CanvasPaint::solid("#2563eb").expect("valid Message stroke"),
                        ),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile Sequence Message theme");
    let engine = merman_render::__private::install_parse_compatibility(&theme, Engine::new());
    let parsed = engine
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
            ParseOptions::strict(),
        )
        .expect("parse Sequence Message source")
        .expect("detect Sequence Message source");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict Sequence Message session");
    let options = SvgRenderOptions {
        diagram_id: Some("seq:prod".to_string()),
        ..SvgRenderOptions::default()
    };

    let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare Sequence Message theme")
        .render_svg(&options, &SvgDebugOptions::default())
        .expect("render Sequence Message theme");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Sequence SVG");
    assert_eq!(document.root_element().attribute("id"), Some("seq-prod"));
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Sequence stylesheet");
    assert!(
        css.contains(r"#seq-prod .messageLine0,#seq-prod .messageLine1{stroke:#2563eb;}"),
        "{css}"
    );
    assert!(!css.contains("#seq:prod"), "{css}");
}

#[test]
fn sequence_number_label_fill_seals_actual_autonumber_text() {
    let source = r#"sequenceDiagram
autonumber
participant Alice
participant Bob
Alice->>Bob: First
Bob-->>Alice: Second"#;

    for (case, fill, expected_css) in [
        (
            "solid",
            CanvasPaint::solid("#123456").expect("valid Sequence number fill"),
            "#123456",
        ),
        ("transparent", CanvasPaint::Transparent, "transparent"),
    ] {
        let theme = sequence_number_label_theme(fill);
        let engine = merman_render::__private::install_parse_compatibility(&theme, Engine::new());
        let parsed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse themed Sequence autonumber source")
            .expect("detect themed Sequence autonumber source");
        let session = RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin strict Sequence autonumber session");
        let diagram_id = format!("sequence-number-{case}");
        let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
            .expect("prepare themed Sequence autonumber")
            .render_svg(
                &SvgRenderOptions {
                    diagram_id: Some(diagram_id.clone()),
                    ..SvgRenderOptions::default()
                },
                &SvgDebugOptions::default(),
            )
            .expect("typed Sequence number label must be portable");
        let svg = rendered.svg().to_owned();
        let document = roxmltree::Document::parse(&svg).expect("valid Sequence autonumber SVG");
        let css = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("Sequence stylesheet");
        let terminal_rule = format!(
            "#{diagram_id} .sequenceNumber,#{diagram_id} .sequenceNumber>tspan{{fill:{expected_css};}}"
        );
        assert!(
            css.contains(&terminal_rule),
            "missing final Sequence number writer rule: {css}"
        );
        assert_eq!(css.matches(&terminal_rule).count(), 1);
        assert!(!css.contains(&format!("#{diagram_id} .sequenceNumber{{fill:")));
        let numbers = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("text") && node.attribute("class") == Some("sequenceNumber")
            })
            .filter_map(|node| node.text())
            .collect::<Vec<_>>();
        assert_eq!(numbers, ["1", "2"]);
        drop(document);

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn sequence_number_label_is_not_applicable_without_autonumber() {
    let theme = sequence_number_label_theme(
        CanvasPaint::solid("#123456").expect("valid Sequence number fill"),
    );
    let engine = merman_render::__private::install_parse_compatibility(&theme, Engine::new());
    let parsed = engine
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
            ParseOptions::strict(),
        )
        .expect("parse Sequence source without autonumber")
        .expect("detect Sequence source without autonumber");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict Sequence session without autonumber");
    let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare Sequence source without autonumber")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("an absent Sequence number label must remain portable");
    assert!(!rendered.svg().contains(r#"class="sequenceNumber""#));

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn sequence_number_label_unsupported_selectors_fail_closed() {
    let rule = || {
        ThemeRule::new(
            ThemeTarget::SequenceNumberLabel,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#123456").expect("valid Sequence number fill")),
        )
    };
    for (case, rule) in [
        (
            "explicit-default",
            rule().with_variant(ThemeVariant::Default),
        ),
        (
            "ordinal",
            rule().with_ordinal(OrdinalSelector::exact(1).expect("valid ordinal")),
        ),
    ] {
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(rule.for_family(DiagramFamilyId::SEQUENCE)),
            ))
            .unwrap_or_else(|error| panic!("compile {case} Sequence number theme: {error}"));
        let engine = merman_render::__private::install_parse_compatibility(&theme, Engine::new());
        let parsed = engine
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nautonumber\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
                ParseOptions::strict(),
            )
            .unwrap_or_else(|error| panic!("parse {case} Sequence number source: {error}"))
            .unwrap_or_else(|| panic!("detect {case} Sequence number source"));
        let session = RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .unwrap_or_else(|error| panic!("begin strict {case} Sequence number session: {error}"));
        let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
            .unwrap_or_else(|error| panic!("prepare {case} Sequence number theme: {error}"));
        let error = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .err()
            .unwrap_or_else(|| panic!("{case} Sequence number selector must remain unsupported"));
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::SEQUENCE, 1)),
            "{case}"
        );
        assert_eq!(error.incomplete_family_theme(), None, "{case}");
    }
}

#[test]
fn sequence_number_label_does_not_emit_a_superseded_direct_fill() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::SequenceNumberLabel,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#123456")
                                    .expect("valid direct Sequence number fill"),
                            ),
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    )
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::SequenceNumberLabel,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#fedcba")
                                    .expect("valid explicit-default Sequence number fill"),
                            ),
                        )
                        .with_variant(ThemeVariant::Default)
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
            ),
        )
        .expect("compile mixed Sequence number theme");
    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync(
            "sequenceDiagram\nautonumber\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n",
            ParseOptions::strict(),
        )
        .expect("parse mixed Sequence number source")
        .expect("detect mixed Sequence number source");
    let session = RenderEnvironment::deterministic()
        .begin_session_with_theme(&theme)
        .expect("begin mixed Sequence number session");
    let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare mixed Sequence number theme")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render mixed Sequence number theme in best-effort mode");

    for fill in ["#123456", "#fedcba"] {
        assert!(
            !rendered.svg().contains(&format!(
                "#merman .sequenceNumber,#merman .sequenceNumber>tspan{{fill:{fill};}}"
            )),
            "an unsupported final winner must not acquire the typed Sequence number terminal: {}",
            rendered.svg()
        );
    }
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn sequence_clear_typography_uses_the_computed_family_base_everywhere() {
    let base_typography = ThemeTextStyle::default()
        .with_font_size_px(21.0)
        .expect("valid Sequence family base font size");
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default()
                        .with_family_style(DiagramFamilyId::SEQUENCE, base_typography),
                )
                .with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::MessageLabel,
                            ThemeStylePatch {
                                typography: ThemeTextStylePatch {
                                    font_size_px: Specified::Clear,
                                    ..ThemeTextStylePatch::default()
                                },
                                ..ThemeStylePatch::default()
                            },
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                ),
        )
        .expect("compile Sequence MessageLabel clear theme");
    let source = r#"sequenceDiagram
participant Alice
participant Bob
Alice->>Bob: Clear Message"#;
    let engine = merman_render::__private::install_parse_compatibility(&theme, Engine::new());
    let host = Arc::new(RecordingSequenceHost::new(SequenceHostResponse::Missing));
    let measurement_identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("sequence-computed-clear-typography").expect("valid profile id"),
        "1",
    )
    .expect("valid measurement profile identity");
    let measurement_policy = TextMeasurementPolicy::host_display(
        measurement_identity,
        host.clone(),
        TextMeasurementPhase::ALL,
    );
    let measurement_session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(measurement_policy)
        .begin_session_with_theme(&theme)
        .expect("begin Sequence clear measurement session");
    let _measurement_artifact = family::prepare(
        engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse measured Sequence clear source")
            .expect("detect measured Sequence clear source"),
        &LayoutOptions::default(),
        measurement_session,
    )
    .expect("prepare measured Sequence clear artifact");
    assert!(
        host.snapshot().iter().any(|exchange| {
            exchange.request.text.contains("Clear Message")
                && exchange.request.font_size_bits == f64::to_bits(21.0)
        }),
        "MessageLabel Clear must measure with the computed Sequence family base"
    );

    let render_session = RenderEnvironment::deterministic()
        .begin_session_with_theme(&theme)
        .expect("begin Sequence clear session");
    let rendered = family::prepare(
        engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse rendered Sequence clear source")
            .expect("detect rendered Sequence clear source"),
        &LayoutOptions::default(),
        render_session,
    )
    .expect("prepare Sequence clear artifact")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("render Sequence clear typography");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid clear typography SVG");
    let message = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.attribute("class") == Some("messageText")
                && node
                    .descendants()
                    .filter_map(|descendant| descendant.text())
                    .any(|text| text.contains("Clear Message"))
        })
        .expect("terminal cleared message label");
    assert_eq!(
        message
            .attribute("style")
            .and_then(|style| inline_style_value(style, "font-size")),
        Some("21px")
    );
}

#[test]
fn sequence_loop_keyword_typography_expands_the_measured_label_box() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::LoopLabel,
                        ThemeStylePatch {
                            typography: ThemeTextStylePatch {
                                font_size_px: Specified::Value(96.0),
                                ..ThemeTextStylePatch::default()
                            },
                            ..ThemeStylePatch::default()
                        },
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile large Sequence LoopLabel theme");
    let source = r#"sequenceDiagram
participant Alice
participant Bob
participant Carol
critical Establish connection
Bob->>Carol: Connect
option Retry later
Carol-->>Bob: Retry
end"#;
    let engine = merman_render::__private::install_parse_compatibility(&theme, Engine::new());
    let host = Arc::new(RecordingSequenceHost::new(SequenceHostResponse::Missing));
    let measurement_identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("sequence-loop-control-label-box").expect("valid profile id"),
        "1",
    )
    .expect("valid measurement profile identity");
    let measurement_policy = TextMeasurementPolicy::host_display(
        measurement_identity,
        host.clone(),
        TextMeasurementPhase::ALL,
    );
    let measurement_session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(measurement_policy)
        .begin_session_with_theme(&theme)
        .expect("begin measured Sequence LoopLabel session");
    let _measurement_artifact = family::prepare(
        engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse measured Sequence LoopLabel source")
            .expect("detect measured Sequence LoopLabel source"),
        &LayoutOptions::default(),
        measurement_session,
    )
    .expect("prepare measured Sequence LoopLabel artifact");
    assert!(
        host.snapshot().iter().any(|exchange| {
            exchange.request.text == "critical"
                && exchange.request.font_size_bits == f64::to_bits(96.0)
        }),
        "the terminal control keyword must participate in LoopLabel layout measurement"
    );

    let render_session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin portable large Sequence LoopLabel session");
    let rendered = family::prepare(
        engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse rendered Sequence LoopLabel source")
            .expect("detect rendered Sequence LoopLabel source"),
        &LayoutOptions::default(),
        render_session,
    )
    .expect("prepare portable large Sequence LoopLabel artifact")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("measured LoopLabel control keyword must seal as portable");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid LoopLabel SVG");
    let control_structure = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-et") == Some("control-structure")
                && node.descendants().any(|descendant| {
                    descendant.has_tag_name("text")
                        && descendant.attribute("class") == Some("labelText")
                        && descendant.text() == Some("critical")
                })
        })
        .expect("critical control structure");
    let label = control_structure
        .descendants()
        .find(|node| node.has_tag_name("text") && node.attribute("class") == Some("labelText"))
        .expect("critical terminal label");
    assert_eq!(
        label
            .attribute("style")
            .and_then(|style| inline_style_value(style, "font-size")),
        Some("96px")
    );
    let points = control_structure
        .descendants()
        .find(|node| node.has_tag_name("polygon") && node.attribute("class") == Some("labelBox"))
        .and_then(|node| node.attribute("points"))
        .expect("critical label box points")
        .split_whitespace()
        .map(|point| {
            let (x, y) = point.split_once(',').expect("label box coordinate pair");
            (
                x.parse::<f64>().expect("numeric label box x"),
                y.parse::<f64>().expect("numeric label box y"),
            )
        })
        .collect::<Vec<_>>();
    let min_x = points.iter().map(|(x, _)| *x).fold(f64::INFINITY, f64::min);
    let max_x = points
        .iter()
        .map(|(x, _)| *x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = points.iter().map(|(_, y)| *y).fold(f64::INFINITY, f64::min);
    let max_y = points
        .iter()
        .map(|(_, y)| *y)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        max_x - min_x > 50.0,
        "large LoopLabel typography must widen the control label box"
    );
    assert!(
        max_y - min_y > 20.0,
        "large LoopLabel typography must increase the control label box height"
    );
    let (view_box, _) = root_view_box_and_max_width(rendered.svg());
    assert!(
        max_x <= view_box[0] + view_box[2],
        "the measured control label box must remain inside the root viewBox"
    );
    let actor_max_x = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_ascii_whitespace()
                        .any(|class| class == "actor")
                })
        })
        .filter_map(|node| {
            Some((
                node.attribute("x")?.parse::<f64>().ok()?,
                node.attribute("width")?.parse::<f64>().ok()?,
            ))
        })
        .map(|(x, width)| x + width)
        .fold(f64::NEG_INFINITY, f64::max);
    let frame_max_x = control_structure
        .descendants()
        .filter(|node| {
            node.has_tag_name("line")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_ascii_whitespace()
                        .any(|class| class == "loopLine")
                })
        })
        .flat_map(|node| [node.attribute("x1"), node.attribute("x2")])
        .flatten()
        .filter_map(|value| value.parse::<f64>().ok())
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        actor_max_x > max_x,
        "the unused leading actor regression must leave the participant row wider than the resolved label box: actor_max_x={actor_max_x}, label_max_x={max_x}"
    );
    let emitted_max_x = actor_max_x.max(frame_max_x).max(max_x);
    assert!(
        ((view_box[0] + view_box[2]) - (emitted_max_x + 50.0)).abs() <= 0.01,
        "the root must add diagramMarginX to the actual emitted geometry rather than a guessed block anchor: viewBox={view_box:?}, actor_max_x={actor_max_x}, frame_max_x={frame_max_x}, label_max_x={max_x}"
    );
}

#[test]
fn sequence_explicit_default_role_typography_remains_a_portability_residual() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::ActorLabel,
                        ThemeStylePatch {
                            typography: ThemeTextStylePatch {
                                font_weight: Specified::Value(700),
                                ..ThemeTextStylePatch::default()
                            },
                            ..ThemeStylePatch::default()
                        },
                    )
                    .with_variant(ThemeVariant::Default)
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
            ),
        )
        .expect("compile explicit-default Sequence ActorLabel theme");
    let source = "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n";
    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse explicit-default Sequence source")
        .expect("detect explicit-default Sequence source");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict explicit-default Sequence session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare explicit-default Sequence theme");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => {
            panic!("explicit Default role typography must fail closed until directly supported")
        }
        Err(error) => error,
    };
    assert!(
        matches!(
            error,
            merman_render::Error::UnverifiedFamilyTheme {
                family_id: DiagramFamilyId::SEQUENCE,
                residual_count: 1,
            }
        ),
        "unexpected explicit-default Sequence typography error: {error:?}"
    );
}

#[test]
fn sequence_lifeline_stroke_does_not_hide_unsupported_sibling_facets() {
    let mut style = ThemeStylePatch::default()
        .with_stroke(CanvasPaint::solid("#2563eb").expect("valid Lifeline stroke"))
        .with_padding(InsetsPx::all(4.0));
    style.geometry.radius = Specified::Value(6.0);
    style.paint.opacity = Specified::Value(0.75);
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                ThemeRule::new(ThemeTarget::Lifeline, style).for_family(DiagramFamilyId::SEQUENCE),
            )),
        )
        .expect("compile mixed Sequence Lifeline theme");
    let source = "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: Hello\n";

    let rendered = family::prepare(
        merman_render::__private::install_parse_compatibility(&theme, Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse mixed Sequence Lifeline source")
            .expect("detect mixed Sequence Lifeline source"),
        &LayoutOptions::default(),
        RenderEnvironment::deterministic()
            .begin_session_with_theme(&theme)
            .expect("begin mixed Sequence Lifeline session"),
    )
    .expect("prepare mixed Sequence Lifeline theme")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort rendering keeps the directly supported Lifeline stroke");
    assert!(
        rendered.svg().contains(".actor-line{stroke:#2563eb;"),
        "the typed Lifeline stroke should still reach terminal CSS: {}",
        rendered.svg()
    );

    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse strict mixed Sequence Lifeline source")
        .expect("detect strict mixed Sequence Lifeline source");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict mixed Sequence Lifeline session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare strict mixed Sequence Lifeline theme");
    let error = match artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("unsupported Lifeline radius, padding, and opacity must fail closed"),
        Err(error) => error,
    };
    assert!(
        matches!(
            error,
            merman_render::Error::UnverifiedFamilyTheme {
                family_id: DiagramFamilyId::SEQUENCE,
                residual_count: 1,
            }
        ),
        "unexpected mixed Sequence Lifeline portability error: {error:?}"
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
fn sequence_issue_86_nested_loop_and_alt_frames_follow_nested_depth() {
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

    let (outer_x1, outer_x2) = sequence_control_frame_x(&svg, "[Outer loop]");
    let (inner_x1, inner_x2) = sequence_control_frame_x(&svg, "[Inner branch]");
    assert!(
        outer_x1 < inner_x1 && outer_x2 > inner_x2,
        "nested frame sides must expand with depth, got outer x={outer_x1}..{outer_x2}, inner x={inner_x1}..{inner_x2}: {svg}"
    );
    assert!(
        (inner_x1 - outer_x1 - 10.0).abs() < 0.0001 && (outer_x2 - inner_x2 - 10.0).abs() < 0.0001,
        "nested frame sides must use Mermaid's box margin, got outer x={outer_x1}..{outer_x2}, inner x={inner_x1}..{inner_x2}: {svg}"
    );
}

#[test]
fn sequence_nested_rect_contributes_to_parent_frame_depth() {
    let svg = render_sequence_svg_from_text(
        r#"sequenceDiagram
    participant A
    participant B
    loop Outer loop
        rect rgb(240,240,240)
            alt Inner branch
                A->>B: Message
            end
        end
    end
"#,
    );
    let (outer_x1, outer_x2) = sequence_control_frame_x(&svg, "[Outer loop]");
    let (inner_x1, inner_x2) = sequence_control_frame_x(&svg, "[Inner branch]");
    assert!(
        outer_x1 < inner_x1 && outer_x2 > inner_x2,
        "the parent frame must remain wider than the nested control frame: {svg}"
    );
    assert!(
        (inner_x1 - outer_x1 - 20.0).abs() < 0.0001 && (outer_x2 - inner_x2 - 20.0).abs() < 0.0001,
        "a nested rect must consume one additional Mermaid sequence-item margin, got outer x={outer_x1}..{outer_x2}, inner x={inner_x1}..{inner_x2}: {svg}"
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
fn sequence_generated_root_typography_shadows_message_config_like_mermaid_11_17_2() {
    let path = workspace_root()
        .join("fixtures")
        .join("sequence")
        .join(
            "upstream_cypress_sequencediagram_spec_should_render_different_message_fonts_when_configured_011.mmd",
        );
    let source = std::fs::read_to_string(path).expect("Sequence configured-font fixture");
    let host = Arc::new(RecordingSequenceHost::new(SequenceHostResponse::Missing));
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("sequence-mermaid-config-font-precedence")
            .expect("valid profile id"),
        "1",
    )
    .expect("valid measurement profile identity");
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::host_display(identity, host.clone(), TextMeasurementPhase::ALL),
    );
    let observation = render_sequence_with_environment(&source, &environment);
    let requests = host.snapshot();

    for message in ["I'm short", "Short as well"] {
        let matching = requests
            .iter()
            .filter(|exchange| exchange.request.text == message)
            .collect::<Vec<_>>();
        assert!(
            !matching.is_empty(),
            "expected layout measurement requests for {message:?}"
        );
        assert!(
            matching.iter().all(|exchange| {
                exchange.request.font_size_bits == 16.0_f64.to_bits()
                    && exchange.request.font_family.as_deref() != Some("Arial")
            }),
            "Mermaid's generated root typography must shadow sequence.messageFont* during measurement: {matching:#?}"
        );
    }

    let document =
        roxmltree::Document::parse(&observation.svg).expect("valid Sequence configured-font SVG");
    for message in ["I'm short", "Short as well"] {
        let text = document
            .descendants()
            .find(|node| {
                node.has_tag_name("text")
                    && node.attribute("class").is_some_and(|classes| {
                        classes
                            .split_ascii_whitespace()
                            .any(|class| class == "messageText")
                    })
                    && node.text() == Some(message)
            })
            .unwrap_or_else(|| panic!("missing Sequence message {message:?}"));
        let inline = text.attribute("style").expect("message inline style");
        assert_eq!(inline_style_value(inline, "font-size"), Some("16px"));
        assert_eq!(
            inline_style_value(inline, "font-family"),
            None,
            "shadowed role-local Arial must not be reasserted by the terminal writer"
        );
    }
}

#[test]
fn sequence_central_connection_rtl_layout_uses_neo_marker_spacing() {
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
    // The central-connection target boundary is 5px from the actor center; Neo spacing and
    // arrowhead shortening add another 3px each for this right-to-left signal.
    assert_eq!(
        edge.points[1].x,
        actor_center("actor-top-Alice") + 5.0 + 3.0 + 3.0
    );
}

#[test]
fn sequence_central_connection_rtl_svg_uses_layout_actor_centers() {
    let fixture = "upstream_cypress_sequencediagram_v2_spec_should_render_central_connection_with_normal_arrows_right_to_lef_033.mmd";
    let svg = render_sequence_svg_from_fixture(fixture);

    assert!(
        svg.contains(r#"<text x="443" y="37""#),
        "expected Bob top actor center from layout to be preserved in SVG: {svg}"
    );
    assert!(
        svg.contains(r#"<text x="820" y="37""#),
        "expected Charlie top actor center from layout to be preserved in SVG: {svg}"
    );
    assert!(
        extract_self_closing_tags(&svg, "line")
            .into_iter()
            .any(|tag| {
                tag.contains(r#"x1="442""#)
                    && tag.contains(r#"x2="86""#)
                    && tag.contains(r#"class="messageLine"#)
            }),
        "expected first message to preserve layout centers and Neo marker spacing: {svg}"
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
        .with_compiled_math_renderer();
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
        .with_compiled_math_renderer();
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

// Isolate typed effects and geometry from Mermaid's built-in Neo shadow and palette.
fn classic_sequence_engine() -> Engine {
    Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "look": "classic",
        "theme": "default"
    })))
}

fn try_render_sequence_theme_request(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Sequence theme request")
        .expect("Sequence diagram");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("Sequence theme session");
    family::prepare(parsed, &LayoutOptions::default(), session).and_then(|artifact| {
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    })
}

#[test]
fn sequence_text_and_title_fill_have_no_legacy_writer_consumer() {
    let source = "sequenceDiagram\ntitle Diagram title\nbox Group\nparticipant A\nend\nparticipant B\nA->>B: Message\nloop Repeat\nNote over A,B: Note\nB-->>A: Reply\nend\n";
    for look in ["classic", "neo", "handDrawn"] {
        for variables in [
            serde_json::json!({}),
            serde_json::json!({"textColor":"#345678", "titleColor":"#876543"}),
        ] {
            let engine = Engine::new().with_site_config(MermaidConfig::from_value(
                serde_json::json!({"look":look, "handDrawnSeed":42, "themeVariables":variables}),
            ));
            let render = |theme: &DiagramTheme, portability| {
                try_render_sequence_theme_request(source, theme, engine.clone(), portability)
            };
            let baseline_theme = DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new())
                .unwrap();
            let baseline =
                render(&baseline_theme, ThemePortabilityRequirement::BestEffort).unwrap();
            assert!(baseline.svg().contains("Diagram title"));
            for target in [ThemeTarget::Text, ThemeTarget::Title] {
                for variant in [None, Some(ThemeVariant::Default)] {
                    for paint in [
                        CanvasPaint::solid("#d12345").unwrap(),
                        CanvasPaint::Transparent,
                    ] {
                        let mut rule =
                            ThemeRule::new(target, ThemeStylePatch::default().with_fill(paint));
                        if let Some(variant) = variant {
                            rule = rule.with_variant(variant);
                        }
                        let theme = DiagramThemeCompiler::new()
                            .compile(
                                DiagramThemeSpec::new()
                                    .with_styles(ThemeRuleSet::default().with_rule(rule)),
                            )
                            .unwrap();
                        let rendered =
                            render(&theme, ThemePortabilityRequirement::BestEffort).unwrap();
                        assert_eq!(
                            rendered.svg(),
                            baseline.svg(),
                            "{look}/{target:?}/{variant:?}/{variables}"
                        );
                        let completion = rendered.into_completion();
                        let evidence =
                            merman_render::__private::family_evidence(completion.report());
                        assert_eq!(evidence.applied_count(), 0);
                        assert_eq!(evidence.theme_residual_count(), 1);
                        assert!(
                            render(&theme, ThemePortabilityRequirement::RequirePortable).is_err()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn sequence_unsupported_text_domains_follow_occurrences_and_winners() {
    let text = || {
        ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        )
    };
    let title = || {
        ThemeRule::new(
            ThemeTarget::Title,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        )
    };
    let source = "sequenceDiagram\ntitle Diagram title\nA->>B: Message\n";
    for (source, rules, not_applicable, residuals) in [
        ("sequenceDiagram\nA->>B: Message\n", vec![title()], 1, 0),
        (
            "sequenceDiagram\ntitle Diagram title\nparticipant A\n",
            vec![text(), title()],
            0,
            2,
        ),
        (
            source,
            vec![title().with_ordinal(OrdinalSelector::exact(2).unwrap())],
            1,
            0,
        ),
        (
            source,
            vec![text().with_ordinal(OrdinalSelector::exact(2).unwrap())],
            0,
            1,
        ),
        (
            source,
            vec![text().with_ordinal(OrdinalSelector::exact(1000).unwrap())],
            1,
            0,
        ),
        (
            source,
            vec![text().with_variant(ThemeVariant::Default), text()],
            1,
            1,
        ),
        (
            source,
            vec![
                title().with_ordinal(OrdinalSelector::exact(1).unwrap()),
                title(),
            ],
            1,
            1,
        ),
    ] {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    rules
                        .into_iter()
                        .fold(ThemeRuleSet::default(), |set, rule| set.with_rule(rule)),
                ),
            )
            .unwrap();
        let rendered = try_render_sequence_theme_request(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), not_applicable, "{source}");
        assert_eq!(evidence.theme_residual_count(), residuals, "{source}");
        assert_eq!(evidence.accounted_count(), not_applicable + residuals);
        assert_eq!(
            try_render_sequence_theme_request(
                source,
                &theme,
                Engine::new(),
                ThemePortabilityRequirement::RequirePortable
            )
            .is_ok(),
            residuals == 0,
            "{source}"
        );
    }
}

#[test]
fn sequence_role_fill_coverage_reconciles_generic_text_without_hiding_other_facets() {
    let source =
        "sequenceDiagram\nA->>B: Message\nNote over A: Note\nloop Work\nB->>A: Reply\nend\n";
    let generic = || {
        ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ff00ff").unwrap()),
        )
    };
    for (roles, title, number, number_fill, extra_stroke, residuals) in [
        (true, false, false, false, false, 0),
        (false, false, false, false, false, 1),
        (true, true, false, false, false, 1),
        (true, false, true, false, false, 1),
        (true, false, true, true, false, 0),
        (true, false, false, false, true, 1),
    ] {
        let mut rules = ThemeRuleSet::default().with_rule(if extra_stroke {
            ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#ff00ff").unwrap())
                    .with_stroke(CanvasPaint::solid("#ff00ff").unwrap()),
            )
        } else {
            generic()
        });
        for target in [
            ThemeTarget::ActorLabel,
            ThemeTarget::MessageLabel,
            ThemeTarget::LoopLabel,
        ] {
            rules = rules.with_rule(ThemeRule::new(
                target,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            ));
        }
        if roles {
            rules = rules.with_rule(ThemeRule::new(
                ThemeTarget::NoteLabel,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            ));
        }
        if number_fill {
            rules = rules.with_rule(ThemeRule::new(
                ThemeTarget::SequenceNumberLabel,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            ));
        }
        let source = source.replacen(
            "sequenceDiagram\n",
            &format!(
                "sequenceDiagram\n{}{}",
                if title { "title Diagram title\n" } else { "" },
                if number { "autonumber\n" } else { "" }
            ),
            1,
        );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap();
        let rendered = try_render_sequence_theme_request(
            &source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(
            evidence.theme_residual_count(),
            residuals,
            "roles={roles} title={title} number={number} stroke={extra_stroke}"
        );
        assert_eq!(
            try_render_sequence_theme_request(
                &source,
                &theme,
                Engine::new(),
                ThemePortabilityRequirement::RequirePortable
            )
            .is_ok(),
            residuals == 0
        );
    }
}

fn sequence_actor_geometry_theme(clear: bool) -> DiagramTheme {
    let mut patch = ThemeStylePatch::default().with_stroke_width(3.0).unwrap();
    patch.geometry.radius = Specified::Value(10.0);
    let mut rules = ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Actor, patch));
    if clear {
        let mut patch = ThemeStylePatch::default();
        patch.geometry.radius = Specified::Clear;
        patch.stroke.width = Specified::Clear;
        rules = rules.with_rule(
            ThemeRule::new(ThemeTarget::Actor, patch).with_variant(ThemeVariant::Default),
        );
    }
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(rules))
        .unwrap()
}

#[test]
fn sequence_actor_geometry_reaches_both_rectangles_without_styling_text_or_lifelines() {
    for mirror in [true, false] {
        let engine = classic_sequence_engine().with_site_config(MermaidConfig::from_value(
            serde_json::json!({
                "sequence":{"mirrorActors":mirror}
            }),
        ));
        let rendered = try_render_sequence_theme_request(
            "sequenceDiagram\nA->>B: Hello",
            &sequence_actor_geometry_theme(false),
            engine,
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("rectangular actor geometry must be consumed");
        let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
        let actors: Vec<_> = doc
            .descendants()
            .filter(|n| {
                n.has_tag_name("rect")
                    && n.attribute("class")
                        .is_some_and(|c| c.split_whitespace().any(|c| c == "actor"))
            })
            .collect();
        assert_eq!(actors.len(), if mirror { 4 } else { 2 });
        for actor in actors {
            assert_eq!(actor.attribute("rx"), Some("10"));
            assert_eq!(actor.attribute("ry"), Some("10"));
            assert_eq!(actor.attribute("style"), Some("stroke-width:3px;"));
        }
        for node in doc
            .descendants()
            .filter(|n| n.has_tag_name("text") || n.has_tag_name("line"))
        {
            assert!(
                !node
                    .attribute("style")
                    .unwrap_or_default()
                    .contains("stroke-width:3px")
            );
        }
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn sequence_actor_geometry_clear_and_source_width_preserve_other_facets() {
    for clear in [false, true] {
        for source_width in [None, Some("7px")] {
            let engine = classic_sequence_engine().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "themeVariables":source_width.map(|w| serde_json::json!({"strokeWidth":w,"actorBorder":"#ff0000"})).unwrap_or(serde_json::json!({"actorBorder":"#ff0000"}))
            })));
            let rendered = try_render_sequence_theme_request(
                "sequenceDiagram\nA->>B: Hello",
                &sequence_actor_geometry_theme(clear),
                engine,
                ThemePortabilityRequirement::RequirePortable,
            )
            .expect("source width and Clear must reconcile per facet");
            let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
            let actor = doc
                .descendants()
                .find(|n| n.has_tag_name("rect") && n.attribute("name") == Some("A"))
                .unwrap();
            assert_eq!(actor.attribute("rx"), Some(if clear { "3" } else { "10" }));
            assert_eq!(
                actor.attribute("style"),
                if clear || source_width.is_some() {
                    None
                } else {
                    Some("stroke-width:3px;")
                }
            );
            if source_width.is_some() {
                assert!(rendered.svg().contains(".actor{stroke:#ff0000;"));
                assert!(rendered.svg().contains("stroke-width:7px;"));
            }
        }
    }
}

#[test]
fn sequence_actor_geometry_does_not_certify_unhandled_glyphs_or_custom_classes() {
    for declaration in [
        "actor A",
        "participant A@{type: database}",
        "participant A@{type: collections}",
        "participant A@{type: queue}",
        "participant A\nproperties A: {\"class\":\"custom\"}",
    ] {
        let source = format!("sequenceDiagram\n{declaration}\nA->>B: Hello");
        let theme = sequence_actor_geometry_theme(false);
        let rendered = try_render_sequence_theme_request(
            &source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 0, "{declaration}");
        assert_eq!(evidence.theme_residual_count(), 1, "{declaration}");
        assert!(
            try_render_sequence_theme_request(
                &source,
                &theme,
                Engine::new(),
                ThemePortabilityRequirement::RequirePortable
            )
            .is_err(),
            "{declaration}"
        );
    }
}

#[test]
fn sequence_actor_geometry_keeps_unsupported_siblings_and_ordinals_visible() {
    let source = "sequenceDiagram\nA->>B: Hello";
    for ordinal in [
        None,
        Some(OrdinalSelector::exact(1).unwrap()),
        Some(OrdinalSelector::exact(9).unwrap()),
    ] {
        let mut patch = ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#00f2ff").unwrap())
            .with_stroke_width(3.0)
            .unwrap();
        patch.geometry.radius = Specified::Value(10.0);
        // Padding has no Sequence Actor consumer; a supported sibling must not hide it.
        patch.spacing.padding = Specified::Value(InsetsPx::all(2.0));
        let mut rule = ThemeRule::new(ThemeTarget::Actor, patch);
        if let Some(ordinal) = ordinal {
            rule = rule.with_ordinal(ordinal);
        }
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
            .unwrap();
        let rendered = try_render_sequence_theme_request(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(
            evidence.theme_residual_count(),
            usize::from(ordinal != Some(OrdinalSelector::exact(9).unwrap()))
        );
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(ordinal == Some(OrdinalSelector::exact(9).unwrap()))
        );
    }
}

#[test]
fn sequence_actor_geometry_source_width_suppresses_only_that_facet_for_special_glyphs() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Actor,
                ThemeStylePatch::default().with_stroke_width(3.0).unwrap(),
            ))),
        )
        .unwrap();
    let source = "---\nconfig:\n  themeVariables:\n    strokeWidth: 7\n---\nsequenceDiagram\nactor A\nA->>B: Hello";
    let rendered = try_render_sequence_theme_request(
        source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .unwrap();
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn sequence_actor_geometry_public_cyberpunk_recipe_preserves_exchange_and_glow() {
    use merman_render::diagram_theme::ThemePreset;
    let compiler = DiagramThemeCompiler::new();
    let recipe = compiler.export_preset(ThemePreset::Cyberpunk).unwrap();
    let saved = serde_json::to_vec(&recipe).unwrap();
    let mut outputs = Vec::new();
    for theme in [
        compiler.compile_preset(ThemePreset::Cyberpunk).unwrap(),
        DiagramThemeCompiler::new()
            .compile_recipe(serde_json::from_slice(&saved).unwrap())
            .unwrap(),
    ] {
        let rendered = try_render_sequence_theme_request(
            include_str!("../../merman-theme-fixtures/fixtures/public-cyberpunk/sequence.mmd"),
            &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "look": "classic"
            }))),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
        let actors: Vec<_> = doc
            .descendants()
            .filter(|n| {
                n.has_tag_name("rect")
                    && n.attribute("class")
                        .is_some_and(|c| c.split_whitespace().any(|c| c == "actor"))
            })
            .collect();
        assert_eq!(actors.len(), 4);
        for actor in actors {
            assert_eq!(actor.attribute("rx"), Some("10"));
            assert_eq!(actor.attribute("style"), Some("stroke-width:3px;"));
        }
        let css: String = doc
            .descendants()
            .filter(|n| n.has_tag_name("style"))
            .filter_map(|n| n.text())
            .collect();
        assert!(css.contains(".messageLine0{stroke-width:2px;stroke-dasharray:none;}"));
        assert!(css.contains(".messageLine1{stroke-width:2px;stroke-dasharray:2,2;}"));
        outputs.push(rendered.svg().to_owned());
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.theme_residual_count(), 0);
        let doc = roxmltree::Document::parse(outputs.last().unwrap()).unwrap();
        let filters: Vec<_> = doc
            .descendants()
            .filter(|n| n.has_tag_name("filter"))
            .collect();
        assert_eq!(filters.len(), 21);
        for filter in filters {
            assert_eq!(
                filter.attribute("color-interpolation-filters"),
                Some("sRGB")
            );
            let deviations: Vec<_> = filter
                .children()
                .filter(|n| n.has_tag_name("feGaussianBlur"))
                .map(|n| n.attribute("stdDeviation").unwrap())
                .collect();
            let is_frame = filter.attribute("id").unwrap().contains("-loop-")
                && !filter.attribute("id").unwrap().contains("-loop-text-");
            let is_keyword = filter
                .attribute("id")
                .unwrap()
                .contains("-loop-label-background-");
            let is_actor_text = filter.attribute("id").unwrap().contains("-actor-text-");
            let is_loop_text = filter.attribute("id").unwrap().contains("-loop-text-");
            let is_note_text = filter.attribute("id").unwrap().contains("-note-text-");
            let is_lifeline = filter.attribute("id").unwrap().contains("-lifeline-");
            let is_message = filter.attribute("id").unwrap().contains("-message-");
            let is_note = filter.attribute("id").unwrap().contains("-note-");
            assert_eq!(
                deviations,
                if is_keyword {
                    vec!["6"]
                } else if is_frame {
                    vec!["4"]
                } else if is_loop_text {
                    vec!["5"]
                } else if is_note_text {
                    vec!["4"]
                } else if is_message || is_lifeline {
                    vec!["6"]
                } else if is_note {
                    vec!["8"]
                } else {
                    vec!["8", "16"]
                }
            );
            let reference = format!("url(#{})", filter.attribute("id").unwrap());
            let consumers: Vec<_> = doc
                .descendants()
                .filter(|n| n.attribute("filter") == Some(reference.as_str()))
                .collect();
            assert_eq!(consumers.len(), 1);
            if is_keyword {
                assert!(consumers[0].has_tag_name("polygon"));
                assert_eq!(consumers[0].attribute("rx"), None);
            } else if is_frame {
                assert_eq!(consumers[0].attribute("class"), Some("loopLine"));
            } else if is_actor_text {
                assert!(consumers[0].has_tag_name("text"));
                assert_eq!(consumers[0].attribute("class"), Some("actor actor-box"));
            } else if is_loop_text {
                assert!(consumers[0].has_tag_name("text"));
                assert!(matches!(
                    consumers[0].attribute("class"),
                    Some("labelText" | "loopText")
                ));
            } else if is_note_text {
                assert!(consumers[0].has_tag_name("text"));
                assert_eq!(consumers[0].attribute("class"), Some("noteText"));
            } else if is_lifeline {
                assert_eq!(consumers[0].attribute("data-et"), Some("life-line"));
            } else if is_message {
                assert!(consumers[0].has_tag_name("line"));
                assert!(
                    consumers[0]
                        .attribute("class")
                        .unwrap()
                        .starts_with("messageLine")
                );
                assert!(consumers[0].attribute("marker-end").is_some());
            } else if is_note {
                assert!(consumers[0].has_tag_name("rect"));
                assert_eq!(consumers[0].attribute("class"), Some("note"));
                assert_eq!(consumers[0].attribute("stroke-width"), Some("2"));
                assert_eq!(consumers[0].attribute("rx"), Some("10"));
                assert_eq!(consumers[0].attribute("ry"), Some("10"));
                let flood = filter
                    .descendants()
                    .find(|n| n.has_tag_name("feFlood"))
                    .unwrap();
                assert_eq!(
                    flood.attribute("flood-color"),
                    Some("rgba(255, 0, 255, 0.4)")
                );
            } else {
                assert!(consumers[0].has_tag_name("rect"));
                assert!(
                    consumers[0]
                        .attribute("class")
                        .unwrap()
                        .starts_with("actor ")
                );
            }
        }
    }
    assert_eq!(outputs[0], outputs[1]);
}

#[test]
fn sequence_actor_geometry_wide_strokes_expand_viewport_without_moving_actors() {
    let mut outputs = Vec::new();
    for width in [0.0, 200.0] {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Actor,
                        ThemeStylePatch::default().with_stroke_width(width).unwrap(),
                    ),
                )),
            )
            .unwrap();
        outputs.push(
            try_render_sequence_theme_request(
                "sequenceDiagram\nA->>B: Hello",
                &theme,
                Engine::new(),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap()
            .svg()
            .to_owned(),
        );
    }
    let documents: Vec<_> = outputs
        .iter()
        .map(|s| roxmltree::Document::parse(s).unwrap())
        .collect();
    let actors = |doc: &roxmltree::Document<'_>| {
        doc.descendants()
            .filter(|n| n.has_tag_name("rect") && n.attribute("name").is_some())
            .map(|n| {
                ["x", "y", "width", "height"]
                    .map(|key| n.attribute(key).unwrap().parse::<f64>().unwrap())
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(actors(&documents[0]), actors(&documents[1]));
    let bounds: Vec<f64> = documents[1]
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect();
    for [x, y, width, height] in actors(&documents[1]) {
        assert!(x - 100.0 >= bounds[0] && y - 100.0 >= bounds[1]);
        assert!(x + width + 100.0 <= bounds[0] + bounds[2]);
        assert!(y + height + 100.0 <= bounds[1] + bounds[3]);
    }
}

fn sequence_shadow_spec(target: ThemeTarget, rules: ThemeRuleSet) -> DiagramThemeSpec {
    use merman_render::diagram_theme::{
        DiagramEffectSet, EffectBinding, EffectGraph, EffectInput, EffectPrimitive, ThemeColorValue,
    };
    DiagramThemeSpec::new().with_styles(rules).with_effects(
        DiagramEffectSet::default()
            .with_graph(
                EffectGraph::new(
                    "actor-shadow",
                    [EffectPrimitive::DropShadow {
                        input: EffectInput::SourceGraphic,
                        offset_x: -9.0,
                        offset_y: 13.0,
                        blur_radius: 8.0,
                        spread: 0.0,
                        color: ThemeColorValue::parse("#00f2ff").unwrap(),
                    }],
                )
                .unwrap(),
            )
            .unwrap()
            .with_binding(EffectBinding::new(target, "actor-shadow").unwrap())
            .unwrap(),
    )
}

#[test]
fn sequence_actor_shadow_binding_clear_and_rule_ownership() {
    for mirror in [false, true] {
        for clear in [None, Some(false), Some(true)] {
            let mut rules = ThemeRuleSet::default();
            if let Some(clear) = clear {
                let mut patch = ThemeStylePatch::default();
                patch.effects.effect = if clear {
                    Specified::Clear
                } else {
                    Specified::Value("actor-shadow".to_owned())
                };
                rules = rules.with_rule(
                    ThemeRule::new(ThemeTarget::Actor, patch).with_variant(ThemeVariant::Default),
                );
            }
            let theme = DiagramThemeCompiler::new()
                .compile(sequence_shadow_spec(ThemeTarget::Actor, rules))
                .unwrap();
            let rendered = try_render_sequence_theme_request(
                "sequenceDiagram\nA->>B: Hello",
                &theme,
                classic_sequence_engine().with_site_config(MermaidConfig::from_value(
                    serde_json::json!({"sequence":{"mirrorActors":mirror}}),
                )),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
            let filters: Vec<_> = doc
                .descendants()
                .filter(|n| n.has_tag_name("filter"))
                .collect();
            assert_eq!(
                filters.len(),
                if clear == Some(true) {
                    0
                } else if mirror {
                    4
                } else {
                    2
                }
            );
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(clear.is_some())
            );
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn sequence_shadows_reject_unhandled_actor_geometry_and_nonzero_spread() {
    for (declaration, width) in [
        ("actor A", "1"),
        ("participant A@{type: database}", "1"),
        ("participant A@{type: collections}", "1"),
        ("participant A@{type: queue}", "1"),
        ("participant A\nproperties A: {\"class\":\"custom\"}", "1"),
        ("participant A", "2em"),
    ] {
        let source = format!("sequenceDiagram\n{declaration}\nA->>B: Hello");
        let theme = DiagramThemeCompiler::new()
            .compile(sequence_shadow_spec(
                ThemeTarget::Actor,
                ThemeRuleSet::default(),
            ))
            .unwrap();
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(
            serde_json::json!({"themeVariables":{"strokeWidth":width}}),
        ));
        let rendered = try_render_sequence_theme_request(
            &source,
            &theme,
            engine.clone(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 0, "{declaration} / {width}");
        assert_eq!(evidence.theme_residual_count(), 1);
        assert!(
            try_render_sequence_theme_request(
                &source,
                &theme,
                engine,
                ThemePortabilityRequirement::RequirePortable
            )
            .is_err()
        );
    }
    use merman_render::diagram_theme::{
        DiagramEffectSet, EffectBinding, EffectGraph, EffectInput, EffectPrimitive, ThemeColorValue,
    };
    for target in [ThemeTarget::Actor, ThemeTarget::Message, ThemeTarget::Note] {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_effects(
                    DiagramEffectSet::default()
                        .with_graph(
                            EffectGraph::new(
                                "spread",
                                [EffectPrimitive::DropShadow {
                                    input: EffectInput::SourceGraphic,
                                    offset_x: 0.0,
                                    offset_y: 0.0,
                                    blur_radius: 8.0,
                                    spread: 2.0,
                                    color: ThemeColorValue::parse("#00ffff").unwrap(),
                                }],
                            )
                            .unwrap(),
                        )
                        .unwrap()
                        .with_binding(EffectBinding::new(target, "spread").unwrap())
                        .unwrap(),
                ),
            )
            .unwrap();
        assert!(
            try_render_sequence_theme_request(
                "sequenceDiagram\nA->>B: Hello\nNote over A,B: Note",
                &theme,
                Engine::new(),
                ThemePortabilityRequirement::RequirePortable
            )
            .is_err()
        );
    }
}

#[test]
fn sequence_actor_shadow_viewport_contains_actual_regions_and_source_strokes() {
    let theme = DiagramThemeCompiler::new()
        .compile(sequence_shadow_spec(
            ThemeTarget::Actor,
            ThemeRuleSet::default(),
        ))
        .unwrap();
    for width in ["1", "200px"] {
        let rendered = try_render_sequence_theme_request(
            "sequenceDiagram\nA->>B: Hello",
            &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(
                serde_json::json!({"themeVariables":{"strokeWidth":width}}),
            )),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
        let viewport: Vec<f64> = doc
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let number =
            |node: roxmltree::Node, name| node.attribute(name).unwrap().parse::<f64>().unwrap();
        for actor in doc
            .descendants()
            .filter(|n| n.has_tag_name("rect") && n.attribute("filter").is_some())
        {
            let id = actor
                .attribute("filter")
                .unwrap()
                .strip_prefix("url(#")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let filter = doc
                .descendants()
                .find(|n| n.attribute("id") == Some(id))
                .unwrap();
            let left = number(actor, "x") + number(actor, "width") * number(filter, "x");
            let top = number(actor, "y") + number(actor, "height") * number(filter, "y");
            let right = left + number(actor, "width") * number(filter, "width");
            let bottom = top + number(actor, "height") * number(filter, "height");
            assert!(left >= viewport[0] - 0.01 && top >= viewport[1] - 0.01);
            assert!(
                right <= viewport[0] + viewport[2] + 0.01
                    && bottom <= viewport[1] + viewport[3] + 0.01
            );
            let half_stroke = if width == "1" { 0.5 } else { 100.0 };
            assert!(left <= number(actor, "x") - half_stroke - 40.9);
            assert!(bottom >= number(actor, "y") + number(actor, "height") + half_stroke + 44.9);
        }
    }
}

#[test]
fn sequence_actor_shadow_preserves_ordinal_residuals_and_resource_admission() {
    for ordinal in [1, 9] {
        let rule = ThemeRule::new(
            ThemeTarget::Actor,
            ThemeStylePatch::default()
                .with_effect("actor-shadow")
                .unwrap(),
        )
        .with_ordinal(OrdinalSelector::exact(ordinal).unwrap());
        let theme = DiagramThemeCompiler::new()
            .compile(sequence_shadow_spec(
                ThemeTarget::Actor,
                ThemeRuleSet::default().with_rule(rule),
            ))
            .unwrap();
        let result = try_render_sequence_theme_request(
            "sequenceDiagram\nA->>B: Hello",
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::RequirePortable,
        );
        assert_eq!(
            result.is_ok(),
            ordinal == 9,
            "matching unsupported ordinal must remain explicit"
        );
    }
    use merman_render::diagram_theme::{
        ThemeResourceLimitId, ThemeResourceLimitPhase, ThemeResourcePolicy,
    };
    let theme = DiagramThemeCompiler::new()
        .with_resource_policy(
            ThemeResourcePolicy::interactive()
                .with_limit(ThemeResourceLimitId::MaxEffectFilterRegionMagnitude, 1)
                .unwrap(),
        )
        .compile(sequence_shadow_spec(
            ThemeTarget::Actor,
            ThemeRuleSet::default(),
        ))
        .unwrap();
    let error = try_render_sequence_theme_request(
        "sequenceDiagram\nA->>B: Hello",
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .err()
    .expect("materialized filter region must exceed the budget");
    assert!(
        matches!(error, Error::ThemeResourceLimitExceeded(ref limit)
        if limit.phase == ThemeResourceLimitPhase::EffectMaterialize && limit.limit == ThemeResourceLimitId::MaxEffectFilterRegionMagnitude.as_str()),
        "{error:?}"
    );
}

#[test]
fn sequence_message_width_reaches_lines_and_self_paths_without_changing_dash_semantics() {
    for width in [0.0, 1.5, 2.0, 12.0] {
        for default_variant in [false, true] {
            let mut rule = ThemeRule::new(
                ThemeTarget::Message,
                ThemeStylePatch::default().with_stroke_width(width).unwrap(),
            );
            if default_variant {
                rule = rule.with_variant(ThemeVariant::Default);
            }
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)),
                )
                .unwrap();
            let rendered = try_render_sequence_theme_request(
                "sequenceDiagram\nA->>B: Request\nB-->>A: Reply\nA->>A: Self",
                &theme,
                Engine::new(),
                ThemePortabilityRequirement::RequirePortable,
            )
            .expect("static Message width must reach every concrete line/path");
            let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
            let messages: Vec<_> = doc
                .descendants()
                .filter(|n| matches!(n.attribute("class"), Some("messageLine0" | "messageLine1")))
                .collect();
            assert_eq!(messages.len(), 3);
            assert!(messages.iter().any(|n| n.has_tag_name("path")));
            let css: String = doc
                .descendants()
                .filter(|n| n.has_tag_name("style"))
                .filter_map(|n| n.text())
                .collect();
            assert!(css.contains(&format!(
                ".messageLine0{{stroke-width:{width}px;stroke-dasharray:none;}}"
            )));
            assert!(css.contains(&format!(
                ".messageLine1{{stroke-width:{width}px;stroke-dasharray:2,2;}}"
            )));
            let reply = messages
                .iter()
                .find(|n| n.attribute("class") == Some("messageLine1"))
                .unwrap();
            assert!(
                reply
                    .attribute("style")
                    .unwrap()
                    .contains("stroke-dasharray: 3, 3")
            );
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn sequence_message_width_preserves_clear_and_unsupported_selector_outcomes() {
    let width = || {
        ThemeRule::new(
            ThemeTarget::Message,
            ThemeStylePatch::default().with_stroke_width(12.0).unwrap(),
        )
    };
    let mut clear = ThemeStylePatch::default();
    clear.stroke.width = Specified::Clear;
    let cleared = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(width()).with_rule(
                ThemeRule::new(ThemeTarget::Message, clear).with_variant(ThemeVariant::Default),
            ),
        ))
        .unwrap();
    let baseline = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    let source = "sequenceDiagram\nA->>B: Request\nB-->>A: Reply";
    let base = try_render_sequence_theme_request(
        source,
        &baseline,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .unwrap();
    let restored = try_render_sequence_theme_request(
        source,
        &cleared,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .unwrap();
    assert_eq!(
        base.svg(),
        restored.svg(),
        "Clear must restore default paint and viewport"
    );
    let completion = restored.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);

    // Message ordinal and non-default variant routes remain conservatively unsupported,
    // including unmatched selectors; this increment only consumes static default width.
    let mut mixed = ThemeStylePatch::default().with_stroke_width(12.0).unwrap();
    mixed.geometry.radius = Specified::Value(4.0);
    for (case, (source, rule, accepted)) in [
        (source, ThemeRule::new(ThemeTarget::Message, mixed), false),
        (
            source,
            width().with_ordinal(OrdinalSelector::exact(1).unwrap()),
            false,
        ),
        (
            source,
            width().with_ordinal(OrdinalSelector::exact(99).unwrap()),
            false,
        ),
        ("sequenceDiagram\nparticipant A", width(), true),
        (source, width().with_variant(ThemeVariant::Active), false),
    ]
    .into_iter()
    .enumerate()
    {
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
            .unwrap();
        assert_eq!(
            try_render_sequence_theme_request(
                source,
                &theme,
                Engine::new(),
                ThemePortabilityRequirement::RequirePortable
            )
            .is_ok(),
            accepted,
            "selector case {case}"
        );
    }
}

#[test]
fn sequence_message_width_contains_actual_paths_and_markers_without_layout_margins() {
    for width in [0.0, 2.0, 12.0] {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Message,
                        ThemeStylePatch::default().with_stroke_width(width).unwrap(),
                    ),
                )),
            )
            .unwrap();
        for (target, right_angles) in [("B", false), ("A", false), ("A", true)] {
            for arrow in ["->>", "-x", "-)", r"-|\", r"-\\", "<<->>", "-|/", r"-//"] {
                let source = format!(
                    "---\nconfig:\n  sequence:\n    mirrorActors: false\n    diagramMarginX: 0\n    diagramMarginY: 0\n    rightAngles: {right_angles}\n---\nsequenceDiagram\nA{arrow}{target}: Request"
                );
                let rendered = try_render_sequence_theme_request(
                    &source,
                    &theme,
                    Engine::new(),
                    ThemePortabilityRequirement::RequirePortable,
                )
                .unwrap();
                let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
                let view_box: Vec<f64> = doc
                    .root_element()
                    .attribute("viewBox")
                    .unwrap()
                    .split_whitespace()
                    .map(|v| v.parse().unwrap())
                    .collect();
                let contains = |x: f64, y: f64, radius: f64| {
                    assert!(
                        x - radius >= view_box[0] - 1e-8
                            && y - radius >= view_box[1] - 1e-8
                            && x + radius <= view_box[0] + view_box[2] + 1e-8
                            && y + radius <= view_box[1] + view_box[3] + 1e-8,
                        "{arrow} to {target}, rightAngles={right_angles}, width={width}: point {x},{y} radius {radius}, viewBox {view_box:?}"
                    );
                };
                let message = doc
                    .descendants()
                    .find(|n| n.attribute("class") == Some("messageLine0"))
                    .unwrap();
                let (start, end) = if message.has_tag_name("line") {
                    let value = |key| message.attribute(key).unwrap().parse::<f64>().unwrap();
                    ((value("x1"), value("y1")), (value("x2"), value("y2")))
                } else {
                    // The generated self paths contain only M/C or M/H/V/H. Read their
                    // terminal coordinates and sample the actual curve, not a padding formula.
                    let numbers: Vec<f64> = message
                        .attribute("d")
                        .unwrap()
                        .split(|c: char| c.is_ascii_alphabetic() || c.is_whitespace() || c == ',')
                        .filter(|s| !s.is_empty())
                        .map(|s| s.parse().unwrap())
                        .collect();
                    if right_angles {
                        for (x, y) in [
                            (numbers[0], numbers[1]),
                            (numbers[2], numbers[1]),
                            (numbers[2], numbers[3]),
                            (numbers[4], numbers[3]),
                        ] {
                            contains(x, y, f64::from(width) / 2.0);
                        }
                        ((numbers[0], numbers[1]), (numbers[4], numbers[3]))
                    } else {
                        for step in 0..=100 {
                            let t = f64::from(step) / 100.0;
                            let u = 1.0 - t;
                            let coordinate = |axis: usize| {
                                u.powi(3) * numbers[axis]
                                    + 3.0 * u.powi(2) * t * numbers[axis + 2]
                                    + 3.0 * u * t.powi(2) * numbers[axis + 4]
                                    + t.powi(3) * numbers[axis + 6]
                            };
                            contains(coordinate(0), coordinate(1), f64::from(width) / 2.0);
                        }
                        ((numbers[0], numbers[1]), (numbers[6], numbers[7]))
                    }
                };
                for (attribute, (x, y)) in [("marker-start", start), ("marker-end", end)] {
                    contains(x, y, f64::from(width) / 2.0);
                    let Some(reference) = message.attribute(attribute) else {
                        continue;
                    };
                    let id = reference
                        .strip_prefix("url(#")
                        .unwrap()
                        .strip_suffix(')')
                        .unwrap();
                    let marker = doc
                        .descendants()
                        .find(|n| n.attribute("id") == Some(id))
                        .unwrap();
                    let value = |key| marker.attribute(key).unwrap().parse::<f64>().unwrap();
                    let scale = if marker.attribute("markerUnits") == Some("userSpaceOnUse") {
                        1.0
                    } else {
                        f64::from(width)
                    };
                    let radius = value("refX")
                        .abs()
                        .max((value("markerWidth") - value("refX")).abs())
                        .hypot(
                            value("refY")
                                .abs()
                                .max((value("markerHeight") - value("refY")).abs()),
                        )
                        * scale;
                    contains(x, y, radius);
                }
            }
        }
    }
}

#[test]
fn sequence_message_shadow_binding_clear_and_rule_ownership() {
    let source = "sequenceDiagram\nA->>B: Request\nB-->>A: Reply\nA->>A: Self\nB-->>B: Dotted self";
    for right_angles in [false, true] {
        for clear in [None, Some(false), Some(true)] {
            let mut rules = ThemeRuleSet::default();
            if let Some(clear) = clear {
                let mut patch = ThemeStylePatch::default();
                patch.effects.effect = if clear {
                    Specified::Clear
                } else {
                    Specified::Value("actor-shadow".to_owned())
                };
                rules = rules.with_rule(
                    ThemeRule::new(ThemeTarget::Message, patch).with_variant(ThemeVariant::Default),
                );
            }
            let theme = DiagramThemeCompiler::new()
                .compile(sequence_shadow_spec(ThemeTarget::Message, rules))
                .unwrap();
            let rendered = try_render_sequence_theme_request(
                source,
                &theme,
                classic_sequence_engine().with_site_config(MermaidConfig::from_value(
                    serde_json::json!({"sequence":{"rightAngles":right_angles,"diagramMarginX":0,"diagramMarginY":0}}),
                )),
                ThemePortabilityRequirement::RequirePortable,
            ).unwrap();
            let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
            let filters: Vec<_> = doc
                .descendants()
                .filter(|n| n.has_tag_name("filter"))
                .collect();
            let lines: Vec<_> = doc
                .descendants()
                .filter(|n| matches!(n.attribute("class"), Some("messageLine0" | "messageLine1")))
                .collect();
            assert_eq!(lines.len(), 4);
            assert_eq!(filters.len(), if clear == Some(true) { 0 } else { 4 });
            for line in &lines {
                assert_eq!(line.attribute("filter").is_some(), clear != Some(true));
                assert!(line.attribute("marker-end").is_some());
            }
            assert!(
                doc.descendants()
                    .filter(|n| n.has_tag_name("text"))
                    .all(|n| n.attribute("filter").is_none())
            );
            let view: Vec<f64> = doc
                .root_element()
                .attribute("viewBox")
                .unwrap()
                .split_whitespace()
                .map(|v| v.parse().unwrap())
                .collect();
            for filter in filters {
                assert_eq!(filter.attribute("filterUnits"), Some("userSpaceOnUse"));
                let [x, y, w, h] = ["x", "y", "width", "height"]
                    .map(|k| filter.attribute(k).unwrap().parse::<f64>().unwrap());
                assert!(
                    x >= view[0] - 0.001
                        && y >= view[1] - 0.001
                        && x + w <= view[0] + view[2] + 0.001
                        && y + h <= view[1] + view[3] + 0.001
                );
            }
            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(clear.is_some())
            );
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn sequence_message_shadow_preserves_unsupported_selectors_and_filter_budget() {
    for (source, rule, succeeds) in [
        (
            "sequenceDiagram\nA->>B: Hello",
            ThemeRule::new(
                ThemeTarget::Message,
                ThemeStylePatch::default()
                    .with_effect("actor-shadow")
                    .unwrap(),
            )
            .with_ordinal(OrdinalSelector::exact(1).unwrap()),
            false,
        ),
        (
            "sequenceDiagram\nA->>B: Hello",
            ThemeRule::new(
                ThemeTarget::Message,
                ThemeStylePatch::default()
                    .with_effect("actor-shadow")
                    .unwrap(),
            )
            .with_variant(ThemeVariant::Primary),
            false,
        ),
        (
            "sequenceDiagram\nparticipant A",
            ThemeRule::new(
                ThemeTarget::Message,
                ThemeStylePatch::default()
                    .with_effect("actor-shadow")
                    .unwrap(),
            ),
            true,
        ),
    ] {
        let theme = DiagramThemeCompiler::new()
            .compile(sequence_shadow_spec(
                ThemeTarget::Message,
                ThemeRuleSet::default().with_rule(rule),
            ))
            .unwrap();
        let result = try_render_sequence_theme_request(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::RequirePortable,
        );
        assert_eq!(result.is_ok(), succeeds, "{:?}", result.as_ref().err());
    }
    use merman_render::diagram_theme::{
        ThemeResourceLimitId, ThemeResourceLimitPhase, ThemeResourcePolicy,
    };
    let theme = DiagramThemeCompiler::new()
        .with_resource_policy(
            ThemeResourcePolicy::interactive()
                .with_limit(ThemeResourceLimitId::MaxEffectFilterRegionMagnitude, 1)
                .unwrap(),
        )
        .compile(sequence_shadow_spec(
            ThemeTarget::Message,
            ThemeRuleSet::default(),
        ))
        .unwrap();
    let error = try_render_sequence_theme_request(
        "sequenceDiagram\nA->>B: Hello",
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .err()
    .expect("materialized region exceeds budget");
    assert!(
        matches!(error, Error::ThemeResourceLimitExceeded(ref limit)
        if limit.phase == ThemeResourceLimitPhase::EffectMaterialize && limit.limit == ThemeResourceLimitId::MaxEffectFilterRegionMagnitude.as_str()),
        "{error:?}"
    );
}

#[test]
fn sequence_note_geometry_shadow_and_clear_reach_rect_terminals() {
    let source = "sequenceDiagram\nparticipant A\nparticipant B\nNote left of A: Left note\nNote right of B: Right note\nNote over A,B: Wide note";
    for clear in [None, Some(false), Some(true)] {
        let mut patch = ThemeStylePatch::default().with_stroke_width(12.0).unwrap();
        patch.geometry.radius = Specified::Value(10.0);
        if let Some(clear) = clear {
            patch.effects.effect = if clear {
                Specified::Clear
            } else {
                Specified::Value("actor-shadow".to_owned())
            };
        }
        let theme = DiagramThemeCompiler::new()
            .compile(sequence_shadow_spec(
                ThemeTarget::Note,
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(ThemeTarget::Note, patch).with_variant(ThemeVariant::Default),
                ),
            ))
            .unwrap();
        let rendered = try_render_sequence_theme_request(
            source,
            &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(
                serde_json::json!({"sequence":{"diagramMarginX":0,"diagramMarginY":0}}),
            )),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
        let view: Vec<f64> = doc
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let notes: Vec<_> = doc
            .descendants()
            .filter(|n| n.has_tag_name("rect") && n.attribute("class") == Some("note"))
            .collect();
        assert_eq!(notes.len(), 3);
        for note in notes {
            assert_eq!(note.attribute("stroke-width"), Some("12"));
            assert_eq!(note.attribute("rx"), Some("10"));
            assert_eq!(note.attribute("ry"), Some("10"));
            assert_eq!(note.attribute("filter").is_some(), clear != Some(true));
            let [left, top, width, height] = ["x", "y", "width", "height"]
                .map(|k| note.attribute(k).unwrap().parse::<f64>().unwrap());
            let bounds = if let Some(binding) = note.attribute("filter") {
                let filter = doc
                    .descendants()
                    .find(|n| n.attribute("id") == Some(&binding[5..binding.len() - 1]))
                    .unwrap();
                assert_eq!(filter.attribute("filterUnits"), Some("objectBoundingBox"));
                let [x, y, w, h] = ["x", "y", "width", "height"]
                    .map(|k| filter.attribute(k).unwrap().parse::<f64>().unwrap());
                [
                    left + x * width,
                    top + y * height,
                    left + (x + w) * width,
                    top + (y + h) * height,
                ]
            } else {
                [
                    left - 6.0,
                    top - 6.0,
                    left + width + 6.0,
                    top + height + 6.0,
                ]
            };
            assert!(
                bounds[0] >= view[0] - 0.001
                    && bounds[1] >= view[1] - 0.001
                    && bounds[2] <= view[0] + view[2] + 0.001
                    && bounds[3] <= view[1] + view[3] + 0.001,
                "{bounds:?} {view:?}"
            );
        }
        assert!(
            doc.descendants()
                .filter(|n| n.has_tag_name("text"))
                .all(|n| n.attribute("filter").is_none())
        );
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.theme_residual_count(), 0);
        assert_eq!(
            evidence.applied_count(),
            if clear.is_none() { 2 } else { 1 }
        );
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(clear.is_some())
        );
    }
}

#[test]
fn sequence_note_geometry_clear_source_paint_and_unsupported_facets() {
    let source = "sequenceDiagram\nNote over A,B: Note";
    for clear in [false, true] {
        let mut patch = ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#ff0000").unwrap())
            .with_stroke(CanvasPaint::solid("#00ff00").unwrap())
            .with_stroke_width(12.0)
            .unwrap();
        patch.geometry.radius = Specified::Value(10.0);
        let mut rules = ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Note, patch));
        if clear {
            let mut patch = ThemeStylePatch::default();
            patch.stroke.width = Specified::Clear;
            patch.geometry.radius = Specified::Clear;
            rules = rules.with_rule(ThemeRule::new(ThemeTarget::Note, patch));
        }
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap();
        let rendered = try_render_sequence_theme_request(source, &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({"themeVariables":{"noteBkgColor":"#123456","noteBorderColor":"#654321","strokeWidth":200}}))),
            ThemePortabilityRequirement::RequirePortable).unwrap();
        let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
        let note = doc
            .descendants()
            .find(|n| n.has_tag_name("rect") && n.attribute("class") == Some("note"))
            .unwrap();
        assert_eq!(
            note.attribute("stroke-width"),
            if clear { None } else { Some("12") }
        );
        assert_eq!(note.attribute("rx"), if clear { None } else { Some("10") });
        let css: String = doc
            .descendants()
            .filter(|n| n.has_tag_name("style"))
            .filter_map(|n| n.text())
            .collect();
        assert!(css.contains("stroke:#654321;fill:#123456;"), "{css}");
    }
    for selector in [0, 1, 2] {
        let mut patch = ThemeStylePatch::default().with_stroke_width(2.0).unwrap();
        if selector == 0 {
            patch.spacing.padding = Specified::Value(InsetsPx::all(2.0));
        }
        let rule = ThemeRule::new(ThemeTarget::Note, patch);
        let rule = match selector {
            1 => rule.with_ordinal(OrdinalSelector::exact(1).unwrap()),
            2 => rule.with_variant(ThemeVariant::Primary),
            _ => rule,
        };
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
            .unwrap();
        assert!(
            try_render_sequence_theme_request(
                source,
                &theme,
                Engine::new(),
                ThemePortabilityRequirement::RequirePortable
            )
            .is_err()
        );
    }
}

#[test]
fn sequence_note_shadow_absence_and_resource_admission() {
    use merman_render::diagram_theme::{
        ThemeResourceLimitId, ThemeResourceLimitPhase, ThemeResourcePolicy,
    };
    let compiler = DiagramThemeCompiler::new();
    let spec = || sequence_shadow_spec(ThemeTarget::Note, ThemeRuleSet::default());
    let theme = compiler.compile(spec()).unwrap();
    let rendered = try_render_sequence_theme_request(
        "sequenceDiagram\nA->>B: No note",
        &theme,
        classic_sequence_engine(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .unwrap();
    assert!(!rendered.svg().contains("<filter"));
    let completion = rendered.into_completion();
    assert_eq!(
        merman_render::__private::family_evidence(completion.report()).not_applicable_count(),
        1
    );
    let theme = DiagramThemeCompiler::new()
        .with_resource_policy(
            ThemeResourcePolicy::interactive()
                .with_limit(ThemeResourceLimitId::MaxEffectFilterRegionMagnitude, 1)
                .unwrap(),
        )
        .compile(spec())
        .unwrap();
    let error = try_render_sequence_theme_request(
        "sequenceDiagram\nNote over A,B: Note",
        &theme,
        classic_sequence_engine(),
        ThemePortabilityRequirement::BestEffort,
    )
    .err()
    .expect("region budget must remain authoritative");
    assert!(
        matches!(error, Error::ThemeResourceLimitExceeded(ref limit) if limit.phase == ThemeResourceLimitPhase::EffectMaterialize && limit.limit == ThemeResourceLimitId::MaxEffectFilterRegionMagnitude.as_str()),
        "{error:?}"
    );
}

#[test]
fn sequence_note_label_shadow_is_text_only_and_clear_preserves_layout() {
    let source = "sequenceDiagram\nautonumber\nA->>B: Message\nNote left of A: 中文 Note<br/>Second line\nNote right of B: Another";
    let mut positions = None;
    for clear in [None, Some(false), Some(true)] {
        let mut rules = ThemeRuleSet::default();
        if let Some(clear) = clear {
            let mut patch = ThemeStylePatch::default();
            patch.effects.effect = if clear {
                Specified::Clear
            } else {
                Specified::Value("actor-shadow".to_owned())
            };
            rules = rules.with_rule(
                ThemeRule::new(ThemeTarget::NoteLabel, patch).with_variant(ThemeVariant::Default),
            );
        }
        let theme = DiagramThemeCompiler::new()
            .compile(sequence_shadow_spec(ThemeTarget::NoteLabel, rules))
            .unwrap();
        let rendered = try_render_sequence_theme_request(
            source,
            &theme,
            classic_sequence_engine(),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
        let labels: Vec<_> = doc
            .descendants()
            .filter(|n| n.has_tag_name("text") && n.attribute("class") == Some("noteText"))
            .collect();
        assert_eq!(labels.len(), 3);
        let current: Vec<_> = labels
            .iter()
            .map(|n| {
                (
                    n.attribute("x").unwrap().to_owned(),
                    n.attribute("y").unwrap().to_owned(),
                )
            })
            .collect();
        if let Some(previous) = &positions {
            assert_eq!(previous, &current);
        }
        positions = Some(current);
        let view: Vec<f64> = doc
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        for label in labels {
            assert_eq!(label.attribute("filter").is_some(), clear != Some(true));
            if let Some(binding) = label.attribute("filter") {
                let filter = doc
                    .descendants()
                    .find(|n| n.attribute("id") == Some(&binding[5..binding.len() - 1]))
                    .unwrap();
                assert_eq!(filter.attribute("filterUnits"), Some("userSpaceOnUse"));
                let [x, y, w, h] = ["x", "y", "width", "height"]
                    .map(|k| filter.attribute(k).unwrap().parse::<f64>().unwrap());
                assert!(
                    x >= view[0] - 0.001
                        && y >= view[1] - 0.001
                        && x + w <= view[0] + view[2] + 0.001
                        && y + h <= view[1] + view[3] + 0.001
                );
            }
        }
        assert!(
            doc.descendants()
                .filter(|n| n.attribute("filter").is_some())
                .all(|n| n.has_tag_name("text") && n.attribute("class") == Some("noteText"))
        );
        if clear == Some(true) {
            let baseline = DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new())
                .unwrap();
            let plain = try_render_sequence_theme_request(
                source,
                &baseline,
                classic_sequence_engine(),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            assert_eq!(
                rendered.svg(),
                plain.svg(),
                "Clear must retain the ordinary no-effect output"
            );
        }
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.theme_residual_count(), 0);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(clear.is_some())
        );
    }
}

#[test]
fn sequence_note_label_effect_residuals_and_absence_are_honest() {
    for (source, rules, accepted) in [
        (
            "sequenceDiagram\nA->>B: No note",
            ThemeRuleSet::default(),
            true,
        ),
        (
            "sequenceDiagram\nNote over A,B: Note",
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::NoteLabel,
                    ThemeStylePatch::default()
                        .with_effect("actor-shadow")
                        .unwrap(),
                )
                .with_ordinal(OrdinalSelector::exact(1).unwrap()),
            ),
            false,
        ),
        (
            "sequenceDiagram\nNote over A,B: Note",
            ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::NoteLabel,
                ThemeStylePatch::default()
                    .with_effect("actor-shadow")
                    .unwrap()
                    .with_stroke_width(3.0)
                    .unwrap(),
            )),
            false,
        ),
    ] {
        let theme = DiagramThemeCompiler::new()
            .compile(sequence_shadow_spec(ThemeTarget::NoteLabel, rules))
            .unwrap();
        let result = try_render_sequence_theme_request(
            source,
            &theme,
            classic_sequence_engine(),
            ThemePortabilityRequirement::RequirePortable,
        );
        assert_eq!(
            result.is_ok(),
            accepted,
            "{source}: {:?}",
            result.as_ref().err()
        );
        if let Ok(rendered) = result {
            assert!(!rendered.svg().contains("<filter"));
        }
    }
}

#[test]
fn sequence_note_label_effect_final_output_and_region_budgets_remain_authoritative() {
    use merman_render::diagram_theme::{ThemeResourceLimitId, ThemeResourcePolicy};
    let source = "sequenceDiagram\nNote left of A: 中文 Long note<br/><br/>Another line";
    let spec = || sequence_shadow_spec(ThemeTarget::NoteLabel, ThemeRuleSet::default());
    let theme = DiagramThemeCompiler::new().compile(spec()).unwrap();
    let render = |policy| {
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session_with_theme(&theme)
            .unwrap();
        let parsed = parse_sequence_for_render(
            &merman_render::__private::install_parse_compatibility(&theme, Engine::new()),
            source,
        );
        family::prepare(parsed, &LayoutOptions::default(), session).and_then(|artifact| {
            artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        })
    };
    let baseline = render(RenderResourcePolicy::unbounded_for_trusted_input()).unwrap();
    let size = baseline.svg().len();
    let exact = render(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, size)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(exact.svg(), baseline.svg());
    let error = render(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, size - 1)
            .unwrap(),
    )
    .err()
    .unwrap();
    assert!(
        matches!(error, Error::ResourceLimitExceeded(_)),
        "{error:?}"
    );
    let theme = DiagramThemeCompiler::new()
        .with_resource_policy(
            ThemeResourcePolicy::interactive()
                .with_limit(ThemeResourceLimitId::MaxEffectFilterRegionMagnitude, 1)
                .unwrap(),
        )
        .compile(spec())
        .unwrap();
    let error = try_render_sequence_theme_request(
        source,
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .err()
    .unwrap();
    assert!(
        matches!(error, Error::ThemeResourceLimitExceeded(_)),
        "{error:?}"
    );
}

#[cfg(feature = "math")]
#[test]
fn sequence_note_label_math_glow_remains_incomplete_and_clear_is_consumed() {
    for clear in [false, true] {
        let mut rules = ThemeRuleSet::default();
        if clear {
            let mut patch = ThemeStylePatch::default();
            patch.effects.effect = Specified::Clear;
            rules = rules.with_rule(ThemeRule::new(ThemeTarget::NoteLabel, patch));
        }
        let theme = DiagramThemeCompiler::new()
            .compile(sequence_shadow_spec(ThemeTarget::NoteLabel, rules))
            .unwrap();
        let session = RenderEnvironment::deterministic()
            .with_compiled_math_renderer()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .unwrap();
        let parsed = parse_sequence_for_render(
            &merman_render::__private::install_parse_compatibility(&theme, Engine::new()),
            "sequenceDiagram\nNote over A,B: Ordinary\nNote over A,B: $$x^2$$",
        );
        let result =
            family::prepare(parsed, &LayoutOptions::default(), session).and_then(|artifact| {
                artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            });
        assert_eq!(result.is_ok(), clear, "{:?}", result.as_ref().err());
    }
}

#[test]
fn sequence_paintless_note_rule_is_consumed_without_claiming_a_filter() {
    let theme = DiagramThemeCompiler::new()
        .compile(sequence_shadow_spec(
            ThemeTarget::NoteLabel,
            ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::NoteLabel,
                ThemeStylePatch::default()
                    .with_effect("actor-shadow")
                    .unwrap(),
            )),
        ))
        .unwrap();
    let rendered = try_render_sequence_theme_request(
        "sequenceDiagram\nNote over A,B: <br/>",
        &theme,
        classic_sequence_engine(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .unwrap();
    assert!(!rendered.svg().contains("<filter"));
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn sequence_loop_label_effects_cover_keyword_primary_and_section_titles() {
    let source = "sequenceDiagram\nloop Retry<br/>Second\nA->>B: Work\nend\nalt Accepted\nB-->>A: Done\nelse Other\nB-->>A: Retry\nend";
    for clear in [false, true] {
        let mut rules = ThemeRuleSet::default();
        if clear {
            let mut patch = ThemeStylePatch::default();
            patch.effects.effect = Specified::Clear;
            rules = rules.with_rule(ThemeRule::new(ThemeTarget::LoopLabel, patch));
        }
        let theme = DiagramThemeCompiler::new()
            .compile(sequence_shadow_spec(ThemeTarget::LoopLabel, rules))
            .unwrap();
        let rendered = try_render_sequence_theme_request(
            source,
            &theme,
            classic_sequence_engine(),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
        let viewport: Vec<f64> = doc
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        for filter in doc.descendants().filter(|n| n.has_tag_name("filter")) {
            let [x, y, w, h] = ["x", "y", "width", "height"]
                .map(|k| filter.attribute(k).unwrap().parse::<f64>().unwrap());
            assert!(
                x >= viewport[0] - 0.001
                    && y >= viewport[1] - 0.001
                    && x + w <= viewport[0] + viewport[2] + 0.001
                    && y + h <= viewport[1] + viewport[3] + 0.001
            );
        }
        for class in ["labelText", "loopText", "sectionTitle"] {
            let labels: Vec<_> = doc
                .descendants()
                .filter(|n| n.has_tag_name("text") && n.attribute("class") == Some(class))
                .collect();
            assert!(!labels.is_empty(), "missing {class}");
            for label in labels {
                assert_eq!(label.attribute("filter").is_some(), !clear, "{class}");
            }
        }
        assert!(
            doc.descendants()
                .filter(|n| n.attribute("filter").is_some())
                .all(|n| n.has_tag_name("text")
                    && matches!(
                        n.attribute("class"),
                        Some("labelText" | "loopText" | "sectionTitle")
                    ))
        );
        if clear {
            let plain = DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new())
                .unwrap();
            let baseline = try_render_sequence_theme_request(
                source,
                &plain,
                classic_sequence_engine(),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            assert_eq!(rendered.svg(), baseline.svg());
        }
    }
}

#[test]
fn sequence_loop_label_effects_preserve_rule_residuals_and_empty_titles() {
    for (source, rules, accepted) in [
        (
            "sequenceDiagram\nA->>B: No control",
            ThemeRuleSet::default(),
            true,
        ),
        (
            "sequenceDiagram\nloop\nA->>B: Work\nend",
            ThemeRuleSet::default(),
            true,
        ),
        (
            "sequenceDiagram\nloop Title\nA->>B: Work\nend",
            ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::LoopLabel,
                ThemeStylePatch::default()
                    .with_effect("actor-shadow")
                    .unwrap(),
            )),
            true,
        ),
        (
            "sequenceDiagram\nloop Title\nA->>B: Work\nend",
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::LoopLabel,
                    ThemeStylePatch::default()
                        .with_effect("actor-shadow")
                        .unwrap(),
                )
                .with_ordinal(OrdinalSelector::exact(1).unwrap()),
            ),
            false,
        ),
        (
            "sequenceDiagram\nloop Title\nA->>B: Work\nend",
            ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::LoopLabel,
                ThemeStylePatch::default()
                    .with_effect("actor-shadow")
                    .unwrap()
                    .with_stroke_width(3.0)
                    .unwrap(),
            )),
            false,
        ),
    ] {
        let theme = DiagramThemeCompiler::new()
            .compile(sequence_shadow_spec(ThemeTarget::LoopLabel, rules))
            .unwrap();
        let result = try_render_sequence_theme_request(
            source,
            &theme,
            classic_sequence_engine(),
            ThemePortabilityRequirement::RequirePortable,
        );
        assert_eq!(
            result.is_ok(),
            accepted,
            "{source}: {:?}",
            result.as_ref().err()
        );
        if source.contains("loop\n") {
            let rendered = result.unwrap();
            let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
            assert_eq!(
                doc.descendants()
                    .filter(|n| n.has_tag_name("filter"))
                    .count(),
                1,
                "empty title has no filter, keyword still does"
            );
        }
    }
}

#[cfg(feature = "math")]
#[test]
fn sequence_loop_label_math_effects_remain_incomplete_but_clear_is_consumed() {
    for source in [
        "sequenceDiagram\nloop $$x^2$$\nA->>B: Work\nend",
        "sequenceDiagram\nalt First\nA->>B: Work\nelse $$x^2$$\nA->>B: Again\nend",
    ] {
        for clear in [false, true] {
            let mut rules = ThemeRuleSet::default();
            if clear {
                let mut patch = ThemeStylePatch::default();
                patch.effects.effect = Specified::Clear;
                rules = rules.with_rule(ThemeRule::new(ThemeTarget::LoopLabel, patch));
            }
            let theme = DiagramThemeCompiler::new()
                .compile(sequence_shadow_spec(ThemeTarget::LoopLabel, rules))
                .unwrap();
            let session = RenderEnvironment::deterministic()
                .with_compiled_math_renderer()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .unwrap();
            let parsed = parse_sequence_for_render(
                &merman_render::__private::install_parse_compatibility(&theme, Engine::new()),
                source,
            );
            let result =
                family::prepare(parsed, &LayoutOptions::default(), session).and_then(|artifact| {
                    artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                });
            assert_eq!(
                result.is_ok(),
                clear,
                "{source}: {:?}",
                result.as_ref().err()
            );
        }
    }
}

#[test]
fn sequence_actor_label_effects_cover_shapes_boxes_and_visible_links() {
    let source = r#"sequenceDiagram
box Team
participant A as Alpha<br/>中文
participant Q@{ "type": "queue" }
participant D@{ "type": "database" }
participant C@{ "type": "collections" }
end
actor U as User
participant B@{ "type": "boundary" }
participant E@{ "type": "entity" }
participant K@{ "type": "control" }
A->>U: Work
"#;
    for clear in [false, true] {
        let rules = if clear {
            let mut patch = ThemeStylePatch::default();
            patch.effects.effect = Specified::Clear;
            ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::ActorLabel, patch))
        } else {
            ThemeRuleSet::default()
        };
        let theme = DiagramThemeCompiler::new()
            .compile(sequence_shadow_spec(ThemeTarget::ActorLabel, rules))
            .unwrap();
        let rendered = try_render_sequence_theme_request(
            source,
            &theme,
            classic_sequence_engine(),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
        let labels: Vec<_> = doc
            .descendants()
            .filter(|n| {
                n.has_tag_name("text")
                    && n.attribute("class")
                        .is_some_and(|c| c == "text" || c.split_whitespace().any(|v| v == "actor"))
            })
            .collect();
        assert_eq!(
            labels.len(),
            19,
            "two copies of eight actors, an extra line each for Alpha, and one box title"
        );
        for label in labels {
            assert_eq!(label.attribute("filter").is_some(), !clear, "{label:?}");
            if let Some(binding) = label.attribute("filter") {
                let id = binding
                    .strip_prefix("url(#")
                    .unwrap()
                    .strip_suffix(')')
                    .unwrap();
                let filter = doc
                    .descendants()
                    .find(|n| n.attribute("id") == Some(id))
                    .unwrap();
                let mut translation_y = 0.0;
                for parent in label.ancestors() {
                    if let Some(transform) = parent.attribute("transform") {
                        let translation = transform
                            .strip_prefix("translate(0,")
                            .unwrap()
                            .strip_suffix(')')
                            .unwrap();
                        translation_y += translation.trim().parse::<f64>().unwrap();
                    }
                }
                let [x, y, w, h] = ["x", "y", "width", "height"]
                    .map(|k| filter.attribute(k).unwrap().parse::<f64>().unwrap());
                let (view, _) = root_view_box_and_max_width(rendered.svg());
                assert!(x >= view[0] - 0.001 && x + w <= view[0] + view[2] + 0.001);
                assert!(
                    y + translation_y >= view[1] - 0.001
                        && y + translation_y + h <= view[1] + view[3] + 0.001
                );
            }
        }
        assert_eq!(
            doc.descendants()
                .filter(|n| n.has_tag_name("filter"))
                .count(),
            if clear { 0 } else { 19 }
        );
        if clear {
            let plain = DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new())
                .unwrap();
            let baseline = try_render_sequence_theme_request(
                source,
                &plain,
                classic_sequence_engine(),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            assert_eq!(rendered.svg(), baseline.svg());
        }
    }
    for (force, clear) in [(false, false), (true, false), (false, true)] {
        let source = format!(
            r#"---
config:
  sequence:
    forceMenus: {force}
    mirrorActors: false
---
sequenceDiagram
participant A
link A: Documentation @ https://example.com
A->>B: Work
"#
        );
        let mut rules = ThemeRuleSet::default();
        if clear {
            let mut patch = ThemeStylePatch::default();
            patch.effects.effect = Specified::Clear;
            rules = rules.with_rule(ThemeRule::new(ThemeTarget::ActorLabel, patch));
        }
        let theme = DiagramThemeCompiler::new()
            .compile(sequence_shadow_spec(ThemeTarget::ActorLabel, rules))
            .unwrap();
        let result = try_render_sequence_theme_request(
            &source,
            &theme,
            classic_sequence_engine(),
            ThemePortabilityRequirement::RequirePortable,
        );
        assert_eq!(
            result.is_ok(),
            force || clear,
            "force={force}, clear={clear}: {:?}",
            result.as_ref().err()
        );
        if force {
            let rendered = result.unwrap();
            let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
            assert_eq!(
                doc.descendants()
                    .filter(|n| n.has_tag_name("filter"))
                    .count(),
                3
            );
            let link = doc
                .descendants()
                .find(|n| n.has_tag_name("text") && n.ancestors().any(|p| p.has_tag_name("a")))
                .unwrap();
            assert!(link.attribute("filter").is_some());
        }
    }
}

#[test]
fn sequence_actor_label_effect_rules_retain_unsupported_facets_and_selectors() {
    for (ordinal, sibling, accepted) in [
        (false, false, true),
        (true, false, false),
        (false, true, false),
    ] {
        let mut patch = ThemeStylePatch::default()
            .with_effect("actor-shadow")
            .unwrap();
        if sibling {
            patch = patch.with_stroke_width(3.0).unwrap();
        }
        let mut rule = ThemeRule::new(ThemeTarget::ActorLabel, patch);
        if ordinal {
            rule = rule.with_ordinal(OrdinalSelector::exact(1).unwrap());
        }
        let theme = DiagramThemeCompiler::new()
            .compile(sequence_shadow_spec(
                ThemeTarget::ActorLabel,
                ThemeRuleSet::default().with_rule(rule),
            ))
            .unwrap();
        let result = try_render_sequence_theme_request(
            "sequenceDiagram\nA->>B: Work",
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::RequirePortable,
        );
        assert_eq!(
            result.is_ok(),
            accepted,
            "ordinal={ordinal}, sibling={sibling}: {:?}",
            result.as_ref().err()
        );
    }
}

#[cfg(feature = "math")]
#[test]
fn sequence_actor_label_math_glow_is_incomplete_and_clear_preserves_fallback() {
    for declaration in ["participant A as $$x^2$$", "actor A as $$x^2$$"] {
        for clear in [false, true] {
            let mut rules = ThemeRuleSet::default();
            if clear {
                let mut patch = ThemeStylePatch::default();
                patch.effects.effect = Specified::Clear;
                rules = rules.with_rule(ThemeRule::new(ThemeTarget::ActorLabel, patch));
            }
            let theme = DiagramThemeCompiler::new()
                .compile(sequence_shadow_spec(ThemeTarget::ActorLabel, rules))
                .unwrap();
            let session = RenderEnvironment::deterministic()
                .with_compiled_math_renderer()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .unwrap();
            let parsed = parse_sequence_for_render(
                &merman_render::__private::install_parse_compatibility(&theme, Engine::new()),
                &format!("sequenceDiagram\n{declaration}\nA->>B: Work"),
            );
            let result =
                family::prepare(parsed, &LayoutOptions::default(), session).and_then(|artifact| {
                    artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                });
            assert_eq!(
                result.is_ok(),
                clear,
                "{declaration}: {:?}",
                result.as_ref().err()
            );
        }
    }
}

#[test]
fn sequence_activation_geometry_and_effects_reach_nested_terminals() {
    let source = "sequenceDiagram\nA->>B: Start\nactivate B\nB->>B: Nested\nactivate B\nB-->>A: Inner\ndeactivate B\nB-->>A: Outer\ndeactivate B";
    for clear in [false, true] {
        let mut patch = ThemeStylePatch::default().with_stroke_width(3.0).unwrap();
        patch.geometry.radius = Specified::Value(4.0);
        if clear {
            patch.stroke.width = Specified::Clear;
            patch.geometry.radius = Specified::Clear;
            patch.effects.effect = Specified::Clear;
        }
        let theme = DiagramThemeCompiler::new()
            .compile(sequence_shadow_spec(
                ThemeTarget::Activation,
                ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Activation, patch)),
            ))
            .unwrap();
        let rendered = try_render_sequence_theme_request(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
        let rectangles: Vec<_> = doc
            .descendants()
            .filter(|n| {
                n.has_tag_name("rect")
                    && n.attribute("class")
                        .is_some_and(|c| c.starts_with("activation"))
            })
            .collect();
        assert_eq!(rectangles.len(), 2);
        for rect in rectangles {
            assert_eq!(
                rect.attribute("stroke-width"),
                if clear { None } else { Some("3") }
            );
            assert_eq!(rect.attribute("rx"), if clear { None } else { Some("4") });
            assert_eq!(rect.attribute("filter").is_some(), !clear);
        }
        if clear {
            let plain = DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new())
                .unwrap();
            let baseline = try_render_sequence_theme_request(
                source,
                &plain,
                Engine::new(),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            assert_eq!(rendered.svg(), baseline.svg());
        }
    }
}

#[test]
fn sequence_lifeline_effects_reach_all_actor_shapes_and_clear_restores_baseline() {
    let source = "sequenceDiagram\nparticipant A\nactor U\nparticipant B@{\"type\":\"boundary\"}\nparticipant E@{\"type\":\"entity\"}\nparticipant C@{\"type\":\"control\"}\nparticipant Q@{\"type\":\"queue\"}\nparticipant D@{\"type\":\"database\"}\nparticipant L@{\"type\":\"collections\"}\nA->>U: Request";
    for clear in [false, true] {
        let mut patch = ThemeStylePatch::default();
        if clear {
            patch.effects.effect = Specified::Clear;
        }
        let theme = DiagramThemeCompiler::new()
            .compile(sequence_shadow_spec(
                ThemeTarget::Lifeline,
                if clear {
                    ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Lifeline, patch))
                } else {
                    ThemeRuleSet::default()
                },
            ))
            .unwrap();
        let rendered = try_render_sequence_theme_request(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
        let lines: Vec<_> = doc
            .descendants()
            .filter(|n| n.attribute("data-et") == Some("life-line"))
            .collect();
        assert_eq!(lines.len(), 8);
        for line in lines {
            assert_eq!(line.attribute("filter").is_some(), !clear);
        }
        if clear {
            let plain = DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new())
                .unwrap();
            let baseline = try_render_sequence_theme_request(
                source,
                &plain,
                Engine::new(),
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            assert_eq!(rendered.svg(), baseline.svg());
        }
    }
}

#[test]
fn sequence_lifeline_and_activation_effects_keep_source_ownership_and_residuals() {
    let source = "---\nconfig:\n  themeVariables:\n    actorLineColor: '#ff0000'\n    activationBkgColor: '#000000'\n    activationBorderColor: '#ffffff'\n---\nsequenceDiagram\nA->>B: Start\nactivate B\nB-->>A: Done\ndeactivate B";
    for target in [ThemeTarget::Lifeline, ThemeTarget::Activation] {
        for (ordinal, sibling, accepted) in [
            (false, false, true),
            (true, false, false),
            (false, true, false),
        ] {
            let mut patch = ThemeStylePatch::default()
                .with_effect("actor-shadow")
                .unwrap()
                .with_stroke(CanvasPaint::solid("#00ffff").unwrap());
            if sibling {
                patch.paint.opacity = Specified::Value(0.5);
            }
            let mut rule = ThemeRule::new(target, patch).with_variant(ThemeVariant::Default);
            if ordinal {
                rule = rule.with_ordinal(OrdinalSelector::exact(1).unwrap());
            }
            let theme = DiagramThemeCompiler::new()
                .compile(sequence_shadow_spec(
                    target,
                    ThemeRuleSet::default().with_rule(rule),
                ))
                .unwrap();
            let result = try_render_sequence_theme_request(
                source,
                &theme,
                classic_sequence_engine(),
                ThemePortabilityRequirement::RequirePortable,
            );
            assert_eq!(
                result.is_ok(),
                accepted,
                "{target:?} ordinal={ordinal} sibling={sibling}: {:?}",
                result.as_ref().err()
            );
            if accepted {
                let rendered = result.unwrap();
                let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
                let filter_count = doc
                    .descendants()
                    .filter(|n| n.has_tag_name("filter"))
                    .count();
                assert_eq!(
                    filter_count,
                    if target == ThemeTarget::Lifeline {
                        2
                    } else {
                        1
                    }
                );
                let css = doc
                    .descendants()
                    .find(|n| n.has_tag_name("style"))
                    .unwrap()
                    .text()
                    .unwrap();
                assert!(css.contains(if target == ThemeTarget::Lifeline {
                    "stroke:#ff0000"
                } else {
                    "stroke:#ffffff"
                }));
            }
        }
    }
}

#[test]
fn sequence_control_frame_effect_reaches_each_line_and_separator() {
    let theme = DiagramThemeCompiler::new()
        .compile(sequence_shadow_spec(
            ThemeTarget::Loop,
            ThemeRuleSet::default(),
        ))
        .unwrap();
    let rendered = try_render_sequence_theme_request(
        "sequenceDiagram\nparticipant A\nparticipant B\nalt First\nA->>B: One\nelse Second\nB-->>A: Two\nend",
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
    ).expect("control frame effect must reach the actual lines");
    let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
    let lines: Vec<_> = doc
        .descendants()
        .filter(|n| n.attribute("class") == Some("loopLine"))
        .collect();
    assert_eq!(lines.len(), 5);
    assert!(lines.iter().all(|n| n.attribute("filter").is_some()));
    assert!(
        doc.descendants()
            .filter(|n| n.attribute("class") == Some("labelBox"))
            .all(|n| n.attribute("filter").is_none())
    );
}

#[test]
fn sequence_control_surfaces_keep_effect_clear_and_paint_independent() {
    let source = "sequenceDiagram\nparticipant A\nparticipant B\nloop Work\nA->>B: Request\nend";
    for source_owned in [false, true] {
        let input = if source_owned {
            format!(
                "---\nconfig:\n  themeVariables:\n    labelBoxBkgColor: '#abcdef'\n    labelBoxBorderColor: '#123456'\n---\n{source}"
            )
        } else {
            source.to_owned()
        };
        for clear_frame in [false, true] {
            for clear_keyword in [false, true] {
                let mut frame = ThemeStylePatch::default()
                    .with_stroke(CanvasPaint::solid("#ff0000").unwrap())
                    .with_stroke_width(6.0)
                    .unwrap();
                frame.effects.effect = if clear_frame {
                    Specified::Clear
                } else {
                    Specified::Value("actor-shadow".to_owned())
                };
                let mut keyword = ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#001122").unwrap())
                    .with_stroke(CanvasPaint::solid("#00ff00").unwrap())
                    .with_stroke_width(4.0)
                    .unwrap();
                keyword.effects.effect = if clear_keyword {
                    Specified::Clear
                } else {
                    Specified::Value("actor-shadow".to_owned())
                };
                let rules = ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(ThemeTarget::Loop, frame)
                            .with_variant(ThemeVariant::Default),
                    )
                    .with_rule(ThemeRule::new(ThemeTarget::LoopLabelBackground, keyword));
                let theme = DiagramThemeCompiler::new()
                    .compile(sequence_shadow_spec(ThemeTarget::Loop, rules))
                    .unwrap();
                let rendered = try_render_sequence_theme_request(
                    &input,
                    &theme,
                    classic_sequence_engine(),
                    ThemePortabilityRequirement::RequirePortable,
                )
                .unwrap();
                let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
                let css = doc
                    .descendants()
                    .find(|n| n.has_tag_name("style"))
                    .unwrap()
                    .text()
                    .unwrap();
                assert!(css.contains(if source_owned {
                    ".labelBox{stroke:#123456;fill:#abcdef;"
                } else {
                    ".labelBox{stroke:#00ff00;fill:#001122;"
                }));
                assert!(css.contains(if source_owned {
                    ".loopLine{stroke-width:2px;stroke-dasharray:2,2;stroke:#123456;"
                } else {
                    ".loopLine{stroke-width:2px;stroke-dasharray:2,2;stroke:#ff0000;"
                }));
                for (class, width, clear, count) in [
                    ("loopLine", "6px", clear_frame, 4),
                    ("labelBox", "4px", clear_keyword, 1),
                ] {
                    let terminals: Vec<_> = doc
                        .descendants()
                        .filter(|n| n.attribute("class") == Some(class))
                        .collect();
                    assert_eq!(terminals.len(), count);
                    for terminal in terminals {
                        assert_eq!(terminal.attribute("filter").is_some(), !clear);
                        assert!(
                            terminal
                                .attribute("style")
                                .unwrap()
                                .contains(&format!("stroke-width:{width};"))
                        );
                    }
                }
                assert_eq!(
                    doc.descendants()
                        .filter(|n| n.has_tag_name("filter"))
                        .count(),
                    usize::from(!clear_frame) * 4 + usize::from(!clear_keyword)
                );
            }
        }
    }
}

#[test]
fn sequence_control_unsupported_siblings_remain_residual() {
    let source = "sequenceDiagram\nloop Work\nA->>B: Request\nend";
    for (target, patch) in [
        (
            ThemeTarget::Loop,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ff0000").unwrap()),
        ),
        (ThemeTarget::Loop, {
            let mut p = ThemeStylePatch::default();
            p.paint.opacity = Specified::Value(0.5);
            p
        }),
        (ThemeTarget::LoopLabelBackground, {
            let mut p = ThemeStylePatch::default();
            p.geometry.radius = Specified::Value(10.0);
            p
        }),
    ] {
        let theme = DiagramThemeCompiler::new()
            .compile(sequence_shadow_spec(
                target,
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    target,
                    patch.with_effect("actor-shadow").unwrap(),
                )),
            ))
            .unwrap();
        assert!(
            try_render_sequence_theme_request(
                source,
                &theme,
                Engine::new(),
                ThemePortabilityRequirement::RequirePortable
            )
            .is_err(),
            "{target:?}"
        );
    }
}

#[test]
fn sequence_control_keyword_shadow_covers_the_actual_narrow_polygon() {
    let theme = DiagramThemeCompiler::new()
        .compile(sequence_shadow_spec(
            ThemeTarget::LoopLabelBackground,
            ThemeRuleSet::default(),
        ))
        .unwrap();
    let rendered = try_render_sequence_theme_request(
        "---\nconfig:\n  sequence:\n    labelBoxWidth: 1\n    diagramMarginX: 0\n---\nsequenceDiagram\nloop Work\nA->>B: Request\nend",
        &theme, classic_sequence_engine(), ThemePortabilityRequirement::RequirePortable,
    ).unwrap();
    let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
    let polygon = doc
        .descendants()
        .find(|n| n.attribute("class") == Some("labelBox"))
        .unwrap();
    let min_x = polygon
        .attribute("points")
        .unwrap()
        .split_whitespace()
        .map(|pair| pair.split_once(',').unwrap().0.parse::<f64>().unwrap())
        .reduce(f64::min)
        .unwrap();
    let first_x: f64 = polygon
        .attribute("points")
        .unwrap()
        .split_once(',')
        .unwrap()
        .0
        .parse()
        .unwrap();
    assert!(
        first_x - min_x > 7.0,
        "fixture must retain the outlying keyword vertex"
    );
    let filter = doc
        .descendants()
        .find(|n| n.has_tag_name("filter"))
        .unwrap();
    let left: f64 = filter.attribute("x").unwrap().parse().unwrap();
    // Shared lowering retains four sigma (32px); the 1px polygon stroke needs its
    // default miter envelope of 2px. Omitting the x3 vertex cannot satisfy this bound.
    assert!(
        left <= min_x - 2.0 - 9.0 - 32.0,
        "left={left}, actual polygon minimum={min_x}"
    );
}

#[test]
fn sequence_control_effects_are_absent_without_control_structures() {
    for target in [ThemeTarget::Loop, ThemeTarget::LoopLabelBackground] {
        let theme = DiagramThemeCompiler::new()
            .compile(sequence_shadow_spec(target, ThemeRuleSet::default()))
            .unwrap();
        let rendered = try_render_sequence_theme_request(
            "sequenceDiagram\nA->>B: Request",
            &theme,
            classic_sequence_engine(),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        assert!(!rendered.svg().contains("<filter"));
    }
}

#[test]
fn sequence_neo_shadow_and_participant_paints_are_scoped_and_look_specific() {
    for (look, theme, flood) in [
        ("neo", "redux", "#000000"),
        ("neo", "default", "#FFFFFF"),
        ("classic", "redux", ""),
    ] {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "look": look,
            "theme": theme
        })));
        let svg = render_sequence_svg_from_text_with_engine(
            engine,
            "sequenceDiagram\nparticipant Alice\nparticipant Bob\nAlice->>Bob: hello\nactivate Bob\nNote over Bob: note\nBob-->>Alice: done\ndeactivate Bob\n",
        );
        let doc = roxmltree::Document::parse(&svg).expect("Sequence SVG");
        let filters: Vec<_> = doc
            .descendants()
            .filter(|node| {
                node.has_tag_name("filter") && node.attribute("id") == Some("merman-drop-shadow")
            })
            .collect();
        assert_eq!(filters.len(), usize::from(look == "neo"));
        if let Some(filter) = filters.first() {
            assert_eq!(filter.attribute("height"), Some("130%"));
            assert_eq!(filter.attribute("width"), Some("130%"));
            let shadow = filter
                .children()
                .find(|node| node.has_tag_name("feDropShadow"))
                .unwrap();
            assert_eq!(shadow.attribute("flood-color"), Some(flood));
            assert_eq!(shadow.attribute("flood-opacity"), Some("0.06"));
            let definitions: Vec<_> = doc
                .root_element()
                .children()
                .filter(|node| node.has_tag_name("defs"))
                .collect();
            assert_eq!(
                definitions.last().unwrap().first_element_child(),
                Some(*filter)
            );
        }
        for rect in doc.descendants().filter(|node| node.has_tag_name("rect")) {
            let class = rect.attribute("class").unwrap_or("");
            if class.starts_with("actor ") || class.starts_with("activation") || class == "note" {
                assert_eq!(
                    rect.attribute("data-look"),
                    (look == "neo").then_some("neo")
                );
            }
            if class.starts_with("actor ") {
                assert_eq!(
                    rect.attribute("rx"),
                    Some(if look == "neo" { "6" } else { "3" })
                );
                assert_eq!(
                    rect.attribute("filter"),
                    (look == "neo").then_some("url(#merman-drop-shadow)")
                );
            }
        }
    }
}

#[test]
fn sequence_neo_participant_type_glyphs_share_band_and_shadow() {
    let source = std::fs::read_to_string(
        workspace_root()
            .join("fixtures")
            .join("sequence")
            .join("participant_types.mmd"),
    )
    .unwrap();
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(
        serde_json::json!({"look": "neo"}),
    ));
    let svg = render_sequence_svg_from_text_with_engine(engine, &source);
    let doc = roxmltree::Document::parse(&svg).expect("Sequence participant-types SVG");
    for (name, kind) in [("boundary", "boundary"), ("C", "control"), ("E", "entity")] {
        let node = doc
            .descendants()
            .find(|node| {
                node.attribute("name") == Some(name) && node.attribute("data-type") == Some(kind)
            })
            .expect("participant glyph");
        if kind == "control" {
            assert_eq!(node.attribute("filter"), None);
            assert_eq!(
                node.descendants()
                    .find(|child| child.has_tag_name("circle"))
                    .and_then(|circle| circle.attribute("filter")),
                Some("url(#merman-drop-shadow)")
            );
        } else {
            assert_eq!(
                node.attribute("filter"),
                Some("url(#merman-drop-shadow)"),
                "{name}"
            );
        }
        let label = node
            .descendants()
            .find(|child| child.has_tag_name("text"))
            .expect("glyph label");
        assert_eq!(
            label.attribute("y"),
            Some("62"),
            "{name} label should share the Neo band"
        );
    }
    let footer_names: Vec<_> = doc
        .descendants()
        .filter(|node| {
            matches!(
                node.attribute("class"),
                Some("actor-man actor-bottom") | Some("actor actor-bottom")
            )
        })
        .filter(|node| matches!(node.attribute("name"), Some("boundary" | "C" | "E")))
        .filter_map(|node| node.attribute("name"))
        .collect();
    assert_eq!(footer_names, ["boundary", "C", "E"]);
}

#[test]
fn sequence_actor_glyphs_follow_overlays_and_precede_messages_and_popups() {
    for look in ["classic", "neo"] {
        for mirror_actors in [false, true] {
            for actor_type in ["actor", "boundary", "control", "entity"] {
                let source = format!(
                    "sequenceDiagram\nparticipant A@{{ \"type\": \"{actor_type}\" }}\nparticipant B\nlinks B: {{\"Docs\": \"https://example.com\"}}\nNote over A: overlay\nloop repeat\nA->>B: ping\nend"
                );
                let engine =
                    Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                        "look": look,
                        "sequence": {"mirrorActors": mirror_actors, "forceMenus": true}
                    })));
                let svg = render_sequence_svg_from_text_with_engine(engine, &source);
                let doc = roxmltree::Document::parse(&svg).expect("Sequence actor order SVG");
                let children: Vec<_> = doc
                    .root_element()
                    .children()
                    .filter(|n| n.is_element())
                    .collect();
                let note = children
                    .iter()
                    .position(|n| n.attribute("data-et") == Some("note"))
                    .expect("note");
                let control = children
                    .iter()
                    .position(|n| n.attribute("data-et") == Some("control-structure"))
                    .expect("loop");
                let actor = children
                    .iter()
                    .position(|n| {
                        n.attribute("data-id") == Some("A")
                            && n.attribute("data-et") == Some("participant")
                    })
                    .expect("top actor glyph");
                let message = children
                    .iter()
                    .position(|n| n.attribute("data-et") == Some("message"))
                    .expect("message");
                let popup = children
                    .iter()
                    .position(|n| n.attribute("class") == Some("actorPopupMenu"))
                    .expect("popup");
                assert!(
                    note < actor && control < actor && actor < message && message < popup,
                    "{look}/{actor_type}/mirror={mirror_actors}"
                );
                let footer = children.iter().position(|n| {
                    n.has_tag_name("g")
                        && n.attribute("name") == Some("A")
                        && n.attribute("class").is_some_and(|class| {
                            class
                                .split_ascii_whitespace()
                                .any(|part| part == "actor-bottom")
                        })
                });
                if mirror_actors {
                    let footer = footer.expect("bottom actor glyph");
                    assert!(message < footer && footer < popup, "{look}/{actor_type}");
                } else {
                    assert!(footer.is_none(), "{look}/{actor_type}");
                }
            }
        }
    }
}

#[test]
fn sequence_activation_palettes_follow_actor_order_across_creation_and_nesting() {
    let source = "sequenceDiagram\nparticipant Idle\nparticipant A\ncreate participant B\nA->>+B: create\nactivate B\nB-->>A: nested\ndeactivate B\nB-->>-A: done\nactivate A\nA->>B: last\ndeactivate A";
    for theme in ["redux-color", "redux-dark-color", "default"] {
        for look in ["classic", "neo"] {
            for backgrounds in [
                serde_json::json!(["#110000", "#220000", "#330000"]),
                serde_json::json!([]),
            ] {
                let engine =
                    Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                        "look": look,
                        "theme": theme,
                        "themeVariables": {
                            "borderColorArray": ["#000011", "#000022"],
                            "bkgColorArray": backgrounds,
                            "mainBkg": "#445566"
                        }
                    })));
                let svg = render_sequence_svg_from_text_with_engine(engine, source);
                let doc =
                    roxmltree::Document::parse(&svg).expect("Sequence activation palette SVG");
                let activations: Vec<_> = doc
                    .descendants()
                    .filter(|node| {
                        node.has_tag_name("rect")
                            && node
                                .attribute("class")
                                .is_some_and(|class| class.starts_with("activation"))
                    })
                    .collect();
                assert_eq!(activations.len(), 3);
                for (activation, actor_index) in activations.iter().zip([2, 2, 1]) {
                    if theme == "default" {
                        assert_eq!(activation.attribute("style"), None);
                        continue;
                    }
                    let stroke = if actor_index == 2 {
                        "rgb(0, 0, 17)"
                    } else {
                        "rgb(0, 0, 34)"
                    };
                    let fill = if backgrounds.as_array().unwrap().is_empty() {
                        "rgb(68, 85, 102)"
                    } else if actor_index == 2 {
                        "rgb(51, 0, 0)"
                    } else {
                        "rgb(34, 0, 0)"
                    };
                    let expected = format!("stroke: {stroke}; fill: {fill};");
                    assert_eq!(
                        activation.attribute("style"),
                        Some(expected.as_str()),
                        "{theme}/{look}/actor={actor_index}"
                    );
                }
            }
        }
    }
}

#[test]
fn sequence_popup_text_uses_resolved_actor_font_style() {
    let source = "sequenceDiagram\nparticipant A\nlinks A: {\"Docs\": \"https://example.com\"}";
    for (config, expected_size, expected_weight) in [
        (
            serde_json::json!({
                "fontSize": 22,
                "fontWeight": 600,
                "sequence": {"forceMenus": true}
            }),
            "22px",
            "600",
        ),
        (
            serde_json::json!({
                "sequence": {
                    "forceMenus": true,
                    "actorFontSize": 19,
                    "actorFontWeight": "bold"
                }
            }),
            "16px",
            "bold",
        ),
    ] {
        let svg = render_sequence_svg_from_text_with_engine(
            Engine::new().with_site_config(MermaidConfig::from_value(config)),
            source,
        );
        let doc = roxmltree::Document::parse(&svg).expect("Sequence popup SVG");
        let text = doc
            .descendants()
            .find(|node| {
                node.has_tag_name("g") && node.attribute("class") == Some("actorPopupMenu")
            })
            .and_then(|popup| popup.descendants().find(|node| node.has_tag_name("text")))
            .expect("popup text");
        let style = text.attribute("style").expect("popup style");
        assert_eq!(
            inline_style_value(style, "font-size"),
            Some(expected_size),
            "{style}"
        );
        assert_eq!(
            inline_style_value(style, "font-weight"),
            Some(expected_weight),
            "{style}"
        );
    }
}

#[test]
fn sequence_popup_inherits_actor_rect_height_position_width_and_corner_radius() {
    let source = "sequenceDiagram\nparticipant A as First<br/>Second<br/>Third<br/>Fourth\nlinks A: {\"Docs\": \"https://example.com\", \"A very long popup menu label requiring a wider panel\": \"https://example.com/long\"}";
    for look in ["classic", "neo"] {
        for theme in ["default", "redux-color"] {
            for mirror_actors in [false, true] {
                let engine = Engine::new().with_site_config(MermaidConfig::from_value(
                    serde_json::json!({
                        "look": look,
                        "theme": theme,
                        "sequence": {"mirrorActors": mirror_actors, "forceMenus": true, "wrap": true},
                        "themeVariables": {"nodeBorderRadius": 22}
                    }),
                ));
                let svg = render_sequence_svg_from_text_with_engine(engine, source);
                let doc = roxmltree::Document::parse(&svg).expect("Sequence popup SVG");
                let actor = doc
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("rect")
                            && node.attribute("name") == Some("A")
                            && node.attribute("class")
                                == Some(if mirror_actors {
                                    "actor actor-bottom"
                                } else {
                                    "actor actor-top"
                                })
                    })
                    .expect("actor rect");
                let popup = doc
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("g") && node.attribute("class") == Some("actorPopupMenu")
                    })
                    .expect("popup");
                let panel = popup
                    .children()
                    .find(|node| node.has_tag_name("rect"))
                    .expect("popup panel");
                assert_eq!(popup.attribute("display"), Some("block !important"));
                assert_eq!(panel.attribute("x"), actor.attribute("x"));
                assert_eq!(panel.attribute("y"), actor.attribute("height"));
                let radius = if look == "neo" { "6" } else { "3" };
                assert_eq!(panel.attribute("rx"), Some(radius));
                assert_eq!(panel.attribute("ry"), Some(radius));
                let actor_height: f64 = actor.attribute("height").unwrap().parse().unwrap();
                assert!(
                    actor_height > 65.0,
                    "wrapped actor must exceed configured height"
                );
                assert!(
                    panel.attribute("width").unwrap().parse::<f64>().unwrap()
                        > actor.attribute("width").unwrap().parse::<f64>().unwrap(),
                    "long link label must widen popup panel"
                );
                let text = popup
                    .descendants()
                    .find(|node| node.has_tag_name("text"))
                    .expect("popup text");
                let text_y: f64 = text.attribute("y").unwrap().parse().unwrap();
                assert_eq!(text_y, actor_height + 30.0);
                let view_box: Vec<f64> = doc
                    .root_element()
                    .attribute("viewBox")
                    .unwrap()
                    .split_whitespace()
                    .map(|n| n.parse().unwrap())
                    .collect();
                let popup_bottom =
                    actor_height + panel.attribute("height").unwrap().parse::<f64>().unwrap();
                assert!(
                    view_box[1] + view_box[3] >= popup_bottom,
                    "popup must fit the root bounds: {look}/{theme}/mirror={mirror_actors}"
                );
            }
        }
    }
}

#[test]
fn sequence_actor_labels_and_popups_share_resolved_weight() {
    let source = "sequenceDiagram\nparticipant A\nlinks A: {\"Docs\": \"https://example.com\"}";
    for (root, expected) in [
        (serde_json::json!("bogus"), None),
        (serde_json::json!(true), None),
        (serde_json::json!("700; fill: red"), None),
        (serde_json::json!("bolder"), Some("bolder")),
        (serde_json::json!("inherit"), Some("inherit")),
        (serde_json::json!(false), Some("700")),
        (serde_json::json!(0), Some("700")),
    ] {
        let config = serde_json::json!({
            "fontWeight": root,
            "sequence": {"actorFontWeight": 700, "forceMenus": true}
        });
        let svg = render_sequence_svg_from_text_with_engine(
            Engine::new().with_site_config(MermaidConfig::from_value(config.clone())),
            source,
        );
        let doc = roxmltree::Document::parse(&svg).expect("Sequence SVG");
        let texts =
            doc.descendants()
                .filter(|node| {
                    node.has_tag_name("text")
                        && (node.attribute("class").is_some_and(|class| {
                            class.split_whitespace().any(|part| part == "actor")
                        }) || node
                            .ancestors()
                            .any(|ancestor| ancestor.attribute("class") == Some("actorPopupMenu")))
                })
                .collect::<Vec<_>>();
        assert_eq!(texts.len(), 3, "two actor labels and one menu label");
        for text in texts {
            let style = text.attribute("style").expect("text style");
            assert_eq!(
                inline_style_value(style, "font-weight").unwrap_or("400"),
                expected.unwrap_or("400"),
                "{config}: {style}"
            );
            assert_eq!(
                inline_style_value(style, "fill"),
                None,
                "CSS must remain one value"
            );
        }
    }
}

#[test]
fn sequence_numeric_actor_weight_matches_string_in_popup_measurement_and_bounds() {
    let mut outputs = Vec::new();
    for weight in [serde_json::json!(700), serde_json::json!("700")] {
        let config =
            serde_json::json!({"sequence": {"actorFontWeight": weight, "forceMenus": true}});
        let source = format!(
            "---\nconfig: {config}\n---\nsequenceDiagram\nparticipant A\nlinks A: {{\"probe-long-popup-label\": \"https://example.com\"}}"
        );
        let observation = render_sequence_with_host_environment(
            &source,
            SequenceHostResponse::WeightSensitiveMetrics,
            "sequence-weight-probe",
            RenderEnvironment::deterministic(),
        );
        let requests = observation
            .requests
            .iter()
            .filter(|request| request.text == "probe-long-popup-label")
            .collect::<Vec<_>>();
        assert!(
            requests.len() >= 2,
            "root bounds and popup rendering must measure the menu"
        );
        for request in requests {
            assert_eq!(request.font_weight.as_deref(), Some("700"), "{request:?}");
        }
        let doc = roxmltree::Document::parse(&observation.render.svg).expect("Sequence SVG");
        let popup = doc
            .descendants()
            .find(|node| node.attribute("class") == Some("actorPopupMenu"))
            .unwrap();
        let panel = popup
            .children()
            .find(|node| node.has_tag_name("rect"))
            .unwrap();
        let width: f64 = panel.attribute("width").unwrap().parse().unwrap();
        assert!(
            width >= 420.0,
            "font-sensitive host width must reach popup geometry"
        );
        let viewbox = doc
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .split_whitespace()
            .map(|value| value.parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        let x: f64 = panel.attribute("x").unwrap().parse().unwrap();
        assert!(
            viewbox[0] + viewbox[2] >= x + width,
            "root bounds must contain the menu"
        );
        outputs.push(observation.render.svg);
    }
    assert_eq!(
        outputs[0], outputs[1],
        "numeric and string weights must have identical geometry and SVG"
    );
}

#[test]
fn sequence_wrapped_actor_height_uses_host_text_dimensions() {
    for description in [
        "probe-one<br>probe-two<br>probe-three<br>probe-four",
        "probe-one probe-two probe-three probe-four",
    ] {
        for (look, expected_height) in [("classic", 96.0), ("neo", 152.0)] {
            let config = serde_json::json!({
                "look": look,
                "sequence": {"wrap": true, "mirrorActors": false}
            });
            let source = format!(
                "---\nconfig: {config}\n---\nsequenceDiagram\nparticipant A as {description}"
            );
            let observation = render_sequence_with_host_environment(
                &source,
                SequenceHostResponse::WeightSensitiveMetrics,
                "sequence-wrapped-actor-height",
                RenderEnvironment::deterministic(),
            );
            let doc = roxmltree::Document::parse(&observation.render.svg).expect("Sequence SVG");
            let actor = doc
                .descendants()
                .find(|node| {
                    node.has_tag_name("rect")
                        && node.attribute("name") == Some("A")
                        && node.attribute("class") == Some("actor actor-top")
                })
                .expect("actor rectangle");
            assert_eq!(
                actor.attribute("height").unwrap().parse::<f64>().unwrap(),
                expected_height,
                "{look}/{description}: all four host-measured 24px lines must size the participant row"
            );
        }
    }
}

#[test]
fn sequence_box_titles_share_measured_height_and_use_actor_font_when_drawn() {
    let config = serde_json::json!({
        "fontSize": 26,
        "sequence": {"actorFontWeight": 700, "messageFontWeight": 400}
    });
    let source = format!(
        "---\nconfig: {config}\n---\nsequenceDiagram\nbox probe-one<br>probe-two\nparticipant A\nend\nbox probe-other\nparticipant B\nend\nA->>B: hello"
    );
    let observation = render_sequence_with_host_environment(
        &source,
        SequenceHostResponse::WeightSensitiveMetrics,
        "sequence-box-height",
        RenderEnvironment::deterministic(),
    );
    let doc = roxmltree::Document::parse(&observation.render.svg).expect("Sequence SVG");
    let actors = doc
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect") && node.attribute("class") == Some("actor actor-top")
        })
        .collect::<Vec<_>>();
    assert_eq!(actors.len(), 2);
    for actor in actors {
        assert_eq!(
            actor.attribute("y"),
            Some("58"),
            "10px margin plus tallest 48px title"
        );
    }
    let titles = doc
        .descendants()
        .filter(|node| node.has_tag_name("text") && node.attribute("class") == Some("text"))
        .collect::<Vec<_>>();
    assert_eq!(titles.len(), 3, "one text/tspan pair per title line");
    let mut offsets = Vec::new();
    for title in titles {
        assert_eq!(
            title.attribute("y"),
            Some("29"),
            "all boxes share the maximum title height"
        );
        let style = title.attribute("style").expect("title style");
        assert_eq!(
            inline_style_value(style, "font-size"),
            Some("26px"),
            "{style}"
        );
        assert_eq!(
            inline_style_value(style, "font-weight"),
            Some("700"),
            "{style}"
        );
        let tspan = title
            .children()
            .find(|node| node.has_tag_name("tspan"))
            .unwrap();
        offsets.push(tspan.attribute("dy").unwrap().parse::<i32>().unwrap());
    }
    offsets.sort_unstable();
    assert_eq!(offsets, [-13, 0, 13]);
    for request in observation.requests.iter().filter(|request| {
        request.text.starts_with("probe-")
            && request.operation == TextMeasurementOperation::MermaidCalculateTextDimensions
    }) {
        assert_eq!(
            f64::from_bits(request.font_size_bits),
            26.0,
            "box dimensions use the resolved messageFont"
        );
        assert_eq!(request.font_weight.as_deref(), Some("400"));
    }
}

#[test]
fn sequence_box_frames_use_source_padding_and_global_cursor() {
    for (mirror, expected_height) in [(false, 139.0), (true, 264.0)] {
        let config = serde_json::json!({
            "look": "classic",
            "sequence": {"boxMargin": 20, "boxTextMargin": 3, "mirrorActors": mirror}
        });
        let source = format!(
            "---\nconfig: {config}\n---\nsequenceDiagram\nbox probe-one\nparticipant A\nend"
        );
        let observation = render_sequence_with_host_environment(
            &source,
            SequenceHostResponse::WeightSensitiveMetrics,
            "sequence-box-frame",
            RenderEnvironment::deterministic(),
        );
        let doc = roxmltree::Document::parse(&observation.render.svg).unwrap();
        let frame = doc
            .descendants()
            .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some("rect"))
            .unwrap();
        assert_eq!(frame.attribute("x"), Some("-40"));
        assert_eq!(frame.attribute("y"), Some("-10"));
        assert_eq!(frame.attribute("width"), Some("236"));
        assert_eq!(
            frame.attribute("height").unwrap().parse::<f64>().unwrap(),
            expected_height
        );
        let title = doc
            .descendants()
            .find(|node| node.has_tag_name("text") && node.attribute("class") == Some("text"))
            .unwrap();
        assert_eq!(title.attribute("y"), Some("15"));
    }
}

#[test]
fn sequence_unnamed_narrow_box_still_reserves_wrap_padding() {
    let config = serde_json::json!({
        "sequence": {"width": 20, "wrap": true, "boxMargin": 0, "boxTextMargin": 5, "wrapPadding": 10}
    });
    let source = format!("---\nconfig: {config}\n---\nsequenceDiagram\nbox\nparticipant A\nend");
    let observation =
        render_sequence_with_environment(&source, &RenderEnvironment::deterministic());
    let doc = roxmltree::Document::parse(&observation.svg).unwrap();
    let actor = doc
        .descendants()
        .find(|node| {
            node.has_tag_name("rect") && node.attribute("class") == Some("actor actor-top")
        })
        .unwrap();
    assert_eq!(actor.attribute("x"), Some("10"));
    let frame = doc
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some("rect"))
        .unwrap();
    assert_eq!(frame.attribute("x"), Some("0"));
    assert_eq!(frame.attribute("width"), Some("40"));
    assert!(
        !doc.descendants()
            .any(|node| { node.has_tag_name("text") && node.attribute("class") == Some("text") })
    );
}

#[test]
fn sequence_box_wraps_title_before_measurement_and_emission() {
    let source = "---\nconfig: {sequence: {wrap: true}}\n---\nsequenceDiagram\nbox probe-long-one probe-long-two probe-long-three\nparticipant A\nend";
    let observation = render_sequence_with_host_environment(
        source,
        SequenceHostResponse::WeightSensitiveMetrics,
        "sequence-wrapped-box",
        RenderEnvironment::deterministic(),
    );
    let doc = roxmltree::Document::parse(&observation.render.svg).unwrap();
    let actor = doc
        .descendants()
        .find(|node| {
            node.has_tag_name("rect") && node.attribute("class") == Some("actor actor-top")
        })
        .unwrap();
    assert_eq!(actor.attribute("y"), Some("82"));
    let titles = doc
        .descendants()
        .filter(|node| node.has_tag_name("text") && node.attribute("class") == Some("text"))
        .collect::<Vec<_>>();
    assert_eq!(titles.len(), 3);
    let lines = titles
        .iter()
        .map(|node| {
            node.children()
                .find(|child| child.has_tag_name("tspan"))
                .unwrap()
                .text()
                .unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        lines,
        ["probe-long-one", "probe-long-two", "probe-long-three"]
    );
    assert!(
        observation
            .requests
            .iter()
            .filter(|request| { request.text.starts_with("probe-") })
            .all(|request| request.phase == TextMeasurementPhase::SvgBBox
                && request.operation == TextMeasurementOperation::MermaidCalculateTextDimensions)
    );
}

#[test]
fn sequence_box_start_precedes_created_actor_half_width_spacing() {
    let source = "---\nconfig: {look: classic, sequence: {boxMargin: 20, boxTextMargin: 3}}\n---\nsequenceDiagram\nparticipant A\ncreate participant B\nA->>B: hello\nbox probe-one\nparticipant B as Bee\nend";
    let observation = render_sequence_with_host_environment(
        source,
        SequenceHostResponse::WeightSensitiveMetrics,
        "sequence-created-box",
        RenderEnvironment::deterministic(),
    );
    let doc = roxmltree::Document::parse(&observation.render.svg).unwrap();
    let frame = doc
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some("rect"))
        .unwrap();
    let actor = doc
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class") == Some("actor actor-top")
                && node.attribute("name") == Some("B")
        })
        .unwrap();
    let actor_x = actor.attribute("x").unwrap().parse::<f64>().unwrap();
    let actor_width = actor.attribute("width").unwrap().parse::<f64>().unwrap();
    assert_eq!(
        frame.attribute("x").unwrap().parse::<f64>().unwrap(),
        actor_x - 3.0 - actor_width / 2.0 - 40.0
    );
    assert_eq!(
        frame.attribute("width").unwrap().parse::<f64>().unwrap(),
        actor_width * 1.5 + 6.0 + 80.0
    );
    assert_eq!(
        frame.attribute("y"),
        Some("-10"),
        "created actor's later y must not move the frame"
    );
}

#[test]
fn sequence_empty_box_is_measured_but_not_drawn() {
    let source = "sequenceDiagram\nbox probe-empty\nend\nparticipant A";
    let observation = render_sequence_with_host_environment(
        source,
        SequenceHostResponse::WeightSensitiveMetrics,
        "sequence-empty-box",
        RenderEnvironment::deterministic(),
    );
    let doc = roxmltree::Document::parse(&observation.render.svg).unwrap();
    assert!(
        !doc.descendants()
            .any(|node| node.attribute("class") == Some("rect"))
    );
    assert!(
        !doc.descendants()
            .any(|node| node.attribute("class") == Some("text"))
    );
    let actor = doc
        .descendants()
        .find(|node| {
            node.has_tag_name("rect") && node.attribute("class") == Some("actor actor-top")
        })
        .unwrap();
    assert_eq!(actor.attribute("y"), Some("34"));
}

#[test]
fn sequence_family_fonts_match_measurement_and_svg_with_falsy_or_string_global_size() {
    for (global_size, override_size) in [
        (serde_json::json!(""), None),
        (serde_json::json!(0), None),
        (serde_json::json!("22"), Some(22.0)),
        (serde_json::json!("22px"), Some(22.0)),
    ] {
        let config = serde_json::json!({
            "fontFamily": "",
            "fontSize": global_size,
            "sequence": {
                "actorFontFamily": "serif",
                "actorFontSize": "19px",
                "noteFontFamily": "monospace",
                "noteFontSize": "32",
                "messageFontFamily": "sans-serif",
                "messageFontSize": "12px"
            }
        });
        let source = format!(
            "---\nconfig: {config}\n---\nsequenceDiagram\nparticipant A as probe-actor\nA->>A: probe-message\nNote right of A: probe-note<br/>probe-note-second"
        );
        let observation = render_sequence_with_host_environment(
            &source,
            SequenceHostResponse::Missing,
            "sequence-family-fonts",
            RenderEnvironment::deterministic(),
        );
        let doc = roxmltree::Document::parse(&observation.render.svg).expect("Sequence SVG");
        for (probe, class, family, size, expected_count) in [
            ("probe-actor", "actor", "serif", 19.0, 2),
            ("probe-note", "noteText", "monospace", 32.0, 2),
            ("probe-message", "messageText", "sans-serif", 12.0, 1),
        ] {
            let size: f64 = override_size.unwrap_or(size);
            let requests = observation
                .requests
                .iter()
                .filter(|request| request.text.contains(probe))
                .collect::<Vec<_>>();
            assert!(!requests.is_empty(), "{config}: {probe} must be measured");
            for request in &requests {
                assert_eq!(
                    request.font_size_bits,
                    size.to_bits(),
                    "{config}: {probe} must use the same size in every measurement phase: {request:?}"
                );
            }
            assert!(
                requests
                    .iter()
                    .any(|request| request.font_family.as_deref() == Some(family)),
                "{config}: {probe} must reach its own configured font family"
            );
            let texts = doc
                .descendants()
                .filter(|node| {
                    node.has_tag_name("text")
                        && node
                            .attribute("class")
                            .is_some_and(|value| value.split_whitespace().any(|part| part == class))
                })
                .collect::<Vec<_>>();
            assert_eq!(texts.len(), expected_count, "{config}: {class}");
            for text in texts {
                let style = text.attribute("style").expect("inline font style");
                let expected_size = format!("{size}px");
                assert_eq!(
                    inline_style_value(style, "font-size"),
                    Some(expected_size.as_str()),
                    "{config}: {class} has the wrong size: {style}"
                );
                assert_eq!(
                    inline_style_value(style, "font-family"),
                    Some(family),
                    "{config}: {class} has the wrong family: {style}"
                );
            }
        }
    }
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

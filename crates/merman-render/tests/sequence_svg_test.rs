mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions, ParsedDiagramRender, RenderSemanticModel};
use merman_render::DiagramFamilyId;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontAssetSpec,
    FontCatalogSpec, FontStack, FontStyle, InsetsPx, OrdinalSelector, Specified,
    TextStylePatch as ThemeTextStylePatch, ThemeAssets, ThemePortabilityRequirement, ThemeRule,
    ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
};
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

fn sequence_role_typography_theme() -> DiagramTheme {
    let font_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    let role = |target, size, weight, style| {
        let typography = ThemeTextStylePatch {
            font_stack: Specified::Value(
                FontStack::single("Excalifont").expect("valid role font stack"),
            ),
            font_size_px: Specified::Value(size),
            font_weight: Specified::Value(weight),
            font_style: Specified::Value(style),
            ..ThemeTextStylePatch::default()
        };
        ThemeRule::new(
            target,
            ThemeStylePatch {
                typography,
                ..ThemeStylePatch::default()
            },
        )
        .for_family(DiagramFamilyId::SEQUENCE)
    };
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_styles(
                    ThemeRuleSet::default()
                        .with_rule(role(ThemeTarget::ActorLabel, 17.0, 500, FontStyle::Italic))
                        .with_rule(role(
                            ThemeTarget::MessageLabel,
                            18.0,
                            600,
                            FontStyle::Oblique,
                        ))
                        .with_rule(role(ThemeTarget::NoteLabel, 19.0, 700, FontStyle::Italic))
                        .with_rule(role(ThemeTarget::LoopLabel, 20.0, 800, FontStyle::Oblique)),
                )
                .with_assets(
                    ThemeAssets::default().with_font_catalog(FontCatalogSpec::new([
                        FontAssetSpec::new("excalifont", font_bytes),
                    ])),
                ),
        )
        .expect("compile Sequence role typography theme")
}

fn sequence_role_font_stack_theme() -> DiagramTheme {
    let font_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    let role = |target| {
        ThemeRule::new(
            target,
            ThemeStylePatch {
                typography: ThemeTextStylePatch {
                    font_stack: Specified::Value(
                        FontStack::single("monospace").expect("valid role font stack"),
                    ),
                    ..ThemeTextStylePatch::default()
                },
                ..ThemeStylePatch::default()
            },
        )
        .for_family(DiagramFamilyId::SEQUENCE)
    };
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_styles(
                    ThemeRuleSet::default()
                        .with_rule(role(ThemeTarget::ActorLabel))
                        .with_rule(role(ThemeTarget::MessageLabel))
                        .with_rule(role(ThemeTarget::NoteLabel))
                        .with_rule(role(ThemeTarget::LoopLabel)),
                )
                .with_assets(
                    ThemeAssets::default().with_font_catalog(FontCatalogSpec::new([
                        FontAssetSpec::new("excalifont", font_bytes),
                    ])),
                ),
        )
        .expect("compile Sequence role font-stack theme")
}

fn sequence_excalifont_asset_theme() -> DiagramTheme {
    let font_bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_assets(ThemeAssets::default().with_font_catalog(
                FontCatalogSpec::new([FontAssetSpec::new("excalifont", font_bytes)]),
            )),
        )
        .expect("compile Sequence Excalifont asset theme")
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
                    .with_rule(role_fill(ThemeTarget::Loop))
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Loop,
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
fn sequence_neo_headless_strokes_share_typed_endpoint_geometry() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "look": "neo"
    })));
    let svg = render_sequence_svg_from_text_with_engine(
        engine,
        r#"sequenceDiagram
participant A
participant B
A->B: Headless solid
A-->B: Headless dotted"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Sequence SVG");
    let endpoints = |id: &str| {
        let message = document
            .descendants()
            .find(|node| node.is_element() && node.attribute("data-id") == Some(id))
            .unwrap_or_else(|| panic!("missing Sequence message {id}: {svg}"));
        let coordinate = |name: &str| {
            message
                .attribute(name)
                .unwrap_or_else(|| panic!("missing {name} for Sequence message {id}: {svg}"))
                .parse::<f64>()
                .unwrap_or_else(|_| panic!("invalid {name} for Sequence message {id}: {svg}"))
        };
        (coordinate("x1"), coordinate("x2"))
    };

    assert_eq!(endpoints("i0"), endpoints("i1"));
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
        let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
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
            ThemeTarget::Loop,
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
            ThemeTarget::Loop,
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
        ("base", ThemeTarget::Loop, "mainBkg", false, true),
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
        ("dark", ThemeTarget::Loop, "border1", true, true),
        ("forest", ThemeTarget::Actor, "mainBkg", false, true),
        ("forest", ThemeTarget::Lifeline, "mainBkg", true, true),
        ("forest", ThemeTarget::Loop, "mainBkg", false, true),
        (
            "forest",
            ThemeTarget::LoopLabel,
            "actorTextColor",
            false,
            true,
        ),
        ("neutral", ThemeTarget::Actor, "mainBkg", false, true),
        ("neutral", ThemeTarget::Lifeline, "border1", true, true),
        ("neutral", ThemeTarget::Loop, "mainBkg", false, true),
        ("default", ThemeTarget::Lifeline, "actorBorder", true, false),
        ("dark", ThemeTarget::Lifeline, "actorBorder", true, false),
        ("default", ThemeTarget::Loop, "actorBkg", false, false),
        ("dark", ThemeTarget::Loop, "actorBkg", false, false),
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
            (theme_id, ThemeTarget::Loop, "mainBkg", false, true),
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
                ThemeTarget::Loop,
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
            (theme_id, ThemeTarget::Loop, "primaryColor", false, false),
            (theme_id, ThemeTarget::Loop, "nodeBkg", false, false),
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
                ThemeTarget::Loop if stroke => "labelBoxBorderColor",
                ThemeTarget::Loop => "labelBoxBkgColor",
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
fn sequence_role_typography_is_shared_by_layout_preparation_and_terminal_svg() {
    let theme = sequence_role_typography_theme();
    let source = r#"sequenceDiagram
autonumber
box Team Role
participant Alice as Alice Role
participant Bob as Bob Role
end
link Alice: A Very Long Actor Popup Role Label @ https://example.test/role
Alice->>Bob: Message Role
Note over Alice,Bob: Note Role
loop Loop Role
Bob-->>Alice: Reply Role
end"#;
    let measurement_parsed =
        merman_render::__private::install_parse_compatibility(&theme, Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse themed Sequence source")
            .expect("detect themed Sequence source");
    let host = Arc::new(RecordingSequenceHost::new(SequenceHostResponse::Missing));
    let measurement_identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("sequence-role-typography").expect("valid profile id"),
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
        .expect("begin Sequence role typography measurement session");
    let _measurement_artifact = family::prepare(
        measurement_parsed,
        &LayoutOptions::default(),
        measurement_session,
    )
    .expect("prepare Sequence role typography measurement artifact");
    let requests = host.snapshot();
    for (text_fragment, size, weight, style) in [
        ("Team Role", 17.0, "500", "italic"),
        ("Alice Role", 17.0, "500", "italic"),
        ("A Very Long Actor Popup Role Label", 17.0, "500", "italic"),
        ("Message Role", 18.0, "600", "oblique"),
        ("Note Role", 19.0, "700", "italic"),
        ("Loop Role", 20.0, "800", "oblique"),
    ] {
        assert!(
            requests.iter().any(|exchange| {
                let request = &exchange.request;
                request.text.contains(text_fragment)
                    && request.font_family.as_deref() == Some("Excalifont")
                    && request.font_size_bits == f64::to_bits(size)
                    && request.font_weight.as_deref() == Some(weight)
                    && request.font_style.as_deref() == Some(style)
            }),
            "layout measurement must consume the resolved {text_fragment:?} role typography"
        );
    }

    let render_parsed =
        merman_render::__private::install_parse_compatibility(&theme, Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse portable themed Sequence source")
            .expect("detect portable themed Sequence source");
    let render_session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin portable Sequence role typography session");
    let rendered = family::prepare(render_parsed, &LayoutOptions::default(), render_session)
        .expect("prepare portable Sequence role typography")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("terminal Sequence labels should seal every role typography rule");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Sequence SVG");

    assert_sequence_role_text_style(
        &document,
        "actor-box",
        "Alice Role",
        "Excalifont",
        "17px",
        "500",
        "italic",
    );
    assert_sequence_role_text_style(
        &document,
        "text",
        "Team Role",
        "Excalifont",
        "17px",
        "500",
        "italic",
    );
    assert_sequence_role_text_style(
        &document,
        "messageText",
        "Message Role",
        "Excalifont",
        "18px",
        "600",
        "oblique",
    );
    assert_sequence_role_text_style(
        &document,
        "noteText",
        "Note Role",
        "Excalifont",
        "19px",
        "700",
        "italic",
    );
    assert_sequence_role_text_style(
        &document,
        "loopText",
        "Loop Role",
        "Excalifont",
        "20px",
        "800",
        "oblique",
    );
    assert_sequence_role_text_style(
        &document,
        "actor",
        "A Very Long Actor Popup Role Label",
        "Excalifont",
        "17px",
        "500",
        "italic",
    );

    let popup_panel = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_ascii_whitespace()
                        .any(|class| class == "actorPopupMenuPanel")
                })
        })
        .expect("actor popup panel");
    let panel_x = popup_panel
        .attribute("x")
        .and_then(|value| value.parse::<f64>().ok())
        .expect("numeric actor popup x");
    let panel_width = popup_panel
        .attribute("width")
        .and_then(|value| value.parse::<f64>().ok())
        .expect("numeric actor popup width");
    assert!(
        panel_width > 150.0,
        "ActorLabel measurement must widen the popup beyond the default actor width: {panel_width}"
    );
    let (view_box, _) = root_view_box_and_max_width(rendered.svg());
    assert!(
        panel_x + panel_width <= view_box[0] + view_box[2],
        "actor popup must remain inside the measured root viewBox"
    );

    let sequence_number = document
        .descendants()
        .find(|node| node.has_tag_name("text") && node.attribute("class") == Some("sequenceNumber"))
        .expect("autonumber terminal label");
    let inline = sequence_number
        .attribute("style")
        .expect("autonumber role typography style");
    assert_eq!(
        inline_style_value(inline, "font-family"),
        Some("Excalifont")
    );
    assert_eq!(inline_style_value(inline, "font-weight"), Some("600"));
    assert_eq!(inline_style_value(inline, "font-style"), Some("oblique"));
    assert_eq!(sequence_number.attribute("font-size"), Some("12px"));
}

#[test]
fn sequence_role_typography_overrides_base_without_shadowing_other_roles() {
    let latin = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
    ));
    let cjk = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/themes/assets/fonts/Xiaolai-Regular-CJK-Test.woff2"
    ));
    let base = ThemeTextStyle::default()
        .with_font_stack(FontStack::single("Excalifont").expect("valid base font stack"))
        .with_font_size_px(23.0)
        .expect("valid base font size");
    let message_stack =
        FontStack::new(["Xiaolai SC", "Excalifont"]).expect("valid message font stack");
    let message_stack_css = message_stack.as_css();
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default().with_family_style(DiagramFamilyId::SEQUENCE, base),
                )
                .with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::MessageLabel,
                            ThemeStylePatch {
                                typography: ThemeTextStylePatch {
                                    font_stack: Specified::Value(message_stack),
                                    font_size_px: Specified::Value(31.0),
                                    ..ThemeTextStylePatch::default()
                                },
                                ..ThemeStylePatch::default()
                            },
                        )
                        .for_family(DiagramFamilyId::SEQUENCE),
                    ),
                )
                .with_assets(
                    ThemeAssets::default().with_font_catalog(FontCatalogSpec::new([
                        FontAssetSpec::new("excalifont", latin),
                        FontAssetSpec::new("xiaolai", cjk),
                    ])),
                ),
        )
        .expect("compile Sequence base plus role typography theme");
    let source = r#"sequenceDiagram
participant Alice as Base Actor
participant Bob as Base Peer
Alice->>Bob: 测试
Note over Alice,Bob: Base Note
loop Base Loop
Bob-->>Alice: Base Reply
end"#;

    let host = Arc::new(RecordingSequenceHost::new(SequenceHostResponse::Missing));
    let measurement_policy = TextMeasurementPolicy::host_display(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("sequence-base-role-typography")
                .expect("valid measurement profile id"),
            "1",
        )
        .expect("valid measurement profile identity"),
        host.clone(),
        TextMeasurementPhase::ALL,
    );
    let measurement_parsed =
        merman_render::__private::install_parse_compatibility(&theme, Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse measured Sequence base plus role source")
            .expect("detect measured Sequence base plus role source");
    family::prepare(
        measurement_parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic()
            .with_text_measurement_policy(measurement_policy)
            .begin_session_with_theme(&theme)
            .expect("begin Sequence base plus role measurement session"),
    )
    .expect("prepare measured Sequence base plus role artifact");
    let requests = host.snapshot();
    assert!(requests.iter().any(|exchange| {
        exchange.request.text.contains("测试")
            && exchange.request.font_family.as_deref() == Some(message_stack_css.as_str())
            && exchange.request.font_size_bits == 31.0_f64.to_bits()
    }));
    assert!(requests.iter().any(|exchange| {
        exchange.request.text.contains("Base Actor")
            && exchange.request.font_family.as_deref() == Some("Excalifont")
            && exchange.request.font_size_bits == 23.0_f64.to_bits()
    }));

    let render_parsed =
        merman_render::__private::install_parse_compatibility(&theme, Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse portable Sequence base plus role source")
            .expect("detect portable Sequence base plus role source");
    let rendered = family::prepare(
        render_parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin portable Sequence base plus role session"),
    )
    .expect("prepare portable Sequence base plus role artifact")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("base and role Sequence typography must both seal terminal evidence");
    let document = roxmltree::Document::parse(rendered.svg())
        .expect("valid Sequence base plus role typography SVG");
    let style_for = |class_name: &str, text_fragment: &str| {
        document
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
                        .filter_map(|descendant| descendant.text())
                        .any(|text| text.contains(text_fragment))
            })
            .and_then(|node| node.attribute("style"))
            .unwrap_or_else(|| panic!("missing {class_name} style for {text_fragment:?}"))
    };
    let message = style_for("messageText", "测试");
    assert!(
        message.contains(r#"font-family:"Xiaolai SC", "Excalifont""#),
        "terminal MessageLabel must preserve the role-specific admitted stack: {message}"
    );
    assert_eq!(inline_style_value(message, "font-size"), Some("31px"));
    let actor = style_for("actor-box", "Base Actor");
    assert_eq!(inline_style_value(actor, "font-family"), Some("Excalifont"));
    assert_eq!(inline_style_value(actor, "font-size"), Some("23px"));
    drop(document);

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 3);
    assert_eq!(evidence.accounted_count(), 3);
    assert_eq!(evidence.applied_count(), 3);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn sequence_role_font_stacks_yield_to_explicit_theme_variable_font_family() {
    const SOURCE: &str = r#"sequenceDiagram
participant Alice as Config Actor
participant Bob
Alice->>Bob: Config Message
Note over Alice,Bob: Config Note
loop Config Loop
Bob-->>Alice: Reply
end"#;
    const CONFIG_FONT: &str = "Excalifont";
    let theme = sequence_role_font_stack_theme();

    for origin in ["site", "source"] {
        let config = serde_json::json!({
            "themeVariables": {"fontFamily": CONFIG_FONT},
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
                panic!("parse {origin} Sequence font ownership source: {error}")
            })
            .unwrap_or_else(|| panic!("detect {origin} Sequence font ownership source"));
        let host = Arc::new(RecordingSequenceHost::new(SequenceHostResponse::Missing));
        let measurement_identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new(format!("sequence-theme-variable-font-{origin}"))
                .expect("valid profile id"),
            "1",
        )
        .expect("valid measurement profile identity");
        let measurement_policy = TextMeasurementPolicy::host_display(
            measurement_identity,
            host.clone(),
            TextMeasurementPhase::ALL,
        );
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(measurement_policy)
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .unwrap_or_else(|error| {
                panic!("begin {origin} Sequence font ownership session: {error}")
            });
        let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
            .unwrap_or_else(|error| {
                panic!("prepare {origin} Sequence font ownership theme: {error}")
            })
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap_or_else(|error| {
                panic!("render {origin} Sequence font ownership theme: {error}")
            });

        let requests = host.snapshot();
        for text_fragment in [
            "Config Actor",
            "Config Message",
            "Config Note",
            "Config Loop",
        ] {
            let matching = requests
                .iter()
                .filter(|exchange| exchange.request.text.contains(text_fragment))
                .collect::<Vec<_>>();
            assert!(
                !matching.is_empty(),
                "expected {origin} measurement requests for {text_fragment:?}"
            );
            assert!(
                matching.iter().all(|exchange| {
                    exchange.request.font_family.as_deref() != Some("monospace")
                }),
                "{origin} themeVariables.fontFamily must suppress the typed role stack during measurement: {matching:#?}"
            );
        }

        let document = roxmltree::Document::parse(rendered.svg())
            .unwrap_or_else(|error| panic!("parse {origin} Sequence font ownership SVG: {error}"));
        for (class_name, text_fragment) in [
            ("actor-box", "Config Actor"),
            ("messageText", "Config Message"),
            ("noteText", "Config Note"),
            ("loopText", "Config Loop"),
        ] {
            let text = document
                .descendants()
                .find(|node| {
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
                })
                .unwrap_or_else(|| {
                    panic!("missing {origin} {class_name} text containing {text_fragment:?}")
                });
            let inline = text
                .attribute("style")
                .unwrap_or_else(|| panic!("missing {origin} {class_name} inline style"));
            assert_eq!(
                inline_style_value(inline, "font-family"),
                Some(CONFIG_FONT),
                "{origin} explicit themeVariables.fontFamily must beat the typed role stack"
            );
        }
        drop(document);

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 4, "{origin}");
        assert_eq!(evidence.accounted_count(), 4, "{origin}");
        assert_eq!(evidence.applied_count(), 0, "{origin}");
        assert_eq!(evidence.not_applicable_count(), 4, "{origin}");
        assert_eq!(evidence.theme_residual_count(), 0, "{origin}");
    }
}

#[test]
fn sequence_explicit_root_font_size_wins_over_role_typography() {
    let theme = sequence_role_typography_theme();
    let source = r#"%%{init: {"fontSize": 23}}%%
sequenceDiagram
participant Alice as Alice Role
participant Bob as Bob Role
Alice->>Bob: Message Role
Note over Alice,Bob: Note Role
loop Loop Role
Bob-->>Alice: Reply Role
end"#;
    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse configured themed Sequence source")
        .expect("detect configured themed Sequence source");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin configured portable Sequence role typography session");
    let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare configured Sequence role typography")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("explicit font size should leave the remaining direct role facets portable");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid configured SVG");

    for (class_name, text_fragment, weight, style) in [
        ("actor-box", "Alice Role", "500", "italic"),
        ("messageText", "Message Role", "600", "oblique"),
        ("noteText", "Note Role", "700", "italic"),
        ("loopText", "Loop Role", "800", "oblique"),
    ] {
        assert_sequence_role_text_style(
            &document,
            class_name,
            text_fragment,
            "Excalifont",
            "23px",
            weight,
            style,
        );
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

    let theme = sequence_excalifont_asset_theme();
    let render_engine =
        merman_render::__private::install_parse_compatibility(&theme, engine.clone());
    let rendered = family::prepare(
        parse_sequence_for_render(&render_engine, source),
        &LayoutOptions::default(),
        RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .expect("begin portable Sequence CSSOM font session"),
    )
    .expect("prepare portable Sequence CSSOM font artifact")
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("render portable Sequence CSSOM font SVG");
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid Sequence CSSOM font SVG");

    for (class_name, text_fragment) in [
        ("actor-box", "CSSOM Actor"),
        ("messageText", "CSSOM Message"),
        ("noteText", "CSSOM Note"),
        ("loopText", "CSSOM Loop"),
    ] {
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
        let style = text
            .attribute("style")
            .expect("prepared Sequence terminal text style");
        assert_eq!(
            inline_style_value(style, "font-family"),
            Some("Excalifont"),
            "prepared terminal text must use the CSSOM-effective inherited theme font"
        );
    }
}

#[test]
fn sequence_config_owned_note_weight_is_reasserted_after_legacy_tspan_css() {
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
        "the surviving legacy init value must outrank the typed NoteLabel default"
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
                && exchange.request.font_weight.as_deref() == Some("600")
        }),
        "themeVariables.noteFontWeight must own NoteLabel layout measurement"
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
        role_rule.contains("font-weight:600"),
        "config-owned NoteLabel weight must be reasserted after legacy child CSS: {role_rule}"
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
        Some("600")
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

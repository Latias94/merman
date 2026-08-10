use super::{AnalysisDiagnosticPolicy, AnalysisOptions, Analyzer, DiagramCaptureInput};
use crate::rules::{AnalysisRuleConfig, AnalysisRuleProfile};
use crate::{
    AnalysisCancellationToken, AnalysisStatus, DiagnosticCategory, DiagnosticSeverity,
    FenceTextIndexSource, SourceDescriptor, SourceMap,
};
use merman_core::{
    EditorSemanticDiagnostic, MermaidConfig, ParseMetadata, ParsedDiagram, SourceSpan,
};
use serde_json::json;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

static REPROJECTION_PARSE_CALLS: AtomicUsize = AtomicUsize::new(0);
static DERIVED_ANALYZER_PARSE_CALLS: AtomicUsize = AtomicUsize::new(0);
static CAPTURED_CONFIG_PANIC_CALLS: AtomicUsize = AtomicUsize::new(0);
static MARKDOWN_CUSTOM_PARSE_CALLS: AtomicUsize = AtomicUsize::new(0);
static MARKDOWN_PARTIAL_CAPTURE_CALLS: AtomicUsize = AtomicUsize::new(0);
static REJECTED_SOURCE_PARSE_CALLS: AtomicUsize = AtomicUsize::new(0);

fn detect_captured_config_fixture(source: &str, _config: &mut merman_core::MermaidConfig) -> bool {
    source.trim_start().starts_with("captured-config-fixture")
}

fn detect_markdown_custom_fixture(source: &str, _config: &mut merman_core::MermaidConfig) -> bool {
    source.trim_start().starts_with("markdown-custom-fixture")
}

fn captured_config_panicking_parser(
    _source: &str,
    _metadata: &ParseMetadata,
    _control: &merman_core::ParseControl,
) -> merman_core::ParseControlResult<merman_core::Result<serde_json::Value>> {
    CAPTURED_CONFIG_PANIC_CALLS.fetch_add(1, Ordering::SeqCst);
    panic!("captured config fixture panic")
}

fn non_string_panicking_parser(
    _source: &str,
    _metadata: &ParseMetadata,
    _control: &merman_core::ParseControl,
) -> merman_core::ParseControlResult<merman_core::Result<serde_json::Value>> {
    std::panic::panic_any(42_u8)
}

fn unknown_warning_flowchart_parser(
    _source: &str,
    _metadata: &ParseMetadata,
    control: &merman_core::ParseControl,
) -> merman_core::ParseControlResult<merman_core::Result<serde_json::Value>> {
    control.checkpoint()?;
    Ok(Ok(json!({
        "warningFacts": [{
            "ruleId": "fixture.unknown_warning_rule",
            "message": "fixture warning",
            "span": { "start": 0, "end": 1 },
        }],
    })))
}

fn counting_flowchart_parser(
    _source: &str,
    _metadata: &ParseMetadata,
    control: &merman_core::ParseControl,
) -> merman_core::ParseControlResult<merman_core::Result<serde_json::Value>> {
    control.checkpoint()?;
    REPROJECTION_PARSE_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(Ok(json!({ "warningFacts": [] })))
}

fn derived_analyzer_counting_flowchart_parser(
    _source: &str,
    _metadata: &ParseMetadata,
    control: &merman_core::ParseControl,
) -> merman_core::ParseControlResult<merman_core::Result<serde_json::Value>> {
    control.checkpoint()?;
    DERIVED_ANALYZER_PARSE_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(Ok(json!({ "warningFacts": [] })))
}

fn markdown_counting_custom_parser(
    _source: &str,
    _metadata: &ParseMetadata,
    control: &merman_core::ParseControl,
) -> merman_core::ParseControlResult<merman_core::Result<serde_json::Value>> {
    control.checkpoint()?;
    MARKDOWN_CUSTOM_PARSE_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(Ok(json!({ "warningFacts": [] })))
}

fn markdown_first_success_then_cancel_parser(
    _source: &str,
    _metadata: &ParseMetadata,
    control: &merman_core::ParseControl,
) -> merman_core::ParseControlResult<merman_core::Result<serde_json::Value>> {
    control.checkpoint()?;
    if MARKDOWN_PARTIAL_CAPTURE_CALLS.fetch_add(1, Ordering::SeqCst) == 0 {
        Ok(Ok(json!({ "warningFacts": [] })))
    } else {
        Err(merman_core::ParseCancelled)
    }
}

fn rejected_source_counting_parser(
    _source: &str,
    _metadata: &ParseMetadata,
    control: &merman_core::ParseControl,
) -> merman_core::ParseControlResult<merman_core::Result<serde_json::Value>> {
    control.checkpoint()?;
    REJECTED_SOURCE_PARSE_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(Ok(json!({ "warningFacts": [] })))
}

fn panicking_flowchart_parser(
    _source: &str,
    _metadata: &ParseMetadata,
    _control: &merman_core::ParseControl,
) -> merman_core::ParseControlResult<merman_core::Result<serde_json::Value>> {
    panic!("fixture parser panic")
}

fn malformed_flowchart_parser(
    _source: &str,
    _metadata: &ParseMetadata,
    control: &merman_core::ParseControl,
) -> merman_core::ParseControlResult<merman_core::Result<serde_json::Value>> {
    control.checkpoint()?;
    Ok(Ok(malformed_flowchart_parsed_diagram().model))
}

fn cancelling_flowchart_parser(
    _source: &str,
    _metadata: &ParseMetadata,
    control: &merman_core::ParseControl,
) -> merman_core::ParseControlResult<merman_core::Result<serde_json::Value>> {
    control.cancel();
    control.checkpoint()?;
    unreachable!("cancelled parser must stop at its checkpoint")
}

#[test]
fn custom_engine_uses_the_runtime_policy_owned_by_analysis_options() {
    let engine = merman_core::Engine::new().with_runtime_policy(
        merman_core::runtime::RuntimePolicy::deterministic().with_fixed_unix_millis(1_000),
    );
    let options = AnalysisOptions::default().with_runtime_policy(
        merman_core::runtime::RuntimePolicy::deterministic().with_fixed_unix_millis(2_000),
    );

    let analyzer = Analyzer::with_engine(engine, options.clone());

    assert_eq!(analyzer.engine.runtime_policy(), options.runtime_policy());
}

#[test]
fn custom_engine_uses_the_exact_site_config_owned_by_analysis_options() {
    let source = "erDiagram\nA ||--o{ B : owns\n";
    let stale_engine = merman_core::Engine::new().with_site_config(MermaidConfig::from_value(
        json!({ "layout": "elk", "theme": "dark" }),
    ));

    for options in [
        AnalysisOptions::default(),
        AnalysisOptions::default()
            .with_site_config(MermaidConfig::from_value(json!({ "theme": "forest" }))),
    ] {
        let expected = Analyzer::with_options(options.clone()).analyze_facts(source);
        let actual = Analyzer::with_engine(stale_engine.clone(), options).analyze_facts(source);
        assert_eq!(actual, expected);
        assert_eq!(
            actual.diagrams[0].syntax.effective_layout.as_deref(),
            Some("dagre")
        );
    }
}

#[test]
fn analyzer_derivations_preserve_custom_registries_and_exact_identity_scope() {
    DERIVED_ANALYZER_PARSE_CALLS.store(0, Ordering::SeqCst);
    let mut engine = merman_core::Engine::new();
    engine
        .diagram_registry_mut()
        .insert("flowchart-v2", derived_analyzer_counting_flowchart_parser);
    let base = Analyzer::with_engine(engine, AnalysisOptions::default());
    let cloned = base.clone();
    let diagnostics = base.with_diagnostic_policy(AnalysisDiagnosticPolicy {
        rule_config: AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended),
    });

    assert_eq!(base.environment_identity(), cloned.environment_identity());
    assert_eq!(
        base.environment_identity(),
        diagnostics.environment_identity()
    );
    assert!(diagnostics.analyze("flowchart TD\nA-->B\n").valid);

    let mut snapshot_policy = base.options().snapshot_policy().clone();
    snapshot_policy.source = SourceDescriptor::diagram().with_path("file:///derived.mmd");
    snapshot_policy.resources.max_source_bytes = Some(4_096);
    snapshot_policy.runtime_policy =
        merman_core::runtime::RuntimePolicy::deterministic().with_fixed_unix_millis(42_000);
    let snapshot = base.with_snapshot_policy(snapshot_policy.clone());

    assert_ne!(base.environment_identity(), snapshot.environment_identity());
    let generation = snapshot
        .analyze_generation("flowchart TD\nA-->B\n")
        .into_ready()
        .expect("derived analyzer should preserve its custom parser registry");
    assert_eq!(DERIVED_ANALYZER_PARSE_CALLS.load(Ordering::SeqCst), 2);
    assert_eq!(
        generation.environment_identity(),
        snapshot.environment_identity()
    );
    assert_eq!(generation.source(), &snapshot_policy.source);
    assert_eq!(
        snapshot
            .options()
            .snapshot_policy()
            .resources
            .max_source_bytes,
        snapshot_policy.resources.max_source_bytes
    );
    assert_eq!(
        snapshot
            .options()
            .snapshot_policy()
            .runtime_policy
            .begin_operation()
            .unwrap()
            .unix_millis(),
        42_000
    );

    let other = Analyzer::with_engine(merman_core::Engine::new(), snapshot.options().clone());
    assert_ne!(
        snapshot.environment_identity(),
        other.environment_identity()
    );
}

#[test]
fn analysis_generation_releases_parse_only_site_config_storage() {
    let mut site_config = MermaidConfig::from_value(json!({
        "unobservedGenerationProbe": "x".repeat(1024 * 1024),
    }));
    let original_allocation = site_config.as_value() as *const serde_json::Value;
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default()
            .with_site_config(site_config.clone())
            .with_fixed_today(merman_core::time::CivilDate::new(2026, 7, 30)),
    );
    let generation = analyzer
        .analyze_generation("flowchart TD\nA-->B\n")
        .into_ready()
        .expect("probe source should produce a generation");

    drop(analyzer);
    let mutable_allocation = site_config.as_value_mut() as *mut serde_json::Value;

    assert_eq!(mutable_allocation.cast_const(), original_allocation);
    assert_eq!(generation.source(), &SourceDescriptor::diagram());
}

#[test]
fn cancellable_generation_capture_preserves_parser_cancellation_as_an_outcome() {
    let mut engine = merman_core::Engine::new();
    engine
        .diagram_registry_mut()
        .insert("flowchart-v2", cancelling_flowchart_parser);
    let analyzer = Analyzer::with_engine(engine, AnalysisOptions::default());
    let cancellation = AnalysisCancellationToken::new();

    assert!(matches!(
        analyzer.analyze_generation_shared_cancellable(
            Arc::<str>::from("flowchart TD\nA-->B\n"),
            &cancellation,
        ),
        Err(crate::AnalysisCancelled)
    ));
}

#[test]
fn cancellable_generation_source_limit_preflight_observes_cancellation() {
    let source = format!("flowchart TD\nA-->B\n{}", "🤓".repeat(4_096));
    let analyzer =
        Analyzer::with_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));
    let cancellation = AnalysisCancellationToken::new();
    cancellation.cancel_after_checkpoints(2);

    assert!(matches!(
        analyzer.analyze_generation_shared_cancellable(Arc::from(source), &cancellation),
        Err(crate::AnalysisCancelled)
    ));
}

#[test]
fn shared_cancellable_generation_reuses_the_caller_source_allocation() {
    let analyzer = Analyzer::new();
    let source = Arc::<str>::from("flowchart TD\nA-->B\n");
    let caller_source = Arc::clone(&source);
    let generation = analyzer
        .analyze_generation_shared_cancellable(source, &AnalysisCancellationToken::new())
        .expect("shared capture should not be cancelled")
        .into_ready()
        .expect("fixture should produce a generation");
    let retained_source = generation.source_map().shared_source().source_arc();

    assert!(Arc::ptr_eq(&caller_source, &retained_source));
}

#[test]
fn markdown_fence_source_map_reuses_the_host_source_allocation() {
    let source = Arc::<str>::from(concat!(
        "before\n",
        "```mermaid\n",
        "flowchart TD\n",
        "A-->B\n",
        "```\n",
    ));
    let descriptor = crate::source_descriptor_for_markdown_path(Some("fixture.md"));
    let document = crate::DocumentSource::new(Arc::clone(&source), descriptor);
    let diagram = &document.diagrams()[0];
    let local_map = super::source_map_for_diagram_cancellable(
        diagram,
        document.source_map(),
        &AnalysisCancellationToken::new(),
    )
    .expect("source-map construction should not be cancelled");

    assert_eq!(local_map.source(), diagram.text.as_str());
    assert!(local_map.shares_source_allocation_with(&diagram.text));
    assert!(Arc::ptr_eq(&source, &diagram.text.source_arc()));
    assert_eq!(local_map.line_col(0).unwrap().line, 1);
}

#[test]
fn blank_source_detection_observes_cancellation_inside_the_scan() {
    let source = "\u{2003}".repeat(16 * 1024);
    let cancellation = AnalysisCancellationToken::new();
    cancellation.cancel_after_checkpoints(1);

    assert!(matches!(
        super::source_is_blank_cancellable(&source, &cancellation),
        Err(crate::AnalysisCancelled)
    ));
}

#[test]
fn analysis_facts_project_canonical_effective_layout() {
    let cases = [
        (
            Analyzer::new(),
            "flowchart-elk TD\nA-->B\n",
            "flowchart-elk",
            "elk",
        ),
        (
            Analyzer::new(),
            "%%{init: {\"layout\": \"elk\"}}%%\nclassDiagram\nclass A\n",
            "class",
            "elk",
        ),
        (
            Analyzer::with_options(
                AnalysisOptions::default()
                    .with_site_config(MermaidConfig::from_value(json!({ "layout": "elk" }))),
            ),
            "erDiagram\nA ||--o{ B : owns\n",
            "er",
            "elk",
        ),
        (
            Analyzer::with_options(AnalysisOptions::default().with_site_config(
                MermaidConfig::from_value(json!({
                    "class": { "defaultRenderer": "elk" }
                })),
            )),
            "classDiagram\nclass A\n",
            "class",
            "dagre",
        ),
    ];

    for (analyzer, source, syntax_id, effective_layout) in cases {
        let payload = analyzer.analyze_facts(source);
        assert!(payload.valid, "{source}");
        let syntax = &payload.diagrams[0].syntax;
        assert_eq!(syntax.diagram_type.as_deref(), Some(syntax_id), "{source}");
        assert_eq!(
            syntax.effective_layout.as_deref(),
            Some(effective_layout),
            "{source}"
        );
    }
}

#[test]
fn parse_failures_retain_operation_effective_layout() {
    for (source, diagram_type, layout) in [
        ("flowchart-elk TD\nA[unterminated\n", "flowchart-elk", "elk"),
        ("swimlane-beta LR\nA[unterminated\n", "swimlane", "swimlane"),
        (
            "---\nconfig:\n  layout: elk\n---\nflowchart TD\nA[unterminated\n",
            "flowchart-v2",
            "elk",
        ),
    ] {
        let payload = Analyzer::new().analyze_facts(source);
        assert!(!payload.valid, "{source}");
        let syntax = &payload.diagrams[0].syntax;
        assert_eq!(syntax.diagram_type.as_deref(), Some(diagram_type));
        assert_eq!(syntax.effective_layout.as_deref(), Some(layout));
    }
}

#[test]
fn analyze_state_parse_failure_deduplicates_matching_recovery_diagnostic() {
    let analyzer = Analyzer::new();
    let source = "stateDiagram-v2\nIdle --> Running\nRunning -->";
    let payload = analyzer.analyze(source);

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    assert_eq!(payload.summary.warnings, 0);
    assert_eq!(payload.diagnostics.len(), 1);

    let parse_error = payload
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == "merman.parse.diagram_parse")
        .expect("parse error diagnostic");
    assert_eq!(parse_error.severity, DiagnosticSeverity::Error);
    assert_eq!(parse_error.category, DiagnosticCategory::Parse);
    assert_eq!(parse_error.diagram_type.as_deref(), Some("stateDiagram"));
    assert!(parse_error.related.iter().any(|related| {
        related
            .message
            .contains("Parser recovery produced the same syntax problem")
    }));
    assert!(
        payload
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.id != "merman.parse.recovered_editor_facts")
    );
}

#[test]
fn analyze_flowchart_parse_failure_deduplicates_matching_recovery_diagnostic() {
    let analyzer = Analyzer::new();
    let source = "flowchart TD\nA[unterminated";
    let payload = analyzer.analyze(source);

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    assert_eq!(payload.summary.warnings, 0);
    assert_eq!(payload.diagnostics.len(), 1);

    let diagnostic = &payload.diagnostics[0];
    assert_eq!(diagnostic.id, "merman.parse.diagram_parse");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.category, DiagnosticCategory::Parse);
    assert_eq!(diagnostic.diagram_type.as_deref(), Some("flowchart-v2"));
    assert_eq!(diagnostic.message, "Unterminated node label (missing `]`)");
}

#[test]
fn analyze_parse_failure_remaps_frontmatter_spans_and_deduplicates_recovery() {
    let source = "---\ntitle: T\n---\nflowchart TD\nA-->\n";
    assert_single_remapped_flowchart_parse_error(source, 5, 5);
}

#[test]
fn analyze_parse_failure_remaps_init_directive_spans_and_deduplicates_recovery() {
    let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->\n";
    assert_single_remapped_flowchart_parse_error(source, 3, 5);
}

#[test]
fn analyze_parse_failure_remaps_crlf_frontmatter_spans_and_deduplicates_recovery() {
    let source = "---\r\ntitle: T\r\n---\r\nflowchart TD\r\nA-->\r\n";
    assert_single_remapped_flowchart_parse_error(source, 5, 5);
}

#[test]
fn analyze_parse_failure_remaps_length_changing_entity_normalization() {
    let source = "---\ntitle: quoted\n---\nflowchart TD\nA[unterminated #quot;\n";
    let analyzer = Analyzer::new();
    let payload = analyzer.analyze(source);

    assert!(!payload.valid);
    let diagnostic = payload
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == crate::rules::DIAGRAM_PARSE_RULE_ID)
        .expect("parse error diagnostic");
    let span = diagnostic.span.as_ref().expect("exact parse span");
    assert_eq!(span.byte_start, source.find('[').unwrap());
    assert!(span.byte_end <= source.len());
    assert!(source[span.byte_start..span.byte_end].contains("#quot;"));
    assert!(!diagnostic.related.iter().any(|related| {
        related
            .message
            .contains("Parser did not report a precise source location")
    }));
}

fn assert_single_remapped_flowchart_parse_error(source: &str, line: usize, column: usize) {
    let analyzer = Analyzer::new();
    let payload = analyzer.analyze(source);

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    assert_eq!(payload.summary.warnings, 0);
    assert_eq!(payload.diagnostics.len(), 1);

    let diagnostic = &payload.diagnostics[0];
    assert_eq!(diagnostic.id, "merman.parse.diagram_parse");
    assert_eq!(diagnostic.diagram_type.as_deref(), Some("flowchart-v2"));
    let span = diagnostic.span.as_ref().expect("parse diagnostic span");
    assert_eq!(span.line, line);
    assert_eq!(span.column, column);
    assert_eq!(span.lsp_range.start.line, line - 1);
    assert_eq!(span.lsp_range.start.character, column - 1);
    assert!(diagnostic.related.iter().any(|related| {
        related
            .message
            .contains("Parser recovery produced the same syntax problem")
    }));
}

#[test]
fn diagnostics_only_capture_does_not_materialize_syntax_indexes() {
    let analyzer = Analyzer::new();
    let captured =
        analyzer.capture_local("flowchart TD\nA-->B\n", super::CaptureMode::DiagnosticsOnly);

    assert!(
        captured
            .project_diagnostics(analyzer.options().diagnostic_policy())
            .is_empty()
    );
    assert_eq!(
        captured.syntax.diagram_type.as_deref(),
        Some("flowchart-v2")
    );
    assert_eq!(captured.syntax.source(), FenceTextIndexSource::Unavailable);
    assert!(captured.syntax.flowchart.is_none());
    assert!(captured.syntax.text_index.node_ids().next().is_none());
    assert!(captured.syntax.text_index.semantic_items().is_empty());
}

#[test]
fn rich_capture_materializes_parser_syntax_indexes() {
    let analyzer = Analyzer::new();
    let captured = analyzer.capture_local("flowchart TD\nA-->B\n", super::CaptureMode::RichFacts);

    assert_eq!(
        captured.syntax.diagram_type.as_deref(),
        Some("flowchart-v2")
    );
    assert_eq!(
        captured.syntax.source(),
        FenceTextIndexSource::ParserComplete
    );
    assert!(captured.syntax.flowchart.is_some());
    assert!(
        captured
            .syntax
            .text_index
            .node_ids()
            .any(|node_id| node_id == "A")
    );
    assert!(
        captured
            .syntax
            .text_index
            .semantic_items()
            .iter()
            .any(|item| item.name == "A")
    );
}

#[test]
fn cynefin_self_loop_diagnostics_match_between_diagnostics_and_rich_analysis() {
    let source = concat!(
        "cynefin-beta\n",
        "  complex\n",
        "  complicated\n",
        "  complex --> complicated : \"Pattern emerges\"\n",
        "  complicated --> complicated : \"Self-loop\"\n",
    );
    let analyzer = Analyzer::new();

    let diagnostics_only = analyzer.analyze(source);
    let rich = analyzer
        .analyze_generation(source)
        .into_ready()
        .expect("Cynefin source should produce a rich analysis result");

    assert_eq!(
        diagnostics_only,
        rich.project(analyzer.options().diagnostic_policy())
    );
    assert_eq!(diagnostics_only.diagnostics.len(), 1);
    assert_eq!(
        diagnostics_only.diagnostics[0].id,
        crate::rules::RECOVERED_EDITOR_FACTS_RULE_ID
    );
    assert!(
        diagnostics_only.diagnostics[0]
            .message
            .contains("self-loop transition on domain \"complicated\" is skipped")
    );
}

#[test]
fn parse_disposition_is_independent_from_diagnostic_severity() {
    let parsed_source = concat!(
        "cynefin-beta\n",
        "  complex\n",
        "  complicated\n",
        "  complicated --> complicated : \"Self-loop\"\n",
    );
    let recovered_source = "flowchart TD\nA[unterminated\n";

    for severity in [
        DiagnosticSeverity::Error,
        DiagnosticSeverity::Warning,
        DiagnosticSeverity::Info,
        DiagnosticSeverity::Hint,
    ] {
        let parsed = Analyzer::with_options(
            AnalysisOptions::default().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_severity(crate::rules::RECOVERED_EDITOR_FACTS_RULE_ID, severity)
                    .unwrap(),
            ),
        )
        .analyze_facts(parsed_source);
        assert_eq!(
            parsed.diagrams[0].parse_disposition,
            crate::DiagramParseDisposition::Parsed,
            "parsed disposition changed for {severity:?}"
        );
        assert_eq!(
            parsed.valid,
            severity != DiagnosticSeverity::Error,
            "parsed diagnostic validity did not reflect {severity:?}"
        );

        let recovered = Analyzer::with_options(
            AnalysisOptions::default().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_severity(crate::rules::DIAGRAM_PARSE_RULE_ID, severity)
                    .unwrap(),
            ),
        )
        .analyze_facts(recovered_source);
        assert_eq!(
            recovered.diagrams[0].parse_disposition,
            crate::DiagramParseDisposition::Recovered,
            "recovered disposition changed for {severity:?}"
        );
        assert_eq!(
            recovered.valid,
            severity != DiagnosticSeverity::Error,
            "recovered diagnostic validity did not reflect {severity:?}"
        );
    }
}

#[test]
fn rich_capture_retains_flowchart_facts_projection_failure_candidate() {
    let analyzer = Analyzer::new();
    let source = "flowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);
    let parsed = malformed_flowchart_parsed_diagram();
    let captured = analyzer.analyze_parsed_diagram(
        DiagramCaptureInput {
            source_map: &source_map,
            metadata: &parsed.meta,
            editor_facts: None,
        },
        &parsed.model,
        Vec::new(),
        super::CaptureMode::RichFacts,
    );

    assert!(captured.syntax.flowchart.is_none());
    let diagnostics = captured.project_diagnostics(analyzer.options().diagnostic_policy());
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == crate::rules::FLOWCHART_FACTS_PROJECTION_RULE_ID)
        .expect("flowchart projection diagnostic");
    assert_eq!(diagnostic.category, DiagnosticCategory::Internal);
    assert_eq!(diagnostic.code, Some(AnalysisStatus::InternalError.code()));
    assert_eq!(diagnostic.diagram_type.as_deref(), Some("flowchart-v2"));
    assert!(
        diagnostic
            .message
            .contains("failed to project flowchart facts from parser model")
    );
}

#[test]
fn flowchart_facts_projection_observes_cancellation_inside_the_model_walk() {
    let analyzer = Analyzer::new();
    let source_map = SourceMap::new("flowchart TD\nA-->B\n");
    let model = json!({
        "type": "flowchart-v2",
        "nodes": vec![serde_json::Value::Null; 1_024],
    });
    let cancellation = AnalysisCancellationToken::new();
    cancellation.cancel_after_checkpoints(2);

    assert!(matches!(
        analyzer.flowchart_facts_projection_cancellable(
            &model,
            "flowchart-v2",
            &source_map,
            &cancellation,
        ),
        Err(crate::AnalysisCancelled)
    ));
}

#[test]
fn diagnostic_reprojection_reuses_the_canonical_flowchart_projection_failure() {
    let analyzer = Analyzer::new();
    let source = "flowchart TD\nA-->B\n";
    let source_descriptor = crate::SourceDescriptor::diagram();
    let text = Arc::<str>::from(source);
    let source_map = SourceMap::new(Arc::clone(&text));
    let document = crate::document::whole_document_diagram(Arc::clone(&text), &source_descriptor);
    let parsed = malformed_flowchart_parsed_diagram();
    let captured = analyzer.analyze_parsed_diagram(
        DiagramCaptureInput {
            source_map: &source_map,
            metadata: &parsed.meta,
            editor_facts: None,
        },
        &parsed.model,
        Vec::new(),
        super::CaptureMode::RichFacts,
    );
    let diagram = crate::AnalyzedDiagram::from_document_diagram(
        &document,
        captured.syntax,
        captured.candidates,
        crate::DiagramParseDisposition::Parsed,
    );
    let result = crate::AnalysisGeneration::new(source_map, vec![diagram], &analyzer);

    let reprojected = result.project(analyzer.options().diagnostic_policy());

    assert_eq!(
        reprojected
            .diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.id == crate::rules::FLOWCHART_FACTS_PROJECTION_RULE_ID
            })
            .count(),
        1
    );
}

#[test]
fn rich_capture_reports_editor_facts_preprocess_failure() {
    let analyzer = Analyzer::new();
    let source = "---\nconfig: [\n---\nflowchart TD\nA-->B\n";
    let result = analyzer
        .analyze_generation(source)
        .into_ready()
        .expect("source is within the analysis limit");

    let payload = result.project(analyzer.options().diagnostic_policy());
    let diagnostic = payload
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == crate::rules::INVALID_FRONT_MATTER_YAML_RULE_ID)
        .expect("editor facts preprocess diagnostic");
    assert_eq!(diagnostic.category, DiagnosticCategory::Config);
    assert_eq!(diagnostic.code, Some(AnalysisStatus::ParseError.code()));
    assert!(diagnostic.message.contains("invalid YAML front-matter"));
}

#[test]
fn preprocessing_failure_still_projects_later_source_config_evidence() {
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default().with_rule_config(
            crate::rules::AnalysisRuleConfig::default()
                .with_profile(crate::rules::AnalysisRuleProfile::Recommended),
        ),
    );
    let source = concat!(
        "---\n",
        "config: [\n",
        "---\n",
        "%%{ initialize: { theme: 'dark' } }%%\n",
        "flowchart TD\nA-->B\n",
    );
    let result = analyzer
        .analyze_generation(source)
        .into_ready()
        .expect("source is within the analysis limit");
    let payload = result.project(analyzer.options().diagnostic_policy());

    assert!(
        payload
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.id == crate::rules::INVALID_FRONT_MATTER_YAML_RULE_ID })
    );
    let source_config = payload
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID)
        .expect("later directive evidence must survive the preprocessing failure");
    let span = source_config.span.as_ref().expect("directive keyword span");
    assert_eq!(&source[span.byte_start..span.byte_end], "initialize");
    assert!(source_config.fixes.is_empty());
}

#[test]
fn diagnostics_only_capture_reports_the_canonical_flowchart_projection_failure() {
    let analyzer = Analyzer::new();
    let source = "flowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);
    let parsed = malformed_flowchart_parsed_diagram();
    let captured = analyzer.analyze_parsed_diagram(
        DiagramCaptureInput {
            source_map: &source_map,
            metadata: &parsed.meta,
            editor_facts: None,
        },
        &parsed.model,
        Vec::new(),
        super::CaptureMode::DiagnosticsOnly,
    );
    let diagnostics = captured.project_diagnostics(analyzer.options().diagnostic_policy());

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].id,
        crate::rules::FLOWCHART_FACTS_PROJECTION_RULE_ID
    );
    assert_eq!(
        captured.syntax.diagram_type.as_deref(),
        Some("flowchart-v2")
    );
    assert_eq!(captured.syntax.source(), FenceTextIndexSource::Unavailable);
    assert!(captured.syntax.flowchart.is_none());
}

#[test]
fn public_analysis_entries_share_flowchart_projection_diagnostics() {
    let mut engine = merman_core::Engine::new();
    engine
        .diagram_registry_mut()
        .insert("flowchart-v2", malformed_flowchart_parser);
    let analyzer = Analyzer::with_engine(engine, AnalysisOptions::default());
    let source = "flowchart TD\nA-->B\n";

    let diagnostics_only = analyzer.analyze(source);
    let rich = analyzer
        .analyze_generation(source)
        .into_ready()
        .expect("source is within the analysis limit");

    assert_eq!(
        diagnostics_only,
        rich.project(analyzer.options().diagnostic_policy())
    );
}

#[test]
fn non_cancellable_analysis_reports_custom_parser_cancellation_without_panicking() {
    let mut engine = merman_core::Engine::new();
    engine
        .diagram_registry_mut()
        .insert("flowchart-v2", cancelling_flowchart_parser);
    let analyzer = Analyzer::with_engine(engine, AnalysisOptions::default());
    let source = "flowchart TD\nA-->B\n";

    let diagnostics = analyzer.analyze(source);
    let result = analyzer
        .analyze_generation(source)
        .into_ready()
        .expect("parser cancellation is an analysis failure, not a source rejection");
    let facts = analyzer.analyze_facts(source);

    assert!(!diagnostics.valid);
    assert_eq!(diagnostics.diagnostics.len(), 1);
    assert_eq!(
        diagnostics.diagnostics[0].id,
        crate::rules::PARSER_CONTRACT_VIOLATION_RULE_ID
    );
    assert_eq!(
        diagnostics.diagnostics[0].category,
        DiagnosticCategory::Internal
    );
    assert_eq!(
        diagnostics.diagnostics[0].code,
        Some(AnalysisStatus::InternalError.code())
    );
    assert_eq!(
        result.project(analyzer.options().diagnostic_policy()),
        diagnostics
    );
    assert_eq!(facts.diagnostics, diagnostics.diagnostics);
}

#[test]
fn non_cancellable_document_analysis_reports_custom_parser_cancellation_without_panicking() {
    let mut engine = merman_core::Engine::new();
    engine
        .diagram_registry_mut()
        .insert("flowchart-v2", cancelling_flowchart_parser);
    let analyzer = Analyzer::with_engine(engine, AnalysisOptions::default());
    let source = crate::source_descriptor_for_markdown_path(Some("file:///tmp/cancelled.md"));
    let text = "```mermaid\nflowchart TD\nA-->B\n```\n";

    let diagnostics = crate::analyze_document(text, &analyzer, source.clone());
    let generation = crate::analyze_document_generation(text, &analyzer, source.clone())
        .into_ready()
        .expect("parser cancellation is an analysis failure, not a source rejection");
    let facts = crate::analyze_document_facts(text, &analyzer, source);

    assert!(!diagnostics.valid);
    assert_eq!(diagnostics.diagnostics.len(), 1);
    assert_eq!(
        diagnostics.diagnostics[0].id,
        crate::rules::PARSER_CONTRACT_VIOLATION_RULE_ID
    );
    assert_eq!(
        diagnostics.diagnostics[0].category,
        DiagnosticCategory::Internal
    );
    assert_eq!(
        diagnostics.diagnostics[0].code,
        Some(AnalysisStatus::InternalError.code())
    );
    assert_eq!(
        generation.project(analyzer.options().diagnostic_policy()),
        diagnostics
    );
    assert_eq!(facts.diagnostics, diagnostics.diagnostics);
}

#[test]
fn protected_resource_limit_rule_still_returns_hard_resource_diagnostic() {
    assert!(
        AnalysisRuleConfig::default()
            .with_rule_disabled(crate::rules::RESOURCE_LIMIT_RULE_ID)
            .is_err()
    );
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default()
            .with_max_source_bytes(Some(8))
            .with_rule_config(AnalysisRuleConfig::default()),
    );
    let rejection = match analyzer.analyze_generation("flowchart TD\nA-->B\n") {
        crate::AnalysisCaptureOutcome::Rejected(rejection) => rejection,
        crate::AnalysisCaptureOutcome::Ready(_) => {
            panic!("source over the configured limit should be rejected")
        }
    };
    let diagnostics = &rejection.payload().diagnostics;

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id, crate::rules::RESOURCE_LIMIT_RULE_ID);
}

#[test]
fn analyzer_rich_entry_points_honor_the_configured_markdown_source_kind() {
    let source = "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n";
    let descriptor = crate::source_descriptor_for_markdown_path(Some("fixture.md"));
    let limited = Analyzer::with_options(
        AnalysisOptions::default()
            .with_source(descriptor.clone())
            .with_max_document_diagrams(Some(0)),
    );

    let limited_outcome = limited.analyze_generation(source);
    let rejection = limited_outcome
        .rejection()
        .expect("the configured Markdown diagram budget must apply to Analyzer entry points");
    assert_eq!(
        rejection.resource_limit(),
        crate::AnalysisResourceLimit::DocumentDiagrams {
            observed_document_diagrams: 1,
            max_document_diagrams: 0,
        }
    );
    assert_eq!(&limited.analyze(source), rejection.payload());

    let analyzer = Analyzer::with_options(AnalysisOptions::default().with_source(descriptor));
    let generation = analyzer
        .analyze_generation(source)
        .into_ready()
        .expect("the Markdown source is within the default document budget");

    assert_eq!(
        generation.environment_identity(),
        analyzer.environment_identity()
    );
    assert_eq!(generation.source(), analyzer.options().source());
    assert_eq!(generation.diagrams().len(), 1);
    assert_eq!(
        generation.diagrams()[0].kind(),
        crate::DocumentDiagramKind::MermaidFence
    );
    assert_eq!(
        generation.diagrams()[0].text().as_str(),
        "flowchart TD\nA-->B\n"
    );
    assert_eq!(
        analyzer.analyze(source),
        generation.project(analyzer.options().diagnostic_policy())
    );
}

#[test]
fn markdown_capture_preserves_custom_registries_across_source_derivation() {
    MARKDOWN_CUSTOM_PARSE_CALLS.store(0, Ordering::SeqCst);
    let mut engine = merman_core::Engine::new();
    engine
        .registry_mut()
        .add_fn("markdown-custom-fixture", detect_markdown_custom_fixture);
    engine
        .diagram_registry_mut()
        .insert("markdown-custom-fixture", markdown_counting_custom_parser);
    let configured_source =
        crate::source_descriptor_for_markdown_path(Some("file:///configured.md"));
    let analyzer = Analyzer::with_engine(
        engine,
        AnalysisOptions::default().with_source(configured_source.clone()),
    );
    let text = "```mermaid\nmarkdown-custom-fixture\npayload\n```\n";

    let configured = analyzer
        .analyze_generation(text)
        .into_ready()
        .expect("configured Markdown capture should use the custom registry");
    assert_eq!(MARKDOWN_CUSTOM_PARSE_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(
        configured.environment_identity(),
        analyzer.environment_identity()
    );
    assert_eq!(configured.source(), &configured_source);

    let per_request_source =
        crate::source_descriptor_for_markdown_path(Some("file:///per-request.md"));
    let per_request =
        crate::analyze_document_generation(text, &analyzer, per_request_source.clone())
            .into_ready()
            .expect("per-request Markdown capture should preserve the custom registry");
    assert_eq!(MARKDOWN_CUSTOM_PARSE_CALLS.load(Ordering::SeqCst), 2);
    assert_ne!(
        per_request.environment_identity(),
        analyzer.environment_identity()
    );
    assert_eq!(per_request.source(), &per_request_source);
}

#[test]
fn cancellable_markdown_custom_parser_cancellation_has_no_partial_generation() {
    MARKDOWN_PARTIAL_CAPTURE_CALLS.store(0, Ordering::SeqCst);
    let mut engine = merman_core::Engine::new();
    engine
        .diagram_registry_mut()
        .insert("flowchart-v2", markdown_first_success_then_cancel_parser);
    let analyzer = Analyzer::with_engine(
        engine,
        AnalysisOptions::default().with_source(crate::source_descriptor_for_markdown_path(Some(
            "file:///cancelled.md",
        ))),
    );
    let cancellation = AnalysisCancellationToken::new();

    assert!(matches!(
        analyzer.analyze_generation_shared_cancellable(
            Arc::<str>::from(concat!(
                "```mermaid\nflowchart TD\nA-->B\n```\n",
                "```mermaid\nflowchart TD\nB-->C\n```\n",
            )),
            &cancellation,
        ),
        Err(crate::AnalysisCancelled)
    ));
    assert_eq!(MARKDOWN_PARTIAL_CAPTURE_CALLS.load(Ordering::SeqCst), 2);
}

#[test]
fn source_and_document_preflight_reject_before_custom_parsers_run() {
    REJECTED_SOURCE_PARSE_CALLS.store(0, Ordering::SeqCst);
    let mut engine = merman_core::Engine::new();
    engine
        .diagram_registry_mut()
        .insert("flowchart-v2", rejected_source_counting_parser);
    let source = Arc::<str>::from("flowchart TD\nA-->B\n");
    let cancellation = AnalysisCancellationToken::new();

    let diagram_analyzer = Analyzer::with_engine(
        engine.clone(),
        AnalysisOptions::default().with_max_source_bytes(Some(8)),
    );
    assert!(matches!(
        diagram_analyzer
            .analyze_generation_shared_cancellable(Arc::clone(&source), &cancellation)
            .expect("source preflight should not be cancelled"),
        crate::AnalysisCaptureOutcome::Rejected(_)
    ));

    let markdown_analyzer = Analyzer::with_engine(
        engine.clone(),
        AnalysisOptions::default()
            .with_max_source_bytes(Some(8))
            .with_source(crate::source_descriptor_for_markdown_path(Some(
                "file:///rejected.md",
            ))),
    );
    let markdown_source = Arc::<str>::from("```mermaid\nflowchart TD\nA-->B\n```\n");
    assert!(matches!(
        markdown_analyzer
            .analyze_generation_shared_cancellable(markdown_source, &cancellation)
            .expect("document preflight should not be cancelled"),
        crate::AnalysisCaptureOutcome::Rejected(_)
    ));

    let diagram_limit_analyzer = Analyzer::with_engine(
        engine,
        AnalysisOptions::default()
            .with_max_document_diagrams(Some(1))
            .with_source(crate::source_descriptor_for_markdown_path(Some(
                "file:///too-many-diagrams.md",
            ))),
    );
    let two_fences = Arc::<str>::from(concat!(
        "```mermaid\nflowchart TD\nA-->B\n```\n",
        "```mermaid\nflowchart TD\nB-->C\n```\n",
    ));
    assert!(matches!(
        diagram_limit_analyzer
            .analyze_generation_shared_cancellable(two_fences, &cancellation)
            .expect("diagram-count preflight should not be cancelled"),
        crate::AnalysisCaptureOutcome::Rejected(_)
    ));
    assert_eq!(REJECTED_SOURCE_PARSE_CALLS.load(Ordering::SeqCst), 0);
}

#[test]
fn frontmatter_alias_materialization_budget_suppresses_only_the_migration_fix() {
    let scalar = "x".repeat(4 * 1024);
    let aliases = std::iter::repeat_n("*blob", 280)
        .collect::<Vec<_>>()
        .join(", ");
    let padding = "p".repeat(80 * 1024);
    let source = format!(
        concat!(
            "---\n",
            "blob: &blob \"{}\"\n",
            "copies: [{}]\n",
            "config:\n  theme: default\n",
            "# {}\n",
            "---\n",
            "%%{{ init: {{ theme: 'dark' }} }}%%\n",
            "flowchart TD\nA-->B\n",
        ),
        scalar, aliases, padding,
    );
    let analyzer = Analyzer::with_options(AnalysisOptions::default().with_rule_config(
        AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended),
    ));

    let payload = analyzer.analyze(&source);
    let migration = payload
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID)
        .expect("the authoring diagnostic must survive an advisory fix budget rejection");

    assert!(payload.valid);
    assert!(migration.fixes.is_empty());
}

fn malformed_flowchart_parsed_diagram() -> ParsedDiagram {
    ParsedDiagram {
        meta: ParseMetadata {
            diagram_type: "flowchart-v2".to_string(),
            config: MermaidConfig::default(),
            effective_config: MermaidConfig::default(),
            title: None,
        },
        model: json!({
            "type": "flowchart-v2",
            "nodes": [
                { "id": 1 }
            ]
        }),
    }
}

#[test]
fn fallback_recovery_merge_uses_structured_location_metadata() {
    let source_map = SourceMap::new("flowchart TD\nA[unterminated");
    let span = source_map.whole_source_span().unwrap();
    let mut primary = crate::diagnostic_projection::rule_diagnostic_without_default_span(
        crate::rules::DIAGRAM_PARSE_RULE_ID,
        AnalysisStatus::ParseError,
        "primary parser message",
        &AnalysisRuleConfig::default(),
    )
    .unwrap()
    .with_diagram_type("flowchart-v2")
    .with_span(span);
    let recovery = crate::diagnostic_projection::rule_diagnostic_without_default_span(
        crate::rules::RECOVERED_EDITOR_FACTS_RULE_ID,
        AnalysisStatus::ParseError,
        "recovered parser message",
        &AnalysisRuleConfig::default(),
    )
    .unwrap()
    .with_diagram_type("flowchart-v2")
    .with_span(span);
    assert!(crate::recovery::merge_duplicate_parse_recovery_diagnostic(
        &mut primary,
        0,
        &crate::recovery::AnalysisRecoveryDiagnostic::parser_backed(
            recovery,
            merman_core::EditorSemanticDiagnosticKind::ParserRecovery,
        ),
        Some(crate::diagnostic_projection::ParseDiagnosticLocation::Fallback),
    ));

    assert_eq!(primary.message, "primary parser message");
    assert!(primary.related.iter().any(|related| {
        related
            .message
            .contains("Parser recovery produced the same syntax problem")
    }));
}

#[test]
fn source_mapped_editor_recovery_diagnostics_keep_original_spans() {
    let source = "sequenceDiagram\nAlice->>Bob: Hello\nBob->>";
    let bob = source.rfind("Bob").expect("Bob reference");
    let source_map = SourceMap::new(source);
    let diagnostics = crate::recovery::editor_recovery_diagnostics(
        vec![EditorSemanticDiagnostic::parser_recovery(
            "unexpected end of input",
            Some(SourceSpan::new(bob, bob + "Bob".len())),
        )],
        "sequence",
        &source_map,
        &AnalysisRuleConfig::default(),
    );

    let span = diagnostics[0]
        .diagnostic
        .span
        .as_ref()
        .expect("source span");
    assert_eq!(&source[span.byte_start..span.byte_end], "Bob");
}

#[test]
fn analyze_init_directive_alias_emits_safe_fix() {
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default()
                .with_profile(AnalysisRuleProfile::Recommended)
                .with_rule_disabled(crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID)
                .unwrap(),
        ),
    );
    let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
    let payload = analyzer.analyze(source);

    assert!(payload.valid);
    assert_eq!(payload.summary.hints, 1);
    let diagnostic = payload
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID)
        .expect("init directive alias diagnostic");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Hint);
    assert_eq!(diagnostic.category, DiagnosticCategory::Config);
    let span = diagnostic.span.as_ref().expect("keyword span");
    assert_eq!(&source[span.byte_start..span.byte_end], "initialize");
    assert_eq!(diagnostic.fixes.len(), 1);
    assert_eq!(diagnostic.fixes[0].edits[0].replacement, "init");
}

#[test]
fn analysis_rule_config_can_disable_source_lints() {
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default()
                .with_profile(AnalysisRuleProfile::Recommended)
                .with_rule_disabled(crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID)
                .unwrap()
                .with_rule_disabled(crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID)
                .unwrap(),
        ),
    );
    let payload =
        analyzer.analyze("%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n");

    assert!(payload.valid);
    assert!(payload.diagnostics.is_empty());
}

#[test]
fn analysis_rule_config_can_disable_no_diagram_rule() {
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_disabled(crate::rules::NO_DIAGRAM_RULE_ID)
                .unwrap(),
        ),
    );
    let payload = analyzer.analyze("");

    assert!(payload.valid);
    assert!(payload.diagnostics.is_empty());
}

#[test]
fn analysis_rule_config_cannot_disable_resource_limit_rule() {
    assert!(
        AnalysisRuleConfig::default()
            .with_rule_disabled(crate::rules::RESOURCE_LIMIT_RULE_ID)
            .is_err()
    );
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default()
            .with_max_source_bytes(Some(8))
            .with_rule_config(AnalysisRuleConfig::default()),
    );
    let payload = analyzer.analyze("flowchart TD\nA-->B\n");

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    assert_eq!(payload.diagnostics.len(), 1);
    assert_eq!(
        payload.diagnostics[0].id,
        crate::rules::RESOURCE_LIMIT_RULE_ID
    );
}

#[test]
fn analysis_rule_config_cannot_override_resource_limit_severity() {
    assert!(
        AnalysisRuleConfig::default()
            .with_rule_severity(
                crate::rules::RESOURCE_LIMIT_RULE_ID,
                DiagnosticSeverity::Hint,
            )
            .is_err()
    );
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default()
            .with_max_source_bytes(Some(8))
            .with_rule_config(AnalysisRuleConfig::default()),
    );
    let payload = analyzer.analyze("flowchart TD\nA-->B\n");

    assert!(!payload.valid);
    assert_eq!(payload.summary.hints, 0);
    assert_eq!(payload.summary.errors, 1);
    assert_eq!(
        payload.diagnostics[0].id,
        crate::rules::RESOURCE_LIMIT_RULE_ID
    );
    assert_eq!(payload.diagnostics[0].severity, DiagnosticSeverity::Error);
}

#[test]
fn analysis_rule_config_can_override_source_lint_severity() {
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default()
                .with_profile(AnalysisRuleProfile::Recommended)
                .with_rule_disabled(crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID)
                .unwrap()
                .with_rule_severity(
                    crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID,
                    DiagnosticSeverity::Warning,
                )
                .unwrap(),
        ),
    );
    let payload =
        analyzer.analyze("%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n");

    assert!(payload.valid);
    assert_eq!(payload.summary.hints, 0);
    assert_eq!(payload.summary.warnings, 1);
    assert_eq!(
        payload.diagnostics[0].id,
        crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID
    );
}

#[test]
fn rule_changes_reproject_from_one_parse_generation() {
    REPROJECTION_PARSE_CALLS.store(0, Ordering::SeqCst);
    let mut engine = merman_core::Engine::new();
    engine
        .diagram_registry_mut()
        .insert("flowchart-v2", counting_flowchart_parser);
    let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
    let base = Analyzer::with_engine(engine, AnalysisOptions::default());
    let result = base
        .analyze_generation(source)
        .into_ready()
        .expect("source is within the analysis limit");
    let candidates = result.diagrams()[0].diagnostic_candidates.as_ptr();
    let base_payload = result.project(base.options().diagnostic_policy());

    let enabled_analyzer = Analyzer::with_options(
        AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default()
                .with_profile(AnalysisRuleProfile::Recommended)
                .with_rule_disabled(crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID)
                .unwrap(),
        ),
    );
    let enabled = result.project(enabled_analyzer.options().diagnostic_policy());
    let disabled_analyzer = Analyzer::with_options(
        AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default()
                .with_profile(AnalysisRuleProfile::Recommended)
                .with_rule_disabled(crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID)
                .unwrap()
                .with_rule_disabled(crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID)
                .unwrap(),
        ),
    );
    let disabled = result.project(disabled_analyzer.options().diagnostic_policy());
    let severity_analyzer = Analyzer::with_options(
        AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default()
                .with_profile(AnalysisRuleProfile::Recommended)
                .with_rule_disabled(crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID)
                .unwrap()
                .with_rule_severity(
                    crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID,
                    DiagnosticSeverity::Error,
                )
                .unwrap(),
        ),
    );
    let severity = result.project(severity_analyzer.options().diagnostic_policy());

    assert_eq!(REPROJECTION_PARSE_CALLS.load(Ordering::SeqCst), 1);
    assert!(base_payload.valid);
    assert_eq!(
        result.diagrams()[0].diagnostic_candidates.as_ptr(),
        candidates
    );
    assert_eq!(enabled.diagnostics.len(), 1);
    assert_eq!(
        enabled.diagnostics[0].id,
        crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID
    );
    assert!(disabled.diagnostics.is_empty());
    assert_eq!(severity.diagnostics.len(), 1);
    assert_eq!(severity.diagnostics[0].severity, DiagnosticSeverity::Error);
}

#[test]
fn captured_config_survives_custom_parser_panic_without_a_second_engine() {
    CAPTURED_CONFIG_PANIC_CALLS.store(0, Ordering::SeqCst);
    let mut engine = merman_core::Engine::new();
    engine
        .registry_mut()
        .add_fn("captured-config-fixture", detect_captured_config_fixture);
    engine
        .diagram_registry_mut()
        .insert("captured-config-fixture", captured_config_panicking_parser);
    let rule_config = AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended);
    let analyzer = Analyzer::with_engine(
        engine,
        AnalysisOptions::default().with_rule_config(rule_config.clone()),
    );
    let source = concat!(
        "%%{ init: {\"theme\":\"dark\"} }%%\n",
        "captured-config-fixture\n",
    );

    let generation = analyzer
        .analyze_generation(source)
        .into_ready()
        .expect("fixture is within the source limit");
    let first = generation.project(&AnalysisDiagnosticPolicy {
        rule_config: rule_config.clone(),
    });
    let second = generation.project(&AnalysisDiagnosticPolicy { rule_config });

    assert_eq!(first, second);
    assert_eq!(CAPTURED_CONFIG_PANIC_CALLS.load(Ordering::SeqCst), 1);
    assert!(first.diagnostics.iter().any(|diagnostic| {
        diagnostic.id == crate::rules::PANIC_RULE_ID
            && diagnostic.message.contains("captured config fixture panic")
    }));
    let migration = first
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID)
        .expect("captured metadata should retain the authoring diagnostic");
    assert_eq!(migration.fixes.len(), 1);
    assert!(migration.fixes[0].is_preferred);
    assert!(migration.fixes[0].edits.iter().any(|edit| {
        edit.replacement.contains("config:") && edit.replacement.contains("theme: dark")
    }));
}

#[test]
fn recovered_incomplete_directive_does_not_create_a_new_migration_fix() {
    let analyzer = Analyzer::with_options(AnalysisOptions::default().with_rule_config(
        AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended),
    ));
    let source = concat!(
        "%%{ init: {\"theme\":\"dark\"} }%%\n",
        "%%{ malformed\n",
        "flowchart TD\n",
        "A-->B\n",
    );
    assert!(
        merman_core::Engine::new()
            .parse_metadata_sync(source)
            .is_err(),
        "the compatibility path only offered a fix after strict metadata capture",
    );

    let generation = analyzer
        .analyze_generation(source)
        .into_ready()
        .expect("directive recovery should retain a ready generation");
    let payload = generation.project(analyzer.options().diagnostic_policy());
    let migration = payload
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID)
        .expect("the authoring diagnostic remains visible without a preferred fix");

    assert!(migration.fixes.is_empty());
}

#[test]
fn non_string_parser_panic_preserves_the_public_fallback_message() {
    let mut engine = merman_core::Engine::new();
    engine
        .registry_mut()
        .add_fn("captured-config-fixture", detect_captured_config_fixture);
    engine
        .diagram_registry_mut()
        .insert("captured-config-fixture", non_string_panicking_parser);
    let analyzer = Analyzer::with_engine(engine, AnalysisOptions::default());

    let payload = analyzer.analyze("captured-config-fixture\npayload\n");
    let panic = payload
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == crate::rules::PANIC_RULE_ID)
        .expect("parser panics must remain public internal diagnostics");

    assert_eq!(panic.message, "panic while analyzing Mermaid source");
}

#[test]
fn policy_neutral_candidate_corpus_covers_the_rule_catalog() {
    struct CorpusCase {
        name: &'static str,
        analyzer: Analyzer,
        source: &'static str,
    }

    let mut unsupported_engine = merman_core::Engine::new();
    *unsupported_engine.diagram_registry_mut() = merman_core::diagram::DiagramRegistry::new();

    let mut panic_engine = merman_core::Engine::new();
    panic_engine
        .diagram_registry_mut()
        .insert("flowchart-v2", panicking_flowchart_parser);

    let mut cancelling_engine = merman_core::Engine::new();
    cancelling_engine
        .diagram_registry_mut()
        .insert("flowchart-v2", cancelling_flowchart_parser);

    let mut registry_gap_engine = merman_core::Engine::new();
    registry_gap_engine
        .diagram_registry_mut()
        .insert("flowchart-v2", unknown_warning_flowchart_parser);

    let mut malformed_flowchart_engine = merman_core::Engine::new();
    malformed_flowchart_engine
        .diagram_registry_mut()
        .insert("flowchart-v2", malformed_flowchart_parser);

    let invalid_theme_analyzer = Analyzer::with_options(
        AnalysisOptions::default()
            .with_site_config(MermaidConfig::from_value(json!({ "secure": [] }))),
    );
    let nested_directive_config = format!(
        "{}0{}",
        "[".repeat(merman_core::MAX_DIAGRAM_NESTING_DEPTH + 1),
        "]".repeat(merman_core::MAX_DIAGRAM_NESTING_DEPTH + 1),
    );
    let invalid_directive_source = Box::leak(
        format!("%%{{ init: {nested_directive_config} }}%%\nflowchart TD\nA-->B\n")
            .into_boxed_str(),
    );
    let cases = vec![
        CorpusCase {
            name: "source config rules",
            analyzer: Analyzer::new(),
            source: concat!(
                "%%{ initialize: {",
                "\"theme\":\"dark\",",
                "\"lazyLoadedDiagrams\":true,",
                "\"flowchart\":{\"htmlLabels\":true}",
                "} }%%\n",
                "flowchart TD\nA-->B\n",
            ),
        },
        CorpusCase {
            name: "empty source",
            analyzer: Analyzer::new(),
            source: "",
        },
        CorpusCase {
            name: "parse recovery",
            analyzer: Analyzer::new(),
            source: "flowchart TD\nA[unterminated",
        },
        CorpusCase {
            name: "unsupported family",
            analyzer: Analyzer::with_engine(unsupported_engine, AnalysisOptions::default()),
            source: "flowchart TD\nA-->B\n",
        },
        CorpusCase {
            name: "malformed frontmatter",
            analyzer: Analyzer::new(),
            source: "---\ntitle: missing terminator\nflowchart TD\nA-->B\n",
        },
        CorpusCase {
            name: "invalid directive json",
            analyzer: Analyzer::new(),
            source: invalid_directive_source,
        },
        CorpusCase {
            name: "invalid frontmatter yaml",
            analyzer: Analyzer::new(),
            source: "---\nconfig: [\n---\nflowchart TD\nA-->B\n",
        },
        CorpusCase {
            name: "invalid theme color",
            analyzer: invalid_theme_analyzer,
            source: concat!(
                "%%{ init: {",
                "\"themeVariables\":{\"primaryColor\":\"not-a-color\"}",
                "} }%%\n",
                "flowchart TD\nA-->B\n",
            ),
        },
        CorpusCase {
            name: "parser panic",
            analyzer: Analyzer::with_engine(panic_engine, AnalysisOptions::default()),
            source: "flowchart TD\nA-->B\n",
        },
        CorpusCase {
            name: "parser contract violation",
            analyzer: Analyzer::with_engine(cancelling_engine, AnalysisOptions::default()),
            source: "flowchart TD\nA-->B\n",
        },
        CorpusCase {
            name: "registry gap",
            analyzer: Analyzer::with_engine(registry_gap_engine, AnalysisOptions::default()),
            source: "flowchart TD\nA-->B\n",
        },
        CorpusCase {
            name: "flowchart facts projection",
            analyzer: Analyzer::with_engine(malformed_flowchart_engine, AnalysisOptions::default()),
            source: "flowchart TD\nA-->B\n",
        },
        CorpusCase {
            name: "block warnings",
            analyzer: Analyzer::new(),
            source: "block-beta\n  columns 1\n  A:1\n  B:2\n  C:3\n",
        },
        CorpusCase {
            name: "flowchart direction",
            analyzer: Analyzer::new(),
            source: "flowchart\nA-->B\n",
        },
        CorpusCase {
            name: "flowchart style target",
            analyzer: Analyzer::new(),
            source: "graph TD;style Q background:#fff;",
        },
        CorpusCase {
            name: "git graph duplicate",
            analyzer: Analyzer::new(),
            source: "gitGraph\ncommit id:\"duplicate\"\ncommit id:\"duplicate\"\n",
        },
    ];

    let mut strict_rules = AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Strict);
    strict_rules
        .disable_rule(crate::rules::FLOWCHART_EXPLICIT_DIRECTION_RULE_ID)
        .unwrap();
    strict_rules
        .set_rule_severity(crate::rules::BLOCK_WIDTH_RULE_ID, DiagnosticSeverity::Error)
        .unwrap();
    let policies = [
        AnalysisDiagnosticPolicy {
            rule_config: AnalysisRuleConfig::default(),
        },
        AnalysisDiagnosticPolicy {
            rule_config: AnalysisRuleConfig::default()
                .with_profile(AnalysisRuleProfile::Recommended),
        },
        AnalysisDiagnosticPolicy {
            rule_config: strict_rules,
        },
    ];
    let mut observed_rule_ids = BTreeSet::new();

    for case in cases {
        let generation = case
            .analyzer
            .analyze_generation(case.source)
            .into_ready()
            .unwrap_or_else(|_| panic!("{} should produce a ready generation", case.name));
        observed_rule_ids.extend(generation.diagnostic_candidate_rule_ids());

        for policy in &policies {
            let projected = generation.project(policy);
            let fresh = case
                .analyzer
                .with_diagnostic_policy(policy.clone())
                .analyze(case.source);
            assert_eq!(projected, fresh, "{} under {policy:?}", case.name);
        }
    }

    let resource_analyzer =
        Analyzer::with_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));
    let resource_source = "flowchart TD\nA-->B\n";
    let rejected = resource_analyzer.analyze_generation(resource_source);
    let rejection = rejected
        .rejection()
        .expect("resource limits should reject before generation construction");
    assert_eq!(
        rejection.payload(),
        &resource_analyzer.analyze(resource_source)
    );
    observed_rule_ids.extend(
        rejection
            .payload()
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.id.as_str()),
    );

    let document_resource_analyzer =
        Analyzer::with_options(AnalysisOptions::default().with_max_document_diagrams(Some(1)));
    let document_resource_source = concat!(
        "```mermaid\nflowchart TD\nA-->B\n```\n",
        "```mermaid\nsequenceDiagram\nA->>B: hi\n```\n",
    );
    let document_rejected = crate::document::analyze_document_generation(
        document_resource_source,
        &document_resource_analyzer,
        crate::source_descriptor_for_markdown_path(Some("catalog.md")),
    );
    observed_rule_ids.extend(
        document_rejected
            .rejection()
            .expect("document resources should reject before generation construction")
            .payload()
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.id.as_str()),
    );

    let catalog_rule_ids = crate::rules::rule_descriptors()
        .iter()
        .map(|descriptor| descriptor.id)
        .collect::<BTreeSet<_>>();
    assert_eq!(observed_rule_ids, catalog_rule_ids);
}

#[test]
fn config_authoring_rule_dominance_reprojects_all_four_enablement_combinations() {
    let analyzer = Analyzer::new();
    let generation = analyzer
        .analyze_generation("%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n")
        .into_ready()
        .expect("fixture is within the source limit");

    let severity_only = AnalysisRuleConfig::default()
        .with_rule_severity(
            crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID,
            DiagnosticSeverity::Error,
        )
        .unwrap()
        .with_rule_severity(
            crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID,
            DiagnosticSeverity::Error,
        )
        .unwrap();
    assert!(
        generation
            .project(&AnalysisDiagnosticPolicy {
                rule_config: severity_only,
            })
            .diagnostics
            .is_empty(),
        "severity overrides must not enable recommended rules",
    );

    for (frontmatter, init_alias, expected) in [
        (false, false, None),
        (
            false,
            true,
            Some(crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID),
        ),
        (
            true,
            false,
            Some(crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID),
        ),
        (
            true,
            true,
            Some(crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID),
        ),
    ] {
        let mut rules = AnalysisRuleConfig::default();
        if frontmatter {
            rules
                .enable_rule(crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID)
                .unwrap();
        }
        if init_alias {
            rules
                .enable_rule(crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID)
                .unwrap();
        }
        rules
            .set_rule_severity(
                crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID,
                DiagnosticSeverity::Info,
            )
            .unwrap();
        rules
            .set_rule_severity(
                crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID,
                DiagnosticSeverity::Warning,
            )
            .unwrap();

        let payload = generation.project(&AnalysisDiagnosticPolicy { rule_config: rules });
        assert_eq!(payload.diagnostics.len(), usize::from(expected.is_some()));
        if let Some(expected) = expected {
            let diagnostic = &payload.diagnostics[0];
            assert_eq!(diagnostic.id, expected);
            assert_eq!(
                diagnostic.severity,
                if frontmatter {
                    DiagnosticSeverity::Info
                } else {
                    DiagnosticSeverity::Warning
                }
            );
            assert_eq!(diagnostic.fixes.len(), 1);
        }
    }
}

#[test]
fn parse_and_recovery_rules_reproject_all_four_enablement_combinations() {
    let analyzer = Analyzer::new();
    let generation = analyzer
        .analyze_generation("flowchart TD\nA[unterminated")
        .into_ready()
        .expect("parse failures retain a generation");
    assert_eq!(
        generation.diagrams()[0].parse_disposition(),
        crate::DiagramParseDisposition::Recovered,
    );

    for (parse, recovery, expected_id, expected_valid) in [
        (false, false, None, true),
        (
            false,
            true,
            Some(crate::rules::RECOVERED_EDITOR_FACTS_RULE_ID),
            true,
        ),
        (
            true,
            false,
            Some(crate::rules::DIAGRAM_PARSE_RULE_ID),
            false,
        ),
        (true, true, Some(crate::rules::DIAGRAM_PARSE_RULE_ID), false),
    ] {
        let mut rules = AnalysisRuleConfig::default();
        if !parse {
            rules
                .disable_rule(crate::rules::DIAGRAM_PARSE_RULE_ID)
                .unwrap();
        }
        if !recovery {
            rules
                .disable_rule(crate::rules::RECOVERED_EDITOR_FACTS_RULE_ID)
                .unwrap();
        }

        let payload = generation.project(&AnalysisDiagnosticPolicy { rule_config: rules });
        assert_eq!(
            payload.valid, expected_valid,
            "parse={parse} recovery={recovery}"
        );
        assert_eq!(
            payload.diagnostics.len(),
            usize::from(expected_id.is_some())
        );
        if let Some(expected_id) = expected_id {
            assert_eq!(payload.diagnostics[0].id, expected_id);
            assert_eq!(
                payload.diagnostics[0]
                    .related
                    .iter()
                    .filter(|related| related.message.contains("Parser recovery produced"))
                    .count(),
                usize::from(parse && recovery)
            );
        }
    }
}

#[test]
fn markdown_fallback_recovery_is_scoped_per_fence_and_decorated_last() {
    let source = Arc::<str>::from(concat!(
        "before\n",
        "```mermaid\n",
        "flowchart TD\n",
        "A[unterminated\n",
        "```\n",
        "between\n",
        "```mermaid\n",
        "flowchart TD\n",
        "B[unterminated\n",
        "```\n",
    ));
    let descriptor = crate::source_descriptor_for_markdown_path(Some("fixture.md"));
    let document = crate::DocumentSource::new(Arc::clone(&source), descriptor.clone());
    let analyzer = Analyzer::with_options(AnalysisOptions::default().with_source(descriptor));
    let mut analyzed = Vec::new();

    for diagram in document.diagrams() {
        let local_map = SourceMap::new(diagram.text.as_str());
        let whole = local_map.whole_source_span().unwrap();
        let node_start = diagram
            .text
            .as_str()
            .find(if diagram.index == 0 { 'A' } else { 'B' })
            .unwrap();
        let node = local_map.span(node_start, node_start + 1).unwrap();
        let mut primary = crate::diagnostic_projection::rule_diagnostic_without_default_span(
            crate::rules::DIAGRAM_PARSE_RULE_ID,
            AnalysisStatus::ParseError,
            "fallback parse failure",
            crate::rules::capture_rule_config(),
        )
        .unwrap()
        .with_diagram_type("flowchart-v2")
        .with_span(whole);
        primary.related.push(crate::DiagnosticRelated {
            message: "Parser reported a fallback location for this syntax error.".to_string(),
            span: Some(whole),
        });
        let recovery = crate::diagnostic_projection::rule_diagnostic_without_default_span(
            crate::rules::RECOVERED_EDITOR_FACTS_RULE_ID,
            AnalysisStatus::ParseError,
            "recovery refinement",
            crate::rules::capture_rule_config(),
        )
        .unwrap()
        .with_diagram_type("flowchart-v2")
        .with_span(node);
        let candidates = vec![
            crate::diagnostic_projection::DiagnosticCandidate::new(primary).with_parse_location(
                Some(crate::diagnostic_projection::ParseDiagnosticLocation::Fallback),
            ),
            crate::diagnostic_projection::DiagnosticCandidate::new(recovery)
                .with_recovery_kind(merman_core::EditorSemanticDiagnosticKind::ParserRecovery),
        ];
        let candidates = crate::document::normalize_document_diagnostic_candidates(
            document.source_map(),
            diagram,
            candidates,
        );
        analyzed.push(crate::AnalyzedDiagram::from_document_diagram(
            diagram,
            crate::AnalysisSyntaxFacts::unavailable(Some("flowchart-v2".to_string())),
            candidates,
            crate::DiagramParseDisposition::Recovered,
        ));
    }

    let generation =
        crate::AnalysisGeneration::new(document.source_map().clone(), analyzed, &analyzer);
    let payload = generation.project(analyzer.options().diagnostic_policy());
    let reprojected = generation.project(analyzer.options().diagnostic_policy());

    assert_eq!(payload, reprojected);
    assert_eq!(payload.diagnostics.len(), 2);
    for (index, diagnostic) in payload.diagnostics.iter().enumerate() {
        assert_eq!(diagnostic.id, crate::rules::DIAGRAM_PARSE_RULE_ID);
        assert_eq!(
            diagnostic
                .related
                .iter()
                .filter(|related| related.message.contains("Parser recovery produced"))
                .count(),
            1,
        );
        assert_eq!(diagnostic.related.len(), 4);
        assert!(diagnostic.related[0].message.contains("fallback location"));
        assert!(
            diagnostic.related[1]
                .message
                .contains("original parse location")
        );
        assert!(
            diagnostic.related[2]
                .message
                .contains("Parser recovery produced")
        );
        assert_eq!(
            diagnostic.related[3].message,
            format!("Mermaid fence {}", index + 1)
        );
    }

    for (parse, recovery, expected_id, expected_recovery_contexts) in [
        (false, false, None, 0),
        (
            false,
            true,
            Some(crate::rules::RECOVERED_EDITOR_FACTS_RULE_ID),
            0,
        ),
        (true, false, Some(crate::rules::DIAGRAM_PARSE_RULE_ID), 0),
        (true, true, Some(crate::rules::DIAGRAM_PARSE_RULE_ID), 1),
    ] {
        let mut rules = AnalysisRuleConfig::default();
        if !parse {
            rules
                .disable_rule(crate::rules::DIAGRAM_PARSE_RULE_ID)
                .unwrap();
        }
        if !recovery {
            rules
                .disable_rule(crate::rules::RECOVERED_EDITOR_FACTS_RULE_ID)
                .unwrap();
        }

        let projected = generation.project(&AnalysisDiagnosticPolicy { rule_config: rules });
        assert_eq!(
            projected.diagnostics.len(),
            usize::from(expected_id.is_some()) * 2,
            "parse={parse} recovery={recovery}"
        );
        for (index, diagnostic) in projected.diagnostics.iter().enumerate() {
            let expected_fence_message = format!("Mermaid fence {}", index + 1);
            assert_eq!(diagnostic.id, expected_id.unwrap());
            assert_eq!(
                diagnostic
                    .related
                    .iter()
                    .filter(|related| related.message.contains("Parser recovery produced"))
                    .count(),
                expected_recovery_contexts
            );
            assert_eq!(
                diagnostic
                    .related
                    .iter()
                    .filter(|related| related.message == expected_fence_message)
                    .count(),
                1
            );
            assert_eq!(
                diagnostic.related.last().unwrap().message,
                expected_fence_message
            );
        }
    }
}

#[test]
fn parse_failure_reprojects_without_changing_diagnostics() {
    let analyzer = Analyzer::new();
    let result = analyzer
        .analyze_generation("flowchart TD\nA[unterminated")
        .into_ready()
        .expect("parse failures retain a canonical analysis generation");

    let payload = result.project(analyzer.options().diagnostic_policy());
    assert_eq!(payload, analyzer.analyze("flowchart TD\nA[unterminated"));
    assert_eq!(payload.diagnostics.len(), 1);
    assert_eq!(payload.diagnostics[0].id, "merman.parse.diagram_parse");
}

#[test]
fn parser_panic_reprojects_from_captured_evidence() {
    let mut engine = merman_core::Engine::new();
    engine
        .diagram_registry_mut()
        .insert("flowchart-v2", panicking_flowchart_parser);
    let analyzer = Analyzer::with_engine(engine, AnalysisOptions::default());
    let result = analyzer
        .analyze_generation("flowchart TD\nA-->B\n")
        .into_ready()
        .expect("parser panics retain a canonical analysis generation");

    let payload = result.project(analyzer.options().diagnostic_policy());
    assert_eq!(payload, analyzer.analyze("flowchart TD\nA-->B\n"));
    assert_eq!(payload.diagnostics.len(), 1);
    assert_eq!(payload.diagnostics[0].id, crate::rules::PANIC_RULE_ID);
    assert!(
        payload.diagnostics[0]
            .message
            .contains("fixture parser panic")
    );
}

#[test]
fn analysis_rule_config_can_disable_git_graph_warning_rules() {
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_disabled(crate::rules::GIT_GRAPH_DUPLICATE_COMMIT_RULE_ID)
                .unwrap(),
        ),
    );
    let payload =
        analyzer.analyze("gitGraph\ncommit id:\"working on MDR\"\ncommit id:\"working on MDR\"\n");

    assert!(payload.valid);
    assert!(payload.diagnostics.is_empty());
}

#[test]
fn analysis_rule_config_can_override_git_graph_warning_severity() {
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity(
                    crate::rules::GIT_GRAPH_DUPLICATE_COMMIT_RULE_ID,
                    DiagnosticSeverity::Hint,
                )
                .unwrap(),
        ),
    );
    let payload =
        analyzer.analyze("gitGraph\ncommit id:\"working on MDR\"\ncommit id:\"working on MDR\"\n");

    assert!(payload.valid);
    assert_eq!(payload.summary.hints, 1);
    assert_eq!(
        payload
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.id == crate::rules::GIT_GRAPH_DUPLICATE_COMMIT_RULE_ID)
            .count(),
        1
    );
    assert!(
        payload
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.severity == DiagnosticSeverity::Hint)
    );
}

#[test]
fn analysis_rule_registry_gap_surfaces_as_internal_error() {
    let source_map = SourceMap::new("flowchart TD\nA-->B\n");
    let diagnostic = crate::diagnostic_projection::rule_diagnostic(
        "merman.unknown.rule",
        AnalysisStatus::Panic,
        "rule ids must be registered",
        &source_map,
        &AnalysisRuleConfig::default(),
    )
    .expect("internal registry gap diagnostic");

    assert_eq!(
        diagnostic.id,
        crate::rules::INTERNAL_RULE_REGISTRY_GAP_RULE_ID
    );
    assert_eq!(diagnostic.category, DiagnosticCategory::Internal);
    assert_eq!(diagnostic.code, Some(AnalysisStatus::InternalError.code()));
    assert!(
        diagnostic
            .message
            .contains("unknown analysis rule id `merman.unknown.rule`")
    );
}

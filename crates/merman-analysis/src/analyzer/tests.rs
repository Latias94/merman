use super::{AnalysisDiagnosticPolicy, AnalysisOptions, Analyzer, DiagramProjectionInput};
use crate::rules::{AnalysisRuleConfig, AnalysisRuleProfile};
use crate::{
    AnalysisCancellationToken, AnalysisStatus, DiagnosticCategory, DiagnosticSeverity,
    FenceTextIndexSource, SourceDescriptor, SourceMap,
};
use merman_core::{
    EditorSemanticDiagnostic, MermaidConfig, ParseMetadata, ParsedDiagram, SourceSpan,
};
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

static REPROJECTION_PARSE_CALLS: AtomicUsize = AtomicUsize::new(0);
static DERIVED_ANALYZER_PARSE_CALLS: AtomicUsize = AtomicUsize::new(0);

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
    snapshot_policy.max_source_bytes = Some(4_096);
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
    assert_eq!(generation.snapshot_policy().source, snapshot_policy.source);
    assert_eq!(
        generation.snapshot_policy().max_source_bytes,
        snapshot_policy.max_source_bytes
    );
    assert_eq!(
        generation
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
fn cancellable_generation_capture_preserves_parser_cancellation_as_an_outcome() {
    let mut engine = merman_core::Engine::new();
    engine
        .diagram_registry_mut()
        .insert("flowchart-v2", cancelling_flowchart_parser);
    let analyzer = Analyzer::with_engine(engine, AnalysisOptions::default());
    let cancellation = AnalysisCancellationToken::new();

    assert!(matches!(
        analyzer.analyze_generation_cancellable("flowchart TD\nA-->B\n", &cancellation,),
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
fn diagnostics_mode_does_not_project_valid_syntax_facts() {
    let analyzer = Analyzer::new();
    let local = analyzer.analyze_local("flowchart TD\nA-->B\n", super::AnalysisMode::Diagnostics);

    assert!(local.diagnostics.is_empty());
    assert_eq!(local.syntax.diagram_type.as_deref(), Some("flowchart-v2"));
    assert_eq!(local.syntax.source(), FenceTextIndexSource::Unavailable);
    assert!(local.syntax.flowchart.is_none());
    assert!(local.syntax.text_index.node_ids().next().is_none());
    assert!(local.syntax.text_index.semantic_items().is_empty());
}

#[test]
fn rich_facts_mode_projects_valid_syntax_facts() {
    let analyzer = Analyzer::new();
    let local = analyzer.analyze_local("flowchart TD\nA-->B\n", super::AnalysisMode::RichFacts);

    assert!(local.diagnostics.is_empty());
    assert_eq!(local.syntax.diagram_type.as_deref(), Some("flowchart-v2"));
    assert_eq!(local.syntax.source(), FenceTextIndexSource::ParserComplete);
    assert!(local.syntax.flowchart.is_some());
    assert!(
        local
            .syntax
            .text_index
            .node_ids()
            .any(|node_id| node_id == "A")
    );
    assert!(
        local
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
fn rich_facts_mode_reports_flowchart_facts_projection_failure() {
    let analyzer = Analyzer::new();
    let source = "flowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);
    let parsed = malformed_flowchart_parsed_diagram();
    let local = analyzer.analyze_parsed_diagram(
        DiagramProjectionInput {
            source,
            source_map: &source_map,
            metadata: &parsed.meta,
            editor_facts: None,
        },
        &parsed.model,
        Vec::new(),
        super::AnalysisMode::RichFacts,
    );

    assert!(local.syntax.flowchart.is_none());
    let diagnostic = local
        .diagnostics
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
    let local = analyzer.analyze_parsed_diagram(
        DiagramProjectionInput {
            source,
            source_map: &source_map,
            metadata: &parsed.meta,
            editor_facts: None,
        },
        &parsed.model,
        Vec::new(),
        super::AnalysisMode::RichFacts,
    );
    let diagram = crate::AnalyzedDiagram::from_document_diagram_with_evidence(
        &document,
        local.syntax,
        Arc::new(crate::result::DiagramAnalysisEvidence::Parsed {
            metadata: parsed.meta,
            model: Arc::new(parsed.model),
            editor_facts: None,
        }),
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
fn rich_facts_mode_reports_editor_facts_preprocess_failure() {
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
fn diagnostics_mode_reports_the_canonical_flowchart_projection_failure() {
    let analyzer = Analyzer::new();
    let source = "flowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);
    let parsed = malformed_flowchart_parsed_diagram();
    let local = analyzer.analyze_parsed_diagram(
        DiagramProjectionInput {
            source,
            source_map: &source_map,
            metadata: &parsed.meta,
            editor_facts: None,
        },
        &parsed.model,
        Vec::new(),
        super::AnalysisMode::Diagnostics,
    );

    assert_eq!(local.diagnostics.len(), 1);
    assert_eq!(
        local.diagnostics[0].id,
        crate::rules::FLOWCHART_FACTS_PROJECTION_RULE_ID
    );
    assert_eq!(local.syntax.diagram_type.as_deref(), Some("flowchart-v2"));
    assert_eq!(local.syntax.source(), FenceTextIndexSource::Unavailable);
    assert!(local.syntax.flowchart.is_none());
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
        crate::rules::DIAGRAM_PARSE_RULE_ID
    );
    assert_eq!(
        result.project(analyzer.options().diagnostic_policy()),
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
    let local = analyzer.analyze_local("flowchart TD\nA-->B\n", super::AnalysisMode::RichFacts);

    assert_eq!(local.diagnostics.len(), 1);
    assert_eq!(
        local.diagnostics[0].id,
        crate::rules::RESOURCE_LIMIT_RULE_ID
    );
    assert_eq!(local.syntax.diagram_type, None);
    assert_eq!(local.syntax.source(), FenceTextIndexSource::Unavailable);
    assert!(local.syntax.text_index.node_ids().next().is_none());
    assert!(local.syntax.text_index.semantic_items().is_empty());
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
    let primary = crate::diagnostic_projection::rule_diagnostic_without_default_span(
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
    let mut diagnostics = vec![primary];

    crate::recovery::merge_recovery_diagnostics(
        &mut diagnostics,
        vec![crate::recovery::AnalysisRecoveryDiagnostic::parser_backed(
            recovery,
            merman_core::EditorSemanticDiagnosticKind::ParserRecovery,
        )],
        Some(crate::diagnostic_projection::ParseDiagnosticLocation::Fallback),
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].message, "primary parser message");
    assert!(diagnostics[0].related.iter().any(|related| {
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
    let evidence = std::sync::Arc::as_ptr(&result.diagrams()[0].evidence);
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
        std::sync::Arc::as_ptr(&result.diagrams()[0].evidence),
        evidence
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

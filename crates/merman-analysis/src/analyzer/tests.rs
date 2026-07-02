use super::{AnalysisOptions, Analyzer};
use crate::rules::{AnalysisRuleConfig, AnalysisRuleProfile};
use crate::{AnalysisStatus, DiagnosticCategory, DiagnosticSeverity, SourceMap};

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
fn fallback_recovery_merge_uses_structured_location_metadata() {
    let source_map = SourceMap::new("flowchart TD\nA[unterminated");
    let span = source_map.whole_source_span().unwrap();
    let primary = super::rule_diagnostic_without_default_span(
        crate::rules::DIAGRAM_PARSE_RULE_ID,
        AnalysisStatus::ParseError,
        "primary parser message",
        &AnalysisRuleConfig::default(),
    )
    .unwrap()
    .with_diagram_type("flowchart-v2")
    .with_span(span.clone());
    let recovery = super::rule_diagnostic_without_default_span(
        crate::rules::RECOVERED_EDITOR_FACTS_RULE_ID,
        AnalysisStatus::ParseError,
        "recovered parser message",
        &AnalysisRuleConfig::default(),
    )
    .unwrap()
    .with_diagram_type("flowchart-v2")
    .with_span(span);
    let mut diagnostics = vec![primary];

    super::merge_recovery_diagnostics(
        &mut diagnostics,
        vec![super::AnalysisRecoveryDiagnostic::parser_backed(
            recovery,
            merman_core::EditorSemanticDiagnosticKind::ParserRecovery,
        )],
        Some(super::ParseDiagnosticLocation::Fallback),
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
fn analyze_init_directive_alias_emits_safe_fix() {
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default()
                .with_profile(AnalysisRuleProfile::Recommended)
                .with_rule_disabled(crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID),
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
                .with_rule_disabled(crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID),
        ),
    );
    let payload =
        analyzer.analyze("%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n");

    assert!(payload.valid);
    assert!(payload.diagnostics.is_empty());
}

#[test]
fn analysis_rule_config_can_disable_no_diagram_rule() {
    let analyzer = Analyzer::with_options(AnalysisOptions::default().with_rule_config(
        AnalysisRuleConfig::default().with_rule_disabled(crate::rules::NO_DIAGRAM_RULE_ID),
    ));
    let payload = analyzer.analyze("");

    assert!(payload.valid);
    assert!(payload.diagnostics.is_empty());
}

#[test]
fn analysis_rule_config_can_disable_resource_limit_rule() {
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default()
            .with_max_source_bytes(Some(8))
            .with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_disabled(crate::rules::RESOURCE_LIMIT_RULE_ID),
            ),
    );
    let payload = analyzer.analyze("flowchart TD\nA-->B\n");

    assert!(payload.valid);
    assert!(payload.diagnostics.is_empty());
}

#[test]
fn analysis_rule_config_can_override_resource_limit_severity() {
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default()
            .with_max_source_bytes(Some(8))
            .with_rule_config(AnalysisRuleConfig::default().with_rule_severity(
                crate::rules::RESOURCE_LIMIT_RULE_ID,
                DiagnosticSeverity::Hint,
            )),
    );
    let payload = analyzer.analyze("flowchart TD\nA-->B\n");

    assert!(payload.valid);
    assert_eq!(payload.summary.hints, 1);
    assert_eq!(payload.summary.errors, 0);
    assert_eq!(
        payload.diagnostics[0].id,
        crate::rules::RESOURCE_LIMIT_RULE_ID
    );
    assert_eq!(payload.diagnostics[0].severity, DiagnosticSeverity::Hint);
}

#[test]
fn analysis_rule_config_can_override_source_lint_severity() {
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default()
                .with_profile(AnalysisRuleProfile::Recommended)
                .with_rule_disabled(crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID)
                .with_rule_severity(
                    crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID,
                    DiagnosticSeverity::Warning,
                ),
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
fn analysis_rule_config_can_disable_git_graph_warning_rules() {
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_disabled(crate::rules::GIT_GRAPH_DUPLICATE_COMMIT_RULE_ID),
        ),
    );
    let payload =
        analyzer.analyze("gitGraph\ncommit id:\"working on MDR\"\ncommit id:\"working on MDR\"\n");

    assert!(payload.valid);
    assert!(payload.diagnostics.is_empty());
}

#[test]
fn analysis_rule_config_can_override_git_graph_warning_severity() {
    let analyzer = Analyzer::with_options(AnalysisOptions::default().with_rule_config(
        AnalysisRuleConfig::default().with_rule_severity(
            crate::rules::GIT_GRAPH_DUPLICATE_COMMIT_RULE_ID,
            DiagnosticSeverity::Hint,
        ),
    ));
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
    let diagnostic = super::rule_diagnostic(
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

use crate::{
    AnalysisDiagnosticPolicy, AnalysisOptions, AnalysisRuleConfig, AnalysisRuleProfile, Analyzer,
    FixGeneration,
};
use std::sync::Arc;

fn recommended() -> AnalysisDiagnosticPolicy {
    AnalysisDiagnosticPolicy {
        rule_config: AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended),
        ..Default::default()
    }
}

const SOURCE: &str = concat!(
    "%%{ initialize: {theme: 'dark'} }%%\n",
    "%%{ init: {flowchart: {curve: 'linear'}} }%%\n",
    "flowchart TD\nA-->B\n",
);

#[test]
fn default_projection_omits_fixes_and_retains_the_original_generation() {
    assert_eq!(FixGeneration::default(), FixGeneration::Omit);
    assert_eq!(
        AnalysisOptions::default()
            .diagnostic_policy()
            .fix_generation,
        FixGeneration::Omit
    );
    let source: Arc<str> = Arc::from(SOURCE);
    let generation = Analyzer::new()
        .analyze_generation_shared(Arc::clone(&source))
        .into_ready()
        .unwrap();
    let candidates = generation.diagrams()[0].diagnostic_candidates.as_ptr();
    let identity = generation.environment_identity().clone();
    let first = generation.project(&recommended());
    let second = generation.project(&recommended());
    assert!(!first.diagnostics.is_empty());
    assert!(
        first
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.fixes.is_empty())
    );
    assert_eq!(first, second);
    assert_eq!(
        generation.diagrams()[0].diagnostic_candidates.as_ptr(),
        candidates
    );
    assert_eq!(generation.environment_identity(), &identity);
    assert!(Arc::ptr_eq(
        &generation.source_map().shared_source().source_arc(),
        &source
    ));
    assert!(Arc::ptr_eq(
        &generation.diagrams()[0].text().source_arc(),
        &source
    ));
}

#[cfg(feature = "fixes")]
#[test]
fn omit_then_generate_reuses_evidence_and_aggregates_edits_per_projection() {
    crate::test_support::reset_fix_work();
    let generation = Analyzer::new()
        .analyze_generation(SOURCE)
        .into_ready()
        .unwrap();
    assert_eq!(crate::test_support::fix_work(), Default::default());
    let weight = generation.estimated_owned_heap_bytes_excluding_source();
    let omitted = generation.project(&recommended());
    assert_eq!(crate::test_support::fix_work(), Default::default());
    let policy = recommended().with_fix_generation(FixGeneration::Generate);
    let generated = generation.project(&policy);
    assert_eq!(generated.diagnostics.len(), omitted.diagnostics.len());
    let mut without_fixes = generated.clone();
    for diagnostic in &mut without_fixes.diagnostics {
        diagnostic.fixes.clear();
    }
    assert_eq!(without_fixes, omitted);
    assert_eq!(crate::test_support::fix_work().materializations, 1);
    assert_eq!(crate::test_support::fix_work().serializations, 1);
    assert!(Arc::ptr_eq(
        &generated.diagnostics[0].fixes[0].edits,
        &generated.diagnostics[1].fixes[0].edits
    ));
    let again = generation.project(&policy);
    assert_eq!(generated, again);
    assert_eq!(crate::test_support::fix_work().serializations, 2);
    assert!(!Arc::ptr_eq(
        &generated.diagnostics[0].fixes[0].edits,
        &again.diagnostics[0].fixes[0].edits
    ));
    assert_eq!(
        generation.estimated_owned_heap_bytes_excluding_source(),
        weight
    );
}

#[cfg(feature = "fixes")]
#[test]
fn filtering_and_suppression_precede_fix_construction() {
    let generation = Analyzer::new()
        .analyze_generation(SOURCE)
        .into_ready()
        .unwrap();
    let disabled = AnalysisDiagnosticPolicy {
        rule_config: recommended()
            .rule_config
            .with_rule_disabled(crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID)
            .unwrap()
            .with_rule_disabled(crate::rules::PREFER_INIT_DIRECTIVE_RULE_ID)
            .unwrap(),
        fix_generation: FixGeneration::Generate,
    };
    crate::test_support::reset_fix_work();
    assert!(generation.project(&disabled).diagnostics.is_empty());
    assert_eq!(crate::test_support::fix_work(), Default::default());

    let alias = AnalysisDiagnosticPolicy {
        rule_config: recommended()
            .rule_config
            .with_rule_disabled(crate::rules::PREFER_FRONTMATTER_CONFIG_RULE_ID)
            .unwrap(),
        fix_generation: FixGeneration::Generate,
    };
    let payload = generation.project(&alias);
    assert_eq!(payload.diagnostics.len(), 1);
    assert_eq!(payload.diagnostics[0].fixes[0].edits[0].replacement, "init");
    assert_eq!(crate::test_support::fix_work().materializations, 1);
    assert_eq!(crate::test_support::fix_work().serializations, 0);
}

#[cfg(feature = "fixes")]
#[test]
fn fix_selection_changes_projection_without_invalidating_environment_identity() {
    let analyzer = Analyzer::new();
    let selected =
        analyzer.with_diagnostic_policy(recommended().with_fix_generation(FixGeneration::Generate));
    assert_eq!(
        analyzer.environment_identity(),
        selected.environment_identity()
    );
    assert_eq!(
        analyzer.options().snapshot_policy(),
        selected.options().snapshot_policy()
    );
    assert_eq!(
        analyzer.options().diagnostic_policy().fix_generation,
        FixGeneration::Omit
    );
}

#[cfg(feature = "fixes")]
#[test]
fn cancelled_fix_projection_leaves_generation_available_for_later_requests() {
    let generation = Analyzer::new()
        .analyze_generation(SOURCE)
        .into_ready()
        .unwrap();
    let candidates = generation.diagrams()[0].diagnostic_candidates.as_ptr();
    let cancellation = crate::AnalysisCancellationToken::new();
    cancellation.cancel_after_checkpoints(8);
    crate::test_support::reset_fix_work();
    assert!(
        generation
            .project_cancellable(
                &recommended().with_fix_generation(FixGeneration::Generate),
                &cancellation,
            )
            .is_err()
    );
    assert_eq!(crate::test_support::fix_work().materializations, 1);
    assert_eq!(
        generation.diagrams()[0].diagnostic_candidates.as_ptr(),
        candidates
    );
    assert!(
        generation
            .project(&recommended())
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.fixes.is_empty())
    );
    assert!(
        generation
            .project(&recommended().with_fix_generation(FixGeneration::Generate))
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.fixes.is_empty())
    );
}

#[cfg(feature = "fixes")]
#[test]
fn markdown_migrations_keep_original_crlf_unicode_coordinates_and_shared_edits() {
    let text = concat!(
        "préface 終\r\n```mermaid\r\n",
        "%%{ initialize: {theme: 'dark'} }%%\r\n",
        "%%{ init: {flowchart: {curve: 'linear'}} }%%\r\n",
        "flowchart TD\r\nA[終]-->B\r\n```\r\n",
    );
    let generation = crate::analyze_document_generation(
        text,
        &Analyzer::new(),
        crate::source_descriptor_for_markdown_path(Some("修复.md")),
    )
    .into_ready()
    .unwrap();
    let fence_id = generation.diagrams()[0].source_id().to_owned();
    let source = generation.diagrams()[0].text().source_arc();
    let omitted = generation.project(&recommended());
    crate::test_support::reset_fix_work();
    let payload = generation.project(&recommended().with_fix_generation(FixGeneration::Generate));
    assert_eq!(payload.diagnostics.len(), 2);
    let mut without_fixes = payload.clone();
    for diagnostic in &mut without_fixes.diagnostics {
        diagnostic.fixes.clear();
    }
    assert_eq!(without_fixes, omitted);
    assert_eq!(generation.diagrams()[0].source_id(), fence_id);
    assert!(Arc::ptr_eq(
        &source,
        &generation.diagrams()[0].text().source_arc()
    ));
    assert!(Arc::ptr_eq(
        &payload.diagnostics[0].fixes[0].edits,
        &payload.diagnostics[1].fixes[0].edits
    ));
    let body = generation.diagrams()[0].body_range();
    for edit in payload.diagnostics[0].fixes[0].edits.iter() {
        assert!(edit.span.byte_start >= body.start && edit.span.byte_end <= body.end);
        assert_eq!(
            edit.span,
            generation
                .source_map()
                .span(edit.span.byte_start, edit.span.byte_end)
                .unwrap()
        );
    }
    assert_eq!(crate::test_support::fix_work().serializations, 1);
}

#[cfg(feature = "fixes")]
#[test]
fn shared_repair_weight_counts_bounded_authored_data_once() {
    let source_map = crate::SourceMap::new(SOURCE);
    let source_config = crate::test_support::capture_source_config_evidence(SOURCE);
    let config = merman_model::MermaidConfig::from_value(
        serde_json::json!({"custom": "x".repeat(256 * 1024)}),
    );
    let capture = || {
        crate::repair::ConfigMigrationEvidence::capture(
            &source_map,
            &config,
            &source_config,
            &crate::AnalysisCancellationToken::new(),
        )
        .unwrap()
        .unwrap()
    };
    let shared = capture();
    let distinct: Vec<_> = (0..4).map(|_| capture()).collect();
    let mut shared_weight = crate::payload::DiagnosticRetainedWeight::default();
    let mut distinct_weight = crate::payload::DiagnosticRetainedWeight::default();
    for evidence in &distinct {
        shared_weight.add_repair_evidence(&crate::repair::RepairEvidence::config_migration(
            Arc::clone(&shared),
        ));
        distinct_weight.add_repair_evidence(&crate::repair::RepairEvidence::config_migration(
            Arc::clone(evidence),
        ));
    }
    assert!(distinct_weight.finish() > shared_weight.finish());
}

#[cfg(feature = "fixes")]
#[test]
fn invalid_document_mapping_drops_the_entire_aggregate_action() {
    let source_map = crate::SourceMap::new(SOURCE);
    let source_config = crate::test_support::capture_source_config_evidence(SOURCE);
    let config = merman_model::MermaidConfig::from_value(serde_json::json!({"theme": "dark"}));
    let cancellation = crate::AnalysisCancellationToken::new();
    let evidence = crate::repair::ConfigMigrationEvidence::capture(
        &source_map,
        &config,
        &source_config,
        &cancellation,
    )
    .unwrap()
    .unwrap();
    let partial_document = crate::SourceMap::new(&SOURCE[..SOURCE.find("%%{ init:").unwrap()]);
    let repair = crate::repair::RepairEvidence::config_migration(evidence)
        .map_to_document(&partial_document, 0, &cancellation)
        .unwrap()
        .unwrap();
    assert!(
        crate::repair::FixProjection::default()
            .materialize(&repair, &cancellation)
            .unwrap()
            .is_none()
    );
}

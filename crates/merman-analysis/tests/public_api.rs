use merman_analysis::{
    AnalysisCancellationToken, AnalysisOptions, AnalysisOptionsJson, AnalysisResourceLimit,
    Analyzer, LintOptionsJson, LintRuleSeverityOverrideJson, ResourceOptionsJson, SharedTextSlice,
    SourceDescriptor, SourceMap, analyze_document_generation_shared_cancellable,
};
use serde_json::json;
use std::sync::Arc;

#[test]
fn root_options_are_permissive_while_direct_nested_schemas_are_strict() {
    let root: AnalysisOptionsJson = serde_json::from_value(json!({
        "future_root": true,
        "lint": {
            "profile": "recommended",
            "future_lint": true,
            "rule_severities": [{
                "rule_id": "merman.parse.no_diagram",
                "severity": "warning",
                "future_override": true
            }]
        }
    }))
    .expect("the shared root format must ignore future root and lint fields");
    assert_eq!(root.lint.unwrap().profile.as_deref(), Some("recommended"));

    assert!(serde_json::from_value::<LintOptionsJson>(json!({ "future_lint": true })).is_err());
    assert!(
        serde_json::from_value::<LintRuleSeverityOverrideJson>(json!({
            "rule_id": "merman.parse.no_diagram",
            "severity": "warning",
            "future_override": true
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<ResourceOptionsJson>(json!({
            "limits": {},
            "future_resources": true
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<AnalysisOptionsJson>(json!({
            "resources": {
                "limits": {},
                "future_resources": true
            }
        }))
        .is_err(),
        "resources remain a strict versioned schema even through the root"
    );

    let resources: ResourceOptionsJson = serde_json::from_value(json!({
        "limits": {
            "max_source_bytes": 64,
            "max_document_diagrams": 2
        }
    }))
    .expect("the alpha.4 limits map is the supported resource shape");
    assert_eq!(resources.limits["max_source_bytes"], 64);
    assert_eq!(resources.limits["max_document_diagrams"], 2);
    assert!(
        serde_json::from_value::<ResourceOptionsJson>(json!({
            "max_source_bytes": 64
        }))
        .is_err(),
        "the alpha.3 resource shape must not decode silently"
    );
}

#[test]
fn shared_text_and_source_map_expose_behavior_without_copying() {
    let source: Arc<str> = Arc::from("a\n😀\n");
    let whole = SharedTextSlice::whole(Arc::clone(&source));
    let emoji = SharedTextSlice::from_range(Arc::clone(&source), 2, 6).unwrap();

    assert_eq!(whole.as_ref(), source.as_ref());
    assert_eq!(&*emoji, "😀");
    assert_eq!(emoji.start(), 2);
    assert_eq!(emoji.end(), 6);
    assert!(Arc::ptr_eq(&emoji.source_arc(), &source));
    assert_eq!(emoji.to_owned_text(), "😀");
    assert!(SharedTextSlice::from_range(Arc::clone(&source), 6, 2).is_none());
    assert!(SharedTextSlice::from_range(Arc::clone(&source), 3, 6).is_none());
    assert!(SharedTextSlice::from_range(Arc::clone(&source), 0, source.len() + 1).is_none());

    let map = SourceMap::new(Arc::clone(&source));
    assert_eq!(map.line_count(), 3);
    assert_eq!(map.line_start(0), Some(0));
    assert_eq!(map.line_start(1), Some(2));
    assert_eq!(map.line_start(2), Some(7));
    assert_eq!(map.line_start(3), None);
    assert_eq!(map.line_bounds(0), Some((0, 1)));
    assert_eq!(map.line_bounds(1), Some((2, 6)));
    assert_eq!(map.line_bounds(2), Some((7, 7)));
    assert!(Arc::ptr_eq(&map.shared_source().source_arc(), &source));
}

#[test]
fn cancellable_generation_capture_reuses_caller_owned_text() {
    let source: Arc<str> = Arc::from("flowchart TD\nA-->B\n");
    let outcome = Analyzer::new()
        .analyze_generation_shared_cancellable(
            Arc::clone(&source),
            &AnalysisCancellationToken::new(),
        )
        .unwrap();
    let generation = outcome.as_ready().expect("the source should be accepted");

    assert!(Arc::ptr_eq(
        &generation.source_map().shared_source().source_arc(),
        &source
    ));
}

#[test]
fn cancellable_document_capture_reuses_caller_owned_text() {
    let source: Arc<str> = Arc::from("flowchart TD\nA-->B\n");
    let outcome = analyze_document_generation_shared_cancellable(
        Arc::clone(&source),
        &Analyzer::new(),
        SourceDescriptor::diagram().with_path("file:///diagram.mmd"),
        &AnalysisCancellationToken::new(),
    )
    .unwrap();
    let generation = outcome.as_ready().expect("the document should be accepted");

    assert!(Arc::ptr_eq(
        &generation.source_map().shared_source().source_arc(),
        &source
    ));
}

#[test]
fn rejection_accessors_preserve_the_typed_limit_and_payload() {
    let analyzer =
        Analyzer::with_options(AnalysisOptions::default().with_max_source_bytes(Some(1)));
    let outcome = analyzer
        .analyze_generation_shared_cancellable(
            Arc::<str>::from("ab"),
            &AnalysisCancellationToken::new(),
        )
        .unwrap();
    let rejection = outcome.rejection().expect("the source must be rejected");
    let limit = rejection.resource_limit();

    assert_eq!(limit.id(), "max_source_bytes");
    assert_eq!(limit.observed(), 2);
    assert_eq!(limit.maximum(), 1);
    assert!(matches!(limit, AnalysisResourceLimit::SourceBytes { .. }));
    assert!(!rejection.payload().diagnostics.is_empty());

    let rejection = outcome.into_ready().unwrap_err();
    let payload = rejection.into_payload();
    assert!(!payload.diagnostics.is_empty());
}

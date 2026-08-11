use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{
    AnalyzerOptionsPreparation, ConfigurationUpdateOutcome,
    DEFAULT_LSP_ANALYSIS_CACHE_BUDGET_BYTES, DEFAULT_LSP_MAX_DOCUMENT_DIAGRAMS,
    DEFAULT_LSP_MAX_SOURCE_BYTES, DocumentDiagnosticState, DocumentSource, SessionState,
    SnapshotConfigurationPlan, StoredDocument, default_lsp_analysis_options,
    prepare_document_source_cancellable,
};
use merman_analysis::{
    AnalysisCancellationToken, AnalysisConfigChange, AnalysisOptions, AnalysisResourceLimit,
    AnalysisRuleConfig, Analyzer, DiagnosticSeverity, source_limit_diagnostic_span,
};
use merman_editor_core::DocumentKind;
use tower_lsp_server::ls_types::{Position, Range, TextDocumentContentChangeEvent, Uri};

static CUSTOM_ASYNC_SESSION_PARSE_CALLS: AtomicUsize = AtomicUsize::new(0);

fn custom_session_flowchart_parser(
    _source: &str,
    _metadata: &merman_core::ParseMetadata,
    control: &merman_core::ParseControl,
) -> merman_core::ParseControlResult<merman_core::Result<serde_json::Value>> {
    control.checkpoint()?;
    Ok(Ok(serde_json::json!({ "warningFacts": [] })))
}

fn custom_async_session_flowchart_parser(
    _source: &str,
    _metadata: &merman_core::ParseMetadata,
    control: &merman_core::ParseControl,
) -> merman_core::ParseControlResult<merman_core::Result<serde_json::Value>> {
    control.checkpoint()?;
    CUSTOM_ASYNC_SESSION_PARSE_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(Ok(serde_json::json!({ "warningFacts": [] })))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceState {
    Available,
    AnalysisRejected,
    ResourceLimited {
        source_len: usize,
        last_max_source_bytes: usize,
        span: merman_analysis::DiagnosticSpan,
        incremental_sync_lost: bool,
    },
    Discarded {
        source_len: usize,
        last_max_source_bytes: usize,
        span: merman_analysis::DiagnosticSpan,
        incremental_sync_lost: bool,
    },
    SyncError,
}

fn source_state(document: &StoredDocument) -> SourceState {
    match &document.source {
        DocumentSource::Available(_) => SourceState::Available,
        DocumentSource::AnalysisRejected { .. } => SourceState::AnalysisRejected,
        DocumentSource::ResourceLimited(evidence) => SourceState::ResourceLimited {
            source_len: evidence.source_len,
            last_max_source_bytes: evidence.last_max_source_bytes,
            span: evidence.span,
            incremental_sync_lost: evidence.incremental_sync_lost,
        },
        DocumentSource::Discarded(evidence) => SourceState::Discarded {
            source_len: evidence.source_len,
            last_max_source_bytes: evidence.last_max_source_bytes,
            span: evidence.span,
            incremental_sync_lost: evidence.incremental_sync_lost,
        },
        DocumentSource::SyncError => SourceState::SyncError,
    }
}

fn analysis_rejection(document: &StoredDocument) -> Option<&merman_analysis::AnalysisRejection> {
    match &document.source {
        DocumentSource::AnalysisRejected { rejection, .. } => Some(rejection),
        DocumentSource::Available(_)
        | DocumentSource::ResourceLimited(_)
        | DocumentSource::Discarded(_)
        | DocumentSource::SyncError => None,
    }
}

fn prepare_source_for_test(
    store: &SessionState,
    uri: &Uri,
    text: String,
    kind: DocumentKind,
) -> DocumentSource {
    prepare_document_source_cancellable(
        text,
        store.analyzer_options().resource_limits(),
        &super::document_source_descriptor(uri, kind),
        &AnalysisCancellationToken::new(),
    )
    .expect("a private source preparation token cannot be cancelled")
}

fn new_session_state() -> SessionState {
    SessionState::with_session_cancellation(AnalysisCancellationToken::new())
}

fn new_session_state_with_analyzer(analyzer: Analyzer) -> SessionState {
    SessionState::with_analyzer_and_cache_budget(
        analyzer,
        AnalysisCancellationToken::new(),
        DEFAULT_LSP_ANALYSIS_CACHE_BUDGET_BYTES,
    )
}

fn apply_analyzer_options(
    store: &mut SessionState,
    options: AnalysisOptions,
) -> AnalysisConfigChange {
    let request = store.begin_analyzer_configuration_request();
    match store
        .prepare_analyzer_options(request, options)
        .expect("a synchronous analyzer update cannot be superseded")
    {
        AnalyzerOptionsPreparation::Applied(change) => change,
        AnalyzerOptionsPreparation::RequiresSnapshotPreparation(plan) => {
            let batch = plan
                .prepare()
                .expect("a synchronous analyzer update cannot be cancelled");
            store
                .commit_snapshot_configuration(batch)
                .expect("a synchronous analyzer update cannot become stale")
        }
    }
}

fn prepare_snapshot_configuration(
    store: &SessionState,
    next_options: AnalysisOptions,
) -> SnapshotConfigurationPlan {
    store.prepare_snapshot_configuration_for(store.latest_configuration_request, next_options)
}

fn open_text(
    store: &mut SessionState,
    uri: Uri,
    version: i32,
    text: String,
    kind: DocumentKind,
) -> StoredDocument {
    let source = prepare_source_for_test(store, &uri, text, kind);
    store.open_document_source(uri, version, source, kind)
}

fn apply_text_changes(
    store: &mut SessionState,
    uri: Uri,
    version: i32,
    changes: impl IntoIterator<Item = TextDocumentContentChangeEvent>,
) -> bool {
    let Some(plan) = store.capture_text_changes(uri, version, changes) else {
        return false;
    };
    store.commit_prepared_document_change(
        plan.prepare()
            .expect("a private text-change token cannot be cancelled"),
    )
}

#[test]
fn new_store_uses_lsp_default_source_limit() {
    let store = new_session_state();

    assert_eq!(
        store.analyzer_options().max_source_bytes(),
        Some(DEFAULT_LSP_MAX_SOURCE_BYTES)
    );
    assert_eq!(
        store.analyzer_options().max_document_diagrams(),
        Some(DEFAULT_LSP_MAX_DOCUMENT_DIAGRAMS)
    );
}

#[test]
fn markdown_diagram_limit_retains_text_without_admitting_a_snapshot() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/limited.md").unwrap();
    let source = concat!(
        "```mermaid\nflowchart TD\nA-->B\n```\n",
        "```mermaid\nsequenceDiagram\nA->>B: hi\n```\n",
    );
    apply_analyzer_options(
        &mut store,
        default_lsp_analysis_options().with_max_document_diagrams(Some(1)),
    );

    let document = open_text(
        &mut store,
        uri.clone(),
        1,
        source.to_string(),
        DocumentKind::Markdown,
    );

    assert_eq!(document.retained_text().unwrap().as_ref(), source);
    assert!(document.is_analysis_unavailable());
    assert_eq!(
        analysis_rejection(&document).unwrap().resource_limit(),
        AnalysisResourceLimit::DocumentDiagrams {
            observed_document_diagrams: 2,
            max_document_diagrams: 1,
        }
    );
    assert_eq!(source_state(&document), SourceState::AnalysisRejected);
}

#[test]
fn ranged_changes_cross_markdown_diagram_limit_in_both_directions() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/ranged-limit.md").unwrap();
    let source = concat!(
        "```mermaid\nflowchart TD\nA-->B\n```\n",
        "```mermaid\nsequenceDiagram\nA->>B: hi\n```\n",
    );
    apply_analyzer_options(
        &mut store,
        default_lsp_analysis_options().with_max_document_diagrams(Some(1)),
    );
    open_text(
        &mut store,
        uri.clone(),
        1,
        source.to_string(),
        DocumentKind::Markdown,
    );

    let recovered = apply_text_changes(
        &mut store,
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(4, 3), Position::new(4, 10))),
            range_length: None,
            text: "text".to_string(),
        }],
    );
    assert!(recovered);
    let document = store.get(&uri).unwrap();
    assert!(!document.is_analysis_unavailable());
    assert_eq!(source_state(document), SourceState::Available);

    let rejected = apply_text_changes(
        &mut store,
        uri.clone(),
        3,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(4, 3), Position::new(4, 7))),
            range_length: None,
            text: "mermaid".to_string(),
        }],
    );
    assert!(rejected);
    let document = store.get(&uri).unwrap();
    assert!(document.is_analysis_unavailable());
    assert_eq!(source_state(document), SourceState::AnalysisRejected);
}

#[test]
fn configuration_reclassifies_retained_markdown_without_reopening() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/config-limit.md").unwrap();
    let source = concat!(
        "```mermaid\nflowchart TD\nA-->B\n```\n",
        "```mermaid\nsequenceDiagram\nA->>B: hi\n```\n",
    );
    apply_analyzer_options(
        &mut store,
        default_lsp_analysis_options().with_max_document_diagrams(Some(2)),
    );
    open_text(
        &mut store,
        uri.clone(),
        1,
        source.to_string(),
        DocumentKind::Markdown,
    );
    let original = Arc::clone(store.get(&uri).unwrap().retained_text().unwrap());

    apply_analyzer_options(
        &mut store,
        default_lsp_analysis_options().with_max_document_diagrams(Some(1)),
    );
    let rejected = store.get(&uri).unwrap();
    assert_eq!(source_state(rejected), SourceState::AnalysisRejected);
    assert!(Arc::ptr_eq(&original, rejected.retained_text().unwrap()));

    apply_analyzer_options(
        &mut store,
        default_lsp_analysis_options().with_max_document_diagrams(Some(2)),
    );
    let recovered = store.get(&uri).unwrap();
    assert!(!recovered.is_analysis_unavailable());
    assert_eq!(source_state(recovered), SourceState::Available);
    assert!(Arc::ptr_eq(&original, recovered.retained_text().unwrap()));
}

#[test]
fn source_byte_limit_precedes_document_diagram_limit_and_discards_text() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/dual-limit.md").unwrap();
    let source = "```mermaid\nflowchart TD\nA-->B\n```\n";
    apply_analyzer_options(
        &mut store,
        AnalysisOptions::default()
            .with_max_source_bytes(Some(1))
            .with_max_document_diagrams(Some(0)),
    );

    let document = open_text(
        &mut store,
        uri,
        1,
        source.to_string(),
        DocumentKind::Markdown,
    );

    assert!(document.retained_text().is_none());
    assert!(matches!(
        source_state(&document),
        SourceState::ResourceLimited { .. }
    ));
}

#[test]
fn newer_versions_replace_the_stored_document() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    let first = open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let second = open_text(
        &mut store,
        uri.clone(),
        2,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );

    assert_eq!(first.version, 1);
    assert_eq!(second.version, 2);

    let stored = store.get(&uri).unwrap();
    assert_eq!(stored.version, 2);
    assert!(stored.text().unwrap().contains("sequenceDiagram"));
    assert!(!stored.text().unwrap().contains("flowchart TD"));
}

#[test]
fn upsert_text_limits_oversized_documents_without_retaining_source() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/large.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();

    apply_analyzer_options(
        &mut store,
        AnalysisOptions::default().with_max_source_bytes(Some(8)),
    );
    let document = open_text(
        &mut store,
        uri.clone(),
        1,
        source.clone(),
        DocumentKind::Diagram,
    );

    assert_eq!(document.version, 1);
    assert!(document.text().is_none());
    assert_eq!(
        source_state(&document),
        SourceState::ResourceLimited {
            source_len: source.len(),
            last_max_source_bytes: 8,
            span: source_limit_diagnostic_span(&source),
            incremental_sync_lost: false,
        }
    );
}

#[test]
fn prepared_text_limits_oversized_documents_without_scanning_under_the_store_lock() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/large-prepared.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();

    apply_analyzer_options(
        &mut store,
        AnalysisOptions::default().with_max_source_bytes(Some(8)),
    );
    let prepared = prepare_source_for_test(&store, &uri, source.clone(), DocumentKind::Diagram);
    let document = store.open_document_source(uri, 1, prepared, DocumentKind::Diagram);

    assert!(document.text().is_none());
    assert_eq!(
        source_state(&document),
        SourceState::ResourceLimited {
            source_len: source.len(),
            last_max_source_bytes: 8,
            span: source_limit_diagnostic_span(&source),
            incremental_sync_lost: false,
        }
    );
}

#[test]
fn prepared_text_only_projects_a_span_when_the_source_is_oversized() {
    let source = "flowchart TD\nA-->B\n".to_string();
    let cancellation = AnalysisCancellationToken::new();

    let descriptor = super::document_source_descriptor(
        &Uri::from_str("file:///tmp/prepared.mmd").unwrap(),
        DocumentKind::Diagram,
    );
    let admitted = prepare_document_source_cancellable(
        source.clone(),
        AnalysisOptions::default()
            .with_max_source_bytes(Some(source.len()))
            .resource_limits(),
        &descriptor,
        &cancellation,
    )
    .expect("the admitted source should be prepared");
    assert!(matches!(admitted, DocumentSource::Available(_)));

    let unlimited = prepare_document_source_cancellable(
        source.clone(),
        AnalysisOptions::default().resource_limits(),
        &descriptor,
        &cancellation,
    )
    .expect("the unlimited source should be prepared");
    assert!(matches!(unlimited, DocumentSource::Available(_)));

    let oversized = prepare_document_source_cancellable(
        source.clone(),
        AnalysisOptions::default()
            .with_max_source_bytes(Some(source.len() - 1))
            .resource_limits(),
        &descriptor,
        &cancellation,
    )
    .expect("the oversized source should be prepared");
    assert_eq!(
        match oversized {
            DocumentSource::ResourceLimited(evidence) => Some(evidence.span),
            _ => None,
        },
        Some(source_limit_diagnostic_span(&source))
    );
}

#[test]
fn source_limit_reclassification_rejects_a_stale_document_epoch() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/reclassified.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();
    open_text(
        &mut store,
        uri.clone(),
        1,
        source.clone(),
        DocumentKind::Diagram,
    );

    let batch = prepare_snapshot_configuration(
        &store,
        default_lsp_analysis_options().with_max_source_bytes(Some(8)),
    )
    .prepare()
    .expect("test projection should not be cancelled");
    open_text(
        &mut store,
        uri.clone(),
        2,
        "flowchart TD\nC-->D\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(store.commit_snapshot_configuration(batch).is_none());
    let document = store.get(&uri).expect("replacement document should remain");
    assert_eq!(document.version, 2);
    assert_eq!(source_state(document), SourceState::Available);
    assert_eq!(
        store.analyzer_options().max_source_bytes(),
        Some(DEFAULT_LSP_MAX_SOURCE_BYTES)
    );
}

#[test]
fn session_cancellation_aborts_pending_source_limit_projection() {
    let cancellation = AnalysisCancellationToken::new();
    let mut store = SessionState::with_session_cancellation(cancellation.clone());
    let uri = Uri::from_str("file:///tmp/cancelled-reclassification.mmd").unwrap();
    open_text(
        &mut store,
        uri,
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let plan = prepare_snapshot_configuration(
        &store,
        default_lsp_analysis_options().with_max_source_bytes(Some(8)),
    );

    cancellation.cancel();

    assert!(matches!(
        plan.prepare(),
        Err(merman_analysis::AnalysisCancelled)
    ));
}

#[test]
fn source_limit_reclassification_rejects_a_superseded_configuration_request() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/configuration-order.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();
    open_text(
        &mut store,
        uri.clone(),
        1,
        source.clone(),
        DocumentKind::Diagram,
    );

    let older_request = store.begin_analyzer_configuration_request();
    let older_plan = match store
        .prepare_analyzer_options(
            older_request,
            default_lsp_analysis_options().with_max_source_bytes(Some(8)),
        )
        .expect("the first request is current")
    {
        AnalyzerOptionsPreparation::RequiresSnapshotPreparation(plan) => plan,
        AnalyzerOptionsPreparation::Applied(_) => {
            panic!("the lower source limit must require source projection")
        }
    };
    let older_batch = older_plan
        .prepare()
        .expect("test projection should not be cancelled");

    let latest_request = store.begin_analyzer_configuration_request();
    let latest = store
        .prepare_analyzer_options(latest_request, default_lsp_analysis_options())
        .expect("the latest request is current");
    assert!(matches!(
        latest,
        AnalyzerOptionsPreparation::Applied(AnalysisConfigChange::Unchanged)
    ));

    assert!(!store.is_analyzer_configuration_request_current(older_request));
    assert!(store.commit_snapshot_configuration(older_batch).is_none());
    assert_eq!(
        store.analyzer_options().max_source_bytes(),
        Some(DEFAULT_LSP_MAX_SOURCE_BYTES)
    );
    let document = store.get(&uri).expect("latest configuration keeps source");
    assert_eq!(document.text().unwrap().as_ref(), source);
    assert_eq!(source_state(document), SourceState::Available);
}

#[test]
fn full_replacement_recovers_from_resource_limited_document() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/large.mmd").unwrap();

    apply_analyzer_options(
        &mut store,
        AnalysisOptions::default().with_max_source_bytes(Some(8)),
    );
    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = apply_text_changes(
        &mut store,
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "A-->B\n".to_string(),
        }],
    );

    assert!(update);
    let stored = store.get(&uri).expect("expected recovered document");
    assert_eq!(stored.version, 2);
    assert_eq!(stored.text().unwrap().as_ref(), "A-->B\n");
    assert_eq!(source_state(stored), SourceState::Available);
}

#[test]
fn ranged_changes_on_resource_limited_documents_keep_lightweight_state() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/large.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();

    apply_analyzer_options(
        &mut store,
        AnalysisOptions::default().with_max_source_bytes(Some(8)),
    );
    open_text(
        &mut store,
        uri.clone(),
        1,
        source.clone(),
        DocumentKind::Diagram,
    );

    let update = apply_text_changes(
        &mut store,
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(1, 0), Position::new(1, 1))),
            range_length: None,
            text: "C".to_string(),
        }],
    );

    assert!(update);
    let stored = store.get(&uri).expect("expected limited document");
    assert_eq!(stored.version, 2);
    assert!(stored.text().is_none());
    assert_eq!(
        source_state(stored),
        SourceState::ResourceLimited {
            source_len: source.len(),
            last_max_source_bytes: 8,
            span: source_limit_diagnostic_span(&source),
            incremental_sync_lost: true,
        }
    );
    apply_analyzer_options(
        &mut store,
        AnalysisOptions::default().with_max_source_bytes(Some(64)),
    );
    assert_eq!(
        source_state(store.get(&uri).expect("expected discarded document")),
        SourceState::Discarded {
            source_len: source.len(),
            last_max_source_bytes: 8,
            span: source_limit_diagnostic_span(&source),
            incremental_sync_lost: true,
        }
    );

    apply_analyzer_options(
        &mut store,
        AnalysisOptions::default().with_max_source_bytes(Some(16)),
    );
    assert_eq!(
        source_state(store.get(&uri).expect("expected limited document")),
        SourceState::ResourceLimited {
            source_len: source.len(),
            last_max_source_bytes: 16,
            span: source_limit_diagnostic_span(&source),
            incremental_sync_lost: true,
        }
    );
}

#[test]
fn resource_limited_documents_update_limit_when_configuration_still_excludes_them() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/large.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();

    apply_analyzer_options(
        &mut store,
        AnalysisOptions::default().with_max_source_bytes(Some(8)),
    );
    open_text(
        &mut store,
        uri.clone(),
        1,
        source.clone(),
        DocumentKind::Diagram,
    );
    apply_analyzer_options(
        &mut store,
        AnalysisOptions::default().with_max_source_bytes(Some(16)),
    );

    let stored = store.get(&uri).expect("expected limited document");
    assert_eq!(
        source_state(stored),
        SourceState::ResourceLimited {
            source_len: source.len(),
            last_max_source_bytes: 16,
            span: source_limit_diagnostic_span(&source),
            incremental_sync_lost: false,
        }
    );
}

#[test]
fn resource_limited_documents_become_discarded_when_configuration_would_allow_them() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/large.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();

    apply_analyzer_options(
        &mut store,
        AnalysisOptions::default().with_max_source_bytes(Some(8)),
    );
    open_text(
        &mut store,
        uri.clone(),
        1,
        source.clone(),
        DocumentKind::Diagram,
    );
    apply_analyzer_options(
        &mut store,
        AnalysisOptions::default().with_max_source_bytes(Some(64)),
    );

    let stored = store.get(&uri).expect("expected discarded document");
    assert_eq!(
        source_state(stored),
        SourceState::Discarded {
            source_len: source.len(),
            last_max_source_bytes: 8,
            span: source_limit_diagnostic_span(&source),
            incremental_sync_lost: false,
        }
    );
    let update = apply_text_changes(
        &mut store,
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: source.clone(),
        }],
    );

    assert!(update);
    let stored = store.get(&uri).expect("expected recovered document");
    assert_eq!(source_state(stored), SourceState::Available);
    assert_eq!(stored.text().unwrap().as_ref(), source);
}

#[test]
fn apply_text_change_rejects_missing_documents() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/missing.mmd").unwrap();

    let update = apply_text_changes(
        &mut store,
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        }],
    );

    assert!(!update);
    assert!(store.get(&uri).is_none());
}

#[test]
fn apply_text_change_rejects_stale_versions_without_invalidating_current_state() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    open_text(
        &mut store,
        uri.clone(),
        3,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );
    let update = apply_text_changes(
        &mut store,
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "flowchart TD\nA-->B\n".to_string(),
        }],
    );

    assert!(!update);
    let stored = store.get(&uri).expect("expected current document");
    assert_eq!(stored.version, 3);
    assert!(stored.text().unwrap().contains("sequenceDiagram"));
}

#[test]
fn apply_text_changes_applies_lsp_utf16_ranges_in_order() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA[🤓]-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = apply_text_changes(
        &mut store,
        uri.clone(),
        2,
        [
            TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(1, 2), Position::new(1, 4))),
                range_length: None,
                text: "C".to_string(),
            },
            TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(1, 8), Position::new(1, 8))),
                range_length: None,
                text: "\nC-->D".to_string(),
            },
        ],
    );

    assert!(update);
    let stored = store.get(&uri).expect("expected updated document");
    assert_eq!(stored.version, 2);
    assert_eq!(
        stored.text().unwrap().as_ref(),
        "flowchart TD\nA[C]-->B\nC-->D\n"
    );
}

#[test]
fn prepared_text_changes_cannot_overwrite_a_newer_document_epoch() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/prepared-change-cas.mmd").unwrap();
    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let plan = store
        .capture_text_changes(
            uri.clone(),
            2,
            [TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "flowchart TD\nA-->C\n".to_string(),
            }],
        )
        .expect("valid change should require lock-free preparation");

    open_text(
        &mut store,
        uri.clone(),
        3,
        "sequenceDiagram\nAlice->>Bob: newer\n".to_string(),
        DocumentKind::Diagram,
    );
    let update = store.commit_prepared_document_change(
        plan.prepare()
            .expect("test text preparation should not be cancelled"),
    );

    assert!(!update);
    let stored = store
        .get(&uri)
        .expect("newer document should remain stored");
    assert_eq!(stored.version, 3);
    assert!(stored.text().unwrap().starts_with("sequenceDiagram"));
}

#[test]
fn session_cancellation_aborts_pending_text_change_preparation() {
    let cancellation = AnalysisCancellationToken::new();
    let mut store = SessionState::with_session_cancellation(cancellation.clone());
    let uri = Uri::from_str("file:///tmp/cancelled-change.mmd").unwrap();
    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let plan = store
        .capture_text_changes(
            uri,
            2,
            [TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "flowchart TD\nA-->C\n".to_string(),
            }],
        )
        .expect("valid change should require lock-free preparation");

    cancellation.cancel();

    assert!(matches!(
        plan.prepare(),
        Err(merman_analysis::AnalysisCancelled)
    ));
}

#[test]
fn apply_text_changes_updates_line_index_between_batched_edits() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\r\nA[🤓]\rB\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = apply_text_changes(
        &mut store,
        uri.clone(),
        2,
        [
            TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(1, 2), Position::new(1, 4))),
                range_length: None,
                text: "C\nD".to_string(),
            },
            TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(2, 0), Position::new(2, 1))),
                range_length: None,
                text: "E".to_string(),
            },
            TextDocumentContentChangeEvent {
                range: Some(Range::new(
                    Position::new(3, 10_000),
                    Position::new(3, 10_000),
                )),
                range_length: None,
                text: "-->C".to_string(),
            },
        ],
    );

    assert!(update);
    let stored = store.get(&uri).expect("expected updated document");
    assert_eq!(
        stored.text().unwrap().as_ref(),
        "flowchart TD\r\nA[C\nE]\rB-->C\n"
    );
}

#[test]
fn apply_text_changes_allows_nonconsecutive_versions_for_incremental_ranges() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = apply_text_changes(
        &mut store,
        uri.clone(),
        3,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(1, 0), Position::new(1, 1))),
            range_length: None,
            text: "C".to_string(),
        }],
    );

    assert!(update);
    let stored = store.get(&uri).expect("expected updated document");
    assert_eq!(stored.version, 3);
    assert_eq!(stored.text().unwrap().as_ref(), "flowchart TD\nC-->B\n");
}

#[test]
fn apply_text_changes_rejects_empty_change_sets_without_advancing_version() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = apply_text_changes(&mut store, uri.clone(), 3, []);

    assert!(!update);
    let stored = store.get(&uri).expect("expected current document");
    assert_eq!(stored.version, 1);
    assert_eq!(stored.text().unwrap().as_ref(), "flowchart TD\nA-->B\n");
}

#[test]
fn apply_text_changes_allows_skipped_versions_for_full_replacements() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = apply_text_changes(
        &mut store,
        uri.clone(),
        4,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        }],
    );

    assert!(update);
    let stored = store.get(&uri).expect("expected updated document");
    assert_eq!(stored.version, 4);
    assert_eq!(
        stored.text().unwrap().as_ref(),
        "sequenceDiagram\nAlice->>Bob: Hi\n"
    );
}

#[test]
fn apply_text_changes_marks_document_unsynced_after_invalid_range() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let update = apply_text_changes(
        &mut store,
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(20, 0), Position::new(20, 1))),
            range_length: None,
            text: "bad".to_string(),
        }],
    );

    assert!(update);
    let stored = store.get(&uri).expect("expected current document");
    assert_eq!(stored.version, 2);
    assert!(stored.text().is_none());
    assert_eq!(source_state(stored), SourceState::SyncError);
}

#[test]
fn apply_text_changes_marks_document_unsynced_after_reversed_range() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = apply_text_changes(
        &mut store,
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(1, 4), Position::new(1, 2))),
            range_length: None,
            text: "bad".to_string(),
        }],
    );

    assert!(update);
    let stored = store.get(&uri).expect("expected unsynced document");
    assert!(stored.text().is_none());
    assert_eq!(source_state(stored), SourceState::SyncError);
}

#[test]
fn apply_text_changes_clamps_utf16_positions_past_line_end() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA[🤓]-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = apply_text_changes(
        &mut store,
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(
                Position::new(1, 10_000),
                Position::new(1, 10_000),
            )),
            range_length: None,
            text: "bad".to_string(),
        }],
    );

    assert!(update);
    let stored = store.get(&uri).expect("expected updated document");
    assert_eq!(stored.version, 2);
    assert_eq!(
        stored.text().unwrap().as_ref(),
        "flowchart TD\nA[🤓]-->Bbad\n"
    );
    assert_eq!(source_state(stored), SourceState::Available);
}

#[test]
fn full_replacement_recovers_from_unsynced_document_after_invalid_range() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(apply_text_changes(
        &mut store,
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(20, 0), Position::new(20, 1))),
            range_length: None,
            text: "bad".to_string(),
        }],
    ));

    let update = apply_text_changes(
        &mut store,
        uri.clone(),
        3,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        }],
    );

    assert!(update);
    let stored = store.get(&uri).expect("expected recovered document");
    assert_eq!(stored.version, 3);
    assert_eq!(
        stored.text().unwrap().as_ref(),
        "sequenceDiagram\nAlice->>Bob: Hi\n"
    );
    assert_eq!(source_state(stored), SourceState::Available);
}

#[test]
fn ranged_changes_on_unsynced_documents_keep_lightweight_state() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(apply_text_changes(
        &mut store,
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(20, 0), Position::new(20, 1))),
            range_length: None,
            text: "bad".to_string(),
        }],
    ));

    let update = apply_text_changes(
        &mut store,
        uri.clone(),
        3,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(1, 0), Position::new(1, 1))),
            range_length: None,
            text: "C".to_string(),
        }],
    );

    assert!(update);
    let stored = store.get(&uri).expect("expected unsynced document");
    assert_eq!(stored.version, 3);
    assert!(stored.text().is_none());
    assert_eq!(source_state(stored), SourceState::SyncError);

    apply_analyzer_options(
        &mut store,
        AnalysisOptions::default().with_max_source_bytes(Some(8)),
    );
    assert_eq!(
        source_state(store.get(&uri).expect("expected unsynced document")),
        SourceState::SyncError
    );
    apply_analyzer_options(&mut store, default_lsp_analysis_options());
    assert_eq!(
        source_state(store.get(&uri).expect("expected unsynced document")),
        SourceState::SyncError
    );
}

#[test]
fn full_replacement_later_in_unsynced_batch_recovers_document() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(apply_text_changes(
        &mut store,
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(20, 0), Position::new(20, 1))),
            range_length: None,
            text: "bad".to_string(),
        }],
    ));

    let update = apply_text_changes(
        &mut store,
        uri.clone(),
        3,
        [
            TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(0, 0), Position::new(0, 1))),
                range_length: None,
                text: "ignored".to_string(),
            },
            TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
            },
        ],
    );

    assert!(update);
    let stored = store.get(&uri).expect("expected recovered document");
    assert_eq!(stored.version, 3);
    assert_eq!(
        stored.text().unwrap().as_ref(),
        "sequenceDiagram\nAlice->>Bob: Hi\n"
    );
    assert_eq!(source_state(stored), SourceState::Available);
}

#[test]
fn full_replacement_later_in_available_batch_ignores_prior_invalid_ranges() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = apply_text_changes(
        &mut store,
        uri.clone(),
        2,
        [
            TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(20, 0), Position::new(20, 1))),
                range_length: None,
                text: "bad".to_string(),
            },
            TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
            },
        ],
    );

    assert!(update);
    let stored = store.get(&uri).expect("expected replaced document");
    assert_eq!(stored.version, 2);
    assert_eq!(
        stored.text().unwrap().as_ref(),
        "sequenceDiagram\nAlice->>Bob: Hi\n"
    );
    assert_eq!(source_state(stored), SourceState::Available);
}

#[test]
fn ranged_changes_after_a_full_replacement_apply_to_the_replacement() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = apply_text_changes(
        &mut store,
        uri.clone(),
        2,
        [
            TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
            },
            TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(1, 13), Position::new(1, 15))),
                range_length: None,
                text: "Hello".to_string(),
            },
        ],
    );

    assert!(update);
    let stored = store.get(&uri).expect("expected updated document");
    assert_eq!(
        stored.text().unwrap().as_ref(),
        "sequenceDiagram\nAlice->>Bob: Hello\n"
    );
}

#[test]
fn diagnostic_state_is_bound_to_the_document_epoch() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/diagnostic-state.mmd").unwrap();
    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let context = store
        .diagnostic_context(&uri)
        .expect("expected diagnostic context");
    let state = DocumentDiagnosticState {
        result_id: "result-1".to_string(),
        diagnostics: Vec::new(),
    };

    assert!(store.set_diagnostic_state_if_current(&context, state.clone()));
    assert_eq!(
        store
            .diagnostic_state(&uri)
            .expect("expected cached diagnostics")
            .result_id,
        "result-1"
    );

    open_text(
        &mut store,
        uri.clone(),
        2,
        "flowchart TD\nA-->C\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(store.diagnostic_state(&uri).is_none());
    assert!(!store.set_diagnostic_state_if_current(&context, state));
}

#[test]
fn no_op_configuration_does_not_supersede_prepared_text_changes() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/no-op-config-text-ticket.mmd").unwrap();
    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let plan = store
        .capture_text_changes(
            uri.clone(),
            2,
            [TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "flowchart TD\nA-->C\n".to_string(),
            }],
        )
        .expect("expected a prepared text transaction");
    let prepared = plan.prepare().expect("text preparation should succeed");

    let request = store.begin_analyzer_configuration_request();
    let preparation = store
        .prepare_analyzer_options(request, store.analyzer_options().clone())
        .expect("current no-op configuration should be classified");
    assert!(matches!(
        preparation,
        AnalyzerOptionsPreparation::Applied(AnalysisConfigChange::Unchanged)
    ));

    assert!(store.commit_prepared_document_change(prepared));
    assert_eq!(store.get(&uri).unwrap().version, 2);
}

#[test]
fn applied_configuration_supersedes_prepared_text_changes() {
    let mut store = new_session_state();
    let uri = Uri::from_str("file:///tmp/applied-config-text-ticket.mmd").unwrap();
    open_text(
        &mut store,
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let plan = store
        .capture_text_changes(
            uri.clone(),
            2,
            [TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "flowchart TD\nA-->C\n".to_string(),
            }],
        )
        .expect("expected a prepared text transaction");
    let prepared = plan.prepare().expect("text preparation should succeed");

    apply_analyzer_options(
        &mut store,
        default_lsp_analysis_options().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                .unwrap(),
        ),
    );

    assert!(!store.commit_prepared_document_change(prepared));
    assert_eq!(store.get(&uri).unwrap().version, 1);
}

#[test]
fn snapshot_update_without_a_source_limit_change_skips_source_projection() {
    let mut store = new_session_state();
    let request = store.begin_analyzer_configuration_request();
    let preparation = store
        .prepare_analyzer_options(
            request,
            default_lsp_analysis_options().with_fixed_today(Some("2026-07-30".parse().unwrap())),
        )
        .expect("current configuration should be classified");

    let AnalyzerOptionsPreparation::RequiresSnapshotPreparation(plan) = preparation else {
        panic!("snapshot-affecting options must be prepared outside the session lock");
    };
    let batch = plan.prepare().expect("snapshot preparation should succeed");
    assert!(batch.resource_rejections.is_none());
    assert!(store.commit_snapshot_configuration(batch).is_some());
}

#[test]
fn snapshot_update_without_a_source_limit_change_ignores_unrelated_document_writes() {
    let mut store = new_session_state();
    let request = store.begin_analyzer_configuration_request();
    let preparation = store
        .prepare_analyzer_options(
            request,
            default_lsp_analysis_options().with_fixed_today(Some("2026-07-30".parse().unwrap())),
        )
        .expect("current configuration should be classified");
    let AnalyzerOptionsPreparation::RequiresSnapshotPreparation(plan) = preparation else {
        panic!("snapshot-affecting options must be prepared outside the session lock");
    };
    let batch = plan.prepare().expect("snapshot preparation should succeed");

    open_text(
        &mut store,
        Uri::from_str("file:///tmp/unrelated-during-config.mmd").unwrap(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(store.commit_snapshot_configuration(batch).is_some());
}

#[test]
fn snapshot_update_rejects_an_environment_replacement_with_the_same_request_id() {
    let mut store = new_session_state();
    let request = store.begin_analyzer_configuration_request();
    let preparation = store
        .prepare_analyzer_options(
            request,
            default_lsp_analysis_options().with_fixed_today(Some("2026-07-30".parse().unwrap())),
        )
        .expect("current configuration should be classified");
    let AnalyzerOptionsPreparation::RequiresSnapshotPreparation(plan) = preparation else {
        panic!("snapshot-affecting options must be prepared outside the session lock");
    };
    let batch = plan.prepare().expect("snapshot preparation should succeed");
    let replacement =
        Analyzer::with_engine(merman_core::Engine::new(), store.analyzer.options().clone());

    store.replace_analyzer(replacement, None);

    assert!(store.is_analyzer_configuration_request_current(request));
    assert!(
        store.commit_snapshot_configuration(batch).is_none(),
        "environment replacement must invalidate a prepared configuration even when its request id remains current"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn language_session_configuration_preserves_a_custom_parser_registry() {
    CUSTOM_ASYNC_SESSION_PARSE_CALLS.store(0, Ordering::SeqCst);
    let mut engine = merman_core::Engine::new();
    engine
        .diagram_registry_mut()
        .insert("flowchart-v2", custom_async_session_flowchart_parser);
    let options = default_lsp_analysis_options();
    let session = super::LanguageSession::with_analyzer_for_tests(Analyzer::with_engine(
        engine,
        options.clone(),
    ));
    let uri = Uri::from_str("file:///tmp/custom-registry-session.mmd").unwrap();

    assert!(
        session
            .open_document(
                uri.clone(),
                1,
                "flowchart TD\nA-->B\n".to_string(),
                DocumentKind::Diagram,
            )
            .await
    );
    session
        .query_structure(&uri, |_| Ok(Some(())))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(CUSTOM_ASYNC_SESSION_PARSE_CALLS.load(Ordering::SeqCst), 1);

    let change = session
        .update_configuration(
            options.with_max_source_bytes(Some(DEFAULT_LSP_MAX_SOURCE_BYTES.saturating_add(1))),
        )
        .await;
    assert!(change.affects_snapshots());
    session
        .query_structure(&uri, |_| Ok(Some(())))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        CUSTOM_ASYNC_SESSION_PARSE_CALLS.load(Ordering::SeqCst),
        2,
        "the typed session configuration path must retain the custom parser registry"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn termination_fences_a_configuration_update_waiting_for_session_state() {
    let options = default_lsp_analysis_options();
    let session = super::LanguageSession::with_cancellation(AnalysisCancellationToken::new());
    let state = session.inner.state.lock().await;
    let before_snapshot_generation = state.snapshot_generation;
    let before_diagnostic_generation = state.diagnostic_generation;
    let before_configuration_revision = state.configuration_revision;
    let before_configuration_request = state.latest_configuration_request;
    let before_documents_revision = state.documents_revision;
    let before_environment = state.analyzer_environment_identity().clone();

    let update = session.update_configuration(
        options.with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                .unwrap(),
        ),
    );
    tokio::pin!(update);
    assert!(futures::poll!(&mut update).is_pending());

    assert!(session.terminate());
    drop(state);

    assert_eq!(update.await, ConfigurationUpdateOutcome::Cancelled);
    let state = session.inner.state.lock().await;
    assert_eq!(state.snapshot_generation, before_snapshot_generation);
    assert_eq!(state.diagnostic_generation, before_diagnostic_generation);
    assert_eq!(state.configuration_revision, before_configuration_revision);
    assert_eq!(
        state.latest_configuration_request,
        before_configuration_request
    );
    assert_eq!(state.documents_revision, before_documents_revision);
    assert_eq!(state.analyzer_environment_identity(), &before_environment);
}

#[test]
fn replacing_an_engine_changes_identity_even_when_options_match() {
    let options = default_lsp_analysis_options();
    let mut first_engine = merman_core::Engine::new();
    first_engine
        .diagram_registry_mut()
        .insert("flowchart-v2", custom_session_flowchart_parser);
    let mut store =
        new_session_state_with_analyzer(Analyzer::with_engine(first_engine, options.clone()));
    let initial_identity = store.analyzer_environment_identity().clone();

    let mut replacement_engine = merman_core::Engine::new();
    replacement_engine
        .diagram_registry_mut()
        .insert("flowchart-v2", custom_session_flowchart_parser);
    store.replace_analyzer(
        Analyzer::with_engine(replacement_engine, options.clone()),
        None,
    );

    assert_eq!(store.analyzer_options(), &options);
    assert_ne!(store.analyzer_environment_identity(), &initial_identity);
}

#[test]
fn open_commit_is_uri_local_and_rejects_changed_target_or_configuration_state() {
    let mut state = new_session_state();
    let uri = Uri::from_str("file:///tmp/open-ticket.mmd").unwrap();
    let other = Uri::from_str("file:///tmp/other.mmd").unwrap();
    let ticket = state.capture_open_document(uri.clone());
    open_text(
        &mut state,
        other,
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    assert!(state.commit_open_document(
        ticket,
        1,
        DocumentSource::Available(Arc::from("flowchart TD\nA-->B\n")),
        DocumentKind::Diagram,
    ));
    assert_eq!(state.get(&uri).unwrap().version, 1);

    let ticket = state.capture_open_document(uri.clone());
    open_text(
        &mut state,
        uri.clone(),
        2,
        "flowchart TD\nA-->C\n".to_string(),
        DocumentKind::Diagram,
    );
    assert!(!state.commit_open_document(
        ticket,
        3,
        DocumentSource::Available(Arc::from("flowchart TD\nA-->D\n")),
        DocumentKind::Diagram,
    ));
    assert_eq!(state.get(&uri).unwrap().version, 2);

    let ticket = state.capture_open_document(uri.clone());
    apply_analyzer_options(
        &mut state,
        default_lsp_analysis_options().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                .unwrap(),
        ),
    );
    assert!(!state.commit_open_document(
        ticket,
        3,
        DocumentSource::Available(Arc::from("flowchart TD\nA-->D\n")),
        DocumentKind::Diagram,
    ));
    assert_eq!(state.get(&uri).unwrap().version, 2);
}

#[test]
fn open_commit_rejects_an_absent_present_absent_aba() {
    let mut state = new_session_state();
    let uri = Uri::from_str("file:///tmp/open-ticket-aba.mmd").unwrap();
    let ticket = state.capture_open_document(uri.clone());

    state.open_document_source(
        uri.clone(),
        1,
        DocumentSource::Available(Arc::from("flowchart TD\nA-->B\n")),
        DocumentKind::Diagram,
    );
    state.remove(&uri);
    assert!(state.get(&uri).is_none());

    assert!(!state.commit_open_document(
        ticket,
        2,
        DocumentSource::Available(Arc::from("flowchart TD\nA-->C\n")),
        DocumentKind::Diagram,
    ));
    assert!(state.get(&uri).is_none());
    assert!(
        crate::sync::lock_recovering_poison(&state.open_document_tracker.entries).is_empty(),
        "completed open tickets must not leave URI tombstones"
    );
}

#[test]
fn open_tickets_share_a_uri_clock_until_the_last_ticket_finishes() {
    let mut state = new_session_state();
    let uri = Uri::from_str("file:///tmp/open-ticket-overlap.mmd").unwrap();
    let first = state.capture_open_document(uri.clone());
    let second = state.capture_open_document(uri.clone());

    assert_eq!(
        crate::sync::lock_recovering_poison(&state.open_document_tracker.entries)
            .get(&uri)
            .map(|clock| clock.active_tickets),
        Some(2)
    );
    drop(first);
    assert_eq!(
        crate::sync::lock_recovering_poison(&state.open_document_tracker.entries)
            .get(&uri)
            .map(|clock| clock.active_tickets),
        Some(1),
        "dropping one ticket must not erase another ticket's URI clock"
    );

    assert!(state.commit_open_document(
        second,
        1,
        DocumentSource::Available(Arc::from("flowchart TD\nA-->B\n")),
        DocumentKind::Diagram,
    ));
    assert!(
        crate::sync::lock_recovering_poison(&state.open_document_tracker.entries).is_empty(),
        "the last completed ticket must release its URI clock"
    );
}

#[test]
fn dropping_an_uncommitted_open_ticket_releases_its_uri_clock() {
    let state = new_session_state();
    let uri = Uri::from_str("file:///tmp/open-ticket-dropped.mmd").unwrap();
    let ticket = state.capture_open_document(uri);

    drop(ticket);

    assert!(crate::sync::lock_recovering_poison(&state.open_document_tracker.entries).is_empty());
}

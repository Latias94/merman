use std::str::FromStr;
use std::sync::Arc;

use crate::session::documents::{
    AnalyzerOptionsPreparation, DEFAULT_LSP_ANALYSIS_CACHE_BUDGET_BYTES,
    DEFAULT_LSP_MAX_SOURCE_BYTES, DocumentDiagnosticState, DocumentDiscardedSource,
    DocumentResourceLimit, DocumentStore, DocumentSyncError, PreparedDocumentText,
    SemanticTokensState, TextChangePreparation, TextDocumentUpdate, default_lsp_analysis_options,
};
use merman_analysis::{
    AnalysisCancellationToken, AnalysisOptions, AnalysisPayload, AnalysisRuleConfig,
    AnalysisRuleProfile, DiagnosticSeverity, FenceSemanticRole, FenceTextIndexSource,
    source_limit_diagnostic_span,
};
use merman_editor_core::DocumentKind;
use tower_lsp_server::ls_types::{
    Position, Range, SemanticToken, TextDocumentContentChangeEvent, Uri,
};

#[test]
fn plain_mermaid_documents_create_single_snapshot_fence() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "flowchart TD\nclassDef highlight fill:#f00\nA-->B\n".to_string(),
    );

    assert_eq!(snapshot.fences().len(), 1);
    assert_eq!(snapshot.fences()[0].body_range().start, 0);
    assert_eq!(
        snapshot.fences()[0].text(),
        "flowchart TD\nclassDef highlight fill:#f00\nA-->B\n"
    );
    assert_eq!(snapshot.fences()[0].diagram_type(), Some("flowchart-v2"));
    assert!(
        snapshot.fences()[0]
            .text_index()
            .has_directive_prefix("classDef")
    );
}

#[test]
fn new_store_uses_lsp_default_source_limit() {
    let store = DocumentStore::new();

    assert_eq!(
        store.analyzer_options().max_source_bytes(),
        Some(DEFAULT_LSP_MAX_SOURCE_BYTES)
    );
}

#[test]
fn markdown_documents_create_fences_for_markdown_extensions() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.markdown").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "before\n```mermaid\n%%{init: {\"theme\": \"dark\"}}%%\nflowchart TD\nA-->B\n```\nafter\n"
            .to_string(),
    );

    assert_eq!(snapshot.fences().len(), 1);
    assert!(snapshot.fences()[0].text().contains("flowchart TD"));
    assert!(
        snapshot.fences()[0]
            .text_index()
            .node_ids()
            .any(|id| id == "A")
    );
    assert_eq!(snapshot.fences()[0].diagram_type(), Some("flowchart-v2"));
    assert!(
        snapshot.fences()[0]
            .text_index()
            .has_directive_prefix("init")
    );
}

#[test]
fn markdown_documents_create_multiple_mermaid_fences() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.markdown").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "before\n",
            "```mermaid\n",
            "flowchart TD\n",
            "A-->B\n",
            "```\n",
            "middle\n",
            "```mermaid\n",
            "sequenceDiagram\n",
            "Alice->>Bob: Hi\n",
            "```\n",
            "after\n",
        )
        .to_string(),
    );

    assert_eq!(snapshot.fences().len(), 2);
    assert_eq!(snapshot.fences()[0].diagram_type(), Some("flowchart-v2"));
    assert_eq!(snapshot.fences()[1].diagram_type(), Some("sequence"));
    assert_eq!(
        snapshot.fences()[0].text_index().source(),
        FenceTextIndexSource::ParserComplete
    );
    assert_eq!(
        snapshot.fences()[1].text_index().source(),
        FenceTextIndexSource::ParserComplete
    );
    assert!(
        snapshot.fences()[0]
            .text_index()
            .node_ids()
            .any(|id| id == "A")
    );
    assert!(
        snapshot.fences()[0]
            .text_index()
            .node_ids()
            .any(|id| id == "B")
    );
    assert!(
        snapshot.fences()[1]
            .text_index()
            .node_ids()
            .any(|id| id == "Alice")
    );
    assert!(
        snapshot.fences()[1]
            .text_index()
            .node_ids()
            .any(|id| id == "Bob")
    );
}

#[test]
fn newer_versions_replace_the_stored_snapshot() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    let first = store.upsert(uri.clone(), 1, "flowchart TD\nA-->B\n".to_string());
    let second = store.upsert(
        uri.clone(),
        2,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
    );

    assert_eq!(first.version(), 1);
    assert_eq!(second.version(), 2);

    let stored = store.get(&uri).unwrap();
    assert_eq!(stored.version, 2);
    assert!(stored.text.contains("sequenceDiagram"));
    assert!(!stored.text.contains("flowchart TD"));
    let stored = store
        .snapshot(&uri)
        .expect("expected stored snapshot after replacement");
    assert_eq!(stored.fences().len(), 1);
    assert_eq!(stored.fences()[0].diagram_type(), Some("sequence"));
}

#[test]
fn upsert_text_defers_snapshot_until_requested() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    let document = store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert_eq!(document.version, 1);
    assert_eq!(document.text.as_ref(), "flowchart TD\nA-->B\n");
    assert!(!store.has_snapshot(&uri));

    let snapshot = store
        .snapshot(&uri)
        .expect("expected lazy snapshot for stored document");
    assert!(store.has_snapshot(&uri));
    assert_eq!(snapshot.version(), 1);
    assert_eq!(snapshot.fences()[0].diagram_type(), Some("flowchart-v2"));
}

#[test]
fn upsert_text_limits_oversized_documents_without_retaining_source() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/large.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();

    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));
    let document = store.open_text(uri.clone(), 1, source.clone(), DocumentKind::Diagram);

    assert_eq!(document.version, 1);
    assert_eq!(document.text.as_ref(), "");
    assert_eq!(
        document.resource_limit,
        Some(DocumentResourceLimit {
            source_len: source.len(),
            max_source_bytes: 8,
            span: source_limit_diagnostic_span(&source),
        })
    );
    assert_eq!(document.discarded_source, None);
    assert!(store.snapshot(&uri).is_none());
}

#[test]
fn prepared_text_limits_oversized_documents_without_scanning_under_the_store_lock() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/large-prepared.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();

    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));
    let prepared = PreparedDocumentText::new(source.clone());
    let document = store.open_prepared_text(uri, 1, prepared, DocumentKind::Diagram);

    assert_eq!(document.text.as_ref(), "");
    assert_eq!(
        document.resource_limit,
        Some(DocumentResourceLimit {
            source_len: source.len(),
            max_source_bytes: 8,
            span: source_limit_diagnostic_span(&source),
        })
    );
}

#[test]
fn source_limit_reclassification_rejects_a_stale_document_epoch() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/reclassified.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();
    store.open_text(uri.clone(), 1, source.clone(), DocumentKind::Diagram);

    let batch = store
        .prepare_source_limit_reclassification(
            default_lsp_analysis_options().with_max_source_bytes(Some(8)),
        )
        .project()
        .expect("test projection should not be cancelled");
    store.open_text(
        uri.clone(),
        2,
        "flowchart TD\nC-->D\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(store.commit_source_limit_reclassification(batch).is_none());
    let document = store.get(&uri).expect("replacement document should remain");
    assert_eq!(document.version, 2);
    assert_eq!(document.resource_limit, None);
    assert_eq!(
        store.analyzer_options().max_source_bytes(),
        Some(DEFAULT_LSP_MAX_SOURCE_BYTES)
    );
}

#[test]
fn session_cancellation_aborts_pending_source_limit_projection() {
    let cancellation = AnalysisCancellationToken::new();
    let mut store = DocumentStore::with_session_cancellation(cancellation.clone());
    let uri = Uri::from_str("file:///tmp/cancelled-reclassification.mmd").unwrap();
    store.open_text(
        uri,
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let plan = store.prepare_source_limit_reclassification(
        default_lsp_analysis_options().with_max_source_bytes(Some(8)),
    );

    cancellation.cancel();

    assert!(matches!(
        plan.project(),
        Err(merman_analysis::AnalysisCancelled)
    ));
}

#[test]
fn source_limit_reclassification_rejects_a_superseded_configuration_request() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/configuration-order.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();
    store.open_text(uri.clone(), 1, source.clone(), DocumentKind::Diagram);

    let older_request = store.begin_analyzer_configuration_request();
    let older_plan = match store
        .prepare_analyzer_options(
            older_request,
            default_lsp_analysis_options().with_max_source_bytes(Some(8)),
        )
        .expect("the first request is current")
    {
        AnalyzerOptionsPreparation::RequiresSourceLimitProjection(plan) => plan,
        AnalyzerOptionsPreparation::Applied(_, _) => {
            panic!("the lower source limit must require source projection")
        }
    };
    let older_batch = older_plan
        .project()
        .expect("test projection should not be cancelled");

    let latest_request = store.begin_analyzer_configuration_request();
    let latest = store
        .prepare_analyzer_options(latest_request, default_lsp_analysis_options())
        .expect("the latest request is current");
    assert!(matches!(
        latest,
        AnalyzerOptionsPreparation::Applied(
            crate::session::documents::AnalyzerConfigurationChange::Unchanged,
            None
        )
    ));

    assert!(!store.is_analyzer_configuration_request_current(older_request));
    assert!(
        store
            .commit_source_limit_reclassification(older_batch)
            .is_none()
    );
    assert_eq!(
        store.analyzer_options().max_source_bytes(),
        Some(DEFAULT_LSP_MAX_SOURCE_BYTES)
    );
    let document = store.get(&uri).expect("latest configuration keeps source");
    assert_eq!(document.text.as_ref(), source);
    assert_eq!(document.resource_limit, None);
}

#[test]
fn full_replacement_recovers_from_resource_limited_document() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/large.mmd").unwrap();

    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));
    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "A-->B\n".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected recovered document");
    assert_eq!(stored.version, 2);
    assert_eq!(stored.text.as_ref(), "A-->B\n");
    assert_eq!(stored.resource_limit, None);
    assert_eq!(stored.discarded_source, None);
}

#[test]
fn ranged_changes_on_resource_limited_documents_keep_lightweight_state() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/large.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();

    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));
    store.open_text(uri.clone(), 1, source.clone(), DocumentKind::Diagram);

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(1, 0), Position::new(1, 1))),
            range_length: None,
            text: "C".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::NeedsFullSync);
    let stored = store.get(&uri).expect("expected limited document");
    assert_eq!(stored.version, 2);
    assert_eq!(stored.text.as_ref(), "");
    assert_eq!(stored.resource_limit, None);
    assert_eq!(stored.discarded_source, None);
    assert_eq!(
        stored.sync_error,
        Some(DocumentSyncError::FullReplacementRequired {
            source_len: source.len(),
            last_max_source_bytes: 8,
        })
    );
    assert!(store.snapshot(&uri).is_none());
}

#[test]
fn resource_limited_documents_update_limit_when_configuration_still_excludes_them() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/large.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();

    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));
    store.open_text(uri.clone(), 1, source.clone(), DocumentKind::Diagram);
    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(16)));

    let stored = store.get(&uri).expect("expected limited document");
    assert_eq!(
        stored.resource_limit,
        Some(DocumentResourceLimit {
            source_len: source.len(),
            max_source_bytes: 16,
            span: source_limit_diagnostic_span(&source),
        })
    );
    assert_eq!(stored.discarded_source, None);
    assert!(store.snapshot(&uri).is_none());
}

#[test]
fn resource_limited_documents_become_discarded_when_configuration_would_allow_them() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/large.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();

    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));
    store.open_text(uri.clone(), 1, source.clone(), DocumentKind::Diagram);
    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(64)));

    let stored = store.get(&uri).expect("expected discarded document");
    assert_eq!(stored.resource_limit, None);
    assert_eq!(
        stored.discarded_source,
        Some(DocumentDiscardedSource {
            source_len: source.len(),
            previous_max_source_bytes: 8,
            span: source_limit_diagnostic_span(&source),
        })
    );
    assert!(store.snapshot(&uri).is_none());

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: source.clone(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected recovered document");
    assert_eq!(stored.resource_limit, None);
    assert_eq!(stored.discarded_source, None);
    assert_eq!(stored.text.as_ref(), source);
}

#[test]
fn upsert_text_invalidates_cached_snapshot() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let first = store
        .snapshot(&uri)
        .expect("expected initial lazy snapshot");
    assert_eq!(first.version(), 1);
    assert!(store.has_snapshot(&uri));

    store.upsert_text(
        uri.clone(),
        2,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(!store.has_snapshot(&uri));
    let second = store
        .snapshot(&uri)
        .expect("expected refreshed lazy snapshot");
    assert_eq!(second.version(), 2);
    assert_eq!(second.fences()[0].diagram_type(), Some("sequence"));
}

#[test]
fn apply_text_change_rejects_missing_documents() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/missing.mmd").unwrap();

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::MissingDocument);
    assert!(store.get(&uri).is_none());
    assert!(!store.has_snapshot(&uri));
}

#[test]
fn apply_text_change_rejects_stale_versions_without_invalidating_current_state() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        3,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot = store
        .snapshot(&uri)
        .expect("expected current snapshot before stale edit");
    assert_eq!(snapshot.version(), 3);
    assert!(store.has_snapshot(&uri));

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "flowchart TD\nA-->B\n".to_string(),
        }],
    );

    assert_eq!(
        update,
        TextDocumentUpdate::StaleVersion {
            current_version: 3,
            attempted_version: 2,
        }
    );
    let stored = store.get(&uri).expect("expected current document");
    assert_eq!(stored.version, 3);
    assert!(stored.text.contains("sequenceDiagram"));
    assert!(store.has_snapshot(&uri));
}

#[test]
fn apply_text_changes_applies_lsp_utf16_ranges_in_order() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA[🤓]-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(
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

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected updated document");
    assert_eq!(stored.version, 2);
    assert_eq!(stored.text.as_ref(), "flowchart TD\nA[C]-->B\nC-->D\n");
}

#[test]
fn prepared_text_changes_cannot_overwrite_a_newer_document_epoch() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/prepared-change-cas.mmd").unwrap();
    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let TextChangePreparation::Prepare(plan) = store.capture_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "flowchart TD\nA-->C\n".to_string(),
        }],
    ) else {
        panic!("valid change should require lock-free preparation");
    };

    store.open_text(
        uri.clone(),
        3,
        "sequenceDiagram\nAlice->>Bob: newer\n".to_string(),
        DocumentKind::Diagram,
    );
    let update = store.commit_prepared_text_changes(
        plan.prepare()
            .expect("test text preparation should not be cancelled"),
    );

    assert_eq!(update, TextDocumentUpdate::Superseded);
    let stored = store
        .get(&uri)
        .expect("newer document should remain stored");
    assert_eq!(stored.version, 3);
    assert!(stored.text.starts_with("sequenceDiagram"));
}

#[test]
fn session_cancellation_aborts_pending_text_change_preparation() {
    let cancellation = AnalysisCancellationToken::new();
    let mut store = DocumentStore::with_session_cancellation(cancellation.clone());
    let uri = Uri::from_str("file:///tmp/cancelled-change.mmd").unwrap();
    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let TextChangePreparation::Prepare(plan) = store.capture_text_changes(
        uri,
        2,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "flowchart TD\nA-->C\n".to_string(),
        }],
    ) else {
        panic!("valid change should require lock-free preparation");
    };

    cancellation.cancel();

    assert!(matches!(
        plan.prepare(),
        Err(merman_analysis::AnalysisCancelled)
    ));
}

#[test]
fn apply_text_changes_updates_line_index_between_batched_edits() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\r\nA[🤓]\rB\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(
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

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected updated document");
    assert_eq!(stored.text.as_ref(), "flowchart TD\r\nA[C\nE]\rB-->C\n");
}

#[test]
fn apply_text_changes_allows_nonconsecutive_versions_for_incremental_ranges() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(
        uri.clone(),
        3,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(1, 0), Position::new(1, 1))),
            range_length: None,
            text: "C".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected updated document");
    assert_eq!(stored.version, 3);
    assert_eq!(stored.text.as_ref(), "flowchart TD\nC-->B\n");
}

#[test]
fn apply_text_changes_rejects_empty_change_sets_without_advancing_version() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(uri.clone(), 3, []);

    assert_eq!(update, TextDocumentUpdate::EmptyChangeSet);
    let stored = store.get(&uri).expect("expected current document");
    assert_eq!(stored.version, 1);
    assert_eq!(stored.text.as_ref(), "flowchart TD\nA-->B\n");
}

#[test]
fn apply_text_changes_allows_skipped_versions_for_full_replacements() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(
        uri.clone(),
        4,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected updated document");
    assert_eq!(stored.version, 4);
    assert_eq!(stored.text.as_ref(), "sequenceDiagram\nAlice->>Bob: Hi\n");
}

#[test]
fn apply_text_changes_marks_document_unsynced_after_invalid_range() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot = store
        .snapshot(&uri)
        .expect("expected current snapshot before invalid edit");
    assert_eq!(snapshot.version(), 1);
    assert!(store.has_snapshot(&uri));

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(20, 0), Position::new(20, 1))),
            range_length: None,
            text: "bad".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::InvalidRange);
    let stored = store.get(&uri).expect("expected current document");
    assert_eq!(stored.version, 2);
    assert_eq!(stored.text.as_ref(), "");
    assert_eq!(stored.resource_limit, None);
    assert_eq!(stored.discarded_source, None);
    assert_eq!(
        stored.sync_error,
        Some(DocumentSyncError::InvalidIncrementalRange)
    );
    assert!(!store.has_snapshot(&uri));
}

#[test]
fn apply_text_changes_marks_document_unsynced_after_reversed_range() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(1, 4), Position::new(1, 2))),
            range_length: None,
            text: "bad".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::InvalidRange);
    let stored = store.get(&uri).expect("expected unsynced document");
    assert_eq!(stored.text.as_ref(), "");
    assert_eq!(
        stored.sync_error,
        Some(DocumentSyncError::InvalidIncrementalRange)
    );
}

#[test]
fn apply_text_changes_clamps_utf16_positions_past_line_end() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA[🤓]-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(
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

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected updated document");
    assert_eq!(stored.version, 2);
    assert_eq!(stored.text.as_ref(), "flowchart TD\nA[🤓]-->Bbad\n");
    assert_eq!(stored.sync_error, None);
}

#[test]
fn full_replacement_recovers_from_unsynced_document_after_invalid_range() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert_eq!(
        store.apply_text_changes(
            uri.clone(),
            2,
            [TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(20, 0), Position::new(20, 1))),
                range_length: None,
                text: "bad".to_string(),
            }],
        ),
        TextDocumentUpdate::InvalidRange
    );

    let update = store.apply_text_changes(
        uri.clone(),
        3,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected recovered document");
    assert_eq!(stored.version, 3);
    assert_eq!(stored.text.as_ref(), "sequenceDiagram\nAlice->>Bob: Hi\n");
    assert_eq!(stored.sync_error, None);
}

#[test]
fn ranged_changes_on_unsynced_documents_keep_lightweight_state() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert_eq!(
        store.apply_text_changes(
            uri.clone(),
            2,
            [TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(20, 0), Position::new(20, 1))),
                range_length: None,
                text: "bad".to_string(),
            }],
        ),
        TextDocumentUpdate::InvalidRange
    );

    let update = store.apply_text_changes(
        uri.clone(),
        3,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(1, 0), Position::new(1, 1))),
            range_length: None,
            text: "C".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::NeedsFullSync);
    let stored = store.get(&uri).expect("expected unsynced document");
    assert_eq!(stored.version, 3);
    assert_eq!(stored.text.as_ref(), "");
    assert_eq!(
        stored.sync_error,
        Some(DocumentSyncError::InvalidIncrementalRange)
    );
}

#[test]
fn full_replacement_later_in_unsynced_batch_recovers_document() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert_eq!(
        store.apply_text_changes(
            uri.clone(),
            2,
            [TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(20, 0), Position::new(20, 1))),
                range_length: None,
                text: "bad".to_string(),
            }],
        ),
        TextDocumentUpdate::InvalidRange
    );

    let update = store.apply_text_changes(
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

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected recovered document");
    assert_eq!(stored.version, 3);
    assert_eq!(stored.text.as_ref(), "sequenceDiagram\nAlice->>Bob: Hi\n");
    assert_eq!(stored.sync_error, None);
}

#[test]
fn full_replacement_later_in_available_batch_ignores_prior_invalid_ranges() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(
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

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected replaced document");
    assert_eq!(stored.version, 2);
    assert_eq!(stored.text.as_ref(), "sequenceDiagram\nAlice->>Bob: Hi\n");
    assert_eq!(stored.sync_error, None);
}

#[test]
fn stale_snapshot_build_request_is_not_committed_after_text_replacement() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let (snapshots, mut requests) = store.snapshot_build_requests();
    assert!(snapshots.is_empty());
    assert_eq!(requests.len(), 1);
    let stale_request = requests.pop().unwrap();
    let stale_snapshot = stale_request
        .build()
        .expect("test source should be accepted");

    store.upsert_text(
        uri.clone(),
        2,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );

    let committed = store.snapshot_contexts_for_requests(vec![(stale_request, stale_snapshot)]);
    assert!(committed.contexts.is_empty());
    assert!(committed.stale_open_documents);
    assert!(!store.has_snapshot(&uri));

    let current = store
        .snapshot(&uri)
        .expect("current snapshot should build after rejecting stale request");
    assert_eq!(current.version(), 2);
    assert_eq!(current.fences()[0].diagram_type(), Some("sequence"));
}

#[test]
fn snapshot_build_requests_reuse_current_cached_snapshots() {
    let mut store = DocumentStore::new();
    let cached_uri = Uri::from_str("file:///tmp/cached.mmd").unwrap();
    let missing_uri = Uri::from_str("file:///tmp/missing.mmd").unwrap();

    store.upsert_text(
        cached_uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let cached_snapshot = store
        .snapshot(&cached_uri)
        .expect("expected cached snapshot");
    store.upsert_text(
        missing_uri.clone(),
        1,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );

    let (contexts, mut requests) = store.snapshot_build_requests();

    assert_eq!(contexts.len(), 1);
    assert_eq!(contexts[0].snapshot.uri(), &cached_uri);
    assert!(std::sync::Arc::ptr_eq(
        &contexts[0].snapshot,
        &cached_snapshot
    ));
    assert!(store.is_snapshot_context_current(&contexts[0]));
    assert_eq!(requests.len(), 1);
    let request = requests.pop().unwrap();
    let built = request.build().expect("test source should be accepted");
    let committed = store.snapshot_contexts_for_requests(vec![(request, built)]);
    assert_eq!(committed.contexts.len(), 1);
    assert_eq!(committed.contexts[0].snapshot.uri(), &missing_uri);
    assert!(!committed.stale_open_documents);
}

#[test]
fn initial_lsp_payload_matches_the_sealed_generation_source() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/shared-payload.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    store.snapshot(&uri).expect("expected cached snapshot");
    let context = store
        .cached_analysis_generation(&uri)
        .expect("expected cached analysis generation");
    assert_eq!(context.payload.source, *context.generation().source());
}

#[test]
fn cached_snapshot_build_context_stales_after_text_replacement() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/cached.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let cached_snapshot = store.snapshot(&uri).expect("expected cached snapshot");

    let (contexts, requests) = store.snapshot_build_requests();
    assert_eq!(contexts.len(), 1);
    assert!(requests.is_empty());
    assert!(std::sync::Arc::ptr_eq(
        &contexts[0].snapshot,
        &cached_snapshot
    ));

    store.upsert_text(
        uri,
        2,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(!store.is_snapshot_contexts_current(&contexts));
}

#[test]
fn unchanged_analyzer_update_preserves_context_generations_snapshots_and_tokens() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot_context = store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    let diagnostic_context = store
        .diagnostic_context(&uri)
        .expect("expected initial diagnostic context");
    assert!(store.set_semantic_tokens_state_if_current(
        &snapshot_context,
        SemanticTokensState::new(Some("tokens-1".to_string()), Vec::new()),
    ));

    assert_eq!(
        store.apply_analyzer_options(default_lsp_analysis_options()),
        crate::session::documents::AnalyzerConfigurationChange::Unchanged
    );

    assert!(store.is_snapshot_context_current(&snapshot_context));
    assert!(store.is_diagnostic_context_current(&diagnostic_context));
    assert!(store.has_snapshot(&uri));
    assert_eq!(
        store
            .semantic_tokens_state(&uri)
            .and_then(|state| state.result_id.as_deref()),
        Some("tokens-1")
    );
}

#[test]
fn diagnostic_only_analyzer_update_reprojects_the_cached_generation() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot_context = store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    let initial_payload = snapshot_context.analysis_payload() as *const AnalysisPayload;
    let initial_text_index = snapshot_context.snapshot.fences()[0].text_index() as *const _;
    let canonical = Arc::clone(
        store
            .cached_analysis_generation(&uri)
            .expect("expected cached analysis generation")
            .generation(),
    );
    let analyzer_environment_identity = store.analyzer_environment_identity().clone();
    let diagnostic_context = store
        .diagnostic_context(&uri)
        .expect("expected initial diagnostic context");
    assert!(store.set_semantic_tokens_state_if_current(
        &snapshot_context,
        SemanticTokensState::new(Some("tokens-1".to_string()), Vec::new()),
    ));

    let (change, plan) = store.begin_analyzer_options(
        default_lsp_analysis_options().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                .unwrap(),
        ),
    );

    assert_eq!(
        change,
        crate::session::documents::AnalyzerConfigurationChange::DiagnosticsOnly
    );
    assert_eq!(
        store.analyzer_environment_identity(),
        &analyzer_environment_identity
    );
    assert!(store.is_snapshot_context_current(&snapshot_context));
    assert!(!store.is_analysis_context_current(&snapshot_context));
    assert!(!store.is_diagnostic_context_current(&diagnostic_context));
    assert!(store.has_snapshot(&uri));
    assert!(!store.has_analysis_payload(&uri));
    assert!(
        store.snapshot_build_request(&uri).is_none(),
        "a stale diagnostic projection must not trigger another parse"
    );

    let committed = store.commit_diagnostic_reprojection(
        plan.expect("cached analysis should produce a reprojection plan")
            .project()
            .expect("current reprojection should complete"),
    );

    assert_eq!(committed, 1);
    assert!(store.has_analysis_payload(&uri));
    let current_context = store
        .snapshot_context(&uri)
        .expect("expected reprojected analysis context");
    assert!(store.is_analysis_context_current(&current_context));
    assert!(Arc::ptr_eq(
        &snapshot_context.snapshot,
        &current_context.snapshot
    ));
    assert_ne!(
        current_context.analysis_payload() as *const AnalysisPayload,
        initial_payload
    );
    assert_eq!(
        current_context.snapshot.fences()[0].text_index() as *const _,
        initial_text_index
    );
    assert!(Arc::ptr_eq(
        &canonical,
        store
            .cached_analysis_generation(&uri)
            .expect("expected reprojected analysis generation")
            .generation()
    ));
    assert_eq!(
        store
            .semantic_tokens_state(&uri)
            .and_then(|state| state.result_id.as_deref()),
        Some("tokens-1")
    );
}

#[test]
fn diagnostic_reprojection_does_not_overwrite_a_newer_document_epoch() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    let (_, plan) = store.begin_analyzer_options(
        default_lsp_analysis_options().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                .unwrap(),
        ),
    );
    let batch = plan
        .expect("cached analysis should produce a reprojection plan")
        .project()
        .expect("current reprojection should complete");

    store.upsert_text(
        uri.clone(),
        2,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );

    assert_eq!(store.commit_diagnostic_reprojection(batch), 0);
    assert!(!store.has_snapshot(&uri));
    assert!(!store.has_analysis_payload(&uri));
    assert_eq!(
        store
            .get(&uri)
            .expect("expected replacement document")
            .version,
        2
    );
}

#[test]
fn diagnostic_reprojection_does_not_commit_a_superseded_policy_epoch() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    let (_, first_plan) = store.begin_analyzer_options(
        default_lsp_analysis_options().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                .unwrap(),
        ),
    );
    let (_, second_plan) = store.begin_analyzer_options(
        default_lsp_analysis_options().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Warning)
                .unwrap(),
        ),
    );

    let first_projection = first_plan
        .expect("first policy should produce a reprojection plan")
        .project();
    let second_batch = second_plan
        .expect("second policy should produce a reprojection plan")
        .project()
        .expect("current reprojection should complete");

    assert!(matches!(
        first_projection,
        Err(merman_analysis::AnalysisCancelled)
    ));
    assert!(!store.has_analysis_payload(&uri));
    assert_eq!(store.commit_diagnostic_reprojection(second_batch), 1);
    assert!(store.has_analysis_payload(&uri));
}

#[test]
fn text_replacement_stales_contexts_but_keeps_committed_token_baseline() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot_context = store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    let diagnostic_context = store
        .diagnostic_context(&uri)
        .expect("expected initial diagnostic context");
    assert!(store.set_semantic_tokens_state_if_current(
        &snapshot_context,
        SemanticTokensState::new(Some("tokens-1".to_string()), Vec::new()),
    ));

    store.upsert_text(
        uri.clone(),
        1,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(!store.is_snapshot_context_current(&snapshot_context));
    assert!(!store.is_diagnostic_context_current(&diagnostic_context));
    assert!(!store.has_snapshot(&uri));
    assert_eq!(
        store
            .semantic_tokens_state(&uri)
            .and_then(|state| state.result_id.as_deref()),
        Some("tokens-1")
    );
}

#[test]
fn snapshot_affecting_analyzer_update_stales_all_contexts_and_clears_snapshot_state() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot_context = store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    let diagnostic_context = store
        .diagnostic_context(&uri)
        .expect("expected initial diagnostic context");
    assert!(store.set_semantic_tokens_state_if_current(
        &snapshot_context,
        SemanticTokensState::new(Some("tokens-1".to_string()), Vec::new()),
    ));

    store.apply_analyzer_options(
        AnalysisOptions::default().with_max_source_bytes(Some("flowchart TD\nA-->B\n".len() - 1)),
    );

    assert!(!store.is_snapshot_context_current(&snapshot_context));
    assert!(!store.is_diagnostic_context_current(&diagnostic_context));
    assert!(!store.has_snapshot(&uri));
    assert!(store.semantic_tokens_state(&uri).is_none());
}

#[test]
fn remove_stales_existing_contexts_and_clears_document_state() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot_context = store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    let diagnostic_context = store
        .diagnostic_context(&uri)
        .expect("expected initial diagnostic context");
    assert!(store.set_semantic_tokens_state_if_current(
        &snapshot_context,
        SemanticTokensState::new(Some("tokens-1".to_string()), Vec::new()),
    ));

    store.remove(&uri);

    assert!(store.get(&uri).is_none());
    assert!(!store.has_snapshot(&uri));
    assert!(store.semantic_tokens_state(&uri).is_none());
    assert!(!store.is_snapshot_context_current(&snapshot_context));
    assert!(!store.is_diagnostic_context_current(&diagnostic_context));
}

#[test]
fn stale_snapshot_context_cannot_record_semantic_token_state_after_text_replacement() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot_context = store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");

    store.upsert_text(
        uri.clone(),
        1,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(!store.set_semantic_tokens_state_if_current(
        &snapshot_context,
        SemanticTokensState::new(Some("tokens-stale".to_string()), Vec::new()),
    ));
    assert!(store.semantic_tokens_state(&uri).is_none());
}

#[test]
fn semantic_token_delta_baseline_survives_text_replacement_but_not_snapshot_config_change() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot_context = store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    assert!(store.set_semantic_tokens_state_if_current(
        &snapshot_context,
        SemanticTokensState::new(
            Some("tokens-1".to_string()),
            vec![SemanticToken {
                delta_line: 0,
                delta_start: 0,
                length: 4,
                token_type: 0,
                token_modifiers_bitset: 0,
            }],
        ),
    ));

    store.upsert_text(
        uri.clone(),
        2,
        "flowchart TD\nAlpha-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(
        store
            .semantic_tokens_state_for_delta(&uri, "tokens-1")
            .is_some(),
        "text edits should preserve the last committed token state as a delta baseline"
    );

    store.apply_analyzer_options(
        AnalysisOptions::default()
            .with_max_source_bytes(Some("flowchart TD\nAlpha-->B\n".len() - 1)),
    );

    assert!(
        store
            .semantic_tokens_state_for_delta(&uri, "tokens-1")
            .is_none()
    );
}

#[test]
fn snapshot_affecting_source_limit_update_rejects_the_cached_generation() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot = store
        .snapshot(&uri)
        .expect("expected initial lazy snapshot");
    assert_eq!(snapshot.fences()[0].diagram_type(), Some("flowchart-v2"));
    let token_context = store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    assert!(store.set_semantic_tokens_state_if_current(
        &token_context,
        SemanticTokensState::new(Some("tokens-1".to_string()), Vec::new()),
    ));

    store.apply_analyzer_options(
        AnalysisOptions::default().with_max_source_bytes(Some("flowchart TD\nA-->B\n".len() - 1)),
    );

    assert!(!store.has_snapshot(&uri));
    assert!(store.semantic_tokens_state(&uri).is_none());
    let stored = store.get(&uri).expect("expected resource-limited document");
    assert_eq!(stored.text.as_ref(), "");
    assert_eq!(
        stored.resource_limit,
        Some(DocumentResourceLimit {
            source_len: "flowchart TD\nA-->B\n".len(),
            max_source_bytes: "flowchart TD\nA-->B\n".len() - 1,
            span: source_limit_diagnostic_span("flowchart TD\nA-->B\n"),
        })
    );
    assert!(store.snapshot_build_request(&uri).is_none());
    assert!(store.snapshot(&uri).is_none());
}

#[test]
fn incomplete_flowchart_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "flowchart TD\nsubgraph group\nA-->B\nC-->".to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "A"));
    assert!(index.node_ids().any(|id| id == "C"));
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "group")
    );
}

#[test]
fn sequence_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "sequenceDiagram\nparticipant Alice\nactor Bob\nAlice->>Bob: Hi\n".to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "Alice"));
    assert!(index.node_ids().any(|id| id == "Bob"));
}

#[test]
fn sequence_payload_facts_do_not_pollute_completion_ids() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "sequenceDiagram\n",
            "title: Diagram Title\n",
            "accTitle: Accessible Title\n",
            "accDescr: Accessible Description\n",
            "participant Alice\n",
            "actor Bob\n",
            "Alice->>Bob: Hello\n",
            "Note over Alice,Bob: Review\n",
            "details Alice: {\"owner\": \"platform\"}\n",
            "links Alice: { \"Repo\": \"https://example.com/\" }\n",
            "link Alice: Endpoint @ https://alice.example.com\n",
            "properties Alice: {\"class\": \"internal-service-actor\"}\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "Alice"));
    assert!(index.node_ids().any(|id| id == "Bob"));
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "Alice" && item.role == FenceSemanticRole::Entity)
    );
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "Bob" && item.role == FenceSemanticRole::Entity)
    );
    for payload in [
        "Diagram Title",
        "Accessible Title",
        "Accessible Description",
        "Hello",
        "Review",
        r#"{"owner": "platform"}"#,
        r#"{ "Repo": "https://example.com/" }"#,
        "Endpoint @ https://alice.example.com",
        r#"{"class": "internal-service-actor"}"#,
    ] {
        assert!(
            index
                .semantic_items()
                .iter()
                .any(|item| item.name == payload && item.role == FenceSemanticRole::Payload),
            "sequence payload {payload:?} was not retained as a semantic item"
        );
    }

    for leaked in [
        "Diagram Title",
        "Accessible Title",
        "Accessible Description",
        "Hello",
        "Review",
        r#"{"owner": "platform"}"#,
        "Repo",
        "Endpoint",
        "https://example.com/",
        "https://alice.example.com",
        "internal-service-actor",
    ] {
        assert!(
            !index.node_ids().any(|id| id == leaked),
            "sequence payload leaked {leaked:?} into completion ids"
        );
        assert!(
            !index.outline_items().iter().any(|item| item.name == leaked),
            "sequence payload leaked {leaked:?} into outline items"
        );
    }

    for prefix in [
        "title",
        "accTitle",
        "accDescr",
        "details",
        "links",
        "link",
        "properties",
    ] {
        assert!(index.has_directive_prefix(prefix));
    }
}

#[test]
fn architecture_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "architecture-beta\n",
            "group platform(cloud)[Platform]\n",
            "service api(server)[API] in platform\n",
            "junction hub in platform\n",
            "api:R -- L:hub\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(snapshot.fences()[0].diagram_type(), Some("architecture"));
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    for id in ["platform", "api", "hub"] {
        assert!(index.node_ids().any(|candidate| candidate == id));
    }
    assert!(index.outline_items().iter().any(
        |item| item.name == "platform" && item.detail.as_deref() == Some("architecture group")
    ));
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "Platform" && item.role == FenceSemanticRole::Payload)
    );
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "API" && item.role == FenceSemanticRole::Payload)
    );
}

#[test]
fn radar_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "radar-beta\n",
            "title Radar diagram\n",
            "accTitle: Radar accTitle\n",
            "accDescr: Radar accDescription\n",
            "axis A[\"Axis A\"], B[\"Axis B\"], C[\"Axis C\"]\n",
            "curve mycurve[\"My Curve\"]{1,2,3}\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(snapshot.fences()[0].diagram_type(), Some("radar"));
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    for id in ["A", "B", "C", "mycurve"] {
        assert!(index.node_ids().any(|candidate| candidate == id));
    }
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "Axis A" && item.role == FenceSemanticRole::Payload)
    );
}

#[test]
fn treemap_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "treemap\n",
            "title Treemap Title\n",
            "accTitle: Treemap accTitle\n",
            "accDescr: Treemap accDescr\n",
            "\"Root\"\n",
            "  \"Leaf\": 42 :::highlight\n",
            "classDef highlight fill:#f00\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(snapshot.fences()[0].diagram_type(), Some("treemap"));
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|candidate| candidate == "Root"));
    assert!(index.node_ids().any(|candidate| candidate == "Leaf"));
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "highlight" && item.role == FenceSemanticRole::Outline)
    );
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "42" && item.role == FenceSemanticRole::Payload)
    );
}

#[test]
fn block_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "block\n",
            "  columns 2\n",
            "  block:group[\"Group label\"]\n",
            "    A[\"Start\"] -- \"edge label\" --> B[\"End\"]\n",
            "  end\n",
            "  arrow<[\"go\"]>(right, down)\n",
            "  classDef hot fill:#f00\n",
            "  class A,B hot\n",
            "  style B stroke:#333\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(snapshot.fences()[0].diagram_type(), Some("block"));
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    for id in ["group", "A", "B", "arrow"] {
        assert!(index.node_ids().any(|candidate| candidate == id));
    }
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "hot" && item.role == FenceSemanticRole::Outline)
    );
    for payload in ["Group label", "Start", "edge label", "End", "go", "right"] {
        assert!(
            index
                .semantic_items()
                .iter()
                .any(|item| item.name == payload && item.role == FenceSemanticRole::Payload),
            "missing block payload semantic item {payload:?}"
        );
    }
}

#[test]
fn c4_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "C4Context\n",
            "title Banking Context\n",
            "accTitle: Banking accessibility title\n",
            "accDescr: Banking accessibility description\n",
            "Boundary(bank, \"Bank\") {\n",
            "Person(customer, \"Customer\", \"Uses the system\")\n",
            "System(system, \"Internet Banking\", \"Core system\")\n",
            "}\n",
            "Rel(customer, system, \"Uses\", \"HTTPS\")\n",
            "UpdateElementStyle(system, $bgColor=\"red\")\n",
            "UpdateRelStyle(customer, system, $lineColor=\"blue\")\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(snapshot.fences()[0].diagram_type(), Some("c4"));
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    for id in ["bank", "customer", "system"] {
        assert!(index.node_ids().any(|candidate| candidate == id));
    }
    for prefix in ["title", "accTitle", "accDescr"] {
        assert!(index.has_directive_prefix(prefix));
    }
    for payload in [
        "Banking Context",
        "Banking accessibility title",
        "Banking accessibility description",
        "Bank",
        "Customer",
        "Uses the system",
        "Internet Banking",
        "Core system",
        "Uses",
        "HTTPS",
        "red",
        "blue",
    ] {
        assert!(
            index
                .semantic_items()
                .iter()
                .any(|item| item.name == payload && item.role == FenceSemanticRole::Payload),
            "missing C4 payload semantic item {payload:?}"
        );
        assert!(!index.node_ids().any(|candidate| candidate == payload));
    }
}

#[test]
fn zenuml_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "zenuml\n",
            "title Login Flow\n",
            "Alice\n",
            "Bob\n",
            "A as API\n",
            "accTitle: Login accessibility title\n",
            "accDescr: Login accessibility description\n",
            "Alice->Bob: Login\n",
            "SomeType result = A.SyncMessage()\n",
            "new Session(with, params)\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(snapshot.fences()[0].diagram_type(), Some("zenuml"));
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    for id in ["Alice", "Bob", "A", "accTitle", "accDescr", "Session"] {
        assert!(index.node_ids().any(|candidate| candidate == id));
    }
    assert!(index.has_directive_prefix("title"));
    assert!(!index.has_directive_prefix("accTitle"));
    assert!(!index.has_directive_prefix("accDescr"));
    for payload in [
        "Login Flow",
        "Login accessibility title",
        "Login accessibility description",
        "API",
        "Login",
        "SyncMessage",
        "result",
    ] {
        assert!(
            index
                .semantic_items()
                .iter()
                .any(|item| item.name == payload && item.role == FenceSemanticRole::Payload),
            "missing ZenUML payload semantic item {payload:?}"
        );
        assert!(!index.node_ids().any(|candidate| candidate == payload));
    }
}

#[test]
fn newer_family_documents_keep_parser_facts_when_recovered() {
    for case in [
        (
            "gitGraph",
            concat!("gitGraph\n", "commit id:\"C1\"\n", "commit id:\"broken\n",),
            "C1",
            FenceSemanticRole::Entity,
        ),
        (
            "radar",
            concat!(
                "radar-beta\n",
                "axis A[\"Axis A\"], B[\"Axis B\"]\n",
                "curve mycurve{1,2}\n",
                "curve broken\n",
            ),
            "A",
            FenceSemanticRole::Entity,
        ),
        (
            "kanban",
            concat!(
                "kanban\n",
                "    root\n",
                "      child1\n",
                "      broken[unfinished\n",
            ),
            "child1",
            FenceSemanticRole::Entity,
        ),
        (
            "treemap",
            concat!(
                "treemap\n",
                "\"Root\"\n",
                "  \"Leaf\": 42\n",
                "\"Broken\":\n",
            ),
            "Leaf",
            FenceSemanticRole::Entity,
        ),
        (
            "block",
            concat!(
                "block\n",
                "  block:group[\"Group label\"]\n",
                "    A[\"Start\"]\n",
            ),
            "A",
            FenceSemanticRole::Entity,
        ),
        (
            "c4",
            concat!(
                "C4Context\n",
                "Person(customer, \"Customer\")\n",
                "NotAMacro customer\n",
            ),
            "customer",
            FenceSemanticRole::Entity,
        ),
        (
            "wardley",
            concat!(
                "wardley-beta\n",
                "component API [0.6, 0.7]\n",
                "component Broken [\n",
            ),
            "API",
            FenceSemanticRole::Entity,
        ),
        (
            "zenuml",
            concat!(
                "zenuml\n",
                "Alice\n",
                "Unsupported ? statement\n",
                "Alice->Bob: Hi\n",
            ),
            "Alice",
            FenceSemanticRole::Entity,
        ),
    ] {
        let mut store = DocumentStore::new();
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri, 1, case.1.to_string());
        let index = snapshot.fences()[0].text_index();

        assert_eq!(
            index.source(),
            FenceTextIndexSource::ParserRecovered,
            "unexpected recovered provenance for {}",
            case.0
        );
        assert!(
            index
                .semantic_items()
                .iter()
                .any(|item| item.name == case.2 && item.role == case.3),
            "missing recovered semantic item {:?} for {}",
            case.2,
            case.0
        );
    }
}

#[test]
fn incomplete_sequence_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "sequenceDiagram\nAlice->>Bob: Hi\nBob->>".to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "Alice"));
    assert!(index.node_ids().any(|id| id == "Bob"));
}

#[test]
fn state_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "stateDiagram-v2\n",
            "[*] --> Idle\n",
            "Idle --> Running\n",
            "Idle: Waiting state\n",
            "Idle --> Running: starts\n",
            "state \"Paused State\" as Paused\n",
            "note right of Running : Running details\n",
            "note \"Floating note\" as note1\n",
            "classDef activeStyle fill:#0f0,border:#333\n",
            "class Idle, Running activeStyle\n",
            "style Running fill:#f00\n",
            "accTitle: Lifecycle chart\n",
            "accDescr: Shows state transitions\n",
            "click Running \"https://example.com/run\" \"Run details\"\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "Idle"));
    assert!(index.node_ids().any(|id| id == "Running"));
    assert!(!index.node_ids().any(|id| id == "activeStyle"));
    assert!(!index.node_ids().any(|id| id == "Waiting state"));
    assert!(!index.node_ids().any(|id| id == "starts"));
    assert!(!index.node_ids().any(|id| id == "Paused State"));
    assert!(!index.node_ids().any(|id| id == "Running details"));
    assert!(!index.node_ids().any(|id| id == "Floating note"));
    assert!(!index.node_ids().any(|id| id == "note1"));
    assert!(!index.node_ids().any(|id| id == "fill:#0f0,border:#333"));
    assert!(!index.node_ids().any(|id| id == "fill:#f00"));
    assert!(!index.node_ids().any(|id| id == "Lifecycle chart"));
    assert!(!index.node_ids().any(|id| id == "Shows state transitions"));
    assert!(!index.node_ids().any(|id| id == "https://example.com/run"));
    assert!(!index.node_ids().any(|id| id == "Run details"));
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "activeStyle"
                && item.detail.as_deref() == Some("state class definition"))
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Waiting state")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "starts")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Paused State")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Running details")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Lifecycle chart")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "https://example.com/run")
    );
}

#[test]
fn incomplete_state_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "stateDiagram-v2\nIdle --> Running\nRunning -->".to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "Idle"));
    assert!(index.node_ids().any(|id| id == "Running"));
}

#[test]
fn class_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "classDiagram\nclass User\nUser <|-- Admin\n".to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "User"));
    assert!(index.node_ids().any(|id| id == "Admin"));
}

#[test]
fn incomplete_class_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "classDiagram\nclass User\nUser <|--".to_string());
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "User"));
}

#[test]
fn class_member_outline_facts_do_not_pollute_completion_ids() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "classDiagram\n",
            "class User {\n",
            "  +login()\n",
            "  -password: String\n",
            "}\n",
            "class Visible[\"Visible label\"]\n",
            "<<interface>> User\n",
            "User: email\n",
            "Class1 \"1\" *-- \"many\" Class02 : contains\n",
            "User <|-- Admin : manages\n",
            "note for User \"Primary user\"\n",
            "note \"Floating note\"\n",
            "click User href \"https://example.com\" \"Open user\" _blank\n",
            "click User call open(userId) \"Open user\"\n",
            "accTitle: Class chart\n",
            "accDescr: Shows class relationships\n",
            "classDef service fill:#eee\n",
            "class User:::service\n",
            "cssClass \"User,Admin\" service\n",
            "style User fill:#fff\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.has_directive_prefix("classDef"));
    assert!(index.has_directive_prefix("class"));
    assert!(index.has_directive_prefix("cssClass"));
    assert!(index.has_directive_prefix("style"));
    assert!(index.has_directive_prefix("click"));
    assert!(index.node_ids().any(|id| id == "User"));
    assert!(!index.node_ids().any(|id| id == "+login()"));
    assert!(!index.node_ids().any(|id| id == "-password: String"));
    assert!(!index.node_ids().any(|id| id == "email"));
    assert!(!index.node_ids().any(|id| id == "interface"));
    assert!(!index.node_ids().any(|id| id == "https://example.com"));
    assert!(!index.node_ids().any(|id| id == "Open user"));
    assert!(!index.node_ids().any(|id| id == "_blank"));
    assert!(!index.node_ids().any(|id| id == "service"));
    assert!(!index.node_ids().any(|id| id == "fill:#eee"));
    assert!(!index.node_ids().any(|id| id == "fill:#fff"));
    assert!(!index.node_ids().any(|id| id == "open"));
    assert!(!index.node_ids().any(|id| id == "userId"));
    assert!(!index.node_ids().any(|id| id == "Visible label"));
    assert!(!index.node_ids().any(|id| id == "1"));
    assert!(!index.node_ids().any(|id| id == "many"));
    assert!(!index.node_ids().any(|id| id == "manages"));
    assert!(!index.node_ids().any(|id| id == "Primary user"));
    assert!(!index.node_ids().any(|id| id == "Floating note"));
    assert!(!index.node_ids().any(|id| id == "Class chart"));
    assert!(!index.node_ids().any(|id| id == "Shows class relationships"));

    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "+login()" && item.detail.as_deref() == Some("class member"))
    );
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "-password: String"
                && item.detail.as_deref() == Some("class member"))
    );
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "email" && item.detail.as_deref() == Some("class member"))
    );
    assert!(
        index.outline_items().iter().any(
            |item| item.name == "service" && item.detail.as_deref() == Some("class definition")
        )
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "interface")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "https://example.com")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Open user")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "_blank")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "fill:#eee")
    );
    assert!(!index.outline_items().iter().any(|item| item.name == "open"));
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "userId")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "manages")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Visible label")
    );
    assert!(!index.outline_items().iter().any(|item| item.name == "1"));
    assert!(!index.outline_items().iter().any(|item| item.name == "many"));
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Primary user")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Class chart")
    );
}

#[test]
fn er_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "erDiagram\nCUSTOMER ||--o{ ORDER : places\n".to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "CUSTOMER"));
    assert!(index.node_ids().any(|id| id == "ORDER"));
}

#[test]
fn incomplete_er_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "erDiagram\nCUSTOMER ||--o{ ORDER :".to_string());
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "CUSTOMER"));
    assert!(index.node_ids().any(|id| id == "ORDER"));
}

#[test]
fn er_attribute_payload_facts_do_not_pollute_completion_ids() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "erDiagram\n",
            "BOOK {\n",
            "  string title PK, FK \"primary title\"\n",
            "}\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "BOOK"));
    assert!(!index.node_ids().any(|id| id == "title"));
    assert!(!index.node_ids().any(|id| id == "string"));
    assert!(!index.node_ids().any(|id| id == "PK"));
    assert!(!index.node_ids().any(|id| id == "FK"));
    assert!(!index.node_ids().any(|id| id == "primary title"));
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "title" && item.detail.as_deref() == Some("er attribute"))
    );
}

#[test]
fn gantt_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "gantt\n",
            "title Roadmap\n",
            "accTitle: Roadmap chart\n",
            "accDescr: Shows release tasks\n",
            "accDescr {\n",
            "  Shows release tasks\n",
            "  across releases\n",
            "}\n",
            "dateFormat YYYY-MM-DD\n",
            "section Demo\n",
            "Task 1: id1,2014-01-01,1d\n",
            "Task 2: id2,after id1,2d\n",
            "click id2 call open(userId) href \"https://example.com/\"\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(snapshot.fences()[0].diagram_type(), Some("gantt"));
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "id1"));
    assert!(index.node_ids().any(|id| id == "id2"));
    assert!(!index.node_ids().any(|id| id == "Demo"));
    assert!(!index.node_ids().any(|id| id == "Roadmap"));
    assert!(!index.node_ids().any(|id| id == "Roadmap chart"));
    assert!(!index.node_ids().any(|id| id == "Shows release tasks"));
    assert!(
        !index
            .node_ids()
            .any(|id| id == "Shows release tasks\n  across releases")
    );
    assert!(!index.node_ids().any(|id| id == "YYYY-MM-DD"));
    assert!(!index.node_ids().any(|id| id == "open"));
    assert!(!index.node_ids().any(|id| id == "userId"));
    assert!(!index.node_ids().any(|id| id == "https://example.com/"));
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "Demo" && item.detail.as_deref() == Some("gantt section"))
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Roadmap")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Roadmap chart")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Shows release tasks")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Shows release tasks\n  across releases")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "YYYY-MM-DD")
    );
    assert!(!index.outline_items().iter().any(|item| item.name == "open"));
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "userId")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "https://example.com/")
    );
    assert!(index.has_directive_prefix("title"));
    assert!(index.has_directive_prefix("accTitle"));
    assert!(index.has_directive_prefix("accDescr"));
    assert!(index.has_directive_prefix("dateFormat"));
    assert!(index.has_directive_prefix("section"));
    assert!(index.has_directive_prefix("click"));
}

#[test]
fn incomplete_gantt_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "gantt\ndateFormat YYYY-MM-DD\nTask 1: id1,2014-01-01,1d\nTask 2".to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "id1"));
    assert!(!index.node_ids().any(|id| id == "Task"));
}

#[test]
fn mindmap_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "mindmap\n",
            "root(Root Node)\n",
            " child1(Child 1)\n",
            " :::hot\n",
            " ::icon(bomb)\n",
            " child2\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "root"));
    assert!(index.node_ids().any(|id| id == "child1"));
    assert!(index.node_ids().any(|id| id == "child2"));
    assert!(!index.node_ids().any(|id| id == "hot"));
    assert!(!index.node_ids().any(|id| id == "bomb"));
    assert!(!index.outline_items().iter().any(|item| item.name == "hot"));
    assert!(!index.outline_items().iter().any(|item| item.name == "bomb"));
}

#[test]
fn incomplete_mindmap_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "mindmap\nroot\n child[unterminated".to_string());
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "root"));
    assert!(!index.node_ids().any(|id| id == "child"));
}

fn estimated_entry_weight(uri: &Uri, text: &str, kind: DocumentKind) -> usize {
    let mut sizing = DocumentStore::new();
    sizing.upsert_text(uri.clone(), 1, text.to_string(), kind);
    let request = sizing
        .snapshot_build_request(uri)
        .expect("sizing document should require analysis");
    let analysis = request.build().expect("sizing analysis should be accepted");
    DocumentStore::estimated_analysis_cache_entry_weight(uri, &analysis)
}

fn assert_default_cache_admits(uri: Uri, text: String, kind: DocumentKind) {
    let mut store = DocumentStore::new();
    store.upsert_text(uri.clone(), 1, text, kind);
    let request = store.snapshot_build_request(&uri).unwrap();
    let analysis = request.build().unwrap();
    let weight = DocumentStore::estimated_analysis_cache_entry_weight(&uri, &analysis);
    assert!(weight <= DEFAULT_LSP_ANALYSIS_CACHE_BUDGET_BYTES);
    assert!(store.insert_built_analysis(&request, analysis).is_some());
    assert_eq!(store.analysis_cache_len(), 1);
    assert_eq!(store.analysis_cache_total_weight(), weight);
    let statistics = store.analysis_cache_statistics();
    assert_eq!(statistics.current_weight, weight);
    assert_eq!(statistics.high_water_weight, weight);
    assert_eq!(statistics.oversized_entries, 0);
}

#[test]
fn weighted_analysis_cache_touches_and_evicts_complete_generations() {
    let a = Uri::from_str("file:///tmp/a.mmd").unwrap();
    let b = Uri::from_str("file:///tmp/b.mmd").unwrap();
    let c = Uri::from_str("file:///tmp/c.mmd").unwrap();
    let text = "flowchart TD\nA-->B\n";
    let entry_weight = estimated_entry_weight(&a, text, DocumentKind::Diagram);
    let budget = entry_weight.checked_mul(2).expect("test cache budget fits");
    let mut store = DocumentStore::with_analysis_cache_budget(budget);

    for uri in [&a, &b] {
        store.upsert_text(uri.clone(), 1, text.to_string(), DocumentKind::Diagram);
        store
            .snapshot(uri)
            .expect("entry should fit the test cache");
    }
    assert_eq!(store.analysis_cache_len(), 2);
    assert_eq!(store.analysis_cache_total_weight(), budget);
    store
        .snapshot(&a)
        .expect("touch should return the cached entry");

    store.upsert_text(c.clone(), 1, text.to_string(), DocumentKind::Diagram);
    store
        .snapshot(&c)
        .expect("third analysis remains request-local");

    assert!(store.has_snapshot(&a));
    assert!(!store.has_snapshot(&b));
    assert!(store.has_snapshot(&c));
    assert_eq!(store.analysis_cache_len(), 2);
    assert_eq!(store.analysis_cache_total_weight(), budget);
    assert_eq!(store.analysis_cache_statistics().evictions, 1);
}

#[test]
fn oversized_analysis_is_returned_current_without_disturbing_cache_invariants() {
    let uri = Uri::from_str("file:///tmp/a.mmd").unwrap();
    let text = "flowchart TD\nA-->B\n";
    let entry_weight = estimated_entry_weight(&uri, text, DocumentKind::Diagram);
    let mut store = DocumentStore::with_analysis_cache_budget(entry_weight - 1);
    store.upsert_text(uri.clone(), 1, text.to_string(), DocumentKind::Diagram);
    let request = store
        .snapshot_build_request(&uri)
        .expect("uncached document should require analysis");
    let analysis = request.build().expect("analysis should succeed");
    let weak = Arc::downgrade(&analysis);

    let context = store
        .insert_built_analysis(&request, Arc::clone(&analysis))
        .expect("oversized current analysis remains request-local");
    drop(analysis);

    assert!(store.is_analysis_context_current(&context));
    assert_eq!(store.analysis_cache_len(), 0);
    assert_eq!(store.analysis_cache_total_weight(), 0);
    let statistics = store.analysis_cache_statistics();
    assert_eq!(statistics.oversized_entries, 1);
    assert_eq!(statistics.current_weight, 0);
    assert_eq!(statistics.high_water_weight, 0);
    assert!(store.get(&uri).is_some());
    assert!(weak.upgrade().is_none());
}

#[test]
fn cancelled_and_stale_builds_do_not_change_cache_weight_or_eviction_order() {
    let uri = Uri::from_str("file:///tmp/a.mmd").unwrap();
    let mut store = DocumentStore::new();
    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let request = store.snapshot_build_request(&uri).unwrap();
    let before = (
        store.analysis_cache_total_weight(),
        store.analysis_cache_len(),
        store.analysis_cache_statistics().evictions,
    );
    let cancellation = AnalysisCancellationToken::new();
    cancellation.cancel();
    assert!(request.build_cancellable(&cancellation).is_err());
    assert_eq!(
        (
            store.analysis_cache_total_weight(),
            store.analysis_cache_len(),
            store.analysis_cache_statistics().evictions,
        ),
        before
    );

    let stale_analysis = request.build().unwrap();
    store.upsert_text(
        uri.clone(),
        2,
        "flowchart TD\nA-->C\n".to_string(),
        DocumentKind::Diagram,
    );
    assert!(
        store
            .insert_built_analysis(&request, stale_analysis)
            .is_none()
    );
    assert_eq!(
        (
            store.analysis_cache_total_weight(),
            store.analysis_cache_len(),
            store.analysis_cache_statistics().evictions,
        ),
        before
    );
}

#[test]
fn eviction_releases_cache_ownership_but_preserves_request_local_generation() {
    let a = Uri::from_str("file:///tmp/a.mmd").unwrap();
    let b = Uri::from_str("file:///tmp/b.mmd").unwrap();
    let text = "flowchart TD\nA-->B\n";
    let entry_weight = estimated_entry_weight(&a, text, DocumentKind::Diagram);
    let mut store = DocumentStore::with_analysis_cache_budget(entry_weight);
    store.upsert_text(a.clone(), 1, text.to_string(), DocumentKind::Diagram);
    let request = store.snapshot_build_request(&a).unwrap();
    let analysis = request.build().unwrap();
    let weak_context = Arc::downgrade(&analysis);
    let weak_generation = Arc::downgrade(analysis.generation());
    let weak_payload = Arc::downgrade(&analysis.payload);
    let request_local = store
        .insert_built_analysis(&request, Arc::clone(&analysis))
        .unwrap();
    drop(analysis);

    store.upsert_text(b.clone(), 1, text.to_string(), DocumentKind::Diagram);
    store.snapshot(&b).unwrap();

    assert!(weak_context.upgrade().is_none());
    assert!(weak_generation.upgrade().is_some());
    assert!(weak_payload.upgrade().is_some());
    assert!(store.is_analysis_context_current(&request_local));
    drop(request_local);
    assert!(weak_generation.upgrade().is_none());
    assert!(weak_payload.upgrade().is_none());
}

#[test]
fn eviction_keeps_open_document_diagnostic_and_token_identity() {
    let a = Uri::from_str("file:///tmp/a.mmd").unwrap();
    let b = Uri::from_str("file:///tmp/b.mmd").unwrap();
    let text = "flowchart TD\nA-->B\n";
    let entry_weight = estimated_entry_weight(&a, text, DocumentKind::Diagram);
    let mut store = DocumentStore::with_analysis_cache_budget(entry_weight);
    store.upsert_text(a.clone(), 1, text.to_string(), DocumentKind::Diagram);
    let snapshot = store.snapshot_context(&a).unwrap();
    assert!(store.set_semantic_tokens_state_if_current(
        &snapshot,
        SemanticTokensState::new(Some("tokens-a".to_string()), Vec::new()),
    ));
    let diagnostic = store
        .diagnostic_contexts()
        .into_iter()
        .find(|context| context.document.uri == a)
        .unwrap();
    assert!(store.set_diagnostic_state_if_current(
        &diagnostic,
        DocumentDiagnosticState {
            result_id: "diagnostics-a".to_string(),
            diagnostics: Vec::new(),
        },
    ));

    store.upsert_text(b.clone(), 1, text.to_string(), DocumentKind::Diagram);
    store.snapshot(&b).unwrap();

    assert!(!store.has_snapshot(&a));
    assert!(store.get(&a).is_some());
    assert_eq!(
        store.diagnostic_state(&a).unwrap().result_id,
        "diagnostics-a"
    );
    assert_eq!(
        store
            .semantic_tokens_state(&a)
            .and_then(|state| state.result_id.as_deref()),
        Some("tokens-a")
    );
}

#[test]
fn larger_diagnostic_reprojection_can_be_current_without_being_cached() {
    let uri = Uri::from_str("file:///tmp/a.mmd").unwrap();
    let text = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
    let entry_weight = estimated_entry_weight(&uri, text, DocumentKind::Diagram);
    let mut store = DocumentStore::with_analysis_cache_budget(entry_weight);
    store.upsert_text(uri.clone(), 1, text.to_string(), DocumentKind::Diagram);
    store.snapshot_context(&uri).unwrap();
    let (_, plan) = store.begin_analyzer_options(default_lsp_analysis_options().with_rule_config(
        AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended),
    ));
    let batch = plan
        .expect("recommended rules should reproject the cached generation")
        .project()
        .unwrap();

    let context = store
        .commit_diagnostic_reprojection_context(&uri, batch)
        .expect("valid projected context must be returned even when oversized");

    assert!(store.is_analysis_context_current(&context));
    assert!(!context.analysis_payload().diagnostics.is_empty());
    assert_eq!(store.analysis_cache_len(), 0);
    assert_eq!(store.analysis_cache_total_weight(), 0);
    assert!(store.get(&uri).is_some());
}

#[test]
fn default_cache_admits_representative_source_limit_boundary_documents() {
    let flow_uri = Uri::from_str("file:///tmp/boundary.mmd").unwrap();
    let flow_prefix = "flowchart TD\nA-->B\n%% ";
    let flow = format!(
        "{flow_prefix}{}",
        "x".repeat(DEFAULT_LSP_MAX_SOURCE_BYTES - flow_prefix.len())
    );
    assert_default_cache_admits(flow_uri, flow, DocumentKind::Diagram);

    let markdown_uri = Uri::from_str("file:///tmp/boundary.md").unwrap();
    let markdown_suffix = "\n```mermaid\nflowchart TD\nA-->B\n```\n";
    let markdown = format!(
        "{}{}",
        "x".repeat(DEFAULT_LSP_MAX_SOURCE_BYTES - markdown_suffix.len()),
        markdown_suffix
    );
    assert_default_cache_admits(markdown_uri, markdown, DocumentKind::Markdown);

    let high_fact_uri = Uri::from_str("file:///tmp/high-fact.mmd").unwrap();
    let mut high_fact = (0..4096)
        .map(|index| format!("N{index}[Node {index}] --> N{}\n", index + 1))
        .fold("flowchart TD\n".to_string(), |mut source, line| {
            source.push_str(&line);
            source
        });
    high_fact.push_str("%% ");
    high_fact.extend(std::iter::repeat_n(
        'x',
        DEFAULT_LSP_MAX_SOURCE_BYTES - high_fact.len(),
    ));
    assert_eq!(high_fact.len(), DEFAULT_LSP_MAX_SOURCE_BYTES);
    assert_default_cache_admits(high_fact_uri, high_fact, DocumentKind::Diagram);
}

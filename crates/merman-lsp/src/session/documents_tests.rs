use std::str::FromStr;
use std::sync::Arc;

use super::{
    AnalyzerOptionsPreparation, ConfigurationUpdateOutcome, DEFAULT_LSP_MAX_DOCUMENT_DIAGRAMS,
    DEFAULT_LSP_MAX_SOURCE_BYTES, DocumentDiagnosticState, DocumentSource, DocumentSyncLoss,
    DocumentUnavailableDiagnostic, SessionState, default_lsp_analysis_options,
};
use merman_analysis::{
    AnalysisCancellationToken, AnalysisConfigChange, AnalysisRuleConfig, DiagnosticSeverity,
};
use merman_editor_core::DocumentKind;
use tower_lsp_server::ls_types::{Position, Range, TextDocumentContentChangeEvent, Uri};

const DIAGRAM_SOURCE: &str = "flowchart TD\nA-->B\n";

fn new_session_state() -> SessionState {
    SessionState::with_session_cancellation(AnalysisCancellationToken::new())
}

fn diagram_uri(name: &str) -> Uri {
    Uri::from_str(&format!("file:///tmp/{name}.mmd")).expect("test URI must be valid")
}

#[test]
fn new_store_uses_lsp_default_resource_limits() {
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

#[tokio::test(flavor = "current_thread")]
async fn document_source_transitions_follow_language_session_behavior() {
    let session = super::LanguageSession::with_cancellation(AnalysisCancellationToken::new());
    let uri = diagram_uri("document-source-transitions");

    assert!(
        session
            .open_document(
                uri.clone(),
                1,
                DIAGRAM_SOURCE.to_string(),
                DocumentKind::Diagram,
            )
            .await
    );
    let available = session
        .diagnostic_context(&uri)
        .await
        .expect("available source should be stored");
    assert!(!available.document.is_analysis_unavailable());
    assert_eq!(
        available.document.retained_text().unwrap().as_ref(),
        DIAGRAM_SOURCE
    );
    assert!(available.document.unavailable_diagnostic().is_none());

    let source_limit = 8;
    assert!(
        session
            .update_configuration(
                default_lsp_analysis_options().with_max_source_bytes(Some(source_limit)),
            )
            .await
            .affects_snapshots()
    );
    let limited = session
        .diagnostic_context(&uri)
        .await
        .expect("limited source should retain document metadata");
    assert!(limited.document.retained_text().is_none());
    assert!(matches!(
        limited.document.unavailable_diagnostic(),
        Some(DocumentUnavailableDiagnostic::ResourceLimited {
            source_len,
            max_source_bytes,
            ..
        }) if source_len == DIAGRAM_SOURCE.len() && max_source_bytes == source_limit
    ));

    assert!(
        session
            .update_configuration(default_lsp_analysis_options())
            .await
            .affects_snapshots()
    );
    let discarded = session
        .diagnostic_context(&uri)
        .await
        .expect("permissive configuration cannot recover discarded text");
    assert!(matches!(
        discarded.document.unavailable_diagnostic(),
        Some(DocumentUnavailableDiagnostic::Discarded {
            source_len,
            previous_max_source_bytes,
            ..
        }) if source_len == DIAGRAM_SOURCE.len() && previous_max_source_bytes == source_limit
    ));

    assert_eq!(
        session
            .change_document(
                uri.clone(),
                2,
                vec![TextDocumentContentChangeEvent {
                    range: Some(Range::new(Position::new(1, 0), Position::new(1, 1))),
                    range_length: None,
                    text: "C".to_string(),
                }],
            )
            .await,
        Some(true)
    );
    let sync_lost = session
        .diagnostic_context(&uri)
        .await
        .expect("ranged edit should preserve lightweight document state");
    assert!(matches!(
        sync_lost.document.unavailable_diagnostic(),
        Some(DocumentUnavailableDiagnostic::SyncLost(
            DocumentSyncLoss::SourceUnavailable {
                source_len,
                last_max_source_bytes,
            }
        )) if source_len == DIAGRAM_SOURCE.len() && last_max_source_bytes == source_limit
    ));

    assert_eq!(
        session
            .change_document(
                uri.clone(),
                3,
                vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: DIAGRAM_SOURCE.to_string(),
                }],
            )
            .await,
        Some(true)
    );
    let recovered = session
        .diagnostic_context(&uri)
        .await
        .expect("full replacement should recover source");
    assert!(!recovered.document.is_analysis_unavailable());
    assert_eq!(recovered.document.version, 3);
    assert_eq!(
        recovered.document.retained_text().unwrap().as_ref(),
        DIAGRAM_SOURCE
    );
}

#[tokio::test(flavor = "current_thread")]
async fn retained_analysis_rejection_allows_ranged_recovery_and_reclassification() {
    let session = super::LanguageSession::with_cancellation(AnalysisCancellationToken::new());
    let uri = Uri::from_str("file:///tmp/markdown-limit.md").unwrap();
    let source = concat!(
        "```mermaid\nflowchart TD\nA-->B\n```\n",
        "```mermaid\nsequenceDiagram\nA->>B: hi\n```\n",
    );

    assert!(
        session
            .update_configuration(
                default_lsp_analysis_options().with_max_document_diagrams(Some(1)),
            )
            .await
            .affects_snapshots()
    );
    assert!(
        session
            .open_document(uri.clone(), 1, source.to_string(), DocumentKind::Markdown,)
            .await
    );
    let rejected = session
        .diagnostic_context(&uri)
        .await
        .expect("analysis rejection should retain the document");
    assert_eq!(rejected.document.retained_text().unwrap().as_ref(), source);
    assert!(matches!(
        rejected.document.unavailable_diagnostic(),
        Some(DocumentUnavailableDiagnostic::AnalysisRejected(_))
    ));

    assert_eq!(
        session
            .change_document(
                uri.clone(),
                2,
                vec![TextDocumentContentChangeEvent {
                    range: Some(Range::new(Position::new(4, 3), Position::new(4, 10))),
                    range_length: None,
                    text: "text".to_string(),
                }],
            )
            .await,
        Some(true)
    );
    let recovered = session
        .diagnostic_context(&uri)
        .await
        .expect("ranged edit should recover retained text");
    assert!(!recovered.document.is_analysis_unavailable());

    assert_eq!(
        session
            .change_document(
                uri.clone(),
                3,
                vec![TextDocumentContentChangeEvent {
                    range: Some(Range::new(Position::new(4, 3), Position::new(4, 7))),
                    range_length: None,
                    text: "mermaid".to_string(),
                }],
            )
            .await,
        Some(true)
    );
    assert!(matches!(
        session
            .diagnostic_context(&uri)
            .await
            .expect("second fence should be rejected again")
            .document
            .unavailable_diagnostic(),
        Some(DocumentUnavailableDiagnostic::AnalysisRejected(_))
    ));

    assert!(
        session
            .update_configuration(
                default_lsp_analysis_options().with_max_document_diagrams(Some(2)),
            )
            .await
            .affects_snapshots()
    );
    assert!(
        !session
            .diagnostic_context(&uri)
            .await
            .expect("configuration should reclassify retained text")
            .document
            .is_analysis_unavailable()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn source_limit_reclassification_cancellation_is_observed_at_the_session_boundary() {
    let session = super::LanguageSession::with_cancellation(AnalysisCancellationToken::new());
    let uri = diagram_uri("cancelled-reclassification");
    assert!(
        session
            .open_document(
                uri.clone(),
                1,
                DIAGRAM_SOURCE.to_string(),
                DocumentKind::Diagram,
            )
            .await
    );

    session.terminate();
    assert_eq!(
        session
            .update_configuration(default_lsp_analysis_options().with_max_source_bytes(Some(8)),)
            .await,
        super::ConfigurationUpdateOutcome::Cancelled
    );
}

#[tokio::test(flavor = "current_thread")]
async fn change_contract_covers_stale_empty_invalid_and_later_full_replacement() {
    let session = super::LanguageSession::with_cancellation(AnalysisCancellationToken::new());
    let uri = diagram_uri("change-contract");

    assert!(
        session
            .open_document(
                uri.clone(),
                3,
                "flowchart TD\nA[🤓]-->B\n".to_string(),
                DocumentKind::Diagram,
            )
            .await
    );
    assert_eq!(
        session
            .change_document(
                uri.clone(),
                2,
                vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: DIAGRAM_SOURCE.to_string(),
                }],
            )
            .await,
        Some(false)
    );
    assert_eq!(
        session.change_document(uri.clone(), 4, Vec::new()).await,
        Some(false)
    );

    assert_eq!(
        session
            .change_document(
                uri.clone(),
                5,
                vec![
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
            )
            .await,
        Some(true)
    );
    assert_eq!(
        session
            .diagnostic_context(&uri)
            .await
            .expect("valid UTF-16 ranges should update the source")
            .document
            .retained_text()
            .unwrap()
            .as_ref(),
        "flowchart TD\nA[C]-->B\nC-->D\n"
    );

    assert_eq!(
        session
            .change_document(
                uri.clone(),
                6,
                vec![TextDocumentContentChangeEvent {
                    range: Some(Range::new(Position::new(20, 0), Position::new(20, 1))),
                    range_length: None,
                    text: "bad".to_string(),
                }],
            )
            .await,
        Some(true)
    );
    assert!(matches!(
        session
            .diagnostic_context(&uri)
            .await
            .expect("invalid edit should preserve sync-loss state")
            .document
            .unavailable_diagnostic(),
        Some(DocumentUnavailableDiagnostic::SyncLost(
            DocumentSyncLoss::InvalidIncrementalRange
        ))
    ));

    assert_eq!(
        session
            .change_document(
                uri.clone(),
                8,
                vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: DIAGRAM_SOURCE.to_string(),
                }],
            )
            .await,
        Some(true)
    );
    assert_eq!(
        session
            .change_document(
                uri.clone(),
                9,
                vec![TextDocumentContentChangeEvent {
                    range: Some(Range::new(Position::new(1, 4), Position::new(1, 2))),
                    range_length: None,
                    text: "bad".to_string(),
                }],
            )
            .await,
        Some(true)
    );
    assert!(matches!(
        session
            .diagnostic_context(&uri)
            .await
            .expect("reversed edit should preserve sync-loss state")
            .document
            .unavailable_diagnostic(),
        Some(DocumentUnavailableDiagnostic::SyncLost(
            DocumentSyncLoss::InvalidIncrementalRange
        ))
    ));

    assert_eq!(
        session
            .change_document(
                uri.clone(),
                10,
                vec![
                    TextDocumentContentChangeEvent {
                        range: Some(Range::new(Position::new(30, 0), Position::new(30, 1))),
                        range_length: None,
                        text: "ignored".to_string(),
                    },
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
            )
            .await,
        Some(true)
    );
    let recovered = session
        .diagnostic_context(&uri)
        .await
        .expect("later full replacement should recover the document");
    assert_eq!(recovered.document.version, 10);
    assert_eq!(
        recovered.document.retained_text().unwrap().as_ref(),
        "sequenceDiagram\nAlice->>Bob: Hello\n"
    );
}

#[test]
fn prepared_change_cannot_overwrite_a_newer_document_epoch() {
    let mut state = new_session_state();
    let uri = diagram_uri("prepared-change-cas");
    state.open_document_source(
        uri.clone(),
        1,
        DocumentSource::Available(Arc::from(DIAGRAM_SOURCE)),
        DocumentKind::Diagram,
    );
    let plan = state
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

    state.open_document_source(
        uri.clone(),
        3,
        DocumentSource::Available(Arc::from("sequenceDiagram\nAlice->>Bob: newer\n")),
        DocumentKind::Diagram,
    );

    assert!(
        !state.commit_prepared_document_change(
            plan.prepare()
                .expect("test text preparation should not be cancelled"),
        )
    );
    let stored = state
        .get(&uri)
        .expect("newer document should remain stored");
    assert_eq!(stored.version, 3);
    assert!(
        stored
            .retained_text()
            .unwrap()
            .starts_with("sequenceDiagram")
    );
}

#[test]
fn session_cancellation_aborts_pending_text_change_preparation() {
    let cancellation = AnalysisCancellationToken::new();
    let mut state = SessionState::with_session_cancellation(cancellation.clone());
    let uri = diagram_uri("cancelled-change");
    state.open_document_source(
        uri.clone(),
        1,
        DocumentSource::Available(Arc::from(DIAGRAM_SOURCE)),
        DocumentKind::Diagram,
    );
    let plan = state
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
fn prepared_source_reclassification_rejects_stale_document_and_configuration() {
    let mut state = new_session_state();
    let uri = diagram_uri("reclassified");
    state.open_document_source(
        uri.clone(),
        1,
        DocumentSource::Available(Arc::from(DIAGRAM_SOURCE)),
        DocumentKind::Diagram,
    );

    let request = state.begin_analyzer_configuration_request();
    let AnalyzerOptionsPreparation::RequiresSnapshotPreparation(plan) = state
        .prepare_analyzer_options(
            request,
            default_lsp_analysis_options().with_max_source_bytes(Some(8)),
        )
        .expect("current configuration should be classified")
    else {
        panic!("lower source limit must require source projection");
    };
    let batch = plan.prepare().expect("test projection should succeed");
    state.open_document_source(
        uri.clone(),
        2,
        DocumentSource::Available(Arc::from("flowchart TD\nC-->D\n")),
        DocumentKind::Diagram,
    );
    assert!(state.commit_snapshot_configuration(batch).is_none());
    assert_eq!(state.get(&uri).unwrap().version, 2);

    let older_request = state.begin_analyzer_configuration_request();
    let AnalyzerOptionsPreparation::RequiresSnapshotPreparation(older_plan) = state
        .prepare_analyzer_options(
            older_request,
            default_lsp_analysis_options().with_max_source_bytes(Some(8)),
        )
        .expect("first request should be current")
    else {
        panic!("lower source limit must require source projection");
    };
    let older_batch = older_plan
        .prepare()
        .expect("test projection should succeed");
    let latest_request = state.begin_analyzer_configuration_request();
    assert!(matches!(
        state.prepare_analyzer_options(latest_request, default_lsp_analysis_options()),
        Some(AnalyzerOptionsPreparation::Applied(
            AnalysisConfigChange::Unchanged
        ))
    ));
    assert!(!state.is_analyzer_configuration_request_current(older_request));
    assert!(state.commit_snapshot_configuration(older_batch).is_none());
}

#[test]
fn diagnostic_state_is_bound_to_the_document_epoch() {
    let mut state = new_session_state();
    let uri = diagram_uri("diagnostic-state");
    state.open_document_source(
        uri.clone(),
        1,
        DocumentSource::Available(Arc::from(DIAGRAM_SOURCE)),
        DocumentKind::Diagram,
    );
    let context = state
        .diagnostic_context(&uri)
        .expect("expected diagnostic context");
    let diagnostic_state = DocumentDiagnosticState {
        result_id: "result-1".to_string(),
        diagnostics: Vec::new(),
    };

    assert!(state.set_diagnostic_state_if_current(&context, diagnostic_state.clone()));
    assert_eq!(
        state
            .diagnostic_state(&uri)
            .expect("expected cached diagnostics")
            .result_id,
        "result-1"
    );

    state.open_document_source(
        uri.clone(),
        2,
        DocumentSource::Available(Arc::from("flowchart TD\nA-->C\n")),
        DocumentKind::Diagram,
    );
    assert!(state.diagnostic_state(&uri).is_none());
    assert!(!state.set_diagnostic_state_if_current(&context, diagnostic_state));
}

#[test]
fn no_op_configuration_does_not_supersede_prepared_text_changes() {
    let mut state = new_session_state();
    let uri = diagram_uri("no-op-config-text-ticket");
    state.open_document_source(
        uri.clone(),
        1,
        DocumentSource::Available(Arc::from(DIAGRAM_SOURCE)),
        DocumentKind::Diagram,
    );
    let prepared = state
        .capture_text_changes(
            uri.clone(),
            2,
            [TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "flowchart TD\nA-->C\n".to_string(),
            }],
        )
        .expect("expected a prepared text transaction")
        .prepare()
        .expect("text preparation should succeed");

    let request = state.begin_analyzer_configuration_request();
    assert!(matches!(
        state.prepare_analyzer_options(request, state.analyzer_options().clone()),
        Some(AnalyzerOptionsPreparation::Applied(
            AnalysisConfigChange::Unchanged
        ))
    ));
    assert!(state.commit_prepared_document_change(prepared));
    assert_eq!(state.get(&uri).unwrap().version, 2);
}

#[test]
fn applied_configuration_supersedes_prepared_text_changes() {
    let mut state = new_session_state();
    let uri = diagram_uri("applied-config-text-ticket");
    state.open_document_source(
        uri.clone(),
        1,
        DocumentSource::Available(Arc::from(DIAGRAM_SOURCE)),
        DocumentKind::Diagram,
    );
    let prepared = state
        .capture_text_changes(
            uri.clone(),
            2,
            [TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "flowchart TD\nA-->C\n".to_string(),
            }],
        )
        .expect("expected a prepared text transaction")
        .prepare()
        .expect("text preparation should succeed");

    let request = state.begin_analyzer_configuration_request();
    assert!(matches!(
        state.prepare_analyzer_options(
            request,
            default_lsp_analysis_options().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                    .unwrap(),
            ),
        ),
        Some(AnalyzerOptionsPreparation::Applied(_))
    ));
    assert!(!state.commit_prepared_document_change(prepared));
    assert_eq!(state.get(&uri).unwrap().version, 1);
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
fn open_commit_is_uri_local_and_rejects_changed_target_or_configuration_state() {
    let mut state = new_session_state();
    let uri = diagram_uri("open-ticket");
    let other = diagram_uri("other");
    let ticket = state.capture_open_document(uri.clone());
    state.open_document_source(
        other,
        1,
        DocumentSource::Available(Arc::from(DIAGRAM_SOURCE)),
        DocumentKind::Diagram,
    );
    assert!(state.commit_open_document(
        ticket,
        1,
        DocumentSource::Available(Arc::from(DIAGRAM_SOURCE)),
        DocumentKind::Diagram,
    ));

    let ticket = state.capture_open_document(uri.clone());
    state.open_document_source(
        uri.clone(),
        2,
        DocumentSource::Available(Arc::from("flowchart TD\nA-->C\n")),
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
    let request = state.begin_analyzer_configuration_request();
    assert!(matches!(
        state.prepare_analyzer_options(
            request,
            default_lsp_analysis_options().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                    .unwrap(),
            ),
        ),
        Some(AnalyzerOptionsPreparation::Applied(_))
    ));
    assert!(!state.commit_open_document(
        ticket,
        3,
        DocumentSource::Available(Arc::from("flowchart TD\nA-->D\n")),
        DocumentKind::Diagram,
    ));
}

#[test]
fn open_commit_rejects_absent_present_absent_aba() {
    let mut state = new_session_state();
    let uri = diagram_uri("open-ticket-aba");
    let ticket = state.capture_open_document(uri.clone());

    state.open_document_source(
        uri.clone(),
        1,
        DocumentSource::Available(Arc::from(DIAGRAM_SOURCE)),
        DocumentKind::Diagram,
    );
    state.remove(&uri);

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
    let uri = diagram_uri("open-ticket-overlap");
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
        Some(1)
    );
    assert!(state.commit_open_document(
        second,
        1,
        DocumentSource::Available(Arc::from(DIAGRAM_SOURCE)),
        DocumentKind::Diagram,
    ));
    assert!(crate::sync::lock_recovering_poison(&state.open_document_tracker.entries).is_empty());
}

use super::LanguageSession;
use super::analysis::executor::LSP_ANALYSIS_CONCURRENCY;
use super::analysis::request::TestAnalysisGate;
use super::documents::{SemanticTokensState, default_lsp_analysis_options};
use crate::client_profile::ClientProtocolProfile;
use crate::snapshot::{AnalysisResultIdentity, SnapshotContext};
use merman_analysis::{AnalysisRuleConfig, DiagnosticSeverity};
use merman_editor_core::DocumentKind;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::task::Poll;
use std::time::Duration;
use tower_lsp_server::jsonrpc::ErrorCode;
use tower_lsp_server::ls_types::{
    DiagnosticSeverity as LspDiagnosticSeverity, NumberOrString, Position, Range,
    TextDocumentContentChangeEvent, Uri,
};

fn session() -> LanguageSession {
    LanguageSession::with_cancellation(merman_analysis::AnalysisCancellationToken::new())
}

fn uri(name: &str) -> Uri {
    Uri::from_str(&format!("file:///tmp/{name}.mmd")).unwrap()
}

fn projected_severity(context: &SnapshotContext, code: &str) -> LspDiagnosticSeverity {
    context
        .diagnostic_round_trip()
        .diagnostics_with_profile(&ClientProtocolProfile::permissive())
        .into_iter()
        .find(|diagnostic| {
            diagnostic.code.as_ref() == Some(&NumberOrString::String(code.to_owned()))
        })
        .and_then(|diagnostic| diagnostic.severity)
        .expect("expected projected diagnostic severity")
}

async fn open(session: &LanguageSession, uri: &Uri, version: i32, text: &str) {
    assert!(
        session
            .open_document(uri.clone(), version, text.to_owned(), DocumentKind::Diagram,)
            .await
    );
}

async fn structure_identity(
    session: &LanguageSession,
    uri: &Uri,
) -> Option<AnalysisResultIdentity> {
    session
        .query_structure(uri, |snapshot| {
            Ok(Some(snapshot.analysis_result_identity()))
        })
        .await
        .expect("test structure query should not fail")
}

async fn wait_for_gate(gate: &TestAnalysisGate, expected: usize) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while gate.started() != expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("analysis did not reach its deterministic gate");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edit_during_structure_analysis_rejects_the_old_commit() {
    let session = session();
    let uri = uri("stale-structure");
    open(&session, &uri, 1, "flowchart TD\nA-->B\n").await;
    let gate = Arc::new(TestAnalysisGate::default());
    session
        .inner
        .state
        .lock()
        .await
        .set_analysis_test_gate(Some(Arc::clone(&gate)));

    let query = tokio::spawn({
        let session = session.clone();
        let uri = uri.clone();
        async move { session.query_structure(&uri, |_| Ok(Some(()))).await }
    });
    wait_for_gate(&gate, 1).await;
    session
        .change_document(
            uri,
            2,
            vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "flowchart TD\nA-->C\n".to_owned(),
            }],
        )
        .await;
    gate.release();

    let error = query.await.unwrap().unwrap_err();
    assert_eq!(error.code, ErrorCode::ContentModified);
    assert!(error.message.contains("structure document changed"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn semantic_token_error_from_stale_snapshot_becomes_content_modified() {
    let session = session();
    let uri = uri("stale-semantic-error");
    open(&session, &uri, 1, "flowchart TD\nA-->B\n").await;
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));

    let query = tokio::spawn({
        let session = session.clone();
        let uri = uri.clone();
        let entered = Arc::clone(&entered);
        let release = Arc::clone(&release);
        async move {
            session
                .query_semantic_tokens::<()>(&uri, None, move |_, _, _| {
                    entered.wait();
                    release.wait();
                    Err(tower_lsp_server::jsonrpc::Error::invalid_params(
                        "old snapshot planner error",
                    ))
                })
                .await
        }
    });
    entered.wait();
    session
        .change_document(
            uri,
            2,
            vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "flowchart TD\nA-->C\n".to_owned(),
            }],
        )
        .await;
    release.wait();

    let error = query.await.unwrap().unwrap_err();
    assert_eq!(error.code, ErrorCode::ContentModified);
    assert!(error.message.contains("semantic tokens document changed"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn empty_semantic_tokens_from_stale_snapshot_become_content_modified() {
    let session = session();
    let uri = uri("stale-semantic-none");
    open(&session, &uri, 1, "flowchart TD\nA-->B\n").await;
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));

    let query = tokio::spawn({
        let session = session.clone();
        let uri = uri.clone();
        let entered = Arc::clone(&entered);
        let release = Arc::clone(&release);
        async move {
            session
                .query_semantic_tokens::<()>(&uri, None, move |_, _, _| {
                    entered.wait();
                    release.wait();
                    Ok(None)
                })
                .await
        }
    });
    entered.wait();
    session
        .change_document(
            uri,
            2,
            vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "flowchart TD\nA-->C\n".to_owned(),
            }],
        )
        .await;
    release.wait();

    let error = query.await.unwrap().unwrap_err();
    assert_eq!(error.code, ErrorCode::ContentModified);
    assert!(error.message.contains("semantic tokens document changed"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn termination_fences_a_ready_semantic_token_state_before_final_commit() {
    let session = session();
    let uri = uri("terminated-semantic-commit");
    open(&session, &uri, 1, "flowchart TD\nA-->B\n").await;
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));

    let query = tokio::spawn({
        let session = session.clone();
        let uri = uri.clone();
        let entered = Arc::clone(&entered);
        let release = Arc::clone(&release);
        async move {
            session
                .query_semantic_tokens(&uri, None, move |_, _, _| {
                    entered.wait();
                    release.wait();
                    Ok(Some((
                        (),
                        Some(SemanticTokensState::new(
                            "terminated".to_owned(),
                            Vec::new(),
                        )),
                    )))
                })
                .await
        }
    });
    entered.wait();

    let state = session.inner.state.lock().await;
    release.wait();
    assert!(session.terminate());
    drop(state);

    assert!(query.await.unwrap().unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pull_diagnostics_retries_an_execution_that_becomes_stale() {
    let session = session();
    let uri = uri("diagnostic-retry");
    open(&session, &uri, 1, "flowchart TD\nA-->B\n").await;
    let gate = Arc::new(TestAnalysisGate::default());
    session
        .inner
        .state
        .lock()
        .await
        .set_analysis_test_gate(Some(Arc::clone(&gate)));

    let pull = tokio::spawn({
        let session = session.clone();
        let uri = uri.clone();
        async move {
            session
                .pull_diagnostics(&uri, |context, _| {
                    super::documents::DocumentDiagnosticState {
                        result_id: context.document.version.to_string(),
                        diagnostics: Vec::new(),
                    }
                })
                .await
        }
    });
    wait_for_gate(&gate, 1).await;
    session
        .change_document(
            uri,
            2,
            vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "flowchart TD\nA-->C\n".to_owned(),
            }],
        )
        .await;
    gate.release();

    let state = pull.await.unwrap().unwrap().unwrap();
    assert_eq!(state.result_id, "2");
    assert_eq!(session.analysis_execution_count(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pull_diagnostics_recaptures_a_document_that_becomes_unavailable() {
    let session = session();
    let uri = uri("diagnostic-unavailable-retry");
    open(&session, &uri, 1, "flowchart TD\nA-->B\n").await;
    let gate = Arc::new(TestAnalysisGate::default());
    session
        .inner
        .state
        .lock()
        .await
        .set_analysis_test_gate(Some(Arc::clone(&gate)));
    let compute_calls = Arc::new(AtomicUsize::new(0));

    let pull = tokio::spawn({
        let session = session.clone();
        let uri = uri.clone();
        let compute_calls = Arc::clone(&compute_calls);
        async move {
            session
                .pull_diagnostics(&uri, move |context, analysis| {
                    compute_calls.fetch_add(1, Ordering::SeqCst);
                    assert!(context.document.is_analysis_unavailable());
                    assert!(analysis.is_none());
                    super::documents::DocumentDiagnosticState {
                        result_id: "unavailable".to_owned(),
                        diagnostics: Vec::new(),
                    }
                })
                .await
        }
    });
    wait_for_gate(&gate, 1).await;

    let change = session
        .update_configuration(default_lsp_analysis_options().with_max_source_bytes(Some(1)))
        .await;
    assert!(change.affects_snapshots());
    gate.release();

    let state = pull.await.unwrap().unwrap().unwrap();
    assert_eq!(state.result_id, "unavailable");
    assert_eq!(compute_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pull_diagnostics_stops_after_three_total_stale_attempts() {
    let session = session();
    let uri = uri("diagnostic-retry-budget");
    open(&session, &uri, 1, "flowchart TD\nA-->B\n").await;
    let attempts = Arc::new(AtomicUsize::new(0));
    let entered = Arc::new(
        (0..3)
            .map(|_| Arc::new(Barrier::new(2)))
            .collect::<Vec<_>>(),
    );
    let releases = Arc::new(
        (0..3)
            .map(|_| Arc::new(Barrier::new(2)))
            .collect::<Vec<_>>(),
    );

    let pull = tokio::spawn({
        let session = session.clone();
        let uri = uri.clone();
        let attempts = Arc::clone(&attempts);
        let entered = Arc::clone(&entered);
        let releases = Arc::clone(&releases);
        async move {
            session
                .pull_diagnostics(&uri, move |context, _| {
                    let attempt = attempts.fetch_add(1, Ordering::SeqCst);
                    entered[attempt].wait();
                    releases[attempt].wait();
                    super::documents::DocumentDiagnosticState {
                        result_id: context.document.version.to_string(),
                        diagnostics: Vec::new(),
                    }
                })
                .await
        }
    });

    for attempt in 0..3 {
        entered[attempt].wait();
        session
            .change_document(
                uri.clone(),
                i32::try_from(attempt + 2).unwrap(),
                vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: format!("flowchart TD\nA-->N{attempt}\n"),
                }],
            )
            .await;
        releases[attempt].wait();
    }

    let error = pull.await.unwrap().unwrap_err();
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
    assert_eq!(error.code, ErrorCode::ContentModified);
    assert_eq!(
        error.message,
        "diagnostic document changed repeatedly while computing"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pull_diagnostics_starts_at_most_three_stale_reprojections() {
    let session = session();
    let uri = uri("diagnostic-reprojection-budget");
    open(&session, &uri, 1, "").await;
    structure_identity(&session, &uri)
        .await
        .expect("snapshot-only query should populate parse evidence");
    assert!(
        session
            .update_configuration(
                default_lsp_analysis_options().with_rule_config(
                    AnalysisRuleConfig::default()
                        .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                        .unwrap(),
                )
            )
            .await
            .affects_diagnostics()
    );

    let gates = (0..4)
        .map(|_| Arc::new(TestAnalysisGate::default()))
        .collect::<Vec<_>>();
    session
        .inner
        .state
        .lock()
        .await
        .set_analysis_test_gate(Some(Arc::clone(&gates[0])));
    let compute_calls = Arc::new(AtomicUsize::new(0));
    let pull = tokio::spawn({
        let session = session.clone();
        let uri = uri.clone();
        let compute_calls = Arc::clone(&compute_calls);
        async move {
            session
                .pull_diagnostics(&uri, move |context, _| {
                    compute_calls.fetch_add(1, Ordering::SeqCst);
                    super::documents::DocumentDiagnosticState {
                        result_id: context.document.version.to_string(),
                        diagnostics: Vec::new(),
                    }
                })
                .await
        }
    });

    for attempt in 0..3 {
        wait_for_gate(&gates[attempt], 1).await;
        session
            .inner
            .state
            .lock()
            .await
            .set_analysis_test_gate(Some(Arc::clone(&gates[attempt + 1])));
        let severity = if attempt.is_multiple_of(2) {
            DiagnosticSeverity::Warning
        } else {
            DiagnosticSeverity::Hint
        };
        assert!(
            session
                .update_configuration(
                    default_lsp_analysis_options().with_rule_config(
                        AnalysisRuleConfig::default()
                            .with_rule_severity("merman.parse.no_diagram", severity)
                            .unwrap(),
                    )
                )
                .await
                .affects_diagnostics()
        );
    }

    let error = pull.await.unwrap().unwrap_err();
    assert_eq!(error.code, ErrorCode::ContentModified);
    assert_eq!(session.diagnostic_reprojection_count(), 3);
    assert_eq!(gates[3].started(), 0);
    assert_eq!(compute_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn diagnostic_policy_reprojects_without_rebuilding_the_snapshot() {
    let session = session();
    let uri = uri("diagnostic-reprojection");
    open(&session, &uri, 1, "").await;
    let initial = structure_identity(&session, &uri).await.unwrap();
    let executions = session.analysis_execution_count();
    assert_eq!(
        session.diagnostic_reprojection_count(),
        0,
        "a snapshot-only structure query must not project diagnostics"
    );
    let options = default_lsp_analysis_options().with_rule_config(
        AnalysisRuleConfig::default()
            .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
            .unwrap(),
    );
    let change = session.update_configuration(options).await;
    assert!(change.affects_diagnostics());
    assert!(!change.affects_snapshots());
    let projected = structure_identity(&session, &uri).await.unwrap();
    let severity = session
        .query_code_actions(&uri, |context| {
            Ok(Some(projected_severity(context, "merman.parse.no_diagram")))
        })
        .await
        .unwrap()
        .unwrap();

    assert_eq!(initial, projected);
    assert_eq!(session.analysis_execution_count(), executions);
    assert_eq!(session.diagnostic_reprojection_count(), 1);
    assert_eq!(severity, LspDiagnosticSeverity::HINT);
}

#[tokio::test]
async fn semantic_token_queries_do_not_build_strict_analysis_snapshots() {
    let session = session();
    let uri = uri("sequential-snapshot-reuse");
    open(&session, &uri, 1, "flowchart TD\nA-->B\n").await;

    session
        .query_semantic_tokens(&uri, None, |document, _, _| {
            assert_eq!(document.version(), 1);
            Ok(Some(((), None)))
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(session.analysis_execution_count(), 0);
    assert_eq!(session.diagnostic_reprojection_count(), 0);

    assert!(structure_identity(&session, &uri).await.is_some());
    assert_eq!(session.analysis_execution_count(), 1);
    assert_eq!(session.diagnostic_reprojection_count(), 0);
}

#[tokio::test]
async fn retained_analysis_rejections_do_not_remove_syntax_state() {
    let session = session();
    let options = default_lsp_analysis_options().with_max_document_diagrams(Some(1));
    assert!(session.update_configuration(options).await.accepted());
    let uri = Uri::from_str("file:///tmp/rejected.md").unwrap();
    assert!(
        session
            .open_document(
                uri.clone(),
                1,
                "```mermaid\nflowchart TD\nA-->B\n```\n\n```mermaid\nsequenceDiagram\nA->>B: hello\n```\n"
                    .to_owned(),
                DocumentKind::Markdown,
            )
            .await
    );
    assert!(
        session
            .query_semantic_tokens(&uri, None, |_, _, _| Ok(Some(((), None))))
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(session.analysis_execution_count(), 0);
}

#[tokio::test]
async fn invalid_incremental_sync_invalidates_syntax_until_a_full_replacement() {
    let session = session();
    let uri = uri("syntax-sync-loss");
    open(&session, &uri, 1, "flowchart TD\nA-->B\n").await;

    assert_eq!(
        session
            .change_document(
                uri.clone(),
                2,
                vec![TextDocumentContentChangeEvent {
                    range: Some(Range::new(Position::new(99, 0), Position::new(99, 0))),
                    range_length: None,
                    text: "C".to_owned(),
                }],
            )
            .await,
        Some(true)
    );
    assert!(
        session
            .query_semantic_tokens(&uri, None, |_, _, _| Ok(Some(((), None))))
            .await
            .unwrap()
            .is_none()
    );

    assert_eq!(
        session
            .change_document(
                uri.clone(),
                3,
                vec![TextDocumentContentChangeEvent {
                    range: Some(Range::new(Position::new(0, 0), Position::new(0, 0))),
                    range_length: None,
                    text: "ignored".to_owned(),
                }],
            )
            .await,
        Some(true)
    );
    assert!(
        session
            .query_semantic_tokens(&uri, None, |_, _, _| Ok(Some(((), None))))
            .await
            .unwrap()
            .is_none()
    );

    assert_eq!(
        session
            .change_document(
                uri.clone(),
                4,
                vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "flowchart TD\nA-->C\n".to_owned(),
                }],
            )
            .await,
        Some(true)
    );
    assert!(
        session
            .query_semantic_tokens(&uri, None, |document, _, _| {
                assert_eq!(document.version(), 4);
                Ok(Some(((), None)))
            })
            .await
            .unwrap()
            .is_some()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn diagnostic_configuration_commits_without_waiting_for_projection_capacity() {
    let session = session();
    let target = uri("diagnostic-config-short-transaction");
    open(&session, &target, 1, "").await;
    session
        .query_code_actions(&target, |_| Ok(Some(())))
        .await
        .unwrap()
        .expect("initial code-action query should build a complete context");
    let initial_reprojections = session.diagnostic_reprojection_count();

    let gate = Arc::new(TestAnalysisGate::default());
    session
        .inner
        .state
        .lock()
        .await
        .set_analysis_test_gate(Some(Arc::clone(&gate)));
    let blockers = [
        uri("diagnostic-config-blocker-a"),
        uri("diagnostic-config-blocker-b"),
    ];
    for blocker in &blockers {
        open(&session, blocker, 1, "flowchart TD\nA-->B\n").await;
    }
    let blocker_queries = blockers
        .iter()
        .cloned()
        .map(|uri| {
            let session = session.clone();
            tokio::spawn(async move { structure_identity(&session, &uri).await })
        })
        .collect::<Vec<_>>();
    wait_for_gate(&gate, LSP_ANALYSIS_CONCURRENCY).await;
    let options = default_lsp_analysis_options().with_rule_config(
        AnalysisRuleConfig::default()
            .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
            .unwrap(),
    );
    let mut update = Box::pin(session.update_configuration(options));
    let change = match futures::poll!(&mut update) {
        Poll::Ready(change) => change,
        Poll::Pending => panic!("diagnostic-only configuration must not await a reprojection"),
    };
    assert!(change.affects_diagnostics());
    assert!(!change.affects_snapshots());
    assert_eq!(
        session.diagnostic_reprojection_count(),
        initial_reprojections,
        "configuration commit must not eagerly consume the blocked projection capacity"
    );

    let query = tokio::spawn({
        let session = session.clone();
        let target = target.clone();
        async move {
            session
                .query_code_actions(&target, |context| {
                    Ok(Some(projected_severity(context, "merman.parse.no_diagram")))
                })
                .await
        }
    });
    tokio::task::yield_now().await;
    assert!(
        !query.is_finished(),
        "the subsequent lazy reprojection should wait behind the occupied CPU permits"
    );

    gate.release();
    for blocker in blocker_queries {
        blocker
            .await
            .expect("blocker query should not panic")
            .expect("blocker snapshot should complete");
    }
    let severity = query
        .await
        .expect("lazy diagnostic query should not panic")
        .unwrap()
        .expect("lazy diagnostic query should return a result");
    assert_eq!(severity, LspDiagnosticSeverity::HINT);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn diagnostic_policy_change_during_build_reuses_the_canonical_generation() {
    let session = session();
    let uri = uri("diagnostic-update-during-build");
    open(&session, &uri, 1, "").await;
    let gate = Arc::new(TestAnalysisGate::default());
    session
        .inner
        .state
        .lock()
        .await
        .set_analysis_test_gate(Some(Arc::clone(&gate)));

    let first = tokio::spawn({
        let session = session.clone();
        let uri = uri.clone();
        async move { structure_identity(&session, &uri).await }
    });
    wait_for_gate(&gate, 1).await;

    let options = default_lsp_analysis_options().with_rule_config(
        AnalysisRuleConfig::default()
            .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
            .unwrap(),
    );
    let change = session.update_configuration(options).await;
    assert!(change.affects_diagnostics());
    assert!(!change.affects_snapshots());

    gate.release();

    let first = first.await.unwrap().expect("first structure snapshot");
    let second = structure_identity(&session, &uri)
        .await
        .expect("second structure snapshot");
    assert_eq!(first, second);
    assert_eq!(session.analysis_execution_count(), 1);

    let severity = session
        .query_code_actions(&uri, |context| {
            Ok(Some(projected_severity(context, "merman.parse.no_diagram")))
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(severity, LspDiagnosticSeverity::HINT);
    assert_eq!(session.analysis_execution_count(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn diagnostic_policy_change_reprojects_an_uncached_request_local_generation() {
    let session = LanguageSession::with_analysis_cache_budget(0);
    let uri = uri("diagnostic-update-without-cache");
    open(&session, &uri, 1, "").await;
    let gate = Arc::new(TestAnalysisGate::default());
    session
        .inner
        .state
        .lock()
        .await
        .set_analysis_test_gate(Some(Arc::clone(&gate)));

    let query = tokio::spawn({
        let session = session.clone();
        let uri = uri.clone();
        async move {
            session
                .query_code_actions(&uri, |context| {
                    Ok(Some(projected_severity(context, "merman.parse.no_diagram")))
                })
                .await
        }
    });
    wait_for_gate(&gate, 1).await;

    let options = default_lsp_analysis_options().with_rule_config(
        AnalysisRuleConfig::default()
            .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
            .unwrap(),
    );
    let change = session.update_configuration(options).await;
    assert!(change.affects_diagnostics());
    assert!(!change.affects_snapshots());
    gate.release();

    let severity = query.await.unwrap().unwrap().unwrap();
    assert_eq!(severity, LspDiagnosticSeverity::HINT);
    assert_eq!(session.analysis_execution_count(), 1);
    assert_eq!(session.diagnostic_reprojection_count(), 1);

    session
        .inner
        .state
        .lock()
        .await
        .set_analysis_test_gate(None);
    let severity = session
        .query_code_actions(&uri, |context| {
            Ok(Some(projected_severity(context, "merman.parse.no_diagram")))
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(severity, LspDiagnosticSeverity::HINT);
    assert_eq!(session.analysis_execution_count(), 2);
    assert_eq!(session.diagnostic_reprojection_count(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn snapshot_policy_change_cancels_an_older_session_query() {
    let session = session();
    let uri = uri("snapshot-policy-cancel");
    open(&session, &uri, 1, "flowchart TD\nA-->B\n").await;
    let gate = Arc::new(TestAnalysisGate::default());
    session
        .inner
        .state
        .lock()
        .await
        .set_analysis_test_gate(Some(Arc::clone(&gate)));
    let query = tokio::spawn({
        let session = session.clone();
        let uri = uri.clone();
        async move { session.query_structure(&uri, |_| Ok(Some(()))).await }
    });
    wait_for_gate(&gate, 1).await;

    let change = session
        .update_configuration(default_lsp_analysis_options().with_max_source_bytes(Some(1024)))
        .await;
    assert!(change.affects_snapshots());
    gate.release();

    let error = query.await.unwrap().unwrap_err();
    assert_eq!(error.code, ErrorCode::ContentModified);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dropping_the_last_session_waiter_does_not_admit_a_cache_entry() {
    let session = session();
    let uri = uri("last-waiter");
    open(&session, &uri, 1, "flowchart TD\nA-->B\n").await;
    let gate = Arc::new(TestAnalysisGate::default());
    session
        .inner
        .state
        .lock()
        .await
        .set_analysis_test_gate(Some(Arc::clone(&gate)));
    let query = tokio::spawn({
        let session = session.clone();
        let uri = uri.clone();
        async move { structure_identity(&session, &uri).await }
    });
    wait_for_gate(&gate, 1).await;
    query.abort();
    let _ = query.await;
    gate.release();
    session.inner.analysis_executor.wait_idle().await;
    session
        .inner
        .state
        .lock()
        .await
        .set_analysis_test_gate(None);
    let executions = session.analysis_execution_count();
    assert!(structure_identity(&session, &uri).await.is_some());
    assert_eq!(session.analysis_execution_count(), executions + 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn termination_rejects_a_finished_analysis_waiting_for_its_final_commit() {
    let session = session();
    let uri = uri("terminated-final-commit");
    open(&session, &uri, 1, "flowchart TD\nA-->B\n").await;
    let gate = Arc::new(TestAnalysisGate::default());
    session
        .inner
        .state
        .lock()
        .await
        .set_analysis_test_gate(Some(Arc::clone(&gate)));

    let query = tokio::spawn({
        let session = session.clone();
        let uri = uri.clone();
        async move { structure_identity(&session, &uri).await }
    });
    wait_for_gate(&gate, 1).await;

    let state = session.inner.state.lock().await;
    gate.release();
    session.inner.analysis_executor.wait_idle().await;
    assert!(session.terminate());
    drop(state);

    assert!(query.await.unwrap().is_none());
}

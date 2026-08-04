use super::LanguageSession;
use super::analysis::executor::{LSP_ANALYSIS_CONCURRENCY, LSP_ANALYSIS_IN_FLIGHT_LIMIT};
use super::analysis::request::TestAnalysisGate;
use super::documents::{SemanticTokensState, default_lsp_analysis_options};
use merman_analysis::{AnalysisRuleConfig, DiagnosticSeverity};
use merman_editor_core::DocumentKind;
use std::str::FromStr;
use std::sync::{Arc, Barrier};
use std::task::Poll;
use std::time::Duration;
use tower_lsp_server::jsonrpc::ErrorCode;
use tower_lsp_server::ls_types::{TextDocumentContentChangeEvent, Uri};

fn session() -> LanguageSession {
    LanguageSession::with_cancellation(merman_analysis::AnalysisCancellationToken::new())
}

fn uri(name: &str) -> Uri {
    Uri::from_str(&format!("file:///tmp/{name}.mmd")).unwrap()
}

async fn open(session: &LanguageSession, uri: &Uri, version: i32, text: &str) {
    assert!(
        session
            .open_document(uri.clone(), version, text.to_owned(), DocumentKind::Diagram,)
            .await
    );
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

async fn wait_for_analysis_waiters(session: &LanguageSession, uri: &Uri, expected: usize) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while session.analysis_waiter_count(uri) != expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("analysis job did not reach the expected waiter count");
}

async fn wait_for_analysis_registry_state(
    session: &LanguageSession,
    expected: (usize, usize, usize),
) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while session.analysis_registry_state() != expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("analysis registry did not reach the expected state");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn identical_queries_share_one_session_analysis_and_context() {
    let session = session();
    let uri = uri("shared-context");
    open(&session, &uri, 1, "flowchart TD\nA-->B\n").await;
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
        async move { session.structure_snapshot(&uri).await }
    });
    wait_for_gate(&gate, 1).await;
    let second = tokio::spawn({
        let session = session.clone();
        let uri = uri.clone();
        async move { session.structure_snapshot(&uri).await }
    });
    wait_for_analysis_waiters(&session, &uri, 2).await;
    assert_eq!(gate.started(), 1);
    gate.release();

    let first = first.await.unwrap();
    let second = second.await.unwrap();

    assert!(Arc::ptr_eq(&first.unwrap(), &second.unwrap()));
    assert_eq!(session.analysis_execution_count(), 1);
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
                .query_semantic_tokens::<()>(&uri, None, move |_, _| {
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
                .query_semantic_tokens::<()>(&uri, None, move |_, _| {
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
                .query_semantic_tokens(&uri, None, move |_, _| {
                    entered.wait();
                    release.wait();
                    Ok(Some((
                        (),
                        Some(SemanticTokensState::new(
                            Some("terminated".to_owned()),
                            Vec::new(),
                        )),
                    )))
                })
                .await
        }
    });
    entered.wait();

    let state = session.inner.state.lock().await;
    assert!(state.semantic_tokens_state(&uri).is_none());
    release.wait();
    assert!(session.terminate());
    drop(state);

    assert!(query.await.unwrap().unwrap().is_none());
    assert!(
        session
            .inner
            .state
            .lock()
            .await
            .semantic_tokens_state(&uri)
            .is_none()
    );
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
                .pull_diagnostics(&uri, |document, _| {
                    super::documents::DocumentDiagnosticState {
                        result_id: document.version.to_string(),
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
async fn diagnostic_policy_reprojects_without_rebuilding_the_snapshot() {
    let session = session();
    let uri = uri("diagnostic-reprojection");
    open(&session, &uri, 1, "").await;
    let initial = session.structure_snapshot(&uri).await.unwrap();
    let executions = session.analysis_execution_count();
    assert_eq!(
        session.diagnostic_reprojection_count(),
        0,
        "a snapshot-only structure query must not project diagnostics"
    );
    assert_eq!(
        session.inner.state.lock().await.analysis_cache_len(),
        1,
        "a snapshot-only structure query must retain reusable parse evidence"
    );
    assert!(!session.inner.state.lock().await.has_analysis_payload(&uri));

    let options = default_lsp_analysis_options().with_rule_config(
        AnalysisRuleConfig::default()
            .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
            .unwrap(),
    );
    let change = session.update_configuration(options).await;
    assert!(change.affects_diagnostics());
    assert!(!change.affects_snapshots());
    let projected = session.structure_snapshot(&uri).await.unwrap();
    let severity = session
        .query_code_actions(&uri, |context| {
            Ok(Some(
                context
                    .analysis_payload()
                    .diagnostics
                    .iter()
                    .find(|diagnostic| diagnostic.id == "merman.parse.no_diagram")
                    .expect("expected reprojected no-diagram diagnostic")
                    .severity,
            ))
        })
        .await
        .unwrap()
        .unwrap();

    assert!(Arc::ptr_eq(&initial, &projected));
    assert_eq!(session.analysis_execution_count(), executions);
    assert_eq!(session.diagnostic_reprojection_count(), 1);
    assert_eq!(severity, DiagnosticSeverity::Hint);
}

#[tokio::test]
async fn sequential_snapshot_queries_reuse_parse_evidence_without_diagnostics() {
    let session = session();
    let uri = uri("sequential-snapshot-reuse");
    open(&session, &uri, 1, "flowchart TD\nA-->B\n").await;

    let first = session.structure_snapshot(&uri).await.unwrap();
    session
        .query_semantic_tokens(&uri, None, |snapshot, _| {
            assert!(std::ptr::eq(snapshot, first.as_ref()));
            Ok(Some(((), None)))
        })
        .await
        .unwrap()
        .unwrap();
    let second = session.structure_snapshot(&uri).await.unwrap();

    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(session.analysis_execution_count(), 1);
    assert_eq!(session.diagnostic_reprojection_count(), 0);
    let state = session.inner.state.lock().await;
    assert_eq!(state.analysis_cache_len(), 1);
    assert!(state.has_snapshot(&uri));
    assert!(!state.has_analysis_payload(&uri));
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
            tokio::spawn(async move { session.structure_snapshot(&uri).await })
        })
        .collect::<Vec<_>>();
    wait_for_gate(&gate, LSP_ANALYSIS_CONCURRENCY).await;
    wait_for_analysis_registry_state(
        &session,
        (LSP_ANALYSIS_CONCURRENCY, LSP_ANALYSIS_CONCURRENCY, 0),
    )
    .await;

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
                    Ok(Some(
                        context
                            .analysis_payload()
                            .diagnostics
                            .iter()
                            .find(|diagnostic| diagnostic.id == "merman.parse.no_diagram")
                            .expect("expected no-diagram diagnostic")
                            .severity,
                    ))
                })
                .await
        }
    });
    wait_for_analysis_registry_state(
        &session,
        (
            LSP_ANALYSIS_CONCURRENCY + 1,
            LSP_ANALYSIS_CONCURRENCY + 1,
            0,
        ),
    )
    .await;
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
    assert_eq!(severity, DiagnosticSeverity::Hint);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn diagnostic_policy_update_reclaims_capacity_after_queued_reprojection_workers_exit() {
    let session = session();
    let targets = (0..LSP_ANALYSIS_IN_FLIGHT_LIMIT - LSP_ANALYSIS_CONCURRENCY)
        .map(|index| uri(&format!("diagnostic-policy-queue-{index}")))
        .collect::<Vec<_>>();
    for target in &targets {
        open(&session, target, 1, "").await;
        session
            .query_code_actions(target, |_| Ok(Some(())))
            .await
            .unwrap()
            .expect("initial code-action query should populate the complete cache");
    }

    let gate = Arc::new(TestAnalysisGate::default());
    session
        .inner
        .state
        .lock()
        .await
        .set_analysis_test_gate(Some(Arc::clone(&gate)));
    let blockers = [
        uri("diagnostic-policy-blocker-a"),
        uri("diagnostic-policy-blocker-b"),
    ];
    for blocker in &blockers {
        open(&session, blocker, 1, "flowchart TD\nA-->B\n").await;
    }
    let blocker_queries = blockers
        .iter()
        .cloned()
        .map(|uri| {
            let session = session.clone();
            tokio::spawn(async move { session.structure_snapshot(&uri).await })
        })
        .collect::<Vec<_>>();
    wait_for_gate(&gate, LSP_ANALYSIS_CONCURRENCY).await;
    wait_for_analysis_registry_state(
        &session,
        (LSP_ANALYSIS_CONCURRENCY, LSP_ANALYSIS_CONCURRENCY, 0),
    )
    .await;

    let hint = default_lsp_analysis_options().with_rule_config(
        AnalysisRuleConfig::default()
            .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
            .unwrap(),
    );
    assert!(
        session
            .update_configuration(hint)
            .await
            .affects_diagnostics()
    );
    let requests = {
        let state = session.inner.state.lock().await;
        targets
            .iter()
            .map(|target| {
                state
                    .diagnostic_reprojection_request(target)
                    .expect("policy change should leave a lazy reprojection for each cached target")
            })
            .collect::<Vec<_>>()
    };
    let reprojections = requests
        .into_iter()
        .map(|request| {
            let executor = session.inner.analysis_executor.clone();
            tokio::spawn(async move {
                executor
                    .execute_diagnostic_reprojection(request.request())
                    .await
            })
        })
        .collect::<Vec<_>>();
    wait_for_analysis_registry_state(
        &session,
        (
            LSP_ANALYSIS_IN_FLIGHT_LIMIT,
            LSP_ANALYSIS_IN_FLIGHT_LIMIT,
            0,
        ),
    )
    .await;

    let warning = default_lsp_analysis_options().with_rule_config(
        AnalysisRuleConfig::default()
            .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Warning)
            .unwrap(),
    );
    assert!(
        session
            .update_configuration(warning)
            .await
            .affects_diagnostics(),
        "the newer policy must supersede every queued diagnostic reprojection"
    );
    for reprojection in reprojections {
        assert!(
            reprojection
                .await
                .expect("queued reprojection should not panic")
                .expect_err("superseded reprojection must be cancelled without committing")
                .is_stale()
        );
    }
    wait_for_analysis_registry_state(
        &session,
        (LSP_ANALYSIS_CONCURRENCY, LSP_ANALYSIS_CONCURRENCY, 0),
    )
    .await;

    let latest = uri("diagnostic-policy-admission-after-cancel");
    open(&session, &latest, 1, "flowchart TD\nA-->C\n").await;
    let latest_query = tokio::spawn({
        let session = session.clone();
        async move { session.structure_snapshot(&latest).await }
    });
    wait_for_analysis_registry_state(
        &session,
        (
            LSP_ANALYSIS_CONCURRENCY + 1,
            LSP_ANALYSIS_CONCURRENCY + 1,
            0,
        ),
    )
    .await;
    assert!(
        !latest_query.is_finished(),
        "a fresh build may enter only after superseded projection workers exit, while the preserved builds still own both CPU permits"
    );

    gate.release();
    for blocker in blocker_queries {
        blocker
            .await
            .expect("blocker query should not panic")
            .expect("blocker snapshot should complete");
    }
    latest_query
        .await
        .expect("fresh query should not panic")
        .expect("fresh snapshot should complete");
    let severity = session
        .query_code_actions(&targets[0], |context| {
            Ok(Some(
                context
                    .analysis_payload()
                    .diagnostics
                    .iter()
                    .find(|diagnostic| diagnostic.id == "merman.parse.no_diagram")
                    .expect("expected no-diagram diagnostic")
                    .severity,
            ))
        })
        .await
        .expect("latest diagnostic query should succeed")
        .expect("latest diagnostic query should return a severity");
    assert_eq!(
        severity,
        DiagnosticSeverity::Warning,
        "the superseded hint projection must not commit over the latest policy"
    );
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
        async move { session.structure_snapshot(&uri).await }
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

    let second = tokio::spawn({
        let session = session.clone();
        let uri = uri.clone();
        async move { session.structure_snapshot(&uri).await }
    });
    wait_for_analysis_waiters(&session, &uri, 2).await;
    gate.release();

    let first = first.await.unwrap().expect("first structure snapshot");
    let second = second.await.unwrap().expect("second structure snapshot");
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(session.analysis_execution_count(), 1);

    let severity = session
        .query_code_actions(&uri, |context| {
            Ok(Some(
                context
                    .analysis_payload()
                    .diagnostics
                    .iter()
                    .find(|diagnostic| diagnostic.id == "merman.parse.no_diagram")
                    .expect("expected latest no-diagram diagnostic")
                    .severity,
            ))
        })
        .await
        .unwrap()
        .unwrap();
    assert_eq!(severity, DiagnosticSeverity::Hint);
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
                    Ok(Some(
                        context
                            .analysis_payload()
                            .diagnostics
                            .iter()
                            .find(|diagnostic| diagnostic.id == "merman.parse.no_diagram")
                            .expect("expected no-diagram diagnostic")
                            .severity,
                    ))
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
    assert_eq!(severity, DiagnosticSeverity::Hint);
    assert_eq!(session.analysis_execution_count(), 1);
    assert_eq!(session.diagnostic_reprojection_count(), 1);
    assert_eq!(session.inner.state.lock().await.analysis_cache_len(), 0);
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
async fn evicted_generation_rebuilds_once_for_concurrent_session_queries() {
    let a = uri("evicted-a");
    let b = uri("evicted-b");
    let text = "flowchart TD\nA-->B\n";
    let sizing = session();
    open(&sizing, &a, 1, text).await;
    sizing.structure_snapshot(&a).await.unwrap();
    let budget = sizing
        .inner
        .state
        .lock()
        .await
        .analysis_cache_total_weight();

    let session = LanguageSession::with_analysis_cache_budget(budget);
    open(&session, &a, 1, text).await;
    open(&session, &b, 1, text).await;
    session.structure_snapshot(&a).await.unwrap();
    session.structure_snapshot(&b).await.unwrap();
    assert!(!session.inner.state.lock().await.has_snapshot(&a));
    let executions = session.analysis_execution_count();

    let (first, second) = tokio::join!(
        session.structure_snapshot(&a),
        session.structure_snapshot(&a),
    );
    assert!(Arc::ptr_eq(&first.unwrap(), &second.unwrap()));
    assert_eq!(session.analysis_execution_count(), executions + 1);
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
        async move { session.structure_snapshot(&uri).await }
    });
    wait_for_gate(&gate, 1).await;
    query.abort();
    let _ = query.await;
    gate.release();
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let (registered, running_workers, available_cpu) = session.analysis_registry_state();
            if (registered, running_workers, available_cpu)
                == (0, 0, super::analysis::executor::LSP_ANALYSIS_CONCURRENCY)
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cancelled analysis did not release its registry and CPU admission");

    let state = session.inner.state.lock().await;
    assert_eq!(state.analysis_cache_len(), 0);
    assert!(!state.has_snapshot(&uri));
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
        async move { session.structure_snapshot(&uri).await }
    });
    wait_for_gate(&gate, 1).await;

    let state = session.inner.state.lock().await;
    gate.release();
    wait_for_analysis_registry_state(&session, (1, 0, LSP_ANALYSIS_CONCURRENCY)).await;
    assert!(session.terminate());
    drop(state);

    assert!(query.await.unwrap().is_none());
    let state = session.inner.state.lock().await;
    assert_eq!(state.analysis_cache_len(), 0);
    assert!(!state.has_snapshot(&uri));
}

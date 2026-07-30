use super::LanguageSession;
use super::analysis::request::TestAnalysisGate;
use super::documents::default_lsp_analysis_options;
use merman_analysis::{AnalysisRuleConfig, DiagnosticSeverity};
use merman_editor_core::DocumentKind;
use std::str::FromStr;
use std::sync::Arc;
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn identical_queries_share_one_session_analysis_and_context() {
    let session = session();
    let uri = uri("shared-context");
    open(&session, &uri, 1, "flowchart TD\nA-->B\n").await;

    let (first, second) = tokio::join!(
        session.structure_snapshot(&uri),
        session.structure_snapshot(&uri),
    );

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
            let (registered, active, available_cpu) = session.analysis_registry_state();
            if (registered, active, available_cpu)
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

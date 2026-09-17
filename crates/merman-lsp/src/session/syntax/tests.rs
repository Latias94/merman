use super::*;
use merman_analysis::AnalysisCancellationToken;
use merman_editor_core::DocumentKind;
use std::sync::mpsc;
use std::time::Duration;
use tokio::sync::oneshot;

async fn opened_session() -> (LanguageSession, Uri) {
    let session = LanguageSession::with_cancellation(AnalysisCancellationToken::new());
    let uri: Uri = "file:///syntax-worker.mmd".parse().unwrap();
    assert!(
        session
            .open_document(
                uri.clone(),
                1,
                "flowchart TD\nA-->B".into(),
                DocumentKind::Diagram
            )
            .await
    );
    (session, uri)
}

#[tokio::test(flavor = "current_thread")]
async fn semantic_computation_allows_same_task_control_progress() {
    let (session, uri) = opened_session().await;
    let (started, entered) = oneshot::channel();
    let (release, released) = mpsc::channel();
    let query = session.query_semantic_tokens(&uri, None, move |_, _, _| {
        started.send(()).unwrap();
        // A watchdog bounds failure on the old inline implementation; no timing speedup is asserted.
        let control_progressed = released.recv_timeout(Duration::from_secs(2)).is_ok();
        Ok(Some((control_progressed, None)))
    });
    let control = async move {
        entered.await.unwrap();
        let _ = release.send(());
    };
    let (result, ()) = tokio::join!(query, control);
    assert_eq!(
        result.unwrap(),
        Some(true),
        "syntax computation blocked the sibling control future"
    );
}

const WORKER_WATCHDOG: Duration = Duration::from_secs(5);

async fn held_semantic_request(
    session: &LanguageSession,
    uri: &Uri,
) -> (
    tokio::task::JoinHandle<Result<Option<bool>>>,
    AnalysisCancellationToken,
    mpsc::Sender<()>,
) {
    let (started, entered) = oneshot::channel();
    let (release, released) = mpsc::channel();
    let query = tokio::spawn({
        let session = session.clone();
        let uri = uri.clone();
        async move {
            session
                .query_semantic_tokens(&uri, None, move |_, _, cancellation| {
                    let _ = started.send(cancellation.clone());
                    // Closing the sender also releases the worker when a test assertion fails.
                    let released = released.recv_timeout(WORKER_WATCHDOG).is_ok();
                    Ok(Some((released, None)))
                })
                .await
        }
    });
    let cancellation = tokio::time::timeout(WORKER_WATCHDOG, entered)
        .await
        .expect("syntax worker did not start")
        .expect("syntax worker did not report its cancellation token");
    (query, cancellation, release)
}

#[tokio::test(flavor = "current_thread")]
async fn aborting_semantic_request_cancels_only_its_own_worker() {
    let (session, uri) = opened_session().await;
    let (query, cancellation, release) = held_semantic_request(&session, &uri).await;
    assert!(!cancellation.is_cancelled());

    query.abort();
    assert!(query.await.unwrap_err().is_cancelled());
    assert!(cancellation.is_cancelled());
    assert_eq!(
        session
            .query_semantic_tokens(&uri, None, |_, _, cancellation| {
                Ok(Some((!cancellation.is_cancelled(), None)))
            })
            .await
            .unwrap(),
        Some(true),
        "request cancellation must not cancel the document's syntax generation"
    );

    release.send(()).unwrap();
    session.wait_stopped().await;
}

#[tokio::test(flavor = "current_thread")]
async fn aborted_semantic_workers_retain_shared_capacity_until_they_exit() {
    let (session, uri) = opened_session().await;
    let (first, _, release_first) = held_semantic_request(&session, &uri).await;
    let (second, _, release_second) = held_semantic_request(&session, &uri).await;
    first.abort();
    second.abort();
    assert!(first.await.unwrap_err().is_cancelled());
    assert!(second.await.unwrap_err().is_cancelled());
    assert_eq!(session.inner.analysis_executor.registry_state(), (0, 2, 0));

    let (started, mut entered) = oneshot::channel();
    let mut syntax = Box::pin(session.query_semantic_tokens(&uri, None, move |_, _, _| {
        let _ = started.send(());
        Ok(Some(((), None)))
    }));
    let mut structure = Box::pin(session.query_structure(&uri, |_| Ok(Some(()))));
    assert!(futures::poll!(&mut syntax).is_pending());
    assert!(futures::poll!(&mut structure).is_pending());
    assert_eq!(session.inner.analysis_executor.registry_state(), (1, 3, 0));
    assert_eq!(session.analysis_execution_count(), 0);
    assert!(matches!(
        entered.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));

    // One real worker exit must let both queued requests progress while the other stays blocked.
    release_first.send(()).unwrap();
    let (syntax, structure) =
        tokio::time::timeout(WORKER_WATCHDOG, async { tokio::join!(syntax, structure) })
            .await
            .expect("queued syntax and structure work did not regain CPU capacity");
    assert_eq!(syntax.unwrap(), Some(()));
    assert_eq!(structure.unwrap(), Some(()));
    assert_eq!(session.analysis_execution_count(), 1);
    entered.await.unwrap();
    release_second.send(()).unwrap();
    session.wait_stopped().await;
    assert_eq!(session.inner.analysis_executor.registry_state(), (0, 0, 2));
}

#[tokio::test(flavor = "current_thread")]
async fn semantic_worker_panic_returns_internal_error_and_releases_capacity() {
    let (session, uri) = opened_session().await;
    let error = session
        .query_semantic_tokens::<()>(&uri, None, |_, _, _| panic!("test syntax worker panic"))
        .await
        .unwrap_err();
    assert_eq!(
        error.code,
        tower_lsp_server::jsonrpc::ErrorCode::InternalError
    );
    assert!(error.message.contains("syntax worker failed"));
    assert_eq!(session.inner.analysis_executor.registry_state(), (0, 0, 2));
    assert_eq!(
        session
            .query_semantic_tokens(&uri, None, |_, _, cancellation| {
                Ok(Some((!cancellation.is_cancelled(), None)))
            })
            .await
            .unwrap(),
        Some(true)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn termination_cancels_queued_syntax_and_waits_for_real_worker_exit() {
    let (session, uri) = opened_session().await;
    let (first, first_cancellation, release_first) = held_semantic_request(&session, &uri).await;
    let (second, second_cancellation, release_second) = held_semantic_request(&session, &uri).await;
    let (started, entered) = oneshot::channel();
    let mut queued = Box::pin(session.query_semantic_tokens(&uri, None, move |_, _, _| {
        let _ = started.send(());
        Ok(Some(((), None)))
    }));
    assert!(futures::poll!(&mut queued).is_pending());

    assert!(session.terminate());
    assert!(first_cancellation.is_cancelled());
    assert!(second_cancellation.is_cancelled());
    assert!(queued.await.unwrap().is_none());
    assert!(
        entered.await.is_err(),
        "queued syntax computation must not start"
    );
    assert!(first.await.unwrap().unwrap().is_none());
    assert!(second.await.unwrap().unwrap().is_none());
    let mut stopped = Box::pin(session.wait_stopped());
    assert!(futures::poll!(&mut stopped).is_pending());
    assert_eq!(session.inner.analysis_executor.registry_state(), (0, 2, 0));

    release_first.send(()).unwrap();
    release_second.send(()).unwrap();
    tokio::time::timeout(WORKER_WATCHDOG, stopped)
        .await
        .expect("session did not stop after its real syntax workers exited");
    assert_eq!(session.inner.analysis_executor.registry_state(), (0, 0, 2));
}

#[tokio::test(flavor = "current_thread")]
async fn edit_cancels_queued_semantic_request_before_computation_starts() {
    let (session, uri) = opened_session().await;
    let (first, _, release_first) = held_semantic_request(&session, &uri).await;
    let (second, _, release_second) = held_semantic_request(&session, &uri).await;
    let (started, entered) = oneshot::channel();
    let mut queued = Box::pin(session.query_semantic_tokens(&uri, None, move |_, _, _| {
        let _ = started.send(());
        Ok(Some(((), None)))
    }));
    assert!(futures::poll!(&mut queued).is_pending());
    assert_eq!(session.inner.analysis_executor.registry_state(), (0, 2, 0));

    assert_eq!(
        session
            .change_document(
                uri.clone(),
                2,
                vec![tower_lsp_server::ls_types::TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "flowchart TD\nA-->C".into(),
                }],
            )
            .await,
        Some(true)
    );
    release_first.send(()).unwrap();
    release_second.send(()).unwrap();

    let error = tokio::time::timeout(WORKER_WATCHDOG, queued)
        .await
        .expect("stale queued syntax request did not finish")
        .unwrap_err();
    assert_eq!(
        error.code,
        tower_lsp_server::jsonrpc::ErrorCode::ContentModified
    );
    assert!(
        entered.await.is_err(),
        "stale queued syntax computation must be dropped without running"
    );
    for request in [first, second] {
        assert_eq!(
            request.await.unwrap().unwrap_err().code,
            tower_lsp_server::jsonrpc::ErrorCode::ContentModified
        );
    }
    session.wait_stopped().await;
}

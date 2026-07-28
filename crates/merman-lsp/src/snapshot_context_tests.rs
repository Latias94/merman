use std::str::FromStr;

use crate::document_store::{DocumentStore, default_lsp_analysis_options};
use crate::snapshot_context::{self, SnapshotContextKind};
use merman_analysis::{AnalysisOptions, AnalysisRuleConfig, DiagnosticSeverity};
use merman_editor_core::DocumentKind;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp_server::jsonrpc::ErrorCode;
use tower_lsp_server::ls_types::Uri;

fn test_store() -> Arc<Mutex<DocumentStore>> {
    Arc::new(Mutex::new(DocumentStore::new()))
}

fn stale_message(kind: SnapshotContextKind) -> &'static str {
    match kind {
        SnapshotContextKind::CodeActions => "code action document changed",
        SnapshotContextKind::Diagnostics => "diagnostic document changed",
        SnapshotContextKind::SemanticTokens => "semantic tokens document changed",
        SnapshotContextKind::Structure => "structure document changed",
        SnapshotContextKind::WorkspaceSymbols => "workspace symbol documents changed",
    }
}

#[tokio::test(flavor = "current_thread")]
async fn stale_snapshot_commit_returns_purpose_error() {
    for kind in [
        SnapshotContextKind::Diagnostics,
        SnapshotContextKind::SemanticTokens,
        SnapshotContextKind::Structure,
    ] {
        let store = test_store();
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

        let request = {
            let mut store = store.lock().await;
            store.upsert_text(
                uri.clone(),
                1,
                "flowchart TD\nA-->B\n".to_string(),
                DocumentKind::Diagram,
            );
            store
                .snapshot_build_request(&uri)
                .expect("expected snapshot build request")
        };
        let snapshot = request.build().expect("test source should be accepted");

        {
            let mut store = store.lock().await;
            store.upsert_text(
                uri.clone(),
                2,
                "flowchart TD\nA-->C\n".to_string(),
                DocumentKind::Diagram,
            );
        }

        let error = snapshot_context::commit_snapshot_context(&store, &request, snapshot, kind)
            .await
            .expect_err("stale snapshot build should fail");

        assert_eq!(error.code, ErrorCode::ContentModified);
        assert!(error.message.contains(stale_message(kind)));
    }
}

#[tokio::test(flavor = "current_thread")]
async fn closed_snapshot_commit_returns_none() {
    let store = test_store();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    let request = {
        let mut store = store.lock().await;
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
        store
            .snapshot_build_request(&uri)
            .expect("expected snapshot build request")
    };
    let snapshot = request.build().expect("test source should be accepted");

    {
        let mut store = store.lock().await;
        store.remove(&uri);
    }

    let context = snapshot_context::commit_snapshot_context(
        &store,
        &request,
        snapshot,
        SnapshotContextKind::SemanticTokens,
    )
    .await
    .expect("closed snapshot build should not fail");

    assert!(context.is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn ensure_snapshot_current_returns_purpose_error() {
    let store = test_store();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    {
        let mut store = store.lock().await;
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
    }
    let context = snapshot_context::snapshot_context_for_uri(
        &store,
        &uri,
        SnapshotContextKind::SemanticTokens,
    )
    .await
    .expect("snapshot context build should not fail")
    .expect("expected snapshot context");
    {
        let mut store = store.lock().await;
        store.upsert_text(
            uri,
            2,
            "flowchart TD\nA-->C\n".to_string(),
            DocumentKind::Diagram,
        );
    }

    let error = snapshot_context::ensure_snapshot_current(
        &store,
        &context,
        SnapshotContextKind::SemanticTokens,
    )
    .await
    .expect_err("stale semantic token snapshot should fail");

    assert_eq!(error.code, ErrorCode::ContentModified);
    assert!(error.message.contains("semantic tokens document changed"));
}

#[tokio::test(flavor = "current_thread")]
async fn diagnostic_only_change_stales_analysis_context_but_preserves_snapshot_context() {
    let store = test_store();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    {
        let mut store = store.lock().await;
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
    }
    let context =
        snapshot_context::snapshot_context_for_uri(&store, &uri, SnapshotContextKind::CodeActions)
            .await
            .expect("snapshot context build should not fail")
            .expect("expected analysis-backed snapshot context");

    {
        let mut store = store.lock().await;
        store.apply_analyzer_options(
            default_lsp_analysis_options().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint),
            ),
        );
    }

    for (kind, message) in [
        (
            SnapshotContextKind::CodeActions,
            "code action document changed",
        ),
        (
            SnapshotContextKind::Diagnostics,
            "diagnostic document changed",
        ),
    ] {
        let error = snapshot_context::ensure_snapshot_current(&store, &context, kind)
            .await
            .expect_err("diagnostic-only changes should stale analysis-backed requests");
        assert_eq!(error.code, ErrorCode::ContentModified);
        assert!(error.message.contains(message));
    }

    snapshot_context::ensure_snapshot_current(&store, &context, SnapshotContextKind::Structure)
        .await
        .expect("diagnostic-only changes should preserve structure snapshots");
}

#[tokio::test(flavor = "current_thread")]
async fn lowered_source_limit_rejects_open_document_before_async_analysis() {
    let store = test_store();
    let uri = Uri::from_str("file:///tmp/large-after-config.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n";

    {
        let mut store = store.lock().await;
        store.upsert_text(uri.clone(), 1, source.to_string(), DocumentKind::Diagram);
        store.apply_analyzer_options(
            AnalysisOptions::default().with_max_source_bytes(Some(source.len() - 1)),
        );
        let document = store
            .get(&uri)
            .expect("expected reclassified open document");
        assert_eq!(document.text.as_ref(), "");
        assert_eq!(
            document.resource_limit,
            Some(crate::document_store::DocumentResourceLimit {
                source_len: source.len(),
                max_source_bytes: source.len() - 1,
            })
        );
    }

    let context =
        snapshot_context::snapshot_context_for_uri(&store, &uri, SnapshotContextKind::CodeActions)
            .await
            .expect("resource rejection should not become an internal error");

    assert!(context.is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn snapshot_result_releases_store_lock_and_preempts_compute_error_when_stale() {
    let store = test_store();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    {
        let mut store = store.lock().await;
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
    }

    let store_for_compute = Arc::clone(&store);
    let stale_uri = uri.clone();
    let error = snapshot_context::snapshot_result::<()>(
        &store,
        &uri,
        SnapshotContextKind::Structure,
        |_snapshot| {
            let mut store = store_for_compute
                .try_lock()
                .expect("snapshot compute should run without the store lock held");
            store.upsert_text(
                stale_uri,
                2,
                "flowchart TD\nA-->C\n".to_string(),
                DocumentKind::Diagram,
            );
            Err(tower_lsp_server::jsonrpc::Error::invalid_params(
                "old snapshot compute error",
            ))
        },
    )
    .await
    .expect_err("stale snapshot should mask compute errors");

    assert_eq!(error.code, ErrorCode::ContentModified);
    assert!(error.message.contains("structure document changed"));
}

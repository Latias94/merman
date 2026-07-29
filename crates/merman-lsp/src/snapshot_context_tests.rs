use std::str::FromStr;

use crate::document_store::{DocumentStore, default_lsp_analysis_options};
use crate::snapshot_context::{self, SnapshotContextKind};
use merman_analysis::{
    AnalysisOptions, AnalysisRuleConfig, DiagnosticSeverity, source_limit_diagnostic_span,
};
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
                    .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                    .unwrap(),
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
async fn diagnostic_reprojection_recovers_a_stale_cache_without_reparsing() {
    let store = test_store();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    {
        let mut store = store.lock().await;
        store.upsert_text(uri.clone(), 1, String::new(), DocumentKind::Diagram);
    }
    let initial =
        snapshot_context::snapshot_context_for_uri(&store, &uri, SnapshotContextKind::Diagnostics)
            .await
            .expect("initial analysis should succeed")
            .expect("expected initial analysis context");

    let (canonical, executor, plan) = {
        let mut store = store.lock().await;
        let canonical = Arc::clone(
            store
                .cached_analysis_generation(&uri)
                .expect("initial analysis should be cached")
                .canonical(),
        );
        let executor = store.analysis_executor();
        let (change, plan) = store.begin_analyzer_options(
            default_lsp_analysis_options().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                    .unwrap(),
            ),
        );
        assert_eq!(
            change,
            crate::document_store::AnalyzerConfigurationChange::DiagnosticsOnly
        );
        assert!(!store.has_analysis_payload(&uri));
        assert!(store.snapshot_build_request(&uri).is_none());
        (
            canonical,
            executor,
            plan.expect("cached analysis needs reprojection"),
        )
    };
    let execution_count = executor.execution_count();

    let recovered =
        snapshot_context::snapshot_context_for_uri(&store, &uri, SnapshotContextKind::Diagnostics)
            .await
            .expect("stale cached diagnostics should reproject")
            .expect("expected reprojected analysis context");
    let code_actions =
        snapshot_context::snapshot_context_for_uri(&store, &uri, SnapshotContextKind::CodeActions)
            .await
            .expect("reprojected cache should serve code actions")
            .expect("expected code action context");

    assert!(Arc::ptr_eq(&initial.snapshot, &recovered.snapshot));
    assert!(Arc::ptr_eq(&recovered.snapshot, &code_actions.snapshot));
    assert!(
        recovered
            .analysis_payload()
            .diagnostics
            .iter()
            .any(|diagnostic| {
                diagnostic.id == "merman.parse.no_diagram"
                    && diagnostic.severity == DiagnosticSeverity::Hint
            })
    );
    assert_eq!(
        executor.execution_count(),
        execution_count,
        "diagnostic reprojection must not rebuild the document analysis"
    );

    let mut store = store.lock().await;
    assert!(store.is_analysis_context_current(&recovered));
    assert!(Arc::ptr_eq(
        &canonical,
        store
            .cached_analysis_generation(&uri)
            .expect("reprojected analysis should be cached")
            .canonical()
    ));
    assert_eq!(
        store.commit_diagnostic_reprojection(
            plan.project()
                .expect("superseded batch can still finish safely")
        ),
        0,
        "a late batch must not overwrite the request-local reprojection"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn edit_during_diagnostic_reprojection_retries_without_building_under_the_store_lock() {
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
    snapshot_context::snapshot_context_for_uri(&store, &uri, SnapshotContextKind::Diagnostics)
        .await
        .expect("initial analysis should succeed")
        .expect("expected initial analysis context");

    let (batch, executor, execution_count) = {
        let mut store = store.lock().await;
        let executor = store.analysis_executor();
        let execution_count = executor.execution_count();
        let (change, _) = store.begin_analyzer_options(
            default_lsp_analysis_options().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                    .unwrap(),
            ),
        );
        assert_eq!(
            change,
            crate::document_store::AnalyzerConfigurationChange::DiagnosticsOnly
        );
        let request = store
            .diagnostic_reprojection_request(&uri)
            .expect("stale cache should produce a request-local reprojection");
        (
            request
                .project()
                .expect("diagnostic reprojection should complete"),
            executor,
            execution_count,
        )
    };

    {
        let mut store = store.lock().await;
        store.upsert_text(
            uri.clone(),
            2,
            "flowchart TD\nA-->C\n".to_string(),
            DocumentKind::Diagram,
        );
    }

    let stale = snapshot_context::commit_diagnostic_reprojection_context(
        &store,
        &uri,
        batch,
        SnapshotContextKind::Diagnostics,
    )
    .await
    .expect("pull diagnostics should retry a stale reprojection");
    assert!(stale.is_none());
    assert_eq!(
        executor.execution_count(),
        execution_count,
        "a stale reprojection must not synchronously rebuild while holding the store lock"
    );

    let current =
        snapshot_context::snapshot_context_for_uri(&store, &uri, SnapshotContextKind::Diagnostics)
            .await
            .expect("latest document analysis should succeed")
            .expect("expected latest analysis context");
    assert_eq!(current.snapshot.version(), 2);
    assert_eq!(executor.execution_count(), execution_count + 1);
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
                span: source_limit_diagnostic_span(source),
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

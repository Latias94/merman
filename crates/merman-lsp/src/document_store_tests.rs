use crate::document_store::{DocumentStore, SemanticTokensState, TextDocumentUpdate};
use merman_analysis::{
    AnalysisOptions, AnalysisRuleConfig, DiagnosticSeverity, FenceSemanticRole,
    FenceTextIndexSource,
};
use merman_editor_core::DocumentKind;
use tower_lsp::lsp_types::{SemanticToken, Url};

#[test]
fn plain_mermaid_documents_create_single_snapshot_fence() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "flowchart TD\nclassDef highlight fill:#f00\nA-->B\n".to_string(),
    );

    assert_eq!(snapshot.fences.len(), 1);
    assert_eq!(snapshot.fences[0].body_start, 0);
    assert_eq!(
        snapshot.fences[0].text.as_ref(),
        "flowchart TD\nclassDef highlight fill:#f00\nA-->B\n"
    );
    assert_eq!(
        snapshot.fences[0].diagram_type.as_deref(),
        Some("flowchart-v2")
    );
    assert!(
        snapshot.fences[0]
            .text_index
            .has_directive_prefix("classDef")
    );
}

#[test]
fn markdown_documents_create_fences_for_markdown_extensions() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.markdown").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "before\n```mermaid\n%%{init: {\"theme\": \"dark\"}}%%\nflowchart TD\nA-->B\n```\nafter\n"
            .to_string(),
    );

    assert_eq!(snapshot.fences.len(), 1);
    assert!(snapshot.fences[0].text.contains("flowchart TD"));
    assert!(snapshot.fences[0].text_index.node_ids().any(|id| id == "A"));
    assert_eq!(
        snapshot.fences[0].diagram_type.as_deref(),
        Some("flowchart-v2")
    );
    assert!(snapshot.fences[0].text_index.has_directive_prefix("init"));
}

#[test]
fn markdown_documents_create_multiple_mermaid_fences() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.markdown").unwrap();
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

    assert_eq!(snapshot.fences.len(), 2);
    assert_eq!(
        snapshot.fences[0].diagram_type.as_deref(),
        Some("flowchart-v2")
    );
    assert_eq!(snapshot.fences[1].diagram_type.as_deref(), Some("sequence"));
    assert_eq!(
        snapshot.fences[0].text_index.source(),
        FenceTextIndexSource::ParserComplete
    );
    assert_eq!(
        snapshot.fences[1].text_index.source(),
        FenceTextIndexSource::ParserComplete
    );
    assert!(snapshot.fences[0].text_index.node_ids().any(|id| id == "A"));
    assert!(snapshot.fences[0].text_index.node_ids().any(|id| id == "B"));
    assert!(
        snapshot.fences[1]
            .text_index
            .node_ids()
            .any(|id| id == "Alice")
    );
    assert!(
        snapshot.fences[1]
            .text_index
            .node_ids()
            .any(|id| id == "Bob")
    );
}

#[test]
fn newer_versions_replace_the_stored_snapshot() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    let first = store.upsert(uri.clone(), 1, "flowchart TD\nA-->B\n".to_string());
    let second = store.upsert(
        uri.clone(),
        2,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
    );

    assert_eq!(first.version, 1);
    assert_eq!(second.version, 2);

    let stored = store.get(&uri).unwrap();
    assert_eq!(stored.version, 2);
    assert!(stored.text.contains("sequenceDiagram"));
    assert!(!stored.text.contains("flowchart TD"));
    let stored = store
        .snapshot(&uri)
        .expect("expected stored snapshot after replacement");
    assert_eq!(stored.fences.len(), 1);
    assert_eq!(stored.fences[0].diagram_type.as_deref(), Some("sequence"));
}

#[test]
fn upsert_text_defers_snapshot_until_requested() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

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
    assert_eq!(snapshot.version, 1);
    assert_eq!(
        snapshot.fences[0].diagram_type.as_deref(),
        Some("flowchart-v2")
    );
}

#[test]
fn upsert_text_invalidates_cached_snapshot() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let first = store
        .snapshot(&uri)
        .expect("expected initial lazy snapshot");
    assert_eq!(first.version, 1);
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
    assert_eq!(second.version, 2);
    assert_eq!(second.fences[0].diagram_type.as_deref(), Some("sequence"));
}

#[test]
fn apply_text_change_rejects_missing_documents() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/missing.mmd").unwrap();

    let update = store.apply_text_change(
        uri.clone(),
        2,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
    );

    assert_eq!(update, TextDocumentUpdate::MissingDocument);
    assert!(store.get(&uri).is_none());
    assert!(!store.has_snapshot(&uri));
}

#[test]
fn apply_text_change_rejects_stale_versions_without_invalidating_current_state() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        3,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot = store
        .snapshot(&uri)
        .expect("expected current snapshot before stale edit");
    assert_eq!(snapshot.version, 3);
    assert!(store.has_snapshot(&uri));

    let update = store.apply_text_change(uri.clone(), 2, "flowchart TD\nA-->B\n".to_string());

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
fn stale_snapshot_build_request_is_not_committed_after_text_replacement() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

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
    let stale_snapshot = stale_request.build();

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
    assert_eq!(current.version, 2);
    assert_eq!(current.fences[0].diagram_type.as_deref(), Some("sequence"));
}

#[test]
fn snapshot_build_requests_reuse_current_cached_snapshots() {
    let mut store = DocumentStore::new();
    let cached_uri = Url::parse("file:///tmp/cached.mmd").unwrap();
    let missing_uri = Url::parse("file:///tmp/missing.mmd").unwrap();

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
    assert_eq!(contexts[0].snapshot.uri, cached_uri);
    assert!(std::sync::Arc::ptr_eq(
        &contexts[0].snapshot,
        &cached_snapshot
    ));
    assert!(store.is_snapshot_context_current(&contexts[0]));
    assert_eq!(requests.len(), 1);
    let request = requests.pop().unwrap();
    let built = request.build();
    let committed = store.snapshot_contexts_for_requests(vec![(request, built)]);
    assert_eq!(committed.contexts.len(), 1);
    assert_eq!(committed.contexts[0].snapshot.uri, missing_uri);
    assert!(!committed.stale_open_documents);
}

#[test]
fn workspace_symbol_build_plan_batches_all_missing_snapshots() {
    let mut store = DocumentStore::new();
    for index in 0..40 {
        let uri = Url::parse(&format!("file:///tmp/workspace-{index}.mmd")).unwrap();
        store.upsert_text(
            uri,
            1,
            format!("flowchart TD\nN{index}-->B\n"),
            DocumentKind::Diagram,
        );
    }

    let plan = store.workspace_symbol_snapshot_build_plan(8);

    assert!(plan.contexts.is_empty());
    assert_eq!(plan.batches.len(), 5);
    assert!(plan.batches.iter().all(|batch| batch.len() <= 8));
    assert_eq!(plan.new_snapshot_request_count(), 40);
}

#[test]
fn workspace_symbol_build_plan_keeps_cached_contexts_with_all_missing_snapshots() {
    let mut store = DocumentStore::new();
    let cached_uri = Url::parse("file:///tmp/cached.mmd").unwrap();
    let cached_snapshot = store.upsert(
        cached_uri.clone(),
        1,
        "flowchart TD\nCached-->B\n".to_string(),
    );
    for index in 0..5 {
        let uri = Url::parse(&format!("file:///tmp/missing-{index}.mmd")).unwrap();
        store.upsert_text(
            uri,
            1,
            format!("flowchart TD\nMissing{index}-->B\n"),
            DocumentKind::Diagram,
        );
    }

    let plan = store.workspace_symbol_snapshot_build_plan(2);

    assert_eq!(plan.contexts.len(), 1);
    assert!(std::sync::Arc::ptr_eq(
        &plan.contexts[0].snapshot,
        &cached_snapshot
    ));
    assert_eq!(plan.batches.len(), 3);
    assert_eq!(plan.batches[0].len(), 2);
    assert_eq!(plan.batches[1].len(), 2);
    assert_eq!(plan.batches[2].len(), 1);
    assert_eq!(plan.new_snapshot_request_count(), 5);
    assert!(store.is_snapshot_contexts_current(&plan.contexts));
}

#[test]
fn workspace_symbol_contexts_current_requires_complete_document_set() {
    let mut store = DocumentStore::new();
    let cached_uri = Url::parse("file:///tmp/cached.mmd").unwrap();
    store.upsert(cached_uri, 1, "flowchart TD\nCached-->B\n".to_string());

    let plan = store.workspace_symbol_snapshot_build_plan(8);
    assert_eq!(plan.contexts.len(), 1);
    assert!(plan.batches.is_empty());
    assert!(store.workspace_symbol_snapshot_contexts_current(&plan.contexts));

    store.upsert_text(
        Url::parse("file:///tmp/added.mmd").unwrap(),
        1,
        "flowchart TD\nAdded-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(!store.workspace_symbol_snapshot_contexts_current(&plan.contexts));
}

#[test]
fn cached_snapshot_build_context_stales_after_text_replacement() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/cached.mmd").unwrap();

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
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

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
        store.apply_analyzer_options(AnalysisOptions::default()),
        crate::document_store::AnalyzerConfigurationChange::Unchanged
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
fn diagnostic_only_analyzer_update_stales_diagnostics_but_preserves_snapshots_and_tokens() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

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
        AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint),
        ),
    );

    assert!(store.is_snapshot_context_current(&snapshot_context));
    assert!(!store.is_diagnostic_context_current(&diagnostic_context));
    assert!(store.has_snapshot(&uri));
    assert_eq!(
        store
            .semantic_tokens_state(&uri)
            .and_then(|state| state.result_id.as_deref()),
        Some("tokens-1")
    );
}

#[test]
fn text_replacement_stales_contexts_but_keeps_committed_token_baseline() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

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
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

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
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

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
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

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
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

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
fn diagnostic_only_analyzer_update_preserves_cached_snapshot_and_tokens() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot = store
        .snapshot(&uri)
        .expect("expected initial lazy snapshot");
    assert_eq!(
        snapshot.fences[0].diagram_type.as_deref(),
        Some("flowchart-v2")
    );
    let token_context = store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    assert!(store.set_semantic_tokens_state_if_current(
        &token_context,
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

    store.apply_analyzer_options(
        AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint),
        ),
    );

    assert!(store.has_snapshot(&uri));
    assert_eq!(
        store
            .semantic_tokens_state(&uri)
            .and_then(|state| state.result_id.as_deref()),
        Some("tokens-1")
    );
    let cached = store
        .snapshot(&uri)
        .expect("expected cached snapshot after diagnostic-only update");
    assert_eq!(
        cached.fences[0].diagram_type.as_deref(),
        Some("flowchart-v2")
    );
}

#[test]
fn snapshot_affecting_analyzer_update_invalidates_cached_snapshot_and_tokens() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot = store
        .snapshot(&uri)
        .expect("expected initial lazy snapshot");
    assert_eq!(
        snapshot.fences[0].diagram_type.as_deref(),
        Some("flowchart-v2")
    );
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
    let rebuilt = store
        .snapshot(&uri)
        .expect("expected rebuilt snapshot after analyzer replacement");
    assert!(rebuilt.fences.is_empty());
}

#[test]
fn incomplete_flowchart_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "flowchart TD\nsubgraph group\nA-->B\nC-->".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

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
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "sequenceDiagram\nparticipant Alice\nactor Bob\nAlice->>Bob: Hi\n".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "Alice"));
    assert!(index.node_ids().any(|id| id == "Bob"));
}

#[test]
fn sequence_payload_facts_do_not_pollute_completion_ids() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
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
    let index = &snapshot.fences[0].text_index;

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
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
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
    let index = &snapshot.fences[0].text_index;

    assert_eq!(
        snapshot.fences[0].diagram_type.as_deref(),
        Some("architecture")
    );
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
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
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
    let index = &snapshot.fences[0].text_index;

    assert_eq!(snapshot.fences[0].diagram_type.as_deref(), Some("radar"));
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
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
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
    let index = &snapshot.fences[0].text_index;

    assert_eq!(snapshot.fences[0].diagram_type.as_deref(), Some("treemap"));
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
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
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
    let index = &snapshot.fences[0].text_index;

    assert_eq!(snapshot.fences[0].diagram_type.as_deref(), Some("block"));
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
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
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
    let index = &snapshot.fences[0].text_index;

    assert_eq!(snapshot.fences[0].diagram_type.as_deref(), Some("c4"));
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
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "zenuml\n",
            "title Login Flow\n",
            "accTitle Login accessibility title\n",
            "accDescr Login accessibility description\n",
            "Alice\n",
            "Bob\n",
            "A as API\n",
            "Alice->Bob: Login\n",
            "SomeType result = A.SyncMessage()\n",
            "new Session(with, params)\n",
        )
        .to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(snapshot.fences[0].diagram_type.as_deref(), Some("zenuml"));
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    for id in ["Alice", "Bob", "A", "Session"] {
        assert!(index.node_ids().any(|candidate| candidate == id));
    }
    for prefix in ["title", "accTitle", "accDescr"] {
        assert!(index.has_directive_prefix(prefix));
    }
    for payload in [
        "Login Flow",
        "Login accessibility title",
        "Login accessibility description",
        "API",
        "Login",
        "SyncMessage()",
        "result",
        "Session(with, params)",
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
        let uri = Url::parse("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri, 1, case.1.to_string());
        let index = &snapshot.fences[0].text_index;

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
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "sequenceDiagram\nAlice->>Bob: Hi\nBob->>".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "Alice"));
    assert!(index.node_ids().any(|id| id == "Bob"));
}

#[test]
fn state_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
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
    let index = &snapshot.fences[0].text_index;

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
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "stateDiagram-v2\nIdle --> Running\nRunning -->".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "Idle"));
    assert!(index.node_ids().any(|id| id == "Running"));
}

#[test]
fn class_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "classDiagram\nclass User\nUser <|-- Admin\n".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "User"));
    assert!(index.node_ids().any(|id| id == "Admin"));
}

#[test]
fn incomplete_class_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "classDiagram\nclass User\nUser <|--".to_string());
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "User"));
}

#[test]
fn class_member_outline_facts_do_not_pollute_completion_ids() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
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
    let index = &snapshot.fences[0].text_index;

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
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "erDiagram\nCUSTOMER ||--o{ ORDER : places\n".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "CUSTOMER"));
    assert!(index.node_ids().any(|id| id == "ORDER"));
}

#[test]
fn incomplete_er_documents_use_recovered_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "erDiagram\nCUSTOMER ||--o{ ORDER :".to_string());
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "CUSTOMER"));
    assert!(index.node_ids().any(|id| id == "ORDER"));
}

#[test]
fn er_attribute_payload_facts_do_not_pollute_completion_ids() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
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
    let index = &snapshot.fences[0].text_index;

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
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
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
    let index = &snapshot.fences[0].text_index;

    assert_eq!(snapshot.fences[0].diagram_type.as_deref(), Some("gantt"));
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
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "gantt\ndateFormat YYYY-MM-DD\nTask 1: id1,2014-01-01,1d\nTask 2".to_string(),
    );
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "id1"));
    assert!(!index.node_ids().any(|id| id == "Task"));
}

#[test]
fn mindmap_documents_use_parser_facts() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
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
    let index = &snapshot.fences[0].text_index;

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
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "mindmap\nroot\n child[unterminated".to_string());
    let index = &snapshot.fences[0].text_index;

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "root"));
    assert!(!index.node_ids().any(|id| id == "child"));
}

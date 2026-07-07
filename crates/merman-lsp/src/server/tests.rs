use super::MermanLanguageServer;
use super::stale_diagnostic_recompute_error;
use crate::diagnostics::analysis_diagnostic_to_versioned_lsp;
use crate::document_store::{
    DocumentDiagnosticState, DocumentStore, StoredDocument, WORKSPACE_SYMBOL_SNAPSHOT_BATCH_SIZE,
};
use crate::protocol::{CONFIG_SCHEMA_METHOD, RULE_CATALOG_METHOD, RULE_CATALOG_RESPONSE_VERSION};
use crate::structure::{
    document_symbols, folding_ranges, goto_definition, hover, prepare_rename, references, rename,
    selection_ranges,
};
use merman_analysis::{
    AnalysisDiagnostic, AnalysisOptions, AnalysisRuleConfig, DiagnosticCategory, DiagnosticFix,
    DiagnosticFixEdit, DiagnosticSeverity, SourceMap,
};
use merman_core::ParseOptions;
use merman_editor_core::DocumentKind;
use tower::{Service, ServiceExt};
use tower_lsp::LanguageServer;
use tower_lsp::jsonrpc::Request;
use tower_lsp::lsp_types::SemanticTokensResult;
use tower_lsp::lsp_types::{
    CodeActionContext, CodeActionKind, CodeActionOrCommand, CodeActionParams,
    CodeActionProviderCapability, DidChangeTextDocumentParams, DidOpenTextDocumentParams,
    DocumentChanges, DocumentDiagnosticParams, DocumentDiagnosticReport,
    DocumentDiagnosticReportResult, DocumentSymbolResponse, FoldingRangeParams,
    FoldingRangeProviderCapability, GotoDefinitionResponse, HoverContents, HoverParams,
    InitializeParams, NumberOrString, Position, Range, RenameParams, SelectionRangeParams,
    SelectionRangeProviderCapability, SemanticTokensFullOptions, SemanticTokensParams,
    SemanticTokensRangeParams, SemanticTokensRangeResult, SemanticTokensServerCapabilities,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind, Url,
    VersionedTextDocumentIdentifier, WorkspaceSymbolParams,
};
use tower_lsp::lsp_types::{HoverProviderCapability, OneOf};

#[test]
fn snapshot_build_requests_keep_cached_contexts_invalidatable() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/workspace-symbols.mmd").unwrap();
    store.upsert(uri.clone(), 1, "flowchart TD\nA[old] --> B\n".to_string());

    let (contexts, requests) = store.snapshot_build_requests();
    assert_eq!(contexts.len(), 1);
    assert!(requests.is_empty());
    assert!(store.is_snapshot_contexts_current(&contexts));

    store.upsert_text(
        uri,
        2,
        "flowchart TD\nA[new] --> C\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(!store.is_snapshot_contexts_current(&contexts));
}

#[test]
fn diagnostic_state_is_bound_to_document_epoch() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/diagnostic-state.mmd").unwrap();
    store.upsert_text(
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

    store.upsert_text(
        uri.clone(),
        2,
        "flowchart TD\nA-->C\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(store.diagnostic_state(&uri).is_none());
    assert!(!store.set_diagnostic_state_if_current(&context, state));
}

#[test]
fn analyzer_configuration_change_classifies_diagnostic_only_rule_changes() {
    let current = AnalysisOptions::default();
    let next = AnalysisOptions::default().with_rule_config(
        AnalysisRuleConfig::default()
            .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint),
    );

    assert_eq!(
        crate::document_store::analyzer_configuration_change(&current, &next),
        crate::document_store::AnalyzerConfigurationChange::DiagnosticsOnly
    );
}

#[test]
fn analyzer_configuration_change_classifies_snapshot_affecting_changes() {
    let current = AnalysisOptions::default();
    let changed_parse = AnalysisOptions::default().with_parse_options(ParseOptions::lenient());
    let changed_resource = AnalysisOptions::default().with_max_source_bytes(Some(1));
    let changed_date =
        AnalysisOptions::default().with_fixed_today(Some("2026-07-02".parse().unwrap()));

    for next in [changed_parse, changed_resource, changed_date] {
        assert_eq!(
            crate::document_store::analyzer_configuration_change(&current, &next),
            crate::document_store::AnalyzerConfigurationChange::SnapshotAffecting
        );
    }
}

#[test]
fn analyzer_configuration_change_classifies_unchanged_options() {
    let current = AnalysisOptions::default();

    assert_eq!(
        crate::document_store::analyzer_configuration_change(&current, &current),
        crate::document_store::AnalyzerConfigurationChange::Unchanged
    );
}

#[test]
fn capabilities_advertise_completion_and_incremental_sync() {
    let capabilities = MermanLanguageServer::capabilities();

    assert!(matches!(
        capabilities.text_document_sync,
        Some(TextDocumentSyncCapability::Kind(
            TextDocumentSyncKind::INCREMENTAL
        ))
    ));
    assert!(matches!(
        capabilities.hover_provider,
        Some(HoverProviderCapability::Simple(true))
    ));
    assert!(matches!(
        capabilities.selection_range_provider,
        Some(SelectionRangeProviderCapability::Simple(true))
    ));
    assert!(matches!(
        capabilities.folding_range_provider,
        Some(FoldingRangeProviderCapability::Simple(true))
    ));
    assert!(matches!(
        capabilities.document_symbol_provider,
        Some(OneOf::Left(true))
    ));
    assert!(matches!(
        capabilities.definition_provider,
        Some(OneOf::Left(true))
    ));
    assert!(matches!(
        capabilities.references_provider,
        Some(OneOf::Left(true))
    ));
    assert!(matches!(
        capabilities.rename_provider,
        Some(OneOf::Right(options)) if options.prepare_provider == Some(true)
    ));
    assert!(matches!(
        capabilities.workspace_symbol_provider,
        Some(OneOf::Left(true))
    ));
    assert!(matches!(
        capabilities.completion_provider,
        Some(ref options) if options.resolve_provider == Some(true)
            && options.trigger_characters.as_deref() == Some(&[
                " ".to_string(),
                "\n".to_string(),
                "-".to_string(),
                "@".to_string(),
                ":".to_string(),
            ])
    ));
    assert!(matches!(
        capabilities.semantic_tokens_provider,
        Some(SemanticTokensServerCapabilities::SemanticTokensOptions(ref options))
            if matches!(options.full, Some(SemanticTokensFullOptions::Delta { delta: Some(true) }))
                && options.range == Some(true)
                && !options.legend.token_types.is_empty()
                && !options.legend.token_modifiers.is_empty()
    ));
    assert!(matches!(
        capabilities.code_action_provider,
        Some(CodeActionProviderCapability::Options(ref options))
            if options
                .code_action_kinds
                .as_ref()
                .is_some_and(|kinds| kinds.contains(&CodeActionKind::QUICKFIX))
                && options.resolve_provider == Some(false)
    ));
    assert_eq!(
        capabilities.experimental.as_ref().unwrap()["merman"]["requests"]["ruleCatalog"],
        RULE_CATALOG_METHOD
    );
    assert_eq!(
        capabilities.experimental.as_ref().unwrap()["merman"]["requests"]["configSchema"],
        CONFIG_SCHEMA_METHOD
    );
}

#[test]
fn diagnostics_use_stored_markdown_kind_for_extensionless_documents() {
    let uri = Url::parse("untitled:notes").unwrap();
    let document = StoredDocument {
        uri: uri.clone(),
        version: 7,
        text: "before\n```mermaid\nflowchart TD\nA[unterminated\n```\nafter\n".into(),
        kind: DocumentKind::Markdown,
    };
    let diagnostics = MermanLanguageServer::diagnostics_for_document(
        &document,
        &merman_analysis::Analyzer::new(),
    );

    assert!(
        diagnostics.iter().all(|diagnostic| {
            diagnostic.code
                != Some(NumberOrString::String(
                    "merman.parse.no_diagram".to_string(),
                ))
        }),
        "expected markdown document analysis, got {diagnostics:?}"
    );
    let parse_diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.code
                == Some(NumberOrString::String(
                    "merman.parse.diagram_parse".to_string(),
                ))
        })
        .expect("expected diagram parse diagnostic from markdown fence");
    assert!(
        parse_diagnostic.range.start.line >= 2,
        "expected markdown fence body range, got {:?}",
        parse_diagnostic.range
    );
    assert_eq!(
        parse_diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("documentVersion")),
        Some(&serde_json::json!(7))
    );
}

#[test]
fn semantic_tokens_refresh_support_comes_from_client_capabilities() {
    let mut params = InitializeParams::default();
    assert!(!MermanLanguageServer::client_supports_semantic_tokens_refresh(&params));

    params.capabilities.workspace = Some(Default::default());
    assert!(!MermanLanguageServer::client_supports_semantic_tokens_refresh(&params));

    params
        .capabilities
        .workspace
        .as_mut()
        .unwrap()
        .semantic_tokens = Some(
        tower_lsp::lsp_types::SemanticTokensWorkspaceClientCapabilities {
            refresh_support: None,
        },
    );
    assert!(!MermanLanguageServer::client_supports_semantic_tokens_refresh(&params));

    params
        .capabilities
        .workspace
        .as_mut()
        .unwrap()
        .semantic_tokens
        .as_mut()
        .unwrap()
        .refresh_support = Some(true);
    assert!(MermanLanguageServer::client_supports_semantic_tokens_refresh(&params));
}

#[test]
fn diagnostic_pull_support_comes_from_text_document_client_capabilities() {
    let mut params = InitializeParams::default();
    assert!(!MermanLanguageServer::client_supports_diagnostic_pull(
        &params
    ));

    params.capabilities.workspace = Some(Default::default());
    params.capabilities.workspace.as_mut().unwrap().diagnostic = Some(
        tower_lsp::lsp_types::DiagnosticWorkspaceClientCapabilities {
            refresh_support: Some(true),
        },
    );
    assert!(!MermanLanguageServer::client_supports_diagnostic_pull(
        &params
    ));

    params.capabilities.text_document = Some(Default::default());
    params
        .capabilities
        .text_document
        .as_mut()
        .unwrap()
        .diagnostic = Some(tower_lsp::lsp_types::DiagnosticClientCapabilities {
        dynamic_registration: None,
        related_document_support: None,
    });
    assert!(MermanLanguageServer::client_supports_diagnostic_pull(
        &params
    ));
}

#[test]
fn diagnostic_refresh_support_comes_from_workspace_client_capabilities() {
    let mut params = InitializeParams::default();
    assert!(!MermanLanguageServer::client_supports_diagnostic_refresh(
        &params
    ));

    params.capabilities.text_document = Some(Default::default());
    params
        .capabilities
        .text_document
        .as_mut()
        .unwrap()
        .diagnostic = Some(tower_lsp::lsp_types::DiagnosticClientCapabilities {
        dynamic_registration: None,
        related_document_support: None,
    });
    assert!(!MermanLanguageServer::client_supports_diagnostic_refresh(
        &params
    ));

    params.capabilities.workspace = Some(Default::default());
    params.capabilities.workspace.as_mut().unwrap().diagnostic = Some(
        tower_lsp::lsp_types::DiagnosticWorkspaceClientCapabilities {
            refresh_support: None,
        },
    );
    assert!(!MermanLanguageServer::client_supports_diagnostic_refresh(
        &params
    ));

    params
        .capabilities
        .workspace
        .as_mut()
        .unwrap()
        .diagnostic
        .as_mut()
        .unwrap()
        .refresh_support = Some(true);
    assert!(MermanLanguageServer::client_supports_diagnostic_refresh(
        &params
    ));
}

#[test]
fn workspace_edit_document_changes_support_comes_from_client_capabilities() {
    let mut params = InitializeParams::default();
    assert!(!MermanLanguageServer::client_supports_workspace_edit_document_changes(&params));

    params.capabilities.workspace = Some(Default::default());
    assert!(!MermanLanguageServer::client_supports_workspace_edit_document_changes(&params));

    params
        .capabilities
        .workspace
        .as_mut()
        .unwrap()
        .workspace_edit = Some(tower_lsp::lsp_types::WorkspaceEditClientCapabilities {
        document_changes: None,
        resource_operations: None,
        failure_handling: None,
        normalizes_line_endings: None,
        change_annotation_support: None,
    });
    assert!(!MermanLanguageServer::client_supports_workspace_edit_document_changes(&params));

    params
        .capabilities
        .workspace
        .as_mut()
        .unwrap()
        .workspace_edit
        .as_mut()
        .unwrap()
        .document_changes = Some(true);
    assert!(MermanLanguageServer::client_supports_workspace_edit_document_changes(&params));
}

#[test]
fn hierarchical_document_symbol_support_comes_from_client_capabilities() {
    let mut params = InitializeParams::default();
    assert!(!MermanLanguageServer::client_supports_hierarchical_document_symbols(&params));

    params.capabilities.text_document = Some(Default::default());
    assert!(!MermanLanguageServer::client_supports_hierarchical_document_symbols(&params));

    params
        .capabilities
        .text_document
        .as_mut()
        .unwrap()
        .document_symbol = Some(tower_lsp::lsp_types::DocumentSymbolClientCapabilities {
        dynamic_registration: None,
        symbol_kind: None,
        hierarchical_document_symbol_support: None,
        tag_support: None,
    });
    assert!(!MermanLanguageServer::client_supports_hierarchical_document_symbols(&params));

    params
        .capabilities
        .text_document
        .as_mut()
        .unwrap()
        .document_symbol
        .as_mut()
        .unwrap()
        .hierarchical_document_symbol_support = Some(true);
    assert!(MermanLanguageServer::client_supports_hierarchical_document_symbols(&params));
}

#[tokio::test(flavor = "current_thread")]
async fn did_open_defers_editor_snapshot_until_editor_request() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "mermaid".to_string(),
                version: 1,
                text: "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
            },
        })
        .await;

    {
        let store = server.store.lock().await;
        assert!(store.get(&uri).is_some());
        assert!(!store.has_snapshot(&uri));
    }

    let hover = server
        .hover(HoverParams {
            text_document_position_params: TextDocumentPositionParams::new(
                TextDocumentIdentifier { uri: uri.clone() },
                Position::new(1, 0),
            ),
            work_done_progress_params: Default::default(),
        })
        .await
        .unwrap();

    assert!(hover.is_some());
    let store = server.store.lock().await;
    assert!(store.has_snapshot(&uri));
}

#[tokio::test(flavor = "current_thread")]
async fn did_open_uses_language_id_and_change_preserves_document_kind() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    let uri = Url::parse("untitled:notes").unwrap();

    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "markdown".to_string(),
                version: 1,
                text: "```mermaid\nflowchart TD\nA-->B\n```\n".to_string(),
            },
        })
        .await;

    let snapshot = server
        .snapshot_for_uri(&uri)
        .await
        .expect("expected markdown snapshot");
    assert_eq!(snapshot.kind, DocumentKind::Markdown);
    assert_eq!(snapshot.fences.len(), 1);
    assert_eq!(
        snapshot.fences[0].diagram_type.as_deref(),
        Some("flowchart-v2")
    );

    server
        .did_change(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version: 2,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "```mermaid\nsequenceDiagram\nAlice->>Bob: Hi\n```\n".to_string(),
            }],
        })
        .await;

    let snapshot = server
        .snapshot_for_uri(&uri)
        .await
        .expect("expected changed markdown snapshot");
    assert_eq!(snapshot.kind, DocumentKind::Markdown);
    assert_eq!(snapshot.fences.len(), 1);
    assert_eq!(snapshot.fences[0].diagram_type.as_deref(), Some("sequence"));
}

#[tokio::test(flavor = "current_thread")]
async fn did_change_rejects_stale_document_versions() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "mermaid".to_string(),
                version: 1,
                text: "flowchart TD\nA-->B\n".to_string(),
            },
        })
        .await;

    server
        .did_change(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version: 3,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
            }],
        })
        .await;

    server
        .did_change(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version: 2,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "flowchart TD\nstale-->text\n".to_string(),
            }],
        })
        .await;

    let stored = {
        let store = server.store.lock().await;
        store.get(&uri).expect("expected stored document").clone()
    };
    assert_eq!(stored.version, 3);
    assert!(stored.text.contains("sequenceDiagram"));
    assert!(!stored.text.contains("stale"));

    let snapshot = server
        .snapshot_for_uri(&uri)
        .await
        .expect("expected current snapshot");
    assert_eq!(snapshot.version, 3);
    assert_eq!(snapshot.fences[0].diagram_type.as_deref(), Some("sequence"));
}

#[tokio::test(flavor = "current_thread")]
async fn did_change_applies_incremental_changes_in_order() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "mermaid".to_string(),
                version: 1,
                text: "flowchart TD\nA-->B\n".to_string(),
            },
        })
        .await;

    server
        .did_change(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version: 2,
            },
            content_changes: vec![
                TextDocumentContentChangeEvent {
                    range: Some(Range::new(Position::new(1, 4), Position::new(1, 5))),
                    range_length: None,
                    text: "C".to_string(),
                },
                TextDocumentContentChangeEvent {
                    range: Some(Range::new(Position::new(1, 5), Position::new(1, 5))),
                    range_length: None,
                    text: "\nC-->D".to_string(),
                },
            ],
        })
        .await;

    let stored = {
        let store = server.store.lock().await;
        store.get(&uri).expect("expected stored document").clone()
    };
    assert_eq!(stored.version, 2);
    assert_eq!(stored.text.as_ref(), "flowchart TD\nA-->C\nC-->D\n");

    let snapshot = server
        .snapshot_for_uri(&uri)
        .await
        .expect("expected changed snapshot");
    assert_eq!(
        snapshot.fences[0].diagram_type.as_deref(),
        Some("flowchart-v2")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn stale_diagnostic_context_returns_content_modified_error() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    {
        let mut store = server.store.lock().await;
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
    }
    let context = {
        let store = server.store.lock().await;
        store
            .diagnostic_context(&uri)
            .expect("expected diagnostic context")
    };
    {
        let mut store = server.store.lock().await;
        store.upsert_text(
            uri.clone(),
            2,
            "flowchart TD\nA-->C\n".to_string(),
            DocumentKind::Diagram,
        );
    }

    let error = server
        .diagnostics_for_current_context(&context)
        .await
        .ok_or_else(stale_diagnostic_recompute_error)
        .expect_err("stale context should fail");

    assert_eq!(error.code, tower_lsp::jsonrpc::ErrorCode::ContentModified);
    assert!(error.message.contains("diagnostic document changed"));
}

#[tokio::test(flavor = "current_thread")]
async fn stale_semantic_tokens_record_returns_content_modified_error() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    {
        let mut store = server.store.lock().await;
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
    }
    let context = crate::snapshot_context::snapshot_context_for_uri(
        &server.store,
        &uri,
        crate::snapshot_context::SnapshotContextKind::SemanticTokens,
    )
    .await
    .expect("snapshot context build should not fail")
    .expect("expected snapshot context");
    {
        let mut store = server.store.lock().await;
        store.upsert_text(
            uri.clone(),
            2,
            "flowchart TD\nA-->C\n".to_string(),
            DocumentKind::Diagram,
        );
    }

    let error = server
        .record_semantic_tokens_state(&context, Vec::new(), Some("stale-result".to_string()))
        .await
        .expect_err("stale semantic tokens should fail");

    assert_eq!(error.code, tower_lsp::jsonrpc::ErrorCode::ContentModified);
    assert!(error.message.contains("semantic tokens document changed"));
}

#[tokio::test(flavor = "current_thread")]
async fn stale_initial_diagnostic_context_recomputes_latest_document() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    {
        let mut store = server.store.lock().await;
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
    }
    let context = {
        let store = server.store.lock().await;
        store
            .diagnostic_context(&uri)
            .expect("expected diagnostic context")
    };
    {
        let mut store = server.store.lock().await;
        store.upsert_text(
            uri.clone(),
            2,
            "flowchart TD\nA[unterminated\n".to_string(),
            DocumentKind::Diagram,
        );
    }

    let (_context, diagnostics) = server
        .diagnostics_or_recompute_latest(context)
        .await
        .expect("latest diagnostic context should recompute");
    let parse_diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.code
                == Some(NumberOrString::String(
                    "merman.parse.diagram_parse".to_string(),
                ))
        })
        .expect("expected latest parse diagnostic");
    let data = parse_diagnostic
        .data
        .as_ref()
        .expect("expected diagnostic data");
    assert_eq!(data["documentVersion"], 2);
}

#[tokio::test(flavor = "current_thread")]
async fn stale_diagnostic_commit_returns_content_modified_error() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    {
        let mut store = server.store.lock().await;
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        );
    }
    let (context, diagnostics) = {
        let store = server.store.lock().await;
        let context = store
            .diagnostic_context(&uri)
            .expect("expected diagnostic context");
        let diagnostics =
            MermanLanguageServer::diagnostics_for_document(&context.document, &context.analyzer);
        (context, diagnostics)
    };
    let state = DocumentDiagnosticState {
        result_id: MermanLanguageServer::diagnostic_result_id(&diagnostics),
        diagnostics,
    };
    {
        let mut store = server.store.lock().await;
        store.upsert_text(
            uri.clone(),
            2,
            "flowchart TD\nA-->C\n".to_string(),
            DocumentKind::Diagram,
        );
    }

    let error = server
        .commit_diagnostic_state_if_current(&context, state)
        .await
        .expect_err("stale diagnostic commit should fail");

    assert_eq!(error.code, tower_lsp::jsonrpc::ErrorCode::ContentModified);
    assert!(error.message.contains("diagnostic document changed"));
}

#[tokio::test(flavor = "current_thread")]
async fn diagnostic_pull_reuses_cached_previous_result() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "mermaid".to_string(),
                version: 1,
                text: "flowchart TD\nA-->B\n".to_string(),
            },
        })
        .await;

    let first = server
        .diagnostic(DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .unwrap();
    let result_id = match first {
        DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(report)) => report
            .full_document_diagnostic_report
            .result_id
            .expect("expected diagnostic result id"),
        other => panic!("unexpected first diagnostic report: {other:?}"),
    };

    let second = server
        .diagnostic(DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier { uri },
            identifier: None,
            previous_result_id: Some(result_id.clone()),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .unwrap();

    assert!(matches!(
        second,
        DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Unchanged(report))
            if report.unchanged_document_diagnostic_report.result_id == result_id
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn code_action_rejects_stale_diagnostic_edits_after_document_change() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "mermaid".to_string(),
                version: 1,
                text: "bad".to_string(),
            },
        })
        .await;

    let map = SourceMap::new("bad");
    let stale_diagnostic = AnalysisDiagnostic::error(
        "merman.test.fix",
        DiagnosticCategory::Semantic,
        "test diagnostic",
    )
    .with_fix(
        DiagnosticFix::new(
            "Replace invalid text",
            vec![DiagnosticFixEdit::new(
                map.whole_source_span().unwrap(),
                "fixed",
            )],
        )
        .preferred(),
    );

    server
        .did_change(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version: 2,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "flowchart TD\nA-->B\n".to_string(),
            }],
        })
        .await;

    let actions = server
        .code_action(CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 3),
            },
            context: CodeActionContext {
                diagnostics: vec![analysis_diagnostic_to_versioned_lsp(
                    &stale_diagnostic,
                    &uri,
                    1,
                )],
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .unwrap();

    assert!(actions.is_none());
}

#[test]
fn structure_helpers_produce_hover_and_nested_symbols() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri.clone(),
        1,
        "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
    );

    let hover = hover(&snapshot, Position::new(1, 0)).unwrap();
    let text = match hover.contents {
        HoverContents::Markup(markup) => markup.value,
        other => panic!("unexpected hover contents: {other:?}"),
    };
    assert!(text.contains("group"));

    let selection_ranges = selection_ranges(&snapshot, &[Position::new(2, 0)]).unwrap();
    assert_eq!(selection_ranges.len(), 1);
    assert!(selection_ranges[0].parent.is_some());

    let markdown_uri = Url::parse("file:///tmp/example.md").unwrap();
    let markdown_snapshot = store.upsert(
        markdown_uri,
        1,
        "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n".to_string(),
    );
    let folding_ranges = folding_ranges(&markdown_snapshot);
    assert!(
        folding_ranges
            .iter()
            .any(|range| range.start_line == 1 && range.end_line == 4)
    );

    let symbols = match document_symbols(&snapshot) {
        DocumentSymbolResponse::Nested(symbols) => symbols,
        other => panic!("unexpected symbol response: {other:?}"),
    };
    assert_eq!(symbols.len(), 1);
    assert!(
        symbols[0]
            .children
            .as_ref()
            .unwrap()
            .iter()
            .any(|symbol| symbol.name == "group")
    );
}

#[test]
fn structure_helpers_cover_navigation_surface() {
    let mut store = DocumentStore::new();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri.clone(), 1, "flowchart TD\nA-->B\nA-->C\n".to_string());
    let position = Position::new(1, 0);

    assert!(matches!(
        goto_definition(&snapshot, position),
        Some(GotoDefinitionResponse::Scalar(_))
    ));
    assert_eq!(references(&snapshot, position, true).unwrap().len(), 2);
    assert!(prepare_rename(&snapshot, position).is_some());
    let rename = rename(
        &snapshot,
        RenameParams {
            text_document_position: TextDocumentPositionParams::new(
                TextDocumentIdentifier { uri },
                position,
            ),
            new_name: "X".to_string(),
            work_done_progress_params: Default::default(),
        },
    )
    .unwrap();
    let edit = rename.unwrap();
    assert!(edit.changes.is_none());
    let document_changes = match edit.document_changes.unwrap() {
        DocumentChanges::Edits(edits) => edits,
        other => panic!("unexpected document changes: {other:?}"),
    };
    assert_eq!(document_changes.len(), 1);
    assert_eq!(document_changes[0].text_document.version, Some(1));
    assert_eq!(document_changes[0].edits.len(), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_handlers_return_hover_and_symbols() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    let uri = Url::parse("file:///tmp/example.mmd").unwrap();

    {
        let mut store = server.store.lock().await;
        store.upsert(
            uri.clone(),
            1,
            "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
        );
        store.upsert(
            Url::parse("file:///tmp/example.md").unwrap(),
            1,
            "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n".to_string(),
        );
    }

    let hover = server
        .hover(HoverParams {
            text_document_position_params: TextDocumentPositionParams::new(
                TextDocumentIdentifier { uri: uri.clone() },
                Position::new(1, 0),
            ),
            work_done_progress_params: Default::default(),
        })
        .await
        .unwrap();
    assert!(hover.is_some());

    let selection_ranges = server
        .selection_range(SelectionRangeParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            positions: vec![Position::new(2, 0)],
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .unwrap()
        .expect("expected selection range response");
    assert_eq!(selection_ranges.len(), 1);
    assert!(selection_ranges[0].parent.is_some());

    let folding_ranges = server
        .folding_range(FoldingRangeParams {
            text_document: TextDocumentIdentifier {
                uri: Url::parse("file:///tmp/example.md").unwrap(),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .unwrap()
        .expect("expected folding range response");
    assert!(
        folding_ranges
            .iter()
            .any(|range| range.start_line == 1 && range.end_line == 4)
    );

    let semantic_tokens = server
        .semantic_tokens_full(SemanticTokensParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .unwrap();
    assert!(matches!(
        semantic_tokens,
        Some(SemanticTokensResult::Tokens(tokens)) if !tokens.data.is_empty()
    ));

    let semantic_tokens_range = server
        .semantic_tokens_range(SemanticTokensRangeParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range {
                start: Position::new(1, 0),
                end: Position::new(2, 7),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .unwrap();
    assert!(matches!(
        semantic_tokens_range,
        Some(SemanticTokensRangeResult::Tokens(tokens)) if !tokens.data.is_empty()
    ));

    let map = SourceMap::new("bad");
    let fix_span = map.whole_source_span().unwrap();
    let diagnostic = AnalysisDiagnostic::error(
        "merman.test.fix",
        DiagnosticCategory::Semantic,
        "test diagnostic",
    )
    .with_fix(
        DiagnosticFix::new(
            "Replace invalid text",
            vec![DiagnosticFixEdit::new(fix_span, "fixed")],
        )
        .preferred(),
    );
    let code_actions = server
        .code_action(CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 3),
            },
            context: CodeActionContext {
                diagnostics: vec![analysis_diagnostic_to_versioned_lsp(&diagnostic, &uri, 1)],
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .unwrap()
        .expect("expected code action response");
    assert_eq!(code_actions.len(), 1);
    assert!(matches!(
        &code_actions[0],
        CodeActionOrCommand::CodeAction(action)
            if action.title == "Replace invalid text"
                && action.kind == Some(CodeActionKind::QUICKFIX)
                && action.is_preferred == Some(true)
    ));

    let document_symbols = server
        .document_symbol(tower_lsp::lsp_types::DocumentSymbolParams {
            text_document: TextDocumentIdentifier { uri },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .unwrap();
    assert!(matches!(
        document_symbols,
        Some(DocumentSymbolResponse::Flat(_))
    ));

    let workspace_symbols = server
        .symbol(WorkspaceSymbolParams {
            partial_result_params: Default::default(),
            work_done_progress_params: Default::default(),
            query: "group".to_string(),
        })
        .await
        .unwrap()
        .expect("expected workspace symbol response");
    assert!(!workspace_symbols.is_empty());
    assert!(
        workspace_symbols
            .iter()
            .any(|symbol| symbol.name == "group")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn workspace_symbols_builds_all_missing_snapshots_on_first_request() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    let document_count = WORKSPACE_SYMBOL_SNAPSHOT_BATCH_SIZE * 5 + 1;
    let last_index = document_count - 1;
    let last_symbol = format!("target_{last_index:02}");
    let first_symbol = "target_00".to_string();
    let first_uri = Url::parse("file:///tmp/workspace-00.mmd").unwrap();

    {
        let mut store = server.store.lock().await;
        for index in 0..document_count {
            let uri = Url::parse(&format!("file:///tmp/workspace-{index:02}.mmd")).unwrap();
            store.upsert_text(
                uri,
                1,
                format!("flowchart TD\nsubgraph target_{index:02}\nA{index}-->B{index}\nend\n"),
                DocumentKind::Diagram,
            );
        }
    }

    let workspace_symbols = server
        .symbol(WorkspaceSymbolParams {
            partial_result_params: Default::default(),
            work_done_progress_params: Default::default(),
            query: "target_".to_string(),
        })
        .await
        .unwrap()
        .expect("expected workspace symbol response");

    assert!(
        workspace_symbols
            .iter()
            .any(|symbol| symbol.name == first_symbol && symbol.location.uri == first_uri),
        "workspace symbol request should include the first document"
    );
    assert!(
        workspace_symbols
            .iter()
            .any(|symbol| symbol.name == last_symbol),
        "workspace symbol request should include documents beyond a single snapshot batch"
    );

    let store = server.store.lock().await;
    let (_contexts, requests) = store.snapshot_build_requests();
    assert!(
        requests.is_empty(),
        "workspace symbol refresh should build every current document before responding"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_serves_rule_catalog_custom_request() {
    let (mut service, _socket) = MermanLanguageServer::service();
    let initialize = Request::build("initialize")
        .params(serde_json::to_value(InitializeParams::default()).unwrap())
        .id(1)
        .finish();

    let initialize_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap()
        .expect("initialize response");
    assert!(initialize_response.is_ok());

    let request = Request::build(RULE_CATALOG_METHOD).id(2).finish();
    let response = service
        .ready()
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap()
        .expect("rule catalog response");
    let result = response.result().expect("rule catalog result");

    assert_eq!(result["version"], RULE_CATALOG_RESPONSE_VERSION);
    assert!(result["rules"].as_array().unwrap().iter().any(|rule| {
        rule["id"] == "merman.authoring.flowchart.explicit_direction"
            && rule["origin"] == "merman_authoring"
            && rule["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == "docs/adr/0072-lint-rule-governance.md")
    }));
}

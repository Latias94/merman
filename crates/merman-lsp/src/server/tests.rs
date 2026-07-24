use std::str::FromStr;

use super::MermanLanguageServer;
use super::semantic_token_planning_error;
use super::stale_diagnostic_recompute_error;
use crate::diagnostics::analysis_diagnostic_to_versioned_lsp;
use crate::document_store::{
    DocumentDiagnosticState, DocumentStore, DocumentSyncError, StoredDocument,
    WORKSPACE_SYMBOL_SNAPSHOT_BATCH_SIZE, default_lsp_analysis_options,
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
use merman_core::EditorRenamePolicy;
use merman_editor_core::{DocumentKind, semantic_token_descriptor};
use tower::{Service, ServiceExt};
use tower_lsp_server::LanguageServer;
use tower_lsp_server::jsonrpc::Request;
use tower_lsp_server::ls_types::SemanticTokensResult;
use tower_lsp_server::ls_types::{
    CodeActionContext, CodeActionKind, CodeActionOrCommand, CodeActionParams, CompletionParams,
    DidChangeTextDocumentParams, DidOpenTextDocumentParams, DocumentChanges,
    DocumentDiagnosticParams, DocumentDiagnosticReport, DocumentDiagnosticReportResult,
    DocumentSymbolParams, DocumentSymbolResponse, FoldingRangeParams,
    FoldingRangeProviderCapability, GotoDefinitionResponse, HoverContents, HoverParams,
    InitializeParams, NumberOrString, Position, Range, RenameParams, SelectionRangeParams,
    SelectionRangeProviderCapability, SemanticTokensParams, SemanticTokensRangeParams,
    SemanticTokensRangeResult, TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentItem, TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind,
    Uri, VersionedTextDocumentIdentifier, WorkspaceSymbolParams, WorkspaceSymbolResponse,
};
use tower_lsp_server::ls_types::{HoverProviderCapability, OneOf};

async fn assert_cached_snapshot_identity(
    server: &MermanLanguageServer,
    uri: &Uri,
    expected: &std::sync::Arc<crate::snapshot::DocumentSnapshot>,
) {
    let actual = server
        .snapshot_for_uri(uri)
        .await
        .expect("expected cached snapshot");
    assert!(std::sync::Arc::ptr_eq(expected, &actual));
    assert_eq!(actual.version, expected.version);
}

#[test]
fn service_can_be_constructed_without_a_tokio_runtime() {
    let (_service, _socket) = MermanLanguageServer::service();
    let (_service, _socket) = MermanLanguageServer::service_with_refresh();
}

#[test]
fn published_server_constructor_signatures_use_tower_lsp_server_types() {
    let _: fn(tower_lsp_server::Client) -> MermanLanguageServer = MermanLanguageServer::new;
    let _: fn() -> (
        tower_lsp_server::LspService<MermanLanguageServer>,
        tower_lsp_server::ClientSocket,
    ) = MermanLanguageServer::service;
}

#[test]
fn snapshot_build_requests_keep_cached_contexts_invalidatable() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/workspace-symbols.mmd").unwrap();
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
fn semantic_token_planner_failures_are_typed_internal_errors() {
    let mut store = DocumentStore::new();
    let snapshot = store.upsert(
        Uri::from_str("file:///tmp/token-plan-error.mmd").unwrap(),
        7,
        "flowchart TD\nA --> B\n".to_string(),
    );
    let error = semantic_token_planning_error(
        &snapshot,
        merman_editor_core::TokenPlanError::PositionOverflow { value: usize::MAX },
    );

    assert_eq!(
        error.code,
        tower_lsp_server::jsonrpc::ErrorCode::InternalError
    );
    assert_eq!(error.message, "semantic token planning failed");
    assert_eq!(
        error.data,
        Some(serde_json::json!({
            "code": "merman.lsp.semantic_token_planning_failed",
            "detail": format!("token position {} exceeds the packed u32 contract", usize::MAX),
        }))
    );
}

#[test]
fn diagnostic_state_is_bound_to_document_epoch() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/diagnostic-state.mmd").unwrap();
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
    let changed_resource = AnalysisOptions::default().with_max_source_bytes(Some(1));
    let changed_date =
        AnalysisOptions::default().with_fixed_today(Some("2026-07-02".parse().unwrap()));

    for next in [changed_resource, changed_date] {
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
fn diagnostics_for_resource_limited_documents_emit_resource_limit_with_document_version() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/large.mmd").unwrap();

    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));
    let document = store.open_text(
        uri.clone(),
        5,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let analyzer = merman_analysis::Analyzer::with_options(
        AnalysisOptions::default().with_max_source_bytes(Some(8)),
    );
    let diagnostics = MermanLanguageServer::diagnostics_for_document(&document, &analyzer);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(NumberOrString::String(
            "merman.resource.source_bytes_exceeded".to_string()
        ))
    );
    assert!(
        diagnostics[0]
            .message
            .contains("exceeding max_source_bytes 8")
    );
    assert_eq!(
        diagnostics[0].range,
        Range::new(Position::new(0, 0), Position::new(0, 0))
    );
    assert_eq!(
        diagnostics[0]
            .data
            .as_ref()
            .and_then(|data| data.get("documentVersion")),
        Some(&serde_json::json!(5))
    );
}

#[test]
fn diagnostics_for_discarded_documents_request_full_resync_after_limit_increase() {
    let mut store = DocumentStore::new();
    let uri = Uri::from_str("file:///tmp/large.mmd").unwrap();

    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));
    store.open_text(
        uri.clone(),
        5,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(64)));
    let document = store
        .get(&uri)
        .expect("expected discarded document")
        .clone();
    let analyzer = merman_analysis::Analyzer::with_options(
        AnalysisOptions::default().with_max_source_bytes(Some(64)),
    );
    let diagnostics = MermanLanguageServer::diagnostics_for_document(&document, &analyzer);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(NumberOrString::String(
            "merman.resource.source_bytes_exceeded".to_string()
        ))
    );
    assert!(
        diagnostics[0]
            .message
            .contains("was discarded after exceeding previous max_source_bytes 8")
    );
    assert_eq!(
        diagnostics[0]
            .data
            .as_ref()
            .and_then(|data| data.get("documentVersion")),
        Some(&serde_json::json!(5))
    );
}

#[test]
fn diagnostics_for_unsynced_documents_request_full_replacement() {
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let document = StoredDocument {
        uri,
        version: 9,
        text: "".into(),
        kind: DocumentKind::Diagram,
        resource_limit: None,
        discarded_source: None,
        sync_error: Some(DocumentSyncError::InvalidIncrementalRange),
    };

    let diagnostics = MermanLanguageServer::diagnostics_for_document(
        &document,
        &merman_analysis::Analyzer::new(),
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(NumberOrString::String(
            "merman.lsp.document_sync_lost".to_string()
        ))
    );
    assert!(diagnostics[0].message.contains("full document replacement"));
    assert_eq!(
        diagnostics[0]
            .data
            .as_ref()
            .and_then(|data| data.get("documentVersion")),
        Some(&serde_json::json!(9))
    );
}

#[test]
fn capabilities_report_the_full_server_envelope() {
    let capabilities = MermanLanguageServer::capabilities();

    assert!(matches!(
        capabilities.text_document_sync,
        Some(TextDocumentSyncCapability::Options(ref options))
            if options.change == Some(TextDocumentSyncKind::INCREMENTAL)
                && options.open_close == Some(true)
                && options.save.is_some()
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
    assert!(capabilities.workspace_symbol_provider.is_none());
    assert!(matches!(
        capabilities.completion_provider,
        Some(ref options) if options.resolve_provider == Some(true)
            && options.trigger_characters.as_deref() == Some(&[
                " ".to_string(),
                "\n".to_string(),
                "-".to_string(),
                ">".to_string(),
                "%".to_string(),
                "[".to_string(),
                "(".to_string(),
                "{".to_string(),
                "/".to_string(),
                "\\".to_string(),
                "@".to_string(),
                ":".to_string(),
            ])
    ));
    assert!(capabilities.semantic_tokens_provider.is_some());
    assert!(capabilities.code_action_provider.is_some());
    assert_eq!(
        capabilities.experimental.as_ref().unwrap()["merman"]["requests"]["ruleCatalog"],
        RULE_CATALOG_METHOD
    );
    assert_eq!(
        capabilities.experimental.as_ref().unwrap()["merman"]["requests"]["configSchema"],
        CONFIG_SCHEMA_METHOD
    );
    let descriptor = semantic_token_descriptor();
    assert_eq!(
        capabilities.experimental.as_ref().unwrap()["merman"]["editorLanguage"],
        serde_json::json!({
            "schemaVersion": descriptor.schema_version,
            "descriptorDigest": descriptor.digest,
            "packedEncoding": descriptor.packed.encoding,
            "wordsPerToken": descriptor.packed.words_per_token,
            "renamePolicies": EditorRenamePolicy::IDS,
        })
    );
}

#[test]
fn diagnostics_use_stored_markdown_kind_for_extensionless_documents() {
    let uri = Uri::from_str("untitled:notes").unwrap();
    let document = StoredDocument {
        uri: uri.clone(),
        version: 7,
        text: "before\n```mermaid\nflowchart TD\nA[unterminated\n```\nafter\n".into(),
        kind: DocumentKind::Markdown,
        resource_limit: None,
        discarded_source: None,
        sync_error: None,
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
fn diagnostics_include_rich_editor_projection_warnings() {
    let uri = Uri::from_str("file:///tmp/cynefin.mmd").unwrap();
    let document = StoredDocument {
        uri,
        version: 3,
        text: "cynefin-beta\n  complicated --> complicated : \"Self-loop\"\n".into(),
        kind: DocumentKind::Diagram,
        resource_limit: None,
        discarded_source: None,
        sync_error: None,
    };

    let diagnostics = MermanLanguageServer::diagnostics_for_document(
        &document,
        &merman_analysis::Analyzer::new(),
    );

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("self-loop transition on domain \"complicated\" is skipped")
    }));
}

#[tokio::test(flavor = "current_thread")]
async fn did_open_diagnostics_and_editor_requests_reuse_one_snapshot() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

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

    let diagnostic_snapshot = {
        let mut store = server.store.lock().await;
        assert!(store.get(&uri).is_some());
        assert!(store.has_snapshot(&uri));
        assert!(store.has_analysis_payload(&uri));
        store.snapshot(&uri).expect("expected diagnostic snapshot")
    };

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
    let mut store = server.store.lock().await;
    assert!(store.has_snapshot(&uri));
    let editor_snapshot = store.snapshot(&uri).expect("expected cached snapshot");
    assert!(std::sync::Arc::ptr_eq(
        &diagnostic_snapshot,
        &editor_snapshot
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn r24_language_capabilities_reuse_one_analysis_snapshot_identity() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    server
        .client_profile
        .set(crate::client_profile::ClientProtocolProfile::permissive())
        .expect("test profile should initialize once");
    let uri = Uri::from_str("file:///tmp/r24-identity.mmd").unwrap();
    let version = 11;

    {
        let mut store = server.store.lock().await;
        store.apply_analyzer_options(
            default_lsp_analysis_options().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_enabled("merman.authoring.flowchart.explicit_direction"),
            ),
        );
        store.upsert_text(
            uri.clone(),
            version,
            "flowchart\nsubgraph group\nA-->B\nA-->C\nend\n".to_string(),
            DocumentKind::Diagram,
        );
        assert!(!store.has_snapshot(&uri));
    }

    let report = server
        .diagnostic(DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .expect("diagnostics should use the shared snapshot");
    let diagnostics = match report {
        DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(report)) => {
            report.full_document_diagnostic_report.items
        }
        other => panic!("unexpected diagnostic report: {other:?}"),
    };
    let direction_diagnostic = diagnostics
        .into_iter()
        .find(|diagnostic| {
            diagnostic.code
                == Some(NumberOrString::String(
                    "merman.authoring.flowchart.explicit_direction".to_string(),
                ))
        })
        .expect("expected snapshot-owned flowchart direction diagnostic");
    let shared = server
        .snapshot_for_uri(&uri)
        .await
        .expect("diagnostics should cache the shared snapshot");
    assert_eq!(shared.version, version);

    let detection = shared
        .detection()
        .expect("diagram detection should be projected by the shared snapshot");
    assert_eq!(detection.diagram_type, "flowchart");
    assert_eq!(detection.syntax_id, "flowchart-v2");
    assert_eq!(detection.effective_layout_id, "dagre");

    let completion = server
        .completion(CompletionParams {
            text_document_position: TextDocumentPositionParams::new(
                TextDocumentIdentifier { uri: uri.clone() },
                Position::new(2, 1),
            ),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        })
        .await
        .expect("completion request");
    assert!(completion.is_some());
    assert_cached_snapshot_identity(server, &uri, &shared).await;

    let hover = server
        .hover(HoverParams {
            text_document_position_params: TextDocumentPositionParams::new(
                TextDocumentIdentifier { uri: uri.clone() },
                Position::new(1, 0),
            ),
            work_done_progress_params: Default::default(),
        })
        .await
        .expect("hover request");
    assert!(hover.is_some());
    assert_cached_snapshot_identity(server, &uri, &shared).await;

    let symbols = server
        .document_symbol(DocumentSymbolParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .expect("document symbol request");
    assert!(symbols.is_some());
    assert_cached_snapshot_identity(server, &uri, &shared).await;

    let rename_position = TextDocumentPositionParams::new(
        TextDocumentIdentifier { uri: uri.clone() },
        Position::new(2, 0),
    );
    assert!(
        server
            .prepare_rename(rename_position.clone())
            .await
            .expect("prepare rename request")
            .is_some()
    );
    assert!(
        server
            .rename(RenameParams {
                text_document_position: rename_position,
                new_name: "Renamed".to_string(),
                work_done_progress_params: Default::default(),
            })
            .await
            .expect("rename request")
            .is_some()
    );
    assert_cached_snapshot_identity(server, &uri, &shared).await;

    let code_actions = server
        .code_action(CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: direction_diagnostic.range,
            context: CodeActionContext {
                diagnostics: vec![direction_diagnostic],
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .expect("code action request")
        .expect("expected snapshot-owned quick fix");
    assert!(code_actions.iter().any(|action| {
        matches!(
            action,
            CodeActionOrCommand::CodeAction(action)
                if action.title == "Insert `TB` into the flowchart header"
        )
    }));
    assert_cached_snapshot_identity(server, &uri, &shared).await;

    let tokens = server
        .semantic_tokens_full(SemanticTokensParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .expect("semantic token request");
    assert!(matches!(
        tokens,
        Some(SemanticTokensResult::Tokens(tokens)) if !tokens.data.is_empty()
    ));
    assert_cached_snapshot_identity(server, &uri, &shared).await;
}

#[tokio::test(flavor = "current_thread")]
async fn code_actions_use_current_diagnostics_after_diagnostic_only_configuration_change() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    server
        .client_profile
        .set(crate::client_profile::ClientProtocolProfile::permissive())
        .expect("test profile should initialize once");
    let uri = Uri::from_str("file:///tmp/current-diagnostic-code-action.mmd").unwrap();

    {
        let mut store = server.store.lock().await;
        store.upsert_text(
            uri.clone(),
            1,
            "flowchart\nsubgraph group\nA-->B\nend\n".to_string(),
            DocumentKind::Diagram,
        );
    }
    let original_snapshot = server
        .snapshot_for_uri(&uri)
        .await
        .expect("expected initial snapshot");

    {
        let mut store = server.store.lock().await;
        let change = store.apply_analyzer_options(
            default_lsp_analysis_options().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_enabled("merman.authoring.flowchart.explicit_direction"),
            ),
        );
        assert_eq!(
            change,
            crate::document_store::AnalyzerConfigurationChange::DiagnosticsOnly
        );
        assert!(store.has_snapshot(&uri));
        assert!(!store.has_analysis_payload(&uri));
    }

    let context = {
        let store = server.store.lock().await;
        store
            .diagnostic_context(&uri)
            .expect("expected diagnostic context")
    };
    let diagnostic = server
        .diagnostics_for_current_context(&context)
        .await
        .expect("expected current diagnostics")
        .into_iter()
        .find(|diagnostic| {
            diagnostic.code
                == Some(NumberOrString::String(
                    "merman.authoring.flowchart.explicit_direction".to_string(),
                ))
        })
        .expect("expected current flowchart direction diagnostic");

    let actions = server
        .code_action(CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: diagnostic.range,
            context: CodeActionContext {
                diagnostics: vec![diagnostic],
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .expect("code action request")
        .expect("expected a server-owned quick fix");
    assert!(actions.iter().any(|action| {
        matches!(
            action,
            CodeActionOrCommand::CodeAction(action)
                if action.title == "Insert `TB` into the flowchart header"
        )
    }));
    assert_cached_snapshot_identity(server, &uri, &original_snapshot).await;
}

#[tokio::test(flavor = "current_thread")]
async fn did_open_uses_language_id_and_change_preserves_document_kind() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    let uri = Uri::from_str("untitled:notes").unwrap();

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
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

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
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

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
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

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

    assert_eq!(
        error.code,
        tower_lsp_server::jsonrpc::ErrorCode::ContentModified
    );
    assert!(error.message.contains("diagnostic document changed"));
}

#[tokio::test(flavor = "current_thread")]
async fn stale_semantic_tokens_record_returns_content_modified_error() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

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

    assert_eq!(
        error.code,
        tower_lsp_server::jsonrpc::ErrorCode::ContentModified
    );
    assert!(error.message.contains("semantic tokens document changed"));
}

#[tokio::test(flavor = "current_thread")]
async fn stale_initial_diagnostic_context_recomputes_latest_document() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    server
        .initialize(
            serde_json::from_value(serde_json::json!({
                "capabilities": {
                    "textDocument": {
                        "publishDiagnostics": { "dataSupport": true }
                    }
                }
            }))
            .unwrap(),
        )
        .await
        .unwrap();

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
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

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
    let state = DocumentDiagnosticState {
        result_id: MermanLanguageServer::diagnostic_result_id(&[]),
        diagnostics: Vec::new(),
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

    assert_eq!(
        error.code,
        tower_lsp_server::jsonrpc::ErrorCode::ContentModified
    );
    assert!(error.message.contains("diagnostic document changed"));
}

#[tokio::test(flavor = "current_thread")]
async fn diagnostic_pull_reuses_cached_previous_result() {
    let (service, _socket) = MermanLanguageServer::service();
    let server = service.inner();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

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
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

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
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
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

    let markdown_uri = Uri::from_str("file:///tmp/example.md").unwrap();
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
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
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
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    server
        .initialize(
            serde_json::from_value(serde_json::json!({
                "capabilities": {
                    "textDocument": {
                        "codeAction": {
                            "codeActionLiteralSupport": {
                                "codeActionKind": { "valueSet": ["quickfix"] }
                            },
                            "isPreferredSupport": true
                        },
                        "publishDiagnostics": { "dataSupport": true },
                        "semanticTokens": {
                            "requests": {
                                "range": true,
                                "full": { "delta": true }
                            },
                            "tokenTypes": [
                                "namespace", "class", "struct", "variable", "property",
                                "event", "function", "string"
                            ],
                            "tokenModifiers": ["mermanEntity"],
                            "formats": ["relative"]
                        }
                    },
                    "workspace": {
                        "workspaceEdit": { "documentChanges": true }
                    }
                }
            }))
            .unwrap(),
        )
        .await
        .unwrap();

    {
        let mut store = server.store.lock().await;
        store.apply_analyzer_options(
            default_lsp_analysis_options().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_enabled("merman.authoring.flowchart.explicit_direction"),
            ),
        );
        store.upsert(
            uri.clone(),
            1,
            "flowchart\nsubgraph group\nA-->B\nend\n".to_string(),
        );
        store.upsert(
            Uri::from_str("file:///tmp/example.md").unwrap(),
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
                uri: Uri::from_str("file:///tmp/example.md").unwrap(),
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

    let context = {
        let store = server.store.lock().await;
        store
            .diagnostic_context(&uri)
            .expect("expected snapshot-backed diagnostics")
    };
    let diagnostic = server
        .diagnostics_for_current_context(&context)
        .await
        .expect("expected current diagnostics")
        .into_iter()
        .find(|diagnostic| {
            diagnostic.code
                == Some(NumberOrString::String(
                    "merman.authoring.flowchart.explicit_direction".to_string(),
                ))
        })
        .expect("expected flowchart direction diagnostic");
    let code_actions = server
        .code_action(CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: Range {
                start: Position::new(0, 0),
                end: Position::new(0, 3),
            },
            context: CodeActionContext {
                diagnostics: vec![diagnostic],
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
            if action.title == "Insert `TB` into the flowchart header"
                && action.kind == Some(CodeActionKind::QUICKFIX)
                && action.is_preferred == Some(true)
    ));

    let document_symbols = server
        .document_symbol(tower_lsp_server::ls_types::DocumentSymbolParams {
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
    let WorkspaceSymbolResponse::Flat(workspace_symbols) = workspace_symbols else {
        panic!("expected flat workspace symbol response");
    };
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
    let first_uri = Uri::from_str("file:///tmp/workspace-00.mmd").unwrap();

    {
        let mut store = server.store.lock().await;
        for index in 0..document_count {
            let uri = Uri::from_str(&format!("file:///tmp/workspace-{index:02}.mmd")).unwrap();
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
    let WorkspaceSymbolResponse::Flat(workspace_symbols) = workspace_symbols else {
        panic!("expected flat workspace symbol response");
    };

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

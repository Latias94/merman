use std::str::FromStr;

use super::MermanLanguageServer;
use super::semantic_token_planning_error;
use super::test_support::TestService;
use crate::protocol::{CONFIG_SCHEMA_METHOD, RULE_CATALOG_METHOD, RULE_CATALOG_RESPONSE_VERSION};
use crate::session::{
    CLIENT_LOG_TRUNCATION_SUFFIX, MAX_CLIENT_LOG_MESSAGE_BYTES, bounded_client_log_message,
    default_lsp_analysis_options,
};
use crate::snapshot::snapshot_for_test;
use crate::structure::{
    document_symbols, folding_ranges, goto_definition, hover, prepare_rename, references, rename,
    selection_ranges,
};
use futures::StreamExt;
use merman_analysis::AnalysisRuleConfig;
use merman_core::EditorRenamePolicy;
use merman_editor_core::DocumentKind;
use tower::{Service, ServiceExt};
use tower_lsp_server::LanguageServer;
use tower_lsp_server::jsonrpc::Request;
use tower_lsp_server::ls_types::SemanticTokensResult;
use tower_lsp_server::ls_types::{
    CodeActionContext, CodeActionKind, CodeActionOrCommand, CodeActionParams, CompletionParams,
    CompletionResponse, Diagnostic, DidChangeConfigurationParams, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    DocumentChanges, DocumentDiagnosticParams, DocumentDiagnosticReport,
    DocumentDiagnosticReportResult, DocumentSymbolResponse, FoldingRangeParams,
    FoldingRangeProviderCapability, GotoDefinitionResponse, HoverContents, HoverParams,
    InitializeParams, LogMessageParams, MessageType, NumberOrString, Position,
    PublishDiagnosticsParams, Range, RenameParams, SelectionRangeParams,
    SelectionRangeProviderCapability, SemanticTokensParams, SemanticTokensRangeParams,
    SemanticTokensRangeResult, TextDocumentContentChangeEvent, TextDocumentIdentifier,
    TextDocumentItem, TextDocumentPositionParams, TextDocumentSyncCapability, TextDocumentSyncKind,
    Uri, VersionedTextDocumentIdentifier,
};
use tower_lsp_server::ls_types::{HoverProviderCapability, OneOf};

fn test_service() -> (
    crate::MermanLspService,
    crate::MermanClientSocket,
    MermanLanguageServer,
) {
    let TestService {
        service,
        socket,
        backend,
        ..
    } = super::test_support::service();
    (service, socket, backend)
}

fn test_session_service() -> (
    crate::MermanLspService,
    crate::MermanClientSocket,
    crate::session::LanguageSession,
) {
    let TestService {
        service,
        socket,
        session,
        ..
    } = super::test_support::service();
    (service, socket, session)
}

#[test]
fn service_can_be_constructed_without_a_tokio_runtime() {
    let (_service, _socket) = MermanLanguageServer::service();
}

#[test]
fn published_server_constructor_signatures_use_tower_lsp_server_types() {
    let _: fn() -> (crate::MermanLspService, crate::MermanClientSocket) =
        MermanLanguageServer::service;
}

async fn initialize_test_service(service: &mut crate::MermanLspService) {
    let request = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "textDocument": { "diagnostic": {} }
            }
        }))
        .id(1)
        .finish();
    let response = service
        .ready()
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap()
        .expect("initialize response");
    assert!(response.is_ok());
}

async fn initialize_push_test_service(service: &mut crate::MermanLspService) {
    let request = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "textDocument": {
                    "publishDiagnostics": { "versionSupport": true }
                }
            }
        }))
        .id(1)
        .finish();
    let response = service
        .ready()
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap()
        .expect("initialize response");
    assert!(response.is_ok());
}

async fn initialize_test_backend(server: &MermanLanguageServer, capabilities: serde_json::Value) {
    let params: InitializeParams = serde_json::from_value(serde_json::json!({
        "capabilities": capabilities,
    }))
    .expect("valid initialize params");
    server.initialize(params).await.expect("initialize backend");
}

async fn initialize_pull_test_backend(server: &MermanLanguageServer) {
    initialize_test_backend(
        server,
        serde_json::json!({
            "textDocument": {
                "diagnostic": {},
                "publishDiagnostics": { "dataSupport": true }
            }
        }),
    )
    .await;
}

async fn pull_document_diagnostics(server: &MermanLanguageServer, uri: Uri) -> Vec<Diagnostic> {
    let report = server
        .diagnostic(DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier { uri },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .expect("document diagnostics should be available");

    match report {
        DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(report)) => {
            report.full_document_diagnostic_report.items
        }
        other => panic!("unexpected diagnostic report: {other:?}"),
    }
}

async fn pull_document_diagnostic_state(
    server: &MermanLanguageServer,
    uri: Uri,
) -> (String, Vec<Diagnostic>) {
    let report = server
        .diagnostic(DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier { uri },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .expect("document diagnostics should be available");

    match report {
        DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(report)) => (
            report
                .full_document_diagnostic_report
                .result_id
                .expect("open documents require a result id"),
            report.full_document_diagnostic_report.items,
        ),
        other => panic!("unexpected diagnostic report: {other:?}"),
    }
}

fn diagnostic_only_configuration() -> DidChangeConfigurationParams {
    DidChangeConfigurationParams {
        settings: serde_json::json!({
            "lint": {
                "disable_rules": ["merman.git_graph.duplicate_commit_id"]
            }
        }),
    }
}

#[test]
fn semantic_token_projection_failures_are_typed_internal_errors() {
    let uri = Uri::from_str("file:///tmp/token-plan-error.mmd").unwrap();
    let error = semantic_token_planning_error(
        &uri,
        7,
        crate::semantic_tokens::SemanticTokenError::PositionOverflow { value: usize::MAX },
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
            "detail": format!("token position {} exceeds the LSP u32 contract", usize::MAX),
        }))
    );
}

#[test]
fn invalid_semantic_token_ranges_are_invalid_params() {
    let uri = Uri::from_str("file:///tmp/token-range-error.mmd").unwrap();
    let error = semantic_token_planning_error(
        &uri,
        7,
        crate::semantic_tokens::SemanticTokenError::InvalidRange(
            "semantic token range start 1:4 is after end 1:2".to_owned(),
        ),
    );

    assert_eq!(
        error.code,
        tower_lsp_server::jsonrpc::ErrorCode::InvalidParams
    );
    assert_eq!(
        error.message,
        "semantic token range start 1:4 is after end 1:2"
    );
    assert!(error.data.is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn pull_diagnostics_follow_discarded_source_lifecycle() {
    let (_service, _socket, server) = test_service();
    initialize_pull_test_backend(&server).await;
    let uri = Uri::from_str("file:///tmp/large.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n";

    server
        .did_change_configuration(DidChangeConfigurationParams {
            settings: serde_json::json!({
                "resources": { "limits": { "max_source_bytes": 8 } }
            }),
        })
        .await;
    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "mermaid".to_string(),
                version: 5,
                text: source.to_string(),
            },
        })
        .await;

    let (limited_result_id, diagnostics) =
        pull_document_diagnostic_state(&server, uri.clone()).await;

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
        Range::new(Position::new(0, 0), Position::new(2, 0))
    );
    assert_eq!(
        diagnostics[0]
            .data
            .as_ref()
            .and_then(|data| data.get("documentVersion")),
        Some(&serde_json::json!(5))
    );
    assert!(
        diagnostics[0]
            .data
            .as_ref()
            .and_then(|data| data.get("id"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|id| id.starts_with("u2:"))
    );

    server
        .did_change_configuration(DidChangeConfigurationParams {
            settings: serde_json::json!({
                "resources": { "limits": { "max_source_bytes": 64 } }
            }),
        })
        .await;
    let (discarded_result_id, diagnostics) =
        pull_document_diagnostic_state(&server, uri.clone()).await;

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
    assert_ne!(limited_result_id, discarded_result_id);
    assert!(
        diagnostics[0]
            .data
            .as_ref()
            .and_then(|data| data.get("id"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|id| id.starts_with("u2:"))
    );

    server
        .did_change(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version: 6,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(1, 0), Position::new(1, 1))),
                range_length: None,
                text: "C".to_string(),
            }],
        })
        .await;
    let diagnostics = pull_document_diagnostics(&server, uri).await;

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(NumberOrString::String(
            "merman.lsp.document_sync_lost".to_string()
        ))
    );
    assert!(
        diagnostics[0]
            .message
            .contains(&format!("{}-byte source", source.len()))
    );
    assert!(diagnostics[0].message.contains("8-byte limit"));
    assert!(diagnostics[0].message.contains("full document replacement"));
    assert_eq!(
        diagnostics[0]
            .data
            .as_ref()
            .and_then(|data| data.get("documentVersion")),
        Some(&serde_json::json!(6))
    );
    assert_eq!(
        diagnostics[0]
            .data
            .as_ref()
            .and_then(serde_json::Value::as_object)
            .map(serde_json::Map::len),
        Some(1)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn unavailable_pull_result_id_rejects_close_reopen_aba_at_the_same_client_version() {
    let (_service, _socket, server) = test_service();
    initialize_pull_test_backend(&server).await;
    let uri = Uri::from_str("file:///tmp/unavailable-aba.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n";
    server
        .did_change_configuration(DidChangeConfigurationParams {
            settings: serde_json::json!({
                "resources": { "limits": { "max_source_bytes": 8 } }
            }),
        })
        .await;
    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "mermaid".to_string(),
                version: 5,
                text: source.to_string(),
            },
        })
        .await;

    let (first_result_id, first_diagnostics) =
        pull_document_diagnostic_state(&server, uri.clone()).await;
    let first_id = first_diagnostics[0].data.as_ref().unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    server.session.close_document(&uri).await;
    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "mermaid".to_string(),
                version: 5,
                text: source.to_string(),
            },
        })
        .await;
    let (second_result_id, second_diagnostics) =
        pull_document_diagnostic_state(&server, uri.clone()).await;
    let second_id = second_diagnostics[0].data.as_ref().unwrap()["id"]
        .as_str()
        .unwrap();

    assert_ne!(first_result_id, second_result_id);
    assert_ne!(first_id, second_id);
    let report = server
        .diagnostic(DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier { uri },
            identifier: None,
            previous_result_id: Some(first_result_id),
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .unwrap();
    assert!(matches!(
        report,
        DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(_))
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn pull_diagnostics_for_document_diagram_rejection_use_canonical_resource_payload() {
    let (_service, _socket, server) = test_service();
    initialize_pull_test_backend(&server).await;
    let uri = Uri::from_str("file:///tmp/limited.md").unwrap();
    let source = concat!(
        "```mermaid\nflowchart TD\nA-->B\n```\n",
        "```mermaid\nsequenceDiagram\nA->>B: hi\n```\n",
    );

    server
        .did_change_configuration(DidChangeConfigurationParams {
            settings: serde_json::json!({
                "resources": { "limits": { "max_document_diagrams": 1 } }
            }),
        })
        .await;
    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "markdown".to_string(),
                version: 5,
                text: source.to_string(),
            },
        })
        .await;

    let (_, diagnostics) = pull_document_diagnostic_state(&server, uri).await;

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code,
        Some(NumberOrString::String(
            "merman.resource.document_diagrams_exceeded".to_string()
        ))
    );
    assert_eq!(
        diagnostics[0].range,
        Range::new(Position::new(4, 0), Position::new(4, 3))
    );
    assert!(!diagnostics[0].message.contains("document_sync_lost"));
    assert_eq!(
        diagnostics[0]
            .data
            .as_ref()
            .and_then(|data| data.get("documentVersion")),
        Some(&serde_json::json!(5))
    );
    assert!(
        diagnostics[0]
            .data
            .as_ref()
            .and_then(|data| data.get("id"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|id| id.starts_with("u2:"))
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
    assert_eq!(
        capabilities.experimental.as_ref().unwrap()["merman"]["editorLanguage"],
        serde_json::json!({
            "renamePolicies": EditorRenamePolicy::IDS,
        })
    );
}

#[tokio::test(flavor = "current_thread")]
async fn configuration_side_effects_follow_the_changed_policy_scope() {
    let (_service, _socket, server) = test_service();
    let server = &server;
    initialize_test_backend(
        server,
        serde_json::json!({
            "textDocument": {
                "semanticTokens": {
                    "requests": { "full": true },
                    "tokenTypes": ["keyword"],
                    "tokenModifiers": [],
                    "formats": ["relative"]
                }
            },
            "workspace": {
                "semanticTokens": { "refreshSupport": true }
            }
        }),
    )
    .await;
    let uri = Uri::from_str("file:///tmp/configuration-effects.mmd").unwrap();

    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri,
                language_id: "mermaid".to_string(),
                version: 1,
                text: "gitGraph\ncommit id:\"dup\"\ncommit id:\"dup\"\n".to_string(),
            },
        })
        .await;
    assert_eq!(server.client_effects.admission_count(), 1);

    server
        .did_change_configuration(DidChangeConfigurationParams {
            settings: serde_json::Value::Null,
        })
        .await;
    assert_eq!(server.client_effects.admission_count(), 1);
    assert_eq!(server.client_effects.refresh_request_counts(), (0, 0));

    server
        .did_change_configuration(diagnostic_only_configuration())
        .await;
    assert_eq!(server.client_effects.admission_count(), 2);
    assert_eq!(
        server.client_effects.refresh_request_counts(),
        (0, 0),
        "diagnostic-only changes must not refresh semantic tokens"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn configuration_refreshes_are_scheduled_before_diagnostic_republish_backpressure() {
    let (_service, _socket, server) = test_service();
    initialize_test_backend(
        &server,
        serde_json::json!({
            "textDocument": {
                "semanticTokens": {
                    "requests": { "full": true },
                    "tokenTypes": ["keyword"],
                    "tokenModifiers": [],
                    "formats": ["relative"]
                }
            },
            "workspace": {
                "semanticTokens": { "refreshSupport": true }
            }
        }),
    )
    .await;
    let uri = Uri::from_str("file:///tmp/configuration-backpressure.mmd").unwrap();
    assert!(
        server
            .session
            .open_document(
                uri,
                1,
                "flowchart TD\nA-->B\n".to_string(),
                DocumentKind::Diagram,
            )
            .await
    );
    let release = server.client_effects.saturate_serial_lane_for_test().await;
    let change = server
        .session
        .update_configuration(default_lsp_analysis_options().with_max_source_bytes(Some(1024)))
        .await;

    let mut effects = Box::pin(server.client_effects.configuration_changed(change));
    assert!(futures::poll!(&mut effects).is_pending());
    assert_eq!(
        server.client_effects.refresh_request_counts(),
        (1, 0),
        "snapshot refresh must be admitted before the saturated diagnostic publisher"
    );

    release.send(()).unwrap();
    effects.await;
    server.client_effects.wait_idle().await;
}

#[tokio::test(flavor = "current_thread")]
async fn pull_diagnostic_effects_require_negotiated_refresh_support() {
    for (refresh_support, expected_refreshes) in [(false, 0), (true, 1)] {
        let (_service, _socket, server) = test_service();
        let server = &server;
        initialize_test_backend(
            server,
            serde_json::json!({
                "textDocument": { "diagnostic": {} },
                "workspace": {
                    "diagnostics": { "refreshSupport": refresh_support }
                }
            }),
        )
        .await;
        let uri = Uri::from_str("file:///tmp/pull-diagnostic-effects.mmd").unwrap();

        server
            .did_open(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri,
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: "gitGraph\ncommit id:\"dup\"\ncommit id:\"dup\"\n".to_string(),
                },
            })
            .await;
        server
            .did_change_configuration(diagnostic_only_configuration())
            .await;

        assert_eq!(
            server.client_effects.admission_count(),
            0,
            "pull diagnostics must not enqueue no-op push effects"
        );
        assert_eq!(
            server.client_effects.refresh_request_counts(),
            (0, expected_refreshes)
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn push_diagnostics_admit_exactly_one_effect_per_document_event() {
    let (_service, _socket, server) = test_service();
    let server = &server;
    initialize_test_backend(server, serde_json::json!({})).await;
    let uri = Uri::from_str("file:///tmp/push-effect-count.mmd").unwrap();

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
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "flowchart LR\nA-->B\n".to_string(),
            }],
        })
        .await;
    server
        .did_save(DidSaveTextDocumentParams {
            text_document: TextDocumentIdentifier { uri },
            text: None,
        })
        .await;

    assert_eq!(server.client_effects.admission_count(), 3);
}

#[tokio::test(flavor = "current_thread")]
async fn queued_diagnostic_sync_clears_a_document_closed_before_execution() {
    let (mut service, socket, server) = test_service();
    let (mut socket, _responses) = socket.split();
    initialize_push_test_service(&mut service).await;
    let server = &server;
    let uri = Uri::from_str("file:///tmp/queued-diagnostic-close.mmd").unwrap();
    let release = server.client_effects.block_serial_lane_for_test().await;

    assert!(
        server
            .session
            .open_document(uri.clone(), 1, String::new(), DocumentKind::Diagram,)
            .await
    );
    server.client_effects.push_diagnostics(uri.clone()).await;
    server.session.close_document(&uri).await;

    release.send(()).expect("client effect gate should be open");
    let notification = socket.next().await.expect("expected diagnostic clear");
    server.client_effects.wait_idle().await;
    assert_eq!(notification.method(), "textDocument/publishDiagnostics");
    let params: PublishDiagnosticsParams = serde_json::from_value(
        notification
            .params()
            .cloned()
            .expect("diagnostic clear params"),
    )
    .expect("valid diagnostic clear params");
    assert_eq!(params.uri, uri);
    assert_eq!(params.version, None);
    assert!(params.diagnostics.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn queued_diagnostic_sync_publishes_a_document_opened_before_execution() {
    let (mut service, socket, server) = test_service();
    let (mut socket, _responses) = socket.split();
    initialize_push_test_service(&mut service).await;
    let server = &server;
    let uri = Uri::from_str("file:///tmp/queued-diagnostic-open.mmd").unwrap();
    let release = server.client_effects.block_serial_lane_for_test().await;

    server.client_effects.push_diagnostics(uri.clone()).await;
    assert!(
        server
            .session
            .open_document(uri.clone(), 7, String::new(), DocumentKind::Diagram,)
            .await
    );

    release.send(()).expect("client effect gate should be open");
    let notification = socket
        .next()
        .await
        .expect("expected current diagnostic publish");
    server.client_effects.wait_idle().await;
    assert_eq!(notification.method(), "textDocument/publishDiagnostics");
    let params: PublishDiagnosticsParams = serde_json::from_value(
        notification
            .params()
            .cloned()
            .expect("diagnostic publish params"),
    )
    .expect("valid diagnostic publish params");
    assert_eq!(params.uri, uri);
    assert_eq!(params.version, Some(7));
    assert!(!params.diagnostics.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn edited_document_cancels_stale_diagnostics_before_client_admission() {
    let (mut service, socket, server) = test_service();
    let (mut socket, _responses) = socket.split();
    initialize_push_test_service(&mut service).await;
    let uri = Uri::from_str("file:///tmp/active-diagnostic-edit.mmd").unwrap();
    let (pre_admission_started, release_stale_pre_admission) = server
        .client_effects
        .block_next_diagnostic_pre_admission_for_test();

    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "mermaid".to_string(),
                version: 1,
                text: String::new(),
            },
        })
        .await;
    pre_admission_started
        .await
        .expect("stale diagnostic publish should reach the pre-admission gate");

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

    let notification = socket
        .next()
        .await
        .expect("expected the edited document diagnostics");
    server.client_effects.wait_idle().await;
    assert!(
        release_stale_pre_admission.send(()).is_err(),
        "the superseded pre-admission future must already be dropped"
    );
    assert_eq!(notification.method(), "textDocument/publishDiagnostics");
    let params: PublishDiagnosticsParams = serde_json::from_value(
        notification
            .params()
            .cloned()
            .expect("diagnostic publish params"),
    )
    .expect("valid diagnostic publish params");
    assert_eq!(params.uri, uri);
    assert_eq!(params.version, Some(2));
    let mut unexpected = Box::pin(socket.next());
    assert!(
        futures::poll!(&mut unexpected).is_pending(),
        "the stale version-one publish must never reach the client socket"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn reopen_cancels_stale_clear_before_client_admission_at_the_same_version() {
    let (mut service, socket, server) = test_service();
    let (mut socket, _responses) = socket.split();
    initialize_push_test_service(&mut service).await;
    let uri = Uri::from_str("file:///tmp/active-diagnostic-reopen.mmd").unwrap();
    let source = "gitGraph\ncommit id:\"dup\"\ncommit id:\"dup\"\n";

    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "mermaid".to_string(),
                version: 5,
                text: source.to_string(),
            },
        })
        .await;
    let initial = socket
        .next()
        .await
        .expect("expected initial diagnostic publish");
    assert_eq!(initial.method(), "textDocument/publishDiagnostics");
    server.client_effects.wait_idle().await;

    let (pre_admission_started, release_stale_pre_admission) = server
        .client_effects
        .block_next_diagnostic_pre_admission_for_test();
    server
        .did_close(DidCloseTextDocumentParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
        })
        .await;
    pre_admission_started
        .await
        .expect("stale diagnostic clear should reach the pre-admission gate");

    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "mermaid".to_string(),
                version: 5,
                text: source.to_string(),
            },
        })
        .await;

    let notification = socket
        .next()
        .await
        .expect("expected reopened document diagnostics");
    server.client_effects.wait_idle().await;
    assert!(
        release_stale_pre_admission.send(()).is_err(),
        "the superseded pre-admission clear future must already be dropped"
    );
    assert_eq!(notification.method(), "textDocument/publishDiagnostics");
    let params: PublishDiagnosticsParams = serde_json::from_value(
        notification
            .params()
            .cloned()
            .expect("diagnostic publish params"),
    )
    .expect("valid diagnostic publish params");
    assert_eq!(params.uri, uri);
    assert_eq!(params.version, Some(5));
    assert!(!params.diagnostics.is_empty());
    let mut unexpected = Box::pin(socket.next());
    assert!(
        futures::poll!(&mut unexpected).is_pending(),
        "the pre-reopen clear must never reach the client socket"
    );
}

#[test]
fn client_log_messages_have_a_bounded_utf8_allocation() {
    let oversized = "界".repeat(MAX_CLIENT_LOG_MESSAGE_BYTES);
    let bounded = bounded_client_log_message(oversized);

    assert!(bounded.len() <= MAX_CLIENT_LOG_MESSAGE_BYTES);
    assert!(bounded.capacity() <= MAX_CLIENT_LOG_MESSAGE_BYTES);
    assert!(bounded.ends_with(CLIENT_LOG_TRUNCATION_SUFFIX));

    let mut oversized_capacity = String::with_capacity(MAX_CLIENT_LOG_MESSAGE_BYTES * 2);
    oversized_capacity.push_str("small");
    let bounded = bounded_client_log_message(oversized_capacity);
    assert_eq!(bounded, "small");
    assert!(bounded.capacity() <= MAX_CLIENT_LOG_MESSAGE_BYTES);
}

#[tokio::test(flavor = "current_thread")]
async fn stalled_configuration_error_log_is_bounded() {
    let (_service, socket, server) = test_service();
    let (mut socket, _responses) = socket.split();
    let server = &server;
    initialize_test_backend(server, serde_json::json!({})).await;
    let release = server.client_effects.block_serial_lane_for_test().await;
    let mut resources = serde_json::Map::new();
    resources.insert(
        "界".repeat(MAX_CLIENT_LOG_MESSAGE_BYTES),
        serde_json::Value::from(1),
    );

    server
        .did_change_configuration(DidChangeConfigurationParams {
            settings: serde_json::json!({
                "resources": serde_json::Value::Object(resources)
            }),
        })
        .await;

    release.send(()).expect("client effect gate should be open");
    let notification = socket
        .next()
        .await
        .expect("expected bounded configuration error log");
    server.client_effects.wait_idle().await;
    assert_eq!(notification.method(), "window/logMessage");
    let params: LogMessageParams =
        serde_json::from_value(notification.params().cloned().expect("client log params"))
            .expect("valid client log params");
    assert_eq!(params.typ, MessageType::ERROR);
    assert!(params.message.len() <= MAX_CLIENT_LOG_MESSAGE_BYTES);
    assert!(params.message.ends_with(CLIENT_LOG_TRUNCATION_SUFFIX));
}

#[tokio::test(flavor = "current_thread")]
async fn stalled_client_logs_are_latest_wins() {
    let (_service, socket, server) = test_service();
    let (mut socket, _responses) = socket.split();
    let server = &server;
    let release = server.client_effects.block_serial_lane_for_test().await;

    server
        .client_effects
        .log_message(MessageType::INFO, "obsolete client log")
        .await;
    server
        .client_effects
        .log_message(MessageType::ERROR, "latest client log")
        .await;

    release.send(()).expect("client effect gate should be open");
    let notification = socket.next().await.expect("expected latest client log");
    server.client_effects.wait_idle().await;
    assert_eq!(notification.method(), "window/logMessage");
    let params: LogMessageParams =
        serde_json::from_value(notification.params().cloned().expect("client log params"))
            .expect("valid client log params");
    assert_eq!(params.typ, MessageType::ERROR);
    assert_eq!(params.message, "latest client log");
}

#[tokio::test(flavor = "current_thread")]
async fn diagnostics_use_stored_markdown_kind_for_extensionless_documents() {
    let (_service, _socket, server) = test_service();
    initialize_pull_test_backend(&server).await;
    let uri = Uri::from_str("untitled:notes").unwrap();
    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "markdown".to_string(),
                version: 7,
                text: "before\n```mermaid\nflowchart TD\nA[unterminated\n```\nafter\n".to_string(),
            },
        })
        .await;
    let diagnostics = pull_document_diagnostics(&server, uri).await;

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

#[tokio::test(flavor = "current_thread")]
async fn diagnostics_include_rich_editor_projection_warnings() {
    let (_service, _socket, server) = test_service();
    initialize_pull_test_backend(&server).await;
    let uri = Uri::from_str("file:///tmp/cynefin.mmd").unwrap();
    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "mermaid".to_string(),
                version: 3,
                text: "cynefin-beta\n  complicated --> complicated : \"Self-loop\"\n".to_string(),
            },
        })
        .await;
    let diagnostics = pull_document_diagnostics(&server, uri).await;

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("self-loop transition on domain \"complicated\" is skipped")
    }));
}

#[tokio::test(flavor = "current_thread")]
async fn code_actions_use_current_diagnostics_after_diagnostic_only_configuration_change() {
    let (_service, _socket, server) = test_service();
    let server = &server;
    server
        .client_profile
        .set(crate::client_profile::ClientProtocolProfile::permissive())
        .expect("test profile should initialize once");
    let uri = Uri::from_str("file:///tmp/current-diagnostic-code-action.mmd").unwrap();

    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "mermaid".to_string(),
                version: 1,
                text: "flowchart\nsubgraph group\nA-->B\nend\n".to_string(),
            },
        })
        .await;
    assert!(
        pull_document_diagnostics(server, uri.clone())
            .await
            .iter()
            .all(|diagnostic| {
                diagnostic.code
                    != Some(NumberOrString::String(
                        "merman.authoring.flowchart.explicit_direction".to_string(),
                    ))
            })
    );

    server
        .did_change_configuration(DidChangeConfigurationParams {
            settings: serde_json::json!({
                "lint": {
                    "enable_rules": ["merman.authoring.flowchart.explicit_direction"]
                }
            }),
        })
        .await;
    let diagnostic = pull_document_diagnostics(server, uri.clone())
        .await
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
}

#[tokio::test(flavor = "current_thread")]
async fn did_open_uses_language_id_and_change_preserves_document_kind() {
    let (_service, _socket, server) = test_service();
    let server = &server;
    let uri = Uri::from_str("untitled:notes").unwrap();
    let initial = "```mermaid\nflowchart TD\nA-->B\n```\n";
    let changed = "```mermaid\nsequenceDiagram\nAlice->>Bob: Hi\n```\n";

    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "markdown".to_string(),
                version: 1,
                text: initial.to_string(),
            },
        })
        .await;

    let stored = server
        .session
        .probe()
        .document(&uri)
        .await
        .expect("expected opened Markdown document");
    assert_eq!(stored.version, 1);
    assert_eq!(stored.kind, DocumentKind::Markdown);
    assert_eq!(stored.retained_text().unwrap().as_ref(), initial);

    server
        .did_change(DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version: 2,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: changed.to_string(),
            }],
        })
        .await;

    let stored = server
        .session
        .probe()
        .document(&uri)
        .await
        .expect("expected changed Markdown document");
    assert_eq!(stored.version, 2);
    assert_eq!(stored.kind, DocumentKind::Markdown);
    assert_eq!(stored.retained_text().unwrap().as_ref(), changed);
}

#[tokio::test(flavor = "current_thread")]
async fn did_change_rejects_stale_document_versions() {
    let (_service, _socket, server) = test_service();
    let server = &server;
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

    let stored = server
        .session
        .probe()
        .document(&uri)
        .await
        .expect("expected stored document");
    assert_eq!(stored.version, 3);
    assert_eq!(stored.kind, DocumentKind::Diagram);
    assert_eq!(
        stored.retained_text().unwrap().as_ref(),
        "sequenceDiagram\nAlice->>Bob: Hi\n"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn did_change_applies_incremental_changes_in_order() {
    let (_service, _socket, server) = test_service();
    let server = &server;
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

    let stored = server
        .session
        .probe()
        .document(&uri)
        .await
        .expect("expected stored document");
    assert_eq!(stored.version, 2);
    assert_eq!(
        stored.retained_text().unwrap().as_ref(),
        "flowchart TD\nA-->C\nC-->D\n"
    );
    assert_eq!(stored.kind, DocumentKind::Diagram);
}

#[tokio::test(flavor = "current_thread")]
async fn session_routes_pipelined_messages_after_initialize_completes() {
    let (mut service, _socket, session) = test_session_service();
    let uri = Uri::from_str("file:///tmp/pipelined-initialize.mmd").unwrap();

    let initialize = service.call(
        Request::build("initialize")
            .params(serde_json::to_value(InitializeParams::default()).unwrap())
            .id(1)
            .finish(),
    );
    let open = service.call(
        Request::build("textDocument/didOpen")
            .params(
                serde_json::to_value(DidOpenTextDocumentParams {
                    text_document: TextDocumentItem {
                        uri: uri.clone(),
                        language_id: "mermaid".to_string(),
                        version: 1,
                        text: "flowchart TD\nA-->B\n".to_string(),
                    },
                })
                .unwrap(),
            )
            .finish(),
    );

    let (initialize, open) = tokio::join!(initialize, open);
    assert!(initialize.unwrap().expect("initialize response").is_ok());
    assert!(open.unwrap().is_none());
    assert!(
        session.probe().document(&uri).await.is_some(),
        "didOpen must be routed after the earlier initialize succeeds"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn dropping_either_session_endpoint_terminates_all_work_exactly_once() {
    let (service, socket, socket_session) = test_session_service();

    drop(socket);
    socket_session.wait_stopped().await;
    assert!(socket_session.analysis_is_cancelled());
    assert_eq!(socket_session.termination_count(), 1);

    drop(service);
    assert_eq!(socket_session.termination_count(), 1);

    let (service, socket, service_session) = test_session_service();

    drop(service);
    service_session.wait_stopped().await;
    assert!(service_session.analysis_is_cancelled());
    assert_eq!(service_session.termination_count(), 1);

    drop(socket);
    assert_eq!(service_session.termination_count(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn exit_cancels_queued_mutations_and_stops_new_admission() {
    let (mut service, socket, session) = test_session_service();
    let uri = Uri::from_str("file:///tmp/exit-cancels-queued-open.mmd").unwrap();

    let open = service.call(
        Request::build("textDocument/didOpen")
            .params(
                serde_json::to_value(DidOpenTextDocumentParams {
                    text_document: TextDocumentItem {
                        uri: uri.clone(),
                        language_id: "mermaid".to_string(),
                        version: 1,
                        text: "flowchart TD\nA-->B\n".to_string(),
                    },
                })
                .unwrap(),
            )
            .finish(),
    );
    let exit = service.call(Request::build("exit").finish());

    assert!(exit.await.unwrap().is_none());
    assert!(open.await.unwrap().is_none());
    assert!(session.probe().document(&uri).await.is_none());
    assert!(
        service
            .call(Request::build(RULE_CATALOG_METHOD).id(2).finish())
            .await
            .unwrap()
            .is_none(),
        "terminated sessions must not admit new work"
    );
    assert_eq!(session.termination_count(), 1);

    drop(socket);
    drop(service);
    assert_eq!(session.termination_count(), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn exit_preserves_an_already_admitted_shutdown_error() {
    let (mut service, socket) = MermanLanguageServer::service();

    let shutdown = service.call(Request::build("shutdown").id(1).finish());
    let exit = service.call(Request::build("exit").finish());

    assert!(exit.await.unwrap().is_none());
    let response = shutdown
        .await
        .unwrap()
        .expect("the rejected shutdown should retain its response");
    assert!(response.error().is_some());

    drop(socket);
    drop(service);
}

#[tokio::test(flavor = "current_thread")]
async fn client_effect_backpressure_does_not_hold_the_mutation_fence() {
    let TestService {
        mut service,
        socket: _socket,
        backend,
        session,
        ..
    } = super::test_support::service();
    let initialize = Request::build("initialize")
        .params(serde_json::json!({ "capabilities": {} }))
        .id(1)
        .finish();
    assert!(
        service
            .ready()
            .await
            .unwrap()
            .call(initialize)
            .await
            .unwrap()
            .is_some_and(|response| response.is_ok())
    );
    let uri = Uri::from_str("file:///tmp/effect-backpressure.mmd").unwrap();
    let _release = backend.client_effects.saturate_serial_lane_for_test().await;

    let save = service.call(
        Request::build("textDocument/didSave")
            .params(
                serde_json::to_value(DidSaveTextDocumentParams {
                    text_document: TextDocumentIdentifier { uri },
                    text: None,
                })
                .unwrap(),
            )
            .finish(),
    );
    tokio::pin!(save);
    assert!(futures::poll!(&mut save).is_pending());

    let response = service
        .call(Request::build(RULE_CATALOG_METHOD).id(2).finish())
        .await
        .unwrap()
        .expect("a read after the committed save should not wait for client capacity");
    assert!(response.is_ok());

    session.terminate();
    assert!(save.await.unwrap().is_none());
    session.wait_stopped().await;
}

#[tokio::test(flavor = "current_thread")]
async fn client_log_backpressure_does_not_hold_mutation_fences() {
    let cases = [
        ("initialized", serde_json::json!({})),
        (
            "workspace/didChangeConfiguration",
            serde_json::json!({
                "settings": {
                    "lint": {
                        "rule_severities": [{
                            "rule_id": "merman.resource.source_bytes_exceeded",
                            "severity": "hint"
                        }]
                    }
                }
            }),
        ),
    ];

    for (method, params) in cases {
        let TestService {
            mut service,
            socket: _socket,
            backend,
            session,
            ..
        } = super::test_support::service();
        initialize_test_service(&mut service).await;
        let _release = backend.client_effects.saturate_serial_lane_for_test().await;

        let notification = service.call(Request::build(method).params(params).finish());
        tokio::pin!(notification);
        assert!(futures::poll!(&mut notification).is_pending());

        let response = service
            .call(Request::build(RULE_CATALOG_METHOD).id(2).finish())
            .await
            .unwrap()
            .expect("a read should not wait for client log capacity");
        assert!(response.is_ok());

        session.terminate();
        assert!(notification.await.unwrap().is_none());
        session.wait_stopped().await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn reads_wait_for_an_earlier_unpolled_shutdown() {
    let (mut service, _socket) = MermanLanguageServer::service();
    initialize_test_service(&mut service).await;

    let shutdown = service.call(Request::build("shutdown").id(2).finish());
    let rule_catalog = service.call(Request::build(RULE_CATALOG_METHOD).id(3).finish());
    tokio::pin!(rule_catalog);
    assert!(futures::poll!(&mut rule_catalog).is_pending());

    let shutdown = shutdown
        .await
        .unwrap()
        .expect("shutdown response after initialize");
    assert!(shutdown.is_ok());
    let rule_catalog = rule_catalog
        .await
        .unwrap()
        .expect("post-shutdown request response");
    assert!(rule_catalog.is_error());
}

#[tokio::test(flavor = "current_thread")]
async fn session_admission_preserves_open_change_and_close_order() {
    let (mut service, _socket, session) = test_session_service();
    initialize_test_service(&mut service).await;
    let uri = Uri::from_str("file:///tmp/ordered-sync.mmd").unwrap();

    let open = service.call(
        Request::build("textDocument/didOpen")
            .params(
                serde_json::to_value(DidOpenTextDocumentParams {
                    text_document: TextDocumentItem {
                        uri: uri.clone(),
                        language_id: "mermaid".to_string(),
                        version: 1,
                        text: "flowchart TD\nA-->B\n".to_string(),
                    },
                })
                .unwrap(),
            )
            .finish(),
    );
    let change = service.call(
        Request::build("textDocument/didChange")
            .params(
                serde_json::to_value(DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: uri.clone(),
                        version: 2,
                    },
                    content_changes: vec![TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: "flowchart TD\nA-->C\n".to_string(),
                    }],
                })
                .unwrap(),
            )
            .finish(),
    );
    let (open, change) = tokio::join!(open, change);
    assert!(open.unwrap().is_none());
    assert!(change.unwrap().is_none());

    let document = session
        .probe()
        .document(&uri)
        .await
        .expect("change must follow open");
    assert_eq!(document.version, 2);
    assert_eq!(
        document.retained_text().unwrap().as_ref(),
        "flowchart TD\nA-->C\n"
    );

    let reopen = service.call(
        Request::build("textDocument/didOpen")
            .params(
                serde_json::to_value(DidOpenTextDocumentParams {
                    text_document: TextDocumentItem {
                        uri: uri.clone(),
                        language_id: "mermaid".to_string(),
                        version: 3,
                        text: "flowchart TD\nA-->D\n".to_string(),
                    },
                })
                .unwrap(),
            )
            .finish(),
    );
    let close = service.call(
        Request::build("textDocument/didClose")
            .params(
                serde_json::to_value(DidCloseTextDocumentParams {
                    text_document: TextDocumentIdentifier { uri: uri.clone() },
                })
                .unwrap(),
            )
            .finish(),
    );
    let (reopen, close) = tokio::join!(reopen, close);
    assert!(reopen.unwrap().is_none());
    assert!(close.unwrap().is_none());

    assert!(
        session.probe().document(&uri).await.is_none(),
        "close must follow the queued reopen"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn session_admission_orders_configuration_before_document_open() {
    let (mut service, _socket, session) = test_session_service();
    initialize_test_service(&mut service).await;
    let uri = Uri::from_str("file:///tmp/configuration-before-open.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n";

    session
        .update_configuration(
            default_lsp_analysis_options().with_max_source_bytes(Some(source.len() - 1)),
        )
        .await;

    let configuration = service.call(
        Request::build("workspace/didChangeConfiguration")
            .params(
                serde_json::to_value(DidChangeConfigurationParams {
                    settings: serde_json::Value::Null,
                })
                .unwrap(),
            )
            .finish(),
    );
    let open = service.call(
        Request::build("textDocument/didOpen")
            .params(
                serde_json::to_value(DidOpenTextDocumentParams {
                    text_document: TextDocumentItem {
                        uri: uri.clone(),
                        language_id: "mermaid".to_string(),
                        version: 1,
                        text: source.to_string(),
                    },
                })
                .unwrap(),
            )
            .finish(),
    );
    let (configuration, open) = tokio::join!(configuration, open);
    assert!(configuration.unwrap().is_none());
    assert!(open.unwrap().is_none());

    let document = session
        .probe()
        .document(&uri)
        .await
        .expect("open must run after the queued configuration");
    assert_eq!(document.version, 1);
    assert_eq!(document.retained_text().unwrap().as_ref(), source);
    assert!(!document.is_analysis_unavailable());
}

#[tokio::test(flavor = "current_thread")]
async fn read_requests_wait_for_earlier_unpolled_mutations() {
    let (mut service, _socket) = MermanLanguageServer::service();
    initialize_test_service(&mut service).await;
    let uri = Uri::from_str("file:///tmp/read-after-open.mmd").unwrap();

    let open = service.call(
        Request::build("textDocument/didOpen")
            .params(
                serde_json::to_value(DidOpenTextDocumentParams {
                    text_document: TextDocumentItem {
                        uri: uri.clone(),
                        language_id: "mermaid".to_string(),
                        version: 1,
                        text: "flowchart TD\nA[opened]-->B\n".to_string(),
                    },
                })
                .unwrap(),
            )
            .finish(),
    );
    let hover = service.call(
        Request::build("textDocument/hover")
            .params(
                serde_json::to_value(HoverParams {
                    text_document_position_params: TextDocumentPositionParams::new(
                        TextDocumentIdentifier { uri: uri.clone() },
                        Position::new(1, 2),
                    ),
                    work_done_progress_params: Default::default(),
                })
                .unwrap(),
            )
            .id(2)
            .finish(),
    );
    tokio::pin!(hover);
    assert!(futures::poll!(&mut hover).is_pending());

    assert!(open.await.unwrap().is_none());
    let response = hover
        .await
        .unwrap()
        .expect("hover response after admitted open");
    assert!(response.is_ok());
    assert_ne!(response.result(), Some(&serde_json::Value::Null));
}

#[tokio::test(flavor = "current_thread")]
async fn completion_after_an_unpolled_change_uses_only_the_committed_text() {
    let (mut service, _socket) = MermanLanguageServer::service();
    initialize_test_service(&mut service).await;
    let uri = Uri::from_str("file:///tmp/completion-after-change.mmd").unwrap();

    let open = service
        .call(
            Request::build("textDocument/didOpen")
                .params(
                    serde_json::to_value(DidOpenTextDocumentParams {
                        text_document: TextDocumentItem {
                            uri: uri.clone(),
                            language_id: "mermaid".to_string(),
                            version: 1,
                            text: concat!(
                                "flowchart TD\n",
                                "A-->B\n",
                                "classDef old fill:#f00\n",
                                "class A o\n",
                            )
                            .to_string(),
                        },
                    })
                    .unwrap(),
                )
                .finish(),
        )
        .await;
    assert!(open.unwrap().is_none());

    let change = service.call(
        Request::build("textDocument/didChange")
            .params(
                serde_json::to_value(DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: uri.clone(),
                        version: 2,
                    },
                    content_changes: vec![TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: concat!(
                            "flowchart TD\n",
                            "A-->B\n",
                            "classDef fresh fill:#0f0\n",
                            "class A f\n",
                        )
                        .to_string(),
                    }],
                })
                .unwrap(),
            )
            .finish(),
    );
    let completion = service.call(
        Request::build("textDocument/completion")
            .params(
                serde_json::to_value(CompletionParams {
                    text_document_position: TextDocumentPositionParams::new(
                        TextDocumentIdentifier { uri },
                        Position::new(3, 9),
                    ),
                    work_done_progress_params: Default::default(),
                    partial_result_params: Default::default(),
                    context: None,
                })
                .unwrap(),
            )
            .id(2)
            .finish(),
    );
    tokio::pin!(completion);
    assert!(futures::poll!(&mut completion).is_pending());

    assert!(change.await.unwrap().is_none());
    let response = completion
        .await
        .unwrap()
        .expect("completion response after admitted change");
    assert!(response.is_ok());
    let result: CompletionResponse = serde_json::from_value(
        response
            .result()
            .cloned()
            .expect("successful completion result"),
    )
    .expect("valid completion response");
    let items = match result {
        CompletionResponse::Array(items) => items,
        CompletionResponse::List(list) => list.items,
    };
    assert!(items.iter().any(|item| item.label == "fresh"));
    assert!(!items.iter().any(|item| item.label == "old"));
}

#[tokio::test(flavor = "current_thread")]
async fn cancellation_reaches_a_read_waiting_for_an_earlier_mutation() {
    let (mut service, _socket, session) = test_session_service();
    initialize_test_service(&mut service).await;
    let uri = Uri::from_str("file:///tmp/cancel-before-route.mmd").unwrap();

    let open = service.call(
        Request::build("textDocument/didOpen")
            .params(
                serde_json::to_value(DidOpenTextDocumentParams {
                    text_document: TextDocumentItem {
                        uri: uri.clone(),
                        language_id: "mermaid".to_string(),
                        version: 1,
                        text: "flowchart TD\nA-->B\n".to_string(),
                    },
                })
                .unwrap(),
            )
            .finish(),
    );
    let hover = service.call(
        Request::build("textDocument/hover")
            .params(
                serde_json::to_value(HoverParams {
                    text_document_position_params: TextDocumentPositionParams::new(
                        TextDocumentIdentifier { uri: uri.clone() },
                        Position::new(1, 0),
                    ),
                    work_done_progress_params: Default::default(),
                })
                .unwrap(),
            )
            .id(2)
            .finish(),
    );
    tokio::pin!(hover);
    assert!(futures::poll!(&mut hover).is_pending());

    let cancel = service.call(
        Request::build("$/cancelRequest")
            .params(serde_json::json!({ "id": 2 }))
            .finish(),
    );

    assert!(cancel.await.unwrap().is_none());
    let hover = match futures::poll!(&mut hover) {
        std::task::Poll::Ready(response) => response,
        std::task::Poll::Pending => {
            panic!("cancelled read must not remain pinned behind ordered admission")
        }
    }
    .unwrap()
    .expect("cancelled hover response");
    assert_eq!(
        hover.error().expect("request cancellation error").code,
        tower_lsp_server::jsonrpc::ErrorCode::RequestCancelled
    );
    assert!(
        session.probe().document(&uri).await.is_none(),
        "the predecessor must still be unpolled when cancellation resolves"
    );

    assert!(open.await.unwrap().is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn stale_push_diagnostic_context_is_suppressed() {
    let (_service, _socket, server) = test_service();
    let server = &server;
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    assert!(
        server
            .session
            .open_document(
                uri.clone(),
                1,
                "flowchart TD\nA-->B\n".to_string(),
                DocumentKind::Diagram,
            )
            .await
    );
    let context = server
        .session
        .diagnostic_context(&uri)
        .await
        .expect("expected diagnostic context");
    assert!(
        server
            .session
            .open_document(
                uri.clone(),
                2,
                "flowchart TD\nA-->C\n".to_string(),
                DocumentKind::Diagram,
            )
            .await
    );

    let diagnostics = server
        .client_effects
        .diagnostics_for_context(&context)
        .await
        .expect("stale push diagnostics should be suppressed cleanly");

    assert!(diagnostics.is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn diagnostic_pull_uses_latest_document() {
    let (_service, _socket, server) = test_service();
    let server = &server;
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

    assert!(
        server
            .session
            .open_document(
                uri.clone(),
                1,
                "flowchart TD\nA-->B\n".to_string(),
                DocumentKind::Diagram,
            )
            .await
    );
    assert!(
        server
            .session
            .open_document(
                uri.clone(),
                2,
                "flowchart TD\nA[unterminated\n".to_string(),
                DocumentKind::Diagram,
            )
            .await
    );

    let report = server
        .diagnostic(DocumentDiagnosticParams {
            text_document: TextDocumentIdentifier { uri },
            identifier: None,
            previous_result_id: None,
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .expect("latest diagnostic context should be analyzed");
    let diagnostics = match report {
        DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(report)) => {
            report.full_document_diagnostic_report.items
        }
        other => panic!("unexpected diagnostic report: {other:?}"),
    };
    let parse_diagnostic = diagnostics
        .into_iter()
        .find(|diagnostic| {
            diagnostic.code
                == Some(NumberOrString::String(
                    "merman.parse.diagram_parse".to_string(),
                ))
        })
        .expect("expected latest parse diagnostic");
    let data = parse_diagnostic.data.expect("expected diagnostic data");
    assert_eq!(data["documentVersion"], 2);
}

#[tokio::test(flavor = "current_thread")]
async fn diagnostic_pull_reuses_cached_previous_result() {
    let (_service, _socket, server) = test_service();
    let server = &server;
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
    let (_service, _socket, server) = test_service();
    let server = &server;
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    server
        .client_profile
        .set(crate::client_profile::ClientProtocolProfile::permissive())
        .expect("test profile should initialize once");
    server
        .session
        .update_configuration(
            default_lsp_analysis_options().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_enabled("merman.authoring.flowchart.explicit_direction")
                    .unwrap(),
            ),
        )
        .await;

    server
        .did_open(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: uri.clone(),
                language_id: "mermaid".to_string(),
                version: 1,
                text: "flowchart\nA-->B\n".to_string(),
            },
        })
        .await;
    let diagnostic_context = server
        .session
        .diagnostic_context(&uri)
        .await
        .expect("opened document diagnostic context");
    let stale_diagnostic = server
        .client_effects
        .diagnostics_for_context(&diagnostic_context)
        .await
        .expect("initial diagnostic projection")
        .expect("initial diagnostics")
        .into_iter()
        .find(|diagnostic| {
            diagnostic.code
                == Some(NumberOrString::String(
                    "merman.authoring.flowchart.explicit_direction".to_string(),
                ))
        })
        .expect("direction diagnostic with server-owned identity");

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
            range: stale_diagnostic.range,
            context: CodeActionContext {
                diagnostics: vec![stale_diagnostic],
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

#[tokio::test(flavor = "current_thread")]
async fn code_action_rejects_close_reopen_aba_with_reused_uri_and_version() {
    let (_service, _socket, server) = test_service();
    let server = &server;
    server
        .client_profile
        .set(crate::client_profile::ClientProtocolProfile::permissive())
        .expect("test profile should initialize once");
    let uri = Uri::from_str("file:///tmp/code-action-aba.mmd").unwrap();
    let source = "flowchart\nA-->B\n";

    server
        .session
        .update_configuration(
            default_lsp_analysis_options().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_enabled("merman.authoring.flowchart.explicit_direction")
                    .unwrap(),
            ),
        )
        .await;

    assert!(
        server
            .session
            .open_document(uri.clone(), 1, source.to_string(), DocumentKind::Diagram,)
            .await
    );
    let old_context = server
        .session
        .diagnostic_context(&uri)
        .await
        .expect("first document incarnation");
    let old_diagnostic = server
        .client_effects
        .diagnostics_for_context(&old_context)
        .await
        .expect("first diagnostic projection")
        .expect("first diagnostics")
        .into_iter()
        .find(|diagnostic| {
            diagnostic.code
                == Some(NumberOrString::String(
                    "merman.authoring.flowchart.explicit_direction".to_string(),
                ))
        })
        .expect("first direction diagnostic");

    server.session.close_document(&uri).await;
    assert!(
        server
            .session
            .open_document(uri.clone(), 1, source.to_string(), DocumentKind::Diagram,)
            .await
    );
    let new_context = server
        .session
        .diagnostic_context(&uri)
        .await
        .expect("second document incarnation");
    let new_diagnostic = server
        .client_effects
        .diagnostics_for_context(&new_context)
        .await
        .expect("second diagnostic projection")
        .expect("second diagnostics")
        .into_iter()
        .find(|diagnostic| {
            diagnostic.code
                == Some(NumberOrString::String(
                    "merman.authoring.flowchart.explicit_direction".to_string(),
                ))
        })
        .expect("second direction diagnostic");
    assert_ne!(old_diagnostic.data, new_diagnostic.data);

    let old_actions = server
        .code_action(CodeActionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            range: old_diagnostic.range,
            context: CodeActionContext {
                diagnostics: vec![old_diagnostic],
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .expect("stale code-action request");
    assert!(old_actions.is_none());

    let new_actions = server
        .code_action(CodeActionParams {
            text_document: TextDocumentIdentifier { uri },
            range: new_diagnostic.range,
            context: CodeActionContext {
                diagnostics: vec![new_diagnostic],
                only: Some(vec![CodeActionKind::QUICKFIX]),
                trigger_kind: None,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        })
        .await
        .expect("current code-action request")
        .expect("current diagnostic must retain its fix");
    assert_eq!(new_actions.len(), 1);
}

#[test]
fn structure_helpers_produce_hover_and_nested_symbols() {
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = snapshot_for_test(uri.clone(), 1, "flowchart TD\nsubgraph group\nA-->B\nend\n");

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
    let markdown_snapshot = snapshot_for_test(
        markdown_uri,
        1,
        "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n",
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
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = snapshot_for_test(uri.clone(), 1, "flowchart TD\nA-->B\nA-->C\n");
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
    let (_service, _socket, server) = test_service();
    let server = &server;
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

    server
        .session
        .update_configuration(
            default_lsp_analysis_options().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_enabled("merman.authoring.flowchart.explicit_direction")
                    .unwrap(),
            ),
        )
        .await;
    assert!(
        server
            .session
            .open_document(
                uri.clone(),
                1,
                "flowchart\nsubgraph group\nA-->B\nend\n".to_string(),
                DocumentKind::Diagram,
            )
            .await
    );
    assert!(
        server
            .session
            .open_document(
                Uri::from_str("file:///tmp/example.md").unwrap(),
                1,
                "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n".to_string(),
                DocumentKind::Markdown,
            )
            .await
    );

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
                end: Position::new(2, 5),
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

    let context = server
        .session
        .diagnostic_context(&uri)
        .await
        .expect("expected snapshot-backed diagnostics");
    let diagnostic = server
        .client_effects
        .diagnostics_for_context(&context)
        .await
        .expect("diagnostic analysis should succeed")
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

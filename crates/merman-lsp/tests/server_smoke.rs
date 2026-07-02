use futures::SinkExt;
use futures::StreamExt;
use merman_lsp::MermanLanguageServer;
use merman_lsp::protocol::{CONFIG_SCHEMA_METHOD, RULE_CATALOG_METHOD};
use serde_json::from_value;
use tokio::time::{Duration, timeout};
use tower::{Service, ServiceExt};
use tower_lsp::jsonrpc::{ErrorCode, Request};
use tower_lsp::lsp_types::{
    CodeActionContext, CodeActionKind, CodeActionParams, DiagnosticServerCapabilities,
    DidChangeConfigurationParams, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DidSaveTextDocumentParams, DocumentDiagnosticParams,
    DocumentDiagnosticReport, DocumentDiagnosticReportResult, DocumentSymbolParams,
    GotoDefinitionParams, HoverContents, HoverParams, InitializeParams, NumberOrString, Position,
    PrepareRenameResponse, PublishDiagnosticsParams, Range, ReferenceContext, ReferenceParams,
    RenameParams, SelectionRange, SelectionRangeParams, SemanticTokensDeltaParams,
    SemanticTokensFullDeltaResult, SemanticTokensParams, SemanticTokensRangeParams,
    SemanticTokensRangeResult, SemanticTokensResult, SymbolInformation,
    TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem,
    TextDocumentPositionParams, VersionedTextDocumentIdentifier, WorkspaceSymbolParams,
};

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_handles_initialize() {
    let (service, _socket) = MermanLanguageServer::service();

    let response = tower_lsp::LanguageServer::initialize(
        service.inner(),
        InitializeParams {
            initialization_options: Some(serde_json::json!({
                "lint": {
                    "disable_rules": ["merman.git_graph.duplicate_commit_id"]
                }
            })),
            ..InitializeParams::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(
        response
            .capabilities
            .completion_provider
            .as_ref()
            .and_then(|options| options.resolve_provider),
        Some(true)
    );
    assert!(response.capabilities.code_action_provider.is_some());
    assert!(response.capabilities.semantic_tokens_provider.is_some());
    assert_eq!(
        response.capabilities.experimental.as_ref().unwrap()["merman"]["requests"]["ruleCatalog"],
        RULE_CATALOG_METHOD
    );
    assert_eq!(
        response.capabilities.experimental.as_ref().unwrap()["merman"]["requests"]["configSchema"],
        CONFIG_SCHEMA_METHOD
    );
    assert!(response.capabilities.diagnostic_provider.is_some());
    assert!(matches!(
        MermanLanguageServer::capabilities().text_document_sync,
        Some(tower_lsp::lsp_types::TextDocumentSyncCapability::Kind(
            tower_lsp::lsp_types::TextDocumentSyncKind::FULL
        ))
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_does_not_advertise_workspace_diagnostics_without_workspace_scan() {
    let (service, _socket) = MermanLanguageServer::service();

    let response =
        tower_lsp::LanguageServer::initialize(service.inner(), InitializeParams::default())
            .await
            .unwrap();

    let provider = response
        .capabilities
        .diagnostic_provider
        .expect("diagnostic provider");
    let options = match provider {
        DiagnosticServerCapabilities::Options(options) => options,
        other => panic!("unexpected diagnostic capability: {other:?}"),
    };
    assert!(!options.workspace_diagnostics);
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_rejects_unadvertised_workspace_diagnostics() {
    let (mut service, _socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "textDocument": {
                    "diagnostic": {}
                },
                "workspace": {
                    "diagnostic": {}
                }
            }
        }))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: "gitGraph\ncommit id:\"working on MDR\"\ncommit id:\"working on MDR\"\n"
                        .to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let request = Request::build("workspace/diagnostic")
        .params(serde_json::json!({ "previousResultIds": [] }))
        .id(2)
        .finish();
    let response = service
        .ready()
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap()
        .expect("workspace diagnostic response");
    assert!(response.is_error());
    assert_eq!(
        response.error().expect("workspace diagnostic error").code,
        ErrorCode::MethodNotFound
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_with_diagnostic_pull_does_not_also_push_diagnostics() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "textDocument": {
                    "diagnostic": {}
                },
                "workspace": {
                    "diagnostic": {}
                }
            }
        }))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri,
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: "flowchart TD\nA-->B\n".to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let pushed = timeout(Duration::from_millis(200), socket.next()).await;
    assert!(
        pushed.is_err(),
        "diagnostic pull clients should not receive push diagnostics"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_workspace_diagnostic_capability_does_not_disable_push_diagnostics() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "workspace": {
                    "diagnostic": {}
                }
            }
        }))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: String::new(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let pushed = socket.next().await.expect("expected push diagnostics");
    assert_eq!(pushed.method(), "textDocument/publishDiagnostics");
    let params: PublishDiagnosticsParams =
        from_value(pushed.params().cloned().expect("publish params")).unwrap();
    assert_eq!(params.uri, uri);
    assert_eq!(params.version, Some(1));
    assert!(!params.diagnostics.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_serves_rule_catalog_custom_request() {
    let (mut service, _socket) = MermanLanguageServer::service();

    let initialize = Request::build("initialize")
        .params(serde_json::to_value(InitializeParams::default()).unwrap())
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

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

    assert_eq!(result["version"], 1);
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

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_serves_config_schema_custom_request() {
    let (mut service, _socket) = MermanLanguageServer::service();

    let initialize = Request::build("initialize")
        .params(serde_json::to_value(InitializeParams::default()).unwrap())
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let request = Request::build(CONFIG_SCHEMA_METHOD).id(2).finish();
    let response = service
        .ready()
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap()
        .expect("config schema response");
    let result = response.result().expect("config schema result");

    assert_eq!(result["version"], 1);
    assert_eq!(result["rule_catalog_method"], RULE_CATALOG_METHOD);
    assert!(
        result["configurable_rule_ids"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "merman.authoring.flowchart.explicit_direction")
    );
    assert_eq!(
        result["schema"]["$defs"]["analysisOptions"]["properties"]["lint"]["properties"]["profile"]
            ["enum"],
        serde_json::json!(["core", "recommended", "strict"])
    );
    assert_eq!(
        result["schema"]["$defs"]["severity"]["enum"],
        serde_json::json!(["error", "warning", "info", "hint"])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_pulls_document_diagnostics() {
    let (mut service, _socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::to_value(InitializeParams::default()).unwrap())
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: "gitGraph\ncommit id:\"working on MDR\"\ncommit id:\"working on MDR\"\n"
                        .to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let request = Request::build("textDocument/diagnostic")
        .params(
            serde_json::to_value(DocumentDiagnosticParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                identifier: None,
                previous_result_id: None,
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(2)
        .finish();
    let response = service
        .ready()
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap()
        .expect("document diagnostic response");
    let result: DocumentDiagnosticReportResult = from_value(
        response
            .result()
            .cloned()
            .expect("document diagnostic result"),
    )
    .unwrap();

    let report = match result {
        DocumentDiagnosticReportResult::Report(report) => report,
        other => panic!("unexpected diagnostic result: {other:?}"),
    };
    let full = match report {
        DocumentDiagnosticReport::Full(report) => report,
        other => panic!("unexpected diagnostic report: {other:?}"),
    };
    assert!(full.full_document_diagnostic_report.result_id.is_some());
    assert_eq!(full.full_document_diagnostic_report.items.len(), 1);
    assert_eq!(
        full.full_document_diagnostic_report.items[0].severity,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING)
    );

    let request = Request::build("textDocument/diagnostic")
        .params(
            serde_json::to_value(DocumentDiagnosticParams {
                text_document: TextDocumentIdentifier { uri },
                identifier: None,
                previous_result_id: full.full_document_diagnostic_report.result_id.clone(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(3)
        .finish();
    let response = service
        .ready()
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap()
        .expect("unchanged document diagnostic response");
    let result: DocumentDiagnosticReportResult = from_value(
        response
            .result()
            .cloned()
            .expect("unchanged document diagnostic result"),
    )
    .unwrap();
    let report = match result {
        DocumentDiagnosticReportResult::Report(report) => report,
        other => panic!("unexpected diagnostic result: {other:?}"),
    };
    match report {
        DocumentDiagnosticReport::Unchanged(report) => {
            assert_eq!(
                report.unchanged_document_diagnostic_report.result_id,
                full.full_document_diagnostic_report.result_id.unwrap()
            );
        }
        other => panic!("unexpected diagnostic report: {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_pull_after_close_returns_stable_empty_report() {
    let (mut service, _socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "textDocument": {
                    "diagnostic": {}
                }
            }
        }))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: String::new(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let first = Request::build("textDocument/diagnostic")
        .params(
            serde_json::to_value(DocumentDiagnosticParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                identifier: None,
                previous_result_id: None,
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(2)
        .finish();
    let response = service
        .ready()
        .await
        .unwrap()
        .call(first)
        .await
        .unwrap()
        .expect("document diagnostic response");
    let result: DocumentDiagnosticReportResult = from_value(
        response
            .result()
            .cloned()
            .expect("document diagnostic result"),
    )
    .unwrap();
    let DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(first_full)) = result
    else {
        panic!("expected full diagnostic report");
    };
    assert!(!first_full.full_document_diagnostic_report.items.is_empty());

    let close = Request::build("textDocument/didClose")
        .params(
            serde_json::to_value(DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(close).await.unwrap(),
        None
    );

    let empty = Request::build("textDocument/diagnostic")
        .params(
            serde_json::to_value(DocumentDiagnosticParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                identifier: None,
                previous_result_id: None,
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(3)
        .finish();
    let response = service
        .ready()
        .await
        .unwrap()
        .call(empty)
        .await
        .unwrap()
        .expect("empty document diagnostic response");
    let result: DocumentDiagnosticReportResult = from_value(
        response
            .result()
            .cloned()
            .expect("empty document diagnostic result"),
    )
    .unwrap();
    let DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(empty_full)) = result
    else {
        panic!("expected empty full diagnostic report");
    };
    assert!(empty_full.full_document_diagnostic_report.items.is_empty());
    let empty_result_id = empty_full
        .full_document_diagnostic_report
        .result_id
        .expect("empty result id");

    let unchanged = Request::build("textDocument/diagnostic")
        .params(
            serde_json::to_value(DocumentDiagnosticParams {
                text_document: TextDocumentIdentifier { uri },
                identifier: None,
                previous_result_id: Some(empty_result_id.clone()),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(4)
        .finish();
    let response = service
        .ready()
        .await
        .unwrap()
        .call(unchanged)
        .await
        .unwrap()
        .expect("unchanged document diagnostic response");
    let result: DocumentDiagnosticReportResult = from_value(
        response
            .result()
            .cloned()
            .expect("unchanged document diagnostic result"),
    )
    .unwrap();
    let DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Unchanged(unchanged)) =
        result
    else {
        panic!("expected unchanged empty diagnostic report");
    };
    assert_eq!(
        unchanged.unchanged_document_diagnostic_report.result_id,
        empty_result_id
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_reports_deprecated_flowchart_html_labels_with_quickfix() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::to_value(InitializeParams::default()).unwrap())
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: "%%{init: { \"flowchart\": { \"htmlLabels\": false, \"curve\": \"linear\" } }}%%\nflowchart TD\nA-->B\n".to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let publish = timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .expect("expected diagnostics publish");
    assert_eq!(publish.method(), "textDocument/publishDiagnostics");
    let params: PublishDiagnosticsParams =
        from_value(publish.params().cloned().expect("publish params")).unwrap();
    assert_eq!(params.diagnostics.len(), 1);
    let diagnostic = params.diagnostics[0].clone();
    assert_eq!(
        diagnostic.severity,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING)
    );
    assert_eq!(
        diagnostic.code,
        Some(NumberOrString::String(
            "merman.compatibility.config.deprecated_flowchart_html_labels".to_string()
        ))
    );
    let request = Request::build("textDocument/codeAction")
        .params(
            serde_json::to_value(CodeActionParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                range: Range {
                    start: Position::new(0, 0),
                    end: Position::new(0, 80),
                },
                context: CodeActionContext {
                    diagnostics: vec![diagnostic],
                    only: Some(vec![CodeActionKind::QUICKFIX]),
                    trigger_kind: None,
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(2)
        .finish();
    let response = service
        .ready()
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap()
        .expect("code action response");
    let result = response.result().expect("code action result");
    let actions = result.as_array().expect("code action array");
    let action = actions
        .iter()
        .find(|action| {
            action.get("title").and_then(|value| value.as_str())
                == Some("Move deprecated `flowchart.htmlLabels` to root `htmlLabels`")
        })
        .expect("missing deprecated htmlLabels quickfix");
    let edits = action["edit"]["changes"][uri.as_str()]
        .as_array()
        .expect("expected text edits");
    assert!(edits.iter().any(|edit| {
        edit["newText"]
            .as_str()
            .is_some_and(|text| text.contains("htmlLabels: false"))
    }));
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_reports_flowchart_unknown_style_target_warning() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::to_value(InitializeParams::default()).unwrap())
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: "flowchart TD\nstyle Q background:#fff\nA-->B\n".to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let publish = timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .expect("expected diagnostics publish");
    assert_eq!(publish.method(), "textDocument/publishDiagnostics");
    let params: PublishDiagnosticsParams =
        from_value(publish.params().cloned().expect("publish params")).unwrap();
    assert_eq!(params.uri, uri);
    assert_eq!(params.diagnostics.len(), 1);
    let diagnostic = params.diagnostics[0].clone();
    assert_eq!(
        diagnostic.severity,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::WARNING)
    );
    assert_eq!(
        diagnostic.code,
        Some(NumberOrString::String(
            "merman.semantic.flowchart.unknown_style_target".to_string()
        ))
    );
    assert_eq!(diagnostic.range.start.line, 1);
    assert_eq!(diagnostic.range.start.character, 6);
    assert_eq!(diagnostic.range.end.line, 1);
    assert_eq!(diagnostic.range.end.character, 7);
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_resolves_completion_items() {
    let (mut service, _socket) = MermanLanguageServer::service();

    let initialize = Request::build("initialize")
        .params(serde_json::to_value(InitializeParams::default()).unwrap())
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let request = Request::build("completionItem/resolve")
        .params(serde_json::json!({
            "label": "flowchart TD",
            "data": {
                "kind": "diagram_header",
                "label": "flowchart TD"
            }
        }))
        .id(2)
        .finish();
    let response = service
        .ready()
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap()
        .expect("completion resolve response");
    let result = response.result().expect("completion resolve result");

    assert_eq!(result["label"], "flowchart TD");
    assert_eq!(result["documentation"]["kind"], "markdown");
    assert!(
        result["documentation"]["value"]
            .as_str()
            .unwrap()
            .contains("Starts a Mermaid")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_applies_configuration_updates() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {},
            "initializationOptions": {
                "lint": {
                    "disable_rules": ["merman.git_graph.duplicate_commit_id"]
                }
            }
        }))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: "gitGraph\ncommit id:\"working on MDR\"\ncommit id:\"working on MDR\"\n"
                        .to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let first = socket.next().await.expect("expected diagnostics publish");
    let first_params: PublishDiagnosticsParams =
        from_value(first.params().cloned().expect("publish params")).unwrap();
    assert!(first_params.diagnostics.is_empty());

    let change = Request::build("workspace/didChangeConfiguration")
        .params(
            serde_json::to_value(DidChangeConfigurationParams {
                settings: serde_json::json!({
                    "lint": {
                        "rule_severities": [
                            {
                                "rule_id": "merman.git_graph.duplicate_commit_id",
                                "severity": "hint"
                            }
                        ]
                    }
                }),
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(change).await.unwrap(),
        None
    );

    let second = socket
        .next()
        .await
        .expect("expected republished diagnostics");
    let second_params: PublishDiagnosticsParams =
        from_value(second.params().cloned().expect("publish params")).unwrap();
    assert_eq!(second_params.version, Some(1));
    assert_eq!(second_params.diagnostics.len(), 1);
    assert_eq!(
        second_params.diagnostics[0].severity,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::HINT)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_refreshes_semantic_tokens_after_configuration_change() {
    let (mut service, mut socket) = MermanLanguageServer::service();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "workspace": {
                    "semanticTokens": {
                        "refreshSupport": true
                    }
                }
            }
        }))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let change = Request::build("workspace/didChangeConfiguration")
        .params(
            serde_json::to_value(DidChangeConfigurationParams {
                settings: serde_json::json!({
                    "parse": {
                        "suppress_errors": true
                    }
                }),
            })
            .unwrap(),
        )
        .finish();
    let mut change_fut = Box::pin(service.ready().await.unwrap().call(change));
    let refresh = tokio::select! {
        result = &mut change_fut => {
            panic!("configuration change finished before refresh request: {result:?}");
        }
        message = socket.next() => {
            message.expect("expected semantic tokens refresh request")
        }
    };
    assert_eq!(refresh.method(), "workspace/semanticTokens/refresh");

    socket
        .send(tower_lsp::jsonrpc::Response::from_ok(
            refresh.id().cloned().expect("refresh request id"),
            serde_json::Value::Null,
        ))
        .await
        .unwrap();

    assert_eq!(change_fut.await.unwrap(), None);
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_does_not_refresh_semantic_tokens_after_diagnostic_only_configuration_change() {
    let (mut service, mut socket) = MermanLanguageServer::service();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "workspace": {
                    "semanticTokens": {
                        "refreshSupport": true
                    }
                }
            }
        }))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let change = Request::build("workspace/didChangeConfiguration")
        .params(
            serde_json::to_value(DidChangeConfigurationParams {
                settings: serde_json::json!({
                    "lint": {
                        "disable_rules": ["merman.git_graph.duplicate_commit_id"]
                    }
                }),
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(change).await.unwrap(),
        None
    );
    assert!(
        timeout(Duration::from_millis(50), socket.next())
            .await
            .is_err(),
        "diagnostic-only configuration changes should not refresh semantic tokens"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_diagnostic_pull_without_refresh_support_finishes_configuration_change() {
    let (mut service, mut socket) = MermanLanguageServer::service();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "textDocument": {
                    "diagnostic": {}
                },
                "workspace": {
                    "diagnostic": {}
                }
            }
        }))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let change = Request::build("workspace/didChangeConfiguration")
        .params(
            serde_json::to_value(DidChangeConfigurationParams {
                settings: serde_json::json!({
                    "lint": {
                        "disable_rules": ["merman.git_graph.duplicate_commit_id"]
                    }
                }),
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(change).await.unwrap(),
        None
    );
    assert!(
        timeout(Duration::from_millis(50), socket.next())
            .await
            .is_err(),
        "unexpected workspace diagnostic refresh request"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_refreshes_diagnostics_after_configuration_change_when_supported() {
    let (mut service, mut socket) = MermanLanguageServer::service();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "textDocument": {
                    "diagnostic": {}
                },
                "workspace": {
                    "diagnostic": {
                        "refreshSupport": true
                    }
                }
            }
        }))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let change = Request::build("workspace/didChangeConfiguration")
        .params(
            serde_json::to_value(DidChangeConfigurationParams {
                settings: serde_json::json!({
                    "lint": {
                        "disable_rules": ["merman.git_graph.duplicate_commit_id"]
                    }
                }),
            })
            .unwrap(),
        )
        .finish();
    let mut change_fut = Box::pin(service.ready().await.unwrap().call(change));
    let refresh = tokio::select! {
        result = &mut change_fut => {
            panic!("configuration change finished before refresh request: {result:?}");
        }
        message = socket.next() => {
            message.expect("expected workspace diagnostic refresh request")
        }
    };
    assert_eq!(refresh.method(), "workspace/diagnostic/refresh");

    socket
        .send(tower_lsp::jsonrpc::Response::from_ok(
            refresh.id().cloned().expect("refresh request id"),
            serde_json::Value::Null,
        ))
        .await
        .unwrap();

    assert_eq!(change_fut.await.unwrap(), None);
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_applies_core_rule_severity_overrides_on_initialize() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {},
            "initializationOptions": {
                "lint": {
                    "rule_severities": [
                        {
                            "rule_id": "merman.parse.no_diagram",
                            "severity": "hint"
                        }
                    ]
                }
            }
        }))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: String::new(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let publish = socket.next().await.expect("expected diagnostics publish");
    let params: PublishDiagnosticsParams =
        from_value(publish.params().cloned().expect("publish params")).unwrap();
    assert_eq!(params.uri, uri);
    assert_eq!(params.diagnostics.len(), 1);
    assert_eq!(
        params.diagnostics[0].severity,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::HINT)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_honors_core_rule_disablement() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {},
            "initializationOptions": {
                "lint": {
                    "disable_rules": [
                        "merman.parse.no_diagram",
                        "merman.resource.source_bytes_exceeded"
                    ]
                },
                "resources": {
                    "max_source_bytes": 8
                }
            }
        }))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let empty_open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: String::new(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service
            .ready()
            .await
            .unwrap()
            .call(empty_open)
            .await
            .unwrap(),
        None
    );

    let empty_publish = socket
        .next()
        .await
        .expect("expected empty diagnostics publish");
    let empty_params: PublishDiagnosticsParams =
        from_value(empty_publish.params().cloned().expect("publish params")).unwrap();
    assert!(empty_params.diagnostics.is_empty());

    let resource_uri = tower_lsp::lsp_types::Url::parse("file:///tmp/limited.mmd").unwrap();
    let resource_open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: resource_uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: "flowchart TD\nA-->B\n".to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service
            .ready()
            .await
            .unwrap()
            .call(resource_open)
            .await
            .unwrap(),
        None
    );

    let resource_publish = socket
        .next()
        .await
        .expect("expected resource diagnostics publish");
    let resource_params: PublishDiagnosticsParams =
        from_value(resource_publish.params().cloned().expect("publish params")).unwrap();
    assert_eq!(resource_params.uri, resource_uri);
    assert!(resource_params.diagnostics.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_applies_resource_limit_severity_override_on_initialize() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {},
            "initializationOptions": {
                "lint": {
                    "rule_severities": [
                        {
                            "rule_id": "merman.resource.source_bytes_exceeded",
                            "severity": "hint"
                        }
                    ]
                },
                "resources": {
                    "max_source_bytes": 8
                }
            }
        }))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
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
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let publish = socket.next().await.expect("expected diagnostics publish");
    let params: PublishDiagnosticsParams =
        from_value(publish.params().cloned().expect("publish params")).unwrap();
    assert_eq!(params.uri, uri);
    assert_eq!(params.diagnostics.len(), 1);
    assert_eq!(
        params.diagnostics[0].severity,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::HINT)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_applies_resource_limit_severity_override_on_configuration_change() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {},
            "initializationOptions": {
                "resources": {
                    "max_source_bytes": 8
                }
            }
        }))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
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
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let first = socket.next().await.expect("expected diagnostics publish");
    let first_params: PublishDiagnosticsParams =
        from_value(first.params().cloned().expect("publish params")).unwrap();
    assert_eq!(first_params.diagnostics.len(), 1);
    assert_eq!(
        first_params.diagnostics[0].severity,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR)
    );

    let change = Request::build("workspace/didChangeConfiguration")
        .params(
            serde_json::to_value(DidChangeConfigurationParams {
                settings: serde_json::json!({
                    "lint": {
                        "rule_severities": [
                            {
                                "rule_id": "merman.resource.source_bytes_exceeded",
                                "severity": "hint"
                            }
                        ]
                    },
                    "resources": {
                        "max_source_bytes": 8
                    }
                }),
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(change).await.unwrap(),
        None
    );

    let second = socket
        .next()
        .await
        .expect("expected republished diagnostics");
    let second_params: PublishDiagnosticsParams =
        from_value(second.params().cloned().expect("publish params")).unwrap();
    assert_eq!(second_params.version, Some(1));
    assert_eq!(second_params.diagnostics.len(), 1);
    assert_eq!(
        second_params.diagnostics[0].severity,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::HINT)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_serves_semantic_tokens_range() {
    let (mut service, _socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({"capabilities": {}}))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
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
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let request = Request::build("textDocument/semanticTokens/range")
        .params(
            serde_json::to_value(SemanticTokensRangeParams {
                text_document: TextDocumentIdentifier { uri },
                range: Range {
                    start: Position::new(0, 0),
                    end: Position::new(10, 0),
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(2)
        .finish();
    let response = service.ready().await.unwrap().call(request).await.unwrap();
    let value = response
        .as_ref()
        .and_then(|response| response.result().clone())
        .expect("expected semantic tokens range result");
    let _: SemanticTokensRangeResult = serde_json::from_value(value.clone()).unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_serves_semantic_tokens_delta() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({"capabilities": {}}))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
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
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );
    let first_diagnostics = timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .expect("expected diagnostics after open");
    assert_eq!(
        first_diagnostics.method(),
        "textDocument/publishDiagnostics"
    );

    let full_request = Request::build("textDocument/semanticTokens/full")
        .params(
            serde_json::to_value(SemanticTokensParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(2)
        .finish();
    let full_response = service
        .ready()
        .await
        .unwrap()
        .call(full_request)
        .await
        .unwrap();
    let full_value = full_response
        .as_ref()
        .and_then(|response| response.result().clone())
        .expect("expected semantic tokens full result");
    let full_result: SemanticTokensResult = serde_json::from_value(full_value.clone()).unwrap();
    let previous_result_id = match full_result {
        SemanticTokensResult::Tokens(tokens) => tokens
            .result_id
            .expect("expected semantic tokens result id"),
        other => panic!("unexpected semantic tokens full result: {other:?}"),
    };

    let change = Request::build("textDocument/didChange")
        .params(
            serde_json::to_value(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    text: "flowchart TD\nAlpha-->B\n".to_string(),
                    range: None,
                    range_length: None,
                }],
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(change).await.unwrap(),
        None
    );
    let second_diagnostics = timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .expect("expected diagnostics after change");
    assert_eq!(
        second_diagnostics.method(),
        "textDocument/publishDiagnostics"
    );

    let delta_request = Request::build("textDocument/semanticTokens/full/delta")
        .params(
            serde_json::to_value(SemanticTokensDeltaParams {
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                text_document: TextDocumentIdentifier { uri },
                previous_result_id,
            })
            .unwrap(),
        )
        .id(3)
        .finish();
    let delta_response = service
        .ready()
        .await
        .unwrap()
        .call(delta_request)
        .await
        .unwrap();
    let delta_value = delta_response
        .as_ref()
        .and_then(|response| response.result().clone())
        .expect("expected semantic tokens delta result");
    let delta_result: SemanticTokensFullDeltaResult =
        serde_json::from_value(delta_value.clone()).unwrap();
    match delta_result {
        SemanticTokensFullDeltaResult::TokensDelta(delta) => {
            assert!(delta.result_id.is_some());
            assert!(!delta.edits.is_empty());
        }
        other => panic!("unexpected semantic tokens delta result: {other:?}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_applies_core_rule_severity_overrides_on_configuration_change() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({"capabilities":{}}))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: String::new(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let first = socket.next().await.expect("expected diagnostics publish");
    let first_params: PublishDiagnosticsParams =
        from_value(first.params().cloned().expect("publish params")).unwrap();
    assert_eq!(first_params.diagnostics.len(), 1);
    assert_eq!(
        first_params.diagnostics[0].severity,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR)
    );

    let change = Request::build("workspace/didChangeConfiguration")
        .params(
            serde_json::to_value(DidChangeConfigurationParams {
                settings: serde_json::json!({
                    "lint": {
                        "rule_severities": [
                            {
                                "rule_id": "merman.parse.no_diagram",
                                "severity": "hint"
                            }
                        ]
                    }
                }),
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(change).await.unwrap(),
        None
    );

    let second = socket
        .next()
        .await
        .expect("expected republished diagnostics");
    let second_params: PublishDiagnosticsParams =
        from_value(second.params().cloned().expect("publish params")).unwrap();
    assert_eq!(second_params.version, Some(1));
    assert_eq!(second_params.diagnostics.len(), 1);
    assert_eq!(
        second_params.diagnostics[0].severity,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::HINT)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_publishes_current_diagnostics_version() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({"capabilities":{}}))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: String::new(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let first = socket.next().await.expect("expected diagnostics publish");
    let first_params: PublishDiagnosticsParams =
        from_value(first.params().cloned().expect("publish params")).unwrap();
    assert_eq!(first.method(), "textDocument/publishDiagnostics");
    assert_eq!(first_params.version, Some(1));
    assert!(!first_params.diagnostics.is_empty());

    let change = Request::build("textDocument/didChange")
        .params(
            serde_json::to_value(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "flowchart TD\nA[Hello] --> B[World]\n".to_string(),
                }],
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(change).await.unwrap(),
        None
    );

    let second = socket.next().await.expect("expected updated diagnostics");
    let second_params: PublishDiagnosticsParams =
        from_value(second.params().cloned().expect("publish params")).unwrap();
    assert_eq!(second.method(), "textDocument/publishDiagnostics");
    assert_eq!(second_params.version, Some(2));
    assert!(second_params.diagnostics.is_empty());

    let save = Request::build("textDocument/didSave")
        .params(
            serde_json::to_value(DidSaveTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                text: None,
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(save).await.unwrap(),
        None
    );

    let third = socket.next().await.expect("expected save diagnostics");
    let third_params: PublishDiagnosticsParams =
        from_value(third.params().cloned().expect("publish params")).unwrap();
    assert_eq!(third.method(), "textDocument/publishDiagnostics");
    assert_eq!(third_params.version, Some(2));
    assert!(third_params.diagnostics.is_empty());

    assert!(
        timeout(Duration::from_millis(50), socket.next())
            .await
            .is_err(),
        "unexpected extra publishDiagnostics message"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_clears_push_diagnostics_on_close() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({"capabilities":{}}))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: String::new(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let first = socket.next().await.expect("expected diagnostics publish");
    let first_params: PublishDiagnosticsParams =
        from_value(first.params().cloned().expect("publish params")).unwrap();
    assert_eq!(first_params.uri, uri);
    assert!(!first_params.diagnostics.is_empty());

    let close = Request::build("textDocument/didClose")
        .params(
            serde_json::to_value(DidCloseTextDocumentParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(close).await.unwrap(),
        None
    );

    let cleared = socket.next().await.expect("expected cleared diagnostics");
    assert_eq!(cleared.method(), "textDocument/publishDiagnostics");
    let cleared_params: PublishDiagnosticsParams =
        from_value(cleared.params().cloned().expect("publish params")).unwrap();
    assert_eq!(cleared_params.uri, uri);
    assert_eq!(cleared_params.version, None);
    assert!(cleared_params.diagnostics.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_handles_hover_and_document_symbols() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({"capabilities":{}}))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: "flowchart TD\nsubgraph group\nA-->B\nend\n".to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );
    let first = socket.next().await.expect("expected first diagnostics");
    let first_params: PublishDiagnosticsParams =
        from_value(first.params().cloned().expect("publish params")).unwrap();
    assert_eq!(first_params.uri, uri);

    let hover = Request::build("textDocument/hover")
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
        .finish();
    let hover_response = service.ready().await.unwrap().call(hover).await.unwrap();
    let hover_value = hover_response
        .and_then(|response| response.result().cloned())
        .expect("expected hover result");
    let hover: tower_lsp::lsp_types::Hover = serde_json::from_value(hover_value).unwrap();
    let hover_text = match hover.contents {
        HoverContents::Markup(markup) => markup.value,
        other => panic!("unexpected hover contents: {other:?}"),
    };
    assert!(hover_text.contains("group"));

    let document_symbol = Request::build("textDocument/documentSymbol")
        .params(
            serde_json::to_value(DocumentSymbolParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(3)
        .finish();
    let document_symbol_response = service
        .ready()
        .await
        .unwrap()
        .call(document_symbol)
        .await
        .unwrap();
    let document_symbol_value = document_symbol_response
        .and_then(|response| response.result().cloned())
        .expect("expected document symbol result");
    let symbols: tower_lsp::lsp_types::DocumentSymbolResponse =
        serde_json::from_value(document_symbol_value).unwrap();
    assert!(matches!(
        symbols,
        tower_lsp::lsp_types::DocumentSymbolResponse::Nested(_)
    ));

    let other_uri = tower_lsp::lsp_types::Url::parse("file:///tmp/second.mmd").unwrap();
    let second_open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: other_uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: "flowchart TD\nsubgraph group\nX-->Y\nend\n".to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service
            .ready()
            .await
            .unwrap()
            .call(second_open)
            .await
            .unwrap(),
        None
    );
    let second = socket.next().await.expect("expected second diagnostics");
    let second_params: PublishDiagnosticsParams =
        from_value(second.params().cloned().expect("publish params")).unwrap();
    assert_eq!(second_params.uri, other_uri);

    let workspace_symbol = Request::build("workspace/symbol")
        .params(
            serde_json::to_value(WorkspaceSymbolParams {
                partial_result_params: Default::default(),
                work_done_progress_params: Default::default(),
                query: "group".to_string(),
            })
            .unwrap(),
        )
        .id(4)
        .finish();
    let workspace_symbol_response = service
        .ready()
        .await
        .unwrap()
        .call(workspace_symbol)
        .await
        .unwrap();
    let workspace_symbol_value = workspace_symbol_response
        .and_then(|response| response.result().cloned())
        .expect("expected workspace symbol result");
    let workspace_symbols: Vec<SymbolInformation> =
        serde_json::from_value(workspace_symbol_value).unwrap();
    assert!(
        workspace_symbols
            .iter()
            .any(|symbol| symbol.name == "group")
    );
    assert!(
        workspace_symbols
            .iter()
            .any(|symbol| symbol.location.uri == uri)
    );
    assert!(
        workspace_symbols
            .iter()
            .any(|symbol| symbol.location.uri == other_uri)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_selection_range_mixed_positions_returns_fallbacks() {
    let (mut service, _socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.md").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({"capabilities":{}}))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "markdown".to_string(),
                    version: 1,
                    text: "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n".to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let inside_only = Request::build("textDocument/selectionRange")
        .params(
            serde_json::to_value(SelectionRangeParams {
                text_document: TextDocumentIdentifier { uri: uri.clone() },
                positions: vec![Position::new(2, 0), Position::new(3, 0)],
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(2)
        .finish();
    let inside_response = service
        .ready()
        .await
        .unwrap()
        .call(inside_only)
        .await
        .unwrap();
    let inside_value = inside_response
        .and_then(|response| response.result().cloned())
        .expect("expected selection ranges result");
    let ranges: Vec<SelectionRange> = serde_json::from_value(inside_value).unwrap();
    assert_eq!(ranges.len(), 2);

    let mixed = Request::build("textDocument/selectionRange")
        .params(
            serde_json::to_value(SelectionRangeParams {
                text_document: TextDocumentIdentifier { uri },
                positions: vec![Position::new(0, 1), Position::new(3, 0)],
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(3)
        .finish();
    let mixed_response = service.ready().await.unwrap().call(mixed).await.unwrap();
    let mixed_value = mixed_response
        .and_then(|response| response.result().cloned())
        .expect("expected selection ranges result");
    let ranges: Vec<SelectionRange> = serde_json::from_value(mixed_value).unwrap();
    assert_eq!(ranges.len(), 2);
    assert_eq!(
        ranges[0].range,
        tower_lsp::lsp_types::Range::new(Position::new(0, 1), Position::new(0, 1))
    );
    assert!(ranges[0].parent.is_none());
    assert_eq!(ranges[1].range.start, Position::new(3, 0));
    assert!(ranges[1].parent.is_some());
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_handles_navigation_requests() {
    let (mut service, _socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({"capabilities":{}}))
        .id(1)
        .finish();
    let init_response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(
        init_response
            .as_ref()
            .is_some_and(|response| response.is_ok())
    );

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: "flowchart TD\nA-->B\nA-->C\n".to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let definition = Request::build("textDocument/definition")
        .params(
            serde_json::to_value(GotoDefinitionParams {
                text_document_position_params: TextDocumentPositionParams::new(
                    TextDocumentIdentifier { uri: uri.clone() },
                    Position::new(1, 0),
                ),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            })
            .unwrap(),
        )
        .id(2)
        .finish();
    let definition_response = service
        .ready()
        .await
        .unwrap()
        .call(definition)
        .await
        .unwrap();
    let definition_value = definition_response
        .and_then(|response| response.result().cloned())
        .expect("expected definition result");
    let definition: tower_lsp::lsp_types::GotoDefinitionResponse =
        serde_json::from_value(definition_value).unwrap();
    assert!(matches!(
        definition,
        tower_lsp::lsp_types::GotoDefinitionResponse::Scalar(_)
    ));

    let references = Request::build("textDocument/references")
        .params(
            serde_json::to_value(ReferenceParams {
                text_document_position: TextDocumentPositionParams::new(
                    TextDocumentIdentifier { uri: uri.clone() },
                    Position::new(1, 0),
                ),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: ReferenceContext {
                    include_declaration: true,
                },
            })
            .unwrap(),
        )
        .id(3)
        .finish();
    let references_response = service
        .ready()
        .await
        .unwrap()
        .call(references)
        .await
        .unwrap();
    let references_value = references_response
        .and_then(|response| response.result().cloned())
        .expect("expected references result");
    let locations: Vec<tower_lsp::lsp_types::Location> =
        serde_json::from_value(references_value).unwrap();
    assert_eq!(locations.len(), 2);

    let prepare = Request::build("textDocument/prepareRename")
        .params(
            serde_json::to_value(TextDocumentPositionParams::new(
                TextDocumentIdentifier { uri: uri.clone() },
                Position::new(1, 0),
            ))
            .unwrap(),
        )
        .id(4)
        .finish();
    let prepare_response = service.ready().await.unwrap().call(prepare).await.unwrap();
    let prepare_value = prepare_response
        .and_then(|response| response.result().cloned())
        .expect("expected prepare rename result");
    let prepare: PrepareRenameResponse = serde_json::from_value(prepare_value).unwrap();
    assert!(matches!(
        prepare,
        PrepareRenameResponse::RangeWithPlaceholder { .. }
    ));

    let rename = Request::build("textDocument/rename")
        .params(
            serde_json::to_value(RenameParams {
                text_document_position: TextDocumentPositionParams::new(
                    TextDocumentIdentifier { uri },
                    Position::new(1, 0),
                ),
                new_name: "X".to_string(),
                work_done_progress_params: Default::default(),
            })
            .unwrap(),
        )
        .id(5)
        .finish();
    let rename_response = service.ready().await.unwrap().call(rename).await.unwrap();
    let rename_value = rename_response
        .and_then(|response| response.result().cloned())
        .expect("expected rename result");
    let edit: tower_lsp::lsp_types::WorkspaceEdit = serde_json::from_value(rename_value).unwrap();
    assert_eq!(edit.changes.unwrap().values().next().unwrap().len(), 2);
}

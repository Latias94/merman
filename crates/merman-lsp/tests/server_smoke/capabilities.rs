use super::prelude::*;

#[test]
fn published_server_constructors_are_visible_with_legacy_signatures() {
    let _: fn(tower_lsp::Client) -> MermanLanguageServer = MermanLanguageServer::new;
    let _: fn() -> (
        tower_lsp::LspService<MermanLanguageServer>,
        tower_lsp::ClientSocket,
    ) = MermanLanguageServer::service;
}

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
    assert_eq!(
        response
            .capabilities
            .completion_provider
            .as_ref()
            .and_then(|options| options.trigger_characters.as_ref())
            .cloned(),
        Some(vec![
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
    );
    assert!(response.capabilities.code_action_provider.is_none());
    assert!(response.capabilities.semantic_tokens_provider.is_none());
    assert!(response.capabilities.workspace_symbol_provider.is_none());
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
        Some(tower_lsp::lsp_types::TextDocumentSyncCapability::Options(ref options))
            if options.change == Some(tower_lsp::lsp_types::TextDocumentSyncKind::INCREMENTAL)
                && options.open_close == Some(true)
                && options.save.is_some()
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_advertises_only_negotiated_protocol_extensions() {
    let (service, _socket) = MermanLanguageServer::service();

    let response = tower_lsp::LanguageServer::initialize(
        service.inner(),
        serde_json::from_value(serde_json::json!({
            "capabilities": {
                "textDocument": {
                    "codeAction": {
                        "codeActionLiteralSupport": {
                            "codeActionKind": { "valueSet": ["quickfix"] }
                        }
                    },
                    "publishDiagnostics": { "dataSupport": true },
                    "semanticTokens": {
                        "requests": { "range": true, "full": { "delta": true } },
                        "tokenTypes": ["namespace", "class", "struct", "variable", "property", "event", "function", "string"],
                        "tokenModifiers": ["mermanEntity"],
                        "formats": ["relative"]
                    }
                }
            }
        }))
        .unwrap(),
    )
    .await
    .unwrap();

    assert!(response.capabilities.code_action_provider.is_some());
    let semantic_tokens = response
        .capabilities
        .semantic_tokens_provider
        .expect("negotiated semantic tokens provider");
    let serialized = serde_json::to_value(semantic_tokens).unwrap();
    assert_eq!(
        serialized["legend"]["tokenModifiers"],
        serde_json::json!(["mermanEntity"])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_does_not_advertise_semantic_tokens_without_relative_format() {
    let (service, _socket) = MermanLanguageServer::service();

    let response = tower_lsp::LanguageServer::initialize(
        service.inner(),
        serde_json::from_value(serde_json::json!({
            "capabilities": {
                "textDocument": {
                    "semanticTokens": {
                        "requests": { "full": true },
                        "tokenTypes": ["variable"],
                        "tokenModifiers": [],
                        "formats": []
                    }
                }
            }
        }))
        .unwrap(),
    )
    .await
    .unwrap();

    assert!(response.capabilities.semantic_tokens_provider.is_none());
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
                "textDocument": {
                    "publishDiagnostics": { "versionSupport": true }
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

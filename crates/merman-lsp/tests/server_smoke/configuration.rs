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
async fn lsp_service_unchanged_configuration_emits_no_refresh_or_diagnostics() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "textDocument": {
                    "diagnostic": {}
                },
                "workspace": {
                    "diagnostic": {
                        "refreshSupport": true
                    },
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

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri,
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: "gitGraph\ncommit id:\"dup\"\ncommit id:\"dup\"\n".to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let change = Request::build("workspace/didChangeConfiguration")
        .params(
            serde_json::to_value(DidChangeConfigurationParams {
                settings: serde_json::Value::Null,
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
        "unchanged configuration should not emit diagnostics or refresh requests"
    );
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
async fn lsp_service_noop_configuration_change_sends_no_push_or_semantic_refresh() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

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
    let first_diagnostics = timeout(Duration::from_secs(5), socket.next())
        .await
        .unwrap()
        .expect("expected diagnostics after open");
    assert_eq!(
        first_diagnostics.method(),
        "textDocument/publishDiagnostics"
    );

    let change = Request::build("workspace/didChangeConfiguration")
        .params(
            serde_json::to_value(DidChangeConfigurationParams {
                settings: serde_json::Value::Null,
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
        "unchanged configuration should not publish diagnostics or refresh semantic tokens"
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
async fn lsp_service_diagnostic_pull_refresh_does_not_push_open_documents() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

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

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: "gitGraph\ncommit id:\"dup\"\ncommit id:\"dup\"\n".to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
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
    assert!(
        timeout(Duration::from_millis(50), socket.next())
            .await
            .is_err(),
        "unexpected publishDiagnostics message in diagnostic-pull mode"
    );

    let request = Request::build("textDocument/diagnostic")
        .params(
            serde_json::to_value(DocumentDiagnosticParams {
                text_document: TextDocumentIdentifier { uri },
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
    let DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(full)) = result
    else {
        panic!("expected full diagnostic report");
    };
    assert!(
        full.full_document_diagnostic_report.items.is_empty(),
        "duplicate-commit diagnostic should be gone after the rule is disabled"
    );
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
                    "disable_rules": ["merman.parse.no_diagram"]
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
    assert_eq!(resource_params.diagnostics.len(), 1);
    assert_eq!(
        resource_params.diagnostics[0]
            .code
            .as_ref()
            .and_then(|code| match code {
                NumberOrString::String(value) => Some(value.as_str()),
                NumberOrString::Number(_) => None,
            }),
        Some("merman.resource.source_bytes_exceeded")
    );
    assert_eq!(
        resource_params.diagnostics[0].severity,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_rejects_resource_rule_severity_on_initialize() {
    let (mut service, _socket) = MermanLanguageServer::service();

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
    let response = init_response.expect("initialize response");
    assert!(response.is_error());
    let error = response.error().expect("initialize error");
    assert_eq!(error.code, ErrorCode::InvalidParams);
    assert!(
        error.message.contains(
            "lint.rule_severities.rule_id entry `merman.resource.source_bytes_exceeded` must reference a configurable analysis rule id"
        ),
        "unexpected initialize error: {}",
        error.message
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_rejects_resource_rule_severity_on_configuration_change() {
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

    let log = socket.next().await.expect("expected invalid settings log");
    assert_eq!(log.method(), "window/logMessage");
    let log_params: LogMessageParams =
        from_value(log.params().cloned().expect("log params")).unwrap();
    assert!(
        log_params
            .message
            .contains("invalid merman analysis settings")
    );
    assert!(
        log_params
            .message
            .contains("merman.resource.source_bytes_exceeded")
    );

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

    let second = socket.next().await.expect("expected saved diagnostics");
    let second_params: PublishDiagnosticsParams =
        from_value(second.params().cloned().expect("publish params")).unwrap();
    assert_eq!(second_params.version, Some(1));
    assert_eq!(second_params.diagnostics.len(), 1);
    assert_eq!(
        second_params.diagnostics[0].severity,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR)
    );
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

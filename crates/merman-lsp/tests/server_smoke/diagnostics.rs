use super::prelude::*;

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
async fn lsp_service_smoke_reports_deprecated_flowchart_html_labels_without_quickfix() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "workspace": {
                    "workspaceEdit": {
                        "documentChanges": true
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
    assert!(result.is_null());
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
async fn lsp_service_smoke_publishes_sync_error_after_invalid_incremental_range() {
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

    let opened = socket.next().await.expect("expected initial diagnostics");
    let opened_params: PublishDiagnosticsParams =
        from_value(opened.params().cloned().expect("publish params")).unwrap();
    assert_eq!(opened.method(), "textDocument/publishDiagnostics");
    assert_eq!(opened_params.version, Some(1));
    assert!(opened_params.diagnostics.is_empty());

    let invalid_change = Request::build("textDocument/didChange")
        .params(
            serde_json::to_value(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: Some(Range {
                        start: Position::new(0, 100),
                        end: Position::new(0, 100),
                    }),
                    range_length: None,
                    text: "x".to_string(),
                }],
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service
            .ready()
            .await
            .unwrap()
            .call(invalid_change)
            .await
            .unwrap(),
        None
    );

    let sync_lost = socket
        .next()
        .await
        .expect("expected sync error diagnostics");
    let sync_lost_params: PublishDiagnosticsParams =
        from_value(sync_lost.params().cloned().expect("publish params")).unwrap();
    assert_eq!(sync_lost.method(), "textDocument/publishDiagnostics");
    assert_eq!(sync_lost_params.uri, uri);
    assert_eq!(sync_lost_params.version, Some(2));
    assert_eq!(sync_lost_params.diagnostics.len(), 1);
    let diagnostic = &sync_lost_params.diagnostics[0];
    assert_eq!(
        diagnostic.severity,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR)
    );
    assert_eq!(
        diagnostic.code,
        Some(NumberOrString::String(
            "merman.lsp.document_sync_lost".to_string()
        ))
    );

    let replacement = Request::build("textDocument/didChange")
        .params(
            serde_json::to_value(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 3,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "flowchart TD\nA-->B\n".to_string(),
                }],
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service
            .ready()
            .await
            .unwrap()
            .call(replacement)
            .await
            .unwrap(),
        None
    );

    let recovered = socket.next().await.expect("expected recovered diagnostics");
    let recovered_params: PublishDiagnosticsParams =
        from_value(recovered.params().cloned().expect("publish params")).unwrap();
    assert_eq!(recovered.method(), "textDocument/publishDiagnostics");
    assert_eq!(recovered_params.uri, uri);
    assert_eq!(recovered_params.version, Some(3));
    assert!(recovered_params.diagnostics.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_pull_reports_sync_error_after_invalid_incremental_range() {
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

    let invalid_change = Request::build("textDocument/didChange")
        .params(
            serde_json::to_value(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 2,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: Some(Range {
                        start: Position::new(0, 100),
                        end: Position::new(0, 100),
                    }),
                    range_length: None,
                    text: "x".to_string(),
                }],
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service
            .ready()
            .await
            .unwrap()
            .call(invalid_change)
            .await
            .unwrap(),
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
    let DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(sync_lost)) = result
    else {
        panic!("expected full sync-lost diagnostic report");
    };
    assert_eq!(sync_lost.full_document_diagnostic_report.items.len(), 1);
    let diagnostic = &sync_lost.full_document_diagnostic_report.items[0];
    assert_eq!(
        diagnostic.severity,
        Some(tower_lsp::lsp_types::DiagnosticSeverity::ERROR)
    );
    assert_eq!(
        diagnostic.code,
        Some(NumberOrString::String(
            "merman.lsp.document_sync_lost".to_string()
        ))
    );

    let replacement = Request::build("textDocument/didChange")
        .params(
            serde_json::to_value(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: uri.clone(),
                    version: 3,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: "flowchart TD\nA-->B\n".to_string(),
                }],
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service
            .ready()
            .await
            .unwrap()
            .call(replacement)
            .await
            .unwrap(),
        None
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
        .id(3)
        .finish();
    let response = service
        .ready()
        .await
        .unwrap()
        .call(request)
        .await
        .unwrap()
        .expect("recovered document diagnostic response");
    let result: DocumentDiagnosticReportResult = from_value(
        response
            .result()
            .cloned()
            .expect("recovered document diagnostic result"),
    )
    .unwrap();
    let DocumentDiagnosticReportResult::Report(DocumentDiagnosticReport::Full(recovered)) = result
    else {
        panic!("expected recovered full diagnostic report");
    };
    assert!(recovered.full_document_diagnostic_report.items.is_empty());
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

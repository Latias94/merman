use super::prelude::*;

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_handles_hover_and_document_symbols() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "textDocument": {
                    "hover": {
                        "contentFormat": ["markdown"]
                    },
                    "documentSymbol": {
                        "hierarchicalDocumentSymbolSupport": true
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
async fn lsp_service_projects_hover_as_negotiated_plain_text() {
    let (mut service, mut socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/plain-hover.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "textDocument": {
                    "hover": {
                        "contentFormat": ["plaintext"]
                    }
                }
            }
        }))
        .id(1)
        .finish();
    let response = service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();
    assert!(response.as_ref().is_some_and(|response| response.is_ok()));

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
    socket.next().await.expect("expected diagnostics");

    let hover = Request::build("textDocument/hover")
        .params(
            serde_json::to_value(HoverParams {
                text_document_position_params: TextDocumentPositionParams::new(
                    TextDocumentIdentifier { uri },
                    Position::new(1, 0),
                ),
                work_done_progress_params: Default::default(),
            })
            .unwrap(),
        )
        .id(2)
        .finish();
    let hover_value = service
        .ready()
        .await
        .unwrap()
        .call(hover)
        .await
        .unwrap()
        .and_then(|response| response.result().cloned())
        .expect("expected hover result");
    let hover: tower_lsp::lsp_types::Hover = serde_json::from_value(hover_value).unwrap();
    let markup = match hover.contents {
        HoverContents::Markup(markup) => markup,
        other => panic!("unexpected hover contents: {other:?}"),
    };

    assert_eq!(markup.kind, tower_lsp::lsp_types::MarkupKind::PlainText);
    assert!(markup.value.contains('A'));
    assert!(!markup.value.contains('`'));
    assert!(!markup.value.contains("### "));
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
async fn lsp_service_smoke_returns_folding_ranges_over_json_rpc() {
    let (mut service, _socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/folding.md").unwrap();

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
                    language_id: "markdown".to_string(),
                    version: 1,
                    text:
                        "before\n```mermaid\nflowchart TD\nsubgraph group\nA-->B\nend\n```\nafter\n"
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

    let request = Request::build("textDocument/foldingRange")
        .params(
            serde_json::to_value(FoldingRangeParams {
                text_document: TextDocumentIdentifier { uri },
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
        .expect("folding range response");
    let ranges: Vec<FoldingRange> =
        from_value(response.result().cloned().expect("folding range result")).unwrap();

    assert!(
        ranges.iter().any(|range| {
            range.start_line == 1
                && range.end_line == 6
                && range.kind == Some(FoldingRangeKind::Region)
        }),
        "expected Mermaid fence fold, got {ranges:#?}"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_handles_navigation_requests() {
    let (mut service, _socket) = MermanLanguageServer::service();
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
    assert!(edit.changes.is_none());
    let document_changes = match edit.document_changes.unwrap() {
        DocumentChanges::Edits(edits) => edits,
        other => panic!("unexpected document changes: {other:?}"),
    };
    assert_eq!(document_changes.len(), 1);
    assert_eq!(document_changes[0].text_document.version, Some(1));
    assert_eq!(document_changes[0].edits.len(), 2);
}

use super::prelude::*;

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_resolves_completion_items() {
    let (mut service, _socket) = MermanLanguageServer::service();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "textDocument": {
                    "completion": {
                        "completionItem": {
                            "documentationFormat": ["markdown"]
                        }
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
async fn completion_without_snippet_support_never_returns_snippet_placeholders() {
    let (mut service, _socket) = MermanLanguageServer::service();
    let uri = tower_lsp::lsp_types::Url::parse("file:///tmp/example.mmd").unwrap();

    let initialize = Request::build("initialize")
        .params(serde_json::json!({
            "capabilities": {
                "textDocument": {
                    "completion": { "completionItem": { "snippetSupport": false } }
                }
            }
        }))
        .id(1)
        .finish();
    service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap()
        .expect("initialize response");

    let open = Request::build("textDocument/didOpen")
        .params(
            serde_json::to_value(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: uri.clone(),
                    language_id: "mermaid".to_string(),
                    version: 1,
                    text: "flow".to_string(),
                },
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(open).await.unwrap(),
        None
    );

    let completion = Request::build("textDocument/completion")
        .params(serde_json::json!({
            "textDocument": { "uri": uri },
            "position": { "line": 0, "character": 4 }
        }))
        .id(2)
        .finish();
    let response = service
        .ready()
        .await
        .unwrap()
        .call(completion)
        .await
        .unwrap()
        .expect("completion response");
    let result = response.result().expect("completion result");
    let items = result["items"].as_array().expect("completion items");

    assert!(!items.is_empty());
    assert!(items.iter().all(|item| item["insertTextFormat"] != 2));
    assert!(items.iter().all(|item| {
        item["insertText"]
            .as_str()
            .is_none_or(|text| !text.contains("${"))
            && item["textEdit"]["newText"]
                .as_str()
                .is_none_or(|text| !text.contains("${"))
    }));
}

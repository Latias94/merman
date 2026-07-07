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

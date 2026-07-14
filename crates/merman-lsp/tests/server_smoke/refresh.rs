use super::prelude::*;

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_smoke_refreshes_semantic_tokens_after_configuration_change() {
    let (mut service, mut socket) = MermanLanguageServer::service_with_refresh();

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
    assert_eq!(
        timeout(
            Duration::from_secs(1),
            service.ready().await.unwrap().call(change)
        )
        .await
        .expect("configuration handler should not wait for refresh response")
        .unwrap(),
        None
    );
    let refresh = timeout(Duration::from_secs(1), socket.next())
        .await
        .expect("expected semantic tokens refresh request")
        .expect("refresh channel closed");
    assert_eq!(refresh.method(), "workspace/semanticTokens/refresh");

    socket
        .send(tower_lsp::jsonrpc::Response::from_ok(
            refresh.id().cloned().expect("refresh request id"),
            serde_json::Value::Null,
        ))
        .await
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn lsp_service_coalesces_refreshes_while_client_response_is_pending() {
    let (mut service, mut socket) = MermanLanguageServer::service_with_refresh();

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
    service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();

    let configuration_change = |suppress_errors| {
        Request::build("workspace/didChangeConfiguration")
            .params(
                serde_json::to_value(DidChangeConfigurationParams {
                    settings: serde_json::json!({
                        "parse": { "suppress_errors": suppress_errors }
                    }),
                })
                .unwrap(),
            )
            .finish()
    };

    assert_eq!(
        service
            .ready()
            .await
            .unwrap()
            .call(configuration_change(true))
            .await
            .unwrap(),
        None
    );
    let first = timeout(Duration::from_secs(1), socket.next())
        .await
        .expect("expected first refresh")
        .expect("refresh channel closed");
    assert_eq!(first.method(), "workspace/semanticTokens/refresh");

    assert_eq!(
        service
            .ready()
            .await
            .unwrap()
            .call(configuration_change(false))
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        service
            .ready()
            .await
            .unwrap()
            .call(configuration_change(true))
            .await
            .unwrap(),
        None
    );
    assert!(
        timeout(Duration::from_millis(50), socket.next())
            .await
            .is_err()
    );

    socket
        .send(tower_lsp::jsonrpc::Response::from_ok(
            first.id().cloned().expect("first refresh request id"),
            serde_json::Value::Null,
        ))
        .await
        .unwrap();
    let follow_up = timeout(Duration::from_secs(1), socket.next())
        .await
        .expect("expected coalesced refresh")
        .expect("refresh channel closed");
    assert_eq!(follow_up.method(), "workspace/semanticTokens/refresh");
    socket
        .send(tower_lsp::jsonrpc::Response::from_ok(
            follow_up
                .id()
                .cloned()
                .expect("follow-up refresh request id"),
            serde_json::Value::Null,
        ))
        .await
        .unwrap();

    assert!(
        timeout(Duration::from_millis(50), socket.next())
            .await
            .is_err()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn pending_semantic_refresh_does_not_block_diagnostic_refresh() {
    let (mut service, mut socket) = MermanLanguageServer::service_with_refresh();

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
    service
        .ready()
        .await
        .unwrap()
        .call(initialize)
        .await
        .unwrap();

    let change = Request::build("workspace/didChangeConfiguration")
        .params(
            serde_json::to_value(DidChangeConfigurationParams {
                settings: serde_json::json!({
                    "parse": { "suppress_errors": true }
                }),
            })
            .unwrap(),
        )
        .finish();
    assert_eq!(
        service.ready().await.unwrap().call(change).await.unwrap(),
        None
    );

    let mut refreshes = Vec::new();
    for _ in 0..2 {
        refreshes.push(
            timeout(Duration::from_secs(1), socket.next())
                .await
                .expect("both refresh kinds should run independently")
                .expect("refresh channel closed"),
        );
    }
    let methods = refreshes
        .iter()
        .map(|refresh| refresh.method())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        methods,
        std::collections::BTreeSet::from([
            "workspace/diagnostic/refresh",
            "workspace/semanticTokens/refresh",
        ])
    );

    for refresh in refreshes {
        socket
            .send(tower_lsp::jsonrpc::Response::from_ok(
                refresh.id().cloned().expect("refresh request id"),
                serde_json::Value::Null,
            ))
            .await
            .unwrap();
    }
}

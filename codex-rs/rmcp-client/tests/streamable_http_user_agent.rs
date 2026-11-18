mod streamable_http_test_support;

use std::collections::HashMap;

use codex_config::types::AuthKeyringBackendKind;
use codex_config::types::OAuthCredentialsStoreMode;
use codex_exec_server::Environment;
use codex_rmcp_client::RmcpClient;
use serde_json::Value;
use serde_json::json;
use streamable_http_test_support::initialize_client;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::Request;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_requests_omit_default_user_agent() -> anyhow::Result<()> {
    check_user_agent(None).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn streamable_http_requests_preserve_configured_user_agent() -> anyhow::Result<()> {
    check_user_agent(Some("custom-agent/9.9")).await
}

async fn check_user_agent(custom_user_agent: Option<&'static str>) -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .and(move |request: &Request| {
            request
                .headers
                .get("user-agent")
                .and_then(|value| value.to_str().ok())
                == custom_user_agent
        })
        .respond_with(|request: &Request| {
            let body: Value = request.body_json().expect("valid JSON-RPC request");
            match body.get("method").and_then(Value::as_str) {
                Some("initialize") => ResponseTemplate::new(200).set_body_json(json!({
                    "jsonrpc": "2.0",
                    "id": body.get("id").cloned().unwrap_or(Value::Null),
                    "result": {
                        "protocolVersion": "2025-06-18",
                        "capabilities": {},
                        "serverInfo": {
                            "name": "user-agent-test",
                            "version": "0.0.0-test",
                        },
                    },
                })),
                Some("notifications/initialized") => ResponseTemplate::new(202),
                method => ResponseTemplate::new(400)
                    .set_body_string(format!("unexpected JSON-RPC method: {method:?}")),
            }
        })
        .expect(2)
        .mount(&server)
        .await;

    let custom_client = RmcpClient::new_streamable_http_client(
        "test-streamable-http",
        &format!("{}/mcp", server.uri()),
        Some("test-bearer".to_string()),
        custom_user_agent
            .map(|value| HashMap::from([("user-agent".to_string(), value.to_string())])),
        /*env_http_headers*/ None,
        OAuthCredentialsStoreMode::File,
        AuthKeyringBackendKind::default(),
        Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
    )
    .await?;
    initialize_client(&custom_client).await?;
    server.verify().await;

    Ok(())
}

use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use wiremock::matchers::{method, path};
use wiremock::{Match, Mock, MockServer, ResponseTemplate};

use cmdify::config::{AuthStyle, Config, ProviderSettings};
use cmdify::orchestrator;

fn make_config(base_url: &str) -> Config {
    Config {
        provider_name: "completions".into(),
        model_name: "test-model".into(),
        max_tokens: 4096,
        system_prompt_override: None,
        spinner: 1,
        allow_unsafe: false,
        quiet: false,
        blind: false,
        no_tools: false,
        yolo: false,
        debug_level: 0,
        tool_level: 1,
        provider_settings: ProviderSettings {
            api_key: None,
            base_url: base_url.into(),
            auth_style: AuthStyle::Header {
                name: "Authorization".into(),
                prefix: "Bearer ".into(),
            },
        },
    }
}

struct NthRequest {
    count: AtomicUsize,
    target: usize,
}

impl NthRequest {
    fn new(target: usize) -> Self {
        Self {
            count: AtomicUsize::new(0),
            target,
        }
    }
}

impl Match for NthRequest {
    fn matches(&self, _request: &wiremock::Request) -> bool {
        self.count.fetch_add(1, Ordering::SeqCst) == self.target
    }
}

#[tokio::test]
async fn single_shot_no_tool_calls() {
    let server = MockServer::start().await;
    let config = make_config(&server.uri());

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "role": "assistant", "content": "find . -name '*.txt'" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let result = orchestrator::run("find all text files", &config, None, None)
        .await
        .unwrap();
    assert_eq!(result, "find . -name '*.txt'");
}

#[tokio::test]
async fn tool_call_loop_returns_final_answer() {
    let server = MockServer::start().await;
    let config = make_config(&server.uri());

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "role": "assistant", "content": "ls -la" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(NthRequest::new(0))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": { "name": "find_command", "arguments": "{\"command\":\"ls\"}" }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        })))
        .mount(&server)
        .await;

    let result = orchestrator::run("list all files", &config, None, None)
        .await
        .unwrap();
    assert_eq!(result, "ls -la");
}

#[tokio::test]
async fn no_tools_with_blind_flag() {
    let server = MockServer::start().await;
    let mut config = make_config(&server.uri());
    config.blind = true;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "role": "assistant", "content": "ls -la" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let result = orchestrator::run("list files", &config, None, None)
        .await
        .unwrap();
    assert_eq!(result, "ls -la");
}

#[tokio::test]
async fn no_tools_with_no_tools_flag() {
    let server = MockServer::start().await;
    let mut config = make_config(&server.uri());
    config.no_tools = true;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "role": "assistant", "content": "ls" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let result = orchestrator::run("list files", &config, None, None)
        .await
        .unwrap();
    assert_eq!(result, "ls");
}

#[tokio::test]
async fn empty_response_errors() {
    let server = MockServer::start().await;
    let config = make_config(&server.uri());

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "role": "assistant", "content": null },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let result = orchestrator::run("test", &config, None, None).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("empty response from provider"));
}

#[tokio::test]
async fn provider_error_propagates() {
    let server = MockServer::start().await;
    let config = make_config(&server.uri());

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(500).set_body_json(json!({
            "error": { "message": "internal server error" }
        })))
        .mount(&server)
        .await;

    let result = orchestrator::run("test", &config, None, None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn no_tools_with_quiet_flag() {
    let server = MockServer::start().await;
    let mut config = make_config(&server.uri());
    config.quiet = true;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "role": "assistant", "content": "ls" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let result = orchestrator::run("list files", &config, None, None)
        .await
        .unwrap();
    assert_eq!(result, "ls");
}

#[tokio::test]
async fn max_iterations_exceeded() {
    let server = MockServer::start().await;
    let config = make_config(&server.uri());

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_loop",
                        "type": "function",
                        "function": { "name": "find_command", "arguments": "{\"command\":\"ls\"}" }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        })))
        .mount(&server)
        .await;

    let result = orchestrator::run("keep asking", &config, None, None).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("tool call loop exceeded maximum iterations"),
        "expected max iterations error, got: {}",
        err
    );
}

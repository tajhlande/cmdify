use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use cmdify::config::{AuthStyle, Config, ProviderSettings};
use cmdify::error::Error;
use cmdify::provider::{create_provider, Message, ProviderResponse, ToolDefinition};
use wiremock::Match;

fn make_config(base_url: &str, api_key: Option<&str>, model: &str) -> Config {
    Config {
        provider_name: "completions".into(),
        model_name: model.into(),
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
            api_key: api_key.map(|k| k.into()),
            base_url: base_url.into(),
            auth_style: AuthStyle::Header {
                name: "Authorization".into(),
                prefix: "Bearer ".into(),
            },
        },
    }
}

#[tokio::test]
async fn successful_completion_response() {
    let server = MockServer::start().await;
    let config = make_config(&server.uri(), Some("test-key"), "test-model");

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "role": "assistant", "content": "find . -name '*.pdf'" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let provider = create_provider(&config).unwrap();
    let messages = vec![
        Message::System {
            content: "You are helpful.".into(),
        },
        Message::User {
            content: "find pdf files".into(),
        },
    ];

    let response: ProviderResponse = provider.send_request(&messages, &[]).await.unwrap();
    assert_eq!(response.content, Some("find . -name '*.pdf'".into()));
    assert!(response.tool_calls.is_empty());
}

#[tokio::test]
async fn successful_completion_no_auth() {
    let server = MockServer::start().await;
    let config = make_config(&server.uri(), None, "test-model");

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

    let provider = create_provider(&config).unwrap();
    let messages = vec![Message::User {
        content: "list files".into(),
    }];

    let response: ProviderResponse = provider.send_request(&messages, &[]).await.unwrap();
    assert_eq!(response.content, Some("ls -la".into()));
}

#[tokio::test]
async fn api_error_response() {
    let server = MockServer::start().await;
    let config = make_config(&server.uri(), Some("bad-key"), "test-model");

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": { "message": "Invalid API key" }
        })))
        .mount(&server)
        .await;

    let provider = create_provider(&config).unwrap();
    let messages = vec![Message::User {
        content: "test".into(),
    }];

    let result: std::result::Result<ProviderResponse, Error> =
        provider.send_request(&messages, &[]).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid API key"));
}

#[tokio::test]
async fn provider_name_is_completions() {
    let config = make_config("http://localhost:11434", None, "test-model");
    let provider = create_provider(&config).unwrap();
    assert_eq!(provider.name(), "completions");
}

#[tokio::test]
async fn supports_tools_true() {
    let config = make_config("http://localhost:11434", None, "test-model");
    let provider = create_provider(&config).unwrap();
    assert!(provider.supports_tools());
}

struct ToolsBodyMatcher;

impl Match for ToolsBodyMatcher {
    fn matches(&self, request: &wiremock::Request) -> bool {
        let body: serde_json::Value = match serde_json::from_slice(&request.body) {
            Ok(v) => v,
            Err(_) => return false,
        };
        let tools = match body.get("tools").and_then(|t| t.as_array()) {
            Some(arr) => arr,
            None => return false,
        };
        if tools.is_empty() {
            return false;
        }
        let first = &tools[0];
        let tool_type = first.get("type").and_then(|t| t.as_str());
        let func = first.get("function");
        let func_name = func.and_then(|f| f.get("name")).and_then(|n| n.as_str());
        let func_desc = func
            .and_then(|f| f.get("description"))
            .and_then(|d| d.as_str());
        let func_params = func.and_then(|f| f.get("parameters"));
        tool_type == Some("function")
            && func_name == Some("find_command")
            && func_desc.map(|d| !d.is_empty()).unwrap_or(false)
            && func_params.is_some()
    }
}

struct NoToolsInBody;

impl Match for NoToolsInBody {
    fn matches(&self, request: &wiremock::Request) -> bool {
        let body: serde_json::Value = match serde_json::from_slice(&request.body) {
            Ok(v) => v,
            Err(_) => return false,
        };
        body.get("tools").is_none()
    }
}

#[tokio::test]
async fn tools_included_in_request_when_provided() {
    let server = MockServer::start().await;
    let config = make_config(&server.uri(), None, "test-model");

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(ToolsBodyMatcher)
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "role": "assistant", "content": "ls -la" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let provider = create_provider(&config).unwrap();
    let tools = vec![ToolDefinition {
        name: "find_command".into(),
        description: "Check if a command exists on the system".into(),
        parameters: json!({
            "type": "object",
            "properties": {"command": {"type": "string"}},
            "required": ["command"]
        }),
    }];
    let messages = vec![Message::User {
        content: "find files".into(),
    }];

    let response = provider.send_request(&messages, &tools).await.unwrap();
    assert_eq!(response.content, Some("ls -la".into()));
}

#[tokio::test]
async fn no_tools_key_when_tools_empty() {
    let server = MockServer::start().await;
    let config = make_config(&server.uri(), None, "test-model");

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(NoToolsInBody)
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "role": "assistant", "content": "ls" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let provider = create_provider(&config).unwrap();
    let messages = vec![Message::User {
        content: "list files".into(),
    }];

    let response = provider.send_request(&messages, &[]).await.unwrap();
    assert_eq!(response.content, Some("ls".into()));
}

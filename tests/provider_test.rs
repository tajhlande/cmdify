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

fn make_named_config(
    provider_name: &str,
    base_url: &str,
    api_key: Option<&str>,
    model: &str,
) -> Config {
    Config {
        provider_name: provider_name.into(),
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

// --- Completions provider tests ---

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

// --- OpenRouter tests ---

#[tokio::test]
async fn openrouter_successful_completion() {
    let server = MockServer::start().await;
    let config = make_named_config(
        "openrouter",
        &server.uri(),
        Some("or-test-key"),
        "openai/gpt-4",
    );

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "role": "assistant", "content": "find . -name '*.rs'" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let provider = create_provider(&config).unwrap();
    let messages = vec![Message::User {
        content: "find rust files".into(),
    }];

    let response: ProviderResponse = provider.send_request(&messages, &[]).await.unwrap();
    assert_eq!(response.content, Some("find . -name '*.rs'".into()));
}

#[tokio::test]
async fn openrouter_missing_api_key() {
    let config = make_named_config(
        "openrouter",
        "https://openrouter.ai/api",
        None,
        "openai/gpt-4",
    );
    let result = create_provider(&config);
    let err = match result {
        Err(e) => e.to_string(),
        Ok(_) => panic!("expected error"),
    };
    assert!(err.contains("OPENROUTER_API_KEY"));
}

#[tokio::test]
async fn openrouter_provider_name() {
    let config = make_named_config(
        "openrouter",
        "https://openrouter.ai/api",
        Some("key"),
        "openai/gpt-4",
    );
    let provider = create_provider(&config).unwrap();
    assert_eq!(provider.name(), "openrouter");
}

#[tokio::test]
async fn openrouter_supports_tools() {
    let config = make_named_config(
        "openrouter",
        "https://openrouter.ai/api",
        Some("key"),
        "openai/gpt-4",
    );
    let provider = create_provider(&config).unwrap();
    assert!(provider.supports_tools());
}

#[tokio::test]
async fn openrouter_tool_call_response() {
    let server = MockServer::start().await;
    let config = make_named_config("openrouter", &server.uri(), Some("or-key"), "openai/gpt-4");

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_or1",
                        "type": "function",
                        "function": { "name": "find_command", "arguments": "{\"command\":\"rg\"}" }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        })))
        .mount(&server)
        .await;

    let provider = create_provider(&config).unwrap();
    let messages = vec![Message::User {
        content: "search in files".into(),
    }];
    let tools = vec![ToolDefinition {
        name: "find_command".into(),
        description: "Find a command".into(),
        parameters: json!({"type": "object"}),
    }];

    let response: ProviderResponse = provider.send_request(&messages, &tools).await.unwrap();
    assert!(response.content.is_none());
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(response.tool_calls[0].name, "find_command");
}

// --- HuggingFace tests ---

#[tokio::test]
async fn huggingface_successful_completion() {
    let server = MockServer::start().await;
    let config = make_named_config(
        "huggingface",
        &server.uri(),
        Some("hf-test-key"),
        "mistralai/Mistral-7B-Instruct-v0.3",
    );

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "role": "assistant", "content": "ls -la /home" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let provider = create_provider(&config).unwrap();
    let messages = vec![Message::User {
        content: "list home directory".into(),
    }];

    let response: ProviderResponse = provider.send_request(&messages, &[]).await.unwrap();
    assert_eq!(response.content, Some("ls -la /home".into()));
}

#[tokio::test]
async fn huggingface_missing_api_key() {
    let config = make_named_config(
        "huggingface",
        "https://router.huggingface.co/v1",
        None,
        "mistralai/Mistral-7B-Instruct-v0.3",
    );
    let result = create_provider(&config);
    let err = match result {
        Err(e) => e.to_string(),
        Ok(_) => panic!("expected error"),
    };
    assert!(err.contains("HUGGINGFACE_API_KEY"));
}

#[tokio::test]
async fn huggingface_provider_name() {
    let config = make_named_config(
        "huggingface",
        "https://router.huggingface.co/v1",
        Some("key"),
        "mistralai/Mistral-7B-Instruct-v0.3",
    );
    let provider = create_provider(&config).unwrap();
    assert_eq!(provider.name(), "huggingface");
}

#[tokio::test]
async fn huggingface_supports_tools() {
    let config = make_named_config(
        "huggingface",
        "https://router.huggingface.co/v1",
        Some("key"),
        "mistralai/Mistral-7B-Instruct-v0.3",
    );
    let provider = create_provider(&config).unwrap();
    assert!(provider.supports_tools());
}

#[tokio::test]
async fn huggingface_api_error() {
    let server = MockServer::start().await;
    let config = make_named_config(
        "huggingface",
        &server.uri(),
        Some("bad-hf-key"),
        "mistralai/Mistral-7B-Instruct-v0.3",
    );

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "error": "Authorization header is missing or invalid"
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
}

// --- User-Agent header tests ---

struct UserAgentMatcher {
    expected_prefix: &'static str,
}

impl UserAgentMatcher {
    fn new(prefix: &'static str) -> Self {
        Self {
            expected_prefix: prefix,
        }
    }
}

impl Match for UserAgentMatcher {
    fn matches(&self, request: &wiremock::Request) -> bool {
        request
            .headers
            .get("user-agent")
            .map(|v| v.to_str().unwrap_or("").starts_with(self.expected_prefix))
            .unwrap_or(false)
    }
}

#[tokio::test]
async fn user_agent_header_sent_by_completions() {
    let server = MockServer::start().await;
    let config = make_config(&server.uri(), Some("key"), "model");

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(UserAgentMatcher::new("cmdify/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "role": "assistant", "content": "echo hi" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let provider = create_provider(&config).unwrap();
    let messages = vec![Message::User {
        content: "say hi".into(),
    }];
    let response = provider.send_request(&messages, &[]).await.unwrap();
    assert_eq!(response.content, Some("echo hi".into()));
}

#[tokio::test]
async fn user_agent_header_sent_by_openrouter() {
    let server = MockServer::start().await;
    let config = make_named_config("openrouter", &server.uri(), Some("key"), "model");

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .and(UserAgentMatcher::new("cmdify/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "role": "assistant", "content": "echo hi" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let provider = create_provider(&config).unwrap();
    let messages = vec![Message::User {
        content: "say hi".into(),
    }];
    let response = provider.send_request(&messages, &[]).await.unwrap();
    assert_eq!(response.content, Some("echo hi".into()));
}

#[tokio::test]
async fn user_agent_header_sent_by_huggingface() {
    let server = MockServer::start().await;
    let config = make_named_config("huggingface", &server.uri(), Some("key"), "model");

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(UserAgentMatcher::new("cmdify/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "role": "assistant", "content": "echo hi" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let provider = create_provider(&config).unwrap();
    let messages = vec![Message::User {
        content: "say hi".into(),
    }];
    let response = provider.send_request(&messages, &[]).await.unwrap();
    assert_eq!(response.content, Some("echo hi".into()));
}

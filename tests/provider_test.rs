use serde_json::json;
use wiremock::matchers::{method, path, path_regex};
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

fn make_query_param_config(
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
            auth_style: AuthStyle::QueryParam { name: "key".into() },
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

// --- Responses provider tests ---

#[tokio::test]
async fn responses_successful_completion() {
    let server = MockServer::start().await;
    let config = make_named_config("responses", &server.uri(), Some("test-key"), "gpt-5");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{ "type": "output_text", "text": "find . -name '*.pdf'" }]
                }
            ]
        })))
        .mount(&server)
        .await;

    let provider = create_provider(&config).unwrap();
    let messages = vec![Message::User {
        content: "find pdf files".into(),
    }];

    let response: ProviderResponse = provider.send_request(&messages, &[]).await.unwrap();
    assert_eq!(response.content, Some("find . -name '*.pdf'".into()));
    assert!(response.tool_calls.is_empty());
}

#[tokio::test]
async fn responses_provider_name() {
    let config = make_named_config("responses", "https://api.example.com", Some("key"), "model");
    let provider = create_provider(&config).unwrap();
    assert_eq!(provider.name(), "responses");
}

#[tokio::test]
async fn responses_supports_tools() {
    let config = make_named_config("responses", "https://api.example.com", Some("key"), "model");
    let provider = create_provider(&config).unwrap();
    assert!(provider.supports_tools());
}

#[tokio::test]
async fn responses_tool_call_response() {
    let server = MockServer::start().await;
    let config = make_named_config("responses", &server.uri(), Some("key"), "gpt-5");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_resp1",
                    "name": "find_command",
                    "arguments": "{\"command\":\"rg\"}"
                }
            ]
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
    assert_eq!(response.tool_calls[0].id, "call_resp1");
}

#[tokio::test]
async fn responses_api_error() {
    let server = MockServer::start().await;
    let config = make_named_config("responses", &server.uri(), Some("bad-key"), "model");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
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

// --- OpenAI provider tests ---

#[tokio::test]
async fn openai_successful_completion() {
    let server = MockServer::start().await;
    let config = make_named_config("openai", &server.uri(), Some("sk-test-key"), "gpt-5");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{ "type": "output_text", "text": "find . -name '*.rs'" }]
                }
            ]
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
async fn openai_missing_api_key() {
    let config = make_named_config("openai", "https://api.openai.com", None, "gpt-5");
    let result = create_provider(&config);
    let err = match result {
        Err(e) => e.to_string(),
        Ok(_) => panic!("expected error"),
    };
    assert!(err.contains("OPENAI_API_KEY"));
}

#[tokio::test]
async fn openai_provider_name() {
    let config = make_named_config("openai", "https://api.openai.com", Some("key"), "gpt-5");
    let provider = create_provider(&config).unwrap();
    assert_eq!(provider.name(), "openai");
}

#[tokio::test]
async fn openai_supports_tools() {
    let config = make_named_config("openai", "https://api.openai.com", Some("key"), "gpt-5");
    let provider = create_provider(&config).unwrap();
    assert!(provider.supports_tools());
}

#[tokio::test]
async fn openai_tool_call_response() {
    let server = MockServer::start().await;
    let config = make_named_config("openai", &server.uri(), Some("sk-key"), "gpt-5");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_oai1",
                    "name": "find_command",
                    "arguments": "{\"command\":\"rg\"}"
                }
            ]
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
    assert_eq!(response.tool_calls[0].id, "call_oai1");
}

// --- Anthropic provider tests ---

#[tokio::test]
async fn anthropic_successful_completion() {
    let server = MockServer::start().await;
    let config = make_named_config(
        "anthropic",
        &server.uri(),
        Some("sk-ant-key"),
        "claude-sonnet-4-20250514",
    );

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": [
                { "type": "text", "text": "ls -la" }
            ],
            "stop_reason": "end_turn"
        })))
        .mount(&server)
        .await;

    let provider = create_provider(&config).unwrap();
    let messages = vec![
        Message::System {
            content: "You are helpful.".into(),
        },
        Message::User {
            content: "list files".into(),
        },
    ];

    let response: ProviderResponse = provider.send_request(&messages, &[]).await.unwrap();
    assert_eq!(response.content, Some("ls -la".into()));
    assert!(response.tool_calls.is_empty());
}

#[tokio::test]
async fn anthropic_missing_api_key() {
    let config = make_named_config(
        "anthropic",
        "https://api.anthropic.com",
        None,
        "claude-sonnet-4-20250514",
    );
    let result = create_provider(&config);
    let err = match result {
        Err(e) => e.to_string(),
        Ok(_) => panic!("expected error"),
    };
    assert!(err.contains("ANTHROPIC_API_KEY"));
}

#[tokio::test]
async fn anthropic_provider_name() {
    let config = make_named_config(
        "anthropic",
        "https://api.anthropic.com",
        Some("key"),
        "claude-sonnet-4-20250514",
    );
    let provider = create_provider(&config).unwrap();
    assert_eq!(provider.name(), "anthropic");
}

#[tokio::test]
async fn anthropic_supports_tools() {
    let config = make_named_config(
        "anthropic",
        "https://api.anthropic.com",
        Some("key"),
        "claude-sonnet-4-20250514",
    );
    let provider = create_provider(&config).unwrap();
    assert!(provider.supports_tools());
}

#[tokio::test]
async fn anthropic_tool_call_response() {
    let server = MockServer::start().await;
    let config = make_named_config(
        "anthropic",
        &server.uri(),
        Some("ant-key"),
        "claude-sonnet-4-20250514",
    );

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": [
                {
                    "type": "tool_use",
                    "id": "toolu_ant1",
                    "name": "find_command",
                    "input": { "command": "fd" }
                }
            ],
            "stop_reason": "tool_use"
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
    assert_eq!(response.tool_calls[0].id, "toolu_ant1");
}

#[tokio::test]
async fn anthropic_api_error() {
    let server = MockServer::start().await;
    let config = make_named_config(
        "anthropic",
        &server.uri(),
        Some("bad-key"),
        "claude-sonnet-4-20250514",
    );

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "type": "error",
            "error": { "type": "authentication_error", "message": "invalid x-api-key" }
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
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("invalid x-api-key"));
}

// --- Gemini provider tests ---

#[tokio::test]
async fn gemini_successful_completion() {
    let server = MockServer::start().await;
    let config = make_query_param_config(
        "gemini",
        &server.uri(),
        Some("gemini-key"),
        "gemini-2.5-flash",
    );

    Mock::given(method("POST"))
        .and(path_regex(
            r"/v1beta/models/gemini-2.5-flash:generateContent",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{ "text": "ls -la" }]
                },
                "finishReason": "STOP"
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
            content: "list files".into(),
        },
    ];

    let response: ProviderResponse = provider.send_request(&messages, &[]).await.unwrap();
    assert_eq!(response.content, Some("ls -la".into()));
    assert!(response.tool_calls.is_empty());
}

#[tokio::test]
async fn gemini_missing_api_key() {
    let config = make_query_param_config(
        "gemini",
        "https://generativelanguage.googleapis.com",
        None,
        "gemini-2.5-flash",
    );
    let result = create_provider(&config);
    let err = match result {
        Err(e) => e.to_string(),
        Ok(_) => panic!("expected error"),
    };
    assert!(err.contains("GEMINI_API_KEY"));
}

#[tokio::test]
async fn gemini_provider_name() {
    let config = make_query_param_config(
        "gemini",
        "https://generativelanguage.googleapis.com",
        Some("key"),
        "gemini-2.5-flash",
    );
    let provider = create_provider(&config).unwrap();
    assert_eq!(provider.name(), "gemini");
}

#[tokio::test]
async fn gemini_supports_tools() {
    let config = make_query_param_config(
        "gemini",
        "https://generativelanguage.googleapis.com",
        Some("key"),
        "gemini-2.5-flash",
    );
    let provider = create_provider(&config).unwrap();
    assert!(provider.supports_tools());
}

#[tokio::test]
async fn gemini_tool_call_response() {
    let server = MockServer::start().await;
    let config = make_query_param_config(
        "gemini",
        &server.uri(),
        Some("gemini-key"),
        "gemini-2.5-flash",
    );

    Mock::given(method("POST"))
        .and(path_regex(
            r"/v1beta/models/gemini-2.5-flash:generateContent",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{
                        "functionCall": { "name": "find_command", "args": { "command": "fd" } }
                    }]
                },
                "finishReason": "STOP"
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
    assert_eq!(response.tool_calls[0].id, "call_0");
    assert_eq!(
        response.finish_reason,
        cmdify::provider::FinishReason::ToolCalls
    );
}

#[tokio::test]
async fn gemini_stop_for_text_not_tool_calls() {
    let server = MockServer::start().await;
    let config = make_query_param_config(
        "gemini",
        &server.uri(),
        Some("gemini-key"),
        "gemini-2.5-flash",
    );

    Mock::given(method("POST"))
        .and(path_regex(
            r"/v1beta/models/gemini-2.5-flash:generateContent",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{ "text": "fd -e pdf" }]
                },
                "finishReason": "STOP"
            }]
        })))
        .mount(&server)
        .await;

    let provider = create_provider(&config).unwrap();
    let messages = vec![Message::User {
        content: "find pdfs".into(),
    }];

    let response: ProviderResponse = provider.send_request(&messages, &[]).await.unwrap();
    assert_eq!(response.content, Some("fd -e pdf".into()));
    assert_eq!(response.finish_reason, cmdify::provider::FinishReason::Stop);
}

#[tokio::test]
async fn gemini_api_error() {
    let server = MockServer::start().await;
    let config =
        make_query_param_config("gemini", &server.uri(), Some("bad-key"), "gemini-2.5-flash");

    Mock::given(method("POST"))
        .and(path_regex(
            r"/v1beta/models/gemini-2.5-flash:generateContent",
        ))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": { "code": 400, "message": "API key not valid", "status": "INVALID_ARGUMENT" }
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
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("API key not valid"));
}

// --- User-Agent for new providers ---

#[tokio::test]
async fn user_agent_header_sent_by_openai() {
    let server = MockServer::start().await;
    let config = make_named_config("openai", &server.uri(), Some("key"), "model");

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .and(UserAgentMatcher::new("cmdify/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{ "type": "output_text", "text": "echo hi" }]
                }
            ]
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
async fn user_agent_header_sent_by_anthropic() {
    let server = MockServer::start().await;
    let config = make_named_config("anthropic", &server.uri(), Some("key"), "model");

    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(UserAgentMatcher::new("cmdify/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "content": [{ "type": "text", "text": "echo hi" }],
            "stop_reason": "end_turn"
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
async fn user_agent_header_sent_by_gemini() {
    let server = MockServer::start().await;
    let config = make_query_param_config("gemini", &server.uri(), Some("key"), "model");

    Mock::given(method("POST"))
        .and(path_regex(r"/v1beta/models/model:generateContent"))
        .and(UserAgentMatcher::new("cmdify/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "candidates": [{
                "content": { "role": "model", "parts": [{ "text": "echo hi" }] },
                "finishReason": "STOP"
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

// --- Z.ai provider tests ---

#[tokio::test]
async fn zai_successful_completion() {
    let server = MockServer::start().await;
    let config = make_named_config("zai", &server.uri(), Some("zai-key"), "glm-4");

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

    let provider = create_provider(&config).unwrap();
    let messages = vec![Message::User {
        content: "find text files".into(),
    }];

    let response: ProviderResponse = provider.send_request(&messages, &[]).await.unwrap();
    assert_eq!(response.content, Some("find . -name '*.txt'".into()));
}

#[tokio::test]
async fn zai_missing_api_key() {
    let config = make_named_config("zai", "https://api.z.ai/api/paas/v4", None, "glm-4");
    let result = create_provider(&config);
    let err = match result {
        Err(e) => e.to_string(),
        Ok(_) => panic!("expected error"),
    };
    assert!(err.contains("ZAI_API_KEY"));
}

#[tokio::test]
async fn zai_provider_name() {
    let config = make_named_config("zai", "https://api.z.ai/api/paas/v4", Some("key"), "glm-4");
    let provider = create_provider(&config).unwrap();
    assert_eq!(provider.name(), "zai");
}

#[tokio::test]
async fn zai_supports_tools() {
    let config = make_named_config("zai", "https://api.z.ai/api/paas/v4", Some("key"), "glm-4");
    let provider = create_provider(&config).unwrap();
    assert!(provider.supports_tools());
}

// --- Minimax provider tests ---

#[tokio::test]
async fn minimax_successful_completion() {
    let server = MockServer::start().await;
    let config = make_named_config("minimax", &server.uri(), Some("mm-key"), "minimax-abab6.5");

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
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
async fn minimax_missing_api_key() {
    let config = make_named_config(
        "minimax",
        "https://api.minimax.chat",
        None,
        "minimax-abab6.5",
    );
    let result = create_provider(&config);
    let err = match result {
        Err(e) => e.to_string(),
        Ok(_) => panic!("expected error"),
    };
    assert!(err.contains("MINIMAX_API_KEY"));
}

#[tokio::test]
async fn minimax_provider_name() {
    let config = make_named_config(
        "minimax",
        "https://api.minimax.chat",
        Some("key"),
        "minimax-abab6.5",
    );
    let provider = create_provider(&config).unwrap();
    assert_eq!(provider.name(), "minimax");
}

#[tokio::test]
async fn minimax_supports_tools() {
    let config = make_named_config(
        "minimax",
        "https://api.minimax.chat",
        Some("key"),
        "minimax-abab6.5",
    );
    let provider = create_provider(&config).unwrap();
    assert!(provider.supports_tools());
}

// --- Qwen provider tests ---

#[tokio::test]
async fn qwen_successful_completion() {
    let server = MockServer::start().await;
    let config = make_named_config("qwen", &server.uri(), Some("qw-key"), "qwen-turbo");

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "role": "assistant", "content": "ls /tmp" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let provider = create_provider(&config).unwrap();
    let messages = vec![Message::User {
        content: "list temp dir".into(),
    }];

    let response: ProviderResponse = provider.send_request(&messages, &[]).await.unwrap();
    assert_eq!(response.content, Some("ls /tmp".into()));
}

#[tokio::test]
async fn qwen_missing_api_key() {
    let config = make_named_config(
        "qwen",
        "https://dashscope.aliyuncs.com/compatible-mode",
        None,
        "qwen-turbo",
    );
    let result = create_provider(&config);
    let err = match result {
        Err(e) => e.to_string(),
        Ok(_) => panic!("expected error"),
    };
    assert!(err.contains("QWEN_API_KEY"));
}

#[tokio::test]
async fn qwen_provider_name() {
    let config = make_named_config(
        "qwen",
        "https://dashscope.aliyuncs.com/compatible-mode",
        Some("key"),
        "qwen-turbo",
    );
    let provider = create_provider(&config).unwrap();
    assert_eq!(provider.name(), "qwen");
}

#[tokio::test]
async fn qwen_supports_tools() {
    let config = make_named_config(
        "qwen",
        "https://dashscope.aliyuncs.com/compatible-mode",
        Some("key"),
        "qwen-turbo",
    );
    let provider = create_provider(&config).unwrap();
    assert!(provider.supports_tools());
}

// --- Kimi provider tests ---

#[tokio::test]
async fn kimi_successful_completion() {
    let server = MockServer::start().await;
    let config = make_named_config("kimi", &server.uri(), Some("km-key"), "moonshot-v1-8k");

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "role": "assistant", "content": "df -h" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let provider = create_provider(&config).unwrap();
    let messages = vec![Message::User {
        content: "disk usage".into(),
    }];

    let response: ProviderResponse = provider.send_request(&messages, &[]).await.unwrap();
    assert_eq!(response.content, Some("df -h".into()));
}

#[tokio::test]
async fn kimi_missing_api_key() {
    let config = make_named_config("kimi", "https://api.moonshot.cn", None, "moonshot-v1-8k");
    let result = create_provider(&config);
    let err = match result {
        Err(e) => e.to_string(),
        Ok(_) => panic!("expected error"),
    };
    assert!(err.contains("KIMI_API_KEY"));
}

#[tokio::test]
async fn kimi_provider_name() {
    let config = make_named_config(
        "kimi",
        "https://api.moonshot.cn",
        Some("key"),
        "moonshot-v1-8k",
    );
    let provider = create_provider(&config).unwrap();
    assert_eq!(provider.name(), "kimi");
}

#[tokio::test]
async fn kimi_supports_tools() {
    let config = make_named_config(
        "kimi",
        "https://api.moonshot.cn",
        Some("key"),
        "moonshot-v1-8k",
    );
    let provider = create_provider(&config).unwrap();
    assert!(provider.supports_tools());
}

// --- Mistral provider tests ---

#[tokio::test]
async fn mistral_successful_completion() {
    let server = MockServer::start().await;
    let config = make_named_config(
        "mistral",
        &server.uri(),
        Some("ms-key"),
        "mistral-small-latest",
    );

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "role": "assistant", "content": "uname -a" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let provider = create_provider(&config).unwrap();
    let messages = vec![Message::User {
        content: "system info".into(),
    }];

    let response: ProviderResponse = provider.send_request(&messages, &[]).await.unwrap();
    assert_eq!(response.content, Some("uname -a".into()));
}

#[tokio::test]
async fn mistral_missing_api_key() {
    let config = make_named_config(
        "mistral",
        "https://api.mistral.ai",
        None,
        "mistral-small-latest",
    );
    let result = create_provider(&config);
    let err = match result {
        Err(e) => e.to_string(),
        Ok(_) => panic!("expected error"),
    };
    assert!(err.contains("MISTRAL_API_KEY"));
}

#[tokio::test]
async fn mistral_provider_name() {
    let config = make_named_config(
        "mistral",
        "https://api.mistral.ai",
        Some("key"),
        "mistral-small-latest",
    );
    let provider = create_provider(&config).unwrap();
    assert_eq!(provider.name(), "mistral");
}

#[tokio::test]
async fn mistral_supports_tools() {
    let config = make_named_config(
        "mistral",
        "https://api.mistral.ai",
        Some("key"),
        "mistral-small-latest",
    );
    let provider = create_provider(&config).unwrap();
    assert!(provider.supports_tools());
}

// --- Ollama provider tests ---

#[tokio::test]
async fn ollama_successful_completion() {
    let server = MockServer::start().await;
    let config = make_named_config("ollama", &server.uri(), None, "llama3");

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

    let provider = create_provider(&config).unwrap();
    let messages = vec![Message::User {
        content: "list files".into(),
    }];

    let response: ProviderResponse = provider.send_request(&messages, &[]).await.unwrap();
    assert_eq!(response.content, Some("ls".into()));
}

#[tokio::test]
async fn ollama_no_api_key_required() {
    let config = make_named_config("ollama", "http://localhost:11434", None, "llama3");
    let result = create_provider(&config);
    assert!(result.is_ok());
}

#[tokio::test]
async fn ollama_provider_name() {
    let config = make_named_config("ollama", "http://localhost:11434", None, "llama3");
    let provider = create_provider(&config).unwrap();
    assert_eq!(provider.name(), "completions");
}

#[tokio::test]
async fn ollama_supports_tools() {
    let config = make_named_config("ollama", "http://localhost:11434", None, "llama3");
    let provider = create_provider(&config).unwrap();
    assert!(provider.supports_tools());
}

#[tokio::test]
async fn ollama_tool_call_response() {
    let server = MockServer::start().await;
    let config = make_named_config("ollama", &server.uri(), None, "llama3");

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_ollama1",
                        "type": "function",
                        "function": { "name": "find_command", "arguments": "{\"command\":\"fd\"}" }
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
    assert_eq!(response.tool_calls[0].id, "call_ollama1");
}

#[tokio::test]
async fn ollama_custom_base_url() {
    let server = MockServer::start().await;
    let config = make_named_config("ollama", &server.uri(), None, "llama3");

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{
                "message": { "role": "assistant", "content": "pwd" },
                "finish_reason": "stop"
            }]
        })))
        .mount(&server)
        .await;

    let provider = create_provider(&config).unwrap();
    let messages = vec![Message::User {
        content: "where am i".into(),
    }];

    let response: ProviderResponse = provider.send_request(&messages, &[]).await.unwrap();
    assert_eq!(response.content, Some("pwd".into()));
}

// --- Unknown provider error ---

#[tokio::test]
async fn unknown_provider_name() {
    let config = make_named_config("nonexistent", "http://localhost", None, "model");
    let result = create_provider(&config);
    let err = match result {
        Err(e) => e.to_string(),
        Ok(_) => panic!("expected error"),
    };
    assert!(err.contains("unknown provider"));
}

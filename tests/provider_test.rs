use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use cmdify::config::{AuthStyle, Config, ProviderSettings};
use cmdify::error::Error;
use cmdify::provider::{create_provider, Message, ProviderResponse};

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

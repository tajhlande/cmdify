use async_trait::async_trait;
use serde_json::json;

use crate::config::Config;
use crate::debug_json;
use crate::error::{Error, Result};
use crate::provider::http::{send_and_parse, user_agent};
use crate::provider::{
    FinishReason, Message, Provider, ProviderResponse, ToolCall, ToolDefinition,
};

const ANTHROPIC_VERSION: &str = "2023-06-01";
const ENDPOINT_PATH: &str = "/v1/messages";
#[allow(dead_code)]
pub(crate) const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
pub(crate) const API_KEY_ENV: &str = "ANTHROPIC_API_KEY";

pub fn create(config: &Config) -> Result<AnthropicProvider> {
    let api_key = config
        .provider_settings
        .api_key
        .as_deref()
        .ok_or_else(|| {
            Error::ConfigError(format!(
                "{} is required for the anthropic provider",
                API_KEY_ENV
            ))
        })?
        .to_string();

    Ok(AnthropicProvider {
        client: reqwest::Client::new(),
        base_url: config.provider_settings.base_url.clone(),
        api_key,
        model_name: config.model_name.clone(),
        max_tokens: config.max_tokens,
    })
}

#[derive(Debug)]
pub struct AnthropicProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model_name: String,
    max_tokens: u32,
}

impl AnthropicProvider {
    fn format_messages(&self, messages: &[Message]) -> (Option<String>, Vec<serde_json::Value>) {
        let mut system = None;
        let mut formatted = Vec::new();

        for m in messages {
            match m {
                Message::System { content } => {
                    system = Some(content.clone());
                }
                Message::User { content } => {
                    formatted.push(json!({ "role": "user", "content": content }));
                }
                Message::Assistant {
                    content,
                    tool_calls,
                } => {
                    if tool_calls.is_empty() {
                        let mut msg = json!({ "role": "assistant" });
                        if let Some(c) = content {
                            msg["content"] = json!(c);
                        }
                        formatted.push(msg);
                    } else {
                        let mut content_blocks = Vec::new();
                        if let Some(c) = content {
                            content_blocks.push(json!({ "type": "text", "text": c }));
                        }
                        for tc in tool_calls {
                            content_blocks.push(json!({
                                "type": "tool_use",
                                "id": tc.id,
                                "name": tc.name,
                                "input": tc.arguments,
                            }));
                        }
                        formatted.push(json!({
                            "role": "assistant",
                            "content": content_blocks,
                        }));
                    }
                }
                Message::ToolResult {
                    tool_call_id,
                    name: _,
                    content,
                } => {
                    formatted.push(json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": tool_call_id,
                            "content": content,
                        }]
                    }));
                }
            }
        }

        (system, formatted)
    }

    fn format_tools(&self, tools: &[ToolDefinition]) -> serde_json::Value {
        let tools_json: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters,
                })
            })
            .collect();
        json!(tools_json)
    }

    fn parse_response(&self, body: &serde_json::Value) -> Result<ProviderResponse> {
        let content_arr = body
            .get("content")
            .and_then(|c| c.as_array())
            .ok_or_else(|| Error::ResponseError("no content in response".into()))?;

        let mut content = None;
        let mut tool_calls = Vec::new();
        for block in content_arr {
            let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match block_type {
                "text" => {
                    if content.is_none() {
                        content = block
                            .get("text")
                            .and_then(|t| t.as_str())
                            .map(|s| s.to_string());
                    }
                }
                "tool_use" => {
                    let id = block
                        .get("id")
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = block
                        .get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string();
                    let arguments = block.get("input").cloned().unwrap_or(json!({}));
                    tool_calls.push(ToolCall {
                        id,
                        name,
                        arguments,
                    });
                }
                _ => {}
            }
        }

        let finish_reason = match body
            .get("stop_reason")
            .and_then(|r| r.as_str())
            .unwrap_or("")
        {
            "end_turn" => FinishReason::Stop,
            "tool_use" => FinishReason::ToolCalls,
            "max_tokens" => FinishReason::Length,
            other => FinishReason::Other(other.to_string()),
        };

        Ok(ProviderResponse {
            content,
            tool_calls,
            finish_reason,
        })
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    async fn send_request(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ProviderResponse> {
        let (system, formatted_messages) = self.format_messages(messages);

        let mut body = json!({
            "model": self.model_name,
            "max_tokens": self.max_tokens,
            "messages": formatted_messages,
        });

        if let Some(sys) = system {
            body["system"] = json!(sys);
        }

        if !tools.is_empty() {
            body["tools"] = self.format_tools(tools);
        }

        let url = format!("{}{}", self.base_url.trim_end_matches('/'), ENDPOINT_PATH);

        debug_json!("Request body", &body);

        let request = self
            .client
            .post(&url)
            .json(&body)
            .header("User-Agent", user_agent())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json");

        send_and_parse(request, &url, |b| self.parse_response(b)).await
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "anthropic"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthStyle, ProviderSettings};
    use crate::provider::Provider;

    fn make_config(base_url: &str, api_key: Option<&str>) -> Config {
        Config {
            provider_name: "anthropic".into(),
            model_name: "claude-sonnet-4-20250514".into(),
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
                    name: "x-api-key".into(),
                    prefix: String::new(),
                },
            },
        }
    }

    fn make_provider() -> AnthropicProvider {
        AnthropicProvider {
            client: reqwest::Client::new(),
            base_url: "https://api.anthropic.com".into(),
            api_key: "test-key".into(),
            model_name: "claude-sonnet-4-20250514".into(),
            max_tokens: 4096,
        }
    }

    #[test]
    fn create_with_key() {
        let provider = create(&make_config("https://custom.url", Some("sk-test"))).unwrap();
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn create_without_key_errors() {
        let result = create(&make_config(DEFAULT_BASE_URL, None));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("ANTHROPIC_API_KEY"));
    }

    #[test]
    fn format_messages_extracts_system() {
        let provider = make_provider();
        let messages = vec![
            Message::System {
                content: "Be helpful.".into(),
            },
            Message::User {
                content: "hello".into(),
            },
        ];
        let (system, formatted) = provider.format_messages(&messages);
        assert_eq!(system.as_deref(), Some("Be helpful."));
        assert_eq!(formatted.len(), 1);
        assert_eq!(formatted[0]["role"], "user");
    }

    #[test]
    fn format_messages_no_system() {
        let provider = make_provider();
        let messages = vec![Message::User {
            content: "hello".into(),
        }];
        let (system, formatted) = provider.format_messages(&messages);
        assert!(system.is_none());
        assert_eq!(formatted.len(), 1);
    }

    #[test]
    fn format_assistant_text_only() {
        let provider = make_provider();
        let messages = vec![Message::Assistant {
            content: Some("ls -la".into()),
            tool_calls: vec![],
        }];
        let (_, formatted) = provider.format_messages(&messages);
        assert_eq!(formatted.len(), 1);
        assert_eq!(formatted[0]["role"], "assistant");
        assert_eq!(formatted[0]["content"], "ls -la");
    }

    #[test]
    fn format_assistant_with_tool_calls() {
        let provider = make_provider();
        let messages = vec![Message::Assistant {
            content: None,
            tool_calls: vec![ToolCall {
                id: "toolu_1".into(),
                name: "find_command".into(),
                arguments: json!({"command": "fd"}),
            }],
        }];
        let (_, formatted) = provider.format_messages(&messages);
        assert_eq!(formatted.len(), 1);
        let content = formatted[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "tool_use");
        assert_eq!(content[0]["id"], "toolu_1");
        assert_eq!(content[0]["name"], "find_command");
    }

    #[test]
    fn format_assistant_with_content_and_tool_calls() {
        let provider = make_provider();
        let messages = vec![Message::Assistant {
            content: Some("Let me check.".into()),
            tool_calls: vec![ToolCall {
                id: "toolu_1".into(),
                name: "find_command".into(),
                arguments: json!({"command": "fd"}),
            }],
        }];
        let (_, formatted) = provider.format_messages(&messages);
        let content = formatted[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "Let me check.");
        assert_eq!(content[1]["type"], "tool_use");
    }

    #[test]
    fn format_tool_result() {
        let provider = make_provider();
        let messages = vec![Message::ToolResult {
            tool_call_id: "toolu_1".into(),
            name: "find_command".into(),
            content: "/bin/ls".into(),
        }];
        let (_, formatted) = provider.format_messages(&messages);
        assert_eq!(formatted.len(), 1);
        assert_eq!(formatted[0]["role"], "user");
        let content = formatted[0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "tool_result");
        assert_eq!(content[0]["tool_use_id"], "toolu_1");
        assert_eq!(content[0]["content"], "/bin/ls");
    }

    #[test]
    fn format_tools_uses_input_schema() {
        let provider = make_provider();
        let tools = vec![ToolDefinition {
            name: "find_command".into(),
            description: "Find a command".into(),
            parameters: json!({"type": "object"}),
        }];
        let result = provider.format_tools(&tools);
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "find_command");
        assert_eq!(arr[0]["input_schema"]["type"], "object");
        assert!(arr[0].get("type").is_none());
        assert!(arr[0].get("function").is_none());
    }

    #[test]
    fn parse_text_response() {
        let provider = make_provider();
        let body = json!({
            "content": [
                { "type": "text", "text": "ls -la" }
            ],
            "stop_reason": "end_turn"
        });
        let response = provider.parse_response(&body).unwrap();
        assert_eq!(response.content, Some("ls -la".into()));
        assert!(response.tool_calls.is_empty());
        assert_eq!(response.finish_reason, FinishReason::Stop);
    }

    #[test]
    fn parse_tool_use_response() {
        let provider = make_provider();
        let body = json!({
            "content": [
                {
                    "type": "tool_use",
                    "id": "toolu_abc",
                    "name": "find_command",
                    "input": { "command": "fd" }
                }
            ],
            "stop_reason": "tool_use"
        });
        let response = provider.parse_response(&body).unwrap();
        assert!(response.content.is_none());
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "find_command");
        assert_eq!(response.tool_calls[0].id, "toolu_abc");
        assert_eq!(response.finish_reason, FinishReason::ToolCalls);
    }

    #[test]
    fn parse_mixed_text_and_tool_use() {
        let provider = make_provider();
        let body = json!({
            "content": [
                { "type": "text", "text": "Let me check." },
                {
                    "type": "tool_use",
                    "id": "toolu_1",
                    "name": "find_command",
                    "input": { "command": "fd" }
                }
            ],
            "stop_reason": "tool_use"
        });
        let response = provider.parse_response(&body).unwrap();
        assert_eq!(response.content, Some("Let me check.".into()));
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.finish_reason, FinishReason::ToolCalls);
    }

    #[test]
    fn parse_max_tokens_stop_reason() {
        let provider = make_provider();
        let body = json!({
            "content": [
                { "type": "text", "text": "incomplete" }
            ],
            "stop_reason": "max_tokens"
        });
        let response = provider.parse_response(&body).unwrap();
        assert_eq!(response.finish_reason, FinishReason::Length);
    }

    #[test]
    fn parse_unknown_stop_reason() {
        let provider = make_provider();
        let body = json!({
            "content": [
                { "type": "text", "text": "test" }
            ],
            "stop_reason": "some_other_reason"
        });
        let response = provider.parse_response(&body).unwrap();
        assert_eq!(
            response.finish_reason,
            FinishReason::Other("some_other_reason".into())
        );
    }

    #[test]
    fn supports_tools_true() {
        let provider = create(&make_config("https://api.anthropic.com", Some("key"))).unwrap();
        assert!(provider.supports_tools());
    }
}

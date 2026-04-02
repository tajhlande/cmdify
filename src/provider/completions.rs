use async_trait::async_trait;
use serde_json::json;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::provider::{
    FinishReason, Message, Provider, ProviderResponse, ToolCall, ToolDefinition,
};

pub struct CompletionsProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    model_name: String,
    max_tokens: u32,
}

impl CompletionsProvider {
    pub fn new(config: &Config) -> Result<Self> {
        let client = reqwest::Client::new();
        Ok(Self {
            client,
            base_url: config.provider_settings.base_url.clone(),
            api_key: config.provider_settings.api_key.clone(),
            model_name: config.model_name.clone(),
            max_tokens: config.max_tokens,
        })
    }

    fn format_messages(&self, messages: &[Message]) -> Vec<serde_json::Value> {
        messages
            .iter()
            .map(|m| match m {
                Message::System { content } => json!({ "role": "system", "content": content }),
                Message::User { content } => json!({ "role": "user", "content": content }),
                Message::Assistant {
                    content,
                    tool_calls,
                } => {
                    let mut msg = json!({ "role": "assistant" });
                    if let Some(c) = content {
                        msg["content"] = json!(c);
                    }
                    if !tool_calls.is_empty() {
                        let tc: Vec<serde_json::Value> = tool_calls
                            .iter()
                            .map(|tc| {
                                json!({
                                    "id": tc.id,
                                    "type": "function",
                                    "function": {
                                        "name": tc.name,
                                        "arguments": tc.arguments,
                                    }
                                })
                            })
                            .collect();
                        msg["tool_calls"] = json!(tc);
                    }
                    msg
                }
                Message::ToolResult {
                    tool_call_id,
                    name: _,
                    content,
                } => json!({
                    "role": "tool",
                    "tool_call_id": tool_call_id,
                    "content": content,
                }),
            })
            .collect()
    }

    fn parse_response(&self, body: &serde_json::Value) -> Result<ProviderResponse> {
        let choice = body
            .get("choices")
            .and_then(|c| c.get(0))
            .ok_or_else(|| Error::ResponseError("no choices in response".into()))?;

        let message = choice
            .get("message")
            .ok_or_else(|| Error::ResponseError("no message in choice".into()))?;

        let content = message
            .get("content")
            .and_then(|c| c.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let tool_calls = message
            .get("tool_calls")
            .and_then(|tc| tc.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|tc| {
                        let id = tc.get("id")?.as_str()?.to_string();
                        let func = tc.get("function")?;
                        let name = func.get("name")?.as_str()?.to_string();
                        let arguments = func.get("arguments").cloned().unwrap_or(json!({}));
                        Some(ToolCall {
                            id,
                            name,
                            arguments,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let finish_reason = match choice
            .get("finish_reason")
            .and_then(|r| r.as_str())
            .unwrap_or("")
        {
            "stop" => FinishReason::Stop,
            "tool_calls" => FinishReason::ToolCalls,
            "length" => FinishReason::Length,
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
impl Provider for CompletionsProvider {
    async fn send_request(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ProviderResponse> {
        let formatted_messages = self.format_messages(messages);

        let mut body = json!({
            "model": self.model_name,
            "messages": formatted_messages,
            "max_tokens": self.max_tokens,
        });

        if !tools.is_empty() {
            let tools_json: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect();
            body["tools"] = json!(tools_json);
        }

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let mut request = self.client.post(&url).json(&body);

        if let Some(key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request.send().await?;

        let status = response.status();
        let body: serde_json::Value = response.json().await?;

        if !status.is_success() {
            let error_msg = body
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            return Err(Error::ProviderError(format!(
                "API returned {} {}: {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Unknown"),
                error_msg,
            )));
        }

        self.parse_response(&body)
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "completions"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config(base_url: &str, api_key: Option<&str>, model: &str, max_tokens: u32) -> Config {
        Config {
            provider_name: "completions".into(),
            model_name: model.into(),
            max_tokens,
            system_prompt_override: None,
            provider_settings: crate::config::ProviderSettings {
                api_key: api_key.map(|k| k.into()),
                base_url: base_url.into(),
                auth_style: crate::config::AuthStyle::Header {
                    name: "Authorization".into(),
                    prefix: "Bearer ".into(),
                },
            },
        }
    }

    #[test]
    fn format_system_message() {
        let provider =
            CompletionsProvider::new(&make_config("http://localhost:11434", None, "llama3", 4096))
                .unwrap();
        let messages = [Message::System {
            content: "You are helpful.".into(),
        }];
        let formatted = provider.format_messages(&messages);
        assert_eq!(formatted.len(), 1);
        assert_eq!(formatted[0]["role"], "system");
        assert_eq!(formatted[0]["content"], "You are helpful.");
    }

    #[test]
    fn format_user_message() {
        let provider =
            CompletionsProvider::new(&make_config("http://localhost:11434", None, "llama3", 4096))
                .unwrap();
        let messages = [Message::User {
            content: "list files".into(),
        }];
        let formatted = provider.format_messages(&messages);
        assert_eq!(formatted[0]["role"], "user");
        assert_eq!(formatted[0]["content"], "list files");
    }

    #[test]
    fn format_assistant_message_with_tool_calls() {
        let provider =
            CompletionsProvider::new(&make_config("http://localhost:11434", None, "llama3", 4096))
                .unwrap();
        let messages = [Message::Assistant {
            content: None,
            tool_calls: vec![ToolCall {
                id: "call_1".into(),
                name: "find_command".into(),
                arguments: json!({"command": "fd"}),
            }],
        }];
        let formatted = provider.format_messages(&messages);
        assert_eq!(formatted[0]["role"], "assistant");
        let tc = &formatted[0]["tool_calls"][0];
        assert_eq!(tc["id"], "call_1");
        assert_eq!(tc["function"]["name"], "find_command");
    }

    #[test]
    fn parse_stop_response() {
        let provider =
            CompletionsProvider::new(&make_config("http://localhost:11434", None, "llama3", 4096))
                .unwrap();
        let body = json!({
            "choices": [{
                "message": { "role": "assistant", "content": "ls -la" },
                "finish_reason": "stop"
            }]
        });
        let response = provider.parse_response(&body).unwrap();
        assert_eq!(response.content, Some("ls -la".into()));
        assert!(response.tool_calls.is_empty());
        assert_eq!(response.finish_reason, FinishReason::Stop);
    }

    #[test]
    fn parse_tool_call_response() {
        let provider =
            CompletionsProvider::new(&make_config("http://localhost:11434", None, "llama3", 4096))
                .unwrap();
        let body = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_abc",
                        "type": "function",
                        "function": { "name": "find_command", "arguments": "{\"command\":\"fd\"}" }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });
        let response = provider.parse_response(&body).unwrap();
        assert!(response.content.is_none());
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "find_command");
        assert_eq!(response.finish_reason, FinishReason::ToolCalls);
    }

    #[test]
    fn parse_no_choices() {
        let provider =
            CompletionsProvider::new(&make_config("http://localhost:11434", None, "llama3", 4096))
                .unwrap();
        let body = json!({ "choices": [] });
        let result = provider.parse_response(&body);
        assert!(result.is_err());
    }
}

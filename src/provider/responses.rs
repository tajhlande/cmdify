use async_trait::async_trait;
use serde_json::json;

use crate::config::{AuthStyle, Config};
use crate::debug_json;
use crate::error::{Error, Result};
use crate::provider::http::{send_and_parse, user_agent};
use crate::provider::{
    FinishReason, Message, Provider, ProviderResponse, ToolCall, ToolDefinition,
};

pub(crate) fn format_messages(messages: &[Message]) -> Vec<serde_json::Value> {
    let mut items = Vec::new();
    for m in messages {
        match m {
            Message::System { content } => {
                items.push(json!({ "role": "system", "content": content }));
            }
            Message::User { content } => {
                items.push(json!({ "role": "user", "content": content }));
            }
            Message::Assistant {
                content,
                tool_calls,
            } => {
                if let Some(c) = content {
                    items.push(json!({ "role": "assistant", "content": c }));
                }
                for tc in tool_calls {
                    items.push(json!({
                        "type": "function_call",
                        "call_id": tc.id,
                        "name": tc.name,
                        "arguments": tc.arguments.to_string(),
                    }));
                }
            }
            Message::ToolResult {
                tool_call_id,
                name: _,
                content,
            } => {
                items.push(json!({
                    "type": "function_call_output",
                    "call_id": tool_call_id,
                    "output": content,
                }));
            }
        }
    }
    items
}

pub(crate) fn format_tools(tools: &[ToolDefinition]) -> serde_json::Value {
    let tools_json: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| {
            json!({
                "type": "function",
                "name": t.name,
                "description": t.description,
                "parameters": t.parameters,
            })
        })
        .collect();
    json!(tools_json)
}

pub(crate) fn parse_response(body: &serde_json::Value) -> Result<ProviderResponse> {
    let output = body
        .get("output")
        .and_then(|o| o.as_array())
        .ok_or_else(|| Error::ResponseError("no output in response".into()))?;

    let mut content = None;
    let mut tool_calls = Vec::new();
    let mut has_tool_calls = false;

    for item in output {
        let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match item_type {
            "message" => {
                if let Some(content_arr) = item.get("content").and_then(|c| c.as_array()) {
                    for part in content_arr {
                        if part.get("type").and_then(|t| t.as_str()) == Some("output_text")
                            && content.is_none()
                        {
                            content = part
                                .get("text")
                                .and_then(|t| t.as_str())
                                .map(|s| s.to_string());
                        }
                    }
                }
            }
            "function_call" => {
                has_tool_calls = true;
                let call_id = item
                    .get("call_id")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = item
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                let arguments = item
                    .get("arguments")
                    .and_then(|a| {
                        if a.is_string() {
                            serde_json::from_str(a.as_str()?).ok()
                        } else {
                            Some(a.clone())
                        }
                    })
                    .unwrap_or(json!({}));
                tool_calls.push(ToolCall {
                    id: call_id,
                    name,
                    arguments,
                });
            }
            _ => {}
        }
    }

    let finish_reason = if has_tool_calls {
        FinishReason::ToolCalls
    } else {
        match body.get("status").and_then(|s| s.as_str()) {
            Some("incomplete") => FinishReason::Length,
            _ => FinishReason::Stop,
        }
    };

    Ok(ProviderResponse {
        content,
        tool_calls,
        finish_reason,
    })
}

pub(crate) struct ResponsesRequest<'a> {
    pub client: &'a reqwest::Client,
    pub base_url: &'a str,
    pub auth_style: &'a AuthStyle,
    pub api_key: &'a Option<String>,
    pub model: &'a str,
    pub messages: &'a [Message],
    pub tools: &'a [ToolDefinition],
    pub endpoint_path: &'a str,
}

pub(crate) async fn send_responses_request(req: &ResponsesRequest<'_>) -> Result<ProviderResponse> {
    let formatted_messages = format_messages(req.messages);

    let mut body = json!({
        "model": req.model,
        "input": formatted_messages,
        "store": false,
    });

    if !req.tools.is_empty() {
        body["tools"] = format_tools(req.tools);
    }

    let url = format!(
        "{}{}",
        req.base_url.trim_end_matches('/'),
        req.endpoint_path
    );

    debug_json!("Request body", &body);

    let mut request = req
        .client
        .post(&url)
        .json(&body)
        .header("User-Agent", user_agent());

    if let (AuthStyle::Header { name, prefix }, Some(key)) = (req.auth_style, req.api_key) {
        request = request.header(name.as_str(), format!("{}{}", prefix, key));
    }

    send_and_parse(request, &url, parse_response).await
}

pub const DEFAULT_ENDPOINT_PATH: &str = "/v1/responses";

#[derive(Debug)]
pub struct ResponsesProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    auth_style: AuthStyle,
    model_name: String,
    #[allow(dead_code)]
    max_tokens: u32,
    endpoint_path: String,
    provider_name: String,
}

impl ResponsesProvider {
    pub fn new(config: &Config) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: config.provider_settings.base_url.clone(),
            api_key: config.provider_settings.api_key.clone(),
            auth_style: config.provider_settings.auth_style.clone(),
            model_name: config.model_name.clone(),
            max_tokens: config.max_tokens,
            endpoint_path: DEFAULT_ENDPOINT_PATH.into(),
            provider_name: "responses".into(),
        }
    }

    pub fn with_options(
        config: &Config,
        provider_name: &str,
        endpoint_path: &str,
        api_key: String,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: config.provider_settings.base_url.clone(),
            api_key: Some(api_key),
            auth_style: AuthStyle::Header {
                name: "Authorization".into(),
                prefix: "Bearer ".into(),
            },
            model_name: config.model_name.clone(),
            max_tokens: config.max_tokens,
            endpoint_path: endpoint_path.into(),
            provider_name: provider_name.into(),
        }
    }
}

#[async_trait]
impl Provider for ResponsesProvider {
    async fn send_request(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ProviderResponse> {
        send_responses_request(&ResponsesRequest {
            client: &self.client,
            base_url: &self.base_url,
            auth_style: &self.auth_style,
            api_key: &self.api_key,
            model: &self.model_name,
            messages,
            tools,
            endpoint_path: &self.endpoint_path,
        })
        .await
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        &self.provider_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProviderSettings;

    fn make_config(base_url: &str, api_key: Option<&str>, model: &str) -> Config {
        Config {
            provider_name: "responses".into(),
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

    #[test]
    fn format_system_message() {
        let messages = [Message::System {
            content: "You are helpful.".into(),
        }];
        let items = format_messages(&messages);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["role"], "system");
        assert_eq!(items[0]["content"], "You are helpful.");
    }

    #[test]
    fn format_user_message() {
        let messages = [Message::User {
            content: "list files".into(),
        }];
        let items = format_messages(&messages);
        assert_eq!(items[0]["role"], "user");
        assert_eq!(items[0]["content"], "list files");
    }

    #[test]
    fn format_assistant_message_with_tool_calls() {
        let messages = [Message::Assistant {
            content: None,
            tool_calls: vec![ToolCall {
                id: "call_1".into(),
                name: "find_command".into(),
                arguments: json!({"command": "fd"}),
            }],
        }];
        let items = format_messages(&messages);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["type"], "function_call");
        assert_eq!(items[0]["call_id"], "call_1");
        assert_eq!(items[0]["name"], "find_command");
    }

    #[test]
    fn format_assistant_with_content_and_tool_calls() {
        let messages = [Message::Assistant {
            content: Some("Let me check.".into()),
            tool_calls: vec![ToolCall {
                id: "call_1".into(),
                name: "find_command".into(),
                arguments: json!({"command": "fd"}),
            }],
        }];
        let items = format_messages(&messages);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0]["role"], "assistant");
        assert_eq!(items[0]["content"], "Let me check.");
        assert_eq!(items[1]["type"], "function_call");
        assert_eq!(items[1]["name"], "find_command");
    }

    #[test]
    fn format_tool_result() {
        let messages = [Message::ToolResult {
            tool_call_id: "call_1".into(),
            name: "find_command".into(),
            content: "/bin/ls".into(),
        }];
        let items = format_messages(&messages);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["type"], "function_call_output");
        assert_eq!(items[0]["call_id"], "call_1");
        assert_eq!(items[0]["output"], "/bin/ls");
    }

    #[test]
    fn format_tools_flat() {
        let tools = vec![ToolDefinition {
            name: "find_command".into(),
            description: "Find a command".into(),
            parameters: json!({"type": "object"}),
        }];
        let result = format_tools(&tools);
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["type"], "function");
        assert_eq!(arr[0]["name"], "find_command");
        assert!(arr[0].get("function").is_none());
    }

    #[test]
    fn format_tools_empty() {
        let tools: Vec<ToolDefinition> = vec![];
        let result = format_tools(&tools);
        assert!(result.as_array().unwrap().is_empty());
    }

    #[test]
    fn parse_text_response() {
        let body = json!({
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{ "type": "output_text", "text": "ls -la" }]
                }
            ]
        });
        let response = parse_response(&body).unwrap();
        assert_eq!(response.content, Some("ls -la".into()));
        assert!(response.tool_calls.is_empty());
        assert_eq!(response.finish_reason, FinishReason::Stop);
    }

    #[test]
    fn parse_function_call_response() {
        let body = json!({
            "output": [
                {
                    "type": "function_call",
                    "call_id": "call_abc",
                    "name": "find_command",
                    "arguments": "{\"command\":\"fd\"}"
                }
            ]
        });
        let response = parse_response(&body).unwrap();
        assert!(response.content.is_none());
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "find_command");
        assert_eq!(response.tool_calls[0].id, "call_abc");
        assert_eq!(response.finish_reason, FinishReason::ToolCalls);
    }

    #[test]
    fn parse_mixed_text_and_function_call() {
        let body = json!({
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{ "type": "output_text", "text": "Let me check." }]
                },
                {
                    "type": "function_call",
                    "call_id": "call_1",
                    "name": "find_command",
                    "arguments": "{\"command\":\"fd\"}"
                }
            ]
        });
        let response = parse_response(&body).unwrap();
        assert_eq!(response.content, Some("Let me check.".into()));
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.finish_reason, FinishReason::ToolCalls);
    }

    #[test]
    fn parse_skips_reasoning_items() {
        let body = json!({
            "output": [
                {
                    "type": "reasoning",
                    "content": [],
                    "summary": []
                },
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{ "type": "output_text", "text": "ls" }]
                }
            ]
        });
        let response = parse_response(&body).unwrap();
        assert_eq!(response.content, Some("ls".into()));
        assert!(response.tool_calls.is_empty());
    }

    #[test]
    fn parse_incomplete_status() {
        let body = json!({
            "status": "incomplete",
            "output": [
                {
                    "type": "message",
                    "role": "assistant",
                    "content": [{ "type": "output_text", "text": "incomplete" }]
                }
            ]
        });
        let response = parse_response(&body).unwrap();
        assert_eq!(response.finish_reason, FinishReason::Length);
    }

    #[test]
    fn parse_empty_output() {
        let body = json!({ "output": [] });
        let response = parse_response(&body).unwrap();
        assert!(response.content.is_none());
        assert!(response.tool_calls.is_empty());
        assert_eq!(response.finish_reason, FinishReason::Stop);
    }

    #[test]
    fn parse_no_output_key() {
        let body = json!({ "id": "resp_123" });
        let result = parse_response(&body);
        assert!(result.is_err());
    }

    #[test]
    fn responses_provider_new() {
        let provider = ResponsesProvider::new(&make_config(
            "https://api.example.com",
            Some("key"),
            "model",
        ));
        assert_eq!(provider.base_url, "https://api.example.com");
        assert_eq!(provider.api_key.as_deref(), Some("key"));
        assert_eq!(provider.model_name, "model");
        assert_eq!(provider.name(), "responses");
        assert_eq!(provider.endpoint_path, DEFAULT_ENDPOINT_PATH);
    }

    #[test]
    fn responses_provider_new_optional_key() {
        let provider =
            ResponsesProvider::new(&make_config("https://api.example.com", None, "model"));
        assert!(provider.api_key.is_none());
    }

    #[test]
    fn with_options_sets_provider_name() {
        let provider = ResponsesProvider::with_options(
            &make_config("https://api.openai.com", Some("key"), "gpt-5"),
            "openai",
            "/v1/responses",
            "sk-test".into(),
        );
        assert_eq!(provider.name(), "openai");
        assert_eq!(provider.endpoint_path, "/v1/responses");
        assert_eq!(provider.api_key.as_deref(), Some("sk-test"));
    }

    #[test]
    fn with_options_overrides_base_url_from_config() {
        let provider = ResponsesProvider::with_options(
            &make_config("https://custom.url", Some("key"), "model"),
            "openai",
            "/v1/responses",
            "sk-key".into(),
        );
        assert_eq!(provider.base_url, "https://custom.url");
    }
}

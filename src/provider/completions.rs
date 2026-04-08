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
                                    "arguments": tc.arguments.to_string(),
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

pub(crate) fn format_tools(tools: &[ToolDefinition]) -> serde_json::Value {
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
    json!(tools_json)
}

pub(crate) fn parse_response(body: &serde_json::Value) -> Result<ProviderResponse> {
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
                    let arguments = func
                        .get("arguments")
                        .and_then(|a| {
                            if a.is_string() {
                                serde_json::from_str(a.as_str()?).ok()
                            } else {
                                Some(a.clone())
                            }
                        })
                        .unwrap_or(json!({}));
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

fn build_url_with_auth(
    base_url: &str,
    auth_style: &AuthStyle,
    api_key: &Option<String>,
    endpoint_path: &str,
) -> String {
    let url = format!("{}{}", base_url.trim_end_matches('/'), endpoint_path);
    match (auth_style, api_key) {
        (AuthStyle::QueryParam { name }, Some(key)) => {
            format!("{}?{}={}", url, name, key)
        }
        _ => url,
    }
}

fn apply_auth_headers(
    request: reqwest::RequestBuilder,
    auth_style: &AuthStyle,
    api_key: &Option<String>,
) -> reqwest::RequestBuilder {
    if let (AuthStyle::Header { name, prefix }, Some(key)) = (auth_style, api_key) {
        request.header(name.as_str(), format!("{}{}", prefix, key))
    } else {
        request
    }
}

pub(crate) struct CompletionsRequest<'a> {
    pub client: &'a reqwest::Client,
    pub base_url: &'a str,
    pub auth_style: &'a AuthStyle,
    pub api_key: &'a Option<String>,
    pub model: &'a str,
    pub messages: &'a [Message],
    pub tools: &'a [ToolDefinition],
    pub max_tokens: u32,
    pub extra_headers: Option<&'a [(String, String)]>,
    pub endpoint_path: &'a str,
}

pub(crate) async fn send_completions_request(
    req: &CompletionsRequest<'_>,
) -> Result<ProviderResponse> {
    let formatted_messages = format_messages(req.messages);

    let mut body = json!({
        "model": req.model,
        "messages": formatted_messages,
        "max_tokens": req.max_tokens,
    });

    if !req.tools.is_empty() {
        body["tools"] = format_tools(req.tools);
    }

    let url = build_url_with_auth(req.base_url, req.auth_style, req.api_key, req.endpoint_path);

    debug_json!("Request body", &body);

    let mut request = req
        .client
        .post(&url)
        .json(&body)
        .header("User-Agent", user_agent());
    request = apply_auth_headers(request, req.auth_style, req.api_key);

    if let Some(headers) = req.extra_headers {
        for (key, value) in headers {
            request = request.header(key.as_str(), value.as_str());
        }
    }

    send_and_parse(request, &url, parse_response).await
}

pub const DEFAULT_ENDPOINT_PATH: &str = "/chat/completions";

#[derive(Debug)]
pub struct CompletionsProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    auth_style: AuthStyle,
    model_name: String,
    max_tokens: u32,
    endpoint_path: String,
    provider_name: String,
}

impl CompletionsProvider {
    pub fn new(config: &Config) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: config.provider_settings.base_url.clone(),
            api_key: config.provider_settings.api_key.clone(),
            auth_style: config.provider_settings.auth_style.clone(),
            model_name: config.model_name.clone(),
            max_tokens: config.max_tokens,
            endpoint_path: DEFAULT_ENDPOINT_PATH.into(),
            provider_name: "completions".into(),
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
impl Provider for CompletionsProvider {
    async fn send_request(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ProviderResponse> {
        send_completions_request(&CompletionsRequest {
            client: &self.client,
            base_url: &self.base_url,
            auth_style: &self.auth_style,
            api_key: &self.api_key,
            model: &self.model_name,
            messages,
            tools,
            max_tokens: self.max_tokens,
            extra_headers: None,
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

    fn make_config(base_url: &str, api_key: Option<&str>, model: &str, max_tokens: u32) -> Config {
        Config {
            provider_name: "completions".into(),
            model_name: model.into(),
            max_tokens,
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
        let formatted = format_messages(&messages);
        assert_eq!(formatted.len(), 1);
        assert_eq!(formatted[0]["role"], "system");
        assert_eq!(formatted[0]["content"], "You are helpful.");
    }

    #[test]
    fn format_user_message() {
        let messages = [Message::User {
            content: "list files".into(),
        }];
        let formatted = format_messages(&messages);
        assert_eq!(formatted[0]["role"], "user");
        assert_eq!(formatted[0]["content"], "list files");
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
        let formatted = format_messages(&messages);
        assert_eq!(formatted[0]["role"], "assistant");
        let tc = &formatted[0]["tool_calls"][0];
        assert_eq!(tc["id"], "call_1");
        assert_eq!(tc["function"]["name"], "find_command");
    }

    #[test]
    fn parse_stop_response() {
        let body = json!({
            "choices": [{
                "message": { "role": "assistant", "content": "ls -la" },
                "finish_reason": "stop"
            }]
        });
        let response = parse_response(&body).unwrap();
        assert_eq!(response.content, Some("ls -la".into()));
        assert!(response.tool_calls.is_empty());
        assert_eq!(response.finish_reason, FinishReason::Stop);
    }

    #[test]
    fn parse_tool_call_response() {
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
        let response = parse_response(&body).unwrap();
        assert!(response.content.is_none());
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "find_command");
        assert_eq!(response.finish_reason, FinishReason::ToolCalls);
    }

    #[test]
    fn parse_no_choices() {
        let body = json!({ "choices": [] });
        let result = parse_response(&body);
        assert!(result.is_err());
    }

    #[test]
    fn format_assistant_message_with_content_and_tool_calls() {
        let messages = [Message::Assistant {
            content: Some("Let me check that.".into()),
            tool_calls: vec![ToolCall {
                id: "call_1".into(),
                name: "find_command".into(),
                arguments: json!({"command": "fd"}),
            }],
        }];
        let formatted = format_messages(&messages);
        assert_eq!(formatted[0]["role"], "assistant");
        assert_eq!(formatted[0]["content"], "Let me check that.");
        assert_eq!(formatted[0]["tool_calls"][0]["id"], "call_1");
        assert_eq!(
            formatted[0]["tool_calls"][0]["function"]["name"],
            "find_command"
        );
    }

    #[test]
    fn format_tool_result_message() {
        let messages = [Message::ToolResult {
            tool_call_id: "call_1".into(),
            name: "find_command".into(),
            content: "/bin/ls".into(),
        }];
        let formatted = format_messages(&messages);
        assert_eq!(formatted.len(), 1);
        assert_eq!(formatted[0]["role"], "tool");
        assert_eq!(formatted[0]["tool_call_id"], "call_1");
        assert_eq!(formatted[0]["content"], "/bin/ls");
    }

    #[test]
    fn parse_length_finish_reason() {
        let body = json!({
            "choices": [{
                "message": { "role": "assistant", "content": "incomplete" },
                "finish_reason": "length"
            }]
        });
        let response = parse_response(&body).unwrap();
        assert_eq!(response.finish_reason, FinishReason::Length);
    }

    #[test]
    fn parse_unknown_finish_reason() {
        let body = json!({
            "choices": [{
                "message": { "role": "assistant", "content": "test" },
                "finish_reason": "content_filter"
            }]
        });
        let response = parse_response(&body).unwrap();
        assert_eq!(
            response.finish_reason,
            FinishReason::Other("content_filter".into())
        );
    }

    #[test]
    fn format_tools_includes_function_schema() {
        let tools = vec![ToolDefinition {
            name: "find_command".into(),
            description: "Find a command".into(),
            parameters: json!({"type": "object"}),
        }];
        let result = format_tools(&tools);
        assert_eq!(result.as_array().unwrap().len(), 1);
        assert_eq!(result[0]["type"], "function");
        assert_eq!(result[0]["function"]["name"], "find_command");
    }

    #[test]
    fn format_tools_empty() {
        let tools: Vec<ToolDefinition> = vec![];
        let result = format_tools(&tools);
        assert!(result.as_array().unwrap().is_empty());
    }

    #[test]
    fn completions_provider_new() {
        let provider = CompletionsProvider::new(&make_config(
            "http://localhost:11434",
            Some("key"),
            "llama3",
            4096,
        ));
        assert_eq!(provider.base_url, "http://localhost:11434");
        assert_eq!(provider.api_key.as_deref(), Some("key"));
        assert_eq!(provider.model_name, "llama3");
        assert_eq!(provider.max_tokens, 4096);
        assert_eq!(provider.name(), "completions");
        assert_eq!(provider.endpoint_path, DEFAULT_ENDPOINT_PATH);
    }

    #[test]
    fn completions_provider_new_optional_key() {
        let provider =
            CompletionsProvider::new(&make_config("http://localhost:11434", None, "llama3", 4096));
        assert!(provider.api_key.is_none());
    }

    #[test]
    fn with_options_sets_provider_name() {
        let provider = CompletionsProvider::with_options(
            &make_config("https://openrouter.ai/api", Some("key"), "model", 4096),
            "openrouter",
            "/v1/chat/completions",
            "sk-test".into(),
        );
        assert_eq!(provider.name(), "openrouter");
        assert_eq!(provider.endpoint_path, "/v1/chat/completions");
        assert_eq!(provider.api_key.as_deref(), Some("sk-test"));
    }

    #[test]
    fn with_options_overrides_base_url_from_config() {
        let provider = CompletionsProvider::with_options(
            &make_config("https://custom.url", Some("key"), "model", 4096),
            "huggingface",
            "/chat/completions",
            "hf-key".into(),
        );
        assert_eq!(provider.base_url, "https://custom.url");
    }
}

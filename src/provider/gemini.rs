use async_trait::async_trait;
use serde_json::json;

use crate::config::Config;
use crate::debug_json;
use crate::error::{Error, Result};
use crate::provider::http::{send_and_parse, user_agent};
use crate::provider::{
    FinishReason, Message, Provider, ProviderResponse, ToolCall, ToolDefinition,
};

#[allow(dead_code)]
pub(crate) const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com";
pub(crate) const API_KEY_ENV: &str = "GEMINI_API_KEY";

pub fn create(config: &Config) -> Result<GeminiProvider> {
    let api_key = config
        .provider_settings
        .api_key
        .as_deref()
        .ok_or_else(|| {
            Error::ConfigError(format!(
                "{} is required for the gemini provider",
                API_KEY_ENV
            ))
        })?
        .to_string();

    Ok(GeminiProvider {
        client: reqwest::Client::new(),
        base_url: config.provider_settings.base_url.clone(),
        api_key,
        model_name: config.model_name.clone(),
        max_tokens: config.max_tokens,
    })
}

#[derive(Debug)]
pub struct GeminiProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model_name: String,
    #[allow(dead_code)]
    max_tokens: u32,
}

impl GeminiProvider {
    fn build_url(&self) -> String {
        format!(
            "{}/v1beta/models/{}:generateContent?key={}",
            self.base_url.trim_end_matches('/'),
            self.model_name,
            self.api_key,
        )
    }

    fn format_messages(&self, messages: &[Message]) -> (Option<String>, Vec<serde_json::Value>) {
        let mut system = None;
        let mut contents = Vec::new();

        for m in messages {
            match m {
                Message::System { content } => {
                    system = Some(content.clone());
                }
                Message::User { content } => {
                    contents.push(json!({
                        "role": "user",
                        "parts": [{ "text": content }]
                    }));
                }
                Message::Assistant {
                    content,
                    tool_calls,
                } => {
                    let mut parts = Vec::new();
                    if let Some(c) = content {
                        parts.push(json!({ "text": c }));
                    }
                    for tc in tool_calls {
                        parts.push(json!({
                            "functionCall": {
                                "name": tc.name,
                                "args": tc.arguments,
                            }
                        }));
                    }
                    if !parts.is_empty() {
                        contents.push(json!({
                            "role": "model",
                            "parts": parts
                        }));
                    }
                }
                Message::ToolResult {
                    tool_call_id: _,
                    name,
                    content,
                } => {
                    contents.push(json!({
                        "role": "user",
                        "parts": [{
                            "functionResponse": {
                                "name": name,
                                "response": { "content": content }
                            }
                        }]
                    }));
                }
            }
        }

        (system, contents)
    }

    fn format_tools(&self, tools: &[ToolDefinition]) -> serde_json::Value {
        let declarations: Vec<serde_json::Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                })
            })
            .collect();
        json!([{ "functionDeclarations": declarations }])
    }

    fn parse_response(&self, body: &serde_json::Value) -> Result<ProviderResponse> {
        let candidate = body
            .get("candidates")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .ok_or_else(|| Error::ResponseError("no candidates in response".into()))?;

        let content = candidate
            .get("content")
            .ok_or_else(|| Error::ResponseError("no content in candidate".into()))?;

        let parts = content
            .get("parts")
            .and_then(|p| p.as_array())
            .ok_or_else(|| Error::ResponseError("no parts in content".into()))?;

        let mut text_content = None;
        let mut tool_calls = Vec::new();
        let mut has_function_call = false;
        let mut call_index = 0u32;

        for part in parts {
            if let Some(fc) = part.get("functionCall") {
                has_function_call = true;
                let name = fc
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                let args = fc.get("args").cloned().unwrap_or(json!({}));
                tool_calls.push(ToolCall {
                    id: format!("call_{}", call_index),
                    name,
                    arguments: args,
                });
                call_index += 1;
            } else if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                if text_content.is_none() {
                    text_content = Some(text.to_string());
                }
            }
        }

        let finish_reason = if has_function_call {
            FinishReason::ToolCalls
        } else {
            FinishReason::Stop
        };

        Ok(ProviderResponse {
            content: text_content,
            tool_calls,
            finish_reason,
        })
    }
}

#[async_trait]
impl Provider for GeminiProvider {
    async fn send_request(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ProviderResponse> {
        let (system, contents) = self.format_messages(messages);

        let mut body = json!({
            "contents": contents,
        });

        if let Some(sys) = system {
            body["systemInstruction"] = json!({
                "parts": [{ "text": sys }]
            });
        }

        if !tools.is_empty() {
            body["tools"] = self.format_tools(tools);
        }

        let url = self.build_url();

        debug_json!("Request body", &body);

        let request = self
            .client
            .post(&url)
            .json(&body)
            .header("User-Agent", user_agent())
            .header("content-type", "application/json");

        send_and_parse(request, &url, |b| self.parse_response(b)).await
    }

    fn supports_tools(&self) -> bool {
        true
    }

    fn name(&self) -> &str {
        "gemini"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthStyle, ProviderSettings};
    use crate::provider::Provider;

    fn make_config(base_url: &str, api_key: Option<&str>) -> Config {
        Config {
            provider_name: "gemini".into(),
            model_name: "gemini-2.5-flash".into(),
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

    fn make_provider() -> GeminiProvider {
        GeminiProvider {
            client: reqwest::Client::new(),
            base_url: "https://generativelanguage.googleapis.com".into(),
            api_key: "test-key".into(),
            model_name: "gemini-2.5-flash".into(),
            max_tokens: 4096,
        }
    }

    #[test]
    fn create_with_key() {
        let provider = create(&make_config("https://custom.url", Some("test-key"))).unwrap();
        assert_eq!(provider.name(), "gemini");
    }

    #[test]
    fn create_without_key_errors() {
        let result = create(&make_config(DEFAULT_BASE_URL, None));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("GEMINI_API_KEY"));
    }

    #[test]
    fn build_url_includes_model_and_key() {
        let provider = make_provider();
        let url = provider.build_url();
        assert!(url.contains("gemini-2.5-flash"));
        assert!(url.contains("key=test-key"));
        assert!(url.contains("generateContent"));
    }

    #[test]
    fn build_url_trims_base_slash() {
        let provider = GeminiProvider {
            client: reqwest::Client::new(),
            base_url: "https://generativelanguage.googleapis.com/".into(),
            api_key: "k".into(),
            model_name: "m".into(),
            max_tokens: 4096,
        };
        let url = provider.build_url();
        assert!(url.contains("/v1beta/models/m:generateContent"));
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
        let (system, contents) = provider.format_messages(&messages);
        assert_eq!(system.as_deref(), Some("Be helpful."));
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["role"], "user");
    }

    #[test]
    fn format_messages_no_system() {
        let provider = make_provider();
        let messages = vec![Message::User {
            content: "hello".into(),
        }];
        let (system, contents) = provider.format_messages(&messages);
        assert!(system.is_none());
        assert_eq!(contents.len(), 1);
    }

    #[test]
    fn format_assistant_text_only() {
        let provider = make_provider();
        let messages = vec![Message::Assistant {
            content: Some("ls -la".into()),
            tool_calls: vec![],
        }];
        let (_, contents) = provider.format_messages(&messages);
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["role"], "model");
        let parts = contents[0]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0]["text"], "ls -la");
    }

    #[test]
    fn format_assistant_with_tool_calls() {
        let provider = make_provider();
        let messages = vec![Message::Assistant {
            content: None,
            tool_calls: vec![ToolCall {
                id: "call_0".into(),
                name: "find_command".into(),
                arguments: json!({"command": "fd"}),
            }],
        }];
        let (_, contents) = provider.format_messages(&messages);
        assert_eq!(contents.len(), 1);
        let parts = contents[0]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 1);
        let fc = &parts[0]["functionCall"];
        assert_eq!(fc["name"], "find_command");
        assert_eq!(fc["args"]["command"], "fd");
    }

    #[test]
    fn format_assistant_with_content_and_tool_calls() {
        let provider = make_provider();
        let messages = vec![Message::Assistant {
            content: Some("Let me check.".into()),
            tool_calls: vec![ToolCall {
                id: "call_0".into(),
                name: "find_command".into(),
                arguments: json!({"command": "fd"}),
            }],
        }];
        let (_, contents) = provider.format_messages(&messages);
        let parts = contents[0]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["text"], "Let me check.");
        assert_eq!(parts[1]["functionCall"]["name"], "find_command");
    }

    #[test]
    fn format_tool_result() {
        let provider = make_provider();
        let messages = vec![Message::ToolResult {
            tool_call_id: "call_0".into(),
            name: "find_command".into(),
            content: "/bin/ls".into(),
        }];
        let (_, contents) = provider.format_messages(&messages);
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["role"], "user");
        let parts = contents[0]["parts"].as_array().unwrap();
        let fr = &parts[0]["functionResponse"];
        assert_eq!(fr["name"], "find_command");
        assert_eq!(fr["response"]["content"], "/bin/ls");
    }

    #[test]
    fn format_tools_uses_function_declarations() {
        let provider = make_provider();
        let tools = vec![ToolDefinition {
            name: "find_command".into(),
            description: "Find a command".into(),
            parameters: json!({"type": "object"}),
        }];
        let result = provider.format_tools(&tools);
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        let decls = arr[0]["functionDeclarations"].as_array().unwrap();
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0]["name"], "find_command");
    }

    #[test]
    fn parse_text_response() {
        let provider = make_provider();
        let body = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{ "text": "ls -la" }]
                },
                "finishReason": "STOP"
            }]
        });
        let response = provider.parse_response(&body).unwrap();
        assert_eq!(response.content, Some("ls -la".into()));
        assert!(response.tool_calls.is_empty());
        assert_eq!(response.finish_reason, FinishReason::Stop);
    }

    #[test]
    fn parse_function_call_response() {
        let provider = make_provider();
        let body = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{
                        "functionCall": { "name": "find_command", "args": { "command": "fd" } }
                    }]
                },
                "finishReason": "STOP"
            }]
        });
        let response = provider.parse_response(&body).unwrap();
        assert!(response.content.is_none());
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "find_command");
        assert_eq!(response.tool_calls[0].id, "call_0");
        assert_eq!(response.finish_reason, FinishReason::ToolCalls);
    }

    #[test]
    fn parse_stop_ambiguity_resolved_for_tool_calls() {
        let provider = make_provider();
        let body = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{
                        "functionCall": { "name": "ask_user", "args": { "question": "Which file?" } }
                    }]
                },
                "finishReason": "STOP"
            }]
        });
        let response = provider.parse_response(&body).unwrap();
        assert_eq!(response.finish_reason, FinishReason::ToolCalls);
    }

    #[test]
    fn parse_stop_ambiguity_resolved_for_text() {
        let provider = make_provider();
        let body = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{ "text": "fd -e pdf" }]
                },
                "finishReason": "STOP"
            }]
        });
        let response = provider.parse_response(&body).unwrap();
        assert_eq!(response.finish_reason, FinishReason::Stop);
    }

    #[test]
    fn parse_multiple_function_calls() {
        let provider = make_provider();
        let body = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        { "functionCall": { "name": "find_command", "args": { "command": "fd" } } },
                        { "functionCall": { "name": "ask_user", "args": { "question": "ok?" } } }
                    ]
                },
                "finishReason": "STOP"
            }]
        });
        let response = provider.parse_response(&body).unwrap();
        assert_eq!(response.tool_calls.len(), 2);
        assert_eq!(response.tool_calls[0].id, "call_0");
        assert_eq!(response.tool_calls[1].id, "call_1");
    }

    #[test]
    fn parse_no_candidates() {
        let provider = make_provider();
        let body = json!({ "candidates": [] });
        let result = provider.parse_response(&body);
        assert!(result.is_err());
    }

    #[test]
    fn supports_tools_true() {
        let provider = create(&make_config(
            "https://generativelanguage.googleapis.com",
            Some("key"),
        ))
        .unwrap();
        assert!(provider.supports_tools());
    }
}

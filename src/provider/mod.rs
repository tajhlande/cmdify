pub mod completions;

use async_trait::async_trait;

use crate::config::Config;
use crate::error::{Error, Result};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Message {
    System {
        content: String,
    },
    User {
        content: String,
    },
    Assistant {
        content: Option<String>,
        tool_calls: Vec<ToolCall>,
    },
    ToolResult {
        tool_call_id: String,
        name: String,
        content: String,
    },
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProviderResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: FinishReason,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FinishReason {
    Stop,
    ToolCalls,
    Length,
    Other(String),
}

#[async_trait]
#[allow(dead_code)]
pub trait Provider: Send + Sync {
    async fn send_request(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ProviderResponse>;

    fn supports_tools(&self) -> bool;
    fn name(&self) -> &str;
}

pub fn create_provider(config: &Config) -> Result<Box<dyn Provider>> {
    match config.provider_name.as_str() {
        "completions" => Ok(Box::new(completions::CompletionsProvider::new(config)?)),
        other => Err(Error::ConfigError(format!("unknown provider: {}", other))),
    }
}

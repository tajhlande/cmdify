pub mod completions;

use async_trait::async_trait;

use crate::config::Config;
use crate::error::{Error, Result};

#[derive(Debug, Clone)]
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
        #[allow(dead_code)]
        name: String,
        content: String,
    },
}

#[derive(Debug, Clone)]
pub struct ProviderResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    #[allow(dead_code)]
    pub finish_reason: FinishReason,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
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
pub trait Provider: Send + Sync {
    async fn send_request(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ProviderResponse>;

    #[allow(dead_code)]
    fn supports_tools(&self) -> bool;
    #[allow(dead_code)]
    fn name(&self) -> &str;
}

// TODO(Phase 4-6): Add provider implementations for openai, anthropic, gemini,
// openrouter, huggingface, zai, minimax, qwen, kimi, mistral, ollama, responses.
// Currently only the completions provider is implemented (Phase 1).
pub fn create_provider(config: &Config) -> Result<Box<dyn Provider>> {
    match config.provider_name.as_str() {
        "completions" => Ok(Box::new(completions::CompletionsProvider::new(config)?)),
        other => Err(Error::ConfigError(format!("unknown provider: {}", other))),
    }
}

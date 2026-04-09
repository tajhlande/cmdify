macro_rules! thin_provider {
    (
        provider_name = $name:literal,
        api_key_env = $key_env:literal,
        default_base_url = $default_url:literal,
        default_model = $default_model:literal,
        endpoint_path = $endpoint:literal,
        backend_mod = $backend_mod:ident,
        backend_ty = $backend_ty:ident,
    ) => {
        use $crate::config::Config;
        use $crate::error::Result;
        use $crate::provider::$backend_mod::$backend_ty;

        pub(crate) const API_KEY_ENV: &str = $key_env;
        #[allow(dead_code)]
        pub(crate) const DEFAULT_BASE_URL: &str = $default_url;
        #[allow(dead_code)]
        pub(crate) const ENDPOINT_PATH: &str = $endpoint;

        pub fn create(config: &Config) -> Result<$backend_ty> {
            let api_key = config
                .provider_settings
                .api_key
                .as_deref()
                .ok_or_else(|| {
                    $crate::error::Error::ConfigError(format!(
                        "{} is required for the {} provider",
                        API_KEY_ENV, $name
                    ))
                })?
                .to_string();

            Ok(<$backend_ty>::with_options(
                config,
                $name,
                ENDPOINT_PATH,
                api_key,
            ))
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use $crate::config::{AuthStyle, ProviderSettings};
            use $crate::provider::Provider;

            fn make_config(base_url: &str, api_key: Option<&str>) -> Config {
                Config {
                    provider_name: $name.into(),
                    model_name: $default_model.into(),
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
            fn create_with_key() {
                let provider =
                    create(&make_config("https://custom.url", Some("test-key"))).unwrap();
                assert_eq!(provider.name(), $name);
            }

            #[test]
            fn create_without_key_errors() {
                let result = create(&make_config(DEFAULT_BASE_URL, None));
                assert!(result.is_err());
                assert!(result.unwrap_err().to_string().contains($key_env));
            }
        }
    };
}

pub mod anthropic;
pub mod completions;
pub mod gemini;
pub mod http;
pub mod huggingface;
pub mod kimi;
pub mod minimax;
pub mod mistral;
pub mod ollama;
pub mod openai;
pub mod openrouter;
pub mod qwen;
pub mod responses;
pub mod zai;

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

pub fn create_provider(config: &Config) -> Result<Box<dyn Provider>> {
    match config.provider_name.as_str() {
        "completions" => Ok(Box::new(completions::CompletionsProvider::new(config))),
        "responses" => Ok(Box::new(responses::ResponsesProvider::new(config))),
        "openai" => Ok(Box::new(openai::create(config)?)),
        "anthropic" => Ok(Box::new(anthropic::create(config)?)),
        "gemini" => Ok(Box::new(gemini::create(config)?)),
        "openrouter" => Ok(Box::new(openrouter::create(config)?)),
        "huggingface" => Ok(Box::new(huggingface::create(config)?)),
        "zai" => Ok(Box::new(zai::create(config)?)),
        "minimax" => Ok(Box::new(minimax::create(config)?)),
        "qwen" => Ok(Box::new(qwen::create(config)?)),
        "kimi" => Ok(Box::new(kimi::create(config)?)),
        "mistral" => Ok(Box::new(mistral::create(config)?)),
        "ollama" => Ok(Box::new(ollama::create(config))),
        other => Err(Error::ConfigError(format!("unknown provider: {}", other))),
    }
}

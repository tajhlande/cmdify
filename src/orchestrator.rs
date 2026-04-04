use crate::config::Config;
use crate::error::Error;
use crate::logger::CmdifyLogger;
use crate::provider::{create_provider, Message};
use crate::tools::ToolRegistry;

const MAX_TOOL_ITERATIONS: usize = 10;

pub async fn run(
    prompt: &str,
    config: &Config,
    logger: Option<&CmdifyLogger>,
) -> Result<String, Error> {
    let system_prompt = crate::prompt::load_system_prompt(config)?;

    let registry = ToolRegistry::new(config.blind, config.no_tools);
    let tool_definitions = registry.definitions();
    let provider = create_provider(config)?;

    let mut messages = vec![
        Message::System {
            content: system_prompt,
        },
        Message::User {
            content: prompt.to_string(),
        },
    ];

    for _ in 0..MAX_TOOL_ITERATIONS {
        let response = provider.send_request(&messages, &tool_definitions).await?;

        if response.content.is_some() && response.tool_calls.is_empty() {
            return response
                .content
                .map(|c| c.trim().to_string())
                .ok_or_else(|| Error::ProviderError("empty response from provider".into()));
        }

        if !response.tool_calls.is_empty() {
            messages.push(Message::Assistant {
                content: response.content,
                tool_calls: response.tool_calls.clone(),
            });

            for tool_call in &response.tool_calls {
                let result = registry
                    .execute(&tool_call.name, tool_call.arguments.clone(), logger)
                    .await?;
                messages.push(Message::ToolResult {
                    tool_call_id: tool_call.id.clone(),
                    name: tool_call.name.clone(),
                    content: result.content,
                });
            }

            continue;
        }

        return Err(Error::ProviderError(
            "empty response from provider (try increasing CMDIFY_MAX_TOKENS)".into(),
        ));
    }

    Err(Error::ProviderError(
        "tool call loop exceeded maximum iterations (10)".into(),
    ))
}

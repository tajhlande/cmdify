use crate::config::Config;
use crate::debug;
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

    debug!(
        "Provider: {} | Tools registered: {}",
        config.provider_name,
        if tool_definitions.is_empty() {
            "none".to_string()
        } else {
            tool_definitions
                .iter()
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );

    let mut messages = vec![
        Message::System {
            content: system_prompt,
        },
        Message::User {
            content: prompt.to_string(),
        },
    ];

    for iteration in 0..MAX_TOOL_ITERATIONS {
        debug!(
            "Loop iteration {}/{}: sending request with {} messages",
            iteration + 1,
            MAX_TOOL_ITERATIONS,
            messages.len()
        );

        let response = provider.send_request(&messages, &tool_definitions).await?;

        if response.content.is_some() && response.tool_calls.is_empty() {
            debug!("Provider returned final answer");
            return response
                .content
                .map(|c| c.trim().to_string())
                .ok_or_else(|| Error::ProviderError("empty response from provider".into()));
        }

        if !response.tool_calls.is_empty() {
            debug!(
                "Provider requested {} tool call(s)",
                response.tool_calls.len()
            );

            messages.push(Message::Assistant {
                content: response.content,
                tool_calls: response.tool_calls.clone(),
            });

            for tool_call in &response.tool_calls {
                debug!("Tool call: {}({})", tool_call.name, tool_call.arguments);
                let result = registry
                    .execute(&tool_call.name, tool_call.arguments.clone(), logger)
                    .await?;
                debug!("Tool result: {} → {}", tool_call.name, result.content);
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

    Err(Error::ProviderError(format!(
        "tool call loop exceeded maximum iterations ({})",
        MAX_TOOL_ITERATIONS
    )))
}

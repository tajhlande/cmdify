use crate::config::Config;
use crate::error::Error;
use crate::provider::{create_provider, Message};

pub async fn run(prompt: &str, config: &Config) -> Result<String, Error> {
    let system_prompt = crate::prompt::load_system_prompt(config)?;

    let messages = vec![
        Message::System {
            content: system_prompt,
        },
        Message::User {
            content: prompt.to_string(),
        },
    ];

    let provider = create_provider(config)?;
    let response = provider.send_request(&messages, &[]).await?;

    match response.content {
        Some(content) => Ok(content.trim().to_string()),
        None => Err(Error::ProviderError(
            "empty response from provider (try increasing CMDIFY_MAX_TOKENS)".into(),
        )),
    }
}

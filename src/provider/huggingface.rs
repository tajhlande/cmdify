use crate::config::Config;
use crate::error::Result;
use crate::provider::completions::CompletionsProvider;

#[cfg(test)]
const DEFAULT_BASE_URL: &str = "https://router.huggingface.co/v1";
const API_KEY_ENV: &str = "HUGGINGFACE_API_KEY";
const ENDPOINT_PATH: &str = "/chat/completions";

pub fn create(config: &Config) -> Result<CompletionsProvider> {
    let api_key = config
        .provider_settings
        .api_key
        .as_deref()
        .ok_or_else(|| {
            crate::error::Error::ConfigError(format!(
                "{} is required for the huggingface provider",
                API_KEY_ENV
            ))
        })?
        .to_string();

    Ok(CompletionsProvider::with_options(
        config,
        "huggingface",
        ENDPOINT_PATH,
        api_key,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthStyle, ProviderSettings};
    use crate::provider::Provider;

    fn make_config(base_url: &str, api_key: Option<&str>) -> Config {
        Config {
            provider_name: "huggingface".into(),
            model_name: "mistralai/Mistral-7B-Instruct-v0.3".into(),
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
        let provider = create(&make_config("https://custom.url", Some("hf_test"))).unwrap();
        assert_eq!(provider.name(), "huggingface");
    }

    #[test]
    fn create_without_key_errors() {
        let result = create(&make_config(DEFAULT_BASE_URL, None));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("HUGGINGFACE_API_KEY"));
    }
}

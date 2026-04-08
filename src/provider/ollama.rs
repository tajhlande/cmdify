use crate::config::Config;
use crate::provider::completions::CompletionsProvider;

pub fn create(config: &Config) -> CompletionsProvider {
    CompletionsProvider::new(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthStyle, ProviderSettings};
    use crate::provider::Provider;

    fn make_config(base_url: &str) -> Config {
        Config {
            provider_name: "ollama".into(),
            model_name: "llama3".into(),
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
                api_key: None,
                base_url: base_url.into(),
                auth_style: AuthStyle::Header {
                    name: "Authorization".into(),
                    prefix: "Bearer ".into(),
                },
            },
        }
    }

    #[test]
    fn create_ollama_provider() {
        let provider = create(&make_config("http://localhost:11434"));
        assert_eq!(provider.name(), "completions");
        assert_eq!(provider.supports_tools(), true);
    }

    #[test]
    fn create_ollama_custom_url() {
        let provider = create(&make_config("http://192.168.1.100:11434"));
        assert_eq!(provider.name(), "completions");
    }
}

use std::env;

use crate::config::Config;
use crate::error::Result;

pub const EMBEDDED_SYSTEM_PROMPT: &str = include_str!("system_prompt.txt");

pub fn load_system_prompt(config: &Config) -> Result<String> {
    let base_prompt = if let Some(ref path) = config.system_prompt_override {
        std::fs::read_to_string(path)?
    } else {
        EMBEDDED_SYSTEM_PROMPT.to_string()
    };

    let shell = env::var("SHELL")
        .ok()
        .and_then(|s| s.rsplit('/').next().map(|n| n.to_string()))
        .unwrap_or_else(|| "bash".to_string());

    Ok(format!("{}\n\nThe user's shell is {}.", base_prompt, shell))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env_lock<F: FnOnce()>(f: F) {
        let _lock = ENV_LOCK.lock().unwrap();
        f();
    }

    fn make_config_with_override(override_path: Option<&str>) -> Config {
        Config {
            provider_name: "completions".into(),
            model_name: "test".into(),
            max_tokens: 4096,
            system_prompt_override: override_path.map(|p| p.to_string()),
            provider_settings: crate::config::ProviderSettings {
                api_key: None,
                base_url: "http://localhost".into(),
                auth_style: crate::config::AuthStyle::Header {
                    name: "Authorization".into(),
                    prefix: "Bearer ".into(),
                },
            },
        }
    }

    #[test]
    fn embedded_prompt_not_empty() {
        assert!(!EMBEDDED_SYSTEM_PROMPT.is_empty());
    }

    #[test]
    fn load_embedded_prompt() {
        with_env_lock(|| {
            env::remove_var("SHELL");
            let config = make_config_with_override(None);
            let prompt = load_system_prompt(&config).unwrap();
            assert!(prompt.contains("shell command generator"));
            assert!(prompt.contains("The user's shell is bash."));
        });
    }

    #[test]
    fn shell_detection_zsh() {
        with_env_lock(|| {
            env::set_var("SHELL", "/bin/zsh");
            let config = make_config_with_override(None);
            let prompt = load_system_prompt(&config).unwrap();
            assert!(prompt.contains("The user's shell is zsh."));
        });
    }

    #[test]
    fn shell_detection_defaults_to_bash() {
        with_env_lock(|| {
            env::remove_var("SHELL");
            let config = make_config_with_override(None);
            let prompt = load_system_prompt(&config).unwrap();
            assert!(prompt.contains("The user's shell is bash."));
        });
    }
}

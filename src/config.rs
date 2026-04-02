use std::env;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};

const DEFAULT_MAX_TOKENS: u32 = 16384;

fn config_file_path() -> Option<PathBuf> {
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        let p = PathBuf::from(xdg).join("cmdify").join("config.toml");
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(home) = env::var("HOME") {
        let p = PathBuf::from(home)
            .join(".config")
            .join("cmdify")
            .join("config.toml");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn load_file_config(path: &Path) -> Result<FileConfig> {
    let content = std::fs::read_to_string(path)?;
    let config: FileConfig = toml::from_str(&content)
        .map_err(|e| Error::ConfigError(format!("failed to parse {}: {}", path.display(), e)))?;
    Ok(config)
}

#[derive(Debug, Default, Deserialize, Clone)]
#[allow(dead_code)]
struct FileConfig {
    provider_name: Option<String>,
    model_name: Option<String>,
    max_tokens: Option<u32>,
    system_prompt: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let file_config = match config_file_path() {
            Some(path) => Some(load_file_config(&path)?),
            None => None,
        };

        let provider_name = env::var("CMDIFY_PROVIDER_NAME").ok()
            .or_else(|| file_config.as_ref().and_then(|f| f.provider_name.clone()))
            .ok_or_else(|| Error::ConfigError(
                "CMDIFY_PROVIDER_NAME is required (set env var or provider_name in config file)".into(),
            ))?;

        let model_name = env::var("CMDIFY_MODEL_NAME")
            .ok()
            .or_else(|| file_config.as_ref().and_then(|f| f.model_name.clone()))
            .ok_or_else(|| {
                Error::ConfigError(
                    "CMDIFY_MODEL_NAME is required (set env var or model_name in config file)"
                        .into(),
                )
            })?;

        let max_tokens = env::var("CMDIFY_MAX_TOKENS")
            .ok()
            .and_then(|v| v.parse().ok())
            .or_else(|| file_config.as_ref().and_then(|f| f.max_tokens))
            .unwrap_or(DEFAULT_MAX_TOKENS);

        let system_prompt_override = env::var("CMDIFY_SYSTEM_PROMPT")
            .ok()
            .or_else(|| file_config.as_ref().and_then(|f| f.system_prompt.clone()));

        let provider_settings = ProviderSettings::from_env(&provider_name)?;

        Ok(Self {
            provider_name,
            model_name,
            max_tokens,
            system_prompt_override,
            provider_settings,
        })
    }
}

impl ProviderSettings {
    pub fn from_env(provider_name: &str) -> Result<Self> {
        match provider_name {
            "completions" => {
                let base_url = env::var("CMDIFY_COMPLETIONS_URL").map_err(|_| {
                    Error::ConfigError(
                        "CMDIFY_COMPLETIONS_URL is required for the completions provider".into(),
                    )
                })?;
                let api_key = env::var("CMDIFY_COMPLETIONS_KEY").ok();
                Ok(Self {
                    api_key,
                    base_url,
                    auth_style: AuthStyle::Header {
                        name: "Authorization".into(),
                        prefix: "Bearer ".into(),
                    },
                })
            }
            other => Err(Error::ConfigError(format!("unknown provider: {}", other))),
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AuthStyle {
    Header { name: String, prefix: String },
    QueryParam { name: String },
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProviderSettings {
    pub api_key: Option<String>,
    pub base_url: String,
    pub auth_style: AuthStyle,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub provider_name: String,
    pub model_name: String,
    pub max_tokens: u32,
    pub system_prompt_override: Option<String>,
    pub provider_settings: ProviderSettings,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env_lock<F: FnOnce()>(f: F) {
        let _lock = ENV_LOCK.lock().unwrap();
        f();
    }

    fn cleanup_vars() {
        env::remove_var("CMDIFY_PROVIDER_NAME");
        env::remove_var("CMDIFY_MODEL_NAME");
        env::remove_var("CMDIFY_COMPLETIONS_URL");
        env::remove_var("CMDIFY_COMPLETIONS_KEY");
        env::remove_var("CMDIFY_MAX_TOKENS");
        env::remove_var("CMDIFY_SYSTEM_PROMPT");
        env::remove_var("XDG_CONFIG_HOME");
    }

    fn setup_completions_env() {
        cleanup_vars();
        env::set_var("CMDIFY_PROVIDER_NAME", "completions");
        env::set_var("CMDIFY_MODEL_NAME", "llama3");
        env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");
    }

    #[test]
    fn missing_provider_name() {
        with_env_lock(|| {
            cleanup_vars();
            let result = Config::from_env();
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(err.contains("CMDIFY_PROVIDER_NAME"));
        });
    }

    #[test]
    fn missing_model_name() {
        with_env_lock(|| {
            env::set_var("CMDIFY_PROVIDER_NAME", "completions");
            env::remove_var("CMDIFY_MODEL_NAME");
            let result = Config::from_env();
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(err.contains("CMDIFY_MODEL_NAME"));
        });
    }

    #[test]
    fn completions_config_full() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_COMPLETIONS_KEY", "test-key");
            env::remove_var("CMDIFY_MAX_TOKENS");
            env::remove_var("CMDIFY_SYSTEM_PROMPT");
            let config = Config::from_env().unwrap();
            assert_eq!(config.provider_name, "completions");
            assert_eq!(config.model_name, "llama3");
            assert_eq!(config.max_tokens, DEFAULT_MAX_TOKENS);
            assert_eq!(
                config.provider_settings.api_key.as_deref(),
                Some("test-key")
            );
            assert_eq!(config.provider_settings.base_url, "http://localhost:11434");
        });
    }

    #[test]
    fn completions_config_no_key() {
        with_env_lock(|| {
            setup_completions_env();
            env::remove_var("CMDIFY_COMPLETIONS_KEY");
            env::remove_var("CMDIFY_MAX_TOKENS");
            env::remove_var("CMDIFY_SYSTEM_PROMPT");
            let config = Config::from_env().unwrap();
            assert!(config.provider_settings.api_key.is_none());
        });
    }

    #[test]
    fn missing_completions_url() {
        with_env_lock(|| {
            env::set_var("CMDIFY_PROVIDER_NAME", "completions");
            env::set_var("CMDIFY_MODEL_NAME", "llama3");
            env::remove_var("CMDIFY_COMPLETIONS_URL");
            let result = Config::from_env();
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(err.contains("CMDIFY_COMPLETIONS_URL"));
        });
    }

    #[test]
    fn max_tokens_override() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_MAX_TOKENS", "1024");
            env::remove_var("CMDIFY_COMPLETIONS_KEY");
            env::remove_var("CMDIFY_SYSTEM_PROMPT");
            let config = Config::from_env().unwrap();
            assert_eq!(config.max_tokens, 1024);
        });
    }

    #[test]
    fn unknown_provider() {
        with_env_lock(|| {
            env::set_var("CMDIFY_PROVIDER_NAME", "fake_provider");
            env::set_var("CMDIFY_MODEL_NAME", "model");
            let result = Config::from_env();
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(err.contains("unknown provider"));
        });
    }

    // --- TOML config file tests ---

    fn write_toml_config(dir: &Path, content: &str) -> PathBuf {
        let config_dir = dir.join("cmdify");
        fs::create_dir_all(&config_dir).unwrap();
        let config_path = config_dir.join("config.toml");
        fs::write(&config_path, content).unwrap();
        config_path
    }

    #[test]
    fn config_file_provides_defaults() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            write_toml_config(
                tmp.path(),
                r#"
                provider_name = "completions"
                model_name = "llama3"
                "#,
            );
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

            let config = Config::from_env().unwrap();
            assert_eq!(config.provider_name, "completions");
            assert_eq!(config.model_name, "llama3");
            assert_eq!(config.max_tokens, DEFAULT_MAX_TOKENS);
        });
    }

    #[test]
    fn env_var_overrides_config_file() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            write_toml_config(
                tmp.path(),
                r#"
                provider_name = "completions"
                model_name = "from-file-model"
                max_tokens = 2048
                "#,
            );
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_PROVIDER_NAME", "completions");
            env::set_var("CMDIFY_MODEL_NAME", "from-env-model");
            env::set_var("CMDIFY_MAX_TOKENS", "8192");
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

            let config = Config::from_env().unwrap();
            assert_eq!(config.model_name, "from-env-model");
            assert_eq!(config.max_tokens, 8192);
        });
    }

    #[test]
    fn max_tokens_from_config_file() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            write_toml_config(
                tmp.path(),
                r#"
                provider_name = "completions"
                model_name = "llama3"
                max_tokens = 2048
                "#,
            );
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");
            env::remove_var("CMDIFY_MAX_TOKENS");

            let config = Config::from_env().unwrap();
            assert_eq!(config.max_tokens, 2048);
        });
    }

    #[test]
    fn system_prompt_from_config_file() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            write_toml_config(
                tmp.path(),
                r#"
                provider_name = "completions"
                model_name = "llama3"
                system_prompt = "/my/prompt.txt"
                "#,
            );
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

            let config = Config::from_env().unwrap();
            assert_eq!(
                config.system_prompt_override.as_deref(),
                Some("/my/prompt.txt")
            );
        });
    }

    #[test]
    fn system_prompt_env_overrides_config_file() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            write_toml_config(
                tmp.path(),
                r#"
                provider_name = "completions"
                model_name = "llama3"
                system_prompt = "/from-file.txt"
                "#,
            );
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");
            env::set_var("CMDIFY_SYSTEM_PROMPT", "/from-env.txt");

            let config = Config::from_env().unwrap();
            assert_eq!(
                config.system_prompt_override.as_deref(),
                Some("/from-env.txt")
            );
        });
    }

    #[test]
    fn missing_config_file_graceful_fallback() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_PROVIDER_NAME", "completions");
            env::set_var("CMDIFY_MODEL_NAME", "llama3");
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

            let config = Config::from_env().unwrap();
            assert_eq!(config.provider_name, "completions");
        });
    }

    #[test]
    fn xdg_takes_precedence_over_home() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            let xdg_dir = tmp.path().join("xdg");
            let home_dir = tmp.path().join("home");
            fs::create_dir_all(xdg_dir.join("cmdify")).unwrap();
            fs::write(
                xdg_dir.join("cmdify").join("config.toml"),
                r#"
                provider_name = "completions"
                model_name = "xdg-model"
                "#,
            )
            .unwrap();
            fs::create_dir_all(home_dir.join(".config").join("cmdify")).unwrap();
            fs::write(
                home_dir.join(".config").join("cmdify").join("config.toml"),
                r#"
                provider_name = "completions"
                model_name = "home-model"
                "#,
            )
            .unwrap();

            env::set_var("XDG_CONFIG_HOME", &xdg_dir);
            env::set_var("HOME", &home_dir);
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

            let config = Config::from_env().unwrap();
            assert_eq!(config.model_name, "xdg-model");
        });
    }

    #[test]
    fn home_fallback_when_no_xdg() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            let home_dir = tmp.path().join("home");
            fs::create_dir_all(home_dir.join(".config").join("cmdify")).unwrap();
            fs::write(
                home_dir.join(".config").join("cmdify").join("config.toml"),
                r#"
                provider_name = "completions"
                model_name = "home-model"
                "#,
            )
            .unwrap();

            env::remove_var("XDG_CONFIG_HOME");
            env::set_var("HOME", &home_dir);
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

            let config = Config::from_env().unwrap();
            assert_eq!(config.model_name, "home-model");
        });
    }

    #[test]
    fn invalid_toml_errors() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            write_toml_config(tmp.path(), "this is not valid toml [[[");
            env::set_var("XDG_CONFIG_HOME", tmp.path());

            let result = Config::from_env();
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(err.contains("failed to parse"));
        });
    }
}

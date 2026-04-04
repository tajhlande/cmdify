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

fn parse_bool_env(var: &str) -> Option<bool> {
    match std::env::var(var).ok()?.to_lowercase().as_str() {
        "1" | "true" | "yes" => Some(true),
        "0" | "false" | "no" => Some(false),
        _ => None,
    }
}

fn parse_u8_env(var: &str) -> Option<u8> {
    std::env::var(var).ok()?.parse().ok()
}

#[derive(Debug, Default, Deserialize, Clone)]
struct ProviderUrls {
    completions_url: Option<String>,
    responses_url: Option<String>,
    openai_base_url: Option<String>,
    anthropic_base_url: Option<String>,
    gemini_base_url: Option<String>,
    mistral_base_url: Option<String>,
    qwen_base_url: Option<String>,
    kimi_base_url: Option<String>,
    openrouter_base_url: Option<String>,
    huggingface_base_url: Option<String>,
    zai_base_url: Option<String>,
    minimax_base_url: Option<String>,
    ollama_base_url: Option<String>,
}

#[derive(Debug, Default, Deserialize, Clone)]
#[allow(dead_code)]
struct FileConfig {
    provider_name: Option<String>,
    model_name: Option<String>,
    max_tokens: Option<u32>,
    system_prompt_file: Option<String>,
    spinner: Option<u8>,
    allow_unsafe: Option<bool>,
    quiet: Option<bool>,
    blind: Option<bool>,
    no_tools: Option<bool>,
    yolo: Option<bool>,
    #[serde(default)]
    providers: ProviderUrls,
}

impl Config {
    pub fn from_env(explicit_config_path: Option<&Path>) -> Result<Self> {
        let effective_path = match explicit_config_path {
            Some(path) => Some(path.to_path_buf()),
            None => env::var("CMDIFY_CONFIG").ok().map(PathBuf::from),
        };

        let file_config = match effective_path {
            Some(path) => {
                if !path.exists() {
                    return Err(Error::ConfigError(format!(
                        "config file not found: {}",
                        path.display()
                    )));
                }
                Some(load_file_config(&path)?)
            }
            None => match config_file_path() {
                Some(path) => Some(load_file_config(&path)?),
                None => None,
            },
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

        let system_prompt_override = env::var("CMDIFY_SYSTEM_PROMPT_FILE").ok().or_else(|| {
            file_config
                .as_ref()
                .and_then(|f| f.system_prompt_file.clone())
        });

        let spinner = parse_u8_env("CMDIFY_SPINNER")
            .or_else(|| file_config.as_ref().and_then(|f| f.spinner))
            .unwrap_or(1);

        let allow_unsafe = parse_bool_env("CMDIFY_UNSAFE")
            .or(file_config.as_ref().and_then(|f| f.allow_unsafe))
            .unwrap_or(false);

        let quiet = parse_bool_env("CMDIFY_QUIET")
            .or(file_config.as_ref().and_then(|f| f.quiet))
            .unwrap_or(false);

        let blind = parse_bool_env("CMDIFY_BLIND")
            .or(file_config.as_ref().and_then(|f| f.blind))
            .unwrap_or(false);

        let no_tools = parse_bool_env("CMDIFY_NO_TOOLS")
            .or(file_config.as_ref().and_then(|f| f.no_tools))
            .unwrap_or(false);

        let yolo = parse_bool_env("CMDIFY_YOLO")
            .or(file_config.as_ref().and_then(|f| f.yolo))
            .unwrap_or(false);

        let provider_settings = ProviderSettings::from_env(&provider_name, file_config.as_ref())?;

        Ok(Self {
            provider_name,
            model_name,
            max_tokens,
            system_prompt_override,
            spinner,
            allow_unsafe,
            quiet,
            blind,
            no_tools,
            yolo,
            provider_settings,
        })
    }
}

impl ProviderSettings {
    #[allow(private_interfaces)]
    pub fn from_env(provider_name: &str, file_config: Option<&FileConfig>) -> Result<Self> {
        let urls = file_config.map(|f| &f.providers);

        match provider_name {
            "completions" => {
                let base_url = env::var("CMDIFY_COMPLETIONS_URL")
                    .ok()
                    .or_else(|| urls.and_then(|u| u.completions_url.clone()))
                    .ok_or_else(|| {
                        Error::ConfigError(
                            "CMDIFY_COMPLETIONS_URL is required for the completions provider"
                                .into(),
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
            "openai" => {
                let base_url = env::var("OPENAI_BASE_URL")
                    .ok()
                    .or_else(|| urls.and_then(|u| u.openai_base_url.clone()))
                    .unwrap_or_else(|| "https://api.openai.com".into());
                let api_key = env::var("OPENAI_API_KEY").ok();
                Ok(Self {
                    api_key,
                    base_url,
                    auth_style: AuthStyle::Header {
                        name: "Authorization".into(),
                        prefix: "Bearer ".into(),
                    },
                })
            }
            "anthropic" => {
                let base_url = env::var("ANTHROPIC_BASE_URL")
                    .ok()
                    .or_else(|| urls.and_then(|u| u.anthropic_base_url.clone()))
                    .unwrap_or_else(|| "https://api.anthropic.com".into());
                let api_key = env::var("ANTHROPIC_API_KEY").ok();
                Ok(Self {
                    api_key,
                    base_url,
                    auth_style: AuthStyle::Header {
                        name: "x-api-key".into(),
                        prefix: String::new(),
                    },
                })
            }
            "gemini" => {
                let base_url = env::var("GEMINI_BASE_URL")
                    .ok()
                    .or_else(|| urls.and_then(|u| u.gemini_base_url.clone()))
                    .unwrap_or_else(|| "https://generativelanguage.googleapis.com".into());
                let api_key = env::var("GEMINI_API_KEY").ok();
                Ok(Self {
                    api_key,
                    base_url,
                    auth_style: AuthStyle::QueryParam { name: "key".into() },
                })
            }
            "mistral" => {
                let base_url = env::var("MISTRAL_BASE_URL")
                    .ok()
                    .or_else(|| urls.and_then(|u| u.mistral_base_url.clone()))
                    .unwrap_or_else(|| "https://api.mistral.ai".into());
                let api_key = env::var("MISTRAL_API_KEY").ok();
                Ok(Self {
                    api_key,
                    base_url,
                    auth_style: AuthStyle::Header {
                        name: "Authorization".into(),
                        prefix: "Bearer ".into(),
                    },
                })
            }
            "qwen" => {
                let base_url = env::var("QWEN_BASE_URL")
                    .ok()
                    .or_else(|| urls.and_then(|u| u.qwen_base_url.clone()))
                    .unwrap_or_else(|| "https://dashscope.aliyuncs.com/compatible-mode".into());
                let api_key = env::var("QWEN_API_KEY").ok();
                Ok(Self {
                    api_key,
                    base_url,
                    auth_style: AuthStyle::Header {
                        name: "Authorization".into(),
                        prefix: "Bearer ".into(),
                    },
                })
            }
            "kimi" => {
                let base_url = env::var("KIMI_BASE_URL")
                    .ok()
                    .or_else(|| urls.and_then(|u| u.kimi_base_url.clone()))
                    .unwrap_or_else(|| "https://api.moonshot.cn".into());
                let api_key = env::var("KIMI_API_KEY").ok();
                Ok(Self {
                    api_key,
                    base_url,
                    auth_style: AuthStyle::Header {
                        name: "Authorization".into(),
                        prefix: "Bearer ".into(),
                    },
                })
            }
            "openrouter" => {
                let base_url = env::var("OPENROUTER_BASE_URL")
                    .ok()
                    .or_else(|| urls.and_then(|u| u.openrouter_base_url.clone()))
                    .unwrap_or_else(|| "https://openrouter.ai/api".into());
                let api_key = env::var("OPENROUTER_API_KEY").ok();
                Ok(Self {
                    api_key,
                    base_url,
                    auth_style: AuthStyle::Header {
                        name: "Authorization".into(),
                        prefix: "Bearer ".into(),
                    },
                })
            }
            "huggingface" => {
                let base_url = env::var("HUGGINGFACE_BASE_URL")
                    .ok()
                    .or_else(|| urls.and_then(|u| u.huggingface_base_url.clone()))
                    .unwrap_or_else(|| "https://api-inference.huggingface.co".into());
                let api_key = env::var("HUGGINGFACE_API_KEY").ok();
                Ok(Self {
                    api_key,
                    base_url,
                    auth_style: AuthStyle::Header {
                        name: "Authorization".into(),
                        prefix: "Bearer ".into(),
                    },
                })
            }
            "zai" => {
                let base_url = env::var("ZAI_BASE_URL")
                    .ok()
                    .or_else(|| urls.and_then(|u| u.zai_base_url.clone()))
                    .unwrap_or_else(|| "https://api.z.ai".into());
                let api_key = env::var("ZAI_API_KEY").ok();
                Ok(Self {
                    api_key,
                    base_url,
                    auth_style: AuthStyle::Header {
                        name: "Authorization".into(),
                        prefix: "Bearer ".into(),
                    },
                })
            }
            "minimax" => {
                let base_url = env::var("MINIMAX_BASE_URL")
                    .ok()
                    .or_else(|| urls.and_then(|u| u.minimax_base_url.clone()))
                    .unwrap_or_else(|| "https://api.minimax.chat".into());
                let api_key = env::var("MINIMAX_API_KEY").ok();
                Ok(Self {
                    api_key,
                    base_url,
                    auth_style: AuthStyle::Header {
                        name: "Authorization".into(),
                        prefix: "Bearer ".into(),
                    },
                })
            }
            "ollama" => {
                let base_url = env::var("OLLAMA_BASE_URL")
                    .ok()
                    .or_else(|| urls.and_then(|u| u.ollama_base_url.clone()))
                    .unwrap_or_else(|| "http://localhost:11434".into());
                Ok(Self {
                    api_key: None,
                    base_url,
                    auth_style: AuthStyle::Header {
                        name: "Authorization".into(),
                        prefix: "Bearer ".into(),
                    },
                })
            }
            "responses" => {
                let base_url = env::var("CMDIFY_RESPONSES_URL")
                    .ok()
                    .or_else(|| urls.and_then(|u| u.responses_url.clone()))
                    .ok_or_else(|| {
                        Error::ConfigError(
                            "CMDIFY_RESPONSES_URL is required for the responses provider".into(),
                        )
                    })?;
                let api_key = env::var("CMDIFY_RESPONSES_KEY").ok();
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
#[allow(dead_code)]
pub struct Config {
    pub provider_name: String,
    pub model_name: String,
    pub max_tokens: u32,
    pub system_prompt_override: Option<String>,
    pub spinner: u8,
    pub allow_unsafe: bool,
    pub quiet: bool,
    pub blind: bool,
    pub no_tools: bool,
    pub yolo: bool,
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
        env::remove_var("CMDIFY_SYSTEM_PROMPT_FILE");
        env::remove_var("CMDIFY_SPINNER");
        env::remove_var("CMDIFY_UNSAFE");
        env::remove_var("CMDIFY_QUIET");
        env::remove_var("CMDIFY_BLIND");
        env::remove_var("CMDIFY_NO_TOOLS");
        env::remove_var("CMDIFY_YOLO");
        env::remove_var("CMDIFY_CONFIG");
        env::set_var("XDG_CONFIG_HOME", "/nonexistent-cmdify-test-config");
        env::set_var("HOME", "/nonexistent-cmdify-test-home");
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
            let result = Config::from_env(None);
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
            let result = Config::from_env(None);
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
            env::remove_var("CMDIFY_SYSTEM_PROMPT_FILE");
            let config = Config::from_env(None).unwrap();
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
            env::remove_var("CMDIFY_SYSTEM_PROMPT_FILE");
            let config = Config::from_env(None).unwrap();
            assert!(config.provider_settings.api_key.is_none());
        });
    }

    #[test]
    fn missing_completions_url() {
        with_env_lock(|| {
            env::set_var("CMDIFY_PROVIDER_NAME", "completions");
            env::set_var("CMDIFY_MODEL_NAME", "llama3");
            env::remove_var("CMDIFY_COMPLETIONS_URL");
            let result = Config::from_env(None);
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
            env::remove_var("CMDIFY_SYSTEM_PROMPT_FILE");
            let config = Config::from_env(None).unwrap();
            assert_eq!(config.max_tokens, 1024);
        });
    }

    #[test]
    fn unknown_provider() {
        with_env_lock(|| {
            env::set_var("CMDIFY_PROVIDER_NAME", "fake_provider");
            env::set_var("CMDIFY_MODEL_NAME", "model");
            let result = Config::from_env(None);
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

            let config = Config::from_env(None).unwrap();
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

            let config = Config::from_env(None).unwrap();
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

            let config = Config::from_env(None).unwrap();
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
                system_prompt_file = "/my/prompt.txt"
                "#,
            );
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

            let config = Config::from_env(None).unwrap();
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
                system_prompt_file = "/from-file.txt"
                "#,
            );
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");
            env::set_var("CMDIFY_SYSTEM_PROMPT_FILE", "/from-env.txt");

            let config = Config::from_env(None).unwrap();
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

            let config = Config::from_env(None).unwrap();
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

            let config = Config::from_env(None).unwrap();
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

            let config = Config::from_env(None).unwrap();
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

            let result = Config::from_env(None);
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(err.contains("failed to parse"));
        });
    }

    #[test]
    fn defaults_all_false() {
        with_env_lock(|| {
            setup_completions_env();
            let config = Config::from_env(None).unwrap();
            assert_eq!(config.spinner, 1);
            assert!(!config.allow_unsafe);
            assert!(!config.quiet);
            assert!(!config.blind);
            assert!(!config.no_tools);
            assert!(!config.yolo);
        });
    }

    #[test]
    fn spinner_from_env() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_SPINNER", "3");
            let config = Config::from_env(None).unwrap();
            assert_eq!(config.spinner, 3);
        });
    }

    #[test]
    fn spinner_from_config_file() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            write_toml_config(
                tmp.path(),
                r#"
                provider_name = "completions"
                model_name = "llama3"
                spinner = 2
                "#,
            );
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");
            env::remove_var("CMDIFY_SPINNER");

            let config = Config::from_env(None).unwrap();
            assert_eq!(config.spinner, 2);
        });
    }

    #[test]
    fn spinner_env_overrides_config() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            write_toml_config(
                tmp.path(),
                r#"
                provider_name = "completions"
                model_name = "llama3"
                spinner = 2
                "#,
            );
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");
            env::set_var("CMDIFY_SPINNER", "3");

            let config = Config::from_env(None).unwrap();
            assert_eq!(config.spinner, 3);
        });
    }

    #[test]
    fn bool_flags_from_env() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_QUIET", "1");
            env::set_var("CMDIFY_BLIND", "true");
            env::set_var("CMDIFY_NO_TOOLS", "yes");
            env::set_var("CMDIFY_YOLO", "1");
            env::set_var("CMDIFY_UNSAFE", "true");
            let config = Config::from_env(None).unwrap();
            assert!(config.quiet);
            assert!(config.blind);
            assert!(config.no_tools);
            assert!(config.yolo);
            assert!(config.allow_unsafe);
        });
    }

    #[test]
    fn bool_flags_false_from_env() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_QUIET", "0");
            env::set_var("CMDIFY_BLIND", "false");
            env::set_var("CMDIFY_YOLO", "no");
            let config = Config::from_env(None).unwrap();
            assert!(!config.quiet);
            assert!(!config.blind);
            assert!(!config.yolo);
        });
    }

    #[test]
    fn bool_flags_from_config_file() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            write_toml_config(
                tmp.path(),
                r#"
                provider_name = "completions"
                model_name = "llama3"
                quiet = true
                yolo = true
                "#,
            );
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

            let config = Config::from_env(None).unwrap();
            assert!(config.quiet);
            assert!(config.yolo);
            assert!(!config.blind);
            assert!(!config.no_tools);
            assert!(!config.allow_unsafe);
        });
    }

    #[test]
    fn bool_flags_env_overrides_config() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            write_toml_config(
                tmp.path(),
                r#"
                provider_name = "completions"
                model_name = "llama3"
                yolo = false
                "#,
            );
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");
            env::set_var("CMDIFY_YOLO", "1");

            let config = Config::from_env(None).unwrap();
            assert!(config.yolo);
        });
    }

    #[test]
    fn invalid_bool_env_uses_default() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_YOLO", "maybe");
            let config = Config::from_env(None).unwrap();
            assert!(!config.yolo);
        });
    }

    #[test]
    fn invalid_spinner_env_uses_default() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_SPINNER", "abc");
            let config = Config::from_env(None).unwrap();
            assert_eq!(config.spinner, 1);
        });
    }

    #[test]
    fn all_flags_from_config_file() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            write_toml_config(
                tmp.path(),
                r#"
                provider_name = "completions"
                model_name = "llama3"
                spinner = 3
                allow_unsafe = true
                quiet = true
                blind = true
                no_tools = true
                yolo = true
                "#,
            );
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

            let config = Config::from_env(None).unwrap();
            assert_eq!(config.spinner, 3);
            assert!(config.allow_unsafe);
            assert!(config.quiet);
            assert!(config.blind);
            assert!(config.no_tools);
            assert!(config.yolo);
        });
    }

    #[test]
    fn explicit_config_path_used() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            let config_path = tmp.path().join("my-cmdify-config.toml");
            fs::write(
                &config_path,
                r#"
                provider_name = "completions"
                model_name = "explicit-model"
                "#,
            )
            .unwrap();
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

            let config = Config::from_env(Some(&config_path)).unwrap();
            assert_eq!(config.model_name, "explicit-model");
        });
    }

    #[test]
    fn explicit_config_path_overrides_xdg() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            let xdg_path = tmp.path().join("cmdify").join("config.toml");
            fs::create_dir_all(xdg_path.parent().unwrap()).unwrap();
            fs::write(
                &xdg_path,
                r#"
                provider_name = "completions"
                model_name = "xdg-model"
                "#,
            )
            .unwrap();
            let explicit_path = tmp.path().join("other.toml");
            fs::write(
                &explicit_path,
                r#"
                provider_name = "completions"
                model_name = "explicit-model"
                "#,
            )
            .unwrap();
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

            let config = Config::from_env(Some(&explicit_path)).unwrap();
            assert_eq!(config.model_name, "explicit-model");
        });
    }

    #[test]
    fn explicit_config_path_not_found_errors() {
        with_env_lock(|| {
            cleanup_vars();
            let missing = PathBuf::from("/nonexistent/cmdify-config.toml");
            let result = Config::from_env(Some(&missing));
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(err.contains("config file not found"));
        });
    }

    #[test]
    fn cmdify_config_env_var_used() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            let config_path = tmp.path().join("env-config.toml");
            fs::write(
                &config_path,
                r#"
                provider_name = "completions"
                model_name = "env-path-model"
                "#,
            )
            .unwrap();
            env::set_var("CMDIFY_CONFIG", &config_path);
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

            let config = Config::from_env(None).unwrap();
            assert_eq!(config.model_name, "env-path-model");
        });
    }

    #[test]
    fn cli_config_overrides_env_config() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            let env_path = tmp.path().join("env-config.toml");
            fs::write(
                &env_path,
                r#"
                provider_name = "completions"
                model_name = "env-path-model"
                "#,
            )
            .unwrap();
            let cli_path = tmp.path().join("cli-config.toml");
            fs::write(
                &cli_path,
                r#"
                provider_name = "completions"
                model_name = "cli-path-model"
                "#,
            )
            .unwrap();
            env::set_var("CMDIFY_CONFIG", &env_path);
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

            let config = Config::from_env(Some(&cli_path)).unwrap();
            assert_eq!(config.model_name, "cli-path-model");
        });
    }

    #[test]
    fn cmdify_config_env_not_found_errors() {
        with_env_lock(|| {
            cleanup_vars();
            env::set_var("CMDIFY_CONFIG", "/nonexistent/cmdify-config.toml");
            let result = Config::from_env(None);
            assert!(result.is_err());
            let err = result.unwrap_err().to_string();
            assert!(err.contains("config file not found"));
        });
    }
}

use std::env;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::{Error, Result};

const DEFAULT_MAX_TOKENS: u32 = 16384;

#[derive(Debug, Clone)]
pub struct ConfigSource {
    pub key: String,
    pub value: String,
    pub source: String,
}

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

fn parse_debug_env(var: &str) -> Option<u8> {
    match std::env::var(var).ok()?.to_lowercase().as_str() {
        "0" | "false" | "no" => Some(0),
        "1" | "true" | "yes" => Some(1),
        "2" => Some(2),
        _ => None,
    }
}

fn parse_u8_env(var: &str) -> Option<u8> {
    std::env::var(var).ok()?.parse().ok()
}

fn resolve_string(
    env_var: &str,
    file_value: Option<String>,
    sources: &mut Vec<ConfigSource>,
    key: &str,
) -> Option<String> {
    match env::var(env_var).ok() {
        Some(v) => {
            sources.push(ConfigSource {
                key: key.into(),
                value: v.clone(),
                source: "env".into(),
            });
            Some(v)
        }
        None => match file_value {
            Some(v) => {
                sources.push(ConfigSource {
                    key: key.into(),
                    value: v.clone(),
                    source: "file".into(),
                });
                Some(v)
            }
            None => None,
        },
    }
}

fn resolve_bool(
    env_var: &str,
    file_value: Option<bool>,
    sources: &mut Vec<ConfigSource>,
    key: &str,
) -> Option<bool> {
    match parse_bool_env(env_var) {
        Some(v) => {
            sources.push(ConfigSource {
                key: key.into(),
                value: v.to_string(),
                source: "env".into(),
            });
            Some(v)
        }
        None => match file_value {
            Some(v) => {
                sources.push(ConfigSource {
                    key: key.into(),
                    value: v.to_string(),
                    source: "file".into(),
                });
                Some(v)
            }
            None => None,
        },
    }
}

fn resolve_u8(
    env_var: &str,
    file_value: Option<u8>,
    sources: &mut Vec<ConfigSource>,
    key: &str,
) -> Option<u8> {
    match parse_u8_env(env_var) {
        Some(v) => {
            sources.push(ConfigSource {
                key: key.into(),
                value: v.to_string(),
                source: "env".into(),
            });
            Some(v)
        }
        None => match file_value {
            Some(v) => {
                sources.push(ConfigSource {
                    key: key.into(),
                    value: v.to_string(),
                    source: "file".into(),
                });
                Some(v)
            }
            None => None,
        },
    }
}

fn resolve_optional_url(
    env_var: &str,
    file_url: Option<String>,
    sources: &mut Vec<ConfigSource>,
) -> Option<String> {
    match env::var(env_var).ok() {
        Some(v) => {
            sources.push(ConfigSource {
                key: "base_url".into(),
                value: v.clone(),
                source: "env".into(),
            });
            Some(v)
        }
        None => match file_url {
            Some(v) => {
                sources.push(ConfigSource {
                    key: "base_url".into(),
                    value: v.clone(),
                    source: "file".into(),
                });
                Some(v)
            }
            None => None,
        },
    }
}

fn resolve_required_url(
    env_var: &str,
    file_url: Option<String>,
    sources: &mut Vec<ConfigSource>,
) -> Result<String> {
    match env::var(env_var).ok() {
        Some(v) => {
            sources.push(ConfigSource {
                key: "base_url".into(),
                value: v.clone(),
                source: "env".into(),
            });
            Ok(v)
        }
        None => match file_url {
            Some(v) => {
                sources.push(ConfigSource {
                    key: "base_url".into(),
                    value: v.clone(),
                    source: "file".into(),
                });
                Ok(v)
            }
            None => Err(Error::ConfigError(format!("{} is required", env_var))),
        },
    }
}

fn record_api_key(env_var: &str, sources: &mut Vec<ConfigSource>) {
    if env::var(env_var).is_ok() {
        sources.push(ConfigSource {
            key: "api_key".into(),
            value: "***".into(),
            source: "env".into(),
        });
    }
}

fn resolve_provider_settings(
    key_var: &str,
    url_env_var: &str,
    file_url: Option<String>,
    default_url: Option<&str>,
    auth_style: AuthStyle,
    sources: &mut Vec<ConfigSource>,
    no_key: bool,
) -> Result<ProviderSettings> {
    let base_url = if let Some(default) = default_url {
        resolve_optional_url(url_env_var, file_url, sources).unwrap_or_else(|| default.into())
    } else {
        resolve_required_url(url_env_var, file_url, sources)?
    };

    let api_key = if no_key { None } else { env::var(key_var).ok() };
    if !no_key {
        record_api_key(key_var, sources);
    }

    Ok(ProviderSettings {
        api_key,
        base_url,
        auth_style,
    })
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
    debug: Option<bool>,
    tool_level: Option<u8>,
    #[serde(default)]
    providers: ProviderUrls,
}

impl Config {
    pub fn from_env(explicit_config_path: Option<&Path>) -> Result<(Self, Vec<ConfigSource>)> {
        let mut sources: Vec<ConfigSource> = Vec::new();

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

        let provider_name = resolve_string(
            "CMDIFY_PROVIDER_NAME",
            file_config.as_ref().and_then(|f| f.provider_name.clone()),
            &mut sources,
            "provider_name",
        )
        .ok_or_else(|| {
            Error::ConfigError(
                "CMDIFY_PROVIDER_NAME is required (set env var or provider_name in config file)"
                    .into(),
            )
        })?;

        let model_name = resolve_string(
            "CMDIFY_MODEL_NAME",
            file_config.as_ref().and_then(|f| f.model_name.clone()),
            &mut sources,
            "model_name",
        )
        .ok_or_else(|| {
            Error::ConfigError(
                "CMDIFY_MODEL_NAME is required (set env var or model_name in config file)".into(),
            )
        })?;

        let max_tokens = env::var("CMDIFY_MAX_TOKENS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .inspect(|v| {
                sources.push(ConfigSource {
                    key: "max_tokens".into(),
                    value: v.to_string(),
                    source: "env".into(),
                });
            })
            .or(file_config.as_ref().and_then(|f| {
                f.max_tokens.inspect(|v| {
                    sources.push(ConfigSource {
                        key: "max_tokens".into(),
                        value: v.to_string(),
                        source: "file".into(),
                    });
                })
            }))
            .unwrap_or(DEFAULT_MAX_TOKENS);

        let system_prompt_override = resolve_string(
            "CMDIFY_SYSTEM_PROMPT_FILE",
            file_config
                .as_ref()
                .and_then(|f| f.system_prompt_file.clone()),
            &mut sources,
            "system_prompt_file",
        );

        let spinner = resolve_u8(
            "CMDIFY_SPINNER",
            file_config.as_ref().and_then(|f| f.spinner),
            &mut sources,
            "spinner",
        )
        .unwrap_or(1);

        let allow_unsafe = resolve_bool(
            "CMDIFY_UNSAFE",
            file_config.as_ref().and_then(|f| f.allow_unsafe),
            &mut sources,
            "allow_unsafe",
        )
        .unwrap_or(false);

        let quiet = resolve_bool(
            "CMDIFY_QUIET",
            file_config.as_ref().and_then(|f| f.quiet),
            &mut sources,
            "quiet",
        )
        .unwrap_or(false);

        let blind = resolve_bool(
            "CMDIFY_BLIND",
            file_config.as_ref().and_then(|f| f.blind),
            &mut sources,
            "blind",
        )
        .unwrap_or(false);

        let no_tools = resolve_bool(
            "CMDIFY_NO_TOOLS",
            file_config.as_ref().and_then(|f| f.no_tools),
            &mut sources,
            "no_tools",
        )
        .unwrap_or(false);

        let tool_level = resolve_u8(
            "CMDIFY_TOOL_LEVEL",
            file_config.as_ref().and_then(|f| f.tool_level),
            &mut sources,
            "tool_level",
        )
        .unwrap_or(1)
        .min(3);

        let yolo = resolve_bool(
            "CMDIFY_YOLO",
            file_config.as_ref().and_then(|f| f.yolo),
            &mut sources,
            "yolo",
        )
        .unwrap_or(false);

        let debug_level = match parse_debug_env("CMDIFY_DEBUG") {
            Some(v) => {
                if v > 0 {
                    sources.push(ConfigSource {
                        key: "debug".into(),
                        value: v.to_string(),
                        source: "env".into(),
                    });
                }
                v
            }
            None => match file_config.as_ref().and_then(|f| f.debug) {
                Some(true) => {
                    sources.push(ConfigSource {
                        key: "debug".into(),
                        value: "1".into(),
                        source: "file".into(),
                    });
                    1
                }
                _ => 0,
            },
        };

        let provider_settings =
            ProviderSettings::from_env(&provider_name, file_config.as_ref(), &mut sources)?;

        Ok((
            Self {
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
                debug_level,
                tool_level,
                provider_settings,
            },
            sources,
        ))
    }
}

impl ProviderSettings {
    #[allow(private_interfaces)]
    pub fn from_env(
        provider_name: &str,
        file_config: Option<&FileConfig>,
        sources: &mut Vec<ConfigSource>,
    ) -> Result<Self> {
        let urls = file_config.map(|f| &f.providers);

        let bearer = AuthStyle::Header {
            name: "Authorization".into(),
            prefix: "Bearer ".into(),
        };

        match provider_name {
            "completions" => resolve_provider_settings(
                "CMDIFY_COMPLETIONS_KEY",
                "CMDIFY_COMPLETIONS_URL",
                urls.and_then(|u| u.completions_url.clone()),
                None,
                bearer,
                sources,
                false,
            ),
            "openai" => resolve_provider_settings(
                "OPENAI_API_KEY",
                "OPENAI_BASE_URL",
                urls.and_then(|u| u.openai_base_url.clone()),
                Some("https://api.openai.com"),
                bearer,
                sources,
                false,
            ),
            "anthropic" => resolve_provider_settings(
                "ANTHROPIC_API_KEY",
                "ANTHROPIC_BASE_URL",
                urls.and_then(|u| u.anthropic_base_url.clone()),
                Some("https://api.anthropic.com"),
                AuthStyle::Header {
                    name: "x-api-key".into(),
                    prefix: String::new(),
                },
                sources,
                false,
            ),
            "gemini" => resolve_provider_settings(
                "GEMINI_API_KEY",
                "GEMINI_BASE_URL",
                urls.and_then(|u| u.gemini_base_url.clone()),
                Some("https://generativelanguage.googleapis.com"),
                AuthStyle::QueryParam { name: "key".into() },
                sources,
                false,
            ),
            "mistral" => resolve_provider_settings(
                "MISTRAL_API_KEY",
                "MISTRAL_BASE_URL",
                urls.and_then(|u| u.mistral_base_url.clone()),
                Some("https://api.mistral.ai"),
                bearer,
                sources,
                false,
            ),
            "qwen" => resolve_provider_settings(
                "QWEN_API_KEY",
                "QWEN_BASE_URL",
                urls.and_then(|u| u.qwen_base_url.clone()),
                Some("https://dashscope.aliyuncs.com/compatible-mode"),
                bearer,
                sources,
                false,
            ),
            "kimi" => resolve_provider_settings(
                "KIMI_API_KEY",
                "KIMI_BASE_URL",
                urls.and_then(|u| u.kimi_base_url.clone()),
                Some("https://api.moonshot.cn"),
                bearer,
                sources,
                false,
            ),
            "openrouter" => resolve_provider_settings(
                "OPENROUTER_API_KEY",
                "OPENROUTER_BASE_URL",
                urls.and_then(|u| u.openrouter_base_url.clone()),
                Some("https://openrouter.ai/api"),
                bearer,
                sources,
                false,
            ),
            "huggingface" => resolve_provider_settings(
                "HUGGINGFACE_API_KEY",
                "HUGGINGFACE_BASE_URL",
                urls.and_then(|u| u.huggingface_base_url.clone()),
                Some("https://api-inference.huggingface.co"),
                bearer,
                sources,
                false,
            ),
            "zai" => resolve_provider_settings(
                "ZAI_API_KEY",
                "ZAI_BASE_URL",
                urls.and_then(|u| u.zai_base_url.clone()),
                Some("https://api.z.ai"),
                bearer,
                sources,
                false,
            ),
            "minimax" => resolve_provider_settings(
                "MINIMAX_API_KEY",
                "MINIMAX_BASE_URL",
                urls.and_then(|u| u.minimax_base_url.clone()),
                Some("https://api.minimax.chat"),
                bearer,
                sources,
                false,
            ),
            "ollama" => resolve_provider_settings(
                "",
                "OLLAMA_BASE_URL",
                urls.and_then(|u| u.ollama_base_url.clone()),
                Some("http://localhost:11434"),
                bearer,
                sources,
                true,
            ),
            "responses" => resolve_provider_settings(
                "CMDIFY_RESPONSES_KEY",
                "CMDIFY_RESPONSES_URL",
                urls.and_then(|u| u.responses_url.clone()),
                None,
                bearer,
                sources,
                false,
            ),
            other => Err(Error::ConfigError(format!("unknown provider: {}", other))),
        }
    }
}

// TODO(Phase 4-6): Remove #[allow(dead_code)] once provider implementations
// read auth_style to set headers/query params. Currently only the completions
// provider exists and hardcodes "Authorization: Bearer" (see provider/completions.rs).
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum AuthStyle {
    Header { name: String, prefix: String },
    QueryParam { name: String },
}

#[derive(Debug, Clone)]
pub struct ProviderSettings {
    pub api_key: Option<String>,
    pub base_url: String,
    #[allow(dead_code)]
    pub auth_style: AuthStyle,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub provider_name: String,
    pub model_name: String,
    pub max_tokens: u32,
    pub system_prompt_override: Option<String>,
    pub spinner: u8,
    #[allow(dead_code)]
    pub allow_unsafe: bool,
    pub quiet: bool,
    pub blind: bool,
    pub no_tools: bool,
    pub yolo: bool,
    pub debug_level: u8,
    pub tool_level: u8,
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
        env::remove_var("CMDIFY_TOOL_LEVEL");
        env::remove_var("CMDIFY_DEBUG");
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
            let (config, _sources) = Config::from_env(None).unwrap();
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
            let (config, _sources) = Config::from_env(None).unwrap();
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
            let (config, _sources) = Config::from_env(None).unwrap();
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

            let (config, _sources) = Config::from_env(None).unwrap();
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

            let (config, _sources) = Config::from_env(None).unwrap();
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

            let (config, _sources) = Config::from_env(None).unwrap();
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

            let (config, _sources) = Config::from_env(None).unwrap();
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

            let (config, _sources) = Config::from_env(None).unwrap();
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

            let (config, _sources) = Config::from_env(None).unwrap();
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

            let (config, _sources) = Config::from_env(None).unwrap();
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

            let (config, _sources) = Config::from_env(None).unwrap();
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
            let (config, _sources) = Config::from_env(None).unwrap();
            assert_eq!(config.spinner, 1);
            assert!(!config.allow_unsafe);
            assert!(!config.quiet);
            assert!(!config.blind);
            assert!(!config.no_tools);
            assert!(!config.yolo);
            assert_eq!(config.debug_level, 0);
            assert_eq!(config.tool_level, 1);
        });
    }

    #[test]
    fn spinner_from_env() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_SPINNER", "3");
            let (config, _sources) = Config::from_env(None).unwrap();
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

            let (config, _sources) = Config::from_env(None).unwrap();
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

            let (config, _sources) = Config::from_env(None).unwrap();
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
            let (config, _sources) = Config::from_env(None).unwrap();
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
            let (config, _sources) = Config::from_env(None).unwrap();
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

            let (config, _sources) = Config::from_env(None).unwrap();
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

            let (config, _sources) = Config::from_env(None).unwrap();
            assert!(config.yolo);
        });
    }

    #[test]
    fn invalid_bool_env_uses_default() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_YOLO", "maybe");
            let (config, _sources) = Config::from_env(None).unwrap();
            assert!(!config.yolo);
        });
    }

    #[test]
    fn invalid_spinner_env_uses_default() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_SPINNER", "abc");
            let (config, _sources) = Config::from_env(None).unwrap();
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

            let (config, _sources) = Config::from_env(None).unwrap();
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

            let (config, _sources) = Config::from_env(Some(&config_path)).unwrap();
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

            let (config, _sources) = Config::from_env(Some(&explicit_path)).unwrap();
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

            let (config, _sources) = Config::from_env(None).unwrap();
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

            let (config, _sources) = Config::from_env(Some(&cli_path)).unwrap();
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

    #[test]
    fn debug_from_env() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_DEBUG", "1");
            let (config, _sources) = Config::from_env(None).unwrap();
            assert_eq!(config.debug_level, 1);
        });
    }

    #[test]
    fn debug_from_env_false() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_DEBUG", "0");
            let (config, _sources) = Config::from_env(None).unwrap();
            assert_eq!(config.debug_level, 0);
        });
    }

    #[test]
    fn debug_from_config_file() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            write_toml_config(
                tmp.path(),
                r#"
                provider_name = "completions"
                model_name = "llama3"
                debug = true
                "#,
            );
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

            let (config, _sources) = Config::from_env(None).unwrap();
            assert_eq!(config.debug_level, 1);
        });
    }

    #[test]
    fn debug_env_overrides_config_file() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            write_toml_config(
                tmp.path(),
                r#"
                provider_name = "completions"
                model_name = "llama3"
                debug = true
                "#,
            );
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");
            env::set_var("CMDIFY_DEBUG", "false");

            let (config, _sources) = Config::from_env(None).unwrap();
            assert_eq!(config.debug_level, 0);
        });
    }

    #[test]
    fn debug_default_false() {
        with_env_lock(|| {
            setup_completions_env();
            env::remove_var("CMDIFY_DEBUG");
            let (config, _sources) = Config::from_env(None).unwrap();
            assert_eq!(config.debug_level, 0);
        });
    }

    #[test]
    fn sources_track_env_overrides() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            write_toml_config(
                tmp.path(),
                r#"
                provider_name = "completions"
                model_name = "file-model"
                max_tokens = 2048
                "#,
            );
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_PROVIDER_NAME", "completions");
            env::set_var("CMDIFY_MODEL_NAME", "env-model");
            env::set_var("CMDIFY_MAX_TOKENS", "8192");
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

            let (_config, sources) = Config::from_env(None).unwrap();
            let model_src = sources.iter().find(|s| s.key == "model_name").unwrap();
            assert_eq!(model_src.source, "env");
            assert_eq!(model_src.value, "env-model");

            let tokens_src = sources.iter().find(|s| s.key == "max_tokens").unwrap();
            assert_eq!(tokens_src.source, "env");
            assert_eq!(tokens_src.value, "8192");
        });
    }

    #[test]
    fn sources_track_file_origin() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            write_toml_config(
                tmp.path(),
                r#"
                provider_name = "completions"
                model_name = "file-model"
                "#,
            );
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

            let (_config, sources) = Config::from_env(None).unwrap();
            let model_src = sources.iter().find(|s| s.key == "model_name").unwrap();
            assert_eq!(model_src.source, "file");
            assert_eq!(model_src.value, "file-model");

            let provider_src = sources.iter().find(|s| s.key == "provider_name").unwrap();
            assert_eq!(provider_src.source, "file");

            let url_src = sources.iter().find(|s| s.key == "base_url").unwrap();
            assert_eq!(url_src.source, "env");
        });
    }

    #[test]
    fn sources_omit_defaults() {
        with_env_lock(|| {
            setup_completions_env();
            let (_config, sources) = Config::from_env(None).unwrap();
            assert!(!sources.iter().any(|s| s.key == "max_tokens"));
            assert!(!sources.iter().any(|s| s.key == "quiet"));
            assert!(!sources.iter().any(|s| s.key == "blind"));
            assert!(!sources.iter().any(|s| s.key == "yolo"));
            assert!(!sources.iter().any(|s| s.key == "no_tools"));
            assert!(!sources.iter().any(|s| s.key == "allow_unsafe"));
            assert!(!sources.iter().any(|s| s.key == "debug"));
        });
    }

    #[test]
    fn sources_mask_api_key() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_COMPLETIONS_KEY", "super-secret-key");
            let (_config, sources) = Config::from_env(None).unwrap();
            let key_src = sources.iter().find(|s| s.key == "api_key").unwrap();
            assert_eq!(key_src.value, "***");
            assert_eq!(key_src.source, "env");
        });
    }

    #[test]
    fn debug_from_env_level_2() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_DEBUG", "2");
            let (config, sources) = Config::from_env(None).unwrap();
            assert_eq!(config.debug_level, 2);
            let debug_src = sources.iter().find(|s| s.key == "debug").unwrap();
            assert_eq!(debug_src.value, "2");
            assert_eq!(debug_src.source, "env");
        });
    }

    #[test]
    fn debug_from_env_true_maps_to_level_1() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_DEBUG", "true");
            let (config, sources) = Config::from_env(None).unwrap();
            assert_eq!(config.debug_level, 1);
            let debug_src = sources.iter().find(|s| s.key == "debug").unwrap();
            assert_eq!(debug_src.value, "1");
            assert_eq!(debug_src.source, "env");
        });
    }

    #[test]
    fn debug_source_omitted_when_level_0() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_DEBUG", "false");
            let (_config, sources) = Config::from_env(None).unwrap();
            assert!(!sources.iter().any(|s| s.key == "debug"));
        });
    }

    #[test]
    fn debug_config_file_maps_true_to_level_1_source() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            write_toml_config(
                tmp.path(),
                r#"
                provider_name = "completions"
                model_name = "llama3"
                debug = true
                "#,
            );
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

            let (_config, sources) = Config::from_env(None).unwrap();
            let debug_src = sources.iter().find(|s| s.key == "debug").unwrap();
            assert_eq!(debug_src.value, "1");
            assert_eq!(debug_src.source, "file");
        });
    }

    #[test]
    fn debug_precedence_cli_max_with_env() {
        let env_level: u8 = 1;
        let cli_level: u8 = 2;
        assert_eq!(std::cmp::max(env_level, cli_level), 2);

        let env_level: u8 = 2;
        let cli_level: u8 = 0;
        assert_eq!(std::cmp::max(env_level, cli_level), 2);

        let env_level: u8 = 0;
        let cli_level: u8 = 1;
        assert_eq!(std::cmp::max(env_level, cli_level), 1);

        let env_level: u8 = 2;
        let cli_level: u8 = 2;
        assert_eq!(std::cmp::max(env_level, cli_level), 2);
    }

    #[test]
    fn invalid_debug_env_uses_default() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_DEBUG", "3");
            let (config, _sources) = Config::from_env(None).unwrap();
            assert_eq!(config.debug_level, 0);
        });
    }

    #[test]
    fn tool_level_from_env() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_TOOL_LEVEL", "2");
            let (config, sources) = Config::from_env(None).unwrap();
            assert_eq!(config.tool_level, 2);
            let src = sources.iter().find(|s| s.key == "tool_level").unwrap();
            assert_eq!(src.value, "2");
            assert_eq!(src.source, "env");
        });
    }

    #[test]
    fn tool_level_from_config_file() {
        with_env_lock(|| {
            cleanup_vars();
            let tmp = tempfile::tempdir().unwrap();
            write_toml_config(
                tmp.path(),
                r#"
                provider_name = "completions"
                model_name = "llama3"
                tool_level = 3
                "#,
            );
            env::set_var("XDG_CONFIG_HOME", tmp.path());
            env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

            let (config, sources) = Config::from_env(None).unwrap();
            assert_eq!(config.tool_level, 3);
            let src = sources.iter().find(|s| s.key == "tool_level").unwrap();
            assert_eq!(src.source, "file");
        });
    }

    #[test]
    fn tool_level_cli_overrides_env() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_TOOL_LEVEL", "2");
            let (mut config, _sources) = Config::from_env(None).unwrap();
            assert_eq!(config.tool_level, 2);
            config.tool_level = Some(3).unwrap_or(config.tool_level);
            assert_eq!(config.tool_level, 3);
        });
    }

    #[test]
    fn tool_level_invalid_env_uses_default() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_TOOL_LEVEL", "abc");
            let (config, _sources) = Config::from_env(None).unwrap();
            assert_eq!(config.tool_level, 1);
        });
    }

    #[test]
    fn tool_level_clamped_to_3() {
        with_env_lock(|| {
            setup_completions_env();
            env::set_var("CMDIFY_TOOL_LEVEL", "99");
            let (config, _sources) = Config::from_env(None).unwrap();
            assert_eq!(config.tool_level, 3);
        });
    }

    #[test]
    fn tool_level_default_is_1() {
        with_env_lock(|| {
            setup_completions_env();
            env::remove_var("CMDIFY_TOOL_LEVEL");
            let (config, sources) = Config::from_env(None).unwrap();
            assert_eq!(config.tool_level, 1);
            assert!(!sources.iter().any(|s| s.key == "tool_level"));
        });
    }
}

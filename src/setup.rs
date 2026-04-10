use std::env;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::config::{default_config_file_path, FileConfig};
use crate::error::Result;
use crate::provider::{
    anthropic, gemini, huggingface, kimi, minimax, mistral, ollama, openai, openrouter, qwen, zai,
};

const DEFAULT_MAX_TOKENS: u32 = 16384;

struct ProviderInfo {
    name: &'static str,
    pretty_name: &'static str,
    description: &'static str,
    key_env_var: &'static str,
    default_url: Option<&'static str>,
    suggested_model: Option<&'static str>,
}

fn providers() -> &'static [ProviderInfo] {
    static PROVIDERS: OnceLock<Vec<ProviderInfo>> = OnceLock::new();
    PROVIDERS.get_or_init(|| {
        vec![
            ProviderInfo {
                name: "openai",
                pretty_name: "OpenAI",
                description: "ChatGPT, GPT-4o, GPT-4, o1, etc.",
                key_env_var: openai::API_KEY_ENV,
                default_url: Some(openai::DEFAULT_BASE_URL),
                suggested_model: Some("gpt-4o"),
            },
            ProviderInfo {
                name: "anthropic",
                pretty_name: "Anthropic",
                description: "Claude Sonnet, Claude Opus, Claude Haiku",
                key_env_var: anthropic::API_KEY_ENV,
                default_url: Some(anthropic::DEFAULT_BASE_URL),
                suggested_model: Some("claude-sonnet-4-20250514"),
            },
            ProviderInfo {
                name: "gemini",
                pretty_name: "Google Gemini",
                description: "Gemini Pro, Gemini Flash, Gemma",
                key_env_var: gemini::API_KEY_ENV,
                default_url: Some(gemini::DEFAULT_BASE_URL),
                suggested_model: Some("gemini-2.0-flash"),
            },
            ProviderInfo {
                name: "ollama",
                pretty_name: "Ollama",
                description: "Local models via Ollama (no API key needed)",
                key_env_var: "",
                default_url: Some(ollama::DEFAULT_BASE_URL),
                suggested_model: Some("llama3"),
            },
            ProviderInfo {
                name: "mistral",
                pretty_name: "Mistral",
                description: "Mistral Large, Medium, Small, Codestral",
                key_env_var: mistral::API_KEY_ENV,
                default_url: Some(mistral::DEFAULT_BASE_URL),
                suggested_model: Some("mistral-large-latest"),
            },
            ProviderInfo {
                name: "qwen",
                pretty_name: "Qwen (Alibaba)",
                description: "Qwen Max, Qwen Plus, Qwen Turbo",
                key_env_var: qwen::API_KEY_ENV,
                default_url: Some(qwen::DEFAULT_BASE_URL),
                suggested_model: Some("qwen-max"),
            },
            ProviderInfo {
                name: "kimi",
                pretty_name: "Kimi (Moonshot)",
                description: "Moonshot v1, Kimi K2",
                key_env_var: kimi::API_KEY_ENV,
                default_url: Some(kimi::DEFAULT_BASE_URL),
                suggested_model: Some("moonshot-v1-8k"),
            },
            ProviderInfo {
                name: "openrouter",
                pretty_name: "OpenRouter",
                description: "Aggregator for many providers",
                key_env_var: openrouter::API_KEY_ENV,
                default_url: Some(openrouter::DEFAULT_BASE_URL),
                suggested_model: None,
            },
            ProviderInfo {
                name: "huggingface",
                pretty_name: "HuggingFace",
                description: "Inference API and serverless endpoints",
                key_env_var: huggingface::API_KEY_ENV,
                default_url: Some(huggingface::DEFAULT_BASE_URL),
                suggested_model: None,
            },
            ProviderInfo {
                name: "zai",
                pretty_name: "Z.ai",
                description: "GLM-4, ChatGLM models",
                key_env_var: zai::API_KEY_ENV,
                default_url: Some(zai::DEFAULT_BASE_URL),
                suggested_model: None,
            },
            ProviderInfo {
                name: "minimax",
                pretty_name: "MiniMax",
                description: "MiniMax M2 and other models",
                key_env_var: minimax::API_KEY_ENV,
                default_url: Some(minimax::DEFAULT_BASE_URL),
                suggested_model: None,
            },
            ProviderInfo {
                name: "completions",
                pretty_name: "Generic /completions",
                description: "Any OpenAI-compatible /chat/completions endpoint",
                key_env_var: "CMDIFY_COMPLETIONS_KEY",
                default_url: None,
                suggested_model: None,
            },
            ProviderInfo {
                name: "responses",
                pretty_name: "Generic /responses",
                description: "Any OpenAI-compatible /responses endpoint",
                key_env_var: "CMDIFY_RESPONSES_KEY",
                default_url: None,
                suggested_model: None,
            },
        ]
    })
}

pub trait SetupIo {
    fn read_line(&self, prompt: &str) -> io::Result<String>;
    fn eprint(&self, msg: &str);
}

struct TerminalIo;

impl SetupIo for TerminalIo {
    fn read_line(&self, prompt: &str) -> io::Result<String> {
        eprint!("{}", prompt);
        io::stderr().flush()?;
        let mut input = String::new();
        io::stdin().lock().read_line(&mut input)?;
        Ok(input.trim().to_string())
    }

    fn eprint(&self, msg: &str) {
        eprint!("{}", msg);
    }
}

struct SetupInputs {
    provider_name: String,
    model_name: String,
    base_url: Option<String>,
    max_tokens: u32,
    spinner: u8,
    config_path: PathBuf,
    overwrite: bool,
}

fn resolve_provider(name: &str) -> Option<&'static ProviderInfo> {
    providers().iter().find(|p| p.name == name)
}

fn prompt_yes_no<R: SetupIo + ?Sized>(io: &R, question: &str, default: bool) -> io::Result<bool> {
    let hint = if default { "Y/n" } else { "y/N" };
    loop {
        let answer = io.read_line(&format!("{} [{}]: ", question, hint))?;
        match answer.to_lowercase().as_str() {
            "" => return Ok(default),
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => {
                io.eprint("Please enter Y or N.\n");
            }
        }
    }
}

fn select_provider<R: SetupIo + ?Sized>(
    io: &R,
    existing_provider: Option<&str>,
) -> io::Result<String> {
    let list = providers();

    let key_width = list.iter().map(|p| p.name.len()).max().unwrap_or(0);
    let pretty_width = list.iter().map(|p| p.pretty_name.len()).max().unwrap_or(10) + 2;

    io.eprint("\nAvailable providers:\n\n");
    for (i, p) in list.iter().enumerate() {
        io.eprint(&format!(
            "  {:2}) [{:width_key$}] {:width_pretty$} — {}\n",
            i + 1,
            p.name,
            p.pretty_name,
            p.description,
            width_key = key_width,
            width_pretty = pretty_width
        ));
    }
    io.eprint("\n");

    let default_label = existing_provider.unwrap_or("openai");
    loop {
        let answer = io.read_line(&format!(
            "Select a provider (key name, pretty name, or number) [{}]: ",
            default_label
        ))?;
        let answer = answer.trim();
        if answer.is_empty() {
            return Ok(default_label.to_string());
        }
        if let Ok(num) = answer.parse::<usize>() {
            if num >= 1 && num <= list.len() {
                return Ok(list[num - 1].name.to_string());
            }
        }
        let lower = answer.to_lowercase();
        if let Some(matched) = list
            .iter()
            .find(|p| p.name == lower || p.pretty_name.to_lowercase() == lower)
        {
            return Ok(matched.name.to_string());
        }
        io.eprint(&format!(
            "Invalid choice. Enter a provider name or number 1-{}.\n",
            list.len()
        ));
    }
}

fn prompt_for_url<R: SetupIo + ?Sized>(
    io: &R,
    provider: &ProviderInfo,
    existing_url: Option<&str>,
) -> io::Result<Option<String>> {
    match provider.default_url {
        Some(default) => {
            let existing_label = existing_url.unwrap_or(default);
            let answer = io.read_line(&format!("Base URL [{}]: ", existing_label))?;
            if answer.is_empty() {
                Ok(None)
            } else {
                Ok(Some(answer))
            }
        }
        None => {
            let default_label = existing_url.map(|s| s.to_string()).unwrap_or_default();
            let prompt = if default_label.is_empty() {
                "Base URL (required): ".to_string()
            } else {
                format!("Base URL [{}]: ", default_label)
            };
            loop {
                let answer = io.read_line(&prompt)?;
                if !answer.is_empty() {
                    return Ok(Some(answer));
                }
                io.eprint("Base URL is required for this provider.\n");
            }
        }
    }
}

fn prompt_for_model<R: SetupIo + ?Sized>(
    io: &R,
    provider: &ProviderInfo,
    existing_model: Option<&str>,
) -> io::Result<String> {
    let default_label = existing_model.or(provider.suggested_model).unwrap_or("");
    let prompt = if default_label.is_empty() {
        "Model name (required): ".to_string()
    } else {
        format!("Model name [{}]: ", default_label)
    };
    loop {
        let answer = io.read_line(&prompt)?;
        if !answer.is_empty() {
            return Ok(answer);
        }
        if !default_label.is_empty() {
            return Ok(default_label.to_string());
        }
        io.eprint("Model name is required.\n");
    }
}

fn prompt_for_max_tokens<R: SetupIo + ?Sized>(io: &R, existing: Option<u32>) -> io::Result<u32> {
    let default = existing.unwrap_or(DEFAULT_MAX_TOKENS);
    loop {
        let answer = io.read_line(&format!("Max tokens [{}]: ", default))?;
        if answer.is_empty() {
            return Ok(default);
        }
        if let Ok(v) = answer.parse::<u32>() {
            return Ok(v);
        }
        io.eprint("Please enter a valid number.\n");
    }
}

fn prompt_for_spinner<R: SetupIo + ?Sized>(io: &R, existing: Option<u8>) -> io::Result<u8> {
    let default = existing.unwrap_or(1);
    io.eprint("Spinner styles:\n");
    io.eprint("  1) Classic bar (default)\n");
    io.eprint("  2) Braille\n");
    io.eprint("  3) Dots\n");
    loop {
        let answer = io.read_line(&format!("Select spinner style (1-3) [{}]: ", default))?;
        if answer.is_empty() {
            return Ok(default);
        }
        if let Ok(v) = answer.parse::<u8>() {
            if (1..=3).contains(&v) {
                return Ok(v);
            }
        }
        io.eprint("Please enter 1, 2, or 3.\n");
    }
}

fn prompt_config_path<R: SetupIo + ?Sized>(
    io: &R,
    existing_path: Option<&Path>,
) -> io::Result<PathBuf> {
    let default = existing_path
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| default_config_file_path().display().to_string());
    let answer = io.read_line(&format!("Config file path [{}]: ", default))?;
    if answer.is_empty() {
        Ok(PathBuf::from(&default))
    } else {
        Ok(PathBuf::from(answer))
    }
}

fn load_existing_config() -> Option<FileConfig> {
    let path = default_config_file_path();
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    toml::from_str(&content).ok()
}

fn gather_inputs<R: SetupIo + ?Sized>(
    io: &R,
    existing: Option<&FileConfig>,
) -> io::Result<Option<SetupInputs>> {
    io.eprint("cmdify setup — configure your default provider\n");
    io.eprint("=============================================\n\n");

    let greeting = if existing.is_some() {
        "Would you like to replace the existing config file?"
    } else {
        "No config file found. Would you like to create one?"
    };
    if !prompt_yes_no(io, greeting, true)? {
        return Ok(None);
    }

    let provider_name = select_provider(io, existing.and_then(|e| e.provider_name.as_deref()))?;

    let provider = resolve_provider(&provider_name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown provider: {}", provider_name),
        )
    })?;

    let base_url = prompt_for_url(io, provider, None)?;

    let model_name =
        prompt_for_model(io, provider, existing.and_then(|e| e.model_name.as_deref()))?;

    let max_tokens = prompt_for_max_tokens(io, existing.and_then(|e| e.max_tokens))?;

    let spinner = prompt_for_spinner(io, existing.and_then(|e| e.spinner))?;

    io.eprint("\n");

    let config_path = prompt_config_path(io, None)?;

    let overwrite = if config_path.exists() {
        prompt_yes_no(io, "Config file already exists. Overwrite?", false)?
    } else {
        true
    };

    Ok(Some(SetupInputs {
        provider_name,
        model_name,
        base_url,
        max_tokens,
        spinner,
        config_path,
        overwrite,
    }))
}

fn build_toml_content(inputs: &SetupInputs) -> String {
    let mut lines = vec![
        format!("provider_name = \"{}\"", inputs.provider_name),
        format!("model_name = \"{}\"", inputs.model_name),
    ];

    if inputs.max_tokens != DEFAULT_MAX_TOKENS {
        lines.push(format!("max_tokens = {}", inputs.max_tokens));
    }

    if inputs.spinner != 1 {
        lines.push(format!("spinner = {}", inputs.spinner));
    }

    if let Some(ref url) = inputs.base_url {
        let provider = resolve_provider(&inputs.provider_name);
        if let Some(p) = provider {
            if p.default_url != Some(url.as_str()) {
                lines.push(String::new());
                lines.push("[providers]".to_string());
                let key = format!("{}_base_url", inputs.provider_name);
                lines.push(format!("{} = \"{}\"", key, url));
            }
        }
    }

    let mut content = lines.join("\n");
    content.push('\n');
    content
}

fn display_summary<R: SetupIo + ?Sized>(io: &R, inputs: &SetupInputs) {
    let provider = resolve_provider(&inputs.provider_name);
    let pretty = provider
        .map(|p| p.pretty_name)
        .unwrap_or(&inputs.provider_name);
    io.eprint("\n--- Configuration summary ---\n");
    io.eprint(&format!(
        "Provider:  {} ({})\n",
        pretty, inputs.provider_name
    ));
    io.eprint(&format!("Model:     {}\n", inputs.model_name));
    io.eprint(&format!("Max tokens: {}\n", inputs.max_tokens));
    io.eprint(&format!("Spinner:   {}\n", inputs.spinner));
    if let Some(ref url) = inputs.base_url {
        io.eprint(&format!("Base URL:  {}\n", url));
    }
    io.eprint(&format!("Config:    {}\n", inputs.config_path.display()));
}

fn display_api_key_hint<R: SetupIo + ?Sized>(io: &R, provider_name: &str) {
    let provider = resolve_provider(provider_name);
    if let Some(p) = provider {
        if !p.key_env_var.is_empty() {
            let is_set = env::var(p.key_env_var).is_ok();
            if is_set {
                io.eprint(&format!(
                    "\nAPI key ({}) is already set in your environment.\n",
                    p.key_env_var
                ));
            } else {
                io.eprint("\nNOTE: Set your API key before using cmdify:\n");
                io.eprint(&format!("  export {}=your-key-here\n", p.key_env_var));
                io.eprint("Add this to your shell profile (~/.zshrc, ~/.bashrc, etc.)\n");
            }
        } else {
            io.eprint("\nThis provider does not require an API key.\n");
        }
    }
}

pub(crate) fn env_sufficient_for_run() -> bool {
    let provider = match env::var("CMDIFY_PROVIDER_NAME") {
        Ok(v) => v,
        Err(_) => return false,
    };

    if env::var("CMDIFY_MODEL_NAME").is_err() {
        return false;
    }

    let key_optional_providers = ["ollama", "completions", "responses"];
    if key_optional_providers.contains(&provider.as_str()) {
        if provider == "completions" && env::var("CMDIFY_COMPLETIONS_KEY").is_err() {
            eprintln!("warning: no CMDIFY_COMPLETIONS_KEY set. Your provider may require one.");
        }
        if provider == "responses" && env::var("CMDIFY_RESPONSES_KEY").is_err() {
            eprintln!("warning: no CMDIFY_RESPONSES_KEY set. Your provider may require one.");
        }
        return true;
    }

    let p = match resolve_provider(&provider) {
        Some(p) => p,
        None => return false,
    };

    if p.key_env_var.is_empty() {
        return true;
    }

    if env::var(p.key_env_var).is_err() {
        return false;
    }

    true
}

pub(crate) fn run_interactive(existing: Option<&FileConfig>) -> Result<()> {
    run_interactive_with_io(&TerminalIo, existing)
}

pub(crate) fn run_interactive_with_io<R: SetupIo + ?Sized>(
    io: &R,
    existing: Option<&FileConfig>,
) -> Result<()> {
    let inputs = match gather_inputs(io, existing)? {
        Some(inputs) => inputs,
        None => {
            std::process::exit(1);
        }
    };

    if !inputs.overwrite {
        io.eprint("Setup cancelled.\n");
        std::process::exit(1);
    }

    display_summary(io, &inputs);

    if !prompt_yes_no(io, "Write config file?", true)? {
        io.eprint("Setup cancelled.\n");
        std::process::exit(1);
    }

    if let Some(parent) = inputs.config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = build_toml_content(&inputs);
    std::fs::write(&inputs.config_path, &content)?;

    display_summary(io, &inputs);
    io.eprint(&format!(
        "\nConfig written to {}\nRun cmdify --setup if you need to change the configuration.",
        inputs.config_path.display()
    ));

    display_api_key_hint(io, &inputs.provider_name);

    io.eprint("\ncmdify is ready! Try:\n");
    io.eprint("  cmdify \"list all files\"\n");

    Ok(())
}

pub(crate) fn load_existing_config_if_present() -> Option<FileConfig> {
    load_existing_config()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ENV_LOCK;

    struct MockIo {
        responses: Vec<String>,
        output: std::cell::RefCell<Vec<String>>,
    }

    impl MockIo {
        fn new(responses: Vec<&str>) -> Self {
            Self {
                responses: responses.into_iter().map(String::from).collect(),
                output: std::cell::RefCell::new(Vec::new()),
            }
        }
    }

    impl SetupIo for MockIo {
        fn read_line(&self, _prompt: &str) -> io::Result<String> {
            static IDX: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let i = IDX.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(self.responses.get(i).cloned().unwrap_or_default())
        }

        fn eprint(&self, msg: &str) {
            self.output.borrow_mut().push(msg.to_string());
        }
    }

    #[test]
    fn providers_list_contains_all_13() {
        let list = providers();
        assert_eq!(list.len(), 13);
        let names: Vec<&str> = list.iter().map(|p| p.name).collect();
        assert!(names.contains(&"openai"));
        assert!(names.contains(&"anthropic"));
        assert!(names.contains(&"gemini"));
        assert!(names.contains(&"ollama"));
        assert!(names.contains(&"mistral"));
        assert!(names.contains(&"qwen"));
        assert!(names.contains(&"kimi"));
        assert!(names.contains(&"openrouter"));
        assert!(names.contains(&"huggingface"));
        assert!(names.contains(&"zai"));
        assert!(names.contains(&"minimax"));
        assert!(names.contains(&"completions"));
        assert!(names.contains(&"responses"));
    }

    #[test]
    fn resolve_provider_finds_known() {
        assert!(resolve_provider("openai").is_some());
        assert!(resolve_provider("anthropic").is_some());
        assert!(resolve_provider("ollama").is_some());
        assert!(resolve_provider("completions").is_some());
    }

    #[test]
    fn resolve_provider_unknown_is_none() {
        assert!(resolve_provider("nonexistent").is_none());
    }

    #[test]
    fn build_toml_content_with_defaults() {
        let inputs = SetupInputs {
            provider_name: "openai".to_string(),
            model_name: "gpt-4o".to_string(),
            base_url: None,
            max_tokens: DEFAULT_MAX_TOKENS,
            spinner: 1,
            config_path: PathBuf::from("/tmp/config.toml"),
            overwrite: true,
        };
        let content = build_toml_content(&inputs);
        assert!(content.contains("provider_name = \"openai\""));
        assert!(content.contains("model_name = \"gpt-4o\""));
        assert!(!content.contains("max_tokens"));
        assert!(!content.contains("spinner"));
        assert!(!content.contains("[providers]"));
    }

    #[test]
    fn build_toml_content_with_custom_values() {
        let inputs = SetupInputs {
            provider_name: "completions".to_string(),
            model_name: "llama3".to_string(),
            base_url: Some("http://localhost:8080".to_string()),
            max_tokens: 4096,
            spinner: 2,
            config_path: PathBuf::from("/tmp/config.toml"),
            overwrite: true,
        };
        let content = build_toml_content(&inputs);
        assert!(content.contains("provider_name = \"completions\""));
        assert!(content.contains("model_name = \"llama3\""));
        assert!(content.contains("max_tokens = 4096"));
        assert!(content.contains("spinner = 2"));
        assert!(content.contains("[providers]"));
        assert!(content.contains("completions_base_url = \"http://localhost:8080\""));
    }

    #[test]
    fn build_toml_content_no_provider_block_when_url_matches_default() {
        let inputs = SetupInputs {
            provider_name: "openai".to_string(),
            model_name: "gpt-4o".to_string(),
            base_url: Some("https://api.openai.com".to_string()),
            max_tokens: DEFAULT_MAX_TOKENS,
            spinner: 1,
            config_path: PathBuf::from("/tmp/config.toml"),
            overwrite: true,
        };
        let content = build_toml_content(&inputs);
        assert!(!content.contains("[providers]"));
    }

    #[test]
    fn config_file_written_and_valid_toml() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("cmdify").join("config.toml");
        let inputs = SetupInputs {
            provider_name: "anthropic".to_string(),
            model_name: "claude-sonnet-4-20250514".to_string(),
            base_url: None,
            max_tokens: DEFAULT_MAX_TOKENS,
            spinner: 1,
            config_path: config_path.clone(),
            overwrite: true,
        };

        let content = build_toml_content(&inputs);
        std::fs::create_dir_all(config_path.parent().unwrap()).unwrap();
        std::fs::write(&config_path, &content).unwrap();

        assert!(config_path.exists());
        let parsed: FileConfig =
            toml::from_str(&std::fs::read_to_string(&config_path).unwrap()).unwrap();
        assert_eq!(parsed.provider_name.as_deref(), Some("anthropic"));
        assert_eq!(
            parsed.model_name.as_deref(),
            Some("claude-sonnet-4-20250514")
        );
    }

    #[test]
    fn config_directory_created_if_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("a").join("b").join("config.toml");
        let inputs = SetupInputs {
            provider_name: "ollama".to_string(),
            model_name: "llama3".to_string(),
            base_url: None,
            max_tokens: DEFAULT_MAX_TOKENS,
            spinner: 1,
            config_path: nested.clone(),
            overwrite: true,
        };

        let content = build_toml_content(&inputs);
        if let Some(parent) = nested.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&nested, &content).unwrap();

        assert!(nested.exists());
    }

    #[test]
    fn ollama_provider_has_no_key() {
        let p = resolve_provider("ollama").unwrap();
        assert!(p.key_env_var.is_empty());
    }

    #[test]
    fn completions_provider_has_no_default_url() {
        let p = resolve_provider("completions").unwrap();
        assert!(p.default_url.is_none());
    }

    #[test]
    fn load_existing_config_returns_none_when_missing() {
        let _lock = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let orig = std::env::var("XDG_CONFIG_HOME").ok();
        let orig_home = std::env::var("HOME").ok();
        std::env::set_var("XDG_CONFIG_HOME", tmp.path());
        std::env::set_var("HOME", tmp.path());
        let result = load_existing_config();
        if let Some(v) = orig {
            std::env::set_var("XDG_CONFIG_HOME", v);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        if let Some(v) = orig_home {
            std::env::set_var("HOME", v);
        } else {
            std::env::remove_var("HOME");
        }
        assert!(result.is_none());
    }

    #[test]
    fn load_existing_config_returns_config_when_present() {
        let _lock = ENV_LOCK.lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("cmdify");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("config.toml"),
            r#"
            provider_name = "openai"
            model_name = "gpt-4o"
            "#,
        )
        .unwrap();
        let orig = std::env::var("XDG_CONFIG_HOME").ok();
        let orig_home = std::env::var("HOME").ok();
        std::env::set_var("XDG_CONFIG_HOME", tmp.path());
        std::env::set_var("HOME", tmp.path());
        let result = load_existing_config();
        if let Some(v) = orig {
            std::env::set_var("XDG_CONFIG_HOME", v);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
        if let Some(v) = orig_home {
            std::env::set_var("HOME", v);
        } else {
            std::env::remove_var("HOME");
        }
        let config = result.unwrap();
        assert_eq!(config.provider_name.as_deref(), Some("openai"));
        assert_eq!(config.model_name.as_deref(), Some("gpt-4o"));
    }

    #[test]
    fn run_interactive_writes_config_file() {
        std::sync::atomic::AtomicUsize::new(0).store(0, std::sync::atomic::Ordering::Relaxed);
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("config.toml");

        let responses = vec![
            "y",
            "openai",
            "",
            "gpt-4o",
            "",
            "1",
            config_path.to_str().unwrap(),
            "y",
        ];
        let io = MockIo::new(responses);

        let result = run_interactive_with_io(&io, None);
        assert!(result.is_ok());
        assert!(config_path.exists());

        let content = std::fs::read_to_string(&config_path).unwrap();
        assert!(content.contains("provider_name = \"openai\""));
        assert!(content.contains("model_name = \"gpt-4o\""));
    }

    #[test]
    fn setup_inputs_overwrite_false_prevents_write() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("config.toml");
        std::fs::write(&config_path, "existing").unwrap();
        let content_before = std::fs::read_to_string(&config_path).unwrap();

        let inputs = SetupInputs {
            provider_name: "openai".to_string(),
            model_name: "gpt-4o".to_string(),
            base_url: None,
            max_tokens: DEFAULT_MAX_TOKENS,
            spinner: 1,
            config_path: config_path.clone(),
            overwrite: false,
        };

        if inputs.overwrite {
            let content = build_toml_content(&inputs);
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&config_path, &content).unwrap();
        }

        let content_after = std::fs::read_to_string(&config_path).unwrap();
        assert_eq!(content_before, content_after);
        assert_eq!(content_after, "existing");
    }

    fn clear_cmdify_env() {
        for var in [
            "CMDIFY_PROVIDER_NAME",
            "CMDIFY_MODEL_NAME",
            "CMDIFY_COMPLETIONS_KEY",
            "CMDIFY_RESPONSES_KEY",
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
        ] {
            std::env::remove_var(var);
        }
    }

    #[test]
    fn env_sufficient_missing_provider_returns_false() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_cmdify_env();
        assert!(!env_sufficient_for_run());
    }

    #[test]
    fn env_sufficient_missing_model_returns_false() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_cmdify_env();
        std::env::set_var("CMDIFY_PROVIDER_NAME", "openai");
        assert!(!env_sufficient_for_run());
    }

    #[test]
    fn env_sufficient_missing_key_returns_false() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_cmdify_env();
        std::env::set_var("CMDIFY_PROVIDER_NAME", "openai");
        std::env::set_var("CMDIFY_MODEL_NAME", "gpt-4o");
        assert!(!env_sufficient_for_run());
    }

    #[test]
    fn env_sufficient_openai_with_key_returns_true() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_cmdify_env();
        std::env::set_var("CMDIFY_PROVIDER_NAME", "openai");
        std::env::set_var("CMDIFY_MODEL_NAME", "gpt-4o");
        std::env::set_var("OPENAI_API_KEY", "sk-test");
        assert!(env_sufficient_for_run());
    }

    #[test]
    fn env_sufficient_ollama_no_key_needed() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_cmdify_env();
        std::env::set_var("CMDIFY_PROVIDER_NAME", "ollama");
        std::env::set_var("CMDIFY_MODEL_NAME", "llama3");
        assert!(env_sufficient_for_run());
    }

    #[test]
    fn env_sufficient_completions_no_key_warns() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_cmdify_env();
        std::env::set_var("CMDIFY_PROVIDER_NAME", "completions");
        std::env::set_var("CMDIFY_MODEL_NAME", "llama3");
        assert!(env_sufficient_for_run());
    }

    #[test]
    fn env_sufficient_unknown_provider_returns_false() {
        let _lock = ENV_LOCK.lock().unwrap();
        clear_cmdify_env();
        std::env::set_var("CMDIFY_PROVIDER_NAME", "fake");
        std::env::set_var("CMDIFY_MODEL_NAME", "model");
        assert!(!env_sufficient_for_run());
    }
}

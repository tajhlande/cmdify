use std::env;
use std::fs;
use std::path::Path;
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
    env::set_var("XDG_CONFIG_HOME", "/nonexistent-cmdify-test-config");
    env::set_var("HOME", "/nonexistent-cmdify-test-home");
}

fn setup_completions_env() {
    cleanup_vars();
    env::set_var("CMDIFY_PROVIDER_NAME", "completions");
    env::set_var("CMDIFY_MODEL_NAME", "test-model");
    env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");
}

fn write_toml_config(dir: &Path, content: &str) {
    let config_dir = dir.join("cmdify");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), content).unwrap();
}

#[test]
fn full_completions_config() {
    with_env_lock(|| {
        setup_completions_env();
        let config = cmdify::config::Config::from_env(None).unwrap();
        assert_eq!(config.provider_name, "completions");
        assert_eq!(config.model_name, "test-model");
        assert_eq!(config.max_tokens, 16384);
        assert_eq!(config.provider_settings.base_url, "http://localhost:11434");
        assert!(config.provider_settings.api_key.is_none());
    });
}

#[test]
fn completions_config_with_key() {
    with_env_lock(|| {
        setup_completions_env();
        env::set_var("CMDIFY_COMPLETIONS_KEY", "my-secret-key");
        let config = cmdify::config::Config::from_env(None).unwrap();
        assert_eq!(
            config.provider_settings.api_key.as_deref(),
            Some("my-secret-key")
        );
    });
}

#[test]
fn missing_provider_name_errors() {
    with_env_lock(|| {
        cleanup_vars();
        let result = cmdify::config::Config::from_env(None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("CMDIFY_PROVIDER_NAME"));
    });
}

#[test]
fn missing_model_name_errors() {
    with_env_lock(|| {
        cleanup_vars();
        env::set_var("CMDIFY_PROVIDER_NAME", "completions");
        let result = cmdify::config::Config::from_env(None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("CMDIFY_MODEL_NAME"));
    });
}

#[test]
fn missing_completions_url_errors() {
    with_env_lock(|| {
        cleanup_vars();
        env::set_var("CMDIFY_PROVIDER_NAME", "completions");
        env::set_var("CMDIFY_MODEL_NAME", "test");
        let result = cmdify::config::Config::from_env(None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("CMDIFY_COMPLETIONS_URL"));
    });
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
            model_name = "file-model"
            "#,
        );
        env::set_var("XDG_CONFIG_HOME", tmp.path());
        env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

        let config = cmdify::config::Config::from_env(None).unwrap();
        assert_eq!(config.provider_name, "completions");
        assert_eq!(config.model_name, "file-model");
        assert_eq!(config.max_tokens, 16384);
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
            model_name = "file-model"
            max_tokens = 2048
            "#,
        );
        env::set_var("XDG_CONFIG_HOME", tmp.path());
        env::set_var("CMDIFY_PROVIDER_NAME", "completions");
        env::set_var("CMDIFY_MODEL_NAME", "env-model");
        env::set_var("CMDIFY_MAX_TOKENS", "8192");
        env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

        let config = cmdify::config::Config::from_env(None).unwrap();
        assert_eq!(config.model_name, "env-model");
        assert_eq!(config.max_tokens, 8192);
    });
}

#[test]
fn config_file_max_tokens() {
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

        let config = cmdify::config::Config::from_env(None).unwrap();
        assert_eq!(config.max_tokens, 2048);
    });
}

#[test]
fn config_file_system_prompt() {
    with_env_lock(|| {
        cleanup_vars();
        let tmp = tempfile::tempdir().unwrap();
        write_toml_config(
            tmp.path(),
            r#"
            provider_name = "completions"
            model_name = "llama3"
            system_prompt_file = "/custom/prompt.txt"
            "#,
        );
        env::set_var("XDG_CONFIG_HOME", tmp.path());
        env::set_var("CMDIFY_COMPLETIONS_URL", "http://localhost:11434");

        let config = cmdify::config::Config::from_env(None).unwrap();
        assert_eq!(
            config.system_prompt_override.as_deref(),
            Some("/custom/prompt.txt")
        );
    });
}

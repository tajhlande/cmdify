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

    let os = detect_os_version();

    Ok(format!(
        "{}\n\nThe user's operating system is {} and their shell is {}.",
        base_prompt, os, shell
    ))
}

fn detect_os_version() -> String {
    if command_exists("sw_vers").is_some() {
        if let Some(version) = parse_sw_vers() {
            return version;
        }
    }

    if command_exists("lsb_release").is_some() {
        if let Some(version) = parse_lsb_release() {
            return version;
        }
    }

    if let Some(version) = parse_os_release() {
        return version;
    }

    match std::process::Command::new("uname").arg("-s").output() {
        Ok(output) => {
            let s = String::from_utf8_lossy(&output.stdout);
            match s.trim() {
                "Darwin" => "macOS".to_string(),
                other => other.to_string(),
            }
        }
        Err(_) => "unknown".to_string(),
    }
}

fn command_exists(cmd: &str) -> Option<String> {
    match std::process::Command::new("which").arg(cmd).output() {
        Ok(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if path.is_empty() {
                None
            } else {
                Some(path)
            }
        }
        _ => None,
    }
}

fn parse_sw_vers() -> Option<String> {
    let output = std::process::Command::new("sw_vers").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut product_name = None;
    let mut product_version = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("ProductName:") {
            product_name = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("ProductVersion:") {
            product_version = Some(rest.trim().to_string());
        }
    }
    match (product_name, product_version) {
        (Some(name), Some(ver)) => Some(format!("{} {}", name, ver)),
        _ => None,
    }
}

fn parse_lsb_release() -> Option<String> {
    let output = std::process::Command::new("lsb_release")
        .arg("-a")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("Description:") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

fn parse_os_release() -> Option<String> {
    let content = std::fs::read_to_string("/etc/os-release").ok()?;
    for line in content.lines() {
        for key in &["PRETTY_NAME", "VERSION", "NAME"] {
            if let Some(rest) = line.strip_prefix(&format!("{}=", key)) {
                let value = rest.trim_matches('"').to_string();
                if !value.is_empty() {
                    return Some(value);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_env_lock<F: FnOnce()>(f: F) {
        let lock = ENV_LOCK.lock();
        if lock.is_err() {
            return;
        }
        let _lock = lock.unwrap();
        f();
    }

    fn make_config_with_override(override_path: Option<&str>) -> Config {
        Config {
            provider_name: "completions".into(),
            model_name: "test".into(),
            max_tokens: 4096,
            system_prompt_override: override_path.map(|p| p.to_string()),
            spinner: 1,
            allow_unsafe: false,
            quiet: false,
            blind: false,
            no_tools: false,
            yolo: false,
            debug_level: 0,
            tool_level: 1,
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
            assert!(prompt.contains("The user's operating system is"));
            assert!(prompt.contains("and their shell is bash."));
        });
    }

    #[test]
    fn shell_detection_zsh() {
        with_env_lock(|| {
            env::set_var("SHELL", "/bin/zsh");
            let config = make_config_with_override(None);
            let prompt = load_system_prompt(&config).unwrap();
            assert!(prompt.contains("and their shell is zsh."));
        });
    }

    #[test]
    fn shell_detection_defaults_to_bash() {
        with_env_lock(|| {
            env::remove_var("SHELL");
            let config = make_config_with_override(None);
            let prompt = load_system_prompt(&config).unwrap();
            assert!(prompt.contains("and their shell is bash."));
        });
    }
}

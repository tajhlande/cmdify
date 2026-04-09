use std::env;

use crate::config::Config;
use crate::error::Result;

pub const PROMPT_BASE: &str = include_str!("system_prompt_base.txt");
pub const PROMPT_TOOLS: &str = include_str!("system_prompt_tools.txt");
pub const PROMPT_SAFETY: &str = include_str!("system_prompt_safety.txt");
pub const PROMPT_UNSAFE: &str = include_str!("system_prompt_unsafe.txt");

// System prompt is assembled from modular pieces depending on the configuration:
//   - Custom override: replaces ALL pieces (base + tools + safety); only OS/shell info appended.
//   - No override:     base + tools (if enabled) + safety-or-unsafe (depending on flag).
pub fn load_system_prompt(config: &Config) -> Result<String> {
    let has_override = config.system_prompt_override.is_some();

    let base = if let Some(ref path) = config.system_prompt_override {
        std::fs::read_to_string(path)?
    } else {
        PROMPT_BASE.to_string()
    };

    let mut parts = vec![base];

    if !has_override && !config.no_tools {
        parts.push(PROMPT_TOOLS.to_string());
    }

    if !has_override {
        if config.allow_unsafe {
            parts.push(PROMPT_UNSAFE.to_string());
        } else {
            parts.push(PROMPT_SAFETY.to_string());
        }
    }

    let os = detect_os_version();

    let shell = env::var("SHELL")
        .ok()
        .and_then(|s| s.rsplit('/').next().map(|n| n.to_string()))
        .unwrap_or_else(|| "bash".to_string());

    parts.push(format!(
        "The user's operating system is {} and their shell is {}.",
        os, shell
    ));

    Ok(parts.join("\n\n"))
}

// OS detection uses a fallback chain: sw_vers (macOS) → lsb_release (Debian/Ubuntu) →
// /etc/os-release (systemd-based distros) → uname -s (broad fallback).
// Each step only runs the next if the previous one fails.
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
    use crate::config::ENV_LOCK;

    fn with_env_lock<F: FnOnce()>(f: F) {
        let lock = ENV_LOCK.lock();
        if lock.is_err() {
            return;
        }
        let _lock = lock.unwrap();
        f();
    }

    fn make_config(no_tools: bool, allow_unsafe: bool, override_path: Option<&str>) -> Config {
        Config {
            provider_name: "completions".into(),
            model_name: "test".into(),
            max_tokens: 4096,
            system_prompt_override: override_path.map(|p| p.to_string()),
            spinner: 1,
            allow_unsafe,
            quiet: false,
            blind: false,
            no_tools,
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
    fn prompt_base_not_empty() {
        assert!(!PROMPT_BASE.is_empty());
    }

    #[test]
    fn prompt_tools_not_empty() {
        assert!(!PROMPT_TOOLS.is_empty());
    }

    #[test]
    fn prompt_safety_not_empty() {
        assert!(!PROMPT_SAFETY.is_empty());
    }

    #[test]
    fn prompt_unsafe_not_empty() {
        assert!(!PROMPT_UNSAFE.is_empty());
    }

    #[test]
    fn default_assembly_includes_base_tools_safety_shell() {
        with_env_lock(|| {
            env::remove_var("SHELL");
            let config = make_config(false, false, None);
            let prompt = load_system_prompt(&config).unwrap();

            assert!(prompt.contains("shell command generator"));
            assert!(prompt.contains("Tool usage:"));
            assert!(prompt.contains("Safety:"));
            assert!(prompt.contains("and their shell is bash."));
        });
    }

    #[test]
    fn no_tools_excludes_tools_piece() {
        with_env_lock(|| {
            env::remove_var("SHELL");
            let config = make_config(true, false, None);
            let prompt = load_system_prompt(&config).unwrap();

            assert!(prompt.contains("shell command generator"));
            assert!(prompt.contains("Safety:"));
            assert!(!prompt.contains("Tool usage:"));
        });
    }

    #[test]
    fn unsafe_includes_unsafe_piece_excludes_safety() {
        with_env_lock(|| {
            env::remove_var("SHELL");
            let config = make_config(false, true, None);
            let prompt = load_system_prompt(&config).unwrap();

            assert!(prompt.contains("shell command generator"));
            assert!(prompt.contains("Tool usage:"));
            assert!(prompt.contains("Unsafe mode is active"));
            assert!(!prompt.contains("Safety:"));
        });
    }

    #[test]
    fn no_tools_unsafe_includes_base_unsafe_shell_only() {
        with_env_lock(|| {
            env::remove_var("SHELL");
            let config = make_config(true, true, None);
            let prompt = load_system_prompt(&config).unwrap();

            assert!(prompt.contains("shell command generator"));
            assert!(prompt.contains("Unsafe mode is active"));
            assert!(!prompt.contains("Tool usage:"));
            assert!(!prompt.contains("Safety:"));
        });
    }

    #[test]
    fn custom_override_replaces_all_pieces() {
        with_env_lock(|| {
            env::remove_var("SHELL");
            let dir = tempfile::tempdir().unwrap();
            let custom_path = dir.path().join("custom_prompt.txt");
            std::fs::write(&custom_path, "Custom system prompt.").unwrap();

            let config = make_config(false, false, Some(custom_path.to_str().unwrap()));
            let prompt = load_system_prompt(&config).unwrap();

            assert!(prompt.starts_with("Custom system prompt."));
            assert!(!prompt.contains("Tool usage:"));
            assert!(!prompt.contains("Safety:"));
            assert!(!prompt.contains("Unsafe mode"));
        });
    }

    #[test]
    fn shell_detection_zsh() {
        with_env_lock(|| {
            env::set_var("SHELL", "/bin/zsh");
            let config = make_config(false, false, None);
            let prompt = load_system_prompt(&config).unwrap();
            assert!(prompt.contains("and their shell is zsh."));
        });
    }

    #[test]
    fn shell_detection_defaults_to_bash() {
        with_env_lock(|| {
            env::remove_var("SHELL");
            let config = make_config(false, false, None);
            let prompt = load_system_prompt(&config).unwrap();
            assert!(prompt.contains("and their shell is bash."));
        });
    }

    #[test]
    fn os_version_included() {
        with_env_lock(|| {
            env::remove_var("SHELL");
            let config = make_config(false, false, None);
            let prompt = load_system_prompt(&config).unwrap();
            assert!(prompt.contains("The user's operating system is"));
        });
    }

    #[test]
    fn shell_detection_no_tools_unsafe() {
        with_env_lock(|| {
            env::set_var("SHELL", "/usr/local/bin/fish");
            let config = make_config(true, true, None);
            let prompt = load_system_prompt(&config).unwrap();
            assert!(prompt.contains("and their shell is fish."));
        });
    }
}

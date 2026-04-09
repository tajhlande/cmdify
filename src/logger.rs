use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

pub struct CmdifyLogger {
    file: Mutex<Option<File>>,
    model: String,
    provider: String,
}

impl CmdifyLogger {
    pub fn new(model: &str, provider: &str) -> Self {
        let log_path = Self::log_file_path();
        let file = Self::open_log_file(&log_path);

        Self {
            file: Mutex::new(file),
            model: model.to_string(),
            provider: provider.to_string(),
        }
    }

    pub fn log(&self, source: &str, command: &str) {
        let timestamp: chrono::DateTime<chrono::Utc> = SystemTime::now().into();
        let timestamp = timestamp.format("%Y-%m-%dT%H:%M:%SZ").to_string();

        let line = format!(
            "[{}] [{}] [{}/{}] {}\n",
            timestamp, source, self.provider, self.model, command
        );

        if let Ok(mut guard) = self.file.lock() {
            if let Some(ref mut f) = *guard {
                let _ = f.write_all(line.as_bytes());
            }
        }
    }

    // History log path follows XDG Base Directory specification:
    //   $XDG_STATE_HOME/cmdify/history.log
    // Falls back to ~/.local/state/cmdify/history.log, or "cmdify.log" in cwd
    // if neither $XDG_STATE_HOME nor $HOME is set.
    fn log_file_path() -> PathBuf {
        let state_home = if let Ok(xdg) = std::env::var("XDG_STATE_HOME") {
            PathBuf::from(xdg)
        } else if let Ok(home) = std::env::var("HOME") {
            PathBuf::from(home).join(".local").join("state")
        } else {
            return PathBuf::from("cmdify.log");
        };

        state_home.join("cmdify").join("history.log")
    }

    fn open_log_file(path: &Path) -> Option<File> {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        OpenOptions::new().create(true).append(true).open(path).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ENV_LOCK;

    fn with_env_lock<F: FnOnce()>(f: F) {
        let _lock = ENV_LOCK.lock().unwrap();
        f();
    }

    #[test]
    fn log_file_path_with_xdg_state_home() {
        with_env_lock(|| {
            std::env::remove_var("XDG_STATE_HOME");
            std::env::set_var("HOME", "/tmp/test-home");
            let path = CmdifyLogger::log_file_path();
            assert!(path.ends_with(".local/state/cmdify/history.log"));
            assert!(path.starts_with("/tmp/test-home"));
        });
    }

    #[test]
    fn log_file_path_with_xdg_override() {
        with_env_lock(|| {
            std::env::set_var("XDG_STATE_HOME", "/tmp/test-state");
            std::env::remove_var("HOME");
            let path = CmdifyLogger::log_file_path();
            assert_eq!(path, PathBuf::from("/tmp/test-state/cmdify/history.log"));
        });
    }

    #[test]
    fn new_logger_creates_file() {
        with_env_lock(|| {
            let dir = tempfile::tempdir().unwrap();
            std::env::set_var("XDG_STATE_HOME", dir.path());
            std::env::remove_var("HOME");

            let logger = CmdifyLogger::new("test-model", "completions");
            logger.log("output", "ls -la");

            let log_path = dir.path().join("cmdify").join("history.log");
            assert!(log_path.exists());

            let contents = std::fs::read_to_string(&log_path).unwrap();
            assert!(contents.contains("[output] [completions/test-model] ls -la"));
            assert!(contents.contains("T"));
        });
    }

    #[test]
    fn multiple_entries_append() {
        with_env_lock(|| {
            let dir = tempfile::tempdir().unwrap();
            std::env::set_var("XDG_STATE_HOME", dir.path());
            std::env::remove_var("HOME");

            let logger = CmdifyLogger::new("llama3", "ollama");
            logger.log("find_command", "command -v fd");
            logger.log("find_command", "command -v rg");

            let log_path = dir.path().join("cmdify").join("history.log");
            let contents = std::fs::read_to_string(&log_path).unwrap();
            let lines: Vec<&str> = contents.lines().collect();
            assert_eq!(lines.len(), 2);
            assert!(lines[0].contains("command -v fd"));
            assert!(lines[1].contains("command -v rg"));
        });
    }

    #[test]
    fn logger_works_without_xdg_or_home() {
        with_env_lock(|| {
            let _ = CmdifyLogger::new("test", "test");
        });
    }
}

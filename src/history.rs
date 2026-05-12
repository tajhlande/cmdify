use std::fs;
use std::io::Write;
use std::path::PathBuf;

const MAX_HISTORY_LINES: usize = 10000;

pub fn history_file_path() -> PathBuf {
    let cache_home = if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(xdg)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".cache")
    } else {
        return PathBuf::from("cmdify_history.txt");
    };

    cache_home.join("cmdify").join("history.txt")
}

pub fn ensure_parent_dir(path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
}

pub fn max_history_lines() -> usize {
    MAX_HISTORY_LINES
}

pub fn append_to_history(entry: &str) {
    let path = history_file_path();
    ensure_parent_dir(&path);
    if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{}", entry);
    }
    trim_history_if_needed(&path);
}

fn trim_history_if_needed(path: &std::path::Path) {
    let contents = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let lines: Vec<&str> = contents.lines().collect();
    if lines.len() <= MAX_HISTORY_LINES {
        return;
    }
    let keep = &lines[lines.len() - MAX_HISTORY_LINES..];
    if let Ok(mut file) = fs::File::create(path) {
        for line in keep {
            let _ = writeln!(file, "{}", line);
        }
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
    fn history_path_with_xdg_cache_home() {
        with_env_lock(|| {
            std::env::set_var("XDG_CACHE_HOME", "/tmp/test-cache");
            std::env::remove_var("HOME");
            let path = history_file_path();
            assert_eq!(path, PathBuf::from("/tmp/test-cache/cmdify/history.txt"));
        });
    }

    #[test]
    fn history_path_with_home_fallback() {
        with_env_lock(|| {
            std::env::remove_var("XDG_CACHE_HOME");
            std::env::set_var("HOME", "/tmp/test-home");
            let path = history_file_path();
            assert_eq!(
                path,
                PathBuf::from("/tmp/test-home/.cache/cmdify/history.txt")
            );
        });
    }

    #[test]
    fn history_path_no_xdg_no_home() {
        with_env_lock(|| {
            std::env::remove_var("XDG_CACHE_HOME");
            std::env::remove_var("HOME");
            let path = history_file_path();
            assert_eq!(path, PathBuf::from("cmdify_history.txt"));
        });
    }

    #[test]
    fn append_creates_file_and_directory() {
        let dir = tempfile::tempdir().unwrap();
        with_env_lock(|| {
            std::env::set_var("XDG_CACHE_HOME", dir.path());
            std::env::remove_var("HOME");

            append_to_history("find all pdf files");

            let path = dir.path().join("cmdify").join("history.txt");
            assert!(path.exists());
            let contents = fs::read_to_string(&path).unwrap();
            assert!(contents.contains("find all pdf files"));
        });
    }

    #[test]
    fn append_multiple_entries() {
        let dir = tempfile::tempdir().unwrap();
        with_env_lock(|| {
            std::env::set_var("XDG_CACHE_HOME", dir.path());
            std::env::remove_var("HOME");

            append_to_history("first command");
            append_to_history("second command");

            let path = dir.path().join("cmdify").join("history.txt");
            let contents = fs::read_to_string(&path).unwrap();
            let lines: Vec<&str> = contents.lines().collect();
            assert_eq!(lines.len(), 2);
            assert_eq!(lines[0], "first command");
            assert_eq!(lines[1], "second command");
        });
    }

    #[test]
    fn xdg_takes_precedence_over_home() {
        with_env_lock(|| {
            std::env::set_var("XDG_CACHE_HOME", "/xdg-path");
            std::env::set_var("HOME", "/home-path");
            let path = history_file_path();
            assert!(path.starts_with("/xdg-path"));
        });
    }

    #[test]
    fn trim_trims_oldest_entries_at_limit() {
        let dir = tempfile::tempdir().unwrap();
        with_env_lock(|| {
            std::env::set_var("XDG_CACHE_HOME", dir.path());
            std::env::remove_var("HOME");

            for i in 0..=MAX_HISTORY_LINES {
                append_to_history(&format!("entry {:05}", i));
            }

            let path = dir.path().join("cmdify").join("history.txt");
            let contents = fs::read_to_string(&path).unwrap();
            let lines: Vec<&str> = contents.lines().collect();
            assert_eq!(lines.len(), MAX_HISTORY_LINES);
            assert!(lines[0].contains("entry 00001"));
            assert!(
                lines[MAX_HISTORY_LINES - 1].contains(&format!("entry {:05}", MAX_HISTORY_LINES))
            );
        });
    }

    #[test]
    fn trim_not_triggered_below_limit() {
        let dir = tempfile::tempdir().unwrap();
        with_env_lock(|| {
            std::env::set_var("XDG_CACHE_HOME", dir.path());
            std::env::remove_var("HOME");

            for i in 0..50 {
                append_to_history(&format!("entry {}", i));
            }

            let path = dir.path().join("cmdify").join("history.txt");
            let contents = fs::read_to_string(&path).unwrap();
            let lines: Vec<&str> = contents.lines().collect();
            assert_eq!(lines.len(), 50);
            assert!(lines[0].contains("entry 0"));
        });
    }

    #[test]
    fn max_history_lines_is_10000() {
        assert_eq!(max_history_lines(), 10000);
    }
}

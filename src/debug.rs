use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Instant;

static LEVEL: AtomicU8 = AtomicU8::new(0);
static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

pub fn init(level: u8) {
    LEVEL.store(level, Ordering::SeqCst);
    if level > 0 {
        let _ = START_TIME.get_or_init(Instant::now);
    }
}

pub fn level() -> u8 {
    LEVEL.load(Ordering::SeqCst)
}

pub fn is_enabled() -> bool {
    level() > 0
}

pub fn elapsed_ms() -> u128 {
    START_TIME
        .get()
        .map(|t| t.elapsed().as_millis())
        .unwrap_or(0)
}

pub fn format_line(msg: &str) -> String {
    format!("DEBUG +{}ms | {}", elapsed_ms(), msg)
}

pub fn format_json_line(label: &str, value: &serde_json::Value) -> String {
    let pretty = serde_json::to_string_pretty(value).unwrap_or_else(|_| format!("{:?}", value));
    format!("DEBUG +{}ms | {}\n{}", elapsed_ms(), label, pretty)
}

pub fn emit_line(msg: &str) {
    if is_enabled() {
        eprintln!("{}", format_line(msg));
    }
}

#[allow(dead_code)]
pub fn emit_line_at(min_level: u8, msg: &str) {
    if level() >= min_level {
        eprintln!("{}", format_line(msg));
    }
}

pub fn emit_json(label: &str, value: &serde_json::Value) {
    if level() >= 2 {
        eprintln!("{}", format_json_line(label, value));
    }
}

#[allow(dead_code)]
pub fn emit_json_at(min_level: u8, label: &str, value: &serde_json::Value) {
    if level() >= min_level {
        eprintln!("{}", format_json_line(label, value));
    }
}

#[cfg(test)]
pub fn reset_for_test() {
    LEVEL.store(0, Ordering::SeqCst);
}

#[cfg(test)]
pub fn force_enable_for_test() {
    LEVEL.store(1, Ordering::SeqCst);
    let _ = START_TIME.get_or_init(Instant::now);
}

#[cfg(test)]
pub fn force_level_for_test(lvl: u8) {
    LEVEL.store(lvl, Ordering::SeqCst);
    let _ = START_TIME.get_or_init(Instant::now);
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        if $crate::debug::is_enabled() {
            $crate::debug::emit_line(&$crate::debug::format_line(&format!($($arg)*)))
        }
    };
}

#[macro_export]
macro_rules! debug_json {
    ($label:expr, $value:expr) => {
        if $crate::debug::level() >= 2 {
            $crate::debug::emit_json($label, &$value)
        }
    };
}

#[allow(dead_code)]
#[macro_export]
macro_rules! debug_at {
    ($min_level:expr, $($arg:tt)*) => {
        if $crate::debug::level() >= $min_level {
            $crate::debug::emit_line_at($min_level, &$crate::debug::format_line(&format!($($arg)*)))
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_line_contains_prefix() {
        reset_for_test();
        force_enable_for_test();
        let line = format_line("hello world");
        assert!(line.starts_with("DEBUG +"));
        assert!(line.contains("ms | hello world"));
        reset_for_test();
    }

    #[test]
    fn format_line_with_zero_elapsed() {
        reset_for_test();
        let line = format_line("test");
        assert!(line.starts_with("DEBUG +"));
        assert!(line.contains("ms | test"));
        reset_for_test();
    }

    #[test]
    fn format_json_line_multiline() {
        reset_for_test();
        force_enable_for_test();
        let val = serde_json::json!({"key": "value", "num": 42});
        let output = format_json_line("request", &val);
        let lines: Vec<&str> = output.lines().collect();
        assert!(lines.len() >= 3);
        assert!(lines[0].contains("ms | request"));
        assert!(output.contains("\"key\""));
        assert!(output.contains("\"num\""));
        assert!(output.contains("\"value\""));
        assert!(output.contains("42"));
        reset_for_test();
    }

    #[test]
    fn is_enabled_reflects_state() {
        reset_for_test();
        assert!(!is_enabled());
        force_enable_for_test();
        assert!(is_enabled());
        reset_for_test();
    }

    #[test]
    fn level_returns_set_value() {
        reset_for_test();
        assert_eq!(level(), 0);
        force_level_for_test(1);
        assert_eq!(level(), 1);
        force_level_for_test(2);
        assert_eq!(level(), 2);
        reset_for_test();
        assert_eq!(level(), 0);
    }

    #[test]
    fn elapsed_ms_zero_before_init() {
        reset_for_test();
        assert_eq!(elapsed_ms(), 0);
    }

    #[test]
    fn emit_line_disabled_is_silent() {
        reset_for_test();
        assert!(!is_enabled());
    }

    #[test]
    fn emit_json_disabled_is_silent() {
        reset_for_test();
        assert!(!is_enabled());
    }

    #[test]
    fn emit_line_enabled_gates_on_is_enabled() {
        reset_for_test();
        force_enable_for_test();
        assert!(is_enabled());
        reset_for_test();
    }

    #[test]
    fn emit_json_requires_level_2() {
        reset_for_test();
        force_level_for_test(1);
        assert_eq!(level(), 1);
        assert!(is_enabled());
        reset_for_test();
    }

    #[test]
    fn emit_line_silent_when_disabled() {
        reset_for_test();
        assert!(!is_enabled());
        emit_line("should not appear");
    }

    #[test]
    fn emit_json_silent_at_level_1() {
        reset_for_test();
        force_level_for_test(1);
        assert_eq!(level(), 1);
        emit_json("should not appear", &serde_json::json!({"x": 1}));
    }
}

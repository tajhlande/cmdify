pub mod ask_user;
pub mod find_command;

use async_trait::async_trait;

use crate::error::{Error, Result};
use crate::logger::CmdifyLogger;
use crate::provider::ToolDefinition;
use crate::spinner::SpinnerPause;

pub use ask_user::AskUserTool;
pub use find_command::FindCommandTool;

#[derive(Debug)]
pub struct ToolOutput {
    pub content: String,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn definition(&self) -> ToolDefinition;
    async fn execute(
        &self,
        arguments: serde_json::Value,
        logger: Option<&CmdifyLogger>,
        spinner: Option<&SpinnerPause>,
    ) -> Result<ToolOutput>;
}

pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new(quiet: bool, blind: bool, no_tools: bool) -> Self {
        let mut tools: Vec<Box<dyn Tool>> = Vec::new();

        if !no_tools {
            if !quiet {
                tools.push(Box::new(AskUserTool::default()));
            }
            if !blind {
                tools.push(Box::new(FindCommandTool::default()));
            }
        }

        Self { tools }
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.iter().map(|t| t.definition()).collect()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub async fn execute(
        &self,
        name: &str,
        args: serde_json::Value,
        logger: Option<&CmdifyLogger>,
        spinner: Option<&SpinnerPause>,
    ) -> Result<ToolOutput> {
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| Error::ToolError(format!("unknown tool: {}", name)))?
            .execute(args, logger, spinner)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_with_both_tools() {
        let registry = ToolRegistry::new(false, false, false);
        let defs = registry.definitions();
        assert_eq!(defs.len(), 2);
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"ask_user"));
        assert!(names.contains(&"find_command"));
        assert!(!registry.is_empty());
    }

    #[test]
    fn registry_with_quiet_flag() {
        let registry = ToolRegistry::new(true, false, false);
        assert_eq!(registry.definitions().len(), 1);
        assert_eq!(registry.definitions()[0].name, "find_command");
    }

    #[test]
    fn registry_with_blind_flag() {
        let registry = ToolRegistry::new(false, true, false);
        assert_eq!(registry.definitions().len(), 1);
        assert_eq!(registry.definitions()[0].name, "ask_user");
    }

    #[test]
    fn registry_with_no_tools_flag() {
        let registry = ToolRegistry::new(false, false, true);
        assert!(registry.is_empty());
        assert!(registry.definitions().is_empty());
    }

    #[tokio::test]
    async fn execute_unknown_tool_errors() {
        let registry = ToolRegistry::new(false, false, false);
        let result = registry
            .execute("nonexistent", serde_json::json!({}), None, None)
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown tool: nonexistent"));
    }

    #[test]
    fn registry_all_flags_set() {
        let registry = ToolRegistry::new(true, true, true);
        assert!(registry.is_empty());
    }

    #[test]
    fn registry_quiet_and_blind() {
        let registry = ToolRegistry::new(true, true, false);
        assert!(registry.is_empty());
    }

    #[tokio::test]
    async fn execute_find_command_directly() {
        let registry = ToolRegistry::new(false, false, false);
        let result = registry
            .execute(
                "find_command",
                serde_json::json!({"command": "sh"}),
                None,
                None,
            )
            .await;
        assert!(result.is_ok());
    }

    #[test]
    fn ask_user_excluded_when_quiet() {
        let registry = ToolRegistry::new(true, false, false);
        let defs = registry.definitions();
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(!names.contains(&"ask_user"));
        assert!(names.contains(&"find_command"));
    }

    #[test]
    fn find_command_excluded_when_blind() {
        let registry = ToolRegistry::new(false, true, false);
        let defs = registry.definitions();
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"ask_user"));
        assert!(!names.contains(&"find_command"));
    }
}

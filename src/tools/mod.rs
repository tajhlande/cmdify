pub mod find_command;
// TODO(Phase 3): Add ask_user module for interactive clarification tool

use async_trait::async_trait;

use crate::error::{Error, Result};
use crate::logger::CmdifyLogger;
use crate::provider::ToolDefinition;

pub use find_command::FindCommandTool;

// Intentionally kept as a struct rather than a type alias so that future tools
// (e.g., ask_user in Phase 3) can add fields like metadata or timing without
// changing the return type signature across the Tool trait.
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
    ) -> Result<ToolOutput>;
}

pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new(blind: bool, no_tools: bool) -> Self {
        let mut tools: Vec<Box<dyn Tool>> = Vec::new();

        if !no_tools && !blind {
            tools.push(Box::new(FindCommandTool::default()));
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
    ) -> Result<ToolOutput> {
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .ok_or_else(|| Error::ToolError(format!("unknown tool: {}", name)))?
            .execute(args, logger)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_with_tools_enabled() {
        let registry = ToolRegistry::new(false, false);
        assert_eq!(registry.definitions().len(), 1);
        assert_eq!(registry.definitions()[0].name, "find_command");
        assert!(!registry.is_empty());
    }

    #[test]
    fn registry_with_blind_flag() {
        let registry = ToolRegistry::new(true, false);
        assert!(registry.is_empty());
        assert!(registry.definitions().is_empty());
    }

    #[test]
    fn registry_with_no_tools_flag() {
        let registry = ToolRegistry::new(false, true);
        assert!(registry.is_empty());
        assert!(registry.definitions().is_empty());
    }

    #[tokio::test]
    async fn execute_unknown_tool_errors() {
        let registry = ToolRegistry::new(false, false);
        let result = registry
            .execute("nonexistent", serde_json::json!({}), None)
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown tool: nonexistent"));
    }

    #[test]
    fn registry_both_flags_set() {
        let registry = ToolRegistry::new(true, true);
        assert!(registry.is_empty());
    }
}

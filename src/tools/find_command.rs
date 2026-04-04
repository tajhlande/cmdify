use async_trait::async_trait;
use serde_json::json;

use crate::error::{Error, Result};
use crate::logger::CmdifyLogger;
use crate::provider::ToolDefinition;

use super::{Tool, ToolOutput};

pub struct FindCommandTool;

const TIMEOUT_SECS: u64 = 5;

#[async_trait]
impl Tool for FindCommandTool {
    fn name(&self) -> &str {
        "find_command"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "find_command".into(),
            description: "Check whether a specific command-line tool is available on the system by running 'command -v' \
                         (with 'which' as fallback). \
                         Use this if you want to check for the presence of an optional tool which might not be installed, \
                         or if you want to find the full path to a command. \
                         Do NOT use this to run arbitrary commands or to execute user input. \
                         It is intended only for checking the existence of well-known command-line tools."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The command name to look up (e.g., 'fd', 'rg', 'jq')."
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
        logger: Option<&CmdifyLogger>,
    ) -> Result<ToolOutput> {
        let command = arguments
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::ToolError("missing 'command' argument".into()))?;

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(TIMEOUT_SECS),
            lookup_command(command, logger),
        )
        .await;

        match result {
            Ok(Ok(path)) => Ok(ToolOutput { content: path }),
            Ok(Err(_)) => Ok(ToolOutput {
                content: "not found".into(),
            }),
            Err(_) => Ok(ToolOutput {
                content: "error: command lookup timed out".into(),
            }),
        }
    }
}

async fn lookup_command(
    command: &str,
    logger: Option<&CmdifyLogger>,
) -> std::result::Result<String, ()> {
    if let Some(lg) = logger {
        lg.log("find_command", &format!("command -v {}", command));
    }

    let output = tokio::process::Command::new("sh")
        .arg("-c")
        .arg("command -v \"$1\"")
        .arg("--")
        .arg(command)
        .output()
        .await
        .map_err(|_| ())?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Ok(path);
        }
    }

    if let Some(lg) = logger {
        lg.log("find_command", &format!("which {}", command));
    }

    let output = tokio::process::Command::new("which")
        .arg(command)
        .output()
        .await
        .map_err(|_| ())?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Ok(path);
        }
    }

    Err(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool() -> FindCommandTool {
        FindCommandTool
    }

    #[test]
    fn tool_name() {
        assert_eq!(tool().name(), "find_command");
    }

    #[test]
    fn tool_definition_has_required_fields() {
        let def = tool().definition();
        assert_eq!(def.name, "find_command");
        assert!(!def.description.is_empty());
        let params = def.parameters.as_object().unwrap();
        let props = params.get("properties").unwrap().as_object().unwrap();
        assert!(props.contains_key("command"));
        let required = params.get("required").unwrap().as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0], "command");
    }

    #[tokio::test]
    async fn missing_command_argument() {
        let result = tool().execute(json!({}), None).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("missing 'command' argument"));
    }

    #[tokio::test]
    async fn find_existing_command_sh() {
        let result = tool().execute(json!({"command": "sh"}), None).await;
        assert!(result.is_ok());
        let output = result.unwrap().content;
        assert!(output.contains("sh"));
        assert_ne!(output, "not found");
    }

    #[tokio::test]
    async fn find_existing_command_ls() {
        let result = tool().execute(json!({"command": "ls"}), None).await;
        assert!(result.is_ok());
        let output = result.unwrap().content;
        assert_ne!(output, "not found");
    }

    #[tokio::test]
    async fn find_nonexistent_command() {
        let result = tool()
            .execute(json!({"command": "nonexistent_cmd_xyz_12345"}), None)
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().content, "not found");
    }

    #[tokio::test]
    async fn shell_injection_safety() {
        let result = tool()
            .execute(json!({"command": "ls; rm -rf /"}), None)
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().content, "not found");
    }

    #[tokio::test]
    async fn command_not_string_errors() {
        let result = tool().execute(json!({"command": 42}), None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn command_with_path() {
        let result = tool().execute(json!({"command": "/bin/sh"}), None).await;
        assert!(result.is_ok());
        let output = result.unwrap().content;
        assert_eq!(output, "/bin/sh");
    }
}

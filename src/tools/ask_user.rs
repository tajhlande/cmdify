use std::io::{BufRead, Write};

use async_trait::async_trait;
use serde_json::json;

use crate::debug;
use crate::error::{Error, Result};
use crate::logger::CmdifyLogger;
use crate::provider::ToolDefinition;
use crate::spinner::SpinnerPause;

use super::{Tool, ToolOutput};

const DEFAULT_TIMEOUT_SECS: u64 = 60;
const NO_RESPONSE_SENTINEL: &str = "(no response)";

pub struct AskUserTool {
    timeout_secs: u64,
}

impl Default for AskUserTool {
    fn default() -> Self {
        Self::new()
    }
}

impl AskUserTool {
    pub fn new() -> Self {
        Self {
            timeout_secs: DEFAULT_TIMEOUT_SECS,
        }
    }

    #[allow(dead_code)]
    pub fn with_timeout(secs: u64) -> Self {
        Self { timeout_secs: secs }
    }

    fn format_display(question: &str, choices: &[(String, String)]) -> String {
        let mut out = format!("[cmdify] {}\n", question);
        for (key, label) in choices {
            out.push_str(&format!("  {}) {}\n", key, label));
        }
        out.push_str("> Your choice: ");
        out
    }

    fn parse_choices(arguments: &serde_json::Value) -> Result<Vec<(String, String)>> {
        let choices_arr = arguments
            .get("choices")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::ToolError("missing 'choices' argument".into()))?;

        let mut choices = Vec::with_capacity(choices_arr.len());
        for (i, item) in choices_arr.iter().enumerate() {
            let key = item
                .get("key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    Error::ToolError(format!("choice at index {} missing 'key' field", i))
                })?
                .trim()
                .to_string();

            let label = item
                .get("label")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    Error::ToolError(format!("choice at index {} missing 'label' field", i))
                })?
                .to_string();

            if key.is_empty() {
                return Err(Error::ToolError(format!(
                    "choice at index {} has empty key",
                    i
                )));
            }

            choices.push((key, label));
        }

        if choices.is_empty() {
            return Err(Error::ToolError("choices array must not be empty".into()));
        }

        Ok(choices)
    }
}

#[async_trait]
impl Tool for AskUserTool {
    fn name(&self) -> &str {
        "ask_user"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ask_user".into(),
            description: "Ask the user a clarifying question when their request is ambiguous. \
                          Present choices as single-letter options so the user can reply with \
                          a single character. Use this when the user's intent is unclear and \
                          you need additional information to generate the correct command."
                .into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "question": {
                        "type": "string",
                        "description": "The clarifying question to ask the user."
                    },
                    "choices": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "key": {
                                    "type": "string",
                                    "description": "A single-letter key (e.g., 'A', 'B', 'Y', 'N')."
                                },
                                "label": {
                                    "type": "string",
                                    "description": "A short descriptive label for this choice (e.g., 'use fd', 'use find')."
                                }
                            },
                            "required": ["key", "label"]
                        },
                        "description": "List of choices, each with a single-letter key and a descriptive label."
                    }
                },
                "required": ["question", "choices"]
            }),
        }
    }

    async fn execute(
        &self,
        arguments: serde_json::Value,
        logger: Option<&CmdifyLogger>,
        spinner: Option<&SpinnerPause>,
    ) -> Result<ToolOutput> {
        let question = arguments
            .get("question")
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::ToolError("missing 'question' argument".into()))?;

        let choices = Self::parse_choices(&arguments)?;

        debug!("ask_user: prompting user with {} choices", choices.len());

        if let Some(lg) = logger {
            lg.log("ask_user", question);
        }

        if let Some(s) = spinner {
            s.pause();
        }

        let display = Self::format_display(question, &choices);

        let mut stderr = std::io::stderr();
        let _ = write!(stderr, "\r   \r");
        let _ = stderr.flush();
        eprint!("{}", display);

        let valid_keys: Vec<String> = choices.iter().map(|(k, _)| k.clone()).collect();

        let result = tokio::time::timeout(
            std::time::Duration::from_secs(self.timeout_secs),
            tokio::task::spawn_blocking(move || {
                let stdin = std::io::stdin();
                let mut reader = stdin.lock();
                read_user_choice(&mut reader, &valid_keys)
            }),
        )
        .await;

        if let Some(s) = spinner {
            s.resume();
        }

        match result {
            Ok(Ok(Ok(content))) => {
                debug!("ask_user: user chose '{}'", content);
                Ok(ToolOutput { content })
            }
            Ok(Ok(Err(e))) => {
                debug!("ask_user: stdin read error: {}", e);
                Ok(ToolOutput {
                    content: NO_RESPONSE_SENTINEL.into(),
                })
            }
            Ok(Err(_)) => {
                debug!("ask_user: spawn_blocking join error");
                Ok(ToolOutput {
                    content: NO_RESPONSE_SENTINEL.into(),
                })
            }
            Err(_) => {
                debug!("ask_user: timed out after {}s", self.timeout_secs);
                Ok(ToolOutput {
                    content: NO_RESPONSE_SENTINEL.into(),
                })
            }
        }
    }
}

pub(crate) fn read_user_choice<R: BufRead>(
    reader: &mut R,
    valid_keys: &[String],
) -> std::io::Result<String> {
    let mut input = String::new();
    reader.read_line(&mut input)?;
    let trimmed = input.trim();
    if let Some(matched) = valid_keys.iter().find(|k| k.eq_ignore_ascii_case(trimmed)) {
        return Ok(matched.clone());
    }
    if trimmed.is_empty() {
        return Ok(NO_RESPONSE_SENTINEL.into());
    }
    Ok(format!("{} (not a valid choice)", trimmed))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool() -> AskUserTool {
        AskUserTool::default()
    }

    #[test]
    fn tool_name() {
        assert_eq!(tool().name(), "ask_user");
    }

    #[test]
    fn tool_definition_has_required_fields() {
        let def = tool().definition();
        assert_eq!(def.name, "ask_user");
        assert!(!def.description.is_empty());
        let params = def.parameters.as_object().unwrap();
        let props = params.get("properties").unwrap().as_object().unwrap();
        assert!(props.contains_key("question"));
        assert!(props.contains_key("choices"));
        let required = params.get("required").unwrap().as_array().unwrap();
        assert!(required.contains(&json!("question")));
        assert!(required.contains(&json!("choices")));
    }

    #[test]
    fn tool_definition_choices_schema() {
        let def = tool().definition();
        let params = def.parameters.as_object().unwrap();
        let props = params.get("properties").unwrap().as_object().unwrap();
        let choices = props.get("choices").unwrap();
        let items = choices.get("items").unwrap();
        let item_props = items.get("properties").unwrap().as_object().unwrap();
        assert!(item_props.contains_key("key"));
        assert!(item_props.contains_key("label"));
        let item_required = items.get("required").unwrap().as_array().unwrap();
        assert!(item_required.contains(&json!("key")));
        assert!(item_required.contains(&json!("label")));
    }

    #[test]
    fn parse_choices_valid() {
        let args = json!({
            "question": "Which tool?",
            "choices": [
                {"key": "A", "label": "use fd"},
                {"key": "B", "label": "use find"}
            ]
        });
        let choices = AskUserTool::parse_choices(&args).unwrap();
        assert_eq!(choices.len(), 2);
        assert_eq!(choices[0], ("A".to_string(), "use fd".to_string()));
        assert_eq!(choices[1], ("B".to_string(), "use find".to_string()));
    }

    #[test]
    fn parse_choices_missing_choices_field() {
        let args = json!({"question": "Which tool?"});
        let result = AskUserTool::parse_choices(&args);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("missing 'choices'"));
    }

    #[test]
    fn parse_choices_empty_array() {
        let args = json!({"question": "Which?", "choices": []});
        let result = AskUserTool::parse_choices(&args);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must not be empty"));
    }

    #[test]
    fn parse_choices_missing_key() {
        let args = json!({
            "question": "Which?",
            "choices": [{"label": "use fd"}]
        });
        let result = AskUserTool::parse_choices(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing 'key'"));
    }

    #[test]
    fn parse_choices_missing_label() {
        let args = json!({
            "question": "Which?",
            "choices": [{"key": "A"}]
        });
        let result = AskUserTool::parse_choices(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing 'label'"));
    }

    #[test]
    fn parse_choices_empty_key() {
        let args = json!({
            "question": "Which?",
            "choices": [{"key": "  ", "label": "bad"}]
        });
        let result = AskUserTool::parse_choices(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty key"));
    }

    #[test]
    fn parse_choices_whitespace_key_trimmed() {
        let args = json!({
            "question": "Which?",
            "choices": [{"key": " A ", "label": "option A"}]
        });
        let choices = AskUserTool::parse_choices(&args).unwrap();
        assert_eq!(choices[0].0, "A");
    }

    #[tokio::test]
    async fn missing_question_argument() {
        let result = tool()
            .execute(
                json!({"choices": [{"key": "A", "label": "yes"}]}),
                None,
                None,
            )
            .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("missing 'question'"));
    }

    #[tokio::test]
    async fn missing_choices_argument() {
        let result = tool()
            .execute(json!({"question": "Which?"}), None, None)
            .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("missing 'choices'"));
    }

    #[test]
    fn format_display_output() {
        let choices = vec![
            ("A".to_string(), "use fd".to_string()),
            ("B".to_string(), "use find".to_string()),
        ];
        let display = AskUserTool::format_display("Use fd or find?", &choices);
        assert!(display.contains("[cmdify] Use fd or find?"));
        assert!(display.contains("  A) use fd"));
        assert!(display.contains("  B) use find"));
        assert!(display.contains("> Your choice:"));
    }

    #[test]
    fn format_display_single_choice() {
        let choices = vec![("Y".to_string(), "yes".to_string())];
        let display = AskUserTool::format_display("Continue?", &choices);
        assert!(display.contains("  Y) yes"));
    }

    #[test]
    fn read_choice_valid_uppercase() {
        let mut reader = std::io::Cursor::new("A\n");
        let keys = vec!["A".to_string(), "B".to_string()];
        let result = read_user_choice(&mut reader, &keys).unwrap();
        assert_eq!(result, "A");
    }

    #[test]
    fn read_choice_valid_lowercase() {
        let mut reader = std::io::Cursor::new("b\n");
        let keys = vec!["A".to_string(), "B".to_string()];
        let result = read_user_choice(&mut reader, &keys).unwrap();
        assert_eq!(result, "B");
    }

    #[test]
    fn read_choice_empty_returns_sentinel() {
        let mut reader = std::io::Cursor::new("\n");
        let keys = vec!["A".to_string(), "B".to_string()];
        let result = read_user_choice(&mut reader, &keys).unwrap();
        assert_eq!(result, NO_RESPONSE_SENTINEL);
    }

    #[test]
    fn read_choice_invalid_returns_note() {
        let mut reader = std::io::Cursor::new("X\n");
        let keys = vec!["A".to_string(), "B".to_string()];
        let result = read_user_choice(&mut reader, &keys).unwrap();
        assert_eq!(result, "X (not a valid choice)");
    }

    #[test]
    fn read_choice_whitespace_only_returns_sentinel() {
        let mut reader = std::io::Cursor::new("   \n");
        let keys = vec!["A".to_string(), "B".to_string()];
        let result = read_user_choice(&mut reader, &keys).unwrap();
        assert_eq!(result, NO_RESPONSE_SENTINEL);
    }

    #[test]
    fn read_choice_with_trailing_whitespace() {
        let mut reader = std::io::Cursor::new("A  \n");
        let keys = vec!["A".to_string(), "B".to_string()];
        let result = read_user_choice(&mut reader, &keys).unwrap();
        assert_eq!(result, "A");
    }

    #[test]
    fn read_choice_single_key() {
        let mut reader = std::io::Cursor::new("Y\n");
        let keys = vec!["Y".to_string()];
        let result = read_user_choice(&mut reader, &keys).unwrap();
        assert_eq!(result, "Y");
    }
}

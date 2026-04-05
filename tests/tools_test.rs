use cmdify::tools::{ask_user::AskUserTool, find_command::FindCommandTool, Tool, ToolRegistry};

fn make_find_tool() -> FindCommandTool {
    FindCommandTool::default()
}

#[tokio::test]
async fn find_command_sh() {
    let tool = make_find_tool();
    let result = tool
        .execute(serde_json::json!({"command": "sh"}), None, None)
        .await
        .unwrap();
    assert!(!result.content.is_empty());
    assert_ne!(result.content, "not found");
}

#[tokio::test]
async fn find_command_ls() {
    let tool = make_find_tool();
    let result = tool
        .execute(serde_json::json!({"command": "ls"}), None, None)
        .await
        .unwrap();
    assert!(!result.content.is_empty());
    assert_ne!(result.content, "not found");
}

#[tokio::test]
async fn find_command_cat() {
    let tool = make_find_tool();
    let result = tool
        .execute(serde_json::json!({"command": "cat"}), None, None)
        .await
        .unwrap();
    assert!(!result.content.is_empty());
    assert_ne!(result.content, "not found");
}

#[tokio::test]
async fn find_command_nonexistent() {
    let tool = make_find_tool();
    let result = tool
        .execute(
            serde_json::json!({"command": "nonexistent_cmd_integration_test_abc123"}),
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(result.content, "not found");
}

#[tokio::test]
async fn ask_user_missing_question_errors() {
    let tool = AskUserTool::default();
    let result = tool
        .execute(
            serde_json::json!({"choices": [{"key": "A", "label": "yes"}]}),
            None,
            None,
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn ask_user_missing_choices_errors() {
    let tool = AskUserTool::default();
    let result = tool
        .execute(serde_json::json!({"question": "Pick one?"}), None, None)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn ask_user_empty_choices_errors() {
    let tool = AskUserTool::default();
    let result = tool
        .execute(
            serde_json::json!({"question": "Pick one?", "choices": []}),
            None,
            None,
        )
        .await;
    assert!(result.is_err());
}

#[test]
fn registry_both_tools_by_default() {
    let registry = ToolRegistry::new(false, false, false);
    let defs = registry.definitions();
    assert_eq!(defs.len(), 2);
    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"ask_user"));
    assert!(names.contains(&"find_command"));
}

#[test]
fn registry_quiet_excludes_ask_user() {
    let registry = ToolRegistry::new(true, false, false);
    let defs = registry.definitions();
    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    assert!(!names.contains(&"ask_user"));
    assert!(names.contains(&"find_command"));
}

#[test]
fn registry_blind_excludes_find_command() {
    let registry = ToolRegistry::new(false, true, false);
    let defs = registry.definitions();
    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"ask_user"));
    assert!(!names.contains(&"find_command"));
}

#[test]
fn registry_no_tools_is_empty() {
    let registry = ToolRegistry::new(false, false, true);
    assert!(registry.definitions().is_empty());
}

#[test]
fn registry_quiet_and_blind_is_empty() {
    let registry = ToolRegistry::new(true, true, false);
    assert!(registry.definitions().is_empty());
}

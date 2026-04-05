use cmdify::tools::{find_command::FindCommandTool, Tool};

fn make_tool() -> FindCommandTool {
    FindCommandTool::default()
}

#[tokio::test]
async fn find_command_sh() {
    let tool = make_tool();
    let result = tool
        .execute(serde_json::json!({"command": "sh"}), None)
        .await
        .unwrap();
    assert!(!result.content.is_empty());
    assert_ne!(result.content, "not found");
}

#[tokio::test]
async fn find_command_ls() {
    let tool = make_tool();
    let result = tool
        .execute(serde_json::json!({"command": "ls"}), None)
        .await
        .unwrap();
    assert!(!result.content.is_empty());
    assert_ne!(result.content, "not found");
}

#[tokio::test]
async fn find_command_cat() {
    let tool = make_tool();
    let result = tool
        .execute(serde_json::json!({"command": "cat"}), None)
        .await
        .unwrap();
    assert!(!result.content.is_empty());
    assert_ne!(result.content, "not found");
}

#[tokio::test]
async fn find_command_nonexistent() {
    let tool = make_tool();
    let result = tool
        .execute(
            serde_json::json!({"command": "nonexistent_cmd_integration_test_abc123"}),
            None,
        )
        .await
        .unwrap();
    assert_eq!(result.content, "not found");
}

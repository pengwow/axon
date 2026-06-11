//! TDD 第三轮：Tool trait + ToolError
//!
//! Tool 是 LLM 可调用的功能单元。必须支持 JSON Schema 声明参数、
//! 异步执行、并通过 `ToolDefinition` 暴露给 LLM 协议。

use std::collections::HashMap;
use std::sync::Arc;

use axon_llm::backend::ToolDefinition;
use axon_llm::tools::{Tool, ToolError};

// ─── 简单 Tool 实现：返回静态结果 ──────────────────────────────

struct EchoTool;

#[async_trait::async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &'static str {
        "echo"
    }
    fn description(&self) -> &'static str {
        "回显输入参数"
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "text": {"type": "string"}
            },
            "required": ["text"]
        })
    }
    async fn execute(&self, arguments: &str) -> Result<String, ToolError> {
        // 验证 arguments 必须是合法 JSON
        let v: serde_json::Value =
            serde_json::from_str(arguments).map_err(|e| ToolError::InvalidArguments(e.to_string()))?;
        let text = v["text"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArguments("缺少 text 字段".into()))?;
        Ok(text.to_string())
    }
}

// ─── Tool trait 基础行为 ─────────────────────────────────────

#[test]
fn test_tool_name_description_schema() {
    let t = EchoTool;
    assert_eq!(t.name(), "echo");
    assert!(!t.description().is_empty());
    assert_eq!(t.parameters_schema()["type"], "object");
}

/// definition() 默认实现必须组合 name + description + schema
#[test]
fn test_tool_definition_composes_name_desc_schema() {
    let t = EchoTool;
    let def = t.definition();
    assert_eq!(def.name, "echo");
    assert_eq!(def.description, t.description());
    assert_eq!(def.parameters, t.parameters_schema());
}

// ─── Tool 执行：成功路径 ─────────────────────────────────────

#[tokio::test]
async fn test_tool_execute_success() {
    let t = EchoTool;
    let result = t.execute(r#"{"text":"hello"}"#).await.unwrap();
    assert_eq!(result, "hello");
}

// ─── Tool 执行：错误路径 ─────────────────────────────────────

#[tokio::test]
async fn test_tool_execute_rejects_invalid_json() {
    let t = EchoTool;
    let err = t.execute("not json").await.unwrap_err();
    assert!(matches!(err, ToolError::InvalidArguments(_)));
}

#[tokio::test]
async fn test_tool_execute_rejects_missing_required_field() {
    let t = EchoTool;
    let err = t.execute(r#"{}"#).await.unwrap_err();
    assert!(matches!(err, ToolError::InvalidArguments(_)));
}

// ─── Tool 可作为 trait object（多态注册到 Agent） ──────────────

#[tokio::test]
async fn test_tool_works_through_dyn_dispatch() {
    let tool: Box<dyn Tool> = Box::new(EchoTool);
    let result = tool.execute(r#"{"text":"via dyn"}"#).await.unwrap();
    assert_eq!(result, "via dyn");
    // 仍可获取 definition
    let def: ToolDefinition = tool.definition();
    assert_eq!(def.name, "echo");
}

// ─── 工具注册表：使用 HashMap 模拟 Agent 的工具池 ─────────────

#[tokio::test]
async fn test_tool_registry_lookups_by_name() {
    let mut registry: HashMap<String, Arc<dyn Tool>> = HashMap::new();
    registry.insert("echo".to_string(), Arc::new(EchoTool));

    let tool = registry.get("echo").expect("echo 应已注册");
    let result = tool.execute(r#"{"text":"x"}"#).await.unwrap();
    assert_eq!(result, "x");

    assert!(registry.get("nonexistent").is_none());
}

// ─── ToolError 错误分类 ──────────────────────────────────────

#[test]
fn test_tool_error_invalid_arguments_displays_message() {
    let err = ToolError::InvalidArguments("bad".into());
    let msg = err.to_string();
    assert!(msg.contains("bad"), "错误信息应包含原始消息: {}", msg);
}

#[test]
fn test_tool_error_execution_failed_displays_message() {
    let err = ToolError::ExecutionFailed("timeout".into());
    let msg = err.to_string();
    assert!(msg.contains("timeout"));
}

#[test]
fn test_tool_error_permission_denied_includes_tool_and_operation() {
    let err = ToolError::PermissionDenied {
        tool: "submit_order".into(),
        operation: "submit_order".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("submit_order"));
}

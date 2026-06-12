//! QueryExplanationTool 单元测试
//!
//! 覆盖：name/description/schema / 存在 ID / 不存在 ID / 无效 JSON / 缺字段 / timeout

#![cfg(feature = "explain")]

use std::sync::Arc;
use std::time::Duration;

use axon_explain::types::{ActionSnapshot, AttentionWeights, CounterfactualExplanation, Explanation};
use axon_llm::explain::{ExplanationStore, QueryExplanationTool};
use axon_llm::tools::{Tool, ToolError};

fn sample_explanation(id: &str) -> Explanation {
    Explanation {
        id: id.to_string(),
        observation_id: "obs".to_string(),
        action: ActionSnapshot {
            position_size: 1.0,
            entry_price: 100.0,
            stop_loss: 90.0,
            take_profit: 120.0,
            order_type: "limit".to_string(),
        },
        feature_importance: Default::default(),
        action_attributions: vec![],
        attention_weights: None,
        counterfactuals: vec![],
        summary: format!("explanation for {}", id),
        confidence: 0.9,
        generated_at: chrono::Utc::now(),
    }
}

fn make_tool() -> (Arc<ExplanationStore>, QueryExplanationTool) {
    let store = Arc::new(ExplanationStore::new(100));
    let tool = QueryExplanationTool::new(Arc::clone(&store));
    (store, tool)
}

#[tokio::test]
async fn test_tool_name_and_description() {
    let (_, tool) = make_tool();
    assert_eq!(tool.name(), "query_explanation");
    assert!(!tool.description().is_empty());
    assert!(tool.parameters_schema().is_object());
    // schema 应要求 decision_id
    let schema = tool.parameters_schema();
    assert_eq!(schema["required"][0], "decision_id");
}

#[tokio::test]
async fn test_tool_query_existing_id_returns_json() {
    let (store, tool) = make_tool();
    store.insert("d1".to_string(), sample_explanation("d1")).await;

    let args = r#"{"decision_id":"d1"}"#;
    let result = tool.execute(args).await;
    assert!(result.is_ok(), "实际错误: {:?}", result.err());
    let json = result.unwrap();
    // Explanation 序列化后应包含 summary 字段
    assert!(json.contains("explanation for d1"));
}

#[tokio::test]
async fn test_tool_query_nonexistent_id_returns_invalid_arguments() {
    let (_, tool) = make_tool();
    let args = r#"{"decision_id":"nope"}"#;
    let result = tool.execute(args).await;
    assert!(result.is_err());
    match result {
        Err(ToolError::InvalidArguments(msg)) => {
            assert!(msg.contains("nope"), "错误消息应包含 ID: {}", msg);
        }
        other => panic!("期望 InvalidArguments，实际 {:?}", other),
    }
}

#[tokio::test]
async fn test_tool_invalid_json_returns_error() {
    let (_, tool) = make_tool();
    let result = tool.execute("not json").await;
    assert!(result.is_err());
    match result {
        Err(ToolError::InvalidArguments(_)) => {}
        other => panic!("期望 InvalidArguments，实际 {:?}", other),
    }
}

#[tokio::test]
async fn test_tool_missing_decision_id_returns_error() {
    let (_, tool) = make_tool();
    let result = tool.execute("{}").await;
    assert!(result.is_err());
    match result {
        Err(ToolError::InvalidArguments(_)) => {}
        other => panic!("期望 InvalidArguments，实际 {:?}", other),
    }
}

#[tokio::test]
async fn test_tool_default_timeout_is_100ms() {
    let (_, tool) = make_tool();
    assert_eq!(tool.timeout().as_millis(), 100);
}

#[tokio::test]
async fn test_tool_with_timeout_overrides() {
    let store = Arc::new(ExplanationStore::new(100));
    let tool = QueryExplanationTool::new(store).with_timeout(Duration::from_millis(250));
    assert_eq!(tool.timeout().as_millis(), 250);
}

// 抑制 unused warning
#[allow(dead_code)]
fn _unused(_: AttentionWeights, _: CounterfactualExplanation) {}

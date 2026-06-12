//! DecisionRecord + ExplainMode 单元测试

#![cfg(feature = "explain")]

use axon_explain::types::ActionSnapshot;
use axon_llm::explain::{DecisionRecord, ExplainMode};
use axon_llm::react_agent::ReasoningStep;

fn sample_action() -> ActionSnapshot {
    ActionSnapshot {
        position_size: 1.0,
        entry_price: 50000.0,
        stop_loss: 48000.0,
        take_profit: 55000.0,
        order_type: "limit".to_string(),
    }
}

#[test]
fn test_decision_record_construction() {
    let record = DecisionRecord {
        decision_id: "abc-123".to_string(),
        timestamp: 1000,
        mode: ExplainMode::WithReasoning,
        query: "Should I buy BTC?".to_string(),
        reasoning_trace: vec![],
        final_action: sample_action(),
    };
    assert_eq!(record.decision_id, "abc-123");
    assert_eq!(record.timestamp, 1000);
    assert_eq!(record.mode, ExplainMode::WithReasoning);
}

#[test]
fn test_explain_mode_equality() {
    assert_eq!(ExplainMode::ActionOnly, ExplainMode::ActionOnly);
    assert_eq!(ExplainMode::WithReasoning, ExplainMode::WithReasoning);
    assert_ne!(ExplainMode::ActionOnly, ExplainMode::WithReasoning);
}

#[test]
fn test_explain_mode_serialization() {
    let mode = ExplainMode::WithReasoning;
    let json = serde_json::to_string(&mode).unwrap();
    assert!(json.contains("WithReasoning"));
}

#[test]
fn test_decision_record_with_reasoning_steps() {
    let step = ReasoningStep {
        step: 0,
        thought: "分析市场".to_string(),
        action: None,
        observation: None,
    };
    let record = DecisionRecord {
        decision_id: "d1".to_string(),
        timestamp: 100,
        mode: ExplainMode::WithReasoning,
        query: "test".to_string(),
        reasoning_trace: vec![step],
        final_action: sample_action(),
    };
    assert_eq!(record.reasoning_trace.len(), 1);
    assert_eq!(record.reasoning_trace[0].thought, "分析市场");
}

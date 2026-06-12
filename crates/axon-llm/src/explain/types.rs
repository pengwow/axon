//! 解释系统的核心数据类型
//!
//! - [`DecisionRecord`]：待解释的决策记录
//! - [`ExplainMode`]：解释模式（仅动作 / 含推理）
//!
//! ## 设计要点
//!
//! - **不 derive `PartialEq` / `Eq`**：`ReasoningStep` 未实现这两个 trait，
//!   会传染到 `DecisionRecord`。测试断言采用逐字段比较。
//! - **`ExplainMode` 默认 `ActionOnly`**：覆盖常见轻量场景；含推理需显式选择。
//! - **构造器自动填 `timestamp`**：调用方无需关心时间细节。

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::react_agent::ReasoningStep;
use axon_explain::types::ActionSnapshot;

/// 解释模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExplainMode {
    /// 仅解释最终动作（轻量、延迟低）
    ActionOnly,
    /// 解释完整推理链（重、延迟高）
    WithReasoning,
}

impl Default for ExplainMode {
    /// 默认走轻量路径：`ActionOnly`
    fn default() -> Self {
        Self::ActionOnly
    }
}

impl ExplainMode {
    /// 序列化为协议字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ActionOnly => "ActionOnly",
            Self::WithReasoning => "WithReasoning",
        }
    }
}

/// 待解释的决策记录
///
/// **不 derive `Serialize` / `Deserialize`**：内部 `reasoning_trace` 引用
/// [`ReasoningStep`]，而该类型当前未实现 serde 派生。如果未来需要持久化
/// `DecisionRecord`，应先为 `ReasoningStep` 加 `Serialize/Deserialize` 派生。
#[derive(Debug, Clone)]
pub struct DecisionRecord {
    /// 决策 ID（UUID v4 字符串）
    pub decision_id: String,
    /// 时间戳（Unix 秒）
    pub timestamp: u64,
    /// 解释模式
    pub mode: ExplainMode,
    /// 用户原始查询
    pub query: String,
    /// 推理链
    pub reasoning_trace: Vec<ReasoningStep>,
    /// 最终动作快照
    pub final_action: ActionSnapshot,
}

impl DecisionRecord {
    /// 构造决策记录（自动填 `timestamp` 为当前时间）
    ///
    /// `reasoning_trace` 默认为空；如需携带推理链请用 [`with_reasoning`](Self::with_reasoning)。
    pub fn new(
        decision_id: impl Into<String>,
        mode: ExplainMode,
        query: impl Into<String>,
        final_action: ActionSnapshot,
    ) -> Self {
        Self {
            decision_id: decision_id.into(),
            timestamp: current_unix_secs(),
            mode,
            query: query.into(),
            reasoning_trace: Vec::new(),
            final_action,
        }
    }

    /// 构造带推理链的决策记录（`mode = WithReasoning`）
    pub fn with_reasoning(
        decision_id: impl Into<String>,
        query: impl Into<String>,
        reasoning_trace: Vec<ReasoningStep>,
        final_action: ActionSnapshot,
    ) -> Self {
        Self {
            decision_id: decision_id.into(),
            timestamp: current_unix_secs(),
            mode: ExplainMode::WithReasoning,
            query: query.into(),
            reasoning_trace,
            final_action,
        }
    }
}

/// 当前 Unix 秒（容错：失败时回退 0）
fn current_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn explain_mode_default_is_action_only() {
        assert_eq!(ExplainMode::default(), ExplainMode::ActionOnly);
    }

    #[test]
    fn explain_mode_as_str_matches_variant() {
        assert_eq!(ExplainMode::ActionOnly.as_str(), "ActionOnly");
        assert_eq!(ExplainMode::WithReasoning.as_str(), "WithReasoning");
    }

    #[test]
    fn new_fills_timestamp_and_empty_trace() {
        let record = DecisionRecord::new(
            "d1",
            ExplainMode::ActionOnly,
            "buy BTC?",
            sample_action(),
        );
        assert_eq!(record.decision_id, "d1");
        assert_eq!(record.mode, ExplainMode::ActionOnly);
        assert_eq!(record.query, "buy BTC?");
        assert!(record.reasoning_trace.is_empty());
        assert!(record.timestamp > 0);
    }

    #[test]
    fn with_reasoning_uses_with_reasoning_mode() {
        let step = ReasoningStep {
            step: 0,
            thought: "分析市场".to_string(),
            action: None,
            observation: None,
        };
        let record = DecisionRecord::with_reasoning(
            "d2",
            "test",
            vec![step],
            sample_action(),
        );
        assert_eq!(record.mode, ExplainMode::WithReasoning);
        assert_eq!(record.reasoning_trace.len(), 1);
    }
}

//! DecisionRecorder
//!
//! fire-and-forget 异步记录器：调用 `record()` 立即返回，内部 `tokio::spawn` 跑
//! [`ExplainerBridge::explain_async`]。失败由 Bridge 内部 `tracing::warn!` 吞掉，
//! 不向调用方传播。
//!
//! ## 设计要点
//!
//! - **不返回 Future**：调用方不阻塞、不需要 await。这是 ReAct 主循环集成的关键。
//! - **暴露 `bridge_clone()`**：让 `ReActAgent::with_explainer` 能给 Compute Tool
//!   共享同一个 `ExplainerBridge`（避免重复创建 Explainer 实例）。
//! - **错误吞掉而非上抛**：异步记录不应污染 ReAct 主流程。调用方若需要同步错误
//!   处理，应直接用 `bridge.explain_async(...).await`。

use std::sync::Arc;

use crate::explain::bridge::ExplainerBridge;
use crate::explain::types::DecisionRecord;

/// 决策记录器
pub struct DecisionRecorder {
    bridge: Arc<ExplainerBridge>,
}

impl DecisionRecorder {
    /// 构造
    pub fn new(bridge: Arc<ExplainerBridge>) -> Self {
        Self { bridge }
    }

    /// 获取内部 bridge 的 Arc 克隆（供 ReActAgent 构造 Compute Tool 用）
    pub fn bridge_clone(&self) -> Arc<ExplainerBridge> {
        Arc::clone(&self.bridge)
    }

    /// 同步触发（**不阻塞**）：spawn 一个 tokio 任务跑 `bridge.explain_async`
    ///
    /// 失败由 Bridge 内部 `tracing::warn!` 记录，不会传播给调用方。
    pub fn record(&self, record: DecisionRecord) {
        let bridge = Arc::clone(&self.bridge);
        tokio::spawn(async move {
            // 显式丢弃结果：失败已由 Bridge 内部 warn 处理
            let _ = bridge.explain_async(record).await;
        });
    }
}

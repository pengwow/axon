//! ExplainerBridge
//!
//! 包装同步 `Explainer` 为异步接口。内部用 `tokio::task::spawn_blocking` 跑同步 SHAP，
//! 避免阻塞 tokio worker。失败时 `tracing::warn!` 记录，不写入 store。
//!
//! ## 设计要点
//!
//! - **`spawn_blocking` 是必须的**：axon-explain 的 `Explainer::explain` 是同步阻塞调用，
//!   若直接在 async 上下文中调用会阻塞 tokio worker 线程。
//! - **JoinError 映射到 `ModelNotLoaded`**：axon-explain 当前没有 `Internal` / `JoinFailed`
//!   变体,`ModelNotLoaded` 是最接近的("模型调用失败")。消息明确写 "task join failed"
//!   区分于"模型未加载"语义。如未来 axon-explain 引入新变体,这里同步切换。
//! - **业务错误上抛**：explainer 算法失败应反馈给同步调用方(如 `ComputeExplanationTool`)。
//!   fire-and-forget 调用方(`DecisionRecorder::record_async`)自行吞掉。
//! - **不重试**：Bridge 是 fire-and-forget 的薄包装，重试由调用方决定。
//! - **observation 简化**：Phase 3 仅用 `query_length` 和 `query_word_count` 两个特征。
//!   Phase 4 可让 ReActAgent 注入更丰富的 observation 字段。

use std::collections::HashMap;
use std::sync::Arc;

use tracing::warn;

use axon_explain::error::ExplainabilityError;
use axon_explain::traits::Explainer;

use crate::explain::store::ExplanationStore;
use crate::explain::types::DecisionRecord;

/// 同步 Explainer → 异步桥接
pub struct ExplainerBridge {
    inner: Arc<dyn Explainer>,
    store: Arc<ExplanationStore>,
}

impl ExplainerBridge {
    /// 构造桥接器
    pub fn new(inner: Arc<dyn Explainer>, store: Arc<ExplanationStore>) -> Self {
        Self { inner, store }
    }

    /// 异步执行 explain；成功写入 store，失败仅 warn
    ///
    /// 立即返回的语义：
    /// - spawn_blocking 把同步 `inner.explain(...)` 放到 blocking thread pool
    /// - await 完成后根据结果写入 store 或记 warn
    pub async fn explain_async(&self, record: DecisionRecord) -> Result<(), ExplainabilityError> {
        let decision_id = record.decision_id.clone();

        // 构造 observation（Phase 3 简化：仅 query 长度/词数）
        let observation = build_observation(&record.query);
        let action = record.final_action.clone();
        let inner = Arc::clone(&self.inner);

        // 在 blocking thread pool 跑同步 explain
        let explain_result =
            tokio::task::spawn_blocking(move || inner.explain(&observation, &action))
                .await
                .map_err(|join_err| {
                    // 任务被取消或 panic。语义上不是"模型未加载",但 axon-explain
                    // 当前没有更合适的变体,复用 ModelNotLoaded 并用消息区分。
                    ExplainabilityError::ModelNotLoaded(format!(
                        "explainer task join failed (panic or cancel): {}",
                        join_err
                    ))
                })?;

        match explain_result {
            Ok(explanation) => {
                self.store.insert(decision_id, explanation).await;
                Ok(())
            }
            Err(e) => {
                warn!(
                    decision_id = %decision_id,
                    error = %e,
                    "Explainer 计算失败，不写入 store"
                );
                Err(e)
            }
        }
    }
}

/// 从 query 提取简化 observation
fn build_observation(query: &str) -> HashMap<String, f64> {
    let mut obs = HashMap::with_capacity(2);
    obs.insert("query_length".to_string(), query.len() as f64);
    obs.insert(
        "query_word_count".to_string(),
        query.split_whitespace().count() as f64,
    );
    obs
}

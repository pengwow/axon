//! 两个 Tool：`QueryExplanationTool` + `ComputeExplanationTool`
//!
//! - `QueryExplanationTool`：查询 store 中已存解释
//! - `ComputeExplanationTool`：现场调用 Explainer 计算解释（带超时降级）

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;
use tracing::warn;

use crate::explain::bridge::ExplainerBridge;
use crate::explain::store::ExplanationStore;
use crate::explain::types::{DecisionRecord, ExplainMode};
use crate::react_agent::ReasoningStep;
use crate::tools::{Tool, ToolError};

// ─── QueryExplanationTool ─────────────────────────────────

/// 查询已存解释（同步 Tool，< 100ms 预算）
pub struct QueryExplanationTool {
    store: Arc<ExplanationStore>,
    timeout: Duration,
}

impl QueryExplanationTool {
    /// 默认 timeout 100ms
    pub fn new(store: Arc<ExplanationStore>) -> Self {
        Self {
            store,
            timeout: Duration::from_millis(DEFAULT_QUERY_TIMEOUT_MS),
        }
    }

    /// 自定义 timeout（builder 风格）
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 当前 timeout
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

#[async_trait]
impl Tool for QueryExplanationTool {
    fn name(&self) -> &str {
        "query_explanation"
    }

    fn description(&self) -> &str {
        "查询已生成的决策解释（按 decision_id）"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "decision_id": {
                    "type": "string",
                    "description": "决策 ID（UUID v4 字符串）"
                }
            },
            "required": ["decision_id"]
        })
    }

    async fn execute(&self, arguments: &str) -> Result<String, ToolError> {
        let args: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(format!("JSON 解析失败: {}", e)))?;

        let decision_id = args
            .get("decision_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArguments("缺少 decision_id 字段".to_string()))?
            .to_string();

        let store = Arc::clone(&self.store);
        let id = decision_id.clone();
        let query = async move { store.get(&id).await };

        match tokio::time::timeout(self.timeout, query).await {
            Ok(Some(exp)) => serde_json::to_string(&exp)
                .map_err(|e| ToolError::ExecutionFailed(format!("序列化失败: {}", e))),
            Ok(None) => Err(ToolError::InvalidArguments(format!(
                "decision_id '{}' 不存在",
                decision_id
            ))),
            Err(_) => Err(ToolError::ExecutionFailed(format!(
                "store 查询超时 ({}ms)",
                self.timeout.as_millis()
            ))),
        }
    }
}

// ─── ComputeExplanationTool ──────────────────────────────

/// Compute tool 输入
#[derive(Debug, Clone, Deserialize)]
pub struct ComputeInput {
    /// 用户查询
    pub query: String,
    /// 最终动作快照
    pub final_action: axon_explain::types::ActionSnapshot,
    /// 推理链（可选；`WithReasoning` 模式时使用）
    #[serde(default)]
    pub reasoning_trace: Vec<ReasoningStep>,
    /// 解释模式（序列化为 "ActionOnly" / "WithReasoning"）
    pub mode: ExplainMode,
    /// 可选决策 ID（`None` 时自动生成 UUID v4）
    ///
    /// 调用方传入可提前持有 ID,方便后续用 `query_explanation` 查回。
    #[serde(default)]
    pub decision_id: Option<String>,
}

/// Compute 工具默认 timeout（500ms）
///
/// **比 Query 工具（100ms）大一个量级**：内部会 `spawn_blocking` 跑 SHAP，
/// 单次 KernelSHAP 在 50 维特征上实测 50~500ms。100ms 几乎必然超时降级。
pub const DEFAULT_COMPUTE_TIMEOUT_MS: u64 = 500;

/// Query 工具默认 timeout（100ms，纯内存读，无需等待）
pub const DEFAULT_QUERY_TIMEOUT_MS: u64 = 100;

/// 现场计算解释（带同步预算 + 超时降级）
pub struct ComputeExplanationTool {
    bridge: Arc<ExplainerBridge>,
    store: Arc<ExplanationStore>,
    timeout: Duration,
}

impl ComputeExplanationTool {
    /// 构造（默认 500ms timeout，适配 SHAP 计算）
    pub fn new(bridge: Arc<ExplainerBridge>, store: Arc<ExplanationStore>) -> Self {
        Self {
            bridge,
            store,
            timeout: Duration::from_millis(DEFAULT_COMPUTE_TIMEOUT_MS),
        }
    }

    /// 自定义 timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 当前 timeout
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

#[async_trait]
impl Tool for ComputeExplanationTool {
    fn name(&self) -> &str {
        "compute_explanation"
    }

    fn description(&self) -> &str {
        "现场计算决策解释（无需预存决策，传入 query + final_action 即时计算）"
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "用户原始查询"
                },
                "final_action": {
                    "type": "object",
                    "description": "最终动作快照",
                    "properties": {
                        "position_size": {"type": "number"},
                        "entry_price": {"type": "number"},
                        "stop_loss": {"type": "number"},
                        "take_profit": {"type": "number"},
                        "order_type": {"type": "string"}
                    },
                    "required": ["position_size", "entry_price", "stop_loss", "take_profit", "order_type"]
                },
                "reasoning_trace": {
                    "type": "array",
                    "description": "推理步骤（WithReasoning 模式时使用）"
                },
                "mode": {
                    "type": "string",
                    "enum": ["ActionOnly", "WithReasoning"],
                    "description": "解释模式"
                },
                "decision_id": {
                    "type": "string",
                    "description": "可选决策 ID（不提供则自动生成 UUID v4）。传入后可凭此 ID 用 query_explanation 查回"
                }
            },
            "required": ["query", "final_action", "mode"]
        })
    }

    async fn execute(&self, arguments: &str) -> Result<String, ToolError> {
        let input: ComputeInput = serde_json::from_str(arguments)
            .map_err(|e| ToolError::InvalidArguments(format!("JSON 解析失败: {}", e)))?;

        // 决策 ID: 调用方提供则用,否则自动生成 UUID v4
        let decision_id = input
            .decision_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // 用构造器而不是手工 struct literal（避免与 current_unix_secs 逻辑重复）
        let mut record = DecisionRecord::new(
            decision_id.clone(),
            input.mode,
            input.query,
            input.final_action,
        );
        // 若提供了推理链,回填到 record（mode 已由调用方决定,不再覆盖）
        if !input.reasoning_trace.is_empty() {
            record.reasoning_trace = input.reasoning_trace;
        }

        // 带 timeout 异步计算
        let bridge = Arc::clone(&self.bridge);
        let compute = async move { bridge.explain_async(record).await };

        match tokio::time::timeout(self.timeout, compute).await {
            Ok(Ok(())) => {
                // 成功：从 store 读 Explanation 并返回
                self.store
                    .get(&decision_id)
                    .await
                    .ok_or_else(|| {
                        ToolError::ExecutionFailed("解释未写入 store（一致性错误）".to_string())
                    })
                    .and_then(|exp| {
                        serde_json::to_string(&exp)
                            .map_err(|e| ToolError::ExecutionFailed(format!("序列化失败: {}", e)))
                    })
            }
            Ok(Err(e)) => {
                warn!(error = %e, "ComputeExplanation: explainer 业务错误");
                Err(ToolError::ExecutionFailed(e.to_string()))
            }
            Err(_) => {
                // 超时降级：返回 partial JSON（不是错误，让 LLM 拿到降级信息）
                Ok(partial_explanation_json())
            }
        }
    }
}

/// 简化版 fallback（超时）
fn partial_explanation_json() -> String {
    json!({
        "id": "partial",
        "observation_id": "partial",
        "action": null,
        "feature_importance": {},
        "action_attributions": [],
        "attention_weights": null,
        "counterfactuals": [],
        "summary": "timeout — partial explanation (top features only)",
        "confidence": 0.0,
        "generated_at": chrono::Utc::now()
    })
    .to_string()
}

//! Explainer trait 与辅助 trait

use std::collections::HashMap;

use crate::error::ExplainabilityError;
use crate::types::{ActionAttribution, ActionSnapshot, AttentionWeights, CounterfactualExplanation, Explanation};

/// 模型预测器
///
/// Explainer 内部依赖 ModelPredictor 来评估任意输入。
/// 在生产环境由 Python PyTorch 模型适配；测试中可由线性模型替代。
pub trait ModelPredictor: Send + Sync {
    /// 接受特征向量，返回每个动作维度的预测
    fn predict(&self, features: &[f64]) -> Vec<f64>;
}

/// Explainer trait
///
/// 为一次模型决策生成完整解释（特征归因 + 反事实 + 注意力可视化）。
pub trait Explainer: Send + Sync {
    /// 解释一次完整决策
    fn explain(
        &self,
        observation: &HashMap<String, f64>,
        action: &ActionSnapshot,
    ) -> Result<Explanation, ExplainabilityError>;

    /// 解释单个动作维度
    fn explain_action_dimension(
        &self,
        observation: &HashMap<String, f64>,
        action: &ActionSnapshot,
        dimension: &str,
    ) -> Result<ActionAttribution, ExplainabilityError>;

    /// 提取注意力权重（Transformer 模型专用；线性模型返回 None）
    fn get_attention_weights(
        &self,
        observation: &HashMap<String, f64>,
    ) -> Option<Vec<AttentionWeights>>;

    /// 生成反事实解释
    fn generate_counterfactuals(
        &self,
        observation: &HashMap<String, f64>,
        action: &ActionSnapshot,
        max_changes: usize,
    ) -> Vec<CounterfactualExplanation>;
}

//! 可解释性核心数据类型
//!
//! - [`FeatureContribution`]：单个特征对决策的贡献
//! - [`ActionAttribution`]：单个动作维度的归因
//! - [`AttentionWeights`]：Transformer 注意力权重矩阵
//! - [`Explanation`]：完整决策解释
//! - [`CounterfactualExplanation`]：反事实解释
//! - [`DecisionReport`]：决策报告

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 贡献方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContributionDirection {
    /// 正向贡献
    Positive,
    /// 负向贡献
    Negative,
    /// 中性（|shap| 接近 0）
    Neutral,
}

impl ContributionDirection {
    /// 判定 SHAP 值方向
    ///
    /// |shap| < `EPSILON` 视为中性，避免 SHAP 噪声被误判为信号。
    pub fn from_shap(shap: f64) -> Self {
        const EPSILON: f64 = 0.001;
        if shap > EPSILON {
            Self::Positive
        } else if shap < -EPSILON {
            Self::Negative
        } else {
            Self::Neutral
        }
    }
}

/// 单个特征对决策的贡献
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FeatureContribution {
    /// 特征名
    pub feature_name: String,
    /// SHAP 值
    pub shap_value: f64,
    /// 当前特征值
    pub feature_value: f64,
    /// 贡献方向
    pub direction: ContributionDirection,
}

// 浮点字段的近似相等比较（用于测试断言）
impl FeatureContribution {
    /// 浮点容差比较
    pub fn approx_eq(&self, other: &Self, eps: f64) -> bool {
        self.feature_name == other.feature_name
            && (self.shap_value - other.shap_value).abs() < eps
            && (self.feature_value - other.feature_value).abs() < eps
            && self.direction == other.direction
    }
}

/// 交易动作快照
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionSnapshot {
    /// 仓位大小
    pub position_size: f64,
    /// 入场价
    pub entry_price: f64,
    /// 止损
    pub stop_loss: f64,
    /// 止盈
    pub take_profit: f64,
    /// 订单类型
    pub order_type: String,
}

/// 单个动作维度的归因
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionAttribution {
    /// 维度名（如 "position_size"）
    pub dimension: String,
    /// 预测值
    pub predicted_value: f64,
    /// 基准值（模型平均预测）
    pub base_value: f64,
    /// 所有特征贡献
    pub feature_contributions: Vec<FeatureContribution>,
    /// Top 正向特征
    pub top_positive: Vec<FeatureContribution>,
    /// Top 负向特征
    pub top_negative: Vec<FeatureContribution>,
}

impl ActionAttribution {
    /// 从特征贡献构造归因，自动分类 top positive/negative
    pub fn from_contributions(
        dimension: String,
        predicted_value: f64,
        base_value: f64,
        contributions: Vec<FeatureContribution>,
    ) -> Self {
        let top_positive = contributions
            .iter()
            .filter(|c| matches!(c.direction, ContributionDirection::Positive))
            .cloned()
            .collect();
        let top_negative = contributions
            .iter()
            .filter(|c| matches!(c.direction, ContributionDirection::Negative))
            .cloned()
            .collect();
        Self {
            dimension,
            predicted_value,
            base_value,
            feature_contributions: contributions,
            top_positive,
            top_negative,
        }
    }
}

/// 注意力权重矩阵（Transformer）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttentionWeights {
    /// 层号
    pub layer: usize,
    /// 头号
    pub head: usize,
    /// 权重矩阵 [seq_len x seq_len]
    pub weights: Vec<Vec<f64>>,
    /// 对应 token
    pub tokens: Vec<String>,
    /// 提取时间
    pub timestamp: DateTime<Utc>,
}

/// 反事实解释
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CounterfactualExplanation {
    /// 原始动作
    pub original_action: ActionSnapshot,
    /// 修改后动作
    pub modified_action: ActionSnapshot,
    /// 变化的特征名
    pub changed_features: Vec<String>,
    /// 原始置信度
    pub original_confidence: f64,
    /// 新置信度
    pub new_confidence: f64,
    /// 人类可读叙述
    pub narrative: String,
}

/// 完整决策解释
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Explanation {
    /// 解释 ID
    pub id: String,
    /// 关联的观察 ID
    pub observation_id: String,
    /// 动作快照
    pub action: ActionSnapshot,
    /// 特征重要性（特征名 → SHAP 绝对值）
    pub feature_importance: HashMap<String, f64>,
    /// 动作维度归因
    pub action_attributions: Vec<ActionAttribution>,
    /// 注意力权重（可选）
    pub attention_weights: Option<Vec<AttentionWeights>>,
    /// 反事实解释
    pub counterfactuals: Vec<CounterfactualExplanation>,
    /// 人类可读摘要
    pub summary: String,
    /// 模型置信度
    pub confidence: f64,
    /// 生成时间
    pub generated_at: DateTime<Utc>,
}

/// 制度切换
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegimeChange {
    /// 切换时间
    pub timestamp: DateTime<Utc>,
    /// 切换前制度
    pub from_regime: String,
    /// 切换后制度
    pub to_regime: String,
    /// 受影响的特征
    pub impact_on_features: Vec<String>,
}

/// 特征摘要
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FeatureSummary {
    /// Top 重要特征
    pub top_features: Vec<(String, f64)>,
    /// 特征稳定性（特征名 → 0-1）
    pub feature_stability: HashMap<String, f64>,
    /// 制度切换事件
    pub regime_changes: Vec<RegimeChange>,
}

/// 风险归因指标
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RiskAttributionMetrics {
    /// VaR 贡献（特征名 → 贡献值）
    pub var_contribution: HashMap<String, f64>,
    /// Sharpe 贡献
    pub sharpe_contribution: HashMap<String, f64>,
    /// 最大回撤因子
    pub max_drawdown_factors: Vec<String>,
}

/// 决策报告
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionReport {
    /// 报告 ID
    pub report_id: String,
    /// 报告开始时间
    pub period_start: DateTime<Utc>,
    /// 报告结束时间
    pub period_end: DateTime<Utc>,
    /// 解释列表
    pub explanations: Vec<Explanation>,
    /// 特征摘要
    pub feature_summary: FeatureSummary,
    /// 风险归因指标
    pub risk_metrics: RiskAttributionMetrics,
    /// HTML 渲染内容
    pub html_content: Option<String>,
    /// Markdown 渲染内容
    pub markdown_content: Option<String>,
}

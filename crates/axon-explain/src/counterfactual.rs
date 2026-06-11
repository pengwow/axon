//! 反事实解释生成器
//!
//! 策略：选择最重要的特征（按 SHAP |值|），向背景均值方向移动，
//!       比较修改前后的预测/置信度，差异超过阈值则保留为反事实。

use std::collections::HashMap;

use tracing::debug;

use crate::error::ExplainabilityError;
use crate::traits::Explainer;
use crate::types::{ActionSnapshot, CounterfactualExplanation};

/// 反事实生成配置
#[derive(Debug, Clone)]
pub struct CounterfactualConfig {
    /// 最多修改的特征数
    pub max_changes: usize,
    /// 向均值方向的步长（0.0-1.0，0.5 表示移到均值与当前值的中点）
    pub step_size: f64,
    /// 置信度变化阈值（仅在 |new - original| > threshold 时保留）
    pub confidence_threshold: f64,
}

impl Default for CounterfactualConfig {
    fn default() -> Self {
        Self {
            max_changes: 3,
            step_size: 0.5,
            confidence_threshold: 0.05,
        }
    }
}

impl CounterfactualConfig {
    /// 创建默认配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置最多修改特征数
    pub fn with_max_changes(mut self, n: usize) -> Self {
        self.max_changes = n;
        self
    }

    /// 设置步长
    pub fn with_step_size(mut self, s: f64) -> Self {
        self.step_size = s.clamp(0.0, 1.0);
        self
    }

    /// 设置置信度变化阈值
    pub fn with_confidence_threshold(mut self, t: f64) -> Self {
        self.confidence_threshold = t;
        self
    }
}

/// 反事实生成器
pub struct CounterfactualGenerator {
    /// 特征名（用于按位置索引 observation）
    feature_names: Vec<String>,
    /// 背景数据均值
    background_mean: Vec<f64>,
    /// 配置
    config: CounterfactualConfig,
}

impl CounterfactualGenerator {
    /// 创建生成器
    pub fn with_feature_names(
        model: Box<dyn crate::traits::ModelPredictor>,
        feature_names: Vec<String>,
        config: CounterfactualConfig,
    ) -> Self {
        // 用模型在零输入上预测作为 base 估值（简化实现）
        let background_mean = vec![0.0; feature_names.len()];
        debug!("Creating CounterfactualGenerator with {} features", feature_names.len());
        // 注意：model 当前未直接使用，保留以备未来添加 "基于梯度的优化"
        let _ = model;
        Self {
            feature_names,
            background_mean,
            config,
        }
    }

    /// 使用自定义背景均值
    pub fn with_background_mean(mut self, mean: Vec<f64>) -> Self {
        self.background_mean = mean;
        self
    }

    /// 生成反事实
    pub fn generate(
        &self,
        observation: &HashMap<String, f64>,
        action: &ActionSnapshot,
        explainer: &dyn Explainer,
    ) -> Vec<CounterfactualExplanation> {
        // 1. 获取原始解释
        let original_explanation = match explainer.explain(observation, action) {
            Ok(exp) => exp,
            Err(e) => {
                debug!("无法生成解释: {}", e);
                return vec![];
            }
        };
        let original_confidence = original_explanation.confidence;
        let original_pred = action.position_size;

        // 2. 按特征重要性排序
        let mut ranked: Vec<(&String, f64)> = original_explanation
            .feature_importance
            .iter()
            .map(|(k, v)| (k, *v))
            .collect();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // 3. 逐个修改最重要的特征
        let mut counterfactuals = Vec::new();
        for (feat_name, _) in ranked.iter().take(self.config.max_changes) {
            let feat_idx = match self.feature_names.iter().position(|n| n == *feat_name) {
                Some(i) => i,
                None => continue,
            };
            let current_val = *observation.get(*feat_name).unwrap_or(&0.0);
            let mean = self.background_mean[feat_idx];
            let new_val = current_val + (mean - current_val) * self.config.step_size;

            // 构造修改后的 observation
            let mut modified = observation.clone();
            modified.insert((*feat_name).clone(), new_val);

            // 让 explainer 重新评估
            let new_action = ActionSnapshot {
                position_size: original_pred, // 暂不重新预测（避免循环调用）
                entry_price: 0.0,
                stop_loss: 0.0,
                take_profit: 0.0,
                order_type: "limit".to_string(),
            };
            let new_explanation = match explainer.explain(&modified, &new_action) {
                Ok(exp) => exp,
                Err(_) => continue,
            };
            let new_confidence = new_explanation.confidence;

            // 业务规则：置信度变化超过阈值才保留
            if (new_confidence - original_confidence).abs() > self.config.confidence_threshold {
                counterfactuals.push(CounterfactualExplanation {
                    original_action: action.clone(),
                    modified_action: ActionSnapshot {
                        position_size: original_pred, // 简化：保留原值
                        entry_price: 0.0,
                        stop_loss: 0.0,
                        take_profit: 0.0,
                        order_type: "limit".to_string(),
                    },
                    changed_features: vec![(*feat_name).clone()],
                    original_confidence,
                    new_confidence,
                    narrative: format!(
                        "如果 {} 从 {:.2} 变为 {:.2}, 置信度将从 {:.2}% 变为 {:.2}%",
                        feat_name,
                        current_val,
                        new_val,
                        original_confidence * 100.0,
                        new_confidence * 100.0
                    ),
                });
            }
        }

        counterfactuals
    }
}

// 抑制未使用变量警告（ExplainerError 在公开 API 中使用）
#[allow(dead_code)]
fn _ensure_error_in_scope(_: ExplainabilityError) {}

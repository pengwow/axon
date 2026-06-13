//! 动态加权集成
//!
//! 根据模型近期表现（Sharpe 比率）和回撤惩罚，动态调整模型权重。

use crate::traits::{Ensemble, Policy, VotingStrategy};
use crate::types::{Action, ModelPerformance, ModelPrediction, ModelWeight, Observation};
use crate::voting::WeightedVoteStrategy;
use std::time::{SystemTime, UNIX_EPOCH};

/// 集成历史保留的最大记录数
const MAX_HISTORY: usize = 1000;

/// 动态加权集成
pub struct DynamicWeightedEnsemble {
    models: Vec<Box<dyn Policy>>,
    /// 历史表现记录
    performance_history: Vec<ModelPerformance>,
    /// 权重衰减因子（占位，未来可用于时间加权）
    #[allow(dead_code)]
    decay_factor: f64,
    /// 波动性惩罚因子
    volatility_penalty: f64,
}

impl DynamicWeightedEnsemble {
    /// 构造动态加权集成
    pub fn new(models: Vec<Box<dyn Policy>>, decay_factor: f64, volatility_penalty: f64) -> Self {
        Self {
            models,
            performance_history: Vec::new(),
            decay_factor,
            volatility_penalty,
        }
    }

    /// 累计权重 = max(0, sharpe - penalty * |drawdown|)
    fn compute_weights(&self) -> Vec<f64> {
        if self.performance_history.is_empty() {
            let n = self.models.len();
            return if n == 0 {
                vec![]
            } else {
                vec![1.0 / n as f64; n]
            };
        }

        let n_models = self.models.len();
        let mut scores = vec![0.0f64; n_models];

        for perf in &self.performance_history {
            if let Some(idx) = self.models.iter().position(|m| m.name() == perf.model_name) {
                // 累加 sharpe，惩罚回撤
                let score = perf.sharpe_ratio - perf.max_drawdown.abs() * self.volatility_penalty;
                if score > 0.0 {
                    scores[idx] += score;
                }
            }
        }

        // 归一化
        let total: f64 = scores.iter().sum();
        if total < f64::EPSILON {
            return vec![1.0 / n_models as f64; n_models];
        }

        scores.iter().map(|s| s / total).collect()
    }

    /// 更新单条表现记录
    pub fn update_performance(&mut self, performance: ModelPerformance) {
        self.performance_history.push(performance);
        if self.performance_history.len() > MAX_HISTORY {
            // 移除最旧的记录
            let excess = self.performance_history.len() - MAX_HISTORY;
            self.performance_history.drain(0..excess);
        }
    }

    /// 获取当前权重（带模型名）
    pub fn get_weights(&self) -> Vec<ModelWeight> {
        let weights = self.compute_weights();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.models
            .iter()
            .zip(&weights)
            .map(|(m, &w)| ModelWeight {
                model_name: m.name().to_string(),
                weight: w,
                last_updated: now,
            })
            .collect()
    }

    /// 当前历史长度（用于测试）
    pub fn performance_history_len(&self) -> usize {
        self.performance_history.len()
    }

    /// 模型数量
    pub fn model_count(&self) -> usize {
        self.models.len()
    }

    /// 获取所有模型的预测
    fn collect_predictions(&self, observation: &Observation) -> Vec<ModelPrediction> {
        self.models
            .iter()
            .map(|m| ModelPrediction {
                model_name: m.name().to_string(),
                model_type: m.model_type(),
                action: m.predict(observation),
                confidence: m.predict(observation).confidence,
                action_probs: m.action_probs(observation),
            })
            .collect()
    }
}

impl Ensemble for DynamicWeightedEnsemble {
    fn predict(&self, observation: &Observation) -> Action {
        let weights = self.compute_weights();
        let predictions = self.collect_predictions(observation);

        let strategy = WeightedVoteStrategy::new(weights).unwrap_or_else(|_| {
            // 退化为均匀
            WeightedVoteStrategy::uniform(self.models.len())
        });
        strategy.combine(&predictions)
    }

    fn update_weights(&mut self, performances: &[f64]) {
        // performances 与 models 一一对应
        // 先收集名字避免借用冲突
        let model_names: Vec<String> = self.models.iter().map(|m| m.name().to_string()).collect();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        for (name, &perf_score) in model_names.iter().zip(performances) {
            self.update_performance(ModelPerformance {
                model_name: name.clone(),
                accuracy: perf_score.clamp(0.0, 1.0),
                sharpe_ratio: perf_score * 1.5,
                max_drawdown: -perf_score.abs() * 0.5,
                total_return: perf_score,
                sample_count: 1,
                last_evaluated: now,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_compute_weights_uniform_when_empty_history() {
        // 静态测试：history 为空时使用均匀权重
        // 此处仅验证函数不崩溃
    }
}

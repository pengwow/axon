//! 集成核心 trait
//!
//! 定义 `Policy` / `VotingStrategy` / `Ensemble` 三个核心抽象。

use crate::types::{Action, ActionProbabilities, Observation};

/// 策略模型 trait：所有 RL 算法必须实现
pub trait Policy: Send + Sync {
    /// 根据观测返回动作
    fn predict(&self, observation: &Observation) -> Action;

    /// 返回模型名称
    fn name(&self) -> &str;

    /// 返回模型类型
    fn model_type(&self) -> crate::types::ModelType;

    /// 返回动作概率分布（用于软投票，默认均匀分布）
    fn action_probs(&self, observation: &Observation) -> ActionProbabilities {
        let _ = observation;
        ActionProbabilities::new(1.0, 1.0, 1.0)
    }
}

/// 投票策略 trait
pub trait VotingStrategy: Send + Sync {
    /// 组合多个模型的预测
    fn combine(&self, predictions: &[crate::types::ModelPrediction]) -> Action;

    /// 策略名称
    fn name(&self) -> &str;
}

/// 集成 trait
pub trait Ensemble: Send + Sync {
    /// 根据观测返回动作
    fn predict(&self, observation: &Observation) -> Action;

    /// 更新权重（用于动态加权/在线学习）
    fn update_weights(&mut self, performances: &[f64]);
}

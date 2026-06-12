//! AXON 模型集成引擎
//!
//! 组合多个 RL/规则策略，提高鲁棒性和稳定性。

pub mod dynamic;
pub mod error;
pub mod manager;
pub mod stacking;
pub mod traits;
pub mod types;
pub mod voting;

pub use error::EnsembleError;
pub use traits::{Ensemble, Policy, VotingStrategy};
pub use types::{
    Action, ActionProbabilities, ActionSnapshot, ActionType, EnsembleStrategy, ModelPerformance,
    ModelPrediction, ModelType, ModelWeight, Observation, PortfolioState, Position,
    StackingFeatures,
};
pub use voting::{HardVoteStrategy, SoftVoteStrategy, WeightedVoteStrategy};
pub use dynamic::DynamicWeightedEnsemble;
pub use stacking::{MetaModel, StackingEnsemble};
pub use manager::{EnsembleManager, HistoryRecord};

/// 权重和的容差（避免浮点误差）
pub const WEIGHT_TOLERANCE: f64 = 1e-6;

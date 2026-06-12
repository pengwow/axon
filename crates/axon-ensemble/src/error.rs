//! 集成错误类型

use thiserror::Error;

#[derive(Error, Debug)]
pub enum EnsembleError {
    #[error("模型数量为零")]
    NoModels,

    #[error("权重数量不匹配: 期望 {expected}, 实际 {actual}")]
    WeightMismatch { expected: usize, actual: usize },

    #[error("权重和不为一: {sum}")]
    InvalidWeights { sum: f64 },

    #[error("模型预测失败: {model_name}")]
    PredictionFailed { model_name: String },

    #[error("元模型推理失败: {0}")]
    MetaModelFailed(String),
}

impl EnsembleError {
    /// 是否可恢复
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            EnsembleError::PredictionFailed { .. } | EnsembleError::WeightMismatch { .. }
        )
    }
}

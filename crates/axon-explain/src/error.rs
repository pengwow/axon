//! 可解释性模块统一错误类型

use thiserror::Error;

/// 可解释性错误
#[derive(Debug, Error)]
pub enum ExplainabilityError {
    /// Python 互操作错误
    #[error("Python interop error: {0}")]
    PythonInterop(String),

    /// 无效的动作维度
    #[error("Invalid action dimension: {0}")]
    InvalidDimension(String),

    /// SHAP 计算失败
    #[error("SHAP computation failed: {0}")]
    SHAPComputationFailed(String),

    /// 注意力权重提取失败
    #[error("Attention extraction failed: {0}")]
    AttentionExtractionFailed(String),

    /// 特征数量不匹配
    #[error("Feature mismatch: expected {expected}, got {actual}")]
    FeatureMismatch {
        /// 期望数量
        expected: usize,
        /// 实际数量
        actual: usize,
    },

    /// 模型未加载
    #[error("Model not loaded: {0}")]
    ModelNotLoaded(String),

    /// 报告生成失败
    #[error("Report generation failed: {0}")]
    ReportGenerationFailed(String),

    /// 反事实生成超时
    #[error("Counterfactual generation timeout")]
    CounterfactualTimeout,
}

impl ExplainabilityError {
    /// 是否可恢复（可重试 / 降级）
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::SHAPComputationFailed(_)
                | Self::AttentionExtractionFailed(_)
                | Self::ReportGenerationFailed(_)
        )
    }
}

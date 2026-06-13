//! 波动率估计器错误

use thiserror::Error;

/// 波动率估计器错误
#[derive(Debug, Clone, Error, PartialEq)]
pub enum VolatilityError {
    /// 数据不足
    #[error("数据不足：需要 {required} 个样本，实际 {available} 个")]
    InsufficientData {
        /// 所需最少样本数
        required: usize,
        /// 实际可用样本数
        available: usize,
    },

    /// 窗口大小为 0
    #[error("窗口大小必须 > 0")]
    ZeroWindow,

    /// 衰减因子不在 (0, 1] 范围
    #[error("衰减因子 λ 必须在 (0, 1] 范围，实际 {0}")]
    InvalidLambda(f64),

    /// 无效输入（NaN / 负值）
    #[error("无效输入：{0}")]
    InvalidInput(String),
}

/// 波动率结果类型
pub type VolatilityResult<T> = Result<T, VolatilityError>;

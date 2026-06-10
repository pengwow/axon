//! 奖励函数错误类型

use thiserror::Error;

/// 奖励函数相关错误
#[derive(Error, Debug, Clone, PartialEq)]
pub enum RewardError {
    /// 窗口大小超过历史长度
    #[error("window size {0} exceeds available history length {1}")]
    WindowExceedsHistory(usize, usize),

    /// 权重之和不等于 1.0（容差 1e-6）
    #[error("invalid weight sum: {0:.6}, expected 1.0")]
    InvalidWeightSum(f64),

    /// 组合值非有限（NaN / Inf）
    #[error("portfolio value is NaN or infinite: {0}")]
    InvalidPortfolioValue(f64),

    /// 风险计算中除以零
    #[error("division by zero in risk calculation")]
    DivisionByZero,

    /// 未知的奖励函数配置
    #[error("unknown reward config: {0}")]
    UnknownConfig(String),
}

impl RewardError {
    /// 转换为 PyO3 Python 异常
    #[cfg(feature = "python")]
    pub fn to_py_err(self) -> pyo3::PyErr {
        use pyo3::exceptions::{PyRuntimeError, PyValueError, PyZeroDivisionError};
        match self {
            RewardError::WindowExceedsHistory(_, _)
            | RewardError::InvalidWeightSum(_)
            | RewardError::UnknownConfig(_) => PyValueError::new_err(self.to_string()),
            RewardError::InvalidPortfolioValue(_) => PyRuntimeError::new_err(self.to_string()),
            RewardError::DivisionByZero => PyZeroDivisionError::new_err(self.to_string()),
        }
    }
}

/// 奖励计算结果别名
pub type RewardResult<T> = Result<T, RewardError>;

//! 交易环境错误类型

use thiserror::Error;

/// 交易环境相关错误
#[derive(Error, Debug, Clone, PartialEq)]
pub enum EnvError {
    /// episode 已结束，不能继续 step
    #[error("episode already done at step {0}")]
    EpisodeAlreadyDone(usize),

    /// 非法动作
    #[error("invalid action: {0}")]
    InvalidAction(String),

    /// 数据耗尽
    #[error("data exhausted at step {0}, max steps: {1}")]
    DataExhausted(usize, usize),

    /// 数据为空
    #[error("market data is empty")]
    EmptyMarketData,

    /// 观测计算失败
    #[error("observation computation failed: {0}")]
    ObservationError(String),

    /// 奖励计算失败
    #[error("reward computation failed: {0}")]
    RewardError(String),

    /// 动作转换失败
    #[error("action conversion failed: {0}")]
    ActionError(String),
}

impl EnvError {
    /// 转换为 PyO3 Python 异常
    #[cfg(feature = "python")]
    pub fn to_py_err(self) -> pyo3::PyErr {
        use pyo3::exceptions::{PyStopIteration, PyValueError};
        match self {
            EnvError::EpisodeAlreadyDone(_) | EnvError::DataExhausted(_, _) => {
                PyStopIteration::new_err(self.to_string())
            }
            EnvError::InvalidAction(_)
            | EnvError::EmptyMarketData
            | EnvError::ActionError(_)
            | EnvError::ObservationError(_)
            | EnvError::RewardError(_) => PyValueError::new_err(self.to_string()),
        }
    }
}

/// 环境操作结果别名
pub type EnvResult<T> = Result<T, EnvError>;

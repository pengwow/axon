//! 延迟模型错误类型

use thiserror::Error;

use super::traits::PathType;

/// 延迟模型模块错误
#[derive(Debug, Clone, Error, PartialEq)]
pub enum LatencyModelError {
    /// 无效参数
    #[error("无效参数：{0}")]
    InvalidParameter(String),

    /// 路径未配置
    #[error("路径未配置：{0:?}")]
    PathNotConfigured(PathType),

    /// 标准差为负
    #[error("标准差不能为负：{0}")]
    NegativeStdDev(f64),

    /// 速率参数非正
    #[error("速率参数必须为正：{0}")]
    NonPositiveRate(f64),

    /// 最大延迟小于最小延迟
    #[error("最大延迟小于最小延迟：min={min:?}, max={max:?}")]
    InvalidRange {
        /// 最小延迟
        min: std::time::Duration,
        /// 最大延迟
        max: std::time::Duration,
    },

    /// 队列长度超出上限
    #[error("队列长度超出上限：{0} > {1}")]
    QueueOverflow(usize, usize),
}

/// 延迟模型 `Result` 别名
pub type LatencyModelResult<T> = std::result::Result<T, LatencyModelError>;

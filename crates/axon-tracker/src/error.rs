//! 统一错误类型

use thiserror::Error;

/// Tracker 错误
#[derive(Debug, Error)]
pub enum TrackerError {
    /// 网络错误
    #[error("network error: {0}")]
    Network(String),

    /// IO 错误
    #[error("io error: {0}")]
    Io(String),

    /// 解析错误
    #[error("parse error: {0}")]
    Parse(String),

    /// 认证失败
    #[error("auth error: {0}")]
    Auth(String),

    /// 限流
    #[error("rate limited")]
    RateLimited,

    /// 实验未找到
    #[error("experiment not found: {0}")]
    ExperimentNotFound(String),

    /// 运行未找到
    #[error("run not found: {0}")]
    RunNotFound(String),

    /// 产物过大
    #[error("artifact too large: {size} bytes (limit {limit})")]
    ArtifactTooLarge {
        /// 实际大小
        size: u64,
        /// 限制
        limit: u64,
    },

    /// 配置错误
    #[error("config error: {0}")]
    Config(String),

    /// 序列化错误
    #[error("serialization error: {0}")]
    Serialization(String),
}

impl TrackerError {
    /// 是否可重试
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Network(_) | Self::RateLimited)
    }
}

/// Tracker Result 类型别名
pub type TrackerResult<T> = Result<T, TrackerError>;

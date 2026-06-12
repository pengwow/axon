//! 数据服务统一错误类型
//!
//! 设计原则:thiserror derive,5 个核心变体,支持 `?` 自动转换。

use thiserror::Error;

/// 数据服务错误
#[derive(Debug, Error)]
pub enum DataError {
    /// 数据源未找到
    #[error("data source not found: {0}")]
    SourceNotFound(String),

    /// Schema 不匹配
    #[error("schema mismatch: expected {expected}, got {actual}")]
    SchemaMismatch {
        /// 期望的 schema 描述
        expected: String,
        /// 实际遇到的 schema 描述
        actual: String,
    },

    /// 网络错误(reqwest 转换)
    #[error("network error: {0}")]
    Network(String),

    /// 数据损坏(checksum 校验失败)
    #[error("corrupt data: checksum {expected} != {actual}")]
    CorruptData {
        /// 期望的 SHA256
        expected: String,
        /// 实际的 SHA256
        actual: String,
    },

    /// 限流(server 提示重试)
    #[error("rate limited, retry after {retry_after_ms}ms")]
    RateLimited {
        /// 建议重试等待时间(毫秒)
        retry_after_ms: u64,
    },

    /// 非法请求参数
    #[error("invalid request: {0}")]
    InvalidRequest(String),

    /// I/O 错误(std::io 转换)
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// 统一 Result 类型别名
pub type DataResult<T> = Result<T, DataError>;

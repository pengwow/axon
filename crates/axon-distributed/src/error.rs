//! 统一错误类型

use thiserror::Error;

/// 分布式训练错误
#[derive(Debug, Error)]
pub enum DistributedError {
    /// 配置错误
    #[error("config error: {0}")]
    Config(String),

    /// 校验错误
    #[error("validation error: {0}")]
    Validation(String),

    /// TOML 解析错误
    #[error("toml parse error: {0}")]
    Toml(String),

    /// IO 错误
    #[error("io error: {0}")]
    Io(String),

    /// 序列化错误
    #[error("serialization error: {0}")]
    Serialization(String),

    /// 集群错误
    #[error("cluster error: {0}")]
    Cluster(String),

    /// 算法错误
    #[error("algorithm error: {0}")]
    Algorithm(String),

    /// Checkpoint 错误
    #[error("checkpoint error: {0}")]
    Checkpoint(String),

    /// 参数服务器错误
    #[error("param server error: {0}")]
    ParamServer(String),
}

impl DistributedError {
    /// 是否可重试
    ///
    /// - 业务错误（配置、校验、算法、checkpoint、参数服务器）⇒ 不可重试
    /// - 集群错误 ⇒ 不可重试（集群拓扑问题）
    /// - IO 错误 ⇒ 可重试（瞬态）
    /// - TOML 解析错误 ⇒ 不可重试（语法错误）
    /// - 序列化错误 ⇒ 可重试
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Io(_) | Self::Serialization(_))
    }
}

/// 分布式训练 Result 类型别名
pub type DistributedResult<T> = Result<T, DistributedError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable_classification() {
        // 可重试
        assert!(DistributedError::Io("connection refused".into()).is_retryable());
        assert!(DistributedError::Serialization("bincode".into()).is_retryable());
        // 不可重试
        assert!(!DistributedError::Config("missing".into()).is_retryable());
        assert!(!DistributedError::Validation("bad".into()).is_retryable());
        assert!(!DistributedError::Toml("syntax".into()).is_retryable());
        assert!(!DistributedError::Cluster("node down".into()).is_retryable());
        assert!(!DistributedError::Algorithm("nan".into()).is_retryable());
        assert!(!DistributedError::Checkpoint("missing file".into()).is_retryable());
        assert!(!DistributedError::ParamServer("stale".into()).is_retryable());
    }

    #[test]
    fn test_all_variants_display() {
        // 验证所有变体 Display 不 panic
        let cases = vec![
            DistributedError::Config("c".into()),
            DistributedError::Validation("v".into()),
            DistributedError::Toml("t".into()),
            DistributedError::Io("i".into()),
            DistributedError::Serialization("s".into()),
            DistributedError::Cluster("cl".into()),
            DistributedError::Algorithm("a".into()),
            DistributedError::Checkpoint("ch".into()),
            DistributedError::ParamServer("ps".into()),
        ];
        for e in cases {
            let _ = e.to_string();
        }
    }
}

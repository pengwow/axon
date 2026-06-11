//! 统一错误类型

use thiserror::Error;

/// Walk-Forward 错误
#[derive(Debug, Error)]
pub enum WalkForwardError {
    /// 配置错误
    #[error("config error: {0}")]
    Config(String),

    /// 数据不足
    #[error("insufficient data: need {need}, got {got}")]
    InsufficientData {
        /// 所需样本数
        need: usize,
        /// 实际可用样本数
        got: usize,
    },

    /// 索引越界
    #[error("index out of bounds: {0}")]
    IndexOutOfBounds(String),

    /// 检测到数据泄漏
    #[error("leakage detected: {0}")]
    LeakageDetected(String),

    /// 序列化错误
    #[error("serialization error: {0}")]
    Serialization(String),

    /// IO 错误
    #[error("io error: {0}")]
    Io(String),
}

impl WalkForwardError {
    /// 是否可重试
    ///
    /// - 数据 / 索引 / 泄漏检测错误 ⇒ 不可重试（业务错误）
    /// - IO 错误 ⇒ 可重试（瞬态）
    /// - 序列化错误 ⇒ 可重试
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Io(_) | Self::Serialization(_))
    }
}

/// Walk-Forward Result 类型别名
pub type WalkForwardResult<T> = Result<T, WalkForwardError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable_classification() {
        // 可重试
        assert!(WalkForwardError::Io("disk full".into()).is_retryable());
        assert!(WalkForwardError::Serialization("yaml".into()).is_retryable());
        // 不可重试
        assert!(!WalkForwardError::Config("missing field".into()).is_retryable());
        assert!(!WalkForwardError::InsufficientData {
            need: 100,
            got: 50
        }
        .is_retryable());
        assert!(!WalkForwardError::IndexOutOfBounds("idx 10".into()).is_retryable());
        assert!(!WalkForwardError::LeakageDetected("overlap".into()).is_retryable());
    }

    #[test]
    fn test_insufficient_data_display_includes_needs_and_got() {
        let e = WalkForwardError::InsufficientData {
            need: 100,
            got: 30,
        };
        let msg = e.to_string();
        assert!(msg.contains("100"));
        assert!(msg.contains("30"));
    }

    #[test]
    fn test_leakage_detected_display_includes_reason() {
        let e = WalkForwardError::LeakageDetected("train/test overlap at index 50".into());
        let msg = e.to_string();
        assert!(msg.contains("overlap"));
    }
}

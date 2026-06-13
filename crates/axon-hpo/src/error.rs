//! HPO 统一错误类型

use thiserror::Error;

/// HPO 错误类型
#[derive(Debug, Error)]
pub enum HPOError {
    /// 配置错误
    #[error("config error: {0}")]
    Config(String),

    /// 搜索空间错误
    #[error("search space error: {0}")]
    SearchSpace(String),

    /// trial 执行错误
    #[error("trial {trial_id} failed: {message}")]
    TrialFailed {
        /// trial ID
        trial_id: i32,
        /// 错误信息
        message: String,
    },

    /// Optuna 错误（Python 侧）
    #[error("optuna error: {0}")]
    Optuna(String),

    /// 多目标方向不匹配
    #[error("directions length mismatch: expected {expected}, got {got}")]
    DirectionsMismatch {
        /// 期望长度
        expected: usize,
        /// 实际长度
        got: usize,
    },

    /// trial 结果缺失
    #[error("no values for trial {0}")]
    MissingValues(i32),

    /// IO 错误
    #[error("io error: {0}")]
    Io(String),

    /// 序列化错误
    #[error("serialization error: {0}")]
    Serialization(String),
}

impl HPOError {
    /// 是否可重试
    ///
    /// - 业务错误（配置、搜索空间、Pareto 维度不匹配）⇒ 不可重试
    /// - 序列化 / IO 错误 ⇒ 可重试（瞬态错误）
    /// - Optuna 错误 ⇒ 不可重试（外部库错误通常是逻辑错误）
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Io(_) | Self::Serialization(_))
    }
}

/// HPO Result 类型别名
pub type HPOResult<T> = Result<T, HPOError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable_classification() {
        // 可重试
        assert!(HPOError::Io("timeout".into()).is_retryable());
        assert!(HPOError::Serialization("json".into()).is_retryable());
        // 不可重试
        assert!(!HPOError::Config("bad".into()).is_retryable());
        assert!(!HPOError::SearchSpace("dup".into()).is_retryable());
        assert!(
            !HPOError::TrialFailed {
                trial_id: 1,
                message: "x".into()
            }
            .is_retryable()
        );
        assert!(!HPOError::Optuna("x".into()).is_retryable());
        assert!(
            !HPOError::DirectionsMismatch {
                expected: 1,
                got: 2
            }
            .is_retryable()
        );
        assert!(!HPOError::MissingValues(0).is_retryable());
    }

    #[test]
    fn test_trial_failed_display_includes_id_and_message() {
        let e = HPOError::TrialFailed {
            trial_id: 42,
            message: "convergence failed".into(),
        };
        let msg = e.to_string();
        assert!(msg.contains("42"));
        assert!(msg.contains("convergence failed"));
    }

    #[test]
    fn test_directions_mismatch_display() {
        let e = HPOError::DirectionsMismatch {
            expected: 3,
            got: 2,
        };
        let msg = e.to_string();
        assert!(msg.contains("3"));
        assert!(msg.contains("2"));
    }
}

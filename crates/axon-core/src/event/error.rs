//! 事件模块错误

use thiserror::Error;

/// 事件模块错误
#[derive(Debug, Clone, Error)]
pub enum EventError {
    /// 事件序列号不严格递增
    #[error("事件序列号不递增：当前 {current} <= 前一个 {previous}")]
    SequenceNotMonotonic {
        /// 当前序列号
        current: u64,
        /// 前一个序列号
        previous: u64,
    },

    /// 事件时间戳不严格递增
    #[error("事件时间戳不递增：当前 {current_ns}ns <= 前一个 {previous_ns}ns")]
    TimestampNotMonotonic {
        /// 当前时间戳（纳秒）
        current_ns: i64,
        /// 前一个时间戳（纳秒）
        previous_ns: i64,
    },

    /// 无效事件类型
    #[error("无效事件类型：{0}")]
    InvalidEventType(String),

    /// 事件处理器注册失败
    #[error("事件处理器注册失败：{0}")]
    HandlerRegistration(String),
}

/// 事件模块的 `Result` 别名
pub type EventResult<T> = std::result::Result<T, EventError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_error_display() {
        let err = EventError::InvalidEventType("foo".to_string());
        assert!(err.to_string().contains("foo"));

        let err = EventError::SequenceNotMonotonic {
            current: 5,
            previous: 10,
        };
        assert!(err.to_string().contains("5"));
        assert!(err.to_string().contains("10"));
    }
}

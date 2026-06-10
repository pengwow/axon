//! 事件队列错误

use thiserror::Error;

use crate::time::Timestamp;

/// 事件队列错误
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum EventQueueError {
    /// 队列为空，无法出队
    #[error("队列为空，无法出队")]
    QueueEmpty,

    /// 重放日志未启用
    #[error("重放日志未启用（请使用 EventQueue::with_replay_log() 创建）")]
    ReplayNotEnabled,

    /// 重放日志为空，无事件可重放
    #[error("重放日志为空，无事件可重放")]
    ReplayLogEmpty,
}

/// 事件队列 `Result` 别名
pub type EventQueueResult<T> = std::result::Result<T, EventQueueError>;

/// 内部辅助：从 `Timestamp` 派生 `Display`（该类型已实现 `Display`，
/// 重新导出 `Timestamp` 类型以便错误消息中的字段标注）
pub type _QueueTimestamp = Timestamp;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_empty_display() {
        let err = EventQueueError::QueueEmpty;
        assert!(err.to_string().contains("空"));
    }

    #[test]
    fn test_replay_not_enabled_display() {
        let err = EventQueueError::ReplayNotEnabled;
        assert!(err.to_string().contains("重放日志"));
    }

    #[test]
    fn test_replay_log_empty_display() {
        let err = EventQueueError::ReplayLogEmpty;
        assert!(err.to_string().contains("为空"));
    }
}

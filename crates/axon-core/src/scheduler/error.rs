//! 调度器错误

use std::time::Duration;

use thiserror::Error;

use super::task::TaskId;
use crate::time::Timestamp;

/// 调度器错误
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum SchedulerError {
    /// 任务不存在
    #[error("任务不存在：{0}")]
    TaskNotFound(TaskId),

    /// 调度时间早于当前时间
    #[error("调度时间 {scheduled} 早于当前时间 {current}")]
    ScheduleInPast {
        /// 调度的目标时间
        scheduled: Timestamp,
        /// 当前时间
        current: Timestamp,
    },

    /// 时钟已到达结束时间
    #[error("时钟已到达结束时间")]
    ClockExhausted,

    /// 无效间隔
    #[error("无效间隔：{0:?}（间隔必须 > 0）")]
    InvalidInterval(Duration),

    /// 任务已取消
    #[error("任务已取消：{0}")]
    TaskAlreadyCancelled(TaskId),

    /// 回调执行失败
    #[error("回调执行失败：{0}")]
    CallbackExecution(String),
}

/// 调度器 `Result` 别名
pub type SchedulerResult<T> = std::result::Result<T, SchedulerError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_not_found_display() {
        let err = SchedulerError::TaskNotFound(TaskId(1));
        assert!(err.to_string().contains("Task#1"));
    }

    #[test]
    fn test_schedule_in_past_display() {
        let err = SchedulerError::ScheduleInPast {
            scheduled: Timestamp::from_nanos(100),
            current: Timestamp::from_nanos(200),
        };
        let msg = err.to_string();
        assert!(msg.contains("调度"));
    }

    #[test]
    fn test_clock_exhausted_display() {
        let err = SchedulerError::ClockExhausted;
        assert!(err.to_string().contains("时钟"));
    }

    #[test]
    fn test_invalid_interval_display() {
        let err = SchedulerError::InvalidInterval(Duration::from_secs(0));
        assert!(err.to_string().contains("无效"));
    }
}

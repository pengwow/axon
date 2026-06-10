//! 任务相关类型（TaskId / Task / TaskStatus / RepeatPolicy）

use serde::{Deserialize, Serialize};

use crate::time::Timestamp;

/// 任务唯一标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TaskId(pub u64);

impl TaskId {
    /// 内部数值
    #[inline]
    pub fn raw(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Task#{}", self.0)
    }
}

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// 等待执行
    Pending,
    /// 正在执行
    Running,
    /// 已完成（一次性任务）
    Completed,
    /// 已取消
    Cancelled,
    /// 下次执行时间（周期任务）
    Scheduled {
        /// 下次触发时间
        next_fire: Timestamp,
    },
}

/// 任务重复策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepeatPolicy {
    /// 一次性任务
    Once,
    /// 固定间隔重复
    Interval {
        /// 间隔
        interval: std::time::Duration,
    },
    /// Cron 表达式（简化版：秒级精度）
    Cron {
        /// 每多少秒执行一次
        every_n_seconds: u64,
    },
}

/// 调度任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// 任务 ID
    pub id: TaskId,
    /// 首次/下次执行时间
    pub scheduled_at: Timestamp,
    /// 重复策略
    pub repeat: RepeatPolicy,
    /// 当前状态
    pub status: TaskStatus,
    /// 任务优先级（数值越大越优先）
    pub priority: i32,
    /// 任务描述（用于调试）
    pub label: String,
    /// 已执行次数
    pub fire_count: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_id_equality() {
        assert_eq!(TaskId(1), TaskId(1));
        assert_ne!(TaskId(1), TaskId(2));
    }

    #[test]
    fn test_task_id_display() {
        assert_eq!(format!("{}", TaskId(42)), "Task#42");
    }

    #[test]
    fn test_task_id_raw() {
        assert_eq!(TaskId(99).raw(), 99);
    }

    #[test]
    fn test_task_status_eq() {
        assert_eq!(TaskStatus::Pending, TaskStatus::Pending);
        assert_eq!(TaskStatus::Completed, TaskStatus::Completed);
        assert_ne!(TaskStatus::Pending, TaskStatus::Completed);
    }

    #[test]
    fn test_repeat_policy_eq() {
        assert_eq!(RepeatPolicy::Once, RepeatPolicy::Once);
        assert_eq!(
            RepeatPolicy::Interval {
                interval: std::time::Duration::from_millis(100)
            },
            RepeatPolicy::Interval {
                interval: std::time::Duration::from_millis(100)
            }
        );
    }
}

//! 任务回调上下文
//!
//! 该模块使用裸指针以避免生命周期约束；
//! `SchedulerContext` 仅在单线程事件循环中使用。

#![allow(unsafe_code)] // 裸指针用于跨生命周期的事件队列访问

use std::collections::HashMap;

use crate::queue::EventQueue;
use crate::time::Timestamp;

/// 调度器上下文：传递给任务回调的共享状态
///
/// 持有 `*mut EventQueue` 以便任务向队列注入事件；
/// 仅在单线程事件循环中使用，通过 `unsafe impl Send + Sync` 支持多线程 API 接收。
pub struct SchedulerContext {
    /// 当前模拟时间
    pub current_time: Timestamp,
    /// 事件队列引用（任务可向队列注入事件）
    pub event_queue: *mut EventQueue,
    /// 用户自定义状态
    pub user_data: HashMap<String, String>,
}

// Safety: SchedulerContext 仅在单线程事件循环中使用
unsafe impl Send for SchedulerContext {}
unsafe impl Sync for SchedulerContext {}

impl SchedulerContext {
    /// 创建新上下文（不绑定事件队列）
    pub fn new(current_time: Timestamp) -> Self {
        Self {
            current_time,
            event_queue: std::ptr::null_mut(),
            user_data: HashMap::new(),
        }
    }

    /// 绑定事件队列
    pub fn with_event_queue(mut self, queue: &mut EventQueue) -> Self {
        self.event_queue = queue as *mut EventQueue;
        self
    }

    /// 访问事件队列
    ///
    /// # Safety
    /// 调用方必须保证：
    /// - `event_queue` 指向有效的 `EventQueue`
    /// - 在 `Scheduler` 借用事件队列期间不会再次借用
    pub unsafe fn event_queue_mut(&mut self) -> &mut EventQueue {
        debug_assert!(!self.event_queue.is_null());
        &mut *self.event_queue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_new() {
        let ctx = SchedulerContext::new(Timestamp::from_nanos(1_000));
        assert_eq!(ctx.current_time, Timestamp::from_nanos(1_000));
        assert!(ctx.user_data.is_empty());
    }

    #[test]
    fn test_user_data_mutation() {
        let mut ctx = SchedulerContext::new(Timestamp::from_nanos(0));
        ctx.user_data.insert("key".into(), "value".into());
        assert_eq!(ctx.user_data.get("key").map(|s| s.as_str()), Some("value"));
    }
}

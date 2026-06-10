//! 任务回调接口

use super::context::SchedulerContext;

/// 任务回调函数接口
///
/// 闭包通过 [`ClosureCallback`] 适配。
pub trait TaskCallback: Send + Sync {
    /// 执行回调
    fn call(&self, ctx: &mut SchedulerContext);
}

/// 简单闭包包装器：将 `Fn(&mut SchedulerContext)` 转换为 [`TaskCallback`]
pub struct ClosureCallback<F: Fn(&mut SchedulerContext) + Send + Sync> {
    /// 闭包函数
    pub func: F,
}

impl<F: Fn(&mut SchedulerContext) + Send + Sync> TaskCallback for ClosureCallback<F> {
    fn call(&self, ctx: &mut SchedulerContext) {
        (self.func)(ctx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::Timestamp;

    #[test]
    fn test_closure_callback_invokes() {
        let callback = ClosureCallback {
            func: |ctx| {
                ctx.current_time = ctx.current_time.add(std::time::Duration::from_millis(10));
            },
        };
        let mut ctx = SchedulerContext::new(Timestamp::from_nanos(0));
        callback.call(&mut ctx);
        assert_eq!(ctx.current_time, Timestamp::from_millis(10));
    }

    #[test]
    fn test_closure_captures_environment() {
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let c = counter.clone();
        let callback = ClosureCallback {
            func: move |_ctx| {
                c.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            },
        };
        let mut ctx = SchedulerContext::new(Timestamp::from_nanos(0));
        callback.call(&mut ctx);
        callback.call(&mut ctx);
        assert_eq!(counter.load(std::sync::atomic::Ordering::Relaxed), 2);
    }
}

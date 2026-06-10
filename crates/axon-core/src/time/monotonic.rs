//! 单调时钟
//!
//! 基于 [`std::time::Instant`]，不受系统时间调整影响，**仅用于测量间隔**。
//! 不要将 `MonotonicClock::now()` 的返回值与 `Timestamp` 比较，二者无关联。

use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// 进程启动时的单调时钟起点（`OnceLock` 全局只初始化一次）
static EPOCH: OnceLock<Instant> = OnceLock::new();

/// 单调时钟类型
///
/// 提供纳秒精度的单调递增时间点，仅用于测量间隔（例如订单延迟、撮合耗时）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MonotonicClock {
    /// 内部使用 `Instant`，但对外仅暴露纳秒值，避免依赖 `Instant` 的具体语义
    inner: Instant,
}

impl MonotonicClock {
    /// 获取单调时钟当前值（自进程启动以来的纳秒数）
    ///
    /// 使用 `OnceLock` 缓存进程启动时刻，调用时仅需一次 `elapsed()`，无系统调用。
    #[inline]
    pub fn now() -> u64 {
        let start = EPOCH.get_or_init(Instant::now);
        start.elapsed().as_nanos() as u64
    }

    /// 测量闭包执行耗时，返回 `(结果, 耗时)`
    #[inline]
    pub fn measure<T, F>(f: F) -> (T, Duration)
    where
        F: FnOnce() -> T,
    {
        let start = Instant::now();
        let result = f();
        let elapsed = start.elapsed();
        (result, elapsed)
    }
}

impl Default for MonotonicClock {
    /// 默认值采用当前单调时间点
    fn default() -> Self {
        Self {
            inner: Instant::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_monotonic_now_is_non_decreasing() {
        let a = MonotonicClock::now();
        let b = MonotonicClock::now();
        assert!(b >= a, "单调时钟应非递减：a={a}, b={b}");
    }

    #[test]
    fn test_monotonic_now_advances_after_sleep() {
        let a = MonotonicClock::now();
        sleep(Duration::from_millis(2));
        let b = MonotonicClock::now();
        assert!(b > a, "sleep 后时间应推进：a={a}, b={b}");
    }

    #[test]
    fn test_measure_returns_elapsed_duration() {
        let (sum, elapsed) = MonotonicClock::measure(|| (0..1000u64).sum::<u64>());
        assert_eq!(sum, 499_500);
        // 耗时为正（不可能严格为 0）
        assert!(elapsed >= Duration::from_nanos(0));
    }
}

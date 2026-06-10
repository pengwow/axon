//! 模拟时钟

use serde::{Deserialize, Serialize};

use crate::time::Timestamp;

/// 模拟时钟
///
/// 回测场景下时间按事件推进，而非真实流逝。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimulatedClock {
    /// 当前时间
    current: Timestamp,
    /// 起始时间
    start: Timestamp,
    /// 结束时间（可选）
    end: Option<Timestamp>,
    /// 时间倍率（1.0 = 实时，>1 = 加速，<1 = 减速）
    time_scale: f64,
}

impl SimulatedClock {
    /// 创建新时钟
    pub fn new(start: Timestamp) -> Self {
        Self {
            current: start,
            start,
            end: None,
            time_scale: 1.0,
        }
    }

    /// 创建带结束时间的时钟
    pub fn with_end(start: Timestamp, end: Timestamp) -> Self {
        Self {
            current: start,
            start,
            end: Some(end),
            time_scale: 1.0,
        }
    }

    /// 当前时间
    #[inline]
    pub fn now(&self) -> Timestamp {
        self.current
    }

    /// 直接设置时间
    #[inline]
    pub fn set(&mut self, time: Timestamp) {
        self.current = time;
    }

    /// 推进指定时长
    #[inline]
    pub fn advance(&mut self, duration: std::time::Duration) {
        self.current = self.current.add(duration);
    }

    /// 时钟是否已到达结束时间
    pub fn is_exhausted(&self) -> bool {
        self.end.is_some_and(|end| self.current >= end)
    }

    /// 起始时间
    #[inline]
    pub fn start(&self) -> Timestamp {
        self.start
    }

    /// 结束时间
    #[inline]
    pub fn end(&self) -> Option<Timestamp> {
        self.end
    }

    /// 设置结束时间
    pub fn set_end(&mut self, end: Option<Timestamp>) {
        self.end = end;
    }

    /// 时间倍率
    #[inline]
    pub fn time_scale(&self) -> f64 {
        self.time_scale
    }

    /// 设置时间倍率
    #[inline]
    pub fn set_time_scale(&mut self, scale: f64) {
        self.time_scale = scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_new() {
        let c = SimulatedClock::new(Timestamp::from_nanos(0));
        assert_eq!(c.now(), Timestamp::from_nanos(0));
        assert!(!c.is_exhausted());
        assert_eq!(c.time_scale(), 1.0);
    }

    #[test]
    fn test_clock_advance() {
        let mut c = SimulatedClock::new(Timestamp::from_nanos(0));
        c.advance(std::time::Duration::from_millis(100));
        assert_eq!(c.now(), Timestamp::from_millis(100));
    }

    #[test]
    fn test_clock_set() {
        let mut c = SimulatedClock::new(Timestamp::from_nanos(0));
        c.set(Timestamp::from_millis(500));
        assert_eq!(c.now(), Timestamp::from_millis(500));
    }

    #[test]
    fn test_clock_exhausted() {
        let c = SimulatedClock::with_end(Timestamp::from_nanos(0), Timestamp::from_nanos(1_000));
        assert!(!c.is_exhausted());
        let mut c2 = c.clone();
        c2.set(Timestamp::from_nanos(2_000));
        assert!(c2.is_exhausted());
    }

    #[test]
    fn test_clock_time_scale() {
        let mut c = SimulatedClock::new(Timestamp::from_nanos(0));
        c.set_time_scale(2.0);
        assert_eq!(c.time_scale(), 2.0);
    }
}

//! 队列统计信息

use serde::{Deserialize, Serialize};

/// 队列统计
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueStats {
    /// 总入队事件数
    pub total_pushed: u64,
    /// 总出队事件数
    pub total_popped: u64,
    /// 被 `fast_forward` 跳过的事件数
    pub total_skipped: u64,
    /// 重放次数
    pub replay_count: u64,
}

impl QueueStats {
    /// 队列中剩余事件数（由调用方推算）
    #[inline]
    pub fn remaining(&self) -> i64 {
        self.total_pushed as i64 - self.total_popped as i64 - self.total_skipped as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_stats_are_zero() {
        let s = QueueStats::default();
        assert_eq!(s.total_pushed, 0);
        assert_eq!(s.total_popped, 0);
        assert_eq!(s.total_skipped, 0);
        assert_eq!(s.replay_count, 0);
    }

    #[test]
    fn test_remaining_calc() {
        let s = QueueStats {
            total_pushed: 100,
            total_popped: 30,
            total_skipped: 20,
            replay_count: 0,
        };
        assert_eq!(s.remaining(), 50);
    }
}

//! 队列条目：封装事件 + 排序元数据

use std::cmp::Ordering;

use serde::{Deserialize, Serialize};

use crate::event::Event;
use crate::time::Timestamp;

/// 事件队列条目：封装事件 + 排序元数据
///
/// BinaryHeap 是最大堆；通过反转 `Ord` 实现最小堆语义。
/// 排序规则：`timestamp` 升序 → `seq` 升序。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedEvent {
    /// 事件发生时间
    pub timestamp: Timestamp,
    /// 序列号：同一时间戳内按此排序
    pub seq: u64,
    /// 事件载荷
    pub event: Event,
}

impl PartialOrd for QueuedEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for QueuedEvent {
    fn cmp(&self, other: &Self) -> Ordering {
        // 反转比较以实现最小堆（BinaryHeap 是最大堆）
        other
            .timestamp
            .cmp(&self.timestamp)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::system::SystemAction;
    use crate::event::SystemEvent;
    use std::collections::BinaryHeap;

    fn make_event(seq: u64, ts_nanos: i64) -> QueuedEvent {
        QueuedEvent {
            timestamp: Timestamp::from_nanos(ts_nanos),
            seq,
            event: Event::System(SystemEvent::new(
                seq,
                Timestamp::from_nanos(ts_nanos),
                SystemAction::Heartbeat,
            )),
        }
    }

    #[test]
    fn test_ord_earlier_timestamp_is_less() {
        let mut heap = BinaryHeap::new();
        heap.push(make_event(0, 1_000));
        heap.push(make_event(0, 500));
        // 弹出最小（最早时间）
        let first = heap.pop().unwrap();
        assert_eq!(first.timestamp, Timestamp::from_nanos(500));
    }

    #[test]
    fn test_ord_same_timestamp_smaller_seq_first() {
        let mut heap = BinaryHeap::new();
        heap.push(make_event(2, 1_000));
        heap.push(make_event(1, 1_000));
        // 弹出最小（同一时间戳内 seq 较小者）
        let first = heap.pop().unwrap();
        assert_eq!(first.seq, 1);
    }

    #[test]
    fn test_partial_eq() {
        let a = make_event(1, 1_000);
        let b = make_event(1, 1_000);
        assert_eq!(a, b);
    }
}

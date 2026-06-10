//! 成交事件

use serde::{Deserialize, Serialize};

use crate::market::Trade;
use crate::time::Timestamp;

/// 成交事件
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FillEvent {
    /// 事件序列号
    pub seq: u64,
    /// 事件时间戳
    pub timestamp: Timestamp,
    /// 成交记录
    pub trade: Trade,
}

impl FillEvent {
    /// 创建成交事件
    pub fn new(seq: u64, timestamp: Timestamp, trade: Trade) -> Self {
        Self {
            seq,
            timestamp,
            trade,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Price, Quantity};

    #[test]
    fn test_fill_event_creation() {
        let ts = Timestamp::from_nanos(1_000);
        let trade = Trade::new(ts, Price::from_f64(100.0), Quantity::from_f64(1.0), 1, 2);
        let event = FillEvent::new(0, ts, trade);
        assert_eq!(event.seq, 0);
        assert_eq!(event.trade.price, Price::from_f64(100.0));
    }
}

//! 市场数据事件

use serde::{Deserialize, Serialize};

use crate::market::{Bar, OrderBookSnapshot, Tick};
use crate::time::Timestamp;

/// 市场数据事件
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketDataEvent {
    /// 事件序列号
    pub seq: u64,
    /// 事件时间戳
    pub timestamp: Timestamp,
    /// 事件载荷
    pub payload: MarketDataPayload,
}

/// 市场数据事件载荷
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum MarketDataPayload {
    /// 逐笔成交
    Tick(Tick),
    /// K线
    Bar(Bar),
    /// 订单簿快照
    OrderBookSnapshot(OrderBookSnapshot),
}

impl MarketDataEvent {
    /// 创建市场数据事件
    pub fn new(seq: u64, timestamp: Timestamp, payload: MarketDataPayload) -> Self {
        Self {
            seq,
            timestamp,
            payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::Side;
    use crate::types::{Price, Quantity};

    #[test]
    fn test_market_data_event_creation() {
        let ts = Timestamp::from_nanos(1_000);
        let tick = Tick::new(
            ts,
            Price::from_f64(100.0),
            Quantity::from_f64(10.0),
            Side::Buy,
        );
        let event = MarketDataEvent::new(0, ts, MarketDataPayload::Tick(tick));
        assert_eq!(event.seq, 0);
        assert_eq!(event.timestamp, ts);
    }
}

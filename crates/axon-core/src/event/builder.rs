//! 事件构建器（类型安全的事件创建 + 自增序列号）

use super::fill::FillEvent;
use super::market::{MarketDataEvent, MarketDataPayload};
use super::order::{OrderAction, OrderEvent};
use super::system::{SystemAction, SystemEvent};
use super::types::Event;
use crate::market::Trade;
use crate::order::OrderId;
use crate::time::Timestamp;

/// 事件构建器
///
/// 持有自增序列号，提供 4 种事件类型的便捷构造方法。
pub struct EventBuilder {
    /// 下一个待分配的序列号
    next_seq: u64,
}

impl EventBuilder {
    /// 创建事件构建器
    pub fn new(start_seq: u64) -> Self {
        Self {
            next_seq: start_seq,
        }
    }

    /// 构建市场数据事件（序列号自增）
    pub fn market_data(&mut self, timestamp: Timestamp, payload: MarketDataPayload) -> Event {
        let seq = self.next_seq;
        self.next_seq += 1;
        Event::MarketData(MarketDataEvent::new(seq, timestamp, payload))
    }

    /// 构建订单事件
    pub fn order(&mut self, timestamp: Timestamp, order_id: OrderId, action: OrderAction) -> Event {
        let seq = self.next_seq;
        self.next_seq += 1;
        Event::Order(OrderEvent {
            seq,
            timestamp,
            order_id,
            action,
        })
    }

    /// 构建成交事件
    pub fn fill(&mut self, timestamp: Timestamp, trade: Trade) -> Event {
        let seq = self.next_seq;
        self.next_seq += 1;
        Event::Fill(FillEvent::new(seq, timestamp, trade))
    }

    /// 构建系统事件
    pub fn system(&mut self, timestamp: Timestamp, action: SystemAction) -> Event {
        let seq = self.next_seq;
        self.next_seq += 1;
        Event::System(SystemEvent::new(seq, timestamp, action))
    }

    /// 获取下一个待分配的序列号
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    /// 获取当前已分配的最大序列号（`next_seq - 1`）
    pub fn current_seq(&self) -> u64 {
        self.next_seq.saturating_sub(1)
    }
}

impl Default for EventBuilder {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::market::{Side, Tick};
    use crate::types::{Price, Quantity};

    #[test]
    fn test_builder_seq_monotonic() {
        let mut b = EventBuilder::new(10);
        let ts = Timestamp::from_nanos(0);
        let e1 = b.market_data(
            ts,
            MarketDataPayload::Tick(Tick::new(
                ts,
                Price::from_f64(100.0),
                Quantity::from_f64(1.0),
                Side::Buy,
            )),
        );
        assert_eq!(e1.seq(), 10);
        let e2 = b.system(ts, SystemAction::Heartbeat);
        assert_eq!(e2.seq(), 11);
        let e3 = b.system(ts, SystemAction::Heartbeat);
        assert_eq!(e3.seq(), 12);
        assert_eq!(b.next_seq(), 13);
        assert_eq!(b.current_seq(), 12);
    }

    #[test]
    fn test_builder_default() {
        let b = EventBuilder::default();
        assert_eq!(b.next_seq(), 0);
    }

    #[test]
    fn test_builder_event_type() {
        let mut b = EventBuilder::new(0);
        let ts = Timestamp::from_nanos(0);
        assert_eq!(
            b.market_data(
                ts,
                MarketDataPayload::Tick(Tick::new(
                    ts,
                    Price::from_f64(100.0),
                    Quantity::from_f64(1.0),
                    Side::Buy,
                ))
            )
            .event_type(),
            super::super::types::EventType::MARKET_DATA
        );
        assert_eq!(
            b.system(ts, SystemAction::Heartbeat).event_type(),
            super::super::types::EventType::SYSTEM
        );
    }
}

//! 事件处理器 trait
//!
//! [`EventHandler`] 定义事件订阅者接口，
//! 通过 [`EventType`] 位掩码声明感兴趣的事件分类。

use super::types::{Event, EventType};

/// 事件处理器 trait
pub trait EventHandler {
    /// 处理单个事件
    fn on_event(&mut self, event: &Event);

    /// 返回处理器感兴趣的事件类型位掩码
    fn event_types(&self) -> EventType;

    /// 是否对指定事件类型感兴趣
    fn is_interested(&self, event_type: EventType) -> bool {
        self.event_types().contains(event_type)
    }

    /// 批量处理事件
    ///
    /// 默认实现按 `is_interested` 过滤后逐个调用 `on_event`。
    fn on_events(&mut self, events: &[Event]) {
        for event in events {
            if self.is_interested(event.event_type()) {
                self.on_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::market::MarketDataPayload;
    use crate::market::{Side, Tick};
    use crate::time::Timestamp;
    use crate::types::{Price, Quantity};

    /// 测试用处理器：只关心市场数据 Tick，记录价格
    struct TickRecorder {
        prices: Vec<f64>,
    }

    impl TickRecorder {
        fn new() -> Self {
            Self { prices: Vec::new() }
        }
    }

    impl EventHandler for TickRecorder {
        fn on_event(&mut self, event: &Event) {
            if let Event::MarketData(md) = event {
                if let MarketDataPayload::Tick(tick) = &md.payload {
                    self.prices.push(tick.price.as_f64());
                }
            }
        }

        fn event_types(&self) -> EventType {
            EventType::MARKET_DATA
        }
    }

    fn make_tick_event(ts: Timestamp, price: f64) -> Event {
        Event::MarketData(crate::event::market::MarketDataEvent::new(
            0,
            ts,
            MarketDataPayload::Tick(Tick::new(
                ts,
                Price::from_f64(price),
                Quantity::from_f64(1.0),
                Side::Buy,
            )),
        ))
    }

    #[test]
    fn test_event_types_mask() {
        let handler = TickRecorder::new();
        let mask = handler.event_types();
        assert!(mask.contains(EventType::MARKET_DATA));
        assert!(!mask.contains(EventType::ORDER));
    }

    #[test]
    fn test_is_interested_default() {
        let handler = TickRecorder::new();
        assert!(handler.is_interested(EventType::MARKET_DATA));
        assert!(!handler.is_interested(EventType::ORDER));
        assert!(!handler.is_interested(EventType::FILL));
        assert!(!handler.is_interested(EventType::SYSTEM));
    }

    #[test]
    fn test_on_event_records_tick() {
        let mut handler = TickRecorder::new();
        handler.on_event(&make_tick_event(Timestamp::from_nanos(0), 100.0));
        handler.on_event(&make_tick_event(Timestamp::from_nanos(1_000), 101.0));
        assert_eq!(handler.prices, vec![100.0, 101.0]);
    }

    #[test]
    fn test_on_events_batch_filter() {
        let mut handler = TickRecorder::new();
        let events = vec![
            make_tick_event(Timestamp::from_nanos(0), 100.0),
            Event::System(crate::event::system::SystemEvent::new(
                1,
                Timestamp::from_nanos(1_000),
                crate::event::system::SystemAction::Heartbeat,
            )),
            make_tick_event(Timestamp::from_nanos(2_000), 102.0),
        ];
        handler.on_events(&events);
        // TickRecorder 只记录 Tick 事件，System 事件被过滤
        assert_eq!(handler.prices, vec![100.0, 102.0]);
    }
}

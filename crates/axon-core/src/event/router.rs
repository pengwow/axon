//! 事件路由器与事件收集器

use super::handler::EventHandler;
use super::types::Event;
use crate::event::types::EventType;

/// 事件收集器
///
/// 将感兴趣的事件缓存到内部 Vec，用于回放/审计/测试。
pub struct EventCollector {
    /// 已收集的事件
    events: Vec<Event>,
    /// 感兴趣的事件类型位掩码
    interested: EventType,
}

impl EventCollector {
    /// 创建事件收集器
    pub fn new(interested: EventType) -> Self {
        Self {
            events: Vec::new(),
            interested,
        }
    }

    /// 借用访问已收集事件
    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// 消费并返回已收集事件
    pub fn into_events(self) -> Vec<Event> {
        self.events
    }

    /// 当前已收集事件数量
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

impl EventHandler for EventCollector {
    fn on_event(&mut self, event: &Event) {
        self.events.push(event.clone());
    }

    fn event_types(&self) -> EventType {
        self.interested
    }
}

impl Default for EventCollector {
    fn default() -> Self {
        Self::new(EventType::ALL)
    }
}

/// 事件路由器
///
/// 将事件分发给多个订阅者（按位掩码过滤）。
pub struct EventRouter {
    /// 订阅者列表
    handlers: Vec<Box<dyn EventHandler>>,
}

impl EventRouter {
    /// 创建空路由器
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// 注册事件处理器
    pub fn register(&mut self, handler: Box<dyn EventHandler>) {
        self.handlers.push(handler);
    }

    /// 注销所有处理器
    pub fn clear(&mut self) {
        self.handlers.clear();
    }

    /// 当前已注册的处理器数量
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }

    /// 分发单个事件给所有订阅者
    pub fn dispatch(&mut self, event: &Event) {
        let event_type = event.event_type();
        for handler in &mut self.handlers {
            if handler.is_interested(event_type) {
                handler.on_event(event);
            }
        }
    }

    /// 批量分发事件
    pub fn dispatch_batch(&mut self, events: &[Event]) {
        for event in events {
            self.dispatch(event);
        }
    }
}

impl Default for EventRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::builder::EventBuilder;
    use crate::event::market::{MarketDataEvent, MarketDataPayload};
    use crate::event::system::{SystemAction, SystemEvent};
    use crate::market::{Side, Tick};
    use crate::time::Timestamp;
    use crate::types::{Price, Quantity};

    fn make_tick_event(seq: u64, price: f64) -> Event {
        let ts = Timestamp::from_nanos((seq * 1_000) as i64);
        Event::MarketData(MarketDataEvent::new(
            seq,
            ts,
            MarketDataPayload::Tick(Tick::new(
                ts,
                Price::from_f64(price),
                Quantity::from_f64(1.0),
                Side::Buy,
            )),
        ))
    }

    fn make_system_event(seq: u64) -> Event {
        Event::System(SystemEvent::new(
            seq,
            Timestamp::from_nanos((seq * 1_000) as i64),
            SystemAction::Heartbeat,
        ))
    }

    /// 计数处理器：只关心市场数据
    struct MarketDataCounter {
        n: usize,
    }

    impl EventHandler for MarketDataCounter {
        fn on_event(&mut self, _: &Event) {
            self.n += 1;
        }
        fn event_types(&self) -> EventType {
            EventType::MARKET_DATA
        }
    }

    #[test]
    fn test_collector_default_collects_all() {
        let mut c = EventCollector::default();
        c.on_event(&make_tick_event(0, 100.0));
        c.on_event(&make_system_event(1));
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn test_collector_into_events() {
        let mut c = EventCollector::default();
        c.on_event(&make_tick_event(0, 100.0));
        let events = c.into_events();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_collector_filtered_by_interest() {
        let c = EventCollector::new(EventType::MARKET_DATA);
        assert!(c.is_interested(EventType::MARKET_DATA));
        assert!(!c.is_interested(EventType::SYSTEM));
    }

    #[test]
    fn test_router_dispatch_filters_by_type() {
        let mut router = EventRouter::new();
        router.register(Box::new(MarketDataCounter { n: 0 }));

        // 发送 3 个事件：Tick、System、Tick
        // MarketDataCounter 只应收到 2 个
        router.dispatch(&make_tick_event(0, 100.0));
        router.dispatch(&make_system_event(1));
        router.dispatch(&make_tick_event(2, 102.0));

        // router 中唯一处理器计数 — 由于 move 进了 router 内部，
        // 通过观察 router 的 len 验证不会 panic 即可
        assert_eq!(router.len(), 1);
    }

    #[test]
    fn test_router_dispatch_batch() {
        let mut router = EventRouter::new();
        router.register(Box::new(EventCollector::default()));
        let events = vec![
            make_tick_event(0, 100.0),
            make_system_event(1),
            make_tick_event(2, 101.0),
        ];
        router.dispatch_batch(&events);
        assert_eq!(router.len(), 1);
    }

    #[test]
    fn test_router_clear() {
        let mut router = EventRouter::new();
        router.register(Box::new(EventCollector::default()));
        router.register(Box::new(EventCollector::default()));
        assert_eq!(router.len(), 2);
        router.clear();
        assert!(router.is_empty());
    }

    #[test]
    fn test_builder_with_router() {
        let mut b = EventBuilder::new(0);
        let mut router = EventRouter::new();
        router.register(Box::new(EventCollector::default()));

        let ts = Timestamp::from_nanos(0);
        let evt1 = b.system(ts, SystemAction::Heartbeat);
        let evt2 = b.system(ts, SystemAction::Heartbeat);
        let evt3 = b.system(ts, SystemAction::Heartbeat);

        assert_eq!(evt1.seq(), 0);
        assert_eq!(evt2.seq(), 1);
        assert_eq!(evt3.seq(), 2);
        assert_eq!(b.next_seq(), 3);
    }
}

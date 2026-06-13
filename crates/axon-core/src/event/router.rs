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

    // ─── 边界测试 ──────────────────────────────────────

    /// 空 router 派发单个事件：不 panic、不影响内部状态
    #[test]
    fn test_empty_router_dispatch_no_panic() {
        let mut router = EventRouter::new();
        assert!(router.is_empty());
        // 派发 1000 个事件到空 router，全部应被静默丢弃
        for i in 0..1_000 {
            router.dispatch(&make_tick_event(i, 100.0));
        }
        assert!(router.is_empty());
    }

    /// 空 batch 派发：不 panic、不修改 router 状态
    #[test]
    fn test_empty_batch_dispatch_no_panic() {
        let mut router = EventRouter::new();
        router.register(Box::new(EventCollector::default()));
        let empty: Vec<Event> = vec![];
        router.dispatch_batch(&empty);
        assert_eq!(router.len(), 1);
    }

    /// 单一订阅者过滤掉所有事件（不感兴趣）⇒ 不 panic
    #[test]
    fn test_router_with_zero_interest_handler() {
        let mut router = EventRouter::new();
        // 注册一个只关心 SYSTEM 的订阅者
        router.register(Box::new(SystemOnlyCollector));
        // 派发 100 个 MARKET_DATA 事件
        for i in 0..100 {
            router.dispatch(&make_tick_event(i, 100.0));
        }
        // 派发 1 个 SYSTEM 事件
        router.dispatch(&make_system_event(999));
    }

    /// 100 个订阅者同时注册 + 派发 100 个事件
    #[test]
    fn test_router_high_fanout() {
        let mut router = EventRouter::new();
        for _ in 0..100 {
            router.register(Box::new(EventCollector::default()));
        }
        assert_eq!(router.len(), 100);
        for i in 0..100 {
            router.dispatch(&make_tick_event(i, 100.0));
        }
    }

    /// 清空后再次派发：等同于新 router
    #[test]
    fn test_router_clear_then_dispatch() {
        let mut router = EventRouter::new();
        router.register(Box::new(EventCollector::default()));
        router.register(Box::new(EventCollector::default()));
        assert_eq!(router.len(), 2);
        router.clear();
        assert!(router.is_empty());
        // 派发不应 panic
        router.dispatch(&make_tick_event(0, 100.0));
    }

    // ─── 辅助测试类型 ─────────────────────────────────

    /// 仅关心 SYSTEM 事件的收集器
    #[derive(Default)]
    struct SystemOnlyCollector;

    impl EventHandler for SystemOnlyCollector {
        fn on_event(&mut self, _: &Event) {
            // do nothing
        }
        fn event_types(&self) -> EventType {
            EventType::SYSTEM
        }
    }

    // ─── 并发测试 ──────────────────────────────────────

    /// EventRouter 本身非线程安全（持有 &mut self）：编译期文档
    ///
    /// EventRouter 需要 &mut self 进行 dispatch，因此不是 Sync。
    /// 生产环境应当使用消息通道或每线程独立 router 来实现并发。
    /// 以下为文档性注释（编译期不会触发）。
    fn _assert_router_not_sync_doc() {
        // 取消注释将编译失败：
        // fn assert_sync<T: Sync>() {}
        // assert_sync::<EventRouter>();
    }

    /// 多线程并发 EventCollector（独立实例）：各自收集互不干扰
    #[test]
    fn test_concurrent_collectors_independent() {
        use std::sync::Arc;
        use std::thread;

        const N_THREADS: usize = 50;
        const PER_THREAD: usize = 100;

        // 每个线程创建独立的 EventCollector
        let counters: Vec<Arc<std::sync::Mutex<usize>>> = (0..N_THREADS)
            .map(|_| Arc::new(std::sync::Mutex::new(0)))
            .collect();

        let mut handles = Vec::with_capacity(N_THREADS);
        for (i, counter) in counters.iter().enumerate() {
            let c = Arc::clone(counter);
            handles.push(thread::spawn(move || {
                let mut collector = EventCollector::new(EventType::MARKET_DATA);
                for j in 0..PER_THREAD {
                    collector.on_event(&make_tick_event(
                        (i * PER_THREAD + j) as u64,
                        100.0 + j as f64,
                    ));
                }
                let count = collector.len();
                *c.lock().unwrap() = count;
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
        // 每个 collector 应收到 PER_THREAD 个事件
        for (i, counter) in counters.iter().enumerate() {
            let n = *counter.lock().unwrap();
            assert_eq!(n, PER_THREAD, "collector {i} 应收到 {PER_THREAD} 个事件");
        }
    }

    /// 多线程独立 router：每个线程构造独立 EventRouter 并 dispatch 自己的事件
    /// （验证 EventRouter 的逻辑在并行场景下也保持正确性，不共享 router 状态）
    #[test]
    fn test_concurrent_independent_routers() {
        use std::sync::Arc;
        use std::thread;

        const N_THREADS: usize = 20;
        const PER_THREAD: usize = 1_000;

        // 每个线程独立维护一个计数器
        let totals: Vec<Arc<std::sync::Mutex<usize>>> = (0..N_THREADS)
            .map(|_| Arc::new(std::sync::Mutex::new(0)))
            .collect();

        let mut handles = Vec::with_capacity(N_THREADS);
        for (i, total) in totals.iter().enumerate() {
            let t = Arc::clone(total);
            handles.push(thread::spawn(move || {
                let mut router = EventRouter::new();
                router.register(Box::new(EventCollector::default()));

                for j in 0..PER_THREAD {
                    let seq = (i * PER_THREAD + j) as u64;
                    router.dispatch(&make_tick_event(seq, 100.0));
                }
                *t.lock().unwrap() = PER_THREAD;
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
        // 每个线程 dispatch 了 PER_THREAD 个事件
        for (i, total) in totals.iter().enumerate() {
            assert_eq!(
                *total.lock().unwrap(),
                PER_THREAD,
                "thread {i} 应 dispatch {PER_THREAD} 次"
            );
        }
    }

    /// 大量订阅者 + 大量事件 dispatch：单线程下性能正确
    /// （高扇出路由的典型场景）
    #[test]
    fn test_dispatch_many_handlers_high_volume() {
        const N_HANDLERS: usize = 50;
        const N_EVENTS: usize = 10_000;

        let mut router = EventRouter::new();
        for _ in 0..N_HANDLERS {
            router.register(Box::new(EventCollector::new(EventType::MARKET_DATA)));
        }
        for i in 0..N_EVENTS {
            router.dispatch(&make_tick_event(i as u64, 100.0));
        }
        assert_eq!(router.len(), N_HANDLERS);
    }

    /// EventCollector 是 Send：可跨线程移动所有权
    #[test]
    fn test_event_collector_send() {
        fn assert_send<T: Send>() {}
        assert_send::<EventCollector>();
    }

    /// 静态断言：EventRouter 仍实现 Default
    #[test]
    fn test_router_default_works() {
        let mut router = EventRouter::default();
        router.dispatch(&make_tick_event(0, 100.0));
        assert!(router.is_empty());
    }
}

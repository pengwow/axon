//! 队列延迟模型
//!
//! 模拟交易所处理队列：基础延迟 + 队列长度 × 单笔处理时间。
//! 需要外部驱动 `queue_length` 增减（enqueue / dequeue / set）。
//! 内部使用 `Mutex<usize>` 保护可变状态以满足 `Send + Sync`。

use std::sync::Mutex;
use std::time::Duration;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::traits::{LatencyModel, LatencyParams, PathType};

/// 队列延迟模型
///
/// 不同路径使用不同的基础延迟权重（订单提交最重、撤单次之、其余更轻）。
/// 当前队列长度通过 `enqueue` / `dequeue` / `set_queue_length` 维护。
#[derive(Debug)]
pub struct QueueLatencyModel {
    /// 基础延迟
    pub base_delay: Duration,
    /// 每个订单的处理时间
    pub processing_time: Duration,
    /// 最大队列长度
    pub max_queue_length: usize,
    /// 当前队列长度（受 Mutex 保护）
    queue_length: Mutex<usize>,
}

impl Clone for QueueLatencyModel {
    fn clone(&self) -> Self {
        // 共享队列状态的副本：使用独立 Mutex
        let len = *self.queue_length.lock().expect("queue mutex poisoned");
        Self {
            base_delay: self.base_delay,
            processing_time: self.processing_time,
            max_queue_length: self.max_queue_length,
            queue_length: Mutex::new(len),
        }
    }
}

#[cfg(feature = "serde")]
impl Serialize for QueueLatencyModel {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let len = *self.queue_length.lock().expect("queue mutex poisoned");
        let base_ms = self.base_delay.as_secs_f64() * 1000.0;
        let proc_ms = self.processing_time.as_secs_f64() * 1000.0;
        let mut state = serializer.serialize_struct("QueueLatencyModel", 4)?;
        state.serialize_field("base_delay_ms", &base_ms)?;
        state.serialize_field("processing_time_ms", &proc_ms)?;
        state.serialize_field("max_queue_length", &self.max_queue_length)?;
        state.serialize_field("queue_length", &len)?;
        state.end()
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for QueueLatencyModel {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "snake_case")]
        struct Raw {
            base_delay_ms: f64,
            processing_time_ms: f64,
            max_queue_length: usize,
            queue_length: usize,
        }

        let raw = Raw::deserialize(deserializer)?;
        Ok(Self {
            base_delay: Duration::from_secs_f64(raw.base_delay_ms / 1000.0),
            processing_time: Duration::from_secs_f64(raw.processing_time_ms / 1000.0),
            max_queue_length: raw.max_queue_length,
            queue_length: Mutex::new(raw.queue_length.min(raw.max_queue_length)),
        })
    }
}

impl QueueLatencyModel {
    /// 创建队列延迟模型（默认 `max_queue_length = 10000`）
    pub fn new(base_delay: Duration, processing_time: Duration) -> Self {
        Self {
            base_delay,
            processing_time,
            max_queue_length: 10_000,
            queue_length: Mutex::new(0),
        }
    }

    /// 设置最大队列长度
    pub fn with_max_queue_length(mut self, max: usize) -> Self {
        self.max_queue_length = max;
        self
    }

    /// 新订单到达，增加队列长度（不超过上限）
    pub fn enqueue(&self) {
        let mut guard = self.queue_length.lock().expect("queue mutex poisoned");
        *guard = guard.saturating_add(1).min(self.max_queue_length);
    }

    /// 订单处理完成，减少队列长度
    pub fn dequeue(&self) {
        let mut guard = self.queue_length.lock().expect("queue mutex poisoned");
        *guard = guard.saturating_sub(1);
    }

    /// 直接设置队列长度（不超过上限）
    pub fn set_queue_length(&self, length: usize) {
        let mut guard = self.queue_length.lock().expect("queue mutex poisoned");
        *guard = length.min(self.max_queue_length);
    }

    /// 获取当前队列长度
    pub fn queue_length(&self) -> usize {
        *self.queue_length.lock().expect("queue mutex poisoned")
    }

    /// 路径权重：OrderSubmit 满额，OrderCancel 半额，其余四分之一
    fn path_factor(&self, path: PathType) -> u32 {
        match path {
            PathType::OrderSubmit => 4,
            PathType::OrderCancel => 2,
            _ => 1,
        }
    }
}

impl LatencyModel for QueueLatencyModel {
    fn sample_delay(&self, path: PathType) -> Duration {
        let factor = self.path_factor(path);
        // base_delay × (factor / 4)，避免引入浮点
        let base = self.base_delay / 4 * factor;
        let queue = self.queue_length();
        let queue_delay = self.processing_time * queue as u32;
        base + queue_delay
    }

    fn name(&self) -> &str {
        "queue"
    }

    fn params(&self) -> LatencyParams {
        LatencyParams {
            model_type: "queue".to_string(),
            base_delay_ms: self.base_delay.as_secs_f64() * 1000.0,
            jitter_ms: Some(
                self.processing_time.as_secs_f64() * 1000.0 * self.queue_length() as f64,
            ),
            path_overrides: std::collections::HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_base_delay_under_light_load() {
        let model = QueueLatencyModel::new(Duration::from_millis(10), Duration::from_millis(1));
        // 轻负载时 OrderSubmit ≈ base_delay
        assert_eq!(
            model.sample_delay(PathType::OrderSubmit),
            Duration::from_millis(10)
        );
        // OrderCancel ≈ base_delay / 2
        assert_eq!(
            model.sample_delay(PathType::OrderCancel),
            Duration::from_millis(5)
        );
        // 其他 ≈ base_delay / 4
        assert_eq!(
            model.sample_delay(PathType::MarketData),
            Duration::from_millis(2) + Duration::from_micros(500)
        );
    }

    #[test]
    fn test_queue_delay_increases_with_length() {
        let model = QueueLatencyModel::new(Duration::from_millis(10), Duration::from_millis(1));
        let d0 = model.sample_delay(PathType::OrderSubmit);
        model.enqueue();
        let d1 = model.sample_delay(PathType::OrderSubmit);
        model.enqueue();
        let d2 = model.sample_delay(PathType::OrderSubmit);
        // d0 < d1 < d2
        assert!(d0 < d1, "d0={d0:?} d1={d1:?}");
        assert!(d1 < d2, "d1={d1:?} d2={d2:?}");
    }

    #[test]
    fn test_queue_enqueue_dequeue() {
        let model = QueueLatencyModel::new(Duration::from_millis(0), Duration::from_millis(1));
        assert_eq!(model.queue_length(), 0);
        model.enqueue();
        model.enqueue();
        assert_eq!(model.queue_length(), 2);
        model.dequeue();
        assert_eq!(model.queue_length(), 1);
        model.dequeue();
        model.dequeue(); // 不会下溢
        assert_eq!(model.queue_length(), 0);
    }

    #[test]
    fn test_queue_max_length_cap() {
        let model = QueueLatencyModel::new(Duration::from_millis(0), Duration::from_millis(1))
            .with_max_queue_length(3);
        for _ in 0..10 {
            model.enqueue();
        }
        assert_eq!(model.queue_length(), 3);
    }

    #[test]
    fn test_queue_set_length() {
        let model = QueueLatencyModel::new(Duration::from_millis(0), Duration::from_millis(1));
        model.set_queue_length(100);
        assert_eq!(model.queue_length(), 100);
    }

    #[test]
    fn test_queue_set_length_respects_max() {
        let model = QueueLatencyModel::new(Duration::from_millis(0), Duration::from_millis(1))
            .with_max_queue_length(50);
        model.set_queue_length(999);
        assert_eq!(model.queue_length(), 50);
    }

    #[test]
    fn test_queue_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<QueueLatencyModel>();
    }

    #[test]
    fn test_name_and_params() {
        let model = QueueLatencyModel::new(Duration::from_millis(10), Duration::from_millis(1));
        model.set_queue_length(5);
        assert_eq!(model.name(), "queue");
        let p = model.params();
        assert_eq!(p.model_type, "queue");
        assert!((p.base_delay_ms - 10.0).abs() < 1e-9);
        assert!((p.jitter_ms.expect("jitter") - 5.0).abs() < 1e-9);
    }

    // ─── 边界测试 ──────────────────────────────────────────

    /// 零基础延迟 + 零处理时间 ⇒ 仅由队列长度贡献
    #[test]
    fn test_zero_base_zero_processing() {
        let model = QueueLatencyModel::new(Duration::ZERO, Duration::ZERO);
        assert_eq!(model.sample_delay(PathType::OrderSubmit), Duration::ZERO);
        model.enqueue();
        model.enqueue();
        model.enqueue();
        // queue=3, processing=0 ⇒ 0
        assert_eq!(model.sample_delay(PathType::OrderSubmit), Duration::ZERO);
    }

    /// 极大队列长度（接近 max）
    #[test]
    fn test_extreme_queue_length() {
        let model = QueueLatencyModel::new(Duration::from_millis(1), Duration::from_millis(1))
            .with_max_queue_length(usize::MAX);
        // 1000 万次 enqueue
        for _ in 0..10_000_000 {
            model.enqueue();
        }
        // 不应 panic，且行为可预测：被 max_queue_length 截断到 usize::MAX
        // usize 必然 <= usize::MAX ⇒ 验证 enqueue 在极端容量下不会溢出
        let _ = model.queue_length();
    }

    /// set_queue_length 超过 max ⇒ 截断到 max
    #[test]
    fn test_set_queue_length_too_large() {
        let model = QueueLatencyModel::new(Duration::from_millis(1), Duration::from_millis(1))
            .with_max_queue_length(100);
        model.set_queue_length(usize::MAX);
        assert_eq!(model.queue_length(), 100);
    }

    /// 满队列时再 dequeue 多次不会下溢
    #[test]
    fn test_dequeue_below_zero_safe() {
        let model = QueueLatencyModel::new(Duration::from_millis(1), Duration::from_millis(1));
        // 初始 0
        for _ in 0..1000 {
            model.dequeue();
        }
        assert_eq!(model.queue_length(), 0);
    }

    /// 序列化往返
    #[test]
    fn test_queue_serde_roundtrip() {
        let model = QueueLatencyModel::new(Duration::from_millis(10), Duration::from_millis(1));
        model.set_queue_length(5);
        let json = serde_json::to_string(&model).unwrap();
        let de: QueueLatencyModel = serde_json::from_str(&json).unwrap();
        assert_eq!(de.queue_length(), 5);
        // sample_delay 应保持一致
        assert_eq!(
            de.sample_delay(PathType::OrderSubmit),
            Duration::from_millis(15)
        );
    }

    /// 反序列化时 queue_length 超过 max ⇒ 截断
    #[test]
    fn test_deserialize_truncates_excessive_queue_length() {
        // 序列化一个 5/10 的 model
        let original = QueueLatencyModel::new(Duration::from_millis(10), Duration::from_millis(1))
            .with_max_queue_length(50);
        original.set_queue_length(5);
        let json = serde_json::to_string(&original).unwrap();
        // 篡改 json 让 queue_length 超过 max
        let json_padded = json.replace("\"queue_length\":5", "\"queue_length\":99999");
        let de: QueueLatencyModel = serde_json::from_str(&json_padded).unwrap();
        assert_eq!(de.queue_length(), 50);
    }

    /// 路径权重：MarketData/AccountQuery/Heartbeat 使用 factor=1
    #[test]
    fn test_path_factor_default_paths() {
        let model = QueueLatencyModel::new(Duration::from_millis(40), Duration::from_millis(1));
        // factor = 1 ⇒ base_delay / 4 = 10ms
        assert_eq!(
            model.sample_delay(PathType::MarketData),
            Duration::from_millis(10)
        );
        assert_eq!(
            model.sample_delay(PathType::AccountQuery),
            Duration::from_millis(10)
        );
        assert_eq!(
            model.sample_delay(PathType::Heartbeat),
            Duration::from_millis(10)
        );
    }

    /// 极小 processing_time
    #[test]
    fn test_epsilon_processing_time() {
        let model = QueueLatencyModel::new(Duration::from_millis(0), Duration::from_nanos(1));
        model.set_queue_length(10);
        // queue=10, processing=1ns ⇒ 10ns
        let d = model.sample_delay(PathType::OrderSubmit);
        assert_eq!(d, Duration::from_nanos(10));
    }

    /// Clone 复制状态
    #[test]
    fn test_clone_preserves_state() {
        let original = QueueLatencyModel::new(Duration::from_millis(10), Duration::from_millis(1));
        original.set_queue_length(7);
        let cloned = original.clone();
        // 独立状态
        cloned.dequeue();
        assert_eq!(original.queue_length(), 7);
        assert_eq!(cloned.queue_length(), 6);
    }

    // ─── 并发测试 ──────────────────────────────────────────

    /// 多线程并发 enqueue：100 个线程各 enqueue 100 次 ⇒ 队列长度 = 10000
    /// 验证 Mutex 保护的正确性，无数据竞争
    #[test]
    fn test_concurrent_enqueue() {
        use std::sync::Arc;
        use std::thread;

        const N_THREADS: usize = 100;
        const PER_THREAD: usize = 100;

        let model = Arc::new(
            QueueLatencyModel::new(Duration::from_millis(1), Duration::from_millis(1))
                .with_max_queue_length(N_THREADS * PER_THREAD + 100),
        );

        let mut handles = Vec::with_capacity(N_THREADS);
        for _ in 0..N_THREADS {
            let m = Arc::clone(&model);
            handles.push(thread::spawn(move || {
                for _ in 0..PER_THREAD {
                    m.enqueue();
                }
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
        assert_eq!(model.queue_length(), N_THREADS * PER_THREAD);
    }

    /// 多线程并发 dequeue：N 次 enqueue + M 次 dequeue ⇒ 队列长度 = N - M
    #[test]
    fn test_concurrent_enqueue_dequeue() {
        use std::sync::Arc;
        use std::thread;

        const N_ENQ: usize = 1_000;
        const N_DEQ: usize = 300;

        let model = Arc::new(
            QueueLatencyModel::new(Duration::from_millis(1), Duration::from_millis(1))
                .with_max_queue_length(usize::MAX),
        );

        // 先填满
        for _ in 0..N_ENQ {
            model.enqueue();
        }
        assert_eq!(model.queue_length(), N_ENQ);

        let mut handles = Vec::with_capacity(N_DEQ);
        for _ in 0..N_DEQ {
            let m = Arc::clone(&model);
            handles.push(thread::spawn(move || {
                m.dequeue();
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
        assert_eq!(model.queue_length(), N_ENQ - N_DEQ);
    }

    /// 多线程混合 enqueue + dequeue + sample_delay：不 panic 且结果一致
    #[test]
    fn test_concurrent_mixed_operations() {
        use std::sync::Arc;
        use std::thread;

        const N_PROD: usize = 50;
        const N_CONS: usize = 50;
        const N_SAMPLE: usize = 20;
        const PER_OP: usize = 100;

        let model = Arc::new(
            QueueLatencyModel::new(Duration::from_millis(10), Duration::from_millis(1))
                .with_max_queue_length(usize::MAX),
        );

        let mut handles = Vec::new();

        // 生产者
        for _ in 0..N_PROD {
            let m = Arc::clone(&model);
            handles.push(thread::spawn(move || {
                for _ in 0..PER_OP {
                    m.enqueue();
                }
            }));
        }
        // 消费者
        for _ in 0..N_CONS {
            let m = Arc::clone(&model);
            handles.push(thread::spawn(move || {
                for _ in 0..PER_OP {
                    m.dequeue();
                }
            }));
        }
        // 采样者
        for _ in 0..N_SAMPLE {
            let m = Arc::clone(&model);
            handles.push(thread::spawn(move || {
                for _ in 0..PER_OP {
                    // sample_delay 内部读取 queue_length ⇒ 也需 Mutex 保护
                    let d = m.sample_delay(PathType::OrderSubmit);
                    assert!(d >= Duration::from_millis(0));
                }
            }));
        }

        for h in handles {
            h.join().expect("thread panicked");
        }
        // 净变化 = 净 enqueue 数 - 净 dequeue 数。
        // 由于 producer/consumer 启动顺序与交错，consumer 可能尝试从空队列 dequeue
        // （saturating_sub 不会下溢），最终值 ≥ 0 且 ≤ N_PROD * PER_OP
        let final_len = model.queue_length();
        assert!(
            final_len <= N_PROD * PER_OP,
            "队列长度不可能超过累计 enqueue 数"
        );
    }

    /// 多线程并发 set_queue_length：最后一个写入生效
    #[test]
    fn test_concurrent_set_queue_length() {
        use std::sync::Arc;
        use std::thread;

        const N_THREADS: usize = 50;

        let model = Arc::new(
            QueueLatencyModel::new(Duration::from_millis(1), Duration::from_millis(1))
                .with_max_queue_length(usize::MAX),
        );

        let mut handles = Vec::with_capacity(N_THREADS);
        for i in 0..N_THREADS {
            let m = Arc::clone(&model);
            handles.push(thread::spawn(move || {
                m.set_queue_length(i * 10);
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
        // 最终队列长度应是 N_THREADS - 1 的 10 倍（0 到 N-1 中某个值被最后写入）
        let final_len = model.queue_length();
        assert_eq!(final_len % 10, 0);
        assert!(final_len < N_THREADS * 10);
    }

    /// 多线程并发 enqueue + max_queue_length 截断：实际长度不超过 max
    #[test]
    fn test_concurrent_enqueue_respects_max() {
        use std::sync::Arc;
        use std::thread;

        const N_THREADS: usize = 100;
        const PER_THREAD: usize = 1_000;
        const MAX: usize = 500;

        let model = Arc::new(
            QueueLatencyModel::new(Duration::from_millis(1), Duration::from_millis(1))
                .with_max_queue_length(MAX),
        );

        let mut handles = Vec::with_capacity(N_THREADS);
        for _ in 0..N_THREADS {
            let m = Arc::clone(&model);
            handles.push(thread::spawn(move || {
                for _ in 0..PER_THREAD {
                    m.enqueue();
                }
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
        // 严格不超过 max（不变量由 Mutex 序列化保证）
        assert!(model.queue_length() <= MAX);
        assert_eq!(model.queue_length(), MAX);
    }

    /// 多线程并发 sample_delay：高并发读取应一致
    /// （保证 sample_delay 内部对 queue_length 的读取是原子的）
    #[test]
    fn test_concurrent_sample_delay_consistent() {
        use std::sync::Arc;
        use std::thread;

        const N_SAMPLE_THREADS: usize = 100;
        const PER_THREAD: usize = 1_000;

        let model = Arc::new(
            QueueLatencyModel::new(Duration::from_millis(10), Duration::from_millis(1))
                .with_max_queue_length(usize::MAX),
        );
        // 固定队列长度 = 5
        model.set_queue_length(5);

        let mut handles = Vec::with_capacity(N_SAMPLE_THREADS);
        for _ in 0..N_SAMPLE_THREADS {
            let m = Arc::clone(&model);
            handles.push(thread::spawn(move || {
                for _ in 0..PER_THREAD {
                    let d = m.sample_delay(PathType::OrderSubmit);
                    // base=10ms, factor=4 ⇒ base/4*4 = 10ms
                    // queue=5, processing=1ms ⇒ +5ms = 15ms
                    assert_eq!(d, Duration::from_millis(15));
                }
            }));
        }
        for h in handles {
            h.join().expect("thread panicked");
        }
    }

    /// Send + Sync 静态断言
    #[test]
    fn test_queue_is_send_sync_static() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<QueueLatencyModel>();
    }
}

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
        let model = QueueLatencyModel::new(
            Duration::from_millis(10),
            Duration::from_millis(1),
        );
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
        let model = QueueLatencyModel::new(
            Duration::from_millis(10),
            Duration::from_millis(1),
        );
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
        let model = QueueLatencyModel::new(
            Duration::from_millis(0),
            Duration::from_millis(1),
        );
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
        let model = QueueLatencyModel::new(
            Duration::from_millis(0),
            Duration::from_millis(1),
        )
        .with_max_queue_length(3);
        for _ in 0..10 {
            model.enqueue();
        }
        assert_eq!(model.queue_length(), 3);
    }

    #[test]
    fn test_queue_set_length() {
        let model = QueueLatencyModel::new(
            Duration::from_millis(0),
            Duration::from_millis(1),
        );
        model.set_queue_length(100);
        assert_eq!(model.queue_length(), 100);
    }

    #[test]
    fn test_queue_set_length_respects_max() {
        let model = QueueLatencyModel::new(
            Duration::from_millis(0),
            Duration::from_millis(1),
        )
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
        let model = QueueLatencyModel::new(
            Duration::from_millis(10),
            Duration::from_millis(1),
        );
        model.set_queue_length(5);
        assert_eq!(model.name(), "queue");
        let p = model.params();
        assert_eq!(p.model_type, "queue");
        assert!((p.base_delay_ms - 10.0).abs() < 1e-9);
        assert!((p.jitter_ms.expect("jitter") - 5.0).abs() < 1e-9);
    }
}

//! Tracker 配置 + MetricBuffer

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use url::Url;

use crate::types::MetricEntry;

/// Tracker 后端配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TrackerBackend {
    /// MLflow 后端
    Mlflow {
        /// tracking URI（如 http://localhost:5000）
        tracking_uri: Url,
    },
    /// WandB 后端
    Wandb {
        /// 项目名
        project: String,
        /// 团队/用户名
        #[serde(default)]
        entity: Option<String>,
        /// API key
        #[serde(default)]
        api_key: Option<String>,
    },
    /// 本地文件系统
    Local {
        /// 基础目录
        base_dir: PathBuf,
    },
    /// 内存
    #[default]
    Memory,
}

/// 指标缓冲（批量刷新）
#[derive(Debug, Clone)]
pub struct MetricBuffer {
    /// 内部条目
    entries: Vec<MetricEntry>,
    /// 容量上限
    capacity: usize,
    /// 刷新间隔
    flush_interval: Duration,
    /// 上次刷新时间
    last_flush: SystemTime,
}

impl MetricBuffer {
    /// 创建新缓冲
    pub fn new(capacity: usize, flush_interval: Duration) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            capacity,
            flush_interval,
            last_flush: SystemTime::now(),
        }
    }

    /// 添加条目
    pub fn push(&mut self, entry: MetricEntry) {
        self.entries.push(entry);
    }

    /// 当前条目数
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 是否应刷新（容量满 或 间隔到达）
    pub fn should_flush(&self) -> bool {
        self.entries.len() >= self.capacity
            || self
                .last_flush
                .elapsed()
                .map(|d| d >= self.flush_interval)
                .unwrap_or(false)
    }

    /// 取出所有条目（不自动重置时间戳，由调用方重置）
    pub fn drain(&mut self) -> Vec<MetricEntry> {
        std::mem::take(&mut self.entries)
    }

    /// 强制重置 last_flush（drain 后调用）
    pub fn mark_flushed(&mut self) {
        self.last_flush = SystemTime::now();
    }
}

impl Default for MetricBuffer {
    fn default() -> Self {
        Self::new(1000, Duration::from_secs(30))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MetricValue;
    use std::thread::sleep;

    fn make_entry(key: &str, step: usize) -> MetricEntry {
        MetricEntry {
            key: key.to_string(),
            value: MetricValue::Scalar(step as f64),
            step,
            timestamp: SystemTime::now(),
        }
    }

    #[test]
    fn test_buffer_default() {
        let buf = MetricBuffer::default();
        assert_eq!(buf.capacity, 1000);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_buffer_push_and_drain() {
        let mut buf = MetricBuffer::new(10, Duration::from_secs(1));
        buf.push(make_entry("loss", 1));
        buf.push(make_entry("loss", 2));
        assert_eq!(buf.len(), 2);
        let entries = buf.drain();
        assert_eq!(entries.len(), 2);
        assert!(buf.is_empty());
    }

    #[test]
    fn test_buffer_should_flush_capacity() {
        let mut buf = MetricBuffer::new(2, Duration::from_secs(60));
        buf.push(make_entry("a", 1));
        assert!(!buf.should_flush());
        buf.push(make_entry("b", 2));
        assert!(buf.should_flush());
    }

    #[test]
    fn test_buffer_should_flush_interval() {
        let mut buf = MetricBuffer::new(1000, Duration::from_millis(10));
        // 等待超过 interval
        sleep(Duration::from_millis(20));
        buf.push(make_entry("a", 1));
        assert!(buf.should_flush());
    }

    #[test]
    fn test_tracker_backend_default() {
        match TrackerBackend::default() {
            TrackerBackend::Memory => {}
            _ => panic!("expected Memory"),
        }
    }

    #[test]
    fn test_tracker_backend_mlflow_serialize() {
        let backend = TrackerBackend::Mlflow {
            tracking_uri: "http://localhost:5000".parse().unwrap(),
        };
        let json = serde_json::to_string(&backend).unwrap();
        assert!(json.contains("mlflow"));
        assert!(json.contains("localhost"));
    }
}

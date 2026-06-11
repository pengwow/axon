//! 内存 Tracker（测试 / mock 用）

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::error::TrackerError;
use crate::tracker::ExperimentTracker;
use crate::types::{
    ArtifactInfo, ExperimentId, ImageFormat, MetricEntry, MetricValue, ParamValue, RunId, RunStatus,
};

/// 内存 Tracker
pub struct MemoryTracker {
    inner: Mutex<MemoryState>,
}

struct MemoryState {
    run_id: RunId,
    #[allow(dead_code)]
    experiment_id: ExperimentId,
    params: HashMap<String, ParamValue>,
    metrics: Vec<MetricEntry>,
    artifacts: Vec<ArtifactInfo>,
    tags: HashMap<String, String>,
    status: RunStatus,
}

impl MemoryTracker {
    /// 创建新 tracker（自动生成 run_id）
    pub fn new() -> Self {
        Self::with_ids(
            ExperimentId("exp_memory".to_string()),
            RunId(format!("run_{}", chrono::Utc::now().timestamp_millis())),
        )
    }

    /// 用指定 ID 创建
    pub fn with_ids(experiment_id: ExperimentId, run_id: RunId) -> Self {
        Self {
            inner: Mutex::new(MemoryState {
                run_id,
                experiment_id,
                params: HashMap::new(),
                metrics: Vec::new(),
                artifacts: Vec::new(),
                tags: HashMap::new(),
                status: RunStatus::Running,
            }),
        }
    }

    /// 获取所有指标
    pub fn get_metrics(&self) -> Vec<MetricEntry> {
        self.inner.lock().unwrap().metrics.clone()
    }

    /// 按 key 过滤指标
    pub fn get_metrics_by_key(&self, key: &str) -> Vec<MetricEntry> {
        self.inner
            .lock()
            .unwrap()
            .metrics
            .iter()
            .filter(|m| m.key == key)
            .cloned()
            .collect()
    }

    /// 获取参数
    pub fn get_param(&self, key: &str) -> Option<ParamValue> {
        self.inner.lock().unwrap().params.get(key).cloned()
    }

    /// 获取所有参数
    pub fn get_all_params(&self) -> HashMap<String, ParamValue> {
        self.inner.lock().unwrap().params.clone()
    }

    /// 获取当前状态
    pub fn get_status(&self) -> RunStatus {
        self.inner.lock().unwrap().status
    }

    /// 获取 run_id
    pub fn run_id(&self) -> RunId {
        self.inner.lock().unwrap().run_id.clone()
    }
}

impl Default for MemoryTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ExperimentTracker for MemoryTracker {
    fn log_param(&self, key: &str, value: &ParamValue) -> Result<(), TrackerError> {
        self.inner
            .lock()
            .unwrap()
            .params
            .insert(key.to_string(), value.clone());
        Ok(())
    }

    fn log_params(&self, params: &[(String, ParamValue)]) -> Result<(), TrackerError> {
        let mut state = self.inner.lock().unwrap();
        for (k, v) in params {
            state.params.insert(k.clone(), v.clone());
        }
        Ok(())
    }

    fn log_metric(&self, key: &str, value: f64, step: usize) -> Result<(), TrackerError> {
        let entry = MetricEntry {
            key: key.to_string(),
            value: MetricValue::Scalar(value),
            step,
            timestamp: std::time::SystemTime::now(),
        };
        self.inner.lock().unwrap().metrics.push(entry);
        Ok(())
    }

    fn log_histogram(&self, key: &str, values: &[f64], step: usize) -> Result<(), TrackerError> {
        let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let num_bins = 30.min(values.len());
        let step_size = if num_bins > 0 {
            (max - min) / num_bins as f64
        } else {
            1.0
        };
        let bins: Vec<f64> = (0..=num_bins).map(|i| min + i as f64 * step_size).collect();
        let entry = MetricEntry {
            key: key.to_string(),
            value: MetricValue::Histogram {
                values: values.to_vec(),
                bins,
            },
            step,
            timestamp: std::time::SystemTime::now(),
        };
        self.inner.lock().unwrap().metrics.push(entry);
        Ok(())
    }

    fn log_image(
        &self,
        key: &str,
        image: &[u8],
        format: ImageFormat,
        step: usize,
    ) -> Result<(), TrackerError> {
        let entry = MetricEntry {
            key: key.to_string(),
            value: MetricValue::Image {
                data: image.to_vec(),
                format,
                width: 0,
                height: 0,
            },
            step,
            timestamp: std::time::SystemTime::now(),
        };
        self.inner.lock().unwrap().metrics.push(entry);
        Ok(())
    }

    fn log_artifact(&self, name: &str, path: &Path) -> Result<(), TrackerError> {
        let metadata = std::fs::metadata(path)
            .map_err(|e| TrackerError::Io(format!("{path:?}: {e}")))?;
        let size = metadata.len();
        let content_hash = format!("{size:x}");
        let info = ArtifactInfo {
            name: name.to_string(),
            path: path.to_path_buf(),
            size_bytes: size,
            content_hash,
            timestamp: std::time::SystemTime::now(),
        };
        self.inner.lock().unwrap().artifacts.push(info);
        Ok(())
    }

    fn set_tag(&self, key: &str, value: &str) -> Result<(), TrackerError> {
        self.inner
            .lock()
            .unwrap()
            .tags
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn finish(&self, status: RunStatus) -> Result<(), TrackerError> {
        self.inner.lock().unwrap().status = status;
        Ok(())
    }

    fn flush(&self) -> Result<(), TrackerError> {
        Ok(()) // 内存模式无缓冲
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_param_and_get() {
        let t = MemoryTracker::new();
        t.log_param("lr", &ParamValue::Float(0.001)).unwrap();
        match t.get_param("lr").unwrap() {
            ParamValue::Float(v) => assert!((v - 0.001).abs() < 1e-9),
            _ => panic!("wrong type"),
        }
    }

    #[test]
    fn test_log_metrics() {
        let t = MemoryTracker::new();
        t.log_metric("loss", 0.5, 0).unwrap();
        t.log_metric("loss", 0.4, 1).unwrap();
        t.log_metric("acc", 0.9, 0).unwrap();
        let loss_metrics = t.get_metrics_by_key("loss");
        assert_eq!(loss_metrics.len(), 2);
    }

    #[test]
    fn test_log_histogram() {
        let t = MemoryTracker::new();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        t.log_histogram("weights", &values, 0).unwrap();
        assert_eq!(t.get_metrics().len(), 1);
    }

    #[test]
    fn test_set_tag_and_finish() {
        let t = MemoryTracker::new();
        t.set_tag("strategy", "ppo").unwrap();
        t.finish(RunStatus::Completed).unwrap();
        assert_eq!(t.get_status(), RunStatus::Completed);
    }

    #[test]
    fn test_log_artifact_existing_file() {
        let t = MemoryTracker::new();
        // 写入临时文件
        let tmp = std::env::temp_dir().join("axon_tracker_test.txt");
        std::fs::write(&tmp, b"hello world").unwrap();
        t.log_artifact("test", &tmp).unwrap();
        let metrics = t.get_metrics();
        // artifact 不在 metrics 中
        assert_eq!(metrics.len(), 0);
    }
}

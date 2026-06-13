//! 本地文件系统 Tracker（离线模式）

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::error::TrackerError;
use crate::tracker::ExperimentTracker;
use crate::types::{
    ArtifactInfo, ExperimentId, ImageFormat, MetricEntry, MetricValue, ParamValue, RunId, RunStatus,
};

/// 本地 Tracker（将所有数据写入 JSON 文件）
pub struct LocalTracker {
    inner: Mutex<LocalState>,
}

struct LocalState {
    base_dir: PathBuf,
    run_id: RunId,
    #[allow(dead_code)]
    experiment_id: ExperimentId,
    params: HashMap<String, ParamValue>,
    tags: HashMap<String, String>,
    metrics_buffer: Vec<MetricEntry>,
    artifacts: Vec<ArtifactInfo>,
    status: RunStatus,
}

impl LocalTracker {
    /// 创建本地 tracker
    pub fn new(base_dir: PathBuf) -> Result<Self, TrackerError> {
        std::fs::create_dir_all(&base_dir).map_err(|e| TrackerError::Io(e.to_string()))?;
        let run_id = RunId(format!(
            "run_{}",
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        ));
        let run_dir = base_dir.join(&run_id.0);
        std::fs::create_dir_all(&run_dir).map_err(|e| TrackerError::Io(e.to_string()))?;
        Ok(Self {
            inner: Mutex::new(LocalState {
                base_dir,
                run_id,
                experiment_id: ExperimentId("exp_local".to_string()),
                params: HashMap::new(),
                tags: HashMap::new(),
                metrics_buffer: Vec::new(),
                artifacts: Vec::new(),
                status: RunStatus::Running,
            }),
        })
    }

    /// 获取 run 目录
    pub fn run_dir(&self) -> PathBuf {
        let state = self.inner.lock().unwrap();
        state.base_dir.join(&state.run_id.0)
    }

    /// 获取 run_id
    pub fn run_id(&self) -> RunId {
        self.inner.lock().unwrap().run_id.clone()
    }

    fn write_params(state: &LocalState) -> Result<(), TrackerError> {
        let path = state.base_dir.join(&state.run_id.0).join("params.json");
        let json = serde_json::to_string_pretty(&state.params)
            .map_err(|e| TrackerError::Serialization(e.to_string()))?;
        std::fs::write(&path, json).map_err(|e| TrackerError::Io(e.to_string()))?;
        Ok(())
    }

    fn append_metrics(state: &LocalState) -> Result<(), TrackerError> {
        let path = state.base_dir.join(&state.run_id.0).join("metrics.jsonl");
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| TrackerError::Io(e.to_string()))?;
        for entry in &state.metrics_buffer {
            let line = serde_json::to_string(entry)
                .map_err(|e| TrackerError::Serialization(e.to_string()))?;
            writeln!(file, "{line}").map_err(|e| TrackerError::Io(e.to_string()))?;
        }
        Ok(())
    }
}

impl ExperimentTracker for LocalTracker {
    fn log_param(&self, key: &str, value: &ParamValue) -> Result<(), TrackerError> {
        let mut state = self.inner.lock().unwrap();
        state.params.insert(key.to_string(), value.clone());
        Self::write_params(&state)
    }

    fn log_params(&self, params: &[(String, ParamValue)]) -> Result<(), TrackerError> {
        let mut state = self.inner.lock().unwrap();
        for (k, v) in params {
            state.params.insert(k.clone(), v.clone());
        }
        Self::write_params(&state)
    }

    fn log_metric(&self, key: &str, value: f64, step: usize) -> Result<(), TrackerError> {
        let mut state = self.inner.lock().unwrap();
        state.metrics_buffer.push(MetricEntry {
            key: key.to_string(),
            value: MetricValue::Scalar(value),
            step,
            timestamp: std::time::SystemTime::now(),
        });
        Ok(())
    }

    fn log_histogram(&self, key: &str, values: &[f64], step: usize) -> Result<(), TrackerError> {
        let mut state = self.inner.lock().unwrap();
        state.metrics_buffer.push(MetricEntry {
            key: key.to_string(),
            value: MetricValue::Histogram {
                values: values.to_vec(),
                bins: vec![],
            },
            step,
            timestamp: std::time::SystemTime::now(),
        });
        Ok(())
    }

    fn log_image(
        &self,
        key: &str,
        image: &[u8],
        format: ImageFormat,
        step: usize,
    ) -> Result<(), TrackerError> {
        let state = self.inner.lock().unwrap();
        let dir = state.base_dir.join(&state.run_id.0).join("images");
        std::fs::create_dir_all(&dir).map_err(|e| TrackerError::Io(e.to_string()))?;
        let ext = match format {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Svg => "svg",
        };
        let path = dir.join(format!("{key}_step{step}.{ext}"));
        std::fs::write(&path, image).map_err(|e| TrackerError::Io(e.to_string()))?;
        Ok(())
    }

    fn log_artifact(&self, name: &str, path: &Path) -> Result<(), TrackerError> {
        let state = self.inner.lock().unwrap();
        let dest = state
            .base_dir
            .join(&state.run_id.0)
            .join("artifacts")
            .join(name);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| TrackerError::Io(e.to_string()))?;
        }
        let size = std::fs::copy(path, &dest).map_err(|e| TrackerError::Io(e.to_string()))?;
        drop(state);
        let mut state2 = self.inner.lock().unwrap();
        state2.artifacts.push(ArtifactInfo {
            name: name.to_string(),
            path: dest,
            size_bytes: size,
            content_hash: format!("{size:x}"),
            timestamp: std::time::SystemTime::now(),
        });
        Ok(())
    }

    fn set_tag(&self, key: &str, value: &str) -> Result<(), TrackerError> {
        let mut state = self.inner.lock().unwrap();
        state.tags.insert(key.to_string(), value.to_string());
        let path = state.base_dir.join(&state.run_id.0).join("tags.json");
        let json = serde_json::to_string_pretty(&state.tags)
            .map_err(|e| TrackerError::Serialization(e.to_string()))?;
        std::fs::write(&path, json).map_err(|e| TrackerError::Io(e.to_string()))?;
        Ok(())
    }

    fn finish(&self, status: RunStatus) -> Result<(), TrackerError> {
        self.flush()?;
        let mut state = self.inner.lock().unwrap();
        state.status = status;
        let path = state.base_dir.join(&state.run_id.0).join("status.json");
        let json = serde_json::json!({
            "status": state.status.as_mlflow_str(),
            "end_time": chrono::Utc::now().to_rfc3339(),
        })
        .to_string();
        std::fs::write(&path, json).map_err(|e| TrackerError::Io(e.to_string()))?;
        Ok(())
    }

    fn flush(&self) -> Result<(), TrackerError> {
        let mut state = self.inner.lock().unwrap();
        Self::append_metrics(&state)?;
        state.metrics_buffer.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ParamValue;

    #[test]
    fn test_log_param_writes_file() {
        let tmp = tempfile_dir();
        let t = LocalTracker::new(tmp.clone()).unwrap();
        t.log_param("lr", &ParamValue::Float(0.001)).unwrap();
        t.flush().unwrap();
        let params_path = t.run_dir().join("params.json");
        assert!(params_path.exists());
        let content = std::fs::read_to_string(&params_path).unwrap();
        assert!(content.contains("0.001"));
    }

    #[test]
    fn test_log_metrics_writes_jsonl() {
        let tmp = tempfile_dir();
        let t = LocalTracker::new(tmp.clone()).unwrap();
        t.log_metric("loss", 0.5, 0).unwrap();
        t.log_metric("loss", 0.4, 1).unwrap();
        t.flush().unwrap();
        let metrics_path = t.run_dir().join("metrics.jsonl");
        let content = std::fs::read_to_string(&metrics_path).unwrap();
        let lines: Vec<&str> = content.trim().split('\n').collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("loss"));
    }

    #[test]
    fn test_finish_writes_status() {
        let tmp = tempfile_dir();
        let t = LocalTracker::new(tmp.clone()).unwrap();
        t.finish(RunStatus::Completed).unwrap();
        let status_path = t.run_dir().join("status.json");
        assert!(status_path.exists());
        let content = std::fs::read_to_string(&status_path).unwrap();
        assert!(content.contains("FINISHED"));
    }

    fn tempfile_dir() -> PathBuf {
        let nanos = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("axon_tracker_{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}

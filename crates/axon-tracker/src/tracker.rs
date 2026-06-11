//! ExperimentTracker trait

use std::path::Path;

use crate::error::TrackerError;
use crate::types::{ImageFormat, ParamValue, RunStatus};

/// 实验追踪器统一接口
pub trait ExperimentTracker: Send + Sync {
    /// 记录单个参数
    fn log_param(&self, key: &str, value: &ParamValue) -> Result<(), TrackerError>;

    /// 批量记录参数
    fn log_params(&self, params: &[(String, ParamValue)]) -> Result<(), TrackerError>;

    /// 记录标量指标
    fn log_metric(&self, key: &str, value: f64, step: usize) -> Result<(), TrackerError>;

    /// 记录直方图
    fn log_histogram(
        &self,
        key: &str,
        values: &[f64],
        step: usize,
    ) -> Result<(), TrackerError>;

    /// 记录图像
    fn log_image(
        &self,
        key: &str,
        image: &[u8],
        format: ImageFormat,
        step: usize,
    ) -> Result<(), TrackerError>;

    /// 上传产物
    fn log_artifact(&self, name: &str, path: &Path) -> Result<(), TrackerError>;

    /// 设置标签
    fn set_tag(&self, key: &str, value: &str) -> Result<(), TrackerError>;

    /// 结束运行
    fn finish(&self, status: RunStatus) -> Result<(), TrackerError>;

    /// 刷新缓冲区
    fn flush(&self) -> Result<(), TrackerError>;
}

/// 扩展方法：批量记录标量指标
pub trait ExperimentTrackerExt: ExperimentTracker {
    /// 批量记录标量指标
    fn log_metrics(&self, metrics: &[(String, f64)], step: usize) -> Result<(), TrackerError> {
        for (k, v) in metrics {
            self.log_metric(k, *v, step)?;
        }
        Ok(())
    }
}

impl<T: ExperimentTracker + ?Sized> ExperimentTrackerExt for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backends::MemoryTracker;

    #[test]
    fn test_trait_object_compile() {
        let tracker = MemoryTracker::new();
        let boxed: Box<dyn ExperimentTracker> = Box::new(tracker);
        assert!(boxed.log_metric("test", 1.0, 0).is_ok());
    }

    #[test]
    fn test_log_metrics_ext() {
        let tracker = MemoryTracker::new();
        let metrics = vec![
            ("loss".to_string(), 0.5),
            ("accuracy".to_string(), 0.9),
        ];
        tracker.log_metrics(&metrics, 0).unwrap();
    }
}

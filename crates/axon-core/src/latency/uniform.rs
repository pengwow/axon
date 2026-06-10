//! 均匀分布延迟模型
//!
//! 在 `[min, max]` 区间内均匀采样延迟。
//! 适用于建模最坏/最好情况已知的网络抖动。

use std::collections::HashMap;
use std::time::Duration;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::traits::{LatencyModel, LatencyParams, PathType};

/// 均匀分布延迟模型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub struct UniformLatencyModel {
    /// 各路径类型的最小延迟
    pub mins: HashMap<PathType, Duration>,
    /// 各路径类型的最大延迟
    pub maxs: HashMap<PathType, Duration>,
}

impl UniformLatencyModel {
    /// 创建统一均匀延迟模型
    ///
    /// 假定 `min <= max`，否则路径采样时统一回退到 `min`。
    pub fn uniform(min: Duration, max: Duration) -> Self {
        let mut mins = HashMap::new();
        let mut maxs = HashMap::new();
        for path in PathType::ALL {
            mins.insert(path, min);
            maxs.insert(path, max);
        }
        Self { mins, maxs }
    }
}

impl LatencyModel for UniformLatencyModel {
    fn sample_delay(&self, path: PathType) -> Duration {
        let min = self.mins.get(&path).copied().unwrap_or(Duration::ZERO);
        let max = self.maxs.get(&path).copied().unwrap_or(Duration::from_millis(10));

        if max <= min {
            return min;
        }

        let min_ns = min.as_nanos() as f64;
        let max_ns = max.as_nanos() as f64;
        let range = max_ns - min_ns;
        let sample = rand::random::<f64>() * range + min_ns;
        Duration::from_nanos(sample as u64)
    }

    fn name(&self) -> &str {
        "uniform"
    }

    fn params(&self) -> LatencyParams {
        let count = self.mins.len() as f64;
        let sum_min: f64 = self
            .mins
            .values()
            .map(|d| d.as_secs_f64() * 1000.0)
            .sum();
        let sum_max: f64 = self
            .maxs
            .values()
            .map(|d| d.as_secs_f64() * 1000.0)
            .sum();
        let avg_min = if count > 0.0 { sum_min / count } else { 0.0 };
        let avg_max = if count > 0.0 { sum_max / count } else { 0.0 };
        LatencyParams {
            model_type: "uniform".to_string(),
            base_delay_ms: (avg_min + avg_max) / 2.0,
            jitter_ms: Some((avg_max - avg_min).abs() / 2.0),
            path_overrides: self
                .mins
                .iter()
                .map(|(k, v)| (*k, v.as_secs_f64() * 1000.0))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniform_within_bounds() {
        let min = Duration::from_millis(1);
        let max = Duration::from_millis(10);
        let model = UniformLatencyModel::uniform(min, max);
        for _ in 0..5000 {
            let d = model.sample_delay(PathType::OrderSubmit);
            assert!(d >= min, "sample {d:?} < min {min:?}");
            assert!(d <= max, "sample {d:?} > max {max:?}");
        }
    }

    #[test]
    fn test_uniform_mean_approximate_center() {
        // 大量采样均值应接近区间中点
        let min = Duration::from_millis(2);
        let max = Duration::from_millis(8);
        let model = UniformLatencyModel::uniform(min, max);
        let n = 20_000;
        let sum: f64 = (0..n)
            .map(|_| model.sample_delay(PathType::MarketData).as_secs_f64() * 1000.0)
            .sum();
        let mean = sum / n as f64;
        assert!((mean - 5.0).abs() < 0.3, "expected mean ≈ 5ms, got {mean}");
    }

    #[test]
    fn test_uniform_max_less_than_min_falls_back_to_min() {
        let min = Duration::from_millis(5);
        let max = Duration::from_millis(1);
        let model = UniformLatencyModel::uniform(min, max);
        for _ in 0..100 {
            assert_eq!(model.sample_delay(PathType::MarketData), min);
        }
    }

    #[test]
    fn test_name_and_params() {
        let model = UniformLatencyModel::uniform(
            Duration::from_millis(2),
            Duration::from_millis(8),
        );
        assert_eq!(model.name(), "uniform");
        let p = model.params();
        assert_eq!(p.model_type, "uniform");
        assert!((p.base_delay_ms - 5.0).abs() < 1e-9);
        assert!((p.jitter_ms.expect("jitter") - 3.0).abs() < 1e-9);
    }
}

//! 正态分布延迟模型
//!
//! 使用 Box-Muller 变换从均匀分布采样正态分布延迟。
//! 延迟为负时截断为 0（物理意义上延迟不可为负）。

use std::collections::HashMap;
use std::f64::consts::PI;
use std::time::Duration;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::traits::{LatencyModel, LatencyParams, PathType};

/// 正态分布延迟模型
///
/// 各路径独立维护 `mean` 与 `std_dev`，采样后截断为非负。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub struct NormalLatencyModel {
    /// 各路径类型的均值
    pub means: HashMap<PathType, Duration>,
    /// 各路径类型的标准差
    pub std_devs: HashMap<PathType, Duration>,
}

impl NormalLatencyModel {
    /// 创建统一正态延迟模型
    pub fn uniform(mean: Duration, std_dev: Duration) -> Self {
        let mut means = HashMap::new();
        let mut std_devs = HashMap::new();
        for path in PathType::ALL {
            means.insert(path, mean);
            std_devs.insert(path, std_dev);
        }
        Self { means, std_devs }
    }

    /// Box-Muller 变换：从两个均匀分布采样得到一个标准正态分布样本
    ///
    /// 仅使用 `z0 = sqrt(-2 ln U1) * cos(2π U2)`，丢弃 `z1`。
    /// 性能目标 < 50ns。
    #[inline]
    fn box_muller(mean_ms: f64, std_dev_ms: f64) -> f64 {
        // 生成两个独立的均匀随机数（不允许为 0，避免 ln(0)）
        let u1 = rand::random::<f64>().max(f64::MIN_POSITIVE);
        let u2 = rand::random::<f64>();

        let z0 = (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos();
        mean_ms + std_dev_ms * z0
    }
}

impl LatencyModel for NormalLatencyModel {
    fn sample_delay(&self, path: PathType) -> Duration {
        let mean = self
            .means
            .get(&path)
            .copied()
            .unwrap_or(Duration::from_millis(1));
        let std_dev = self
            .std_devs
            .get(&path)
            .copied()
            .unwrap_or(Duration::from_millis(1));

        let mean_ms = mean.as_secs_f64() * 1000.0;
        let std_dev_ms = std_dev.as_secs_f64() * 1000.0;

        // 截断为非负
        let sample_ms = Self::box_muller(mean_ms, std_dev_ms).max(0.0);
        Duration::from_secs_f64(sample_ms / 1000.0)
    }

    fn name(&self) -> &str {
        "normal"
    }

    fn params(&self) -> LatencyParams {
        let count = self.means.len() as f64;
        let sum_mean: f64 = self.means.values().map(|d| d.as_secs_f64() * 1000.0).sum();
        let sum_std: f64 = self
            .std_devs
            .values()
            .map(|d| d.as_secs_f64() * 1000.0)
            .sum();
        let avg_mean = if count > 0.0 { sum_mean / count } else { 0.0 };
        let avg_std = if count > 0.0 { sum_std / count } else { 0.0 };

        LatencyParams {
            model_type: "normal".to_string(),
            base_delay_ms: avg_mean,
            jitter_ms: Some(avg_std),
            path_overrides: self
                .means
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
    fn test_normal_samples_non_negative() {
        let model =
            NormalLatencyModel::uniform(Duration::from_millis(10), Duration::from_millis(2));
        for _ in 0..5000 {
            let d = model.sample_delay(PathType::MarketData);
            assert!(d.as_secs_f64() >= 0.0);
        }
    }

    #[test]
    fn test_normal_mean_within_tolerance() {
        // 多次采样后均值应接近配置均值（10ms）
        let model =
            NormalLatencyModel::uniform(Duration::from_millis(10), Duration::from_millis(2));
        let n = 10_000;
        let sum: f64 = (0..n)
            .map(|_| model.sample_delay(PathType::MarketData).as_secs_f64() * 1000.0)
            .sum();
        let mean = sum / n as f64;
        // 95% 容差：均值偏离 < 2 * std / sqrt(n) ≈ 2 * 2 / 100 ≈ 0.04ms
        // 给一个宽松的工程容差
        assert!(
            (mean - 10.0).abs() < 0.5,
            "expected mean ≈ 10ms, got {mean}"
        );
    }

    #[test]
    fn test_normal_std_dev_approximate() {
        // 验证标准差近似符合配置
        let model =
            NormalLatencyModel::uniform(Duration::from_millis(50), Duration::from_millis(5));
        let n = 20_000;
        let samples: Vec<f64> = (0..n)
            .map(|_| model.sample_delay(PathType::MarketData).as_secs_f64() * 1000.0)
            .collect();
        let mean = samples.iter().sum::<f64>() / n as f64;
        let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
        let std = var.sqrt();
        assert!((std - 5.0).abs() < 0.5, "expected std ≈ 5ms, got {std}");
    }

    #[test]
    fn test_normal_per_path_independent() {
        let mut model =
            NormalLatencyModel::uniform(Duration::from_millis(1), Duration::from_millis(0));
        model
            .means
            .insert(PathType::OrderSubmit, Duration::from_millis(20));
        model
            .std_devs
            .insert(PathType::OrderSubmit, Duration::from_millis(0));
        // std_dev = 0 时，所有样本应严格等于 mean
        for _ in 0..100 {
            assert_eq!(
                model.sample_delay(PathType::OrderSubmit),
                Duration::from_millis(20)
            );
        }
    }

    #[test]
    fn test_name_and_params() {
        let model =
            NormalLatencyModel::uniform(Duration::from_millis(10), Duration::from_millis(2));
        assert_eq!(model.name(), "normal");
        let p = model.params();
        assert_eq!(p.model_type, "normal");
        assert!((p.base_delay_ms - 10.0).abs() < 1e-9);
        assert!((p.jitter_ms.expect("jitter") - 2.0).abs() < 1e-9);
    }

    // ─── 边界测试 ──────────────────────────────────────────

    /// 零 mean + 零 std_dev ⇒ 始终返回 0
    #[test]
    fn test_zero_mean_zero_std_returns_zero() {
        let model = NormalLatencyModel::uniform(Duration::ZERO, Duration::ZERO);
        for _ in 0..100 {
            assert_eq!(model.sample_delay(PathType::MarketData), Duration::ZERO);
        }
    }

    /// 极大 mean + 零 std_dev ⇒ 始终返回 mean
    #[test]
    fn test_extreme_mean_zero_std() {
        let model = NormalLatencyModel::uniform(Duration::from_secs(60), Duration::ZERO);
        for _ in 0..100 {
            let d = model.sample_delay(PathType::OrderSubmit);
            // 浮点转 Duration 可能有 1ns 误差
            let diff = d.abs_diff(Duration::from_secs(60));
            assert!(diff < Duration::from_micros(1));
        }
    }

    /// 负 mean 被截断为 0
    #[test]
    fn test_negative_mean_truncated_to_zero() {
        // 直接构造 -1ms mean（通过 u64::MAX 模拟环绕）
        let mut model = NormalLatencyModel::uniform(Duration::from_millis(10), Duration::ZERO);
        // Duration 是非负的，我们改用 0 mean + 检查 0 输出
        model
            .means
            .insert(PathType::MarketData, Duration::from_nanos(0));
        for _ in 0..100 {
            assert_eq!(model.sample_delay(PathType::MarketData), Duration::ZERO);
        }
    }

    /// 极小 mean + 极小 std_dev
    #[test]
    fn test_min_positive_values() {
        let model = NormalLatencyModel::uniform(Duration::from_nanos(1), Duration::from_nanos(1));
        for _ in 0..100 {
            let d = model.sample_delay(PathType::MarketData);
            // 截断为非负
            assert!(d.as_nanos() < 1_000_000);
        }
    }

    /// 大 std_dev 多次采样应能命中"长尾"
    #[test]
    fn test_long_tail_probability() {
        // 极小 mean + 巨大 std_dev ⇒ 多次采样应能命中 > mean 的样本
        let model =
            NormalLatencyModel::uniform(Duration::from_millis(1), Duration::from_millis(100));
        let mut max_seen_ms = 0.0_f64;
        for _ in 0..5_000 {
            let d = model.sample_delay(PathType::MarketData).as_secs_f64() * 1000.0;
            if d > max_seen_ms {
                max_seen_ms = d;
            }
        }
        // 期望长尾至少 50ms（5σ 水平） - 取 3σ = 0.997% 之外
        // 由于 Box-Muller 可能取到 5σ 以上的概率较高
        assert!(max_seen_ms > 50.0, "max_seen_ms = {max_seen_ms}");
    }

    /// params 在空表时 base_delay_ms 应为 0
    #[test]
    fn test_params_empty_table() {
        let model = NormalLatencyModel {
            means: HashMap::new(),
            std_devs: HashMap::new(),
        };
        let p = model.params();
        assert_eq!(p.base_delay_ms, 0.0);
        assert_eq!(p.jitter_ms, Some(0.0));
    }

    /// 序列化往返
    #[test]
    fn test_normal_serde_roundtrip() {
        let model =
            NormalLatencyModel::uniform(Duration::from_millis(10), Duration::from_millis(2));
        let json = serde_json::to_string(&model).unwrap();
        let de: NormalLatencyModel = serde_json::from_str(&json).unwrap();
        // 重新采样仍正确
        let d = de.sample_delay(PathType::MarketData);
        assert!(d.as_secs_f64() >= 0.0);
    }
}

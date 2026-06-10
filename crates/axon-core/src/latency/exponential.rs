//! 指数分布延迟模型
//!
//! 适合建模排队等待延迟（无记忆性，长尾）。
//! 采样公式：X = -ln(U) / λ，其中 U ~ Uniform(0, 1)。

use std::collections::HashMap;
use std::time::Duration;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use super::traits::{LatencyModel, LatencyParams, PathType};

/// 指数分布延迟模型
///
/// 各路径维护速率参数 `rate`（λ = 1/mean），单位为 1/秒。
/// 采样后转换为毫秒，固定为非负。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub struct ExponentialLatencyModel {
    /// 各路径类型的速率参数 λ（1/秒）
    pub rates: HashMap<PathType, f64>,
}

impl ExponentialLatencyModel {
    /// 创建统一指数延迟模型
    pub fn uniform(rate: f64) -> Self {
        let mut rates = HashMap::new();
        for path in PathType::ALL {
            rates.insert(path, rate);
        }
        Self { rates }
    }

    /// 通过均值（毫秒）创建
    pub fn from_mean_ms(mean_ms: f64) -> Self {
        // mean_ms 毫秒 → mean_sec 秒 → rate = 1/mean_sec
        let rate = if mean_ms > 0.0 { 1000.0 / mean_ms } else { 1.0 };
        Self::uniform(rate)
    }
}

impl LatencyModel for ExponentialLatencyModel {
    fn sample_delay(&self, path: PathType) -> Duration {
        let rate = self.rates.get(&path).copied().unwrap_or(1.0);
        // 防御：rate 必须 > 0
        let rate = if rate > 0.0 { rate } else { 1.0 };

        // 指数分布逆变换采样（毫秒）
        // 防止 U=0 导致 ln(0) = -inf
        let u = rand::random::<f64>().max(f64::MIN_POSITIVE);
        let sample_ms = -u.ln() / rate * 1000.0;

        Duration::from_secs_f64((sample_ms / 1000.0).max(0.0))
    }

    fn name(&self) -> &str {
        "exponential"
    }

    fn params(&self) -> LatencyParams {
        let count = self.rates.len() as f64;
        let sum: f64 = self.rates.values().sum();
        let avg_rate = if count > 0.0 { sum / count } else { 1.0 };
        let mean_ms = 1.0 / avg_rate.max(f64::MIN_POSITIVE) * 1000.0;
        LatencyParams {
            model_type: "exponential".to_string(),
            base_delay_ms: mean_ms,
            // 指数分布 std = mean
            jitter_ms: Some(mean_ms),
            path_overrides: self
                .rates
                .iter()
                .map(|(k, v)| (*k, 1.0 / v.max(f64::MIN_POSITIVE) * 1000.0))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_samples_non_negative() {
        let model = ExponentialLatencyModel::uniform(100.0);
        for _ in 0..2000 {
            let d = model.sample_delay(PathType::MarketData);
            assert!(d.as_secs_f64() >= 0.0);
        }
    }

    #[test]
    fn test_exponential_mean_approximate() {
        // rate=100 → mean = 1/100 s = 10ms
        let model = ExponentialLatencyModel::uniform(100.0);
        let n = 20_000;
        let sum: f64 = (0..n)
            .map(|_| model.sample_delay(PathType::MarketData).as_secs_f64() * 1000.0)
            .sum();
        let mean = sum / n as f64;
        assert!((mean - 10.0).abs() < 1.0, "expected mean ≈ 10ms, got {mean}");
    }

    #[test]
    fn test_exponential_from_mean_ms() {
        let model = ExponentialLatencyModel::from_mean_ms(5.0);
        let n = 20_000;
        let sum: f64 = (0..n)
            .map(|_| model.sample_delay(PathType::MarketData).as_secs_f64() * 1000.0)
            .sum();
        let mean = sum / n as f64;
        assert!((mean - 5.0).abs() < 0.5, "expected mean ≈ 5ms, got {mean}");
    }

    #[test]
    fn test_exponential_zero_rate_falls_back() {
        // rate=0 应回退到 rate=1（避免除零）
        let model = ExponentialLatencyModel::uniform(0.0);
        let d = model.sample_delay(PathType::OrderSubmit);
        // 1/1 秒 = 1000ms 量级
        assert!(d.as_secs_f64() >= 0.0);
        assert!(d.as_secs_f64() < 10.0);
    }

    #[test]
    fn test_name_and_params() {
        let model = ExponentialLatencyModel::uniform(100.0);
        assert_eq!(model.name(), "exponential");
        let p = model.params();
        assert_eq!(p.model_type, "exponential");
        assert!((p.base_delay_ms - 10.0).abs() < 1e-6);
        // std == mean
        assert!((p.jitter_ms.expect("jitter") - 10.0).abs() < 1e-6);
    }
}
